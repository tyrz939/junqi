# jane-bot

Owns the headless player over `jane-sim`: a policy over `View` that answers each frame with an `InputFrame` and `Command`s. Float-free. It never reads `GameState` (what it needs is added to `View`); only `crawl::setup`, the console's part of a test, reads a blueprint.

**Map:** `src/`
- `lib.rs` `Bot`, `Act` and the two models (the Reader and the Rusher); `run.rs` the session runner
- `nav.rs`, `sense.rs`, `task.rs`, `fight.rs`, `talk.rs`, `story.rs` walk, ask the view, do one thing, fight, converse, read the quest log
- `crawl.rs` and `tactics/` dungeons (keys, locks, plates, bosses, per-dungeon tactics); `console.rs` story-test setup
- `audit.rs`, `coarse.rs`, `experience.rs`, `lost.rs`, `telemetry.rs`, `pair.rs` audits and measures; `fixture.rs` the bot-session hash

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; a run is deterministic (seed, model and build make the same frames), so every run is a tape.
- It may know what the view shows, no more (VERIFICATION.md §2 L3 "a model may not be better informed").
- `tests/fixtures/bot-hash-x86_64.txt` moves only when the sim does, on purpose.

**Test it:** `cargo test --release -p jane-bot --test fixture` first, then `determinism`, `first_minutes`, `start_is_fair`, `save_form`, `audit`. The story, dungeon and region proofs are mostly `#[ignore]` (`story`, `dungeons`, `clarity`, `experience`, `regions`, `story/growth`): `cargo test --release -p jane-bot -- --ignored --skip full_`.
Dependents: `jane-net` (dev), `jane-app`, `jane-cli`. A sim or worldgen change usually needs this crate's ignored tier.

**Docs:** `VERIFICATION.md` §2 L3 and §3; `ARCHITECTURE.md` §1, §8, §11; `PORT.md` §4.
