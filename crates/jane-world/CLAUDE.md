# jane-world

Owns worldgen: `build_zone(zone, seed) -> Blueprint`. It never calls the sim and is float-free. Depends on `jane-core` and `jane-data` only (`jane-art` is a dev-dependency).

**Map:** `src/`
- `skeleton/` the county's skeleton (regions, river, hill, where story places stand); `county/` the 2000 x 2000 county built from `data/world/county/`
- `interiors.rs` hand-built interiors; `dungeon/` generated dungeons; `solve/` the lock-and-key solver and its checks
- `kit.rs` the builder's kit (tile canvas + spawn lists); `canvas.rs` big-zone tiles while built; `bits.rs` packed flag planes
- `steps.rs` every dice-throwing stage as a closed enum (a stage draws only its own stream); `names.rs` place names; `hash.rs` the blueprint hash

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; `no_std` unless `std`/tests.
- Deterministic: same seed, same Blueprint, same hash on every target. `tests/determinism.rs` pins `tests/fixtures/hashes-x86_64.txt`; move it only on purpose.
- Randomness goes through `jane_core::rng`/`dice()` keyed by `steps.rs`; add a stage there rather than an ad hoc Rng.

**Test it:** `cargo test --release -p jane-world --test determinism` first, then `skeleton`, `solver`, `interiors`, or the `county_*` / `dungeon_*` file for the area. Slow `#[ignore]`: `proven_sweep` (every zone over 96 seeds), `terraced_county`, one test each in `county_land`, `county_places`, `county_ways` (`-- --ignored`).
Dependents: `jane-sim`, `jane-bot`, `jane-present`, `jane-app`, `jane-cli`, psp-game. After a layout change run `jane-sim` (`ground_digest`, `county_baked`) and `jane-bot --test fixture`.

**Docs:** `WORLDGEN.md`, `MAP.md`, `DUNGEONS.md`; `PORT.md` §4, §6 (steps, solver, stages), §13.3.
