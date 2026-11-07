# jane-sim

Owns the deterministic simulation: state tree, tick, verbs, save, replay, hash and the read-only `View`. Float-free. Depends on `jane-core`, `jane-data`, `jane-world` only; it knows no renderer, device or socket.

**Map:** `src/` (the full module table is in `lib.rs`'s `//!`)
- `state.rs` the authoritative tree; `sim.rs` the scheduler (step order); `zone.rs`, `blueprints.rs`, `baked.rs` zones from blueprints / the baked county
- `actions.rs`, `verbs.rs`, `interact.rs`, `combat.rs`, `cast.rs`, `flush.rs`, `status.rs` verbs and the combat pipeline
- `ai.rs`, `npc.rs`, `presence.rs`, `living.rs`, `night.rs`, `fire.rs`, `regrow.rs` controllers, schedules, the living world
- `save.rs` save, load and the hash; `replay.rs` tapes (`.jrp`); `trace.rs` traces (`.jtr`); `view.rs` what a seat sees; `metrics.rs` cost, never state
- `seats.rs`, `travel.rs`, `zone_cache.rs`, `ring.rs` seats, zone travel, the load ring

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; `no_std` unless `std`/tests. No wall time, devices or floats in state.
- Same seed and inputs give the same hash; `View` is derived and read-only (ARCHITECTURE.md §11).
- Default features `std` and `baked`; psp-game links it with `default-features = false`.

**Test it:** `cargo test --release -p jane-sim --test determinism` and `--test ground_digest`, then the file for the area (`combat`, `fight`, `quests`, `save`, `snapshot`, `fires`, `night`, `regrow`, `spine` ...). Unit tests also sit in `src/*_tests.rs`. Slow `#[ignore]`: `ai` timings (`--ignored --nocapture`), `chase_strip`, `combat`, `route`, `zone_cache`.
Dependents: `jane-bot`, `jane-net`, `jane-present`, all render crates, `jane-audio-psp`, `jane-app`, `jane-cli`, psp-game. After any sim change run `jane-bot --test fixture` and the bot-hash gate.

**Docs:** `ARCHITECTURE.md` §3 to §5, §8, §9, §11; `PORT.md` §4; `VERIFICATION.md` §3.1.
