//! Worldgen: skeleton, county, interiors, dungeons, placements, stories, names, the solver and
//! its checks. `build_zone(zone, seed) -> Blueprint` (PORT.md §4, §6). Never calls the sim.
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]
// `no_std` plus `alloc` without the `std` feature, for the consoles (PORT.md §13.9). Every path
// below is `core::` or `alloc::`, which std re-exports, so the std build is the same code.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

pub mod bits;
pub mod county;
pub mod dungeon;
pub mod hash;
pub mod interiors;
pub mod kit;
pub mod names;
pub mod skeleton;
pub mod solve;
pub mod steps;

use jane_core::{Blueprint, ZoneId};

/// The zone a seed builds, proven: every candidate is judged by the solver (and, for a generated
/// dungeon, checks C1 to C13) and re-rolled until one holds (`buildZone`).
///
/// All thirteen zones: the county ([`county::build_proven`]), the eight generated dungeons and
/// the four hand-built interiors (house, cellar, arms, church). `None` when [`build_zone_with`]
/// refuses ([`ZoneError`]): a zone with no builder, a county whose skeleton rows cannot be
/// satisfied, or a seed none of whose attempts proves. Never panics.
pub fn build_zone(zone: ZoneId, seed: u32) -> Option<Blueprint> {
    build_zone_with(zone, seed, &mut |_| {}).ok()
}

/// Why a zone was not built for a seed. An unproven zone is never handed out: New Game re-rolls
/// to a seed that proves, and a chosen seed that does not is an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZoneError {
    /// No builder for this zone.
    NoBuilder(ZoneId),
    /// The county's skeleton rows cannot be satisfied at all.
    Skeleton(skeleton::SkeletonError),
    /// Every attempt was refused by the solver (or, for a dungeon, by checks C1 to C13).
    Unproven(ZoneId),
}

impl From<skeleton::SkeletonError> for ZoneError {
    fn from(e: skeleton::SkeletonError) -> Self {
        ZoneError::Skeleton(e)
    }
}

impl core::fmt::Display for ZoneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ZoneError::NoBuilder(z) => write!(f, "zone {} has no builder", z.name()),
            ZoneError::Skeleton(e) => write!(f, "the county's skeleton: {e}"),
            ZoneError::Unproven(z) => write!(f, "zone {} did not prove in any attempt", z.name()),
        }
    }
}

impl core::error::Error for ZoneError {}

/// What a build says as each of its stages starts: `"skeleton"`, a county stage's name
/// ([`county::STAGES`]), `"solve"`, or a zone's own name (`"house"`, `"mine"`). The loading
/// screen listens (PRESENTATION.md §3.2); nothing that is built depends on who listens.
pub type Report<'a> = &'a mut dyn FnMut(&'static str);

/// Every name a build of all thirteen zones reports ([`Report`]), in the order a first attempt
/// reports them. A county re-rolled says its stages again.
pub fn build_stages() -> Vec<&'static str> {
    let mut out = vec!["skeleton"];
    out.extend(county::STAGES.iter().map(|&(name, _)| name));
    out.push("solve");
    out.extend(ZoneId::ALL.iter().filter(|&&z| z != ZoneId::County).map(|z| z.name()));
    out
}

/// [`build_zone`], saying each stage to `report` as it starts, and why a zone was refused.
pub fn build_zone_with(zone: ZoneId, seed: u32, report: Report<'_>) -> Result<Blueprint, ZoneError> {
    let mut bp = build_zone_loose(zone, seed, report)?;
    bp.shrink_to_fit();
    Ok(bp)
}

/// [`build_zone_with`] before the blueprint gives back its spare capacity.
fn build_zone_loose(zone: ZoneId, seed: u32, report: Report<'_>) -> Result<Blueprint, ZoneError> {
    if zone == ZoneId::County {
        return county::build_proven_with(seed, report);
    }
    report(zone.name());
    if interiors::is_interior(zone) {
        return interiors::build_interior(zone, seed).ok_or(ZoneError::Unproven(zone));
    }
    if jane_data::catalog().dungeons.mission_of(zone).is_none() {
        return Err(ZoneError::NoBuilder(zone));
    }
    let built = dungeon::build(zone, seed);
    if built.info.errors.is_empty() { Ok(built.blueprint) } else { Err(ZoneError::Unproven(zone)) }
}

/// Is this a zone [`build_zone`] can build today?
pub fn builds(zone: ZoneId) -> bool {
    zone == ZoneId::County || interiors::is_interior(zone) || jane_data::catalog().dungeons.mission_of(zone).is_some()
}
