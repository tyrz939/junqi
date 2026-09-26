//! The first minutes, played headless on the real seed. Carries `jane/test/sim.test.ts` "the
//! first five minutes, played headless" and `jane/test/bot.ts`'s walk: in at the yard gate; the
//! stoop hands the letter in; the dog gives the quest; the house door is locked until the dog
//! gives the key; the skeleton put down with the bar's melee (it does not fight back until the ai
//! lands; the kill counts through `hooks::quest_kill`); the key; the kitchen, the note, the pantry
//! chest, the bench's potion, the orb; and back out of the front door.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::ZoneId;
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::state::{FactKey, Source};
use jane_sim::{Command, Seat};

#[test]
fn letter_dog_key_kitchen_note_chest_bench_orb() {
    let cat = jane_data::catalog();
    let mut s = new_game();
    // New Game stands her on the station platform; the walk begins inside Julie's gate.
    let gate = sym(&s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    assert_eq!(s.state().hour(), 17);
    assert!(quest_active(&s, "the_letter") && !quest_done(&s, "the_letter"));

    // Walk up to the dog. Reaching the stoop completes the letter.
    assert!(walk_to_unit(&mut s, "dog"));
    assert!(quest_done(&s, "the_letter"), "the stoop hands the letter in");
    let stoop = sym(&s, "stoop");
    assert_eq!(s.view(Seat(0)).unwrap().known(FactKey::Place(stoop)).map(|k| k.how), Some(Source::Visited));

    // Talk to the dog; take the quest.
    s.drain_events();
    cmd(&mut s, Command::Use);
    let d = s.view(Seat(0)).unwrap().dialogue().expect("the dog talks");
    assert_eq!(d.tree, cat.story.dialogue_id("dog"));
    let dog = unit(&s, "dog");
    assert_eq!(s.view(Seat(0)).unwrap().known(FactKey::Person(dog.key.unwrap())).map(|k| k.how), Some(Source::Met));
    talk_through(&mut s, &[0]);
    assert!(quest_active(&s, "defeat_skeleton"));
    let given = cat.story.quest_id("defeat_skeleton").unwrap();
    assert!(
        s.drain_events().iter().any(|e| e.kind == EventKind::Toast(ToastKind::QuestGiven(given)) && e.to.is_none())
    );

    // The door is locked until the dog says so.
    assert!(walk_to_prop(&mut s, "house_door"));
    let door = prop_id(&s, "house_door");
    let focus = s.view(Seat(0)).unwrap().focus().expect("the door is in reach");
    assert_eq!(focus.verb, jane_sim::interact::Verb::Unlock);
    cmd(&mut s, Command::Use);
    assert!(s.drain_events().iter().any(|e| e.kind == EventKind::Toast(ToastKind::Locked { prop: door })));
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::County);

    // The thing in the yard: walk up to it and put it down (it does not fight back until the ai
    // lands, and the kill counts through `hooks::quest_kill`).
    let bones = unit(&s, "yard_skeleton");
    assert!(walk_to(&mut s, bones.pos, jane_core::Fx::from_px(20)));
    assert!(fight(&mut s, "yard_skeleton"));
    assert!(me(&s).alive);
    let q = s.state().quests.active.iter().find(|p| p.quest == given).unwrap();
    assert_eq!(q.counts[0], 1, "the kill is counted");

    assert!(walk_to_unit(&mut s, "dog"));
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[0]);
    assert!(quest_done(&s, "defeat_skeleton"));
    assert_eq!(holds(&s, "key_auntie_house"), 1);
    assert!(quest_active(&s, "see_the_kitchen"), "the reward list gives the next quest");

    // Unlock, then enter.
    assert!(walk_to_prop(&mut s, "house_door"));
    cmd(&mut s, Command::Use);
    assert!(!prop(&s, "house_door").locked);
    assert_eq!(holds(&s, "key_auntie_house"), 1, "a bound key is not used up");
    cmd(&mut s, Command::Use);
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::House);
    let house = sym(&s, "house");
    assert_eq!(s.view(Seat(0)).unwrap().known(FactKey::Place(house)).map(|k| k.how), Some(Source::Seen));

    // The note on the kitchen table: the kitchen is seen on the way, and the note gives the bench.
    assert!(walk_to_prop(&mut s, "julies_note"));
    assert!(quest_done(&s, "see_the_kitchen"));
    cmd(&mut s, Command::Use);
    assert_eq!(s.view(Seat(0)).unwrap().dialogue().unwrap().tree, cat.story.dialogue_id("julies_note"));
    assert!(s.frozen(), "alone, the world holds still while she reads");
    talk_through(&mut s, &[]);
    assert!(quest_active(&s, "stock_the_bench"));

    // The pantry chest.
    assert!(walk_to_prop(&mut s, "pantry_chest"));
    let before = (holds(&s, "pansy"), holds(&s, "small_water"), holds(&s, "gold_dust"));
    cmd(&mut s, Command::Use);
    let after = (holds(&s, "pansy"), holds(&s, "small_water"), holds(&s, "gold_dust"));
    assert!(after.0 > before.0 && after.1 > before.1 && after.2 > before.2, "{before:?} -> {after:?}");
    assert!(prop(&s, "pantry_chest").used);
    assert!(
        s.view(Seat(0))
            .unwrap()
            .focus()
            .is_none_or(|f| f.target != jane_sim::interact::FocusRef::Prop(prop_id(&s, "pantry_chest")))
    );

    // The bench: dust, water and a pansy, in any order.
    assert!(walk_to_prop(&mut s, "bench"));
    assert!(s.view(Seat(0)).unwrap().near_bench());
    let slot = |s: &jane_sim::Sim, item: &str| {
        let id = cat.combat.item_id(item).unwrap();
        s.state().players[0].bag.iter().position(|x| x.is_some_and(|x| x.item == id)).unwrap() as u8
    };
    for (i, item) in ["pansy", "small_water", "gold_dust"].into_iter().enumerate() {
        let b = slot(&s, item);
        cmd(&mut s, Command::CraftPut { bag: b, slot: i as u8 });
    }
    assert_eq!(s.view(Seat(0)).unwrap().craft_output().map(|o| o.0), cat.combat.item_id("potion_manashield"));
    cmd(&mut s, Command::CraftTake);
    assert_eq!(holds(&s, "potion_manashield"), 1);
    assert!(s.state().players[0].craft.iter().all(Option::is_none));
    let ready = cat.story.quest_id("stock_the_bench").unwrap();
    assert!(s.view(Seat(0)).unwrap().quests().any(|q| q.quest == ready && q.ready), "acquire reads the bag live");

    // The orb: touched, it teaches Icebolt to the world and the bar.
    assert!(walk_to_prop(&mut s, "ice_orb"));
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[]);
    let ice = cat.combat.spell_id("icebolt").unwrap();
    assert!(s.state().growth.spells.contains(&ice));
    assert!(s.state().players[0].bar.contains(&Some(jane_data::BarSlot::Spell(ice))));

    // And back out of the front door to where she came in.
    assert!(walk_to_prop(&mut s, "front_door"));
    cmd(&mut s, Command::Use);
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::County);
    let near_door = me(&s).pos;
    let front = s.blueprint(ZoneId::County).marks[&jane_core::Key::Name(cat.name_id("house_front").unwrap())];
    let (cx, cy) = near_door.cell();
    assert!((cx - i32::from(front.cell.x)).abs() <= 8 && (cy - i32::from(front.cell.y)).abs() <= 8);
}

#[test]
fn the_dog_only_says_the_city_dog_line_if_you_keep_bothering_it() {
    // rest.test.ts "the dog only says the city-dog line if you keep bothering it".
    let cat = jane_data::catalog();
    let mut s = new_game();
    let gate = sym(&s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    // Past the intro, nothing to hand in: the idle chain.
    s.state_mut().quests.done.push(cat.story.quest_id("defeat_skeleton").unwrap());
    assert!(walk_to_unit(&mut s, "dog"));
    let mut heard = Vec::new();
    for _ in 0..6 {
        cmd(&mut s, Command::Use);
        let d = s.view(Seat(0)).unwrap().dialogue().expect("the dog answers");
        assert_eq!(d.tree, cat.story.dialogue_id("dog"));
        heard.push(d.node.unwrap().id);
        talk_through(&mut s, &[]);
    }
    assert_eq!(heard, ["idle", "poke_1", "poke_2", "poke_3", "poke_4", "poke_5"]);
}
