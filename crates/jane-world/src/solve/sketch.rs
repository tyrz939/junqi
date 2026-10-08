//! A small blueprint drawn in text, for the solver's tests and for anyone proving a rule on a
//! room of their own (the checks, the template harness): `#` is wall, anything else floor.
//! Names are content names when content has them (`start`, `generic`), generator names otherwise.

use alloc::vec::Vec;
use jane_core::Tile;
use jane_core::action::{Action, Cond, CondsRef, FlagKey, ListRef};
use jane_core::blueprint::{Blueprint, Mark, PropSpawn, UnitSpawn};
use jane_core::grid::{Cell, Grid, Rect};
use jane_core::ids::{ItemId, Key, SpellId, ZoneId};
use jane_data::Catalog;

use super::model::{Options, ZoneRules};
use super::report::Report;

/// A blueprint under construction, with the rules and options it will be solved by.
#[derive(Debug)]
pub struct Sketch {
    pub bp: Blueprint,
    pub rules: ZoneRules,
    pub opts: Options,
    pub cat: &'static Catalog,
}

impl Sketch {
    /// A zone drawn row by row: `#` is `CaveWall`, anything else `CaveFloor`. Rules: [`ZoneRules::open`].
    pub fn new(zone: ZoneId, rows: &[&str]) -> Self {
        let w = rows.first().map_or(0, |r| r.len()) as u32;
        assert!(rows.iter().all(|r| r.len() as u32 == w), "rows of one width");
        let cells =
            rows.iter().flat_map(|r| r.chars()).map(|c| if c == '#' { Tile::CaveWall } else { Tile::CaveFloor });
        let mut bp = Blueprint::new(zone, w, rows.len() as u32, Tile::CaveWall);
        bp.tiles = Grid::from_vec(w, rows.len() as u32, cells.collect());
        Sketch { bp, rules: ZoneRules::open(zone), opts: Options::default(), cat: jane_data::catalog() }
    }

    /// A name: content's when content has it, else the blueprint's own.
    pub fn name(&mut self, s: &str) -> Key {
        match self.cat.name_id(s) {
            Some(n) => Key::Name(n),
            None => self.bp.local(s),
        }
    }

    pub fn flag(&mut self, s: &str) -> FlagKey {
        FlagKey::Named(self.name(s))
    }

    pub fn item(&self, id: &str) -> ItemId {
        self.cat.combat.item_id(id).unwrap_or_else(|| panic!("no item {id}"))
    }

    pub fn spell(&self, id: &str) -> SpellId {
        self.cat.combat.spell_id(id).unwrap_or_else(|| panic!("no spell {id}"))
    }

    pub fn mark(&mut self, name: &str, x: u16, y: u16) -> Key {
        let k = self.name(name);
        self.bp.marks.insert(k, Mark { cell: Cell::new(x, y), facing: None });
        k
    }

    pub fn rect(&mut self, name: &str, x: i32, y: i32, w: i32, h: i32) -> Key {
        let k = self.name(name);
        self.bp.rects.insert(k, Rect::new(x, y, w, h));
        k
    }

    /// A prop of content's def `def` at `(x, y)`, plain: unlocked, shown, holding nothing. Its index.
    pub fn prop(&mut self, key: &str, def: &str, x: u16, y: u16) -> usize {
        let key = self.name(key);
        let def = self.cat.story.prop_id(def).unwrap_or_else(|| panic!("no prop def {def}"));
        self.bp.props.push(PropSpawn::new(key, def, Cell::new(x, y)));
        self.bp.props.len() - 1
    }

    /// A unit of content's def `def` at `(x, y)`. Its index.
    pub fn unit(&mut self, key: &str, def: &str, x: u16, y: u16) -> usize {
        let key = self.name(key);
        let def = self.cat.combat.unit_id(def).unwrap_or_else(|| panic!("no unit def {def}"));
        self.bp.units.push(UnitSpawn { key, def, cell: Cell::new(x, y), facing: None, patrol: Vec::new(), phase: 0 });
        self.bp.units.len() - 1
    }

    pub fn list(&mut self, l: Vec<Action>) -> ListRef {
        self.bp.push_list(l)
    }

    pub fn conds(&mut self, c: Vec<Cond>) -> CondsRef {
        self.bp.push_conds(c)
    }

    /// Solve it.
    pub fn validate(&self) -> Report {
        super::run::solve(&self.bp, &self.rules, &self.opts)
    }

    /// Set a solve up (the checks before the flood must pass) and hand it over, for a pass's test.
    #[cfg(test)]
    pub(crate) fn with(&self, f: impl FnOnce(&mut super::model::Solve<'_>)) {
        match super::model::Solve::new(&self.bp, &self.rules, &self.opts) {
            Ok(mut s) => f(&mut s),
            Err(e) => panic!("{:?}", e.iter().map(|e| e.show(&self.bp).to_string()).collect::<Vec<_>>()),
        }
    }
}
