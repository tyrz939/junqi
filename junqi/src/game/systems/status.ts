import { getEffect } from "@/game/systems/catalog";
import type { Unit } from "@/game/entities/Unit";
import type { DamageType, IncomingHit } from "@/game/types";

export type StatusInst = { id: string; t: number; pulse: number };

export function applyStatus(unit: Unit, id: string): void {
  const def = getEffect(id);
  if (def.kind === "heal") {
    unit.enqueueDamage(-(def.amount ?? 25), unit.id, "heal");
    return;
  }
  const found = unit.statuses.find((s) => s.id === id);
  const life = def.duration ?? 6;
  if (found) {
    found.t = Math.max(found.t, life);
    return;
  }
  unit.statuses.push({ id, t: life, pulse: 0 });
  unit.flags[id] = life;
}

export function hasStatus(unit: Unit, id: string): boolean {
  return unit.statuses.some((s) => s.id === id && s.t > 0);
}

export function speedMul(unit: Unit): number {
  if (unit.statuses.some((s) => getEffect(s.id).kind === "root")) return 0;
  let m = 1;
  const slow = unit.statuses.find((s) => getEffect(s.id).kind === "slow");
  if (slow) m *= getEffect(slow.id).amount ?? 0.5;
  if (hasStatus(unit, "haste")) m *= 1.28;
  return m;
}

export function isRooted(unit: Unit): boolean {
  return speedMul(unit) === 0;
}

export function tickStatuses(unit: Unit, dt: number): IncomingHit[] {
  const dots: IncomingHit[] = [];
  for (const row of unit.statuses) {
    row.t -= dt;
    row.pulse += dt;
    const def = getEffect(row.id);
    unit.flags[row.id] = row.t;
    if (def.kind === "dot" && row.pulse >= (def.tick ?? 1)) {
      row.pulse = 0;
      dots.push({
        amount: def.amount ?? 4,
        fromId: null,
        type: (def.school ?? "nature") as DamageType,
      });
    }
  }
  unit.statuses = unit.statuses.filter((s) => s.t > 0);
  for (const key of Object.keys(unit.flags)) {
    if (!unit.statuses.some((s) => s.id === key)) delete unit.flags[key];
  }
  return dots;
}

export function mitigate(unit: Unit, amount: number, type: DamageType): number {
  let next = amount;
  const resist = unit.resist[type] ?? 0;
  if (resist) next *= 1 - resist;
  if (type === "physical" && hasStatus(unit, "stoneskin")) next *= 0.6;
  if (type === "fire" && hasStatus(unit, "wet")) next *= 1.25;
  return Math.max(0, Math.round(next));
}
