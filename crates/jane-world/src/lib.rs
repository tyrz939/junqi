//! Worldgen: skeleton, county, interiors, dungeons, placements, stories, names, the solver and
//! its checks. `build_zone(zone, seed) -> Blueprint` (PORT.md §4, §6). Never calls the sim.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod skeleton;
pub mod solve;
pub mod steps;
