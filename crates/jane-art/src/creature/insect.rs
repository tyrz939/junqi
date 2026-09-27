//! `flyer_insect` (butterfly, moth, the Emperor): ART.md §2.2. Drawn in half-px units so one
//! set of shapes serves a butterfly at one and a half times and the Emperor at two and a half.
//!
//! Four wings, each its own shape: a forewing with a pointed apex, a rounder hindwing. The wing
//! is a soft volume in its colour (`belly`) with a margin of the body's dark, a band across it
//! and veins in its shade, and eyespots in rings: black, the `mark` colour, a pale centre with a
//! pupil. A butterfly's eyespots sit toward the apex; a moth's and the Emperor's in the middle
//! of every wing, the Emperor's ringed twice. The body is furred, the abdomen banded, the head
//! with its eyes; a butterfly's feelers end in clubs, a moth's are feathered.
//!
//! The wing beat changes the shape, not only the size: spread wide and flat, raised half way
//! (shorter, higher, the hindwing tucked), and clapped up over the back (tall and narrow). From
//! the side the wings are seen face on when raised and edge on when spread.

use jane_core::grid::Rect;
use jane_data::Anatomy;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z, bresenham};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 3);
    pub const BODY: Z = Z::new(4, 7);
    pub const NEAR: Z = Z::new(6, 8);
}

/// Half-px units: a butterfly or a moth 3, the Emperor 5.
fn unit(k: &Coat) -> i32 {
    if k.look.anatomy == Anatomy::Emperor { 5 } else { 3 }
}

/// A line of px that touch by their sides, never only by a corner, so the finish never takes
/// it for a run of spikes.
pub(crate) fn stair(c: &mut Canvas, a: (i32, i32), b: (i32, i32), ix: Ix, z: u8) {
    let mut last: Option<(i32, i32)> = None;
    bresenham(a.0, a.1, b.0, b.1, |x, y| {
        if let Some((px, py)) = last {
            if px != x && py != y {
                c.dot(x, py, ix, z);
            }
        }
        c.dot(x, y, ix, z);
        last = Some((x, y));
    });
}

/// A forewing's four points and a hindwing's five, in units.
type Fore = [(i32, i32); 4];
type Hind = [(i32, i32); 5];
/// A wing's outline in px.
type Outline = Vec<(i32, i32)>;

/// The wings' shapes by how open they are, in units about the body's middle, for the east side
/// (the west mirrors): the forewing's root, apex, outer corner and inner corner; the hindwing's
/// root, outer points and inner end.
fn shapes(open: i32) -> (Fore, Hind) {
    match open {
        2 => ([(1, -2), (10, -7), (9, -1), (2, 1)], [(1, 0), (8, 1), (7, 5), (3, 7), (1, 3)]),
        1 => ([(1, -2), (7, -10), (7, -4), (2, 0)], [(1, -1), (6, -2), (6, 3), (3, 4), (1, 2)]),
        _ => ([(1, -2), (3, -11), (4, -6), (2, -1)], [(1, -1), (3, -7), (4, -4), (3, -1), (1, 0)]),
    }
}

/// The lowest unit any shape reaches below the body's middle (the abdomen's tip seen from
/// behind), the same for every beat so the body rides level over its shadow.
const REACH_DOWN: i32 = 8;

/// How open the wings are and the body's rise, px, on the beat.
fn beat_wings(beat: Beat) -> (i32, i32) {
    match beat {
        Beat::Walk(k) => [(2, 0), (1, 1), (0, 2), (1, 1), (2, 0), (1, -1)][usize::from(k % 6)],
        Beat::Breathe => (2, 1),
        Beat::Idle(0) | Beat::Attack(0) => (0, 0),
        Beat::Idle(_) => (1, 0),
        Beat::Hurt => (1, 2),
        Beat::Attack(1) => (2, -2),
        Beat::Attack(_) => (1, 1),
        Beat::Dead => (2, 0),
    }
}

/// How far it drifts on the beat, px east: it never hangs still in the air.
fn drift(beat: Beat) -> i32 {
    match beat {
        Beat::Walk(k) => [0, 0, 1, 1, 0, -1][usize::from(k % 6)],
        _ => 0,
    }
}

/// One wing: its mask filled as a soft volume, then the margin, the band, the veins and the
/// eyespot laid over it inside the mask.
#[allow(clippy::too_many_arguments)]
fn wing(c: &mut Canvas, k: &Coat, pts: &[(i32, i32)], spot: Option<(i32, i32)>, spot_r: i32, u: i32, far: bool, z: Z) {
    let mut m = Canvas::new(c.w(), c.h());
    m.polyline_fill(pts, Ix::INK, 1);
    let w = k.belly;
    c.inflate(&m, w, (u / 2).max(1), z);
    let inside = |x: i32, y: i32| m.get(x, y).is_opaque();
    let root = pts[0];
    // A band across the wing a little past its middle, a tone into the mark's shade.
    let far_pt = pts[pts.len() / 2];
    let (bx, by) = (root.0 + (far_pt.0 - root.0) * 3 / 5, root.1 + (far_pt.1 - root.1) * 3 / 5);
    let band = |x: i32, y: i32| {
        let (dx, dy) = (x - root.0, y - root.1);
        let d2 = dx * dx + dy * dy;
        let r = (bx - root.0) * (bx - root.0) + (by - root.1) * (by - root.1);
        (d2 - r).abs() <= r / 6
    };
    let (x0, x1) = (pts.iter().map(|p| p.0).min().unwrap_or(0), pts.iter().map(|p| p.0).max().unwrap_or(0));
    let (y0, y1) = (pts.iter().map(|p| p.1).min().unwrap_or(0), pts.iter().map(|p| p.1).max().unwrap_or(0));
    let margin = if u >= 5 { 2 } else { 1 };
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !inside(x, y) {
                continue;
            }
            let edge =
                (1..=margin).any(|d| !inside(x - d, y) || !inside(x + d, y) || !inside(x, y - d) || !inside(x, y + d));
            if edge {
                let t = if y < (y0 + y1) / 2 { Tone::Base } else { Tone::Shade };
                c.put(x, y, k.body.at(t), crate::canvas::FLAT, z.lo + 1);
            } else if band(x, y) {
                c.put(x, y, k.mark.at(Tone::Shade), crate::canvas::FLAT, z.lo + 1);
            }
        }
    }
    // Veins from the root to the outer points.
    for &p in &pts[1..pts.len() - 1] {
        let tip = (root.0 + (p.0 - root.0) * 4 / 5, root.1 + (p.1 - root.1) * 4 / 5);
        stair(c, root, tip, w.at(Tone::Mid), z.lo + 1);
    }
    if let Some((sx, sy)) = spot {
        // An eyespot in rings: black, the mark's colour, a pale centre and its pupil.
        let r = spot_r;
        c.ellipse(Rect::new(sx - r, sy - r, 2 * r + 1, 2 * r + 1), k.body.at(Tone::Deep), z.hi);
        if r >= 2 {
            c.ellipse(Rect::new(sx - r + 1, sy - r + 1, 2 * r - 1, 2 * r - 1), k.mark.at(Tone::Base), z.hi);
        }
        if r >= 3 {
            c.ellipse(Rect::new(sx - 1, sy - 1, 3, 3), w.at(Tone::High), z.hi);
        }
        c.dot(sx, sy, k.body.at(Tone::Deep), z.hi);
        c.dot(sx - 1, sy - 1, w.at(Tone::Glint), z.hi);
    }
    if far {
        let (lo, hi) = (x0.min(x1), x0.max(x1));
        c.shade(Rect::new(lo, y0, hi - lo + 1, y1 - y0 + 1), w, 1);
    }
}

/// The body: a furred thorax, a banded abdomen, the head with its eyes, and the feelers.
fn body(c: &mut Canvas, k: &Coat, cx: i32, cy: i32, u: i32, head_down: bool, side: bool) {
    let sc = |v: i32| v * u / 2;
    let z = relief::BODY;
    let mut m = Canvas::new(c.w(), c.h());
    let moth = matches!(k.look.anatomy, Anatomy::Moth | Anatomy::Emperor);
    let fat = i32::from(moth);
    let dir = if head_down { 1 } else { -1 };
    let (head, abdomen) = if side {
        let th = Rect::new(cx - sc(1), cy - sc(2), sc(3), sc(3));
        m.ellipse(th, Ix::INK, 1);
        let ab = Rect::new(cx - sc(8), cy - sc(1), sc(7) + 1, sc(2) + 1 + fat);
        let hd = Rect::new(cx + sc(2), cy - sc(2), sc(2), sc(2));
        (hd, ab)
    } else {
        let th = Rect::new(cx - sc(1) - fat, cy - sc(2), sc(2) + 1 + 2 * fat, sc(4));
        m.ellipse(th, Ix::INK, 1);
        let hd = if head_down {
            Rect::new(cx - sc(1), th.bottom() - 1, sc(2) + 1, sc(2))
        } else {
            Rect::new(cx - sc(1), th.y - sc(2) + 1, sc(2) + 1, sc(2))
        };
        let ab = if head_down {
            Rect::new(cx - sc(1) + 1 - fat, th.y - sc(6) + 1, sc(2) - 1 + 2 * fat, sc(6))
        } else {
            Rect::new(cx - sc(1) + 1 - fat, th.bottom() - 1, sc(2) - 1 + 2 * fat, sc(6))
        };
        (hd, ab)
    };
    m.ellipse(head, Ix::INK, 1);
    m.ellipse(abdomen, Ix::INK, 1);
    c.inflate(&m, k.body, sc(1).max(1), z);
    c.strokes(
        Rect::new(cx - sc(8), cy - sc(8), sc(16), sc(16)),
        k.body,
        StrokeKind::Fur,
        8,
        h32(k.seed, 5, salt::STROKES),
    );
    // The abdomen's bands.
    if side {
        for x in (abdomen.x + 1..abdomen.right() - 1).step_by(2) {
            c.tint(x, abdomen.y + abdomen.h / 2, k.body, Tone::Deep);
        }
    } else {
        for y in (abdomen.y + 1..abdomen.bottom() - 1).step_by(2) {
            for x in abdomen.x..abdomen.right() {
                c.tint(x, y, k.body, Tone::Deep);
            }
        }
    }
    // The eyes.
    let eye = if k.eye_emits { Ramp::Ember.at(Tone::High) } else { k.belly.at(Tone::High) };
    c.set_emitting(k.eye_emits);
    if side {
        c.dot(head.right() - 1, head.y + 1, eye, z.hi + 1);
    } else if head_down {
        c.dot(head.x, head.y + head.h / 2, eye, z.hi + 1);
        c.dot(head.right() - 1, head.y + head.h / 2, eye, z.hi + 1);
    }
    c.set_emitting(false);
    // The feelers: a club on a butterfly's, barbs down a moth's.
    let (hx, hy) = if side {
        (head.right() - 1, head.y)
    } else if head_down {
        (cx, head.bottom() - 1)
    } else {
        (cx, head.y)
    };
    for s in [-1, 1] {
        let tip = if side {
            (hx + sc(3) + i32::from(s > 0) * sc(1), hy - sc(3) + i32::from(s > 0))
        } else {
            (hx + s * sc(3), hy + dir * sc(4))
        };
        let from = if side { (hx, hy) } else { (hx + s, hy) };
        let ink = k.body.at(Tone::Shade);
        stair(c, from, tip, ink, z.hi);
        if moth {
            // Feathered: short barbs off the shaft, back toward the body.
            let n = 3.max(u / 2);
            for i in 1..=n {
                let (px, py) = (from.0 + (tip.0 - from.0) * i / (n + 1), from.1 + (tip.1 - from.1) * i / (n + 1));
                c.dot(px - s, py, k.body.at(Tone::Base), z.hi);
                c.dot(px, py - dir, k.body.at(Tone::Base), z.hi);
            }
        } else {
            c.fill_rect(Rect::new(tip.0 - 1, tip.1 - 1, 2, 2), k.body.at(Tone::Base), z.hi);
            c.dot(tip.0 - 1, tip.1 - 1, k.mark.at(Tone::Light), z.hi);
        }
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let u = unit(k);
    let sc = |v: i32| v * u / 2;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    if beat == Beat::Dead {
        dead(c, k, ax, ay, u);
        return;
    }
    let (open, rise) = beat_wings(beat);
    let hover = 4;
    let cx = ax + drift(beat) * u / 2;
    let (fore, hind) = shapes(open);
    let emperor = k.look.anatomy == Anatomy::Emperor;
    let moth = matches!(k.look.anatomy, Anatomy::Moth | Anatomy::Emperor);
    let spot_r = if emperor { 3 } else { 1 };
    match facing {
        Facing::Down | Facing::Up => {
            // The lowest of it hovers over the anchor; the box holds the tallest beat.
            let cy = ay - hover - sc(REACH_DOWN) - rise;
            let head_down = facing == Facing::Down;
            for side in [-1, 1] {
                let p = |q: (i32, i32)| (cx + side * sc(q.0), cy + sc(q.1));
                let f: Vec<(i32, i32)> = fore.iter().map(|&q| p(q)).collect();
                let h: Vec<(i32, i32)> = hind.iter().map(|&q| p(q)).collect();
                // The fore and hind wings' eyespots: a butterfly's toward the apex, a moth's in the middle.
                let fspot = if moth {
                    p(((fore[1].0 + fore[2].0) / 2 - 3, (fore[1].1 + fore[2].1) / 2 + 1))
                } else {
                    p((fore[1].0 - 3, fore[1].1 + 2))
                };
                let hspot = p(((hind[1].0 + hind[2].0 + hind[3].0) / 3 - 1, (hind[1].1 + hind[2].1 + hind[3].1) / 3));
                wing(c, k, &h, (moth && open > 0).then_some(hspot), spot_r, u, false, relief::FAR);
                wing(c, k, &f, (open > 0).then_some(fspot), spot_r, u, false, relief::NEAR);
            }
            body(c, k, cx, cy, u, head_down, false);
        }
        Facing::Side => {
            // Seen from the side: raised wings face on over the back, spread ones edge on.
            let cy = ay - hover - sc(3) - rise;
            for (far, dx) in [(true, sc(1)), (false, 0)] {
                let z = if far { relief::FAR } else { relief::NEAR };
                // Raised, the pair faces the viewer: a forewing up and forward, a rounder hindwing
                // behind and below it. Spread, both are edge on: a thin blade either way.
                let (f, h): (Outline, Outline) = match open {
                    0 => (
                        vec![
                            (cx + dx, cy - sc(1)),
                            (cx - sc(2) + dx, cy - sc(11)),
                            (cx + sc(5) + dx, cy - sc(9)),
                            (cx + sc(2) + dx, cy - sc(1)),
                        ],
                        vec![
                            (cx + dx, cy),
                            (cx - sc(6) + dx, cy - sc(6)),
                            (cx - sc(5) + dx, cy - sc(2)),
                            (cx - sc(1) + dx, cy + sc(1)),
                        ],
                    ),
                    1 => (
                        vec![
                            (cx + dx, cy - sc(1)),
                            (cx - sc(6) + dx, cy - sc(8)),
                            (cx + sc(3) + dx, cy - sc(7)),
                            (cx + sc(2) + dx, cy - sc(1)),
                        ],
                        vec![
                            (cx + dx, cy),
                            (cx - sc(7) + dx, cy - sc(3)),
                            (cx - sc(6) + dx, cy),
                            (cx - sc(1) + dx, cy + sc(1)),
                        ],
                    ),
                    _ => (
                        vec![
                            (cx + dx, cy - sc(2)),
                            (cx - sc(8) + dx, cy - sc(4)),
                            (cx + sc(3) + dx, cy - sc(4)),
                            (cx + sc(2) + dx, cy - sc(1)),
                        ],
                        vec![
                            (cx + dx, cy - sc(1)),
                            (cx - sc(7) + dx, cy - sc(2)),
                            (cx - sc(6) + dx, cy),
                            (cx + dx, cy),
                        ],
                    ),
                };
                let spot = (open < 2 && !far).then(|| (cx + dx, (f[1].1 + f[3].1) / 2));
                let hspot = (open < 2 && !far && moth).then(|| ((h[1].0 + h[2].0) / 2 + 1, (h[1].1 + h[2].1) / 2));
                wing(c, k, &h, hspot, spot_r, u, far, z);
                wing(c, k, &f, spot, spot_r, u, far, z);
            }
            body(c, k, cx, cy, u, true, true);
        }
    }
}

/// Fallen: on its back on the ground, the wings spread flat and ragged, the legs up.
fn dead(c: &mut Canvas, k: &Coat, ax: i32, ay: i32, u: i32) {
    let sc = |v: i32| v * u / 2;
    let cy = ay - sc(3);
    for side in [-1, 1] {
        let p = |q: (i32, i32)| (ax + side * sc(q.0), cy + sc(q.1) / 2);
        let f: Vec<(i32, i32)> = [(1, -2), (12, -6), (11, 0), (2, 1)].iter().map(|&q| p(q)).collect();
        let h: Vec<(i32, i32)> = [(1, 0), (9, 1), (8, 5), (1, 3)].iter().map(|&q| p(q)).collect();
        wing(c, k, &h, None, 1, u, false, Z::new(1, 2));
        wing(c, k, &f, None, 1, u, false, Z::new(1, 2));
        // Torn: a notch out of the forewing's edge.
        c.clear_px(ax + side * sc(10), cy - sc(2));
        c.clear_px(ax + side * sc(10) + side, cy - sc(2));
    }
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(Rect::new(ax - sc(1), cy - sc(3), sc(2) + 1, sc(6)), Ix::INK, 1);
    c.inflate(&m, k.body, 1, Z::new(2, 3));
    for s in [-1, 1] {
        stair(c, (ax + s, cy), (ax + s * sc(2), cy - sc(3)), k.body.at(Tone::Shade), 3);
    }
}
