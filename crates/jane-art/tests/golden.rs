//! Goldens (ART.md §1, §5): the FNV-1a hash of every step-1 artefact over its size and all four
//! layers equals `tests/golden.txt`. Unintended drift fails here; intended drift is a diff of
//! that file, which the owner sees.
//!
//! Re-bless after an intended change with either
//!
//! ```text
//! cargo run -p jane-cli -- sheet --bless
//! JANE_BLESS=1 cargo test -p jane-art --test golden
//! ```

use std::collections::BTreeMap;

use jane_art::{Font, demo};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.txt");

fn parse(s: &str) -> BTreeMap<String, String> {
    s.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once(' ').map(|(k, v)| (k.to_string(), v.trim().to_string())))
        .collect()
}

#[test]
fn every_artefact_matches_its_golden() {
    let now = demo::golden_file(&Font::build());
    if std::env::var_os("JANE_BLESS").is_some() {
        std::fs::write(PATH, &now).unwrap();
        return;
    }
    let want = parse(&std::fs::read_to_string(PATH).expect("tests/golden.txt: bless it (see this file's header)"));
    let got = parse(&now);
    let mut diff = Vec::new();
    for (k, v) in &got {
        match want.get(k) {
            None => diff.push(format!("{k}: new ({v}), not in golden.txt")),
            Some(w) if w != v => diff.push(format!("{k}: {w} in golden.txt, now {v}")),
            Some(_) => {}
        }
    }
    diff.extend(want.keys().filter(|k| !got.contains_key(*k)).map(|k| format!("{k}: in golden.txt, no longer drawn")));
    assert!(diff.is_empty(), "art drifted; re-bless if this is intended:\n{}", diff.join("\n"));
}
