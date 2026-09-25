//! ARCHITECTURE.md §8 `compiled_equals_dev_data`: the statics and a fresh compile of /data are
//! the same catalog, so dev reload and the shipped tables cannot differ.

use std::path::Path;

fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

#[test]
fn compiled_equals_dev_data() {
    let built = jane_schema::compile::build(&data_dir());
    let fresh = built.catalog.unwrap_or_else(|| panic!("{}", built.diag));
    let compiled = jane_data::catalog();
    assert_eq!(compiled.content_hash, fresh.content_hash);
    assert!(*compiled == *fresh, "the static catalog differs from a fresh compile of /data");
}

#[test]
fn the_content_hash_ignores_english_and_nothing_else() {
    let c = jane_data::catalog();
    let texts: Vec<&'static str> = c.texts.iter().map(|_| "x").collect();
    let reworded = jane_schema::model::Catalog { texts: Box::leak(texts.into_boxed_slice()), ..*c };
    assert_eq!(jane_schema::compile::content_hash(&reworded), c.content_hash);
    let fewer = jane_schema::model::Catalog { names: &c.names[..c.names.len().saturating_sub(1)], ..*c };
    if !c.names.is_empty() {
        assert_ne!(jane_schema::compile::content_hash(&fewer), c.content_hash);
    }
}
