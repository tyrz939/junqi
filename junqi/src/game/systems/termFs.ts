import dialogue from "@/data/dialogue.json";
import pockets from "@/data/pockets.json";
import triggers from "@/data/triggers.json";
import {
  beatSpine,
  effectCatalog,
  enemyCatalog,
  itemCatalog,
  questCatalog,
  recipeCatalog,
  spellCatalog,
} from "@/game/systems/catalog";
import { session } from "@/game/systems/session";

export type VNode = { kind: "dir"; kids: Record<string, VNode> } | { kind: "file"; text: () => string };

export function formatPath(parts: string[]): string {
  return parts.length ? `\\${parts.join("\\")}` : "\\";
}

export function parsePath(raw: string): string[] {
  return raw.replace(/\\/g, "/").split("/").filter((p) => p && p !== ".");
}

export function resolvePath(cwd: string[], raw: string): string[] | null {
  const absolute = raw.startsWith("\\") || raw.startsWith("/");
  const stack = absolute ? [] : [...cwd];
  for (const part of parsePath(raw)) {
    if (part === "..") {
      stack.pop();
      continue;
    }
    stack.push(part);
  }
  return stack;
}

export function walk(root: VNode, parts: string[]): VNode | null {
  let node: VNode = root;
  for (const part of parts) {
    if (node.kind !== "dir") return null;
    const hit = matchKid(node, part);
    if (!hit) return null;
    node = hit;
  }
  return node;
}

function matchKid(dir: Extract<VNode, { kind: "dir" }>, name: string): VNode | undefined {
  if (name in dir.kids) return dir.kids[name];
  const lower = name.toLowerCase();
  return Object.entries(dir.kids).find(([key]) => key.toLowerCase() === lower)?.[1];
}

export function buildFs(binNames: string[]): VNode {
  const inspect = session.inspectWorld?.();
  return dir({
    bin: files(Object.fromEntries(binNames.map((name) => [name, () => `${name} — see help ${name}`]))),
    data: dir({
      items: jsonDir(itemCatalog),
      spells: jsonDir(spellCatalog),
      quests: jsonDir(questCatalog),
      effects: jsonDir(effectCatalog),
      enemies: jsonDir(enemyCatalog),
      beats: jsonDir(Object.fromEntries(beatSpine.map((b) => [b.id, b]))),
      recipes: file(() => JSON.stringify(recipeCatalog, null, 2)),
      triggers: jsonDir(triggers as Record<string, unknown>),
      dialogue: jsonDir(dialogue as Record<string, unknown>),
      pockets: jsonDir(pockets as Record<string, unknown>),
    }),
    proc: dir({
      player: file(() => dumpPlayer(inspect)),
      session: file(() =>
        JSON.stringify(
          {
            zone: session.zone,
            time: Number(session.time.toFixed(3)),
            seed: session.seed.toString(16),
            god: session.god,
            debug: session.debug,
            started: session.started,
            visited: session.visited,
            carrying: session.carrying,
            dogStep: session.dogStep,
          },
          null,
          2,
        ),
      ),
      bag: file(() => dumpBag()),
      quests: file(() => JSON.stringify(session.quests.snapshot(), null, 2)),
      beats: file(() => JSON.stringify({ have: session.beats.list(), now: session.beats.debugLine() }, null, 2)),
      flags: file(() => JSON.stringify(session.flags, null, 2)),
      zone: file(() => dumpZone(inspect)),
    }),
  });
}

export function completePath(cwd: string[], partial: string, binNames: string[]): string[] {
  const slash = partial.lastIndexOf("\\") >= 0 ? "\\" : partial.includes("/") ? "/" : "";
  const cut = Math.max(partial.lastIndexOf("\\"), partial.lastIndexOf("/"));
  const prefix = cut >= 0 ? partial.slice(0, cut + 1) : "";
  const leaf = cut >= 0 ? partial.slice(cut + 1) : partial;
  const dest = resolvePath(cwd, prefix || (partial.startsWith("\\") || partial.startsWith("/") ? "\\" : "."));
  if (!dest) return [];
  const node = walk(buildFs(binNames), dest);
  if (!node || node.kind !== "dir") return [];
  return Object.keys(node.kids)
    .filter((name) => name.toLowerCase().startsWith(leaf.toLowerCase()))
    .map((name) => prefix + name + (node.kids[name].kind === "dir" ? (slash || "\\") : ""));
}

function dir(kids: Record<string, VNode>): VNode {
  return { kind: "dir", kids };
}

function file(text: () => string): VNode {
  return { kind: "file", text };
}

function files(map: Record<string, () => string>): VNode {
  return dir(Object.fromEntries(Object.entries(map).map(([k, text]) => [k, file(text)])));
}

function jsonDir(map: Record<string, unknown>): VNode {
  return dir(Object.fromEntries(Object.entries(map).map(([k, v]) => [k, file(() => JSON.stringify(v, null, 2))])));
}

function dumpPlayer(inspect: ReturnType<NonNullable<typeof session.inspectWorld>> | undefined): string {
  const p = session.player;
  return JSON.stringify(
    {
      id: p.id,
      name: p.name,
      zone: session.zone,
      xy: inspect?.player ?? null,
      hp: `${Math.round(p.hp)}/${p.maxhp}`,
      mp: `${Math.round(p.mp)}/${p.maxmp}`,
      energy: `${Math.round(p.energy)}/${p.maxenergy}`,
      god: session.god,
      statuses: p.statuses.map((s) => s.id),
      spells: p.spellbook,
      target: p.currentTargetId,
    },
    null,
    2,
  );
}

function dumpBag(): string {
  return session.player.inventory
    .map((slot, i) => (slot.id ? `[${i}] ${slot.id} x${slot.qty}` : `[${i}]`))
    .join("\n");
}

function dumpZone(inspect: ReturnType<NonNullable<typeof session.inspectWorld>> | undefined): string {
  if (!inspect) return "no world (title)";
  return JSON.stringify(
    {
      zone: inspect.zone,
      actors: inspect.actors.length,
      props: inspect.props.length,
      bolts: inspect.bolts,
      asleep: inspect.asleep,
      chunks: inspect.chunks,
      units: inspect.actors,
      propsList: inspect.props,
    },
    null,
    2,
  );
}

export function listNames(node: VNode): string[] {
  if (node.kind !== "dir") return [];
  return Object.keys(node.kids).sort();
}
