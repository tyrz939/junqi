// One Unit. Player, dog, skeleton and snake are the same shape with a different
// `controller`. Do not split Player / Enemy; add a field here or a row in units.json.

import type { Catalog, UnitDef } from "@/sim/catalog";
import {
  BAG_SLOTS,
  BODY_HALF,
  CELL,
  ENERGY_MAX,
  HP_PER_STRENGTH,
  MP_PER_SPIRIT,
} from "@/sim/constants";
import { cellOf } from "@/sim/grid";
import { irandom } from "@/sim/rng";
import { occupy, vacate, type World } from "@/sim/runtime";
import { FACING_DX, FACING_DY, type Facing, type GameState, type Unit } from "@/sim/state";
import { AGGRO_PERIOD } from "@/sim/constants";

export function maxHp(u: Unit): number {
  return u.strength * HP_PER_STRENGTH;
}

export function maxMp(u: Unit): number {
  return u.spirit * MP_PER_SPIRIT;
}

/** `is_enemy` was simply `faction !=`. It still is. */
/**
 * Everything that bites is her enemy and none of it is anyone else's. The county is thick with
 * camps of different families now, a ruffian's fire forty metres from a nest of rats; if every
 * faction fought every other the fields would empty themselves and the fighting would follow her
 * round. The county does not take sides against itself: only the friendly side has enemies.
 */
export function isEnemy(a: Unit, b: Unit): boolean {
  return a.faction !== b.faction && (a.faction === "friendly" || b.faction === "friendly");
}

export function createUnit(
  state: GameState,
  catalog: Catalog,
  defId: string,
  key: string,
  x: number,
  y: number,
  facing: Facing = 1,
): Unit {
  const def: UnitDef = catalog.units[defId];
  if (!def) throw new Error(`Unknown unit def "${defId}"`);
  const u: Unit = {
    id: state.nextId++,
    key,
    def: defId,
    controller: def.controller,
    faction: def.faction,
    x,
    y,
    facing,
    strength: def.strength,
    spirit: def.spirit,
    hp: def.strength * HP_PER_STRENGTH,
    mp: def.spirit * MP_PER_SPIRIT,
    energy: ENERGY_MAX,
    energyLocked: false,
    alive: true,
    anim: "idle",
    animTick: 0,
    gcd: 0,
    stop: 0,
    cooldowns: {},
    itemCooldowns: {},
    book: [...def.book],
    bag: def.controller === "player" ? Array.from({ length: BAG_SLOTS }, () => null) : null,
    target: 0,
    combat: "idle",
    homeX: x,
    homeY: y,
    patrol: null,
    patrolAt: 0,
    patrolDwell: null,
    dwell: 0,
    order: null,
    path: null,
    pathGoal: -1,
    pathAt: 0,
    repathIn: 0,
    // 2020 drew irandom(9) once and compared a decrementing counter to 0; a unit
    // that drew 0 never scanned again. Here the offset feeds (tick + offset) % period.
    thinkOffset: irandom(state.rng, AGGRO_PERIOD - 1),
    incoming: [],
    statuses: [],
    deadFor: 0,
    respawn: def.respawn,
    awake: true,
    hidden: false,
    carrying: 0,
    hold: 0,
    segments: null,
    hx: FACING_DX[facing],
    hy: FACING_DY[facing],
    phase: 0,
    phaseTick: 0,
    phaseStep: 0,
  };
  if (def.body) {
    u.segments = [];
    for (let i = 0; i < def.body.segments; i++) u.segments.push(x, y);
  }
  return u;
}

export function setAnim(u: Unit, anim: Unit["anim"]): void {
  if (u.anim === anim) return;
  u.anim = anim;
  u.animTick = 0;
}

/** Dominant axis wins; ties keep the current facing so diagonals do not flicker. */
export function faceVector(u: Unit, dx: number, dy: number): void {
  const ax = Math.abs(dx);
  const ay = Math.abs(dy);
  if (ax === 0 && ay === 0) return;
  if (ax > ay) u.facing = dx > 0 ? 0 : 2;
  else if (ay > ax) u.facing = dy > 0 ? 1 : 3;
  else if (u.facing === 0 || u.facing === 2) u.facing = dx > 0 ? 0 : 2;
  else u.facing = dy > 0 ? 1 : 3;
}

export function facePoint(u: Unit, x: number, y: number): void {
  faceVector(u, x - u.x, y - u.y);
}

/**
 * Move by (dx, dy) px with axis-separated sliding against solid cells.
 * The body is a (2*BODY_HALF)^2 box on the feet. Other units never block this;
 * they shape paths through occupancy instead, so nobody wedges in a corridor.
 * Returns true when the unit actually moved.
 */
export function moveUnit(w: World, u: Unit, dx: number, dy: number): boolean {
  const grid = w.rt.grid;
  const ox = u.x;
  const oy = u.y;
  const oldCx = cellOf(ox);
  const oldCy = cellOf(oy);
  if (dx !== 0) {
    const nx = u.x + dx;
    if (!boxBlocked(grid, nx, u.y)) u.x = nx;
    else u.x = slideTo(u.x, dx);
  }
  if (dy !== 0) {
    const ny = u.y + dy;
    if (!boxBlocked(grid, u.x, ny)) u.y = ny;
    else u.y = slideTo(u.y, dy);
  }
  if (boxBlocked(grid, u.x, u.y)) {
    u.x = ox;
    u.y = oy;
  }
  if (cellOf(u.x) !== oldCx || cellOf(u.y) !== oldCy) {
    const nx = u.x;
    const ny = u.y;
    u.x = ox;
    u.y = oy;
    vacate(w.rt, u);
    u.x = nx;
    u.y = ny;
    occupy(w.rt, u);
  }
  return u.x !== ox || u.y !== oy;
}

function boxBlocked(grid: World["rt"]["grid"], x: number, y: number): boolean {
  const x0 = cellOf(x - BODY_HALF);
  const x1 = cellOf(x + BODY_HALF - 0.001);
  const y0 = cellOf(y - BODY_HALF);
  const y1 = cellOf(y + BODY_HALF - 0.001);
  for (let cy = y0; cy <= y1; cy++) {
    for (let cx = x0; cx <= x1; cx++) if (grid.solid(cx, cy)) return true;
  }
  return false;
}

/** Blocked on this axis: close the gap to the cell border so the unit sits flush. */
function slideTo(pos: number, delta: number): number {
  if (delta > 0) {
    const edge = (cellOf(pos + BODY_HALF - 0.001) + 1) * CELL - BODY_HALF;
    return Math.max(pos, Math.min(pos + delta, edge));
  }
  const edge = cellOf(pos - BODY_HALF) * CELL + BODY_HALF;
  return Math.min(pos, Math.max(pos + delta, edge));
}

/** Teleport with occupancy bookkeeping. Used by travel, respawn and the terminal. */
export function placeUnit(w: World, u: Unit, x: number, y: number): void {
  vacate(w.rt, u);
  u.x = x;
  u.y = y;
  u.path = null;
  if (u.alive && u.awake) occupy(w.rt, u);
}

export function spendEnergy(u: Unit, amount: number): void {
  u.energy = Math.max(0, u.energy - amount);
  if (u.energy === 0) u.energyLocked = true;
}

export function restoreEnergy(u: Unit, amount: number): void {
  u.energy = Math.min(ENERGY_MAX, u.energy + amount);
  if (u.energy >= ENERGY_MAX) u.energyLocked = false;
}

export function distance(ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  return Math.sqrt(dx * dx + dy * dy);
}
