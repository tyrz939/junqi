//! The thirteen blueprints of a seed. A blueprint is a pure function of `(zone, seed)` and never
//! saved; the save holds the seed and what changed since.
//!
//! Two ways to hold them:
//!
//! - **All at once** ([`Blueprints::build`], [`Blueprints::build_packed_with`],
//!   [`Blueprints::from_parts`]): every zone built up front and kept (tests, benches, tools, a
//!   lockstep guest).
//! - **On demand** ([`Blueprints::on_demand_with`], PORT.md §13.3): New Game builds the county
//!   alone; any other zone is built when the sim first needs it (a seat walks in, a save names it)
//!   through a [`ZoneSource`], and let go again once no seat is in it ([`Blueprints::release`]).
//!   The county is never let go (its clock rows run every hour, whoever is out).
//!
//! Nothing the sim steps, saves or hashes can tell the two apart: a zone built again is the same
//! blueprint, and what the save and the hash need of a zone whose blueprint was let go is kept in
//! its [`ZoneDigest`], made from the blueprint before it goes.
//!
//! Shared by `Arc` so a test, a bench or a lockstep peer can hand one set to several sims.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use jane_core::{Blueprint, Names, ZONE_COUNT, ZoneId};

use crate::state::SpawnBase;

/// A zone was refused for this seed (`jane_world::ZoneError`): no builder, or no attempt proved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildError(pub ZoneId, pub jane_world::ZoneError);

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "zone {} did not build: {}", self.0.name(), self.1)
    }
}

impl core::error::Error for BuildError {}

/// One zone's blueprint, proven (`jane_world::build_zone`). An unproven zone is an error, never
/// played.
pub fn build_one(zone: ZoneId, seed: u32) -> Result<Blueprint, BuildError> {
    build_one_with(zone, seed, &mut |_| {})
}

/// [`build_one`], saying each stage to `report` as it starts (`jane_world::Report`).
pub fn build_one_with(zone: ZoneId, seed: u32, report: jane_world::Report<'_>) -> Result<Blueprint, BuildError> {
    jane_world::build_zone_with(zone, seed, report).map_err(|e| BuildError(zone, e))
}

/// [`build_one_with`] in the console form (`jane_world::build_zone_packed_with`): the same
/// blueprint packed, built so the county never holds its grids beside the solver's planes.
pub fn build_one_packed_with(zone: ZoneId, seed: u32, report: jane_world::Report<'_>) -> Result<Blueprint, BuildError> {
    jane_world::build_zone_packed_with(zone, seed, report).map_err(|e| BuildError(zone, e))
}

/// How many of the sim's steps a zone handed in ahead of her ([`Blueprints::offer`]) is kept
/// if no seat goes in: ten seconds.
pub const OFFER_KEPT: u16 = 600;

/// How many seeds [`Blueprints::build_rerolled`] tries before it gives up.
pub const REROLLS: u32 = 64;

/// The seed tried after `seed` when it is refused (a New Game nobody chose a seed for).
pub fn next_seed(seed: u32) -> u32 {
    // Weyl step: every u32 is visited before one repeats.
    seed.wrapping_add(0x9E37_79B9)
}

/// Where an on-demand set's zones come from (the storage seam). It must give the same blueprint
/// for the same `(seed, zone)` every time, in the form the set holds (packed or not): the sim
/// cannot tell a zone built once from one built again, and must never be able to.
pub trait ZoneSource: Send + Sync + core::fmt::Debug {
    /// Zone `zone` of `seed`, proven, saying its stages to `report` as they start.
    fn zone(&self, seed: u32, zone: ZoneId, report: jane_world::Report<'_>) -> Result<Blueprint, BuildError>;
}

/// Read a zone from somewhere it was kept (a cache of built zones): `None` when it is not there,
/// and the zone is built. What it gives must be what the build gives.
pub type LoadZone = fn(seed: u32, zone: ZoneId) -> Option<Blueprint>;

/// The [`ZoneSource`] that builds (`jane_world`), in the console form when `packed`, after asking
/// `load` (a cache) if one is set.
#[derive(Clone, Copy, Debug, Default)]
pub struct Build {
    pub packed: bool,
    pub load: Option<LoadZone>,
}

impl ZoneSource for Build {
    fn zone(&self, seed: u32, zone: ZoneId, report: jane_world::Report<'_>) -> Result<Blueprint, BuildError> {
        if let Some(bp) = self.load.and_then(|load| load(seed, zone)) {
            return Ok(bp);
        }
        if self.packed { build_one_packed_with(zone, seed, report) } else { build_one_with(zone, seed, report) }
    }
}

/// What the sim needs of a zone whose blueprint is not held (PORT.md §13.3): made once from the
/// blueprint when the zone is first built, its spawns' part when the blueprint is let go.
#[derive(Clone, Debug)]
pub struct ZoneDigest {
    pub zone: ZoneId,
    pub indoor: bool,
    /// The blueprint's `local_names` (shared with it, and with the sim's name table).
    pub names: Names,
    pub unit_rows: u32,
    pub prop_rows: u32,
    /// Each spawn row's instance as the zone's state first made it, hashed: `None` until the zone
    /// has a state and its blueprint has been let go.
    pub(crate) spawns: Option<Arc<SpawnDigest>>,
}

/// A zone's spawn rows as first made (`zone::spawn_unit`, `zone::spawn_prop`) for the ids and
/// tick of `base`, each the xxh3-64 of its save encoding: what the save compares an instance
/// with to know whether it is still its row exactly (`save::ZoneForm`).
#[derive(Clone, Debug)]
pub(crate) struct SpawnDigest {
    pub base: SpawnBase,
    pub units: Vec<u64>,
    pub props: Vec<u64>,
}

impl ZoneDigest {
    fn of(bp: &Blueprint) -> Self {
        Self {
            zone: bp.zone,
            indoor: bp.indoor,
            names: bp.local_names.clone(),
            unit_rows: bp.units.len() as u32,
            prop_rows: bp.props.len() as u32,
            spawns: None,
        }
    }
}

/// A seed's blueprints, every one held or built on demand (see the module doc).
#[derive(Clone, Debug)]
pub struct Blueprints {
    seed: u32,
    zones: [Option<Arc<Blueprint>>; ZONE_COUNT],
    /// `None`: all thirteen are held, always. Boxed: a set is moved and cloned whole.
    lazy: Option<Box<OnDemand>>,
}

/// An on-demand set's own: where its zones come from and what it keeps of each.
#[derive(Clone, Debug)]
struct OnDemand {
    source: Arc<dyn ZoneSource>,
    /// Every zone built at least once.
    digests: [Option<ZoneDigest>; ZONE_COUNT],
    /// Kept although no seat is in it (a zone handed in ahead of her, [`Blueprints::offer`]):
    /// the sim's steps left before it may be let go, [`OFFER_KEPT`] when handed in, 0 once a
    /// seat has been in it.
    pinned: [u16; ZONE_COUNT],
}

impl Blueprints {
    pub fn build(seed: u32) -> Result<Self, BuildError> {
        Self::build_with(seed, &mut |_| {})
    }

    /// [`Blueprints::build`], saying each stage to `report` as it starts: what the loading
    /// screen hears (PRESENTATION.md §3.2). The blueprints are the same whoever listens.
    pub fn build_with(seed: u32, report: jane_world::Report<'_>) -> Result<Self, BuildError> {
        let mut built: Vec<Arc<Blueprint>> = Vec::with_capacity(ZONE_COUNT);
        for z in ZoneId::ALL {
            built.push(Arc::new(build_one_with(z, seed, report)?));
        }
        Ok(Self::held(seed, built))
    }

    /// [`Blueprints::build_with`] for a seed nobody chose (New Game from the clock): a seed that
    /// does not prove is set aside for [`next_seed`], up to [`REROLLS`] times. The blueprints
    /// carry the seed that proved ([`Blueprints::seed`]); the error is the last seed's.
    pub fn build_rerolled(seed: u32, report: jane_world::Report<'_>) -> Result<Self, BuildError> {
        rerolled(seed, |s| Self::build_with(s, report))
    }

    /// [`Blueprints::build_with`] in the console form: every blueprint packed as it is built
    /// ([`build_one_packed_with`]); the same blueprints [`Blueprints::packed`] makes of PC's.
    pub fn build_packed_with(seed: u32, report: jane_world::Report<'_>) -> Result<Self, BuildError> {
        let mut built: Vec<Arc<Blueprint>> = Vec::with_capacity(ZONE_COUNT);
        for z in ZoneId::ALL {
            built.push(Arc::new(build_one_packed_with(z, seed, report)?));
        }
        Ok(Self::held(seed, built))
    }

    /// On demand (see the module doc): the county built now from `source`, every other zone when
    /// the sim first needs it. The county is the only zone New Game needs.
    pub fn on_demand_with(
        seed: u32,
        source: Arc<dyn ZoneSource>,
        report: jane_world::Report<'_>,
    ) -> Result<Self, BuildError> {
        let mut b = Self {
            seed,
            zones: core::array::from_fn(|_| None),
            lazy: Some(Box::new(OnDemand { source, digests: core::array::from_fn(|_| None), pinned: [0; ZONE_COUNT] })),
        };
        b.ensure_with(ZoneId::County, report)?;
        Ok(b)
    }

    /// [`Blueprints::on_demand_with`] for a seed nobody chose, set aside as
    /// [`Blueprints::build_rerolled`] does when its county does not prove.
    pub fn on_demand_rerolled(
        seed: u32,
        source: &Arc<dyn ZoneSource>,
        report: jane_world::Report<'_>,
    ) -> Result<Self, BuildError> {
        rerolled(seed, |s| Self::on_demand_with(s, Arc::clone(source), report))
    }

    fn held(seed: u32, built: Vec<Arc<Blueprint>>) -> Self {
        let zones: [Arc<Blueprint>; ZONE_COUNT] = built.try_into().unwrap_or_else(|_| unreachable!("thirteen zones"));
        Self::from_parts(seed, zones)
    }

    /// Blueprints made some other way (a harness playing one room as the whole zone), all held.
    /// Every `zones[i].zone` must be `ZoneId::ALL[i]`.
    pub fn from_parts(seed: u32, zones: [Arc<Blueprint>; ZONE_COUNT]) -> Self {
        for (i, bp) in zones.iter().enumerate() {
            assert_eq!(bp.zone, ZoneId::ALL[i], "blueprint {i} is for {:?}", bp.zone);
        }
        Self { seed, zones: zones.map(Some), lazy: None }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Whether zones are built on demand and let go ([`Blueprints::on_demand_with`]).
    pub fn on_demand(&self) -> bool {
        self.lazy.is_some()
    }

    /// Zone `z`'s blueprint, which must be held: the county always is, and so is every zone a
    /// seat is in (the sim builds it as she walks in). For any other zone of an on-demand set,
    /// [`Blueprints::fetch`].
    ///
    /// Panics if it is not held.
    pub fn get(&self, z: ZoneId) -> &Arc<Blueprint> {
        self.zones[z.index()].as_ref().unwrap_or_else(|| {
            panic!("zone {}'s blueprint is not held (built on demand): fetch it, or enter it", z.name())
        })
    }

    /// Zone `z`'s blueprint if it is held now.
    pub fn held_now(&self, z: ZoneId) -> Option<&Arc<Blueprint>> {
        self.zones[z.index()].as_ref()
    }

    /// Zone `z`'s blueprint, held or built now from the set's source (and not kept: a reader of
    /// another zone, the quest compass or a bot, that should cache what it learns). The same
    /// blueprint either way.
    ///
    /// Panics if the zone does not build for this seed (worldgen proves every zone of every seed
    /// it has been run on; `tests/zones_on_demand.rs`).
    pub fn fetch(&self, z: ZoneId) -> Arc<Blueprint> {
        if let Some(bp) = &self.zones[z.index()] {
            return Arc::clone(bp);
        }
        let src = &self.lazy.as_ref().expect("a set that holds every zone holds this one").source;
        Arc::new(src.zone(self.seed, z, &mut |_| {}).unwrap_or_else(|e| panic!("seed {}: {e}", self.seed)))
    }

    /// Hold zone `z`'s blueprint, building it if need be. Panics if it does not build (see
    /// [`Blueprints::fetch`]).
    pub fn ensure(&mut self, z: ZoneId) -> &Arc<Blueprint> {
        let seed = self.seed;
        if let Err(e) = self.ensure_with(z, &mut |_| {}) {
            panic!("seed {seed}: {e}");
        }
        self.get(z)
    }

    /// [`Blueprints::ensure`], saying the build's stages to `report`.
    pub fn ensure_with(&mut self, z: ZoneId, report: jane_world::Report<'_>) -> Result<(), BuildError> {
        if self.zones[z.index()].is_some() {
            return Ok(());
        }
        let src = &self.lazy.as_ref().expect("a set that holds every zone holds this one").source;
        let bp = src.zone(self.seed, z, report)?;
        self.hold(Arc::new(bp));
        Ok(())
    }

    /// Hand in zone `bp.zone`'s blueprint built elsewhere (a loader thread, ahead of her at a
    /// door): held, and kept until a seat has been in the zone, or for [`OFFER_KEPT`] steps if
    /// none goes in. It must be what the set's source
    /// gives for this seed. Ignored if the zone is held already, or the set holds every zone.
    pub fn offer(&mut self, bp: Arc<Blueprint>) {
        let z = bp.zone;
        if self.lazy.is_none() || self.zones[z.index()].is_some() {
            return;
        }
        self.hold(bp);
        if let Some(l) = &mut self.lazy {
            l.pinned[z.index()] = OFFER_KEPT;
        }
    }

    fn hold(&mut self, bp: Arc<Blueprint>) {
        let z = bp.zone;
        // The world's rolls draw for every area of every zone, held or not (`living.rs`), and a
        // zone's sky is its own unless it maps regions: both only the county's. A zone that broke
        // this could not be let go without changing the draws.
        assert!(
            z == ZoneId::County || (bp.areas.is_empty() && bp.regions.is_empty()),
            "zone {} has areas or regions: only the county may",
            z.name()
        );
        if let Some(l) = &mut self.lazy
            && l.digests[z.index()].is_none()
        {
            l.digests[z.index()] = Some(ZoneDigest::of(&bp));
        }
        self.zones[z.index()] = Some(bp);
    }

    /// What is kept of zone `z` once built (on-demand sets only).
    pub fn digest(&self, z: ZoneId) -> Option<&ZoneDigest> {
        self.lazy.as_ref()?.digests[z.index()].as_ref()
    }

    /// Zone `z`'s generated names, if it has been built: from the blueprint, else its digest.
    pub fn names(&self, z: ZoneId) -> Option<&Names> {
        match &self.zones[z.index()] {
            Some(bp) => Some(&bp.local_names),
            None => self.digest(z).map(|d| &d.names),
        }
    }

    /// Whether zone `z` is indoors: from the blueprint, else its digest. `None` for a zone never
    /// built.
    pub fn indoor(&self, z: ZoneId) -> Option<bool> {
        match &self.zones[z.index()] {
            Some(bp) => Some(bp.indoor),
            None => self.digest(z).map(|d| d.indoor),
        }
    }

    /// The spawn digest of a zone whose blueprint is not held.
    pub(crate) fn spawns(&self, z: ZoneId) -> Option<&SpawnDigest> {
        self.digest(z)?.spawns.as_deref()
    }

    /// Let go of zone `z`'s blueprint if this set is on demand, `z` is not the county and not
    /// pinned, and no seat is in it (`live`). `spawns(bp)`, given the blueprint, makes its spawn
    /// digest when the zone has a state (and returns `None` when it has none); it is asked only
    /// when the digest is missing or for another base. Whether it was let go.
    pub(crate) fn release(
        &mut self,
        z: ZoneId,
        live: bool,
        base: Option<SpawnBase>,
        spawns: impl FnOnce(&Blueprint) -> Option<SpawnDigest>,
    ) -> bool {
        let i = z.index();
        let Some(l) = &mut self.lazy else { return false };
        if live {
            l.pinned[i] = 0;
        }
        if l.pinned[i] > 0 {
            l.pinned[i] -= 1;
            return false;
        }
        if z == ZoneId::County || live {
            return false;
        }
        let Some(bp) = self.zones[i].take() else { return false };
        let d = l.digests[i].get_or_insert_with(|| ZoneDigest::of(&bp));
        if let Some(base) = base
            && d.spawns.as_ref().is_none_or(|s| s.base != base)
        {
            d.spawns = spawns(&bp).map(Arc::new);
        }
        true
    }

    /// Every blueprint's tiles and paint packed in chunks (`Blueprint::pack`, PORT.md §13.3): the
    /// console form, about a third of the county's bytes. The sim plays the same (its tiles read
    /// through `Blueprint::tile`); what is drawn must read the packed planes, since `tiles` is
    /// hollow and `paint` empty after (PC's renderers read the grids, so PC keeps them). A
    /// blueprint shared elsewhere is copied before it is packed. An on-demand set held as built
    /// is not changed here: its source decides the form.
    #[must_use]
    pub fn packed(mut self) -> Self {
        for bp in self.zones.iter_mut().flatten() {
            Arc::make_mut(bp).pack();
        }
        self
    }

    /// How many blueprints are held now.
    pub fn held_count(&self) -> usize {
        self.zones.iter().flatten().count()
    }
}

/// `build(seed)`, then the next seeds while it is refused as unproven, up to [`REROLLS`] tries.
fn rerolled(seed: u32, mut build: impl FnMut(u32) -> Result<Blueprints, BuildError>) -> Result<Blueprints, BuildError> {
    let mut seed = seed;
    let mut tries = 1;
    loop {
        match build(seed) {
            Err(BuildError(_, jane_world::ZoneError::Unproven(_))) if tries < REROLLS => {
                seed = next_seed(seed);
                tries += 1;
            }
            r => return r,
        }
    }
}
