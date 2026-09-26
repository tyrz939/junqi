//! The composer's parts (ART.md §2.1): legs, boots, coat, front, arms, skin, face, hair, hat,
//! pack, in that order, for each facing, all read from one [`Rig`] so the frame table moves
//! everything together. Bodies are `polygon_lit` and `ellipse_lit`, cloth hangs in fold lines, and every
//! part writes its normals and relief as it goes (relief: what stands in front of what, which
//! the outline reads for seams). Then [`finish`] gives each material its own few tones, lays the
//! cast shades as clusters, draws the contact shadow, runs the selective outline and stands the
//! frame up ([`Canvas::upright`]), which writes the true height of every pixel.

use jane_core::grid::Rect;
use jane_data::{Boots, Coat, Extra, Face, Front, Hat, Legs};

use super::build::Proportions;
use super::hair;
use super::pose::{Facing, Pose};
use super::{AY, Dress};
use crate::canvas::{Canvas, Z};
use crate::palette::{self, Ix, Ramp, Tone};

/// The centre line: a symmetric span of even width `w` is `CX - w / 2 ..= CX + w / 2 - 1`.
pub(crate) const CX: i32 = 16;

/// Relief, px: what stands in front of what. A part two above its neighbour gets a seam.
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

/// A material's own tones, by the tone the light gave it (darkest first): cloth and leather
/// keep a light, a base and a mid (a shade for what the light never reaches), so shading falls in clusters and never in every band a curve makes.
const CLOTH: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Base, Tone::Light, Tone::Light];
/// Skin: the same four a step lighter at the top, never its darkest (sel-out draws that).
const SKIN: [Tone; 8] =
    [Tone::Mid, Tone::Base, Tone::Base, Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::Light];

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
    /// The coat's last row: it hangs from the hip a frame behind the body.
    pub hem: i32,
    /// The upper body's lean (side), px east.
    pub lean: i32,
    /// How far what hangs loose trails the body: `(dx, dy)`, the previous frame's lean and bob
    /// less this frame's.
    pub trail: (i32, i32),
}

impl Rig {
    fn new(p: Proportions, pose: Pose, coat: Coat, facing: Facing, stoop: bool) -> Rig {
        let breathe = i32::from(pose.breathe);
        // A stoop carries the shoulders a px forward and down and the head a px further.
        let (bent, sunk) = (i32::from(stoop && facing == Facing::Side), i32::from(stoop));
        let lean = if facing == Facing::Side { pose.lean + bent } else { 0 };
        let (hy, top) = (pose.bob - breathe + sunk, p.shoulder_y() + pose.bob - breathe + sunk);
        let skull = Rect::new(CX - (p.head_w - 2) / 2 + lean + bent, p.skull_y() + hy, p.head_w - 2, p.skull_h());
        let hip = p.hip_y() + pose.bob;
        let hang = match coat {
            Coat::Jacket | Coat::Cardigan => -1,
            Coat::Coat | Coat::Smock | Coat::Canvas => 1,
            Coat::Dress | Coat::Apron | Coat::Overcoat => 3,
            Coat::Gown | Coat::Nightdress => AY - 2 - p.hip_y(),
        };
        let hem = (p.hip_y() + pose.lag.0 + hang).min(AY - 2);
        let trail = (pose.lag.1 + bent - lean, pose.lag.0 - pose.bob);
        Rig { p, pose, skull, top, waist: (top + hip) / 2 + 1, hip, hem, lean, trail }
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

/// One living frame of a person, finished.
pub fn frame(d: &Dress, p: Proportions, facing: Facing, pose: Pose) -> Canvas {
    let mut c = Canvas::new(super::W, super::H);
    let r = Rig::new(p, pose, d.look.body.coat, facing, d.look.extras.contains(&Extra::Stoop));
    match facing {
        Facing::Down => down(&mut c, d, &r),
        Facing::Up => up(&mut c, d, &r),
        Facing::Side => side(&mut c, d, &r),
    }
    finish(&mut c, d);
    c
}

/// Every material to its own tones, skin never in a checker, the contact shadow under the feet,
/// the selective outline (seams by relief), then each pixel's true height.
fn finish(c: &mut Canvas, d: &Dress) {
    for r in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack] {
        c.retone(r, CLOTH);
    }
    c.retone(d.skin, SKIN);
    c.unchecker(d.skin);
    for r in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, d.skin, d.hair] {
        c.declutter(r);
    }
    c.despike();
    c.ao_contact(Rect::new(CX - 7, AY - 1, 14, 4), 0);
    c.outline_sel();
    for r in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, d.skin, d.hair] {
        c.declutter(r);
    }
    c.unchecker(d.skin);
    c.upright(AY);
}

// ---------------------------------------------------------------------------------------------
// Facing the viewer

fn down(c: &mut Canvas, d: &Dress, r: &Rig) {
    hair::back_down(c, d, r);
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, foot);
    }
    coat_front(c, d, r, true);
    front_down(c, d, r);
    arms_front(c, d, r);
    neck(c, d, r);
    head(c, d, r, false);
    face_down(c, d, r);
    hair::front_down(c, d, r);
    hat_down(c, d, r);
}

fn arms_front(c: &mut Canvas, d: &Dress, r: &Rig) {
    let (s0, s1) = span(r.p.shoulder_w);
    for i in 0..2 {
        let hand = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + r.pose.arm[i];
        let x0 = if i == 0 { s0 - 3 } else { s1 };
        arm_front(c, d, r, x0, hand, i == 0);
    }
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
    let ramp = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(x0, foot - boot_h + 1, 4, boot_h), ramp, 1, 1, relief::BOOT);
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
        let b = r.hip + 3 + r.trail.1;
        c.polygon_lit(&[(x0, r.hip - 2), (x1, r.hip - 2), (x1 + 1, b), (x0 - 1, b)], d.legs, 80, relief::LEG);
        fold_lines(c, d.legs, x0 - 1, x1 + 1, b, 2, r.pose.phase);
    }
}

/// The neck's width: a pencil on a girl, a post on a smith.
fn neck_w(r: &Rig) -> i32 {
    if r.p.shoulder_w >= 14 { 6 } else { 4 }
}

/// The body's depth seen from the side: narrower than its breadth.
fn side_w(r: &Rig) -> i32 {
    (r.p.shoulder_w - 2).max(8)
}

/// Fold lines on a skirt or a coat's tail: a crease every few px across `x0..=x1`, each a
/// pixel of the ramp's shade rising `len` rows from the hem, the middle ones longest; the set
/// sways a pixel with the walk's phase.
fn fold_lines(c: &mut Canvas, ramp: Ramp, x0: i32, x1: i32, hem: i32, len: i32, phase: u16) {
    let sway = i32::from(phase >= 32768);
    let w = x1 - x0 + 1;
    let n = (w / 4).max(1);
    for k in 0..n {
        let x = x0 + (k + 1) * w / (n + 1) + sway - 1;
        let l = if k == 0 || k == n - 1 { len - 1 } else { len };
        for y in hem - l..hem {
            c.tint(x, y, ramp, Tone::Shade);
        }
    }
}

fn flare(coat: Coat) -> i32 {
    match coat {
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => 2,
        Coat::Overcoat | Coat::Coat | Coat::Smock => 1,
        _ => 0,
    }
}

/// The coat from the front (or behind): shoulders, waist and a hem by the coat's cut, lit as an
/// upright body, with folds below the waist, the head's shade on the collar and the cut's
/// details facing the viewer.
fn coat_front(c: &mut Canvas, d: &Dress, r: &Rig, facing_us: bool) {
    let (s0, s1) = span(r.p.shoulder_w);
    let (w0, w1) = span(r.p.waist_w);
    let coat = d.look.body.coat;
    let (h0, h1) = span(r.p.hip_w + 2 * flare(coat));
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    let (k0, k1) = span(r.p.hip_w);
    skirt(c, d, r, k0, k1);
    let pts = [(s0 + 1, t), (s1 - 1, t), (s1, t + 1), (w1, wy), (h1, hem), (h0, hem), (w0, wy), (s0, t + 1)];
    c.polygon_lit(&pts, d.coat, 80, relief::COAT);
    fold_lines(c, d.coat, h0, h1, hem, (hem - wy - 1).min(4), r.pose.phase);
    // The head's shade on the shoulders under it.
    c.shade(Rect::new(CX - 5, t - 2, 10, 5), d.coat, 1);
    let deep = d.coat.at(Tone::Shade);
    match coat {
        Coat::Coat | Coat::Overcoat | Coat::Jacket | Coat::Cardigan | Coat::Canvas if facing_us => {
            // Lapels to the fastening, the opening below it, a belt on a coat.
            c.line((CX - 4, t), (CX - 1, t + 4), deep, 1, relief::COAT.hi);
            c.line((CX + 3, t), (CX, t + 4), deep, 1, relief::COAT.hi);
            c.vline(CX, t + 5, hem - 1, deep, relief::COAT.hi);
            for k in 0..2 {
                c.dot(CX - 1, t + 5 + 2 * k, Ramp::Brass.at(Tone::Light), relief::COAT.hi + 1);
            }
            if coat == Coat::Coat {
                c.rect_round(Rect::new(w0, wy, w1 - w0 + 1, 2), Ramp::Leather, 1, 0, Z::flat(relief::COAT.hi + 1));
            }
        }
        Coat::Coat | Coat::Overcoat => {
            if coat == Coat::Coat {
                c.rect_round(Rect::new(w0, wy, w1 - w0 + 1, 2), Ramp::Leather, 1, 0, Z::flat(relief::COAT.hi + 1));
            }
            c.vline(CX - 1, wy + 2, hem - 1, deep, relief::COAT.hi);
        }
        Coat::Smock => {
            c.hline(s0 + 2, s1 - 2, t + 3, deep, relief::COAT.hi);
        }
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => {
            c.hline(w0, w1, wy, deep, relief::COAT.hi);
        }
        _ => {}
    }
}

/// What shows at the front of the coat, facing the viewer.
fn front_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    match d.look.body.front {
        Front::Scarf => {
            // A wrap round the neck and a tail over the chest, swinging a frame behind.
            c.rect_round(Rect::new(CX - 5, t - 2, 10, 4), f, 1, 2, Z::new(relief::FRONT, relief::FRONT + 1));
            let (sx, sy) = (i32::from(r.pose.phase >= 32768), -r.trail.1);
            c.polygon_lit(
                &[(CX + 1, t + 1), (CX + 3, t + 1), (CX + 3 + sx, t + 6 + sy), (CX + 1 + sx, t + 7 + sy)],
                f,
                60,
                Z::flat(relief::FRONT + 1),
            );
            c.hline(CX + 1 + sx, CX + 3 + sx, t + 6 + sy, f.at(Tone::Shade), relief::FRONT + 1);
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
                    Ramp::ClothRed,
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
    if d.look.body.coat == Coat::Apron && d.look.body.front != Front::Apron {
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

/// An arm from the front: a sleeve 4 px wide from the shoulder to the wrist, then a mitt of a
/// hand. `outer_left`: the arm on the screen's left.
fn arm_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, hand: i32, outer_left: bool) {
    let top = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe);
    let wrist = hand - 3;
    // The sleeve leans out from the shoulder: its top a pixel toward the body.
    let a = if outer_left { x0 + 1 } else { x0 - 1 };
    c.polygon_lit(&[(a, top), (a + 3, top), (x0 + 3, wrist), (x0, wrist)], d.coat, 80, relief::ARM);
    c.hline(x0, x0 + 3, wrist, d.coat.at(Tone::Shade), relief::ARM.hi);
    c.ellipse(Rect::new(x0, wrist + 1, 4, 4), d.skin.at(Tone::Base), relief::ARM.hi);
}

fn neck(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    let h = neck_w(r) / 2;
    c.polygon_lit(&[(CX - h, y), (CX + h - 1, y), (CX + h - 1, r.top), (CX - h, r.top)], d.skin, 60, Z::new(1, 2));
    // Under the chin.
    c.shade(Rect::new(CX - h - 1, y, 2 * h + 2, 5), d.skin, 2);
}

/// The eyes, each two px square with a glint in its top-left, the light's side.
fn eyes(c: &mut Canvas, d: &Dress, xs: &[i32], ey: i32, z: u8) {
    c.set_emitting(d.eye_emits);
    for &x in xs {
        c.fill_rect(Rect::new(x, ey, 2, 2), d.eye, z);
        if !d.eye_emits {
            c.dot(x, ey, palette::letter('w').unwrap_or(Ix::BEVEL_LIGHT), z);
        }
    }
    c.set_emitting(false);
}

fn face_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    // The fringe's shade across the brow.
    c.shade(Rect::new(r.skull.x + 1, r.brow_y() - 2, r.skull.w - 2, 4), d.skin, 1);
    eyes(c, d, &[CX - 5, CX + 3], ey, z);
    // A small mouth (skin's deep, which the skin's own tones make its mid).
    c.hline(CX - 1, CX, ey + 4, d.skin.at(Tone::Deep), z);
    match d.look.head.face {
        Face::Glasses => {
            for x in [CX - 6, CX + 2] {
                c.hline(x, x + 3, ey - 1, Ramp::Iron.at(Tone::Shade), z);
                c.vline(x, ey, ey + 1, Ramp::Glass.at(Tone::Light), z);
                c.vline(x + 3, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            }
        }
        Face::Beard => {
            let s = r.skull;
            c.polygon_lit(
                &[(CX - 5, ey + 3), (CX + 4, ey + 3), (CX + 3, s.bottom()), (CX - 4, s.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
            c.hline(CX - 1, CX, ey + 3, d.skin.at(Tone::Deep), z + 1);
            hair::tame(c, d.hair, Tone::Shade, Tone::Lift);
        }
        Face::Grim => {
            c.hline(CX - 2, CX + 1, ey + 4, d.skin.at(Tone::Deep), z);
            c.hline(CX - 6, CX - 3, ey - 1, d.hair.at(Tone::Deep), z);
            c.hline(CX + 2, CX + 5, ey - 1, d.hair.at(Tone::Deep), z);
        }
        Face::Plain | Face::None => {}
    }
}

/// The head: a lit skull and, under it, a flat jaw a little squarer than an egg (forward of the
/// skull's middle in profile), so a face sits on its neck and not on a stalk.
fn head(c: &mut Canvas, d: &Dress, r: &Rig, profile: bool) {
    let s = r.skull;
    c.ellipse_lit(s, d.skin, relief::SKULL);
    let ey = r.eye_y();
    let (x0, x1) = if profile { (s.x + 5, s.right() - 1) } else { (s.x + 2, s.right() - 3) };
    c.polyline_fill(
        &[(x0, ey), (x1, ey), (x1 - 2, s.bottom() - 1), (x0 + 2, s.bottom() - 1)],
        d.skin.at(Tone::Base),
        relief::SKULL.lo,
    );
}

pub(crate) fn hides_hair(d: &Dress) -> bool {
    matches!(d.look.head.hat, Hat::Scarf | Hat::Helmet | Hat::Diving | Hat::Veil)
}

/// The hat from the front (and, but for a scarf, from behind).
fn hat_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    let brow = r.brow_y();
    match d.look.head.hat {
        Hat::None => {}
        Hat::Cap => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, 10), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x, brow - 2, s.w, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
            c.shade(Rect::new(s.x + 1, brow - 1, s.w - 2, 3), d.skin, 1);
        }
        Hat::Peaked => {
            c.rect_round(Rect::new(s.x - 1, s.y - 3, s.w + 2, 6), h, 1, 2, z);
            c.rect_round(Rect::new(s.x, s.y + 2, s.w, 2), Ramp::Leather, 1, 1, Z::new(z.hi, z.hi + 1));
            c.disc_lit(CX - 1, s.y, 1, Ramp::Brass, Z::flat(z.hi + 2));
        }
        Hat::Brim | Hat::Panama => {
            c.ellipse_lit(Rect::new(s.x + 1, s.y - 5, s.w - 2, 8), h, z);
            if d.look.head.hat == Hat::Panama {
                c.hline(s.x + 1, s.right() - 2, s.y + 1, d.coat.at(Tone::Shade), z.hi);
            } else {
                c.hline(s.x + 1, s.right() - 2, s.y + 1, h.at(Tone::Shade), z.hi);
            }
            c.ellipse_lit(Rect::new(s.x - 4, s.y + 1, s.w + 8, 4), h, Z::new(z.hi, z.hi + 1));
            c.shade(Rect::new(s.x, s.y + 3, s.w, 4), d.skin, 1);
        }
        Hat::Cloche => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow + 1)));
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, 12), h, z);
            c.set_clip(None);
            c.hline(s.x - 1, s.right(), brow, h.at(Tone::Shade), z.hi);
        }
        Hat::Helmet => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x, s.y - 7, s.w, 14), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x - 1, brow - 2, s.w + 2, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
            c.disc_lit(CX - 1, s.y - 1, 1, Ramp::Brass, Z::flat(z.hi + 2));
            c.shade(Rect::new(s.x + 1, brow - 1, s.w - 2, 3), d.skin, 1);
        }
        Hat::Scarf => {
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h), h, z);
            // The face shows through the scarf's opening.
            c.set_clip(Some(Rect::new(s.x + 2, brow, s.w - 4, s.bottom() - brow - 1)));
            c.ellipse_lit(r.skull, d.skin, relief::SKULL);
            c.set_clip(None);
            face_down(c, d, r);
            c.ellipse_lit(Rect::new(CX - 2, s.bottom() - 2, 4, 3), h, Z::new(z.hi, z.hi + 1));
        }
        Hat::Veil | Hat::Diving => {
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, s.h + 2), h, z);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// From behind

fn up(c: &mut Canvas, d: &Dress, r: &Rig) {
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, foot);
    }
    coat_front(c, d, r, false);
    if d.look.body.pack {
        let t = r.top;
        c.rect_round(Rect::new(CX - 5, t + 2, 10, 8), d.pack, 1, 2, relief::PACK);
        c.rect_round(Rect::new(CX - 5, t + 2, 10, 3), d.pack, 1, 1, Z::new(relief::PACK.hi, relief::PACK.hi + 1));
        c.dot(CX - 1, t + 5, Ramp::Brass.at(Tone::Light), relief::PACK.hi + 1);
    }
    arms_front(c, d, r);
    neck(c, d, r);
    hair::whole_up(c, d, r);
    match d.look.head.hat {
        Hat::Scarf => {
            let s = r.skull;
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h + 1), d.hat, relief::HAT);
        }
        Hat::Helmet | Hat::Peaked => {
            // From behind: no badge, no peak.
            let s = r.skull;
            let brow = r.brow_y();
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(
                Rect::new(
                    s.x - i32::from(d.look.head.hat == Hat::Peaked),
                    s.y - 7,
                    s.w + 2 * i32::from(d.look.head.hat == Hat::Peaked),
                    14,
                ),
                d.hat,
                relief::HAT,
            );
            c.set_clip(None);
            c.rect_round(
                Rect::new(s.x - 1, brow - 2, s.w + 2, 2),
                d.hat,
                1,
                1,
                Z::new(relief::HAT.hi, relief::HAT.hi + 1),
            );
        }
        _ => hat_down(c, d, r),
    }
}

// ---------------------------------------------------------------------------------------------
// From the side, facing east

fn side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let t = r.top;
    let shoulder = (CX - 1 + r.lean, r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe));
    // The far arm and leg, behind everything.
    arm_side(c, d, r, shoulder, r.pose.arm[1], relief::FAR);
    leg_side(c, d, r, r.pose.leg[1], r.pose.lift[1], true);
    hair::back_side(c, d, r);
    if d.look.body.pack {
        c.rect_round(Rect::new(CX - 11 + r.lean, t + 1, 6, 8), d.pack, 1, 2, relief::FAR);
    }
    leg_side(c, d, r, r.pose.leg[0], r.pose.lift[0], false);
    coat_side(c, d, r);
    front_side(c, d, r);
    if d.look.body.pack {
        c.line((CX + 1 + r.lean, t), (CX - 3, r.waist), d.pack.at(Tone::Base), 1, relief::FRONT);
    }
    neck_side(c, d, r);
    head(c, d, r, true);
    face_side(c, d, r);
    hair::side(c, d, r);
    hat_side(c, d, r);
    arm_side(c, d, r, shoulder, r.pose.arm[0], relief::ARM);
}

/// A leg seen from the side, hip to foot, the boot pointing east.
fn leg_side(c: &mut Canvas, d: &Dress, r: &Rig, swing: i32, lift: i32, far: bool) {
    let legs = leg_ramp(d);
    let hx = CX - 2;
    let foot = AY - lift;
    let fx = hx + swing;
    let rows = boot_rows(d);
    let top = r.hip - 1;
    let z = if far { relief::FAR } else { relief::LEG };
    c.polygon_lit(&[(hx, top), (hx + 3, top), (fx + 3, foot - rows), (fx, foot - rows)], legs, 70, z);
    let boot = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(fx, foot - rows + 1, 5, rows), boot, 1, 1, if far { relief::FAR } else { relief::BOOT });
    if far {
        // The far leg stands in the near one's shade.
        c.shade(Rect::new(fx - 1, top, 7, foot - top + 1), legs, 1);
        c.shade(Rect::new(fx - 1, foot - rows, 7, rows + 2), boot, 1);
    }
}

fn coat_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let w = side_w(r);
    let x0 = CX - w / 2 - 1;
    let x1 = x0 + w - 1;
    let fl = flare(d.look.body.coat);
    let (t, wy, hem, l) = (r.top, r.waist, r.hem, r.lean);
    // The hem swings behind the walker a frame late.
    let tr = r.trail.0.min(0);
    skirt(c, d, r, x0, x1);
    let pts = [
        (x0 + 2 + l, t),
        (x1 - 2 + l, t),
        (x1 + l, t + 2),
        (x1, wy),
        (x1 + fl + tr, hem),
        (x0 - fl + tr, hem),
        (x0, wy),
        (x0 + l, t + 2),
    ];
    c.polygon_lit(&pts, d.coat, 80, relief::COAT);
    fold_lines(c, d.coat, x0 - fl + tr, x1 + fl + tr, hem, (hem - wy - 1).min(4), r.pose.phase);
    c.shade(Rect::new(CX - 3 + l, t - 2, 8, 5), d.coat, 1);
    if matches!(d.look.body.coat, Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress) {
        c.hline(x0, x1, wy, d.coat.at(Tone::Shade), relief::COAT.hi);
    }
    if d.look.body.coat == Coat::Coat {
        c.rect_round(Rect::new(x0, wy, w, 2), Ramp::Leather, 1, 0, Z::flat(relief::COAT.hi + 1));
    }
}

fn front_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    let w = side_w(r);
    let x1 = CX - w / 2 - 1 + w - 1 + r.lean;
    match d.look.body.front {
        Front::Scarf => {
            c.rect_round(Rect::new(CX - 3 + r.lean, t - 2, 8, 4), f, 1, 2, Z::new(relief::FRONT, relief::FRONT + 1));
            // The tail streams back a frame behind the lean.
            let sx = r.trail.0.min(0) - i32::from(r.pose.lean > 0);
            let sy = -r.trail.1;
            c.polygon_lit(
                &[(x1 - 1, t + 1), (x1 + 1, t + 1), (x1 + 1 + sx, t + 6 + sy), (x1 - 1 + sx, t + 6 + sy)],
                f,
                60,
                Z::flat(relief::FRONT + 1),
            );
        }
        Front::Shirt | Front::Tie | Front::Waistcoat => {
            c.polygon_lit(&[(x1 - 2, t), (x1, t), (x1, t + 4), (x1 - 1, t + 4)], f, 40, Z::flat(relief::FRONT));
        }
        Front::Braces => c.vline(CX + r.lean, t, r.hip - 1, f.at(Tone::Base), relief::FRONT),
        Front::Apron => apron_side(c, f, r, x1),
        Front::None => {}
    }
    if d.look.body.coat == Coat::Apron && d.look.body.front != Front::Apron {
        apron_side(c, f, r, x1);
    }
}

fn apron_side(c: &mut Canvas, f: Ramp, r: &Rig, x1: i32) {
    c.polygon_lit(
        &[(x1 - 1, r.top + 1), (x1, r.top + 1), (x1 + 1 - r.lean, r.hem - 1), (x1 - 2 - r.lean, r.hem - 1)],
        f,
        30,
        Z::flat(relief::FRONT),
    );
    c.hline(CX - 3, x1 - r.lean, r.waist, f.at(Tone::Shade), relief::FRONT + 1);
}

/// An arm from the side: shoulder to hand, the hand `swing` px along the facing.
fn arm_side(c: &mut Canvas, d: &Dress, r: &Rig, shoulder: (i32, i32), swing: i32, z: Z) {
    let (sx, sy) = shoulder;
    let hand = (sx + swing, sy + r.p.arm_l - 4);
    c.polygon_lit(&[(sx - 1, sy), (sx + 2, sy), (hand.0 + 2, hand.1), (hand.0 - 1, hand.1)], d.coat, 80, z);
    c.hline(hand.0 - 1, hand.0 + 2, hand.1, d.coat.at(Tone::Shade), z.hi);
    c.ellipse(Rect::new(hand.0 - 1, hand.1, 4, 4), d.skin.at(Tone::Base), z.hi);
    if z == relief::FAR {
        c.shade(Rect::new(hand.0 - 3, sy, 8, hand.1 - sy + 4), d.coat, 1);
    }
}

fn neck_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    let l = r.lean;
    let w = neck_w(r);
    c.polygon_lit(
        &[(CX - 1 + l, y), (CX + w - 2 + l, y), (CX + w - 2 + l, r.top), (CX - 1 + l, r.top)],
        d.skin,
        60,
        Z::new(1, 2),
    );
    c.shade(Rect::new(CX - 2 + l, y, 6, 5), d.skin, 2);
}

fn face_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    let fx = s.right() - 4;
    c.shade(Rect::new(s.x + 3, r.brow_y() - 2, s.w - 3, 4), d.skin, 1);
    eyes(c, d, &[fx], ey, z);
    // The nose, two px past the skull; a cheek; the mouth's corner.
    c.vline(s.right(), ey + 2, ey + 3, d.skin.at(Tone::Base), z);
    c.hline(fx - 2, fx - 1, ey + 2, d.skin.at(Tone::Mid), z);
    c.dot(s.right() - 2, ey + 4, d.skin.at(Tone::Deep), z);
    match d.look.head.face {
        Face::Glasses => {
            c.hline(fx - 1, fx + 2, ey - 1, Ramp::Iron.at(Tone::Shade), z);
            c.vline(fx + 2, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            c.hline(s.x + 6, fx - 2, ey, Ramp::Iron.at(Tone::Shade), z);
        }
        Face::Beard => {
            c.polygon_lit(
                &[(fx - 3, ey + 3), (s.right() - 1, ey + 3), (s.right() - 2, s.bottom()), (fx - 3, s.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
            hair::tame(c, d.hair, Tone::Shade, Tone::Lift);
        }
        Face::Grim => c.hline(fx - 1, fx + 1, ey - 1, d.hair.at(Tone::Deep), z),
        Face::Plain | Face::None => {}
    }
}

fn hat_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let h = d.hat;
    let z = relief::HAT;
    let brow = r.brow_y();
    match d.look.head.hat {
        Hat::None => {}
        Hat::Cap => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, 10), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x + 6, brow - 2, s.w - 3, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
            c.shade(Rect::new(s.x + 5, brow - 1, s.w - 4, 3), d.skin, 1);
        }
        Hat::Peaked => {
            c.rect_round(Rect::new(s.x - 1, s.y - 3, s.w + 1, 6), h, 1, 2, z);
            c.rect_round(Rect::new(s.x + 8, s.y + 2, s.w - 4, 2), Ramp::Leather, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Brim | Hat::Panama => {
            c.ellipse_lit(Rect::new(s.x + 1, s.y - 5, s.w - 2, 8), h, z);
            let band = if d.look.head.hat == Hat::Panama { d.coat } else { h };
            c.hline(s.x + 1, s.right() - 2, s.y + 1, band.at(Tone::Shade), z.hi);
            c.ellipse_lit(Rect::new(s.x - 3, s.y + 1, s.w + 6, 3), h, Z::new(z.hi, z.hi + 1));
            c.shade(Rect::new(s.x + 3, s.y + 3, s.w - 2, 4), d.skin, 1);
        }
        Hat::Cloche => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow + 1)));
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, 12), h, z);
            c.set_clip(None);
        }
        Hat::Helmet => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x, s.y - 7, s.w, 14), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x - 1, brow - 2, s.w + 3, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
        }
        Hat::Scarf => {
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 1, s.h), h, z);
            c.set_clip(Some(Rect::new(s.right() - 6, brow, 7, s.bottom() - brow - 1)));
            c.ellipse_lit(r.skull, d.skin, relief::SKULL);
            c.set_clip(None);
            face_side(c, d, r);
        }
        Hat::Veil | Hat::Diving => c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, s.h + 2), h, z),
    }
}
