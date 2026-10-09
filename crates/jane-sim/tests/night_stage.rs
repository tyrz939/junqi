//! The night's stage (NIGHT.md §3.2, §9.1 R1): latched at the turn (the bell's first stroke, or
//! 21:00 where no bell rings), never changed mid-night, cleared at the dawn turn; computed from
//! the spine's flags and capped by the ending; saved and hashed; the party's, not a seat's; and
//! the same over every way the blueprints are held. Warm light is safe; a wrong lamp is not.

mod field;

use std::sync::Arc;

use field::{cmd, field, party, start_sym};
use jane_core::action::Facing;
use jane_core::blueprint::{Mark, PropSpawn};
use jane_core::{Blueprint, Cell, Key, Rect, Tick, Tile, Vec2, ZoneId};
use jane_sim::blueprints::Build;
use jane_sim::event::EventKind;
use jane_sim::night::{EARLY_BELL, Night};
use jane_sim::state::FlagKey;
use jane_sim::tuning::{TICKS_PER_DAY, TICKS_PER_HOUR};
use jane_sim::{Blueprints, ClientToken, Command, Seat, Sim, StepInput};

const H: u32 = TICKS_PER_HOUR;

/// A world flag set and, for a consequence's name (`mine_quiet`, `works_dark`, `burial_quiet`,
/// `bell_stopped`), the consequence fired, as a boss's fall fires it.
fn set_flag(s: &mut Sim, name: &str, v: i32) {
    let c = jane_data::catalog().living.consequence_id(name);
    if let Some(c) = c {
        s.state_mut().consequences_done.set(u32::from(c.0), v != 0);
    }
    match s.state().syms.find(name) {
        Some(k) => {
            s.state_mut().flags.insert(FlagKey::Named(k), v);
        }
        None => assert!(c.is_some(), "{name} is neither a flag nor a consequence"),
    }
}

/// The clock set to one tick before `at`, then stepped onto it.
fn step_onto(s: &mut Sim, at: u32) {
    s.state_mut().clock = (at + TICKS_PER_DAY - 1) % TICKS_PER_DAY;
    s.step(&StepInput::IDLE);
    assert_eq!(s.state().clock, at);
}

fn steps(s: &mut Sim, n: u32) {
    for _ in 0..n {
        s.step(&StepInput::IDLE);
    }
}

fn night(s: &Sim) -> Night {
    s.state().night
}

#[test]
fn night_stage_latches_at_the_turn_and_only_there() {
    let mut s = field();
    assert_eq!(night(&s), Night::default(), "New Game is day");
    step_onto(&mut s, 21 * H - 1);
    assert_eq!(night(&s).stage, 0, "nothing before the bell");
    s.step(&StepInput::IDLE);
    let turned = s.state().tick;
    assert_eq!(night(&s), Night { stage: 1, turned_at: turned }, "N1 from New Game, at the stroke");

    // The mine falls at 23:00: tonight is unchanged, tomorrow's is deeper.
    step_onto(&mut s, 23 * H);
    set_flag(&mut s, "mine_quiet", 1);
    steps(&mut s, 30);
    assert_eq!(night(&s), Night { stage: 1, turned_at: turned }, "no change mid-night");
    step_onto(&mut s, 3 * H);
    assert_eq!(night(&s).stage, 1);

    // The dawn turn clears it, with its own tick.
    step_onto(&mut s, 6 * H);
    assert_eq!(night(&s), Night { stage: 0, turned_at: s.state().tick });
    step_onto(&mut s, 20 * H + 55 * H / 60);
    assert_eq!(night(&s).stage, 0, "no early latch without the omen");

    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s), Night { stage: 2, turned_at: s.state().tick }, "the next night is the worse");
    set_flag(&mut s, "works_dark", 1);
    set_flag(&mut s, "burial_quiet", 1);
    step_onto(&mut s, 6 * H);
    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s).stage, 4, "the Ball is up");
}

/// The early bell (§2.2): on a Tuesday under the omen it rings at 20:50 and the turn comes with
/// it; the sim's own night keeps 21:00.
#[test]
fn the_early_bell_turns_the_night_at_ten_to_nine() {
    let mut s = field();
    set_flag(&mut s, "omen:early_bell", 1);
    s.state_mut().day = 2; // a Tuesday
    assert_eq!(s.state().weekday(), 2);
    step_onto(&mut s, EARLY_BELL);
    let at = s.state().tick;
    let rang = s.drain_events().iter().any(|e| matches!(e.kind, EventKind::Bell { strikes: 9, church: false, .. }));
    assert!(rang, "the bell rang at 20:50");
    assert_eq!(night(&s), Night { stage: 1, turned_at: at }, "the turn is the bell's first stroke");
    assert!(!s.state().is_night(), "the sim's night keeps 21:00");
    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s), Night { stage: 1, turned_at: at }, "nine o'clock does not turn it again");

    // Another day of the week: nine o'clock, as built.
    s.state_mut().day = 3;
    step_onto(&mut s, 6 * H);
    step_onto(&mut s, EARLY_BELL);
    assert_eq!(night(&s).stage, 0);
    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s), Night { stage: 1, turned_at: s.state().tick });
}

/// After the Timekeeper (§2.2, STORY CHANGE 2): nothing rings, and the night turns anyway, N4.
#[test]
fn the_silent_turn_comes_at_nine_after_the_bell_stops() {
    let mut s = field();
    set_flag(&mut s, "bell_stopped", 1);
    set_flag(&mut s, "omen:early_bell", 1);
    s.state_mut().day = 2;
    step_onto(&mut s, EARLY_BELL);
    assert_eq!(night(&s).stage, 0, "no early bell once the bell has stopped");
    step_onto(&mut s, 21 * H);
    let rang = s.drain_events().iter().any(|e| matches!(e.kind, EventKind::Bell { .. }));
    assert!(!rang, "no bell");
    assert_eq!(night(&s), Night { stage: 4, turned_at: s.state().tick });
}

/// Each ending's rule (§3.1): the shield held keeps no night world, the hill no night, the train
/// N4.
#[test]
fn each_ending_keeps_its_night() {
    for (end, want) in [(1, 0), (2, 0), (3, 4)] {
        let mut s = field();
        set_flag(&mut s, "the_end", end);
        step_onto(&mut s, 21 * H);
        steps(&mut s, 5);
        assert_eq!(night(&s).stage, want, "the_end {end}");
    }
    let mut s = field();
    set_flag(&mut s, "night_gone", 1);
    step_onto(&mut s, 22 * H);
    assert_eq!(night(&s).stage, 0, "night_gone");
}

/// A clock set by hand at night (the console's `time`, the PSP's script) lands in the night as it
/// is: latched at the next step, its turn an hour past.
#[test]
fn a_clock_set_into_the_night_finds_it_turned() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Dev(jane_sim::DevOp::Time { hour: 22 }));
    s.step(&StepInput::IDLE);
    let now = s.state().tick.0;
    let since = s.state().clock - 21 * H;
    assert_eq!(night(&s), Night { stage: 1, turned_at: Tick(now.saturating_sub(since)) });
    cmd(&mut s, Some(0), Command::Dev(jane_sim::DevOp::Time { hour: 10 }));
    s.step(&StepInput::IDLE);
    assert_eq!(night(&s).stage, 0);
}

/// §2.6: a save at 21:00:02 (inside the turn) and one at 23:00 at N4 load as the same state, the
/// turn's progress derived (`tick - turned_at`); and the hash covers the stage.
#[test]
fn save_and_load_at_night_and_mid_turn_round_trip() {
    let mut s = field();
    step_onto(&mut s, 21 * H);
    steps(&mut s, 2 * jane_core::num::TICK_RATE);
    for deep in [false, true] {
        if deep {
            for f in ["mine_quiet", "works_dark", "burial_quiet"] {
                set_flag(&mut s, f, 1);
            }
            step_onto(&mut s, 6 * H);
            step_onto(&mut s, 23 * H);
            assert_eq!(night(&s).stage, 4);
        } else {
            assert!(s.state().tick.0 - night(&s).turned_at.0 < 4 * jane_core::num::TICK_RATE, "mid-turn");
        }
        let (bytes, hash) = s.save_and_hash();
        let b = Sim::from_snapshot_with(&bytes, s.blueprints().clone()).expect("loads");
        assert_eq!(b.state(), s.state(), "decode(save(s)) == s");
        assert_eq!(b.hash(), hash);
        let mut c = s.state().clone();
        c.night.stage ^= 1;
        assert_ne!(jane_sim::save::hash_of(&jane_sim::save::Form::of(&c, s.blueprints())), hash, "the stage is hashed");
    }
}

/// §2.5: the stage is the party's. Four seats, one night, the same in every view.
#[test]
fn co_op_seats_share_the_stage() {
    let mut s = party(4);
    set_flag(&mut s, "mine_quiet", 1);
    step_onto(&mut s, 21 * H);
    let views: Vec<Night> = (0..4).map(|i| s.view(Seat(i)).expect("a seat").night()).collect();
    assert!(views.iter().all(|&n| n == Night { stage: 2, turned_at: s.state().tick }), "{views:?}");
    // A seat that joins at night joins the night as it is, with no turn.
    cmd(&mut s, Some(0), Command::Open(true));
    cmd(&mut s, None, Command::Join { who: ClientToken(9) });
    assert_eq!(s.view(Seat(0)).unwrap().night(), night(&s));
}

/// Two worlds stepped alike across the turn hash alike at every step; the stage moves the hash.
#[test]
fn the_turn_is_deterministic() {
    let mut a = field();
    let mut b = field();
    for s in [&mut a, &mut b] {
        set_flag(s, "works_dark", 1);
        s.state_mut().clock = 21 * H - 30;
    }
    for _ in 0..60 {
        a.step(&StepInput::IDLE);
        b.step(&StepInput::IDLE);
        assert_eq!(a.hash(), b.hash());
    }
    assert_eq!(night(&a).stage, 2);
}

/// A field whose county has one lamp post at (20, 10), ranked to go wrong from N2.
fn lamp_field() -> Sim {
    let cat = jane_data::catalog();
    let lamp = cat.story.prop_id("lamp_post").expect("a lamp post row");
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        if z == ZoneId::County {
            let mut row = PropSpawn::new(Key::Name(cat.name_id("start").unwrap()), lamp, Cell::new(20, 10));
            row.on = true;
            bp.props.push(row);
            bp.shield = vec![(0, 2)];
        }
        Arc::new(bp)
    });
    let s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    let _ = start_sym();
    s
}

/// §5.4: warm is safe. A lamp the shield has left is lit (to see by) but not warm (no safety)
/// from its stage on; by day and at shallower stages it is kept.
#[test]
fn a_wrong_lamp_lights_but_does_not_keep() {
    let mut s = lamp_field();
    let at = Vec2::centre(21, 11);
    let lit = |s: &Sim, warm: bool| {
        let st = s.state();
        let (z, rt) = (st.zone(ZoneId::County).unwrap(), s.runtime(ZoneId::County).unwrap());
        jane_sim::light::lit_at(z, rt, st.clock, at, warm.then_some(st.night.stage))
    };
    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s).stage, 1);
    assert!(lit(&s, false) && lit(&s, true), "kept at N1");
    set_flag(&mut s, "mine_quiet", 1);
    step_onto(&mut s, 6 * H);
    step_onto(&mut s, 21 * H);
    assert_eq!(night(&s).stage, 2);
    assert!(lit(&s, false), "a wrong lamp still lights");
    assert!(!lit(&s, true), "and keeps nothing off");
}

/// The PSP's way (PORT.md §13.3): blueprints built on demand and packed. The night is the same
/// there as over every blueprint held, at the same tick, with the same hash.
#[test]
fn the_packed_on_demand_world_gets_the_same_stage() {
    let seed = 3;
    let full = Blueprints::build(seed).expect("builds");
    let lazy = Blueprints::on_demand_with(seed, Arc::new(Build { packed: true, load: None }), &mut |_| {})
        .expect("the county builds");
    let mut a = Sim::new_game_with(full, "Jane");
    let mut b = Sim::new_game_with(lazy, "Jane");
    for s in [&mut a, &mut b] {
        set_flag(s, "mine_quiet", 1);
        cmd(s, Some(0), Command::Dev(jane_sim::DevOp::Time { hour: 22 }));
        steps(s, 3);
    }
    assert_eq!(night(&a), night(&b));
    assert_eq!(night(&a).stage, 2);
    assert_eq!(a.hash(), b.hash());
    let (va, vb) = (a.view(Seat(0)).unwrap(), b.view(Seat(0)).unwrap());
    let (ma, mb) = (va.night_map(), vb.night_map());
    for y in (0..2000).step_by(37) {
        for x in (0..2000).step_by(41) {
            assert_eq!(ma.intensity(2, x, y), mb.intensity(2, x, y), "({x}, {y})");
        }
    }

    // §3.2 by place: Julie's ground never turns, a hub's is a stage behind, the Works a stage on.
    let bp = a.blueprints().get(ZoneId::County).clone();
    let mid = |r: Rect| (r.x + r.w / 2, r.y + r.h / 2);
    for stage in 1..=4u8 {
        let (x, y) = mid(site(&bp, "site_julie_house"));
        assert_eq!(ma.intensity(stage, x, y), 0, "Julie's house at N{stage}");
        for hub in ["site_town", "site_canteen", "site_reed_camp", "site_station"] {
            let (x, y) = mid(site(&bp, hub));
            assert_eq!(ma.intensity(stage, x, y), stage - 1, "{hub} at N{stage}");
        }
        let (x, y) = mid(site(&bp, "site_graveyard"));
        assert_eq!(bp.regions.region_at(x, y), Some(2), "the graveyard is the Works'");
        assert_eq!(ma.intensity(stage, x, y), (stage + 1).min(4), "the Works at N{stage}");
    }
    assert_eq!(ma.intensity(0, 5, 5), 0, "by day nothing");
}

/// A generated site's ground.
fn site(bp: &Blueprint, name: &str) -> Rect {
    bp.rects
        .iter()
        .find_map(|(k, r)| match *k {
            Key::Local(i) => (bp.local_names.get(i as usize) == Some(name)).then_some(*r),
            Key::Name(_) => None,
        })
        .unwrap_or_else(|| panic!("no {name}"))
}
