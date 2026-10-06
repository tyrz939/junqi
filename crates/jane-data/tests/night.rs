//! The night's rules as the data keeps them (WORLD.md §2.1, the owner, 2026-10-06): a bed sleeps
//! the clock to six only at Julie's or an inn (and the two written seats that say otherwise), a
//! fire never passes the clock, and every timed window can be kept at a 24-minute day.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("data/") {
        let p = e.expect("an entry").path();
        if p.is_dir() {
            json_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Every `{"do": "rest", "until": h}` under `v`, with the top-level key it sits under.
fn rests(v: &Value, top: &str, out: &mut Vec<(String, u64)>) {
    match v {
        Value::Object(m) => {
            if m.get("do").and_then(Value::as_str) == Some("rest") {
                if let Some(h) = m.get("until").and_then(Value::as_u64) {
                    out.push((top.to_owned(), h));
                }
            }
            m.values().for_each(|x| rests(x, top, out));
        }
        Value::Array(a) => a.iter().for_each(|x| rests(x, top, out)),
        _ => {}
    }
}

/// Every `{"if": "hours", ...}` under `v`: `(from, to)`.
fn windows(v: &Value, out: &mut Vec<(u64, u64)>) {
    match v {
        Value::Object(m) => {
            if m.get("if").and_then(Value::as_str) == Some("hours") {
                let h = |k| m.get(k).and_then(Value::as_u64).expect("an hour");
                out.push((h("from"), h("to")));
            }
            m.values().for_each(|x| windows(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| windows(x, out)),
        _ => {}
    }
}

fn all() -> Vec<(PathBuf, Value)> {
    let mut files = Vec::new();
    json_files(&data(), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let v = serde_json::from_str(&std::fs::read_to_string(&p).expect("readable")).expect("json");
            (p, v)
        })
        .collect()
}

/// A bed passes the night only where she may sleep: Julie's bed (`bed`), the Arms (`arms_bed`),
/// the Halfway House (`country_inn`), and the seats written to say otherwise (the School's two
/// sick-bay beds, the Museum's bench). A fire never passes the clock.
#[test]
fn beds_sleep_only_where_allowed_and_fires_never_do() {
    let allowed: BTreeSet<&str> =
        ["bed", "arms_bed", "country_inn", "school_bed_day", "school_bed_bell", "museum_bench"].into();
    let mut found = Vec::new();
    for (p, v) in all() {
        let Value::Object(m) = &v else { continue };
        for (k, x) in m {
            let mut here = Vec::new();
            rests(x, k, &mut here);
            found.extend(here.into_iter().map(|(k, h)| (p.display().to_string(), k, h)));
        }
    }
    assert!(!found.is_empty());
    for (file, key, h) in &found {
        assert!(allowed.contains(key.as_str()), "{file}: {key} sleeps to {h}");
        assert!(!key.contains("fire") && !key.contains("grate") && !key.contains("stove"), "{file}: a fire sleeps");
        if !matches!(key.as_str(), "school_bed_bell" | "museum_bench") {
            assert_eq!(*h, 6, "{file}: {key} sleeps to six");
        }
    }
    // Julie's bed and an inn are among them.
    for k in ["bed", "arms_bed", "country_inn"] {
        assert!(found.iter().any(|f| f.1 == k), "{k} sleeps");
    }
}

/// Timed windows at a 24-minute day: every hours window is at least a game hour (a real minute),
/// and every timed vigil is kept by standing in its place (a `while` trigger she can be early
/// for), so a walk never has to land inside a window to keep it.
#[test]
fn timed_windows_stay_feasible() {
    let mut n = 0;
    for (p, v) in all() {
        let mut w = Vec::new();
        windows(&v, &mut w);
        for (from, to) in w {
            n += 1;
            let len = (to + 24 - from) % 24;
            assert!(len >= 1, "{}: {from} to {to}", p.display());
        }
        // A trigger with an hours window is a `while` on a rect: she waits there for it.
        if p.parent().is_some_and(|d| d.ends_with("triggers")) {
            let Value::Object(m) = &v else { continue };
            for (k, t) in m {
                let mut w = Vec::new();
                windows(t, &mut w);
                if !w.is_empty() {
                    assert_eq!(t.get("mode").and_then(Value::as_str), Some("while"), "{k}: a vigil she can wait out");
                    assert!(t.get("rect").is_some(), "{k}: a place to wait");
                }
            }
        }
    }
    assert!(n >= 5, "{n} windows");
    // The longest walk a window may ask is across the county: 2 km at 360 m a real minute is under
    // six game hours now, inside the Museum's ten-to-four (and its bench waits for ten).
    let (county_m, metres_a_minute, museum_hours) = (2000_u32, 360_u32, 16 - 10);
    let walk_s = county_m * 60 / metres_a_minute;
    assert!(walk_s < museum_hours * 60, "the Museum's hours outlast a walk across the county: {walk_s} s");
}
