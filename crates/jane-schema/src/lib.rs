//! The content schema: the compiled catalog's types (`model`), how they print as Rust
//! (`emit`), and, with the `compile` feature, the compile from `/data` itself: serde structs
//! for every table, merge, unit conversion, interning, validation and codegen (PORT.md §5).
//!
//! `jane-data` depends on the model for its types and runs the compile from its build script.
//!
//! Float-free: no floating-point type anywhere in this crate; `compile/fraction.rs` reads a JSON
//! number as digits and a power of ten (PORT.md §3.4, Grok #7).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
// `no_std` plus `alloc` without the `std` feature, for the consoles (PORT.md §13.9): the model and
// the emit use `core::` and `alloc::` paths only. `compile` turns `std` on; it reads files, host only.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

#[cfg(feature = "compile")]
pub mod compile;
pub mod emit;
pub mod model;

/// What the exported `model!` and `model_enum!` name, so they expand the same in a `no_std` crate.
#[doc(hidden)]
pub mod __private {
    pub use alloc::string::String;
}
