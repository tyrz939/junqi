//! The silhouette shadow's integers (PRESENTATION.md §1.7, T0 and T1): what `soft` lays in its
//! mask and `gl2` draws as span quads, one function for both so the two tiers lay the same
//! shadow to the px (the gl2 albedo test holds them to it).
//!
//! A caster stands on its foot row. Each row of its silhouette `hv` rows above the foot is
//! `height_of_rows(hv)` true px up (the 3/4 view's one projection, `rows_up`), so the sun lays it
//! that height times the cotangent of its elevation away from the sun, stretched to meet the row
//! above it and as thick as the caster is deep, its depth behind the foot row (a sprite's lowest
//! px is the front of what it stands on; T2's field stands it the same). Under the rows, a **foot**: an ellipse two px
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

use crate::frame::{Caster, Directional, Rgb, SpriteCmd, height_of_rows, rows_up};

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

/// A gap in a row of 2 px or more parts it into two runs: a lantern hung off its post, a hand
/// held out from the body. A single px (a dithered edge, a notch between leaves) does not: T2's
/// field keeps a column's whole height, and closes it with the rows over and under.
pub const GAP: usize = 2;

/// How many px wider than its one dither step a silhouette's edge is feathered for a sun or
/// moon of `spread` (`Directional::spread`): none under a high clear sun, one for the low sun of
/// five o'clock or seven, two at the horizon, three under cloud. T2's penumbra widens with the
/// spread and the distance from what casts; T0 and T1 widen the edge by this.
pub fn feather(spread: u16) -> i32 {
    let deg = |d: i32| d * 65536 / 360;
    match i32::from(spread) {
        s if s <= deg(2) => 0,
        s if s <= deg(4) => 1,
        s if s <= deg(6) => 2,
        _ => 3,
    }
}

/// A silhouette's multiplier `shade` laid at `strength` (of 255, `Directional::strength`): the
/// whole of it under a high clear sun, lighter for a low one, faint under cloud.
pub fn shade_at(shade: Rgb, strength: u8) -> Rgb {
    shade.map(|c| (255 - (255 - u32::from(c)) * u32::from(strength) / 255) as u8)
}

/// A caster's rows, from the foot up: `(rows above the foot, first, last)` px from the sprite's
/// left of each opaque run in the row, a row parted where it has a [`GAP`] (the contact shadow,
/// index 1, is not the silhouette), for the sprite `s` drawn from an atlas page's albedo
/// `albedo`, `page_w` wide, standing on row `foot_y` of the canvas. Appended to `out`. So a run
/// that is not over another, lower one (a canopy past its trunk, a lamp's head past its post, a
/// lantern on its bracket) lays its shadow only where its own height throws it, apart from the
/// root's, as T2's field has it floating.
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
        let mut run: Option<(usize, usize)> = None;
        let mut emit = |(first, last): (usize, usize)| {
            let (u0, u1) = if s.flags.mirror {
                (sw - 1 - last as i32, sw - 1 - first as i32)
            } else {
                (first as i32, last as i32)
            };
            out.push((hv, u0, u1));
        };
        for (u, _) in row.iter().enumerate().filter(|&(_, &i)| i > 1) {
            run = match run {
                Some((a, b)) if u - b <= GAP => Some((a, u)),
                Some(r) => {
                    emit(r);
                    Some((u, u))
                }
                None => Some((u, u)),
            };
        }
        if let Some(r) = run {
            emit(r);
        }
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
    // A row stands its rows' true height up, and no higher than the caster's tallest px.
    let tall = i32::from(c.height).max(1);
    let up = |hv: i32| height_of_rows(hv).min(tall);
    // The foot: as wide as the lowest rows and two px more each side, under the feet, drawn out
    // along the shadow as far as the feet's own shadow reaches. What stands on nothing (a bat
    // in flight, a lantern hung over the ground) has none: its shadow is only where its height
    // throws it.
    if rows[0].0 > FOOT_ROWS {
        return bands_of_rows(rows, x, c, (kx, ky), top, emit);
    }
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
    bands_of_rows(rows, x, c, (kx, ky), top, emit);
}

/// The rows' bands of [`bands`], past the foot: each row its true height's shear away, stretched
/// to meet the row above, as thick as the caster is deep.
fn bands_of_rows(
    rows: &[(i32, i32, i32)],
    x: i32,
    c: &Caster,
    (kx, ky): (i32, i32),
    top: i32,
    mut emit: impl FnMut(Band),
) {
    let fy = i32::from(c.foot.1);
    let depth = i32::from(c.depth).max(2);
    let tall = i32::from(c.height).max(1);
    let up = |hv: i32| height_of_rows(hv).min(tall);
    let htop = up(top);
    for &(hv, u0, u1) in rows {
        let (h0, h1) = (up(hv), up(hv + 1));
        let (ax, bx) = ((h0 * kx) >> 8, (h1 * kx) >> 8);
        let (ay, by) = ((h0 * ky) >> 8, (h1 * ky) >> 8);
        let strength = 256 - (256 - TIP) * hv.min(top) / top;
        // No deeper than it is wide (T2's field: its middle column's `2 inset + 2`): a post
        // 2 px wide throws a shadow 2 px thick whichever way it falls.
        let deep = depth.min(2 * ((u1 - u0) / 2) + 2);
        emit(Band {
            x0: x + u0 + ax.min(bx),
            x1: x + u1 + 1 + ax.max(bx),
            y0: fy + ay.min(by) - deep + 1,
            y1: fy + ay.max(by) + 1,
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
        Directional { azimuth, elevation: Angle::from_degrees(deg), colour: [255; 3], spread: 0, strength: 255 }
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

    /// A 40 x 58 sprite of index 2 where `shape` says, over a contact shadow row, its foot on
    /// row 56, standing on canvas `(120, 100)`; and its bands in a sun in the north at 40
    /// degrees, so its shadow runs down the screen.
    fn cast(shape: impl Fn(i32, i32) -> bool) -> (Vec<Band>, (i32, i32)) {
        let (w, h) = (40usize, 58usize);
        let mut albedo = vec![0u16; w * h];
        for y in 0..h {
            for x in 0..w {
                if shape(x as i32, y as i32) {
                    albedo[y * w + x] = 2;
                }
            }
        }
        albedo[57 * w + 16..57 * w + 24].fill(1);
        let foot = (120, 100);
        let s = SpriteCmd {
            page: 0,
            src: Src { x: 0, y: 0, w: w as u16, h: h as u16 },
            x: (foot.0 - 20) as i16,
            y: (foot.1 - 56) as i16,
            flags: Flags::default(),
            height_px: 70,
        };
        let c = Caster { sprite: 0, foot: (foot.0 as i16, foot.1 as i16), height: 70, depth: 6 };
        let mut r = Vec::new();
        rows(&albedo, w as u16, &s, foot.1, &mut r);
        let mut bands = Vec::new();
        super::bands(&r, i32::from(s.x), &c, shear(&sun(Angle::NORTH, 40)).unwrap(), |b| bands.push(b));
        (bands, foot)
    }

    fn covered(bands: &[Band], y: i32) -> Vec<i32> {
        (0..400).filter(|&x| bands.iter().any(|b| b.x0 <= x && x < b.x1 && b.y0 <= y && y < b.y1)).collect()
    }

    #[test]
    fn a_trees_shadow_is_its_trunks_at_the_root_and_its_crowns_further_out() {
        // A crown 32 wide from row 4 to row 30 (33 px to 65 px up) on a trunk 4 wide.
        let (bands, foot) =
            cast(|x, y| ((4..=30).contains(&y) && (4..36).contains(&x)) || (y > 30 && (18..22).contains(&x)));
        // cot 40 degrees is 1.19: the trunk's shadow is the 39 rows south of the foot, and near
        // the root it is the trunk, its foot two px wider each side, and no wider.
        for dy in 4..30 {
            let n = covered(&bands, foot.1 + dy).len();
            assert!((3..=8).contains(&n), "{dy} rows south of the root: {n} px, the crown's shadow stands on the root");
        }
        for dy in 50..70 {
            let n = covered(&bands, foot.1 + dy).len();
            assert!(n >= 32, "{dy} rows south of the root: {n} px, no crown's shadow");
        }
    }

    #[test]
    fn a_lantern_hung_off_its_post_throws_its_shadow_apart_from_the_posts() {
        // A post 2 wide, an arm east along rows 8 and 9, a lantern 8 wide hanging from it, rows 10
        // to 22, 8 px clear of the post.
        let (bands, foot) = cast(|x, y| {
            (y >= 8 && (10..12).contains(&x))
                || ((8..=9).contains(&y) && (10..28).contains(&x))
                || ((10..=22).contains(&y) && (20..28).contains(&x))
        });
        let x0 = foot.0 - 20;
        // Near the root the post's alone; under where the lantern hangs, nothing.
        for dy in 4..30 {
            let c = covered(&bands, foot.1 + dy);
            assert!(
                !c.iter().any(|x| (x0 + 20..x0 + 28).contains(x)),
                "{dy} rows south: the lantern's shadow is on the root: {c:?}"
            );
        }
        // Further out the lantern's, and between it and the post's, the ground.
        let c = covered(&bands, foot.1 + 58);
        assert!((x0 + 20..x0 + 28).all(|x| c.contains(&x)), "no lantern's shadow: {c:?}");
        assert!((x0 + 13..x0 + 19).any(|x| !c.contains(&x)), "the lantern's shadow runs into the post's: {c:?}");
    }

    #[test]
    fn what_flies_has_no_foot() {
        // A bat: a body 12 wide from row 30 to row 40, 20 rows over its anchor on row 56.
        let (bands, foot) = cast(|x, y| (30..=40).contains(&y) && (14..26).contains(&x));
        // Nothing under it: the nearest band is its lowest row's, 16 rows' height (20 px) times
        // 1.19 away (23 rows), its depth (6 rows) behind that.
        let near = bands.iter().map(|b| b.y0).min().unwrap();
        assert!(near >= foot.1 + 18, "a shadow at its anchor: {near}");
    }
}
