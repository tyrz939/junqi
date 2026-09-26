//! Compiles `data/bindings.json` into `$OUT_DIR/bindings.rs` (PRESENTATION.md §4; PORT.md §5):
//! the default bindings are data, checked here, and a name this build does not know (an action,
//! a key, a pad input, a mouse button) is a build error with the row, not a key that silently does
//! nothing. The names are `src/input_names.rs`, shared with the code that reads `config.json`.

use std::fmt::Write as _;
use std::path::Path;

#[allow(dead_code)]
mod names {
    include!("src/input_names.rs");
}

use names::{ACTION_NAMES, KEY_NAMES, MOUSE_NAMES, PAD_NAMES};

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bindings.json");
    println!("cargo:rerun-if-changed={}", path.display());
    println!("cargo:rerun-if-changed=src/input_names.rs");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("bindings.json: {e}"));
    let rows = rows.as_array().unwrap_or_else(|| panic!("bindings.json: a list of rows"));
    let mut out = String::from("/// `data/bindings.json`, compiled (build.rs).\npub const BINDINGS: &[Binding] = &[\n");
    let mut seen: Vec<&str> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let obj = row.as_object().unwrap_or_else(|| panic!("bindings.json row {i}: an object"));
        for k in obj.keys() {
            assert!(
                matches!(k.as_str(), "action" | "keys" | "mouse" | "pad"),
                "bindings.json row {i}: unknown field {k}"
            );
        }
        let action = obj.get("action").and_then(|a| a.as_str()).unwrap_or_else(|| panic!("row {i}: no action"));
        let (_, expr, _) = ACTION_NAMES
            .iter()
            .find(|a| a.0 == action)
            .unwrap_or_else(|| panic!("bindings.json row {i}: unknown action {action}"));
        assert!(!seen.contains(&action), "bindings.json: {action} bound twice");
        seen.push(action);
        let mut keys = [0u16; 2];
        if let Some(ks) = obj.get("keys").and_then(|k| k.as_array()) {
            assert!(ks.len() <= 2, "bindings.json {action}: at most two keys");
            for (j, k) in ks.iter().enumerate() {
                let name = k.as_str().unwrap_or_else(|| panic!("{action}: a key is a name"));
                keys[j] = KEY_NAMES
                    .iter()
                    .find(|n| n.0 == name)
                    .unwrap_or_else(|| panic!("bindings.json {action}: unknown key {name}"))
                    .1;
            }
        }
        let opt = |field: &str, table: &[(&str, &str)]| -> String {
            match obj.get(field).and_then(|v| v.as_str()) {
                None => "None".into(),
                Some(name) => {
                    let e = table
                        .iter()
                        .find(|n| n.0 == name)
                        .unwrap_or_else(|| panic!("bindings.json {action}: unknown {field} {name}"))
                        .1;
                    format!("Some({e})")
                }
            }
        };
        let _ = writeln!(
            out,
            "    Binding {{ action: {expr}, keys: [{}, {}], mouse: {}, pad: {} }},",
            keys[0],
            keys[1],
            opt("mouse", MOUSE_NAMES),
            opt("pad", PAD_NAMES)
        );
    }
    for (name, _, _) in ACTION_NAMES {
        assert!(seen.contains(name), "bindings.json: no row for {name}");
    }
    out.push_str("];\n\n/// Every action by its data name.\npub const ACTIONS: &[(&str, Action, &str)] = &[\n");
    for (name, expr, label) in ACTION_NAMES {
        let _ = writeln!(out, "    ({name:?}, {expr}, {label:?}),");
    }
    out.push_str("];\n\n/// Every pad input by its name.\npub const PADS: &[(&str, PadInput)] = &[\n");
    for (name, expr) in PAD_NAMES {
        let _ = writeln!(out, "    ({name:?}, {expr}),");
    }
    out.push_str("];\n\n/// Every mouse button by its name.\npub const MICE: &[(&str, MouseButton)] = &[\n");
    for (name, expr) in MOUSE_NAMES {
        let _ = writeln!(out, "    ({name:?}, {expr}),");
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("bindings.rs");
    std::fs::write(&dest, out).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
}
