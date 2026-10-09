//! The T0 lightmap (PRESENTATION.md §1.7): a light buffer at a quarter of the canvas, cleared
//! to the ambient, every light the view says is showing added over its disc as `colour *
//! LUT[(d2 * 255) / r2]`, upsampled bilinear and multiplied into the frame, `dst = dst * L >> 8`
//! (a light may take a pixel up to twice as bright). Integer only.

use jane_core::angle::{cos_q15, sin_q15};
use jane_present::{Band, Light, Rgb};

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

/// The most casting lights whose own pools the buffer keeps apart (`own`): T0's `shadows` row
/// at its most.
pub const OWN: usize = jane_present::frame::T0_SHADOWS as usize;

/// A cell's four corners as [`LightMap::corners`] gives them.
pub type Corners = [[u32; 3]; 4];

/// The light buffer, reused frame to frame.
#[derive(Debug, Default)]
pub struct LightMap {
    cells: Vec<[u16; 3]>,
    /// Each cell's light before the cap: what the cap is taken from again when a light is left
    /// out (a point light's shadow, `light_without`).
    total: Vec<[u16; 3]>,
    /// The pools of the first [`OWN`] casting lights, each on its own, and which light each is.
    own: Vec<[u16; 3]>,
    own_of: Vec<usize>,
    /// Each cell's light as a reciprocal, `2^24 / light` per channel, for the share left in a
    /// shadow (made with the kept-apart pools, only when a light casts).
    recip: Vec<[u32; 3]>,
    w: i32,
    h: i32,
    rows: Vec<[u16; 3]>,
}

impl LightMap {
    /// Fills the buffer for a canvas `cw x ch`: the ambient (by row through the night's turn,
    /// `band`), then every light; and apart, the pool of each of the first [`OWN`] lights that
    /// cast.
    pub fn build(&mut self, (cw, ch): (i32, i32), ambient: Rgb, lights: &[Light], band: Band) {
        self.w = cw / CELL + 2;
        self.h = ch / CELL + 2;
        let n = (self.w * self.h) as usize;
        let base = ambient.map(|c| u16::from(c) + u16::from(c >> 7));
        self.own_of.clear();
        self.own_of.extend(lights.iter().enumerate().filter(|(_, l)| l.casts).map(|(i, _)| i).take(OWN));
        self.own.clear();
        self.own.resize(n * self.own_of.len(), [0; 3]);
        self.total.clear();
        self.total.resize(n, base);
        // A pool shows against the dark: by day it adds a little, at night all of it, by the
        // rule every tier keeps (`light::pool`).
        let dark = jane_present::light::pool(ambient);
        self.cells.clear();
        self.cells.resize(n, base);
        if !band.is_none() {
            // The turn: each row of cells keeps the band's share of the sky's light there.
            let w = self.w as usize;
            for cy in 0..self.h {
                let k = band.at(cy * CELL);
                let row = base.map(|c| (u32::from(c) * k / 256) as u16);
                let at = cy as usize * w;
                self.cells[at..at + w].fill(row);
                self.total[at..at + w].fill(row);
            }
        }
        for (li, l) in lights.iter().enumerate() {
            let own = self.own_of.iter().position(|&o| o == li);
            let r = i32::from(l.radius);
            if r <= 0 {
                continue;
            }
            let r2 = (r * r) as u32;
            // Flame light leans warm, as on T2: a yellow lamp on green grass is not lime.
            let [cr, cg, cb] = l.colour.map(|c| (u32::from(c) * GAIN * dark / 256) >> 8);
            let col = [cr, cg * 13 / 16, cb * 10 / 16];
            // A spot lights its cone alone (a sentry's eye, a lit window's spill), as on T1 and
            // T2: a cell whose direction from the light is further round than its half-angle.
            let cone = match l.kind {
                jane_present::LightKind::Spot { dir, cone } => {
                    let (c, sn, k) = (cos_q15(dir).0, sin_q15(dir).0, cos_q15(cone).0);
                    Some((i64::from(c), i64::from(sn), i64::from(k)))
                }
                jane_present::LightKind::Point => None,
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
                    let i = (cy * self.w + cx) as usize;
                    let add = [0, 1, 2].map(|ch| (col[ch] * k) >> 8);
                    let c = &mut self.cells[i];
                    for ch in 0..3 {
                        c[ch] = (u32::from(c[ch]) + add[ch]).min(CAP) as u16;
                    }
                    let t = &mut self.total[i];
                    for ch in 0..3 {
                        t[ch] = (u32::from(t[ch]) + add[ch]).min(u32::from(u16::MAX)) as u16;
                    }
                    if let Some(o) = own {
                        self.own[o * n + i] = add.map(|a| a as u16);
                    }
                }
            }
        }
        self.recip.clear();
        if !self.own_of.is_empty() {
            self.recip
                .extend(self.cells.iter().map(|c| c.map(|v| if v == 0 { 0 } else { (1u32 << 24) / u32::from(v) })));
        }
    }

    /// Where the casting light `li` (its index in the lights `build` was handed) is kept apart, if
    /// it is.
    pub fn own_slot(&self, li: usize) -> Option<usize> {
        self.own_of.iter().position(|&o| o == li)
    }

    /// The four cells round canvas px `(x, y)` (`[at, below, right, right below]`), each as the
    /// share of its light, of 256, that is left without the pools of the kept-apart lights in
    /// `gone` (a bit a slot): what a px in their shadows keeps of what [`apply`](Self::apply)
    /// multiplies it by. The same for every px of a cell: [`blend`](Self::blend) takes them to a
    /// px.
    pub fn corners(&self, x: i32, y: i32, gone: u8) -> Corners {
        let n = (self.w * self.h) as usize;
        let at = |cx: i32, cy: i32| -> [u32; 3] {
            let i = (cy.min(self.h - 1) * self.w + cx.min(self.w - 1)) as usize;
            let mut t = self.total[i].map(u32::from);
            let mut left = gone;
            while left != 0 {
                let o = left.trailing_zeros() as usize;
                left &= left - 1;
                let c = self.own[o * n + i];
                for ch in 0..3 {
                    // Its umbra keeps a little of it (`shadow::LAMP_BOUNCE`).
                    t[ch] = t[ch].saturating_sub((u32::from(c[ch]) * (256 - jane_present::shadow::LAMP_BOUNCE)) >> 8);
                }
            }
            let r = self.recip[i];
            [0, 1, 2].map(|k| ((u64::from(t[k].min(CAP)) * u64::from(r[k])) >> 16) as u32)
        };
        let (cx, cy) = (x / CELL, y / CELL);
        [at(cx, cy), at(cx, cy + 1), at(cx + 1, cy), at(cx + 1, cy + 1)]
    }

    /// The share left at canvas px `(x, y)`, of 256, from its cell's [`corners`](Self::corners),
    /// bilinear by `apply`'s steps.
    pub fn blend(c: &Corners, x: i32, y: i32) -> [u32; 3] {
        let (fx, fy) = ((x % CELL) as u32, (y % CELL) as u32);
        let lerp = |p: [u32; 3], q: [u32; 3], f: u32| [0, 1, 2].map(|k| (p[k] * (4 - f) + q[k] * f) / 4);
        let [p0, q0, p1, q1] = *c;
        lerp(lerp(p0, q0, fy), lerp(p1, q1, fy), fx)
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
        m.build((96, 96), [64, 64, 128], &[lamp], Band::NONE);
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
