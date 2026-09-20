import { getEffect, getItem } from "@/game/systems/catalog";
import { applyStatus } from "@/game/systems/status";
import type { Unit } from "@/game/entities/Unit";

export function inventoryCanFit(unit: Unit, itemId: string, qty: number): boolean {
  const item = getItem(itemId);
  let space = 0;
  for (const slot of unit.inventory) {
    if (slot.id === itemId) space += item.maxStack - slot.qty;
    else if (slot.id === null) space += item.maxStack;
  }
  return space >= qty;
}

export function inventoryQuantity(unit: Unit, itemId: string): number {
  return unit.inventory.reduce((sum, slot) => (slot.id === itemId ? sum + slot.qty : sum), 0);
}

export function inventoryAdd(unit: Unit, itemId: string, qty: number): number {
  const item = getItem(itemId);
  let left = qty;
  for (const slot of unit.inventory) {
    if (left <= 0) break;
    if (slot.id === itemId && slot.qty < item.maxStack) {
      const space = item.maxStack - slot.qty;
      const add = Math.min(space, left);
      slot.qty += add;
      left -= add;
    }
  }
  for (const slot of unit.inventory) {
    if (left <= 0) break;
    if (slot.id === null) {
      const add = Math.min(item.maxStack, left);
      slot.id = itemId;
      slot.qty = add;
      left -= add;
    }
  }
  return left;
}

export function inventoryRemove(unit: Unit, itemId: string, qty = 1): boolean {
  let need = qty;
  for (const slot of unit.inventory) {
    if (need <= 0) break;
    if (slot.id !== itemId) continue;
    const take = Math.min(slot.qty, need);
    slot.qty -= take;
    need -= take;
    if (slot.qty <= 0) {
      slot.id = null;
      slot.qty = 0;
    }
  }
  return need === 0;
}

export function inventoryOnCooldown(unit: Unit, itemId: string): boolean {
  return unit.itemCooldowns.some((c) => c.id === itemId);
}

export function tryUseItem(unit: Unit, itemId: string): string {
  if (!unit.alive) return "I can't do that while dead";
  if (unit.gcd > 0) return "Not ready";
  if (inventoryQuantity(unit, itemId) <= 0) return "You don't have that";
  if (inventoryOnCooldown(unit, itemId)) return "On cooldown";
  const item = getItem(itemId);
  if (!item.usable) return "I can't use that";

  if (item.opens) return `key:${item.opens}`;
  if (!item.effect && !item.food && !item.warm) return item.description;
  if (item.effect) applyStatus(unit, item.effect);
  if (item.food) {
    unit.hunger = Math.min(100, unit.hunger + item.food);
    if (item.food >= 20 && item.effect !== "poison_food") applyStatus(unit, "well_fed");
  }
  if (item.warm) unit.warmth = Math.min(100, unit.warmth + item.warm);
  inventoryRemove(unit, itemId, 1);
  unit.itemCooldowns.push({ id: itemId, t: item.cooldown });
  unit.gcd = 1.5;
  const bits: string[] = [];
  if (item.effect) {
    const fx = getEffect(item.effect);
    if (fx.kind === "heal") bits.push(`+${fx.amount ?? 0} HP`);
  }
  if (item.food) bits.push(`+${item.food} food`);
  if (item.warm) bits.push(`+${item.warm} warm`);
  if (bits.length) return bits.join(" · ");
  return `Used ${item.name}`;
}

export function inventorySwap(unit: Unit, a: number, b: number): void {
  if (a === b || a < 0 || b < 0 || a >= unit.inventorySize || b >= unit.inventorySize) return;
  const left = unit.inventory[a];
  const right = unit.inventory[b];
  unit.inventory[a] = { id: right.id, qty: right.qty };
  unit.inventory[b] = { id: left.id, qty: left.qty };
}
