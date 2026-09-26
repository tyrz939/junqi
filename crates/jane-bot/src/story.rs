//! The quest log read as objectives: what the Reader and the Rusher do in the county, the house
//! and the cellar (VERIFICATION.md §2 L3, §4.1).
//!
//! Each frame, in order: answer a conversation; fight what is fighting her (or what the plan
//! sent her after); eat when low; carry on with the task in hand (dropped for a fire when she is
//! low with nothing to eat, or when she has been chased off it three times); else choose:
//!
//! 0. **A fire or a bed** when low with nothing to eat (the nearest she has not been chased off).
//! 1. **Hand in** a quest whose every step is done, where content says it is taken back: a
//!    trigger, a talker (seen here or remembered elsewhere), or a thing to read.
//! 2. **A step** of a quest in the log: a place (a trigger's rect, or a thing that marks it), a
//!    kill (units of the kind in sight, else where she saw one standing, else the dungeon the
//!    text names), a thing to hold (on the ground, in a chest, made at a bench from what she
//!    holds, or dropped by what the text says to kill).
//! 3. **What she passes**: someone to talk to since the log last changed; a thing to read or
//!    open; back to a quest-giver she met, once the log has moved. The Reader stops for every
//!    sign, note and chest near her way and sits at a fire it passes after a fight; the Rusher
//!    only for what teaches or grows her (and, indoors, a note that gives a quest), and talks
//!    only to whoever can give or take back one.
//!
//! Nearest first, with one weight: a quest someone is waiting on (it goes back to a person or a
//! place, [`someone_waits`]) counts at half its distance, so the dog's errands come before the
//! lost property book's. Where the target is in another zone she walks to a door that leads there
//! (through the county, or the house for the cellar; of two doors, one not yet taken). In a
//! dungeon, a target she cannot walk to is behind something: the [`crawl`](crate::crawl)'s next
//! thing is done instead, and with nothing to do on this side she goes out and in by another way.
//!
//! An objective that fails, or is reached four times and does not count, is set aside for a
//! while (longer each time; logged from the second); chased off the fields she keeps to the road from then on; after
//! a death what was set aside is tried again (she wakes whole). When nothing is left she stops.
//!
//! What the text says is read structurally (P4b's Reader matches landmark names in English): a
//! step's target is the catalog's own name for it, and a zone is found in the text by its name.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use jane_core::action::Action;
use jane_core::num::CELL_FX;
use jane_core::{Fx, ItemId, NameId, QuestId, UnitDefId, Vec2, ZoneId};
use jane_data::ReqTarget;
use jane_sim::View;
use jane_sim::ids::{PropId, UnitId};

use crate::nav::{dist, walkable};
use crate::sense::{self, doors_to, holds, talkers, to_prop, tree_has, units_of};
use crate::task::{Ctx, Status, Task, UseProp};
use crate::{Act, Mark, Model, fight, talk};

/// An objective, as the log names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Goal {
    HandIn(QuestId),
    Step(QuestId, u8),
    Talk(ZoneId, UnitId),
    Look(ZoneId, PropId),
    /// Mend at a fire or a bed.
    Rest,
    /// In a dungeon with the quest's thing out of reach: what the crawl would do next.
    Explore(crate::crawl::Try),
}

/// Where an objective is, resolved against what she can see and remembers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    Task(Task),
    Fight(UnitId),
    /// Get into this zone.
    Zone(ZoneId),
    /// A point in another zone.
    At(ZoneId, Vec2),
}

#[derive(Debug, Default)]
pub struct Story {
    task: Option<(Task, Goal)>,
    fails: BTreeMap<Goal, u32>,
    /// Set aside until this frame.
    blocked: BTreeMap<Goal, u32>,
    /// The goal last finished, and how often in a row it was chosen again at once.
    last: Option<Goal>,
    again: u32,
    idle: u32,
    done: bool,
    /// Deaths seen, to notice a new one.
    deaths: u32,
    /// In a dungeon, the crawl that finds the way to what the quest wants.
    explorer: Option<crate::crawl::Crawl>,
    /// Times she had backed off a fight when the task in hand began.
    fled_at: u32,
    /// Fires she could not get to (something guards the way).
    bad_fires: std::collections::BTreeSet<PropId>,
}

/// Frames to set a failed objective aside (doubling with each failure).
const SET_ASIDE: u32 = 60 * 30;

/// The dungeons by the name a text would call them, where the text says the thing is in one: a
/// name used to give a direction ("up the quarry track from the mine", "on the mine road near the
/// mine", "past the ruined library") is a landmark on the way, not where the thing is.
pub fn zone_in_text(text: &str) -> Option<ZoneId> {
    let t = text.to_lowercase();
    let named = |w: &str| {
        t.match_indices(w).any(|(i, _)| {
            let before = &t[..i];
            let after = &t[i + w.len()..];
            !(before.ends_with("from the ")
                || before.ends_with("near the ")
                || before.ends_with("past the ")
                || before.ends_with("past the ruined ")
                || after.starts_with(" road"))
        })
    };
    [
        ("cellar", ZoneId::Cellar),
        ("mine", ZoneId::Mine),
        ("burial", ZoneId::Burial),
        ("factory", ZoneId::Factory),
        ("forest", ZoneId::Forest),
        ("library", ZoneId::Library),
        ("museum", ZoneId::Museum),
        ("pipes", ZoneId::Pipes),
        ("school", ZoneId::School),
    ]
    .into_iter()
    .find(|(w, _)| named(w))
    .map(|(_, z)| z)
}

/// The zone a quest's step names in its text, else the quest's description.
fn zone_of_step(q: QuestId, i: usize) -> Option<ZoneId> {
    let cat = jane_data::catalog();
    let def = cat.story.quest(q);
    zone_in_text(cat.text(def.requirements[i].text)).or_else(|| zone_in_text(cat.text(def.description)))
}

impl Story {
    pub fn new() -> Story {
        Story::default()
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// What it is doing, for a debugging line.
    pub fn status(&self) -> String {
        match &self.task {
            Some((t, g)) => format!("{g:?} {t:?}"),
            None => "-".into(),
        }
    }

    /// Every quest objective and where it resolves now, for a debugging line.
    pub fn explain(&self, v: &View<'_>, cx: &Ctx) -> String {
        let cat = jane_data::catalog();
        let mut out = format!("blocked {:?}\n", self.blocked);
        if let Some(ex) = &self.explorer {
            let _ = writeln!(out, "  explorer: {}", ex.why_stuck(v));
        }
        for q in v.quests() {
            if q.ready {
                let _ = writeln!(out, "  hand in {}: {:?}", cat.story.quest(q.quest).id, hand_in(v, cx, q.quest));
            } else {
                for (i, r) in cat.story.quest(q.quest).requirements.iter().enumerate() {
                    if q.count(i) < r.qty {
                        let _ = writeln!(
                            out,
                            "  {} {}: {:?}",
                            cat.story.quest(q.quest).id,
                            i,
                            step(v, cx, q.quest, i, r.target)
                        );
                    }
                }
            }
        }
        out
    }

    pub fn think(&mut self, v: &View<'_>, cx: &mut Ctx, notes: &mut Vec<Mark>) -> Act {
        if let Some(a) = talk::answer(v, cx) {
            return a;
        }
        if !v.body().alive {
            // Whatever she was doing got her killed: something else first, for a while.
            let deaths = v.me().stats.deaths;
            if deaths != self.deaths {
                self.deaths = deaths;
                // She wakes whole: what was set aside for want of health is worth trying again,
                // all but what got her killed, and by the road.
                self.blocked.clear();
                self.bad_fires.clear();
                cx.nav.roads = true;
                if let Some((_, g)) = self.task.take() {
                    notes.push(Mark::Stuck(format!("{}: died on the way", goal_name(v, g))));
                    self.blocked.insert(g, v.frame() + 600);
                }
            }
            self.task = None;
            return Act::idle();
        }
        if let Some(id) = fight::threat(v, cx) {
            if let Some(a) = fight::engage(v, cx, id) {
                return a;
            }
        }
        if let Some(c) = fight::eat(v) {
            return Act::press(c);
        }
        // Low with nothing to eat: whatever she was doing waits for a fire.
        let low = sense::hp_permille(v.body()) < fight::EAT_BELOW && !fight::has_food(v);
        if low && self.open(v, Goal::Rest) && self.task.as_ref().is_some_and(|(_, g)| *g != Goal::Rest) {
            self.task = None;
        }
        for _ in 0..3 {
            // Chased off three times on the way: something guards it; another way, or later.
            if self.task.is_some() && cx.fight.fled >= self.fled_at + 3 {
                if let Some((Task::Use(u), Goal::Rest)) = &self.task {
                    self.bad_fires.insert(u.prop);
                }
                if let Some((_, g)) = self.task.take() {
                    self.set_aside(v, g, "chased off on the way", notes);
                }
                // Straight across the fields was the wrong way: from now on, the road.
                cx.nav.roads = true;
            }
            if let Some((task, goal)) = &mut self.task {
                let goal = *goal;
                match task.tick(v, cx) {
                    Status::Act(a) => return a,
                    Status::Done => {
                        self.task = None;
                        self.fails.remove(&goal);
                        if self.last == Some(goal) {
                            self.again += 1;
                        } else {
                            self.last = Some(goal);
                            self.again = 0;
                        }
                    }
                    Status::Failed(why) => {
                        if let Some((Task::Use(u), Goal::Rest)) = &self.task {
                            self.bad_fires.insert(u.prop);
                        }
                        self.task = None;
                        if let (Goal::Explore(t), Some(ex)) = (goal, self.explorer.as_mut()) {
                            ex.failed(t, &why);
                        } else {
                            self.set_aside(v, goal, &why, notes);
                        }
                    }
                }
            }
            match self.choose(v, cx) {
                Some((Target::Fight(id), _)) => {
                    cx.fight.hunt = Some(id);
                    if let Some(a) = fight::engage(v, cx, id) {
                        return a;
                    }
                }
                Some((Target::Task(t), goal)) => {
                    if self.last == Some(goal) && self.again >= 4 {
                        self.again = 0;
                        self.set_aside(v, goal, "reached, and it did not count", notes);
                        continue;
                    }
                    self.idle = 0;
                    self.task = Some((t, goal));
                    self.fled_at = cx.fight.fled;
                }
                Some((other, goal)) => {
                    // A zone or a point elsewhere: the door that leads there.
                    let (Target::Zone(z) | Target::At(z, _)) = other else { unreachable!() };
                    match route(v, cx, z) {
                        Some(t) => {
                            self.task = Some((t, goal));
                            self.fled_at = cx.fight.fled;
                        }
                        None => self.set_aside(v, goal, &format!("no door toward {}", z.name()), notes),
                    }
                }
                None => {
                    self.idle += 1;
                    if self.idle == 600 {
                        notes.push(Mark::Stuck("nothing left to do".into()));
                    }
                    if self.idle >= 600 && self.blocked.values().all(|&u| u <= v.frame()) {
                        self.done = true;
                    }
                    return Act::idle();
                }
            }
        }
        Act::idle()
    }

    fn set_aside(&mut self, v: &View<'_>, goal: Goal, why: &str, notes: &mut Vec<Mark>) {
        let n = self.fails.entry(goal).or_insert(0);
        *n += 1;
        let wait = SET_ASIDE << (*n).min(6);
        self.blocked.insert(goal, v.frame() + wait);
        if *n >= 2 {
            notes.push(Mark::Stuck(format!("{}: {why}", goal_name(v, goal))));
        }
    }

    fn open(&self, v: &View<'_>, g: Goal) -> bool {
        self.blocked.get(&g).is_none_or(|&until| until <= v.frame())
    }

    fn choose(&mut self, v: &View<'_>, cx: &mut Ctx) -> Option<(Target, Goal)> {
        let cat = jane_data::catalog();
        let here = v.zone();
        let at = v.body().pos;
        // Candidates with a cost: (distance-ish, goal, target). Nearest first; ties by goal.
        let mut best: Option<(i64, Goal, Target)> = None;
        let offer = |cost: i64, g: Goal, t: Target, best: &mut Option<(i64, Goal, Target)>| {
            if best.as_ref().is_none_or(|(c, bg, _)| (cost, g) < (*c, *bg)) {
                *best = Some((cost, g, t));
            }
        };
        let cost_of = |t: &Target| -> i64 {
            match t {
                Target::Task(Task::Walk { to, .. }) => dist(at, *to),
                Target::Task(Task::Use(u)) => v.prop(u.prop).map_or(0, |p| to_prop(p, at)),
                Target::Task(Task::Talk { unit, .. }) | Target::Fight(unit) => {
                    v.unit(*unit).map_or(0, |u| dist(at, u.pos))
                }
                Target::Task(Task::Pickup { drop, .. }) => {
                    v.drops().iter().find(|d| d.id == *drop).map_or(0, |d| dist(at, d.pos))
                }
                Target::Task(_) => 0,
                // Another zone: far.
                Target::Zone(_) | Target::At(..) => i64::from(400 * CELL_FX),
            }
        };
        // 0: low, with nothing to eat: a fire or a bed first.
        // Or a fire close by and a fight or two behind her: sit down while it is on the way.
        let hp = sense::hp_permille(v.body());
        let fire = v
            .props()
            .filter(|p| {
                cat.story.prop(p.def).rest
                    && v.prop_spawn(p).is_some_and(|s| s.talk.is_some())
                    && !self.bad_fires.contains(&p.id)
            })
            .min_by_key(|p| (to_prop(p, at), p.id));
        let low = hp < fight::EAT_BELOW && !fight::has_food(v);
        if low && self.open(v, Goal::Rest) {
            if let Some(p) = fire {
                return Some((Target::Task(Task::Use(UseProp::new(p.id))), Goal::Rest));
            }
            let mut known: Vec<(i64, ZoneId, Vec2)> = cx
                .notes
                .iter()
                .filter(|(z, _)| **z != here)
                .flat_map(|(&z, ns)| ns.iter().filter(|n| n.rest).map(move |n| (0, z, n.at)))
                .collect();
            known.sort_by_key(|&(c, z, a)| (c, z, a.x.0, a.y.0));
            if let Some(&(_, z, a)) = known.first() {
                return Some((Target::At(z, a), Goal::Rest));
            }
        }
        // 1 and 2: the log. The nearest objective of any quest; one that someone is waiting
        // on (it goes back to a person, or to a place) counted at half its distance before
        // an errand for a book or a board.
        for q in v.quests() {
            let waited = someone_waits(q.quest);
            let near = |c: i64| if waited { c / 2 } else { c };
            let g = Goal::HandIn(q.quest);
            if q.ready {
                if self.open(v, g) {
                    if let Some(t) = hand_in(v, cx, q.quest) {
                        offer(near(cost_of(&t)), g, t, &mut best);
                    }
                }
                continue;
            }
            let def = cat.story.quest(q.quest);
            for (i, r) in def.requirements.iter().enumerate() {
                if q.count(i) >= r.qty {
                    continue;
                }
                let g = Goal::Step(q.quest, i as u8);
                if !self.open(v, g) {
                    continue;
                }
                if let Some(t) = step(v, cx, q.quest, i, r.target) {
                    offer(near(cost_of(&t)), g, t, &mut best);
                }
            }
        }
        // What she passes is taken first only when it is nearer than the quest's next step.
        let quest = best.take();
        // 3: who she passes. Talk again only once the log has changed since.
        let log = Ctx::log_size(v);
        for u in talkers(v) {
            let g = Goal::Talk(here, u.id);
            if !self.open(v, g) || cx.talked.get(&(here, u.id)) == Some(&log) {
                continue;
            }
            let tree = cat.combat.unit(u.def).talk.expect("a talker");
            let story = tree_has(v, tree, &|a| matches!(a, Action::Quest(_) | Action::HandIn(_)));
            if cx.model == Model::Rusher && !story {
                continue;
            }
            // The Reader stops for anyone near its way; anyone who can give or take back a
            // quest is worth a walk.
            let d = dist(at, u.pos);
            if !story && d > i64::from(24 * CELL_FX) {
                continue;
            }
            offer(d, g, Target::Task(Task::talk(u.id)), &mut best);
        }
        // What she passes: things to read or open.
        let reach = i64::from(if cx.model == Model::Reader { 20 } else { 12 } * CELL_FX);
        for p in v.props() {
            let g = Goal::Look(here, p.id);
            if cx.used.contains_key(&(here, p.id)) || !self.open(v, g) {
                continue;
            }
            let d = to_prop(p, at);
            if !curious(v, cx.model, p) {
                continue;
            }
            // What teaches is worth crossing the zone for; the rest only when passing.
            let teaches = sense::prop_does(v, p, &|a| matches!(a, Action::Learn(_) | Action::Grow { .. }));
            if d > reach && !(teaches && v.indoor()) {
                continue;
            }
            offer(d + i64::from(6 * CELL_FX), g, Target::Task(Task::Use(UseProp::new(p.id))), &mut best);
        }
        // Whoever gives and takes back quests, met before, once the log has moved since: back
        // to them (the dog on the step).
        for (&def, &(z, pos)) in &cx.seen {
            let Some(tree) = cat.combat.unit(def).talk else { continue };
            if !tree_has(v, tree, &|a| matches!(a, Action::Quest(_) | Action::HandIn(_))) {
                continue;
            }
            let g = Goal::Talk(z, jane_sim::ids::UnitId::new(u32::from(def.0) + 1).expect("nonzero"));
            if !self.open(v, g) || cx.talked_def.get(&def) == Some(&log) {
                continue;
            }
            if z == here && talkers(v).iter().any(|u| u.def == def) {
                continue;
            }
            let t = if z == here {
                Target::Task(Task::Walk { to: pos, near: Fx::from_px(10) })
            } else {
                Target::At(z, pos)
            };
            offer(i64::from(300 * CELL_FX), g, t, &mut best);
        }
        // The Reader sits down at a fire it passes after a fight or two.
        if cx.model == Model::Reader && hp < 750 && self.open(v, Goal::Rest) {
            if let Some(p) = fire.filter(|p| to_prop(p, at) < i64::from(30 * CELL_FX)) {
                offer(
                    to_prop(p, at) + i64::from(8 * CELL_FX),
                    Goal::Rest,
                    Target::Task(Task::Use(UseProp::new(p.id))),
                    &mut best,
                );
            }
        }
        let quest_wants = quest.is_some();
        let mut pick = match (quest, best) {
            (Some(q), Some(c)) => Some(if c.0 < q.0 { c } else { q }),
            (q, c) => q.or(c),
        };
        // In a dungeon, what cannot be walked to is behind something: the crawl's next thing.
        if !matches!(here, ZoneId::County | ZoneId::House) {
            let mut ex = match self.explorer.take() {
                Some(c) if c.zone == here => c,
                _ => crate::crawl::Crawl::new(here),
            };
            let point = |t: &Target| -> Option<Vec2> {
                match t {
                    Target::Fight(u) | Target::Task(Task::Talk { unit: u, .. }) => v.unit(*u).map(|u| u.pos),
                    Target::Task(Task::Use(u)) => v.prop(u.prop).map(|p| crate::sense::bench_side(v, p)),
                    Target::Task(Task::Walk { to, .. }) => Some(*to),
                    Target::Task(Task::Pickup { drop, .. }) => v.drops().iter().find(|d| d.id == *drop).map(|d| d.pos),
                    _ => None,
                }
            };
            if let Some(at) = pick.as_ref().and_then(|p| point(&p.2)) {
                if !ex.reaches(v, at) {
                    pick = None;
                }
            }
            if pick.is_none() {
                if let Some((t, what)) = ex.pick(v, cx) {
                    self.explorer = Some(ex);
                    return Some((Target::Task(t), Goal::Explore(what)));
                }
                // Nothing to be done from this side: back out, and in again by another way.
                if quest_wants && self.open(v, Goal::Explore(crate::crawl::Try::Travel)) {
                    self.explorer = Some(ex);
                    let out = if here == ZoneId::Cellar { ZoneId::House } else { ZoneId::County };
                    if let Some(t) = route(v, cx, out) {
                        return Some((Target::Task(t), Goal::Explore(crate::crawl::Try::Travel)));
                    }
                    return None;
                }
            }
            self.explorer = Some(ex);
        }
        if let Some((_, Goal::Talk(z, u), t)) = &pick {
            cx.talked.insert((*z, *u), log);
            if let Target::Task(Task::Talk { unit, .. }) = t {
                if let Some(d) = v.unit(*unit).map(|u| u.def) {
                    cx.talked_def.insert(d, log);
                }
            }
        }
        pick.map(|(_, g, t)| (t, g))
    }
}

fn goal_name(v: &View<'_>, g: Goal) -> String {
    let cat = jane_data::catalog();
    match g {
        Goal::HandIn(q) => format!("hand in {}", cat.story.quest(q).id),
        Goal::Step(q, i) => format!("{} step {}", cat.story.quest(q).id, i + 1),
        Goal::Talk(z, u) => format!("talk to {:?} in {}", u, z.name()),
        Goal::Rest => "rest at a fire or a bed".into(),
        Goal::Explore(t) => format!("explore: {t:?}"),
        Goal::Look(z, p) => {
            let name = v.prop(p).filter(|_| v.zone() == z).map_or("?", |p| v.name(p.key));
            format!("look at {name} in {}", z.name())
        }
    }
}

/// Would this model stop for this prop in passing?
fn curious(v: &View<'_>, model: Model, p: &jane_sim::Prop) -> bool {
    let cat = jane_data::catalog();
    let def = cat.story.prop(p.def);
    let Some(s) = v.prop_spawn(p) else { return false };
    if s.to.is_some() || p.locked || def.bench || def.carry || def.answers.is_some() {
        return false;
    }
    let teaches = sense::prop_does(v, p, &|a| matches!(a, Action::Learn(_) | Action::Grow { .. }));
    match model {
        // Indoors, a note that gives a quest is in the way; out on the road the Rusher keeps going.
        Model::Rusher => {
            teaches || (v.indoor() && sense::prop_does(v, p, &|a| matches!(a, Action::Quest(_) | Action::HandIn(_))))
        }
        Model::Reader => {
            teaches
                || s.talk.is_some()
                || (!p.used && !s.loot.is_empty())
                || (s.use_list.is_some() && !(def.once && p.used))
        }
    }
}

/// Does a person (a talker's conversation) or a place (a trigger) take `q` back, rather than a
/// thing read? The dog's errands before the lost property book's.
pub fn someone_waits(q: QuestId) -> bool {
    let cat = jane_data::catalog();
    let does = |a: &Action| matches!(a, Action::HandIn(x) if *x == q);
    let mut hit = false;
    for u in cat.combat.units {
        if let Some(t) = u.talk {
            sense::visit_tree(&sense::catalog_lists, t, &mut |a| hit |= does(a));
        }
    }
    for t in cat.story.triggers {
        sense::visit(&sense::catalog_lists, t.trigger.actions, &mut |a| hit |= does(a));
    }
    hit
}

/// Where to hand `q` in.
fn hand_in(v: &View<'_>, cx: &Ctx, q: QuestId) -> Option<Target> {
    let cat = jane_data::catalog();
    let here = v.zone();
    let does = |a: &Action| matches!(a, Action::HandIn(x) if *x == q);
    // A trigger here.
    for (t, fired) in v.triggers() {
        if (fired && t.trigger.once) || !sense::list_has(v, t.trigger.actions, &does) {
            continue;
        }
        if let Some(to) = inside(v, t.rect) {
            return Some(Target::Task(Task::Walk { to, near: Fx::from_px(2) }));
        }
    }
    // Someone here who takes it.
    for u in talkers(v) {
        if tree_has(v, cat.combat.unit(u.def).talk.expect("a talker"), &does) {
            return Some(Target::Task(Task::talk(u.id)));
        }
    }
    // A thing here.
    for p in v.props() {
        if sense::prop_does(v, p, &does) {
            return Some(Target::Task(Task::Use(UseProp::new(p.id))));
        }
    }
    // Someone she saw elsewhere, or saw here and has lost sight of.
    for (i, u) in cat.combat.units.iter().enumerate() {
        let Some(tree) = u.talk else { continue };
        if !tree_has(v, tree, &does) {
            continue;
        }
        if let Some(&(z, at)) = cx.seen.get(&UnitDefId(i as u16)) {
            return Some(if z == here {
                Target::Task(Task::Walk { to: at, near: Fx::from_px(10) })
            } else {
                Target::At(z, at)
            });
        }
    }
    // A thing she saw elsewhere.
    for (&z, notes) in &cx.notes {
        if z != here {
            if let Some(n) = notes.iter().find(|n| n.hands_in.contains(&q)) {
                return Some(Target::At(z, n.at));
            }
        }
    }
    // A trigger in another zone.
    for t in cat.story.triggers {
        if t.zone != here {
            let mut hit = false;
            sense::visit(&sense::catalog_lists, t.trigger.actions, &mut |a| hit |= does(a));
            if hit {
                return Some(Target::Zone(t.zone));
            }
        }
    }
    None
}

/// A walkable cell inside a named rect of this zone, the nearest to her.
fn inside(v: &View<'_>, rect: jane_core::Sym) -> Option<Vec2> {
    let r = v.rect(rect)?;
    let at = v.body().pos;
    r.cells()
        .filter(|&(x, y)| walkable(v, x, y))
        .map(|(x, y)| Vec2::centre(x, y))
        .min_by_key(|c| (dist(at, *c), c.x.0, c.y.0))
}

/// Where step `i` of `q` is.
fn step(v: &View<'_>, cx: &Ctx, q: QuestId, i: usize, target: ReqTarget) -> Option<Target> {
    // A step whose text names a dungeon is done in that dungeon, as the text says, whatever
    // else of the kind she has seen on the way (the cellar's rats, not the allotments').
    let named = zone_of_step(q, i).filter(|&z| z != v.zone());
    match target {
        ReqTarget::Location(name) => place(v, cx, name).or_else(|| named.map(Target::Zone)),
        ReqTarget::Kill(def) => match named {
            Some(z) => Some(Target::Zone(z)),
            None => kill(v, cx, def),
        },
        ReqTarget::Acquire(item) => {
            let at = v.body().pos;
            if let Some(d) = v.drops().iter().filter(|d| d.item == item).min_by_key(|d| (dist(at, d.pos), d.id)) {
                return Some(Target::Task(Task::Pickup { drop: d.id, t: 0 }));
            }
            match named {
                Some(z) => Some(Target::Zone(z)),
                None => get(v, cx, item, 0),
            }
        }
    }
}

/// A place by its content name: a trigger rect here, a thing that marks it, or its zone.
fn place(v: &View<'_>, cx: &Ctx, name: NameId) -> Option<Target> {
    let cat = jane_data::catalog();
    let here = v.zone();
    let does = |a: &Action| matches!(a, Action::Location(jane_core::Key::Name(n)) if *n == name);
    for (t, fired) in v.triggers() {
        if fired && t.trigger.once {
            continue;
        }
        if sense::list_has(v, t.trigger.actions, &does) {
            if let Some(to) = inside(v, t.rect) {
                return Some(Target::Task(Task::Walk { to, near: Fx::from_px(2) }));
            }
        }
    }
    for p in v.props() {
        if sense::prop_does(v, p, &does) {
            return Some(Target::Task(Task::Use(UseProp::new(p.id))));
        }
    }
    for t in cat.story.triggers {
        if t.zone != here {
            let mut hit = false;
            sense::visit(&sense::catalog_lists, t.trigger.actions, &mut |a| hit |= does(a));
            if hit {
                return Some(Target::Zone(t.zone));
            }
        }
    }
    for (&z, notes) in &cx.notes {
        if z != here {
            if let Some(n) = notes.iter().find(|n| n.places.contains(&name)) {
                return Some(Target::At(z, n.at));
            }
        }
    }
    None
}

/// Units of a kind to put down: in sight, or where she last saw one.
fn kill(v: &View<'_>, cx: &Ctx, def: UnitDefId) -> Option<Target> {
    if let Some(u) = units_of(v, def).into_iter().find(|u| fight::fightable(u) && fight::reachable(cx, u.id, v.frame()))
    {
        return Some(Target::Fight(u.id));
    }
    // Where she last saw one standing: this zone's nearest first. Arrived and nothing there:
    // wait a little (it may be about), then forget it.
    let here = v.zone();
    let me = v.body().pos;
    let seen = cx.seen_foes.get(&def)?;
    let (_, &(z, at)) =
        seen.iter().min_by_key(|(id, (z, at))| (*z != here, if *z == here { dist(me, *at) } else { 0 }, **id))?;
    Some(if z != here {
        Target::At(z, at)
    } else if dist(me, at) > i64::from(4 * CELL_FX) {
        Target::Task(Task::Walk { to: at, near: Fx::from_px(8) })
    } else {
        Target::Task(Task::Wait(120))
    })
}

/// A way to come to hold `item`.
fn get(v: &View<'_>, cx: &Ctx, item: ItemId, depth: u8) -> Option<Target> {
    let cat = jane_data::catalog();
    let here = v.zone();
    let at = v.body().pos;
    // On the ground.
    if let Some(d) = v.drops().iter().filter(|d| d.item == item).min_by_key(|d| (dist(at, d.pos), d.id)) {
        return Some(Target::Task(Task::Pickup { drop: d.id, t: 0 }));
    }
    // In a chest here.
    let mut chests: Vec<&jane_sim::Prop> = v
        .props()
        .filter(|p| !p.used && v.prop_spawn(p).is_some_and(|s| s.loot.iter().any(|s| s.item == item)))
        .collect();
    chests.sort_by_key(|p| (to_prop(p, at), p.id));
    if let Some(p) = chests.first() {
        return Some(Target::Task(Task::Use(UseProp { presses: if p.locked { 2 } else { 1 }, ..UseProp::new(p.id) })));
    }
    // Made at a bench from what she holds.
    for r in cat.combat.recipes.iter().filter(|r| r.output == item) {
        let missing = r
            .inputs
            .iter()
            .copied()
            .find(|&i| holds(v, i) < u32::from(r.inputs.iter().filter(|&&x| x == i).count() as u16));
        match missing {
            None => {
                if v.near_bench() {
                    return Some(Target::Task(Task::Craft { inputs: r.inputs.to_vec(), stage: 0, t: 0 }));
                }
                if let Some(b) =
                    v.props().filter(|p| cat.story.prop(p.def).bench).min_by_key(|p| (to_prop(p, at), p.id))
                {
                    return Some(Target::Task(Task::Walk { to: sense::bench_side(v, b), near: Fx::from_px(3) }));
                }
                for (&z, notes) in &cx.notes {
                    if let Some(n) = notes.iter().find(|n| n.bench).filter(|_| z != here) {
                        return Some(Target::At(z, n.at));
                    }
                }
            }
            Some(i) if depth < 2 => {
                if let Some(t) = get(v, cx, i, depth + 1) {
                    return Some(t);
                }
            }
            Some(_) => {}
        }
    }
    // In a chest she saw elsewhere.
    for (&z, notes) in &cx.notes {
        if z != here {
            if let Some(n) = notes.iter().find(|n| n.loot.contains(&item)) {
                return Some(Target::At(z, n.at));
            }
        }
    }
    // Dropped by something.
    for (i, u) in cat.combat.units.iter().enumerate() {
        if u.loot.iter().any(|l| l.item == item) {
            if let Some(t) = kill(v, cx, UnitDefId(i as u16)) {
                return Some(t);
            }
        }
    }
    None
}

/// The door to take toward zone `z`: one straight there, else back toward the county (the house
/// for the cellar).
pub fn route(v: &View<'_>, cx: &Ctx, z: ZoneId) -> Option<Task> {
    let here = v.zone();
    if here == z {
        return None;
    }
    let via = |z: ZoneId| match z {
        ZoneId::Cellar => ZoneId::House,
        _ => ZoneId::County,
    };
    let mut next = z;
    let mut doors = doors_to(v, next);
    if doors.is_empty() {
        next = if here == ZoneId::County { via(z) } else { via(here) };
        doors = doors_to(v, next);
    }
    if doors.is_empty() && here != ZoneId::County {
        doors = doors_to(v, ZoneId::County);
    }
    // Of several doors there, one she can open (the mine's mouth, not the adit barred from
    // inside), then one not yet taken (the other hatch).
    doors.sort_by_key(|p| {
        (!crate::sense::can_open(v, p), cx.used.contains_key(&(here, p.id)), to_prop(p, v.body().pos), p.id)
    });
    let d = doors.first()?;
    Some(Task::Use(UseProp { presses: if d.locked { 2 } else { 1 }, ..UseProp::new(d.id) }))
}
