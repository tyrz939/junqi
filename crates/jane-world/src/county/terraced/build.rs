//! The terraced county's build (PLAN-map-r4 section 1), from its source (`data/world/county/`,
//! [`super::source`]): the source's macro plan carved, its skeleton stood where the source says, laid by
//! the county's stages (the roads, footpaths and stories where the source says, the dressing on the
//! source's dice) and proved. Nothing is searched for or re-rolled: a source that does not hold is
//! refused, and `jane bake-world` writes nothing. Same source, same county, on every target.

use alloc::boxed::Box;
use alloc::vec::Vec;
use jane_core::{Blueprint, ZoneId};

use super::carve::{CarveFail, Carved, W, carve_proven};
use super::prove::{Proof, Refusal, prove};
use super::roads::{lay_paths, lay_roads};
use super::skel::{Refused, skeleton_from};
use super::source::Source;
use super::stage::{carve_tiles, lay_rail, repair_pits};
use crate::Report;
use crate::county::{County, STAGES, StageFn};
use crate::skeleton::SkeletonRows;
use crate::solve::ZoneRules;

/// The county attempt the stages throw their dice on: always the first.
const COUNTY_ATTEMPT: u8 = 0;

/// The county as built: its blueprint, the carve it was laid on, and what the proofs found.
#[derive(Debug)]
pub struct Built {
    pub bp: Blueprint,
    pub carved: Carved,
    pub proof: Proof,
}

/// Why the source is not a county that holds: the carve refused its plan, the skeleton's things stand
/// where they cannot, or the county failed a proof.
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

/// The county laid over its carve and skeleton from `source`: every one of `stages`.
fn lay_on(
    carved: Carved,
    sk: &crate::skeleton::Skeleton,
    source: &Source,
    stages: &[(&'static str, StageFn)],
    report: Report<'_>,
    pack: bool,
) -> (Blueprint, Carved) {
    let mut c = County::new_sized(sk, COUNTY_ATTEMPT, pack, W, W);
    c.terraced = Some(Box::new(carved));
    c.source = Some(source);
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
    (bp, *carved)
}

/// The county from its source: the plan carved on the source's salt, the skeleton's things where the
/// source stands them, the roads, footpaths and stories laid where it says, and the proofs. Says
/// `"carve"`, `"skeleton"`, each stage, `"prove"` and `"solve"` to `report`. `pack` builds the console
/// form.
pub fn build(src: &Source, report: Report<'_>, pack: bool) -> Result<Built, Failure> {
    report("carve");
    let carved = carve_proven(src.plan.clone(), src.salt).map_err(Failure::Carve)?;
    report("skeleton");
    let sk =
        skeleton_from(&carved, &SkeletonRows::catalog(), &src.places, &src.routes.lit).map_err(Failure::Skeleton)?;
    let (bp, carved) = lay_on(carved, &sk, src, &stages(), report, pack);
    let proof = prove(&bp, &carved, &ZoneRules::for_zone(ZoneId::County), report).map_err(Failure::Proof)?;
    Ok(Built { bp, carved, proof })
}
