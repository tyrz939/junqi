//! `Send` and walking by path (`actions.ts send`, `ai.ts followOrder`, `followTo`). A sent unit
//! walks to a mark minding nothing else: no aggro, no leash, no patrol. On arrival its `then`
//! list runs with it as the subject, on behalf of whoever sent it if she is still here. When its
//! budget runs out it gives up where it stands and nothing happens. A unit under orders does not
//! sleep (the ring keeps it awake), so it walks however far away she is.
//!
//! [`follow_to`] is the one walker along a cached cell path (the ai unit's chase, leash and patrol
//! use it too): it re-plans when the path is used up, when the goal moved, or every 20 ticks, at
//! most four searches a tick in a zone; a cell someone else stepped into since is waited at.

use jane_core::num::{dist_sq, isqrt, mul_div_floor};
use jane_core::{CellIx, Fx, Tick, Vec2};
use jane_data::Controller;

use crate::actions::{Subject, run_actions};
use crate::ctx::Ctx;
use crate::ids::UnitId;
use crate::light::lit_at;
use crate::path::{PathAsk, REPATH_TICKS, cost_of_cells};
use crate::runtime::cell_ix;
use crate::state::{CombatState, Order, PathCache};
use crate::tuning::{ORDER_ARRIVED_FX, ORDER_BASE, ORDER_PATH_CELLS};
use crate::units::{def_of, face_vector, move_unit};

/// The `Send` verb: `unit` is to walk to `to` and then run `then` (on behalf of the actor).
/// Not one of the party, and not the snake: that one has a mover of its own.
pub fn send(cx: &mut Ctx<'_>, unit: UnitId, to: Vec2, then: Option<jane_core::ListRef>) {
    let tick = cx.world.tick;
    let seat = cx.actor;
    let party = cx.party.seat_of(unit).is_some();
    let Some(u) = cx.zone.unit_mut(unit) else { return };
    if !u.alive || party || matches!(u.controller, Controller::Player | Controller::Snake) {
        return;
    }
    let def = def_of(u);
    let speed = def.run.0.max(def.walk.0);
    if speed <= 0 {
        return;
    }
    let d = isqrt(dist_sq(u.pos, to) as u64);
    let walk = (u64::from(d) * 3).div_ceil(speed as u64) as u32;
    u.order = Some(Box::new(Order { to, until: tick.after(Tick(ORDER_BASE.0 + walk)), then, seat }));
    u.target = None;
    u.combat = CombatState::Idle;
    u.path = None;
    u.dwell_until = Tick::ZERO;
    if !u.awake && !cx.ops.wake.contains(&unit) {
        cx.ops.wake.push(unit);
    }
}

/// Step 7's share of this unit: every awake, living, unhidden ai or npc unit under orders follows
/// them, over the awake list as it stood (a wake or a spawn lands at step 13). The ai unit folds
/// this into its controller dispatch (an order comes before anything else a unit would do).
pub fn step_orders(cx: &mut Ctx<'_>) {
    let mut i = 0;
    while i < cx.rt.awake_units.len() {
        let id = cx.rt.awake_units[i];
        i += 1;
        let Some(u) = cx.zone.unit(id) else { continue };
        if !u.alive || u.hidden || u.order.is_none() || !matches!(u.controller, Controller::Ai | Controller::Npc) {
            continue;
        }
        follow_order(cx, id);
    }
}

/// One tick of a unit's order (`ai.ts followOrder`).
pub fn follow_order(cx: &mut Ctx<'_>, id: UnitId) {
    let tick = cx.world.tick;
    let Some(ix) = cx.zone.unit_ix(id) else { return };
    let u = &mut cx.zone.units[ix];
    let Some(o) = u.order.as_deref().copied() else { return };
    let def = def_of(u);
    u.target = None;
    u.combat = CombatState::Idle;
    let speed = def.run.0.max(def.walk.0);
    if tick >= o.until || speed <= 0 {
        u.order = None;
        u.path = None;
        return;
    }
    // Beside the mark will do: someone may be standing on it.
    if dist_sq(u.pos, o.to) <= i64::from(ORDER_ARRIVED_FX) * i64::from(ORDER_ARRIVED_FX) {
        u.order = None;
        u.path = None;
        u.home = u.pos;
        if let Some(then) = o.then {
            // On behalf of whoever sent it, if she is still here.
            let z = cx.zone.id;
            let here = o.seat.filter(|&s| {
                cx.world.player(s).is_some_and(|p| p.connected && p.zone == z && cx.zone.unit(p.unit).is_some())
            });
            let before = std::mem::replace(&mut cx.actor, here);
            run_actions(cx, then, Subject::Unit(id));
            cx.actor = before;
        }
        return;
    }
    let shy = def.shuns_light;
    follow_to(cx, id, o.to, Fx(speed), ORDER_PATH_CELLS, shy);
}

/// Walk unit `id` toward `goal` at `speed` along a cached cell path, planning at most
/// `max_cells` of path (`ai.ts followTo`). Returns false only when a search ran and found
/// nothing. `shy`: it will not step into warm light; its searches go round lit cells and one
/// that cannot reach the goal ends at the nearest dark cell, where it waits.
pub fn follow_to(cx: &mut Ctx<'_>, id: UnitId, goal: Vec2, speed: Fx, max_cells: u32, shy: bool) -> bool {
    let tick = cx.world.tick;
    let clock = cx.world.clock;
    let Some(ix) = cx.zone.unit_ix(id) else { return false };
    let w = cx.rt.grid.w();
    let goal_cell = cell_ix(goal, w);
    let u = &cx.zone.units[ix];
    let (used, stale_goal, due) = match u.path.as_deref() {
        None => (true, true, true),
        Some(p) => (usize::from(p.at) >= p.cells.len(), p.goal != goal_cell, tick >= p.repath_at),
    };
    // Something that shuns light waits at the edge of it: a path used up is not stale for it.
    let stale = if shy { u.path.is_none() || stale_goal } else { used || stale_goal };
    if (stale || due) && cx.rt.take_path_search() {
        let start = u.pos.cell();
        let ask = PathAsk::new(start, goal.cell(), cost_of_cells(max_cells));
        let (zone, rt) = (&*cx.zone, &*cx.rt);
        let found = if shy {
            cx.scratch.path.find_shunning(&rt.grid, ask, true, |x, y| lit_at(zone, rt, clock, Vec2::centre(x, y), true))
        } else {
            cx.scratch.path.find(&rt.grid, ask)
        };
        let u = &mut cx.zone.units[ix];
        if found.is_none() {
            u.path = None;
            return false;
        }
        let cells = cx.scratch.path.out.iter().map(|&(x, y)| CellIx(y as u32 * w + x as u32));
        match u.path.as_deref_mut() {
            Some(p) => {
                p.cells.clear();
                p.cells.extend(cells);
                p.at = 0;
                p.goal = goal_cell;
                p.repath_at = tick.after(Tick(REPATH_TICKS));
            }
            None => {
                u.path = Some(Box::new(PathCache {
                    cells: cells.collect(),
                    at: 0,
                    goal: goal_cell,
                    repath_at: tick.after(Tick(REPATH_TICKS)),
                }));
            }
        }
    }
    // The status unit multiplies in the slows (`speedFactor`).
    let mut budget = speed.0;
    while budget > 0 {
        let u = &cx.zone.units[ix];
        let Some(p) = u.path.as_deref() else { break };
        let Some(&c) = p.cells.get(usize::from(p.at)) else { break };
        let last = usize::from(p.at) + 1 >= p.cells.len();
        let (cx_, cy_) = ((c.0 % w) as i32, (c.0 / w) as i32);
        // Held by someone else: if it is the goal, that is a target's own feet, so stop beside
        // it; otherwise someone stepped in since planning: wait, and plan again soon.
        let own = u.pos.cell();
        let lit = shy && lit_at(cx.zone, cx.rt, clock, Vec2::centre(cx_, cy_), true);
        if !cx.rt.grid.free(cx_, cy_, Some(own)) || lit {
            if !last || lit {
                let soon = tick.after(Tick(4));
                if let Some(p) = cx.zone.units[ix].path.as_deref_mut() {
                    p.repath_at = p.repath_at.min(soon);
                }
            }
            break;
        }
        let to = Vec2::centre(cx_, cy_);
        let u = &mut cx.zone.units[ix];
        let d = isqrt(dist_sq(u.pos, to) as u64) as i32;
        if d <= budget {
            let (dx, dy) = (Fx(to.x.0 - u.pos.x.0), Fx(to.y.0 - u.pos.y.0));
            move_unit(cx.rt, u, dx, dy);
            budget -= d;
            if let Some(p) = u.path.as_deref_mut() {
                p.at += 1;
            }
        } else {
            let dx = mul_div_floor(to.x.0 - u.pos.x.0, budget, d);
            let dy = mul_div_floor(to.y.0 - u.pos.y.0, budget, d);
            face_vector(u, i64::from(dx), i64::from(dy));
            move_unit(cx.rt, u, Fx(dx), Fx(dy));
            budget = 0;
        }
    }
    true
}
