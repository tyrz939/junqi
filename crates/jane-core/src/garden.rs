//! A house's front garden as the world and the painter both read it off the tiles, so the fence
//! she walks into is the fence she sees. A house is a 4-connected block of roof and wall
//! ([`blocks`]); its garden the rows of grass in front of its walls, three at most, its front row
//! a boundary only where the sim has one: a row of fence or low wall, the gate a gap in it
//! ([`front`]). `jane-world`'s county lays those fences (`county::gardens`); `jane-art` draws
//! each house's own boundary on them and its gate in the gap.

use alloc::vec;
use alloc::vec::Vec;

use crate::grid::Rect;
use crate::tile::Tile;

/// The most rows of garden in front of a house, its boundary row with them.
pub const PLOT_ROWS: i32 = 3;

/// Whether `t` is a house's roof or wall.
pub const fn built(t: Tile) -> bool {
    matches!(t, Tile::HouseWall | Tile::HouseRoof | Tile::Eaves)
}

/// Whether `t` is ground a front garden is laid on.
pub const fn plot_ground(t: Tile) -> bool {
    matches!(t, Tile::Grass | Tile::FlowerBed | Tile::Garden | Tile::GrassTall)
}

/// Whether `t` is a garden's boundary to the sim: a fence or a low wall (feet stop, eyes pass).
pub const fn boundary(t: Tile) -> bool {
    matches!(t, Tile::Fence | Tile::StoneWall)
}

/// A house's block of roof and wall, and the first row of its walls (the roof is the rows above).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub rect: Rect,
    pub eave: i32,
}

/// Every house block of a zone of `(w, h)` cells whose tile at `(x, y)` is `tile(x, y)`, in
/// reading order of their first cell: 4-connected roof and wall with at least one cell of wall.
pub fn blocks((w, h): (i32, i32), tile: impl Fn(i32, i32) -> Tile) -> Vec<Block> {
    let mut out = Vec::new();
    let mut seen = vec![0u64; ((w.max(0) * h.max(0)) as usize).div_ceil(64)];
    let mark = |seen: &mut Vec<u64>, x: i32, y: i32| {
        let k = (y * w + x) as usize;
        let was = seen[k / 64] >> (k % 64) & 1 == 1;
        seen[k / 64] |= 1 << (k % 64);
        was
    };
    let mut stack = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !built(tile(x, y)) || mark(&mut seen, x, y) {
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
            let mut eave = i32::MAX;
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                (x0, y0, x1, y1) = (x0.min(cx), y0.min(cy), x1.max(cx), y1.max(cy));
                if tile(cx, cy) == Tile::HouseWall {
                    eave = eave.min(cy);
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx + dx, cy + dy);
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && built(tile(nx, ny)) && !mark(&mut seen, nx, ny) {
                        stack.push((nx, ny));
                    }
                }
            }
            if eave != i32::MAX {
                out.push(Block { rect: Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1), eave });
            }
        }
    }
    out
}

/// The front garden of block `rect` in a zone `h` rows tall: how many rows it has (0: none) and
/// whether the last of them is its boundary. Rows of mostly garden ground (two cells in three)
/// count, three at most; after the first, a row at least half fence or low wall is the boundary
/// and ends it. Without one the garden is open: the painter draws it no boundary.
pub fn front(rect: Rect, h: i32, tile: impl Fn(i32, i32) -> Tile) -> (i32, bool) {
    let mut rows = 0;
    while rows < PLOT_ROWS {
        let gy = rect.bottom() + rows;
        if gy >= h {
            break;
        }
        let count = |f: fn(Tile) -> bool| (rect.x..rect.right()).filter(|&gx| f(tile(gx, gy))).count() as i32;
        if rows >= 1 && count(boundary) * 2 >= rect.w {
            return (rows + 1, true);
        }
        if count(plot_ground) * 3 < rect.w * 2 {
            break;
        }
        rows += 1;
    }
    (rows, false)
}

/// The west cell of the two a gate is laid in, in a garden `plot` whose house's door's west cell
/// is `door`: under the door, else the middle; never past either end.
pub fn gate_x(plot: Rect, door: Option<i32>) -> i32 {
    door.unwrap_or(plot.x + plot.w / 2 - 1).clamp(plot.x, plot.right() - 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fence_row_ends_the_garden_and_an_open_one_runs_three() {
        // A house of rows 0..4, cols 0..6; grass below, a fence on row 6 with a gap at 2 and 3.
        let tile = |fenced: bool| {
            move |x: i32, y: i32| match y {
                0..=1 => Tile::HouseRoof,
                2..=4 => Tile::HouseWall,
                6 if fenced && x != 2 && x != 3 => Tile::Fence,
                _ => Tile::Grass,
            }
        };
        let b = blocks((6, 12), tile(true));
        assert_eq!(b, vec![Block { rect: Rect::new(0, 0, 6, 5), eave: 2 }]);
        assert_eq!(front(b[0].rect, 12, tile(true)), (2, true));
        assert_eq!(front(b[0].rect, 12, tile(false)), (3, false));
        assert_eq!(gate_x(Rect::new(0, 5, 6, 2), Some(5)), 4);
        assert_eq!(gate_x(Rect::new(0, 5, 6, 2), None), 2);
    }
}
