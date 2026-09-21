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
//   if        An `if` row runs its `then` once its conditions can hold and its `else` if they
//             did not hold when the list was first run. A `then` that could not run yet is
//             kept and tried again, unless the list belongs to something that only happens once.
//   hops      A `to` that names this zone is a one-way edge to its mark.
//   states    THE STATEFUL FLOOD. A building with a breaker is two buildings. With `states`
//             given, the flood's nodes are (cell, state): one layer of "seen" for every
//             combination of the state flags (at most three flags, so at most eight layers).
//             Walking keeps the state. A CONTROL (a prop whose `use` list sets a state flag)
//             that is reached in one layer is an edge to the layer its list leads to, and
//             the new layer's flood starts AT THE CONTROL, not at the entrance: you are where
//             you stood when you pulled it. What a control's list locks, unlocks, shows and
//             hides is how the layers differ; keys, loot and everything else stay monotone
//             and shared, as they always were. A flag that is unset means the state's
//             `initial` value: nothing has to set it when the zone is made.
//   ablation  `withhold` takes one thing away (a key tag, a spell, a flag, a prop's list) so
//             a caller can prove a lock really holds; `shut` keeps a gate closed.

import { actionRowErrors, eachAction, type Catalog, type TriggerDef } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import type { Action, ActionList, Condition } from "@/sim/state";
import type { Blueprint, PropSpawn, ZoneContract } from "@/world/blueprint";

/** A two-valued mechanism of the whole zone, held in a world flag: 0 or unset is how the zone starts, 1 is the other way. */
export type SolveState = { id: string; flag: string };

export type SolveOptions = {
  /** Spells known on arrival. Leave it out and nothing is gated on knowing a spell. */
  verbs?: readonly string[];
  /** Ablation: things the player is never given, to prove that the lock they open holds without them. */
  withhold?: { keys?: readonly string[]; verbs?: readonly string[]; flags?: readonly string[]; props?: readonly string[] };
  /** Props that stay shut whatever is unlocked: "could she get here with this gate down?" */
  shut?: readonly string[];
  /** Flood from this mark instead of the zone's entrance. */
  entry?: string;
  /**
   * This blueprint is a PIECE of a zone (one room, stamped alone by the template harness), so a
   * list naming something in another room is expected, not a broken row.
   */
  fragment?: boolean;
  /** Keep the order things happened in. Costs two bytes a cell, so the county does not ask for it. */
  trace?: boolean;
  /** Reversible mechanisms (a breaker, a valve). At most three. Given, the flood is stateful. */
  states?: readonly SolveState[];
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
  /** Combinations of the state flags she could bring about, as bit masks (bit n = `states[n]` is not at its initial value). [0] without states. */
  layers: number[];
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

/** What a control's list does to a prop in the state it leads to. */
type Governed = { locked?: boolean; hidden?: boolean };

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
  //
  // Unless this IS only a piece of one (`fragment`). The template harness stamps one room by
  // itself to prove it, and a control's list names things in other rooms by definition: that
  // is what a building-wide state is. In a fragment those names are simply absent, and an
  // action that cannot find its prop does nothing, so the room is still honestly judged.
  const checkNames = (where: string, list: ActionList | undefined): void => {
    if (opts.fragment) return;
    eachAction(list, (a) => {
      if ((a.do === "lock" || a.do === "unlock" || a.do === "show" || a.do === "hide" || a.do === "switch") && !propKeys.has(a.prop)) {
        errors.push(`${where}: no prop "${a.prop}" in this zone`);
      } else if (a.do === "spawn" && !bp.marks[a.at]) errors.push(`${where}: no mark "${a.at}" in this zone`);
      else if ((a.do === "fill" || a.do === "strike") && !bp.rects[a.rect]) errors.push(`${where}: no rect "${a.rect}" in this zone`);
      else if (a.do === "send" && !bp.marks[a.to]) errors.push(`${where}: no mark "${a.to}" in this zone`);
      else if (a.do === "reveal") for (const r of a.rects) if (!bp.rects[r]) errors.push(`${where}: no rect "${r}" in this zone`);
    });
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
  const states = opts.states ?? [];
  if (states.length > 3) errors.push(`a zone may have three states at most, and this one has ${states.length}`);
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
  /** Units an `enter` trigger (or a breaker) would stand up, waiting for the flood to reach their mark. */
  const waiting: { unit: string; def: string; at: string }[] = [];
  const itemTag = (item: string): string => catalog.items[item]?.opens ?? `item:${item}`;

  // --- what blocks, in which state -----------------------------------------------------------
  const open = new Set<string>(); // opened for good: a key, a lever, a kill
  const hidden = new Set<string>(bp.props.filter((p) => p.hidden).map((p) => p.key));
  /** Hidden for good by something that is not a control (blown up, picked up): no breaker brings it back. */
  const removed = new Set<string>();
  const shut = new Set<string>(opts.shut ?? []);

  const stateBit = new Map<string, number>();
  states.forEach((s, n) => stateBit.set(s.flag, n));
  const layerCount = 1 << states.length;
  /** Does this condition hold in this state? Null when it is not about a state at all. */
  const stateHolds = (c: Condition, s: number): boolean | null => {
    if (c.if !== "flag") return null;
    const bit = stateBit.get(c.flag);
    if (bit === undefined) return null;
    const v = (s >> bit) & 1;
    const r = c.eq !== undefined ? v === c.eq : c.min !== undefined ? v >= c.min : v !== 0;
    return c.not ? !r : r;
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
  const holdsIn = (c: Condition, s: number): boolean => stateHolds(c, s) ?? holds(c);

  // Controls, and what each state looks like. Worked out on the flags alone, before anyone
  // walks anywhere: from how the zone starts, pull every control in every state it leads to.
  const controls: { p: PropSpawn; list: ActionList }[] = [];
  const governed: (Map<string, Governed> | undefined)[] = [];
  governed[0] = new Map();
  /** Run a control's list on the flags alone: the state it leads to, and (given a map) what it does to props on the way. */
  const pull = (list: ActionList, from: number, into: Map<string, Governed> | null): number => {
    let to = from;
    const walk = (l: ActionList): void => {
      for (const a of l) {
        if (a.do === "if") {
          // Conditions about a state are asked of the state as the list began; the rest are assumed.
          if (a.when.every((c) => stateHolds(c, from) ?? true)) walk(a.then);
          else if (a.else) walk(a.else);
        } else if (a.do === "flag") {
          const bit = stateBit.get(a.flag);
          if (bit === undefined) continue;
          const v = a.add !== undefined ? (((to >> bit) & 1) + a.add !== 0 ? 1 : 0) : (a.value ?? 1) !== 0 ? 1 : 0;
          to = v ? to | (1 << bit) : to & ~(1 << bit);
        } else if (!into) continue;
        else if (a.do === "lock" || a.do === "unlock") into.set(a.prop, { ...into.get(a.prop), locked: a.do === "lock" });
        else if (a.do === "show" || a.do === "hide") into.set(a.prop, { ...into.get(a.prop), hidden: a.do === "hide" });
      }
    };
    walk(list);
    return to;
  };
  if (states.length > 0) {
    for (const p of bp.props) {
      if (!p.use || p.loot || catalog.props[p.def].plate) continue;
      let sets = false;
      eachAction(p.use, (a) => {
        if (a.do === "flag" && stateBit.has(a.flag)) sets = true;
      });
      if (sets) controls.push({ p, list: p.use });
    }
    /** How a prop stands in a state: what the lists left it as, over how the blueprint made it. */
    const standing = (map: Map<string, Governed> | undefined, key: string): string => {
      const made = propKeys.get(key);
      const g = map?.get(key);
      return `${g?.locked ?? made?.locked ?? false}/${g?.hidden ?? made?.hidden ?? false}`;
    };
    const todo = [0];
    while (todo.length > 0) {
      const s = todo.shift() as number;
      for (const c of controls) {
        const next = new Map(governed[s]);
        const to = pull(c.list, s, next);
        if (to === s) continue;
        if (governed[to] === undefined) {
          governed[to] = next;
          todo.push(to);
          continue;
        }
        // A state must look the same however it was reached, or "lit" means nothing. That
        // includes the state the zone starts in: two pulls must leave the building as it was made.
        for (const key of new Set([...next.keys(), ...(governed[to]?.keys() ?? [])])) {
          if (standing(next, key) !== standing(to === 0 ? undefined : governed[to], key)) {
            errors.push(`states: "${key}" is left differently depending on how the state was reached (pulling "${c.p.key}")`);
          }
        }
      }
    }
  }
  if (errors.length > 0) return { ok: false, errors: [...new Set(errors)], reachedCells: 0 };

  const hiddenIn = (p: PropSpawn, s: number): boolean => {
    if (removed.has(p.key)) return true;
    const g = governed[s]?.get(p.key);
    return g?.hidden !== undefined ? g.hidden : hidden.has(p.key);
  };
  const lockedIn = (p: PropSpawn, s: number): boolean => {
    if (open.has(p.key)) return false;
    const g = governed[s]?.get(p.key);
    return g?.locked !== undefined ? g.locked : (p.locked ?? false);
  };
  /** Does this prop ever stand in the way of feet? Only then is its opening worth a new flood. */
  const blocksFeet = (p: PropSpawn | undefined): boolean => {
    if (!p) return false;
    const def = catalog.props[p.def];
    return def.gate ? true : def.solid && !def.push && !def.carry;
  };

  const seenL: Uint8Array[] = [];
  const blockedL: Uint8Array[] = [];
  for (let s = 0; s < layerCount; s++) {
    seenL.push(new Uint8Array(w * h));
    blockedL.push(new Uint8Array(w * h));
  }
  /** Reached in any state. Without states it IS the one layer, so the county pays nothing for the idea. */
  const seenAny = layerCount === 1 ? seenL[0] : new Uint8Array(w * h);
  const reachedLayer = new Uint8Array(layerCount);
  const restamp = (s: number): void => {
    const blocked = blockedL[s];
    blocked.fill(0);
    for (const p of bp.props) {
      const def = catalog.props[p.def];
      if (hiddenIn(p, s)) continue;
      const isShut = def.gate ? shut.has(p.key) || lockedIn(p, s) : def.solid && !def.push && !def.carry;
      if (!isShut) continue;
      for (let y = p.cy; y < p.cy + def.h; y++) for (let x = p.cx; x < p.cx + def.w; x++) if (x >= 0 && y >= 0 && x < w && y < h) blocked[y * w + x] = 1;
    }
  };

  /** Marks a same-zone `to` leads to, and the state she was in when she took it: more places the flood starts from. */
  const hops: { mark: string; s: number }[] = [];
  const firstSeen = opts.trace ? new Int16Array(w * h).fill(-1) : null;
  const queue = new Int32Array(w * h);
  const starts = Object.values(bp.marks).filter((_, i) => i >= 0);
  const entry = (opts.entry ? bp.marks[opts.entry] : undefined) ?? bp.marks.start ?? bp.marks.entry ?? bp.marks.front ?? starts[0];

  // A flood is the whole zone, and the county is seven million cells: flood again only when
  // something that blocks FEET has changed. A locked chest opening, a herb picked, a rat dying
  // and a lever that only sets a flag change what she holds, not where she can walk: those go
  // round the settle loop below, on the flood there already is.
  let opened = false;
  let pass = 0;
  /** `then` lists whose `if` could not hold yet, kept to be tried again. */
  const deferred: { when: Condition[]; then: ActionList; s: number; control: boolean }[] = [];

  const applyAction = (a: Action, control: boolean): void => {
    if (a.do === "unlock" || a.do === "lock" || a.do === "show" || a.do === "hide") {
      // What a control does to props is what its state IS (worked out above), not something that happens once.
      if (control) return;
      if (a.do === "unlock") {
        open.add(a.prop);
        firedAt[a.prop] ??= pass;
        if (blocksFeet(propKeys.get(a.prop))) opened = true;
      } else if (a.do === "hide") {
        hidden.add(a.prop);
        removed.add(a.prop);
        if (blocksFeet(propKeys.get(a.prop))) opened = true;
      } else if (a.do === "show") {
        hidden.delete(a.prop);
        removed.delete(a.prop);
        if (blocksFeet(propKeys.get(a.prop))) opened = true;
      }
    } else if (a.do === "give") addKey(itemTag(a.item), a.qty ?? 1);
    else if (a.do === "flag") {
      if (!noFlags.has(a.flag) && !stateBit.has(a.flag)) flags.set(a.flag, a.add !== undefined ? (flags.get(a.flag) ?? 0) + a.add : (a.value ?? 1));
    } else if (a.do === "learn") {
      if (verbs && !noVerbs.has(a.spell)) verbs.add(a.spell);
    } else if (a.do === "spawn") {
      if (catalog.units[a.def] && catalog.units[a.def].faction !== "friendly" && !deadUnits.has(a.unit) && !waiting.some((x) => x.unit === a.unit)) waiting.push({ unit: a.unit, def: a.def, at: a.at });
    }
  };
  /**
   * Run a list as it would run in state `s`. `again`: whatever owns the list can run it again
   * later (a lever, a `while` row), so a `then` that cannot hold yet is worth keeping.
   */
  const applyActions = (list: ActionList | undefined | null, s = 0, again = false, control = false): void => {
    for (const a of list ?? []) {
      if (a.do === "if") {
        if (a.when.every((c) => holdsIn(c, s))) applyActions(a.then, s, again, control);
        else {
          applyActions(a.else, s, again, control);
          // Only what is not about a state can come true later in the same state.
          if (again && a.when.every((c) => stateHolds(c, s) ?? true)) deferred.push({ when: a.when, then: a.then, s, control });
        }
      } else if (a.do === "send") {
        // The solver assumes fights are won; it assumes walks are finished too.
        applyActions(a.then, s, again, control);
      } else applyAction(a, control);
    }
  };

  /** Does she know a spell that would switch this prop on? */
  const canAnswer = (answers: string): boolean => {
    if (!verbs) return true;
    for (const id of verbs) {
      const sp = catalog.spells[id];
      if (!sp) continue;
      if (sp.kind === "world" ? sp.world === answers : sp.kind === "bolt" && sp.school === answers) return true;
    }
    return false;
  };
  const hasNeeds = (p: PropSpawn): boolean => (p.needs ?? []).every((n) => (keys.get(`item:${n.item}`) ?? 0) >= n.qty);
  /** Every `learn` in a dialogue tree. The solver does not read; it assumes she does. */
  const readTree = (tree: string): void => {
    const t = catalog.dialogue[tree];
    if (!t) return;
    const learn = (a: Action): void => {
      if (a.do === "learn") applyAction(a, false);
    };
    for (const node of Object.values(t.nodes)) {
      eachAction(node.actions, learn);
      for (const o of node.options ?? []) eachAction(o.actions, learn);
    }
  };
  const killed = (key: string, defId: string): void => {
    deadUnits.add(key);
    const def = catalog.units[defId];
    for (const l of def.loot) if (l.chance >= 1) addKey(itemTag(l.item), l.qty);
    applyActions(def.onDeath);
    for (const ph of def.phases ?? []) applyActions(ph.onEnter);
  };

  const touchesIn = (p: PropSpawn, s: number): boolean => {
    const seen = seenL[s];
    const def = catalog.props[p.def];
    for (let y = p.cy - 1; y <= p.cy + def.h; y++) {
      for (let x = p.cx - 1; x <= p.cx + def.w; x++) {
        if (x >= 0 && y >= 0 && x < w && y < h && seen[y * w + x]) return true;
      }
    }
    return false;
  };
  /** Can this control be worked in this state, by someone standing in it? */
  const canWork = (p: PropSpawn, s: number): boolean => {
    if (noProps.has(p.key) || hiddenIn(p, s) || lockedIn(p, s)) return false;
    const def = catalog.props[p.def];
    if (def.answers && (!canAnswer(def.answers) || !hasNeeds(p))) return false;
    return true;
  };

  let reachedCells = 0;
  for (pass = 0; pass < 64; pass++) {
    // --- flood: every state she can bring about, each from where she stood when she did --------
    reachedLayer.fill(0);
    let reachedNow = 0;
    const floodLayer = (s: number, seeds: number[]): void => {
      const seen = seenL[s];
      const blocked = blockedL[s];
      let head = 0;
      let tail = 0;
      const push = (x: number, y: number): void => {
        if (x < 0 || y < 0 || x >= w || y >= h) return;
        const i = y * w + x;
        if (seen[i] || tileSolid(i) || blocked[i]) return;
        seen[i] = 1;
        queue[tail++] = i;
      };
      for (let n = 0; n + 1 < seeds.length; n += 2) push(seeds[n], seeds[n + 1]);
      while (head < tail) {
        const i = queue[head++];
        const x = i % w;
        const y = (i - x) / w;
        push(x + 1, y);
        push(x - 1, y);
        push(x, y + 1);
        push(x, y - 1);
      }
      if (tail > 0) reachedLayer[s] = 1;
      if (seenAny !== seen) for (let n = 0; n < tail; n++) seenAny[queue[n]] = 1;
      if (firstSeen) for (let n = 0; n < tail; n++) if (firstSeen[queue[n]] < 0) firstSeen[queue[n]] = pass;
      if (layerCount === 1) reachedNow = tail;
    };
    for (let s = 0; s < layerCount; s++) {
      if (governed[s] === undefined) continue;
      restamp(s);
      seenL[s].fill(0);
    }
    if (seenAny !== seenL[0]) seenAny.fill(0);
    const seedsOf: number[][] = [];
    for (let s = 0; s < layerCount; s++) seedsOf.push([]);
    seedsOf[0].push(entry.cx, entry.cy);
    for (const hop of hops) seedsOf[hop.s].push(bp.marks[hop.mark].cx, bp.marks[hop.mark].cy);
    const dirty: number[] = [];
    for (let s = 0; s < layerCount; s++) if (seedsOf[s].length > 0) dirty.push(s);
    while (dirty.length > 0) {
      const s = dirty.shift() as number;
      const seeds = seedsOf[s];
      seedsOf[s] = [];
      floodLayer(s, seeds);
      for (const c of controls) {
        if (!canWork(c.p, s) || !touchesIn(c.p, s)) continue;
        // Where the list leads, asked of the flags alone (the same walk that made `governed`).
        const to = pull(c.list, s, null);
        if (to === s || governed[to] === undefined) continue;
        // She is where she stood when she pulled it: the cells round the control that she had
        // reached, and that are still floor in the state she has just made.
        const def = catalog.props[c.p.def];
        const from = seenL[s];
        const into = seenL[to];
        const wall = blockedL[to];
        let any = false;
        for (let y = c.p.cy - 1; y <= c.p.cy + def.h; y++) {
          for (let x = c.p.cx - 1; x <= c.p.cx + def.w; x++) {
            if (x < 0 || y < 0 || x >= w || y >= h) continue;
            const i = y * w + x;
            if (!from[i] || into[i] || wall[i]) continue;
            seedsOf[to].push(x, y);
            any = true;
          }
        }
        if (any && !dirty.includes(to)) dirty.push(to);
      }
    }
    if (layerCount > 1) {
      reachedNow = 0;
      for (let i = 0; i < seenAny.length; i++) reachedNow += seenAny[i];
    }
    reachedCells = reachedNow;

    // --- settle: keep collecting and spending on what this flood reached until nothing new happens
    opened = false;
    let progress = true;
    while (progress && !opened) {
      progress = false;
      for (const p of bp.props) {
        if (noProps.has(p.key)) continue;
        // The first state she can stand beside it in, seeing it. Most zones have the one.
        let at = -1;
        for (let s = 0; s < layerCount && at < 0; s++) if (reachedLayer[s] && !hiddenIn(p, s) && touchesIn(p, s)) at = s;
        if (at < 0) continue;
        const def = catalog.props[p.def];
        if (lockedIn(p, at)) {
          const have = p.keyTag ? (keys.get(p.keyTag) ?? 0) : 0;
          if (have > 0 && !shut.has(p.key)) {
            keys.set(p.keyTag as string, have - 1);
            open.add(p.key);
            firedAt[p.key] ??= pass;
            // Only a gate's opening changes where she can walk. A chest's is for the next turn of this loop.
            if (blocksFeet(p) && def.gate) opened = true;
            progress = true;
          }
          // A lock is no answer to a spell. `schoolTouch` in the sim does not look at it, so a locked
          // case really can be blown open; the proof must know that, or a locked thing that answers a
          // verb would be a lock nothing in the game can open and every candidate would be refused.
          if (!def.answers) continue;
        }
        if (p.to && p.to.zone === bp.zone && !hops.some((x) => x.mark === p.to?.mark && x.s === at)) {
          hops.push({ mark: p.to.mark, s: at });
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
          applyActions(p.use, at);
          progress = true;
        } else if (p.use && !p.loot) {
          const control = controls.some((c) => c.p === p);
          // A control is worked in every state she reaches it in; anything else happens once.
          for (let s = at; s < layerCount; s++) {
            if (s !== at && (!control || !reachedLayer[s] || hiddenIn(p, s) || lockedIn(p, s) || !touchesIn(p, s))) continue;
            const id = control ? `${p.key}@${s}` : p.key;
            if (fired.has(id)) continue;
            if (def.answers) {
              if (!canAnswer(def.answers) || !hasNeeds(p)) continue;
              // It is mended once, in whichever state she first mends it.
              if (!fired.has(`paid:${p.key}`)) {
                fired.add(`paid:${p.key}`);
                for (const n of p.needs ?? []) keys.set(`item:${n.item}`, (keys.get(`item:${n.item}`) ?? 0) - n.qty);
              }
            }
            fired.add(id);
            firedAt[p.key] ??= pass;
            applyActions(p.use, s, !def.once && !def.answers, control);
            progress = true;
          }
        }
      }
      for (const u of bp.units) {
        if (deadUnits.has(u.key) || !seenAny[u.cy * w + u.cx]) continue;
        const def = catalog.units[u.def];
        if (def.faction === "friendly") continue;
        killed(u.key, u.def);
        progress = true;
      }
      for (let n = waiting.length - 1; n >= 0; n--) {
        const s = waiting[n];
        const m = bp.marks[s.at];
        if (!m || !seenAny[m.cy * w + m.cx]) continue;
        waiting.splice(n, 1);
        killed(s.unit, s.def);
        progress = true;
      }
      for (const [id, t] of triggers) {
        if (fired.has(`trigger:${id}`)) continue;
        const r = bp.rects[t.rect];
        let inside = false;
        for (let y = r.cy; y < r.cy + r.h && !inside; y++) for (let x = r.cx; x < r.cx + r.w && !inside; x++) inside = seenAny[y * w + x] === 1;
        if (!inside) continue;
        // Only the "everything is dead" half of a lock-in matters to reachability: the solver
        // assumes fights are won, so `while` triggers fire once their conditions can hold, and
        // an `enter` trigger's locks are skipped. What it would stand up is still there to be killed.
        if ((t.mode ?? "enter") === "while") {
          if (!(t.when ?? []).every(holds)) continue;
          fired.add(`trigger:${id}`);
          applyActions(t.actions, 0, !t.once);
          progress = true;
        } else {
          fired.add(`trigger:${id}`);
          const before = waiting.length;
          eachAction(t.actions, (a) => {
            if (a.do === "spawn") applyAction(a, false);
          });
          if (waiting.length > before) progress = true;
        }
      }
      for (let n = deferred.length - 1; n >= 0; n--) {
        const d = deferred[n];
        if (!d.when.every((c) => holdsIn(c, d.s))) continue;
        deferred.splice(n, 1);
        applyActions(d.then, d.s, true, d.control);
        progress = true;
      }
      // With states, anything new may be what a control was waiting for (a spell, a part): look again.
      if (layerCount > 1 && progress) opened = true;
    }
    if (!opened) break;
  }

  for (const [name, m] of Object.entries(bp.marks)) if (!seenAny[m.cy * w + m.cx]) errors.push(`mark "${name}" is unreachable`);
  for (const k of contract.units) {
    const u = bp.units.find((x) => x.key === k);
    if (u && !seenAny[u.cy * w + u.cx]) errors.push(`unit "${k}" is unreachable`);
  }
  for (const k of contract.props) {
    const p = propKeys.get(k);
    if (!p) continue;
    let shown = false;
    let opens = false;
    for (let s = 0; s < layerCount; s++) {
      if (!reachedLayer[s] && s !== 0) continue;
      if (!hiddenIn(p, s)) shown = true;
      if (!lockedIn(p, s)) opens = true;
    }
    if (!shown) continue;
    const def = catalog.props[p.def];
    let near = false;
    for (let y = p.cy - 1; y <= p.cy + def.h && !near; y++) {
      for (let x = p.cx - 1; x <= p.cx + def.w && !near; x++) near = x >= 0 && y >= 0 && x < w && y < h && seenAny[y * w + x] === 1;
    }
    if (!near) errors.push(`prop "${k}" is unreachable`);
    if (!opens && def.gate) errors.push(`gate "${k}" can never be opened`);
  }
  const layers: number[] = [];
  for (let s = 0; s < layerCount; s++) if (reachedLayer[s]) layers.push(s);
  const trace: SolveTrace | undefined = firstSeen
    ? { passes: pass + 1, firstSeen, firedAt, verbs: verbs ? [...verbs] : [], flags: Object.fromEntries(flags), dead: [...deadUnits], layers }
    : undefined;
  return { ok: errors.length === 0, errors, reachedCells, trace };
}
