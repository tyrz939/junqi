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
    assert_eq!(k.spells.len(), 33);
    assert_eq!(k.effects.len(), 21);
    assert_eq!(k.units.len(), 167);
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
    // The snake's ring is 360 degrees of evenly spaced bolts; the cactus sprays six inside 45 (PLAY-PLAN.md 0.3).
    assert_eq!((spell("snake_ring").count, spell("snake_ring").fan), (15, Angle(u16::MAX)));
    assert_eq!((spell("cactus_spray").count, spell("cactus_spray").fan), (6, Angle(8192)));
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

/// PLAN.md §2.6 *Aggro* and *Leash* (2026-10-06; the leash half again longer from the owner's
/// playtest, 2026-10-07): a hostile that is not a boss notices her from no further than 10 m (a
/// little inside a third of the 40 x 22.5 view) and follows her three and three quarter times
/// that from home; the yard's bones by Julie's gate twice that again, so they are not snapped
/// home while the gate is still on the screen; a boss keeps its arena's rows.
#[test]
fn hostile_rows_notice_inside_the_screen_and_leash_at_three_and_three_quarter_times_it() {
    let cat = jane_data::catalog();
    let mut n = 0;
    for u in cat.combat.units.iter().filter(|u| u.aggro.0 > 0 && !u.boss) {
        n += 1;
        // A row fed rather than fought (the Burial's lurker) keeps the notice its puzzle was
        // built round: the bait is thrown from outside it and smelt from twice it.
        assert!(u.aggro.0 <= 10 * METRE_FX || u.bait.is_some(), "{}: aggro {}", u.id, u.aggro.0);
        let quarters = if u.id == "yard_bones" { 30 } else { 15 };
        assert_eq!(
            i64::from(u.leash.0) * 4,
            i64::from(u.aggro.0) * quarters,
            "{}: leash not {quarters}/4 x aggro",
            u.id
        );
    }
    assert!(n >= 30, "{n} hostile rows");
}

#[test]
fn unit_numbers_convert_as_the_architecture_says() {
    // data/units.json bat: aggro 7 m, leash 26.25 m, bounds 0.75 m, walk 0.8 and run 1.7 px per tick, respawn 600 s.
    let bat = unit("bat");
    assert_eq!(bat.aggro, Fx(7 * METRE_FX));
    assert_eq!(bat.leash, Fx(105 * METRE_FX / 4));
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
    // The dog keeps Julie's hours only once the skeleton is dealt with: gone from the bell to six
    // after `defeat_skeleton`, on the step the rest, and at a meeting place while the spine asks
    // (WORLD.md §3.6). The overrides come first; the plain row covers the whole day.
    let dog = unit("dog");
    assert!(!dog.day_only);
    // Two rows the endings hold: gone for good once the night goes (the Ball back in the hill),
    // and at the Halt on the day she signals the Sunday train (STORY.md §10).
    let gone = dog.schedule.iter().find(|r| r.slot == jane_data::ScheduleSlot::Absent && r.hour_from == r.hour_to);
    assert!(gone.is_some_and(|r| matches!(r.when, Some(jane_data::ScheduleWhen::Flag(_)))), "gone with the night");
    let halt = dog.schedule.iter().filter(|r| matches!(r.when, Some(jane_data::ScheduleWhen::Flag(_)))).count();
    assert_eq!(halt, 2, "gone with the night, and at the Halt for the train");
    let night = dog
        .schedule
        .iter()
        .find(|r| r.slot == jane_data::ScheduleSlot::Absent && r.hour_from != r.hour_to)
        .expect("a night row");
    assert_eq!((night.hour_from, night.hour_to), (21, 6));
    assert!(matches!(night.when, Some(jane_data::ScheduleWhen::After(_))));
    let meets = dog.schedule.iter().filter(|r| matches!(r.when, Some(jane_data::ScheduleWhen::While(_)))).count();
    assert_eq!(meets, 4, "the Museum's steps, the graveyard gate twice, the top of Church Lane");
    let last = dog.schedule.last().expect("rows");
    assert!(last.when.is_none() && last.hour_from == last.hour_to, "the step, all day");
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
/// Ringer is 3,000 HP at phase 6 (DUNGEONS.md §3.6, as played) with three phases that each do
/// something, and tolls on the count.
#[test]
fn the_school_bosses() {
    let k = &c().combat;
    let care = unit("caretaker");
    assert!(care.night_only && care.boss);
    let key = k.item_id("key_tower").unwrap();
    assert!(care.loot.iter().any(|l| l.item == key && l.qty == 1 && l.chance == Permille::ONE));
    let ringer = unit("ringer");
    assert_eq!(u32::from(ringer.strength) * 5 * 8, 3000);
    assert!(ringer.book.iter().any(|&s| k.spell(s).id == "school_toll"));
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

/// What can be had again (`ItemDef::replaceable`), on the real data: the county's furnishing, a
/// respawning unit's drop, a recipe and a trade over those; never a one-off. Printed: what
/// destroy keeps.
#[test]
fn what_can_be_had_again_and_what_destroy_keeps() {
    let k = &c().combat;
    let is = |id: &str| k.item(k.item_id(id).unwrap_or_else(|| panic!("no item {id}"))).replaceable;
    // Lying about the county (herbs, chests, orchards).
    for id in ["apple", "wood", "small_water", "pansy", "white_water_rose", "coal", "iron"] {
        assert!(is(id), "{id}: the country has it lying about");
    }
    // Dropped by what stands up again: rats (rat meat), quarrymen (rock).
    assert!(is("rat_meat") && is("rock"));
    // Made from those: stone from rock, Stone Skin from stone, water and a rose, poisoned meat.
    assert!(is("stone") && is("potion_stoneskin") && is("poisoned_rat_meat"));
    // Once each: a boss's drop, what is made only from it, a quest's one thing, a tale's.
    for id in ["gold_bar", "gold_dust", "lost_glove", "net", "tale_lamp_oil"] {
        assert!(!is(id), "{id}: nothing gives a second");
    }
    assert!(k.items.iter().filter(|d| d.opens.is_some()).all(jane_data::ItemDef::kept), "every key is kept");
    let kept: Vec<&str> = k.items.iter().filter(|d| d.kept()).map(|d| d.id).collect();
    println!("destroy keeps {} of {} items: {}", kept.len(), k.items.len(), kept.join(", "));
}
