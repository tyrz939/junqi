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

use alloc::vec::Vec;

use jane_core::search::{Astar, PathEnd, PathQuery, octile_to};
use jane_core::tile::{BLOCK_MOVE, F_OCC};

use crate::grid::{Meets, ZoneGrid};
use crate::tuning::LEDGE_PATH_EXTRA;

pub const PATH_WINDOW: u32 = 256;
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
}

impl PathAsk {
    pub const fn new(start: (i32, i32), goal: (i32, i32), max_cost: u32) -> Self {
        Self { start, goal, max_cost, budget: PATH_BUDGET, ledges: None, level: None }
    }
}

/// The one A* scratch of a sim (`Sim.scratch`, ARCHITECTURE.md §3.3).
#[derive(Debug)]
pub struct PathScratch {
    astar: Astar,
    /// The last path found, without the start, in cells.
    pub out: Vec<(i32, i32)>,
    pub stats: PathStats,
}

impl Default for PathScratch {
    fn default() -> Self {
        Self::new()
    }
}

impl PathScratch {
    pub fn new() -> Self {
        Self { astar: Astar::new(PATH_WINDOW, PATH_WINDOW), out: Vec::with_capacity(1024), stats: PathStats::default() }
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
        mut shun: impl FnMut(i32, i32) -> bool,
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
        mut extra: impl FnMut(i32, i32) -> Option<u32>,
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
            Some(((lx, ly), (faces as u32 + 1) * STRAIGHT + LEDGE_PATH_EXTRA))
        };
        let end = self.astar.find_with_jumps(&q, step, jump, octile_to(ask.goal), &mut self.out);
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
