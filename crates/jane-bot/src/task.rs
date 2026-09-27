//! One thing being done, a frame at a time: walk somewhere, use a prop, talk to someone, pick
//! something up, make something at the bench, cast a world verb. A task answers each frame with
//! an [`Act`], or says it is done or why it failed. What a bot remembers across tasks (what it
//! has used, whom it has talked to, where it saw things) is its [`Ctx`].

use std::collections::{BTreeMap, BTreeSet};

use jane_core::num::CELL_FX;
use jane_core::{Angle, Fx, ItemId, SpellId, Vec2, ZoneId};
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::ids::{DropId, PropId, UnitId};
use jane_sim::interact::FocusRef;
use jane_sim::{Command, Event, InputFrame, View};

use crate::nav::{Go, Nav, dist, walkable};
use crate::sense::{prop_centre, prop_rect};
use crate::{Act, Model};

/// A frame's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Act(Act),
    Done,
    Failed(String),
}

/// What a bot carries between tasks.
#[derive(Debug)]
pub struct Ctx {
    pub model: Model,
    pub nav: Nav,
    pub fight: crate::fight::Fight,
    pub talk: crate::talk::Talk,
    /// Props used, by zone, with the frame.
    pub used: BTreeMap<(ZoneId, PropId), u32>,
    /// Units talked to, with the quest log's shape ([`Ctx::log_size`]) at the time.
    pub talked: BTreeMap<(ZoneId, UnitId), u32>,
    /// Where each talking unit def was last seen.
    pub seen: BTreeMap<jane_core::UnitDefId, (ZoneId, Vec2)>,
    /// Talking unit defs talked to, with the quest log's shape at the time.
    pub talked_def: BTreeMap<jane_core::UnitDefId, u32>,
    /// Zones she has stood in.
    pub visited: BTreeSet<ZoneId>,
    /// The last step's toasts for her.
    pub toasts: Vec<ToastKind>,
    /// Units she hurt or was hurt by lately (for the kill log).
    pub foes: BTreeSet<UnitId>,
    pub zone: Option<ZoneId>,
    /// Where she came into this zone (the way back out).
    pub came_in: Option<Vec2>,
    pub frames: u32,
    /// What she noticed about each zone's props the last time she stood in it.
    pub notes: BTreeMap<ZoneId, Vec<PropNote>>,
    /// Enemies she has seen standing, by def: where each was last seen (forgotten once seen down).
    pub seen_foes: BTreeMap<jane_core::UnitDefId, BTreeMap<UnitId, (ZoneId, Vec2)>>,
    /// Which of the three endings she chooses at Yours to Say, if told (the choice policy).
    pub ending: Option<crate::Ending>,
    /// She wants the night (or the day) slept away at the next bed: waiting for a Sunday.
    pub sleep: bool,
    /// The day she signalled the Sunday train at the name board.
    pub signalled: Option<u32>,
    /// Butterfly Forest's tactic (`tactics::forest`).
    pub forest: crate::tactics::forest::Forest,
}

/// What a prop was seen to do: enough to go back for it from another zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropNote {
    pub id: PropId,
    pub at: Vec2,
    pub hands_in: Vec<jane_core::QuestId>,
    pub gives: Vec<jane_core::QuestId>,
    /// Places (content names) it marks visited.
    pub places: Vec<jane_core::NameId>,
    /// What it held (a chest's loot, while unopened).
    pub loot: Vec<ItemId>,
    pub bench: bool,
    /// A bed or a fire.
    pub rest: bool,
    pub door: Option<ZoneId>,
    /// Which ending it plays, when it is one of the three (`the_end`).
    pub ending: Option<u8>,
    /// It signals the Sunday train.
    pub signals: bool,
    /// A bed that sleeps the night away.
    pub sleeps: bool,
}

impl PropNote {
    pub fn of(v: &View<'_>, p: &jane_sim::Prop) -> PropNote {
        use jane_core::action::Action;
        let mut n = PropNote {
            id: p.id,
            at: prop_centre(p),
            hands_in: Vec::new(),
            gives: Vec::new(),
            places: Vec::new(),
            loot: Vec::new(),
            bench: jane_data::catalog().story.prop(p.def).bench,
            rest: jane_data::catalog().story.prop(p.def).rest,
            door: crate::sense::door_of(v, p).map(|d| d.zone),
            ending: None,
            signals: false,
            sleeps: false,
        };
        crate::sense::visit_prop(v, p, &mut |a| match *a {
            Action::HandIn(q) => n.hands_in.push(q),
            Action::Quest(q) => n.gives.push(q),
            Action::Location(jane_core::Key::Name(name)) => n.places.push(name),
            Action::Rest { until: Some(_), .. } => n.sleeps = true,
            _ => {
                if let Some(e) = crate::sense::sets_the_end(a) {
                    n.ending = Some(e);
                }
                n.signals |= crate::sense::signals_train(a);
            }
        });
        if !p.used {
            if let Some(s) = v.prop_spawn(p) {
                n.loot.extend(s.loot.iter().map(|s| s.item));
            }
        }
        n
    }
}

impl Ctx {
    pub fn new(model: Model) -> Ctx {
        Ctx {
            model,
            nav: Nav::keeping_to_roads(model == Model::Reader),
            fight: crate::fight::Fight::default(),
            talk: crate::talk::Talk::default(),
            used: BTreeMap::new(),
            talked: BTreeMap::new(),
            seen: BTreeMap::new(),
            talked_def: BTreeMap::new(),
            visited: BTreeSet::new(),
            toasts: Vec::new(),
            foes: BTreeSet::new(),
            zone: None,
            came_in: None,
            frames: 0,
            notes: BTreeMap::new(),
            seen_foes: BTreeMap::new(),
            ending: None,
            sleep: false,
            signalled: None,
            forest: crate::tactics::forest::Forest::default(),
        }
    }

    /// Was `id` someone she fought?
    pub fn fighting(&self, id: UnitId) -> bool {
        self.foes.contains(&id)
    }

    /// Read the last step's events and what the view shows.
    pub fn observe(&mut self, v: &View<'_>, events: &[Event]) {
        self.frames += 1;
        let me = v.me().unit;
        self.toasts.clear();
        for e in events {
            if e.to.is_some_and(|s| s != v.seat()) {
                continue;
            }
            match e.kind {
                EventKind::Toast(t) => self.toasts.push(t),
                EventKind::Damage { unit, from: Some(f), .. } if unit == me => {
                    self.foes.insert(f);
                }
                EventKind::Damage { unit, from: Some(f), .. } if f == me => {
                    self.foes.insert(unit);
                }
                _ => {}
            }
        }
        if self.zone != Some(v.zone()) {
            self.zone = Some(v.zone());
            self.visited.insert(v.zone());
            self.came_in = Some(v.body().pos);
            self.nav.reset();
            self.fight.target = None;
            self.fight.hunt = None;
            let notes = v
                .props()
                .filter(|p| {
                    let d = jane_data::catalog().story.prop(p.def);
                    d.bench
                        || d.rest
                        || v.prop_spawn(p).is_some_and(|s| {
                            s.talk.is_some() || s.use_list.is_some() || s.to.is_some() || !s.loot.is_empty()
                        })
                })
                .map(|p| PropNote::of(v, p))
                .collect();
            self.notes.insert(v.zone(), notes);
        }
        if self.frames % 30 == 1 {
            for u in crate::sense::talkers(v) {
                self.seen.insert(u.def, (v.zone(), u.pos));
            }
            for u in v.units_in(crate::sense::everywhere(v)).map(|u| u.unit).filter(|u| crate::sense::hostile(u)) {
                let seen = self.seen_foes.entry(u.def).or_default();
                if u.alive {
                    seen.insert(u.id, (v.zone(), u.pos));
                } else {
                    seen.remove(&u.id);
                }
            }
        }
    }

    /// The quest log's shape (done, active): changes whenever a quest is given or handed in.
    pub fn log_size(v: &View<'_>) -> u32 {
        (v.quests_done().len() as u32) << 8 | v.quests().count() as u32
    }

    pub fn sprint(&self) -> bool {
        self.model.sprints()
    }
}

/// One thing being done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task {
    /// Walk to within `near` of a point.
    Walk { to: Vec2, near: Fx },
    /// Walk up to a prop and press USE on it (and on nothing else).
    Use(UseProp),
    /// Walk up to a unit and talk.
    Talk { unit: UnitId, t: u32, pressed: bool },
    /// Walk onto a stack on the ground and take it.
    Pickup { drop: DropId, t: u32 },
    /// At a bench: put the inputs in the craft row, take what it makes.
    Craft { inputs: Vec<ItemId>, stage: u8, t: u32 },
    /// Beside a prop, cast a world verb at it (Repair, Grow).
    WorldCast { spell: SpellId, prop: PropId, stage: u8, t: u32, side: u8, sides: Vec<(Vec2, Angle)> },
    /// Walk to `from`, then cast a bolt at a point (a torch, a brazier).
    Aim { spell: SpellId, from: Vec2, at: Vec2, t: u32 },
    /// Put a unit down (the fight is [`crate::fight`]'s).
    Hunt(UnitId),
    /// Push a prop along a line of cells: stand behind it, lean in (hold USE and the stick), and
    /// again for the next cell; `path` is the cells its origin passes through, in order.
    Push(Push),
    /// Stand still.
    Wait(u32),
    /// What the Burial asks beyond the crawl's order (`tactics::burial`).
    Burial(crate::tactics::burial::Job),
}

/// Walking up to a prop, then pressing USE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseProp {
    pub prop: PropId,
    /// Which side of it is being tried (0..4).
    pub side: u8,
    /// 0 walk, 1 face, 2 press, 3 after.
    pub stage: u8,
    pub t: u32,
    /// Press it this many times (a lock, then the door; a chest the pantry fills again).
    pub presses: u8,
    /// The points beside it to stand at, nearest first when the walk began.
    pub sides: Vec<(Vec2, Angle)>,
}

impl Task {
    pub fn cast(spell: SpellId, prop: PropId) -> Task {
        Task::WorldCast { spell, prop, stage: 0, t: 0, side: 0, sides: Vec::new() }
    }

    pub fn talk(unit: UnitId) -> Task {
        Task::Talk { unit, t: 0, pressed: false }
    }
}

impl UseProp {
    pub fn new(prop: PropId) -> UseProp {
        UseProp { prop, side: 0, stage: 0, t: 0, presses: 1, sides: Vec::new() }
    }
}

/// The four points just outside a prop's sides, nearest to `from` first.
pub fn sides(v: &View<'_>, p: &jane_sim::Prop, from: Vec2) -> Vec<(Vec2, Angle)> {
    let r = prop_rect(p);
    let c = prop_centre(p);
    let out = 5 * 256;
    let mut s = vec![
        (Vec2::new(c.x, Fx(r.bottom() * CELL_FX + out)), Angle::NORTH),
        (Vec2::new(c.x, Fx(r.y * CELL_FX - out)), Angle::SOUTH),
        (Vec2::new(Fx(r.x * CELL_FX - out), c.y), Angle::EAST),
        (Vec2::new(Fx(r.right() * CELL_FX + out), c.y), Angle::WEST),
    ];
    s.retain(|(at, _)| {
        let (x, y) = at.cell();
        walkable(v, x, y)
    });
    s.sort_by_key(|(at, f)| (dist(*at, from), f.0));
    s
}

/// A small push of the stick so her facing turns that way (below the move deadzone is no turn,
/// so a nudge moves her a hair).
pub fn nudge(dir: Angle) -> InputFrame {
    InputFrame { mv_dir: dir, mv_mag: 20, ..InputFrame::IDLE }
}

fn walk(cx: &mut Ctx, v: &View<'_>, to: Vec2, near: Fx) -> Option<Status> {
    match cx.nav.go(v, to, near, cx.sprint()) {
        Go::Walk(f) => Some(Status::Act(Act::hold(f))),
        Go::Arrived => None,
        Go::NoWay => Some(Status::Failed(format!(
            "no way to {},{} from {:?}: {}",
            to.x.0 / CELL_FX,
            to.y.0 / CELL_FX,
            v.body().pos.cell(),
            cx.nav.why
        ))),
    }
}

impl Task {
    /// This frame of the task.
    pub fn tick(&mut self, v: &View<'_>, cx: &mut Ctx) -> Status {
        match self {
            Task::Walk { to, near } => walk(cx, v, *to, *near).unwrap_or(Status::Done),
            Task::Use(u) => use_prop(u, v, cx),
            Task::Talk { unit, t, pressed } => {
                *t += 1;
                if *pressed || v.dialogue().is_some() {
                    return Status::Done;
                }
                if *t > 60 * 90 {
                    return Status::Failed("could not reach them to talk".into());
                }
                let Some(u) = v.unit(*unit) else { return Status::Failed("they are not here".into()) };
                if v.focus().is_some_and(|f| f.target == FocusRef::Unit(*unit)) {
                    *pressed = true;
                    return Status::Act(Act::press(Command::Use));
                }
                if let Some(s) = walk(cx, v, u.pos, Fx::from_px(12)) {
                    return s;
                }
                // Turn to them; if that is not enough, step closer.
                let dir = jane_core::angle::iatan2(u.pos.y.0 - v.body().pos.y.0, u.pos.x.0 - v.body().pos.x.0);
                Status::Act(Act::hold(nudge(dir)))
            }
            Task::Pickup { drop, t } => {
                *t += 1;
                let Some(d) = v.drops().iter().find(|d| d.id == *drop) else { return Status::Done };
                if *t > 60 * 60 {
                    return Status::Failed("could not reach a drop".into());
                }
                if v.focus().is_some_and(|f| f.target == FocusRef::Drop(*drop)) {
                    return Status::Act(Act::press(Command::Use));
                }
                walk(cx, v, d.pos, Fx::from_px(4))
                    .unwrap_or_else(|| Status::Act(Act::hold(crate::nav::stick(v.body().pos, d.pos, false))))
            }
            Task::Craft { inputs, stage, t } => craft(inputs, stage, t, v),
            Task::WorldCast { spell, prop, stage, t, side, sides: near } => {
                *t += 1;
                let Some(p) = v.prop(*prop).filter(|p| !p.hidden) else { return Status::Done };
                if *t > 60 * 90 {
                    return Status::Failed("could not cast at it".into());
                }
                match *stage {
                    0 => {
                        if near.is_empty() {
                            *near = sides(v, p, v.body().pos);
                        }
                        let Some(&(at, face)) = near.get(*side as usize) else {
                            let tried: Vec<(i32, i32)> = near.iter().map(|(a, _)| a.cell()).collect();
                            return Status::Failed(format!("no side to stand at (tried {tried:?})"));
                        };
                        match cx.nav.go(v, at, Fx::from_px(3), cx.sprint()) {
                            Go::Walk(f) => Status::Act(Act::hold(f)),
                            Go::NoWay => {
                                *side += 1;
                                cx.nav.reset();
                                Status::Act(Act::idle())
                            }
                            Go::Arrived => {
                                *stage = 1;
                                Status::Act(Act::hold(nudge(face)))
                            }
                        }
                    }
                    1 => {
                        // Wait out a cooldown (Repair's is ten seconds) rather than waste the press.
                        if !crate::fight::ready(v.body(), *spell, v.tick()) {
                            return Status::Act(Act::idle());
                        }
                        *stage = 2;
                        *t = 0;
                        Status::Act(Act::press(Command::Cast { spell: *spell, on: None }))
                    }
                    _ => {
                        if p.used || p.on {
                            Status::Done
                        } else if *t > 20 {
                            Status::Failed("the verb did not take".into())
                        } else {
                            Status::Act(Act::idle())
                        }
                    }
                }
            }
            Task::Aim { spell, from, at, t } => {
                // Waiting frames are counted above `WAITED` (a bolt pressed unready is not thrown).
                const WAITED: u32 = 1 << 16;
                if *t == 0 || *t >= WAITED {
                    if *t == 0 {
                        if let Some(s) = walk(cx, v, *from, Fx::from_px(2)) {
                            return s;
                        }
                    }
                    // Wait out a cooldown, or the mana for it, ten seconds at most.
                    if !crate::fight::ready(v.body(), *spell, v.tick()) {
                        *t = (*t).max(WAITED) + 1;
                        if *t > WAITED + 600 {
                            return Status::Failed("the bolt was never ready".into());
                        }
                        return Status::Act(Act::idle());
                    }
                    *t = 0;
                }
                *t += 1;
                let me = v.body();
                let dir = jane_core::angle::iatan2(at.y.0 - me.pos.y.0, at.x.0 - me.pos.x.0);
                if *t == 1 {
                    return Status::Act(Act {
                        frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
                        cmds: vec![Command::Cast { spell: *spell, on: None }],
                    });
                }
                if *t > 45 { Status::Done } else { Status::Act(Act::idle()) }
            }
            Task::Hunt(id) => {
                cx.fight.hunt = Some(*id);
                match crate::fight::engage(v, cx, *id) {
                    Some(a) => Status::Act(a),
                    None => Status::Done,
                }
            }
            Task::Push(p) => push(p, v, cx),
            Task::Burial(j) => j.tick(v, cx),
            Task::Wait(n) => {
                if *n == 0 {
                    return Status::Done;
                }
                *n -= 1;
                Status::Act(Act::idle())
            }
        }
    }
}

fn use_prop(u: &mut UseProp, v: &View<'_>, cx: &mut Ctx) -> Status {
    u.t += 1;
    if u.t > 60 * 60 * 8 {
        return Status::Failed("took too long to reach it".into());
    }
    let Some(p) = v.prop(u.prop).filter(|p| !p.hidden) else {
        // Gone (a door that took her elsewhere, a herb gathered).
        return if u.stage >= 2 { Status::Done } else { Status::Failed("it is not here".into()) };
    };
    let me = v.body().pos;
    match u.stage {
        0 => {
            // Already in reach and focused: press.
            if v.focus().is_some_and(|f| f.target == FocusRef::Prop(u.prop)) {
                u.stage = 2;
                return use_prop(u, v, cx);
            }
            if u.sides.is_empty() || u.side == 0 && u.t == 1 {
                u.sides = sides(v, p, me);
            }
            let Some(&(at, _)) = u.sides.get(u.side as usize) else {
                return Status::Failed(format!(
                    "no side of it to stand at (tried {}; the last: {})",
                    u.side, cx.nav.why
                ));
            };
            match cx.nav.go(v, at, Fx::from_px(3), cx.sprint()) {
                Go::Walk(f) => Status::Act(Act::hold(f)),
                Go::Arrived => {
                    u.stage = 1;
                    use_prop(u, v, cx)
                }
                Go::NoWay => {
                    u.side += 1;
                    cx.nav.reset();
                    Status::Act(Act::idle())
                }
            }
        }
        1 => {
            let c = prop_centre(p);
            let dx = c.x.0 - me.x.0;
            let dy = c.y.0 - me.y.0;
            let face = if dx.abs() >= dy.abs() {
                if dx >= 0 { Angle::EAST } else { Angle::WEST }
            } else if dy >= 0 {
                Angle::SOUTH
            } else {
                Angle::NORTH
            };
            u.stage = 2;
            Status::Act(Act::hold(nudge(face)))
        }
        // A gate unlocked is open: nothing more to press.
        2 if !p.locked
            && jane_data::catalog().story.prop(p.def).gate
            && u.t > 2
            && cx.used.contains_key(&(v.zone(), u.prop)) =>
        {
            Status::Done
        }
        2 => match v.focus().map(|f| f.target) {
            Some(FocusRef::Prop(id)) if id == u.prop => {
                u.presses = u.presses.saturating_sub(1);
                if u.presses == 0 {
                    u.stage = 3;
                }
                cx.used.insert((v.zone(), u.prop), v.frame());
                Status::Act(Act::press(Command::Use))
            }
            // A stack lying in the way: take it first.
            Some(FocusRef::Drop(_)) => Status::Act(Act::press(Command::Use)),
            _ => {
                // Something else is nearer, or she is not facing it: the next side.
                u.side += 1;
                u.stage = 0;
                cx.nav.reset();
                if u.side >= 4 {
                    return Status::Failed("USE beside it never acts on it".into());
                }
                Status::Act(Act::idle())
            }
        },
        _ => {
            // What the press came to is in this frame's toasts.
            for t in &cx.toasts {
                match *t {
                    ToastKind::Locked { prop } if prop == u.prop => return Status::Failed("it is locked".into()),
                    ToastKind::NightLock(_) => return Status::Failed("it is not answered after dark".into()),
                    ToastKind::InventoryFull => return Status::Failed("the bag is full".into()),
                    _ => {}
                }
            }
            Status::Done
        }
    }
}

/// Pushing a prop cell by cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Push {
    pub prop: PropId,
    pub path: Vec<(i32, i32)>,
    /// The next cell of `path`.
    pub at: usize,
    pub t: u32,
    /// Frames leaning on the current cell.
    pub lean: u32,
}

fn push(p: &mut Push, v: &View<'_>, cx: &mut Ctx) -> Status {
    p.t += 1;
    if p.t > 60 * 120 {
        return Status::Failed("the push took too long".into());
    }
    let Some(prop) = v.prop(p.prop) else { return Status::Failed("it is gone".into()) };
    let here = (i32::from(prop.cell.x), i32::from(prop.cell.y));
    // Past cells already reached.
    while p.path.get(p.at) == Some(&here) {
        p.at += 1;
        p.lean = 0;
    }
    let Some(&next) = p.path.get(p.at) else { return Status::Done };
    let (dx, dy) = (next.0 - here.0, next.1 - here.1);
    if dx.abs() + dy.abs() != 1 {
        return Status::Failed(format!("the push path jumps from {here:?} to {next:?}"));
    }
    // Stand in the cell behind it, facing along the push.
    let def = jane_data::catalog().story.prop(prop.def);
    let (bx, by) = crate::crawl::behind(here, (dx, dy), i32::from(def.w), i32::from(def.h));
    let behind = Vec2::centre(bx, by);
    let dir = match (dx, dy) {
        (1, _) => Angle::EAST,
        (-1, _) => Angle::WEST,
        (_, 1) => Angle::SOUTH,
        _ => Angle::NORTH,
    };
    let me = v.body();
    if dist(me.pos, behind) > 2 * 256 && p.lean == 0 {
        return match cx.nav.go(v, behind, Fx::from_px(1), false) {
            Go::Walk(f) => Status::Act(Act::hold(f)),
            Go::Arrived => Status::Act(Act::hold(nudge(dir))),
            Go::NoWay => Status::Failed(format!("cannot stand behind it at {:?}", (bx, by))),
        };
    }
    p.lean += 1;
    if p.lean > 150 {
        return Status::Failed(format!("it will not move from {here:?} toward {next:?}"));
    }
    if cx.toasts.contains(&ToastKind::TooTired) || me.energy_locked {
        // Arms need a rest.
        return Status::Act(Act::hold(nudge(dir)));
    }
    Status::Act(Act::hold(InputFrame { use_held: true, ..InputFrame::walk(dir) }))
}

fn craft(inputs: &[ItemId], stage: &mut u8, t: &mut u32, v: &View<'_>) -> Status {
    *t += 1;
    if *t > 30 {
        return Status::Failed("the bench would not make it".into());
    }
    if !v.near_bench() {
        return Status::Failed("not at a bench".into());
    }
    let i = *stage as usize;
    if i < inputs.len() {
        let bag = &v.me().bag;
        let Some(slot) = bag.iter().position(|s| s.is_some_and(|s| s.item == inputs[i])) else {
            return Status::Failed("an input is not in the bag".into());
        };
        *stage += 1;
        return Status::Act(Act::press(Command::CraftPut { bag: slot as u8, slot: i as u8 }));
    }
    if i == inputs.len() {
        *stage += 1;
        return Status::Act(Act::press(Command::CraftTake));
    }
    Status::Done
}
