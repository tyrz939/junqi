//! ART-PLAN §7 rule 5, "something moves": every outdoor frame by day has at least one living
//! thing that is not a person (a bird, a duck, a crow, a cat, a butterfly, smoke). And the
//! ambient layer's other promises: its caps by tier, its birds fly off when she comes near, the
//! empty houses do not smoke.

use jane_present::{Present, Tier};
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

const CANVAS: (u16, u16) = (768, 432);

/// A new game on `seed`, her at county mark `mark` at `hour`, the presenter at `tier` ticked
/// beside it for `ticks` after she lands; and the fewest living things it showed on any of the
/// last `ticks` ticks.
fn at(seed: u32, mark: &str, hour: u8, tier: Tier, ticks: u32) -> Option<(Present, usize)> {
    let mut sim = Sim::new_game(seed, "Jane");
    let mut p = Present::new(tier);
    p.set_canvas(CANVAS);
    let mark = sim.state().syms.find(mark)?;
    let seat = Some(Seat(0));
    let cmds = [
        StampedCommand { seat, seq: 1, cmd: Command::Dev(DevOp::God(true)) },
        StampedCommand { seat, seq: 2, cmd: Command::Dev(DevOp::Time { hour }) },
        StampedCommand { seat, seq: 3, cmd: Command::Dev(DevOp::Tp { zone: jane_core::ids::ZoneId::County, mark }) },
    ];
    let mut fewest = usize::MAX;
    for k in 0..ticks + 20 {
        let cmds: &[StampedCommand] = if k == 0 { &cmds } else { &[] };
        let idle = InputFrame::IDLE;
        sim.step(&StepInput { frames: [idle; 4], commands: cmds });
        let events = sim.drain_events().to_vec();
        let v = sim.view(Seat(0)).expect("seat 0 plays");
        p.tick(&v, &events);
        if k >= 20 {
            fewest = fewest.min(p.ambient().living());
        }
    }
    Some((p, fewest))
}

#[test]
fn something_lives_in_every_outdoor_frame_by_day() {
    let marks = ["town_square", "pound_lane", "reed_camp_gate", "hedge_stile_farm", "lake_bank", "yard_gate"];
    // One game and one presenter a seed (an atlas is dear in a dev build); she is sent from
    // place to place and hour to hour, and every tick of each stretch is counted, not one frame.
    for seed in 1..=2 {
        let mut sim = Sim::new_game(seed, "Jane");
        let mut p = Present::new(Tier::T0);
        p.set_canvas(CANVAS);
        let seat = Some(Seat(0));
        let mut seq = 1;
        let god = [StampedCommand { seat, seq: 0, cmd: Command::Dev(DevOp::God(true)) }];
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &god });
        for mark in marks {
            let Some(at) = sim.state().syms.find(mark) else { continue };
            for hour in [8, 12, 16] {
                let cmds = [
                    StampedCommand { seat, seq, cmd: Command::Dev(DevOp::Time { hour }) },
                    StampedCommand {
                        seat,
                        seq: seq + 1,
                        cmd: Command::Dev(DevOp::Tp { zone: jane_core::ids::ZoneId::County, mark: at }),
                    },
                ];
                seq += 2;
                let mut fewest = usize::MAX;
                for k in 0..80 {
                    let cmds: &[StampedCommand] = if k == 0 { &cmds } else { &[] };
                    sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: cmds });
                    let events = sim.drain_events().to_vec();
                    let v = sim.view(Seat(0)).expect("seat 0 plays");
                    p.tick(&v, &events);
                    if k >= 20 {
                        fewest = fewest.min(p.ambient().living());
                    }
                }
                assert!(fewest >= 1, "seed {seed} at {mark} {hour}:00: nothing lives on screen");
            }
        }
    }
}

#[test]
fn each_tier_keeps_to_its_cap() {
    for (tier, cap) in [(Tier::T0, 12), (Tier::T1, 24), (Tier::T2, 48)] {
        let (p, _) = at(1, "town_square", 11, tier, 60).expect("the square");
        // Smoke's five puffs are one actor; lily pads lie flat and are not counted.
        let a = p.ambient().actors();
        let mut plumes: Vec<u32> = a.iter().filter(|a| a.key & 0x10_0000 != 0).map(|a| a.key >> 3).collect();
        plumes.sort_unstable();
        plumes.dedup();
        let birds = a.iter().filter(|a| !a.flat && a.key & 0x10_0000 == 0).count();
        assert!(birds + plumes.len() <= cap, "{tier:?}: {} over {cap}", birds + plumes.len());
    }
}
