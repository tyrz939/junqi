//! The silhouette shadow's integers (PRESENTATION.md §1.7, T0 and T1): what `soft` lays in its
//! mask and `gl2` draws as span quads, one function for both so the two tiers lay the same
//! shadow to the px (the gl2 albedo test holds them to it).
//!
//! A caster stands on its foot row. Each row of its silhouette `hv` rows above the foot is
//! `height_of_rows(hv)` true px up (the 3/4 view's one projection, `rows_up`), so the sun lays it
//! that height times the cotangent of its elevation away from the sun, stretched to meet the row
//! above it and as thick as the caster is deep. Under the rows, a **foot**: an ellipse two px
//! wider each side than the silhouette's lowest rows, drawn out a few px along the shadow, so the
//! shadow is seen to grow out of the feet even where the body hides where it starts (in the 3/4
//! view a shadow thrown up the screen runs behind the body that casts it).
//!
//! Every band carries a **reach**: how high over the ground it lies the shadow reaches, the ray
//! over the caster's top there. A lifted receiver (a wall's face, a roof) is shadowed where the
//! ground under it (`rows_up` of its height below it) is, if the shadow there reaches its height:
//! a post's shadow runs along the ground to a wall's foot and climbs it, and never lies across
//! the wall's face as if it were ground.

use jane_core::angle::{cos_q15, sin_q15};

use crate::frame::{Caster, Directional, SpriteCmd, height_of_rows, rows_up};

/// The longest a shadow gets, in heights: a sun this low casts no further (8 x 256).
pub const MAX_COT_Q8: i32 = 8 * 256;
/// How much of its strength a shadow keeps at its tip, of 256.
pub const TIP: i32 = 170;
/// A receiver at or under this height is the ground (a tuft, a cobble's relief): it takes the
/// shadow laid on it.
pub const GROUND: i32 = 4;
/// How far up a thing standing right beside a caster's feet its foot shadow climbs, px.
pub const FOOT_REACH: u8 = 8;
/// The foot's rows each side of the row under the feet, and how far along the shadow it is
/// drawn out at most, px across and down the screen.
const FOOT_B: i32 = 2;
const FOOT_ALONG: (i32, i32) = (6, 3);
/// The silhouette's rows the foot is as wide as: the feet and what is just above them.
const FOOT_ROWS: i32 = 3;

/// The shadow's reach per px of true height, Q8, across the ground: away from the sun.
pub fn shear(sun: &Directional) -> Option<(i32, i32)> {
    let (se, ce) = (sin_q15(sun.elevation).0, cos_q15(sun.elevation).0);
    if se <= 0 {
        return None;
    }
    let cot = (ce * 256 / se).min(MAX_COT_Q8);
    let (ca, sa) = (cos_q15(sun.azimuth).0, sin_q15(sun.azimuth).0);
    Some((-(ca * cot) >> 15, -(sa * cot) >> 15))
}

/// A caster's rows, from the foot up: `(rows above the foot, first, last)` px from the sprite's
/// left of each row's opaque run (the contact shadow, index 1, is not the silhouette), for the
/// sprite `s` drawn from an atlas page's albedo `albedo`, `page_w` wide, standing on row
/// `foot_y` of the canvas. Appended to `out`.
pub fn rows(albedo: &[u16], page_w: u16, s: &SpriteCmd, foot_y: i32, out: &mut Vec<(i32, i32, i32)>) {
    let pw = usize::from(page_w);
    let sw = i32::from(s.src.w);
    for v in (0..i32::from(s.src.h)).rev() {
        let hv = foot_y - (i32::from(s.y) + v);
        if hv <= 0 {
            continue;
        }
        if hv > 255 {
            break;
        }
        let start = (usize::from(s.src.y) + v as usize) * pw + usize::from(s.src.x);
        let Some(row) = albedo.get(start..start + usize::from(s.src.w)) else { continue };
        let Some(first) = row.iter().position(|&i| i > 1) else { continue };
        let last = row.iter().rposition(|&i| i > 1).unwrap_or(first);
        let (u0, u1) =
            if s.flags.mirror { (sw - 1 - last as i32, sw - 1 - first as i32) } else { (first as i32, last as i32) };
        out.push((hv, u0, u1));
    }
}

/// One band of a shadow: `[x0, x1) x [y0, y1)` canvas px at `strength` (of 255), reaching
/// `reach` px up over the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
    pub strength: u8,
    pub reach: u8,
}

/// The bands of caster `c`'s shadow: its foot, then its `rows` (from [`rows`]) sheared by
/// `(kx, ky)` ([`shear`]), for its sprite drawn with its left edge at `x`.
pub fn bands(rows: &[(i32, i32, i32)], x: i32, c: &Caster, (kx, ky): (i32, i32), mut emit: impl FnMut(Band)) {
    let Some(&(top, _, _)) = rows.last() else { return };
    let fy = i32::from(c.foot.1);
    let depth = i32::from(c.depth).max(2);
    // A row stands its rows' true height up, and no higher than the caster's tallest px.
    let tall = i32::from(c.height).max(1);
    let up = |hv: i32| height_of_rows(hv).min(tall);
    let htop = up(top);
    // The foot: as wide as the lowest rows and two px more each side, under the feet, drawn out
    // along the shadow as far as the feet's own shadow reaches.
    let (lo, hi) = rows
        .iter()
        .take_while(|r| r.0 <= FOOT_ROWS)
        .fold((rows[0].1, rows[0].2), |(lo, hi), r| (lo.min(r.1), hi.max(r.2)));
    let reach = up(FOOT_ROWS + 1);
    let ox = ((reach * kx) >> 8).clamp(-FOOT_ALONG.0, FOOT_ALONG.0);
    let oy = ((reach * ky) >> 8).clamp(-FOOT_ALONG.1, FOOT_ALONG.1);
    let a = (hi - lo + 1) / 2 + 2;
    let cx = x + (lo + hi + 1) / 2;
    for dy in -FOOT_B..=FOOT_B {
        // Half the ellipse's width on this row, a px at its tips.
        let across = FOOT_B * FOOT_B - dy * dy;
        let w = (a * jane_core::num::isqrt((across * 256) as u64) as i32 / (FOOT_B * 16)).max(1);
        let row = fy + 1 + dy;
        emit(Band {
            x0: cx - w + ox.min(0),
            x1: cx + w + ox.max(0),
            y0: row + oy.min(0),
            y1: row + oy.max(0) + 1,
            strength: 255,
            reach: FOOT_REACH,
        });
    }
    for &(hv, u0, u1) in rows {
        let (h0, h1) = (up(hv), up(hv + 1));
        let (ax, bx) = ((h0 * kx) >> 8, (h1 * kx) >> 8);
        let (ay, by) = ((h0 * ky) >> 8, (h1 * ky) >> 8);
        let strength = 256 - (256 - TIP) * hv.min(top) / top;
        emit(Band {
            x0: x + u0 + ax.min(bx),
            x1: x + u1 + 1 + ax.max(bx),
            y0: fy + ay.min(by) - depth / 2,
            y1: fy + ay.max(by) + depth - depth / 2,
            strength: strength.clamp(1, 255) as u8,
            reach: (htop - h0).clamp(1, 255) as u8,
        });
    }
}

/// Where a receiver `h` px up at canvas row `y` takes its shadow from: the ground row under it
/// and the reach the shadow there must have (the ground takes any).
pub fn ground_of(y: i32, h: u8) -> (i32, u8) {
    let h = i32::from(h);
    if h <= GROUND { (y, 0) } else { (y + rows_up(h), h as u8) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Flags, Src};
    use jane_core::Angle;

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

    /// A 4 x 20 post of index 2 over a contact shadow row, standing on canvas row 30.
    fn post() -> (Vec<u16>, SpriteCmd, Caster) {
        let mut albedo = vec![2u16; 4 * 21];
        albedo[80..84].fill(1);
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: 4, h: 21 },
            x: 20,
            y: 11,
            flags: Flags::default(),
            height_px: 25,
        };
        (albedo, s, Caster { sprite: 0, foot: (22, 31), height: 25, depth: 4 })
    }

    #[test]
    fn the_shadow_grows_out_of_the_feet_whatever_the_hour() {
        let (albedo, s, c) = post();
        let mut r = Vec::new();
        rows(&albedo, 4, &s, i32::from(c.foot.1), &mut r);
        // The contact shadow's row is not the silhouette; the first row is the one on the foot.
        assert_eq!(r.first(), Some(&(1, 0, 3)));
        assert_eq!(r.last().map(|r| r.0), Some(20));
        for (az, el) in [(Angle::WEST, 15), (Angle::SOUTH, 46), (Angle::EAST, 30), (Angle::NORTH, 20)] {
            let k = shear(&sun(az, el)).unwrap();
            let mut bands = Vec::new();
            super::bands(&r, i32::from(s.x), &c, k, |b| bands.push(b));
            // The foot covers the feet's own px and the row under them, full strength.
            let (fx, fy) = (i32::from(c.foot.0), i32::from(c.foot.1));
            let covers = |x: i32, y: i32| bands.iter().any(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1);
            for x in 20..24 {
                assert!(covers(x, fy) && covers(x, fy + 1), "{az:?} {el}: ({x}, {fy})");
            }
            // Two px clear of the feet each side, whichever way the shadow runs.
            assert!(covers(18, fy + 1) && covers(25, fy + 1), "{az:?} {el}");
            // And the shadow of the lowest row touches the foot: no gap between the two.
            let first = bands[(2 * FOOT_B + 1) as usize];
            assert!(first.y0 <= fy + FOOT_B + 1 && first.y1 >= fy - FOOT_B, "{az:?} {el}: {first:?}");
            assert!(first.x0 <= fx + 2 && first.x1 >= fx - 1, "{az:?} {el}: {first:?}");
        }
    }

    #[test]
    fn a_shadow_reaches_up_as_high_as_the_ray_over_the_top() {
        let (albedo, s, c) = post();
        let mut r = Vec::new();
        rows(&albedo, 4, &s, i32::from(c.foot.1), &mut r);
        let mut bands = Vec::new();
        super::bands(&r, i32::from(s.x), &c, shear(&sun(Angle::WEST, 20)).unwrap(), |b| bands.push(b));
        let rows_only = &bands[(2 * FOOT_B + 1) as usize..];
        // At the root it reaches nearly the post's height (25 px); at the tip, nothing.
        assert!(rows_only[0].reach >= 23, "{:?}", rows_only[0]);
        assert!(rows_only.last().unwrap().reach <= 2);
        assert!(rows_only.windows(2).all(|w| w[0].reach >= w[1].reach));
        // A 20-row post is 25 px tall: its shadow is that times cot 20 degrees, 2.75, long,
        // past its right edge.
        let far = rows_only.iter().map(|b| b.x1).max().unwrap();
        assert_eq!(far, 24 + ((25 * 704) >> 8), "{far}");
        // A face 10 px up takes its shadow from the ground 8 rows under it, if it reaches 10.
        assert_eq!(ground_of(50, 10), (58, 10));
        assert_eq!(ground_of(50, 3), (50, 0));
    }
}
