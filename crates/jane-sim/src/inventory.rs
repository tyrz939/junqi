//! Her bags, her craft row, her items (`sim/inventory.ts`, `loot.ts pickUp`). Bags are hers
//! (`PlayerState.bag`, ARCHITECTURE.md §3.2): 24 slots, stacks, a bar of 8, a craft row of 3
//! keyed by sorted inputs. Everything here runs inside a zone's context for one seat; the
//! slot arithmetic is `bag.rs`.
//!
//! - **Bound items** (keys the story is using, Julie's letter) refuse to be destroyed.
//! - **Story items** are handed on when their holder leaves (`seats.rs`) and never age out.
//! - **Using an item** needs the global cooldown clear and its own cooldown; it starts both and
//!   roots her for half a second. Keys are never used from the bag: doors ask for them
//!   (`interact.rs`), one path for every key.
//! - **Crafting** takes the output only when it fits; the inputs are consumed only then.

use jane_core::{Action, ItemId, Stack, Tick, Vec2};

use crate::actions::{Subject, run_actions};
use crate::bag::{bag_add, bag_count, bag_has_room, bag_move, bag_remove};
use crate::ctx::Ctx;
use crate::event::{Event, EventKind, ToastKind};
use crate::ids::{DropId, Seat};
use crate::journal;
use crate::state::Drop;
use crate::tuning::{BAG_SLOTS, CRAFT_INPUTS, GCD, ITEM_STOP};
use crate::units::max_hp;

/// An event for one seat, where she is.
pub(crate) fn emit_to(cx: &mut Ctx<'_>, seat: Seat, kind: EventKind) {
    let in_zone = Some(cx.zone.id);
    cx.events.push(Event { to: Some(seat), in_zone, kind });
}

/// Into her bag: what fits goes in (a `Loot` event and the journal's `Held`); returns what did
/// not fit.
pub fn add(cx: &mut Ctx<'_>, seat: Seat, item: ItemId, qty: u16) -> u16 {
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return qty };
    let left = bag_add(&mut p.bag[..], item, qty);
    if left < qty {
        emit_to(cx, seat, EventKind::Bag);
        emit_to(cx, seat, EventKind::Loot { item, qty: qty - left });
        journal::held(cx, item);
    }
    left
}

/// Out of her bag, last stacks first; returns how many.
pub fn remove(cx: &mut Ctx<'_>, seat: Seat, item: ItemId, qty: u16) -> u16 {
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return 0 };
    let n = bag_remove(&mut p.bag[..], item, qty);
    if n > 0 {
        emit_to(cx, seat, EventKind::Bag);
    }
    n
}

pub fn count(cx: &Ctx<'_>, seat: Seat, item: ItemId) -> u32 {
    cx.world.player(seat).map_or(0, |p| bag_count(&p.bag[..], item))
}

/// A drop on the ground of this zone.
pub fn spawn_drop(cx: &mut Ctx<'_>, item: ItemId, qty: u16, pos: Vec2) -> DropId {
    let id = cx.world.next.drop();
    let born = cx.world.tick;
    cx.zone.drops.push(Drop { id, item, qty, pos, born });
    id
}

/// `Give`: never lose a reward. What does not fit lands at her feet.
pub fn give(cx: &mut Ctx<'_>, item: ItemId, qty: u16) {
    let (Some(seat), Some(body)) = (cx.actor, cx.actor_unit()) else { return };
    let left = add(cx, seat, item, qty);
    if left > 0 {
        let pos = cx.zone.unit(body).map_or(Vec2::ZERO, |u| u.pos);
        spawn_drop(cx, item, left, pos);
        cx.emit(EventKind::Toast(ToastKind::InventoryFull));
    }
}

/// Pick up a drop (`loot.ts pickUp`): what fits; the rest stays on the ground.
pub fn pick_up(cx: &mut Ctx<'_>, seat: Seat, drop: DropId) -> bool {
    let Some(i) = cx.zone.drops.iter().position(|d| d.id == drop) else { return false };
    let d = cx.zone.drops[i];
    let left = add(cx, seat, d.item, d.qty);
    if left == d.qty {
        cx.emit(EventKind::Toast(ToastKind::InventoryFull));
        return false;
    }
    if left == 0 {
        cx.zone.drops.remove(i);
    } else {
        cx.zone.drops[i].qty = left;
    }
    true
}

/// Drag from one bag slot to another: merge the same item, else swap.
pub fn move_slot(cx: &mut Ctx<'_>, seat: Seat, from: u8, to: u8) {
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return };
    if bag_move(&mut p.bag[..], usize::from(from), usize::from(to)) {
        emit_to(cx, seat, EventKind::Bag);
    }
}

/// Drag out of the window. A bound item refuses.
pub fn destroy(cx: &mut Ctx<'_>, seat: Seat, slot: u8) -> bool {
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return false };
    let Some(s) = p.bag.get(usize::from(slot)).copied().flatten() else { return false };
    if cx.cat.combat.item(s.item).bound {
        cx.emit(EventKind::Toast(ToastKind::ShouldKeep));
        return false;
    }
    p.bag[usize::from(slot)] = None;
    emit_to(cx, seat, EventKind::Bag);
    true
}

/// Stunned (a status whose effect row says so, still running).
fn stunned(u: &crate::state::Unit, now: Tick) -> bool {
    let cat = jane_data::catalog();
    u.statuses.iter().any(|s| s.until > now && cat.combat.effect(s.effect).stun)
}

/// Use the first stack of `item` (the `Item` command, and a bar slot holding an item). The
/// item's list runs on her. Returns whether it was used.
pub fn use_item(cx: &mut Ctx<'_>, item: ItemId) -> bool {
    let (Some(seat), Some(body)) = (cx.actor, cx.actor_unit()) else { return false };
    let def = cx.cat.combat.item(item);
    let tick = cx.world.tick;
    let Some(ix) = cx.zone.unit_ix(body) else { return false };
    if !cx.zone.units[ix].alive || count(cx, seat, item) == 0 {
        return false;
    }
    let Some(list) = def.use_list.filter(|_| def.usable) else {
        if def.opens.is_some() {
            cx.emit(EventKind::Toast(ToastKind::FitsALock));
        }
        return false;
    };
    let u = &cx.zone.units[ix];
    if stunned(u, tick) || u.gcd_until > tick || u.item_cooldowns.iter().any(|&(i, t)| i == item && t > tick) {
        return false;
    }
    let actions: &[Action] = cx.cat.list(list);
    if matches!(actions, [Action::Heal(_)]) && u.hp >= max_hp(u) {
        cx.emit(EventKind::Toast(ToastKind::NotHurt));
        return false;
    }
    run_actions(cx, list, Subject::Unit(body));
    if !def.keep {
        remove(cx, seat, item, 1);
    }
    if let Some(u) = cx.zone.unit_mut(body) {
        if def.cooldown.0 > 0 {
            let until = tick.after(def.cooldown);
            match u.item_cooldowns.iter_mut().find(|(i, _)| *i == item) {
                Some(c) => c.1 = until,
                None => u.item_cooldowns.push((item, until)),
            }
        }
        u.gcd_until = tick.after(GCD);
        u.stop_until = u.stop_until.max(tick.after(ITEM_STOP));
    }
    emit_to(cx, seat, EventKind::Bag);
    true
}

// --- crafting: three inputs and one output, keyed by sorted item ids ----------------------

/// What the craft row makes, if anything: `(item, qty)`.
pub fn craft_output(craft: &[Option<Stack>; CRAFT_INPUTS]) -> Option<(ItemId, u16)> {
    let mut inputs = [ItemId(0); CRAFT_INPUTS];
    let mut n = 0;
    for s in craft.iter().flatten() {
        inputs[n] = s.item;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let inputs = &mut inputs[..n];
    inputs.sort();
    jane_data::catalog().combat.recipe_for(inputs).map(|r| (r.output, r.qty))
}

/// One of a bag stack onto an empty craft slot.
pub fn craft_put(cx: &mut Ctx<'_>, seat: Seat, bag: u8, slot: u8) {
    let (b, c) = (usize::from(bag), usize::from(slot));
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return };
    if b >= BAG_SLOTS || c >= CRAFT_INPUTS || p.craft[c].is_some() {
        return;
    }
    let Some(mut s) = p.bag[b] else { return };
    p.craft[c] = Some(Stack { item: s.item, qty: 1 });
    s.qty -= 1;
    p.bag[b] = (s.qty > 0).then_some(s);
    emit_to(cx, seat, EventKind::Bag);
}

/// A craft slot back into the bag (it stays if the bag has no room).
pub fn craft_clear(cx: &mut Ctx<'_>, seat: Seat, slot: u8) {
    let c = usize::from(slot);
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return };
    let Some(s) = p.craft.get(c).copied().flatten() else { return };
    if bag_add(&mut p.bag[..], s.item, s.qty) == 0 {
        p.craft[c] = None;
    }
    emit_to(cx, seat, EventKind::Bag);
}

pub fn craft_clear_all(cx: &mut Ctx<'_>, seat: Seat) {
    for slot in 0..CRAFT_INPUTS as u8 {
        craft_clear(cx, seat, slot);
    }
}

/// Take the output: the inputs are consumed only if it fits.
pub fn craft_take(cx: &mut Ctx<'_>, seat: Seat) -> bool {
    let Some(p) = cx.world.players.get_mut(seat.index()) else { return false };
    let Some((item, qty)) = craft_output(&p.craft) else { return false };
    if !bag_has_room(&p.bag[..], item, qty) {
        cx.emit(EventKind::Toast(ToastKind::InventoryFull));
        return false;
    }
    p.craft = [None; CRAFT_INPUTS];
    add(cx, seat, item, qty);
    true
}
