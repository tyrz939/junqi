//! Orders and the npc controller (`ai.ts tickNpc`, `followOrder`; `actions.ts send`).
//!
//! **An order** (`Send`, and presence walking a unit to its schedule's mark, §4.6.a) is walked
//! minding nothing else: no aggro, no leash, no patrol. On arrival (beside the mark will do:
//! someone may be standing on it) its `then` list runs with the unit as the subject, on behalf of
//! whoever sent it if she is still here, and the unit's home becomes where it stopped. When the
//! order's time runs out it gives up where it stands and nothing happens. A unit under orders
//! does not sleep (the ring keeps it awake), so it walks however far away she is.
//!
//! **An npc** (the dog, a butterfly, a hen) is fought by nobody, but it may be sent somewhere,
//! keep a patrol with dwells, and run from what its row `flees` (§4.6.c).

use jane_core::num::{dist_sq, isqrt};
use jane_core::{Fx, ListRef, Tick, Vec2};
use jane_data::{Controller, UnitDef};

use crate::actions::{Subject, run_actions};
use crate::ai::{clear_path, follow_to, patrol};
use crate::ctx::Ctx;
use crate::ids::UnitId;
use crate::state::{CombatState, Order};
use crate::tuning::{ORDER_ARRIVED_FX, ORDER_BASE, ORDER_PATH_CELLS};
use crate::units::def_of;

/// One tick of an npc: its order, else running from what it flees, else its patrol at a walk.
pub fn tick_npc(cx: &mut Ctx<'_>, id: UnitId) {
    let Some(u) = cx.zone.unit(id) else { return };
    let def = def_of(u);
    if u.order.is_some() {
        follow_order_with(cx, id, def);
        return;
    }
    if crate::ai::flee(cx, id, def, def.shuns_light) {
        return;
    }
    patrol(cx, id, def.walk, def.shuns_light);
}

/// How long an order lasts: `ORDER_BASE` plus three times the straight walk at `speed`.
pub fn order_budget(from: Vec2, to: Vec2, speed: Fx) -> Tick {
    let d = u64::from(isqrt(dist_sq(from, to) as u64));
    Tick(ORDER_BASE.0 + (d * 3).div_ceil(speed.0.max(1) as u64) as u32)
}

/// The `Send` verb's work: `unit` is to walk to `to` and then run `then` for the actor. Not one
/// of the party, not the dead, not a snake (it has a mover of its own), not what cannot walk.
/// It wakes at step 13 if it slept.
pub fn send(cx: &mut Ctx<'_>, unit: UnitId, to: Vec2, then: Option<ListRef>) {
    let now = cx.world.tick;
    let seat = cx.actor;
    let party = cx.party.seat_of(unit).is_some();
    let Some(u) = cx.zone.unit_mut(unit) else { return };
    if !u.alive || party || matches!(u.controller, Controller::Player | Controller::Snake) {
        return;
    }
    let def = def_of(u);
    let speed = Fx(def.run.0.max(def.walk.0));
    if speed.0 <= 0 {
        return;
    }
    let until = now.after(order_budget(u.pos, to, speed));
    u.order = Some(Box::new(Order { to, until, then, seat }));
    u.target = None;
    u.combat = CombatState::Idle;
    clear_path(u);
    u.dwell_until = Tick::ZERO;
    if !u.awake && !cx.ops.wake.contains(&unit) {
        cx.ops.wake.push(unit);
    }
}

/// One tick of a unit's order (`ai.ts followOrder`), with `def` as its row.
pub fn follow_order_with(cx: &mut Ctx<'_>, id: UnitId, def: &UnitDef) {
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(id) else { return };
    let Some(o) = u.order.as_deref().copied() else { return };
    u.target = None;
    u.combat = CombatState::Idle;
    let speed = Fx(def.run.0.max(def.walk.0));
    if now >= o.until || speed.0 <= 0 {
        u.order = None;
        clear_path(u);
        return;
    }
    if dist_sq(u.pos, o.to) <= i64::from(ORDER_ARRIVED_FX) * i64::from(ORDER_ARRIVED_FX) {
        u.order = None;
        clear_path(u);
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
    follow_to(cx, id, o.to, speed, ORDER_PATH_CELLS, def.shuns_light);
}
