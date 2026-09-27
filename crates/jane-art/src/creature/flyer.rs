//! `flyer_bat` (bat), and the dispatch of `flyer_insect` to [`super::insect`]: ART.md §2.2. A
//! flyer hovers four rows over its anchor, where its shadow lies, and carries its wing beat
//! under the walk names. A bat is a furred body with its ears, and wings of stretched leather on
//! three fingers each, beating from a V to spread to down.

use jane_core::grid::Rect;
use jane_data::Anatomy;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 3);
    pub const BODY: Z = Z::new(4, 6);
    pub const NEAR: Z = Z::new(6, 8);
}

/// How far a flyer drifts on the beat, px east: it never hangs still in the air.
fn drift(beat: Beat) -> i32 {
    match beat {
        Beat::Walk(k) => [0, 0, 1, 1, 0, -1][usize::from(k % 6)],
        _ => 0,
    }
}

/// How far open the wings are (2 spread, 1 half, 0 closed, up over the back) and the body's
/// rise on the beat.
fn beat_wings(beat: Beat) -> (i32, i32) {
    match beat {
        Beat::Walk(k) => [(2, 0), (1, 1), (0, 1), (1, 0), (2, 0), (1, 1)][usize::from(k % 6)],
        Beat::Breathe => (1, 0),
        Beat::Idle(0) | Beat::Attack(0) => (0, -1),
        Beat::Idle(_) => (1, -1),
        Beat::Hurt => (1, 2),
        Beat::Attack(1) => (2, -2),
        Beat::Attack(_) => (1, 1),
        Beat::Dead => (2, 0),
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    if k.look.anatomy != Anatomy::Bat {
        super::insect::draw(c, k, facing, beat);
        return;
    }
    match beat {
        Beat::Dead => dead(c, k),
        _ => bat_frame(c, k, facing, beat),
    }
}

fn bat_frame(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let (open, rise) = beat_wings(beat);
    let (cx, cy) = (ax + drift(beat), ay - 4 - 7 - rise);
    let skin = k.belly;
    // Each wing: a shoulder, an elbow, three finger tips, the membrane between them.
    let tips = |side: i32| -> [(i32, i32); 4] {
        let s = side;
        match open {
            2 => [(cx + s * 3, cy - 2), (cx + s * 8, cy - 5), (cx + s * 14, cy - 3), (cx + s * 13, cy + 2)],
            1 => [(cx + s * 3, cy - 3), (cx + s * 7, cy - 9), (cx + s * 11, cy - 10), (cx + s * 12, cy - 5)],
            _ => [(cx + s * 3, cy), (cx + s * 7, cy + 4), (cx + s * 9, cy + 7), (cx + s * 6, cy + 7)],
        }
    };
    let side_view = facing == Facing::Side;
    // Turned (the diagonals), the wing swinging away is foreshortened and in the body's shade:
    // the right one facing down and to the right, the left one facing up and to the right. The
    // head comes round with the facing.
    let turned = matches!(facing, Facing::DownRight | Facing::UpRight);
    let away_side = if facing == Facing::DownRight { 1 } else { -1 };
    let turn_x = i32::from(turned);
    for side in [-1, 1] {
        let short = turned && side == away_side;
        let far = (side_view && side < 0) || short;
        let z = if far { relief::FAR } else { relief::NEAR };
        let t = tips(side).map(|(x, y)| if short { (cx + (x - cx) * 3 / 5, y + 1) } else { (x, y) });
        let shoulder = (cx + side, cy - 1);
        let mut m = Canvas::new(c.w(), c.h());
        let mut pts = vec![shoulder, t[0], t[1], t[2], t[3]];
        // The trailing edge scalloped back to the hip.
        pts.push((t[3].0 - side * 3, t[3].1 + 1));
        pts.push((cx + side * 2, cy + 3));
        m.polyline_fill(&pts, Ix::INK, 1);
        c.inflate(&m, skin, 1, z);
        for &f in &t[1..] {
            c.line(t[0], f, k.body.at(Tone::Shade), 1, z.hi);
        }
        c.line(shoulder, t[0], k.body.at(Tone::Base), 1, z.hi);
        if far {
            c.shade(Rect::new(cx - 16, cy - 12, 16, 22), skin, 1);
        }
    }
    // The body, furred, and the head with its ears.
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(Rect::new(cx - 3, cy - 3, 7, 9), Ix::INK, 1);
    let hy = if matches!(facing, Facing::Up | Facing::UpRight) { cy - 5 } else { cy - 6 };
    let hx = cx + turn_x;
    m.ellipse(Rect::new(hx - 3, hy, 7, 5), Ix::INK, 1);
    for side in [-1, 1] {
        // Turned, the ear on the far side of the turn is seen edge on: narrower.
        let e = if turned && side == away_side { 2 } else { 3 };
        m.polyline_fill(&[(hx + side, hy + 1), (hx + side * e, hy - 3), (hx + side * e, hy + 2)], Ix::INK, 1);
    }
    c.inflate(&m, k.body, 2, relief::BODY);
    c.strokes(Rect::new(cx - 3, cy - 3, 7, 9), k.body, StrokeKind::Fur, 10, h32(k.seed, 3, salt::STROKES));
    if matches!(facing, Facing::Down | Facing::Side | Facing::DownRight) {
        c.set_emitting(k.eye_emits);
        // Turned, both eyes come round, the far one closing up on the near.
        let eyes: [i32; 2] = if turned { [-1, 2] } else { [-2, 1] };
        for dx in eyes {
            c.dot(hx + dx, hy + 2, if k.eye_emits { k.iris() } else { Ix::INK }, relief::BODY.hi + 1);
        }
        c.set_emitting(false);
        c.dot(hx + turn_x, hy + 4, Ramp::ClothLinen.at(Tone::Light), relief::BODY.hi + 1);
    } else if turned {
        // From behind and to the right: the snout's end past the cheek.
        c.dot(hx + 3, hy + 3, k.body.at(Tone::Light), relief::BODY.hi + 1);
    }
}

/// Fallen: on the ground, the wings spread flat.
fn dead(c: &mut Canvas, k: &Coat) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let s = 1;
    let (cx, cy) = (ax, ay - 3 * s);
    for side in [-1, 1] {
        let r = Rect::new(if side < 0 { cx - 8 * s } else { cx + 1 }, cy - 3 * s, 7 * s, 5 * s);
        let mut m = Canvas::new(c.w(), c.h());
        m.ellipse(r, Ix::INK, 1);
        c.inflate(&m, k.belly, 1, Z::new(1, 2));
        c.hline(r.x + 1, r.right() - 2, r.bottom() - 1, k.body.at(Tone::Shade), 2);
    }
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(Rect::new(cx - s, cy - 3 * s, 2 * s + 1, 6 * s), Ix::INK, 1);
    c.inflate(&m, k.body, 1, Z::new(2, 3));
}
