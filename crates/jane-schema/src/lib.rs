//! The content schema: serde structs for every table, merge, interning, validation and codegen.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
