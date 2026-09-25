//! The Blueprint: what a zone builder returns and the sim builds a zone from (PORT.md §4:
//! `world` and `sim` share nothing but core types and this). A pure function of
//! `(zone, seed)`, never saved; the save holds the seed and what changed since.
//!
//! Builders address nothing by coordinate outside themselves: story, triggers, dialogue and
//! travel name keys, marks and rects. That contract lets geometry roll per seed while the
//! story stays fixed.
//!
//! **Rule:** a change here is a `core` branch first (PORT.md §11).

use indexmap::IndexMap;

use crate::action::{Action, Cond, CondsRef, Facing, ListRef, NamesRef, Stack, TextRef};
use crate::grid::{Cell, Grid, Rect};
use crate::ids::{DialogueId, Key, PropDefId, StoryId, UnitDefId, ZoneId};
use crate::num::{Permille, Tick};
use crate::tile::{Material, Tile};

/// Candidates a zone rolls for one seed before it gives up; a generated dungeon spends the
/// last on its hand-placed fallback, so it never does.
pub const ZONE_ATTEMPTS: u8 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Mark {
    pub cell: Cell,
    pub facing: Option<Facing>,
}

/// A patrol waypoint; `dwell` is how long to stand there before walking on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Waypoint {
    pub cell: Cell,
    pub dwell: Tick,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct UnitSpawn {
    pub key: Key,
    pub def: UnitDefId,
    pub cell: Cell,
    pub facing: Option<Facing>,
    pub patrol: Vec<Waypoint>,
    /// The threat (1..=6) of the ground it stands on; 0 = the row as written.
    pub phase: u8,
}

/// Where a door leads.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Door {
    pub zone: ZoneId,
    pub mark: Key,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PropSpawn {
    pub key: Key,
    pub def: PropDefId,
    pub cell: Cell,
    pub locked: bool,
    pub key_tag: Option<Key>,
    pub hidden: bool,
    pub on: bool,
    pub to: Option<Door>,
    pub loot: Vec<Stack>,
    pub use_list: Option<ListRef>,
    /// Pressure plates: runs when the last thing steps off.
    pub release: Option<ListRef>,
    /// Materials a world verb (Repair) consumes from the caster's bag.
    pub needs: Vec<Stack>,
    pub talk: Option<DialogueId>,
    pub label: Option<TextRef>,
    /// This door does not open after dark, and this is what it says instead.
    pub night_lock: Option<TextRef>,
    /// The hidden prop this one lies on, shown when this one is pushed off it...
    pub under: Option<Key>,
    /// ...and only while these hold.
    pub under_when: Option<CondsRef>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TriggerMode {
    /// Fires on walking in.
    Enter,
    /// Fires once inside and `when` holds.
    While,
}

/// A trigger: a rect, when it fires, what it does, and what undoes it when the party dies.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Trigger {
    pub rect: Key,
    pub mode: TriggerMode,
    pub once: bool,
    pub when: Option<CondsRef>,
    pub actions: ListRef,
    /// Run when the player dies after this fired; the trigger then re-arms.
    pub reset: Option<ListRef>,
}

/// Where a story found its place on this seed, or why it found none.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum StoryPlace {
    Placed { kind: Key, name: TextRef, bounds: Rect, path: Vec<Cell> },
    Skipped(TextRef),
}

/// A zone as built.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Blueprint {
    pub zone: ZoneId,
    pub name: TextRef,
    pub tiles: Grid<Tile>,
    pub units: Vec<UnitSpawn>,
    pub props: Vec<PropSpawn>,
    pub marks: IndexMap<Key, Mark>,
    pub rects: IndexMap<Key, Rect>,
    /// Interiors ignore the day clock and keep a fine fog.
    pub indoor: bool,
    /// Ambient light for interiors.
    pub ambient: Permille,
    /// Candidates rolled; more than 1 means one failed validation and was re-rolled.
    pub attempts: u8,
    /// Trigger rows the builder wrote itself, by name; merged with the catalog's for this zone.
    pub triggers: IndexMap<Key, Trigger>,
    /// The county only: where each story landed.
    pub stories: IndexMap<StoryId, StoryPlace>,
    /// Render-only material over terrain, in paint order (PORT.md §6.i).
    pub paint: Vec<(Rect, Material)>,
    /// Action lists `ListRef::Blueprint` indexes.
    pub lists: Vec<Vec<Action>>,
    /// Condition lists `CondsRef::Blueprint` indexes.
    pub conds: Vec<Vec<Cond>>,
    /// Name lists `NamesRef::Blueprint` indexes.
    pub name_lists: Vec<Vec<Key>>,
    /// Strings the generator wrote: a dungeon's name, a story's place name.
    pub texts: Vec<String>,
    /// Names the generator made: `Key::Local(i)` is `local_names[i]`.
    pub local_names: Vec<String>,
}

impl Blueprint {
    /// An empty zone of `w x h` cells of `fill`.
    pub fn new(zone: ZoneId, w: u32, h: u32, fill: Tile) -> Self {
        Self {
            zone,
            name: TextRef::Local(0),
            tiles: Grid::new(w, h, fill),
            units: Vec::new(),
            props: Vec::new(),
            marks: IndexMap::new(),
            rects: IndexMap::new(),
            indoor: false,
            ambient: Permille::ONE,
            attempts: 1,
            triggers: IndexMap::new(),
            stories: IndexMap::new(),
            paint: Vec::new(),
            lists: Vec::new(),
            conds: Vec::new(),
            name_lists: Vec::new(),
            texts: vec![String::new()],
            local_names: Vec::new(),
        }
    }

    pub fn w(&self) -> u32 {
        self.tiles.w()
    }

    pub fn h(&self) -> u32 {
        self.tiles.h()
    }

    /// Intern a generator-made name. The same string gives the same key.
    pub fn local(&mut self, name: &str) -> Key {
        if let Some(i) = self.local_names.iter().position(|n| n == name) {
            return Key::Local(i as u32);
        }
        self.local_names.push(name.to_owned());
        Key::Local(self.local_names.len() as u32 - 1)
    }

    pub fn push_list(&mut self, list: Vec<Action>) -> ListRef {
        self.lists.push(list);
        ListRef::Blueprint(self.lists.len() as u16 - 1)
    }

    pub fn push_conds(&mut self, conds: Vec<Cond>) -> CondsRef {
        self.conds.push(conds);
        CondsRef::Blueprint(self.conds.len() as u16 - 1)
    }

    pub fn push_names(&mut self, names: Vec<Key>) -> NamesRef {
        self.name_lists.push(names);
        NamesRef::Blueprint(self.name_lists.len() as u16 - 1)
    }

    pub fn push_text(&mut self, text: String) -> TextRef {
        self.texts.push(text);
        TextRef::Local(self.texts.len() as u16 - 1)
    }

    pub fn list(&self, r: ListRef) -> Option<&[Action]> {
        match r {
            ListRef::Blueprint(i) => self.lists.get(usize::from(i)).map(Vec::as_slice),
            ListRef::Catalog(_) => None,
        }
    }

    pub fn conds_of(&self, r: CondsRef) -> Option<&[Cond]> {
        match r {
            CondsRef::Blueprint(i) => self.conds.get(usize::from(i)).map(Vec::as_slice),
            CondsRef::Catalog(_) => None,
        }
    }

    pub fn names_of(&self, r: NamesRef) -> Option<&[Key]> {
        match r {
            NamesRef::Blueprint(i) => self.name_lists.get(usize::from(i)).map(Vec::as_slice),
            NamesRef::Catalog(_) => None,
        }
    }

    pub fn text(&self, r: TextRef) -> Option<&str> {
        match r {
            TextRef::Local(i) => self.texts.get(usize::from(i)).map(String::as_str),
            TextRef::Text(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals_intern_and_lists_index() {
        let mut bp = Blueprint::new(ZoneId::Mine, 10, 8, Tile::CaveWall);
        let a = bp.local("gate_3");
        assert_eq!(bp.local("gate_3"), a);
        assert_ne!(bp.local("gate_4"), a);
        let l = bp.push_list(vec![Action::Show(a)]);
        assert_eq!(bp.list(l), Some(&[Action::Show(a)][..]));
        let t = bp.push_text("The Old Adit".into());
        assert_eq!(bp.text(t), Some("The Old Adit"));
        assert_eq!((bp.w(), bp.h()), (10, 8));
    }
}
