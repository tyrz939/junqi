//! The packed level plane (section 2.5): a byte a cell, but never held whole. A cell's level is its
//! block's, except where a made thing (a join's lower half, an embankment, a cutting) sets its own.

use jane_core::{Plane, Rect};

use super::blocks::Blocks;
use super::{B, BW, W};

/// Pack the level plane a band of 16 rows at a time from the blocks, then the `fixes` (rect, level)
/// in order.
pub fn pack_levels(b: &Blocks, fixes: &[(Rect, u8)]) -> Plane {
    Plane::pack_bands(W as u32, W as u32, |y0, rows, out| {
        for r in 0..rows {
            let y = (y0 + r) as i32;
            let by = y / B;
            let row = &mut out[(r * W as u32) as usize..((r + 1) * W as u32) as usize];
            for bx in 0..BW {
                let l = b.level[Blocks::ix(bx, by)];
                row[(bx * B) as usize..(bx * B + B) as usize].fill(l);
            }
            for (rect, l) in fixes {
                if y >= rect.y && y < rect.bottom() {
                    let (x0, x1) = (rect.x.max(0), rect.right().min(W));
                    if x1 > x0 {
                        row[x0 as usize..x1 as usize].fill(*l);
                    }
                }
            }
        }
    })
}
