//! The cross-group checks (ARCHITECTURE.md §5.3; `compile/integrate.rs`). The county's set places
//! are `.chunk` files and provide what they draw; until its dressed areas are data too, a name only
//! they draw has no declared provider. Those are pinned here so that any *new* miss fails, and the
//! list only shrinks.

use std::path::Path;

/// Names only `jane/src/world/areas.ts` draws, in code (the Quarry Steps' top: its rect and mark).
const DRAWN_BY_AREA_CODE: &[&str] = &["rect \"quarry_top\""];

fn warnings() -> Vec<String> {
    let built = jane_schema::compile::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    assert!(built.catalog.is_some(), "{}", built.diag);
    built.diag.warnings.iter().map(|d| format!("{}: {}", d.at, d.msg)).collect()
}

#[test]
fn every_used_name_has_a_provider_but_the_area_code_ones() {
    for w in warnings().iter().filter(|w| w.starts_with("providers:")) {
        let list = w.split_once("still code): ").map_or("", |(_, l)| l);
        for name in list.split(", ") {
            assert!(DRAWN_BY_AREA_CODE.contains(&name), "a new name with no provider: {name}");
        }
    }
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
