//! Compile the dungeon themes (ART.md §2.6.1) into [`model::DungeonThemes`]:
//! `data/looks/dungeon_themes.json`, keyed by theme id, its fields in camel case.
//!
//! The checks: every zone named is a zone, and no zone wears two themes; a motif repeats every
//! one cell or more; a lane's wear is -1, 0 or 1; spokes are 3 to 24; colours are `#rrggbb`.
//! That every ramp names a palette ramp is `jane-art`'s test: the palette lives there.
//!
//! A source with no `looks/dungeon_themes.json` compiles to no themes: its dungeons are drawn
//! unframed.

use serde::Deserialize;

use crate::compile::ctx::{Ctx, leak, leak_str};
use crate::compile::source::{Source, typed};
use crate::model::{
    self, DungeonTheme, FloorMotif, FloorMotifKind, ThemeBorder, ThemeCarving, ThemeEmblem, ThemeTrim, ThemeUpper,
    WallMotif, WallMotifKind,
};

/// The file, as the looks' compile must leave it out of the sprites' looks.
pub const FILE: &str = "looks/dungeon_themes.json";

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawTrim {
    Plain,
    Cap,
    Dentil,
    Frieze,
    Cornice,
    Beam,
    Course,
    Rail,
}

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RawUpper {
    #[default]
    Carry,
    Shelves,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawBorder {
    Edging,
    Lozenge,
    Inlay,
    Stripes,
    Kerb,
    Line,
    Pebbles,
}

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RawCarving {
    #[default]
    Plain,
    Angel,
    Lamp,
    Fluted,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawWall {
    Lamp,
    Niche,
    Ivy,
    Painting,
    Sign,
    Valve,
    Window,
    Radiator,
    Pegs,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawFloor {
    Rails,
    Channel,
    Belt,
    Gantry,
    Carpet,
    Moon,
    Heaps,
    Plinths,
    Stanchions,
    Chalk,
    Toadstools,
}

#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum RawEmblem {
    #[default]
    Plain,
    Wheel,
    Headframe,
    Angel,
    Furnace,
    Clock,
    Portrait,
    Rose,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWallMotif {
    motif: RawWall,
    every: u8,
    #[serde(default)]
    at: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFloorMotif {
    motif: RawFloor,
    #[serde(default)]
    every: u8,
    #[serde(default)]
    count: u8,
    #[serde(default)]
    ramp: String,
    #[serde(default)]
    gathers: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawBoss {
    inlay: String,
    #[serde(default = "twelve")]
    spokes: u8,
    #[serde(default = "eight")]
    set_spokes: u8,
    #[serde(default = "four")]
    braziers: u8,
    #[serde(default)]
    emblem: RawEmblem,
    #[serde(default)]
    emblem_ramps: [String; 2],
}

const fn twelve() -> u8 {
    12
}

const fn four() -> u8 {
    4
}

const fn eight() -> u8 {
    8
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawGrade {
    tint: String,
    lift: String,
    saturation: u8,
    fight: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawTheme {
    zones: Vec<String>,
    trim: RawTrim,
    #[serde(default)]
    trim_ramp: String,
    #[serde(default)]
    upper: RawUpper,
    frame: String,
    #[serde(default)]
    carving: RawCarving,
    border: RawBorder,
    #[serde(default)]
    border_ramps: [String; 2],
    #[serde(default)]
    lane_wear: i8,
    #[serde(default)]
    wall: Vec<RawWallMotif>,
    #[serde(default)]
    floor: Vec<RawFloorMotif>,
    boss: RawBoss,
    grade: RawGrade,
}

fn hex(s: &str) -> Option<u32> {
    let h = s.strip_prefix('#')?;
    (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten()
}

pub fn compile(src: &Source, cx: &mut Ctx) -> model::DungeonThemes {
    let table = "looks/dungeon_themes";
    if src.file(FILE).is_none() {
        return model::DungeonThemes::EMPTY;
    }
    let rows = src.table(table, &mut cx.diag);
    let mut themes: Vec<DungeonTheme> = Vec::with_capacity(rows.len());
    for (id, row) in &rows {
        let at = format!("{}: {table}.{id}", row.file);
        let Some(r) = typed::<RawTheme>(row, &at, &mut cx.diag) else { continue };
        let zones: Vec<_> = r.zones.iter().filter_map(|z| cx.zone(&format!("{at}.zones"), z)).collect();
        for z in &zones {
            let twice = themes.iter().find(|t| t.zones.contains(z));
            cx.diag.need(
                twice.is_none(),
                format!("{at}.zones"),
                format!("{z:?} already wears {}", twice.map_or("", |t| t.id)),
            );
        }
        cx.diag.need((-1..=1).contains(&r.lane_wear), format!("{at}.laneWear"), "is -1, 0 or 1");
        for (k, w) in r.wall.iter().enumerate() {
            cx.diag.need(w.every >= 1, format!("{at}.wall[{k}].every"), "a motif repeats every one cell or more");
        }
        for (k, f) in r.floor.iter().enumerate() {
            let repeats = matches!(f.motif, RawFloor::Gantry | RawFloor::Plinths | RawFloor::Stanchions);
            cx.diag.need(
                !repeats || f.every >= 1,
                format!("{at}.floor[{k}].every"),
                "a motif repeats every one cell or more",
            );
        }
        for (what, n) in [("boss.spokes", r.boss.spokes), ("boss.setSpokes", r.boss.set_spokes)] {
            cx.diag.need((3..=24).contains(&n), format!("{at}.{what}"), "is 3 to 24");
        }
        cx.diag.need(matches!(r.boss.braziers, 0 | 2 | 4), format!("{at}.boss.braziers"), "is 0, 2 or 4");
        let colour = |what: &str, s: &str, cx: &mut Ctx| {
            let c = hex(s);
            cx.diag.need(c.is_some(), format!("{at}.grade.{what}"), "is #rrggbb");
            c.unwrap_or(0xffffff)
        };
        let (tint, lift, fight) =
            (colour("tint", &r.grade.tint, cx), colour("lift", &r.grade.lift, cx), colour("fight", &r.grade.fight, cx));
        let s = |v: &String| leak_str(v);
        themes.push(DungeonTheme {
            id: leak_str(id),
            zones: leak(zones),
            trim: match r.trim {
                RawTrim::Plain => ThemeTrim::Plain,
                RawTrim::Cap => ThemeTrim::Cap,
                RawTrim::Dentil => ThemeTrim::Dentil,
                RawTrim::Frieze => ThemeTrim::Frieze,
                RawTrim::Cornice => ThemeTrim::Cornice,
                RawTrim::Beam => ThemeTrim::Beam,
                RawTrim::Course => ThemeTrim::Course,
                RawTrim::Rail => ThemeTrim::Rail,
            },
            trim_ramp: s(&r.trim_ramp),
            upper: match r.upper {
                RawUpper::Carry => ThemeUpper::Carry,
                RawUpper::Shelves => ThemeUpper::Shelves,
            },
            frame: s(&r.frame),
            carving: match r.carving {
                RawCarving::Plain => ThemeCarving::Plain,
                RawCarving::Angel => ThemeCarving::Angel,
                RawCarving::Lamp => ThemeCarving::Lamp,
                RawCarving::Fluted => ThemeCarving::Fluted,
            },
            border: match r.border {
                RawBorder::Edging => ThemeBorder::Edging,
                RawBorder::Lozenge => ThemeBorder::Lozenge,
                RawBorder::Inlay => ThemeBorder::Inlay,
                RawBorder::Stripes => ThemeBorder::Stripes,
                RawBorder::Kerb => ThemeBorder::Kerb,
                RawBorder::Line => ThemeBorder::Line,
                RawBorder::Pebbles => ThemeBorder::Pebbles,
            },
            border_ramps: [s(&r.border_ramps[0]), s(&r.border_ramps[1])],
            lane_wear: r.lane_wear,
            wall: leak(
                r.wall
                    .iter()
                    .map(|w| WallMotif {
                        kind: match w.motif {
                            RawWall::Lamp => WallMotifKind::Lamp,
                            RawWall::Niche => WallMotifKind::Niche,
                            RawWall::Ivy => WallMotifKind::Ivy,
                            RawWall::Painting => WallMotifKind::Painting,
                            RawWall::Sign => WallMotifKind::Sign,
                            RawWall::Valve => WallMotifKind::Valve,
                            RawWall::Window => WallMotifKind::Window,
                            RawWall::Radiator => WallMotifKind::Radiator,
                            RawWall::Pegs => WallMotifKind::Pegs,
                        },
                        every: w.every,
                        at: w.at,
                    })
                    .collect(),
            ),
            floor: leak(
                r.floor
                    .iter()
                    .map(|f| FloorMotif {
                        kind: match f.motif {
                            RawFloor::Rails => FloorMotifKind::Rails,
                            RawFloor::Channel => FloorMotifKind::Channel,
                            RawFloor::Belt => FloorMotifKind::Belt,
                            RawFloor::Gantry => FloorMotifKind::Gantry,
                            RawFloor::Carpet => FloorMotifKind::Carpet,
                            RawFloor::Moon => FloorMotifKind::Moon,
                            RawFloor::Heaps => FloorMotifKind::Heaps,
                            RawFloor::Plinths => FloorMotifKind::Plinths,
                            RawFloor::Stanchions => FloorMotifKind::Stanchions,
                            RawFloor::Chalk => FloorMotifKind::Chalk,
                            RawFloor::Toadstools => FloorMotifKind::Toadstools,
                        },
                        every: f.every,
                        count: f.count,
                        ramp: s(&f.ramp),
                        gathers: leak(f.gathers.iter().map(|g| leak_str(g)).collect()),
                    })
                    .collect(),
            ),
            inlay: s(&r.boss.inlay),
            boss_spokes: r.boss.spokes,
            set_spokes: r.boss.set_spokes,
            braziers: r.boss.braziers,
            emblem: match r.boss.emblem {
                RawEmblem::Plain => ThemeEmblem::Plain,
                RawEmblem::Wheel => ThemeEmblem::Wheel,
                RawEmblem::Headframe => ThemeEmblem::Headframe,
                RawEmblem::Angel => ThemeEmblem::Angel,
                RawEmblem::Furnace => ThemeEmblem::Furnace,
                RawEmblem::Clock => ThemeEmblem::Clock,
                RawEmblem::Portrait => ThemeEmblem::Portrait,
                RawEmblem::Rose => ThemeEmblem::Rose,
            },
            emblem_ramps: [s(&r.boss.emblem_ramps[0]), s(&r.boss.emblem_ramps[1])],
            tint,
            lift,
            saturation: r.grade.saturation,
            fight,
        });
    }
    themes.sort_by_key(|t| t.id);
    model::DungeonThemes { themes: leak(themes) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::diag::Diagnostics;

    fn build(json: &str) -> (model::DungeonThemes, Diagnostics) {
        let src = Source::from_files(&[(FILE, json)]).unwrap();
        let mut cx = Ctx::default();
        let t = compile(&src, &mut cx);
        (t, cx.diag)
    }

    const OK: &str = r##"{ "cave": { "zones": ["mine"], "trim": "cap", "frame": "wood_dark", "border": "edging",
        "laneWear": 1, "wall": [{ "motif": "lamp", "every": 6, "at": 5 }], "floor": [{ "motif": "rails" }],
        "boss": { "inlay": "brass", "emblem": "wheel", "emblemRamps": ["iron", "brass"] },
        "grade": { "tint": "#ffe8c8", "lift": "#140c04", "saturation": 140, "fight": "#ffc8a0" } } }"##;

    #[test]
    fn a_theme_is_checked_and_compiled() {
        let (t, d) = build(OK);
        assert!(d.is_ok(), "{d}");
        let th = t.themes[0];
        assert_eq!(
            (th.id, th.trim, th.wall[0].every, th.boss_spokes, th.tint),
            ("cave", ThemeTrim::Cap, 6, 12, 0xffe8c8)
        );
        assert_eq!(t.of(jane_core::ids::ZoneId::Mine).map(|t| t.id), Some("cave"));
        assert!(!build(&OK.replace("\"mine\"", "\"nowhere\"")).1.is_ok(), "an unknown zone");
        assert!(!build(&OK.replace("\"every\": 6", "\"every\": 0")).1.is_ok(), "a motif every no cells");
        assert!(!build(&OK.replace("#ffe8c8", "amber")).1.is_ok(), "a bad colour");
        assert!(!build(&OK.replace("\"laneWear\": 1", "\"laneWear\": 3")).1.is_ok(), "a wear out of range");
        assert!(!build(&OK.replace("\"trim\"", "\"trimm\"")).1.is_ok(), "an unknown field");
    }
}
