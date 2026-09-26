//! The session the app plays through (ARCHITECTURE.md §7 "local play"): alone, the sim is
//! stepped as it always was; hosting or joining, the next frame is stepped only when every
//! seat's input for it has come. The app's loop is the same either way: each tick it hands over
//! its seat's held input and its presses and asks for a step.
//!
//! **Pause is solo-only** (ENGINE.md §4, PLATFORM.md §2): a local session does not step while
//! paused; a lockstep session always steps, with this seat's stick idle, because a table with
//! company (or open to it) never stops for one person's menu.

use std::time::Duration;

use jane_sim::event::Event;
use jane_sim::input::{Command, InputFrame, StampedCommand, StepInput};
use jane_sim::tuning::MAX_PLAYERS;
use jane_sim::{Blueprints, Seat, Sim, Stepped};

use crate::guest::{Guest, GuestConfig, Phase};
use crate::host::{Host, HostConfig, StallView};
use crate::link::{LinkError, TcpLink, TcpListen};
use crate::wire::Report;

/// Alone: today's game.
#[derive(Debug)]
pub struct Local {
    sim: Sim,
    seq: u16,
    events: Vec<Event>,
    rested: bool,
}

#[derive(Debug)]
pub enum Session {
    Local(Box<Local>),
    Host(Box<Host>),
    Guest(Box<Guest>),
}

/// What the title bar (and later the HUD) shows of the session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// `"alone"`, `"hosting"` or `"joined"`.
    pub role: &'static str,
    /// Seats connected, this one's included.
    pub seats: u8,
    pub stall: Option<StallView>,
    /// While a guest is not yet at the table: what it is doing.
    pub joining: Option<String>,
    /// The session is over, and why (a guest whose host went, a refused join).
    pub ended: Option<String>,
    /// The last desync reported.
    pub desync: Option<Report>,
}

impl Session {
    pub fn local(sim: Sim) -> Session {
        Session::Local(Box::new(Local { sim, seq: 0, events: Vec::new(), rested: false }))
    }

    /// Host `sim` on the LAN: listen on `port` (TCP) and answer discovery on it (UDP).
    pub fn host(sim: Sim, cfg: HostConfig, port: u16) -> Result<Session, LinkError> {
        Ok(Session::host_on(sim, cfg, TcpListen::bind(port)?))
    }

    fn host_on(sim: Sim, cfg: HostConfig, l: TcpListen) -> Session {
        let port = l.port();
        let mut host = Host::new(sim, cfg, Box::new(l)).with_port(port);
        // A second host on one machine cannot answer discovery on the same port; joining it by
        // address still works.
        if let Ok(b) = crate::discovery::Beacon::bind(port) {
            host = host.with_beacon(b);
        }
        Session::Host(Box::new(host))
    }

    /// Open this world to the LAN (the pause menu's "Open this world", `AppIntent::Host`): a
    /// local session becomes a host with the same sim. Anything else is returned as it was;
    /// on failure the local session comes back with the error.
    pub fn open_to_lan(self, cfg: HostConfig, port: u16) -> Result<Session, (Session, LinkError)> {
        let Session::Local(l) = self else { return Ok(self) };
        match TcpListen::bind(port) {
            Ok(listener) => Ok(Session::host_on(l.sim, cfg, listener)),
            Err(e) => Err((Session::Local(l), e)),
        }
    }

    /// Join the host at `addr` (`host[:port]`). `bps`: blueprints already built for the seed
    /// the host will name, if the caller has them.
    pub fn join(addr: &str, cfg: GuestConfig, bps: Option<Blueprints>, now: u64) -> Result<Session, LinkError> {
        let link = TcpLink::connect(addr, Duration::from_secs(5))?;
        Ok(Session::Guest(Box::new(Guest::new(Box::new(link), cfg, bps, now))))
    }

    /// The world, `None` while a guest is not yet welcomed.
    pub fn sim(&self) -> Option<&Sim> {
        match self {
            Session::Local(l) => Some(&l.sim),
            Session::Host(h) => Some(h.sim()),
            Session::Guest(g) => g.sim(),
        }
    }

    /// The seat this machine plays.
    pub fn seat(&self) -> Option<Seat> {
        match self {
            Session::Local(_) => Some(Seat::HOST),
            Session::Host(h) => h.seat(),
            Session::Guest(g) => g.seat().filter(|_| g.sim().is_some()),
        }
    }

    /// The port a host listens on.
    pub fn port(&self) -> Option<u16> {
        match self {
            Session::Host(h) => Some(h.port()),
            _ => None,
        }
    }

    /// Whether a pause stops the world: only alone.
    pub fn pauses(&self) -> bool {
        matches!(self, Session::Local(_))
    }

    /// The network: take what came, send what is due. Nothing for a local session.
    pub fn poll(&mut self, now: u64) {
        match self {
            Session::Local(_) => {}
            Session::Host(h) => h.poll(now),
            Session::Guest(g) => g.poll(now),
        }
    }

    /// One tick: step the next frame if it can be stepped, with `held` as this seat's input and
    /// `presses` its commands (taken only when a step happens). `paused`: this seat's menu is
    /// open; alone the world holds still, with company her stick is idle and nothing else.
    pub fn try_step(
        &mut self,
        now: u64,
        held: InputFrame,
        presses: &mut Vec<Command>,
        paused: bool,
    ) -> Option<Stepped> {
        let held = if paused { InputFrame::IDLE } else { held };
        match self {
            Session::Local(l) => {
                if paused {
                    return None;
                }
                let cmds: Vec<StampedCommand> = presses
                    .drain(..)
                    .map(|cmd| {
                        l.seq = l.seq.wrapping_add(1);
                        StampedCommand { seat: Some(Seat::HOST), seq: l.seq, cmd }
                    })
                    .collect();
                let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
                frames[0] = held;
                let out = l.sim.step(&StepInput { frames, commands: &cmds });
                l.events.clear();
                l.events.extend_from_slice(l.sim.drain_events());
                if l.events.iter().any(|e| e.kind == jane_sim::EventKind::Rest) {
                    l.rested = true;
                }
                Some(out)
            }
            Session::Host(h) => {
                let plays = h.seat().is_some();
                h.try_step(now, plays.then_some((held, presses)))
            }
            Session::Guest(g) => g.try_step(now, Some((held, presses))),
        }
    }

    /// The last step's events.
    pub fn events(&self) -> &[Event] {
        match self {
            Session::Local(l) => &l.events,
            Session::Host(h) => h.events(),
            Session::Guest(g) => g.events(),
        }
    }

    /// Frames a guest could step now beyond the next (it is behind the host by this many).
    pub fn backlog(&self) -> u32 {
        match self {
            Session::Guest(g) => g.backlog(),
            _ => 0,
        }
    }

    /// Whether anyone rested since the last call: the save is written where the world lives
    /// (alone or hosting); a guest's rest saves the host's world, not the guest's.
    pub fn take_rested(&mut self) -> bool {
        match self {
            Session::Local(l) => std::mem::take(&mut l.rested),
            Session::Host(h) => h.take_rested(),
            Session::Guest(_) => false,
        }
    }

    pub fn status(&mut self) -> Status {
        let seats = self.sim().map_or(0, |s| s.state().party_size());
        match self {
            Session::Local(_) => Status { role: "alone", seats, ..Status::default() },
            Session::Host(h) => {
                let desync = h.drain_notes().into_iter().rev().find_map(|n| match n {
                    crate::host::Note::Desync(r) => Some(r),
                    _ => None,
                });
                Status { role: "hosting", seats, stall: h.stall(), desync, ..Status::default() }
            }
            Session::Guest(g) => {
                let (joining, ended) = match g.phase() {
                    Phase::Hello => (Some("knocking".to_owned()), None),
                    Phase::Building { seed } => (Some(format!("building the county of seed {seed}")), None),
                    Phase::Ready => (Some("waiting for the world".to_owned()), None),
                    Phase::Playing => (None, None),
                    Phase::Refused(_) => (None, g.refusal().map(|r| format!("refused: {r}"))),
                    Phase::Ended(why) => (None, Some(why.clone())),
                };
                let desync = g.drain_reports().pop();
                Status { role: "joined", seats, stall: g.stall(), joining, ended, desync }
            }
        }
    }

    /// Leave (a guest) or close the table (a host).
    pub fn close(&mut self) {
        match self {
            Session::Local(_) => {}
            Session::Host(h) => h.close(),
            Session::Guest(g) => g.leave(),
        }
    }
}
