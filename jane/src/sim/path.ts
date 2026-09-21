// A* on the 8 px grid. 8-way, no corner cutting, integer costs (10 / 14).
//
// WINDOWED: all scratch lives in a fixed 256 x 256 cell window centred on the
// start, so memory is constant (about 3 MB) whether the zone is a kitchen or a
// ten-minute county of seven million cells. The first version sized its scratch
// to the grid: 115 MB at that scale. Nothing the game asks for needs more: AI
// paths are bounded by leash range and the node budget long before the window edge.
//
// Allocation-free after construction. "Clearing" between searches is a generation
// bump, not a fill. The open set is a binary heap of local indices keyed by f.
//
// Every search has a node budget and a max cost. A unit that cannot find a route
// gets `null` and leashes, exactly like 2020's "path longer than max distance ->
// fail". A goal OUTSIDE the window is different: it cannot be proven unreachable,
// so the search returns the best partial path toward it, and the caller re-plans
// when it gets there. That is how something walks a long road.

import { BLOCK_MOVE, F_OCC, type Grid } from "@/sim/grid";

const STRAIGHT = 10;
const DIAGONAL = 14;
export const PATH_WINDOW = 256;

// dx, dy pairs: 4 straight then 4 diagonal. Order is fixed; it is part of determinism.
const DIRS = [1, 0, -1, 0, 0, 1, 0, -1, 1, 1, 1, -1, -1, 1, -1, -1];

export type PathStats = { searches: number; expanded: number; failed: number; partial: number };

/**
 * Cells a walker will not step on, beyond what is solid: light, for something that shuns it
 * (sim/light.ts). Asked at most once per cell per search.
 */
export type Shunned = { cellLit(cx: number, cy: number): boolean };

export class PathFinder {
  private readonly grid: Grid;
  private readonly ww: number;
  private readonly wh: number;
  private readonly g: Int32Array;
  private readonly from: Int32Array;
  private readonly stamp: Uint32Array; // generation when g/from were written
  private readonly closed: Uint32Array; // generation when the node was expanded
  private readonly heap: Int32Array;
  private readonly heapF: Int32Array;
  // Only something that shuns light pays for these: made on its first search.
  private shunStamp: Uint32Array | null = null;
  private shunVal: Uint8Array | null = null;
  private heapSize = 0;
  private generation = 0;
  // Window origin of the current search, in cells.
  private ox = 0;
  private oy = 0;
  readonly stats: PathStats = { searches: 0, expanded: 0, failed: 0, partial: 0 };

  constructor(grid: Grid) {
    this.grid = grid;
    this.ww = Math.min(PATH_WINDOW, grid.w);
    this.wh = Math.min(PATH_WINDOW, grid.h);
    const n = this.ww * this.wh;
    this.g = new Int32Array(n);
    this.from = new Int32Array(n);
    this.stamp = new Uint32Array(n);
    this.closed = new Uint32Array(n);
    // Lazy-deletion heap. A node can in theory be pushed once per improving neighbour (8),
    // in practice under 3 times on a grid; 4n is ample and a full heap drops, never throws.
    this.heap = new Int32Array(n * 4);
    this.heapF = new Int32Array(n * 4);
  }

  /** Bytes of scratch held. Constant per zone; the debug overlay prints it. */
  get scratchBytes(): number {
    return (this.g.length * 4 + this.heap.length * 2) * 4;
  }

  /**
   * Cell path (global cell indices) from (sx,sy) to (tx,ty), excluding the start.
   * `self` is the moving unit's id, so its own occupancy is ignored.
   * The goal cell is always enterable for the search (it is usually a unit's feet);
   * callers stop short using range checks.
   * Returns null when blocked, over `maxCost` (tenths of a cell), or over budget.
   * A goal outside the search window returns the best partial path toward it.
   * With `shun`, cells it names are never entered (the goal included), and a goal that cannot
   * be reached returns the path to the nearest cell that can: whatever shuns the light walks to
   * the edge of it and waits there. That path may be empty.
   */
  find(
    sx: number,
    sy: number,
    tx: number,
    ty: number,
    self: number,
    maxCost: number,
    budget = 6000,
    shun: Shunned | null = null,
  ): number[] | null {
    const grid = this.grid;
    const gw = grid.w;
    this.stats.searches++;
    if (!grid.inside(sx, sy) || !grid.inside(tx, ty)) return this.fail();
    if (sx === tx && sy === ty) return [];
    if ((grid.flags[ty * gw + tx] & BLOCK_MOVE) !== 0) return this.fail();

    const ww = this.ww;
    const wh = this.wh;
    const ox = (this.ox = Math.max(0, Math.min(gw - ww, sx - (ww >> 1))));
    const oy = (this.oy = Math.max(0, Math.min(grid.h - wh, sy - (wh >> 1))));
    const goalInside = tx >= ox && ty >= oy && tx < ox + ww && ty < oy + wh;
    const goal = goalInside ? (ty - oy) * ww + (tx - ox) : -1;
    const start = (sy - oy) * ww + (sx - ox);

    const gen = ++this.generation;
    const { g, from, stamp, closed } = this;
    if (shun && !this.shunStamp) {
      this.shunStamp = new Uint32Array(ww * wh);
      this.shunVal = new Uint8Array(ww * wh);
    }
    const shunStamp = this.shunStamp as Uint32Array;
    const shunVal = this.shunVal as Uint8Array;
    const flags = grid.flags;
    this.heapSize = 0;
    g[start] = 0;
    from[start] = -1;
    stamp[start] = gen;
    const h0 = heuristic(sx, sy, tx, ty);
    this.push(start, h0);
    let bestNode = start;
    let bestH = h0;

    let expanded = 0;
    while (this.heapSize > 0) {
      const node = this.pop();
      if (closed[node] === gen) continue;
      closed[node] = gen;
      if (node === goal) {
        this.stats.expanded += expanded;
        return this.unwind(goal, start);
      }
      if (++expanded > budget) break;
      const lx = node % ww;
      const ly = (node - lx) / ww;
      const nx = lx + ox;
      const ny = ly + oy;
      const base = g[node];
      if (!goalInside || shun) {
        const h = heuristic(nx, ny, tx, ty);
        if (h < bestH) {
          bestH = h;
          bestNode = node;
        }
      }
      for (let d = 0; d < 16; d += 2) {
        const dx = DIRS[d];
        const dy = DIRS[d + 1];
        const cx = nx + dx;
        const cy = ny + dy;
        if (cx < ox || cy < oy || cx >= ox + ww || cy >= oy + wh) continue;
        const next = (cy - oy) * ww + (cx - ox);
        if (closed[next] === gen) continue;
        const gi = cy * gw + cx;
        if (next !== goal) {
          const f = flags[gi];
          if ((f & BLOCK_MOVE) !== 0) continue;
          // The occupancy bit is in the same byte; the Map is only consulted when it is set.
          if ((f & F_OCC) !== 0 && grid.occupant(gi) !== self) continue;
        }
        if (shun) {
          if (shunStamp[next] !== gen) {
            shunStamp[next] = gen;
            shunVal[next] = shun.cellLit(cx, cy) ? 1 : 0;
          }
          if (shunVal[next] === 1) continue;
        }
        let step = STRAIGHT;
        if (dx !== 0 && dy !== 0) {
          // No corner cutting: both orthogonal neighbours must be open terrain.
          if ((flags[ny * gw + cx] & BLOCK_MOVE) !== 0) continue;
          if ((flags[cy * gw + nx] & BLOCK_MOVE) !== 0) continue;
          step = DIAGONAL;
        }
        const cost = base + step;
        if (cost > maxCost) continue;
        if (stamp[next] === gen && g[next] <= cost) continue;
        g[next] = cost;
        from[next] = node;
        stamp[next] = gen;
        this.push(next, cost + heuristic(cx, cy, tx, ty));
      }
    }
    this.stats.expanded += expanded;
    if ((!goalInside || shun) && bestNode !== start) {
      this.stats.partial++;
      return this.unwind(bestNode, start);
    }
    // Already as near as it can get without stepping into the light: stand here.
    if (shun) return [];
    return this.fail();
  }

  private fail(): null {
    this.stats.failed++;
    return null;
  }

  /** Local indices back to the start, converted to global cell indices. */
  private unwind(end: number, start: number): number[] {
    const out: number[] = [];
    const ww = this.ww;
    const gw = this.grid.w;
    for (let n = end; n !== start && n !== -1; n = this.from[n]) {
      const lx = n % ww;
      out.push(((n - lx) / ww + this.oy) * gw + lx + this.ox);
    }
    out.reverse();
    return out;
  }

  private push(node: number, f: number): void {
    const { heap, heapF } = this;
    let i = this.heapSize;
    if (i >= heap.length) return; // full: drop. Budget makes this unreachable in practice.
    this.heapSize++;
    while (i > 0) {
      const parent = (i - 1) >> 1;
      // Tie-break on node index so equal-f ordering never depends on push history.
      if (heapF[parent] < f || (heapF[parent] === f && heap[parent] <= node)) break;
      heap[i] = heap[parent];
      heapF[i] = heapF[parent];
      i = parent;
    }
    heap[i] = node;
    heapF[i] = f;
  }

  private pop(): number {
    const { heap, heapF } = this;
    const top = heap[0];
    const size = --this.heapSize;
    if (size > 0) {
      const node = heap[size];
      const f = heapF[size];
      let i = 0;
      for (;;) {
        let child = 2 * i + 1;
        if (child >= size) break;
        const right = child + 1;
        if (
          right < size &&
          (heapF[right] < heapF[child] || (heapF[right] === heapF[child] && heap[right] < heap[child]))
        ) {
          child = right;
        }
        if (heapF[child] > f || (heapF[child] === f && heap[child] >= node)) break;
        heap[i] = heap[child];
        heapF[i] = heapF[child];
        i = child;
      }
      heap[i] = node;
      heapF[i] = f;
    }
    return top;
  }
}

/** Octile distance, same integer scale as step costs. Admissible and consistent. */
function heuristic(x: number, y: number, tx: number, ty: number): number {
  const dx = Math.abs(x - tx);
  const dy = Math.abs(y - ty);
  return dx > dy ? STRAIGHT * dx + (DIAGONAL - STRAIGHT) * dy : STRAIGHT * dy + (DIAGONAL - STRAIGHT) * dx;
}

/** Cost units are tenths of a cell; this converts a distance in cells. */
export function costOfCells(cells: number): number {
  return Math.round(cells * STRAIGHT);
}
