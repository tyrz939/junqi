//! Atmosphere is art too (ART.md §2.8): the mist tile the fog volumes drift, the sky's hour
//! ramp and its stars, the moon in its phases, the School as a far silhouette, a far treeline per
//! region, and the looks of rain. The runtime that moves them is the presenter's weather pass
//! (PRESENTATION.md §1.9); every piece here is a function of a seed, with no hand grid.
//!
//! Integer throughout, like the rest of the crate.

use jane_core::grid::Rect;
use jane_data::Region;

use crate::canvas::Canvas;
use crate::hash::{below, h32};
use crate::palette::{Ramp, Tone};

/// Salts, one per use.
mod salt {
    pub const MIST: u32 = 0x4d49_5354;
    pub const STARS: u32 = 0x5354_4152;
    pub const TREES: u32 = 0x5452_4545;
    pub const SCHOOL: u32 = 0x5343_484c;
}

/// The mist tile's side, px (PRESENTATION.md §1.4: 8-bit alpha, 64 KB).
pub const MIST_SIDE: i32 = 256;

/// The mist tile: an 8-bit alpha mask `MIST_SIDE` square, seamless at its edges (its left column
/// runs on into its right, its top row into its bottom), built from 26 soft blobs stretched
/// along the wind and a finer wisp over them. The fog volumes drift two layers of it.
pub fn mist_tile(seed: u32) -> Vec<u8> {
    let n = MIST_SIDE as usize;
    let mut acc = vec![0u32; n * n];
    for i in 0..26u32 {
        let h = h32(seed, i, salt::MIST);
        let (cx, cy) = (below(h, 256) as i32, below(h.rotate_right(8), 256) as i32);
        let r = 26 + below(h32(i, seed, salt::MIST), 44) as i32;
        let (rx, ry) = (r * 3 / 2, r * 3 / 4 + 4);
        let s = 60 + below(h >> 3, 90);
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                // (dx/rx)^2 + (dy/ry)^2 in 1/4096ths.
                let d = (dx * dx * 4096) / (rx * rx) + (dy * dy * 4096) / (ry * ry);
                if d >= 4096 {
                    continue;
                }
                let f = (4096 - d) as u32;
                let k = f * f / 4096;
                let (x, y) = ((cx + dx).rem_euclid(MIST_SIDE), (cy + dy).rem_euclid(MIST_SIDE));
                acc[(y * MIST_SIDE + x) as usize] += s * k / 16;
            }
        }
    }
    // A finer wisp: value noise on a 16 px lattice, wrapped.
    let lattice = |x: i32, y: i32| below(h32(x.rem_euclid(16) as u32, y.rem_euclid(16) as u32, seed ^ salt::MIST), 256);
    let mut out = vec![0u8; n * n];
    // The blobs carry the shape and the wisp breaks their edges up; then the whole is stretched
    // so a fifth of it is clear and a fifth thick, by where those fall in this tile.
    let mut v = vec![0u32; n * n];
    for y in 0..MIST_SIDE {
        for x in 0..MIST_SIDE {
            let (gx, gy, fx, fy) = (x / 16, y / 16, (x % 16) as u32, (y % 16) as u32);
            let (a, b) = (lattice(gx, gy), lattice(gx + 1, gy));
            let (c, d) = (lattice(gx, gy + 1), lattice(gx + 1, gy + 1));
            let top = a * (16 - fx) + b * fx;
            let bottom = c * (16 - fx) + d * fx;
            let wisp = (top * (16 - fy) + bottom * fy) / 256;
            let i = (y * MIST_SIDE + x) as usize;
            v[i] = acc[i] / 16 * (160 + wisp / 2) / 256;
        }
    }
    let mut sorted = v.clone();
    sorted.sort();
    let (lo, hi) = (sorted[n * n / 5], sorted[n * n * 4 / 5].max(sorted[n * n / 5] + 1));
    for (o, &x) in out.iter_mut().zip(&v) {
        let t = (x.saturating_sub(lo) * 255 / (hi - lo)).min(255);
        // Smoothstep: thin mist fades softly to nothing, thick mist rolls over.
        *o = (t * t * (765 - 2 * t) / (255 * 255)) as u8;
    }
    out
}

/// The sky's colours at one moment (ART.md §2.8 `Sky`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkyColours {
    /// At the zenith.
    pub zenith: [u8; 3],
    /// At the horizon.
    pub horizon: [u8; 3],
    /// Low in the west after sunset (east at dawn): the afterglow.
    pub glow: [u8; 3],
}

/// Ticks in an hour of the clock.
const HOUR: i32 = jane_core::num::TICKS_PER_HOUR as i32;

/// The hour ramp: twelve keyframes, `(tick of the day, zenith, horizon, glow)`, linear between.
/// Night is deep blue, never black; dusk runs gold to amber low in the west under a blue zenith
/// (golden to blue, never rose to violet); dawn is pale rose.
/// A keyframe of the sky: tick of the day, zenith, horizon, afterglow.
type SkyKey = (i32, [i32; 3], [i32; 3], [i32; 3]);

const SKY: [SkyKey; 13] = [
    (0, [12, 16, 38], [30, 36, 66], [30, 36, 66]),
    (HOUR * 9 / 2, [14, 18, 42], [36, 40, 72], [40, 40, 76]),
    (HOUR * 11 / 2, [42, 50, 96], [150, 116, 132], [214, 142, 124]),
    (HOUR * 13 / 2, [88, 118, 172], [236, 196, 164], [250, 186, 136]),
    (HOUR * 8, [98, 150, 210], [196, 214, 226], [214, 220, 222]),
    (HOUR * 12, [84, 140, 214], [186, 210, 230], [196, 214, 230]),
    (HOUR * 16, [92, 140, 204], [210, 208, 200], [226, 200, 170]),
    (HOUR * 35 / 2, [92, 122, 180], [248, 196, 128], [255, 190, 100]),
    (HOUR * 37 / 2, [50, 76, 142], [246, 176, 100], [255, 172, 84]),
    (HOUR * 77 / 4, [28, 46, 102], [200, 140, 96], [236, 150, 80]),
    (HOUR * 81 / 4, [22, 26, 62], [60, 62, 108], [120, 80, 78]),
    (HOUR * 43 / 2, [12, 16, 38], [30, 36, 66], [34, 36, 70]),
    (HOUR * 24, [12, 16, 38], [30, 36, 66], [30, 36, 66]),
];

/// The sky at `clock` ticks since midnight.
pub fn sky(clock: u32) -> SkyColours {
    let t = (clock % (24 * HOUR as u32)) as i32;
    let lerp = |a: [i32; 3], b: [i32; 3], n: i32, d: i32| [0, 1, 2].map(|k| (a[k] + (b[k] - a[k]) * n / d) as u8);
    for w in SKY.windows(2) {
        let ((t0, z0, h0, g0), (t1, z1, h1, g1)) = (w[0], w[1]);
        if t >= t0 && t <= t1 {
            let (n, d) = (t - t0, (t1 - t0).max(1));
            return SkyColours { zenith: lerp(z0, z1, n, d), horizon: lerp(h0, h1, n, d), glow: lerp(g0, g1, n, d) };
        }
    }
    SkyColours { zenith: [12, 16, 38], horizon: [30, 36, 66], glow: [30, 36, 66] }
}

/// One star on the sky backdrop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Star {
    /// Across a band 1024 px wide the canvas wraps over.
    pub x: u16,
    /// Px above the horizon.
    pub up: u8,
    /// 0..=255.
    pub bright: u8,
    /// One in eight twinkle by tick (ART.md §2.8).
    pub twinkles: bool,
}

/// The sky's 120 stars, hashed: fewer near the horizon, where the haze is.
pub fn stars(seed: u32) -> Vec<Star> {
    (0..120u32)
        .map(|i| {
            let h = h32(seed, i, salt::STARS);
            let up = 24 + below(below(h >> 10, 216) * below(h >> 18, 216), 216) as u8;
            Star {
                x: below(h, 1024) as u16,
                up: 239 - up.min(215),
                bright: 90 + below(h32(i, seed, salt::STARS), 166) as u8,
                twinkles: h.trailing_zeros() >= 3,
            }
        })
        .collect()
}

/// The moon in phase `phase` (0 new, 4 full, 7 a thin waning crescent; eight a month), 15 px
/// square: a lit disc of bone with its dark side cut by the terminator, emitting.
pub fn moon(phase: u8) -> Canvas {
    let r = 6;
    let mut c = Canvas::new(2 * r + 3, 2 * r + 3);
    c.set_emitting(true);
    let o = r + 1;
    // The terminator's x across the disc, in 1/8ths of r: waxing lights the right, waning the left.
    let p = i32::from(phase % 8);
    for y in -r..=r {
        for x in -r..=r {
            let d2 = x * x + y * y;
            if d2 > r * r + r / 2 {
                continue;
            }
            let half = (jane_core::num::isqrt((r * r - y * y) as u64) as i32).max(1);
            // -1..1 of the half-chord, in eighths.
            let u = x * 8 / half;
            let lit = match p {
                0 => false,
                1..=3 => u > 8 - 4 * p,
                4 => true,
                _ => u < 4 * (8 - p) - 8,
            };
            let tone = if !lit {
                if p == 0 && d2 >= (r - 1) * (r - 1) { Tone::Shade } else { continue }
            } else if d2 >= (r - 1) * (r - 1) {
                Tone::Light
            } else if (x + 2 * y) % 5 == 0 && d2 < (r - 2) * (r - 2) && (x - 1) * (x - 1) + (y + 1) * (y + 1) < 8 {
                Tone::Base
            } else {
                Tone::High
            };
            c.dot(o + x, o + y, Ramp::Bone.at(tone), 1);
        }
    }
    c
}

/// The School as a far silhouette (ART.md §2.8; PLAN.md §2.2): `house(silhouette, storeys 3,
/// steeple)` in the distance `band` (0 far, 1 middle, 2 near), its windows dark but for `lit`
/// of them (one steady, then one that flickers), emitting. The foot is its bottom row.
pub fn school(band: u8, lit: u8) -> Canvas {
    // A unit is 2, 3 or 4 px.
    let u = 3 + i32::from(band.min(2));
    let (w, h) = (40 * u, 30 * u);
    let mut c = Canvas::new(w, h);
    let body = Ramp::Slate.at(Tone::Deep);
    let rim = Ramp::Slate.at(Tone::Shade);
    let dark = Ramp::WallDark.at(Tone::Deep);
    let base = h - 1;
    let r = |x: i32, y: i32, ww: i32, hh: i32| Rect::new(x * u, base - (y + hh) * u + 1, ww * u, hh * u);
    // The main block, three storeys, and its roof pitched to a ridge.
    c.fill_rect(r(8, 0, 24, 11), body, 10);
    c.polyline_fill(&[(8 * u, base - 11 * u), (20 * u, base - 16 * u), (32 * u, base - 11 * u)], body, 10);
    // Two lower wings.
    c.fill_rect(r(1, 0, 8, 7), body, 8);
    c.polyline_fill(&[(u, base - 7 * u), (5 * u, base - 10 * u), (9 * u, base - 7 * u)], body, 8);
    c.fill_rect(r(31, 0, 8, 7), body, 8);
    c.polyline_fill(&[(31 * u, base - 7 * u), (35 * u, base - 10 * u), (39 * u, base - 7 * u)], body, 8);
    // The bell tower left of the ridge, its belfry open, a steeple over it.
    c.fill_rect(r(12, 11, 5, 9), body, 12);
    c.fill_rect(r(13, 17, 3, 2), dark, 12);
    c.polyline_fill(&[(12 * u, base - 20 * u), (14 * u + u / 2, base - 28 * u), (17 * u, base - 20 * u)], body, 12);
    // Chimneys on the ridge.
    c.fill_rect(r(24, 12, 1, 5), body, 10);
    c.fill_rect(r(28, 11, 1, 4), body, 10);
    // The sky's light along the roof lines and the steeple: the silhouette's lit edge.
    let edge = |c: &mut Canvas, a: (i32, i32), b: (i32, i32)| c.line(a, b, rim, 1, 10);
    edge(&mut c, (8 * u, base - 11 * u), (20 * u, base - 16 * u));
    edge(&mut c, (12 * u, base - 20 * u), (14 * u + u / 2, base - 28 * u));
    edge(&mut c, (u, base - 7 * u), (5 * u, base - 10 * u));
    edge(&mut c, (31 * u, base - 7 * u), (35 * u, base - 10 * u));
    // Windows: three rows on the block, one on each wing; dark glass, `lit` of them glowing.
    let mut n = 0u8;
    let order = [(1, 5), (2, 2), (0, 7), (2, 6)];
    for row in 0..3 {
        for col in 0..9 {
            let (x, y) = (10 + col * 5 / 2, 1 + row * 3 + row / 2);
            let pick = order.iter().position(|&(rr, cc)| rr == row && cc == col);
            let glow = pick.is_some_and(|k| (k as u8) < lit);
            let win = Rect::new(x * u, base - (y + 2) * u + 1, u.max(2) - u / 4, 2 * u - u / 3);
            if glow {
                c.set_emitting(true);
                c.fill_rect(win, Ramp::GlassLit.at(Tone::Light), 10);
                c.dot(win.x, win.y, Ramp::GlassLit.at(Tone::High), 10);
                c.set_emitting(false);
                n += 1;
            } else if h32(row as u32, col as u32, salt::SCHOOL) % 3 != 0 {
                c.fill_rect(win, dark, 10);
            }
        }
    }
    let _ = n;
    c
}

/// A far treeline for `region` (ART.md §2.8): a silhouette strip 256 px wide that tiles across,
/// hashed crowns in the region's deepest green on a ragged top, the Works' chimney stacks among
/// its thinner trees. The foot is its bottom row.
pub fn treeline(region: Region, seed: u32) -> Canvas {
    let (w, h) = (256, 30);
    let mut c = Canvas::new(w, h);
    let (ramp, stacks) = match region {
        Region::Lowfields => (Ramp::LeafDeep, 0),
        Region::Waters => (Ramp::Needle, 0),
        Region::Works => (Ramp::Works, 3),
    };
    let body = ramp.at(Tone::Deep);
    let rim = ramp.at(Tone::Shade);
    c.fill_rect(Rect::new(0, h - 6, w, 6), body, 4);
    let mut x = 0;
    let mut i = 0u32;
    while x < w + 12 {
        let hh = h32(seed, i, salt::TREES);
        let r = 4 + below(hh, 6) as i32;
        let top = h - 6 - below(hh >> 8, 10) as i32;
        let cx = x;
        // A crown: a disc, and a second smaller one on it; wrapped so the strip tiles.
        for (dx, dy, rr) in [(0, 0, r), (r / 3, -r / 2, r * 2 / 3)] {
            for yy in -rr..=rr {
                for xx in -rr..=rr {
                    if xx * xx + yy * yy <= rr * rr {
                        let px = (cx + dx + xx).rem_euclid(w);
                        let py = top + dy + yy;
                        if py >= 0 && py < h {
                            c.dot(px, py, body, 4);
                        }
                    }
                }
            }
        }
        // The top edge catches the sky.
        c.dot((cx + r / 3).rem_euclid(w), (top - r / 2 - r * 2 / 3).max(0), rim, 4);
        c.fill_rect(Rect::new(cx.rem_euclid(w), top, 1, h - top), body, 4);
        x += r + below(hh >> 16, 6) as i32 + 3;
        i += 1;
    }
    for k in 0..stacks {
        let sx = 40 + k * 80 + below(h32(seed, k as u32, salt::TREES), 30) as i32;
        let top = 2 + below(h32(k as u32, seed, salt::TREES), 8) as i32;
        c.fill_rect(Rect::new(sx, top, 4, h - top), body, 6);
        c.fill_rect(Rect::new(sx - 1, top, 6, 2), rim, 6);
    }
    c
}

/// How rain looks (ART.md §2.8): a drop is a stroke of `len` px in two greys lit by the sky, a
/// splash a ring of three frames, a ripple a ring that widens on water.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RainLook {
    /// The drop's head, before light.
    pub head: [u8; 3],
    /// The drop's tail.
    pub tail: [u8; 3],
    /// Stroke length range, px (storm draws them longer, the wind leans them).
    pub len: (u8, u8),
    /// The splash on the ground.
    pub splash: [u8; 3],
    /// The ripple on water.
    pub ripple: [u8; 3],
}

/// The rain of a region: the Works' is sooty, the Waters' cold.
pub fn rain(region: Region) -> RainLook {
    let (head, tail) = match region {
        Region::Lowfields => ([196, 206, 222], [124, 138, 160]),
        Region::Waters => ([186, 208, 222], [112, 140, 160]),
        Region::Works => ([180, 180, 186], [118, 116, 122]),
    };
    RainLook { head, tail, len: (6, 12), splash: [200, 210, 224], ripple: [176, 200, 222] }
}

/// The mist's colour by region (ART.md §2.8; WORLD.md §5.2: in the Waters mist is fog, in the
/// Works it is smoke).
pub fn mist_colour(region: Region) -> [u8; 3] {
    match region {
        Region::Lowfields => [206, 212, 222],
        Region::Waters => [196, 214, 220],
        Region::Works => [170, 158, 146],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mist_tile_is_seamless_and_has_both_thick_and_clear() {
        let m = mist_tile(7);
        let s = MIST_SIDE as usize;
        assert_eq!(m.len(), s * s);
        // Opposite edges run on into each other: no seam steps more than a smooth slope would.
        for i in 0..s {
            let (l, r) = (i32::from(m[i * s]), i32::from(m[i * s + s - 1]));
            let (t, b) = (i32::from(m[i]), i32::from(m[(s - 1) * s + i]));
            assert!((l - r).abs() < 24 && (t - b).abs() < 24, "row/col {i}: {l} {r} {t} {b}");
        }
        let thick = m.iter().filter(|&&v| v > 160).count();
        let clear = m.iter().filter(|&&v| v < 40).count();
        assert!(thick > s * s / 20 && clear > s * s / 20, "thick {thick}, clear {clear}");
    }

    #[test]
    fn night_is_deep_blue_dusk_is_warm_low_and_noon_is_blue() {
        let night = sky(0);
        assert!(night.zenith[2] > night.zenith[0] + 15 && night.zenith[2] > 30, "{night:?}");
        let dusk = sky((18 * HOUR + HOUR / 2) as u32);
        assert!(dusk.horizon[0] > dusk.horizon[2] + 60, "{dusk:?}");
        assert!(dusk.zenith[2] > dusk.zenith[0], "{dusk:?}");
        let noon = sky((12 * HOUR) as u32);
        assert!(noon.zenith[2] > 200 && noon.horizon[2] > noon.zenith[2], "{noon:?}");
    }

    #[test]
    fn every_parallax_piece_draws_pixels() {
        for band in 0..3 {
            for lit in 0..3 {
                let s = school(band, lit);
                let px = s.albedo().iter().filter(|a| a.is_opaque()).count();
                assert!(px > 400, "band {band}: {px}");
                let glow = s.emissive().iter().filter(|a| a.is_opaque()).count();
                assert_eq!(glow > 0, lit > 0, "band {band} lit {lit}");
            }
        }
        for r in [Region::Lowfields, Region::Waters, Region::Works] {
            let t = treeline(r, 3);
            // Its two ends meet: the strip tiles.
            let col = |x: i32| (0..t.h()).filter(|&y| t.get(x, y).is_opaque()).count() as i32;
            assert!((col(0) - col(t.w() - 1)).abs() < 12, "{r:?}");
            assert!(t.albedo().iter().filter(|a| a.is_opaque()).count() > 2000);
        }
        for p in 0..8 {
            let m = moon(p);
            let lit = m.albedo().iter().filter(|a| a.is_opaque()).count();
            assert!(lit > 0, "phase {p}");
        }
        assert!(
            moon(4).albedo().iter().filter(|a| a.is_opaque()).count()
                > moon(2).albedo().iter().filter(|a| a.is_opaque()).count()
        );
        assert_eq!(stars(1).len(), 120);
        assert!(stars(1).iter().all(|s| s.up <= 239 && s.x < 1024));
    }
}
