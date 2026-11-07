# jane-art

Owns procedural sprites, tiles, font, icons and chrome, each with albedo, normal, emissive and height (four layers). Float-free; depends on `jane-core` and `jane-data` only. Nothing is a stored image.

**Map:** `src/`
- `canvas.rs` the four-layer 16 px canvas and every primitive; `palette.rs` the one RGB table (all else indexes it); `hue.rs` hue-shifted ramps; `hash.rs` the salts
- `person/`, `creature/`, `kit/` (props), `house.rs`, `flora.rs`, `rock.rs`, `garden.rs`, `far.rs` the generators; `looks.rs` renders every row of the looks table
- `terrain/` the tile painter (16 x 16 cell chunks); `deck.rs` decks; `weather.rs`, `night.rs`, `light.rs` atmosphere, night kit, the integer light pass
- `icon.rs`, `chrome.rs`, `font.rs` (stroke glyphs rasterised at boot), `fx.rs` effect tables
- `sprite.rs` `SpriteSet`; `sheet*.rs` contact sheets and the PNG encoder (`jane sheet ...`)

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`, `#![warn(missing_docs)]`; `no_std` unless `std`/tests.
- Output is pinned byte for byte: `tests/golden.rs` against `golden.txt`, and `terrain_golden.txt`. A look change moves them only on purpose, and holds to the art bar (ART.md §3.1).

**Test it:** `cargo test --release -p jane-art` (tests: `golden`, `terrain`, `terrain_reach`, `buildings`, `creatures`, `persons`, `props`, `icons`, `height`, `no_grids`). To see a change, use `jane sheet ...` (`jane-cli`); `examples/` holds small drivers.
Dependents: `jane-present`, `jane-render-soft`, `-wgpu`, `-psp`, `jane-app`, `jane-cli`, and `jane-world` (dev). Re-run `jane-present` and `jane-render-soft` tests after a look change.

**Docs:** `ART.md` (§3.1 quality bar, §5 sheets, §8 migration); `PRESENTATION.md` §1.4, §1.6.
