//! Silhouette sun shadows (PRESENTATION.md §1.3 `silhouettes`, §1.7): each caster's bands
//! (`jane_present::shadow`: its foot, then its rows sheared along the sun by their true height)
//! gathered in a coverage mask, the strongest kept so two shadows never darken twice, and applied
//! once, by a multiply toward the ambient's shade, its edge one dither step soft. Each mask px also
//! keeps how high the shadow over it reaches, so a wall's face or a roof takes its shadow from the
//! ground under it, and only as high as the shadow climbs. Integer only, like every pixel `soft`
//! writes.

use jane_present::shadow::{self, Band};
use jane_present::{Caster, Page, Rgb, SpriteCmd};

use crate::blit::Target;

/// The 4 x 4 ordered dither (thresholds 0..16), for the soft edge.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// Rows over the mask's box a lifted receiver may stand and still take its shadow from inside it:
/// the tallest terrain's `rows_up`.
const UP: i32 = jane_present::rows_up(100);

pub use jane_present::shadow::shear;

/// The mask the shadows are gathered in, the canvas's size, and the box it was written in.
#[derive(Debug, Default)]
pub struct Mask {
    px: Vec<u8>,
    reach: Vec<u8>,
    w: i32,
    h: i32,
    dirty: Option<(i32, i32, i32, i32)>,
    rows: Vec<(i32, i32, i32)>,
}

impl Mask {
    pub fn fit(&mut self, w: i32, h: i32) {
        let n = (w * h) as usize;
        if self.px.len() != n {
            self.px.clear();
            self.px.resize(n, 0);
            self.reach.clear();
            self.reach.resize(n, 0);
        }
        (self.w, self.h) = (w, h);
    }

    /// Covers a band's box at its strength and reach, the strongest and the highest kept.
    fn band(&mut self, b: Band) {
        let (x0, x1, y0, y1) = (b.x0.max(0), b.x1.min(self.w), b.y0.max(0), b.y1.min(self.h));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let (a, z) = ((y * self.w + x0) as usize, (y * self.w + x1) as usize);
            for m in &mut self.px[a..z] {
                *m = (*m).max(b.strength);
            }
            for r in &mut self.reach[a..z] {
                *r = (*r).max(b.reach);
            }
        }
        self.dirty = Some(match self.dirty {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
}

/// Lays one caster's shadow into `mask`, reading its silhouette from `page`.
pub fn cast(mask: &mut Mask, page: &Page, s: &SpriteCmd, c: &Caster, k: (i32, i32)) {
    let mut rows = std::mem::take(&mut mask.rows);
    rows.clear();
    shadow::rows(&page.albedo, page.w, s, i32::from(c.foot.1), &mut rows);
    shadow::bands(&rows, i32::from(s.x), c, k, |b| mask.band(b));
    mask.rows = rows;
}

/// Applies the mask to `t` and clears it. `heights` is the terrain's height under each px of `t`
/// (empty: all ground). A px takes the mask at the ground under it (`shadow::ground_of`) where the
/// shadow there reaches its height: a covered px goes toward `dst * shade` by its strength, and
/// its edge is `feather` px wider than one dither step (`shadow::feather`: 0 under a high clear
/// sun, more as it sinks or clouds over). A covered px `k` px in from the edge (along a row or a
/// column, `k` up to `feather + 1`) takes `8 - 3 (feather + 2 - k) / (feather + 1)` eighths of
/// it, and a px `k` px outside takes `3 (feather + 2 - k) / (feather + 1)` eighths where the 4 x 4
/// ordered dither is under `8 (feather + 2 - k) / (feather + 1)`: with no feather, five eighths
/// on the edge and three on the odd squares of the ring outside, one dither step soft, and a
/// post's thin shadow keeps its body. Returns pixels written.
pub fn apply(t: &mut Target<'_>, mask: &mut Mask, shade: Rgb, heights: &[u8], feather: i32) -> u64 {
    let Some((x0, y0, x1, y1)) = mask.dirty.take() else { return 0 };
    let (w, h) = (mask.w, mask.h);
    let (px, reach) = (&mask.px, &mask.reach);
    let at = |x: i32, y: i32, need: u8| {
        if x < 0 || y < 0 || x >= w || y >= h {
            return 0;
        }
        let i = (y * w + x) as usize;
        if reach[i] >= need { i32::from(px[i]) } else { 0 }
    };
    let f = feather.clamp(0, 3);
    let mut n = 0;
    let [sr, sg, sb] = shade.map(|c| 256 - i32::from(c) - i32::from(c >> 7));
    for y in (y0 - 1 - f - UP).max(0)..(y1 + 1 + f).min(h) {
        for x in (x0 - 1 - f).max(0)..(x1 + 1 + f).min(w) {
            let lift = heights.get((y * t.w + x) as usize).copied().unwrap_or(0);
            let (gy, need) = shadow::ground_of(y, lift);
            if gy > y1 + f || gy < y0 - 1 - f {
                continue;
            }
            let m = at(x, gy, need);
            let ring = |k: i32| [(x - k, gy), (x + k, gy), (x, gy - k), (x, gy + k)].map(|(a, b)| at(a, b, need));
            let s = if m > 0 {
                match (1..=f + 1).find(|&k| ring(k).contains(&0)) {
                    Some(k) => m * (8 - 3 * (f + 2 - k) / (f + 1)) / 8,
                    None => m,
                }
            } else {
                let Some((k, most)) =
                    (1..=f + 1).find_map(|k| Some((k, ring(k).into_iter().max().unwrap_or(0))).filter(|v| v.1 > 0))
                else {
                    continue;
                };
                if i32::from(BAYER4[(y & 3) as usize][(x & 3) as usize]) >= 8 * (f + 2 - k) / (f + 1) {
                    continue;
                }
                most * (3 * (f + 2 - k) / (f + 1)) / 8
            };
            let s = s + (s >> 7);
            let d = &mut t.px[(y * t.w + x) as usize];
            let c = *d;
            let ch = |shift: u32, k: i32| {
                let v = ((c >> shift) & 0xff) as i32;
                ((v * (256 - ((k * s) >> 8))) >> 8) as u32
            };
            *d = 0xff00_0000 | ch(16, sr) << 16 | ch(8, sg) << 8 | ch(0, sb);
            n += 1;
        }
    }
    // Cleared once every row is laid: a row cleared as it went would read as clear to the row
    // under it, and every px of every shadow would take the edge's dither.
    for y in y0..y1 {
        let (a, z) = ((y * w + x0) as usize, (y * w + x1) as usize);
        mask.px[a..z].fill(0);
        mask.reach[a..z].fill(0);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_core::Angle;
    use jane_present::{Directional, Flags, Src};

    fn sun(azimuth: Angle, deg: i32) -> Directional {
        Directional { azimuth, elevation: Angle::from_degrees(deg), colour: [255; 3], spread: 0, strength: 255 }
    }

    /// A 2 x 10 post of index 2 standing on row 10 of a 40 x 20 canvas.
    fn post() -> (Page, SpriteCmd, Caster) {
        let page = Page { w: 2, h: 10, albedo: vec![2; 20], ..Page::default() };
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 2, h: 10 },
            x: 4,
            y: 0,
            flags: Flags::default(),
            height_px: 12,
        };
        (page, s, Caster { sprite: 0, foot: (5, 10), height: 12, depth: 2 })
    }

    #[test]
    fn a_post_casts_a_band_that_starts_at_its_foot_and_is_applied_once() {
        let (page, s, c) = post();
        let mut mask = Mask::default();
        mask.fit(40, 20);
        let k = shear(&sun(Angle::WEST, 20)).unwrap();
        cast(&mut mask, &page, &s, &c, k);
        // Twice over the same ground: still one shadow.
        cast(&mut mask, &page, &s, &c, k);
        let mut px = vec![0xff80_8080u32; 800];
        let mut t = Target { px: &mut px, w: 40, h: 20 };
        let n = apply(&mut t, &mut mask, [128, 128, 200], &[], 0);
        assert!(n > 20, "{n}");
        // East of the post on its foot row is shadowed, and bluer than it is red; well west is not.
        let east = px[10 * 40 + 14];
        assert!(east & 0xff > (east >> 16) & 0xff && east != 0xff80_8080, "{east:08x}");
        assert_eq!(px[10 * 40], 0xff80_8080);
        // Right beside the foot, either side, the foot's shadow grounds it.
        assert_ne!(px[11 * 40 + 3], 0xff80_8080);
        assert_ne!(px[11 * 40 + 6], 0xff80_8080);
        // The mask is clear for the next frame.
        assert!(mask.px.iter().all(|&m| m == 0) && mask.reach.iter().all(|&m| m == 0) && mask.dirty.is_none());
    }

    #[test]
    fn a_shadow_is_solid_inside_and_dithered_only_at_its_edge() {
        let mut mask = Mask::default();
        mask.fit(20, 20);
        mask.band(Band { x0: 2, x1: 18, y0: 2, y1: 18, strength: 200, reach: 50 });
        let mut px = vec![0xff80_8080u32; 400];
        let mut t = Target { px: &mut px, w: 20, h: 20 };
        apply(&mut t, &mut mask, [128, 128, 200], &[], 0);
        // Every px inside the edge is shaded, whatever the dither says there.
        for y in 3..17 {
            for x in 3..17 {
                assert_ne!(px[y * 20 + x], 0xff80_8080, "({x}, {y}) left unshaded");
            }
        }
    }

    /// A band 16 px square laid under a clear sun `deg` degrees up: how many px of row 10 are
    /// shaded at all but less than the band's middle (its soft edge), and how red the middle
    /// still is.
    fn edge_and_depth(deg: i32) -> (usize, u32) {
        let s = jane_core::angle::sin_q15(Angle::from_degrees(deg)).0;
        let (spread, strength) = (jane_present::light::spread(s), jane_present::light::strength(s));
        let sun = Directional { spread, strength, ..sun(Angle::WEST, deg) };
        let mut mask = Mask::default();
        mask.fit(40, 20);
        mask.band(Band { x0: 12, x1: 28, y0: 2, y1: 18, strength: 255, reach: 50 });
        let mut px = vec![0xff80_8080u32; 800];
        let mut t = Target { px: &mut px, w: 40, h: 20 };
        let shade = shadow::shade_at([128, 128, 200], sun.strength);
        apply(&mut t, &mut mask, shade, &[], shadow::feather(sun.spread));
        let row: Vec<u32> = (0..40).map(|x| px[10 * 40 + x] >> 16 & 0xff).collect();
        let middle = row[20];
        (row.iter().filter(|&&r| r != 0x80 && r != middle).count(), middle)
    }

    #[test]
    fn a_noon_shadow_is_crisper_and_darker_than_five_oclocks() {
        // The sun at noon is 46 degrees up, at five 16.
        let (noon_edge, noon) = edge_and_depth(46);
        let (five_edge, five) = edge_and_depth(16);
        assert!(noon_edge < five_edge, "the edge at noon {noon_edge} px, at five {five_edge}");
        assert!(noon < five, "the middle at noon {noon}, at five {five}: no darker");
    }

    #[test]
    fn a_shadow_climbs_a_wall_as_high_as_it_reaches_and_never_lies_across_it() {
        // A wall's face on rows 0..=9 of a 20 x 24 canvas, its foot on row 9: row y stands
        // `height_of_rows(10 - y)` px up (the lowest three, 4 px and under, are ground). A
        // shadow lies on the ground rows 5..12 of columns 5..10, reaching 8 px up.
        let (w, h) = (20, 24);
        let mut heights = vec![1u8; (w * h) as usize];
        for y in 0..10 {
            for x in 0..w {
                heights[(y * w + x) as usize] = jane_present::height_of_rows(10 - y) as u8;
            }
        }
        let mut mask = Mask::default();
        mask.fit(w, h);
        mask.band(Band { x0: 5, x1: 10, y0: 5, y1: 12, strength: 200, reach: 8 });
        let mut px = vec![0xff80_8080u32; (w * h) as usize];
        let mut t = Target { px: &mut px, w, h };
        apply(&mut t, &mut mask, [128, 128, 200], &heights, 0);
        let dark = |x: i32, y: i32| px[(y * w + x) as usize] != 0xff80_8080;
        // Up the face over the columns it covers, as high as 8 px (the face's rows 4 and more
        // are 7 px and less; row 3 is 8 px; row 2 is 10 px and stays lit).
        for y in 3..10 {
            assert!(dark(7, y), "row {y} of the face is lit under the shadow");
        }
        assert!(!dark(7, 1) && !dark(7, 2));
        // Beside it on the face, lit (the ring's dither aside).
        assert!(!dark(15, 5) && !dark(2, 5));
        // On the ground in front, as laid.
        assert!(dark(7, 11) && !dark(7, 14));
    }
}
