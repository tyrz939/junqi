//! No allocation in the tick after warm-up, across a span's layers (ARCHITECTURE.md §9; MAP.md
//! §9.1), counted by `cap`'s allocator, this test binary's own (its one test). On the `viaduct`
//! she crosses the deck and back for ever, a free bolt along it now and then, while one chaser
//! follows her onto the deck and another walks the towpath under it after her.

mod field;

use std::sync::Arc;

use field::{Z, body_of, cmd, edit, spawn, spell};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Angle, Blueprint, Cell, Key, Tile, Vec2, ZoneId};
use jane_sim::span::viaduct as V;
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput};

#[global_allocator]
static ALLOC: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);

fn world() -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            return Arc::new(V::blueprint(z));
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Learn(spell("spark"))));
    let me = body_of(&s, 0);
    edit(&mut s, me, |u| u.pos = Vec2::centre(31, 14));
    for (x, y) in [(36, 10), (20, 24)] {
        let f = spawn(&mut s, "quarryman", x, y);
        edit(&mut s, f, |u| {
            u.strength = 2000;
            u.hp = jane_sim::units::max_hp(u);
            u.target = Some(me);
            u.combat = CombatState::Combat;
        });
    }
    s
}

fn tick(s: &mut Sim, f: u32) {
    let dir = if (f / 200) % 2 == 0 { Angle::SOUTH } else { Angle::NORTH };
    let cast = [
        StampedCommand { seat: Some(Seat(0)), seq: 0, cmd: Command::Dev(DevOp::Mp(100)) },
        StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Cast { spell: spell("spark"), on: None } },
    ];
    let commands: &[StampedCommand] = if f % 90 == 45 { &cast } else { &[] };
    let frame = InputFrame { aim: Some(dir), ..InputFrame::walk(dir) };
    s.step(&StepInput { frames: [frame, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands });
    s.drain_events();
}

#[test]
fn the_tick_allocates_nothing_after_warm_up_across_spans() {
    let mut s = world();
    let decked = |s: &Sim| s.state().zone(Z).is_some_and(|z| z.units.iter().any(|u| u.on_span.is_some()));
    let mut on = 0;
    for f in 0..2400 {
        tick(&mut s, f);
        on += u32::from(decked(&s));
    }
    assert!(on > 0, "the loop crosses the deck");
    let before = ALLOC.total_allocated();
    let mut on = 0;
    for f in 2400..4800 {
        tick(&mut s, f);
        on += u32::from(decked(&s));
    }
    let grown = ALLOC.total_allocated() - before;
    assert!(on > 0, "on the deck while counted");
    assert_eq!(grown, 0, "bytes allocated in 2400 ticks after warm-up");
}
