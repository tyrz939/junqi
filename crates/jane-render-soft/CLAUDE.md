# jane-render-soft

Owns T0, the CPU rasteriser: draws a `Frame` with an integer pixel path, so frames are byte-identical across targets. It is the reference every other backend is held to.

**Map:** `src/`
- `lib.rs` the backend; `blit.rs` the pixel path (scalar code that autovectorises, no float)
- `lightmap.rs` the lightmap at a quarter of the canvas; `silhouette.rs` sun shadows; `pointshadow.rs` point-light shadows
- `atmos.rs` sky and far things; `glow.rs` glow and bloom; `grade.rs` the `Post` grade (as T2 draws it)
- `ui.rs` the `Ui` pass: the reference for how every backend draws a `UiCmd`

**Invariants:**
- Integer only in the pixel path; output is byte-exact. `soft`'s frames are the reference the gl2 `exact` mode and the PSP's list are compared with.
- `tests/no_alloc.rs` guards the draw path against allocating.
- Depends on `jane-present` for the `Frame`; it must not reach into the sim (sim and data are dev-dependencies only).

**Test it:** `cargo test --release -p jane-render-soft` (`scene`, `night`, `seen`, `no_alloc`).
Dependents: `jane-app`, `jane-cli` (`sheet scene`, `play --snap`), and `jane-render-gl2` (its test compares against soft). A change to the look means gl2's slow test too: `cargo test --release -p jane-render-gl2 -- --ignored`.

**Docs:** `PRESENTATION.md` §1.2, §1.3, §1.4, §1.7, §1.9, §3.1.
