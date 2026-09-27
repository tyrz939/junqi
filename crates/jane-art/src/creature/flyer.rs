//! `flyer_insect` (butterfly, moth, the Emperor) and `flyer_bat` (bat): ART.md §2.2. Both
//! hover four rows over their anchor, where their shadow lies. The walk names carry the wing
//! beat: open, half, closed, half, open, half, the body rising a px on the down-stroke.
//!
//! An insect is a dark body (head, thorax, a tapering abdomen, two feelers) between two pairs of
//! wings, each a soft volume in the wing's colour with a dark margin, veins in its shade and an
//! eye spot; a moth's are dusty and broad, the Emperor's pale and huge with four eyes on them.
//! Seen from the side the wings stand up over the back. A bat is a furred body with its ears,
//! and wings of stretched leather on three fingers each, beating from a V to spread to down.

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
    let bat = k.look.anatomy == Anatomy::Bat;
    match (bat, beat) {
        (_, Beat::Dead) => dead(c, k, bat),
        (true, _) => bat_frame(c, k, facing, beat),
        (false, _) => insect(c, k, facing, beat),
    }
}

/// The insect's scale: the Emperor is drawn at twice the size.
fn scale(k: &Coat) -> i32 {
    if k.look.anatomy == Anatomy::Emperor { 2 } else { 1 }
}

/// One wing: a soft volume in the wing colour, a dark margin, veins, an eye spot.
#[allow(clippy::too_many_arguments)]
fn wing(c: &mut Canvas, k: &Coat, r: Rect, root: (i32, i32), spot: bool, far: bool, z: Z) {
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(r, Ix::INK, 1);
    m.polyline_fill(&[root, (r.x + r.w / 2, r.y + r.h / 2 - 1), (r.x + r.w / 2, r.y + r.h / 2 + 1)], Ix::INK, 1);
    let wing = k.belly;
    c.inflate(&m, wing, 2, z);
    // The margin: the wing's rim in the body's dark.
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            // Two px in from the rim, so the outline leaves a margin of the wing's dark.
            let inside = |x: i32, y: i32| m.get(x, y).is_opaque();
            let rim = |d: i32| !inside(x - d, y) || !inside(x + d, y) || !inside(x, y - d) || !inside(x, y + d);
            if inside(x, y) && (rim(1) || rim(2)) {
                let t = if y < r.y + r.h / 2 { Tone::Base } else { Tone::Shade };
                c.put(x, y, k.body.at(t), crate::canvas::FLAT, z.lo + 1);
            }
        }
    }
    // Veins from the root.
    let cx = r.x + r.w / 2;
    for dy in [-r.h / 4, r.h / 4] {
        c.line(root, (cx + (cx - root.0).signum() * r.w / 4, r.y + r.h / 2 + dy), wing.at(Tone::Shade), 1, z.lo + 1);
    }
    if spot && r.w >= 5 {
        let (sx, sy) = (cx + (cx - root.0).signum() * (r.w / 5), r.y + r.h / 2 - 1);
        c.fill_rect(Rect::new(sx - 1, sy, 2, 2), k.mark.at(Tone::Base), z.lo + 1);
        c.dot(sx, sy, k.mark.at(Tone::High), z.lo + 1);
    }
    if far {
        c.shade(r, wing, 1);
    }
}

/// The body: a head with its feelers, a thorax, an abdomen tapering behind.
#[allow(clippy::too_many_arguments)]
fn body(c: &mut Canvas, k: &Coat, cx: i32, cy: i32, s: i32, head_down: bool, side: bool, z: Z) {
    let mut m = Canvas::new(c.w(), c.h());
    let dir = if head_down { 1 } else { -1 };
    let bw = 2 * s + 1;
    // The head's front end, where the feelers leave it.
    let (hx, hy) = if side {
        // Abdomen trailing west, thorax, head east.
        m.ellipse(Rect::new(cx - 7 * s, cy - s, 7 * s, 2 * s + 1), Ix::INK, 1);
        m.ellipse(Rect::new(cx - s, cy - 2 * s, 3 * s + 1, 4 * s), Ix::INK, 1);
        m.ellipse(Rect::new(cx + 2 * s, cy - 2 * s, 2 * s + 1, 2 * s + 1), Ix::INK, 1);
        (cx + 4 * s, cy - 2 * s)
    } else {
        m.ellipse(Rect::new(cx - s, cy - 2 * s, bw, 4 * s), Ix::INK, 1);
        let (head, abdomen) = if head_down {
            (Rect::new(cx - s, cy + 2 * s - 1, bw, 2 * s + 1), Rect::new(cx - s, cy - 8 * s + 1, bw, 6 * s))
        } else {
            (Rect::new(cx - s, cy - 4 * s + 1, bw, 2 * s + 1), Rect::new(cx - s, cy + 2 * s, bw, 6 * s))
        };
        m.ellipse(head, Ix::INK, 1);
        m.ellipse(abdomen, Ix::INK, 1);
        (cx, if head_down { head.bottom() } else { head.y - 1 })
    };
    c.inflate(&m, k.body, s + 1, z);
    c.strokes(Rect::new(cx - 7 * s, cy - 8 * s, 14 * s, 16 * s), k.body, StrokeKind::Fur, 6, h32(k.seed, 5, salt::STROKES));
    // The feelers.
    for dx in [-1, 1] {
        let (tx, ty) = if side { (hx + 2 * s + dx.min(0) * s, hy - 3 * s) } else { (hx + dx * 2 * s, hy + dir * 3 * s) };
        c.line((hx + if side { 0 } else { dx }, hy), (tx, ty), k.body.at(Tone::Shade), 1, z.hi);
        c.dot(tx, ty, k.body.at(Tone::Light), z.hi);
    }
    if k.eye_emits {
        c.set_emitting(true);
        if side {
            c.dot(hx - 1, hy + s, k.iris(), z.hi);
        } else {
            for dx in [-s, s] {
                c.dot(cx + dx, hy - dir * s, k.iris(), z.hi);
            }
        }
        c.set_emitting(false);
    }
}

fn insect(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let s = scale(k);
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let (open, rise) = beat_wings(beat);
    let hover = 4 * s;
    let (cx, cy) = (ax + drift(beat) * s, ay - hover - 5 * s - rise * s);
    let moth = matches!(k.look.anatomy, Anatomy::Moth | Anatomy::Emperor);
    let (fw, fh) = if moth { (8 * s, 7 * s) } else { (7 * s, 7 * s) };
    match facing {
        Facing::Side => {
            // The wings stand over the back: the far pair behind, the near pair in front.
            let hgt = [3, 6, 8][open as usize] * s;
            let lean = [3, 1, 0][open as usize] * s;
            // Wings raised over the back stay inside the box: the body drops to make room.
            let cy = cy.max(hgt + s + 1);
            for (far, dx) in [(true, 2 * s), (false, 0)] {
                let z = if far { relief::FAR } else { relief::NEAR };
                let r = Rect::new(cx - 4 * s + dx - lean, cy - hgt - s, fw - s, hgt + s);
                wing(c, k, r, (cx + dx, cy - s), !far, far, z);
                let h = Rect::new(cx - 6 * s + dx - lean, cy - hgt / 2 - s, fw - 2 * s, hgt / 2 + 2 * s);
                wing(c, k, h, (cx - s + dx, cy), false, far, z);
            }
            body(c, k, cx, cy, s, true, true, relief::BODY);
        }
        Facing::Down | Facing::Up => {
            let head_down = facing == Facing::Down;
            let span = [2 * s, 5 * s, 7 * s][open as usize];
            let hh = [fh + 2 * s, fh, fh][open as usize];
            for side in [-1, 1] {
                let near = side < 0;
                let fx = if side < 0 { cx - span - fw / 2 } else { cx + span - fw / 2 + 1 };
                let fore = Rect::new(fx, cy - hh + s, fw, hh);
                let hind = Rect::new(fx + side * -s, cy + s, fw - 2 * s, hh - 2 * s);
                let z = if near { relief::NEAR } else { relief::FAR };
                let (a, b) = if head_down { (hind, fore) } else { (fore, hind) };
                // The fore pair is the larger; the head is toward the viewer on `Down`.
                let (a, b) = if head_down { (Rect::new(a.x, a.y - s, a.w, a.h), Rect::new(b.x, b.y + 2 * s, b.w, b.h)) } else { (a, b) };
                wing(c, k, a, (cx, cy - s), head_down, false, z);
                wing(c, k, b, (cx, cy + s), !head_down, false, z);
            }
            body(c, k, cx, cy, s, head_down, false, relief::BODY);
        }
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
    for side in [-1, 1] {
        let far = side_view && side < 0;
        let z = if far { relief::FAR } else { relief::NEAR };
        let t = tips(side);
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
    let hy = if facing == Facing::Up { cy - 5 } else { cy - 6 };
    m.ellipse(Rect::new(cx - 3, hy, 7, 5), Ix::INK, 1);
    for side in [-1, 1] {
        m.polyline_fill(&[(cx + side, hy + 1), (cx + side * 3, hy - 3), (cx + side * 3, hy + 2)], Ix::INK, 1);
    }
    c.inflate(&m, k.body, 2, relief::BODY);
    c.strokes(Rect::new(cx - 3, cy - 3, 7, 9), k.body, StrokeKind::Fur, 10, h32(k.seed, 3, salt::STROKES));
    if facing != Facing::Up {
        c.set_emitting(k.eye_emits);
        for dx in [-2, 1] {
            c.dot(cx + dx, hy + 2, if k.eye_emits { k.iris() } else { Ix::INK }, relief::BODY.hi + 1);
        }
        c.set_emitting(false);
        c.dot(cx, hy + 4, Ramp::ClothLinen.at(Tone::Light), relief::BODY.hi + 1);
    }
}

/// Fallen: on the ground, the wings spread flat.
fn dead(c: &mut Canvas, k: &Coat, bat: bool) {
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let s = if bat { 1 } else { scale(k) };
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
