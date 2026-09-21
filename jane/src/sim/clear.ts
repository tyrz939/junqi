// Nothing solid ever lands on a unit (DUNGEONS.md E12). Three verbs can make a cell solid
// under someone's feet: `show` (an exhibit back on its plinth), `lock` (a gate dropping) and
// a `fill` with a solid tile (a hedge growing). A unit inside a solid cannot move at all: the
// mover refuses any step that ends with its box in a wall. So:
//
//   show, lock   the thing appears, and whoever it landed on is moved to the nearest free cell
//                she could have WALKED to. A search outward over open floor, never a ring
//                through walls, so nobody is nudged into the next room or out of the map.
//   fill         a solid fill waits until nobody is touching its rect, then lands whole. The
//                zone remembers what it is owed (`pendingFill`, saved) and looks again every
//                tick. Whole, not cell by cell: a hedge that grew round the one cell she stood
//                on would leave her standing in a box.
//
// "On a unit" means its body box touches the cell, not only that its feet are in it: a body
// is 6 px wide on an 8 px cell and may hang over the edge of the next one.

import { BODY_HALF, CELL } from "@/sim/constants";
import { cellOf, centre, F_SOLID, TILE_FLAGS } from "@/sim/grid";
import { flushPropFlags, type World } from "@/sim/runtime";
import type { Prop, Unit } from "@/sim/state";
import { placeUnit } from "@/sim/units";

/** Most cells the search for somewhere to stand will look at. A room's worth. */
const NUDGE_CELLS = 1500;

function boxTouches(u: Unit, cx: number, cy: number, cw: number, ch: number): boolean {
  return u.x + BODY_HALF > cx * CELL && u.x - BODY_HALF < (cx + cw) * CELL && u.y + BODY_HALF > cy * CELL && u.y - BODY_HALF < (cy + ch) * CELL;
}

/**
 * Move a unit to the nearest cell it can stand on, reached over open floor from where it is.
 * The rect is what has just become solid around it: the search may pass through that (it is
 * standing in it) and may not end in it. False when there is nowhere, and it stays put.
 */
export function nudgeOut(w: World, u: Unit, cx0: number, cy0: number, cw: number, ch: number): boolean {
  const grid = w.rt.grid;
  const inRect = (x: number, y: number): boolean => x >= cx0 && y >= cy0 && x < cx0 + cw && y < cy0 + ch;
  const sx = cellOf(u.x);
  const sy = cellOf(u.y);
  const seen = new Set<number>([grid.index(sx, sy)]);
  const queue: number[] = [sx, sy];
  for (let q = 0; q < queue.length && seen.size < NUDGE_CELLS; q += 2) {
    const x = queue[q];
    const y = queue[q + 1];
    if (!inRect(x, y) && grid.free(x, y, u.id)) {
      placeUnit(w, u, centre(x), centre(y));
      u.hold = 0;
      return true;
    }
    // Fixed order: east, west, south, north. It is part of determinism.
    for (let d = 0; d < 4; d++) {
      const nx = x + (d === 0 ? 1 : d === 1 ? -1 : 0);
      const ny = y + (d === 2 ? 1 : d === 3 ? -1 : 0);
      if (!grid.inside(nx, ny)) continue;
      const i = grid.index(nx, ny);
      if (seen.has(i) || (grid.solid(nx, ny) && !inRect(nx, ny))) continue;
      seen.add(i);
      queue.push(nx, ny);
    }
  }
  return false;
}

/** A prop has just become solid and visible: nobody may be left standing in it. */
export function clearFootprint(w: World, p: Prop): void {
  if (!p.solid || p.hidden) return;
  const def = w.catalog.props[p.def];
  let stamped = false;
  for (const u of w.zone.units) {
    if (!u.alive || u.hidden || !boxTouches(u, p.cx, p.cy, def.w, def.h)) continue;
    // The grid must show the prop before anyone looks for a free cell, or its own cells look free.
    if (!stamped) flushPropFlags(w.catalog, w.rt, w.zone);
    stamped = true;
    nudgeOut(w, u, p.cx, p.cy, def.w, def.h);
  }
}

/** `pendingFill` is flat: cx, cy, w, h, tile, and again. */
const PENDING = 5;

/** Does any living body touch this rect of cells? */
function rectHeld(w: World, cx: number, cy: number, cw: number, ch: number): boolean {
  for (const u of w.zone.units) if (u.alive && !u.hidden && boxTouches(u, cx, cy, cw, ch)) return true;
  return false;
}

function layTiles(w: World, cx0: number, cy0: number, cw: number, ch: number, tile: number): void {
  const grid = w.rt.grid;
  for (let y = cy0; y < cy0 + ch; y++) {
    for (let x = cx0; x < cx0 + cw; x++) {
      if (!grid.inside(x, y) || grid.tileAt(x, y) === tile) continue;
      grid.setTile(x, y, tile);
      w.zone.tileDeltas.push(grid.index(x, y), tile);
    }
  }
  w.emit({ e: "tiles", cx: cx0, cy: cy0, w: cw, h: ch });
}

/**
 * `fill`. A tile feet can cross goes down at once. A solid one goes down at once if nobody is
 * touching the rect; otherwise it is owed, and `stepPendingFill` pays it when the rect is clear.
 */
export function fillRect(w: World, cx0: number, cy0: number, cw: number, ch: number, tile: number): void {
  // A newer fill of the same ground wins: what an older one was still owed there is forgotten.
  const pending = w.zone.pendingFill;
  for (let i = pending.length - PENDING; i >= 0; i -= PENDING) {
    if (pending[i] === cx0 && pending[i + 1] === cy0 && pending[i + 2] === cw && pending[i + 3] === ch) pending.splice(i, PENDING);
  }
  if ((TILE_FLAGS[tile] & F_SOLID) !== 0 && rectHeld(w, cx0, cy0, cw, ch)) {
    pending.push(cx0, cy0, cw, ch, tile);
    return;
  }
  layTiles(w, cx0, cy0, cw, ch, tile);
}

/** Housekeeping, every tick: the fills the zone is still owed, tried again in the order they were asked for. Nearly always nothing. */
export function stepPendingFill(w: World): void {
  const pending = w.zone.pendingFill;
  if (pending.length === 0) return;
  for (let i = 0; i + PENDING <= pending.length; ) {
    if (rectHeld(w, pending[i], pending[i + 1], pending[i + 2], pending[i + 3])) {
      i += PENDING;
      continue;
    }
    const [cx, cy, cw, ch, tile] = pending.splice(i, PENDING);
    layTiles(w, cx, cy, cw, ch, tile);
  }
}
