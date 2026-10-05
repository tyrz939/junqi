//! The snake (`sim/snake.ts`): the one custom mover (SYSTEMS.md §6). Everything else about it is
//! ordinary: it is a `Unit`, it casts through `try_cast`, and its blows land in the flush. Only
//! its locomotion and its phase clock are its own.
//!
//! Read from 2020's `obj_snake_boss`:
//!
//! | | |
//! | --- | --- |
//! | idle | regen, look for the party every [`AGGRO_PERIOD`] ticks (staggered), steer round the patrol at a walk |
//! | phase 0, "follow" | [`SNAKE_FOLLOW_TICKS`] moving ticks of steering at her at its phase's run, casting its phase's book (melee) |
//! | phase 1, "spit" | steer home; once coiled within 16 px, [`SNAKE_SPIT_TICKS`] ticks of its phase's book (poison rings), then follow again |
//! | reset | a wall across the line to her (walls only: 2020's pillars never counted), or no target: home, whole, calm, its body gathered in |
//!
//! **Steering.** The heading is an [`Angle`] turned toward what it wants by at most `speed x 4`
//! degrees a tick (0.75 px a tick turns 3°, 1.2 turns 4.8°), then stepped along at `speed`.
//! **The body** is the trail: every [`SNAKE_NODE_EVERY`]th tick it moves, its feet go on the
//! front of the trail and the oldest point falls off, so the body is its own `segments` points
//! long. Every eighth point is hit-tested (`combat::body_dist_sq`). The trail's spacing in px is
//! presentation's (the row's `body.spacing`).
//!
//! **The phase clock** is `SnakeBody.phase_tick`, a count of ticks spent in the phase as the TS
//! counted them (moving ticks in "follow", coiled ticks in "spit"), not a time: the snake's
//! phases are by clock, never by health, and it reads its phase table by index (phase 0 is row
//! 0), where the ordinary AI's phase `n` is row `n - 1`.

use jane_core::angle::{along, bearing};
use jane_core::num::{dist_sq, mul_div_floor};
use jane_core::{Angle, Fx, Tick, Vec2};
use jane_data::UnitDef;

use crate::combat::{distance, metres_between, try_cast};
use crate::ctx::{Ctx, unit_mut_or_skip};
use crate::ids::UnitId;
use crate::los::line_of_sight_walls;
use crate::state::CombatState;
use crate::status::clear_statuses;
use crate::tuning::{
    AGGRO_PERIOD, SNAKE_FOLLOW_TICKS, SNAKE_HOME_FX, SNAKE_NODE_EVERY, SNAKE_PATROL_REACHED_FX, SNAKE_SPIT_TICKS,
    SNAKE_TURN_DEN, SNAKE_TURN_NUM,
};
use crate::units::{def_of, face_angle, move_unit, place_unit, think_offset};

/// One tick of a snake.
pub fn tick_snake(cx: &mut Ctx<'_>, id: UnitId) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit(id) else { return };
    let def = def_of(u);
    if u.combat != CombatState::Combat {
        // Its own regen is step 5's (`life::pay_regen`: a calm snake mends, a fighting one does not).
        if (now.0 + think_offset(id)) % AGGRO_PERIOD == 0 {
            if let Some(prey) = prey(cx, id, def) {
                let u = unit_mut_or_skip!(cx, id, "snake::tick_snake");
                u.target = Some(prey);
                u.combat = CombatState::Combat;
                u.phase = 0;
                if let Some(s) = u.snake.as_deref_mut() {
                    s.phase_tick = Tick::ZERO;
                }
                return;
            }
        }
        let u = unit_mut_or_skip!(cx, id, "snake::tick_snake");
        if let Some(p) = u.patrol.as_deref().filter(|p| p.points.len() >= 2) {
            let n = p.points.len();
            let mut at = usize::from(u.patrol_at) % n;
            if dist_sq(u.pos, p.points[at].0) <= i64::from(SNAKE_PATROL_REACHED_FX).pow(2) {
                at = (at + 1) % n;
                u.patrol_at = at as u16;
            }
            let to = p.points[at].0;
            steer(cx, id, to, def.walk);
        }
        return;
    }

    let target = u.target.and_then(|t| cx.zone.unit(t)).filter(|t| t.alive).map(|t| (t.id, t.pos));
    let Some((tid, tpos)) = target else {
        reset(cx, id);
        return;
    };
    // Walls only: 2020 tested `obj_wall` on the line, and its pillars never counted.
    if !line_of_sight_walls(&cx.rt.grid, u.pos, tpos) {
        reset(cx, id);
        return;
    }
    let row = def.phases.get(usize::from(u.phase));
    let book = row.map_or(def.book, |r| r.book);
    let speed = row.and_then(|r| r.run).unwrap_or(def.run);
    let touching = cx.zone.unit(tid).is_some_and(|t| metres_between(u, t) <= 0);
    let (phase, home, pos) = (u.phase, u.home, u.pos);

    if phase == 0 {
        if !touching {
            steer(cx, id, tpos, speed);
            bump_clock(cx, id);
        }
        cast_first(cx, id, book);
        let u = unit_mut_or_skip!(cx, id, "snake::tick_snake");
        if u.snake.as_deref().is_some_and(|s| s.phase_tick.0 >= SNAKE_FOLLOW_TICKS) {
            next_phase(u, 1);
        }
        return;
    }
    // Phase 1: home first; the spit clock runs only once it is coiled there.
    if distance(pos, home) > i64::from(SNAKE_HOME_FX) {
        steer(cx, id, home, speed);
        return;
    }
    cast_first(cx, id, book);
    bump_clock(cx, id);
    let u = unit_mut_or_skip!(cx, id, "snake::tick_snake");
    if u.snake.as_deref().is_some_and(|s| s.phase_tick.0 >= SNAKE_SPIT_TICKS) {
        next_phase(u, 0);
    }
}

/// Whoever of the party here is nearest (centre to centre) within its aggro, alive, not a god,
/// with no wall between. A tie goes to the later seat.
fn prey(cx: &Ctx<'_>, id: UnitId, def: &UnitDef) -> Option<UnitId> {
    let u = cx.zone.unit(id)?;
    let mut best = i64::from(def.aggro.0);
    let mut prey = None;
    for (seat, body) in cx.party.bodies.iter().enumerate() {
        let Some((z, bid, _)) = *body else { continue };
        if z != cx.zone.id || cx.world.players.get(seat).is_some_and(|p| p.god) {
            continue;
        }
        let Some(b) = cx.zone.unit(bid).filter(|b| b.alive) else { continue };
        let d = distance(u.pos, b.pos);
        if d <= best && line_of_sight_walls(&cx.rt.grid, u.pos, b.pos) {
            best = d;
            prey = Some(bid);
        }
    }
    prey
}

/// Its phase's book in order; the first cast that lands ends the tick's casting.
fn cast_first(cx: &mut Ctx<'_>, id: UnitId, book: &[jane_core::SpellId]) {
    for &spell in book {
        if try_cast(cx, id, spell, None, None).is_ok() {
            break;
        }
    }
}

fn bump_clock(cx: &mut Ctx<'_>, id: UnitId) {
    if let Some(s) = cx.zone.unit_mut(id).and_then(|u| u.snake.as_deref_mut()) {
        s.phase_tick = Tick(s.phase_tick.0 + 1);
    }
}

fn next_phase(u: &mut crate::state::Unit, phase: u8) {
    u.phase = phase;
    if let Some(s) = u.snake.as_deref_mut() {
        s.phase_tick = Tick::ZERO;
    }
}

/// The largest turn a tick allows at `speed`: `speed x 4` degrees for a speed in px a tick.
pub fn max_turn(speed: Fx) -> i32 {
    mul_div_floor(speed.0, SNAKE_TURN_NUM, SNAKE_TURN_DEN)
}

/// Turn `heading` toward `want` by at most `max`.
pub fn turn_toward(heading: Angle, want: Angle, max: i32) -> Angle {
    heading.wrapping_add(heading.diff(want).clamp(-max, max))
}

/// Steer toward `to` at `speed`: turn, step, and on every fourth moving tick lay the feet on
/// the front of the trail.
fn steer(cx: &mut Ctx<'_>, id: UnitId, to: Vec2, speed: Fx) {
    if speed.0 <= 0 {
        return;
    }
    let Some(u) = cx.zone.unit_mut(id) else { return };
    if u.pos == to {
        return;
    }
    let want = bearing(u.pos, to);
    let Some(body) = u.snake.as_deref_mut() else { return };
    body.heading = turn_toward(body.heading, want, max_turn(speed));
    let heading = body.heading;
    let d = along(heading, speed);
    let moved = move_unit(cx.rt, u, d.x, d.y);
    face_angle(u, heading);
    if !moved {
        return;
    }
    let pos = u.pos;
    let Some(body) = u.snake.as_deref_mut() else { return };
    body.step = (body.step + 1) % SNAKE_NODE_EVERY;
    if body.step != 0 || body.trail.is_empty() {
        return;
    }
    body.trail.rotate_right(1);
    body.trail[0] = pos;
}

/// Lost her (a wall between, or she is gone): home at once, whole, calm, at phase 0, every
/// status off and every blow still owed it dropped, its body gathered in at home: 2020's
/// anti-cheese reset.
pub fn reset(cx: &mut Ctx<'_>, id: UnitId) {
    let Some(u) = cx.zone.unit_mut(id) else { return };
    let home = u.home;
    place_unit(cx.rt, u, home);
    crate::life::pay_regen(u, cx.world.tick);
    crate::zone::heal_full(u);
    u.target = None;
    u.combat = CombatState::Idle;
    next_phase(u, 0);
    if let Some(s) = u.snake.as_deref_mut() {
        s.trail.fill(home);
    }
    cx.scratch.hits[cx.zone.id.index()].retain(|h| h.to != id);
    clear_statuses(cx, id);
}
