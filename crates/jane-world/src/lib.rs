//! Worldgen: skeleton, county, interiors, dungeons, placements, stories, names, the solver and
//! its checks. `build_zone(zone, seed) -> Blueprint` (PORT.md §4, §6). Never calls the sim.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod county;
pub mod dungeon;
pub mod hash;
pub mod interiors;
pub mod kit;
pub mod skeleton;
pub mod solve;
pub mod steps;

use jane_core::{Blueprint, ZoneId};

/// The zone a seed builds, proven: every candidate is judged by the solver (and, for a generated
/// dungeon, checks C1 to C12) and re-rolled until one holds (`buildZone`).
///
/// `None` for a zone whose builder has not landed yet: today that is the county, whose later
/// stages (placements, stories) are still being ported (PORT.md §6.m). The eight generated
/// dungeons and the four hand-built interiors (house, cellar, arms, church) are here. Never panics.
pub fn build_zone(zone: ZoneId, seed: u32) -> Option<Blueprint> {
    if interiors::is_interior(zone) {
        return interiors::build_interior(zone, seed);
    }
    jane_data::catalog().dungeons.mission_of(zone)?;
    Some(dungeon::build(zone, seed).blueprint)
}

/// Is this a zone [`build_zone`] can build today?
pub fn builds(zone: ZoneId) -> bool {
    interiors::is_interior(zone) || jane_data::catalog().dungeons.mission_of(zone).is_some()
}
