//! What a solve says: typed errors ([`SolveError`], English only in [`Shown`]'s `Display`) and
//! what it found on the way ([`BuildInfo`]: floods run, cells reached, when each prop was worked,
//! the spells and flags she ends with, and, traced, which flood first reached each cell).

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt;

use jane_core::action::{CondsRef, FlagKey, ListRef, NamesRef};
use jane_core::blueprint::Blueprint;
use jane_core::grid::{Grid, Rect};
use jane_core::ids::{DialogueId, EffectId, ItemId, Key, PropDefId, QuestId, SpellId, TriggerId, UnitDefId};
use jane_data::NameKind;

/// A trigger of a zone: a catalog row, or one the blueprint carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TriggerName {
    Catalog(TriggerId),
    Blueprint(Key),
}

/// Whose list a row fault or a missing name is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Trigger(Key),
    TriggerReset(Key),
    Prop(Key),
    PropRelease(Key),
}

/// What is wrong with a row a blueprint wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowFault {
    UnknownList(ListRef),
    UnknownConds(CondsRef),
    UnknownNames(NamesRef),
    UnknownQuest(QuestId),
    UnknownItem(ItemId),
    UnknownSpell(SpellId),
    UnknownEffect(EffectId),
    UnknownUnitDef(UnitDefId),
    UnknownDialogue(DialogueId),
    /// A strike needs a rect, an amount and a damage school.
    BadStrike,
    /// A grow needs an amount.
    BadGrow,
    /// An `if` needs a `when`.
    EmptyIf,
    /// A reveal needs rects.
    EmptyReveal,
    /// Lists nest deeper than [`super::rows::MAX_DEPTH`] (a list that names itself).
    TooDeep,
}

/// Why a blueprint is refused. A refused candidate is re-rolled, never thrown at the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SolveError {
    // --- the contract, and the names lists lean on
    MissingUnit(Key),
    MissingProp(Key),
    MissingMark(Key),
    MissingRect(Key),
    DuplicateProp(Key),
    DuplicateUnit(Key),
    UnknownPropDef {
        prop: Key,
        def: PropDefId,
    },
    UnknownUnitDef {
        unit: Key,
        def: UnitDefId,
    },
    UnknownLootItem {
        prop: Key,
        item: ItemId,
    },
    UnknownDialogue {
        prop: Key,
        tree: DialogueId,
    },
    /// A trigger the blueprint carries has a catalog row's name.
    TriggerClash(Key),
    TriggerNoRect {
        trigger: TriggerName,
        rect: Key,
    },
    BadRow {
        at: Owner,
        fault: RowFault,
    },
    /// A list names a prop, mark or rect this blueprint does not have.
    NoSuchName {
        at: Owner,
        kind: NameKind,
        name: Key,
    },
    /// A same-zone door leads to a mark this zone does not have.
    DoorToNoMark {
        prop: Key,
        mark: Key,
    },
    TooManyStates(usize),
    /// None of the zone's entrances is a mark of this blueprint (and no `entry` was given).
    NoEntrance,
    // --- walls
    MarkInWall(Key),
    UnitInWall(Key),
    PropOutside(Key),
    // --- the states
    /// A state looks different depending on how it was reached: pulling `control` leaves `prop` otherwise.
    StateLeftDifferently {
        prop: Key,
        control: Key,
    },
    // --- the flood
    MarkUnreachable(Key),
    UnitUnreachable(Key),
    PropUnreachable(Key),
    GateNeverOpens(Key),
}

impl SolveError {
    /// The error in English, with the blueprint's names.
    pub fn show<'b>(&'b self, bp: &'b Blueprint) -> Shown<'b> {
        Shown { e: self, bp }
    }
}

/// A name as content or the generator wrote it.
pub fn name_of(bp: &Blueprint, k: Key) -> &str {
    match k {
        Key::Name(n) => jane_data::catalog().names.get(n.index()).copied().unwrap_or("?"),
        Key::Local(i) => bp.local_names.get(i as usize).map_or("?", String::as_str),
    }
}

/// [`SolveError`] in English.
#[derive(Debug)]
pub struct Shown<'b> {
    e: &'b SolveError,
    bp: &'b Blueprint,
}

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = |k: Key| name_of(self.bp, k);
        let cat = jane_data::catalog();
        let owner = |o: Owner| match o {
            Owner::Trigger(k) => format!("trigger \"{}\"", n(k)),
            Owner::TriggerReset(k) => format!("trigger \"{}\" reset", n(k)),
            Owner::Prop(k) => format!("prop \"{}\"", n(k)),
            Owner::PropRelease(k) => format!("prop \"{}\" release", n(k)),
        };
        let trigger = |t: TriggerName| match t {
            TriggerName::Catalog(id) => cat.story.triggers.get(id.index()).map_or("?", |t| t.id).to_owned(),
            TriggerName::Blueprint(k) => n(k).to_owned(),
        };
        match *self.e {
            SolveError::MissingUnit(k) => write!(f, "missing unit \"{}\"", n(k)),
            SolveError::MissingProp(k) => write!(f, "missing prop \"{}\"", n(k)),
            SolveError::MissingMark(k) => write!(f, "missing mark \"{}\"", n(k)),
            SolveError::MissingRect(k) => write!(f, "missing rect \"{}\"", n(k)),
            SolveError::DuplicateProp(k) => write!(f, "duplicate prop key \"{}\"", n(k)),
            SolveError::DuplicateUnit(k) => write!(f, "duplicate unit key \"{}\"", n(k)),
            SolveError::UnknownPropDef { prop, def } => write!(f, "prop \"{}\": unknown def {def:?}", n(prop)),
            SolveError::UnknownUnitDef { unit, def } => write!(f, "unit \"{}\": unknown def {def:?}", n(unit)),
            SolveError::UnknownLootItem { prop, item } => {
                write!(f, "prop \"{}\": unknown item {item:?} in its loot", n(prop))
            }
            SolveError::UnknownDialogue { prop, tree } => write!(f, "prop \"{}\": unknown dialogue {tree:?}", n(prop)),
            SolveError::TriggerClash(k) => write!(f, "trigger \"{}\" is in the blueprint and in the catalog", n(k)),
            SolveError::TriggerNoRect { trigger: t, rect } => {
                write!(f, "trigger \"{}\": zone has no rect \"{}\"", trigger(t), n(rect))
            }
            SolveError::BadRow { at, fault } => write!(f, "{}: {}", owner(at), fault_text(fault)),
            SolveError::NoSuchName { at, kind, name } => {
                let kind = match kind {
                    NameKind::Unit => "unit",
                    NameKind::Prop => "prop",
                    NameKind::Mark => "mark",
                    NameKind::Rect => "rect",
                    NameKind::Area => "area",
                };
                write!(f, "{}: no {kind} \"{}\" in this zone", owner(at), n(name))
            }
            SolveError::DoorToNoMark { prop, mark } => {
                write!(f, "prop \"{}\": leads to mark \"{}\", which this zone does not have", n(prop), n(mark))
            }
            SolveError::TooManyStates(k) => write!(f, "a zone may have three states at most, and this one has {k}"),
            SolveError::NoEntrance => write!(f, "no entrance: the zone has none of the marks she could come in by"),
            SolveError::MarkInWall(k) => write!(f, "mark \"{}\" is inside a wall", n(k)),
            SolveError::UnitInWall(k) => write!(f, "unit \"{}\" spawns inside a wall", n(k)),
            SolveError::PropOutside(k) => write!(f, "prop \"{}\" is outside the zone", n(k)),
            SolveError::StateLeftDifferently { prop, control } => write!(
                f,
                "states: \"{}\" is left differently depending on how the state was reached (pulling \"{}\")",
                n(prop),
                n(control)
            ),
            SolveError::MarkUnreachable(k) => write!(f, "mark \"{}\" is unreachable", n(k)),
            SolveError::UnitUnreachable(k) => write!(f, "unit \"{}\" is unreachable", n(k)),
            SolveError::PropUnreachable(k) => write!(f, "prop \"{}\" is unreachable", n(k)),
            SolveError::GateNeverOpens(k) => write!(f, "gate \"{}\" can never be opened", n(k)),
        }
    }
}

fn fault_text(fault: RowFault) -> String {
    match fault {
        RowFault::UnknownList(r) => format!("unknown list {r:?}"),
        RowFault::UnknownConds(r) => format!("unknown condition list {r:?}"),
        RowFault::UnknownNames(r) => format!("unknown name list {r:?}"),
        RowFault::UnknownQuest(q) => format!("unknown quest {q:?}"),
        RowFault::UnknownItem(i) => format!("unknown item {i:?}"),
        RowFault::UnknownSpell(s) => format!("unknown spell {s:?}"),
        RowFault::UnknownEffect(e) => format!("unknown effect {e:?}"),
        RowFault::UnknownUnitDef(u) => format!("unknown unit def {u:?}"),
        RowFault::UnknownDialogue(d) => format!("unknown dialogue {d:?}"),
        RowFault::BadStrike => "strike needs a rect, an amount and a damage school".to_owned(),
        RowFault::BadGrow => "grow needs a stat, an amount and the id of the thing found".to_owned(),
        RowFault::EmptyIf => "if needs a \"when\" and a \"then\"".to_owned(),
        RowFault::EmptyReveal => "reveal needs a list of rects".to_owned(),
        RowFault::TooDeep => "lists nest too deep (does one name itself?)".to_owned(),
    }
}

/// What a solve found on the way. Everything but `first_seen` is kept on every solve.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildInfo {
    /// Cells reached (in any state) by the last flood.
    pub reached_cells: u32,
    /// Floods run: the loop's passes.
    pub passes: u16,
    /// Layer floods run (a stateful pass floods each layer it reaches, some more than once).
    pub floods: u32,
    /// Cells those floods visited between them.
    pub cells_visited: u64,
    /// The combinations of the state flags she could bring about, as bit masks (bit `n` is
    /// `ZoneRules::states[n]` set). `[0]` without states.
    pub layers: Vec<u8>,
    /// The flood after which each prop was opened, looted, fired or read.
    pub fired_at: BTreeMap<Key, u16>,
    /// Spells known at the end; `None` when nothing was gated on one.
    pub verbs: Option<Vec<SpellId>>,
    pub flags: BTreeMap<FlagKey, i32>,
    /// Units the solver assumed killed, in key order.
    pub dead: Vec<Key>,
    /// For each cell, the first flood that reached it, or -1. Only with `Options::trace`.
    pub first_seen: Option<Grid<i16>>,
}

impl BuildInfo {
    /// The first flood that reached this cell. `None` unreached, or untraced.
    pub fn first_seen_at(&self, x: i32, y: i32) -> Option<u16> {
        let g = self.first_seen.as_ref()?;
        u16::try_from(g.read(x, y, -1)).ok()
    }

    /// The first flood that reached any cell of `r` (`passOf`). `None` unreached, or untraced.
    pub fn first_in(&self, r: Rect) -> Option<u16> {
        r.cells().filter_map(|(x, y)| self.first_seen_at(x, y)).min()
    }

    pub fn reached_rect(&self, r: Rect) -> bool {
        self.first_in(r).is_some()
    }

    pub fn fired(&self, prop: Key) -> Option<u16> {
        self.fired_at.get(&prop).copied()
    }

    pub fn knows(&self, spell: SpellId) -> bool {
        self.verbs.as_ref().is_none_or(|v| v.contains(&spell))
    }
}

/// A solve's verdict.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Empty: the blueprint holds.
    pub errors: Vec<SolveError>,
    pub info: BuildInfo,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Every error in English.
    pub fn lines(&self, bp: &Blueprint) -> Vec<String> {
        self.errors.iter().map(|e| e.show(bp).to_string()).collect()
    }
}
