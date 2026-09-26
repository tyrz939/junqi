//! The thirteen blueprints of a seed, built up front (ARCHITECTURE.md §9: New Game builds every
//! zone behind a loading screen; the sim is handed finished blueprints and never builds
//! mid-play). A blueprint is a pure function of `(zone, seed)` and never saved; the save holds
//! the seed and what changed since.
//!
//! Shared by `Arc` so a test, a bench or a lockstep peer can hand one set to several sims.

use std::sync::Arc;

use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::{Blueprint, ZONE_COUNT, ZoneId};

#[derive(Clone, Debug)]
pub struct Blueprints {
    seed: u32,
    zones: [Arc<Blueprint>; ZONE_COUNT],
}

/// A zone's builder gave nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildError(pub ZoneId);

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "zone {} did not build", self.0.name())
    }
}

impl std::error::Error for BuildError {}

/// One zone's blueprint. The county comes from `jane_world::county::build_county` directly
/// while `build_zone` still says `None` for it (its last stages are being ported), re-rolled
/// through its attempts as `buildZone` did.
pub fn build_one(zone: ZoneId, seed: u32) -> Result<Blueprint, BuildError> {
    if let Some(bp) = jane_world::build_zone(zone, seed) {
        return Ok(bp);
    }
    if zone == ZoneId::County {
        for attempt in 0..ZONE_ATTEMPTS {
            if let Ok(bp) = jane_world::county::build_county(seed, attempt) {
                return Ok(bp);
            }
        }
    }
    Err(BuildError(zone))
}

impl Blueprints {
    pub fn build(seed: u32) -> Result<Self, BuildError> {
        let mut built: Vec<Arc<Blueprint>> = Vec::with_capacity(ZONE_COUNT);
        for z in ZoneId::ALL {
            built.push(Arc::new(build_one(z, seed)?));
        }
        let zones: [Arc<Blueprint>; ZONE_COUNT] = built.try_into().unwrap_or_else(|_| unreachable!("thirteen zones"));
        Ok(Self { seed, zones })
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
