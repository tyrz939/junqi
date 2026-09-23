// Where the lamps go (DUNGEONS.md 2.9). A lamp hangs on a wall, in the wall cell itself, so it
// never stands in anyone's way and never claims a floor cell a crate might be pushed through.
// Nothing here is rolled: the same rooms and corridors are lit the same way on every seed.
//
//   rooms      a pair flanking every door the room uses, one clear cell from the opening; then
//              along each wall at the dungeon's rhythm, spaced evenly between those pairs and the
//              corners. North and south walls at `every`, east and west at half the count.
//   corridors  one every `corridor` cells, on the wall a lamp would be seen on (the north face of
//              a corridor running east-west, alternate sides of one running north-south).
//   dark       a node marked `dark`, and every corridor that leads to a boss or a mini-boss. The
//              walk to him is the one walk nobody lit.

import { F_SOLID, TILE_FLAGS, type Tile } from "@/sim/grid";
import type { Rect } from "@/world/blueprint";
import type { Kit } from "@/world/kit";
import type { DungeonDef, MissionNode, Side } from "@/world/dungeon/types";

export type LitRoom = { node: MissionNode; x: number; y: number; w: number; h: number };
export type LitCorridor = { rects: Rect[]; dark: boolean };
export type Lamp = { key: string; def: string; cx: number; cy: number };

/** Into the room from a wall on that side. */
const IN: Record<Side, [number, number]> = { n: [0, 1], s: [0, -1], w: [1, 0], e: [-1, 0] };
/** Along that wall. */
const ALONG: Record<Side, [number, number]> = { n: [1, 0], s: [1, 0], w: [0, 1], e: [0, 1] };

/**
 * Indices along a run of wall of length `len` where lamps hang. `doorAt0` / `doorAtEnd`: the run
 * stops at a doorway there (else at a corner). A doorway gets a lamp one cell clear of it; the
 * rest are spread evenly, and a corner counts as half a gap, so lamps sit symmetrically.
 */
export function lampsAlong(len: number, every: number, doorAt0: boolean, doorAtEnd: boolean): number[] {
  if (len <= 0) return [];
  const half = Math.floor(every / 2);
  const out = new Set<number>();
  const a = doorAt0 ? Math.min(1, len - 1) : -half;
  const b = doorAtEnd ? Math.max(0, len - 2) : len - 1 + half;
  if (doorAt0) out.add(a);
  if (doorAtEnd) out.add(b);
  const gap = b - a;
  const m = Math.max(1, Math.round(gap / every));
  for (let i = 1; i < m; i++) {
    const p = a + Math.round((i * gap) / m);
    if (p >= 0 && p < len) out.add(p);
  }
  return [...out].sort((p, q) => p - q);
}

/**
 * The wall a lamp standing at (x, y) should hang on instead: north first, the face she sees,
 * then west, east, south. The lamp goes in that wall cell. Null if no wall touches the cell.
 */
export function mountOn(k: Kit, wall: Tile, x: number, y: number): { side: Side; x: number; y: number } | null {
  for (const side of ["n", "w", "e", "s"] as Side[]) {
    const [ix, iy] = IN[side];
    if (k.get(x - ix, y - iy) === wall) return { side, x: x - ix, y: y - iy };
  }
  return null;
}

export function placeLamps(def: DungeonDef, k: Kit, wall: Tile, rooms: LitRoom[], corridors: LitCorridor[], reserved = new Set<number>()): Lamp[] {
  const plan = def.lights;
  if (!plan) return [];
  const zone = def.id;
  const lamps: Lamp[] = [];
  const taken = new Set<number>(reserved);
  const open = (x: number, y: number): boolean => (TILE_FLAGS[k.get(x, y)] & F_SOLID) === 0;
  const isWall = (x: number, y: number): boolean => k.get(x, y) === wall;
  const add = (key: string, side: Side, x: number, y: number): void => {
    const at = y * k.w + x;
    if (taken.has(at)) return;
    taken.add(at);
    lamps.push({ key, def: `${plan.prop}_${side}`, cx: x, cy: y });
  };

  // --- rooms -------------------------------------------------------------------------------
  const inBox = (r: LitRoom, x: number, y: number): boolean => x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h;
  const inner = (r: LitRoom, x: number, y: number): boolean => x > r.x && y > r.y && x < r.x + r.w - 1 && y < r.y + r.h - 1;
  for (const r of rooms) {
    if (r.node.dark) continue;
    let n = 0;
    for (const side of ["n", "s", "w", "e"] as Side[]) {
      const [ix, iy] = IN[side];
      const [ax, ay] = ALONG[side];
      const every = side === "n" || side === "s" ? plan.every : plan.every * 2;
      /** A cell of this room's wall that shows its face to the room's floor on this side. */
      // The floor it faces is inside the rim: the cells of a doorway are not a room to light.
      const face = (x: number, y: number): boolean => inBox(r, x, y) && isWall(x, y) && inner(r, x + ix, y + iy) && open(x + ix, y + iy);
      const seen = new Set<number>();
      for (let y = r.y; y < r.y + r.h; y++) {
        for (let x = r.x; x < r.x + r.w; x++) {
          if (seen.has(y * k.w + x) || !face(x, y) || face(x - ax, y - ay)) continue;
          // A run of face from here, along the wall.
          let len = 0;
          while (face(x + ax * len, y + ay * len)) {
            seen.add((y + ay * len) * k.w + x + ax * len);
            len++;
          }
          // The run ends in an opening (a doorway, or the room going on round a pillar) or a corner.
          const doorAt0 = inBox(r, x - ax, y - ay) && open(x - ax, y - ay);
          const doorAtEnd = inBox(r, x + ax * len, y + ay * len) && open(x + ax * len, y + ay * len);
          // Only the room's own rim flanks doors; a wall standing inside the room is just wall.
          const rim = (x - r.x === 0 || y - r.y === 0 || x - r.x === r.w - 1 || y - r.y === r.h - 1);
          for (const i of lampsAlong(len, every, doorAt0 && rim, doorAtEnd && rim)) add(`${zone}_${r.node.id}_lamp_${n++}`, side, x + ax * i, y + ay * i);
        }
      }
    }
  }

  // --- corridors ---------------------------------------------------------------------------
  if (plan.corridor > 0) {
    const mask = new Int8Array(k.w * k.h); // 1 lit corridor, -1 dark corridor
    for (const c of corridors) {
      for (const q of c.rects) {
        for (let y = q.cy; y < q.cy + q.h; y++) {
          for (let x = q.cx; x < q.cx + q.w; x++) {
            if (x < 0 || y < 0 || x >= k.w || y >= k.h) continue;
            const at = y * k.w + x;
            // Where a lit corridor shares the lane, the lane is lit: only the last stretch is his.
            if (!c.dark) mask[at] = 1;
            else if (mask[at] === 0) mask[at] = -1;
          }
        }
      }
    }
    const inRoom = (x: number, y: number): boolean => rooms.some((r) => inBox(r, x, y));
    const lit = (x: number, y: number): boolean => x >= 0 && y >= 0 && x < k.w && y < k.h && mask[y * k.w + x] === 1 && open(x, y) && !inRoom(x, y);
    const every = plan.corridor;
    const phase = (v: number, shift: number): boolean => (v + shift) % every === 0;
    let n = 0;
    for (let y = 0; y < k.h; y++) {
      for (let x = 0; x < k.w; x++) {
        if (!isWall(x, y) || inRoom(x, y)) continue;
        // East-west: the face above the corridor, the one a lamp is seen on.
        if (lit(x, y + 1) && lit(x, y + 2) && lit(x, y + 3) && phase(x, 0)) add(`${zone}_lamp_${n++}`, "n", x, y);
        // North-south: the two walls in turn, so the light steps down the passage.
        else if (lit(x + 1, y) && lit(x + 2, y) && lit(x + 3, y) && phase(y, 0)) add(`${zone}_lamp_${n++}`, "w", x, y);
        else if (lit(x - 1, y) && lit(x - 2, y) && lit(x - 3, y) && phase(y, every >> 1)) add(`${zone}_lamp_${n++}`, "e", x, y);
      }
    }
  }
  return lamps;
}
