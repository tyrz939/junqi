//! C2's lightmap (PORT.md §13.5: the multiply lightmap, `soft`'s method): a buffer at a quarter
//! of the canvas, cleared to the ambient, every light added over its disc as `colour *
//! LUT[(d2 * 255) / r2]` warm-leaning and held under half again as bright, as `soft` builds
//! it (`jane-render-soft/src/lightmap.rs`, without the casting lights' own pools: C2 casts no
//! shadows). The GE stretches it over the frame with bilinear filtering and a doubled multiply,
//! so a texel holds half the light. Integer only.

use alloc::vec::Vec;

use jane_present::{Light, LightKind, Rgb};

/// Canvas px per light cell, each way.
pub const CELL: i32 = 4;
/// The texture's side: a quarter of 480 x 272 and a cell round it fits in 128.
pub const SIDE: usize = 128;

const fn falloff() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let x = 255 - i as u32;
        t[i] = (x * x * x * 256 / (255 * 255 * 255)) as u16;
        i += 1;
    }
    t
}

const LUT: [u16; 256] = falloff();
const GAIN: u32 = 150;
const CAP: u32 = 384;

/// The light buffer as the GE's texture, `SIDE x SIDE` `0xAABBGGRR`, half the light a channel.
#[derive(Debug, Default)]
pub struct LightMap {
    cells: Vec<[u16; 3]>,
    pub px: Vec<u32>,
    /// Cells across and down this frame.
    pub w: i32,
    pub h: i32,
}

impl LightMap {
    /// Fills it for a canvas `cw x ch` from the ambient and the lights.
    pub fn build(&mut self, (cw, ch): (i32, i32), ambient: Rgb, lights: &[Light]) {
        self.w = (cw / CELL + 2).min(SIDE as i32);
        self.h = (ch / CELL + 2).min(SIDE as i32);
        let n = (self.w * self.h) as usize;
        let base = ambient.map(|c| u16::from(c) + u16::from(c >> 7));
        let dark = jane_present::light::pool(ambient);
        self.cells.clear();
        self.cells.resize(n, base);
        for l in lights {
            let r = i32::from(l.radius);
            if r <= 0 {
                continue;
            }
            let r2 = (r * r) as u32;
            let [cr, cg, cb] = l.colour.map(|c| (u32::from(c) * GAIN * dark / 256) >> 8);
            let col = [cr, cg * 13 / 16, cb * 10 / 16];
            let cone = match l.kind {
                LightKind::Spot { dir, cone } => {
                    use jane_core::angle::{cos_q15, sin_q15};
                    let (c, sn, k) = (cos_q15(dir).0, sin_q15(dir).0, cos_q15(cone).0);
                    Some((i64::from(c), i64::from(sn), i64::from(k)))
                }
                LightKind::Point => None,
            };
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
                    if let Some((c, sn, k)) = cone {
                        let dot = i64::from(dx) * c + i64::from(dy) * sn;
                        if dot < 0 && k >= 0 || dot * dot.abs() < k * k.abs() * i64::from(d2) {
                            continue;
                        }
                    }
                    let k = u32::from(LUT[(d2 * 255 / r2) as usize]);
                    let c = &mut self.cells[(cy * self.w + cx) as usize];
                    for ch in 0..3 {
                        c[ch] = (u32::from(c[ch]) + ((col[ch] * k) >> 8)).min(CAP) as u16;
                    }
                }
            }
        }
        self.px.clear();
        self.px.resize(SIDE * SIDE, 0xff00_0000);
        for cy in 0..self.h {
            for cx in 0..self.w {
                let c = self.cells[(cy * self.w + cx) as usize].map(|v| u32::from(v / 2).min(255));
                self.px[cy as usize * SIDE + cx as usize] = 0xff00_0000 | c[2] << 16 | c[1] << 8 | c[0];
            }
        }
    }
}

/// The halo texture's side: a soft disc a light's glow is drawn with (`Tex::Disc`).
pub const DISC: usize = 32;

/// The halo: white, its alpha `(1 - d^2 / r^2)^2` from the middle out, `0xAABBGGRR`.
pub fn disc() -> Vec<u32> {
    let r = DISC as i32 / 2;
    let mut v = alloc::vec![0u32; DISC * DISC];
    for y in 0..DISC as i32 {
        for x in 0..DISC as i32 {
            let (dx, dy) = (2 * x + 1 - 2 * r, 2 * y + 1 - 2 * r);
            let d2 = (dx * dx + dy * dy) as u32;
            let r2 = (4 * r * r) as u32;
            if d2 < r2 {
                let f = 255 * (r2 - d2) / r2;
                v[(y as usize) * DISC + x as usize] = (f * f / 255) << 24 | 0x00ff_ffff;
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lamp_lifts_its_pool_over_the_night_and_halves_into_the_texture() {
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
        let mut m = LightMap::default();
        m.build((96, 96), [64, 64, 128], &[lamp]);
        let at = |x: usize, y: usize| m.px[y * SIDE + x];
        // Far off: the ambient, halved (64 -> 32, 128 -> 129 / 2).
        assert_eq!(at(0, 0), 0xff40_2020);
        // Under the lamp: brighter, and warm (red over blue).
        let c = at(10, 10);
        assert!((c & 0xff) > 0x50 && (c & 0xff) > (c >> 16 & 0xff) / 2, "{c:08x}");
    }
}
