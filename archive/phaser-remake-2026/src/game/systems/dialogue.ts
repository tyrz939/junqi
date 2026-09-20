import dialogue from "@/data/dialogue.json";
import { getQuest } from "@/game/systems/catalog";
import { session } from "@/game/systems/session";
import { getNpc } from "@/game/world/generateTowns";

export { runActions } from "@/game/systems/actions";

export type DialogueOption = { label: string; actions?: string[]; next?: string };
export type DialogueNode = { lines: string[]; options?: DialogueOption[] };
export type DialogueStart = { when?: string; node: string };
export type DialogueTree = { start: DialogueStart[]; nodes: Record<string, DialogueNode> };

export const dialogueCatalog = dialogue as Record<string, DialogueTree>;

export function pickNode(id: string): DialogueNode | null {
  const tree = dialogueCatalog[id] ?? npcTree(id) ?? boardTree(id);
  if (!tree) return null;
  for (const row of tree.start) {
    if (!row.when || matchWhen(row.when)) return tree.nodes[row.node] ?? null;
  }
  return null;
}

function npcTree(id: string): DialogueTree | null {
  const npc = getNpc(id);
  if (!npc) return null;
  const q = npc.quest;
  return {
    start: [
      { when: `questMet:${q}`, node: "reward" },
      { when: `questHas:${q}`, node: "wait" },
      { node: "ask" },
    ],
    nodes: {
      ask: {
        lines: npc.ask,
        options: [
          { label: "I'll do it", actions: [`quest:${q}`] },
          { label: "Not now" },
        ],
      },
      wait: { lines: ["The log still has my name on it. Check it."] },
      reward: {
        lines: npc.done,
        options: [{ label: "Here", actions: [`handin:${q}`] }],
      },
    },
  };
}

function boardTree(id: string): DialogueTree | null {
  if (!id.startsWith("board_")) return null;
  const town = id.slice(6);
  const hunts: Record<string, string[]> = {
    castle_cross: ["visit_rivermill", "hunt_conscripts", "fetch_cheese"],
    rivermill: ["visit_acreton", "hunt_sewer", "fetch_wheat"],
    acreton: ["visit_kiln", "hunt_foxes", "fetch_potato"],
    kiln_end: ["visit_boneford", "hunt_ash_pumpkin", "fetch_salt"],
    boneford: ["visit_ridge", "hunt_captain", "fetch_tea_leaf"],
    ridgegate: ["visit_rivermill", "hunt_gold_rats", "fetch_honey"],
  };
  const list = hunts[town] ?? ["fetch_berry"];
  const names = list.map((q) => {
    try {
      return getQuest(q).name;
    } catch {
      return q;
    }
  });
  return {
    start: [{ node: "read" }],
    nodes: {
      read: {
        lines: [`TOWN BOARD  ·  ${town.replace(/_/g, " ")}`, `Posted: ${names.join(" · ")}`],
        options: [
          { label: "I'll take the posts", actions: list.map((q) => `quest:${q}`) },
          { label: "Just looking" },
        ],
      },
    },
  };
}

function matchWhen(when: string): boolean {
  if (when.startsWith("questMet:")) return session.quests.met(when.slice(9));
  if (when.startsWith("questHas:")) return session.quests.has(when.slice(9));
  if (when.startsWith("flag:")) {
    const body = when.slice(5);
    const eq = /^(\w+)==(.+)$/.exec(body);
    if (!eq) return session.flag(body);
    if (eq[1] === "dogStep") return session.dogStep === Number(eq[2]);
    return String(session.flags[eq[1]] ?? "") === eq[2];
  }
  return false;
}

