//! VOICE.md held against every written line: no en or em dashes, nobody's name baked in, and a
//! phrase budget, so a good turn of phrase does not become the county's tic (the story audit,
//! 1 October 2026, §3.4: "all the same" had reached fifteen, "No bell last night." opened ten of
//! the town's fourteen answers to the bell). Raise a budget only by cutting a use somewhere else.

use std::path::{Path, PathBuf};

/// Phrase, then the most times it may appear across every written line (case ignored).
const BUDGET: &[(&str, usize)] = &[
    ("all the same", 3),
    ("never once", 3),
    ("no bell last night", 2),
    ("halfway between", 2),
    ("he'll know", 3),
    ("couldn't tell you why", 4),
    ("thirty years", 3),
];

/// The most `news_bell` answers that may open on "No bell".
const NO_BELL_OPENERS: usize = 2;

/// The files that hold what Castle says: dialogue, quests, stories, items, units' toasts, the
/// clock, triggers, omens and consequences.
fn text_files() -> Vec<PathBuf> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut out: Vec<PathBuf> = [
        "dialogue.json",
        "quests.json",
        "items.json",
        "units.json",
        "omens.json",
        "consequences.json",
        "clock.json",
        "triggers.json",
    ]
    .iter()
    .map(|f| data.join(f))
    .filter(|p| p.exists())
    .collect();
    for dir in ["dialogue", "quests", "stories", "items", "units", "clock", "triggers"] {
        let Ok(rd) = std::fs::read_dir(data.join(dir)) else { continue };
        let mut more: Vec<PathBuf> =
            rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        more.sort();
        out.extend(more);
    }
    out
}

/// Every string value (not key) in a JSON tree.
fn strings<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match v {
        serde_json::Value::String(s) => out.push(s),
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

/// (file, string) for every written string.
fn lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for f in text_files() {
        let raw = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let mut s = Vec::new();
        strings(&v, &mut s);
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        out.extend(s.into_iter().map(|t| (name.clone(), t.to_owned())));
    }
    out
}

#[test]
fn no_written_line_has_a_dash_or_her_name() {
    for (f, t) in lines() {
        assert!(!t.contains('\u{2013}') && !t.contains('\u{2014}'), "{f}: a dash in \"{t}\"");
        // The player unit's own name row is the one place "Jane" is the data, not a line.
        assert!(!t.contains("Jane") || t == "Jane", "{f}: her name baked in: \"{t}\"");
    }
}

#[test]
fn every_phrase_keeps_to_its_budget() {
    let all = lines();
    let mut over = Vec::new();
    for &(phrase, max) in BUDGET {
        let hits: Vec<&(String, String)> = all.iter().filter(|(_, t)| t.to_lowercase().contains(phrase)).collect();
        let n: usize = hits.iter().map(|(_, t)| t.to_lowercase().matches(phrase).count()).sum();
        if n > max {
            let at: Vec<String> = hits.iter().map(|(f, t)| format!("  {f}: {t}")).collect();
            over.push(format!("\"{phrase}\": {n} (budget {max})\n{}", at.join("\n")));
        }
    }
    assert!(over.is_empty(), "over budget:\n{}", over.join("\n"));
}

#[test]
fn the_town_does_not_answer_the_bell_with_one_voice() {
    let c = jane_data::catalog();
    let mut openers = Vec::new();
    for t in c.story.dialogue {
        for n in t.nodes.iter().filter(|n| n.id == "news_bell") {
            let first = c.text(n.lines[0].text);
            if first.starts_with("No bell") {
                openers.push(format!("{}: {first}", t.id));
            }
        }
    }
    assert!(
        openers.len() <= NO_BELL_OPENERS,
        "{} news_bell lines open on \"No bell\":\n{}",
        openers.len(),
        openers.join("\n")
    );
}
