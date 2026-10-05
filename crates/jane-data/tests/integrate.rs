//! The cross-group checks (ARCHITECTURE.md §5.3; `compile/integrate.rs`). Every name used has a
//! declared provider (a chunk, a placement, a zone's contract), and a miss is a build error
//! (Grok #8), so a built catalog has none.

use std::path::Path;

fn warnings() -> Vec<String> {
    let built = jane_schema::compile::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    assert!(built.catalog.is_some(), "{}", built.diag);
    built.diag.warnings.iter().map(|d| format!("{}: {}", d.at, d.msg)).collect()
}

#[test]
fn every_used_name_has_a_provider() {
    let built = jane_schema::compile::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    assert!(built.catalog.is_some(), "{}", built.diag);
    assert!(built.diag.errors.iter().all(|d| d.at != "providers"), "{}", built.diag);
    assert!(warnings().iter().all(|w| !w.starts_with("providers:")));
}

/// PORT.md §12: a flag read but never set is a build error since P1's triage (the omens set
/// theirs at New Game, `data/omens.json`), so the catalog built at all says there is none; and it
/// is no longer a warning.
#[test]
fn every_flag_read_is_set_somewhere() {
    assert!(warnings().iter().all(|w| !w.starts_with("flags:")));
    let cat = jane_data::catalog();
    let scarecrow = cat.story.omens.iter().find(|o| o.id == "scarecrow_closer").expect("the scarecrow omen");
    assert_eq!(cat.name(scarecrow.flag), "omen:scarecrow_closer", "what the scarecrow triggers read");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a temp dir");
    for e in std::fs::read_dir(from).expect("the data dir") {
        let e = e.expect("an entry");
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("a copy");
        }
    }
}

/// Grok #8: a name with no provider (a typo in a trigger's rect) is a build error, not a warning.
#[test]
fn a_name_with_no_provider_is_an_error() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let tmp = std::env::temp_dir().join(format!("jane-integrate-{}", std::process::id()));
    copy_dir(&data, &tmp);
    let file = tmp.join("triggers/lowfields.json");
    let text = std::fs::read_to_string(&file).expect("the triggers");
    assert!(text.contains("\"rect\": \"quarry_top\""));
    std::fs::write(&file, text.replacen("\"rect\": \"quarry_top\"", "\"rect\": \"quarry_tpo\"", 1)).expect("a write");
    let built = jane_schema::compile::build(&tmp);
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(built.catalog.is_none(), "the typo built");
    assert!(built.diag.errors.iter().any(|d| d.at == "providers" && d.msg.contains("quarry_tpo")), "{}", built.diag);
}
