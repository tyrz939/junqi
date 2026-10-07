//! The presenter's tables (`JPT1`, PORT.md §13.12): a presenter built from them, as a console
//! boots, draws the same `Frame` as one built by the generators, scene for scene.

use jane_core::Angle;
use jane_present::{Frame, Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{Blueprints, Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

/// Everything a backend reads from a frame, as text (the chunk layers by their albedo).
fn seen(f: &Frame, slots: &mut Vec<Vec<u32>>) -> String {
    slots.clear();
    for c in &f.chunks {
        slots.push(f.layers[usize::from(c.slot)].albedo.clone());
    }
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        f.tier,
        f.canvas,
        f.camera,
        f.clear,
        f.passes,
        f.chunks,
        f.sprites,
        f.lights,
        f.casters,
        f.blocks,
        f.water,
        f.fog,
        f.parts,
        f.stars,
        f.tick
    )
}

/// Steps one sim and two presenters together through `ticks`, walking her by `walk`, with the
/// clock set to `hour` first; the frames must match at every `every`th tick.
fn same_frames(a: &mut Present, b: &mut Present, hour: u8, walk: &[(Option<Angle>, u32)], every: u32) {
    same_frames_over(a, b, None, hour, walk, every);
}

/// As [`same_frames`]; with `packed`, `b` is shown a second sim over those (packed) blueprints.
fn same_frames_over(
    a: &mut Present,
    b: &mut Present,
    packed: Option<Blueprints>,
    hour: u8,
    walk: &[(Option<Angle>, u32)],
    every: u32,
) {
    let mut sim = Sim::new_game(1, "Jane");
    let mut other = packed.map(|bps| Sim::new_game_with(bps, "Jane"));
    let cmd = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour }) }];
    let mut k = 0u32;
    let (mut sa, mut sb) = (Vec::new(), Vec::new());
    for &(dir, n) in walk {
        for _ in 0..n {
            let cmds: &[StampedCommand] = if k == 0 { &cmd } else { &[] };
            let frame = dir.map_or(InputFrame::IDLE, InputFrame::walk);
            let input =
                StepInput { frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands: cmds };
            sim.step(&input);
            let events = sim.drain_events().to_vec();
            let v = sim.view(Seat(0)).expect("seat 0 plays");
            a.tick(&v, &events);
            match &mut other {
                Some(o) => {
                    o.step(&input);
                    let theirs = o.drain_events().to_vec();
                    assert!(theirs == events, "tick {k}: the packed sim's events differ");
                    b.tick(&o.view(Seat(0)).expect("seat 0 plays"), &theirs);
                }
                None => b.tick(&v, &events),
            }
            k += 1;
            if k % every == 0 {
                let fa = seen(a.draw(128, (480, 272)), &mut sa);
                let fb = seen(b.draw(128, (480, 272)), &mut sb);
                assert!(fa == fb, "tick {k}: the frames differ");
                assert!(sa == sb, "tick {k}: the chunks differ");
            }
        }
    }
}

#[test]
fn the_presenter_from_its_tables_draws_what_the_generators_draw() {
    let mut a = Present::new(Tier::T0);
    let bytes = a.tables();
    let mut b = Present::from_tables(Tier::T0, &bytes).expect("the tables read back");
    // The tables read back to the same bytes, and the sprite table is the atlas's.
    assert!(b.tables() == bytes, "the tables do not round-trip");
    assert_eq!(a.sprites().refs, b.sprites().refs);
    assert_eq!(a.ui_art().page, b.ui_art().page);
    // The town by day, walking east and south, then by night.
    same_frames(
        &mut a,
        &mut b,
        12,
        &[(None, 30), (Some(Angle::EAST), 60), (Some(Angle::SOUTH), 60), (Some(Angle::from_degrees(225)), 40)],
        10,
    );
    let mut a = Present::new(Tier::T0);
    let mut b = Present::from_tables(Tier::T0, &bytes).unwrap();
    same_frames(&mut a, &mut b, 22, &[(None, 20), (Some(Angle::from_degrees(30)), 60)], 20);
}

#[test]
fn a_short_or_long_pack_is_refused() {
    let bytes = Present::new(Tier::T0).tables();
    assert!(Present::from_tables(Tier::T0, &bytes[..bytes.len() - 1]).is_err());
    let mut long = bytes.clone();
    long.push(0);
    assert!(Present::from_tables(Tier::T0, &long).is_err());
    assert!(Present::from_tables(Tier::T0, b"JAT1").is_err());
}

/// A console's sim runs over packed blueprints (PORT.md §13.3): the presenter reads the paint from
/// the packed plane and draws the same frames.
#[test]
fn packed_blueprints_draw_the_same_frames() {
    let mut a = Present::new(Tier::T0);
    let mut b = Present::new(Tier::T0);
    let bps = Blueprints::build(1).expect("seed 1 builds").packed();
    same_frames_over(&mut a, &mut b, Some(bps), 12, &[(None, 10), (Some(Angle::from_degrees(300)), 90)], 15);
}
