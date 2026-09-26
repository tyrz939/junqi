//! Hair (ART.md §2.1): drawn as `polygon_lit` and `soft_ellipse` masses on a layer of its own,
//! its tones held between `deep` and `light` so it reads as hair and not as a lit ball, a few
//! `strokes(hair)` for its lie, and stamped on the figure as one part, so the pieces of one
//! head of hair never seam against each other.

use jane_core::grid::Rect;
use jane_data::Hair;

use super::draw::{CX, Rig, hides_hair, relief};
use super::{Dress, H, W};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ramp, Tone};

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

/// Finish a hair layer and lay it on the figure.
fn lay(c: &mut Canvas, mut l: Canvas, d: &Dress, strokes: Rect, seed: u32, salt_k: u32) {
    let (lo, hi) = if d.look.head.hair == Hair::Wet { (Tone::Deep, Tone::Mid) } else { (Tone::Deep, Tone::Lift) };
    tame(&mut l, d.hair, lo, hi);
    l.strokes(strokes, d.hair, StrokeKind::Hair, 4, h32(seed, salt_k, salt::PERSON));
    tame(&mut l, d.hair, lo, hi.step(1));
    c.stamp(&l, 0, 0);
}

/// Long hair behind the head, seen from the front: it shows beside the neck and over the
/// shoulders. Drawn first.
pub fn back_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    if !long(d) || hides_hair(d) {
        return;
    }
    let s = r.skull;
    let mut l = Canvas::new(W, H);
    let b = r.top + 3;
    l.polygon_lit(&[(s.x, s.y + 5), (s.right() - 1, s.y + 5), (s.right(), b), (s.x - 1, b)], d.hair, 110, relief::FAR);
    tame(&mut l, d.hair, Tone::Deep, Tone::Base);
    c.stamp(&l, 0, 0);
}

/// The hair over the skull from the front: a cap to the brow, a fringe swept from a parting
/// left of centre, and locks by the style.
pub fn front_down(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
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
            l.soft_ellipse(Rect::new(x0, r.eye_y() - 2, 3, 5), hair, z);
        }
        lay(c, l, d, Rect::new(0, 0, 0, 0), seed, 1);
        return;
    }
    let cap_to = if style == Hair::Cropped { brow - 1 } else { brow };
    l.set_clip(Some(Rect::new(0, 0, W, cap_to + 1)));
    l.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 2), hair, z);
    l.set_clip(None);
    if style != Hair::Cropped {
        // The fringe: a short tuft left of the parting, a longer sweep right of it.
        l.polygon_lit(&[(s.x + 1, brow), (CX - 3, brow), (s.x + 1, brow + 1)], hair, 60, z);
        l.polygon_lit(
            &[(CX - 1, brow), (s.right() - 2, brow), (s.right() - 2, brow + 1), (CX + 3, brow + 1)],
            hair,
            60,
            z,
        );
    }
    let lock_to = match style {
        Hair::Long | Hair::Wet => r.top + 2,
        Hair::Short | Hair::Curlers | Hair::Bun | Hair::Pigtails => r.eye_y() + 1,
        Hair::Cropped | Hair::Bald => r.eye_y() - 1,
    };
    let wide = if long(d) { 3 } else { 2 };
    l.polygon_lit(
        &[(s.x - 1, brow - 1), (s.x - 2 + wide, brow - 1), (s.x - 2 + wide, lock_to - 1), (s.x - 1, lock_to)],
        hair,
        100,
        z,
    );
    l.polygon_lit(
        &[
            (s.right() + 1 - wide, brow - 1),
            (s.right(), brow - 1),
            (s.right(), lock_to),
            (s.right() + 1 - wide, lock_to - 1),
        ],
        hair,
        100,
        z,
    );
    match style {
        Hair::Bun => l.soft_ellipse(Rect::new(CX - 4, s.y - 5, 8, 6), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => {
            for x0 in [s.x - 4, s.right()] {
                l.soft_ellipse(Rect::new(x0, r.eye_y() - 1, 4, 8), hair, z);
            }
        }
        _ => {}
    }
    lay(c, l, d, Rect::new(s.x - 1, s.y - 2, s.w + 2, lock_to - s.y + 3), seed, 1);
    if style == Hair::Curlers {
        for k in 0..3 {
            c.disc_lit(CX - 5 + 5 * k, s.y - 1, 1, Ramp::ClothLinen, Z::flat(z.hi + 2));
        }
    }
}

/// The whole head of hair from behind, over the coat, the pack and the neck.
pub fn whole_up(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
    let s = r.skull;
    let style = d.look.head.hair;
    if hides_hair(d) || style == Hair::Bald {
        let back = if style == Hair::Bald { d.skin } else { d.hair };
        c.ellipse_lit(s, back, relief::SKULL);
        if style == Hair::Bald && !hides_hair(d) {
            let mut l = Canvas::new(W, H);
            l.polygon_lit(
                &[
                    (s.x, r.eye_y() - 1),
                    (s.right() - 1, r.eye_y() - 1),
                    (s.right() - 2, s.bottom() - 2),
                    (s.x + 1, s.bottom() - 2),
                ],
                d.hair,
                100,
                relief::HAIR,
            );
            lay(c, l, d, Rect::new(0, 0, 0, 0), seed, 2);
        }
        return;
    }
    let hair = d.hair;
    let z = relief::HAIR;
    let mut l = Canvas::new(W, H);
    l.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 3), hair, z);
    let mut bottom = s.bottom() + 1;
    if long(d) {
        bottom = r.top + 3;
        l.polygon_lit(
            &[
                (s.x, s.y + 5),
                (s.right() - 1, s.y + 5),
                (s.right() - 2, bottom - 1),
                (CX, bottom),
                (CX - 1, bottom),
                (s.x + 1, bottom - 1),
            ],
            hair,
            110,
            z,
        );
    }
    match style {
        Hair::Bun => l.soft_ellipse(Rect::new(CX - 4, s.y - 4, 8, 7), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => {
            for x0 in [s.x - 4, s.right()] {
                l.soft_ellipse(Rect::new(x0, r.eye_y() - 1, 4, 8), hair, z);
            }
        }
        _ => {}
    }
    lay(c, l, d, Rect::new(s.x - 1, s.y - 2, s.w + 2, bottom - s.y + 3), seed, 2);
    if style == Hair::Curlers {
        for k in 0..3 {
            c.disc_lit(CX - 5 + 5 * k, s.y - 1, 1, Ramp::ClothLinen, Z::flat(z.hi + 2));
        }
    }
}

/// Long hair seen from the side, behind the shoulder: drawn before the body.
pub fn back_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    if !long(d) || hides_hair(d) {
        return;
    }
    let s = r.skull;
    let mut l = Canvas::new(W, H);
    let b = r.top + 4;
    l.polygon_lit(&[(s.x, s.y + 5), (s.x + 6, s.y + 5), (s.x + 5, b), (s.x + 1, b)], d.hair, 110, relief::FAR);
    tame(&mut l, d.hair, Tone::Deep, Tone::Base);
    c.stamp(&l, 0, 0);
}

/// The hair from the side, facing east: a cap to the brow, the back of the head to the nape,
/// the ear showing where short hair stops.
pub fn side(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
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
        l.soft_ellipse(Rect::new(s.x + 1, r.eye_y() - 2, 5, 5), hair, z);
        lay(c, l, d, Rect::new(0, 0, 0, 0), seed, 3);
        ear(c, d, r);
        return;
    }
    let cap_to = if style == Hair::Cropped { brow - 1 } else { brow };
    l.set_clip(Some(Rect::new(0, 0, W, cap_to + 1)));
    l.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 2), hair, z);
    l.set_clip(None);
    let nape = if long(d) { r.top + 1 } else { s.bottom() - 3 };
    let back = 5;
    l.polygon_lit(&[(s.x - 1, brow - 1), (s.x + back, brow - 1), (s.x + back - 1, nape), (s.x, nape)], hair, 100, z);
    match style {
        Hair::Bun => l.soft_ellipse(Rect::new(s.x - 3, s.y - 3, 7, 7), hair, Z::new(z.hi, z.hi + 1)),
        Hair::Pigtails => l.soft_ellipse(Rect::new(s.x - 2, r.eye_y() - 1, 5, 8), hair, z),
        _ => {}
    }
    lay(c, l, d, Rect::new(s.x - 1, s.y - 2, s.w + 2, nape - s.y + 3), seed, 3);
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
    c.ellipse_lit(Rect::new(s.x + 6, r.eye_y(), 3, 4), d.skin, Z::new(relief::HAIR.lo, relief::HAIR.hi));
}
