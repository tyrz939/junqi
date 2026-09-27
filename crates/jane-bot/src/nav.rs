//! Feet: a path over the View's ground and the stick that walks it (`bot.ts walkTo`).
//!
//! The path is core's A* ([`jane_core::search::Astar`]) over `View::flags`, the cells feet may
//! cross (`BLOCK_MOVE` clear; a cell someone stands on is crossed, as the sim lets feet do), 10
//! straight and 14 diagonal, no corner cut. The window is [`WINDOW`] cells square about her:
//! a goal outside it gets the best partial path toward it, and the walker plans again from the
//! end of it, which is how a long road is walked. The stick points at the middle of the next
//! cell; the path is planned again every [`REPLAN`] frames, when it runs out short of the goal,
//! and when she has not moved for [`STUCK`] frames (the cell she was making for is then shunned
//! for ten seconds, in case something the flags do not show stands in it). A walker that keeps to
//! roads (the Reader, and anyone chased off the fields) pays [`OFF_ROAD`] more for a step off
//! one out of doors. [`HOPELESS`] frames without getting nearer is no way there.

use std::collections::BTreeMap;

use jane_core::angle::iatan2;
use jane_core::num::{CELL_FX, dist_sq, isqrt};
use jane_core::search::{Astar, PathEnd, PathQuery, octile_to};
use jane_core::tile::BLOCK_MOVE;
use jane_core::{Fx, Vec2, ZoneId};
use jane_sim::{InputFrame, View};

/// The A* window's side, cells.
pub const WINDOW: u32 = 320;
/// Nodes a search may expand.
pub const BUDGET: u32 = 120_000;
/// Frames between plans while walking a path.
pub const REPLAN: u32 = 120;
/// Frames without moving that count as stuck.
pub const STUCK: u32 = 24;
/// Frames without getting nearer the goal before giving up on it.
pub const HOPELESS: u32 = 60 * 20;

/// What a frame of walking came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    /// Hold this stick.
    Walk(InputFrame),
    /// Within reach of the goal.
    Arrived,
    /// No way there from here (after plans and re-plans).
    NoWay,
}

#[derive(Debug)]
pub struct Nav {
    astar: Astar,
    path: Vec<(i32, i32)>,
    at: usize,
    goal: Option<(i32, i32)>,
    zone: Option<ZoneId>,
    replan_in: u32,
    /// Where she was `STUCK` frames ago, and how many frames she has stood about there.
    anchor: Vec2,
    still: u32,
    /// Frames since she was last nearer the goal than ever before (by a cell).
    fruitless: u32,
    best_dist: i64,
    /// Cells shunned for a while (stuck against something the flags do not show: someone
    /// standing in a doorway), with the frame they may be tried again.
    shun: BTreeMap<(i32, i32), u32>,
    /// Frames walked.
    frames: u32,
    /// Plans made, for the log.
    pub plans: u64,
    /// Keep to roads out of doors: a step off one costs [`OFF_ROAD`] tenths more (the Reader;
    /// the Rusher goes straight).
    pub roads: bool,
    /// Why the last `NoWay` was said (for a debugging line).
    pub why: &'static str,
    /// The plan over blocks for a goal beyond the window, and the waypoint on it walked to now.
    pub coarse: crate::coarse::Coarse,
    waypoint: Option<(i32, i32)>,
    /// What the path in hand leads to: the goal, or a waypoint toward it.
    target: Option<(i32, i32)>,
    /// What a dungeon's tactic says a cell costs over its step, in tenths, and that she runs
    /// across it (the Factory's lit floor, where the sentries see her): `(zone, width, costs)`,
    /// for the zone it was set in. `None` everywhere else.
    pub toll: Option<(ZoneId, u32, Vec<u8>)>,
}

/// Cells (the larger of across and down) beyond which a goal is planned over blocks first.
pub const FAR: i32 = WINDOW as i32 / 2 - 24;
/// How far along the block plan a waypoint is taken, cells.
pub const WAYPOINT_REACH: i32 = 110;

/// What a step off the road costs a walker who keeps to roads, over the step's own cost.
pub const OFF_ROAD: u32 = 15;

/// A road, a street: what a walker who keeps to roads walks on.
pub fn road(t: jane_core::Tile) -> bool {
    matches!(t, jane_core::Tile::Road | jane_core::Tile::Cobble)
}

impl Default for Nav {
    fn default() -> Self {
        Self::new()
    }
}

/// Centre distance, `Fx`.
pub fn dist(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64))
}

/// Can feet stand in this cell?
pub fn walkable(v: &View<'_>, x: i32, y: i32) -> bool {
    let (w, h) = v.size();
    x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h && v.flags(x, y) & BLOCK_MOVE == 0
}

/// The nearest walkable cell to `(x, y)` within `r` rings, in the sim's ring order.
pub fn nearest_walkable(v: &View<'_>, x: i32, y: i32, r: i32) -> Option<(i32, i32)> {
    if walkable(v, x, y) {
        return Some((x, y));
    }
    for k in 1..=r {
        for d in -k..=k {
            for (cx, cy) in [(x + d, y - k), (x + d, y + k), (x - k, y + d), (x + k, y + d)] {
                if walkable(v, cx, cy) {
                    return Some((cx, cy));
                }
            }
        }
    }
    None
}

impl Nav {
    pub fn new() -> Self {
        Self {
            astar: Astar::new(WINDOW, WINDOW),
            path: Vec::with_capacity(1024),
            at: 0,
            goal: None,
            zone: None,
            replan_in: 0,
            anchor: Vec2::ZERO,
            still: 0,
            fruitless: 0,
            best_dist: i64::MAX,
            shun: BTreeMap::new(),
            frames: 0,
            plans: 0,
            roads: false,
            why: "",
            coarse: crate::coarse::Coarse::new(),
            waypoint: None,
            target: None,
            toll: None,
        }
    }

    /// A walker that keeps to roads out of doors, or not.
    pub fn keeping_to_roads(roads: bool) -> Self {
        Self { roads, ..Self::new() }
    }

    /// Forget the path (a new goal, a new zone).
    pub fn reset(&mut self) {
        self.path.clear();
        self.waypoint = None;
        self.target = None;
        self.at = 0;
        self.goal = None;
        self.replan_in = 0;
        self.still = 0;
        self.fruitless = 0;
        self.best_dist = i64::MAX;
    }

    fn plan(&mut self, v: &View<'_>, from: (i32, i32), goal: (i32, i32)) -> bool {
        self.plans += 1;
        let (w, h) = v.size();
        let q = PathQuery {
            grid_w: w,
            grid_h: h,
            start: from,
            goal,
            max_cost: 10 * 4000,
            budget: BUDGET,
            partial: true,
            window: None,
            cut_corners: false,
        };
        let shun = &self.shun;
        let now = v.frame();
        let step = |_: (i32, i32), (cx, cy): (i32, i32)| -> Option<u32> {
            if (cx, cy) != goal && (!walkable(v, cx, cy) || shun.get(&(cx, cy)).is_some_and(|&t| t > now)) {
                return None;
            }
            Some(10)
        };
        // Diagonals cost 14: core's step is asked per neighbour, so the cost is settled here.
        let roads = self.roads && !v.indoor();
        let toll = self.toll.as_ref().filter(|t| t.0 == v.zone());
        let step14 = |a: (i32, i32), b: (i32, i32)| {
            step(a, b).map(|c| {
                let c = if a.0 != b.0 && a.1 != b.1 { 14 } else { c };
                let c = c + toll.map_or(0, |t| toll_at(t, b));
                if roads && !road(v.tile(b.0, b.1)) { c + OFF_ROAD } else { c }
            })
        };
        let end = self.astar.find(&q, step14, octile_to(goal), &mut self.path);
        self.at = 0;
        self.replan_in = REPLAN;
        !matches!(end, PathEnd::None) || from == goal
    }

    /// One frame toward `to`, arriving within `near`. `sprint`: run while there is energy.
    pub fn go(&mut self, v: &View<'_>, to: Vec2, near: Fx, sprint: bool) -> Go {
        let me = v.body();
        self.frames += 1;
        if self.zone != Some(v.zone()) {
            self.zone = Some(v.zone());
            self.shun.clear();
            self.reset();
        }
        let pos = me.pos;
        let d = dist(pos, to);
        if d <= i64::from(near.0) {
            self.still = 0;
            return Go::Arrived;
        }
        let (tx, ty) = to.cell();
        let Some(goal) = nearest_walkable(v, tx, ty, 6) else {
            self.why = "nothing walkable by the goal";
            return Go::NoWay;
        };
        if self.goal != Some(goal) {
            self.reset();
            self.goal = Some(goal);
        }
        // Stuck: nothing moved her for a while. Shun the cell she was making for and plan again.
        if dist(pos, self.anchor) > i64::from(2 * 256) {
            self.anchor = pos;
            self.still = 0;
        } else {
            self.still += 1;
        }
        if self.still > STUCK {
            if let Some(&c) = self.path.get(self.at) {
                if c != goal {
                    self.shun.insert(c, v.frame() + 600);
                }
            }
            self.still = 0;
            self.replan_in = 0;
        }
        if d + i64::from(CELL_FX) < self.best_dist {
            self.best_dist = d;
            self.fruitless = 0;
        } else {
            self.fruitless += 1;
            if self.fruitless > HOPELESS {
                self.why = "no nearer for twenty seconds";
                return Go::NoWay;
            }
        }
        let own = pos.cell();
        // A goal beyond the window: toward a waypoint on the plan over blocks, the next one
        // once she is near it (or the path to it runs out).
        let cheb = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0).abs().max((a.1 - b.1).abs());
        // Not in a zone the window holds whole (a dungeon: the Factory is 220 across): the plan
        // over blocks does not see its gates, and she walked the Factory's top corridor back and
        // forth between waypoints, never out.
        let (zw, zh) = v.size();
        let whole = zw <= WINDOW && zh <= WINDOW;
        let target = if cheb(own, goal) > FAR && !whole {
            match self.waypoint {
                Some(w) if cheb(w, own) > 6 && self.at < self.path.len() => w,
                _ => {
                    let roads = self.roads && !v.indoor();
                    let w = self.coarse.waypoint(v, own, goal, roads, WAYPOINT_REACH).unwrap_or(goal);
                    if self.waypoint != Some(w) {
                        // A new waypoint on the plan is progress along it.
                        self.fruitless = 0;
                        self.best_dist = i64::MAX;
                    }
                    self.waypoint = Some(w);
                    w
                }
            }
        } else {
            self.waypoint = None;
            goal
        };
        if self.target != Some(target) {
            self.target = Some(target);
            self.path.clear();
            self.at = 0;
        }
        if self.at >= self.path.len() || self.replan_in == 0 {
            if own == goal {
                // Standing on the goal cell but not within `near` of the point: walk straight at it.
                return Go::Walk(stick(pos, to, false));
            }
            // Cells shunned for someone in the way may be what walls her in: forget them first.
            if (!self.plan(v, own, target) || self.path.is_empty()) && !self.shun.is_empty() {
                self.shun.clear();
                self.plan(v, own, target);
            }
            if self.path.is_empty() {
                if target != goal {
                    // That crossing of the block plan cannot be made from here: round it.
                    self.coarse.failed(own, target, v.frame());
                    self.waypoint = None;
                    self.target = None;
                    return Go::Walk(InputFrame::IDLE);
                }
                self.why = "no path";
                return Go::NoWay;
            }
        }
        self.replan_in = self.replan_in.saturating_sub(1);
        // The next cell whose middle she has not reached.
        while let Some(&(cx, cy)) = self.path.get(self.at) {
            let c = Vec2::centre(cx, cy);
            if dist(pos, c) < 384 {
                self.at += 1;
                // A step along a path that ends at the goal is progress, however far round it
                // goes (a gallery that doubles back is not hopeless).
                if self.path.last() == Some(&target) {
                    self.fruitless = 0;
                }
                continue;
            }
            let far = d > i64::from(3 * CELL_FX);
            // Across a tolled cell (in a sentry's light) she runs.
            let tolled = self.toll.as_ref().is_some_and(|t| t.0 == v.zone() && toll_at(t, own) > 0);
            return Go::Walk(stick(pos, c, (sprint && far) || tolled));
        }
        self.replan_in = 0;
        Go::Walk(stick(pos, to, false))
    }
}

/// A toll's cost for a cell (0 off its grid).
pub fn toll_at(t: &(ZoneId, u32, Vec<u8>), (x, y): (i32, i32)) -> u32 {
    if x < 0 || y < 0 || x as u32 >= t.1 {
        return 0;
    }
    t.2.get((y as u32 * t.1 + x as u32) as usize).map_or(0, |&c| u32::from(c))
}

/// Full tilt from `from` toward `to`.
pub fn stick(from: Vec2, to: Vec2, sprint: bool) -> InputFrame {
    let dir = iatan2(to.y.0 - from.y.0, to.x.0 - from.x.0);
    InputFrame { sprint, ..InputFrame::walk(dir) }
}
