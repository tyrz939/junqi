// Save slots. localStorage, three slots, each a versioned SaveFile (sim/save.ts).
// Every call is wrapped: private windows and blocked site data throw on access,
// and a corrupt slot must read as "empty", never as a crash on the title screen.

import { decodeSave, encodeSave, type SaveFile } from "@/sim/save";
import type { Sim } from "@/sim/sim";
import { TICKS_PER_HOUR } from "@/sim/constants";
import { maxHp } from "@/sim/units";
import type { SlotInfo } from "@/ui/host";

export const SLOT_COUNT = 3;
const key = (slot: number): string => `jane.save.${slot}`;
/** Where slots lived before the project was renamed. Read once, then moved. */
const oldKey = (slot: number): string => `junqi.save.${slot}`;

export function writeSlot(slot: number, sim: Sim): boolean {
  const state = sim.prepareSave();
  const p = sim.player;
  const text = encodeSave(
    state,
    { zone: sim.rt.bp.name, day: state.day, hour: state.clock / TICKS_PER_HOUR, hp: Math.round(p.hp), maxhp: maxHp(p) },
    new Date(),
  );
  try {
    localStorage.setItem(key(slot), text);
    return true;
  } catch {
    return false;
  }
}

export function readSlot(slot: number): SaveFile | null {
  try {
    let text = localStorage.getItem(key(slot));
    if (!text) {
      text = localStorage.getItem(oldKey(slot));
      if (!text) return null;
      localStorage.setItem(key(slot), text);
      localStorage.removeItem(oldKey(slot));
    }
    const result = decodeSave(text);
    return result.ok ? result.file : null;
  } catch {
    return null;
  }
}

export function slotInfos(): (SlotInfo | null)[] {
  const out: (SlotInfo | null)[] = [];
  for (let slot = 0; slot < SLOT_COUNT; slot++) {
    const f = readSlot(slot);
    out.push(f ? { slot, savedAt: f.savedAt, ...f.summary } : null);
  }
  return out;
}

export function clearSlot(slot: number): void {
  try {
    localStorage.removeItem(key(slot));
    localStorage.removeItem(oldKey(slot));
  } catch {
    /* nothing to clear */
  }
}
