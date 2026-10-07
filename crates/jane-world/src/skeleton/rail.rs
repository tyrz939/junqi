//! The railway. One train a week stops at Castle Halt, and it came from somewhere and goes on
//! somewhere: the line does not stop at the ends of the platform. It runs in from the south edge
//! along the county's western fence, past the halt, north to the Works, then east across the Works
//! (past the yards, over the river on a trestle, across the roads on the level) and out at the
//! eastern fence. Carries `jane/src/world/skeleton/rail.ts`, with the costs in tenths.

use alloc::collections::BinaryHeap;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;

use jane_core::ZoneId;
use jane_core::grid::Grid;

use super::roads::ROAD;
use super::terrain::Terrain;
use super::types::{MACRO, Region, SKEL_H, SKEL_W, Water, inside, metres_sq};
use crate::steps::{Step, dice};

/// Nothing the line passes comes nearer a set place than this (m): the biggest chunk is 60 x 44 cells.
pub const KEEP_OFF_SITES: i32 = 96;
/// Headings: east, south, west, north, so `(d + 2) & 3` is the way back.
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
const NORTH: usize = 3;

/// What the search charges, in tenths of a macro cell of straight line. A test re-tunes these and
/// checks what else moves (streams).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailCost {
    /// A change of heading.
    pub turn: u32,
    /// Per height step between two cells.
    pub slope: u32,
    /// Per row nearer the north fence than row 10.
    pub fence: u32,
    /// A trestle over the river.
    pub water: u32,
    /// A level crossing: dear, so it never runs along a road.
    pub road: u32,
    /// Out of the Works, when it must.
    pub outside_works: u32,
}

impl Default for RailCost {
    fn default() -> Self {
        Self { turn: 60, slope: 6, fence: 4, water: 60, road: 50, outside_works: 40 }
    }
}

/// The line, macro cells in order from the south edge, through the halt, to the east edge. With no
/// way east (the lake and the sites could in principle wall it off) it keeps to the west fence and
/// leaves by the north edge: it never stops dead. `sites` are the other set places it keeps off.
pub fn lay_rail(
    seed: u32,
    attempt: u8,
    t: &Terrain,
    road: &Grid<u8>,
    station: (i32, i32),
    sites: &[(i32, i32)],
    cost: RailCost,
) -> Vec<(i32, i32)> {
    let mut rng = dice(seed, ZoneId::County, Step::SkelRail, attempt, 0, 0);
    let mut out = Vec::new();
    // In from the south edge, up the west fence to the halt.
    for y in (station.1 + 1..SKEL_H).rev() {
        out.push((0, y));
    }
    // Past the halt, north until the Works have begun and a little more: it turns east in the yards.
    let mut turn = station.1;
    while turn > 4 && t.region.read(0, turn, Region::Works) != Region::Works {
        turn -= 1;
    }
    let turn = (turn - 3).max(4);
    for y in (turn + 1..=station.1).rev() {
        out.push((0, y));
    }
    // Where it leaves: a row of the east edge still in the Works, clear of the corners.
    let exits: Vec<i32> = (6..SKEL_H - 6)
        .filter(|&y| {
            t.region.read(SKEL_W - 1, y, Region::Lowfields) == Region::Works
                && t.water.read(SKEL_W - 1, y, Water::Dry) != Water::Lake
        })
        .collect();
    if !exits.is_empty() {
        let pick = (rng.below((exits.len() as u32).saturating_sub(3).max(1)) as usize).min(exits.len() - 1);
        let east = exits[pick];
        let across =
            search(t, road, sites, turn, east, true, cost).or_else(|| search(t, road, sites, turn, east, false, cost));
        if let Some(across) = across {
            out.extend(across);
            return out;
        }
    }
    for y in (0..=turn).rev() {
        out.push((0, y));
    }
    out
}

fn search(
    t: &Terrain,
    road: &Grid<u8>,
    sites: &[(i32, i32)],
    from_y: i32,
    to_y: i32,
    works_only: bool,
    cost: RailCost,
) -> Option<Vec<(i32, i32)>> {
    let (w, h) = (SKEL_W as u32, SKEL_H as u32);
    let mut blocked = Grid::new(w, h, false);
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            // The lake is never crossed; the rows under the tree line are the fence, not the line.
            let water = t.water.read(x, y, Water::Dry);
            let b = water == Water::Lake
                || !(4..=SKEL_H - 5).contains(&y)
                || (works_only && t.region.read(x, y, Region::Works) != Region::Works && water == Water::Dry);
            blocked.set(x, y, b);
        }
    }
    let keep = i64::from(KEEP_OFF_SITES) * i64::from(KEEP_OFF_SITES);
    let r = (KEEP_OFF_SITES + MACRO - 1) / MACRO;
    for &(sx, sy) in sites {
        for y in sy - r..=sy + r {
            for x in sx - r..=sx + r {
                if inside(x, y) && metres_sq(x, y, sx, sy) <= keep {
                    blocked.set(x, y, true);
                }
            }
        }
    }
    let start = (0, from_y);
    let goal = (SKEL_W - 1, to_y);
    blocked.set(start.0, start.1, false);
    blocked.set(goal.0, goal.1, false);

    // Over (cell, heading): long straights and few, wide bends, square to the grid. county rounds
    // each bend into a curve.
    let n = (w * h) as usize * 4;
    let mut dist = vec![u32::MAX; n];
    let mut prev = vec![u32::MAX; n];
    let mut closed = vec![false; n];
    let mut heap = BinaryHeap::new();
    let state = |x: i32, y: i32, d: usize| ((y * SKEL_W + x) as usize * 4 + d) as u32;
    let hcost = |x: i32, y: i32| 10 * ((goal.0 - x).unsigned_abs() + (to_y - y).unsigned_abs());
    // It comes in heading north, up the west fence.
    let first = state(start.0, start.1, NORTH);
    dist[first as usize] = 0;
    heap.push(Reverse((hcost(start.0, start.1), first)));
    let mut end = None;
    while let Some(Reverse((_, s))) = heap.pop() {
        if closed[s as usize] {
            continue;
        }
        let cell = (s >> 2) as i32;
        let (cx, cy) = (cell % SKEL_W, cell / SKEL_W);
        if (cx, cy) == goal {
            end = Some(s);
            break;
        }
        closed[s as usize] = true;
        let dir = (s & 3) as usize;
        for (d, &(dx, dy)) in DIRS.iter().enumerate() {
            if d == (dir + 2) & 3 {
                continue; // it never doubles back
            }
            let (nx, ny) = (cx + dx, cy + dy);
            if !inside(nx, ny) || blocked.read(nx, ny, true) {
                continue;
            }
            let ns = state(nx, ny, d);
            if closed[ns as usize] {
                continue;
            }
            // Only the exit, and the line still running up the west fence, may stand on the side columns.
            if (nx == 0 || nx == SKEL_W - 1) && (nx, ny) != goal && !(nx == 0 && cx == 0 && d == NORTH) {
                continue;
            }
            // A railway hates a gradient, would rather not build a trestle, and meets a road square on and once.
            let slope = u32::from(t.height.read(nx, ny, 0).abs_diff(t.height.read(cx, cy, 0)));
            let mut step = 10 + slope * cost.slope;
            if d != dir {
                step += cost.turn;
            }
            // It keeps in from the fence: a line that hugs the tree line is going nowhere.
            if ny < 10 {
                step += (10 - ny) as u32 * cost.fence;
            }
            if t.water.read(nx, ny, Water::Dry) == Water::River {
                step += cost.water;
            }
            if road.read(nx, ny, 0) & ROAD != 0 {
                step += cost.road;
            }
            if !works_only && t.region.read(nx, ny, Region::Works) != Region::Works {
                step += cost.outside_works;
            }
            let c = dist[s as usize] + step;
            if c < dist[ns as usize] {
                dist[ns as usize] = c;
                prev[ns as usize] = s;
                heap.push(Reverse((c + hcost(nx, ny), ns)));
            }
        }
    }
    let end = end?;
    let mut cells = Vec::new();
    let mut s = end;
    while s != u32::MAX {
        let cell = (s >> 2) as i32;
        cells.push((cell % SKEL_W, cell / SKEL_W));
        s = prev[s as usize];
    }
    cells.reverse();
    Some(cells)
}

/// Every macro cell the line is on, and with `margin` the cells beside it.
pub fn rail_mask(rail: &[(i32, i32)], margin: i32) -> Grid<bool> {
    let mut mask = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    for &(x, y) in rail {
        for oy in -margin..=margin {
            for ox in -margin..=margin {
                mask.set(x + ox, y + oy, true);
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::build_terrain;

    #[test]
    fn the_line_runs_off_both_edges_and_never_doubles_back() {
        for seed in 1..=16 {
            let t = build_terrain(seed, 0);
            let road = Grid::new(SKEL_W as u32, SKEL_H as u32, 0u8);
            let station = (0, 90);
            let rail = lay_rail(seed, 0, &t, &road, station, &[(20, 60)], RailCost::default());
            assert_eq!(rail.first(), Some(&(0, SKEL_H - 1)), "seed {seed}: starts at the south edge");
            let &(ex, ey) = rail.last().unwrap();
            assert!(ex == SKEL_W - 1 || ey == 0, "seed {seed}: ends at ({ex},{ey})");
            assert!(rail.contains(&station), "seed {seed}: misses the halt");
            for w in rail.windows(2) {
                assert_eq!((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs(), 1, "seed {seed}: a gap in the line");
            }
            let mut sorted = rail.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(sorted.len(), rail.len(), "seed {seed}: the line crosses itself");
            assert!(rail.iter().all(|&(x, y)| t.water.read(x, y, Water::Dry) != Water::Lake));
            assert!(rail.iter().all(|&(x, y)| metres_sq(x, y, 20, 60) > i64::from(KEEP_OFF_SITES * KEEP_OFF_SITES)));
        }
    }
}
