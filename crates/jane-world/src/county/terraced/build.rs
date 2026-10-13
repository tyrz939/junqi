//! The terraced county's build (PLAN-map-r4 section 1): a macro plan, carved, read as a skeleton, laid by
//! the county's stages and proved. The county the game ships is one map picked by hand, so the build
//! takes a [`Pin`], the seed, the plan's attempt and the carve's salt, and gives that map or says why it
//! is not one that holds ([`build_pinned`]). [`find`] walks a seed's plans in order for the first that
//! does, to choose a pin from. Same pin, same county, on every target.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::{Blueprint, ZoneId};

use super::carve::{CarveFail, Carved, W, carve_from, carve_proven};
use super::macro_plan;
use super::prove::{Proof, Refusal, prove};
use super::roads::{lay_paths, lay_roads};
use super::skel::{Refused, skeleton_from, skeleton_of};
use super::source::Source;
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
    Ok(lay_on(carved, &sk, None, stages, report, pack, &mut |_, _| {}))
}

/// What a hook sees after each stage: the stage's name and the county as it stands.
pub type Hook<'h> = &'h mut dyn FnMut(&'static str, &County<'_>);

/// The county laid over a carve and its skeleton, from `source` where it has one.
fn lay_on(
    carved: Carved,
    sk: &crate::skeleton::Skeleton,
    source: Option<&Source>,
    stages: &[(&'static str, StageFn)],
    report: Report<'_>,
    pack: bool,
    hook: Hook<'_>,
) -> (Blueprint, Carved) {
    let mut c = County::new_sized(sk, COUNTY_ATTEMPT, pack, W, W);
    c.terraced = Some(Box::new(carved));
    c.source = source;
    for &(name, stage) in stages {
        report(name);
        stage(&mut c);
        hook(name, &c);
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

/// The county from its source (`data/world/county/`): the plan carved on the source's salt, the
/// skeleton's things where the source stands them, the roads, footpaths and stories laid where it says,
/// and the proofs. Says `"carve"`, `"skeleton"`, each stage, `"prove"` and `"solve"` to `report`.
pub fn build(src: &Source, report: Report<'_>, pack: bool) -> Result<Built, Failure> {
    report("carve");
    let carved = carve_proven(src.plan.clone(), src.salt).map_err(Failure::Carve)?;
    report("skeleton");
    let sk =
        skeleton_from(&carved, &SkeletonRows::catalog(), &src.places, &src.routes.lit).map_err(Failure::Skeleton)?;
    let pin = Pin { seed: src.plan.seed, plan: src.plan.attempt, salt: src.salt };
    let (bp, carved) = lay_on(carved, &sk, Some(src), &stages(), report, pack, &mut |_, _| {});
    let proof = prove(&bp, &carved, &ZoneRules::for_zone(ZoneId::County), report).map_err(Failure::Proof)?;
    Ok(Built { pin, bp, carved, proof })
}

/// The source the pinned map's searches found: `plan.txt`, `places.txt` and `routes.txt` as they would
/// read. Phase A of the world as data: written once, then the source is the county.
pub fn export_pinned(pin: Pin) -> Result<(String, String, String), Failure> {
    use super::source::{PathLine, Places, Routes, StoryAt, write_places, write_plan, write_routes};
    use crate::skeleton::ROAD_LIT;
    let plan = macro_plan::build(pin.seed, pin.plan);
    let carved = carve_proven(plan, pin.salt).map_err(Failure::Carve)?;
    let sk = skeleton_of(&carved, &SkeletonRows::catalog()).map_err(Failure::Skeleton)?;
    let cat = jane_data::catalog();
    let county = &cat.county;
    let mut places = Places::default();
    for a in &sk.areas {
        places.areas.push((a.row, (a.mx, a.my)));
    }
    for p in sk.pois.iter().filter(|p| p.anchor.is_none()) {
        let row = county.pois.iter().position(|d| Some(d.kind) == p.kind && Some(d.name) == p.name);
        places.pois.push((row.expect("a rolled small place is a row") as u8, (p.mx, p.my)));
    }
    for a in &sk.anchors {
        places.anchors.push((a.row, (a.mx, a.my)));
    }
    let mut routes = Routes::default();
    for y in 0..sk.road.h() as i32 {
        for x in 0..sk.road.w() as i32 {
            if sk.road.read(x, y, 0) & ROAD_LIT != 0 {
                routes.lit.push((x, y));
            }
        }
    }
    let ways = carved.ways.len();
    let names = crate::names::Names::new(sk.seed);
    let mut stories: Vec<StoryAt> = Vec::new();
    let mut hook = |stage: &'static str, c: &County<'_>| match stage {
        "paths" => {
            routes.roads = c.lines[..ways].to_vec();
            for (l, line) in c.lines.iter().enumerate().skip(ways) {
                let row = c.footpaths.iter().find(|f| f.line == l).map(|f| f.row as u8);
                routes.paths.push(PathLine { row, line: line.clone() });
            }
        }
        "stories" => {
            for s in county.stories {
                let at = c.story_claims.place_of(s.id).map(|i| (c.places[i].bounds.x, c.places[i].bounds.y));
                let name = s.name.is_none().then(|| names.story(s.id));
                stories.push(StoryAt { story: s.id, at, name });
            }
        }
        _ => {}
    };
    let plan = carved.plan.clone();
    let _ = lay_on(carved, &sk, None, &stages(), &mut |_| {}, false, &mut hook);
    places.stories = stories;
    Ok((write_plan(&plan, pin.salt), write_places(&places), write_routes(&routes, &plan)))
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
