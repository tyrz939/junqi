//! The integer light pass: a canvas lit by one directional light over an ambient, with the
//! shadows its height layer casts (PRESENTATION.md §1.7). `jane sheet light` draws through it,
//! and it is the kernel the renderer's lighting reproduces: N·L with the light's elevation as
//! `z`, ambient first, the direct term removed where the height field occludes the light,
//! emissive added unlit after the multiply.
//!
//! Units: colours and ambient are per channel in 1/256ths (256 is full), angles are
//! jane-core [`Angle`]s (0 east, clockwise with y down), heights are screen px.

use jane_core::angle::{Angle, cos_q15, sin_q15};

use crate::canvas::{Canvas, UNIT, decode};
use crate::palette::{Ix, rgb};

/// A directional light: the sun, the moon, or a sheet's key light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sun {
    /// Where the light is, seen from the sprite: `Angle::NORTH` is a light up the screen.
    pub azimuth: Angle,
    /// Its height above the ground plane: 0 grazes, a quarter turn (`Angle(16384)`) is overhead.
    pub elevation: Angle,
    /// Its colour per channel, 1/256ths.
    pub colour: [u16; 3],
}

impl Sun {
    /// The unit vector toward the light, Q15, in canvas axes (`+x` east, `+y` south, `+z` up).
    pub fn toward(&self) -> [i32; 3] {
        let (ce, se) = (cos_q15(self.elevation).0, sin_q15(self.elevation).0);
        let (ca, sa) = (cos_q15(self.azimuth).0, sin_q15(self.azimuth).0);
        [(ce * ca) >> 15, (ce * sa) >> 15, se]
    }
}

/// How the pass treats what is not drawn: the ground under a sprite on a sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ground {
    /// The ground's albedo; clear pixels show it, flat and at height 0.
    pub ix: Ix,
}

/// Light `c` by `sun` over `ambient` (1/256ths a channel) and return RGB per pixel, row-major.
/// Clear pixels are `ground`; AO pixels are the ground at 70 %. A pixel whose height field
/// blocks the sun (itself or the ground in the shadow of something taller) gets ambient only.
pub fn light(c: &Canvas, sun: &Sun, ambient: [u16; 3], ground: Ground) -> Vec<[u8; 3]> {
    let l = sun.toward();
    let march = March::new(c, sun);
    let mut out = Vec::with_capacity(c.albedo().len());
    for y in 0..c.h() {
        for x in 0..c.w() {
            let ix = c.get(x, y);
            let base = match ix {
                Ix::CLEAR | Ix::AO => crate::palette::ao(rgb(ground.ix), crate::palette::ao_cover(c, x, y)),
                _ => rgb(ix),
            };
            let [nx, ny, nz] = decode(c.normal_at(x, y));
            let dot = (nx * l[0] + ny * l[1] + nz * l[2]) >> 15;
            let lam = if dot <= 0 || march.blocked(c, x, y) { 0 } else { dot.min(UNIT) };
            let e = c.emissive_at(x, y);
            let glow = if e == Ix::CLEAR { [0; 3] } else { rgb(e) };
            let mut px = [0u8; 3];
            for k in 0..3 {
                let f = i32::from(ambient[k]) + i32::from(sun.colour[k]) * lam / UNIT;
                let v = (i32::from(base[k]) * f) >> 8;
                px[k] = (v + i32::from(glow[k])).min(255) as u8;
            }
            out.push(px);
        }
    }
    out
}

/// A shadow ray toward the sun through the height field, in 1/256 px.
struct March {
    step: [i32; 2],
    rise: i32,
    top: i32,
    on: bool,
}

impl March {
    fn new(c: &Canvas, sun: &Sun) -> March {
        let ce = cos_q15(sun.elevation).0;
        let se = sin_q15(sun.elevation).0;
        let top = i32::from(c.heights().iter().copied().max().unwrap_or(0)) * 256;
        // Overhead (within about 3°) casts nothing.
        let on = ce > 1700 && se > 0;
        let rise = if on { se * 256 / ce } else { 0 };
        let step = [cos_q15(sun.azimuth).0 >> 7, sin_q15(sun.azimuth).0 >> 7];
        March { step, rise, top, on }
    }

    /// Whether something in the height field stands between `(x, y)` and the sun. A 1 px bias
    /// keeps a surface from shadowing itself on its own slope.
    fn blocked(&self, c: &Canvas, x: i32, y: i32) -> bool {
        if !self.on {
            return false;
        }
        let (mut fx, mut fy) = (x * 256 + 128, y * 256 + 128);
        let mut z = i32::from(c.height_at(x, y)) * 256 + 256;
        loop {
            fx += self.step[0];
            fy += self.step[1];
            z += self.rise;
            let (px, py) = (fx >> 8, fy >> 8);
            if z > self.top || px < 0 || py < 0 || px >= c.w() || py >= c.h() {
                return false;
            }
            if i32::from(c.height_at(px, py)) * 256 > z {
                return true;
            }
        }
    }
}

/// The eight compass lights of `jane sheet light` at `elevation`, north first, clockwise, in
/// `colour`; then the ninth, overhead.
pub fn compass(elevation: Angle, colour: [u16; 3]) -> [Sun; 9] {
    let mut out = [Sun { azimuth: Angle::NORTH, elevation: Angle(16384), colour }; 9];
    for (k, s) in out.iter_mut().take(8).enumerate() {
        s.azimuth = Angle::NORTH.wrapping_add(k as i32 * 8192);
        s.elevation = elevation;
    }
    out
}

#[cfg(test)]
mod tests {
    use jane_core::grid::Rect;

    use super::*;
    use crate::canvas::Z;
    use crate::palette::Ramp;

    /// ART.md §5 "lit sphere": a soft_ellipse lit from eight directions through this pass. The
    /// brightest quarter of its pixels lies in the light's half of the disc for every direction,
    /// and the eight results are pairwise distinct.
    ///
    /// At 30°, the sheet's elevation. The albedo is baked lit from the top-left (ART.md §3), so
    /// a light much higher than this lets the baked crown outshine the lit flank: at 35° three
    /// pixels 1.5 px left of centre win under an east light.
    #[test]
    fn lit_sphere() {
        let r = Rect::new(4, 4, 32, 32);
        let mut c = Canvas::new(40, 40);
        c.soft_ellipse(r, Ramp::Stone, Z::new(1, 16));
        let ground = Ground { ix: Ramp::Grass.at(crate::palette::Tone::Base) };
        let mut seen: Vec<Vec<[u8; 3]>> = Vec::new();
        for sun in compass(Angle::from_degrees(30), [256, 244, 220]).iter().take(8) {
            let lit = light(&c, sun, [40, 40, 52], ground);
            let l = sun.toward();
            let mut disc: Vec<(u32, usize)> = Vec::new();
            for (i, p) in lit.iter().enumerate() {
                let (x, y) = (i as i32 % c.w(), i as i32 / c.w());
                if c.get(x, y).is_opaque() {
                    let luma = 299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]);
                    disc.push((luma, i));
                }
            }
            // Brightest first; ties by index, a total key.
            disc.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            // The dividing line runs through the disc's centre across the light; a pixel within
            // 1 px of it straddles the line and counts as on it. Doubled coordinates, so 1 px
            // is 2 units of distance times the length of the light's ground vector.
            let reach = 2 * jane_core::num::isqrt((l[0] * l[0] + l[1] * l[1]) as u64) as i32;
            let far: Vec<(i32, i32)> = disc[..disc.len() / 4]
                .iter()
                .map(|&(_, i)| (i as i32 % c.w(), i as i32 / c.w()))
                .filter(|&(x, y)| {
                    let (ux, uy) = (2 * x + 1 - (2 * r.x + r.w), 2 * y + 1 - (2 * r.y + r.h));
                    ux * l[0] + uy * l[1] < -reach
                })
                .collect();
            assert!(
                far.is_empty(),
                "{:?}: {} of {} bright on the far side: {far:?}",
                sun.azimuth,
                far.len(),
                disc.len() / 4
            );
            assert!(!seen.contains(&lit), "{:?} repeats an earlier direction", sun.azimuth);
            seen.push(lit);
        }
    }

    #[test]
    fn a_tall_thing_shadows_the_ground_away_from_the_sun() {
        let mut c = Canvas::new(40, 40);
        c.fill_rect(Rect::new(18, 18, 4, 4), Ramp::Stone.at(crate::palette::Tone::Base), 20);
        let sun = Sun { azimuth: Angle::WEST, elevation: Angle::from_degrees(45), colour: [256; 3] };
        let lit = light(&c, &sun, [32; 3], Ground { ix: Ramp::Grass.at(crate::palette::Tone::Base) });
        let at = |x: i32, y: i32| lit[(y * 40 + x) as usize][1];
        assert!(at(26, 20) < at(12, 20), "east of the block is in its shadow");
        assert_eq!(at(12, 20), at(20, 35), "open ground is evenly lit");
    }
}
