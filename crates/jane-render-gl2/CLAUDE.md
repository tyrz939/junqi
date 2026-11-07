# jane-render-gl2

Owns T1, the OpenGL 2.1 / OpenGL ES 2.0 backend through `glow` (GLSL 1.20 / ES 1.00), for GPUs from about 2006 and every Raspberry Pi. Draws the same `Frame` as every tier.

**Map:** `src/`
- `lib.rs` the frame in order (albedo, normal and height, emissive, shadows, light, compose, bloom and shafts, ui); its `//!` is the full pass list
- `prep.rs` a `Frame` into vertex lists and steps; `shaders.rs` the shaders, written once in the common GLSL subset; `ui.rs` the `Ui` pass
- `sdl.rs` the GL context through SDL2; `gl.rs` the one module with the GL path's unsafe code

**Invariants:**
- `unsafe_code` is `deny`, not `forbid`, in this crate's own `[lints]` table (the workspace's, copied); unsafe stays in `gl.rs` (PORT.md §3.4, §4).
- In its exact mode the albedo is `soft`'s frame before its light, byte for byte (`prep.rs`).
- Rendering needs a GL context from SDL2, so it cannot run on a headless box.

**Test it:** `cargo test --release -p jane-render-gl2` compiles and runs what needs no GPU; the real check, `tests/albedo.rs`, is `#[ignore]` (four scenes on two GL APIs against soft, needs a GL context): `cargo test --release -p jane-render-gl2 -- --ignored`.
Dependents: `jane-app`, `jane-cli` (feature `gpu`). Depends on `jane-render-soft` (dev) and `jane-present`.

**Docs:** `PRESENTATION.md` §1.2, §1.3, §1.7 (T1), §3.1; `VERIFICATION.md` §6 (slow tier).
