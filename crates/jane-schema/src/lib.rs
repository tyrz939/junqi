//! The content schema: the compiled catalog's types (`model`), how they print as Rust
//! (`emit`), and, with the `compile` feature, the compile from `/data` itself: serde structs
//! for every table, merge, unit conversion, interning, validation and codegen (PORT.md §5).
//!
//! `jane-data` depends on the model for its types and runs the compile from its build script.
//!
//! Float-free: no floating-point type anywhere in this crate; `compile/fraction.rs` reads a JSON
//! number as digits and a power of ten (PORT.md §3.4, Grok #7).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

#[cfg(feature = "compile")]
pub mod compile;
pub mod emit;
pub mod model;
