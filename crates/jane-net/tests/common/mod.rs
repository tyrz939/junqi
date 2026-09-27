//! A table in one process: a host and guests over in-memory links (or a lossy shim over them),
//! each seat played by a `jane-bot` model on its own peer's view, and a clock the test drives
//! (a tick is 1/60 s).

#![allow(dead_code)]

use std::sync::OnceLock;

use jane_bot::{Act, Bot, Model};
use jane_net::link::{Link, LinkError, Listener, Loss, Lossy, MemDialer, MemListener, mem_listener};
use jane_net::{Guest, GuestConfig, Host, HostConfig, Note, Phase};
use jane_sim::event::Event;
use jane_sim::input::Command;
use jane_sim::{Blueprints, ClientToken, Seat, Sim};

pub const SEED: u32 = 1;

pub fn bps() -> Blueprints {
    static B: OnceLock<Blueprints> = OnceLock::new();
    B.get_or_init(|| Blueprints::build(SEED).expect("the seed builds")).clone()
}

/// A hand-written seat: a closure over its view and the events since it last acted.
pub type Script = Box<dyn FnMut(&jane_sim::View<'_>, &[Event]) -> Act>;

/// What plays a seat.
pub enum Policy {
    Bot(Box<Bot>),
    /// Stands still.
    Idle,
    Script(Script),
}

impl Policy {
    pub fn bot(model: Model) -> Policy {
        Policy::Bot(Box::new(Bot::story(model)))
    }

    fn act(&mut self, seat: Option<Seat>, sim: Option<&Sim>, events: &[Event]) -> Act {
        let v = seat.and_then(|s| sim.and_then(|sim| sim.view(s)));
        match self {
            Policy::Bot(b) => {
                if let Some(s) = seat {
                    b.seat = s;
                }
                b.act(v.as_ref(), events)
            }
            Policy::Idle => Act::idle(),
            Policy::Script(f) => v.map_or_else(Act::idle, |v| f(&v, events)),
        }
    }
}

pub struct Peer {
    pub g: Guest,
    pub policy: Policy,
    pub presses: Vec<Command>,
    /// Events since the policy last acted.
    pub heard: Vec<Event>,
    /// Frozen: neither polled nor stepped (a machine that hangs).
    pub frozen: bool,
    pub token: u64,
}

/// A listener whose links lose and reorder what they send.
#[derive(Debug)]
struct LossyListener {
    inner: MemListener,
    loss: Loss,
    n: u32,
}

impl Listener for LossyListener {
    fn accept(&mut self) -> Result<Option<Box<dyn Link>>, LinkError> {
        Ok(self.inner.accept()?.map(|l| {
            self.n += 1;
            Box::new(Lossy::new(BoxLink(l), Loss { seed: self.loss.seed + 100 * self.n, ..self.loss })) as Box<dyn Link>
        }))
    }
}

#[derive(Debug)]
struct BoxLink(Box<dyn Link>);

impl Link for BoxLink {
    fn send(&mut self, frame: Vec<u8>) -> Result<(), LinkError> {
        self.0.send(frame)
    }
    fn recv(&mut self) -> Result<Option<Vec<u8>>, LinkError> {
        self.0.recv()
    }
    fn flush(&mut self) -> Result<(), LinkError> {
        self.0.flush()
    }
    fn peer(&self) -> String {
        self.0.peer()
    }
    fn close(&mut self) {
        self.0.close();
    }
}

pub struct Table {
    pub tick: u64,
    pub host: Host,
    pub host_policy: Policy,
    pub host_presses: Vec<Command>,
    pub host_heard: Vec<Event>,
    pub dialer: MemDialer,
    pub peers: Vec<Peer>,
    pub loss: Option<Loss>,
    pub notes: Vec<Note>,
    /// Hash points at which every peer at the table held the host's hash.
    pub agreed: u32,
}

impl Table {
    pub fn new(cfg: HostConfig, host_policy: Policy) -> Table {
        Self::with_loss(cfg, host_policy, None)
    }

    pub fn with_loss(cfg: HostConfig, host_policy: Policy, loss: Option<Loss>) -> Table {
        let (l, dialer) = mem_listener();
        let listener: Box<dyn Listener> = match loss {
            Some(loss) => Box::new(LossyListener { inner: l, loss, n: 0 }),
            None => Box::new(l),
        };
        let sim = Sim::new_game_with(bps(), "Jane");
        Table {
            tick: 0,
            host: Host::new(sim, cfg, listener),
            host_policy,
            host_presses: Vec::new(),
            host_heard: Vec::new(),
            dialer,
            peers: Vec::new(),
            loss,
            notes: Vec::new(),
            agreed: 0,
        }
    }

    pub fn now(&self) -> u64 {
        self.tick * 1000 / 60
    }

    /// A guest knocks with `token`; returns her index.
    pub fn knock(&mut self, token: u64, policy: Policy) -> usize {
        self.knock_with(GuestConfig::new(ClientToken(token)), policy)
    }

    pub fn knock_with(&mut self, cfg: GuestConfig, policy: Policy) -> usize {
        let token = cfg.hello.token;
        let link = self.dialer.dial();
        let link: Box<dyn Link> = match self.loss {
            Some(loss) => Box::new(Lossy::new(link, Loss { seed: loss.seed + token as u32, ..loss })),
            None => Box::new(link),
        };
        let g = Guest::new(link, cfg, Some(bps()), self.now());
        self.peers.push(Peer { g, policy, presses: Vec::new(), heard: Vec::new(), frozen: false, token });
        self.peers.len() - 1
    }

    /// One tick of wall time: everyone polls, the host steps if it can, each guest steps what
    /// it has (at most a few frames, as the app does).
    pub fn tick(&mut self) {
        self.tick += 1;
        let now = self.now();
        self.host.poll(now);
        for p in self.peers.iter_mut().filter(|p| !p.frozen) {
            p.g.poll(now);
        }
        self.host.poll(now);

        let act = self.host_policy.act(self.host.seat(), Some(self.host.sim()), &self.host_heard);
        self.host_presses.extend(act.cmds);
        if self.host.try_step(now, Some((act.frame, &mut self.host_presses))).is_some() {
            self.host_heard.clear();
            self.host_heard.extend_from_slice(self.host.events());
        }
        self.notes.extend(self.host.drain_notes());
        self.host.poll(now);

        for p in self.peers.iter_mut().filter(|p| !p.frozen) {
            p.g.poll(now);
            for _ in 0..4 {
                if p.g.backlog() == 0 {
                    break;
                }
                let act = p.policy.act(p.g.seat(), p.g.sim(), &p.heard);
                p.presses.extend(act.cmds);
                if p.g.try_step(now, Some((act.frame, &mut p.presses))).is_none() {
                    break;
                }
                p.heard.clear();
                p.heard.extend_from_slice(p.g.events());
            }
            p.g.poll(now);
        }
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Tick until `f` holds, at most `max` ticks. Panics if it never does.
    pub fn until(&mut self, max: u32, what: &str, mut f: impl FnMut(&Table) -> bool) {
        for _ in 0..max {
            if f(self) {
                return;
            }
            self.tick();
        }
        assert!(f(self), "never: {what} (tick {}, frame {})", self.tick, self.host.sim().state().frame);
    }

    /// Until guest `i` is at the table.
    pub fn seated(&mut self, i: usize) -> Seat {
        self.until(600, "the guest sits down", |t| {
            t.peers[i].g.phase() == &Phase::Playing
                && t.peers[i].g.seat().is_some_and(|s| t.host.sim().state().player(s).is_some_and(|p| p.connected))
        });
        self.peers[i].g.seat().unwrap()
    }

    /// Every playing guest's hash equals the host's, compared at the host's frame once each has
    /// caught up. Steps the table with the host holding still until they have.
    pub fn same_hash(&mut self) -> u64 {
        let h = self.host.sim().hash();
        let f = self.host.sim().state().frame;
        for p in &mut self.peers {
            if p.frozen || p.g.phase() != &Phase::Playing {
                continue;
            }
            let now = self.tick * 1000 / 60;
            for _ in 0..50 {
                if p.g.sim().is_some_and(|s| s.state().frame == f) {
                    break;
                }
                p.g.poll(now);
                let mut none = Vec::new();
                p.g.try_step(now, Some((jane_sim::InputFrame::IDLE, &mut none)));
            }
            let s = p.g.sim().unwrap();
            assert_eq!(s.state().frame, f, "guest caught up");
            assert_eq!(s.hash(), h, "seat {:?}: the guest's world is the host's at frame {f}", p.g.seat());
        }
        h
    }

    pub fn desyncs(&self) -> Vec<&jane_net::wire::Report> {
        self.notes
            .iter()
            .filter_map(|n| match n {
                Note::Desync(r) => Some(r),
                _ => None,
            })
            .collect()
    }
}
