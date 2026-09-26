//! One story, several people: the non-combat parts of `jane/test/coop.test.ts`. Everyone is paid
//! at a hand-in, once each; growth is the world's (a jar found by one grows them all, once, the
//! away and the late included; what one learns they all know); quest news goes to everyone and
//! what is hers goes to her; the acquire count is the party's.

mod common;

use jane_core::Action;
use jane_core::action::{Facing, Stat};
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::state::{Dialogue, Speaker};
use jane_sim::{ClientToken, Command, Seat, Sim, StampedCommand, StepInput};

use common::bot::*;
use common::room::{Room, item};

fn join(s: &mut Sim, who: u64) {
    let cmds = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(who) } }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
}

fn party(s: &mut Sim, n: u64) {
    cmd(s, Command::Open(true));
    for who in 1..n {
        join(s, 10 + who);
    }
    assert_eq!(s.state().party_size() as u64, n);
}

fn count(s: &Sim, seat: usize, name: &str) -> u32 {
    jane_sim::bag::bag_count(&s.state().players[seat].bag[..], item(name))
}

#[test]
fn everyone_is_paid_at_the_hand_in_once_each_whoever_did_the_talking() {
    let cat = jane_data::catalog();
    let mut s = common::new_game();
    party(&mut s, 3);
    let q = cat.story.quest_id("defeat_skeleton").unwrap();
    cmd(&mut s, Command::Dev(DevOp::Quest(q)));
    s.state_mut().quests.active.iter_mut().find(|p| p.quest == q).unwrap().counts[0] = 1;
    let dog = cat.story.dialogue_id("dog").unwrap();
    let tree = cat.story.dialogue(dog);
    let node = tree.node_index("reward").unwrap();
    let line = tree.node(node).lines.len() as u16 - 1;
    s.state_mut().players[2].dialogue =
        Some(Dialogue { tree: Some(dog), node, line, speaker: Speaker::None, read: None });
    s.drain_events();
    cmd_as(&mut s, 2, Command::Choose { option: 0 });
    for seat in 0..3 {
        assert_eq!(count(&s, seat, "key_auntie_house"), 1, "seat {seat} is paid");
    }
    let ev = s.drain_events().to_vec();
    let done = ev.iter().filter(|e| e.kind == EventKind::Toast(ToastKind::QuestDone(q))).count();
    assert_eq!(done, 1, "the news once, to everyone");
    assert!(ev.iter().any(|e| e.kind == EventKind::Toast(ToastKind::QuestDone(q)) && e.to.is_none()));
    // Each seat's loot is hers.
    let key = item("key_auntie_house");
    for seat in 0..3u8 {
        assert!(ev.iter().any(|e| e.kind == EventKind::Loot { item: key, qty: 1 } && e.to == Some(Seat(seat))));
    }
    // A `quest` row run once per seat gives the quest once.
    let kitchen = cat.story.quest_id("see_the_kitchen").unwrap();
    assert_eq!(s.state().quests.active.iter().filter(|p| p.quest == kitchen).count(), 1);
}

#[test]
fn a_jar_found_by_one_grows_everyone_once_the_away_and_the_late_included() {
    // coop.test.ts "a jar found by one makes all of them stronger, once, including the away and
    // the late".
    let mut r = Room::new(false);
    let jar = r.key("jar_one");
    let grow = r.list(vec![Action::Grow { stat: Stat::Strength, amount: 4, id: jar }]);
    r.prop("jar", "lever", 10, 16, |p| p.use_list = Some(grow));
    r.prop("same_jar", "lever", 10, 20, |p| p.use_list = Some(grow));
    let mut s = r.build();
    party(&mut s, 3);
    cmd_as(&mut s, 2, Command::Leave);
    let strength = |s: &Sim, seat: usize| {
        let p = &s.state().players[seat];
        match &p.parked {
            Some(b) => b.strength,
            None => s.state().zone(p.zone).unwrap().unit(p.unit).unwrap().strength,
        }
    };
    let before: Vec<u16> = (0..3).map(|i| strength(&s, i)).collect();
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    for (i, b) in before.iter().enumerate() {
        assert_eq!(strength(&s, i), b + 4, "seat {i}");
    }
    assert_eq!(s.state().growth.strength, 4);
    // The same jar again (a reward paid to every seat): nothing.
    place(&mut s, 9, 20, Facing::East);
    cmd(&mut s, Command::Use);
    assert_eq!(strength(&s, 0), before[0] + 4);
    // Whoever sits down later is caught up.
    join(&mut s, 99);
    assert_eq!(strength(&s, 3), before[0] + 4);
    // And the one who left has it when she comes back.
    join(&mut s, 12);
    assert_eq!(strength(&s, 2), before[2] + 4);
}

/// The console's growth (a test kit's, `DevOp::Grow`) is the same growth: the world's, on every
/// body at once, health and mana with it; it remembers no finding, so a second is a second.
#[test]
fn the_consoles_growth_is_the_worlds() {
    let mut s = common::new_game();
    party(&mut s, 2);
    let body = |s: &Sim, seat: usize| {
        let p = &s.state().players[seat];
        let u = s.state().zone(p.zone).unwrap().unit(p.unit).unwrap();
        (u.strength, u.spirit, jane_sim::units::max_hp(u).0)
    };
    let before = [body(&s, 0), body(&s, 1)];
    cmd(&mut s, Command::Dev(DevOp::Grow { stat: Stat::Strength, amount: 10 }));
    cmd(&mut s, Command::Dev(DevOp::Grow { stat: Stat::Strength, amount: 10 }));
    cmd(&mut s, Command::Dev(DevOp::Grow { stat: Stat::Spirit, amount: 6 }));
    for (seat, b) in before.iter().enumerate() {
        let a = body(&s, seat);
        assert_eq!((a.0, a.1), (b.0 + 20, b.1 + 6), "seat {seat}");
        assert_eq!(a.2, b.2 + 20 * 5_000, "seat {seat}: five health a point of strength");
    }
    assert_eq!((s.state().growth.strength, s.state().growth.spirit), (20, 6));
    assert!(s.state().growth.found.is_empty(), "nothing was found");
}

#[test]
fn what_one_learns_they_all_know_and_it_takes_a_bar_slot_once() {
    let cat = jane_data::catalog();
    let mut s = common::new_game();
    party(&mut s, 2);
    let ice = cat.combat.spell_id("icebolt").unwrap();
    s.drain_events();
    cmd_as(&mut s, 1, Command::Dev(DevOp::Learn(ice)));
    cmd_as(&mut s, 1, Command::Dev(DevOp::Learn(ice)));
    for seat in 0..2 {
        let bar = &s.state().players[seat].bar;
        assert_eq!(bar.iter().filter(|b| **b == Some(jane_data::BarSlot::Spell(ice))).count(), 1);
    }
    let ev = s.drain_events().to_vec();
    assert_eq!(ev.iter().filter(|e| e.kind == EventKind::Learn(ice)).count(), 1);
    assert!(s.state().growth.spells.contains(&ice));
}

#[test]
fn what_the_party_holds_between_them_counts_for_acquire() {
    let cat = jane_data::catalog();
    let mut s = common::new_game();
    party(&mut s, 2);
    let q = cat.story.quest_id("stock_the_bench").unwrap();
    cmd(&mut s, Command::Dev(DevOp::Quest(q)));
    let ready = |s: &Sim| s.view(Seat(0)).unwrap().quests().any(|v| v.quest == q && v.ready);
    assert!(!ready(&s));
    cmd_as(&mut s, 1, Command::Dev(DevOp::Give { item: item("potion_manashield"), qty: 1 }));
    assert!(ready(&s), "her friend holds it");
}
