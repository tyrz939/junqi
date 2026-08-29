import { getQuest } from "@/game/systems/catalog";
import { inventoryAdd, inventoryQuantity, inventoryRemove } from "@/game/systems/inventory";
import type { Unit } from "@/game/entities/Unit";
import type { QuestDef } from "@/game/types";

export type QuestRun = {
  id: string;
  progress: number[];
};

export class QuestLog {
  active: QuestRun[] = [];
  completed: string[] = [];
  selection = 0;

  acquire(id: string): boolean {
    if (this.has(id) || this.completed.includes(id)) return false;
    const def = getQuest(id);
    this.active.push({ id, progress: def.requirements.map(() => 0) });
    return true;
  }

  has(id: string): boolean {
    return this.active.some((q) => q.id === id);
  }

  def(id: string): QuestDef {
    return getQuest(id);
  }

  slay(id: string, kind?: string): void {
    for (const run of this.active) {
      const def = getQuest(run.id);
      def.requirements.forEach((req, i) => {
        if (req.type !== "kill" || run.progress[i] >= req.qty) return;
        if (req.target === id || (kind && req.target === kind)) run.progress[i] += 1;
      });
    }
  }

  syncAcquire(player: Unit): void {
    for (const run of this.active) {
      const def = getQuest(run.id);
      def.requirements.forEach((req, i) => {
        if (req.type === "acquire") {
          run.progress[i] = Math.min(req.qty, inventoryQuantity(player, req.target));
        }
      });
    }
  }

  enterLocation(target: string): void {
    for (const run of this.active) {
      const def = getQuest(run.id);
      def.requirements.forEach((req, i) => {
        if (req.type === "location" && req.target === target) {
          run.progress[i] = req.qty;
        }
      });
    }
  }

  met(id: string): boolean {
    const run = this.active.find((q) => q.id === id);
    if (!run) return false;
    const def = getQuest(id);
    return def.requirements.every((req, i) => run.progress[i] >= req.qty);
  }

  handIn(id: string, player?: Unit): boolean {
    if (!this.met(id)) return false;
    const def = getQuest(id);
    if (player) {
      for (const req of def.requirements) {
        if (req.type === "acquire") inventoryRemove(player, req.target, req.qty);
      }
      for (const reward of def.rewards ?? []) inventoryAdd(player, reward.id, reward.qty);
    }
    this.active = this.active.filter((q) => q.id !== id);
    this.completed.push(id);
    this.selection = 0;
    if (player) this.syncAcquire(player);
    return true;
  }

  snapshot(): { active: QuestRun[]; completed: string[] } {
    return { active: this.active, completed: this.completed };
  }

  restore(data: { active: QuestRun[]; completed: string[] }): void {
    this.active = data.active ?? [];
    this.completed = data.completed ?? [];
  }
}
