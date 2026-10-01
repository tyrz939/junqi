//! The School act's close and the town that notices (STORY.md §7, §10, §11): the dog takes The
//! Bell at Nine back and gives Yours to Say; each of the three endings is a place and an act in the
//! world (the ring on the study desk, the hollow at the back of the mine's vault, the Sunday train
//! at Castle Halt), each hands the choice in, sets the world's `the_end` and plays its own last page;
//! and what she did reaches the people who would hear it first, when they would hear it, and moves
//! where they stand and what the county shows, out of her sight.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::action::Facing;
use jane_core::num::CELL_FX;
use jane_core::{Fx, Tick, Vec2, ZoneId};
use jane_sim::event::EventKind;
use jane_sim::input::DevOp;
use jane_sim::state::FlagKey;
use jane_sim::tuning::TICKS_PER_HOUR;
use jane_sim::{Command, Seat, Sim};

const SPINE: [&str; 11] = [
    "the_letter",
    "defeat_skeleton",
    "see_the_kitchen",
    "stock_the_bench",
    "rats_below",
    "the_mine",
    "the_museum",
    "the_forest",
    "the_factory",
    "the_burial",
    "the_school",
];
const OFFERED: [&str; 8] = [
    "offered_rats",
    "offered_mine",
    "offered_museum",
    "offered_forest",
    "offered_factory",
    "offered_burial",
    "offered_school",
    "offered_choice",
];

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

fn flag(s: &Sim, name: &str) -> i32 {
    s.state().syms.find(name).and_then(|k| s.state().flags.get(&FlagKey::Named(k)).copied()).unwrap_or(0)
}

fn give(s: &mut Sim, item: &str) {
    let id = jane_data::catalog().combat.item_id(item).unwrap();
    cmd(s, Command::Dev(DevOp::Give { item: id, qty: 1 }));
}

fn given(s: &mut Sim, quest: &str) {
    let q = jane_data::catalog().story.quest_id(quest).unwrap();
    cmd(s, Command::Dev(DevOp::Quest(q)));
}

/// Talk it through, taking `choices` in order at each choice, and say every node it passed.
fn conversation(s: &mut Sim, choices: &[u8]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut n = 0;
    for _ in 0..200 {
        let Some(d) = s.view(Seat(0)).and_then(|v| v.dialogue()) else { return seen };
        if let Some(node) = d.node {
            if seen.last().is_none_or(|l| l != node.id) {
                seen.push(node.id.to_owned());
            }
        }
        if d.awaiting_choice {
            let option = choices.get(n).copied().unwrap_or(0);
            n += 1;
            cmd(s, Command::Choose { option });
        } else {
            cmd(s, Command::Advance);
        }
    }
    panic!("a conversation that does not end");
}

/// Every line of a node, as she reads it.
fn lines_of(tree: &str, node: &str) -> Vec<&'static str> {
    let cat = jane_data::catalog();
    let t = cat.story.dialogue(cat.story.dialogue_id(tree).unwrap_or_else(|| panic!("no tree {tree}")));
    let n = t.nodes.iter().find(|n| n.id == node).unwrap_or_else(|| panic!("no node {tree}.{node}"));
    n.lines.iter().map(|l| cat.text(l.text)).collect()
}

/// Stand beside a prop of her zone, on its first open side, facing it, and press USE.
fn use_prop(s: &mut Sim, key: &str) {
    let p = prop(s, key);
    let def = jane_data::catalog().story.prop(p.def);
    let (x, y, w, h) = (i32::from(p.cell.x), i32::from(p.cell.y), i32::from(def.w), i32::from(def.h));
    let sides = [
        (x + w / 2, y + h, Facing::North),
        (x + w / 2, y - 1, Facing::South),
        (x - 1, y + h / 2, Facing::East),
        (x + w, y + h / 2, Facing::West),
    ];
    let rt = s.runtime(zone_of(s)).expect("runtime");
    let (cx, cy, f) = sides.into_iter().find(|&(cx, cy, _)| !rt.grid.solid(cx, cy)).expect("an open side");
    place(s, cx, cy, f);
    let c = Vec2::new(Fx(x * CELL_FX + w * CELL_FX / 2), Fx(y * CELL_FX + h * CELL_FX / 2));
    face(s, c);
    cmd(s, Command::Use);
}

/// Stand on an open cell beside a unit, face it, and press USE (a sign beside it would answer a
/// diagonal).
fn talk_to(s: &mut Sim, key: &str) {
    let (x, y) = unit(s, key).pos.cell();
    let rt = s.runtime(zone_of(s)).expect("runtime");
    let sides =
        [(x, y + 1, Facing::North), (x, y - 1, Facing::South), (x - 1, y, Facing::East), (x + 1, y, Facing::West)];
    let (cx, cy, f) = sides.into_iter().find(|&(cx, cy, _)| !rt.grid.solid(cx, cy)).expect("an open side");
    place(s, cx, cy, f);
    let at = unit(s, key).pos;
    face(s, at);
    cmd(s, Command::Use);
}

/// Through the spine to the Timekeeper's fall, with the Ball in her bag, the dog at the top of
/// Church Lane in the morning: it takes The Bell at Nine back and gives Yours to Say.
fn to_the_choice() -> Sim {
    let mut s = new_game();
    been_through(&mut s, &SPINE[..10], &OFFERED[..7]);
    given(&mut s, "the_school");
    give(&mut s, "the_ball");
    give(&mut s, "key_stair");
    for place in ["school", "ringer_down"] {
        let k = FlagKey::Been(sym(&s, place));
        s.state_mut().flags.insert(k, 1);
    }
    // Away and back at ten: the dog keeps its morning at the top of Church Lane.
    let front = sym(&s, "front");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    idle(&mut s, 2);
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    let top = sym(&s, "church_lane_top");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: top }));
    idle(&mut s, 3);
    assert!(walk_to_unit(&mut s, "dog"), "walk to the dog at the top of Church Lane");
    talk_to(&mut s, "dog");
    let said = conversation(&mut s, &[0, 1]);
    assert_eq!(said.first().map(String::as_str), Some("school_done"), "the dog takes the School back: {said:?}");
    assert!(said.iter().any(|n| n == "the_three"), "and says what the Ball can do: {said:?}");
    assert!(quest_done(&s, "the_school"));
    assert!(quest_active(&s, "the_choice"), "Yours to Say is in her log");
    assert_eq!(holds(&s, "the_ball"), 1);
    assert_eq!(s.view(Seat(0)).unwrap().the_end(), 0, "the story is open");
    s
}

fn closed_by(s: &Sim, ending: u8) {
    assert!(quest_done(s, "the_choice"), "the choice is handed in by the act itself");
    assert_eq!(holds(s, "the_ball"), 0, "the Ball is where she set it down");
    assert_eq!(s.view(Seat(0)).unwrap().the_end(), ending);
}

#[test]
fn the_dog_names_three_ways_and_calls_none_of_them_right() {
    let mut s = to_the_choice();
    // Asked what she would do, it slips once, as it did in the forest, and takes it back.
    talk_to(&mut s, "dog");
    let said = conversation(&mut s, &[0, 0]);
    assert_eq!(said, ["epilogue", "the_three", "the_ask"], "{said:?}");
    let ask = lines_of("dog", "the_ask");
    assert_eq!(ask[0], "I held it.");
    assert!(ask[1].starts_with("She held it."));
    // No line calls one of the three right or wrong.
    for (tree, nodes) in [
        ("dog", &["the_three", "the_ask", "epilogue"][..]),
        ("study_desk", &["ring", "held_1", "held_present", "held_2", "held_3"][..]),
        ("mine_seam", &["ball", "hill_1", "hill_2", "hill_3", "hill_present", "hill_4"][..]),
        ("the_train", &["in", "train_ball", "train_present", "train_1", "train_2", "train_3"][..]),
    ] {
        for n in nodes {
            for l in lines_of(tree, n) {
                let low = l.to_lowercase();
                assert!(!low.contains("the right") && !low.contains("wrong"), "{tree}.{n} judges: {l}");
            }
        }
    }
}

#[test]
fn the_first_ending_is_the_ring_on_the_study_desk() {
    let mut s = to_the_choice();
    let study = sym(&s, "cellar_study");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Cellar, mark: study }));
    idle(&mut s, 3);
    use_prop(&mut s, "study_desk");
    assert_eq!(holds(&s, "birthday_present"), 1, "she came with it");
    let said = conversation(&mut s, &[0]);
    assert_eq!(said, ["ring", "held_1", "held_present", "held_2", "held_3"], "{said:?}");
    closed_by(&s, 1);
    assert_eq!(holds(&s, "birthday_present"), 0, "the present is on the desk beside the Ball");
    assert!(lines_of("study_desk", "held_present").join(" ").contains("It is still for her."));
    assert_eq!(holds(&s, "key_stair"), 0, "her Other Key goes on the nail by the stove");
    let last = lines_of("study_desk", "held_3").join(" ");
    assert!(last.contains("She is not dead, {name},") && last.contains("I would know."));
    assert!(last.contains("hand on its head"));
    let ernest = lines_of("study_desk", "held_2").join(" ");
    assert!(ernest.contains("Ernest Dunn comes down") && ernest.contains("night shift"));
}

/// Without the present in her bag the ending goes straight on: it is only set down if she holds it.
#[test]
fn an_ending_without_the_present_says_nothing_of_it() {
    let mut s = to_the_choice();
    let present = jane_data::catalog().combat.item_id("birthday_present").unwrap();
    jane_sim::bag::bag_remove(&mut s.state_mut().players[0].bag[..], present, 1);
    let study = sym(&s, "cellar_study");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Cellar, mark: study }));
    idle(&mut s, 3);
    use_prop(&mut s, "study_desk");
    let said = conversation(&mut s, &[0]);
    assert_eq!(said, ["ring", "held_1", "held_2", "held_3"], "{said:?}");
    closed_by(&s, 1);
}

#[test]
fn the_second_ending_puts_the_ball_back_in_the_hill_and_the_dog_goes_with_the_night() {
    let mut s = to_the_choice();
    let entry = sym(&s, "entry");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Mine, mark: entry }));
    idle(&mut s, 3);
    // The seam reads the same all game: a hollow the Ball's size, at the back of the vault.
    let look = lines_of("mine_seam", "look").join(" ");
    assert!(look.contains("the size of a fist"));
    use_prop(&mut s, "mine_seam");
    let said = conversation(&mut s, &[0]);
    assert_eq!(said, ["ball", "hill_1", "hill_2", "hill_3", "hill_present", "hill_4"], "{said:?}");
    closed_by(&s, 2);
    assert_eq!(holds(&s, "birthday_present"), 0, "the present is left on the step");
    assert_eq!(flag(&s, "night_gone"), 1);
    let last = lines_of("mine_seam", "hill_4").join(" ");
    assert!(last.contains("She is not dead, {name},") && last.contains("hand on its head"));
    assert!(lines_of("mine_seam", "hill_2").join(" ").contains("one tray fewer"), "Ernest goes with the night");

    // Home by day: the step is empty, now and after.
    let gate = sym(&s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    idle(&mut s, 3);
    assert!(unit(&s, "dog").hidden, "the dog is gone with the night, and was not seen going");
}

#[test]
fn the_third_ending_is_the_sunday_train_and_it_stops_because_she_signals() {
    let mut s = to_the_choice();
    // A Sunday that the train stops on, whatever the seed's omen said of this one.
    s.state_mut().day = 14;
    let through = sym(&s, "omen:train_through");
    cmd(&mut s, Command::Dev(DevOp::Flag { flag: through, value: 0 }));
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 15 }));
    let start = sym(&s, "start");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    idle(&mut s, 3);
    use_prop(&mut s, "station_board");
    let said = conversation(&mut s, &[0]);
    assert_eq!(said, ["signal", "signalled"], "{said:?}");
    assert_eq!(flag(&s, "train_signalled"), 1);
    // The dog comes down to the Halt for the day she means to go.
    let dog = jane_data::catalog().combat.unit(jane_data::catalog().combat.unit_id("dog").unwrap());
    assert!(dog.schedule.iter().any(|r| r.when == Some(jane_data::ScheduleWhen::Flag(nid("train_signalled")))));

    // On the platform at five: the train stops for her, and the door is in front of her.
    let start = sym(&s, "start");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    idle(&mut s, 2);
    s.state_mut().clock = 17 * TICKS_PER_HOUR - 2;
    s.drain_events();
    idle(&mut s, 4);
    let d = s.view(Seat(0)).unwrap().dialogue().expect("the train stops, and the guard is at the door");
    assert_eq!(d.node.map(|n| n.id), Some("in"));
    let said = conversation(&mut s, &[0]);
    assert_eq!(said, ["in", "train_present", "train_1", "train_2", "train_3"], "{said:?}");
    closed_by(&s, 3);
    assert_eq!(holds(&s, "birthday_present"), 0, "the present is on the bench with the Ball");
    assert!(lines_of("the_train", "train_present").join(" ").contains("ONE PARCEL"));
    assert_eq!(holds(&s, "key_stair"), 1, "she takes her Other Key with her, and reads its back");
    assert!(lines_of("the_train", "train_2").join(" ").contains("OR FOR YOU, IF YOU WOULD RATHER NOT"));
    assert!(lines_of("the_train", "train_1").join(" ").contains("She is not dead, {name},"));
}

#[test]
fn a_train_she_steps_back_from_leaves_the_choice_open() {
    let mut s = to_the_choice();
    s.state_mut().day = 14;
    let through = sym(&s, "omen:train_through");
    cmd(&mut s, Command::Dev(DevOp::Flag { flag: through, value: 0 }));
    let signalled = sym(&s, "train_signalled");
    cmd(&mut s, Command::Dev(DevOp::Flag { flag: signalled, value: 1 }));
    let start = sym(&s, "start");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: start }));
    idle(&mut s, 2);
    s.state_mut().clock = 17 * TICKS_PER_HOUR - 2;
    idle(&mut s, 4);
    let said = conversation(&mut s, &[1]);
    assert_eq!(said, ["in"]);
    assert!(quest_active(&s, "the_choice") && holds(&s, "the_ball") == 1);
    assert_eq!(s.view(Seat(0)).unwrap().the_end(), 0);
    // It does not stop for her twice: the request lapsed when she stepped back.
    idle(&mut s, 30);
    assert!(s.view(Seat(0)).unwrap().dialogue().is_none());
}

fn nid(name: &str) -> jane_core::NameId {
    jane_data::catalog().name_id(name).unwrap_or_else(|| panic!("no name {name}"))
}

// --- the town notices ---------------------------------------------------------------------------

/// Kill a named unit as the world records it: its dead flag (the sim writes the same one).
fn dies(s: &mut Sim, unit: &str) {
    let k = FlagKey::Dead(sym(s, unit));
    s.state_mut().flags.insert(k, 1);
    idle(s, 2);
}

/// Let `secs` of real time pass for the rumours, as the tick they compare against.
fn later(s: &mut Sim, secs: u32) {
    let t = s.state().tick.0 + secs * 60;
    s.state_mut().tick = Tick(t);
}

/// Stand in the county at `mark` at `hour`, coming back from the house (nobody watched).
fn in_town(s: &mut Sim, mark: &str, hour: u8) {
    let front = sym(s, "front");
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    idle(s, 2);
    cmd(s, Command::Dev(DevOp::Time { hour }));
    let m = sym(s, mark);
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: m }));
    idle(s, 3);
}

fn says_first(s: &mut Sim, who: &str) -> String {
    assert!(walk_to_unit(s, who), "walk to {who}");
    talk_to(s, who);
    let said = conversation(s, &[1]);
    said.first().cloned().unwrap_or_else(|| panic!("{who} talks"))
}

#[test]
fn the_mine_is_heard_of_at_the_arms_before_the_milk_round_and_never_before_it_happens() {
    let mut s = new_game();
    been_through(&mut s, &SPINE[..5], &OFFERED[..2]);
    in_town(&mut s, "town_square", 10);
    assert_ne!(says_first(&mut s, "mr_cobb"), "news_mine", "nothing has happened yet");
    dies(&mut s, "iron_knuckles");
    let cat = jane_data::catalog();
    let c = cat.living.consequence_id("mine_quiet").unwrap();
    assert!(s.state().consequences_done.get(u32::from(c.0)), "the mine goes quiet when he falls");
    // Straight after, nobody in town knows.
    in_town(&mut s, "town_square", 11);
    assert_ne!(says_first(&mut s, "mr_cobb"), "news_mine", "rumour is slower than she is");
    // Mr Cobb, who sees who comes up the street, first; the milk round last.
    let row = &cat.living.consequences[c.index()];
    let heard = |n: &str| row.spreads.iter().position(|g| g.to.iter().any(|&t| cat.name(t) == n)).expect(n);
    assert!(heard("mr_cobb") < heard("mrs_garland") && heard("mrs_garland") < heard("milkman"));
    later(&mut s, 400);
    in_town(&mut s, "town_square", 12);
    assert_eq!(says_first(&mut s, "mr_cobb"), "news_mine");
    assert_ne!(says_first(&mut s, "mr_cobb"), "news_mine", "said once");
}

#[test]
fn the_bell_stops_when_the_timekeeper_falls_and_the_town_hears_the_silence() {
    let mut s = new_game();
    been_through(&mut s, &SPINE[..10], &OFFERED[..7]);
    given(&mut s, "the_school");
    in_town(&mut s, "town_square", 10);
    dies(&mut s, "ringer");
    assert_eq!(flag(&s, "bell_stopped"), 1);
    // Nine o'clock: no bell, and the night comes all the same.
    s.state_mut().clock = 21 * TICKS_PER_HOUR - 2;
    s.drain_events();
    idle(&mut s, 4);
    let ev = s.drain_events().to_vec();
    assert!(!ev.iter().any(|e| matches!(e.kind, EventKind::Bell { .. })), "no bell is rung at all");
    let toasts: Vec<String> = ev
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Toast(jane_sim::event::ToastKind::Text(jane_core::TextRef::Text(t))) => {
                Some(jane_data::catalog().text(t).to_owned())
            }
            _ => None,
        })
        .collect();
    assert!(toasts.iter().any(|t| t == "Nine o'clock. No bell."), "{toasts:?}");
    assert!(!toasts.iter().any(|t| t.contains("A school bell")), "{toasts:?}");
    assert!(s.view(Seat(0)).unwrap().is_night());
    assert_eq!(flag(&s, "nine_silent"), 1);
    // The next morning Mrs Fenn has sat up with her kitchen clock, and Mrs Tace has gone up to
    // the top of Church Lane to see them come down; the School's lamps are out.
    in_town(&mut s, "town_square", 10);
    assert_eq!(says_first(&mut s, "mrs_fenn"), "news_bell");
    let tace = unit(&s, "mrs_tace");
    let top = s.view(Seat(0)).unwrap().mark(sym(&s, "church_lane_top")).expect("the top of Church Lane");
    let (tx, ty) = tace.pos.cell();
    let near = (tx - i32::from(top.cell.x)).abs() <= 2 && (ty - i32::from(top.cell.y)).abs() <= 2;
    assert!(near || tace.order.is_some(), "Mrs Tace is at the top of Church Lane, or on her way there");
    for lamp in ["school_lamp_a", "school_lamp_b"] {
        let p = s.state().zone(ZoneId::County).unwrap().props.iter().find(|p| p.key == sym(&s, lamp)).cloned();
        assert!(p.is_some_and(|p| !p.on), "{lamp} is out");
    }
}

#[test]
fn each_act_changes_what_the_county_shows_out_of_her_sight() {
    let mut s = new_game();
    let cat = jane_data::catalog();
    // Each spine boss's fall is a consequence with an edit the county shows, and each is written
    // to land where she is not: a death happens inside its own dungeon.
    for (boss, row) in [
        ("iron_knuckles", "mine_quiet"),
        ("attendant", "wing_lit"),
        ("emperor", "forest_quiet"),
        ("foreman", "works_dark"),
        ("goldskin", "burial_quiet"),
        ("ringer", "bell_stopped"),
    ] {
        let c = cat.living.consequence_id(row).unwrap_or_else(|| panic!("no consequence {row}"));
        let r = &cat.living.consequences[c.index()];
        assert_eq!(r.zone, ZoneId::County, "{row} is seen in the county");
        assert!(matches!(r.on, jane_core::action::Condition::Dead(_)), "{row} fires on {boss}'s death");
        dies(&mut s, boss);
        assert!(s.state().consequences_done.get(u32::from(c.0)), "{row} fired");
    }
    in_town(&mut s, "town_square", 10);
    let county = s.state().zone(ZoneId::County).unwrap();
    let find = |k: &str| county.props.iter().find(|p| p.key == sym(&s, k)).cloned();
    assert!(find("museum_wing_lamp").is_some_and(|p| p.on), "the wing's lamp is lit");
    assert!(find("factory_lamp").is_some_and(|p| !p.on), "the Factory is dark");
    for u in ["forest_road_fly_1", "forest_road_fly_2", "graveyard_skeleton_a", "graveyard_skeleton_b"] {
        let gone = county.units.iter().find(|x| x.key == Some(sym(&s, u))).is_none_or(|x| !x.alive || x.hidden);
        assert!(gone, "{u} is gone");
    }
    // The constable sees the Museum's lamp at the lamps, and keeps the east end of the street of
    // an evening from then on.
    assert_eq!(flag(&s, "wing_seen"), 0);
    s.state_mut().clock = 18 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2 - 1;
    idle(&mut s, 2);
    assert_eq!(flag(&s, "wing_seen"), 1);
    let constable = cat.combat.unit(cat.combat.unit_id("town_constable").unwrap());
    let evening = constable.schedule.first().expect("rows");
    assert_eq!(
        (evening.hour_from, evening.hour_to, evening.slot),
        (18, 21, jane_data::ScheduleSlot::Mark(nid("street_east")))
    );
}
