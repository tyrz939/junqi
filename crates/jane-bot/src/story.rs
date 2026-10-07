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
    /// Deadwood off a stump on the way, while fires are made.
    Gather,
    /// Home before dark: the night slept away in Julie's bed.
    Sleep,
    /// Home to Julie's cupboard with a bag nearly full of what destroy refuses (`sense::stow_slot`).
    Stow,
    /// Ready for an act: a potion brewed at Julie's bench, or food picked up, before the dungeon.
    Provision(ItemId),
    /// In a dungeon with the quest's thing out of reach: what the crawl would do next.
    Explore(crate::crawl::Try),
    /// The Lost, looking for step `.1` of a quest from its words (255: whoever takes it back).
    Search(QuestId, u8),
    /// The Explorer, to the map's edge: the fog block it makes for.
    Frontier(i32, i32),
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
    /// Nothing to do for it for this many hours (whoever takes it back is not about till then).
    Later(u8),
}

#[derive(Debug, Default)]
pub struct Story {
    task: Option<(Task, Goal)>,
    /// The zone she was in last frame, and the frames she lately changed zone on (in and out of
    /// a door every frame is a goal on each side of it that wants the other: [`FLIPS`]).
    zone_was: Option<ZoneId>,
    flips: Vec<u32>,
    fails: BTreeMap<Goal, u32>,
    /// Set aside until this tick (a night slept counts: the clock is what she waits on).
    blocked: BTreeMap<Goal, u32>,
    /// The goal last finished, and how often in a row it was chosen again at once.
    last: Option<Goal>,
    again: u32,
    /// Quest steps reached since the log last moved, and how often each: two steps that each
    /// want an hour she is not at (Mrs Loveday's walk "after the bell", a loaf on a step) were
    /// reached by turns all day on seed 1, never twice in a row, and the Choice never came.
    reached: BTreeMap<Goal, u32>,
    /// The log's progress when `reached` was last cleared ([`log_progress`]).
    progress: u64,
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
    /// A dungeon that killed her over and over, let be until this tick (and how often): not
    /// cleared when she wakes whole, as a set-aside is (`died_out`).
    died_out: BTreeMap<ZoneId, (u32, u32, i32)>,
    /// A goal that got her killed on the way to it, how often, and the tick it waits till: not
    /// cleared when she wakes whole (`died_for`).
    killed_on: BTreeMap<Goal, (u32, u32)>,
    /// The tick Yours to Say was first ready: not told how to end it, she takes it in once it
    /// has waited [`CHOICE_PATIENCE`] (the errands round town otherwise outlast her, more so
    /// since the day was halved and their set-asides run out sooner).
    choice_ready: Option<u32>,
    /// A dungeon a quest step sends her into, played whole by a crawl (in by its door, through
    /// its locks and verbs to its boss, and out), and the step it is for.
    dungeon: Option<(Box<crate::crawl::Crawl>, Goal)>,
    /// Nothing to do: the frame to look again.
    quiet_until: u32,
    /// The Lost's and the Explorer's looking.
    pub looking: Looking,
}

/// What the Lost has spent looking for each step, and what she gave up on; where the Lost and
/// the Explorer found no way.
#[derive(Debug, Default)]
pub struct Looking {
    /// Frames spent looking for each step (255: the hand-in).
    pub frames: BTreeMap<(QuestId, u8), u32>,
    /// Steps looked for past [`crate::lost::SEARCH_BUDGET`]: from then she is told.
    pub gave_up: std::collections::BTreeSet<(QuestId, u8)>,
    /// Steps found after looking (logged once).
    pub found: std::collections::BTreeSet<(QuestId, u8)>,
    /// Road frontier blocks with no way to them.
    pub bad_road: std::collections::BTreeSet<(i32, i32)>,
    /// Fog blocks with no way to them.
    pub bad_fog: std::collections::BTreeSet<(i32, i32)>,
    /// Each step's landmark words (the words do not change on a seed).
    pub words: BTreeMap<(QuestId, u8), Vec<String>>,
    /// Leads walked to their end (a sign, and which place on it).
    pub spent: std::collections::BTreeSet<(PropId, u8)>,
    /// Steps walked to by a sign's way: the target, and how many signs she had read then (a
    /// newer one, or the target come on screen, turns the walk).
    leading: BTreeMap<(QuestId, u8), (Target, u32)>,
}

/// Cells a quest's thing may be from the Explorer for it to go and do it (quests are incidental).
const EXPLORER_NEAR: i32 = 48;

/// What a search's walk costs over its distance, cells: what she knows comes first, and so does
/// reading what she passes (a sign may name the place).
const SEARCH_EXTRA: i32 = 120;

/// A landmark the words name costs less than a road not walked.
const LANDMARK_EXTRA: i32 = 40;

/// Changes of zone, within frames, that are a door gone in and out of for nothing.
const FLIPS: (usize, u32) = (8, 120);

/// A way a sign gave costs least of what she does not know: it is the way.
const LEAD_EXTRA: i32 = 20;

/// Squared cells between two cells.
fn sq(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(a.0 - b.0), i64::from(a.1 - b.1));
    dx * dx + dy * dy
}

/// What a walk to the map's edge costs over its distance, cells (the Explorer).
const FRONTIER_EXTRA: i32 = 30;

/// What she packs for an act, and how many of each: Stone Skin and a life-steal from the bench
/// (the roses, stones and flowers she has picked up on the way), and food.
const PROVISIONS: [(&str, u32); 3] = [("potion_stoneskin", 2), ("potion_lifesteal", 1), ("apple", APPLES_HELD)];

/// The apples she carries when she can.
const APPLES_HELD: u32 = 6;

/// Short of food (under this many apples), she will go twice as far for more: a walk to a
/// fire with none in her bag was the county's commonest death.
const APPLES_SHORT: u32 = 3;

/// What she brews for Under the Stone, and how many: the dog's bait for the small snakes ("Feed
/// the small snakes; do not fight them"), rat meat soaked in Stranglethorn at a bench. There is
/// no bench inside, so it is made before she goes down; two, for the two in the east hall.
pub const BAIT: (&str, u32) = ("poisoned_rat_meat", 2);

/// Cells she will go for a provision: the bench, or food seen near (not across the county: a
/// long walk for an apple was a walk through the ruffians, and she mostly packs at home).
const PROVISION_REACH: i32 = 300;

/// Ticks Yours to Say waits, ready, before a Reader not told how to end it takes it in: eight
/// days (the nights slept count).
const CHOICE_PATIENCE: u32 = 8 * jane_sim::tuning::TICKS_PER_DAY;

/// Cells from Julie's door within which she walks home for the night.
const HOME_NEAR: i32 = 220;

/// Cells to a fire she would sit the night out by instead (and at most half the way home).
const FIRE_NEAR: i32 = 160;

/// Frames between looks when there was nothing to do.
const QUIET: u32 = 30;

/// Frames a dungeon's crawl may run before the story takes her out of it and tries later: forty
/// real minutes (the crawl test gives one twenty).
const CRAWL_FRAMES: u32 = 60 * 60 * 40;

/// The Burial's: five keepers deep, each a walk back to a fire first, it takes the crawl thirty
/// to fifty-five minutes (its test gives an hour).
const BURIAL_FRAMES: u32 = 60 * 60 * 70;

/// A zone a quest step is played in whole: a dungeon, not the county or the house.
pub fn dungeon(z: ZoneId) -> bool {
    !matches!(z, ZoneId::County | ZoneId::House)
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

/// The crawl for a quest step in dungeon `z`: out, when it is done, by a door into the dungeon
/// the quest's next step is in if there is one to hand (the pipes' outfall, up into the Factory),
/// else to the county.
fn crawl_for(v: &View<'_>, z: ZoneId, g: Goal, lost: bool) -> crate::crawl::Crawl {
    let mut c = crate::crawl::Crawl::new(z);
    if let Goal::Step(q, i) = g {
        // The Lost reads the boards beside the doors for the room the step names.
        if lost {
            c.want = crate::crawl::room_words(&crate::lost::step_text(v, q, Some(usize::from(i))), z);
        }
        let cat = jane_data::catalog();
        let def = cat.story.quest(q);
        let counts = v.quests().find(|x| x.quest == q);
        c.leave_to = (usize::from(i) + 1..def.requirements.len())
            .filter(|&j| counts.as_ref().is_none_or(|x| x.count(j) < def.requirements[j].qty))
            .filter_map(|j| zone_of_step(q, j))
            .find(|&n| n != z && dungeon(n));
    }
    c
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

    /// The objective in hand (a dungeon's step while its crawl plays it).
    pub fn objective(&self) -> Option<Goal> {
        self.dungeon.as_ref().map(|(_, g)| *g).or(self.task.as_ref().map(|(_, g)| *g))
    }

    /// What it is doing, for a debugging line.
    pub fn status(&self) -> String {
        if let Some((c, g)) = &self.dungeon {
            return format!("{g:?} by crawling the {}: {}", c.zone.name(), c.status());
        }
        match &self.task {
            Some((t, g)) => format!("{g:?} {t:?}"),
            None => "-".into(),
        }
    }

    /// Every quest objective and where it resolves now, for a debugging line.
    pub fn explain(&self, v: &View<'_>, cx: &Ctx) -> String {
        let cat = jane_data::catalog();
        let mut out = format!("blocked {:?}\n", self.blocked);
        if let Some((c, _)) = &self.dungeon {
            let _ = writeln!(out, "  crawl: {}", c.why_stuck(v));
        }
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
                        if cx.model.has_eyes_only() {
                            let want = crate::lost::landmark_words(v, q.quest, Some(i));
                            let lm = crate::lost::landmark(v, &cx.eyes, &want);
                            let lm_name = lm.and_then(|c| {
                                cx.eyes
                                    .props
                                    .iter()
                                    .find(|(k, pc)| k.0 == v.zone() && **pc == c)
                                    .and_then(|(k, _)| v.prop(k.1))
                                    .map(|p| crate::lost::prop_words(v, p).join(" "))
                            });
                            let _ = writeln!(
                                out,
                                "    lost: words {want:?}; landmark {lm:?} {lm_name:?}; signs say {:?}; looked {} frames; seen {} props, {} checked, road ends {:?}",
                                cx.eyes
                                    .leads
                                    .iter()
                                    .filter(|l| crate::lost::names(&l.words, &want))
                                    .map(|l| (l.name.as_str(), l.to, self.looking.spent.contains(&(l.post, l.nth))))
                                    .collect::<Vec<_>>(),
                                self.looking.frames.get(&(q.quest, i as u8)).copied().unwrap_or(0),
                                cx.eyes.props.len(),
                                cx.eyes.checked.len(),
                                cx.eyes.frontier.values().map(|v| v.0).collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
        out
    }

    pub fn think(&mut self, v: &View<'_>, cx: &mut Ctx, events: &[jane_sim::Event], notes: &mut Vec<Mark>) -> Act {
        if let Some(a) = talk::answer(v, cx) {
            return a;
        }
        // After an ending the game closes (STORY.md §10): the last page has been read.
        if v.the_end() != 0 {
            if !self.done {
                self.done = true;
                let how = ["", "held the shield", "put the Ball back in the hill", "took the Sunday train"];
                notes.push(Mark::Note(format!("the end: {}", how[usize::from(v.the_end().min(3))])));
            }
            return Act::idle();
        }
        // A dungeon under way: the crawl plays it, in and out again.
        if let Some((c, g)) = &mut self.dungeon {
            let g = *g;
            // Out of doors on the way to it and the night come on: home, and back in the
            // morning (the county's night kills a walk that its day would not).
            // (Julie's cellar is under the house: no road to it.)
            let outside = v.zone() == ZoneId::County && c.zone != ZoneId::Cellar;
            if outside && !(6..20).contains(&v.hour()) && has_home(v) && v.dialogue().is_none() {
                let hours = u32::from((30 - v.hour()) % 24);
                let until = v.tick().0 + hours * jane_sim::tuning::TICKS_PER_HOUR;
                let zone = c.zone;
                self.dungeon = None;
                self.task = None;
                for s in std::iter::once(g).chain(Self::steps_in(v, zone)) {
                    self.blocked.insert(s, until);
                }
                notes.push(Mark::Note(format!("the {} in the morning", zone.name())));
                return Act::idle();
            }
            // On the road to it, low with nothing to eat: the crawl waits while she mends at a
            // fire (the county's own rule, below), and starts again from where she is.
            let low = low_out_of_doors(v);
            let rest_open = self.blocked.get(&Goal::Rest).is_none_or(|&until| until <= v.tick().0);
            if low && v.zone() == ZoneId::County && c.entered.is_none() && rest_open {
                self.dungeon = None;
                self.task = None;
                return Act::idle();
            }
            // Done in there with the night come on: she waits it out inside, by what she has
            // cleared, rather than walk the county home in the dark (the road back from the
            // forest killed her again and again; the dog is not about till six anyway).
            let night = !(6..20).contains(&v.hour());
            if night && v.zone() == c.zone && c.stage == crate::crawl::Stage::Leave && v.dialogue().is_none() {
                return Act::idle();
            }
            let budget = if c.zone == ZoneId::Burial { BURIAL_FRAMES } else { CRAWL_FRAMES };
            if !c.done() && c.frames < budget {
                let a = c.think(v, cx, events, notes);
                if !c.done() {
                    return a;
                }
            }
            let (c, _) = self.dungeon.take().expect("a crawl");
            // Deaths in there were the crawl's to count.
            self.deaths = v.me().stats.deaths;
            self.task = None;
            let why = match (&c.stuck, c.done()) {
                (Some(why), _) => Some(format!("the {}: {why}", c.zone.name())),
                (None, false) => Some(format!("the {} took too long", c.zone.name())),
                (None, true) => None,
            };
            // Turned away at its door by the hours ("Open ten to four"): back when it opens, no
            // failure counted.
            let wait = if c.entered.is_none() { sense::hours_till_open(v, c.zone) } else { 0 };
            if wait > 0 && why.is_some() {
                let until = v.tick().0 + u32::from(wait) * jane_sim::tuning::TICKS_PER_HOUR;
                for s in std::iter::once(g).chain(Self::steps_in(v, c.zone)) {
                    self.blocked.insert(s, until);
                }
                notes.push(Mark::Note(format!("the {} is shut for {wait} h", c.zone.name())));
                return Act::idle();
            }
            match why {
                // Every step still open there waits with it, not only the one that sent her in.
                Some(why) => {
                    // Beaten back by deaths: the dungeon waits hours, not a moment (a death
                    // anywhere since used to clear the set-aside, and she walked straight back in
                    // to the same guard, fifty times): an hour the first time. Beaten back
                    // twice, it waits until she is stronger (her most health up by a
                    // twentieth: a jar, a page) or a day: the same fight at the same strength is
                    // the same deaths.
                    if c.stuck.as_ref().is_some_and(|s| s.starts_with("died ")) {
                        let most = jane_sim::units::max_hp(v.body()).points();
                        let e = self.died_out.entry(c.zone).or_insert((0, 0, most));
                        e.1 += 1;
                        e.2 = most;
                        let hours = if e.1 >= 2 { 24 } else { 1 };
                        e.0 = v.tick().0 + hours * jane_sim::tuning::TICKS_PER_HOUR;
                        notes.push(Mark::Note(format!("the {} again in {hours} h, or stronger", c.zone.name())));
                    }
                    self.set_aside(v, g, &why, notes);
                    for s in Self::steps_in(v, c.zone) {
                        if s != g {
                            self.set_aside(v, s, &why, &mut Vec::new());
                        }
                    }
                }
                None => notes.push(Mark::Note(format!("done with the {}", c.zone.name()))),
            }
            return Act::idle();
        }
        // Beaten out of this dungeon a while: out of it, unless shut in (then the fight is the way).
        if v.body().alive
            && dungeon(v.zone())
            && self.task.is_none()
            && self.beaten_out(v)
            && !crate::crawl::shut_in_with_boss(v)
        {
            if let Some(t) = route(v, cx, ZoneId::County) {
                self.task = Some((t, Goal::Explore(crate::crawl::Try::Travel)));
                return Act::idle();
            }
        }
        // Shut in with a dungeon's boss (a lock-in behind her, and the crawl given up or never
        // begun: an explorer's chest in the arena): nothing but the fight lets her out, so the
        // crawl, whatever was set aside. A fire she cannot walk to is no way out.
        if dungeon(v.zone()) && crate::crawl::shut_in_with_boss(v) {
            if let Some(&g) = Self::steps_in(v, v.zone()).first() {
                self.task = None;
                self.dungeon = Some((Box::new(crate::crawl::Crawl::new(v.zone())), g));
                return Act::idle();
            }
        }
        // In a dungeon a quest step names: play it whole.
        let here = v.zone();
        if dungeon(here) && self.task.is_none() {
            if let Some(g) = self.step_in(v, here) {
                self.dungeon = Some((Box::new(crawl_for(v, here, g, cx.model.has_eyes_only())), g));
                return Act::idle();
            }
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
                    self.died_for(v, g);
                }
            }
            self.task = None;
            return Act::idle();
        }
        // In and out of a door, frame after frame (seed 4's Lost at Julie's door, 29 September
        // 2026: for hours): what she is doing is set aside.
        if self.zone_was != Some(v.zone()) {
            self.zone_was = Some(v.zone());
            let now = v.frame();
            self.flips.retain(|&f| f + FLIPS.1 > now);
            self.flips.push(now);
            if self.flips.len() >= FLIPS.0 {
                self.flips.clear();
                if let Some((_, g)) = self.task.take() {
                    self.set_aside(v, g, "in and out of a door", notes);
                }
            }
        }
        // The Lost: every twenty minutes, her own map and her objective go; the log's words, the
        // journal (where she stood when she learned each thing) and what she sees again are what
        // she has.
        if cx.model.has_eyes_only() && cx.frames % crate::lost::FORGET_EVERY == 0 {
            cx.eyes = crate::lost::Eyes::default();
            cx.eyes.recall(v);
            if !dungeon(v.zone()) {
                self.task = None;
            }
            notes.push(Mark::Note("lost: forgot her own map".into()));
        }
        // Looking for a step from the words: past the budget she is told where it is.
        if let Some((_, Goal::Search(q, i))) = &self.task {
            let key = (*q, *i);
            let n = self.looking.frames.entry(key).or_insert(0);
            *n += 1;
            if *n >= crate::lost::SEARCH_BUDGET && self.looking.gave_up.insert(key) {
                notes.push(Mark::Note(format!("lost: gave up looking for {} (told where)", search_name(key))));
                self.task = None;
            }
            // On a sign's way: a newer sign read, or the thing come on screen, turns her.
            if cx.frames % 30 == 2 {
                if let Some((t, reads)) = self.looking.leading.get(&key) {
                    if *reads != cx.eyes.reads || known(v, cx, t) {
                        self.looking.leading.remove(&key);
                        self.task = None;
                    }
                }
            }
        }
        // Out of doors, what she would lose to, trading blows as the rows say, is not fought: she
        // goes on her way at a run (most of what walks the county's roads is slower than her, and
        // goes home past its leash). What she was sent after is fought, and so is anything that
        // has her cornered with no legs left in her.
        cx.run = false;
        if let Some(id) = fight::threat(v, cx) {
            if self.runs_from(v, cx, id) {
                cx.run = true;
            } else if let Some(a) = fight::engage(v, cx, id) {
                return a;
            }
        }
        if let Some(c) = fight::eat(v) {
            return Act::press(c);
        }
        // Low with nothing to eat: whatever she was doing waits for a fire.
        let low = low_out_of_doors(v);
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
                // Waiting where she was told to wait is not "reached, and it did not count".
                // (Nor is a thing made at the bench: brewing the snakes' bait is a walk to the
                // bench and four makings, and it was set aside half made as "reached, and it did
                // not count".)
                let waiting = matches!(task, Task::Wait(_) | Task::Craft { .. })
                    || matches!(goal, Goal::Search(..) | Goal::Frontier(..));
                match task.tick(v, cx) {
                    Status::Act(a) => return a,
                    Status::Done => {
                        // Sat by a fire (`jane_sim::fire`): not again at once if something gets
                        // her up; the fight, or the way on, comes first.
                        if goal == Goal::Rest && v.fires_made() {
                            self.blocked.insert(Goal::Rest, v.tick().0 + REST_AGAIN);
                        }
                        self.task = None;
                        // A step reached with the log no further on is not done with.
                        let now = log_progress(v);
                        if now != self.progress {
                            self.progress = now;
                            self.reached.clear();
                            self.fails.remove(&goal);
                        } else if matches!(goal, Goal::Step(..)) && !waiting {
                            *self.reached.entry(goal).or_insert(0) += 1;
                        } else {
                            self.fails.remove(&goal);
                        }
                        if waiting {
                            self.last = None;
                            self.again = 0;
                        } else if self.last == Some(goal) {
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
                        // No way to where she was looking: somewhere else next time.
                        if let Some((Task::Walk { to, .. }, Goal::Search(..) | Goal::Frontier(..))) = &self.task {
                            let c = to.cell();
                            self.looking.bad_road.insert(crate::lost::block_of(c));
                            self.looking.bad_fog.insert(crate::lost::fog_block_of(v, c));
                            let z = v.zone();
                            let near: Vec<_> = cx
                                .eyes
                                .props
                                .iter()
                                .filter(|((pz, _), pc)| *pz == z && (pc.0 - c.0).abs() <= 4 && (pc.1 - c.1).abs() <= 4)
                                .map(|(&k, _)| k)
                                .collect();
                            cx.eyes.checked.extend(near);
                            self.task = None;
                            continue;
                        }
                        self.task = None;
                        if let (Goal::Explore(t), Some(ex)) = (goal, self.explorer.as_mut()) {
                            ex.failed(t, &why);
                            // Out and in again has no try of its own for the crawl to count: a
                            // door she finds no way to is set aside a while, not picked again
                            // every frame (seed 2's Burial, the hatch across the hall).
                            if t == crate::crawl::Try::Travel {
                                self.set_aside(v, goal, &why, notes);
                            }
                        } else {
                            self.set_aside(v, goal, &why, notes);
                        }
                    }
                }
            }
            // Nothing to do a moment ago: look again in a little while, not every frame (a
            // choice reads every prop and person about her).
            if v.frame() < self.quiet_until {
                return Act::idle();
            }
            match self.choose(v, cx) {
                Some((Target::Fight(id), _)) => {
                    cx.fight.hunt = Some(id);
                    if let Some(a) = fight::engage(v, cx, id) {
                        return a;
                    }
                }
                Some((Target::Task(t), goal)) => {
                    let by_turns = self.reached.get(&goal).is_some_and(|&n| n >= 4) && log_progress(v) == self.progress;
                    if (self.last == Some(goal) && self.again >= 4) || by_turns {
                        self.again = 0;
                        self.reached.remove(&goal);
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
                    // Out of a dungeon into the county's night: the night is sat out by the
                    // dungeon's own fire instead (out of the Butterfly Forest at ten at night, she
                    // died on the road home a dozen times, woken each time by the forest's fire).
                    if let Some(t) = self.night_in(v, z) {
                        self.task = Some((t, Goal::Sleep));
                        self.fled_at = cx.fight.fled;
                        continue;
                    }
                    // A quest step in a dungeon: the dungeon played whole, from its door.
                    if matches!(goal, Goal::Step(..)) && dungeon(z) {
                        // Its door keeps hours ("Open ten to four"): come back when it is open.
                        let wait = sense::hours_till_open(v, z);
                        if wait > 0 {
                            let until = v.tick().0 + u32::from(wait) * jane_sim::tuning::TICKS_PER_HOUR;
                            self.blocked.insert(goal, until);
                            notes.push(Mark::Note(format!("the {} is shut for {wait} h", z.name())));
                            continue;
                        }
                        self.dungeon = Some((Box::new(crawl_for(v, z, goal, cx.model.has_eyes_only())), goal));
                        return Act::idle();
                    }
                    match route(v, cx, z) {
                        Some(t) => {
                            self.task = Some((t, goal));
                            self.fled_at = cx.fight.fled;
                        }
                        None => self.set_aside(v, goal, &format!("no door toward {}", z.name()), notes),
                    }
                }
                None => {
                    self.idle += QUIET;
                    self.quiet_until = v.frame() + QUIET;
                    if (600..600 + QUIET).contains(&self.idle) {
                        notes.push(Mark::Stuck("nothing left to do".into()));
                    }
                    if self.idle >= 600 && self.blocked.values().all(|&u| u <= v.tick().0) {
                        self.done = true;
                    }
                    return Act::idle();
                }
            }
        }
        Act::idle()
    }

    /// Is a step of a quest in the log to be played in a dungeon (an act ahead)?
    fn act_ahead(v: &View<'_>) -> bool {
        let cat = jane_data::catalog();
        v.quests().filter(|q| !q.ready).any(|q| {
            let def = cat.story.quest(q.quest);
            (0..def.requirements.len()).any(|i| {
                q.count(i) < def.requirements[i].qty
                    && zone_of_step(q.quest, i).is_some_and(|z| dungeon(z) && z != ZoneId::Cellar)
            })
        })
    }

    /// An open quest step that `z` is the place for (its text names the dungeon), not set aside.
    fn step_in(&self, v: &View<'_>, z: ZoneId) -> Option<Goal> {
        Self::steps_in(v, z).into_iter().find(|&g| self.open(v, g))
    }

    /// Every quest step not yet done that `z` is the place for.
    fn steps_in(v: &View<'_>, z: ZoneId) -> Vec<Goal> {
        let cat = jane_data::catalog();
        let mut out = Vec::new();
        for q in v.quests().filter(|q| !q.ready) {
            let def = cat.story.quest(q.quest);
            for (i, r) in def.requirements.iter().enumerate() {
                if q.count(i) < r.qty && zone_of_step(q.quest, i) == Some(z) {
                    out.push(Goal::Step(q.quest, i as u8));
                }
            }
        }
        out
    }

    fn set_aside(&mut self, v: &View<'_>, goal: Goal, why: &str, notes: &mut Vec<Mark>) {
        let n = self.fails.entry(goal).or_insert(0);
        *n += 1;
        let wait = SET_ASIDE << (*n).min(6);
        self.blocked.insert(goal, v.tick().0 + wait);
        if *n >= 2 {
            notes.push(Mark::Stuck(format!("{}: {why}", goal_name(v, goal))));
        }
    }

    /// She died on her way to `g`. What she only wanted (a provision, a fire to mend at: there
    /// are others, and later) waits hours; the log's steps a moment the first time. Killed on the
    /// way to the same thing again, it waits longer each time (an hour, three, eight): the same
    /// walk past the same camp is the same death, and waking whole used to wipe the set-aside,
    /// so she walked straight back out into it (three deaths in two minutes at the Burial's
    /// mouth). Home to bed is not let be: the night has nowhere else.
    fn died_for(&mut self, v: &View<'_>, g: Goal) {
        let now = v.tick().0;
        let base = if matches!(g, Goal::Provision(_) | Goal::Rest) { 3 } else { 0 };
        let e = self.killed_on.entry(g).or_insert((0, 0));
        // Last killed on it long ago (a day): counted afresh.
        if e.1 + 24 * jane_sim::tuning::TICKS_PER_HOUR < now {
            e.0 = 0;
        }
        e.0 += 1;
        let more = if g == Goal::Sleep { 0 } else { [0, 1, 3, 8][(e.0 as usize - 1).min(3)] };
        e.1 = now + 600 + (base + more) * jane_sim::tuning::TICKS_PER_HOUR;
        self.blocked.insert(g, e.1);
    }

    fn open(&self, v: &View<'_>, g: Goal) -> bool {
        let now = v.tick().0;
        if self.killed_on.get(&g).is_some_and(|&(_, until)| until > now) {
            return false;
        }
        let zone = match g {
            Goal::Step(q, i) => zone_of_step(q, usize::from(i)),
            Goal::Search(q, i) if i != 255 => zone_of_step(q, usize::from(i)),
            Goal::Look(z, _) => Some(z),
            Goal::Explore(_) => Some(v.zone()),
            _ => None,
        };
        let let_be = zone.and_then(|z| self.died_out.get(&z)).is_some_and(|&e| Self::let_be(v, e));
        !let_be && self.blocked.get(&g).is_none_or(|&until| until <= now)
    }

    /// In a dungeon she was beaten out of (`died_out`), with no fight on: out to the county.
    fn beaten_out(&self, v: &View<'_>) -> bool {
        self.died_out.get(&v.zone()).is_some_and(|&e| Self::let_be(v, e))
    }

    /// A dungeon beaten out of (until, times, her most health then) still let be: before `until`,
    /// unless she has grown a twentieth since.
    fn let_be(v: &View<'_>, (until, _, most): (u32, u32, i32)) -> bool {
        until > v.tick().0 && jane_sim::units::max_hp(v.body()).points() * 20 < most * 21
    }

    /// Run from `id` rather than fight it: out of doors, on her way somewhere (a task in hand), not
    /// sent after it, and losing the trade of blows with everything on her; with the energy to
    /// run, or it slower than her walk.
    /// Or, out of doors and well (three fifths of her health or more), within a couple of dozen
    /// cells of a word to say or a place to reach: the errand first, then the fight. Now that the county's things come for her on sight by day
    /// (PLAN.md §2.6), the yard skeleton met her on the step before the dog could ask her to put
    /// it down, and one she downs before she is asked stands up again only ten minutes on.
    fn runs_from(&self, v: &View<'_>, cx: &Ctx, id: UnitId) -> bool {
        self.task.is_some() && (fight::outrun(v, cx, id) || self.errand_first(v, cx, id))
    }

    fn errand_first(&self, v: &View<'_>, cx: &Ctx, id: UnitId) -> bool {
        const NEAR: i64 = 24 * CELL_FX as i64;
        if v.zone() != ZoneId::County || cx.fight.hunt == Some(id) {
            return false;
        }
        let me = v.body();
        if me.hp.points() * 5 < jane_sim::units::max_hp(me).points() * 3 {
            return false;
        }
        let to = match self.task.as_ref().map(|(t, _)| t) {
            Some(Task::Walk { to, .. }) => Some(*to),
            Some(Task::Talk { unit, .. }) => v.unit(*unit).map(|u| u.pos),
            _ => None,
        };
        to.is_some_and(|to| dist(me.pos, to) <= NEAR)
    }

    /// Night, in a dungeon, on her way out to `to` (by the county: not the house): the fire here
    /// she can walk to, rested at, or waited by once she is whole. `None`: day, or no such fire.
    fn night_in(&self, v: &View<'_>, to: ZoneId) -> Option<Task> {
        let here = v.zone();
        if (6..20).contains(&v.hour()) || !dungeon(here) || to == here || to == ZoneId::House {
            return None;
        }
        let cat = jane_data::catalog();
        let at = v.body().pos;
        let mut reach = crate::crawl::Reach::default();
        reach.update(v, 0);
        let fire = v
            .props()
            .filter(|p| !p.hidden && cat.story.prop(p.def).rest && !self.bad_fires.contains(&p.id) && reach.beside(p))
            .min_by_key(|p| (to_prop(p, at), p.id))?;
        let whole = v.body().hp >= jane_sim::units::max_hp(v.body());
        Some(if to_prop(fire, at) <= i64::from(3 * CELL_FX) && whole {
            Task::Wait(600)
        } else {
            Task::Use(UseProp::new(fire.id))
        })
    }

    /// Night out of doors, far from Julie's: the night is sat out by the nearest fire instead
    /// (rested at, then waited by), not walked home through (the county's night killed her on
    /// the way, and woke her by the same far fire to try again). `None`: home is near enough,
    /// or no fire is much nearer than it.
    fn night_by_a_fire(&self, v: &View<'_>, cx: &Ctx) -> Option<Target> {
        if v.zone() != ZoneId::County {
            return None;
        }
        let cat = jane_data::catalog();
        let at = v.body().pos;
        let home = doors_to(v, ZoneId::House).first().map_or(i64::MAX, |p| to_prop(p, at));
        if home <= i64::from(HOME_NEAR * CELL_FX) {
            return None;
        }
        let fire = v
            .props()
            .filter(|p| {
                !p.hidden
                    && cat.story.prop(p.def).rest
                    && v.prop_spawn(p).is_some_and(|s| s.talk.is_some())
                    && !self.bad_fires.contains(&p.id)
            })
            .min_by_key(|p| {
                (to_prop(p, at) + danger_on_way(&cx.nav.dangers(ZoneId::County), at, sense::prop_centre(p)), p.id)
            })?;
        let d = to_prop(fire, at);
        if d > (home / 2).min(i64::from(FIRE_NEAR * CELL_FX)) {
            return None;
        }
        let whole = v.body().hp >= jane_sim::units::max_hp(v.body());
        Some(if d <= i64::from(3 * CELL_FX) && whole {
            Target::Task(Task::Wait(600))
        } else {
            Target::Task(Task::Use(UseProp::new(fire.id)))
        })
    }

    fn choose(&mut self, v: &View<'_>, cx: &mut Ctx) -> Option<(Target, Goal)> {
        let cat = jane_data::catalog();
        // Home before dark (the first thing the county teaches): out of doors from eight in the
        // evening she goes back to Julie's and sleeps till six. In a dungeon the hour is its own.
        let night = !(6..20).contains(&v.hour());
        let home = night && matches!(v.zone(), ZoneId::County | ZoneId::House) && has_home(v);
        cx.sleep = waits_for_sunday(v, cx) || home;
        if home && self.open(v, Goal::Sleep) {
            if let Some(t) = self.night_by_a_fire(v, cx) {
                return Some((t, Goal::Sleep));
            }
            return Some((bed(v, cx), Goal::Sleep));
        }
        // A Sunday morning with Yours to Say ready and the train her way: the train before
        // anything else, or an errand the far side of the county runs her past one o'clock and the
        // week goes round again (seed 6 walked to a talker every Sunday for eighty days). A hand-in
        // set aside on another day is not set aside on this one.
        if let Some(c) = the_choice().filter(|_| cx.ending == Some(crate::Ending::Train)) {
            let ready = v.quests().any(|q| q.quest == c && q.ready);
            let g = Goal::HandIn(c);
            if ready && v.weekday() == 0 && (6..14).contains(&v.hour()) {
                if let Some(t) = train(v, cx) {
                    return Some((t, g));
                }
            }
        }
        // A bag nearly full of what she may not throw out: some of it into Julie's cupboard
        // before anything else, or nothing more goes in (seed 6 provisioned apples it had no room
        // for till the run ran out).
        let jammed = sense::stow_slot(v, sense::BAG_SPARE).is_some();
        if jammed && matches!(v.zone(), ZoneId::County | ZoneId::House) && has_home(v) && self.open(v, Goal::Stow) {
            if v.zone() == ZoneId::House {
                if let Some(p) = sense::cupboard(v) {
                    return Some((Target::Task(Task::Use(UseProp::new(p))), Goal::Stow));
                }
            } else {
                return Some((Target::Zone(ZoneId::House), Goal::Stow));
            }
        }
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
                Target::Later(_) => i64::MAX,
            }
        };
        // 0: low, with nothing to eat: a fire or a bed first.
        // Or a fire close by and a fight or two behind her: sit down while it is on the way.
        let hp = sense::hp_permille(v.body());
        // The nearest fire, by the way there: every place she died on it costs as much again
        // as the straight line (a fire past a camp is not the near one).
        let danger = cx.nav.dangers(here);
        let way = |p: &&jane_sim::Prop| (to_prop(p, at) + danger_on_way(&danger, at, sense::prop_centre(p)), p.id);
        // A fire burning (kept, or made and still lit); and, low, a cold pit she holds the
        // makings for (`jane_sim::fire`).
        let fire = v
            .props()
            .filter(|p| rest_fire(v, p).is_some_and(|u| u.hold == 0) && !self.bad_fires.contains(&p.id))
            .min_by_key(way);
        let pit = v
            .props()
            .filter(|p| rest_fire(v, p).is_some_and(|u| u.hold > 0) && !self.bad_fires.contains(&p.id))
            .min_by_key(way);
        let low = low_out_of_doors(v);
        if low && self.open(v, Goal::Rest) {
            let near = [fire, pit].into_iter().flatten().min_by_key(way);
            if let Some(u) = near.and_then(|p| rest_fire(v, p)) {
                return Some((Target::Task(Task::Use(u)), Goal::Rest));
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
        // Under the Stone ahead of her, short of bait for its small snakes and holding what
        // makes it (rat meat, and Stranglethorn or its root and water: the Burial's own cold
        // chest holds the root and the water): brewed at the bench before anything else, and
        // before the water goes into anything else. Without the makings she goes down for them.
        //
        // Brewed as soon as she holds the makings and a bench is near, not only once the Burial
        // is in the log: the dog pays the root and the water with the rats' meat (Under the
        // House), when her bag has room; by the Burial it was full of what cannot be thrown out,
        // and the first Stranglethorn had nowhere to go.
        let burial_ahead = v.quests().any(|q| !q.ready && cat.story.quest(q.quest).id == "the_burial");
        let burial_done = cat.story.quest_id("the_burial").is_some_and(|q| v.quests_done().contains(&q));
        let bait = sense::item(BAIT.0);
        let short = !burial_done && holds(v, bait) < BAIT.1;
        if short && matches!(here, ZoneId::County | ZoneId::House | ZoneId::Cellar) {
            let g = Goal::Provision(bait);
            let brew = holds(v, sense::item("potion_stranglethorn")) > 0
                || holds(v, sense::item("small_water")) > 0 && holds(v, sense::item("savage_snakeroot")) > 0;
            if brew && holds(v, sense::item("rat_meat")) > 0 && self.open(v, g) {
                if let Some(t) = get(v, cx, bait, 0) {
                    let home_near = here == ZoneId::House
                        || doors_to(v, ZoneId::House)
                            .first()
                            .is_some_and(|p| to_prop(p, at) <= i64::from(PROVISION_REACH * CELL_FX));
                    if burial_ahead || home_near || cost_of(&t) <= i64::from(PROVISION_REACH * CELL_FX) {
                        return Some((t, g));
                    }
                }
            }
        }
        // Before an act's dungeon: ready for it, as a player packs for a long walk (the potions
        // the bench makes from what she carries, food she has seen lying about). Only out of
        // doors or in the house, and only while a dungeon step is in the log. (With the bait
        // still to brew, no potion: every one of them wants the water it needs.)
        // The water the dog paid with the snakeroot (Under the House: "It wants you to keep
        // them") is kept for the bait while Under the Stone is still to come: a potion never
        // takes the last of it while a root waits for it. (It went into Stone Skin for the mine,
        // and she came to the Burial with neither bait nor the makings.)
        let owed = if burial_done {
            0
        } else {
            let brewed = holds(v, bait) + holds(v, sense::item("potion_stranglethorn"));
            holds(v, sense::item("savage_snakeroot")).min(BAIT.1.saturating_sub(brewed))
        };
        let keep_water = holds(v, sense::item("small_water")) <= owed;
        if matches!(here, ZoneId::County | ZoneId::House) && !night && hp >= COUNTY_LOW && Self::act_ahead(v) {
            for (name, want) in PROVISIONS {
                let item = sense::item(name);
                let g = Goal::Provision(item);
                if holds(v, item) >= want
                    || !self.open(v, g)
                    || (burial_ahead && short || keep_water) && name.starts_with("potion_")
                {
                    continue;
                }
                match get(v, cx, item, 0) {
                    None | Some(Target::Fight(_)) => {}
                    Some(t) => {
                        let c = cost_of(&t);
                        // Not where she has died: a chest by a camp is not worth another life.
                        let danger = cx.nav.dangers(here);
                        if target_point(v, &t)
                            .is_some_and(|p| crate::nav::near_danger(&danger, p.cell(), 2 * crate::nav::DANGER_R))
                        {
                            continue;
                        }
                        let short_of_food = name == "apple" && holds(v, item) < APPLES_SHORT;
                        let reach = if short_of_food { 2 * PROVISION_REACH } else { PROVISION_REACH };
                        if c <= i64::from(reach * CELL_FX) {
                            return Some((t, g));
                        }
                    }
                }
            }
        }
        // 1 and 2: the log. The nearest objective of any quest; one that someone is waiting
        // on (it goes back to a person, or to a place) counted at half its distance before
        // an errand for a book or a board.
        let bag_tight =
            v.me().bag[..jane_sim::tuning::BAG_SLOTS].iter().filter(|s| s.is_none()).count() < sense::BAG_SPARE;
        for q in v.quests() {
            let waited = someone_waits(q.quest);
            // Told how to end it, she goes and does it: Yours to Say before any errand.
            let is_choice = Some(q.quest) == the_choice();
            if is_choice && q.ready {
                self.choice_ready.get_or_insert(v.tick().0);
            }
            let patience = self.choice_ready.is_some_and(|t| v.tick().0.saturating_sub(t) > CHOICE_PATIENCE);
            let decided = is_choice && (cx.ending.is_some() || patience);
            let near = |c: i64| {
                if decided {
                    0
                } else if waited {
                    c / 2
                } else {
                    c
                }
            };
            let g = Goal::HandIn(q.quest);
            if q.ready {
                if self.open(v, g) {
                    match hand_in(v, cx, q.quest) {
                        // Whoever takes it back is not about: come back when they are.
                        Some(Target::Later(h)) => {
                            self.blocked.insert(g, v.tick().0 + u32::from(h) * jane_sim::tuning::TICKS_PER_HOUR);
                        }
                        // A bag with no room to spare, and this hand-in takes things out of
                        // it: first, before any dungeon (seed 2 walked into the School with a bag
                        // of keys and gold and three lost things for the lost property box, and
                        // had no slot for what the School gives).
                        Some(t) => {
                            let tight = bag_tight && takes_from_bag(q.quest);
                            if let Some((extra, t, g)) = self.as_lost(v, cx, q.quest, None, t, g) {
                                let c = if tight && extra == 0 { 0 } else { near(cost_of(&t)) + extra };
                                if Self::explorer_takes(cx, c) {
                                    offer(c, g, t, &mut best);
                                }
                            }
                        }
                        None => {}
                    }
                }
                continue;
            }
            let def = cat.story.quest(q.quest);
            // A dungeon step waits for an earlier one in another dungeon that is still to do
            // and on offer: the log's way is walked in its order (Not Relieved: the pipes, then
            // up through them into the Factory, not across the county to its wicket first).
            let mut dungeon_ahead: Option<ZoneId> = None;
            for (i, r) in def.requirements.iter().enumerate() {
                if q.count(i) >= r.qty {
                    continue;
                }
                let g = Goal::Step(q.quest, i as u8);
                if !self.open(v, g) {
                    continue;
                }
                let zone = zone_of_step(q.quest, i).filter(|&z| dungeon(z));
                if dungeon_ahead.is_some_and(|a| zone.is_some_and(|z| z != a)) {
                    continue;
                }
                if let Some((extra, t, g)) =
                    step(v, cx, q.quest, i, r.target).and_then(|t| self.as_lost(v, cx, q.quest, Some(i), t, g))
                {
                    if dungeon_ahead.is_none() {
                        dungeon_ahead = zone;
                    }
                    // A dungeon that keeps hours and is open now (the Museum, ten to four): go
                    // while it is, before any errand.
                    let open_now = match t {
                        Target::Zone(z) | Target::At(z, _) => {
                            dungeon(z)
                                && doors_to(v, z)
                                    .iter()
                                    .any(|p| sense::night_lock(v, p).is_some() && !sense::shut_at(v, p, v.hour()))
                        }
                        _ => false,
                    };
                    let c = if open_now && extra == 0 { 0 } else { near(cost_of(&t)) + extra };
                    if Self::explorer_takes(cx, c) {
                        offer(c, g, t, &mut best);
                    }
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
            if cx.model.base() == Model::Rusher && !story {
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
        let reach = i64::from(if cx.model.base() == Model::Reader { 20 } else { 12 } * CELL_FX);
        // Out of doors nothing past reach is taken, so that is asked first, and cheaply: the
        // county has thousands of props, and this runs every frame she has nothing in hand.
        let indoor = v.indoor();
        for p in v.props() {
            if !indoor && sense::plainly_past(p, at, reach) {
                continue;
            }
            let g = Goal::Look(here, p.id);
            if cx.used.contains_key(&(here, p.id)) || !self.open(v, g) {
                continue;
            }
            let d = to_prop(p, at);
            if !curious(v, cx.model.base(), p) {
                continue;
            }
            // What teaches is worth crossing the zone for; the rest only when passing.
            let teaches = sense::prop_does(v, p, &|a| matches!(a, Action::Learn(_) | Action::Grow { .. }));
            if d > reach && !(teaches && indoor) {
                continue;
            }
            // Out of doors, not what something hostile stands by (a den is looked at from the
            // road) nor where she has died; what makes her stronger is worth the fight.
            if here == ZoneId::County && !teaches {
                let c = sense::prop_centre(p);
                let guarded = sense::enemies(v).iter().any(|u| dist(u.pos, c) <= i64::from(10 * CELL_FX));
                if guarded || crate::nav::near_danger(&cx.nav.dangers(here), c.cell(), crate::nav::DANGER_R) {
                    continue;
                }
            }
            // In a dungeon, not where she fell a little while ago (`Ctx::fell_near`).
            if here != ZoneId::County && cx.fell_near(here, sense::prop_centre(p).cell(), 7, v.tick().0) {
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
            // Someone who keeps hours (the dog) is looked for where the hours put them now, not
            // where she last saw them; not about now, later.
            let t = if !cat.combat.unit(def).schedule.is_empty() {
                match keeps_hours(v, def) {
                    Some(Target::Later(_)) | None => continue,
                    Some(t) => t,
                }
            } else if z == here {
                Target::Task(Task::Walk { to: pos, near: Fx::from_px(10) })
            } else {
                Target::At(z, pos)
            };
            offer(i64::from(300 * CELL_FX), g, t, &mut best);
        }
        // The Reader sits down at a fire it passes after a fight or two; the Cautious at every
        // fire it passes hurt at all.
        let sit = match cx.model {
            Model::Reader | Model::Lost => Some((750, 30)),
            Model::Cautious => Some((950, 40)),
            Model::Rusher | Model::Explorer => None,
        };
        if let Some((below, within)) = sit.filter(|&(below, _)| hp < below && self.open(v, Goal::Rest)) {
            let _ = below;
            if let Some(p) = fire.filter(|p| to_prop(p, at) < i64::from(within * CELL_FX)) {
                offer(
                    to_prop(p, at) + i64::from(8 * CELL_FX),
                    Goal::Rest,
                    Target::Task(Task::Use(UseProp::new(p.id))),
                    &mut best,
                );
            }
        }
        // Deadwood on the way, while fires are made and she is short of it: a stop in passing.
        if self.open(v, Goal::Gather) {
            if let Some(p) = wood_near(v).and_then(|id| v.prop(id)) {
                offer(to_prop(p, at), Goal::Gather, Target::Task(Task::Use(UseProp::new(p.id))), &mut best);
            }
        }
        // The Explorer: the nearest ground not yet seen, unless something is nearer.
        if cx.model == Model::Explorer && here == ZoneId::County {
            if let Some(c) = crate::lost::fog_frontier(v, 80, &self.looking.bad_fog, &cx.nav.dangers(here)) {
                let to = Vec2::centre(c.0, c.1);
                let (bx, by) = crate::lost::fog_block_of(v, c);
                offer(
                    dist(at, to) + i64::from(FRONTIER_EXTRA * CELL_FX),
                    Goal::Frontier(bx, by),
                    Target::Task(Task::Walk { to, near: Fx(3 * CELL_FX) }),
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
            // A thing to use is reached from any side of it: the side nearest her can be the
            // far side of a wall (the Seam, at the back of the Mine's vault, was "out of reach"
            // from the side facing the corridor, and she went out and in again for ever).
            let used = match pick.as_ref().map(|p| &p.2) {
                Some(Target::Task(Task::Use(u))) => v.prop(u.prop),
                _ => None,
            };
            if let Some(p) = used {
                if !ex.reaches_prop(v, p) {
                    pick = None;
                }
            } else if let Some(at) = pick.as_ref().and_then(|p| point(&p.2)) {
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
                    let out = if here == ZoneId::Cellar { ZoneId::House } else { ZoneId::County };
                    let t = route_by(v, cx, out, |p| ex.reaches_prop(v, p));
                    self.explorer = Some(ex);
                    if let Some(t) = t {
                        return Some((Target::Task(t), Goal::Explore(crate::crawl::Try::Travel)));
                    }
                    return None;
                }
            }
            self.explorer = Some(ex);
        }
        // The Explorer with nothing here: back out to the county's edge.
        if cx.model == Model::Explorer && pick.is_none() && here != ZoneId::County {
            if let Some(t) = route(v, cx, ZoneId::County) {
                return Some((Target::Task(t), Goal::Frontier(-1, -1)));
            }
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

impl Story {
    /// The Explorer goes after a quest's thing only when it is near (quests are incidental).
    fn explorer_takes(cx: &Ctx, cost: i64) -> bool {
        cx.model != Model::Explorer || cost <= i64::from(EXPLORER_NEAR * CELL_FX)
    }

    /// The Lost's view of a target the Reader's reading of the log found (step `i` of `q`, or its
    /// hand-in): the target itself once it has been on screen (or when she has looked past the
    /// budget and been told); until then where she would look for it, from the words, as a
    /// search: a landmark they name, a road not walked, the map's edge. With the extra cost over
    /// the walk's distance. `None`: nowhere left to look.
    fn as_lost(
        &mut self,
        v: &View<'_>,
        cx: &Ctx,
        q: QuestId,
        i: Option<usize>,
        t: Target,
        g: Goal,
    ) -> Option<(i64, Target, Goal)> {
        let key = (q, i.map_or(255, |i| i as u8));
        if !cx.model.has_eyes_only() || v.zone() != ZoneId::County || self.looking.gave_up.contains(&key) {
            return Some((0, t, g));
        }
        if known(v, cx, &t) {
            if self.looking.frames.get(&key).is_some_and(|&n| n > 0) {
                self.looking.found.insert(key);
            }
            return Some((0, t, g));
        }
        let want = self.looking.words.entry(key).or_insert_with(|| crate::lost::landmark_words(v, q, i)).clone();
        let at = v.body().pos.cell();
        let dangers = cx.nav.dangers(v.zone());
        // A lead walked to its end is spent (for any step: the place is not about here).
        let arrived = i64::from(crate::lost::LEAD_ARRIVED);
        for l in &cx.eyes.leads {
            if sq(l.to, at) <= arrived * arrived {
                self.looking.spent.insert((l.post, l.nth));
            }
        }
        // What a sign said first; then a landmark the words name, the roads, the map's edge.
        let spent = |l: &crate::lost::Lead| {
            self.looking.spent.contains(&(l.post, l.nth))
                || self.looking.bad_road.contains(&crate::lost::block_of(l.to))
                || crate::nav::near_danger(&dangers, l.to, crate::lost::SHUNNED)
        };
        let led = cx.eyes.lead(&want, &spent).map(|l| l.to);
        if led.is_some() {
            self.looking.leading.insert(key, (t.clone(), cx.eyes.reads));
        } else {
            self.looking.leading.remove(&key);
        }
        let (extra, c) = led
            .map(|c| (LEAD_EXTRA, c))
            .or_else(|| {
                cx.eyes
                    .unread_post(v)
                    .and_then(|c| crate::nav::nearest_walkable(v, c.0, c.1, 4))
                    .filter(|&c| !self.looking.bad_road.contains(&crate::lost::block_of(c)))
                    .map(|c| (LEAD_EXTRA, c))
            })
            .or_else(|| {
                crate::lost::landmark(v, &cx.eyes, &want)
                    .and_then(|c| crate::nav::nearest_walkable(v, c.0, c.1, 4))
                    .map(|c| (LANDMARK_EXTRA, c))
            })
            .or_else(|| cx.eyes.road_frontier(at, &self.looking.bad_road, &dangers).map(|c| (SEARCH_EXTRA, c)))
            .or_else(|| crate::lost::fog_frontier(v, 60, &self.looking.bad_fog, &dangers).map(|c| (SEARCH_EXTRA, c)))?;
        let to = Vec2::centre(c.0, c.1);
        Some((
            i64::from(extra * CELL_FX),
            Target::Task(Task::Walk { to, near: Fx(3 * CELL_FX) }),
            Goal::Search(key.0, key.1),
        ))
    }
}

/// Has the Lost had this target on screen (or, for a door into a dungeon, seen the door)?
fn known(v: &View<'_>, cx: &Ctx, t: &Target) -> bool {
    let z = v.zone();
    match t {
        Target::Task(Task::Use(u)) => cx.eyes.props.contains_key(&(z, u.prop)),
        Target::Task(Task::Talk { unit, .. }) | Target::Fight(unit) => cx.eyes.units.contains_key(&(z, *unit)),
        Target::Task(Task::Pickup { drop, .. }) => {
            let cam = jane_sim::trace::camera(v.body().pos.cell());
            v.drops().iter().find(|d| d.id == *drop).is_some_and(|d| {
                let (x, y) = d.pos.cell();
                cam.contains(x, y)
            })
        }
        Target::Task(Task::Walk { to, .. }) => cx.eyes.looked_at(z, to.cell()),
        Target::Zone(y) | Target::At(y, _) => !dungeon(*y) || cx.eyes.doors.contains(y) || cx.visited.contains(y),
        _ => true,
    }
}

/// A search's step, in words.
pub fn search_name((q, i): (QuestId, u8)) -> String {
    let id = jane_data::catalog().story.quest(q).id;
    if i == 255 { format!("{id} hand-in") } else { format!("{id} step {}", i + 1) }
}

fn goal_name(v: &View<'_>, g: Goal) -> String {
    let cat = jane_data::catalog();
    match g {
        Goal::HandIn(q) => format!("hand in {}", cat.story.quest(q).id),
        Goal::Step(q, i) => format!("{} step {}", cat.story.quest(q).id, i + 1),
        Goal::Talk(z, u) => format!("talk to {:?} in {}", u, z.name()),
        Goal::Rest => "rest at a fire or a bed".into(),
        Goal::Gather => "gather deadwood".into(),
        Goal::Sleep => "home to sleep".into(),
        Goal::Stow => "home to the cupboard".into(),
        Goal::Provision(i) => format!("provision {}", cat.combat.item(i).id),
        Goal::Explore(t) => format!("explore: {t:?}"),
        Goal::Search(q, i) => format!("look for {}", search_name((q, i))),
        Goal::Frontier(x, y) => format!("the map's edge at {x},{y}"),
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
        // Food in a chest she passes, short of it: a rusher still eats (she walked the county's
        // roads to a fire with no apple in her bag, and died on the way, again and again).
        Model::Rusher => {
            let apple = sense::item("apple");
            let food = !p.used && s.loot.iter().any(|l| l.item == apple) && holds(v, apple) < APPLES_HELD;
            teaches
                || food
                || (v.indoor() && sense::prop_does(v, p, &|a| matches!(a, Action::Quest(_) | Action::HandIn(_))))
        }
        Model::Reader | Model::Explorer | Model::Cautious | Model::Lost => {
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
    // A question of the catalog alone, asked of every quest in the log whenever she chooses
    // (every frame she has nothing in hand): answered for every quest once, and kept.
    static WAITS: std::sync::OnceLock<Vec<bool>> = std::sync::OnceLock::new();
    let waits = WAITS
        .get_or_init(|| (0..jane_data::catalog().story.quests.len()).map(|i| waits_for(QuestId(i as u16))).collect());
    waits[q.index()]
}

fn waits_for(q: QuestId) -> bool {
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

/// Yours to Say's quest, by its row.
fn the_choice() -> Option<QuestId> {
    jane_data::catalog().story.quest_id("the_choice")
}

/// Is she waiting for a Sunday: Yours to Say ready, the train her way, and not on the day?
fn waits_for_sunday(v: &View<'_>, cx: &Ctx) -> bool {
    let ready = the_choice().is_some_and(|c| v.quests().any(|q| q.quest == c && q.ready));
    ready && cx.ending == Some(crate::Ending::Train) && !(v.weekday() == 0 && v.hour() < 13)
}

/// Does handing `q` in take things out of the bag (what it asked her to fetch)?
fn takes_from_bag(q: QuestId) -> bool {
    let cat = jane_data::catalog();
    cat.story.quest(q).requirements.iter().any(|r| matches!(r.target, jane_data::ReqTarget::Acquire(_)))
}

/// Where Yours to Say is taken in (STORY.md §10): the thing that plays the ending she has chosen
/// (any, when she has not been told), here or seen elsewhere; the train on a Sunday.
fn choice(v: &View<'_>, cx: &Ctx) -> Option<Target> {
    let here = v.zone();
    if cx.ending == Some(crate::Ending::Train) {
        return train(v, cx);
    }
    let fits = |n: u8| cx.ending.is_none_or(|e| e.the_end() == n);
    for p in v.props().filter(|p| !p.hidden) {
        let mut ends: Option<u8> = None;
        sense::visit_prop(v, p, &mut |a| ends = ends.or(sense::sets_the_end(a)));
        if ends.is_some_and(fits) {
            return Some(Target::Task(Task::Use(UseProp::new(p.id))));
        }
    }
    for (&z, notes) in &cx.notes {
        if z != here {
            if let Some(n) = notes.iter().find(|n| n.ending.is_some_and(fits)) {
                return Some(Target::At(z, n.at));
            }
        }
    }
    // Not seen yet: the place the log names.
    match cx.ending {
        Some(crate::Ending::Hold) => Some(Target::Zone(ZoneId::Cellar)),
        Some(crate::Ending::Hill) => Some(Target::Zone(ZoneId::Mine)),
        _ => None,
    }
}

/// The Sunday train: sleep the days away at a bed until a Sunday morning, then signal at the
/// name board ("Trains stop by request") and stand on the platform for one.
fn train(v: &View<'_>, cx: &Ctx) -> Option<Target> {
    let here = v.zone();
    let (day, hour) = (v.clock().1, v.hour());
    let sunday = v.weekday() == 0;
    if sunday && hour < 14 && cx.signalled == Some(day) {
        // On the platform, and wait there.
        if here != ZoneId::County {
            return Some(Target::Zone(ZoneId::County));
        }
        let r = v.rect(v.sym("platform")?)?;
        let at = v.body().pos.cell();
        if r.contains(at.0, at.1) {
            return Some(Target::Task(Task::Wait(120)));
        }
        return inside(v, v.sym("platform")?).map(|to| Target::Task(Task::Walk { to, near: Fx::from_px(2) }));
    }
    if sunday && (6..13).contains(&hour) {
        if let Some(p) = v.props().find(|p| !p.hidden && sense::prop_does(v, p, &sense::signals_train)) {
            return Some(Target::Task(Task::Use(UseProp::new(p.id))));
        }
        return cx
            .notes
            .iter()
            .find_map(|(&z, ns)| ns.iter().find(|n| n.signals).map(|n| Target::At(z, n.at)))
            .or(Some(Target::Zone(ZoneId::County)));
    }
    // Not the day: Julie's bed, and the night slept away.
    Some(bed(v, cx))
}

/// Low with nothing to eat, so that what she was doing waits for a fire. Out of doors that is
/// under three fifths: the county's roads are long and what is on them hits hard, and a walk
/// begun at half her health was a walk that did not end (the fire is free, and near).
fn low_out_of_doors(v: &View<'_>) -> bool {
    let at = v.body().pos;
    // Only with a fire near: a long walk to one is a walk through what hurt her.
    let fire_near = || v.props().any(|p| rest_fire(v, p).is_some() && to_prop(p, at) <= i64::from(FIRE_NEAR * CELL_FX));
    let line = if v.zone() == ZoneId::County && fire_near() { COUNTY_LOW } else { fight::EAT_BELOW };
    sense::hp_permille(v.body()) < line && !fight::has_food(v)
}

/// Could she light a fire now (`jane_sim::fire::plan`, the rain aside)? Two deadwood, and a
/// match, a fire stone, or Fire with the mana for it (only Fire, at an old grate). Only while
/// fires are made.
fn can_make_fire(v: &View<'_>, fire_only: bool) -> bool {
    if !v.fires_made() {
        return false;
    }
    let held = |n: &str| sense::holds(v, sense::item(n));
    if held("deadwood") < u32::from(jane_sim::tuning::FIRE_LAY) {
        return false;
    }
    let cat = jane_data::catalog();
    let fire =
        jane_sim::fire::fire_spell().is_some_and(|f| v.learned().contains(&f) && v.body().mp >= cat.combat.spell(f).mp);
    fire || (!fire_only && (held("match") > 0 || held("fire_stone") > 0))
}

/// A fire she can rest at, and how: a kept one (a `rest` row, read by its talk) or a made one
/// burning, pressed; a cold pit she holds the makings for, held a second first. `None` for
/// anything else (a pit she cannot light, a quest's brazier, ash).
fn rest_fire(v: &View<'_>, p: &jane_sim::Prop) -> Option<UseProp> {
    let d = jane_data::catalog().story.prop(p.def);
    if p.hidden {
        return None;
    }
    let spawn = v.prop_spawn(p);
    if d.rest && spawn.is_some_and(|s| s.talk.is_some()) {
        return Some(UseProp::new(p.id));
    }
    if !v.fires_made() || !d.made || spawn.is_some_and(|s| s.talk.is_some() || s.use_list.is_some()) {
        return None;
    }
    if p.on {
        return Some(UseProp::new(p.id));
    }
    can_make_fire(v, d.fire_only).then(|| UseProp::make_fire(p.id))
}

/// Deadwood to pick up on the way, while fires are made and she holds under [`WOOD_LOW`]: the
/// nearest stump, woodpile or log not yet gathered within [`WOOD_NEAR`] cells.
fn wood_near(v: &View<'_>) -> Option<PropId> {
    if !v.fires_made() || sense::holds(v, sense::item("deadwood")) >= WOOD_LOW {
        return None;
    }
    let cat = jane_data::catalog();
    let at = v.body().pos;
    v.props()
        .filter(|p| {
            !p.hidden
                && !p.used
                && cat.story.prop(p.def).wood > 0
                && v.prop_spawn(p).is_none_or(|s| s.talk.is_none())
                && to_prop(p, at) <= i64::from(WOOD_NEAR * CELL_FX)
        })
        .min_by_key(|p| (to_prop(p, at), p.id))
        .map(|p| p.id)
}

/// Gather deadwood under this many sticks (two fires' worth).
const WOOD_LOW: u32 = 4;
/// From this near, cells.
const WOOD_NEAR: i32 = 10;
/// Ticks after sitting down by a fire before she makes for one again (ten seconds).
const REST_AGAIN: u32 = 600;

/// The line under which she mends before walking on, out of doors, permille.
const COUNTY_LOW: i32 = 600;

/// What the places she died near the straight way from `a` to `b` add to its length: each one
/// within twice [`crate::nav::DANGER_R`] of the line, as much again as the line.
fn danger_on_way(spots: &[(i32, i32)], a: Vec2, b: Vec2) -> i64 {
    let len = dist(a, b);
    let r = i64::from(2 * crate::nav::DANGER_R * CELL_FX);
    let (ax, ay, bx, by) = (i64::from(a.x.0), i64::from(a.y.0), i64::from(b.x.0), i64::from(b.y.0));
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = (dx * dx + dy * dy).max(1);
    spots
        .iter()
        .filter(|&&(x, y)| {
            let p = Vec2::centre(x, y);
            let (px, py) = (i64::from(p.x.0) - ax, i64::from(p.y.0) - ay);
            // The nearest point of the segment, in thousandths along it.
            let t = ((px * dx + py * dy) * 1000 / l2).clamp(0, 1000);
            let (nx, ny) = (ax + dx * t / 1000, ay + dy * t / 1000);
            let (ex, ey) = (i64::from(p.x.0) - nx, i64::from(p.y.0) - ny);
            ex * ex + ey * ey <= r * r
        })
        .count() as i64
        * len
}

/// Where a target in this zone is, when it is a place here.
fn target_point(v: &View<'_>, t: &Target) -> Option<Vec2> {
    match t {
        Target::Task(Task::Use(u)) => v.prop(u.prop).map(sense::prop_centre),
        Target::Task(Task::Walk { to, .. }) => Some(*to),
        Target::Task(Task::Pickup { drop, .. }) => v.drops().iter().find(|d| d.id == *drop).map(|d| d.pos),
        Target::Task(Task::Talk { unit, .. }) | Target::Fight(unit) => v.unit(*unit).map(|u| u.pos),
        _ => None,
    }
}

/// Has she been let into Julie's house (the kitchen stood in)?
/// How far the quest log has come: quests done, and every count of every quest in hand.
fn log_progress(v: &View<'_>) -> u64 {
    let cat = jane_data::catalog();
    let counts: u64 = v
        .quests()
        .map(|q| (0..cat.story.quest(q.quest).requirements.len()).map(|i| u64::from(q.count(i))).sum::<u64>())
        .sum();
    (v.quests_done().len() as u64) << 32 | counts
}

fn has_home(v: &View<'_>) -> bool {
    let cat = jane_data::catalog();
    cat.story.quest_id("see_the_kitchen").is_some_and(|q| v.quests_done().contains(&q))
}

/// Julie's bed, to sleep in (a bed in the county may be behind a door she has no key to).
fn bed(v: &View<'_>, cx: &Ctx) -> Target {
    if v.zone() == ZoneId::House {
        if let Some(p) = v
            .props()
            .filter(|p| !p.hidden && sense::prop_does(v, p, &|a| matches!(a, Action::Rest { until: Some(_), .. })))
            .min_by_key(|p| (to_prop(p, v.body().pos), p.id))
        {
            return Target::Task(Task::Use(UseProp::new(p.id)));
        }
    }
    let house = cx.notes.get(&ZoneId::House).and_then(|ns| ns.iter().find(|n| n.sleeps));
    house.map_or(Target::Zone(ZoneId::House), |n| Target::At(ZoneId::House, n.at))
}

/// Where to hand `q` in.
fn hand_in(v: &View<'_>, cx: &Ctx, q: QuestId) -> Option<Target> {
    let cat = jane_data::catalog();
    let here = v.zone();
    if Some(q) == the_choice() {
        return choice(v, cx);
    }
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
    // Someone who keeps hours (the dog: the step by day, the Museum's steps from four for the
    // forest...), where their hours put them now, as the log's `returnTo` says; not about till
    // later, then later.
    for (i, u) in cat.combat.units.iter().enumerate() {
        let Some(tree) = u.talk else { continue };
        if u.schedule.is_empty() || !tree_has(v, tree, &does) {
            continue;
        }
        if let Some(t) = keeps_hours(v, UnitDefId(i as u16)) {
            return Some(t);
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

/// Where a person who keeps hours stands now (a county mark), or how long till they are about.
fn keeps_hours(v: &View<'_>, def: UnitDefId) -> Option<Target> {
    use jane_data::ScheduleSlot;
    let hour = v.hour();
    match v.slot_at_hour(def, hour)? {
        ScheduleSlot::Mark(n) => {
            if v.zone() != ZoneId::County {
                return Some(Target::Zone(ZoneId::County));
            }
            let m = v.mark(jane_sim::sym::of_name(n))?;
            let at = Vec2::centre(i32::from(m.cell.x), i32::from(m.cell.y));
            Some(Target::Task(Task::Walk { to: at, near: Fx::from_px(10) }))
        }
        ScheduleSlot::Absent => (1..24u8)
            .find(|h| matches!(v.slot_at_hour(def, (hour + h) % 24), Some(ScheduleSlot::Mark(_))))
            .map(Target::Later),
        ScheduleSlot::Patrol | ScheduleSlot::Inside(_) => None,
    }
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
                // None seen yet: the one the log names ("made at the bench in Julie's kitchen").
                let kitchen = cat.story.quests.iter().flat_map(|q| q.requirements.iter()).any(|r| {
                    let t = cat.text(r.text).to_lowercase();
                    t.contains("bench") && t.contains("kitchen")
                });
                if kitchen && here != ZoneId::House {
                    return Some(Target::Zone(ZoneId::House));
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
    // Where the log said it is to be had ("Rat meat, from the rats in Julie's cellar"), done
    // or not: there.
    let named = cat.story.quests.iter().flat_map(|q| q.requirements.iter()).find_map(|r| {
        (r.target == jane_data::ReqTarget::Acquire(item)).then(|| zone_in_text(cat.text(r.text))).flatten()
    });
    if let Some(z) = named.filter(|&z| z != here) {
        return Some(Target::Zone(z));
    }
    None
}

/// The door to take toward zone `z`: one straight there, else back toward the county (the house
/// for the cellar).
pub fn route(v: &View<'_>, cx: &Ctx, z: ZoneId) -> Option<Task> {
    route_by(v, cx, z, |_| true)
}

/// [`route`], by a door `reach` says she can get to when there is one (the crawl's flood: out
/// of a dungeon by the way she can walk, not a far hatch she has no way to, tried every frame
/// for ever on seed 2's Burial).
pub fn route_by(v: &View<'_>, cx: &Ctx, z: ZoneId, mut reach: impl FnMut(&jane_sim::Prop) -> bool) -> Option<Task> {
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
    // Every door here into it shut to her (the School's front doors, bolted from inside): the
    // other way in, through a place she has seen a door from into it, or one the log names.
    if !doors.is_empty() && !doors.iter().any(|p| crate::sense::can_open(v, p)) {
        if let Some(y) = way_round(v, cx, z).filter(|&y| y != here && !doors_to(v, y).is_empty()) {
            next = y;
            doors = doors_to(v, y);
        }
    }
    if doors.is_empty() {
        next = if here == ZoneId::County { via(z) } else { via(here) };
        doors = doors_to(v, next);
    }
    if doors.is_empty() && here != ZoneId::County {
        doors = doors_to(v, ZoneId::County);
    }
    let _ = next;
    // Of several doors there, one she can open (the mine's mouth, not the adit barred from
    // inside), then one not yet taken (the other hatch).
    doors.sort_by_key(|p| {
        (!crate::sense::can_open(v, p), cx.used.contains_key(&(here, p.id)), to_prop(p, v.body().pos), p.id)
    });
    let d = doors.iter().find(|p| reach(p)).or(doors.first())?;
    Some(Task::Use(UseProp { presses: if d.locked { 2 } else { 1 }, ..UseProp::new(d.id) }))
}

/// Another way into `z`: a zone she has seen a door from into it, else a dungeon the log's
/// words about `z` name ("The stair out of the Burial Chamber comes up in its boiler room").
fn way_round(v: &View<'_>, cx: &Ctx, z: ZoneId) -> Option<ZoneId> {
    let cat = jane_data::catalog();
    let here = v.zone();
    if let Some((&y, _)) =
        cx.notes.iter().find(|(y, ns)| **y != z && **y != here && ns.iter().any(|n| n.door == Some(z)))
    {
        return Some(y);
    }
    v.quests().find_map(|q| {
        let text = cat.text(cat.story.quest(q.quest).description);
        let named = zones_in_text(text);
        named.contains(&z).then(|| named.into_iter().find(|&y| y != z && y != here)).flatten()
    })
}

/// Every dungeon a text names where the thing is (see [`zone_in_text`]), in the text's order.
fn zones_in_text(text: &str) -> Vec<ZoneId> {
    let t = text.to_lowercase();
    let mut out: Vec<(usize, ZoneId)> = [
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
    .filter_map(|(w, z)| t.find(w).map(|i| (i, z)))
    .collect();
    out.sort();
    out.into_iter().map(|(_, z)| z).collect()
}
