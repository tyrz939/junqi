// Builder kit: a tile canvas plus spawn lists. Zone builders paint with this and
// return a Blueprint. Motifs (fence ring, torch run, barrel pile, house stamp)
// are painters here, not rooms: WORLDGEN.md's "draw a fence ring around a lot,
// do not place 400 fences one by one".

import { Tile, TILE_FLAGS, F_SOLID } from "@/sim/grid";
import { hashString, irandom, rngChance, rngFloat, rngPick, rngRange, rngSeed, stepDice, type RngState } from "@/sim/rng";
import type { ZoneId } from "@/sim/state";
import type { Blueprint, Mark, PropSpawn, Rect, UnitSpawn } from "@/world/blueprint";

/** A 32-bit finaliser (murmur3's): every input bit moves every output bit. */
function mix32(h: number): number {
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
  return (h ^ (h >>> 16)) >>> 0;
}

export class Kit {
  readonly zone: ZoneId;
  readonly w: number;
  readonly h: number;
  readonly tiles: Uint8Array;
  /** The dice in hand. The zone's own stream until a step takes up its own (`within`). */
  rng: RngState;
  readonly units: UnitSpawn[] = [];
  readonly props: PropSpawn[] = [];
  readonly marks: Record<string, Mark> = {};
  readonly rects: Record<string, Rect> = {};
  /** Cells claimed by a prop footprint or a mark, so scatter never lands on them. */
  private readonly claimed: Uint8Array;
  private anon = 0;
  private readonly seed: number;
  private readonly attempt: number;
  /**
   * Name anonymous things by where they stand (`county_rock_812_40`), not by how many came before them.
   * With the steps on dice of their own (`within`), a count would still tie every step to the ones
   * before it: one more lamp by the rail and every sheep in the county has a new name.
   */
  private readonly keysByPlace: boolean;
  private readonly used = new Set<string>();
  private readonly bases = new Map<string, number>();

  constructor(zone: ZoneId, w: number, h: number, seed: number, attempt: number, fill: Tile, opts: { keysByPlace?: boolean } = {}) {
    this.zone = zone;
    this.w = w;
    this.h = h;
    this.tiles = new Uint8Array(w * h).fill(fill);
    this.claimed = new Uint8Array(w * h);
    this.seed = seed >>> 0;
    this.attempt = attempt;
    this.keysByPlace = opts.keysByPlace ?? false;
    // One stream per (seed, zone, attempt): adding a call in one zone never shifts another.
    this.rng = rngSeed(seed ^ hashString(zone), 100 + attempt);
  }

  /**
   * The dice of one named step of the build: a stream of its own from (seed, zone, step, attempt), in
   * hand while `fn` runs and the caller's put back afterwards. A step that throws more dice or fewer (a
   * rail with a new bend, one more placement row, a re-tuned scatter) moves nothing drawn under any
   * other name. Name the smallest thing that is decided on its own: a road, a lattice point, a macro cell.
   */
  within<T>(step: string, fn: () => T, a?: number, b?: number): T {
    const outer = this.rng;
    if (a === undefined) this.rng = stepDice(this.seed, `${this.zone}:${step}`, this.attempt);
    else {
      // A step taken once per cell, point or beat (`within("wild", fn, mx, my)`): a stream of its own for
      // each (step, a, b), from the step's name hashed once and the numbers mixed in, with no string per call.
      let base = this.bases.get(step);
      if (base === undefined) this.bases.set(step, (base = hashString(`${this.seed}:${this.zone}:${step}`)));
      let h = mix32(base ^ Math.imul(a + 1, 0x9e3779b1));
      if (b !== undefined) h = mix32(h ^ Math.imul(b + 1, 0x85ebca6b));
      this.rng = rngSeed(h, this.attempt);
    }
    try {
      return fn();
    } finally {
      this.rng = outer;
    }
  }

  private anonKey(def: string, cx: number, cy: number): string {
    if (!this.keysByPlace) return `${this.zone}_${def}_${this.anon++}`;
    const base = `${this.zone}_${def}_${cx}_${cy}`;
    let key = base;
    for (let n = 2; this.used.has(key); n++) key = `${base}_${n}`;
    this.used.add(key);
    return key;
  }

  // --- random helpers ---
  int(lo: number, hi: number): number {
    return rngRange(this.rng, lo, hi);
  }
  chance(p: number): boolean {
    return rngChance(this.rng, p);
  }
  float(): number {
    return rngFloat(this.rng);
  }
  pick<T>(list: readonly T[]): T {
    return rngPick(this.rng, list);
  }
  roll(n: number): number {
    return irandom(this.rng, n);
  }

  // --- tiles ---
  inside(x: number, y: number): boolean {
    return x >= 0 && y >= 0 && x < this.w && y < this.h;
  }
  get(x: number, y: number): Tile {
    return this.inside(x, y) ? this.tiles[y * this.w + x] : Tile.Void;
  }
  set(x: number, y: number, t: Tile): void {
    if (this.inside(x, y)) this.tiles[y * this.w + x] = t;
  }
  fill(x: number, y: number, w: number, h: number, t: Tile): void {
    for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) this.set(i, j, t);
  }
  outline(x: number, y: number, w: number, h: number, t: Tile): void {
    for (let i = x; i < x + w; i++) {
      this.set(i, y, t);
      this.set(i, y + h - 1, t);
    }
    for (let j = y; j < y + h; j++) {
      this.set(x, j, t);
      this.set(x + w - 1, j, t);
    }
  }
  solid(x: number, y: number): boolean {
    return (TILE_FLAGS[this.get(x, y)] & F_SOLID) !== 0;
  }
  /** A room: floor inside, wall ring outside. Rect is the floor. */
  room(r: Rect, floor: Tile, wall: Tile): Rect {
    this.fill(r.cx - 1, r.cy - 1, r.w + 2, r.h + 2, wall);
    this.fill(r.cx, r.cy, r.w, r.h, floor);
    return r;
  }
  /** Carve floor only where the tile is currently `from` (does not eat other rooms' floors). */
  carve(x: number, y: number, w: number, h: number, floor: Tile): void {
    this.fill(x, y, w, h, floor);
  }
  /** L-shaped corridor of `width` cells between two points, horizontal leg first. */
  corridor(x0: number, y0: number, x1: number, y1: number, width: number, floor: Tile): void {
    const half = Math.floor(width / 2);
    const xa = Math.min(x0, x1);
    const xb = Math.max(x0, x1);
    this.fill(xa - half, y0 - half, xb - xa + width, width, floor);
    const ya = Math.min(y0, y1);
    const yb = Math.max(y0, y1);
    this.fill(x1 - half, ya - half, width, yb - ya + width, floor);
  }
  /** Thick wiggly stroke, used for roads and rivers. Returns the centre-line points. */
  stroke(points: [number, number][], width: number, t: Tile, wobble: number): [number, number][] {
    const line: [number, number][] = [];
    const half = Math.floor(width / 2);
    for (let p = 0; p + 1 < points.length; p++) {
      const [ax, ay] = points[p];
      const [bx, by] = points[p + 1];
      const steps = Math.max(Math.abs(bx - ax), Math.abs(by - ay));
      let drift = 0;
      for (let s = 0; s <= steps; s++) {
        const k = steps === 0 ? 0 : s / steps;
        if (wobble > 0 && s % 6 === 0) drift = Math.max(-wobble, Math.min(wobble, drift + this.int(-1, 1)));
        const horizontal = Math.abs(bx - ax) >= Math.abs(by - ay);
        const x = Math.round(ax + (bx - ax) * k) + (horizontal ? 0 : drift);
        const y = Math.round(ay + (by - ay) * k) + (horizontal ? drift : 0);
        this.fill(x - half, y - half, width, width, t);
        line.push([x, y]);
      }
    }
    return line;
  }
  /** Scatter a tile over cells currently equal to `over`, with probability p, inside a rect. */
  sprinkle(r: Rect, over: Tile, t: Tile, p: number): void {
    for (let y = r.cy; y < r.cy + r.h; y++) {
      for (let x = r.cx; x < r.cx + r.w; x++) if (this.get(x, y) === over && this.chance(p)) this.set(x, y, t);
    }
  }
  /** Blobby patch: random walk of filled discs. */
  blob(cx: number, cy: number, size: number, over: Tile, t: Tile): void {
    let x = cx;
    let y = cy;
    for (let n = 0; n < size; n++) {
      const r = this.int(1, 3);
      for (let j = -r; j <= r; j++) {
        for (let i = -r; i <= r; i++) {
          if (i * i + j * j <= r * r && this.get(x + i, y + j) === over && !this.isClaimed(x + i, y + j)) this.set(x + i, y + j, t);
        }
      }
      x += this.int(-2, 2);
      y += this.int(-2, 2);
    }
  }

  // --- spawns ---
  mark(name: string, cx: number, cy: number, facing?: Mark["facing"]): void {
    this.marks[name] = { cx, cy, facing };
    this.claim(cx - 1, cy - 1, 3, 3);
  }
  rect(name: string, r: Rect): Rect {
    this.rects[name] = r;
    return r;
  }
  claim(x: number, y: number, w: number, h: number): void {
    for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (this.inside(i, j)) this.claimed[j * this.w + i] = 1;
  }
  isClaimed(x: number, y: number): boolean {
    return !this.inside(x, y) || this.claimed[y * this.w + x] === 1;
  }
  /** True when a w*h footprint at (x,y) sits on open, unclaimed floor. */
  fits(x: number, y: number, w: number, h: number, margin = 0): boolean {
    for (let j = y - margin; j < y + h + margin; j++) {
      for (let i = x - margin; i < x + w + margin; i++) if (this.solid(i, j) || this.isClaimed(i, j)) return false;
    }
    return true;
  }
  prop(spawn: Omit<PropSpawn, "key"> & { key?: string }, w: number, h: number): PropSpawn {
    const full: PropSpawn = { ...spawn, key: spawn.key ?? this.anonKey(spawn.def, spawn.cx, spawn.cy) };
    this.props.push(full);
    this.claim(full.cx, full.cy, w, h);
    return full;
  }
  unit(key: string | null, def: string, cx: number, cy: number, patrol?: UnitSpawn["patrol"]): UnitSpawn {
    const u: UnitSpawn = { key: key ?? this.anonKey(def, cx, cy), def, cx, cy, patrol };
    this.units.push(u);
    this.claim(cx, cy, 1, 1);
    return u;
  }
  /** Random open spot for a w*h footprint inside a rect, or null. Deterministic attempts. */
  spot(r: Rect, w: number, h: number, margin = 1, tries = 60): { cx: number; cy: number } | null {
    for (let n = 0; n < tries; n++) {
      const x = this.int(r.cx, Math.max(r.cx, r.cx + r.w - w));
      const y = this.int(r.cy, Math.max(r.cy, r.cy + r.h - h));
      if (this.fits(x, y, w, h, margin)) return { cx: x, cy: y };
    }
    return null;
  }

  // --- motifs ---
  /** Fence ring around a lot with one gap. */
  fenceRing(r: Rect, gapSide: "n" | "s" | "e" | "w", gap = 4): void {
    this.outline(r.cx, r.cy, r.w, r.h, Tile.Fence);
    const mx = r.cx + Math.floor(r.w / 2) - Math.floor(gap / 2);
    const my = r.cy + Math.floor(r.h / 2) - Math.floor(gap / 2);
    if (gapSide === "n") this.fill(mx, r.cy, gap, 1, Tile.Dirt);
    if (gapSide === "s") this.fill(mx, r.cy + r.h - 1, gap, 1, Tile.Dirt);
    if (gapSide === "w") this.fill(r.cx, my, 1, gap, Tile.Dirt);
    if (gapSide === "e") this.fill(r.cx + r.w - 1, my, 1, gap, Tile.Dirt);
  }
  /** House stamp: roof block over a wall strip. Size table from 2020: 7x9, 11x6, 10x12 tiles of 16 px. */
  house(cx: number, cy: number, w: number, h: number): void {
    this.fill(cx, cy, w, h - 3, Tile.HouseRoof);
    this.fill(cx, cy + h - 3, w, 3, Tile.HouseWall);
    this.claim(cx - 1, cy - 1, w + 2, h + 3);
  }
  /** Props at even spacing along the inside of a room's walls. */
  torchRun(r: Rect, every: number, def = "torch"): void {
    for (let x = r.cx + 2; x < r.cx + r.w - 1; x += every) {
      if (!this.isClaimed(x, r.cy) && !this.solid(x, r.cy)) this.prop({ def, cx: x, cy: r.cy }, 1, 1);
    }
    for (let x = r.cx + 2 + Math.floor(every / 2); x < r.cx + r.w - 1; x += every) {
      const y = r.cy + r.h - 1;
      if (!this.isClaimed(x, y) && !this.solid(x, y)) this.prop({ def, cx: x, cy: y }, 1, 1);
    }
  }
  /** A few pushables piled into a corner. */
  pile(r: Rect, def: string, count: number): void {
    for (let n = 0; n < count; n++) {
      const s = this.spot(r, 2, 2, 0);
      if (s) this.prop({ def, cx: s.cx, cy: s.cy }, 2, 2);
    }
  }

  done(name: string, indoor: boolean, ambient: number, attempt: number): Blueprint {
    return {
      zone: this.zone,
      name,
      w: this.w,
      h: this.h,
      tiles: this.tiles,
      units: this.units,
      props: this.props,
      marks: this.marks,
      rects: this.rects,
      indoor,
      ambient,
      attempts: attempt + 1,
    };
  }
}
