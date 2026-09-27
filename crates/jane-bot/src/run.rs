//! A model's session, traced (VERIFICATION.md §3.1): the bot plays, the sim's
//! [`Observer`](jane_sim::trace::Observer) writes what it saw after every step, and this adds
//! what only the player's side knows: each change of objective (a [`Kind::Decision`], with the
//! words it acts on), each quest step's target first on screen (a [`Kind::Sight`]: the harness
//! knows where the target is, the model does not), and the bot's log lines.
//!
//! Nothing here changes what the bot presses: a traced session steps and hashes as an untraced
//! one (`tests/trace.rs`).

use std::collections::BTreeSet;

use jane_core::action::Action;
use jane_core::{QuestId, Rect};
use jane_data::ReqTarget;
use jane_sim::trace::{Header, Kind, Observer, SAMPLE_EVERY, TRACE_VERSION, Trace, camera};
use jane_sim::{Sim, View};

use crate::story::Goal;
use crate::{Bot, Host, Plan};

/// A bot and the trace of its play.
#[derive(Debug)]
pub struct Session {
    pub bot: Bot,
    pub trace: Trace,
    obs: Observer,
    /// The objective last recorded (`started`: one has been).
    last: Option<Goal>,
    started: bool,
    /// (quest, step; 255 the hand-in) whose target has been on screen.
    sighted: BTreeSet<(QuestId, u8)>,
    shown: usize,
}

/// A trace's header for a session starting on `sim`.
pub fn header(sim: &Sim, model: &str, seats: u8, minutes: u32) -> Header {
    let st = sim.state();
    Header {
        version: TRACE_VERSION,
        content_hash: jane_data::catalog().content_hash,
        build: env!("CARGO_PKG_VERSION").to_owned(),
        seed: st.seed,
        model: model.to_owned(),
        seats,
        minutes,
        started_clock: (st.clock, st.day),
    }
}

impl Session {
    pub fn new(bot: Bot, sim: &Sim, minutes: u32) -> Session {
        let name = bot.model.name();
        Session::named(bot, header(sim, name, 1, minutes))
    }

    /// With a header of the caller's (a pair's seat, a model's variant).
    pub fn named(bot: Bot, header: Header) -> Session {
        Session {
            bot,
            trace: Trace::new(header),
            obs: Observer::new(),
            last: None,
            started: false,
            sighted: BTreeSet::new(),
            shown: 0,
        }
    }

    /// Frames played.
    pub fn frames(&self) -> u32 {
        self.obs.frames()
    }

    /// Play one frame on `host`, and trace it.
    pub fn step<H: Host>(&mut self, host: &mut H) {
        self.bot.step(host);
        self.after(host.sim(), None);
    }

    /// Trace a frame someone else stepped (a lockstep peer): `events` are the step's, all of
    /// them; `observe` is false for a seat whose sim another session already observes.
    pub fn after(&mut self, sim: &Sim, events: Option<&[jane_sim::Event]>) {
        let events = events.unwrap_or(self.bot.events());
        self.obs.observe(sim, events, &mut self.trace);
        self.record(sim);
    }

    /// Only the bot's side of a frame (a second seat on a sim whose observer is the first
    /// seat's session): decisions, sights and log lines.
    pub fn record(&mut self, sim: &Sim) {
        let frame = self.obs.frames().max(1);
        self.record_at(sim, frame);
    }

    /// [`record`](Self::record) at a frame the caller counts (a lockstep peer's).
    pub fn record_at(&mut self, sim: &Sim, frame: u32) {
        let tick = sim.state().tick.0;
        let seat = Some(self.bot.seat);
        while self.shown < self.bot.log.len() {
            let line = self.bot.log[self.shown].line();
            self.trace.push(tick, frame, seat, Kind::Note(line));
            self.shown += 1;
        }
        let Some(v) = sim.view(self.bot.seat) else { return };
        let goal = match &self.bot.plan {
            Plan::Story(s) => s.objective(),
            Plan::Crawl(_) => None,
        };
        if !self.started || self.last != goal {
            self.started = true;
            self.last = goal;
            self.trace.push(tick, frame, seat, decision(&v, goal));
        }
        if tick % SAMPLE_EVERY == 0 {
            for (q, i, cell) in sights(&v, &self.sighted) {
                self.sighted.insert((q, i));
                let me = v.body().pos.cell();
                let d = (cell.0 - me.0).unsigned_abs().max((cell.1 - me.1).unsigned_abs());
                self.trace.push(
                    tick,
                    frame,
                    seat,
                    Kind::Sight { quest: q.0, step: i, zone: v.zone().index() as u8, cell, dist: d },
                );
            }
        }
    }

    /// Close the trace.
    pub fn finish(mut self, sim: &Sim) -> (Bot, Trace) {
        let frames = self.obs.frames();
        self.trace.finish(sim.state().tick.0, frames);
        (self.bot, self.trace)
    }
}

/// A decision record for an objective.
fn decision(v: &View<'_>, g: Option<Goal>) -> Kind {
    let cat = jane_data::catalog();
    let text = |q: QuestId, i: Option<usize>| crate::lost::step_text(v, q, i);
    let (objective, quest, step, because) = match g {
        None => ("none".to_owned(), None, None, String::new()),
        Some(Goal::HandIn(q)) => (format!("hand in {}", cat.story.quest(q).id), Some(q.0), None, text(q, None)),
        Some(Goal::Step(q, i)) => {
            (format!("{} step {}", cat.story.quest(q).id, i + 1), Some(q.0), Some(i), text(q, Some(usize::from(i))))
        }
        Some(Goal::Search(q, i)) => {
            let step = (i != 255).then_some(usize::from(i));
            (format!("search {}", crate::story::search_name((q, i))), Some(q.0), step.map(|s| s as u8), text(q, step))
        }
        Some(Goal::Frontier(..)) => ("frontier".to_owned(), None, None, String::new()),
        Some(Goal::Rest) => ("rest".to_owned(), None, None, String::new()),
        Some(Goal::Sleep) => ("sleep".to_owned(), None, None, String::new()),
        Some(Goal::Talk(..)) => ("talk".to_owned(), None, None, String::new()),
        Some(Goal::Look(..)) => ("look".to_owned(), None, None, String::new()),
        Some(Goal::Provision(i)) => (format!("provision {}", cat.combat.item(i).id), None, None, String::new()),
        Some(Goal::Explore(t)) => (format!("dungeon {t:?}"), None, None, String::new()),
    };
    Kind::Decision { objective, quest, step, because }
}

/// Every open step (and every ready quest's hand-in) whose target stands on screen now and was
/// not before: (quest, step or 255, a cell of it).
pub fn sights(v: &View<'_>, already: &BTreeSet<(QuestId, u8)>) -> Vec<(QuestId, u8, (i32, i32))> {
    let cat = jane_data::catalog();
    let cam = camera(v.body().pos.cell());
    let mut want: Vec<(QuestId, u8, Want)> = Vec::new();
    for q in v.quests() {
        let def = cat.story.quest(q.quest);
        if q.ready {
            if !already.contains(&(q.quest, 255)) {
                want.push((q.quest, 255, Want::HandIn(q.quest)));
            }
            continue;
        }
        for (i, r) in def.requirements.iter().enumerate() {
            if q.count(i) < r.qty && !already.contains(&(q.quest, i as u8)) {
                want.push((q.quest, i as u8, Want::Req(r.target)));
            }
        }
    }
    if want.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(QuestId, u8, (i32, i32))> = Vec::new();
    let hit = |q: QuestId, i: u8, c: (i32, i32), out: &mut Vec<(QuestId, u8, (i32, i32))>| {
        if !out.iter().any(|&(oq, oi, _)| (oq, oi) == (q, i)) {
            out.push((q, i, c));
        }
    };
    let props: Vec<&jane_sim::Prop> = v.props_in(cam).collect();
    for &(q, i, w) in &want {
        match w {
            Want::Req(ReqTarget::Kill(def)) => {
                if let Some(u) = v.units_in(cam).find(|u| u.unit.def == def && u.unit.alive) {
                    hit(q, i, u.unit.pos.cell(), &mut out);
                }
            }
            Want::Req(ReqTarget::Acquire(item)) => {
                if let Some(d) = v.drops().iter().find(|d| {
                    d.item == item && {
                        let (x, y) = d.pos.cell();
                        cam.contains(x, y)
                    }
                }) {
                    hit(q, i, d.pos.cell(), &mut out);
                }
                if let Some(p) = props
                    .iter()
                    .find(|p| !p.used && v.prop_spawn(p).is_some_and(|s| s.loot.iter().any(|l| l.item == item)))
                {
                    hit(q, i, crate::sense::prop_centre(p).cell(), &mut out);
                }
            }
            Want::Req(ReqTarget::Location(name)) => {
                let does = |a: &Action| matches!(a, Action::Location(jane_core::Key::Name(n)) if *n == name);
                if let Some(r) = trigger_rect(v, cam, &does) {
                    hit(q, i, (r.x + r.w / 2, r.y + r.h / 2), &mut out);
                }
                if let Some(p) = props.iter().find(|p| crate::sense::prop_does(v, p, &does)) {
                    hit(q, i, crate::sense::prop_centre(p).cell(), &mut out);
                }
            }
            Want::HandIn(quest) => {
                let does = |a: &Action| matches!(a, Action::HandIn(x) if *x == quest);
                if let Some(r) = trigger_rect(v, cam, &does) {
                    hit(q, i, (r.x + r.w / 2, r.y + r.h / 2), &mut out);
                }
                if let Some(p) = props.iter().find(|p| crate::sense::prop_does(v, p, &does)) {
                    hit(q, i, crate::sense::prop_centre(p).cell(), &mut out);
                }
                if let Some(u) = v
                    .units_in(cam)
                    .find(|u| cat.combat.unit(u.unit.def).talk.is_some_and(|t| crate::sense::tree_has(v, t, &does)))
                {
                    hit(q, i, u.unit.pos.cell(), &mut out);
                }
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Want {
    Req(ReqTarget),
    HandIn(QuestId),
}

/// A trigger rect of this zone overlapping `cam` that does `pred` (and can still fire).
fn trigger_rect(v: &View<'_>, cam: Rect, pred: &impl Fn(&Action) -> bool) -> Option<Rect> {
    v.triggers().find_map(|(t, fired)| {
        if fired && t.trigger.once {
            return None;
        }
        let r = v.rect(t.rect)?;
        (r.overlaps(cam) && crate::sense::list_has(v, t.trigger.actions, pred)).then_some(r)
    })
}
