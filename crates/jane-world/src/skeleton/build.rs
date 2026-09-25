//! `build_skeleton(seed)`: the county decided, in a few milliseconds, as data. Land, then the
//! story's places and the roads between them, then the railway, the patches, the lamps, the small
//! places and the anchors, then how dangerous each patch is. Then every row is checked against what
//! was built, and a skeleton that fails is thrown away and the next attempt tried: the player never
//! sees a county that breaks the story. Carries `jane/src/world/skeleton/index.ts`.
//!
//! Every stage has dice of its own, `(seed, Step, attempt, row)`, or a ranking of its own
//! (rank.rs): re-tune one and the others stand where they stood.

use std::fmt::Write as _;

use jane_core::grid::Grid;
use jane_core::num::isqrt;
use jane_core::{Sfc32, ZoneId};
use jane_data::{AnchorDef, AreaDef, County, DistBy, PoiDef, Region, SiteDef};

use super::anchors::{AnchorCtx, PlacedAnchor, place_anchors};
use super::place::{
    POI_BUDGET, PlaceCtx, PlacedArea, PlacedPoi, PlacedSite, Walk, blame, place_areas, place_pois, road_tenths,
    site_candidates, walk_key,
};
use super::rail::{RailCost, lay_rail, rail_mask};
use super::rank::{rank_base, rank_permille};
use super::roads::{
    FAR, Land, ROAD, ROAD_LIT, Road, RoadEnd, Router, count_bridges, distance_to, lay_road, road_distances,
};
use super::terrain::{Terrain, build_terrain};
use super::types::{MACRO, REGIONS, SKEL_H, SKEL_W, inside, metres_sq, nearer_than, region_name, xy_of};
use crate::steps::{Step, dice};

/// Attempts one build may take before it gives up and hands back its last full try.
pub const MAX_ATTEMPTS: u8 = 40;
/// Spots tried for one site before it steps back.
const SITE_TRIES: u32 = 14;
/// Steps back to an earlier site one attempt may take before it is given up.
const MAX_JUMPS: u32 = 8;
/// A dungeon's approach is part of the dungeon: threat rises by one inside this ring (m).
const DUNGEON_RING: i32 = 170;
/// The longest stretch of road with nothing to see from it (m). PLAN.md 2.4: "up to about two
/// minutes, used on purpose".
const LONGEST_EMPTY: u32 = 900;
/// A small place is seen from a road cell this near it (m); a site from 40 m further.
const SEEN_FROM_ROAD: i64 = 120;

/// The rows a skeleton is built from: the catalog's by default, any others in a test.
#[derive(Clone, Copy, Debug)]
pub struct SkeletonRows<'a> {
    pub sites: &'a [SiteDef],
    pub areas: &'a [AreaDef],
    pub pois: &'a [PoiDef],
    pub anchors: &'a [AnchorDef],
}

impl<'a> From<&'a County> for SkeletonRows<'a> {
    fn from(c: &'a County) -> Self {
        Self { sites: c.sites, areas: c.areas, pois: c.pois, anchors: c.anchors }
    }
}

impl SkeletonRows<'_> {
    /// Every reference between rows points where the builder can follow it: a site to an earlier
    /// site, a patch to a site, an anchor to a site, a patch or an earlier anchor. The catalog's rows
    /// are checked at build; this is for rows a test or a tool made.
    pub fn check(&self) -> Result<(), SkeletonError> {
        let bad = |what: String| Err(SkeletonError::BadRow(what));
        let (sites, areas) = (self.sites.len(), self.areas.len());
        for (i, s) in self.sites.iter().enumerate() {
            let earlier = |r: u8| usize::from(r) < i;
            if !s.road_from.is_none_or(earlier)
                || !s.at.across_river_from.is_none_or(earlier)
                || !s.dist.iter().all(|d| earlier(d.to))
            {
                return bad(format!("site {}: a reference to a later or missing site", s.id));
            }
        }
        for (i, a) in self.areas.iter().enumerate() {
            if a.at.near.is_some_and(|n| usize::from(n) >= sites) {
                return bad(format!("area row {i}: near a missing site"));
            }
        }
        for (i, a) in self.anchors.iter().enumerate() {
            let w = &a.at;
            let site = |r: u8| usize::from(r) < sites;
            let area = |r: u8| usize::from(r) < areas;
            let earlier = |r: u8| usize::from(r) < i;
            let ok = w.road.is_none_or(|(x, y)| site(x) && site(y))
                && w.dist.is_none_or(|d| site(d.to))
                && w.area.is_none_or(area)
                && w.rim.is_none_or(|r| area(r.area) && site(r.toward))
                && w.near_area.is_none_or(area)
                && w.after.is_none_or(|x| earlier(x.anchor))
                && w.apart.is_none_or(|x| earlier(x.from))
                && w.near_anchor.is_none_or(|x| earlier(x.anchor))
                && w.nearest.is_none_or(site);
            if !ok {
                return bad(format!("anchor row {i}: a reference to a missing site or patch, or a later anchor"));
            }
        }
        Ok(())
    }
}

impl SkeletonRows<'static> {
    /// The compiled catalog's rows.
    pub fn catalog() -> Self {
        Self::from(&jane_data::catalog().county)
    }
}

/// Why a build could not start or finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkeletonError {
    /// The builder names this site in code (the first walk, the School) and the rows lack it.
    MissingSite(&'static str),
    /// A row refers to a row the builder cannot follow (a later site, a missing patch).
    BadRow(String),
    /// No attempt could even place the story's sites: the site rows contradict each other.
    NoAttempt { seed: u32 },
}

impl std::fmt::Display for SkeletonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkeletonError::MissingSite(id) => {
                write!(f, "the site rows have no \"{id}\", which the skeleton builds around")
            }
            SkeletonError::BadRow(what) => write!(f, "{what}"),
            SkeletonError::NoAttempt { seed } => write!(
                f,
                "seed {seed}: no attempt could even place the story's sites; the site rows contradict each other"
            ),
        }
    }
}

impl std::error::Error for SkeletonError {}

/// The sites the builder names in code, looked up by id once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named {
    pub station: u8,
    pub julie_house: u8,
    pub town: u8,
    pub school: u8,
}

impl Named {
    pub fn find(sites: &[SiteDef]) -> Result<Self, SkeletonError> {
        let ix = |id: &'static str| {
            sites.iter().position(|s| s.id == id).map(|i| i as u8).ok_or(SkeletonError::MissingSite(id))
        };
        Ok(Self { station: ix("station")?, julie_house: ix("julie_house")?, town: ix("town")?, school: ix("school")? })
    }

    fn end(i: u8) -> RoadEnd {
        RoadEnd::Site(u16::from(i))
    }

    /// The first walk's two roads, as laid: station to Julie's, Julie's to town.
    pub fn first_walk(self) -> [(RoadEnd, RoadEnd); 2] {
        [(Self::end(self.station), Self::end(self.julie_house)), (Self::end(self.julie_house), Self::end(self.town))]
    }
}

/// One rule checked against what was built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub rule: String,
    pub ok: bool,
    pub detail: String,
}

/// Attempts thrown away, by the stage that threw them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Failed {
    /// A site row found nowhere to stand (or no spot of fourteen met its road rules).
    pub site: u32,
    /// A site that must not be seen from a road has one beside it.
    pub off_road: u32,
    /// The road rules, reachability or bridges, checked early.
    pub road_checks: u32,
    /// A `required` patch found nowhere to lie.
    pub area: u32,
    /// An anchor found nowhere to stand.
    pub anchor: u32,
    /// The final checks.
    pub checks: u32,
}

/// What a build cost (PORT.md §6.j). Wall time is the caller's: worldgen never reads a clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenStats {
    /// Attempts tried, the kept one included.
    pub attempts: u32,
    /// Road searches (A*), trial roads included.
    pub routes: u32,
    /// Steps back to re-place an earlier site that crowded a later one out.
    pub backjumps: u32,
    /// Whole-map distance passes: walks along the network and chamfers.
    pub floods: u32,
    /// Macro cells the road searches expanded.
    pub cells_visited: u64,
    pub failed: Failed,
}

/// The county of one seed, decided.
#[derive(Clone, Debug)]
pub struct Skeleton {
    pub seed: u32,
    /// The attempt kept. A skeleton that cannot satisfy its rows is never shown; the next is tried.
    pub attempt: u8,
    /// Height, water, regions, biomes and the rest of the land.
    pub terrain: Terrain,
    /// [`ROAD`], [`ROAD_LIT`], [`super::roads::ROAD_BRIDGE`] bits per macro cell.
    pub road: Grid<u8>,
    /// Daytime threat, 0..=6. Night is a rule applied on top ([`threat_at`]).
    pub threat: Grid<u8>,
    /// Every site row, in row order.
    pub sites: Vec<PlacedSite>,
    pub areas: Vec<PlacedArea>,
    /// Rolled small places and, after them, the anchors' own.
    pub pois: Vec<PlacedPoi>,
    /// Small places the story needs, in row order. Each is also in `pois`.
    pub anchors: Vec<PlacedAnchor>,
    pub roads: Vec<Road>,
    /// The railway, macro cells in order from the south edge, through the halt, to the east edge.
    /// Walkable (sleepers and ballast), crossed by roads on the level and the river on a trestle.
    pub rail: Vec<(i32, i32)>,
    pub named: Named,
    pub checks: Vec<Check>,
    pub ok: bool,
    pub stats: GenStats,
}

impl Skeleton {
    /// A site by its row (every row is placed, in order).
    pub fn site(&self, row: u8) -> &PlacedSite {
        &self.sites[usize::from(row)]
    }

    /// A patch by its row, if this seed placed it.
    pub fn area(&self, row: u8) -> Option<&PlacedArea> {
        self.areas.iter().find(|a| a.row == row)
    }

    pub fn region_at(&self, mx: i32, my: i32) -> Region {
        self.terrain.region.read(mx, my, Region::Lowfields)
    }

    /// Bridges over the river, cells within two of each other counted as one.
    pub fn bridges(&self) -> u32 {
        count_bridges(&self.road)
    }
}

/// The first valid skeleton of `seed` from attempt `from` on: the county builder asks for the next
/// one when it has to re-roll. If none of [`MAX_ATTEMPTS`] passes every check, the last that got as
/// far as the checks is returned with `ok` false.
pub fn build_skeleton(seed: u32, rows: &SkeletonRows<'_>, from: u8) -> Result<Skeleton, SkeletonError> {
    let named = Named::find(rows.sites)?;
    rows.check()?;
    let mut stats = GenStats::default();
    let mut router = Router::new();
    let mut last = None;
    for k in 0..MAX_ATTEMPTS {
        let Some(attempt) = from.checked_add(k) else { break };
        stats.attempts += 1;
        match try_build(seed, attempt, rows, named, &mut router, &mut stats) {
            Ok(mut s) => {
                stats.cells_visited = router.expanded();
                if s.ok {
                    s.stats = stats;
                    return Ok(s);
                }
                stats.failed.checks += 1;
                last = Some(s);
            }
            Err(Fail::RoadChecks(s)) => {
                stats.failed.road_checks += 1;
                last = Some(*s);
            }
            Err(f) => f.count(&mut stats.failed),
        }
    }
    stats.cells_visited = router.expanded();
    let mut s = last.ok_or(SkeletonError::NoAttempt { seed })?;
    s.stats = stats;
    Ok(s)
}

/// The catalog's county for `seed`, from attempt 0.
pub fn skeleton(seed: u32) -> Result<Skeleton, SkeletonError> {
    build_skeleton(seed, &SkeletonRows::catalog(), 0)
}

/// Why an attempt was thrown away.
#[derive(Debug)]
enum Fail {
    Site,
    OffRoad,
    /// The early road checks failed: the skeleton as far as it got, kept in case no attempt passes.
    RoadChecks(Box<Skeleton>),
    Area,
    Anchor,
}

impl Fail {
    fn count(self, f: &mut Failed) {
        match self {
            Fail::Site => f.site += 1,
            Fail::OffRoad => f.off_road += 1,
            Fail::RoadChecks(_) => f.road_checks += 1,
            Fail::Area => f.area += 1,
            Fail::Anchor => f.anchor += 1,
        }
    }
}

fn try_build(
    seed: u32,
    attempt: u8,
    rows: &SkeletonRows<'_>,
    named: Named,
    router: &mut Router,
    stats: &mut GenStats,
) -> Result<Skeleton, Fail> {
    let t = build_terrain(seed, attempt);
    let Network { sites, mut road, roads } = place_sites(seed, attempt, &t, rows.sites, router, stats)?;

    // The first walk: station to Julie's to the town. Nothing above threat 1 may touch it by day.
    let first_walk = named.first_walk();
    let mut safe = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    for r in roads.iter().filter(|r| first_walk.contains(&(r.from, r.to))) {
        for &(x, y) in &r.cells {
            safe.set(x, y, true);
        }
    }

    stats.floods += 1;
    let road_dist = distance_to(&road, ROAD);
    // A site that must not be seen from a road is checked now that there are roads. Cheaper to
    // re-roll than to repair.
    for s in &sites {
        if s.def.at.off_road.is_some_and(|off| road_tenths(road_dist.read(s.mx, s.my, u16::MAX)) < i64::from(off) * 10)
        {
            return Err(Fail::OffRoad);
        }
    }

    let skeleton =
        |t: Terrain, road: Grid<u8>, sites: Vec<PlacedSite>, roads: Vec<Road>, checks: Vec<Check>| Skeleton {
            seed,
            attempt,
            terrain: t,
            road,
            threat: Grid::new(SKEL_W as u32, SKEL_H as u32, 0),
            sites,
            areas: Vec::new(),
            pois: Vec::new(),
            anchors: Vec::new(),
            roads,
            rail: Vec::new(),
            named,
            checks,
            ok: false,
            stats: GenStats::default(),
        };
    // The road rules are the ones most likely to fail, and they need nothing below this line. Check
    // them now, so a bad attempt costs a few milliseconds instead of the whole build.
    let early = road_checks(&sites, &road, named, stats);
    if !early.iter().all(|c| c.ok) {
        return Err(Fail::RoadChecks(Box::new(skeleton(t, road, sites, roads, early))));
    }

    // The railway, now that the places it must keep off and the roads it must cross are known.
    let station = &sites[usize::from(named.station)];
    let others: Vec<(i32, i32)> = sites.iter().filter(|x| x.row != named.station).map(|x| (x.mx, x.my)).collect();
    let rail = lay_rail(seed, attempt, &t, &road, (station.mx, station.my), &others, RailCost::default());
    let near_rail = rail_mask(&rail, 1);
    stats.floods += 1;
    let safe_dist = distance_to_mask(&safe);
    let ctx = PlaceCtx {
        t: &t,
        road: &road,
        road_dist: &road_dist,
        sites: &sites,
        safe_dist: &safe_dist,
        near_rail: &near_rail,
    };
    let areas = place_areas(&ctx, rows.areas, seed, attempt);
    // The patches the story needs are not optional: without them this attempt is over.
    for (i, row) in rows.areas.iter().enumerate() {
        if row.required && !areas.iter().any(|a| usize::from(a.row) == i) {
            return Err(Fail::Area);
        }
    }
    // The line is walked for the roadside beat like a road: a dead signal, a slag wagon, a hut beside
    // it. Lamps come after: the beat reads where the roads are, not which are lit.
    let mut walks: Vec<Walk<'_>> = roads.iter().map(|r| Walk { from: r.from, to: r.to, cells: &r.cells }).collect();
    walks.push(Walk { from: RoadEnd::Rail, to: RoadEnd::Rail, cells: &rail });
    let first = [Named::end(named.station), Named::end(named.julie_house), Named::end(named.town)];
    let mut pois = place_pois(&ctx, &walks, rows.pois, first, seed, attempt);
    drop(walks);
    light_lamps(seed, attempt, &mut road, &roads, &sites[usize::from(named.town)], &safe, &t);
    let mut actx = AnchorCtx {
        t: &t,
        road: &mut road,
        roads: &roads,
        sites: &sites,
        areas: &areas,
        pois: &mut pois,
        safe: &safe,
        near_rail: &near_rail,
        first_walk,
    };
    let anchors = place_anchors(&mut actx, rows.anchors, seed, attempt).ok_or(Fail::Anchor)?;
    let threat = build_threat(&t, &sites, &areas, &road, named);
    let mut s = skeleton(t, road, sites, roads, Vec::new());
    s.threat = threat;
    s.areas = areas;
    s.pois = pois;
    s.anchors = anchors;
    s.rail = rail;
    s.checks = validate(&s, &safe, stats);
    s.ok = s.checks.iter().all(|c| c.ok);
    Ok(s)
}

/// Sites and roads together, one row at a time: stand the site somewhere its row allows, lay its
/// road from the site it hangs off, and measure the row's road rules on the network as it now is. A
/// spot that fails is dropped and another tried, so one awkward site costs a few retries, not the
/// whole county. Row order is road order: the first walk is laid first, and everything later bends
/// toward it.
///
/// A row with nowhere to stand blames the earlier site that crowded it out ([`blame`]; or, when
/// every spot failed its road rules, the latest site those rules or its road hang off), and the
/// rows from that one on are placed again, each drawing on from its own dice. Where the land
/// itself has no room, or after [`MAX_JUMPS`] such steps back, the attempt is over.
fn place_sites(
    seed: u32,
    attempt: u8,
    t: &Terrain,
    rows: &[SiteDef],
    router: &mut Router,
    stats: &mut GenStats,
) -> Result<Network, Fail> {
    let mut sites: Vec<PlacedSite> = Vec::with_capacity(rows.len());
    let mut land = Land::new(t);
    let mut roads: Vec<Road> = Vec::new();
    // The network as it was before each placed row laid its road, to step back to.
    let mut before: Vec<(Land<'_>, usize)> = Vec::with_capacity(rows.len());
    let mut rngs: Vec<Sfc32> =
        (0..rows.len()).map(|ix| dice(seed, ZoneId::County, Step::SkelSite, attempt, ix as i32, 0)).collect();
    let mut jumps = 0;
    while sites.len() < rows.len() {
        let ix = sites.len();
        let row = &rows[ix];
        let mut candidates = site_candidates(t, row, &sites);
        if let Some(off) = row.at.off_road {
            stats.floods += 1;
            let d = distance_to(&land.road, ROAD);
            candidates.retain(|&c| {
                let (x, y) = xy_of(c);
                road_tenths(d.read(x, y, u16::MAX)) >= i64::from(off) * 10
            });
        }
        let parent = row.road_from.map(|i| sites[usize::from(i)]);
        let rng = &mut rngs[ix];
        let mut placed = None;
        for _ in 0..SITE_TRIES {
            if candidates.is_empty() {
                break;
            }
            let (mx, my) = xy_of(candidates[rng.below(candidates.len() as u32) as usize]);
            let here = PlacedSite { row: ix as u8, def: *row, mx, my };
            let Some(parent) = parent else {
                placed = Some((here, None));
                break;
            };
            let mut trial = land.clone();
            stats.routes += 1;
            let Some(cells) = router.route(&trial, (parent.mx, parent.my), (mx, my)) else { continue };
            let laid = lay_road(&mut trial, RoadEnd::Site(u16::from(parent.row)), RoadEnd::Site(ix as u16), cells);
            let ok = row.dist.iter().filter(|r| r.by == DistBy::Road).all(|rule| {
                let other = sites[usize::from(rule.to)];
                stats.floods += 1;
                let d = road_distances(&trial.road, other.mx, other.my).read(mx, my, FAR);
                d != FAR
                    && rule.min.is_none_or(|m| d >= u32::from(m) * 10)
                    && rule.max.is_none_or(|m| d <= u32::from(m) * 10)
            });
            if ok {
                placed = Some((here, Some((trial, laid))));
                break;
            }
        }
        match placed {
            Some((here, laid)) => {
                before.push((land.clone(), roads.len()));
                if let Some((trial, r)) = laid {
                    land = trial;
                    roads.push(r);
                }
                sites.push(here);
            }
            None => {
                let culprit = if candidates.is_empty() {
                    blame(t, row, &sites)
                } else {
                    row.dist
                        .iter()
                        .filter(|r| r.by == DistBy::Road)
                        .map(|r| usize::from(r.to))
                        .chain(row.road_from.map(usize::from))
                        .max()
                };
                let Some(culprit) = culprit.filter(|_| jumps < MAX_JUMPS) else { return Err(Fail::Site) };
                jumps += 1;
                stats.backjumps += 1;
                let (was, n) = before.swap_remove(culprit);
                before.truncate(culprit);
                land = was;
                roads.truncate(n);
                sites.truncate(culprit);
            }
        }
    }
    Ok(Network { sites, road: land.road, roads })
}

/// The story's sites and the roads between them.
struct Network {
    sites: Vec<PlacedSite>,
    road: Grid<u8>,
    roads: Vec<Road>,
}

/// Chamfer distance to the cells of a mask, tenths of a macro cell.
fn distance_to_mask(mask: &Grid<bool>) -> Grid<u16> {
    jane_core::search::chamfer(mask.w(), mask.h(), |x, y| mask.read(x, y, false))
}

/// Lamp posts are a road attribute. The first walk is lit end to end, and so is the town. Beyond
/// that they come in runs and fail with distance: fewer east of the river, almost none in the
/// Works. Electric relights dead runs later, for good.
fn light_lamps(
    seed: u32,
    attempt: u8,
    road: &mut Grid<u8>,
    roads: &[Road],
    town: &PlacedSite,
    safe: &Grid<bool>,
    t: &Terrain,
) {
    for r in roads {
        // Each road's runs of lamps are its own ranking, by what it joins, not where it is in the list.
        let runs = rank_base(seed, Step::SkelLamps, attempt, walk_key(r.from, r.to), 0);
        for (k, &(x, y)) in r.cells.iter().enumerate() {
            let d2 = metres_sq(x, y, town.mx, town.my);
            // A chance in thousandths: three in four at the town, none by 1125 m.
            let mut p = (750 - isqrt(d2 as u64) as i32 * 2 / 3).max(0);
            match t.region.read(x, y, Region::Lowfields) {
                Region::Waters => p /= 2,
                Region::Works => p = 60,
                Region::Lowfields => {}
            }
            // Decided per run of six cells (about 100 m), so lamps stand in rows and go dark in rows.
            let run = rank_permille(runs, (k / 6) as u32);
            if safe.read(x, y, false) || d2 < 420 * 420 || run < p {
                road.set(x, y, road.read(x, y, 0) | ROAD_LIT);
            }
        }
    }
}

/// Daytime threat, 0..=6. Layers in PLAN.md 2.6, applied in this order.
fn build_threat(t: &Terrain, sites: &[PlacedSite], areas: &[PlacedArea], road: &Grid<u8>, named: Named) -> Grid<u8> {
    let mut threat = Grid::new(SKEL_W as u32, SKEL_H as u32, 0u8);
    let school = &sites[usize::from(named.school)];
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            let v = match t.region.read(x, y, Region::Lowfields) {
                Region::Lowfields => 1,
                Region::Waters if (x - t.river_x[y as usize]) * MACRO > 550 => 3,
                Region::Waters => 2,
                Region::Works if nearer_than(x, y, school.mx, school.my, 4500) => 5,
                Region::Works => 4,
            };
            threat.set(x, y, v);
        }
    }
    let stamp = |threat: &mut Grid<u8>, mx: i32, my: i32, radius: i32, f: &dyn Fn(u8) -> u8| {
        let reach = (radius + MACRO - 1) / MACRO;
        let r2 = i64::from(radius) * i64::from(radius);
        for y in my - reach..=my + reach {
            for x in mx - reach..=mx + reach {
                if inside(x, y) && metres_sq(x, y, mx, my) <= r2 {
                    threat.set(x, y, f(threat.read(x, y, 0)));
                }
            }
        }
    };
    for a in areas {
        stamp(&mut threat, a.mx, a.my, i32::from(a.def.radius), &|_| a.def.threat);
    }
    for s in sites.iter().filter(|s| s.def.dungeon) {
        stamp(&mut threat, s.mx, s.my, DUNGEON_RING, &|old| (old + 1).min(6));
    }
    // A road is the safer way, always: one less, never below one.
    for (v, &r) in threat.as_mut_slice().iter_mut().zip(road.as_slice()) {
        if r & ROAD != 0 {
            *v = v.saturating_sub(1).max(1);
        }
    }
    for s in sites {
        if let Some(hub) = s.def.hub {
            stamp(&mut threat, s.mx, s.my, i32::from(hub), &|_| 0);
        }
    }
    threat
}

/// Threat at a macro cell, by day or by night. Night is a rule on top of the field, not a second
/// field: +1 outside lamplight, +2 in the Works. Havens stay havens.
pub fn threat_at(s: &Skeleton, mx: i32, my: i32, night: bool) -> u8 {
    let base = s.threat.read(mx, my, 0);
    let road = s.road.read(mx, my, 0);
    if !night || base == 0 || road & ROAD_LIT != 0 {
        return base;
    }
    // An unlit road loses its discount after dark, then the night is added.
    let unlit_road = u8::from(road & ROAD != 0);
    let dark = if s.region_at(mx, my) == Region::Works { 2 } else { 1 };
    (base + unlit_road + dark).min(6)
}

/// Whole metres from tenths, rounded.
fn round_m(tenths: u32) -> u32 {
    (tenths + 5) / 10
}

/// Distance rules and reachability: everything that depends only on sites and roads.
fn road_checks(sites: &[PlacedSite], road: &Grid<u8>, named: Named, stats: &mut GenStats) -> Vec<Check> {
    let mut checks = Vec::new();
    let mut check = |rule: String, ok: bool, detail: String| checks.push(Check { rule, ok, detail });

    // Every road rule, measured along the network that was actually built. Walks cached per site.
    let mut cache: Vec<Option<Grid<u32>>> = vec![None; sites.len()];
    for a in sites {
        for rule in a.def.dist {
            let b = &sites[usize::from(rule.to)];
            // Tenths of a metre.
            let d = match rule.by {
                DistBy::Road => {
                    let walk = cache[usize::from(b.row)].get_or_insert_with(|| {
                        stats.floods += 1;
                        road_distances(road, b.mx, b.my)
                    });
                    walk.read(a.mx, a.my, FAR)
                }
                DistBy::Line => isqrt(metres_sq(a.mx, a.my, b.mx, b.my) as u64 * 100),
            };
            let ok = d != FAR
                && rule.min.is_none_or(|m| d >= u32::from(m) * 10)
                && rule.max.is_none_or(|m| d <= u32::from(m) * 10);
            let by = match rule.by {
                DistBy::Road => "road",
                DistBy::Line => "line",
            };
            let max = rule.max.map_or_else(|| "∞".to_string(), |m| m.to_string());
            let detail = if d == FAR { "no way".to_string() } else { format!("{} m", round_m(d)) };
            check(format!("{} {}..{max} m from {} by {by}", a.def.id, rule.min.unwrap_or(0), b.def.id), ok, detail);
        }
    }

    // Every story place that should be on the network is reachable from the platform.
    let station = &sites[usize::from(named.station)];
    let from = match cache[usize::from(station.row)].take() {
        Some(g) => g,
        None => {
            stats.floods += 1;
            road_distances(road, station.mx, station.my)
        }
    };
    for x in sites.iter().filter(|x| x.def.on_road) {
        let d = from.read(x.mx, x.my, FAR);
        let detail = if d == FAR { "no way".to_string() } else { format!("{} m from the station", round_m(d)) };
        check(format!("{} is reachable by road", x.def.id), d != FAR, detail);
    }

    let bridges = count_bridges(road);
    check("the river is crossed in one to three places".to_string(), (1..=3).contains(&bridges), bridges.to_string());
    checks
}

fn validate(s: &Skeleton, safe: &Grid<bool>, stats: &mut GenStats) -> Vec<Check> {
    let mut checks = road_checks(&s.sites, &s.road, s.named, stats);
    let mut check = |rule: String, ok: bool, detail: String| checks.push(Check { rule, ok, detail });

    // The line never stops dead: it leaves the county at both ends (rail.rs).
    let on_edge = |&(x, y): &(i32, i32)| x == 0 || y == 0 || x == SKEL_W - 1 || y == SKEL_H - 1;
    let ends_ok = s.rail.len() > 1 && on_edge(&s.rail[0]) && on_edge(&s.rail[s.rail.len() - 1]);
    check("the railway runs off the map at both ends".to_string(), ends_ok, format!("{} cells", s.rail.len()));

    // The first evening is a walk, not a fight.
    let worst =
        safe.as_slice().iter().zip(s.threat.as_slice()).filter(|(on, _)| **on).map(|(_, &t)| t).max().unwrap_or(0);
    check(
        "the walk from the station to Julie's and on to town never crosses threat above 1 by day".to_string(),
        worst <= 1,
        format!("worst {worst}"),
    );

    // Every region has somewhere to rest that is no more dangerous than the region itself.
    for (r, base) in REGIONS.into_iter().zip([1u8, 3, 5]) {
        let rests: Vec<&PlacedSite> = s.sites.iter().filter(|x| x.def.rest && s.region_at(x.mx, x.my) == r).collect();
        let ok = rests.iter().any(|x| s.threat.read(x.mx, x.my, 6) <= base);
        let mut detail = rests.iter().map(|x| x.def.id).collect::<Vec<_>>().join(", ");
        if detail.is_empty() {
            detail.push_str("none");
        }
        check(format!("{} has a bed or a fire inside its own threat", region_name(r)), ok, detail);
    }

    // Density: the budget, and the longest stretch of road with nothing to look at.
    for r in REGIONS {
        // The places the story needs sit on top of the rolled budget: they are not the seed's to spend.
        let n = s.pois.iter().filter(|p| p.region == r).count() as u32;
        let rolled = s.pois.iter().filter(|p| p.region == r && p.anchor.is_none()).count() as u32;
        check(
            format!("{} has at least 22 small places, at most {POI_BUDGET} of them rolled", region_name(r)),
            n >= 22 && rolled <= POI_BUDGET,
            format!("{n} ({rolled} rolled)"),
        );
    }
    // Which road cells see something, worked out once: a small place within 120 m, a site within 160.
    let mut seen = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    let reach = (SEEN_FROM_ROAD as i32 + 40 + MACRO - 1) / MACRO;
    let mut mark = |mx: i32, my: i32, m: i64| {
        for y in my - reach..=my + reach {
            for x in mx - reach..=mx + reach {
                if inside(x, y) && metres_sq(x, y, mx, my) <= m * m {
                    seen.set(x, y, true);
                }
            }
        }
    };
    for p in &s.pois {
        mark(p.mx, p.my, SEEN_FROM_ROAD);
    }
    for x in &s.sites {
        mark(x.mx, x.my, SEEN_FROM_ROAD + 40);
    }
    // Tenths of a metre: a macro step along a road counts 1.2 cells, as the roadside beat does.
    let mut longest = 0u32;
    for r in &s.roads {
        let mut run = 0u32;
        for &(x, y) in &r.cells {
            run = if seen.read(x, y, false) { 0 } else { run + 192 };
            longest = longest.max(run);
        }
    }
    check(
        format!("no stretch of road longer than {LONGEST_EMPTY} m has nothing to see"),
        longest <= LONGEST_EMPTY * 10,
        format!("{} m", round_m(longest)),
    );
    checks
}

/// A skeleton as text, for a failed test or `jane view --why`: the checks that failed.
pub fn failures(s: &Skeleton) -> String {
    let mut out = String::new();
    for c in s.checks.iter().filter(|c| !c.ok) {
        let _ = write!(out, "{} ({}); ", c.rule, c.detail);
    }
    out
}
