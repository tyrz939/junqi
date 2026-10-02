//! Knockback and the puzzles (`feel.rs`, PLAY-PLAN.md §2.1): a push never carries a body onto a
//! plate (plates read only who walks onto them), and walking onto one still presses it.

mod common;

use jane_core::Angle;
use jane_core::action::Facing;
use jane_sim::{Command, InputFrame};

use common::bot::*;
use common::room::Room;

#[test]
fn her_swing_never_pushes_a_foe_onto_a_plate() {
    let mut r = Room::new(false);
    let gate = r.prop("gate", "gate_h", 30, 8, |s| s.locked = true);
    let open = r.list(vec![jane_core::Action::Unlock(gate)]);
    r.prop("plate", "plate", 22, 16, |s| s.use_list = Some(open));
    r.unit("skel", "skeleton", 21, 16);
    let mut s = r.build();
    place(&mut s, 20, 16, Facing::East);
    idle(&mut s, 2);
    let melee = jane_data::catalog().combat.spell_id("melee_player").unwrap();
    let before = unit(&s, "skel").pos;
    for _ in 0..3 {
        step(&mut s, InputFrame { aim: Some(Angle::EAST), ..InputFrame::IDLE });
        cmd(&mut s, Command::Cast { spell: melee, on: None });
        idle(&mut s, 80);
        place(&mut s, 20, 16, Facing::East);
    }
    let after = unit(&s, "skel");
    assert!(after.pos.cell().0 < 22, "pushed toward the plate, it stops at its edge: {:?} from {before:?}", after.pos);
    assert!(!prop(&s, "plate").on && prop(&s, "gate").locked, "no push pressed the plate");
}

#[test]
fn her_swing_pushes_a_foe_six_px_along_the_blow() {
    let mut r = Room::new(false);
    r.unit("skel", "skeleton", 21, 16);
    let mut s = r.build();
    place(&mut s, 20, 16, Facing::East);
    idle(&mut s, 2);
    let melee = jane_data::catalog().combat.spell_id("melee_player").unwrap();
    let before = unit(&s, "skel").pos;
    cmd(&mut s, Command::Cast { spell: melee, on: None });
    // Frozen a few ticks, then pushed over four.
    idle(&mut s, 11);
    let after = unit(&s, "skel").pos;
    assert!(after.x.0 - before.x.0 >= 6 * 256, "{before:?} -> {after:?}");
}
