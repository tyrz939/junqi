//! A dungeon, from its door to its boss and back out (VERIFICATION.md §4.2; `dungeons.test.ts`
//! played instead of scripted).
//!
//! The crawl knows no dungeon by name. Each time it chooses, it floods the ground she can reach
//! from where she stands (closed gates and locked doors are solid props, so they bound it) and
//! takes the first thing it can do there, in this order, nearest first within a kind:
//!
//! 0. mend at a bed or a stove, when low with nothing to eat;
//! 1. pick up what is lying about;
//! 2. open a chest (a locked one only with a key whose tag fits);
//! 3. unlock a door or gate with a key that fits;
//! 4. read what can be read, use what can be used (a lever, a page, a drawer);
//! 5. cast a verb she has at what answers it: Repair or Grow beside it, paying its materials; a
//!    bolt of the right school at a torch or a brazier;
//! 6. push a pushable onto a plate that is up (a small Sokoban over the cells it may cross);
//! 7. put down whatever hostile she can reach, or walk back to one she saw and left (bosses
//!    last, class 9);
//! 8. go through a door that leads elsewhere in the dungeon;
//! 10. walk to the nearest ground she has not been near (what sleeps out of sight wakes).
//!
//! Each thing tried is remembered with a signature of what she holds and how the zone stands;
//! it is tried again only once that has changed (a key found, a gate opened); a fight is taken
//! up again whenever she is ready. When nothing is left and the dungeon's boss is down (or it has
//! none) she walks out by a door, and the crawl is done. Otherwise it stops and says exactly what
//! stood in the way (`Mark::Stuck`: every lock and the tag it wants, every plate up, every verb
//! prop and its materials, every enemy out of reach), and so it does after [`MAX_DEATHS`]
//! deaths, saying where and to what.

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use jane_core::action::School;
use jane_core::num::CELL_FX;
use jane_core::tile::F_NOPUSH;
use jane_core::{ItemId, SpellId, UnitDefId, Vec2, ZoneId};
use jane_data::{Answers, SpellKind};
use jane_sim::event::EventKind;
use jane_sim::ids::{DropId, PropId, UnitId};
use jane_sim::{Prop, View};

use crate::nav::{dist, walkable};
use crate::sense::{self, door_of, holds, knows, prop_rect};
use crate::task::{Ctx, Push, Status, Task, UseProp};
use crate::{Act, Mark, fight, story, talk};

/// Something the crawl tried.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Try {
    Pickup(DropId),
    Prop(PropId),
    Cast(PropId),
    Push(PropId, PropId),
    Fight(UnitId),
    Door(PropId),
    /// Through a door to or from the dungeon.
    Travel,
    /// Mend at a bed or a stove.
    Rest(PropId),
    /// Walk to ground she has not seen.
    Explore(i32, i32),
}

/// Cells about her feet she counts as seen as she walks.
pub const SEEN_RADIUS: i32 = 6;

/// Deaths before a crawl gives up on a dungeon (the kit, not the dungeon, is then the question).
pub const MAX_DEATHS: usize = 4;

/// Where the crawl is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Walking to the dungeon's door.
    Enter,
    Explore,
    /// Nothing left: out by a door to the county.
    Leave,
    Done,
}

#[derive(Debug)]
pub struct Crawl {
    pub zone: ZoneId,
    pub stage: Stage,
    task: Option<(Task, Try)>,
    /// What was tried, with the signature it was tried under and how often.
    tried: BTreeMap<Try, (u64, u32)>,
    /// Bosses seen to die here (def, tick).
    pub bosses: Vec<(UnitDefId, u32)>,
    /// Frames the crawl has run.
    pub frames: u32,
    /// Frames with nothing chosen in a row.
    idle: u32,
    reach: Reach,
    /// Ticks: entered, first boss down, out again.
    pub entered: Option<u32>,
    pub left: Option<u32>,
    deaths: u32,
    /// Where and to what she fell.
    pub deaths_at: Vec<String>,
    /// Cells she has been near (her own record of what she has seen of the dungeon).
    seen: Vec<bool>,
    seen_w: u32,
    /// Why it stopped short, when it did.
    pub stuck: Option<String>,
    /// Why each failed try failed, the last time (for a debugging dump).
    pub failures: BTreeMap<Try, String>,
}

/// The cells she can walk to from where she stands (flood over `View::flags`).
#[derive(Debug, Default)]
pub struct Reach {
    w: u32,
    h: u32,
    seen: Vec<bool>,
    zone: Option<ZoneId>,
    at: u32,
    sig: u64,
}

impl Reach {
    fn update(&mut self, v: &View<'_>, sig: u64) {
        let (w, h) = v.size();
        let (x, y) = v.body().pos.cell();
        if self.zone == Some(v.zone()) && self.sig == sig && self.at + 30 > v.frame() && self.get(x, y) {
            return;
        }
        self.zone = Some(v.zone());
        self.sig = sig;
        self.at = v.frame();
        self.w = w;
        self.h = h;
        self.seen.clear();
        self.seen.resize((w * h) as usize, false);
        let mut q = VecDeque::new();
        let Some(start) = crate::nav::nearest_walkable(v, x, y, 2) else { return };
        self.seen[(start.1 as u32 * w + start.0 as u32) as usize] = true;
        q.push_back(start);
        while let Some((cx, cy)) = q.pop_front() {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if walkable(v, nx, ny) && !self.get(nx, ny) {
                    self.seen[(ny as u32 * w + nx as u32) as usize] = true;
                    q.push_back((nx, ny));
                }
            }
        }
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as u32) < self.w
            && (y as u32) < self.h
            && self.seen[(y as u32 * self.w + x as u32) as usize]
    }

    pub fn point(&self, p: Vec2) -> bool {
        let (x, y) = p.cell();
        self.get(x, y)
    }

    /// Within `r` cells of a cell she can stand in (a stack is taken from a little way off).
    pub fn near(&self, p: Vec2, r: i32) -> bool {
        let (x, y) = p.cell();
        (-r..=r).any(|dy| (-r..=r).any(|dx| self.get(x + dx, y + dy)))
    }

    /// A cell beside the prop's footprint she can stand in.
    pub fn beside(&self, p: &Prop) -> bool {
        let r = prop_rect(p);
        (r.x - 1..=r.right()).any(|x| self.get(x, r.y - 1) || self.get(x, r.bottom()))
            || (r.y..r.bottom()).any(|y| self.get(r.x - 1, y) || self.get(r.right(), y))
    }
}

/// What she holds and how the zone stands, folded: a try is repeated only when this moves.
fn signature(v: &View<'_>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x0100_0000_01b3);
    for s in v.me().bag.iter().flatten() {
        mix(u64::from(s.item.0) << 16 | u64::from(s.qty));
    }
    mix(v.learned().len() as u64);
    for p in v.props() {
        mix(u64::from(p.locked)
            | u64::from(p.solid) << 1
            | u64::from(p.used) << 2
            | u64::from(p.on) << 3
            | u64::from(p.hidden) << 4);
        mix(u64::from(p.cell.x) << 16 | u64::from(p.cell.y));
    }
    mix(v.props().count() as u64);
    h
}

/// The key in her bag that opens a lock tagged `tag` (a content name).
fn key_for(v: &View<'_>, tag: jane_core::Key) -> Option<ItemId> {
    let cat = jane_data::catalog();
    let jane_core::Key::Name(n) = tag else { return None };
    v.me().bag.iter().flatten().map(|s| s.item).find(|&i| cat.combat.item(i).opens == Some(n))
}

/// A learned spell that is a bolt of `school`.
fn bolt_of(v: &View<'_>, school: School) -> Option<SpellId> {
    let cat = jane_data::catalog();
    cat.combat
        .spells
        .iter()
        .enumerate()
        .map(|(i, s)| (SpellId(i as u16), s))
        .find(|(id, s)| {
            s.school == school && s.kind == SpellKind::Bolt && knows(v, *id) && s.mp.0 <= v.body().mp.0.max(0) + 1
        })
        .map(|(id, _)| id)
}

/// A learned world verb that `answers` wants.
fn verb_for(v: &View<'_>, a: Answers) -> Option<SpellId> {
    let name = match a {
        Answers::Repair => "repair",
        Answers::Grow => "grow",
        _ => return None,
    };
    let s = jane_data::catalog().combat.spell_id(name)?;
    knows(v, s).then_some(s)
}

impl Crawl {
    pub fn new(zone: ZoneId) -> Crawl {
        Crawl {
            zone,
            stage: Stage::Enter,
            task: None,
            tried: BTreeMap::new(),
            bosses: Vec::new(),
            frames: 0,
            idle: 0,
            reach: Reach::default(),
            entered: None,
            left: None,
            deaths: 0,
            deaths_at: Vec::new(),
            seen: Vec::new(),
            seen_w: 0,
            stuck: None,
            failures: BTreeMap::new(),
        }
    }

    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }

    /// What was tried and how often, for a debugging line.
    pub fn tried_list(&self) -> Vec<(Try, u32)> {
        self.tried.iter().map(|(t, &(_, n))| (*t, n)).collect()
    }

    pub fn status(&self) -> String {
        format!("{:?} {:?}", self.stage, self.task)
    }

    fn stop(&mut self, why: String, notes: &mut Vec<Mark>) {
        notes.push(Mark::Stuck(why.clone()));
        self.stuck = Some(why);
        self.stage = Stage::Done;
    }

    pub fn think(&mut self, v: &View<'_>, cx: &mut Ctx, events: &[jane_sim::Event], notes: &mut Vec<Mark>) -> Act {
        self.frames += 1;
        for e in events {
            if let EventKind::Death { def, .. } = e.kind {
                if jane_data::catalog().combat.unit(def).boss && v.zone() == self.zone {
                    self.bosses.push((def, v.tick().0));
                    notes.push(Mark::Note(format!("boss down: {}", jane_data::catalog().combat.unit(def).id)));
                }
            }
        }
        if let Some(a) = talk::answer(v, cx) {
            return a;
        }
        if !v.body().alive {
            if v.me().stats.deaths != self.deaths {
                self.deaths = v.me().stats.deaths;
                let cat = jane_data::catalog();
                let by = cx.fight.target.and_then(|t| v.unit(t)).map_or("?", |u| cat.combat.unit(u.def).id);
                let doing = self.task.as_ref().map_or("nothing".to_owned(), |(_, w)| format!("{w:?}"));
                let at = v.body().pos.cell();
                self.deaths_at.push(format!("at {at:?} by {by} while {doing}"));
                self.task = None;
                if self.deaths_at.len() >= MAX_DEATHS {
                    let why = format!(
                        "died {} times in the {}: {}",
                        self.deaths_at.len(),
                        self.zone.name(),
                        self.deaths_at.join("; ")
                    );
                    self.stop(why, notes);
                }
            }
            return Act::idle();
        }
        match self.stage {
            Stage::Done => return Act::idle(),
            Stage::Enter => {
                if v.zone() == self.zone {
                    self.stage = Stage::Explore;
                    self.entered = Some(v.tick().0);
                    notes.push(Mark::Note(format!("in the {}", self.zone.name())));
                } else {
                    return self.enter(v, cx, notes);
                }
            }
            Stage::Leave => {
                if v.zone() != self.zone {
                    self.stage = Stage::Done;
                    self.left = Some(v.tick().0);
                    notes.push(Mark::Note(format!("out of the {}", self.zone.name())));
                    return Act::idle();
                }
            }
            Stage::Explore => {
                if v.zone() != self.zone {
                    // A door took her out (or she woke outside): back in.
                    self.stage = Stage::Enter;
                    self.task = None;
                    return Act::idle();
                }
            }
        }
        self.look(v);
        // A dungeon done with what the story needs from it (`tactics/`): out.
        if self.stage == Stage::Explore && crate::tactics::museum::done(v) {
            self.stage = Stage::Leave;
            self.task = None;
            notes.push(Mark::Note("what the story needs is in the bag: leaving".into()));
        }
        // A boss room's own play (`tactics/`), before the general fight.
        if let Some(a) = crate::tactics::museum::fight(v, cx, &self.reach) {
            return a;
        }
        if let Some(id) = fight::threat(v, cx) {
            if let Some(a) = fight::engage(v, cx, id) {
                return a;
            }
        }
        if let Some(c) = fight::eat(v) {
            return Act::press(c);
        }
        let sig = signature(v);
        // Low with nothing to eat: whatever she was doing waits for a bed or a stove.
        // Low with nothing to eat: whatever she was doing waits for a bed or a stove she can
        // reach (shut in with a boss, there is none, and she carries on).
        let low = sense::hp_permille(v.body()) < 500 && !fight::has_food(v);
        if low && self.task.as_ref().is_some_and(|(_, w)| !matches!(w, Try::Rest(_))) {
            self.reach.update(v, sig);
            if self.rest_in_reach(v).is_some() {
                self.task = None;
            }
        }
        for _ in 0..4 {
            if let Some((t, what)) = &mut self.task {
                let what = *what;
                match t.tick(v, cx) {
                    Status::Act(a) => return a,
                    Status::Done => self.task = None,
                    Status::Failed(why) => {
                        self.task = None;
                        self.tried.entry(what).or_insert((sig, 0)).1 += 2;
                        self.failures.insert(what, why);
                        // Ground she cannot get to is as good as seen.
                        if let Try::Explore(x, y) = what {
                            self.mark_seen(v, x, y);
                        }
                    }
                }
            }
            if self.stage == Stage::Leave {
                match story::route(v, cx, jane_core::ZoneId::County) {
                    Some(t) => {
                        self.task = Some((t, Try::Travel));
                        continue;
                    }
                    None => {
                        self.stop("no door out to the county from where she stands".into(), notes);
                        return Act::idle();
                    }
                }
            }
            self.reach.update(v, sig);
            match self.choose(v, cx, sig) {
                Some((t, what)) => {
                    self.idle = 0;
                    let e = self.tried.entry(what).or_insert((sig, 0));
                    e.0 = sig;
                    e.1 += 1;
                    self.task = Some((t, what));
                }
                None => {
                    self.idle += 1;
                    if self.idle < 90 {
                        // Things settle (a gate drops, a plate is read every 6 ticks): look again.
                        return Act::idle();
                    }
                    if boss_of(self.zone).is_some_and(|b| !self.bosses.iter().any(|&(d, _)| d == b)) {
                        let why = self.why_stuck(v);
                        self.stop(why, notes);
                        return Act::idle();
                    }
                    self.stage = Stage::Leave;
                    notes.push(Mark::Note("nothing left: leaving".into()));
                }
            }
        }
        Act::idle()
    }

    fn enter(&mut self, v: &View<'_>, cx: &mut Ctx, notes: &mut Vec<Mark>) -> Act {
        if let Some(a) = talk::answer(v, cx) {
            return a;
        }
        if let Some(id) = fight::threat(v, cx) {
            if let Some(a) = fight::engage(v, cx, id) {
                return a;
            }
        }
        if let Some((t, _)) = &mut self.task {
            match t.tick(v, cx) {
                Status::Act(a) => return a,
                Status::Done => self.task = None,
                Status::Failed(why) => {
                    self.task = None;
                    let n = self.tried.entry(Try::Travel).or_insert((0, 0));
                    n.1 += 1;
                    if n.1 >= 3 {
                        self.stop(format!("cannot get in to the {}: {why}", self.zone.name()), notes);
                    }
                    return Act::idle();
                }
            }
        }
        match story::route(v, cx, self.zone) {
            Some(t) => {
                self.task = Some((t, Try::Travel));
                Act::idle()
            }
            None => {
                self.stop(format!("no door toward the {} from the {}", self.zone.name(), v.zone().name()), notes);
                Act::idle()
            }
        }
    }

    /// Mark the ground about her as seen (a box [`SEEN_RADIUS`] cells about her feet).
    fn look(&mut self, v: &View<'_>) {
        let (w, h) = v.size();
        if self.seen_w != w || self.seen.len() != (w * h) as usize {
            self.seen_w = w;
            self.seen.clear();
            self.seen.resize((w * h) as usize, false);
        }
        let (x, y) = v.body().pos.cell();
        self.mark_seen(v, x, y);
    }

    fn mark_seen(&mut self, v: &View<'_>, x: i32, y: i32) {
        let (w, h) = v.size();
        if self.seen.len() != (w * h) as usize {
            return;
        }
        for cy in (y - SEEN_RADIUS).max(0)..=(y + SEEN_RADIUS).min(h as i32 - 1) {
            for cx in (x - SEEN_RADIUS).max(0)..=(x + SEEN_RADIUS).min(w as i32 - 1) {
                self.seen[(cy as u32 * w + cx as u32) as usize] = true;
            }
        }
    }

    /// The nearest cell she can walk to and has not been near, by a flood from her feet.
    fn frontier(&self, v: &View<'_>) -> Option<(i32, i32)> {
        let (w, h) = v.size();
        if self.seen.len() != (w * h) as usize {
            return None;
        }
        let start = v.body().pos.cell();
        let mut q = VecDeque::new();
        let mut been = vec![false; (w * h) as usize];
        let ix = |x: i32, y: i32| (y as u32 * w + x as u32) as usize;
        if !walkable(v, start.0, start.1) {
            return None;
        }
        been[ix(start.0, start.1)] = true;
        q.push_back(start);
        while let Some((cx, cy)) = q.pop_front() {
            if !self.seen[ix(cx, cy)] {
                return Some((cx, cy));
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if walkable(v, nx, ny) && !been[ix(nx, ny)] {
                    been[ix(nx, ny)] = true;
                    q.push_back((nx, ny));
                }
            }
        }
        None
    }

    /// One thing to do here, for a plan that is not a crawl (the story in the cellar): the
    /// crawl's own choice, remembered as tried.
    pub fn pick(&mut self, v: &View<'_>, cx: &Ctx) -> Option<(Task, Try)> {
        let sig = signature(v);
        self.reach.update(v, sig);
        let (t, what) = self.choose(v, cx, sig)?;
        let e = self.tried.entry(what).or_insert((sig, 0));
        e.0 = sig;
        e.1 += 1;
        Some((t, what))
    }

    /// A failed try counts double.
    pub fn failed(&mut self, what: Try, why: &str) {
        self.failures.insert(what, why.to_owned());
        self.tried.entry(what).or_insert((0, 0)).1 += 2;
    }

    /// Can she walk to this point from where she stands (as of the last flood)?
    pub fn reaches(&mut self, v: &View<'_>, at: Vec2) -> bool {
        self.reach.update(v, signature(v));
        self.reach.point(at)
    }

    fn fresh(&self, what: Try, sig: u64) -> bool {
        match (what, self.tried.get(&what)) {
            (_, None) => true,
            // A fight is taken up again whenever she is ready to (it may have healed; so has she).
            (Try::Fight(_), Some(&(_, n))) => n < 12,
            (_, Some(&(s, n))) => s != sig && n < 6,
        }
    }

    fn choose(&self, v: &View<'_>, cx: &Ctx, sig: u64) -> Option<(Task, Try)> {
        let cat = jane_data::catalog();
        let at = v.body().pos;
        let reach = &self.reach;
        let near_prop = |p: &Prop| sense::to_prop(p, at);
        let mut best: Option<(u8, i64, Try, Task)> = None;
        let offer = |class: u8, cost: i64, what: Try, t: Task, best: &mut Option<(u8, i64, Try, Task)>| {
            if !self.fresh(what, sig) {
                return;
            }
            if best.as_ref().is_none_or(|(c, k, w, _)| (class, cost, what) < (*c, *k, *w)) {
                *best = Some((class, cost, what, t));
            }
        };
        // 0. Low, with nothing to eat: a bed or a stove she can reach.
        if sense::hp_permille(v.body()) < 500 && !fight::has_food(v) {
            if let Some(p) = self.rest_in_reach(v) {
                return Some((Task::Use(UseProp::new(p)), Try::Rest(p)));
            }
        }
        // What the dungeon's own idea puts first (`tactics/`).
        if let Some((t, what)) = crate::tactics::museum::first(v, cx, reach).filter(|(_, w)| self.fresh(*w, sig)) {
            return Some((t, what));
        }
        // 1. Lying about.
        for d in v.drops() {
            if reach.near(d.pos, 1) {
                offer(1, dist(at, d.pos), Try::Pickup(d.id), Task::Pickup { drop: d.id, t: 0 }, &mut best);
            }
        }
        for p in v.props() {
            if p.hidden || !reach.beside(p) {
                continue;
            }
            let def = cat.story.prop(p.def);
            let Some(s) = v.prop_spawn(p) else { continue };
            let d = near_prop(p);
            let door = door_of(v, p);
            // 2 and 3. Locks: a key that fits, then open (or go through, for a door here).
            if p.locked {
                if s.key_tag.and_then(|t| key_for(v, t)).is_some() {
                    let presses = if door.is_some() || def.gate { 1 } else { 2 };
                    offer(3, d, Try::Prop(p.id), Task::Use(UseProp { presses, ..UseProp::new(p.id) }), &mut best);
                }
                // A locked thing that answers a verb (a cracked case) is opened by the verb.
                if def.answers.is_none() {
                    continue;
                }
            } else if !p.used && !s.loot.is_empty() {
                offer(2, d, Try::Prop(p.id), Task::Use(UseProp::new(p.id)), &mut best);
                continue;
            }
            // 5. Verbs.
            if let Some(a) = def.answers {
                if !p.used && !p.on {
                    if let Some(verb) = verb_for(v, a) {
                        let has = s.needs.iter().all(|n| holds(v, n.item) >= u32::from(n.qty));
                        if has {
                            offer(5, d, Try::Cast(p.id), Task::cast(verb, p.id), &mut best);
                        }
                    } else if let Some(bolt) = a.school().and_then(|sc| bolt_of(v, sc)) {
                        if let Some(t) = bolt_at(v, reach, p, bolt) {
                            offer(5, d, Try::Cast(p.id), t, &mut best);
                        }
                    }
                }
                continue;
            }
            // 8. Doors: elsewhere in the dungeon (a hop), not out of it.
            if let Some(dr) = door {
                if dr.zone == v.zone() {
                    offer(8, d, Try::Door(p.id), Task::Use(UseProp::new(p.id)), &mut best);
                }
                continue;
            }
            // 4. Read, use.
            let once_used = def.once && p.used;
            if !once_used
                && (s.talk.is_some() || s.use_list.is_some())
                && !def.plate
                && !def.bench
                && !cx.used.contains_key(&(v.zone(), p.id))
            {
                offer(4, d, Try::Prop(p.id), Task::Use(UseProp::new(p.id)), &mut best);
            }
        }
        // 6. Plates that are up, and something to push onto one.
        if best.as_ref().is_none_or(|b| b.0 > 6) {
            for plate in v.props().filter(|p| !p.hidden && cat.story.prop(p.def).plate && !p.on && reach.beside(p)) {
                for thing in v.props().filter(|p| {
                    let d = cat.story.prop(p.def);
                    !p.hidden && d.push && p.solid && reach.beside(p)
                }) {
                    let what = Try::Push(thing.id, plate.id);
                    if !self.fresh(what, sig) {
                        continue;
                    }
                    if let Some(path) = sokoban(v, reach, thing, plate) {
                        let cost = path.len() as i64 * i64::from(CELL_FX);
                        offer(
                            6,
                            cost,
                            what,
                            Task::Push(Push { prop: thing.id, path, at: 0, t: 0, lean: 0 }),
                            &mut best,
                        );
                    }
                }
            }
        }
        // 7. Whatever hostile she can reach; bosses last. One she saw and has walked away from
        // (it sleeps out of her sight) is walked back to.
        for u in sense::enemies(v) {
            if !fight::fightable(u) || !reach.point(u.pos) {
                continue;
            }
            let boss = cat.combat.unit(u.def).boss;
            offer(if boss { 9 } else { 7 }, dist(at, u.pos), Try::Fight(u.id), Task::Hunt(u.id), &mut best);
        }
        for (&def, seen) in &cx.seen_foes {
            let d = cat.combat.unit(def);
            if d.bait.is_some() {
                continue;
            }
            for (&id, &(z, pos)) in seen {
                if z != v.zone() || v.unit(id).is_some() || !reach.point(pos) {
                    continue;
                }
                let t = Task::Walk { to: pos, near: jane_core::Fx::from_px(12) };
                offer(if d.boss { 9 } else { 7 }, dist(at, pos) + i64::from(4 * CELL_FX), Try::Fight(id), t, &mut best);
            }
        }
        // 10. Ground she has not seen (what sleeps out of sight wakes as she comes).
        if best.is_none() {
            if let Some((x, y)) = self.frontier(v) {
                let t = Task::Walk { to: Vec2::centre(x, y), near: jane_core::Fx::from_px(6) };
                return Some((t, Try::Explore(x, y)));
            }
            // Nothing in the general order: what the dungeon's own idea asks (`tactics/`).
            if let Some((t, what)) = crate::tactics::museum::idle(v, reach).filter(|(_, w)| self.fresh(*w, sig)) {
                return Some((t, what));
            }
        }
        best.map(|(_, _, what, t)| (t, what))
    }

    /// The nearest bed or stove she can walk to, as of the last flood.
    fn rest_in_reach(&self, v: &View<'_>) -> Option<PropId> {
        let cat = jane_data::catalog();
        let at = v.body().pos;
        v.props()
            .filter(|p| {
                !p.hidden
                    && cat.story.prop(p.def).rest
                    && self.reach.beside(p)
                    && v.prop_spawn(p).is_some_and(|s| s.talk.is_some())
            })
            .min_by_key(|p| (sense::to_prop(p, at), p.id))
            .map(|p| p.id)
    }

    /// Everything that stood in the way, for the log.
    pub fn why_stuck(&self, v: &View<'_>) -> String {
        let cat = jane_data::catalog();
        let cat_boss = boss_of(self.zone).map_or("none", |b| jane_data::catalog().combat.unit(b).id);
        let mut out =
            format!("nothing left to try in the {} (its boss: {cat_boss}; down: {:?});", self.zone.name(), self.bosses);
        for p in v.props().filter(|p| !p.hidden) {
            let def = cat.story.prop(p.def);
            let Some(s) = v.prop_spawn(p) else { continue };
            let seen = if self.reach.beside(p) { "reachable" } else { "out of reach" };
            if p.locked {
                let tag = s.key_tag.map_or("no tag".to_owned(), |k| match k {
                    jane_core::Key::Name(n) => cat.name(n).to_owned(),
                    jane_core::Key::Local(_) => "a local tag".to_owned(),
                });
                let _ = write!(out, " locked {} ({seen}, wants {tag});", v.name(p.key));
            } else if def.plate && !p.on {
                let _ = write!(out, " plate {} up ({seen});", v.name(p.key));
            } else if def.answers.is_some() && !p.used && !p.on && self.reach.beside(p) {
                let needs: Vec<_> =
                    s.needs.iter().map(|n| format!("{}x{}", n.qty, cat.combat.item(n.item).id)).collect();
                let _ = write!(out, " {} answers {:?} (needs {:?});", v.name(p.key), def.answers, needs);
                if self.failures.contains_key(&Try::Cast(p.id)) {
                    let _ = write!(out, "\n{}", crate::ascii(v, prop_rect(p), 4));
                }
            }
        }
        for u in sense::enemies(v) {
            let def = cat.combat.unit(u.def);
            if def.boss || !self.reach.point(u.pos) {
                let _ = write!(
                    out,
                    " {} {} at {:?}{};",
                    if def.boss { "boss" } else { "enemy" },
                    def.id,
                    u.pos.cell(),
                    if self.reach.point(u.pos) { "" } else { " (out of reach)" }
                );
            }
        }
        out
    }
}

/// The unit a dungeon's boss room holds (`None`: the cellar, the library and the pipes have none).
pub fn boss_of(z: ZoneId) -> Option<UnitDefId> {
    let cat = jane_data::catalog();
    let m = cat.dungeons.missions.iter().find(|m| m.zone == z)?;
    m.nodes
        .iter()
        .filter(|n| n.kind == jane_data::MissionNodeKind::Boss)
        .flat_map(|n| n.holds.iter())
        .find_map(|h| h.unit)
}

/// A bolt at a prop that answers its school: from a reachable cell 2 to 6 cells off, in sight.
fn bolt_at(v: &View<'_>, reach: &Reach, p: &Prop, spell: SpellId) -> Option<Task> {
    let c = sense::prop_centre(p);
    let (px, py) = c.cell();
    let me = v.body().pos;
    let mut best: Option<(i64, Vec2)> = None;
    for r in 2..=6 {
        for (dx, dy) in [(0, r), (0, -r), (r, 0), (-r, 0), (r, r), (-r, r), (r, -r), (-r, -r)] {
            let (x, y) = (px + dx, py + dy);
            if !reach.get(x, y) {
                continue;
            }
            let at = Vec2::centre(x, y);
            if !v.sight(at, face_of(p, at)) {
                continue;
            }
            let d = dist(me, at);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, at));
            }
        }
        if best.is_some() {
            break;
        }
    }
    let (_, from) = best?;
    Some(Task::Aim { spell, from, at: c, t: 0 })
}

/// The point just outside a prop's footprint nearest `at`: what she must see to hit it (a prop
/// that blocks sight, an arch or a case, hides its own middle).
fn face_of(p: &Prop, at: Vec2) -> Vec2 {
    let r = prop_rect(p);
    let clamp = |v: i32, lo: i32, hi: i32| v.clamp(lo * CELL_FX - 1, hi * CELL_FX);
    Vec2::new(jane_core::Fx(clamp(at.x.0, r.x, r.right())), jane_core::Fx(clamp(at.y.0, r.y, r.bottom())))
}

/// The origins a pushable passes through to cover the plate, pushed only (each push needs a
/// cell behind it to stand in, reachable, and the cells ahead free of walls, props and sills).
/// Breadth-first over its origin.
fn sokoban(v: &View<'_>, reach: &Reach, thing: &Prop, plate: &Prop) -> Option<Vec<(i32, i32)>> {
    let cat = jane_data::catalog();
    let d = cat.story.prop(thing.def);
    let (w, h) = (i32::from(d.w), i32::from(d.h));
    let start = (i32::from(thing.cell.x), i32::from(thing.cell.y));
    let own = prop_rect(thing);
    let goal = prop_rect(plate);
    let cell_free = |x: i32, y: i32| (walkable(v, x, y) || own.contains(x, y)) && v.flags(x, y) & F_NOPUSH == 0;
    let fits = |(x, y): (i32, i32)| (0..w).all(|i| (0..h).all(|j| cell_free(x + i, y + j)));
    let stand = |(x, y): (i32, i32)| reach.get(x, y) || own.contains(x, y);
    let mut prev: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    let mut q = VecDeque::new();
    q.push_back(start);
    prev.insert(start, start);
    let mut found = None;
    while let Some(c) = q.pop_front() {
        if jane_core::Rect::new(c.0, c.1, w, h).overlaps(goal) {
            found = Some(c);
            break;
        }
        if prev.len() > 4000 {
            break;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dy);
            if prev.contains_key(&n) || !fits(n) || !stand(behind(c, (dx, dy), w, h)) {
                continue;
            }
            prev.insert(n, c);
            q.push_back(n);
        }
    }
    let mut c = found?;
    let mut path = vec![c];
    while let Some(&p) = prev.get(&c) {
        if p == start {
            break;
        }
        path.push(p);
        c = p;
    }
    path.reverse();
    Some(path)
}

/// The cell to stand in to push a `w x h` prop at origin `o` along `(dx, dy)`: against the far
/// side, level with its first row or column.
pub fn behind(o: (i32, i32), (dx, dy): (i32, i32), w: i32, h: i32) -> (i32, i32) {
    match (dx, dy) {
        (1, _) => (o.0 - 1, o.1),
        (-1, _) => (o.0 + w, o.1),
        (_, 1) => (o.0, o.1 - 1),
        _ => (o.0, o.1 + h),
    }
}

/// Where a crawl sets her down for the console's setup: the zone and mark a dungeon's main door
/// opens onto (the county's mouth; the house for the cellar).
pub fn mouth_of(bp: &jane_core::Blueprint) -> Option<(ZoneId, jane_core::Key)> {
    bp.props.iter().find_map(|p| p.to.filter(|d| d.zone != bp.zone).map(|d| (d.zone, d.mark)))
}

/// The dungeons in the order the story walks them (WORLD.md §9.1: the acts), the cellar first.
pub const ORDER: [ZoneId; 9] = [
    ZoneId::Cellar,
    ZoneId::Mine,
    ZoneId::Museum,
    ZoneId::Library,
    ZoneId::Forest,
    ZoneId::Pipes,
    ZoneId::Factory,
    ZoneId::Burial,
    ZoneId::School,
];

/// Growth as (strength, spirit).
pub type Growth = (i32, i32);

/// The `grow`s in an action list (an `if` both ways), added to `out`. `bp` resolves a
/// blueprint's own lists; a catalog list needs none.
fn grows(bp: Option<&jane_core::Blueprint>, acts: &[jane_core::Action], out: &mut Growth) {
    use jane_core::action::{Action, Stat};
    for a in acts {
        match *a {
            Action::Grow { stat: Stat::Strength, amount, .. } => out.0 += i32::from(amount),
            Action::Grow { stat: Stat::Spirit, amount, .. } => out.1 += i32::from(amount),
            Action::If { then, els, .. } => {
                grows(bp, list_of(bp, then), out);
                if let Some(e) = els {
                    grows(bp, list_of(bp, e), out);
                }
            }
            _ => {}
        }
    }
}

fn list_of(bp: Option<&jane_core::Blueprint>, r: jane_core::ListRef) -> &[jane_core::Action] {
    match r {
        jane_core::ListRef::Blueprint(i) => bp.and_then(|b| b.lists.get(usize::from(i))).map_or(&[], Vec::as_slice),
        jane_core::ListRef::Catalog(_) => jane_data::catalog().list(r),
    }
}

/// The growth on offer in a blueprint: every `grow` in its props' `use` lists and in the
/// conversations its props open (jars, gold-leaf pages, the library's margins).
pub fn growth_in(bp: &jane_core::Blueprint) -> Growth {
    let cat = jane_data::catalog();
    let mut out = (0, 0);
    for p in &bp.props {
        if let Some(u) = p.use_list {
            grows(Some(bp), list_of(Some(bp), u), &mut out);
        }
        for n in p.talk.map(|t| cat.story.dialogue(t).nodes).unwrap_or_default() {
            for a in n.actions.into_iter().chain(n.options.iter().filter_map(|o| o.actions)) {
                grows(Some(bp), cat.list(a), &mut out);
            }
        }
    }
    out
}

/// The growth a player has found by the time she reaches `z` (PLAN.md §2.6: "growth comes from
/// finding things"): everything the dungeons before it in [`ORDER`] offer and, from the Museum on,
/// what the county's quests pay in growth (every one of them is a Lowfields errand today, done
/// between the mine and the river). The cellar and the mine are met as the first hour leaves her:
/// nothing found. Counted off this seed's own blueprints and the catalog, so the kit follows the
/// content.
pub fn growth_before(bps: &jane_sim::Blueprints, z: ZoneId) -> Growth {
    let cat = jane_data::catalog();
    let Some(at) = ORDER.iter().position(|&o| o == z) else { return (0, 0) };
    let mut out = (0, 0);
    for &d in &ORDER[..at] {
        let (s, p) = growth_in(bps.get(d));
        out = (out.0 + s, out.1 + p);
    }
    let after_mine = at > ORDER.iter().position(|&o| o == ZoneId::Mine).unwrap_or(0);
    if after_mine {
        for q in cat.story.quests {
            grows(None, cat.list(q.rewards), &mut out);
        }
    }
    out
}

/// The materials a player carries into `z`: of every item a mending or a growing anywhere in the
/// story's dungeons asks for (a prop's `needs`: wood, iron), what the places before it in
/// [`ORDER`] hold in their chests less what their own broken things take, never under nothing.
/// The cellar's storage room is where the mine's wood comes from (the dog: "Broken stairs want
/// wood. There is some in the cellar storage"); the mine leaves nothing over.
pub fn materials_before(bps: &jane_sim::Blueprints, z: ZoneId) -> Vec<(ItemId, u16)> {
    let Some(at) = ORDER.iter().position(|&o| o == z) else { return Vec::new() };
    let mut kinds: Vec<ItemId> =
        ORDER.iter().flat_map(|&d| bps.get(d).props.iter().flat_map(|p| p.needs.iter().map(|s| s.item))).collect();
    kinds.sort();
    kinds.dedup();
    let mut carried: Vec<i32> = vec![0; kinds.len()];
    for &d in &ORDER[..at] {
        let bp = bps.get(d);
        for (k, have) in kinds.iter().zip(carried.iter_mut()) {
            let found: i32 =
                bp.props.iter().flat_map(|p| &p.loot).filter(|s| s.item == *k).map(|s| i32::from(s.qty)).sum();
            let spent: i32 =
                bp.props.iter().flat_map(|p| &p.needs).filter(|s| s.item == *k).map(|s| i32::from(s.qty)).sum();
            *have = (*have + found - spent).max(0);
        }
    }
    kinds
        .into_iter()
        .zip(carried)
        .filter(|&(_, n)| n > 0)
        .map(|(k, n)| (k, n.min(i32::from(u16::MAX)) as u16))
        .collect()
}

/// The console setup for a crawl of `z` with the kit a player would carry there: the dungeon's
/// given verbs learned, its given keys and the key to its county door in the bag, the growth she
/// would have found before it ([`growth_before`]) and the materials left over from the places
/// before it ([`materials_before`]), food, the clock at ten in the morning (the
/// museum is shut after four), and her set down at its door. A county door whose key lies inside
/// the dungeon (the School's front doors, opened from inside) she cannot carry the first time:
/// she is set down inside, where the other way in arrives.
pub fn setup(bps: &jane_sim::Blueprints, z: ZoneId) -> Vec<jane_sim::Command> {
    use jane_sim::{Command, DevOp};
    let cat = jane_data::catalog();
    let (county, dungeon) = (&**bps.get(ZoneId::County), &**bps.get(z));
    let mut out = vec![Command::Dev(DevOp::Time { hour: 10 })];
    let (strength, spirit) = growth_before(bps, z);
    for (stat, amount) in [(jane_core::action::Stat::Strength, strength), (jane_core::action::Stat::Spirit, spirit)] {
        if amount > 0 {
            out.push(Command::Dev(DevOp::Grow { stat, amount: amount.min(i32::from(i16::MAX)) as i16 }));
        }
    }
    if let Some(m) = cat.dungeons.missions.iter().find(|m| m.zone == z) {
        for &s in m.given_verbs {
            out.push(Command::Dev(DevOp::Learn(s)));
        }
        for &tag in m.given_keys {
            if let Some(i) = cat.combat.items.iter().position(|i| i.opens == Some(tag)) {
                out.push(Command::Dev(DevOp::Give { item: ItemId(i as u16), qty: 1 }));
            }
        }
    }
    let inside = keys_inside(county, dungeon);
    for k in keys_for(county, z) {
        if !inside.contains(&k) {
            out.push(Command::Dev(DevOp::Give { item: k, qty: 1 }));
        }
    }
    for (item, qty) in materials_before(bps, z) {
        out.push(Command::Dev(DevOp::Give { item, qty }));
    }
    out.push(Command::Dev(DevOp::Give { item: sense::item("apple"), qty: 8 }));
    // What the bench and the first quests will have given her by then.
    for (name, qty) in [("potion_stoneskin", 2), ("potion_manashield", 1)] {
        out.push(Command::Dev(DevOp::Give { item: sense::item(name), qty }));
    }
    // A county door locked with a tag nothing in the catalog opens cannot be played through, and
    // one whose key is found inside is not, the first time: she is set down inside, at the mark
    // by its way out.
    if keyless(county, z) || !inside.is_empty() {
        if let Some(n) = inside_mark(dungeon) {
            out.push(Command::Dev(DevOp::Tp { zone: z, mark: jane_sim::sym::of_name(n) }));
        }
    } else if let Some((zone, jane_core::Key::Name(n))) = mouth_of(dungeon) {
        out.push(Command::Dev(DevOp::Tp { zone, mark: jane_sim::sym::of_name(n) }));
    }
    out
}

/// A dungeon's `entry` mark, else its first named one.
fn inside_mark(bp: &jane_core::Blueprint) -> Option<jane_core::NameId> {
    let cat = jane_data::catalog();
    let entry = cat.name_id("entry");
    let named = || {
        bp.marks.keys().filter_map(|k| match *k {
            jane_core::Key::Name(n) => Some(n),
            jane_core::Key::Local(_) => None,
        })
    };
    named().find(|&n| Some(n) == entry).or_else(|| named().next())
}

/// Does the county's door into `z` want a key nothing opens? (The crawl then starts inside.)
pub fn keyless(county: &jane_core::Blueprint, z: ZoneId) -> bool {
    let cat = jane_data::catalog();
    county.props.iter().any(|p| {
        p.to.is_some_and(|d| d.zone == z)
            && p.locked
            && p.key_tag.is_some_and(|t| match t {
                jane_core::Key::Name(n) => !cat.combat.items.iter().any(|i| i.opens == Some(n)),
                jane_core::Key::Local(_) => true,
            })
    })
}

/// The keys to the county's doors into a dungeon that lie in the dungeon itself (the School's
/// front doors: the key is on the caretaker's bin, inside).
pub fn keys_inside(county: &jane_core::Blueprint, dungeon: &jane_core::Blueprint) -> Vec<ItemId> {
    keys_for(county, dungeon.zone)
        .into_iter()
        .filter(|&k| dungeon.props.iter().any(|p| p.loot.iter().any(|s| s.item == k)))
        .collect()
}

/// The keys that open the county's doors into `z` (the kit a player would carry to it).
pub fn keys_for(county: &jane_core::Blueprint, z: ZoneId) -> Vec<ItemId> {
    let cat = jane_data::catalog();
    county
        .props
        .iter()
        .filter(|p| p.to.is_some_and(|d| d.zone == z))
        .filter_map(|p| p.key_tag)
        .filter_map(|t| match t {
            jane_core::Key::Name(n) => Some(n),
            jane_core::Key::Local(_) => None,
        })
        .filter_map(|n| cat.combat.items.iter().position(|i| i.opens == Some(n)).map(|i| ItemId(i as u16)))
        .collect()
}
