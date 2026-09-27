//! A dungeon, from its door to its boss and back out (VERIFICATION.md §4.2; `dungeons.test.ts`
//! played instead of scripted).
//!
//! The crawl knows no dungeon by name. Each time it chooses, it floods the ground she can reach
//! from where she stands (closed gates and locked doors are solid props, so they bound it) and
//! takes the first thing it can do there, in this order, nearest first within a kind:
//!
//! 0. mend at a bed or a stove when low (the apples are kept for fights);
//! 1. rest at the dungeon's rest room the first time she can walk to it (a death then wakes her
//!    inside); pick up what is lying about;
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
//! Hurt, she mends where she can before a hop through a door within the dungeon (it may not
//! come back) and before going near a boss. A dungeon's tactic ([`crate::tactics`]) may hold
//! her fire, put a boss earlier, name guards to hunt, and say when the story has what it wants
//! from the place (she walks out then): not while a jar or a gold-leaf page is still to be had
//! near where she can walk, as a thorough player plays it (`Crawl::growth_left`; the growth is
//! the only kind the game has). A jar or page taken up and broken off is gone back to.
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
    /// Talk to someone (a butterfly, with the net: `tactics::forest`).
    Talk(UnitId),
    Door(PropId),
    /// Through a door to or from the dungeon.
    Travel,
    /// Mend at a bed or a stove.
    Rest(PropId),
    /// What a dungeon's tactic asks (`tactics/*.rs`), by its own number: the tactic decides
    /// when it is offered again.
    Tactic(u32),
    /// Walk to ground she has not seen.
    Explore(i32, i32),
}

/// How near a jar or a page she cannot yet get to (cells) a thing to do or ground to walk is
/// taken to be on the way to it (`Crawl::growth_left`).
const GROWTH_NEAR: i32 = 16;

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
    /// The Gold Mine's hoists over Iron Knuckles (`tactics/mine.rs`).
    pub mine: crate::tactics::mine::Mine,
    /// The School's tactic (the rope, the beds: `tactics::school`).
    school: crate::tactics::school::School,
    /// The Burial: whether she has stood still getting nowhere (`tactics::burial::Watch`).
    watch: crate::tactics::burial::Watch,
    /// Out, when done, by a door into this dungeon if one is to hand, not to the county (the
    /// story sets it: the pipes' outfall, up into the Factory the quest goes to next).
    pub leave_to: Option<ZoneId>,
    /// The zone's jars and gold-leaf pages (shown or not), found as she comes in: a try at one
    /// cut short (she was hurt and went to mend, a fight came to her) is taken up again while
    /// it has not failed six times, whether or not anything else has changed.
    growth: std::collections::BTreeSet<PropId>,
    /// The last frame [`Self::growth_left`] held her: things settle for a moment after (a
    /// shelf lets go a few ticks after the second plinth goes down) before she is let go.
    held: Option<u32>,
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
    pub(crate) fn update(&mut self, v: &View<'_>, sig: u64) {
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

/// The growth lying about a zone: its jars and gold-leaf pages, shown or not.
fn growth_props(v: &View<'_>) -> std::collections::BTreeSet<PropId> {
    let cat = jane_data::catalog();
    v.props().filter(|p| matches!(cat.story.prop(p.def).id, "jar" | "jar_big" | "leaf_page")).map(|p| p.id).collect()
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
            mine: crate::tactics::mine::Mine::default(),
            school: crate::tactics::school::School::default(),
            watch: crate::tactics::burial::Watch::default(),
            growth: std::collections::BTreeSet::new(),
            held: None,
            leave_to: None,
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

    /// What the task in hand is about, by name and place (for a debugging line).
    pub fn doing(&self, v: &View<'_>) -> String {
        let cat = jane_data::catalog();
        match self.task.as_ref().map(|(_, w)| *w) {
            Some(Try::Prop(p) | Try::Cast(p) | Try::Door(p) | Try::Rest(p) | Try::Push(p, _)) => {
                v.prop(p).map_or("?".into(), |p| format!("{} at {:?}", v.name(p.key), p.cell))
            }
            Some(Try::Fight(u) | Try::Talk(u)) => {
                v.unit(u).map_or("?".into(), |u| format!("{} at {:?}", cat.combat.unit(u.def).id, u.pos.cell()))
            }
            w => format!("{w:?}"),
        }
    }

    fn stop(&mut self, why: String, notes: &mut Vec<Mark>) {
        notes.push(Mark::Stuck(why.clone()));
        self.stuck = Some(why);
        self.stage = Stage::Done;
    }

    pub fn think(&mut self, v: &View<'_>, cx: &mut Ctx, events: &[jane_sim::Event], notes: &mut Vec<Mark>) -> Act {
        self.frames += 1;
        cx.run = false;
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
                // Only a death in the dungeon counts against it (one on the road to it is the
                // county's: a crow on the way is not the dungeon beating her).
                if v.zone() == self.zone {
                    self.deaths_at.push(format!("at {at:?} by {by} while {doing}"));
                }
                // What she died doing is a try that failed (it is not walked back into blind).
                if let Some((_, what)) = self.task {
                    self.failed(what, &format!("she died doing it, by {by}"));
                }
                self.task = None;
                // She wakes whole: whatever she was backing off from is not after her now (a
                // flight left counting would run her from the next thing she meets).
                cx.fight.fleeing = 0;
                cx.fight.retreat = None;
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
                    self.growth = growth_props(v);
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
                // The Factory: what the story wants from it is in hand (tactics::works).
                let won = boss_of(self.zone).is_some_and(|b| self.bosses.iter().any(|&(d, _)| d == b));
                if won {
                    self.reach.update(v, signature(v));
                }
                if crate::tactics::works::done(v, &self.reach, self.zone, won) && !self.growth_left(v, cx) {
                    self.stage = Stage::Leave;
                    self.task = None;
                    cx.fight.hunt = None;
                    notes.push(Mark::Note("what the story wants is in hand: leaving".into()));
                }
            }
        }
        self.look(v);
        // The Gold Mine: Iron Knuckles is fought under the hoists (DUNGEONS.md §3.1).
        if let Some(a) = self.mine.think(v, cx) {
            return a;
        }
        // A dungeon done with what the story needs from it (`tactics/`): out.
        if self.stage == Stage::Explore && crate::tactics::museum::done(v) && !self.growth_left(v, cx) {
            self.stage = Stage::Leave;
            self.task = None;
            notes.push(Mark::Note("what the story needs is in the bag: leaving".into()));
        }
        // A boss room's own play (`tactics/`), before the general fight.
        // (The flood is looked at again first: a lock-in behind her changes the ground.)
        if v.zone() == self.zone {
            self.reach.update(v, signature(v));
        }
        if let Some(a) = crate::tactics::museum::fight(v, cx, &self.reach) {
            return a;
        }
        crate::tactics::works::observe(v, cx);
        // What a dungeon's own idea has her notice each frame (tactics/*.rs).
        crate::tactics::forest::look(v, cx);
        // The Burial: where her feet keep off, and out of the sight of a small snake she woke.
        if v.zone() == jane_core::ZoneId::Burial {
            if let Some(a) = crate::tactics::burial::before(v, cx) {
                return a;
            }
        }
        // The Burial: stood still getting nowhere (her task and a fight pulling two ways, a thing
        // after her that cannot get round to her), the task is chosen afresh and what is not at
        // her elbow is let be a while (`tactics::burial::Watch`). Nothing is marked failed: the
        // task was not what stopped her.
        if v.zone() == jane_core::ZoneId::Burial && self.watch.stalled(v) {
            self.task = None;
        }
        let calm = v.zone() == jane_core::ZoneId::Burial && self.watch.calm(v);
        // The Museum: the armours are walked past, not fought (tactics::museum::walk_past).
        let doing = self.task.as_ref().map(|(_, w)| *w);
        if let Some(id) = fight::threat(v, cx)
            .filter(|&id| !calm || crate::tactics::burial::at_elbow(v, id))
            .filter(|&id| !crate::tactics::museum::walk_past(v, id, doing))
        {
            // The Burial: nothing is chased into the sight of a snake still to be fed.
            match (v.zone() == jane_core::ZoneId::Burial)
                .then(|| crate::tactics::burial::fight(v, cx, id, self.task.as_ref().map(|(t, _)| t)))
                .flatten()
            {
                Some(Some(a)) => return a,
                Some(None) => {}
                None => {
                    if let Some(a) = fight::engage(v, cx, id) {
                        return a;
                    }
                }
            }
        }
        if v.zone() == jane_core::ZoneId::Burial {
            if let Some(c) = fight::eat(v) {
                return Act::press(c);
            }
        }
        // Idle (about to wait out the night, or nothing chosen) under fire from something with no
        // feet she is not fighting: out of its reach first (`fight::out_of_fire`).
        if self.task.is_none() {
            if let Some(f) = fight::out_of_fire(v, cx) {
                self.task = None;
                return Act::hold(f);
            }
        }
        let sig = signature(v);
        // A dungeon shut for the night (tactics/*.rs): the night waited out by its fire (the
        // county's night is worse), and not counted as the crawl's time.
        if self.stage == Stage::Explore && !matches!(self.task, Some((_, Try::Rest(_)))) {
            self.reach.update(v, sig);
            if let Some(a) = crate::tactics::forest::night(v, cx, &self.reach) {
                self.frames = self.frames.saturating_sub(1);
                self.task = None;
                return a;
            }
        }
        // A dungeon whose story is done (tactics/*.rs): out by a door, the rest left for later.
        let down = boss_of(self.zone).is_some_and(|b| self.bosses.iter().any(|&(d, _)| d == b));
        if self.stage == Stage::Explore && down {
            self.reach.update(v, sig);
            if crate::tactics::forest::done(v, &self.reach) && !self.growth_left(v, cx) {
                self.stage = Stage::Leave;
                self.task = None;
                notes.push(Mark::Note("what the story wants is done: leaving".into()));
            }
        }
        // Low: whatever she was doing waits for a bed or a stove she can reach, and the apples
        // are kept for a fight; shut in with a boss there is none, and she eats and carries on.
        if sense::hp_permille(v.body()) < 500 {
            self.reach.update(v, sig);
            match self.rest_in_reach(v, cx) {
                None => {
                    if let Some(c) = fight::eat(v) {
                        return Act::press(c);
                    }
                }
                // Walking out too: the way out is no shorter for being walked half dead.
                Some(p) if self.task.as_ref().is_none_or(|(_, w)| !matches!(w, Try::Rest(_))) => {
                    self.task = Some((Task::Use(UseProp::new(p)), Try::Rest(p)));
                    // What she was hunting waits too (else the fight walks her back to it).
                    cx.fight.hunt = None;
                }
                Some(_) => {}
            }
        }
        // The School keeps its apples: hurt, she breaks off for the sick bay fire all the same.
        if v.zone() == ZoneId::School && self.task.as_ref().is_some_and(|(_, w)| !matches!(w, Try::Rest(_))) {
            self.reach.update(v, sig);
            if crate::tactics::school::breaks_off(v, &self.reach) {
                // Broken off, not failed: worth another go once she is whole.
                if let Some((_, what)) = self.task.take() {
                    if let Some(e) = self.tried.get_mut(&what) {
                        e.0 = 0;
                    }
                }
            }
        }
        // The Burial is done with once its keeper is down and his box is in her bag (`tactics::burial`).
        if v.zone() == jane_core::ZoneId::Burial
            && self.stage == Stage::Explore
            && crate::tactics::burial::done(v, &self.bosses)
            && !self.growth_left(v, cx)
        {
            self.stage = Stage::Leave;
            self.task = None;
            notes.push(Mark::Note("what she came down for is in her bag: leaving".into()));
        }
        // The Burial's tactics cut in on whatever she was doing (something to feed in view).
        if v.zone() == jane_core::ZoneId::Burial && self.task.is_some() && self.frames % 15 == 0 {
            let cut = self
                .task
                .as_ref()
                .map(|(t, w)| crate::tactics::burial::cuts_in(v, cx, &self.reach, t, *w))
                .unwrap_or_default();
            if cut.into_iter().any(|w| self.fresh(w, sig)) {
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
                self.reach.update(v, sig);
                let reach = &self.reach;
                let next = self.leave_to.filter(|&z| {
                    sense::doors_to(v, z).iter().any(|p| !p.hidden && sense::can_open(v, p) && reach.beside(p))
                });
                match story::route(v, cx, next.unwrap_or(jane_core::ZoneId::County)) {
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
        // On the county road to it, what she would lose to is run past (`fight::outrun`).
        cx.run = false;
        if let Some(id) = fight::threat(v, cx) {
            if self.task.is_some() && fight::outrun(v, cx, id) {
                cx.run = true;
            } else if let Some(a) = fight::engage(v, cx, id) {
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

    /// Is growth still on offer here, so that a dungeon's story is not yet done with (a tactic's
    /// `done`, the Burial's box)? She plays a place as a thorough player does: a jar or a
    /// gold-leaf page not yet opened, beside ground she can walk and not given up on, is taken
    /// before she walks out (DUNGEONS.md §3, "Growth on the story's path").
    fn growth_left(&mut self, v: &View<'_>, cx: &Ctx) -> bool {
        let sig = signature(v);
        self.reach.update(v, sig);
        let zone = v.zone();
        let left = |p: &Prop| self.growth.contains(&p.id) && !p.used && !cx.used.contains_key(&(zone, p.id));
        let now = v.frame();
        if v.props()
            .any(|p| left(p) && !p.hidden && !p.locked && self.reach.beside(p) && self.fresh(Try::Prop(p.id), sig))
        {
            self.held = Some(now);
            return true;
        }
        // Growth in sight but not to hand (on a shelf a puzzle lets go, down a passage not yet
        // walked, across ground that has not been opened): what can be done near it is done
        // first. And a thing a verb mends or breaks open to show what is behind it (a damaged
        // door, a cracked case), or clears out of the way (a fall of rock), is worth the verb.
        let near: Vec<Vec2> = v.props().filter(|p| left(p) && !p.hidden).map(sense::prop_centre).collect();
        let by = |at: Vec2| near.iter().any(|&g| dist(g, at) <= i64::from(GROWTH_NEAR * CELL_FX));
        let opens = |p: &Prop| {
            sense::prop_does(v, p, &|a| {
                matches!(
                    a,
                    jane_core::action::Action::Show(_)
                        | jane_core::action::Action::Unlock(_)
                        | jane_core::action::Action::Hide(_)
                )
            })
        };
        let more = self
            .choose_where(v, cx, sig, &|w| match w {
                Try::Cast(id) => v.prop(id).is_some_and(|p| opens(p) || by(sense::prop_centre(p))),
                Try::Prop(id) | Try::Door(id) => v.prop(id).is_some_and(|p| by(sense::prop_centre(p))),
                Try::Push(_, plate) => v.prop(plate).is_some_and(|p| by(sense::prop_centre(p))),
                Try::Explore(x, y) => by(Vec2::centre(x, y)),
                _ => false,
            })
            .is_some();
        if more {
            self.held = Some(now);
        }
        more || self.held.is_some_and(|t| now < t + 90)
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

    /// Can she stand beside `p` (any side of it, not only the one nearest her)?
    pub fn reaches_prop(&mut self, v: &View<'_>, p: &jane_sim::Prop) -> bool {
        self.reach.update(v, signature(v));
        self.reach.beside(p)
    }

    fn fresh(&self, what: Try, sig: u64) -> bool {
        match (what, self.tried.get(&what)) {
            (Try::Tactic(_), _) | (_, None) => true,
            // A fight is taken up again whenever she is ready to (it may have healed; so has she).
            (Try::Fight(_), Some(&(_, n))) => n < 12,
            (Try::Prop(id), Some(&(_, n))) if self.growth.contains(&id) => n < 6,
            (_, Some(&(s, n))) => s != sig && n < 6,
        }
    }

    fn choose(&mut self, v: &View<'_>, cx: &Ctx, sig: u64) -> Option<(Task, Try)> {
        // The School: the sick bay fire first; the rope and the beds when nothing else is left.
        if v.zone() == ZoneId::School {
            let mut s = std::mem::take(&mut self.school);
            if s.take_woke() {
                self.tried.retain(|w, _| !matches!(w, Try::Cast(_) | Try::Prop(_) | Try::Pickup(_)));
            }
            let down = boss_of(self.zone).is_some_and(|b| self.bosses.iter().any(|&(d, _)| d == b));
            let t = crate::tactics::school::first(&mut s, v, cx, &self.reach, &|w| self.fresh(w, sig), down)
                .or_else(|| self.choose_any(v, cx, sig))
                .or_else(|| crate::tactics::school::last(&mut s, v, &self.reach));
            self.school = s;
            return t;
        }
        self.choose_any(v, cx, sig)
    }

    fn choose_any(&self, v: &View<'_>, cx: &Ctx, sig: u64) -> Option<(Task, Try)> {
        self.choose_where(v, cx, sig, &|_| true)
    }

    /// [`Self::choose_any`], offering only what `keep` lets through.
    fn choose_where(&self, v: &View<'_>, cx: &Ctx, sig: u64, keep: &dyn Fn(Try) -> bool) -> Option<(Task, Try)> {
        let cat = jane_data::catalog();
        let at = v.body().pos;
        let reach = &self.reach;
        let near_prop = |p: &Prop| sense::to_prop(p, at);
        let mut best: Option<(u8, i64, Try, Task)> = None;
        let offer = |class: u8, cost: i64, what: Try, t: Task, best: &mut Option<(u8, i64, Try, Task)>| {
            // The Burial: a corner is not gone into before she has what it asks (`tactics::burial`).
            if !keep(what)
                || !self.fresh(what, sig)
                || v.zone() == jane_core::ZoneId::Burial && crate::tactics::burial::not_yet(v, &t)
            {
                return;
            }
            // Not where she fell a little while ago (a boss excepted: that fight is the dungeon).
            if class < 9 && task_point(v, &t).is_some_and(|p| cx.fell_near(v.zone(), p.cell(), FELL_R, v.tick().0)) {
                return;
            }
            if best.as_ref().is_none_or(|(c, k, w, _)| (class, cost, what) < (*c, *k, *w)) {
                *best = Some((class, cost, what, t));
            }
        };
        // 0. Low: a bed or a stove she can reach (the apples are for a fight).
        if sense::hp_permille(v.body()) < 500 {
            if let Some(p) = self.rest_in_reach(v, cx).filter(|&p| keep(Try::Rest(p))) {
                return Some((Task::Use(UseProp::new(p)), Try::Rest(p)));
            }
        }
        // What the dungeon's own idea puts first (`tactics/`).
        if let Some((t, what)) =
            crate::tactics::museum::first(v, cx, reach).filter(|(_, w)| keep(*w) && self.fresh(*w, sig))
        {
            return Some((t, what));
        }
        // 1. The dungeon's rest room, the first time she can walk to it: a death then wakes her
        // inside, not out on the county road (the pipes' and the Factory's walks back in were
        // what killed her).
        if let Some(p) = self.rest_in_reach(v, cx) {
            if !self.tried.contains_key(&Try::Rest(p)) {
                let d = v.prop(p).map_or(0, near_prop);
                offer(1, d, Try::Rest(p), Task::Use(UseProp::new(p)), &mut best);
            }
        }
        // 1. Lying about.
        for d in v.drops() {
            if reach.near(d.pos, 1) {
                offer(1, dist(at, d.pos), Try::Pickup(d.id), Task::Pickup { drop: d.id, t: 0 }, &mut best);
            }
        }
        for p in v.props() {
            if p.hidden || !reach.beside(p) || crate::tactics::forest::skip(v, p) {
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
                        // The Factory: a floor grid, a fuse by the Foreman, wait (tactics::works).
                        if crate::tactics::works::hold_fire(v, p) {
                            continue;
                        }
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
                && !turns_clock(v, p)
                && !cx.used.contains_key(&(v.zone(), p.id))
            {
                offer(4, d, Try::Prop(p.id), Task::Use(UseProp::new(p.id)), &mut best);
            }
        }
        // What a dungeon's own idea puts up (tactics/*.rs).
        for (class, cost, what, t) in crate::tactics::forest::offers(v, cx, reach) {
            offer(class, cost, what, t, &mut best);
        }
        // 6. Plates that are up, and something to push onto one.
        if best.as_ref().is_none_or(|b| b.0 > 6) {
            // What already holds a plate down stays where it is (else two barrels and two plates
            // are pushed back and forth for ever).
            let holding = |t: &Prop| {
                v.props().any(|q| cat.story.prop(q.def).plate && q.on && prop_rect(q).overlaps(prop_rect(t)))
            };
            for plate in v.props().filter(|p| !p.hidden && cat.story.prop(p.def).plate && !p.on && reach.beside(p)) {
                for thing in v.props().filter(|p| {
                    let d = cat.story.prop(p.def);
                    !p.hidden && d.push && p.solid && reach.beside(p) && !holding(p)
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
        // 7. Whatever hostile she can reach; bosses last (the Factory's Foreman as soon as she
        // has the verb for him: tactics::works). One she saw and has walked away from (it sleeps
        // out of her sight) is walked back to. With the dungeon's boss down, what is left (and
        // what has come back since) is walked past: it is fought only when it comes at her.
        let burial = v.zone() == jane_core::ZoneId::Burial;
        let cleared = boss_of(self.zone).is_some_and(|b| self.bosses.iter().any(|&(d, _)| d == b));
        for u in sense::enemies(v) {
            // One her feet (or her patience) lately found no way to waits its time; in the
            // Factory, what sees only by light is left be unless it has her (tactics::works).
            if !fight::fightable(u)
                || !reach.point(u.pos)
                || burial && crate::tactics::burial::not_hunted(u.def)
                || cleared
                || !fight::reachable(cx, u.id, v.frame())
                || crate::tactics::works::leave_be(v, u)
                || crate::tactics::museum::leave_be(v, u.def)
            {
                continue;
            }
            // The School hunts only what the story needs down (`tactics::school::hunts`).
            if v.zone() == ZoneId::School && !crate::tactics::school::hunts(u.def) {
                continue;
            }
            let boss = cat.combat.unit(u.def).boss;
            let class = if boss { crate::tactics::works::boss_class(v, u.def) } else { 7 };
            offer(class, dist(at, u.pos), Try::Fight(u.id), Task::Hunt(u.id), &mut best);
        }
        // The Factory: a guard that sees only by light, over a locked thing (tactics::works).
        for (id, pos) in crate::tactics::works::guards(v, reach) {
            offer(7, dist(at, pos), Try::Fight(id), Task::Hunt(id), &mut best);
        }
        for (&def, seen) in &cx.seen_foes {
            let d = cat.combat.unit(def);
            if d.bait.is_some()
                || cleared
                || burial && crate::tactics::burial::not_hunted(def)
                || (d.sight == jane_data::UnitSight::Lit && !d.boss)
                || v.zone() == ZoneId::School && !crate::tactics::school::hunts(def)
                // Butterfly Forest: a thing with no feet left behind stays there (a cactus across
                // the forest is walked back to for nothing, and some cannot be walked to at all).
                || v.zone() == ZoneId::Forest && d.run.0 <= 0 && d.walk.0 <= 0
                || crate::tactics::museum::leave_be(v, def)
            {
                continue;
            }
            for (&id, &(z, pos)) in seen {
                if z != v.zone() || v.unit(id).is_some() || !reach.point(pos) {
                    continue;
                }
                let t = Task::Walk { to: pos, near: jane_core::Fx::from_px(12) };
                let class = if d.boss { crate::tactics::works::boss_class(v, def) } else { 7 };
                offer(class, dist(at, pos) + i64::from(4 * CELL_FX), Try::Fight(id), t, &mut best);
            }
        }
        // A boss is met mended: with nothing left but the boss, a bed or a stove she can reach
        // first, when she is hurt.
        if best.as_ref().is_some_and(|b| b.0 == 9) && sense::hp_permille(v.body()) < 850 {
            if let Some(p) = self
                .rest_in_reach(v, cx)
                .filter(|p| keep(Try::Rest(*p)) && self.tried.get(&Try::Rest(*p)).is_none_or(|t| t.1 < 20))
            {
                return Some((Task::Use(UseProp::new(p)), Try::Rest(p)));
            }
        }
        // The Burial's tactics (feeding what is fed, not fought): `tactics::burial`.
        if v.zone() == jane_core::ZoneId::Burial {
            for (class, cost, what, t) in crate::tactics::burial::offers(v, cx, reach, &self.bosses) {
                offer(class, cost, what, t, &mut best);
            }
        }
        // 10. Ground she has not seen (what sleeps out of sight wakes as she comes). Not in the
        // School: what sleeps there is better left asleep (`tactics::school`).
        if best.is_none() && v.zone() != ZoneId::School {
            if let Some((x, y)) = self.frontier(v).filter(|&(x, y)| keep(Try::Explore(x, y))) {
                let t = Task::Walk { to: Vec2::centre(x, y), near: jane_core::Fx::from_px(6) };
                return Some((t, Try::Explore(x, y)));
            }
            // Nothing in the general order: what the dungeon's own idea asks (`tactics/`).
            if let Some((t, what)) =
                crate::tactics::museum::idle(v, reach).filter(|(_, w)| keep(*w) && self.fresh(*w, sig))
            {
                return Some((t, what));
            }
        }
        // A hop within the dungeon may not come back (the Factory's vent drops into the
        // generator hall), and a boss is met whole (the Charge Hand takes most of her in one
        // grip): hurt, she mends first where she can.
        let hurt = sense::hp_permille(v.body()) < 900;
        if hurt && best.as_ref().is_some_and(|b| b.0 == 8 || Self::near_boss(v, cx, b.2)) {
            if let Some(p) = self
                .rest_in_reach(v, cx)
                .filter(|&p| keep(Try::Rest(p)) && self.tried.get(&Try::Rest(p)).is_none_or(|t| t.1 < 30))
            {
                return Some((Task::Use(UseProp::new(p)), Try::Rest(p)));
            }
        }
        best.map(|(_, _, what, t)| (t, what))
    }

    /// The nearest bed or stove she can walk to, as of the last flood.
    fn rest_in_reach(&self, v: &View<'_>, cx: &Ctx) -> Option<PropId> {
        let cat = jane_data::catalog();
        let at = v.body().pos;
        v.props()
            .filter(|p| {
                !p.hidden
                    && cat.story.prop(p.def).rest
                    && !turns_clock(v, p)
                    && self.reach.beside(p)
                    && v.prop_spawn(p).is_some_and(|s| s.talk.is_some())
                    // The Burial: not a fire in a statue's line (`tactics::burial::shot_at`).
                    && !(v.zone() == ZoneId::Burial && crate::tactics::burial::shot_at(v, cx, p))
            })
            .min_by_key(|p| (sense::to_prop(p, at), p.id))
            .map(|p| p.id)
    }

    /// Does this try take her within twenty cells of a boss standing here (seen, or last seen)?
    fn near_boss(v: &View<'_>, cx: &Ctx, what: Try) -> bool {
        let cat = jane_data::catalog();
        let to = match what {
            Try::Pickup(d) => v.drops().iter().find(|x| x.id == d).map(|x| x.pos),
            Try::Prop(p) | Try::Cast(p) | Try::Door(p) | Try::Push(p, _) => v.prop(p).map(sense::prop_centre),
            Try::Fight(u) | Try::Talk(u) => v.unit(u).map(|u| u.pos),
            Try::Explore(x, y) => Some(Vec2::centre(x, y)),
            Try::Travel | Try::Rest(_) | Try::Tactic(_) => None,
        };
        let Some(to) = to else { return false };
        let r = i64::from(20 * CELL_FX);
        let seen = sense::enemies(v).into_iter().filter(|u| cat.combat.unit(u.def).boss).map(|u| u.pos);
        let known = cx
            .seen_foes
            .iter()
            .filter(|(d, _)| cat.combat.unit(**d).boss)
            .flat_map(|(_, m)| m.values())
            .filter(|(z, _)| *z == v.zone())
            .map(|&(_, p)| p);
        seen.chain(known).any(|b| dist(b, to) <= r)
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
        for u in sense::talkers(v) {
            let reach = if self.reach.near(u.pos, 2) { "" } else { " (out of reach)" };
            let _ = write!(out, " talker {} at {:?}{reach};", cat.combat.unit(u.def).id, u.pos.cell());
        }
        out
    }
}

/// A bed that sleeps the world forward to an hour (`rest` with `until`): never lain on in passing
/// or to mend (the night it passes stands the dead up again); a dungeon's tactic says when.
fn turns_clock(v: &View<'_>, p: &Prop) -> bool {
    jane_data::catalog().story.prop(p.def).rest
        && sense::prop_does(v, p, &|a| matches!(a, jane_core::action::Action::Rest { until: Some(_) }))
}

/// Shut in with the zone's boss: it stands where she can walk, and no door out of the zone is
/// where she can walk (a lock-in behind her). Nothing but the fight lets her out.
pub fn shut_in_with_boss(v: &View<'_>) -> bool {
    if !v.body().alive {
        return false;
    }
    let Some(boss) = boss_of(v.zone()).and_then(|b| sense::units_of(v, b).into_iter().next()) else { return false };
    let mut reach = Reach::default();
    reach.update(v, 0);
    // A fire or a bed she can walk to is a way to go on (the School's bolted front doors leave her
    // no door out, and its sick bay is still hers): only the boss's own lock-in shuts her in.
    let cat = jane_data::catalog();
    reach.point(boss.pos)
        && !v.props().any(|p| {
            door_of(v, p).is_some_and(|d| d.zone != v.zone()) && reach.beside(p)
                || !p.hidden && cat.story.prop(p.def).rest && reach.beside(p)
        })
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
    let s = jane_data::catalog().combat.spell(spell);
    // A bolt switches what it ends beside: a prop that stops it (a brazier, a fuse board) where
    // it hits, a torch standing in the open only where its flight runs out, or against the wall
    // behind it. From each place she could stand, where would this bolt end?
    let touch = i64::from(s.touch.unwrap_or(jane_core::Fx::from_px(14)).0) - 2 * 256;
    let mut best: Option<(i64, Vec2)> = None;
    for r in 2..=22 {
        for (dx, dy) in [(0, r), (0, -r), (r, 0), (-r, 0), (r, r), (-r, r), (r, -r), (-r, -r)] {
            let (x, y) = (px + dx, py + dy);
            if !reach.get(x, y) {
                continue;
            }
            let at = Vec2::centre(x, y);
            // Where the bolt ends, as the sim flies it: on the prop (a prop that stops shots, a
            // fuse box or a socket, is touched where the bolt stops on its face), or near enough
            // its middle; or, for a thing that blocks sight and hides its own middle (a fallen
            // rock, a web across a passage), in sight of its near face, or it the first thing
            // seen that way, and within the bolt's flight (from farther it falls short).
            let end = bolt_end(v, at, jane_core::angle::bearing(at, c), spell);
            let (ex, ey) = end.cell();
            let on_it = dist(end, c) <= touch || prop_rect(p).contains(ex, ey);
            let face = face_toward(p, at);
            let in_range = dist(at, face) <= i64::from(s.range.0);
            let hidden = !v.sight(at, c) && in_range && (v.sight(at, face) || first_seen_is(v, at, c, p));
            if !on_it && !hidden {
                continue;
            }
            let d = dist(me, at);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, at));
            }
        }
    }
    let (_, from) = best?;
    Some(Task::Aim { spell, from, at: c, t: 0 })
}

/// Where a bolt of `spell` thrown from `from` along `heading` ends, as the sim flies it
/// (`flight.rs`): it starts a little ahead of her feet, moves its speed a tick, dies on the first
/// move into a cell that stops a shot, or once it has flown its range and two of her bodies.
fn bolt_end(v: &View<'_>, from: Vec2, heading: jane_core::Angle, spell: SpellId) -> Vec2 {
    let cat = jane_data::catalog();
    let s = cat.combat.spell(spell);
    let speed = s.speed.unwrap_or(jane_core::Fx::from_px(2));
    let mut left = i64::from(s.range.0) + 2 * i64::from(cat.combat.unit(v.body().def).bounds.0);
    let vel = jane_core::angle::along(heading, speed);
    let mut pos = from + jane_core::angle::along(heading, jane_core::Fx::from_px(4));
    for _ in 0..400 {
        let to = pos + vel;
        if shot_stopped(v, pos, to) {
            return to;
        }
        pos = to;
        left -= i64::from(speed.0.max(1));
        if left <= 0 {
            break;
        }
    }
    pos
}

/// Does a move from `a` to `b` enter a cell that stops a shot (the sim's grid walk,
/// `los::first_blocked_cell`, over the flags the view shows)?
fn shot_stopped(v: &View<'_>, a: Vec2, b: Vec2) -> bool {
    let cell = i64::from(CELL_FX);
    let (x0, y0) = (i64::from(a.x.0), i64::from(a.y.0));
    let (dx, dy) = (i64::from(b.x.0) - x0, i64::from(b.y.0) - y0);
    let (mut cx, mut cy) = a.cell();
    let (tx, ty) = b.cell();
    let (sx, sy) = (if dx > 0 { 1 } else { -1 }, if dy > 0 { 1 } else { -1 });
    let (adx, ady) = (dx.abs(), dy.abs());
    let mut nx = if dx > 0 { (i64::from(cx) + 1) * cell - x0 } else { x0 - i64::from(cx) * cell };
    let mut ny = if dy > 0 { (i64::from(cy) + 1) * cell - y0 } else { y0 - i64::from(cy) * cell };
    let mut steps = (tx - cx).abs() + (ty - cy).abs();
    while steps > 0 {
        steps -= 1;
        let x_first = if dx == 0 {
            false
        } else if dy == 0 {
            true
        } else {
            nx * ady < ny * adx
        };
        if x_first {
            nx += cell;
            cx += sx;
        } else {
            ny += cell;
            cy += sy;
        }
        if v.flags(cx, cy) & jane_core::tile::BLOCK_SHOT != 0 {
            return true;
        }
    }
    false
}

/// Is the first thing that stops sight on the way from `a` to `b` the prop itself (a web across
/// a passage blocks sight, and it is what the bolt is for)? A quarter cell at a time.
pub fn first_seen_is(v: &View<'_>, a: Vec2, b: Vec2, p: &Prop) -> bool {
    let r = prop_rect(p);
    let n = (dist(a, b) / i64::from(CELL_FX / 4)).max(1);
    let start = a.cell();
    for i in 1..=n {
        let q = Vec2::new(
            jane_core::Fx(a.x.0 + ((i64::from(b.x.0 - a.x.0) * i) / n) as i32),
            jane_core::Fx(a.y.0 + ((i64::from(b.y.0 - a.y.0) * i) / n) as i32),
        );
        let (x, y) = q.cell();
        if (x, y) != start && v.flags(x, y) & jane_core::tile::BLOCK_SIGHT != 0 {
            return r.contains(x, y);
        }
    }
    false
}

/// The point of a prop's footprint nearest `at`, a quarter cell out toward it (in the free cell
/// before its face).
fn face_toward(p: &Prop, at: Vec2) -> Vec2 {
    let r = prop_rect(p);
    let q = CELL_FX / 4;
    let clamp = |v: i32, lo: i32, hi: i32| {
        if v < lo {
            lo - q
        } else if v > hi {
            hi + q
        } else {
            v
        }
    };
    Vec2::new(
        jane_core::Fx(clamp(at.x.0, r.x * CELL_FX, r.right() * CELL_FX)),
        jane_core::Fx(clamp(at.y.0, r.y * CELL_FX, r.bottom() * CELL_FX)),
    )
}

/// Cells about where she fell that the crawl lets be a while ([`crate::task::Ctx::fell_near`]).
const FELL_R: i32 = 7;

/// Where a task takes her: the prop, the drop, the unit, the point.
fn task_point(v: &View<'_>, t: &Task) -> Option<Vec2> {
    match t {
        Task::Use(u) => v.prop(u.prop).map(sense::prop_centre),
        Task::WorldCast { prop, .. } => v.prop(*prop).map(sense::prop_centre),
        Task::Push(p) => v.prop(p.prop).map(sense::prop_centre),
        Task::Pickup { drop, .. } => v.drops().iter().find(|d| d.id == *drop).map(|d| d.pos),
        Task::Aim { at, .. } => Some(*at),
        Task::Hunt(id) => v.unit(*id).map(|u| u.pos),
        Task::Walk { to, .. } => Some(*to),
        _ => None,
    }
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

/// Every `grow` a prop of a blueprint gives (its `use` list, an `if` both ways, and the
/// conversation it opens), as (stat, amount, the finding's id): for tools that ask which were found.
pub fn grows_of(
    bp: &jane_core::Blueprint,
    p: &jane_core::blueprint::PropSpawn,
) -> Vec<(jane_core::action::Stat, i16, jane_core::Key)> {
    fn walk(
        bp: &jane_core::Blueprint,
        acts: &[jane_core::Action],
        out: &mut Vec<(jane_core::action::Stat, i16, jane_core::Key)>,
    ) {
        use jane_core::action::Action;
        for a in acts {
            match *a {
                Action::Grow { stat, amount, id } => out.push((stat, amount, id)),
                Action::If { then, els, .. } => {
                    walk(bp, list_of(Some(bp), then), out);
                    if let Some(e) = els {
                        walk(bp, list_of(Some(bp), e), out);
                    }
                }
                _ => {}
            }
        }
    }
    let cat = jane_data::catalog();
    let mut out = Vec::new();
    if let Some(u) = p.use_list {
        walk(bp, list_of(Some(bp), u), &mut out);
    }
    for n in p.talk.map(|t| cat.story.dialogue(t).nodes).unwrap_or_default() {
        for a in n.actions.into_iter().chain(n.options.iter().filter_map(|o| o.actions)) {
            walk(bp, cat.list(a), &mut out);
        }
    }
    out
}

/// The growth on offer in a blueprint: every `grow` in its props' `use` lists and in the
/// conversations its props open (jars, gold-leaf pages, the library's margins).
pub fn growth_in(bp: &jane_core::Blueprint) -> Growth {
    let cat = jane_data::catalog();
    let mut out = (0, 0);
    let later = rooms_for_later(bp);
    for p in &bp.props {
        if later.iter().any(|r| r.contains(i32::from(p.cell.x), i32::from(p.cell.y))) {
            continue;
        }
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

/// The rooms of a dungeon that ask a verb she does not have by the time she is done with it
/// (neither known on arrival, `givenVerbs`, nor granted by a room of the place): Butterfly
/// Forest's seed tree wants the Fireball the Burial gives, its lamp glade the Factory's spark.
/// They are for coming back to, and what they hold is not counted as found on the story's way
/// (DUNGEONS.md §3, "Growth on the story's path").
pub fn rooms_for_later(bp: &jane_core::Blueprint) -> Vec<jane_core::Rect> {
    let cat = jane_data::catalog();
    let Some(m) = cat.dungeons.mission_of(bp.zone) else { return Vec::new() };
    let granted = |s: &jane_core::SpellId| {
        m.given_verbs.contains(s)
            || m.nodes.iter().any(|n| n.grants.iter().any(|g| matches!(g, jane_data::MissionGrant::Verb(v) if v == s)))
    };
    m.nodes
        .iter()
        .filter(|n| !n.demands.iter().all(granted))
        .flat_map(|n| n.names.iter().flat_map(|t| t.rects.iter()))
        .filter_map(|&r| bp.rects.get(&jane_core::Key::Name(r)).copied())
        .collect()
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
    // The bait for what in there is fed rather than fought (the dog: "The small snakes down
    // there cannot be fought. They can be fed."), made at the bench before she goes: one each.
    let mut baits: BTreeMap<ItemId, u16> = BTreeMap::new();
    for u in &dungeon.units {
        if let Some(b) = cat.combat.unit(u.def).bait {
            *baits.entry(b).or_default() += 1;
        }
    }
    for (item, qty) in baits {
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
