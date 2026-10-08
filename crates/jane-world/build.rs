//! `SOURCE_STAMP`: a hash of every source a blueprint is a function of, besides its seed (the
//! worldgen, the core types, the content and the compiler of the content). A cache of built
//! blueprints keys on it (PORT.md §13.13, `jane_sim::zone_cache`): a change to any of these
//! files is a stamp no cached zone carries, so a stale zone is rebuilt, never played. Line ends
//! are dropped from the hash, so a checkout's CRLF does not move it.

use std::path::{Path, PathBuf};

/// FNV-1a 64.
fn feed(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let crates = manifest.join("..");
    let roots = [
        crates.join("jane-world/src"),
        crates.join("jane-core/src"),
        crates.join("jane-data/src"),
        crates.join("jane-data/build.rs"),
        crates.join("jane-schema/src"),
        crates.join("../data"),
    ];
    let mut all = Vec::new();
    for r in &roots {
        println!("cargo:rerun-if-changed={}", r.display());
        if r.is_dir() {
            files(r, &mut all);
        } else {
            all.push(r.clone());
        }
    }
    // By path from the crates' directory, sorted, so the order is every machine's.
    let mut named: Vec<(String, PathBuf)> = all
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(&crates).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            (rel, p)
        })
        .collect();
    named.sort();
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (rel, p) in &named {
        feed(&mut h, rel.as_bytes());
        feed(&mut h, &[0]);
        let bytes: Vec<u8> = std::fs::read(p).unwrap_or_default().into_iter().filter(|&b| b != b'\r').collect();
        feed(&mut h, &(bytes.len() as u64).to_le_bytes());
        feed(&mut h, &bytes);
    }
    println!("cargo:rustc-env=JANE_WORLD_SOURCE_STAMP={h:016x}");
}
