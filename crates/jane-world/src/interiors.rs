//! The hand-built interiors (PORT.md §6.m stage 10; `jane/src/world/interiors.ts` and the two
//! registered zones `world/zones/arms.ts` and `church.ts`): Julie's house, the cellar under it,
//! the Castle Arms and St Anne's. "The hatches are a cellar, Jane. Not a dungeon."
//!
//! - [`interior_candidate`]: one candidate for `(zone, seed, attempt)`.
//! - [`build_interior`]: the attempt loop over `ZONE_ATTEMPTS`, the solver as the judge.
//!
//! The coordinates are the rooms as drawn, cell by cell; none of them is a view size.
//!
//! Dice: the house throws [`Step::IntHouse`] and the cellar [`Step::IntCellar`], one stream per
//! decision (the step's `a`), so a pile that tries once more moves no rat. The Arms and the
//! church are the same on every seed and throw nothing.
//!
//! Names: a key content knows (a contract's, a dialogue's, a door's far mark) is its `NameId`;
//! any other (`fruit_bowl`, `rat_chest`) is made for the blueprint, and nothing nobody named
//! (`house_shelf_3`) is named by a count, as the TypeScript's kit did.

use jane_core::action::{Action, Cond, Condition, Facing, FlagKey, FlagOp, FlagTest, Stack};
use jane_core::blueprint::{Door, PropSpawn, ZONE_ATTEMPTS};
use jane_core::num::Permille;
use jane_core::{Blueprint, Key, PropDefId, Rect, Tile, ZoneId};
use jane_data::catalog;

use crate::kit::{Kit, SPOT_TRIES};
use crate::solve::{ZoneRules, validate};
use crate::steps::Step;

/// Whether `zone` is one of the hand-built interiors.
pub fn is_interior(zone: ZoneId) -> bool {
    matches!(zone, ZoneId::House | ZoneId::Cellar | ZoneId::Arms | ZoneId::Church)
}

/// One candidate of an interior, or `None` for a zone that is not one.
pub fn interior_candidate(zone: ZoneId, seed: u32, attempt: u8) -> Option<Blueprint> {
    Some(match zone {
        ZoneId::House => build_house(seed, attempt),
        ZoneId::Cellar => build_cellar(seed, attempt),
        ZoneId::Arms => build_arms(seed, attempt),
        ZoneId::Church => build_church(seed, attempt),
        _ => return None,
    })
}

/// The interior for a seed (world/index.ts `buildZone`): candidates in attempt order until the
/// solver passes one with the zone's rules. `None` for a zone that is not an interior, and for a
/// seed none of `ZONE_ATTEMPTS` proves (never played: the caller refuses the seed).
pub fn build_interior(zone: ZoneId, seed: u32) -> Option<Blueprint> {
    if !is_interior(zone) {
        return None;
    }
    let rules = ZoneRules::for_zone(zone);
    let mut attempt = 0;
    loop {
        let bp = interior_candidate(zone, seed, attempt)?;
        if validate(&bp, &rules).ok() {
            return Some(bp);
        }
        if attempt + 1 >= ZONE_ATTEMPTS {
            return None;
        }
        attempt += 1;
    }
}

// --- the hand ------------------------------------------------------------------------------

/// A kit, and the catalog lookups a hand-built room makes by the TypeScript's string ids.
struct Hand {
    k: Kit,
}

impl Hand {
    fn new(zone: ZoneId, w: u32, h: u32, seed: u32, attempt: u8, fill: Tile) -> Self {
        Hand { k: Kit::new(zone, w, h, seed, attempt, fill, false) }
    }

    /// A content name if content has it, else one made for this blueprint.
    fn key(&mut self, s: &str) -> Key {
        match catalog().name_id(s) {
            Some(n) => Key::Name(n),
            None => self.k.local(s),
        }
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, t: Tile) {
        self.k.fill(Rect::new(x, y, w, h), t);
    }

    fn rect(&mut self, name: &str, r: Rect) {
        let key = self.key(name);
        self.k.rect(key, r);
    }

    fn mark(&mut self, name: &str, x: i32, y: i32, facing: Facing) {
        let key = self.key(name);
        self.k.mark(key, x, y, Some(facing));
    }

    /// A prop of `def` at `(x, y)`, keyed `key` or by the kit's count.
    fn prop(&mut self, key: Option<&str>, def: &str, x: i32, y: i32) -> &mut PropSpawn {
        let key = key.map(|s| self.key(s));
        self.k.prop(key, prop_def(def), x, y)
    }

    /// A prop that talks: `tree` is its dialogue.
    fn talker(&mut self, key: &str, def: &str, x: i32, y: i32, tree: &str) {
        let talk = catalog().story.dialogue_id(tree).unwrap_or_else(|| panic!("no dialogue {tree:?}"));
        self.prop(Some(key), def, x, y).talk = Some(talk);
    }

    /// What gathering a white rose does, the same as Sallow Bottom's (`data/placements`): after
    /// dark, on a seed the rose omen is true, it is remembered (`rose_after_dark`).
    fn picked_after_dark(&mut self) -> jane_core::ListRef {
        let (omen, flag) = (self.key("omen:roses_after_dark"), self.key("rose_after_dark"));
        let when = self.k.conds(vec![
            Cond { not: false, c: Condition::Night },
            Cond { not: false, c: Condition::Flag { key: FlagKey::Named(omen), test: FlagTest::NonZero } },
        ]);
        let then = self.k.list(vec![Action::Flag { key: FlagKey::Named(flag), op: FlagOp::Set(1) }]);
        self.k.list(vec![Action::If { when, then, els: None }])
    }

    /// A prop with `loot` in it.
    fn chest(&mut self, key: Option<&str>, def: &str, x: i32, y: i32, loot: &[(&str, u16)]) {
        let loot = loot.iter().map(|&(item, qty)| stack(item, qty)).collect();
        self.prop(key, def, x, y).loot = loot;
    }

    /// A door that leads to `mark` of `zone`, labelled `label` if given. The far mark is a
    /// content name when content has it; the county's `arms_front` and `church_door` are marks of
    /// its authored places, which become names when their `.chunk` files land (PORT.md §6.f), and
    /// until then are made here, as the county makes them, and meet by their string in the sim.
    fn door(&mut self, key: &str, def: &str, x: i32, y: i32, (zone, mark): (ZoneId, &str), label: Option<&str>) {
        let to = Door { zone, mark: self.key(mark) };
        let label = label.map(|s| self.k.text(s));
        let p = self.prop(Some(key), def, x, y);
        p.to = Some(to);
        p.label = label;
    }

    /// A locked gate that a key opening `tag` fits.
    fn gate(&mut self, key: &str, def: &str, x: i32, y: i32, tag: &str, label: &str) {
        let tag = self.key(tag);
        let label = self.k.text(label);
        let p = self.prop(Some(key), def, x, y);
        p.locked = true;
        p.key_tag = Some(tag);
        p.label = Some(label);
    }

    /// A unit of `def` at `(x, y)`, keyed `key` or by the kit's count.
    fn unit(&mut self, key: Option<&str>, def: &str, x: i32, y: i32, facing: Option<Facing>) {
        let key = key.map(|s| self.key(s));
        let id = catalog().combat.unit_id(def).unwrap_or_else(|| panic!("no unit row {def:?}"));
        self.k.unit(key, id, x, y, Vec::new()).facing = facing;
    }

    /// A rat at an open cell of `r`, if there is one.
    fn rat_in(&mut self, rng: &mut jane_core::Sfc32, r: Rect) {
        if let Some((x, y)) = self.k.spot(rng, r, 1, 1, 1, SPOT_TRIES) {
            self.unit(None, "rat", x, y, None);
        }
    }

    fn done(self, name: &str, ambient: i16) -> Blueprint {
        self.k.done(name, true, Permille(ambient))
    }
}

fn prop_def(def: &str) -> PropDefId {
    catalog().story.prop_id(def).unwrap_or_else(|| panic!("no prop row {def:?}"))
}

fn stack(item: &str, qty: u16) -> Stack {
    Stack { item: catalog().combat.item_id(item).unwrap_or_else(|| panic!("no item {item:?}")), qty }
}

// --- Julie's house -------------------------------------------------------------------------

/// A cottage's inside: a kitchen and a front room, 28 m by 20 of floor between them. It was 44
/// by 30 until Sept 24: Julie is one more person on the station road, and her house is the size
/// of her neighbours'. Everything the story uses is still here. 2020's room_building2_auntie had
/// two basement stairs on one wall and furniture down the other.
pub fn build_house(seed: u32, attempt: u8) -> Blueprint {
    let mut b = Hand::new(ZoneId::House, 34, 25, seed, attempt, Tile::Wall);
    let kitchen = Rect::new(2, 2, 15, 20);
    let living = Rect::new(19, 2, 13, 20);
    b.k.fill(kitchen, Tile::FloorWood);
    b.k.fill(living, Tile::FloorWood);
    let doorway = 8 + b.k.dice(Step::IntHouse, 0, 0).range(0, 6);
    b.fill(17, doorway, 2, 5, Tile::FloorWood);
    b.rect("kitchen", kitchen);

    // Front door, in the living room's south wall.
    b.door("front_door", "door", 25, 22, (ZoneId::County, "house_front"), None);
    b.mark("front", 25, 20, Facing::North);

    // Two hatches on one wall, both to the cellar, chalked in Julie's hand. Each goes down on its
    // own side: the west one to the keys in the cellar's west room, the east one to the other way
    // out in its east room, and a note there says so. (They were crossed until the owner's first
    // playtest: the west hatch came up the east stair.)
    b.door("hatch_a", "hatch", 4, 4, (ZoneId::Cellar, "stair_a"), Some("The hatch chalked KEYS"));
    b.door("hatch_b", "hatch", 9, 4, (ZoneId::Cellar, "stair_b"), Some("The hatch chalked OUT"));
    b.mark("hatch_a", 5, 7, Facing::South);
    b.mark("hatch_b", 10, 7, Facing::South);

    // Julie's dresser against the north wall, between the hatches: somewhere to leave what the
    // bag cannot carry and cannot throw away (`jane_sim::store`, WORLD.md §8.3).
    b.prop(Some("julies_dresser"), "dresser", 6, 2);
    b.prop(None, "stove", 12, 2);
    b.talker("ice_orb", "orb_ice", 15, 2, "orb_ice");
    b.prop(None, "table", 6, 11);
    // The fruit bowl stands on the kitchen table, on its front half (the owner, 2026-10-01: not
    // on the floor). Its footprint lies on the table's, and the presenter draws it on the top.
    b.chest(Some("fruit_bowl"), "fruit_bowl", 7, 12, &[("apple", 4)]);
    b.talker("julies_note", "note", 10, 12, "julies_note");
    b.prop(Some("bench"), "bench", 3, 17);
    b.chest(Some("pantry_chest"), "chest", 9, 19, &[("gold_dust", 2), ("small_water", 3), ("pansy", 2)]);
    // The kitchen drawer: a box of matches, kept full (`tuning/sim.json` `regrow.restock`).
    b.chest(Some("kitchen_drawer"), "kitchen_drawer", 12, 20, &[("match", 10)]);

    // Front room: leftover furniture along the wall, as 2020 stacked its benches.
    b.talker("julies_bed", "bed", 29, 3, "bed");
    b.prop(None, "shelf", 21, 2);
    // Her cupboard by the bed, where the second shelf stood.
    b.prop(Some("julies_cupboard"), "cupboard", 25, 2);
    let mut rng = b.k.dice(Step::IntHouse, 1, 0);
    let (tx, ty) = (21 + rng.range(0, 4), 10 + rng.range(0, 3));
    b.prop(None, "table", tx, ty);
    b.prop(None, "torch", 20, 3);
    b.prop(None, "torch", 3, 10);
    b.prop(None, "torch", 30, 19);
    b.done("Julie's House", 720)
}

// --- the cellar ----------------------------------------------------------------------------

const CELLAR_W: i32 = 100;
const CELLAR_H: i32 = 76;
/// The long east-west corridor's top row: every room's spur runs down to it.
const CORRIDOR_Y: i32 = 37;

/// The cellar's decisions, each its own stream of [`Step::IntCellar`].
#[derive(Clone, Copy)]
#[repr(i32)]
enum Cellar {
    Rooms,
    PileA,
    Rats,
    Crates,
    LoneRat,
    Roses,
    PileB,
}

/// The cellar under the house. 2020's room_basement_auntie was a keyed loop with no enemies, two
/// iron doors that take the same key, and a rose patch; this one keeps a few rats.
pub fn build_cellar(seed: u32, attempt: u8) -> Blueprint {
    let mut b = Hand::new(ZoneId::Cellar, CELLAR_W as u32, CELLAR_H as u32, seed, attempt, Tile::Wall);
    let dice = |b: &Hand, d: Cellar| b.k.dice(Step::IntCellar, d as i32, 0);
    let floor = Tile::Floor;

    // Room A, south-west: where hatch A comes down. The iron key is here.
    let a = Rect::new(4, 50, 22, 20);
    b.k.fill(a, floor);
    b.door("stair_a", "stairs", 6, 66, (ZoneId::House, "hatch_a"), None);
    b.mark("stair_a", 10, 66, Facing::East);
    b.chest(Some("cellar_chest"), "chest", 20, 52, &[("key_basement", 2)]);
    b.prop(Some("cellar_cupboard"), "cupboard", 8, 50);
    let mut rng = dice(&b, Cellar::PileA);
    b.k.pile(&mut rng, Rect::new(a.x + 1, a.y + 8, 10, 6), prop_def("barrel"), 3);

    // North out of room A through the first iron door, up to the long corridor.
    b.fill(13, 40, 3, 10, floor);
    b.gate("iron_door_a", "gate_h", 13, 45, "basement", "The iron door");
    b.fill(6, CORRIDOR_Y, 85, 3, floor);

    // Four rooms north of the corridor, each with a spur down to it.
    let mut rng = dice(&b, Cellar::Rooms);
    let rooms = [6, 26, 46, 66].map(|x| Rect::new(x, 16 + rng.range(0, 3), 16, 14));
    for r in rooms {
        b.k.fill(r, floor);
        b.fill(r.x + 6, r.bottom(), 3, CORRIDOR_Y - r.bottom(), floor);
    }
    let [rat_room, potion_room, storage, study] = rooms;

    // The chest first, so the rats are put where it does not stand.
    b.chest(Some("rat_chest"), "chest", rat_room.x + 1, rat_room.y + 1, &[("key_generic", 1), ("small_empty_vial", 2)]);
    let mut rng = dice(&b, Cellar::Rats);
    for _ in 0..3 {
        b.rat_in(&mut rng, rat_room);
    }

    // "Potion making Room" from the 2020 basement sketch.
    b.prop(Some("potion_bench"), "bench", potion_room.x + 2, potion_room.y + 1);
    b.chest(
        Some("potion_chest"),
        "chest",
        potion_room.x + 12,
        potion_room.y + 1,
        &[("small_water", 4), ("hemshade_root", 1), ("night_lich_moss", 1), ("stone", 1)],
    );
    b.prop(None, "shelf", potion_room.x + 6, potion_room.y);

    // "storage room locked": wood and iron, which the mine's broken things will want.
    b.gate("storage_gate", "gate_h", storage.x + 6, storage.bottom() + 1, "generic", "The storage room");
    b.chest(Some("storage_chest"), "chest", storage.x + 7, storage.y + 1, &[("wood", 4), ("iron", 4)]);
    let mut rng = dice(&b, Cellar::Crates);
    b.k.pile(&mut rng, Rect::new(storage.x + 1, storage.y + 5, storage.w - 2, 5), prop_def("crate"), 3);

    // The study. 2020's sketch put an electricity orb here; that spell is a later door. The desk
    // has a clean ring in its dust where the stand stood, which the Factory's orb pays off.
    b.talker("study_desk", "study_desk", study.x + 6, study.y + 4, "study_desk");
    b.prop(None, "shelf", study.x + 2, study.y);
    b.prop(None, "shelf", study.x + 10, study.y);
    let mut rng = dice(&b, Cellar::LoneRat);
    b.rat_in(&mut rng, study);
    // Where something stands if a white rose was picked after dark on a seed the rose omen is true
    // (`data/omens.json`, `data/triggers/lowfields.json`): the study, never the stairs.
    b.mark("cellar_study", study.x + 8, study.y + 10, Facing::South);

    // Rose alcove south of the corridor. White Water Rose goes into Stone Skin.
    let alcove = Rect::new(40, 40, 12, 8);
    b.k.fill(alcove, Tile::Garden);
    let mut rng = dice(&b, Cellar::Roses);
    let bed = Rect::new(alcove.x + 1, alcove.y + 2, alcove.w - 2, alcove.h - 3);
    let picked = b.picked_after_dark();
    for _ in 0..3 {
        if let Some((x, y)) = b.k.spot(&mut rng, bed, 1, 1, 0, SPOT_TRIES) {
            let rose = b.prop(None, "rose", x, y);
            rose.loot = vec![stack("white_water_rose", 1)];
            rose.use_list = Some(picked);
        }
    }

    // East leg: the other iron door, then room B and hatch B. Either door closes the loop.
    b.fill(88, 40, 3, 20, floor);
    b.gate("iron_door_b", "gate_h", 88, 48, "basement", "The other iron door");
    let room_b = Rect::new(74, 58, 20, 14);
    b.k.fill(room_b, floor);
    b.door("stair_b", "stairs", 90, 68, (ZoneId::House, "hatch_b"), None);
    b.mark("stair_b", 87, 68, Facing::West);
    // Come down this end first and the iron door is shut with nothing to open it: the note by it
    // says where the keys are, in the hand from the kitchen.
    b.talker("cellar_other_way", "note", 86, 62, "cellar_other_way");
    let mut rng = dice(&b, Cellar::PileB);
    b.k.pile(&mut rng, Rect::new(room_b.x + 1, room_b.y + 1, 8, 6), prop_def("barrel"), 2);

    let torch = prop_def("torch");
    for r in [a, room_b, rat_room, potion_room, storage, study] {
        b.k.torch_run(r, 9, torch);
    }
    for x in (10..88).step_by(12) {
        b.k.prop(None, torch, x, CORRIDOR_Y);
    }
    b.rect("cellar", Rect::new(0, 0, CELLAR_W, CELLAR_H));
    b.done("Julie's Cellar", 300)
}

// --- Castle: the two doors on the square that open -----------------------------------------
//
// The Castle Arms and St Anne's. Small rooms, full: a Zelda house is a counter, a fire, a table
// and somebody to talk to. Both are the same on every seed.

/// The Castle Arms, on the square in Castle: the one door on the high street that opens. The tap
/// room, the bar, a fire, the regulars and the landlady, and a room at the back with a bed.
pub fn build_arms(seed: u32, attempt: u8) -> Blueprint {
    let mut b = Hand::new(ZoneId::Arms, 34, 25, seed, attempt, Tile::Wall);
    b.fill(2, 2, 30, 20, Tile::FloorWood);
    // The back room: a partition with a doorway in it.
    b.fill(23, 2, 1, 8, Tile::Wall);
    b.fill(23, 10, 9, 1, Tile::Wall);
    b.fill(27, 10, 2, 1, Tile::FloorWood);
    b.rect("tap_room", Rect::new(2, 2, 21, 20));

    b.door("exit_door", "door", 15, 22, (ZoneId::County, "arms_front"), Some("The street"));
    b.mark("entry", 15, 20, Facing::North);

    // The bar along the north wall, the shelves behind it, the landlady in front of it.
    b.prop(None, "shelf", 4, 2);
    b.prop(None, "shelf", 8, 2);
    b.prop(Some("arms_bar"), "town_bar", 4, 4);
    b.prop(None, "town_bar", 7, 4);
    b.prop(None, "town_bar", 10, 4);
    b.unit(Some("mrs_garland"), "town_landlady", 8, 8, Some(Facing::South));
    b.talker("arms_notice", "sign", 14, 2, "arms_notice");
    b.prop(None, "barrel", 18, 2);
    b.prop(None, "barrel", 20, 2);
    b.chest(None, "barrel", 19, 5, &[("small_water", 1)]);

    // The fire, and four tables, two of them taken.
    b.prop(Some("arms_stove"), "stove", 2, 11);
    for (x, y) in [(7, 12), (14, 12), (7, 17), (14, 17)] {
        b.prop(None, "table", x, y);
    }
    b.unit(Some("mr_quill"), "town_regular", 10, 13, Some(Facing::West));
    b.unit(Some("mr_ennis"), "town_regular_b", 17, 18, Some(Facing::West));
    b.prop(None, "torch", 2, 3);
    b.prop(None, "torch", 22, 15);
    b.prop(None, "torch", 2, 20);

    // The room at the back. The door locks from her side; nobody comes in.
    b.talker("arms_bed", "bed", 29, 3, "arms_bed");
    b.prop(None, "shelf", 25, 2);
    b.prop(None, "torch", 24, 8);
    // Beyond the partition, the rest of the house: kegs and the landing.
    b.prop(None, "crate", 25, 14);
    // A cupboard on the landing for a lodger's things, in the corner under the partition.
    b.prop(Some("arms_cupboard"), "cupboard", 30, 11);
    b.prop(None, "barrel", 28, 14);
    b.prop(None, "barrel", 28, 18);
    b.done("The Castle Arms", 700)
}

/// St Anne's, on the square in Castle: pews either side of an aisle, the altar with its two
/// candles that do not burn down, the visitors' book with Julie's hand in it by the door, and
/// the vicar, who is always in.
pub fn build_church(seed: u32, attempt: u8) -> Blueprint {
    let mut b = Hand::new(ZoneId::Church, 22, 36, seed, attempt, Tile::TempleWall);
    b.fill(2, 2, 18, 30, Tile::TempleFloor);
    b.rect("nave", Rect::new(2, 2, 18, 30));
    b.door("exit_door", "door", 10, 32, (ZoneId::County, "church_door"), Some("The square"));
    b.mark("entry", 10, 30, Facing::North);

    b.talker("altar", "town_altar", 9, 3, "altar");
    b.prop(None, "torch", 5, 3);
    b.prop(None, "torch", 16, 3);
    b.unit(Some("vicar"), "town_vicar", 10, 7, Some(Facing::South));
    for y in (10..=25).step_by(3) {
        b.prop(None, "town_pew", 3, y);
        b.prop(None, "town_pew", 15, y);
    }
    b.prop(None, "torch", 2, 17);
    b.prop(None, "torch", 19, 17);
    b.prop(None, "table", 15, 28);
    b.talker("visitors_book", "note", 14, 29, "visitors_book");
    b.prop(None, "shelf", 3, 30);
    b.done("St Anne's", 550)
}
