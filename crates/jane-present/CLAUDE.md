# jane-present

Owns the scene: builds the backend-agnostic `Frame` from a `View`; also the UI, input mapping, text and the sound cue table. Headless and GPU-free: everything runs in a test with no window. Does not write sim state.

**Map:** `src/` (big: read by section, `grep 'fn '`)
- `present/` `Present`: `tick()` once per accumulated tick, `draw(alpha, canvas)` once per frame; `present/draw.rs` is huge (`draw()` alone is about 940 lines)
- `frame.rs` the `Frame`; `drawlist.rs` the allocation-free draw list; `backend.rs` the backend contract; `atlas.rs` sprite pages
- `people.rs`, `creatures.rs`, `props.rs`, `decks.rs` art into the atlas; `terrain.rs`, `chunks.rs` the chunk cache; `chunk_pack.rs` baked console chunks
- `light.rs`, `night.rs`, `atmos.rs`, `shadow.rs`, `fx.rs`, `ambient.rs`, `camera.rs`, `turn.rs`, `memory.rs` the look and feel
- `ui/` immediate-mode UI and screens; `text.rs` / `text/` the English (the sim has none); `input.rs` device to `InputFrame`; `audio.rs` the cue table
- `tables.rs` the presenter's tables as a pack (`JPT1`); `bench.rs` the built-in benchmark; `pad_psp.rs`, `gfx_psp.rs` console pad and Graphics page

**Invariants:**
- `Present::draw` is pure in the ticked state and `alpha` (PORT.md §7.1 contract). A backend is a function of the `Frame`.
- `no_std` unless `std`/tests; PSP builds link it with `default-features = false`. The float gate does not cover this crate, but the console path stays integer.
- Dependencies by path with `default-features = false` (PORT.md §13.9).

**Test it:** `cargo test --release -p jane-present` (`night`, `lights`, `terrain`, `tables`, `lesson`, `dungeons`, `houses`, `map_memory` ...; `#[ignore]` ones in `src/present/tests.rs` and `src/stand_in.rs`).
Dependents: all render crates, `jane-audio-psp`, `jane-app`, `jane-cli`, psp-game. A `Frame` change needs `jane-render-soft` tests and the PSP build.

**Docs:** `PRESENTATION.md` (§1 frame and tiers, §3 UI, §4 input, §5 audio); `PORT.md` §7.1, §13.4.
