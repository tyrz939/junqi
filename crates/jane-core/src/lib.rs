//! Rng, hashes, the numeric types, grids, flood, chamfer, A*, ids and the Blueprint: what every
//! other crate stands on (PORT.md §4).
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
// `no_std` plus `alloc` without the `std` feature, for the consoles (PORT.md §13.9). Every path
// below is `core::` or `alloc::`, which std re-exports, so the std build is the same code.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

pub mod action;
pub mod angle;
pub mod blueprint;
pub mod garden;
pub mod grid;
pub mod hash;
pub mod ids;
mod misc;
pub mod names;
pub mod noise;
pub mod num;
pub mod plane;
pub mod rare;
pub mod rng;
pub mod search;
pub mod tile;

#[cfg(test)]
mod rng_vectors;
mod trig_table;

pub use indexmap;

pub use action::{Action, Cond, Condition, CondsRef, FlagKey, ListRef, NamesRef, NightLock, Stack, TextRef};
pub use angle::Angle;
pub use blueprint::{Blueprint, Packed, RegionMap};
pub use grid::{Cell, CellIx, Grid, Rect};
pub use ids::*;
pub use misc::{IndexMap, Lookup, pick_weighted, sort_by_total_key, view};
pub use names::{NameIndex, Names};
pub use num::{Fx, Milli, Permille, Q15, Q16, Tick, Vec2};
pub use plane::Plane;
pub use rare::{Rare, Thin};
pub use rng::{Sfc32, dice};
pub use tile::{Material, Tile};
