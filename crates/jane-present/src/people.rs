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

/// One rendered set's frames in the atlas, in `person::frame_ids()` order.
#[derive(Clone, Debug)]
struct Set {
    sprite: SpriteId,
    variant: u8,
    seat: u8,
    frames: Vec<RefId>,
}

/// Every person look, packed.
#[derive(Clone, Debug, Default)]
pub struct People {
    sets: Vec<Set>,
    ids: Vec<FrameId>,
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
}

impl People {
    /// Renders every look and packs its frames. A look that fails to render is left out, and
    /// its units keep their stand-in.
    pub fn build(atlas: &mut Atlas) -> People {
        let ids: Vec<FrameId> = person::frame_ids().collect();
        let mut sets = Vec::new();
        for r in looks::all().unwrap_or_default() {
            let anchor = (r.set.ax as i16, r.set.ay as i16);
            let frames = ids
                .iter()
                .filter_map(|&f| r.set.frame(f))
                .map(|c| atlas.add_canvas(c, anchor, HEIGHT, |_, _, t| t))
                .collect::<Vec<_>>();
            if frames.len() == ids.len() {
                sets.push(Set { sprite: r.sprite, variant: r.variant, seat: r.seat, frames });
            }
        }
        People { sets, ids }
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

    /// The frame set `set` shows for `pose`, and whether it is drawn mirrored (west).
    pub fn frame(&self, set: u16, pose: Pose) -> (RefId, bool) {
        let id = pick(pose);
        let s = &self.sets[usize::from(set)];
        let k = self.ids.iter().position(|&f| f == id).unwrap_or(0);
        (s.frames[k], !pose.dead && pose.facing == Facing::West)
    }
}

/// Which frame a person shows (ART.md §4, PRESENTATION §1.11): dead, one of the two dead
/// frames by id; walking, the six-frame cycle by ticks walked; standing, the standing frame and
/// the breathe in turn, each unit a little out of step with the next.
pub fn pick(p: Pose) -> FrameId {
    use FrameId as F;
    if p.dead {
        return if p.id & 1 == 0 { F::Dead } else { F::Dead2 };
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
        Pose { facing, anim, tick, dead: false, id: 0 }
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
