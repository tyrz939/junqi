//! The people in the atlas (ART.md §5, PRESENTATION.md §1.11): every person look, every variant
//! and every seat, all their frames packed at boot in all four layers (the lit tiers read the
//! normals and the true heights); and the pick of a frame for a unit this
//! tick. A unit whose sprite has no look yet keeps its stand-in (`stand_in`).

use jane_art::looks;
use jane_art::person;
use jane_art::sprite::FrameId;
use jane_core::ids::SpriteId;
use jane_data::Task;
use jane_sim::living::{ScheduleState, ScheduleWhere};

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

/// Ticks the frame after a turn shows (ART-PLAN B3): one walk frame's time.
pub const TURN_TICKS: u32 = WALK_TICKS;
/// Ticks a landing's squash shows (ART-PLAN B3).
pub const LAND_TICKS: u32 = 5;
/// Ticks a person stands still before going back to its work (ART-PLAN Q4): a pause on a round
/// is a pause, not a chore.
pub const TASK_AFTER: u32 = 30;
/// A blink (ART-PLAN Q4) falls in the standing half of a breathe's turn ([`BREATHE_TICKS`] each
/// way), every third, fourth or fifth turn by the unit's id (4, 5.3 or 6.7 s at 60 ticks a
/// second), a few ticks into it by the turn: 3 to 7 s apart.
const BLINK_TURN: u32 = 2 * BREATHE_TICKS;
/// How far into the standing half a blink may start, ticks.
const BLINK_LATE: u32 = 16;
const BLINK_SALT: u32 = 0xb11c;

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
    /// The look's work (ART-PLAN Q4).
    task: Task,
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
    /// Ticks stood still (its work starts after [`TASK_AFTER`]).
    pub still: u32,
    /// Ticks since it turned to this facing ([`TURN_TICKS`]); `u32::MAX` if it never has.
    pub turned: u32,
    /// The facing it turned from.
    pub from: Face8,
    /// Ticks since it landed (came into the zone, pulled up out of a sprint: [`LAND_TICKS`]);
    /// `u32::MAX` if it never has.
    pub landed: u32,
    /// Its work, while its schedule row is the one the work belongs to ([`People::at_work`]).
    pub task: Option<Task>,
}

impl Pose {
    /// A pose standing or walking with nothing else going on: no blow, no turn, no landing, no
    /// work.
    pub const fn plain(facing: Face8, anim: u32, tick: u32, id: u32) -> Pose {
        Pose {
            facing,
            anim,
            tick,
            dead: false,
            id,
            act: None,
            hurt: false,
            still: 0,
            turned: u32::MAX,
            from: facing,
            landed: u32::MAX,
            task: None,
        }
    }
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
                pack(atlas, &r.set.frames, anchor).into_iter().map(|(f, r, _)| (f, r)).collect();
            if ids.iter().all(|f| frames.iter().any(|(g, _)| g == f)) {
                let lit = lantern(&r).map(|set| pack(atlas, &set.frames, anchor)).unwrap_or_default();
                let task = match looks::find(r.name) {
                    Some((_, jane_data::Look::Person(p))) => p.variant(usize::from(r.variant)).task,
                    _ => Task::None,
                };
                sets.push(Set { sprite: r.sprite, variant: r.variant, seat: r.seat, frames, lit, task });
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

    /// Set `set`'s work, if the unit is at it now by its schedule row (ART-PLAN Q4): the sweeper
    /// and the milkman on their rounds, a reader standing at his mark, a knitter always (she
    /// keeps no hours). `row` is asked only of a look that works.
    pub fn at_work(&self, set: u16, row: impl FnOnce() -> Option<ScheduleState>) -> Option<Task> {
        let task = self.sets[usize::from(set)].task;
        (task != Task::None && works_at(task, row())).then_some(task)
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

    /// The frame set `set` shows for `pose`, and whether it is drawn mirrored (the three west
    /// sectors mirror the east ones). A diagonal the look has no frames for is shown from the
    /// side; a fight frame it has none for, as the plain pick.
    pub fn frame(&self, set: u16, pose: Pose) -> (RefId, bool) {
        let s = &self.sets[usize::from(set)];
        let pose = through(pose);
        let at = |f: FrameId| s.frames.iter().find(|(g, _)| *g == f).map(|(_, r)| *r);
        let side = Pose { facing: Face8::of(pose.facing.cardinal()), ..pose };
        // What a look has no frames for (a townsperson's blow or landing) falls back to the plain
        // pick.
        let plain = |p: Pose| Pose { act: None, hurt: false, landed: u32::MAX, task: None, ..p };
        let (id, r) = [pose, side, plain(pose), plain(side)]
            .into_iter()
            .map(pick)
            .find_map(|f| at(f).map(|r| (f, r)))
            .unwrap_or((FrameId::Down, s.frames[0].1));
        (r, !pose.dead && pose.facing.west() && id.faces_east())
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

/// `frames` packed into the atlas, each with where its glass glows: a frame the same as one
/// before it in the set (a blink of a look whose eyes do not show, a turn with nothing loose to
/// swing) shares that one's place rather than taking another.
fn pack(atlas: &mut Atlas, frames: &[(FrameId, jane_art::Canvas)], anchor: (i16, i16)) -> Vec<LitFrame> {
    let mut out: Vec<LitFrame> = Vec::with_capacity(frames.len());
    let mut seen: Vec<(u32, usize)> = Vec::with_capacity(frames.len());
    for (k, (f, c)) in frames.iter().enumerate() {
        let h = c.hash();
        let same = seen.iter().find(|&&(g, j)| g == h && frames[j].1 == *c).map(|&(_, j)| out[j].1);
        let r = same.unwrap_or_else(|| atlas.add_canvas(c, anchor, HEIGHT, |_, _, t| t));
        seen.push((h, k));
        out.push((*f, r, glass(c)));
    }
    out
}

/// Whether work `task` goes on in schedule row `row` (ART-PLAN Q4): sweeping and the milk on a
/// round (a patrol); reading at a mark, stood at it; knitting whatever the hour.
pub fn works_at(task: Task, row: Option<ScheduleState>) -> bool {
    use jane_data::ScheduleSlot as Slot;
    match task {
        Task::None => false,
        Task::Sweep | Task::Bottles => row.is_some_and(|r| r.slot == Slot::Patrol),
        Task::Read => row.is_some_and(|r| matches!((r.slot, r.at), (Slot::Mark(_), ScheduleWhere::Mark(_)))),
        Task::Knit => true,
    }
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
/// the breathe in turn, each unit a little out of step with the next. Eight facings: the west
/// ones are the east ones' frames, mirrored by [`People::frame`].
pub fn pick(p: Pose) -> FrameId {
    use FrameId as F;
    let p = through(p);
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
        return cycle[(1 + (p.anim - 1) / WALK_TICKS) as usize % 6];
    }
    // (A turn to face the viewer or away shows a diagonal's: [`through`].)
    let (land, turn, blink) = match p.facing {
        Face8::South => (F::DownLand, None, Some(F::DownBlink)),
        Face8::North => (F::UpLand, None, None),
        Face8::East | Face8::West => (F::SideLand, Some(F::SideTurn), Some(F::SideBlink)),
        Face8::SouthEast | Face8::SouthWest => (F::DownRightLand, Some(F::DownRightTurn), Some(F::DownRightBlink)),
        Face8::NorthEast | Face8::NorthWest => (F::UpRightLand, Some(F::UpRightTurn), None),
    };
    if p.landed < LAND_TICKS {
        land
    } else if let Some(t) = turn.filter(|_| p.turned < TURN_TICKS) {
        t
    } else if let Some(t) = p.task.filter(|_| p.still >= TASK_AFTER) {
        task_beat(t, p.tick, p.id)
    } else if let Some(b) = blink.filter(|_| blinking(p.tick, p.id)) {
        b
    } else if (p.tick + p.id * 7) / BREATHE_TICKS % 2 == 1 {
        cycle[6]
    } else {
        cycle[0]
    }
}

/// A turn to face the viewer or away (ART-PLAN B3): for its frame-time the body is seen passing
/// through the diagonal on the side it turned from, the hair and the hem trailing (that
/// diagonal's turn frame); from straight behind or in front, by the unit's id. Anything else is
/// as it was.
pub fn through(p: Pose) -> Pose {
    let busy = p.dead || p.act.is_some() || p.hurt || p.anim > 0 || p.landed < LAND_TICKS;
    if busy || p.turned >= TURN_TICKS || !matches!(p.facing, Face8::South | Face8::North) {
        return p;
    }
    let east = match p.from {
        Face8::East | Face8::SouthEast | Face8::NorthEast => true,
        Face8::West | Face8::SouthWest | Face8::NorthWest => false,
        Face8::South | Face8::North => p.id & 1 == 0,
    };
    let facing = match (p.facing == Face8::South, east) {
        (true, true) => Face8::SouthEast,
        (true, false) => Face8::SouthWest,
        (false, true) => Face8::NorthEast,
        (false, false) => Face8::NorthWest,
    };
    Pose { facing, ..p }
}

/// Whether unit `id` has its eyes shut at `tick` (ART-PLAN Q4): 3 or 4 ticks every 3 to 7
/// seconds, in the standing half of its breathe (so the blink frame is the standing frame with
/// its eyes shut), out of step with its neighbours by its id.
pub fn blinking(tick: u32, id: u32) -> bool {
    let t = tick.wrapping_add(id.wrapping_mul(7));
    let (turn, into) = (t / BLINK_TURN, t % BLINK_TURN);
    let every = 3 + jane_core::hash::hash2(id as i32, 0, BLINK_SALT) % 3;
    let phase = jane_core::hash::hash2(id as i32, 1, BLINK_SALT) % every;
    if (turn + phase) % every != 0 {
        return false;
    }
    let h = jane_core::hash::hash2(id as i32, turn as i32, BLINK_SALT);
    let (start, len) = (h % (BLINK_LATE + 1), 3 + (h >> 16) % 2);
    (start..start + len).contains(&into)
}

/// The beat of work `task` at `tick` (ART-PLAN Q4), each unit out of step with the next: the
/// broom's four strokes at a brisk walk's pace; a page read a while, then turned over in three
/// quick beats; a bottle carried, set down on the step and left there; the needles and the
/// chair's rock.
pub fn task_beat(task: Task, tick: u32, id: u32) -> FrameId {
    use FrameId as F;
    let down = [F::Task1, F::Task2, F::Task3, F::Task4];
    let (ids, ticks) = match task {
        Task::Sweep => ([F::TaskSide1, F::TaskSide2, F::TaskSide3, F::TaskSide4], [8, 8, 8, 8]),
        Task::Read => (down, [150, 6, 6, 10]),
        Task::Bottles => (down, [60, 10, 16, 70]),
        Task::Knit | Task::None => (down, [20, 20, 20, 20]),
    };
    let mut k = tick.wrapping_add(id.wrapping_mul(13)) % ticks.iter().sum::<u32>();
    for (f, n) in ids.into_iter().zip(ticks) {
        if k < n {
            return f;
        }
        k -= n;
    }
    ids[0]
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
    use std::collections::BTreeSet;

    fn pose(facing: Facing, anim: u32, tick: u32) -> Pose {
        Pose::plain(Face8::of(facing), anim, tick, 0)
    }

    /// The ticks unit `id` starts a blink in, standing facing the viewer over `secs` seconds,
    /// and each blink's length.
    fn blinks(id: u32, secs: u32) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        let mut open = true;
        for t in 0..secs * 60 {
            let shut = pick(Pose { id, ..pose(Facing::South, 0, t) }) == FrameId::DownBlink;
            match (shut, open) {
                (true, true) => out.push((t, 1)),
                (true, false) => out.last_mut().unwrap().1 += 1,
                _ => {}
            }
            open = !shut;
        }
        out
    }

    #[test]
    fn a_blink_is_three_or_four_ticks_every_three_to_seven_seconds_out_of_step() {
        let mut starts = Vec::new();
        for id in [0, 1, 2, 7, 40, 41, 1000, 65_537] {
            let b = blinks(id, 120);
            assert!(b.len() >= 120 / 7, "id {id}: {} blinks in two minutes", b.len());
            for &(_, n) in &b {
                assert!((3..=4).contains(&n), "id {id}: a blink of {n} ticks");
            }
            for w in b.windows(2) {
                let gap = w[1].0 - w[0].0;
                assert!((180..=420).contains(&gap), "id {id}: blinks {gap} ticks apart");
            }
            starts.push(b.iter().map(|&(t, _)| t).collect::<Vec<_>>());
        }
        // Neighbours blink at their own times.
        for (i, a) in starts.iter().enumerate() {
            for b in &starts[i + 1..] {
                let together = a.iter().filter(|t| b.contains(t)).count();
                assert!(together * 4 < a.len(), "two units blink together {together} of {} times", a.len());
            }
        }
        // Never walking, dead, struck or hurt; on the side and three quarters too; never from
        // behind (no eyes show).
        for t in 0..60 * 60 {
            for f in [Facing::South, Facing::East, Facing::North] {
                let walk = pick(pose(f, 1 + t % 36, t));
                assert!(!matches!(walk, FrameId::DownBlink | FrameId::SideBlink | FrameId::DownRightBlink));
                assert_ne!(pick(Pose { hurt: true, ..pose(f, 0, t) }), FrameId::DownBlink);
            }
        }
        let shut = |f: Face8| {
            (0..60 * 20).map(|t| pick(Pose { facing: f, ..pose(Facing::South, 0, t) })).collect::<BTreeSet<_>>()
        };
        assert!(shut(Face8::West).contains(&FrameId::SideBlink));
        assert!(shut(Face8::SouthWest).contains(&FrameId::DownRightBlink));
        assert!(!shut(Face8::North).iter().any(|f| f.name().contains("blink")));
    }

    #[test]
    fn work_goes_on_by_the_schedule_row_and_only_standing_still() {
        use jane_data::ScheduleSlot as Slot;
        use jane_sim::living::{ScheduleState, ScheduleWhere};
        let mark = jane_core::NameId(3);
        let row = |slot, at| Some(ScheduleState { slot, at });
        let on_round = row(Slot::Patrol, ScheduleWhere::Walking(jane_core::Cell::new(4, 4)));
        let at_mark = row(Slot::Mark(mark), ScheduleWhere::Mark(mark));
        let to_mark = row(Slot::Mark(mark), ScheduleWhere::Walking(jane_core::Cell::new(4, 4)));
        // The sweeper and the milkman on their rounds, not at a mark or indoors.
        for t in [Task::Sweep, Task::Bottles] {
            assert!(works_at(t, on_round));
            assert!(!works_at(t, at_mark) && !works_at(t, None));
        }
        // Mr Cobb at his mark, not on his way to it; his double with no hours never.
        assert!(works_at(Task::Read, at_mark));
        assert!(!works_at(Task::Read, to_mark) && !works_at(Task::Read, on_round) && !works_at(Task::Read, None));
        // Mrs Wakes keeps no hours: she knits.
        assert!(works_at(Task::Knit, None));
        assert!(!works_at(Task::None, on_round));
        // The beats play once it has stood a while, never walking; a turn or a landing first.
        let busy = |still, anim, tick| Pose { task: Some(Task::Sweep), still, ..pose(Facing::East, anim, tick) };
        assert!(pick(busy(TASK_AFTER, 0, 0)).name().starts_with("task_side"));
        assert!(!pick(busy(TASK_AFTER - 1, 0, 0)).name().starts_with("task"));
        assert_eq!(pick(busy(TASK_AFTER, 3, 0)), FrameId::Side1);
        assert_eq!(pick(Pose { turned: 0, ..busy(TASK_AFTER, 0, 0) }), FrameId::SideTurn);
        let beats: BTreeSet<FrameId> = (0..400).map(|t| task_beat(Task::Read, t, 5)).collect();
        assert_eq!(beats.len(), 4, "a page read, then turned in three beats");
        // The sweeper's look has its strokes; Mr Cobb's his paper; the grocer has no work.
        let mut atlas = Atlas::new();
        let people = People::build(&mut atlas);
        let set = |name| people.set(jane_art::looks::find(name).unwrap().0, 0, 0).unwrap();
        assert_eq!(people.at_work(set("town_sweeper"), || on_round), Some(Task::Sweep));
        assert_eq!(people.at_work(set("town_sweeper"), || None), None);
        assert_eq!(people.at_work(set("town_drinker"), || at_mark), Some(Task::Read));
        assert_eq!(people.at_work(set("tale_wakes"), || None), Some(Task::Knit));
        assert_eq!(people.at_work(set("town_grocer"), || at_mark), None);
        let sweeping = Pose { facing: Face8::West, ..busy(TASK_AFTER, 0, 0) };
        let (r, mirror) = people.frame(set("town_sweeper"), sweeping);
        assert!(mirror, "she sweeps westward too");
        assert_ne!(r, people.frame(set("town_sweeper"), pose(Facing::West, 0, 0)).0);
    }

    #[test]
    fn a_turn_swings_a_frame_and_a_landing_squashes_hers() {
        assert_eq!(pick(Pose { turned: 0, ..pose(Facing::East, 0, 0) }), FrameId::SideTurn);
        // Turning to face us from the west, she passes through the south-west, mirrored.
        let to_us = Pose { turned: 0, from: Face8::West, ..pose(Facing::South, 0, 0) };
        assert_eq!(pick(to_us), FrameId::DownRightTurn);
        assert_eq!(through(to_us).facing, Face8::SouthWest);
        let away = Pose { turned: TURN_TICKS - 1, from: Face8::East, ..pose(Facing::North, 0, 0) };
        assert_eq!(pick(away), FrameId::UpRightTurn);
        assert_eq!(through(away).facing, Face8::NorthEast);
        assert_eq!(pick(Pose { turned: TURN_TICKS, ..away }), FrameId::Up);
        assert_eq!(pick(Pose { turned: 0, ..pose(Facing::East, 4, 0) }), FrameId::Side1, "not mid-stride");
        assert_eq!(pick(Pose { landed: 0, turned: 0, ..pose(Facing::West, 0, 0) }), FrameId::SideLand);
        assert_eq!(pick(Pose { landed: LAND_TICKS, ..pose(Facing::North, 0, BREATHE_TICKS) }), FrameId::UpB);
        let mut atlas = Atlas::new();
        let people = People::build(&mut atlas);
        let jane = people.set(jane_art::looks::find("jane").unwrap().0, 0, 0).unwrap();
        let grocer = people.set(jane_art::looks::find("town_grocer").unwrap().0, 0, 0).unwrap();
        let stand = pose(Facing::South, 0, BREATHE_TICKS * 2);
        let landed = Pose { landed: 0, ..stand };
        assert_ne!(people.frame(jane, landed), people.frame(jane, stand), "she lands");
        assert_eq!(people.frame(grocer, landed), people.frame(grocer, stand), "a townsman has no landing");
        // Her coat's hem is still out after a turn, and she is seen half turned.
        let turned = people.frame(jane, Pose { turned: 0, from: Face8::West, ..stand });
        assert!(turned.1, "through the south-west");
        assert_ne!(turned.0, people.frame(jane, Pose { facing: Face8::SouthWest, ..stand }).0);
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
    fn her_lantern_is_in_her_hand_in_all_eight_facings() {
        let mut atlas = Atlas::with_layers(true);
        let people = People::build(&mut atlas);
        let jane = jane_art::looks::find("jane").unwrap().0;
        let set = people.set(jane, 0, 0).unwrap();
        for f in Face8::ALL {
            for anim in [0, 1, 13] {
                let p = Pose { facing: f, ..pose(Facing::South, anim, 0) };
                let h = people.holding_lantern(set, p, &atlas).unwrap_or_else(|| panic!("{f:?} {anim}: no lantern"));
                assert_eq!(h.mirror, f.west(), "{f:?}");
                // Its glass hangs from her hand: beside her, under her shoulders.
                let (dx, up) = h.glass;
                assert!(dx.abs() <= 14 && (3..=22).contains(&up), "{f:?} {anim}: glass at {:?}", h.glass);
            }
        }
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
