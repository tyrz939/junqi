// Proof (DUNGEONS.md 2.6). The lock-and-key solver says a blueprint CAN be finished; these
// checks say it is the dungeon that was designed: in the authored order, with locks that
// really hold, a rest room where it should be, a shortcut, a tease, and a way in to every
// room that seals. A blueprint that fails any of them is re-rolled, and the name of the
// check that failed is kept (`C3: ...`) so a bad pool or a bad mission can be seen.
//
//   C1  every critical node reached, in order, and every lock holds without its grant
//   C2  no key behind its own lock                     (falls out of C1's forward solve)
//   C3  no order of spending plain keys strands her    (on the room graph)
//   C4  materials cannot be starved
//   C5  the verb is taught before it is demanded, and first used in safety
//   C6  every lock-in can be followed into
//   C7  a rest room off the hub, with no enemies, at the right depth
//   C8  the first completion is the right length
//   C9  a loop, and a shortcut that comes out near the rest room
//   C10 the verb's first lock and the boss gate are seen before they can be opened
//   C11 plates can be held
//   C12 nothing solid appears on a person
//
// A building with states (a breaker, a valve) is solved with the stateful flood (validate.ts):
// every solve here passes the mission's `states`, and a `state` edge's lock is proven like any
// other, by taking its controls away and finding the far room unreached.

import { eachAction, type Catalog } from "@/sim/catalog";
import { F_BLOCK_LOS, F_NOPUSH, F_SOLID, TILE_FLAGS } from "@/sim/grid";
import type { ActionList } from "@/sim/state";
import type { Blueprint, PropSpawn, Rect, ZoneContract } from "@/world/blueprint";
import { infoOf, type BuildInfo, type RoomInfo } from "@/world/dungeon/generate";
import { pushPath } from "@/world/dungeon/room";
import type { DungeonDef, EdgeKind, MissionEdge, MissionNode } from "@/world/dungeon/types";
import { validateBlueprint, type SolveOptions, type SolveTrace } from "@/world/validate";

/** Every name the mission binds for a node that is always there. The story may lean on these and no others. */
export function contractOf(def: DungeonDef): ZoneContract {
  const c: ZoneContract = { units: [], props: [], marks: [], rects: [] };
  const critical = (id: string): boolean => def.nodes.find((n) => n.id === id)?.critical ?? false;
  for (const n of def.nodes) {
    if (!n.critical) continue;
    for (const b of n.binds) c[`${b.what}s` as keyof ZoneContract].push(b.as);
  }
  for (const e of def.edges) {
    if (!critical(e.from) || !critical(e.to)) continue;
    for (const kind of [e.kind, ...(e.also ?? [])]) {
      if ((kind.t === "key" || kind.t === "lockin" || kind.t === "oneway" || kind.t === "state") && kind.gateAs && !c.props.includes(kind.gateAs)) c.props.push(kind.gateAs);
      if (kind.t === "verb" && kind.propAs) c.props.push(kind.propAs);
    }
  }
  return c;
}

// --- what a node gives, read off its holdings ----------------------------------------------

type Gains = { keys: Record<string, number>; items: Record<string, number>; verbs: string[]; flags: string[]; states: string[] };

/** What the solver is told about a mission before it floods: the spells at the door, and the states. */
export function solveOptionsOf(def: DungeonDef): SolveOptions {
  return { verbs: def.givenVerbs, states: (def.states ?? []).map((s) => ({ id: s.id, flag: s.flag })) };
}

function gainsOf(node: MissionNode, catalog: Catalog): Gains {
  const g: Gains = { keys: {}, items: {}, verbs: [], flags: [], states: [] };
  const item = (id: string, qty: number): void => {
    const tag = catalog.items[id]?.opens;
    if (tag) g.keys[tag] = (g.keys[tag] ?? 0) + qty;
    else g.items[id] = (g.items[id] ?? 0) + qty;
  };
  const actions = (list: ActionList | undefined): void => {
    eachAction(list, (a) => {
      if (a.do === "give") item(a.item, a.qty ?? 1);
      else if (a.do === "learn") g.verbs.push(a.spell);
      else if (a.do === "flag") g.flags.push(a.flag);
    });
  };
  for (const h of node.holds) {
    if ("unit" in h) {
      const u = catalog.units[h.unit];
      for (const l of u?.loot ?? []) if (l.chance >= 1) item(l.item, l.qty);
      actions(u?.onDeath);
      continue;
    }
    if ("loot" in h) for (const s of h.loot ?? []) item(s.item, s.qty);
    if ("use" in h) actions(h.use);
    if ("controls" in h && h.controls) {
      g.states.push(h.controls);
      for (const list of Object.values(h.becomes ?? {})) actions(list);
    }
    if ("talk" in h && h.talk) {
      const tree = catalog.dialogue[h.talk];
      for (const n of Object.values(tree?.nodes ?? {})) {
        actions(n.actions);
        for (const o of n.options ?? []) actions(o.actions);
      }
    }
  }
  return g;
}

/** Mistakes in the mission itself, before any layout: a grant nothing gives, a sight line with no corridor. */
export function lintDef(def: DungeonDef, catalog: Catalog): string[] {
  const errors: string[] = [];
  const ids = new Set<string>();
  for (const n of def.nodes) {
    if (ids.has(n.id)) errors.push(`node "${n.id}" is defined twice`);
    ids.add(n.id);
    // A thing that answers a verb and holds loot must be locked with no key. Opening a thing for its
    // loot runs its `use` (that is what `useProp` does), so otherwise the verb's own payload comes out
    // without the verb and the lock only reads as one. Locked with nothing that fits, the spell is the
    // only way in, which is the sound shape: the museum's cracked case is written that way.
    for (const h of n.holds) {
      if (!("loot" in h) || !("prop" in h) || h.prop === undefined) continue;
      const answers = catalog.props[h.prop]?.answers;
      if (answers && !(h.locked && !("keyTag" in h))) {
        errors.push(`node "${n.id}": ${h.socket} holds "${h.prop}", which answers ${answers}, and loot she could reach without it. Lock it with no key, or hide a chest behind it`);
      }
    }
    const g = gainsOf(n, catalog);
    for (const want of n.grants) {
      const ok =
        "key" in want
          ? (g.keys[want.key] ?? 0) > 0
          : "verb" in want
            ? g.verbs.includes(want.verb)
            : "flag" in want
              ? g.flags.includes(want.flag)
              : "state" in want
                ? g.states.includes(want.state)
                : (g.items[want.item] ?? 0) >= want.qty;
      if (!ok) errors.push(`node "${n.id}" says it grants ${JSON.stringify(want)} and nothing it holds gives that`);
    }
  }
  for (const e of def.edges) {
    if (!ids.has(e.from) || !ids.has(e.to)) errors.push(`edge ${e.from} -> ${e.to} names a node that does not exist`);
    if (e.kind.t === "sight" && !def.edges.some((f) => f !== e && f.kind.t !== "sight" && ((f.from === e.from && f.to === e.to) || (f.from === e.to && f.to === e.from)))) {
      errors.push(`edge ${e.from} -> ${e.to} is a sight line with no corridor to look along`);
    }
  }
  const stateIds = (def.states ?? []).map((s) => s.id);
  if (stateIds.length > 3) errors.push("a dungeon may have three states at most");
  for (const s of def.states ?? []) {
    if (!s.values.includes(s.initial)) errors.push(`state "${s.id}" starts as "${s.initial}", which is not one of its values`);
    if (!def.nodes.some((n) => n.holds.some((h) => "controls" in h && h.controls === s.id))) errors.push(`state "${s.id}" has no control: nothing in the mission can change it`);
  }
  for (const e of def.edges) {
    for (const kind of [e.kind, ...(e.also ?? [])]) {
      if (kind.t !== "state") continue;
      const s = (def.states ?? []).find((x) => x.id === kind.var);
      if (!s) errors.push(`edge ${e.from} -> ${e.to} waits on a state called "${kind.var}", and the mission has none`);
      else if (!s.values.includes(kind.is)) errors.push(`edge ${e.from} -> ${e.to}: state "${kind.var}" has no value "${kind.is}"`);
    }
  }
  if (def.nodes.filter((n) => n.kind === "entrance").length !== 1) errors.push("a dungeon has exactly one entrance");
  const first = [...def.nodes].sort((a, b) => a.order - b.order)[0];
  if (first && first.kind !== "entrance") errors.push("the entrance must come first in the order");
  errors.push(...spendingOrders(def, catalog, []));
  return errors;
}

// --- C3: every order of spending plain keys -------------------------------------------------

type GraphLock = { edge: number; a: string; b: string; kinds: EdgeKind[] };

/**
 * A plain key fits more than one lock, so she chooses where it goes. Search every order of
 * opening the plain locks she can reach; whenever she runs out of choices, every critical
 * node must have been reached. Everything that is not a choice (a named key, a verb, a
 * flag) is opened the moment it can be.
 */
function spendingOrders(def: DungeonDef, catalog: Catalog, dropped: readonly string[]): string[] {
  const nodes = def.nodes.filter((n) => !dropped.includes(n.id));
  const locks: GraphLock[] = [];
  def.edges.forEach((e, i) => {
    if (e.kind.t === "sight" || dropped.includes(e.from) || dropped.includes(e.to)) return;
    locks.push({ edge: i, a: e.from, b: e.to, kinds: [e.kind, ...(e.also ?? [])] });
  });
  const tagUses: Record<string, number> = {};
  for (const l of locks) for (const kind of l.kinds) if (kind.t === "key") tagUses[kind.tag] = (tagUses[kind.tag] ?? 0) + 1;
  const plain = (l: GraphLock): string | null => {
    for (const kind of l.kinds) if (kind.t === "key" && tagUses[kind.tag] > 1) return kind.tag;
    return null;
  };
  const gains = nodes.map((n) => gainsOf(n, catalog));
  const entrance = nodes.find((n) => n.kind === "entrance");
  if (!entrance) return [];
  const errors: string[] = [];
  const seenStates = new Set<string>();

  const explore = (opened: number[], spent: Record<string, number>, order: string[]): void => {
    const id = [...opened].sort((x, y) => x - y).join(",");
    if (seenStates.has(id) || errors.length > 0) return;
    seenStates.add(id);
    // Settle: reach, collect, open what is not a choice, until nothing moves.
    const open = [...opened];
    let reached: string[] = [];
    for (let guard = 0; guard < 64; guard++) {
      reached = [entrance.id];
      for (let q = 0; q < reached.length; q++) {
        for (const l of locks) {
          if (!open.includes(l.edge) && l.kinds.some((kind) => kind.t !== "open" && kind.t !== "lockin")) continue;
          const next = l.a === reached[q] ? l.b : l.b === reached[q] ? l.a : null;
          if (next && !reached.includes(next)) reached.push(next);
        }
      }
      const verbs = [...def.givenVerbs];
      const flags: string[] = [];
      const controlled: string[] = [];
      const keys: Record<string, number> = {};
      for (const tag of def.givenKeys) keys[tag] = 99;
      nodes.forEach((n, i) => {
        if (!reached.includes(n.id)) return;
        verbs.push(...gains[i].verbs);
        flags.push(...gains[i].flags);
        controlled.push(...gains[i].states);
        for (const tag in gains[i].keys) keys[tag] = (keys[tag] ?? 0) + gains[i].keys[tag];
      });
      let moved = false;
      for (const l of locks) {
        if (open.includes(l.edge) || plain(l) || (!reached.includes(l.a) && !reached.includes(l.b))) continue;
        // A state edge that is open as the building starts needs nothing; otherwise she needs a hand on its control.
        const stateOpen = (id: string, is: string): boolean => (def.states ?? []).some((s) => s.id === id && s.initial === is) || controlled.includes(id);
        const can = l.kinds.every((kind) =>
          kind.t === "key" ? (keys[kind.tag] ?? 0) > 0 : kind.t === "verb" ? verbs.includes(kind.verb) : kind.t === "oneway" ? flags.includes(kind.flag) : kind.t === "state" ? stateOpen(kind.var, kind.is) : true,
        );
        if (can) {
          open.push(l.edge);
          moved = true;
        }
      }
      if (moved) continue;
      // Her choices now: each plain lock in reach that a key in hand fits.
      const choices = locks.filter((l) => {
        const tag = plain(l);
        return tag !== null && !open.includes(l.edge) && (reached.includes(l.a) || reached.includes(l.b)) && (keys[tag] ?? 0) - (spent[tag] ?? 0) > 0;
      });
      if (choices.length === 0) {
        const lost = nodes.filter((n) => n.critical && !reached.includes(n.id)).map((n) => n.id);
        if (lost.length > 0) errors.push(`C3: opening plain locks in the order [${order.join(", ")}] strands her short of ${lost.join(", ")}`);
        return;
      }
      for (const l of choices) {
        const tag = plain(l) as string;
        explore([...open, l.edge], { ...spent, [tag]: (spent[tag] ?? 0) + 1 }, [...order, `${l.a}-${l.b}`]);
      }
      return;
    }
  };
  explore([], {}, []);
  return errors;
}

// --- the checks ---------------------------------------------------------------------------

export function checkDungeon(bp: Blueprint, catalog: Catalog): string[] {
  const info = infoOf(bp);
  if (!info) return ["not a generated dungeon"];
  if (info.errors.length > 0 || !info.layout) return info.errors;
  const def = info.def;
  const contract = contractOf(def);
  const solveWith = solveOptionsOf(def);
  const base = validateBlueprint(bp, catalog, contract, def.givenKeys, { ...solveWith, trace: true });
  if (!base.ok || !base.trace) return base.errors.map((e) => `solver: ${e}`);
  const errors: string[] = [];
  const trace = base.trace;
  const w = bp.w;
  const roomOf = (id: string): RoomInfo | undefined => info.rooms.find((r) => r.node.id === id);
  const passOf = (t: SolveTrace, r: Rect): number => {
    let best = -1;
    for (let y = r.cy; y < r.cy + r.h; y++) {
      for (let x = r.cx; x < r.cx + r.w; x++) {
        const p = t.firstSeen[y * w + x];
        if (p >= 0 && (best < 0 || p < best)) best = p;
      }
    }
    return best;
  };

  // C1. In order, and every lock holds.
  const critical = info.rooms.filter((r) => r.node.critical).sort((a, b) => a.node.order - b.node.order);
  let last = 0;
  let lastOrder = -1;
  let floor = 0;
  for (const r of critical) {
    const p = passOf(trace, r.rect);
    if (p < 0) errors.push(`C1: ${r.node.id} is never reached`);
    // Nodes that share a place in the order may be reached either way round.
    if (r.node.order !== lastOrder) {
      floor = last;
      lastOrder = r.node.order;
    }
    if (p >= 0 && p < floor) errors.push(`C1: ${r.node.id} (order ${r.node.order}) is reached before something that comes earlier in the mission`);
    if (p > last) last = p;
  }
  for (const lock of info.locks) {
    const edge = def.edges[lock.edge];
    if (edge.shortcut) continue;
    const a = roomOf(edge.from);
    const b = roomOf(edge.to);
    if (!a || !b) continue;
    // The far side is the one the lock keeps her out of. Normally that is whichever room the flood
    // reached later; but if one end was never reached at all (it is behind a verb this dungeon does
    // not give her: the glade that waits for Fire), THAT is the far side, however its pass compares.
    const pa = passOf(trace, a.rect);
    const pb = passOf(trace, b.rect);
    const far = pb < 0 ? b : pa < 0 ? a : pb >= pa ? b : a;
    const kind = lock.kind;
    // A state edge is held by its controls: take every one of them away. (One that stands open
    // as the building starts is not a lock on the way in, and proves nothing by this.)
    const initial = kind.t === "state" && (def.states ?? []).some((s) => s.id === kind.var && s.initial === kind.is);
    const withhold =
      kind.t === "key"
        ? { keys: [kind.tag] }
        : kind.t === "verb"
          ? { verbs: [kind.verb] }
          : kind.t === "oneway"
            ? { flags: [kind.flag] }
            : kind.t === "state" && !initial
              ? { props: info.controls.filter((c) => c.state === kind.var).map((c) => c.prop) }
              : null;
    if (!withhold) continue;
    const t = validateBlueprint(bp, catalog, contract, def.givenKeys, { ...solveWith, trace: true, withhold }).trace;
    if (t && passOf(t, far.rect) >= 0) errors.push(`C1: ${far.node.id} can be reached without ${JSON.stringify(withhold)}: the lock on ${edge.from} -> ${edge.to} does not hold`);
  }

  // C3. Plain keys, every order.
  errors.push(...spendingOrders(def, catalog, info.layout.dropped));

  // C4. Everything every sink can eat is no more than the zone supplies.
  const supply: Record<string, number> = {};
  const sinks: Record<string, number> = {};
  const gives = (list: ActionList | undefined): void => {
    eachAction(list, (a) => {
      if (a.do === "give") supply[a.item] = (supply[a.item] ?? 0) + (a.qty ?? 1);
    });
  };
  for (const p of bp.props) {
    for (const s of p.loot ?? []) supply[s.item] = (supply[s.item] ?? 0) + s.qty;
    gives(p.use);
    for (const n of p.needs ?? []) sinks[n.item] = (sinks[n.item] ?? 0) + n.qty;
  }
  for (const u of bp.units) {
    const row = catalog.units[u.def];
    // A thing that respawns is a supply without end; only count what is certain the first time.
    for (const l of row.loot) if (l.chance >= 1) supply[l.item] = (supply[l.item] ?? 0) + l.qty;
  }
  for (const item of Object.keys(sinks).sort()) {
    if (sinks[item] > (supply[item] ?? 0)) errors.push(`C4: the zone can eat ${sinks[item]} ${item} and supplies ${supply[item] ?? 0}`);
  }

  // C5. Taught before demanded falls out of the gated solve. First use in safety does not.
  const inRect = (r: Rect, x: number, y: number): boolean => x >= r.cx && y >= r.cy && x < r.cx + r.w && y < r.cy + r.h;
  for (const room of info.rooms) {
    for (const grant of room.node.grants) {
      if (!("verb" in grant) || def.givenVerbs.includes(grant.verb)) continue;
      const spell = catalog.spells[grant.verb];
      const answers = spell?.kind === "world" ? spell.world : spell?.school;
      const here = bp.props.filter((p) => inRect(room.rect, p.cx, p.cy));
      const teacher = here.find((p) => teaches(p, grant.verb, catalog));
      const firstUse = here.find((p) => catalog.props[p.def].answers === answers);
      if (!teacher) errors.push(`C5: nothing in ${room.node.id} teaches ${grant.verb}`);
      if (!firstUse) {
        errors.push(`C5: ${room.node.id} teaches ${grant.verb} and has nothing to try it on`);
        continue;
      }
      for (const need of firstUse.needs ?? []) {
        const have = here.reduce((n, p) => n + (p.loot ?? []).filter((s) => s.item === need.item).reduce((m, s) => m + s.qty, 0), 0);
        if (have < need.qty) errors.push(`C5: ${firstUse.key} wants ${need.qty} ${need.item} and ${room.node.id} holds ${have}`);
      }
      const guard = teacher ? bp.triggers?.[`${teacher.key}_free`] : undefined;
      const guards = (guard?.when ?? []).flatMap((c) => (c.if === "dead" ? [c.unit] : []));
      for (const u of bp.units) {
        if (!inRect(room.rect, u.cx, u.cy) || catalog.units[u.def].faction === "friendly") continue;
        if (!guards.includes(u.key)) errors.push(`C5: ${grant.verb} can be learned in ${room.node.id} while ${u.key} is still up`);
      }
    }
  }

  // C6. Every lock-in can be followed into: the way in is hidden until the room seals, shown
  // when it does, hidden again after, reached with the gate down, and lands inside the rect.
  for (const l of info.lockins) {
    const wayIn = bp.props.find((p) => p.key === l.wayIn);
    const mark = bp.marks[l.mark];
    const rect = bp.rects[l.rect];
    const lockRow = bp.triggers?.[l.lock];
    const clearRow = bp.triggers?.[l.clear];
    if (!wayIn || !mark || !rect || !lockRow || !clearRow) {
      errors.push(`C6: the lock-in of ${l.node} is missing a part`);
      continue;
    }
    if (!wayIn.hidden) errors.push(`C6: ${l.wayIn} is not hidden, so it is a way round ${l.gate}`);
    if (!inRect(rect, mark.cx, mark.cy)) errors.push(`C6: ${l.mark} is outside ${l.rect}`);
    const shows = lockRow.actions.some((a) => a.do === "show" && a.prop === l.wayIn) && lockRow.actions.some((a) => a.do === "lock" && a.prop === l.gate);
    const hides = clearRow.actions.some((a) => a.do === "hide" && a.prop === l.wayIn) && (lockRow.reset ?? []).some((a) => a.do === "hide" && a.prop === l.wayIn);
    if (!shows || !hides) errors.push(`C6: ${l.lock} must show ${l.wayIn} when it locks ${l.gate}, and the clear and the reset must hide it`);
    const shutOut = validateBlueprint(bp, catalog, contract, def.givenKeys, { ...solveWith, trace: true, shut: [l.gate] }).trace;
    if (shutOut && shutOut.firstSeen[wayIn.cy * w + wayIn.cx] < 0) errors.push(`C6: ${l.wayIn} cannot be reached with ${l.gate} down`);
  }

  // C8 (and C7's depth): the first completion, walked.
  const walk = firstCompletion(bp, catalog, info);
  errors.push(...walk.errors);
  const [lo, hi] = def.budget.critPathCells;
  if (walk.errors.length === 0 && (walk.cells < lo || walk.cells > hi)) errors.push(`C8: the first completion is ${walk.cells} cells of walking, outside ${lo} to ${hi}`);

  // C7. The rest room.
  const rest = info.rooms.find((r) => r.node.kind === "rest");
  if (!rest) errors.push("C7: no rest room");
  else {
    if (rest.node.heat !== 0) errors.push("C7: the rest room has heat");
    if (rest.shape.sockets.some((s) => s.kind === "spawn")) errors.push(`C7: ${rest.shape.template.id} is a rest room with spawn sockets`);
    if (!bp.props.some((p) => inRect(rest.rect, p.cx, p.cy) && catalog.props[p.def].rest)) errors.push("C7: the rest room has nowhere to rest");
    const offHub = def.edges.some((e) => (e.from === rest.node.id && roomOf(e.to)?.node.kind === "hub") || (e.to === rest.node.id && roomOf(e.from)?.node.kind === "hub"));
    if (!offHub) errors.push("C7: the rest room is not off the hub");
    const at = walk.reachedAt[rest.node.id];
    const [from, to] = def.budget.restAt ?? [0.4, 0.6];
    if (walk.errors.length === 0 && walk.cells > 0 && (at === undefined || at / walk.cells < from || at / walk.cells > to)) {
      errors.push(`C7: the rest room is first reached ${at === undefined ? "never" : `${Math.round((100 * at) / walk.cells)}%`} of the way through, outside ${from * 100}% to ${to * 100}%`);
    }
  }

  // C9. A loop, and a shortcut that comes out near the rest room.
  const placedEdges = info.layout.corridors.length;
  if (placedEdges - info.rooms.length + 1 < 1) errors.push("C9: the room graph has no loop");
  const boss = info.rooms.find((r) => r.node.kind === "boss");
  if (rest && boss) {
    const everything = new Set(info.locks.map((l) => l.prop));
    const dist = distances(bp, catalog, rest.centre, everything);
    const toBoss = dist[boss.centre[1] * w + boss.centre[0]];
    if (toBoss < 0 || toBoss > def.budget.restToBossCells) errors.push(`C9: the boss is ${toBoss} cells from the rest room with everything open, over ${def.budget.restToBossCells}`);
    const ends = info.layout.corridors.filter((c) => def.edges[c.edge].shortcut).flatMap((c) => [roomOf(c.a.node), roomOf(c.b.node)]);
    if (ends.length === 0) errors.push("C9: no shortcut was placed");
    else if (!ends.some((r) => r && dist[r.centre[1] * w + r.centre[0]] >= 0 && dist[r.centre[1] * w + r.centre[0]] <= def.budget.restToBossCells)) errors.push("C9: no shortcut comes out near the rest room");
  }

  // C10. The tease: the verb's first lock and the boss gate are seen before they open.
  const teases: string[] = [];
  // Only a verb she finds INSIDE can be teased: the lock is meant to be seen, wondered at, and
  // opened later. A verb she already had at the door is opened in the same flood that reaches it,
  // so there is nothing to tease and nothing to prove (every lock in a return visit is of that kind).
  const given = new Set(def.givenVerbs ?? []);
  const firstVerb = info.locks.find((l) => l.kind.t === "verb" && !def.edges[l.edge].shortcut && !given.has(l.kind.verb));
  if (firstVerb) teases.push(firstVerb.prop);
  const bossGate = boss ? info.locks.find((l) => def.edges[l.edge].to === boss.node.id && l.kind.t !== "verb") : undefined;
  if (bossGate) teases.push(bossGate.prop);
  for (const key of teases) {
    const p = bp.props.find((x) => x.key === key);
    const openedAt = trace.firedAt[key];
    if (!p || openedAt === undefined) continue;
    if (!seenBefore(bp, catalog, trace, p, openedAt)) errors.push(`C10: ${key} is not seen before it can be opened`);
  }

  // C11. A plate whose release re-locks something has something in its room to hold it down.
  const tileFlags = TILE_FLAGS;
  for (const plate of bp.props) {
    if (!catalog.props[plate.def].plate || !(plate.release ?? []).some((a) => a.do === "lock")) continue;
    const room = info.rooms.find((r) => inRect(r.rect, plate.cx, plate.cy));
    if (!room) continue;
    const pd = catalog.props[plate.def];
    const solidAt = new Uint8Array(bp.w * bp.h);
    for (const p of bp.props) {
      const d = catalog.props[p.def];
      if (!d.solid || p.hidden) continue;
      for (let y = p.cy; y < p.cy + d.h; y++) for (let x = p.cx; x < p.cx + d.w; x++) solidAt[y * w + x] = 1;
    }
    const ok = bp.props.some((p) => {
      const d = catalog.props[p.def];
      if (!d.push || !inRect(room.rect, p.cx, p.cy)) return false;
      const own = { cx: p.cx, cy: p.cy, w: d.w, h: d.h };
      return pushPath(bp.w, bp.h, own, { cx: plate.cx, cy: plate.cy, w: pd.w, h: pd.h }, (x, y) => {
        const f = tileFlags[bp.tiles[y * w + x]];
        if ((f & (F_SOLID | F_NOPUSH)) !== 0) return false;
        return solidAt[y * w + x] === 0 || inRect({ cx: own.cx, cy: own.cy, w: own.w, h: own.h }, x, y);
      });
    });
    if (!ok) errors.push(`C11: nothing can be pushed onto ${plate.key}`);
  }

  // C12. Nothing solid appears on whoever caused it. (The engine would stand her aside if it
  // did, sim/clear.ts; a dungeon should still never ask it to.)
  for (const [id, t] of Object.entries(bp.triggers ?? {})) {
    const rect = bp.rects[t.rect];
    eachAction(t.actions, (a) => {
      if (a.do !== "show" && a.do !== "lock") return;
      const p = bp.props.find((x) => x.key === a.prop);
      if (!p) return;
      const d = catalog.props[p.def];
      if (!(d.gate || d.solid)) return;
      if (p.cx < rect.cx + rect.w && p.cx + d.w > rect.cx && p.cy < rect.cy + rect.h && p.cy + d.h > rect.cy) errors.push(`C12: trigger ${id} would put ${p.key} on top of whoever tripped it`);
    });
  }
  return errors;
}

function teaches(p: PropSpawn, verb: string, catalog: Catalog): boolean {
  let found = false;
  const look = (list: ActionList | undefined): void =>
    eachAction(list, (a) => {
      if (a.do === "learn" && a.spell === verb) found = true;
    });
  look(p.use);
  const tree = p.talk ? catalog.dialogue[p.talk] : undefined;
  for (const n of Object.values(tree?.nodes ?? {})) {
    look(n.actions);
    for (const o of n.options ?? []) look(o.actions);
  }
  return found;
}

/** Walking distance in cells from one cell to everywhere, with the named props out of the way. -1 where it cannot get. */
function distances(bp: Blueprint, catalog: Catalog, from: [number, number], open: ReadonlySet<string>): Int32Array {
  const { w, h } = bp;
  const tileFlags = TILE_FLAGS;
  const solidBit = F_SOLID;
  const tiles = bp.tiles;
  const blocked = new Uint8Array(w * h);
  for (const p of bp.props) {
    const d = catalog.props[p.def];
    if (p.hidden || open.has(p.key)) continue;
    const shut = d.gate ? (p.locked ?? false) : d.solid && !d.push && !d.carry;
    if (!shut) continue;
    for (let y = p.cy; y < p.cy + d.h; y++) for (let x = p.cx; x < p.cx + d.w; x++) blocked[y * w + x] = 1;
  }
  const dist = new Int32Array(w * h).fill(-1);
  const queue = new Int32Array(w * h);
  let head = 0;
  let tail = 0;
  // The room's middle may have a pillar on it: start from the nearest cell that is floor.
  let start = from[1] * w + from[0];
  for (let r = 0; r < 8 && ((tileFlags[tiles[start]] & solidBit) !== 0 || blocked[start]); r++) {
    for (let y = from[1] - r; y <= from[1] + r; y++) {
      for (let x = from[0] - r; x <= from[0] + r; x++) {
        const i = y * w + x;
        if (x >= 0 && y >= 0 && x < w && y < h && (tileFlags[tiles[i]] & solidBit) === 0 && !blocked[i]) start = i;
      }
    }
  }
  dist[start] = 0;
  queue[tail++] = start;
  while (head < tail) {
    const i = queue[head++];
    const x = i % w;
    const d = dist[i] + 1;
    if (x + 1 < w && dist[i + 1] < 0 && (tileFlags[tiles[i + 1]] & solidBit) === 0 && !blocked[i + 1]) (dist[i + 1] = d), (queue[tail++] = i + 1);
    if (x > 0 && dist[i - 1] < 0 && (tileFlags[tiles[i - 1]] & solidBit) === 0 && !blocked[i - 1]) (dist[i - 1] = d), (queue[tail++] = i - 1);
    if (i + w < w * h && dist[i + w] < 0 && (tileFlags[tiles[i + w]] & solidBit) === 0 && !blocked[i + w]) (dist[i + w] = d), (queue[tail++] = i + w);
    if (i - w >= 0 && dist[i - w] < 0 && (tileFlags[tiles[i - w]] & solidBit) === 0 && !blocked[i - w]) (dist[i - w] = d), (queue[tail++] = i - w);
  }
  return dist;
}

/** Nearest reachable cell to a room's middle, as a distance. */
function distanceTo(dist: Int32Array, w: number, at: [number, number]): number {
  for (let r = 0; r < 8; r++) {
    let best = -1;
    for (let y = at[1] - r; y <= at[1] + r; y++) {
      for (let x = at[0] - r; x <= at[0] + r; x++) {
        const d = dist[y * w + x];
        if (d >= 0 && (best < 0 || d < best)) best = d;
      }
    }
    if (best >= 0) return best;
  }
  return -1;
}

export type Walk = { cells: number; order: string[]; reachedAt: Record<string, number>; errors: string[] };

/**
 * C8: the first completion. Critical nodes in the mission's order; between each pair, the
 * real walking distance with the locks she has opened so far out of the way. She opens a
 * lock when her next room is behind it and she holds what it wants.
 */
export function firstCompletion(bp: Blueprint, catalog: Catalog, info: BuildInfo): Walk {
  const def = info.def;
  const out: Walk = { cells: 0, order: [], reachedAt: {}, errors: [] };
  const entrance = info.rooms.find((r) => r.node.kind === "entrance");
  if (!entrance || !info.layout) return { ...out, errors: ["C8: no entrance"] };
  const corridors = info.layout.corridors;
  const todo = info.rooms.filter((r) => r.node.critical && r !== entrance).sort((a, b) => a.node.order - b.node.order || def.nodes.indexOf(a.node) - def.nodes.indexOf(b.node));
  const keys: Record<string, number> = {};
  for (const tag of def.givenKeys) keys[tag] = 99;
  const items: Record<string, number> = {};
  const verbs = [...def.givenVerbs];
  const flags: string[] = [];
  const controlled: string[] = [];
  const opened = new Set<string>();
  const openEdges: number[] = [];
  const collect = (node: MissionNode): void => {
    const g = gainsOf(node, catalog);
    for (const t in g.keys) keys[t] = (keys[t] ?? 0) + g.keys[t];
    for (const t in g.items) items[t] = (items[t] ?? 0) + g.items[t];
    verbs.push(...g.verbs);
    flags.push(...g.flags);
    controlled.push(...g.states);
  };
  const canOpen = (kind: EdgeKind): boolean => {
    if (kind.t === "state") return controlled.includes(kind.var) || (def.states ?? []).some((s) => s.id === kind.var && s.initial === kind.is);
    if (kind.t === "key") return (keys[kind.tag] ?? 0) > 0;
    if (kind.t === "verb") return verbs.includes(kind.verb) && (kind.needs ?? []).every((n) => (items[n.item] ?? 0) >= n.qty);
    if (kind.t === "oneway") return flags.includes(kind.flag);
    return true;
  };
  const pay = (kind: EdgeKind): void => {
    if (kind.t === "key" && (keys[kind.tag] ?? 0) < 99) keys[kind.tag]--;
    if (kind.t === "verb") for (const n of kind.needs ?? []) items[n.item] -= n.qty;
  };
  let here: RoomInfo = entrance;
  collect(here.node);
  out.order.push(here.node.id);
  out.reachedAt[here.node.id] = 0;
  while (todo.length > 0) {
    // The first room in the order that she can get to now, by the fewest doors.
    let pick: { room: RoomInfo; path: number[] } | null = null;
    for (const room of todo) {
      const prev: Record<string, { from: string; edge: number }> = {};
      const queue = [here.node.id];
      for (let q = 0; q < queue.length && !prev[room.node.id]; q++) {
        for (const c of corridors) {
          const e: MissionEdge = def.edges[c.edge];
          const next = e.from === queue[q] ? e.to : e.to === queue[q] ? e.from : null;
          if (!next || next === here.node.id || prev[next]) continue;
          if (!openEdges.includes(c.edge) && ![e.kind, ...(e.also ?? [])].every(canOpen)) continue;
          prev[next] = { from: queue[q], edge: c.edge };
          queue.push(next);
        }
      }
      if (!prev[room.node.id]) continue;
      const path: number[] = [];
      for (let at = room.node.id; at !== here.node.id; at = prev[at].from) path.push(prev[at].edge);
      pick = { room, path };
      break;
    }
    if (!pick) {
      out.errors.push(`C8: the walk is stuck in ${here.node.id}, with ${todo.map((r) => r.node.id).join(", ")} still to reach`);
      return out;
    }
    for (const edge of pick.path) {
      if (openEdges.includes(edge)) continue;
      openEdges.push(edge);
      const e = def.edges[edge];
      for (const kind of [e.kind, ...(e.also ?? [])]) pay(kind);
      for (const l of info.locks) if (l.edge === edge) opened.add(l.prop);
    }
    const d = distanceTo(distances(bp, catalog, here.centre, opened), bp.w, pick.room.centre);
    if (d < 0) {
      out.errors.push(`C8: no way to walk from ${here.node.id} to ${pick.room.node.id}`);
      return out;
    }
    out.cells += d;
    here = pick.room;
    todo.splice(todo.indexOf(here), 1);
    out.order.push(here.node.id);
    out.reachedAt[here.node.id] = out.cells;
    collect(here.node);
  }
  return out;
}

/** Is there a cell she stood on in an earlier flood from which the prop is on screen with no wall between? */
function seenBefore(bp: Blueprint, catalog: Catalog, trace: SolveTrace, p: PropSpawn, openedAt: number): boolean {
  const { w, h } = bp;
  const d = catalog.props[p.def];
  const px = p.cx + (d.w >> 1);
  const py = p.cy + (d.h >> 1);
  const tileFlags = TILE_FLAGS;
  const losBit = F_BLOCK_LOS;
  const tiles = bp.tiles;
  const clear = (x0: number, y0: number): boolean => {
    // An integer line walk, cell to cell. Walls only: a gate is what she is looking at.
    let x = x0;
    let y = y0;
    const dx = Math.abs(px - x);
    const dy = Math.abs(py - y);
    const sx = x < px ? 1 : -1;
    const sy = y < py ? 1 : -1;
    let err = dx - dy;
    while (x !== px || y !== py) {
      if ((tileFlags[tiles[y * w + x]] & losBit) !== 0) return false;
      const e2 = err * 2;
      if (e2 > -dy) {
        err -= dy;
        x += sx;
      }
      if (e2 < dx) {
        err += dx;
        y += sy;
      }
    }
    return true;
  };
  // Half a view each way: about 48 x 27 cells are on screen at once.
  for (let y = Math.max(0, py - 12); y <= Math.min(h - 1, py + 12); y++) {
    for (let x = Math.max(0, px - 22); x <= Math.min(w - 1, px + 22); x++) {
      // Strictly earlier: a cell first reached in the flood that opened it may have been
      // reached with the key already in hand, and that is an open door, not a tease.
      const seen = trace.firstSeen[y * w + x];
      if (seen < 0 || seen >= openedAt) continue;
      if (clear(x, y)) return true;
    }
  }
  return false;
}
