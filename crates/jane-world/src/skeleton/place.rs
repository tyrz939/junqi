//! Layers 1b, 3 and 4 of the skeleton: where things go. Sites are solved from their rows, then
//! the named patches, then a budgeted scatter of small places. Everything is "list the cells that
//! satisfy the row, pick one with the seed": no hill-climbing, nothing that can wander. Carries
//! `jane/src/world/skeleton/place.ts`.
//!
//! Distances are integers: crow's metres are compared squared ([`metres_sq`], [`nearer_than`]),
//! distance to a road is the chamfer's tenths of a macro cell, and a fraction is written out as a
//! ratio of integers.

use jane_core::grid::Grid;
use jane_core::{NameId, TextId, ZoneId};
use jane_data::{AreaDef, DistBy, Edge, PoiDef, PoiWhere, Region, SiteDef, Terrain as Ground};

use super::rank::{rank_base, rank_of, ranked};
use super::roads::{ROAD, RoadEnd};
use super::terrain::Terrain;
use super::types::{MACRO, SKEL_H, SKEL_W, Water, cell_of, inside, metres_sq, nearer_than, xy_of};
use crate::steps::{Step, dice};

/// A story site where it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedSite {
    /// Its row in the site table; every row is placed, in order, so this is also its index here.
    pub row: u8,
    pub def: SiteDef,
    pub mx: i32,
    pub my: i32,
}

/// A named patch where it lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedArea {
    /// Its row in the area table. A patch that is not `required` may be missing on a seed.
    pub row: u8,
    pub def: AreaDef,
    pub mx: i32,
    pub my: i32,
}

/// A small place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedPoi {
    /// What `smallPlace` dresses; `None` for an anchor's bare spot (a lamp post stands there).
    pub kind: Option<NameId>,
    /// `None`: an anchor without a name, called by its id.
    pub name: Option<TextId>,
    pub mx: i32,
    pub my: i32,
    pub region: Region,
    /// A small place the story needs, by its row in the anchor table (anchors.rs).
    pub anchor: Option<u8>,
}

/// How far a patch's edge may lie from the nearest road and still be seen from it (m): about a
/// half-screen and a half.
const PATCH_SEEN: i64 = 40;
/// Extra metres a patch of threat 4 or more keeps from a haven's fence.
const HAVEN_MARGIN: i64 = 260;
/// Sites keep at least this far apart unless a row says otherwise (m).
const SITE_SPACING: i64 = 160;
/// A road rule is aimed at with the crow's distance: roads wander 1.08 to 1.55 times the crow's
/// line. In hundredths.
const WANDER_MIN: i64 = 108;
const WANDER_MAX: i64 = 155;

/// Metres to a road, in tenths, from the chamfer's tenths of a macro cell.
pub const fn road_tenths(chamfer10: u16) -> i64 {
    chamfer10 as i64 * MACRO as i64
}

/// Whether a macro cell is ground of `kind` (any dry ground for `None`).
pub fn terrain_fits(t: &Terrain, kind: Option<Ground>, x: i32, y: i32) -> bool {
    if t.water.read(x, y, Water::Lake) != Water::Dry {
        return false;
    }
    let slope = t.slope.read(x, y, u8::MAX);
    let wet = t.wet.read(x, y, 0);
    let biome = t.biome.read(x, y, super::types::Biome::Field);
    match kind {
        None => true,
        Some(Ground::Flat) => slope <= 7 && wet >= 20,
        Some(Ground::Bank) => (20..=50).contains(&wet),
        Some(Ground::Wood) => matches!(biome, super::types::Biome::Wood | super::types::Biome::WetWood),
        Some(Ground::Foothill) => biome == super::types::Biome::Foothill && slope <= 14,
        Some(Ground::Crown) => metres_sq(x, y, t.crown.0, t.crown.1) <= 80 * 80 && slope <= 16,
    }
}

/// What one placed site allows a row: squared crow's metres, at least and at most.
#[derive(Clone, Copy, Debug)]
struct Limit {
    x: i32,
    y: i32,
    min2: i64,
    max2: i64,
}

impl Limit {
    fn allows(self, x: i32, y: i32) -> bool {
        let d2 = metres_sq(x, y, self.x, self.y);
        d2 >= self.min2 && d2 <= self.max2
    }
}

/// What each placed site allows, worked out once per row. A road rule is aimed at with the crow's
/// distance (roads wander); the real road is measured once it is laid.
fn limits(row: &SiteDef, placed: &[PlacedSite]) -> Vec<Limit> {
    placed
        .iter()
        .map(|p| {
            let (min, max) = match row.dist.iter().find(|r| r.to == p.row) {
                None => (SITE_SPACING, i64::MAX),
                Some(r) => {
                    let (lo, hi) = if r.by == DistBy::Road { (WANDER_MAX, WANDER_MIN) } else { (100, 100) };
                    (r.min.map_or(0, |m| i64::from(m) * 100 / lo), r.max.map_or(i64::MAX, |m| i64::from(m) * 100 / hi))
                }
            };
            Limit { x: p.mx, y: p.my, min2: min * min, max2: max.saturating_mul(max) }
        })
        .collect()
}

/// Every macro cell this row's own ground allows (region, terrain, edge, height, river, lake, the
/// other side of the river), before the placed sites' distances are asked.
fn site_ground(t: &Terrain, row: &SiteDef, placed: &[PlacedSite]) -> Vec<u32> {
    let w = &row.at;
    let across = w.across_river_from.and_then(|i| placed.iter().find(|p| p.row == i));
    let (x0, x1) = match w.edge {
        Some(Edge::West) => (0, 0),
        Some(Edge::East) => (SKEL_W - 1, SKEL_W - 1),
        _ => (3, SKEL_W - 4),
    };
    let (y0, y1) = match w.edge {
        Some(Edge::North) => (0, 0),
        Some(Edge::South) => (SKEL_H - 1, SKEL_H - 1),
        _ => (3, SKEL_H - 4),
    };
    let mut out = Vec::new();
    for y in y0..=y1 {
        let river = t.river_x[y as usize];
        for x in x0..=x1 {
            if t.region.read(x, y, Region::Lowfields) != row.region || !terrain_fits(t, w.terrain, x, y) {
                continue;
            }
            if w.max_height.is_some_and(|m| t.height.read(x, y, 0) > m) {
                continue;
            }
            if w.river_within.is_some_and(|m| (x - river).abs() * MACRO > i32::from(m)) {
                continue;
            }
            if let Some(m) = w.lake_within {
                let reach = i64::from(t.lake.r * MACRO) + i64::from(m);
                if metres_sq(x, y, t.lake.mx, t.lake.my) > reach * reach {
                    continue;
                }
            }
            if across.is_some_and(|o| (x > river) == (o.mx > t.river_x[o.my as usize])) {
                continue;
            }
            out.push(cell_of(x, y));
        }
    }
    out
}

/// Every macro cell (as [`cell_of`]) this row could stand on, given the sites already placed.
/// In cell order, so the seed's pick is stable.
pub fn site_candidates(t: &Terrain, row: &SiteDef, placed: &[PlacedSite]) -> Vec<u32> {
    let limits = limits(row, placed);
    let mut out = site_ground(t, row, placed);
    out.retain(|&c| {
        let (x, y) = xy_of(c);
        limits.iter().all(|l| l.allows(x, y))
    });
    out
}

/// Which placed site to blame when a row finds nowhere to stand: the one whose distance alone
/// leaves this row's ground the fewest cells (the later on a tie). `None` when the ground itself is
/// empty: no earlier site is at fault, the land is.
pub fn blame(t: &Terrain, row: &SiteDef, placed: &[PlacedSite]) -> Option<usize> {
    let ground = site_ground(t, row, placed);
    if ground.is_empty() {
        return None;
    }
    let limits = limits(row, placed);
    let count = |l: Limit| {
        ground
            .iter()
            .filter(|&&c| {
                let (x, y) = xy_of(c);
                l.allows(x, y)
            })
            .count()
    };
    (0..placed.len()).min_by_key(|&i| (count(limits[i]), std::cmp::Reverse(i)))
}

/// What the patch and small-place stages read of the county built so far.
#[derive(Clone, Copy, Debug)]
pub struct PlaceCtx<'a> {
    pub t: &'a Terrain,
    pub road: &'a Grid<u8>,
    /// Chamfer distance to the nearest road, tenths of a macro cell.
    pub road_dist: &'a Grid<u16>,
    pub sites: &'a [PlacedSite],
    /// Chamfer distance to the first walk (station, Julie's, town), tenths of a macro cell. Nothing
    /// above threat 1 may reach it.
    pub safe_dist: &'a Grid<u16>,
    /// The railway's macro cells and those beside them: nothing small stands on the line.
    pub near_rail: &'a Grid<bool>,
}

/// The named patches, in row order (PLAN.md 2.6). A row that cannot be placed on this seed is
/// skipped, not fatal: patches are texture, sites are story (the caller re-rolls for a missing
/// `required` one).
pub fn place_areas(ctx: &PlaceCtx<'_>, rows: &[AreaDef], seed: u32, attempt: u8) -> Vec<PlacedArea> {
    let mut out: Vec<PlacedArea> = Vec::new();
    for (ix, row) in rows.iter().enumerate() {
        let w = &row.at;
        let near = match w.near {
            Some(i) => match ctx.sites.iter().find(|s| s.row == i) {
                Some(s) => Some(s),
                None => continue,
            },
            None => None,
        };
        let radius = i64::from(row.radius);
        let reach = (i32::from(row.radius) + MACRO - 1) / MACRO;
        // A patch never swallows a haven, a dangerous one never sits on a story door that is not its
        // anchor, and two patches do not overlap. The worst patches (4 and up) also keep a long field's
        // width from any haven: you should see them coming. Tenths of a metre.
        let keep_out: Vec<(i32, i32, i64)> = ctx
            .sites
            .iter()
            .filter(|s| near.is_none_or(|n| n.row != s.row))
            .map(|s| {
                let r10 = match s.def.hub {
                    Some(hub) => {
                        let margin = if row.threat >= 4 { HAVEN_MARGIN } else { 0 };
                        (i64::from(hub) + radius + 30 + margin) * 10
                    }
                    None => radius * 6,
                };
                (s.mx, s.my, r10)
            })
            .chain(out.iter().map(|a| (a.mx, a.my, (i64::from(a.def.radius) + radius) * 9)))
            .collect();
        let mut candidates = Vec::new();
        for y in reach..SKEL_H - reach {
            for x in reach..SKEL_W - reach {
                if ctx.t.region.read(x, y, Region::Lowfields) != row.region || !terrain_fits(ctx.t, w.terrain, x, y) {
                    continue;
                }
                let rd = road_tenths(ctx.road_dist.read(x, y, u16::MAX));
                if w.on_road && rd > 0 {
                    continue;
                }
                if w.off_road.is_some_and(|m| rd < i64::from(m) * 10) {
                    continue;
                }
                if let Some(n) = near {
                    let d2 = metres_sq(x, y, n.mx, n.my);
                    let lo = i64::from(w.near_min.unwrap_or(0));
                    if d2 < lo * lo || w.near_max.is_some_and(|m| d2 > i64::from(m) * i64::from(m)) {
                        continue;
                    }
                }
                if row.threat > 1 && i32::from(ctx.safe_dist.read(x, y, u16::MAX)) <= (reach + 2) * 10 {
                    continue;
                }
                if keep_out.iter().any(|&(kx, ky, r10)| nearer_than(x, y, kx, ky, r10)) {
                    continue;
                }
                candidates.push(cell_of(x, y));
            }
        }
        // A patch is somewhere she is sent by name, so its edge is seen from a road wherever the row
        // leaves room for that; only where it does not may it lie out of sight.
        let seen: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|&c| {
                let (x, y) = xy_of(c);
                road_tenths(ctx.road_dist.read(x, y, u16::MAX)) <= (radius + PATCH_SEEN) * 10
            })
            .collect();
        // Each patch ranks the ground on its own: a cell ruled out moves it only if it was the pick.
        let base = rank_base(seed, Step::SkelAreas, attempt, ix as i32, 0);
        if let Some(c) = ranked(base, if seen.is_empty() { &candidates } else { &seen }) {
            let (mx, my) = xy_of(c);
            out.push(PlacedArea { row: ix as u8, def: *row, mx, my });
        }
    }
    out
}

/// Small places per region, rolled (anchors come on top). PLAN.md 2.4: 25 to 35 of them.
pub const POI_BUDGET: u32 = 38;
/// Near enough that a road is never bare for long, far enough that two places never read as one (m).
const POI_SPACING: i64 = 90;
/// "Something visible from the road every 20 to 30 seconds of walking": 170 to 250 m, in tenths.
const ROADSIDE_EVERY_MIN: u32 = 1700;
const ROADSIDE_EVERY_MAX: u32 = 2500;
/// What one macro step along a road counts for the beat, tenths of a metre (1.2 cells: roads bend).
const BEAT_STEP: u32 = 192;

/// One line the roadside beat walks: a road, or the railway (from and to [`RoadEnd::Rail`]).
#[derive(Clone, Copy, Debug)]
pub struct Walk<'a> {
    pub from: RoadEnd,
    pub to: RoadEnd,
    pub cells: &'a [(i32, i32)],
}

/// A road's or the rail's key for its own rankings: by what it joins, never by where it falls in
/// the list.
pub fn walk_key(from: RoadEnd, to: RoadEnd) -> i32 {
    let code = |e: RoadEnd| match e {
        RoadEnd::Site(i) => i32::from(i) + 1,
        RoadEnd::Rail => 0,
    };
    code(from) * 1024 + code(to)
}

/// Minor places. The roads are walked first and something is set beside them at a steady beat,
/// because the density rule is about what you see from the road; then the rest of each region's
/// budget goes deep and along the banks, for whoever leaves it. `first_walk` are the three sites
/// the first walk joins (station, Julie's, town): their roads get twice the beat.
pub fn place_pois(
    ctx: &PlaceCtx<'_>,
    walks: &[Walk<'_>],
    rows: &[PoiDef],
    first_walk: [RoadEnd; 3],
    seed: u32,
    attempt: u8,
) -> Vec<PlacedPoi> {
    let mut out: Vec<PlacedPoi> = Vec::new();
    // Nothing here shares dice with anything else. What kind a place is, is its cell's rank; each
    // road's beat and its spots are that road's own ranking; the deep scatter throws its own stream.
    // So the rail re-laid, or one more road, moves the small places along it and leaves the rest.
    let kind_base = rank_base(seed, Step::SkelPoi, attempt, 0, 0);
    let mut count = [0u32; 3];
    let free = |out: &[PlacedPoi], x: i32, y: i32| -> bool {
        if !inside(x, y) || x < 2 || y < 2 || x > SKEL_W - 3 || y > SKEL_H - 3 {
            return false;
        }
        if ctx.t.water.read(x, y, Water::Dry) != Water::Dry
            || ctx.road.read(x, y, 0) & ROAD != 0
            || ctx.near_rail.read(x, y, false)
        {
            return false;
        }
        if ctx.sites.iter().any(|s| {
            let keep = i64::from(s.def.hub.unwrap_or(0)) + 40;
            nearer_than(x, y, s.mx, s.my, keep.max(110) * 10)
        }) {
            return false;
        }
        !out.iter().any(|p| nearer_than(x, y, p.mx, p.my, POI_SPACING * 10))
    };
    let kind_for = |region: Region, at: PoiWhere, x: i32, y: i32| -> Option<&PoiDef> {
        let fits = || rows.iter().filter(|r| r.regions.contains(&region) && (r.at == at || r.at == PoiWhere::Any));
        let total: u64 = fits().map(|r| u64::from(r.weight)).sum();
        if total == 0 {
            return None;
        }
        let mut roll = (u64::from(rank_of(kind_base, cell_of(x, y))) * total) >> 32;
        for r in fits() {
            if roll < u64::from(r.weight) {
                return Some(r);
            }
            roll -= u64::from(r.weight);
        }
        fits().next_back()
    };
    let add = |out: &mut Vec<PlacedPoi>, count: &mut [u32; 3], x: i32, y: i32, at: PoiWhere| -> bool {
        let region = ctx.t.region.read(x, y, Region::Lowfields);
        if count[region as usize] >= POI_BUDGET {
            return false;
        }
        let Some(row) = kind_for(region, at, x, y) else { return false };
        out.push(PlacedPoi { kind: Some(row.kind), name: Some(row.name), mx: x, my: y, region, anchor: None });
        count[region as usize] += 1;
        true
    };

    for walk in walks {
        // The first walk is the establishing shot: two or three minutes on a lit road with nothing
        // dangerous on it. If that stretch is empty the county reads as empty however full the rest
        // is, so it gets something to look at about twice as often.
        let first = first_walk.contains(&walk.from) && first_walk.contains(&walk.to);
        let (min, max) = if first {
            (ROADSIDE_EVERY_MIN / 2, ROADSIDE_EVERY_MAX / 2)
        } else {
            (ROADSIDE_EVERY_MIN, ROADSIDE_EVERY_MAX)
        };
        let key = walk_key(walk.from, walk.to);
        let beat = rank_base(seed, Step::SkelPoi, attempt, key, 1);
        let side = rank_base(seed, Step::SkelPoi, attempt, key, 2);
        let gap = |n: u32| min + ((u64::from(rank_of(beat, n)) * u64::from(max - min)) >> 32) as u32;
        let mut since = 0;
        let mut next = gap(0);
        for (k, &(cx, cy)) in walk.cells.iter().enumerate().skip(1) {
            since += BEAT_STEP;
            if since < next {
                continue;
            }
            // One or two cells off the road, either side, seeded.
            let mut spots = Vec::new();
            for oy in -2..=2 {
                for ox in -2..=2 {
                    if (ox, oy) != (0, 0) && free(&out, cx + ox, cy + oy) {
                        spots.push(cell_of(cx + ox, cy + oy));
                    }
                }
            }
            let Some(c) = ranked(side, &spots) else { continue };
            let (x, y) = xy_of(c);
            if add(&mut out, &mut count, x, y, PoiWhere::Roadside) {
                since = 0;
                next = gap(k as u32);
            }
        }
    }

    // The rest of the budget: banks, then deep country. Bounded tries, so a cramped region ends
    // short, not in a loop.
    let mut rng = dice(seed, ZoneId::County, Step::SkelPoi, attempt, 0, 3);
    let mut tries = 0;
    while tries < 4000 && count.iter().any(|&n| n < POI_BUDGET) {
        tries += 1;
        let x = 2 + rng.below((SKEL_W - 4) as u32) as i32;
        let y = 2 + rng.below((SKEL_H - 4) as u32) as i32;
        if !free(&out, x, y) {
            continue;
        }
        let wet = ctx.t.wet.read(x, y, 0);
        if (15..=40).contains(&wet) {
            add(&mut out, &mut count, x, y, PoiWhere::Bank);
        } else if road_tenths(ctx.road_dist.read(x, y, u16::MAX)) >= 1300 {
            add(&mut out, &mut count, x, y, PoiWhere::Deep);
        }
    }
    out
}
