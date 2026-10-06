//! Food that comes back (WORLD.md §4.6, `data/tuning/sim.json` `regrow`). The county's apples
//! are not one-off: an apple tree or a windfall (a prop whose def is `regrow`) that she empties is
//! full again, with what its spawn row held, some days later; and a named larder (`restock`:
//! Julie's pantry chest and fruit bowl) fills again with its few staples, whatever it first held.
//! A dungeon's chest is as it was designed: taken is taken.
//!
//! **When.** Emptying one ([`emptied`], from `interact::open_loot`) writes `Prop::regrow`: the tick
//! it was emptied plus `days` game days, give or take half a day by the zone and the prop's id
//! ([`due_at`]), so the orchard's trees do not all come back in the same minute. The tick moves
//! through a bed's night too, so sleeping brings the day nearer as the clock does.
//!
//! **Where it runs.** Step 12, housekeeping, in every live zone ([`regrow_due`]): a prop whose
//! tick has come is full again, not used, and shown if using it hid it. The runtime keeps the
//! soonest tick of its zone ([`soonest`], `ZoneRuntime::regrow_next`) so the pass looks at the
//! props only when something is due. A zone nobody is in keeps its ticks and fills up the moment
//! she walks in. The state is the world's, so every seat finds the same tree bare or full.

use jane_core::{Stack, Tick, ZoneId};

use crate::ctx::Ctx;
use crate::ids::{PropId, PropIx};
use crate::state::{LootState, Prop, ZoneState};
use crate::sym::of_name;
use crate::tuning::TICKS_PER_DAY;

/// What a food source fills again with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refill {
    /// What its spawn row holds (a `regrow` def: a tree, a windfall).
    Spawn,
    /// A named larder's staples (`regrow.restock`).
    Restock(&'static [Stack]),
}

/// Whether a prop comes back when emptied, and with what.
pub fn refill_of(p: &Prop) -> Option<Refill> {
    let cat = jane_data::catalog();
    let def = cat.story.prop(p.def);
    // Deadwood comes back on the same clock as the apples (`fire.rs`).
    if (def.regrow || def.wood > 0) && p.spawn.is_some() {
        return Some(Refill::Spawn);
    }
    cat.living.tuning.regrow.restock.iter().find(|r| of_name(r.prop) == p.key).map(|r| Refill::Restock(r.loot))
}

/// A 32-bit mix (lowbias32): the jitter of one prop, the same on every machine.
const fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// When a source of zone `zone` emptied at `now` is full again: `days` game days on, give or
/// take half a day by the prop's own id.
pub fn due_at(now: Tick, days: u8, zone: ZoneId, id: PropId) -> Tick {
    let jitter = mix(id.get() ^ (zone.index() as u32).wrapping_mul(0x9e37_79b9)) % TICKS_PER_DAY;
    now.after(Tick(u32::from(days) * TICKS_PER_DAY - TICKS_PER_DAY / 2 + jitter))
}

/// The soonest tick anything in the zone comes back.
pub fn soonest(zone: &ZoneState) -> Option<Tick> {
    zone.props.iter().filter_map(|p| p.regrow).min()
}

/// The prop at `ix` has just been emptied: if it is food that comes back, start its clock.
pub fn emptied(cx: &mut Ctx<'_>, ix: PropIx) {
    let p = &cx.zone.props[ix as usize];
    if refill_of(p).is_none() {
        return;
    }
    let at = due_at(cx.world.tick, cx.cat.living.tuning.regrow.days, cx.zone.id, p.id);
    cx.zone.props[ix as usize].regrow = Some(at);
    cx.rt.regrow_next = Some(cx.rt.regrow_next.map_or(at, |t| t.min(at)));
}

/// Step 12: whatever is due is full again.
pub fn regrow_due(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    if cx.rt.regrow_next.is_none_or(|t| t > now) {
        return;
    }
    for ix in 0..cx.zone.props.len() {
        let p = &cx.zone.props[ix];
        if p.regrow.is_none_or(|t| t > now) {
            continue;
        }
        let hid = cx.cat.story.prop(p.def).hide_when_used && p.hidden;
        let p = &mut cx.zone.props[ix];
        p.regrow = None;
        p.used = false;
        p.loot = match refill_of(p) {
            Some(Refill::Restock(loot)) => LootState::Left(loot.to_vec()),
            _ => LootState::AsSpawned,
        };
        if hid {
            p.hidden = false;
            cx.rt.touch_prop(cx.zone, ix as PropIx);
        }
    }
    cx.rt.regrow_next = soonest(cx.zone);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_is_days_on_give_or_take_half_a_day_and_spread() {
        let now = Tick(1000);
        let mut seen = std::collections::BTreeSet::new();
        for n in 1..200 {
            let at = due_at(now, 3, ZoneId::County, PropId::new(n).unwrap());
            let d = at.0 - now.0;
            assert!((5 * TICKS_PER_DAY / 2..7 * TICKS_PER_DAY / 2).contains(&d), "{d}");
            seen.insert(d / (TICKS_PER_DAY / 24));
        }
        assert!(seen.len() >= 20, "spread over the day, not all at once ({} hours)", seen.len());
    }
}
