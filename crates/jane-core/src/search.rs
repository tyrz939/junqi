//! One flood, one chamfer, one A* (PORT.md §6.e). The TypeScript had about nine floods; every
//! reachability question in world and sim asks this one.
//!
//! Neighbour order is fixed ([`DIRS4`], [`DIRS8`]) and is part of determinism. Ties in the A*
//! heap break on the local node index.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::grid::{CellIx, DIRS4, DIRS8, Grid, Rect};
use crate::num::octile10;

/// Which steps a flood takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conn {
    /// E, S, W, N.
    Four,
    /// Eight ways, never cutting a corner: a diagonal needs both orthogonal cells passable.
    Eight,
}

/// The cell was not reached.
pub const UNREACHED: u32 = u32::MAX;

/// What a flood reached: steps from the nearest start, and the cells in the order they were reached.
#[derive(Clone, Debug, Default)]
pub struct Reach {
    w: u32,
    h: u32,
    dist: Vec<u32>,
    order: Vec<CellIx>,
    /// The budget ran out before the frontier did.
    pub exhausted: bool,
}

impl Reach {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn w(&self) -> u32 {
        self.w
    }

    pub fn h(&self) -> u32 {
        self.h
    }

    /// Steps from the nearest start, or [`UNREACHED`]; outside the grid is unreached.
    pub fn dist(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return UNREACHED;
        }
        self.dist[y as usize * self.w as usize + x as usize]
    }

    pub fn reached(&self, x: i32, y: i32) -> bool {
        self.dist(x, y) != UNREACHED
    }

    /// Cells in the order they were reached, starts first.
    pub fn order(&self) -> &[CellIx] {
        &self.order
    }

    pub fn count(&self) -> u32 {
        self.order.len() as u32
    }

    fn reset(&mut self, w: u32, h: u32) {
        self.w = w;
        self.h = h;
        self.dist.clear();
        self.dist.resize((w as usize) * (h as usize), UNREACHED);
        self.order.clear();
        self.exhausted = false;
    }
}

/// Breadth-first flood over a `w x h` grid from `starts`, entering cells `passable` allows, until
/// the frontier empties or `budget` cells are reached. Starts are reached whether passable or not.
/// `reach` is reused between floods; nothing else allocates.
pub fn flood(
    w: u32,
    h: u32,
    starts: &[(i32, i32)],
    conn: Conn,
    budget: u32,
    mut passable: impl FnMut(i32, i32) -> bool,
    reach: &mut Reach,
) {
    reach.reset(w, h);
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h;
    let ix = |x: i32, y: i32| y as usize * w as usize + x as usize;
    for &(x, y) in starts {
        if inside(x, y) && reach.dist[ix(x, y)] == UNREACHED {
            reach.dist[ix(x, y)] = 0;
            reach.order.push(CellIx(y as u32 * w + x as u32));
        }
    }
    let dirs: &[(i32, i32)] = match conn {
        Conn::Four => &DIRS4,
        Conn::Eight => &DIRS8,
    };
    let mut head = 0;
    while head < reach.order.len() {
        let c = reach.order[head];
        head += 1;
        let (x, y) = ((c.0 % w) as i32, (c.0 / w) as i32);
        let d = reach.dist[ix(x, y)];
        for &(dx, dy) in dirs {
            let (nx, ny) = (x + dx, y + dy);
            if !inside(nx, ny) || reach.dist[ix(nx, ny)] != UNREACHED {
                continue;
            }
            if dx != 0 && dy != 0 && !(passable(nx, y) && passable(x, ny)) {
                continue;
            }
            if !passable(nx, ny) {
                continue;
            }
            if reach.order.len() as u32 >= budget {
                reach.exhausted = true;
                return;
            }
            reach.dist[ix(nx, ny)] = d + 1;
            reach.order.push(CellIx(ny as u32 * w + nx as u32));
        }
    }
}

/// Chamfer distance transform in tenths of a cell (10 straight, 14 diagonal), two raster passes,
/// from every cell `source` names. Saturates at `u16::MAX`; with no source every cell is `u16::MAX`.
pub fn chamfer(w: u32, h: u32, mut source: impl FnMut(i32, i32) -> bool) -> Grid<u16> {
    let mut g = Grid::new(w, h, u16::MAX);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if source(x, y) {
                g.set(x, y, 0);
            }
        }
    }
    let relax = |g: &mut Grid<u16>, x: i32, y: i32, nx: i32, ny: i32, step: u16| {
        let n = g.read(nx, ny, u16::MAX);
        let via = n.saturating_add(step);
        if via < g.read(x, y, u16::MAX) {
            g.set(x, y, via);
        }
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            relax(&mut g, x, y, x - 1, y, 10);
            relax(&mut g, x, y, x, y - 1, 10);
            relax(&mut g, x, y, x - 1, y - 1, 14);
            relax(&mut g, x, y, x + 1, y - 1, 14);
        }
    }
    for y in (0..h as i32).rev() {
        for x in (0..w as i32).rev() {
            relax(&mut g, x, y, x + 1, y, 10);
            relax(&mut g, x, y, x, y + 1, 10);
            relax(&mut g, x, y, x + 1, y + 1, 14);
            relax(&mut g, x, y, x - 1, y + 1, 14);
        }
    }
    g
}

/// How a search ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathEnd {
    /// The goal was reached; the path ends on it.
    Found,
    /// The goal was outside the window, or unreachable while `partial` was asked for: the path
    /// ends at the cell nearest the goal by the heuristic.
    Partial,
    /// No path: blocked, over cost, or over budget with nothing better than the start.
    None,
}

/// One search.
#[derive(Clone, Copy, Debug)]
pub struct PathQuery {
    /// The whole grid, for bounds.
    pub grid_w: u32,
    pub grid_h: u32,
    pub start: (i32, i32),
    pub goal: (i32, i32),
    /// Largest `g` accepted, in the step cost's units.
    pub max_cost: u32,
    /// Nodes expanded before giving up.
    pub budget: u32,
    /// Return the best partial path when the goal cannot be reached inside the window.
    pub partial: bool,
}

/// Windowed A* on a grid. All scratch lives in a fixed window centred on the start, so memory is
/// constant whatever the zone's size; "clearing" between searches is a generation bump.
#[derive(Debug)]
pub struct Astar {
    ww: u32,
    wh: u32,
    g: Vec<u32>,
    from: Vec<u32>,
    stamp: Vec<u32>,
    closed: Vec<u32>,
    heap: BinaryHeap<Reverse<(u64, u32)>>,
    generation: u32,
    /// Nodes expanded, summed over searches.
    pub expanded: u64,
}

impl Astar {
    /// Scratch for a `ww x wh` window (the sim uses 256 x 256).
    pub fn new(ww: u32, wh: u32) -> Self {
        let n = (ww as usize) * (wh as usize);
        Self {
            ww,
            wh,
            g: vec![0; n],
            from: vec![0; n],
            stamp: vec![0; n],
            closed: vec![0; n],
            heap: BinaryHeap::with_capacity(n.min(1 << 16)),
            generation: 0,
            expanded: 0,
        }
    }

    /// Find a path from `q.start` toward `q.goal`. `step(from, to)` is the cost of one step
    /// between adjacent cells (both in grid coordinates), or `None` when it cannot be taken;
    /// a diagonal step is offered only after both orthogonal steps are `Some` (no corner
    /// cutting). `heuristic(cell)` must not overestimate. The path, without the start, is
    /// written to `out` in grid coordinates.
    pub fn find(
        &mut self,
        q: &PathQuery,
        mut step: impl FnMut((i32, i32), (i32, i32)) -> Option<u32>,
        heuristic: impl Fn((i32, i32)) -> u32,
        out: &mut Vec<(i32, i32)>,
    ) -> PathEnd {
        out.clear();
        let (sx, sy) = q.start;
        let (tx, ty) = q.goal;
        let inside = |x: i32, y: i32| x >= 0 && y >= 0 && (x as u32) < q.grid_w && (y as u32) < q.grid_h;
        if !inside(sx, sy) || !inside(tx, ty) {
            return PathEnd::None;
        }
        if (sx, sy) == (tx, ty) {
            return PathEnd::Found;
        }
        let ww = self.ww.min(q.grid_w) as i32;
        let wh = self.wh.min(q.grid_h) as i32;
        let ox = (sx - (ww >> 1)).clamp(0, q.grid_w as i32 - ww);
        let oy = (sy - (wh >> 1)).clamp(0, q.grid_h as i32 - wh);
        let window = Rect::new(ox, oy, ww, wh);
        let local = |x: i32, y: i32| ((y - oy) * ww + (x - ox)) as u32;
        let global = |n: u32| ((n as i32 % ww) + ox, (n as i32 / ww) + oy);
        let goal_inside = window.contains(tx, ty);
        let goal = if goal_inside { local(tx, ty) } else { u32::MAX };
        let start = local(sx, sy);

        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamp.fill(0);
            self.closed.fill(0);
            self.generation = 1;
        }
        let gen_ = self.generation;
        self.heap.clear();
        self.g[start as usize] = 0;
        self.from[start as usize] = u32::MAX;
        self.stamp[start as usize] = gen_;
        let h0 = heuristic((sx, sy));
        self.heap.push(Reverse((u64::from(h0), start)));
        let mut best = start;
        let mut best_h = h0;
        let mut expanded = 0u32;

        while let Some(Reverse((_, node))) = self.heap.pop() {
            if self.closed[node as usize] == gen_ {
                continue;
            }
            self.closed[node as usize] = gen_;
            if node == goal {
                self.expanded += u64::from(expanded);
                self.unwind(node, start, global, out);
                return PathEnd::Found;
            }
            expanded += 1;
            if expanded > q.budget {
                break;
            }
            let (nx, ny) = global(node);
            let h = heuristic((nx, ny));
            if h < best_h {
                best_h = h;
                best = node;
            }
            let base = self.g[node as usize];
            let mut straight_ok = [false; 4];
            for (d, &(dx, dy)) in DIRS8.iter().enumerate() {
                let (cx, cy) = (nx + dx, ny + dy);
                let cost = if d < 4 {
                    let c = if window.contains(cx, cy) { step((nx, ny), (cx, cy)) } else { None };
                    straight_ok[d] = c.is_some();
                    c
                } else {
                    // DIRS8[4..] are SE, SW, NW, NE: each needs its two orthogonals.
                    let (a, b) = match d {
                        4 => (0, 1),
                        5 => (2, 1),
                        6 => (2, 3),
                        _ => (0, 3),
                    };
                    if straight_ok[a] && straight_ok[b] && window.contains(cx, cy) {
                        step((nx, ny), (cx, cy))
                    } else {
                        None
                    }
                };
                let Some(cost) = cost else { continue };
                let next = local(cx, cy);
                if self.closed[next as usize] == gen_ {
                    continue;
                }
                let g = base.saturating_add(cost);
                if g > q.max_cost {
                    continue;
                }
                if self.stamp[next as usize] == gen_ && self.g[next as usize] <= g {
                    continue;
                }
                self.g[next as usize] = g;
                self.from[next as usize] = node;
                self.stamp[next as usize] = gen_;
                self.heap.push(Reverse((u64::from(g) + u64::from(heuristic((cx, cy))), next)));
            }
        }
        self.expanded += u64::from(expanded);
        if q.partial && best != start {
            self.unwind(best, start, global, out);
            return PathEnd::Partial;
        }
        PathEnd::None
    }

    fn unwind(&self, end: u32, start: u32, global: impl Fn(u32) -> (i32, i32), out: &mut Vec<(i32, i32)>) {
        let mut n = end;
        while n != start && n != u32::MAX {
            out.push(global(n));
            n = self.from[n as usize];
        }
        out.reverse();
    }
}

/// The octile heuristic in tenths of a cell toward `goal`, for 10 / 14 step costs.
pub fn octile_to(goal: (i32, i32)) -> impl Fn((i32, i32)) -> u32 {
    move |(x, y)| octile10(goal.0 - x, goal.1 - y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn maze(rows: &[&str]) -> Grid<bool> {
        let h = rows.len() as u32;
        let w = rows[0].len() as u32;
        let cells = rows.iter().flat_map(|r| r.bytes().map(|b| b != b'#')).collect();
        Grid::from_vec(w, h, cells)
    }

    /// The reference BFS: a plain queue, no budget, 4-connected.
    fn reference_bfs(g: &Grid<bool>, start: (i32, i32)) -> Vec<u32> {
        let mut d = vec![UNREACHED; g.len() as usize];
        let mut q = std::collections::VecDeque::new();
        d[g.ix(start.0 as u32, start.1 as u32)] = 0;
        q.push_back(start);
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in DIRS4 {
                let (nx, ny) = (x + dx, y + dy);
                if g.read(nx, ny, false) && d[g.ix(nx as u32, ny as u32)] == UNREACHED {
                    d[g.ix(nx as u32, ny as u32)] = d[g.ix(x as u32, y as u32)] + 1;
                    q.push_back((nx, ny));
                }
            }
        }
        d
    }

    #[test]
    fn flood_equals_the_reference_bfs() {
        let mut rng = crate::rng::Sfc32::seeded(5, 0);
        for _ in 0..20 {
            let (w, h) = (31, 17);
            let cells: Vec<bool> = (0..w * h).map(|_| rng.below(100) < 70).collect();
            let mut g = Grid::from_vec(w, h, cells);
            g.set(0, 0, true);
            let mut r = Reach::new();
            flood(w, h, &[(0, 0)], Conn::Four, u32::MAX, |x, y| g.read(x, y, false), &mut r);
            let want = reference_bfs(&g, (0, 0));
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    assert_eq!(r.dist(x, y), want[g.ix(x as u32, y as u32)], "({x},{y})");
                }
            }
        }
    }

    #[test]
    fn flood_respects_budget_and_corners() {
        let g = maze(&[
            "..#", //
            "#..", "...",
        ]);
        let mut r = Reach::new();
        // (0,0) -> (1,1) diagonally would cut between (1,0) and (0,1): (0,1) is a wall.
        flood(3, 3, &[(0, 0)], Conn::Eight, u32::MAX, |x, y| g.read(x, y, false), &mut r);
        assert_eq!(r.dist(1, 1), 2);
        flood(3, 3, &[(0, 0)], Conn::Four, 3, |x, y| g.read(x, y, false), &mut r);
        assert!(r.exhausted);
        assert_eq!(r.count(), 3);
    }

    #[test]
    fn chamfer_within_one_step_of_the_true_distance() {
        let g = chamfer(40, 30, |x, y| (x, y) == (5, 7));
        for y in 0..30i32 {
            for x in 0..40i32 {
                let (dx, dy) = (i64::from(x - 5), i64::from(y - 7));
                let exact10 = crate::num::isqrt(((dx * dx + dy * dy) * 100) as u64);
                let c = u32::from(*g.get(x, y).unwrap());
                // 10/14 is within about 8 % over and 1 % under (14 < 10 * sqrt 2).
                assert!(c + exact10 / 50 + 1 >= exact10, "({x},{y}) {c} < {exact10}");
                assert!(c <= exact10 + exact10 / 12 + 10, "({x},{y}) {c} vs {exact10}");
            }
        }
        assert_eq!(*g.get(5, 7).unwrap(), 0);
        assert_eq!(*g.get(6, 8).unwrap(), 14);
        assert_eq!(*g.get(8, 7).unwrap(), 30);
    }

    fn step_on(g: &Grid<bool>) -> impl FnMut((i32, i32), (i32, i32)) -> Option<u32> + '_ {
        |a, b| g.read(b.0, b.1, false).then_some(if a.0 != b.0 && a.1 != b.1 { 14 } else { 10 })
    }

    #[test]
    fn astar_finds_the_short_way_and_never_cuts_corners() {
        let g = maze(&["..........", ".########.", ".#......#.", ".#.####.#.", "...#..#..."]);
        let mut a = Astar::new(64, 64);
        let mut out = Vec::new();
        let q = PathQuery {
            grid_w: 10,
            grid_h: 5,
            start: (0, 4),
            goal: (9, 4),
            max_cost: 10_000,
            budget: 10_000,
            partial: false,
        };
        assert_eq!(a.find(&q, step_on(&g), octile_to(q.goal), &mut out), PathEnd::Found);
        assert_eq!(*out.last().unwrap(), (9, 4));
        // Every step is to a neighbour and on open ground; a diagonal has both orthogonals open.
        let mut prev = q.start;
        for &c in &out {
            assert!((c.0 - prev.0).abs() <= 1 && (c.1 - prev.1).abs() <= 1);
            assert!(g.read(c.0, c.1, false));
            if c.0 != prev.0 && c.1 != prev.1 {
                assert!(g.read(c.0, prev.1, false) && g.read(prev.0, c.1, false));
            }
            prev = c;
        }
        // Over the top: up 4, across 9, down 4 with corners smoothed to two diagonals each side.
        assert!(out.len() <= 15, "{out:?}");
    }

    #[test]
    fn astar_blocked_over_cost_and_partial() {
        let g = maze(&[
            "..#..", //
            "..#..", "..#..",
        ]);
        let mut a = Astar::new(64, 64);
        let mut out = Vec::new();
        let mut q = PathQuery {
            grid_w: 5,
            grid_h: 3,
            start: (0, 1),
            goal: (4, 1),
            max_cost: 1000,
            budget: 1000,
            partial: false,
        };
        assert_eq!(a.find(&q, step_on(&g), octile_to(q.goal), &mut out), PathEnd::None);
        q.partial = true;
        assert_eq!(a.find(&q, step_on(&g), octile_to(q.goal), &mut out), PathEnd::Partial);
        assert_eq!(out.last().map(|c| c.0), Some(1));
        let open = maze(&["....."]);
        let q =
            PathQuery { grid_w: 5, grid_h: 1, start: (0, 0), goal: (4, 0), max_cost: 30, budget: 1000, partial: false };
        assert_eq!(a.find(&q, step_on(&open), octile_to(q.goal), &mut out), PathEnd::None);
    }

    #[test]
    fn astar_window_returns_partial_toward_a_far_goal() {
        let w = 300;
        let g = Grid::new(w, 1, true);
        let mut a = Astar::new(64, 64);
        let mut out = Vec::new();
        let q = PathQuery {
            grid_w: w,
            grid_h: 1,
            start: (0, 0),
            goal: (299, 0),
            max_cost: u32::MAX,
            budget: 100_000,
            partial: true,
        };
        assert_eq!(a.find(&q, step_on(&g), octile_to(q.goal), &mut out), PathEnd::Partial);
        assert_eq!(out.last(), Some(&(63, 0)));
    }
}
