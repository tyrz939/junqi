//! The terrain painter behind the presenter (PRESENTATION.md §1.6), from a real New Game: at
//! most two chunks land a tick once she is in the zone, and the flora stand in the draw list.

use jane_core::Angle;
use jane_present::{Pass, Present, Tier};
use jane_sim::{InputFrame, Seat, Sim, StepInput};

fn tick(sim: &mut Sim, p: &mut Present, input: InputFrame) {
    sim.step(&StepInput::solo(input));
    let events = sim.drain_events().to_vec();
    let v = sim.view(Seat(0)).expect("seat 0 plays");
    p.tick(&v, &events);
}

#[test]
fn at_most_two_chunks_land_a_tick_once_she_is_in() {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    tick(&mut sim, &mut p, InputFrame::IDLE);
    // The tick she arrives paints what the view shows outright.
    let first = p.chunks_landed();
    assert!(first >= 4, "{first} chunks on the first tick");
    // Then she walks, east then south, and the view keeps finding new ground.
    let mut last = first;
    for t in 0..900 {
        let dir = if t < 450 { Angle::EAST } else { Angle::SOUTH };
        tick(&mut sim, &mut p, InputFrame { mv_dir: dir, mv_mag: 127, sprint: true, ..InputFrame::IDLE });
        let now = p.chunks_landed();
        assert!(now - last <= 2, "{} chunks landed at tick {t}", now - last);
        last = now;
    }
    assert!(last > first, "she walked into no new ground");
}

#[test]
fn the_view_is_painted_and_its_flora_stand_among_the_units() {
    let mut sim = Sim::new_game(1, "Jane");
    let mut p = Present::new(Tier::T0);
    for _ in 0..60 {
        tick(&mut sim, &mut p, InputFrame::IDLE);
    }
    let (units, props) = p.seen();
    let landed = p.chunks_landed() as usize;
    let f = p.draw(0, (768, 432));
    let standing = f
        .passes
        .iter()
        .find_map(|pass| match *pass {
            Pass::Sprites { layer: jane_present::Depth::Standing, cmds } => Some(cmds.len),
            _ => None,
        })
        .expect("a standing pass");
    // More stands than the units and props the view holds: the trees, shrubs and stones.
    assert!(standing as usize > units + props, "{standing} standing, {units} units, {props} props");
    // Every chunk the frame draws was painted by the painter, not left as swatches.
    assert!(!f.chunks.is_empty());
    assert!(landed >= f.chunks.len(), "{landed} landed, {} drawn", f.chunks.len());
}
