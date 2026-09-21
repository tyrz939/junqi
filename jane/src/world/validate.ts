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

import type { Catalog } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import type { Action, ActionList } from "@/sim/state";
import type { Blueprint, PropSpawn, ZoneContract } from "@/world/blueprint";

export type Validation = { ok: boolean; errors: string[]; reachedCells: number };

export function validateBlueprint(
  bp: Blueprint,
  catalog: Catalog,
  contract: ZoneContract,
  givenKeys: readonly string[] = [],
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
  for (const [id, t] of Object.entries(catalog.triggers)) {
    if (t.zone === bp.zone && !bp.rects[t.rect]) errors.push(`trigger "${id}": zone has no rect "${t.rect}"`);
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
  const blocked = new Uint8Array(w * h);
  const restamp = (): void => {
    blocked.fill(0);
    for (const p of bp.props) {
      const def = catalog.props[p.def];
      if (hidden.has(p.key)) continue;
      const isShut = def.gate ? (p.locked ?? false) && !open.has(p.key) : def.solid && !def.push && !def.carry;
      if (!isShut) continue;
      for (let y = p.cy; y < p.cy + def.h; y++) for (let x = p.cx; x < p.cx + def.w; x++) if (x >= 0 && y >= 0 && x < w && y < h) blocked[y * w + x] = 1;
    }
  };

  for (const p of bp.props) {
    const def = catalog.props[p.def];
    if (p.cx < 0 || p.cy < 0 || p.cx + def.w > w || p.cy + def.h > h) errors.push(`prop "${p.key}" is outside the zone`);
  }
  if (errors.length > 0) return { ok: false, errors, reachedCells: 0 };

  const keys = new Map<string, number>();
  for (const k of givenKeys) keys.set(k, (keys.get(k) ?? 0) + 99);
  const looted = new Set<string>();
  const fired = new Set<string>();
  const deadUnits = new Set<string>();
  const seen = new Uint8Array(w * h);
  const queue = new Int32Array(w * h);
  const starts = Object.values(bp.marks).filter((_, i) => i >= 0);
  const entry = bp.marks.start ?? bp.marks.entry ?? bp.marks.front ?? starts[0];

  const applyActions = (list: ActionList | undefined | null): void => {
    for (const a of list ?? []) applyAction(a);
  };
  // A flood is the whole zone, and the county is seven million cells: flood again only when
  // something that blocks has changed, not every time a herb is picked or a rat dies.
  let opened = false;
  const applyAction = (a: Action): void => {
    if (a.do === "unlock") {
      open.add(a.prop);
      opened = true;
    } else if (a.do === "hide") {
      hidden.add(a.prop);
      opened = true;
    } else if (a.do === "show") {
      hidden.delete(a.prop);
      opened = true;
    }
    else if (a.do === "give") keys.set(itemTag(a.item), (keys.get(itemTag(a.item)) ?? 0) + (a.qty ?? 1));
  };
  const itemTag = (item: string): string => catalog.items[item]?.opens ?? `item:${item}`;

  let reachedCells = 0;
  for (let pass = 0; pass < 64; pass++) {
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
      if (hidden.has(p.key) || !touches(p)) continue;
      const def = catalog.props[p.def];
      const lockedNow = (p.locked ?? false) && !open.has(p.key);
      if (lockedNow) {
        const have = p.keyTag ? (keys.get(p.keyTag) ?? 0) : 0;
        if (have > 0) {
          keys.set(p.keyTag as string, have - 1);
          open.add(p.key);
          opened = true;
          progress = true;
        }
        continue;
      }
      if (p.loot && !looted.has(p.key)) {
        looted.add(p.key);
        for (const s of p.loot) keys.set(itemTag(s.item), (keys.get(itemTag(s.item)) ?? 0) + s.qty);
        applyActions(p.use);
        progress = true;
      } else if (p.use && !p.loot && !fired.has(p.key)) {
        if (def.answers === "repair") {
          const needs = p.needs ?? [];
          if (!needs.every((n) => (keys.get(`item:${n.item}`) ?? 0) >= n.qty)) continue;
          for (const n of needs) keys.set(`item:${n.item}`, (keys.get(`item:${n.item}`) ?? 0) - n.qty);
        }
        fired.add(p.key);
        applyActions(p.use);
        progress = true;
      }
    }
    for (const u of bp.units) {
      if (deadUnits.has(u.key) || !seen[u.cy * w + u.cx]) continue;
      const def = catalog.units[u.def];
      if (def.faction === "friendly") continue;
      deadUnits.add(u.key);
      for (const l of def.loot) if (l.chance >= 1) keys.set(itemTag(l.item), (keys.get(itemTag(l.item)) ?? 0) + l.qty);
      applyActions(def.onDeath);
      progress = true;
    }
    for (const [id, t] of Object.entries(catalog.triggers)) {
      if (t.zone !== bp.zone || fired.has(`trigger:${id}`)) continue;
      const r = bp.rects[t.rect];
      let inside = false;
      for (let y = r.cy; y < r.cy + r.h && !inside; y++) for (let x = r.cx; x < r.cx + r.w && !inside; x++) inside = seen[y * w + x] === 1;
      if (!inside) continue;
      // Only the "everything is dead" half of a lock-in matters to reachability:
      // the solver assumes fights are won, so `while` triggers fire, `enter` locks are skipped.
      if ((t.mode ?? "enter") === "while") {
        fired.add(`trigger:${id}`);
        applyActions(t.actions);
        progress = true;
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
  return { ok: errors.length === 0, errors, reachedCells };
}
