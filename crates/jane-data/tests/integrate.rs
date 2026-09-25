//! The cross-group checks (ARCHITECTURE.md §5.3; `compile/integrate.rs`). The county's set places
//! are `.chunk` files and provide what they draw; until its dressed areas are data too, a name only
//! they draw has no declared provider. Those are pinned here so that any *new* miss fails, and the
//! list only shrinks.

use std::path::Path;

/// Names only `jane/src/world/areas.ts` draws, in code (the Quarry Steps' top: its rect and mark).
const DRAWN_BY_AREA_CODE: &[&str] = &["rect \"quarry_top\""];

/// Read and never set: a real content bug, reported to the owner (the `scarecrow_night` trigger
/// can never fire). Remove it from here when the data is fixed.
const KNOWN_UNSET_FLAGS: &[&str] = &["omen:scarecrow_closer"];

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

#[test]
fn every_flag_read_is_set_somewhere() {
    for w in warnings().iter().filter(|w| w.starts_with("flags:")) {
        let list = w.split_once("never set: ").map_or("", |(_, l)| l);
        for flag in list.split(", ") {
            assert!(KNOWN_UNSET_FLAGS.contains(&flag), "a flag read but never set: {flag}");
        }
    }
}
