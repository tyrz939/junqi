//! The spine in STORY.md §4's order (QUEST-TREE.md §5), held on the real seed: after the mine
//! the dog offers the Museum, not the Burial, and each hand-in after that offers the next place
//! (the forest, the Factory, the Burial, the School) and then the choice (`endings.rs` plays
//! it); the Burial's stair stays shut until she carries the Chairman's Key from the Factory,
//! whatever else she holds; the walk
//! back to the dog is a walk and never a slog, on three seeds, because the dog meets her nearer
//! for the far acts, and is really there; and every door the story has not sent her to yet is shut
//! to her, with the one way round the night allows.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::search::{Conn, Reach, UNREACHED, flood};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Grid, Key, NameId, ZoneId};
use jane_data::{ScheduleSlot, ScheduleWhen};
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
    // The School handed in, the last thing the dog gives is the choice (STORY.md §10).
    been_through(&mut s, &["the_school"], &[]);
    assert_eq!(the_dog_says(&mut s), "school_done_again");
    talk_through(&mut s, &[0, 1]);
    assert!(quest_active(&s, "the_choice"), "Yours to Say");
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

    // The Chairman's Key from the Factory's locker opens it, and is kept.
    give(&mut s, "key_stone");
    assert!(!try_it(&mut s));
    assert!(!prop(&s, "burial_door").locked);
    assert_eq!(holds(&s, "key_stone"), 1, "a story key is not spent");
    cmd(&mut s, Command::Use);
    idle(&mut s, 3);
    assert_eq!(zone_of(&s), ZoneId::Burial);
}

// --- where the dog takes it back ---------------------------------------------------------------

/// The seeds QUEST-TREE.md's walks are measured on: the two the tests play, and one nobody tuned.
const WALK_SEEDS: [u32; 3] = [3, 2026, 77];
/// The walk back from the place she comes out to whoever takes it in, in steps (cells) on foot:
/// at most a "near" errand (the quest audit's 1500, and a hundred for the Museum's own road),
/// never a slog; at least a stretch of road, never a hand-in at the door she came out of.
const RETURN_MOST: u32 = 1600;
const RETURN_LEAST: u32 = 150;
/// The walk out from wherever the dog gave it, to the furthest door it sends her through: inside
/// the audit's "far".
const OUT_MOST: u32 = 2800;

/// Each spine quest from the mine on: the county doors its steps send her through, the last
/// being the one she comes back out of (the School: its front doors, which its own key opens
/// from outside once she is in).
const SPINE: [(&str, &[&str]); 6] = [
    ("the_mine", &["mine_mouth"]),
    ("the_museum", &["museum_mouth"]),
    ("the_forest", &["library_mouth", "butterfly_forest_mouth"]),
    ("the_factory", &["pipes_mouth", "factory_mouth"]),
    ("the_burial", &["burial_mouth"]),
    ("the_school", &["school_mouth"]),
];

fn nid(name: &str) -> NameId {
    jane_data::catalog().name_id(name).unwrap_or_else(|| panic!("no name {name}"))
}

/// Where the dog takes a quest back: the mark its hours name while the quest is in her log, else
/// its step (WORLD.md §3.6). Read off the row, never typed here.
fn meets(quest: &str) -> (NameId, Option<(u8, u8)>) {
    let cat = jane_data::catalog();
    let q = cat.story.quest_id(quest).unwrap();
    let dog = cat.combat.unit(cat.combat.unit_id("dog").unwrap());
    for r in dog.schedule {
        if let (Some(ScheduleWhen::While(w)), ScheduleSlot::Mark(n)) = (r.when, r.slot) {
            if w == q {
                return (n, Some((r.hour_from, r.hour_to)));
            }
        }
    }
    let step = dog.schedule.iter().find(|r| r.when.is_none()).expect("a plain row");
    let ScheduleSlot::Mark(n) = step.slot else { panic!("the dog's day is a mark") };
    (n, None)
}

/// Cells nobody can stand on: solid ground and every solid prop's footprint.
fn blocked(bp: &Blueprint) -> Grid<bool> {
    let cat = jane_data::catalog();
    let mut g = Grid::new(bp.w(), bp.h(), false);
    for (i, t) in bp.tiles.as_slice().iter().enumerate() {
        g.as_mut_slice()[i] = t.flags() & F_SOLID != 0;
    }
    for p in &bp.props {
        let row = cat.story.prop(p.def);
        if row.solid {
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            g.fill_rect(jane_core::Rect::new(x, y, i32::from(row.w), i32::from(row.h)), true);
        }
    }
    g
}

fn mark_at(bp: &Blueprint, n: NameId) -> (i32, i32) {
    let m = bp.marks.get(&Key::Name(n)).unwrap_or_else(|| panic!("no mark {}", jane_data::catalog().name(n)));
    (i32::from(m.cell.x), i32::from(m.cell.y))
}

/// Steps on foot from `from` to within a cell of `to`.
fn walk(bp: &Blueprint, solid: &Grid<bool>, from: (i32, i32), to: (i32, i32)) -> u32 {
    let mut reach = Reach::new();
    flood(bp.w(), bp.h(), &[from], Conn::Four, u32::MAX, |x, y| !solid.read(x, y, true), &mut reach);
    let mut best = UNREACHED;
    for dy in -1..=1 {
        for dx in -1..=1 {
            best = best.min(reach.dist(to.0 + dx, to.1 + dy));
        }
    }
    best
}

#[test]
fn the_walk_back_to_the_dog_is_a_walk_and_never_a_slog() {
    let cat = jane_data::catalog();
    let mut rows = vec![format!("{:>5} {:<12} {:<16} {:>6} {:>6}", "seed", "quest", "taken back at", "out", "back")];
    let mut bad = Vec::new();
    for seed in WALK_SEEDS {
        let bp = jane_sim::blueprints::build_one(ZoneId::County, seed).expect("the county builds");
        let solid = blocked(&bp);
        // The first is offered on the step, after the rats; each after it where the last was taken.
        let mut offered = meets("rats_below").0;
        for (quest, doors) in SPINE {
            let (back, _) = meets(quest);
            let out =
                doors.iter().map(|d| walk(&bp, &solid, mark_at(&bp, offered), mark_at(&bp, nid(d)))).max().unwrap();
            let last = mark_at(&bp, nid(doors[doors.len() - 1]));
            let home = walk(&bp, &solid, last, mark_at(&bp, back));
            rows.push(format!("{seed:>5} {quest:<12} {:<16} {out:>6} {home:>6}", cat.name(back)));
            if out > OUT_MOST {
                bad.push(format!("seed {seed} {quest}: {out} steps out, over {OUT_MOST}"));
            }
            if !(RETURN_LEAST..=RETURN_MOST).contains(&home) {
                bad.push(format!("seed {seed} {quest}: {home} steps back, not {RETURN_LEAST} to {RETURN_MOST}"));
            }
            offered = back;
        }
    }
    println!("{}", rows.join("\n"));
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Mark quests given, as the dog would have given them.
fn given(s: &mut Sim, quest: &str) {
    let q = jane_data::catalog().story.quest_id(quest).unwrap();
    cmd(s, Command::Dev(DevOp::Quest(q)));
}

/// Away into the house and back at `hour`: nobody watched the county while she was gone, so the
/// dog is wherever its hours put it (presence on arrival).
fn back_at(s: &mut Sim, hour: u8) {
    let front = sym(s, "front");
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark: front }));
    idle(s, 2);
    cmd(s, Command::Dev(DevOp::Time { hour }));
    let gate = sym(s, "yard_gate");
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    idle(s, 3);
}

/// Is the dog within two cells of this mark, and in the world?
fn dog_at(s: &Sim, mark: NameId) -> bool {
    let d = unit(s, "dog");
    let (mx, my) = mark_at(s.blueprints().get(ZoneId::County), mark);
    let (x, y) = d.pos.cell();
    !d.hidden && (x - mx).abs() <= 2 && (y - my).abs() <= 2
}

#[test]
fn the_dog_is_where_the_log_says_and_the_log_says_when() {
    let mut s = new_game();
    let cat = jane_data::catalog();
    been_through(&mut s, &["the_letter", "defeat_skeleton"], &[]);
    let step = meets("the_mine").0;
    back_at(&mut s, 10);
    assert!(dog_at(&s, step), "by day on the step");
    back_at(&mut s, 22);
    assert!(unit(&s, "dog").hidden, "after the bell, nowhere");

    // Each meeting: the returnTo and the offer say where and when, the hours say the same, and
    // the dog is there in them and on the step outside them (never both).
    for (quest, place, when) in [
        ("the_forest", "Museum's steps", "from four till the bell"),
        ("the_factory", "graveyard gate", "from six till the bell"),
        ("the_burial", "graveyard gate", "from six till the bell"),
        ("the_school", "top of Church Lane", "from six till noon"),
    ] {
        let (mark, hours) = meets(quest);
        let (from, to) = hours.expect("a meeting has hours");
        let q = cat.story.quest(cat.story.quest_id(quest).unwrap());
        let back = cat.text(q.return_to);
        assert!(back.contains(place) && back.contains(when), "{quest}: \"{back}\"");
        let said = match (from, to) {
            (16, 21) => "four till the bell",
            (18, 21) => "six till the bell",
            (6, 12) => "noon",
            _ => panic!("{quest}: hours {from} to {to} that no line says"),
        };
        assert!(back.contains(said), "{quest}: the hours {from} to {to} and \"{back}\"");
        given(&mut s, quest);
        back_at(&mut s, from);
        assert!(dog_at(&s, mark), "{quest}: at {from}:00 at {}", cat.name(mark));
        back_at(&mut s, to - 1);
        assert!(dog_at(&s, mark), "{quest}: at {}:00 still there", to - 1);
        let outside = if from == 6 { 14 } else { from - 3 };
        back_at(&mut s, outside);
        assert!(dog_at(&s, step) && !dog_at(&s, mark), "{quest}: at {outside}:00 on the step, not both");
        been_through(&mut s, &[quest], &[]);
        back_at(&mut s, from);
        assert!(dog_at(&s, step), "{quest} handed in: home again");
    }
}

// --- the doors the story has not opened yet ------------------------------------------------------

fn give(s: &mut Sim, item: &str) {
    let id = jane_data::catalog().combat.item_id(item).unwrap();
    cmd(s, Command::Dev(DevOp::Give { item: id, qty: 1 }));
}

fn take_all(s: &mut Sim, item: &str) {
    let id = jane_data::catalog().combat.item_id(item).unwrap();
    for bag in s.state_mut().players[0].bag.iter_mut() {
        if bag.is_some_and(|b| b.item == id) {
            *bag = None;
        }
    }
}

/// Stand at a county door's mark, press USE on it, and say what it did: shut (locked or night
/// locked, the toast) or where it took her.
fn try_door(s: &mut Sim, mark: &str, door: &str) -> Result<ZoneId, ToastKind> {
    let at = sym(s, mark);
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: at }));
    idle(s, 3);
    assert!(walk_to_prop(s, door), "walk to {door}");
    s.drain_events();
    cmd(s, Command::Use);
    let shut = s.drain_events().iter().find_map(|e| match e.kind {
        EventKind::Toast(t @ (ToastKind::Locked { .. } | ToastKind::NightLock(_))) => Some(t),
        _ => None,
    });
    if let Some(t) = shut {
        return Err(t);
    }
    // A key turned: the door is open now; through it.
    if zone_of(s) == ZoneId::County {
        cmd(s, Command::Use);
    }
    idle(s, 3);
    let z = zone_of(s);
    let back = sym(s, "yard_gate");
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: back }));
    idle(s, 3);
    Ok(z)
}

#[test]
fn the_library_opens_to_the_green_tag_and_the_works_to_the_watchman_or_the_night() {
    let mut s = new_game();
    // The library is the council's, and its key is on the Museum's green tag.
    assert!(matches!(try_door(&mut s, "library_mouth", "library_door"), Err(ToastKind::Locked { .. })));
    give(&mut s, "key_forest");
    assert_eq!(try_door(&mut s, "library_mouth", "library_door"), Ok(ZoneId::Library));

    // The Works' wicket: by day only the Night Watchman's Key opens it; at nine it is opened for the
    // night shift, and anyone may walk in with them (the way round, at the county's worst hour).
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    assert!(matches!(try_door(&mut s, "factory_mouth", "factory_door"), Err(ToastKind::NightLock(_))));
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 22 }));
    assert_eq!(try_door(&mut s, "factory_mouth", "factory_door"), Ok(ZoneId::Factory), "open for the night shift");
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    give(&mut s, "key_works");
    assert_eq!(try_door(&mut s, "factory_mouth", "factory_door"), Ok(ZoneId::Factory), "the watchman's key by day");

    // The Company's grate to the pipes: the same key, and no way round.
    take_all(&mut s, "key_works");
    assert!(matches!(try_door(&mut s, "pipes_mouth", "pipes_grate"), Err(ToastKind::Locked { .. })));
    give(&mut s, "key_works");
    assert_eq!(try_door(&mut s, "pipes_mouth", "pipes_grate"), Ok(ZoneId::Pipes));
    assert_eq!(holds(&s, "key_works"), 1, "a story key is not spent");
}

#[test]
fn the_stair_up_to_the_school_is_julies_to_open_and_the_dog_gives_her_key_with_the_school() {
    let cat = jane_data::catalog();
    let s = new_game();
    let stair = sym(&s, "stair_up");
    let bp = s.blueprints().get(ZoneId::Burial);
    let p = bp.props.iter().find(|p| p.key == Key::Name(NameId(stair.0 as u16))).expect("the stair up");
    assert!(p.locked, "the stair up is locked when he is down, too");
    assert_eq!(p.key_tag, Some(Key::Name(nid("school_stair"))));
    let key = cat.combat.item(cat.combat.item_id("key_stair").unwrap());
    assert_eq!(key.opens, Some(nid("school_stair")));
    assert!(key.bound, "a key that gates is a bound story key");
    // Every key the spine gates with is bound, and there is exactly one of each to be had.
    for k in ["key_forest", "key_works", "key_stone", "key_stair"] {
        assert!(cat.combat.item(cat.combat.item_id(k).unwrap()).opens.is_some(), "{k} opens something");
    }
    for k in ["key_works", "key_stone", "key_stair"] {
        assert!(cat.combat.item(cat.combat.item_id(k).unwrap()).bound, "{k} is bound");
    }

    // The dog hands it over with the School, not before.
    let mut s = new_game();
    let gate = sym(&s, "yard_gate");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::County, mark: gate }));
    let spine = ["the_letter", "defeat_skeleton", "see_the_kitchen", "stock_the_bench", "rats_below", "the_mine"];
    been_through(&mut s, &spine, &["offered_rats", "offered_mine", "offered_museum", "offered_forest"]);
    been_through(&mut s, &["the_museum", "the_forest", "the_factory"], &["offered_factory", "offered_burial"]);
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    assert_eq!(holds(&s, "key_stair"), 0);
    been_through(&mut s, &["the_burial"], &[]);
    assert_eq!(the_dog_says(&mut s), "offer_school");
    talk_through(&mut s, &[0]);
    assert!(quest_active(&s, "the_school"));
    assert_eq!(holds(&s, "key_stair"), 1, "her other key, off the dog");
}
