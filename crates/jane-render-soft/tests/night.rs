//! The night's look through `soft` (NIGHT.md §4.2, §9.1): a frame at every stage draws, and its
//! sprites read the night's CLUT (a frame's own colours change, not just the grade's).

use jane_present::{Backend, Present, Tier};
use jane_render_soft::Soft;
use jane_sim::input::DevOp;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

#[test]
fn every_stage_draws_through_soft_and_the_sprites_take_the_night() {
    let mut last: Option<Vec<u32>> = None;
    for flags in
        [&[][..], &["mine_quiet"], &["mine_quiet", "works_dark"], &["mine_quiet", "works_dark", "burial_quiet"]]
    {
        let mut sim = Sim::new_game(1, "Jane");
        for f in flags {
            if let Some(c) = jane_data::catalog().living.consequence_id(f) {
                sim.state_mut().consequences_done.set(u32::from(c.0), true);
            }
            if let Some(k) = sim.state().syms.find(f) {
                sim.state_mut().flags.insert(jane_sim::state::FlagKey::Named(k), 1);
            }
        }
        let mut p = Present::new(Tier::T0);
        let time = [StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Dev(DevOp::Time { hour: 22 }) }];
        sim.step(&StepInput { frames: [InputFrame::IDLE; 4], commands: &time });
        for _ in 0..90 {
            sim.step(&StepInput::IDLE);
            let events = sim.drain_events().to_vec();
            p.tick(&sim.view(Seat(0)).unwrap(), &events);
        }
        let mut soft = Soft::new();
        soft.upload_atlas(p.atlas());
        let f = p.draw(0, (640, 360));
        let stage = sim.view(Seat(0)).unwrap().night().stage;
        assert_eq!(usize::from(stage), flags.len() + 1, "the stage latched");
        soft.draw(f);
        let px = soft.pixels().0.to_vec();
        if let Some(l) = &last {
            assert_ne!(l, &px, "stage {stage}: the night deepened on screen");
        }
        last = Some(px);
    }
}
