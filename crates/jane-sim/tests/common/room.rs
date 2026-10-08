//! A room standing in for the county, for rule tests that want exact geometry: the seed's
//! blueprints with the county swapped for a walled floor, `start` at `(6, 16)` facing east.

use std::sync::Arc;

use jane_core::blueprint::{Door, Mark, PropSpawn, Trigger, TriggerMode, UnitSpawn};
use jane_core::{Action, Blueprint, Cell, Key, ListRef, Rect, Stack, Tile, ZoneId};
use jane_sim::{Blueprints, Sim};

pub const W: u32 = 48;
pub const H: u32 = 32;

pub fn def(id: &str) -> jane_core::PropDefId {
    jane_data::catalog().story.prop_id(id).unwrap_or_else(|| panic!("no prop row {id}"))
}

pub fn item(id: &str) -> jane_core::ItemId {
    jane_data::catalog().combat.item_id(id).unwrap_or_else(|| panic!("no item {id}"))
}

/// A prop row with nothing on it.
pub fn spawn(key: Key, row: &str, x: u16, y: u16) -> PropSpawn {
    PropSpawn::new(key, def(row), Cell::new(x, y))
}

/// The builder's hands.
pub struct Room {
    pub bp: Blueprint,
}

impl Room {
    pub fn new(indoor: bool) -> Self {
        let cat = jane_data::catalog();
        let mut bp = Blueprint::new(ZoneId::County, W, H, Tile::Floor);
        bp.indoor = indoor;
        bp.tiles.fill_rect(Rect::new(0, 0, W as i32, 1), Tile::Wall);
        bp.tiles.fill_rect(Rect::new(0, H as i32 - 1, W as i32, 1), Tile::Wall);
        bp.tiles.fill_rect(Rect::new(0, 0, 1, H as i32), Tile::Wall);
        bp.tiles.fill_rect(Rect::new(W as i32 - 1, 0, 1, H as i32), Tile::Wall);
        bp.marks.insert(
            Key::Name(cat.story.start.mark),
            Mark { cell: Cell::new(6, 16), facing: Some(jane_core::action::Facing::East) },
        );
        Self { bp }
    }

    pub fn key(&mut self, name: &str) -> Key {
        match jane_data::catalog().name_id(name) {
            Some(n) => Key::Name(n),
            None => self.bp.local(name),
        }
    }

    pub fn list(&mut self, actions: Vec<Action>) -> ListRef {
        self.bp.push_list(actions)
    }

    /// A prop; `f` fills in its row.
    pub fn prop(&mut self, name: &str, row: &str, x: u16, y: u16, f: impl FnOnce(&mut PropSpawn)) -> Key {
        let k = self.key(name);
        let mut s = spawn(k, row, x, y);
        f(&mut s);
        self.bp.props.push(s);
        k
    }

    pub fn chest(&mut self, name: &str, x: u16, y: u16, loot: &[(&str, u16)]) -> Key {
        let loot = loot.iter().map(|&(i, qty)| Stack { item: item(i), qty }).collect();
        self.prop(name, "chest", x, y, |s| s.loot = loot)
    }

    pub fn door(&mut self, name: &str, x: u16, y: u16, zone: ZoneId, mark: &str) -> Key {
        let mark = self.key(mark);
        self.prop(name, "door", x, y, |s| s.to = Some(Door { zone, mark }))
    }

    pub fn mark(&mut self, name: &str, x: u16, y: u16) -> Key {
        let k = self.key(name);
        self.bp.marks.insert(k, Mark { cell: Cell::new(x, y), facing: None });
        k
    }

    pub fn rect(&mut self, name: &str, r: Rect) -> Key {
        let k = self.key(name);
        self.bp.rects.insert(k, r);
        k
    }

    pub fn unit(&mut self, name: &str, row: &str, x: u16, y: u16) -> Key {
        let k = self.key(name);
        let def = jane_data::catalog().combat.unit_id(row).unwrap();
        self.bp.units.push(UnitSpawn {
            key: k,
            def,
            cell: Cell::new(x, y),
            facing: None,
            patrol: Vec::new(),
            phase: 0,
        });
        k
    }

    pub fn trigger(&mut self, name: &str, r: Rect, mode: TriggerMode, once: bool, actions: Vec<Action>) -> Key {
        let rect = self.rect(name, r);
        let actions = self.list(actions);
        self.bp.triggers.insert(rect, Trigger { rect, mode, once, when: None, actions, reset: None });
        rect
    }

    pub fn build(self) -> Sim {
        let real = super::bps();
        let bp = Arc::new(self.bp);
        let zones =
            std::array::from_fn(|i| if i == 0 { Arc::clone(&bp) } else { Arc::clone(real.get(ZoneId::ALL[i])) });
        Sim::new_game_with(Blueprints::from_parts(super::SEED, zones), "Jane")
    }
}
