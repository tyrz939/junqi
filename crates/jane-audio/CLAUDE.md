# jane-audio

Owns Jane's sound, all made by code (no audio file ships): the synth, sound effects, ambient beds, the music sequencer, the mixer, and the consoles' integer tracker. Nothing here is read by the sim and nothing here reads it; the cue table that turns the game into sound is `jane-present::audio`.

**Map:** `src/`
- `patch.rs` the sound effects of `data/audio/sfx.json`, rendered at boot; `bed.rs` ambient loops made live; `seq.rs` the music sequencer over `data/audio/songs`
- `dsp.rs`, `voice.rs`, `model.rs`, `pattern.rs` the synth pieces; `engine.rs` the mixer the app's audio callback runs
- `analysis.rs`, `wav.rs` checking and hearing without ears
- `bake.rs` renders the synth once into a module and shared sample bank (PORT.md §13.4); `tracker.rs` the integer player and mixer for it

**Invariants:**
- Floats are allowed on the PC path (the float gate does not cover this crate). Without `std` (the consoles) only `tracker` builds: `no_std` plus `alloc`, integers only; keep it that way.
- Not part of the sim's state or hash; a change to sound never moves a fixture.
- `build.rs` reads `data/audio` at build time.

**Test it:** `cargo test --release -p jane-audio` (`music`, `tracker`; four `#[ignore]` tests in `tracker.rs`, run with `-- --ignored`). Check the console build still compiles: `cargo check -p jane-audio --no-default-features`.
Dependents: `jane-audio-psp` (links the tracker), `jane-app`, `jane-cli` (`jane audio`, `jane sheet audio`).

**Docs:** `PRESENTATION.md` §5; `PORT.md` §13.4.
