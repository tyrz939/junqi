//! The world verbs with more to them than a line (`actions.ts rest, teach, grow, throw, reveal`):
//! resting at a bed or a fire, growth (spells learned, jars found), bait thrown, and a notice that
//! is a map. `actions.rs` dispatches to these.

use jane_core::action::Stat;
use jane_core::num::CELL_FX;
use jane_core::{Fx, ItemId, Rect, Sym, Vec2};

use crate::ctx::{Ctx, WorldOp};
use crate::event::{EventKind, ToastKind};
use crate::ids::UnitId;
use crate::interact::{Here, near_rest, near_rest_scan};
use crate::inventory::spawn_drop;
use crate::los::line_of_sight;
use crate::state::{GameState, RestPoint};
use crate::tuning::{ENERGY_MAX, TICKS_PER_HOUR};

/// Is every connected seat within reach of a bed or a fire? Asked live: the actor's zone through
/// its runtime, anyone elsewhere by a scan of her zone's props.
pub fn everyone_resting(cx: &mut Ctx<'_>) -> bool {
    let z = cx.zone.id;
    for i in 0..cx.world.players.len() {
        let p = &cx.world.players[i];
        if !p.connected {
            continue;
        }
        let (pz, unit) = (p.zone, p.unit);
        let ok = if pz == z {
            let Some(at) = cx.zone.unit(unit).map(|u| u.pos) else { return false };
            let h = Here { world: cx.world, zone: cx.zone, rt: cx.rt, bp: cx.bp };
            near_rest(&h, at, &mut cx.scratch.props_b)
        } else {
            cx.world.zone(pz).is_some_and(|zs| zs.unit(unit).is_some_and(|u| near_rest_scan(zs, u.pos)))
        };
        if !ok {
            return false;
        }
    }
    true
}

/// `Rest`: a bed or a fire mends her, becomes the party's waking place, and asks the app to
/// save. `until` sleeps the clock forward to that hour (a bed), and only when the whole party is
/// resting; without it no time passes (a fire).
pub fn rest(cx: &mut Ctx<'_>, until: Option<u8>) {
    let Some(body) = cx.actor_unit() else { return };
    let now = cx.world.tick;
    let Some(u) = cx.zone.unit_mut(body) else { return };
    crate::life::pay_regen(u, now);
    crate::zone::heal_full(u);
    u.energy = ENERGY_MAX;
    u.energy_locked = false;
    let pos = u.pos;
    cx.world.rest = Some(RestPoint { zone: cx.zone.id, pos });
    if let Some(h) = until {
        // The clock is everyone's. The night only passes when the whole party is resting.
        if everyone_resting(cx) {
            let target = u32::from(h) * TICKS_PER_HOUR;
            if cx.world.clock >= target {
                cx.world.day += 1;
            }
            cx.world.clock = target;
        } else {
            cx.emit(EventKind::Toast(ToastKind::NightWaits));
        }
    }
    // To everyone: the world lives on the host's machine, and a guest's rest saves it too.
    cx.emit_all(EventKind::Rest);
}

/// Growth by finding: `id` is the jar itself, so a reward paid to four seats is eaten once.
/// The world's growth moves now; every body, parked ones too, is patched by `WorldOp::Grow`.
pub fn grow(cx: &mut Ctx<'_>, stat: Stat, amount: i16, id: Sym) -> bool {
    let g = &mut cx.world.growth;
    if g.found.contains(&id) {
        return false;
    }
    g.found.push(id);
    let v = match stat {
        Stat::Strength => &mut g.strength,
        Stat::Spirit => &mut g.spirit,
    };
    *v = v.saturating_add_signed(amount);
    cx.wops.ops.push(WorldOp::Grow { stat, amount });
    cx.emit_all(EventKind::Toast(if stat == Stat::Strength { ToastKind::Stronger } else { ToastKind::WordsStay }));
    true
}

/// The console's growth (`DevOp::Grow`): what a finding gives, with no finding to remember and
/// nothing said.
pub fn dev_grow(cx: &mut Ctx<'_>, stat: Stat, amount: i16) {
    let g = &mut cx.world.growth;
    let v = match stat {
        Stat::Strength => &mut g.strength,
        Stat::Spirit => &mut g.spirit,
    };
    *v = v.saturating_add_signed(amount);
    cx.wops.ops.push(WorldOp::Grow { stat, amount });
}

/// `WorldOp::Grow` landing: every body gains it; the new health is hers at once.
pub fn grow_bodies(state: &mut GameState, stat: Stat, amount: i16) {
    let now = state.tick;
    let GameState { players, zones, .. } = state;
    for p in players.iter_mut() {
        if let Some(u) = p.parked.as_deref_mut() {
            crate::units::grow_body(u, stat, amount);
        } else if let Some(u) = zones[p.zone.index()].as_deref_mut().and_then(|z| z.unit_mut(p.unit)) {
            // What regen she was owed is hers first, so the gain lands on her health as it is.
            crate::life::pay_regen(u, now);
            crate::units::grow_body(u, stat, amount);
        }
    }
}

/// `Throw`: bait lands three cells ahead, or at the feet if a wall is in the way.
pub fn throw(cx: &mut Ctx<'_>, thrower: Option<UnitId>, item: ItemId) {
    let Some(u) = thrower.and_then(|id| cx.zone.unit(id)) else { return };
    let (fx, fy) = u.facing.delta();
    let to = Vec2::new(Fx(u.pos.x.0 + fx * 3 * CELL_FX), Fx(u.pos.y.0 + fy * 3 * CELL_FX));
    let (tx, ty) = to.cell();
    let open = line_of_sight(&cx.rt.grid, u.pos, to) && !cx.rt.grid.solid(tx, ty);
    let at = if open { to } else { u.pos };
    spawn_drop(cx, item, 1, at);
}

/// `Reveal`: the fog's seen-bits over a rect, indoors (outdoors the ground is always drawn).
pub fn reveal_rect(cx: &mut Ctx<'_>, r: Rect) {
    if !cx.bp.indoor {
        return;
    }
    let g = cx.rt.fog;
    let size = g.cells as i32;
    let bx0 = ((r.x - 1).div_euclid(size)).max(0);
    let by0 = ((r.y - 1).div_euclid(size)).max(0);
    let bx1 = (r.right().div_euclid(size)).min(g.w as i32 - 1);
    let by1 = (r.bottom().div_euclid(size)).min(g.h as i32 - 1);
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            let bit = y as u32 * g.w + x as u32;
            if let Some(w) = cx.zone.fog.get_mut((bit >> 5) as usize) {
                *w |= 1 << (bit & 31);
            }
        }
    }
}
