//! Smooth value noise over a box of world px, for the ground's broad tonal patches and the wobble
//! of its edges: a lattice every `2^shift` px hashed once per box, then bilinear with a
//! smoothstep. A pure function of world px and salt, so two chunks agree along their seam.
//! Integer only; the buffers are reused, never reallocated after the first box.

use alloc::vec::Vec;
use jane_core::hash::hash2;

/// A noise field over one box.
#[derive(Clone, Debug, Default)]
pub(crate) struct Field {
    shift: u32,
    /// World px of the box's top-left.
    x0: i32,
    y0: i32,
    /// Lattice index of the first lattice column and row.
    lx: i32,
    ly: i32,
    /// Lattice points per row.
    n: i32,
    vals: Vec<u8>,
}

/// `3t² - 2t³` for `t` in 0..=256, in 1/256ths.
pub(crate) const fn smooth(t: i32) -> i32 {
    (t * t * (768 - 2 * t)) >> 16
}

impl Field {
    /// Hash the lattice over the box `(x0, y0, w, h)` of world px, a point every `2^shift` px.
    pub fn fill(&mut self, x0: i32, y0: i32, w: i32, h: i32, shift: u32, salt: u32) {
        self.shift = shift;
        self.x0 = x0;
        self.y0 = y0;
        self.lx = x0 >> shift;
        self.ly = y0 >> shift;
        let (lx1, ly1) = ((x0 + w - 1) >> shift, (y0 + h - 1) >> shift);
        self.n = lx1 - self.lx + 2;
        let rows = ly1 - self.ly + 2;
        self.vals.clear();
        for j in 0..rows {
            for i in 0..self.n {
                self.vals.push((hash2(self.lx + i, self.ly + j, salt) >> 24) as u8);
            }
        }
    }

    /// `k` times the field at every px of the box `(x0, y0, w, h)` of world px (inside the filled
    /// box), added into `out` row by row: the same numbers as [`Field::at`], worked a lattice
    /// cell at a time, so a chunk's worth costs a multiply a px.
    ///
    /// `out` is `i16`: a field is 0..=255, so `k` times it summed over a few fields (at most 2 040
    /// today) fits, at half the scratch of an `i32` a px (a console's RAM).
    pub fn add_box(&self, x0: i32, y0: i32, w: i32, h: i32, k: i32, out: &mut [i16]) {
        let mask = (1 << self.shift) - 1;
        let n = self.n as usize;
        let v = |i: usize| i32::from(self.vals.get(i).copied().unwrap_or(128));
        for y in 0..h {
            let wy = y0 + y;
            let gy = (wy >> self.shift) - self.ly;
            let ty = smooth(((wy & mask) * 256) >> self.shift);
            let mut last = i32::MIN;
            let (mut a, mut b) = (0, 0);
            let row = &mut out[(y * w) as usize..((y + 1) * w) as usize];
            for (x, o) in row.iter_mut().enumerate() {
                let wx = x0 + x as i32;
                let gx = (wx >> self.shift) - self.lx;
                if gx != last {
                    last = gx;
                    let kk = usize::try_from(gy * self.n + gx).unwrap_or(0);
                    a = v(kk) * (256 - ty) + v(kk + n) * ty;
                    b = v(kk + 1) * (256 - ty) + v(kk + 1 + n) * ty;
                }
                let tx = smooth(((wx & mask) * 256) >> self.shift);
                *o += (k * ((a * (256 - tx) + b * tx) >> 16)) as i16;
            }
        }
    }

    /// The field at world px `(x, y)` inside the box, 0..=255.
    pub fn at(&self, x: i32, y: i32) -> i32 {
        let (gx, gy) = ((x >> self.shift) - self.lx, (y >> self.shift) - self.ly);
        let mask = (1 << self.shift) - 1;
        let (tx, ty) = (smooth(((x & mask) * 256) >> self.shift), smooth(((y & mask) * 256) >> self.shift));
        let Some(k) = usize::try_from(gy * self.n + gx).ok() else { return 128 };
        let n = self.n as usize;
        let v = |i: usize| i32::from(self.vals.get(i).copied().unwrap_or(128));
        let top = v(k) * (256 - tx) + v(k + 1) * tx;
        let bottom = v(k + n) * (256 - tx) + v(k + n + 1) * tx;
        (top * (256 - ty) + bottom * ty) >> 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_boxes_agree_where_they_overlap() {
        let (mut a, mut b) = (Field::default(), Field::default());
        a.fill(0, 0, 256, 256, 5, 7);
        b.fill(200, -40, 256, 256, 5, 7);
        for y in 0..200 {
            for x in 200..256 {
                assert_eq!(a.at(x, y), b.at(x, y), "({x}, {y})");
            }
        }
        let lo = (0..256).map(|x| a.at(x, 9)).min().unwrap();
        let hi = (0..256).map(|x| a.at(x, 9)).max().unwrap();
        assert!(hi - lo > 40, "it varies: {lo}..{hi}");
    }
}
