// A headless player. It only ever does what a person can: hold a direction,
// press USE, press a bar slot, aim. It finds its way with the sim's own
// pathfinder, so "the bot got there" also proves the map is walkable.

import { buildCatalog, type Catalog } from "@/sim/catalog";
import { CELL } from "@/sim/constants";
import { cellOf, centre } from "@/sim/grid";
import { costOfCells } from "@/sim/path";
import { HOST, newGameState, NO_INPUT, Sim, type InputFrame } from "@/sim/sim";
import { blueprintFor } from "@/sim/zones";
import type { Unit } from "@/sim/state";
import { propCentre } from "@/sim/runtime";

export function idle(sim: Sim, ticks: number): void {
  for (let i = 0; i < ticks; i++) sim.tick(NO_INPUT);
}

/** Walk (sprinting) to within `near` px of a pixel point. Returns false on timeout. */
export function walkTo(sim: Sim, x: number, y: number, near = 6, maxTicks = 60 * 120): boolean {
  let path: number[] | null = null;
  let at = 0;
  let replan = 0;
  for (let t = 0; t < maxTicks; t++) {
    const p = sim.player;
    if (!p.alive) return false;
    const dx = x - p.x;
    const dy = y - p.y;
    if (Math.sqrt(dx * dx + dy * dy) <= near) return true;
    if (sim.me.dialogue) return false;
    if (!path || at >= path.length || replan-- <= 0) {
      const goal = sim.rt.grid.nearestFree(cellOf(x), cellOf(y), 6, p.id);
      if (!goal) return false;
      path = sim.rt.path.find(cellOf(p.x), cellOf(p.y), goal.cx, goal.cy, p.id, costOfCells(4000), 400000);
      at = 0;
      replan = 90;
      if (!path) return false;
      if (path.length === 0) return true;
    }
    const cell = path[at];
    const tx = centre(cell % sim.rt.grid.w);
    const ty = centre(Math.floor(cell / sim.rt.grid.w));
    const ddx = tx - p.x;
    const ddy = ty - p.y;
    const d = Math.sqrt(ddx * ddx + ddy * ddy);
    if (d < 1.5) {
      at++;
      continue;
    }
    const frame: InputFrame = { mx: ddx / d, my: ddy / d, sprint: d > CELL * 2, useHeld: false, ax: 0, ay: 0 };
    sim.tick(frame);
  }
  return false;
}

export function walkToProp(sim: Sim, key: string): boolean {
  const prop = sim.rt.propsByKey.get(key);
  if (!prop) throw new Error(`no prop ${key}`);
  const c = propCentre(sim.catalog, prop);
  const def = sim.catalog.props[prop.def];
  // Try each side of the footprint until one is reachable.
  const sides: [number, number][] = [
    [c.x, (prop.cy + def.h) * CELL + 5],
    [c.x, prop.cy * CELL - 5],
    [prop.cx * CELL - 5, c.y],
    [(prop.cx + def.w) * CELL + 5, c.y],
  ];
  for (const [x, y] of sides) {
    if (sim.rt.grid.solid(cellOf(x), cellOf(y))) continue;
    if (walkTo(sim, x, y, 3)) {
      face(sim, c.x, c.y);
      return true;
    }
  }
  return false;
}

/**
 * Push a `push` prop one cell, as a person does: stand against a side, face it, and hold USE
 * leaning into it. Tries each side until the prop moves. Returns false if it never did.
 */
export function pushProp(sim: Sim, key: string): boolean {
  const prop = sim.rt.propsByKey.get(key);
  if (!prop) throw new Error(`no prop ${key}`);
  const def = sim.catalog.props[prop.def];
  const c = propCentre(sim.catalog, prop);
  const sides: [number, number, number, number][] = [
    [c.x, (prop.cy + def.h) * CELL + 4, 0, -1],
    [c.x, prop.cy * CELL - 4, 0, 1],
    [prop.cx * CELL - 4, c.y, 1, 0],
    [(prop.cx + def.w) * CELL + 4, c.y, -1, 0],
  ];
  const [x0, y0] = [prop.cx, prop.cy];
  for (const [x, y, mx, my] of sides) {
    if (sim.rt.grid.solid(cellOf(x), cellOf(y))) continue;
    if (!walkTo(sim, x, y, 2)) continue;
    sim.tick({ mx: mx * 0.2, my: my * 0.2, sprint: false, useHeld: false, ax: 0, ay: 0 });
    for (let t = 0; t < 45 && prop.cx === x0 && prop.cy === y0; t++) sim.tick({ mx, my, sprint: false, useHeld: true, ax: 0, ay: 0 });
    idle(sim, 2);
    if (prop.cx !== x0 || prop.cy !== y0) return true;
  }
  return false;
}

export function walkToUnit(sim: Sim, key: string, near = 14): Unit {
  const u = sim.rt.unitsByKey.get(key);
  if (!u) throw new Error(`no unit ${key}`);
  walkTo(sim, u.x, u.y, near);
  face(sim, u.x, u.y);
  return u;
}

/** Nudge toward a point for two ticks so facing turns that way. */
export function face(sim: Sim, x: number, y: number): void {
  const p = sim.player;
  const dx = x - p.x;
  const dy = y - p.y;
  const d = Math.sqrt(dx * dx + dy * dy) || 1;
  const ax = Math.abs(dx) >= Math.abs(dy) ? Math.sign(dx) : 0;
  const ay = ax === 0 ? Math.sign(dy) : 0;
  void d;
  sim.tick({ mx: ax * 0.2, my: ay * 0.2, sprint: false, useHeld: false, ax: 0, ay: 0 });
}

/** Click through a dialogue, taking `choices` in order at each option prompt (default: first). */
export function talkThrough(sim: Sim, choices: number[] = []): void {
  let guard = 0;
  let n = 0;
  while (sim.me.dialogue && guard++ < 200) {
    const d = sim.me.dialogue;
    const node = sim.catalog.dialogue[d.tree].nodes[d.node];
    const last = d.line >= node.lines.length - 1;
    if (last && (node.options?.length ?? 0) > 0) sim.command({ t: "choose", option: choices[n++] ?? 0 });
    else sim.command({ t: "advance" });
  }
}

/** Melee an enemy to death, chasing it. Bar slot 0 is melee on a new game. */
export function fight(sim: Sim, target: Unit, slot = 0, maxTicks = 60 * 90): boolean {
  for (let t = 0; t < maxTicks; t++) {
    const p = sim.player;
    if (!p.alive) return false;
    if (!target.alive) return true;
    const dx = target.x - p.x;
    const dy = target.y - p.y;
    const d = Math.sqrt(dx * dx + dy * dy) || 1;
    sim.setAim(dx / d, dy / d);
    if (d < 18) sim.command({ t: "bar", slot });
    const move = d > 12 ? 1 : 0;
    sim.tick({ mx: (dx / d) * move, my: (dy / d) * move, sprint: false, useHeld: false, ax: dx / d, ay: dy / d });
  }
  return !target.alive;
}

/**
 * A game that begins inside a zone, standing on one of its marks. Everyone who sits down
 * arrives at the party's last fire, so the fire is put there first: no county is built, which
 * is a second saved per test and keeps a dungeon's tests about the dungeon.
 */
export function simIn(catalog: Catalog, zone: string, seed: number, mark = "entry"): Sim {
  const state = newGameState(seed);
  const at = blueprintFor(zone, state.seed).marks[mark];
  if (!at) throw new Error(`zone ${zone} has no mark ${mark}`);
  state.rest = { zone, x: centre(at.cx), y: centre(at.cy) };
  const sim = Sim.fromState(catalog, state);
  sim.command(-1, { t: "join", who: HOST });
  sim.me.lastMark = mark;
  return sim;
}

/**
 * The catalog the older tests play on. New Game stands her on the station platform now, two
 * minutes' walk from anything; these tests are about the yard, the house and what is under
 * them, so they begin where the small county used to: just inside Julie's gate.
 */
export function yardCatalog(): Catalog {
  const catalog = buildCatalog();
  catalog.start.mark = "yard_gate";
  return catalog;
}
