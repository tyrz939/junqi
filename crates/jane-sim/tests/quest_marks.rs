//! The quest marks and sparkles (PRESENTATION.md §3.8), as the `View` says them: a "!" over a
//! person with a quest to give her, a "?" over whoever takes a ready one back, nothing while a
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

/// "A place" for the crowd rule: forty cells round any "!" (PLAY-PLAN.md 0.4).
const CROWD_CELLS: i32 = 40;
/// The most a place may show at once.
const CROWD_MAX: usize = 3;

/// The Castle's own errands, the people and the board, as the town's start rules give them.
const TOWN: [(&str, &str); 19] = [
    ("parish_board", "footpath_three"),
    ("parish_board", "rats_in_the_sheds"),
    ("parish_board", "plot_nine"),
    ("mr_hale", "the_last_name"),
    ("mr_hale", "roberts_cap"),
    ("mr_hale", "the_new_stone"),
    ("mr_sallis", "white_roses"),
    ("mr_dunn", "on_the_hour"),
    ("mr_dunn", "the_back_room"),
    ("dot", "the_fourth_lane"),
    ("dot", "school_lane"),
    ("constable", "the_constables_paces"),
    ("tilly", "sixpence"),
    ("miss_dray", "second_post"),
    ("mrs_oddie", "two_loaves"),
    ("milkman", "paid_to_sunday"),
    ("mrs_marsh", "washing_day"),
    ("mrs_bex", "never_any_eggs"),
    ("mrs_hobb", "too_red"),
];

fn set_flag(s: &mut Sim, name: &str, value: i32) {
    let flag = sym(s, name);
    cmd(s, Command::Dev(DevOp::Flag { flag, value }));
}

/// Whoever (or whatever) of the county talks as `tree`.
fn speaker_of(s: &Sim, tree: &str) -> Speaker {
    let cat = jane_data::catalog();
    let t = cat.story.dialogue_id(tree).unwrap_or_else(|| panic!("no tree {tree}"));
    let v = s.view(Seat(0)).unwrap();
    let z = s.state().zone(jane_core::ZoneId::County).unwrap();
    if let Some(u) = z.units.iter().find(|u| cat.combat.unit(u.def).talk == Some(t)) {
        return Speaker::Unit(u.id);
    }
    let p = v.props().find(|p| v.prop_spawn(p).is_some_and(|sp| sp.talk == Some(t)));
    Speaker::Prop(p.unwrap_or_else(|| panic!("nobody talks as {tree}")).id)
}

fn offers(s: &Sim, tree: &str, q: &str) -> bool {
    let t = jane_data::catalog().story.dialogue_id(tree).unwrap();
    s.view(Seat(0)).unwrap().would_offer(t, speaker_of(s, tree), quest(q))
}

#[track_caller]
fn no_crowd(s: &Sim, seed: u32, when: &str) {
    let c = s.view(Seat(0)).unwrap().offer_crowd(CROWD_CELLS);
    assert!(c.len() <= CROWD_MAX, "seed {seed} {when}: {} \"!\" within {CROWD_CELLS} cells: {c:?}", c.len());
}

#[test]
fn the_castle_never_shows_more_than_three_offers_and_every_errand_comes_round() {
    for seed in 1..=8 {
        let mut s = Sim::new_game_with(jane_sim::Blueprints::build(seed).unwrap(), "Jane");
        idle(&mut s, 2);
        no_crowd(&s, seed, "the first evening");
        // The first morning, before and after the board; the constable already met.
        set_flag(&mut s, "mornings", 1);
        no_crowd(&s, seed, "the first morning");
        assert!(!offers(&s, "tilly", "sixpence"), "seed {seed}: Tilly waits for the board or a second day");
        set_flag(&mut s, "read_board", 1);
        assert!(offers(&s, "tilly", "sixpence"), "seed {seed}: the board is the way in");
        set_flag(&mut s, "talked_constable", 1);
        no_crowd(&s, seed, "the first morning, the board read");
        // Each day: everything offered is taken (all at once, the worst case), then all of it
        // done; until nothing more comes round that day.
        for day in 1..=4 {
            set_flag(&mut s, "mornings", day);
            no_crowd(&s, seed, &format!("morning {day}"));
            loop {
                let open: Vec<&str> = TOWN.iter().filter(|(t, q)| offers(&s, t, q)).map(|e| e.1).collect();
                if open.is_empty() {
                    break;
                }
                for q in &open {
                    cmd(&mut s, Command::Dev(DevOp::Quest(quest(q))));
                    no_crowd(&s, seed, &format!("day {day}, {q} taken"));
                }
                for q in &open {
                    hand_in(&mut s, quest(q));
                    no_crowd(&s, seed, &format!("day {day}, {q} done"));
                }
            }
        }
        let undone: Vec<&str> =
            TOWN.iter().map(|e| e.1).filter(|q| !s.state().quests.done.contains(&quest(q))).collect();
        assert!(undone.is_empty(), "seed {seed}: never offered {undone:?}");
    }
}
