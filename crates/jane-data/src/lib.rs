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

/// The atmosphere's layers compiled into this build (`data/atmosphere.json`, WORLD.md §5.3):
/// fog volumes by place, hour and sky. Presentation only, outside the content hash.
pub fn atmosphere() -> Atmosphere {
    generated::ATMOSPHERE
}

/// Compile a data dir at startup through the same code the build script ran (`dev-data`).
/// A catalog reload needs a new game.
///
/// A compiled catalog is leaked to be `'static` (the compile leaks its interned tables too), so a
/// reload of files that have not changed hands back the catalog already built rather than leaking
/// another (Grok #15): only a change to the files' bytes compiles, and leaks, again.
#[cfg(any(feature = "dev-data", test))]
pub fn load(dir: &std::path::Path) -> Result<&'static Catalog, jane_schema::compile::diag::Diagnostics> {
    use std::sync::Mutex;
    static LOADED: Mutex<Option<(u64, &'static Catalog)>> = Mutex::new(None);
    let key = dir_hash(dir);
    let mut loaded = LOADED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let (Some(k), Some((had, cat))) = (key, *loaded) {
        if k == had {
            return Ok(cat);
        }
    }
    let built = jane_schema::compile::build(dir);
    let cat = built.catalog.ok_or(built.diag)?;
    if let Some(k) = key {
        *loaded = Some((k, cat));
    }
    Ok(cat)
}

/// FNV-1a 64 over every file under `dir`, path and bytes, in path order. `None` if any of it
/// cannot be read (the compile then says why).
#[cfg(any(feature = "dev-data", test))]
fn dir_hash(dir: &std::path::Path) -> Option<u64> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
        for e in std::fs::read_dir(d)? {
            let p = e?.path();
            if p.is_dir() { walk(&p, out)? } else { out.push(p) }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, &mut files).ok()?;
    files.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for f in files {
        eat(f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().as_bytes());
        eat(&[0]);
        eat(&std::fs::read(&f).ok()?);
        eat(&[0]);
    }
    Some(h)
}

#[cfg(test)]
mod tests {
    /// Grok #15: a reload of unchanged files is the catalog already built, not a second leak.
    #[test]
    fn a_reload_of_the_same_files_leaks_nothing_new() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let a = super::load(&dir).expect("the data");
        let b = super::load(&dir).expect("the data");
        assert!(std::ptr::eq(a, b), "a second catalog was built and leaked");
        assert_eq!(a.content_hash, super::catalog().content_hash);
    }
}
