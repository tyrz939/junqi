//! Compile the terrain's looks (ART.md §2.6) into [`model::TileLooks`]: `data/looks/tiles.json` (and any
//! `data/looks/tiles/*.json` beside it): one [`TileStyle`] per sim tile and one per render-only
//! material, keyed by the tile's or material's name in snake case (`grass_tall`, `roof_slate`),
//! its fields in camel case like every table's (`wallLike`).
//!
//! The checks: a key names a tile or a material; every tile and every material has a row (a
//! thing with no look is a build error, ART.md §5); `inherit` names a tile; a `flora` row has an
//! `inherit`; `wet` is 0, 1 or 2. That every `ramp` names a palette ramp is `jane-art`'s test: the
//! palette lives there.
//!
//! A source with no `looks/tiles.json` at all (a fixture of another group) compiles to no looks.
//! The person looks' compile (`looks.rs`) reads every other file under `data/looks`.

use serde::Deserialize;

use jane_core::{Material, Tile};

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::source::{Source, typed};
use crate::model::{self, TileGroup, TileHeight, TileNormal, TilePattern, TileStyle};

/// The render-only materials, in their order (PORT.md §6.i).
const MATERIALS: [Material; 4] = [Material::RoofSlate, Material::RoofThatch, Material::BrickWall, Material::Pine];

/// `GrassTall` as `grass_tall`.
pub fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn tile_named(name: &str) -> Option<Tile> {
    Tile::ALL.iter().copied().find(|t| snake(t.name()) == name)
}

fn material_named(name: &str) -> Option<Material> {
    MATERIALS.iter().copied().find(|m| snake(&format!("{m:?}")) == name)
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawGroup {
    Ground,
    Water,
    Wall,
    Roof,
    Flora,
    Made,
}

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RawHeight {
    #[default]
    Flat,
    Raised,
    Canopy,
}

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RawNormal {
    #[default]
    Flat,
    Cliff,
    Wall,
    Roof,
    Water,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawPattern {
    Void,
    Turf,
    Earth,
    Gravel,
    Sand,
    Marsh,
    Cracked,
    Soil,
    Crops,
    Setts,
    Water,
    Ice,
    Cliff,
    Plaster,
    Brick,
    RoofTile,
    Slate,
    Thatch,
    Slabs,
    Boards,
    Block,
    Rock,
    RockFloor,
    Rail,
    Sill,
    Glass,
    Hedge,
    Boardwalk,
    Tuft,
    Flowers,
    Stepping,
    Tree,
    Pine,
    DeadTree,
    Bush,
    Rubble,
    Fence,
    StoneWall,
    Timbered,
    Crypt,
    Ironwork,
    Panelled,
    Pipework,
    Wainscot,
    Parquet,
    Plates,
    Flags,
    Grating,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawStyle {
    group: RawGroup,
    #[serde(default)]
    inherit: Option<String>,
    #[serde(default)]
    height: RawHeight,
    #[serde(default)]
    rise: u8,
    ramp: String,
    #[serde(default)]
    accent: String,
    pattern: RawPattern,
    #[serde(default)]
    detail: u8,
    #[serde(default)]
    normal: RawNormal,
    #[serde(default)]
    wet: u8,
    #[serde(default)]
    wall_like: bool,
    #[serde(default)]
    wild: bool,
}

fn group(g: RawGroup) -> TileGroup {
    match g {
        RawGroup::Ground => TileGroup::Ground,
        RawGroup::Water => TileGroup::Water,
        RawGroup::Wall => TileGroup::Wall,
        RawGroup::Roof => TileGroup::Roof,
        RawGroup::Flora => TileGroup::Flora,
        RawGroup::Made => TileGroup::Made,
    }
}

fn height(h: RawHeight) -> TileHeight {
    match h {
        RawHeight::Flat => TileHeight::Flat,
        RawHeight::Raised => TileHeight::Raised,
        RawHeight::Canopy => TileHeight::Canopy,
    }
}

fn normal(n: RawNormal) -> TileNormal {
    match n {
        RawNormal::Flat => TileNormal::Flat,
        RawNormal::Cliff => TileNormal::Cliff,
        RawNormal::Wall => TileNormal::Wall,
        RawNormal::Roof => TileNormal::Roof,
        RawNormal::Water => TileNormal::Water,
    }
}

fn pattern(p: RawPattern) -> TilePattern {
    use RawPattern as R;
    use TilePattern as P;
    match p {
        R::Void => P::Void,
        R::Turf => P::Turf,
        R::Earth => P::Earth,
        R::Gravel => P::Gravel,
        R::Sand => P::Sand,
        R::Marsh => P::Marsh,
        R::Cracked => P::Cracked,
        R::Soil => P::Soil,
        R::Crops => P::Crops,
        R::Setts => P::Setts,
        R::Water => P::Water,
        R::Ice => P::Ice,
        R::Cliff => P::Cliff,
        R::Plaster => P::Plaster,
        R::Brick => P::Brick,
        R::RoofTile => P::RoofTile,
        R::Slate => P::Slate,
        R::Thatch => P::Thatch,
        R::Slabs => P::Slabs,
        R::Boards => P::Boards,
        R::Block => P::Block,
        R::Rock => P::Rock,
        R::RockFloor => P::RockFloor,
        R::Rail => P::Rail,
        R::Sill => P::Sill,
        R::Glass => P::Glass,
        R::Hedge => P::Hedge,
        R::Boardwalk => P::Boardwalk,
        R::Tuft => P::Tuft,
        R::Flowers => P::Flowers,
        R::Stepping => P::Stepping,
        R::Tree => P::Tree,
        R::Pine => P::Pine,
        R::DeadTree => P::DeadTree,
        R::Bush => P::Bush,
        R::Rubble => P::Rubble,
        R::Fence => P::Fence,
        R::StoneWall => P::StoneWall,
        R::Timbered => P::Timbered,
        R::Crypt => P::Crypt,
        R::Ironwork => P::Ironwork,
        R::Panelled => P::Panelled,
        R::Pipework => P::Pipework,
        R::Wainscot => P::Wainscot,
        R::Parquet => P::Parquet,
        R::Plates => P::Plates,
        R::Flags => P::Flags,
        R::Grating => P::Grating,
    }
}

pub fn compile(src: &Source, cx: &mut Ctx) -> model::TileLooks {
    let table = "looks/tiles";
    if src.file("looks/tiles.json").is_none() {
        return model::TileLooks::EMPTY;
    }
    let rows = src.table(table, &mut cx.diag);
    let mut tiles: Vec<Option<TileStyle>> = vec![None; Tile::ALL.len()];
    let mut mats: Vec<Option<TileStyle>> = vec![None; MATERIALS.len()];
    for (key, row) in &rows {
        let at = format!("{}: {table}.{key}", row.file);
        let Some(r) = typed::<RawStyle>(row, &at, &mut cx.diag) else { continue };
        let inherit = match r.inherit.as_deref() {
            None => None,
            Some(n) => {
                let t = tile_named(n);
                cx.diag.need(t.is_some(), format!("{at}.inherit"), format!("no tile is called \"{n}\""));
                t
            }
        };
        cx.diag.need(r.wet <= 2, format!("{at}.wet"), "is 0 (matt), 1 (darkens) or 2 (darkens and shines)");
        let style = TileStyle {
            group: group(r.group),
            inherit,
            height: height(r.height),
            rise: r.rise,
            ramp: leak_str(&r.ramp),
            accent: leak_str(&r.accent),
            pattern: pattern(r.pattern),
            detail: r.detail,
            normal: normal(r.normal),
            wet: r.wet,
            wall_like: r.wall_like,
            wild: r.wild,
        };
        cx.diag.need(
            style.group != TileGroup::Flora || inherit.is_some(),
            format!("{at}.inherit"),
            "a flora row names the ground under it when no neighbour lends one",
        );
        if let Some(t) = tile_named(key) {
            tiles[Tile::ALL.iter().position(|&x| x == t).unwrap_or(0)] = Some(style);
        } else if let Some(m) = material_named(key) {
            mats[MATERIALS.iter().position(|&x| x == m).unwrap_or(0)] = Some(style);
        } else {
            cx.diag.error(&at, format!("\"{key}\" is neither a tile nor a material"));
        }
    }
    for (t, s) in Tile::ALL.iter().zip(&tiles) {
        cx.diag.need(s.is_some(), table, format!("tile \"{}\" has no look", snake(t.name())));
    }
    for (m, s) in MATERIALS.iter().zip(&mats) {
        cx.diag.need(s.is_some(), table, format!("material \"{}\" has no look", snake(&format!("{m:?}"))));
    }
    model::TileLooks {
        tiles: leak(Tile::ALL.iter().zip(tiles).filter_map(|(&t, s)| s.map(|s| (t, s))).collect()),
        materials: leak(MATERIALS.iter().zip(mats).filter_map(|(&m, s)| s.map(|s| (m, s))).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::diag::Diagnostics;

    fn build(json: &str) -> (model::TileLooks, Diagnostics) {
        let src = Source::from_files(&[("looks/tiles.json", json)]).unwrap();
        let mut cx = Ctx::default();
        let looks = compile(&src, &mut cx);
        (looks, cx.diag)
    }

    #[test]
    fn names_are_snake_case() {
        assert_eq!(snake("GrassTall"), "grass_tall");
        assert_eq!(tile_named("house_roof"), Some(Tile::HouseRoof));
        assert_eq!(material_named("roof_slate"), Some(Material::RoofSlate));
        assert_eq!(tile_named("roof_slate"), None);
    }

    #[test]
    fn a_missing_tile_an_unknown_key_and_a_bad_inherit_are_errors() {
        let (looks, d) = build(
            r#"{"grass": {"group": "ground", "ramp": "turf", "pattern": "turf", "wild": true},
                "tree": {"group": "flora", "inherit": "gras", "ramp": "leaf", "pattern": "tree"},
                "lawn": {"group": "ground", "ramp": "turf", "pattern": "turf"}}"#,
        );
        let all = d.errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        assert!(all.contains("no tile is called \"gras\""), "{all}");
        assert!(all.contains("\"lawn\" is neither"), "{all}");
        assert!(all.contains("tile \"water\" has no look"), "{all}");
        assert!(all.contains("material \"pine\" has no look"), "{all}");
        assert_eq!(looks.tile(Tile::Grass).map(|s| s.pattern), Some(TilePattern::Turf));
    }

    #[test]
    fn an_unknown_field_is_an_error_and_no_file_is_no_looks() {
        let (_, d) = build(r#"{"grass": {"group": "ground", "ramp": "turf", "pattern": "turf", "colour": 1}}"#);
        assert!(d.errors.iter().any(|e| e.msg.contains("unknown field")), "{:?}", d.errors);
        let src = Source::from_files(&[("items.json", "{}")]).unwrap();
        let mut cx = Ctx::default();
        assert_eq!(compile(&src, &mut cx), model::TileLooks::EMPTY);
        assert!(cx.diag.is_ok());
    }
}
