//! A deck is a layer of its own: a body in the middle of one is planned from the deck's ends,
//! not from the water under it (the Library bridge, the reader bot's one long stall).

mod common;

use jane_bot::nav::{Go, Nav};
use jane_core::{Fx, Vec2, ZoneId};
use jane_sim::{Seat, StepInput};

#[test]
fn a_body_mid_deck_walks_the_deck_off() {
    let mut sim = common::new_game(1);
    // The Library bridge: a deck of the county, x 965..967, y 174..192, the river under its south
    // half. She is put on it at (965, 187), just where the bot stalled.
    let id = sim.state().players[0].unit;
    {
        let u = sim.state_mut().zone_mut(ZoneId::County).unwrap().unit_mut(id).unwrap();
        u.pos = Vec2::centre(965, 187);
        u.on_span = Some(2);
    }
    sim.rebuild_runtimes();
    let goal = Vec2::centre(966, 170);
    let mut nav = Nav::default();
    for _ in 0..1800 {
        let v = sim.view(Seat(0)).unwrap();
        let go = nav.go(&v, goal, Fx::from_cells(1), false);
        let f = match go {
            Go::Walk(f) => f,
            Go::Arrived => break,
            Go::NoWay => panic!("no way off the deck"),
        };
        sim.step(&StepInput::solo(f));
    }
    let v = sim.view(Seat(0)).unwrap();
    let (_, y) = v.body().pos.cell();
    assert!(y <= 173, "still on the deck at y {y}");
}
