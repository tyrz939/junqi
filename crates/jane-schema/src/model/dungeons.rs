//! Missions (`data/dungeons/*.json`) and room templates (`data/rooms/<dungeon>/*.room`)
//! (DUNGEONS.md §2.2-2.4, PORT.md §5.3).
//!
//! A mission is the authored order of challenges; a template is a room in a pool; the P3
//! generator (`jane-world::dungeon`) picks one template per node, lays them on the bay lattice
//! and stamps a `Blueprint`. Everything that needs only data is done here, at build: templates
//! are parsed, linted (rules 1-6 of `room.ts`) and turned into every transform they allow
//! (`RoomTemplate::shapes`); missions are linted (`lintDef`, node order, pools, binds, edges,
//! states, the fallback stamp) and every name they can put in a blueprint is interned.
//!
//! **Names.** A socket, mark or rect of a node is named by its bind, else
//! `<zone>_<node>_<local>` with `:` as `_` (generate.ts `localName`). Those names depend on the
//! node and the local id only, never on the template or the seed, so they are interned here:
//! `MissionNode::names` has one row per pool template, index-aligned with that template's
//! sockets, marks and rects. Action lists are compiled through `compile::lists` with `@socket`
//! names interned as written (`"@chest:reward"` is a `NameId` of its own); the generator binds
//! them per node through `MissionNode::at_names`, and `@self` to the holding's `key` (or, in a
//! verb edge's `use`, to the edge's prop name). `MissionDef::provides` lists every name a
//! blueprint of the mission can hold, for the provider check (ARCHITECTURE.md §5.3).

use std::fmt::Write;

use jane_core::action::{ListRef, Stack};
use jane_core::blueprint::TriggerMode;
use jane_core::grid::{Cell, Rect};
use jane_core::ids::{
    DialogueId, DungeonId, NameId, PoolId, PropDefId, SpellId, TemplateId, TextId, UnitDefId, ZoneId,
};
use jane_core::num::{Permille, Tick};
use jane_core::tile::Tile;

use crate::emit::{Emit, EmitDyn};
use crate::{model, model_enum};

// --- room templates ------------------------------------------------------------------------

/// A bay, in cells. Corridors run on the lines between bays and never through a room.
pub const BAY_W: i32 = 36;
pub const BAY_H: i32 = 28;
/// Wall kept round the whole lattice, so a corridor can go round the outside.
pub const BORDER: i32 = 8;
/// A mouth is three cells, centred on this cell of its bay side.
pub const MOUTH_X: i32 = 18;
pub const MOUTH_Y: i32 = 14;

model_enum! {
    /// A side of a room: its doors are named for it (`n1`, `e2`).
    pub enum RoomSide { N, E, S, W }
}

impl RoomSide {
    /// The letter a door id starts with.
    pub const fn letter(self) -> char {
        match self {
            RoomSide::N => 'n',
            RoomSide::E => 'e',
            RoomSide::S => 's',
            RoomSide::W => 'w',
        }
    }
}

model_enum! {
    /// A quarter turn, clockwise.
    pub enum RoomTurn { R0, R90, R180, R270 }
}

impl RoomTurn {
    pub const fn degrees(self) -> u16 {
        match self {
            RoomTurn::R0 => 0,
            RoomTurn::R90 => 90,
            RoomTurn::R180 => 180,
            RoomTurn::R270 => 270,
        }
    }

    pub const fn from_degrees(d: u16) -> Option<RoomTurn> {
        match d {
            0 => Some(RoomTurn::R0),
            90 => Some(RoomTurn::R90),
            180 => Some(RoomTurn::R180),
            270 => Some(RoomTurn::R270),
            _ => None,
        }
    }
}

model_enum! {
    /// What a socket is for (`room.ts` `SocketKind`).
    pub enum RoomSocketKind {
        Chest, Plate, Push, Carry, Lever, Verbprop, Rest, Jar, Page, Notice, Spawn, Boss, Unit, Dress, Prop, Exit,
    }
}

impl RoomSocketKind {
    pub const ALL: [RoomSocketKind; 16] = [
        RoomSocketKind::Chest,
        RoomSocketKind::Plate,
        RoomSocketKind::Push,
        RoomSocketKind::Carry,
        RoomSocketKind::Lever,
        RoomSocketKind::Verbprop,
        RoomSocketKind::Rest,
        RoomSocketKind::Jar,
        RoomSocketKind::Page,
        RoomSocketKind::Notice,
        RoomSocketKind::Spawn,
        RoomSocketKind::Boss,
        RoomSocketKind::Unit,
        RoomSocketKind::Dress,
        RoomSocketKind::Prop,
        RoomSocketKind::Exit,
    ];

    /// The word a `.room` legend writes.
    pub const fn word(self) -> &'static str {
        match self {
            RoomSocketKind::Chest => "chest",
            RoomSocketKind::Plate => "plate",
            RoomSocketKind::Push => "push",
            RoomSocketKind::Carry => "carry",
            RoomSocketKind::Lever => "lever",
            RoomSocketKind::Verbprop => "verbprop",
            RoomSocketKind::Rest => "rest",
            RoomSocketKind::Jar => "jar",
            RoomSocketKind::Page => "page",
            RoomSocketKind::Notice => "notice",
            RoomSocketKind::Spawn => "spawn",
            RoomSocketKind::Boss => "boss",
            RoomSocketKind::Unit => "unit",
            RoomSocketKind::Dress => "dress",
            RoomSocketKind::Prop => "prop",
            RoomSocketKind::Exit => "exit",
        }
    }

    pub fn from_word(w: &str) -> Option<RoomSocketKind> {
        RoomSocketKind::ALL.into_iter().find(|k| k.word() == w)
    }
}

model_enum! {
    /// A co-op affordance a template offers (DUNGEONS.md §2.8).
    pub enum RoomCoop { PlateOrFriend, TwinHold, LureAndLever, CarryRelay, GuardThePusher }
}

impl RoomCoop {
    pub const ALL: [RoomCoop; 5] = [
        RoomCoop::PlateOrFriend,
        RoomCoop::TwinHold,
        RoomCoop::LureAndLever,
        RoomCoop::CarryRelay,
        RoomCoop::GuardThePusher,
    ];

    pub const fn word(self) -> &'static str {
        match self {
            RoomCoop::PlateOrFriend => "plate_or_friend",
            RoomCoop::TwinHold => "twin_hold",
            RoomCoop::LureAndLever => "lure_and_lever",
            RoomCoop::CarryRelay => "carry_relay",
            RoomCoop::GuardThePusher => "guard_the_pusher",
        }
    }

    pub fn from_word(w: &str) -> Option<RoomCoop> {
        RoomCoop::ALL.into_iter().find(|k| k.word() == w)
    }
}

model! {
    /// A door: three cells of the rim, a mouth onto a bay line.
    pub struct RoomDoor {
        /// `"n1"`: side and 1-based bay. In a shape, recomputed from the turned side and bay.
        pub id: &'static str,
        pub side: RoomSide,
        /// 0-based bay along that side.
        pub bay: u8,
        /// The layout must use it.
        pub required: bool,
        /// The centre cell of the three, in grid cells.
        pub x: u16,
        pub y: u16,
    }
}

model! {
    /// A socket: a rectangle the mission fills (a chest, a plate, a boss) or dressing.
    pub struct RoomSocket {
        /// `"chest:reward"`; unnamed sockets are numbered in reading order (`"spawn:0"`) and
        /// dressing by its name (`"dress:pile:0"`).
        pub id: &'static str,
        pub kind: RoomSocketKind,
        /// Top-left, in grid cells.
        pub x: u16,
        pub y: u16,
        /// Footprint, in cells.
        pub w: u16,
        pub h: u16,
        /// Feet cannot cross what stands here (`socketIsSolid`).
        pub solid: bool,
        /// Prop rows a `push` socket the mission leaves empty may hold (`barrel | crate`), each
        /// the socket's size.
        pub options: &'static [PropDefId],
    }
}

model! {
    /// A named cell: a spawn point, a patrol flower, where she lands.
    pub struct RoomMark {
        pub id: &'static str,
        /// In grid cells.
        pub x: u16,
        pub y: u16,
    }
}

model! {
    /// A named rect: `room` (the whole floor, always first), `inner`, `drop:1`.
    pub struct RoomRect {
        pub id: &'static str,
        /// In grid cells.
        pub rect: Rect,
    }
}

model! {
    /// A negative guarantee: `what` stays shut with `until`'s list suppressed. Socket indices.
    pub struct RoomBlock {
        pub what: u16,
        pub until: u16,
    }
}

model! {
    /// A legend tile: grid character to tile (`: tile Track`).
    pub struct RoomTileChar {
        /// An ASCII grid character.
        pub ch: u8,
        pub tile: Tile,
    }
}

model! {
    /// A template as it will be stamped: turned, mirrored, and told where in its bays it sits
    /// (`room.ts` `Shape`). Sockets, marks and rects are index-aligned with the template's.
    pub struct RoomShape {
        pub turn: RoomTurn,
        /// Mirrored left to right before the turn.
        pub mirror: bool,
        /// Grid size in cells.
        pub w: u16,
        pub h: u16,
        /// Bays covered, (columns, rows).
        pub bays: (u8, u8),
        /// Offset of the grid inside its bay group, in cells, so every door lies on its bay's mouth line.
        pub ox: i32,
        pub oy: i32,
        /// Row-major, `w * h` ASCII bytes: `#` wall, `.` floor (sockets and marks are floor here),
        /// `_` sill, any other byte on the rim a door cell, anywhere else a legend tile. Read it
        /// through [`RoomShape::cell`].
        pub cells: &'static str,
        /// In id order (the ids of the turned shape).
        pub doors: &'static [RoomDoor],
        /// Each socket's box in shape cells.
        pub sockets: &'static [Rect],
        pub marks: &'static [Cell],
        pub rects: &'static [Rect],
    }
}

/// What a cell of a shape is before a layout decides which doors are in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomCell {
    Wall,
    Floor,
    /// Floor just inside a door: kept as `Tile::Sill` under a door in use, floor otherwise.
    Sill,
    /// A door cell of the rim: floor when the door is in use, wall otherwise.
    Door,
    Tile(Tile),
}

impl RoomShape {
    /// The cell at `(x, y)` of this shape; `tiles` is the template's legend.
    pub fn cell(&self, tiles: &[RoomTileChar], x: u16, y: u16) -> RoomCell {
        let ch = self.cells.as_bytes()[usize::from(y) * usize::from(self.w) + usize::from(x)];
        let rim = x == 0 || y == 0 || x + 1 == self.w || y + 1 == self.h;
        match ch {
            b'#' => RoomCell::Wall,
            b'.' => RoomCell::Floor,
            b'_' => RoomCell::Sill,
            _ if rim => RoomCell::Door,
            _ => tiles.iter().find(|t| t.ch == ch).map_or(RoomCell::Wall, |t| RoomCell::Tile(t.tile)),
        }
    }
}

model! {
    /// A room template (`data/rooms/<dungeon>/<name>.room`), parsed, linted and transformed.
    pub struct RoomTemplate {
        /// `"mine.plate.a"`.
        pub id: &'static str,
        pub pool: PoolId,
        /// Bays covered untransformed, (columns, rows).
        pub bays: (u8, u8),
        /// The turns the file allows, as written (0 is always among them).
        pub turns: &'static [RoomTurn],
        /// Each turn may also be mirrored.
        pub mirror: bool,
        /// Grid size in cells, untransformed.
        pub w: u16,
        pub h: u16,
        /// In id order.
        pub doors: &'static [RoomDoor],
        /// In reading order of their top-left cell.
        pub sockets: &'static [RoomSocket],
        pub marks: &'static [RoomMark],
        /// `room` first, then the header's `rect` lines.
        pub rects: &'static [RoomRect],
        /// What the player must bring: spells.
        pub needs_verbs: &'static [SpellId],
        /// What the player must bring: items.
        pub needs_items: &'static [Stack],
        /// Socket indices she can then reach.
        pub grants: &'static [u16],
        pub blocks: &'static [RoomBlock],
        pub coop: &'static [RoomCoop],
        /// Most enemy cost points the room holds fairly.
        pub heat_max: u16,
        /// The legend's tiles.
        pub tiles: &'static [RoomTileChar],
        /// Every transform the template allows, in `shapesOf` order (each turn as written,
        /// unmirrored then mirrored); every one fits its bays (lint).
        pub shapes: &'static [RoomShape],
    }
}

impl RoomTemplate {
    /// The shape for a turn and mirror, if the template allows it.
    pub fn shape(&self, turn: RoomTurn, mirror: bool) -> Option<&'static RoomShape> {
        self.shapes.iter().find(|s| s.turn == turn && s.mirror == mirror)
    }

    pub fn socket_index(&self, id: &str) -> Option<usize> {
        self.sockets.iter().position(|s| s.id == id)
    }

    pub fn mark_index(&self, id: &str) -> Option<usize> {
        self.marks.iter().position(|m| m.id == id)
    }

    pub fn rect_index(&self, id: &str) -> Option<usize> {
        self.rects.iter().position(|r| r.id == id)
    }
}

model! {
    /// A pool: templates that share an interface; a mission node names only its pool.
    pub struct RoomPool {
        /// `"mine.plate"`.
        pub id: &'static str,
        /// In file path order (which is id order): "the first template of the pool".
        pub templates: &'static [TemplateId],
    }
}

// --- missions ------------------------------------------------------------------------------

model_enum! {
    /// A node's part in the mission.
    pub enum MissionNodeKind { Entrance, Teach, Fight, Puzzle, Key, Hub, Rest, Miniboss, Verb, Bosskey, Boss, Reward, Exit, Side }
}

model_enum! {
    /// What a name stands for in a blueprint.
    pub enum MissionNameWhat { Prop, Unit, Mark, Rect, Trigger, Flag }
}

model! {
    /// A name a mission's blueprint can hold.
    pub struct MissionName {
        pub name: NameId,
        pub what: MissionNameWhat,
    }
}

model! {
    /// Socket, mark or rect of a template to a contract name.
    pub struct MissionBind {
        /// The template's local id (`"chest:reward"`, `"entry"`, `"room"`).
        pub from: &'static str,
        pub name: NameId,
        pub what: MissionNameWhat,
    }
}

/// What finishing a node gives the mission (checked against its holdings at build).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionGrant {
    /// An `opens` tag.
    Key(NameId),
    Verb(SpellId),
    Item(Stack),
    Flag(NameId),
    /// Control of a state: an index into `MissionDef::states`.
    State(u8),
}

/// The prop a holding stands in its socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionProp {
    Row(PropDefId),
    /// A lamp named by its family (`gas_lamp`): the rows `_n`, `_e`, `_s`, `_w`, hung on the
    /// wall its socket stands against.
    Family([PropDefId; 4]),
}

model! {
    /// A patrol waypoint: a mark of the same template.
    pub struct MissionPatrol {
        pub mark: &'static str,
        /// Ticks to stand there.
        pub dwell: Tick,
    }
}

model! {
    /// Where a door holding leads.
    pub struct MissionDoorTo {
        pub zone: ZoneId,
        pub mark: NameId,
    }
}

model! {
    /// What goes into one of a template's sockets.
    pub struct MissionHolding {
        /// The template socket (`"chest:reward"`).
        pub socket: &'static str,
        /// Its name in the blueprint (the bind, else `<zone>_<node>_<socket>`); what `@self`
        /// means in its lists.
        pub key: NameId,
        /// A unit stands here, not a prop.
        pub unit: Option<UnitDefId>,
        /// For a unit: marks of the template, in order.
        pub patrol: &'static [MissionPatrol],
        /// For anything but a unit: the prop (`chest` when the mission names only loot).
        pub prop: Option<MissionProp>,
        pub loot: Option<&'static [Stack]>,
        pub talk: Option<DialogueId>,
        /// Materials a world verb consumes.
        pub needs: &'static [Stack],
        /// `@socket` names inside are bound per node (`MissionNode::at_names`).
        pub use_list: Option<ListRef>,
        pub release: Option<ListRef>,
        pub locked: bool,
        /// Locked to this key tag (a key the story hands over from outside the zone, which the
        /// mission's `given_keys` then names).
        pub key_tag: Option<NameId>,
        pub hidden: bool,
        /// Built switched on or off; `None` is the prop row's default.
        pub on: Option<bool>,
        pub label: Option<TextId>,
        pub to: Option<MissionDoorTo>,
        /// Locked until these units are dead (resolved names).
        pub guarded_by: &'static [NameId],
        /// The trigger that unlocks a guarded holding (`<key>_free`).
        pub free_trigger: Option<NameId>,
        /// This prop controls a state: an index into `MissionDef::states`.
        pub controls: Option<u8>,
        /// For a control: what else happens on the way to each value of its state, by value index.
        pub becomes: [Option<ListRef>; 2],
    }
}

model! {
    /// A trigger a node carries with it.
    pub struct MissionTrigger {
        pub id: &'static str,
        /// `<zone>_<node>_<id>`.
        pub name: NameId,
        /// The resolved rect (the template's `rect`, `room` by default).
        pub rect: NameId,
        pub mode: TriggerMode,
        pub once: bool,
        pub when: Option<jane_core::action::CondsRef>,
        pub actions: ListRef,
        pub reset: Option<ListRef>,
    }
}

model! {
    /// The names of one node's socket, marks and rects for one template of its pool,
    /// index-aligned with the template's lists.
    pub struct MissionNodeNames {
        pub template: TemplateId,
        pub sockets: &'static [NameId],
        pub marks: &'static [NameId],
        pub rects: &'static [NameId],
    }
}

model! {
    /// One challenge of the mission.
    pub struct MissionNode {
        /// Also the prefix of everything anonymous in it.
        pub id: &'static str,
        pub kind: MissionNodeKind,
        /// Place in the fixed challenge order.
        pub order: u8,
        pub critical: bool,
        pub pool: PoolId,
        pub holds: &'static [MissionHolding],
        pub binds: &'static [MissionBind],
        /// Verbs this node cannot be finished without.
        pub demands: &'static [SpellId],
        pub grants: &'static [MissionGrant],
        /// Share of `budget.base_heat`, 0 to 1300 permille.
        pub heat: Permille,
        /// Enemy cost points: `round(heat * base_heat)`, before the template's `heat_max` cap.
        pub heat_cost: u16,
        pub triggers: &'static [MissionTrigger],
        /// No lamps on its walls.
        pub dark: bool,
        /// One row per template of the pool, in pool order.
        pub names: &'static [MissionNodeNames],
        /// `@socket` placeholder as written to the name it binds to in this node, for every
        /// placeholder its lists (and the `use` of verb edges from it) contain. `@self` is not
        /// here: it is the holding's `key` or the edge's prop.
        pub at_names: &'static [(NameId, NameId)],
    }
}

model! {
    /// A reversible, building-wide mechanism in a world flag: unset or 0 is `values[initial]`,
    /// 1 the other.
    pub struct MissionState {
        pub id: &'static str,
        pub values: [&'static str; 2],
        /// Index into `values`.
        pub initial: u8,
        pub flag: NameId,
    }
}

model! {
    /// Units stood up when a lock-in seals.
    pub struct MissionLockinSpawn {
        pub def: UnitDefId,
        /// Resolved mark names.
        pub at: &'static [NameId],
        /// Unit names, one per mark (`as`, else `<zone>_<node>_lockin_<n>`).
        pub names: &'static [NameId],
    }
}

/// What stands between two nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionEdgeKind {
    Open,
    /// A locked gate an `opens` tag fits.
    Key {
        tag: NameId,
        label: Option<TextId>,
    },
    /// A prop that seals the corridor until a verb is cast on it.
    Verb {
        verb: SpellId,
        prop: PropDefId,
        needs: &'static [Stack],
        /// `propAs`, else `<zone>_<prop>_<from>_<to>`; what `@self` means in `use_list`.
        name: NameId,
        label: Option<TextId>,
        toast: Option<TextId>,
        /// Runs after it opens; `@socket` names bind against the `from` node.
        use_list: Option<ListRef>,
    },
    /// The far room seals on entry and opens on the clear.
    Lockin {
        spawn: Option<MissionLockinSpawn>,
        toast: Option<TextId>,
        /// The chest the clear unlocks.
        reward: Option<NameId>,
        /// `<zone>_<node>_inner` (the template's `inner`, or the whole floor).
        rect: NameId,
        /// `<zone>_<node>_in`: where the way in lands.
        mark: NameId,
        /// `<zone>_<node>_wayin`.
        way_in: NameId,
        /// `<zone>_<node>_lock`.
        lock: NameId,
        /// `<zone>_<node>_clear`: the trigger and the flag it sets.
        clear: NameId,
    },
    /// A gate that opens once `flag` is set.
    Oneway {
        flag: NameId,
        /// `<zone>_<from>_<to>_opens`.
        trigger: NameId,
        /// The gate's label; `None` is the generator's "The gate".
        label: Option<TextId>,
    },
    /// Passable only while a state has a value.
    State {
        var: u8,
        is: u8,
        label: Option<TextId>,
    },
    /// See, do not pass: rides on another edge's corridor.
    Sight,
}

model! {
    /// The gate prop an edge stands in its corridor.
    pub struct MissionGate {
        /// `gateAs`, else `<zone>_gate_<from>_<to>`.
        pub name: NameId,
        /// Rows for a north-south and an east-west corridor: 3x1 and 1x3.
        pub rows: [PropDefId; 2],
    }
}

model! {
    pub struct MissionEdge {
        /// Node indices.
        pub from: u8,
        pub to: u8,
        pub kind: MissionEdgeKind,
        pub also: &'static [MissionEdgeKind],
        pub shortcut: bool,
        /// Set when a `key`, `oneway`, `state` or `lockin` kind puts a gate in the corridor.
        pub gate: Option<MissionGate>,
    }
}

model! {
    pub struct MissionBudget {
        /// Optional rooms placed, least and most.
        pub side_rooms: (u8, u8),
        /// Walking length of the first completion, in cells.
        pub crit_path_cells: (u16, u16),
        /// Where along that walk the rest room is first reached, permille.
        pub rest_at: Option<(Permille, Permille)>,
        /// Most cells from the rest room to the boss, and to a shortcut, everything open.
        pub rest_to_boss_cells: u16,
        /// Enemy cost points for a node with heat 1.
        pub base_heat: u16,
        /// The bestiary: unit and cost points, in unit id order.
        pub enemies: &'static [(UnitDefId, u16)],
    }
}

model! {
    /// What a `dress:<name>` socket may become.
    pub struct MissionDress {
        pub name: &'static str,
        pub props: &'static [PropDefId],
        /// How often it is there at all.
        pub chance: Permille,
    }
}

model! {
    /// How the building is lit (lights.ts).
    pub struct MissionLights {
        /// The lamp family's rows `_n`, `_e`, `_s`, `_w`.
        pub family: [PropDefId; 4],
        /// Cells between lamps along a room's north and south walls.
        pub every: u16,
        /// Cells between lamps along a corridor; 0 leaves corridors dark.
        pub corridor: u16,
        /// Lamps on only while a state (index) has a value (index).
        pub state: Option<(u8, u8)>,
    }
}

model! {
    /// One room of the hand-placed fallback embedding.
    pub struct MissionPlacement {
        /// Node index.
        pub node: u8,
        pub template: TemplateId,
        /// First bay, (column, row).
        pub bay: (u8, u8),
        pub turn: RoomTurn,
        pub mirror: bool,
    }
}

model! {
    /// The names the story may lean on (`contractOf`): binds of critical nodes, and the
    /// `gateAs` and `propAs` of edges between critical nodes.
    pub struct MissionContract {
        pub units: &'static [NameId],
        pub props: &'static [NameId],
        pub marks: &'static [NameId],
        pub rects: &'static [NameId],
    }
}

model! {
    /// A dungeon's mission (`data/dungeons/<zone>.json`).
    pub struct MissionDef {
        pub id: &'static str,
        pub zone: ZoneId,
        pub name: TextId,
        /// DESIGN-2020 §3.1; every unit's `phase`.
        pub phase: u8,
        pub floor: Tile,
        pub wall: Tile,
        pub alt: &'static [Tile],
        pub indoor: bool,
        pub ambient: Permille,
        /// The bay lattice.
        pub cols: u8,
        pub rows: u8,
        /// Spells known on arrival.
        pub given_verbs: &'static [SpellId],
        /// `opens` tags handed over outside.
        pub given_keys: &'static [NameId],
        /// At most three.
        pub states: &'static [MissionState],
        pub nodes: &'static [MissionNode],
        pub edges: &'static [MissionEdge],
        pub budget: MissionBudget,
        pub dress: &'static [MissionDress],
        /// `None`: no lamps at all.
        pub lights: Option<MissionLights>,
        pub fallback: &'static [MissionPlacement],
        /// `<zone>_all`: the whole zone.
        pub all_rect: NameId,
        /// The zone contract this mission derives (`zones.json` marks the dungeon zones
        /// mission-driven and reads theirs from here).
        pub contract: MissionContract,
        /// Every name a blueprint of this mission can hold, sorted by name id.
        pub provides: &'static [MissionName],
    }
}

impl MissionDef {
    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
}

model! {
    pub struct Dungeons {
        /// Indexed by `DungeonId` (file stem order).
        pub missions: &'static [MissionDef],
        /// Indexed by `TemplateId` (id order).
        pub templates: &'static [RoomTemplate],
        /// Indexed by `PoolId` (id order).
        pub pools: &'static [RoomPool],
    }
}

impl Dungeons {
    pub fn mission(&self, id: DungeonId) -> &'static MissionDef {
        &self.missions[id.index()]
    }

    /// The mission that builds a zone, if the zone is a generated dungeon.
    pub fn mission_of(&self, zone: ZoneId) -> Option<&'static MissionDef> {
        self.missions.iter().find(|m| m.zone == zone)
    }

    pub fn template(&self, id: TemplateId) -> &'static RoomTemplate {
        &self.templates[id.index()]
    }

    pub fn pool(&self, id: PoolId) -> &'static RoomPool {
        &self.pools[id.index()]
    }

    /// The templates of a pool, in pool order.
    pub fn templates_of(&self, id: PoolId) -> impl Iterator<Item = &'static RoomTemplate> {
        let templates = self.templates;
        self.pools[id.index()].templates.iter().map(move |t| &templates[t.index()])
    }

    /// By id, by a scan: for tools and tests.
    pub fn template_by_id(&self, id: &str) -> Option<&'static RoomTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// By id, by a scan: for tools and tests.
    pub fn pool_by_id(&self, id: &str) -> Option<PoolId> {
        self.pools.iter().position(|p| p.id == id).map(|i| PoolId(i as u16))
    }
}

// --- emitters for the enums with data ------------------------------------------------------

/// `Name { a: .., b: .. }`, or `Name` alone with no fields.
fn variant(out: &mut String, name: &str, fields: &[(&str, &dyn EmitDyn)]) {
    out.push_str(name);
    if fields.is_empty() {
        return;
    }
    out.push_str(" {");
    for (i, (f, v)) in fields.iter().enumerate() {
        out.push_str(if i == 0 { " " } else { ", " });
        let _ = write!(out, "{f}: ");
        v.emit_dyn(out);
    }
    out.push_str(" }");
}

fn tuple(out: &mut String, name: &str, v: &dyn EmitDyn) {
    out.push_str(name);
    out.push('(');
    v.emit_dyn(out);
    out.push(')');
}

impl Emit for MissionGrant {
    fn emit(&self, out: &mut String) {
        match self {
            MissionGrant::Key(n) => tuple(out, "MissionGrant::Key", n),
            MissionGrant::Verb(s) => tuple(out, "MissionGrant::Verb", s),
            MissionGrant::Item(s) => tuple(out, "MissionGrant::Item", s),
            MissionGrant::Flag(n) => tuple(out, "MissionGrant::Flag", n),
            MissionGrant::State(i) => tuple(out, "MissionGrant::State", i),
        }
    }
}

impl Emit for MissionProp {
    fn emit(&self, out: &mut String) {
        match self {
            MissionProp::Row(p) => tuple(out, "MissionProp::Row", p),
            MissionProp::Family(f) => tuple(out, "MissionProp::Family", f),
        }
    }
}

impl Emit for MissionEdgeKind {
    fn emit(&self, out: &mut String) {
        match self {
            MissionEdgeKind::Open => variant(out, "MissionEdgeKind::Open", &[]),
            MissionEdgeKind::Key { tag, label } => {
                variant(out, "MissionEdgeKind::Key", &[("tag", tag), ("label", label)]);
            }
            MissionEdgeKind::Verb { verb, prop, needs, name, label, toast, use_list } => variant(
                out,
                "MissionEdgeKind::Verb",
                &[
                    ("verb", verb),
                    ("prop", prop),
                    ("needs", needs),
                    ("name", name),
                    ("label", label),
                    ("toast", toast),
                    ("use_list", use_list),
                ],
            ),
            MissionEdgeKind::Lockin { spawn, toast, reward, rect, mark, way_in, lock, clear } => variant(
                out,
                "MissionEdgeKind::Lockin",
                &[
                    ("spawn", spawn),
                    ("toast", toast),
                    ("reward", reward),
                    ("rect", rect),
                    ("mark", mark),
                    ("way_in", way_in),
                    ("lock", lock),
                    ("clear", clear),
                ],
            ),
            MissionEdgeKind::Oneway { flag, trigger, label } => {
                variant(out, "MissionEdgeKind::Oneway", &[("flag", flag), ("trigger", trigger), ("label", label)]);
            }
            MissionEdgeKind::State { var, is, label } => {
                variant(out, "MissionEdgeKind::State", &[("var", var), ("is", is), ("label", label)]);
            }
            MissionEdgeKind::Sight => variant(out, "MissionEdgeKind::Sight", &[]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::to_rust;

    #[test]
    fn enums_with_data_emit_as_rust() {
        assert_eq!(to_rust(&MissionGrant::State(1)), "MissionGrant::State(1)");
        assert_eq!(
            to_rust(&MissionEdgeKind::Key { tag: NameId(3), label: None }),
            "MissionEdgeKind::Key { tag: NameId(3), label: None }"
        );
        assert_eq!(to_rust(&MissionEdgeKind::Sight), "MissionEdgeKind::Sight");
        assert_eq!(
            to_rust(&MissionProp::Family([PropDefId(1), PropDefId(2), PropDefId(3), PropDefId(4)])),
            "MissionProp::Family([PropDefId(1), PropDefId(2), PropDefId(3), PropDefId(4)])"
        );
        assert_eq!(to_rust(&RoomTurn::R90), "RoomTurn::R90");
    }

    #[test]
    fn a_shape_reads_its_cells() {
        let tiles = [RoomTileChar { ch: b':', tile: Tile::Track }];
        let s = RoomShape {
            turn: RoomTurn::R0,
            mirror: false,
            w: 4,
            h: 3,
            bays: (1, 1),
            ox: 0,
            oy: 0,
            cells: "#nn##_:##..#",
            doors: &[],
            sockets: &[],
            marks: &[],
            rects: &[],
        };
        assert_eq!(s.cell(&tiles, 1, 0), RoomCell::Door);
        assert_eq!(s.cell(&tiles, 1, 1), RoomCell::Sill);
        assert_eq!(s.cell(&tiles, 2, 1), RoomCell::Tile(Tile::Track));
        assert_eq!(s.cell(&tiles, 1, 2), RoomCell::Floor);
        assert_eq!(s.cell(&tiles, 0, 2), RoomCell::Wall);
    }
}
