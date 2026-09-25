//! Compile /data into `catalog.rs` (PORT.md §5.5). A data error is a build error that names
//! the file and the row.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join("../../data");
    println!("cargo:rerun-if-changed={}", root.display());
    let built = jane_schema::compile::build(&root);
    for w in &built.diag.warnings {
        println!("cargo:warning={w}");
    }
    let Some(catalog) = built.catalog else {
        eprintln!("{}", built.diag);
        panic!("/data does not compile: {} error(s); `cargo jane check` lists them", built.diag.errors.len());
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("catalog.rs");
    std::fs::write(&out, jane_schema::compile::codegen(catalog)).expect("write catalog.rs");
}
