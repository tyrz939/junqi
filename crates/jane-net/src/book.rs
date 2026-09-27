//! What every peer keeps of the recent past, to step the next frame and to explain a desync
//! (ARCHITECTURE.md §7): the bundles it stepped, its state hash at every hash point, and its
//! save at the last few, so that after a mismatch both sides can re-simulate from the point
//! before and say which step first differed, and what differs.

use std::collections::VecDeque;

use jane_sim::{Blueprints, Sim, StepInput, Stepped};

use crate::wire::{Bundle, HASH_EVERY};

/// Saves kept (the hash points `frame`, `frame - 60`, `frame - 120`): a mismatch is found about
/// a round trip after its hash point, so the point before it is still here.
const RING: usize = 3;
/// Own hashes kept, for guests' hashes that arrive late.
const HASHES: usize = 64;

/// Step one bundle.
pub fn step(sim: &mut Sim, b: &Bundle) -> Stepped {
    debug_assert_eq!(sim.state().frame, b.frame, "a bundle steps its own frame");
    sim.step(&StepInput { frames: b.frames, commands: &b.cmds })
}

#[derive(Debug, Default)]
pub struct Book {
    history: VecDeque<Bundle>,
    ring: VecDeque<(u32, Vec<u8>)>,
    hashes: VecDeque<(u32, u64)>,
}

impl Book {
    /// Forget everything: the sim was loaded afresh.
    pub fn reset(&mut self) {
        self.history.clear();
        self.ring.clear();
        self.hashes.clear();
    }

    /// `sim` has just stepped `b`. At a hash point, keep the save and the hash and return
    /// `(frame, hash)`.
    pub fn stepped(&mut self, sim: &Sim, b: Bundle) -> Option<(u32, u64)> {
        self.history.push_back(b);
        let frame = sim.state().frame;
        if frame % HASH_EVERY != 0 {
            return None;
        }
        let (save, h) = sim.save_and_hash();
        self.hashes.push_back((frame, h));
        if self.hashes.len() > HASHES {
            self.hashes.pop_front();
        }
        self.ring.push_back((frame, save));
        if self.ring.len() > RING {
            self.ring.pop_front();
        }
        // Bundles older than the oldest save can never be re-simulated.
        let oldest = self.ring.front().map_or(frame, |r| r.0);
        while self.history.front().is_some_and(|b| b.frame < oldest) {
            self.history.pop_front();
        }
        Some((frame, h))
    }

    /// The last hash point: `(frame, hash)`.
    pub fn last_hash(&self) -> Option<(u32, u64)> {
        self.hashes.back().copied()
    }

    pub fn hash_at(&self, frame: u32) -> Option<u64> {
        self.hashes.iter().find(|h| h.0 == frame).map(|h| h.1)
    }

    pub fn save_at(&self, frame: u32) -> Option<&[u8]> {
        self.ring.iter().find(|r| r.0 == frame).map(|r| r.1.as_slice())
    }

    /// Bundles from `frame` on (a guest that lacks them).
    pub fn bundles_from(&self, frame: u32) -> impl Iterator<Item = &Bundle> {
        self.history.iter().filter(move |b| b.frame >= frame)
    }

    /// The hash after each step from `from` (a hash point with a save) to `to`, re-simulated
    /// from the save at `from` through the bundles kept: `trail[i]` is the hash after stepping
    /// frame `from + i`. `None` without the save or the bundles.
    pub fn trail(&self, from: u32, to: u32, bps: &Blueprints) -> Option<Vec<u64>> {
        let save = self.save_at(from)?;
        let mut sim = Sim::from_snapshot_with(save, bps.clone()).ok()?;
        let mut out = Vec::with_capacity((to - from) as usize);
        for f in from..to {
            let b = self.history.iter().find(|b| b.frame == f)?;
            step(&mut sim, b);
            out.push(sim.hash());
        }
        Some(out)
    }
}
