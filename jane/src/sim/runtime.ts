// Derived, unsaved data for the zone the player is in, plus the `World` context
// every system function takes. Rebuilt from (blueprint, ZoneState) on zone entry
// and on load; throwing it away and rebuilding must never change behaviour.

import type { Catalog, TriggerDef } from "@/sim/catalog";
import { CELL } from "@/sim/constants";
import type { SimEvent } from "@/sim/events";
import { cellOf, Grid } from "@/sim/grid";
import { PathFinder } from "@/sim/path";
import type { GameState, PlayerState, Prop, Unit, ZoneState } from "@/sim/state";
import type { Blueprint } from "@/world/blueprint";

export type ZoneRuntime = {
  bp: Blueprint;
  grid: Grid;
  path: PathFinder;
  units: Map<number, Unit>;
  unitsByKey: Map<string, Unit>;
  props: Map<number, Prop>;
  propsByKey: Map<string, Prop>;
  /**
   * Props bucketed by PROP_BLOCK-cell block, keyed `by * propBlocksW + bx`, each bucket in
   * ascending prop id. Only ever looked up by key, never iterated as a whole.
   */
  propBuckets: (Prop[] | undefined)[];
  propBlocksW: number;
  propBlocksH: number;
  /** Largest prop footprint in the catalog, minus one, in cells: how far back a query must reach. */
  propReachW: number;
  propReachH: number;
  /** Largest light radius in the catalog, px: how far a "is this point lit" query must look (sim/light.ts). */
  lightReach: number;
  /** Every prop whose `awake` flag is set. The ring clears these and sets the new ones; it never scans the zone. */
  awakeProps: Prop[];
  /** Pressure plates, in id order. A plate is a def field, so the list only changes when a prop is added. */
  plates: Prop[];
  /** This zone's trigger rows by id: the catalog's, plus whatever the blueprint carries. Looked up, never iterated. */
  triggers: Record<string, TriggerDef>;
  /** Everything changed: re-stamp every solid prop at the end of the tick. */
  propFlagsDirty: boolean;
  /** Something small changed: inclusive cell rects (cx0, cy0, cx1, cy1, ...) to re-stamp at the end of the tick. */
  propDirty: number[];
  /** Signature of every player's load-ring block in this zone; "" forces a rebuild next tick. */
  ringKey: string;
  pathsThisTick: number;
  fogW: number;
  fogH: number;
};

/**
 * The context every system function takes: ONE zone, and optionally the player a
 * call is being made on behalf of.
 *
 * Up to four people share a world and may be in different zones, so there is no
 * such thing as "the zone" or "the player" any more. The scheduler keeps one of
 * these per live zone, sets `actor` before anything player-scoped (her input, her
 * command, a trigger she tripped, a dialogue action she chose) and clears it after.
 * Zone-wide steps (AI, bolts, the flush) run with `actor = null`.
 */
export type World = {
  readonly catalog: Catalog;
  state: GameState;
  zone: ZoneState;
  rt: ZoneRuntime;
  /** Who this call is for. Null during zone-wide steps. */
  actor: PlayerState | null;
  /** The party, across every zone. */
  party: Party;
  /**
   * Presentation events. With an actor, personal kinds (toast, loot, shake...) go to
   * her alone; `all` sends to the whole party regardless (quest news, the bell).
   */
  emit(ev: SimEvent, all?: boolean): void;
};

export type Party = {
  /** Connected players. The co-op penalty reads this, wherever they are standing. */
  size(): number;
  /** The player whose body this unit is, if any. */
  ofUnit(unitId: number): PlayerState | undefined;
  /** Every connected player's unit, in seat order, whatever zone it is in. */
  units(): Unit[];
  /** Every body that has ever sat down, connected or parked. What the whole table learns, they all learn. */
  bodies(): Unit[];
  /** True when every connected player is within reach of a bed or a fire. */
  everyoneResting(): boolean;
  /** Run something once for each connected player, in her own zone's context, acting as her. */
  each(fn: (w: World, player: PlayerState) => void): void;
};

/**
 * Every trigger row of a zone: the catalog's rows for it, then the rows its blueprint wrote
 * (a generated lock-in, a gate that opens on a flag). The catalog wins an id clash here;
 * the validator has already refused a blueprint that has one.
 */
export function zoneTriggers(catalog: Catalog, bp: Blueprint): Record<string, TriggerDef> {
  const out: Record<string, TriggerDef> = {};
  for (const id in catalog.triggers) if (catalog.triggers[id].zone === bp.zone) out[id] = catalog.triggers[id];
  for (const id in bp.triggers ?? {}) if (!(id in out)) out[id] = (bp.triggers as Record<string, TriggerDef>)[id];
  return out;
}

export function buildRuntime(catalog: Catalog, bp: Blueprint, zone: ZoneState): ZoneRuntime {
  const tiles = bp.tiles.slice();
  for (let i = 0; i + 1 < zone.tileDeltas.length; i += 2) tiles[zone.tileDeltas[i]] = zone.tileDeltas[i + 1];
  const grid = new Grid(bp.w, bp.h, tiles);
  let reachW = 0;
  let reachH = 0;
  let lightReach = 0;
  for (const id in catalog.props) {
    reachW = Math.max(reachW, catalog.props[id].w - 1);
    reachH = Math.max(reachH, catalog.props[id].h - 1);
    lightReach = Math.max(lightReach, catalog.props[id].light?.radius ?? 0);
  }
  const rt: ZoneRuntime = {
    bp,
    grid,
    path: new PathFinder(grid),
    units: new Map(),
    unitsByKey: new Map(),
    props: new Map(),
    propsByKey: new Map(),
    propBuckets: [],
    propBlocksW: Math.max(1, Math.ceil(bp.w / PROP_BLOCK)),
    propBlocksH: Math.max(1, Math.ceil(bp.h / PROP_BLOCK)),
    propReachW: reachW,
    propReachH: reachH,
    lightReach,
    awakeProps: [],
    plates: [],
    triggers: zoneTriggers(catalog, bp),
    propFlagsDirty: true,
    propDirty: [],
    ringKey: "",
    pathsThisTick: 0,
    fogW: Math.ceil(bp.w / 2),
    fogH: Math.ceil(bp.h / 2),
  };
  for (const p of zone.props) indexProp(catalog, rt, p);
  for (const u of zone.units) {
    rt.units.set(u.id, u);
    if (u.key) rt.unitsByKey.set(u.key, u);
  }
  refreshPropFlags(catalog, rt, zone);
  for (const u of zone.units) if (u.alive && u.awake) occupy(rt, u);
  return rt;
}

/**
 * Re-stamp every solid prop onto the grid. A pass over the whole zone: for zone entry,
 * load, and anyone who sets `propFlagsDirty` by hand. One prop changing goes through
 * `touchProp` and costs its own footprint.
 */
export function refreshPropFlags(catalog: Catalog, rt: ZoneRuntime, zone: ZoneState): void {
  rt.grid.clearPropFlags();
  for (const p of zone.props) {
    if (!p.solid || p.hidden) continue;
    const def = catalog.props[p.def];
    rt.grid.stampProp(p.cx, p.cy, def.w, def.h, def.blockLos);
  }
  rt.propFlagsDirty = false;
  rt.propDirty.length = 0;
}

/** Housekeeping: bring the grid's prop flags up to date with whatever changed since the last call. */
export function flushPropFlags(catalog: Catalog, rt: ZoneRuntime, zone: ZoneState): void {
  if (rt.propFlagsDirty) {
    refreshPropFlags(catalog, rt, zone);
    return;
  }
  const d = rt.propDirty;
  if (d.length === 0) return;
  for (let i = 0; i + 3 < d.length; i += 4) restampCells(catalog, rt, d[i], d[i + 1], d[i + 2], d[i + 3]);
  d.length = 0;
}

/**
 * Clear the prop flags of a rect and stamp back every solid prop that touches it.
 * Stamping is an OR, so a prop that pokes out of the rect re-stamps cells that
 * already carry its bits and nothing else changes.
 */
export function restampCells(catalog: Catalog, rt: ZoneRuntime, cx0: number, cy0: number, cx1: number, cy1: number): void {
  rt.grid.clearPropFlagsIn(cx0, cy0, cx1, cy1);
  for (const p of propsInCells(rt, cx0, cy0, cx1, cy1)) {
    if (!p.solid || p.hidden) continue;
    const def = catalog.props[p.def];
    rt.grid.stampProp(p.cx, p.cy, def.w, def.h, def.blockLos);
  }
}

/** `solid` or `hidden` changed on this prop: its footprint is re-stamped at the end of the tick. */
export function touchProp(w: World, p: Prop): void {
  const def = w.catalog.props[p.def];
  w.rt.propDirty.push(p.cx, p.cy, p.cx + def.w - 1, p.cy + def.h - 1);
}

// --- props by block ----------------------------------------------------------
// A county of thousands of props cannot be scanned for every "what is in front of
// her". Props are bucketed by block, and everything per tick or per frame asks the
// buckets.
//
//   one bucket each   A prop lives in the bucket of its ORIGIN cell (cx, cy) and
//                     nowhere else. A query reaches back by the largest footprint in
//                     the catalog (the car wreck, 5 x 3) instead, so a prop is found
//                     from any cell its footprint covers, and nothing needs de-duping.
//   id order          zone.props is in ascending id: createZoneState numbers props as
//                     it makes them, addProp appends, nothing reorders or removes. A
//                     query answers in ascending id too, so "first match" and "ties
//                     go to the earlier prop" mean what they meant under a linear scan.
//   a superset        A query returns whole blocks. Callers still test exactly, as
//                     they did when the candidate list was the entire zone.
//
// A carried prop keeps the cell it was lifted from, as it always has, and so keeps
// its bucket. It is not solid while carried; it is re-bucketed when it is put down.

/** Block edge, in cells. 16 cells = 128 px; a view's worth is about a dozen blocks. */
export const PROP_BLOCK = 16;

function propBlockOf(rt: ZoneRuntime, cx: number, cy: number): number {
  const bx = Math.min(rt.propBlocksW - 1, Math.max(0, Math.floor(cx / PROP_BLOCK)));
  const by = Math.min(rt.propBlocksH - 1, Math.max(0, Math.floor(cy / PROP_BLOCK)));
  return by * rt.propBlocksW + bx;
}

function bucketInsert(rt: ZoneRuntime, block: number, p: Prop): void {
  const bucket = (rt.propBuckets[block] ??= []);
  let at = bucket.length;
  while (at > 0 && bucket[at - 1].id > p.id) at--;
  bucket.splice(at, 0, p);
}

function indexProp(catalog: Catalog, rt: ZoneRuntime, p: Prop): void {
  rt.props.set(p.id, p);
  if (p.key) rt.propsByKey.set(p.key, p);
  bucketInsert(rt, propBlockOf(rt, p.cx, p.cy), p);
  if (p.awake) rt.awakeProps.push(p);
  if (catalog.props[p.def].plate) rt.plates.push(p);
}

/**
 * Every prop whose footprint may touch the inclusive cell rect, in ascending id.
 * Pass `out` to reuse an array (it is emptied first); the renderer does, per frame.
 */
export function propsInCells(rt: ZoneRuntime, cx0: number, cy0: number, cx1: number, cy1: number, out: Prop[] = []): Prop[] {
  out.length = 0;
  // A prop off the edge of the grid is bucketed in the edge block, so the query clamps the same way.
  const bx0 = Math.min(rt.propBlocksW - 1, Math.max(0, Math.floor((cx0 - rt.propReachW) / PROP_BLOCK)));
  const by0 = Math.min(rt.propBlocksH - 1, Math.max(0, Math.floor((cy0 - rt.propReachH) / PROP_BLOCK)));
  const bx1 = Math.min(rt.propBlocksW - 1, Math.max(0, Math.floor(cx1 / PROP_BLOCK)));
  const by1 = Math.min(rt.propBlocksH - 1, Math.max(0, Math.floor(cy1 / PROP_BLOCK)));
  let buckets = 0;
  for (let by = by0; by <= by1; by++) {
    for (let bx = bx0; bx <= bx1; bx++) {
      const bucket = rt.propBuckets[by * rt.propBlocksW + bx];
      if (!bucket || bucket.length === 0) continue;
      for (let i = 0; i < bucket.length; i++) out.push(bucket[i]);
      buckets++;
    }
  }
  // Each bucket is sorted; two or more together are not. Ids are unique, so the sort has one answer.
  if (buckets > 1) out.sort((a, b) => a.id - b.id);
  return out;
}

/**
 * Every prop that may lie within `radiusPx` of a point, in ascending id. One cell of
 * slack each way: reach is measured to a footprint's closed edge, which is the next
 * cell's first pixel.
 */
export function propsNear(rt: ZoneRuntime, x: number, y: number, radiusPx: number, out?: Prop[]): Prop[] {
  return propsInCells(rt, cellOf(x - radiusPx) - 1, cellOf(y - radiusPx) - 1, cellOf(x + radiusPx) + 1, cellOf(y + radiusPx) + 1, out);
}

/**
 * A new prop in a live zone. Ids only grow, so appending keeps zone.props in id order.
 * A new prop can only add to what is solid, so it is stamped here and now.
 */
export function addProp(w: World, p: Prop): void {
  w.zone.props.push(p);
  indexProp(w.catalog, w.rt, p);
  const def = w.catalog.props[p.def];
  if (p.solid && !p.hidden) w.rt.grid.stampProp(p.cx, p.cy, def.w, def.h, def.blockLos);
}

/**
 * The ONLY way a prop changes cell: push, pull, put down, dropped where she fell.
 * Re-buckets it and marks both footprints for re-stamping.
 */
export function moveProp(w: World, p: Prop, cx: number, cy: number): void {
  const rt = w.rt;
  touchProp(w, p);
  const from = propBlockOf(rt, p.cx, p.cy);
  const to = propBlockOf(rt, cx, cy);
  p.cx = cx;
  p.cy = cy;
  if (from !== to) {
    const bucket = rt.propBuckets[from];
    const at = bucket ? bucket.indexOf(p) : -1;
    if (bucket && at >= 0) bucket.splice(at, 1);
    bucketInsert(rt, to, p);
  }
  touchProp(w, p);
}

export function occupy(rt: ZoneRuntime, u: Unit): void {
  const cx = cellOf(u.x);
  const cy = cellOf(u.y);
  if (!rt.grid.inside(cx, cy)) return;
  rt.grid.occupy(rt.grid.index(cx, cy), u.id);
}

export function vacate(rt: ZoneRuntime, u: Unit): void {
  const cx = cellOf(u.x);
  const cy = cellOf(u.y);
  if (!rt.grid.inside(cx, cy)) return;
  rt.grid.vacate(rt.grid.index(cx, cy), u.id);
}

export function addUnit(w: World, u: Unit): void {
  w.zone.units.push(u);
  w.rt.units.set(u.id, u);
  if (u.key) w.rt.unitsByKey.set(u.key, u);
  if (u.alive && u.awake) occupy(w.rt, u);
}

export function removeUnit(w: World, u: Unit): void {
  vacate(w.rt, u);
  const i = w.zone.units.indexOf(u);
  if (i >= 0) w.zone.units.splice(i, 1);
  w.rt.units.delete(u.id);
  if (u.key && w.rt.unitsByKey.get(u.key) === u) w.rt.unitsByKey.delete(u.key);
}

/** The acting player's body. Only valid in player-scoped calls, in her own zone. */
export function playerOf(w: World): Unit {
  if (!w.actor) throw new Error("No acting player in this context (a zone-wide step asked for one)");
  const p = w.rt.units.get(w.actor.unitId);
  if (!p) throw new Error(`Player ${w.actor.index} is not in zone ${w.zone.id}`);
  return p;
}

/** Connected players whose body is in this zone, in seat order. */
export function playersHere(w: World): PlayerState[] {
  const out: PlayerState[] = [];
  for (const p of w.state.players) if (p.connected && p.zone === w.zone.id && w.rt.units.has(p.unitId)) out.push(p);
  return out;
}

/** Run `fn` on behalf of a player, restoring whoever the context was acting for before. */
export function asPlayer<T>(w: World, actor: PlayerState | null, fn: () => T): T {
  const before = w.actor;
  w.actor = actor;
  try {
    return fn();
  } finally {
    w.actor = before;
  }
}

export function propCentre(catalog: Catalog, p: Prop): { x: number; y: number } {
  const def = catalog.props[p.def];
  return { x: (p.cx + def.w / 2) * CELL, y: (p.cy + def.h / 2) * CELL };
}
