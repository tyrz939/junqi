//! Procedural sprites, tiles, font, icons and chrome, with albedo, normal, emissive and height
//! (ART.md). Today: the contact-sheet PNG encoder.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod sheet;
