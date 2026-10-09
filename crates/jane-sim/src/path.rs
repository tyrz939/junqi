//! Paths for feet (`sim/path.ts`) on core's one A* (`jane_core::search::Astar`).
//!
//! **Windowed:** the scratch is a fixed 256 x 256 window centred on the start (about 3 MB, one
//! per sim, shared by every zone), whether the zone is a kitchen or the county. A goal outside
//! the window cannot be proven unreachable, so the search returns the best partial path toward
//! it and the walker re-plans on arrival: that is how something walks a long road.
//!
//! Costs are tenths of a cell, 10 straight and 14 diagonal; a diagonal needs both orthogonal
//! cells open *terrain* (no corner cutting; an occupied orthogonal does not forbid it, as in
//! the TS). Occupancy shapes the path: a cell someone stands on is not entered, except the goal,
//! which is always enterable (it is usually a unit's feet; callers stop short by range). Every
//! search has a node budget and a max cost; a zone may run [`PATHS_PER_TICK`] per tick.
//!
//! **Across height** (MAP.md §3.5): ledges are one-way jumps; a span's deck is a second layer of
//! nodes (`Astar::find_layered`), walked along its axis only and stepped onto and off at its
//! ends. A search toward a goal inside its window first asks the zone's level regions
//! (`regions.rs`) whether the goal can be reached at all (if not, it is not run: it could only
//! have failed), and then steers by them: its heuristic is the cheapest way out of the cell's
//! region toward the goal by the portals' lower bounds, admissible and consistent, so the path's
//! cost is the plain search's while the cliff's foot is not flooded.

use alloc::vec::Vec;

use jane_core::blueprint::{SPANS_MAX, SpanIx};
use jane_core::search::{Astar, DECK_NODES, Decks, PathEnd, PathQuery, octile_to};
use jane_core::tile::{BLOCK_MOVE, F_OCC};
use jane_core::{Rect, num::octile10};

use crate::grid::{Meets, ZoneGrid};
use crate::regions::{FAR, Route};
use crate::tuning::LEDGE_PATH_EXTRA;

pub const PATH_WINDOW: u32 = 256;
/// A portal's crossing: the cell made for, the cell stepped (or hopped) to, and its cost.
type Crossing = ((i32, i32), (i32, i32), u32);

/// The most legs (regions crossed, plus one) a chained search walks before it searches whole.
const CHAIN_LEGS: usize = 16;
pub const PATH_BUDGET: u32 = 6000;
pub const PATHS_PER_TICK: u8 = 4;
pub const REPATH_TICKS: u32 = 20;
/// Doublings of `REPATH_TICKS` a walker waits after searches toward one goal keep failing, and
/// before a partial path toward a fixed goal is planned again unwalked: 20 << 5, about ten
/// seconds.
pub const REPATH_BACKOFF_MAX: u16 = 5;
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;

/// Cost units are tenths of a cell; this is a distance in cells.
pub const fn cost_of_cells(cells: u32) -> u32 {
    cells * STRAIGHT
}

/// Counters for the debug overlay. Derived; never saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathStats {
    pub searches: u64,
    pub expanded: u64,
    pub failed: u64,
    pub partial: u64,
    /// Searches not run: their goal's region cannot be reached from their start's.
    pub unreachable: u64,
    /// Searches steered by the level regions.
    pub steered: u64,
    /// Of those, the ones walked leg by leg ([`PathScratch::chained`]).
    pub chained: u64,
}

/// One ask.
#[derive(Clone, Copy, Debug)]
pub struct PathAsk {
    pub start: (i32, i32),
    pub goal: (i32, i32),
    /// Tenths of a cell.
    pub max_cost: u32,
    pub budget: u32,
    /// Across height (MAP.md §3.5): the walker may hop ledges, landing no further than this many
    /// cells from this home cell (its leash). `None`: a ledge is a wall, as in a flat zone.
    pub ledges: Option<((i32, i32), i32)>,
    /// A perch row (`holds: level`): it never sets foot off ground of this level.
    pub level: Option<u8>,
    /// The walker stands on this span's deck (MAP.md §2.5).
    pub start_deck: Option<SpanIx>,
    /// The goal stands on the deck over its cell.
    pub goal_deck: bool,
}

impl PathAsk {
    pub const fn new(start: (i32, i32), goal: (i32, i32), max_cost: u32) -> Self {
        Self {
            start,
            goal,
            max_cost,
            budget: PATH_BUDGET,
            ledges: None,
            level: None,
            start_deck: None,
            goal_deck: false,
        }
    }
}

/// The one A* scratch of a sim (`Sim.scratch`, ARCHITECTURE.md §3.3).
#[derive(Debug)]
pub struct PathScratch {
    astar: Astar,
    /// The last path found, without the start, in cells.
    pub out: Vec<(i32, i32)>,
    pub stats: PathStats,
    /// The level regions' view of the last search (`regions.rs`).
    route: Route,
    /// One leg of a chained search ([`PathScratch::chained`]).
    leg: Vec<(i32, i32)>,
}

impl Default for PathScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl PathScratch {
    pub fn new() -> Self {
        Self {
            astar: Astar::new(PATH_WINDOW, PATH_WINDOW),
            out: Vec::with_capacity(1024),
            stats: PathStats::default(),
            route: Route::default(),
            leg: Vec::with_capacity(1024),
        }
    }

    /// For each cell of the last path found (`out`): whether it is stood on a span's deck. Empty
    /// when the path has no deck.
    pub fn out_on_deck(&self) -> &[bool] {
        self.astar.path_on_deck()
    }

    /// Find a path; it is in `self.out` (empty when the start is the goal). `None` when there is
    /// none: blocked, over cost, over budget, or a goal that is solid or off the grid.
    pub fn find(&mut self, grid: &ZoneGrid, ask: PathAsk) -> Option<PathEnd> {
        self.find_shunning(grid, ask, false, |_, _| false)
    }

    /// The same, never entering a cell `shun` names (light, for something that shuns it); a goal
    /// it cannot reach returns the path to the nearest cell it can, which may be empty: whatever
    /// shuns the light walks to the edge of it and waits there.
    pub fn find_shunning(
        &mut self,
        grid: &ZoneGrid,
        ask: PathAsk,
        shunning: bool,
        shun: impl Fn(i32, i32) -> bool,
    ) -> Option<PathEnd> {
        self.find_weighted(grid, ask, shunning, |x, y| if shunning && shun(x, y) { None } else { Some(0) })
    }

    /// The general search: `extra` says what entering a cell costs beyond its step (tenths of a
    /// cell), or `None` to never enter it (the goal included). With `partial`, a goal it cannot
    /// reach returns the path to the nearest cell it can (possibly empty), as for a shunner; a
    /// click-walk asks this way through the cells she has seen, preferring roads and light
    /// (`walk.rs`). Extra costs are never negative, so the octile estimate stays admissible.
    pub fn find_weighted(
        &mut self,
        grid: &ZoneGrid,
        ask: PathAsk,
        partial: bool,
        extra: impl Fn(i32, i32) -> Option<u32>,
    ) -> Option<PathEnd> {
        let shunning = partial;
        self.stats.searches += 1;
        self.out.clear();
        let (sx, sy) = ask.start;
        let (tx, ty) = ask.goal;
        if !grid.inside(sx, sy) || !grid.inside(tx, ty) {
            return self.fail();
        }
        if ask.start == ask.goal {
            return Some(PathEnd::Found);
        }
        // A goal in a prop's cell that feet can stand in part of (the notch behind a crate, the
        // ground beside a trunk: `PropDef::solid_parts`) is walked to; one solid whole is not.
        if grid.flags_at(tx, ty) & BLOCK_MOVE != 0 && !matches!(grid.feet_meet(tx, ty), Meets::Part(_)) {
            return self.fail();
        }
        // Mirror `sim/path.ts`'s window: the goal is inside it or the search is partial.
        let ww = PATH_WINDOW.min(grid.w()) as i32;
        let wh = PATH_WINDOW.min(grid.h()) as i32;
        let ox = (sx - (ww >> 1)).clamp(0, grid.w() as i32 - ww);
        let oy = (sy - (wh >> 1)).clamp(0, grid.h() as i32 - wh);
        let goal_inside = tx >= ox && ty >= oy && tx < ox + ww && ty < oy + wh;
        let q = PathQuery {
            grid_w: grid.w(),
            grid_h: grid.h(),
            start: ask.start,
            goal: ask.goal,
            max_cost: ask.max_cost,
            budget: ask.budget,
            partial: !goal_inside || shunning,
            window: None,
            // The corner rule is the step's own (terrain only), so core offers every diagonal.
            cut_corners: true,
        };
        let before = self.astar.expanded;
        // Height's rules, asked only in a zone with levels (MAP.md §3.5).
        let levels = grid.has_levels();
        let keep = ask.level.filter(|_| levels);
        let ledges = ask.ledges.filter(|_| levels && keep.is_none());
        let window = Rect::new(ox, oy, ww, wh);
        // The level regions: a goal they cannot reach is not searched for (the search could only
        // fail: it is not partial), and one they can is steered to by them.
        let mut steer = None;
        if levels && !q.partial && ask.start_deck.is_none() && !ask.goal_deck {
            if let Some(r) = grid.regions() {
                if let Some(l) = keep {
                    // A perch row's goal off its level is never entered.
                    if grid.level_at(tx, ty) != l {
                        self.stats.unreachable += 1;
                        return self.fail();
                    }
                } else if let (Some(rs), Some(rg)) = (r.region_of_feet(grid, sx, sy), r.region_of_feet(grid, tx, ty)) {
                    self.route.prepare(r, grid, (ask.start, ask.goal), rg, ledges);
                    if self.route.bound(r, rs, ask.start) >= FAR {
                        self.stats.unreachable += 1;
                        return self.fail();
                    }
                    steer = Some(r);
                    self.stats.steered += 1;
                }
            }
        }
        // The decks in the window (MAP.md §2.5): each whole span over it numbers its cells from
        // its own first slot, in span order.
        let mut first_slot = [u16::MAX; SPANS_MAX];
        let mut slots = 0usize;
        if grid.has_spans() {
            for (i, sp) in grid.spans().iter().enumerate() {
                if grid.span_whole(i as SpanIx)
                    && sp.rect.overlaps(window)
                    && slots + (sp.rect.area() as usize) <= DECK_NODES
                {
                    first_slot[i] = slots as u16;
                    slots += sp.rect.area() as usize;
                }
            }
        }
        let decked = slots > 0;
        if decked {
            steer = None;
        }
        let step = |(nx, ny): (i32, i32), (cx, cy): (i32, i32)| -> Option<u32> {
            if (cx, cy) != (tx, ty) {
                let f = grid.flags_at(cx, cy);
                if f & BLOCK_MOVE != 0 || f & F_OCC != 0 {
                    return None;
                }
            }
            if keep.is_some_and(|l| grid.level_at(cx, cy) != l) {
                return None;
            }
            let more = extra(cx, cy)?;
            if cx != nx && cy != ny {
                if grid.flags_at(cx, ny) & BLOCK_MOVE != 0 || grid.flags_at(nx, cy) & BLOCK_MOVE != 0 {
                    return None;
                }
                return Some(DIAGONAL + more);
            }
            Some(STRAIGHT + more)
        };
        // A ledge entered the way it is hopped is a jump to its landing, one way (MAP.md §3.5).
        let jump = |(nx, ny): (i32, i32), (cx, cy): (i32, i32)| -> Option<((i32, i32), u32)> {
            let ((hx, hy), reach) = ledges?;
            let ((lx, ly), faces) = crate::height::ledge_landing(grid, (cx, cy), (cx - nx, cy - ny))?;
            let (ex, ey) = (lx - hx, ly - hy);
            if ex * ex + ey * ey > reach * reach || (lx, ly) != (tx, ty) && grid.flags_at(lx, ly) & F_OCC != 0 {
                return None;
            }
            // A hop never lands on a span's end rows (they are solid) nor starts on a deck.
            Some(((lx, ly), (faces as u32 + 1) * STRAIGHT + LEDGE_PATH_EXTRA))
        };
        // Over the cost is never offered (the heuristics here are admissible and consistent);
        // a steered search breaks its ties toward the deeper node.
        self.astar.prune = true;
        self.astar.deep_ties = steer.is_some();
        // Steered: leg by leg, region by region, through the portals the regions name; if a leg
        // cannot be walked now (a portal held, a window too small), the whole search as one.
        if let Some(r) = steer
            && self.chained(grid, r, &q, &step, &jump)
        {
            self.stats.expanded += self.astar.expanded - before;
            self.stats.chained += 1;
            return Some(PathEnd::Found);
        }
        let goal = ask.goal;
        let route = &self.route;
        let heuristic = |c: (i32, i32)| -> u32 {
            match steer {
                Some(r) => match r.region_of_feet(grid, c.0, c.1) {
                    Some(rc) => route.bound(r, rc, c),
                    None => octile10(goal.0 - c.0, goal.1 - c.1),
                },
                None => octile10(goal.0 - c.0, goal.1 - c.1),
            }
        };
        let end = if decked {
            let slot = |x: i32, y: i32| -> Option<u16> {
                let i = grid.deck_at(x, y)?;
                let base = first_slot[usize::from(i)];
                if base == u16::MAX {
                    return None;
                }
                let r = grid.spans()[usize::from(i)].rect;
                Some(base + ((y - r.y) * r.w + (x - r.x)) as u16)
            };
            let deck_step = |from: (i32, i32), from_deck: bool, to: (i32, i32), to_deck: bool| -> Option<u32> {
                // A perch row keeps to its ground.
                if keep.is_some() {
                    return None;
                }
                let along = |i: SpanIx| {
                    let sp = &grid.spans()[usize::from(i)];
                    if sp.along_x { from.1 == to.1 } else { from.0 == to.0 }
                };
                match (from_deck, to_deck) {
                    (true, true) => {
                        let i = grid.deck_at(from.0, from.1)?;
                        (grid.deck_at(to.0, to.1) == Some(i) && along(i)).then_some(())?;
                    }
                    (false, true) => {
                        let (i, _) = grid.deck_end_at(from.0, from.1)?;
                        (grid.deck_at(to.0, to.1) == Some(i) && along(i)).then_some(())?;
                    }
                    (true, false) => {
                        let i = grid.deck_at(from.0, from.1)?;
                        (grid.deck_end_at(to.0, to.1).is_some_and(|(j, _)| j == i) && along(i)).then_some(())?;
                        if to != (tx, ty) && grid.flags_at(to.0, to.1) & (BLOCK_MOVE | F_OCC) != 0 {
                            return None;
                        }
                    }
                    (false, false) => return None,
                }
                if to_deck && to != (tx, ty) && grid.deck_occupants(to.0, to.1) > 0 {
                    return None;
                }
                Some(STRAIGHT + extra(to.0, to.1)?)
            };
            let decks = Decks { slot, step: deck_step, start: ask.start_deck.is_some(), goal: ask.goal_deck };
            self.astar.find_layered(&q, step, jump, Some(decks), heuristic, &mut self.out)
        } else {
            self.astar.find_with_jumps(&q, step, jump, heuristic, &mut self.out)
        };
        self.stats.expanded += self.astar.expanded - before;
        match end {
            PathEnd::Found => Some(PathEnd::Found),
            PathEnd::Partial => {
                self.stats.partial += 1;
                Some(PathEnd::Partial)
            }
            // Already as near as it can get without stepping into the light: stand here.
            PathEnd::None if shunning => Some(PathEnd::Partial),
            PathEnd::None => self.fail(),
        }
    }

    /// A steered search in legs (MAP R2): from the start, the best portal out of its region by
    /// the regions' bounds; a search to the nearest cell of that portal that is open, the step
    /// (or hop) across it, and on from the far side, until the goal's region, then a search to the
    /// goal. Each leg is a search within one region toward a near target, its heuristic all but
    /// exact, so it opens about its own length; a chase across height costs what one on the flat
    /// does. The path is in `self.out` when it returns true; false (and `out` to be written again)
    /// when a leg cannot be found now, or the legs cost more than the ask allows.
    fn chained(
        &mut self,
        grid: &ZoneGrid,
        r: &crate::regions::Regions,
        q: &PathQuery,
        step: &impl Fn((i32, i32), (i32, i32)) -> Option<u32>,
        jump: &impl Fn((i32, i32), (i32, i32)) -> Option<((i32, i32), u32)>,
    ) -> bool {
        let Some(goal_region) = r.region_of_feet(grid, q.goal.0, q.goal.1) else { return false };
        let Some(mut region) = r.region_of_feet(grid, q.start.0, q.start.1) else { return false };
        self.out.clear();
        let mut at = q.start;
        let (mut cost, mut budget) = (0u32, q.budget);
        for _ in 0..CHAIN_LEGS {
            let last = region == goal_region;
            // Where this leg goes, and the cell across the portal it then takes.
            let (to, across) = if last {
                (q.goal, None)
            } else {
                let Some(e) = self.route.best_exit(r, region, at) else { return false };
                let Some((t, u, c)) = Self::crossing(grid, r, &e, at, step, jump) else { return false };
                (t, Some((u, c, e.to)))
            };
            if at != to {
                let lq = PathQuery { start: at, goal: to, max_cost: q.max_cost - cost, budget, partial: false, ..*q };
                let before = self.astar.expanded;
                let end = self.astar.find_with_jumps(&lq, step, jump, octile_to(to), &mut self.leg);
                budget = budget.saturating_sub((self.astar.expanded - before) as u32);
                if end != PathEnd::Found {
                    return false;
                }
                let mut prev = at;
                for &c in &self.leg {
                    let d = (c.0 - prev.0).abs().max((c.1 - prev.1).abs()) as u32;
                    cost += match d {
                        1 if c.0 != prev.0 && c.1 != prev.1 => DIAGONAL,
                        1 => STRAIGHT,
                        _ => d * STRAIGHT + LEDGE_PATH_EXTRA,
                    };
                    prev = c;
                }
                self.out.extend_from_slice(&self.leg);
            }
            let Some((u, c, next)) = across else { return cost <= q.max_cost };
            cost += c;
            if cost > q.max_cost || budget == 0 {
                return false;
            }
            self.out.push(u);
            at = u;
            region = next;
        }
        false
    }

    /// The cell of portal `e` a walker at `from` makes for (the nearest open one, first in the
    /// portal's order), the cell across it it steps (or hops) to, and what that costs; `None` if
    /// none can be crossed now. Asked of the search's own steps and jumps, so it crosses only as
    /// a search would.
    fn crossing(
        grid: &ZoneGrid,
        r: &crate::regions::Regions,
        e: &crate::regions::Edge,
        from: (i32, i32),
        step: &impl Fn((i32, i32), (i32, i32)) -> Option<u32>,
        jump: &impl Fn((i32, i32), (i32, i32)) -> Option<((i32, i32), u32)>,
    ) -> Option<Crossing> {
        let mut best: Option<(u32, Crossing)> = None;
        for t in e.a.cells() {
            // A portal's rects bound its cells: only those of its own regions count.
            if t != from && grid.flags_at(t.0, t.1) & (BLOCK_MOVE | F_OCC) != 0
                || r.region_of_feet(grid, t.0, t.1) != Some(e.from)
            {
                continue;
            }
            let near = octile10(t.0 - from.0, t.1 - from.1);
            if best.is_some_and(|(b, _)| near >= b) {
                continue;
            }
            let over = match e.kind {
                crate::regions::Kind::Join => e
                    .b
                    .cells()
                    .filter(|u| (u.0 - t.0).abs() <= 1 && (u.1 - t.1).abs() <= 1 && r.region(u.0, u.1) == Some(e.to))
                    .find_map(|u| step(t, u).map(|c| (u, c))),
                crate::regions::Kind::Ledge => {
                    let dir = grid.tile_at(t.0, t.1).ledge_dir().or_else(|| {
                        [(0, 1), (1, 0), (0, -1), (-1, 0)]
                            .into_iter()
                            .find(|d| grid.tile_at(t.0 + d.0, t.1 + d.1).ledge_dir() == Some(*d))
                    })?;
                    jump(t, (t.0 + dir.0, t.1 + dir.1))
                }
                crate::regions::Kind::Span => None,
            };
            if let Some((u, c)) = over {
                best = Some((near, (t, u, c)));
            }
        }
        best.map(|(_, c)| c)
    }

    fn fail(&mut self) -> Option<PathEnd> {
        self.stats.failed += 1;
        None
    }
}

#[cfg(test)]
mod tests {
    use jane_core::grid::Grid;
    use jane_core::{Rect, Tile};

    use super::*;

    fn grid(w: u32, h: u32) -> ZoneGrid {
        ZoneGrid::new(Grid::new(w, h, Tile::Floor))
    }

    fn fill(g: &mut ZoneGrid, r: Rect, t: Tile) {
        for (x, y) in r.cells() {
            g.set_tile(x, y, t);
        }
    }

    /// engine.test.ts `walled()`: a 64 x 64 room with a wall across x = 30 from y = 0 to 49.
    fn walled() -> ZoneGrid {
        let mut g = grid(64, 64);
        fill(&mut g, Rect::new(30, 0, 1, 50), Tile::Wall);
        g
    }

    fn assert_walkable(g: &ZoneGrid, start: (i32, i32), path: &[(i32, i32)]) {
        let mut prev = start;
        for &c in path {
            assert!((c.0 - prev.0).abs() <= 1 && (c.1 - prev.1).abs() <= 1, "{prev:?} -> {c:?}");
            assert!(!g.solid(c.0, c.1), "{c:?} is solid");
            if c.0 != prev.0 && c.1 != prev.1 {
                assert!(!g.solid(c.0, prev.1) && !g.solid(prev.0, c.1), "cut a corner at {c:?}");
            }
            prev = c;
        }
    }

    #[test]
    fn routes_round_a_wall_and_never_cuts_a_corner() {
        let g = walled();
        let mut p = PathScratch::new();
        assert_eq!(p.find(&g, PathAsk::new((10, 10), (50, 10), cost_of_cells(400))), Some(PathEnd::Found));
        assert_eq!(p.out.last(), Some(&(50, 10)));
        assert!(p.out.iter().any(|c| c.1 >= 50));
        assert_walkable(&g, (10, 10), &p.out);
    }

    #[test]
    fn reaches_a_goal_cell_that_another_unit_is_standing_on() {
        // The 2026 Phaser build returned null here, so nothing could chase a standing player.
        let mut g = walled();
        g.occupy(20, 10);
        let mut p = PathScratch::new();
        assert!(p.find(&g, PathAsk::new((10, 10), (20, 10), cost_of_cells(100))).is_some());
    }

    #[test]
    fn paths_around_other_units_cells() {
        let mut g = grid(16, 5);
        fill(&mut g, Rect::new(0, 0, 16, 1), Tile::Wall);
        fill(&mut g, Rect::new(0, 4, 16, 1), Tile::Wall);
        g.occupy(8, 2);
        let mut p = PathScratch::new();
        assert!(p.find(&g, PathAsk::new((2, 2), (14, 2), cost_of_cells(100))).is_some());
        assert!(!p.out.contains(&(8, 2)));
    }

    #[test]
    fn gives_up_inside_its_budget_when_the_goal_is_sealed_off() {
        let mut g = grid(400, 400);
        fill(&mut g, Rect::new(100, 100, 9, 1), Tile::Wall);
        fill(&mut g, Rect::new(100, 108, 9, 1), Tile::Wall);
        fill(&mut g, Rect::new(100, 100, 1, 9), Tile::Wall);
        fill(&mut g, Rect::new(108, 100, 1, 9), Tile::Wall);
        let mut p = PathScratch::new();
        assert_eq!(p.find(&g, PathAsk::new((10, 10), (104, 104), cost_of_cells(2000))), None);
        assert!(p.stats.expanded <= u64::from(PATH_BUDGET) + 1, "{}", p.stats.expanded);
    }

    #[test]
    fn paths_anywhere_in_a_ten_minute_county_in_constant_memory() {
        // PLAN.md: 3600 x 2000 cells. The first A* sized its scratch to the grid: 115 MB here.
        let mut g = grid(3600, 2000);
        fill(&mut g, Rect::new(3400, 1700, 1, 200), Tile::Wall);
        let mut p = PathScratch::new();
        assert_eq!(p.find(&g, PathAsk::new((3390, 1900), (3410, 1900), cost_of_cells(600))), Some(PathEnd::Found));
        assert_eq!(p.out.last(), Some(&(3410, 1900)));
        assert_walkable(&g, (3390, 1900), &p.out);
        assert!(p.out.iter().any(|c| c.1 < 1700 || c.1 >= 1900));
    }

    #[test]
    fn returns_a_partial_path_toward_a_goal_beyond_the_window() {
        let g = grid(2000, 400);
        let mut p = PathScratch::new();
        let mut ask = PathAsk::new((100, 200), (1500, 200), cost_of_cells(5000));
        ask.budget = 60_000;
        assert_eq!(p.find(&g, ask), Some(PathEnd::Partial));
        let end = *p.out.last().unwrap();
        assert!(end.0 > 180, "it got meaningfully closer: {end:?}");
        assert!(end.0 < 1500, "and did not pretend to arrive");
        assert_eq!(p.stats.partial, 1);
    }

    #[test]
    fn respects_max_path_length() {
        let g = walled();
        let mut p = PathScratch::new();
        assert_eq!(p.find(&g, PathAsk::new((10, 10), (50, 10), cost_of_cells(45))), None);
    }

    #[test]
    fn a_diagonal_past_someone_is_allowed_a_diagonal_past_a_wall_is_not() {
        let mut g = grid(3, 3);
        g.occupy(1, 0);
        let mut p = PathScratch::new();
        assert!(p.find(&g, PathAsk::new((0, 0), (1, 1), 100)).is_some());
        assert_eq!(p.out, [(1, 1)]);
        g.set_tile(0, 1, Tile::Wall);
        assert!(p.find(&g, PathAsk::new((0, 0), (1, 1), 100)).is_none());
    }

    #[test]
    fn a_shunner_stops_at_the_edge() {
        let g = grid(20, 1);
        let mut p = PathScratch::new();
        let end = p.find_shunning(&g, PathAsk::new((0, 0), (19, 0), 1000), true, |x, _| x >= 10);
        assert_eq!(end, Some(PathEnd::Partial));
        assert_eq!(p.out.last(), Some(&(9, 0)));
    }
}
