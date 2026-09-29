//! The Factory's press hall on the real seed (DUNGEONS.md §3.4, "Can she get stuck?"): the call
//! box is the only way the patched wall comes down, and the wall is the only way to the
//! foreman's office. A hauler she has killed, or one that gave up on the walk, must not leave the
//! box spent with the wall still standing: the box rings again until the wall is down, and a
//! hauler stands up again.

mod common;

use common::bot::*;
use common::new_game;
use jane_core::action::Facing;
use jane_core::angle::iatan2;
use jane_core::{Milli, ZoneId};
use jane_sim::event::{EventKind, ToastKind};
use jane_sim::input::DevOp;
use jane_sim::state::FlagKey;
use jane_sim::{Command, InputFrame, Seat, Sim, StampedCommand, StepInput};

fn toasts(sim: &mut Sim) -> Vec<String> {
    let cat = jane_data::catalog();
    events(sim)
        .iter()
        .filter_map(|e| match e.kind {
            EventKind::Toast(ToastKind::Text(jane_core::TextRef::Text(t))) => Some(cat.text(t).to_owned()),
            _ => None,
        })
        .collect()
}

/// Stand beside the call box, on the nearest free cell below or beside it.
fn stand_by_the_box(s: &mut Sim) {
    let c = prop_centre(&prop(s, "factory_call_box"));
    let (x, y) = c.cell();
    let rt = s.runtime(ZoneId::Factory).expect("the factory");
    let (fx, fy) = rt.grid.nearest_free(x, y + 1, 4, None).expect("room by the box");
    place(s, fx, fy, Facing::North);
}

/// Spark the call box from where she stands.
fn spark_the_box(s: &mut Sim) {
    let c = prop_centre(&prop(s, "factory_call_box"));
    let at = me(s).pos;
    let aim = iatan2(c.y.0 - at.y.0, c.x.0 - at.x.0);
    let cmds = [StampedCommand {
        seat: Some(Seat(0)),
        seq: 0,
        cmd: Command::Cast { spell: jane_data::catalog().combat.spell_id("spark").expect("spark"), on: None },
    }];
    let mut frames = [InputFrame::IDLE; 4];
    frames[0] = InputFrame { aim: Some(aim), ..InputFrame::IDLE };
    s.step(&StepInput { frames, commands: &cmds });
    idle(s, 30);
}

fn wall_down(s: &Sim) -> bool {
    let f = FlagKey::Named(sym(s, "factory_wall_down"));
    s.state().flags.get(&f).is_some_and(|&v| v != 0)
}

#[test]
fn a_dead_hauler_leaves_the_call_box_to_ring_again_and_it_stands_up_to_answer() {
    let mut s = new_game();
    let entry = sym(&s, "entry");
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Factory, mark: entry }));
    idle(&mut s, 2);
    assert_eq!(zone_of(&s), ZoneId::Factory);
    cmd(&mut s, Command::Dev(DevOp::God(true)));
    cmd(&mut s, Command::Dev(DevOp::Learn(jane_data::catalog().combat.spell_id("spark").expect("spark"))));

    // She kills the press hall's hauler.
    let hauler = unit(&s, "factory_press_hauler");
    let (hx, hy) = hauler.pos.cell();
    {
        let z = s.state_mut().zone_mut(ZoneId::Factory).expect("the factory");
        let u = z.unit_mut(hauler.id).expect("the hauler");
        u.hp = Milli::from_points(1);
        u.awake = true;
    }
    s.rebuild_runtimes();
    let rt = s.runtime(ZoneId::Factory).expect("the factory");
    let (fx, fy) = rt.grid.nearest_free(hx + 2, hy, 4, None).expect("room by the hauler");
    place(&mut s, fx, fy, Facing::West);
    for _ in 0..10 {
        cmd(&mut s, Command::Dev(DevOp::Kill));
        idle(&mut s, 2);
        if !unit(&s, "factory_press_hauler").alive {
            break;
        }
    }
    assert!(!unit(&s, "factory_press_hauler").alive, "the hauler is down");

    // The box rings, nothing comes, and it says so; the wall stands and the box can ring again.
    stand_by_the_box(&mut s);
    events(&mut s);
    spark_the_box(&mut s);
    let said = toasts(&mut s);
    assert!(said.iter().any(|t| t.contains("nothing comes")), "{said:?}");
    assert!(!wall_down(&s));
    assert!(!prop(&s, "factory_call_box").on, "the box is ready to ring again");
    assert!(!prop(&s, "factory_weak_wall").hidden);

    // Away a while, out of its sight: it stands up again.
    cmd(&mut s, Command::Dev(DevOp::Tp { zone: ZoneId::Factory, mark: entry }));
    for _ in 0..60 * 60 * 4 {
        s.step(&StepInput::IDLE);
        if unit(&s, "factory_press_hauler").alive {
            break;
        }
    }
    assert!(unit(&s, "factory_press_hauler").alive, "the hauler stands up again");

    // Rung again, it answers: the wall comes down and the box stays rung.
    stand_by_the_box(&mut s);
    events(&mut s);
    spark_the_box(&mut s);
    let said = toasts(&mut s);
    assert!(said.iter().any(|t| t.contains("starts walking")), "{said:?}");
    for _ in 0..60 * 120 {
        s.step(&StepInput::IDLE);
        if wall_down(&s) {
            break;
        }
    }
    assert!(wall_down(&s), "the hauler walks through the wall");
    assert!(prop(&s, "factory_weak_wall").hidden);
    assert!(prop(&s, "factory_call_box").on, "rung for good once the wall is down");
}
