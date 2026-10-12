//! The terraced county's skeleton (PLAN-map-r4 section 7): the macro plan and its carve read as the
//! 96 x 96 [`Skeleton`] the county's stages are written against, so they run as they are. The plan
//! places the sites and the roads, so those are read; the patches, the small places and the anchors
//! go through the skeleton's own placement with a filter that keeps them off water, faces, joins and
//! gates; and the threat is the plan's ([`super::threat::by_district`]).

use alloc::vec;
use alloc::vec::Vec;
use jane_core::grid::Grid;
use jane_core::noise::fbm_box;
use jane_core::num::isqrt;
use jane_core::search::chamfer;
use jane_core::{Rect, ZoneId};
use jane_data::{AnchorDef, AreaDef, Region};

use super::carve::blocks::{Blocks, CUT, FIXED, LEVEE, NECK, RIVER, WALL};
use super::carve::{B, Carved, W, check};
use super::macro_plan::{CELLS, MH, MW, MacroPlan, Reg, Role, STEP_CELLS, Wet, idx};
use super::threat;
use crate::skeleton::anchors::{AnchorCtx, place_anchors};
use crate::skeleton::place::{PlaceCtx, Walk, place_areas, place_pois};
use crate::skeleton::rail::rail_mask;
use crate::skeleton::roads::distance_to;
use crate::skeleton::{
    Allow, Biome, GenStats, Lake, MACRO, Named, PlacedArea, PlacedSite, ROAD, ROAD_BRIDGE, Road, RoadEnd, Skeleton,
    SkeletonError, SkeletonRows, Terrain, Water, inside_in, light_lamps,
};
use crate::steps::{Step, dice};

/// Why a plan's skeleton was refused: the rows cannot be satisfied at all (a build error), or the
/// plan's ground left a thing the story needs nowhere to stand (the plan is re-rolled).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    Rows(SkeletonError),
    /// A patch the story needs (its row in the area table) found no ground.
    Area(u8),
    /// A small place the story needs (its row in the anchor table) found no ground.
    Anchor(u8),
    /// The first walk crosses a patch of threat above one.
    Walk,
}

impl From<SkeletonError> for Refused {
    fn from(e: SkeletonError) -> Self {
        Refused::Rows(e)
    }
}

const fn region_of(r: Reg) -> Region {
    match r {
        Reg::Lowfields => Region::Lowfields,
        Reg::Waters => Region::Waters,
        Reg::Works => Region::Works,
    }
}

/// The land of the plan: heights by level (smooth within one, so a macro cell is flat unless it
/// holds a step), the districts' regions and biomes, no water on the macro grid (the carve lays the
/// water, cell by cell) and how far each macro cell is from the plan's.
fn terrain(c: &Carved) -> Terrain {
    let plan = &c.plan;
    let (w, h) = (MW as u32, MH as u32);
    let salt = dice(plan.seed, ZoneId::County, Step::MapLand, plan.attempt, 300, 0).next_u32();
    let lumps = fbm_box(Rect::new(0, 0, MW, MH), 16, salt, 2, 1);
    let mut height = Grid::new(w, h, 0u8);
    let mut region = Grid::new(w, h, Region::Lowfields);
    let mut biome = Grid::new(w, h, Biome::Field);
    for y in 0..MH {
        for x in 0..MW {
            let i = idx(x, y);
            let d = &plan.districts[usize::from(plan.district_of[i])];
            height.set(x, y, (40 + 50 * i32::from(d.level) + ((lumps[i].0 * 15) >> 16)) as u8);
            region.set(x, y, region_of(d.reg));
            // The Foothills are where the quarry's patch must lie, and it asks for foothill ground.
            biome.set(x, y, if d.role == Role::Foothills { Biome::Foothill } else { d.biome });
        }
    }
    let mut slope = Grid::new(w, h, 0u8);
    for y in 0..MH {
        for x in 0..MW {
            let here = height.read(x, y, 0);
            let mut worst = 0u8;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    if inside_in(MW, MH, x + ox, y + oy) {
                        worst = worst.max(here.abs_diff(height.read(x + ox, y + oy, 0)));
                    }
                }
            }
            slope.set(x, y, worst);
        }
    }
    let wet = chamfer(w, h, |x, y| plan.wet[idx(x, y)] != Wet::Dry);
    // The river's column at each row: the channel's, else the Lowfields' last.
    let river_x: Vec<i32> = (0..MH)
        .map(|y| {
            (0..MW)
                .find(|&x| plan.wet[idx(x, y)] == Wet::River)
                .or_else(|| {
                    (0..MW).rev().find(|&x| region.read(x, y, Region::Works) == Region::Lowfields).map(|x| x + 1)
                })
                .unwrap_or(0)
        })
        .collect();
    let crown = plan.site_cell("school");
    Terrain {
        height,
        water: Grid::new(w, h, Water::Dry),
        region,
        biome,
        wet,
        slope,
        rough: Grid::new(w, h, 256u16),
        river_x,
        crown,
        lake: lake_of(plan),
    }
}

/// The lake as a disc: the mean of its macro cells and the radius that gives their area.
fn lake_of(plan: &MacroPlan) -> Lake {
    let cells: Vec<(i32, i32)> =
        (0..CELLS).filter(|&i| plan.wet[i] == Wet::Lake).map(|i| ((i as i32) % MW, (i as i32) / MW)).collect();
    let n = cells.len() as i32;
    if n == 0 {
        return Lake { mx: MW * 5 / 6, my: MH * 3 / 4, r: 7 };
    }
    let (sx, sy) = cells.iter().fold((0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
    // pi r^2 = n, with pi as 355/113.
    Lake { mx: sx / n, my: sy / n, r: isqrt((n * 113 / 355) as u64).max(1) as i32 }
}

/// The plan's sites in the order of the site table, each where the plan stands it.
fn placed_sites(plan: &MacroPlan, rows: &SkeletonRows<'_>) -> Result<Vec<PlacedSite>, SkeletonError> {
    rows.sites
        .iter()
        .enumerate()
        .map(|(row, def)| {
            let s = plan.site(def.id).ok_or(SkeletonError::MissingSite(def.id))?;
            Ok(PlacedSite { row: row as u8, def: *def, mx: i32::from(s.pos.0), my: i32::from(s.pos.1) })
        })
        .collect()
}

/// The plan's routes as the skeleton's roads, and the road grid they set (a river cell a road
/// crosses is a bridge).
fn roads_of(plan: &MacroPlan, sites: &[PlacedSite]) -> (Vec<Road>, Grid<u8>) {
    let end = |id: &str| RoadEnd::Site(sites.iter().position(|s| s.def.id == id).unwrap_or(0) as u16);
    let mut road = Grid::new(MW as u32, MH as u32, 0u8);
    let roads: Vec<Road> = plan
        .routes
        .iter()
        .map(|r| {
            let cells: Vec<(i32, i32)> = r.pts.iter().map(|&(x, y)| (i32::from(x), i32::from(y))).collect();
            let mut len10 = 0u32;
            for (k, &(x, y)) in cells.iter().enumerate() {
                let mut bits = road.read(x, y, 0) | ROAD;
                if plan.wet[idx(x, y)] == Wet::River {
                    bits |= ROAD_BRIDGE;
                }
                road.set(x, y, bits);
                if k > 0 {
                    let (px, py) = cells[k - 1];
                    len10 += if px != x && py != y { 14 } else { 10 };
                }
            }
            Road { from: end(r.from), to: end(r.to), cells, metres: (len10 * MACRO as u32 + 5) / 10 }
        })
        .collect();
    (roads, road)
}

/// Macro cells small things may be laid on: no water in the cell, every block of it and of the ring
/// round it at one level with nothing made on them (a face's run, a join, a span, a bay, a dyke, the
/// river's floor), and no join's, span's or plank's ground within two cells. A rim or a south face
/// drawn in the cell, or a landing at its edge, would be under whatever stands there.
pub fn plain_cells(c: &Carved) -> Vec<bool> {
    let b = &c.blocks;
    let mut plain = vec![true; CELLS];
    let made = FIXED | CUT | NECK | LEVEE | RIVER | WALL;
    let bw = W / B;
    for my in 0..MH {
        for mx in 0..MW {
            let (bx0, by0) = (mx * 4, my * 4);
            let level = b.level[Blocks::ix(bx0, by0)];
            let ok = (by0 - 1..by0 + 5).all(|by| {
                (bx0 - 1..bx0 + 5).all(|bx| {
                    if bx < 0 || by < 0 || bx >= bw || by >= bw {
                        return true;
                    }
                    let i = Blocks::ix(bx, by);
                    b.level[i] == level && b.flags[i] & made == 0
                })
            });
            plain[idx(mx, my)] = ok;
        }
    }
    // Water: any bit of the macro cell's 16 x 16 cells.
    let words = W as usize / 64;
    for y in 0..W {
        for mx in 0..MW {
            let at = y as usize * words + (mx as usize * 16) / 64;
            if (c.water.bits.word(at) >> ((mx as usize * 16) % 64)) & 0xffff != 0 {
                plain[idx(mx, y / STEP_CELLS)] = false;
            }
        }
    }
    for r in c.claims() {
        let g = r.grow(2);
        for my in (g.y.max(0) / STEP_CELLS)..=((g.bottom() - 1).min(W - 1) / STEP_CELLS) {
            for mx in (g.x.max(0) / STEP_CELLS)..=((g.right() - 1).min(W - 1) / STEP_CELLS) {
                plain[idx(mx, my)] = false;
            }
        }
    }
    // And reached from the Halt on the carve's own ground, ledges and decks crossed: a pocket the
    // faces shut off is no place for a patch or a prop the story walks to.
    if let Some(start) = c.pads.iter().find(|p| p.site == "station").map(|p| p.boxr.centre()) {
        let open = check::planes(c).open;
        let broken = c.spans.iter().enumerate().fold(0u64, |m, (i, s)| m | u64::from(s.broken) << i);
        let reached = check::reach_words(&open, &c.portals(), &[start], broken, false);
        for my in 0..MH {
            for mx in 0..MW {
                let (x, y) = (mx * STEP_CELLS + STEP_CELLS / 2, my * STEP_CELLS + STEP_CELLS / 2);
                let i = (y * W + x) as usize;
                plain[idx(mx, my)] &= reached[i >> 6] >> (i & 63) & 1 != 0;
            }
        }
    }
    plain
}

/// The macro cells within one of a gate: where nothing small is laid.
fn near_gates(plan: &MacroPlan) -> Vec<bool> {
    let mut near = vec![false; CELLS];
    for g in &plan.gates {
        for q in [g.a, g.b] {
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let (x, y) = (i32::from(q.0) + ox, i32::from(q.1) + oy);
                    if inside_in(MW, MH, x, y) {
                        near[idx(x, y)] = true;
                    }
                }
            }
        }
    }
    near
}

/// A length in the plain county's metres in this county's: its 1,536 cells to the 2,000 of the plain
/// one, three in four. The rows' distances (a patch's radius and the band it lies in, the span of a
/// small place from a site) were written for the larger county; kept as they are, the patches would
/// crowd one another out and the props of the station road would lie beyond Julie's door.
const fn down(m: u16) -> u16 {
    (m as u32 * 3 / 4) as u16
}

/// The patch rows in the metres of this county, and the order they are laid in: the quarry's first (it
/// asks for foothill ground, the one patch that does), then the rest as they are. `order[k]` is the row
/// the `k`-th laid is. A patch of threat 4 or more is laid as one of 3: the plain county keeps those a
/// long way off every haven, which this county has not the room for.
fn patch_rows(rows: &[AreaDef]) -> (Vec<AreaDef>, Vec<usize>) {
    let cat = jane_data::catalog();
    let first = rows.iter().position(|a| cat.name(a.id) == "quarry_steps");
    let order: Vec<usize> = first.into_iter().chain((0..rows.len()).filter(|&i| Some(i) != first)).collect();
    let laid = order
        .iter()
        .map(|&i| {
            let mut a = rows[i];
            a.radius = down(a.radius);
            a.threat = a.threat.min(3);
            a.at.near_min = a.at.near_min.map(down);
            a.at.near_max = a.at.near_max.map(down);
            a.at.off_road = a.at.off_road.map(down);
            a
        })
        .collect();
    (laid, order)
}

/// What a row's distance becomes in this county's metres.
type Metres = fn(u16) -> u16;

/// How many times the anchors are asked for, each with their distances looser than the last.
const ANCHOR_LEVELS: u8 = 4;
/// A road of fewer macro cells than this has its last lamp three cells in from its start.
const SHORT_ROAD: usize = 32;
/// Dice a level of anchors is thrown on before it is loosened.
const ANCHOR_RETRIES: u8 = 3;

/// The anchor rows in the metres of this county at `level`: 0 scales every distance by three in four; 1
/// halves the nearest each may be and leaves the farthest; 2 drops the nearest and lets the farthest
/// be half as far again; 3 drops the distances, and the place need only stand beside its road, in its
/// patch, on its rim. The station road is shorter here than in the plain county (a minute to two and
/// a half, not two to four), and a roadside prop is kept off every site by a few cells, so a band
/// written for the larger county may leave no cell at all.
fn anchor_rows(rows: &[AnchorDef], level: u8) -> Vec<AnchorDef> {
    let (lo, hi): (Metres, Metres) = match level {
        0 => (down, down),
        1 => (|m| m / 2, |m| m),
        _ => (|_| 0, |m| m.saturating_mul(3) / 2),
    };
    rows.iter()
        .map(|r| {
            let mut r = *r;
            if level >= 3 {
                r.at.dist = None;
                r.at.apart = None;
                r.at.near_anchor = None;
                return r;
            }
            r.at.dist = r.at.dist.map(|mut d| {
                d.min = d.min.map(lo);
                d.max = d.max.map(hi);
                d
            });
            r.at.apart = r.at.apart.map(|mut a| {
                a.min = lo(a.min);
                a
            });
            r.at.near_anchor = r.at.near_anchor.map(|mut n| {
                n.min = lo(n.min);
                n.max = hi(n.max);
                n
            });
            r
        })
        .collect()
}

/// The skeleton of a carved plan, or why it was refused. The attempt the skeleton's dice throw is the
/// plan's.
pub fn skeleton_of(c: &Carved, rows: &SkeletonRows<'_>) -> Result<Skeleton, Refused> {
    rows.check()?;
    let named = Named::find(rows.sites)?;
    let plan = &c.plan;
    let (seed, attempt) = (plan.seed, plan.attempt);
    let (w, h) = (MW as u32, MH as u32);
    let t = terrain(c);
    let sites = placed_sites(plan, rows)?;
    let (roads, mut road) = roads_of(plan, &sites);
    let rail = c.rail.clone();

    // The first walk: the Halt to Julie's and on to the Castle. Nothing above threat 1 touches it.
    let first_walk = named.first_walk();
    let mut safe = Grid::new(w, h, false);
    for r in roads.iter().filter(|r| first_walk.contains(&(r.from, r.to))) {
        for &(x, y) in &r.cells {
            safe.set(x, y, true);
        }
    }
    let road_dist = distance_to(&road, ROAD);
    let safe_dist = chamfer(w, h, |x, y| safe.read(x, y, false));
    let near_rail = rail_mask(&rail, 1);

    let plain = plain_cells(c);
    let near_gate = near_gates(plan);
    // A patch, a small place or an anchor goes on a plain macro cell away from the gates; a district
    // holds two patches at most.
    let allow = |x: i32, y: i32, placed: &[PlacedArea]| -> bool {
        let i = idx(x, y);
        plain[i]
            && !near_gate[i]
            && placed.iter().filter(|a| plan.district_of[idx(a.mx, a.my)] == plan.district_of[i]).count() < 2
    };
    // An anchor stands beside its road, which a gate is on: plain ground is all it asks.
    let roadside = |x: i32, y: i32, _: &[PlacedArea]| -> bool { plain[idx(x, y)] };
    let ctx = PlaceCtx {
        t: &t,
        road: &road,
        road_dist: &road_dist,
        sites: &sites,
        safe_dist: &safe_dist,
        near_rail: &near_rail,
        allow: Some(Allow(&allow)),
    };
    let (laid, order) = patch_rows(rows.areas);
    let mut areas = place_areas(&ctx, &laid, seed, attempt);
    for a in &mut areas {
        // Back to the row it is, and the danger it plays at.
        a.row = order[usize::from(a.row)] as u8;
        a.def = rows.areas[usize::from(a.row)];
        a.def.radius = down(a.def.radius);
    }
    areas.sort_by_key(|a| a.row);
    let missing =
        rows.areas.iter().enumerate().position(|(i, a)| a.required && areas.iter().all(|p| usize::from(p.row) != i));
    if let Some(i) = missing {
        return Err(Refused::Area(i as u8));
    }
    let mut walks: Vec<Walk<'_>> = roads.iter().map(|r| Walk { from: r.from, to: r.to, cells: &r.cells }).collect();
    walks.push(Walk { from: RoadEnd::Rail, to: RoadEnd::Rail, cells: &rail });
    let first = [
        RoadEnd::Site(u16::from(named.station)),
        RoadEnd::Site(u16::from(named.julie_house)),
        RoadEnd::Site(u16::from(named.town)),
    ];
    let mut pois = place_pois(&ctx, &walks, rows.pois, first, seed, attempt);
    drop(walks);
    light_lamps(seed, attempt, &mut road, &roads, &sites[usize::from(named.town)], &safe, &t);

    // The anchors name the farm's road as the town's (the plain county hangs the farm off the town);
    // the plan hangs it off Julie's gate, and the walk from the town goes by her door all the same.
    let town = RoadEnd::Site(u16::from(named.town));
    let farm = sites.iter().position(|s| s.def.id == "farm").map(|i| RoadEnd::Site(i as u16));
    let aliased: Vec<Road> = roads
        .iter()
        .map(|r| {
            let mut r = r.clone();
            if Some(r.to) == farm && r.from == RoadEnd::Site(u16::from(named.julie_house)) {
                r.from = town;
            }
            r
        })
        .collect();
    // The anchors are asked for at the rows' distances scaled, and for looser ones if they will not go.
    let mut placed = None;
    'levels: for level in 0..ANCHOR_LEVELS {
        // The last lamp of a short road is three cells from its start, which is a hub's yard; from the far
        // end of the road it has room.
        let back: Vec<Road> = aliased
            .iter()
            .map(|r| {
                let mut r = r.clone();
                if level >= 2 && r.cells.len() < SHORT_ROAD && !first_walk.contains(&(r.from, r.to)) {
                    r.cells.reverse();
                    (r.from, r.to) = (r.to, r.from);
                }
                r
            })
            .collect();
        let anchor_rows = anchor_rows(rows.anchors, level);
        // A row that finds its place taken by one laid before it may find another on other dice.
        for retry in 0..ANCHOR_RETRIES {
            let (mut lit, mut small) = (road.clone(), pois.clone());
            let mut actx = AnchorCtx {
                t: &t,
                road: &mut lit,
                roads: &back,
                sites: &sites,
                areas: &areas,
                pois: &mut small,
                safe: &safe,
                near_rail: &near_rail,
                first_walk,
                allow: Some(Allow(&roadside)),
            };
            if let Some(anchors) = place_anchors(&mut actx, &anchor_rows, seed, attempt.wrapping_add(retry * 41)) {
                placed = Some((anchors, lit, small));
                break 'levels;
            }
        }
    }
    let Some((anchors, lit, small)) = placed else {
        // The first row that will not go, on the loosest rows: the words of the refusal.
        let loosest = anchor_rows(rows.anchors, ANCHOR_LEVELS - 1);
        let stuck = (1..=loosest.len()).find(|&n| {
            let (mut lit, mut small) = (road.clone(), pois.clone());
            let mut actx = AnchorCtx {
                t: &t,
                road: &mut lit,
                roads: &aliased,
                sites: &sites,
                areas: &areas,
                pois: &mut small,
                safe: &safe,
                near_rail: &near_rail,
                first_walk,
                allow: Some(Allow(&roadside)),
            };
            place_anchors(&mut actx, &loosest[..n], seed, attempt).is_none()
        });
        return Err(Refused::Anchor(stuck.map_or(0, |n| (n - 1) as u8)));
    };
    (road, pois) = (lit, small);
    let threat = threat::by_district(plan, &t, &sites, &areas, &road);
    // P-8: the walk from the Halt to the Castle is a walk by day.
    let worst = safe.as_slice().iter().zip(threat.as_slice()).filter(|(on, _)| **on).map(|(_, &t)| t).max();
    if worst.is_some_and(|t| t > 1) {
        return Err(Refused::Walk);
    }
    Ok(Skeleton {
        seed,
        attempt,
        terrain: t,
        road,
        threat,
        sites,
        areas,
        pois,
        anchors,
        roads,
        rail,
        named,
        checks: Vec::new(),
        ok: true,
        stats: GenStats::default(),
    })
}
