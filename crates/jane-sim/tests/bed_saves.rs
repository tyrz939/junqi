//! A bed by day saves (the owner, 2026-10-01): E at Julie's bed when she is not tired says so
//! ("Not tired. The day's written down all the same.") and asks the app to write the world, with
//! whose rest it was; she may still lie down. By night the bed is a bed, and sleep saves as it
//! did. A guest's rest asks the host's machine to save, and says it was hers.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::ZoneId;
use jane_data::catalog;
use jane_sim::event::EventKind;
use jane_sim::input::DevOp;
use jane_sim::{Command, Seat, Sim};

fn to_the_bed(s: &mut Sim, hour: u8) {
    let mark = jane_sim::sym::of_name(catalog().name_id("start").unwrap());
    cmd(s, Command::Dev(DevOp::Tp { zone: ZoneId::House, mark }));
    assert!(walk_to_prop(s, "julies_bed"));
    cmd(s, Command::Dev(DevOp::Time { hour }));
    events(s);
}

fn opens_on(s: &Sim) -> &'static str {
    s.view(Seat(0)).unwrap().dialogue().and_then(|d| d.node).map_or("", |n| n.id)
}

#[test]
fn by_day_the_bed_writes_the_day_down_and_she_need_not_sleep() {
    let mut s = new_game();
    to_the_bed(&mut s, 12);
    let clock = s.state().clock;
    cmd(&mut s, Command::Use);
    assert_eq!(opens_on(&s), "day", "not tired");
    // "Leave it": no sleep, and the save asked for all the same.
    talk_through(&mut s, &[1]);
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::Rest), "the app is asked to save");
    assert!(ev.iter().any(|e| e.kind == EventKind::Rested { by: Seat(0) }), "and told whose rest it was");
    assert!(s.state().clock - clock < 7200, "no night passed");
    assert!(s.state().rest.is_some_and(|r| r.zone == ZoneId::House), "she wakes here if it goes badly");
}

#[test]
fn by_night_the_bed_is_for_sleeping_and_sleep_saves() {
    let mut s = new_game();
    to_the_bed(&mut s, 22);
    cmd(&mut s, Command::Use);
    assert_eq!(opens_on(&s), "bed");
    talk_through(&mut s, &[0]);
    let ev = events(&mut s);
    assert!(ev.iter().any(|e| e.kind == EventKind::Rest));
    assert_eq!(s.state().hour(), 6, "slept to six");
}
