//! Procedural sprites, tiles, font, icons and chrome, with albedo, normal, emissive and height
//! (ART.md). Today, migration step 1 (ART.md §8): the palette, the four-layer canvas and its
//! primitives, the hash salts, the stroke font, the chrome, the integer light pass, the step-1
//! demo sprites and their goldens, and the contact sheets with the PNG encoder.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
#![warn(missing_docs)]
// `no_std` plus `alloc` without the `std` feature, for the consoles (PORT.md §13.9, §13.11): the
// terrain painter runs on the PSP. Every path below is `core::` or `alloc::`, which std re-exports,
// so the std build is the same code.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

pub mod canvas;
pub mod chrome;
pub mod creature;
pub mod demo;
pub mod far;
pub mod flora;
pub mod font;
pub mod fx;
pub mod garden;
pub mod hash;
pub mod house;
pub mod hue;
pub mod icon;
pub mod kit;
pub mod light;
pub mod looks;
pub mod palette;
pub mod person;
pub mod rock;
pub mod sheet;
pub mod sheet_kit;
pub mod sheet_person;
pub mod sprite;
pub mod terrain;
pub mod weather;

pub use canvas::{Canvas, Z};
pub use font::{Face, Font, Style};
pub use palette::{Ix, Ramp, Tone};
