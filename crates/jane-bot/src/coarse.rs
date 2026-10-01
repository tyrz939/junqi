//! The long way round: a plan over blocks of a zone, for a goal beyond the walker's window.
//!
//! The walker's A* ([`crate::nav`]) searches a window about her; a goal outside it gets the best
//! partial path toward it, and on a county 2 km square that is a trap: the road nearest the
//! goal as the crow flies may end at a river the bridge for which is two windows away. So for a
//! far goal the walker first plans over [`BLOCK`]-cell blocks, and walks to a waypoint on that
//! plan inside its window, then the next.
//!
//! A block is not one place: a wall, a hedge or a river through it leaves pieces feet cannot
//! cross between inside it. So the plan's nodes are the pieces (each block's walkable cells,
//! joined four ways inside the block), and two pieces in neighbouring blocks join where a cell
//! of one is beside a cell of the other. A plan over whole blocks once walked her into a block
//! open on both sides of a park wall, called it one place, and its next crossing could never be
//! made from her side: Rusher seed 8 stood by the Museum's far side from minute 98 to the end.
//!
//! The pieces are made from the view's own flags (what feet can cross), once per zone and again
//! every [`REBUILD`] frames (a gate opened, a door shut). A crossing the walker then fails to
//! make is marked bad for a while and planned round.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use jane_core::ZoneId;
use jane_sim::View;

use crate::nav::{road, walkable};

/// Cells a block is on a side.
pub const BLOCK: i32 = 16;
/// Frames before the blocks are made again.
pub const REBUILD: u32 = 60 * 60;
/// Frames a failed crossing is kept off the plan.
pub const BAD_FOR: u32 = 60 * 90;

/// From one piece to a neighbouring one.
type Crossing = (u32, u32);

#[derive(Debug, Default)]
pub struct Coarse {
    zone: Option<ZoneId>,
    w: i32,
    h: i32,
    bw: i32,
    bh: i32,
    /// Per cell: its piece of its block, from 1 (0: feet cannot stand there).
    piece: Vec<u8>,
    /// Per block: its first piece's node.
    first: Vec<u32>,
    /// Per node: its block, whether a road runs in it, and the nodes it joins.
    block: Vec<u32>,
    road: Vec<bool>,
    links: Vec<Vec<u32>>,
    made: u32,
    /// The last plan, start to goal.
    path: Vec<u32>,
    /// Crossings the walker failed to make, with the frame they may be tried again.
    bad: BTreeMap<Crossing, u32>,
    /// Plans made, for the log.
    pub plans: u64,
}

impl Coarse {
    pub fn new() -> Self {
        Self::default()
    }

    fn block_ix(&self, bx: i32, by: i32) -> usize {
        (by * self.bw + bx) as usize
    }

    /// The node of the piece `(x, y)` stands in, if feet can stand there.
    fn node_at(&self, (x, y): (i32, i32)) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        let p = self.piece[(y * self.w + x) as usize];
        (p > 0).then(|| self.first[self.block_ix(x / BLOCK, y / BLOCK)] + u32::from(p) - 1)
    }

    fn build(&mut self, v: &View<'_>) {
        let (w, h) = v.size();
        let (w, h) = (w as i32, h as i32);
        if self.zone != Some(v.zone()) {
            self.bad.clear();
        }
        self.zone = Some(v.zone());
        self.made = v.frame();
        (self.w, self.h) = (w, h);
        self.bw = (w + BLOCK - 1) / BLOCK;
        self.bh = (h + BLOCK - 1) / BLOCK;
        let open: Vec<bool> =
            (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| walkable(v, x, y)).collect();
        self.piece = vec![0; (w * h) as usize];
        self.first = vec![0; (self.bw * self.bh) as usize];
        self.block.clear();
        self.road.clear();
        self.links.clear();
        let mut stack: Vec<(i32, i32)> = Vec::new();
        for by in 0..self.bh {
            for bx in 0..self.bw {
                let b = self.block_ix(bx, by);
                self.first[b] = self.block.len() as u32;
                let (x0, y0) = (bx * BLOCK, by * BLOCK);
                let (x1, y1) = ((x0 + BLOCK).min(w), (y0 + BLOCK).min(h));
                let mut pieces: u8 = 0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * w + x) as usize;
                        if !open[i] || self.piece[i] != 0 || pieces == u8::MAX {
                            continue;
                        }
                        pieces += 1;
                        let mut on_road = false;
                        self.piece[i] = pieces;
                        stack.push((x, y));
                        while let Some((cx, cy)) = stack.pop() {
                            on_road |= road(v.tile(cx, cy));
                            for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                                if nx < x0 || ny < y0 || nx >= x1 || ny >= y1 {
                                    continue;
                                }
                                let j = (ny * w + nx) as usize;
                                if open[j] && self.piece[j] == 0 {
                                    self.piece[j] = pieces;
                                    stack.push((nx, ny));
                                }
                            }
                        }
                        self.block.push(b as u32);
                        self.road.push(on_road);
                        self.links.push(Vec::new());
                    }
                }
            }
        }
        // Pieces joined across a block's east and south edges.
        for y in 0..h {
            for x in 0..w {
                let east = x % BLOCK == BLOCK - 1 && x + 1 < w;
                let south = y % BLOCK == BLOCK - 1 && y + 1 < h;
                if !east && !south {
                    continue;
                }
                let Some(a) = self.node_at((x, y)) else { continue };
                for (go, n) in [(east, (x + 1, y)), (south, (x, y + 1))] {
                    if let Some(b) = go.then(|| self.node_at(n)).flatten() {
                        if !self.links[a as usize].contains(&b) {
                            self.links[a as usize].push(b);
                            self.links[b as usize].push(a);
                        }
                    }
                }
            }
        }
    }

    /// The block (x, y) of a node.
    fn block_xy(&self, n: u32) -> (i32, i32) {
        let b = self.block[n as usize] as i32;
        (b % self.bw, b / self.bw)
    }

    /// A cell to walk to next on the way from `from` to a far `goal`: on the plan over pieces,
    /// the farthest piece within `reach` cells of her, at a cell of it nearest its block's
    /// middle. `None` when the pieces join no way there.
    pub fn waypoint(
        &mut self,
        v: &View<'_>,
        from: (i32, i32),
        goal: (i32, i32),
        roads: bool,
        reach: i32,
        danger: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        if self.zone != Some(v.zone()) || v.frame() >= self.made + REBUILD {
            self.build(v);
        }
        self.plans += 1;
        let now = v.frame();
        // Feet on a cell the flags call shut (a door's sill, a prop's edge): the nearest open one.
        let near = |c: (i32, i32)| crate::nav::nearest_walkable(v, c.0, c.1, 2).unwrap_or(c);
        let (start, end) = (self.node_at(near(from))?, self.node_at(near(goal))?);
        let gb = (goal.0 / BLOCK, goal.1 / BLOCK);
        // A* over the pieces: a step into a road's piece 7 (with `roads`), any other 10, and a
        // piece by a place she died is gone round if the way round is not long. The heuristic,
        // blocks across and down at the road's 7, never overestimates.
        let n = self.block.len();
        let mut cost = vec![u32::MAX; n];
        let mut prev = vec![u32::MAX; n];
        let mut open = BinaryHeap::new();
        let h = |c: &Self, k: u32| {
            let (bx, by) = c.block_xy(k);
            ((bx - gb.0).unsigned_abs() + (by - gb.1).unsigned_abs()) * 7
        };
        cost[start as usize] = 0;
        open.push(Reverse((h(self, start), start)));
        while let Some(Reverse((f, a))) = open.pop() {
            if a == end {
                break;
            }
            let ca = cost[a as usize];
            if f > ca + h(self, a) {
                continue;
            }
            for &b in &self.links[a as usize] {
                if self.bad.get(&(a, b)).is_some_and(|&t| t > now) {
                    continue;
                }
                let base = if roads && self.road[b as usize] { 7 } else { 10 };
                let (bx, by) = self.block_xy(b);
                let mid = (bx * BLOCK + BLOCK / 2, by * BLOCK + BLOCK / 2);
                let risky = crate::nav::near_danger(danger, mid, crate::nav::DANGER_R + BLOCK / 2);
                let c = ca + if risky { base + 60 } else { base };
                if c < cost[b as usize] {
                    cost[b as usize] = c;
                    prev[b as usize] = a;
                    open.push(Reverse((c + h(self, b), b)));
                }
            }
        }
        if cost[end as usize] == u32::MAX {
            return None;
        }
        self.path.clear();
        let mut k = end;
        while k != start {
            self.path.push(k);
            k = prev[k as usize];
        }
        self.path.push(start);
        self.path.reverse();
        // The farthest piece on the plan still inside her reach.
        let within = |c: &Self, k: u32| {
            let (bx, by) = c.block_xy(k);
            let (cx, cy) = (bx * BLOCK + BLOCK / 2, by * BLOCK + BLOCK / 2);
            (cx - from.0).abs().max((cy - from.1).abs()) <= reach
        };
        let at = self.path.iter().rposition(|&k| within(self, k))?;
        let k = self.path[at];
        let (bx, by) = self.block_xy(k);
        let p = (k - self.first[self.block_ix(bx, by)] + 1) as u8;
        let (cx, cy) = (bx * BLOCK + BLOCK / 2, by * BLOCK + BLOCK / 2);
        let mut best: Option<(i32, (i32, i32))> = None;
        for y in by * BLOCK..((by + 1) * BLOCK).min(self.h) {
            for x in bx * BLOCK..((bx + 1) * BLOCK).min(self.w) {
                if self.piece[(y * self.w + x) as usize] == p {
                    let d = (x - cx).abs() + (y - cy).abs();
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, (x, y)));
                    }
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// The walker could not get from the piece she stands in toward `to`'s: keep that crossing
    /// off the plan for a while.
    pub fn failed(&mut self, from: (i32, i32), to: (i32, i32), now: u32) {
        if self.piece.is_empty() {
            return;
        }
        let (Some(a), b) = (self.node_at(from), self.node_at(to)) else { return };
        // The first step of the plan out of her piece.
        if let Some(i) = self.path.iter().position(|&p| p == a) {
            if let Some(&n) = self.path.get(i + 1) {
                self.bad.insert((a, n), now + BAD_FOR);
                return;
            }
        }
        if let Some(b) = b.filter(|&b| b != a) {
            self.bad.insert((a, b), now + BAD_FOR);
        }
    }
}
