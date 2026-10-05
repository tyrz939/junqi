//! The content compile (ARCHITECTURE.md §6): parse, convert units, intern, check, emit.
//!
//! `build` is what `jane-data`'s build script and the `dev-data` loader both call, so the
//! compiled statics and a startup load cannot differ; `jane check` calls it without the emit.

pub mod ctx;
pub mod diag;
pub mod fraction;
pub mod integrate;
pub mod lists;
pub mod source;
pub mod tables;

use std::path::Path;

use crate::emit::{Emit, PRELUDE};
use crate::model::{Atmosphere, Catalog, DungeonThemes, Looks, TileLooks};
use ctx::{Ctx, Ids, leak, leak_str};
use diag::Diagnostics;
use source::Source;

/// The result of a compile: the catalog, if there were no errors, and every diagnostic.
#[derive(Debug)]
pub struct Built {
    pub catalog: Option<&'static Catalog>,
    /// The looks table (`data/looks`), a static of its own beside the catalog: looks change no
    /// behaviour, so they stay out of the content hash. Empty when the compile failed.
    pub looks: Looks,
    /// The terrain's looks (`data/looks/tiles.json`), a static of their own for the same reason.
    pub tile_looks: TileLooks,
    /// The atmosphere's layers (`data/atmosphere.json`), presentation only: a static of its own.
    pub atmosphere: Atmosphere,
    /// The dungeon themes (`data/looks/dungeon_themes.json`), presentation only: a static of its own.
    pub dungeon_themes: DungeonThemes,
    pub diag: Diagnostics,
}

/// Compile the data dir at `root`.
pub fn build(root: &Path) -> Built {
    let mut diag = Diagnostics::default();
    let src = Source::read(root, &mut diag);
    let mut built = build_source(&src);
    diag.errors.append(&mut built.diag.errors);
    diag.warnings.append(&mut built.diag.warnings);
    let ok = diag.is_ok();
    Built {
        catalog: if ok { built.catalog } else { None },
        looks: if ok { built.looks } else { &[] },
        tile_looks: if ok { built.tile_looks } else { TileLooks::EMPTY },
        atmosphere: if ok { built.atmosphere } else { Atmosphere::EMPTY },
        dungeon_themes: if ok { built.dungeon_themes } else { DungeonThemes::EMPTY },
        diag,
    }
}

/// Compile an already-read source.
pub fn build_source(src: &Source) -> Built {
    let mut cx = Ctx::default();
    cx.ids = Ids::collect(src, &mut cx.diag);
    let mut combat = tables::combat::compile(src, &mut cx);
    let story = tables::story::compile(src, &mut cx);
    let mut county = tables::county::compile(src, &mut cx);
    let dungeons = tables::dungeons::compile(src, &mut cx);
    let chunks = tables::chunks::compile(src, &mut cx, &story, &county, &dungeons);
    // What names people and places by the hour comes last, so nothing it names moves a name
    // the groups above interned: the unit rows' hours, the stories' listeners, the living world.
    combat.units = tables::combat::late_schedules(src, &mut cx, combat.units);
    county.stories = tables::county::late_spreads(src, &mut cx, county.stories);
    let living = tables::living::compile(src, &mut cx, &county);
    // What another can be had of, once everything that gives one is known.
    combat.items = tables::combat::late_replaceable(&cx, &combat, &living, &county);
    check_limits(&mut cx);
    let looks = tables::looks::compile(src, &mut cx);
    let tile_looks = tables::tile_looks::compile(src, &mut cx);
    let atmosphere = tables::atmosphere::compile(src, &mut cx);
    let dungeon_themes = tables::dungeon_themes::compile(src, &mut cx);

    let mut catalog = Catalog {
        content_hash: 0,
        names: leak(cx.names.strings().map(leak_str).collect()),
        texts: leak(cx.texts.strings().map(leak_str).collect()),
        sprites: leak(cx.sprites.strings().map(leak_str).collect()),
        lists: leak(cx.lists.drain(..).map(leak).collect()),
        conds: leak(cx.conds.drain(..).map(leak).collect()),
        name_lists: leak(cx.name_lists.drain(..).map(leak).collect()),
        combat,
        story,
        county,
        dungeons,
        chunks,
        living,
    };
    integrate::check(&catalog, &mut cx.diag);
    catalog.content_hash = content_hash(&catalog);
    let ok = cx.diag.is_ok();
    Built {
        catalog: ok.then(|| &*Box::leak(Box::new(catalog))),
        looks: if ok { looks } else { &[] },
        tile_looks: if ok { tile_looks } else { TileLooks::EMPTY },
        atmosphere: if ok { atmosphere } else { Atmosphere::EMPTY },
        dungeon_themes: if ok { dungeon_themes } else { DungeonThemes::EMPTY },
        diag: cx.diag,
    }
}

/// Every pool is indexed by a `u16`.
fn check_limits(cx: &mut Ctx) {
    let pools = [
        ("names", cx.names.len()),
        ("texts", cx.texts.len()),
        ("sprites", cx.sprites.len()),
        ("lists", cx.lists.len()),
        ("condition lists", cx.conds.len()),
        ("name lists", cx.name_lists.len()),
    ];
    for (what, n) in pools {
        cx.diag.need(u16::try_from(n).is_ok(), "catalog", format!("{n} {what}: more than a u16 can index"));
    }
    // The town's news is kept under story ids counted down from the top (`living::news_key`).
    let (stories, news) = (cx.ids.stories.len(), cx.ids.consequences.len());
    cx.diag.need(
        stories + news < usize::from(u16::MAX),
        "catalog",
        "stories and consequences share the story id range",
    );
}

/// xxh3 of the emitted catalog with the English left out: everything that changes behaviour.
pub fn content_hash(c: &Catalog) -> u64 {
    let behaviour = Catalog { content_hash: 0, texts: &[], ..*c };
    xxhash_rust::xxh3::xxh3_64(crate::emit::to_rust(&behaviour).as_bytes())
}

/// The Rust source of `pub static LOOKS`, which follows the catalog in the same file.
pub fn codegen_looks(looks: Looks) -> String {
    let mut out = String::with_capacity(1 << 16);
    out.push_str(
        "
pub static LOOKS: Looks = ",
    );
    looks.emit(&mut out);
    out.push_str(
        ";
",
    );
    out
}

/// The Rust source of `pub static CATALOG`.
pub fn codegen(c: &Catalog) -> String {
    let mut out = String::with_capacity(1 << 22);
    out.push_str("// Generated by jane-schema from /data. Do not edit.\n");
    out.push_str(PRELUDE);
    out.push_str("\npub static CATALOG: Catalog = ");
    c.emit(&mut out);
    out.push_str(";\n");
    out
}

/// The Rust source of `pub static ATMOSPHERE`, which follows the tile looks in the same file.
pub fn codegen_atmosphere(a: Atmosphere) -> String {
    let mut out = String::from("\npub static ATMOSPHERE: Atmosphere = ");
    a.emit(&mut out);
    out.push_str(";\n");
    out
}

/// The Rust source of `pub static DUNGEON_THEMES`, which follows the atmosphere in the same file.
pub fn codegen_dungeon_themes(t: DungeonThemes) -> String {
    let mut out = String::from(
        "
pub static DUNGEON_THEMES: DungeonThemes = ",
    );
    t.emit(&mut out);
    out.push_str(
        ";
",
    );
    out
}

/// The Rust source of `pub static TILE_LOOKS`, which follows the looks in the same file.
pub fn codegen_tile_looks(looks: TileLooks) -> String {
    let mut out = String::from("\npub static TILE_LOOKS: TileLooks = ");
    looks.emit(&mut out);
    out.push_str(";\n");
    out
}
