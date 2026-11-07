# jane-render-psp

Owns C2, the PSP backend: the GE draws the same `Frame` every tier draws. Pure integer quad building (testable on a PC) plus one module of GE glue.

**Map:** `src/`
- `pack.rs` the PSP pack's tables (`JPK2`); pages stay on the Memory Stick until a frame names one
- `list.rs` and `list/` (`build`, `sun`, `lightmap`, `sprites`) turn a `Frame` into quads: pure and integer, so a PC test reads what the PSP draws
- `cache.rs` which pages are held in RAM (LRU under a byte budget) and in VRAM (LRU of slots)
- `ui.rs` the `Ui` pass as quads; `light.rs`, `lamps.rs`, `normals.rs`, `grade.rs`, `water.rs` C2's lighting, lamp-shadow cache, grade (term for term `soft`'s) and water
- `capture.rs` the detailed capture (L + R + START); `ge.rs` (PSP only) the quads to the GE through rust-psp's `sceGu*`

**Invariants:**
- `no_std` plus `alloc` without `std`; integer only. `unsafe_code` is `deny`, not `forbid`, here: `ge.rs` is the one module with unsafe code.
- Dependencies by path with `default-features = false` (PORT.md §13.9). User-mode PSP only (root rule 6); a look change needs the side-by-side A/B.
- Heavy per-frame cost: PSP speed is the constraint, so read `PLAN-PSP-SPEED.md` budgets before adding work to `list`.

**Test it:** `cargo test --release -p jane-render-psp` (unit tests in `src/`; no `tests/`; `jane-sim` is a dev-dependency). The real check is `bash tools/psp-dist.sh` plus the bench.
Dependents: `jane-cli` (`bake`, `psp-profile`) and psp-game. It depends on `jane-present`, `jane-art`, `jane-core`.

**Docs:** `PORT.md` §13.5, §13.12, §13.13; `PSP-NOW.md`, `PLAN-PSP-SPEED.md`.
