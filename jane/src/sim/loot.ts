// Ground drops. They live in ZoneState, so they survive zone travel and saves
// (the Phaser build lost every drop on zone leave, and respawned pocket herbs
// on every zone enter: an infinite farm).

import type { World } from "@/sim/runtime";
import type { Drop, Unit } from "@/sim/state";
import { rngFloat } from "@/sim/rng";
import { bagAdd } from "@/sim/inventory";
import { distance } from "@/sim/units";

/** 2020: `obj_drop_parent` lives 18000 frames (5 minutes). */
const DROP_LIFE = 18000;
const PICKUP_REACH = 14;

export function spawnDrop(w: World, item: string, qty: number, x: number, y: number): Drop {
  const d: Drop = { id: w.state.nextId++, item, qty, x, y, age: 0 };
  w.zone.drops.push(d);
  return d;
}

export function rollLoot(w: World, dead: Unit): void {
  const def = w.catalog.units[dead.def];
  let n = 0;
  for (const roll of def.loot) {
    if (rngFloat(w.state.rng) >= roll.chance) continue;
    // Fan drops out so stacked loot is individually visible.
    spawnDrop(w, roll.item, roll.qty, dead.x + (n % 3) * 6 - 6, dead.y + Math.floor(n / 3) * 6);
    n++;
  }
}

export function nearestDrop(w: World, u: Unit): Drop | null {
  let best: Drop | null = null;
  let bestD = PICKUP_REACH;
  for (const d of w.zone.drops) {
    const dist = distance(u.x, u.y, d.x, d.y);
    if (dist <= bestD) {
      bestD = dist;
      best = d;
    }
  }
  return best;
}

export function pickUp(w: World, u: Unit, drop: Drop): boolean {
  const left = bagAdd(w, u, drop.item, drop.qty);
  const got = drop.qty - left;
  if (got === 0) {
    w.emit({ e: "toast", text: "Inventory full" });
    return false;
  }
  w.emit({ e: "loot", item: drop.item, qty: got });
  if (left === 0) w.zone.drops.splice(w.zone.drops.indexOf(drop), 1);
  else drop.qty = left;
  return true;
}

/** Story drops (keys, quest items) never expire; everything else ages out. */
export function stepDrops(w: World): void {
  const list = w.zone.drops;
  for (let i = list.length - 1; i >= 0; i--) {
    const d = list[i];
    if (w.catalog.items[d.item].bound || w.catalog.storyItems.has(d.item)) continue;
    if (++d.age > DROP_LIFE) list.splice(i, 1);
  }
}
