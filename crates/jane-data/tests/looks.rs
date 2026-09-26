//! The looks table (ART.md §5): the static and a fresh compile of /data agree, and the looks stay
//! out of the content hash.

use std::path::Path;

#[test]
fn compiled_looks_equal_dev_data() {
    let built = jane_schema::compile::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    assert!(built.catalog.is_some(), "{}", built.diag);
    assert_eq!(jane_data::looks(), built.looks);
    assert!(!built.looks.is_empty());
}

#[test]
fn every_look_names_a_sprite_of_the_catalog() {
    let c = jane_data::catalog();
    for (id, _) in jane_data::looks() {
        assert!(usize::from(id.0) < c.sprites.len(), "{id:?}");
    }
}
