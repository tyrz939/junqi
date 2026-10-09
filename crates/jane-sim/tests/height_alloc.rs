//! No allocation in the tick after warm-up, across height (ARCHITECTURE.md §9; MAP.md §9.1),
//! counted by `cap`'s allocator, this test binary's own (its one test, so nothing else allocates
//! beside it). On the `terraces` she walks a loop for ever (along the plateau, off the ledge in a
//! hop, east below the face, up the ladder) casting free bolts over the edge, while chasers hop
//! after her, hold at the foot and evade. And the tick's cost there against the same ground with
//! no levels (the ledge a gap of road, so she walks the same loop), printed for the report (`cargo test -p jane-sim --release --test height_alloc --
//! --nocapture`).

// The clock here times the steps from outside them, for the report; it never reaches the sim.
#![allow(clippy::disallowed_types)]

mod field;

use std::sync::Arc;
use std::time::Instant;

use field::{Z, body_of, cmd, edit, spawn, spell};
use jane_core::action::Facing;
use jane_core::blueprint::Mark;
use jane_core::{Angle, Blueprint, Cell, Key, Tile, Vec2, ZoneId};
use jane_sim::height::{self, terraces as T};
use jane_sim::state::CombatState;
use jane_sim::{Blueprints, Command, DevOp, InputFrame, Seat, Sim, StampedCommand, StepInput};

#[global_allocator]
static ALLOC: cap::Cap<std::alloc::System> = cap::Cap::new(std::alloc::System, usize::MAX);

fn world(levels: bool) -> Sim {
    let cat = jane_data::catalog();
    let zones = std::array::from_fn(|i| {
        let z = ZoneId::ALL[i];
        if z == Z {
            let mut bp = T::blueprint(z);
            if !levels {
                // The same ground flat: the ledge a gap of road, so her loop is the same walk.
                bp.level = None;
                bp.tiles.fill_rect(T::LEDGE, Tile::Road);
            }
            return Arc::new(bp);
        }
        let mut bp = Blueprint::new(z, 128, 64, Tile::Grass);
        bp.marks.insert(Key::Name(cat.story.start.mark), Mark { cell: Cell::new(10, 10), facing: Some(Facing::East) });
        Arc::new(bp)
    });
    let mut s = Sim::new_game_with(Blueprints::from_parts(7, zones), "Jane");
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    cmd(&mut s, Some(0), Command::Dev(DevOp::Learn(spell("spark"))));
    let me = body_of(&s, 0);
    edit(&mut s, me, |u| u.pos = Vec2::centre(30, 10));
    for (x, y) in [(20, 8), (40, 6), (24, 22), (44, 24), (70, 20), (66, 9)] {
        let f = spawn(&mut s, "quarryman", x, y);
        // Strong enough to outlive her bolts: a death and its respawn are not this test's.
        edit(&mut s, f, |u| {
            u.strength = 2000;
            u.hp = jane_sim::units::max_hp(u);
            u.target = Some(me);
            u.combat = CombatState::Combat;
        });
    }
    s
}

/// Her loop: west along the plateau, south off the ledge, east below the face, north up the
/// ladder (lined up on its middle first).
fn lap(s: &Sim) -> InputFrame {
    let me = body_of(s, 0);
    let u = s.state().zone(Z).and_then(|z| z.unit(me)).expect("her");
    let (x, y) = u.pos.cell();
    let up = y < T::FACE.y;
    let dir = if up {
        if x > 12 { if y > 10 { Angle::NORTH } else { Angle::WEST } } else { Angle::SOUTH }
    } else if y < 20 && x < 58 {
        Angle::SOUTH
    } else {
        let mid = Vec2::centre(60, 0).x.0;
        if u.pos.x.0 < mid - 128 {
            Angle::EAST
        } else if u.pos.x.0 > mid + 128 {
            Angle::WEST
        } else {
            Angle::NORTH
        }
    };
    InputFrame::walk(dir)
}

/// One tick of the loop; a free bolt south over the edge now and then.
fn tick(s: &mut Sim, f: u32) {
    let frame = lap(s);
    let cast = [
        StampedCommand { seat: Some(Seat(0)), seq: 0, cmd: Command::Dev(DevOp::Mp(100)) },
        StampedCommand { seat: Some(Seat(0)), seq: 1, cmd: Command::Cast { spell: spell("spark"), on: None } },
    ];
    let commands: &[StampedCommand] = if f % 90 == 45 { &cast } else { &[] };
    let aim = InputFrame { aim: Some(Angle::SOUTH), ..frame };
    s.step(&StepInput { frames: [aim, InputFrame::IDLE, InputFrame::IDLE, InputFrame::IDLE], commands });
    s.drain_events();
}

#[test]
fn the_tick_allocates_nothing_after_warm_up_across_height() {
    let mut s = world(true);
    let mut hops = 0;
    for f in 0..2400 {
        tick(&mut s, f);
        hops += u32::from(s.state().zone(Z).is_some_and(|z| z.units.iter().any(height::hopping)));
    }
    assert!(hops > 0, "the loop hops the ledge");
    let before = ALLOC.total_allocated();
    let mut air = 0;
    for f in 2400..4800 {
        tick(&mut s, f);
        air += u32::from(s.state().zone(Z).is_some_and(|z| z.units.iter().any(height::hopping)));
    }
    let grown = ALLOC.total_allocated() - before;
    assert!(air > 0, "hops while counted");
    assert_eq!(grown, 0, "bytes allocated in 2400 ticks after warm-up");

    // The tick's cost (printed, not held: a machine's timing is not a test).
    for levels in [false, true] {
        let mut s = world(levels);
        for f in 0..600 {
            tick(&mut s, f);
        }
        let t = Instant::now();
        for f in 600..6600 {
            tick(&mut s, f);
        }
        let per = t.elapsed().as_nanos() / 6000;
        let ps = s.path_stats();
        println!(
            "terraces, {}: {per} ns a tick ({} searches, {} failed, {} partial, {} steered, {} in legs, {} not run, {} nodes expanded)",
            if levels { "levels" } else { "flat" },
            ps.searches,
            ps.failed,
            ps.partial,
            ps.steered,
            ps.chained,
            ps.unreachable,
            ps.expanded
        );
    }
    // Sight alone: 200 000 lines across the terraces (most of them crossing a level), eye to
    // eye, against the same lines on the flat copy.
    for levels in [false, true] {
        let s = world(levels);
        let g = &s.runtime(Z).expect("the terraces").grid;
        let mut rng = jane_core::Sfc32::seeded(9, 9);
        let mut seen = 0u32;
        let t = Instant::now();
        for _ in 0..200_000 {
            let mut p = || Vec2::centre(1 + rng.below(T::W - 2) as i32, 1 + rng.below(T::H - 2) as i32);
            let (a, b) = (p(), p());
            seen += u32::from(jane_sim::los::line_of_sight(g, a, b));
        }
        let per = t.elapsed().as_nanos() / 200_000;
        println!(
            "terraces, {}: {per} ns a sight line ({seen} of 200000 clear)",
            if levels { "levels" } else { "flat" }
        );
    }
}
