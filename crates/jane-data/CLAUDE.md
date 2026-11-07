# jane-data

Owns the compiled content: `/data` as plain statics, emitted by `build.rs` through `jane-schema`. Float-free. Nothing here parses JSON at runtime.

**Map:**
- `src/lib.rs` the accessors over the generated statics
- `build.rs` runs the `jane-schema` compile over `/data` and writes the Rust
- `tests/` one file per table family

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; `no_std` unless `std`/tests.
- Content is data: a new item, foe or line is a row in `data/*.json`, not code (root rule 2). A bad row fails this crate's build.
- The `dev-data` feature (std + schema `compile`) puts the compile in the library; the build script and dev-dependency use it regardless.
- A data change can move `tests/fixtures/hashes-x86_64.txt` and `bot-hash-x86_64.txt`; see root rule 5.

**Test it:** `cargo test --release -p jane-data` (tests: `chunks`, `combat`, `compiled`, `county`, `dungeons`, `integrate`, `living`, `looks`, `night`, `regions`, `story`, `voice`).
Depended on by `jane-world`, `jane-sim`, `jane-art`, `jane-present`, `jane-bot`, `jane-net`, `jane-render-soft`, `jane-app`, `jane-cli` and psp-game. After a data change also run `jane-world` (`--test determinism`) and `jane-sim`, then the full gates.

**Docs:** `ARCHITECTURE.md` §6; `PORT.md` §5; `VOICE.md` for any text row.
