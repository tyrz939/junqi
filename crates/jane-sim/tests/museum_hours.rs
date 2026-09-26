//! The Museum opens ten to four (DUNGEONS.md §3.2 "Opening hours"): the county's door is shut
//! from four to ten and says when it opens; the way out is never shut, so nobody is kept in; the
//! bench on the steps knows the hours, and waits with her until ten.

mod common;

use common::bot::*;
use jane_core::ZoneId;
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::{Command, Seat, Sim};

fn tp(s: &mut Sim, zone: ZoneId, mark: &str) {
    let mark = sym(s, mark);
    cmd(s, Command::Dev(DevOp::Time { hour: 12 }));
    cmd(s, Command::Dev(DevOp::Tp { zone, mark }));
    idle(s, 2);
    assert_eq!(zone_of(s), zone);
}

fn hour(s: &mut Sim, hour: u8) {
    cmd(s, Command::Dev(DevOp::Time { hour }));
}

/// What trying the door said, if it said it was shut.
fn shut_says(s: &mut Sim) -> Option<String> {
    s.drain_events();
    cmd(s, Command::Use);
    let cat = jane_data::catalog();
    s.drain_events().iter().find_map(|e| match e.kind {
        EventKind::Toast(ToastKind::NightLock(t)) => Some(match t {
            jane_core::TextRef::Text(id) => cat.text(id).to_owned(),
            jane_core::TextRef::Local(_) => String::from("?"),
        }),
        _ => None,
    })
}

#[test]
fn the_museum_door_opens_ten_to_four_and_says_so() {
    for (at, open) in [(9, false), (10, true), (15, true), (16, false), (22, false), (3, false)] {
        let mut s = common::new_game();
        tp(&mut s, ZoneId::County, "museum_mouth");
        assert!(walk_to_prop(&mut s, "museum_door"));
        // The door wants the Museum's key whatever the hour; she has it, and it has turned.
        let key = jane_data::catalog().combat.item_id("key_museum").unwrap();
        cmd(&mut s, Command::Dev(DevOp::Give { item: key, qty: 1 }));
        cmd(&mut s, Command::Use);
        assert!(!prop(&s, "museum_door").locked);
        hour(&mut s, at);
        let says = shut_says(&mut s);
        assert_eq!(says.is_none(), open, "at {at}:00");
        if let Some(t) = says {
            assert!(t.starts_with("Open ten to four."), "{t}");
        }
        idle(&mut s, 2);
        assert_eq!(zone_of(&s), if open { ZoneId::Museum } else { ZoneId::County }, "at {at}:00");
    }
}

/// Nobody is ever shut in: no door of the Museum's own is night-locked, on any seed's blueprint
/// the tests build, and the solver proves the zone with no clock at all.
#[test]
fn the_museum_way_out_is_never_shut() {
    let bps = common::bps();
    let bp = bps.get(ZoneId::Museum);
    assert!(bp.props.iter().any(|p| p.to.is_some()), "a way out");
    assert!(bp.props.iter().all(|p| p.night_lock.is_none()));
    let mut s = common::new_game();
    tp(&mut s, ZoneId::Museum, "entry");
    hour(&mut s, 23);
    let exit = bp.props.iter().find(|p| p.to.is_some_and(|d| d.zone == ZoneId::County)).expect("a door out");
    let jane_core::Key::Name(n) = exit.key else { panic!("a named way out") };
    let name = jane_data::catalog().name(n);
    assert!(walk_to_prop(&mut s, name));
    assert!(shut_says(&mut s).is_none(), "out at eleven at night");
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::County);
}

/// Out of hours the bench offers to wait, and waiting sleeps the clock to ten; in them it only
/// says the doors are open.
#[test]
fn the_bench_on_the_steps_waits_for_ten() {
    let node = |s: &Sim| s.view(Seat(0)).unwrap().dialogue().expect("talking to the bench").node.expect("a node").id;
    let mut s = common::new_game();
    tp(&mut s, ZoneId::County, "museum_mouth");
    assert!(walk_to_prop(&mut s, "museum_bench"));
    hour(&mut s, 12);
    cmd(&mut s, Command::Use);
    assert_eq!(node(&s), "open");
    talk_through(&mut s, &[]);
    hour(&mut s, 17);
    let day = s.state().day;
    cmd(&mut s, Command::Use);
    assert_eq!(node(&s), "shut");
    talk_through(&mut s, &[0]);
    idle(&mut s, 2);
    assert_eq!((s.state().hour(), s.state().day), (10, day + 1), "the night passed on the bench");
}
