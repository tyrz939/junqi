//! The terraced county's build (PLAN-map-r4 section 1): a macro plan, carved, read as a skeleton, laid by
//! the county's stages and proved. The county the game ships is one map picked by hand, so the build
//! takes a [`Pin`], the seed, the plan's attempt and the carve's salt, and gives that map or says why it
//! is not one that holds ([`build_pinned`]). [`find`] walks a seed's plans in order for the first that
//! does, to choose a pin from. Same pin, same county, on every target.

use alloc::boxed::Box;
use alloc::vec::Vec;
use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::{Blueprint, ZoneId};

use super::carve::{CarveFail, Carved, W, carve_from, carve_proven};
use super::macro_plan;
use super::prove::{Proof, Refusal, prove};
use super::roads::{lay_paths, lay_roads};
use super::skel::{Refused, skeleton_of};
use super::stage::{carve_tiles, lay_rail, repair_pits};
use crate::county::{County, STAGES, StageFn};
use crate::skeleton::SkeletonRows;
use crate::solve::ZoneRules;
use crate::{Report, ZoneError};

/// The county attempt the stages throw their dice on. A pinned map is the same map whatever else was
/// tried before it, so it is always the first.
const COUNTY_ATTEMPT: u8 = 0;

/// A map pinned: the seed, the macro plan's attempt (the roll of the plan that is kept, not how many
/// were tried to reach it) and the carve's salt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    pub seed: u32,
    pub plan: u8,
    pub salt: u8,
}

/// A pinned map as built: the county, the carve it was laid on, and what the proofs found.
#[derive(Debug)]
pub struct Built {
    pub pin: Pin,
    pub bp: Blueprint,
    pub carved: Carved,
    pub proof: Proof,
}

/// Why a pinned map is not a map that holds: the carve refused the plan on that salt, the skeleton found
/// no room for something the story needs, or the county failed a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Carve(CarveFail),
    Skeleton(Refused),
    Proof(Refusal),
}

/// The county's stages over a carve: the plain county's, with the carve's tiles laid after the edge, the
/// roads, the paths and the railway made over it, and the pits mended before the cut through thicket.
pub fn stages() -> Vec<(&'static str, StageFn)> {
    let mut out: Vec<(&'static str, StageFn)> = Vec::with_capacity(STAGES.len() + 2);
    for &(name, stage) in STAGES {
        match name {
            "edge" => {
                out.push((name, stage));
                out.push(("carve", carve_tiles));
            }
            "roads" => out.push((name, lay_roads)),
            "paths" => out.push((name, lay_paths)),
            "rail" => out.push((name, lay_rail)),
            "cut_through" => {
                out.push(("pits", repair_pits));
                out.push((name, stage));
            }
            _ => out.push((name, stage)),
        }
    }
    out
}

/// The county laid over a carve, unproven: its skeleton, then every one of `stages`. Gives back the
/// blueprint and the carve it was laid on.
pub fn lay_county(
    carved: Carved,
    rows: &SkeletonRows<'_>,
    stages: &[(&'static str, StageFn)],
    report: Report<'_>,
    pack: bool,
) -> Result<(Blueprint, Carved), Refused> {
    report("skeleton");
    let sk = skeleton_of(&carved, rows)?;
    let mut c = County::new_sized(&sk, COUNTY_ATTEMPT, pack, W, W);
    c.terraced = Some(Box::new(carved));
    for &(name, stage) in stages {
        report(name);
        stage(&mut c);
        c.release_after(name);
    }
    let carved = c.terraced.take().expect("the carve is kept to the end");
    let mut bp = c.done_as(pack);
    if pack {
        bp.shrink_to_fit();
        bp.pack();
    }
    Ok((bp, *carved))
}

/// A carve laid and proved.
fn lay_and_prove(carved: Carved, report: Report<'_>, pack: bool) -> Result<Built, Failure> {
    let pin = Pin { seed: carved.plan.seed, plan: carved.plan.attempt, salt: carved.salt };
    let (bp, carved) =
        lay_county(carved, &SkeletonRows::catalog(), &stages(), report, pack).map_err(Failure::Skeleton)?;
    let proof = prove(&bp, &carved, &ZoneRules::for_zone(ZoneId::County), report).map_err(Failure::Proof)?;
    Ok(Built { pin, bp, carved, proof })
}

/// The map `pin` names, built and proved. Says `"plan"`, `"carve"` and `"skeleton"` to `report`, each
/// stage's name, `"prove"` and `"solve"` as each starts. `pack` builds the console form: packed before its
/// proofs.
pub fn build_pinned(pin: Pin, report: Report<'_>, pack: bool) -> Result<Built, Failure> {
    report("plan");
    let plan = macro_plan::build(pin.seed, pin.plan);
    report("carve");
    let carved = carve_proven(plan, pin.salt).map_err(Failure::Carve)?;
    lay_and_prove(carved, report, pack)
}

/// The first map of `seed` that holds, walking its plans in order (each on the first salt the carve's checks
/// take): a pin to start a hand pick from. A plan the carve or the skeleton cannot take is passed over; at most
/// [`ZONE_ATTEMPTS`] counties are laid.
pub fn find(seed: u32, report: Report<'_>, pack: bool) -> Result<Built, ZoneError> {
    let mut from = 0u8;
    let mut laid = 0;
    while laid < ZONE_ATTEMPTS {
        let Ok(carved) = carve_from(seed, from, report).0 else { break };
        from = carved.plan.attempt.saturating_add(1);
        match lay_and_prove(carved, report, pack) {
            Ok(built) => return Ok(built),
            Err(Failure::Skeleton(Refused::Rows(e))) => return Err(e.into()),
            Err(Failure::Skeleton(_)) => {}
            Err(_) => laid += 1,
        }
    }
    Err(ZoneError::Unproven(ZoneId::County))
}

/// The terraced county of `seed`, proven: the first map [`find`] gives.
pub fn build_proven_as(seed: u32, report: Report<'_>, pack: bool) -> Result<Blueprint, ZoneError> {
    find(seed, report, pack).map(|built| built.bp)
}
