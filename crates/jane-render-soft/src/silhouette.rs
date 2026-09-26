//! Silhouette sun shadows (PRESENTATION.md §1.3 `silhouettes`): each caster's albedo mask
//! sheared along the sun by its height and laid on the ground, tinted by the ambient's shade,
//! its edge softened by one dither step. Integer only, like every pixel `soft` writes.
//!
//! A sprite stands on its foot row: a pixel `h` rows above it is `h` px above the ground, so the
//! sun lays it `h * cot(elevation)` px away from the sun, across the ground (the 3/4 view maps a
//! step across the ground to a step on the screen one for one). Each row's opaque run becomes a
//! span on the ground, stretched to meet the next row's, and as thick as the caster is deep. The
//! spans go into a coverage mask (the strongest wins, so two shadows never darken twice), whose
//! strength falls from the foot to the tip; the mask is applied once, by a multiply toward the
//! shade.

use jane_core::angle::{cos_q15, sin_q15};
use jane_present::{Caster, Directional, Page, Rgb, SpriteCmd};

use crate::blit::Target;

/// The longest a shadow gets, in heights: a sun this low casts no further (8 x 256).
const MAX_COT_Q8: i32 = 8 * 256;
/// How much of its strength a shadow keeps at its tip, of 256.
const TIP: i32 = 170;

/// The 4 x 4 ordered dither (thresholds 0..16), for the soft edge.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// The shadow's reach per px of height, Q8, across the ground: away from the sun.
pub fn shear(sun: &Directional) -> Option<(i32, i32)> {
    let (se, ce) = (sin_q15(sun.elevation).0, cos_q15(sun.elevation).0);
    if se <= 0 {
        return None;
    }
    let cot = (ce * 256 / se).min(MAX_COT_Q8);
    let (ca, sa) = (cos_q15(sun.azimuth).0, sin_q15(sun.azimuth).0);
    Some((-(ca * cot) >> 15, -(sa * cot) >> 15))
}

/// The mask the shadows are gathered in, the canvas's size, and the box it was written in.
#[derive(Debug, Default)]
pub struct Mask {
    px: Vec<u8>,
    w: i32,
    h: i32,
    dirty: Option<(i32, i32, i32, i32)>,
}

impl Mask {
    pub fn fit(&mut self, w: i32, h: i32) {
        let n = (w * h) as usize;
        if self.px.len() != n {
            self.px.clear();
            self.px.resize(n, 0);
        }
        (self.w, self.h) = (w, h);
    }

    /// Covers `[x0, x1) x [y0, y1)` at `s`, the strongest kept.
    fn span(&mut self, x0: i32, x1: i32, y0: i32, y1: i32, s: u8) {
        let (x0, x1, y0, y1) = (x0.max(0), x1.min(self.w), y0.max(0), y1.min(self.h));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let row = &mut self.px[(y * self.w + x0) as usize..(y * self.w + x1) as usize];
            for m in row {
                *m = (*m).max(s);
            }
        }
        self.dirty = Some(match self.dirty {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
}

/// Lays one caster's shadow into `mask`, reading its silhouette from `page`.
pub fn cast(mask: &mut Mask, page: &Page, s: &SpriteCmd, c: &Caster, (kx, ky): (i32, i32)) {
    let (sw, sh) = (i32::from(s.src.w), i32::from(s.src.h));
    let pw = usize::from(page.w);
    let (x, y) = (i32::from(s.x), i32::from(s.y));
    let (fy, depth) = (i32::from(c.foot.1), i32::from(c.depth).max(2));
    let height = i32::from(c.height).max(1);
    for v in 0..sh {
        // How high this row stands above the foot; the foot row and below cast nothing.
        let hv = fy - (y + v);
        if hv <= 0 || hv > 255 {
            continue;
        }
        let row = &page.albedo[(usize::from(s.src.y) + v as usize) * pw + usize::from(s.src.x)..][..sw as usize];
        let Some(first) = row.iter().position(|&i| i > 1) else { continue };
        let last = row.iter().rposition(|&i| i > 1).unwrap_or(first);
        let (u0, u1) =
            if s.flags.mirror { (sw - 1 - last as i32, sw - 1 - first as i32) } else { (first as i32, last as i32) };
        // This row laid on the ground, stretched to where the row above it lies.
        let (ax, bx) = ((hv * kx) >> 8, ((hv + 1) * kx) >> 8);
        let (ay, by) = ((hv * ky) >> 8, ((hv + 1) * ky) >> 8);
        let s8 = 256 - (256 - TIP) * hv.min(height) / height;
        mask.span(
            x + u0 + ax.min(bx),
            x + u1 + 1 + ax.max(bx),
            fy + ay.min(by) - depth / 2,
            fy + ay.max(by) + depth - depth / 2,
            s8.clamp(1, 255) as u8,
        );
    }
}

/// Applies the mask to `t` and clears it: each covered pixel toward `dst * shade` by its
/// strength, a pixel on the shadow's edge only where the dither says. Returns pixels written.
pub fn apply(t: &mut Target<'_>, mask: &mut Mask, shade: Rgb) -> u64 {
    let Some((x0, y0, x1, y1)) = mask.dirty.take() else { return 0 };
    let w = mask.w;
    let at =
        |m: &[u8], x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= mask.h { 0 } else { m[(y * w + x) as usize] };
    let mut n = 0;
    let [sr, sg, sb] = shade.map(|c| 256 - i32::from(c) - i32::from(c >> 7));
    for y in y0..y1 {
        for x in x0..x1 {
            let m = mask.px[(y * w + x) as usize];
            if m == 0 {
                continue;
            }
            let edge = at(&mask.px, x - 1, y) == 0
                || at(&mask.px, x + 1, y) == 0
                || at(&mask.px, x, y - 1) == 0
                || at(&mask.px, x, y + 1) == 0;
            if edge && BAYER4[(y & 3) as usize][(x & 3) as usize] >= 8 {
                continue;
            }
            let m = i32::from(m) + i32::from(m >> 7);
            let d = &mut t.px[(y * t.w + x) as usize];
            let c = *d;
            let ch = |shift: u32, k: i32| {
                let v = ((c >> shift) & 0xff) as i32;
                ((v * (256 - ((k * m) >> 8))) >> 8) as u32
            };
            *d = 0xff00_0000 | ch(16, sr) << 16 | ch(8, sg) << 8 | ch(0, sb);
            n += 1;
        }
        let row = &mut mask.px[(y * w + x0) as usize..(y * w + x1) as usize];
        row.fill(0);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_core::Angle;
    use jane_present::{Flags, Src};

    fn sun(azimuth: Angle, deg: i32) -> Directional {
        Directional { azimuth, elevation: Angle::from_degrees(deg), colour: [255; 3], spread: 0 }
    }

    #[test]
    fn a_low_western_sun_lays_shadows_long_to_the_east() {
        let (kx, ky) = shear(&sun(Angle::WEST, 20)).unwrap();
        // cot 20 degrees is 2.75: east, level.
        assert!((kx - 704).abs() < 8 && ky.abs() < 4, "{kx} {ky}");
        let (kx, ky) = shear(&sun(Angle::SOUTH, 45)).unwrap();
        assert!(kx.abs() < 4 && (ky + 256).abs() < 4, "{kx} {ky}");
        assert!(shear(&sun(Angle::WEST, 0)).is_none());
    }

    #[test]
    fn a_post_casts_a_band_that_starts_at_its_foot_and_is_applied_once() {
        // A 2 x 10 post of index 2 standing on row 10 of a 40 x 20 canvas.
        let page = Page { w: 2, h: 10, albedo: vec![2; 20], ..Page::default() };
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 2, h: 10 },
            x: 4,
            y: 0,
            flags: Flags::default(),
            height_px: 10,
        };
        let c = Caster { sprite: 0, foot: (5, 10), height: 10, depth: 2 };
        let mut mask = Mask::default();
        mask.fit(40, 20);
        let k = shear(&sun(Angle::WEST, 20)).unwrap();
        cast(&mut mask, &page, &s, &c, k);
        // Twice over the same ground: still one shadow.
        cast(&mut mask, &page, &s, &c, k);
        let mut px = vec![0xff80_8080u32; 800];
        let mut t = Target { px: &mut px, w: 40, h: 20 };
        let n = apply(&mut t, &mut mask, [128, 128, 200]);
        assert!(n > 20, "{n}");
        // East of the post on its foot row is shadowed, and bluer than it is red; west is not.
        let east = px[10 * 40 + 14];
        assert!(east & 0xff > (east >> 16) & 0xff && east != 0xff80_8080, "{east:08x}");
        assert_eq!(px[10 * 40], 0xff80_8080);
        // The mask is clear for the next frame.
        assert!(mask.px.iter().all(|&m| m == 0) && mask.dirty.is_none());
    }
}
