//! The integer light pass: a canvas lit by one directional light over an ambient, with the
//! shadows its height layer casts (PRESENTATION.md §1.7). `jane sheet light` draws through it,
//! and it is the kernel the renderer's lighting reproduces: N·L with the light's elevation as
//! `z`, ambient first, the direct term removed where the height field occludes the light,
//! emissive added unlit after the multiply.
//!
//! Units: colours and ambient are per channel in 1/256ths (256 is full), angles are
//! jane-core [`Angle`]s (0 east, clockwise with y down), heights are screen px.

use alloc::vec;
use alloc::vec::Vec;
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
    let hf = Heights { w: c.w(), h: c.h(), z: c.heights() };
    let march = March::new(&hf, sun);
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
            let lam = if dot <= 0 || march.blocked(&hf, x, y) { 0 } else { dot.min(UNIT) };
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

/// A height field: px above the ground per pixel, row-major.
struct Heights<'a> {
    w: i32,
    h: i32,
    z: &'a [u8],
}

impl Heights<'_> {
    fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.z[(y * self.w + x) as usize]
    }
}

/// Light an upright sprite (a person: its height layer is each pixel's height above the feet on
/// row `ay`, `Canvas::upright`) the way a renderer should: every drawn pixel stands over the
/// ground point under it on the feet's row and throws its shadow `h / tan(elevation)` px away
/// from the sun, so the ground takes the silhouette's shadow, head and all, soft at its edge by
/// its cover of each pixel's 3 x 3. Drawn pixels are lit by their normals and never by the
/// ground's shadow. Returns RGB per pixel, row-major, as [`light`].
pub fn light_upright(c: &Canvas, ay: i32, sun: &Sun, ambient: [u16; 3], ground: Ground) -> Vec<[u8; 3]> {
    let l = sun.toward();
    let (ce, se) = (cos_q15(sun.elevation).0, sin_q15(sun.elevation).0);
    let (w, h) = (c.w(), c.h());
    let mut shade = vec![false; (w * h) as usize];
    if ce > 1700 && se > 0 {
        // Away from the sun, in 1/256 px per px of height.
        let (dx, dy) = (-(cos_q15(sun.azimuth).0 >> 7) * ce / se, -(sin_q15(sun.azimuth).0 >> 7) * ce / se);
        for y in 0..h {
            for x in 0..w {
                if !c.get(x, y).is_opaque() {
                    continue;
                }
                let z = i32::from(c.height_at(x, y));
                // A body is some px deep: it stands on the feet's row and the few behind it.
                for depth in 0..BODY_DEPTH {
                    let foot = (x * 256 + 128, (ay - depth) * 256 + 128);
                    // From this pixel's height to the next row's, so the shadow has no gaps.
                    let at = |z: i32| ((foot.0 + dx * z) >> 8, (foot.1 + dy * z) >> 8);
                    let (a, b) = (at(z), at(z + 2));
                    crate::canvas::bresenham(a.0, a.1, b.0, b.1, |sx, sy| {
                        if sx >= 0 && sy >= 0 && sx < w && sy < h {
                            shade[(sy * w + sx) as usize] = true;
                        }
                    });
                }
            }
        }
    }
    let cover = |x: i32, y: i32| -> i32 {
        let mut n = 0;
        for yy in y - 1..=y + 1 {
            for xx in x - 1..=x + 1 {
                if xx >= 0 && yy >= 0 && xx < w && yy < h && shade[(yy * w + xx) as usize] {
                    n += 1;
                }
            }
        }
        n
    };
    let mut out = Vec::with_capacity(c.albedo().len());
    for y in 0..h {
        for x in 0..w {
            let ix = c.get(x, y);
            let drawn = ix.is_opaque();
            let base =
                if drawn { rgb(ix) } else { crate::palette::ao(rgb(ground.ix), crate::palette::ao_cover(c, x, y)) };
            let [nx, ny, nz] = decode(c.normal_at(x, y));
            let dot = (nx * l[0] + ny * l[1] + nz * l[2]) >> 15;
            let mut lam = dot.clamp(0, UNIT);
            if !drawn {
                lam = lam * (9 - cover(x, y)) / 9;
            }
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

/// How deep an upright body stands on the ground, px: its shadow in a low side light is this
/// wide.
const BODY_DEPTH: i32 = 5;

/// A shadow ray toward the sun through the height field, in 1/256 px.
struct March {
    step: [i32; 2],
    rise: i32,
    top: i32,
    on: bool,
}

impl March {
    fn new(c: &Heights<'_>, sun: &Sun) -> March {
        let ce = cos_q15(sun.elevation).0;
        let se = sin_q15(sun.elevation).0;
        let top = i32::from(c.z.iter().copied().max().unwrap_or(0)) * 256;
        // Overhead (within about 3°) casts nothing.
        let on = ce > 1700 && se > 0;
        let rise = if on { se * 256 / ce } else { 0 };
        let step = [cos_q15(sun.azimuth).0 >> 7, sin_q15(sun.azimuth).0 >> 7];
        March { step, rise, top, on }
    }

    /// Whether something in the height field stands between `(x, y)` and the sun. A 1 px bias
    /// keeps a surface from shadowing itself on its own slope.
    fn blocked(&self, c: &Heights<'_>, x: i32, y: i32) -> bool {
        if !self.on {
            return false;
        }
        let (mut fx, mut fy) = (x * 256 + 128, y * 256 + 128);
        let mut z = i32::from(c.at(x, y)) * 256 + 256;
        loop {
            fx += self.step[0];
            fy += self.step[1];
            z += self.rise;
            let (px, py) = (fx >> 8, fy >> 8);
            if z > self.top || px < 0 || py < 0 || px >= c.w || py >= c.h {
                return false;
            }
            if i32::from(c.at(px, py)) * 256 > z {
                return true;
            }
        }
    }
}

/// A point light over a scene (PRESENTATION.md §1.7): a lamp, a window's spill, her lantern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    /// Where it hangs, scene px.
    pub x: i32,
    /// Where it hangs, scene px.
    pub y: i32,
    /// Its height above the ground, px.
    pub z: i32,
    /// Its colour per channel, 1/256ths.
    pub colour: [u16; 3],
    /// How far it reaches, px.
    pub radius: i32,
}

/// A whole scene's layers, resolved albedo as `0xAARRGGBB` (alpha 0 is clear), for
/// [`light_scene`]: a composed county window, a chunk.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// Size in px.
    pub w: i32,
    /// Size in px.
    pub h: i32,
    /// Resolved albedo.
    pub albedo: &'a [u32],
    /// Normals.
    pub normal: &'a [crate::canvas::Normal],
    /// Emissive palette indices; 0 is none.
    pub emissive: &'a [Ix],
    /// Px above the ground.
    pub height: &'a [u8],
}

/// Light a scene by `sun` over `ambient`, adding `points` with their falloff and N·L from their
/// height (no shadows from points: that is the renderer's T1 row) and a faint haze of their
/// colour in the air of the pool, and the emissive layer unlit after the multiply scaled by
/// `glow` (0..=256: how dark it is, so windows shine at night and not at noon). The same kernel as
/// [`light`], over resolved colour, carried in 1/4096ths so a pool's falloff shows no rings.
pub fn light_scene(s: &Scene<'_>, sun: &Sun, ambient: [u16; 3], points: &[Point], glow: u16) -> Vec<[u8; 3]> {
    /// The haze: this many 256ths of a lamp's colour, added where it falls, whatever it lands on.
    const HAZE: i64 = 22;
    /// How much of what a lamp lights it sees as grey, of 256: a warm lamp on green grass warms
    /// it toward the lamp's colour instead of multiplying it into lime.
    const WASH: i32 = 128;
    let l = sun.toward();
    let hf = Heights { w: s.w, h: s.h, z: s.height };
    let march = March::new(&hf, sun);
    let n = (s.w * s.h) as usize;
    // Light per pixel per channel in 1/4096ths of full, and the haze in 1/16ths of a level.
    let mut acc: Vec<[i32; 3]> = Vec::with_capacity(n);
    let mut haze: Vec<[i32; 3]> = vec![[0; 3]; n];
    let mut lamp: Vec<[i32; 3]> = vec![[0; 3]; n];
    for y in 0..s.h {
        for x in 0..s.w {
            let i = (y * s.w + x) as usize;
            let [nx, ny, nz] = decode(s.normal[i]);
            let dot = (nx * l[0] + ny * l[1] + nz * l[2]) >> 15;
            let lam = if dot <= 0 || march.blocked(&hf, x, y) { 0 } else { dot.min(UNIT) };
            acc.push([0, 1, 2].map(|k| (i32::from(ambient[k]) + i32::from(sun.colour[k]) * lam / UNIT) * 16));
        }
    }
    for p in points {
        let r2 = p.radius * p.radius;
        for y in (p.y - p.radius).max(0)..(p.y + p.radius + 1).min(s.h) {
            for x in (p.x - p.radius).max(0)..(p.x + p.radius + 1).min(s.w) {
                let (dx, dy) = (p.x - x, p.y - y);
                let d2 = dx * dx + dy * dy;
                if d2 >= r2 {
                    continue;
                }
                let i = (y * s.w + x) as usize;
                let dz = p.z - i32::from(s.height[i]);
                let len = jane_core::num::isqrt((d2 + dz * dz) as u64).max(1) as i64;
                let [nx, ny, nz] = decode(s.normal[i]);
                // N·L in 1/4096ths.
                let dot = (i64::from(nx * dx + ny * dy + nz * dz) * 4096 / (len * i64::from(UNIT))).clamp(0, 4096);
                // (1 - d²/r²)² in 1/65536ths.
                let t = i64::from(r2 - d2) * 65536 / i64::from(r2.max(1));
                let f = t * t / 65536;
                for k in 0..3 {
                    let c = i64::from(p.colour[k]);
                    lamp[i][k] += (c * 16 * f / 65536 * dot / 4096) as i32;
                    haze[i][k] += (c * 16 * f / 65536 * HAZE / 256) as i32;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = s.albedo[i];
        let base = [(a >> 16) as u8, (a >> 8) as u8, a as u8];
        let e = s.emissive[i];
        let g = if e == Ix::CLEAR { [0; 3] } else { rgb(e) };
        let luma = (299 * i32::from(base[0]) + 587 * i32::from(base[1]) + 114 * i32::from(base[2])) / 1000;
        let mut px = [0u8; 3];
        for k in 0..3 {
            let washed = (i32::from(base[k]) * (256 - WASH) + luma * WASH) >> 8;
            let v = (i32::from(base[k]) * acc[i][k] + washed * lamp[i][k]) >> 12;
            px[k] = (v + haze[i][k] / 16 + i32::from(g[k]) * i32::from(glow) / 256).clamp(0, 255) as u8;
        }
        out.push(px);
    }
    out
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
    fn an_upright_figure_casts_its_silhouette_from_its_feet() {
        // A 4 px wide post standing 20 rows tall on row 30, heights true (5 px a 4 rows).
        let mut c = Canvas::new(60, 40);
        c.fill_rect(Rect::new(28, 10, 4, 21), Ramp::Stone.at(crate::palette::Tone::Base), 1);
        c.upright(30);
        let ground = Ground { ix: Ramp::Grass.at(crate::palette::Tone::Base) };
        let sun = Sun { azimuth: Angle::WEST, elevation: Angle::from_degrees(45), colour: [256; 3] };
        let lit = light_upright(&c, 30, &sun, [32; 3], ground);
        let at = |x: i32, y: i32| lit[(y * 60 + x) as usize][1];
        // East of the feet, as long as the post is tall (25 px at 45°), and not west of it.
        assert!(at(45, 29) < at(15, 29), "the shadow lies east");
        assert!(at(54, 29) < at(15, 29), "and reaches the post's height away");
        assert_eq!(at(15, 29), at(15, 5), "open ground is evenly lit");
        assert!(at(59, 29) > at(45, 29), "and stops");
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
