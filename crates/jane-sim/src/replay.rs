//! Replays (ARCHITECTURE.md §4.1, §7, §8; `sim/replay.ts`). A run is its seed, the heroine's
//! name and, per frame, the four seats' held input and the commands stamped for that frame. A
//! `Bundle` on the wire is one such frame, so a session log is a valid `.jrp`.
//!
//! ```text
//! "JRPL"  header_len: u32 le  header: postcard(ReplayHeader)  body: lz4 block of postcard(Body)
//! ```
//!
//! The header (version, content hash, save version, seed, name, build, hash period) reads
//! without decoding the body. The body is the frames as [`Run`]s: a run is one frame's input
//! held for `len` frames, its commands applied on the first of them. Held input barely
//! changes and a frozen stretch (she is reading, alone) is idle input, so a run-length entry
//! covers both. Beside the frames the body carries the hash stream: the state hash after every
//! step that brought the tick to a multiple of `hash_every`, and after the last frame. Playing
//! a tape back through a fresh [`Sim`] must land on every one of them ([`verify`]).
//!
//! A tape always begins at New Game: `Sim::new_game(seed, name)`, the host sat down, frame 0.
//! Everything after (the console's setup included) is a command in it.

use serde::{Deserialize, Serialize};

use crate::blueprints::Blueprints;
use crate::event::Event;
use crate::input::{InputFrame, StampedCommand, StepInput, Stepped};
use crate::sim::Sim;
use crate::state::SAVE_VERSION;
use crate::tuning::MAX_PLAYERS;
use crate::view::View;
use crate::{SaveError, Seat};

pub const MAGIC: [u8; 4] = *b"JRPL";
/// The tape's own layout. Bumped by any change to [`ReplayHeader`], [`Run`] or the body.
pub const REPLAY_VERSION: u16 = 1;
/// Ticks between hashes on a tape (PORT.md §9.3: a bot session hashes every 600 ticks).
pub const HASH_EVERY: u32 = 600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub version: u16,
    /// The content the tape was made with: a different one is refused, as a save is.
    pub content_hash: u64,
    /// The state schema its hashes were taken over.
    pub save_version: u16,
    pub seed: u32,
    /// The heroine's name, the host's choice: part of the world, so part of the tape.
    pub name: String,
    pub build: String,
    /// A hash is taken after every step that brings the tick to a multiple of this.
    pub hash_every: u32,
}

/// One frame's input, held for `len` frames (`len >= 1`); `commands` land on the first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The frame the run begins on (`GameState::frame` during that step).
    pub frame: u32,
    pub len: u32,
    pub frames: [InputFrame; MAX_PLAYERS],
    /// Sorted `(seat, seq)`.
    pub commands: Vec<StampedCommand>,
}

/// A state hash on the tape, taken after the step of `frame` (so `GameState::frame == frame + 1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashPoint {
    pub frame: u32,
    pub tick: u32,
    pub hash: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Body {
    runs: Vec<Run>,
    hashes: Vec<HashPoint>,
    frames: u32,
}

/// A decoded `.jrp`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tape {
    pub header: ReplayHeader,
    pub runs: Vec<Run>,
    /// In frame order; the last is the final state's.
    pub hashes: Vec<HashPoint>,
    /// Steps on the tape.
    pub frames: u32,
}

#[derive(Debug)]
pub enum ReplayError {
    NotATape,
    Truncated,
    /// A tape in another layout, or hashed over another state schema.
    Version {
        tape: u16,
        save: u16,
    },
    ContentDrift {
        tape: u64,
        ours: u64,
    },
    Decode(postcard::Error),
    Decompress(String),
    /// The runs do not follow one another frame by frame.
    Malformed(String),
    Build(SaveError),
    /// Re-simulated, the state hash after `frame` is not the one on the tape. `zones` is which
    /// zones' states differ from a second run's, when the caller asked (`diff`).
    Desync {
        frame: u32,
        tick: u32,
        want: u64,
        got: u64,
    },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::NotATape => write!(f, "not a Jane replay"),
            ReplayError::Truncated => write!(f, "the replay is cut short"),
            ReplayError::Version { tape, save } => write!(
                f,
                "replay version {tape} over save version {save}; this build writes {REPLAY_VERSION} over {SAVE_VERSION}"
            ),
            ReplayError::ContentDrift { tape, ours } => {
                write!(f, "recorded with content {tape:016x}, this build has {ours:016x}")
            }
            ReplayError::Decode(e) => write!(f, "cannot decode: {e}"),
            ReplayError::Decompress(e) => write!(f, "cannot decompress: {e}"),
            ReplayError::Malformed(e) => write!(f, "malformed: {e}"),
            ReplayError::Build(e) => write!(f, "{e}"),
            ReplayError::Desync { frame, tick, want, got } => {
                write!(f, "desync after frame {frame} (tick {tick}): the tape says {want:016x}, this build {got:016x}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl Tape {
    pub fn encode(&self) -> Vec<u8> {
        let head = postcard::to_allocvec(&self.header).expect("a header encodes");
        let body = Body { runs: self.runs.clone(), hashes: self.hashes.clone(), frames: self.frames };
        let raw = postcard::to_allocvec(&body).expect("a body encodes");
        let packed = lz4_flex::block::compress_prepend_size(&raw);
        let mut out = Vec::with_capacity(8 + head.len() + packed.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(head.len() as u32).to_le_bytes());
        out.extend_from_slice(&head);
        out.extend_from_slice(&packed);
        out
    }

    /// The header alone, without decoding the body or checking the content.
    pub fn read_header(bytes: &[u8]) -> Result<ReplayHeader, ReplayError> {
        split(bytes).and_then(|(h, _)| postcard::from_bytes(h).map_err(ReplayError::Decode))
    }

    /// Decode a tape, checking its version and content hash, and that its runs are whole.
    pub fn decode(bytes: &[u8]) -> Result<Tape, ReplayError> {
        let tape = Self::decode_unchecked(bytes)?;
        let ours = jane_data::catalog().content_hash;
        if tape.header.content_hash != ours {
            return Err(ReplayError::ContentDrift { tape: tape.header.content_hash, ours });
        }
        Ok(tape)
    }

    /// Decode without the content check (`jane replay diff` shows two tapes' headers whatever
    /// they were made with).
    pub fn decode_unchecked(bytes: &[u8]) -> Result<Tape, ReplayError> {
        let (head, body) = split(bytes)?;
        let header: ReplayHeader = postcard::from_bytes(head).map_err(ReplayError::Decode)?;
        if header.version != REPLAY_VERSION || header.save_version != SAVE_VERSION {
            return Err(ReplayError::Version { tape: header.version, save: header.save_version });
        }
        let raw =
            lz4_flex::block::decompress_size_prepended(body).map_err(|e| ReplayError::Decompress(e.to_string()))?;
        let body: Body = postcard::from_bytes(&raw).map_err(ReplayError::Decode)?;
        let mut next = 0u32;
        for r in &body.runs {
            if r.frame != next || r.len == 0 {
                return Err(ReplayError::Malformed(format!(
                    "a run at frame {} of {} after frame {next}",
                    r.frame, r.len
                )));
            }
            if !r.commands.is_sorted_by_key(|c| (c.seat, c.seq)) {
                return Err(ReplayError::Malformed(format!("frame {}'s commands are out of order", r.frame)));
            }
            next = r.frame.checked_add(r.len).ok_or_else(|| ReplayError::Malformed("too many frames".into()))?;
        }
        if next != body.frames {
            return Err(ReplayError::Malformed(format!("runs cover {next} frames, the tape says {}", body.frames)));
        }
        if header.hash_every == 0 {
            return Err(ReplayError::Malformed("hash_every is 0".into()));
        }
        Ok(Tape { header, runs: body.runs, hashes: body.hashes, frames: body.frames })
    }

    /// The last hash on the tape (the final state's).
    pub fn final_hash(&self) -> Option<u64> {
        self.hashes.last().map(|h| h.hash)
    }

    /// Frame `f`'s input: the run holding it, and whether `f` is its first frame (the one its
    /// commands land on).
    pub fn frame(&self, f: u32) -> Option<(&Run, bool)> {
        let i = self.runs.partition_point(|r| r.frame + r.len <= f);
        self.runs.get(i).filter(|r| r.frame <= f).map(|r| (r, r.frame == f))
    }

    /// A new game for this tape over blueprints already built for its seed.
    pub fn new_game_with(&self, bps: Blueprints) -> Sim {
        Sim::new_game_with(bps, &self.header.name)
    }

    /// Step `sim` through every frame, calling `each(sim, frame)` after each step. The sim must
    /// be this tape's new game.
    pub fn play(
        &self,
        sim: &mut Sim,
        mut each: impl FnMut(&mut Sim, u32) -> Result<(), ReplayError>,
    ) -> Result<(), ReplayError> {
        for r in &self.runs {
            for i in 0..r.len {
                let commands: &[StampedCommand] = if i == 0 { &r.commands } else { &[] };
                sim.step(&StepInput { frames: r.frames, commands });
                each(sim, r.frame + i)?;
            }
        }
        Ok(())
    }
}

fn split(bytes: &[u8]) -> Result<(&[u8], &[u8]), ReplayError> {
    if bytes.len() < 8 || bytes[..4] != MAGIC {
        return Err(ReplayError::NotATape);
    }
    let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let rest = &bytes[8..];
    if rest.len() < n {
        return Err(ReplayError::Truncated);
    }
    Ok(rest.split_at(n))
}

/// Records every step of a sim that began at New Game. It owns the sim, so nothing reaches the
/// state except through a step on the tape (tests that edit the state do not record).
#[derive(Debug)]
pub struct Recorder {
    sim: Sim,
    tape: Tape,
    /// The tick the last hash was taken at: frozen steps do not move the tick, and a tick is
    /// hashed once.
    last_hash_tick: Option<u32>,
}

impl Recorder {
    /// A new game of `seed`, recorded.
    pub fn new_game(seed: u32, name: &str) -> Recorder {
        Self::new(Sim::new_game(seed, name))
    }

    /// Record `sim`, which must be a new game not yet stepped (frame 0, only the host).
    pub fn new(sim: Sim) -> Recorder {
        Self::with_period(sim, HASH_EVERY)
    }

    /// The same with a hash every `every` ticks.
    pub fn with_period(sim: Sim, every: u32) -> Recorder {
        assert_eq!(sim.state().frame, 0, "a tape begins at New Game");
        assert!(every > 0);
        let s = sim.state();
        let header = ReplayHeader {
            version: REPLAY_VERSION,
            content_hash: jane_data::catalog().content_hash,
            save_version: SAVE_VERSION,
            seed: s.seed,
            name: s.name.clone(),
            build: env!("CARGO_PKG_VERSION").to_owned(),
            hash_every: every,
        };
        Recorder { sim, tape: Tape { header, runs: Vec::new(), hashes: Vec::new(), frames: 0 }, last_hash_tick: None }
    }

    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    pub fn view(&self, seat: Seat) -> Option<View<'_>> {
        self.sim.view(seat)
    }

    pub fn drain_events(&mut self) -> &[Event] {
        self.sim.drain_events()
    }

    /// The tape so far (without a final hash until [`finish`](Self::finish)).
    pub fn tape(&self) -> &Tape {
        &self.tape
    }

    /// Step the sim and record the frame.
    pub fn step(&mut self, input: &StepInput<'_>) -> Stepped {
        let frame = self.sim.state().frame;
        debug_assert_eq!(frame, self.tape.frames);
        let out = self.sim.step(input);
        match self.tape.runs.last_mut() {
            Some(r) if input.commands.is_empty() && r.frames == input.frames && r.frame + r.len == frame => {
                r.len += 1;
            }
            _ => self.tape.runs.push(Run { frame, len: 1, frames: input.frames, commands: input.commands.to_vec() }),
        }
        self.tape.frames = frame + 1;
        let tick = self.sim.state().tick.0;
        let every = self.tape.header.hash_every;
        if out.ran && tick % every == 0 && self.last_hash_tick != Some(tick) {
            self.last_hash_tick = Some(tick);
            self.tape.hashes.push(HashPoint { frame, tick, hash: self.sim.hash() });
        }
        out
    }

    /// The finished tape (with the final state's hash) and the sim.
    pub fn finish(mut self) -> (Sim, Tape) {
        if self.tape.frames > 0 && self.tape.hashes.last().is_none_or(|h| h.frame + 1 != self.tape.frames) {
            let tick = self.sim.state().tick.0;
            self.tape.hashes.push(HashPoint { frame: self.tape.frames - 1, tick, hash: self.sim.hash() });
        }
        (self.sim, self.tape)
    }
}

/// Re-simulate a tape from New Game (building its seed's blueprints). No hash is checked.
pub fn replay(bytes: &[u8]) -> Result<Sim, ReplayError> {
    let tape = Tape::decode(bytes)?;
    let bps = Blueprints::build(tape.header.seed).map_err(|e| ReplayError::Build(SaveError::Build(e)))?;
    let mut sim = tape.new_game_with(bps);
    tape.play(&mut sim, |_, _| Ok(()))?;
    Ok(sim)
}

/// What [`verify`] checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verified {
    pub frames: u32,
    pub ticks: u32,
    pub hashes: u32,
    pub final_hash: u64,
}

/// Re-simulate a tape and hold it to its hash stream; the first hash that differs is the error.
pub fn verify(bytes: &[u8]) -> Result<Verified, ReplayError> {
    let tape = Tape::decode(bytes)?;
    let bps = Blueprints::build(tape.header.seed).map_err(|e| ReplayError::Build(SaveError::Build(e)))?;
    verify_tape(&tape, bps)
}

/// [`verify`] over a decoded tape and blueprints already built for its seed.
pub fn verify_tape(tape: &Tape, bps: Blueprints) -> Result<Verified, ReplayError> {
    let mut sim = tape.new_game_with(bps);
    let mut next = 0;
    tape.play(&mut sim, |sim, frame| {
        while let Some(h) = tape.hashes.get(next).filter(|h| h.frame == frame) {
            let got = sim.hash();
            if got != h.hash {
                return Err(ReplayError::Desync { frame, tick: sim.state().tick.0, want: h.hash, got });
            }
            next += 1;
        }
        Ok(())
    })?;
    if next != tape.hashes.len() {
        return Err(ReplayError::Malformed(format!("{} hashes after the last frame", tape.hashes.len() - next)));
    }
    Ok(Verified { frames: tape.frames, ticks: sim.state().tick.0, hashes: next as u32, final_hash: sim.hash() })
}

/// Where two states part, for `jane replay diff`: every top-level part of the state whose
/// encoding differs, and inside each zone that differs, its parts and the first unit and prop
/// that differ (by id). Empty when the states are equal.
pub fn diff_states(a: &crate::state::GameState, b: &crate::state::GameState) -> Vec<String> {
    use crate::save::hash_of;
    let mut out = Vec::new();
    let mut part = |name: &str, x: u64, y: u64| {
        if x != y {
            out.push(name.to_owned());
        }
    };
    part("frame", u64::from(a.frame), u64::from(b.frame));
    part("tick", u64::from(a.tick.0), u64::from(b.tick.0));
    part("clock", hash_of(&(a.clock, a.day)), hash_of(&(b.clock, b.day)));
    part("counters", hash_of(&a.next), hash_of(&b.next));
    part("world rng", hash_of(&a.rng), hash_of(&b.rng));
    part("players", hash_of(&a.players), hash_of(&b.players));
    part("flags", hash_of(&a.flags), hash_of(&b.flags));
    part("quests", hash_of(&a.quests), hash_of(&b.quests));
    part("rest", hash_of(&a.rest), hash_of(&b.rest));
    part("growth", hash_of(&a.growth), hash_of(&b.growth));
    part("syms", hash_of(&a.syms), hash_of(&b.syms));
    part("journal", hash_of(&a.journal), hash_of(&b.journal));
    part(
        "living world",
        hash_of(&(a.weather, &a.consequences_done, &a.consequences_owed, &a.rumours)),
        hash_of(&(b.weather, &b.consequences_done, &b.consequences_owed, &b.rumours)),
    );
    for (i, (za, zb)) in a.zones.iter().zip(&b.zones).enumerate() {
        let z = jane_core::ZoneId::ALL[i].name();
        match (za.as_deref(), zb.as_deref()) {
            (Some(x), Some(y)) if x != y => {
                let mut zp = |name: &str, p: u64, q: u64| {
                    if p != q {
                        out.push(format!("{z}.{name}"));
                    }
                };
                zp("rng", hash_of(&x.rng), hash_of(&y.rng));
                zp("spawned", hash_of(&x.spawned), hash_of(&y.spawned));
                zp("drops", hash_of(&x.drops), hash_of(&y.drops));
                zp("projectiles", hash_of(&x.projectiles), hash_of(&y.projectiles));
                zp("grounds", hash_of(&x.grounds), hash_of(&y.grounds));
                zp("triggers", hash_of(&x.triggers), hash_of(&y.triggers));
                zp("tiles", hash_of(&x.tile_deltas), hash_of(&y.tile_deltas));
                zp("fog", hash_of(&x.fog), hash_of(&y.fog));
                zp("fills", hash_of(&x.pending_fill), hash_of(&y.pending_fill));
                zp("sleeping", hash_of(&x.sleeping_due), hash_of(&y.sleeping_due));
                zp("ring", hash_of(&x.ring_key), hash_of(&y.ring_key));
                zp("living", hash_of(&(x.wetness, &x.pressure)), hash_of(&(y.wetness, &y.pressure)));
                if let Some((u, v)) = x.units.iter().zip(&y.units).find(|(u, v)| u != v) {
                    out.push(format!("{z}.units: first differs at {:?} / {:?}", u.id, v.id));
                } else if x.units.len() != y.units.len() {
                    out.push(format!("{z}.units: {} / {}", x.units.len(), y.units.len()));
                }
                if let Some((p, q)) = x.props.iter().zip(&y.props).find(|(p, q)| p != q) {
                    out.push(format!("{z}.props: first differs at {:?} / {:?}", p.id, q.id));
                }
            }
            (Some(_), None) | (None, Some(_)) => out.push(format!("{z}: made in one only")),
            _ => {}
        }
    }
    out
}

/// Frame `f`'s input as a step takes it (`None` past the end).
pub fn input_at(tape: &Tape, f: u32) -> Option<([InputFrame; MAX_PLAYERS], &[StampedCommand])> {
    let (r, first) = tape.frame(f)?;
    Some((r.frames, if first { &r.commands[..] } else { &[] }))
}

#[cfg(test)]
mod tests {
    use jane_core::{Angle, Sfc32};

    use super::*;
    use crate::input::{Command, DevOp};

    fn bps() -> Blueprints {
        static B: std::sync::OnceLock<Blueprints> = std::sync::OnceLock::new();
        B.get_or_init(|| Blueprints::build(3).expect("seed 3 builds")).clone()
    }

    /// A hand's input: the stick changes twice a second, a bar press now and then, USE, the
    /// console, and a stretch standing still.
    fn record(frames: u32) -> (Sim, Tape) {
        let mut rec = Recorder::with_period(Sim::new_game_with(bps(), "Tess"), 120);
        let mut rng = Sfc32::seeded(5, 5);
        let mut held = InputFrame::IDLE;
        let mut seq = 0u16;
        for f in 0..frames {
            if f % 30 == 0 && !(300..600).contains(&f) {
                held = InputFrame {
                    mv_dir: Angle(rng.next_u32() as u16),
                    mv_mag: 127,
                    aim: Some(Angle(rng.next_u32() as u16)),
                    sprint: rng.below(3) == 0,
                    ..InputFrame::IDLE
                };
            }
            if (300..600).contains(&f) {
                held = InputFrame::IDLE;
            }
            let mut cmds = Vec::new();
            let mut push = |c: Command| {
                seq = seq.wrapping_add(1);
                cmds.push(StampedCommand { seat: Some(Seat(0)), seq, cmd: c });
            };
            if f % 40 == 0 {
                push(Command::Bar { slot: 0, on: None });
            }
            if f % 170 == 0 {
                push(Command::Use);
            }
            if f == 650 {
                let apple = jane_data::catalog().combat.item_id("apple").unwrap();
                push(Command::Dev(DevOp::Give { item: apple, qty: 3 }));
            }
            if rec.sim().state().players[0].dialogue.is_some() {
                push(Command::CloseDialogue);
            }
            let mut frames = [InputFrame::IDLE; MAX_PLAYERS];
            frames[0] = held;
            rec.step(&StepInput { frames, commands: &cmds });
        }
        rec.finish()
    }

    #[test]
    fn a_tape_round_trips_and_re_simulates_to_every_hash() {
        let (sim, tape) = record(1500);
        assert!(tape.runs.len() < 1500 / 2, "run-length encoding did something: {} runs", tape.runs.len());
        assert_eq!(tape.frames, 1500);
        assert_eq!(tape.final_hash(), Some(sim.hash()));
        assert!(tape.hashes.len() >= 1500 / 120, "{}", tape.hashes.len());
        let bytes = tape.encode();
        let back = Tape::decode(&bytes).unwrap();
        assert_eq!(back, tape);
        let v = verify_tape(&back, bps()).unwrap();
        assert_eq!(v.final_hash, sim.hash());
        assert_eq!(v.frames, 1500);
        assert_eq!(v.hashes as usize, tape.hashes.len());
        assert_eq!(Tape::read_header(&bytes).unwrap().name, "Tess");
    }

    #[test]
    fn a_changed_input_is_a_desync_at_the_next_hash() {
        let (_, mut tape) = record(900);
        // Push the stick of one frame in the middle of a walk the other way.
        let i = tape.runs.iter().position(|r| r.frame >= 700 && r.frames[0].mv_mag > 0).unwrap();
        tape.runs[i].frames[0].mv_dir = Angle(tape.runs[i].frames[0].mv_dir.0.wrapping_add(0x8000));
        let at = tape.runs[i].frame;
        match verify_tape(&tape, bps()) {
            Err(ReplayError::Desync { frame, .. }) => assert!(frame >= at, "{frame} before {at}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_tape_from_other_content_or_cut_short_is_refused() {
        let (_, mut tape) = record(10);
        let bytes = tape.encode();
        assert!(matches!(
            Tape::decode(&bytes[..bytes.len() - 3]),
            Err(ReplayError::Decompress(_) | ReplayError::Decode(_))
        ));
        assert!(matches!(Tape::decode(b"JANE0000"), Err(ReplayError::NotATape)));
        tape.header.content_hash ^= 1;
        assert!(matches!(Tape::decode(&tape.encode()), Err(ReplayError::ContentDrift { .. })));
        assert!(Tape::decode_unchecked(&tape.encode()).is_ok());
    }

    #[test]
    fn frame_finds_its_run() {
        let (_, tape) = record(700);
        for f in [0, 1, 299, 300, 301, 599, 600, 699] {
            let (r, first) = tape.frame(f).unwrap();
            assert!(r.frame <= f && f < r.frame + r.len);
            assert_eq!(first, r.frame == f);
        }
        assert!(tape.frame(700).is_none());
    }
}
