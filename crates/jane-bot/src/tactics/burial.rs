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
//!   The burst (Explosion) goes at a crowd; the queen's web is got out of before it takes hold.
//!   Something rooted is taken on from ground where nothing else rooted reaches her, or let be;
//!   something that walks comes to her, or is let be (only a keeper that throws is gone to).
//! - **What comes first** (`not_yet`, `offers`): a corner's keeper before anything else in its
//!   room; the garden's flower before the rest of the garden (the scroll is under it); not the
//!   glasshouse before the scroll has taught her Fire; not a keeper's room (or the ground before
//!   one that walks) short of breath: the fire first (`ready`). Drops of plain things left in
//!   rooms she has walked out of, and torches that only give light, are not walked back for.
//! - **The great torch** (`under_the_torch`, `held_off`): the crawl pushes it onto the plate by
//!   HIS SOLDIER's gate like any plate's barrel; the shades at the edge of its light are let be
//!   while she pushes, never hunted, and shot from inside warm light when they stand off at its
//!   edge (waiting for them, or backing off from them, is a stand-off for ever).
//! - **Getting nowhere** (`Watch`): fifteen seconds standing still with nothing about her going
//!   down, the crawl drops her task and she lets be what is not at her elbow a while.
//! - **Done** (`done`): Goldskin down and the ball in her bag, she leaves by the way she came.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use jane_core::action::Facing;
use jane_core::num::CELL_FX;
use jane_core::{Angle, Fx, ItemId, Vec2};
use jane_data::SpellKind;
use jane_sim::ids::UnitId;
use jane_sim::state::CombatState;
use jane_sim::{Command, InputFrame, Unit, View};

use crate::Act;
use crate::crawl::{Reach, Try};
use crate::nav::{Go, dist};
use crate::sense::{self, holds};
use crate::task::{Ctx, Status, Task, nudge};

/// How far `u` notices her, body to body, as the sim reckons it (`ai::aggro_reach`: her growth
/// against its phase), reckoned dark after the bell whether or not it stands in a lamp's light.
fn aggro_of(v: &View<'_>, u: &Unit) -> i64 {
    let d = jane_data::catalog().combat.unit(u.def);
    let me = v.body();
    let her = u32::from(me.strength) + u32::from(me.spirit);
    jane_sim::ai::aggro_reach(d, u.strength, Some(her), i32::from(v.is_night()))
}

/// A baited creature notices her within its aggro, body to body (`ai::seen`): at or under this,
/// centre to centre and in its sight, it has her.
fn notice(v: &View<'_>, u: &Unit) -> i64 {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(u.def);
    aggro_of(v, u) + i64::from(d.bounds.0) + i64::from(cat.combat.unit(v.body().def).bounds.0)
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
            && dist(u.pos, at) <= aggro_of(v, u) + i64::from(d.bounds.0) + me + i64::from(CELL_FX)
            && v.sight(u.pos, at)
    })
}

/// Can `u` get to the bait at `to`: it smells it only in its sight (`ai::seek_bait`), and walks
/// to it by a path, so the line from its middle is clear and so is the ground about the bait.
fn clear_walk(v: &View<'_>, u: &Unit, to: Vec2) -> bool {
    let (tx, ty) = to.cell();
    v.sight(u.pos, to) && (-1..=1).all(|j| (-1..=1).all(|i| crate::nav::walkable(v, tx + i, ty + j)))
}

const FACES: [(Facing, Angle); 4] = [
    (Facing::East, Angle::EAST),
    (Facing::South, Angle::SOUTH),
    (Facing::West, Angle::WEST),
    (Facing::North, Angle::NORTH),
];

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
            if steps.len() > 60_000 {
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
    /// The piece she threw, once it is down.
    pub meat: Option<jane_sim::ids::DropId>,
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
        // Her feet will not plan through the snake's notice (the way in when there is no other):
        // the way is walked cell by cell as it was laid.
        Go::NoWay if *step < path.len() => {
            let next = path[(*step + 1).min(path.len() - 1)];
            Some(Status::Act(Act::hold(crate::nav::stick(me, Vec2::centre(next.0, next.1), false))))
        }
        Go::NoWay => Some(Status::Failed("no safe way there".into())),
    }
}

/// The bolt she knows, can pay for and has ready that does `u` the most harm by its resists
/// (Icebolt; Fireball once the garden has taught it).
pub fn best_bolt(v: &View<'_>, u: &Unit) -> Option<jane_core::SpellId> {
    let cat = jane_data::catalog();
    let me = v.body();
    let now = v.tick();
    let near = |s: jane_core::SpellId| {
        crate::fight::gap(me, u) <= i64::from(cat.combat.spell(s).range.0) * 9 / 10 && crate::fight::may_cast(me, u, s)
    };
    // The burst ("Everything near the burst takes all of it"): at a crowd (something else inside
    // its reach of the one she aims at), or at a rooted keeper while she has mana to spare. It hits
    // harder than either bolt and never her, and waits six seconds between.
    let burst = sense::spell("explosion");
    let bd = cat.combat.spell(burst);
    let crowd = bd.splash.is_some_and(|sp| {
        sense::enemies(v)
            .into_iter()
            .any(|o| o.id != u.id && o.alive && crate::fight::gap(o, u) <= i64::from(sp.radius.0))
    });
    let spare = i64::from(me.mp.0) * 2 > i64::from(jane_sim::units::max_mp(me).0);
    // (Not at a keeper that walks: the frost's slow is what keeps it off her.)
    let kd = cat.combat.unit(u.def);
    let keeper = kd.boss && kd.walk.0 == 0 && kd.run.0 == 0;
    if sense::knows(v, burst)
        && crate::fight::ready(me, burst, now)
        && near(burst)
        && (crowd || keeper && spare)
        && jane_sim::status::resist_factor(u, bd.school, now) >= 500
    {
        return Some(burst);
    }
    // What gets through of each (its row and its statuses: softened lets fire in), and the
    // cheaper on a tie.
    ["icebolt", "fireball"]
        .into_iter()
        .map(sense::spell)
        .filter(|&s| sense::knows(v, s) && crate::fight::ready(me, s, now))
        .filter(|&s| near(s))
        .max_by_key(|&s| {
            let d = cat.combat.spell(s);
            (jane_sim::status::resist_factor(u, d.school, now), -i64::from(d.mp.0))
        })
        // Too close for a cast that takes time: the instant spark, while it gets through.
        .or_else(|| {
            let s = sense::spell("spark");
            let gets = jane_sim::status::resist_factor(u, cat.combat.spell(s).school, now) >= 500;
            (sense::knows(v, s) && crate::fight::ready(me, s, now) && near(s) && gets).then_some(s)
        })
}

fn feed(f: &mut Feed, v: &View<'_>, cx: &mut Ctx) -> Status {
    f.t += 1;
    // Thrown, the piece is looked for until it is gone from where it fell (eaten: the one that
    // ate it is dead of it, wherever it lies).
    if f.stage >= 3 && f.meat.is_none() {
        f.meat = v.drops().iter().filter(|d| d.item == f.bait).min_by_key(|d| (dist(d.pos, f.to), d.id)).map(|d| d.id);
    }
    let eaten = f.meat.is_some_and(|m| !v.drops().iter().any(|d| d.id == m));
    let forget = |cx: &mut Ctx| {
        if let Some(seen) = cx.seen_foes.values_mut().find(|s| s.contains_key(&f.unit)) {
            seen.remove(&f.unit);
        }
    };
    let Some(u) = v.unit(f.unit).filter(|u| u.alive) else {
        // Dead where she can see it, or gone off out of her sight to the piece and the piece
        // eaten: not kept off again. (Out of sight with the piece still lying, it is waited on.)
        let dead = v.unit(f.unit).is_some_and(|u| !u.alive);
        if dead || eaten {
            forget(cx);
            return Status::Done;
        }
        if f.stage < 3 {
            return Status::Done;
        }
        if f.t > 60 * 12 {
            return Status::Failed("it did not take the bait".into());
        }
        return Status::Act(Act::idle());
    };
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
            // (Thrown is asked first: her last piece thrown leaves none in her hand.)
            if f.held == 0 {
                if n == 0 {
                    return Status::Failed("no bait left".into());
                }
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
            // Eaten and still standing: another ate it, or the bite lands next tick. Waited on a
            // moment, then let be (the one fed may yet come to more).
            if eaten && f.t > 60 {
                return Status::Done;
            }
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
pub fn offers(
    v: &View<'_>,
    cx: &Ctx,
    reach: &Reach,
    downed: &[(jane_core::UnitDefId, u32)],
) -> Vec<(u8, i64, Try, Task)> {
    let cat = jane_data::catalog();
    let me = v.body().pos;
    let mut out = Vec::new();
    // The fire down here: sat at as soon as she can get to it, so a fall wakes her beside it
    // and not at the Halt (the fire's own words: "If it goes badly out there, this is where you
    // will wake"), and again whenever she is hurt and nothing is on her.
    // Not worn but not whole either, the fire waits until everything else she can do is done:
    // what is left then is a keeper's room, and those are not walked into short (`not_yet`).
    let hurt = worn(v);
    let short = !ready(v);
    for p in v.props() {
        let d = cat.story.prop(p.def);
        if p.hidden
            || !d.rest
            || !reach.beside(p)
            || v.prop_spawn(p).is_none_or(|s| s.talk.is_none())
            || shot_at(v, cx, p)
        {
            continue;
        }
        let sat = cx.used.contains_key(&(v.zone(), p.id));
        if !sat || hurt {
            out.push((0, sense::to_prop(p, me), Try::Tactic(p.id.get()), Task::Use(crate::task::UseProp::new(p.id))));
        } else if short {
            out.push((9, sense::to_prop(p, me), Try::Tactic(p.id.get()), Task::Use(crate::task::UseProp::new(p.id))));
        }
    }
    // The seal in the hall: "It will not turn while any of them is standing." Turned again once
    // she has put down four keepers of this place (the notice names the four corners), while the
    // wizard's door behind it is still shut.
    let keepers = {
        let mut k: Vec<jane_core::UnitDefId> =
            downed.iter().map(|&(d, _)| d).filter(|&d| Some(d) != crate::crawl::boss_of(v.zone())).collect();
        k.sort();
        k.dedup();
        k.len()
    };
    let shut = sense::prop_named(v, "gate_wizard").is_some_and(|g| g.locked);
    // (Or the hall says so: "A flame stands up in each corner of the hall. Nobody lit them." A
    // crawl begun again after a fall has not seen the four fall.)
    let flames = ["hall_flame_a", "hall_flame_b", "hall_flame_c", "hall_flame_d"]
        .iter()
        .all(|n| sense::prop_named(v, n).is_some_and(|p| p.on));
    if (keepers >= 4 || flames) && shut {
        if let Some(seal) = sense::prop_named(v, "wizard_seal").filter(|p| reach.beside(p)) {
            out.push((
                1,
                sense::to_prop(seal, me),
                Try::Tactic(seal.id.get()),
                Task::Use(crate::task::UseProp::new(seal.id)),
            ));
        }
    }
    // A chest locked with no keyhole (the lock-in's ornate chest: "Quiet. The ornate chest
    // clicks.") opens for whoever stands in its room when the room is done with her: she goes
    // and stands by it, and what the room has for her comes then.
    for p in v.props() {
        let Some(s) = v.prop_spawn(p) else { continue };
        if p.hidden || !p.locked || s.key_tag.is_some() || s.loot.is_empty() || !door_of_none(v, p) || !reach.beside(p)
        {
            continue;
        }
        let at = sense::prop_centre(p);
        out.push((4, sense::to_prop(p, me), Try::Prop(p.id), Task::Walk { to: at, near: Fx::from_px(24) }));
    }
    // What stands rooted over something and does not stand up again (the garden's flower over
    // the scroll): put down before the rest of the room is looked at, from as far as her bolt
    // carries (it cannot reach her there).
    let shut: Vec<Vec2> = v
        .props()
        .filter(|p| {
            p.locked && door_of_none(v, p) && v.prop_spawn(p).is_some_and(|s| s.talk.is_some() || !s.loot.is_empty())
        })
        .map(sense::prop_centre)
        .collect();
    for u in sense::enemies(v) {
        let d = cat.combat.unit(u.def);
        let rooted = d.walk.0 == 0 && d.run.0 == 0;
        let guards = shut.iter().any(|&c| dist(c, u.pos) <= i64::from(8 * CELL_FX));
        // (Only once she is in its room: from outside, the glass or a shut gate takes the bolts.)
        let with_her = room_of(v, me).is_some_and(|r| r.contains(u.pos.cell().0, u.pos.cell().1));
        let guard = guards && d.respawn.0 == 0 && !d.boss && d.bait.is_none();
        // And what sprays from the spot, in the room she is in (the rat room's cactuses): put
        // down from the far edge of its reach before anything else in the room is gone to. Its
        // fan thins with the distance; walked past close, a spray is half of her. It stays down
        // while she is in its sight.
        let sprayer = sprays(u.def) && let_be(u.def);
        if u.alive && rooted && with_her && (guard || sprayer) && reach.near(u.pos, 16) {
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
        let job = Feed {
            unit: u.id,
            bait,
            path,
            step: 0,
            at,
            face,
            dir,
            to,
            stage: 0,
            t: 0,
            hide: None,
            held: 0,
            meat: None,
        };
        out.push((1, dist(me, u.pos), Try::Fight(u.id), Task::Burial(Job::Feed(job))));
    }
    out
}

/// What the Burial asks that comes before the task she is on (a creature to feed has come into
/// her view, calm, and she has its bait and a place to throw it from): the tries, for the crawl
/// to weigh as it weighs its own (one tried too often is not cut in for).
///
/// Hurt, with nothing on her, the fire comes before whatever she was walking to: the next
/// thing down here may be a keeper's room that shuts behind her.
pub fn cuts_in(v: &View<'_>, cx: &Ctx, reach: &Reach, task: &Task, what: Try) -> Vec<Try> {
    if matches!(task, Task::Burial(_)) || matches!(what, Try::Tactic(_)) {
        return Vec::new();
    }
    let mut out: Vec<Try> = feeds(v, cx).into_iter().map(|(_, _, w, _)| w).collect();
    let calm = !sense::enemies(v).into_iter().any(|u| crate::fight::on_me(v, u) && crate::fight::can_reach(v, u));
    if calm && worn(v) {
        let cat = jane_data::catalog();
        out.extend(
            v.props()
                .filter(|p| !p.hidden && cat.story.prop(p.def).rest && reach.beside(p) && !shot_at(v, cx, p))
                .filter(|p| v.prop_spawn(p).is_some_and(|s| s.talk.is_some()))
                .map(|p| Try::Tactic(p.id.get())),
        );
    }
    out
}

/// Worn enough to go back to a fire when nothing is on her: down to three quarters of her
/// health, or two fifths of her mana. Every room left down here is a fight, most of them shut behind
/// her, and a fall costs the walk back and the keeper whole again; the fire costs only the walk.
fn worn(v: &View<'_>) -> bool {
    let me = v.body();
    let mp_max = jane_sim::units::max_mp(me).0.max(1);
    sense::hp_permille(me) < 750 || i64::from(me.mp.0) * 1000 / i64::from(mp_max) < 400
}

/// Whether she is getting anywhere: she has moved three cells, or something about her has lost
/// health, within the last fifteen seconds. Standing still with nothing going down is her task
/// and a fight pulling two ways (a thing after her through a wall, a keeper at the end of its
/// leash); then the crawl drops the task and she lets be for fifteen seconds whatever is not at
/// her elbow.
#[derive(Debug, Default)]
pub struct Watch {
    anchor: Option<Vec2>,
    since: u32,
    hp: i64,
    calm_until: u32,
}

impl Watch {
    /// Fifteen seconds without getting anywhere (once each time).
    pub fn stalled(&mut self, v: &View<'_>) -> bool {
        const STALL: u32 = 15 * 60;
        let me = v.body();
        let f = v.frame();
        let near: i64 = sense::enemies(v)
            .into_iter()
            .filter(|u| u.alive && dist(u.pos, me.pos) <= i64::from(16 * CELL_FX))
            .map(|u| i64::from(u.hp.0))
            .sum();
        let moved = self.anchor.is_none_or(|a| dist(a, me.pos) > i64::from(3 * CELL_FX));
        let hurt = near < self.hp;
        self.hp = near;
        if moved || hurt || !me.alive || v.dialogue().is_some() {
            self.anchor = Some(me.pos);
            self.since = f;
            return false;
        }
        if f.saturating_sub(self.since) < STALL {
            return false;
        }
        self.since = f;
        self.calm_until = f + STALL;
        true
    }

    /// Letting be what is not at her elbow, after a stall.
    pub fn calm(&self, v: &View<'_>) -> bool {
        v.frame() < self.calm_until
    }
}

/// Is `id` at her elbow (inside three cells)?
pub fn at_elbow(v: &View<'_>, id: UnitId) -> bool {
    v.unit(id).is_some_and(|u| crate::fight::gap(v.body(), u) < i64::from(3 * CELL_FX))
}

/// Done down here: Goldskin is down and what his box held (the ball) is in her bag. The rest of
/// the Burial fills again behind her; nothing in it is worth the walk.
pub fn done(v: &View<'_>, downed: &[(jane_core::UnitDefId, u32)]) -> bool {
    let boss = crate::crawl::boss_of(v.zone());
    boss.is_some_and(|b| downed.iter().any(|&(d, _)| d == b)) && holds(v, sense::item("the_ball")) > 0
}

/// Is this fire in the line of something rooted that shoots (a statue beside the orchard's
/// hearth, on some seeds)? She would wake there under its fire after a fall, and fall again.
pub fn shot_at(v: &View<'_>, cx: &Ctx, p: &jane_sim::Prop) -> bool {
    let c = sense::prop_centre(p);
    keep_off(v, cx).iter().any(|&(o, r, k)| k > 0 && dist(o, c) <= r && v.sight(o, c))
}

/// Whole enough to walk into a keeper's room: nine tenths of her health and four fifths of her
/// mana. The room shuts behind her and the keeper does not tire; what she brings in is all she
/// has.
fn ready(v: &View<'_>) -> bool {
    let me = v.body();
    let mp_max = jane_sim::units::max_mp(me).0.max(1);
    sense::hp_permille(me) >= 900 && i64::from(me.mp.0) * 1000 / i64::from(mp_max) >= 800
}

/// Is there a small snake here still to feed (alive, and she has its bait)? One she saw and has
/// walked away from (it sleeps out of her sight) is remembered.
fn feeding(v: &View<'_>, cx: &Ctx) -> bool {
    let cat = jane_data::catalog();
    let awake =
        sense::enemies(v).into_iter().any(|u| cat.combat.unit(u.def).bait.is_some_and(|b| holds(v, b) > 0) && u.alive);
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
    let cat = jane_data::catalog();
    let me = v.body();
    // Held (a web, a stun) she cannot get away: stepping aside or backing off at a quarter pace is
    // standing still under the bites. She fights from where she is.
    let held = me.statuses.iter().any(|st| st.until > v.tick() && cat.combat.effect(st.effect).speed.0 < 500);
    // A pool laid by her (the queen's web) is got out of before it takes hold, away from it and
    // from the keeper on her: standing in it is standing still under the bites for as long as
    // it lasts. Once it has her she fights from where she is. Then what is thrown at her is
    // stepped out of.
    if !held {
        if let Some(f) = out_of_pool(v, &cx.nav.keep_off) {
            return Some(Some(Act::hold(f)));
        }
        if let Some(f) = dodge(v, &cx.nav.keep_off) {
            return Some(Some(Act::hold(f)));
        }
        // With nothing at her elbow the ordinary fight steps out of a pool.
        if crate::fight::tell_under(v).is_some() {
            return None;
        }
    }
    // A bite winding up at her: out of its reach (a hop on its last ticks). What is thrown is
    // left to the room's own rules above (its cover, its pools).
    let bite = crate::fight::tell_on_me(v).is_some_and(|(_, w, _)| cat.combat.spell(w.spell).kind == SpellKind::Melee);
    if !held && bite {
        if let Some(a) = crate::fight::dodge_tell(v, cx) {
            return Some(Some(a));
        }
    }
    if let Some(c) = crate::fight::eat(v) {
        return Some(Some(Act::press(c)));
    }
    let t = v.unit(id).filter(|t| t.alive)?;
    if under_the_torch(v, t, task) {
        return Some(None);
    }
    if let Some(a) = held_off(v, cx, t, id) {
        return Some(a);
    }
    let now = v.tick();
    let td = cat.combat.unit(t.def);
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    let aim = InputFrame { aim: Some(dir), ..InputFrame::IDLE };
    let g = gap(me, t);
    cx.fight.target = Some(id);
    cx.foes.insert(id);
    // What she brought, against a keeper, one at a time (poison or frost on her is no reason not).
    // Not the mana shield: it pays for every bite out of the mana her bolts are, and a keeper
    // down here outlasts what she carries of that.
    let helped = me.statuses.iter().any(|st| !cat.combat.effect(st.effect).harmful && st.until > now);
    if td.boss && g < i64::from(14 * CELL_FX) && !helped {
        for name in ["potion_stoneskin", "potion_lifesteal"] {
            let p = sense::item(name);
            if holds(v, p) > 0 && item_ready(me, p, now) {
                return Some(Some(Act::press(Command::Item(p))));
            }
        }
    }
    let mobile = td.walk.0 > 0 || td.run.0 > 0;
    // A keeper that swings is got away from before it is at her elbow and before anything is
    // cast, whatever else she is fighting: a cast holds her still for half a second, and what
    // they swing stuns.
    let elbow = sense::enemies(v).into_iter().find(|u| {
        let d = cat.combat.unit(u.def);
        u.alive
            && d.boss
            && (d.walk.0 > 0 || d.run.0 > 0)
            && d.book.iter().any(|&s| cat.combat.spell(s).kind == SpellKind::Melee)
            && gap(me, u) < i64::from(5 * CELL_FX)
    });
    if let Some(k) = elbow.filter(|_| !held) {
        let kd = cat.combat.unit(k.def);
        let leash = i64::from(kd.leash.0);
        // (Only while she is inside it: out past it already, the way back is past the keeper.)
        let tether = (leash > 0).then_some((k.home, leash * 2 / 3)).filter(|&(h, r)| dist(me.pos, h) <= r);
        let room = room_of(v, k.home).filter(|r| kd.controller == jane_data::Controller::Snake && inside(r, me.pos));
        // Out past its tether already, backing off further only leads it home to mend (the
        // Spider's leash is twelve metres): its blows are dodged as they wind up instead.
        let leads_home = leash > 0 && tether.is_none() && kd.controller != jane_data::Controller::Snake;
        if !leads_home {
            if let Some(f) = retreat(v, cx, k.pos, tether, room, slips_past(k.def)) {
                return Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f })));
            }
        }
    }
    // Proof against everything she has (Goldskin's gilding), with a fire to light in its room
    // that softens what stands in it: the fire first (DUNGEONS.md §3.5: "Fire literally opens
    // him").
    if let Some(a) = soften(v, t) {
        return Some(Some(a));
    }
    // Until then a bolt at him is a fifth of a bolt: the mana is kept for when the fire is on him,
    // while there is a fire in his room left to light. With every one burning (he puts them out
    // as he turns), or with him nearly down, a fifth is better than none.
    let unlit = room_of(v, t.home).is_some_and(|r| {
        v.props().any(|p| {
            cat.story.prop(p.def).answers == Some(jane_data::Answers::Fire)
                && !p.on
                && r.contains(i32::from(p.cell.x), i32::from(p.cell.y))
        })
    });
    let gilded = gilded(v, t) && unlit && sense::hp_permille(t) > 100;
    // What is rooted and stands up again is let shoot from beyond its reach, and so is anything
    // rooted while there is feeding to do: stopping for it is standing in its line longer. Inside
    // its reach, it is put down (it stays down while she is in its sight): walking on under a
    // cactus's thorns is how she falls.
    let inside_reach = {
        let range = td.book.iter().map(|&s| cat.combat.spell(s).range.0).max().unwrap_or(0);
        g <= i64::from(range)
    };
    if !mobile && (careful || let_be(t.def) && !inside_reach) {
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
        // No such ground, and no clear shot from here: it is let be. Walking at something rooted
        // is standing longer in its line (and a nook it sits in may have no way to it at all).
        if !shot_clear(v, me.pos, t.pos) || g > i64::from(cat.combat.spell(sense::spell("icebolt")).range.0) * 9 / 10 {
            return Some(None);
        }
    }
    // (At her elbow the bolt is out of her hand and into it before any wall: its middle line is
    // enough.)
    let close = g < i64::from(2 * CELL_FX) && v.sight(me.pos, t.pos);
    if !gilded && (close || shot_clear(v, me.pos, t.pos)) {
        if let Some(bolt) = best_bolt(v, t) {
            return Some(Some(Act { frame: aim, cmds: vec![Command::Cast { spell: bolt, on: Some(id) }] }));
        }
    }
    if mobile && g < i64::from(6 * CELL_FX) {
        let leash = i64::from(td.leash.0);
        let tether = (leash > 0).then_some((t.home, leash * 2 / 3)).filter(|&(h, r)| dist(me.pos, h) <= r);
        let leads_home = leash > 0 && tether.is_none() && td.controller != jane_data::Controller::Snake;
        // The snake loses her the moment a wall stands between them, and goes home whole: its
        // room is kept to (the room it was met in, as the map shows it).
        let room = (td.controller == jane_data::Controller::Snake)
            .then(|| room_of(v, t.home))
            .flatten()
            .filter(|r| inside(r, me.pos));
        if let Some(f) = retreat(v, cx, t.pos, tether, room, slips_past(t.def)).filter(|_| !leads_home) {
            return Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f })));
        }
    }
    let melee = sense::spell("melee_player");
    if g <= i64::from(cat.combat.spell(melee).range.0) {
        let cmds = if crate::fight::ready(me, melee, now) {
            vec![Command::Cast { spell: melee, on: Some(id) }]
        } else {
            Vec::new()
        };
        return Some(Some(Act { frame: aim, cmds }));
    }
    if careful {
        // What is not after her (a keeper she is sent to, in its room) is gone to by the
        // ordinary road, which keeps off every calm snake's sight; waiting for it is for ever.
        if !crate::fight::on_me(v, t) {
            return Some(None);
        }
        // At her elbow and nowhere to go: whatever she has, from where she stands.
        if g < i64::from(2 * CELL_FX) {
            if let Some(bolt) = best_bolt(v, t) {
                return Some(Some(Act { frame: aim, cmds: vec![Command::Cast { spell: bolt, on: Some(id) }] }));
            }
        }
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
    // What walks comes to her if it can: only a keeper that throws from out of her reach is gone
    // to (Goldskin keeps his distance). Inside her reach with no clear shot, or something after
    // her that cannot get round to her, is not gone round to: her feet and her task (or her
    // backing off) pulling two ways is standing still for good.
    let throws = td.book.iter().any(|&s| cat.combat.spell(s).kind == SpellKind::Bolt);
    let go_to = if g > reach { td.boss && throws } else { shot_clear(v, me.pos, t.pos) };
    if !go_to {
        return Some(None);
    }
    if g > reach {
        return match cx.nav.go(v, t.pos, Fx(reach as i32), true) {
            Go::Walk(f) => Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f }))),
            _ => None,
        };
    }
    Some(Some(Act::hold(aim)))
}

/// A keeper that walks and throws as well as swings (Goldskin), whose blow reaches no further
/// than a quarter cell: slipped past rather than stood against in a corner.
fn slips_past(def: jane_core::UnitDefId) -> bool {
    let cat = jane_data::catalog();
    let d = cat.combat.unit(def);
    d.boss
        && d.controller != jane_data::Controller::Snake
        && (d.walk.0 > 0 || d.run.0 > 0)
        && d.book.iter().any(|&s| cat.combat.spell(s).kind == SpellKind::Bolt)
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
    slip: bool,
) -> Option<InputFrame> {
    let me = v.body().pos;
    let off = cx.nav.keep_off.clone();
    let banned = |c: Vec2| off.iter().any(|&(o, r, k)| k == 0 && dist(o, c) <= r && v.sight(o, c));
    let price = |c: Vec2| {
        off.iter()
            .filter(|&&(o, r, k)| k > 0 && dist(o, c) <= r && v.sight(o, c))
            .map(|&(_, _, k)| i64::from(k))
            .sum::<i64>()
    };
    let strayed = strayed_rooms(v);
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    let mut best: Option<(i64, (i32, i32))> = None;
    while let Some((c, steps)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        let inside = tether.is_none_or(|(home, r)| dist(at, home) <= r)
            && room.is_none_or(|r| r.x < c.0 && c.0 < r.right() - 1 && r.y < c.1 && c.1 < r.bottom() - 1)
            && !strayed.iter().any(|r| r.contains(c.0, c.1));
        if inside && steps > 0 {
            // Open ground over a corner: backed into one, she is caught (what is kited is slow,
            // but it is between her and every way out).
            let open = (-3..=3)
                .flat_map(|j| (-3..=3).map(move |i| (i, j)))
                .filter(|&(i, j)| crate::nav::walkable(v, c.0 + i, c.1 + j))
                .count();
            let score = dist(at, from) - i64::from(steps) * i64::from(CELL_FX) / 3 - price(at) * i64::from(CELL_FX) / 4
                + open as i64 * i64::from(CELL_FX) / 2;
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
    // (Past one that only reaches a quarter cell beyond its own bulk she slips, at three times
    // its pace, by any way not straight at it: stood in a corner, Goldskin took all of her.)
    let cone = if slip { 5_000 } else { 11_000 };
    match cx.nav.go(v, to, Fx::from_px(3), true) {
        // Cornered, the way out is past it: no backing off through it (she fights instead).
        Go::Walk(f) if f.mv_dir.diff(jane_core::angle::iatan2(from.y.0 - me.y.0, from.x.0 - me.x.0)).abs() < cone => {
            None
        }
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
        // A small snake is kept out of the notice of whether or not she has its bait: without
        // it, walking into its notice is its spit (she comes back with the meat).
        let baited = d.bait.is_some();
        let fed = d.bait.is_some_and(|b| holds(v, b) > 0);
        if !baited && !rooted {
            continue;
        }
        let (r, cost) = if baited {
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
                // A baited one seen is kept off by its reach as the sim has it (her growth against it).
                Some(u) if u.alive => {
                    let r = if baited {
                        r.max(aggro_of(v, u) + i64::from(d.bounds.0) + me + i64::from(CELL_FX))
                    } else {
                        r
                    };
                    out.push((u.pos, r, cost));
                }
                Some(_) => {}
                None => out.push((pos, r, cost)),
            }
        }
    }
    // A keeper's room whose keeper is out of it (after her in the corridor) is not walked into,
    // nor through: it shuts on whoever steps in, and the keeper would be shut outside for good.
    for r in strayed_rooms(v) {
        let c = Vec2::centre(r.x + r.w / 2, r.y + r.h / 2);
        out.push((c, i64::from(r.w.max(r.h)) * i64::from(CELL_FX) / 2 + i64::from(CELL_FX), 0));
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
        // It falls short of her (its range runs out first): nothing to step from.
        let speed = i64::from(jane_core::num::isqrt(v2 as u64));
        if speed * t > i64::from(p.left.0) + speed + body {
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
            (crate::nav::walkable(v, tx, ty)
                && !off.iter().any(|&(o, r, k)| k == 0 && dist(o, to) <= r && v.sight(o, to)))
            .then(|| InputFrame { sprint: true, ..crate::nav::stick(me.pos, to, true) })
        };
        let Some(f) = step(px, py).or_else(|| step(-px, -py)) else { continue };
        if soonest.as_ref().is_none_or(|s| t < s.0) {
            soonest = Some((t, f));
        }
    }
    soonest.map(|(_, f)| f)
}

/// Out of a pool of theirs she is in (or at the edge of) while a keeper that walks is on her:
/// of the eight ways two and a half cells off with open floor all the way (nobody standing in
/// it), the one farthest from its middle and from the keeper (none open: she fights from where
/// she is).
fn out_of_pool(v: &View<'_>, off: &[(Vec2, i64, u32)]) -> Option<InputFrame> {
    let me = v.body();
    let body = i64::from(jane_data::catalog().combat.unit(me.def).bounds.0);
    let now = v.tick();
    let pool = v
        .grounds()
        .iter()
        .filter(|g| g.faction != me.faction && g.until > now)
        .find(|g| dist(g.pos, me.pos) <= i64::from(g.radius.0) + body + i64::from(CELL_FX))?
        .pos;
    // Only with a keeper that walks on her, near: a pool is only worth the steps when the bites
    // are what it holds her for (the web spinners' webs, laid all over, are walked through, and
    // what is small is put down from where she stands).
    let cat = jane_data::catalog();
    let foe = sense::enemies(v)
        .into_iter()
        .find(|u| {
            let d = cat.combat.unit(u.def);
            u.alive
                && d.boss
                && crate::fight::on_me(v, u)
                && (d.walk.0 > 0 || d.run.0 > 0)
                && crate::fight::gap(me, u) < i64::from(6 * CELL_FX)
        })?
        .pos;
    let step = Fx(5 * CELL_FX / 2);
    (0..8u16)
        .filter_map(|i| {
            let a = Angle(i * 8192);
            let to = me.pos + jane_core::angle::along(a, step);
            // Every half cell of the way walkable (not through the wall to the corridor beyond).
            let clear = (1..=5).all(|k| {
                let p = me.pos + jane_core::angle::along(a, Fx(k * CELL_FX / 2));
                let (x, y) = p.cell();
                crate::nav::walkable(v, x, y)
                    && !v.units_in(sense::everywhere(v)).map(|w| w.unit).any(|w| {
                        w.alive && w.id != me.id && dist(w.pos, p) < body + i64::from(cat.combat.unit(w.def).bounds.0)
                    })
            });
            let ok = clear && !off.iter().any(|&(o, r, k)| k == 0 && dist(o, to) <= r && v.sight(o, to));
            ok.then(|| (dist(to, pool) + dist(to, foe), i, to))
        })
        .max_by_key(|&(score, i, _)| (score, std::cmp::Reverse(i)))
        .map(|(_, _, to)| InputFrame { sprint: true, ..crate::nav::stick(me.pos, to, true) })
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
            cat.combat.unit(u.def).bait.is_some_and(|b| holds(v, b) == 0)
                && crate::fight::on_me(v, u)
                && v.sight(u.pos, me)
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
    let shooters = shooters(v, t);
    let off = &cx.nav.keep_off;
    // A spray is stood back from, to where its fan has thinned (two thirds of her bolt's reach),
    // if there is such ground; else the nearest.
    let far = if sprays(t.def) { reach * 5 / 6 } else { 0 };
    let good = |at: Vec2, far: i64| {
        (far..=reach).contains(&dist(at, t.pos))
            && shot_clear(v, at, t.pos)
            && shooters.iter().all(|&(o, r)| dist(o, at) > r || !v.sight(o, at))
            && !off.iter().any(|&(o, r, k)| k == 0 && dist(o, at) <= r && v.sight(o, at))
    };
    let me = v.body().pos;
    if good(me, far) {
        return Some(me);
    }
    let start = me.cell();
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    seen.insert(start);
    q.push_back((start, 0u32));
    let mut near = None;
    while let Some((c, n)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if n > 0 && good(at, far) {
            return Some(at);
        }
        if near.is_none() && good(at, 0) {
            near = Some(at);
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
    near
}

/// The other rooted things that shoot, and how far (for `clean_spot`).
fn shooters(v: &View<'_>, t: &Unit) -> Vec<(Vec2, i64)> {
    let cat = jane_data::catalog();
    sense::enemies(v)
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
        .collect()
}

/// Not gone looking for down here: whatever stands up again ten seconds after she has gone (all
/// but the keepers). It is fought when it comes at her; hunting it is a round that never ends.
/// Nor is a shade: it waits at the edge of warm light for her to step out of it, so going after
/// it from the great torch's light is a stand-off that never ends either.
pub fn not_hunted(def: jane_core::UnitDefId) -> bool {
    let d = jane_data::catalog().combat.unit(def);
    d.respawn.0 > 0 && !d.boss || d.shuns_light
}

/// Pushing something that gives warm light (the great torch) with a shade after her: she keeps
/// pushing. It cannot step into the light she is in, and turning to fight it from there is the
/// stand-off `not_hunted` keeps her out of.
fn under_the_torch(v: &View<'_>, t: &Unit, task: Option<&Task>) -> bool {
    let cat = jane_data::catalog();
    let Some(Task::Push(p)) = task else { return false };
    let warm = v.prop(p.prop).and_then(|q| cat.story.prop(q.def).light).is_some_and(|l| !l.cold);
    warm && cat.combat.unit(t.def).shuns_light
}

/// Does warm light (a lit brazier, the great torch; not a cold torch) cover the point, a cell in
/// from its edge? The sim's rule (`light::lit_at`), drawn in a little: a shade at the very edge
/// still reaches over it.
fn warm_at(v: &View<'_>, at: Vec2) -> bool {
    let cat = jane_data::catalog();
    v.props().any(|p| {
        v.light_showing(p).is_some_and(|l| {
            let r = i64::from(l.radius.0) - i64::from(CELL_FX);
            !l.cold && r > 0 && dist(jane_sim::light::prop_centre(cat.story.prop(p.def), p), at) <= r
        })
    })
}

/// A shade kept off her by the warm light she stands in (the great torch on its plate, with the
/// shades crowding its edge): it will not come, so backing off from it or waiting for it is a
/// stand-off for ever. It is shot from inside the light, from where she is or from ground in the
/// light with a clear line to it; with no such ground, or no bolt to hand, it is let be.
/// (`fight`'s answer: `None` not this one's to say, `Some(None)` let it be.)
#[allow(clippy::option_option)]
fn held_off(v: &View<'_>, cx: &mut Ctx, t: &Unit, id: UnitId) -> Option<Option<Act>> {
    let cat = jane_data::catalog();
    let me = v.body();
    if !cat.combat.unit(t.def).shuns_light || !warm_at(v, me.pos) {
        return None;
    }
    let bolt = best_bolt(v, t)?;
    let reach = i64::from(cat.combat.spell(bolt).range.0) * 9 / 10;
    let dir = jane_core::angle::iatan2(t.pos.y.0 - me.pos.y.0, t.pos.x.0 - me.pos.x.0);
    let good = |at: Vec2| dist(at, t.pos) <= reach && warm_at(v, at) && shot_clear(v, at, t.pos);
    if good(me.pos) {
        let aim = InputFrame { aim: Some(dir), ..InputFrame::IDLE };
        return Some(Some(Act { frame: aim, cmds: vec![Command::Cast { spell: bolt, on: Some(id) }] }));
    }
    let start = me.pos.cell();
    let mut seen = BTreeSet::from([start]);
    let mut q = VecDeque::from([(start, 0u32)]);
    while let Some((c, n)) = q.pop_front() {
        let at = Vec2::centre(c.0, c.1);
        if n > 0 && good(at) {
            return match cx.nav.go(v, at, Fx::from_px(4), true) {
                Go::Walk(f) => Some(Some(Act::hold(InputFrame { aim: Some(dir), ..f }))),
                _ => Some(None),
            };
        }
        if n >= 12 {
            continue;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nb = (c.0 + dx, c.1 + dy);
            if crate::nav::walkable(v, nb.0, nb.1) && seen.insert(nb) {
                q.push_back((nb, n + 1));
            }
        }
    }
    Some(None)
}

/// Does it throw a fan of many (the cactus's needles)?
fn sprays(def: jane_core::UnitDefId) -> bool {
    let cat = jane_data::catalog();
    cat.combat.unit(def).book.iter().any(|&s| {
        let d = cat.combat.spell(s);
        d.kind == SpellKind::Bolt && d.count > 1 && d.fan.0 != u16::MAX
    })
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
        && d.book.iter().all(|&s| cat.combat.spell(s).kind == SpellKind::Bolt)
}

/// Will a bolt from `a` fly to `b`? Walls stop it as they stop sight, and so does anything solid
/// standing in the way (a crate, a coffin, a chest: `BLOCK_SHOT`), which sight goes over. The
/// line is walked a quarter cell at a time.
pub fn shot_clear(v: &View<'_>, a: Vec2, b: Vec2) -> bool {
    let d = dist(a, b).max(1);
    let step = i64::from(CELL_FX) / 4;
    let n = (d / step).max(1);
    let start = a.cell();
    let end = b.cell();
    // The bolt has a body: the line either side of its middle, a quarter cell off, is walked too.
    let (px, py) = ((-i64::from(b.y.0 - a.y.0) * step / d) as i32, (i64::from(b.x.0 - a.x.0) * step / d) as i32);
    [-1, 0, 1].into_iter().all(|side| {
        (1..n).all(|i| {
            let p = Vec2::new(
                Fx(a.x.0 + ((i64::from(b.x.0 - a.x.0) * i) / n) as i32 + side * px),
                Fx(a.y.0 + ((i64::from(b.y.0 - a.y.0) * i) / n) as i32 + side * py),
            );
            let c = p.cell();
            c == start || c == end || v.flags(c.0, c.1) & jane_core::tile::BLOCK_SHOT == 0
        })
    })
}

/// Not a door (a locked thing that is a way somewhere is a gate, not a thing guarded).
fn door_of_none(v: &View<'_>, p: &jane_sim::Prop) -> bool {
    sense::door_of(v, p).is_none() && !jane_data::catalog().story.prop(p.def).gate
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
        u.alive
            && cat.combat.unit(u.def).boss
            && room_of(v, u.home).is_some_and(|r| r.contains(me.cell().0, me.cell().1))
    });
    if let Some(k) = keeper {
        if !matches!(t, Task::Hunt(id) if *id == k.id) {
            return true;
        }
    }
    // Something rooted is not gone after from outside its room: from a doorway the gate or the
    // glass takes the bolts, and rooted, it is in nobody's way but its own room's.
    if let Task::Hunt(id) = t {
        if let Some(u) = v.unit(*id) {
            let d = cat.combat.unit(u.def);
            let (x, y) = u.pos.cell();
            if d.walk.0 == 0 && d.run.0 == 0 && !d.boss && !room_of(v, me).is_some_and(|r| r.contains(x, y)) {
                return true;
            }
        }
    }
    // Meat she threw is the small snake's: it is not picked up again while something that eats
    // it lives.
    if let Task::Pickup { drop, .. } = t {
        if let Some(d) = v.drops().iter().find(|d| d.id == *drop) {
            if sense::enemies(v).into_iter().any(|u| u.alive && cat.combat.unit(u.def).bait == Some(d.item)) {
                return true;
            }
        }
    }
    // A plain thing (the rats' meat) left in a room she has walked out of is not walked back
    // for: the room has filled again behind her.
    if let Task::Pickup { drop, .. } = t {
        if let Some(d) = v.drops().iter().find(|d| d.id == *drop) {
            let def = cat.combat.item(d.item);
            let plain = !def.story && !def.usable && def.opens.is_none();
            let here = room_of(v, me).is_some_and(|r| r.contains(d.pos.cell().0, d.pos.cell().1))
                || dist(me, d.pos) <= i64::from(8 * CELL_FX);
            if plain && !here {
                return true;
            }
        }
    }
    // A torch that only gives light (nothing written on it, nothing it does) is not lit: the
    // walk to it is through rooms that fill again.
    let lamp = |p: &jane_sim::Prop| {
        let d = cat.story.prop(p.def);
        d.light.is_some()
            && !d.gate
            && sense::door_of(v, p).is_none()
            && v.prop_spawn(p).is_some_and(|s| s.use_list.is_none() && s.talk.is_none())
    };
    let lit = match t {
        Task::Aim { at, .. } => v.props().any(|p| lamp(p) && dist(sense::prop_centre(p), *at) <= i64::from(CELL_FX)),
        Task::WorldCast { prop, .. } => v.prop(*prop).is_some_and(lamp),
        _ => false,
    };
    if lit {
        return true;
    }
    // A keeper that has come out after her is not followed back into its room: the room shuts
    // on whoever walks in, and it would be shut outside.
    if let Some(at) = task_point(v, t) {
        let (x, y) = at.cell();
        if strayed_rooms(v).iter().any(|r| r.contains(x, y)) {
            return true;
        }
    }
    // A keeper's room is not walked into short of breath (`ready`), nor the ground before a
    // keeper that walks: the fire first.
    if !ready(v) {
        if let Some(at) = task_point(v, t) {
            let (x, y) = at.cell();
            let live: Vec<&Unit> =
                sense::enemies(v).into_iter().filter(|u| u.alive && cat.combat.unit(u.def).boss).collect();
            if live
                .iter()
                .filter_map(|u| room_of(v, u.home))
                .any(|r| r.contains(x, y) && !r.contains(me.cell().0, me.cell().1))
            {
                return true;
            }
            // Nor the room at its door: the queen comes out to meet her in the web. (In its room
            // already, with the door shut behind her, there is no fire to go back to.)
            let near = |u: &&&Unit| {
                dist(u.home, at) <= i64::from(20 * CELL_FX)
                    && dist(u.home, me) > i64::from(20 * CELL_FX)
                    && !room_of(v, u.home).is_some_and(|r| r.contains(me.cell().0, me.cell().1))
            };
            if live.iter().filter(near).any(|u| cat.combat.unit(u.def).walk.0 > 0) {
                return true;
            }
        }
    }
    if sense::knows(v, sense::spell("fireball")) {
        return false;
    }
    let Some(at) = task_point(v, t) else { return false };
    let (x, y) = at.cell();
    let inside = |name: &str| v.sym(name).and_then(|s| v.rect(s)).is_some_and(|r| r.contains(x, y));
    let door =
        sense::prop_named(v, "gate_glasshouse").is_some_and(|g| crate::sense::to_prop(g, at) <= i64::from(2 * CELL_FX));
    inside("burial_glasshouse") || door
}

/// Apples kept for the keepers.
const KEEPERS_FOOD: u32 = 4;

/// May she eat now? Every room down here but a keeper's has the vigil fire behind it, and a
/// keeper's room shuts behind her: the apples are for the keepers, unless she is nearly done
/// for. (Eaten in the trash between, she met the Spider with none and fell with the Spider at a
/// tenth of her health, three times on seed 1.) What is kept is a keeper's worth: past it, or
/// with more than one thing on her, she eats (keeping all of them, she fell a dozen times in the
/// rat room on seed 7 with thirteen apples in her bag).
pub fn may_eat(v: &View<'_>) -> bool {
    let cat = jane_data::catalog();
    let food = ["apple", "grape"].into_iter().map(|n| sense::holds(v, sense::item(n))).sum::<u32>();
    let crowd = sense::enemies(v).iter().filter(|u| crate::fight::on_me(v, u)).count() > 1;
    sense::hp_permille(v.body()) < 200
        || food > KEEPERS_FOOD
        || crowd
        || sense::enemies(v)
            .iter()
            .any(|u| cat.combat.unit(u.def).boss && (crate::fight::on_me(v, u) || u.combat == CombatState::Combat))
}

/// The snake, with her in its room: fought out, never fled. It goes home whole the moment it
/// loses her, and its room's gates stay shut behind her with the key spent: fled at 95 health
/// with it at 500, she stood outside for good (a fall at least opens them again).
pub fn fought_out(v: &View<'_>, t: &Unit) -> bool {
    v.zone() == jane_core::ZoneId::Burial
        && jane_data::catalog().combat.unit(t.def).controller == jane_data::Controller::Snake
        && room_of(v, t.home).is_some_and(|r| in_room(&r, v.body().pos))
}

/// Anywhere in `r`, its edge cells too.
fn in_room(r: &jane_core::Rect, at: Vec2) -> bool {
    let (x, y) = at.cell();
    r.contains(x, y)
}

/// Is `at` inside room `r`, clear of its walls?
fn inside(r: &jane_core::Rect, at: Vec2) -> bool {
    let (x, y) = at.cell();
    r.x < x && x < r.right() - 1 && r.y < y && y < r.bottom() - 1
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

/// The rooms whose keeper is out of them (after her, in the corridor): not to be backed into.
fn strayed_rooms(v: &View<'_>) -> Vec<jane_core::Rect> {
    let cat = jane_data::catalog();
    let me = v.body().pos.cell();
    sense::enemies(v)
        .into_iter()
        .filter(|u| u.alive && cat.combat.unit(u.def).boss)
        .filter_map(|u| {
            let r = room_of(v, u.home)?;
            let (x, y) = u.pos.cell();
            (!r.contains(x, y) && !r.contains(me.0, me.1)).then_some(r)
        })
        .collect()
}

/// Does `t` turn aside most of every bolt she has (four fifths or more), as it stands now?
fn gilded(v: &View<'_>, t: &Unit) -> bool {
    let cat = jane_data::catalog();
    let now = v.tick();
    ["icebolt", "fireball", "spark"]
        .map(|n| cat.combat.spell(sense::spell(n)).school)
        .iter()
        .all(|&s| jane_sim::status::resist_factor(t, s, now) <= 200)
}

/// A fireball at an unlit brazier in `t`'s room when `t` turns aside most of what she has:
/// the brazier's use (read off its row, as its label and look would tell a player) softens
/// what stands in the room, and a softened thing takes everything.
fn soften(v: &View<'_>, t: &Unit) -> Option<Act> {
    let cat = jane_data::catalog();
    let fire = sense::spell("fireball");
    let me = v.body();
    let now = v.tick();
    if !sense::knows(v, fire) || !crate::fight::ready(me, fire, now) {
        return None;
    }
    if !gilded(v, t) {
        return None;
    }
    let room = room_of(v, t.home)?;
    let range = i64::from(cat.combat.spell(fire).range.0) * 9 / 10;
    let brazier = v
        .props()
        .filter(|p| {
            let d = cat.story.prop(p.def);
            d.answers == Some(jane_data::Answers::Fire)
                && !p.on
                && room.contains(i32::from(p.cell.x), i32::from(p.cell.y))
        })
        .filter(|p| {
            let c = sense::prop_centre(p);
            dist(me.pos, c) <= range && (shot_clear(v, me.pos, c) || crate::crawl::first_seen_is(v, me.pos, c, p))
        })
        .min_by_key(|p| (dist(me.pos, sense::prop_centre(p)), p.id))?;
    let c = sense::prop_centre(brazier);
    let dir = jane_core::angle::iatan2(c.y.0 - me.pos.y.0, c.x.0 - me.pos.x.0);
    Some(Act {
        frame: InputFrame { aim: Some(dir), ..InputFrame::IDLE },
        cmds: vec![Command::Cast { spell: fire, on: None }],
    })
}
