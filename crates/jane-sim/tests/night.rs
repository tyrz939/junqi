//! The night (WORLD.md §2.1 and §4.3; the owner, 2026-10-06): a 24-minute day; the night shift
//! up at the bell out of the light and gone at six; finds that show only at night; a bed that
//! sleeps the clock only at Julie's or an inn, and a fire that never does.

mod common;

use common::new_game;
use jane_core::ZoneId;
use jane_sim::light::lit_at;
use jane_sim::{Command, DevOp, Seat, Sim, StampedCommand, StepInput};

fn hour(s: &mut Sim, h: u8) {
    let cmds = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour: h }) }];
    s.step(&StepInput { frames: StepInput::IDLE.frames, commands: &cmds });
}

fn steps(s: &mut Sim, n: u32) {
    for _ in 0..n {
        s.step(&StepInput::IDLE);
    }
}

/// The night shift standing in the county now: `(def id, out in the light?)`.
fn night_shift(s: &Sim) -> Vec<(&'static str, bool)> {
    let cat = jane_data::catalog();
    let zone = s.state().zone(ZoneId::County).expect("the county");
    let rt = s.runtime(ZoneId::County).expect("its runtime");
    zone.units
        .iter()
        .filter(|u| u.alive && !u.hidden && cat.combat.unit(u.def).night_only)
        .map(|u| (cat.combat.unit(u.def).id, lit_at(zone, rt, s.state().clock, u.pos, Some(s.state().night.stage))))
        .collect()
}

/// A game hour is a real minute: the bell comes eight game hours, not minutes, after New Game.
#[test]
fn the_day_is_twenty_four_real_minutes() {
    let mut s = new_game();
    assert_eq!(s.state().hour(), 13, "the one o'clock train");
    steps(&mut s, 8 * 60 * 60);
    assert_eq!(s.state().hour(), 21, "eight real minutes from the train to the bell");
    assert!(s.state().is_night());
}

/// At the bell the night shift is up, every one of it out of the lamps' and fires' light, black
/// dogs among the bones; at six it is gone.
#[test]
fn the_night_shift_rises_out_of_the_light_and_is_gone_at_six() {
    let mut s = new_game();
    steps(&mut s, 31);
    assert!(night_shift(&s).is_empty(), "nothing of the night's by day");
    hour(&mut s, 21);
    steps(&mut s, 60);
    let up = night_shift(&s);
    assert!(up.len() >= 40, "{} up after the bell", up.len());
    let lit: Vec<_> = up.iter().filter(|u| u.1).collect();
    assert!(lit.is_empty(), "standing in warm light: {lit:?}");
    assert!(up.iter().any(|u| u.0 == "night_hound"), "a black dog among them");
    hour(&mut s, 6);
    steps(&mut s, 60);
    assert!(night_shift(&s).is_empty(), "gone at six: {:?}", night_shift(&s));
}

/// The night's finds: hidden by day, shown at the bell (glinting, a cold light that keeps nothing
/// off), hidden again at six.
#[test]
fn a_night_find_shows_only_at_night() {
    let mut s = new_game();
    let shown = |s: &Sim| {
        let v = s.view(Seat(0)).expect("her view");
        let keys = ["night_glint_wood", "night_glint_pool", "night_glint_bellfield"];
        let syms: Vec<_> = keys.iter().filter_map(|k| v.sym(k)).collect();
        assert_eq!(syms.len(), 3, "all three are placed");
        v.props().filter(|p| syms.contains(&p.key) && !p.hidden).count()
    };
    assert_eq!(shown(&s), 0);
    hour(&mut s, 20);
    steps(&mut s, jane_sim::tuning::TICKS_PER_HOUR + 1);
    assert_eq!(shown(&s), 3, "at the bell");
    hour(&mut s, 5);
    steps(&mut s, jane_sim::tuning::TICKS_PER_HOUR + 1);
    assert_eq!(shown(&s), 0, "at six");
}
