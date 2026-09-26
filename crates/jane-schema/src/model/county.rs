//! The zones, the skeleton's rows (sites, pois, anchors, areas, paths), doors, names, placements, stories.
//!
//! Units, as the skeleton and the county builder use them (WORLDGEN.md; `jane/src/world/skeleton/types.ts`):
//! a cell is 1 m (8 px); a macro cell is 16 x 16 cells, so 16 m; the county is 125 x 125 macro cells,
//! 2000 m square. Skeleton rows (sites, areas, anchors) speak **metres**, which the skeleton compares
//! against squared macro distances (`dist_sq` against `(m / 16)^2`). County rows (placements, stories,
//! paths, doors) speak **cells**. Every numeric field below names its unit.
//!
//! Rows reference each other by index into their own table here (`site: u8` is `County::sites[site]`),
//! so the skeleton never looks a site up by string. Names a blueprint carries (marks, rects, prop and
//! unit keys) are `NameId`s; English is `TextId`.

use std::fmt::Write;

use jane_core::action::{CondsRef, Facing, FlagKey, ListRef, Stack};
use jane_core::ids::{DialogueId, DungeonId, NameId, PropDefId, QuestId, SpellId, StoryId, TextId, UnitDefId, ZoneId};
use jane_core::num::Tick;

use crate::emit::Emit;
use crate::{model, model_enum};

// --- zones -----------------------------------------------------------------------------------

model_enum! {
    /// Which builder makes a zone (`data/zones.json` `kind`).
    pub enum ZoneKind {
        /// The open county (`jane_world::county`).
        County,
        /// A hand-built interior (`jane_world::interiors`), matched on the zone id there.
        Interior,
        /// A generated dungeon: its mission in `data/dungeons/<id>.json` gives the rest of its contract,
        /// its given keys and verbs, and its states (the dungeons group).
        Dungeon,
    }
}

model! {
    /// What every seed of a zone must contain: the names story, triggers and travel lean on.
    pub struct Contract {
        pub units: &'static [NameId],
        pub props: &'static [NameId],
        pub marks: &'static [NameId],
        pub rects: &'static [NameId],
    }
}

impl Contract {
    pub const EMPTY: Contract = Contract { units: &[], props: &[], marks: &[], rects: &[] };

    /// Every name in it, units then props then marks then rects, each with its kind.
    pub fn names(&self) -> impl Iterator<Item = (NameKind, NameId)> + '_ {
        let tag = |k: NameKind, s: &'static [NameId]| s.iter().map(move |n| (k, *n));
        tag(NameKind::Unit, self.units)
            .chain(tag(NameKind::Prop, self.props))
            .chain(tag(NameKind::Mark, self.marks))
            .chain(tag(NameKind::Rect, self.rects))
    }

    pub fn has(&self, kind: NameKind, name: NameId) -> bool {
        self.names().any(|(k, n)| k == kind && n == name)
    }
}

model! {
    /// A reversible mechanism of a whole zone (a breaker, a valve) for the solver's stateful flood.
    pub struct SolveStateDef {
        pub id: NameId,
        /// The world flag that holds it.
        pub flag: FlagKey,
    }
}

model! {
    /// A row of `data/zones.json`: one per zone, in tick order (`ZoneId::ALL`, checked at build).
    pub struct ZoneDef {
        pub id: ZoneId,
        pub kind: ZoneKind,
        /// The mission a `Dungeon` zone is built from; `None` for the others.
        pub mission: Option<DungeonId>,
        /// The contract as `zones.json` writes it. For the county, `County::promised` is added to it
        /// (`County::contract_has`); for a dungeon, the mission's binds are (the dungeons group), and
        /// this holds only the names the zone adds beyond them (the burial's `everywhere`).
        pub contract: Contract,
        /// Key tags (an item's `opens`) the story hands over from outside the zone. Empty for a
        /// dungeon here: its mission has them.
        pub given_keys: &'static [NameId],
        /// Spells she is known to have at the door. `None`: the solver gates nothing on a spell
        /// (every hand-built zone); a dungeon's come from its mission.
        pub given_verbs: Option<&'static [SpellId]>,
        /// Reversible mechanisms for the stateful flood; a dungeon's come from its mission.
        pub states: &'static [SolveStateDef],
    }
}

// --- the skeleton's rows ---------------------------------------------------------------------

model_enum! {
    /// The three regions of the county, west to east.
    pub enum Region { Lowfields, Waters, Works }
}

model_enum! {
    /// Ground a skeleton row asks for.
    pub enum Terrain {
        /// Rising ground, not too steep.
        Foothill,
        /// The highest ground in the region (within 5 macro cells of the crown).
        Crown,
        /// Beside water.
        Bank,
        Flat,
        /// Wood or wet wood.
        Wood,
    }
}

model_enum! {
    pub enum Edge { West, East, North, South }
}

model_enum! {
    /// How a site's distance rule is measured: along the road network built, or as the crow flies.
    pub enum DistBy { Road, Line }
}

model! {
    /// A distance rule from a site to an earlier site.
    pub struct DistRule {
        /// Index into `County::sites`; always an earlier row.
        pub to: u8,
        /// Metres.
        pub min: Option<u16>,
        /// Metres.
        pub max: Option<u16>,
        pub by: DistBy,
    }
}

model! {
    /// Where a site may stand.
    pub struct SiteWhere {
        pub edge: Option<Edge>,
        pub terrain: Option<Terrain>,
        /// The other side of the river from this earlier site (index into `County::sites`).
        pub across_river_from: Option<u8>,
        /// No higher than this, on the terrain's 0..=255 height scale.
        pub max_height: Option<u8>,
        /// Metres from the river, at most.
        pub river_within: Option<u16>,
        /// Metres beyond the lake's shore, at most.
        pub lake_within: Option<u16>,
        /// Metres from any road, at least.
        pub off_road: Option<u16>,
    }
}

model! {
    /// A story site (`data/sites.json`), in row order: the order they are placed in.
    pub struct SiteDef {
        /// The content id (`"julie_house"`): chunks, dice keys, tools.
        pub id: &'static str,
        pub name: TextId,
        pub region: Region,
        /// Joined to the road network. `false`: deliberately off it (the burial, the car).
        pub on_road: bool,
        /// A dungeon mouth: raises the threat in a ring round it.
        pub dungeon: bool,
        /// A bed or a fire to rest at.
        pub rest: bool,
        /// Metres: threat 0 inside this radius.
        pub hub: Option<u16>,
        pub at: SiteWhere,
        pub dist: &'static [DistRule],
        /// The earlier site its road is laid from (index into `County::sites`).
        pub road_from: Option<u8>,
    }
}

model_enum! {
    /// Where a kind of small place stands.
    pub enum PoiWhere { Roadside, Deep, Bank, Any }
}

model! {
    /// A kind of small place (`data/pois.json`), in row order (the weighted pick walks it).
    pub struct PoiDef {
        /// The kind (`"well"`): what `smallPlace` dresses, what a placement's `at.poi` asks for.
        pub kind: NameId,
        pub name: TextId,
        pub regions: &'static [Region],
        /// Relative weight in the pick; no unit.
        pub weight: u16,
        pub at: PoiWhere,
    }
}

model! {
    /// A distance band to a site, in metres.
    pub struct SiteBand {
        /// Index into `County::sites`.
        pub to: u8,
        /// Metres.
        pub min: Option<u16>,
        /// Metres.
        pub max: Option<u16>,
    }
}

model! {
    /// On the rim of a patch, on the side nearest a site.
    pub struct Rim {
        /// Index into `County::areas`.
        pub area: u8,
        /// Index into `County::sites`.
        pub toward: u8,
    }
}

model! {
    /// Along the same road as an earlier anchor, further into the dark.
    pub struct After {
        /// Index into `County::anchors`; an earlier row.
        pub anchor: u8,
        /// Macro cells along the road.
        pub steps: u8,
    }
}

model! {
    /// At least this far from an earlier anchor.
    pub struct Apart {
        /// Index into `County::anchors`; an earlier row.
        pub from: u8,
        /// Metres.
        pub min: u16,
    }
}

model! {
    /// Within a band of an earlier anchor.
    pub struct NearAnchor {
        /// Index into `County::anchors`; an earlier row.
        pub anchor: u8,
        /// Metres.
        pub min: u16,
        /// Metres.
        pub max: u16,
    }
}

model! {
    /// Where an anchor may stand. Any combination; every one given must hold.
    pub struct AnchorWhere {
        /// Beside the road between these two sites (indices into `County::sites`).
        pub road: Option<(u8, u8)>,
        /// Crow's metres to a site.
        pub dist: Option<SiteBand>,
        /// Inside this patch (index into `County::areas`).
        pub area: Option<u8>,
        pub rim: Option<Rim>,
        /// Beside any road, as near this patch as a roadside can be (index into `County::areas`).
        pub near_area: Option<u8>,
        /// Beside the last lamp that works on the longest Lowfields road that is not the first walk.
        pub last_lamp: bool,
        pub after: Option<After>,
        pub apart: Option<Apart>,
        pub near_anchor: Option<NearAnchor>,
        /// Of what is left, the few nearest this site (index into `County::sites`).
        pub nearest: Option<u8>,
    }
}

model! {
    /// A small place the story needs by name (`data/anchors.json`), in row order. The county gives it
    /// a mark and a rect of its name.
    pub struct AnchorDef {
        pub id: NameId,
        /// A kind from `County::pois`, or `None` for a bare spot ("none": a lamp post stands there).
        pub kind: Option<NameId>,
        /// `None`: the place is called by its id.
        pub name: Option<TextId>,
        pub region: Region,
        pub at: AnchorWhere,
    }
}

model! {
    /// Where a patch may lie.
    pub struct AreaWhere {
        /// Index into `County::sites`.
        pub near: Option<u8>,
        /// Metres from `near`, at least.
        pub near_min: Option<u16>,
        /// Metres from `near`, at most.
        pub near_max: Option<u16>,
        pub terrain: Option<Terrain>,
        /// Metres from any road, at least.
        pub off_road: Option<u16>,
        /// On a road.
        pub on_road: bool,
    }
}

model! {
    /// A named patch with its own danger (`data/areas.json`), in row order.
    pub struct AreaDef {
        /// A placement's `at.area`, a path's `via`, an anchor's `area`.
        pub id: NameId,
        pub name: TextId,
        pub region: Region,
        /// The phase it plays at, 0..=6; 0 is a haven.
        pub threat: u8,
        /// Metres.
        pub radius: u16,
        pub rest: bool,
        /// A quest is written against it: a county without it is re-rolled. The rest may be skipped.
        pub required: bool,
        pub at: AreaWhere,
    }
}

/// What a footpath goes by way of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// Index into `County::anchors`.
    Anchor(u8),
    /// Index into `County::areas`.
    Area(u8),
}

impl Emit for Via {
    fn emit(&self, out: &mut String) {
        let _ = match self {
            Via::Anchor(i) => write!(out, "Via::Anchor({i})"),
            Via::Area(i) => write!(out, "Via::Area({i})"),
        };
    }
}

model! {
    /// A footpath (`data/paths.json`): site to site by way of a named small place or patch.
    pub struct PathDef {
        /// The content id: its dice key.
        pub id: &'static str,
        /// Index into `County::sites`.
        pub from: u8,
        pub via: Via,
        /// Index into `County::sites`.
        pub to: u8,
        /// Cells.
        pub width: u8,
        /// The marks at its two ends: the `from` end, then the `to` end.
        pub marks: [NameId; 2],
    }
}

// --- doors -----------------------------------------------------------------------------------

/// Where a door into a dungeon stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorAt {
    /// Set into this landmark's face, at its chunk's `<site>_door` slot (index into `County::sites`).
    Chunk(u8),
    /// On open ground beside this site (index into `County::sites`).
    Near(u8),
}

impl Emit for DoorAt {
    fn emit(&self, out: &mut String) {
        let _ = match self {
            DoorAt::Chunk(i) => write!(out, "DoorAt::Chunk({i})"),
            DoorAt::Near(i) => write!(out, "DoorAt::Near({i})"),
        };
    }
}

model! {
    /// A door's `nightLock`: what it says instead of opening, and the hours it is shut, `from` up
    /// to `to`, wrapping midnight (`nightHours`; the bell's night, 21 to 6, when left out).
    pub struct NightLockDef {
        pub says: TextId,
        pub from: u8,
        pub to: u8,
    }
}

impl NightLockDef {
    /// The blueprint's lock (a row's is never `keyed`: only a verb makes one of those).
    pub const fn lock(self) -> jane_core::NightLock {
        jane_core::NightLock { says: jane_core::TextRef::Text(self.says), from: self.from, to: self.to, keyed: false }
    }
}

model! {
    /// A way into a dungeon from the county (`data/doors.json`), in row order. Every zone exists, so
    /// every row applies (PORT.md §6.l).
    pub struct DoorDef {
        pub zone: ZoneId,
        pub at: DoorAt,
        /// The prop's key in the county.
        pub key: NameId,
        /// `door` unless the row says otherwise.
        pub def: PropDefId,
        pub label: TextId,
        /// Locked to this key tag.
        pub key_tag: Option<NameId>,
        /// What it says at the hours it is shut instead of opening.
        pub night_lock: Option<NightLockDef>,
        /// The `key_tag` does not lock it: a key that fits answers the `night_lock` at its shut
        /// hours, and at the others the door opens for anyone (the Works' wicket: the night shift
        /// goes in at nine; day men by the gate key).
        pub keyed: bool,
        /// A county mark in front of it: where the dungeon's own way out arrives.
        pub mark: Option<NameId>,
        /// The mark in `zone` it leads to (`entry`); `None` for a way that opens only from below (a
        /// manhole lifts from the pipes side), which is a cover and a mark and no door down.
        pub to: Option<NameId>,
    }
}

// --- names -----------------------------------------------------------------------------------

model_enum! {
    /// Kinds of generated place a story may claim, and the name pools.
    pub enum PlaceKind { Hamlet, Farmstead, Cottage, Inn, Woodcutter, Camp, Ruin }
}

impl PlaceKind {
    pub const ALL: [PlaceKind; 7] = [
        PlaceKind::Hamlet,
        PlaceKind::Farmstead,
        PlaceKind::Cottage,
        PlaceKind::Inn,
        PlaceKind::Woodcutter,
        PlaceKind::Camp,
        PlaceKind::Ruin,
    ];

    /// The id content uses (`"farmstead"`).
    pub const fn name(self) -> &'static str {
        match self {
            PlaceKind::Hamlet => "hamlet",
            PlaceKind::Farmstead => "farmstead",
            PlaceKind::Cottage => "cottage",
            PlaceKind::Inn => "inn",
            PlaceKind::Woodcutter => "woodcutter",
            PlaceKind::Camp => "camp",
            PlaceKind::Ruin => "ruin",
        }
    }

    /// Every place of these kinds is named; a camp or a ruin only when a story claims it.
    pub const fn always_named(self) -> bool {
        !matches!(self, PlaceKind::Camp | PlaceKind::Ruin)
    }
}

model! {
    /// The names one kind of place is given (`data/names.json`), before the seed shuffles them.
    pub struct NamePool {
        pub kind: PlaceKind,
        /// In file order. The woodcutters' clearings are expanded here, ends by roots, leaving out a
        /// root the end already holds ("Coppice Coppice Wood").
        pub names: &'static [TextId],
        /// What a board at such a place says; `{NAME}` is the name in capitals.
        pub boards: &'static [TextId],
    }
}

// --- placements ------------------------------------------------------------------------------

/// Where a placement row puts its thing, by name (exactly one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceAt {
    /// A named mark (a chunk's, a dressed area's, a path end's).
    Mark(NameId),
    /// A story site: somewhere open in or beside its chunk (index into `County::sites`).
    Site(u8),
    /// A named patch: somewhere open inside its radius (index into `County::areas`).
    Area(u8),
    /// The nearest small place of this kind to `near` (index into `County::sites`; `None`: the town).
    Poi { kind: NameId, near: Option<u8> },
    /// A small place the skeleton guarantees by name (index into `County::anchors`).
    Anchor(u8),
    /// An exact cell a chunk or a dressed area offers.
    Slot(NameId),
    /// The open ground in front of a prop some chunk or earlier row placed, by key.
    Prop(NameId),
    /// A story's place, and what in it: a kept cell, one of its props, one of its people ("folk",
    /// "folk_2"), its "hostiles". `None` for a row set by position (`dx`, `dy`, `on`).
    Place { story: StoryId, slot: Option<NameId> },
}

impl Emit for PlaceAt {
    fn emit(&self, out: &mut String) {
        let one = |out: &mut String, v: &str, e: &dyn Fn(&mut String)| {
            let _ = write!(out, "PlaceAt::{v}(");
            e(out);
            out.push(')');
        };
        match self {
            PlaceAt::Mark(n) => one(out, "Mark", &|o| n.emit(o)),
            PlaceAt::Site(i) => one(out, "Site", &|o| i.emit(o)),
            PlaceAt::Area(i) => one(out, "Area", &|o| i.emit(o)),
            PlaceAt::Poi { kind, near } => {
                out.push_str("PlaceAt::Poi { kind: ");
                kind.emit(out);
                out.push_str(", near: ");
                near.emit(out);
                out.push_str(" }");
            }
            PlaceAt::Anchor(i) => one(out, "Anchor", &|o| i.emit(o)),
            PlaceAt::Slot(n) => one(out, "Slot", &|o| n.emit(o)),
            PlaceAt::Prop(n) => one(out, "Prop", &|o| n.emit(o)),
            PlaceAt::Place { story, slot } => {
                out.push_str("PlaceAt::Place { story: ");
                story.emit(out);
                out.push_str(", slot: ");
                slot.emit(out);
                out.push_str(" }");
            }
        }
    }
}

model! {
    /// A prop a row sets down: a blueprint `PropSpawn` without its key and cell.
    pub struct PropTemplate {
        pub def: PropDefId,
        pub locked: bool,
        pub key_tag: Option<NameId>,
        pub hidden: bool,
        pub on: bool,
        pub loot: &'static [Stack],
        pub use_list: Option<ListRef>,
        /// Pressure plates: runs when the last thing steps off.
        pub release: Option<ListRef>,
        /// Materials a world verb consumes.
        pub needs: &'static [Stack],
        pub talk: Option<DialogueId>,
        pub label: Option<TextId>,
        pub night_lock: Option<NightLockDef>,
    }
}

model! {
    /// The fields an `edit` row sets on a prop that is already there; `None` leaves one as it is.
    pub struct PropEdit {
        /// A new key, so words can name the thing (a tale's house).
        pub key: Option<NameId>,
        /// Only for a def of the same footprint.
        pub def: Option<PropDefId>,
        pub locked: Option<bool>,
        pub key_tag: Option<NameId>,
        pub hidden: Option<bool>,
        pub loot: Option<&'static [Stack]>,
        pub use_list: Option<ListRef>,
        pub talk: Option<DialogueId>,
        pub label: Option<TextId>,
        pub night_lock: Option<NightLockDef>,
    }
}

model! {
    /// A unit a row stands up.
    pub struct PlacedUnit {
        pub def: UnitDefId,
        /// `None`: the threat of the ground under it.
        pub phase: Option<u8>,
        pub facing: Option<Facing>,
    }
}

model! {
    /// A rect a row names, centred on the named place.
    pub struct PlacedRect {
        pub name: NameId,
        /// Cells.
        pub w: u16,
        /// Cells.
        pub h: u16,
    }
}

model! {
    /// A hidden thing lying under the placed prop, shown when that is pushed off it.
    pub struct Hides {
        pub key: NameId,
        pub prop: PropTemplate,
        /// Only while these hold.
        pub when: Option<CondsRef>,
    }
}

model! {
    /// A placement row (`data/placements/*.json`), in file then row order: the order decides who
    /// gets a contested spot. Content says where by name and the county works out the cells.
    pub struct PlacementDef {
        /// What the story calls it.
        pub key: NameId,
        /// The keys it places: `key` alone, or `key_1` to `key_n` with `count` n > 1.
        pub keys: &'static [NameId],
        /// With several in an area: each somewhere of its own about the patch.
        pub spread: bool,
        pub at: PlaceAt,
        /// Cells from the spot it may land.
        pub within: Option<u16>,
        /// At a story's place, by position: cells right of the place's top-left.
        pub dx: Option<i16>,
        /// Cells down from the place's top-left.
        pub dy: Option<i16>,
        /// At a story's place, on the cell of an earlier row's thing (its key), moved by `dx`, `dy`.
        pub on: Option<NameId>,
        /// With `dx`, `dy`: open ground to find, `[w, h, ox, oy]` in cells, and where in it this row's thing goes.
        pub room: Option<[u8; 4]>,
        /// Instead of placing: change the prop `key` names (or, at a story's place, the one its slot names).
        pub edit: Option<PropEdit>,
        pub unit: Option<PlacedUnit>,
        pub prop: Option<PropTemplate>,
        pub rect: Option<PlacedRect>,
        /// A mark below the thing, named this.
        pub mark: Option<NameId>,
        pub hides: Option<Hides>,
        /// Once placed, moved to the nearest open cell beside this prop (by key).
        pub beside: Option<NameId>,
    }
}

impl PlacementDef {
    /// A story's row: there when the seed found the story a place, and only then. Not in the contract.
    pub fn at_place(&self) -> bool {
        matches!(self.at, PlaceAt::Place { .. })
    }
}

// --- stories ---------------------------------------------------------------------------------

model_enum! {
    /// A tale at a ruin: a roofless cottage or four walls.
    pub enum RuinKind { House, Walls }
}

model! {
    /// Within `max` of the place another story claimed: the second half of a chain.
    pub struct StoryNear {
        pub story: StoryId,
        /// Cells, centre to centre.
        pub max: u16,
    }
}

model! {
    /// Who hears of a story, and when (ARCHITECTURE.md §4.6.e). No row has one yet.
    pub struct Spreads {
        /// The people it is told to, by unit key.
        pub to: &'static [NameId],
        /// After the story's own trigger fires.
        pub after: Tick,
    }
}

model! {
    /// A story at a generated place (`data/stories/*.json`).
    pub struct StoryDef {
        pub id: StoryId,
        /// The content id (`"leckie"`): `{place:leckie}` in the words, tools.
        pub key: &'static str,
        pub kind: PlaceKind,
        pub region: Region,
        /// The threat of the ground under it, 0..=6, inclusive both ends.
        pub threat: (u8, u8),
        /// Cells from the first walk, inclusive both ends.
        pub first: Option<(u16, u16)>,
        pub near: Option<StoryNear>,
        /// A camp of these, any mix, at least `count` standing. Empty: no camp asked for.
        pub hostile: &'static [UnitDefId],
        /// With `hostile`: how many at least.
        pub count: u8,
        /// The quests told here, in the order she meets them.
        pub quests: &'static [QuestId],
        /// A tale (QUEST-TREE.md): its own name, the same on every seed.
        pub tale: bool,
        /// A fixed name; `None` takes the seed's next from the kind's pool.
        pub name: Option<TextId>,
        /// What the board says instead of the kind's usual line.
        pub board: Option<TextId>,
        pub ruin: Option<RuinKind>,
        /// `story_<id>`: the rect round its place and the mark in front of its board.
        pub place_name: NameId,
        pub spreads: Option<Spreads>,
    }
}

// --- providers -------------------------------------------------------------------------------

model_enum! {
    /// What a provided name is in a zone.
    pub enum NameKind {
        Unit,
        Prop,
        Mark,
        Rect,
        /// A named patch of the skeleton (`at.area`, a path's `via`).
        Area,
    }
}

model_enum! {
    /// Which table provides a name.
    pub enum ProvidedBy {
        /// A zone's contract in `zones.json`.
        Zones,
        /// A placement row's key, `hides`, mark or rect, promised on every seed (the county contract).
        Placement,
        /// A placement row at a story's place, or the story's `story_<id>`: there when the story found a place.
        Story,
        /// An anchor's mark and rect.
        Anchor,
        /// A skeleton area's id.
        Area,
        /// A footpath's end marks.
        Path,
        /// A door's prop and mark.
        Door,
    }
}

model! {
    /// One name a county table provides in a zone (ARCHITECTURE.md §5.3 providers).
    pub struct Provided {
        pub zone: ZoneId,
        pub kind: NameKind,
        pub name: NameId,
        pub by: ProvidedBy,
    }
}

// --- the group -------------------------------------------------------------------------------

model! {
    pub struct County {
        /// Indexed by `ZoneId` (`ZoneId::ALL` order, which is tick order).
        pub zones: &'static [ZoneDef],
        /// What the placement rows promise the county on every seed (their keys, `hides`, marks,
        /// rects; not edits, not a story's rows). Part of the county's contract.
        pub promised: Contract,
        /// In row order: the order they are placed in.
        pub sites: &'static [SiteDef],
        /// In row order.
        pub pois: &'static [PoiDef],
        /// In row order: the order they are placed in.
        pub anchors: &'static [AnchorDef],
        /// In row order: the order they are placed in.
        pub areas: &'static [AreaDef],
        /// In row order.
        pub paths: &'static [PathDef],
        /// In row order.
        pub doors: &'static [DoorDef],
        /// One per `PlaceKind`, in `PlaceKind::ALL` order.
        pub names: &'static [NamePool],
        /// In file then row order.
        pub placements: &'static [PlacementDef],
        /// In file then row order: the order decides who gets a contested place.
        pub stories: &'static [StoryDef],
        /// `StoryId` to its index in `stories`.
        pub story_ix: &'static [u16],
    }
}

impl County {
    pub fn zone(&self, z: ZoneId) -> &'static ZoneDef {
        &self.zones[z.index()]
    }

    /// A name in a zone's contract as this group knows it: `zones.json` plus, for the county, what
    /// the placement rows promise. A dungeon's mission binds are the dungeons group's.
    pub fn contract_has(&self, z: ZoneId, kind: NameKind, name: NameId) -> bool {
        self.zone(z).contract.has(kind, name) || (z == ZoneId::County && self.promised.has(kind, name))
    }

    pub fn site(&self, i: u8) -> &'static SiteDef {
        &self.sites[usize::from(i)]
    }

    /// The index of a site by its content id, by a scan: for tools and tests.
    pub fn site_ix(&self, id: &str) -> Option<u8> {
        self.sites.iter().position(|s| s.id == id).map(|i| i as u8)
    }

    pub fn anchor(&self, i: u8) -> &'static AnchorDef {
        &self.anchors[usize::from(i)]
    }

    pub fn area(&self, i: u8) -> &'static AreaDef {
        &self.areas[usize::from(i)]
    }

    pub fn story(&self, id: StoryId) -> &'static StoryDef {
        &self.stories[usize::from(self.story_ix[id.index()])]
    }

    pub fn pool(&self, kind: PlaceKind) -> &'static NamePool {
        &self.names[kind as usize]
    }

    /// Every name these tables provide, zone by zone, for the provider check (ARCHITECTURE.md §5.3):
    /// the `zones.json` contracts; the placement rows' keys (a row with both a unit and a prop names
    /// both), `hides` keys, marks, rects and edit renames; each story's `story_<id>` rect and mark;
    /// each anchor's mark and rect; each area's id; each footpath's end marks; each door's prop and
    /// mark. All in the county but the contracts. A dungeon's mission binds are the dungeons group's.
    pub fn provides(&self) -> Vec<Provided> {
        let mut out = Vec::new();
        let county = |kind, name, by| Provided { zone: ZoneId::County, kind, name, by };
        for z in self.zones {
            out.extend(z.contract.names().map(|(kind, name)| Provided {
                zone: z.id,
                kind,
                name,
                by: ProvidedBy::Zones,
            }));
        }
        for p in self.placements {
            let by = if p.at_place() { ProvidedBy::Story } else { ProvidedBy::Placement };
            if let Some(e) = p.edit {
                if let Some(k) = e.key {
                    out.push(county(NameKind::Prop, k, by));
                }
                continue;
            }
            for k in p.keys {
                if p.unit.is_some() {
                    out.push(county(NameKind::Unit, *k, by));
                }
                if p.prop.is_some() {
                    out.push(county(NameKind::Prop, *k, by));
                }
            }
            if let Some(h) = p.hides {
                out.push(county(NameKind::Prop, h.key, by));
            }
            if let Some(m) = p.mark {
                out.push(county(NameKind::Mark, m, by));
            }
            if let Some(r) = p.rect {
                out.push(county(NameKind::Rect, r.name, by));
            }
        }
        for s in self.stories {
            out.push(county(NameKind::Rect, s.place_name, ProvidedBy::Story));
            out.push(county(NameKind::Mark, s.place_name, ProvidedBy::Story));
        }
        for a in self.anchors {
            out.push(county(NameKind::Mark, a.id, ProvidedBy::Anchor));
            out.push(county(NameKind::Rect, a.id, ProvidedBy::Anchor));
        }
        for a in self.areas {
            out.push(county(NameKind::Area, a.id, ProvidedBy::Area));
        }
        for p in self.paths {
            for m in p.marks {
                out.push(county(NameKind::Mark, m, ProvidedBy::Path));
            }
        }
        for d in self.doors {
            out.push(county(NameKind::Prop, d.key, ProvidedBy::Door));
            if let Some(m) = d.mark {
                out.push(county(NameKind::Mark, m, ProvidedBy::Door));
            }
        }
        out
    }
}
