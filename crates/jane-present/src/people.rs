//! The people in the atlas (ART.md §5, PRESENTATION.md §1.11): every person look, every variant
//! and every seat, all their frames packed at boot in all four layers (the lit tiers read the
//! normals and the true heights); and the pick of a frame for a unit this
//! tick. A unit whose sprite has no look yet keeps its stand-in (`stand_in`).

use jane_art::looks;
use jane_art::person;
use jane_art::sprite::FrameId;
use jane_core::action::Facing;
use jane_core::ids::SpriteId;

use crate::atlas::{Atlas, RefId};

/// Ticks a walk frame shows: six frames a cycle, 36 ticks a stride pair (PRESENTATION §1.11).
pub const WALK_TICKS: u32 = 6;
/// Ticks between the standing frame and the breathe while idle (ART.md §4).
pub const BREATHE_TICKS: u32 = 40;
/// How tall a person stands, px (the head: `ART.md` §1.1).
const HEIGHT: u8 = 40;

/// Ticks each beat of an attack or a cast shows: wind-up, strike, recover.
pub const ACT_TICKS: u32 = 5;

/// One rendered set's frames in the atlas: the walk, the breathe and the dead of every person,
/// and the attack, cast and hurt frames of one that fights.
#[derive(Clone, Debug)]
struct Set {
    sprite: SpriteId,
    variant: u8,
    seat: u8,
    frames: Vec<(FrameId, RefId)>,
    /// A player's look again with her lantern lit in her hand (ART.md §2.1 `held`, drawn by the
    /// composer at the hand each frame puts where it is): each frame and where its glass glows,
    /// `(column, row)` in the sprite, the middle of its emissive px.
    lit: Vec<LitFrame>,
}

/// A frame of a look holding her lit lantern, and its glass's `(column, row)` in the sprite.
type LitFrame = (FrameId, RefId, Option<(i16, i16)>);

/// Her lantern in her hand this frame: the frame to draw, whether it is mirrored, and where on
/// the canvas its glass glows, relative to the sprite's anchor (its feet): `(dx, rows up)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub look: RefId,
    pub mirror: bool,
    pub glass: (i32, i32),
}

/// Every person look, packed.
#[derive(Clone, Debug, Default)]
pub struct People {
    sets: Vec<Set>,
    /// A cast's light between the hands, a school each (`jane_art::fx::SCHOOLS` order).
    glows: Vec<RefId>,
}

/// What a unit is doing with its hands this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// A blow, this many ticks in.
    Attack(u32),
    /// A spell, this many ticks in.
    Cast(u32),
}

/// What a unit is showing this tick, as the people's frame pick needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    pub facing: Facing,
    /// Ticks walked without stopping; 0 standing.
    pub anim: u32,
    /// The presenter's tick (for the breathe).
    pub tick: u32,
    pub dead: bool,
    /// The unit's id (which of the two dead frames; a breathe out of step with its neighbours).
    pub id: u32,
    /// A blow or a spell under way.
    pub act: Option<Act>,
    /// A blow just taken.
    pub hurt: bool,
}

impl People {
    /// Renders every look and packs its frames. A look that fails to render is left out, and
    /// its units keep their stand-in.
    pub fn build(atlas: &mut Atlas) -> People {
        let ids: Vec<FrameId> = person::frame_ids().collect();
        let mut sets = Vec::new();
        for r in looks::family(looks::Family::Person).unwrap_or_default() {
            let anchor = (r.set.ax as i16, r.set.ay as i16);
            let frames: Vec<(FrameId, RefId)> =
                r.set.frames.iter().map(|(f, c)| (*f, atlas.add_canvas(c, anchor, HEIGHT, |_, _, t| t))).collect();
            if ids.iter().all(|f| frames.iter().any(|(g, _)| g == f)) {
                let lit = lantern(&r)
                    .map(|set| {
                        set.frames
                            .iter()
                            .map(|(f, c)| (*f, atlas.add_canvas(c, anchor, HEIGHT, |_, _, t| t), glass(c)))
                            .collect()
                    })
                    .unwrap_or_default();
                sets.push(Set { sprite: r.sprite, variant: r.variant, seat: r.seat, frames, lit });
            }
        }
        let glows = jane_art::fx::SCHOOLS
            .iter()
            .map(|&s| {
                let c = jane_art::fx::cast_glow(s);
                atlas.add_canvas(&c, (0, c.h() as i16), 2, |_, _, t| t)
            })
            .collect();
        People { sets, glows }
    }

    /// The light a cast of `school` gathers between the hands.
    pub fn glow(&self, school: jane_core::action::School) -> RefId {
        let k = jane_art::fx::SCHOOLS.iter().position(|&s| s == school).unwrap_or(0);
        self.glows[k]
    }

    /// Whether sprite `s` has a look here.
    pub fn has(&self, s: SpriteId) -> bool {
        self.sets.iter().any(|x| x.sprite == s)
    }

    /// The set for sprite `s`: variant `variant % n` of its `n` (ART.md §3: the renderer picks),
    /// seat `seat` if it has seats (else seat 0). Returns an index for [`People::frame`].
    pub fn set(&self, s: SpriteId, variant: u8, seat: u8) -> Option<u16> {
        let n = self.sets.iter().filter(|x| x.sprite == s && x.seat == 0).count().max(1) as u8;
        let v = variant % n;
        let has_seat = self.sets.iter().any(|x| x.sprite == s && x.seat == seat);
        let seat = if has_seat { seat } else { 0 };
        self.sets.iter().position(|x| x.sprite == s && x.variant == v && x.seat == seat).map(|i| i as u16)
    }

    /// The frame set `set` shows for `pose` with her lantern lit in her hand, where the set has
    /// it (a player's look), and where its glass is (§1.7: her lantern's light shines from it).
    pub fn holding_lantern(&self, set: u16, pose: Pose, atlas: &Atlas) -> Option<Held> {
        let s = &self.sets[usize::from(set)];
        if s.lit.is_empty() || pose.dead {
            return None;
        }
        let (plain, mirror) = self.frame(set, pose);
        let k = s.frames.iter().position(|(_, r)| *r == plain)?;
        let (_, look, glass) = *s.lit.iter().find(|(f, _, _)| *f == s.frames[k].0)?;
        let r = atlas.get(look);
        // The glass where the frame has it, else at her hip on her near side.
        let (u, v) = glass.unwrap_or((r.ax + 6, r.ay - 14));
        let u = if mirror { i32::from(r.src.w) - 1 - i32::from(u) } else { i32::from(u) };
        Some(Held { look, mirror, glass: (u - i32::from(r.ax), i32::from(r.ay) - i32::from(v)) })
    }

    /// The frame set `set` shows for `pose`, and whether it is drawn mirrored (west).
    pub fn frame(&self, set: u16, pose: Pose) -> (RefId, bool) {
        let s = &self.sets[usize::from(set)];
        let at = |f: FrameId| s.frames.iter().find(|(g, _)| *g == f).map(|(_, r)| *r);
        // A fight frame the look does not have falls back to the plain pick.
        let id = pick(pose);
        let (id, r) = match at(id) {
            Some(r) => (id, r),
            None => {
                let plain = pick(Pose { act: None, hurt: false, ..pose });
                (plain, at(plain).unwrap_or(s.frames[0].1))
            }
        };
        let side = !matches!(
            id,
            FrameId::AtkDown1
                | FrameId::AtkDown2
                | FrameId::AtkDown3
                | FrameId::AtkUp1
                | FrameId::AtkUp2
                | FrameId::AtkUp3
                | FrameId::CastDown1
                | FrameId::CastDown2
                | FrameId::CastDown3
                | FrameId::CastUp1
                | FrameId::CastUp2
                | FrameId::CastUp3
                | FrameId::HurtDown
                | FrameId::HurtUp
        );
        (r, side && !pose.dead && pose.facing == Facing::West)
    }
}

/// A player's look rendered again holding her lantern, lit (§1.7: her lantern shows when the
/// flat light is low, and the one who carries it is seen carrying it): the look as written with
/// `held` a lantern and the held thing glowing, her seat's coat, her fight frames. `None` for a
/// look no player drives, or one whose hand is already full.
fn lantern(r: &looks::Rendered) -> Option<jane_art::sprite::SpriteSet> {
    use jane_data::{EmitRole, HeldItem, Look};
    if !looks::has_seats(r.sprite) {
        return None;
    }
    let Some((_, Look::Person(p))) = looks::find(r.name) else { return None };
    let v = p.variant(usize::from(r.variant));
    if v.held != HeldItem::None {
        return None;
    }
    let held = jane_data::PersonLook { held: HeldItem::Lantern, emits: &[EmitRole::Held], ..v };
    let set = person::render_fighting(&held, person::seed(r.name), looks::fight(r.sprite)).ok()?;
    Some(person::seat(&set, usize::from(r.seat)))
}

/// The middle of what glows in a frame, `(column, row)`: a held lantern's glass.
fn glass(c: &jane_art::Canvas) -> Option<(i16, i16)> {
    let (mut n, mut sx, mut sy) = (0, 0, 0);
    for y in 0..c.h() {
        for x in 0..c.w() {
            if c.emissive_at(x, y) != jane_art::palette::Ix::CLEAR {
                (n, sx, sy) = (n + 1, sx + x, sy + y);
            }
        }
    }
    (n > 0).then(|| ((sx / n) as i16, (sy / n) as i16))
}

/// Which frame a person shows (ART.md §4, PRESENTATION §1.11): dead, one of the two dead
/// frames by id; walking, the six-frame cycle by ticks walked; standing, the standing frame and
/// the breathe in turn, each unit a little out of step with the next.
pub fn pick(p: Pose) -> FrameId {
    use FrameId as F;
    if p.dead {
        return if p.id & 1 == 0 { F::Dead } else { F::Dead2 };
    }
    let beat = |t: u32| (t / ACT_TICKS).min(2) as usize;
    let (atk, cast, hurt) = match p.facing {
        Facing::South => {
            ([F::AtkDown1, F::AtkDown2, F::AtkDown3], [F::CastDown1, F::CastDown2, F::CastDown3], F::HurtDown)
        }
        Facing::North => ([F::AtkUp1, F::AtkUp2, F::AtkUp3], [F::CastUp1, F::CastUp2, F::CastUp3], F::HurtUp),
        Facing::East | Facing::West => ([F::Atk1, F::Atk2, F::Atk3], [F::Cast1, F::Cast2, F::Cast3], F::Hurt),
    };
    match p.act {
        Some(Act::Attack(t)) => return atk[beat(t)],
        Some(Act::Cast(t)) => return cast[beat(t)],
        None if p.hurt => return hurt,
        None => {}
    }
    let cycle = match p.facing {
        Facing::South => [F::Down, F::Down1, F::Down2, F::Down3, F::Down4, F::Down5, F::DownB],
        Facing::North => [F::Up, F::Up1, F::Up2, F::Up3, F::Up4, F::Up5, F::UpB],
        Facing::East | Facing::West => [F::Side, F::Side1, F::Side2, F::Side3, F::Side4, F::Side5, F::SideB],
    };
    if p.anim > 0 {
        // Step off on the first contact, not the standing frame.
        cycle[(1 + (p.anim - 1) / WALK_TICKS) as usize % 6]
    } else if (p.tick + p.id * 7) / BREATHE_TICKS % 2 == 1 {
        cycle[6]
    } else {
        cycle[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(facing: Facing, anim: u32, tick: u32) -> Pose {
        Pose { facing, anim, tick, dead: false, id: 0, act: None, hurt: false }
    }

    #[test]
    fn blows_and_spells_play_three_beats_per_facing_and_a_hurt_shows() {
        let p = |facing, act, hurt| pick(Pose { act, hurt, ..pose(facing, 3, 0) });
        assert_eq!(p(Facing::East, Some(Act::Attack(0)), false), FrameId::Atk1);
        assert_eq!(p(Facing::South, Some(Act::Attack(ACT_TICKS)), false), FrameId::AtkDown2);
        assert_eq!(p(Facing::North, Some(Act::Cast(2 * ACT_TICKS)), false), FrameId::CastUp3);
        assert_eq!(p(Facing::West, Some(Act::Cast(ACT_TICKS)), false), FrameId::Cast2);
        assert_eq!(p(Facing::South, None, true), FrameId::HurtDown);
        // A townsperson has no fight frames: the plain pick stands in.
        let mut atlas = Atlas::new();
        let people = People::build(&mut atlas);
        let grocer = jane_art::looks::find("town_grocer").unwrap().0;
        let set = people.set(grocer, 0, 0).unwrap();
        let hurt = Pose { hurt: true, ..pose(Facing::South, 0, 0) };
        assert_eq!(people.frame(set, hurt), people.frame(set, pose(Facing::South, 0, 0)));
        let jane = jane_art::looks::find("jane").unwrap().0;
        let js = people.set(jane, 0, 0).unwrap();
        assert_ne!(
            people.frame(js, Pose { act: Some(Act::Attack(ACT_TICKS)), ..pose(Facing::East, 0, 0) }).0,
            people.frame(js, pose(Facing::East, 0, 0)).0
        );
    }

    #[test]
    fn walks_cycle_six_frames_and_idle_breathes() {
        let frames: Vec<FrameId> = (1..=36).step_by(6).map(|a| pick(pose(Facing::South, a, 0))).collect();
        assert_eq!(
            frames,
            [FrameId::Down1, FrameId::Down2, FrameId::Down3, FrameId::Down4, FrameId::Down5, FrameId::Down]
        );
        assert_eq!(pick(pose(Facing::North, 0, 0)), FrameId::Up);
        assert_eq!(pick(pose(Facing::North, 0, BREATHE_TICKS)), FrameId::UpB);
        assert_eq!(pick(pose(Facing::West, 7, 0)), FrameId::Side2);
        assert_eq!(pick(Pose { dead: true, id: 3, ..pose(Facing::East, 0, 0) }), FrameId::Dead2);
    }

    #[test]
    fn every_look_is_packed_with_its_seats_and_west_mirrors() {
        let mut atlas = Atlas::new();
        let people = People::build(&mut atlas);
        let jane = jane_art::looks::find("jane").expect("jane has a look").0;
        let seats: Vec<u16> = (0..4).map(|s| people.set(jane, 0, s).unwrap()).collect();
        for (i, a) in seats.iter().enumerate() {
            for b in &seats[i + 1..] {
                assert_ne!(a, b, "each seat its own coat");
            }
        }
        let (_, west) = people.frame(seats[0], pose(Facing::West, 0, 0));
        let (_, east) = people.frame(seats[0], pose(Facing::East, 0, 0));
        assert!(west && !east);
        // A townsperson has no seats: any seat asks for the look as written.
        let grocer = jane_art::looks::find("town_grocer").unwrap().0;
        assert_eq!(people.set(grocer, 0, 2), people.set(grocer, 0, 0));
        // Variants wrap: villager_old has four.
        let old = jane_art::looks::find("villager_old").unwrap().0;
        assert_eq!(people.set(old, 5, 0), people.set(old, 1, 0));
        let r = atlas.get(people.frame(seats[0], pose(Facing::South, 0, 0)).0);
        assert_eq!((r.src.w, r.src.h, r.ax, r.ay), (32, 40, 16, 36));
    }
}
