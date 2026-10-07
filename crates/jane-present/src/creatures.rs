//! The creatures in the atlas (ART.md §2.2, §5, PRESENTATION.md §1.11): every creature look,
//! all its frames packed at boot in all four layers, and the pick of a frame for a unit this
//! tick. A unit whose sprite has a creature look draws it here; `people` draws the people.

use alloc::vec::Vec;
use jane_art::looks::{self, Family};
use jane_art::sprite::FrameId;
use jane_core::ids::SpriteId;

use crate::atlas::{Atlas, Key, RefId, cat};
use crate::facing::Face8;
use crate::people::{BREATHE_TICKS, WALK_TICKS, walk_cycle};

/// Ticks a creature stands still before it sits (or grazes, or pecks): the idle pair.
pub const IDLE_AFTER: u32 = 90;
/// Ticks each beat of the idle pair shows: a dog tilts its head, a hen pecks.
pub const IDLE_TICKS: u32 = 48;
/// Ticks each beat of an attack shows: wind-up, strike, recover.
pub const ATTACK_TICKS: u32 = 4;

/// One creature look's frames in the atlas.
#[derive(Clone, Debug)]
struct Set {
    sprite: SpriteId,
    frames: Vec<(FrameId, RefId)>,
}

/// Every creature look, packed.
#[derive(Clone, Debug, Default)]
pub struct Creatures {
    sets: Vec<Set>,
    /// A serpent's body along its trail: its segments, largest (at the neck) first.
    segments: Vec<(SpriteId, Vec<RefId>)>,
}

/// What a creature is doing this tick, as the frame pick needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose {
    /// Which of the eight ways it faces.
    pub facing: Face8,
    /// Ticks walked without stopping; 0 standing.
    pub anim: u32,
    /// Ticks stood still.
    pub still: u32,
    /// The presenter's tick.
    pub tick: u32,
    pub dead: bool,
    /// Ticks since it last struck, if it is still striking.
    pub attack: Option<u32>,
    /// The unit's id: the breathe and the idle a little out of step with its neighbours.
    pub id: u32,
}

impl Creatures {
    /// Renders every creature look and packs its frames. A look that fails to render is left
    /// out, and its units keep their stand-in.
    pub fn build(atlas: &mut Atlas) -> Creatures {
        let mut sets = Vec::new();
        for r in looks::family(Family::Creature).unwrap_or_default() {
            let anchor = (r.set.ax as i16, r.set.ay as i16);
            let height = r.set.ay.clamp(1, 255) as u8;
            let vs = (r.variant << 4) | r.seat;
            let frames = r
                .set
                .frames
                .iter()
                .map(|(f, c)| {
                    atlas.key_next(Key { cat: cat::UNITS, sprite: r.sprite.0, vs, frame: *f as u8 });
                    (*f, atlas.add_canvas(c, anchor, height, |_, _, t| t))
                })
                .collect();
            sets.push(Set { sprite: r.sprite, frames });
        }
        let mut segments = Vec::new();
        for (sprite, look) in jane_data::looks() {
            let jane_data::Look::Creature(l) = look else { continue };
            if l.plan != jane_data::Plan::SerpentHead {
                continue;
            }
            let seed = jane_art::creature::seed(looks::name_of(*sprite));
            if let Ok(segs) = jane_art::creature::snake_segments(l, seed) {
                let refs = segs
                    .iter()
                    .map(|c| {
                        atlas.add_canvas(
                            c,
                            ((c.w() / 2) as i16, (c.h() - 2) as i16),
                            c.h().clamp(1, 255) as u8,
                            |_, _, t| t,
                        )
                    })
                    .collect();
                segments.push((*sprite, refs));
            }
        }
        Creatures { sets, segments }
    }

    /// The body segments of set `set`, if it is a serpent: largest first.
    pub fn segments(&self, set: u16) -> Option<&[RefId]> {
        let s = self.sets.get(usize::from(set))?.sprite;
        self.segments.iter().find(|(x, _)| *x == s).map(|(_, r)| r.as_slice())
    }

    /// The set for sprite `s`, if it is a creature's.
    pub fn set(&self, s: SpriteId) -> Option<u16> {
        self.sets.iter().position(|x| x.sprite == s).map(|i| i as u16)
    }

    /// The frame set `set` shows for `pose`, and whether it is drawn mirrored (the three west
    /// sectors mirror the east ones). A plan without diagonals shows a diagonal from the side.
    pub fn frame(&self, set: u16, pose: Pose) -> (RefId, bool) {
        let s = &self.sets[usize::from(set)];
        let has = |f: FrameId| s.frames.iter().any(|(g, _)| *g == f);
        let at = |f: FrameId| s.frames.iter().find(|(g, _)| *g == f).map(|(_, r)| *r);
        let side = Pose { facing: Face8::of(pose.facing.cardinal()), ..pose };
        let (id, r) = [pose, side]
            .into_iter()
            .map(|p| pick(p, has(FrameId::Atk1)))
            .find_map(|f| at(f).map(|r| (f, r)))
            .unwrap_or((FrameId::Down, at(FrameId::Down).unwrap_or(s.frames[0].1)));
        (r, !pose.dead && pose.facing.west() && id.faces_east())
    }
}

/// Which frame a creature shows (ART.md §2.2, §4): dead, its dead pose; striking, the attack's
/// three beats; walking, the six-frame cycle by ticks walked; standing a while, the idle pair
/// (it sits and looks at you, grazes, pecks); else the standing frame and the breathe in turn.
pub fn pick(p: Pose, attacks: bool) -> FrameId {
    use FrameId as F;
    if p.dead {
        return F::Dead;
    }
    if let Some(t) = p.attack.filter(|_| attacks) {
        return [F::Atk1, F::Atk2, F::Atk3][(t / ATTACK_TICKS).min(2) as usize];
    }
    let cycle = walk_cycle(p.facing);
    if p.anim > 0 {
        return cycle[(1 + (p.anim - 1) / WALK_TICKS) as usize % 6];
    }
    if p.still >= IDLE_AFTER {
        return if (p.still - IDLE_AFTER + p.id * 11) / IDLE_TICKS % 2 == 1 { F::Idle2 } else { F::Idle };
    }
    if (p.tick + p.id * 7) / BREATHE_TICKS % 2 == 1 { cycle[6] } else { cycle[0] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_core::action::Facing;

    fn pose(facing: Facing, anim: u32, still: u32) -> Pose {
        Pose { facing: Face8::of(facing), anim, still, tick: 0, dead: false, attack: None, id: 0 }
    }

    #[test]
    fn a_creature_trots_sits_when_still_and_strikes_in_three_beats() {
        assert_eq!(pick(pose(Facing::East, 1, 0), false), FrameId::Side1);
        assert_eq!(pick(pose(Facing::South, 0, 0), false), FrameId::Down);
        assert_eq!(pick(pose(Facing::North, 0, IDLE_AFTER), false), FrameId::Idle);
        assert_eq!(pick(pose(Facing::North, 0, IDLE_AFTER + IDLE_TICKS), false), FrameId::Idle2);
        let striking = Pose { attack: Some(ATTACK_TICKS), ..pose(Facing::East, 3, 0) };
        assert_eq!(pick(striking, true), FrameId::Atk2);
        assert_eq!(pick(striking, false), FrameId::Side1, "a creature that never fights walks on");
        assert_eq!(pick(Pose { dead: true, ..pose(Facing::East, 0, 0) }, true), FrameId::Dead);
    }

    #[test]
    fn every_creature_is_packed_and_west_mirrors_the_side() {
        let mut atlas = Atlas::with_layers(true);
        let c = Creatures::build(&mut atlas);
        let dog = jane_art::looks::find("dog").expect("the dog has a look").0;
        let set = c.set(dog).expect("the dog is a creature");
        let (east, m) = c.frame(set, pose(Facing::East, 0, 0));
        let (west, mw) = c.frame(set, pose(Facing::West, 0, 0));
        assert!(!m && mw && east == west);
        // The diagonals are her own frames, the west ones the east ones mirrored.
        let side = c.frame(set, pose(Facing::East, 0, 0)).0;
        let diag = |f: Face8| c.frame(set, Pose { facing: f, ..pose(Facing::South, 0, 0) });
        let (se, mse) = diag(Face8::SouthEast);
        let (sw, msw) = diag(Face8::SouthWest);
        let (ne, mne) = diag(Face8::NorthEast);
        let (nw, mnw) = diag(Face8::NorthWest);
        assert!(se != side && ne != side && se != ne, "the dog has diagonals of her own");
        assert!(se == sw && ne == nw && !mse && msw && !mne && mnw, "the west diagonals mirror the east");
        let (sit, ms) = c.frame(set, pose(Facing::West, 0, IDLE_AFTER));
        assert!(!ms, "she sits facing you, never mirrored");
        let r = atlas.get(sit);
        assert_eq!((r.src.w, r.src.h, r.ax, r.ay), (32, 28, 16, 24));
    }
}
