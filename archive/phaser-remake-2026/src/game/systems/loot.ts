import type { LootRoll } from "@/game/types";

export function rollLoot(table: LootRoll[] | undefined): { id: string; qty: number }[] {
  const out: { id: string; qty: number }[] = [];
  for (const row of table ?? []) {
    if (Math.random() <= row.chance) out.push({ id: row.id, qty: row.qty });
  }
  return out;
}
