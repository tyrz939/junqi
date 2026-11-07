# jane-schema

Owns the content schema: the compiled catalog's types (`model`), how they print as Rust (`emit`), and, with the `compile` feature, the compile from `/data` (parse, merge, unit conversion, interning, validation, codegen). Float-free.

**Map:** `src/`
- `model/` the compiled catalog types the game runs on (`jane-data` holds one as a static)
- `compile/` the pipeline: serde structs per table, merge, units, interning, checks; `compile/fraction.rs` reads a JSON number as digits and a power of ten
- `emit.rs` prints a value as Rust source for `jane-data`'s build script

**Invariants:**
- `#![deny(clippy::float_arithmetic, clippy::float_cmp)]`; no float type, even for JSON numbers (PORT.md §3.4).
- `no_std` unless `std`/tests; `compile` pulls in serde, serde_json, indexmap, xxhash and implies `std`.
- `compile` is enabled by `jane-data`'s build script and dev-dependency and by `jane-cli`; nothing parses JSON at runtime.

**Test it:** `cargo test --release -p jane-schema --features compile` (unit tests in `src/`; there is no `tests/`).
Dependents: `jane-data` (its build runs this compile) and `jane-cli`. A schema change recompiles everything downstream of `jane-data`, so also run `cargo test --release -p jane-data`.

**Docs:** `ARCHITECTURE.md` §6; `PORT.md` §5, §3.4.
