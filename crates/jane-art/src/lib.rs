//! Procedural sprites, tiles, font, icons and chrome, with albedo, normal, emissive and height
//! (ART.md). Today, migration step 1 (ART.md §8): the palette, the four-layer canvas and its
//! primitives, the hash salts, the stroke font, the chrome, the integer light pass, the step-1
//! demo sprites and their goldens, and the contact sheets with the PNG encoder.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
#![warn(missing_docs)]

pub mod canvas;
pub mod chrome;
pub mod demo;
pub mod font;
pub mod hash;
pub mod light;
pub mod looks;
pub mod palette;
pub mod person;
pub mod sheet;
pub mod sheet_person;
pub mod sprite;

pub use canvas::{Canvas, Z};
pub use font::{Face, Font, Style};
pub use palette::{Ix, Ramp, Tone};
