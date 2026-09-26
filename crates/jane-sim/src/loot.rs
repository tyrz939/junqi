//! Ground drops (`sim/loot.ts`). They live in `ZoneState`, so they survive zone travel and
//! saves (the Phaser build lost every drop on zone leave, and respawned pocket herbs on every
//! zone enter: an infinite farm). Picking one up is the interact unit's.

use jane_core::{Fx, ItemId, UnitDefId, Vec2};

use crate::ctx::Ctx;
use crate::ids::DropId;
use crate::state::Drop;
use crate::tuning::{DROP_LIFE, LOOT_SPREAD_FX};

/// Put a stack on the ground here.
pub fn spawn_drop(cx: &mut Ctx<'_>, item: ItemId, qty: u16, pos: Vec2) -> DropId {
    let id = cx.world.next.drop();
    cx.zone.drops.push(Drop { id, item, qty, pos, born: cx.world.tick });
    id
}

/// A creature's loot rolls, each by its own chance from the zone's dice, fanned out three to a
/// row so stacked loot is each visible.
pub fn roll_loot(cx: &mut Ctx<'_>, def: UnitDefId, at: Vec2) {
    let mut n = 0i32;
    for roll in cx.cat.combat.unit(def).loot {
        if !cx.zone.rng.chance(roll.chance) {
            continue;
        }
        let pos =
            Vec2::new(Fx(at.x.0 + (n % 3) * LOOT_SPREAD_FX - LOOT_SPREAD_FX), Fx(at.y.0 + (n / 3) * LOOT_SPREAD_FX));
        spawn_drop(cx, roll.item, roll.qty, pos);
        n += 1;
    }
}

/// Step 12: what nobody picked up ages out after five minutes. What the story needs (anything
/// that opens something, anything a quest asks for) and what is bound never does.
pub fn step_drops(cx: &mut Ctx<'_>) {
    let now = cx.world.tick;
    let cat = cx.cat;
    cx.zone.drops.retain(|d| {
        let def = cat.combat.item(d.item);
        def.bound || def.story || now.since(d.born) < DROP_LIFE
    });
}
