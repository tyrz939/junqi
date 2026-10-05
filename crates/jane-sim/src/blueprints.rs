//! The thirteen blueprints of a seed, built up front (ARCHITECTURE.md §9: New Game builds every
//! zone behind a loading screen; the sim is handed finished blueprints and never builds
//! mid-play). A blueprint is a pure function of `(zone, seed)` and never saved; the save holds
//! the seed and what changed since.
//!
//! Shared by `Arc` so a test, a bench or a lockstep peer can hand one set to several sims.

use std::sync::Arc;

use jane_core::{Blueprint, ZONE_COUNT, ZoneId};

#[derive(Clone, Debug)]
pub struct Blueprints {
    seed: u32,
    zones: [Arc<Blueprint>; ZONE_COUNT],
}

/// A zone was refused for this seed (`jane_world::ZoneError`): no builder, or no attempt proved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildError(pub ZoneId, pub jane_world::ZoneError);

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "zone {} did not build: {}", self.0.name(), self.1)
    }
}

impl std::error::Error for BuildError {}

/// One zone's blueprint, proven (`jane_world::build_zone`). An unproven zone is an error, never
/// played.
pub fn build_one(zone: ZoneId, seed: u32) -> Result<Blueprint, BuildError> {
    build_one_with(zone, seed, &mut |_| {})
}

/// [`build_one`], saying each stage to `report` as it starts (`jane_world::Report`).
pub fn build_one_with(zone: ZoneId, seed: u32, report: jane_world::Report<'_>) -> Result<Blueprint, BuildError> {
    jane_world::build_zone_with(zone, seed, report).map_err(|e| BuildError(zone, e))
}

/// How many seeds [`Blueprints::build_rerolled`] tries before it gives up.
pub const REROLLS: u32 = 64;

/// The seed tried after `seed` when it is refused (a New Game nobody chose a seed for).
pub fn next_seed(seed: u32) -> u32 {
    // Weyl step: every u32 is visited before one repeats.
    seed.wrapping_add(0x9E37_79B9)
}

impl Blueprints {
    pub fn build(seed: u32) -> Result<Self, BuildError> {
        Self::build_with(seed, &mut |_| {})
    }

    /// [`Blueprints::build`], saying each stage to `report` as it starts: what the loading
    /// screen hears (PRESENTATION.md §3.2). The blueprints are the same whoever listens.
    pub fn build_with(seed: u32, report: jane_world::Report<'_>) -> Result<Self, BuildError> {
        let mut built: Vec<Arc<Blueprint>> = Vec::with_capacity(ZONE_COUNT);
        for z in ZoneId::ALL {
            built.push(Arc::new(build_one_with(z, seed, report)?));
        }
        let zones: [Arc<Blueprint>; ZONE_COUNT] = built.try_into().unwrap_or_else(|_| unreachable!("thirteen zones"));
        Ok(Self { seed, zones })
    }

    /// [`Blueprints::build_with`] for a seed nobody chose (New Game from the clock): a seed that
    /// does not prove is set aside for [`next_seed`], up to [`REROLLS`] times. The blueprints
    /// carry the seed that proved ([`Blueprints::seed`]); the error is the last seed's.
    pub fn build_rerolled(seed: u32, report: jane_world::Report<'_>) -> Result<Self, BuildError> {
        let mut seed = seed;
        let mut tries = 1;
        loop {
            match Self::build_with(seed, report) {
                Err(BuildError(_, jane_world::ZoneError::Unproven(_))) if tries < REROLLS => {
                    seed = next_seed(seed);
                    tries += 1;
                }
                r => return r,
            }
        }
    }

    /// Blueprints made some other way (a harness playing one room as the whole zone). Every
    /// `zones[i].zone` must be `ZoneId::ALL[i]`.
    pub fn from_parts(seed: u32, zones: [Arc<Blueprint>; ZONE_COUNT]) -> Self {
        for (i, bp) in zones.iter().enumerate() {
            assert_eq!(bp.zone, ZoneId::ALL[i], "blueprint {i} is for {:?}", bp.zone);
        }
        Self { seed, zones }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn get(&self, z: ZoneId) -> &Arc<Blueprint> {
        &self.zones[z.index()]
    }
}
