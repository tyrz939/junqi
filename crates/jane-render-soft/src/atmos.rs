//! T0's atmosphere (PRESENTATION.md §1.3, §1.8, §1.9): the sky where the view meets the zone's
//! north edge, the far things on its horizon, the water's shimmer, one drift of the mist tile at
//! the strongest fog in view, and the particles. Integer, so a frame is the same bytes on every
//! target.

use jane_present::atmos::sky_at;
use jane_present::frame::{FogVolume, PartShape, Particle, SkyLook, StarCmd, WaterCmd};
use jane_present::{Flags, Page, SpriteCmd};

use crate::blit::{self, Target, lerp};

/// The mist tile's side.
const MIST: i32 = jane_art::weather::MIST_SIDE;

/// Packs `[r, g, b]` opaque.
fn pack(c: [u8; 3]) -> u32 {
    0xff00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])
}

/// The sky above the zone's top edge, if the view shows any: the backdrop row by row, dithered
/// between the rows' colours by the 4 x 4 Bayer matrix, and its stars. Returns px written.
pub fn sky(t: &mut Target<'_>, s: &SkyLook, stars: &[StarCmd]) -> u64 {
    let top = s.zone.1.clamp(0, t.h);
    if top <= 0 {
        return 0;
    }
    for y in 0..top {
        let up = top - y;
        let row = &mut t.px[(y * t.w) as usize..((y + 1) * t.w) as usize];
        // The afterglow varies across; the rest only by row, so a row is one colour and its
        // neighbour's, dithered.
        for (x, d) in row.iter_mut().enumerate() {
            let x = x as i32;
            let (a, b) = (sky_at(s, x, up), sky_at(s, x, up + 2));
            let k = u32::from(jane_art::canvas::bayer(x, y)) * 16;
            *d = lerp(pack(a), pack(b), k);
        }
    }
    for st in stars {
        let y = top - i32::from(st.up);
        if y >= 0 && y < top && (0..t.w).contains(&i32::from(st.x)) {
            let d = &mut t.px[(y * t.w + i32::from(st.x)) as usize];
            *d = lerp(*d, 0xfff0_f0ff, u32::from(st.bright) + 1);
        }
    }
    (top * t.w) as u64
}

/// The far things on the sky (`Parallax`): sprites standing on the horizon, which is the zone's
/// top edge; clipped to the sky above it.
pub fn parallax(t: &mut Target<'_>, s: &SkyLook, page: &Page, clut: &[u32], sp: &SpriteCmd) -> u64 {
    let top = s.zone.1.clamp(0, t.h);
    if top <= 0 {
        return 0;
    }
    let y = top + i32::from(sp.y);
    // Clip to the sky: draw into a target that ends at the horizon.
    let n = (top * t.w) as usize;
    let mut sky = Target { px: &mut t.px[..n], w: t.w, h: top };
    blit::sprite(&mut sky, page, clut, sp.src, i32::from(sp.x), y, Flags::default());
    u64::from(sp.src.w) * u64::from(sp.src.h)
}

/// The shimmer (§1.8): one bright glint a water cell, two px wide, walking by tick from a place
/// the cell's phase sets.
pub fn shimmer(t: &mut Target<'_>, cells: &[WaterCmd], tick: u32) -> u64 {
    let mut n = 0;
    for c in cells {
        let p = u32::from(c.phase);
        let step = (tick / 6 + p * 5) % 64;
        // A glint lives eight steps of the sixty-four, then the cell is calm a while.
        if step >= 40 {
            continue;
        }
        let gx = i32::from(c.x) + 2 + ((p * 7 + step / 5) % 12) as i32;
        let gy = i32::from(c.y) + 3 + ((p * 3 + step / 10) % 10) as i32;
        let lift = if step % 8 < 4 { 70 } else { 40 };
        for dx in 0..2 {
            let (x, y) = (gx + dx, gy);
            if x >= 0 && y >= 0 && x < t.w && y < t.h {
                let d = &mut t.px[(y * t.w + x) as usize];
                *d = lerp(*d, 0xffe8_f4ff, lift);
                n += 1;
            }
        }
    }
    n
}

/// One drift of the mist tile at the strongest fog volume in view (§1.9, T0), blended toward
/// its colour over its rect, fading in over its edge.
pub fn fog(t: &mut Target<'_>, vols: &[FogVolume], tile: &[u8], camera: (i32, i32), drift: (i16, i16)) -> u64 {
    let Some(v) = vols.iter().max_by_key(|v| v.density) else { return 0 };
    // Below this a fog is a tint T0 cannot afford to lay: it would cost the frame a full-screen
    // blend for a change of a few levels.
    if tile.len() != (MIST * MIST) as usize || v.density < 20 {
        return 0;
    }
    let (x0, y0) = (v.rect.0.max(0), v.rect.1.max(0));
    let (x1, y1) = (v.rect.2.min(t.w), v.rect.3.min(t.h));
    if x0 >= x1 || y0 >= y1 {
        return 0;
    }
    let c = pack(v.colour);
    let edge = i32::from(v.edge.max(1));
    let dens = u32::from(v.density);
    // The tile's alpha to blend weight, once: a base of haze under the wisps, so thick fog is
    // never holed.
    let mut weight = [0u32; 256];
    for (m, w) in weight.iter_mut().enumerate() {
        *w = ((m as u32 / 2 + 64) * dens / 255).min(230);
    }
    let tx0 = (x0 + camera.0 - i32::from(drift.0)).rem_euclid(MIST);
    for y in y0..y1 {
        let ey = (y - v.rect.1).min(v.rect.3 - 1 - y).clamp(0, edge);
        let ty = ((y + camera.1 - i32::from(drift.1)).rem_euclid(MIST) * MIST) as usize;
        let mist = &tile[ty..ty + MIST as usize];
        let row = &mut t.px[(y * t.w + x0) as usize..(y * t.w + x1) as usize];
        let mut tx = tx0 as usize;
        for (i, d) in row.iter_mut().enumerate() {
            let x = x0 + i as i32;
            let mut a = weight[usize::from(mist[tx])];
            let ex = (x - v.rect.0).min(v.rect.2 - 1 - x);
            if ex < edge || ey < edge {
                a = a * (ex.min(ey).clamp(0, edge) * 256 / edge) as u32 / 256;
            }
            *d = lerp(*d, c, a);
            tx = (tx + 1) & (MIST as usize - 1);
        }
    }
    ((x1 - x0) * (y1 - y0)) as u64
}

/// The particles (§2): strokes fading toward their tails, squares, rings squashed to the ground,
/// soft discs. Their colours are already lit (the presenter did it for this tier).
pub fn particles(t: &mut Target<'_>, parts: &[Particle]) -> u64 {
    let mut n = 0u64;
    let mut put = |x: i32, y: i32, c: u32, a: u32| {
        if x >= 0 && y >= 0 && x < t.w && y < t.h && a > 0 {
            let d = &mut t.px[(y * t.w + x) as usize];
            *d = lerp(*d, c, a.min(256));
            n += 1;
        }
    };
    for p in parts {
        let c = pack(p.colour);
        let a = u32::from(p.alpha) + 1;
        let (x, y) = (i32::from(p.x), i32::from(p.y));
        match p.shape {
            PartShape::Streak { dx, dy } => {
                let (tx, ty) = (x + i32::from(dx), y + i32::from(dy));
                let len = i32::from(dx).abs().max(i32::from(dy).abs()).max(1);
                let mut k = 0;
                jane_art::canvas::bresenham(x, y, tx, ty, |px, py| {
                    put(px, py, c, a * (len + 1 - k.min(len)) as u32 / (len + 1) as u32);
                    k += 1;
                });
            }
            PartShape::Dot { size } => {
                for yy in 0..i32::from(size.max(1)) {
                    for xx in 0..i32::from(size.max(1)) {
                        put(x + xx, y + yy, c, a);
                    }
                }
            }
            PartShape::Ring { r } => {
                let r = i32::from(r);
                // A ring on the ground: an ellipse half as tall as it is wide, one px thick.
                for yy in -r / 2 - 1..=r / 2 + 1 {
                    for xx in -r - 1..=r + 1 {
                        let d = xx * xx + 4 * yy * yy;
                        if d <= r * r + r && d >= r * r - 2 * r {
                            put(x + xx, y + yy, c, a);
                        }
                    }
                }
            }
            PartShape::Glow { r } => {
                let r = i32::from(r).max(1);
                for yy in -r..=r {
                    for xx in -r..=r {
                        let d2 = xx * xx + yy * yy;
                        if d2 <= r * r {
                            let k = (r * r - d2) * 256 / (r * r);
                            put(x + xx, y + yy, c, a * k as u32 / 256);
                        }
                    }
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::Span;

    fn look(top: i32) -> SkyLook {
        SkyLook {
            zenith: [10, 12, 40],
            horizon: [200, 120, 100],
            glow: [255, 120, 60],
            glow_x: 0,
            glow_amount: 0,
            stars: 255,
            star_list: Span::default(),
            moon: None,
            zone: (0, top, 64, 64),
            tick: 0,
        }
    }

    #[test]
    fn the_sky_shows_above_the_zone_and_nowhere_else() {
        let mut px = vec![0xff00_0000; 64 * 64];
        let mut t = Target { px: &mut px, w: 64, h: 64 };
        let n = sky(&mut t, &look(20), &[StarCmd { x: 3, up: 10, bright: 255 }]);
        assert_eq!(n, 20 * 64);
        assert_ne!(px[0], 0xff00_0000);
        assert_eq!(px[20 * 64], 0xff00_0000, "the zone is left to the terrain");
        // The horizon is warmer than the zenith.
        assert!((px[19 * 64 + 40] >> 16) & 0xff > (px[40] >> 16) & 0xff);
        // The star is bright.
        assert!(px[10 * 64 + 3] & 0xff > 200);
        let mut px2 = vec![0xff00_0000; 64 * 64];
        let mut t = Target { px: &mut px2, w: 64, h: 64 };
        assert_eq!(sky(&mut t, &look(0), &[]), 0);
    }

    #[test]
    fn a_ring_and_a_streak_draw_and_stay_on_the_canvas() {
        let mut px = vec![0xff00_0000; 32 * 32];
        let mut t = Target { px: &mut px, w: 32, h: 32 };
        let p = |x, y, shape| Particle { x, y, shape, colour: [255, 255, 255], alpha: 255, glow: 0, height: 0 };
        let n = particles(
            &mut t,
            &[p(16, 16, PartShape::Ring { r: 5 }), p(2, 2, PartShape::Streak { dx: -8, dy: -12 }), p(31, 31, PartShape::Glow { r: 4 })],
        );
        assert!(n > 20);
        assert_eq!(px[16 * 32 + 16], 0xff00_0000, "a ring is hollow");
        assert_ne!(px[16 * 32 + 21], 0xff00_0000);
    }
}
