//! Cupboards (`store.rs`): what goes in comes out again, a whole bag goes in keys and all, what
//! is kept is saved with the world, and the whole party shares one cupboard's shelves.

mod common;

use jane_core::action::Facing;
use jane_core::{Stack, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::input::DevOp;
use jane_sim::interact::{FocusRef, Verb};
use jane_sim::tuning::{BAG_SLOTS, STORE_SLOTS};
use jane_sim::{ClientToken, Command, Seat, Sim, StampedCommand, StepInput};

use common::bot::*;
use common::room::item;

/// A new game, seat 0 standing at Julie's dresser. Returns the dresser.
fn at_the_dresser() -> (Sim, jane_sim::ids::PropId) {
    let mut s = common::new_game();
    let front = sym(&s, "front");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::House);
    assert!(walk_to_prop(&mut s, "julies_dresser"), "the dresser can be walked up to");
    let dresser = prop_id(&s, "julies_dresser");
    (s, dresser)
}

fn kept(s: &Sim, seat: u8, prop: jane_sim::ids::PropId) -> Vec<Option<Stack>> {
    s.view(Seat(seat)).and_then(|v| v.store(prop).map(|a| a.to_vec())).expect("the cupboard is in reach")
}

fn kept_count(s: &Sim, prop: jane_sim::ids::PropId, name: &str) -> u32 {
    jane_sim::bag::bag_count(&kept(s, 0, prop), item(name))
}

fn bag_slot_of(s: &Sim, name: &str) -> u8 {
    s.state().players[0].bag.iter().position(|b| b.is_some_and(|b| b.item == item(name))).expect("in the bag") as u8
}

#[test]
fn a_cupboard_keeps_what_she_puts_in_and_gives_it_back() {
    let (mut s, dresser) = at_the_dresser();
    let v = s.view(Seat(0)).unwrap();
    assert_eq!(v.focus().map(|f| (f.target, f.verb)), Some((FocusRef::Prop(dresser), Verb::Open)));
    assert!(kept(&s, 0, dresser).iter().all(Option::is_none), "a new cupboard is empty");
    events(&mut s);
    cmd(&mut s, Command::Use);
    let ev = events(&mut s);
    assert!(
        ev.iter().any(|e| e.kind == EventKind::Store { prop: dresser } && e.to == Some(Seat(0))),
        "USE opens it, for her"
    );

    cmd(&mut s, Command::Dev(DevOp::Give { item: item("apple"), qty: 5 }));
    let apples = holds(&s, "apple");
    // A quick move: the whole stack in, into the first hole.
    let slot = bag_slot_of(&s, "apple");
    let n = s.state().players[0].bag[usize::from(slot)].unwrap().qty;
    cmd(&mut s, Command::StorePut { prop: dresser, bag: slot, to: None });
    assert_eq!(holds(&s, "apple"), apples - u32::from(n));
    assert_eq!(kept_count(&s, dresser, "apple"), u32::from(n));
    assert_eq!(kept(&s, 0, dresser)[0].map(|k| k.item), Some(item("apple")));
    // Dragged to a slot of its own, then within the cupboard, then back to a bag slot.
    cmd(&mut s, Command::StoreMove { prop: dresser, from: 0, to: 9 });
    assert_eq!(kept(&s, 0, dresser)[9].map(|k| k.qty), Some(n));
    let hole = s.state().players[0].bag.iter().position(Option::is_none).unwrap() as u8;
    cmd(&mut s, Command::StoreTake { prop: dresser, slot: 9, to: Some(hole) });
    assert_eq!(holds(&s, "apple"), apples);
    assert_eq!(s.state().players[0].bag[usize::from(hole)].map(|b| b.qty), Some(n));
    assert!(s.state().stores.is_empty(), "an emptied cupboard is saved as one never used");

    // Onto something else, the two swap.
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("rock"), qty: 1 }));
    let rock = bag_slot_of(&s, "rock");
    cmd(&mut s, Command::StorePut { prop: dresser, bag: rock, to: Some(3) });
    cmd(&mut s, Command::StorePut { prop: dresser, bag: hole, to: Some(3) });
    assert_eq!(kept(&s, 0, dresser)[3].map(|k| k.item), Some(item("apple")));
    assert_eq!(s.state().players[0].bag[usize::from(hole)].map(|b| b.item), Some(item("rock")));

    // Out of reach, nothing moves, and the view says it is not hers to see.
    place(&mut s, 25, 18, Facing::South);
    let before = s.state().stores.clone();
    cmd(&mut s, Command::StoreTake { prop: dresser, slot: 3, to: None });
    assert_eq!(s.state().stores, before, "a cupboard across the room is out of reach");
    assert!(s.view(Seat(0)).unwrap().store(dresser).is_none());
    // And a thing that is not a cupboard takes nothing.
    let bed = prop_id(&s, "julies_bed");
    cmd(&mut s, Command::StorePut { prop: bed, bag: hole, to: None });
    assert_eq!(s.state().stores, before);
}

#[test]
fn a_full_bag_goes_into_a_cupboard_keys_and_all_and_is_no_longer_held() {
    let (mut s, dresser) = at_the_dresser();
    let cat = jane_data::catalog();
    let q = cat.story.quest_id("rats_below").unwrap();
    cmd(&mut s, Command::Dev(DevOp::Quest(q)));
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("rat_meat"), qty: 3 }));
    assert!(jane_sim::quests::ready(s.state(), q), "three held: ready");
    for (name, qty) in [("key_basement", 1), ("gold_bar", 16), ("key_generic", 1)] {
        cmd(&mut s, Command::Dev(DevOp::Give { item: item(name), qty }));
    }
    // Every hole a stack of its own: a bag with no room left.
    let rock = item("rock");
    for b in s.state_mut().players[0].bag.iter_mut().filter(|b| b.is_none()) {
        *b = Some(Stack { item: rock, qty: 1 });
    }
    let bag: Vec<Stack> = s.state().players[0].bag.iter().flatten().copied().collect();
    assert_eq!(bag.len(), BAG_SLOTS, "a full bag");
    assert_eq!(STORE_SLOTS, BAG_SLOTS);
    cmd(&mut s, Command::StorePutAll { prop: dresser });
    assert!(s.state().players[0].bag.iter().all(Option::is_none), "all of it went in: {:?}", s.state().players[0].bag);
    for st in &bag {
        let had = jane_sim::bag::bag_count(&bag.iter().map(|b| Some(*b)).collect::<Vec<_>>(), st.item);
        assert_eq!(jane_sim::bag::bag_count(&kept(&s, 0, dresser), st.item), had, "all of {:?} is kept", st.item);
    }
    assert_eq!(kept_count(&s, dresser, "key_basement"), 1, "a key goes in like anything else");
    assert_eq!(holds(&s, "key_basement"), 0);
    assert!(!jane_sim::quests::ready(s.state(), q), "what is in a cupboard is not held");
    let slot = kept(&s, 0, dresser).iter().position(|k| k.is_some_and(|k| k.item == item("rat_meat"))).unwrap();
    cmd(&mut s, Command::StoreTake { prop: dresser, slot: slot as u8, to: None });
    assert!(jane_sim::quests::ready(s.state(), q), "taken out again: held again");
}

#[test]
fn what_is_kept_is_saved_with_the_world() {
    let (mut s, dresser) = at_the_dresser();
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("gold_bar"), qty: 7 }));
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("key_basement"), qty: 1 }));
    let bar = bag_slot_of(&s, "gold_bar");
    cmd(&mut s, Command::StorePut { prop: dresser, bag: bar, to: Some(5) });
    let key = bag_slot_of(&s, "key_basement");
    cmd(&mut s, Command::StorePut { prop: dresser, bag: key, to: None });
    let before = kept(&s, 0, dresser);
    let bytes = s.save();
    let loaded = Sim::from_save_with(&bytes, common::bps()).expect("the save loads");
    assert_eq!(loaded.state().stores, s.state().stores);
    assert_eq!(loaded.hash(), s.hash(), "the cupboard is in the hash as it is in the save");
    assert_eq!(kept(&loaded, 0, dresser), before);
    assert_eq!(before[5], Some(Stack { item: item("gold_bar"), qty: 7 }));
    // A different cupboard is a different hash.
    let mut other = loaded;
    cmd(&mut other, Command::StoreMove { prop: dresser, from: 5, to: 6 });
    assert_ne!(other.hash(), s.hash());
}

#[test]
fn the_party_shares_one_cupboards_shelves() {
    let (mut s, dresser) = at_the_dresser();
    cmd(&mut s, Command::Open(true));
    let cmds = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(11) } }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
    let front = sym(&s, "front");
    cmd_as(&mut s, 1, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    idle(&mut s, 2);
    let at = me(&s).pos;
    let (x, y) = at.cell();
    place_seat(&mut s, 1, x, y, Facing::North);
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("gold_bar"), qty: 3 }));
    let bar = bag_slot_of(&s, "gold_bar");
    cmd(&mut s, Command::StorePut { prop: dresser, bag: bar, to: Some(0) });
    assert_eq!(kept(&s, 1, dresser), kept(&s, 0, dresser), "both see the same shelves");
    assert_eq!(kept(&s, 1, dresser)[0].map(|k| k.qty), Some(3));
    let theirs = |s: &Sim| jane_sim::bag::bag_count(&s.state().players[1].bag[..], item("gold_bar"));
    let before = theirs(&s);
    cmd_as(&mut s, 1, Command::StoreTake { prop: dresser, slot: 0, to: None });
    assert_eq!(theirs(&s), before + 3, "what one put in, another takes out");
    assert!(kept(&s, 0, dresser).iter().all(Option::is_none));
    // Her USE opens it for her alone.
    events(&mut s);
    cmd_as(&mut s, 1, Command::Use);
    let ev = events(&mut s);
    let opened: Vec<_> = ev.iter().filter(|e| matches!(e.kind, EventKind::Store { .. })).collect();
    assert!(!opened.is_empty() && opened.iter().all(|e| e.to == Some(Seat(1))));
}
