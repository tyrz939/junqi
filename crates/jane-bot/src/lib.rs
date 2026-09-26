//! The headless player over jane-sim (ARCHITECTURE.md §1, §8; PORT.md §4; VERIFICATION.md §2
//! L3; `jane/test/bot.ts`).
//!
//! Float-free: no floating-point type anywhere in this crate (PORT.md §3.4).
//!
//! A [`Bot`] is a policy over [`View`]: each frame it reads its seat's view and the events of the
//! last step and answers with what a person could do that frame, a held [`InputFrame`] and the
//! [`Command`]s pressed ([`Act`]). It never reads `GameState`; what it needed and the view did
//! not have was added to `View`, derived and read-only (ARCHITECTURE.md §11). Content (quest
//! steps, conversations, item and unit rows) it reads from the catalog, as the text on screen
//! would tell a player; only [`crawl::setup`], the console's part of a test, reads a blueprint.
//! It is deterministic: the same seed, model and build make the same frames, so every run is a
//! tape ([`jane_sim::replay`]), and a run saved, loaded and played on is the same run.
//!
//! What it may know is P4's, not yet P4b's (VERIFICATION.md §2 L3 "a model may not be better
//! informed"): it sees the whole zone the view shows (every prop, every awake unit), not only
//! the fog it has cleared, and resolves a step by the catalog's name for it.
//!
//! | Module | What |
//! | --- | --- |
//! | [`nav`] | a path over the view's ground, and the stick that walks it |
//! | [`sense`] | questions asked of a view: props and units by name, doors, bags, enemies |
//! | [`task`] | one thing being done: walk, use a prop, talk, go through a door, pick up, craft, cast |
//! | [`fight`] | the bar: melee, bolts, an apple when low, backing off |
//! | [`talk`] | a conversation: which line to take |
//! | [`story`] | the quest log read as objectives (the Reader and the Rusher) |
//! | [`console`] | a story test's setup: a new game put at the start of an act |
//! | [`crawl`] | a dungeon: keys, locks, plates, verbs, the boss, the way out |
//! | [`tactics`] | what each dungeon and boss asks beyond the crawl's general order |
//! | [`fixture`] | the bot-session hash fixture (`bot-hash-<target>.txt`) |
//!
//! Two player models (VERIFICATION.md §2 L3; P4b adds the rest): the **Reader** takes every
//! quest, reads every sign and note it passes, keeps to the roads and walks what the text names;
//! the **Rusher** keeps to the spine (what gives, hands in or teaches), goes straight across and
//! sprints whenever it can (until something chases it off the fields; then it keeps to the road).

#![deny(clippy::float_arithmetic, clippy::float_cmp)]

pub mod coarse;
pub mod console;
pub mod crawl;
pub mod fight;
pub mod fixture;
pub mod nav;
pub mod sense;
pub mod story;
pub mod tactics;
pub mod talk;
pub mod task;

use std::fmt::Write as _;

use jane_core::{QuestId, SpellId, ZoneId};
use jane_sim::event::{EventKind, QuestChange};
use jane_sim::replay::Recorder;
use jane_sim::{Command, Event, InputFrame, Seat, Sim, StampedCommand, StepInput, Stepped, View};

/// Which player (VERIFICATION.md §2 L3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Model {
    /// Follows quest text and signs literally; reads everything it passes.
    Reader,
    /// The spine only, in straight lines, sprinting on cooldown.
    Rusher,
}

impl Model {
    pub fn parse(s: &str) -> Option<Model> {
        match s {
            "reader" => Some(Model::Reader),
            "rusher" => Some(Model::Rusher),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Model::Reader => "reader",
            Model::Rusher => "rusher",
        }
    }

    /// Runs whenever there is energy to (the Reader walks unless it is going far).
    pub const fn sprints(self) -> bool {
        matches!(self, Model::Rusher)
    }
}

/// Which of the three endings a bot chooses at Yours to Say (STORY.md §10): the choice policy
/// that makes each reachable in a test. Without one it carries the Ball to the nearest of the
/// three.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ending {
    /// The ring on the study desk under Julie's house (`the_end` 1).
    Hold,
    /// The seam at the back of the Gold Mine's vault (`the_end` 2).
    Hill,
    /// The Sunday train at Castle Halt (`the_end` 3).
    Train,
}

impl Ending {
    pub fn parse(s: &str) -> Option<Ending> {
        match s {
            "hold" | "a" | "1" => Some(Ending::Hold),
            "hill" | "b" | "2" => Some(Ending::Hill),
            "train" | "c" | "3" => Some(Ending::Train),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Ending::Hold => "hold",
            Ending::Hill => "hill",
            Ending::Train => "train",
        }
    }

    /// The world's `the_end` for it.
    pub const fn the_end(self) -> u8 {
        match self {
            Ending::Hold => 1,
            Ending::Hill => 2,
            Ending::Train => 3,
        }
    }
}

/// One frame of play: the stick and what was pressed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Act {
    pub frame: InputFrame,
    pub cmds: Vec<Command>,
}

impl Act {
    pub fn idle() -> Act {
        Act { frame: InputFrame::IDLE, cmds: Vec::new() }
    }

    pub fn hold(frame: InputFrame) -> Act {
        Act { frame, cmds: Vec::new() }
    }

    pub fn press(c: Command) -> Act {
        Act { frame: InputFrame::IDLE, cmds: vec![c] }
    }
}

/// Something worth a line in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    Zone(ZoneId),
    QuestGiven(QuestId),
    QuestReady(QuestId),
    QuestDone(QuestId),
    Learned(SpellId),
    Died,
    Killed(jane_core::UnitDefId),
    /// A plan's own milestone ("boss down", "left the mine").
    Note(String),
    /// What the plan gave up on, and why.
    Stuck(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub tick: u32,
    pub frame: u32,
    pub zone: ZoneId,
    pub mark: Mark,
}

impl Milestone {
    /// One line: `mm:ss tick zone what`.
    pub fn line(&self) -> String {
        let cat = jane_data::catalog();
        let secs = self.tick / 60;
        let what = match &self.mark {
            Mark::Zone(z) => format!("enter {}", z.name()),
            Mark::QuestGiven(q) => format!("quest given: {}", cat.story.quest(*q).id),
            Mark::QuestReady(q) => format!("quest ready: {}", cat.story.quest(*q).id),
            Mark::QuestDone(q) => format!("quest done: {}", cat.story.quest(*q).id),
            Mark::Learned(s) => format!("learned {}", cat.combat.spell(*s).id),
            Mark::Died => "died".to_owned(),
            Mark::Killed(d) => format!("killed {}", cat.combat.unit(*d).id),
            Mark::Note(s) => s.clone(),
            Mark::Stuck(s) => format!("STUCK: {s}"),
        };
        format!("{:02}:{:02} {:>6} {:<8} {what}", secs / 60, secs % 60, self.tick, self.zone.name())
    }
}

/// The ground about a rect as text, for a debugging dump: `#` a cell feet cannot cross, `.` one
/// they can, a prop's footprint by the first letter of its key (upper case when solid), `@` her.
pub fn ascii(v: &View<'_>, r: jane_core::Rect, pad: i32) -> String {
    let me = v.body().pos.cell();
    let mut out = String::new();
    for y in r.y - pad..r.bottom() + pad {
        for x in r.x - pad..r.right() + pad {
            let c = if (x, y) == me {
                '@'
            } else if let Some(p) = v.props().find(|p| !p.hidden && sense::prop_rect(p).contains(x, y)) {
                let ch = v.name(p.key).chars().next().unwrap_or('?');
                if p.solid { ch.to_ascii_uppercase() } else { ch.to_ascii_lowercase() }
            } else if nav::walkable(v, x, y) {
                '.'
            } else {
                '#'
            };
            out.push(c);
        }
        out.push('\n');
    }
    out
}

/// What a bot plays on: a sim, or a sim being recorded.
pub trait Host {
    fn view(&self, seat: Seat) -> Option<View<'_>>;
    fn step(&mut self, input: &StepInput<'_>) -> Stepped;
    fn drain_events(&mut self) -> &[Event];
    fn sim(&self) -> &Sim;
}

impl Host for Sim {
    fn view(&self, seat: Seat) -> Option<View<'_>> {
        Sim::view(self, seat)
    }

    fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        Sim::step(self, input)
    }

    fn drain_events(&mut self) -> &[Event] {
        Sim::drain_events(self)
    }

    fn sim(&self) -> &Sim {
        self
    }
}

impl Host for Recorder {
    fn view(&self, seat: Seat) -> Option<View<'_>> {
        Recorder::view(self, seat)
    }

    fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        Recorder::step(self, input)
    }

    fn drain_events(&mut self) -> &[Event] {
        Recorder::drain_events(self)
    }

    fn sim(&self) -> &Sim {
        Recorder::sim(self)
    }
}

/// The plan a bot follows.
#[derive(Debug)]
pub enum Plan {
    /// The quest log (the Reader's and the Rusher's first hour).
    Story(story::Story),
    /// A dungeon, from its door to its boss and back out.
    Crawl(crawl::Crawl),
}

/// A headless player on one seat.
#[derive(Debug)]
pub struct Bot {
    pub seat: Seat,
    pub model: Model,
    pub plan: Plan,
    pub ctx: task::Ctx,
    /// Commands pressed so far (the seq of the next).
    seq: u16,
    events: Vec<Event>,
    pub log: Vec<Milestone>,
    /// Console commands to send before playing (the tests' setup), one a frame.
    pub setup: Vec<Command>,
}

impl Bot {
    pub fn new(model: Model, plan: Plan) -> Bot {
        Bot {
            seat: Seat(0),
            model,
            plan,
            ctx: task::Ctx::new(model),
            seq: 0,
            events: Vec::new(),
            log: Vec::new(),
            setup: Vec::new(),
        }
    }

    /// The story from New Game.
    pub fn story(model: Model) -> Bot {
        Bot::new(model, Plan::Story(story::Story::new()))
    }

    /// Has the plan finished (or given up)?
    pub fn done(&self) -> bool {
        match &self.plan {
            Plan::Story(s) => s.done(),
            Plan::Crawl(c) => c.done(),
        }
    }

    /// The last thing given up on, if the plan stopped there.
    pub fn stuck(&self) -> Option<&str> {
        self.log.iter().rev().find_map(|m| match &m.mark {
            Mark::Stuck(s) => Some(s.as_str()),
            _ => None,
        })
    }

    fn note_events(&mut self, v: &View<'_>) {
        let me = v.me().unit;
        let (tick, frame, zone) = (v.tick().0, v.frame(), v.zone());
        let mut marks = Vec::new();
        for e in &self.events {
            if e.to.is_some_and(|s| s != self.seat) {
                continue;
            }
            match e.kind {
                EventKind::Zone { zone, .. } => marks.push(Mark::Zone(zone)),
                EventKind::Quest { quest, change: QuestChange::Given } => marks.push(Mark::QuestGiven(quest)),
                EventKind::Quest { quest, change: QuestChange::Ready } => marks.push(Mark::QuestReady(quest)),
                EventKind::Quest { quest, change: QuestChange::Done } => marks.push(Mark::QuestDone(quest)),
                EventKind::Learn(s) => marks.push(Mark::Learned(s)),
                EventKind::PlayerDied => marks.push(Mark::Died),
                EventKind::Death { unit, def, .. } if unit != me && self.ctx.fighting(unit) => {
                    marks.push(Mark::Killed(def));
                }
                _ => {}
            }
        }
        for m in marks {
            self.log.push(Milestone { tick, frame, zone, mark: m });
        }
    }

    /// What it holds, what it is doing and what stands in its way, for a debugging dump.
    pub fn explain(&self, v: &View<'_>) -> String {
        let cat = jane_data::catalog();
        let bag: Vec<String> =
            v.me().bag.iter().flatten().map(|s| format!("{}x{}", s.qty, cat.combat.item(s.item).id)).collect();
        let mut out = format!(
            "at {:?} in the {}, clock {:?}; hp {}; bag: {}\n",
            v.body().pos.cell(),
            v.zone().name(),
            v.clock(),
            v.body().hp.points(),
            bag.join(" ")
        );
        let (x, y) = v.body().pos.cell();
        out.push_str(&ascii(v, jane_core::Rect::new(x, y, 1, 1), 8));
        match &self.plan {
            Plan::Story(s) => {
                let _ = writeln!(out, "doing: {}", s.status());
                out.push_str(&s.explain(v, &self.ctx));
            }
            Plan::Crawl(c) => {
                let _ = write!(
                    out,
                    "doing: {}\n{}\ntried: {:?}\nfailed: {:?}\n",
                    c.status(),
                    c.why_stuck(v),
                    c.tried_list(),
                    c.failures
                );
            }
        }
        out
    }

    /// Log a plan's milestone.
    pub fn note(&mut self, v: &View<'_>, mark: Mark) {
        self.log.push(Milestone { tick: v.tick().0, frame: v.frame(), zone: v.zone(), mark });
    }

    /// This frame's act.
    pub fn think(&mut self, v: &View<'_>) -> Act {
        self.note_events(v);
        self.ctx.observe(v, &self.events);
        if let Some(c) = self.setup.first().copied() {
            self.setup.remove(0);
            return Act::press(c);
        }
        let mut notes = Vec::new();
        let act = match &mut self.plan {
            Plan::Story(s) => s.think(v, &mut self.ctx, &self.events, &mut notes),
            Plan::Crawl(c) => c.think(v, &mut self.ctx, &self.events, &mut notes),
        };
        for m in notes {
            self.note(v, m);
        }
        act
    }

    /// Play one frame on `host`.
    pub fn step<H: Host>(&mut self, host: &mut H) -> Stepped {
        let act = match host.view(self.seat) {
            Some(v) => self.think(&v),
            None => Act::idle(),
        };
        let mut frames = [InputFrame::IDLE; 4];
        frames[self.seat.index()] = act.frame;
        let cmds: Vec<StampedCommand> = act
            .cmds
            .into_iter()
            .map(|cmd| {
                self.seq = self.seq.wrapping_add(1);
                StampedCommand { seat: Some(self.seat), seq: self.seq, cmd }
            })
            .collect();
        let out = host.step(&StepInput { frames, commands: &cmds });
        self.events.clear();
        self.events.extend_from_slice(host.drain_events());
        out
    }

    /// Play until the plan is done or `frames` have passed. Returns the frames played.
    pub fn play<H: Host>(&mut self, host: &mut H, frames: u32) -> u32 {
        for n in 0..frames {
            if self.done() {
                return n;
            }
            self.step(host);
        }
        frames
    }
}
