//! A blueprint's hash (PORT.md §9.3 gates 1 and 2, ARCHITECTURE.md §8 `cross_target_hash`): one
//! stable 64-bit number per `(zone, seed)` that says the same world was built.
//!
//! It is FNV-1a 64 over explicit little-endian bytes, field by field, in a fixed order: tiles,
//! units, props, marks, rects, triggers, stories, paint, the lists, conditions and name lists,
//! texts and generator-made names. Never through `usize` (every count is a `u32`), never through
//! `Debug` or `std::hash` (whose layouts are the compiler's to change), and every enum by an
//! explicit tag, so a hash is the same on every target and every toolchain. Ids hash as their
//! content index: the same content, the same hash; a content change moves every hash, which is
//! why the fixture file sits beside the content hash that made it.
//!
//! Every match below is exhaustive: a new field, verb or condition is a compile error here.

use alloc::string::String;
use alloc::vec::Vec;
use jane_core::action::{
    Action, CameraMode, Cond, Condition, CondsRef, Facing, FactKey, FlagKey, FlagOp, FlagTest, Heal, ListRef, NamesRef,
    NightLock, School, Stack, Stat, TextRef, Thing,
};
use jane_core::blueprint::{Door, Mark, PropSpawn, StoryPlace, Trigger, TriggerMode, UnitSpawn, Waypoint};
use jane_core::grid::{Cell, Rect};
use jane_core::ids::Key;
use jane_core::tile::Material;
use jane_core::{Blueprint, Tile};

const FNV64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Bump when the byte layout below changes, so an old fixture cannot match by accident.
pub const LAYOUT: u32 = 4;

/// FNV-1a 64 over bytes written little-endian.
#[derive(Clone, Copy, Debug)]
pub struct Hash64(u64);

impl Default for Hash64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash64 {
    pub const fn new() -> Self {
        Self(FNV64_OFFSET)
    }

    pub fn byte(&mut self, b: u8) {
        self.0 = (self.0 ^ u64::from(b)).wrapping_mul(FNV64_PRIME);
    }

    pub fn bytes(&mut self, bs: &[u8]) {
        for &b in bs {
            self.byte(b);
        }
    }

    pub fn u8(&mut self, v: u8) {
        self.byte(v);
    }

    pub fn bool(&mut self, v: bool) {
        self.byte(u8::from(v));
    }

    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn i16(&mut self, v: i16) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }

    /// A count, as a `u32`: never a `usize`.
    pub fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("fewer than 2^32 of anything in a blueprint"));
    }

    /// UTF-8 bytes after their length.
    pub fn str(&mut self, s: &str) {
        self.count(s.len());
        self.bytes(s.as_bytes());
    }

    pub fn finish(self) -> u64 {
        self.0
    }
}

/// Something a blueprint is made of, written into a [`Hash64`].
pub trait Feed {
    fn feed(&self, h: &mut Hash64);
}

impl<T: Feed> Feed for Option<T> {
    fn feed(&self, h: &mut Hash64) {
        match self {
            None => h.u8(0),
            Some(v) => {
                h.u8(1);
                v.feed(h);
            }
        }
    }
}

impl<T: Feed> Feed for [T] {
    fn feed(&self, h: &mut Hash64) {
        h.count(self.len());
        for v in self {
            v.feed(h);
        }
    }
}

impl<T: Feed> Feed for Vec<T> {
    fn feed(&self, h: &mut Hash64) {
        self.as_slice().feed(h);
    }
}

impl Feed for bool {
    fn feed(&self, h: &mut Hash64) {
        h.bool(*self);
    }
}

impl Feed for String {
    fn feed(&self, h: &mut Hash64) {
        h.str(self);
    }
}

impl Feed for Key {
    fn feed(&self, h: &mut Hash64) {
        match *self {
            Key::Name(n) => {
                h.u8(0);
                h.u16(n.0);
            }
            Key::Local(i) => {
                h.u8(1);
                h.u32(i);
            }
        }
    }
}

impl Feed for Cell {
    fn feed(&self, h: &mut Hash64) {
        h.u16(self.x);
        h.u16(self.y);
    }
}

impl Feed for Rect {
    fn feed(&self, h: &mut Hash64) {
        h.i32(self.x);
        h.i32(self.y);
        h.i32(self.w);
        h.i32(self.h);
    }
}

impl Feed for Tile {
    fn feed(&self, h: &mut Hash64) {
        h.u8(self.id());
    }
}

impl Feed for Facing {
    fn feed(&self, h: &mut Hash64) {
        h.u8(*self as u8);
    }
}

impl Feed for Stack {
    fn feed(&self, h: &mut Hash64) {
        h.u16(self.item.0);
        h.u16(self.qty);
    }
}

impl Feed for ListRef {
    fn feed(&self, h: &mut Hash64) {
        let (t, i) = match *self {
            ListRef::Catalog(i) => (0, i),
            ListRef::Blueprint(i) => (1, i),
        };
        h.u8(t);
        h.u16(i);
    }
}

impl Feed for CondsRef {
    fn feed(&self, h: &mut Hash64) {
        let (t, i) = match *self {
            CondsRef::Catalog(i) => (0, i),
            CondsRef::Blueprint(i) => (1, i),
        };
        h.u8(t);
        h.u16(i);
    }
}

impl Feed for NamesRef {
    fn feed(&self, h: &mut Hash64) {
        let (t, i) = match *self {
            NamesRef::Catalog(i) => (0, i),
            NamesRef::Blueprint(i) => (1, i),
        };
        h.u8(t);
        h.u16(i);
    }
}

impl Feed for TextRef {
    fn feed(&self, h: &mut Hash64) {
        match *self {
            TextRef::Text(t) => {
                h.u8(0);
                h.u16(t.0);
            }
            TextRef::Local(i) => {
                h.u8(1);
                h.u16(i);
            }
        }
    }
}

impl Feed for NightLock {
    fn feed(&self, h: &mut Hash64) {
        self.says.feed(h);
        h.u8(self.from);
        h.u8(self.to);
        h.bool(self.keyed);
    }
}

impl Feed for FlagKey {
    fn feed(&self, h: &mut Hash64) {
        let (t, k) = match *self {
            FlagKey::Named(k) => (0, k),
            FlagKey::Been(k) => (1, k),
            FlagKey::Dead(k) => (2, k),
        };
        h.u8(t);
        k.feed(h);
    }
}

fn school(s: School) -> u8 {
    match s {
        School::Heal => 0,
        School::Physical => 1,
        School::Frost => 2,
        School::Fire => 3,
        School::Nature => 4,
        School::Blast => 5,
        School::Shock => 6,
    }
}

impl Feed for Waypoint {
    fn feed(&self, h: &mut Hash64) {
        self.cell.feed(h);
        h.u32(self.dwell.0);
    }
}

impl Feed for UnitSpawn {
    fn feed(&self, h: &mut Hash64) {
        let UnitSpawn { key, def, cell, facing, patrol, phase } = self;
        key.feed(h);
        h.u16(def.0);
        cell.feed(h);
        facing.feed(h);
        patrol.feed(h);
        h.u8(*phase);
    }
}

impl Feed for Door {
    fn feed(&self, h: &mut Hash64) {
        h.u8(self.zone as u8);
        self.mark.feed(h);
    }
}

impl Feed for PropSpawn {
    fn feed(&self, h: &mut Hash64) {
        let PropSpawn {
            key,
            def,
            cell,
            locked,
            key_tag,
            hidden,
            on,
            to,
            loot,
            use_list,
            release,
            needs,
            talk,
            label,
            night_lock,
            under,
            under_when,
        } = self;
        key.feed(h);
        h.u16(def.0);
        cell.feed(h);
        h.bool(*locked);
        key_tag.feed(h);
        h.bool(*hidden);
        h.bool(*on);
        to.feed(h);
        loot.feed(h);
        use_list.feed(h);
        release.feed(h);
        needs.feed(h);
        match talk {
            None => h.u8(0),
            Some(t) => {
                h.u8(1);
                h.u16(t.0);
            }
        }
        label.feed(h);
        night_lock.feed(h);
        under.feed(h);
        under_when.feed(h);
    }
}

impl Feed for Mark {
    fn feed(&self, h: &mut Hash64) {
        self.cell.feed(h);
        self.facing.feed(h);
    }
}

impl Feed for Trigger {
    fn feed(&self, h: &mut Hash64) {
        let Trigger { rect, mode, once, when, actions, reset } = self;
        rect.feed(h);
        h.u8(match mode {
            TriggerMode::Enter => 0,
            TriggerMode::While => 1,
        });
        h.bool(*once);
        when.feed(h);
        actions.feed(h);
        reset.feed(h);
    }
}

impl Feed for StoryPlace {
    fn feed(&self, h: &mut Hash64) {
        match self {
            StoryPlace::Placed { kind, name, bounds, path } => {
                h.u8(0);
                kind.feed(h);
                name.feed(h);
                bounds.feed(h);
                path.feed(h);
            }
            StoryPlace::Skipped(t) => {
                h.u8(1);
                t.feed(h);
            }
        }
    }
}

impl Feed for Material {
    fn feed(&self, h: &mut Hash64) {
        h.u8(match self {
            Material::RoofSlate => 0,
            Material::RoofThatch => 1,
            Material::BrickWall => 2,
            Material::Pine => 3,
            Material::WildEarth => 4,
        });
    }
}

/// Each verb's tag in the hash: its place in `Action` as written today, fixed here so a new
/// verb added anywhere in `Action` does not move the others.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Verb {
    Quest,
    HandIn,
    Flag,
    Rest,
    Grow,
    Give,
    Take,
    Learn,
    Toast,
    Read,
    Lock,
    Unlock,
    Show,
    Hide,
    Switch,
    Spawn,
    Despawn,
    Aggro,
    Location,
    Fill,
    Strike,
    Status,
    Heal,
    Travel,
    Talk,
    Throw,
    Shake,
    Camera,
    If,
    Send,
    Reveal,
    Place,
    NightLock,
    NightUnlock,
    SwitchAll,
    Ring,
}

impl Feed for Action {
    fn feed(&self, h: &mut Hash64) {
        match *self {
            Action::Quest(q) => {
                h.u8(Verb::Quest as u8);
                h.u16(q.0);
            }
            Action::HandIn(q) => {
                h.u8(Verb::HandIn as u8);
                h.u16(q.0);
            }
            Action::Flag { key, op } => {
                h.u8(Verb::Flag as u8);
                key.feed(h);
                match op {
                    FlagOp::Set(v) => {
                        h.u8(0);
                        h.i32(v);
                    }
                    FlagOp::Add(v) => {
                        h.u8(1);
                        h.i32(v);
                    }
                }
            }
            Action::Rest { until } => {
                h.u8(Verb::Rest as u8);
                match until {
                    None => h.u8(0),
                    Some(u) => {
                        h.u8(1);
                        h.u8(u);
                    }
                }
            }
            Action::Grow { stat, amount, id } => {
                h.u8(Verb::Grow as u8);
                h.u8(match stat {
                    Stat::Strength => 0,
                    Stat::Spirit => 1,
                });
                h.i16(amount);
                id.feed(h);
            }
            Action::Give(s) => {
                h.u8(Verb::Give as u8);
                s.feed(h);
            }
            Action::Take(s) => {
                h.u8(Verb::Take as u8);
                s.feed(h);
            }
            Action::Learn(s) => {
                h.u8(Verb::Learn as u8);
                h.u16(s.0);
            }
            Action::Toast(t) => {
                h.u8(Verb::Toast as u8);
                t.feed(h);
            }
            Action::Read(t) => {
                h.u8(Verb::Read as u8);
                t.feed(h);
            }
            Action::Lock(k) => {
                h.u8(Verb::Lock as u8);
                k.feed(h);
            }
            Action::Unlock(k) => {
                h.u8(Verb::Unlock as u8);
                k.feed(h);
            }
            Action::Show(k) => {
                h.u8(Verb::Show as u8);
                k.feed(h);
            }
            Action::Hide(k) => {
                h.u8(Verb::Hide as u8);
                k.feed(h);
            }
            Action::Switch { prop, on } => {
                h.u8(Verb::Switch as u8);
                prop.feed(h);
                on.feed(h);
            }
            Action::Spawn { key, def, at } => {
                h.u8(Verb::Spawn as u8);
                key.feed(h);
                h.u16(def.0);
                at.feed(h);
            }
            Action::Despawn(k) => {
                h.u8(Verb::Despawn as u8);
                k.feed(h);
            }
            Action::Aggro(k) => {
                h.u8(Verb::Aggro as u8);
                k.feed(h);
            }
            Action::Location(k) => {
                h.u8(Verb::Location as u8);
                k.feed(h);
            }
            Action::Fill { rect, tile } => {
                h.u8(Verb::Fill as u8);
                rect.feed(h);
                tile.feed(h);
            }
            Action::Strike { rect, amount, school: s, effect, hits_friends } => {
                h.u8(Verb::Strike as u8);
                rect.feed(h);
                h.i32(amount.0);
                h.u8(school(s));
                match effect {
                    None => h.u8(0),
                    Some(e) => {
                        h.u8(1);
                        h.u16(e.0);
                    }
                }
                h.bool(hits_friends);
            }
            Action::Status(e) => {
                h.u8(Verb::Status as u8);
                h.u16(e.0);
            }
            Action::Heal(heal) => {
                h.u8(Verb::Heal as u8);
                match heal {
                    Heal::Flat(m) => {
                        h.u8(0);
                        h.i32(m.0);
                    }
                    Heal::Pct(p) => {
                        h.u8(1);
                        h.i16(p.0);
                    }
                }
            }
            Action::Travel { zone, mark } => {
                h.u8(Verb::Travel as u8);
                h.u8(zone as u8);
                mark.feed(h);
            }
            Action::Talk(d) => {
                h.u8(Verb::Talk as u8);
                h.u16(d.0);
            }
            Action::Throw(i) => {
                h.u8(Verb::Throw as u8);
                h.u16(i.0);
            }
            Action::Place { prop, item } => {
                h.u8(Verb::Place as u8);
                h.u16(prop.0);
                h.u16(item.0);
            }
            Action::NightLock { prop, lock } => {
                h.u8(Verb::NightLock as u8);
                prop.feed(h);
                lock.feed(h);
            }
            Action::NightUnlock(k) => {
                h.u8(Verb::NightUnlock as u8);
                k.feed(h);
            }
            Action::SwitchAll { def, on } => {
                h.u8(Verb::SwitchAll as u8);
                h.u16(def.0);
                on.feed(h);
            }
            Action::Shake(s) => {
                h.u8(Verb::Shake as u8);
                h.u8(s);
            }
            Action::Ring { strikes, from, church } => {
                h.u8(Verb::Ring as u8);
                h.u8(strikes);
                from.feed(h);
                h.u8(u8::from(church));
            }
            Action::Camera { mode, rect } => {
                h.u8(Verb::Camera as u8);
                h.u8(match mode {
                    CameraMode::Follow => 0,
                    CameraMode::Lock => 1,
                });
                rect.feed(h);
            }
            Action::If { when, then, els } => {
                h.u8(Verb::If as u8);
                when.feed(h);
                then.feed(h);
                els.feed(h);
            }
            Action::Send { unit, to, then } => {
                h.u8(Verb::Send as u8);
                unit.feed(h);
                to.feed(h);
                then.feed(h);
            }
            Action::Reveal(n) => {
                h.u8(Verb::Reveal as u8);
                n.feed(h);
            }
        }
    }
}

impl Feed for FactKey {
    fn feed(&self, h: &mut Hash64) {
        match *self {
            FactKey::Place(k) => {
                h.u8(0);
                k.feed(h);
            }
            FactKey::Person(k) => {
                h.u8(1);
                k.feed(h);
            }
            FactKey::Thing(t) => {
                h.u8(2);
                match t {
                    Thing::Item(i) => {
                        h.u8(0);
                        h.u16(i.0);
                    }
                    Thing::Prop(p) => {
                        h.u8(1);
                        h.u16(p.0);
                    }
                }
            }
            FactKey::Claim(t) => {
                h.u8(3);
                h.u16(t.0);
            }
            FactKey::Route(a, b) => {
                h.u8(4);
                a.feed(h);
                b.feed(h);
            }
            FactKey::Danger(k) => {
                h.u8(5);
                k.feed(h);
            }
            FactKey::Rumour(s) => {
                h.u8(6);
                h.u16(s.0);
            }
        }
    }
}

impl Feed for Cond {
    fn feed(&self, h: &mut Hash64) {
        h.bool(self.not);
        match self.c {
            Condition::Flag { key, test } => {
                h.u8(0);
                key.feed(h);
                match test {
                    FlagTest::Eq(v) => {
                        h.u8(0);
                        h.i32(v);
                    }
                    FlagTest::Min(v) => {
                        h.u8(1);
                        h.i32(v);
                    }
                    FlagTest::NonZero => h.u8(2),
                }
            }
            Condition::Night => h.u8(1),
            Condition::QuestActive(q) => {
                h.u8(2);
                h.u16(q.0);
            }
            Condition::QuestReady(q) => {
                h.u8(3);
                h.u16(q.0);
            }
            Condition::QuestDone(q) => {
                h.u8(4);
                h.u16(q.0);
            }
            Condition::HasItem(s) => {
                h.u8(5);
                s.feed(h);
            }
            Condition::HasSpell(s) => {
                h.u8(6);
                h.u16(s.0);
            }
            Condition::Dead(k) => {
                h.u8(7);
                k.feed(h);
            }
            Condition::Knows(f) => {
                h.u8(8);
                f.feed(h);
            }
            Condition::Heard(t) => {
                h.u8(9);
                h.u16(t.0);
            }
            Condition::SpeakerKnows(s) => {
                h.u8(10);
                h.u16(s.0);
            }
            Condition::Hours { from, to } => {
                h.u8(11);
                h.u8(from);
                h.u8(to);
            }
            Condition::Weekday(d) => {
                h.u8(12);
                h.u8(d);
            }
            Condition::SpeakerHeard(c) => {
                h.u8(13);
                h.u16(c.0);
            }
            Condition::Within { unit, rect } => {
                h.u8(14);
                unit.feed(h);
                rect.feed(h);
            }
            Condition::SpeakerLit => h.u8(15),
        }
    }
}

impl Feed for Blueprint {
    fn feed(&self, h: &mut Hash64) {
        let Blueprint {
            zone,
            name,
            tiles,
            units,
            props,
            marks,
            rects,
            indoor,
            ambient,
            attempts,
            triggers,
            stories,
            paint,
            lists,
            conds,
            name_lists,
            texts,
            local_names,
            areas,
            regions,
            sanctuary,
            packed,
        } = self;
        // A packed blueprint has let its paint's order go, which this hash reads: hash it as
        // built, before `Blueprint::pack` (PORT.md §13.3).
        assert!(packed.is_none(), "hash a blueprint before it is packed");
        h.u32(LAYOUT);
        h.u8(*zone as u8);
        name.feed(h);
        h.u32(tiles.w());
        h.u32(tiles.h());
        for t in tiles.as_slice() {
            h.u8(t.id());
        }
        units.feed(h);
        props.feed(h);
        h.count(marks.len());
        for (k, m) in marks {
            k.feed(h);
            m.feed(h);
        }
        h.count(rects.len());
        for (k, r) in rects {
            k.feed(h);
            r.feed(h);
        }
        h.bool(*indoor);
        h.i16(ambient.0);
        h.u8(*attempts);
        h.count(triggers.len());
        for (k, t) in triggers {
            k.feed(h);
            t.feed(h);
        }
        h.count(stories.len());
        for (id, place) in stories {
            h.u16(id.0);
            place.feed(h);
        }
        h.count(paint.len());
        for (r, m) in paint {
            r.feed(h);
            m.feed(h);
        }
        lists.feed(h);
        conds.feed(h);
        name_lists.feed(h);
        texts.feed(h);
        local_names.feed(h);
        h.count(areas.len());
        for a in areas {
            a.name.feed(h);
            a.rect.feed(h);
        }
        h.count(sanctuary.len());
        for r in sanctuary {
            r.feed(h);
        }
        h.u16(regions.scale);
        h.u16(regions.w);
        h.u16(regions.h);
        h.count(regions.cells.len());
        h.bytes(&regions.cells);
    }
}

/// The blueprint's hash: equal blueprints, equal hashes, on every target.
pub fn hash(bp: &Blueprint) -> u64 {
    let mut h = Hash64::new();
    bp.feed(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use jane_core::ZoneId;

    use super::*;

    #[test]
    fn fnv1a_64_matches_the_reference_vectors() {
        let of = |b: &[u8]| {
            let mut h = Hash64::new();
            h.bytes(b);
            h.finish()
        };
        assert_eq!(of(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(of(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(of(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn every_field_moves_the_hash() {
        let a = Blueprint::new(ZoneId::Mine, 10, 8, Tile::CaveWall);
        let base = hash(&a);
        assert_eq!(hash(&a.clone()), base);
        let mut b = a.clone();
        b.tiles.set(3, 3, Tile::Sill);
        assert_ne!(hash(&b), base);
        let mut b = a.clone();
        b.marks.insert(Key::Local(0), Mark { cell: Cell::new(1, 1), facing: None });
        assert_ne!(hash(&b), base);
        let mut b = a.clone();
        b.push_list(vec![Action::Show(Key::Local(0))]);
        let with_list = hash(&b);
        assert_ne!(with_list, base);
        b.lists[0] = vec![Action::Hide(Key::Local(0))];
        assert_ne!(hash(&b), with_list);
        let mut b = a.clone();
        b.attempts = 2;
        assert_ne!(hash(&b), base);
        let mut b = a.clone();
        b.regions = jane_core::RegionMap::new(16, 1, 1, 0);
        let with_map = hash(&b);
        assert_ne!(with_map, base);
        b.regions.set(0, 0, 2);
        assert_ne!(hash(&b), with_map);
        // Two strings that concatenate the same do not hash the same.
        let mut b = a.clone();
        b.local_names = vec!["ab".into(), "c".into()];
        let mut c = a;
        c.local_names = vec!["a".into(), "bc".into()];
        assert_ne!(hash(&b), hash(&c));
    }
}
