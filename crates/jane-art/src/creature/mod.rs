//! The creatures (ART.md §2.2, §4, §4.1, §8 step 4): body plans with their own boxes, anchors
//! and gaits, and the animals each plan draws.
//!
//! ```text
//! creature::render(look, seed, attacks) -> SpriteSet   every frame the plan promises
//! creature::frame_ids(attacks)          -> [FrameId]   in order
//! creature::size(plan)                  -> (w, h, ax, ay)
//! ```
//!
//! Frames: the six-frame walk and a breathe for each facing (`Down .. Down5, DownB`, the same
//! for `Up` and `Side`; west is `Side` mirrored at draw time), the idle pair (`Idle, Idle2`: a
//! dog sitting and tilting its head, a cat sitting and flicking its tail, a sheep grazing, a hen
//! pecking), `Hurt`, `Dead`, and `Atk1 .. Atk3` when some unit drawn as it fights.
//!
//! Every part is a silhouette filled as a soft volume ([`Canvas::inflate`]), so a pelt turns like
//! a body; the painter then lays deliberate clusters (a lit topline, a chest ruff, markings dyed
//! over the shading, a glint in the eye), and [`finish`] cleans the clusters, lays the contact
//! shadow, runs the selective outline and stands the frame up, as the people's does.

mod bird;
mod quad;

use jane_core::grid::Rect;
use jane_data::{CreatureLook, EmitRole, Plan};

use crate::canvas::Canvas;
use crate::palette::{Ix, Ramp, Tone, pallor};
use crate::sprite::{FrameId, Role, SpriteSet};

/// Which way a frame faces. West is `Side` mirrored at draw time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Facing {
    Down,
    Up,
    Side,
}

/// What a frame shows: a walk beat, the breathe, the idle pair, hurt, an attack beat, dead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Beat {
    /// The walk, frame 0 (standing) to 5.
    Walk(u8),
    Breathe,
    Idle(u8),
    Hurt,
    Attack(u8),
    Dead,
}

/// The box, px, and the anchor (the feet: `(w / 2, h - 4)`) of a plan.
pub const fn size(plan: Plan) -> (i32, i32, i32, i32) {
    match plan {
        Plan::QuadrupedMid => (32, 28, 16, 24),
        Plan::QuadrupedSmall => (24, 20, 12, 16),
        Plan::Bird => (20, 20, 10, 16),
    }
}

/// Every frame a creature promises, in order; the attack cycle only if it fights.
pub fn frame_ids(attacks: bool) -> Vec<FrameId> {
    use FrameId as F;
    let mut v = vec![
        F::Down,
        F::Down1,
        F::Down2,
        F::Down3,
        F::Down4,
        F::Down5,
        F::DownB,
        F::Up,
        F::Up1,
        F::Up2,
        F::Up3,
        F::Up4,
        F::Up5,
        F::UpB,
        F::Side,
        F::Side1,
        F::Side2,
        F::Side3,
        F::Side4,
        F::Side5,
        F::SideB,
        F::Idle,
        F::Idle2,
        F::Hurt,
    ];
    if attacks {
        v.extend([F::Atk1, F::Atk2, F::Atk3]);
    }
    v.push(F::Dead);
    v
}

/// The facing and beat of frame `id`.
fn beat_of(id: FrameId) -> (Facing, Beat) {
    use FrameId as F;
    match id {
        F::Down => (Facing::Down, Beat::Walk(0)),
        F::Down1 => (Facing::Down, Beat::Walk(1)),
        F::Down2 => (Facing::Down, Beat::Walk(2)),
        F::Down3 => (Facing::Down, Beat::Walk(3)),
        F::Down4 => (Facing::Down, Beat::Walk(4)),
        F::Down5 => (Facing::Down, Beat::Walk(5)),
        F::DownB => (Facing::Down, Beat::Breathe),
        F::Up => (Facing::Up, Beat::Walk(0)),
        F::Up1 => (Facing::Up, Beat::Walk(1)),
        F::Up2 => (Facing::Up, Beat::Walk(2)),
        F::Up3 => (Facing::Up, Beat::Walk(3)),
        F::Up4 => (Facing::Up, Beat::Walk(4)),
        F::Up5 => (Facing::Up, Beat::Walk(5)),
        F::UpB => (Facing::Up, Beat::Breathe),
        F::Side => (Facing::Side, Beat::Walk(0)),
        F::Side1 => (Facing::Side, Beat::Walk(1)),
        F::Side2 => (Facing::Side, Beat::Walk(2)),
        F::Side3 => (Facing::Side, Beat::Walk(3)),
        F::Side4 => (Facing::Side, Beat::Walk(4)),
        F::Side5 => (Facing::Side, Beat::Walk(5)),
        F::SideB => (Facing::Side, Beat::Breathe),
        F::Idle => (Facing::Down, Beat::Idle(0)),
        F::Idle2 => (Facing::Down, Beat::Idle(1)),
        F::Hurt => (Facing::Side, Beat::Hurt),
        F::Atk1 => (Facing::Side, Beat::Attack(0)),
        F::Atk2 => (Facing::Side, Beat::Attack(1)),
        F::Atk3 => (Facing::Side, Beat::Attack(2)),
        _ => (Facing::Side, Beat::Dead),
    }
}

/// A look resolved to ramps.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Coat {
    pub look: CreatureLook,
    pub body: Ramp,
    pub belly: Ramp,
    pub mark: Ramp,
    pub collar: Option<Ramp>,
    pub eye_emits: bool,
    pub seed: u32,
}

impl Coat {
    fn new(look: &CreatureLook, seed: u32) -> Result<Coat, String> {
        let body = crate::person::ramp(look.ramps.body)?;
        Ok(Coat {
            look: *look,
            body,
            belly: look.ramps.belly.map_or(Ok(body), crate::person::ramp)?,
            mark: look.ramps.mark.map_or(Ok(body), crate::person::ramp)?,
            collar: look.collar.map(crate::person::ramp).transpose()?,
            eye_emits: look.emits.contains(&EmitRole::Eye),
            seed,
        })
    }

    /// Every ramp the creature is painted in, its own first.
    pub fn ramps(&self) -> Vec<Ramp> {
        let mut v = vec![self.body, self.belly, self.mark];
        v.extend(self.collar);
        v.extend(extra_ramps(self.look.anatomy));
        v.dedup();
        v
    }

    fn roles(&self) -> Vec<(Role, Ramp)> {
        let mut v = vec![(Role::Fur, self.body)];
        if self.belly != self.body {
            v.push((Role::Belly, self.belly));
        }
        if self.mark != self.body && self.mark != self.belly {
            v.push((Role::Mark, self.mark));
        }
        v
    }

    /// The eye's colour: glass that shines when it emits, else a deep dark.
    pub fn iris(&self) -> Ix {
        if self.eye_emits { Ramp::GlassLit.at(Tone::High) } else { Ix::INK }
    }
}

/// The ramps an anatomy paints beyond its coat: a hen's comb and beak, a rat's bare tail.
pub(crate) fn extra_ramps(a: jane_data::Anatomy) -> &'static [Ramp] {
    use jane_data::Anatomy as A;
    match a {
        A::Hen => &[Ramp::ClothRed, Ramp::ClothMustard],
        A::Crow => &[Ramp::HairGrey],
        A::Rat | A::Rabbit | A::Cat | A::Dog | A::Fox | A::Sheep => &[Ramp::Skin],
    }
}

/// Render every frame of `look`. `seed` is the sprite's stable id; `attacks`: some unit drawn as
/// this sprite fights, so it has the attack cycle.
pub fn render(look: &CreatureLook, seed: u32, attacks: bool) -> Result<SpriteSet, String> {
    let coat = Coat::new(look, seed)?;
    let (w, h, ax, ay) = size(look.plan);
    let mut frames = Vec::new();
    for id in frame_ids(attacks) {
        let (facing, beat) = beat_of(id);
        let mut c = Canvas::new(w, h);
        match look.plan {
            Plan::QuadrupedMid | Plan::QuadrupedSmall => quad::draw(&mut c, &coat, facing, beat),
            Plan::Bird => bird::draw(&mut c, &coat, facing, beat),
        }
        if beat == Beat::Dead {
            finish_dead(&mut c, &coat, ax, ay);
        } else {
            finish(&mut c, &coat, ax, ay);
        }
        frames.push((id, c));
    }
    let emits = if coat.eye_emits { vec![Role::Eye] } else { Vec::new() };
    Ok(SpriteSet { w, h, ax, ay, frames, roles: coat.roles(), emits })
}

/// What the lit primitives' bands become in fur and feather: a shade, a mid, a base and a light,
/// and the high only where the light strikes square, so the pelt falls in clusters.
pub(crate) const FUR: [Tone; 8] =
    [Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Base, Tone::Light, Tone::Light, Tone::High];
/// A dark pelt's (a black dog, a crow): held low, so it reads black with a sheen along the lit
/// side and never goes grey.
pub(crate) const FUR_DARK: [Tone; 8] =
    [Tone::Deep, Tone::Deep, Tone::Shade, Tone::Shade, Tone::Mid, Tone::Base, Tone::Lift, Tone::Lift];
/// A pale pelt's (a white chest, a fleece): its shade stays light, so white reads as white.
pub(crate) const FUR_PALE: [Tone; 8] =
    [Tone::Shade, Tone::Mid, Tone::Mid, Tone::Base, Tone::Base, Tone::Lift, Tone::Light, Tone::High];

/// The fur map for `ramp`, by how dark its key is.
pub(crate) fn fur_map(ramp: Ramp) -> [Tone; 8] {
    let l = crate::palette::luma(ramp.at(Tone::Base));
    if l < 70_000 {
        FUR_DARK
    } else if l > 170_000 {
        FUR_PALE
    } else {
        FUR
    }
}

/// Clusters, the contact shadow, the selective outline, the true heights.
fn finish(c: &mut Canvas, coat: &Coat, ax: i32, ay: i32) {
    let ramps = coat.ramps();
    for &r in &ramps {
        c.retone(r, fur_map(r));
        c.declutter(r);
    }
    c.despike();
    let (w, _, _, _) = size(coat.look.plan);
    let spread = w / 2 - 2;
    c.ao_contact(Rect::new(ax - spread, ay - 1, 2 * spread, 4), 0);
    c.outline();
    for &r in &ramps {
        c.declutter(r);
    }
    c.upright(ay);
}

/// A dead frame: lined as it lay, pallid, never emitting, as thick as it is wide.
fn finish_dead(c: &mut Canvas, coat: &Coat, ax: i32, ay: i32) {
    let ramps = coat.ramps();
    for &r in &ramps {
        c.retone(r, fur_map(r));
        c.declutter(r);
    }
    c.despike();
    let (w, _, _, _) = size(coat.look.plan);
    let spread = w / 2 - 1;
    c.ao_contact(Rect::new(ax - spread, ay - 2, 2 * spread, 4), 0);
    c.outline();
    c.remap(pallor);
    c.quench();
    c.dome_heights(4);
}

/// The stable seed of a sprite id (the people's: FNV-1a of its name).
pub fn seed(sprite: &str) -> u32 {
    crate::person::seed(sprite)
}
