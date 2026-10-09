//! Line of sight on the cell grid (`sim/los.ts`): a supercover walk that visits every cell the
//! segment touches, so a bolt cannot slip between two diagonal walls. Sight and projectiles
//! share it.
//!
//! The TS compared float crossing times; here the times are compared exactly by
//! cross-multiplying in `i64` (`dist_x / |dx| < dist_y / |dy|` as `dist_x * |dy| < dist_y *
//! |dx|`), and a tie steps along y as the TS's `<` did.
//!
//! **Across height** (MAP.md §3.2): in a zone with levels the line runs from one end's eye
//! (its ground's level in rungs plus [`EYE_RUNGS`]) to the other's, linearly in the walk's step
//! count, and a visited cell blocks when its ground plus its tile's `top` (or a blocking prop's
//! [`PROP_TOP`]) stands over the line there. One integer test, the same from either end. Where the
//! ground is one level under the whole walk ([`ZoneGrid::flat_between`]) it is the flags test,
//! which the tiles' `top` table makes equal to it (`a_tile_blocks_sight_exactly_when_it_stands_over_the_eye`),
//! and a zone without levels never asks.

use jane_core::num::CELL_FX;
use jane_core::tile::{BLOCK_SIGHT, EYE_RUNGS, F_BLOCK_LOS, F_PROP_LOS, F_PROP_SOLID, PROP_TOP, RUNGS_PER_LEVEL};
use jane_core::{CellIx, Vec2};

use crate::grid::ZoneGrid;

/// A rung in the units the walk's heights are compared in: fine enough for a bolt's height
/// part way along its line (`flight.rs`).
pub const RUNG: i64 = 256;

/// Nothing that blocks sight lies between the two points.
pub fn line_of_sight(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    first_blocked(g, a, b, BLOCK_SIGHT).is_none()
}

/// Walls only: props (pillars, gates) are ignored. The snake's anti-cheese reset asks this.
pub fn line_of_sight_walls(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    first_blocked(g, a, b, F_BLOCK_LOS).is_none()
}

/// The supercover walk from `a` to `b`: `test(x, y, k, n)` for each cell after the start, `k`
/// its step (1 to `n`) of the walk's `n`, until it says stop; that cell, or `None`.
#[inline]
fn walk(a: Vec2, b: Vec2, mut test: impl FnMut(i32, i32, i64, i64) -> bool) -> Option<(i32, i32)> {
    let cell = i64::from(CELL_FX);
    let (x0, y0) = (i64::from(a.x.0), i64::from(a.y.0));
    let (x1, y1) = (i64::from(b.x.0), i64::from(b.y.0));
    let mut cx = a.x.cell();
    let mut cy = a.y.cell();
    let (tx, ty) = (b.x.cell(), b.y.cell());
    let (dx, dy) = (x1 - x0, y1 - y0);
    let step_x = if dx > 0 { 1 } else { -1 };
    let step_y = if dy > 0 { 1 } else { -1 };
    let (adx, ady) = (dx.abs(), dy.abs());
    // Distance to the next vertical / horizontal border, grown by a cell per step taken.
    let mut nx = if dx > 0 { (i64::from(cx) + 1) * cell - x0 } else { x0 - i64::from(cx) * cell };
    let mut ny = if dy > 0 { (i64::from(cy) + 1) * cell - y0 } else { y0 - i64::from(cy) * cell };
    // Manhattan cell distance bounds the walk.
    let n = (tx - cx).abs() + (ty - cy).abs();
    let mut steps = n;
    while steps > 0 {
        steps -= 1;
        // tMaxX < tMaxY, with an infinite time on an axis the segment does not move along.
        let x_first = if dx == 0 {
            false
        } else if dy == 0 {
            true
        } else {
            nx * ady < ny * adx
        };
        if x_first {
            nx += cell;
            cx += step_x;
        } else {
            ny += cell;
            cy += step_y;
        }
        if test(cx, cy, i64::from(n - steps), i64::from(n)) {
            return Some((cx, cy));
        }
    }
    None
}

/// The supercover walk's cells after `a`, each to `test` until it says stop: that cell, or `None`.
pub fn walk_cells(a: Vec2, b: Vec2, mut test: impl FnMut(i32, i32) -> bool) -> Option<(i32, i32)> {
    walk(a, b, |x, y, _, _| test(x, y))
}

/// The first cell on the segment whose flags meet `mask`, as `(x, y)`, or `None`. The start
/// cell is never tested: a unit hugging a wall can still see out. A cell outside the grid
/// blocks everything. Across height, the line runs eye to eye ([`first_blocked_cell_levels`]).
pub fn first_blocked_cell(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<(i32, i32)> {
    if !g.flat_between(a.cell(), b.cell()) {
        return first_blocked_cell_levels(g, a, b, mask);
    }
    walk(a, b, |cx, cy, _, _| g.flags_at(cx, cy) & mask != 0)
}

/// [`first_blocked_cell`] as a cell index, for a cell inside the grid (`None` also when the
/// blocking cell is outside it: callers that need that distinction ask for the cell).
pub fn first_blocked(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<CellIx> {
    first_blocked_cell(g, a, b, mask).map(|(x, y)| if g.inside(x, y) { g.ix(x, y) } else { CellIx(u32::MAX) })
}

// --- across height ---------------------------------------------------------------------------------

/// The ground's height at `(x, y)` in rungs: its level's.
#[inline]
pub fn ground_rungs(g: &ZoneGrid, x: i32, y: i32) -> i64 {
    i64::from(g.level_at(x, y)) * i64::from(RUNGS_PER_LEVEL)
}

/// The eye of a body (or a point on the ground) at `p`, in [`RUNG`]s.
#[inline]
pub fn eye(g: &ZoneGrid, p: Vec2) -> i64 {
    let (x, y) = p.cell();
    (ground_rungs(g, x, y) + i64::from(EYE_RUNGS)) * RUNG
}

/// How high what is on cell `(x, y)` stands, in rungs, for a line asking `mask`: its ground,
/// plus its tile's top or a blocking prop's, whichever is taller. `None` outside the grid.
#[inline]
fn stands(g: &ZoneGrid, x: i32, y: i32, mask: u8) -> Option<i64> {
    if !g.inside(x, y) {
        return None;
    }
    let mut top = i64::from(g.tile_at(x, y).top());
    if g.flags_at(x, y) & mask & (F_PROP_LOS | F_PROP_SOLID) != 0 {
        top = top.max(i64::from(PROP_TOP));
    }
    Some(ground_rungs(g, x, y) + top)
}

/// The sight across height (MAP.md §3.2) whatever the ground: eye to eye. What
/// [`first_blocked_cell`] asks where the level changes; on flat ground it gives the flags test's
/// answer (but for a [`Tile::Cliff`](jane_core::Tile::Cliff), whose height is its level).
pub fn first_blocked_cell_levels(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<(i32, i32)> {
    first_blocked_between(g, a, eye(g, a), b, eye(g, b), mask)
}

/// The line from `a` at `ha` to `b` at `hb` (in [`RUNG`]s): the first cell that stands over it.
pub fn first_blocked_between(g: &ZoneGrid, a: Vec2, ha: i64, b: Vec2, hb: i64, mask: u8) -> Option<(i32, i32)> {
    walk(a, b, |x, y, k, n| match stands(g, x, y, mask) {
        None => true,
        // `top > ha + (hb - ha) * k / n`, multiplied through by `n`.
        Some(top) => top * RUNG * n > ha * n + (hb - ha) * k,
    })
}

/// A free-aimed bolt's move from `a` to `b` (MAP.md §3.3): it flies at its ground's height plus
/// the eye, falling with the ground under it, and stops at the first cell that stands over it, a
/// face of higher ground among them: shots fall off cliffs and never climb them. On one level's
/// ground it is the flags test.
pub fn first_blocked_free_shot(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<(i32, i32)> {
    if g.flat_between(a.cell(), b.cell()) {
        return walk(a, b, |cx, cy, _, _| g.flags_at(cx, cy) & mask != 0);
    }
    let (sx, sy) = a.cell();
    let mut h = ground_rungs(g, sx, sy) + i64::from(EYE_RUNGS);
    walk(a, b, |x, y, _, _| match stands(g, x, y, mask) {
        None => true,
        Some(top) if top > h => true,
        Some(_) => {
            h = ground_rungs(g, x, y) + i64::from(EYE_RUNGS);
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use jane_core::grid::Grid;
    use jane_core::{Fx, Rect, Tile};

    use super::*;

    fn px(x: i32, y: i32) -> Vec2 {
        Vec2::new(Fx::from_px(x), Fx::from_px(y))
    }

    fn room(w: u32, h: u32) -> ZoneGrid {
        ZoneGrid::new(Grid::new(w, h, Tile::Floor))
    }

    #[test]
    fn blocked_by_walls_and_open_across_water() {
        let mut g = room(32, 8);
        for (x, y) in Rect::new(10, 0, 1, 8).cells() {
            g.set_tile(x, y, Tile::Wall);
        }
        for (x, y) in Rect::new(20, 0, 2, 8).cells() {
            g.set_tile(x, y, Tile::Water);
        }
        assert!(line_of_sight(&g, px(4 * 8, 20), px(9 * 8, 22)));
        assert!(!line_of_sight(&g, px(4 * 8, 20), px(14 * 8, 22)));
        // Bolts crossed ponds in 2020; water blocks feet, not sight.
        assert!(line_of_sight(&g, px(14 * 8, 20), px(28 * 8, 30)));
        assert!(g.solid(20, 3));
    }

    #[test]
    fn cannot_slip_between_diagonal_walls() {
        let mut g = room(8, 8);
        g.set_tile(3, 4, Tile::Wall);
        g.set_tile(4, 3, Tile::Wall);
        assert!(!line_of_sight(&g, px(3 * 8 + 1, 3 * 8 + 1), px(4 * 8 + 7, 4 * 8 + 7)));
    }

    #[test]
    fn the_start_cell_is_never_tested_and_the_walls_ask_ignores_props() {
        let mut g = room(8, 8);
        g.set_tile(1, 1, Tile::Wall);
        assert!(line_of_sight(&g, px(12, 12), px(40, 12)));
        g.stamp_prop(Rect::new(3, 1, 1, 1), true);
        assert!(!line_of_sight(&g, px(12, 12), px(40, 12)));
        assert!(line_of_sight_walls(&g, px(12, 12), px(40, 12)));
        assert_eq!(first_blocked_cell(&g, px(12, 12), px(40, 12), BLOCK_SIGHT), Some((3, 1)));
    }

    #[test]
    fn visits_every_cell_it_touches() {
        // A shallow line from the middle of (0,0) to the middle of (7,2) crosses 10 cells.
        let g = room(8, 8);
        let mut seen = Vec::new();
        let (a, b) = (px(4, 4), px(60, 20));
        let mut mask_grid = g.clone();
        for x in 0..8 {
            for y in 0..8 {
                mask_grid.set_tile(x, y, Tile::Wall);
                if first_blocked_cell(&mask_grid, a, b, BLOCK_SIGHT) == Some((x, y)) && (x, y) != (0, 0) {
                    seen.push((x, y));
                }
                mask_grid.set_tile(x, y, Tile::Floor);
            }
        }
        seen.sort_by_key(|&(x, y)| (x, y));
        assert_eq!(seen.len(), 9, "{seen:?}");
        assert!(seen.contains(&(7, 2)));
    }
}
