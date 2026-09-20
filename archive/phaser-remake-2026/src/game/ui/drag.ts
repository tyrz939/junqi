import { getItem } from "@/game/systems/catalog";
import { session } from "@/game/systems/session";

export const DRAG2 = 64;

export type HeldKind = "bag" | "bar" | "book";
export type HeldFrom = { kind: HeldKind; index: number };

export function keepItem(id: string): boolean {
  if (id === "julies_letter") return true;
  return Boolean(getItem(id).opens);
}

export function destroyBagSlot(index: number): string | null {
  const slot = session.player.inventory[index];
  const id = slot?.id;
  if (!id) return null;
  if (keepItem(id)) return "You keep that.";
  const name = getItem(id).name;
  slot.id = null;
  slot.qty = 0;
  return `Dropped ${name}`;
}

export function unbindBar(index: number): string {
  session.actionBar[index] = null;
  return "Unbound";
}
