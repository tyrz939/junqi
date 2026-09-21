// Blueprint validation: a lock-and-key solver. The 2020 design folder holds a
// "Dungeon Graph Making Tools" kit (entrance -> small key -> locked door -> boss
// key -> boss door -> BOSS); this is that graph, checked by machine on every seed.
//
// It plays the zone as a flood fill: walk everywhere reachable, pick up every key
// in reach (chests, guaranteed unit drops), open every gate a held key fits, fire
// every lever / plate / repairable / kill-trigger in reach, repeat until nothing
// changes. A zone is valid when the contract (required keys, marks, rects) exists,
// nothing spawns inside a wall, and everything required is reached.
//
// A failed candidate is re-rolled, never thrown at the player. The 2026 Phaser
// build threw on a bad seed and spawned Jane on a mountain tile on a good one.
//
// What it knows beyond keys (DUNGEONS.md 2.6):
//   verbs     With `verbs` given, a prop that answers Repair, Grow or a school fires only
//             once a spell of that kind is known. Spells are learned from `learn` rows in a
//             reached prop's `use` list or `talk` tree. Without `verbs` nothing is gated,
//             which is how the older zones are still judged.
//   when      A `while` trigger fires only when its conditions can hold: a flag some fired
//             list has set, a unit that was reached (or spawned by a reached trigger onto a
//             reached mark). A negated condition never blocks: it held earlier if at all.
//   hops      A `to` that names this zone is a one-way edge to its mark.
//   ablation  `withhold` takes one thing away (a key tag, a spell, a flag, a prop's list) so
//             a caller can prove a lock really holds; `shut` keeps a gate closed.

import { actionRowErrors, type Catalog, type TriggerDef } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import type { Action, ActionList, Condition } from "@/sim/state";
import type { Blueprint, PropSpawn, ZoneContract } from "@/world/blueprint";

export type SolveOptions = {
  /** Spells known on arrival. Leave it out and nothing is gated on knowing a spell. */
  verbs?: readonly string[];
  /** Ablation: things the player is never given, to prove that the lock they open holds without them. */
  withhold?: { keys?: readonly string[]; verbs?: readonly string[]; flags?: readonly string[]; props?: readonly string[] };
  /** Props that stay shut whatever is unlocked: "could she get here with this gate down?" */
  shut?: readonly string[];
  /** Flood from this mark instead of the zone's entrance. */
  entry?: string;
  /** Keep the order things happened in. Costs two bytes a cell, so the county does not ask for it. */
  trace?: boolean;
};

export type SolveTrace = {
  /** Floods run. */
  passes: number;
  /** For each cell, the first flood that reached it, or -1. */
  firstSeen: Int16Array;
  /** The flood after which each prop was opened, looted, fired or read. */
  firedAt: Record<string, number>;
  /** Spells known at the end. Empty when nothing was gated. */
  verbs: string[];
  flags: Record<string, number>;
  dead: string[];
};

export type Validation = { ok: boolean; errors: string[]; reachedCells: number; trace?: SolveTrace };

/** The catalog's trigger rows for this zone, then the blueprint's own. Same merge as the sim's. */
function triggersOf(bp: Blueprint, catalog: Catalog, errors: string[]): [string, TriggerDef][] {
  const out: [string, TriggerDef][] = [];
  for (const [id, t] of Object.entries(catalog.triggers)) if (t.zone === bp.zone) out.push([id, t]);
  for (const [id, t] of Object.entries(bp.triggers ?? {})) {
    if (id in catalog.triggers) errors.push(`trigger "${id}" is in the blueprint and in the catalog`);
    else out.push([id, t]);
  }
  return out;
}

export function validateBlueprint(
  bp: Blueprint,
  catalog: Catalog,
  contract: ZoneContract,
  givenKeys: readonly string[] = [],
  opts: SolveOptions = {},
): Validation {
  const errors: string[] = [];
  const unitKeys = new Set(bp.units.map((u) => u.key));
  const propKeys = new Map(bp.props.map((p) => [p.key, p]));
  for (const k of contract.units) if (!unitKeys.has(k)) errors.push(`missing unit "${k}"`);
  for (const k of contract.props) if (!propKeys.has(k)) errors.push(`missing prop "${k}"`);
  for (const k of contract.marks) if (!bp.marks[k]) errors.push(`missing mark "${k}"`);
  for (const k of contract.rects) if (!bp.rects[k]) errors.push(`missing rect "${k}"`);
  if (bp.props.length !== propKeys.size) errors.push("duplicate prop keys");
  if (bp.units.length !== unitKeys.size) errors.push("duplicate unit keys");
  for (const p of bp.props) if (!catalog.props[p.def]) errors.push(`prop "${p.key}": unknown def "${p.def}"`);
  for (const u of bp.units) if (!catalog.units[u.def]) errors.push(`unit "${u.key}": unknown def "${u.def}"`);
  const triggers = triggersOf(bp, catalog, errors);
  for (const [id, t] of triggers) if (!bp.rects[t.rect]) errors.push(`trigger "${id}": zone has no rect "${t.rect}"`);

  // Lists the blueprint wrote are rows like any other: the catalog checked its own at boot,
  // these are checked here. Names they lean on must exist in this blueprint.
  const checkNames = (where: string, list: ActionList | undefined): void => {
    for (const a of list ?? []) {
      if ((a.do === "lock" || a.do === "unlock" || a.do === "show" || a.do === "hide" || a.do === "switch") && !propKeys.has(a.prop)) {
        errors.push(`${where}: no prop "${a.prop}" in this zone`);
      } else if (a.do === "spawn" && !bp.marks[a.at]) errors.push(`${where}: no mark "${a.at}" in this zone`);
      else if ((a.do === "fill" || a.do === "strike") && !bp.rects[a.rect]) errors.push(`${where}: no rect "${a.rect}" in this zone`);
    }
  };
  for (const [id, t] of Object.entries(bp.triggers ?? {})) {
    errors.push(...actionRowErrors(catalog, `trigger "${id}"`, t.actions, t.when), ...actionRowErrors(catalog, `trigger "${id}" reset`, t.reset));
    checkNames(`trigger "${id}"`, t.actions);
    checkNames(`trigger "${id}" reset`, t.reset);
  }
  for (const p of bp.props) {
    if (!p.use && !p.release) continue;
    errors.push(...actionRowErrors(catalog, `prop "${p.key}"`, p.use), ...actionRowErrors(catalog, `prop "${p.key}" release`, p.release));
    checkNames(`prop "${p.key}"`, p.use);
    checkNames(`prop "${p.key}" release`, p.release);
  }
  for (const p of bp.props) {
    if (p.talk && !catalog.dialogue[p.talk]) errors.push(`prop "${p.key}": unknown dialogue "${p.talk}"`);
    if (p.to && p.to.zone === bp.zone && !bp.marks[p.to.mark]) errors.push(`prop "${p.key}": leads to mark "${p.to.mark}", which this zone does not have`);
  }
  if (errors.length > 0) return { ok: false, errors, reachedCells: 0 };

  const { w, h } = bp;
  // Locals: the flood below asks this of every cell, and the county has seven million. An imported
  // name is a module lookup each time (and a getter under the test runner); a local is free.
  const tileFlags = TILE_FLAGS;
  const solidBit = F_SOLID;
  const tileArray = bp.tiles;
  const tileSolid = (i: number): boolean => (tileFlags[tileArray[i]] & solidBit) !== 0;

  for (const [name, m] of Object.entries(bp.marks)) {
    if (m.cx < 0 || m.cy < 0 || m.cx >= w || m.cy >= h || tileSolid(m.cy * w + m.cx)) errors.push(`mark "${name}" is inside a wall`);
  }
  for (const u of bp.units) {
    if (u.cx < 0 || u.cy < 0 || u.cx >= w || u.cy >= h || tileSolid(u.cy * w + u.cx)) errors.push(`unit "${u.key}" spawns inside a wall`);
  }

  // Props that block: solid, not pushable/carriable, and (for gates) currently locked.
  const open = new Set<string>(); // gate keys that have been opened
  const hidden = new Set<string>(bp.props.filter((p) => p.hidden).map((p) => p.key));
  const shut = new Set<string>(opts.shut ?? []);
  const blocked = new Uint8Array(w * h);
  const restamp = (): void => {
    blocked.fill(0);
    for (const p of bp.props) {
      const def = catalog.props[p.def];
      if (hidden.has(p.key)) continue;
      const isShut = def.gate ? shut.has(p.key) || ((p.locked ?? false) && !open.has(p.key)) : def.solid && !def.push && !def.carry;
      if (!isShut) continue;
      for (let y = p.cy; y < p.cy + def.h; y++) for (let x = p.cx; x < p.cx + def.w; x++) if (x >= 0 && y >= 0 && x < w && y < h) blocked[y * w + x] = 1;
    }
  };

  for (const p of bp.props) {
    const def = catalog.props[p.def];
    if (p.cx < 0 || p.cy < 0 || p.cx + def.w > w || p.cy + def.h > h) errors.push(`prop "${p.key}" is outside the zone`);
  }
  if (errors.length > 0) return { ok: false, errors, reachedCells: 0 };

  const noKeys = new Set<string>(opts.withhold?.keys ?? []);
  const noVerbs = new Set<string>(opts.withhold?.verbs ?? []);
  const noFlags = new Set<string>(opts.withhold?.flags ?? []);
  const noProps = new Set<string>(opts.withhold?.props ?? []);
  const keys = new Map<string, number>();
  const addKey = (tag: string, qty: number): void => {
    if (!noKeys.has(tag)) keys.set(tag, (keys.get(tag) ?? 0) + qty);
  };
  for (const k of givenKeys) addKey(k, 99);
  /** Null means nothing is gated on knowing a spell. */
  const verbs: Set<string> | null = opts.verbs ? new Set(opts.verbs.filter((v) => !noVerbs.has(v))) : null;
  const flags = new Map<string, number>();
  const looted = new Set<string>();
  const fired = new Set<string>();
  const firedAt: Record<string, number> = {};
  const deadUnits = new Set<string>();
  /** Units an `enter` trigger would stand up, waiting for the flood to reach their mark. */
  const waiting: { unit: string; def: string; at: string }[] = [];
  /** Marks a same-zone `to` leads to: more places the flood starts from. */
  const hops: string[] = [];
  const seen = new Uint8Array(w * h);
  const firstSeen = opts.trace ? new Int16Array(w * h).fill(-1) : null;
  const queue = new Int32Array(w * h);
  const starts = Object.values(bp.marks).filter((_, i) => i >= 0);
  const entry = (opts.entry ? bp.marks[opts.entry] : undefined) ?? bp.marks.start ?? bp.marks.entry ?? bp.marks.front ?? starts[0];

  const applyActions = (list: ActionList | undefined | null): void => {
    for (const a of list ?? []) applyAction(a);
  };
  // A flood is the whole zone, and the county is seven million cells: flood again only when
  // something that blocks has changed, not every time a herb is picked or a rat dies.
  let opened = false;
  let pass = 0;
  const applyAction = (a: Action): void => {
    if (a.do === "unlock") {
      open.add(a.prop);
      firedAt[a.prop] ??= pass;
      opened = true;
    } else if (a.do === "hide") {
      hidden.add(a.prop);
      opened = true;
    } else if (a.do === "show") {
      hidden.delete(a.prop);
      opened = true;
    } else if (a.do === "give") addKey(itemTag(a.item), a.qty ?? 1);
    else if (a.do === "flag") {
      if (!noFlags.has(a.flag)) flags.set(a.flag, a.add !== undefined ? (flags.get(a.flag) ?? 0) + a.add : (a.value ?? 1));
    } else if (a.do === "learn") {
      if (verbs && !noVerbs.has(a.spell)) verbs.add(a.spell);
    }
  };
  const itemTag = (item: string): string => catalog.items[item]?.opens ?? `item:${item}`;

  /** Does she know a spell that would switch this prop on? */
  const canAnswer = (answers: string): boolean => {
    if (!verbs) return true;
    for (const id of verbs) {
      const s = catalog.spells[id];
      if (!s) continue;
      if (s.kind === "world" ? s.world === answers : s.kind === "bolt" && s.school === answers) return true;
    }
    return false;
  };
  /** Every `learn` in a dialogue tree. The solver does not read; it assumes she does. */
  const readTree = (tree: string): void => {
    const t = catalog.dialogue[tree];
    if (!t) return;
    for (const node of Object.values(t.nodes)) {
      for (const a of node.actions ?? []) if (a.do === "learn") applyAction(a);
      for (const o of node.options ?? []) for (const a of o.actions ?? []) if (a.do === "learn") applyAction(a);
    }
  };
  const holds = (c: Condition): boolean => {
    // "Not yet" can only have been true earlier, so it never stops anything from happening.
    if (c.not) return true;
    if (c.if === "flag") {
      const v = flags.get(c.flag) ?? 0;
      return c.eq !== undefined ? v === c.eq : c.min !== undefined ? v >= c.min : v !== 0;
    }
    if (c.if === "dead") return deadUnits.has(c.unit);
    if (c.if === "knows") return verbs ? verbs.has(c.spell) : true;
    if (c.if === "hasItem") return (keys.get(itemTag(c.item)) ?? 0) >= (c.qty ?? 1);
    // The clock and the quest log are outside the zone. Assume the story gets there.
    return true;
  };
  const killed = (key: string, defId: string): void => {
    deadUnits.add(key);
    const def = catalog.units[defId];
    for (const l of def.loot) if (l.chance >= 1) addKey(itemTag(l.item), l.qty);
    applyActions(def.onDeath);
  };

  let reachedCells = 0;
  for (pass = 0; pass < 64; pass++) {
    restamp();
    seen.fill(0);
    let head = 0;
    let tail = 0;
    const push = (x: number, y: number): void => {
      if (x < 0 || y < 0 || x >= w || y >= h) return;
      const i = y * w + x;
      if (seen[i] || tileSolid(i) || blocked[i]) return;
      seen[i] = 1;
      queue[tail++] = i;
    };
    push(entry.cx, entry.cy);
    for (const name of hops) push(bp.marks[name].cx, bp.marks[name].cy);
    while (head < tail) {
      const i = queue[head++];
      const x = i % w;
      const y = (i - x) / w;
      push(x + 1, y);
      push(x - 1, y);
      push(x, y + 1);
      push(x, y - 1);
    }
    reachedCells = tail;
    if (firstSeen) for (let n = 0; n < tail; n++) if (firstSeen[queue[n]] < 0) firstSeen[queue[n]] = pass;
    const touches = (p: PropSpawn): boolean => {
      const def = catalog.props[p.def];
      for (let y = p.cy - 1; y <= p.cy + def.h; y++) {
        for (let x = p.cx - 1; x <= p.cx + def.w; x++) {
          if (x >= 0 && y >= 0 && x < w && y < h && seen[y * w + x]) return true;
        }
      }
      return false;
    };

    opened = false;
    let progress = true;
    // Settle: keep collecting and spending keys on what this flood reached until nothing new happens.
    while (progress && !opened) {
    progress = false;
    for (const p of bp.props) {
      if (hidden.has(p.key) || noProps.has(p.key) || !touches(p)) continue;
      const def = catalog.props[p.def];
      const lockedNow = (p.locked ?? false) && !open.has(p.key);
      if (lockedNow) {
        const have = p.keyTag ? (keys.get(p.keyTag) ?? 0) : 0;
        if (have > 0 && !shut.has(p.key)) {
          keys.set(p.keyTag as string, have - 1);
          open.add(p.key);
          firedAt[p.key] ??= pass;
          opened = true;
          progress = true;
        }
        continue;
      }
      if (p.to && p.to.zone === bp.zone && !hops.includes(p.to.mark)) {
        hops.push(p.to.mark);
        firedAt[p.key] ??= pass;
        opened = true;
        progress = true;
      }
      if (p.talk && !fired.has(`talk:${p.key}`)) {
        fired.add(`talk:${p.key}`);
        firedAt[p.key] ??= pass;
        readTree(p.talk);
        progress = true;
      }
      if (p.loot && !looted.has(p.key)) {
        looted.add(p.key);
        firedAt[p.key] ??= pass;
        for (const s of p.loot) addKey(itemTag(s.item), s.qty);
        applyActions(p.use);
        progress = true;
      } else if (p.use && !p.loot && !fired.has(p.key)) {
        if (def.answers) {
          if (!canAnswer(def.answers)) continue;
          const needs = p.needs ?? [];
          if (!needs.every((n) => (keys.get(`item:${n.item}`) ?? 0) >= n.qty)) continue;
          for (const n of needs) keys.set(`item:${n.item}`, (keys.get(`item:${n.item}`) ?? 0) - n.qty);
        }
        fired.add(p.key);
        firedAt[p.key] ??= pass;
        applyActions(p.use);
        progress = true;
      }
    }
    for (const u of bp.units) {
      if (deadUnits.has(u.key) || !seen[u.cy * w + u.cx]) continue;
      const def = catalog.units[u.def];
      if (def.faction === "friendly") continue;
      killed(u.key, u.def);
      progress = true;
    }
    for (let n = waiting.length - 1; n >= 0; n--) {
      const s = waiting[n];
      const m = bp.marks[s.at];
      if (!m || !seen[m.cy * w + m.cx]) continue;
      waiting.splice(n, 1);
      killed(s.unit, s.def);
      progress = true;
    }
    for (const [id, t] of triggers) {
      if (fired.has(`trigger:${id}`)) continue;
      const r = bp.rects[t.rect];
      let inside = false;
      for (let y = r.cy; y < r.cy + r.h && !inside; y++) for (let x = r.cx; x < r.cx + r.w && !inside; x++) inside = seen[y * w + x] === 1;
      if (!inside) continue;
      // Only the "everything is dead" half of a lock-in matters to reachability: the solver
      // assumes fights are won, so `while` triggers fire once their conditions can hold, and
      // an `enter` trigger's locks are skipped. What it would stand up is still there to be killed.
      if ((t.mode ?? "enter") === "while") {
        if (!(t.when ?? []).every(holds)) continue;
        fired.add(`trigger:${id}`);
        applyActions(t.actions);
        progress = true;
      } else {
        fired.add(`trigger:${id}`);
        for (const a of t.actions) {
          if (a.do === "spawn" && catalog.units[a.def] && catalog.units[a.def].faction !== "friendly") waiting.push({ unit: a.unit, def: a.def, at: a.at });
        }
        if (waiting.length > 0) progress = true;
      }
    }
    }
    if (!opened) break;
  }

  for (const [name, m] of Object.entries(bp.marks)) if (!seen[m.cy * w + m.cx]) errors.push(`mark "${name}" is unreachable`);
  for (const k of contract.units) {
    const u = bp.units.find((x) => x.key === k);
    if (u && !seen[u.cy * w + u.cx]) errors.push(`unit "${k}" is unreachable`);
  }
  for (const k of contract.props) {
    const p = propKeys.get(k);
    if (!p || hidden.has(k)) continue;
    const def = catalog.props[p.def];
    let near = false;
    for (let y = p.cy - 1; y <= p.cy + def.h && !near; y++) {
      for (let x = p.cx - 1; x <= p.cx + def.w && !near; x++) near = x >= 0 && y >= 0 && x < w && y < h && seen[y * w + x] === 1;
    }
    if (!near) errors.push(`prop "${k}" is unreachable`);
    if ((p.locked ?? false) && !open.has(k) && def.gate) errors.push(`gate "${k}" can never be opened`);
  }
  const trace: SolveTrace | undefined = firstSeen
    ? { passes: pass + 1, firstSeen, firedAt, verbs: verbs ? [...verbs] : [], flags: Object.fromEntries(flags), dead: [...deadUnits] }
    : undefined;
  return { ok: errors.length === 0, errors, reachedCells, trace };
}
