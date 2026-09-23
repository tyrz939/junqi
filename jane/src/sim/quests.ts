// Three requirement types, no more: kill, acquire, location.
//
// Two fixes over both earlier builds:
//  - `acquire` is read live from the bag, so it cannot desync from what you hold.
//  - `location` reads a persistent "been here" flag, so visiting a place before
//    you take the quest still counts. (The Phaser build credited locations only
//    to quests active at that instant; a once-only trigger then lost it forever.)

import type { QuestDef } from "@/sim/catalog";
import type { World } from "@/sim/runtime";
import type { QuestProgress, Unit } from "@/sim/state";
import { bagCount } from "@/sim/inventory";
import { runActions } from "@/sim/actions";
import { uncoverAll } from "@/sim/under";

export function questActive(w: World, id: string): QuestProgress | undefined {
  return w.state.quests.active.find((q) => q.quest === id);
}

export function questDone(w: World, id: string): boolean {
  return w.state.quests.done.includes(id);
}

export function locationFlag(name: string): string {
  return `been:${name}`;
}

/** Progress for requirement `i`, clamped to its qty. */
export function requirementCount(w: World, def: QuestDef, prog: QuestProgress, i: number): number {
  const r = def.requirements[i];
  let n = prog.counts[i] ?? 0;
  // One story, one log: what the party holds between them counts.
  if (r.type === "acquire") {
    n = 0;
    for (const u of w.party.units()) n += bagCount(u, r.target);
  }
  else if (r.type === "location") n = w.state.flags[locationFlag(r.target)] ? 1 : 0;
  return Math.min(r.qty, n);
}

export function questReady(w: World, id: string): boolean {
  const prog = questActive(w, id);
  if (!prog) return false;
  const def = w.catalog.quests[id];
  for (let i = 0; i < def.requirements.length; i++) {
    if (requirementCount(w, def, prog, i) < def.requirements[i].qty) return false;
  }
  return true;
}

export function giveQuest(w: World, id: string): boolean {
  if (questActive(w, id) || questDone(w, id)) return false;
  const def = w.catalog.quests[id];
  w.state.quests.active.push({ quest: id, counts: def.requirements.map(() => 0) });
  // Quest news goes to the whole party, wherever they are.
  w.emit({ e: "quest", quest: id, change: "given" }, true);
  w.emit({ e: "toast", text: `Quest: ${def.name}` }, true);
  if (questReady(w, id)) w.emit({ e: "quest", quest: id, change: "ready" }, true);
  // A stone already pushed off the spot before she was told what is under it: it is there now.
  uncoverAll(w);
  return true;
}

/** A kill only counts for quests already in the log, exactly as 2020's quest_unit hook. */
export function onUnitKilled(w: World, victim: Unit): void {
  for (const prog of w.state.quests.active) {
    const def = w.catalog.quests[prog.quest];
    const wasReady = questReady(w, prog.quest);
    let touched = false;
    def.requirements.forEach((r, i) => {
      if (r.type !== "kill" || r.target !== victim.def) return;
      if ((prog.counts[i] ?? 0) >= r.qty) return;
      prog.counts[i] = (prog.counts[i] ?? 0) + 1;
      touched = true;
      w.emit({ e: "toast", text: `${r.text}: ${prog.counts[i]}/${r.qty}` }, true);
    });
    if (touched) {
      w.emit({ e: "quest", quest: prog.quest, change: "progress" }, true);
      if (!wasReady && questReady(w, prog.quest)) w.emit({ e: "quest", quest: prog.quest, change: "ready" }, true);
    }
  }
}

export function onLocation(w: World, name: string): void {
  const flag = locationFlag(name);
  if (w.state.flags[flag]) return;
  w.state.flags[flag] = 1;
  for (const prog of w.state.quests.active) {
    const def = w.catalog.quests[prog.quest];
    if (def.requirements.some((r) => r.type === "location" && r.target === name)) {
      w.emit({ e: "quest", quest: prog.quest, change: questReady(w, prog.quest) ? "ready" : "progress" }, true);
    }
  }
}

/** Hand in: only when every requirement is met. Rewards are an action list, not a hardcoded grant. */
export function handIn(w: World, id: string): boolean {
  if (!questReady(w, id)) return false;
  const def = w.catalog.quests[id];
  w.state.quests.active = w.state.quests.active.filter((q) => q.quest !== id);
  w.state.quests.done.push(id);
  w.emit({ e: "quest", quest: id, change: "done" }, true);
  w.emit({ e: "toast", text: `Done: ${def.name}` }, true);
  // Everyone in the party is paid, wherever she is standing: nobody is left without the
  // house key because a friend did the talking. (Rewards are give / quest / flag rows;
  // running a `quest` or `flag` row once per player is harmless.)
  w.party.each((pw, player) => runActions(pw, def.rewards, player.unitId));
  return true;
}
