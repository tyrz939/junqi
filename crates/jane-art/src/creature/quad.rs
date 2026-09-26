//! `quadruped_mid` (dog, sheep) and `quadruped_small` (cat, rat, rabbit, fox): ART.md §2.2.
//!
//! One rig for every four-legged animal, read from its [`Anat`]: a torso of two masses (the
//! rump and the chest) joined by the waist, a neck up to a skull and a muzzle, two legs a side
//! (the far pair behind the body and in its shade), ears and a tail by feature. Each mass is a
//! silhouette filled as one soft volume, so a flank turns and a chest rounds; the pelt's
//! clusters, the markings and the face are painted over it.
//!
//! The gait is a six-frame trot, the diagonal pairs together (near fore with far hind), the
//! standing frame first: stand, contact, down, pass, contact, down, the tail swinging a beat
//! behind the body. The idle pair faces the viewer: a dog or a cat sits (the second beat tilts
//! the head and sweeps the tail), a sheep grazes, a rabbit sits up, a rat washes. Dead is the
//! plan's own pose: a dog or a sheep on its side, a small beast on its back.

use jane_core::grid::Rect;
use jane_data::{Anatomy, Ears, Marking, Plan, Tail};

use super::{Beat, Coat, Facing};
use crate::canvas::{Canvas, StrokeKind, Z};
use crate::hash::{h32, salt};
use crate::palette::{Ix, Ramp, Tone};

/// Relief, px: what stands in front of what (the outline seams a part two above its neighbour).
mod relief {
    use crate::canvas::Z;
    pub const FAR: Z = Z::new(1, 2);
    pub const TAIL: Z = Z::new(1, 3);
    pub const BODY: Z = Z::new(2, 5);
    pub const LEG: Z = Z::new(3, 5);
    pub const HEAD: Z = Z::new(7, 10);
    pub const EAR: u8 = 11;
}

/// An animal's build, px, in its plan's box, facing east and standing.
#[derive(Clone, Copy, Debug)]
struct Anat {
    /// The rump's mass (an ellipse).
    rump: Rect,
    /// The chest's mass (an ellipse).
    chest: Rect,
    /// The belly's row under the waist.
    belly: i32,
    /// The skull (an ellipse) and the muzzle (a rounded box; `w` 0 for none).
    skull: Rect,
    muzzle: Rect,
    /// The eye's top-left, and its size (1 or 2).
    eye: (i32, i32),
    eye_w: i32,
    /// The front and hind legs' left column, the row they leave the body, their width.
    fore: i32,
    hind: i32,
    leg_top: i32,
    leg_w: i32,
    /// The tail's root.
    tail: (i32, i32),
    /// Facing the viewer: the body's breadth, the head's box (its top row), the legs' width.
    front_w: i32,
    head_w: i32,
    head_h: i32,
    head_top: i32,
    /// Facing the viewer: the top of the back, running away up the screen behind the head.
    back: i32,
}

fn anat(a: Anatomy, plan: Plan) -> Anat {
    let (_, _, _, ay) = super::size(plan);
    match a {
        // Dog: a working dog's build, deep in the chest, tucked at the waist, the head carried
        // high on a strong neck; big dark eyes with a glint.
        Anatomy::Dog => Anat {
            rump: Rect::new(7, ay - 15, 9, 8),
            chest: Rect::new(14, ay - 16, 10, 9),
            belly: ay - 10,
            skull: Rect::new(20, ay - 22, 8, 7),
            muzzle: Rect::new(26, ay - 19, 4, 4),
            eye: (24, ay - 20),
            eye_w: 2,
            fore: 18,
            hind: 9,
            leg_top: ay - 9,
            leg_w: 3,
            tail: (8, ay - 14),
            front_w: 12,
            head_w: 12,
            head_h: 9,
            head_top: ay - 22,
            back: ay - 20,
        },
        Anatomy::Sheep => Anat {
            rump: Rect::new(5, ay - 13, 11, 10),
            chest: Rect::new(13, ay - 14, 11, 11),
            belly: ay - 5,
            skull: Rect::new(21, ay - 15, 6, 6),
            muzzle: Rect::new(25, ay - 13, 4, 4),
            eye: (24, ay - 13),
            eye_w: 1,
            fore: 18,
            hind: 9,
            leg_top: ay - 5,
            leg_w: 2,
            tail: (6, ay - 11),
            front_w: 16,
            head_w: 8,
            head_h: 7,
            head_top: ay - 13,
            back: ay - 18,
        },
        Anatomy::Cat => Anat {
            rump: Rect::new(5, ay - 9, 7, 6),
            chest: Rect::new(10, ay - 9, 7, 6),
            belly: ay - 5,
            skull: Rect::new(14, ay - 14, 7, 6),
            muzzle: Rect::new(19, ay - 11, 2, 2),
            eye: (17, ay - 12),
            eye_w: 1,
            fore: 13,
            hind: 6,
            leg_top: ay - 5,
            leg_w: 2,
            tail: (5, ay - 8),
            front_w: 8,
            head_w: 8,
            head_h: 6,
            head_top: ay - 12,
            back: ay - 13,
        },
        Anatomy::Rat => Anat {
            rump: Rect::new(6, ay - 8, 7, 6),
            chest: Rect::new(10, ay - 7, 6, 5),
            belly: ay - 3,
            skull: Rect::new(14, ay - 8, 5, 5),
            muzzle: Rect::new(18, ay - 6, 3, 2),
            eye: (16, ay - 7),
            eye_w: 1,
            fore: 14,
            hind: 7,
            leg_top: ay - 3,
            leg_w: 2,
            tail: (6, ay - 4),
            front_w: 7,
            head_w: 6,
            head_h: 5,
            head_top: ay - 7,
            back: ay - 8,
        },
        Anatomy::Rabbit => Anat {
            rump: Rect::new(5, ay - 10, 9, 9),
            chest: Rect::new(11, ay - 8, 6, 6),
            belly: ay - 3,
            skull: Rect::new(14, ay - 11, 6, 6),
            muzzle: Rect::new(18, ay - 8, 2, 2),
            eye: (17, ay - 10),
            eye_w: 1,
            fore: 15,
            hind: 6,
            leg_top: ay - 3,
            leg_w: 2,
            tail: (5, ay - 8),
            front_w: 9,
            head_w: 7,
            head_h: 6,
            head_top: ay - 10,
            back: ay - 11,
        },
        Anatomy::Fox => Anat {
            rump: Rect::new(5, ay - 10, 7, 6),
            chest: Rect::new(10, ay - 10, 8, 7),
            belly: ay - 5,
            skull: Rect::new(15, ay - 14, 6, 6),
            muzzle: Rect::new(19, ay - 11, 4, 3),
            eye: (19, ay - 11),
            eye_w: 1,
            fore: 14,
            hind: 7,
            leg_top: ay - 5,
            leg_w: 2,
            tail: (5, ay - 9),
            front_w: 9,
            head_w: 9,
            head_h: 7,
            head_top: ay - 12,
            back: ay - 13,
        },
        // Birds are the bird plan's; a stand-in so the table is total.
        Anatomy::Hen | Anatomy::Crow => anat(Anatomy::Cat, plan),
    }
}

/// One frame's pose.
#[derive(Clone, Copy, Debug, Default)]
struct Pose {
    /// The body's drop, px (down positive).
    bob: i32,
    /// Each leg's reach along the facing (forward positive) and lift: near fore, far fore,
    /// near hind, far hind.
    reach: [i32; 4],
    lift: [i32; 4],
    /// The tail's swing, -2..=2: where its tip is.
    wag: i32,
    /// The head's offset (a lunge, a flinch).
    head: (i32, i32),
    /// The chest a px up (the breathe).
    breathe: bool,
    mouth: bool,
    shut: bool,
}

/// The trot: pair A is the near fore and the far hind, pair B the far fore and the near hind.
/// `(reach A, lift A, reach B, lift B, bob, wag)`.
const TROT: [Stride; 6] = [
    stride((0, 0), (0, 0), 0, 0),
    stride((2, 0), (-2, 0), 0, 1),
    stride((1, 0), (-1, 1), 1, 2),
    stride((0, 0), (0, 2), 0, 1),
    stride((-2, 0), (2, 0), 0, -1),
    stride((-1, 1), (1, 0), 1, -2),
];

/// One beat of a gait: pair A's and pair B's `(reach, lift)`, the body's bob, the tail's swing.
#[derive(Clone, Copy, Debug)]
struct Stride {
    a: (i32, i32),
    b: (i32, i32),
    bob: i32,
    wag: i32,
}

const fn stride(a: (i32, i32), b: (i32, i32), bob: i32, wag: i32) -> Stride {
    Stride { a, b, bob, wag }
}

fn pose(beat: Beat) -> Pose {
    match beat {
        Beat::Walk(k) => {
            let Stride { a: (ra, la), b: (rb, lb), bob, wag } = TROT[usize::from(k % 6)];
            Pose { bob, reach: [ra, rb, rb, ra], lift: [la, lb, lb, la], wag, ..Pose::default() }
        }
        Beat::Breathe => Pose { breathe: true, wag: -1, ..Pose::default() },
        Beat::Hurt => Pose { head: (-1, 1), shut: true, wag: -2, reach: [-1, -1, 1, 1], ..Pose::default() },
        Beat::Attack(0) => Pose { head: (-2, 1), bob: 1, reach: [-1, -1, 1, 1], wag: -1, ..Pose::default() },
        Beat::Attack(1) => Pose { head: (3, 1), mouth: true, reach: [3, 2, 1, 1], wag: 2, ..Pose::default() },
        Beat::Attack(_) => Pose { head: (1, 0), reach: [1, 1, 0, 0], wag: 1, ..Pose::default() },
        Beat::Idle(k) => Pose { wag: if k == 0 { -1 } else { 2 }, ..Pose::default() },
        Beat::Dead => Pose { shut: true, ..Pose::default() },
    }
}

pub(crate) fn draw(c: &mut Canvas, k: &Coat, facing: Facing, beat: Beat) {
    let a = anat(k.look.anatomy, k.look.plan);
    let p = pose(beat);
    match (facing, beat) {
        (_, Beat::Dead) => dead(c, k, &a),
        (_, Beat::Idle(i)) => idle(c, k, &a, i),
        (Facing::Side, _) => side(c, k, &a, &p),
        (Facing::Down, _) => front(c, k, &a, &p, true),
        (Facing::Up, _) => front(c, k, &a, &p, false),
    }
}

// -------------------------------------------------------------------------------------------
// Parts

/// A fresh mask the size of `c`.
fn mask(c: &Canvas) -> Canvas {
    Canvas::new(c.w(), c.h())
}

/// A leg seen from the side: from its top `(x, top)` down to the ground on `ay`, `reach` px along
/// the facing and lifted `lift`; `hind` bends back at the hock. A paw a px longer than the leg,
/// pointing forward.
#[allow(clippy::too_many_arguments)]
fn leg_side(c: &mut Canvas, ramp: Ramp, x: i32, top: i32, w: i32, reach: i32, lift: i32, hind: bool, ay: i32, z: Z) {
    let foot = ay - lift;
    let fx = x + reach;
    let mut m = mask(c);
    if hind && foot - top >= 4 {
        // The thigh, broad at the top; the hock a px behind the foot two rows up.
        let hock = foot - 2;
        let pts = [(x - 1, top - 1), (x + w, top - 1), (x + w, top + 1), (fx + w - 1, hock), (fx - 1, hock)];
        m.polyline_fill(&pts, Ix::INK, 1);
        m.fill_rect(Rect::new(fx, hock, w - 1, foot - hock), Ix::INK, 1);
        m.fill_rect(Rect::new(fx - 1, hock, 1, 1), Ix::INK, 1);
    } else {
        let pts = [(x, top - 1), (x + w - 1, top - 1), (fx + w - 1, foot - 1), (fx, foot - 1)];
        m.polyline_fill(&pts, Ix::INK, 1);
    }
    // The paw.
    m.fill_rect(Rect::new(fx, foot - 1, w + 1, 2), Ix::INK, 1);
    c.inflate(&m, ramp, 1, z);
}

/// A leg facing the viewer or away: a column `w` wide from `top` to the ground, a paw under it.
#[allow(clippy::too_many_arguments)]
fn leg_post(c: &mut Canvas, ramp: Ramp, x: i32, top: i32, w: i32, lift: i32, ay: i32, z: Z) {
    let foot = ay - lift;
    let mut m = mask(c);
    m.fill_rect(Rect::new(x, top, w, (foot - top).max(1)), Ix::INK, 1);
    m.fill_rect(Rect::new(x, foot - 1, w, 2), Ix::INK, 1);
    c.inflate(&m, ramp, 1, z);
    // The toes: a line of shade up into the paw.
    if w >= 3 {
        c.tint(x + w / 2, foot, ramp, Tone::Shade);
    }
}

/// An eye at `(x, y)`: dark, `w` px square, a glint in its top corner toward the light; shut, a
/// lid line.
fn eye(c: &mut Canvas, k: &Coat, x: i32, y: i32, w: i32, shut: bool, z: u8) {
    if shut {
        c.hline(x, x + w - 1, y + w - 1, k.body.at(Tone::Deep), z);
        return;
    }
    c.set_emitting(k.eye_emits);
    c.fill_rect(Rect::new(x, y, w, w), k.iris(), z);
    c.set_emitting(false);
    if w >= 2 && !k.eye_emits {
        c.dot(x, y, crate::palette::letter('w').unwrap_or(Ix::BEVEL_LIGHT), z);
    }
}

/// The nose: `k`, with a glint (the dog's), or a skin-pink one.
fn nose(c: &mut Canvas, a: Anatomy, x: i32, y: i32, z: u8) {
    match a {
        Anatomy::Dog | Anatomy::Fox => {
            c.fill_rect(Rect::new(x, y, 2, 2), Ix::INK, z);
            c.dot(x, y, Ramp::ClothBlack.at(Tone::Light), z);
        }
        Anatomy::Cat | Anatomy::Rabbit | Anatomy::Rat => c.dot(x, y, Ramp::Skin.at(Tone::Mid), z),
        _ => {}
    }
}

/// The muzzle's ramp: grizzled, marked or the body's.
fn muzzle_ramp(k: &Coat) -> Ramp {
    if k.look.markings.contains(&Marking::TanPoints) {
        k.mark
    } else if k.look.anatomy == Anatomy::Fox {
        // A fox's white cheeks and throat.
        k.belly
    } else {
        k.body
    }
}

/// A tail from `root`, facing east (it streams west), swung `wag` (-2..=2).
fn tail_side(c: &mut Canvas, k: &Coat, root: (i32, i32), wag: i32) {
    let (x, y) = root;
    let mut m = mask(c);
    let ramp = if k.look.anatomy == Anatomy::Rat { Ramp::Skin } else { k.body };
    match k.look.tail {
        Tail::Plume => {
            // A working dog's tail: up and back in a feathered curve, the tip a px higher with the
            // swing.
            let tip = (x - 5, y - 4 - wag.max(0));
            m.polyline_fill(&[(x + 1, y - 1), (x + 1, y + 2), (x - 3, y), (tip.0 - 1, tip.1 + 2), tip, (x - 2, y - 3)], Ix::INK, 1);
            // The feathering under it.
            m.polyline_fill(&[(x - 1, y + 1), (x - 4, y + 2), (tip.0, tip.1 + 3)], Ix::INK, 1);
        }
        Tail::Brush => {
            // A fox's brush: long, full, carried low and back.
            let tip = (x - 7, y + 3 - wag / 2);
            m.polyline_fill(
                &[(x + 1, y - 1), (x - 3, y - 2), (tip.0 - 1, tip.1 - 1), (tip.0 - 1, tip.1 + 1), (x - 3, y + 3), (x + 1, y + 2)],
                Ix::INK,
                1,
            );
        }
        Tail::Long => {
            // A cat's: out and up in an S, the tip hooked.
            let s = wag.signum();
            let pts = [(x, y), (x - 2, y - 1), (x - 3, y - 3), (x - 3, y - 5 - s), (x - 2 + s, y - 7)];
            m.polyline(&pts, Ix::INK, 2, 1);
        }
        Tail::Thin => {
            // A rat's: bare, long, trailing on the ground.
            let pts = [(x, y), (x - 3, y + 2), (x - 5, y + 3 + wag.clamp(-1, 0)), (x - 6, y + 3)];
            m.polyline(&pts, Ix::INK, 1, 1);
        }
        Tail::Puff => {
            m.ellipse(Rect::new(x - 3, y - 1, 4, 4), Ix::INK, 1);
        }
        Tail::Stub => {
            m.ellipse(Rect::new(x - 2, y, 3, 3), Ix::INK, 1);
        }
        Tail::Fan | Tail::None => return,
    }
    c.inflate(&m, ramp, 2, relief::TAIL);
    match k.look.tail {
        Tail::Plume if k.look.markings.contains(&Marking::TipWhite) => {
            c.dye_ellipse(Rect::new(x - 7, y - 5 - wag.max(0), 4, 4), k.body, k.belly);
        }
        Tail::Puff => c.dye_ellipse(Rect::new(x - 4, y - 2, 6, 6), k.body, k.belly),
        Tail::Brush if k.look.markings.contains(&Marking::TipWhite) => {
            c.dye_ellipse(Rect::new(x - 9, y + 1 - wag / 2, 4, 4), k.body, k.belly);
        }
        _ => {}
    }
}

/// An ear, facing east, over the skull `s`: `far` is the other side's.
fn ear_side(c: &mut Canvas, k: &Coat, s: Rect, far: bool, lag: i32) {
    let ramp = if k.look.markings.contains(&Marking::DarkFace) { k.mark } else { k.body };
    // At the back of the skull, behind the eye; the far one a px further back.
    let x = s.x + 1 + i32::from(far) + i32::from(k.look.plan != Plan::QuadrupedMid);
    let y = s.y + 1;
    let kind = match (k.look.ears, far) {
        (Ears::FlopOne, true) => Ears::Prick,
        (Ears::FlopOne, false) => Ears::Flop,
        (e, _) => e,
    };
    let mut m = mask(c);
    match kind {
        Ears::Prick => {
            let h = if k.look.plan == Plan::QuadrupedMid { 4 } else { 3 };
            m.polyline_fill(&[(x - 1, y + 1), (x + 2, y + 1), (x + lag, y - h)], Ix::INK, 1);
        }
        Ears::Flop => {
            // Folded over at the top and hanging behind the eye, its tip a px lower as the
            // head comes down.
            m.polyline_fill(&[(x - 1, y - 2), (x + 1, y - 2), (x + 1, y + 2 + lag), (x, y + 3 + lag), (x - 1, y + 1)], Ix::INK, 1);
        }
        Ears::Tall => {
            let lean = -1 - lag;
            m.polyline_fill(&[(x - 1, y + 1), (x + 1, y + 1), (x + 1 + lean, y - 6), (x + lean, y - 7), (x - 1 + lean, y - 6)], Ix::INK, 1);
        }
        Ears::Round => m.ellipse(Rect::new(x - 1, y - 3, 3, 3), Ix::INK, 1),
        Ears::Side => m.polyline_fill(&[(x - 3, y + 1), (x + 1, y), (x + 1, y + 2)], Ix::INK, 1),
        Ears::None | Ears::FlopOne => return,
    }
    let z = if far { Z::new(relief::HEAD.lo - 1, relief::HEAD.lo) } else { Z::flat(relief::EAR) };
    c.inflate(&m, ramp, 1, z);
    // The inside of an ear that stands: a line of pink or of shade.
    if matches!(kind, Ears::Prick | Ears::Tall) && !far {
        let inner = if matches!(k.look.anatomy, Anatomy::Rabbit | Anatomy::Cat | Anatomy::Rat) {
            Ramp::Skin
        } else {
            ramp
        };
        for d in 0..2 {
            c.dot(x + lag.max(0), y - d, inner.at(Tone::Mid), relief::EAR);
        }
    }
    if far {
        c.shade(Rect::new(x - 3, y - 8, 7, 12), ramp, 1);
    }
}

/// The torso seen from the side: the rump and the chest joined along the topline and down to
/// the belly, and the neck up to the skull. One volume.
fn torso_side(c: &mut Canvas, k: &Coat, a: &Anat, bob: i32, breathe: bool, head: (i32, i32)) -> Rect {
    let mut m = mask(c);
    let lift = i32::from(breathe);
    let rump = Rect::new(a.rump.x, a.rump.y + bob, a.rump.w, a.rump.h);
    let chest = Rect::new(a.chest.x, a.chest.y + bob - lift, a.chest.w, a.chest.h + lift);
    m.ellipse(rump, Ix::INK, 1);
    m.ellipse(chest, Ix::INK, 1);
    // The waist: from the rump's middle to the chest's, down to the belly.
    let (x0, x1) = (rump.x + rump.w / 2, chest.x + chest.w / 2);
    let top = rump.y.max(chest.y) + 1;
    m.fill_rect(Rect::new(x0, top, x1 - x0, a.belly + bob - top), Ix::INK, 1);
    // The neck, from the chest's top front up to under the skull.
    let s = Rect::new(a.skull.x + head.0, a.skull.y + bob + head.1, a.skull.w, a.skull.h);
    if s.bottom() > chest.y - 1 || k.look.anatomy != Anatomy::Rat {
        let neck = [
            (chest.x + chest.w / 2 - 1, chest.y + 2),
            (s.x + 1, s.y + s.h / 2),
            (s.x + s.w / 2 + 1, s.bottom() - 1),
            (chest.right() - 1, chest.y + chest.h / 2),
        ];
        m.polyline_fill(&neck, Ix::INK, 1);
    }
    let radius = if k.look.plan == Plan::QuadrupedMid { 4 } else { 3 };
    c.inflate(&m, k.body, radius, relief::BODY);
    s
}

/// The head seen from the side on the skull `s`: the skull, the muzzle with its nose and mouth,
/// the eye, the near ear.
fn head_side(c: &mut Canvas, k: &Coat, a: &Anat, s: Rect, p: &Pose) {
    let an = k.look.anatomy;
    let (dx, dy) = (s.x - a.skull.x, s.y - a.skull.y);
    let mut m = mask(c);
    m.ellipse(s, Ix::INK, 1);
    let mz = Rect::new(a.muzzle.x + dx, a.muzzle.y + dy, a.muzzle.w, a.muzzle.h);
    if mz.w > 0 {
        match an {
            Anatomy::Fox | Anatomy::Rat => {
                // A pointed snout: a wedge to the nose.
                m.polyline_fill(&[(mz.x - 1, mz.y), (mz.right(), mz.y + mz.h / 2), (mz.x - 1, mz.bottom())], Ix::INK, 1);
            }
            _ => m.fill_rect(Rect::new(mz.x, mz.y + 1, mz.w, mz.h - 1), Ix::INK, 1),
        }
        if an == Anatomy::Dog || an == Anatomy::Sheep {
            m.ellipse(mz, Ix::INK, 1);
        }
    }
    let skin = if k.look.markings.contains(&Marking::DarkFace) { k.mark } else { k.body };
    c.inflate(&m, skin, 2, relief::HEAD);
    // Painted on the face at its rim height, so no detail stands proud enough to take a seam.
    let z = relief::HEAD.lo;
    let (ex, ey) = (a.eye.0 + dx, a.eye.1 + dy);
    // Tan points: the lips and the cheek under the eye, the bridge left dark.
    if mz.w > 0 && muzzle_ramp(k) != skin {
        c.dye_poly(
            &[(ex - 1, ey + 2), (ex + 2, ey + 2), (mz.right() - 2, mz.y + 2), (mz.right() - 1, mz.bottom()), (ex - 1, mz.bottom())],
            skin,
            muzzle_ramp(k),
        );
    }
    if k.look.markings.contains(&Marking::Grizzle) && mz.w > 0 {
        // An old dog's grey chin.
        for (x, y) in [(mz.x + 1, mz.bottom() - 1), (mz.x + 2, mz.bottom() - 1), (mz.x + 2, mz.bottom() - 2)] {
            if c.get(x, y).is_opaque() {
                c.dot(x, y, Ramp::HairGrey.at(Tone::Light), z);
            }
        }
    }
    // Nose and mouth.
    match an {
        Anatomy::Dog => {
            nose(c, an, mz.right() - 2, mz.y, z);
            let open = i32::from(p.mouth);
            c.hline(mz.x + 1, mz.right() - 2, mz.bottom() - 1 + open, skin.at(Tone::Deep), z);
            if p.mouth {
                c.dot(mz.right() - 2, mz.bottom() - 1, Ramp::Skin.at(Tone::Shade), z);
            }
        }
        Anatomy::Fox | Anatomy::Rat => {
            nose(c, an, mz.right() - 1, mz.y + mz.h / 2 - 1, z);
            if p.mouth {
                c.dot(mz.right() - 2, mz.y + mz.h / 2 + 1, Ix::INK, z);
            }
        }
        Anatomy::Cat | Anatomy::Rabbit => {
            nose(c, an, mz.right() - 1, mz.y, z);
            c.dot(mz.x, mz.bottom() - 1, k.belly.at(Tone::Light), z);
        }
        Anatomy::Sheep => {
            c.dot(mz.right() - 1, mz.y + 1, skin.at(Tone::Deep), z);
        }
        _ => {}
    }
    // The eye, with the tan brow over it.
    if k.look.markings.contains(&Marking::TanPoints) {
        c.dot(ex, ey - 1, k.mark.at(Tone::Light), z);
    }
    eye(c, k, ex, ey, a.eye_w, p.shut, z);
    if an == Anatomy::Cat && !p.shut {
        // A cat's eye is green-gold, a slit through it.
        c.fill_rect(Rect::new(ex, ey, 1, 2), Ramp::ClothMustard.at(Tone::Light), z);
        c.dot(ex, ey + 1, Ix::INK, z);
    }
}

/// A sheep's fleece seen from the side: five overlapping lit balls of wool over the torso.
fn fleece_side(c: &mut Canvas, k: &Coat, a: &Anat, bob: i32, breathe: bool) {
    let lift = i32::from(breathe);
    let (x0, x1) = (a.rump.x, a.chest.right());
    let (y0, y1) = (a.chest.y + bob - lift, a.belly + bob);
    let w = x1 - x0;
    let mut m = mask(c);
    let balls = [
        Rect::new(x0, y0 + 2, w * 2 / 5, y1 - y0 - 1),
        Rect::new(x0 + w / 5, y0, w * 2 / 5, (y1 - y0) * 2 / 3),
        Rect::new(x0 + w * 2 / 5, y0 + 1, w * 2 / 5, (y1 - y0) * 3 / 4),
        Rect::new(x1 - w * 2 / 5, y0 + 2, w * 2 / 5, y1 - y0 - 2),
        Rect::new(x0 + w / 4, y0 + (y1 - y0) / 3, w / 2, (y1 - y0) * 2 / 3 + 1),
    ];
    for b in balls {
        m.ellipse(b, Ix::INK, 1);
    }
    c.inflate(&m, k.body, 3, relief::BODY);
    // The wool's curls: small lit loops, hashed but clustered, never speckle.
    for (i, b) in balls.iter().enumerate().take(4) {
        let h = h32(k.seed, i as u32, salt::STROKES);
        let (cx, cy) = (b.x + b.w / 2 - 1 + (h & 1) as i32, b.y + 1 + (h >> 1 & 1) as i32);
        c.tint(cx, cy, k.body, Tone::High);
        c.tint(cx + 1, cy, k.body, Tone::Light);
        c.tint(cx, cy + 1, k.body, Tone::Light);
        c.tint(cx + 1, cy + 2, k.body, Tone::Mid);
        c.tint(cx + 2, cy + 1, k.body, Tone::Mid);
    }
}

// -------------------------------------------------------------------------------------------
// From the side, facing east

fn side(c: &mut Canvas, k: &Coat, a: &Anat, p: &Pose) {
    let (_, _, _, ay) = super::size(k.look.plan);
    let an = k.look.anatomy;
    let legs = if an == Anatomy::Sheep { k.mark } else { k.body };
    let leg_ramp = |i: usize| -> Ramp {
        if k.look.markings.contains(&Marking::Socks) { k.mark } else if i >= 2 && an == Anatomy::Rabbit { k.body } else { legs }
    };
    // The tail behind everything but the far legs' shade.
    tail_side(c, k, (a.tail.0, a.tail.1 + p.bob), p.wag);
    // The far legs, a px behind the near ones and in the body's shade.
    let top = a.leg_top + p.bob;
    leg_side(c, leg_ramp(1), a.fore - 1, top, a.leg_w, p.reach[1], p.lift[1], false, ay, relief::FAR);
    leg_side(c, leg_ramp(3), a.hind - 1, top, a.leg_w, p.reach[3], p.lift[3], true, ay, relief::FAR);
    c.shade(Rect::new(a.fore - 4, top - 2, a.leg_w + 8, ay - top + 4), leg_ramp(1), 1);
    c.shade(Rect::new(a.hind - 4, top - 2, a.leg_w + 8, ay - top + 4), leg_ramp(3), 1);
    let far = Rect::new(a.skull.x + p.head.0, a.skull.y + p.bob + p.head.1, a.skull.w, a.skull.h);
    ear_side(c, k, far, true, 0);
    // The body.
    let s = if an == Anatomy::Sheep {
        fleece_side(c, k, a, p.bob, p.breathe);
        Rect::new(a.skull.x + p.head.0, a.skull.y + p.bob + p.head.1, a.skull.w, a.skull.h)
    } else {
        torso_side(c, k, a, p.bob, p.breathe, p.head)
    };
    pelt_side(c, k, a, p);
    // The near legs.
    leg_side(c, leg_ramp(0), a.fore, top, a.leg_w, p.reach[0], p.lift[0], false, ay, relief::LEG);
    let hind_w = if an == Anatomy::Rabbit { a.leg_w + 2 } else { a.leg_w + i32::from(an == Anatomy::Dog) };
    leg_side(c, leg_ramp(2), a.hind, top, hind_w, p.reach[2], p.lift[2], true, ay, relief::LEG);
    if an == Anatomy::Rabbit {
        // The long hind foot flat on the ground.
        c.rect_round(Rect::new(a.hind - 1 + p.reach[2], ay - 1 - p.lift[2], 5, 2), k.body, 1, 1, relief::LEG);
    }
    if k.look.markings.contains(&Marking::TanPoints) {
        // Tan stockings on all four feet.
        c.dye_poly(&[(0, ay - 2), (c.w(), ay - 2), (c.w(), ay + 1), (0, ay + 1)], k.body, k.mark);
    }
    head_side(c, k, a, s, p);
    ear_side(c, k, s, false, p.bob.min(1) - i32::from(p.breathe));
    collar_side(c, k, s, a, p.bob);
}

/// The pelt's painted clusters seen from the side: the lit topline, the belly's colour, the
/// markings.
fn pelt_side(c: &mut Canvas, k: &Coat, a: &Anat, p: &Pose) {
    let an = k.look.anatomy;
    if an == Anatomy::Sheep {
        return;
    }
    let b = p.bob;
    // The belly and chest in their own colour: a band along the underside.
    if k.belly != k.body {
        let chest = a.chest;
        if k.look.markings.contains(&Marking::Blaze) {
            // A white shirt-front: the chest from the throat down, round at the front.
            c.dye_ellipse(Rect::new(chest.right() - 6, chest.y + 2 + b - i32::from(p.breathe), 7, chest.h), k.body, k.belly);
        } else {
            let pts = [
                (a.rump.x + 3, a.belly - 1 + b),
                (chest.right() - 1, chest.bottom() - 3 + b),
                (chest.right(), chest.bottom() + b),
                (a.rump.x + 3, a.belly + 1 + b),
            ];
            c.dye_poly(&pts, k.body, k.belly);
        }
    }
    if k.look.markings.contains(&Marking::Tabby) {
        // Stripes over the back and flank, each a two-px band in the mark's tone.
        for i in 0..3 {
            let x = a.rump.x + 2 + i * 3;
            for y in a.rump.y + b + 1..a.rump.y + b + 4 {
                c.tint(x, y, k.body, Tone::Shade);
                c.tint(x + 1, y + 1, k.body, Tone::Shade);
            }
        }
    }
    // A fur texture along the flank: two or three short tufts in the shade band, clustered.
    if k.look.plan == Plan::QuadrupedMid {
        let r = Rect::new(a.rump.x + 1, a.belly - 3 + b, a.chest.right() - a.rump.x - 3, 3);
        c.strokes(r, k.body, StrokeKind::Fur, 5, h32(k.seed, 1, salt::STROKES));
    }
}

/// A collar round the neck with a brass tag hanging from it (the dog's, with Julie's key).
fn collar_side(c: &mut Canvas, k: &Coat, s: Rect, a: &Anat, bob: i32) {
    let Some(ramp) = k.collar else { return };
    let _ = (a, bob);
    let (x, y) = (s.x + 1, s.bottom() - 1);
    let z = relief::HEAD.lo;
    c.line((x, y - 1), (x + 2, y + 2), ramp.at(Tone::Base), 2, z);
    c.dot(x + 1, y - 1, ramp.at(Tone::Light), z);
    c.fill_rect(Rect::new(x + 2, y + 3, 2, 2), Ramp::Brass.at(Tone::Base), z + 1);
    c.dot(x + 2, y + 3, Ramp::Brass.at(Tone::High), z + 1);
}

// -------------------------------------------------------------------------------------------
// Facing the viewer and away

fn front(c: &mut Canvas, k: &Coat, a: &Anat, p: &Pose, facing_us: bool) {
    let (w, _, ax, ay) = super::size(k.look.plan);
    let _ = w;
    let an = k.look.anatomy;
    let legs = if an == Anatomy::Sheep || k.look.markings.contains(&Marking::Socks) { k.mark } else { k.body };
    let b = p.bob;
    let bw = a.front_w;
    let body_top = a.back + b - i32::from(p.breathe);
    let leg_top = a.leg_top + b;
    if !facing_us {
        // The head is beyond the back: the body will cover its lower half.
        head_back(c, k, a, b + p.head.1);
    }
    // Front and hind legs alternate: the near pair are the fore legs facing us, the hind away.
    let lw = a.leg_w;
    let gap = if bw >= 12 { 2 } else { 1 };
    let (l0, l1) = (ax - gap / 2 - lw - (gap % 2), ax + (gap + 1) / 2);
    // The far pair, peeking out wide and higher up the screen (further away).
    let far_y = ay - 2;
    for (i, x) in [(0, l0 - 2), (1, l1 + 2)] {
        leg_post(c, legs, x, leg_top - 2, lw, p.lift[3 - i], far_y, relief::FAR);
    }
    // The tail: behind the body facing us (its tip over the back), over the rump facing away.
    if facing_us {
        tail_front(c, k, a, body_top, p.wag, true);
    }
    // The body.
    let mut m = mask(c);
    // Seen end on, the body is the chest (or the rump) over the legs, and the back running away
    // up the screen behind the head.
    let bh = (leg_top + 3 - body_top).max(6);
    m.ellipse(Rect::new(ax - bw / 2, body_top, bw, bh), Ix::INK, 1);
    if an == Anatomy::Sheep {
        for (dx, dy) in [(-bw / 2 - 1, 2), (bw / 2 - 3, 2), (-3, -1)] {
            m.ellipse(Rect::new(ax + dx, body_top + dy, 5, bh - 3), Ix::INK, 1);
        }
    }
    c.inflate(&m, k.body, 3, relief::BODY);
    if an == Anatomy::Sheep {
        for i in 0..3 {
            let h = h32(k.seed, 10 + i, salt::STROKES);
            let (x, y) = (ax - bw / 2 + 2 + (i as i32) * bw / 3 + (h & 1) as i32, body_top + 1 + (h >> 1 & 1) as i32);
            c.tint(x, y, k.body, Tone::High);
            c.tint(x + 1, y + 1, k.body, Tone::Mid);
        }
    }
    // The near legs.
    for (i, x) in [(0, l0), (1, l1)] {
        leg_post(c, legs, x, leg_top, lw, p.lift[i], ay, relief::LEG);
    }
    if !facing_us {
        tail_front(c, k, a, body_top, p.wag, false);
        if let Some(ramp) = k.collar {
            // The collar round the back of the neck, where the back meets the head.
            let y = a.head_top + a.head_h - 2 + b;
            c.hline(ax - 3, ax + 2, y, ramp.at(Tone::Base), relief::HEAD.hi);
        }
        return;
    }
    // The chest's colour: a bib under the chin, narrowing between the forelegs.
    if k.belly != k.body && an != Anatomy::Sheep {
        let cw = bw / 2 + 1;
        let y0 = a.head_top + a.head_h - 2 + b;
        c.dye_poly(&[(ax - cw / 2, y0), (ax + cw / 2, y0), (ax + 1, leg_top + 2), (ax - 2, leg_top + 2)], k.body, k.belly);
    }
    head_front(c, k, a, b + p.head.1, p.shut, p.mouth, 0);
    collar_front(c, k, a, b);
}

/// The tail seen from the front (its tip over the back, swinging) or from behind (over the rump).
fn tail_front(c: &mut Canvas, k: &Coat, a: &Anat, body_top: i32, wag: i32, behind: bool) {
    let (_, _, ax, ay) = super::size(k.look.plan);
    let _ = a;
    let ramp = if k.look.anatomy == Anatomy::Rat { Ramp::Skin } else { k.body };
    let mut m = mask(c);
    let sw = wag.signum() + wag / 2;
    match k.look.tail {
        Tail::Plume | Tail::Long => {
            let (x, y) = (ax + sw * 2, body_top - 2);
            if behind {
                m.polyline(&[(ax + sw, body_top + 2), (x + sw, y + 1), (x + 2 * sw, y - 1)], Ix::INK, 2, 1);
            } else {
                // Seen from behind it sweeps down over the hocks, side to side with the walk.
                let pw = if k.look.tail == Tail::Plume { 3 } else { 2 };
                m.polyline(&[(ax, ay - 9), (ax - 1 + sw, ay - 6), (ax - 2 + 2 * sw, ay - 3)], Ix::INK, pw, 1);
            }
        }
        Tail::Brush => {
            if behind {
                m.ellipse(Rect::new(ax + 3 + sw, body_top + 2, 5, 7), Ix::INK, 1);
            } else {
                m.ellipse(Rect::new(ax - 2 + sw, ay - 8, 5, 8), Ix::INK, 1);
            }
        }
        Tail::Thin => {
            if behind {
                return;
            }
            m.polyline(&[(ax, ay - 3), (ax + sw, ay), (ax + 2 + sw, ay + 2)], Ix::INK, 1, 1);
        }
        Tail::Puff if !behind => m.ellipse(Rect::new(ax - 2, ay - 6, 4, 4), Ix::INK, 1),
        _ => return,
    }
    c.inflate(&m, ramp, 2, if behind { relief::FAR } else { relief::HEAD });
    if k.look.tail == Tail::Puff {
        c.dye_ellipse(Rect::new(ax - 3, ay - 7, 6, 6), k.body, k.belly);
    }
    if k.look.tail == Tail::Plume && k.look.markings.contains(&Marking::TipWhite) {
        let tip = if behind { (ax + 3 * sw - 1, body_top - 4) } else { (ax - 3 + 2 * sw, ay - 4) };
        c.dye_ellipse(Rect::new(tip.0 - 1, tip.1, 4, 4), k.body, k.belly);
    }
    if k.look.tail == Tail::Brush && k.look.markings.contains(&Marking::TipWhite) {
        let y = if behind { body_top + 1 } else { ay - 3 };
        c.dye_ellipse(Rect::new(ax - 3 + sw, y, 8, 4), k.body, k.belly);
    }
}

/// The head facing the viewer: the skull, the muzzle under the eyes, the ears; `tilt` leans it
/// a px (the idle's second beat).
fn head_front(c: &mut Canvas, k: &Coat, a: &Anat, b: i32, shut: bool, mouth: bool, tilt: i32) {
    let (_, _, ax, _) = super::size(k.look.plan);
    let an = k.look.anatomy;
    let (hw, hh) = (a.head_w, a.head_h);
    let top = a.head_top + b;
    let s = Rect::new(ax - hw / 2 + tilt, top, hw, hh);
    // Ears behind the skull first where they stand, over it where they hang.
    let skin = if k.look.markings.contains(&Marking::DarkFace) { k.mark } else { k.body };
    ears_front(c, k, s, true, tilt);
    let mut m = mask(c);
    m.ellipse(s, Ix::INK, 1);
    // The muzzle: narrower, under the eyes, a little forward (lower on the screen).
    let (mw, mh) = match an {
        Anatomy::Dog => (6, 4),
        Anatomy::Sheep | Anatomy::Fox => (4, 3),
        Anatomy::Rat => (3, 2),
        _ => (4, 2),
    };
    let mz = Rect::new(ax - mw / 2 + tilt, s.bottom() - mh + 1, mw, mh);
    m.ellipse(mz, Ix::INK, 1);
    c.inflate(&m, skin, 3, relief::HEAD);
    // Painted on the face at its rim height, so no detail stands proud enough to take a seam.
    let z = relief::HEAD.lo;
    let ew = a.eye_w;
    // The eyes sit just above the muzzle.
    let ey = (mz.y - ew).min(s.y + hh / 2 - 1);
    let t = tilt;
    if muzzle_ramp(k) != skin {
        // Tan cheeks, and a tan dot over each eye: the brows that make a dog's face speak.
        c.dye_ellipse(mz, skin, muzzle_ramp(k));
        c.dot(ax - 4 + t, ey - 1, k.mark.at(Tone::Light), z);
        c.dot(ax + 3 + t, ey - 1, k.mark.at(Tone::Base), z);
    }
    if k.look.markings.contains(&Marking::Blaze) && an == Anatomy::Dog {
        // The white stripe down the middle of the face from the brow, widening over the muzzle
        // to the nose.
        let mut d = mask(c);
        d.fill_rect(Rect::new(ax - 1 + t, s.y + 1, 2, mz.y - s.y - 1), Ix::INK, 1);
        d.fill_rect(Rect::new(ax - 2 + t, mz.y - 1, 4, 3), Ix::INK, 1);
        c.dye(&d, skin, k.belly);
        c.dye(&d, muzzle_ramp(k), k.belly);
    }
    if k.look.markings.contains(&Marking::Grizzle) {
        for (x, y) in [(mz.x + 1, mz.bottom() - 1), (mz.right() - 2, mz.bottom() - 1), (mz.right() - 1, mz.bottom() - 2)] {
            if c.get(x, y).is_opaque() {
                c.dot(x, y, Ramp::HairGrey.at(Tone::Light), z);
            }
        }
    }
    if k.look.markings.contains(&Marking::Tabby) {
        for x in [ax - 2, ax, ax + 2] {
            c.tint(x + tilt, s.y + 1, k.body, Tone::Shade);
        }
    }
    // Eyes: set wide in a small face, close in a long one.
    let spread = match an {
        Anatomy::Dog | Anatomy::Sheep | Anatomy::Rabbit => 3,
        _ => 2,
    };
    for x in [ax - spread - ew + 1 + tilt, ax + spread - 1 + tilt] {
        eye(c, k, x, ey, ew, shut, z);
        if an == Anatomy::Cat && !shut {
            c.fill_rect(Rect::new(x, ey, 1, 1), Ramp::ClothMustard.at(Tone::Light), z);
        }
    }
    // The nose at the muzzle's top; the mouth under it.
    match an {
        Anatomy::Dog | Anatomy::Fox => {
            let ny = mz.y + i32::from(mz.h >= 4);
            c.fill_rect(Rect::new(ax - 1 + tilt, ny, 2, 2), Ix::INK, z);
            c.dot(ax - 1 + tilt, ny, Ramp::ClothBlack.at(Tone::Light), z);
            if mouth {
                c.hline(ax - 1 + tilt, ax + tilt, mz.bottom(), Ramp::Skin.at(Tone::Shade), z);
            }
        }
        Anatomy::Cat | Anatomy::Rabbit | Anatomy::Rat => {
            c.dot(ax - 1 + tilt, mz.y, Ramp::Skin.at(Tone::Mid), z);
            c.dot(ax + tilt, mz.y, Ramp::Skin.at(Tone::Mid), z);
        }
        Anatomy::Sheep => c.hline(ax - 1, ax, mz.bottom() - 1, skin.at(Tone::Deep), z),
        _ => {}
    }
    ears_front(c, k, s, false, tilt);
}

/// Ears facing the viewer: `behind` draws the ones that stand behind the skull's top, else the
/// ones that hang over its sides.
fn ears_front(c: &mut Canvas, k: &Coat, s: Rect, behind: bool, tilt: i32) {
    let an = k.look.anatomy;
    for side in [-1, 1] {
        let kind = match (k.look.ears, side) {
            // Seen from the front, the pricked one is on the screen's left.
            (Ears::FlopOne, -1) => Ears::Prick,
            (Ears::FlopOne, _) => Ears::Flop,
            (e, _) => e,
        };
        let stands = matches!(kind, Ears::Prick | Ears::Tall | Ears::Round);
        if stands != behind {
            continue;
        }
        let x = if side < 0 { s.x + 1 } else { s.right() - 2 };
        let mut m = mask(c);
        // The raised ear of the tilt goes up a px.
        let up = i32::from(tilt != 0 && side == tilt.signum());
        match kind {
            Ears::Prick => {
                let h = 4;
                let (a, b) = if side < 0 { (x - 1, x + 2) } else { (x - 2, x + 1) };
                m.polyline_fill(&[(a, s.y + 2), (b, s.y + 2), (x + side, s.y + 2 - h - up)], Ix::INK, 1);
            }
            Ears::Tall => {
                m.polyline_fill(&[(x - 1, s.y + 1), (x + 1, s.y + 1), (x + 1 + side, s.y - 7 - up), (x + side, s.y - 8 - up), (x - 1 + side, s.y - 7 - up)], Ix::INK, 1);
            }
            Ears::Round => m.ellipse(Rect::new(x - 1 + side, s.y - 2, 3, 3), Ix::INK, 1),
            Ears::Flop => {
                let o = if side < 0 { s.x - 2 } else { s.right() - 1 };
                m.polyline_fill(&[(o, s.y), (o + 2, s.y), (o + 2 + side.min(0), s.y + 5 - up), (o + 1, s.y + 6 - up), (o - side.max(0), s.y + 4 - up)], Ix::INK, 1);
            }
            Ears::Side => {
                let o = if side < 0 { s.x - 3 } else { s.right() - 1 };
                m.ellipse(Rect::new(o, s.y + 2, 4, 2), Ix::INK, 1);
            }
            Ears::None | Ears::FlopOne => continue,
        }
        let skin = if k.look.markings.contains(&Marking::DarkFace) { k.mark } else { k.body };
        let z = if behind { Z::new(relief::BODY.hi, relief::HEAD.lo) } else { Z::flat(relief::EAR) };
        c.inflate(&m, skin, 1, z);
        if stands && kind != Ears::Round {
            let inner = if matches!(an, Anatomy::Rabbit | Anatomy::Cat | Anatomy::Rat) { Ramp::Skin } else { skin };
            let ix = x + if side < 0 { 0 } else { -1 } + i32::from(side < 0);
            for d in 1..3 {
                c.tint(ix, s.y + 2 - d, skin, Tone::Shade);
                if inner != skin && c.get(ix, s.y + 2 - d).is_opaque() {
                    c.dot(ix, s.y + 2 - d, inner.at(Tone::Mid), z.hi);
                }
            }
        }
    }
}

/// The head from behind: the back of the skull and the ears.
fn head_back(c: &mut Canvas, k: &Coat, a: &Anat, b: i32) {
    let (_, _, ax, _) = super::size(k.look.plan);
    let s = Rect::new(ax - a.head_w / 2, a.head_top + b, a.head_w, a.head_h - 1);
    ears_front(c, k, s, true, 0);
    let mut m = mask(c);
    m.ellipse(s, Ix::INK, 1);
    let skin = if k.look.markings.contains(&Marking::DarkFace) { k.mark } else { k.body };
    c.inflate(&m, skin, 3, relief::HEAD);
    ears_front(c, k, s, false, 0);
}

fn collar_front(c: &mut Canvas, k: &Coat, a: &Anat, b: i32) {
    let Some(ramp) = k.collar else { return };
    let (_, _, ax, _) = super::size(k.look.plan);
    let y = a.head_top + a.head_h + b;
    let z = relief::HEAD.lo;
    c.hline(ax - 4, ax + 3, y, ramp.at(Tone::Base), z);
    c.hline(ax - 3, ax + 2, y + 1, ramp.at(Tone::Shade), z);
    c.dot(ax - 4, y, ramp.at(Tone::Light), z);
    c.fill_rect(Rect::new(ax - 1, y + 1, 2, 2), Ramp::Brass.at(Tone::Base), z + 1);
    c.dot(ax - 1, y + 1, Ramp::Brass.at(Tone::High), z + 1);
}

// -------------------------------------------------------------------------------------------
// The idle pair, facing the viewer

fn idle(c: &mut Canvas, k: &Coat, a: &Anat, beat: u8) {
    let (_, _, ax, ay) = super::size(k.look.plan);
    let an = k.look.anatomy;
    let tilt = i32::from(beat == 1);
    match an {
        Anatomy::Dog | Anatomy::Cat | Anatomy::Fox => {
            // Sitting: the haunches wide on the ground, the forelegs straight, the chest up, the
            // head high; the tail round the side on the ground, swept on the second beat.
            let lift = if an == Anatomy::Dog { 3 } else { 2 };
            let head_top = (a.head_top - lift).max(if k.look.plan == Plan::QuadrupedMid { 4 } else { 3 });
            let legs = if k.look.markings.contains(&Marking::Socks) { k.mark } else { k.body };
            // The tail round the far side, on the ground.
            let mut t = mask(c);
            let sweep = if beat == 1 { 2 } else { 0 };
            let tw = if an == Anatomy::Cat { 2 } else { 3 };
            t.polyline(&[(ax + 3, ay - 2), (ax + 6 + sweep, ay - 1), (ax + 8 + sweep, ay - 2 - sweep / 2)], Ix::INK, tw, 1);
            c.inflate(&t, k.body, 1, relief::FAR);
            if k.look.markings.contains(&Marking::TipWhite) || an == Anatomy::Fox {
                c.dye_ellipse(Rect::new(ax + 6 + sweep, ay - 4 - sweep / 2, 4, 4), k.body, k.belly);
            }
            // Haunches.
            let mut m = mask(c);
            let hw = a.front_w / 2 + 1;
            m.ellipse(Rect::new(ax - hw - 1, ay - 7, hw + 1, 7), Ix::INK, 1);
            m.ellipse(Rect::new(ax, ay - 7, hw + 1, 7), Ix::INK, 1);
            // The chest, upright.
            let top = head_top + a.head_h - 2;
            m.ellipse(Rect::new(ax - a.front_w / 2 + 1, top, a.front_w - 2, ay - top - 1), Ix::INK, 1);
            c.inflate(&m, k.body, 3, relief::BODY);
            // Hind paws forward at the haunches' fronts.
            let feet = if k.look.markings.contains(&Marking::TanPoints) { k.mark } else { legs };
            for x in [ax - hw - 1, ax + hw - 2] {
                c.rect_round(Rect::new(x, ay - 1, 3, 2), feet, 1, 1, relief::LEG);
            }
            // The bib: a shield from the throat narrowing down between the forelegs.
            if k.belly != k.body {
                let cw = a.front_w / 2 + 2;
                c.dye_poly(
                    &[(ax - cw / 2, top + 2), (ax + cw / 2, top + 2), (ax + cw / 2, top + 5), (ax + 1, ay - 3), (ax - 2, ay - 3), (ax - cw / 2 - 1, top + 5)],
                    k.body,
                    k.belly,
                );
            }
            // Forelegs over it.
            let lw = a.leg_w;
            for x in [ax - lw - 1, ax + 1] {
                leg_post(c, legs, x, ay - 7, lw, 0, ay, relief::LEG);
                if feet != legs {
                    c.dye_poly(&[(x, ay - 2), (x + lw, ay - 2), (x + lw, ay + 1), (x, ay + 1)], legs, feet);
                }
            }
            if k.look.markings.contains(&Marking::Tabby) {
                for y in [top + 3, top + 5] {
                    c.tint(ax - a.front_w / 2 + 1, y, k.body, Tone::Shade);
                    c.tint(ax + a.front_w / 2 - 2, y, k.body, Tone::Shade);
                }
            }
            let fake = Anat { head_top, ..*a };
            head_front(c, k, &fake, 0, false, false, tilt);
            collar_front(c, k, &fake, 0);
        }
        Anatomy::Sheep => {
            // Grazing: from the front, the head down to the grass, chewing on the second beat.
            let p = Pose { head: (0, 4 + i32::from(beat == 1)), ..Pose::default() };
            front(c, k, a, &p, true);
        }
        Anatomy::Rabbit => {
            // Sitting up, the ears high, the nose twitching on the second beat.
            let p = Pose { bob: -2 + i32::from(beat == 1), ..Pose::default() };
            front(c, k, a, &p, true);
        }
        _ => {
            // A rat washes its face: the head down a px, then up.
            let p = Pose { head: (0, i32::from(beat == 1)), ..Pose::default() };
            front(c, k, a, &p, true);
        }
    }
}

// -------------------------------------------------------------------------------------------
// Dead

/// The plan's own dead pose: a dog, a sheep or a fox on its side, legs out toward the viewer; a
/// small beast on its back, legs up.
fn dead(c: &mut Canvas, k: &Coat, a: &Anat) {
    let (_, _, _, ay) = super::size(k.look.plan);
    let an = k.look.anatomy;
    let drop = ay - 3 - a.belly;
    let low = Anat {
        rump: Rect::new(a.rump.x, a.rump.y + drop + 2, a.rump.w, a.rump.h - 2),
        chest: Rect::new(a.chest.x, a.chest.y + drop + 2, a.chest.w, a.chest.h - 2),
        belly: a.belly + drop,
        skull: Rect::new(a.skull.x + 1, ay - a.skull.h - 1, a.skull.w, a.skull.h),
        muzzle: Rect::new(a.muzzle.x + 1, ay - a.muzzle.h - 1, a.muzzle.w, a.muzzle.h),
        eye: (a.eye.0 + 1, a.eye.1 + (ay - a.skull.h - 1 - a.skull.y)),
        tail: (a.tail.0, a.tail.1 + drop + 2),
        ..*a
    };
    let legs = if an == Anatomy::Sheep || k.look.markings.contains(&Marking::Socks) { k.mark } else { k.body };
    let p = Pose { shut: true, ..Pose::default() };
    let small = !matches!(an, Anatomy::Dog | Anatomy::Sheep | Anatomy::Fox);
    // The legs, stiff: out toward the viewer on its side, up on its back.
    for (i, x) in [low.hind, low.hind + 2, low.fore, low.fore + 2].into_iter().enumerate() {
        let y0 = low.belly - 1;
        let (tx, ty) = if small { (x + 1 - (i as i32 % 2) * 2, y0 - 4) } else { (x + 3 + i as i32 % 2, ay) };
        let z = if i % 2 == 0 { relief::LEG } else { relief::FAR };
        c.line((x, y0), (tx, ty), legs.at(Tone::Base), a.leg_w.min(2), z.hi);
    }
    tail_side(c, k, low.tail, 0);
    let s = if an == Anatomy::Sheep {
        fleece_side(c, k, &low, 0, false);
        low.skull
    } else {
        torso_side(c, k, &low, 0, false, (0, 0))
    };
    head_side(c, k, &low, s, &p);
    ear_side(c, k, s, false, 1);
}
