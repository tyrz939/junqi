//! Layer 2 of the skeleton: roads, routed site to site over a cost field, so every seed's
//! network is different and always joins the story up. Later roads prefer cells an earlier road
//! already used, which makes a network (and one bridge) rather than a bundle of lines.
//! Carries `jane/src/world/skeleton/roads.ts`.
//!
//! Costs are integers in Q8 x tenths of a macro cell (ARCHITECTURE.md §2): a step is 10 straight
//! or 14 diagonal, times the cell's cost in Q8 (256 = 1.0). Walking distances along the network
//! are tenths of a metre: 160 straight, 226 diagonal.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use jane_core::grid::{Grid, Rect};
use jane_core::num::octile10;
use jane_core::search::{Astar, PathEnd, PathQuery, chamfer};

use super::terrain::Terrain;
use super::types::{MACRO, SKEL_H, SKEL_W, Water, inside};

/// Road bits of a macro cell.
pub const ROAD: u8 = 1;
pub const ROAD_LIT: u8 = 2;
pub const ROAD_BRIDGE: u8 = 4;

/// Along an existing road: the cheapest step there is (0.3 a cell).
const ON_ROAD_Q8: u32 = 77;
/// Bridging the river, dearly (30 a cell).
const RIVER_Q8: u32 = 7680;
/// Climbing: 0.22 a height step.
const SLOPE_Q8: u32 = 56;
/// How far outside the box of its two ends a road may wander, in macro cells (about 550 m).
pub const CORRIDOR: i32 = 34;

/// Walking distance along roads, tenths of a metre, per straight and diagonal macro step.
pub const ALONG_STRAIGHT: u32 = MACRO as u32 * 10;
pub const ALONG_DIAGONAL: u32 = 226;
/// Not reached along the network.
pub const FAR: u32 = u32::MAX;

/// What a road joins: a site row by index, or the railway walked like a road.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoadEnd {
    Site(u16),
    Rail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Road {
    pub from: RoadEnd,
    pub to: RoadEnd,
    /// Macro cells, `(x, y)`, from `from` to `to`.
    pub cells: Vec<(i32, i32)>,
    /// Its length in whole metres.
    pub metres: u32,
}

/// The land a road is laid over, and the road bits laid so far.
#[derive(Clone, Debug)]
pub struct Land<'a> {
    pub terrain: &'a Terrain,
    pub road: Grid<u8>,
}

impl<'a> Land<'a> {
    pub fn new(terrain: &'a Terrain) -> Self {
        Self { terrain, road: Grid::new(SKEL_W as u32, SKEL_H as u32, 0) }
    }

    /// The cost of one step, `None` where no road may go (the lake).
    fn step_cost(&self, from: (i32, i32), to: (i32, i32)) -> Option<u32> {
        let t = self.terrain;
        let len10 = if from.0 != to.0 && from.1 != to.1 { 14 } else { 10 };
        let per = match t.water.read(to.0, to.1, Water::Lake) {
            Water::Lake => return None,
            _ if self.road.read(to.0, to.1, 0) & ROAD != 0 => ON_ROAD_Q8,
            Water::River => RIVER_Q8,
            Water::Dry => {
                let slope = u32::from(t.height.read(to.0, to.1, 0).abs_diff(t.height.read(from.0, from.1, 0)));
                u32::from(t.rough.read(to.0, to.1, 0)) + slope * SLOPE_Q8
            }
        };
        Some(len10 * per)
    }
}

/// A* scratch sized for the whole macro grid, reused by every road of a build.
#[derive(Debug)]
pub struct Router {
    astar: Astar,
    out: Vec<(i32, i32)>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        Self { astar: Astar::new(SKEL_W as u32, SKEL_H as u32), out: Vec::new() }
    }

    /// The cheapest way from `a` to `b` over the cost field: inside a corridor around the two
    /// ends first (most roads are short and the map is not), then over the whole map if the
    /// corridor holds no way through. `None` if there is no way at all. The path includes `a`.
    pub fn route(&mut self, land: &Land<'_>, a: (i32, i32), b: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        self.search(land, a, b, CORRIDOR).or_else(|| self.search(land, a, b, SKEL_W))
    }

    fn search(&mut self, land: &Land<'_>, a: (i32, i32), b: (i32, i32), margin: i32) -> Option<Vec<(i32, i32)>> {
        let x0 = a.0.min(b.0) - margin;
        let y0 = a.1.min(b.1) - margin;
        let window = Rect::new(x0, y0, a.0.max(b.0) + margin - x0 + 1, a.1.max(b.1) + margin - y0 + 1);
        let q = PathQuery {
            grid_w: SKEL_W as u32,
            grid_h: SKEL_H as u32,
            start: a,
            goal: b,
            max_cost: u32::MAX,
            budget: u32::MAX,
            partial: false,
            window: Some(window),
            cut_corners: true,
        };
        let on_edge = |(x, y): (i32, i32)| x == 0 || y == 0 || x == SKEL_W - 1 || y == SKEL_H - 1;
        let step = |from: (i32, i32), to: (i32, i32)| {
            // Roads keep one cell off the map's edge, except where a site sits on it (the station).
            if on_edge(to) && to != a && to != b {
                return None;
            }
            land.step_cost(from, to)
        };
        // No step is cheaper than an existing road, so the road rate times the octile distance never overestimates.
        let h = move |(x, y): (i32, i32)| ON_ROAD_Q8 * octile10(b.0 - x, b.1 - y);
        match self.astar.find(&q, step, h, &mut self.out) {
            PathEnd::Found => {
                let mut cells = Vec::with_capacity(self.out.len() + 1);
                cells.push(a);
                cells.extend_from_slice(&self.out);
                Some(cells)
            }
            PathEnd::Partial | PathEnd::None => None,
        }
    }
}

/// Lay a road: set its bits (a river cell becomes a bridge) and measure it.
pub fn lay_road(land: &mut Land<'_>, from: RoadEnd, to: RoadEnd, cells: Vec<(i32, i32)>) -> Road {
    let mut len10 = 0u32;
    for (i, &(x, y)) in cells.iter().enumerate() {
        let mut bits = land.road.read(x, y, 0) | ROAD;
        if land.terrain.water.read(x, y, Water::Dry) == Water::River {
            bits |= ROAD_BRIDGE;
        }
        land.road.set(x, y, bits);
        if i > 0 {
            let (px, py) = cells[i - 1];
            len10 += if px != x && py != y { 14 } else { 10 };
        }
    }
    let metres = (len10 * MACRO as u32 + 5) / 10;
    Road { from, to, cells, metres }
}

/// Walking distance along the road network from `(fx, fy)` to every road cell, in tenths of a
/// metre; [`FAR`] where the network does not reach. The start counts as on the network.
pub fn road_distances(road: &Grid<u8>, fx: i32, fy: i32) -> Grid<u32> {
    let mut dist = Grid::new(SKEL_W as u32, SKEL_H as u32, FAR);
    let mut heap = BinaryHeap::new();
    dist.set(fx, fy, 0);
    heap.push(Reverse((0u32, fy * SKEL_W + fx)));
    while let Some(Reverse((cost, cell))) = heap.pop() {
        let (cx, cy) = (cell % SKEL_W, cell / SKEL_W);
        if cost > dist.read(cx, cy, FAR) {
            continue;
        }
        for &(dx, dy) in &jane_core::grid::DIRS8 {
            let (nx, ny) = (cx + dx, cy + dy);
            if !inside(nx, ny) || road.read(nx, ny, 0) & ROAD == 0 {
                continue;
            }
            let c = cost + if dx != 0 && dy != 0 { ALONG_DIAGONAL } else { ALONG_STRAIGHT };
            if c < dist.read(nx, ny, FAR) {
                dist.set(nx, ny, c);
                heap.push(Reverse((c, ny * SKEL_W + nx)));
            }
        }
    }
    dist
}

/// Chamfer distance from every macro cell to the nearest cell with `bit` set, in tenths of a
/// macro cell (10 straight, 14 diagonal).
pub fn distance_to(mask: &Grid<u8>, bit: u8) -> Grid<u16> {
    chamfer(mask.w(), mask.h(), |x, y| mask.read(x, y, 0) & bit != 0)
}

/// Bridge cells within two cells of each other are one bridge.
pub fn count_bridges(road: &Grid<u8>) -> u32 {
    let mut seen = Grid::new(road.w(), road.h(), false);
    let mut n = 0;
    let mut stack = Vec::new();
    for y in 0..SKEL_H {
        for x in 0..SKEL_W {
            if road.read(x, y, 0) & ROAD_BRIDGE == 0 || seen.read(x, y, true) {
                continue;
            }
            n += 1;
            seen.set(x, y, true);
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                for oy in -2..=2 {
                    for ox in -2..=2 {
                        let (nx, ny) = (cx + ox, cy + oy);
                        if inside(nx, ny) && road.read(nx, ny, 0) & ROAD_BRIDGE != 0 && !seen.read(nx, ny, true) {
                            seen.set(nx, ny, true);
                            stack.push((nx, ny));
                        }
                    }
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skeleton::build_terrain;

    #[test]
    fn a_road_joins_its_ends_and_never_wets_its_feet_in_the_lake() {
        for seed in 1..=8 {
            let t = build_terrain(seed, 0);
            let mut land = Land::new(&t);
            let mut router = Router::new();
            // West to east across the river, south of the Works.
            let a = (10, 100);
            let b = (t.lake.mx, t.lake.my - t.lake.r - 4);
            let cells = router.route(&land, a, b).expect("a way east");
            assert_eq!(cells.first(), Some(&a));
            assert_eq!(cells.last(), Some(&b));
            for w in cells.windows(2) {
                assert!((w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1);
            }
            assert!(cells.iter().all(|&(x, y)| t.water.read(x, y, Water::Dry) != Water::Lake));
            let road = lay_road(&mut land, RoadEnd::Site(0), RoadEnd::Site(1), cells);
            assert!(count_bridges(&land.road) >= 1, "seed {seed}: crossed the river without a bridge");
            let d = road_distances(&land.road, a.0, a.1);
            let end = d.read(b.0, b.1, FAR);
            assert_ne!(end, FAR);
            // Along the road is the road's own length, give or take the diagonal's rounding.
            assert!(end.abs_diff(road.metres * 10) <= road.metres / 10 + 10, "{end} vs {} m", road.metres);
            // A second road between the same ends rides the first.
            let again = router.route(&land, a, b).unwrap();
            let shared = again.iter().filter(|&&(x, y)| land.road.read(x, y, 0) & ROAD != 0).count();
            assert!(shared * 10 >= again.len() * 9, "seed {seed}: the second road left the first");
        }
    }

    #[test]
    fn distance_to_roads_is_zero_on_them() {
        let t = build_terrain(3, 0);
        let mut land = Land::new(&t);
        let cells = Router::new().route(&land, (20, 60), (40, 70)).unwrap();
        lay_road(&mut land, RoadEnd::Site(0), RoadEnd::Rail, cells.clone());
        let d = distance_to(&land.road, ROAD);
        for &(x, y) in &cells {
            assert_eq!(d.read(x, y, u16::MAX), 0);
        }
        assert!(d.read(0, 0, 0) > 100);
    }
}
