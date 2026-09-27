//! PORT.md §6.m stage 10: the hand-built interiors. Carries the interior parts of
//! `jane/test/world.test.ts` ("every seed passes the lock-and-key solver" with at most three
//! attempts, "is a pure function of (zone, seed)", "every door leads to a zone that exists, and
//! to a mark that zone has"), the worldgen half of `dungeons.test.ts`'s cellar ("one iron key type
//! opens both iron doors; the storage room wants a plain key"), and what `sim.test.ts` (the ice
//! orb in the kitchen), `rest.test.ts` (Julie's bed from the front door) and `town.test.ts` (the
//! Arms' bed and landlady, the church's book and vicar) walk to, as reach from the entrance.

mod common;

use jane_core::blueprint::ZONE_ATTEMPTS;
use jane_core::num::Permille;
use jane_core::tile::F_INDOOR;
use jane_core::{Blueprint, Key, Rect, Tile, ZoneId};
use jane_data::catalog;
use jane_world::interiors::{build_interior, interior_candidate, is_interior};
use jane_world::solve::{Grant, KeyTag, Options, ZoneRules, lock_holds, solve, validate};

const INTERIORS: [ZoneId; 4] = [ZoneId::House, ZoneId::Cellar, ZoneId::Arms, ZoneId::Church];

fn name(s: &str) -> Key {
    Key::Name(catalog().name_id(s).unwrap_or_else(|| panic!("no content name {s:?}")))
}

fn key_name(bp: &Blueprint, k: Key) -> String {
    jane_world::solve::report::name_of(bp, k).to_owned()
}

fn prop<'b>(bp: &'b Blueprint, key: &str) -> &'b jane_core::blueprint::PropSpawn {
    let k = name_or_local(bp, key);
    bp.props.iter().find(|p| p.key == k).unwrap_or_else(|| panic!("{}: no prop {key}", bp.zone.name()))
}

fn name_or_local(bp: &Blueprint, s: &str) -> Key {
    match catalog().name_id(s) {
        Some(n) => Key::Name(n),
        None => Key::Local(bp.local_names.iter().position(|n| n == s).unwrap_or_else(|| panic!("no name {s}")) as u32),
    }
}

#[test]
fn every_seed_of_every_interior_passes_the_solver_within_three_attempts() {
    for zone in INTERIORS {
        let rules = ZoneRules::for_zone(zone);
        let mut worst = 0;
        for seed in 1..=common::seeds() {
            let bp = build_interior(zone, seed).expect("an interior");
            let r = validate(&bp, &rules);
            assert!(r.ok(), "{} seed {seed}: {:?}", zone.name(), r.lines(&bp));
            assert!(bp.attempts <= 3, "{} seed {seed} needed {} attempts", zone.name(), bp.attempts);
            worst = worst.max(bp.attempts);
        }
        println!("{}: worst attempts {worst}/{ZONE_ATTEMPTS}", zone.name());
    }
}

#[test]
fn every_contract_name_is_there() {
    let cat = catalog();
    for zone in INTERIORS {
        let c = &cat.county.zone(zone).contract;
        for seed in 1..=common::seeds().min(16) {
            let bp = build_interior(zone, seed).expect("an interior");
            for &u in c.units {
                assert!(bp.units.iter().any(|x| x.key == Key::Name(u)), "{}: unit {}", zone.name(), cat.name(u));
            }
            for &p in c.props {
                assert!(bp.props.iter().any(|x| x.key == Key::Name(p)), "{}: prop {}", zone.name(), cat.name(p));
            }
            for &m in c.marks {
                assert!(bp.marks.contains_key(&Key::Name(m)), "{}: mark {}", zone.name(), cat.name(m));
            }
            for &r in c.rects {
                assert!(bp.rects.contains_key(&Key::Name(r)), "{}: rect {}", zone.name(), cat.name(r));
            }
        }
    }
}

#[test]
fn only_the_four_are_interiors() {
    for zone in ZoneId::ALL {
        let expect = catalog().county.zone(zone).kind == jane_data::ZoneKind::Interior;
        assert_eq!(is_interior(zone), expect, "{}", zone.name());
        assert_eq!(build_interior(zone, 1).is_some(), expect, "{}", zone.name());
    }
}

#[test]
fn the_same_seed_gives_the_same_interior_and_the_house_and_cellar_vary() {
    for zone in INTERIORS {
        for seed in [1, 4242] {
            assert_eq!(interior_candidate(zone, seed, 0), interior_candidate(zone, seed, 0), "{}", zone.name());
            assert_eq!(build_interior(zone, seed), build_interior(zone, seed), "{}", zone.name());
        }
    }
    for zone in [ZoneId::House, ZoneId::Cellar] {
        let first = interior_candidate(zone, 1, 0);
        assert!((2..40).any(|s| interior_candidate(zone, s, 0) != first), "{} is the same on every seed", zone.name());
    }
    // The Arms and the church are the same on every seed.
    for zone in [ZoneId::Arms, ZoneId::Church] {
        let (a, b) = (interior_candidate(zone, 1, 0).expect("built"), interior_candidate(zone, 99, 0).expect("built"));
        assert_eq!((a.tiles, a.props, a.units), (b.tiles, b.props, b.units), "{}", zone.name());
    }
}

#[test]
fn indoor_with_the_typescripts_light_and_size() {
    let want = [
        (ZoneId::House, "Julie's House", 720, (34, 25)),
        (ZoneId::Cellar, "Julie's Cellar", 300, (100, 76)),
        (ZoneId::Arms, "The Castle Arms", 700, (34, 25)),
        (ZoneId::Church, "St Anne's", 550, (22, 36)),
    ];
    for (zone, title, ambient, (w, h)) in want {
        let bp = build_interior(zone, 7).expect("an interior");
        assert_eq!(bp.zone, zone);
        assert!(bp.indoor, "{}", zone.name());
        assert_eq!(bp.ambient, Permille(ambient), "{}", zone.name());
        assert_eq!(bp.text(bp.name), Some(title));
        assert_eq!((bp.w(), bp.h()), (w, h));
        // Every tile of an interior is an indoor tile: no sky over any of it.
        assert!(bp.tiles.as_slice().iter().all(|t| t.flags() & F_INDOOR != 0), "{}", zone.name());
    }
}

/// `world.test.ts` "every door leads to a zone that exists, and to a mark that zone has", for
/// the doors between interiors; a door out to the county names a content mark.
#[test]
fn every_door_leads_to_a_mark_its_zone_has() {
    for seed in [1, 1000, 4242] {
        let built: Vec<Blueprint> = INTERIORS.iter().map(|&z| build_interior(z, seed).expect("an interior")).collect();
        for bp in &built {
            let mut doors = 0;
            for p in &bp.props {
                let Some(to) = p.to else { continue };
                doors += 1;
                if let Some(dest) = built.iter().find(|d| d.zone == to.zone) {
                    assert!(
                        matches!(to.mark, Key::Name(_)),
                        "{}: {} leads to a made-up mark",
                        bp.zone.name(),
                        key_name(bp, p.key)
                    );
                    assert!(
                        dest.marks.contains_key(&to.mark),
                        "{}: {} leads to {}, which has no mark {}",
                        bp.zone.name(),
                        key_name(bp, p.key),
                        to.zone.name(),
                        key_name(bp, to.mark)
                    );
                } else {
                    // Out to the county: to the house's front, or to a mark of one of the town's
                    // authored places (a name when its `.chunk` lands; made here until then).
                    assert_eq!(to.zone, ZoneId::County, "{}: {}", bp.zone.name(), key_name(bp, p.key));
                    let far = key_name(bp, to.mark);
                    assert!(["house_front", "arms_front", "church_door"].contains(&far.as_str()), "{far}");
                }
            }
            assert!(doors >= 1, "{} has no way out", bp.zone.name());
        }
    }
}

/// Everyone and everything the sim tests walk to is reached from the entrance (the solver reaches
/// every prop or refuses; this adds the units it does not require: the Arms' regulars, the rats).
#[test]
fn every_unit_stands_where_she_can_reach_it() {
    for zone in INTERIORS {
        let rules = ZoneRules::for_zone(zone);
        for seed in 1..=common::seeds().min(16) {
            let bp = build_interior(zone, seed).expect("an interior");
            let r = solve(&bp, &rules, &Options { trace: true, ..Options::default() });
            assert!(r.ok());
            for u in &bp.units {
                let (x, y) = (i32::from(u.cell.x), i32::from(u.cell.y));
                assert!(
                    r.info.first_seen_at(x, y).is_some(),
                    "{} seed {seed}: {} out of reach",
                    zone.name(),
                    key_name(&bp, u.key)
                );
            }
        }
    }
    let house = build_interior(ZoneId::House, 3).expect("the house");
    for p in ["ice_orb", "julies_bed", "julies_note", "hatch_a", "hatch_b", "pantry_chest"] {
        assert!(
            prop(&house, p).talk.is_some() || !prop(&house, p).loot.is_empty() || prop(&house, p).to.is_some(),
            "{p}"
        );
    }
    assert_eq!(prop(&house, "ice_orb").talk, catalog().story.dialogue_id("orb_ice"));
    assert_eq!(
        prop(&build_interior(ZoneId::Arms, 3).expect("the Arms"), "arms_bed").talk,
        catalog().story.dialogue_id("arms_bed")
    );
    let church = build_interior(ZoneId::Church, 3).expect("the church");
    assert_eq!(prop(&church, "visitors_book").talk, catalog().story.dialogue_id("visitors_book"));
}

/// `dungeons.test.ts` "one iron key type opens both iron doors; the storage room wants a plain
/// key": the chest by the stairs holds two iron keys, both iron doors take them, and the storage
/// room's gate takes the plain key from the rats' chest. Taking either key away shuts what it opens.
#[test]
fn the_cellar_is_a_keyed_loop() {
    let cat = catalog();
    let basement = name("basement");
    let generic = name("generic");
    let key_basement = cat.combat.item_id("key_basement").expect("the iron key");
    let key_generic = cat.combat.item_id("key_generic").expect("a plain key");
    assert_eq!(cat.combat.item(key_basement).opens.map(Key::Name), Some(basement));
    assert_eq!(cat.combat.item(key_generic).opens.map(Key::Name), Some(generic));
    let rules = ZoneRules::for_zone(ZoneId::Cellar);
    for seed in 1..=common::seeds().min(16) {
        let bp = build_interior(ZoneId::Cellar, seed).expect("the cellar");
        let chest = prop(&bp, "cellar_chest");
        assert_eq!(chest.loot.iter().find(|s| s.item == key_basement).map(|s| s.qty), Some(2));
        for door in ["iron_door_a", "iron_door_b"] {
            let d = prop(&bp, door);
            assert!(d.locked && d.key_tag == Some(basement), "{door}");
        }
        let gate = prop(&bp, "storage_gate");
        assert!(gate.locked && gate.key_tag == Some(generic));
        assert!(prop(&bp, "rat_chest").loot.iter().any(|s| s.item == key_generic));

        // Behind the iron door: the far stairs. Behind the plain one: the storage chest's floor.
        let far = bp.marks[&name("stair_b")].cell;
        let far = Rect::new(i32::from(far.x), i32::from(far.y), 1, 1);
        let store = prop(&bp, "storage_chest").cell;
        let store = Rect::new(i32::from(store.x) - 1, i32::from(store.y) - 1, 4, 4);
        let opts = Options::default();
        let traced = solve(&bp, &rules, &Options { trace: true, ..Options::default() });
        assert!(traced.ok() && traced.info.reached_rect(far) && traced.info.reached_rect(store));
        assert!(lock_holds(&bp, &rules, &opts, Grant::Key(KeyTag::Tag(basement)), far), "seed {seed}");
        assert!(lock_holds(&bp, &rules, &opts, Grant::Key(KeyTag::Tag(generic)), store), "seed {seed}");
    }
}

/// The house: the kitchen and the front room joined by a doorway, the front door in the south
/// wall under its mark, the two hatches on the kitchen's north wall.
#[test]
fn the_house_is_two_rooms_and_two_hatches() {
    for seed in 1..=common::seeds().min(16) {
        let bp = build_interior(ZoneId::House, seed).expect("the house");
        let kitchen = bp.rects[&name("kitchen")];
        assert_eq!(kitchen, Rect::new(2, 2, 15, 20));
        let open: Vec<i32> =
            (0..bp.h() as i32).filter(|&y| bp.tiles.read(17, y, Tile::Wall) == Tile::FloorWood).collect();
        assert_eq!(open.len(), 5, "seed {seed}: a doorway five cells high");
        assert!((8..=14).contains(&open[0]));
        for h in ["hatch_a", "hatch_b"] {
            let p = prop(&bp, h);
            assert_eq!(p.to.map(|t| t.zone), Some(ZoneId::Cellar));
            assert!(kitchen.contains(i32::from(p.cell.x), i32::from(p.cell.y)));
        }
        assert_eq!(prop(&bp, "front_door").to.map(|t| (t.zone, t.mark)), Some((ZoneId::County, name("house_front"))));
    }
}

/// Each hatch goes down on its own side: the west hatch to the stair in the cellar's west, the
/// east hatch to the east (the owner's first playtest found them crossed), and each stair comes
/// back up the hatch it went down.
#[test]
fn each_hatch_goes_down_on_its_own_side() {
    for seed in 1..=common::seeds().min(8) {
        let house = build_interior(ZoneId::House, seed).expect("the house");
        let cellar = build_interior(ZoneId::Cellar, seed).expect("the cellar");
        let mut downs: Vec<(u16, u16)> = ["hatch_a", "hatch_b"]
            .iter()
            .map(|h| {
                let p = prop(&house, h);
                let to = p.to.expect("a hatch leads down");
                let stair = cellar
                    .props
                    .iter()
                    .find(|s| s.to.is_some_and(|t| t.zone == ZoneId::House && t.mark == p.key))
                    .expect("a stair back up to it");
                assert_eq!(to.mark, stair.key, "seed {seed}: {h} arrives by the stair that returns to it");
                (p.cell.x, stair.cell.x)
            })
            .collect();
        downs.sort();
        assert!(downs[0].1 < downs[1].1, "seed {seed}: the west hatch goes to the west stair: {downs:?}");
    }
}

/// The judge judges: a house with its kitchen doorway bricked up is refused (the hatches, the orb
/// and the pantry are out of reach of the front door), and so is a cellar with no iron key.
#[test]
fn the_solver_refuses_a_broken_interior() {
    let mut bp = build_interior(ZoneId::House, 5).expect("the house");
    bp.tiles.fill_rect(Rect::new(17, 0, 2, bp.h() as i32), Tile::Wall);
    assert!(!validate(&bp, &ZoneRules::for_zone(ZoneId::House)).ok());

    let mut bp = build_interior(ZoneId::Cellar, 5).expect("the cellar");
    let chest = name("cellar_chest");
    bp.props.iter_mut().filter(|p| p.key == chest).for_each(|p| p.loot.clear());
    assert!(!validate(&bp, &ZoneRules::for_zone(ZoneId::Cellar)).ok());
}
