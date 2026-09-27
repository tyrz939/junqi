//! Checks `data/audio` and embeds it (PRESENTATION.md §5): `sfx.json`, `instruments.json` and
//! every song under `songs/` are read through the crate's own model (`src/model.rs`) and checked
//! (`model::check`: names, patterns against their sections, instruments that exist). A file that
//! does not fit is a build error with its row. It is presentation, not content: outside
//! `jane-data`'s catalog and its content hash, as `data/bindings.json` is.

use std::fmt::Write as _;
use std::path::Path;

#[path = "src/model.rs"]
#[allow(dead_code, clippy::cast_precision_loss)]
mod model;
#[path = "src/pattern.rs"]
#[allow(dead_code, clippy::cast_precision_loss)]
mod pattern;

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> T {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/audio");
    println!("cargo:rerun-if-changed={}", dir.display());
    let songs_dir = dir.join("songs");
    println!("cargo:rerun-if-changed={}", songs_dir.display());
    let sfx_path = dir.join("sfx.json");
    let inst_path = dir.join("instruments.json");
    let mut song_paths: Vec<_> = std::fs::read_dir(&songs_dir)
        .unwrap_or_else(|e| panic!("{}: {e}", songs_dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    song_paths.sort();
    let lib = model::Library {
        sfx: read(&sfx_path),
        instruments: read(&inst_path),
        songs: song_paths.iter().map(|p| read(p)).collect(),
    };
    let errs = model::check(&lib);
    assert!(errs.is_empty(), "data/audio:\n  {}", errs.join("\n  "));
    let path = |p: &Path| p.display().to_string();
    let mut out = String::from("/// `data/audio`, embedded (build.rs checked it).\n");
    let _ = writeln!(out, "pub const SFX_JSON: &str = include_str!({:?});", path(&sfx_path));
    let _ = writeln!(out, "pub const INSTRUMENTS_JSON: &str = include_str!({:?});", path(&inst_path));
    out.push_str("pub const SONGS_JSON: &[&str] = &[\n");
    for p in &song_paths {
        let _ = writeln!(out, "    include_str!({:?}),", path(p));
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("audio_data.rs");
    std::fs::write(&dest, out).unwrap_or_else(|e| panic!("{}: {e}", dest.display()));
}
