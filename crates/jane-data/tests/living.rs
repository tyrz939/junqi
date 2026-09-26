//! The living world as shipped (ARCHITECTURE.md §4.6, §6; WORLD.md): every system has a row in
//! `/data` that uses it, and the rows say what WORLD.md says they say.

use jane_core::action::{Action, Condition, FactKey};
use jane_core::ids::{Key, ZoneId};
use jane_data::{Region, ScheduleSlot, Sky, catalog};

fn name(s: &str) -> jane_core::NameId {
    catalog().name_id(s).unwrap_or_else(|| panic!("no name \"{s}\""))
}

/// `weather.json`: the three regions of WORLD.md §5.2, shares out of ten as weights out of a
/// thousand, every zone under one sky.
#[test]
fn the_weather_table_is_world_md_s() {
    let l = &catalog().living;
    let regions: Vec<Region> = l.weather.iter().map(|w| w.region).collect();
    assert_eq!(regions, [Region::Lowfields, Region::Waters, Region::Works]);
    for z in ZoneId::ALL {
        assert_eq!(l.weather.iter().filter(|w| w.zones.contains(&z)).count(), 1, "{z:?}");
    }
    assert_eq!(l.region_of(ZoneId::County), Region::Lowfields);
    assert_eq!(l.region_of(ZoneId::Museum), Region::Waters);
    assert_eq!(l.region_of(ZoneId::Factory), Region::Works);
    for w in l.weather {
        for h in 0..24 {
            let b = w.band_at(h).unwrap_or_else(|| panic!("{:?} {h}:00", w.region));
            assert_eq!(b.weights.iter().map(|&x| u32::from(x)).sum::<u32>(), 1000);
        }
    }
    // Lowfields by day: seven in ten clear; never a storm. The Works after the bell: two in ten.
    let low = l.sky(Region::Lowfields).unwrap();
    assert_eq!(low.band_at(8).unwrap().weights, [700, 100, 200, 0]);
    assert!((0..24).all(|h| low.band_at(h).unwrap().weights[Sky::Storm as usize] == 0));
    assert_eq!(l.sky(Region::Works).unwrap().band_at(23).unwrap().weights, [400, 200, 200, 200]);
    assert_eq!(l.sky(Region::Waters).unwrap().band_at(2).unwrap().weights[Sky::Mist as usize], 600);
    // The first walk is clear.
    assert!(l.tuning.clear_hours >= 1);
}

/// `ecology.json`: the kill patches of WORLD.md §4.1 hold what the placement rows put there.
#[test]
fn the_patches_have_their_populations() {
    let c = catalog();
    let rat = c.combat.unit_id("rat").unwrap();
    let e = c.living.ecology_of(name("allotments")).expect("the allotments");
    let p = e.population(rat).expect("rats");
    // Eight placed (four in the plots, four at the sheds, WORLD.md §4.1) and the odd wild one.
    assert!(p.cap >= 8, "{}", p.cap);
    // One kill does not hold a patch back; clearing it does, for hours: `recover` comes off every
    // ten game minutes (six marks an hour), and four kills are over the line by six hours of it.
    assert!(p.weight < p.hold && 4 * p.weight >= p.hold + 6 * 6 * e.recover);
    for area in ["top_field", "quarry_steps"] {
        assert!(c.living.ecology_of(name(area)).is_some(), "{area}");
    }
}

/// `consequences.json`: world verbs, more than a flag, a trigger that exists.
#[test]
fn consequences_change_the_world() {
    let c = catalog();
    let ids: Vec<&str> = c.living.consequences.iter().map(|r| r.id).collect();
    assert_eq!(ids, ["allotments_thinned", "house_kept", "mine_quiet", "yard_clear"]);
    let thinned = &c.living.consequences[0];
    assert_eq!(thinned.on, Condition::QuestDone(c.story.quest_id("rats_in_the_sheds").unwrap()));
    assert!(c.list(thinned.edits).contains(&Action::Despawn(Key::Name(name("rat_allotment_4")))));
    // Julie's Kitchen (WORLD.md §6): the house is hers; its door is night-locked, the bell's hours,
    // and her key opens it.
    let kept = &c.living.consequences[1];
    assert_eq!(kept.on, Condition::QuestDone(c.story.quest_id("see_the_kitchen").unwrap()));
    assert_eq!(kept.zone, ZoneId::County);
    let [Action::NightLock { prop, lock }] = c.list(kept.edits) else { panic!("one night lock") };
    assert_eq!(*prop, Key::Name(name("house_door")));
    assert!(lock.keyed && (lock.from, lock.to) == (21, 6));
    let quiet = &c.living.consequences[2];
    assert_eq!(quiet.on, Condition::Dead(Key::Name(name("iron_knuckles"))));
    assert_eq!(quiet.zone, ZoneId::County, "it fires in the mine and lands on the mine road");
    assert!(c.list(quiet.edits).iter().any(|a| matches!(a, Action::Spawn { .. })));
    let said = c.text(quiet.contradicts.expect("the sign's claim"));
    assert!(said.starts_with("GOLDSKIN MINING Co."), "{said}");
    // The Thing in the Yard: the fence line clear for good (WORLD.md §6).
    let yard = &c.living.consequences[3];
    assert_eq!(yard.on, Condition::QuestDone(c.story.quest_id("defeat_skeleton").unwrap()));
    assert_eq!(c.list(yard.edits), [Action::Despawn(Key::Name(name("yard_skeleton")))]);
}

/// A story that spreads, a door that hears, and the line that tells it.
#[test]
fn a_rumour_travels_to_a_door_that_can_say_it() {
    let c = catalog();
    let ames = c.county.stories.iter().find(|s| s.key == "ames").unwrap();
    let sp = ames.spreads.expect("ames spreads");
    assert_eq!(sp.to, [name("door_pound_3")]);
    assert_eq!(sp.after.0, 1440 * 60, "half a game day");
    let t = c.story.dialogue(c.story.dialogue_id("door_pound_3_heard").unwrap());
    let rule = c.conds_of(t.start[0].when.unwrap());
    assert_eq!(rule[0].c, Condition::SpeakerKnows(ames.id));
    let heard = t.node(t.node_index("heard").unwrap());
    assert_eq!(heard.lines[0].tells, [FactKey::Rumour(ames.id)]);
}

/// Mr Cobb's hours (WORLD.md §3.1): in the Arms after the bell, the yard in the morning, his
/// seat outside the Arms by day.
#[test]
fn mr_cobb_keeps_hours() {
    let c = catalog();
    let cobb = c.combat.unit(c.combat.unit_id("town_drinker").unwrap());
    assert!(!cobb.day_only && !cobb.night_only);
    let slot = |h: u8| cobb.schedule.iter().find(|r| jane_data::in_span(h, r.hour_from, r.hour_to)).unwrap().slot;
    assert_eq!(slot(23), ScheduleSlot::Inside(name("arms_door")));
    assert_eq!(slot(7), ScheduleSlot::Mark(name("arms_yard")));
    assert_eq!(slot(14), ScheduleSlot::Mark(name("arms_front")));
}

/// An open fire goes out in the rain; a lamp does not.
#[test]
fn a_campfire_douses_and_a_lamp_does_not() {
    let s = &catalog().story;
    assert_eq!(s.prop(s.prop_id("campfire").unwrap()).douse, Some(160));
    assert_eq!(s.prop(s.prop_id("lamp_post").unwrap()).douse, None);
    let t = catalog().living.tuning;
    assert_eq!(t.journal_ring, 512);
    assert!(t.wetness.rise > t.wetness.fall && t.wetness.every.0 > 0);
}
