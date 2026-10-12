//! Integer value noise for the carve: a lattice of draws every `sc` cells, bilinear between. One
//! dice stream per field (`Step::MapCarve`), so a re-tune of one field moves nothing drawn under
//! another.

use alloc::vec::Vec;
use jane_core::ZoneId;

use crate::steps::{Step, dice};

/// A lattice noise over `extent x extent` cells (and a lattice cell beyond, so every read is
/// inside), values in `lo..=hi` at the nodes.
#[derive(Clone, Debug)]
pub(crate) struct Lattice {
    g: Vec<i32>,
    sc: i32,
    n: i32,
}

impl Lattice {
    #[allow(clippy::too_many_arguments)]
    pub fn new(seed: u32, attempt: u8, step: Step, a: i32, b: i32, extent: i32, sc: i32, lo: i32, hi: i32) -> Self {
        let n = extent / sc + 2;
        let mut r = dice(seed, ZoneId::County, step, attempt, a, b);
        let g = (0..n * n).map(|_| r.range(lo, hi)).collect();
        Self { g, sc, n }
    }

    /// The value at `(x, y)` (cells, `>= 0`), bilinear between the four nodes round it.
    pub fn at(&self, x: i32, y: i32) -> i32 {
        let (x, y) = (x.max(0), y.max(0));
        let (gx, gy) = ((x / self.sc).min(self.n - 2), (y / self.sc).min(self.n - 2));
        let (fx, fy) = (x - gx * self.sc, y - gy * self.sc);
        let v = |i: i32, j: i32| self.g[(j * self.n + i) as usize];
        let top = v(gx, gy) * (self.sc - fx) + v(gx + 1, gy) * fx;
        let bot = v(gx, gy + 1) * (self.sc - fx) + v(gx + 1, gy + 1) * fx;
        (top * (self.sc - fy) + bot * fy) / (self.sc * self.sc)
    }
}

/// `b` for a carve die: the salt in the high half, the axis (or any small index) in the low.
pub(crate) const fn salt_b(salt: u8, axis: i32) -> i32 {
    salt as i32 * 65536 + axis
}
