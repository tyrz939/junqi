// The open country: everything between the set places, furnished. The skeleton decided where
// the story stands and where the roads go; county.ts drew the ground, the roads and the chunks;
// this fills the rest of the county the way somewhere lived in and then left is filled.
//
//   the roads     lamps at an even step on ONE side of each road, set just off its verge; a
//                 lamp at each end of a bridge; a fingerpost at every fork that names where each
//                 way goes and how far; milestones; a fence or a hedge in straight runs along
//                 the field side, with gates where the paths cross
//   the places    a jittered lattice over the whole county, one candidate every ~44 m. Each
//                 becomes the kind of place its ground allows: by the road a hamlet, a farm, a
//                 cottage, an inn, a field, an orchard, a well, a shrine, a wreck; off the road
//                 a camp of something hostile round its fire, a den, a ruin, a pond, standing
//                 stones, an outcrop, a woodcutters' clearing, a flock. Zelda-tight: a cottage is
//                 8 x 6 cells, a hamlet about 34 x 28.
//   the living    people in the hamlets and farms (by day), animals about them, and the hostile
//                 ground: wildlife in every macro cell's chance, thicker with the threat, kept
//                 back from the roads so that a road is the safe way and a field is not
//   the gaps      last, any screen of the county still empty gets something small of its own
//
// Same seed, same county: every choice comes from the kit's stream, in a fixed order.

import { Tile } from "@/sim/grid";
import type { UnitSpawn } from "@/world/blueprint";
import type { Chunk } from "@/world/chunks";
import type { Kit } from "@/world/kit";
import { at, Biome, MACRO, Region, ROAD, SKEL_H, SKEL_W, type Skeleton } from "@/world/skeleton";
import { roadDistances } from "@/world/skeleton/roads";

type Pt = readonly [number, number];

export type Country = {
  k: Kit;
  sk: Skeleton;
  chunks: readonly Chunk[];
  /** Stroked centre lines. The first `sk.roads.length` are the skeleton's roads, in order; the rest are footpaths. */
  lines: readonly (readonly Pt[])[];
  lit: readonly (readonly boolean[])[];
  /** The ground before any road was drawn: where a road crosses water, it is a bridge. */
  before: Uint8Array;
  sizes: Readonly<Record<string, { w: number; h: number }>>;
};

// --- numbers the county is tuned by -------------------------------------------------------

/** Lamps along a lit road, cells apart. */
const LAMP_STEP = 22;
/** From the road's centre line to a lamp: past the metal (1) and the verge (2). */
const LAMP_OFF = 3;
/** Milestones, cells apart along a road. */
const MILE_STEP = 230;
/** The lattice places are thrown on. */
const LATTICE = 44;
/** Nothing that bites stands nearer a road than this (cells): its eye is 16 m. */
const ROAD_CLEAR = 22;
/** Camps stand back from the road by this much, so a careful walker passes them. */
const CAMP_BACK = 26;
/** Nothing that bites within this of the first walk (station, Julie's, the town). */
const FIRST_CLEAR = 44;
/** Wildlife: the chance a macro cell away from the road has something in it, by threat. */
const WILD = [0, 0.018, 0.026, 0.04, 0.09, 0.11, 0.13];
/** A screen, for the last pass: the camera's view in cells. */
const SCREEN_W = 48;
const SCREEN_H = 27;

// Distances are kept on a 4-cell grid: fine enough to stand a camp by, 16 times cheaper to fill.
const DB = 4;
const DW = Math.ceil(3600 / DB);
const DH = Math.ceil(2000 / DB);

/** What the builder may build over: open ground and growth, never water, a road or anything made. */
const BUILDABLE = new Uint8Array(64);
for (const t of [Tile.Grass, Tile.GrassTall, Tile.Dirt, Tile.Bush, Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Moss, Tile.Garden, Tile.Crops, Tile.FlowerBed, Tile.DryBed, Tile.Sand, Tile.Rubble, Tile.Cobble, Tile.Track, Tile.Rail]) BUILDABLE[t] = 1;
/** Ground a lane may be laid over. */
const SOFT = new Uint8Array(64);
for (const t of [Tile.Grass, Tile.GrassTall, Tile.Bush, Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Moss, Tile.DryBed, Tile.Sand]) SOFT[t] = 1;

const WILDLIFE: Record<Region, { def: string; biomes?: Biome[] }[]> = {
  [Region.Lowfields]: [
    { def: "rat" },
    { def: "skeleton" },
    { def: "crow", biomes: [Biome.Field, Biome.Hedge] },
    { def: "crow", biomes: [Biome.Field, Biome.Hedge] },
    { def: "bat" },
    { def: "pumpkin", biomes: [Biome.Field, Biome.Hedge] },
    { def: "spider", biomes: [Biome.Wood] },
    { def: "skeleton", biomes: [Biome.Foothill, Biome.Wood] },
  ],
  [Region.Waters]: [
    { def: "flower", biomes: [Biome.Garden, Biome.Marsh] },
    { def: "statue", biomes: [Biome.Garden] },
    { def: "spider", biomes: [Biome.WetWood] },
    { def: "rat", biomes: [Biome.Reed, Biome.Marsh] },
    { def: "crow", biomes: [Biome.Reed, Biome.Garden] },
    { def: "bat" },
    { def: "skeleton" },
  ],
  [Region.Works]: [
    { def: "soldier" },
    { def: "skeleton_guard" },
    { def: "skeleton_clerk", biomes: [Biome.Yard] },
    { def: "wall_spider", biomes: [Biome.Hill, Biome.Slag] },
    { def: "cactus", biomes: [Biome.Slag] },
    { def: "crow" },
  ],
};

export function furnishCountry(c: Country, stage: "roads" | "places" | "life"): void {
  const ctx = context(c);
  if (stage === "roads") {
    bridges(ctx);
    forks(ctx);
    lamps(ctx);
    milestones(ctx);
  } else if (stage === "places") {
    fences(ctx);
    places(ctx);
  } else {
    wildlife(ctx);
    wanderers(ctx);
    gaps(ctx);
  }
}

// --- context -----------------------------------------------------------------------------------

type Ctx = Country & {
  /** Cells to the nearest road or footpath, on the 4-cell grid, capped at 255. */
  dRoad: Uint8Array;
  /** The same to the first walk only. */
  dFirst: Uint8Array;
  lampAt: number[];
  firstLines: Set<number>;
};

let cached: { c: Country; ctx: Ctx } | null = null;

function context(c: Country): Ctx {
  if (cached && cached.c.k === c.k) return cached.ctx;
  const firstLines = new Set<number>();
  c.sk.roads.forEach((r, n) => {
    const a = r.from;
    const b = r.to;
    if ((a === "station" && b === "julie_house") || (a === "julie_house" && b === "town")) firstLines.add(n);
  });
  const ctx: Ctx = {
    ...c,
    dRoad: distanceField(c.lines, () => true),
    dFirst: distanceField(c.lines, (n) => firstLines.has(n)),
    lampAt: [],
    firstLines,
  };
  cached = { c, ctx };
  return ctx;
}

/** Two-pass chamfer distance from every point of the chosen lines, in cells, on the 4-cell grid. */
function distanceField(lines: readonly (readonly Pt[])[], use: (n: number) => boolean): Uint8Array {
  const d = new Uint16Array(DW * DH).fill(1000);
  lines.forEach((line, n) => {
    if (!use(n)) return;
    for (const [x, y] of line) {
      const bx = Math.floor(x / DB);
      const by = Math.floor(y / DB);
      if (bx >= 0 && by >= 0 && bx < DW && by < DH) d[by * DW + bx] = 0;
    }
  });
  const O = DB;
  const D = Math.round(DB * 1.414);
  for (let y = 0; y < DH; y++) {
    for (let x = 0; x < DW; x++) {
      const i = y * DW + x;
      let v = d[i];
      if (x > 0 && d[i - 1] + O < v) v = d[i - 1] + O;
      if (y > 0) {
        if (d[i - DW] + O < v) v = d[i - DW] + O;
        if (x > 0 && d[i - DW - 1] + D < v) v = d[i - DW - 1] + D;
        if (x < DW - 1 && d[i - DW + 1] + D < v) v = d[i - DW + 1] + D;
      }
      d[i] = v;
    }
  }
  for (let y = DH - 1; y >= 0; y--) {
    for (let x = DW - 1; x >= 0; x--) {
      const i = y * DW + x;
      let v = d[i];
      if (x < DW - 1 && d[i + 1] + O < v) v = d[i + 1] + O;
      if (y < DH - 1) {
        if (d[i + DW] + O < v) v = d[i + DW] + O;
        if (x < DW - 1 && d[i + DW + 1] + D < v) v = d[i + DW + 1] + D;
        if (x > 0 && d[i + DW - 1] + D < v) v = d[i + DW - 1] + D;
      }
      d[i] = v;
    }
  }
  const out = new Uint8Array(DW * DH);
  for (let i = 0; i < out.length; i++) out[i] = Math.min(255, d[i]);
  return out;
}

const dist = (field: Uint8Array, x: number, y: number): number => {
  const bx = Math.floor(x / DB);
  const by = Math.floor(y / DB);
  if (bx < 0 || by < 0 || bx >= DW || by >= DH) return 255;
  return field[by * DW + bx];
};

const macroOf = (x: number, y: number): number => at(Math.max(0, Math.min(SKEL_W - 1, x >> 4)), Math.max(0, Math.min(SKEL_H - 1, y >> 4)));

function nearChunk(c: Ctx, x: number, y: number, margin: number): boolean {
  for (const ch of c.chunks) {
    const b = ch.box;
    if (x >= b.cx - margin && y >= b.cy - margin && x < b.cx + b.w + margin && y < b.cy + b.h + margin) return true;
  }
  return false;
}

// --- small building tools ------------------------------------------------------------------

/** Every cell of the rect is open or growing ground nobody has claimed. */
function room(c: Ctx, x0: number, y0: number, w: number, h: number): boolean {
  const k = c.k;
  if (x0 < 8 || y0 < 8 || x0 + w > k.w - 8 || y0 + h > k.h - 8) return false;
  const tiles = k.tiles;
  const kw = k.w;
  for (let y = y0; y < y0 + h; y++) {
    for (let x = x0; x < x0 + w; x++) {
      if (!BUILDABLE[tiles[y * kw + x]] || k.isClaimed(x, y)) return false;
    }
  }
  return true;
}

/** Growth gives way: trees, bushes, rubble become the ground a place stands on. */
function clear(c: Ctx, x0: number, y0: number, w: number, h: number, t: Tile = Tile.Grass): void {
  const k = c.k;
  for (let y = y0; y < y0 + h; y++) {
    for (let x = x0; x < x0 + w; x++) {
      const was = k.get(x, y);
      if (was === Tile.Tree || was === Tile.Pine || was === Tile.DeadTree || was === Tile.Bush || was === Tile.Rubble) k.set(x, y, t);
    }
  }
}

/** Worn ground: an ellipse of `t` with a ragged rim. */
function pad(c: Ctx, cx: number, cy: number, rx: number, ry: number, t: Tile): void {
  const k = c.k;
  const jn = Math.ceil(ry) + 1;
  const in_ = Math.ceil(rx) + 1;
  for (let j = -jn; j <= jn; j++) {
    for (let i = -in_; i <= in_; i++) {
      const d = (i * i) / (rx * rx) + (j * j) / (ry * ry);
      const x = cx + i;
      const y = cy + j;
      if (!BUILDABLE[k.get(x, y)] || k.isClaimed(x, y)) continue;
      if (d <= 0.75 || (d <= 1.15 && k.chance(0.6))) k.set(x, y, t);
    }
  }
}

function put(c: Ctx, def: string, x: number, y: number, extra: { key?: string; talk?: string; label?: string; loot?: { item: string; qty: number }[]; use?: import("@/sim/state").ActionList } = {}): boolean {
  const size = c.sizes[def] ?? { w: 1, h: 1 };
  x = Math.round(x);
  y = Math.round(y);
  if (!c.k.fits(x, y, size.w, size.h)) return false;
  c.k.prop({ def, cx: x, cy: y, ...extra }, size.w, size.h);
  return true;
}

/** Somebody who lives here: walks a short round between a few points, and stands at each. */
function folk(c: Ctx, def: string, x: number, y: number, reach: number): void {
  const k = c.k;
  x = Math.round(x);
  y = Math.round(y);
  if (k.solid(x, y) || k.isClaimed(x, y)) return;
  const patrol: [number, number, number][] = [[x, y, 120 + k.roll(240)]];
  for (let n = 0; n < 2; n++) {
    const px = x + k.int(-reach, reach);
    const py = y + k.int(-reach, reach);
    if (!k.solid(px, py)) patrol.push([px, py, 90 + k.roll(300)]);
  }
  k.unit(null, def, x, y, patrol.length > 1 ? patrol : undefined);
}

/** Something that bites, playing at the threat of the ground it stands on. */
function hostile(c: Ctx, def: string, x: number, y: number, patrol?: UnitSpawn["patrol"]): boolean {
  const k = c.k;
  x = Math.round(x);
  y = Math.round(y);
  if (k.solid(x, y) || k.isClaimed(x, y) || k.get(x, y) === Tile.Water) return false;
  const threat = c.sk.threat[macroOf(x, y)];
  if (threat === 0) return false;
  if (dist(c.dFirst, x, y) < FIRST_CLEAR) return false;
  k.unit(null, def, x, y, patrol).phase = threat;
  return true;
}

/** A lane of worn dirt from a place's front to the nearest road, downhill on the distance field. */
function lane(c: Ctx, x: number, y: number, width = 2): void {
  const k = c.k;
  let bx = Math.floor(x / DB);
  let by = Math.floor(y / DB);
  let px = x;
  let py = y;
  for (let steps = 0; steps < 30; steps++) {
    const here = c.dRoad[by * DW + bx];
    if (here <= 2) break;
    let best = here;
    let nx = bx;
    let ny = by;
    // Straight steps first: a lane runs square to things where it can.
    for (const [ox, oy] of [[0, 1], [0, -1], [1, 0], [-1, 0], [1, 1], [-1, 1], [1, -1], [-1, -1]]) {
      const qx = bx + ox;
      const qy = by + oy;
      if (qx < 0 || qy < 0 || qx >= DW || qy >= DH) continue;
      const v = c.dRoad[qy * DW + qx];
      if (v < best) {
        best = v;
        nx = qx;
        ny = qy;
      }
    }
    if (nx === bx && ny === by) break;
    const tx = nx * DB + 1;
    const ty = ny * DB + 1;
    const n = Math.max(Math.abs(tx - px), Math.abs(ty - py));
    for (let s = 0; s <= n; s++) {
      const lx = Math.round(px + ((tx - px) * s) / Math.max(1, n));
      const ly = Math.round(py + ((ty - py) * s) / Math.max(1, n));
      for (let j = 0; j < width; j++) {
        for (let i = 0; i < width; i++) {
          const t = k.get(lx + i, ly + j);
          // A roadside fence gets a gate where the lane meets it.
          if (t === Tile.Fence && dist(c.dRoad, lx + i, ly + j) <= 6) {
            k.set(lx + i, ly + j, Tile.Dirt);
            continue;
          }
          // Over growth and open grass only; claimed ground is crossed only where it is still grass (a road's margin).
          if (!SOFT[t] && t !== Tile.Grass) continue;
          if (k.isClaimed(lx + i, ly + j) && t !== Tile.Grass && t !== Tile.GrassTall) continue;
          k.set(lx + i, ly + j, Tile.Dirt);
        }
      }
    }
    px = tx;
    py = ty;
    bx = nx;
    by = ny;
  }
}

/** A straight fence, hedge or wall with a gate every so often; never across a path, a road or anything claimed. */
function run(c: Ctx, x0: number, y0: number, len: number, horizontal: boolean, t: Tile, gateEvery = 0): void {
  const k = c.k;
  for (let i = 0; i < len; i++) {
    const x = horizontal ? x0 + i : x0;
    const y = horizontal ? y0 : y0 + i;
    if (gateEvery > 0 && i % gateEvery >= gateEvery - 3) continue;
    const was = k.get(x, y);
    if (!(was === Tile.Grass || was === Tile.GrassTall || was === Tile.Moss || was === Tile.Bush || was === Tile.Tree || was === Tile.Pine) || k.isClaimed(x, y)) continue;
    k.set(x, y, t);
  }
}

/** A closed fence round a rect, with a gate of three on one side. */
function pen(c: Ctx, x0: number, y0: number, w: number, h: number, gate: "n" | "s" | "e" | "w", t: Tile = Tile.Fence): void {
  const k = c.k;
  const g = (i: number, len: number): boolean => i >= (len >> 1) - 1 && i <= (len >> 1) + 1;
  for (let i = 0; i < w; i++) {
    if (!(gate === "n" && g(i, w))) k.set(x0 + i, y0, t);
    if (!(gate === "s" && g(i, w))) k.set(x0 + i, y0 + h - 1, t);
  }
  for (let j = 0; j < h; j++) {
    if (!(gate === "w" && g(j, h))) k.set(x0, y0 + j, t);
    if (!(gate === "e" && g(j, h))) k.set(x0 + w - 1, y0 + j, t);
  }
}

/** Which way the road is from here, as a side of a rect. */
function roadSide(c: Ctx, x: number, y: number): "n" | "s" | "e" | "w" {
  let best = 255;
  let side: "n" | "s" | "e" | "w" = "s";
  for (const [s, ox, oy] of [["s", 0, 12], ["n", 0, -12], ["e", 12, 0], ["w", -12, 0]] as const) {
    const v = dist(c.dRoad, x + ox, y + oy);
    if (v < best) {
      best = v;
      side = s;
    }
  }
  return side;
}

// --- 1 the roads --------------------------------------------------------------------------------

const roadLines = (c: Ctx): number => c.sk.roads.length;

/** Unit normal of a line at point i, to the right of travel, as whole cells (cardinal or diagonal). */
function normal(line: readonly Pt[], i: number): [number, number] {
  const a = line[Math.max(0, i - 5)];
  const b = line[Math.min(line.length - 1, i + 5)];
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = Math.sqrt(dx * dx + dy * dy) || 1;
  return [-dy / len, dx / len];
}

function lampFree(c: Ctx, x: number, y: number, apart: number): boolean {
  for (let n = 0; n < c.lampAt.length; n += 2) {
    const dx = c.lampAt[n] - x;
    const dy = c.lampAt[n + 1] - y;
    if (dx * dx + dy * dy < apart * apart) return false;
  }
  return true;
}

/** A lamp beside the road at point i, on the given side. Returns true if one went down. */
function lampBeside(c: Ctx, line: readonly Pt[], i: number, side: number, apart: number, def = "lamp_post"): boolean {
  const k = c.k;
  const [nx, ny] = normal(line, i);
  for (const off of [LAMP_OFF, LAMP_OFF + 1, LAMP_OFF + 2]) {
    const x = Math.round(line[i][0] + nx * off * side);
    const y = Math.round(line[i][1] + ny * off * side);
    const t = k.get(x, y);
    if (t === Tile.Road || t === Tile.Water || k.solid(x, y) || k.isClaimed(x, y) || nearChunk(c, x, y, 2)) continue;
    if (!lampFree(c, x, y, apart)) return false;
    k.prop({ def, cx: x, cy: y }, 1, 1);
    c.lampAt.push(x, y);
    return true;
  }
  return false;
}

/** A lamp at each end of a bridge, on the road's lamp side. Bridges are lit whatever the road is. */
function bridges(c: Ctx): void {
  const k = c.k;
  const kw = k.w;
  for (let n = 0; n < roadLines(c); n++) {
    const line = c.lines[n];
    let wet = false;
    for (let i = 0; i < line.length; i++) {
      const [x, y] = line[i];
      const now = c.before[y * kw + x] === Tile.Water;
      if (now !== wet) {
        // The last dry point before the water, or the first one after it.
        const at = now ? Math.max(0, i - 4) : Math.min(line.length - 1, i + 3);
        lampBeside(c, line, at, 1, 6);
        wet = now;
      }
    }
  }
}

/**
 * Forks: where a road leaves the network that was there before it. Each gets a fingerpost off
 * the corner, naming the places each way goes and how far by road, and a lamp.
 */
function forks(c: Ctx): void {
  const sk = c.sk;
  const k = c.k;
  const named = sk.sites.filter((s) => s.row.onRoad && s.id !== "julie_house");
  const far = new Map<string, Float64Array>();
  const from = (id: string): Float64Array => {
    let d = far.get(id);
    if (!d) {
      const s = sk.sites.find((x) => x.id === id)!;
      far.set(id, (d = roadDistances(sk.road, s.mx, s.my)));
    }
    return d;
  };
  const seen = new Set<number>();
  const older = new Set<number>();
  sk.roads.forEach((r, n) => {
    const line = c.lines[n];
    for (let i = 1; i < r.cells.length; i++) {
      const was = older.has(r.cells[i - 1]);
      const now = older.has(r.cells[i]);
      if (was === now || n === 0) continue;
      const fork = was ? r.cells[i - 1] : r.cells[i];
      if (seen.has(fork)) continue;
      seen.add(fork);
      signFork(c, fork, named, from, line);
    }
    for (const cell of r.cells) older.add(cell);
  });
  void k;
}

export function compass(dx: number, dy: number): string {
  const ax = Math.abs(dx);
  const ay = Math.abs(dy);
  const ns = dy < 0 ? "NORTH" : "SOUTH";
  const ew = dx < 0 ? "WEST" : "EAST";
  if (ax > ay * 2.2) return ew;
  if (ay > ax * 2.2) return ns;
  return `${ns}-${ew}`;
}

export function distanceWords(m: number): string {
  if (m < 1000) return `${Math.max(50, Math.round(m / 50) * 50)} m`;
  return `${(Math.round(m / 100) / 10).toFixed(1)} km`;
}

function signFork(c: Ctx, fork: number, named: readonly Skeleton["sites"][number][], from: (id: string) => Float64Array, line: readonly Pt[]): void {
  const sk = c.sk;
  const k = c.k;
  const fx = fork % SKEL_W;
  const fy = Math.floor(fork / SKEL_W);
  // Each way out of the fork: the neighbouring road cells. For each place, the way it lies is the neighbour nearest it.
  const ways = new Map<string, { dir: string; list: { name: string; m: number }[] }>();
  for (const s of named) {
    const d = from(s.id);
    const here = d[fork];
    if (!isFinite(here) || here < 60) continue;
    let best = here;
    let step = -1;
    for (let oy = -1; oy <= 1; oy++) {
      for (let ox = -1; ox <= 1; ox++) {
        const x = fx + ox;
        const y = fy + oy;
        if ((ox === 0 && oy === 0) || x < 0 || y < 0 || x >= SKEL_W || y >= SKEL_H) continue;
        const j = at(x, y);
        if (!(sk.road[j] & ROAD) || d[j] >= best) continue;
        best = d[j];
        step = j;
      }
    }
    if (step < 0) continue;
    // Look a few cells down that way, so a wiggle at the fork does not name the wrong wind.
    let cur = step;
    for (let n = 0; n < 5; n++) {
      const cx = cur % SKEL_W;
      const cy = Math.floor(cur / SKEL_W);
      let nb = d[cur];
      let nxt = cur;
      for (let oy = -1; oy <= 1; oy++) {
        for (let ox = -1; ox <= 1; ox++) {
          const x = cx + ox;
          const y = cy + oy;
          if (x < 0 || y < 0 || x >= SKEL_W || y >= SKEL_H) continue;
          const j = at(x, y);
          if (sk.road[j] & ROAD && d[j] < nb) {
            nb = d[j];
            nxt = j;
          }
        }
      }
      if (nxt === cur) break;
      cur = nxt;
    }
    const dir = compass((cur % SKEL_W) - fx, Math.floor(cur / SKEL_W) - fy);
    const key = String(step);
    const way = ways.get(key) ?? { dir, list: [] };
    way.list.push({ name: s.name.toUpperCase(), m: here });
    ways.set(key, way);
  }
  if (ways.size < 2) return;
  const parts = [...ways.values()]
    .map((w) => {
      w.list.sort((a, b) => a.m - b.m);
      return `${w.dir}: ${w.list
        .slice(0, 2)
        .map((x) => `${x.name}, ${distanceWords(x.m)}`)
        .join("; ")}`;
    })
    .sort();
  // The post stands off the corner, on open ground beside the verge.
  const cx = fx * MACRO + MACRO / 2;
  const cy = fy * MACRO + MACRO / 2;
  let near = 0;
  let bd = Infinity;
  line.forEach(([x, y], i) => {
    const dd = (x - cx) * (x - cx) + (y - cy) * (y - cy);
    if (dd < bd) {
      bd = dd;
      near = i;
    }
  });
  const text = `${parts.join(". ")}.`;
  for (const side of [-1, 1]) {
    const [nx, ny] = normal(line, near);
    for (const off of [4, 5, 6]) {
      const x = Math.round(line[near][0] + nx * off * side);
      const y = Math.round(line[near][1] + ny * off * side);
      if (nearChunk(c, x, y, 4) || !placeable(c, x, y, 2, 1)) continue;
      k.prop({ def: "fingerpost", cx: x, cy: y, label: "A fingerpost", use: [{ do: "read", text }] }, 2, 1);
      // And a lamp on the other corner, so a fork can be found after dark.
      lampBeside(c, line, near, -side, 8);
      return;
    }
  }
}

/** Open, dry, unclaimed ground off the metal for a small thing by the road. */
function placeable(c: Ctx, x: number, y: number, w: number, h: number): boolean {
  const k = c.k;
  for (let j = y; j < y + h; j++) {
    for (let i = x; i < x + w; i++) {
      const t = k.get(i, j);
      if (t === Tile.Road || t === Tile.Water || k.solid(i, j) || k.isClaimed(i, j)) return false;
    }
  }
  return true;
}

/** Lamps along every lit stretch: one side of each road, an even step, set off the verge. */
function lamps(c: Ctx): void {
  for (let n = 0; n < roadLines(c); n++) {
    const line = c.lines[n];
    const lit = c.lit[n];
    // The side is the road's, not the step's: every lamp on this road is on its right.
    const side = 1;
    let walked = 0;
    let next = LAMP_STEP * 0.6;
    for (let i = 6; i < line.length - 6; i++) {
      const dx = line[i][0] - line[i - 1][0];
      const dy = line[i][1] - line[i - 1][1];
      walked += Math.sqrt(dx * dx + dy * dy);
      if (walked < next || !lit[i]) continue;
      if (lampBeside(c, line, i, side, LAMP_STEP - 6)) next = walked + LAMP_STEP;
    }
  }
}

/** Milestones: the distance to Castle, cut in stone, on the side the lamps are not. */
function milestones(c: Ctx): void {
  const sk = c.sk;
  const town = sk.sites.find((s) => s.id === "town");
  if (!town) return;
  const d = roadDistances(sk.road, town.mx, town.my);
  for (let n = 0; n < roadLines(c); n++) {
    const line = c.lines[n];
    let walked = 0;
    let next = MILE_STEP * 0.5;
    for (let i = 6; i < line.length - 6; i++) {
      const dx = line[i][0] - line[i - 1][0];
      const dy = line[i][1] - line[i - 1][1];
      walked += Math.sqrt(dx * dx + dy * dy);
      if (walked < next) continue;
      const m = d[macroOf(line[i][0], line[i][1])];
      if (!isFinite(m) || m < 250) continue;
      const [nx, ny] = normal(line, i);
      const x = Math.round(line[i][0] - nx * LAMP_OFF);
      const y = Math.round(line[i][1] - ny * LAMP_OFF);
      if (nearChunk(c, x, y, 4) || !placeable(c, x, y, 1, 1)) continue;
      c.k.prop({ def: "milestone", cx: x, cy: y, label: "A milestone", use: [{ do: "read", text: `CASTLE ${distanceWords(m)}` }] }, 1, 1);
      next = walked + MILE_STEP;
    }
  }
}

/**
 * Field edges along the roads: where a road runs straight for a while through farmland, the
 * field beside it has a fence (or a hedge, or a low wall) along it, on the side the lamps are
 * not, in a straight line with a gate every so often.
 */
function fences(c: Ctx): void {
  const sk = c.sk;
  const k = c.k;
  for (let n = 0; n < roadLines(c); n++) {
    const line = c.lines[n];
    const SPAN = 26;
    for (let i = 10; i + SPAN < line.length - 10; i += SPAN + 6) {
      const a = line[i];
      const b = line[i + SPAN];
      const m = macroOf(a[0], a[1]);
      const region = sk.region[m] as Region;
      const biome = sk.biome[m] as Biome;
      let t: Tile | null = null;
      if (region === Region.Lowfields && (biome === Biome.Field || biome === Biome.Hedge)) t = biome === Biome.Hedge ? Tile.Bush : Tile.Fence;
      else if (region === Region.Lowfields && biome === Biome.Foothill) t = Tile.StoneWall;
      else if (region === Region.Waters && (biome === Biome.Garden || biome === Biome.Reed)) t = biome === Biome.Garden ? Tile.StoneWall : Tile.Fence;
      if (t === null || !k.chance(0.7)) continue;
      const horizontal = Math.abs(b[0] - a[0]) >= Math.abs(b[1] - a[1]) * 3;
      const vertical = Math.abs(b[1] - a[1]) >= Math.abs(b[0] - a[0]) * 3;
      if (!horizontal && !vertical) continue;
      // The field side: away from the lamps, which are on the right of travel.
      const [nx, ny] = normal(line, i + (SPAN >> 1));
      // Lamps stand on +normal; the field edge is on the other side, four cells past the road's furthest wobble.
      if (horizontal) {
        const north = ny > 0;
        let edge = north ? Infinity : -Infinity;
        for (let j = i; j <= i + SPAN; j++) edge = north ? Math.min(edge, line[j][1]) : Math.max(edge, line[j][1]);
        const y = north ? edge - 4 : edge + 4;
        run(c, Math.min(a[0], b[0]), y, Math.abs(b[0] - a[0]) + 1, true, t, 13);
        k.claim(Math.min(a[0], b[0]), y, Math.abs(b[0] - a[0]) + 1, 1);
      } else {
        const west = nx > 0;
        let edge = west ? Infinity : -Infinity;
        for (let j = i; j <= i + SPAN; j++) edge = west ? Math.min(edge, line[j][0]) : Math.max(edge, line[j][0]);
        const x = west ? edge - 4 : edge + 4;
        run(c, x, Math.min(a[1], b[1]), Math.abs(b[1] - a[1]) + 1, false, t, 13);
        k.claim(x, Math.min(a[1], b[1]), 1, Math.abs(b[1] - a[1]) + 1);
      }
    }
  }
}

// --- 2 the places --------------------------------------------------------------------------------

type Kind =
  | "hamlet"
  | "farmstead"
  | "cottage"
  | "inn"
  | "orchard"
  | "field"
  | "shrine"
  | "well"
  | "wreck"
  | "hay"
  | "meadow"
  | "herd"
  | "woodcutter"
  | "pond"
  | "stones"
  | "camp"
  | "den"
  | "ruin"
  | "outcrop"
  | "reedhut"
  | "glass"
  | "slag"
  | "graves";

type Weights = Partial<Record<Kind, number>>;

const TABLES: Record<Region, { road: Weights; deep: Weights }> = {
  [Region.Lowfields]: {
    road: { hamlet: 6, farmstead: 6, cottage: 7, inn: 1, orchard: 3, field: 5, shrine: 2, well: 2, wreck: 2, hay: 2, meadow: 3, herd: 3, woodcutter: 3, pond: 2, stones: 1 },
    deep: { camp: 5, den: 3, ruin: 4, pond: 3, outcrop: 2, stones: 2, orchard: 1, herd: 4, woodcutter: 2, meadow: 3, field: 2, farmstead: 1 },
  },
  [Region.Waters]: {
    road: { reedhut: 6, cottage: 3, ruin: 4, glass: 3, pond: 3, shrine: 2, meadow: 3, wreck: 2, well: 1, herd: 1, graves: 1 },
    deep: { camp: 6, den: 4, ruin: 4, pond: 4, meadow: 3, stones: 2, glass: 2, outcrop: 1, reedhut: 1 },
  },
  [Region.Works]: {
    road: { ruin: 6, slag: 5, wreck: 3, graves: 2, shrine: 1, outcrop: 1 },
    deep: { camp: 9, den: 4, ruin: 5, slag: 4, outcrop: 3, graves: 2 },
  },
};

/** Footprint each kind asks for, centred on the lattice point. */
const SIZE: Record<Kind, [number, number]> = {
  hamlet: [36, 30],
  farmstead: [40, 30],
  cottage: [22, 16],
  inn: [28, 20],
  orchard: [20, 16],
  field: [24, 16],
  shrine: [10, 9],
  well: [12, 10],
  wreck: [12, 9],
  hay: [16, 12],
  meadow: [14, 12],
  herd: [14, 12],
  woodcutter: [20, 18],
  pond: [16, 12],
  stones: [15, 15],
  camp: [16, 14],
  den: [12, 10],
  ruin: [16, 13],
  outcrop: [12, 10],
  reedhut: [16, 13],
  glass: [18, 13],
  slag: [16, 12],
  graves: [16, 12],
};

/** Kinds that must stand beside a road (their front faces it), and kinds that must not. */
const ROADSIDE: Set<Kind> = new Set(["hamlet", "farmstead", "cottage", "inn", "shrine", "well", "wreck", "reedhut"]);

/** Roadside kinds, the road table without the kinds that belong in the middle of a field. */
const ALONG: Record<Region, Weights> = {
  [Region.Lowfields]: { hamlet: 6, farmstead: 6, cottage: 7, inn: 1, orchard: 3, field: 4, shrine: 2, well: 2, wreck: 2, hay: 2, meadow: 2, woodcutter: 3, pond: 1 },
  [Region.Waters]: { reedhut: 6, cottage: 3, ruin: 4, glass: 3, pond: 2, shrine: 2, meadow: 2, wreck: 2, well: 1, graves: 1 },
  [Region.Works]: { ruin: 6, slag: 5, wreck: 3, graves: 2, shrine: 1 },
};

/**
 * The roads first: something beside every road at a steady beat, the way farms and cottages
 * string out along a lane. Each road is walked and, every sixty-odd cells, a place is set just
 * off its verge on one side or the other, its front toward the road.
 */
function alongRoads(c: Ctx): void {
  const sk = c.sk;
  const k = c.k;
  for (let n = 0; n < roadLines(c); n++) {
    const line = c.lines[n];
    let next = 20 + k.roll(30);
    for (let i = 12; i < line.length - 12; i++) {
      if (i < next) continue;
      const [lx, ly] = line[i];
      const m = macroOf(lx, ly);
      if (nearChunk(c, lx, ly, 20) || inDressedArea(c, lx, ly)) continue;
      const region = sk.region[m] as Region;
      let placed = false;
      for (let tries = 0; tries < 3 && !placed; tries++) {
        const kind = pickKind(k, ALONG[region]);
        if (!kind) break;
        const [w, h] = SIZE[kind];
        const [nx, ny] = normal(line, i);
        const first = k.chance(0.5) ? 1 : -1;
        for (const side of [first, -first]) {
          // Far enough out that the whole footprint clears the road and its verge, and no further.
          const reach = Math.abs(nx) * (w / 2) + Math.abs(ny) * (h / 2) + 8;
          for (const extra of [0, 5, 10]) {
            const x = Math.round(lx + nx * (reach + extra) * side);
            const y = Math.round(ly + ny * (reach + extra) * side);
            if (!allowed(c, kind, x, y, -1)) continue;
            if (stamp(c, kind, x, y)) {
              placed = true;
              break;
            }
          }
          if (placed) break;
        }
      }
      // The first walk is the establishing shot, and gets something about twice as often.
      next = i + (placed ? (c.firstLines.has(n) ? 36 + k.roll(20) : 60 + k.roll(36)) : 10);
    }
  }
}

function places(c: Ctx): void {
  const sk = c.sk;
  const k = c.k;
  alongRoads(c);
  const cols = Math.floor(k.w / LATTICE);
  const rowsN = Math.floor(k.h / LATTICE);
  for (let gy = 0; gy < rowsN; gy++) {
    for (let gx = 0; gx < cols; gx++) {
      const x = gx * LATTICE + 6 + k.roll(LATTICE - 12);
      const y = gy * LATTICE + 6 + k.roll(LATTICE - 12);
      if (x < 24 || y < 24 || x > k.w - 24 || y > k.h - 24) continue;
      if (nearChunk(c, x, y, 16) || inDressedArea(c, x, y)) continue;
      const m = macroOf(x, y);
      if (sk.water[m] && k.get(x, y) === Tile.Water) continue;
      const region = sk.region[m] as Region;
      const d = dist(c.dRoad, x, y);
      if (d < 7) continue;
      const band = d <= 28 ? "road" : "deep";
      const table = TABLES[region][band];
      // Three throws: the kind the dice chose, and then others, until one the ground allows fits.
      for (let tries = 0; tries < 3; tries++) {
        const kind = pickKind(k, table);
        if (!kind || !allowed(c, kind, x, y, d)) continue;
        if (stamp(c, kind, x, y)) break;
      }
    }
  }
}

/** Patches world/areas.ts draws as places of their own (the allotments, the Top Field, the quarry): nothing is built over them. */
const DRESSED: Record<string, number> = { allotments: 0.7, top_field: 0.7, quarry_steps: 0.5 };

function inDressedArea(c: Ctx, x: number, y: number): boolean {
  for (const a of c.sk.areas) {
    const share = DRESSED[a.id];
    if (share === undefined) continue;
    const dx = a.mx * MACRO + MACRO / 2 - x;
    const dy = a.my * MACRO + MACRO / 2 - y;
    const r = a.row.radius * share + 20;
    if (dx * dx + dy * dy < r * r) return true;
  }
  return false;
}

function pickKind(k: Kit, w: Weights): Kind | null {
  const entries = Object.entries(w) as [Kind, number][];
  let total = 0;
  for (const [, v] of entries) total += v;
  let roll = k.float() * total;
  for (const [kind, v] of entries) if ((roll -= v) <= 0) return kind;
  return entries.length > 0 ? entries[entries.length - 1][0] : null;
}

function allowed(c: Ctx, kind: Kind, x: number, y: number, d: number): boolean {
  const sk = c.sk;
  const m = macroOf(x, y);
  const biome = sk.biome[m] as Biome;
  const threat = sk.threat[m];
  // d < 0: set by the roadside pass, which has already put it beside the road.
  const big = kind === "hamlet" || kind === "farmstead" || kind === "inn";
  if (d >= 0 && ROADSIDE.has(kind) && (d < (big ? 14 : 9) || d > (big ? 28 : 22))) return false;
  switch (kind) {
    case "hamlet":
    case "farmstead":
    case "inn":
      return biome !== Biome.Wood && biome !== Biome.WetWood && biome !== Biome.Marsh && threat <= 3;
    case "cottage":
      return threat <= 3 && biome !== Biome.Marsh;
    case "camp":
    case "den":
      return d >= CAMP_BACK && threat > 0 && dist(c.dFirst, x, y) >= FIRST_CLEAR + 8;
    case "woodcutter":
      return biome === Biome.Wood || biome === Biome.WetWood || biome === Biome.Foothill;
    case "reedhut":
      return biome === Biome.Reed || biome === Biome.Marsh || biome === Biome.WetWood || biome === Biome.Garden;
    case "outcrop":
      return biome === Biome.Foothill || biome === Biome.Hill || biome === Biome.Slag || biome === Biome.Field || biome === Biome.Yard;
    case "field":
    case "orchard":
    case "hay":
      return biome === Biome.Field || biome === Biome.Hedge || biome === Biome.Garden;
    default:
      return true;
  }
}

function stamp(c: Ctx, kind: Kind, x: number, y: number): boolean {
  const [w, h] = SIZE[kind];
  const x0 = x - (w >> 1);
  const y0 = y - (h >> 1);
  if (!room(c, x0, y0, w, h)) return false;
  clear(c, x0, y0, w, h, kind === "woodcutter" ? Tile.GrassTall : Tile.Grass);
  switch (kind) {
    case "hamlet":
      hamlet(c, x0, y0);
      break;
    case "farmstead":
      farmstead(c, x0, y0);
      break;
    case "cottage":
      cottage(c, x0, y0);
      break;
    case "inn":
      inn(c, x0, y0);
      break;
    case "orchard":
      orchard(c, x0, y0);
      break;
    case "field":
      field(c, x0, y0, w, h);
      break;
    case "shrine":
      pad(c, x, y + 1, 4, 3, Tile.Cobble);
      put(c, "wayside_shrine", x - 1, y - 2, { talk: "country_shrine" });
      for (let n = 0; n < 4; n++) put(c, "flowers", x - 4 + c.k.roll(9), y + c.k.roll(3));
      break;
    case "well":
      pad(c, x, y, 5, 4, Tile.Cobble);
      put(c, "well", x - 1, y - 2, { talk: "country_well" });
      put(c, "trough", x + 2, y + 1);
      put(c, "log", x - 5, y + 1);
      break;
    case "wreck":
      pad(c, x, y, 5, 3, Tile.Dirt);
      put(c, "cart_wreck", x - 2, y - 1);
      put(c, c.k.chance(0.5) ? "crate" : "barrel", x + 2, y + 1);
      put(c, "bones", x - 3, y + 2);
      if (c.k.chance(0.35)) put(c, "chest", x - 5, y - 2, { loot: loot(c, x, y) });
      break;
    case "hay":
      pad(c, x, y, 7, 5, Tile.Dirt);
      put(c, "haystack", x - 5, y - 3);
      put(c, "haystack", x - 1, y - 4);
      put(c, "hay_cart", x + 3, y);
      if (c.k.chance(0.6)) put(c, "haystack", x - 3, y + 1);
      break;
    case "meadow":
      for (let n = 0; n < 9; n++) put(c, "flowers", x - 6 + c.k.roll(13), y - 5 + c.k.roll(11));
      if (c.k.chance(0.6)) {
        put(c, "beehive", x - 2, y - 1, { talk: "country_hive" });
        put(c, "beehive", x + 1, y - 1);
      }
      break;
    case "herd":
      herd(c, x, y);
      break;
    case "woodcutter":
      woodcutter(c, x0, y0, w, h);
      break;
    case "pond":
      pond(c, x, y);
      break;
    case "stones":
      stones(c, x, y);
      break;
    case "camp":
      camp(c, x, y);
      break;
    case "den":
      den(c, x, y);
      break;
    case "ruin":
      ruin(c, x0, y0);
      break;
    case "outcrop":
      outcrop(c, x, y);
      break;
    case "reedhut":
      reedhut(c, x0, y0);
      break;
    case "glass":
      glasshouse(c, x0, y0);
      break;
    case "slag":
      slag(c, x, y);
      break;
    case "graves":
      graves(c, x0, y0);
      break;
  }
  c.k.claim(x0, y0, w, h);
  return true;
}

const COTTAGES = ["cottage_thatch", "cottage_timber", "cottage_slate", "cottage_tile"];
const DOORS = ["country_door_1", "country_door_2", "country_door_3", "country_door_4"];

function loot(c: Ctx, x: number, y: number): { item: string; qty: number }[] {
  const region = c.sk.region[macroOf(x, y)] as Region;
  const k = c.k;
  if (region === Region.Lowfields) return [{ item: k.pick(["apple", "small_water", "wood"]), qty: 1 + k.roll(2) }];
  if (region === Region.Waters) return [{ item: k.pick(["small_water", "white_water_cap", "honeylace_lily"]), qty: 1 + k.roll(2) }];
  return [{ item: k.pick(["coal", "iron", "small_water"]), qty: 1 + k.roll(2) }];
}

/** Two to four cottages round a green with a well, a hen house, a garden, and people. */
function hamlet(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  // The north row: up to three houses, doors on the green.
  const n = 2 + k.roll(2);
  const slots = n === 2 ? [3, 21] : [0, 12, 24];
  const first = k.roll(4);
  slots.forEach((sx, i) => {
    const def = COTTAGES[(first + i) % 4];
    const w = def === "cottage_tile" ? 9 : 8;
    const hx = x0 + sx + (i === slots.length - 1 && w === 9 ? -1 : 0);
    put(c, def, hx, y0 + 1, { talk: k.pick(DOORS) });
    put(c, "flowerbed", hx + (k.chance(0.5) ? 0 : w - 2), y0 + 7);
    if (k.chance(0.5)) for (let i = 0; i < 3; i++) k.set(hx + 3 + i, y0 + 8, Tile.FlowerBed);
  });
  // The green: worn ground, and the well in the middle of it.
  const gx = x0 + 18;
  const gy = y0 + 15;
  pad(c, gx, gy, 11, 5, Tile.Dirt);
  pad(c, gx, gy, 4, 2.5, Tile.Cobble);
  put(c, "well", gx - 1, gy - 1, { talk: "country_well" });
  put(c, "trough", gx + 3, gy + 1);
  put(c, "log", gx - 7, gy - 1);
  // South of the green: a hen house and its hens, a vegetable plot, a line of washing.
  put(c, "hen_coop", x0 + 4, y0 + 22, { talk: "country_coop" });
  for (let h = 0; h < 3; h++) folk(c, "hen", x0 + 3 + k.roll(6), y0 + 24 + k.roll(3), 3);
  const plotX = x0 + 21;
  const plotY = y0 + 21;
  for (let j = 0; j < 6; j++) for (let i = 0; i < 12; i++) k.set(plotX + i, plotY + j, j % 2 === 0 ? Tile.Crops : Tile.Dirt);
  for (let i = 1; i < 12; i += 4) put(c, "crop", plotX + i, plotY + 2);
  run(c, plotX - 1, plotY + 6, 14, true, Tile.Fence);
  put(c, "washing_line", x0 + 11, y0 + 24);
  if (k.chance(0.5)) put(c, "woodpile", x0 + 13, y0 + 20);
  // People: two or three, about the green.
  const people = ["folk_old", "folk_woman", "folk_man", "folk_wife"];
  const m = 2 + k.roll(2);
  for (let i = 0; i < m; i++) folk(c, k.pick(people), gx - 8 + k.roll(16), gy - 3 + k.roll(6), 5);
  k.claim(x0, y0, 36, 30);
  // The lane leaves by the side the road is on: between two houses if that is north.
  const side = roadSide(c, gx, gy);
  if (side === "n") lane(c, x0 + (n === 2 ? 15 : 10), y0 + 4);
  else if (side === "e") lane(c, x0 + 34, gy);
  else if (side === "w") lane(c, x0 + 1, gy);
  else lane(c, gx, gy + 5);
}

/** A farmhouse and a barn across a yard, a fenced field in crops, a pen of sheep, a cart. */
function farmstead(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  put(c, "farmhouse", x0 + 2, y0 + 1, { talk: k.pick(DOORS) });
  put(c, "barn", x0 + 27, y0 + 1, { talk: "country_barn" });
  pad(c, x0 + 20, y0 + 11, 12, 3.5, Tile.Dirt);
  put(c, "hay_cart", x0 + 23, y0 + 9);
  put(c, "haystack", x0 + 15, y0 + 2);
  put(c, "pump", x0 + 14, y0 + 8, { talk: "country_pump" });
  put(c, "hen_coop", x0 + 3, y0 + 10, { talk: "country_coop" });
  for (let h = 0; h < 3; h++) folk(c, "hen", x0 + 2 + k.roll(6), y0 + 12 + k.roll(2), 3);
  // The field: rows of crops inside a fence, the gate on the yard.
  const fx = x0 + 1;
  const fy = y0 + 15;
  for (let j = 1; j < 13; j++) for (let i = 1; i < 23; i++) k.set(fx + i, fy + j, j % 2 === 1 ? Tile.Crops : Tile.Dirt);
  for (let j = 1; j < 13; j += 4) for (let i = 3; i < 22; i += 6) put(c, "crop", fx + i, fy + j);
  pen(c, fx, fy, 24, 14, "n");
  if (k.chance(0.6)) put(c, "scarecrow", fx + 11, fy + 6);
  // The sheep pen.
  const px = x0 + 27;
  const py = y0 + 17;
  pen(c, px, py, 12, 10, "w");
  for (let s = 0; s < 3; s++) folk(c, "sheep", px + 3 + k.roll(6), py + 3 + k.roll(4), 2);
  folk(c, "folk_farmer", x0 + 20, y0 + 12, 6);
  if (k.chance(0.7)) folk(c, "folk_wife", x0 + 8, y0 + 9, 4);
  k.claim(x0, y0, 40, 30);
  // Out of the yard by the side the road is on, never through the field.
  const side = roadSide(c, x0 + 20, y0 + 15);
  if (side === "n") lane(c, x0 + 19, y0 + 8);
  else if (side === "s") lane(c, x0 + 25, y0 + 14);
  else if (side === "e") lane(c, x0 + 38, y0 + 12);
  else lane(c, x0 + 1, y0 + 12);
}

/** One house, its garden, its washing, its woodpile, and whoever lives there. */
function cottage(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  const def = k.pick(COTTAGES);
  const hx = x0 + 2;
  put(c, def, hx, y0 + 1, { talk: k.pick(DOORS) });
  put(c, "flowerbed", hx, y0 + 7);
  // The garden to the east: a fenced plot, rows and a gate.
  const gx = x0 + 12;
  const gy = y0 + 2;
  for (let j = 1; j < 9; j++) for (let i = 1; i < 9; i++) k.set(gx + i, gy + j, j % 2 === 1 ? Tile.Crops : Tile.Dirt);
  for (let j = 1; j < 9; j += 4) put(c, "crop", gx + 2 + k.roll(5), gy + j);
  pen(c, gx, gy, 10, 10, "s");
  put(c, k.chance(0.5) ? "woodpile" : "beehive", x0 + 1, y0 + 10);
  if (k.chance(0.6)) put(c, "washing_line", x0 + 4, y0 + 12);
  folk(c, k.pick(["folk_old", "folk_woman", "folk_man", "folk_wife"]), hx + 4, y0 + 9, 4);
  k.claim(x0, y0, 22, 16);
  lane(c, hx + 4, y0 + 9);
}

/** The inn: a long stone house with a sign, a yard, a trough and a cart, and a lamp by the door. */
function inn(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  put(c, "inn", x0 + 2, y0 + 1, { talk: "country_inn" });
  pad(c, x0 + 12, y0 + 12, 12, 4, Tile.Cobble);
  put(c, "lamp_post", x0 + 15, y0 + 9);
  put(c, "trough", x0 + 16, y0 + 12);
  put(c, "hay_cart", x0 + 20, y0 + 14);
  put(c, "log", x0 + 3, y0 + 10);
  put(c, "log", x0 + 8, y0 + 13);
  put(c, "barrel", x0 + 24, y0 + 2);
  put(c, "barrel", x0 + 24, y0 + 5);
  folk(c, "folk_keeper", x0 + 10, y0 + 10, 3);
  folk(c, "folk_man", x0 + 6, y0 + 12, 4);
  k.claim(x0, y0, 28, 20);
  lane(c, x0 + 12, y0 + 14, 3);
}

/** Apple trees in rows, a fence along one side, hives. */
function orchard(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  const cols = 4;
  const rowsN = 3;
  for (let j = 0; j < rowsN; j++) {
    for (let i = 0; i < cols; i++) {
      const picked = k.chance(0.5);
      put(c, "apple_tree", x0 + 2 + i * 5, y0 + 2 + j * 5, picked ? {} : { loot: [{ item: "apple", qty: 1 }] });
    }
  }
  run(c, x0, y0 + 15, 20, true, Tile.Fence, 10);
  if (k.chance(0.5)) put(c, "beehive", x0 + 19, y0 + 8, { talk: "country_hive" });
  if (k.chance(0.4)) folk(c, "folk_farmer", x0 + 9, y0 + 13, 5);
}

/** A field in crops, fenced, the gate toward the road. Sometimes a scarecrow. */
function field(c: Ctx, x0: number, y0: number, w: number, h: number): void {
  const k = c.k;
  const crop = k.chance(0.5);
  for (let j = 1; j < h - 1; j++) for (let i = 1; i < w - 1; i++) k.set(x0 + i, y0 + j, j % 2 === 1 ? (crop ? Tile.Crops : Tile.Garden) : crop ? Tile.Dirt : Tile.Grass);
  for (let j = 1; j < h - 1; j += 4) for (let i = 3; i < w - 2; i += 6) put(c, "crop", x0 + i, y0 + j);
  const gate = roadSide(c, x0 + (w >> 1), y0 + (h >> 1));
  pen(c, x0, y0, w, h, gate, k.chance(0.25) ? Tile.Bush : Tile.Fence);
  if (k.chance(0.45)) put(c, "scarecrow", x0 + (w >> 1) - 1, y0 + (h >> 1));
}

/** A flock: sheep on grass, hens by a house, rabbits anywhere. Friendly, and gone at night. */
function herd(c: Ctx, x: number, y: number): void {
  const k = c.k;
  const region = c.sk.region[macroOf(x, y)] as Region;
  const def = region === Region.Works ? "rabbit" : k.pick(["sheep", "sheep", "rabbit", "hen"]);
  const n = def === "sheep" ? 3 + k.roll(3) : 2 + k.roll(2);
  for (let i = 0; i < n; i++) folk(c, def, x - 5 + k.roll(11), y - 4 + k.roll(9), 4);
  if (def === "sheep" && k.chance(0.5)) put(c, "trough", x - 1, y + 4);
}

/** A clearing in the wood: stumps, logs, stacked wood, a shelter, and in the Lowfields the woodcutter. */
function woodcutter(c: Ctx, x0: number, y0: number, w: number, h: number): void {
  const k = c.k;
  pad(c, x0 + (w >> 1), y0 + (h >> 1), 8, 6, Tile.Dirt);
  for (let n = 0; n < 6; n++) put(c, "stump", x0 + 2 + k.roll(w - 4), y0 + 2 + k.roll(h - 4));
  put(c, "log", x0 + 4, y0 + h - 5);
  put(c, "log", x0 + 12, y0 + 3);
  put(c, "woodpile", x0 + 7, y0 + 4);
  put(c, "woodpile", x0 + 10, y0 + 4);
  put(c, k.chance(0.5) ? "shed" : "tent", x0 + 13, y0 + 8, { talk: "country_shed" });
  put(c, "campfire_cold", x0 + 6, y0 + 9);
  const region = c.sk.region[macroOf(x0, y0)] as Region;
  if (region === Region.Lowfields) folk(c, "folk_woodcutter", x0 + 9, y0 + 12, 5);
}

/** Still water in a ragged bowl, tall grass round it, a plank to stand on. */
function pond(c: Ctx, x: number, y: number): void {
  const k = c.k;
  const rx = 3 + k.roll(3);
  const ry = 2 + k.roll(2);
  for (let j = -ry - 2; j <= ry + 2; j++) {
    for (let i = -rx - 2; i <= rx + 2; i++) {
      const d = (i * i) / (rx * rx) + (j * j) / (ry * ry);
      if (d <= 1) k.set(x + i, y + j, Tile.Water);
      else if (d <= 1.6) k.set(x + i, y + j, k.chance(0.5) ? Tile.GrassTall : Tile.Moss);
    }
  }
  if (k.chance(0.5)) for (let j = 0; j < 3; j++) k.set(x, y + ry - j + 1, Tile.FloorWood);
  for (let n = 0; n < 3; n++) put(c, "flowers", x - rx - 3 + k.roll(2 * rx + 6), y + ry + 2 + k.roll(2));
}

/** A ring of stones, some fallen; bones or flowers in the middle, depending on who comes. */
function stones(c: Ctx, x: number, y: number): void {
  const k = c.k;
  pad(c, x, y, 6, 6, Tile.Dirt);
  const ring: Pt[] = [[0, -5], [4, -3], [5, 1], [3, 4], [-1, 5], [-4, 3], [-5, -1], [-3, -4]];
  for (const [ox, oy] of ring) if (k.chance(0.8)) put(c, "standing_stone", x + ox, y + oy);
  put(c, k.chance(0.5) ? "bones" : "flowers", x, y);
}

/** A camp of something hostile round its fire: three to six of them, and what they keep. */
function camp(c: Ctx, x: number, y: number): void {
  const k = c.k;
  const m = macroOf(x, y);
  const region = c.sk.region[m] as Region;
  const biome = c.sk.biome[m] as Biome;
  pad(c, x, y, 7, 5.5, Tile.Dirt);
  let who: string[];
  let lit = false;
  if (region === Region.Lowfields) {
    const r = k.roll(2);
    if (r === 0) {
      who = ["ruffian", "ruffian", "ruffian", "ruffian"];
      lit = true;
    } else if (r === 1) who = ["skeleton", "skeleton", "skeleton"];
    else who = biome === Biome.Wood ? ["spider", "spider", "spider"] : ["crow", "crow", "crow", "crow"];
  } else if (region === Region.Waters) {
    const r = k.roll(2);
    if (r === 0) {
      who = ["ruffian", "ruffian", "ruffian"];
      lit = true;
    } else if (r === 1) who = ["skeleton", "skeleton", "bat", "skeleton"];
    else who = biome === Biome.Garden ? ["flower", "flower", "statue"] : ["rat", "rat", "rat", "rat"];
  } else {
    who = k.chance(0.5) ? ["soldier", "soldier", "skeleton_guard"] : ["skeleton_guard", "skeleton_clerk", "skeleton_guard", "soldier"];
  }
  if (k.chance(0.35)) who.push(who[0]);
  put(c, lit ? "camp_fire" : "campfire_cold", x - 1, y - 1);
  if (lit) {
    put(c, "tent", x - 6, y - 5, { talk: "country_tent" });
    put(c, "bedroll", x + 2, y - 4);
    put(c, "bedroll", x - 5, y + 2);
  } else if (region === Region.Works) {
    put(c, "sleepers", x - 6, y - 3);
    put(c, "barrel", x + 3, y - 4);
  } else {
    put(c, "bones", x + 2, y + 2);
    put(c, "bones", x - 4, y - 3);
  }
  put(c, "chest", x + 3, y + 1, { loot: loot(c, x, y) });
  put(c, "crate", x - 6, y + 1);
  const ringPts: Pt[] = [[-3, -3], [3, -3], [4, 1], [-4, 1], [0, 4], [0, -4], [-2, 3]];
  who.forEach((def, i) => {
    const [ox, oy] = ringPts[i % ringPts.length];
    hostile(c, def, x + ox, y + oy);
  });
}

/** A hole in a bank, and what lives in it. */
function den(c: Ctx, x: number, y: number): void {
  const k = c.k;
  const m = macroOf(x, y);
  const region = c.sk.region[m] as Region;
  const biome = c.sk.biome[m] as Biome;
  pad(c, x, y + 1, 5, 4, Tile.Dirt);
  let who: string;
  if (region === Region.Works) who = biome === Biome.Slag ? "cactus" : "wall_spider";
  else if (biome === Biome.Wood || biome === Biome.WetWood) who = "spider";
  else who = k.chance(0.3) ? "bat" : "rat";
  if (who === "spider") put(c, "web", x - 5, y - 3);
  else put(c, "den", x - 1, y - 2, { talk: "country_den" });
  put(c, "bones", x + 3, y + 2);
  put(c, "bones", x - 3, y + 3);
  const n = 2 + k.roll(3);
  for (let i = 0; i < n; i++) hostile(c, who, x - 3 + k.roll(7), y + 1 + k.roll(3));
}

/** A house with no roof: its walls to the sill, a doorway, rubble inside, growth coming in. */
function ruin(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  const region = c.sk.region[macroOf(x0, y0)] as Region;
  if (region !== Region.Works && k.chance(0.35)) {
    put(c, "cottage_empty", x0 + 4, y0 + 2, { talk: "country_door_empty" });
    pad(c, x0 + 8, y0 + 10, 5, 2, Tile.Dirt);
    for (let n = 0; n < 4; n++) {
      const bx = x0 + 1 + k.roll(14);
      const by = y0 + 9 + k.roll(3);
      if (!k.isClaimed(bx, by) && !k.solid(bx - 1, by) && !k.solid(bx + 1, by) && !k.solid(bx, by - 1) && !k.solid(bx, by + 1)) k.set(bx, by, Tile.Bush);
    }
    return;
  }
  const wall = region === Region.Works ? Tile.Wall : k.chance(0.5) ? Tile.HouseWall : Tile.Wall;
  const wx = x0 + 3;
  const wy = y0 + 2;
  const w = 10;
  const h = 8;
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) k.set(wx + i, wy + j, k.chance(0.25) ? Tile.Rubble : Tile.Dirt);
  // The walls, with a doorway south and a gap where the east wall came down.
  for (let i = 0; i < w; i++) {
    k.set(wx + i, wy, wall);
    if (i < 3 || i > 5) k.set(wx + i, wy + h - 1, wall);
  }
  for (let j = 0; j < h; j++) {
    k.set(wx, wy + j, wall);
    if (j < 2 || j > 4) k.set(wx + w - 1, wy + j, wall);
  }
  // Rubble is solid: inside, only where it cannot close the doorway off.
  for (let j = 1; j < h - 1; j++) for (let i = 1; i < w - 1; i++) if (k.get(wx + i, wy + j) === Tile.Rubble && (j >= h - 3 || i === 1 || i === w - 2)) k.set(wx + i, wy + j, Tile.Dirt);
  if (region === Region.Works) {
    put(c, "barrel", wx + 2, wy + 2);
    put(c, "sleepers", wx + w + 1, wy + 3);
  } else {
    put(c, "crate", wx + 2, wy + 2);
    put(c, "campfire_cold", wx + 5, wy + 3);
  }
  if (k.chance(0.3)) put(c, "chest", wx + 6, wy + 1, { loot: loot(c, wx, wy) });
}

/** Bare rock breaking the turf: a crag you walk round, boulders fallen off it. */
function outcrop(c: Ctx, x: number, y: number): void {
  const k = c.k;
  const r = 2 + k.roll(2);
  for (let j = -r; j <= r; j++) for (let i = -r - 1; i <= r + 1; i++) if (i * i * 0.7 + j * j <= r * r) k.set(x + i, y + j, Tile.Cliff);
  put(c, "boulder", x + r + 2, y + 1);
  put(c, "rock", x - r - 2, y + 2);
  put(c, "rock", x + 1, y + r + 2);
}

/** A reedcutter's hut on the wet ground: thatch, a plank landing, a stack of cut reed. */
function reedhut(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  pad(c, x0 + 8, y0 + 8, 7, 4, Tile.Dirt);
  put(c, "reed_hut", x0 + 2, y0 + 1, { talk: k.pick(DOORS) });
  put(c, "haystack", x0 + 10, y0 + 2);
  put(c, "crate", x0 + 10, y0 + 6);
  for (let i = 0; i < 4; i++) k.set(x0 + 12 + i, y0 + 10, Tile.FloorWood);
  const region = c.sk.region[macroOf(x0, y0)] as Region;
  if (region === Region.Waters && k.chance(0.8)) folk(c, "folk_reedcutter", x0 + 7, y0 + 8, 4);
}

/** A glasshouse with the glass mostly out of it, beds of something still growing inside. */
function glasshouse(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  const w = 14;
  const h = 9;
  const gx = x0 + 2;
  const gy = y0 + 2;
  for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) k.set(gx + i, gy + j, j % 2 === 0 ? Tile.Garden : Tile.Dirt);
  for (let i = 0; i < w; i++) {
    if (k.chance(0.7)) k.set(gx + i, gy, Tile.Glass);
    if ((i < 5 || i > 8) && k.chance(0.7)) k.set(gx + i, gy + h - 1, Tile.Glass);
  }
  for (let j = 0; j < h; j++) {
    if (k.chance(0.7)) k.set(gx, gy + j, Tile.Glass);
    if (k.chance(0.7)) k.set(gx + w - 1, gy + j, Tile.Glass);
  }
  for (let n = 0; n < 5; n++) put(c, "flowers", gx + 2 + k.roll(w - 4), gy + 2 + k.roll(h - 4));
  put(c, "barrel", gx + w + 1, gy + 1);
}

/** The Works' leavings: a heap of slag, a length of track, sleepers, a tub off its wheels. */
function slag(c: Ctx, x: number, y: number): void {
  const k = c.k;
  pad(c, x, y, 7, 5, Tile.DryBed);
  put(c, "slag_heap", x - 5, y - 4);
  if (k.chance(0.5)) put(c, "slag_heap", x + 1, y - 5);
  for (let i = -6; i <= 6; i++) if (k.get(x + i, y + 3) !== Tile.Water) k.set(x + i, y + 3, Tile.Track);
  put(c, "minecart", x + 2, y + 1);
  put(c, "sleepers", x - 4, y + 1);
}

/** A few graves inside a low wall with a gate. */
function graves(c: Ctx, x0: number, y0: number): void {
  const k = c.k;
  const w = 14;
  const h = 10;
  const gx = x0 + 1;
  const gy = y0 + 1;
  pen(c, gx, gy, w, h, "s", Tile.StoneWall);
  for (let j = 0; j < 2; j++) for (let i = 0; i < 4; i++) if (k.chance(0.8)) put(c, "gravestone", gx + 2 + i * 3, gy + 2 + j * 3, i === 1 && j === 0 ? { talk: "country_grave" } : {});
  put(c, "flowers", gx + 6, gy + 7);
}

// --- 3 the living -----------------------------------------------------------------------------

/**
 * The ground's own creatures, macro cell by macro cell. Thicker with the threat, and kept back
 * from the roads: nothing that bites stands within ROAD_CLEAR of one. The road is the safe way
 * because the country is kept off it, not because the country is empty.
 */
function wildlife(c: Ctx): void {
  const sk = c.sk;
  const k = c.k;
  for (let my = 1; my < SKEL_H - 1; my++) {
    for (let mx = 1; mx < SKEL_W - 1; mx++) {
      const i = at(mx, my);
      const threat = sk.threat[i];
      if (threat === 0 || sk.water[i]) continue;
      const cx = mx * MACRO + MACRO / 2;
      const cy = my * MACRO + MACRO / 2;
      if (dist(c.dRoad, cx, cy) < ROAD_CLEAR || dist(c.dFirst, cx, cy) < FIRST_CLEAR) continue;
      if (!k.chance(WILD[Math.min(6, threat)])) continue;
      const table = WILDLIFE[sk.region[i] as Region].filter((w) => !w.biomes || w.biomes.includes(sk.biome[i] as Biome));
      if (table.length === 0) continue;
      // Company, where the ground is bad: a pair is a reason to go another way.
      const company = threat >= 3 && k.chance(threat * 0.05) ? 2 : 1;
      const def = k.pick(table).def;
      for (let n = 0; n < company; n++) {
        const spot = k.spot({ cx: mx * MACRO, cy: my * MACRO, w: MACRO, h: MACRO }, 1, 1, 1, 8);
        if (!spot || dist(c.dRoad, spot.cx, spot.cy) < ROAD_CLEAR) break;
        hostile(c, def, spot.cx, spot.cy);
      }
    }
  }
}

/**
 * Now and then something walks the field edge beside a road, up and back: the one thing on a
 * road that might come at her by day. Never on the first walk, never at a town's gate.
 */
function wanderers(c: Ctx): void {
  const k = c.k;
  for (let n = 0; n < roadLines(c); n++) {
    if (c.firstLines.has(n)) continue;
    const line = c.lines[n];
    for (let i = 60; i < line.length - 60; i += 340 + k.roll(200)) {
      const [x, y] = line[i];
      if (nearChunk(c, x, y, 40) || dist(c.dFirst, x, y) < FIRST_CLEAR + 20) continue;
      const m = macroOf(x, y);
      const table = WILDLIFE[c.sk.region[m] as Region].filter((w) => !w.biomes || w.biomes.includes(c.sk.biome[m] as Biome));
      if (table.length === 0) continue;
      const def = k.pick(table).def;
      const side = k.chance(0.5) ? 1 : -1;
      const [nx, ny] = normal(line, i);
      const a = line[Math.max(0, i - 20)];
      const b = line[Math.min(line.length - 1, i + 20)];
      const off = 12;
      const p0: [number, number] = [Math.round(a[0] + nx * off * side), Math.round(a[1] + ny * off * side)];
      const p1: [number, number] = [Math.round(b[0] + nx * off * side), Math.round(b[1] + ny * off * side)];
      if (k.solid(p0[0], p0[1]) || k.solid(p1[0], p1[1])) continue;
      hostile(c, def, p0[0], p0[1], [[p0[0], p0[1], 240], [p1[0], p1[1], 240]]);
    }
  }
}

/**
 * The last pass: any screen of open land that still has nothing on it gets something small
 * of its own. By a road, flowers or a fallen log; out in the fields, a thing that lives there.
 */
function gaps(c: Ctx): void {
  const k = c.k;
  const sk = c.sk;
  const gw = Math.floor(k.w / SCREEN_W);
  const gh = Math.floor(k.h / SCREEN_H);
  const has = new Uint8Array(gw * gh);
  const mark = (x: number, y: number): void => {
    const sx = Math.floor(x / SCREEN_W);
    const sy = Math.floor(y / SCREEN_H);
    if (sx < gw && sy < gh) has[sy * gw + sx] = 1;
  };
  for (const p of k.props) if (p.def !== "herb" && p.def !== "rock" && p.def !== "lamp_post" && p.def !== "lamp_run") mark(p.cx, p.cy);
  for (const u of k.units) mark(u.cx, u.cy);
  for (let sy = 0; sy < gh; sy++) {
    for (let sx = 0; sx < gw; sx++) {
      if (has[sy * gw + sx]) continue;
      const r = { cx: sx * SCREEN_W + 4, cy: sy * SCREEN_H + 3, w: SCREEN_W - 8, h: SCREEN_H - 6 };
      let spot = k.spot(r, 3, 3, 1, 30);
      if (!spot) {
        // A screen of thick wood or scrub: open a small clearing in it, off anything claimed.
        for (let tries = 0; tries < 40 && !spot; tries++) {
          const x = k.int(r.cx, r.cx + r.w - 4);
          const y = k.int(r.cy, r.cy + r.h - 4);
          if (room(c, x - 1, y - 1, 5, 5)) {
            clear(c, x - 1, y - 1, 5, 5, Tile.GrassTall);
            spot = { cx: x, cy: y };
          }
        }
      }
      if (!spot) continue;
      const x = spot.cx + 1;
      const y = spot.cy + 1;
      if (nearChunk(c, x, y, 2)) continue;
      const m = macroOf(x, y);
      const region = sk.region[m] as Region;
      // Gentle ground: beside a road, in a haven, or within sight of the first walk.
      const gentle = dist(c.dRoad, x, y) < ROAD_CLEAR || sk.threat[m] === 0 || dist(c.dFirst, x, y) < FIRST_CLEAR + 8;
      const roll = k.roll(3);
      if (region === Region.Works) {
        put(c, ["sleepers", "gravestone", "bones", "boulder"][roll], x, y);
      } else if (gentle) {
        if (roll === 0) put(c, "log", x, y);
        else if (roll === 1) put(c, "boulder", x, y);
        else if (roll === 2) {
          put(c, "stump", x, y);
          put(c, "flowers", x + 1, y + 1);
        } else for (let n = 0; n < 4; n++) put(c, "flowers", x - 2 + k.roll(4), y - 1 + k.roll(2));
      } else if (roll === 0) {
        for (let n = 0; n < 2; n++) folk(c, "rabbit", x + n * 2, y, 4);
      } else if (roll === 1) {
        if (!hostile(c, region === Region.Lowfields ? "crow" : "rat", x, y)) put(c, "boulder", x, y);
      } else if (roll === 2) {
        put(c, "boulder", x, y);
      } else {
        put(c, "stump", x, y);
        put(c, "log", x + 2, y + 1);
      }
    }
  }
}
