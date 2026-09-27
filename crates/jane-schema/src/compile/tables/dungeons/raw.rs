//! Missions as written (`data/dungeons/<zone>.json`, `jane/src/world/dungeon/types.ts`
//! `DungeonDef`): serde structs, every one `deny_unknown_fields`.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::compile::fraction::Num;
use crate::compile::lists::{RawAction, RawCond};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RawMission {
    pub id: String,
    pub name: String,
    pub phase: u8,
    pub tiles: RawTiles,
    pub indoor: bool,
    pub ambient: Num,
    pub lattice: RawLattice,
    pub given_verbs: Vec<String>,
    pub given_keys: Vec<String>,
    #[serde(default)]
    pub states: Vec<RawState>,
    pub nodes: Vec<RawNode>,
    pub edges: Vec<RawEdge>,
    pub budget: RawBudget,
    pub dress: BTreeMap<String, RawDress>,
    /// Set pieces and the rooms that take them (DUNGEONS.md §2.10).
    #[serde(default)]
    pub sets: RawSets,
    pub lights: Option<RawLights>,
    pub fallback: Vec<RawPlacement>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTiles {
    pub floor: String,
    pub wall: String,
    pub alt: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLattice {
    pub cols: u8,
    pub rows: u8,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawState {
    pub id: String,
    pub values: [String; 2],
    pub initial: String,
    pub flag: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawNodeKind {
    Entrance,
    Teach,
    Fight,
    Puzzle,
    Key,
    Hub,
    Rest,
    Miniboss,
    Verb,
    Bosskey,
    Boss,
    Reward,
    Exit,
    Side,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNode {
    pub id: String,
    pub kind: RawNodeKind,
    pub order: u8,
    pub critical: bool,
    pub pool: String,
    pub holds: Vec<RawHolding>,
    pub binds: Vec<RawBind>,
    pub demands: Vec<String>,
    pub grants: Vec<RawGrant>,
    pub heat: Num,
    #[serde(default)]
    pub triggers: Vec<RawTrigger>,
    #[serde(default)]
    pub dark: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStack {
    pub item: String,
    pub qty: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPatrol {
    pub mark: String,
    /// Ticks.
    pub dwell: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTo {
    pub zone: String,
    pub mark: String,
}

/// The three shapes of `Holding` in one struct; `unit` excludes everything but `patrol`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RawHolding {
    pub socket: String,
    pub loot: Option<Vec<RawStack>>,
    pub unit: Option<String>,
    pub patrol: Option<Vec<RawPatrol>>,
    pub prop: Option<String>,
    pub talk: Option<String>,
    pub needs: Option<Vec<RawStack>>,
    #[serde(rename = "use")]
    pub use_list: Option<Vec<RawAction>>,
    pub release: Option<Vec<RawAction>>,
    pub locked: Option<bool>,
    /// Locked to a key the story hands over from outside (the mission's `givenKeys` names it).
    pub key_tag: Option<String>,
    pub hidden: Option<bool>,
    pub on: Option<bool>,
    pub label: Option<String>,
    pub to: Option<RawTo>,
    pub guarded_by: Option<Vec<String>>,
    pub controls: Option<String>,
    pub becomes: Option<BTreeMap<String, Vec<RawAction>>>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawWhat {
    Prop,
    Unit,
    Mark,
    Rect,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBind {
    pub from: String,
    #[serde(rename = "as")]
    pub name: String,
    pub what: RawWhat,
}

/// `{key} | {verb} | {item, qty} | {flag} | {state}`: exactly one shape, checked after typing.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGrant {
    pub key: Option<String>,
    pub verb: Option<String>,
    pub item: Option<String>,
    pub qty: Option<u16>,
    pub flag: Option<String>,
    pub state: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RawMode {
    Enter,
    While,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTrigger {
    pub id: String,
    pub rect: Option<String>,
    pub mode: Option<RawMode>,
    pub once: Option<bool>,
    pub when: Option<Vec<RawCond>>,
    pub actions: Vec<RawAction>,
    pub reset: Option<Vec<RawAction>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSpawn {
    pub def: String,
    pub at: Vec<String>,
    #[serde(rename = "as")]
    pub names: Option<Vec<String>>,
}

/// Only `opens_on` is built (DUNGEONS.md §2.2, corrected): a gate plus a `while` trigger.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum RawHow {
    #[serde(rename = "opens_on")]
    OpensOn,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase", deny_unknown_fields)]
pub enum RawEdgeKind {
    Open,
    Key {
        tag: String,
        #[serde(rename = "gateAs")]
        gate_as: Option<String>,
        label: Option<String>,
    },
    Verb {
        verb: String,
        prop: String,
        needs: Option<Vec<RawStack>>,
        #[serde(rename = "propAs")]
        prop_as: Option<String>,
        label: Option<String>,
        toast: Option<String>,
        #[serde(rename = "use")]
        use_list: Option<Vec<RawAction>>,
    },
    Lockin {
        #[serde(rename = "gateAs")]
        gate_as: Option<String>,
        spawn: Option<RawSpawn>,
        toast: Option<String>,
        reward: Option<String>,
    },
    Oneway {
        // Read only to refuse anything but `opens_on`.
        #[allow(dead_code)]
        how: RawHow,
        flag: String,
        #[serde(rename = "gateAs")]
        gate_as: Option<String>,
        /// The gate's label (the TypeScript dropped it and wrote "The gate").
        label: Option<String>,
    },
    State {
        var: String,
        is: String,
        #[serde(rename = "gateAs")]
        gate_as: Option<String>,
        gate: Option<[String; 2]>,
        label: Option<String>,
    },
    Sight,
}

impl RawEdgeKind {
    /// `gateAs` of the kinds that put a gate in the corridor.
    pub fn gate_as(&self) -> Option<&str> {
        match self {
            RawEdgeKind::Key { gate_as, .. }
            | RawEdgeKind::Lockin { gate_as, .. }
            | RawEdgeKind::Oneway { gate_as, .. }
            | RawEdgeKind::State { gate_as, .. } => gate_as.as_deref(),
            _ => None,
        }
    }

    /// Does this kind put a gate in the corridor?
    pub fn gates(&self) -> bool {
        matches!(
            self,
            RawEdgeKind::Key { .. }
                | RawEdgeKind::Lockin { .. }
                | RawEdgeKind::Oneway { .. }
                | RawEdgeKind::State { .. }
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawEdge {
    pub from: String,
    pub to: String,
    pub kind: RawEdgeKind,
    #[serde(default)]
    pub shortcut: bool,
    #[serde(default)]
    pub also: Vec<RawEdgeKind>,
}

impl RawEdge {
    /// `[kind, ...also]`.
    pub fn kinds(&self) -> impl Iterator<Item = &RawEdgeKind> {
        std::iter::once(&self.kind).chain(self.also.iter())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RawBudget {
    pub side_rooms: [u8; 2],
    pub crit_path_cells: [u16; 2],
    pub rest_at: Option<[Num; 2]>,
    pub rest_to_boss_cells: u16,
    pub base_heat: u16,
    pub enemies: BTreeMap<String, u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDress {
    pub props: Vec<String>,
    pub chance: Num,
}

/// `"sets": { "pieces": { name: piece }, "rooms": { node: room } }`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSets {
    #[serde(default)]
    pub pieces: BTreeMap<String, RawSetPiece>,
    #[serde(default)]
    pub rooms: BTreeMap<String, RawSetRoom>,
}

/// A set piece: where it stands and its parts, each `[prop, x, y]` in cells from its corner.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSetPiece {
    pub against: String,
    #[serde(default)]
    pub mirror: bool,
    pub parts: Vec<(String, u8, u8)>,
}

/// A room's set pieces: up to `most` of `take`, the first tried first.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSetRoom {
    pub most: u8,
    pub take: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLightState {
    pub var: String,
    pub is: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLights {
    pub prop: String,
    pub every: u16,
    pub corridor: u16,
    pub state: Option<RawLightState>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPlacement {
    pub node: String,
    pub template: String,
    pub bay: [u8; 2],
    pub turn: u16,
    pub mirror: bool,
}
