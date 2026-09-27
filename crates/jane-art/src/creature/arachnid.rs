//! `arachnid` (spider, the queen, the lurker): ART.md §2.2. A cephalothorax and a bigger
//! abdomen behind it, each a soft volume, the abdomen marked down its back (a spider's chevrons,
//! the queen's pale skull), eight legs of two segments whose knees stand above the body, a
//! cluster of eyes that shine, and fangs under them. It walks on alternate legs: four lift and
//! reach while four hold. The queen is drawn half as large again, bristling, with eight eyes in
//! two rows; the lurker is a small pale body slung low between long thin legs.

use jane_core::grid::Rect;
use jane_data::Anatomy;

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 3);
    pub const BODY: Z = Z::new(3, 7);
    pub const LEG: Z = Z::new(6, 8);
}

/// Half-px units: a spider is 2, the queen 3.
fn unit(k: &Coat) -> i32 {
    if k.look.anatomy == Anatomy::Queen { 3 } else { 2 }
}

/// How far a leg reaches, in half-px units.
fn reach(k: &Coat) -> i32 {
    // Its body slung higher is what makes the lurker's legs long; its knees stand as high a
    // proportion over it as a spider's.
    let _ = k;
    2
}

/// How high a leg of group 0 (near front, far second, ...) or group 1 lifts on a beat, in
/// half-px units: 2 as it lifts, 1 as it reaches down, 0 holding.
fn lifts(beat: Beat, group: usize) -> i32 {
    match beat {
        Beat::Walk(k) => {
            let k = usize::from(k % 6);
            // Lifted high on the first beat, reaching down to set on the second.
            let (a, b) = if group == 0 { (1, 2) } else { (4, 5) };
            if k == a { 2 } else { i32::from(k == b) }
        }
        Beat::Idle(1) => i32::from(group == 0),
        _ => 0,
    }
}

/// The eyes' light: a hot ember.
fn eye_ix(k: &Coat) -> Ix {
    if k.eye_emits { Ramp::Ember.at(Tone::High) } else { Ix::INK }
}

/// A leg as laid out: its side, its index front to back, its hip, knee and foot.
type Leg = (i32, i32, (i32, i32), (i32, i32), (i32, i32));

fn leg(c: &mut Canvas, k: &Coat, hip: (i32, i32), knee: (i32, i32), foot: (i32, i32), w: i32, z: Z) {
    // A spider's legs are its belly's brown, so they read against its black body.
    let r = if k.look.anatomy == Anatomy::Spider { k.belly } else { k.body };
    c.line(hip, knee, r.at(Tone::Base), w, z.lo);
    c.line((hip.0, hip.1 - 1), (knee.0, knee.1 - 1), r.at(Tone::Light), 1, z.hi);
    // Two px: a line of one is a run of spikes the finish would take off.
    c.line(knee, foot, r.at(Tone::Shade), 2, z.lo);
    c.dot(knee.0, knee.1, r.at(Tone::Lift), z.hi);
    if k.belly != k.body {
        // Banded legs: a ring of the belly's colour below each knee.
        let (mx, my) = ((knee.0 * 2 + foot.0) / 3, (knee.1 * 2 + foot.1) / 3);
        c.dot(mx, my, k.belly.at(Tone::Base), z.hi);
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    match (facing, beat) {
        (_, Beat::Dead) => dead(c, k),
        (Facing::Side, _) => side(c, k, beat),
        (Facing::Down, _) | (_, Beat::Idle(_)) => top(c, k, beat, true),
        (Facing::Up, _) => top(c, k, beat, false),
    }
}

/// The abdomen's marking along its back.
fn mark(c: &mut Canvas, k: &Coat, ab: Rect, toward_us: bool) {
    let mid = ab.x + ab.w / 2;
    if k.look.anatomy == Anatomy::Queen {
        // A pale skull on her back, two dark sockets in it.
        let (sx, sy) = (mid - 3, ab.y + ab.h / 2 - 3);
        c.dye_ellipse(Rect::new(sx, sy, 7, 6), k.body, k.mark);
        c.dye_ellipse(Rect::new(sx + 1, sy + 4, 5, 3), k.body, k.mark);
        for dx in [1, 4] {
            c.put(sx + dx, sy + 2, k.body.at(Tone::Deep), crate::canvas::FLAT, relief::BODY.hi);
            c.put(sx + dx + 1, sy + 2, k.body.at(Tone::Deep), crate::canvas::FLAT, relief::BODY.hi);
        }
        c.strokes(ab, k.body, StrokeKind::Fur, 14, h32(k.seed, 9, salt::STROKES));
    } else {
        // Chevrons down the back, pointing to the head.
        let n = (ab.h / 3).max(2);
        for i in 0..n {
            let y = if toward_us { ab.y + 2 + i * 3 } else { ab.bottom() - 3 - i * 3 };
            let d = if toward_us { 1 } else { -1 };
            for dx in 0..3 - i.min(2) {
                c.tint(mid - 1 - dx, y - d * dx, k.body, Tone::Deep);
                c.tint(mid + dx, y - d * dx, k.body, Tone::Deep);
            }
            c.dye_ellipse(Rect::new(mid - 1, y - 1, 2, 2), k.body, k.mark);
        }
    }
}

/// From the front or behind, 3/4 top-down: the abdomen behind the head up the screen (or
/// toward the viewer from behind), the legs in a ring round it.
fn top(c: &mut Canvas, k: &Coat, beat: Beat, toward_us: bool) {
    let u = unit(k);
    let sc = |v: i32| v * u / 2;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let lurker = k.look.anatomy == Anatomy::Lurker;
    let (rise, lunge) = match beat {
        Beat::Attack(0) => (sc(2), -sc(2)),
        Beat::Attack(1) => (-1, sc(2)),
        Beat::Hurt => (-1, 0),
        Beat::Breathe => (1, 0),
        _ => (0, 0),
    };
    // Seen from behind the abdomen is nearest, so the head stands further up the screen.
    let lift_body = match (lurker, toward_us) {
        (true, _) => 11,
        (false, true) => 8,
        (false, false) => 14,
    };
    let ceph_y = ay - sc(lift_body) - rise + if toward_us { lunge } else { -lunge };
    let (ab_y, dir) = if toward_us { (ceph_y - sc(8), 1) } else { (ceph_y + sc(6), -1) };
    let ab = match k.look.anatomy {
        Anatomy::Lurker => Rect::new(ax - sc(4), ab_y - sc(2), sc(8), sc(7)),
        // A spider's abdomen is its own size, not the queen's swollen one, so its legs show.
        Anatomy::Spider => Rect::new(ax - sc(5), ab_y - sc(3), sc(10), sc(9)),
        _ => Rect::new(ax - sc(7), ab_y - sc(5), sc(14), sc(12)),
    };
    let ceph = if lurker { Rect::new(ax - sc(3), ceph_y - sc(3), sc(6), sc(5)) } else { Rect::new(ax - sc(5), ceph_y - sc(4), sc(10), sc(8)) };
    let r = reach(k);
    let lw = if u == 3 { 3 } else { 2 };
    // The legs behind the body first, then the body, then the front legs over it.
    let mut legs: Vec<Leg> = Vec::new();
    for side in [-1, 1] {
        for i in 0..4 {
            // The legs fan round the body: the front pair's knees low and forward, the back
            // pair's high behind, so every knee stands clear of its neighbour's.
            const KX: [i32; 4] = [7, 10, 10, 8];
            const KY: [i32; 4] = [-4, -8, -12, -16];
            const FY: [i32; 4] = [0, -5, -10, -15];
            let group = (i + usize::from(side > 0)) % 2;
            let lift = lifts(beat, group) * sc(1);
            // From behind the order runs the other way: the back pair nearest the viewer.
            let j = if toward_us { i } else { 3 - i };
            let hip = (ax + side * sc(2), ceph_y + sc(1 - i as i32) * dir);
            let knee = (ax + side * sc(KX[j]), ceph_y + sc(KY[j]) * r / 2 - lift);
            let raised = matches!(beat, Beat::Attack(0 | 1)) && i == 0;
            let foot = if raised {
                (ax + side * sc(5), ceph_y - sc(10))
            } else {
                (knee.0 + side * sc(3) + side * lift, ay - 1 + sc(FY[j]) - lift)
            };
            legs.push((side, i as i32, hip, knee, foot));
        }
    }
    let back = |i: i32| if toward_us { i >= 2 } else { i < 2 };
    for &(_, i, hip, knee, foot) in legs.iter().filter(|l| back(l.1)) {
        leg(c, k, hip, knee, foot, lw, relief::FAR);
        let _ = i;
    }
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(ab, Ix::INK, 1);
    c.inflate(&m, k.body, sc(3), relief::BODY);
    mark(c, k, ab, toward_us);
    let mut h = Canvas::new(c.w(), c.h());
    h.ellipse(ceph, Ix::INK, 1);
    c.inflate(&h, k.body, sc(2), Z::new(relief::BODY.lo + 1, relief::BODY.hi + 1));
    for &(_, _, hip, knee, foot) in legs.iter().filter(|l| !back(l.1)) {
        leg(c, k, hip, knee, foot, lw, relief::LEG);
    }
    if toward_us {
        // The eyes in a cluster at the front, the fangs under them.
        let ey = ceph.bottom() - sc(3);
        let z = relief::LEG.hi;
        c.set_emitting(k.eye_emits);
        let eyes: &[(i32, i32)] = match k.look.anatomy {
            Anatomy::Queen => &[(-3, 0), (-1, -1), (1, -1), (3, 0), (-2, 2), (0, 1), (2, 2), (0, -2)],
            Anatomy::Lurker => &[(-1, 0), (1, 0)],
            _ => &[(-2, 0), (-1, -1), (1, -1), (2, 0)],
        };
        for &(dx, dy) in eyes {
            c.dot(ax + dx - i32::from(dx > 0), ey + dy, eye_ix(k), z);
        }
        c.set_emitting(false);
        let fang = if k.look.anatomy == Anatomy::Queen { Ramp::Bone } else { k.body };
        let open = i32::from(beat == Beat::Attack(1));
        for dx in [-1 - open, open] {
            c.vline(ax + dx, ceph.bottom(), ceph.bottom() + sc(2) - 1, fang.at(Tone::Light), z);
        }
    }
}

fn side(c: &mut Canvas, k: &Coat, beat: Beat) {
    let u = unit(k);
    let sc = |v: i32| v * u / 2;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let lurker = k.look.anatomy == Anatomy::Lurker;
    let r = reach(k);
    let (rise, lunge) = match beat {
        Beat::Attack(0) => (sc(3), -sc(2)),
        Beat::Attack(1) => (0, sc(3)),
        Beat::Hurt => (-1, -1),
        Beat::Breathe => (1, 0),
        _ => (0, 0),
    };
    let cy = ay - sc(if lurker { 12 } else { 8 }) - rise;
    let ceph = Rect::new(ax + sc(1) + lunge, cy - sc(4), sc(8), sc(7));
    let ab = if lurker {
        Rect::new(ax - sc(6) + lunge, cy - sc(5), sc(8), sc(7))
    } else {
        Rect::new(ax - sc(10) + lunge, cy - sc(8), sc(13), sc(11))
    };
    let lw = if u == 3 { 3 } else { 2 };
    let draw_legs = |c: &mut Canvas, far: bool| {
        for i in 0..4 {
            let group = (i + usize::from(far)) % 2;
            let lift = lifts(beat, group) * sc(1);
            let hip = (ceph.x + sc(2) + (i as i32) * sc(1), ceph.bottom() - sc(2));
            let fx = [sc(12), sc(5), -sc(3), -sc(10)][i] + i32::from(far) * sc(2) - lift;
            let knee = (ax + lunge + fx / 2 + sc(2) * (1 - i as i32 / 2), cy - sc(6) * r / 2 - lift);
            let raised = matches!(beat, Beat::Attack(0 | 1)) && i == 0;
            let foot = if raised { (ceph.right() + sc(4), cy - sc(8)) } else { (ax + lunge + fx, ay - 1 - lift) };
            leg(c, k, hip, knee, foot, lw, if far { relief::FAR } else { relief::LEG });
            if far {
                c.shade(Rect::new(foot.0.min(knee.0) - 2, knee.1 - 2, (foot.0 - knee.0).abs() + 5, ay - knee.1 + 3), k.body, 1);
            }
        }
    };
    draw_legs(c, true);
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(ab, Ix::INK, 1);
    m.ellipse(ceph, Ix::INK, 1);
    c.inflate(&m, k.body, sc(3), relief::BODY);
    mark(c, k, ab, false);
    draw_legs(c, false);
    let z = relief::LEG.hi;
    c.set_emitting(k.eye_emits);
    c.dot(ceph.right() - sc(2), ceph.y + sc(2), eye_ix(k), z);
    if u == 3 {
        c.dot(ceph.right() - sc(3), ceph.y + sc(1), eye_ix(k), z);
    }
    c.set_emitting(false);
    let fang = if k.look.anatomy == Anatomy::Queen { Ramp::Bone } else { k.body };
    c.line((ceph.right() - 1, ceph.bottom() - sc(2)), (ceph.right() + i32::from(beat == Beat::Attack(1)), ceph.bottom()), fang.at(Tone::Light), 1, z);
}

/// On its back, the legs curled up over it.
fn dead(c: &mut Canvas, k: &Coat) {
    let u = unit(k);
    let sc = |v: i32| v * u / 2;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let body = Rect::new(ax - sc(7), ay - sc(6), sc(14), sc(6));
    for i in 0..4 {
        for side in [-1, 1] {
            let hip = (ax + side * sc(2 + i), ay - sc(5));
            let knee = (ax + side * sc(4 + i * 2), ay - sc(10));
            let foot = (ax + side * sc(2 + i), ay - sc(12));
            c.line(hip, knee, k.body.at(Tone::Base), 2, 3);
            c.line(knee, foot, k.body.at(Tone::Shade), 2, 3);
        }
    }
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(body, Ix::INK, 1);
    c.inflate(&m, k.body, sc(2), Z::new(2, 4));
    if k.belly != k.body {
        c.dye_ellipse(Rect::new(body.x + sc(2), body.y + 1, body.w - sc(4), body.h - 2), k.body, k.belly);
    }
}
