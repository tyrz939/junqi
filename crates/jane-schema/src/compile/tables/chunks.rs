//! Compile the county's authored places, `data/chunks/*.chunk` (PORT.md §6.f), and their tuning,
//! `data/tuning/chunks.json`, into [`model::Chunks`].
//!
//! The format and the parser are `chunks/parse.rs`; the layout and the lint that needs only the
//! chunk and the defs are `chunks/lint.rs`. Here: each chunk stands at a site of `sites.json` and
//! its file is named for it; every prop's JSON is typed (`deny_unknown_fields`) and its lists go
//! through `compile::lists`; a door leads to a mark its zone provides; every fill has a tuning row;
//! and no name a chunk exports is exported by another chunk or provided by another county table
//! (two props with one key in one blueprint).

mod lint;
mod parse;
#[cfg(test)]
mod tests;

use serde::Deserialize;

use jane_core::action::Stack;
use jane_core::grid::{Cell, Rect};
use jane_core::ids::{NameId, ZoneId};
use jane_core::num::Tick;

use crate::compile::ctx::{Ctx, Ids, leak, leak_str};
use crate::compile::lists::{self, RawAction};
use crate::compile::source::{Row, Source, typed};
use crate::model::{
    self, ChunkAround, ChunkDef, ChunkDoorTo, ChunkFill, ChunkFillDef, ChunkGate, ChunkMark, ChunkPin, ChunkProp,
    ChunkRect, ChunkSide, ChunkSlot, ChunkTuning, ChunkUnit, ChunkWaypoint, MissionNameWhat, NameKind, PropDef,
    PropTemplate, ProvidedBy,
};

pub use lint::{Facts, Laid, Placed, lay_out, tile_named};
pub use parse::{Chunk, Entry, Layer, LineError, Side, parse};

const TUNING: &str = "tuning/chunks.json";

fn at_line(file: &str, line: usize) -> String {
    if line == 0 { file.to_owned() } else { format!("{file}:{line}") }
}

/// The lint's view of the catalog: prop footprints and solidity from the compiled props, unit ids.
struct CatalogFacts<'a> {
    ids: &'a Ids,
    props: &'a [PropDef],
}

impl Facts for CatalogFacts<'_> {
    fn prop(&self, def: &str) -> Option<(i32, i32, bool)> {
        let p = self.props.get(usize::from(self.ids.props.get(def)?))?;
        Some((i32::from(p.w), i32::from(p.h), p.solid))
    }

    fn unit(&self, def: &str) -> bool {
        self.ids.units.get(def).is_some()
    }
}

// --- tuning ----------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTuning {
    box_margin: u16,
    #[serde(default)]
    fills: Vec<RawFill>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFill {
    id: String,
    def: String,
    count: u8,
    tries: u8,
    margin: u8,
}

fn tuning(src: &Source, cx: &mut Ctx) -> ChunkTuning {
    let mut out = ChunkTuning { box_margin: 8, fills: &[] };
    let Some(v) = src.file(TUNING) else {
        cx.diag.error(TUNING, "missing: the chunks' tuning (box margin, fills)");
        return out;
    };
    let row = Row { file: TUNING.to_owned(), value: v.clone() };
    let Some(raw) = typed::<RawTuning>(&row, "tuning", &mut cx.diag) else { return out };
    out.box_margin = raw.box_margin;
    let mut fills = Vec::new();
    for (n, f) in raw.fills.iter().enumerate() {
        let at = format!("{TUNING}: fills.{}", f.id);
        cx.diag.need(!raw.fills[..n].iter().any(|g| g.id == f.id), &at, "a fill id given twice");
        cx.diag.need(f.count >= 1 && f.tries >= 1, &at, "count and tries are at least 1");
        if let Some(def) = cx.prop(&at, &f.def) {
            fills.push(ChunkFillDef { id: leak_str(&f.id), def, count: f.count, tries: f.tries, margin: f.margin });
        }
    }
    cx.diag.need(u8::try_from(fills.len()).is_ok(), TUNING, "more fills than a u8 indexes");
    out.fills = leak(fills);
    out
}

// --- a prop's fields -------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStack {
    item: String,
    qty: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTo {
    zone: String,
    mark: String,
}

/// A `PropSpawn`'s fields other than its def, key and cell.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawExtra {
    #[serde(default)]
    locked: bool,
    key_tag: Option<String>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    on: bool,
    to: Option<RawTo>,
    #[serde(default)]
    loot: Vec<RawStack>,
    #[serde(rename = "use")]
    use_list: Option<Vec<RawAction>>,
    release: Option<Vec<RawAction>>,
    #[serde(default)]
    needs: Vec<RawStack>,
    talk: Option<String>,
    label: Option<String>,
    night_lock: Option<String>,
    night_hours: Option<[u8; 2]>,
}

/// English, interned, held to VOICE.md's mechanical rules.
fn say(cx: &mut Ctx, at: &str, s: &str) -> jane_core::TextId {
    cx.diag.need(!s.contains(['\u{2013}', '\u{2014}']), at, format!("an en or em dash in \"{s}\" (VOICE.md)"));
    cx.diag.need(!s.contains("Jane"), at, format!("\"Jane\" is baked into \"{s}\"; the player names her"));
    cx.text(s)
}

fn stacks(cx: &mut Ctx, at: &str, raw: &[RawStack]) -> &'static [Stack] {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        cx.diag.need(s.qty >= 1, at, format!("{} x{}: qty < 1", s.item, s.qty));
        if let Some(item) = cx.item(at, &s.item) {
            out.push(Stack { item, qty: s.qty });
        }
    }
    leak(out)
}

/// Does a zone provide this mark: its `zones.json` contract, or its mission's names?
fn zone_has_mark(county: &model::County, dungeons: &model::Dungeons, zone: ZoneId, mark: NameId) -> bool {
    let contract = county.zones.get(zone.index()).is_some_and(|z| z.contract.marks.contains(&mark));
    let mission = dungeons
        .mission_of(zone)
        .is_some_and(|m| m.provides.iter().any(|n| n.name == mark && n.what == MissionNameWhat::Mark));
    contract || mission
}

struct Groups<'a> {
    county: &'a model::County,
    dungeons: &'a model::Dungeons,
}

fn prop(
    cx: &mut Ctx,
    g: &Groups<'_>,
    at: &str,
    def: &str,
    extra: Option<&str>,
) -> Option<(PropTemplate, Option<ChunkDoorTo>)> {
    let raw: RawExtra = match extra {
        None => RawExtra::default(),
        Some(json) => match serde_json::from_str(json) {
            Ok(r) => r,
            Err(e) => {
                cx.diag.error(at, format!("prop {def}: {e}"));
                return None;
            }
        },
    };
    let to = match &raw.to {
        None => None,
        Some(t) => {
            let zone = cx.zone(at, &t.zone)?;
            let mark = cx.name(&t.mark);
            cx.diag.need(zone != ZoneId::County, at, "a door out of the county into the county");
            cx.diag.need(
                zone_has_mark(g.county, g.dungeons, zone, mark),
                at,
                format!("leads to \"{}\" in {}, which provides no such mark", t.mark, t.zone),
            );
            Some(ChunkDoorTo { zone, mark })
        }
    };
    let t = PropTemplate {
        def: cx.prop(at, def)?,
        locked: raw.locked,
        key_tag: raw.key_tag.as_deref().map(|t| cx.name(t)),
        hidden: raw.hidden,
        on: raw.on,
        loot: stacks(cx, &format!("{at}.loot"), &raw.loot),
        use_list: lists::list(cx, &format!("{at}.use"), raw.use_list.as_deref()),
        release: lists::list(cx, &format!("{at}.release"), raw.release.as_deref()),
        needs: stacks(cx, &format!("{at}.needs"), &raw.needs),
        talk: raw.talk.as_deref().and_then(|t| cx.dialogue(at, t)),
        label: raw.label.as_deref().map(|t| say(cx, at, t)),
        night_lock: crate::compile::tables::county::night_lock(cx, at, raw.night_lock.as_deref(), raw.night_hours),
    };
    Some((t, to))
}

// --- one chunk -------------------------------------------------------------------------------

fn side(s: Side) -> ChunkSide {
    match s {
        Side::N => ChunkSide::N,
        Side::E => ChunkSide::E,
        Side::S => ChunkSide::S,
        Side::W => ChunkSide::W,
    }
}

fn u16_of(v: i32) -> u16 {
    u16::try_from(v).unwrap_or(0)
}

fn cell(x: i32, y: i32) -> Cell {
    Cell::new(u16_of(x), u16_of(y))
}

/// A row-major mask as row runs (`h` 1), in reading order.
fn runs(w: i32, h: i32, on: impl Fn(i32, i32) -> bool) -> Vec<Rect> {
    let mut out = Vec::new();
    for y in 0..h {
        let mut x = 0;
        while x < w {
            if on(x, y) {
                let x0 = x;
                while x < w && on(x, y) {
                    x += 1;
                }
                out.push(Rect::new(x0, y, x - x0, 1));
            } else {
                x += 1;
            }
        }
    }
    out
}

fn chunk(cx: &mut Ctx, g: &Groups<'_>, tune: &ChunkTuning, c: &Chunk, laid: &Laid) -> Option<ChunkDef> {
    let file = format!("data/{}", c.file);
    let at = |line: usize| at_line(&file, line);
    let site = g.county.site_ix(&c.id);
    if site.is_none() {
        cx.diag.error(at(c.head_line("id")), format!("\"{}\" is no site in sites.json", c.id));
    }

    // The ground: a palette in first-seen order, and an index per cell.
    let mut palette: Vec<jane_core::Tile> = Vec::new();
    let mut cells = Vec::with_capacity(laid.tiles.len());
    for t in &laid.tiles {
        let i = palette.iter().position(|p| p == t).unwrap_or_else(|| {
            palette.push(*t);
            palette.len() - 1
        });
        cells.push(i as u8);
    }
    cx.diag.need(palette.len() <= 256, at(0), "more than 256 tiles");

    let mut props = Vec::new();
    for p in &laid.props {
        let l = &c.legend[p.entry];
        let Entry::Prop { def, key, extra } = &l.entry else { continue };
        let Some((prop, to)) = prop(cx, g, &at(l.line), def, extra.as_deref()) else { continue };
        props.push(ChunkProp { key: key.as_deref().map(|k| cx.name(k)), at: cell(p.x, p.y), prop, to });
    }
    let mut units = Vec::new();
    for p in &laid.units {
        let l = &c.legend[p.entry];
        let Entry::Unit { def, key, facing, route } = &l.entry else { continue };
        let Some(def) = cx.unit(&at(l.line), def) else { continue };
        let route: Vec<ChunkWaypoint> =
            route.iter().map(|&(x, y, t)| ChunkWaypoint { at: cell(x, y), dwell: t.map(Tick) }).collect();
        units.push(ChunkUnit {
            key: key.as_deref().map(|k| cx.name(k)),
            def,
            at: cell(p.x, p.y),
            facing: *facing,
            route: leak(route),
        });
    }
    let mut marks = Vec::new();
    for p in &laid.marks {
        if let Entry::Mark { name, facing } = &c.legend[p.entry].entry {
            marks.push(ChunkMark { name: cx.name(name), at: cell(p.x, p.y), facing: *facing });
        }
    }
    let mut rects = Vec::new();
    for p in &laid.rects {
        if let Entry::Rect(name) = &c.legend[p.entry].entry {
            rects.push(ChunkRect { name: cx.name(name), rect: Rect::new(p.x, p.y, p.w, p.h) });
        }
    }
    let mut fills = Vec::new();
    for (entry, at_cells) in &laid.fills {
        let l = &c.legend[*entry];
        let Entry::Fill(id) = &l.entry else { continue };
        let Some(ix) = tune.fills.iter().position(|f| f.id == id) else {
            cx.diag.error(at(l.line), format!("fill \"{id}\" has no row in {TUNING}"));
            continue;
        };
        let on = |x: i32, y: i32| at_cells.contains(&(x, y));
        fills.push(ChunkFill { fill: ix as u8, cells: leak(runs(c.w, c.h, on)) });
    }
    let claims = runs(c.w, c.h, |x, y| laid.claims[(y * c.w + x) as usize]);

    let gates: Vec<ChunkGate> = c.gates.iter().map(|&(s, a)| ChunkGate { side: side(s), along: u16_of(a) }).collect();
    let slots: Vec<ChunkSlot> =
        c.slots.iter().map(|(n, x, y)| ChunkSlot { name: cx.name(&n.name), at: cell(*x, *y) }).collect();
    let around: Vec<ChunkAround> = c
        .around
        .iter()
        .map(|(n, m)| ChunkAround {
            name: cx.name(&n.name),
            n: u16_of(m[0]),
            e: u16_of(m[1]),
            s: u16_of(m[2]),
            w: u16_of(m[3]),
        })
        .collect();
    Some(ChunkDef {
        id: leak_str(&c.id),
        site: site?,
        w: u16_of(c.w),
        h: u16_of(c.h),
        anchor: cell(c.anchor.0, c.anchor.1),
        pin: c.pin.map(|(s, a)| ChunkPin { side: side(s), at: u16_of(a) }),
        face: side(c.face),
        gates: leak(gates),
        slots: leak(slots),
        around: leak(around),
        palette: leak(palette),
        cells: leak(cells),
        claims: leak(claims),
        props: leak(props),
        units: leak(units),
        marks: leak(marks),
        rects: leak(rects),
        fills: leak(fills),
    })
}

/// No name is exported twice: by two chunks, or by a chunk and another county table.
fn once_each(cx: &mut Ctx, county: &model::County, defs: &[ChunkDef]) {
    let kind_word = |k: NameKind| match k {
        NameKind::Unit => "unit",
        NameKind::Prop => "prop",
        NameKind::Mark => "mark",
        NameKind::Rect => "rect",
        NameKind::Area => "area",
    };
    let mut seen: Vec<(NameKind, NameId, &str)> = Vec::new();
    for d in defs {
        for (k, n) in d.exports() {
            if let Some((_, _, other)) = seen.iter().find(|(sk, sn, _)| *sk == k && *sn == n) {
                let what = format!("{} \"{}\"", kind_word(k), cx.names.strings().nth(n.index()).unwrap_or_default());
                cx.diag.error(format!("data/chunks/{}.chunk", d.id), format!("{what} is also drawn by {other}"));
            }
            seen.push((k, n, d.id));
        }
    }
    for p in county.provides() {
        if p.by == ProvidedBy::Zones || p.zone != ZoneId::County {
            continue;
        }
        if let Some((_, _, id)) = seen.iter().find(|(k, n, _)| *k == p.kind && *n == p.name) {
            let what =
                format!("{} \"{}\"", kind_word(p.kind), cx.names.strings().nth(p.name.index()).unwrap_or_default());
            cx.diag.error(format!("data/chunks/{id}.chunk"), format!("{what} is also provided by {:?}", p.by));
        }
    }
}

pub fn compile(
    src: &Source,
    cx: &mut Ctx,
    story: &model::Story,
    county: &model::County,
    dungeons: &model::Dungeons,
) -> model::Chunks {
    let files: Vec<(String, String)> =
        src.texts_in("chunks", "chunk").map(|(f, t)| (f.to_owned(), t.to_owned())).collect();
    if files.is_empty() && src.file(TUNING).is_none() {
        return model::Chunks::EMPTY;
    }
    let tune = tuning(src, cx);
    let g = Groups { county, dungeons };
    let mut defs = Vec::new();
    for (file, text) in &files {
        let at = |line: usize| at_line(&format!("data/{file}"), line);
        let c = match parse(text, file) {
            Ok(c) => c,
            Err((line, msg)) => {
                cx.diag.error(at(line), msg);
                continue;
            }
        };
        let stem = file.trim_start_matches("chunks/").trim_end_matches(".chunk");
        cx.diag.need(stem == c.id, at(c.head_line("id")), format!("id \"{}\" in a file named {stem}", c.id));
        let facts = CatalogFacts { ids: &cx.ids, props: story.props };
        let (laid, errs) = lay_out(&c, &facts);
        let ok = errs.is_empty();
        for (line, msg) in errs {
            cx.diag.error(at(line), msg);
        }
        if !ok {
            continue;
        }
        if let Some(d) = chunk(cx, &g, &tune, &c, &laid) {
            defs.push(d);
        }
    }
    for f in tune.fills {
        let used = defs.iter().flat_map(|d| d.fills).any(|u| tune.fills[usize::from(u.fill)].id == f.id);
        cx.diag.need(used, TUNING, format!("fill \"{}\" is drawn by no chunk", f.id));
    }
    once_each(cx, county, &defs);
    model::Chunks { defs: leak(defs), tuning: tune }
}
