//! Generated dungeons (DUNGEONS.md §2, PORT.md §6.m stages 12 to 16): a mission compiled at
//! build (`jane_data::catalog().dungeons`), its room templates turned into every shape they
//! allow, and this module, which lays the mission onto the bay lattice, stamps a `Blueprint`,
//! and proves it.
//!
//! - [`build_candidate`] / [`build_dungeon`]: one candidate for `(zone, seed, attempt)`.
//! - [`build`]: the attempt loop over `ZONE_ATTEMPTS`, each candidate judged by [`Proven`]: the
//!   lock-and-key solver (`crate::solve`), then checks C1 to C13 ([`checks`]). The last attempt
//!   spends the mission's hand-placed fallback, and is judged too; if even that is refused it is
//!   returned with the reasons in `info.errors`, because the player is never thrown at.
//! - [`build_with`]: the same loop with another judge ([`AcceptAll`] takes the first candidate
//!   that embedded: the generator's own tests use it).
//! - [`harness`]: every template proves its promises alone, from every door (stage 16).

pub mod bind;
pub mod checks;
pub mod generate;
pub mod harness;
pub mod layout;
pub mod lights;
pub mod sets;

use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::{Blueprint, ZoneId};

pub use generate::{
    BuildInfo, Built, ControlInfo, EdgeLock, LockinInfo, RoomInfo, build_candidate, build_mission, build_room_alone,
    door_mark,
};
pub use layout::{Corridor, DoorUse, Layout};

/// Judges a candidate: `Err` with the reasons re-rolls it.
pub trait Validate {
    fn validate(&self, built: &Built) -> Result<(), Vec<String>>;
}

impl<F: Fn(&Built) -> Result<(), Vec<String>>> Validate for F {
    fn validate(&self, built: &Built) -> Result<(), Vec<String>> {
        self(built)
    }
}

/// Takes every candidate that embedded: the generator judged on its own.
#[derive(Clone, Copy, Debug, Default)]
pub struct AcceptAll;

impl Validate for AcceptAll {
    fn validate(&self, _: &Built) -> Result<(), Vec<String>> {
        Ok(())
    }
}

/// The real judge (world/index.ts `buildZone`: `validateBlueprint`, then `check`): the solver
/// with the mission's contract, keys, spells and states, then C1 to C13.
#[derive(Clone, Copy, Debug, Default)]
pub struct Proven;

impl Validate for Proven {
    fn validate(&self, built: &Built) -> Result<(), Vec<String>> {
        let faults = checks::check_dungeon(built);
        if faults.is_empty() { Ok(()) } else { Err(faults.iter().map(ToString::to_string).collect()) }
    }
}

/// One candidate's blueprint. Panics if `zone` is not a generated dungeon.
pub fn build_dungeon(zone: ZoneId, seed: u32, attempt: u8) -> Blueprint {
    build_candidate(zone, seed, attempt).blueprint
}

/// The dungeon for a seed: the first candidate the solver and C1 to C13 accept.
pub fn build(zone: ZoneId, seed: u32) -> Built {
    build_with(zone, seed, &Proven)
}

/// The attempt loop (world/index.ts `buildZone`): candidates in attempt order until one embeds
/// and `v` accepts it. The last attempt is the mission's fallback; if even that is rejected it is
/// returned anyway, with the reasons in `info.errors`, because the player is never thrown at.
/// Every refused attempt is kept in `info.rejected` with its first reason.
pub fn build_with(zone: ZoneId, seed: u32, v: &dyn Validate) -> Built {
    let mut rejected = Vec::new();
    let mut attempt = 0;
    loop {
        let mut built = build_candidate(zone, seed, attempt);
        let verdict = if built.info.errors.is_empty() { v.validate(&built) } else { Err(built.info.errors.clone()) };
        match verdict {
            Ok(()) => {
                built.info.rejected = rejected;
                return built;
            }
            Err(errors) if attempt + 1 >= ZONE_ATTEMPTS => {
                if built.info.errors.is_empty() {
                    built.info.errors.extend(errors);
                }
                built.info.rejected = rejected;
                return built;
            }
            Err(errors) => {
                let first = errors.into_iter().next().unwrap_or_else(|| "refused".to_owned());
                rejected.push((attempt, first));
                attempt += 1;
            }
        }
    }
}
