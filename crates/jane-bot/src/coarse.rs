//! The long way round: a plan over blocks of a zone, for a goal beyond the walker's window.
//!
//! The walker's A* ([`crate::nav`]) searches a window about her; a goal outside it gets the best
//! partial path toward it, and on a county 2 km square that is a trap: the road nearest the
//! goal as the crow flies may end at a river the bridge for which is two windows away. So for a
//! far goal the walker first plans over [`BLOCK`]-cell blocks (a block is open if feet can stand
//! anywhere in it, and two neighbours join if feet can cross the edge between them somewhere),
//! and walks to a waypoint on that plan inside its window, then the next.
//!
//! The blocks are made from the view's own flags (what feet can cross), once per zone and again
//! every [`REBUILD`] frames (a gate opened, a door shut). A crossing the walker then fails to
//! make is marked bad for a while and planned round.

use std::collections::BTreeMap;

use jane_core::ZoneId;
use jane_core::search::{Astar, PathEnd, PathQuery, octile_to};
use jane_sim::View;

use crate::nav::{road, walkable};

/// Cells a block is on a side.
pub const BLOCK: i32 = 16;
/// Frames before the blocks are made again.
pub const REBUILD: u32 = 60 * 60;
/// Frames a failed crossing is kept off the plan.
pub const BAD_FOR: u32 = 60 * 90;

/// From one block to a neighbour.
type Crossing = ((i32, i32), (i32, i32));

#[derive(Debug)]
pub struct Coarse {
    zone: Option<ZoneId>,
    bw: i32,
    bh: i32,
    open: Vec<bool>,
    road: Vec<bool>,
    /// Feet can cross from block (x, y) to (x + 1, y); to (x, y + 1).
    east: Vec<bool>,
    south: Vec<bool>,
    made: u32,
    astar: Option<Astar>,
    path: Vec<(i32, i32)>,
    /// Block crossings the walker failed to make, with the frame they may be tried again.
    bad: BTreeMap<Crossing, u32>,
    /// Plans made, for the log.
    pub plans: u64,
}

impl Default for Coarse {
    fn default() -> Self {
        Self::new()
    }
}

impl Coarse {
    pub fn new() -> Self {
        Self {
            zone: None,
            bw: 0,
            bh: 0,
            open: Vec::new(),
            road: Vec::new(),
            east: Vec::new(),
            south: Vec::new(),
            made: 0,
            astar: None,
            path: Vec::new(),
            bad: BTreeMap::new(),
            plans: 0,
        }
    }

    fn ix(&self, bx: i32, by: i32) -> usize {
        (by * self.bw + bx) as usize
    }

    fn build(&mut self, v: &View<'_>) {
        let (w, h) = v.size();
        let (w, h) = (w as i32, h as i32);
        self.zone = Some(v.zone());
        self.made = v.frame();
        self.bw = (w + BLOCK - 1) / BLOCK;
        self.bh = (h + BLOCK - 1) / BLOCK;
        let n = (self.bw * self.bh) as usize;
        self.open = vec![false; n];
        self.road = vec![false; n];
        self.east = vec![false; n];
        self.south = vec![false; n];
        for y in 0..h {
            for x in 0..w {
                if walkable(v, x, y) {
                    let i = self.ix(x / BLOCK, y / BLOCK);
                    self.open[i] = true;
                    if !self.road[i] && road(v.tile(x, y)) {
                        self.road[i] = true;
                    }
                    // A crossing out of the block's east or south edge.
                    if x % BLOCK == BLOCK - 1 && walkable(v, x + 1, y) {
                        self.east[i] = true;
                    }
                    if y % BLOCK == BLOCK - 1 && walkable(v, x, y + 1) {
                        self.south[i] = true;
                    }
                }
            }
        }
        self.astar = Some(Astar::new(self.bw.max(1) as u32, self.bh.max(1) as u32));
    }

    /// Can feet cross from block `a` to its neighbour `b`?
    fn joins(&self, a: (i32, i32), b: (i32, i32), now: u32) -> bool {
        if b.0 < 0 || b.1 < 0 || b.0 >= self.bw || b.1 >= self.bh || !self.open[self.ix(b.0, b.1)] {
            return false;
        }
        if self.bad.get(&(a, b)).is_some_and(|&t| t > now) {
            return false;
        }
        match (b.0 - a.0, b.1 - a.1) {
            (1, 0) => self.east[self.ix(a.0, a.1)],
            (-1, 0) => self.east[self.ix(b.0, b.1)],
            (0, 1) => self.south[self.ix(a.0, a.1)],
            (0, -1) => self.south[self.ix(b.0, b.1)],
            _ => false,
        }
    }

    /// A cell to walk to next on the way from `from` to a far `goal`: on the block plan, the
    /// farthest block within `reach` cells of her, at a cell feet can stand on in it (the one
    /// nearest its middle). `None` when the blocks join no way there.
    pub fn waypoint(
        &mut self,
        v: &View<'_>,
        from: (i32, i32),
        goal: (i32, i32),
        roads: bool,
        reach: i32,
    ) -> Option<(i32, i32)> {
        if self.zone != Some(v.zone()) || v.frame() >= self.made + REBUILD {
            self.build(v);
        }
        self.plans += 1;
        let now = v.frame();
        let (sb, gb) = ((from.0 / BLOCK, from.1 / BLOCK), (goal.0 / BLOCK, goal.1 / BLOCK));
        let q = PathQuery {
            grid_w: self.bw as u32,
            grid_h: self.bh as u32,
            start: sb,
            goal: gb,
            max_cost: 10 * 100_000,
            budget: (self.bw * self.bh) as u32,
            partial: false,
            window: None,
            cut_corners: false,
        };
        let mut astar = self.astar.take().expect("built");
        let mut path = std::mem::take(&mut self.path);
        let end = {
            let this = &*self;
            let step = |a: (i32, i32), b: (i32, i32)| -> Option<u32> {
                if !this.joins(a, b, now) {
                    return None;
                }
                Some(if roads && this.road[this.ix(b.0, b.1)] { 7 } else { 10 })
            };
            // The octile heuristic in tenths overestimates road steps at 7: scale it down.
            let h = octile_to(gb);
            astar.find(&q, step, |c| h(c) * 7 / 10, &mut path)
        };
        self.astar = Some(astar);
        self.path = path;
        if end != PathEnd::Found {
            return None;
        }
        // The farthest block on the plan still inside her reach.
        let within = |b: (i32, i32)| {
            let (cx, cy) = (b.0 * BLOCK + BLOCK / 2, b.1 * BLOCK + BLOCK / 2);
            (cx - from.0).abs().max((cy - from.1).abs()) <= reach
        };
        let k = self.path.iter().rposition(|&b| within(b))?;
        let b = self.path[k];
        let (cx, cy) = (b.0 * BLOCK + BLOCK / 2, b.1 * BLOCK + BLOCK / 2);
        let mut best: Option<(i32, (i32, i32))> = None;
        for y in b.1 * BLOCK..(b.1 + 1) * BLOCK {
            for x in b.0 * BLOCK..(b.0 + 1) * BLOCK {
                if walkable(v, x, y) {
                    let d = (x - cx).abs() + (y - cy).abs();
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, (x, y)));
                    }
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// The walker could not get from the block she stands in toward `to`'s block: keep that
    /// crossing off the plan for a while.
    pub fn failed(&mut self, from: (i32, i32), to: (i32, i32), now: u32) {
        let (a, b) = ((from.0 / BLOCK, from.1 / BLOCK), (to.0 / BLOCK, to.1 / BLOCK));
        // The first step of the plan out of her block.
        if let Some(i) = self.path.iter().position(|&p| p == a) {
            if let Some(&n) = self.path.get(i + 1) {
                self.bad.insert((a, n), now + BAD_FOR);
                return;
            }
        }
        if a != b {
            self.bad.insert((a, b), now + BAD_FOR);
        }
    }
}
