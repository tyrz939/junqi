//! Hair (ART.md §2.1): the masses are `ellipse_lit` and `polygon_lit` on a layer of their own,
//! held to three tones (shade, mid, base) so they read as hair and not as a lit ball; then a
//! highlight band arcs across the crown on the light's side, broken where the locks part, and
//! two or three strand lines run with the lie of the hair. The layer is stamped on the figure
//! as one part, so the pieces of one head of hair never seam against each other. The ends of
//! long hair hang a frame behind the body.

use jane_core::grid::Rect;
use jane_data::Hair;

use super::draw::{CX, Rig, hides_hair, relief};
use super::pose::Facing;
use super::{Dress, H, W};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ramp, Tone};

/// Hair's own tones, by the tone the light gave it: a shade, a mid and a base; its light comes
/// from the band laid on afterwards.
const HAIR: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Lift, Tone::Lift];

fn long(d: &Dress) -> bool {
    matches!(d.look.head.hair, Hair::Long | Hair::Wet)
}

/// Hold `ramp`'s tones on `c` between `lo` and `hi`.
pub(crate) fn tame(c: &mut Canvas, ramp: Ramp, lo: Tone, hi: Tone) {
    c.remap(|ix| match Ramp::of(ix) {
        Some((r, t)) if r == ramp => ramp.at(t.clamp(lo, hi)),
        _ => ix,
    });
}

/// The highlight band: a two-row arc over the crown from the left edge to a little past the
/// middle, peaking left of centre (the light is top-left), broken every fifth column where one
/// lock parts from the next. `top` is the crown's first row, `x0..=x1` the head's columns.
fn sheen(l: &mut Canvas, ramp: Ramp, x0: i32, x1: i32, top: i32) {
    let peak = x0 + (x1 - x0) * 2 / 5;
    let end = x0 + (x1 - x0) * 3 / 5;
    for x in x0 + 1..=end {
        if (x - x0) % 5 == 4 {
            continue;
        }
        let dx = x - peak;
        let y = top + 2 + dx * dx / 9;
        // The band's core, where the light strikes square, is the hair's high tone: a lock's
        // gloss, two px at the peak.
        l.tint(x, y, ramp, if dx == 0 { Tone::High } else { Tone::Light });
        l.tint(x, y + 1, ramp, Tone::Lift);
    }
}

/// Clear every third px of the row `y` across `x0..=x1` from `l` (offset by `phase`), so a hem
/// of hair ends in points rather than a ruled line.
fn points(l: &mut Canvas, x0: i32, x1: i32, y: i32, phase: i32) {
    for x in x0..=x1 {
        if (x + phase).rem_euclid(3) == 0 {
            l.clear_px(x, y);
        }
    }
}

/// A strand line from `a` to `b` a tone under the hair it crosses (base goes to mid, mid to
/// shade), over what is already hair.
fn strand(l: &mut Canvas, ramp: Ramp, a: (i32, i32), b: (i32, i32)) {
    crate::canvas::bresenham(a.0, a.1, b.0, b.1, |x, y| {
        if let Some((r, t)) = Ramp::of(l.get(x, y)) {
            if r == ramp {
                l.tint(x, y, ramp, if t >= Tone::Base { Tone::Mid } else { Tone::Shade });
            }
        }
    });
}

/// A plain hair layer (no band): its tones, then onto the figure.
fn lay(c: &mut Canvas, mut l: Canvas, d: &Dress) {
    l.retone(d.hair, HAIR);
    c.stamp(&l, 0, 0);
}

/// Long hair behind the head, seen from the front: it shows beside the neck and over the
/// shoulders, in the head's shade. Drawn first.
pub fn back_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    if !long(d) || hides_hair(d) {
        return;
    }
    let s = r.skull;
    let mut l = Canvas::new(W, H);
    let b = r.top + 4 + r.trail.1;
    // Three quarters on, it falls behind her back: toward the screen's left.
    let k = -i32::from(r.facing == Facing::DownRight);
    // It spreads over the shoulders as it falls, wider than the head at its ends (an A of
    // hair, the silhouette's), and ends in points.
    l.polygon_lit(
        &[
            (s.x + k, s.y + 5),
            (s.right() - 1 + k, s.y + 5),
            (s.right() + 1 + 2 * k, b - 2),
            (s.right() + 1 + 2 * k, b),
            (s.x - 2 + 2 * k, b),
            (s.x - 2 + 2 * k, b - 2),
        ],
        d.hair,
        110,
        relief::FAR,
    );
    points(&mut l, s.x - 2 + 2 * k, s.right() + 1 + 2 * k, b, r.trail.0);
    l.retone(d.hair, HAIR);
    l.shade(Rect::new(s.x - 3, s.y + 5, s.w + 6, b - s.y + 1), d.hair, 1);
    c.stamp(&l, 0, 0);
}

/// The hair over the skull from the front: a cap to the brow, a fringe swept from a parting
/// left of centre, and locks by the style.
pub fn front_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let style = d.look.head.hair;
    if hides_hair(d) {
        return;
    }
    let hair = d.hair;
    let z = relief::HAIR;
    let brow = r.brow_y();
    let mut l = Canvas::new(W, H);
    if style == Hair::Bald {
        for x0 in [s.x - 1, s.right() - 2] {
            l.ellipse_lit(Rect::new(x0, r.eye_y() - 2, 3, 5), hair, z);
        }
        lay(c, l, d);
        return;
    }
    let cap_to = if style == Hair::Cropped { brow - 1 } else { brow };
    // The crown stands a row over the skull's own curve: hair has volume.
    let lift = i32::from(style != Hair::Cropped);
    l.set_clip(Some(Rect::new(0, 0, W, cap_to + 1)));
    l.ellipse_lit(Rect::new(s.x - 1, s.y - 2 - lift, s.w + 2, s.h + 2 + lift), hair, z);
    l.set_clip(None);
    // Three quarters on the parting turns with the face, and more of the head's near side
    // (the screen's left) shows as hair: its lock is a px wider and the far one a px narrower.
    let turned = r.facing == Facing::DownRight;
    let part = CX - 3 + 2 * i32::from(turned);
    if style != Hair::Cropped {
        // The fringe: a short tuft left of the parting, a longer sweep right of it that comes
        // down over the brow in two points.
        l.polygon_lit(&[(s.x + 1, brow), (part - 1, brow), (s.x + 1, brow + 1)], hair, 60, z);
        l.polygon_lit(
            &[(part + 1, brow), (s.right() - 2, brow), (s.right() - 2, brow + 1), (CX + 3, brow + 1)],
            hair,
            60,
            z,
        );
        for x in [part + 2, part + 3] {
            l.polyline_fill(&[(x, brow), (x, brow + 1)], hair.at(Tone::Base), z.lo);
        }
    }
    let lock_to = match style {
        Hair::Long | Hair::Wet => r.top + 2 + r.trail.1,
        Hair::Short | Hair::Curlers | Hair::Bun | Hair::Pigtails => r.eye_y() + 1,
        Hair::Cropped | Hair::Bald => r.eye_y() - 1,
    };
    let wide = if long(d) { 3 } else { 2 };
    let (wl, wr) = if turned { (wide + 1, (wide - 1).max(1)) } else { (wide, wide) };
    if long(d) {
        // Long locks frame the face to the jaw, then spread out over the shoulders a px each
        // side and end in points, the ends a frame behind the body.
        let (jaw, sway) = (s.bottom() - 2, r.trail.0);
        let (xl, xr) = (s.x - 2 + wl, s.right() + 1 - wr);
        l.polygon_lit(
            &[
                (s.x - 1, brow - 1),
                (xl, brow - 1),
                (xl, jaw),
                (xl - 1, lock_to - 1),
                (xl - 1 + sway, lock_to),
                (s.x - 2 + sway, lock_to),
                (s.x - 2, jaw + 1),
                (s.x - 1, jaw),
            ],
            hair,
            100,
            z,
        );
        // Turned three quarters, the far lock comes in under the cheek to meet the jaw.
        let tuck = i32::from(turned);
        l.polygon_lit(
            &[
                (xr, brow - 1),
                (s.right(), brow - 1),
                (s.right(), jaw),
                (s.right() + 1, jaw + 1),
                (s.right() + 1 + sway, lock_to),
                (xr + 1 + sway, lock_to),
                (xr + 1, lock_to - 1),
                (xr - tuck, jaw + 1),
                (xr - tuck, jaw - 2),
                (xr, jaw - 3),
            ],
            hair,
            100,
            z,
        );
        points(&mut l, s.x - 2 + sway, s.right() + 1 + sway, lock_to, 1);
    } else {
        l.polygon_lit(
            &[(s.x - 1, brow - 1), (s.x - 2 + wl, brow - 1), (s.x - 2 + wl, lock_to - 1), (s.x - 1, lock_to)],
            hair,
            100,
            z,
        );
        l.polygon_lit(
            &[
                (s.right() + 1 - wr, brow - 1),
                (s.right(), brow - 1),
                (s.right(), lock_to),
                (s.right() + 1 - wr, lock_to - 1),
            ],
            hair,
            100,
            z,
        );
    }
    match style {
        Hair::Bun => l.ellipse_lit(Rect::new(CX - 4, s.y - 5, 8, 6), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => {
            for x0 in [s.x - 4, s.right()] {
                l.ellipse_lit(Rect::new(x0, r.eye_y() - 1, 4, 8), hair, z);
            }
        }
        _ => {}
    }
    l.retone(hair, HAIR);
    if style != Hair::Cropped {
        sheen(&mut l, hair, s.x - 1, s.right(), s.y - 2);
        if style == Hair::Bun {
            sheen(&mut l, hair, CX - 4, CX + 3, s.y - 6);
        }
        strand(&mut l, hair, (part, s.y + 1), (part, brow));
        strand(&mut l, hair, (part + 4, s.y), (part + 6, brow));
        if long(d) {
            strand(&mut l, hair, (s.x, brow + 1), (s.x - 1, lock_to - 2));
            strand(&mut l, hair, (s.right() - 1, brow + 1), (s.right(), lock_to - 2));
            // A lit streak down the near lock, where it turns to the light.
            for y in brow + 2..s.bottom() - 2 {
                l.tint(s.x - 1, y, hair, if y < brow + 5 { Tone::Light } else { Tone::Lift });
            }
        }
    }
    c.stamp(&l, 0, 0);
    if style == Hair::Curlers {
        for k in 0..3 {
            c.disc_lit(CX - 5 + 5 * k, s.y - 1, 1, Ramp::ClothLinen, Z::flat(z.hi + 2));
        }
    }
}

/// The whole head of hair from behind, over the coat, the pack and the neck.
pub fn whole_up(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let style = d.look.head.hair;
    if hides_hair(d) || style == Hair::Bald {
        if style == Hair::Bald {
            c.ellipse_lit(s, d.skin, relief::SKULL);
        } else {
            // Under a scarf or a helmet: the back of the head is its hair, cut short.
            let mut l = Canvas::new(W, H);
            l.ellipse_lit(s, d.hair, relief::SKULL);
            lay(c, l, d);
        }
        if style == Hair::Bald && !hides_hair(d) {
            let mut l = Canvas::new(W, H);
            l.polygon_lit(
                &[
                    (s.x, r.eye_y() + 1),
                    (s.right() - 1, r.eye_y() + 1),
                    (s.right() - 2, s.bottom() - 2),
                    (s.x + 1, s.bottom() - 2),
                ],
                d.hair,
                100,
                relief::HAIR,
            );
            lay(c, l, d);
        }
        return;
    }
    let hair = d.hair;
    let z = relief::HAIR;
    // Three quarters from behind: the back of the head turns toward the screen's left, and on
    // the right a sliver of cheek and jaw shows past the hair.
    let turned = r.facing == Facing::UpRight;
    let k = -i32::from(turned);
    let mid = CX + 2 * k;
    let mut l = Canvas::new(W, H);
    l.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 2 - i32::from(turned), s.h + 3), hair, z);
    let mut bottom = s.bottom() + 1;
    if long(d) {
        bottom = r.top + 3 + r.trail.1;
        l.polygon_lit(
            &[
                (s.x + k, s.y + 5),
                (s.right() - 1 + k, s.y + 5),
                (s.right() - 2 + 2 * k, bottom - 1),
                (mid, bottom),
                (mid - 1, bottom),
                (s.x + 1 + k, bottom - 1),
            ],
            hair,
            110,
            z,
        );
    }
    match style {
        Hair::Bun => l.ellipse_lit(Rect::new(CX - 4, s.y - 4, 8, 7), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => {
            for x0 in [s.x - 4, s.right()] {
                l.ellipse_lit(Rect::new(x0, r.eye_y() - 1, 4, 8), hair, z);
            }
        }
        _ => {}
    }
    l.retone(hair, HAIR);
    if style != Hair::Cropped {
        sheen(&mut l, hair, s.x - 1, s.right(), s.y - 2);
        strand(&mut l, hair, (mid - 1, s.y + 1), (mid - 1, bottom - 1));
        strand(&mut l, hair, (mid - 4, s.y + 3), (mid - 5, bottom - 2));
        strand(&mut l, hair, (mid + 3, s.y + 3), (mid + 4, bottom - 2));
    }
    c.stamp(&l, 0, 0);
    if turned {
        // The cheek past the hair, from under the eye to the jaw: two px, one past long hair,
        // lit on its edge toward the light, in shade toward the jaw.
        let ey = r.eye_y();
        let w = 2;
        for y in ey + 1..s.bottom() - 1 {
            let x1 = s.right() - 1 - i32::from(y >= s.bottom() - 3);
            for x in x1 + 1 - w..=x1 {
                let t = if y >= s.bottom() - 3 { Tone::Mid } else { Tone::Base };
                c.fill_rect(Rect::new(x, y, 1, 1), d.skin.at(t), relief::SKULL.lo);
            }
        }
    }
    if style == Hair::Curlers {
        for k in 0..3 {
            c.disc_lit(CX - 5 + 5 * k, s.y - 1, 1, Ramp::ClothLinen, Z::flat(z.hi + 2));
        }
    }
}

/// Long hair seen from the side, behind the shoulder: drawn before the body. It streams back a
/// frame behind the walker.
pub fn back_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    if !long(d) || hides_hair(d) {
        return;
    }
    let s = r.skull;
    let mut l = Canvas::new(W, H);
    let b = r.top + 4 + r.trail.1;
    let t = r.trail.0.min(0);
    l.polygon_lit(&[(s.x, s.y + 5), (s.x + 6, s.y + 5), (s.x + 5 + t, b), (s.x + 1 + t, b)], d.hair, 110, relief::FAR);
    l.retone(d.hair, HAIR);
    l.shade(Rect::new(s.x - 2, s.y + 5, 10, b - s.y), d.hair, 1);
    c.stamp(&l, 0, 0);
}

/// The hair from the side, facing east: a cap to the brow, the back of the head to the nape,
/// the ear showing where short hair stops.
pub fn side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    // Under a helmet the hair shows at the back of the head, cut short.
    let helmet = d.look.head.hat == jane_data::Hat::Helmet;
    let style = if helmet && d.look.head.hair != Hair::Bald { Hair::Cropped } else { d.look.head.hair };
    if hides_hair(d) && !helmet {
        return;
    }
    let hair = d.hair;
    let z = relief::HAIR;
    let brow = r.brow_y();
    let mut l = Canvas::new(W, H);
    if style == Hair::Bald {
        // What is left: round the back of the head, behind the ear.
        l.ellipse_lit(Rect::new(s.x - 1, r.eye_y() - 1, 5, 6), hair, z);
        lay(c, l, d);
        ear(c, d, r);
        return;
    }
    let cap_to = if style == Hair::Cropped { brow - 1 } else { brow };
    l.set_clip(Some(Rect::new(0, 0, W, cap_to + 1)));
    l.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 2), hair, z);
    l.set_clip(None);
    let nape = if long(d) { r.top + 1 + r.trail.1 } else { s.bottom() - 3 };
    l.polygon_lit(&[(s.x - 1, brow - 1), (s.x + 5, brow - 1), (s.x + 4, nape), (s.x, nape)], hair, 100, z);
    if style != Hair::Cropped {
        // A fringe lock over the brow, ahead of the ear.
        l.polygon_lit(&[(s.right() - 7, brow), (s.right() - 3, brow), (s.right() - 5, brow + 1)], hair, 60, z);
    }
    match style {
        Hair::Bun => l.ellipse_lit(Rect::new(s.x - 3, s.y - 3, 7, 7), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => l.ellipse_lit(Rect::new(s.x - 2, r.eye_y() - 1, 5, 8), hair, z),
        _ => {}
    }
    l.retone(hair, HAIR);
    if style != Hair::Cropped {
        sheen(&mut l, hair, s.x - 1, s.right(), s.y - 2);
        strand(&mut l, hair, (s.x + 7, s.y), (s.x + 3, brow + 1));
        strand(&mut l, hair, (s.x + 2, brow), (s.x + 1, nape - 1));
    }
    c.stamp(&l, 0, 0);
    if !long(d) {
        ear(c, d, r);
    }
    if style == Hair::Curlers {
        for k in 0..3 {
            c.disc_lit(s.x + 2 + 4 * k, s.y - 1, 1, Ramp::ClothLinen, Z::flat(z.hi + 2));
        }
    }
}

fn ear(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    c.ellipse_lit(Rect::new(s.x + 4, r.eye_y(), 3, 4), d.skin, Z::new(relief::HAIR.lo, relief::HAIR.hi));
}
