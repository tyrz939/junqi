//! Save and hash (ARCHITECTURE.md §3.5, §3.6): one schema. The save is the postcard encoding of
//! [`GameState`] in an lz4 block; the hash is xxh3-64 of the same bytes, streamed into the
//! hasher without building them, so what is saved and what is hashed cannot drift.
//!
//! ```text
//! "JANE"  header_len: u32 le  header: postcard(Header)  body: lz4 block, size prepended
//! ```
//!
//! The header (version, content hash, build, a summary) reads without decoding the body.

use jane_core::{Milli, ZONE_COUNT, ZoneId};
use postcard::ser_flavors::Flavor;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::Xxh3Default;

use crate::blueprints::{Blueprints, BuildError};
use crate::sim::Sim;
use crate::state::{CombatState, GameState, SAVE_VERSION};
use crate::units::max_hp;

pub const MAGIC: [u8; 4] = *b"JANE";

/// What a save slot shows without loading the world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub zone: ZoneId,
    pub day: u32,
    pub hour: u8,
    pub hp: Milli,
    pub max_hp: Milli,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub save_version: u16,
    /// The content the save was made with: a different one is refused (no migrations yet).
    pub content_hash: u64,
    pub build: String,
    pub summary: Summary,
}

#[derive(Debug)]
pub enum SaveError {
    NotASave,
    Truncated,
    /// A save from another schema version (the migration chain is empty so far).
    Version(u16),
    /// A save made with other content (`--allow-content-drift` is a dev feature to come).
    ContentDrift {
        saved: u64,
        ours: u64,
    },
    Decode(postcard::Error),
    Decompress(String),
    Build(BuildError),
    /// The blueprints handed to `from_save_with` are for another seed.
    Seed {
        saved: u32,
        given: u32,
    },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NotASave => write!(f, "not a Jane save"),
            SaveError::Truncated => write!(f, "the save is cut short"),
            SaveError::Version(v) => write!(f, "save version {v}, this build reads {SAVE_VERSION}"),
            SaveError::ContentDrift { saved, ours } => {
                write!(f, "made with content {saved:016x}, this build has {ours:016x}")
            }
            SaveError::Decode(e) => write!(f, "cannot decode: {e}"),
            SaveError::Decompress(e) => write!(f, "cannot decompress: {e}"),
            SaveError::Build(e) => write!(f, "{e}"),
            SaveError::Seed { saved, given } => write!(f, "save is seed {saved}, blueprints are seed {given}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// A postcard flavor that feeds xxh3 through a small buffer instead of storing the bytes.
struct HashFlavor {
    h: Xxh3Default,
    buf: [u8; 256],
    n: usize,
}

impl HashFlavor {
    fn new() -> Self {
        Self { h: Xxh3Default::new(), buf: [0; 256], n: 0 }
    }

    fn flush(&mut self) {
        self.h.update(&self.buf[..self.n]);
        self.n = 0;
    }
}

impl Flavor for HashFlavor {
    type Output = u64;

    fn try_push(&mut self, b: u8) -> postcard::Result<()> {
        if self.n == self.buf.len() {
            self.flush();
        }
        self.buf[self.n] = b;
        self.n += 1;
        Ok(())
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        if self.n + data.len() > self.buf.len() {
            self.flush();
        }
        if data.len() >= self.buf.len() {
            self.h.update(data);
        } else {
            self.buf[self.n..self.n + data.len()].copy_from_slice(data);
            self.n += data.len();
        }
        Ok(())
    }

    fn finalize(mut self) -> postcard::Result<u64> {
        self.flush();
        Ok(self.h.digest())
    }
}

/// xxh3-64 of the postcard encoding of `v`.
pub fn hash_of<T: Serialize + ?Sized>(v: &T) -> u64 {
    postcard::serialize_with_flavor(v, HashFlavor::new()).expect("hashing cannot run out of room")
}

/// Read a save's header.
pub fn read_header(bytes: &[u8]) -> Result<(Header, &[u8]), SaveError> {
    if bytes.len() < 8 || bytes[..4] != MAGIC {
        return Err(SaveError::NotASave);
    }
    let n = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let rest = &bytes[8..];
    if rest.len() < n {
        return Err(SaveError::Truncated);
    }
    let header: Header = postcard::from_bytes(&rest[..n]).map_err(SaveError::Decode)?;
    Ok((header, &rest[n..]))
}

/// Decode a save's state, checking version and content.
pub fn decode_state(bytes: &[u8]) -> Result<GameState, SaveError> {
    let (header, body) = read_header(bytes)?;
    if header.save_version != SAVE_VERSION {
        return Err(SaveError::Version(header.save_version));
    }
    let ours = jane_data::catalog().content_hash;
    if header.content_hash != ours {
        return Err(SaveError::ContentDrift { saved: header.content_hash, ours });
    }
    let raw = lz4_flex::block::decompress_size_prepended(body).map_err(|e| SaveError::Decompress(e.to_string()))?;
    let state: GameState = postcard::from_bytes(&raw).map_err(SaveError::Decode)?;
    if state.version != SAVE_VERSION {
        return Err(SaveError::Version(state.version));
    }
    Ok(state)
}

impl Sim {
    /// The state hash: xxh3-64 of the save's encoding (§3.6).
    pub fn hash(&self) -> u64 {
        hash_of(&self.state)
    }

    /// Each zone's state hashed alone, for finding where two peers parted (§7 `Desync`).
    pub fn zone_hashes(&self) -> [u64; ZONE_COUNT] {
        std::array::from_fn(|i| hash_of(&self.state.zones[i]))
    }

    pub fn summary(&self) -> Summary {
        let s = &self.state;
        let host = s.players.first();
        let body = host.and_then(|p| s.zone(p.zone).and_then(|z| z.unit(p.unit)).or(p.parked.as_deref()));
        Summary {
            zone: host.map_or(ZoneId::County, |p| p.zone),
            day: s.day,
            hour: s.hour() as u8,
            hp: body.map_or(Milli::ZERO, |u| u.hp),
            max_hp: body.map_or(Milli::ZERO, max_hp),
        }
    }

    /// The bytes of a save. Slots are the app's and `jane serve`'s; the sim only encodes.
    pub fn save(&self) -> Vec<u8> {
        let header = Header {
            save_version: SAVE_VERSION,
            content_hash: jane_data::catalog().content_hash,
            build: env!("CARGO_PKG_VERSION").to_owned(),
            summary: self.summary(),
        };
        let head = postcard::to_allocvec(&header).expect("a header encodes");
        let body = postcard::to_allocvec(&self.state).expect("the state encodes");
        let packed = lz4_flex::block::compress_prepend_size(&body);
        let mut out = Vec::with_capacity(8 + head.len() + packed.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(head.len() as u32).to_le_bytes());
        out.extend_from_slice(&head);
        out.extend_from_slice(&packed);
        out
    }

    /// Load a save, building its seed's blueprints.
    pub fn from_save(bytes: &[u8]) -> Result<Sim, SaveError> {
        let state = decode_state(bytes)?;
        let bps = Blueprints::build(state.seed).map_err(SaveError::Build)?;
        Ok(Self::from_state(state, bps))
    }

    /// Load a save over blueprints already built for its seed.
    pub fn from_save_with(bytes: &[u8], bps: Blueprints) -> Result<Sim, SaveError> {
        let state = decode_state(bytes)?;
        if bps.seed() != state.seed {
            return Err(SaveError::Seed { saved: state.seed, given: bps.seed() });
        }
        Ok(Self::from_state(state, bps))
    }

    /// A save is the host's world (§3.5): it loads closed, every seat but the host's is parked
    /// (her body waits with what she owned), and the runtimes of live zones are rebuilt, the
    /// props' awake bits derived from the saved ring key. The ring itself does not run here: the
    /// saved key may lag the seats by a block (the ring runs before movement in a step), and the
    /// next step re-runs it exactly where the unbroken game would. No step.
    pub fn from_state(mut state: GameState, bps: Blueprints) -> Sim {
        state.open = false;
        for i in 1..state.players.len() {
            let (zone, unit) = {
                let p = &mut state.players[i];
                if !p.connected {
                    continue;
                }
                p.connected = false;
                p.dialogue = None;
                p.travel = None;
                (p.zone, p.unit)
            };
            let Some(zs) = state.zones[zone.index()].as_deref_mut() else { continue };
            let Some(body) = zs.remove_unit(unit) else { continue };
            for u in &mut zs.units {
                if u.target == Some(unit) {
                    u.target = None;
                    if u.combat == CombatState::Combat {
                        u.combat = CombatState::Leash;
                    }
                }
            }
            state.players[i].parked = Some(Box::new(body));
        }
        let mut sim = Sim::adopt(state, bps);
        for z in ZoneId::ALL {
            if sim.state.is_live(z) {
                sim.ensure_runtime(z);
            }
        }
        sim
    }
}
