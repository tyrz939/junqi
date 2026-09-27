//! T0's grade (PRESENTATION.md §1.9, the `grade` row): the frame's `Post` drawn as T2 draws it,
//! so a soft frame is the same hour and mood as a wgpu one. T2 grades in linear light
//! (exposure, a soft shoulder, saturation, tint, a coloured lift) and writes sRGB; T0's pixels are
//! display values, so the grade is a table per channel built once a frame from the `Post`:
//! display to linear, exposure, shoulder, tint, lift, and back. Saturation is the one term that
//! mixes channels: a luma mix per pixel before the table. Integer throughout (a frame hashes the
//! same on every target).

use jane_present::Post;
use jane_present::frame::SkyLook;

use crate::blit::Target;

/// A display byte as linear light, of 65535 (the sRGB curve, `round(lin * 65535)`).
#[rustfmt::skip]
pub(crate) const S2L: [u16; 256] = [
    0, 20, 40, 60, 80, 99, 119, 139, 159, 179, 199, 219, 241, 264, 288, 313,
    340, 367, 396, 427, 458, 491, 526, 562, 599, 637, 677, 718, 761, 805, 851, 898,
    947, 997, 1048, 1101, 1156, 1212, 1270, 1330, 1391, 1453, 1517, 1583, 1651, 1720, 1790, 1863,
    1937, 2013, 2090, 2170, 2250, 2333, 2418, 2504, 2592, 2681, 2773, 2866, 2961, 3058, 3157, 3258,
    3360, 3464, 3570, 3678, 3788, 3900, 4014, 4129, 4247, 4366, 4488, 4611, 4736, 4864, 4993, 5124,
    5257, 5392, 5530, 5669, 5810, 5953, 6099, 6246, 6395, 6547, 6700, 6856, 7014, 7174, 7335, 7500,
    7666, 7834, 8004, 8177, 8352, 8528, 8708, 8889, 9072, 9258, 9445, 9635, 9828, 10022, 10219, 10417,
    10619, 10822, 11028, 11235, 11446, 11658, 11873, 12090, 12309, 12530, 12754, 12980, 13209, 13440, 13673, 13909,
    14146, 14387, 14629, 14874, 15122, 15371, 15623, 15878, 16135, 16394, 16656, 16920, 17187, 17456, 17727, 18001,
    18277, 18556, 18837, 19121, 19407, 19696, 19987, 20281, 20577, 20876, 21177, 21481, 21787, 22096, 22407, 22721,
    23038, 23357, 23678, 24002, 24329, 24658, 24990, 25325, 25662, 26001, 26344, 26688, 27036, 27386, 27739, 28094,
    28452, 28813, 29176, 29542, 29911, 30282, 30656, 31033, 31412, 31794, 32179, 32567, 32957, 33350, 33745, 34143,
    34544, 34948, 35355, 35764, 36176, 36591, 37008, 37429, 37852, 38278, 38706, 39138, 39572, 40009, 40449, 40891,
    41337, 41785, 42236, 42690, 43147, 43606, 44069, 44534, 45002, 45473, 45947, 46423, 46903, 47385, 47871, 48359,
    48850, 49344, 49841, 50341, 50844, 51349, 51858, 52369, 52884, 53401, 53921, 54445, 54971, 55500, 56032, 56567,
    57105, 57646, 58190, 58737, 59287, 59840, 60396, 60955, 61517, 62082, 62650, 63221, 63795, 64372, 64952, 65535,
];

/// Linear light of 65535 back to the display byte whose linear value is nearest.
fn to_display(l: u32) -> u8 {
    let l = l.min(65535) as u16;
    let i = S2L.partition_point(|&v| v < l);
    if i == 0 {
        return 0;
    }
    if i >= 256 {
        return 255;
    }
    // Between S2L[i - 1] and S2L[i]: the nearer.
    if l - S2L[i - 1] <= S2L[i] - l { (i - 1) as u8 } else { i as u8 }
}

/// T2's shoulder (`post.wgsl`): linear up to the knee (0.78), then easing toward white, so the
/// art's colours hold below it. Of 65535; a rational curve for T2's exponential.
fn shoulder(x: u32) -> u32 {
    const KNEE: u32 = 51_118;
    const TOP: u32 = 65_535 - KNEE;
    if x <= KNEE {
        return x;
    }
    let over = x - KNEE;
    KNEE + over * TOP / (TOP + over)
}

/// The display byte nearest linear light `l` of 65535, by a table of 4096 steps (within a level
/// of `to_display`): for the glow and the bloom, added per px.
pub fn to_display_fast(l: u32) -> u8 {
    L2S[(l.min(65535) >> 4) as usize]
}

static L2S: [u8; 4096] = l2s();

const fn l2s() -> [u8; 4096] {
    let mut t = [0u8; 4096];
    let (mut i, mut v) = (0usize, 0usize);
    while i < 4096 {
        let l = (i * 16 + 8) as u16;
        while v < 255 && S2L[v + 1] <= l {
            v += 1;
        }
        t[i] = if v < 255 && S2L[v + 1] - l < l - S2L[v] { (v + 1) as u8 } else { v as u8 };
        i += 1;
    }
    t
}

/// Columns a band of the afterglow's tables spans.
const BAND: usize = 16;

/// The afterglow across the frame (T2's `fs_grade`, §1.9) at dusk and dawn, integer: per band
/// of columns, the multiply and the add it lays on each channel's linear light before the
/// exposure (of 65536 and of 65535); and toward the top of the frame, the horizon's air.
#[derive(Debug)]
struct Afterglow {
    /// Per band: the multiply (of 65536) and the add (of 65535) per channel.
    bands: Vec<([u32; 3], [u32; 3])>,
    /// The far edge's colour as a display value, and its weight at the top row, of 65536.
    far: ([i32; 3], u32),
}

impl Afterglow {
    fn new(sky: &SkyLook, (w, h): (u16, u16)) -> Option<Afterglow> {
        if sky.glow_amount == 0 || w == 0 || h == 0 {
            return None;
        }
        let gw = u64::from(sky.glow_amount) * 257;
        // The air's glow: its hue at the sky byte's chroma, `sqrt(c / m) * m` = `sqrt(c * m)`.
        let g = sky.glow.map(|c| u64::from(S2L[usize::from(c)]));
        let m = g[0].max(g[1]).max(g[2]).max(1);
        let air = g.map(|c| (c * m).isqrt());
        let am = air[0].max(air[1]).max(air[2]).max(1);
        let warm = air.map(|a| a * 65535 / am);
        let span = u64::from(w) * 6 / 5;
        let n = usize::from(w).div_ceil(BAND);
        let bands = (0..n)
            .map(|b| {
                let x = (b * BAND + BAND / 2) as i64;
                let off = (x - i64::from(sky.glow_x)).unsigned_abs().min(span);
                let toward = 65535 - off * 65535 / span.max(1);
                let k = gw * toward / 65535 * toward / 65535;
                let mul = [0, 1, 2].map(|c| {
                    // 1 + (0.8 + warm * 0.45 - 1) * k * 0.55, of 65536.
                    let lean = 52_429 + warm[c] as i64 * 29_491 / 65_536 - 65_536;
                    (65_536 + lean * (k as i64 * 36_045 / 65_536) / 65_536).max(0) as u32
                });
                let add = [0, 1, 2].map(|c| (air[c] * k / 65_536 * 1311 / 65_536) as u32);
                (mul, add)
            })
            .collect();
        let target = [0, 1, 2].map(|c| {
            let hz = u64::from(S2L[usize::from(sky.horizon[c])]);
            i32::from(to_display(((hz + air[c]) / 4) as u32))
        });
        // At the top row: 0.07 of the strength.
        Some(Afterglow { bands, far: (target, (gw * 4588 / 65_535) as u32) })
    }
}

/// The grade's per-channel tables and its saturation, built from a `Post`: one set of tables,
/// or at dusk and dawn one per band of columns with the afterglow in it.
#[derive(Debug)]
pub struct Grade {
    luts: Vec<[[u8; 256]; 3]>,
    saturation: i32,
    /// The far edge's pull toward the horizon: its display colour and top-row weight of 65536.
    far: Option<([i32; 3], u32)>,
}

impl Grade {
    /// The tables for `post` alone (no afterglow).
    pub fn new(post: &Post) -> Grade {
        Grade::with_sky(post, None, (0, 0))
    }

    /// The tables for `post` and, where the frame's sky has an afterglow, T2's afterglow term
    /// across a canvas `(w, h)`. The bloom is drawn before, by the glow (`glow.rs`).
    pub fn with_sky(post: &Post, sky: Option<&SkyLook>, canvas: (u16, u16)) -> Grade {
        let glow = sky.and_then(|s| Afterglow::new(s, canvas));
        let flat = [([65_536; 3], [0; 3])];
        let bands = glow.as_ref().map_or(&flat[..], |g| &g.bands[..]);
        let exposure = u32::from(post.exposure);
        // One set of tables is exact; a band's are by the fast curve (dusk builds fifty sets).
        let fast = bands.len() > 1;
        let luts = bands
            .iter()
            .map(|&(mul, add)| {
                let mut lut = [[0u8; 256]; 3];
                for (k, t) in lut.iter_mut().enumerate() {
                    let tint = u32::from(post.tint[k]);
                    let lift = u32::from(post.lift[k]) * 257;
                    for (v, out) in t.iter_mut().enumerate() {
                        let l = (u64::from(S2L[v]) * u64::from(mul[k]) / 65_536) as u32 + add[k];
                        let l = shoulder(l * exposure / 128);
                        let l = l * tint / 255;
                        let dark = 65535 - l.min(65535);
                        let l = l + lift * (dark * dark / 65535) / 65535;
                        *out = if fast { to_display_fast(l) } else { to_display(l) };
                    }
                }
                lut
            })
            .collect();
        Grade { luts, saturation: i32::from(post.saturation), far: glow.map(|g| g.far) }
    }

    /// Grades every px of `t`: returns px written.
    pub fn apply(&self, t: &mut Target<'_>) -> u64 {
        let s = self.saturation;
        if self.luts.len() == 1 && self.far.is_none() {
            let [lr, lg, lb] = &self.luts[0];
            for d in t.px.iter_mut() {
                let c = *d;
                let (r, g, b) = (((c >> 16) & 0xff) as i32, ((c >> 8) & 0xff) as i32, (c & 0xff) as i32);
                let (r, g, b) = if s == 128 {
                    (r, g, b)
                } else {
                    let y = (r * 54 + g * 183 + b * 19) >> 8;
                    let m = |v: i32| (y + (v - y) * s / 128).clamp(0, 255);
                    (m(r), m(g), m(b))
                };
                *d = 0xff00_0000
                    | u32::from(lr[r as usize]) << 16
                    | u32::from(lg[g as usize]) << 8
                    | u32::from(lb[b as usize]);
            }
            return t.px.len() as u64;
        }
        let w = t.w.max(1) as usize;
        let h = t.h.max(1) as u64;
        for (y, row) in t.px.chunks_exact_mut(w).enumerate() {
            // The far edge: weight of 256 by `(1 - y / h)^2`.
            let pull = self.far.map(|(c, top)| {
                let far = (h - (y as u64).min(h)) * 65_536 / h;
                (c, (far * far / 65_536 * u64::from(top) / 65_536 / 256) as i32)
            });
            for (band, seg) in row.chunks_mut(BAND).enumerate() {
                let [lr, lg, lb] = &self.luts[band.min(self.luts.len() - 1)];
                for d in seg {
                    let c = *d;
                    let (mut r, mut g, mut b) =
                        (((c >> 16) & 0xff) as i32, ((c >> 8) & 0xff) as i32, (c & 0xff) as i32);
                    if let Some((f, a)) = pull
                        && a > 0
                    {
                        r += (f[0] - r) * a / 256;
                        g += (f[1] - g) * a / 256;
                        b += (f[2] - b) * a / 256;
                    }
                    let (r, g, b) = if s == 128 {
                        (r, g, b)
                    } else {
                        let y = (r * 54 + g * 183 + b * 19) >> 8;
                        let m = |v: i32| (y + (v - y) * s / 128).clamp(0, 255);
                        (m(r), m(g), m(b))
                    };
                    *d = 0xff00_0000
                        | u32::from(lr[r as usize]) << 16
                        | u32::from(lg[g as usize]) << 8
                        | u32::from(lb[b as usize]);
                }
            }
        }
        t.px.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_grade_is_the_identity_and_the_curve_round_trips() {
        for v in 0..=255u8 {
            assert_eq!(to_display(u32::from(S2L[usize::from(v)])), v);
        }
        let g = Grade::new(&Post::NONE);
        for t in &g.luts[0] {
            for (v, &o) in t.iter().enumerate() {
                // The shoulder bends the top only.
                assert!(o.abs_diff(v as u8) <= 1 || v > 225, "{v}: {o}");
            }
        }
    }

    #[test]
    fn a_blue_lift_raises_the_darks_blue_and_leaves_the_lights() {
        let g = Grade::new(&Post { lift: [3, 4, 12], ..Post::NONE });
        assert!(g.luts[0][2][0] > g.luts[0][0][0] + 20, "{} {}", g.luts[0][2][0], g.luts[0][0][0]);
        assert!(g.luts[0][2][200].abs_diff(200) <= 2);
    }
}

#[cfg(test)]
mod afterglow {
    use super::*;

    #[test]
    fn the_fast_curve_is_within_a_level() {
        for l in (0..65_536u32).step_by(7) {
            assert!(to_display_fast(l).abs_diff(to_display(l)) <= 1, "{l}");
        }
    }

    #[test]
    fn the_afterglow_warms_the_side_toward_the_sun_and_leaves_the_rest() {
        let sky = SkyLook {
            zenith: [40, 60, 120],
            horizon: [200, 150, 110],
            glow: [255, 180, 90],
            glow_x: 0,
            glow_amount: 255,
            stars: 0,
            star_list: jane_present::Span::default(),
            moon: None,
            zone: (0, 0, 0, 0),
            tick: 0,
        };
        let g = Grade::with_sky(&Post::NONE, Some(&sky), (768, 432));
        let (near, far) = (&g.luts[0], &g.luts[g.luts.len() - 1]);
        // A grey px: warmer (red over blue) toward the glow, as it was far from it.
        assert!(near[0][100] > near[2][100] + 4, "{} {}", near[0][100], near[2][100]);
        assert!(far[0][100].abs_diff(far[2][100]) <= 1, "{} {}", far[0][100], far[2][100]);
    }
}
