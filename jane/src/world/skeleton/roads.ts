// Layer 2: roads. Routed site to site over a cost field, so every seed's network
// is different and always joins the story up. Later roads prefer cells an earlier
// road already used, which is what makes it a network (and one bridge) rather
// than a bundle of separate lines.

import { at, inside, MACRO, ROAD, ROAD_BRIDGE, SKEL_H, SKEL_W, type Road } from "@/world/skeleton/types";

const DX = [1, 0, -1, 0, 1, 1, -1, -1];
const DY = [0, 1, 0, -1, 1, -1, 1, -1];
const LEN = [1, 1, 1, 1, Math.SQRT2, Math.SQRT2, Math.SQRT2, Math.SQRT2];

/** A binary heap of (cost, cell). Ties break on cell index, so the route never depends on insertion order. */
export class Heap {
  // Typed and preallocated: this is the inner loop of every road, and the solver lays a few dozen per county.
  private cost = new Float64Array(4096);
  private cell = new Int32Array(4096);
  size = 0;
  /** The entry `pop` just removed. Fields, not a returned object, so the loop allocates nothing. */
  topCost = 0;
  topCell = 0;
  private less(a: number, b: number): boolean {
    return this.cost[a] < this.cost[b] || (this.cost[a] === this.cost[b] && this.cell[a] < this.cell[b]);
  }
  private swap(a: number, b: number): void {
    const c = this.cost[a];
    this.cost[a] = this.cost[b];
    this.cost[b] = c;
    const k = this.cell[a];
    this.cell[a] = this.cell[b];
    this.cell[b] = k;
  }
  push(cost: number, cell: number): void {
    if (this.size === this.cost.length) {
      const c = new Float64Array(this.size * 2);
      c.set(this.cost);
      this.cost = c;
      const k = new Int32Array(this.size * 2);
      k.set(this.cell);
      this.cell = k;
    }
    let i = this.size++;
    this.cost[i] = cost;
    this.cell[i] = cell;
    while (i > 0) {
      const p = (i - 1) >> 1;
      if (!this.less(i, p)) break;
      this.swap(i, p);
      i = p;
    }
  }
  pop(): void {
    this.topCost = this.cost[0];
    this.topCell = this.cell[0];
    const last = --this.size;
    if (last === 0) return;
    this.cost[0] = this.cost[last];
    this.cell[0] = this.cell[last];
    let i = 0;
    for (;;) {
      const l = i * 2 + 1;
      const r = l + 1;
      let m = i;
      if (l < last && this.less(l, m)) m = l;
      if (r < last && this.less(r, m)) m = r;
      if (m === i) break;
      this.swap(i, m);
      i = m;
    }
  }
}

export type Land = { height: Uint8Array; water: Uint8Array; rough: Float32Array; road: Uint8Array };

/** 1 = river (bridgeable, dearly), 2 = lake (never). */
function stepCost(land: Land, from: number, to: number, len: number): number {
  const w = land.water[to];
  if (w === 2) return Infinity;
  if (land.road[to] & ROAD) return len * 0.3;
  if (w === 1) return len * 30;
  const slope = Math.abs(land.height[to] - land.height[from]);
  return len * (land.rough[to] + slope * 0.22);
}

/** How far outside the box of its two ends a road may wander, in macro cells (about 550 m). */
const CORRIDOR = 34;

/**
 * Cheapest way from one cell to another over the cost field. Searched inside a corridor
 * around the two ends first, because most roads are short and the map is not; if the
 * corridor holds no way through (the lake is in it), the whole map is searched.
 * Null if there is no way at all.
 */
export function route(land: Land, ax: number, ay: number, bx: number, by: number): number[] | null {
  return search(land, ax, ay, bx, by, CORRIDOR) ?? search(land, ax, ay, bx, by, SKEL_W);
}

function search(land: Land, ax: number, ay: number, bx: number, by: number, margin: number): number[] | null {
  const x0 = Math.min(ax, bx) - margin;
  const x1 = Math.max(ax, bx) + margin;
  const y0 = Math.min(ay, by) - margin;
  const y1 = Math.max(ay, by) + margin;
  const n = SKEL_W * SKEL_H;
  const dist = new Float64Array(n).fill(Infinity);
  const prev = new Int32Array(n).fill(-1);
  const heap = new Heap();
  const start = at(ax, ay);
  const goal = at(bx, by);
  // A*: no step is cheaper than an existing road (0.3 a cell), so 0.3 x the octile distance never overestimates.
  const h = (x: number, y: number): number => {
    const dx = Math.abs(x - bx);
    const dy = Math.abs(y - by);
    return 0.3 * (Math.max(dx, dy) + (Math.SQRT2 - 1) * Math.min(dx, dy));
  };
  const closed = new Uint8Array(n);
  dist[start] = 0;
  heap.push(h(ax, ay), start);
  while (heap.size > 0) {
    heap.pop();
    const cell = heap.topCell;
    if (cell === goal) break;
    if (closed[cell]) continue;
    closed[cell] = 1;
    const cost = dist[cell];
    const cx = cell % SKEL_W;
    const cy = (cell - cx) / SKEL_W;
    for (let d = 0; d < 8; d++) {
      const nx = cx + DX[d];
      const ny = cy + DY[d];
      // Roads keep one cell off the map's edge, except where a site sits on it (the station).
      if (!inside(nx, ny) || nx < x0 || nx > x1 || ny < y0 || ny > y1) continue;
      const to = at(nx, ny);
      if (closed[to]) continue;
      if ((nx === 0 || ny === 0 || nx === SKEL_W - 1 || ny === SKEL_H - 1) && to !== goal && to !== start) continue;
      const c = cost + stepCost(land, cell, to, LEN[d]);
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

export function layRoad(land: Land, from: string, to: string, cells: number[]): Road {
  let len = 0;
  for (let i = 0; i < cells.length; i++) {
    const c = cells[i];
    land.road[c] |= ROAD;
    if (land.water[c] === 1) land.road[c] |= ROAD_BRIDGE;
    if (i > 0) {
      const dx = Math.abs((cells[i] % SKEL_W) - (cells[i - 1] % SKEL_W));
      const dy = Math.abs(Math.floor(cells[i] / SKEL_W) - Math.floor(cells[i - 1] / SKEL_W));
      len += dx + dy === 2 ? Math.SQRT2 : 1;
    }
  }
  return { from, to, cells, metres: Math.round(len * MACRO) };
}

/** Walking distance along the road network from one cell to every road cell, in metres. */
export function roadDistances(road: Uint8Array, fromX: number, fromY: number): Float64Array {
  const n = SKEL_W * SKEL_H;
  const dist = new Float64Array(n).fill(Infinity);
  const heap = new Heap();
  const start = at(fromX, fromY);
  dist[start] = 0;
  heap.push(0, start);
  while (heap.size > 0) {
    heap.pop();
    const cost = heap.topCost;
    const cell = heap.topCell;
    if (cost > dist[cell]) continue;
    const cx = cell % SKEL_W;
    const cy = (cell - cx) / SKEL_W;
    for (let d = 0; d < 8; d++) {
      const nx = cx + DX[d];
      const ny = cy + DY[d];
      if (!inside(nx, ny)) continue;
      const to = at(nx, ny);
      if (!(road[to] & ROAD)) continue;
      const c = cost + LEN[d] * MACRO;
      if (c < dist[to]) {
        dist[to] = c;
        heap.push(c, to);
      }
    }
  }
  return dist;
}

/** Distance (macro cells, chamfer) from every cell to the nearest cell of `mask` with `bit` set. Roads by default. */
export function distanceToRoad(mask: Uint8Array, bit: number = ROAD): Float32Array {
  const n = SKEL_W * SKEL_H;
  const d = new Float32Array(n).fill(999);
  for (let i = 0; i < n; i++) if (mask[i] & bit) d[i] = 0;
  // Unrolled: this runs for every cell of the map several times a county, and must not allocate.
  const look = (x: number, y: number, c: number, best: number): number => (inside(x, y) && d[at(x, y)] + c < best ? d[at(x, y)] + c : best);
  const pass = (x: number, y: number, dir: 1 | -1): void => {
    let best = d[at(x, y)];
    best = look(x + dir, y, 1, best);
    best = look(x, y + dir, 1, best);
    best = look(x + dir, y + dir, 1.4, best);
    best = look(x - dir, y + dir, 1.4, best);
    d[at(x, y)] = best;
  };
  for (let y = 0; y < SKEL_H; y++) for (let x = 0; x < SKEL_W; x++) pass(x, y, -1);
  for (let y = SKEL_H - 1; y >= 0; y--) for (let x = SKEL_W - 1; x >= 0; x--) pass(x, y, 1);
  return d;
}
