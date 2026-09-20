// Dialogue runner. A tree is data: a list of `start` rules (first match wins)
// and nodes of lines + up to two options, like 2020's dialogue_add_option_2.
//
// A conversation belongs to ONE player (`w.actor`). Alone, the world holds still
// while she talks (Sim.frozen), as it did in 2020. With company nothing pauses:
// she stands there reading while the county carries on around her. Either way
// every transition happens through her commands, which keeps replays exact.

import type { DialogueNode } from "@/sim/catalog";
import type { World } from "@/sim/runtime";
import type { PlayerState } from "@/sim/state";
import { conditionsMet, runActions } from "@/sim/actions";

function me(w: World): PlayerState {
  if (!w.actor) throw new Error("Dialogue needs an acting player");
  return w.actor;
}

export function startDialogue(w: World, treeId: string, speaker: number): boolean {
  const tree = w.catalog.dialogue[treeId];
  if (!tree || !w.actor) return false;
  const entry = tree.start.find((s) => conditionsMet(w, s.when));
  if (!entry) return false;
  w.actor.dialogue = { tree: treeId, node: entry.node, line: 0, speaker };
  w.emit({ e: "dialogue" });
  return true;
}

export function currentNode(w: World): DialogueNode | null {
  const d = w.actor?.dialogue;
  if (!d) return null;
  return w.catalog.dialogue[d.tree].nodes[d.node] ?? null;
}

/** True when the current line is the node's last and it offers a choice. */
export function awaitingChoice(w: World): boolean {
  const d = w.actor?.dialogue;
  const node = currentNode(w);
  if (!d || !node) return false;
  return d.line >= node.lines.length - 1 && (node.options?.length ?? 0) > 0;
}

export function advanceDialogue(w: World): void {
  const d = me(w).dialogue;
  const node = currentNode(w);
  if (!d || !node) return;
  if (d.line < node.lines.length - 1) {
    d.line++;
    w.emit({ e: "dialogue" });
    return;
  }
  if ((node.options?.length ?? 0) > 0) return; // needs chooseOption
  leaveNode(w, node.actions, node.goto);
}

export function chooseOption(w: World, index: number): void {
  const node = currentNode(w);
  if (!node || !awaitingChoice(w)) return;
  const option = node.options?.[index];
  if (!option) return;
  leaveNode(w, [...(node.actions ?? []), ...(option.actions ?? [])], option.goto ?? node.goto);
}

function leaveNode(w: World, actions: DialogueNode["actions"], goto: string | undefined): void {
  const player = me(w);
  const d = player.dialogue;
  if (!d) return;
  if (goto) {
    d.node = goto;
    d.line = 0;
  } else {
    player.dialogue = null;
  }
  // Actions run after the transition so a `talk` or `travel` action wins over the close.
  // The subject of an action (who is healed, who gets the status) is the player, not the speaker.
  if (actions && actions.length > 0) runActions(w, actions, player.unitId);
  w.emit({ e: "dialogue" });
}

export function closeDialogue(w: World): void {
  const player = me(w);
  if (!player.dialogue) return;
  player.dialogue = null;
  w.emit({ e: "dialogue" });
}
