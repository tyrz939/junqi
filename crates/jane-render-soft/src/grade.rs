//! T0's grade (PRESENTATION.md §1.9, the `grade` row): the frame's `Post` drawn as T2 draws it,
//! so a soft frame is the same hour and mood as a wgpu one. T2 grades in linear light
//! (exposure, a soft shoulder, saturation, tint, a coloured lift) and writes sRGB; T0's pixels are
//! display values, so the grade is a table per channel built once a frame from the `Post`:
//! display to linear, exposure, shoulder, tint, lift, and back. Saturation is the one term that
//! mixes channels: a luma mix per pixel before the table. Integer throughout (a frame hashes the
//! same on every target).

use jane_present::Post;

use crate::blit::Target;

/// A display byte as linear light, of 65535 (the sRGB curve, `round(lin * 65535)`).
#[rustfmt::skip]
const S2L: [u16; 256] = [
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

/// The grade's per-channel tables and its saturation, built from a `Post`.
#[derive(Debug)]
pub struct Grade {
    lut: [[u8; 256]; 3],
    saturation: i32,
}

impl Grade {
    /// The tables for `post` (its bloom is T2's and not drawn here).
    pub fn new(post: &Post) -> Grade {
        let mut lut = [[0u8; 256]; 3];
        let exposure = u32::from(post.exposure);
        for (k, t) in lut.iter_mut().enumerate() {
            let tint = u32::from(post.tint[k]);
            let lift = u32::from(post.lift[k]) * 257;
            for (v, out) in t.iter_mut().enumerate() {
                let l = shoulder(u32::from(S2L[v]) * exposure / 128);
                let l = l * tint / 255;
                let dark = 65535 - l.min(65535);
                let l = l + lift * (dark * dark / 65535) / 65535;
                *out = to_display(l);
            }
        }
        Grade { lut, saturation: i32::from(post.saturation) }
    }

    /// Grades every px of `t`: returns px written.
    pub fn apply(&self, t: &mut Target<'_>) -> u64 {
        let s = self.saturation;
        let [lr, lg, lb] = &self.lut;
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
        for t in &g.lut {
            for (v, &o) in t.iter().enumerate() {
                // The shoulder bends the top only.
                assert!(o.abs_diff(v as u8) <= 1 || v > 225, "{v}: {o}");
            }
        }
    }

    #[test]
    fn a_blue_lift_raises_the_darks_blue_and_leaves_the_lights() {
        let g = Grade::new(&Post { lift: [3, 4, 12], ..Post::NONE });
        assert!(g.lut[2][0] > g.lut[0][0] + 20, "{} {}", g.lut[2][0], g.lut[0][0]);
        assert!(g.lut[2][200].abs_diff(200) <= 2);
    }
}
