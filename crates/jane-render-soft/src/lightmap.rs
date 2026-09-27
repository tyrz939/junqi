//! The T0 lightmap (PRESENTATION.md §1.7): a light buffer at a quarter of the canvas, cleared
//! to the ambient, every light the view says is showing added over its disc as `colour *
//! LUT[(d2 * 255) / r2]`, upsampled bilinear and multiplied into the frame, `dst = dst * L >> 8`
//! (a light may take a pixel up to twice as bright). Integer only.

use jane_present::{Light, Rgb};

use crate::blit::Target;

/// Canvas px per light cell, each way.
const CELL: i32 = 4;

/// How much of a light's colour reaches a point `(d2 * 255) / r2` of the way out, 0..=256: a
/// soft pool, full at the middle and nothing at the rim.
const fn falloff() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let x = 255 - i as u32;
        // (1 - d2/r2)^3: a bright heart and a long soft tail.
        t[i] = (x * x * x * 256 / (255 * 255 * 255)) as u16;
        i += 1;
    }
    t
}

const LUT: [u16; 256] = falloff();

/// How bright a light's middle is over the ambient, of 256: lamps read as sources at night.
const GAIN: u32 = 150;
/// The most a pool lifts a pixel, of 256: half again as bright.
const CAP: u32 = 384;

/// The light buffer, reused frame to frame.
#[derive(Debug, Default)]
pub struct LightMap {
    cells: Vec<[u16; 3]>,
    w: i32,
    h: i32,
    rows: Vec<[u16; 3]>,
}

impl LightMap {
    /// Fills the buffer for a canvas `cw x ch`: the ambient, then every light.
    pub fn build(&mut self, (cw, ch): (i32, i32), ambient: Rgb, lights: &[Light]) {
        self.w = cw / CELL + 2;
        self.h = ch / CELL + 2;
        let n = (self.w * self.h) as usize;
        let base = ambient.map(|c| u16::from(c) + u16::from(c >> 7));
        // A pool shows against the dark: by day it adds a little, at night all of it.
        let dark = (300 - ambient.iter().map(|&c| u32::from(c)).sum::<u32>() / 3).min(220);
        self.cells.clear();
        self.cells.resize(n, base);
        for l in lights {
            let r = i32::from(l.radius);
            if r <= 0 {
                continue;
            }
            let r2 = (r * r) as u32;
            // Flame light leans warm, as on T2: a yellow lamp on green grass is not lime.
            let [cr, cg, cb] = l.colour.map(|c| (u32::from(c) * GAIN * dark / 220) >> 8);
            let col = [cr, cg * 13 / 16, cb * 11 / 16];
            let (x0, x1) = (((l.pos.0 - r) / CELL).max(0), ((l.pos.0 + r) / CELL + 1).min(self.w - 1));
            let (y0, y1) = (((l.pos.1 - r) / CELL).max(0), ((l.pos.1 + r) / CELL + 1).min(self.h - 1));
            for cy in y0..=y1 {
                let dy = cy * CELL - l.pos.1;
                for cx in x0..=x1 {
                    let dx = cx * CELL - l.pos.0;
                    let d2 = (dx * dx + dy * dy) as u32;
                    if d2 >= r2 {
                        continue;
                    }
                    let k = u32::from(LUT[(d2 * 255 / r2) as usize]);
                    let c = &mut self.cells[(cy * self.w + cx) as usize];
                    for ch in 0..3 {
                        c[ch] = (u32::from(c[ch]) + ((col[ch] * k) >> 8)).min(CAP) as u16;
                    }
                }
            }
        }
    }

    /// Multiplies `t` by the buffer, bilinear between cells: returns pixels written.
    pub fn apply(&mut self, t: &mut Target<'_>) -> u64 {
        let w = self.w;
        self.rows.resize(w as usize, [0; 3]);
        for y in 0..t.h {
            let (cy, fy) = (y / CELL, (y % CELL) as u32);
            let (a, b) = ((cy * w) as usize, ((cy + 1).min(self.h - 1) * w) as usize);
            for (x, r) in self.rows.iter_mut().enumerate() {
                let (p, q) = (self.cells[a + x], self.cells[b + x]);
                *r = [0, 1, 2].map(|k| ((u32::from(p[k]) * (4 - fy) + u32::from(q[k]) * fy) / 4) as u16);
            }
            let row = &mut t.px[(y * t.w) as usize..((y + 1) * t.w) as usize];
            for (x, d) in row.iter_mut().enumerate() {
                let (cx, fx) = (x / CELL as usize, (x % CELL as usize) as u32);
                let (p, q) = (self.rows[cx], self.rows[(cx + 1).min(w as usize - 1)]);
                let c = *d;
                let ch = |shift: u32, k: usize| {
                    let l = (u32::from(p[k]) * (4 - fx) + u32::from(q[k]) * fx) / 4;
                    ((((c >> shift) & 0xff) * l) >> 8).min(255) << shift
                };
                *d = 0xff00_0000 | ch(16, 0) | ch(8, 1) | ch(0, 2);
            }
        }
        (t.w * t.h) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::LightKind;

    #[test]
    fn a_lamp_lights_its_pool_and_nothing_past_its_rim() {
        let mut m = LightMap::default();
        let lamp = Light {
            pos: (40, 40),
            height: 30,
            colour: [255, 200, 100],
            radius: 24,
            size: 4,
            casts: true,
            kind: LightKind::Point,
            holder: None,
        };
        m.build((96, 96), [64, 64, 128], &[lamp]);
        let mut px = vec![0xff80_8080; 96 * 96];
        let mut t = Target { px: &mut px, w: 96, h: 96 };
        m.apply(&mut t);
        let at = |x: usize, y: usize| px[y * 96 + x];
        // Under the lamp: over three times the night's red, and warm.
        let mid = at(40, 40);
        assert!((mid >> 16) & 0xff > 3 * 0x20 && (mid >> 16) & 0xff > mid & 0xff, "{mid:08x}");
        // Far away: the ambient alone, blue night.
        assert_eq!(at(90, 90), 0xff20_2040);
        assert_eq!(LUT[0], 256);
        assert_eq!(LUT[255], 0);
    }
}
