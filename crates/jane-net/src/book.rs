//! What every peer keeps of the recent past, to step the next frame and to explain a desync
//! (ARCHITECTURE.md §7): the bundles it stepped, its state hash at every hash point, and its
//! save at the last few, so that after a mismatch both sides can re-simulate from the point
//! before and say which step first differed, and what differs.
//!
//! **Off the frame.** Encoding the state for the save and the hash is the dear part of a hash
//! point: 0.43 ms at the median on a desk in release for a four-seat county (0.7 ms at p99),
//! several on a Pi, once a second. A snapshot for a worker was measured and is dearer still (a
//! clone of the state is 1.6 ms: the name table's strings and the county's units). So where
//! there is a second core, a **replica** does it: a worker thread keeps its own copy of the sim,
//! loaded from the same save the peer's timeline began from, steps every bundle the peer steps,
//! and encodes each hash point there. The peer's thread only sends it the bundle. Determinism is
//! what makes the replica's world the peer's: same code, same machine, same bundles.
//!
//! What a replica cannot see is the peer's own world going wrong outside a step (a stray write,
//! a bad load). So every [`SPOT_EVERY`]th hash point is taken on the peer's own thread, from its
//! own world, as before: that is what a guest sends and the host compares there, and a world
//! that has drifted from its replica is found at the next one. The cost on the frame is one
//! encoding every ten seconds instead of every second. With one core everything is done on the
//! peer's thread.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};

use jane_sim::{Blueprints, Sim, StepInput, Stepped};

use crate::wire::{Bundle, HASH_EVERY};

/// Saves kept (the last six hash points): a mismatch is found a round trip after its hash point,
/// or later while a replica catches up, and the point before it must still be here.
const RING: usize = 6;
/// Own hashes kept, for guests' hashes that arrive late.
const HASHES: usize = 64;
/// Every this many hash points, the peer's own thread takes it from its own world.
pub const SPOT_EVERY: u32 = 10;

/// Step one bundle.
pub fn step(sim: &mut Sim, b: &Bundle) -> Stepped {
    debug_assert_eq!(sim.state().frame, b.frame, "a bundle steps its own frame");
    sim.step(&StepInput { frames: b.frames, commands: &b.cmds })
}

/// A hash point the peer's own thread takes.
pub fn is_spot(frame: u32) -> bool {
    frame % (HASH_EVERY * SPOT_EVERY) == 0
}

enum Job {
    /// Begin a timeline: the save it starts from, and its blueprints.
    Load {
        epoch: u32,
        save: Vec<u8>,
        bps: Blueprints,
    },
    Step(Bundle),
}

/// A hash point encoded: the epoch, the frame, the save and the hash.
type Done = (u32, u32, Vec<u8>, u64);

#[derive(Debug)]
struct Replica {
    tx: Sender<Job>,
    rx: Receiver<Done>,
    /// Hash points sent for and not yet come back.
    out: u32,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Job::Load { epoch, .. } => write!(f, "Load({epoch})"),
            Job::Step(b) => write!(f, "Step({})", b.frame),
        }
    }
}

impl Replica {
    fn spawn() -> Replica {
        let (tx, jobs) = channel::<Job>();
        let (done, rx) = channel::<Done>();
        std::thread::spawn(move || {
            let mut sim: Option<(u32, Sim)> = None;
            for job in jobs {
                match job {
                    Job::Load { epoch, save, bps } => {
                        sim = Sim::from_snapshot_with(&save, bps).ok().map(|s| (epoch, s));
                    }
                    Job::Step(b) => {
                        let Some((epoch, s)) = &mut sim else { continue };
                        if s.state().frame != b.frame {
                            continue;
                        }
                        step(s, &b);
                        let _ = s.drain_events();
                        let frame = s.state().frame;
                        if frame % HASH_EVERY == 0 && !is_spot(frame) {
                            let (save, h) = s.save_and_hash();
                            if done.send((*epoch, frame, save, h)).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        Replica { tx, rx, out: 0 }
    }
}

#[derive(Debug, Default)]
pub struct Book {
    history: VecDeque<Bundle>,
    ring: VecDeque<(u32, Vec<u8>)>,
    hashes: VecDeque<(u32, u64)>,
    replica: Option<Replica>,
    /// Bumped by each new timeline: what an older one's replica sends back is not kept.
    epoch: u32,
    /// Taken on this thread and not yet polled.
    late: Vec<(u32, u64)>,
}

impl Book {
    /// A timeline begins from `save` (the host's world when it opens, a guest's welcome): the
    /// past is forgotten and, with a second core, the replica loads the same world.
    pub fn start(&mut self, save: Vec<u8>, bps: Blueprints) {
        self.history.clear();
        self.ring.clear();
        self.hashes.clear();
        self.late.clear();
        self.epoch = self.epoch.wrapping_add(1);
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        if cores < 2 {
            self.replica = None;
            return;
        }
        let r = self.replica.get_or_insert_with(Replica::spawn);
        if r.tx.send(Job::Load { epoch: self.epoch, save, bps }).is_err() {
            self.replica = None;
        }
    }

    /// `sim` has just stepped `b`. At a hash point the save and the hash are taken: by the
    /// replica, or here at a spot point (or with no replica); they come back through
    /// [`poll`](Self::poll).
    pub fn stepped(&mut self, sim: &Sim, b: Bundle) {
        let frame = sim.state().frame;
        let point = frame % HASH_EVERY == 0;
        let mut here = point && (is_spot(frame) || self.replica.is_none());
        if let Some(r) = &mut self.replica {
            if r.tx.send(Job::Step(b.clone())).is_ok() {
                if point && !is_spot(frame) {
                    r.out += 1;
                }
            } else {
                self.replica = None;
                here = point;
            }
        }
        self.history.push_back(b);
        if here {
            let (save, h) = sim.save_and_hash();
            self.keep(frame, save, h);
            self.late.push((frame, h));
        }
    }

    /// The hash points taken since the last call: `(frame, hash)`, in frame order within a call
    /// (a spot point may come before the replica's that precede it).
    pub fn poll(&mut self) -> Vec<(u32, u64)> {
        let mut done = Vec::new();
        if let Some(r) = &mut self.replica {
            while let Ok(d) = r.rx.try_recv() {
                r.out = r.out.saturating_sub(u32::from(d.0 == self.epoch));
                done.push(d);
            }
        }
        self.take(done)
    }

    /// Wait for every hash point the replica owes (a desync's report needs the saves).
    pub fn settle(&mut self) -> Vec<(u32, u64)> {
        let mut done = Vec::new();
        if let Some(r) = &mut self.replica {
            while r.out > 0 {
                let Ok(d) = r.rx.recv() else { break };
                r.out = r.out.saturating_sub(u32::from(d.0 == self.epoch));
                done.push(d);
            }
            while let Ok(d) = r.rx.try_recv() {
                done.push(d);
            }
        }
        self.take(done)
    }

    fn take(&mut self, done: Vec<Done>) -> Vec<(u32, u64)> {
        let mut got = std::mem::take(&mut self.late);
        for (epoch, frame, save, h) in done {
            if epoch == self.epoch {
                self.keep(frame, save, h);
                got.push((frame, h));
            }
        }
        got.sort_by_key(|&(f, _)| f);
        got
    }

    fn keep(&mut self, frame: u32, save: Vec<u8>, h: u64) {
        let at = self.hashes.partition_point(|x| x.0 < frame);
        self.hashes.insert(at, (frame, h));
        if self.hashes.len() > HASHES {
            self.hashes.pop_front();
        }
        let at = self.ring.partition_point(|x| x.0 < frame);
        self.ring.insert(at, (frame, save));
        if self.ring.len() > RING {
            self.ring.pop_front();
        }
        // Bundles older than the oldest save can never be re-simulated.
        let oldest = self.ring.front().map_or(frame, |r| r.0);
        while self.history.front().is_some_and(|b| b.frame < oldest) {
            self.history.pop_front();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use jane_sim::InputFrame;
    use jane_sim::tuning::MAX_PLAYERS;

    #[test]
    fn the_replica_hashes_what_the_peer_would_and_spot_points_are_the_peers_own() {
        let bps = Blueprints::build(2).unwrap();
        let mut sim = Sim::new_game_with(bps.clone(), "Jane");
        let mut book = Book::default();
        book.start(sim.save(), bps);
        let mut got = Vec::new();
        let mut own = Vec::new();
        for f in 0..(HASH_EVERY * SPOT_EVERY + 130) {
            let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
            frames[0] = InputFrame::walk(jane_core::Angle((f * 97) as u16));
            let b = Bundle { frame: f, frames, cmds: Vec::new() };
            step(&mut sim, &b);
            book.stepped(&sim, b);
            if sim.state().frame % HASH_EVERY == 0 {
                own.push((sim.state().frame, sim.hash()));
            }
            got.extend(book.poll());
        }
        got.extend(book.settle());
        // A spot point may come before the replica's that precede it: order is not promised.
        got.sort_by_key(|&(f, _)| f);
        assert_eq!(got, own, "every hash point, the peer's own hash");
        assert!(book.save_at(own.last().unwrap().0).is_some());
        // A world changed outside a step is seen at the next spot point: the peer's own.
        let (spot, _) = *own.iter().find(|(f, _)| is_spot(*f)).unwrap();
        assert_eq!(spot, HASH_EVERY * SPOT_EVERY);
    }
}
