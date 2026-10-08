//! Save and hash (ARCHITECTURE.md §3.5, §3.6): one schema. The save is the postcard encoding of
//! [`Form`], the state as it differs from what its seed would build, in an lz4 block; the hash is
//! xxh3-64 of the same bytes, streamed into the hasher without building them, so what is saved
//! and what is hashed cannot drift.
//!
//! ```text
//! "JANE"  header_len: u32 le  header: postcard(Header)  body: lz4 block, size prepended
//! ```
//!
//! The header (version, content hash, build, a summary) reads without decoding the body.
//!
//! The form is canonical: a function of the state and the seed's blueprints alone, so a state
//! reached by playing and the same state reached by loading encode to the same bytes, and
//! decoding gives the state back field for field ([`Form::of`], [`Form::into_state`]).

use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use jane_core::{
    Blueprint, CellIx, ConsequenceId, Key, Milli, NameId, Sfc32, StoryId, Sym, Tick, Tile, ZONE_COUNT, ZoneId,
};
use postcard::ser_flavors::Flavor;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::Xxh3Default;

use alloc::sync::Arc;

use crate::blueprints::{Blueprints, BuildError, SpawnDigest};
use crate::ids::{Counters, PropId, UnitId};
use crate::sim::Sim;
use crate::state::{
    Bits, CombatState, Drop, Fill, FlagKey, GameState, Ground, Growth, Journal, PlayerState, Projectile, Prop, Quests,
    REGIONS, RestPoint, RingKey, SAVE_VERSION, SpawnBase, Store, TriggerBits, Unit, WeatherState, ZoneState,
};
use crate::sym::{SymTable, of_name};
use crate::units::max_hp;
use crate::zone::{spawn_prop, spawn_unit};

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
    /// The body decoded but does not describe a state (spawns out of order, a name missing).
    Malformed(&'static str),
}

impl core::fmt::Display for SaveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
            SaveError::Malformed(why) => write!(f, "malformed: {why}"),
        }
    }
}

impl core::error::Error for SaveError {}

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

/// Decode a save's form, checking version and content.
fn decode_form(bytes: &[u8]) -> Result<Form<'static>, SaveError> {
    let (header, body) = read_header(bytes)?;
    if header.save_version != SAVE_VERSION {
        return Err(SaveError::Version(header.save_version));
    }
    let ours = jane_data::catalog().content_hash;
    if header.content_hash != ours {
        return Err(SaveError::ContentDrift { saved: header.content_hash, ours });
    }
    let raw = lz4_flex::block::decompress_size_prepended(body).map_err(|e| SaveError::Decompress(e.to_string()))?;
    let form: Form<'static> = postcard::from_bytes(&raw).map_err(SaveError::Decode)?;
    if form.version != SAVE_VERSION {
        return Err(SaveError::Version(form.version));
    }
    Ok(form)
}

/// Decode a save's state over its seed's blueprints, checking version, content and seed.
pub fn decode_state(bytes: &[u8], bps: &Blueprints) -> Result<GameState, SaveError> {
    let form = decode_form(bytes)?;
    if bps.seed() != form.seed {
        return Err(SaveError::Seed { saved: form.seed, given: bps.seed() });
    }
    form.into_state(bps)
}

/// [`decode_state`] over an on-demand set that holds every zone the save names afterwards (the
/// sim lets go of those nobody is in once loaded).
fn decode_state_holding(bytes: &[u8], bps: &mut Blueprints) -> Result<GameState, SaveError> {
    let form = decode_form(bytes)?;
    if bps.seed() != form.seed {
        return Err(SaveError::Seed { saved: form.seed, given: bps.seed() });
    }
    form.into_state_with(&mut Source::Hold(bps))
}

impl Sim {
    /// The state hash: xxh3-64 of the save's encoding (§3.6).
    pub fn hash(&self) -> u64 {
        hash_of(&Form::of(&self.state, &self.bps))
    }

    /// Each zone's state hashed alone (its part of the form), for finding where two peers
    /// parted (§7 `Desync`).
    pub fn zone_hashes(&self) -> [u64; ZONE_COUNT] {
        let s = &self.state;
        core::array::from_fn(|i| hash_of(&s.zones[i].as_deref().map(|z| ZoneForm::of_in(z, &self.bps, &s.syms))))
    }

    pub fn summary(&self) -> Summary {
        summary_of(&self.state)
    }

    /// The bytes of a save. Slots are the app's and `jane serve`'s; the sim only encodes.
    pub fn save(&self) -> Vec<u8> {
        let body = postcard::to_allocvec(&Form::of(&self.state, &self.bps)).expect("the state encodes");
        self.save_of(&body)
    }

    /// The save's bytes and the state hash (`hash()`) from one encoding of the form: a lockstep
    /// peer takes both at every hash point (ARCHITECTURE.md §7), and the hash of the body's
    /// bytes is the hash streamed.
    pub fn save_and_hash(&self) -> (Vec<u8>, u64) {
        save_and_hash_of(&self.state, &self.bps)
    }

    /// The state and its blueprints, owned: [`Snapshot::save_and_hash`] can run anywhere, a
    /// worker thread included, while the sim steps on (ARCHITECTURE.md §7: a peer's hash point
    /// kept off the frame). Cloning the state is a fraction of encoding it.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { state: self.state.clone(), bps: self.bps.clone() }
    }

    fn save_of(&self, body: &[u8]) -> Vec<u8> {
        save_bytes(&self.state, body)
    }
}

/// A state and its seed's blueprints, as [`Sim::snapshot`] took them.
#[derive(Clone, Debug)]
pub struct Snapshot {
    state: GameState,
    bps: Blueprints,
}

impl Snapshot {
    /// The same bytes and hash `Sim::save_and_hash` gives for the sim this was taken from.
    pub fn save_and_hash(&self) -> (Vec<u8>, u64) {
        save_and_hash_of(&self.state, &self.bps)
    }

    /// The frame count it was taken at.
    pub fn frame(&self) -> u32 {
        self.state.frame
    }
}

fn summary_of(s: &GameState) -> Summary {
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

fn save_and_hash_of(state: &GameState, bps: &Blueprints) -> (Vec<u8>, u64) {
    let body = postcard::to_allocvec(&Form::of(state, bps)).expect("the state encodes");
    (save_bytes(state, &body), xxhash_rust::xxh3::xxh3_64(&body))
}

fn save_bytes(state: &GameState, body: &[u8]) -> Vec<u8> {
    let header = Header {
        save_version: SAVE_VERSION,
        content_hash: jane_data::catalog().content_hash,
        build: env!("CARGO_PKG_VERSION").to_owned(),
        summary: summary_of(state),
    };
    let head = postcard::to_allocvec(&header).expect("a header encodes");
    let packed = lz4_flex::block::compress_prepend_size(body);
    let mut out = Vec::with_capacity(8 + head.len() + packed.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&(head.len() as u32).to_le_bytes());
    out.extend_from_slice(&head);
    out.extend_from_slice(&packed);
    out
}

impl Sim {
    /// Load a save, building its seed's blueprints.
    pub fn from_save(bytes: &[u8]) -> Result<Sim, SaveError> {
        let form = decode_form(bytes)?;
        let mut bps = Blueprints::build(form.seed).map_err(SaveError::Build)?;
        let state = form.into_state_with(&mut Source::Hold(&mut bps))?;
        Ok(Self::from_state(state, bps))
    }

    /// Load a save over blueprints already built for its seed.
    pub fn from_save_with(bytes: &[u8], mut bps: Blueprints) -> Result<Sim, SaveError> {
        let state = decode_state_holding(bytes, &mut bps)?;
        Ok(Self::from_state(state, bps))
    }

    /// A lockstep snapshot (ARCHITECTURE.md §7): the save's bytes of a live world, loaded as the
    /// world it was, over blueprints already built for its seed. Unlike [`from_save`](Self::from_save),
    /// which loads the host's world (closed, every guest parked), nothing is changed: every seat
    /// stays where it sat and the world as open as it was, so the joiner's sim is the host's at
    /// that frame (`decode(save(s)) == s`; its hash is the host's). The runtimes of live zones are
    /// rebuilt, which no later step can see (§8 `runtime_rebuild_is_invisible`).
    pub fn from_snapshot_with(bytes: &[u8], mut bps: Blueprints) -> Result<Sim, SaveError> {
        let state = decode_state_holding(bytes, &mut bps)?;
        let mut sim = Sim::adopt(state, bps);
        for z in ZoneId::ALL {
            if sim.state.is_live(z) {
                sim.ensure_runtime(z);
            }
        }
        sim.release_blueprints();
        Ok(sim)
    }

    /// A save is the host's world (§3.5): it loads closed, every seat but the host's is parked
    /// (her body waits with what she owned), and the runtimes of live zones are rebuilt, the
    /// props' awake bits derived from the saved ring key. The ring itself does not run here: the
    /// saved key may lag the seats by a block (the ring runs before movement in a step), and the
    /// next step re-runs it exactly where the unbroken game would. No step.
    pub fn from_state(mut state: GameState, bps: Blueprints) -> Sim {
        state.open = false;
        state.table_delay = 0;
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
        sim.release_blueprints();
        sim
    }
}

// --- the form ----------------------------------------------------------------------------

/// The one encoding of a [`GameState`], for the save and the hash alike (§3.5, §3.6). It is the
/// state's fields in the state's order, but for two things the seed rebuilds:
///
/// - a zone's units and props ([`Spawns`]): a spawn still as its blueprint row first made it is
///   not stored; one changed is stored whole; one gone is a tombstone (its row's index); one
///   made at runtime, or one that walked in from elsewhere, is stored whole;
/// - the name tail ([`SymRun`]): the names a blueprint's `local_names` appended when its zone was
///   made are stored as that zone; a name interned at runtime is stored as itself.
///
/// Serializing borrows the state ([`Form::of`]); deserializing owns ([`Form::into_state`]). Both
/// destructure the state whole, so a field added to it cannot be left out of the save.
#[derive(Debug, Serialize, Deserialize)]
pub struct Form<'a> {
    version: u16,
    seed: u32,
    frame: u32,
    tick: Tick,
    clock: u32,
    day: u32,
    name: Cow<'a, str>,
    open: bool,
    table_delay: u8,
    next: Counters,
    rng: Sfc32,
    players: Cow<'a, [PlayerState]>,
    zones: [Option<ZoneForm<'a>>; ZONE_COUNT],
    flags: Cow<'a, BTreeMap<FlagKey, i32>>,
    quests: Cow<'a, Quests>,
    rest: Option<RestPoint>,
    growth: Cow<'a, Growth>,
    syms: Vec<SymRun<'a>>,
    journal: Cow<'a, Journal>,
    weather: [WeatherState; REGIONS],
    consequences_done: Cow<'a, Bits>,
    consequences_owed: Cow<'a, [(ZoneId, ConsequenceId)]>,
    rumours: Cow<'a, BTreeMap<(NameId, StoryId), Tick>>,
    stores: Cow<'a, BTreeMap<(ZoneId, PropId), Store>>,
    fires_made: bool,
}

/// A zone's part of the [`Form`]: [`ZoneState`]'s fields in its order, units and props as
/// [`Spawns`].
#[derive(Debug, Serialize, Deserialize)]
pub struct ZoneForm<'a> {
    id: ZoneId,
    rng: Sfc32,
    spawned: SpawnBase,
    units: Spawns<'a, Unit>,
    props: Spawns<'a, Prop>,
    drops: Cow<'a, [Drop]>,
    projectiles: Cow<'a, [Projectile]>,
    grounds: Cow<'a, [Ground]>,
    triggers: Cow<'a, TriggerBits>,
    tile_deltas: Cow<'a, BTreeMap<CellIx, Tile>>,
    fog: Cow<'a, [u32]>,
    pending_fill: Cow<'a, [Fill]>,
    sleeping_due: Cow<'a, [(Tick, UnitId)]>,
    wetness: [u8; REGIONS],
    pressure: Cow<'a, [u16]>,
    ring_key: RingKey,
}

/// A zone's units (or props) against its blueprint's rows: `removed` are the rows (ascending)
/// whose spawn is no longer in the zone; `stored` is every instance, in id order, that is not
/// its row exactly as first made. Every other row is rebuilt by `zone::spawn_unit` or
/// `zone::spawn_prop` with the id and tick of [`SpawnBase`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
pub struct Spawns<'a, T: Clone> {
    removed: Vec<u32>,
    stored: Vec<Cow<'a, T>>,
}

/// A run of the name tail: what interning a zone's `local_names` appended, or one name.
#[derive(Debug, Serialize, Deserialize)]
pub enum SymRun<'a> {
    Zone(ZoneId),
    Name(Cow<'a, str>),
}

impl<'a> Form<'a> {
    /// The state's form over its seed's blueprints.
    pub fn of(s: &'a GameState, bps: &'a Blueprints) -> Form<'a> {
        let GameState {
            version,
            seed,
            frame,
            tick,
            clock,
            day,
            name,
            open,
            table_delay,
            next,
            rng,
            players,
            zones,
            flags,
            quests,
            rest,
            growth,
            syms,
            journal,
            weather,
            consequences_done,
            consequences_owed,
            rumours,
            stores,
            fires_made,
        } = s;
        Form {
            version: *version,
            seed: *seed,
            frame: *frame,
            tick: *tick,
            clock: *clock,
            day: *day,
            name: Cow::Borrowed(name),
            open: *open,
            table_delay: *table_delay,
            next: *next,
            rng: *rng,
            players: Cow::Borrowed(players),
            zones: core::array::from_fn(|i| zones[i].as_deref().map(|z| ZoneForm::of_in(z, bps, syms))),
            flags: Cow::Borrowed(flags),
            quests: Cow::Borrowed(quests),
            rest: *rest,
            growth: Cow::Borrowed(growth),
            syms: sym_runs(syms, bps),
            journal: Cow::Borrowed(journal),
            weather: *weather,
            consequences_done: Cow::Borrowed(consequences_done),
            consequences_owed: Cow::Borrowed(consequences_owed),
            rumours: Cow::Borrowed(rumours),
            stores: Cow::Borrowed(stores),
            fires_made: *fires_made,
        }
    }

    /// The state back, over the same seed's blueprints (a zone not held is built for the call
    /// and not kept).
    pub fn into_state(self, bps: &Blueprints) -> Result<GameState, SaveError> {
        self.into_state_with(&mut Source::Fetch(bps))
    }

    fn into_state_with(self, bps: &mut Source<'_>) -> Result<GameState, SaveError> {
        let Form {
            version,
            seed,
            frame,
            tick,
            clock,
            day,
            name,
            open,
            table_delay,
            next,
            rng,
            players,
            zones,
            flags,
            quests,
            rest,
            growth,
            syms,
            journal,
            weather,
            consequences_done,
            consequences_owed,
            rumours,
            stores,
            fires_made,
        } = self;
        let mut table = SymTable::default();
        for run in syms {
            match run {
                SymRun::Zone(z) => {
                    table.intern_all(&bps.names(z));
                }
                SymRun::Name(n) => {
                    let len = table.len();
                    if table.intern(&n).0 != len {
                        return Err(SaveError::Malformed("a name saved twice"));
                    }
                }
            }
        }
        let syms = table;
        let mut out: [Option<Box<ZoneState>>; ZONE_COUNT] = core::array::from_fn(|_| None);
        for (i, z) in zones.into_iter().enumerate() {
            if let Some(z) = z {
                out[i] = Some(Box::new(z.into_zone(&bps.blueprint(ZoneId::ALL[i]), &syms)?));
            }
        }
        Ok(GameState {
            version,
            seed,
            frame,
            tick,
            clock,
            day,
            name: name.into_owned(),
            open,
            table_delay,
            next,
            rng,
            players: players.into_owned(),
            zones: out,
            flags: flags.into_owned(),
            quests: quests.into_owned(),
            rest,
            growth: growth.into_owned(),
            syms,
            journal: journal.into_owned(),
            weather,
            consequences_done: consequences_done.into_owned(),
            consequences_owed: consequences_owed.into_owned(),
            rumours: rumours.into_owned(),
            stores: stores.into_owned(),
            fires_made,
        })
    }
}

/// Where [`Form::into_state`] finds a zone's blueprint: built for the call and not kept, or held
/// in an on-demand set (which the sim lets go of again once loaded).
enum Source<'b> {
    Fetch(&'b Blueprints),
    Hold(&'b mut Blueprints),
}

impl Source<'_> {
    fn blueprint(&mut self, z: ZoneId) -> Arc<Blueprint> {
        match self {
            Source::Fetch(b) => b.fetch(z),
            Source::Hold(b) => Arc::clone(b.ensure(z)),
        }
    }

    fn names(&mut self, z: ZoneId) -> jane_core::Names {
        let known = match self {
            Source::Fetch(b) => b.names(z).cloned(),
            Source::Hold(b) => b.names(z).cloned(),
        };
        known.unwrap_or_else(|| self.blueprint(z).local_names.clone())
    }
}

/// Zone `bp.zone`'s spawn rows as its state first made them for `base`
/// (`zone::create_zone_state`), each hashed ([`SpawnDigest`]): what [`ZoneForm`] compares an
/// instance with when the zone's blueprint is not held. The zone's names must be interned (they
/// are, once it has a state).
pub(crate) fn spawn_digest(base: SpawnBase, bp: &Blueprint, syms: &SymTable) -> SpawnDigest {
    let locals = crate::runtime::find_locals(syms, bp);
    let key = |k: Key| crate::sym::of_key(k, &locals);
    let units = bp
        .units
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let id = UnitId::new(base.unit + 1 + i as u32).expect("a spawn id is past its base");
            hash_of(&spawn_unit(row, id, key(row.key), base.tick))
        })
        .collect();
    let props = bp
        .props
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let id = PropId::new(base.prop + 1 + i as u32).expect("a spawn id is past its base");
            hash_of(&spawn_prop(row, i as u16, id, key(row.key)))
        })
        .collect();
    SpawnDigest { base, units, props }
}

impl<'a> ZoneForm<'a> {
    /// A zone's form over the seed's blueprints: over its own if held, else over its digest (an
    /// on-demand set lets go of a zone nobody is in; PORT.md §13.3). The same form either way: an
    /// instance is its row exactly when its encoding hashes as the row's first instance did.
    pub fn of_in(z: &'a ZoneState, bps: &Blueprints, syms: &SymTable) -> ZoneForm<'a> {
        if let Some(bp) = bps.held_now(z.id) {
            return ZoneForm::of(z, bp, syms);
        }
        let d = bps
            .spawns(z.id)
            .unwrap_or_else(|| panic!("zone {} has a state but neither a blueprint nor a digest", z.id.name()));
        assert_eq!(d.base, z.spawned, "zone {}'s spawn digest is for another base", z.id.name());
        ZoneForm::with_spawns(
            z,
            Spawns::of(
                &z.units,
                |u| u.id.get(),
                z.spawned.unit,
                d.units.len() as u32,
                |u, row| hash_of(u) == d.units[row as usize],
            ),
            Spawns::of(
                &z.props,
                |p| p.id.get(),
                z.spawned.prop,
                d.props.len() as u32,
                |p, row| hash_of(p) == d.props[row as usize],
            ),
        )
    }

    /// A zone's form over its blueprint and the world's names.
    pub fn of(z: &'a ZoneState, bp: &Blueprint, syms: &SymTable) -> ZoneForm<'a> {
        let at = z.spawned.tick;
        ZoneForm::with_spawns(
            z,
            Spawns::of(
                &z.units,
                |u| u.id.get(),
                z.spawned.unit,
                bp.units.len() as u32,
                |u, i| {
                    let row = &bp.units[i as usize];
                    u.key.is_some_and(|k| key_is(syms, bp, k, row.key) && *u == spawn_unit(row, u.id, k, at))
                },
            ),
            Spawns::of(
                &z.props,
                |p| p.id.get(),
                z.spawned.prop,
                bp.props.len() as u32,
                |p, i| {
                    let row = &bp.props[i as usize];
                    key_is(syms, bp, p.key, row.key) && *p == spawn_prop(row, i as u16, p.id, p.key)
                },
            ),
        )
    }

    /// The zone's fields in order, its spawns as given (made over its `units` and `props`).
    fn with_spawns(z: &'a ZoneState, units: Spawns<'a, Unit>, props: Spawns<'a, Prop>) -> ZoneForm<'a> {
        let ZoneState {
            id,
            rng,
            spawned,
            units: _,
            props: _,
            drops,
            projectiles,
            grounds,
            triggers,
            tile_deltas,
            fog,
            pending_fill,
            sleeping_due,
            wetness,
            pressure,
            ring_key,
        } = z;
        ZoneForm {
            id: *id,
            rng: *rng,
            spawned: *spawned,
            units,
            props,
            drops: Cow::Borrowed(drops),
            projectiles: Cow::Borrowed(projectiles),
            grounds: Cow::Borrowed(grounds),
            triggers: Cow::Borrowed(triggers),
            tile_deltas: Cow::Borrowed(tile_deltas),
            fog: Cow::Borrowed(fog),
            pending_fill: Cow::Borrowed(pending_fill),
            sleeping_due: Cow::Borrowed(sleeping_due),
            wetness: *wetness,
            pressure: Cow::Borrowed(pressure),
            ring_key: *ring_key,
        }
    }

    fn into_zone(self, bp: &Blueprint, syms: &SymTable) -> Result<ZoneState, SaveError> {
        let ZoneForm {
            id,
            rng,
            spawned,
            units,
            props,
            drops,
            projectiles,
            grounds,
            triggers,
            tile_deltas,
            fog,
            pending_fill,
            sleeping_due,
            wetness,
            pressure,
            ring_key,
        } = self;
        if id != bp.zone {
            return Err(SaveError::Malformed("a zone out of its place"));
        }
        let locals = bp
            .local_names
            .iter()
            .map(|n| syms.find(n))
            .collect::<Option<Vec<Sym>>>()
            .ok_or(SaveError::Malformed("a zone's local name is not in the save"))?;
        let key = |k: Key| crate::sym::of_key(k, &locals);
        let at = spawned.tick;
        let units = units.into_vec(
            spawned.unit,
            bp.units.len(),
            |u| u.id.get(),
            |i, id| {
                let row = &bp.units[i];
                spawn_unit(row, UnitId::new(id).expect("a spawn id is past its base"), key(row.key), at)
            },
        )?;
        let props = props.into_vec(
            spawned.prop,
            bp.props.len(),
            |p| p.id.get(),
            |i, id| {
                let row = &bp.props[i];
                spawn_prop(row, i as u16, PropId::new(id).expect("a spawn id is past its base"), key(row.key))
            },
        )?;
        Ok(ZoneState {
            id,
            rng,
            spawned,
            units,
            props,
            drops: drops.into_owned(),
            projectiles: projectiles.into_owned(),
            grounds: grounds.into_owned(),
            triggers: triggers.into_owned(),
            tile_deltas: tile_deltas.into_owned(),
            fog: fog.into_owned().into_boxed_slice(),
            pending_fill: pending_fill.into_owned(),
            sleeping_due: sleeping_due.into_owned(),
            wetness,
            pressure: pressure.into_owned(),
            ring_key,
        })
    }
}

/// What a zone's form keeps of its spawns, for tests and tools.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpawnCounts {
    /// Units stored whole: changed spawns, and every unit made at runtime or come from elsewhere.
    pub units_stored: u32,
    /// Unit rows whose spawn is gone.
    pub units_removed: u32,
    pub props_stored: u32,
    pub props_removed: u32,
}

impl ZoneForm<'_> {
    pub fn counts(&self) -> SpawnCounts {
        SpawnCounts {
            units_stored: self.units.stored.len() as u32,
            units_removed: self.units.removed.len() as u32,
            props_stored: self.props.stored.len() as u32,
            props_removed: self.props.removed.len() as u32,
        }
    }
}

/// Is `got` the sym blueprint key `want` interns to? Without the zone's locals at hand: a
/// local is compared by its name.
fn key_is(syms: &SymTable, bp: &Blueprint, got: Sym, want: Key) -> bool {
    match want {
        Key::Name(n) => got == of_name(n),
        Key::Local(i) => bp.local_names.get(i as usize).is_some_and(|n| syms.is(got, n)),
    }
}

impl<'a, T: Clone> Spawns<'a, T> {
    /// `items` (ascending id) against the `n` rows spawned from `base + 1`. `fresh(item, i)` says
    /// whether an item is row `i` exactly as first made.
    fn of(items: &'a [T], id: impl Fn(&T) -> u32, base: u32, n: u32, fresh: impl Fn(&T, u32) -> bool) -> Spawns<'a, T> {
        let mut removed = Vec::new();
        let mut stored = Vec::new();
        // The first row not yet met.
        let mut next = 0;
        let mut last = 0;
        for it in items {
            let i = id(it);
            debug_assert!(i > last, "instances are in ascending id");
            last = i;
            if i > base && i - base - 1 < n {
                let row = i - base - 1;
                removed.extend(next..row);
                next = row + 1;
                if fresh(it, row) {
                    continue;
                }
            }
            stored.push(Cow::Borrowed(it));
        }
        removed.extend(next..n);
        Spawns { removed, stored }
    }

    /// The instances back, in ascending id: each row neither removed nor stored made by
    /// `make(row, id)`, merged with the stored.
    fn into_vec(
        self,
        base: u32,
        n: usize,
        id: impl Fn(&T) -> u32,
        make: impl Fn(usize, u32) -> T,
    ) -> Result<Vec<T>, SaveError> {
        let bad = SaveError::Malformed;
        let mut out: Vec<T> = Vec::with_capacity(n + self.stored.len());
        let mut stored = self.stored.into_iter().map(Cow::into_owned).peekable();
        let mut removed = self.removed.into_iter().peekable();
        for row in 0..n {
            let sid = base + 1 + row as u32;
            while let Some(s) = stored.next_if(|s| id(s) < sid) {
                out.push(s);
            }
            if let Some(s) = stored.next_if(|s| id(s) == sid) {
                if removed.next_if_eq(&(row as u32)).is_some() {
                    return Err(bad("a spawn both removed and stored"));
                }
                out.push(s);
            } else if removed.next_if_eq(&(row as u32)).is_none() {
                out.push(make(row, sid));
            }
        }
        out.extend(stored);
        if removed.next().is_some() {
            return Err(bad("spawns removed out of order or past the rows"));
        }
        if out.windows(2).any(|w| id(&w[0]) >= id(&w[1])) {
            return Err(bad("instances out of id order"));
        }
        Ok(out)
    }
}

/// The name tail as runs: at each point, the first zone (in zone order, not run yet) whose
/// `local_names`, interned there, append the next names of the tail ([`run_of`]), else the one name there.
fn sym_runs<'a>(syms: &'a SymTable, bps: &Blueprints) -> Vec<SymRun<'a>> {
    let tail = syms.tail();
    let mut out = Vec::new();
    let mut p = 0;
    // A zone's names once interned are all in the table: its second run would append nothing.
    let mut used = [false; ZONE_COUNT];
    'runs: while p < tail.len() {
        for z in ZoneId::ALL {
            if used[z.index()] {
                continue;
            }
            // A zone never built has never been interned: its names are its own (`run_of` finds
            // one of them in no table), so it would append nothing here.
            let Some(names) = bps.names(z) else { continue };
            let k = run_of(syms, p, names);
            if k > p {
                out.push(SymRun::Zone(z));
                used[z.index()] = true;
                p = k;
                continue 'runs;
            }
        }
        out.push(SymRun::Name(Cow::Borrowed(tail.get(p).unwrap_or_default())));
        p += 1;
    }
    out
}

/// Where the tail would be after interning `names` over its first `p` names, if what that
/// appends is exactly the tail's next names; `p` if it is not. A name appended is the tail's
/// next one (tail names are unique, and never catalog names); one not appended is already in
/// the table, among the catalog's or the tail's first names (a lookup, taken only for those).
fn run_of(syms: &SymTable, p: usize, names: &jane_core::Names) -> usize {
    let tail = syms.tail();
    let before = syms.len() as usize - tail.len();
    let mut k = p;
    for n in names {
        if tail.get(k).is_some_and(|t| t == n) {
            k += 1;
        } else if syms.find(n).is_none_or(|s| (s.0 as usize) >= before + k) {
            return p;
        }
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows 0..5 spawned as ids 11..=15; 13 and 15 gone, 12 changed, 3 and 20 made at runtime.
    #[test]
    fn spawns_keep_what_changed_and_merge_back_in_id_order() {
        let items = [3u32, 11, 12, 14, 20];
        let s = Spawns::of(&items, |&i| i, 10, 5, |&i, _| i != 12);
        assert_eq!(s.removed, [2, 4]);
        assert_eq!(s.stored.iter().map(|c| **c).collect::<Vec<_>>(), [3, 12, 20]);
        let back = s.into_vec(10, 5, |&i| i, |_, id| id).unwrap();
        assert_eq!(back, items);
    }

    #[test]
    fn spawns_that_contradict_are_refused() {
        let both = Spawns::<u32> { removed: vec![1], stored: vec![Cow::Owned(12)] };
        assert!(both.into_vec(10, 5, |&i| i, |_, id| id).is_err());
        let past = Spawns::<u32> { removed: vec![5], stored: Vec::new() };
        assert!(past.into_vec(10, 5, |&i| i, |_, id| id).is_err());
        let unsorted = Spawns::<u32> { removed: Vec::new(), stored: vec![Cow::Owned(30), Cow::Owned(20)] };
        assert!(unsorted.into_vec(10, 5, |&i| i, |_, id| id).is_err());
    }
}
