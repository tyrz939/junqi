//! The `bone` skin (ART.md §2.1, §8 step 4): the skeleton, a townsperson who was outside when
//! the county changed its mind (STORY.md §2.3), still in what is left of their coat. The skull
//! is the head's own lit ellipse with deep sockets (a faint ember in each when the look emits
//! its eyes), a nasal hollow and a row of teeth over the jaw's line; the neck is two px of
//! vertebrae, the hands three px of knuckles, bare legs two-px shins with a knee's knob. The
//! coat hangs open on a ribcage, its hem torn into tongues, and a tear in the back shows the
//! spine.

use jane_core::grid::Rect;

use super::Dress;
use super::draw::{CX, Rig, relief};
use crate::canvas::{Canvas, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

/// A socket: three px wide and two deep, in `K`, with a pinpoint of ember light at night.
fn socket(c: &mut Canvas, d: &Dress, x: i32, y: i32, w: i32, z: u8) {
    c.fill_rect(Rect::new(x, y, w, 2), Ix::SEAM, z);
    c.dot(x + w / 2, y - 1, Ix::SEAM, z);
    if d.eye_emits {
        c.set_emitting(true);
        c.dot(x + w / 2, y + 1, Ramp::Ember.at(Tone::High), z);
        c.set_emitting(false);
    }
}

/// Carve the head the composer drew into a skull: under the cheekbones only the jaw is left,
/// narrowing to the chin (`front`: facing east, the jaw at the front of the face).
fn carve(c: &mut Canvas, r: &Rig, front: Option<i32>) {
    let s = r.skull;
    let ey = r.eye_y();
    // The neck's two columns stay; three quarters on, the jaw turns a px with the face.
    let neck = if front.is_some() { CX - 3 + r.lean } else { CX - 1 };
    let jc = CX + r.turn() / 2;
    for y in ey + 3..=s.bottom() + 1 {
        let half = if y <= ey + 4 { 5 } else { 4 };
        for x in s.x - 1..=s.right() {
            let out = match front {
                None => x < jc - half || x >= jc + half,
                Some(fx) => x < fx - half - 1,
            };
            if out && c.get(x, y).is_opaque() && y < r.top && !(neck..neck + 2).contains(&x) {
                c.clear_px(x, y);
            }
        }
    }
}

/// The skull facing the viewer, over the head the composer drew.
pub(super) fn face_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    carve(c, r, None);
    let s = r.skull;
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    let b = d.skin;
    // Three quarters on, the face turns a px toward the right.
    let cx = CX + r.turn() / 2;
    // The brow ridge lit, the temples and cheeks in shade under it.
    for x in s.x + 2..s.right() - 2 {
        c.tint(x, ey - 2, b, Tone::Lift);
    }
    c.shade(Rect::new(s.x, ey, 4, 5), b, 1);
    c.shade(Rect::new(s.right() - 4, ey - 2, 4, 7), b, 1);
    socket(c, d, cx - 6, ey, 3, z);
    socket(c, d, cx + 2, ey, 3, z);
    // The nasal hollow, and the teeth over the jaw's line.
    c.hline(cx - 1, cx, ey + 3, Ix::SEAM, z);
    c.dot(cx - 1, ey + 2, b.at(Tone::Shade), z);
    let t = ey + 4;
    c.hline(cx - 3, cx + 2, t, b.at(Tone::Light), z);
    for x in [cx - 2, cx + 1] {
        c.dot(x, t, Ix::SEAM, z);
    }
    c.hline(cx - 3, cx + 2, t + 1, b.at(Tone::Shade), z);
}

/// The skull in profile, facing east.
pub(super) fn face_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    let b = d.skin;
    let fx = s.right() - 4;
    carve(c, r, Some(s.right()));
    c.shade(Rect::new(s.x, ey - 1, 6, 7), b, 1);
    socket(c, d, fx - 1, ey, 3, z);
    c.dot(s.right() - 1, ey + 2, Ix::SEAM, z);
    c.dot(s.right() - 1, ey + 3, b.at(Tone::Shade), z);
    let t = ey + 4;
    c.hline(s.right() - 4, s.right() - 1, t, b.at(Tone::Light), z);
    c.dot(s.right() - 2, t, Ix::SEAM, z);
    c.hline(s.right() - 5, s.right() - 1, t + 1, b.at(Tone::Shade), z);
}

/// The back of the skull: plain bone with the seam where it meets the neck in shade.
pub(super) fn skull_back(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    c.ellipse_lit(s, d.skin, relief::SKULL);
    c.shade(Rect::new(s.x + 2, s.bottom() - 4, s.w - 4, 4), d.skin, 1);
}

/// A bony hand: three px of knuckles with the gap between the fingers.
pub(super) fn hand(c: &mut Canvas, d: &Dress, x: i32, y: i32, z: u8) {
    c.fill_rect(Rect::new(x, y, 3, 3), d.skin.at(Tone::Base), z);
    c.dot(x, y, d.skin.at(Tone::Lift), z);
    c.dot(x + 1, y + 2, Ix::SEAM, z);
}

/// A shin: a bone two px wide from `(x0, top)` to `(x1, bot)`, its lit side left, and the knee's
/// knob half way.
pub(super) fn shin(c: &mut Canvas, d: &Dress, x0: i32, top: i32, x1: i32, bot: i32, z: Z) {
    if bot < top {
        return;
    }
    c.line((x0, top), (x1, bot), d.skin.at(Tone::Lift), 1, z.hi);
    c.line((x0 + 1, top), (x1 + 1, bot), d.skin.at(Tone::Mid), 1, z.hi);
    let (kx, ky) = ((x0 + x1) / 2, (top + bot) / 2);
    c.fill_rect(Rect::new(kx - 1, ky, 4, 2), d.skin.at(Tone::Base), z.hi);
    c.hline(kx - 1, kx, ky, d.skin.at(Tone::Lift), z.hi);
}

/// Tear the hem of what was drawn in the coat's ramp into tongues, by the sprite's seed.
fn tear(c: &mut Canvas, d: &Dress, hem: i32, x0: i32, x1: i32) {
    for x in x0..=x1 {
        let h = h32(x as u32, hem as u32, salt::STROKES) % 5;
        let depth = match h {
            0 => 2,
            1 | 2 => 1,
            _ => 0,
        };
        for y in hem - depth + 1..=hem {
            if matches!(Ramp::of(c.get(x, y)), Some((r, _)) if r == d.coat) {
                c.clear_px(x, y);
            }
        }
    }
}

/// What is left of the coat, facing the viewer or away: the hem torn; open on a ribcage in
/// front, a tear over the spine behind.
pub(super) fn rags(c: &mut Canvas, d: &Dress, r: &Rig, facing_us: bool) {
    let t = r.top;
    tear(c, d, r.hem, CX - 10, CX + 9);
    let b = d.skin;
    let z = relief::FRONT;
    let cx = r.cx();
    if facing_us {
        // The ribcage, the sternum lit down the middle, the ribs in shade lines between.
        let cage = Rect::new(cx - 4, t + 1, 8, r.waist - t);
        c.ellipse_lit(cage, b, Z::new(z, z + 1));
        for y in (cage.y + 2..cage.bottom() - 1).step_by(2) {
            c.hline(cage.x + 1, cage.right() - 2, y, b.at(Tone::Shade), z + 1);
        }
        c.vline(cx - 1, cage.y + 1, cage.bottom() - 2, b.at(Tone::Light), z + 1);
        // The spine below it, to the pelvis.
        c.fill_rect(Rect::new(cx - 1, cage.bottom(), 2, r.hip - cage.bottom()), b.at(Tone::Mid), z);
    } else {
        // A tear in the back and the spine through it.
        let hole = Rect::new(cx - 3, t + 3, 6, r.waist - t - 1);
        c.ellipse(hole, Ix::SEAM, z);
        c.vline(cx - 1, hole.y, hole.bottom() - 1, b.at(Tone::Base), z + 1);
        for y in (hole.y + 1..hole.bottom() - 1).step_by(2) {
            c.dot(cx - 1, y, b.at(Tone::Light), z + 1);
        }
    }
}

/// The coat in rags from the side: the hem torn, a rib showing at the open front.
pub(super) fn rags_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    tear(c, d, r.hem, CX - 10, CX + 9);
    // Two ribs curving out of the open front, each a lit bone over its dark gap.
    let x1 = CX + r.lean + 3;
    let b = d.skin;
    for y in [r.top + 2, r.top + 5] {
        c.fill_rect(Rect::new(x1 - 2, y, 3, 2), b.at(Tone::Light), relief::FRONT);
        c.hline(x1 - 2, x1, y + 2, Ix::SEAM, relief::FRONT);
    }
}
