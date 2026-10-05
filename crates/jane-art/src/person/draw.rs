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
        let stoop = i32::from(d.look.extras.contains(&Extra::Stoop));
        let breathe = i32::from(pose.breathe);
        // A stoop carries the shoulders a px forward and down and the head a px further; an
        // old build carries the shoulders forward by half its stoop and the head by all of it
        // (three quarters on, the head by half).
        let (bent, ahead) = match facing {
            Facing::Side => (stoop + p.stoop / 2, stoop + p.stoop),
            Facing::DownRight | Facing::UpRight => (0, p.stoop / 2),
            Facing::Down | Facing::Up => (0, 0),
        };
        let sunk = stoop;
        let lean = if facing == Facing::Side { pose.lean + bent } else { 0 };
        let (hy, top) = (pose.bob - breathe + sunk, p.shoulder_y() + pose.bob - breathe + sunk);
        let skull = Rect::new(CX - (p.head_w - 2) / 2 + lean + ahead, p.skull_y() + hy, p.head_w - 2, p.skull_h());
        let hip = p.hip_y() + pose.bob;
        let hang = match coat {
            Coat::Jacket | Coat::Cardigan => -1,
            Coat::Smock | Coat::Canvas => 1,
            Coat::Coat | Coat::Dress | Coat::Apron | Coat::Overcoat => 3,
            Coat::Gown | Coat::Nightdress => AY - 2 - p.hip_y(),
        };
        let hem = (p.hip_y() + pose.lag.0 + hang).min(AY - 2);
        let trail = (pose.lag.1 + bent - lean, pose.lag.0 - pose.bob);
        Rig {
            p,
            pose,
            build: d.look.build,
            skull,
            top,
            waist: (top + hip) / 2 + 1,
            hip,
            hem,
            lean,
            trail,
            bone: bony(d),
            facing,
        }
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

    /// An old back (the stooped build): its shoulders slope further and it carries a hump.
    pub(crate) fn hunched(&self) -> bool {
        self.p.stoop > 0
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
            Build::Slim | Build::Child | Build::Stooped | Build::Tall => 4,
        }
    }

    /// The body's depth seen from the side: narrower than its breadth.
    pub(crate) fn side_w(&self) -> i32 {
        (self.p.shoulder_w - 2).max(8)
    }

    /// How far a three-quarter view turns what is on the front of her (a face, lapels, a
    /// buckle) or the back (the spine, a pack) across the screen, px: the face and the chest
    /// toward the right facing down and to the right; the spine toward the left facing away and
    /// to the right. Nothing on the straight facings.
    pub(crate) fn turn(&self) -> i32 {
        match self.facing {
            Facing::DownRight => 2,
            Facing::UpRight => -2,
            _ => 0,
        }
    }

    /// The front's (or the back's) centre line: [`CX`] turned by [`Rig::turn`].
    pub(crate) fn cx(&self) -> i32 {
        CX + self.turn()
    }

    /// On a diagonal, a swing of `s` px along the facing as a move on the screen, `(dx, dy)`:
    /// two thirds of it across to the right and two thirds of it down (facing the viewer) or up
    /// (facing away). Straight on, nothing.
    pub(crate) fn along(&self, s: i32) -> (i32, i32) {
        // Across the screen a stride shows two thirds of itself; down it, foreshortened, a
        // third (so a foot on the ground stays on the ground's row, give or take one).
        let (k, j) = (s * 2 / 3, s / 3);
        match self.facing {
            Facing::DownRight => (k, j),
            Facing::UpRight => (k, -j),
            _ => (0, 0),
        }
    }

    /// The near foot's toe column and sole row (a bell at the ankle goes where it goes).
    pub(crate) fn near_foot(&self) -> (i32, i32) {
        match self.facing {
            Facing::Side => (CX - 2 + self.pose.leg[0] + 4, AY - self.pose.lift[0]),
            Facing::DownRight | Facing::UpRight => {
                // Outside the near leg: its left facing down and to the right, its right facing
                // away.
                let (dx, dy) = self.along(self.pose.leg[0]);
                let x0 = if self.facing == Facing::DownRight { CX - 5 } else { CX + 5 };
                (x0 + dx, AY + dy - self.pose.lift[0])
            }
            Facing::Down | Facing::Up => (CX - 6 - self.pose.splay[0], AY + self.pose.leg[0] - self.pose.lift[0]),
        }
    }
}

/// One living frame of a person, finished.
pub fn frame(d: &Dress, p: Proportions, facing: Facing, pose: Pose) -> Canvas {
    let mut c = Canvas::new(super::W, super::H);
    // Squashed (a landing, a recoil, a blow taken): the body a px lower over shorter legs and a
    // px wider each side; the hem comes down with it, not a frame late.
    let (p, pose) = if pose.squash {
        let wide = Proportions { shoulder_w: p.shoulder_w + 2, waist_w: p.waist_w + 2, hip_w: p.hip_w + 2, ..p };
        (wide, Pose { bob: pose.bob + 1, lag: (pose.lag.0 + 1, pose.lag.1), ..pose })
    } else {
        (p, pose)
    };
    // Seated in her rocking chair: she faces out of it (or is its back, from behind), lower by a
    // chair's seat, and the walk's beats rock it.
    let seated = d.look.extras.contains(&Extra::Seated);
    let (facing, pose, lean) = if seated { super::special::seat(facing, pose) } else { (facing, pose, 0) };
    let r = Rig::new(p, pose, d, facing);
    if seated {
        super::special::chair_rockers(&mut c);
        if facing.front() {
            super::special::chair_back(&mut c, &r, lean);
        }
    }
    match facing {
        Facing::Down | Facing::DownRight => down(&mut c, d, &r),
        Facing::Up | Facing::UpRight => up(&mut c, d, &r),
        Facing::Side => side(&mut c, d, &r),
    }
    if seated {
        if facing.back() {
            super::special::chair_back(&mut c, &r, lean);
        }
        super::special::chair_front(&mut c, d, &r, facing.front());
    }
    super::task::draw(&mut c, d, &r);
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
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, ROLL, STRIPE] {
        c.retone(m, CLOTH);
    }
    c.retone(d.skin, SKIN);
    c.unchecker(d.skin);
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, ROLL, d.skin, d.hair] {
        c.declutter(m);
    }
    super::special::material(c, d, r);
    c.despike();
    c.ao_contact(Rect::new(CX - 7, AY - 1, 14, 4), 0);
    c.outline();
    for m in [d.coat, d.front, d.legs, d.hat, d.boots, d.pack, d.skin, d.hair] {
        c.declutter(m);
    }
    // A pass can leave a checker where it cleared one beside it: until none is left.
    for _ in 0..3 {
        c.unchecker(d.skin);
    }
    c.relight(&glow);
    c.upright(AY);
    if d.look.ghost {
        super::special::ghost(c, r);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared painting

/// Hem folds: `n` creases rising from the hem across `x0..=x1`, each a short line of the
/// cloth's shade with its lit ridge a px to the left on its lower rows: the middle one `len`
/// rows at most three, the others a row shorter, so the cloth above them stays calm and a
/// long skirt never reads as pinstripe. The set sways a px with the walk.
fn hem_folds(c: &mut Canvas, ramp: Ramp, x0: i32, x1: i32, hem: i32, len: i32, phase: u16) {
    let sway = i32::from(phase >= 32768);
    let w = x1 - x0 + 1;
    let n = if w >= 18 { 3 } else { 2 };
    let len = len.min(3);
    for k in 0..n {
        let x = x0 + (k + 1) * w / (n + 1) + sway;
        let l = if n == 3 && k == 1 { len } else { len - 1 };
        for y in hem - l..hem {
            c.tint(x, y, ramp, Tone::Shade);
            if y >= hem - 2 {
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

/// A skin mitt of a hand, 4 x 4 from `(x, y)` with its corners off, lit in its top left and
/// with its shadow px at the bottom right: big enough that the outline leaves it a heart of
/// skin, so it reads as a hand and not as a dark knot.
pub(crate) fn hand(c: &mut Canvas, d: &Dress, x: i32, y: i32, z: u8) {
    if bony(d) {
        super::bone::hand(c, d, x, y, z);
        return;
    }
    c.ellipse(Rect::new(x, y, 4, 4), d.skin.at(Tone::Base), z);
    c.dot(x + 1, y + 1, d.skin.at(Tone::Lift), z);
    c.dot(x + 2, y + 2, d.skin.at(Tone::Mid), z);
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
        Coat::Gown | Coat::Nightdress | Coat::Coat => 2,
        Coat::Overcoat | Coat::Smock => 1,
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
                || (y >= s.bottom() - 1 && x >= CX - 1);
            let t = if shade {
                Tone::Mid
            } else if (x == s.x + 3 || x == s.x + 4) && y == ey + 2 {
                Tone::Lift
            } else {
                Tone::Base
            };
            c.tint(x, y, skin, t);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The pack

/// The blanket rolled and strapped over a pack: a traveller's, and from behind or the side
/// the silhouette's. Cream wool with a red stripe near each end.
pub(crate) const ROLL: Ramp = Ramp::ClothCream;
/// The blanket's stripes.
pub(crate) const STRIPE: Ramp = Ramp::ClothRed;

/// The bedroll lying across her back, seen from behind or in front: a roll `w` px long from
/// `x0`, its top on row `y`, lit along its top, its ends in shade, a red stripe near each end
/// and two leather straps round it. From in front it is not drawn: its ends past her
/// shoulders read as pegs, and her arms raised to cast tangled with them.
fn bedroll_across(c: &mut Canvas, x0: i32, y: i32, w: i32, z: Z) {
    c.rect_round(Rect::new(x0, y, w, 4), ROLL, 1, 2, z);
    c.hline(x0 + 2, x0 + w - 3, y + 1, ROLL.at(Tone::Light), z.hi);
    for x in [x0, x0 + w - 1] {
        c.vline(x, y + 1, y + 2, ROLL.at(Tone::Shade), z.hi);
    }
    // The stripes two px in, each two px wide so it stays a stripe where only its end shows.
    for x in [x0 + 2, x0 + w - 4] {
        c.fill_rect(Rect::new(x, y, 2, 4), STRIPE.at(Tone::Base), z.hi + 1);
    }
    for x in [x0 + 5, x0 + w - 6] {
        c.vline(x, y, y + 3, Ramp::Leather.at(Tone::Shade), z.hi + 1);
    }
}

/// The bedroll's end seen from the side: a disc of wool six px across and five high, its
/// spiral a dark curl in it, a strap round it.
fn bedroll_end(c: &mut Canvas, x: i32, y: i32, z: Z) {
    c.ellipse_lit(Rect::new(x, y, 6, 5), ROLL, z);
    c.hline(x + 2, x + 3, y + 1, ROLL.at(Tone::Shade), z.hi);
    c.dot(x + 1, y + 2, ROLL.at(Tone::Shade), z.hi);
    c.dot(x + 3, y + 2, ROLL.at(Tone::Deep), z.hi);
    c.hline(x + 2, x + 3, y + 3, ROLL.at(Tone::Shade), z.hi);
    c.dot(x + 4, y + 2, STRIPE.at(Tone::Base), z.hi);
}

/// A rucksack seen from behind, its middle on `px`: the body, a flap lit along its top edge,
/// two straps down it with brass buckles, and the bedroll across its top.
fn rucksack_back(c: &mut Canvas, d: &Dress, px: i32, t: i32) {
    let z = relief::PACK;
    c.rect_round(Rect::new(px - 5, t + 2, 10, 9), d.pack, 1, 2, z);
    c.rect_round(Rect::new(px - 5, t + 2, 10, 4), d.pack, 1, 1, Z::new(z.hi, z.hi + 1));
    c.hline(px - 4, px + 3, t + 5, d.pack.at(Tone::Shade), z.hi + 1);
    for x in [px - 3, px + 2] {
        c.vline(x, t + 4, t + 9, d.pack.at(Tone::Shade), z.hi + 1);
        c.dot(x, t + 7, Ramp::Brass.at(Tone::Light), z.hi + 2);
    }
    if d.look.body.roll {
        bedroll_across(c, px - 10, t - 1, 20, Z::new(z.hi, z.hi + 1));
    }
}

// ---------------------------------------------------------------------------------------------
// Facing the viewer

fn down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let bone = bony(d);
    let t = r.top;
    if !bone {
        hair::back_down(c, d, r);
    }
    if r.facing == Facing::DownRight {
        // Three quarters on: the pack peeks past her near side, the far arm behind her.
        if d.look.body.pack && !bone {
            c.rect_round(Rect::new(CX - 10, t + 1, 5, 9), d.pack, 1, 2, relief::FAR);
            c.shade(Rect::new(CX - 10, t + 1, 5, 9), d.pack, 1);
        }
        arm_diag(c, d, r, false);
    }
    legs_front(c, d, r);
    coat_front(c, d, r, true);
    if d.look.body.pack && !bone {
        // Its straps over her shoulders.
        let (s0, s1) = span(r.p.shoulder_w);
        let turn = r.turn();
        for x in [s0 + 2 + turn / 2, s1 - 2 + turn] {
            c.vline(x, t, t + 6, d.pack.at(Tone::Base), relief::FRONT);
            c.dot(x, t + 1, d.pack.at(Tone::Light), relief::FRONT);
        }
    }
    if bone {
        super::bone::rags(c, d, r, true);
    }
    // The neck first, so a scarf or a collar wraps over it.
    neck(c, d, r);
    front_down(c, d, r);
    arms_front(c, d, r);
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
    if r.facing.diagonal() {
        // Three quarters: the legs a px closer, the far one first, a px up the screen (it
        // stands further off) and in the near one's shade; each foot strides along the diagonal
        // and its boot points the way she faces.
        let near_left = r.facing == Facing::DownRight;
        let order = if near_left { [1, 0] } else { [0, 1] };
        for i in order {
            let near = (i == 0) == near_left;
            let k = usize::from(!near);
            let (dx, dy) = r.along(r.pose.leg[k]);
            let x0 = if i == 0 { CX - 4 } else { CX + 1 };
            let foot = AY + dy - r.pose.lift[k] - i32::from(!near);
            leg_front(c, d, r, x0, dx, foot, 1, !near);
        }
        return;
    }
    for i in 0..2 {
        let x0 = if i == 0 { CX - 5 } else { CX + 1 };
        let out = if i == 0 { -r.pose.splay[i] } else { r.pose.splay[i] };
        let foot = AY + r.pose.leg[i] - r.pose.lift[i];
        leg_front(c, d, r, x0, out, foot, 0, false);
    }
}

/// A leg and its boot seen from the front or behind, 4 px wide from `x0` at the hip, its foot
/// `out` px further out, the sole on `foot`; the boot `toe` px longer to the right (turned
/// three quarters), and `far`: behind the other, in its shade.
#[allow(clippy::too_many_arguments)]
fn leg_front(c: &mut Canvas, d: &Dress, r: &Rig, x0: i32, out: i32, foot: i32, toe: i32, far: bool) {
    let boot_h = boot_rows(d);
    let legs = leg_ramp(d);
    let top = r.hip - 1;
    let bot = foot - boot_h;
    let (zl, zb) = if far { (relief::FAR, relief::FAR) } else { (relief::LEG, relief::BOOT) };
    if bony(d) && d.look.body.legs == Legs::Bare {
        // A shin bone, two px, with the knee's knob.
        super::bone::shin(c, d, x0 + 1, top, x0 + 1 + out, bot, zl);
    } else if bot >= top {
        c.polygon_cloth(&[(x0, top), (x0 + 3, top), (x0 + 3 + out, bot), (x0 + out, bot)], legs, 70, zl);
    }
    if d.look.body.legs == Legs::Pyjamas {
        c.folds(Rect::new(x0 + out.min(0), top, 4 + out.abs(), bot - top + 1), legs, 2, 0);
    }
    let ramp = if d.look.body.boots == Boots::Bare { d.skin } else { d.boots };
    c.rect_round(Rect::new(x0 + out, foot - boot_h + 1, 4 + toe, boot_h), ramp, 1, 1, zb);
    if far {
        c.shade(Rect::new(x0 + out.min(0) - 1, top, 6 + out.abs() + toe, foot - top + 1), legs, 1);
        c.shade(Rect::new(x0 + out - 1, foot - boot_h, 6 + toe, boot_h + 2), ramp, 1);
    }
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
    // Just turned: a hem below the hip is still flung out a px each side.
    let swung = i32::from(r.pose.turn && r.hem > r.hip);
    let fl = flare(coat) + spread + swung;
    let hem = r.hem;
    let (h0, h1) = span(r.p.hip_w + 2 * fl);
    let (k0, k1) = span(r.p.hip_w);
    skirt(c, d, r, k0, k1);
    // A fitted bodice is narrower at the waist than the build.
    let waist_w = if skirted(coat) { r.p.waist_w - 2 } else { r.p.waist_w };
    let (w0, w1) = span(waist_w);
    let mut pts: Vec<(i32, i32)> = Vec::with_capacity(12);
    // An old back's shoulders fall away further: round, not sloped.
    let slope = if r.hunched() { 3 } else { 2 };
    if r.square() {
        pts.extend([(s0, t), (s1, t)]);
    } else {
        pts.extend([(s0 + slope, t), (s1 - slope, t), (s1, t + slope)]);
    }
    pts.extend([(w1, wy), (h1, hem), (h0, hem), (w0, wy)]);
    if !r.square() {
        pts.push((s0, t + slope));
    }
    c.polygon_cloth(&pts, d.coat, 80, relief::COAT);
    let z = relief::COAT.hi;
    let (shade, lift, light) = (d.coat.at(Tone::Shade), d.coat.at(Tone::Lift), d.coat.at(Tone::Light));
    // What is on the front (or the back) follows the turn; the outline does not.
    let cx = r.cx();
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
            belt(c, w0, w1, wy, cx - 1, z);
        }
        // The back seam, and a vent in a long coat.
        if matches!(coat, Coat::Overcoat | Coat::Coat) {
            c.vline(cx - 1, wy + 2, hem - 1, shade, z);
        }
        return;
    }
    match coat {
        Coat::Coat | Coat::Overcoat | Coat::Jacket | Coat::Canvas => {
            // Lapels: the near one lit, the far one in shade; the opening under them.
            let deep = if coat == Coat::Overcoat { 6 } else { 4 };
            c.polyline_fill(&[(cx - 4, t), (cx - 1, t), (cx - 1, t + deep)], lift, z + 1);
            c.polyline_fill(&[(cx, t), (cx + 3, t), (cx, t + deep)], shade, z + 1);
            c.vline(cx, t + deep, hem - 1, shade, z);
            c.vline(cx - 1, t + deep + 1, hem - 1, light, z);
            let buttons: &[i32] = if coat == Coat::Overcoat { &[cx - 3, cx + 2] } else { &[cx - 2] };
            for &bx in buttons {
                for k in 0..2 {
                    c.dot(bx, t + deep + 1 + 3 * k, Ramp::Brass.at(Tone::Light), z + 2);
                }
            }
            if coat == Coat::Coat {
                belt(c, w0, w1, wy, cx - 1, z);
            }
            if coat == Coat::Jacket {
                // Pocket flaps.
                for x in [w0 + 1, cx + 2] {
                    c.hline(x, x + 2, r.hip - 2, shade, z + 1);
                }
            }
        }
        Coat::Cardigan => {
            // Open down the front, the front showing; a line of buttons on the near edge.
            c.polyline_fill(
                &[(cx - 2, t), (cx + 1, t), (cx + 1, hem - 1), (cx - 2, hem - 1)],
                d.front.at(Tone::Base),
                z + 1,
            );
            c.vline(cx + 2, t, hem - 1, shade, z + 1);
            for k in 0..3 {
                c.dot(cx - 3, t + 2 + 2 * k, Ramp::Brass.at(Tone::Light), z + 2);
            }
        }
        Coat::Smock => {
            // A yoke across the chest, gathered under it.
            c.hline(s0 + 2, s1 - 2, t + 3, shade, z);
            c.hline(s0 + 2, s1 - 2, t + 2, lift, z);
            for x in [cx - 3, cx, cx + 3] {
                c.vline(x, t + 4, t + 5, shade, z);
            }
        }
        Coat::Dress | Coat::Apron | Coat::Gown | Coat::Nightdress => {
            // A round neck; the waist seam, lit on its upper lip.
            c.hline(w0, w1, wy, shade, z);
            c.hline(w0, cx - 1, wy - 1, lift, z);
            c.hline(cx - 2, cx + 1, t, shade, z);
        }
    }
}

/// What shows at the front of the coat, facing the viewer.
fn front_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    let f = d.front;
    let t = r.top;
    let cx = r.cx();
    match d.look.body.front {
        Front::Scarf => {
            // Wool wound twice round the neck and tucked under the chin: two rolls, the upper
            // lit on the light's side, a crease between them; knotted on her near side, its end
            // hanging over the chest and swinging a frame behind, fringed.
            let fz = relief::FRONT;
            c.rect_round(Rect::new(CX - 6, t - 3, 12, 5), f, 1, 2, Z::new(fz, fz + 1));
            c.hline(CX - 5, CX + 4, t - 1, f.at(Tone::Shade), fz + 1);
            c.hline(CX - 5, CX, t - 2, f.at(Tone::Light), fz + 1);
            c.hline(CX - 4, CX - 1, t, f.at(Tone::Lift), fz + 1);
            let (sx, sy) = (i32::from(r.pose.phase >= 32768) - r.trail.0.signum(), -r.trail.1);
            let kx = cx - 4;
            scarf_end(c, f, (kx, t + 1), (kx + sx, t + 7 + sy), fz + 1);
            // The fringe: two tassels under the end.
            for x in [kx + sx, kx + 2 + sx] {
                c.dot(x, t + 8 + sy, f.at(Tone::Shade), fz + 1);
            }
            // The knot, a lit lump where the end comes out.
            c.ellipse_lit(Rect::new(kx - 1, t, 4, 3), f, Z::flat(fz + 2));
        }
        Front::Shirt | Front::Tie => {
            c.polyline_fill(
                &[(cx - 3, t - 1), (cx + 2, t - 1), (cx, t + 2), (cx - 1, t + 2)],
                f.at(Tone::Light),
                relief::FRONT,
            );
            c.dot(cx, t, f.at(Tone::Shade), relief::FRONT);
            if d.look.body.front == Front::Tie {
                c.polygon_cloth(
                    &[(cx - 1, t), (cx, t), (cx, t + 5), (cx - 1, t + 5)],
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
                    (cx - 4, t),
                    (cx - 1, t + 3),
                    (cx, t + 3),
                    (cx + 3, t),
                    (cx + 3, r.waist + 1),
                    (cx, r.waist + 3),
                    (cx - 1, r.waist + 3),
                    (cx - 4, r.waist + 1),
                ],
                f,
                60,
                Z::flat(relief::FRONT),
            );
            for k in 0..3 {
                c.dot(cx - 1, t + 4 + 2 * k, Ramp::Brass.at(Tone::Light), relief::FRONT + 1);
            }
        }
        Front::Braces => {
            for x in [cx - 3, cx + 2] {
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

/// A scarf's hanging end, three px wide from `top` (its top-left) to `bot` (its bottom-left):
/// the wool's base with its lit edge on the light's side and a px of shade on the other, so it
/// reads as the scarf's own colour and not as its shadow.
fn scarf_end(c: &mut Canvas, f: Ramp, top: (i32, i32), bot: (i32, i32), z: u8) {
    c.polyline_fill(&[top, (top.0 + 2, top.1), (bot.0 + 2, bot.1), bot], f.at(Tone::Base), z);
    c.line(top, bot, f.at(Tone::Light), 1, z);
    c.line((top.0 + 2, top.1), (bot.0 + 2, bot.1), f.at(Tone::Mid), 1, z);
}

/// An apron: a bib on the chest, a panel from the waist that widens to its hem, the ties at the
/// waist and the neck strap.
fn apron_down(c: &mut Canvas, f: Ramp, r: &Rig) {
    let (t, wy, hem) = (r.top, r.waist, r.hem);
    let z = relief::FRONT;
    let cx = r.cx();
    c.polygon_cloth(&[(cx - 3, t + 2), (cx + 2, t + 2), (cx + 2, wy), (cx - 3, wy)], f, 50, Z::flat(z));
    c.polygon_cloth(&[(cx - 4, wy), (cx + 3, wy), (cx + 5, hem - 1), (cx - 6, hem - 1)], f, 50, Z::flat(z));
    c.hline(cx - 5, cx + 4, wy, f.at(Tone::Shade), z + 1);
    for x in [cx - 3, cx + 2] {
        c.vline(x, t, t + 1, f.at(Tone::Shade), z);
    }
    c.vline(cx, wy + 2, hem - 2, f.at(Tone::Shade), z + 1);
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
    let pc = r.cx();
    c.polygon_cloth(
        &[
            (s0 + 2, t),
            (s1 - 2, t),
            (s1, t + 3),
            (s1 - 1, t + 5),
            (pc, point),
            (pc - 1, point),
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
    if r.facing.diagonal() {
        // The far arm went in behind the coat; the near one, which holds, over it.
        arm_diag(c, d, r, true);
        return;
    }
    for i in 0..2 {
        let (x0, out, hand_y) = arm_front_at(r, i);
        arm_front(c, d, r, x0, out, hand_y, i == 0, i == 0, false);
    }
}

/// Arm `i` facing the viewer or away (0 the screen's left): its shoulder's column, how far out
/// the hand swings, and the hand's last row.
fn arm_front_at(r: &Rig, i: usize) -> (i32, i32, i32) {
    let (s0, s1) = span(r.p.shoulder_w);
    let aw = r.arm_w();
    let spread = r.pose.spread[i];
    let hand_y = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + r.pose.arm[i]
        - spread / 2
        - r.pose.raise[i];
    let (x0, out) = if i == 0 { (s0 - aw + 1, -spread) } else { (s1, spread) };
    (x0, out, hand_y)
}

/// The top-left of hand `i`'s mitt facing the viewer (0 the screen's left), as [`arms_front`]
/// draws it.
pub(crate) fn hand_front(r: &Rig, i: usize) -> (i32, i32) {
    let (x0, out, hand_y) = arm_front_at(r, i);
    (x0 + out + (r.arm_w() - 4) / 2, hand_y - 2)
}

/// An arm three quarters on. `near`: the arm on her near side (the screen's left facing down
/// and to the right, its right facing away), which holds, drawn over the coat; else the far
/// one, tucked two px in behind the body and in its shade, drawn before the coat. Its swing
/// goes along the diagonal.
fn arm_diag(c: &mut Canvas, d: &Dress, r: &Rig, near: bool) {
    let (s0, s1) = span(r.p.shoulder_w);
    let aw = r.arm_w();
    let left = near == (r.facing == Facing::DownRight);
    let k = usize::from(!near);
    // The far arm's swing is half hidden behind her: half of it shows.
    let (dx, dy) = r.along(if near { r.pose.arm[k] } else { r.pose.arm[k] / 2 });
    let hand_y = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe) + r.p.arm_l - 1 + dy - r.pose.raise[k];
    let x0 = match (left, near) {
        (true, true) => s0 - aw + 1,
        (true, false) => s0 - aw + 3,
        (false, true) => s1,
        (false, false) => s1 - 2,
    };
    arm_front(c, d, r, x0, dx, hand_y, left, near, !near);
}

/// An arm from the front: a sleeve `arm_w` px wide from the shoulder to the wrist, lit on its
/// light side, a cuff, then a mitt of a hand. `out` swings the hand outward; `outer_left`: the
/// arm on the screen's left; `holds`: the held thing is in this hand; `far`: behind the body,
/// in its shade.
#[allow(clippy::too_many_arguments)]
fn arm_front(
    c: &mut Canvas,
    d: &Dress,
    r: &Rig,
    x0: i32,
    out: i32,
    hand_y: i32,
    outer_left: bool,
    holds: bool,
    far: bool,
) {
    let aw = r.arm_w();
    let top = r.p.arm_y + r.pose.bob - i32::from(r.pose.breathe);
    let wrist = hand_y - 3;
    // The sleeve leans out from the shoulder: its top a pixel toward the body.
    let a = if outer_left { x0 + 1 } else { x0 - 1 };
    let (bx, z) = (x0 + out, if far { relief::FAR } else { relief::ARM });
    c.polygon_cloth(&[(a, top), (a + aw - 1, top), (bx + aw - 1, wrist), (bx, wrist)], d.coat, 80, z);
    // The cuff, turned back and catching the light.
    c.hline(bx, bx + aw - 1, wrist, d.coat.at(Tone::Light), z.hi);
    c.dot(bx + aw - 1, wrist, d.coat.at(Tone::Shade), z.hi);
    hand(c, d, bx + (aw - 4) / 2, wrist + 1, z.hi);
    if holds {
        super::held::draw(c, d, r, bx + (aw - 4) / 2, wrist + 1);
    }
    if far {
        let (l, rt) = (a.min(bx) - 1, a.max(bx) + aw);
        c.shade(Rect::new(l, top, rt - l + 1, wrist - top + 1), d.coat, 1);
        c.shade(Rect::new(bx - 1, wrist, aw + 2, 5), d.skin, 1);
    }
}

fn neck(c: &mut Canvas, d: &Dress, r: &Rig) {
    let y = r.skull.bottom() - 2;
    let h = r.neck_w() / 2;
    // A skeleton's vertebrae stop at the collar, not in it.
    let b = r.top - i32::from(r.bone);
    c.polygon_cloth(&[(CX - h, y), (CX + h - 1, y), (CX + h - 1, b), (CX - h, b)], d.skin, 60, Z::new(1, 2));
    // Under the chin: the jaw's shade on the throat, a step down, two rows.
    c.shade(Rect::new(CX - h - 1, y, 2 * h + 2, 3), d.skin, 1);
}

/// The face three quarters on, turned to the right: the near eye in from the skull's edge by a
/// cheek, the far one near the edge; the nose between them with its shade on the far side; the
/// mouth under it; the far cheek and the jaw's far side in shade.
fn face_turned(c: &mut Canvas, d: &Dress, r: &Rig) {
    let s = r.skull;
    let (ey, brow) = (r.eye_y(), r.brow_y());
    let z = relief::SKULL.lo;
    let skin = d.skin;
    let cx = r.cx();
    for y in s.y..s.bottom() {
        for x in s.x..s.right() {
            let shade = y == brow + 1
                || (y == brow + 2 && x >= cx + 2)
                || (x >= s.right() - 2 && y >= ey - 1)
                || (y >= s.bottom() - 2 && x >= cx);
            let t = if shade {
                Tone::Mid
            } else if (x == s.x + 3 || x == s.x + 4) && (y == ey + 2 || y == ey + 3) && !(x == s.x + 4 && y == ey + 3) {
                Tone::Lift
            } else {
                Tone::Base
            };
            c.tint(x, y, skin, t);
        }
    }
    let (near, far) = (cx - 5, cx);
    eyes(c, d, r.pose.shut, &[(near, true), (far, false)], ey, z);
    // The nose: its bridge lit, its shade toward the far side; the mouth under it, a lip of
    // light below.
    c.dot(cx - 1, ey + 2, skin.at(Tone::Mid), z);
    c.dot(cx - 2, ey + 2, skin.at(Tone::Lift), z);
    c.hline(cx - 2, cx - 1, ey + 4, skin.at(Tone::Shade), z);
    c.hline(cx - 2, cx - 1, ey + 5, skin.at(Tone::Lift), z);
    match d.look.head.face {
        Face::Glasses => {
            for x in [near - 1, far - 1] {
                c.hline(x, x + 3, ey - 1, Ramp::Iron.at(Tone::Shade), z);
                c.vline(x, ey, ey + 1, Ramp::Glass.at(Tone::Light), z);
                c.vline(x + 3, ey, ey + 1, Ramp::Glass.at(Tone::High), z);
            }
        }
        Face::Beard => {
            c.polygon_cloth(
                &[(cx - 6, ey + 3), (s.right() - 1, ey + 3), (s.right() - 2, s.bottom()), (cx - 5, s.bottom())],
                d.hair,
                90,
                Z::flat(z),
            );
            c.hline(cx - 1, cx, ey + 4, d.skin.at(Tone::Shade), z + 1);
            hair::tame(c, d.hair, Tone::Shade, Tone::Lift);
        }
        Face::Grim => {
            c.hline(cx - 2, cx + 1, ey + 5, d.skin.at(Tone::Shade), z);
            c.hline(near - 1, near + 2, ey - 1, d.hair.at(Tone::Deep), z);
            c.hline(far - 1, far + 2, ey - 1, d.hair.at(Tone::Deep), z);
        }
        Face::Plain | Face::None => {}
    }
}

fn face_down(c: &mut Canvas, d: &Dress, r: &Rig) {
    if r.facing == Facing::DownRight {
        face_turned(c, d, r);
        return;
    }
    let ey = r.eye_y();
    let z = relief::SKULL.lo;
    face_tones(c, d, r);
    // The eyes set in toward the nose, clear of the hair either side: the face reads at 1x.
    eyes(c, d, r.pose.shut, &[(CX - 4, true), (CX + 2, false)], ey, z);
    // The nose: a px of shade on its far side and its lit bridge; the mouth under it, a lip
    // of light below; a warmth on the near cheek.
    c.dot(CX, ey + 2, d.skin.at(Tone::Mid), z);
    c.dot(CX - 1, ey + 2, d.skin.at(Tone::Lift), z);
    c.hline(CX - 1, CX, ey + 4, d.skin.at(Tone::Shade), z);
    c.hline(CX - 1, CX, ey + 5, d.skin.at(Tone::Lift), z);
    match d.look.head.face {
        Face::Glasses => {
            for x in [CX - 5, CX + 1] {
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
            c.hline(CX - 5, CX - 2, ey - 1, d.hair.at(Tone::Deep), z);
            c.hline(CX + 1, CX + 4, ey - 1, d.hair.at(Tone::Deep), z);
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
    let t = i32::from(r.facing == Facing::DownRight);
    let (x0, x1) = if profile { (s.x + 5, s.right() - 1) } else { (s.x + 2 + t, s.right() - 3 + t) };
    // Facing the viewer the jaw tapers to a softer chin than an egg's; in profile it is the
    // jaw's line forward of the skull.
    let k = if profile { 2 } else { 3 };
    c.polyline_fill(
        &[(x0, ey), (x1, ey), (x1 - k, s.bottom() - 1), (x0 + k, s.bottom() - 1)],
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
    // A badge or a peak turns with the face.
    let (cx, t) = (r.cx(), i32::from(r.facing == Facing::DownRight));
    match d.look.head.hat {
        Hat::None => {}
        Hat::Cap => {
            c.set_clip(Some(Rect::new(0, 0, 32, brow - 1)));
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 3, s.w + 2, 10), h, z);
            c.set_clip(None);
            c.rect_round(Rect::new(s.x + t, brow - 2, s.w, 2), h, 1, 1, Z::new(z.hi, z.hi + 1));
            c.shade(Rect::new(s.x + 1 + t, brow - 1, s.w - 2, 3), d.skin, 1);
        }
        Hat::Peaked => {
            c.rect_round(Rect::new(s.x - 1, s.y - 3, s.w + 2, 6), h, 1, 2, z);
            c.rect_round(Rect::new(s.x, s.y + 2, s.w, 2), Ramp::Leather, 1, 1, Z::new(z.hi, z.hi + 1));
            c.disc_lit(cx - 1, s.y, 1, Ramp::Brass, Z::flat(z.hi + 2));
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
            c.disc_lit(cx - 1, s.y - 1, 1, Ramp::Brass, Z::flat(z.hi + 2));
            c.shade(Rect::new(s.x + 1, brow - 1, s.w - 2, 3), d.skin, 1);
        }
        Hat::Scarf => {
            c.ellipse_lit(Rect::new(s.x - 1, s.y - 2, s.w + 2, s.h), h, z);
            // The face shows through the scarf's opening.
            c.set_clip(Some(Rect::new(s.x + 2 + t, brow, s.w - 4, s.bottom() - brow - 1)));
            c.ellipse_lit(r.skull, d.skin, relief::SKULL);
            c.set_clip(None);
            face_down(c, d, r);
            c.ellipse_lit(Rect::new(cx - 2, s.bottom() - 2, 4, 3), h, Z::new(z.hi, z.hi + 1));
        }
        Hat::Veil | Hat::Diving => super::special::hood(c, d, r),
    }
}

// ---------------------------------------------------------------------------------------------
// From behind

fn up(c: &mut Canvas, d: &Dress, r: &Rig) {
    legs_front(c, d, r);
    if r.facing == Facing::UpRight {
        arm_diag(c, d, r, false);
    }
    coat_front(c, d, r, false);
    if bony(d) {
        super::bone::rags(c, d, r, false);
        arms_front(c, d, r);
        neck(c, d, r);
        super::bone::skull_back(c, d, r);
        hat_down(c, d, r);
        return;
    }
    arms_front(c, d, r);
    neck(c, d, r);
    hair::whole_up(c, d, r);
    // The pack over the hair: long hair lies under its straps.
    if d.look.body.pack {
        rucksack_back(c, d, r.cx(), r.top);
    }
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
        let px = CX - 11 + r.lean;
        c.rect_round(Rect::new(px, t + 1, 6, 9), d.pack, 1, 2, relief::FAR);
        c.rect_round(Rect::new(px, t + 1, 6, 4), d.pack, 1, 1, Z::new(relief::FAR.hi, relief::FAR.hi + 1));
        c.dot(px + 1, t + 6, Ramp::Brass.at(Tone::Light), relief::FAR.hi + 1);
        if d.look.body.roll {
            bedroll_end(c, px, t - 3, Z::new(relief::FAR.hi, relief::FAR.hi + 1));
        }
    }
    leg_side(c, d, r, r.pose.leg[0], r.pose.lift[0], false);
    coat_side(c, d, r);
    if bone {
        super::bone::rags_side(c, d, r);
    }
    neck_side(c, d, r);
    front_side(c, d, r);
    if d.look.body.pack {
        c.line((CX + 1 + r.lean, t), (CX - 3, r.waist), d.pack.at(Tone::Base), 1, relief::FRONT);
    }
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
    // The hem swings behind the walker a frame late, and opens with the stride: the front
    // edge carried by the leading knee, the back left behind.
    let tr = r.trail.0.min(0);
    let open = if hem > r.hip { r.pose.leg[0].abs().max(r.pose.leg[1].abs()) / 2 } else { 0 };
    skirt(c, d, r, x0, x1);
    // The front of the body: a chest on the broad, a belly on the stout, a bust over a bodice.
    let (chest, belly) = match r.build {
        Build::Broad => (1, 0),
        Build::Stout => (0, 3),
        _ if skirted(coat) => (1, -1),
        _ => (0, 0),
    };
    // An old back curves out behind the shoulders, its top rounding over to the collar.
    let hump = r.p.stoop / 2;
    let mut pts = vec![
        (x0 + 2 + l, t),
        (x1 - 2 + l, t),
        (x1 + l, t + 2),
        (x1 + chest + l, t + 4),
        (x1 + belly, wy),
        (x1 + fl + tr + open, hem),
        (x0 - fl + tr - open, hem),
        (x0, wy),
    ];
    if hump > 0 {
        pts.push((x0 - hump + l, t + 2 + hump));
    }
    pts.push((x0 + l - hump, t + 2));
    c.polygon_cloth(&pts, d.coat, 80, relief::COAT);
    if hem - wy > 3 {
        hem_folds(
            c,
            d.coat,
            x0 - fl + tr - open + 1,
            x1 + fl + tr + open - 1,
            hem,
            (hem - wy - 2).min(5),
            r.pose.phase,
        );
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
        belt(c, x0, x1 + belly, wy, x1 + belly - 2, z);
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
    // The profile's line, row by row down from the brow: the forehead, the nose a px proud of
    // it for two rows (the face under it filled, so the nose stands on something and survives
    // the despike), the upper lip back under it, the mouth a px further in, the chin.
    let face_x = s.right() - 1;
    for (dy, reach) in [(-2, 0), (-1, 0), (0, 0), (1, 0), (2, 1), (3, 1), (4, 0), (5, -1), (6, -1)] {
        let y = ey + dy;
        if y < s.bottom() {
            c.hline(face_x - 3, face_x + reach, y, skin.at(Tone::Base), relief::SKULL.lo);
        }
    }
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
