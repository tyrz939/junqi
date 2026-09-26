//! The spine in STORY.md §4's order (QUEST-TREE.md §5), held on the real seed: after the mine
//! the dog offers the Museum, not the Burial, and each hand-in after that offers the next place
//! (the forest, the Factory, the Burial, the School) until the epilogue; and the Burial's stair
//! stays shut until she carries the Company's key from the Factory, whatever else she holds.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::ZoneId;
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::state::FlagKey;
use jane_sim::{Command, Seat, Sim};

/// Mark quests done and flags set, as a player who got this far would have them.
fn been_through(s: &mut Sim, quests: &[&str], flags: &[&str]) {
    let cat = jane_data::catalog();
    for q in quests {
        let id = cat.story.quest_id(q).unwrap_or_else(|| panic!("no quest {q}"));
        s.state_mut().quests.active.retain(|p| p.quest != id);
        if !s.state().quests.done.contains(&id) {
            s.state_mut().quests.done.push(id);
        }
    }
    for f in flags {
        let k = FlagKey::Named(sym(s, f));
        s.state_mut().flags.insert(k, 1);
    }
}

/// Walk up to the dog, press USE, and say which node answers.
fn the_dog_says(s: &mut Sim) -> String {
    assert!(walk_to_unit(s, "dog"), "walk to the dog");
    cmd(s, Command::Use);
    let d = s.view(Seat(0)).unwrap().dialogue().expect("the dog talks");
    d.node.map(|n| n.id.to_owned()).expect("a node")
}

#[test]
fn after_the_mine_the_dog_offers_the_museum_and_the_spine_runs_to_the_school() {
    let mut s = new_game();
    let gate = sym(&s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    let first_hour = ["the_letter", "defeat_skeleton", "see_the_kitchen", "stock_the_bench", "rats_below"];
    been_through(&mut s, &first_hour, &["offered_rats"]);

    // Before the mine is done, the dog offers the mine and nothing further.
    assert_eq!(the_dog_says(&mut s), "offer_mine");
    talk_through(&mut s, &[0]);
    assert!(quest_active(&s, "the_mine"));

    // The mine handed in: the next offer is the Museum, and the Burial is not offered.
    been_through(&mut s, &["the_mine"], &[]);
    assert_eq!(the_dog_says(&mut s), "offer_museum");
    talk_through(&mut s, &[0]);
    assert!(quest_active(&s, "the_museum"));
    assert!(!quest_active(&s, "the_burial"));

    // Each place done offers the next, in STORY's order.
    for (done, offer, next) in [
        ("the_museum", "offer_forest", "the_forest"),
        ("the_forest", "offer_factory", "the_factory"),
        ("the_factory", "offer_burial", "the_burial"),
        ("the_burial", "offer_school", "the_school"),
    ] {
        been_through(&mut s, &[done], &[]);
        assert_eq!(the_dog_says(&mut s), offer, "after {done}");
        talk_through(&mut s, &[0]);
        assert!(quest_active(&s, next), "{offer} gives {next}");
    }
    been_through(&mut s, &["the_school"], &[]);
    assert_eq!(the_dog_says(&mut s), "epilogue");
}

#[test]
fn a_boss_put_down_before_the_dog_asks_still_counts() {
    // The boss steps are places (`attendant_down`, ...) that the boss's death marks, so the
    // Museum done on the way home from the mine is not lost when the dog then asks for it.
    let cat = jane_data::catalog();
    let q = cat.story.quest(cat.story.quest_id("the_museum").unwrap());
    let down = cat.name_id("attendant_down").unwrap();
    assert!(q.requirements.iter().any(|r| r.target == jane_data::ReqTarget::Location(down)));
    let attendant = cat.combat.unit(cat.combat.unit_id("attendant").unwrap());
    let marks = cat.list(attendant.on_death.expect("the attendant's death does something"));
    assert!(
        marks.iter().any(|a| matches!(a, jane_core::action::Action::Location(jane_core::Key::Name(n)) if *n == down))
    );
}

#[test]
fn the_burial_stair_is_shut_until_the_factorys_key() {
    let mut s = new_game();
    let mouth = sym(&s, "burial_mouth");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: mouth }));
    idle(&mut s, 3);
    assert!(walk_to_prop(&mut s, "burial_door"));
    let door = prop_id(&s, "burial_door");
    let give = |s: &mut Sim, item: &str| {
        let id = jane_data::catalog().combat.item_id(item).unwrap();
        cmd(s, Command::Dev(DevOp::Give { item: id, qty: 1 }));
    };
    let try_it = |s: &mut Sim| -> bool {
        s.drain_events();
        cmd(s, Command::Use);
        let locked = s.drain_events().iter().any(|e| e.kind == EventKind::Toast(ToastKind::Locked { prop: door }));
        idle(s, 2);
        locked
    };

    // Shut with nothing, and shut with the Burial's own Glasshouse Key, which is inside.
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(jane_sim::interact::Verb::Unlock));
    assert!(try_it(&mut s), "the stair is locked");
    give(&mut s, "key_burial");
    assert!(try_it(&mut s), "the Glasshouse Key does not fit it");
    assert!(prop(&s, "burial_door").locked);
    assert_eq!(zone_of(&s), ZoneId::County);

    // The Company's key from the Factory's locker opens it, and is kept.
    give(&mut s, "key_stone");
    assert!(!try_it(&mut s));
    assert!(!prop(&s, "burial_door").locked);
    assert_eq!(holds(&s, "key_stone"), 1, "a story key is not spent");
    cmd(&mut s, Command::Use);
    idle(&mut s, 3);
    assert_eq!(zone_of(&s), ZoneId::Burial);
}
