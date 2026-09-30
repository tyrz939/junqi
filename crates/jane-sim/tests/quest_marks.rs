//! The quest marks and sparkles (PRESENTATION.md §3.8), as the `View` says them: a "?" over a
//! person with a quest to give her, a "!" over whoever takes a ready one back, nothing while a
//! quest is under way or once it is done; and the things a step still wants, until it has them.
//! Mr Hale and his three (`the_last_name`, `roberts_cap`, `the_new_stone`) on the real seed.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::QuestId;
use jane_sim::input::DevOp;
use jane_sim::state::{Dialogue, FlagKey, Speaker};
use jane_sim::view::QuestMark;
use jane_sim::{ClientToken, Command, Seat, Sim, StampedCommand, StepInput, UnitId};

fn quest(name: &str) -> QuestId {
    jane_data::catalog().story.quest_id(name).unwrap_or_else(|| panic!("no quest {name}"))
}

/// Mr Hale's id in the county.
fn hale(s: &Sim) -> UnitId {
    let sym = sym(s, "mr_hale");
    let z = s.state().zone(jane_core::ZoneId::County).unwrap();
    z.units.iter().find(|u| u.key == Some(sym)).expect("Mr Hale").id
}

fn mark(s: &Sim, seat: u8, who: UnitId) -> Option<QuestMark> {
    let v = s.view(Seat(seat)).unwrap();
    let u = s.state().zone(v.zone()).unwrap().unit(who).unwrap();
    v.quest_mark(u)
}

/// The memorial's words have been read: `the_last_name` is ready.
fn read_the_memorial(s: &mut Sim) {
    let n = jane_data::catalog().name_id("memorial_read").unwrap();
    s.state_mut().flags.insert(FlagKey::Been(jane_sim::sym::of_name(n)), 1);
}

/// Handed in, as `quests::hand_in` leaves the log (the rewards are the quest suite's).
fn hand_in(s: &mut Sim, q: QuestId) {
    let st = s.state_mut();
    st.quests.active.retain(|p| p.quest != q);
    st.quests.done.push(q);
}

/// Whether any prop of her zone talking as `tree` is something a step wants.
fn wanted(s: &Sim, tree: &str) -> bool {
    let v = s.view(Seat(0)).unwrap();
    let t = jane_data::catalog().story.dialogue_id(tree).unwrap();
    let w = v.quest_wants();
    v.props().any(|p| v.prop_spawn(p).is_some_and(|sp| sp.talk == Some(t)) && v.prop_wanted(&w, p))
}

#[test]
fn a_question_over_the_giver_nothing_while_it_is_under_way_then_the_hand_in() {
    let mut s = new_game();
    let h = hale(&s);
    assert_eq!(mark(&s, 0, h), Some(QuestMark::Offer), "he has the last name to ask about");
    assert!(!wanted(&s, "memorial"), "no step wants the memorial yet");
    cmd(&mut s, Command::Dev(DevOp::Quest(quest("the_last_name"))));
    assert_eq!(mark(&s, 0, h), None, "asked; nothing over him while she reads it");
    assert!(wanted(&s, "memorial"), "the memorial sparkles while the step wants it");
    read_the_memorial(&mut s);
    assert_eq!(mark(&s, 0, h), Some(QuestMark::HandIn), "ready: back to him");
    assert!(!wanted(&s, "memorial"), "read: it stops");
    hand_in(&mut s, quest("the_last_name"));
    // His next, then the last; with all three done there is nothing over him.
    assert_eq!(mark(&s, 0, h), Some(QuestMark::Offer), "Robert's cap");
    for q in ["roberts_cap", "the_new_stone"] {
        hand_in(&mut s, quest(q));
    }
    assert_eq!(mark(&s, 0, h), None, "nothing left to give her");
}

#[test]
fn a_quest_hidden_behind_another_shows_nothing_until_it_is_reached() {
    // Mr Hale's cap is behind the last name: with the last name under way his start rules open
    // on "name_wait", which gives nothing, so the cap's offer is no mark over him.
    let mut s = new_game();
    let h = hale(&s);
    cmd(&mut s, Command::Dev(DevOp::Quest(quest("the_last_name"))));
    assert_eq!(mark(&s, 0, h), None);
    // And at New Game only people are marked: no animal, no stranger with nothing to give.
    let v = s.view(Seat(0)).unwrap();
    let z = s.state().zone(v.zone()).unwrap();
    let cat = jane_data::catalog();
    let marked: Vec<&str> =
        z.units.iter().filter(|u| v.quest_mark(u).is_some()).map(|u| cat.combat.unit(u.def).id).collect();
    assert!(!marked.is_empty());
    assert!(!marked.iter().any(|d| d.contains("rabbit") || d.contains("hen") || d.starts_with("folk_")), "{marked:?}");
}

#[test]
fn each_seat_its_own_marks() {
    let mut s = new_game();
    cmd(&mut s, Command::Open(true));
    let cmds = [StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: ClientToken(11) } }];
    s.step(&StepInput { commands: &cmds, ..StepInput::IDLE });
    let start = sym(&s, "start");
    cmd_as(&mut s, 1, Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::County, mark: start }));
    idle(&mut s, 2);
    let h = hale(&s);
    assert_eq!(mark(&s, 0, h), Some(QuestMark::Offer));
    assert_eq!(mark(&s, 1, h), Some(QuestMark::Offer), "both of them see what he has to give");
    // Seat 1 goes into Julie's house: Mr Hale is not in her world now, and seat 0's mark stays.
    let front = sym(&s, "front");
    cmd_as(&mut s, 1, Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::House, mark: front }));
    idle(&mut s, 2);
    let v1 = s.view(Seat(1)).unwrap();
    assert!(v1.quest_marks().all(|(id, _)| id != h), "not in her zone");
    assert_eq!(mark(&s, 0, h), Some(QuestMark::Offer));
    // Talking to him is the presenter's to hide (a seat's own conversation), and the start rules
    // are asked as that seat: a speaker's knowledge, her own bag.
    s.state_mut().players[0].dialogue =
        Some(Dialogue { tree: None, node: 0, line: 0, speaker: Speaker::Unit(h), read: None });
    assert_eq!(mark(&s, 0, h), Some(QuestMark::Offer), "the view says it; the presenter hides it");
}
