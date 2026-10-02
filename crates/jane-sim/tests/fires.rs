//! Made fires, a fire's rest over time, and growth banked by a rest (PLAY-PLAN.md §2.2, `fire.rs`),
//! with `fires_made` set by hand: a pit is made with two deadwood and a light held a second, and
//! never by dice; a match will not take in the rain and nothing is spent; planks are refused; a
//! made fire burns four hours and is ash, and ash still wakes her; seated she mends to full in
//! thirty seconds and a step gets her up; a jar found since the rest lies where she fell.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::action::Facing;
use jane_core::{Milli, Tick, ZoneId};
use jane_sim::event::{FireWant, ToastKind};
use jane_sim::input::InputFrame;
use jane_sim::interact::{FocusRef, Verb};
use jane_sim::tuning::{FIRE_BURNS, FIRE_HOLD_TICKS, TICKS_PER_HOUR};
use jane_sim::units::max_hp;
use jane_sim::{Command, EventKind, PropId, Seat, Sim};

fn item(id: &str) -> jane_core::ItemId {
    jane_data::catalog().combat.item_id(id).unwrap()
}

fn give(s: &mut Sim, id: &str, n: u16) {
    jane_sim::bag::bag_add(&mut s.state_mut().players[0].bag[..], item(id), n);
}

fn county_prop(s: &Sim, id: PropId) -> jane_sim::Prop {
    s.state().zone(ZoneId::County).unwrap().props.iter().find(|p| p.id == id).unwrap().clone()
}

/// A new game with made fires, stood south of a cold pit with it in focus. The pit's id.
fn at_a_pit() -> (Sim, PropId) {
    let mut s = new_game();
    s.state_mut().fires_made = true;
    let pit = jane_data::catalog().story.prop_id("campfire_cold").unwrap();
    let pits: Vec<_> = s
        .state()
        .zone(ZoneId::County)
        .unwrap()
        .props
        .iter()
        .filter(|p| p.def == pit && !p.hidden)
        .map(|p| (p.id, p.cell))
        .collect();
    assert!(!pits.is_empty(), "the county has cold pits");
    for (id, cell) in pits {
        let (x, y) = (i32::from(cell.x), i32::from(cell.y) + 2);
        if s.runtime(ZoneId::County).unwrap().grid.solid(x, y) {
            continue;
        }
        place_seat(&mut s, 0, x, y, Facing::North);
        idle(&mut s, 1);
        if s.view(Seat(0)).unwrap().focus().is_some_and(|f| f.target == FocusRef::Prop(id)) {
            s.drain_events();
            return (s, id);
        }
    }
    panic!("no pit to stand at");
}

fn held(s: &mut Sim, n: u32) {
    let f = InputFrame { use_held: true, ..InputFrame::IDLE };
    for _ in 0..n {
        step(s, f);
    }
}

fn toasts(s: &mut Sim) -> Vec<ToastKind> {
    events(s)
        .into_iter()
        .filter_map(|e| match e.kind {
            EventKind::Toast(t) => Some(t),
            _ => None,
        })
        .collect()
}

#[test]
fn a_pit_is_made_with_two_deadwood_and_a_match_held_a_second() {
    let (mut s, id) = at_a_pit();
    // Nothing in the bag: the notice, and what it wants.
    cmd(&mut s, Command::Use);
    assert!(toasts(&mut s).contains(&ToastKind::FireWants(FireWant::Wood)));
    give(&mut s, "wood", 2);
    cmd(&mut s, Command::Use);
    assert!(toasts(&mut s).contains(&ToastKind::FireWants(FireWant::Planks)), "planks are too good to burn");
    give(&mut s, "deadwood", 3);
    cmd(&mut s, Command::Use);
    assert!(toasts(&mut s).contains(&ToastKind::FireWants(FireWant::Light)));
    give(&mut s, "match", 2);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::MakeFire));
    held(&mut s, u32::from(FIRE_HOLD_TICKS) - 1);
    assert!(!county_prop(&s, id).on, "a second's hold, not less");
    let t = s.state().tick;
    held(&mut s, 1);
    let p = county_prop(&s, id);
    assert!(p.on && !p.used, "lit");
    assert_eq!(p.burns_until, Some(Tick(t.0 + 1 + FIRE_BURNS)));
    assert_eq!((holds(&s, "deadwood"), holds(&s, "match"), holds(&s, "wood")), (1, 1, 2), "two sticks and a match");
    assert!(toasts(&mut s).iter().any(|t| matches!(t, ToastKind::FireLit { by: Seat(0), .. })));
    assert!(s.view(Seat(0)).unwrap().near_rest(), "a rest while it burns");
    // Her fire, a stick in her bag: feeding it is the hold; two hours more.
    idle(&mut s, 2);
    assert_eq!(s.view(Seat(0)).unwrap().focus().map(|f| f.verb), Some(Verb::AddWood));
    held(&mut s, u32::from(FIRE_HOLD_TICKS));
    assert_eq!(county_prop(&s, id).burns_until, Some(Tick(t.0 + 1 + FIRE_BURNS + 2 * TICKS_PER_HOUR)));
    assert_eq!(holds(&s, "deadwood"), 0);
}

#[test]
fn a_match_will_not_take_in_the_rain_and_nothing_is_spent() {
    let (mut s, id) = at_a_pit();
    give(&mut s, "deadwood", 2);
    give(&mut s, "match", 1);
    s.state_mut().zone_mut(ZoneId::County).unwrap().wetness = [255; 3];
    held(&mut s, u32::from(FIRE_HOLD_TICKS) + 5);
    assert!(!county_prop(&s, id).on);
    cmd(&mut s, Command::Use);
    assert!(toasts(&mut s).contains(&ToastKind::FireWants(FireWant::Wet)), "it says why");
    assert_eq!((holds(&s, "deadwood"), holds(&s, "match")), (2, 1), "nothing spent");
}

#[test]
fn a_made_fire_burns_four_hours_then_is_ash_and_ash_still_wakes_her() {
    let (mut s, id) = at_a_pit();
    give(&mut s, "deadwood", 2);
    give(&mut s, "match", 1);
    held(&mut s, u32::from(FIRE_HOLD_TICKS));
    // Rest: the waking point is here at once.
    cmd(&mut s, Command::Use);
    let rest = s.state().rest.expect("a waking point");
    assert_eq!(rest.zone, ZoneId::County);
    let until = county_prop(&s, id).burns_until.unwrap();
    // Dry skies all the while, so only the clock puts it out.
    s.state_mut().tick = Tick(until.0 - 2);
    idle(&mut s, 1);
    assert!(county_prop(&s, id).on, "not before its time");
    idle(&mut s, (TICKS_PER_HOUR / 6) + 2);
    let p = county_prop(&s, id);
    assert!(!p.on && p.used && p.burns_until.is_none(), "ash: {p:?}");
    assert!(!s.view(Seat(0)).unwrap().near_rest(), "ash is no rest");
    assert_eq!(s.state().rest, Some(rest), "the place keeps her waking point");
    // She falls far off; she wakes beside the ash.
    let body = s.state().players[0].unit;
    s.queue_hit(
        ZoneId::County,
        jane_sim::Hit {
            to: body,
            amount: Milli::from_points(100_000),
            school: jane_core::action::School::Physical,
            from: None,
            crit: false,
            status: None,
        },
    );
    idle(&mut s, 242);
    let u = unit_by_id(&s, body);
    assert!(u.alive);
    assert_eq!(u.pos, rest.pos, "woken at the ash");
}

#[test]
fn seated_she_mends_to_full_in_thirty_seconds_and_a_step_gets_her_up() {
    let (mut s, id) = at_a_pit();
    give(&mut s, "deadwood", 2);
    give(&mut s, "match", 1);
    held(&mut s, u32::from(FIRE_HOLD_TICKS));
    assert!(county_prop(&s, id).on);
    let body = s.state().players[0].unit;
    let low = |s: &mut Sim| {
        let u = s.state_mut().zone_mut(ZoneId::County).unwrap().unit_mut(body).unwrap();
        u.hp = Milli(1000);
    };
    low(&mut s);
    cmd(&mut s, Command::Use);
    assert!(me(&s).seated.is_some(), "sat down");
    assert!(me(&s).hp.0 < 5_000, "a fire does not mend at once");
    idle(&mut s, 900);
    let half = me(&s).hp;
    assert!(half.0 > max_hp(me(&s)).0 / 3 && half < max_hp(me(&s)), "halfway in fifteen seconds: {half:?}");
    idle(&mut s, 900);
    assert_eq!(me(&s).hp, max_hp(me(&s)), "whole in thirty");
    assert!(me(&s).seated.is_none());
    // Again, and a step gets her up.
    low(&mut s);
    cmd(&mut s, Command::Use);
    idle(&mut s, 60);
    for _ in 0..10 {
        step(&mut s, InputFrame::walk(jane_core::Angle::SOUTH));
    }
    assert!(me(&s).seated.is_none(), "up");
    let hp = me(&s).hp;
    idle(&mut s, 300);
    assert_eq!(me(&s).hp, hp, "no mending standing");
}

#[test]
fn without_the_rule_every_fire_is_as_it_was() {
    let mut s = new_game();
    assert!(!s.state().fires_made || jane_sim::tuning::FIRES_MADE);
    s.state_mut().fires_made = false;
    let pit = jane_data::catalog().story.prop_id("campfire_cold").unwrap();
    let p = s.state().zone(ZoneId::County).unwrap().props.iter().find(|p| p.def == pit).unwrap().clone();
    place_seat(&mut s, 0, i32::from(p.cell.x), i32::from(p.cell.y) + 2, Facing::North);
    idle(&mut s, 1);
    let f = s.view(Seat(0)).unwrap().focus();
    assert!(f.is_none_or(|f| f.target != FocusRef::Prop(p.id)), "a cold pit is nothing to use");
}

fn fell(s: &mut Sim) {
    let body = s.state().players[0].unit;
    let z = s.state().players[0].zone;
    s.queue_hit(
        z,
        jane_sim::Hit {
            to: body,
            amount: Milli::from_points(100_000),
            school: jane_core::action::School::Physical,
            from: None,
            crit: false,
            status: None,
        },
    );
    idle(s, 242);
    assert!(unit_by_id(s, body).alive, "up again");
}

#[test]
fn a_jar_found_since_the_rest_lies_where_she_fell_then_goes_home() {
    use jane_core::Action;
    use jane_core::action::Stat;
    let mut r = common::room::Room::new(false);
    let k = r.key("strong_jar");
    let list = r.list(vec![Action::Grow { stat: Stat::Strength, amount: 4, id: k }]);
    r.prop("strong_jar", "jar", 10, 16, |s| s.use_list = Some(list));
    let mut s = r.build();
    s.state_mut().fires_made = true;
    place(&mut s, 9, 16, Facing::East);
    idle(&mut s, 1);
    let before = s.state().growth.strength;
    let body_before = me(&s).strength;
    cmd(&mut s, Command::Use);
    assert_eq!(s.state().growth.strength, before + 4, "found");
    assert_eq!(s.state().growth.unbanked.len(), 1, "unbanked till a rest");
    // She falls across the room: the growth comes off, and the jar lies there.
    place(&mut s, 30, 10, Facing::East);
    idle(&mut s, 1);
    fell(&mut s);
    assert_eq!(s.state().growth.strength, before, "it came off");
    assert_eq!(me(&s).strength, body_before, "off her body too");
    let jar = prop(&s, "strong_jar");
    assert!(!jar.used && !jar.hidden, "to be found again: {jar:?}");
    assert!((i32::from(jar.cell.x) - 30).abs() <= 4 && (i32::from(jar.cell.y) - 10).abs() <= 4, "where she fell");
    assert!(s.state().growth.unbanked[0].lying);
    // She falls again before taking it back: it goes home.
    fell(&mut s);
    let jar = prop(&s, "strong_jar");
    assert_eq!((jar.cell.x, jar.cell.y), (10, 16), "home");
    assert!(!jar.used && s.state().growth.unbanked.is_empty());
    // Found again and banked by a rest: a death takes nothing.
    place(&mut s, 9, 16, Facing::East);
    idle(&mut s, 1);
    cmd(&mut s, Command::Use);
    assert_eq!(s.state().growth.strength, before + 4);
    jane_sim::fire::bank(s.state_mut());
    fell(&mut s);
    assert_eq!(s.state().growth.strength, before + 4, "banked");
}
