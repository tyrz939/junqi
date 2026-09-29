//! Cupboards (PRESENTATION.md §3.2 "at a cupboard", WORLD.md §8.3): somewhere to put things
//! down and come back for them. A prop whose row says `store` (a cupboard, Julie's dresser, the
//! left-luggage lockers) keeps [`STORE_SLOTS`] slots, stacked as a bag stacks (`bag.rs`).
//!
//! - **The world's, not hers.** A cupboard's shelves are on [`GameState::stores`], keyed by zone
//!   and prop, and every seat at the table sees and uses the same ones (as the quests and the rest
//!   point are the party's; bags stay each seat's own). Saved and hashed with the rest of the world.
//! - **Anything goes in**, a key and a quest's one lost glove included: this is where the keys and
//!   the gold she cannot throw away can go. What is in a cupboard is not in her bag, so a quest
//!   step or a lock that wants a thing *held* does not see it there (`quests.rs` reads the bags).
//! - **Only within reach.** Every move names the prop and is refused unless she stands within
//!   reach of it (the bench's reach), in its zone, alive. USE on one says so to her alone
//!   ([`EventKind::Store`]); presentation opens the window on it.
//! - A move is a drag: onto an empty slot it goes whole, onto the same thing it tops up and the
//!   rest stays where it was, onto something else the two swap. A quick move (no slot named) tops
//!   up the other side's stacks and then fills its first holes; what does not fit stays put.

use jane_core::Stack;

use crate::bag::{bag_add, bag_move};
use crate::ctx::Ctx;
use crate::event::EventKind;
use crate::ids::{PropId, Seat};
use crate::interact::prop_distance_sq;
use crate::inventory::emit_to;
use crate::state::GameState;
use crate::tuning::{BAG_SLOTS, STORE_SLOTS, USE_REACH_FX};

/// How near she must stand, to the footprint: the bench's reach.
pub const STORE_REACH_FX: i32 = 2 * USE_REACH_FX;

/// Nothing kept.
pub const EMPTY: [Option<Stack>; STORE_SLOTS] = [None; STORE_SLOTS];

/// What cupboard `prop` of `zone` holds (all empty when nothing was ever put in).
pub fn slots_of(world: &GameState, zone: jane_core::ZoneId, prop: PropId) -> &[Option<Stack>; STORE_SLOTS] {
    world.stores.get(&(zone, prop)).map_or(&EMPTY, |s| s)
}

/// Is `prop` a cupboard of this zone that the actor stands within reach of?
fn in_reach(cx: &Ctx<'_>, prop: PropId) -> bool {
    let Some(body) = cx.actor_unit().and_then(|b| cx.zone.unit(b)) else { return false };
    let Some(ix) = cx.zone.prop_ix(prop) else { return false };
    let p = &cx.zone.props[ix as usize];
    let def = cx.cat.story.prop(p.def);
    body.alive && def.store && !p.hidden && prop_distance_sq(def, p, body.pos) <= i64::from(STORE_REACH_FX).pow(2)
}

/// Drop the row of a cupboard left empty, so an emptied cupboard is saved as one never used.
fn tidy(world: &mut GameState, key: (jane_core::ZoneId, PropId)) {
    if world.stores.get(&key).is_some_and(|s| s.iter().all(Option::is_none)) {
        world.stores.remove(&key);
    }
}

/// One stack from `src[si]` to `dst` at `di` (a drag), or into `dst` wherever it fits (`None`, a
/// quick move). Returns whether anything moved.
fn cross(src: &mut [Option<Stack>], si: usize, dst: &mut [Option<Stack>], di: Option<usize>) -> bool {
    let Some(a) = src.get(si).copied().flatten() else { return false };
    match di {
        None => {
            let left = bag_add(dst, a.item, a.qty);
            src[si] = (left > 0).then_some(Stack { item: a.item, qty: left });
            left < a.qty
        }
        Some(di) if di >= dst.len() => false,
        Some(di) => match dst[di] {
            Some(b) if b.item == a.item => {
                let max = jane_data::catalog().combat.item(a.item).max_stack.max(1);
                let n = a.qty.min(max.saturating_sub(b.qty));
                dst[di] = Some(Stack { item: b.item, qty: b.qty + n });
                src[si] = (a.qty > n).then_some(Stack { item: a.item, qty: a.qty - n });
                n > 0
            }
            b => {
                dst[di] = Some(a);
                src[si] = b;
                true
            }
        },
    }
}

/// Which way a stack goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    In,
    Out,
}

fn shift(cx: &mut Ctx<'_>, seat: Seat, prop: PropId, way: Way, from: u8, to: Option<u8>) {
    if !in_reach(cx, prop) {
        return;
    }
    let key = (cx.zone.id, prop);
    let world = &mut *cx.world;
    let Some(p) = world.players.get_mut(seat.index()) else { return };
    let store = world.stores.entry(key).or_insert_with(|| Box::new(EMPTY));
    let (from, to) = (usize::from(from), to.map(usize::from));
    let moved = match way {
        Way::In if from < BAG_SLOTS => cross(&mut p.bag[..], from, &mut store[..], to),
        Way::Out if from < STORE_SLOTS => cross(&mut store[..], from, &mut p.bag[..], to),
        _ => false,
    };
    tidy(world, key);
    if moved {
        emit_to(cx, seat, EventKind::Bag);
    }
}

/// Bag slot `bag` into the cupboard: onto slot `to`, or wherever it fits.
pub fn put(cx: &mut Ctx<'_>, seat: Seat, prop: PropId, bag: u8, to: Option<u8>) {
    shift(cx, seat, prop, Way::In, bag, to);
}

/// Cupboard slot `slot` into her bag: onto bag slot `to`, or wherever it fits.
pub fn take(cx: &mut Ctx<'_>, seat: Seat, prop: PropId, slot: u8, to: Option<u8>) {
    shift(cx, seat, prop, Way::Out, slot, to);
}

/// One cupboard slot onto another: merge the same thing, else swap.
pub fn arrange(cx: &mut Ctx<'_>, prop: PropId, from: u8, to: u8) {
    if !in_reach(cx, prop) {
        return;
    }
    let key = (cx.zone.id, prop);
    if let Some(s) = cx.world.stores.get_mut(&key) {
        bag_move(&mut s[..], usize::from(from), usize::from(to));
    }
}

/// Everything in her bag that goes, into the cupboard (the window's "Put all away"): each stack
/// in slot order, as a quick move. What does not fit stays in the bag.
pub fn put_all(cx: &mut Ctx<'_>, seat: Seat, prop: PropId) {
    for slot in 0..BAG_SLOTS as u8 {
        shift(cx, seat, prop, Way::In, slot, None);
    }
}
