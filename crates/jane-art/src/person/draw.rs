//! The composer's parts (ART.md §2.1): legs, boots, coat, front, arms, skin, face, hair, hat,
//! pack, in that order, for each facing, all read from one [`Rig`] so the frame table moves
//! everything together. Bodies are `polygon_lit` and `soft_ellipse`, cloth is `folds`, hair is
//! `strokes`; every one of them writes its normals and relief as it goes. Heights here are
//! relief (a limb in front of a coat stands two px proud of it); [`Canvas::upright`] adds each
//! row's height above the feet afterwards.

use jane_core::grid::Rect;
use jane_data::{Boots, Coat, Face, Front, Hat, Legs};

use super::build::Proportions;
use super::hair;
use super::pose::{Facing, Pose};
use super::{AY, Dress};
use crate::canvas::{Canvas, Z};
use crate::palette::{Ramp, Tone};

/// The centre line: a symmetric span of even width `w` is `CX - w / 2 ..= CX + w / 2 - 1`.
pub(crate) const CX: i32 = 16;

/// Relief, px: what stands in front of what. A part two above its neighbour gets a `K` seam.
pub(crate) mod relief {
    use crate::canvas::Z;
    pub const LEG: Z = Z::new(1, 2);
    pub const BOOT: Z = Z::new(2, 3);
    pub const COAT: Z = Z::new(3, 5);
    pub const FRONT: u8 = 7;
    pub const ARM: Z = Z::new(6, 8);
    pub const FAR: Z = Z::new(1, 2);
    pub const PACK: Z = Z::new(6, 9);
    pub const SKULL: Z = Z::new(2, 6);
    pub const HAIR: Z = Z::new(2, 4);
    pub const HAT: Z = Z::new(9, 12);
}

/// A symmetric span of width `w` about the centre line: `(first, last)` column.
const fn span(w: i32) -> (i32, i32) {
    (CX - w / 2, CX - w / 2 + w - 1)
}

/// Where everything is in one frame: the build moved by the pose.
pub(crate) struct Rig {
    pub p: Proportions,
    pub pose: Pose,
    /// The skull's box.
    pub skull: Rect,
    /// The shoulders' top row.
    pub top: i32,
    /// The waist's row.
    pub waist: i32,
    /// The hip's row: where the legs leave the coat.
    pub hip: i32,
    /// The coat's last row.
    pub hem: i32,
}

impl Rig {
    fn new(p: Proportions, pose: Pose, coat: Coat) -> Rig {
        let breathe = i32::from(pose.breathe);
        let (hy, top) = (pose.bob - breathe, p.shoulder_y() + pose.bob - breathe);
        let skull = Rect::new(CX - (p.head_w - 2) / 2, p.skull_y() + hy, p.head_w - 2, p.skull_h());
        let hip = p.hip_y() + pose.bob;
        let hem = match coat {
            Coat::Jacket | Coat::Cardigan => hip - 1,
            Coat::Coat | Coat::Smock | Coat::Canvas => hip + 1,
            Coat::Dress | Coat::Apron | Coat::Overcoat => hip + 3,
            Coat::Gown | Coat::Nightdress => AY - 2,
        };
        Rig { p, pose, skull, top, waist: (top + hip) / 2 + 1, hip, hem }
    }

    /// The eyes' first row.
    pub fn eye_y(&self) -> i32 {
        self.skull.y + self.skull.h / 2
    }

    /// The fringe's row: where hair meets forehead.
    pub fn brow_y(&self) -> i32 {
        self.eye_y() - 2
    }
}

/// One living frame of a person, before the outline.
pub fn frame(d: &Dress, p: Proportions, facing: Facing, pose: Pose, seed: u32) -> Canvas {
    let mut c = Canvas::new(super::W, super::H);
    let r = Rig::new(p, pose, d.look.body.coat);
    match facing {
        Facing::Down => down(&mut c, d, &r, seed),
        Facing::Up => up(&mut c, d, &r, seed),
        Facing::Side => side(&mut c, d, &r, seed),
    }
    c.unchecker(d.skin);
    c.ao_contact(Rect::new(CX - 9, AY - 2, 18, 5), 3);
    // Seams by relief alone, before each row's height is added: a fringe is not a step up.
    c.outline();
    c.upright(AY);
    c
}

// ---------------------------------------------------------------------------------------------
// Facing the viewer

fn down(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
    hair::back_down(c, d, r);
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, foot);
    }
    coat_front(c, d, r);
    front_down(c, d, r);
    let (s0, s1) = span(r.p.shoulder_w);
    for i in 0..2 {
        let hand = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + r.pose.arm[i];
        let x0 = if i == 0 { s0 - 3 } else { s1 };
        arm_front(c, d, r, x0, hand, i == 0);
    }
    neck(c, d, r);
    c.ellipse_lit(r.skull, d.skin, relief::SKULL);
    face_down(c, d, r);
    hair::front_down(c, d, r, seed);
    hat_down(c, d, r);
}

/// A leg and its boot seen from the front or behind, 4 px wide from `x0`, the sole on `foot`.
fn leg_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, foot: i32) {
    let boot_h = boot_rows(d);
    let legs = leg_ramp(d);
    let top = r.hip - 1;
    let bot = foot - boot_h;
    if bot >= top {
        c.polygon_lit(&[(x0, top), (x0 + 3, top), (x0 + 3, bot), (x0, bot)], legs, 70, relief::LEG);
    }
    if d.look.body.legs == Legs::Pyjamas {
        c.folds(Rect::new(x0, top, 4, bot - top + 1), legs, 2, 0);
    }
    boot_front(c, d, x0, foot, boot_h);
}

fn boot_rows(d: &Dress) -> i32 {
    match d.look.body.boots {
        Boots::Boots => 3,
        Boots::Shoes | Boots::Bare => 2,
    }
}

/// The legs' ramp: bare legs are skin; under a skirt, stockings in the legs' ramp.
fn leg_ramp(d: &Dress) -> Ramp {
    match d.look.body.legs {
        Legs::Bare => d.skin,
        Legs::Skirt | Legs::Trousers | Legs::Pyjamas => d.legs,
    }
}

/// A skirt below a coat too short to cover the knee: from the hip to three rows under it.
fn skirt(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, x1: i32) {
    if d.look.body.legs == Legs::Skirt && r.hem < r.hip + 3 {
        let b = r.hip + 3;
        c.polygon_lit(&[(x0, r.hip - 2), (x1, r.hip - 2), (x1 + 1, b), (x0 - 1, b)], d.legs, 80, relief::LEG);
        c.folds(Rect::new(x0 - 1, r.hip - 2, x1 - x0 + 3, 6), d.legs, 3, r.pose.phase);
    }
}

fn boot_front(c: &mut Canvas, d: &Dress, x0: i32, foot: i32, rows: i32) {
    let ramp = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(x0, foot - rows + 1, 4, rows), ramp, 1, 1, relief::BOOT);
}

/// The coat from the front: shoulders, waist and a hem by the coat's cut, lit as an upright
/// body, with folds below the waist.
fn coat_front(c: &mut Canvas, d: &Dress, r: &Rig) {
    let (s0, s1) = span(r.p.shoulder_w);
    let (w0, w1) = span(r.p.waist_w);
    let flare = match d.look.body.coat {
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => 2,
        Coat::Overcoat | Coat::Coat | Coat::Smock => 1,
        _ => 0,
    };
    let (h0, h1) = span(r.p.hip_w + 2 * flare);
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    let (k0, k1) = span(r.p.hip_w);
    skirt(c, d, r, k0, k1);
    let pts = [(s0 + 1, t), (s1 - 1, t), (s1, t + 1), (w1, wy), (h1, hem), (h0, hem), (w0, wy), (s0, t + 1)];
    c.polygon_lit(&pts, d.coat, 80, relief::COAT);
    c.folds(Rect::new(h0, wy + 1, h1 - h0 + 1, hem - wy), d.coat, 4, r.pose.phase.wrapping_add(8192));
    match d.look.body.coat {
        Coat::Coat | Coat::Overcoat | Coat::Jacket | Coat::Cardigan | Coat::Canvas => {
            // The opening, lit on its left lip, and two buttons.
            c.vline(CX, wy, hem - 1, d.coat.at(Tone::Shade), relief::COAT.hi);
            for k in 0..2 {
                c.dot(CX - 1, t + 3 + 2 * k, d.coat.at(Tone::Deep), relief::COAT.hi + 1);
            }
        }
        Coat::Smock => {
            c.hline(s0 + 1, s1 - 1, t + 3, d.coat.at(Tone::Shade), relief::COAT.hi);
        }
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => {
            c.hline(w0, w1, wy, d.coat.at(Tone::Shade), relief::COAT.hi);
        }
    }
}

/// What shows at the front of the coat, facing the viewer.
fn front_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    let apron_coat = d.look.body.coat == Coat::Apron;
    match d.look.body.front {
        Front::Scarf => {
            c.rect_round(Rect::new(CX - 5, t - 2, 10, 4), f, 1, 1, Z::new(relief::FRONT, relief::FRONT + 1));
            let sway = i32::from(r.pose.phase >= 32768);
            c.polygon_lit(
                &[(CX + 1, t + 1), (CX + 3, t + 1), (CX + 3 + sway, t + 6), (CX + 1 + sway, t + 7)],
                f,
                60,
                Z::flat(relief::FRONT + 1),
            );
            c.hline(CX + 1 + sway, CX + 3 + sway, t + 6, f.at(Tone::Shade), relief::FRONT + 1);
        }
        Front::Shirt | Front::Tie => {
            c.polyline_fill(
                &[(CX - 3, t - 1), (CX + 2, t - 1), (CX, t + 2), (CX - 1, t + 2)],
                f.at(Tone::Light),
                relief::FRONT,
            );
            if d.look.body.front == Front::Tie {
                c.polygon_lit(
                    &[(CX - 1, t), (CX, t), (CX, t + 5), (CX - 1, t + 5)],
                    d.coat,
                    40,
                    Z::flat(relief::FRONT + 1),
                );
            }
        }
        Front::Waistcoat => {
            c.polygon_lit(
                &[(CX - 3, t), (CX + 2, t), (CX + 2, r.waist + 1), (CX - 3, r.waist + 1)],
                f,
                60,
                Z::flat(relief::FRONT),
            );
            for k in 0..3 {
                c.dot(CX - 1, t + 1 + 2 * k, Ramp::Brass.at(Tone::Light), relief::FRONT + 1);
            }
        }
        Front::Braces => {
            for x in [CX - 3, CX + 2] {
                c.vline(x, t, r.hip - 1, f.at(Tone::Base), relief::FRONT);
            }
        }
        Front::Apron => apron_down(c, f, r),
        Front::None => {}
    }
    if apron_coat && d.look.body.front != Front::Apron {
        apron_down(c, f, r);
    }
}

fn apron_down(c: &mut Canvas, f: Ramp, r: &Rig) {
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    c.polygon_lit(
        &[(CX - 3, t + 1), (CX + 2, t + 1), (CX + 2, wy), (CX + 4, hem - 1), (CX - 5, hem - 1), (CX - 3, wy)],
        f,
        50,
        Z::flat(relief::FRONT),
    );
    c.hline(CX - 4, CX + 3, wy, f.at(Tone::Shade), relief::FRONT + 1);
}

/// An arm from the front: a sleeve 4 px wide from the shoulder to the wrist, then the hand.
/// `outer_left`: the arm on the screen's left.
fn arm_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, hand: i32, outer_left: bool) {
    let top = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe);
    let wrist = hand - 2;
    // The sleeve leans out from the shoulder: its top a pixel toward the body.
    let a = if outer_left { x0 + 1 } else { x0 - 1 };
    c.polygon_lit(&[(a, top), (a + 3, top), (x0 + 3, wrist), (x0, wrist)], d.coat, 80, relief::ARM);
    c.ellipse_lit(Rect::new(x0, wrist, 4, 3), d.skin, Z::new(relief::ARM.lo, relief::ARM.hi));
}

fn neck(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    c.polygon_lit(&[(CX - 2, y), (CX + 1, y), (CX + 1, r.top), (CX - 2, r.top)], d.skin, 60, Z::new(1, 2));
    c.hline(CX - 2, CX + 1, y + 2, d.skin.at(Tone::Shade), 2);
}

fn face_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    c.set_emitting(d.eye_emits);
    for x in [CX - 4, CX + 3] {
        c.vline(x, ey, ey + 1, d.eye, z);
    }
    c.set_emitting(false);
    c.dot(CX, ey + 2, d.skin.at(Tone::Shade), z);
    c.hline(CX - 1, CX, ey + 4, d.skin.at(Tone::Shade), z);
    match d.look.head.face {
        Face::Glasses => {
            // Wire rims round each eye, lit on the glass above it; a bridge between.
            // Two lenses catching the light either side of each eye, and the bridge.
            for x in [CX - 5, CX + 2] {
                c.vline(x, ey, ey + 1, Ramp::Glass.at(Tone::Light), z);
                c.vline(x + 2, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            }
            c.hline(CX - 2, CX + 1, ey, Ramp::Iron.at(Tone::Shade), z);
        }
        Face::Beard => {
            c.polygon_lit(
                &[(CX - 5, ey + 2), (CX + 4, ey + 2), (CX + 3, r.skull.bottom()), (CX - 4, r.skull.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
            c.hline(CX - 1, CX, ey + 3, d.skin.at(Tone::Deep), z + 1);
        }
        Face::Grim => {
            c.hline(CX - 2, CX + 1, ey + 4, d.skin.at(Tone::Deep), z);
            c.hline(CX - 5, CX - 3, ey - 1, d.hair.at(Tone::Deep), z);
            c.hline(CX + 2, CX + 4, ey - 1, d.hair.at(Tone::Deep), z);
        }
        Face::Plain | Face::None => {}
    }
}

pub(crate) fn hides_hair(d: &Dress) -> bool {
    matches!(d.look.head.hat, Hat::Scarf | Hat::Helmet | Hat::Diving | Hat::Veil)
}

/// The hat from the front.
fn hat_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    match d.look.head.hat {
        Hat::None => {}
        Hat::Cap => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() - 1)));
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, 10), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x, r.brow_y() - 2, s.w, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Peaked => {
            c.rect_round(Rect::new(s.x - 1, s.y - 3, s.w + 2, 6), h, 1, 2, z);
            c.rect_round(Rect::new(s.x, s.y + 2, s.w, 2), Ramp::Leather, 1, 1, Z::new(z.hi, z.hi + 1));
            c.disc_lit(CX - 1, s.y, 1, Ramp::Brass, Z::flat(z.hi + 2));
        }
        Hat::Brim | Hat::Panama => {
            c.soft_ellipse(Rect::new(s.x + 1, s.y - 5, s.w - 2, 8), h, z);
            if d.look.head.hat == Hat::Panama {
                c.hline(s.x + 1, s.right() - 2, s.y + 1, d.coat.at(Tone::Shade), z.hi);
            }
            c.soft_ellipse(Rect::new(s.x - 4, s.y + 1, s.w + 8, 4), h, Z::new(z.hi, z.hi + 1));
        }
        Hat::Cloche => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() + 1)));
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, 12), h, z);
            c.set_clip(None);
            c.hline(s.x - 1, s.right(), r.brow_y(), h.at(Tone::Shade), z.hi);
        }
        Hat::Helmet => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() - 1)));
            c.soft_ellipse(Rect::new(s.x, s.y - 7, s.w, 14), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x - 1, r.brow_y() - 2, s.w + 2, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
            c.disc_lit(CX - 1, s.y - 1, 1, Ramp::Brass, Z::flat(z.hi + 2));
        }
        Hat::Scarf => {
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h), h, z);
            // The face shows through the scarf's opening.
            c.set_clip(Some(Rect::new(s.x + 2, r.brow_y(), s.w - 4, s.bottom() - r.brow_y() - 1)));
            c.ellipse_lit(r.skull, d.skin, relief::SKULL);
            c.set_clip(None);
            face_down(c, d, r);
            c.soft_ellipse(Rect::new(CX - 2, s.bottom() - 2, 4, 3), h, Z::new(z.hi, z.hi + 1));
        }
        Hat::Veil | Hat::Diving => {
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, s.h + 2), h, z);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// From behind

fn up(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, foot);
    }
    coat_front(c, d, r);
    if d.look.body.pack {
        let t = r.top;
        c.rect_round(Rect::new(CX - 5, t + 2, 10, 8), d.pack, 1, 2, relief::PACK);
        c.hline(CX - 4, CX + 3, t + 5, d.pack.at(Tone::Shade), relief::PACK.hi + 1);
        c.dot(CX - 1, t + 6, Ramp::Brass.at(Tone::Light), relief::PACK.hi + 1);
    }
    let (s0, s1) = span(r.p.shoulder_w);
    for i in 0..2 {
        let hand = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + r.pose.arm[i];
        let x0 = if i == 0 { s0 - 3 } else { s1 };
        arm_front(c, d, r, x0, hand, i == 0);
    }
    neck(c, d, r);
    hair::whole_up(c, d, r, seed);
    hat_up(c, d, r);
}

fn hat_up(c: &mut Canvas, d: &Dress, r: &Rig) {
    // From behind a hat is its crown and brim; a scarf covers the back of the head.
    match d.look.head.hat {
        Hat::Scarf => {
            let s = r.skull;
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 1), d.hat, relief::HAT);
        }
        _ => hat_down(c, d, r),
    }
}

// ---------------------------------------------------------------------------------------------
// From the side, facing east

fn side(c: &mut Canvas, d: &Dress, r: &Rig, seed: u32) {
    let t = r.top;
    let shoulder = (CX - 1, r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe));
    // The far arm and leg, behind everything.
    arm_side(c, d, r, shoulder, r.pose.arm[1], relief::FAR, true);
    leg_side(c, d, r, r.pose.leg[1], r.pose.lift[1], true);
    hair::back_side(c, d, r);
    if d.look.body.pack {
        c.rect_round(Rect::new(CX - 11, t + 1, 6, 8), d.pack, 1, 2, relief::FAR);
    }
    leg_side(c, d, r, r.pose.leg[0], r.pose.lift[0], false);
    coat_side(c, d, r);
    front_side(c, d, r);
    if d.look.body.pack {
        c.line((CX + 1, t), (CX - 3, r.waist), d.pack.at(Tone::Base), 1, relief::FRONT);
    }
    neck_side(c, d, r);
    let s = Rect::new(r.skull.x, r.skull.y, r.skull.w, r.skull.h);
    c.ellipse_lit(s, d.skin, relief::SKULL);
    face_side(c, d, r);
    hair::side(c, d, r, seed);
    hat_side(c, d, r);
    arm_side(c, d, r, shoulder, r.pose.arm[0], relief::ARM, false);
}

/// A leg seen from the side, hip to foot, the boot pointing east.
fn leg_side(c: &mut Canvas, d: &Dress, r: &Rig, swing: i32, lift: i32, far: bool) {
    let legs = leg_ramp(d);
    let ramp = if far { dim(legs) } else { legs };
    let hx = CX - 2;
    let foot = AY - lift;
    let fx = hx + swing;
    let rows = boot_rows(d);
    let top = r.hip - 1;
    let z = if far { relief::FAR } else { relief::LEG };
    c.polygon_lit(&[(hx, top), (hx + 3, top), (fx + 3, foot - rows), (fx, foot - rows)], ramp, 70, z);
    let boot = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    let boot = if far { dim(boot) } else { boot };
    c.rect_round(Rect::new(fx, foot - rows + 1, 5, rows), boot, 1, 1, if far { relief::FAR } else { relief::BOOT });
}

/// The far side's ramp: the same (the light pass shades it through its lower relief).
const fn dim(r: Ramp) -> Ramp {
    r
}

fn coat_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let w = r.p.waist_w + 2;
    let x0 = CX - w / 2 - 1;
    let x1 = x0 + w - 1;
    let flare = match d.look.body.coat {
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => 2,
        Coat::Overcoat | Coat::Coat | Coat::Smock => 1,
        _ => 0,
    };
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    skirt(c, d, r, x0, x1);
    let pts =
        [(x0 + 2, t), (x1 - 2, t), (x1, t + 2), (x1, wy), (x1 + flare, hem), (x0 - flare, hem), (x0, wy), (x0, t + 2)];
    c.polygon_lit(&pts, d.coat, 80, relief::COAT);
    c.folds(Rect::new(x0 - flare, wy + 1, w + 2 * flare, hem - wy), d.coat, 4, r.pose.phase);
    if matches!(d.look.body.coat, Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress) {
        c.hline(x0, x1, wy, d.coat.at(Tone::Shade), relief::COAT.hi);
    }
}

fn front_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    let w = r.p.waist_w + 2;
    let x1 = CX - w / 2 - 1 + w - 1;
    match d.look.body.front {
        Front::Scarf => {
            c.rect_round(Rect::new(CX - 3, t - 2, 8, 4), f, 1, 1, Z::new(relief::FRONT, relief::FRONT + 1));
            let sway = i32::from(r.pose.phase >= 32768);
            c.polygon_lit(
                &[(x1 - 1, t + 1), (x1 + 1, t + 1), (x1 + 1 - sway, t + 6), (x1 - 1 - sway, t + 6)],
                f,
                60,
                Z::flat(relief::FRONT + 1),
            );
        }
        Front::Shirt | Front::Tie | Front::Waistcoat => {
            c.polygon_lit(&[(x1 - 2, t), (x1, t), (x1, t + 4), (x1 - 1, t + 4)], f, 40, Z::flat(relief::FRONT));
        }
        Front::Braces => c.vline(CX, t, r.hip - 1, f.at(Tone::Base), relief::FRONT),
        Front::Apron => apron_side(c, f, r, x1),
        Front::None => {}
    }
    if d.look.body.coat == Coat::Apron && d.look.body.front != Front::Apron {
        apron_side(c, f, r, x1);
    }
}

fn apron_side(c: &mut Canvas, f: Ramp, r: &Rig, x1: i32) {
    c.polygon_lit(
        &[(x1 - 1, r.top + 1), (x1, r.top + 1), (x1 + 1, r.hem - 1), (x1 - 2, r.hem - 1)],
        f,
        30,
        Z::flat(relief::FRONT),
    );
    c.hline(CX - 3, x1, r.waist, f.at(Tone::Shade), relief::FRONT + 1);
}

/// An arm from the side: shoulder to hand, the hand `swing` px along the facing.
fn arm_side(c: &mut Canvas, d: &Dress, r: &Rig, shoulder: (i32, i32), swing: i32, z: Z, far: bool) {
    let (sx, sy) = shoulder;
    let hand = (sx + swing, sy + r.p.arm_l - 3);
    let ramp = if far { dim(d.coat) } else { d.coat };
    c.polygon_lit(&[(sx - 1, sy), (sx + 2, sy), (hand.0 + 2, hand.1), (hand.0 - 1, hand.1)], ramp, 80, z);
    c.ellipse_lit(Rect::new(hand.0 - 1, hand.1, 4, 3), d.skin, z);
}

fn neck_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    c.polygon_lit(&[(CX - 1, y), (CX + 2, y), (CX + 2, r.top), (CX - 1, r.top)], d.skin, 60, Z::new(1, 2));
}

fn face_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    let fx = s.right() - 3;
    c.set_emitting(d.eye_emits);
    c.vline(fx, ey, ey + 1, d.eye, z);
    c.set_emitting(false);
    // The nose: a pixel past the skull.
    c.dot(s.right(), ey + 2, d.skin.at(Tone::Base), z);
    c.dot(s.right() - 1, ey + 4, d.skin.at(Tone::Shade), z);
    match d.look.head.face {
        Face::Glasses => {
            c.vline(fx + 1, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            c.vline(fx - 1, ey, ey + 1, Ramp::Glass.at(Tone::Light), z);
            c.hline(s.x + 6, fx - 2, ey, Ramp::Iron.at(Tone::Shade), z);
        }
        Face::Beard => {
            c.polygon_lit(
                &[(fx - 3, ey + 2), (s.right() - 1, ey + 3), (s.right() - 2, s.bottom()), (fx - 3, s.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
        }
        Face::Grim => c.hline(fx - 1, fx + 1, ey - 1, d.hair.at(Tone::Deep), z),
        Face::Plain | Face::None => {}
    }
}

fn hat_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    match d.look.head.hat {
        Hat::None => {}
        Hat::Cap => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() - 1)));
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, 10), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x + 6, r.brow_y() - 2, s.w - 3, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Peaked => {
            c.rect_round(Rect::new(s.x - 1, s.y - 3, s.w + 1, 6), h, 1, 2, z);
            c.rect_round(Rect::new(s.x + 8, s.y + 2, s.w - 4, 2), Ramp::Leather, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Brim | Hat::Panama => {
            c.soft_ellipse(Rect::new(s.x + 1, s.y - 5, s.w - 2, 8), h, z);
            if d.look.head.hat == Hat::Panama {
                c.hline(s.x + 1, s.right() - 2, s.y + 1, d.coat.at(Tone::Shade), z.hi);
            }
            c.soft_ellipse(Rect::new(s.x - 3, s.y + 1, s.w + 6, 3), h, Z::new(z.hi, z.hi + 1));
        }
        Hat::Cloche => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() + 1)));
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, 12), h, z);
            c.set_clip(None);
        }
        Hat::Helmet => {
            c.set_clip(Some(Rect::new(0, 0, 32, r.brow_y() - 1)));
            c.soft_ellipse(Rect::new(s.x, s.y - 7, s.w, 14), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x - 1, r.brow_y() - 2, s.w + 3, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Scarf => {
            c.soft_ellipse(Rect::new(s.x - 1, s.y - 2, s.w + 1, s.h), h, z);
            c.set_clip(Some(Rect::new(s.right() - 5, r.brow_y(), 6, s.bottom() - r.brow_y() - 1)));
            c.ellipse_lit(r.skull, d.skin, relief::SKULL);
            c.set_clip(None);
            face_side(c, d, r);
        }
        Hat::Veil | Hat::Diving => c.soft_ellipse(Rect::new(s.x - 1, s.y - 3, s.w + 2, s.h + 2), h, z),
    }
}
