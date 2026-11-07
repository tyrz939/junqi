# jane-audio-psp

Owns the PSP's sound: the PC's cue table (`jane_present::audio::Soundtrack`) asks a bus for music, placed effects and beds; the bus turns each ask into a `jane_audio::tracker::Cmd` in integers and hands it to the audio thread through a lock-free ring, where the tracker mixer plays the baked module (`jane-psp.jau`).

**Map:** `src/`
- `lib.rs` `Ring` (single producer, single consumer, atomics only, never waits), `Bus` (cue asks as commands), `Sound` (cue table and bus, ticked with the game)
- `psp.rs` (PSP only) the audio thread on sceAudio, the module read from the Memory Stick, the scripted-run capture
- `vfpu.rs` the VFPU spike for the mixer's group pan (PLAN-PSP-SPEED.md, "VFPU: where it pays")

**Invariants:**
- `no_std` plus `alloc` without `std`; integers on the console path. With `std` on (the workspace's tests) the bus also takes the PC's float placings.
- `unsafe_code` is `deny`, not `forbid`, in this crate's own `[lints]` table (a copy of the workspace's), because of the PSP glue in `psp.rs` and `vfpu.rs`; keep unsafe there.
- Dependencies are by path with `default-features = false`, so psp-game links no `std` (PORT.md §13.9).
- User-mode only (root rule 6): firmware audio channels and the VFPU are fine.

**Test it:** `cargo test --release -p jane-audio-psp` (unit tests in `src/`; there is no `tests/`). The PSP side is proved only by `bash tools/psp-dist.sh` and a hardware or PPSSPP run.
Depended on only by `spikes/psp-game`. It also depends on `jane-audio`, `jane-present`, `jane-sim` and `jane-core`, so a change in those can break it.

**Docs:** `PORT.md` §13.4, §13.9; `PLAN-PSP-SPEED.md` (VFPU); `PSP-NOW.md`.
