//! The Burial Chamber (DUNGEONS.md §3.5): see `mod.rs`.
//!
//! What the Burial asks beyond the crawl's order, each hooked into the crawl or its fight in a
//! line or two (`crawl.rs`, look for `tactics::burial`):
//!
//! - **The fire** (`offers`): the vigil fire is sat at as soon as she can reach it, so a fall
//!   wakes her beside it and not at the Halt, and any fire again when she is hurt.
//! - **Feed the small snakes** (the dog: "The small snakes down there cannot be fought. They can
//!   be fed."; `feeds`, `Feed`). A row with a `bait` is never fought (`fight::fightable`). Its
//!   spit kills from forty cells, so she keeps out of its notice (`keep_off`), throws the bait
//!   three cells ahead of her where it can smell it, and walks off out of its sight while it
//!   crawls over and eats. Everything else in the room waits (`cuts_in`).
//! - **The fight** (`fight`): everything here out-hits her swing, so it is fought with the bolt
//!   that gets through best, from range, stepping out of what is thrown at her and backing off
//!   from what walks; what is rooted and stands up again once she has gone (the statues, the
//!   cactus) is walked past, not fought (`let_be`); the snake is kept to its room, or it goes
//!   home whole.
//! - **What comes first** (`not_yet`, `offers`): a corner's keeper before anything else in its
//!   room; the garden's flower before the rest of the garden (the scroll is under it); and not
//!   the glasshouse before the scroll has taught her Fire.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use jane_core::action::Facing;
use jane_core::num::CELL_FX;
use jane_core::{Angle, Fx, ItemId, Vec2};
use jane_sim::ids::UnitId;
use jane_sim::state::CombatState;
use jane_sim::{Command, InputFrame, Unit, View};

use crate::Act;
use crate::crawl::{Reach, Try};
use crate::nav::{Go, dist};
use crate::sense::{self, holds};
use crate::task::{Ctx, Status, Task, nudge};

/// A baited creature notices her within its aggro, body to body (`ai::seen`): at or under this,
/// centre to centre and in its sight, it has her.
fn notice(v: &View<'_>, u: &Unit) -> i64 {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(u.def);
    i64::from(d.aggro.0) + i64::from(d.bounds.0) + i64::from(cat.combat.unit(v.body().def).bounds.0)
}

/// It goes for a bait it can see within its nose, centre to the bait: twice as far as it
/// notices anyone (`ai::seek_bait`; the dog: it "cannot be fought", it can be fed).
fn nose(u: &Unit) -> i64 {
    i64::from(jane_data::catalog().combat.unit(u.def).aggro.0) * jane_sim::tuning::BAIT_NOSE_TIMES
}

/// Would `u` notice her standing at `at` (with `margin` to spare)?
fn sees(v: &View<'_>, u: &Unit, at: Vec2, margin: i64) -> bool {
    dist(u.pos, at) <= notice(v, u) + margin && v.sight(u.pos, at)
}

/// Would any other awake hostile here notice her at `at`?
fn watched(v: &View<'_>, not: UnitId, at: Vec2) -> bool {
    let cat = jane_data::catalog();
    let me = i64::from(cat.combat.unit(v.body().def).bounds.0);
    sense::enemies(v).into_iter().filter(|u| u.id != not && u.alive).any(|u| {
        let d = cat.combat.unit(u.def);
        d.aggro.0 > 0
            && dist(u.pos, at) <= i64::from(d.aggro.0) + i64::from(d.bounds.0) + me + i64::from(CELL_FX)
            && v.sight(u.pos, at)
    })
}

/// Can `u` walk straight to `to` with its whole body (its walker catches on a corner a line of
/// sight goes past): the line from its middle and the lines either side of it, a body's width
/// apart, are clear, and so is the ground about the bait.
fn clear_walk(v: &View<'_>, u: &Unit, to: Vec2) -> bool {
    let b = jane_data::catalog().combat.unit(u.def).bounds.0 as i64;
    let (dx, dy) = (i64::from(to.x.0 - u.pos.x.0), i64::from(to.y.0 - u.pos.y.0));
    let len = dist(u.pos, to).max(1);
    let (px, py) = ((-dy * b / len) as i32, (dx * b / len) as i32);
    let side = |s: i32| {
        let a = Vec2::new(Fx(u.pos.x.0 + s * px), Fx(u.pos.y.0 + s * py));
        let z = Vec2::new(Fx(to.x.0 + s * px), Fx(to.y.0 + s * py));
        v.sight(a, z)
    };
    let (tx, ty) = to.cell();
    side(0) && side(1) && side(-1) && (-1..=1).all(|j| (-1..=1).all(|i| crate::nav::walkable(v, tx + i, ty + j)))
}

const FACES: [(Facing, Angle); 4] =
    [(Facing::East, Angle::EAST), (Facing::South, Angle::SOUTH), (Facing::West, Angle::WEST), (Facing::North, Angle::NORTH)];

/// Where to stand and which way to face to throw `u` its bait: a cell she can walk to without
/// waking it, with the bait's landing point (three cells ahead) in its sight, under its nose and
/// clear for its body to crawl to. Best out of its notice (it smells the meat from twice as far
/// as it notices her), and out of the sight of the rest of the room; failing that, anywhere it
/// smells the throw from: it wakes, and spits, and still cannot resist the meat. One already
/// woken is fed from wherever is nearest.
fn throw_spot(v: &View<'_>, u: &Unit, safe: &SafeGround) -> Option<Spot> {
    let woken = u.combat == CombatState::Combat;
    let (ux, uy) = u.pos.cell();
    let reach = nose(u) - i64::from(CELL_FX);
    let r = (reach / i64::from(CELL_FX)) as i32 + 4;
    let mut best: Option<(i64, Vec2, Facing, Angle, Vec2)> = None;
    for y in uy - r..=uy + r {
        for x in ux - r..=ux + r {
            let Some(&(_, steps)) = safe.steps.get(&(x, y)) else { continue };
            let at = Vec2::centre(x, y);
            if dist(at, u.pos) > reach + i64::from(3 * CELL_FX) {
                continue;
            }
            let seen = sees(v, u, at, i64::from(CELL_FX) / 2);
            for (f, a) in FACES {
                let (dx, dy) = f.delta();
                let to = Vec2::new(Fx(at.x.0 + dx * 3 * CELL_FX), Fx(at.y.0 + dy * 3 * CELL_FX));
                let (tx, ty) = to.cell();
                if !crate::nav::walkable(v, tx, ty) || !v.sight(at, to) {
                    continue;
                }
                if dist(u.pos, to) > reach || !clear_walk(v, u, to) {
                    continue;
                }
                let cost = i64::from(steps) * i64::from(CELL_FX)
                    + if watched(v, u.id, at) { i64::from(40 * CELL_FX) } else { 0 }
                    + if seen && !woken { i64::from(80 * CELL_FX) } else { 0 };
                if best.as_ref().is_none_or(|b| cost < b.0) {
                    best = Some((cost, at, f, a, to));
                }
            }
        }
    }
    let (_, at, face, dir, to) = best?;
    Some(Spot { path: safe.path_to(at.cell()), at, face, dir, to })
}

/// Where a throw is made from, and the safe way there.
struct Spot {
    path: Vec<(i32, i32)>,
    at: Vec2,
    face: Facing,
    dir: Angle,
    to: Vec2,
}

/// The ground she can walk to from her feet without a calm snake noticing her on the way: each
/// cell's parent and its steps from her.
struct SafeGround {
    steps: BTreeMap<(i32, i32), ((i32, i32), u32)>,
}

impl SafeGround {
    fn flood(v: &View<'_>, off: &[(Vec2, i64, u32)]) -> SafeGround {
        let danger = |c: (i32, i32)| {
            let at = Vec2::centre(c.0, c.1);
            off.iter().any(|&(o, r, k)| k == 0 && dist(o, at) <= r && v.sight(o, at))
        };
        let start = v.body().pos.cell();
        let mut steps = BTreeMap::new();
        let mut q = VecDeque::new();
        steps.insert(start, (start, 0u32));
        q.push_back(start);
        while let Some(c) = q.pop_front() {
            let n0 = steps[&c].1;
            if steps.len() > 12_000 {
                break;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (c.0 + dx, c.1 + dy);
                if !steps.contains_key(&n) && crate::nav::walkable(v, n.0, n.1) && !danger(n) {
                    steps.insert(n, (c, n0 + 1));
                    q.push_back(n);
                }
            }
        }
        SafeGround { steps }
    }

    /// The cells from her feet to `to`, in order.
    fn path_to(&self, to: (i32, i32)) -> Vec<(i32, i32)> {
        let mut out = vec![to];
        let mut c = to;
        while let Some(&(p, n)) = self.steps.get(&c) {
            if n == 0 {
                break;
            }
            out.push(p);
            c = p;
        }
        out.reverse();
        out
    }
}

/// A cell near `from` out of the sight of every point of the creature's walk to the bait (or
/// beyond its notice): where she waits while it eats.
fn hide_from(v: &View<'_>, u: &Unit, bait: Vec2, from: Vec2) -> Option<Vec2> {
    let mid = Vec2::new(Fx((u.pos.x.0 + bait.x.0) / 2), Fx((u.pos.y.0 + bait.y.0) / 2));
    let watch = [u.pos, mid, bait];
    let safe = |at: Vec2| watch.iter().all(|&w| dist(w, at) > notice(v, u) + i64::from(CELL_FX) || !v.sight(w, at));
    let start = from.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back(start);
    while let Some(c) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if safe(at) && dist(at, from) >= i64::from(2 * CELL_FX) {
            return Some(at);
        }
        if seen.len() > 3000 {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back(n);
            }
        }
    }
    None
}

/// Feeding one creature its bait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    pub unit: UnitId,
    pub bait: ItemId,
    /// The safe way to the spot, cells, and the next of them.
    pub path: Vec<(i32, i32)>,
    pub step: usize,
    pub at: Vec2,
    pub face: Facing,
    pub dir: Angle,
    pub to: Vec2,
    /// 0 walk, 1 face, 2 throw, 3 hide, 4 wait.
    pub stage: u8,
    pub t: u32,
    pub hide: Option<Vec2>,
    /// How many she held before the throw.
    pub held: u32,
}

/// A job the Burial's tactics run as a crawl task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    Feed(Feed),
}

impl Job {
    pub fn tick(&mut self, v: &View<'_>, cx: &mut Ctx) -> Status {
        match self {
            Job::Feed(f) => feed(f, v, cx),
        }
    }
}

/// Along a safe way, a few cells ahead at a time, then onto the spot itself. `None`: there.
fn walk_safe(path: &[(i32, i32)], step: &mut usize, at: Vec2, v: &View<'_>, cx: &mut Ctx) -> Option<Status> {
    let me = v.body().pos;
    while *step < path.len() && dist(me, Vec2::centre(path[*step].0, path[*step].1)) <= i64::from(CELL_FX) {
        *step += 1;
    }
    let (goal, near) = match path.get((*step + 2).min(path.len().saturating_sub(1))) {
        Some(&(x, y)) if *step < path.len() => (Vec2::centre(x, y), Fx::from_px(4)),
        _ => (at, Fx::from_px(2)),
    };
    match cx.nav.go(v, goal, near, false) {
        Go::Walk(fr) => Some(Status::Act(Act::hold(fr))),
        Go::Arrived if goal != at => {
            *step += 1;
            Some(Status::Act(Act::idle()))
        }
        Go::Arrived => None,
        Go::NoWay => Some(Status::Failed("no safe way there".into())),
    }
}

/// The bolt she knows, can pay for and has ready that does `u` the most harm by its resists
/// (Icebolt; Fireball once the garden has taught it).
pub fn best_bolt(v: &View<'_>, u: &Unit) -> Option<jane_core::SpellId> {
    let cat = jane_data::catalog();
    let me = v.body();
    let now = v.tick();
    // What gets through of each (its row and its statuses: softened lets fire in), and the
    // cheaper on a tie.
    ["icebolt", "fireball"]
        .into_iter()
        .map(sense::spell)
        .filter(|&s| sense::knows(v, s) && crate::fight::ready(me, s, now))
        .filter(|&s| crate::fight::gap(me, u) <= i64::from(cat.combat.spell(s).range.0) * 9 / 10)
        .max_by_key(|&s| {
            let d = cat.combat.spell(s);
            (jane_sim::status::resist_factor(u, d.school, now), -i64::from(d.mp.0))
        })
}

fn feed(f: &mut Feed, v: &View<'_>, cx: &mut Ctx) -> Status {
    f.t += 1;
    let Some(u) = v.unit(f.unit).filter(|u| u.alive) else { return Status::Done };
    if f.t > 60 * 60 {
        return Status::Failed("the feeding took too long".into());
    }

    match f.stage {
        0 => match walk_safe(&f.path, &mut f.step, f.at, v, cx) {
            Some(s) => s,
            None => {
                f.stage = 1;
                f.t = 0;
                Status::Act(Act::hold(nudge(f.dir)))
            }
        },
        1 => {
            if v.body().facing == f.face {
                f.stage = 2;
                f.t = 0;
                return Status::Act(Act::idle());
            }
            if f.t > 30 {
                return Status::Failed("could not face the throw".into());
            }
            Status::Act(Act::hold(nudge(f.dir)))
        }
        2 => {
            let n = holds(v, f.bait);
            if n == 0 {
                return Status::Failed("no bait left".into());
            }
            if f.held == 0 {
                f.held = n;
            }
            if n < f.held {
                // Thrown: out of its sight while it goes for it.
                f.stage = 3;
                f.t = 0;
                f.hide = hide_from(v, u, f.to, v.body().pos);
                cx.nav.reset();
                return Status::Act(Act::idle());
            }
            if v.body().facing != f.face {
                return Status::Act(Act::hold(nudge(f.dir)));
            }
            if f.t > 60 * 5 {
                return Status::Failed("the bait would not leave her hand".into());
            }
            // Between casts and with the throw off its cooldown, or the press is lost.
            let me = v.body();
            let now = v.tick();
            if me.gcd_until > now || me.item_cooldowns.iter().any(|&(i, t)| i == f.bait && t > now) {
                return Status::Act(Act::idle());
            }
            Status::Act(Act::press(Command::Item(f.bait)))
        }
        3 => {
            let Some(h) = f.hide else {
                f.stage = 4;
                return Status::Act(Act::idle());
            };
            match cx.nav.go(v, h, Fx::from_px(3), true) {
                Go::Walk(fr) => Status::Act(Act::hold(InputFrame { sprint: true, ..fr })),
                _ => {
                    f.stage = 4;
                    f.t = 0;
                    Status::Act(Act::idle())
                }
            }
        }
        _ => {
            // It eats within a few seconds of setting off, or it did not smell it.
            if f.t > 60 * 12 {
                return Status::Failed("it did not take the bait".into());
            }
            Status::Act(Act::idle())
        }
    }
}

/// What the Burial offers the crawl beyond its order, as (class, cost, try, task): each
/// creature that is fed rather than fought, while it is calm and she has its bait.
pub fn offers(v: &View<'_>, cx: &Ctx, reach: &Reach) -> Vec<(u8, i64, Try, Task)> {
    let cat = jane_data::catalog();
    let me = v.body().pos;
    let mut out = Vec::new();
    // The fire down here: sat at as soon as she can get to it, so a fall wakes her beside it
    // and not at the Halt (the fire's own words: "If it goes badly out there, this is where you
    // will wake"), and again whenever she is hurt and nothing is on her.
    let hurt = sense::hp_permille(v.body()) < 500;
    for p in v.props() {
        let d = cat.story.prop(p.def);
        if p.hidden || !d.rest || !reach.beside(p) || v.prop_spawn(p).is_none_or(|s| s.talk.is_none()) {
            continue;
        }
        let sat = cx.used.contains_key(&(v.zone(), p.id));
        if !sat || hurt {
            out.push((0, sense::to_prop(p, me), Try::Tactic(p.id.get()), Task::Use(crate::task::UseProp::new(p.id))));
        }
    }
    // What stands rooted over something and does not stand up again (the garden's flower over
    // the scroll): put down before the rest of the room is looked at, from as far as her bolt
    // carries (it cannot reach her there).
    for u in sense::enemies(v) {
        let d = cat.combat.unit(u.def);
        let rooted = d.walk.0 == 0 && d.run.0 == 0;
        if u.alive && rooted && d.respawn.0 == 0 && !d.boss && d.bait.is_none() && reach.near(u.pos, 16) {
            out.push((3, dist(me, u.pos), Try::Fight(u.id), Task::Hunt(u.id)));
        }
    }
    out.extend(feeds(v, cx));
    out
}

/// Each creature that is fed rather than fought, while it is calm and she has its bait and a
/// place to throw it from: before anything else there (a statue in the room is let shoot while
/// she throws; killed, it stands up again anyway).
fn feeds(v: &View<'_>, cx: &Ctx) -> Vec<(u8, i64, Try, Task)> {
    let cat = jane_data::catalog();
    let me = v.body().pos;
    let mut out = Vec::new();
    let calm: Vec<&Unit> = sense::enemies(v)
        .into_iter()
        .filter(|u| u.alive && cat.combat.unit(u.def).bait.is_some_and(|b| holds(v, b) > 0))
        .collect();
    if calm.is_empty() {
        return out;
    }
    let safe = SafeGround::flood(v, &keep_off(v, cx));
    for u in &calm {
        let Some(bait) = cat.combat.unit(u.def).bait else { continue };
        let Some(Spot { path, at, face, dir, to }) = throw_spot(v, u, &safe) else { continue };
        let job = Feed { unit: u.id, bait, path, step: 0, at, face, dir, to, stage: 0, t: 0, hide: None, held: 0 };
        out.push((1, dist(me, u.pos), Try::Fight(u.id), Task::Burial(Job::Feed(job))));
    }
    out
}

/// What the Burial asks that comes before the task she is on (a creature to feed has come into
/// her view, calm, and she has its bait and a place to throw it from): the tries, for the crawl
/// to weigh as it weighs its own (one tried too often is not cut in for).
pub fn cuts_in(v: &View<'_>, cx: &Ctx, task: &Task, what: Try) -> Vec<Try> {
    if matches!(task, Task::Burial(_)) || matches!(what, Try::Tactic(_)) {
        return Vec::new();
    }
    feeds(v, cx).into_iter().map(|(_, _, w, _)| w).collect()
}

/// Is there a small snake here still to feed (alive, and she has its bait)? One she saw and has
/// walked away from (it sleeps out of her sight) is remembered.
fn feeding(v: &View<'_>, cx: &Ctx) -> bool {
    let cat = jane_data::catalog();
    let awake = sense::enemies(v).into_iter().any(|u| {
        cat.combat.unit(u.def).bait.is_some_and(|b| holds(v, b) > 0) && u.alive
    });
    awake
        || cx.seen_foes.iter().any(|(&def, seen)| {
            cat.combat.unit(def).bait.is_some_and(|b| holds(v, b) > 0)
                && seen.iter().any(|(&id, &(z, _))| z == v.zone() && v.unit(id).is_none())
        })
}

/// Out of harm's way while something is still to be fed: no calm snake would notice her there
/// and nothing that stands still has her in its sight and range.
fn covered(v: &View<'_>, at: Vec2) -> bool {
    let cat = jane_data::catalog();
    sense::enemies(v).into_iter().filter(|u| u.alive).all(|u| {
        let d = cat.combat.unit(u.def);
        if d.bait.is_some() {
            return u.combat == CombatState::Combat || !sees(v, u, at, i64::from(CELL_FX));
        }
        if d.run.0 > 0 || d.walk.0 > 0 {
            return true;
        }
        let range = d.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
        dist(u.pos, at) > i64::from(range) + i64::from(2 * CELL_FX) || !v.sight(u.pos, at)
    })
}

/// The nearest covered cell she can walk to (a short flood from her feet).
fn cover(v: &View<'_>) -> Option<Vec2> {
    let start = v.body().pos.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back(start);
    while let Some(c) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if covered(v, at) {
            return Some(at);
        }
        if seen.len() > 2500 {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back(n);
            }
        }
    }
    None
}

/// The Burial's fight (`None`: the ordinary one, which steps out of a pool laid under her).
/// Everything down here out-hits her swing, so everything is fought with the bolt that gets
/// through best (Fireball once she has it), from as far as it carries, stepping aside from what
/// is thrown at her and backing off between casts from what walks (frost slows it; she is
/// quicker), inside two thirds of its leash so it keeps coming rather than going home to mend.
/// What she brought is drunk against a boss.
///
/// While a small snake is still to be fed nothing takes her into its sight: she backs off only
/// over ground it cannot see, a thing that walks is waited for in cover (where nothing rooted
/// can shoot her) rather than walked to, and a thing rooted to the spot is let be
/// (`Some(None)`) while she gets on with the feeding; with the bait in her hand at the spot,
/// the throw comes first.
pub fn fight(v: &View<'_>, cx: &mut Ctx, id: UnitId, task: Option<&Task>) -> Option<Option<Act>> {
    use crate::fight::{gap, item_ready};
    let careful = feeding(v, cx);
    if careful {
        if let Some(Task::Burial(Job::Feed(f))) = task {
            if (1..=2).contains(&f.stage) {
                return Some(None);
            }
        }
    }
    // What is thrown at her is stepped out of first; then a pool laid under her (the ordinary
    // fight steps out of that).
    if let Some(f) = dodge(v, &cx.nav.keep_off) {
        return Some(Some(Act::hold(f)));
    }
    if crate::fight::tell_under(v).is_some() {
        return None;
    }
    if let Some(c) = crate::fight::eat(v) {
        return Some(Some(Act::press(c)));
    }
    let cat = jane_data::catalog();
    let me = v.body();
    let t = v.unit(id).filter(|t| t.alive)?;
    let now = v.tick();
    let td = cat.combat.unit(t.def);
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    let aim = InputFrame { aim: Some(dir), ..InputFrame::IDLE };
    let g = gap(me, t);
    cx.fight.target = Some(id);
    cx.foes.insert(id);
    if td.boss && g < i64::from(14 * CELL_FX) && me.statuses.is_empty() {
        for name in ["potion_stoneskin", "potion_lifesteal", "potion_manashield"] {
            let p = sense::item(name);
            if holds(v, p) > 0 && item_ready(me, p, now) {
                return Some(Some(Act::press(Command::Item(p))));
            }
        }
    }
    let mobile = td.walk.0 > 0 || td.run.0 > 0;
    // What is rooted and stands up again is let shoot, and so is anything rooted while there is
    // feeding to do: stopping for it is standing in its line longer.
    if !mobile && (careful || let_be(t.def)) {
        return Some(None);
    }
    // Otherwise something rooted to the spot is taken on alone where it can be: from ground in
    // its sight and her bolt's reach where nothing else rooted can shoot her (a doorway, a
    // pillar's lee).
    if !mobile {
        match clean_spot(v, cx, t) {
            Some(at) if dist(me.pos, at) > i64::from(CELL_FX) => {
                if let Go::Walk(f) = cx.nav.go(v, at, Fx::from_px(4), true) {
                    return Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f })));
                }
            }
            Some(_) => {
                if let Some(bolt) = best_bolt(v, t) {
                    return Some(Some(Act { frame: aim, cmds: vec![Command::Cast { spell: bolt, on: Some(id) }] }));
                }
                return Some(Some(Act::hold(aim)));
            }
            None => {}
        }
    }
    if shot_clear(v, me.pos, t.pos) {
        if let Some(bolt) = best_bolt(v, t) {
            return Some(Some(Act { frame: aim, cmds: vec![Command::Cast { spell: bolt, on: Some(id) }] }));
        }
    }
    if mobile && g < i64::from(6 * CELL_FX) {
        let leash = i64::from(td.leash.0);
        let tether = (leash > 0).then_some((t.home, leash * 2 / 3));
        // The snake loses her the moment a wall stands between them, and goes home whole: its
        // room is kept to (the room it was met in, as the map shows it).
        let room = (td.controller == jane_data::Controller::Snake).then(|| room_of(v, t.home)).flatten();
        if let Some(f) = retreat(v, cx, t.pos, tether, room) {
            return Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f })));
        }
    }
    let melee = sense::spell("melee_player");
    if g <= i64::from(cat.combat.spell(melee).range.0) {
        let cmds =
            if crate::fight::ready(me, melee, now) { vec![Command::Cast { spell: melee, on: Some(id) }] } else { Vec::new() };
        return Some(Some(Act { frame: aim, cmds }));
    }
    if careful {
        if !covered(v, me.pos) {
            if let Some(c) = cover(v) {
                if let Go::Walk(f) = cx.nav.go(v, c, Fx::from_px(3), true) {
                    return Some(Some(Act::hold(InputFrame { aim: Some(dir), sprint: true, ..f })));
                }
            }
        }
        return Some(Some(Act::hold(aim)));
    }
    let reach = i64::from(cat.combat.spell(sense::spell("icebolt")).range.0) * 8 / 10;
    if g > reach || !shot_clear(v, me.pos, t.pos) {
        let near = if g > reach { reach } else { i64::from(3 * CELL_FX) };
        return match cx.nav.go(v, t.pos, Fx(near as i32), true) {
            Go::Walk(f) => Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f }))),
            _ => None,
        };
    }
    Some(Some(Act::hold(aim)))
}

/// Backing off from `from`: to the cell within a short walk that is farthest from it, over
/// ground no calm snake can see and, where it can, out of what is rooted's line, kept inside
/// `tether` (its home and leash, shrunk). A stick toward it.
fn retreat(
    v: &View<'_>,
    cx: &mut Ctx,
    from: Vec2,
    tether: Option<(Vec2, i64)>,
    room: Option<jane_core::Rect>,
) -> Option<InputFrame> {
    let me = v.body().pos;
    let off = cx.nav.keep_off.clone();
    let banned = |c: Vec2| off.iter().any(|&(o, r, k)| k == 0 && dist(o, c) <= r && v.sight(o, c));
    let price = |c: Vec2| {
        off.iter().filter(|&&(o, r, k)| k > 0 && dist(o, c) <= r && v.sight(o, c)).map(|&(_, _, k)| i64::from(k)).sum::<i64>()
    };
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    let mut best: Option<(i64, (i32, i32))> = None;
    while let Some((c, steps)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        let inside = tether.is_none_or(|(home, r)| dist(at, home) <= r)
            && room.is_none_or(|r| r.x < c.0 && c.0 < r.right() - 1 && r.y < c.1 && c.1 < r.bottom() - 1);
        if inside && steps > 0 {
            let score = dist(at, from) - i64::from(steps) * i64::from(CELL_FX) / 3 - price(at) * i64::from(CELL_FX) / 4;
            if best.is_none_or(|b| (score, c) > b) {
                best = Some((score, c));
            }
        }
        if steps >= 16 || seen.len() > 1200 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && !banned(Vec2::centre(n.0, n.1)) && seen.insert(n) {
                q.push_back((n, steps + 1));
            }
        }
    }
    let (_, c) = best?;
    let to = Vec2::centre(c.0, c.1);
    match cx.nav.go(v, to, Fx::from_px(3), true) {
        Go::Walk(f) => Some(InputFrame { sprint: true, ..f }),
        _ => None,
    }
}

/// The ground her feet keep off (`Nav::keep_off`): for good, where a small snake still to be
/// fed would notice her (and, once it has, all it can see); at a price, where something rooted to the spot would have her in its
/// sight and range (the hall's cactus, the statues: killed, they stand up again once she is out
/// of sight, so the way round is cheaper than the fight). Remembered ones asleep out of her
/// sight count too.
pub fn keep_off(v: &View<'_>, cx: &Ctx) -> Vec<(Vec2, i64, u32)> {
    let cat = jane_data::catalog();
    let me = i64::from(cat.combat.unit(v.body().def).bounds.0);
    let mut out: Vec<(Vec2, i64, u32)> = Vec::new();
    for (&def, seen) in &cx.seen_foes {
        let d = cat.combat.unit(def);
        let rooted = d.walk.0 == 0 && d.run.0 == 0 && d.aggro.0 > 0 && !d.boss;
        let fed = d.bait.is_some_and(|b| holds(v, b) > 0);
        if !fed && !rooted {
            continue;
        }
        let (r, cost) = if fed {
            (i64::from(d.aggro.0) + i64::from(d.bounds.0) + me + i64::from(CELL_FX), 0)
        } else {
            let range = d.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
            (i64::from(range.max(d.aggro.0)) + i64::from(d.bounds.0) + me, 12)
        };
        for (&id, &(z, pos)) in seen {
            if z != v.zone() {
                continue;
            }
            match v.unit(id) {
                // Woken, it is fed where it is (it cannot resist the meat, even spitting).
                Some(u) if u.alive && fed && u.combat == CombatState::Combat => {}
                Some(u) if u.alive => out.push((u.pos, r, cost)),
                Some(_) => {}
                None => out.push((pos, r, cost)),
            }
        }
    }
    out
}

/// A slow bolt or a needle coming at her (the statues' poison is "slow enough to walk around"):
/// a step aside, square to its line, on the side she stands of it, while there is time.
fn dodge(v: &View<'_>, off: &[(Vec2, i64, u32)]) -> Option<InputFrame> {
    let me = v.body();
    let body = i64::from(jane_data::catalog().combat.unit(me.def).bounds.0);
    let mut soonest: Option<(i64, InputFrame)> = None;
    for p in v.projectiles() {
        // What is let be (the statues, walked past under their fire) is not danced round: in a
        // room of them that is all she would ever do.
        if p.faction == me.faction || p.from.and_then(|f| v.unit(f)).is_some_and(|u| let_be(u.def)) {
            continue;
        }
        let (vx, vy) = (i64::from(p.vel.x.0), i64::from(p.vel.y.0));
        let v2 = vx * vx + vy * vy;
        if v2 == 0 {
            continue;
        }
        let (rx, ry) = (i64::from(me.pos.x.0 - p.pos.x.0), i64::from(me.pos.y.0 - p.pos.y.0));
        let t = (rx * vx + ry * vy) / v2;
        if !(3..=70).contains(&t) {
            continue;
        }
        // Where it passes her, and how near.
        let (cx, cy) = (rx - vx * t, ry - vy * t);
        let miss = i64::from(jane_core::num::isqrt((cx * cx + cy * cy) as u64));
        if miss > body + i64::from(CELL_FX) {
            continue;
        }
        // Square to its line, away from it; straight on, whichever side is open.
        let cross = vx * ry - vy * rx;
        let (px, py) = if cross >= 0 { (-vy, vx) } else { (vy, -vx) };
        let len = i64::from(jane_core::num::isqrt(v2 as u64)).max(1);
        let step = |sx: i64, sy: i64| {
            let to = Vec2::new(
                Fx(me.pos.x.0 + (sx * 2 * i64::from(CELL_FX) / len) as i32),
                Fx(me.pos.y.0 + (sy * 2 * i64::from(CELL_FX) / len) as i32),
            );
            let (tx, ty) = to.cell();
            (crate::nav::walkable(v, tx, ty) && !off.iter().any(|&(o, r, k)| k == 0 && dist(o, to) <= r && v.sight(o, to))).then(|| InputFrame { sprint: true, ..crate::nav::stick(me.pos, to, true) })
        };
        let Some(f) = step(px, py).or_else(|| step(-px, -py)) else { continue };
        if soonest.as_ref().is_none_or(|s| t < s.0) {
            soonest = Some((t, f));
        }
    }
    soonest.map(|(_, f)| f)
}

/// Each frame in the Burial, before anything else: the ground her feet keep off is brought up
/// to date, and a small snake that has her in its sight, and that she has nothing left to feed,
/// is got out of the sight of (it cannot follow, and it spits from forty cells).
pub fn before(v: &View<'_>, cx: &mut Ctx) -> Option<Act> {
    let off = keep_off(v, cx);
    if off != cx.nav.keep_off {
        cx.nav.keep_off = off;
        cx.nav.reset();
    }
    let cat = jane_data::catalog();
    let me = v.body().pos;
    let woke: Vec<&Unit> = sense::enemies(v)
        .into_iter()
        .filter(|u| {
            cat.combat.unit(u.def).bait.is_some_and(|b| holds(v, b) == 0) && crate::fight::on_me(v, u) && v.sight(u.pos, me)
        })
        .collect();
    if woke.is_empty() {
        return None;
    }
    if let Some(c) = crate::fight::eat(v) {
        return Some(Act::press(c));
    }
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back(start);
    while let Some(c) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if woke.iter().all(|u| !v.sight(u.pos, at)) && dist(at, me) > i64::from(CELL_FX) {
            return match cx.nav.go(v, at, Fx::from_px(3), true) {
                Go::Walk(f) => Some(Act::hold(InputFrame { sprint: true, ..f })),
                _ => Some(Act::hold(InputFrame { sprint: true, ..crate::nav::stick(me, at, true) })),
            };
        }
        if seen.len() > 3000 {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, n.0, n.1) && seen.insert(n) {
                q.push_back(n);
            }
        }
    }
    None
}

/// Ground to bolt a rooted `t` from: within a short walk, in its sight and inside her bolt's
/// reach, where no other rooted thing awake here can shoot her and no calm snake would see her;
/// the nearest such, where she stands if it will do.
fn clean_spot(v: &View<'_>, cx: &Ctx, t: &Unit) -> Option<Vec2> {
    let cat = jane_data::catalog();
    let reach = i64::from(cat.combat.spell(sense::spell("icebolt")).range.0) * 8 / 10;
    let shooters: Vec<(Vec2, i64)> = sense::enemies(v)
        .into_iter()
        .filter(|u| u.alive && u.id != t.id)
        .filter_map(|u| {
            let d = cat.combat.unit(u.def);
            if d.walk.0 > 0 || d.run.0 > 0 || d.bait.is_some() {
                return None;
            }
            let range = d.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
            Some((u.pos, i64::from(range) + i64::from(2 * CELL_FX)))
        })
        .collect();
    let off = &cx.nav.keep_off;
    let good = |at: Vec2| {
        dist(at, t.pos) <= reach
            && shot_clear(v, at, t.pos)
            && shooters.iter().all(|&(o, r)| dist(o, at) > r || !v.sight(o, at))
            && !off.iter().any(|&(o, r, k)| k == 0 && dist(o, at) <= r && v.sight(o, at))
    };
    let me = v.body().pos;
    if good(me) {
        return Some(me);
    }
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    while let Some((c, n)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if n > 0 && good(at) {
            return Some(at);
        }
        if n >= 20 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nb = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, nb.0, nb.1) && seen.insert(nb) {
                q.push_back((nb, n + 1));
            }
        }
    }
    None
}

/// Not worth a fight down here: something rooted to the spot that stands up again once she is
/// out of its sight and only throws what can be stepped out of (the statues' poison, the
/// cactus's needles). It is walked past, round its line where there is a way round
/// (`keep_off`). A web spinner is not: its webs hold her where she stands.
pub fn let_be(def: jane_core::UnitDefId) -> bool {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(def);
    d.walk.0 == 0
        && d.run.0 == 0
        && d.respawn.0 > 0
        && !d.boss
        && d.bait.is_none()
        && d.book.iter().all(|&s| cat.combat.spell(s).kind == jane_data::SpellKind::Bolt)
}

/// Will a bolt from `a` fly to `b`? Walls stop it as they stop sight, and so does anything solid
/// standing in the way (a crate, a coffin, a chest: `BLOCK_SHOT`), which sight goes over. The
/// line is walked a quarter cell at a time.
pub fn shot_clear(v: &View<'_>, a: Vec2, b: Vec2) -> bool {
    let d = dist(a, b);
    let step = i64::from(CELL_FX) / 4;
    let n = (d / step).max(1);
    let start = a.cell();
    let end = b.cell();
    (1..n).all(|i| {
        let p = Vec2::new(
            Fx(a.x.0 + ((i64::from(b.x.0 - a.x.0) * i) / n) as i32),
            Fx(a.y.0 + ((i64::from(b.y.0 - a.y.0) * i) / n) as i32),
        );
        let c = p.cell();
        c == start || c == end || v.flags(c.0, c.1) & jane_core::tile::BLOCK_SHOT == 0
    })
}

/// Where a task would take her (for `not_yet`).
fn task_point(v: &View<'_>, t: &Task) -> Option<Vec2> {
    match t {
        Task::Walk { to, .. } => Some(*to),
        Task::Use(u) => v.prop(u.prop).map(sense::prop_centre),
        Task::WorldCast { prop, .. } => v.prop(*prop).map(sense::prop_centre),
        Task::Aim { from, .. } => Some(*from),
        Task::Hunt(id) => v.unit(*id).map(|u| u.pos),
        Task::Pickup { drop, .. } => v.drops().iter().find(|d| d.id == *drop).map(|d| d.pos),
        Task::Push(p) => v.prop(p.prop).map(sense::prop_centre),
        Task::Burial(Job::Feed(f)) => Some(f.at),
        _ => None,
    }
}

/// A corner not to be gone into yet: the glasshouse shuts behind her on the gardener's flower,
/// which only fire keeps down (DUNGEONS.md §3.5: burning is the one hurt that outpaces its
/// mending), so it waits until the garden's scroll has taught her Fireball. Its door counts as
/// inside it.
pub fn not_yet(v: &View<'_>, t: &Task) -> bool {
    // In a corner with its keeper awake, the keeper first: nothing else in the room is worth
    // turning her back on it for (the snake's cold torches least of all).
    let cat = jane_data::catalog();
    let me = v.body().pos;
    let keeper = sense::enemies(v).into_iter().find(|u| {
        u.alive && cat.combat.unit(u.def).boss && room_of(v, u.home).is_some_and(|r| r.contains(me.cell().0, me.cell().1))
    });
    if let Some(k) = keeper {
        if !matches!(t, Task::Hunt(id) if *id == k.id) {
            return true;
        }
    }
    if sense::knows(v, sense::spell("fireball")) {
        return false;
    }
    let Some(at) = task_point(v, t) else { return false };
    let (x, y) = at.cell();
    let inside = |name: &str| v.sym(name).and_then(|s| v.rect(s)).is_some_and(|r| r.contains(x, y));
    let door = sense::prop_named(v, "gate_glasshouse").is_some_and(|g| crate::sense::to_prop(g, at) <= i64::from(2 * CELL_FX));
    inside("burial_glasshouse") || door
}

/// The smallest named room rect holding `at` (the map's rooms, as a player reads them).
fn room_of(v: &View<'_>, at: Vec2) -> Option<jane_core::Rect> {
    let (x, y) = at.cell();
    let (w, h) = v.size();
    v.rects()
        .map(|(_, r)| r)
        .filter(|r| r.contains(x, y) && (r.w as u32) < w && (r.h as u32) < h)
        .min_by_key(|r| (i64::from(r.w) * i64::from(r.h), r.x, r.y))
}
