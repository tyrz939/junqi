//! The Blueprint: what a zone builder returns and the sim builds a zone from (PORT.md §4:
//! `world` and `sim` share nothing but core types and this). A pure function of
//! `(zone, seed)`, never saved; the save holds the seed and what changed since.
//!
//! Builders address nothing by coordinate outside themselves: story, triggers, dialogue and
//! travel name keys, marks and rects. That contract lets geometry roll per seed while the
//! story stays fixed.
//!
//! **Rule:** a change here is a `core` branch first (PORT.md §11).

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::misc::IndexMap;

use crate::action::{Action, Cond, CondsRef, Facing, ListRef, NamesRef, NightLock, Stack, TextRef};
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
    /// This door is not answered at some hours, and this is what it says instead.
    pub night_lock: Option<NightLock>,
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

/// A named patch of the skeleton as laid in a zone (the county's allotments, the Top Field): its
/// name and the square it covers. The ecology keeps a pressure per area, in this list's order
/// (ARCHITECTURE.md §4.6.c); a unit belongs to the first area whose rect holds its home cell.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Area {
    pub name: Key,
    pub rect: Rect,
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
    /// The county only: the skeleton's patches as placed, in the skeleton's order (§4.6.c).
    pub areas: Vec<Area>,
    /// The county only: the region under each part of it, whose sky rains there (§4.6.b).
    /// Empty for every other zone, which is under its zone's sky.
    pub regions: RegionMap,
    /// A generated dungeon only: where nothing follows her (WoW's instance edge, PLAN.md §2.6
    /// *Leash*): its rest room's floor and the threshold of each way out of the zone. A foe whose
    /// quarry stands in one, or that would itself step in, lets her go and evades home.
    pub sanctuary: Vec<Rect>,
}

/// Which region each part of a zone lies in, on a coarse grid: the county's is the skeleton's
/// macro grid (125 x 125, 16 cells to a macro cell). A byte is a region's index in region order
/// (`jane_data::Region`: 0 Lowfields, 1 Waters, 2 Works); core does not know the names. Empty
/// for a zone under one sky.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct RegionMap {
    /// Cells to a map cell, each way.
    pub scale: u16,
    pub w: u16,
    pub h: u16,
    /// Row-major, `w * h` bytes.
    pub cells: Vec<u8>,
}

impl RegionMap {
    /// A map of `w x h` map cells of `scale` cells each, all region `fill`.
    pub fn new(scale: u16, w: u16, h: u16, fill: u8) -> Self {
        Self { scale, w, h, cells: vec![fill; usize::from(w) * usize::from(h)] }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Set the region of map cell `(mx, my)`; outside is a no-op.
    pub fn set(&mut self, mx: u16, my: u16, region: u8) {
        if mx < self.w && my < self.h {
            self.cells[usize::from(my) * usize::from(self.w) + usize::from(mx)] = region;
        }
    }

    /// The region under cell `(x, y)`; `None` off the map or on an empty one.
    pub fn region_at(&self, x: i32, y: i32) -> Option<u8> {
        if self.scale == 0 || x < 0 || y < 0 {
            return None;
        }
        let s = i32::from(self.scale);
        let (mx, my) = (x / s, y / s);
        if mx >= i32::from(self.w) || my >= i32::from(self.h) {
            return None;
        }
        self.cells.get(my as usize * usize::from(self.w) + mx as usize).copied()
    }
}

impl Blueprint {
    /// Every list held at exactly its length (PORT.md §13.3): a finished blueprint is read, never
    /// grown, so the spare capacity its builder left is given back. Nothing it holds changes.
    pub fn shrink_to_fit(&mut self) {
        self.tiles.shrink_to_fit();
        self.units.shrink_to_fit();
        for u in &mut self.units {
            u.patrol.shrink_to_fit();
        }
        self.props.shrink_to_fit();
        for p in &mut self.props {
            p.loot.shrink_to_fit();
            p.needs.shrink_to_fit();
        }
        self.marks.shrink_to_fit();
        self.rects.shrink_to_fit();
        self.triggers.shrink_to_fit();
        self.stories.shrink_to_fit();
        for s in self.stories.values_mut() {
            if let StoryPlace::Placed { path, .. } = s {
                path.shrink_to_fit();
            }
        }
        self.paint.shrink_to_fit();
        self.lists.shrink_to_fit();
        self.lists.iter_mut().for_each(Vec::shrink_to_fit);
        self.conds.shrink_to_fit();
        self.conds.iter_mut().for_each(Vec::shrink_to_fit);
        self.name_lists.shrink_to_fit();
        self.name_lists.iter_mut().for_each(Vec::shrink_to_fit);
        self.texts.shrink_to_fit();
        self.texts.iter_mut().for_each(String::shrink_to_fit);
        self.local_names.shrink_to_fit();
        self.local_names.iter_mut().for_each(String::shrink_to_fit);
        self.areas.shrink_to_fit();
        self.regions.cells.shrink_to_fit();
        self.sanctuary.shrink_to_fit();
    }

    /// An empty zone of `w x h` cells of `fill`.
    pub fn new(zone: ZoneId, w: u32, h: u32, fill: Tile) -> Self {
        Self {
            zone,
            name: TextRef::Local(0),
            tiles: Grid::new(w, h, fill),
            units: Vec::new(),
            props: Vec::new(),
            marks: IndexMap::default(),
            rects: IndexMap::default(),
            indoor: false,
            ambient: Permille::ONE,
            attempts: 1,
            triggers: IndexMap::default(),
            stories: IndexMap::default(),
            paint: Vec::new(),
            lists: Vec::new(),
            conds: Vec::new(),
            name_lists: Vec::new(),
            texts: vec![String::new()],
            local_names: Vec::new(),
            areas: Vec::new(),
            regions: RegionMap::default(),
            sanctuary: Vec::new(),
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

    /// The region under cell `(x, y)` by the zone's [`RegionMap`]; `None` for a zone with none.
    pub fn region_at(&self, x: i32, y: i32) -> Option<u8> {
        self.regions.region_at(x, y)
    }

    /// The index of the first area whose rect holds cell `(x, y)`.
    pub fn area_at(&self, x: i32, y: i32) -> Option<usize> {
        self.areas.iter().position(|a| a.rect.contains(x, y))
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

    #[test]
    fn a_region_map_answers_by_its_coarse_cell() {
        let bp = Blueprint::new(ZoneId::County, 64, 64, Tile::Grass);
        assert_eq!(bp.region_at(3, 3), None, "no map, no answer: the zone's own sky");
        let mut m = RegionMap::new(16, 4, 4, 0);
        m.set(1, 0, 2);
        m.set(3, 3, 1);
        m.set(9, 9, 1);
        assert_eq!(m.region_at(15, 15), Some(0));
        assert_eq!(m.region_at(16, 0), Some(2));
        assert_eq!(m.region_at(31, 15), Some(2));
        assert_eq!(m.region_at(63, 63), Some(1));
        assert_eq!(m.region_at(64, 0), None);
        assert_eq!(m.region_at(-1, 0), None);
    }
}
