//! Line of sight on the cell grid (`sim/los.ts`): a supercover walk that visits every cell the
//! segment touches, so a bolt cannot slip between two diagonal walls. Sight and projectiles
//! share it.
//!
//! The TS compared float crossing times; here the times are compared exactly by
//! cross-multiplying in `i64` (`dist_x / |dx| < dist_y / |dy|` as `dist_x * |dy| < dist_y *
//! |dx|`), and a tie steps along y as the TS's `<` did.

use jane_core::num::CELL_FX;
use jane_core::tile::{BLOCK_SIGHT, F_BLOCK_LOS};
use jane_core::{CellIx, Vec2};

use crate::grid::ZoneGrid;

/// Nothing that blocks sight lies between the two points.
pub fn line_of_sight(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    first_blocked(g, a, b, BLOCK_SIGHT).is_none()
}

/// Walls only: props (pillars, gates) are ignored. The snake's anti-cheese reset asks this.
pub fn line_of_sight_walls(g: &ZoneGrid, a: Vec2, b: Vec2) -> bool {
    first_blocked(g, a, b, F_BLOCK_LOS).is_none()
}

/// The first cell on the segment whose flags meet `mask`, as `(x, y)`, or `None`. The start
/// cell is never tested: a unit hugging a wall can still see out. A cell outside the grid
/// blocks everything.
pub fn first_blocked_cell(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<(i32, i32)> {
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
    let mut steps = (tx - cx).abs() + (ty - cy).abs();
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
        if g.flags_at(cx, cy) & mask != 0 {
            return Some((cx, cy));
        }
    }
    None
}

/// [`first_blocked_cell`] as a cell index, for a cell inside the grid (`None` also when the
/// blocking cell is outside it: callers that need that distinction ask for the cell).
pub fn first_blocked(g: &ZoneGrid, a: Vec2, b: Vec2, mask: u8) -> Option<CellIx> {
    first_blocked_cell(g, a, b, mask).map(|(x, y)| if g.inside(x, y) { g.ix(x, y) } else { CellIx(u32::MAX) })
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
