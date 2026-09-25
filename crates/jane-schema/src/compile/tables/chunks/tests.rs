//! The parser and the lint: a small chunk that is clean, and each rule rejecting its bad case. Then
//! the compile over a fixture: sites, file names, a prop's JSON, doors, fills and names given twice.

use jane_core::Tile;
use jane_core::action::Facing;

use super::lint::{Facts, lay_out};
use super::parse::{Entry, Layer, parse};
use crate::compile::build_source;
use crate::compile::source::Source;
use crate::model::{ChunkSide, NameKind};

struct Fixture;

impl Facts for Fixture {
    fn prop(&self, def: &str) -> Option<(i32, i32, bool)> {
        match def {
            "lamp_post" => Some((1, 1, true)),
            "crate" => Some((2, 2, true)),
            "flower_bed" => Some((3, 1, false)),
            _ => None,
        }
    }

    fn unit(&self, def: &str) -> bool {
        def == "dog"
    }
}

const YARD: &str = "\
// A yard.
id      yard
box     6x4
anchor  centre
face    w
gates   w1, e2
slots   hole 3 0
around  near n2 e0 s1 w3
grid
,,,,,,
,,::,,
::::::
,,,,,,
things
L.CC..
..CCd.
......
FFF...
names
p....m
......
.....p
......
claims
xxx...
......
......
......
legend
, tile Grass
: tile Dirt
L prop lamp_post
C prop crate box1 {\"talk\": \"x\"}
F prop flower_bed
d unit dog dog south [[0,2],[5,2,60]]
m mark here east
p rect yard
";

/// Replace the first occurrence of `from` in the clean yard.
fn yard_with(from: &str, to: &str) -> String {
    assert!(YARD.contains(from), "the fixture has no {from:?}");
    YARD.replacen(from, to, 1)
}

/// Every error the parser or the lint finds, as text.
fn errors(text: &str) -> Vec<String> {
    match parse(text, "chunks/yard.chunk") {
        Err((_, e)) => vec![e],
        Ok(c) => lay_out(&c, &Fixture).1.into_iter().map(|(_, e)| e).collect(),
    }
}

fn rejects(text: &str, needle: &str) {
    let errs = errors(text);
    assert!(errs.iter().any(|e| e.contains(needle)), "expected an error with {needle:?}, got {errs:?}");
}

#[test]
fn the_yard_parses_and_lays_out_clean() {
    let c = parse(YARD, "chunks/yard.chunk").expect("parses");
    assert_eq!((c.id.as_str(), c.w, c.h, c.anchor), ("yard", 6, 4, (3, 2)));
    assert_eq!(c.slots[0].0.name, "hole");
    assert_eq!(c.around[0].1, [2, 0, 1, 3]);
    let (laid, errs) = lay_out(&c, &Fixture);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(laid.tiles[0], Tile::Grass);
    assert_eq!(laid.tiles[2 * 6], Tile::Dirt);
    let at = |p: &super::lint::Placed| (p.x, p.y, p.w, p.h);
    assert_eq!(laid.props.iter().map(at).collect::<Vec<_>>(), [(0, 0, 1, 1), (2, 0, 2, 2), (0, 3, 3, 1)]);
    assert_eq!(laid.units.iter().map(at).collect::<Vec<_>>(), [(4, 1, 1, 1)]);
    assert_eq!(laid.marks.iter().map(at).collect::<Vec<_>>(), [(5, 0, 1, 1)]);
    assert_eq!(laid.rects.iter().map(at).collect::<Vec<_>>(), [(0, 0, 6, 3)]);
    assert_eq!(laid.claims.iter().filter(|c| **c).count(), 3);
    let Entry::Unit { route, facing, .. } = &c.legend[5].entry else { panic!("unit") };
    assert_eq!((*facing, route.clone()), (Some(Facing::South), vec![(0, 2, None), (5, 2, Some(60))]));
}

#[test]
fn layers_may_be_left_out() {
    let text = "id a\nbox 2x1\nface e\ngates e0\ngrid\n,,\nlegend\n, tile Grass\n";
    let c = parse(text, "chunks/a.chunk").expect("parses");
    assert_eq!(c.layer(Layer::Things).rows, vec![vec!['.', '.']]);
    assert!(lay_out(&c, &Fixture).1.is_empty());
}

#[test]
fn the_grid_is_the_box_and_rectangular() {
    rejects(&yard_with(",,::,,\n", ",,::,\n"), "row 1 is 5 wide, the box is 6");
    rejects(&yard_with("box     6x4", "box     6x5"), "has 4 rows, the box is 5 high");
    rejects(&yard_with("......\nFFF...", "FFF..."), "things has 3 rows, the box is 4 high");
    rejects(&yard_with("box     6x4", "box     six"), "is not WxH");
}

#[test]
fn every_character_is_in_the_legend_and_every_entry_is_drawn() {
    rejects(&yard_with(",,::,,\n", ",,::,?\n"), "\"?\" is not in the legend");
    rejects(&yard_with("m mark here east\n", "m mark here east\nz mark there\n"), "\"z\" (mark) is never drawn");
    rejects(&yard_with(", tile Grass\n", ", tile Grass\n. tile Dirt\n"), "may not define \".\"");
    rejects(&yard_with(", tile Grass\n", ", tile Grass\n, tile Dirt\n"), "defines \",\" twice");
    rejects(&yard_with(", tile Grass", ", tile Grassy"), "no tile called \"Grassy\"");
    rejects(&yard_with("p rect yard", "p lot yard"), "unknown legend kind \"lot\"");
}

#[test]
fn each_kind_keeps_to_its_layer() {
    rejects(&yard_with("L.CC..\n", "L.CC.m\n"), "is a mark: it belongs in names");
    rejects(&yard_with("p....m\n", "p...Lm\n"), "is a prop: it belongs in things");
    rejects(&yard_with(",,,,,,\n,,::", "L,,,,,\n,,::"), "is a prop, not a tile");
    rejects(&yard_with("xxx...", "xxo..."), "only x (claimed) and . (open)");
}

#[test]
fn props_are_whole_footprints_of_existing_defs() {
    rejects(&yard_with("..CCd.", "..C.d."), "is not a whole 2x2 footprint");
    rejects(&yard_with("FFF...", "FF...."), "is not a whole 3x1 footprint");
    rejects(&yard_with("L prop lamp_post", "L prop lamp_pole"), "unknown prop def \"lamp_pole\"");
    rejects(&yard_with("d unit dog", "d unit wolf"), "unknown unit def \"wolf\"");
}

#[test]
fn keyed_things_and_marks_are_drawn_once_and_named_once() {
    rejects(&yard_with("......\nFFF...", "....CC\nFFF.CC"), "box1 is keyed and drawn 2 times");
    rejects(&yard_with("......\n.....p", "m.....\n.....p"), "mark here is drawn 2 times");
    rejects(&yard_with("around  near", "around  yard"), "rect \"yard\" is named twice");
    rejects(&yard_with("p....m\n......\n", "p....m\n..p...\n"), "draw its top-left and bottom-right corners");
    rejects(&yard_with("d unit dog dog", "d unit dog - bad"), "\"bad\" is not a facing");
}

#[test]
fn nothing_leaves_the_box() {
    rejects(&yard_with("slots   hole 3 0", "slots   hole 6 0"), "slot hole (6,0) is outside the box");
    rejects(&yard_with("[[0,2],[5,2,60]]", "[[0,2],[6,2,60]]"), "waypoint (6,2) is outside the box");
    rejects(&yard_with("anchor  centre", "anchor  9 9"), "anchor (9,9) is outside");
    rejects(&yard_with("[[0,2],[5,2,60]]", "[[0,2,-1]]"), "-1 ticks");
}

#[test]
fn gates_are_on_the_ring_walkable_and_one_faces_front() {
    rejects(&yard_with("gates   w1, e2", "gates   w1, e4"), "gate e4 is off the e side");
    rejects(&yard_with("gates   w1, e2", "gates   w1, e2, e2"), "gate e2 is given twice");
    // The lamp stands just inside a north gate at x 0.
    rejects(&yard_with("gates   w1, e2", "gates   w1, n0"), "gate n0: the cell inside it (0,0) is not walkable");
    rejects(
        &yard_with("::::::\n,,,,,,\nthings", ":::::|\n,,,,,,\nthings")
            .replace(", tile Grass", ", tile Grass\n| tile Fence"),
        "gate e2: the cell inside it (5,2)",
    );
    rejects(&yard_with("face    w", "face    n"), "face n: no gate on that side");
    rejects(&yard_with("gates   w1, e2", "gates   w1, x2"), "cannot read gate \"x2\"");
}

#[test]
fn nobody_stands_in_a_wall() {
    rejects(
        &yard_with("..CCd.\n", "..CC.d\n")
            .replace(",,::,,\n", ",,::,|\n")
            .replace(", tile Grass", ", tile Grass\n| tile Fence"),
        "unit dog at (5,1) stands on ground feet cannot take",
    );
}

#[test]
fn the_header_is_known_keys_once() {
    rejects(&yard_with("id      yard", "id      yard\nrotate  90"), "unknown header key \"rotate\"");
    rejects(&yard_with("face    w", "face    w\nface    e"), "\"face\" is given twice");
    rejects(&yard_with("id      yard\n", ""), "no \"id\" line");
    rejects(&yard_with("anchor  centre", "anchor  centre\npin     q 4"), "pin \"q 4\" is not a side");
    rejects(&yard_with("things\n", "claims\n"), "out of order");
}

// --- the compile over a fixture --------------------------------------------------------------

fn zones_json() -> String {
    let rows: Vec<String> = jane_core::ids::ZoneId::ALL
        .iter()
        .map(|z| format!(r#"{{"id": "{}", "kind": "interior", "contract": {{"marks": ["entry"]}}}}"#, z.name()))
        .collect();
    format!("[{}]", rows.join(",\n")).replacen(r#""kind": "interior""#, r#""kind": "county""#, 1)
}

const PROPS: &str = r#"{
    "lamp_post": {"name": "Lamp Post", "sprite": "lamp_post", "w": 1, "h": 1, "solid": true, "blockLos": false},
    "door": {"name": "Door", "sprite": "door", "w": 2, "h": 2, "solid": true, "blockLos": false},
    "coffin": {"name": "Coffin", "sprite": "coffin", "w": 2, "h": 3, "solid": true, "blockLos": false}
}"#;

const STATION: &str = "\
id      station
box     4x3
face    e
gates   e2
grid
,,,,
,,,,
,,,,
things
L.DD
..DD
....
names
.c..
..cs
....
legend
, tile Grass
L prop lamp_post
D prop door station_door {\"to\": {\"zone\": \"arms\", \"mark\": \"entry\"}, \"label\": \"In\"}
s mark start east
c fill coffins
";

/// Build the fixture with `station.chunk` replaced by `chunk` and the tuning by `tuning`, and
/// return the chunk group's errors (the fixture's other tables are left thin, and speak too).
fn compile_errors(files: &[(&str, &str)]) -> Vec<String> {
    let zones = zones_json();
    let mut all: Vec<(&str, &str)> = vec![
        ("zones.json", zones.as_str()),
        ("sites.json", r#"[{"id": "station", "name": "Castle Halt", "region": "lowfields", "onRoad": true}]"#),
        ("props.json", PROPS),
        ("units.json", r#"{"dog": {}}"#),
        (
            "tuning/chunks.json",
            r#"{"boxMargin": 8, "fills": [{"id": "coffins", "def": "coffin", "count": 1, "tries": 9, "margin": 1}]}"#,
        ),
        ("chunks/station.chunk", STATION),
    ];
    for (path, text) in files {
        match all.iter_mut().find(|(p, _)| p == path) {
            Some(f) => f.1 = text,
            None => all.push((path, text)),
        }
    }
    let src = Source::from_files(&all).expect("fixture JSON");
    let built = build_source(&src);
    built.diag.errors.iter().filter(|d| d.at.contains("chunks")).map(ToString::to_string).collect()
}

fn compile_rejects(files: &[(&str, &str)], needle: &str) {
    let errs = compile_errors(files);
    assert!(errs.iter().any(|e| e.contains(needle)), "expected an error with {needle:?}, got {errs:?}");
}

#[test]
fn the_fixture_compiles_clean() {
    let errs = compile_errors(&[]);
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn the_fixture_compiles_into_the_model() {
    let zones = zones_json();
    let src = Source::from_files(&[
        ("zones.json", zones.as_str()),
        ("sites.json", r#"[{"id": "station", "name": "Castle Halt", "region": "lowfields", "onRoad": true}]"#),
        ("props.json", PROPS),
        ("units.json", r#"{"dog": {}}"#),
        (
            "tuning/chunks.json",
            r#"{"boxMargin": 8, "fills": [{"id": "coffins", "def": "coffin", "count": 1, "tries": 9, "margin": 1}]}"#,
        ),
        ("chunks/station.chunk", STATION),
    ])
    .expect("fixture");
    let mut cx = crate::compile::ctx::Ctx::default();
    cx.ids = crate::compile::ctx::Ids::collect(&src, &mut cx.diag);
    let story = crate::compile::tables::story::compile(&src, &mut cx);
    let county = crate::compile::tables::county::compile(&src, &mut cx);
    let dungeons = crate::compile::tables::dungeons::compile(&src, &mut cx);
    let chunks = super::compile(&src, &mut cx, &story, &county, &dungeons);
    let c = chunks.get("station").expect("the station");
    assert_eq!((c.w, c.h, c.face, c.site), (4, 3, ChunkSide::E, 0));
    assert_eq!(c.tile(3, 2), Tile::Grass);
    assert_eq!(c.props.len(), 2);
    assert_eq!(c.props[1].to.map(|t| t.zone), Some(jane_core::ids::ZoneId::Arms));
    assert_eq!(c.fills[0].cells.len(), 2, "one run a row");
    assert_eq!(chunks.fill(&c.fills[0]).id, "coffins");
    let exported: Vec<NameKind> = c.exports().map(|(k, _)| k).collect();
    assert_eq!(exported, [NameKind::Prop, NameKind::Mark]);
    assert_eq!(c.gate_cell(c.gates[0]), (4, 2));
    assert_eq!(c.origin(100, 100, 500, 500, 8), (98, 99));
    assert_eq!(c.origin(2, 499, 500, 500, 8), (8, 489));
}

#[test]
fn a_chunk_stands_at_a_site_and_is_named_for_it() {
    compile_rejects(&[("chunks/station.chunk", &STATION.replace("id      station", "id      halt"))], "is no site");
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("id      station", "id      halt"))],
        "in a file named station",
    );
}

#[test]
fn a_props_json_is_typed_and_its_door_leads_somewhere() {
    compile_rejects(&[("chunks/station.chunk", &STATION.replace("\"label\"", "\"lable\""))], "unknown field `lable`");
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("\"mark\": \"entry\"", "\"mark\": \"cellar\""))],
        "provides no such mark",
    );
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("\"zone\": \"arms\"", "\"zone\": \"attic\""))],
        "unknown zone \"attic\"",
    );
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("\"label\": \"In\"", "\"label\": \"In \u{2014} out\""))],
        "en or em dash",
    );
}

#[test]
fn every_fill_has_a_tuning_row_and_every_row_a_fill() {
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("c fill coffins", "c fill urns"))],
        "fill \"urns\" has no row",
    );
    compile_rejects(
        &[("chunks/station.chunk", &STATION.replace("c fill coffins", "c fill urns"))],
        "fill \"coffins\" is drawn by no chunk",
    );
    compile_rejects(&[("tuning/chunks.json", r#"{"boxMargin": 8, "fils": []}"#)], "unknown field `fils`");
}

#[test]
fn a_name_is_exported_by_one_chunk() {
    let other = STATION.replace("id      station", "id      station2").replace("c fill coffins", "c rect elsewhere");
    compile_rejects(
        &[
            (
                "sites.json",
                r#"[{"id": "station", "name": "A", "region": "lowfields", "onRoad": true}, {"id": "station2", "name": "B", "region": "lowfields", "onRoad": true}]"#,
            ),
            ("chunks/station2.chunk", &other),
        ],
        "prop \"station_door\" is also drawn by station",
    );
}
