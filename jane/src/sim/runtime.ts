// Derived, unsaved data for the zone the player is in, plus the `World` context
// every system function takes. Rebuilt from (blueprint, ZoneState) on zone entry
// and on load; throwing it away and rebuilding must never change behaviour.

import type { Catalog } from "@/sim/catalog";
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
  propFlagsDirty: boolean;
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

export function buildRuntime(catalog: Catalog, bp: Blueprint, zone: ZoneState): ZoneRuntime {
  const tiles = bp.tiles.slice();
  for (let i = 0; i + 1 < zone.tileDeltas.length; i += 2) tiles[zone.tileDeltas[i]] = zone.tileDeltas[i + 1];
  const grid = new Grid(bp.w, bp.h, tiles);
  const rt: ZoneRuntime = {
    bp,
    grid,
    path: new PathFinder(grid),
    units: new Map(),
    unitsByKey: new Map(),
    props: new Map(),
    propsByKey: new Map(),
    propFlagsDirty: true,
    ringKey: "",
    pathsThisTick: 0,
    fogW: Math.ceil(bp.w / 2),
    fogH: Math.ceil(bp.h / 2),
  };
  for (const p of zone.props) {
    rt.props.set(p.id, p);
    if (p.key) rt.propsByKey.set(p.key, p);
  }
  for (const u of zone.units) {
    rt.units.set(u.id, u);
    if (u.key) rt.unitsByKey.set(u.key, u);
  }
  refreshPropFlags(catalog, rt, zone);
  for (const u of zone.units) if (u.alive && u.awake) occupy(rt, u);
  return rt;
}

/** Re-stamp solid props onto the grid. Cheap enough to do whenever a prop changes. */
export function refreshPropFlags(catalog: Catalog, rt: ZoneRuntime, zone: ZoneState): void {
  rt.grid.clearPropFlags();
  for (const p of zone.props) {
    if (!p.solid || p.hidden) continue;
    const def = catalog.props[p.def];
    rt.grid.stampProp(p.cx, p.cy, def.w, def.h, def.blockLos);
  }
  rt.propFlagsDirty = false;
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
