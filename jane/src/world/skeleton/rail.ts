// The railway. One train a week stops at Castle Halt, and it came from somewhere and goes on
// somewhere: the line does not stop at the ends of the platform. It runs in from the south edge
// along the county's western fence, past the halt, north to the Works, and then east across the
// Works (past the yards the line was laid for, over the river on a trestle, across the roads on
// the level) and out of the county at the eastern fence.
//
// Decided here, on the macro grid, after the sites and roads and before anything small is placed,
// so the small places can keep off it and the roadside beat can use it. county.ts lays the cells.

import { rngFloat, rngSeed } from "@/sim/rng";
import { Heap } from "@/world/skeleton/roads";
import type { Terrain } from "@/world/skeleton/terrain";
import { at, inside, metres, Region, ROAD, SKEL_H, SKEL_W, type PlacedSite } from "@/world/skeleton/types";

/** Nothing the line passes comes nearer a set place than this (m): the biggest chunk is 60 x 44 cells. */
const KEEP_OFF_SITES = 96;
const DX = [1, 0, -1, 0, 1, 1, -1, -1];
const DY = [0, 1, 0, -1, 1, -1, 1, -1];
const LEN = [1, 1, 1, 1, Math.SQRT2, Math.SQRT2, Math.SQRT2, Math.SQRT2];

/**
 * The line, macro cells in order: the south edge, up column 0 through the halt, then from where
 * the Works begin, east to the east edge. A county where no way east can be found (it never
 * happens on the seeds tested, but the lake and the sites could in principle wall it off) keeps
 * the line along the west fence and out at the north edge instead: it still never stops dead.
 */
export function layRail(seed: number, attempt: number, t: Terrain, road: Uint8Array, sites: readonly PlacedSite[]): number[] {
  const station = sites.find((s) => s.id === "station");
  if (!station) return [];
  const rng = rngSeed(seed, 700 + attempt);
  const out: number[] = [];
  // In from the south edge, up the west fence to the halt.
  for (let y = SKEL_H - 1; y > station.my; y--) out.push(at(0, y));
  // Past the halt, north until the Works have begun and a little more: the line turns east in the yards, not the fields.
  let turn = station.my;
  while (turn > 4 && t.region[at(0, turn)] !== Region.Works) turn--;
  turn = Math.max(4, turn - 3);
  for (let y = station.my; y > turn; y--) out.push(at(0, y));

  // Where it leaves in the east: a row of the east edge still in the Works, well clear of the corners.
  const exits: number[] = [];
  for (let y = 6; y < SKEL_H - 6; y++) if (t.region[at(SKEL_W - 1, y)] === Region.Works && t.water[at(SKEL_W - 1, y)] !== 2) exits.push(y);
  const east = exits.length > 0 ? exits[Math.min(exits.length - 1, Math.floor(rngFloat(rng) * Math.max(1, exits.length - 3)))] : -1;
  const across = east < 0 ? null : (search(t, road, sites, turn, east, true) ?? search(t, road, sites, turn, east, false));
  if (across) {
    out.push(...across);
    return out;
  }
  for (let y = turn; y >= 0; y--) out.push(at(0, y));
  return out;
}

function search(t: Terrain, road: Uint8Array, sites: readonly PlacedSite[], fromY: number, toY: number, worksOnly: boolean): number[] | null {
  const n = SKEL_W * SKEL_H;
  const blocked = new Uint8Array(n);
  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const i = at(x, y);
      // The lake is never crossed; the map's edge rows are the fence, not the line.
      if (t.water[i] === 2 || y < 2 || y > SKEL_H - 3) blocked[i] = 1;
      else if (worksOnly && t.region[i] !== Region.Works && !t.water[i]) blocked[i] = 1;
    }
  }
  for (const s of sites) {
    if (s.id === "station") continue;
    const r = Math.ceil(KEEP_OFF_SITES / 16);
    for (let y = s.my - r; y <= s.my + r; y++) for (let x = s.mx - r; x <= s.mx + r; x++) if (inside(x, y) && metres(x, y, s.mx, s.my) <= KEEP_OFF_SITES) blocked[at(x, y)] = 1;
  }
  const start = at(0, fromY);
  const goal = at(SKEL_W - 1, toY);
  blocked[start] = 0;
  blocked[goal] = 0;
  const dist = new Float64Array(n).fill(Infinity);
  const prev = new Int32Array(n).fill(-1);
  const closed = new Uint8Array(n);
  const heap = new Heap();
  const bx = SKEL_W - 1;
  const h = (x: number, y: number): number => {
    const dx = Math.abs(x - bx);
    const dy = Math.abs(y - toY);
    return Math.max(dx, dy) + (Math.SQRT2 - 1) * Math.min(dx, dy);
  };
  dist[start] = 0;
  heap.push(h(0, fromY), start);
  while (heap.size > 0) {
    heap.pop();
    const cell = heap.topCell;
    if (cell === goal) break;
    if (closed[cell]) continue;
    closed[cell] = 1;
    const cx = cell % SKEL_W;
    const cy = (cell - cx) / SKEL_W;
    for (let d = 0; d < 8; d++) {
      const nx = cx + DX[d];
      const ny = cy + DY[d];
      if (!inside(nx, ny)) continue;
      const to = at(nx, ny);
      if (blocked[to] || closed[to]) continue;
      // Only the start and the exit may stand on the map's side columns.
      if ((nx === 0 || nx === SKEL_W - 1) && to !== goal) continue;
      // A railway hates a gradient, crosses water on a trestle it would rather not build, and meets
      // a road square on and once: a road cell is dear, so it never runs along one.
      const slope = Math.abs(t.height[to] - t.height[cell]);
      let step = LEN[d] * (1 + slope * 0.6);
      if (t.water[to] === 1) step += 6;
      if (road[to] & ROAD) step += 5;
      // Out of the Works the line may go, if it must, but dearly: it is the Works' railway.
      if (!worksOnly && t.region[to] !== Region.Works) step += 4;
      const c = dist[cell] + step;
      if (c < dist[to]) {
        dist[to] = c;
        prev[to] = cell;
        heap.push(c + h(nx, ny), to);
      }
    }
  }
  if (dist[goal] === Infinity) return null;
  const cells: number[] = [];
  for (let c = goal; c !== -1; c = prev[c]) cells.push(c);
  return cells.reverse();
}

/** Every macro cell the line is on, and (with `margin`) the cells beside it. */
export function railMask(rail: readonly number[], margin = 0): Uint8Array {
  const mask = new Uint8Array(SKEL_W * SKEL_H);
  for (const c of rail) {
    const x = c % SKEL_W;
    const y = Math.floor(c / SKEL_W);
    for (let oy = -margin; oy <= margin; oy++) for (let ox = -margin; ox <= margin; ox++) if (inside(x + ox, y + oy)) mask[at(x + ox, y + oy)] = 1;
  }
  return mask;
}
