//! The people composer (ART.md §2.1, §4, §4.1): one generator for every person at 32 x 40 with
//! the feet on (16, 36).
//!
//! ```text
//! person::render(look, seed) -> SpriteSet     every frame a person promises, four layers each
//! person::seat(&set, seat)   -> SpriteSet     the coat swap of seat 1..=3 (seat 0 is the set)
//! person::seed(sprite_name)  -> u32           the stable seed of a sprite id
//! ```
//!
//! Frames (ART.md §4): `Down, Down1 .. Down5, DownB`, the same for `Up` and `Side`, then
//! `Dead` and `Dead2` (§4.1, [`fallen`]). West is `Side` mirrored at draw time; a mirrored
//! normal has its `nx` flipped by the blit.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

mod bone;
mod build;
mod draw;
mod fallen;
mod hair;
mod held;
mod pose;
mod special;
mod task;

use jane_core::hash::fnv1a;
use jane_data::{EmitRole, PersonLook, Skin};

pub use build::{Proportions, of as proportions};
pub use pose::{ASIDE, BLINK, BREATHE, FALLEN, Facing, LAND, LIVING, Pose, TURN, WALK_DOWN, WALK_SIDE, WALK_UP};

use crate::canvas::Canvas;
use crate::palette::{Ix, Ramp, Tone};
use crate::sprite::{FrameId, Role, SpriteSet};

/// A person's frame, px.
pub const W: i32 = 32;
/// A person's frame, px.
pub const H: i32 = 40;
/// The feet: `(w / 2, h - 4)` (ART.md §1).
pub const AX: i32 = 16;
/// The feet's row.
pub const AY: i32 = 36;

/// The four seats' coats (ART.md §3): seat 0 wears the look's own coat; seats 1 to 3 swap the
/// coat role to these, and nothing else. Chosen to stay apart in the dark and the mist and off
/// the enemy reds, as the TS build's `SEAT_COATS` were.
pub const SEAT_COATS: [Ramp; 3] = [Ramp::ClothTeal, Ramp::ClothMoss, Ramp::ClothOchre];

/// Every frame every person promises, in order: the walks and breathes, the dead, then the
/// blinks and the turns ([`ASIDE`]).
pub fn frame_ids() -> impl Iterator<Item = FrameId> {
    LIVING.iter().map(|(f, _, _)| *f).chain([FrameId::Dead, FrameId::Dead2]).chain(ASIDE.iter().map(|(f, _, _)| *f))
}

/// Every frame a person promises: [`frame_ids`], then, if it attacks or casts, its attack, cast,
/// hurt and landing frames (ART.md §4), then the beats of its work (`task`, ART-PLAN Q4).
pub fn frame_ids_for(fight: Fight, work: jane_data::Task) -> Vec<FrameId> {
    frame_ids()
        .chain(pose::fight(fight.attacks, fight.casts).into_iter().map(|(f, _, _)| f))
        .chain(task::frames(work).into_iter().map(|(f, _, _)| f))
        .collect()
}

/// What a person does besides walk: strike, cast (either brings the hurt frames).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fight {
    /// It has a spell whose animation is an attack.
    pub attacks: bool,
    /// It has a spell whose animation is a cast.
    pub casts: bool,
}

/// The stable seed of a sprite id: FNV-1a of its name, so it never moves when rows are added.
pub fn seed(sprite: &str) -> u32 {
    fnv1a(sprite.as_bytes())
}

/// A look resolved to ramps: what every part is drawn in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dress {
    look: PersonLook,
    skin: Ramp,
    hair: Ramp,
    hat: Ramp,
    coat: Ramp,
    front: Ramp,
    legs: Ramp,
    boots: Ramp,
    pack: Ramp,
    eye: Ix,
    eye_emits: bool,
}

/// A ramp named in a look, or an error naming it.
pub fn ramp(name: &str) -> Result<Ramp, String> {
    Ramp::by_name(name).ok_or_else(|| format!("no ramp \"{name}\""))
}

impl Dress {
    fn new(look: &PersonLook) -> Result<Dress, String> {
        let b = &look.body;
        let skin = match look.head.skin {
            Skin::Skin => Ramp::Skin,
            Skin::SkinPale => Ramp::SkinPale,
            Skin::SkinDark => Ramp::SkinDark,
            Skin::Bone => Ramp::Bone,
            Skin::Wax => Ramp::Plaster,
            Skin::Stone | Skin::None => Ramp::Stone,
            Skin::Metal => Ramp::Iron,
            Skin::Gilt => Ramp::Brass,
        };
        let coat = ramp(b.coat_ramp)?;
        let eye_emits = look.emits.contains(&EmitRole::Eye);
        Ok(Dress {
            look: *look,
            skin,
            hair: ramp(look.head.hair_ramp)?,
            hat: look.head.hat_ramp.map_or(Ok(coat), ramp)?,
            coat,
            front: b.front_ramp.map_or(Ok(Ramp::ClothLinen), ramp)?,
            legs: ramp(b.legs_ramp)?,
            boots: b.boots_ramp.map_or(Ok(Ramp::Leather), ramp)?,
            pack: Ramp::Leather,
            eye: if eye_emits { Ramp::Ember.at(Tone::High) } else { Ix::SEAM },
            eye_emits,
        })
    }

    /// The ramp each role is drawn in.
    fn roles(&self) -> Vec<(Role, Ramp)> {
        let mut out = vec![(Role::Skin, self.skin), (Role::Coat, self.coat), (Role::Legs, self.legs)];
        if self.look.head.hair != jane_data::Hair::Bald {
            out.push((Role::Hair, self.hair));
        }
        if self.look.head.hat != jane_data::Hat::None || self.look.head.hat_ramp.is_some() {
            out.push((Role::Hat, self.hat));
        }
        if self.look.body.front != jane_data::Front::None
            || self.look.body.coat == jane_data::Coat::Apron
            || self.look.extras.contains(&jane_data::Extra::Shawl)
        {
            out.push((Role::Front, self.front));
        }
        out.push((Role::Boots, self.boots));
        if self.look.body.pack {
            out.push((Role::Pack, self.pack));
        }
        out
    }
}

/// Render every frame of `look` (its `vary` already resolved: [`PersonLook::variant`]). `seed`
/// is the sprite's stable id ([`seed`]); it sizes the fallen's pool.
pub fn render(look: &PersonLook, seed: u32) -> Result<SpriteSet, String> {
    render_fighting(look, seed, Fight::default())
}

/// [`render`] with the attack, cast and hurt frames `fight` asks for.
pub fn render_fighting(look: &PersonLook, seed: u32, fight: Fight) -> Result<SpriteSet, String> {
    let d = Dress::new(look)?;
    let p = proportions(look.build);
    let mut frames: Vec<(FrameId, Canvas)> =
        LIVING.iter().map(|&(id, facing, pose)| (id, draw::frame(&d, p, facing, pose))).collect();
    // The dead: posed from the front (on her back, limbs thrown) and laid down.
    for (k, id) in [FrameId::Dead, FrameId::Dead2].into_iter().enumerate() {
        // A child's limbs are short and a stout body wide: flung as far, they would lie taller
        // than they stood.
        let mut pose = FALLEN[k];
        if matches!(look.build, jane_data::Build::Child | jane_data::Build::Stout) {
            pose.spread = pose.spread.map(|s| s / 2);
        }
        // The dead are out of any chair: the fallen pose is drawn standing.
        let standing = Dress { look: PersonLook { extras: unseated(look.extras), ..d.look }, ..d };
        let body = draw::frame(&standing, p, Facing::Down, pose);
        // Only flesh bleeds: a skeleton, a waxwork, a statue, an armour and a shade lie dry.
        frames.push((
            id,
            fallen::fallen(&body, seed ^ k as u32, matches!(d.skin, Ramp::Skin | Ramp::SkinPale | Ramp::SkinDark)),
        ));
    }
    for &(id, facing, pose) in &ASIDE {
        // The dead's eyes never close: a blink of a look whose eyes are lights is its stare.
        let pose = if d.eye_emits { Pose { shut: false, ..pose } } else { pose };
        frames.push((id, draw::frame(&d, p, facing, pose)));
    }
    for (id, facing, pose) in pose::fight(fight.attacks, fight.casts).into_iter().chain(task::frames(look.task)) {
        frames.push((id, draw::frame(&d, p, facing, pose)));
    }
    let mut emits = Vec::new();
    for e in look.emits {
        emits.push(match e {
            EmitRole::Eye => Role::Eye,
            EmitRole::Glass => Role::Glass,
            EmitRole::Held => Role::Held,
        });
    }
    Ok(SpriteSet { w: W, h: H, ax: AX, ay: AY, frames, roles: d.roles(), emits })
}

/// `extras` without `seated` (a slice made once at boot for a seated look, else the same one).
fn unseated(extras: &'static [jane_data::Extra]) -> &'static [jane_data::Extra] {
    if extras.contains(&jane_data::Extra::Seated) {
        Box::leak(
            extras.iter().copied().filter(|&e| e != jane_data::Extra::Seated).collect::<Vec<_>>().into_boxed_slice(),
        )
    } else {
        extras
    }
}

/// Seat `seat`'s set (ART.md §3): seat 0 is `set` itself; seats 1 to 3 swap the coat role to
/// [`SEAT_COATS`] and nothing else.
pub fn seat(set: &SpriteSet, seat: usize) -> SpriteSet {
    match seat.checked_sub(1).and_then(|k| SEAT_COATS.get(k)) {
        Some(&coat) => set.swap(Role::Coat, coat),
        None => set.clone(),
    }
}
