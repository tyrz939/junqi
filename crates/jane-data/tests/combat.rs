//! Rules the compiled combat tables keep, on the real /data: what `validateCatalog` (catalog.ts)
//! promised at boot, and the rows the TypeScript tests named. Each test says what it carries.

use jane_core::action::{Action, Heal, School};
use jane_core::num::{Fx, METRE_FX, Milli, Permille, Tick};
use jane_core::{Angle, ListRef};
use jane_data::{Catalog, Controller, Faction, SpellDef, SpellKind, UnitDef, UnitSight, catalog};

fn c() -> &'static Catalog {
    catalog()
}

fn unit(id: &str) -> &'static UnitDef {
    let c = c();
    c.combat.unit(c.combat.unit_id(id).unwrap_or_else(|| panic!("no unit \"{id}\"")))
}

fn spell(id: &str) -> &'static SpellDef {
    let c = c();
    c.combat.spell(c.combat.spell_id(id).unwrap_or_else(|| panic!("no spell \"{id}\"")))
}

fn list(r: Option<ListRef>) -> &'static [Action] {
    r.map_or(&[], |r| c().list(r))
}

#[test]
fn every_table_is_there_in_id_order() {
    let k = &c().combat;
    assert_eq!(k.spells.len(), 24);
    assert_eq!(k.effects.len(), 20);
    assert_eq!(k.units.len(), 149);
    for ids in [
        k.spells.iter().map(|s| s.id).collect::<Vec<_>>(),
        k.effects.iter().map(|e| e.id).collect(),
        k.units.iter().map(|u| u.id).collect(),
    ] {
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids are in sorted-string order");
    }
}

/// catalog.ts validateCatalog: a bolt needs speed, melee and bolt need power, power divisors > 0,
/// ground needs radius and duration, a world spell needs a verb, touch is a bolt's.
#[test]
fn spells_are_what_their_kind_needs() {
    for s in c().combat.spells {
        let at = s.id;
        match s.kind {
            SpellKind::Bolt => assert!(s.speed.is_some_and(|v| v > Fx(0)), "{at}: a bolt has speed"),
            _ => assert!(s.touch.is_none(), "{at}: touch is a bolt's"),
        }
        if matches!(s.kind, SpellKind::Melee | SpellKind::Bolt) {
            assert!(s.power.is_some(), "{at}: needs power");
        }
        if let Some(p) = s.power {
            assert!(p.div > 0 && p.var_div > 0, "{at}");
        }
        assert_eq!(s.ground.is_some(), s.kind == SpellKind::Ground, "{at}");
        if let Some(g) = s.ground {
            assert!(g.radius > Fx(0) && g.duration > Tick(0) && g.pulse > Tick(0), "{at}");
        }
        assert_eq!(s.world.is_some(), s.kind == SpellKind::World, "{at}");
        assert!(s.count >= 1 && s.range >= Fx(0) && s.mp >= Milli(0), "{at}");
    }
}

#[test]
fn spell_numbers_convert_as_the_architecture_says() {
    // data/spells.json icebolt: range 15 m, cooldown 2 s, speed 5 px per tick, div 1.25, splash 25 px.
    let ice = spell("icebolt");
    assert_eq!(ice.range, Fx(15 * METRE_FX));
    assert_eq!(ice.cooldown, Tick(120));
    assert_eq!(ice.speed, Some(Fx(5 * 256)));
    assert_eq!(ice.power.map(|p| (p.div, p.var_div)), Some((1250, 8000)));
    assert_eq!(ice.splash.map(|s| (s.radius, s.div)), Some((Fx(25 * 256), 5)));
    assert_eq!(ice.stop, Tick(30));
    assert_eq!(ice.effect, c().combat.effect_id("chilled"));
    // The snake's ring is 360 degrees of evenly spaced bolts; the cactus sprays inside 45.
    assert_eq!((spell("snake_ring").count, spell("snake_ring").fan), (15, Angle(u16::MAX)));
    assert_eq!((spell("cactus_spray").count, spell("cactus_spray").fan), (10, Angle(8192)));
}

/// catalog.ts buildCatalog: effect pulses come at most once a tick; resists are multipliers.
#[test]
fn effects_are_well_formed() {
    let k = &c().combat;
    for e in k.effects {
        if let Some(p) = e.pulse {
            assert!(p.every >= Tick(1), "{}", e.id);
        }
        assert!((0..=1000).contains(&e.speed.0), "{}: speed is a multiplier", e.id);
        assert_ne!(e.only_if_weak, Some(School::Heal), "{}", e.id);
    }
    let chilled = k.effect(k.effect_id("chilled").unwrap());
    assert_eq!((chilled.duration, chilled.speed), (Tick(180), Permille(600)));
    // effects/verbs.json: jolted stops a machine for half a second, only what is weak to shock.
    let jolted = k.effect(k.effect_id("jolted").unwrap());
    assert_eq!((jolted.duration, jolted.speed, jolted.stun), (Tick(30), Permille(0), true));
    assert_eq!(jolted.only_if_weak, Some(School::Shock));
}

/// catalog.ts validateCatalog: strength > 0; a friendly unit is an npc or the player; the snake
/// controller needs a body (and, here, only the snake has one).
#[test]
fn units_keep_the_catalog_rules() {
    for u in c().combat.units {
        assert!(u.strength > 0, "{}", u.id);
        if u.faction == Faction::Friendly {
            assert!(matches!(u.controller, Controller::Npc | Controller::Player), "{}", u.id);
        }
        assert_eq!(u.body.is_some(), u.controller == Controller::Snake, "{}: every snake has a body", u.id);
        assert!(!(u.day_only && u.night_only), "{}", u.id);
        assert!(u.bounds > Fx(0), "{}", u.id);
        for l in u.loot {
            assert!(l.qty >= 1 && l.chance > Permille(0) && l.chance <= Permille::ONE, "{}", u.id);
        }
    }
    assert!(c().combat.units.iter().any(|u| u.controller == Controller::Snake));
}

/// ARCHITECTURE.md §6: phases in falling `hpBelow`, each a fraction of full health; the snake
/// reads its table by its own clock (snake.ts).
#[test]
fn phases_fall_in_hp_below() {
    let mut bosses = 0;
    for u in c().combat.units {
        for p in u.phases {
            assert!(p.hp_below > Permille(0) && p.hp_below <= Permille::ONE, "{}", u.id);
        }
        if u.controller != Controller::Snake && !u.phases.is_empty() {
            bosses += 1;
            assert!(u.phases.windows(2).all(|w| w[1].hp_below < w[0].hp_below), "{}", u.id);
        }
    }
    assert!(bosses >= 8, "{bosses} phased bosses");
}

#[test]
fn unit_numbers_convert_as_the_architecture_says() {
    // data/units.json bat: aggro 14 m, leash 110 m, bounds 0.75 m, walk 0.8 and run 1.7 px per tick, respawn 600 s.
    let bat = unit("bat");
    assert_eq!(bat.aggro, Fx(14 * METRE_FX));
    assert_eq!(bat.leash, Fx(110 * METRE_FX));
    assert_eq!(bat.bounds, Fx(1536));
    assert_eq!((bat.walk, bat.run), (Fx(205), Fx(435)));
    assert_eq!(bat.respawn, Tick(36000));
    // The 2020 bat wore a red lamp.
    assert_eq!(bat.glow.map(|g| (g.radius, g.color)), Some((Fx(24 * 256), 0xff2020)));
    // Jane: the player walks 1 and runs 2 px per tick, as 2020's did.
    let jane = unit("jane");
    assert_eq!((jane.walk, jane.run, jane.controller), (Fx(256), Fx(512), Controller::Player));
}

/// forest.test.ts: day and night are two forests: the butterflies are day only, the moth and
/// the spiders are not.
#[test]
fn the_forest_has_a_day_and_a_night() {
    for n in 1..=7 {
        assert!(unit(&format!("forest_butterfly_{n}")).day_only, "butterfly {n}");
    }
    assert!(unit("forest_moth_8").night_only);
    assert!(unit("forest_night_spider").night_only);
    // catalog.ts dayOnlyAfter: the dog keeps Julie's hours only once the skeleton is dealt with.
    let dog = unit("dog");
    assert!(dog.day_only);
    // The quest resolved (to `defeat_skeleton`; quests are the story group's table).
    assert!(dog.day_only_after.is_some());
}

/// factory.test.ts: the machines only see what is lit, and the Foreman is plated against
/// everything she owns and weak to the one verb this place gives.
#[test]
fn the_factory_machines() {
    assert_eq!(unit("sentry").sight, UnitSight::Lit);
    assert_eq!(unit("hauler").sight, UnitSight::Lit);
    let f = unit("foreman");
    assert!(f.resist_of(School::Shock) < Permille(0));
    for s in [School::Physical, School::Frost, School::Fire, School::Blast] {
        assert!(f.resist_of(s) < Permille::ONE, "{s:?}");
        assert!(f.resist_of(s) > Permille(0), "{s:?}");
    }
    assert_eq!(f.resist_of(School::Nature), Permille(0), "a school left out is no reduction");
}

/// school.test.ts: the Caretaker is a night-only mini-boss who carries the tower key, and the
/// Ringer is 5,000 HP at phase 6 with three phases that each do something.
#[test]
fn the_school_bosses() {
    let k = &c().combat;
    let care = unit("caretaker");
    assert!(care.night_only && care.boss);
    let key = k.item_id("key_tower").unwrap();
    assert!(care.loot.iter().any(|l| l.item == key && l.qty == 1 && l.chance == Permille::ONE));
    let ringer = unit("ringer");
    assert_eq!(u32::from(ringer.strength) * 5 * 8, 5000);
    assert_eq!(ringer.phases.len(), 3);
    for p in ringer.phases {
        assert!(!list(p.on_enter).is_empty());
    }
}

/// items.json apple: 0.25 is a fraction of max hp (lists.rs heal's rule): the 2020 apple healed
/// 25 % while its tooltip said 25.
#[test]
fn the_apple_heals_a_quarter() {
    let k = &c().combat;
    let apple = k.item(k.item_id("apple").unwrap());
    assert_eq!(list(apple.use_list), &[Action::Heal(Heal::Pct(Permille(250)))]);
}
