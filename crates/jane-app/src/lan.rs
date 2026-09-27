//! The app's side of playing together (ARCHITECTURE.md §7; PRESENTATION.md §3.2 "Host, Join"):
//! the Host and Join screens' state, the LAN list from `discovery::Finder`, a join under way
//! until the host welcomes it, the client token, and what the table's changes say as toasts.
//!
//! The loop itself steps through `jane_net::Session` whatever the session (`app.rs`): alone it
//! is today's game and pause holds the world; hosting or joined, nothing freezes.

use jane_net::discovery::Finder;
use jane_net::{GuestConfig, HostConfig, Phase, Session, StallView};
use jane_present::ui::lan::{HostChoice, HostState, JoinState, LanRow};
use jane_sim::{ClientToken, Sim};

use crate::config::Config;
use crate::saves::Dirs;

/// The coats' names, seat by seat: how players are told apart (PLATFORM.md §2).
pub const COAT_NAMES: [&str; 4] = ["plum", "teal", "moss", "ochre"];

/// Ticks between discovery asks while the Join screen is up.
const ASK_EVERY: u64 = 60;

#[derive(Debug, Default)]
pub struct Lan {
    pub host_form: HostState,
    pub join_form: JoinState,
    finder: Option<Finder>,
    asked: Option<u64>,
    /// The hosts that answered, as the Join list shows them.
    pub found: Vec<LanRow>,
    /// A join under way: knocking, building the county, waiting for the world.
    pub joining: Option<Session>,
    /// What the Join screen says under the field: progress, or why it ended.
    pub status: Option<(String, bool)>,
    /// The table waits for someone (a lockstep session's stall).
    pub stall: Option<StallView>,
    /// The seats connected when the last step ended, a bit each.
    pub seats: u8,
    /// The port hosts listen on and joins dial by default.
    pub port: u16,
}

impl Lan {
    pub fn new(port: u16) -> Lan {
        Lan { port, ..Lan::default() }
    }

    /// The Join screen opened: ask the network who hosts.
    pub fn open_finder(&mut self, last: Option<&str>) {
        // Every host answers on the discovery port whatever port it plays on; this app's port
        // too, for a second host on one machine.
        self.finder = Finder::new(jane_net::discovery::DISCOVERY_PORT).ok().map(|f| f.also(self.port));
        self.asked = None;
        self.found.clear();
        self.status = None;
        if self.join_form.addr.is_empty() {
            if let Some(a) = last {
                a.clone_into(&mut self.join_form.addr);
            }
        }
    }

    /// Stop asking (the Join screen closed).
    pub fn close_finder(&mut self) {
        self.finder = None;
    }

    /// Asks again every second and reads the answers.
    pub fn poll_finder(&mut self, tick: u64) {
        let Some(f) = &mut self.finder else { return };
        if self.asked.is_none_or(|a| tick.saturating_sub(a) >= ASK_EVERY) {
            f.ask();
            self.asked = Some(tick);
        }
        self.found = f
            .poll()
            .iter()
            .map(|h| LanRow {
                name: h.offer.name.clone(),
                addr: h.addr.to_string(),
                seats: format!("{} of {}", h.offer.seats_used, h.offer.seats),
                ok: h.offer.joinable(),
            })
            .collect();
    }

    /// Knock on `addr` (a port given, else this app's).
    pub fn start_join(&mut self, addr: &str, token: ClientToken, now: u64) {
        let addr = addr.trim();
        let with_port = if addr.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) {
            addr.to_owned()
        } else {
            format!("{addr}:{}", self.port)
        };
        self.cancel_join();
        match Session::join(&with_port, GuestConfig::new(token), None, now) {
            Ok(s) => {
                self.status = Some((format!("Knocking at {with_port}"), false));
                self.joining = Some(s);
            }
            Err(e) => self.status = Some((format!("Nobody answers at {with_port}: {e}"), true)),
        }
    }

    pub fn cancel_join(&mut self) {
        if let Some(mut s) = self.joining.take() {
            s.close();
        }
    }

    /// Moves a join along; the session once the host has welcomed it with the world.
    pub fn poll_join(&mut self, now: u64) -> Option<Session> {
        let s = self.joining.as_mut()?;
        s.poll(now);
        if s.sim().is_some() && s.seat().is_some() {
            self.status = None;
            return self.joining.take();
        }
        if let Session::Guest(g) = s {
            let st = match g.phase() {
                Phase::Hello => Some(("Knocking".to_owned(), false)),
                Phase::Building { seed } => Some((format!("Building the host's county (seed {seed})"), false)),
                Phase::Ready => Some(("Waiting for the host's world".to_owned(), false)),
                Phase::Playing => None,
                Phase::Refused(_) => g.refusal().map(|r| (format!("Refused: {r}"), true)),
                Phase::Ended(why) => Some((format!("The join ended: {why}"), true)),
            };
            if let Some(st) = st {
                let over = st.1;
                self.status = Some(st);
                if over {
                    self.joining = None;
                }
            }
        }
        None
    }

    /// The seats that sat down and got up since the last step, as toasts: "The teal coat sat
    /// down". The same on every machine, since it is read from the sim.
    pub fn seat_changes(&mut self, sim: &Sim, mut say: impl FnMut(String, bool)) {
        let now = sim.state().connected().fold(0u8, |m, p| m | 1 << p.seat.0);
        let was = std::mem::replace(&mut self.seats, now);
        if was == 0 {
            return;
        }
        for (i, name) in COAT_NAMES.iter().enumerate() {
            let bit = 1 << i;
            if now & bit != 0 && was & bit == 0 {
                say(format!("The {name} coat sat down"), true);
            } else if was & bit != 0 && now & bit == 0 {
                say(format!("The {name} coat got up"), false);
            }
        }
        let (n, m) = (now.count_ones(), was.count_ones());
        if n != m {
            say(
                match n {
                    1 => "Alone again: as strong as ever".to_owned(),
                    n => format!("{} at the table: each of us is weaker", ["", "", "Two", "Three", "Four"][n as usize]),
                },
                n < m,
            );
        }
    }
}

/// The table a Host choice asks for, named for the host's heroine.
pub fn host_config(name: &str, c: HostChoice) -> HostConfig {
    HostConfig {
        delay: c.delay,
        seats: c.seats,
        wait: c.wait,
        plays: true,
        name: format!("{name}'s world"),
        desync_dir: None,
        ..HostConfig::default()
    }
}

/// This machine's client token: `--token`, else the one in `config.json`, else a new one kept
/// there.
pub fn token(explicit: Option<u64>, config: &mut Config, dirs: &Dirs) -> ClientToken {
    if let Some(t) = explicit {
        return ClientToken(t.max(1));
    }
    if let Some(t) = config.client_token {
        return ClientToken(t.max(1));
    }
    let t = fresh_token();
    config.client_token = Some(t);
    let _ = config.save(dirs);
    ClientToken(t)
}

fn fresh_token() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    // splitmix64 over the clock and the process: distinct enough among four friends.
    let mut z = (t.as_nanos() as u64) ^ (u64::from(std::process::id()) << 32);
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seats_coming_and_going_are_said_by_coat() {
        let mut sim = Sim::new_game(3, "Jane");
        let mut lan = Lan::new(7777);
        let mut said = Vec::new();
        lan.seat_changes(&sim, |s, _| said.push(s));
        assert!(said.is_empty(), "the first look says nothing");
        let step = |sim: &mut Sim, seat, cmd| {
            let c = [jane_sim::StampedCommand { seat, seq: 0, cmd }];
            sim.step(&jane_sim::StepInput { commands: &c, ..jane_sim::StepInput::IDLE });
        };
        step(&mut sim, Some(jane_sim::Seat(0)), jane_sim::Command::Open(true));
        step(&mut sim, None, jane_sim::Command::Join { who: ClientToken(9) });
        lan.seat_changes(&sim, |s, _| said.push(s));
        assert_eq!(said, ["The teal coat sat down", "Two at the table: each of us is weaker"]);
        said.clear();
        step(&mut sim, Some(jane_sim::Seat(1)), jane_sim::Command::Leave);
        lan.seat_changes(&sim, |s, _| said.push(s));
        assert_eq!(said, ["The teal coat got up", "Alone again: as strong as ever"]);
    }
}
