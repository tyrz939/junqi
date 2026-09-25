//! Generated dungeons (DUNGEONS.md §2, PORT.md §6.m stages 12 and 13): a mission compiled at
//! build (`jane_data::catalog().dungeons`), its room templates turned into every shape they
//! allow, and this module, which lays the mission onto the bay lattice and stamps a `Blueprint`.
//!
//! - [`build_candidate`] / [`build_dungeon`]: one candidate for `(zone, seed, attempt)`.
//! - [`build`] / [`build_with`]: the attempt loop over `ZONE_ATTEMPTS`; the last attempt spends
//!   the mission's hand-placed fallback, so a dungeon is never missing.
//!
//! **Validation hook.** Where the TypeScript called `validateBlueprint` and then `check` (C1 to
//! C12), `build_with` asks a [`Validate`]. The solver and the checks (`crate::solve`) plug in
//! there at integration; until then [`AcceptAll`] takes the first candidate that embedded.

pub mod bind;
pub mod generate;
pub mod layout;
pub mod lights;

use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::{Blueprint, ZoneId};

pub use generate::{BuildInfo, Built, ControlInfo, EdgeLock, LockinInfo, RoomInfo, build_candidate};
pub use layout::{Corridor, DoorUse, Layout};

/// Judges a candidate: `Err` with the reasons re-rolls it. **Integration hook**: the solver
/// (`validateBlueprint`) followed by checks C1-C12 (`checkDungeon`) go here.
pub trait Validate {
    fn validate(&self, built: &Built) -> Result<(), Vec<String>>;
}

impl<F: Fn(&Built) -> Result<(), Vec<String>>> Validate for F {
    fn validate(&self, built: &Built) -> Result<(), Vec<String>> {
        self(built)
    }
}

/// Takes every candidate that embedded. Stands in for the solver until it is wired in.
#[derive(Clone, Copy, Debug, Default)]
pub struct AcceptAll;

impl Validate for AcceptAll {
    fn validate(&self, _: &Built) -> Result<(), Vec<String>> {
        Ok(())
    }
}

/// One candidate's blueprint. Panics if `zone` is not a generated dungeon.
pub fn build_dungeon(zone: ZoneId, seed: u32, attempt: u8) -> Blueprint {
    build_candidate(zone, seed, attempt).blueprint
}

/// The dungeon for a seed: the first candidate that embeds (see [`build_with`]).
pub fn build(zone: ZoneId, seed: u32) -> Built {
    build_with(zone, seed, &AcceptAll)
}

/// The attempt loop (world/index.ts `buildZone`): candidates in attempt order until one embeds
/// and `v` accepts it. The last attempt is the mission's fallback; if even that is rejected it is
/// returned anyway, with the reasons in `info.errors`, because the player is never thrown at.
pub fn build_with(zone: ZoneId, seed: u32, v: &dyn Validate) -> Built {
    let mut attempt = 0;
    loop {
        let mut built = build_candidate(zone, seed, attempt);
        let verdict = if built.info.errors.is_empty() { v.validate(&built) } else { Err(Vec::new()) };
        match verdict {
            Ok(()) => return built,
            Err(errors) if attempt + 1 >= ZONE_ATTEMPTS => {
                built.info.errors.extend(errors);
                return built;
            }
            Err(_) => attempt += 1,
        }
    }
}
