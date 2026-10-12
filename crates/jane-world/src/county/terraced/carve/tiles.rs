//! What the carve lays on a row of cells: faces and rims from the block levels (R1 and R3's
//! convention, MAP.md 2.1), and then the made things over them (water, joins, spans, planks, the
//! cutting) as the later stages add them.

use jane_core::Tile;

use super::blocks::Blocks;
use super::{B, BW, W};

/// The faces and rims of row `y` from the blocks' levels: a south drop of `d` levels makes the last
/// `2 d` rows of the upper ground `Cliff`, and a drop to the east, west or north makes its outermost
/// cell a `Cliff` rim. Cells the carve lays nothing on stay `None`. `out` is cleared by the caller.
pub fn face_row(b: &Blocks, y: i32, out: &mut [Option<Tile>]) {
    debug_assert_eq!(out.len(), W as usize);
    let (by, r) = (y / B, y % B);
    let lv = |bx: i32, by: i32| b.lv(bx, by);
    for bx in 0..BW {
        let l = b.level[Blocks::ix(bx, by)];
        let south = lv(bx, by + 1).map_or(0, |s| l.saturating_sub(s));
        let face = south > 0 && r >= B - 2 * i32::from(south);
        let north_rim = r == 0 && lv(bx, by - 1).is_some_and(|n| n < l);
        let x0 = (bx * B) as usize;
        if face || north_rim {
            out[x0..x0 + B as usize].fill(Some(Tile::Cliff));
            continue;
        }
        if lv(bx - 1, by).is_some_and(|w| w < l) {
            out[x0] = Some(Tile::Cliff);
        }
        if lv(bx + 1, by).is_some_and(|e| e < l) {
            out[x0 + B as usize - 1] = Some(Tile::Cliff);
        }
    }
}

/// Is cell `(x, y)` a face or a rim on the blocks' levels? (One cell of [`face_row`].)
pub fn cliff_at(b: &Blocks, x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= W || y >= W {
        return false;
    }
    let (bx, by, r, c) = (x / B, y / B, y % B, x % B);
    let l = b.level[Blocks::ix(bx, by)];
    let south = b.lv(bx, by + 1).map_or(0, |s| l.saturating_sub(s));
    if south > 0 && r >= B - 2 * i32::from(south) {
        return true;
    }
    (r == 0 && b.lv(bx, by - 1).is_some_and(|n| n < l))
        || (c == 0 && b.lv(bx - 1, by).is_some_and(|w| w < l))
        || (c == B - 1 && b.lv(bx + 1, by).is_some_and(|e| e < l))
}
