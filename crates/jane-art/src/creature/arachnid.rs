//! `arachnid` (spider, the queen, the lurker): ART.md §2.2. A cephalothorax and a bigger
//! abdomen behind it, each a soft volume, the abdomen marked down its back (a spider's chevrons,
//! the queen's pale skull), eight legs of two segments whose knees stand above the body, a
//! cluster of eyes that shine, and fangs under them. It walks on alternate legs: four lift and
//! reach while four hold. The queen is drawn half as large again, bristling, with eight eyes in
//! two rows; the lurker is a small pale body slung low between long thin legs.

use alloc::vec::Vec;
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
    // Jointed and lit: the thigh dark with its upper edge catching the light in the belly's
    // colour (a black spider's legs sheen brown, the queen's plum), a bright knee, the shin in
    // shade with a band, a pale claw. Every stroke two px or side-touching, so none is taken for
    // a spike.
    let (r, hl) = (k.body, k.belly);
    c.line(hip, knee, r.at(Tone::Base), w, z.lo);
    super::insect::stair(c, (hip.0, hip.1 - 1), (knee.0, knee.1 - 1), hl.at(Tone::Light), z.hi);
    c.line(knee, foot, r.at(Tone::Shade), 2, z.lo);
    let (mx, my) = ((knee.0 * 2 + foot.0) / 3, (knee.1 * 2 + foot.1) / 3);
    c.fill_rect(Rect::new(mx, my, 2, 1), hl.at(Tone::Base), z.hi);
    c.fill_rect(Rect::new(knee.0 - 1, knee.1 - 1, 2, 2), hl.at(Tone::Light), z.hi);
    c.dot(knee.0 - 1, knee.1 - 1, hl.at(Tone::High), z.hi);
    c.dot(foot.0, foot.1, hl.at(Tone::Light), z.hi);
}

/// A gloss on a round body part: a bright cluster in its upper left and a glint in it.
fn gloss(c: &mut Canvas, k: &Coat, r: Rect, z: u8) {
    let (x, y) = (r.x + r.w / 4, r.y + r.h / 5);
    let big = r.w >= 12;
    c.fill_rect(Rect::new(x, y, if big { 4 } else { 3 }, 2), k.body.at(Tone::Light), z);
    c.fill_rect(Rect::new(x + 1, y, if big { 2 } else { 1 }, 1), k.body.at(Tone::High), z);
    c.dot(x, y, k.body.at(Tone::Glint), z);
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    match (facing, beat) {
        (_, Beat::Dead) => dead(c, k),
        (Facing::Side, _) => side(c, k, beat),
        (Facing::Down, _) | (_, Beat::Idle(_)) => top(c, k, beat, true, false),
        (Facing::Up, _) => top(c, k, beat, false, false),
        (Facing::DownRight, _) => top(c, k, beat, true, true),
        (Facing::UpRight, _) => top(c, k, beat, false, true),
    }
}

/// `p` turned an eighth about `o` on the ground (ART.md §2.2): the body's line from the
/// abdomen to the head swung from straight down (or up) the screen to down (or up) and to the
/// right, every leg with it, the screen's depth a little squashed.
fn turn_about(o: (i32, i32), p: (i32, i32), toward: bool) -> (i32, i32) {
    let (x, y) = (p.0 - o.0, p.1 - o.1);
    let (tx, ty) = if toward { (x + y, y - x) } else { (x - y, x + y) };
    (o.0 + tx * 7 / 10, o.1 + ty * 6 / 10)
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
        // An hourglass in the mark's red down the middle of the back: two triangles tip to tip.
        let cy = ab.y + ab.h / 2;
        let hh = (ab.h / 3).max(3);
        let hw = (ab.w / 5).max(2);
        c.dye_poly(&[(mid - hw, cy - hh), (mid + hw - 1, cy - hh), (mid, cy), (mid - 1, cy)], k.body, k.mark);
        c.dye_poly(&[(mid - 1, cy), (mid, cy), (mid + hw - 1, cy + hh), (mid - hw, cy + hh)], k.body, k.mark);
        let _ = toward_us;
    }
}

/// From the front or behind, 3/4 top-down: the abdomen behind the head up the screen (or
/// toward the viewer from behind), the legs in a ring round it.
fn top(c: &mut Canvas, k: &Coat, beat: Beat, toward_us: bool, turned: bool) {
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
    let ceph = if lurker {
        Rect::new(ax - sc(3), ceph_y - sc(3), sc(6), sc(5))
    } else {
        Rect::new(ax - sc(5), ceph_y - sc(4), sc(10), sc(8))
    };
    // Turned (the diagonals), everything swings an eighth about the body's middle, and the
    // lowest foot is set back on the ground.
    let o = ((ab.x + ab.w / 2 + ceph.x + ceph.w / 2) / 2, (ab.y + ab.h / 2 + ceph.y + ceph.h / 2) / 2);
    let rt = |p: (i32, i32)| if turned { turn_about(o, p, toward_us) } else { p };
    let move_rect = |r: Rect| {
        let (x, y) = rt((r.x + r.w / 2, r.y + r.h / 2));
        Rect::new(x - r.w / 2, y - r.h / 2, r.w, r.h)
    };
    let (ab, ceph) = (move_rect(ab), move_rect(ceph));
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
            let (hip, knee, mut foot) = (rt(hip), rt(knee), rt(foot));
            foot.1 = foot.1.min(ay - 1);
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
    gloss(c, k, ab, relief::BODY.hi);
    let mut h = Canvas::new(c.w(), c.h());
    h.ellipse(ceph, Ix::INK, 1);
    c.inflate(&h, k.body, sc(2), Z::new(relief::BODY.lo + 1, relief::BODY.hi + 1));
    gloss(c, k, ceph, relief::BODY.hi + 1);
    for &(_, _, hip, knee, foot) in legs.iter().filter(|l| !back(l.1)) {
        leg(c, k, hip, knee, foot, lw, relief::LEG);
    }
    if toward_us {
        // The eyes in a cluster at the front, the fangs under them. Turned, the cluster sits
        // toward the head's lower right, the far eyes closing up on the near.
        let (hx, ey) = (ceph.x + ceph.w / 2 + i32::from(turned) * sc(2), ceph.bottom() - sc(3));
        let squeeze = |dx: i32| if turned { dx * 2 / 3 } else { dx };
        let z = relief::LEG.hi;
        c.set_emitting(k.eye_emits);
        // Two big eyes in front, 2 x 2, and the small ones in a ring over them.
        for dx in [-2, 1] {
            c.fill_rect(Rect::new(hx + squeeze(dx), ey, 2, 2), eye_ix(k), z);
        }
        let small: &[(i32, i32)] = match k.look.anatomy {
            Anatomy::Queen => &[(-4, -1), (-3, -3), (-1, -3), (2, -3), (4, -3), (5, -1)],
            _ => &[(-3, -2), (-1, -2), (2, -2), (4, -2)],
        };
        for &(dx, dy) in small {
            c.dot(hx + squeeze(dx) - 1, ey + dy + 1, eye_ix(k), z);
        }
        c.set_emitting(false);
        let fang = if k.look.anatomy == Anatomy::Queen { Ramp::Bone } else { k.body };
        let open = i32::from(beat == Beat::Attack(1));
        for dx in [-1 - open, open] {
            let x = hx + dx + i32::from(turned);
            c.vline(x, ceph.bottom(), ceph.bottom() + sc(2) - 1, fang.at(Tone::Light), z);
        }
    } else if turned {
        // From behind and to the right, the far edge of the eye cluster shows past the head.
        let z = relief::LEG.hi;
        c.set_emitting(k.eye_emits);
        c.fill_rect(Rect::new(ceph.right() - sc(2), ceph.y + 1, 2, 1), eye_ix(k), z);
        c.set_emitting(false);
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
                c.shade(
                    Rect::new(foot.0.min(knee.0) - 2, knee.1 - 2, (foot.0 - knee.0).abs() + 5, ay - knee.1 + 3),
                    k.body,
                    1,
                );
            }
        }
    };
    draw_legs(c, true);
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(ab, Ix::INK, 1);
    m.ellipse(ceph, Ix::INK, 1);
    c.inflate(&m, k.body, sc(3), relief::BODY);
    mark(c, k, ab, false);
    gloss(c, k, ab, relief::BODY.hi);
    gloss(c, k, ceph, relief::BODY.hi);
    draw_legs(c, false);
    let z = relief::LEG.hi;
    c.set_emitting(k.eye_emits);
    c.dot(ceph.right() - sc(2), ceph.y + sc(2), eye_ix(k), z);
    if u == 3 {
        c.dot(ceph.right() - sc(3), ceph.y + sc(1), eye_ix(k), z);
    }
    c.set_emitting(false);
    let fang = if k.look.anatomy == Anatomy::Queen { Ramp::Bone } else { k.body };
    c.line(
        (ceph.right() - 1, ceph.bottom() - sc(2)),
        (ceph.right() + i32::from(beat == Beat::Attack(1)), ceph.bottom()),
        fang.at(Tone::Light),
        1,
        z,
    );
}

/// On its back: the belly up in its own colour, the legs drawn in over it in hooks, pair by
/// pair, the way a dead spider's close.
fn dead(c: &mut Canvas, k: &Coat) {
    let u = unit(k);
    let sc = |v: i32| v * u / 2;
    let (_, _, ax, ay) = super::size_of(k.look.plan, k.look.anatomy);
    let body = Rect::new(ax - sc(6), ay - sc(6), sc(12), sc(6));
    let mut m = Canvas::new(c.w(), c.h());
    m.ellipse(body, Ix::INK, 1);
    c.inflate(&m, k.body, sc(2), Z::new(2, 4));
    c.dye_ellipse(Rect::new(body.x + sc(2), body.y + 1, body.w - sc(4), body.h - 2), k.body, k.belly);
    for i in 0..4 {
        for side in [-1, 1] {
            let hip = (ax + side * sc(2 + i), body.y + sc(1));
            let knee = (ax + side * sc(4 + i), body.y - sc(3 + i % 2 * 2));
            let claw = (ax + side * sc(2 + i / 2), knee.1 - sc(2));
            super::insect::stair(c, hip, knee, k.belly.at(Tone::Base), 5);
            super::insect::stair(c, knee, claw, k.body.at(Tone::Light), 5);
            c.dot(knee.0, knee.1, k.belly.at(Tone::Light), 5);
        }
    }
}
