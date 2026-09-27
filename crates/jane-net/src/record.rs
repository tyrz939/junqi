//! A session is a tape (ARCHITECTURE.md §7: a `Bundle` is a replay frame): the host records
//! every bundle it steps as a `.jrp` run, with the tape's own hash stream, so a whole LAN
//! session can be re-simulated offline (`jane replay verify`) and held to the hashes the host
//! saw. A tape begins at New Game, so only a session hosted from frame 0 is recorded.

use jane_sim::replay::{HASH_EVERY, HashPoint, REPLAY_VERSION, ReplayHeader, Run, Tape};
use jane_sim::state::SAVE_VERSION;
use jane_sim::{Sim, Stepped};

use crate::wire::Bundle;

#[derive(Debug)]
pub struct SessionTape {
    tape: Tape,
    last_hash_tick: Option<u32>,
}

impl SessionTape {
    /// Start recording `sim`, which must be a new game not yet stepped; `None` otherwise.
    pub fn new(sim: &Sim) -> Option<SessionTape> {
        let s = sim.state();
        if s.frame != 0 {
            return None;
        }
        let header = ReplayHeader {
            version: REPLAY_VERSION,
            content_hash: jane_data::catalog().content_hash,
            save_version: SAVE_VERSION,
            seed: s.seed,
            name: s.name.clone(),
            build: crate::wire::BUILD.to_owned(),
            hash_every: HASH_EVERY,
        };
        Some(SessionTape {
            tape: Tape { header, runs: Vec::new(), hashes: Vec::new(), frames: 0 },
            last_hash_tick: None,
        })
    }

    /// `sim` has just stepped `b` (what `Recorder::step` records, from a bundle).
    pub fn stepped(&mut self, b: &Bundle, out: Stepped, sim: &Sim) {
        let frame = b.frame;
        debug_assert_eq!(frame, self.tape.frames);
        match self.tape.runs.last_mut() {
            Some(r) if b.cmds.is_empty() && r.frames == b.frames && r.frame + r.len == frame => r.len += 1,
            _ => self.tape.runs.push(Run { frame, len: 1, frames: b.frames, commands: b.cmds.clone() }),
        }
        self.tape.frames = frame + 1;
        let tick = sim.state().tick.0;
        let every = self.tape.header.hash_every;
        if out.ran && tick % every == 0 && self.last_hash_tick != Some(tick) {
            self.last_hash_tick = Some(tick);
            self.tape.hashes.push(HashPoint { frame, tick, hash: sim.hash() });
        }
    }

    /// The tape, with the final state's hash.
    pub fn finish(mut self, sim: &Sim) -> Tape {
        if self.tape.frames > 0 && self.tape.hashes.last().is_none_or(|h| h.frame + 1 != self.tape.frames) {
            let tick = sim.state().tick.0;
            self.tape.hashes.push(HashPoint { frame: self.tape.frames - 1, tick, hash: sim.hash() });
        }
        self.tape
    }
}
