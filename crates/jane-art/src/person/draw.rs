//! The composer's parts (ART.md §2.1): legs, boots, coat, front, arms, skin, face, hair, hat,
//! pack, in that order, for each facing, all read from one [`Rig`] so the frame table moves
//! everything together.
//!
//! Cloth is `polygon_cloth`: large calm areas of its base, a lit edge on the light's side and a
//! clear shadow side, and then the painter's deliberate clusters: folds under the arms, at the
//! waist and at the hem, lapels, collars, belts and buckles as shapes with their own light and
//! shade. Faces are painted in four skin tones: the fringe's cast shadow on the brow, a cheek
//! and jaw in shade on the side away from the light, a nose and a mouth, eyes with a lid and a
//! glint. Builds and cuts change the silhouette: square or sloping shoulders, a belly, a
//! fitted bodice over a flared skirt, a stoop.
//!
//! Every part writes its normals and relief as it goes (relief: what stands in front of what,
//! which the outline reads for seams); [`finish`] cleans the clusters, lays the contact shadow,
//! runs the selective outline and stands the frame up ([`Canvas::upright`]), which writes the
//! true height of every pixel.

use jane_core::grid::Rect;
use jane_data::{Boots, Build, Coat, Extra, Face, Front, Hat, Legs};

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

/// What the lit primitives' bands become in cloth, leather and hats: a shade, a base, a lift
/// and a light, so what a curve makes falls in clusters.
const CLOTH: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Shade, Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::Light];
/// Skin's four: shade, mid, base, lift.
const SKIN: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Lift, Tone::Lift];

/// A symmetric span of width `w` about the centre line: `(first, last)` column.
const fn span(w: i32) -> (i32, i32) {
    (CX - w / 2, CX - w / 2 + w - 1)
}

/// Where everything is in one frame: the build moved by the pose.
pub(crate) struct Rig {
    pub p: Proportions,
    pub pose: Pose,
    pub build: Build,
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
    /// A skeleton: the bone skin, drawn as bones (ART.md §2.1).
    pub bone: bool,
    /// Which way the frame faces.
    pub facing: Facing,
}

/// Whether `d` is a skeleton: the `bone` skin draws a skull, ribs and bones, not a face and
/// limbs.
pub(crate) fn bony(d: &Dress) -> bool {
    d.skin == Ramp::Bone
}

impl Rig {
    fn new(p: Proportions, pose: Pose, d: &Dress, facing: Facing) -> Rig {
        let coat = d.look.body.coat;
        let stoop = d.look.extras.contains(&Extra::Stoop);
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
        Rig { p, pose, build: d.look.build, skull, top, waist: (top + hip) / 2 + 1, hip, hem, lean, trail, bone: bony(d), facing }
    }

    /// The eyes' first row (their lids are the row above).
    pub fn eye_y(&self) -> i32 {
        self.skull.y + self.skull.h / 2
    }

    /// The fringe's row: where hair meets forehead.
    pub fn brow_y(&self) -> i32 {
        self.eye_y() - 2
    }

    /// Square shoulders (broad) or sloping ones.
    fn square(&self) -> bool {
        self.build == Build::Broad
    }

    /// A sleeve's width.
    fn arm_w(&self) -> i32 {
        if self.build == Build::Broad { 5 } else { 4 }
    }

    /// The neck's width: a pencil on a girl, a post on a smith.
    fn neck_w(&self) -> i32 {
        match self.build {
            _ if self.bone => 2,
            Build::Broad | Build::Stout => 6,
            Build::Slim | Build::Child => 4,
        }
    }

    /// The body's depth seen from the side: narrower than its breadth.
    pub(crate) fn side_w(&self) -> i32 {
        (self.p.shoulder_w - 2).max(8)
    }
}

/// One living frame of a person, finished.
pub fn frame(d: &Dress, p: Proportions, facing: Facing, pose: Pose) -> Canvas {
    let mut c = Canvas::new(super::W, super::H);
    let r = Rig::new(p, pose, d, facing);
    match facing {
        Facing::Down => down(&mut c, d, &r),
        Facing::Up => up(&mut c, d, &r),
        Facing::Side => side(&mut c, d, &r),
    }
    super::special::extras(&mut c, d, &r);
    super::special::wet(&mut c, d, &r);
    super::held::at_face(&mut c, d, &r);
    finish(&mut c, d, &r);
    c
}

/// Every material to its own tones, skin never in a checker, the clusters cleaned, the contact
/// shadow under the feet, the selective outline (seams by relief), then each pixel's true height.
fn finish(c: &mut Canvas, d: &Dress, r: &Rig) {
    // A held lantern's light is light, not paint: it keeps its colour through the outline.
    let glow: Vec<(i32, i32, Ix)> = if d.look.held == jane_data::HeldItem::Lantern {
        (0..c.h())
            .flat_map(|y| (0..c.w()).map(move |x| (x, y)))
            .filter_map(|(x, y)| {
                let e = c.emissive_at(x, y);
                (e != Ix::CLEAR).then_some((x, y, e))
            })
            .collect()
    } else {
        Vec::new()
    };
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack] {
        c.retone(m, CLOTH);
    }
    c.retone(d.skin, SKIN);
    c.unchecker(d.skin);
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, d.skin, d.hair] {
        c.declutter(m);
    }
    super::special::material(c, d, r);
    c.despike();
    c.ao_contact(Rect::new(CX - 7, AY - 1, 14, 4), 0);
    c.outline();
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, d.skin, d.hair] {
        c.declutter(m);
    }
    c.unchecker(d.skin);
    c.relight(&glow);
    c.upright(AY);
    if d.look.ghost {
        super::special::ghost(c, r);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared painting

/// Hem folds: `n` creases rising `len` rows from the hem across `x0..=x1`, each a line of the
/// cloth's shade with its lit ridge a px to the left; the set sways a px with the walk.
fn hem_folds(c: &mut Canvas, ramp: Ramp, x0: i32, x1: i32, hem: i32, len: i32, phase: u16) {
    let sway = i32::from(phase >= 32768);
    let w = x1 - x0 + 1;
    let n = if w >= 16 { 3 } else { 2 };
    for k in 0..n {
        let x = x0 + (k + 1) * w / (n + 1) + sway;
        let l = if k == n / 2 { len } else { len - 1 };
        for y in hem - l..hem {
            c.tint(x, y, ramp, Tone::Shade);
            if y > hem - l {
                c.tint(x - 1, y, ramp, Tone::Lift);
            }
        }
    }
}

/// A belt across `x0..=x1` on row `y`: two rows of leather lit on top, and a brass buckle.
fn belt(c: &mut Canvas, x0: i32, x1: i32, y: i32, buckle: i32, z: u8) {
    c.fill_rect(Rect::new(x0, y, x1 - x0 + 1, 1), Ramp::Leather.at(Tone::Light), z);
    c.fill_rect(Rect::new(x0, y + 1, x1 - x0 + 1, 1), Ramp::Leather.at(Tone::Base), z);
    c.fill_rect(Rect::new(buckle, y, 2, 2), Ramp::Brass.at(Tone::Base), z + 1);
    c.dot(buckle, y, Ramp::Brass.at(Tone::High), z + 1);
}

/// A skin mitt of a hand, 4 x 3 from `(x, y)`, with its shadow px at the bottom right.
fn hand(c: &mut Canvas, d: &Dress, x: i32, y: i32, z: u8) {
    if bony(d) {
        super::bone::hand(c, d, x, y, z);
        return;
    }
    c.ellipse(Rect::new(x, y, 4, 3), d.skin.at(Tone::Base), z);
    c.dot(x + 1, y, d.skin.at(Tone::Lift), z);
    c.dot(x + 2, y + 1, d.skin.at(Tone::Mid), z);
    super::special::knuckles(c, d, x, y, z);
}

/// The legs' ramp: bare legs are skin; under a skirt, stockings in the legs' ramp.
fn leg_ramp(d: &Dress) -> Ramp {
    match d.look.body.legs {
        Legs::Bare => d.skin,
        Legs::Skirt | Legs::Trousers | Legs::Pyjamas => d.legs,
    }
}

fn boot_rows(d: &Dress) -> i32 {
    match d.look.body.boots {
        Boots::Boots => 3,
        Boots::Shoes | Boots::Bare => 2,
    }
}

/// How far a cut flares at the hem each side, px: a dress's skirt is a trapezoid.
fn flare(coat: Coat) -> i32 {
    match coat {
        Coat::Dress | Coat::Apron => 3,
        Coat::Gown | Coat::Nightdress => 2,
        Coat::Overcoat | Coat::Coat | Coat::Smock => 1,
        _ => 0,
    }
}

/// A dress, an apron's dress or a gown: a fitted bodice over a skirt.
fn skirted(coat: Coat) -> bool {
    matches!(coat, Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress)
}

/// The eyes: each two px square and dark, a lid line three px wide over it, and a glint in its
/// top corner on the light's side.
fn eyes(c: &mut Canvas, d: &Dress, shut: bool, xs: &[(i32, bool)], ey: i32, z: u8) {
    let iris = if d.eye_emits { d.eye } else { Ramp::Leather.at(Tone::Deep) };
    for &(x, outward_left) in xs {
        let lid = if outward_left { x - 1 } else { x };
        if shut {
            // Screwed shut: the lid pressed down to a line where the eye was.
            c.hline(lid, lid + 2, ey + 1, Ix::SEAM, z);
            continue;
        }
        c.hline(lid, lid + 2, ey - 1, Ix::SEAM, z);
        if d.eye_emits {
            // The dead's eyes: a dark eye and one pinpoint of light in it, toward the nose.
            c.fill_rect(Rect::new(x, ey, 2, 2), Ramp::Leather.at(Tone::Deep), z);
            c.set_emitting(true);
            c.dot(if outward_left { x + 1 } else { x }, ey, iris, z);
            c.set_emitting(false);
        } else {
            c.fill_rect(Rect::new(x, ey, 2, 2), iris, z);
            c.dot(x, ey, palette::letter('w').unwrap_or(Ix::BEVEL_LIGHT), z);
        }
    }
}

/// The face's four skin tones over the skull box `s`, facing the viewer: base, the fringe's
/// shadow on the brow, the cheek and jaw on the far side in shade, a lift on the near cheek.
fn face_tones(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let (ey, brow) = (r.eye_y(), r.brow_y());
    let skin = d.skin;
    for y in s.y..s.bottom() {
        for x in s.x..s.right() {
            let shade = y == brow + 1
                || (y == brow + 2 && x >= CX + 2)
                || (x >= s.right() - 3 && y >= ey - 1)
                || (y >= s.bottom() - 2 && x >= CX);
            let t = if shade {
                Tone::Mid
            } else if (x == s.x + 2 || x == s.x + 3) && (y == ey + 2 || y == ey + 3) && !(x == s.x + 3 && y == ey + 3) {
                Tone::Lift
            } else {
                Tone::Base
            };
            c.tint(x, y, skin, t);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Facing the viewer

fn down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let bone = bony(d);
    if !bone {
        hair::back_down(c, d, r);
    }
    legs_front(c, d, r);
    coat_front(c, d, r, true);
    if bone {
        super::bone::rags(c, d, r, true);
    }
    front_down(c, d, r);
    arms_front(c, d, r);
    neck(c, d, r);
    head(c, d, r, false);
    if bone {
        super::bone::face_down(c, d, r);
        // A skeleton keeps its hat: a soldier's helmet on a skull.
        hat_down(c, d, r);
        return;
    }
    face_down(c, d, r);
    hair::front_down(c, d, r);
    hat_down(c, d, r);
    if d.look.extras.contains(&Extra::Shawl) {
        shawl(c, d, r, true);
    }
}

fn legs_front(c: &mut Canvas, d: &Dress, r: &Rig) {
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let out = if i == 0 { -r.pose.splay[i] } else { r.pose.splay[i] };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, out, foot);
    }
}

/// A leg and its boot seen from the front or behind, 4 px wide from `x0` at the hip, its foot
/// `out` px further out, the sole on `foot`.
fn leg_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, out: i32, foot: i32) {
    let boot_h = boot_rows(d);
    let legs = leg_ramp(d);
    let top = r.hip - 1;
    let bot = foot - boot_h;
    if bony(d) && d.look.body.legs == Legs::Bare {
        // A shin bone, two px, with the knee's knob.
        super::bone::shin(c, d, x0 + 1, top, x0 + 1 + out, bot, relief::LEG);
    } else if bot >= top {
        c.polygon_cloth(&[(x0, top), (x0 + 3, top), (x0 + 3 + out, bot), (x0 + out, bot)], legs, 70, relief::LEG);
    }
    if d.look.body.legs == Legs::Pyjamas {
        c.folds(Rect::new(x0 + out.min(0), top, 4 + out.abs(), bot - top + 1), legs, 2, 0);
    }
    let ramp = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(x0 + out, foot - boot_h + 1, 4, boot_h), ramp, 1, 1, relief::BOOT);
}

/// A skirt below a coat too short to cover the knee: a trapezoid from the hip.
fn skirt(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, x1: i32) {
    if d.look.body.legs == Legs::Skirt && r.hem < r.hip + 3 {
        let b = r.hip + 3 + r.trail.1;
        c.polygon_cloth(&[(x0, r.hip - 2), (x1, r.hip - 2), (x1 + 2, b), (x0 - 2, b)], d.legs, 80, relief::LEG);
        hem_folds(c, d.legs, x0 - 2, x1 + 2, b, 3, r.pose.phase);
    }
}

/// The coat from the front (or behind): the build's shoulders, the cut's waist and hem, calm
/// cloth, and the cut's details facing the viewer.
fn coat_front(c: &mut Canvas, d: &Dress, r: &Rig, facing_us: bool) {
    let coat = d.look.body.coat;
    let (s0, s1) = span(r.p.shoulder_w);
    let (t, wy) = (r.top, r.waist);
    let spread = i32::from(r.pose.spread != [0, 0]);
    let fl = flare(coat) + spread;
    let hem = r.hem;
    let (h0, h1) = span(r.p.hip_w + 2 * fl);
    let (k0, k1) = span(r.p.hip_w);
    skirt(c, d, r, k0, k1);
    // A fitted bodice is narrower at the waist than the build.
    let waist_w = if skirted(coat) { r.p.waist_w - 2 } else { r.p.waist_w };
    let (w0, w1) = span(waist_w);
    let mut pts: Vec<(i32, i32)> = Vec::with_capacity(12);
    if r.square() {
        pts.extend([(s0, t), (s1, t)]);
    } else {
        pts.extend([(s0 + 2, t), (s1 - 2, t), (s1, t + 2)]);
    }
    pts.extend([(w1, wy), (h1, hem), (h0, hem), (w0, wy)]);
    if !r.square() {
        pts.push((s0, t + 2));
    }
    c.polygon_cloth(&pts, d.coat, 80, relief::COAT);
    let z = relief::COAT.hi;
    let (shade, lift, light) = (d.coat.at(Tone::Shade), d.coat.at(Tone::Lift), d.coat.at(Tone::Light));
    // Folds under the arms: where the sleeve meets the body.
    for y in t + 2..t + 5 {
        c.tint(s0 + 1, y, d.coat, Tone::Shade);
        c.tint(s1 - 1, y, d.coat, Tone::Shade);
    }
    // The head's shade on the shoulders under it.
    c.shade(Rect::new(CX - 4, t - 2, 8, 4), d.coat, 1);
    if hem - wy > 3 {
        hem_folds(c, d.coat, h0 + 1, h1 - 1, hem, (hem - wy - 2).min(5), r.pose.phase);
    }
    if !facing_us {
        if matches!(coat, Coat::Coat | Coat::Canvas) {
            belt(c, w0, w1, wy, CX - 1, z + 1);
        }
        // The back seam, and a vent in a long coat.
        if matches!(coat, Coat::Overcoat | Coat::Coat) {
            c.vline(CX - 1, wy + 2, hem - 1, shade, z);
        }
        return;
    }
    match coat {
        Coat::Coat | Coat::Overcoat | Coat::Jacket | Coat::Canvas => {
            // Lapels: the near one lit, the far one in shade; the opening under them.
            let deep = if coat == Coat::Overcoat { 6 } else { 4 };
            c.polyline_fill(&[(CX - 4, t), (CX - 1, t), (CX - 1, t + deep)], lift, z + 1);
            c.polyline_fill(&[(CX, t), (CX + 3, t), (CX, t + deep)], shade, z + 1);
            c.vline(CX, t + deep, hem - 1, shade, z);
            c.vline(CX - 1, t + deep + 1, hem - 1, light, z);
            let buttons: &[i32] = if coat == Coat::Overcoat { &[CX - 3, CX + 2] } else { &[CX - 2] };
            for &bx in buttons {
                for k in 0..2 {
                    c.dot(bx, t + deep + 1 + 3 * k, Ramp::Brass.at(Tone::Light), z + 2);
                }
            }
            if coat == Coat::Coat {
                belt(c, w0, w1, wy, CX - 1, z + 2);
            }
            if coat == Coat::Jacket {
                // Pocket flaps.
                for x in [w0 + 1, CX + 2] {
                    c.hline(x, x + 2, r.hip - 2, shade, z + 1);
                }
            }
        }
        Coat::Cardigan => {
            // Open down the front, the front showing; a line of buttons on the near edge.
            c.polyline_fill(
                &[(CX - 2, t), (CX + 1, t), (CX + 1, hem - 1), (CX - 2, hem - 1)],
                d.front.at(Tone::Base),
                z + 1,
            );
            c.vline(CX + 2, t, hem - 1, shade, z + 1);
            for k in 0..3 {
                c.dot(CX - 3, t + 2 + 2 * k, Ramp::Brass.at(Tone::Light), z + 2);
            }
        }
        Coat::Smock => {
            // A yoke across the chest, gathered under it.
            c.hline(s0 + 2, s1 - 2, t + 3, shade, z);
            c.hline(s0 + 2, s1 - 2, t + 2, lift, z);
            for x in [CX - 3, CX, CX + 3] {
                c.vline(x, t + 4, t + 5, shade, z);
            }
        }
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => {
            // A round neck; the waist seam, lit on its upper lip.
            c.hline(w0, w1, wy, shade, z);
            c.hline(w0, CX - 1, wy - 1, lift, z);
            c.hline(CX - 2, CX + 1, t, shade, z);
        }
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
            c.hline(CX - 4, CX + 3, t, f.at(Tone::Shade), relief::FRONT + 1);
            let (sx, sy) = (i32::from(r.pose.phase >= 32768), -r.trail.1);
            c.polygon_cloth(
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
            c.dot(CX, t, f.at(Tone::Shade), relief::FRONT);
            if d.look.body.front == Front::Tie {
                c.polygon_cloth(
                    &[(CX - 1, t), (CX, t), (CX, t + 5), (CX - 1, t + 5)],
                    Ramp::ClothRed,
                    40,
                    Z::flat(relief::FRONT + 1),
                );
            }
        }
        Front::Waistcoat => {
            // A vest: two panels to a point below the waist, a V at the neck.
            c.polygon_cloth(
                &[
                    (CX - 4, t),
                    (CX - 1, t + 3),
                    (CX, t + 3),
                    (CX + 3, t),
                    (CX + 3, r.waist + 1),
                    (CX, r.waist + 3),
                    (CX - 1, r.waist + 3),
                    (CX - 4, r.waist + 1),
                ],
                f,
                60,
                Z::flat(relief::FRONT),
            );
            for k in 0..3 {
                c.dot(CX - 1, t + 4 + 2 * k, Ramp::Brass.at(Tone::Light), relief::FRONT + 1);
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

/// An apron: a bib on the chest, a panel from the waist that widens to its hem, the ties at the
/// waist and the neck strap.
fn apron_down(c: &mut Canvas, f: Ramp, r: &Rig) {
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    let z = relief::FRONT;
    c.polygon_cloth(&[(CX - 3, t + 2), (CX + 2, t + 2), (CX + 2, wy), (CX - 3, wy)], f, 50, Z::flat(z));
    c.polygon_cloth(&[(CX - 4, wy), (CX + 3, wy), (CX + 5, hem - 1), (CX - 6, hem - 1)], f, 50, Z::flat(z));
    c.hline(CX - 5, CX + 4, wy, f.at(Tone::Shade), z + 1);
    for x in [CX - 3, CX + 2] {
        c.vline(x, t, t + 1, f.at(Tone::Shade), z);
    }
    c.vline(CX, wy + 2, hem - 2, f.at(Tone::Shade), z + 1);
}

/// The shawl's wool: the hat's ramp when she wears no hat but names one, else the front's.
fn shawl_ramp(d: &Dress) -> Ramp {
    if d.look.head.hat == Hat::None && d.look.head.hat_ramp.is_some() { d.hat } else { d.front }
}

/// A shawl over the shoulders, down to a point at the back or over the chest.
fn shawl(c: &mut Canvas, d: &Dress, r: &Rig, facing_us: bool) {
    let (s0, s1) = span(r.p.shoulder_w + 6);
    let t = r.top - 1;
    let point = if facing_us { r.waist } else { r.waist + 2 };
    let w = shawl_ramp(d);
    c.polygon_cloth(
        &[
            (s0 + 2, t),
            (s1 - 2, t),
            (s1, t + 3),
            (s1 - 1, t + 5),
            (CX, point),
            (CX - 1, point),
            (s0 + 1, t + 5),
            (s0, t + 3),
        ],
        w,
        90,
        Z::flat(relief::FRONT + 2),
    );
    for x in [CX - 4, CX + 3] {
        c.vline(x, t + 3, point - 3, w.at(Tone::Shade), relief::FRONT + 2);
    }
}

fn arms_front(c: &mut Canvas, d: &Dress, r: &Rig) {
    let (s0, s1) = span(r.p.shoulder_w);
    let aw = r.arm_w();
    for i in 0..2 {
        let spread = r.pose.spread[i];
        let hand_y = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + r.pose.arm[i] - spread / 2 - r.pose.raise[i];
        let (x0, out) = if i == 0 { (s0 - aw + 1, -spread) } else { (s1, spread) };
        arm_front(c, d, r, x0, out, hand_y, i == 0);
    }
}

/// An arm from the front: a sleeve `arm_w` px wide from the shoulder to the wrist, lit on its
/// light side, a cuff, then a mitt of a hand. `out` swings the hand outward; `outer_left`: the
/// arm on the screen's left.
fn arm_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, out: i32, hand_y: i32, outer_left: bool) {
    let aw = r.arm_w();
    let top = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe);
    let wrist = hand_y - 3;
    // The sleeve leans out from the shoulder: its top a pixel toward the body.
    let a = if outer_left { x0 + 1 } else { x0 - 1 };
    let (bx, z) = (x0 + out, relief::ARM);
    c.polygon_cloth(&[(a, top), (a + aw - 1, top), (bx + aw - 1, wrist), (bx, wrist)], d.coat, 80, z);
    // The cuff, turned back and catching the light.
    c.hline(bx, bx + aw - 1, wrist, d.coat.at(Tone::Light), z.hi);
    c.dot(bx + aw - 1, wrist, d.coat.at(Tone::Shade), z.hi);
    hand(c, d, bx + (aw - 4) / 2, wrist + 1, z.hi);
    if outer_left {
        super::held::draw(c, d, r, bx + (aw - 4) / 2, wrist + 1);
    }
}

fn neck(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    let h = r.neck_w() / 2;
    c.polygon_cloth(&[(CX - h, y), (CX + h - 1, y), (CX + h - 1, r.top), (CX - h, r.top)], d.skin, 60, Z::new(1, 2));
    // Under the chin.
    c.shade(Rect::new(CX - h - 1, y, 2 * h + 2, 5), d.skin, 2);
}

fn face_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    face_tones(c, d, r);
    eyes(c, d, r.pose.shut, &[(CX - 5, true), (CX + 3, false)], ey, z);
    // The nose: its shadow on the far side; the mouth under it.
    c.vline(CX, ey + 2, ey + 3, d.skin.at(Tone::Mid), z);
    c.dot(CX - 1, ey + 3, d.skin.at(Tone::Lift), z);
    c.hline(CX - 1, CX, ey + 5, d.skin.at(Tone::Shade), z);
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
            c.polygon_cloth(
                &[(CX - 5, ey + 3), (CX + 4, ey + 3), (CX + 3, s.bottom()), (CX - 4, s.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
            c.hline(CX - 1, CX, ey + 4, d.skin.at(Tone::Shade), z + 1);
            hair::tame(c, d.hair, Tone::Shade, Tone::Lift);
        }
        Face::Grim => {
            c.hline(CX - 2, CX + 1, ey + 5, d.skin.at(Tone::Shade), z);
            c.hline(CX - 6, CX - 3, ey - 1, d.hair.at(Tone::Deep), z);
            c.hline(CX + 2, CX + 5, ey - 1, d.hair.at(Tone::Deep), z);
        }
        Face::Plain | Face::None => {}
    }
}

/// The head: a lit skull and, under it, a jaw a little squarer than an egg (in profile forward
/// of the skull's middle, the back of the skull rounding over the nape).
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
            let band = if d.look.head.hat == Hat::Panama { d.coat } else { h };
            c.hline(s.x + 1, s.right() - 2, s.y + 1, band.at(Tone::Shade), z.hi);
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
        Hat::Veil | Hat::Diving => super::special::hood(c, d, r),
    }
}

// ---------------------------------------------------------------------------------------------
// From behind

fn up(c: &mut Canvas, d: &Dress, r: &Rig) {
    legs_front(c, d, r);
    coat_front(c, d, r, false);
    if bony(d) {
        super::bone::rags(c, d, r, false);
        arms_front(c, d, r);
        neck(c, d, r);
        super::bone::skull_back(c, d, r);
        hat_down(c, d, r);
        return;
    }
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
            let peaked = i32::from(d.look.head.hat == Hat::Peaked);
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x - peaked, s.y - 7, s.w + 2 * peaked, 14), d.hat, relief::HAT);
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
    if d.look.extras.contains(&Extra::Shawl) {
        shawl(c, d, r, false);
    }
}

// ---------------------------------------------------------------------------------------------
// From the side, facing east

fn side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let t = r.top;
    let shoulder = (CX - 1 + r.lean, r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe));
    // The far arm and leg, behind everything.
    arm_side(c, d, r, shoulder, r.pose.arm[1], true);
    leg_side(c, d, r, r.pose.leg[1], r.pose.lift[1], true);
    let bone = bony(d);
    if !bone {
        hair::back_side(c, d, r);
    }
    if d.look.body.pack {
        c.rect_round(Rect::new(CX - 11 + r.lean, t + 1, 6, 8), d.pack, 1, 2, relief::FAR);
    }
    leg_side(c, d, r, r.pose.leg[0], r.pose.lift[0], false);
    coat_side(c, d, r);
    if bone {
        super::bone::rags_side(c, d, r);
    }
    front_side(c, d, r);
    if d.look.body.pack {
        c.line((CX + 1 + r.lean, t), (CX - 3, r.waist), d.pack.at(Tone::Base), 1, relief::FRONT);
    }
    neck_side(c, d, r);
    head(c, d, r, true);
    if bone {
        super::bone::face_side(c, d, r);
    } else {
        face_side(c, d, r);
        hair::side(c, d, r);
    }
    hat_side(c, d, r);
    if d.look.extras.contains(&Extra::Shawl) {
        let (x0, x1) = (CX - r.side_w() / 2 - 2 + r.lean, CX + r.side_w() / 2 + 1 + r.lean);
        c.polygon_cloth(
            &[(x0 + 2, t - 1), (x1 - 1, t - 1), (x1 + 1, t + 4), (x0 - 1, r.waist + 1), (x0, t + 2)],
            shawl_ramp(d),
            90,
            Z::flat(relief::FRONT + 2),
        );
    }
    arm_side(c, d, r, shoulder, r.pose.arm[0], false);
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
    if bony(d) && d.look.body.legs == Legs::Bare {
        super::bone::shin(c, d, hx + 1, top, fx + 1, foot - rows, z);
    } else {
        c.polygon_cloth(&[(hx, top), (hx + 3, top), (fx + 3, foot - rows), (fx, foot - rows)], legs, 70, z);
    }
    let boot = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(fx, foot - rows + 1, 5, rows), boot, 1, 1, if far { relief::FAR } else { relief::BOOT });
    if far {
        // The far leg stands in the near one's shade.
        c.shade(Rect::new(fx - 1, top, 7, foot - top + 1), legs, 1);
        c.shade(Rect::new(fx - 1, foot - rows, 7, rows + 2), boot, 1);
    }
}

fn coat_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let coat = d.look.body.coat;
    let w = r.side_w();
    let x0 = CX - w / 2 - 1;
    let x1 = x0 + w - 1;
    let fl = flare(coat);
    let (t, wy, hem, l) = (r.top, r.waist, r.hem, r.lean);
    // The hem swings behind the walker a frame late.
    let tr = r.trail.0.min(0);
    skirt(c, d, r, x0, x1);
    // The front of the body: a chest on the broad, a belly on the stout, a bust over a bodice.
    let (chest, belly) = match r.build {
        Build::Broad => (1, 0),
        Build::Stout => (0, 3),
        _ if skirted(coat) => (1, -1),
        _ => (0, 0),
    };
    let pts = [
        (x0 + 2 + l, t),
        (x1 - 2 + l, t),
        (x1 + l, t + 2),
        (x1 + chest + l, t + 4),
        (x1 + belly, wy),
        (x1 + fl + tr, hem),
        (x0 - fl + tr, hem),
        (x0, wy),
        (x0 + l, t + 2),
    ];
    c.polygon_cloth(&pts, d.coat, 80, relief::COAT);
    if hem - wy > 3 {
        hem_folds(c, d.coat, x0 - fl + tr + 1, x1 + fl + tr - 1, hem, (hem - wy - 2).min(5), r.pose.phase);
    }
    c.shade(Rect::new(CX - 3 + l, t - 2, 8, 4), d.coat, 1);
    let z = relief::COAT.hi;
    if skirted(coat) {
        c.hline(x0, x1 + belly, wy, d.coat.at(Tone::Shade), z);
    }
    if matches!(coat, Coat::Coat | Coat::Overcoat | Coat::Jacket | Coat::Canvas) {
        // The lapel along the front edge, lit.
        c.vline(x1 - 1 + l, t + 1, t + 4, d.coat.at(Tone::Lift), z + 1);
    }
    if matches!(coat, Coat::Coat | Coat::Canvas) {
        belt(c, x0, x1 + belly, wy, x1 + belly - 2, z + 2);
    }
}

fn front_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    let w = r.side_w();
    let x1 = CX - w / 2 - 1 + w - 1 + r.lean;
    match d.look.body.front {
        Front::Scarf => {
            c.rect_round(Rect::new(CX - 3 + r.lean, t - 2, 8, 4), f, 1, 2, Z::new(relief::FRONT, relief::FRONT + 1));
            // The tail streams back a frame behind the lean.
            let sx = r.trail.0.min(0) - i32::from(r.pose.lean > 0);
            let sy = -r.trail.1;
            c.polygon_cloth(
                &[(x1 - 1, t + 1), (x1 + 1, t + 1), (x1 + 1 + sx, t + 6 + sy), (x1 - 1 + sx, t + 6 + sy)],
                f,
                60,
                Z::flat(relief::FRONT + 1),
            );
        }
        Front::Shirt | Front::Tie | Front::Waistcoat => {
            c.polygon_cloth(&[(x1 - 2, t), (x1, t), (x1, t + 4), (x1 - 1, t + 4)], f, 40, Z::flat(relief::FRONT));
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
    let belly = if r.build == Build::Stout { 3 } else { 0 };
    c.polygon_cloth(
        &[
            (x1 - 1, r.top + 2),
            (x1, r.top + 2),
            (x1 + belly, r.waist),
            (x1 + 2 - r.lean, r.hem - 1),
            (x1 - 2 - r.lean, r.hem - 1),
        ],
        f,
        30,
        Z::flat(relief::FRONT),
    );
    c.hline(CX - 3, x1 + belly - r.lean, r.waist, f.at(Tone::Shade), relief::FRONT + 1);
}

/// An arm from the side: the upper arm from the shoulder to an elbow, the forearm on to the
/// hand, `swing` px along the facing; the elbow's crease in shade and a cuff at the wrist.
fn arm_side(c: &mut Canvas, d: &Dress, r: &Rig, shoulder: (i32, i32), swing: i32, far: bool) {
    let (sx, sy) = shoulder;
    let z = if far { relief::FAR } else { relief::ARM };
    let hand_y = sy + r.p.arm_l - 4 - r.pose.raise[usize::from(far)];
    let elbow = (sx + swing / 3 - i32::from(swing > 1), sy + (hand_y - sy) / 2);
    let hx = sx + swing;
    c.polygon_cloth(&[(sx - 1, sy), (sx + 2, sy), (elbow.0 + 2, elbow.1), (elbow.0 - 1, elbow.1)], d.coat, 80, z);
    c.polygon_cloth(
        &[(elbow.0 - 1, elbow.1), (elbow.0 + 2, elbow.1), (hx + 2, hand_y), (hx - 1, hand_y)],
        d.coat,
        80,
        z,
    );
    c.dot(elbow.0 - 1, elbow.1, d.coat.at(Tone::Shade), z.hi);
    c.hline(hx - 1, hx + 2, hand_y, d.coat.at(Tone::Light), z.hi);
    hand(c, d, hx - 1, hand_y + 1, z.hi);
    if !far {
        super::held::draw(c, d, r, hx - 1, hand_y + 1);
    }
    if far {
        c.shade(Rect::new(hx - 3, sy, 8, hand_y - sy + 4), d.coat, 1);
        c.shade(Rect::new(hx - 3, hand_y, 8, 5), d.skin, 1);
    }
}

fn neck_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    // The neck leaves the skull behind the jaw, not under its middle.
    let l = r.lean - 2;
    let w = r.neck_w();
    c.polygon_cloth(
        &[(CX - 1 + l, y), (CX + w - 2 + l, y), (CX + w - 2 + l, r.top), (CX - 1 + l, r.top)],
        d.skin,
        60,
        Z::new(1, 2),
    );
    c.shade(Rect::new(CX - 2 + l, y, w + 2, 5), d.skin, 2);
}

fn face_side(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    let fx = s.right() - 4;
    let skin = d.skin;
    // The face in profile: the brow in the fringe's shade, a lit cheek, the jaw and the back of
    // the face toward the ear in shade.
    for y in s.y..s.bottom() {
        for x in s.x..=s.right() {
            let shade = (y == r.brow_y() + 1 && x >= s.x + 4) || y >= s.bottom() - 2 || (x <= s.x + 5 && y >= ey);
            let t = if shade {
                Tone::Mid
            } else if (x == fx - 2 || x == fx - 1) && y == ey + 2 {
                Tone::Lift
            } else {
                Tone::Base
            };
            c.tint(x, y, skin, t);
        }
    }
    eyes(c, d, r.pose.shut, &[(fx, false)], ey, z);
    // The nose, two px past the skull with its shadow under it; the mouth's corner.
    c.vline(s.right(), ey + 2, ey + 3, skin.at(Tone::Base), z);
    c.dot(s.right() - 1, ey + 4, skin.at(Tone::Mid), z);
    c.dot(s.right() - 2, ey + 5, skin.at(Tone::Shade), z);
    match d.look.head.face {
        Face::Glasses => {
            c.hline(fx - 1, fx + 2, ey - 1, Ramp::Iron.at(Tone::Shade), z);
            c.vline(fx + 2, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            c.hline(s.x + 6, fx - 2, ey, Ramp::Iron.at(Tone::Shade), z);
        }
        Face::Beard => {
            c.polygon_cloth(
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
        Hat::Veil | Hat::Diving => super::special::hood(c, d, r),
    }
}
