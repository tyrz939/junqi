//! The night's turn on screen (NIGHT.md §2.2, §2.6, §9 R2): one timeline, a pure function of
//! `tick - turned_at` and the stage, handed alike to every tier (the PC's three and the PSP's
//! console presenter); a save loaded mid-turn continues it.

use jane_present::turn::{Kind, Look, Room, look};
use jane_present::{Band, Frame, Pass, Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::tuning::TICKS_PER_HOUR;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const S: u32 = jane_core::num::TICK_RATE;

/// Seed 1 in the town square, the clock a second short of nine.
fn before_nine() -> Sim {
    let mut sim = Sim::new_game(1, "Jane");
    let mark = sim.view(Seat(0)).and_then(|v| v.sym("town_square")).expect("the square's mark");
    let tp = [StampedCommand {
        seat: Some(Seat(0)),
        seq: 1,
        cmd: Command::Dev(DevOp::Tp { zone: jane_core::ZoneId::County, mark }),
    }];
    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &tp });
    for _ in 0..30 {
        sim.step(&StepInput::IDLE);
    }
    assert_eq!(sim.view(Seat(0)).unwrap().zone(), jane_core::ZoneId::County, "in the county");
    sim.state_mut().clock = 21 * TICKS_PER_HOUR - S;
    sim
}

fn step(sim: &mut Sim, ps: &mut [&mut Present]) {
    sim.step(&StepInput::IDLE);
    let events = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    for p in ps {
        p.tick(&v, &events);
    }
}

fn band(f: &Frame) -> Band {
    f.passes
        .iter()
        .find_map(|p| match p {
            Pass::Lights { band, .. } => Some(*band),
            _ => None,
        })
        .unwrap_or(Band::NONE)
}

/// What a frame hands its tier of the turn: the light pass's band, and each light's colour by
/// where it stands (a tier's count of lights is its own; the colour of the same light is not).
/// Each light's ground point and colour.
type Lit = Vec<((i32, i32), [u8; 3])>;

fn handed(f: &Frame) -> (Band, Lit) {
    let mut l: Vec<_> = f.lights.iter().map(|l| (l.pos, l.colour)).collect();
    l.sort_unstable();
    (band(f), l)
}

#[test]
fn every_tier_and_the_console_are_handed_the_same_turn_at_the_same_tick() {
    let mut sim = before_nine();
    let mut t0 = Present::new(Tier::T0);
    let mut t1 = Present::new(Tier::T1);
    let mut t2 = Present::new(Tier::T2);
    let mut c2 = Present::from_tables_console(Tier::T0, &t0.tables(), 12).unwrap();
    let canvas = (480, 272);
    let mut seen = Vec::new();
    for t in 0..(6 * S) {
        step(&mut sim, &mut [&mut t0, &mut t1, &mut t2, &mut c2]);
        let v = sim.view(Seat(0)).unwrap();
        let night = v.night();
        let since = v.tick().0.wrapping_sub(night.turned_at.0);
        let want = if night.stage > 0 && since < 4 * S {
            look(Kind::Bell, Room::Open, since, night.stage, canvas.1)
        } else {
            Look::NONE
        };
        let a = handed(t0.draw(0, canvas));
        assert_eq!(t0.turn_now(), want, "tick {t}: the turn is the timeline's at tick - turned_at");
        for (name, p) in [("T1", &mut t1), ("T2", &mut t2), ("C2", &mut c2)] {
            let b = handed(p.draw(0, canvas));
            assert_eq!(a.0, b.0, "tick {t}: {name}'s band");
            // The same light, the same colour, whichever tier draws it.
            for l in &b.1 {
                if let Some(m) = a.1.iter().find(|m| m.0 == l.0) {
                    assert_eq!(m.1, l.1, "tick {t}: {name}'s light at {:?}", l.0);
                }
            }
        }
        seen.push(a.0);
    }
    // It turned: the whole screen dipped, then the band came down, then it was over.
    assert!(seen.iter().any(|b| b.dark == 51 && b.at(0) == 51), "the held dark");
    assert!(seen.iter().any(|b| b.at(0) == 256 && b.at(271) < 256), "the band from the north");
    assert_eq!(*seen.last().unwrap(), Band::NONE, "over by 21:00:04");
}

#[test]
fn a_save_loaded_mid_turn_continues_it() {
    let mut sim = before_nine();
    let mut a = Present::new(Tier::T0);
    // Into the turn: 21:00:01, the held dark.
    for _ in 0..(2 * S) {
        step(&mut sim, &mut [&mut a]);
    }
    let night = sim.view(Seat(0)).unwrap().night();
    assert!(night.stage == 1 && sim.state().tick.0 - night.turned_at.0 == S, "{night:?}");
    let (bytes, _) = sim.save_and_hash();
    let mut loaded = Sim::from_snapshot_with(&bytes, sim.blueprints().clone()).expect("loads");
    let mut b = Present::new(Tier::T0);
    let canvas = (640, 360);
    // The loaded game's presenter has seen one tick of it; the first has seen the whole turn.
    step(&mut loaded, &mut [&mut b]);
    step(&mut sim, &mut [&mut a]);
    for t in 0..(4 * S) {
        assert_eq!(a.turn_now(), b.turn_now(), "tick {t} after the load");
        let (fa, fb) = (handed(a.draw(0, canvas)), handed(b.draw(0, canvas)));
        assert_eq!(fa.0, fb.0, "tick {t}: the band");
        step(&mut sim, &mut [&mut a]);
        step(&mut loaded, &mut [&mut b]);
    }
    assert_eq!(b.turn_now(), Look::NONE);
}
