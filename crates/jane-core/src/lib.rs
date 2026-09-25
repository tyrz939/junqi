//! Rng, hashes, the numeric types, grids, flood, chamfer, A*, ids and the Blueprint: what every
//! other crate stands on (PORT.md §4).
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod action;
pub mod angle;
pub mod blueprint;
pub mod grid;
pub mod hash;
pub mod ids;
mod misc;
pub mod noise;
pub mod num;
pub mod rng;
pub mod search;
pub mod tile;

#[cfg(test)]
mod rng_vectors;
mod trig_table;

pub use indexmap;

pub use action::{Action, Cond, Condition, CondsRef, FlagKey, ListRef, NamesRef, Stack, TextRef};
pub use angle::Angle;
pub use blueprint::Blueprint;
pub use grid::{Cell, CellIx, Grid, Rect};
pub use ids::*;
pub use misc::{Lookup, pick_weighted, sort_by_total_key, view};
pub use num::{Fx, Milli, Permille, Q15, Q16, Tick, Vec2};
pub use rng::{Sfc32, dice};
pub use tile::{Material, Tile};
