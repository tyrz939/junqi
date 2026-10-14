//! Sixteen hostiles after her at the Castle Viaduct, over its deck and the terrace edge under it
//! (the benchmark's `mob` scene, `jane-present/src/bench.rs`): the PSP's sim step took about
//! 300 ms there, again and again. A crowd round her on the terraces asks for paths it cannot have
//! (the cells by her are bodies, or across a ledge), and each such search opened all of a search's
//! budget, two of them in a tick. The zone's searches now share a hard bound of nodes a tick
//! ([`PATH_TICK_BUDGET`]): no search opens more than is left of it, and one cut short by it is
//! refused, not failed, and goes first the next tick.
//!
//! What must hold: no tick's searches open more than the bound (and a leg's overshoot), and the
//! mob still chases and fights her.

use jane_core::{Angle, Vec2, ZoneId};
use jane_sim::path::PATH_TICK_BUDGET;
use jane_sim::state::TravelRequest;
use jane_sim::{Blueprints, Command, DevOp, EventKind, InputFrame, Seat, Sim, StampedCommand, StepInput};

/// The benchmark's seed.
const SEED: u32 = 570_944_055;
/// What a tick may open past the bound: a search stops one node over its budget, a chained one
/// once a leg (`path::CHAIN_LEGS`, sixteen), and the last of a tick may start with one node left.
const OVERSHOOT: u32 = 32;

fn step(s: &mut Sim, f: InputFrame, c: Option<Command>) {
    let cmds: Vec<StampedCommand> =
        c.into_iter().map(|cmd| StampedCommand { seat: Some(Seat(0)), seq: 0, cmd }).collect();
    let mut frames = [InputFrame::IDLE; 4];
    frames[0] = f;
    s.step(&StepInput { frames, commands: &cmds });
}

#[test]
fn a_mob_on_the_viaduct_never_opens_more_than_the_ticks_nodes() {
    let bps = Blueprints::build(SEED).expect("the bench's seed builds");
    let mut s = Sim::new_game_with(bps, "Jane");
    let mark = s.view(Seat(0)).and_then(|v| v.sym("start")).expect("the county's start");
    s.state_mut().players[0].travel =
        Some(TravelRequest { zone: ZoneId::County, mark, at: Some(Vec2::centre(940, 948)) });
    for _ in 0..5 {
        step(&mut s, InputFrame::IDLE, None);
    }
    assert_eq!(s.state().players[0].zone, ZoneId::County, "she is at the viaduct");
    let cat = &jane_data::catalog().combat;
    let mob = ["skeleton"; 8].into_iter().chain(["night_hound"; 4]).chain(["bandit"; 4]);
    for name in mob {
        let def = cat.unit_id(name).unwrap_or_else(|| panic!("no unit {name}"));
        step(&mut s, InputFrame::IDLE, Some(Command::Dev(DevOp::Spawn(def))));
    }
    let her = s.state().players[0].unit;
    let (mut worst, mut nodes, mut hits) = (0u32, 0u64, 0u32);
    for t in 0..1800u32 {
        // Back and forth along the deck and off its end, a lap each way every three seconds.
        let dir = if (t / 90) % 2 == 0 { Angle::EAST } else { Angle::WEST };
        let f = if t % 360 < 300 { InputFrame::walk(dir) } else { InputFrame::IDLE };
        // Her health filled every ten ticks (the bench's `tough`), so the mob does not end it.
        step(&mut s, f, (t % 10 == 0).then_some(Command::Dev(DevOp::Hp(i32::MAX))));
        let m = s.metrics();
        worst = worst.max(m.path_expanded);
        nodes += u64::from(m.path_expanded);
        hits +=
            s.drain_events().iter().filter(|e| matches!(e.kind, EventKind::Damage { unit, .. } if unit == her)).count()
                as u32;
    }
    println!("worst tick {worst} nodes, {nodes} in all, {hits} blows on her; {:?}", s.path_stats());
    assert!(
        worst <= PATH_TICK_BUDGET + OVERSHOOT,
        "a tick opened {worst} nodes, the bound is {PATH_TICK_BUDGET} and {OVERSHOOT} over"
    );
    assert!(hits >= 60, "the mob still fights her: {hits} blows landed");
}
