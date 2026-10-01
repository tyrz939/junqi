//! PORT.md §6.m stages 7 (the placement rows, the dressed patches) and 9 (scatter, the way cut
//! through, the unreachable dropped). Carries the placement and patch parts of
//! `jane/test/county.test.ts`, `town.test.ts` and `quest-audit.test.ts` that need no stories. Over
//! `SEEDS` seeds (64 by default), every seed's county built once on every core and every check run
//! on it:
//!
//! - every placement row not at a story's place lands: its keys, what it hides, its mark and rect,
//!   an edit's prop (the county contract, less the stories' names);
//! - the solver finds nothing missing that a placement row or a dressed patch answers;
//! - what a row places stands where its row says: near its named place, in no wall, a creature at
//!   the threat of its ground, a thing moved beside another touching it, a slot's thing on the slot;
//! - every dressed patch the skeleton placed has its marks, rects and slots;
//! - every named mark can be walked to from the platform (the way cut through);
//! - scatter lands on open, unclaimed ground only, and no rolled small place or creature is left
//!   where nobody can reach;
//! - the same seed builds the same county.

mod common;

use std::sync::OnceLock;

use jane_core::search::{Conn, Reach, flood};
use jane_core::{Blueprint, Key, NameId, Rect, ZoneId};
use jane_data::{PlaceAt, PlacementDef, PropTemplate};
use jane_world::county::placements::{PoiSpot, Stage, apply_placements, claim_pois, threat_at};
use jane_world::county::{County, STAGES, build_county, county_skeleton};
use jane_world::skeleton::Skeleton;
use jane_world::solve::{Report, SolveError, ZoneRules, validate};

fn seed_list() -> Vec<u32> {
    (0..common::seeds()).map(|i| i.wrapping_mul(2654435761)).collect()
}

type Check = fn(&Skeleton, &County<'_>, &Report, &mut Vec<String>);

const CHECKS: &[(&str, Check)] = &[
    ("rows", every_row_lands),
    ("solver", solver_finds_them),
    ("where", things_stand_where_their_rows_say),
    ("areas", patches_are_dressed),
    ("reach", every_mark_is_reached),
    ("scatter", scatter_is_on_open_ground),
];

/// What each check complained of, by check, in seed order; and the solver's remaining county
/// errors, by kind, over every seed.
struct Verdicts {
    bad: Vec<Vec<String>>,
    remaining: Vec<String>,
}

fn verdicts() -> &'static Verdicts {
    static ALL: OnceLock<Verdicts> = OnceLock::new();
    ALL.get_or_init(|| {
        let seeds = seed_list();
        let threads = std::thread::available_parallelism().map_or(1, usize::from).clamp(1, 8);
        let per = seeds.len().div_ceil(threads).max(1);
        let parts: Vec<(Vec<Vec<String>>, Vec<String>)> = std::thread::scope(|s| {
            let jobs: Vec<_> = seeds
                .chunks(per)
                .map(|part| {
                    s.spawn(move || {
                        let mut bad = vec![Vec::new(); CHECKS.len()];
                        let mut remaining = Vec::new();
                        for &seed in part {
                            let sk = county_skeleton(seed, 0).expect("the catalog's rows build");
                            let mut c = County::new(&sk, 0);
                            for (_, stage) in STAGES {
                                stage(&mut c);
                            }
                            let report = validate(c.k.blueprint(), &ZoneRules::for_zone(ZoneId::County));
                            for (i, (_, check)) in CHECKS.iter().enumerate() {
                                check(&sk, &c, &report, &mut bad[i]);
                            }
                            remaining.extend(solver_kinds(c.k.blueprint(), &report));
                        }
                        (bad, remaining)
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().expect("a county builds")).collect()
        });
        let bad = (0..CHECKS.len()).map(|i| parts.iter().flat_map(|p| p.0[i].iter().cloned()).collect()).collect();
        let remaining = parts.into_iter().flat_map(|p| p.1).collect();
        Verdicts { bad, remaining }
    })
}

fn verdict(name: &str) {
    let i = CHECKS.iter().position(|(n, _)| *n == name).expect("a check of that name");
    let bad = &verdicts().bad[i];
    assert!(bad.is_empty(), "{} failures, the first: {:#?}", bad.len(), &bad[..bad.len().min(8)]);
}

fn name(n: NameId) -> &'static str {
    jane_data::catalog().name(n)
}

fn cell_of(c: jane_core::Cell) -> (i32, i32) {
    (i32::from(c.x), i32::from(c.y))
}

fn prop_at(bp: &Blueprint, n: NameId) -> Option<&jane_core::blueprint::PropSpawn> {
    bp.props.iter().find(|p| p.key == Key::Name(n))
}

fn unit_at(bp: &Blueprint, n: NameId) -> Option<&jane_core::blueprint::UnitSpawn> {
    bp.units.iter().find(|u| u.key == Key::Name(n))
}

fn size(def: jane_core::PropDefId) -> (i32, i32) {
    let row = jane_data::catalog().story.prop(def);
    (i32::from(row.w), i32::from(row.h))
}

/// The rows promised on every seed: not a story's.
fn contract_rows() -> impl Iterator<Item = &'static PlacementDef> {
    jane_data::catalog().county.placements.iter().filter(|r| !r.at_place())
}

/// A row at an anchor whose small place is not dressed yet: `small_places` (the other half of
/// PORT.md §6.m stage 7) clears an anchor's ground and leaves its mark, and until it has, a row
/// there may find the anchor inside a thicket or an outcrop. Such rows are held to the checks
/// once the anchor's mark is there, and are not before.
fn anchor_undressed(bp: &Blueprint, row: &PlacementDef) -> bool {
    let PlaceAt::Anchor(a) = row.at else { return false };
    !bp.marks.contains_key(&Key::Name(jane_data::catalog().county.anchor(a).id))
}

// --- every row lands ------------------------------------------------------------------------

#[test]
fn every_placement_row_not_at_a_story_place_lands() {
    verdict("rows");
}

fn every_row_lands(sk: &Skeleton, c: &County<'_>, _: &Report, bad: &mut Vec<String>) {
    let bp = c.k.blueprint();
    let s = sk.seed;
    for row in contract_rows().filter(|r| !anchor_undressed(bp, r)) {
        if let Some(e) = row.edit {
            let key = e.key.unwrap_or(row.key);
            match prop_at(bp, key) {
                Some(p) if e.def.is_none_or(|d| p.def == d) && e.locked.is_none_or(|l| p.locked == l) => {}
                _ => bad.push(format!("seed {s}: {}'s edit is not on it", name(row.key))),
            }
            continue;
        }
        for &k in row.keys {
            if row.unit.is_some() && unit_at(bp, k).is_none() {
                bad.push(format!("seed {s}: no unit {}", name(k)));
            }
            if row.prop.is_some() && prop_at(bp, k).is_none() {
                bad.push(format!("seed {s}: no prop {}", name(k)));
            }
        }
        for h in row.hides.iter().filter(|h| !prop_at(bp, h.key).is_some_and(|p| p.hidden)) {
            bad.push(format!("seed {s}: nothing hidden as {}", name(h.key)));
        }
        for m in row.mark.iter().filter(|&&m| !bp.marks.contains_key(&Key::Name(m))) {
            bad.push(format!("seed {s}: no mark {}", name(*m)));
        }
        for r in row.rect.iter().filter(|r| !bp.rects.contains_key(&Key::Name(r.name))) {
            bad.push(format!("seed {s}: no rect {}", name(r.name)));
        }
    }
}

// --- the solver -----------------------------------------------------------------------------

/// The names a placement row (not a story's, nor one waiting on its anchor) or a dressed patch
/// provides.
fn answered_here(bp: &Blueprint, k: Key) -> bool {
    let Key::Name(n) = k else { return false };
    let s = name(n);
    AREA_NAMES.iter().any(|(_, names)| names.contains(&s))
        || contract_rows().filter(|r| !anchor_undressed(bp, r)).any(|r| {
            r.keys.contains(&n)
                || r.edit.is_some_and(|e| e.key == Some(n))
                || r.hides.is_some_and(|h| h.key == n)
                || r.mark == Some(n)
                || r.rect.is_some_and(|x| x.name == n)
        })
}

/// Each error the solver reports, as its kind and, for a missing name, the name.
fn solver_kinds(bp: &Blueprint, report: &Report) -> Vec<String> {
    report
        .errors
        .iter()
        .map(|e| {
            let kind = format!("{e:?}").split(['(', ' ', '{']).next().unwrap_or("?").to_owned();
            match e {
                SolveError::MissingUnit(k)
                | SolveError::MissingProp(k)
                | SolveError::MissingMark(k)
                | SolveError::MissingRect(k)
                | SolveError::TriggerNoRect { rect: k, .. }
                | SolveError::PropUnreachable(k)
                | SolveError::UnitUnreachable(k)
                | SolveError::MarkUnreachable(k) => format!("{kind} {}", key_name(bp, *k)),
                _ => kind,
            }
        })
        .collect()
}

fn key_name(bp: &Blueprint, k: Key) -> String {
    match k {
        Key::Name(n) => name(n).to_owned(),
        Key::Local(i) => bp.local_names.get(i as usize).cloned().unwrap_or_default(),
    }
}

#[test]
fn the_solver_finds_nothing_missing_that_a_row_or_a_patch_answers() {
    verdict("solver");
}

fn solver_finds_them(sk: &Skeleton, c: &County<'_>, report: &Report, bad: &mut Vec<String>) {
    let bp = c.k.blueprint();
    for e in &report.errors {
        let (SolveError::MissingUnit(k)
        | SolveError::MissingProp(k)
        | SolveError::MissingMark(k)
        | SolveError::MissingRect(k)
        | SolveError::TriggerNoRect { rect: k, .. }) = *e
        else {
            continue;
        };
        if answered_here(bp, k) {
            bad.push(format!("seed {}: {}", sk.seed, e.show(bp)));
        }
    }
}

/// The solver's county errors that remain, by kind, over every seed: for the report (`cargo test
/// --test county_places -- --nocapture`). The names missing are the stories' and the later stages'.
#[test]
fn the_solvers_remaining_county_errors() {
    let mut tally: Vec<(String, usize)> = Vec::new();
    for k in &verdicts().remaining {
        match tally.iter_mut().find(|(n, _)| n == k) {
            Some(t) => t.1 += 1,
            None => tally.push((k.clone(), 1)),
        }
    }
    tally.sort();
    println!("remaining county errors over {} seeds (error, seeds):", common::seeds());
    for (e, n) in tally {
        println!("  {e}: {n}");
    }
}

// --- where things stand ---------------------------------------------------------------------

#[test]
fn what_a_row_places_stands_where_its_row_says() {
    verdict("where");
}

/// The cell a row's things were looked for round, and how far they may be from it.
fn named_place(sk: &Skeleton, c: &County<'_>, row: &PlacementDef) -> Option<((i32, i32), i32)> {
    let bp = c.k.blueprint();
    let within = |d: i32| row.within.map_or(d, i32::from);
    let centre = |m: i32| m * 16 + 8;
    Some(match row.at {
        PlaceAt::Mark(m) => (cell_of(bp.marks.get(&Key::Name(m))?.cell), within(8)),
        PlaceAt::Site(s) => {
            let b = c.chunks.iter().find(|ch| ch.site == s)?.bounds;
            ((b.x + b.w / 2, b.y + b.h / 2), within(b.w.max(b.h) / 2 + 4))
        }
        PlaceAt::Area(a) => {
            let a = sk.area(a)?;
            ((centre(a.mx), centre(a.my)), within(i32::from(a.def.radius) * 4 / 5))
        }
        PlaceAt::Anchor(a) => {
            let p = sk.pois.iter().find(|p| p.anchor == Some(a))?;
            ((centre(p.mx), centre(p.my) + 3), within(6))
        }
        PlaceAt::Prop(k) => {
            let q = prop_at(bp, k)?;
            let (w, h) = size(q.def);
            ((i32::from(q.cell.x) + w / 2, i32::from(q.cell.y) + h), within(3))
        }
        PlaceAt::Poi { .. } | PlaceAt::Slot(_) | PlaceAt::Place { .. } => return None,
    })
}

fn things_stand_where_their_rows_say(sk: &Skeleton, c: &County<'_>, _: &Report, bad: &mut Vec<String>) {
    let bp = c.k.blueprint();
    let s = sk.seed;
    let cat = jane_data::catalog();
    for row in contract_rows().filter(|r| r.edit.is_none() && !anchor_undressed(bp, r)) {
        if let PlaceAt::Slot(slot) = row.at {
            let at = c
                .chunks
                .iter()
                .find_map(|ch| ch.slot(slot))
                .or_else(|| c.area_slots.iter().find(|a| a.0 == slot).map(|a| a.1));
            if prop_at(bp, row.key).map(|p| cell_of(p.cell)) != at {
                bad.push(format!("seed {s}: {} is not on its slot {}", name(row.key), name(slot)));
            }
            continue;
        }
        let Some((at, within)) = named_place(sk, c, row) else { continue };
        let (w, h) = row.prop.map_or((1, 1), |t| size(t.def));
        // A spread row's things are each at a point of their own about the patch, then open ground
        // within six of that.
        let reach = if row.spread { within + 6 } else { within } + w.max(h) + 1;
        // Pell's numbered lamps are street lamps, moved to the verge of the road nearest them
        // (`county::stories`): within 24 of where their row put them, and 4 of the road.
        let lamp = ["lamp_12", "lamp_13", "lamp_15"].contains(&name(row.key));
        let reach = if lamp { reach + 28 } else { reach };
        let far = |(x, y): (i32, i32)| (x - at.0).abs().max((y - at.1).abs()) > reach;
        for &k in row.keys {
            let prop = prop_at(bp, k).map(|p| (cell_of(p.cell), p.def));
            let unit = unit_at(bp, k).map(|u| (cell_of(u.cell), u.phase));
            if let Some((p, def)) = prop {
                if row.beside.is_none() && far(p) {
                    bad.push(format!("seed {s}: {} at {p:?} is far from {at:?}", name(k)));
                }
                let (pw, ph) = size(def);
                if (0..ph).any(|j| (0..pw).any(|i| c.k.solid(p.0 + i, p.1 + j))) {
                    bad.push(format!("seed {s}: {} at {p:?} stands in a wall", name(k)));
                }
                if let Some(b) = row.beside {
                    // What it is beside may itself wait on its anchor's ground (`anchor_undressed`).
                    if let Some(q) = prop_at(bp, b) {
                        let (qw, qh) = size(q.def);
                        let ring = Rect::new(i32::from(q.cell.x), i32::from(q.cell.y), qw, qh).grow(3);
                        if !ring.contains(p.0, p.1) {
                            bad.push(format!("seed {s}: {} at {p:?} is not beside {}", name(k), name(b)));
                        }
                    }
                }
            }
            if let Some((u, phase)) = unit {
                if row.prop.is_none() && far(u) {
                    bad.push(format!("seed {s}: {} at {u:?} is far from {at:?}", name(k)));
                }
                if c.k.solid(u.0, u.1) {
                    bad.push(format!("seed {s}: {} at {u:?} stands in a wall", name(k)));
                }
                let want = row.unit.and_then(|x| x.phase).unwrap_or_else(|| threat_at(sk, u.0, u.1));
                if phase != if want > 1 { want } else { 0 } {
                    bad.push(format!("seed {s}: {} plays at {phase}, its ground at {want}", name(k)));
                }
            }
        }
        if let Some(r) = row.rect {
            let got = bp.rects.get(&Key::Name(r.name)).copied();
            let (rw, rh) = (i32::from(r.w), i32::from(r.h));
            if got != Some(Rect::new(at.0 - rw / 2, at.1 - rh / 2, rw, rh)) {
                bad.push(format!("seed {s}: rect {} is not centred on its place", name(r.name)));
            }
        }
    }
    // town.test.ts: the text says "in the yard of the Arms": the sheet is inside the yard's rails.
    let sheet = cat.name_id("washing_arms").and_then(|n| prop_at(bp, n));
    let yard = cat.name_id("arms_yard").and_then(|n| bp.marks.get(&Key::Name(n)));
    match (sheet, yard) {
        (Some(p), Some(m)) if (p.cell.x.abs_diff(m.cell.x) <= 7 && i32::from(p.cell.y) - i32::from(m.cell.y) <= 3) => {}
        _ => bad.push(format!("seed {s}: the sheet is not in the Arms yard")),
    }
}

// --- the dressed patches ----------------------------------------------------------------------

/// What each dressed patch leaves, by its id: its marks and rects (`areas.ts`).
const AREA_NAMES: &[(&str, &[&str])] = &[
    ("allotments", &["plot_nine", "plot_nine_stake", "allotment_shed"]),
    ("top_field", &["top_field"]),
    ("quarry_steps", &["quarry_top", "quarry_adit"]),
];

#[test]
fn every_dressed_patch_leaves_its_marks_rects_and_slots() {
    verdict("areas");
}

fn patches_are_dressed(sk: &Skeleton, c: &County<'_>, _: &Report, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let key = |s: &str| {
        cat.name_id(s)
            .map(Key::Name)
            .or_else(|| bp.local_names.iter().position(|n| n == s).map(|i| Key::Local(i as u32)))
    };
    for (id, names) in AREA_NAMES {
        let Some(row) = cat.county.areas.iter().position(|a| cat.name(a.id) == *id) else {
            bad.push(format!("no area row {id}"));
            continue;
        };
        if sk.area(row as u8).is_none() {
            if cat.county.areas[row].required {
                bad.push(format!("seed {}: the required patch {id} was not placed", sk.seed));
            }
            continue;
        }
        for n in *names {
            if !key(n).is_some_and(|k| bp.marks.contains_key(&k)) {
                bad.push(format!("seed {}: {id} has no mark {n}", sk.seed));
            }
        }
    }
    for r in ["plot_nine", "quarry_top"] {
        if !key(r).is_some_and(|k| bp.rects.contains_key(&k)) {
            bad.push(format!("seed {}: no rect {r}", sk.seed));
        }
    }
    let adit = cat.name_id("quarry_adit").expect("the adit's slot is a name");
    if !c.area_slots.iter().any(|a| a.0 == adit) {
        bad.push(format!("seed {}: no quarry_adit slot", sk.seed));
    }
}

// --- reachability -----------------------------------------------------------------------------

/// Every cell a walker reaches from the platform, four ways over tiles that do not stop feet (the
/// same set as eight ways without cutting a corner, the TypeScript test's rule).
fn from_start(bp: &Blueprint) -> Option<Reach> {
    let start = jane_data::catalog().name_id("start")?;
    let m = bp.marks.get(&Key::Name(start))?;
    let mut reach = Reach::new();
    let solid = |x: i32, y: i32| bp.tiles.read(x, y, jane_core::Tile::Void).flags() & jane_core::tile::F_SOLID != 0;
    flood(bp.w(), bp.h(), &[cell_of(m.cell)], Conn::Four, u32::MAX, |x, y| !solid(x, y), &mut reach);
    Some(reach)
}

#[test]
fn every_named_mark_can_be_walked_to_from_the_platform() {
    verdict("reach");
}

fn every_mark_is_reached(sk: &Skeleton, c: &County<'_>, _: &Report, bad: &mut Vec<String>) {
    let bp = c.k.blueprint();
    let Some(reach) = from_start(bp) else {
        bad.push(format!("seed {}: no start", sk.seed));
        return;
    };
    for (k, m) in &bp.marks {
        let (x, y) = cell_of(m.cell);
        if !reach.reached(x, y) {
            let n = match *k {
                Key::Name(n) => name(n).to_owned(),
                Key::Local(_) => c.k.local_name(*k).unwrap_or("?").to_owned(),
            };
            bad.push(format!("seed {}: mark {n} at ({x}, {y}) is not reached ({:?})", sk.seed, c.k.get(x, y)));
        }
    }
    // county.test.ts: the car is off every road and has no mark: somewhere beside it is reached.
    let car = jane_data::catalog().name_id("car_wreck").and_then(|n| prop_at(bp, n));
    match car {
        Some(p) => {
            let (x, y) = cell_of(p.cell);
            if !(-2..=4).any(|oy| (-2..=6).any(|ox| reach.reached(x + ox, y + oy))) {
                bad.push(format!("seed {}: nothing beside the car is reached", sk.seed));
            }
        }
        None => bad.push(format!("seed {}: no car", sk.seed)),
    }
    for u in &bp.units {
        let (x, y) = cell_of(u.cell);
        if u.phase != 0 && !reach.reached(x, y) {
            bad.push(format!("seed {}: a creature at ({x}, {y}) nobody can reach", sk.seed));
        }
    }
}

// --- scatter ----------------------------------------------------------------------------------

#[test]
fn scatter_lands_on_open_ground() {
    verdict("scatter");
}

fn scatter_is_on_open_ground(sk: &Skeleton, c: &County<'_>, _: &Report, bad: &mut Vec<String>) {
    let cat = jane_data::catalog();
    let bp = c.k.blueprint();
    let herb = cat.story.prop_id("herb").expect("a herb row");
    let rock = cat.story.prop_id("rock").expect("a rock row");
    let (mut herbs, mut rocks) = (0, 0);
    for p in &bp.props {
        if p.def != herb && p.def != rock {
            continue;
        }
        let (x, y) = cell_of(p.cell);
        let t = c.k.get(x, y);
        if c.k.solid(x, y) || matches!(t, jane_core::Tile::Water | jane_core::Tile::Road) {
            bad.push(format!("seed {}: a {} at ({x}, {y}) on {t:?}", sk.seed, cat.story.prop(p.def).id));
        }
        if p.def == herb {
            herbs += 1;
            if p.loot.len() != 1 {
                bad.push(format!("seed {}: a herb at ({x}, {y}) holds {:?}", sk.seed, p.loot));
            }
        } else {
            rocks += 1;
        }
    }
    // Most throws land: the county is mostly open ground.
    if herbs < 100 || rocks < 40 {
        bad.push(format!("seed {}: {herbs} herbs and {rocks} rocks", sk.seed));
    }
}

// --- the same seed ----------------------------------------------------------------------------

#[test]
fn the_same_seed_builds_the_same_county() {
    for seed in seed_list().into_iter().take(common::seeds().min(3) as usize) {
        let a = build_county(seed, 0).expect("builds");
        let b = build_county(seed, 0).expect("builds");
        assert!(a == b, "seed {seed}: two builds differ");
    }
}

// --- kinds of small place ---------------------------------------------------------------------

/// `county.test.ts`'s rows at a kind of small place (no data row asks for one yet): two rows that
/// ask for the scarecrow nearest the farm share one; if the seed rolled no scarecrow, the nearest
/// small place becomes one; a rect is centred on it and a mark left below the thing.
#[test]
fn a_kind_of_small_place_is_claimed_once_and_shared() {
    let cat = jane_data::catalog();
    let n = name_of;
    // Names content uses elsewhere, borrowed as keys: a test cannot intern names of its own.
    let (pocket, again, field, mark) = (n("entry"), n("front"), n("platform"), n("stair_a"));
    let chest = cat.story.prop_id("chest").expect("a chest row");
    let sign = cat.story.prop_id("sign").expect("a sign row");
    let at = PlaceAt::Poi { kind: n("scarecrow"), near: cat.county.site_ix("farm") };
    let mut first = probe_row(pocket, &[pocket], at, chest);
    first.rect = Some(jane_data::PlacedRect { name: field, w: 24, h: 24 });
    first.mark = Some(mark);
    let rows = [first, probe_row(again, &[again], at, sign)];
    for seed in seed_list().into_iter().take(common::seeds().min(3) as usize) {
        let sk = county_skeleton(seed, 0).expect("builds");
        let mut c = County::new(&sk, 0);
        for (name, stage) in STAGES {
            stage(&mut c);
            if *name == "small_places" {
                break;
            }
        }
        let mut pois = PoiSpot::all(&sk);
        c.claimed = claim_pois(&sk, &mut pois, &rows);
        c.pois = pois;
        assert_eq!(c.claimed.len(), 2, "seed {seed}: both rows claim");
        assert_eq!(c.claimed[0].1, c.claimed[1].1, "seed {seed}: one scarecrow for both");
        let p = c.pois[c.claimed[0].1];
        assert_eq!(p.kind, Some(n("scarecrow")));
        assert_eq!(p.anchor, None, "an anchor is never handed out");
        apply_placements(&mut c, Stage::Pois, &rows);
        let bp = c.k.blueprint();
        let (a, b) = (prop_at(bp, pocket).expect("the chest"), prop_at(bp, again).expect("the sign"));
        let near = |q: &jane_core::blueprint::PropSpawn| {
            (i32::from(q.cell.x) - p.x).abs().max((i32::from(q.cell.y) - p.y - 3).abs()) <= 7
        };
        assert!(near(a) && near(b), "seed {seed}: both at the scarecrow at ({}, {})", p.x, p.y);
        assert_eq!(bp.rects.get(&Key::Name(field)), Some(&Rect::new(p.x - 12, p.y + 3 - 12, 24, 24)));
        let m = bp.marks.get(&Key::Name(mark)).expect("the mark");
        assert_eq!((m.cell.x, m.cell.y), (a.cell.x, a.cell.y + size(chest).1 as u16));
    }
}

fn name_of(s: &str) -> NameId {
    jane_data::catalog().name_id(s).unwrap_or_else(|| panic!("no name {s}"))
}

/// A row a test writes: a prop of `def` for each of `keys` at `at`, nothing else set.
fn probe_row(key: NameId, keys: &[NameId], at: PlaceAt, def: jane_core::PropDefId) -> PlacementDef {
    let template = PropTemplate {
        def,
        locked: false,
        key_tag: None,
        hidden: false,
        on: false,
        loot: &[],
        use_list: None,
        release: None,
        needs: &[],
        talk: None,
        label: None,
        night_lock: None,
    };
    PlacementDef {
        key,
        keys: Box::leak(keys.to_vec().into_boxed_slice()),
        spread: false,
        at,
        within: None,
        dx: None,
        dy: None,
        on: None,
        room: None,
        edit: None,
        unit: None,
        prop: Some(template),
        rect: None,
        mark: None,
        hides: None,
        beside: None,
    }
}

// --- streams ------------------------------------------------------------------------------------

/// A county built with `extra` placement rows after the catalog's in the patches' stage.
fn county_with(sk: &Skeleton, extra: &[PlacementDef]) -> Blueprint {
    let mut c = County::new(sk, 0);
    for (name, stage) in STAGES {
        stage(&mut c);
        if *name == "place_areas" {
            apply_placements(&mut c, Stage::Areas, extra);
        }
    }
    c.done()
}

/// `streams.test.ts`: one more placement row moves nothing but what it put down. Three crates spread
/// about a patch throw dice for where each goes, early in the build, before the scatter and the
/// wildlife; on one shared stream that reshuffled all of them. Nothing a macro cell or more from a
/// crate moves, and only a few things near one (a herb whose cell a crate now stands on).
#[test]
fn one_more_placement_row_moves_nothing_but_what_it_put_down() {
    let cat = jane_data::catalog();
    let field = cat.county.areas.iter().position(|a| cat.name(a.id) == "top_field").expect("a top_field row") as u8;
    let crate_ = cat.story.prop_id("crate").expect("a crate row");
    let keys = [name_of("entry"), name_of("front"), name_of("stair_a")];
    let mut row = probe_row(name_of("platform"), &keys, PlaceAt::Area(field), crate_);
    row.spread = true;
    row.within = Some(30);
    // By name, not by key: a generator-made key is its index in the order names were first made, and
    // one more row early in the build renumbers every one made after it.
    let sig = |bp: &Blueprint| -> Vec<(String, u16, u16)> {
        let mut v: Vec<_> = bp
            .props
            .iter()
            .map(|p| (key_name(bp, p.key), p.cell.x, p.cell.y))
            .chain(bp.units.iter().map(|u| (key_name(bp, u.key), u.cell.x, u.cell.y)))
            .collect();
        v.sort();
        v
    };
    let mut tried = 0;
    for seed in seed_list() {
        let sk = county_skeleton(seed, 0).expect("builds");
        if sk.area(field).is_none() {
            continue;
        }
        let (a, b) = (county_with(&sk, &[]), county_with(&sk, std::slice::from_ref(&row)));
        let probes: Vec<(i32, i32)> =
            b.props.iter().filter(|p| keys.iter().any(|&k| p.key == Key::Name(k))).map(|p| cell_of(p.cell)).collect();
        assert_eq!(probes.len(), 3, "seed {seed}: the new row placed its crates");
        let (sa, sb) = (sig(&a), sig(&b));
        let differ: Vec<_> = sa
            .iter()
            .filter(|t| sb.binary_search(t).is_err())
            .chain(sb.iter().filter(|t| sa.binary_search(t).is_err() && !keys.iter().any(|&k| t.0 == name(k))))
            .collect();
        let far: Vec<_> = differ
            .iter()
            .filter(|t| probes.iter().all(|p| (p.0 - i32::from(t.1)).abs().max((p.1 - i32::from(t.2)).abs()) > 32))
            .collect();
        assert!(far.is_empty(), "seed {seed}: moved far from the crates: {far:?}");
        assert!(differ.len() < 8, "seed {seed}: the crates displaced {}", differ.len());
        tried += 1;
        if tried == common::seeds().min(2) {
            break;
        }
    }
    assert!(tried > 0, "no seed placed the Top Field");
}
