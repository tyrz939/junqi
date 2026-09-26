//! `bird` (hen, crow): ART.md §2.2. An egg of a body in its plumage, a folded wing laid over it
//! in the wing's own clusters, a head on a short neck with its beak (and a hen's comb and wattle
//! in red), stick legs with the toes spread forward. The walk is six frames: a step each foot,
//! the head bobbing forward on the step as a hen's does, the tail flicking a beat behind. The
//! idle pair pecks (a hen) or cocks its head (a crow); dead is on its back, the feet up.

use jane_core::grid::Rect;
use jane_data::Anatomy;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 2);
    pub const BODY: Z = Z::new(2, 5);
    pub const WING: Z = Z::new(6, 7);
    pub const HEAD: Z = Z::new(7, 9);
}

/// The beak and the legs: a hen's are yellow, a crow's a dark horn grey.
fn horn(k: &Coat) -> Ramp {
    if k.look.anatomy == Anatomy::Hen { Ramp::ClothMustard } else { Ramp::HairGrey }
}

/// One frame's pose: the feet (reach, lift), the head's bob, the tail's flick, the head down.
#[derive(Clone, Copy, Debug, Default)]
struct Pose {
    reach: [i32; 2],
    lift: [i32; 2],
    bob: i32,
    head: (i32, i32),
    flick: i32,
    shut: bool,
    open: bool,
}

/// `(near reach, near lift, far reach, far lift, bob, head forward)`.
const STEP: [Step; 6] = [
    step((0, 0), (0, 0), 0, 0),
    step((2, 1), (-1, 0), 0, 1),
    step((1, 0), (-1, 0), 1, 1),
    step((0, 0), (0, 1), 0, 0),
    step((-1, 0), (2, 1), 0, 1),
    step((-1, 0), (1, 0), 1, 1),
];

/// One beat of the walk: the near and far foot's `(reach, lift)`, the bob, the head forward.
#[derive(Clone, Copy, Debug)]
struct Step {
    near: (i32, i32),
    far: (i32, i32),
    bob: i32,
    fwd: i32,
}

const fn step(near: (i32, i32), far: (i32, i32), bob: i32, fwd: i32) -> Step {
    Step { near, far, bob, fwd }
}

fn pose(beat: Beat) -> Pose {
    match beat {
        Beat::Walk(k) => {
            let Step { near: (r0, l0), far: (r1, l1), bob, fwd } = STEP[usize::from(k % 6)];
            Pose { reach: [r0, r1], lift: [l0, l1], bob, head: (fwd, 0), flick: i32::from(k % 3 == 2), ..Pose::default() }
        }
        Beat::Breathe => Pose { flick: 1, head: (0, 1), ..Pose::default() },
        Beat::Hurt => Pose { head: (-1, 1), shut: true, flick: -1, ..Pose::default() },
        Beat::Attack(0) => Pose { head: (-2, -1), flick: 1, ..Pose::default() },
        Beat::Attack(1) => Pose { head: (3, 3), open: true, bob: 1, ..Pose::default() },
        Beat::Attack(_) => Pose { head: (1, 1), ..Pose::default() },
        Beat::Idle(_) | Beat::Dead => Pose::default(),
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let p = pose(beat);
    match (facing, beat) {
        (_, Beat::Dead) => dead(c, k),
        (_, Beat::Idle(i)) => idle(c, k, i),
        (Facing::Side, _) => side(c, k, &p),
        (Facing::Down, _) => front(c, k, &p, true, 0),
        (Facing::Up, _) => front(c, k, &p, false, 0),
    }
}

fn mask(c: &Canvas) -> Canvas {
    Canvas::new(c.w(), c.h())
}

/// A stick leg from `(x, top)` to the ground, `reach` forward and lifted `lift`, the toes
/// spread forward (east).
#[allow(clippy::too_many_arguments)]
fn leg(c: &mut Canvas, ramp: Ramp, x: i32, top: i32, reach: i32, lift: i32, ay: i32, z: Z) {
    // Two px wide, so the outline leaves it its colour: a lit side and a dark one.
    let foot = ay - lift;
    c.line((x, top), (x + reach, foot), ramp.at(Tone::Light), 1, z.hi);
    c.line((x + 1, top), (x + 1 + reach, foot), ramp.at(Tone::Base), 1, z.hi);
    c.hline(x + reach - 1, x + reach + 2, foot, ramp.at(Tone::Base), z.hi);
}

fn side(c: &mut Canvas, k: &Coat, p: &Pose) {
    let (_, _, _, ay) = super::size(k.look.plan);
    let hen = k.look.anatomy == Anatomy::Hen;
    let b = p.bob;
    // The far leg, in the body's shade.
    leg(c, horn(k), 8, ay - 4 + b, p.reach[1], p.lift[1], ay, relief::FAR);
    c.shade(Rect::new(5, ay - 5, 7, 6), horn(k), 1);
    // The tail: a hen's fan up at the back, a crow's wedge straight out.
    let mut m = mask(c);
    if hen {
        m.polyline_fill(&[(6, 8 + b), (2, 3 + b - p.flick), (4, 2 + b - p.flick), (6, 3 + b), (8, 7 + b)], Ix::INK, 1);
    } else {
        m.polyline_fill(&[(6, 8 + b), (1, 10 + b + p.flick), (1, 12 + b + p.flick), (7, 11 + b)], Ix::INK, 1);
    }
    // The body: an egg, fuller at the breast.
    let body = if hen { Rect::new(4, 5 + b, 10, 8) } else { Rect::new(4, 6 + b, 10, 7) };
    m.ellipse(body, Ix::INK, 1);
    m.ellipse(Rect::new(8, body.y - 1, 6, 7), Ix::INK, 1);
    // The neck up to the head.
    let (hx, hy) = (11 + p.head.0, 1 + b + p.head.1 + i32::from(!hen));
    m.polyline_fill(&[(10, body.y + 2), (hx, hy + 3), (hx + 3, hy + 4), (13, body.y + 4)], Ix::INK, 1);
    c.inflate(&m, k.body, 3, relief::BODY);
    // The breast in its colour.
    if k.belly != k.body {
        c.dye_ellipse(Rect::new(10, body.y + 1, 5, body.h), k.body, k.belly);
    }
    // The folded wing: an ellipse in the wing's clusters, its primaries a darker edge at the back.
    let mut w = mask(c);
    w.ellipse(Rect::new(5, body.y + 2, 7, 5), Ix::INK, 1);
    w.polyline_fill(&[(5, body.y + 4), (3, body.y + 5), (6, body.y + 6)], Ix::INK, 1);
    c.inflate(&w, k.mark, 2, relief::WING);
    c.strokes(Rect::new(4, body.y + 3, 8, 4), k.mark, StrokeKind::Feather, 10, h32(k.seed, 2, salt::STROKES));
    c.hline(4, 7, body.y + 6, k.mark.at(Tone::Shade), relief::WING.hi);
    // The near leg.
    leg(c, horn(k), 10, body.bottom() - 1, p.reach[0], p.lift[0], ay, relief::WING);
    head_side(c, k, hx, hy, p);
}

fn head_side(c: &mut Canvas, k: &Coat, hx: i32, hy: i32, p: &Pose) {
    let hen = k.look.anatomy == Anatomy::Hen;
    let mut m = mask(c);
    m.ellipse(Rect::new(hx, hy, 5, 5), Ix::INK, 1);
    c.inflate(&m, k.body, 2, relief::HEAD);
    // Painted on the face at its rim height, so no detail stands proud enough to take a seam.
    let z = relief::HEAD.lo;
    // The beak, lit on top; open to peck.
    let beak = horn(k);
    let long = i32::from(!hen);
    c.fill_rect(Rect::new(hx + 5, hy + 2, 2 + long, 1), beak.at(Tone::Light), z);
    c.fill_rect(Rect::new(hx + 5, hy + 3 + i32::from(p.open), 1 + long, 1), beak.at(Tone::Shade), z);
    if hen {
        // The comb in three lobes, the wattle under the beak.
        let red = Ramp::ClothRed;
        let mut cm = mask(c);
        cm.fill_rect(Rect::new(hx + 1, hy - 1, 4, 2), Ix::INK, 1);
        cm.fill_rect(Rect::new(hx + 1, hy - 2, 2, 1), Ix::INK, 1);
        cm.fill_rect(Rect::new(hx + 3, hy - 3, 2, 2), Ix::INK, 1);
        c.inflate(&cm, red, 1, Z::flat(z + 1));
        c.fill_rect(Rect::new(hx + 4, hy + 4, 2, 2), red.at(Tone::Base), z + 1);
        c.dot(hx + 5, hy + 5, red.at(Tone::Shade), z + 1);
    }
    // The eye: a bright ring for the hen, a dark bead with a glint for the crow.
    let (ex, ey) = (hx + 3, hy + 1);
    if p.shut {
        c.dot(ex, ey + 1, k.body.at(Tone::Deep), z);
    } else if hen {
        c.dot(ex, ey, Ramp::ClothMustard.at(Tone::High), z);
        c.dot(ex, ey + 1, Ix::INK, z);
    } else {
        c.dot(ex, ey + 1, Ix::INK, z);
        c.dot(ex, ey, Ramp::HairGrey.at(Tone::High), z);
    }
}

/// Facing the viewer or away; `peck` drops the head (the idle).
fn front(c: &mut Canvas, k: &Coat, p: &Pose, facing_us: bool, peck: i32) {
    let (_, _, ax, ay) = super::size(k.look.plan);
    let hen = k.look.anatomy == Anatomy::Hen;
    let b = p.bob;
    // Legs, alternating.
    for (i, x) in [(0, ax - 3), (1, ax + 1)] {
        leg(c, horn(k), x, ay - 4 + b, 0, p.lift[i], ay, relief::FAR);
    }
    let mut m = mask(c);
    let body = Rect::new(ax - 5, 6 + b, 10, 9);
    m.ellipse(body, Ix::INK, 1);
    if !facing_us {
        // The tail toward the viewer: a hen's fan over the back, a crow's wedge down.
        if hen {
            m.polyline_fill(&[(ax - 3, 8 + b), (ax - 1, 3 + b - p.flick), (ax + 2, 3 + b - p.flick), (ax + 3, 8 + b)], Ix::INK, 1);
        } else {
            m.polyline_fill(&[(ax - 1, 12 + b), (ax, 12 + b), (ax + 1, 15 + b), (ax - 2, 15 + b)], Ix::INK, 1);
        }
    }
    c.inflate(&m, k.body, 3, relief::BODY);
    if facing_us && k.belly != k.body {
        c.dye_ellipse(Rect::new(ax - 3, body.y + 3, 6, 6), k.body, k.belly);
    }
    // The wings folded at the sides.
    for x in [ax - 5, ax + 3] {
        let mut w = mask(c);
        w.ellipse(Rect::new(x, body.y + 2, 3, 6), Ix::INK, 1);
        c.inflate(&w, k.mark, 1, relief::WING);
    }
    // The head.
    let (hx, hy) = (ax - 3 + p.head.0.signum(), 1 + b + p.head.1 + peck);
    let mut h = mask(c);
    h.ellipse(Rect::new(hx, hy, 6, 6), Ix::INK, 1);
    c.inflate(&h, k.body, 2, relief::HEAD);
    // Painted on the face at its rim height, so no detail stands proud enough to take a seam.
    let z = relief::HEAD.lo;
    if hen {
        let red = Ramp::ClothRed;
        c.fill_rect(Rect::new(hx + 2, hy - 2, 2, 3), red.at(Tone::Base), z + 1);
        c.dot(hx + 2, hy - 2, red.at(Tone::Light), z + 1);
    }
    if facing_us {
        let beak = horn(k);
        c.fill_rect(Rect::new(hx + 2, hy + 3, 2, 2), beak.at(Tone::Light), z + 1);
        c.dot(hx + 3, hy + 4, beak.at(Tone::Shade), z + 1);
        if hen {
            c.dot(hx + 2, hy + 5, Ramp::ClothRed.at(Tone::Base), z + 1);
        }
        for x in [hx + 1, hx + 4] {
            if p.shut {
                c.dot(x, hy + 2, k.body.at(Tone::Deep), z);
            } else {
                c.dot(x, hy + 2, Ix::INK, z);
            }
        }
        if !p.shut {
            c.dot(hx + 1, hy + 1, Ramp::HairGrey.at(Tone::High), z);
        }
    }
}

fn idle(c: &mut Canvas, k: &Coat, beat: u8) {
    if k.look.anatomy == Anatomy::Hen {
        // Pecking: the head down to the ground and back up.
        let p = Pose::default();
        front(c, k, &p, true, if beat == 0 { 5 } else { 0 });
    } else {
        // A crow cocks its head: a px over and a px down.
        let p = Pose { head: (if beat == 0 { -1 } else { 1 }, i32::from(beat == 1)), ..Pose::default() };
        front(c, k, &p, true, 0);
    }
}

/// On its back, the feet up.
fn dead(c: &mut Canvas, k: &Coat) {
    let (_, _, ax, ay) = super::size(k.look.plan);
    for x in [ax - 2, ax + 2] {
        c.line((x, ay - 5), (x + 1, ay - 9), horn(k).at(Tone::Base), 1, 2);
        c.hline(x, x + 2, ay - 9, horn(k).at(Tone::Shade), 2);
    }
    let mut m = mask(c);
    m.ellipse(Rect::new(ax - 6, ay - 7, 11, 7), Ix::INK, 1);
    m.ellipse(Rect::new(ax + 3, ay - 5, 5, 5), Ix::INK, 1);
    c.inflate(&m, k.body, 2, relief::BODY);
    let mut w = mask(c);
    w.polyline_fill(&[(ax - 5, ay - 4), (ax - 9, ay - 1), (ax - 3, ay - 1)], Ix::INK, 1);
    c.inflate(&w, k.mark, 1, relief::WING);
    c.fill_rect(Rect::new(ax + 8, ay - 3, 2, 1), horn(k).at(Tone::Base), relief::HEAD.lo);
    c.dot(ax + 6, ay - 4, k.body.at(Tone::Deep), relief::HEAD.lo);
    if k.look.anatomy == Anatomy::Hen {
        c.fill_rect(Rect::new(ax + 4, ay - 1, 3, 1), Ramp::ClothRed.at(Tone::Shade), relief::HEAD.lo);
    }
}
