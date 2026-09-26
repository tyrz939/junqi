//! The snake (`sim/snake.ts`; SYSTEMS.md §6, the one custom mover), on the field of
//! `tests/field`: the heading turns at most `speed x 4` degrees a tick (`engine.test.ts`
//! "angles: turns by a clamped amount"), it steers round its patrol, finds the party on its
//! beat, follows at its phase's run laying its body down behind it, the phase clock turns
//! "follow" into "spit" and back, and a wall between them (walls only) sends it home whole.

mod common;
mod field;

use field::*;
use jane_core::num::{dist_sq, isqrt};
use jane_core::{Angle, Fx, Milli, Tick, Vec2};
use jane_sim::snake::{max_turn, turn_toward};
use jane_sim::state::{CombatState, Patrol};
use jane_sim::units::max_hp;
use jane_sim::{Command, DevOp, Sim, UnitId};

fn heading(s: &Sim, id: UnitId) -> Angle {
    unit(s, id).snake.as_ref().unwrap().heading
}

fn clock(s: &Sim, id: UnitId) -> u32 {
    unit(s, id).snake.as_ref().unwrap().phase_tick.0
}

fn px(a: Vec2, b: Vec2) -> i64 {
    i64::from(isqrt(dist_sq(a, b) as u64)) / 256
}

/// engine.test.ts "turns by a clamped amount", in angles: 0.75 px a tick turns 3°, 1.2 turns
/// 4.8°; a turn never overshoots, and within the limit it lands on what it wants.
#[test]
fn it_turns_at_most_speed_times_four_degrees() {
    assert_eq!(max_turn(Fx(192)), Angle::from_degrees(3).0 as i32);
    assert_eq!(max_turn(Fx::from_px(1)), 728, "4 degrees");
    let t = max_turn(Fx(307));
    assert_eq!(t, Angle::from_decidegrees(48).0 as i32 - 1, "4.8 degrees, floored");
    let east = Angle::EAST;
    assert_eq!(turn_toward(east, Angle::SOUTH, t), Angle(t as u16));
    assert_eq!(turn_toward(east, Angle::NORTH, t), east.wrapping_add(-t));
    assert_eq!(turn_toward(east, Angle(300), t), Angle(300), "within the limit: exactly");
    // Straight behind: it turns one way, by the limit.
    assert_eq!(east.gap(turn_toward(east, Angle::WEST, t)), t as u32);
}

/// Idle, it steers round its patrol at a walk, turning a little each tick, and its body follows.
#[test]
fn it_steers_round_its_patrol() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let snake = spawn(&mut s, "burial_snake", 70, 40);
    let (a, b) = (Vec2::centre(100, 40), Vec2::centre(70, 56));
    edit(&mut s, snake, |u| u.patrol = Some(Box::new(Patrol { points: vec![(a, Tick(0)), (b, Tick(0))] })));
    let walk = jane_data::catalog().combat.unit(unit(&s, snake).def).walk;
    let (mut near_a, mut near_b) = (false, false);
    for _ in 0..5000 {
        let h = heading(&s, snake);
        steps(&mut s, 1);
        assert!(h.gap(heading(&s, snake)) <= max_turn(walk) as u32);
        near_a |= px(unit(&s, snake).pos, a) <= 12;
        near_b |= near_a && px(unit(&s, snake).pos, b) <= 12;
    }
    assert!(near_a && near_b, "it went round");
    assert_eq!(unit(&s, snake).combat, CombatState::Idle);
    let trail = &unit(&s, snake).snake.as_ref().unwrap().trail;
    assert_eq!(trail.len(), 64);
    assert!(trail.iter().any(|&p| p != trail[0]), "its body is laid out behind it");
}

/// It finds her on its beat (no wall between), follows at its phase's run turning 3° a tick at
/// most, bites, and its "follow" clock counts only the ticks it was not touching her.
#[test]
fn it_finds_her_follows_and_bites() {
    let mut s = field();
    let snake = spawn(&mut s, "burial_snake", 26, 10);
    let run = jane_data::catalog().combat.unit(unit(&s, snake).def).phases[0].run.unwrap_or(Fx(192));
    steps(&mut s, 10);
    assert_eq!(unit(&s, snake).combat, CombatState::Combat);
    assert_eq!(unit(&s, snake).target, Some(me(&s)));
    let hp = unit(&s, me(&s)).hp;
    let (mut apart, before) = (0, clock(&s, snake));
    for _ in 0..300 {
        let h = heading(&s, snake);
        let touching = jane_sim::combat::metres_between(unit(&s, snake), unit(&s, me(&s))) <= 0;
        steps(&mut s, 1);
        assert!(h.gap(heading(&s, snake)) <= max_turn(run) as u32);
        apart += u32::from(!touching);
    }
    assert!(unit(&s, me(&s)).hp < hp, "it bit her");
    assert_eq!(clock(&s, snake) - before, apart, "the clock counts the ticks it was not touching her");
    assert!(apart < 300, "and it did touch her");
    assert_eq!(unit(&s, snake).phase, 0);
}

/// The phase clock: 900 moving ticks of "follow" become "spit"; it goes home first, coils, and
/// after 300 ticks of spitting it follows again.
#[test]
fn the_phase_clock_turns_follow_into_spit_and_back() {
    let mut s = field();
    cmd(&mut s, Some(0), Command::Dev(DevOp::God(true)));
    let snake = spawn(&mut s, "burial_snake", 30, 10);
    let her = me(&s);
    edit(&mut s, snake, |u| {
        u.target = Some(her);
        u.combat = CombatState::Combat;
        u.pos = Vec2::centre(24, 10);
        u.snake.as_mut().unwrap().phase_tick = Tick(899);
    });
    steps(&mut s, 1);
    assert_eq!((unit(&s, snake).phase, clock(&s, snake)), (1, 0), "follow is over");
    for _ in 0..400 {
        steps(&mut s, 1);
        if px(unit(&s, snake).pos, unit(&s, snake).home) <= 16 && clock(&s, snake) > 0 {
            break;
        }
    }
    assert!(px(unit(&s, snake).pos, unit(&s, snake).home) <= 16, "home first");
    edit(&mut s, snake, |u| u.snake.as_mut().unwrap().phase_tick = Tick(299));
    steps(&mut s, 1);
    assert_eq!((unit(&s, snake).phase, clock(&s, snake)), (0, 0), "and it follows again");
}

/// Anti-cheese: a wall across the line to her (the field's wall at x = 40; walls only, pillars
/// never counted) and it is home at once, whole, calm, at phase 0, every status off, its body
/// gathered in. Calm, it mends a three-hundredth of its health a tick.
#[test]
fn a_wall_between_sends_it_home_whole() {
    let mut s = field();
    let her = me(&s);
    let snake = spawn(&mut s, "burial_snake", 46, 30);
    edit(&mut s, her, |u| u.pos = Vec2::centre(34, 30));
    let burning = jane_data::catalog().combat.effect_id("burning").unwrap();
    let now = s.state().tick;
    edit(&mut s, snake, |u| {
        u.target = Some(her);
        u.combat = CombatState::Combat;
        u.phase = 1;
        u.pos = Vec2::centre(44, 34);
        u.hp = Milli(u.hp.0 / 2);
        u.statuses.push(jane_sim::state::StatusInst {
            effect: burning,
            until: now.after(Tick(600)),
            next_pulse: now.after(Tick(30)),
            from: None,
            pool: Milli::ZERO,
        });
        u.snake.as_mut().unwrap().trail[5] = Vec2::centre(50, 50);
    });
    steps(&mut s, 1);
    let u = unit(&s, snake);
    assert_eq!((u.combat, u.target, u.phase), (CombatState::Idle, None, 0));
    assert_eq!(u.pos, u.home);
    assert_eq!(u.hp, max_hp(u));
    assert!(u.statuses.is_empty());
    assert!(u.snake.as_ref().unwrap().trail.iter().all(|&p| p == u.home));

    // Calm and hurt, it mends; it does not see her through the wall.
    edit(&mut s, snake, |u| u.hp = Milli(100_000));
    steps(&mut s, 30);
    let u = unit(&s, snake);
    assert_eq!(u.combat, CombatState::Idle);
    assert_eq!(u.hp.0, 100_000 + 30 * (max_hp(u).0 / 300));
}
