//! The dungeons compile against the real `/data` with one thing broken at a time. Carries the
//! mission half of `jane/test/templates.test.ts` (`lintDef` is clean; each rule rejects its bad
//! case) and the data-only rejections of `jane/test/dungeon-gen.test.ts` ("the checks really
//! reject": C3), plus the PORT.md §5.3 build checks: node order, pools, binds, placeholders,
//! edges, states, the fallback stamp.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::compile::build_source;
use crate::compile::source::Source;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, root, out);
        } else {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&p).unwrap()));
        }
    }
}

fn files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    walk(&data_dir(), &data_dir(), &mut out);
    out
}

/// Every error of a compile of `/data` with `file` changed by `change`.
fn errors_with(file: &str, change: impl Fn(String) -> String) -> Vec<String> {
    let mut fs = files();
    let f = fs.iter_mut().find(|(p, _)| p == file).unwrap_or_else(|| panic!("no {file}"));
    f.1 = change(std::mem::take(&mut f.1));
    let refs: Vec<(&str, &str)> = fs.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let src = Source::from_files(&refs).unwrap();
    build_source(&src).diag.errors.iter().map(ToString::to_string).collect()
}

/// The same, with the mine's mission changed as JSON.
fn mine_with(change: impl Fn(&mut Value)) -> String {
    errors_with("dungeons/mine.json", |t| {
        let mut v: Value = serde_json::from_str(&t).unwrap();
        change(&mut v);
        serde_json::to_string(&v).unwrap()
    })
    .join("\n")
}

fn node<'a>(v: &'a mut Value, id: &str) -> &'a mut Value {
    v["nodes"].as_array_mut().unwrap().iter_mut().find(|n| n["id"] == id).unwrap()
}

#[test]
fn the_real_data_compiles() {
    let built = crate::compile::build(&data_dir());
    assert!(built.diag.is_ok(), "{}", built.diag);
    let c = built.catalog.unwrap();
    assert_eq!(c.dungeons.templates.len(), 109);
    assert_eq!(c.dungeons.missions.len(), 8);
}

#[test]
fn an_unchanged_copy_compiles_clean() {
    assert_eq!(mine_with(|_| {}), "");
}

#[test]
fn a_mission_field_nobody_reads_is_an_error() {
    let e = mine_with(|v| v["budget"]["baseHeet"] = 6.into());
    assert!(e.contains("dungeons/mine.json") && e.contains("unknown field `baseHeet`"), "{e}");
}

#[test]
fn lint_def_catches_each_mistake() {
    // A grant nothing the node holds gives.
    let e =
        mine_with(|v| node(v, "guard")["grants"].as_array_mut().unwrap().push(serde_json::json!({ "key": "nowhere" })));
    assert!(e.contains("node \"guard\" says it grants {\"key\":\"nowhere\"} and nothing it holds gives that"), "{e}");
    // A thing that answers a verb holds loot she could reach without it.
    let e = mine_with(|v| {
        let office = node(v, "office");
        let h = office["holds"].as_array_mut().unwrap().iter_mut().find(|h| h["socket"] == "verbprop:cabinet").unwrap();
        h["loot"] = serde_json::json!([{ "item": "wood", "qty": 1 }]);
    });
    assert!(e.contains("which answers repair, and loot she could reach without it"), "{e}");
    // A sight line with no corridor to look along.
    let e = mine_with(|v| {
        v["edges"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "from": "entry", "to": "vault", "kind": { "t": "sight" } }));
    });
    assert!(e.contains("edge entry -> vault is a sight line with no corridor to look along"), "{e}");
    // An edge to nowhere.
    let e = mine_with(|v| {
        v["edges"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "from": "entry", "to": "attic", "kind": { "t": "open" } }));
    });
    assert!(e.contains("edge entry -> attic names a node that does not exist"), "{e}");
    // Two entrances; an entrance that is not first.
    let e = mine_with(|v| node(v, "store")["kind"] = "entrance".into());
    assert!(e.contains("a dungeon has exactly one entrance"), "{e}");
    let e = mine_with(|v| node(v, "entry")["order"] = 3.into());
    assert!(e.contains("the entrance must come first in the order"), "{e}");
    // A node twice.
    let e = mine_with(|v| {
        let dup = node(v, "nook").clone();
        v["nodes"].as_array_mut().unwrap().push(dup);
    });
    assert!(e.contains("node \"nook\" is defined twice"), "{e}");
}

#[test]
fn states_are_at_most_three_and_each_has_a_control_and_its_values() {
    let state =
        |id: &str| serde_json::json!({ "id": id, "values": ["on", "off"], "initial": "on", "flag": format!("f_{id}") });
    let e = mine_with(|v| v["states"] = Value::Array(["a", "b", "c", "d"].map(state).to_vec()));
    assert!(e.contains("a dungeon may have three states at most"), "{e}");
    assert!(e.contains("state \"a\" has no control: nothing in the mission can change it"), "{e}");
    let e = mine_with(|v| {
        v["states"] = serde_json::json!([{ "id": "a", "values": ["on", "off"], "initial": "lit", "flag": "f_a" }]);
    });
    assert!(e.contains("state \"a\" starts as \"lit\", which is not one of its values"), "{e}");
    let e = mine_with(|v| {
        v["edges"].as_array_mut().unwrap()[0]["kind"] = serde_json::json!({ "t": "state", "var": "power", "is": "on" });
    });
    assert!(e.contains("waits on a state called \"power\", and the mission has none"), "{e}");
}

#[test]
fn c3_catches_a_plain_key_spent_on_the_wrong_door() {
    // dungeon-gen.test.ts: one plain key for two plain locks: spend it on the guard room and the hub is lost.
    let e = mine_with(|v| node(v, "store")["holds"] = serde_json::json!([]));
    assert!(e.contains("C3: opening plain locks in the order [entry-guard] strands her short of"), "{e}");
}

#[test]
fn node_order_is_monotone_unless_the_edge_is_a_shortcut() {
    let e = mine_with(|v| node(v, "vault")["order"] = 4.into());
    assert!(e.contains("edge office -> vault runs back in the order (5 -> 4) and is not a shortcut"), "{e}");
    // A shortcut may run back: gallery -> plate is one.
    let e = mine_with(|v| {
        for e in v["edges"].as_array_mut().unwrap() {
            if e["from"] == "gallery" && e["to"] == "plate" {
                e["shortcut"] = false.into();
            }
        }
    });
    assert!(e.contains("edge gallery -> plate runs back in the order (6 -> 1)"), "{e}");
}

#[test]
fn pools_must_have_the_sockets_binds_and_placeholders_a_node_asks_for() {
    let e = mine_with(|v| node(v, "plate")["pool"] = "mine.attic".into());
    assert!(e.contains("pool \"mine.attic\" has no templates"), "{e}");
    let e = mine_with(|v| node(v, "store")["holds"][0]["socket"] = "chest:nope".into());
    assert!(e.contains("mine.store.a has no socket chest:nope for node store"), "{e}");
    let e = mine_with(|v| node(v, "entry")["binds"][1]["from"] = "nowhere".into());
    assert!(e.contains("mine.entry.a has nothing called nowhere for node entry"), "{e}");
    // `@socket` must be a socket, mark or rect of every template of the pool...
    let e = mine_with(|v| node(v, "plate")["holds"][0]["use"][0]["prop"] = "@chest:nope".into());
    assert!(e.contains("@chest:nope: mine.plate.a has no socket, mark or rect called chest:nope"), "{e}");
    // ...and stand where the generator binds names: a flag keeps its `@`.
    let e = mine_with(|v| {
        node(v, "plate")["holds"][0]["use"] = serde_json::json!([{ "do": "flag", "flag": "@chest:reward" }]);
    });
    assert!(e.contains("\"@chest:reward\" stands where the generator does not bind names"), "{e}");
    // A holding whose prop does not fit its socket.
    let e = mine_with(|v| node(v, "store")["holds"][0]["prop"] = "gate_h".into());
    assert!(e.contains("\"gate_h\" (3x1) does not fit socket chest:main of mine.store.a"), "{e}");
    // A unit holding takes a patrol and nothing else.
    let e = mine_with(|v| node(v, "guard")["holds"][0]["locked"] = true.into());
    assert!(e.contains("unit:clerk holds a unit, which takes a patrol and nothing else"), "{e}");
    // A bind that says unit where the node holds a prop.
    let e = mine_with(|v| node(v, "store")["binds"][0]["what"] = "unit".into());
    assert!(e.contains("bind chest:main -> store_chest says it is a Unit"), "{e}");
}

#[test]
fn edges_seal_their_corridors() {
    let e = mine_with(|v| {
        for e in v["edges"].as_array_mut().unwrap() {
            if e["kind"]["prop"] == "broken_steps" {
                e["kind"]["prop"] = "barrel".into();
            }
        }
    });
    assert!(e.contains("\"barrel\" is 2x2 and cannot seal a corridor of 3 both ways"), "{e}");
    let e = mine_with(|v| {
        v["edges"].as_array_mut().unwrap()[0]["kind"] =
            serde_json::json!({ "t": "oneway", "how": "drop", "flag": "x" });
    });
    assert!(e.contains("unknown variant `drop`"), "{e}");
}

#[test]
fn the_fallback_must_stamp() {
    // A critical node the fallback leaves out.
    let e = mine_with(|v| {
        v["fallback"].as_array_mut().unwrap().retain(|f| f["node"] != "vault");
    });
    assert!(e.contains("the fallback does not place \"vault\""), "{e}");
    // Two rooms on one bay.
    let e = mine_with(|v| {
        for f in v["fallback"].as_array_mut().unwrap() {
            if f["node"] == "store" {
                f["bay"] = serde_json::json!([1, 3]);
            }
        }
    });
    assert!(e.contains("does not fit"), "{e}");
    // A transform the template does not allow.
    let e = mine_with(|v| v["fallback"][0]["turn"] = 90.into());
    assert!(e.contains("the fallback row for \"entry\" does not fit"), "{e}");
    // A template from another pool.
    let e = mine_with(|v| v["fallback"][0]["template"] = "mine.vault.a".into());
    assert!(e.contains("mine.vault.a is not in entry's pool mine.entry"), "{e}");
}

#[test]
fn room_errors_name_the_file_and_line() {
    // Rule 1 on a real template: a hole in the rim.
    let e = errors_with("rooms/mine/plate.a.room", |t| {
        let lines: Vec<&str> = t.lines().collect();
        let gi = lines.iter().position(|l| *l == "grid").unwrap();
        let mut out: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
        // Row 3 of the grid: its first character is the west rim.
        out[gi + 3].replace_range(0..1, ".");
        out.join("\n")
    });
    let rim: Vec<&String> = e.iter().filter(|x| x.contains("rule 1")).collect();
    assert!(!rim.is_empty(), "{e:?}");
    let gi = std::fs::read_to_string(data_dir().join("rooms/mine/plate.a.room"))
        .unwrap()
        .lines()
        .position(|l| l == "grid")
        .unwrap();
    assert!(rim[0].starts_with(&format!("rooms/mine/plate.a.room:{}: rule 1", gi + 4)), "{rim:?}");
    // A parse error stops the file, and names its line.
    let e = errors_with("rooms/mine/nook.a.room", |t| t.replace("bays    1x1", "bays    wide"));
    assert!(
        e.iter().any(|x| x.starts_with("rooms/mine/nook.a.room:") && x.contains("bays \"wide\" is not WxH")),
        "{e:?}"
    );
    // A variant that changes its pool's interface.
    let e = errors_with("rooms/mine/plate.b.room", |t| {
        t.lines().map(|l| if l.starts_with("grants") { "grants  -" } else { l }).collect::<Vec<_>>().join("\n")
    });
    assert!(e.iter().any(|x| x.contains("mine.plate.b differs from mine.plate.a in its interface")), "{e:?}");
}

#[test]
fn derived_zones_read_the_missions_as_strings() {
    let mut fs = files();
    fs.retain(|(p, _)| !p.starts_with("dungeons/") || p == "dungeons/mine.json");
    let refs: Vec<(&str, &str)> = fs.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let src = Source::from_files(&refs).unwrap();
    let z = super::derived_zones(&src);
    assert_eq!(z.len(), 1);
    let mine = &z[0];
    assert_eq!(mine.zone, "mine");
    assert_eq!(mine.units, ["clerk", "headmaster", "iron_knuckles"]);
    assert_eq!(mine.marks, ["entry", "adit"]);
    assert_eq!(mine.rects, ["mine_entry", "boss_arena"]);
    assert!(mine.props.iter().any(|p| p == "gate_boss") && mine.props.iter().any(|p| p == "broken_track"));
    assert_eq!(mine.given_verbs, ["icebolt"]);
    assert!(mine.given_keys.is_empty() && mine.states.is_empty());
    let has = |s: &str| mine.provides.iter().any(|(n, _)| n == s);
    for n in [
        "mine_all",
        "mine_arena_wayin",
        "mine_gate_arena_nook",
        "hm_drawer_free",
        "mine_cage_both_down",
        "mine_office_chest_wood",
    ] {
        assert!(has(n), "{n}");
    }
}
