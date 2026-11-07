# jane-core

Owns what every crate stands on: Rng, hashes, numeric types, grids, flood/chamfer/A*, ids and the `Blueprint`. Float-free; it depends on nothing in the workspace.

**Map:** `src/`
- `num.rs` numeric types (authoritative); `angle.rs` angles (65 536 per turn, 0 = east); `noise.rs` integer value noise in `Q16`
- `rng.rs` sfc32 + splitmix32 and `dice()`, the one worldgen seed scheme; `hash.rs` FNV-1a over explicit little-endian bytes, murmur3 finaliser
- `grid.rs` grids/cells/rects (sizes are `u32`, never `usize`, in anything saved or hashed); `plane.rs` packed chunked byte planes; `search.rs` the one flood, chamfer and A*
- `ids.rs` content ids (`u16` newtypes); `tile.rs` tiles and cell flags; `action.rs` verbs and conditions (exhaustive enums)
- `blueprint.rs` what a zone builder returns; `names.rs`, `rare.rs` compact storage; `garden.rs` front garden read off tiles; `misc.rs` weighted pick, sorts, `Lookup`

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; no floating-point type anywhere (PORT.md §3.4).
- `no_std` unless the `std` feature (default) or tests; `serde` is optional (jane-sim turns it on).
- Changing the Rng, a hash or the Blueprint shape moves every fixture and save: treat it as a world change.

**Test it:** `cargo test --release -p jane-core` (unit tests live in `src/`; there is no `tests/`).
Every other crate depends on it (and psp-game), so a real change here needs the full gates.

**Docs:** `PORT.md` §4 (also §3.4, §6.e search, §13.3 packed forms); `ARCHITECTURE.md` §2, §3.1, §5.1.
