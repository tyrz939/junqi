//! USE and the world verbs on a room of known geometry, and a few on the real seed. Carries the
//! non-combat parts of `jane/test/sim.test.ts` (bags and crafting), `verbs2.test.ts` (E1 `if`,
//! E9 `send`, E12 nothing solid lands on a unit, E13 `reveal`, the row's own prompt),
//! `dungeon-verbs.test.ts` (E4 the sill, E3 a `to` into the same zone), `rest.test.ts` (beds
//! and fires, the night-locked door) and ENGINE.md's interact rules: loot, keys and gates,
//! push and pull, carry, things under things, plates and triggers, the journal.

mod common;

use jane_core::action::{Facing, FlagOp, FlagTest};
use jane_core::blueprint::TriggerMode;
use jane_core::{Action, Angle, Cond, Condition, FlagKey, Rect, Stack, TextRef, Tile, ZoneId};
use jane_sim::event::{EventKind, PropChange, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::interact::{FocusRef, Verb};
use jane_sim::state::{Dialogue, FactKey, Source, Speaker};
use jane_sim::{Command, Seat, Sim};

use common::bot::*;
use common::room::{Room, item};

fn flag_named(s: &Sim, name: &str) -> i32 {
    let k = jane_sim::state::FlagKey::Named(sym(s, name));
    s.state().flags.get(&k).copied().unwrap_or(0)
}

fn toasts(ev: &[jane_sim::Event]) -> Vec<ToastKind> {
    ev.iter()
        .filter_map(|e| match e.kind {
            EventKind::Toast(t) => Some(t),
            _ => None,
        })
        .collect()
}

// --- loot ----------------------------------------------------------------------------------

#[test]
fn a_chest_empties_into_her_bag_and_keeps_what_does_not_fit() {
    let mut r = Room::new(false);
    r.chest("apples", 10, 15, &[("apple", 3)]);
    r.chest("bars", 10, 20, &[("gold_bar", 16), ("apple", 2)]);
    let mut s = r.build();
    place(&mut s, 9, 16, Facing::East);
    let apples = prop_id(&s, "apples");
    assert_eq!(
        s.view(Seat(0)).unwrap().focus().map(|f| (f.target, f.verb)),
        Some((FocusRef::Prop(apples), Verb::Open))
    );
    let before = holds(&s, "apple");
    events(&mut s);
    cmd(&mut s, Command::Use);
    assert_eq!(holds(&s, "apple"), before + 3);
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::Loot { item: item("apple"), qty: 3 } && e.to == Some(Seat(0))));
    assert!(ev.iter().any(|e| e.kind == EventKind::Prop { prop: apples, change: PropChange::Open }));
    assert!(prop(&s, "apples").used);
    assert!(s.view(Seat(0)).unwrap().focus().is_none(), "an empty chest is nothing to do");
    // Held: the journal knows it.
    let held = FactKey::Thing(jane_core::action::Thing::Item(item("apple")));
    assert_eq!(s.view(Seat(0)).unwrap().known(held).map(|k| k.how), Some(Source::Held));

    // A bag with room for nothing: the bars stay in the chest, and so does the chest's use.
    for _ in 0..24 {
        cmd(&mut s, Command::Dev(DevOp::Give { item: item("rock"), qty: 16 }));
    }
    let apple_room = 16 - holds(&s, "apple") % 16;
    place(&mut s, 9, 21, Facing::East);
    events(&mut s);
    cmd(&mut s, Command::Use);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::InventoryFull));
    let p = prop(&s, "bars");
    assert!(!p.used, "the chest keeps what did not fit");
    assert_eq!(holds(&s, "gold_bar"), 0);
    assert_eq!(holds(&s, "apple") % 16, (16 - apple_room + 2.min(apple_room)) % 16);
    assert_eq!(
        s.view(Seat(0)).unwrap().focus().map(|f| f.verb),
        Some(Verb::Open),
        "what is left is still there to be taken"
    );
}

#[test]
fn a_row_with_its_own_prompt_says_it_and_a_herb_is_gone_once_gathered() {
    let mut r = Room::new(false);
    r.prop("sprig", "herb", 10, 16, |s| s.loot = vec![Stack { item: item("pansy"), qty: 1 }]);
    let mut s = r.build();
    place(&mut s, 9, 16, Facing::East);
    let f = s.view(Seat(0)).unwrap().focus().unwrap();
    let gather = jane_data::catalog().story.prop(prop(&s, "sprig").def).prompt.unwrap();
    assert_eq!(f.verb, Verb::Custom(gather), "the row knows best what it is: a herb is gathered");
    cmd(&mut s, Command::Use);
    assert_eq!(holds(&s, "pansy"), 1);
    assert!(prop(&s, "sprig").hidden, "gathered things vanish once looted");
}

// --- keys, doors, gates ----------------------------------------------------------------------

#[test]
fn a_lock_takes_the_key_that_fits_and_a_bound_key_is_kept() {
    let mut r = Room::new(false);
    let tag = r.key("generic");
    r.prop("gate", "gate_h", 12, 15, |s| {
        s.locked = true;
        s.key_tag = Some(tag);
    });
    r.prop("trunk", "chest", 12, 20, |s| {
        s.locked = true;
        s.key_tag = Some(tag);
        s.loot = vec![Stack { item: item("apple"), qty: 1 }];
    });
    let mut s = r.build();
    place(&mut s, 13, 17, Facing::North);
    let gate = prop_id(&s, "gate");
    assert!(prop(&s, "gate").solid, "a gate is shut while locked");
    events(&mut s);
    cmd(&mut s, Command::Use);
    let ev = events(&mut s);
    assert!(toasts(&ev).contains(&ToastKind::Locked { prop: gate }));
    assert!(prop(&s, "gate").locked);

    cmd(&mut s, Command::Dev(DevOp::Give { item: item("key_generic"), qty: 2 }));
    events(&mut s);
    cmd(&mut s, Command::Use);
    let ev = events(&mut s);
    assert!(toasts(&ev).contains(&ToastKind::UnlockedWith(item("key_generic"))));
    let g = prop(&s, "gate");
    assert!(!g.locked && !g.solid, "an unlocked gate is open");
    assert_eq!(holds(&s, "key_generic"), 1, "an ordinary key is used up");
    idle(&mut s, 1);
    let rt = s.runtime(ZoneId::County).unwrap();
    assert!(!rt.grid.solid(13, 15), "and the grid knows it");

    // Julie's key is bound: it opens her door and stays in the bag. Here: the trunk's tag is the
    // generic one, and the last generic key goes.
    place(&mut s, 13, 19, Facing::South);
    cmd(&mut s, Command::Use);
    assert!(!prop(&s, "trunk").locked);
    assert_eq!(holds(&s, "key_generic"), 0);
    assert!(jane_data::catalog().combat.item(item("key_auntie_house")).bound);
}

#[test]
fn a_door_into_this_zone_is_a_hop_and_full_arms_refuse_it() {
    let mut r = Room::new(false);
    r.mark("far_side", 40, 8);
    r.door("vent", 10, 15, ZoneId::County, "far_side");
    r.prop("stone", "rock", 8, 20, |_| {});
    let front = r.key("front");
    r.trigger(
        "stairs",
        Rect::new(30, 20, 2, 2),
        TriggerMode::Enter,
        false,
        vec![Action::Travel { zone: ZoneId::House, mark: front }],
    );
    let mut s = r.build();
    place(&mut s, 9, 17, Facing::East);
    let door = jane_data::catalog().story.prop(prop(&s, "vent").def);
    let enter = door.prompt.map_or(Verb::Enter, Verb::Custom);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(enter));
    cmd(&mut s, Command::Use);
    assert_eq!(me(&s).pos.cell(), (40, 8), "moved to the mark, nothing reloaded");
    assert_eq!(zone_of(&s), ZoneId::County);

    // With something in her arms, USE puts it down, and a way out is refused.
    place(&mut s, 8, 19, Facing::South);
    cmd(&mut s, Command::Use);
    assert!(me(&s).carrying.is_some());
    place(&mut s, 9, 17, Facing::East);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::PutDown));
    place(&mut s, 30, 20, Facing::East);
    events(&mut s);
    idle(&mut s, 2);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::PutDownFirst));
    assert_eq!(zone_of(&s), ZoneId::County);
}

#[test]
fn a_door_marked_for_it_is_not_answered_after_dark() {
    // rest.test.ts "a door marked for it is not answered after dark, and is an ordinary door by day".
    let mut r = Room::new(false);
    let says = r.bp.push_text("Nobody comes to the door after dark.".into());
    r.door("shop", 10, 15, ZoneId::House, "front");
    r.bp.props.last_mut().unwrap().night_lock = Some(says);
    let mut s = r.build();
    place(&mut s, 9, 17, Facing::East);
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 22 }));
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::TryTheDoor));
    events(&mut s);
    cmd(&mut s, Command::Use);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::NightLock(says)));
    idle(&mut s, 1);
    assert_eq!(zone_of(&s), ZoneId::County);
    cmd(&mut s, Command::Dev(DevOp::Time { hour: 10 }));
    let door = jane_data::catalog().story.prop(prop(&s, "shop").def);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(door.prompt.map_or(Verb::Enter, Verb::Custom)));
    cmd(&mut s, Command::Use);
    idle(&mut s, 1);
    assert_eq!(zone_of(&s), ZoneId::House);
}

// --- push, pull, carry, under ----------------------------------------------------------------

#[test]
fn hold_to_push_moves_a_barrel_a_cell_and_walking_away_pulls_it_back() {
    let mut r = Room::new(false);
    r.prop("barrel", "barrel", 20, 16, |_| {});
    let mut s = r.build();
    place(&mut s, 19, 16, Facing::East);
    let f = s.view(Seat(0)).unwrap().focus().unwrap();
    assert_eq!(f.verb, Verb::HoldToPush, "nothing to open, but it moves: the prompt says so");
    events(&mut s);
    cmd(&mut s, Command::Use);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::ItShifts));

    // Leaning into it: braced, not walking, for 29 ticks; the 30th moves it and her.
    let x0 = me(&s).pos.x;
    hold(&mut s, Angle::EAST, 29);
    assert_eq!(prop(&s, "barrel").cell.x, 20);
    assert_eq!(me(&s).pos.x, x0, "braced against it, she does not walk");
    let e0 = me(&s).energy;
    hold(&mut s, Angle::EAST, 1);
    assert_eq!(prop(&s, "barrel").cell.x, 21);
    assert!(me(&s).energy < e0, "a push costs energy");
    // She follows it up (the step after a push meets the old footprint until housekeeping, as
    // in the TS) and leans on it again.
    hold(&mut s, Angle::EAST, 6);
    assert_eq!(me(&s).pos.cell(), (20, 16));
    assert!(me(&s).hold > 0, "braced again");

    // Pull: hold USE and walk away; she steps back a cell and it follows.
    hold(&mut s, Angle::WEST, 30);
    assert_eq!(prop(&s, "barrel").cell.x, 20);
    assert_eq!(me(&s).pos.cell(), (19, 16));
    let rt = s.runtime(ZoneId::County).unwrap();
    assert!(rt.grid.solid(20, 16) && rt.grid.solid(21, 17) && !rt.grid.solid(22, 16));
}

#[test]
fn a_pushed_barrel_stops_at_a_sill_that_feet_walk_over() {
    // dungeon-verbs.test.ts E4: the sill.
    let mut r = Room::new(false);
    r.bp.tiles.fill_rect(Rect::new(22, 10, 1, 12), Tile::Sill);
    r.prop("barrel", "barrel", 20, 16, |_| {});
    let mut s = r.build();
    place(&mut s, 19, 16, Facing::East);
    hold(&mut s, Angle::EAST, 30);
    assert_eq!(prop(&s, "barrel").cell.x, 20, "its footprint would cover the sill");
    place(&mut s, 21, 12, Facing::East);
    walk_to(&mut s, jane_core::Vec2::centre(24, 12), jane_core::Fx::from_px(1));
    assert_eq!(me(&s).pos.cell(), (24, 12), "feet walk straight over");
}

#[test]
fn a_thing_lifted_is_put_down_ahead_and_what_lay_under_it_shows() {
    let mut r = Room::new(false);
    let key = r.prop("tin", "herb", 10, 20, |s| {
        s.hidden = true;
        s.loot = vec![Stack { item: item("apple"), qty: 1 }];
    });
    r.prop("stone", "rock", 10, 20, |s| s.under = Some(key));
    let mut s = r.build();
    place(&mut s, 10, 19, Facing::South);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::PickUp));
    cmd(&mut s, Command::Use);
    let stone = prop_id(&s, "stone");
    assert_eq!(me(&s).carrying, Some(stone));
    assert!(!prop(&s, "stone").solid);
    assert!(prop(&s, "tin").hidden, "lifted, not moved: it still covers the tin");
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::PutDown));
    // Carrying spends energy.
    let e = me(&s).energy;
    idle(&mut s, 10);
    assert!(me(&s).energy < e);
    // Put down east of her feet.
    place(&mut s, 12, 19, Facing::East);
    events(&mut s);
    cmd(&mut s, Command::Use);
    let p = prop(&s, "stone");
    assert_eq!((p.cell.x, p.cell.y), (13, 19));
    assert!(p.solid && me(&s).carrying.is_none());
    let ev = events(&mut s);
    let tin = prop_id(&s, "tin");
    assert!(!prop(&s, "tin").hidden, "off the spot: the tin is there");
    assert!(toasts(&ev).contains(&ToastKind::Under { top: stone, found: tin }));
}

#[test]
fn under_when_waits_for_its_condition_and_a_quest_given_finds_it_later() {
    let mut r = Room::new(false);
    let key = r.prop("tin", "herb", 10, 20, |s| s.hidden = true);
    let told = r.key("told_about_tin");
    let when = r.bp.push_conds(vec![Cond {
        not: false,
        c: Condition::Flag { key: FlagKey::Named(told), test: FlagTest::NonZero },
    }]);
    r.prop("crate", "crate", 10, 20, |s| {
        s.under = Some(key);
        s.under_when = Some(when);
    });
    let mut s = r.build();
    place(&mut s, 12, 21, Facing::West);
    hold(&mut s, Angle::WEST, 30);
    assert_eq!(prop(&s, "crate").cell.x, 9, "pushed west one cell");
    assert!(prop(&s, "tin").hidden, "a cell of it still covers the tin");
    hold(&mut s, Angle::WEST, 45);
    let c = prop(&s, "crate");
    assert_eq!(c.cell.x, 8, "clear of the tin");
    assert!(prop(&s, "tin").hidden, "nothing is there yet: she has not been told");
    // Told, and then a quest given: it is found lying there.
    let told = sym(&s, "told_about_tin");
    cmd(&mut s, Command::Dev(DevOp::Flag { flag: told, value: 1 }));
    cmd(&mut s, Command::Dev(DevOp::Quest(jane_data::catalog().story.quest_id("defeat_skeleton").unwrap())));
    assert!(!prop(&s, "tin").hidden);
}

// --- plates and triggers ---------------------------------------------------------------------

#[test]
fn a_plate_holds_a_gate_open_while_a_barrel_stands_on_it() {
    let mut r = Room::new(false);
    let gate = r.prop("gate", "gate_h", 30, 8, |s| s.locked = true);
    let open = r.list(vec![Action::Unlock(gate)]);
    let shut = r.list(vec![Action::Lock(gate)]);
    r.prop("plate", "plate", 22, 16, |s| {
        s.use_list = Some(open);
        s.release = Some(shut);
    });
    r.prop("barrel", "barrel", 20, 16, |_| {});
    let mut s = r.build();
    // She stands on it: pressed.
    place(&mut s, 23, 17, Facing::East);
    idle(&mut s, 6);
    assert!(prop(&s, "plate").on && !prop(&s, "gate").locked);
    place(&mut s, 12, 12, Facing::East);
    idle(&mut s, 6);
    assert!(!prop(&s, "plate").on && prop(&s, "gate").locked, "released, it drops again");
    // A barrel pushed onto it holds it down, with nobody near.
    place(&mut s, 19, 16, Facing::East);
    hold(&mut s, Angle::EAST, 30);
    hold(&mut s, Angle::EAST, 45);
    assert_eq!(prop(&s, "barrel").cell.x, 22);
    place(&mut s, 5, 5, Facing::East);
    idle(&mut s, 12);
    assert!(prop(&s, "plate").on && !prop(&s, "gate").locked);
}

#[test]
fn enter_fires_on_walking_in_once_and_while_waits_for_its_conditions() {
    let mut r = Room::new(false);
    let count = r.key("count");
    r.trigger(
        "door_mat",
        Rect::new(20, 14, 4, 4),
        TriggerMode::Enter,
        false,
        vec![Action::Flag { key: FlagKey::Named(count), op: FlagOp::Add(1) }],
    );
    let once = r.key("once");
    r.trigger(
        "hall",
        Rect::new(30, 14, 4, 4),
        TriggerMode::Enter,
        true,
        vec![Action::Flag { key: FlagKey::Named(once), op: FlagOp::Add(1) }],
    );
    let lever = r.key("lever");
    let done = r.key("done");
    let rect = r.rect("yard", Rect::new(1, 1, 46, 30));
    let when = r.bp.push_conds(vec![Cond {
        not: false,
        c: Condition::Flag { key: FlagKey::Named(lever), test: FlagTest::NonZero },
    }]);
    let actions = r.list(vec![Action::Flag { key: FlagKey::Named(done), op: FlagOp::Add(1) }]);
    r.bp.triggers.insert(
        rect,
        jane_core::blueprint::Trigger {
            rect,
            mode: TriggerMode::While,
            once: true,
            when: Some(when),
            actions,
            reset: None,
        },
    );
    let mut s = r.build();
    for _ in 0..2 {
        place(&mut s, 21, 15, Facing::East);
        idle(&mut s, 3);
        place(&mut s, 31, 15, Facing::East);
        idle(&mut s, 3);
        place(&mut s, 10, 10, Facing::East);
        idle(&mut s, 3);
    }
    assert_eq!(flag_named(&s, "count"), 2, "each walk in");
    assert_eq!(flag_named(&s, "once"), 1);
    assert_eq!(flag_named(&s, "done"), 0);
    let lever = sym(&s, "lever");
    cmd(&mut s, Command::Dev(DevOp::Flag { flag: lever, value: 1 }));
    idle(&mut s, 2);
    assert_eq!(flag_named(&s, "done"), 1, "inside and the condition holds");
    idle(&mut s, 5);
    assert_eq!(flag_named(&s, "done"), 1, "once");
}

// --- if, knows, heard, the journal -----------------------------------------------------------

#[test]
fn if_runs_then_or_else_for_whoever_is_acting_and_the_journal_is_a_condition() {
    // verbs2.test.ts E1, with the journal's conditions (§3.7).
    let mut r = Room::new(false);
    let well = r.key("stoop");
    let seen = r.key("seen");
    let yes = r.list(vec![Action::Flag { key: FlagKey::Named(seen), op: FlagOp::Set(1) }]);
    let no = r.list(vec![Action::Flag { key: FlagKey::Named(seen), op: FlagOp::Set(2) }]);
    let when = r.bp.push_conds(vec![Cond { not: false, c: Condition::Knows(jane_core::action::FactKey::Place(well)) }]);
    let has = r.bp.push_conds(vec![Cond { not: false, c: Condition::HasItem(Stack { item: item("apple"), qty: 3 }) }]);
    let apples = r.key("apples");
    let a_yes = r.list(vec![Action::Flag { key: FlagKey::Named(apples), op: FlagOp::Set(1) }]);
    let a_no = r.list(vec![Action::Flag { key: FlagKey::Named(apples), op: FlagOp::Set(2) }]);
    let lever = r.list(vec![
        Action::If { when, then: yes, els: Some(no) },
        Action::If { when: has, then: a_yes, els: Some(a_no) },
    ]);
    r.prop("lever", "lever", 10, 16, |s| s.use_list = Some(lever));
    let visit = vec![Action::Location(well)];
    r.trigger("mat", Rect::new(20, 20, 2, 2), TriggerMode::Enter, true, visit);
    let mut s = r.build();
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    assert_eq!(flag_named(&s, "seen"), 2, "not known yet: else");
    assert_eq!(flag_named(&s, "apples"), 1, "hers: the start kit's three apples");
    place(&mut s, 20, 20, Facing::East);
    idle(&mut s, 2);
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    assert_eq!(flag_named(&s, "seen"), 1, "known: then");
}

#[test]
fn the_journal_upgrades_by_source_keeps_its_first_tick_and_speaks_only_on_change() {
    let mut r = Room::new(true);
    let place_k = r.key("stoop");
    let text = jane_core::TextId(3);
    let read = r.list(vec![Action::Read(TextRef::Text(text))]);
    r.prop("sign", "sign", 10, 16, |s| s.use_list = Some(read));
    r.trigger("mat", Rect::new(20, 20, 2, 2), TriggerMode::Enter, false, vec![Action::Location(place_k)]);
    let mut s = r.build();
    let stoop = FactKey::Place(sym(&s, "stoop"));
    let journal_events =
        |s: &mut Sim| s.drain_events().iter().filter(|e| matches!(e.kind, EventKind::Journal(_))).count();
    journal_events(&mut s);
    place(&mut s, 20, 20, Facing::East);
    idle(&mut s, 2);
    assert_eq!(journal_events(&mut s), 1, "a new fact");
    let first = s.view(Seat(0)).unwrap().known(stoop).unwrap();
    assert_eq!(first.how, Source::Visited);
    // Again: nothing new, nothing said.
    place(&mut s, 10, 10, Facing::East);
    idle(&mut s, 2);
    place(&mut s, 20, 20, Facing::East);
    idle(&mut s, 2);
    assert_eq!(journal_events(&mut s), 0, "a repeat is not news");
    // A weaker source changes nothing; a stronger one upgrades and keeps the first tick.
    let n = s.state().journal.entries.len();
    {
        let st = s.state_mut();
        let tick = st.tick;
        let mut ev = Vec::new();
        assert!(!jane_sim::journal::record(
            st,
            &mut ev,
            stoop,
            Source::Seen,
            ZoneId::County,
            jane_core::Cell::new(0, 0)
        ));
        assert!(ev.is_empty());
        st.tick = jane_core::Tick(tick.0 + 100);
        assert!(jane_sim::journal::record(
            st,
            &mut ev,
            stoop,
            Source::Named,
            ZoneId::County,
            jane_core::Cell::new(0, 0)
        ));
        assert_eq!(ev.len(), 1);
    }
    let k = s.view(Seat(0)).unwrap().known(stoop).unwrap();
    assert_eq!((k.how, k.since), (Source::Named, first.since));
    assert_eq!(s.state().journal.entries.len(), n + 1);
    // A claim read; a Confirmed claim is final.
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    assert!(s.view(Seat(0)).unwrap().dialogue().is_some_and(|d| d.read == Some(TextRef::Text(text))));
    let claim = FactKey::Claim(text);
    assert_eq!(s.view(Seat(0)).unwrap().known(claim).map(|k| k.how), Some(Source::Read));
    let st = s.state_mut();
    let mut ev = Vec::new();
    assert!(jane_sim::journal::record(
        st,
        &mut ev,
        claim,
        Source::Contradicted,
        ZoneId::County,
        jane_core::Cell::new(0, 0)
    ));
    assert!(!jane_sim::journal::record(
        st,
        &mut ev,
        claim,
        Source::Confirmed,
        ZoneId::County,
        jane_core::Cell::new(0, 0)
    ));
    assert!(!jane_sim::journal::record(st, &mut ev, claim, Source::Told, ZoneId::County, jane_core::Cell::new(0, 0)));
    assert!(jane_sim::journal::heard(st, text));
    // The ring is per kind and bounded; the view reads it oldest first.
    let kinds: Vec<_> = s.view(Seat(0)).unwrap().journal().map(jane_sim::state::JournalEntry::kind).collect();
    assert!(
        kinds.contains(&jane_sim::state::JournalKind::Place) && kinds.contains(&jane_sim::state::JournalKind::Claim)
    );
}

#[test]
fn the_journal_ring_drops_the_oldest_of_a_kind_only() {
    let mut s = common::new_game();
    let st = s.state_mut();
    let mut ev = Vec::new();
    let at = jane_core::Cell::new(0, 0);
    jane_sim::journal::record(st, &mut ev, FactKey::Claim(jane_core::TextId(0)), Source::Told, ZoneId::County, at);
    for i in 0..jane_sim::tuning::journal_ring() + 5 {
        let fact = FactKey::Person(jane_core::Sym(10_000 + i));
        jane_sim::journal::record(st, &mut ev, fact, Source::Met, ZoneId::County, at);
    }
    let people = st.journal.entries.iter().filter(|e| e.kind() == jane_sim::state::JournalKind::Person).count();
    assert_eq!(people as u32, jane_sim::tuning::journal_ring());
    assert!(
        st.journal.entries.iter().any(|e| e.fact == FactKey::Claim(jane_core::TextId(0))),
        "another kind keeps its own"
    );
    assert_eq!(
        st.journal.known.len() as u32,
        jane_sim::tuning::journal_ring() + 5 + 1 + 1,
        "the known map is not a ring"
    );
}

// --- nothing solid lands on a unit; fill; reveal -------------------------------------------------

#[test]
fn a_gate_dropped_on_her_stands_her_aside_on_her_side() {
    // verbs2.test.ts E12, lock: a wall across the room with a gateway in it, and a trigger in the
    // gateway that drops the gate on whoever walks in.
    let mut r = Room::new(false);
    r.bp.tiles.fill_rect(Rect::new(1, 15, 19, 1), Tile::Wall);
    r.bp.tiles.fill_rect(Rect::new(23, 15, 24, 1), Tile::Wall);
    let gate = r.prop("gate", "gate_h", 20, 15, |_| {});
    r.trigger("gateway", Rect::new(20, 15, 3, 1), TriggerMode::Enter, true, vec![Action::Lock(gate)]);
    let mut s = r.build();
    place(&mut s, 21, 15, Facing::South);
    idle(&mut s, 2);
    assert!(prop(&s, "gate").solid);
    let (x, y) = me(&s).pos.cell();
    assert!(y != 15, "not in the gate: {x},{y}");
    assert!(!s.runtime(ZoneId::County).unwrap().grid.solid(x, y));
    // She can walk away.
    let before = me(&s).pos;
    step(&mut s, jane_sim::InputFrame::walk(if y < 15 { Angle::NORTH } else { Angle::SOUTH }));
    assert_ne!(me(&s).pos, before);
}

#[test]
fn a_hedge_does_not_grow_through_her_it_waits_through_a_save() {
    // verbs2.test.ts E12, fill.
    let mut r = Room::new(false);
    let rect = r.rect("bed", Rect::new(20, 14, 4, 4));
    r.trigger("mat", Rect::new(30, 10, 2, 2), TriggerMode::Enter, true, vec![Action::Fill { rect, tile: Tile::Hedge }]);
    r.trigger(
        "mat2",
        Rect::new(30, 20, 2, 2),
        TriggerMode::Enter,
        true,
        vec![Action::Fill { rect: jane_core::Key::Local(0), tile: Tile::Garden }],
    );
    let mut s = r.build();
    // A friend stands in the bed; she steps on the mat.
    cmd(&mut s, Command::Open(true));
    let cmds = [jane_sim::StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: jane_sim::ClientToken(5) } }];
    s.step(&jane_sim::StepInput { commands: &cmds, ..jane_sim::StepInput::IDLE });
    place_seat(&mut s, 1, 21, 15, Facing::South);
    place(&mut s, 30, 10, Facing::South);
    idle(&mut s, 2);
    assert_eq!(s.state().zone(ZoneId::County).unwrap().pending_fill.len(), 1, "owed, not laid");
    assert_eq!(s.runtime(ZoneId::County).unwrap().grid.tile_at(21, 15), Tile::Floor);
    // Saved and loaded: still owed.
    let bytes = s.save();
    let bps = s.blueprints().clone();
    let mut s = Sim::from_save_with(&bytes, bps).unwrap();
    // A save loads with the guest parked: the bed is clear, and the hedge lands whole.
    idle(&mut s, 1);
    assert!(s.state().zone(ZoneId::County).unwrap().pending_fill.is_empty());
    let rt = s.runtime(ZoneId::County).unwrap();
    assert!((20..24).all(|x| (14..18).all(|y| rt.grid.tile_at(x, y) == Tile::Hedge)));
}

#[test]
fn reveal_marks_the_rooms_it_names_seen_and_nothing_else() {
    // verbs2.test.ts E13.
    let mut r = Room::new(true);
    let a = r.rect("room_a", Rect::new(30, 4, 6, 6));
    let nowhere = r.key("room_b");
    let names = r.bp.push_names(vec![a, nowhere]);
    let list = r.list(vec![Action::Reveal(names)]);
    r.prop("notice", "notice", 10, 16, |s| s.use_list = Some(list));
    let mut s = r.build();
    let geom = s.runtime(ZoneId::County).unwrap().fog;
    let seen = |s: &Sim, cx: i32, cy: i32| {
        let z = s.state().zone(ZoneId::County).unwrap();
        jane_sim::fog::fog_seen(&z.fog, geom, cx / geom.cells as i32, cy / geom.cells as i32)
    };
    assert!(!seen(&s, 32, 6));
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    assert!(seen(&s, 32, 6) && seen(&s, 35, 9));
    assert!(!seen(&s, 40, 20), "no corridor she has not walked");
}

// --- send --------------------------------------------------------------------------------------

#[test]
fn a_sent_unit_walks_to_the_mark_and_its_list_runs_with_it_as_the_subject() {
    // verbs2.test.ts E9.
    let mut r = Room::new(false);
    r.unit("pup", "dog", 10, 24);
    let pup = r.key("pup");
    let there = r.mark("kennel", 40, 24);
    let arrived = r.key("arrived");
    let then = r.list(vec![Action::Flag { key: FlagKey::Named(arrived), op: FlagOp::Add(1) }]);
    let go = r.list(vec![Action::Send { unit: pup, to: there, then: Some(then) }]);
    r.prop("whistle", "lever", 8, 16, |s| s.use_list = Some(go));
    let mut s = r.build();
    place(&mut s, 7, 16, Facing::East);
    cmd(&mut s, Command::Use);
    assert!(unit(&s, "pup").order.is_some());
    for _ in 0..600 {
        idle(&mut s, 1);
        if unit(&s, "pup").order.is_none() {
            break;
        }
    }
    let u = unit(&s, "pup");
    assert!(u.order.is_none());
    let (x, y) = u.pos.cell();
    assert!((x - 40).abs() <= 1 && (y - 24).abs() <= 1, "{x},{y}");
    assert_eq!(flag_named(&s, "arrived"), 1);
    assert_eq!(u.home, u.pos, "it stays where it was sent");
}

#[test]
fn a_walk_it_cannot_finish_is_given_up_after_its_budget_and_nothing_happens() {
    let mut r = Room::new(false);
    r.bp.tiles.fill_rect(Rect::new(30, 1, 1, 30), Tile::Wall);
    r.unit("pup", "dog", 10, 24);
    let pup = r.key("pup");
    let there = r.mark("kennel", 40, 24);
    let arrived = r.key("arrived");
    let then = r.list(vec![Action::Flag { key: FlagKey::Named(arrived), op: FlagOp::Add(1) }]);
    let go = r.list(vec![Action::Send { unit: pup, to: there, then: Some(then) }]);
    r.prop("whistle", "lever", 8, 16, |s| s.use_list = Some(go));
    let mut s = r.build();
    place(&mut s, 7, 16, Facing::East);
    cmd(&mut s, Command::Use);
    let until = unit(&s, "pup").order.as_ref().unwrap().until;
    let wait = until.0 - s.state().tick.0 + 2;
    idle(&mut s, wait);
    assert!(unit(&s, "pup").order.is_none());
    assert_eq!(flag_named(&s, "arrived"), 0);
}

// --- rest ----------------------------------------------------------------------------------------

#[test]
fn a_fire_mends_her_and_remembers_the_spot_and_a_bed_sleeps_to_morning() {
    // rest.test.ts "resting mends her, remembers the spot, and asks the app to save" and "a bed
    // can sleep the clock to morning".
    let cat = jane_data::catalog();
    let mut r = Room::new(false);
    r.prop("fire", "campfire", 10, 15, |s| s.talk = cat.story.dialogue_id("fire"));
    r.prop("bed", "bed", 30, 15, |s| s.talk = cat.story.dialogue_id("bed"));
    let mut s = r.build();
    {
        let id = s.state().players[0].unit;
        let u = s.state_mut().zone_mut(ZoneId::County).unwrap().unit_mut(id).unwrap();
        u.hp = jane_core::Milli(1000);
        u.energy = jane_core::Milli(10);
    }
    place(&mut s, 9, 16, Facing::East);
    assert!(s.view(Seat(0)).unwrap().near_rest());
    let clock = s.state().clock;
    events(&mut s);
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[]);
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::Rest && e.to.is_none()));
    let u = me(&s);
    assert_eq!(u.hp, jane_sim::units::max_hp(u));
    assert_eq!(u.energy, jane_sim::tuning::ENERGY_MAX);
    let rest = s.state().rest.unwrap();
    assert_eq!(rest.zone, ZoneId::County);
    assert!(s.state().clock < clock + 60, "no time passes at a fire");
    // The bed: sleep until morning.
    place(&mut s, 29, 16, Facing::East);
    let day = s.state().day;
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[0]);
    assert_eq!(s.state().hour(), 6);
    assert_eq!(s.state().day, day + 1);
}

#[test]
fn the_night_only_passes_when_the_whole_party_is_resting() {
    // coop.test.ts "the night only passes when the whole party is resting".
    let cat = jane_data::catalog();
    let mut r = Room::new(false);
    r.prop("bed", "bed", 30, 15, |s| s.talk = cat.story.dialogue_id("bed"));
    r.prop("bed2", "bed", 34, 15, |_| {});
    let mut s = r.build();
    cmd(&mut s, Command::Open(true));
    let cmds = [jane_sim::StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: jane_sim::ClientToken(5) } }];
    s.step(&jane_sim::StepInput { commands: &cmds, ..jane_sim::StepInput::IDLE });
    place_seat(&mut s, 1, 10, 10, Facing::East);
    place(&mut s, 29, 16, Facing::East);
    let hour = s.state().hour();
    events(&mut s);
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[0]);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::NightWaits));
    assert_eq!(s.state().hour(), hour);
    place_seat(&mut s, 1, 33, 16, Facing::East);
    place(&mut s, 29, 16, Facing::East);
    cmd(&mut s, Command::Use);
    talk_through(&mut s, &[0]);
    assert_eq!(s.state().hour(), 6);
}

// --- items, bags, crafting ------------------------------------------------------------------------

#[test]
fn items_ask_for_the_cooldowns_and_say_why_not() {
    let mut s = common::new_game();
    // An apple at full health: "I'm not hurt", and nothing is spent.
    let apples = holds(&s, "apple");
    events(&mut s);
    cmd(&mut s, Command::Item(item("apple")));
    assert!(toasts(&events(&mut s)).contains(&ToastKind::NotHurt));
    assert_eq!(holds(&s, "apple"), apples);
    // Hurt, it is eaten: one fewer, the gcd and its cooldown start, she is rooted a moment.
    {
        let id = s.state().players[0].unit;
        let u = s.state_mut().zone_mut(ZoneId::County).unwrap().unit_mut(id).unwrap();
        u.hp = jane_core::Milli(1000);
    }
    cmd(&mut s, Command::Item(item("apple")));
    assert_eq!(holds(&s, "apple"), apples - 1);
    let u = me(&s);
    assert!(u.gcd_until > s.state().tick && u.stop_until > s.state().tick);
    cmd(&mut s, Command::Item(item("apple")));
    assert_eq!(holds(&s, "apple"), apples - 1, "on the global cooldown");
    // A key is not used from the bag.
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("key_generic"), qty: 1 }));
    events(&mut s);
    cmd(&mut s, Command::Item(item("key_generic")));
    assert!(toasts(&events(&mut s)).contains(&ToastKind::FitsALock));
    // Julie's letter is kept, and opens her letter.
    idle(&mut s, 120);
    cmd(&mut s, Command::Item(item("julies_letter")));
    assert_eq!(holds(&s, "julies_letter"), 1);
    assert_eq!(
        s.view(Seat(0)).unwrap().dialogue().and_then(|d| d.tree),
        jane_data::catalog().story.dialogue_id("julies_letter")
    );
}

#[test]
fn bags_merge_and_swap_and_a_bound_thing_is_never_destroyed() {
    let mut s = common::new_game();
    let bag = |s: &Sim| s.state().players[0].bag.clone();
    let letter = bag(&s).iter().position(|x| x.is_some_and(|x| x.item == item("julies_letter"))).unwrap() as u8;
    events(&mut s);
    cmd(&mut s, Command::BagDestroy { slot: letter });
    assert!(toasts(&events(&mut s)).contains(&ToastKind::ShouldKeep));
    assert_eq!(holds(&s, "julies_letter"), 1);
    let apple = bag(&s).iter().position(|x| x.is_some_and(|x| x.item == item("apple"))).unwrap() as u8;
    cmd(&mut s, Command::BagMove { from: apple, to: 20 });
    assert_eq!(bag(&s)[20].map(|x| x.item), Some(item("apple")));
    assert_eq!(bag(&s)[usize::from(apple)], None);
    cmd(&mut s, Command::Dev(DevOp::Give { item: item("apple"), qty: 2 }));
    let first = bag(&s).iter().position(|x| x.is_some_and(|x| x.item == item("apple"))).unwrap();
    assert_eq!(first, 20, "a gift tops up the stack it has");
    cmd(&mut s, Command::BagDestroy { slot: 20 });
    assert_eq!(holds(&s, "apple"), 0);
}

#[test]
fn crafts_by_sorted_ids_and_only_consumes_inputs_when_the_output_fits() {
    // sim.test.ts "crafts by sorted ids and only consumes inputs when the output fits".
    let mut s = common::new_game();
    for i in ["small_water", "pansy", "gold_dust"] {
        cmd(&mut s, Command::Dev(DevOp::Give { item: item(i), qty: 1 }));
    }
    let slot_of = |s: &Sim, i: &str| {
        s.state().players[0].bag.iter().position(|x| x.is_some_and(|x| x.item == item(i))).unwrap() as u8
    };
    // In an order no recipe is written in.
    for (n, i) in ["small_water", "gold_dust", "pansy"].into_iter().enumerate() {
        let b = slot_of(&s, i);
        cmd(&mut s, Command::CraftPut { bag: b, slot: n as u8 });
    }
    assert_eq!(s.view(Seat(0)).unwrap().craft_output(), Some((item("potion_manashield"), 1)));
    assert_eq!(holds(&s, "pansy"), 0);
    // Cleared, they come back.
    cmd(&mut s, Command::CraftClear { slot: 2 });
    assert_eq!(holds(&s, "pansy"), 1);
    assert_eq!(s.view(Seat(0)).unwrap().craft_output(), None);
    let b = slot_of(&s, "pansy");
    cmd(&mut s, Command::CraftPut { bag: b, slot: 2 });
    // A full bag: the output does not fit, and nothing is consumed.
    for _ in 0..24 {
        cmd(&mut s, Command::Dev(DevOp::Give { item: item("rock"), qty: 16 }));
    }
    events(&mut s);
    cmd(&mut s, Command::CraftTake);
    assert!(toasts(&events(&mut s)).contains(&ToastKind::InventoryFull));
    assert!(s.state().players[0].craft.iter().all(Option::is_some));
    // Room made: it is taken.
    let rock = slot_of(&s, "rock");
    cmd(&mut s, Command::BagDestroy { slot: rock });
    cmd(&mut s, Command::CraftTake);
    assert_eq!(holds(&s, "potion_manashield"), 1);
    assert!(s.state().players[0].craft.iter().all(Option::is_none));
    // One input: a recipe too.
    let b = slot_of(&s, "rock");
    cmd(&mut s, Command::CraftPut { bag: b, slot: 1 });
    assert_eq!(s.view(Seat(0)).unwrap().craft_output(), Some((item("stone"), 1)));
    cmd(&mut s, Command::CraftClearAll);
    assert!(s.state().players[0].craft.iter().all(Option::is_none));
}

#[test]
fn never_loses_a_reward_when_the_bag_is_full() {
    // sim.test.ts "never loses a reward when the bag is full": the hand-in through the same list
    // the dog uses.
    let cat = jane_data::catalog();
    let mut s = common::new_game();
    for _ in 0..40 {
        cmd(&mut s, Command::Dev(DevOp::Give { item: item("gold_bar"), qty: 16 }));
    }
    assert!(s.state().players[0].bag.iter().all(Option::is_some));
    let drops = s.state().zone(ZoneId::County).unwrap().drops.len();
    let q = cat.story.quest_id("defeat_skeleton").unwrap();
    cmd(&mut s, Command::Dev(DevOp::Quest(q)));
    s.state_mut().quests.active.iter_mut().find(|p| p.quest == q).unwrap().counts[0] = 1;
    let dog = cat.story.dialogue_id("dog").unwrap();
    let tree = cat.story.dialogue(dog);
    let node = tree.node_index("reward").unwrap();
    let line = tree.node(node).lines.len() as u16 - 1;
    s.state_mut().players[0].dialogue =
        Some(Dialogue { tree: Some(dog), node, line, speaker: Speaker::None, read: None });
    cmd(&mut s, Command::Choose { option: 0 });
    assert!(quest_done(&s, "defeat_skeleton"));
    let z = s.state().zone(ZoneId::County).unwrap();
    assert_eq!(z.drops.len(), drops + 1);
    assert_eq!(z.drops.last().unwrap().item, item("key_auntie_house"));
    // Picked up again once there is room.
    let slot = 0;
    cmd(&mut s, Command::BagDestroy { slot });
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::Take(item("key_auntie_house"))));
    cmd(&mut s, Command::Use);
    assert_eq!(holds(&s, "key_auntie_house"), 1);
    assert!(s.state().zone(ZoneId::County).unwrap().drops.len() == drops);
}

// --- dialogue -----------------------------------------------------------------------------------

#[test]
fn use_advances_a_conversation_and_close_ends_it_and_with_company_nothing_pauses() {
    let cat = jane_data::catalog();
    let mut r = Room::new(false);
    r.prop("fire", "campfire", 10, 15, |s| s.talk = cat.story.dialogue_id("fire"));
    let mut s = r.build();
    place(&mut s, 9, 16, Facing::East);
    cmd(&mut s, Command::Use);
    let d = s.view(Seat(0)).unwrap().dialogue().unwrap();
    assert_eq!((d.line, d.awaiting_choice), (0, false));
    assert!(s.frozen());
    let tick = s.state().tick;
    cmd(&mut s, Command::Use);
    assert_eq!(s.view(Seat(0)).unwrap().dialogue().unwrap().line, 1, "USE is Advance in a conversation");
    assert_eq!(s.state().tick, tick, "the world held still");
    cmd(&mut s, Command::CloseDialogue);
    assert!(s.view(Seat(0)).unwrap().dialogue().is_none());
    assert!(!s.frozen());
    // With company, nothing pauses.
    cmd(&mut s, Command::Open(true));
    let cmds = [jane_sim::StampedCommand { seat: None, seq: 0, cmd: Command::Join { who: jane_sim::ClientToken(5) } }];
    s.step(&jane_sim::StepInput { commands: &cmds, ..jane_sim::StepInput::IDLE });
    cmd(&mut s, Command::Use);
    assert!(s.view(Seat(0)).unwrap().dialogue().is_some() && !s.frozen());
    let tick = s.state().tick;
    idle(&mut s, 3);
    assert_eq!(s.state().tick.0, tick.0 + 3);
}
