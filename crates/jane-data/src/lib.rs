//! The compiled content: `/data` as plain statics, emitted by `build.rs` through `jane-schema`
//! (ARCHITECTURE.md §6). Nothing parses JSON at runtime.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub use jane_schema::model::*;

#[allow(clippy::all, clippy::pedantic, clippy::unreadable_literal)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/catalog.rs"));
}

/// The catalog compiled into this build.
pub fn catalog() -> &'static Catalog {
    &generated::CATALOG
}

/// The looks compiled into this build (`data/looks`, ART.md §5): every look by the sprite id it
/// draws. Apart from the catalog, and outside its content hash.
pub fn looks() -> Looks {
    generated::LOOKS
}

/// The terrain's looks compiled into this build (`data/looks/tiles.json`, ART.md §2.6): a
/// `TileStyle` per tile and per render-only material. Beside the looks, outside the content hash.
pub fn tile_looks() -> TileLooks {
    generated::TILE_LOOKS
}

/// Compile a data dir at startup through the same code the build script ran (`dev-data`).
/// A catalog reload needs a new game.
#[cfg(feature = "dev-data")]
pub fn load(dir: &std::path::Path) -> Result<&'static Catalog, jane_schema::compile::diag::Diagnostics> {
    let built = jane_schema::compile::build(dir);
    built.catalog.ok_or(built.diag)
}
