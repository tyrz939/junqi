# jane-render-wgpu

Owns T2, the `wgpu` backend (Vulkan, DX12, Metal or GLES 3), WGSL shaders. The full-quality tier: height-field shadows, bloom, a per-region and hour grade.

**Map:** `src/`
- `lib.rs` the frame: G-buffer, height field (compute), light, bloom, grade, ui; its `//!` is the pass list
- `gpu.rs` the device and adapter; `prep.rs` a `Frame` into instance lists in draw order and the lights; `atmos.rs` sky and far things; `ui.rs` the `Ui` pass
- `shaders/` the WGSL (for example `light.wgsl`)

**Invariants:**
- Sprite albedo and emissive are 16-bit master-palette indices looked up in a 1024-entry CLUT (PRESENTATION.md §1.4).
- A backend is a function of the `Frame`: no game state read here. Depends on `jane-present` and `jane-art`; `jane-core` and `jane-sim` are dev-dependencies.
- The root gates exclude this crate (`--exclude jane-render-wgpu`), because it needs a GPU adapter. Do not assume a green workspace run covered it.

**Test it:** `cargo test --release -p jane-render-wgpu --test shadow` (needs a working adapter; run it yourself when you touch shaders or `prep.rs`), and `cargo check -p jane-render-wgpu` otherwise.
Dependents: `jane-app` and `jane-cli` (feature `gpu`). Compare a look change against `jane-render-soft`.

**Docs:** `PRESENTATION.md` §1.2, §1.3, §1.7 (T2), §1.8, §1.9, §3.1.
