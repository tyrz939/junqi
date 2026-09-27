//! The host (ARCHITECTURE.md §7): holds the world, seats guests, paces the table and says what
//! every frame is. A star: every guest talks only to the host; the host also simulates (and,
//! in `jane-app --host`, plays seat 0; `jane serve` plays nobody).
//!
//! Per frame `f` the host needs every active seat's input for `f` (its own included, sampled
//! `delay` frames earlier); when it has them it makes `Bundle(f)` (the seats' held frames and
//! their presses in `(seat, seq)` order, its own joins and leaves among them), steps it, and
//! sends it to every guest. A seat whose input is late stalls the table: after
//! [`HostConfig::stall_ms`] everyone is told who is awaited, after [`HostConfig::drop_ms`] the
//! seat is dropped (a `Leave` in the next bundle) unless the host's **wait** toggle is on.
//!
//! A newcomer says hello (content hash, build, token), is told the seed, builds the county and
//! says ready; at the next step boundary the host sends the world as it is (a save of the live
//! state) and puts her `Join` in the bundle it makes next. The same welcome without a join puts
//! a guest back on the timeline after a desync (a resync).

use std::collections::BTreeMap;
use std::path::PathBuf;

use jane_sim::event::{Event, EventKind};
use jane_sim::input::{Command, InputFrame, StampedCommand};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{ClientToken, Seat, Sim, Stepped};

use crate::book::{Book, step};
use crate::link::{Link, Listener};
use crate::wire::{
    self, BUILD, Bundle, ByeWhy, DEFAULT_DELAY, Hello, Item, MAX_DELAY, MIN_DELAY, Msg, PROTO, Refusal, Report, Why,
    content_hash,
};

/// Frames a joiner has to load the world and catch up before her input is awaited.
const JOIN_SLACK: u32 = 30;
/// A guest whose acknowledgement has not moved for this long is sent again what it lacks.
const RESEND_MS: u64 = 100;
/// At most this many bundles in one resend.
const RESEND_MAX: usize = 120;
/// The host speaks at least this often.
const BEAT_MS: u64 = 500;
/// A connection that has not sat down in this long is hung up.
const HANDSHAKE_MS: u64 = 60_000;
/// A welcome not answered with input in this long is sent again (it was lost).
const REWELCOME_MS: u64 = 1_000;
/// A desync's explanation is waited for this long before the guest is resynced without it.
const DUMP_MS: u64 = 5_000;

#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Frames of input delay, `MIN_DELAY..=MAX_DELAY`.
    pub delay: u8,
    /// At most this many at the table, the host's own seat included (1..=4).
    pub seats: u8,
    /// The wait toggle (§12): a stalled seat is never dropped.
    pub wait: bool,
    /// The host plays seat 0 (`jane-app --host`); off, the host plays nobody (`jane serve`).
    pub plays: bool,
    /// What discovery shows.
    pub name: String,
    /// A seat late this long is shown as stalled.
    pub stall_ms: u64,
    /// A seat late this long is dropped (unless `wait`).
    pub drop_ms: u64,
    /// Where `desync-<frame>-<seat>.save` files are written; `None` writes none.
    pub desync_dir: Option<PathBuf>,
    /// After a desync, put the odd guest back on the host's timeline.
    pub resync: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            delay: DEFAULT_DELAY,
            seats: MAX_PLAYERS as u8,
            wait: false,
            plays: true,
            name: "Jane".to_owned(),
            stall_ms: 500,
            drop_ms: 10_000,
            desync_dir: None,
            resync: true,
        }
    }
}

/// Something the app or the log may want to say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    Joined { seat: Seat, token: u64 },
    Left { seat: Seat },
    Dropped { seat: Seat },
    Refused { peer: String, why: Why },
    Desync(Report),
    Resynced { seat: Seat },
}

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Note::Joined { seat, token } => write!(f, "seat {} sat down (token {token:x})", seat.0),
            Note::Left { seat } => write!(f, "seat {} got up", seat.0),
            Note::Dropped { seat } => write!(f, "seat {} dropped: its input stopped coming", seat.0),
            Note::Refused { peer, why } => write!(f, "refused {peer}: {why:?}"),
            Note::Desync(r) => write!(f, "{r}"),
            Note::Resynced { seat } => write!(f, "seat {} was sent the world again", seat.0),
        }
    }
}

/// The table waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StallView {
    pub frame: u32,
    /// A bit per seat awaited.
    pub seats: u8,
    pub waited_ms: u64,
    pub wait: bool,
}

/// Hash checks so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checks {
    /// Guest hashes that matched the host's.
    pub ok: u64,
    pub bad: u64,
    /// The last hash point any guest matched.
    pub last: u32,
}

#[derive(Debug, Default)]
struct SeatIn {
    /// Input is awaited from this seat (it is at the table, or about to be).
    active: bool,
    /// Frames before this are idle for it.
    need_from: u32,
    inputs: BTreeMap<u32, Item>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Waiting for hello.
    Hello,
    /// Told the seed; waiting for ready.
    Accepted,
    /// To be welcomed at the next step boundary; `resync`: already seated.
    Joining {
        resync: bool,
    },
    Seated,
    Closed,
}

#[derive(Debug)]
struct Conn {
    link: Box<dyn Link>,
    state: State,
    token: u64,
    seat: Option<Seat>,
    epoch: u32,
    since: u64,
    welcomed: u64,
    /// Input has come since the last welcome.
    heard: bool,
    /// The next bundle the guest lacks, as it last said.
    acked: u32,
    ack_moved: u64,
    resent: u64,
    last_sent: u64,
    /// A desync being explained: the hash point, the guest's hash, and when it was found.
    desync: Option<(u32, u64, u64)>,
}

#[derive(Debug)]
pub struct Host {
    sim: Sim,
    cfg: HostConfig,
    listener: Box<dyn Listener>,
    beacon: Option<crate::discovery::Beacon>,
    port: u16,
    conns: Vec<Conn>,
    seats: [SeatIn; MAX_PLAYERS],
    /// Commands the host adds to the next bundle: joins (`None`), leaves, the door.
    inject: Vec<(Option<Seat>, Command)>,
    /// The token that sits in seat 0 on a host that plays nobody (the first to join).
    owner: Option<u64>,
    accepting: bool,
    book: Book,
    own_next_in: u32,
    blocked: Option<(u32, u64)>,
    stall: Option<StallView>,
    stall_said: u64,
    events: Vec<Event>,
    notes: Vec<Note>,
    checks: Checks,
    rested: bool,
    /// Guests' hashes that came before the host's own for that frame was encoded: `(token,
    /// frame, hash)`.
    waiting: Vec<(u64, u32, u64)>,
    tape: Option<crate::record::SessionTape>,
}

impl Host {
    /// Host `sim` (a new game or the host's loaded save) to whoever `listener` hands over. The
    /// first bundle opens the world (seat 0's `Open`); a host that plays nobody also gets seat 0
    /// up, so the first guest to come sits in it.
    pub fn new(sim: Sim, cfg: HostConfig, listener: Box<dyn Listener>) -> Host {
        let mut cfg = cfg;
        cfg.delay = cfg.delay.clamp(MIN_DELAY, MAX_DELAY);
        cfg.seats = cfg.seats.clamp(1, MAX_PLAYERS as u8);
        let f = sim.state().frame;
        let mut seats: [SeatIn; MAX_PLAYERS] = Default::default();
        let mut inject = vec![(Some(Seat::HOST), Command::Open(true))];
        let host_sat = sim.state().player(Seat::HOST).is_some_and(|p| p.connected);
        if cfg.plays {
            seats[0] = SeatIn { active: true, need_from: f + u32::from(cfg.delay), inputs: BTreeMap::new() };
        } else if host_sat {
            inject.push((Some(Seat::HOST), Command::Leave));
        }
        // The timeline begins from the world as it is (the replica loads the same, book.rs).
        let mut book = Book::default();
        book.start(sim.save(), sim.blueprints().clone());
        Host {
            own_next_in: f + u32::from(cfg.delay),
            sim,
            cfg,
            listener,
            beacon: None,
            port: 0,
            conns: Vec::new(),
            seats,
            inject,
            owner: None,
            accepting: true,
            book,
            blocked: None,
            stall: None,
            stall_said: 0,
            events: Vec::new(),
            notes: Vec::new(),
            checks: Checks::default(),
            rested: false,
            waiting: Vec::new(),
            tape: None,
        }
    }

    /// The port guests dial (what discovery tells askers, and the title bar shows).
    pub fn with_port(mut self, port: u16) -> Host {
        self.port = port;
        self
    }

    /// Answer LAN discovery (`JANE?`) on this beacon.
    pub fn with_beacon(mut self, beacon: crate::discovery::Beacon) -> Host {
        self.beacon = Some(beacon);
        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// For tests and tools: a change here is in no bundle (a desync on purpose).
    pub fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }

    pub fn into_sim(self) -> Sim {
        self.sim
    }

    pub fn config(&self) -> &HostConfig {
        &self.cfg
    }

    /// The seat this machine plays, if it plays.
    pub fn seat(&self) -> Option<Seat> {
        self.cfg.plays.then_some(Seat::HOST)
    }

    /// The last step's events.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn drain_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    pub fn stall(&self) -> Option<StallView> {
        self.stall
    }

    pub fn checks(&self) -> Checks {
        self.checks
    }

    /// The state hash at the last hash point: `(frame, hash)`.
    pub fn last_hash(&self) -> Option<(u32, u64)> {
        self.book.last_hash()
    }

    /// Whether anyone at the table rested since the last call: the world lives on the host's
    /// machine, so a guest's rest saves it (PLATFORM.md §2).
    /// Record the session as a `.jrp` tape from here; only a new game not yet stepped can be
    /// (a tape begins at New Game). Whether recording began.
    pub fn record(&mut self) -> bool {
        self.tape = crate::record::SessionTape::new(&self.sim);
        self.tape.is_some()
    }

    /// The tape so far, finished with the state's hash, and recording stops.
    pub fn take_tape(&mut self) -> Option<jane_sim::replay::Tape> {
        self.tape.take().map(|t| t.finish(&self.sim))
    }

    pub fn take_rested(&mut self) -> bool {
        std::mem::take(&mut self.rested)
    }

    /// The wait toggle: on, a stalled seat is waited for however long.
    pub fn set_wait(&mut self, on: bool) {
        self.cfg.wait = on;
    }

    /// Let newcomers sit down, or stop letting them (seat 0's `Open` when the host plays).
    /// Closing sends nobody home.
    pub fn set_open(&mut self, on: bool) {
        self.accepting = on;
        if self.cfg.plays {
            self.inject.push((Some(Seat::HOST), Command::Open(on)));
        }
    }

    /// Guests connected and seated.
    pub fn guests(&self) -> impl Iterator<Item = Seat> + '_ {
        self.conns.iter().filter(|c| c.state == State::Seated).filter_map(|c| c.seat)
    }

    /// Say goodbye to everyone.
    pub fn close(&mut self) {
        for c in &mut self.conns {
            if c.state != State::Closed {
                let _ = c.link.send(wire::encode(&Msg::Bye(ByeWhy::HostClosed)));
                c.link.close();
                c.state = State::Closed;
            }
        }
    }

    // --- the network ---------------------------------------------------------------------

    /// Take what has come, answer it, resend what was lost, and flush.
    pub fn poll(&mut self, now: u64) {
        while let Ok(Some(link)) = self.listener.accept() {
            self.conns.push(Conn {
                link,
                state: State::Hello,
                token: 0,
                seat: None,
                epoch: 0,
                since: now,
                welcomed: now,
                heard: false,
                acked: 0,
                ack_moved: now,
                resent: now,
                last_sent: now,
                desync: None,
            });
        }
        for i in 0..self.conns.len() {
            loop {
                if self.conns[i].state == State::Closed {
                    break;
                }
                match self.conns[i].link.recv() {
                    Ok(Some(bytes)) => match wire::decode(&bytes) {
                        Ok(m) => self.on_msg(i, m, now),
                        Err(_) => self.lost(i, None),
                    },
                    Ok(None) => break,
                    Err(_) => self.lost(i, None),
                }
            }
        }
        let next = self.sim.state().frame;
        let oldest = self.book.bundles_from(0).next().map_or(next, |b| b.frame);
        for i in 0..self.conns.len() {
            let c = &mut self.conns[i];
            match c.state {
                State::Seated => {
                    if c.acked < next
                        && now.saturating_sub(c.ack_moved) >= RESEND_MS
                        && now.saturating_sub(c.resent) >= RESEND_MS
                    {
                        if c.acked < oldest {
                            // Too far behind for the bundles kept: the world again.
                            c.state = State::Joining { resync: true };
                        } else {
                            c.resent = now;
                            let from = c.acked;
                            let bundles: Vec<Bundle> = self.book.bundles_from(from).take(RESEND_MAX).cloned().collect();
                            for b in bundles {
                                let _ = c.link.send(wire::encode(&Msg::Bundle(b)));
                            }
                            c.last_sent = now;
                        }
                    }
                    if now.saturating_sub(c.last_sent) >= BEAT_MS {
                        let _ = c.link.send(wire::encode(&Msg::Beat));
                        c.last_sent = now;
                    }
                    // A desync whose explanation never came (lost, or a guest that cannot say):
                    // put her back on the timeline all the same.
                    if c.desync.is_some_and(|(_, _, found)| now.saturating_sub(found) >= DUMP_MS) {
                        c.desync = None;
                        if self.cfg.resync {
                            c.state = State::Joining { resync: true };
                        }
                    }
                }
                State::Hello | State::Accepted if now.saturating_sub(c.since) >= HANDSHAKE_MS => {
                    c.link.close();
                    c.state = State::Closed;
                }
                _ => {}
            }
        }
        for i in 0..self.conns.len() {
            if self.conns[i].state != State::Closed && self.conns[i].link.flush().is_err() {
                self.lost(i, None);
            }
        }
        self.conns.retain(|c| c.state != State::Closed);
        // The host's own hash points as the replica finishes them; guests' that waited for them.
        if !self.book.poll().is_empty() {
            self.resolve_waiting(now);
        }
        if let Some(b) = &mut self.beacon {
            let used = self.sim.state().party_size();
            b.answer(&crate::discovery::Offer {
                name: self.cfg.name.clone(),
                port: self.port,
                seats_used: used,
                seats: self.cfg.seats,
                frame: self.sim.state().frame,
                content_hash: content_hash(),
                proto: PROTO,
                build: BUILD.to_owned(),
            });
        }
    }

    fn send(&mut self, i: usize, m: &Msg, now: u64) {
        let c = &mut self.conns[i];
        if c.link.send(wire::encode(m)).is_ok() {
            c.last_sent = now;
        }
    }

    fn refuse(&mut self, i: usize, why: Why) {
        let r = Refusal { why, proto: PROTO, content_hash: content_hash(), build: BUILD.to_owned() };
        let c = &mut self.conns[i];
        let _ = c.link.send(wire::encode(&Msg::Refuse(r)));
        let peer = c.link.peer();
        c.link.close();
        c.state = State::Closed;
        self.notes.push(Note::Refused { peer, why });
    }

    /// A connection is gone (hung up, or `bye`): her seat gets up in the next bundle.
    fn lost(&mut self, i: usize, why: Option<ByeWhy>) {
        let c = &mut self.conns[i];
        if c.state == State::Closed {
            return;
        }
        if let Some(w) = why {
            let _ = c.link.send(wire::encode(&Msg::Bye(w)));
        }
        c.link.close();
        let was = c.state;
        c.state = State::Closed;
        if let (Some(s), State::Seated | State::Joining { resync: true }) = (c.seat, was) {
            self.get_up(s);
            self.notes.push(if why == Some(ByeWhy::Dropped) {
                Note::Dropped { seat: s }
            } else {
                Note::Left { seat: s }
            });
        }
    }

    /// A command the host adds to the next bundle for a seat, or a sitting-down (`None`): the
    /// console's `join` (an idle body; its input is always idle, nobody sends it) and `leave`
    /// (a guest's seat so got up is hung up on, and may come back).
    pub fn inject(&mut self, seat: Option<Seat>, cmd: Command) {
        if let (Some(s), Command::Leave) = (seat, cmd) {
            if let Some(i) = self.conns.iter().position(|c| c.seat == Some(s) && c.state != State::Closed) {
                self.lost(i, Some(ByeWhy::Dropped));
                return;
            }
        }
        self.inject.push((seat, cmd));
    }

    fn get_up(&mut self, s: Seat) {
        let si = &mut self.seats[s.index()];
        if si.active {
            si.active = false;
            si.inputs.clear();
            self.inject.push((Some(s), Command::Leave));
        }
    }

    fn on_msg(&mut self, i: usize, m: Msg, now: u64) {
        let state = self.conns[i].state;
        match m {
            Msg::Hello(h) if matches!(state, State::Hello | State::Accepted) => self.on_hello(i, &h, now),
            Msg::Ready => match state {
                State::Accepted => self.conns[i].state = State::Joining { resync: false },
                State::Seated if !self.conns[i].heard && now.saturating_sub(self.conns[i].welcomed) >= REWELCOME_MS => {
                    // The welcome was lost: the world again, as it is now.
                    self.conns[i].state = State::Joining { resync: true };
                }
                _ => {}
            },
            Msg::Inputs { epoch, ack, first, items } if state == State::Seated => {
                self.on_inputs(i, epoch, ack, first, items, now);
            }
            Msg::Hash { frame, hash } if state == State::Seated => self.on_hash(i, frame, hash, now),
            Msg::DesyncDump { frame, trail_from, trail, save } => {
                self.on_dump(i, frame, trail_from, &trail, &save, now);
            }
            Msg::Bye(_) => self.lost(i, None),
            _ => {}
        }
    }

    fn on_hello(&mut self, i: usize, h: &Hello, now: u64) {
        if h.proto != PROTO {
            return self.refuse(i, Why::Proto);
        }
        if h.content_hash != content_hash() {
            return self.refuse(i, Why::Content);
        }
        if h.build != BUILD {
            return self.refuse(i, Why::Build);
        }
        if !self.accepting {
            return self.refuse(i, Why::Closed);
        }
        // The same person again: the older connection is presumed dead, and she gets up there.
        if let Some(j) = (0..self.conns.len()).find(|&j| {
            j != i
                && self.conns[j].token == h.token
                && self.conns[j].state != State::Closed
                && self.conns[j].seat.is_some()
        }) {
            self.lost(j, Some(ByeWhy::Replaced));
        }
        let joining = self.conns.iter().filter(|c| c.state == State::Joining { resync: false }).count();
        let active = self.seats.iter().filter(|s| s.active).count();
        if active + joining >= usize::from(self.cfg.seats) {
            return self.refuse(i, Why::Full);
        }
        let c = &mut self.conns[i];
        c.token = h.token;
        c.state = State::Accepted;
        let m = Msg::Accept { seed: self.sim.state().seed, name: self.sim.state().name.clone(), delay: self.cfg.delay };
        self.send(i, &m, now);
    }

    fn on_inputs(&mut self, i: usize, epoch: u32, ack: u32, first: u32, items: Vec<Item>, now: u64) {
        let next = self.sim.state().frame;
        let c = &mut self.conns[i];
        if ack > c.acked {
            c.acked = ack;
            c.ack_moved = now;
        }
        if epoch == c.epoch {
            c.heard = true;
        }
        let Some(s) = c.seat else { return };
        let si = &mut self.seats[s.index()];
        if !si.active {
            return;
        }
        for (k, item) in items.into_iter().enumerate() {
            let f = first + k as u32;
            // Already bundled, not yet wanted, or absurdly far ahead.
            if f < next || f < si.need_from || f > next + 600 {
                continue;
            }
            si.inputs.entry(f).or_insert(item);
        }
    }

    /// Wait for the replica to finish every hash point it owes, and compare the guests' hashes
    /// that waited for them (tests, which step faster than real time; `book.rs`).
    pub fn settle(&mut self, now: u64) {
        if !self.book.settle().is_empty() {
            self.resolve_waiting(now);
        }
    }

    fn resolve_waiting(&mut self, now: u64) {
        for (token, frame, hash) in std::mem::take(&mut self.waiting) {
            if let Some(i) = self.conns.iter().position(|c| c.token == token && c.state == State::Seated) {
                self.on_hash(i, frame, hash, now);
            }
        }
    }

    fn on_hash(&mut self, i: usize, frame: u32, hash: u64, now: u64) {
        let Some(own) = self.book.hash_at(frame) else {
            // Not encoded yet (the worker is a frame or two behind): ask again when it is.
            // (The replica may lag the host by several hash points; a spot point may already be
            // in. What waits more than a minute of frames is let go.)
            let now_frame = self.sim.state().frame;
            self.waiting.retain(|w| w.1 + 3600 > now_frame);
            if frame + 3600 > now_frame && self.waiting.len() < 256 {
                self.waiting.push((self.conns[i].token, frame, hash));
            }
            return;
        };
        if own == hash {
            self.checks.ok += 1;
            self.checks.last = self.checks.last.max(frame);
            return;
        }
        self.checks.bad += 1;
        if self.conns[i].desync.is_some() {
            return;
        }
        self.conns[i].desync = Some((frame, hash, now));
        if let (Some(dir), Some(save)) = (&self.cfg.desync_dir, self.book.save_at(frame)) {
            let _ = std::fs::write(dir.join(format!("desync-{frame}-host.save")), save);
        }
        self.send(i, &Msg::DesyncFound { frame }, now);
    }

    fn on_dump(&mut self, i: usize, frame: u32, trail_from: u32, trail: &[u64], save: &[u8], now: u64) {
        let Some((at, guest, _)) = self.conns[i].desync.take() else { return };
        if at != frame {
            return;
        }
        let seat = self.conns[i].seat.unwrap_or(Seat(0));
        let bps = self.sim.blueprints().clone();
        // The saves are the worker's: wait for what it has in hand.
        self.book.settle();
        let first_step = if trail.is_empty() {
            None
        } else {
            self.book
                .trail(trail_from, frame, &bps)
                .and_then(|ours| ours.iter().zip(trail).position(|(a, b)| a != b))
                .map(|k| trail_from + k as u32)
        };
        let parts = match (self.book.save_at(frame), jane_sim::save::decode_state(save, &bps)) {
            (Some(own), Ok(theirs)) => match jane_sim::save::decode_state(own, &bps) {
                Ok(ours) => jane_sim::replay::diff_states(&ours, &theirs),
                Err(e) => vec![format!("the host's own save does not load: {e}")],
            },
            (_, Err(e)) => vec![format!("the guest's save does not load: {e}")],
            (None, _) => vec!["the host kept no save of that frame".to_owned()],
        };
        if let Some(dir) = &self.cfg.desync_dir {
            let _ = std::fs::write(dir.join(format!("desync-{frame}-{}.save", seat.0)), save);
        }
        let report = Report { frame, seat, host: self.book.hash_at(frame).unwrap_or(0), guest, first_step, parts };
        for j in 0..self.conns.len() {
            if self.conns[j].state == State::Seated {
                self.send(j, &Msg::DesyncReport(report.clone()), now);
            }
        }
        self.notes.push(Note::Desync(report));
        if self.cfg.resync {
            self.conns[i].state = State::Joining { resync: true };
            self.notes.push(Note::Resynced { seat });
        }
    }

    /// The seat token a guest joins the sim with. On a host that plays nobody, seat 0 (the
    /// world's own, got up at the start) is the first guest's, and hers again when she returns.
    fn sim_token(&mut self, token: u64) -> ClientToken {
        if !self.cfg.plays {
            if self.owner == Some(token) {
                return ClientToken::HOST;
            }
            if self.owner.is_none() && self.sim.seat_for(ClientToken::HOST) == Some(Seat::HOST) {
                self.owner = Some(token);
                return ClientToken::HOST;
            }
        }
        ClientToken(token)
    }

    /// At the step boundary `f`: welcome whoever waits (the world as it is now, before bundle
    /// `f`), putting at most one join in it.
    fn welcome(&mut self, f: u32, now: u64) {
        // A seat getting up in this bundle may be the one a returning guest wants back.
        let leaving = self.inject.iter().any(|(_, c)| matches!(c, Command::Leave));
        // One join a bundle: a second would be seated by a prediction that did not know of the first.
        let mut joined = self.inject.iter().any(|(s, _)| s.is_none());
        for i in 0..self.conns.len() {
            let State::Joining { resync } = self.conns[i].state else { continue };
            if resync {
                let Some(s) = self.conns[i].seat else { continue };
                let need_from = self.seats[s.index()].need_from;
                self.send_welcome(i, s, f, need_from, now);
                continue;
            }
            if joined || leaving || !self.sim.state().open {
                continue;
            }
            let token = self.conns[i].token;
            let who = self.sim_token(token);
            let active = self.seats.iter().filter(|s| s.active).count();
            let Some(s) = self.sim.seat_for(who).filter(|_| active < usize::from(self.cfg.seats)) else {
                self.refuse(i, Why::Full);
                continue;
            };
            joined = true;
            self.inject.push((None, Command::Join { who }));
            let need_from = f + u32::from(self.cfg.delay) + JOIN_SLACK;
            self.seats[s.index()] = SeatIn { active: true, need_from, inputs: BTreeMap::new() };
            self.conns[i].seat = Some(s);
            self.send_welcome(i, s, f, need_from, now);
            self.notes.push(Note::Joined { seat: s, token });
        }
    }

    fn send_welcome(&mut self, i: usize, seat: Seat, f: u32, need_from: u32, now: u64) {
        let snapshot = self.sim.save();
        let c = &mut self.conns[i];
        c.epoch += 1;
        c.state = State::Seated;
        c.welcomed = now;
        c.heard = false;
        c.acked = f;
        c.ack_moved = now;
        c.resent = now;
        c.desync = None;
        let m = Msg::Welcome { epoch: c.epoch, seat, frame: f, delay: self.cfg.delay, need_from, snapshot };
        self.send(i, &m, now);
    }

    // --- the table -----------------------------------------------------------------------

    /// Step the next frame if every seat's input for it has come. `local` is this machine's
    /// seat's held input and presses (a host that plays); they are for frame `f + delay`, and
    /// the presses are taken only when the step happens.
    pub fn try_step(&mut self, now: u64, local: Option<(InputFrame, &mut Vec<Command>)>) -> Option<Stepped> {
        let f = self.sim.state().frame;
        self.welcome(f, now);
        let mut missing = 0u8;
        for (s, si) in self.seats.iter().enumerate() {
            if si.active && f >= si.need_from && !si.inputs.contains_key(&f) {
                missing |= 1 << s;
            }
        }
        if missing != 0 {
            self.stalled(f, missing, now);
            return None;
        }
        self.blocked = None;
        if self.stall.take().is_some() {
            self.broadcast(&Msg::Stall { frame: f, seats: 0, waited_ms: 0, wait: self.cfg.wait }, now);
        }

        // The bundle: joins, then each seat's presses and what the host adds for it.
        let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
        let mut cmds = Vec::new();
        for (seq, (_, c)) in self.inject.iter().filter(|(s, _)| s.is_none()).enumerate() {
            cmds.push(StampedCommand { seat: None, seq: seq as u16, cmd: *c });
        }
        for (s, frame) in frames.iter_mut().enumerate() {
            let seat = Seat(s as u8);
            let mut seq = 0u16;
            if let Some(item) = self.seats[s].inputs.remove(&f) {
                *frame = item.frame;
                for c in item.cmds {
                    // A seat's own presses are hers; joins and the door's opening are the host's.
                    if matches!(c, Command::Join { .. }) {
                        continue;
                    }
                    cmds.push(StampedCommand { seat: Some(seat), seq, cmd: c });
                    seq = seq.wrapping_add(1);
                }
            }
            for (_, c) in self.inject.iter().filter(|(t, _)| *t == Some(seat)) {
                cmds.push(StampedCommand { seat: Some(seat), seq, cmd: *c });
                seq = seq.wrapping_add(1);
            }
        }
        self.inject.clear();
        let b = Bundle { frame: f, frames, cmds };
        let out = step(&mut self.sim, &b);
        if let Some(t) = &mut self.tape {
            t.stepped(&b, out, &self.sim);
        }
        self.events.clear();
        self.events.extend_from_slice(self.sim.drain_events());
        if self.events.iter().any(|e| e.kind == EventKind::Rest) {
            self.rested = true;
        }
        let m = wire::encode(&Msg::Bundle(b.clone()));
        for c in &mut self.conns {
            if c.state == State::Seated && c.link.send(m.clone()).is_ok() {
                c.last_sent = now;
            }
        }
        self.book.stepped(&self.sim, b);

        // This machine's input for `f + delay`.
        if let (Some((held, presses)), true) = (local, self.cfg.plays) {
            let si = &mut self.seats[0];
            while self.own_next_in <= f + u32::from(self.cfg.delay) {
                si.inputs.insert(self.own_next_in, Item { frame: held, cmds: std::mem::take(presses) });
                self.own_next_in += 1;
            }
        }
        Some(out)
    }

    fn broadcast(&mut self, m: &Msg, now: u64) {
        for i in 0..self.conns.len() {
            if self.conns[i].state == State::Seated {
                self.send(i, m, now);
            }
        }
    }

    fn stalled(&mut self, f: u32, missing: u8, now: u64) {
        let since = match self.blocked {
            Some((bf, t)) if bf == f => t,
            _ => {
                self.blocked = Some((f, now));
                now
            }
        };
        let waited = now.saturating_sub(since);
        if waited >= self.cfg.stall_ms {
            let v = StallView { frame: f, seats: missing, waited_ms: waited, wait: self.cfg.wait };
            self.stall = Some(v);
            if now.saturating_sub(self.stall_said) >= self.cfg.stall_ms {
                self.stall_said = now;
                let m = Msg::Stall { frame: f, seats: missing, waited_ms: waited as u32, wait: self.cfg.wait };
                self.broadcast(&m, now);
            }
        }
        if waited >= self.cfg.drop_ms && !self.cfg.wait {
            for s in 0..MAX_PLAYERS {
                if missing & (1 << s) == 0 {
                    continue;
                }
                let seat = Seat(s as u8);
                match self.conns.iter().position(|c| c.seat == Some(seat) && c.state != State::Closed) {
                    Some(i) => self.lost(i, Some(ByeWhy::Dropped)),
                    None => {
                        self.get_up(seat);
                        self.notes.push(Note::Dropped { seat });
                    }
                }
            }
            self.blocked = None;
        }
    }
}
