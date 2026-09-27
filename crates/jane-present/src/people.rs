//! The people in the atlas (ART.md §5, PRESENTATION.md §1.11): every person look, every variant
//! and every seat, all their frames packed at boot in all four layers (the lit tiers read the
//! normals and the true heights); and the pick of a frame for a unit this
//! tick. A unit whose sprite has no look yet keeps its stand-in (`stand_in`).

use jane_art::looks;
use jane_art::person;
use jane_art::sprite::FrameId;
use jane_core::ids::SpriteId;

use crate::atlas::{Atlas, RefId};
pub use crate::facing::Face8;

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
    /// Which of the eight ways it faces ([`Face8`]).
    pub facing: Face8,
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
                sets.push(Set { sprite: r.sprite, variant: r.variant, seat: r.seat, frames });
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

    /// The frame set `set` shows for `pose`, and whether it is drawn mirrored (the three west
    /// sectors mirror the east ones). A diagonal the look has no frames for is shown from the
    /// side; a fight frame it has none for, as the plain pick.
    pub fn frame(&self, set: u16, pose: Pose) -> (RefId, bool) {
        let s = &self.sets[usize::from(set)];
        let at = |f: FrameId| s.frames.iter().find(|(g, _)| *g == f).map(|(_, r)| *r);
        let side = Pose { facing: Face8::of(pose.facing.cardinal()), ..pose };
        let plain = |p: Pose| Pose { act: None, hurt: false, ..p };
        let (id, r) = [pose, side, plain(pose), plain(side)]
            .into_iter()
            .map(pick)
            .find_map(|f| at(f).map(|r| (f, r)))
            .unwrap_or((FrameId::Down, s.frames[0].1));
        (r, !pose.dead && pose.facing.west() && id.faces_east())
    }
}

/// Which frame a person shows (ART.md §4, PRESENTATION §1.11): dead, one of the two dead
/// frames by id; walking, the six-frame cycle by ticks walked; standing, the standing frame and
/// the breathe in turn, each unit a little out of step with the next. Eight facings: the west
/// ones are the east ones' frames, mirrored by [`People::frame`].
pub fn pick(p: Pose) -> FrameId {
    use FrameId as F;
    if p.dead {
        return if p.id & 1 == 0 { F::Dead } else { F::Dead2 };
    }
    let beat = |t: u32| (t / ACT_TICKS).min(2) as usize;
    let (atk, cast, hurt) = match p.facing {
        Face8::South => {
            ([F::AtkDown1, F::AtkDown2, F::AtkDown3], [F::CastDown1, F::CastDown2, F::CastDown3], F::HurtDown)
        }
        Face8::North => ([F::AtkUp1, F::AtkUp2, F::AtkUp3], [F::CastUp1, F::CastUp2, F::CastUp3], F::HurtUp),
        Face8::East | Face8::West => ([F::Atk1, F::Atk2, F::Atk3], [F::Cast1, F::Cast2, F::Cast3], F::Hurt),
        Face8::SouthEast | Face8::SouthWest => (
            [F::AtkDownRight1, F::AtkDownRight2, F::AtkDownRight3],
            [F::CastDownRight1, F::CastDownRight2, F::CastDownRight3],
            F::HurtDownRight,
        ),
        Face8::NorthEast | Face8::NorthWest => (
            [F::AtkUpRight1, F::AtkUpRight2, F::AtkUpRight3],
            [F::CastUpRight1, F::CastUpRight2, F::CastUpRight3],
            F::HurtUpRight,
        ),
    };
    match p.act {
        Some(Act::Attack(t)) => return atk[beat(t)],
        Some(Act::Cast(t)) => return cast[beat(t)],
        None if p.hurt => return hurt,
        None => {}
    }
    let cycle = walk_cycle(p.facing);
    if p.anim > 0 {
        // Step off on the first contact, not the standing frame.
        cycle[(1 + (p.anim - 1) / WALK_TICKS) as usize % 6]
    } else if (p.tick + p.id * 7) / BREATHE_TICKS % 2 == 1 {
        cycle[6]
    } else {
        cycle[0]
    }
}

/// The six walk frames and the breathe a facing shows, people's and creatures' alike.
pub fn walk_cycle(f: Face8) -> [FrameId; 7] {
    use FrameId as F;
    match f {
        Face8::South => [F::Down, F::Down1, F::Down2, F::Down3, F::Down4, F::Down5, F::DownB],
        Face8::North => [F::Up, F::Up1, F::Up2, F::Up3, F::Up4, F::Up5, F::UpB],
        Face8::East | Face8::West => [F::Side, F::Side1, F::Side2, F::Side3, F::Side4, F::Side5, F::SideB],
        Face8::SouthEast | Face8::SouthWest => {
            [F::DownRight, F::DownRight1, F::DownRight2, F::DownRight3, F::DownRight4, F::DownRight5, F::DownRightB]
        }
        Face8::NorthEast | Face8::NorthWest => {
            [F::UpRight, F::UpRight1, F::UpRight2, F::UpRight3, F::UpRight4, F::UpRight5, F::UpRightB]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_core::action::Facing;

    fn pose(facing: Facing, anim: u32, tick: u32) -> Pose {
        Pose { facing: Face8::of(facing), anim, tick, dead: false, id: 0, act: None, hurt: false }
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
        // The diagonals: their own walks, breathes, blows, casts and hurts.
        let diag = |facing, anim, tick| Pose { facing, ..pose(Facing::South, anim, tick) };
        assert_eq!(pick(diag(Face8::SouthEast, 1, 0)), FrameId::DownRight1);
        assert_eq!(pick(diag(Face8::SouthWest, 0, BREATHE_TICKS)), FrameId::DownRightB);
        assert_eq!(pick(diag(Face8::NorthWest, 13, 0)), FrameId::UpRight3);
        assert_eq!(pick(Pose { act: Some(Act::Cast(0)), ..diag(Face8::NorthEast, 0, 0) }), FrameId::CastUpRight1);
        assert_eq!(pick(Pose { hurt: true, ..diag(Face8::SouthWest, 0, 0) }), FrameId::HurtDownRight);
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
        // The diagonals are her own frames, the west ones the east ones mirrored.
        for (f, mirrored) in [(Face8::SouthEast, false), (Face8::SouthWest, true), (Face8::NorthWest, true)] {
            let (r, m) = people.frame(seats[0], Pose { facing: f, ..pose(Facing::South, 0, 0) });
            assert_eq!(m, mirrored, "{f:?}");
            assert_ne!(r, people.frame(seats[0], pose(Facing::East, 0, 0)).0, "{f:?} has frames of its own");
        }
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
