# spikes/psp-game

Owns the PSP program: boots, reads the presenter's tables and the PSP pack from beside the program, shows the title, loads the baked county, and plays as the PC does (sim at 60 ticks, `jane-present` `Frame`, drawn by `jane-render-psp` on the GE). It is outside the cargo workspace (own `Cargo.toml`, own `rust-toolchain.toml`, `no_std`, `no_main`), so workspace clippy and tests do not cover it.

**Map:** `src/`
- `main.rs` entry and boot; `game.rs` the game thread and `run()` (about 1180 lines: read by section, grep `fn `); `shell.rs` the game around the world (the PC `App`'s role)
- `workers.rs` the terrain painter's worker and its boost; `platform.rs` the heap (talc pool plus a kernel block) and the platform; `files.rs` Memory Stick files
- `loader.rs` the baked county's loader; `county_cache.rs` zones on the Memory Stick; `ahead.rs` read-ahead
- `script.rs` pad and `script.txt` (scripted walk, `bench`, shots); `bench.rs`, `micro.rs`, `perf.rs`, `log.rs`, `diag.rs` benchmark, micro-bench, overlay, 2 s log, `crash.txt`
- `vendor/` is the vendored `psp` crate: do not edit it or search it.

**Invariants:**
- Integers only; the only unsafe is platform glue. User-mode only, no kernel mode, no custom Media Engine code (root rule 6).
- Links `jane-sim`, `jane-data`, `jane-core`, `jane-world`, `jane-present`, `jane-render-psp`, `jane-audio-psp` with `default-features = false`, so a `std` use in any of them breaks this build.
- Release profile is `opt-level = "s"`, fat LTO, `panic = "abort"`; `jane-art`, `jane-render-psp` and some others override to `opt-level = 3`. `crashtest` is the one feature.
- Threads (lower runs first): audio 16, loader 30, game 32, painter 48.

**Test it:** `bash tools/psp-dist.sh` builds it and is the compile check; then the PPSSPP headless bench or a hardware bench (root CLAUDE.md "The PSP loop"). Format with `cargo +1.94.1 fmt` and `git checkout` the unrelated files that touches (`ahead.rs`, `loader.rs`, `micro.rs`, `perf.rs`).
Rebuild after a change in any crate it links (list above).

**Docs:** `PSP-NOW.md` (read first), `PLAN-PSP-SPEED.md`; `PORT.md` §13.12, §13.13.
