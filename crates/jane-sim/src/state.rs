//! The authoritative state (ARCHITECTURE.md §3.2, §3.3): everything that can change a future
//! tick, and nothing else. It is saved and hashed as one postcard encoding, the state as it
//! differs from what its seed builds ([`crate::save::Form`], §3.5, §3.6); what is derived from it
//! lives in [`crate::runtime::ZoneRuntime`] and is rebuilt, never saved.
//!
//! Iteration order is array order or `BTreeMap` order. No `usize` is stored.
//!
//! Some fields belong to systems that land later (PORT.md §7 P4 is several units). They are
//! here, with their final types where the architecture fixes them, and nothing in this unit
//! reads or writes them; each says which unit owns it. A field changed by that unit bumps
//! [`SAVE_VERSION`].

use std::collections::BTreeMap;

use jane_core::{
    Angle, Cell, CellIx, ConsequenceId, DialogueId, EffectId, ItemId, ListRef, Milli, NameId, PropDefId, QuestId,
    Sfc32, SpellId, Stack, StoryId, Sym, TextRef, Tick, Tile, UnitDefId, Vec2, ZONE_COUNT, ZoneId,
};
use jane_data::{BarSlot, Controller, Faction};
use serde::{Deserialize, Serialize};

use crate::codec;
use crate::ids::{ClientToken, Counters, DropId, GroundId, ProjId, PropId, Seat, UnitId};
use crate::sym::SymTable;
use crate::tuning::{BAG_SLOTS, BAR_SLOTS, CRAFT_INPUTS, STORE_SLOTS};

/// The save and hash schema's version. Bumped by any change to a type in this module.
///
/// 2: a projectile's faction, velocity and blow (combat). 3: the journal (`Journal`'s entries
/// and known facts). 4: the controllers' fields live (`patrol_at`, `dwell_until`, `order`, `path`,
/// `snake`; a path let go keeps its box with no goal, and the snake's `phase_tick` is a count).
/// 5: the living world lives (a sky per region, the consequences owed a zone, the journal's
/// `Consequence` fact; `wetness`, `pressure`, `consequences_done` and `rumours` written).
/// 6: a zone's rain ramp is one per region (`wetness: [u8; 3]`: the county is under three
/// skies); the ecology steps every ten game minutes; a bed's night moves the tick too.
/// 8: a zone's units and props are saved as what differs from its blueprint's spawn
/// (`ZoneState::spawned`, `save::Form`), and the name tail as runs of blueprint locals.
/// 9: food comes back (`Prop::regrow`, `regrow.rs`).
/// 10: cupboards (`GameState::stores`, `store.rs`).
pub const SAVE_VERSION: u16 = 10;

/// A fixed-size bit set (trigger bits, consequences done).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bits(Vec<u32>);

impl Bits {
    pub fn new(n: u32) -> Self {
        Self(vec![0; n.div_ceil(32) as usize])
    }

    pub fn get(&self, i: u32) -> bool {
        self.0.get((i >> 5) as usize).is_some_and(|w| w & (1 << (i & 31)) != 0)
    }

    /// Set or clear bit `i`; outside the set is a no-op.
    pub fn set(&mut self, i: u32, v: bool) {
        if let Some(w) = self.0.get_mut((i >> 5) as usize) {
            if v {
                *w |= 1 << (i & 31);
            } else {
                *w &= !(1 << (i & 31));
            }
        }
    }

    /// Grow to hold `n` bits (a trigger row added since the save was written).
    pub fn grow_to(&mut self, n: u32) {
        let words = n.div_ceil(32) as usize;
        if self.0.len() < words {
            self.0.resize(words, 0);
        }
    }

    pub fn words(&self) -> &[u32] {
        &self.0
    }

    pub fn words_mut(&mut self) -> &mut [u32] {
        &mut self.0
    }
}

/// A flag's key at runtime. Content's `FlagKey` names a `Key`; here it is a `Sym`, which is a
/// `NameId` for content names (ARCHITECTURE.md §3.1 `FlagKey`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FlagKey {
    Named(Sym),
    Been(Sym),
    Dead(Sym),
}

// --- the world ---------------------------------------------------------------------------

/// Not `Serialize`: the save and the hash encode it through [`crate::save::Form`], which needs
/// the blueprints (§3.5), so nothing can encode it another way by mistake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub version: u16,
    pub seed: u32,
    /// Step calls: the wire and replay clock.
    pub frame: u32,
    /// Advances only when not frozen, one a step; a bed's night advances it by the time slept,
    /// with the clock (`living::Sim::sleep_to`).
    pub tick: Tick,
    /// Ticks since midnight.
    pub clock: u32,
    pub day: u32,
    /// The heroine's name. The host chooses it; everyone who joins plays her.
    pub name: String,
    /// Whether anyone else may sit down. A save always loads closed.
    pub open: bool,
    pub next: Counters,
    /// The world stream: weather on the hour and ecology every ten game minutes, drawn in a
    /// fixed order (§4.4, `living.rs`). Nothing else draws from it.
    pub rng: Sfc32,
    /// In seat order; at most four. A seat is never removed: a leaver's body is parked.
    pub players: Vec<PlayerState>,
    /// A zone's state is made on its first visit and kept for ever.
    pub zones: [Option<Box<ZoneState>>; ZONE_COUNT],
    pub flags: BTreeMap<FlagKey, i32>,
    pub quests: Quests,
    /// The last bed or fire anyone rested at (`verbs::rest`).
    pub rest: Option<RestPoint>,
    pub growth: Growth,
    pub syms: SymTable,
    /// What is known (§3.7, `journal.rs`).
    pub journal: Journal,
    /// The sky over each region, in region order (§4.6.b, `living.rs`). `Clear` at New Game.
    pub weather: [WeatherState; REGIONS],
    /// One bit per `ConsequenceId`: set when it fires, so it fires once per save, ever (§4.6.d).
    pub consequences_done: Bits,
    /// Consequences fired whose edits wait for their zone to be live, in the order they fired.
    pub consequences_owed: Vec<(ZoneId, ConsequenceId)>,
    /// Who hears of which story, from when (§4.6.e): written when one of the story's quests is
    /// first handed in, at that tick plus the row's `after`. A person keyed by a content name.
    pub rumours: BTreeMap<(NameId, StoryId), Tick>,
    /// What is kept in the cupboards (`store.rs`), by zone and prop: the world's, so the whole
    /// party shares one cupboard's shelves, as it shares the quests and the rest point (her bags
    /// stay hers). Only a cupboard with something in it has a row; the last thing out removes it.
    pub stores: BTreeMap<(ZoneId, PropId), Store>,
}

/// One cupboard's slots, stacked like a bag's (`bag.rs`).
pub type Store = Box<[Option<Stack>; STORE_SLOTS]>;

/// The three regions under their own skies (`jane_data::Region` order).
pub const REGIONS: usize = 3;

impl GameState {
    pub fn zone(&self, z: ZoneId) -> Option<&ZoneState> {
        self.zones[z.index()].as_deref()
    }

    pub fn zone_mut(&mut self, z: ZoneId) -> Option<&mut ZoneState> {
        self.zones[z.index()].as_deref_mut()
    }

    pub fn player(&self, s: Seat) -> Option<&PlayerState> {
        self.players.get(s.index())
    }

    /// Connected seats.
    pub fn connected(&self) -> impl Iterator<Item = &PlayerState> {
        self.players.iter().filter(|p| p.connected)
    }

    pub fn party_size(&self) -> u8 {
        self.connected().count() as u8
    }

    /// Is anyone connected standing in this zone?
    pub fn is_live(&self, z: ZoneId) -> bool {
        self.connected().any(|p| p.zone == z)
    }

    /// The whole hour of the day, 0..=23.
    pub fn hour(&self) -> u32 {
        self.clock / crate::tuning::TICKS_PER_HOUR
    }

    /// The day of the week, 0 Sunday to 6 Saturday: `day` counts from 0 at New Game, a Sunday
    /// (WORLD.md §2.3's day 1).
    pub fn weekday(&self) -> u8 {
        (self.day % 7) as u8
    }

    /// 21:00 to 06:00.
    pub fn is_night(&self) -> bool {
        let h = self.hour();
        !(crate::tuning::NIGHT_END_HOUR..crate::tuning::NIGHT_START_HOUR).contains(&h)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quests {
    pub active: Vec<QuestProgress>,
    pub done: Vec<QuestId>,
}

/// A quest in the log (`quests.rs`); the host's start quests are given at New Game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestProgress {
    pub quest: QuestId,
    pub counts: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestPoint {
    pub zone: ZoneId,
    pub pos: Vec2,
}

/// Growth belongs to the world: everyone at the table is as far along as everyone else.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Growth {
    pub spells: Vec<SpellId>,
    pub strength: u16,
    pub spirit: u16,
    /// The ids of the upgrades already taken (`Grow { id }`).
    pub found: Vec<Sym>,
}

/// The understood record (§3.7): what she has been, whom she has met, what she was told and
/// whether the world bore it out. The fog is the *seen* record; this is the *understood* one.
/// Written by [`crate::journal`]; read by `Condition::Knows` and `Heard` and by `View`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    /// Every change to `known`, oldest first; bounded to `JOURNAL_RING` entries per kind, the
    /// oldest of a kind dropped for its newest.
    pub entries: Vec<JournalEntry>,
    /// What is known, and how well: one row per fact.
    pub known: BTreeMap<FactKey, Known>,
}

/// A fact at runtime: content's `jane_core::action::FactKey` with its names as syms (a content
/// name's sym is its `NameId`, as for [`FlagKey`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FactKey {
    Place(Sym),
    Person(Sym),
    Thing(jane_core::action::Thing),
    Claim(jane_core::TextId),
    /// From, to.
    Route(Sym, Sym),
    /// An area.
    Danger(Sym),
    Rumour(StoryId),
    /// Something the county did for good (§4.6.d), seen.
    Consequence(ConsequenceId),
}

impl FactKey {
    pub const fn kind(self) -> JournalKind {
        match self {
            FactKey::Place(_) => JournalKind::Place,
            FactKey::Person(_) => JournalKind::Person,
            FactKey::Thing(_) => JournalKind::Thing,
            FactKey::Claim(_) => JournalKind::Claim,
            FactKey::Route(..) => JournalKind::Route,
            FactKey::Danger(_) => JournalKind::Danger,
            FactKey::Rumour(_) => JournalKind::Rumour,
            FactKey::Consequence(_) => JournalKind::Consequence,
        }
    }
}

/// What a journal entry is about; the ring is kept per kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum JournalKind {
    Place,
    Person,
    Thing,
    Claim,
    Route,
    Danger,
    Rumour,
    /// A consequence fired (§4.6.d, `living.rs`).
    Consequence,
}

/// How a fact came to be known. Within a kind, a stronger source replaces a weaker one
/// ([`Source::rank`]); `Confirmed` and `Contradicted` are final.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Source {
    // Place
    Seen,
    Visited,
    Named,
    // Person
    Met,
    Talked,
    Dead,
    // Thing (`Seen` shared)
    Held,
    // Claim: how it was heard, and what the world later did to it
    Read,
    Told,
    Confirmed,
    Contradicted,
    // Route
    Walked,
    // Danger
    AttackedIn,
    Fled,
    // Rumour
    Heard,
}

impl Source {
    /// Strength inside its kind: `Seen` < `Visited` < `Named`; `Met` < `Talked` < `Dead`;
    /// `Seen` < `Held`; `Read` = `Told` < `Confirmed` = `Contradicted`; `AttackedIn` < `Fled`.
    pub const fn rank(self) -> u8 {
        match self {
            Source::Seen | Source::Met => 0,
            Source::Visited
            | Source::Talked
            | Source::Held
            | Source::Read
            | Source::Told
            | Source::Walked
            | Source::AttackedIn
            | Source::Heard => 1,
            Source::Named | Source::Dead | Source::Fled => 2,
            Source::Confirmed | Source::Contradicted => 3,
        }
    }

    /// Nothing replaces it.
    pub const fn is_final(self) -> bool {
        matches!(self, Source::Confirmed | Source::Contradicted)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Known {
    /// When it was first known; an upgrade keeps it.
    pub since: Tick,
    pub how: Source,
}

/// One change to what is known: the fact, the source it is known by from now, where and when.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub tick: Tick,
    pub fact: FactKey,
    pub how: Source,
    pub zone: ZoneId,
    /// Where she stood (the actor's cell; the zone's origin with no actor).
    pub at: Cell,
}

impl JournalEntry {
    pub const fn kind(&self) -> JournalKind {
        self.fact.kind()
    }
}

/// What a region's sky is doing (content's `Sky`, WORLD.md §5.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WeatherKind {
    #[default]
    Clear,
    Mist,
    Rain,
    Storm,
}

impl WeatherKind {
    pub const fn of(s: jane_data::Sky) -> Self {
        match s {
            jane_data::Sky::Clear => WeatherKind::Clear,
            jane_data::Sky::Mist => WeatherKind::Mist,
            jane_data::Sky::Rain => WeatherKind::Rain,
            jane_data::Sky::Storm => WeatherKind::Storm,
        }
    }

    /// Rain and storm wet the ground.
    pub const fn wets(self) -> bool {
        matches!(self, WeatherKind::Rain | WeatherKind::Storm)
    }
}

/// A region's sky: what it is doing, since when, and until when it holds whatever is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeatherState {
    pub kind: WeatherKind,
    pub since: Tick,
    pub until: Tick,
}

// --- a seat ------------------------------------------------------------------------------

/// One person at the table. What is hers lives here (her bags, her bar, her conversation);
/// what is the world's stays on [`GameState`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub seat: Seat,
    pub who: ClientToken,
    pub unit: UnitId,
    /// The zone her body is in (or was in, when parked).
    pub zone: ZoneId,
    /// The arrival mark last used; she wakes here if she has never rested.
    pub last_mark: Sym,
    /// When a dead seat stands back up. The revive itself is the combat unit's.
    pub respawn_at: Option<Tick>,
    /// Bags are hers, off the body (`bag.rs`, `inventory.rs`; the start kit goes in here).
    pub bag: Box<[Option<Stack>; BAG_SLOTS]>,
    #[serde(with = "codec::bar")]
    pub bar: [Option<BarSlot>; BAR_SLOTS],
    /// Her craft row (`inventory.rs`).
    pub craft: [Option<Stack>; CRAFT_INPUTS],
    /// Her conversation (`dialogue.rs`). Alone, the world holds still while this is `Some`.
    pub dialogue: Option<Dialogue>,
    pub god: bool,
    pub stats: Stats,
    /// Set by `Travel`, a door or `Dev(Tp)`; performed in step 14.
    pub travel: Option<TravelRequest>,
    pub connected: bool,
    /// Her body while she is away, with everything she owned.
    pub parked: Option<Box<Unit>>,
    /// The sticky unit of aim assist (§5.4). Owned by the aim-assist unit.
    pub assist: Option<Assisted>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub kills: u32,
    pub deaths: u32,
    pub casts: u32,
}

/// A conversation, or a thing read (`dialogue.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dialogue {
    pub tree: Option<DialogueId>,
    pub node: u16,
    pub line: u16,
    pub speaker: Speaker,
    /// A thing read once, whose words are its own: one line, no tree.
    pub read: Option<TextRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Speaker {
    Unit(UnitId),
    Prop(PropId),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelRequest {
    pub zone: ZoneId,
    pub mark: Sym,
    /// An exact spot (waking at a bed or fire) instead of the mark.
    pub at: Option<Vec2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assisted {
    pub unit: UnitId,
    pub until: Tick,
}

// --- a zone ------------------------------------------------------------------------------

/// Not `Serialize`, as [`GameState`]: `save::ZoneForm` is its encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneState {
    pub id: ZoneId,
    /// This zone's combat, loot and fan dice (§4.4).
    pub rng: Sfc32,
    /// How its blueprint's spawns were numbered, and when they were made: what the save
    /// compares units and props with (§3.5). Set once, when the zone is made; nothing reads it
    /// in a step.
    pub spawned: SpawnBase,
    /// Ascending id: spawn appends, an arrival inserts in place, despawn removes in place.
    pub units: Vec<Unit>,
    /// Ascending id, never removed.
    pub props: Vec<Prop>,
    /// Owned by the loot unit.
    pub drops: Vec<Drop>,
    /// Owned by the combat unit.
    pub projectiles: Vec<Projectile>,
    /// Owned by the combat unit.
    pub grounds: Vec<Ground>,
    /// One bit each per row of the merged trigger table (`ZoneRuntime::triggers`), in its order.
    /// Stepped by `triggers.rs`.
    pub triggers: TriggerBits,
    /// Tiles that differ from the blueprint's; a tile set back to the blueprint's is removed.
    pub tile_deltas: BTreeMap<CellIx, Tile>,
    /// Seen-bits, one per fog block (2 cells indoors, 8 out), 32 to a word; fixed size.
    pub fog: Box<[u32]>,
    /// Solid fills waiting for their rect to clear (`clear.rs`).
    pub pending_fill: Vec<Fill>,
    /// Corpses to stand up, sorted `(tick, id)`. Owned by the combat unit.
    pub sleeping_due: Vec<(Tick, UnitId)>,
    /// The rain ramps (§4.6.b, `living.rs`), one per region in region order: 0 dry, 255 soaked;
    /// a fire goes out at its `douse` by the ramp of the region it stands in. The county steps
    /// all three (it lies under three skies), any other zone only its own region's.
    pub wetness: [u8; REGIONS],
    /// Per area, in the blueprint's area order (§4.6.c, `living.rs`): what hunting has taken.
    pub pressure: Vec<u16>,
    /// The block every seat here stood in when the ring last ran; `None` before it ever ran.
    /// Authoritative: whether the ring runs next tick depends on it, and a unit's `awake` bit
    /// (kept up by combat or an order) can outlast the reason it woke until the ring runs
    /// again, so a rebuilt runtime that forgot it would run the ring when the live one did not.
    pub ring_key: RingKey,
}

/// A zone's spawns as it was made (`zone::create_zone_state`): blueprint unit row `i` became
/// unit `unit + 1 + i` and prop row `i` prop `prop + 1 + i`, all at `tick`. With the blueprint
/// and the names, that rebuilds every spawn exactly as it was first made (`zone::spawn_unit`,
/// `zone::spawn_prop`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnBase {
    /// The unit counter before the first spawn took its id.
    pub unit: u32,
    /// The prop counter before the first spawn took its id.
    pub prop: u32,
    pub tick: Tick,
}

/// Per seat, the ring block it stood in (`None`: not in this zone).
pub type RingKey = Option<[Option<(i32, i32)>; crate::tuning::MAX_PLAYERS]>;

impl ZoneState {
    /// The index of a unit by id (binary search: `units` is in ascending id).
    pub fn unit_ix(&self, id: UnitId) -> Option<usize> {
        self.units.binary_search_by_key(&id, |u| u.id).ok()
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.unit_ix(id).map(|i| &self.units[i])
    }

    pub fn unit_mut(&mut self, id: UnitId) -> Option<&mut Unit> {
        self.unit_ix(id).map(|i| &mut self.units[i])
    }

    /// Put a unit in its place by id. Returns its index.
    pub fn insert_unit(&mut self, u: Unit) -> usize {
        let at = self.units.partition_point(|x| x.id < u.id);
        debug_assert!(self.units.get(at).is_none_or(|x| x.id != u.id), "unit {:?} twice", u.id);
        self.units.insert(at, u);
        at
    }

    pub fn remove_unit(&mut self, id: UnitId) -> Option<Unit> {
        self.unit_ix(id).map(|i| self.units.remove(i))
    }

    pub fn prop_ix(&self, id: PropId) -> Option<u32> {
        self.props.binary_search_by_key(&id, |p| p.id).ok().map(|i| i as u32)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerBits {
    pub fired: Bits,
    pub inside: Bits,
}

/// A solid fill owed to a rect somebody stands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    pub rect: jane_core::Rect,
    pub tile: Tile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drop {
    pub id: DropId,
    pub item: ItemId,
    pub qty: u16,
    pub pos: Vec2,
    pub born: Tick,
}

/// A bolt in flight (`combat.ts Projectile`). It flies straight and never homes; its blow was
/// rolled when it was cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projectile {
    pub id: ProjId,
    pub spell: SpellId,
    pub from: Option<UnitId>,
    /// The caster's side when it was cast: whom it may hit, whoever the caster is by now.
    #[serde(with = "codec::faction")]
    pub faction: Faction,
    pub pos: Vec2,
    /// Per tick, fixed at the cast (`along(heading, speed)`), so the line is exact.
    pub vel: Vec2,
    pub heading: Angle,
    /// Range left.
    pub left: jane_core::Fx,
    pub born: Tick,
    /// The blow it carries, whole points.
    pub hit: Milli,
    pub crit: bool,
}

/// A pool on the ground. The combat unit owns its final shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ground {
    pub id: GroundId,
    pub spell: SpellId,
    pub from: Option<UnitId>,
    #[serde(with = "codec::faction")]
    pub faction: Faction,
    pub pos: Vec2,
    pub radius: jane_core::Fx,
    pub until: Tick,
    pub next_pulse: Tick,
}

// --- a unit ------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CombatState {
    #[default]
    Idle,
    Combat,
    Leash,
}

/// One unit. Player, dog, skeleton and snake are the same shape with a different controller.
///
/// Live in the foundation: `id key def controller faction pos facing strength spirit hp mp energy
/// energy_locked alive stop_until synced home awake hidden carrying hold`. Live in combat
/// (`combat`, `status`, `flush`, `life`): `gcd_until cooldowns target combat statuses died_at
/// phase`, and `hp` changes only in the flush (and `life`: regen, respawn, revive; and the
/// snake's reset). Live in the controllers (`ai`, `npc`, `snake`, `presence`): `patrol patrol_at
/// dwell_until order path snake`, and `hidden` by the hour. Present and inert until its owner
/// lands: `item_cooldowns` (inventory).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    /// The blueprint key ("dog", "yard_skeleton"): story, triggers and saves address units by it.
    pub key: Option<Sym>,
    pub def: UnitDefId,
    #[serde(with = "codec::controller")]
    pub controller: Controller,
    #[serde(with = "codec::faction")]
    pub faction: Faction,
    /// The feet.
    pub pos: Vec2,
    pub facing: jane_core::action::Facing,
    pub strength: u16,
    pub spirit: u16,
    pub hp: Milli,
    pub mp: Milli,
    pub energy: Milli,
    /// Emptied: no sprint until full again.
    pub energy_locked: bool,
    pub alive: bool,
    pub gcd_until: Tick,
    /// Rooted in place after a cast until this tick.
    pub stop_until: Tick,
    pub cooldowns: Vec<(SpellId, Tick)>,
    pub item_cooldowns: Vec<(ItemId, Tick)>,
    /// Regen is paid up to here (`pay_regen`, §4.3).
    pub synced: Tick,
    pub target: Option<UnitId>,
    pub combat: CombatState,
    pub home: Vec2,
    pub patrol: Option<Box<Patrol>>,
    /// The patrol point it is walking to (modulo the points).
    pub patrol_at: u16,
    /// Standing at a patrol point until this tick, inclusive.
    pub dwell_until: Tick,
    /// Somewhere it has been sent (`Send`).
    pub order: Option<Box<Order>>,
    /// Authoritative: it moves the unit next tick (§3.3). A path let go keeps its box with
    /// `goal == ai::NO_GOAL` (`ai::clear_path`), so a chase allocates nothing once warm.
    pub path: Option<Box<PathCache>>,
    pub statuses: Vec<StatusInst>,
    pub died_at: Option<Tick>,
    /// Inside some seat's load ring (or held up by combat or an order). Changes only when the
    /// ring is re-run; saved so the invariant can be checked.
    pub awake: bool,
    /// Not in the world right now (the dog after dark).
    pub hidden: bool,
    pub carrying: Option<PropId>,
    /// Ticks USE has been held against a pushable.
    pub hold: u8,
    pub phase: u8,
    pub snake: Option<Box<SnakeBody>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patrol {
    /// Waypoint centres and how long to stand at each.
    pub points: Vec<(Vec2, Tick)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub to: Vec2,
    /// Gives up at this tick.
    pub until: Tick,
    pub then: Option<ListRef>,
    /// Whoever sent it; `then` runs on her behalf.
    pub seat: Option<Seat>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathCache {
    pub cells: Vec<CellIx>,
    /// The next cell to walk to.
    pub at: u16,
    /// The cell it was planned toward; a partial path does not end on it.
    pub goal: CellIx,
    /// Plan again at this tick.
    pub repath_at: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInst {
    pub effect: EffectId,
    pub until: Tick,
    pub next_pulse: Tick,
    pub from: Option<UnitId>,
    /// Absorb left, 0 when unused.
    pub pool: Milli,
}

/// A snake's own mover (`snake.rs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnakeBody {
    /// Where it is going; turned at most `speed x 4` degrees a tick.
    pub heading: Angle,
    /// Ticks spent in the phase as the phase counts them (moving ticks following her, coiled
    /// ticks spitting): a count, not a time.
    pub phase_tick: Tick,
    /// Moving ticks since the last trail point, `0..SNAKE_NODE_EVERY`.
    pub step: u16,
    /// The body's trail, newest first; always the row's `segments` long.
    pub trail: Vec<Vec2>,
}

// --- a prop ------------------------------------------------------------------------------

/// What a prop still holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LootState {
    /// What its spawn row says.
    #[default]
    AsSpawned,
    /// Some was taken; this is what is left.
    Left(Vec<Stack>),
}

/// A door's hours, once a verb has changed them (`Action::NightLock`, `NightUnlock`): the
/// state beats the spawn row's `night_lock` from then on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NightState {
    /// What its spawn row says.
    #[default]
    AsSpawned,
    /// Shut at these hours, saying this.
    Locked(jane_core::NightLock),
    /// Answered at every hour.
    Open,
}

/// A prop. What a placed prop leads to, holds, runs and says lives on its blueprint spawn row
/// (`spawn`), rebuilt from the seed on load; only what changes is here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub id: PropId,
    pub key: Sym,
    pub def: PropDefId,
    /// Index into `Blueprint.props`; `None` for a prop made at runtime.
    pub spawn: Option<u16>,
    /// The origin (top-left) cell of its footprint.
    pub cell: Cell,
    pub solid: bool,
    pub hidden: bool,
    pub locked: bool,
    pub used: bool,
    pub on: bool,
    pub loot: LootState,
    /// What lay under it has been shown.
    pub under_done: bool,
    /// Its hours, if a verb set them.
    pub night: NightState,
    /// Emptied food that comes back (`regrow.rs`): the tick it is full again. `None` for
    /// anything else, and once it is back.
    pub regrow: Option<Tick>,
}
