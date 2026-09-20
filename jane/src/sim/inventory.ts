// Bags: 24 slots of { item, qty } | null. Stack into existing stacks first, then
// the first hole; whatever does not fit is returned, never silently dropped
// (the 2026 Phaser build ignored the leftover and could lose the house key).

import { CRAFT_INPUTS, GCD_TICKS } from "@/sim/constants";
import { recipeKey } from "@/sim/catalog";
import type { World } from "@/sim/runtime";
import type { Stack, Unit } from "@/sim/state";
import { runActions } from "@/sim/actions";
import { isStunned } from "@/sim/status";
import { maxHp } from "@/sim/units";

/** Returns the quantity that did NOT fit. */
export function bagAdd(w: World, u: Unit, item: string, qty: number): number {
  if (!u.bag || qty <= 0) return qty;
  const max = w.catalog.items[item].maxStack;
  let left = qty;
  for (const s of u.bag) {
    if (left === 0) break;
    if (s && s.item === item && s.qty < max) {
      const put = Math.min(left, max - s.qty);
      s.qty += put;
      left -= put;
    }
  }
  for (let i = 0; i < u.bag.length && left > 0; i++) {
    if (u.bag[i] === null) {
      const put = Math.min(left, max);
      u.bag[i] = { item, qty: put };
      left -= put;
    }
  }
  if (left < qty) w.emit({ e: "bag" });
  return left;
}

export function bagCount(u: Unit, item: string): number {
  let n = 0;
  if (u.bag) for (const s of u.bag) if (s && s.item === item) n += s.qty;
  return n;
}

/** Removes up to qty, last stacks first. Returns how many were removed. */
export function bagRemove(w: World, u: Unit, item: string, qty: number): number {
  if (!u.bag) return 0;
  let left = qty;
  for (let i = u.bag.length - 1; i >= 0 && left > 0; i--) {
    const s = u.bag[i];
    if (!s || s.item !== item) continue;
    const take = Math.min(left, s.qty);
    s.qty -= take;
    left -= take;
    if (s.qty === 0) u.bag[i] = null;
  }
  if (left < qty) w.emit({ e: "bag" });
  return qty - left;
}

export function bagHasRoom(w: World, u: Unit, item: string, qty: number): boolean {
  if (!u.bag) return false;
  const max = w.catalog.items[item].maxStack;
  let room = 0;
  for (const s of u.bag) {
    if (s === null) room += max;
    else if (s.item === item) room += max - s.qty;
    if (room >= qty) return true;
  }
  return false;
}

/** Drag from one bag slot to another: merge same items, otherwise swap. */
export function bagMove(w: World, u: Unit, from: number, to: number): void {
  if (!u.bag || from === to || !inRange(u.bag, from) || !inRange(u.bag, to)) return;
  const a = u.bag[from];
  const b = u.bag[to];
  if (!a) return;
  if (b && b.item === a.item) {
    const max = w.catalog.items[a.item].maxStack;
    const put = Math.min(a.qty, max - b.qty);
    b.qty += put;
    a.qty -= put;
    if (a.qty === 0) u.bag[from] = null;
  } else {
    u.bag[from] = b;
    u.bag[to] = a;
  }
  w.emit({ e: "bag" });
}

/** Drag out of the window. Bound items (keys in use by the story, Julie's letter) refuse. */
export function bagDestroy(w: World, u: Unit, slot: number): boolean {
  if (!u.bag || !inRange(u.bag, slot)) return false;
  const s = u.bag[slot];
  if (!s) return false;
  if (w.catalog.items[s.item].bound) {
    w.emit({ e: "toast", text: "I should keep that" });
    return false;
  }
  u.bag[slot] = null;
  w.emit({ e: "bag" });
  return true;
}

/**
 * Use the first stack of `item`. 2020 rules: needs GCD clear, starts GCD and the
 * per-item cooldown, roots the user for half a second. Keys are not used from
 * the bag; doors ask for them (interact.ts), one path for every key.
 */
export function useItem(w: World, u: Unit, item: string): boolean {
  const def = w.catalog.items[item];
  if (!def || !u.alive || !u.bag) return false;
  if (bagCount(u, item) === 0) return false;
  if (!def.usable || !def.use) {
    if (def.opens) w.emit({ e: "toast", text: "It fits a lock somewhere" });
    return false;
  }
  if (isStunned(w, u)) return false;
  if (u.gcd > 0 || (u.itemCooldowns[item] ?? 0) > 0) return false;
  if (def.use.length === 1 && def.use[0].do === "heal" && u.hp >= maxHp(u)) {
    w.emit({ e: "toast", text: "I'm not hurt" });
    return false;
  }
  runActions(w, def.use, u.id);
  if (!def.keep) bagRemove(w, u, item, 1);
  if (def.cooldown > 0) u.itemCooldowns[item] = def.cooldown;
  u.gcd = GCD_TICKS;
  u.stop = Math.max(u.stop, 30);
  w.emit({ e: "bag" });
  return true;
}

// --- Crafting: 3 inputs + 1 output, keyed by sorted item ids ---------------

/** The craft row belongs to whoever is standing at the bench. */
function craftOf(w: World): (Stack | null)[] {
  if (!w.actor) throw new Error("Crafting needs an acting player");
  return w.actor.craft;
}

export function craftOutput(w: World): { item: string; qty: number } | null {
  const inputs: string[] = [];
  for (const s of craftOf(w)) if (s) inputs.push(s.item);
  if (inputs.length === 0) return null;
  const r = w.catalog.recipeIndex[recipeKey(inputs)];
  return r ? { item: r.output, qty: r.qty } : null;
}

/** Move one unit of a bag stack onto a craft input slot. */
export function craftPut(w: World, u: Unit, bagSlot: number, craftSlot: number): void {
  if (!u.bag || !inRange(u.bag, bagSlot) || craftSlot < 0 || craftSlot >= CRAFT_INPUTS) return;
  const s = u.bag[bagSlot];
  if (!s || craftOf(w)[craftSlot]) return;
  craftOf(w)[craftSlot] = { item: s.item, qty: 1 };
  s.qty -= 1;
  if (s.qty === 0) u.bag[bagSlot] = null;
  w.emit({ e: "bag" });
}

export function craftClear(w: World, u: Unit, craftSlot: number): void {
  const s = craftOf(w)[craftSlot];
  if (!s) return;
  if (bagAdd(w, u, s.item, s.qty) === 0) craftOf(w)[craftSlot] = null;
  w.emit({ e: "bag" });
}

export function craftClearAll(w: World, u: Unit): void {
  for (let i = 0; i < craftOf(w).length; i++) craftClear(w, u, i);
}

/** Take the output: inputs are consumed only if the result fits in the bag. */
export function craftTake(w: World, u: Unit): boolean {
  const out = craftOutput(w);
  if (!out) return false;
  if (!bagHasRoom(w, u, out.item, out.qty)) {
    w.emit({ e: "toast", text: "Inventory full" });
    return false;
  }
  for (let i = 0; i < craftOf(w).length; i++) craftOf(w)[i] = null;
  bagAdd(w, u, out.item, out.qty);
  w.emit({ e: "loot", item: out.item, qty: out.qty });
  return true;
}

function inRange(list: readonly unknown[], i: number): boolean {
  return Number.isInteger(i) && i >= 0 && i < list.length;
}

export type { Stack };
