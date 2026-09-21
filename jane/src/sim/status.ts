// Statuses are effect rows on a unit. 2020 had one `speed_multiplier` + Alarm 1
// that only the player's movement read, so AI could never be slowed or stunned.
// Here every unit reads the same modifiers, because every unit is the same Unit.

import type { EffectDef } from "@/sim/catalog";
import type { World } from "@/sim/runtime";
import type { School, Unit } from "@/sim/state";
import { maxHp, maxMp } from "@/sim/units";

export function hasStatus(u: Unit, effect: string): boolean {
  for (const s of u.statuses) if (s.effect === effect) return true;
  return false;
}

/**
 * Apply an effect row. Instant parts land now; timed parts refresh rather than
 * stack (a second Winterbite potion resets the clock, it does not double it).
 */
export function applyEffect(w: World, u: Unit, effectId: string, from: number): void {
  const def: EffectDef = w.catalog.effects[effectId];
  if (!u.alive) return;
  // Some effects only take on what is weak to them: a spark jolts a machine, not a rat.
  if (def.onlyIfWeak !== undefined && (w.catalog.units[u.def].resist?.[def.onlyIfWeak] ?? 0) >= 0) return;
  if (def.heal) u.incoming.push({ amount: def.heal, school: "heal", from, crit: false });
  if (def.mana) u.mp = Math.min(maxMp(u), u.mp + def.mana);
  if (def.duration <= 0) return;
  const existing = u.statuses.find((s) => s.effect === effectId);
  if (existing) {
    existing.left = def.duration;
    existing.from = from;
    return;
  }
  u.statuses.push({
    effect: effectId,
    left: def.duration,
    nextTick: def.pulse ? def.pulse.every : 0,
    from,
    pool: 0,
  });
  w.emit({ e: "status", unit: u.id, effect: effectId, on: true });
}

export function clearStatuses(w: World, u: Unit): void {
  for (const s of u.statuses) w.emit({ e: "status", unit: u.id, effect: s.effect, on: false });
  u.statuses.length = 0;
}

/** Count down, pulse DoTs/HoTs into the incoming queue, drop what expired. */
export function tickStatuses(w: World, u: Unit): void {
  if (u.statuses.length === 0) return;
  for (let i = u.statuses.length - 1; i >= 0; i--) {
    const s = u.statuses[i];
    const def = w.catalog.effects[s.effect];
    if (def.pulse) {
      if (--s.nextTick <= 0) {
        s.nextTick = def.pulse.every;
        u.incoming.push({ amount: def.pulse.amount, school: def.pulse.school, from: s.from, crit: false });
      }
    }
    if (--s.left <= 0) {
      u.statuses.splice(i, 1);
      w.emit({ e: "status", unit: u.id, effect: s.effect, on: false });
    }
  }
}

/** Product of every speed modifier. 0 means rooted. */
export function speedFactor(w: World, u: Unit): number {
  let f = 1;
  for (const s of u.statuses) {
    const def = w.catalog.effects[s.effect];
    if (def.speed !== undefined) f *= def.speed;
    if (def.stun) f = 0;
  }
  return f;
}

export function isStunned(w: World, u: Unit): boolean {
  for (const s of u.statuses) if (w.catalog.effects[s.effect].stun) return true;
  return false;
}

/** Incoming damage multiplier for a school: unit row resist, then each status resist. */
export function resistFactor(w: World, u: Unit, school: School): number {
  let own = w.catalog.units[u.def].resist?.[school] ?? 0;
  let f = 1;
  for (const s of u.statuses) {
    const e = w.catalog.effects[s.effect];
    const r = e.resist?.[school];
    if (r !== undefined) f *= r;
    // Softened: what it was proof against, it is not. What it was weak to, it still is.
    if (e.noResist && own > 0) own = 0;
  }
  return Math.max(0, f * (1 - own));
}

export type OffenceMods = { lifesteal: number; critOneIn: number; onMelee: EffectDef["onMelee"][] };

export function offenceMods(w: World, u: Unit): OffenceMods {
  const mods: OffenceMods = { lifesteal: 0, critOneIn: 0, onMelee: [] };
  for (const s of u.statuses) {
    const def = w.catalog.effects[s.effect];
    if (def.lifesteal) mods.lifesteal += def.lifesteal;
    if (def.critOneIn && (mods.critOneIn === 0 || def.critOneIn < mods.critOneIn)) mods.critOneIn = def.critOneIn;
    if (def.onMelee) mods.onMelee.push(def.onMelee);
  }
  return mods;
}

export type DefenceMods = { manaShield: number; manaOnHit: number };

export function defenceMods(w: World, u: Unit): DefenceMods {
  const mods: DefenceMods = { manaShield: 0, manaOnHit: 0 };
  for (const s of u.statuses) {
    const def = w.catalog.effects[s.effect];
    if (def.manaShield && (mods.manaShield === 0 || def.manaShield < mods.manaShield)) mods.manaShield = def.manaShield;
    if (def.manaOnHit) mods.manaOnHit += def.manaOnHit;
  }
  return mods;
}

export function healthFraction(u: Unit): number {
  return u.hp / maxHp(u);
}
