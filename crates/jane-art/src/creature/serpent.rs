//! `serpent_head` (the Snake): ART.md §2.2. The sprite is the snake's forepart: a coil of it
//! on the ground and the neck reared out of the coil to a wedge of a head, heavy at the jaw,
//! with gold eyes slit by a pupil and a forked tongue. The rest of it is its trail: the
//! presenter lays [`segments`] along it from the tail to the neck, each a lit sphere of scales,
//! smaller toward the tail, so the body is as long as the sim says and slithers as it moves.
//!
//! The walk names carry the slither, the neck swaying a px either side; the idle pair rests the
//! head on the coil and flicks the tongue. It strikes in three beats: drawn back, struck low
//! with the jaws open on their fangs, recovered. Dead, it lies slack in an S.

use jane_core::grid::Rect;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const COIL: Z = Z::new(2, 6);
    pub const NECK: Z = Z::new(5, 9);
    pub const HEAD: Z = Z::new(8, 11);
}

/// The eyes: gold, a slit of a pupil.
const EYE: Ramp = Ramp::ClothMustard;

/// One beat's pose: the head's offset (x, y), the jaws open, the tongue out.
fn pose(beat: Beat) -> ((i32, i32), bool, bool) {
    match beat {
        Beat::Walk(k) => {
            (([0, 1, 2, 1, 0, -1][usize::from(k % 6)], [0, 0, 1, 0, 0, 1][usize::from(k % 6)]), false, false)
        }
        Beat::Breathe => ((0, 1), false, true),
        Beat::Idle(0) => ((0, 7), false, false),
        Beat::Idle(_) => ((0, 7), false, true),
        Beat::Hurt => ((-2, -1), true, false),
        Beat::Attack(0) => ((-3, -3), false, false),
        Beat::Attack(1) => ((5, 5), true, false),
        Beat::Attack(_) => ((1, 1), false, true),
        Beat::Dead => ((0, 0), false, false),
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    if beat == Beat::Dead {
        dead(c, k);
        return;
    }
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let (off, open, tongue) = pose(beat);
    coil(c, k, ax, ay);
    match facing {
        Facing::Side => {
            // The neck in an S up from the back of the coil, the head facing east.
            let (hx, hy) = (ax + 4 + off.0, ay - 20 + off.1);
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline_fill(
                &[
                    (ax - 5, ay - 6),
                    (ax - 1, ay - 7),
                    (ax + 1, ay - 13),
                    (hx - 1, hy + 4),
                    (hx - 5, hy + 3),
                    (ax - 3, ay - 13),
                    (ax - 7, ay - 8),
                ],
                Ix::INK,
                1,
            );
            c.inflate(&m, k.body, 3, relief::NECK);
            belly_side(c, k, ax, ay, hx, hy);
            head_side(c, k, hx, hy, open, tongue);
        }
        Facing::Down | Facing::Up => {
            let toward = facing == Facing::Down;
            let (hx, hy) = (ax + off.0, ay - 21 + off.1 + i32::from(!toward));
            // The neck reared in an S out of the coil, thinner than the coil, to the head.
            let neck = [(ax - 2, ay - 7), (ax + 3, ay - 11), (ax + off.0 - 2, ay - 15), (hx, hy + 5)];
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline(&neck, Ix::INK, 5, 1);
            c.inflate(&m, k.body, 2, relief::NECK);
            if toward && k.belly != k.body {
                // The pale throat down its front, scaled across in bands.
                c.polyline(&neck, k.belly.at(Tone::Base), 2, relief::NECK.hi);
                for y in (hy + 6..ay - 7).step_by(2) {
                    for x in 0..c.w() {
                        c.tint(x, y, k.belly, Tone::Mid);
                    }
                }
            }
            head_front(c, k, hx, hy, toward, open, tongue);
        }
        Facing::DownRight | Facing::UpRight => {
            let toward = facing == Facing::DownRight;
            let (hx, hy) = (ax + 3 + off.0, ay - 21 + off.1 + i32::from(!toward));
            // The neck reared out of the coil and leaning into the turn.
            let neck = [(ax - 3, ay - 7), (ax + 2, ay - 11), (ax + off.0, ay - 15), (hx - 1, hy + 5)];
            let mut m = Canvas::new(c.w(), c.h());
            m.polyline(&neck, Ix::INK, 5, 1);
            c.inflate(&m, k.body, 2, relief::NECK);
            if toward && k.belly != k.body {
                // The throat's pale scales down the near side of the neck.
                let throat: Vec<(i32, i32)> = neck.iter().map(|&(x, y)| (x + 1, y)).collect();
                c.polyline(&throat, k.belly.at(Tone::Base), 2, relief::NECK.hi);
                for y in (hy + 6..ay - 7).step_by(2) {
                    for x in 0..c.w() {
                        c.tint(x, y, k.belly, Tone::Mid);
                    }
                }
            }
            head_turned(c, k, hx, hy, toward, open, tongue);
        }
    }
}

/// The head turned an eighth: the wedge's snout down and to the right (`toward`) with both gold
/// eyes, the far one close to the snout, the nostrils at its tip; or up and to the right, the
/// crown's pattern toward us and the right eye at its edge. The tongue flicks along the facing.
fn head_turned(c: &mut Canvas, k: &Coat, hx: i32, hy: i32, toward: bool, open: bool, tongue: bool) {
    let mut m = Canvas::new(c.w(), c.h());
    let pts: [(i32, i32); 6] = if toward {
        [(hx - 6, hy), (hx + 4, hy - 1), (hx + 7, hy + 2), (hx + 5, hy + 5), (hx, hy + 6), (hx - 5, hy + 3)]
    } else {
        [(hx - 5, hy + 1), (hx, hy - 1), (hx + 6, hy - 2), (hx + 7, hy + 1), (hx + 3, hy + 4), (hx - 4, hy + 5)]
    };
    m.polyline_fill(&pts, Ix::INK, 1);
    c.inflate(&m, k.body, 2, relief::HEAD);
    let z = relief::HEAD.hi;
    let r = Ramp::ClothRed;
    if !toward {
        c.dye_poly(&[(hx - 1, hy), (hx + 2, hy + 1), (hx, hy + 4), (hx - 3, hy + 2)], k.body, k.mark);
        eye(c, k, hx + 4, hy - 1, z);
        if tongue {
            c.line((hx + 7, hy - 1), (hx + 9, hy - 3), r.at(Tone::Base), 1, z);
            c.dot(hx + 10, hy - 3, r.at(Tone::Base), z);
            c.dot(hx + 9, hy - 4, r.at(Tone::Base), z);
        }
        return;
    }
    c.hline(hx - 4, hx + 2, hy, k.body.at(Tone::Light), z);
    c.dye_poly(&[(hx - 1, hy), (hx + 1, hy + 2), (hx - 1, hy + 4), (hx - 3, hy + 2)], k.body, k.mark);
    eye(c, k, hx - 4, hy + 1, z);
    eye(c, k, hx + 3, hy, z);
    c.dot(hx + 5, hy + 3, Ix::INK, z);
    c.dot(hx + 4, hy + 4, Ix::INK, z);
    if open {
        c.polyline_fill(&[(hx + 5, hy + 5), (hx - 1, hy + 6), (hx + 3, hy + 9)], Ramp::ClothRose.at(Tone::Base), z);
        c.vline(hx + 4, hy + 5, hy + 7, Ramp::Bone.at(Tone::High), z);
        c.vline(hx, hy + 6, hy + 7, Ramp::Bone.at(Tone::High), z);
    }
    if tongue {
        c.line((hx + 6, hy + 5), (hx + 8, hy + 7), r.at(Tone::Base), 1, z);
        c.dot(hx + 9, hy + 7, r.at(Tone::Base), z);
        c.dot(hx + 8, hy + 8, r.at(Tone::Base), z);
    }
}

/// The coil on the ground: a thick ring of it, its pattern down its back.
fn coil(c: &mut Canvas, k: &Coat, ax: i32, ay: i32) {
    let mut m = Canvas::new(c.w(), c.h());
    let outer = Rect::new(ax - 12, ay - 10, 24, 11);
    m.ellipse(outer, Ix::INK, 1);
    c.inflate(&m, k.body, 4, relief::COIL);
    // The inner turn in shade: the coil lies on itself.
    let inner = Rect::new(ax - 6, ay - 7, 12, 5);
    c.shade(inner, k.body, 2);
    c.hline(inner.x + 2, inner.right() - 3, inner.y + 1, k.body.at(Tone::Light), relief::COIL.hi);
    diamonds(c, k, outer);
    c.strokes(outer, k.body, StrokeKind::Feather, 10, h32(k.seed, 4, salt::STROKES));
}

/// The pattern: dark-edged diamonds in the mark's colour along the body.
fn diamonds(c: &mut Canvas, k: &Coat, r: Rect) {
    let mut x = r.x + 3;
    let y = r.y + 1;
    while x + 3 < r.right() - 2 {
        c.dye_poly(&[(x + 2, y), (x + 4, y + 2), (x + 2, y + 4), (x, y + 2)], k.body, k.mark);
        x += 6;
    }
}

fn belly_side(c: &mut Canvas, k: &Coat, ax: i32, ay: i32, hx: i32, hy: i32) {
    if k.belly == k.body {
        return;
    }
    c.dye_poly(
        &[(ax - 1, ay - 7), (ax + 1, ay - 12), (hx - 2, hy + 5), (hx - 1, hy + 3), (ax + 2, ay - 12), (ax, ay - 6)],
        k.body,
        k.belly,
    );
}

fn eye(c: &mut Canvas, k: &Coat, x: i32, y: i32, z: u8) {
    c.set_emitting(k.eye_emits);
    c.fill_rect(Rect::new(x, y, 2, 2), EYE.at(Tone::High), z);
    c.set_emitting(false);
    c.vline(x + 1, y, y + 1, Ix::INK, z);
}

fn head_front(c: &mut Canvas, k: &Coat, hx: i32, hy: i32, toward: bool, open: bool, tongue: bool) {
    // The wedge: broad at the jaw's hinge, narrowing to the snout toward the viewer.
    let mut m = Canvas::new(c.w(), c.h());
    // Flat and broad at the hinge of the jaw, a blunt snout toward the viewer.
    let (w, h) = (13, 6);
    m.polyline_fill(
        &[
            (hx - w / 2, hy),
            (hx + w / 2, hy),
            (hx + w / 2 - 1, hy + 3),
            (hx + 2, hy + h),
            (hx - 2, hy + h),
            (hx - w / 2 + 1, hy + 3),
        ],
        Ix::INK,
        1,
    );
    c.inflate(&m, k.body, 2, relief::HEAD);
    let z = relief::HEAD.hi;
    if !toward {
        // From behind: the pattern over the crown.
        c.dye_poly(&[(hx, hy + 1), (hx + 3, hy + 4), (hx, hy + 7), (hx - 3, hy + 4)], k.body, k.mark);
        return;
    }
    // The brow scales lit, the eyes at the corners, the nostrils at the snout.
    c.hline(hx - 3, hx + 2, hy, k.body.at(Tone::Light), z);
    c.dye_poly(&[(hx, hy), (hx + 2, hy + 2), (hx, hy + 4), (hx - 2, hy + 2)], k.body, k.mark);
    eye(c, k, hx - 6, hy + 1, z);
    eye(c, k, hx + 4, hy + 1, z);
    c.dot(hx - 1, hy + h - 1, Ix::INK, z);
    c.dot(hx, hy + h - 1, Ix::INK, z);
    if open {
        // Struck: the jaw dropped, the mouth pale, two fangs.
        c.fill_rect(Rect::new(hx - 3, hy + h, 6, 4), Ramp::ClothRose.at(Tone::Base), z);
        c.hline(hx - 3, hx + 2, hy + h + 3, k.body.at(Tone::Base), z);
        for x in [hx - 3, hx + 2] {
            c.vline(x, hy + h, hy + h + 2, Ramp::Bone.at(Tone::High), z);
        }
    }
    if tongue {
        let r = Ramp::ClothRed;
        c.vline(hx, hy + h, hy + h + 2, r.at(Tone::Base), z);
        c.dot(hx - 1, hy + h + 3, r.at(Tone::Base), z);
        c.dot(hx + 1, hy + h + 3, r.at(Tone::Base), z);
    }
}

fn head_side(c: &mut Canvas, k: &Coat, hx: i32, hy: i32, open: bool, tongue: bool) {
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(
        &[(hx - 6, hy + 1), (hx + 1, hy), (hx + 7, hy + 3), (hx + 7, hy + 5), (hx + 1, hy + 6), (hx - 6, hy + 6)],
        Ix::INK,
        1,
    );
    c.inflate(&m, k.body, 2, relief::HEAD);
    let z = relief::HEAD.hi;
    c.hline(hx - 4, hx + 4, hy + 1, k.body.at(Tone::Light), z);
    eye(c, k, hx + 1, hy + 2, z);
    // The mouth's line back to the hinge.
    c.line((hx + 6, hy + 5), (hx - 4, hy + 5), k.body.at(Tone::Deep), 1, z);
    if k.belly != k.body {
        c.hline(hx - 5, hx + 4, hy + 6, k.belly.at(Tone::Base), z);
    }
    if open {
        c.polyline_fill(&[(hx + 6, hy + 5), (hx - 3, hy + 6), (hx + 5, hy + 9)], Ramp::ClothRose.at(Tone::Base), z);
        c.vline(hx + 5, hy + 5, hy + 7, Ramp::Bone.at(Tone::High), z);
    }
    if tongue {
        let r = Ramp::ClothRed;
        c.hline(hx + 8, hx + 10, hy + 4, r.at(Tone::Base), z);
        c.dot(hx + 11, hy + 3, r.at(Tone::Base), z);
        c.dot(hx + 11, hy + 5, r.at(Tone::Base), z);
    }
}

/// Slack in an S on the ground, the head on its side.
fn dead(c: &mut Canvas, k: &Coat) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let mut m = Canvas::new(c.w(), c.h());
    let pts = [(ax - 13, ay - 2), (ax - 7, ay - 6), (ax - 1, ay - 3), (ax + 5, ay - 6), (ax + 11, ay - 3)];
    m.polyline(&pts, Ix::INK, 5, 1);
    c.inflate(&m, k.body, 2, Z::new(2, 4));
    diamonds(c, k, Rect::new(ax - 13, ay - 8, 26, 6));
    let mut h = Canvas::new(c.w(), c.h());
    h.ellipse(Rect::new(ax + 10, ay - 6, 6, 5), Ix::INK, 1);
    c.inflate(&h, k.body, 2, Z::new(3, 5));
    c.hline(ax + 12, ax + 14, ay - 4, k.body.at(Tone::Deep), 5);
}

/// The body's segments for the trail, largest (at the neck) first: lit spheres of scales with
/// the pattern on their backs and the belly under them. Drawn by the presenter along the trail.
pub(crate) fn segments(k: &Coat) -> Vec<Canvas> {
    (0..6)
        .map(|i| {
            let d = 12 - i;
            let mut c = Canvas::new(d + 2, d + 2);
            let mut m = Canvas::new(d + 2, d + 2);
            m.ellipse(Rect::new(1, 1, d, d - 1), Ix::INK, 1);
            c.inflate(&m, k.body, d / 3, Z::new(1, (d / 2) as u8 + 1));
            c.strokes(
                Rect::new(1, 1, d, d),
                k.body,
                StrokeKind::Feather,
                12,
                h32(k.seed, 20 + i as u32, salt::STROKES),
            );
            let mid = (d + 2) / 2;
            c.dye_poly(&[(mid, 2), (mid + 2, 4), (mid, 6), (mid - 2, 4)], k.body, k.mark);
            if k.belly != k.body {
                c.dye_ellipse(Rect::new(3, d - 2, d - 4, 3), k.body, k.belly);
            }
            c
        })
        .collect()
}
