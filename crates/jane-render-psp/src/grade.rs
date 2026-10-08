//! C2's grade (PRESENTATION.md §1.9, §1.12 the `grade` row): `soft`'s, term for term. T0 grades
//! with a table per channel (display to linear, the afterglow's multiply and add, the exposure,
//! T2's shoulder, the tint, the lift, and back) after a luma mix per px for the saturation, and
//! at dusk and dawn pulls the frame's top toward the horizon's air. The GE does the same: the
//! tables become CLUTs the frame is read back through (as `T32`, a channel a pass, each pass
//! writing its own channel alone), the saturation a mix with the frame's luma (made at half size
//! in the lightmap's target), the pull a smooth-shaded strip. These are `soft`'s integers
//! (`jane-render-soft/src/grade.rs`), so the two grades agree to a level; the afterglow's bands
//! are 32 columns here (16 on `soft`).

use alloc::vec::Vec;

use jane_present::Post;
use jane_present::frame::SkyLook;

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
pub(crate) fn to_display(l: u32) -> u8 {
    let l = l.min(65535) as u16;
    let i = S2L.partition_point(|&v| v < l);
    if i == 0 {
        return 0;
    }
    if i >= 256 {
        return 255;
    }
    if l - S2L[i - 1] <= S2L[i] - l { (i - 1) as u8 } else { i as u8 }
}

/// T2's shoulder: linear up to the knee (0.78), then easing toward white. Of 65535.
fn shoulder(x: u32) -> u32 {
    const KNEE: u32 = 51_118;
    const TOP: u32 = 65_535 - KNEE;
    if x <= KNEE {
        return x;
    }
    let over = x - KNEE;
    KNEE + over * TOP / (TOP + over)
}

/// The display byte nearest linear light `l`, by a table of 4096 steps (within a level).
fn to_display_fast(l: u32) -> u8 {
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

/// Columns a band of the afterglow's tables spans on C2.
pub const BAND: usize = 32;

/// A band's multiply (of 65536) and add (of 65535), per channel.
type Band = ([u32; 3], [u32; 3]);
/// The far edge's display colour and its top-row weight of 65536.
type Far = ([i32; 3], u32);

/// The afterglow across the frame at dusk and dawn (`soft`'s `Afterglow`): per band of columns
/// the multiply (of 65536) and the add (of 65535) on each channel's linear light before the
/// exposure; the far edge's display colour and its weight at the top row, of 65536.
fn afterglow(sky: &SkyLook, (w, h): (u16, u16)) -> Option<(Vec<Band>, Far)> {
    if sky.glow_amount == 0 || w == 0 || h == 0 {
        return None;
    }
    let gw = u64::from(sky.glow_amount) * 257;
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
    Some((bands, (target, (gw * 4588 / 65_535) as u32)))
}

/// What a frame's grade is built from: rebuilt only when it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    post: Post,
    glow: Option<([u8; 3], [u8; 3], i16, u8)>,
    canvas: (u16, u16),
}

/// The grade's tables for the GE, kept between frames.
#[derive(Debug, Default)]
pub struct Grade {
    key: Option<Key>,
    /// One set of three tables (red, green, blue), or one a band of [`BAND`] columns at dusk.
    pub luts: Vec<[[u8; 256]; 3]>,
    /// 128 is as lit.
    pub saturation: i32,
    /// The far edge's pull toward the horizon: its display colour and top-row weight of 65536.
    pub far: Option<Far>,
    /// Whether the tables change nothing (no grade pass needed but the saturation's).
    pub identity: bool,
}

impl Grade {
    /// Brings the tables up to `post` and the sky's afterglow over a canvas; true when they
    /// were rebuilt.
    pub fn update(&mut self, post: &Post, sky: Option<&SkyLook>, canvas: (u16, u16)) -> bool {
        let key = Key {
            post: *post,
            glow: sky.filter(|s| s.glow_amount > 0).map(|s| (s.glow, s.horizon, s.glow_x, s.glow_amount)),
            canvas,
        };
        if self.key == Some(key) {
            return false;
        }
        self.key = Some(key);
        let glow = sky.and_then(|s| afterglow(s, canvas));
        let flat = [([65_536u32; 3], [0u32; 3])];
        let bands = glow.as_ref().map_or(&flat[..], |g| &g.0[..]);
        let exposure = u32::from(post.exposure);
        let fast = bands.len() > 1;
        self.luts.clear();
        for &(mul, add) in bands {
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
            self.luts.push(lut);
        }
        self.saturation = i32::from(post.saturation);
        self.far = glow.map(|g| g.1);
        self.identity = self.luts.len() == 1
            && self.luts[0].iter().all(|t| t.iter().enumerate().all(|(v, &o)| usize::from(o) == v));
        true
    }

    /// The far pull's weight of 256 at row `y` of `h`: `(1 - y / h)^2` of the top row's.
    pub fn pull_at(top: u32, y: i32, h: i32) -> u32 {
        let h = i64::from(h.max(1));
        let far = (h - i64::from(y).clamp(0, h)) as u64 * 65_536 / h as u64;
        (far * far / 65_536 * u64::from(top) / 65_536 / 256) as u32
    }
}

/// The luma CLUTs: entry `v` of channel `c` is grey at `v` times the channel's weight (54, 183
/// and 19 of 256, `soft`'s), opaque; summed over the three, a px's luma. `0xAABBGGRR`.
pub fn luma_clut(c: usize) -> [u32; 256] {
    let w = [54u32, 183, 19][c];
    core::array::from_fn(|v| {
        let y = (v as u32 * w + 128) >> 8;
        0xff00_0000 | y << 16 | y << 8 | y
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_grade_is_nearly_the_identity_and_lumas_sum_to_white() {
        let mut g = Grade::default();
        assert!(g.update(&Post::NONE, None, (480, 272)));
        assert!(!g.update(&Post::NONE, None, (480, 272)), "kept while nothing changes");
        for t in &g.luts[0] {
            for (v, &o) in t.iter().enumerate() {
                assert!(o.abs_diff(v as u8) <= 1 || v > 225, "{v}: {o}");
            }
        }
        let y: u32 = (0..3).map(|c| luma_clut(c)[255] & 0xff).sum();
        assert!((254..=256).contains(&y), "{y}");
    }

    #[test]
    fn a_dusk_sky_makes_bands_warmer_toward_the_sun_and_a_far_pull() {
        let sky = SkyLook {
            zenith: [40, 50, 120],
            horizon: [230, 140, 90],
            glow: [255, 130, 60],
            glow_x: 0,
            glow_amount: 200,
            stars: 0,
            star_list: jane_present::Span::default(),
            moon: None,
            zone: (0, 0, 480, 272),
            tick: 0,
        };
        let mut g = Grade::default();
        g.update(&Post::NONE, Some(&sky), (480, 272));
        assert_eq!(g.luts.len(), 480 / BAND);
        let near = &g.luts[0];
        let far = &g.luts[g.luts.len() - 1];
        assert!(near[0][128] > far[0][128], "red lifted toward the sun");
        assert!(g.far.is_some_and(|f| f.1 > 0));
        assert!(Grade::pull_at(4000, 0, 272) > Grade::pull_at(4000, 136, 272));
        assert_eq!(Grade::pull_at(4000, 272, 272), 0);
    }
}
