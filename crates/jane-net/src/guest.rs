//! A guest (ARCHITECTURE.md §7): says hello, builds the county the host names, is welcomed with
//! the world as it is at a step boundary, and from then steps every bundle the host sends,
//! sending its own seat's input `delay` frames ahead of the frame it steps.
//!
//! What it sends is idempotent: every input the host has not bundled yet goes again with each
//! [`Msg::Inputs`], with the next bundle it lacks, so the host can resend what was lost.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::thread::JoinHandle;

use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame};
use jane_sim::{Blueprints, ClientToken, Seat, Sim, Stepped};

use crate::book::{Book, step};
use crate::host::StallView;
use crate::link::Link;
use crate::wire::{self, Bundle, ByeWhy, HASH_EVERY, Hello, Item, Msg, Refusal, Report};

const HELLO_EVERY_MS: u64 = 500;
const READY_EVERY_MS: u64 = 1_000;
/// Input goes at least this often, as a heartbeat, even when nothing is new.
const INPUTS_EVERY_MS: u64 = 50;
/// The host is gone after this long without a word.
const HOST_GONE_MS: u64 = 10_000;

#[derive(Clone, Debug)]
pub struct GuestConfig {
    /// What is said at the door. [`Hello::ours`] unless a test says otherwise.
    pub hello: Hello,
    pub desync_dir: Option<PathBuf>,
}

impl GuestConfig {
    pub fn new(token: ClientToken) -> GuestConfig {
        GuestConfig { hello: Hello::ours(token), desync_dir: None }
    }
}

/// Where a guest is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Knocking.
    Hello,
    /// Building the county of this seed.
    Building {
        seed: u32,
    },
    /// Built; waiting for the world.
    Ready,
    /// At the table.
    Playing,
    Refused(Refusal),
    /// Over, and why.
    Ended(String),
}

#[derive(Debug)]
pub struct Guest {
    link: Box<dyn Link>,
    cfg: GuestConfig,
    phase: Phase,
    bps: Option<Blueprints>,
    building: Option<JoinHandle<Result<Blueprints, String>>>,
    sim: Option<Sim>,
    seat: Option<Seat>,
    epoch: u32,
    delay: u8,
    /// Bundles come and not yet stepped.
    bundles: BTreeMap<u32, Bundle>,
    /// Inputs sent and not yet bundled.
    outbox: BTreeMap<u32, Item>,
    next_in: u32,
    book: Book,
    events: Vec<Event>,
    hashes: Vec<(u32, u64)>,
    heard: u64,
    said: u64,
    fresh: bool,
    stall: Option<StallView>,
    reports: Vec<Report>,
    /// A fault for tests: the bundle of this frame is stepped with seat 0's stick turned, as a
    /// broken build would (a reproducible desync).
    #[doc(hidden)]
    pub corrupt_at: Option<u32>,
}

impl Guest {
    /// Knock on `link`. `bps`: blueprints already built (a test's, a cache's), used if they are
    /// the seed the host names; otherwise the county is built on a thread.
    pub fn new(link: Box<dyn Link>, cfg: GuestConfig, bps: Option<Blueprints>, now: u64) -> Guest {
        let mut g = Guest {
            link,
            cfg,
            phase: Phase::Hello,
            bps,
            building: None,
            sim: None,
            seat: None,
            epoch: 0,
            delay: wire::DEFAULT_DELAY,
            bundles: BTreeMap::new(),
            outbox: BTreeMap::new(),
            next_in: 0,
            book: Book::default(),
            events: Vec::new(),
            hashes: Vec::new(),
            heard: now,
            said: now,
            fresh: false,
            stall: None,
            reports: Vec::new(),
            corrupt_at: None,
        };
        let hello = Msg::Hello(g.cfg.hello.clone());
        g.send(&hello, now);
        g
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Why the host said no, with both sides of what was compared (a refused join shows both
    /// content hashes, §7).
    pub fn refusal(&self) -> Option<String> {
        match &self.phase {
            Phase::Refused(r) => Some(r.explain(&self.cfg.hello)),
            _ => None,
        }
    }

    /// The world, once welcomed.
    pub fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    /// For tests: a change here is in no bundle (a desync on purpose).
    pub fn sim_mut(&mut self) -> Option<&mut Sim> {
        self.sim.as_mut()
    }

    pub fn seat(&self) -> Option<Seat> {
        self.seat
    }

    pub fn delay(&self) -> u8 {
        self.delay
    }

    /// The last step's events.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn stall(&self) -> Option<StallView> {
        self.stall
    }

    /// The state hash at the last hash point: `(frame, hash)`.
    pub fn last_hash(&self) -> Option<(u32, u64)> {
        self.book.last_hash()
    }

    /// Wait for the replica to finish every hash point it owes; they go at the next poll (tests,
    /// which step faster than real time; `book.rs`).
    pub fn settle(&mut self) {
        let done = self.book.settle();
        self.hashes.extend(done);
    }

    pub fn drain_reports(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.reports)
    }

    /// Bundles in hand in a row from the next frame (how far behind the host this guest is).
    pub fn backlog(&self) -> u32 {
        let Some(sim) = &self.sim else { return 0 };
        let mut f = sim.state().frame;
        while self.bundles.contains_key(&f) {
            f += 1;
        }
        f - sim.state().frame
    }

    /// Get up and go.
    pub fn leave(&mut self) {
        let _ = self.link.send(wire::encode(&Msg::Bye(ByeWhy::Left)));
        self.link.close();
        self.phase = Phase::Ended("left".to_owned());
    }

    fn send(&mut self, m: &Msg, now: u64) {
        if self.link.send(wire::encode(m)).is_err() {
            self.end("the connection closed");
        }
        self.said = now;
    }

    fn end(&mut self, why: &str) {
        if !matches!(self.phase, Phase::Ended(_) | Phase::Refused(_)) {
            self.phase = Phase::Ended(why.to_owned());
        }
        self.link.close();
    }

    fn over(&self) -> bool {
        matches!(self.phase, Phase::Ended(_) | Phase::Refused(_))
    }

    /// Take what has come, and say what is due.
    pub fn poll(&mut self, now: u64) {
        while !self.over() {
            match self.link.recv() {
                Ok(Some(bytes)) => {
                    self.heard = now;
                    match wire::decode(&bytes) {
                        Ok(m) => self.on_msg(m, now),
                        Err(e) => self.end(&e.to_string()),
                    }
                }
                Ok(None) => break,
                Err(_) => self.end("the host hung up"),
            }
        }
        if self.over() {
            return;
        }
        if now.saturating_sub(self.heard) >= HOST_GONE_MS {
            self.end("the host has said nothing for 10 s");
            return;
        }
        match self.phase {
            Phase::Hello if now.saturating_sub(self.said) >= HELLO_EVERY_MS => {
                let hello = Msg::Hello(self.cfg.hello.clone());
                self.send(&hello, now);
            }
            Phase::Building { .. } => {
                if self.building.as_ref().is_some_and(JoinHandle::is_finished) {
                    match self.building.take().expect("a build").join() {
                        Ok(Ok(bps)) => {
                            self.bps = Some(bps);
                            self.phase = Phase::Ready;
                            self.send(&Msg::Ready, now);
                        }
                        Ok(Err(e)) => self.end(&format!("the county would not build: {e}")),
                        Err(_) => self.end("the county's build panicked"),
                    }
                }
            }
            Phase::Ready if now.saturating_sub(self.said) >= READY_EVERY_MS => self.send(&Msg::Ready, now),
            Phase::Playing => {
                // The hash points the worker has encoded since.
                let done = self.book.poll();
                self.hashes.extend(done);
                for (frame, hash) in std::mem::take(&mut self.hashes) {
                    self.send(&Msg::Hash { frame, hash }, now);
                }
                if self.fresh || now.saturating_sub(self.said) >= INPUTS_EVERY_MS {
                    self.send_inputs(now);
                }
            }
            _ => {}
        }
        if self.link.flush().is_err() {
            self.end("the connection closed");
        }
    }

    fn ack(&self) -> u32 {
        let Some(sim) = &self.sim else { return 0 };
        let mut f = sim.state().frame;
        while self.bundles.contains_key(&f) {
            f += 1;
        }
        f
    }

    fn send_inputs(&mut self, now: u64) {
        let ack = self.ack();
        // What the host has bundled needs no resending.
        self.outbox = self.outbox.split_off(&ack);
        let first = self.outbox.keys().next().copied().unwrap_or(self.next_in);
        let items: Vec<Item> = self.outbox.values().cloned().collect();
        self.fresh = false;
        self.send(&Msg::Inputs { epoch: self.epoch, ack, first, items }, now);
    }

    fn on_msg(&mut self, m: Msg, now: u64) {
        match m {
            Msg::Refuse(r) => {
                self.phase = Phase::Refused(r);
                self.link.close();
            }
            Msg::Accept { seed, delay, .. } if self.phase == Phase::Hello => {
                self.delay = delay;
                if self.bps.as_ref().is_some_and(|b| b.seed() == seed) {
                    self.phase = Phase::Ready;
                    self.send(&Msg::Ready, now);
                } else {
                    self.bps = None;
                    self.phase = Phase::Building { seed };
                    self.building =
                        Some(std::thread::spawn(move || Blueprints::build(seed).map_err(|e| e.to_string())));
                }
            }
            Msg::Welcome { epoch, seat, frame, delay, need_from, snapshot } if epoch > self.epoch => {
                let Some(bps) = self.bps.clone() else { return };
                match Sim::from_snapshot_with(&snapshot, bps.clone()) {
                    Ok(sim) => {
                        debug_assert_eq!(sim.state().frame, frame);
                        self.sim = Some(sim);
                        self.seat = Some(seat);
                        self.epoch = epoch;
                        self.delay = delay;
                        self.next_in = self.next_in.max(need_from);
                        self.bundles = self.bundles.split_off(&frame);
                        self.book.start(snapshot, bps);
                        self.phase = Phase::Playing;
                        self.fresh = true;
                    }
                    Err(e) => self.end(&format!("the world did not load: {e}")),
                }
            }
            Msg::Bundle(b) => {
                if self.sim.as_ref().is_none_or(|s| b.frame >= s.state().frame) {
                    self.bundles.entry(b.frame).or_insert(b);
                }
            }
            Msg::Stall { frame, seats, waited_ms, wait } => {
                self.stall = (seats != 0).then_some(StallView { frame, seats, waited_ms: u64::from(waited_ms), wait });
            }
            Msg::DesyncFound { frame } => self.dump(frame, now),
            Msg::DesyncReport(r) => self.reports.push(r),
            Msg::Bye(why) => self.end(match why {
                ByeWhy::Left => "the host left",
                ByeWhy::Dropped => "the host dropped this seat: its input stopped coming",
                ByeWhy::HostClosed => "the host closed the session",
                ByeWhy::Replaced => "another connection with this token took the seat",
            }),
            _ => {}
        }
    }

    /// The host says our hash at `frame` was not its own: send the save of that frame and the
    /// hash after every step since the hash point before, re-simulated from our own save.
    fn dump(&mut self, frame: u32, now: u64) {
        let Some(bps) = self.bps.clone() else { return };
        // The saves are the worker's: wait for what it has in hand.
        let done = self.book.settle();
        self.hashes.extend(done);
        let Some(save) = self.book.save_at(frame).map(<[u8]>::to_vec) else { return };
        let from = frame.saturating_sub(HASH_EVERY);
        let trail = self.book.trail(from, frame, &bps).unwrap_or_default();
        if let (Some(dir), Some(seat)) = (&self.cfg.desync_dir, self.seat) {
            let _ = std::fs::write(dir.join(format!("desync-{frame}-{}.save", seat.0)), &save);
        }
        self.send(&Msg::DesyncDump { frame, trail_from: from, trail, save }, now);
    }

    /// Step the next frame if its bundle has come. `local` is this seat's held input and
    /// presses, for frame `f + delay`; the presses are taken only when the step happens.
    pub fn try_step(&mut self, _now: u64, local: Option<(InputFrame, &mut Vec<Command>)>) -> Option<Stepped> {
        let sim = self.sim.as_mut()?;
        let f = sim.state().frame;
        let mut b = self.bundles.remove(&f)?;
        if self.corrupt_at == Some(f) {
            b.frames[0].mv_dir = jane_core::Angle(b.frames[0].mv_dir.0.wrapping_add(0x4000));
            b.frames[0].mv_mag = 127;
        }
        let out = step(sim, &b);
        self.events.clear();
        self.events.extend_from_slice(sim.drain_events());
        self.book.stepped(sim, b);
        if let (Some((held, presses)), Some(_)) = (local, self.seat) {
            while self.next_in <= f + u32::from(self.delay) {
                self.outbox.insert(self.next_in, Item { frame: held, cmds: std::mem::take(presses) });
                self.next_in += 1;
                self.fresh = true;
            }
        }
        Some(out)
    }
}
