import { getSpell } from "@/game/systems/catalog";
import { inventoryAdd } from "@/game/systems/inventory";
import { session } from "@/game/systems/session";

export type ActionWorld = {
  lock?(id: string): void;
  unlock?(id: string): void;
  setSolid?(id: string, on: boolean): void;
  spawn?(id: string): void;
  light?(id: string): void;
  aggro?(id: string): void;
  drain?(id: string): void;
};

export function runActions(actions: string[] | undefined, world?: ActionWorld): string[] {
  const notes: string[] = [];
  for (const raw of actions ?? []) {
    const [kind, rest] = splitAction(raw);
    if (kind === "quest") {
      if (session.quests.acquire(rest)) notes.push(`Quest: ${session.quests.def(rest).name}`);
    } else if (kind === "handin") {
      if (session.quests.handIn(rest, session.player)) {
        notes.push(session.quests.def(rest).completion);
      }
    } else if (kind === "beat") {
      session.beats.complete(rest);
    } else if (kind === "set") {
      const [key, value] = rest.split("=");
      if (key === "dogStep") session.dogStep = Number(value);
      else session.setFlag(key, value ?? true);
    } else if (kind === "give") {
      const [item, qty] = rest.split(",");
      inventoryAdd(session.player, item, Number(qty ?? 1));
    } else if (kind === "location") {
      session.quests.enterLocation(rest);
    } else if (kind === "note") {
      notes.push(rest);
    } else if (kind === "learn") {
      if (!session.player.spellbook.includes(rest)) {
        session.player.spellbook.push(rest);
        const hole = session.actionBar.findIndex((s) => !s);
        session.actionBar[hole >= 0 ? hole : 2] = { source: "spell", id: rest };
        notes.push(`Learned ${getSpell(rest).name}`);
      }
    } else if (kind === "lock") {
      world?.lock?.(rest);
    } else if (kind === "unlock") {
      world?.unlock?.(rest);
    } else if (kind === "solid") {
      const [id, value] = rest.split("=");
      world?.setSolid?.(id, value !== "0");
    } else if (kind === "spawn") {
      world?.spawn?.(rest);
    } else if (kind === "light") {
      world?.light?.(rest);
    } else if (kind === "aggro") {
      world?.aggro?.(rest);
    } else if (kind === "drain") {
      world?.drain?.(rest);
    }
  }
  return notes;
}

function splitAction(raw: string): [string, string] {
  const i = raw.indexOf(":");
  return i < 0 ? [raw, ""] : [raw.slice(0, i), raw.slice(i + 1)];
}
