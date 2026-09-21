// The county: 3600 x 2000 cells, 3.6 km by 2 km, ten minutes across by road.
// (2020's room_zone1 was 640 x 384. PLAN.md 2.1 is why this one is not.)
//
// It is built FROM the skeleton (world/skeleton): that decided, on a coarse grid,
// where the river runs, where the story's places stand, which roads join them, which
// lamps still work and how dangerous each patch is. This file turns that into cells:
//
//   1 land and water   every cell, from the skeleton's biome and water, edges softened
//   2 roads            the skeleton's routes as strokes; a road over water is a bridge
//   3 set chunks       the authored places, stamped where the skeleton put them
//   4 links            every road joined to a gate of every chunk it meets, going round
//   5 dressing         lamps where the skeleton lit them, small places, herbs, rocks
//   6 wildlife         by region and by threat; nothing on the first walk, nothing in a haven
//
// Same seed, same county. The story's names (dog, house_door, stoop, mine_mouth...)
// are the contract; every coordinate is the seed's.

import { F_SOLID, Tile, TILE_FLAGS } from "@/sim/grid";
import { hashString } from "@/sim/rng";
import type { Blueprint, Rect } from "@/world/blueprint";
import { CHUNKS, type Chunk, type Gate } from "@/world/chunks";
import { Kit } from "@/world/kit";
import { at, Biome, buildSkeleton, COUNTY_H, COUNTY_W, MACRO, Region, ROAD_LIT, SKEL_H, SKEL_W, type Skeleton } from "@/world/skeleton";

export { COUNTY_H, COUNTY_W };

const ROAD_WIDTH = 4;
const FIELD_HERBS = ["pansy", "nasturtium", "honeylace_lily", "hemshade_root"];
const WATER_HERBS = ["white_water_cap", "white_water_rose", "honeylace_lily", "night_lich_moss"];
const WORKS_HERBS = ["night_lich_moss", "savage_snakeroot", "hemshade_root"];

/** What lives where. One row per creature; `phase` (the threat of the ground it stands on) sets what it costs. */
const WILDLIFE: Record<Region, { def: string; biomes?: Biome[] }[]> = {
  [Region.Lowfields]: [
    { def: "rat" },
    { def: "rat" },
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
    { def: "bat" },
    { def: "skeleton" },
  ],
  [Region.Works]: [{ def: "soldier" }, { def: "skeleton_guard" }, { def: "skeleton_clerk", biomes: [Biome.Yard] }, { def: "wall_spider", biomes: [Biome.Hill, Biome.Slag] }, { def: "cactus", biomes: [Biome.Slag] }],
};

// One skeleton per seed is plenty to remember: a county is asked for once a game, and again only when validation re-rolls it.
let lastSkeleton: { key: string; list: Skeleton[] } | null = null;

/** The `attempt`-th valid skeleton of a seed. Attempt 0 is the county the seed viewer shows. */
export function countySkeleton(seed: number, attempt: number): Skeleton {
  const key = String(seed >>> 0);
  if (!lastSkeleton || lastSkeleton.key !== key) lastSkeleton = { key, list: [] };
  const list = lastSkeleton.list;
  while (list.length <= attempt) {
    const from = list.length === 0 ? 0 : list[list.length - 1].attempt + 1;
    list.push(buildSkeleton(seed, undefined, from));
  }
  return list[attempt];
}

export function buildCounty(seed: number, attempt: number): Blueprint {
  const sk = countySkeleton(seed, attempt);
  const k = new Kit("county", COUNTY_W, COUNTY_H, seed, attempt, Tile.Grass);
  paintLand(k, sk, seed);

  // Tree line round the edge: the county is a bowl, not a plane.
  k.fill(0, 0, COUNTY_W, 4, Tile.Tree);
  k.fill(0, COUNTY_H - 4, COUNTY_W, 4, Tile.Tree);
  k.fill(0, 0, 4, COUNTY_H, Tile.Tree);
  k.fill(COUNTY_W - 4, 0, 4, COUNTY_H, Tile.Tree);

  // --- 2 roads ------------------------------------------------------------------
  const centre = (m: number): number => m * MACRO + MACRO / 2;
  const lines: [number, number][][] = [];
  const lit: boolean[][] = [];
  for (const r of sk.roads) {
    const pts = r.cells.map((c): [number, number] => [centre(c % SKEL_W), centre(Math.floor(c / SKEL_W))]);
    const line = k.stroke(pts, ROAD_WIDTH, Tile.Road, 1);
    lines.push(line);
    lit.push(line.map(([x, y]) => (sk.road[at(Math.min(SKEL_W - 1, x >> 4), Math.min(SKEL_H - 1, y >> 4))] & ROAD_LIT) !== 0));
  }
  // The Burial Chamber is off the road on purpose. A footpath from the graveyard finds it, and only that.
  const grave = sk.sites.find((s) => s.id === "graveyard");
  const burial = sk.sites.find((s) => s.id === "burial");
  if (grave && burial) {
    lines.push(k.stroke([[centre(grave.mx), centre(grave.my)], [centre(burial.mx), centre(burial.my)]], 2, Tile.Dirt, 2));
    lit.push([]);
  }

  // --- 3 set chunks, 4 links ------------------------------------------------------
  const chunks: Chunk[] = [];
  for (const s of sk.sites) {
    const build = CHUNKS[s.id];
    if (build) chunks.push(build(k, centre(s.mx), centre(s.my)));
  }
  for (const line of lines) for (const c of chunks) linkRoad(k, line, c);
  for (const c of chunks) k.claim(c.box.cx - 6, c.box.cy - 6, c.box.w + 12, c.box.h + 12);
  for (const line of lines) for (const [x, y] of line) k.claim(x - 3, y - 3, 7, 7);

  // --- 5 dressing -------------------------------------------------------------------
  lines.forEach((line, n) => {
    for (let i = 20; i < line.length - 10; i += 26) {
      if (!lit[n][i]) continue;
      const [x, y] = line[i];
      const side = i % 52 === 20 ? -4 : 4;
      if (!k.solid(x, y + side) && k.get(x, y + side) !== Tile.Water && k.get(x, y + side) !== Tile.Road) k.prop({ def: "lamp_post", cx: x, cy: y + side }, 1, 1);
    }
  });
  sk.pois.forEach((p, n) => smallPlace(k, p.kind, centre(p.mx), centre(p.my), n));
  scatter(k, sk);

  // --- 6 wildlife ---------------------------------------------------------------------
  const safe = new Uint8Array(SKEL_W * SKEL_H);
  for (const r of sk.roads) {
    if (!((r.from === "station" && r.to === "julie_house") || (r.from === "julie_house" && r.to === "town"))) continue;
    for (const c of r.cells) {
      const cx = c % SKEL_W;
      const cy = Math.floor(c / SKEL_W);
      for (let oy = -3; oy <= 3; oy++) for (let ox = -3; ox <= 3; ox++) if (cx + ox >= 0 && cy + oy >= 0 && cx + ox < SKEL_W && cy + oy < SKEL_H) safe[at(cx + ox, cy + oy)] = 1;
    }
  }
  for (let my = 1; my < SKEL_H - 1; my++) {
    for (let mx = 1; mx < SKEL_W - 1; mx++) {
      const i = at(mx, my);
      const threat = sk.threat[i];
      if (threat === 0 || safe[i] || sk.water[i]) continue;
      if (!k.chance((0.014 + threat * 0.007) * (sk.road[i] ? 0.35 : 1))) continue;
      const table = WILDLIFE[sk.region[i] as Region].filter((w) => !w.biomes || w.biomes.includes(sk.biome[i] as Biome));
      if (table.length === 0) continue;
      const spot = k.spot({ cx: mx * MACRO, cy: my * MACRO, w: MACRO, h: MACRO }, 1, 1, 1, 8);
      if (!spot) continue;
      k.unit(null, k.pick(table).def, spot.cx, spot.cy).phase = threat;
    }
  }

  dropUnreachable(k);
  return k.done("Castle", false, 1, attempt);
}

/**
 * A small place on an island of cliff or in a ring of water is not a place. Flood from the
 * platform, the way the validator will, and forget any `poi_` mark the flood never reached.
 * Story marks are left for the validator to judge: if one of those is cut off the county is
 * wrong and must be re-rolled, not quietly trimmed.
 */
function dropUnreachable(k: Kit): void {
  const start = k.marks.start;
  if (!start) return;
  const w = k.w;
  const seen = new Uint8Array(w * k.h);
  const queue = new Int32Array(w * k.h);
  let head = 0;
  let tail = 0;
  // Locals, for the same reason as the painter's: this runs once per cell of the county.
  const flags = TILE_FLAGS;
  const solid = F_SOLID;
  const tiles = k.tiles;
  const push = (i: number): void => {
    if (seen[i] || (flags[tiles[i]] & solid) !== 0) return;
    seen[i] = 1;
    queue[tail++] = i;
  };
  push(start.cy * w + start.cx);
  while (head < tail) {
    const i = queue[head++];
    const x = i % w;
    if (x + 1 < w) push(i + 1);
    if (x > 0) push(i - 1);
    if (i + w < seen.length) push(i + w);
    if (i >= w) push(i - w);
  }
  for (const name of Object.keys(k.marks)) {
    const m = k.marks[name];
    if (name.startsWith("poi_") && !seen[m.cy * w + m.cx]) delete k.marks[name];
  }
  // The same goes for anything alive: a creature walled in where nobody can come is only a cost.
  for (let n = k.units.length - 1; n >= 0; n--) {
    const u = k.units[n];
    if (u.phase !== undefined && !seen[u.cy * w + u.cx]) k.units.splice(n, 1);
  }
}

// Imported names are copied into locals for the painter below. It touches them dozens of times a
// cell, seven million cells; through a module binding (and the test runner wraps every one in
// a getter) that is most of the build. Plain constants cost nothing.
const T = { ...Tile } as typeof Tile;
const B = {
  Field: Biome.Field,
  Hedge: Biome.Hedge,
  Wood: Biome.Wood,
  Foothill: Biome.Foothill,
  Reed: Biome.Reed,
  Marsh: Biome.Marsh,
  WetWood: Biome.WetWood,
  Garden: Biome.Garden,
  Slag: Biome.Slag,
  Yard: Biome.Yard,
  Hill: Biome.Hill,
} as const;
const M = MACRO;
const SW = SKEL_W;
const SH = SKEL_H;
const CW = COUNTY_W;

// --- land ----------------------------------------------------------------------------

function hash01(seed: number, x: number, y: number): number {
  let h = (seed ^ Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** A lattice of hashed values; `sample` reads it smoothly. Coarse noise for seven million cells without a noise call per cell. */
function lattice(seed: number, w: number, h: number): Float32Array {
  const out = new Float32Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) out[y * w + x] = hash01(seed, x, y);
  return out;
}

/**
 * Every cell's ground. The skeleton's macro cells are 16 m squares; looked up through a
 * smooth warp they stop being squares, and water is read as a smooth field so the river
 * has banks instead of steps.
 *
 * Seven million cells, so it is written block by block: inside one macro block the three
 * smooth fields (warp x, warp y, clumps) are straight lines along a row, which turns three
 * bilinear samples a cell into three additions.
 */
function paintLand(k: Kit, sk: Skeleton, seed: number): void {
  const s = (seed ^ hashString("county-land")) >>> 0;
  const LW = SW + 1;
  const warpX = lattice(s + 1, LW, SH + 1);
  const warpY = lattice(s + 2, LW, SH + 1);
  const clumps = lattice(s + 3, LW, SH + 1);
  const wet = new Float32Array(SW * SH);
  // Only blocks with water within two macro cells need the water field at all.
  const nearWater = new Uint8Array(SW * SH);
  for (let my = 0; my < SH; my++) {
    for (let mx = 0; mx < SW; mx++) {
      if (!sk.water[at(mx, my)]) continue;
      wet[at(mx, my)] = 1;
      for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) if (mx + ox >= 0 && my + oy >= 0 && mx + ox < SW && my + oy < SH) nearWater[at(mx + ox, my + oy)] = 1;
    }
  }
  const tiles = k.tiles;
  const biomes = sk.biome;
  const W = 20; // how far the warp may push a lookup, in cells, end to end
  // One macro block at a time, as its own small function: called 28,000 times it is compiled
  // once and stays compiled, where one seven-million-turn loop ran mostly in the slow tier.
  const paintBlock = (mx: number, my: number): void => {
    {
      const n = my * LW + mx;
      const watery = nearWater[my * SW + mx] === 1;
      for (let j = 0; j < M; j++) {
        const ty = j / M;
        const y = my * M + j;
        const xl = warpX[n] + (warpX[n + LW] - warpX[n]) * ty;
        const xs = (warpX[n + 1] + (warpX[n + LW + 1] - warpX[n + 1]) * ty - xl) / M;
        const yl = warpY[n] + (warpY[n + LW] - warpY[n]) * ty;
        const ys = (warpY[n + 1] + (warpY[n + LW + 1] - warpY[n + 1]) * ty - yl) / M;
        const cl = clumps[n] + (clumps[n + LW] - clumps[n]) * ty;
        const cs = (clumps[n + 1] + (clumps[n + LW + 1] - clumps[n + 1]) * ty - cl) / M;
        let row = y * CW + mx * M;
        for (let i = 0; i < M; i++, row++) {
          const x = mx * M + i;
          let u = (x + (xl + xs * i - 0.5) * W) / M - 0.5;
          let v = (y + (yl + ys * i - 0.5) * W) / M - 0.5;
          if (u < 0) u = 0;
          else if (u > SW - 1) u = SW - 1;
          if (v < 0) v = 0;
          else if (v > SH - 1) v = SH - 1;
          if (watery) {
            const u0 = u >= SW - 1 ? SW - 2 : u | 0;
            const v0 = v >= SH - 1 ? SH - 2 : v | 0;
            const tx = u - u0;
            const tv = v - v0;
            const wi = v0 * SW + u0;
            const water = wet[wi] * (1 - tx) * (1 - tv) + wet[wi + 1] * tx * (1 - tv) + wet[wi + SW] * (1 - tx) * tv + wet[wi + SW + 1] * tx * tv;
            if (water >= 0.4) {
              tiles[row] = water >= 0.5 ? T.Water : T.Sand;
              continue;
            }
          }
          const r = hash01(s, x, y);
          const biome = biomes[((v + 0.5) | 0) * SW + ((u + 0.5) | 0)] as Biome;
          // A little of the per-cell value on the smooth one, so thickets have ragged edges.
          tiles[row] = ground(biome, cl + cs * i + (r - 0.5) * 0.12, r, x, y, s);
        }
      }
    }
  };
  for (let my = 0; my < SH; my++) for (let mx = 0; mx < SW; mx++) paintBlock(mx, my);
}

/** `clump` is smooth (thickets, outcrops, pools), `r` is per cell (the odd tree, the long grass). */
function ground(biome: Biome, clump: number, r: number, x: number, y: number, s: number): Tile {
  switch (biome) {
    case B.Field:
      return r < 0.004 ? T.Tree : r < 0.012 ? T.Bush : r < 0.1 ? T.GrassTall : T.Grass;
    case B.Hedge: {
      // Hedged fields: lines of bush on a loose grid, with gaps where a gate once was.
      const line = x % 34 === 0 || y % 30 === 0;
      if (line && hash01(s + 7, Math.floor(x / 9), Math.floor(y / 9)) > 0.3) return T.Bush;
      return r < 0.006 ? T.Tree : r < 0.16 ? T.GrassTall : T.Grass;
    }
    case B.Wood:
      return clump > 0.66 || r < 0.05 ? T.Tree : r < 0.4 ? T.GrassTall : T.Grass;
    case B.Foothill:
      return clump > 0.74 ? T.Cliff : r < 0.02 ? T.Tree : clump < 0.3 ? T.Dirt : r < 0.2 ? T.GrassTall : T.Grass;
    case B.Reed:
      return clump > 0.78 ? T.Water : r < 0.55 ? T.GrassTall : T.Moss;
    case B.Marsh:
      return clump > 0.76 ? T.Water : clump < 0.22 ? T.DryBed : r < 0.3 ? T.GrassTall : r < 0.34 ? T.Bush : T.Moss;
    case B.WetWood:
      return clump > 0.68 || r < 0.05 ? T.Tree : r < 0.5 ? T.Moss : T.GrassTall;
    case B.Garden:
      return clump > 0.7 ? T.Bush : r < 0.03 ? T.Rubble : r < 0.4 ? T.Garden : T.Grass;
    case B.Slag:
      // Rubble is solid: a scatter of it is cover, a carpet of it is a maze.
      return clump > 0.78 ? T.Cliff : r < 0.05 ? T.Rubble : clump < 0.3 ? T.DryBed : T.Dirt;
    case B.Yard:
      return y % 46 < 2 && hash01(s + 9, Math.floor(x / 40), Math.floor(y / 46)) > 0.5 ? T.Track : r < 0.04 ? T.Rubble : clump > 0.6 ? T.Cobble : T.Dirt;
    case B.Hill:
      return clump > 0.72 ? T.Cliff : r < 0.04 ? T.Rubble : clump < 0.35 ? T.Grass : T.Dirt;
    default:
      return T.Grass;
  }
}

// --- links ---------------------------------------------------------------------------

const RING = 4;

/**
 * Where a road crosses into a chunk's box, the stamp has overwritten it. Join the last
 * cell outside to the chunk's nearest gate by a lane that goes ROUND the box, so it can
 * never cut a fence, a wall or a kitchen.
 */
function linkRoad(k: Kit, line: readonly [number, number][], c: Chunk): void {
  const b = c.box;
  const within = (x: number, y: number): boolean => x >= b.cx - 2 && y >= b.cy - 2 && x < b.cx + b.w + 2 && y < b.cy + b.h + 2;
  let was = line.length > 0 && within(line[0][0], line[0][1]);
  for (let i = 1; i < line.length; i++) {
    const now = within(line[i][0], line[i][1]);
    if (now !== was) connect(k, now ? line[i - 1] : line[i], c);
    was = now;
  }
}

function connect(k: Kit, p: readonly [number, number], c: Chunk): void {
  let gate: Gate = c.gates[0];
  let best = Infinity;
  for (const g of c.gates) {
    const d = Math.abs(g[0] - p[0]) + Math.abs(g[1] - p[1]);
    if (d < best) {
      best = d;
      gate = g;
    }
  }
  const ring: Rect = { cx: Math.max(6, c.box.cx - RING), cy: Math.max(6, c.box.cy - RING), w: 0, h: 0 };
  ring.w = Math.min(k.w - 7, c.box.cx + c.box.w + RING - 1) - ring.cx + 1;
  ring.h = Math.min(k.h - 7, c.box.cy + c.box.h + RING - 1) - ring.cy + 1;
  const path = perimeter(ring);
  const nearest = (q: readonly [number, number]): number => {
    let at = 0;
    let d = Infinity;
    path.forEach(([x, y], n) => {
      const dd = Math.abs(x - q[0]) + Math.abs(y - q[1]);
      if (dd < d) {
        d = dd;
        at = n;
      }
    });
    return at;
  };
  const a = nearest(p);
  const g = nearest(gate);
  const lane = (x: number, y: number): void => k.fill(x - 1, y - 1, 3, 3, Tile.Dirt);
  const forward = (g - a + path.length) % path.length;
  const step = forward <= path.length - forward ? 1 : -1;
  for (let n = a; ; n = (n + step + path.length) % path.length) {
    lane(path[n][0], path[n][1]);
    if (n === g) break;
  }
  straight(p, path[a], lane);
  straight(path[g], gate, lane);
}

function perimeter(r: Rect): [number, number][] {
  const out: [number, number][] = [];
  for (let x = r.cx; x < r.cx + r.w; x++) out.push([x, r.cy]);
  for (let y = r.cy + 1; y < r.cy + r.h; y++) out.push([r.cx + r.w - 1, y]);
  for (let x = r.cx + r.w - 2; x >= r.cx; x--) out.push([x, r.cy + r.h - 1]);
  for (let y = r.cy + r.h - 2; y > r.cy; y--) out.push([r.cx, y]);
  return out;
}

function straight(a: readonly [number, number], b: readonly [number, number], lane: (x: number, y: number) => void): void {
  const steps = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
  for (let n = 0; n <= steps; n++) {
    const t = steps === 0 ? 0 : n / steps;
    lane(Math.round(a[0] + (b[0] - a[0]) * t), Math.round(a[1] + (b[1] - a[1]) * t));
  }
}

// --- dressing --------------------------------------------------------------------------

/**
 * The skeleton's small places, dressed with what the art already has. They are shapes to
 * walk past, and named marks (`poi_<n>`) for the side quests and omens that will be written
 * onto them; nothing here talks yet.
 */
function smallPlace(k: Kit, kind: string, x: number, y: number, n: number): void {
  if (k.isClaimed(x, y) || k.get(x, y) === Tile.Water) return;
  const pad = (w: number, h: number, t: Tile): void => {
    k.fill(x - Math.floor(w / 2), y - Math.floor(h / 2), w, h, t);
  };
  const put = (def: string, ox: number, oy: number, w = 1, h = 1): void => {
    if (k.fits(x + ox, y + oy, w, h)) k.prop({ def, cx: x + ox, cy: y + oy }, w, h);
  };
  switch (kind) {
    case "well":
      pad(5, 5, Tile.Cobble);
      k.set(x, y, Tile.Water);
      break;
    case "shrine":
    case "statue":
    case "scarecrow":
    case "signal":
      pad(5, 5, kind === "scarecrow" ? Tile.Dirt : Tile.Cobble);
      put("pillar", 0, 0);
      break;
    case "stones":
      pad(9, 9, Tile.Dirt);
      for (const [ox, oy] of [[-3, -3], [3, -3], [-4, 1], [4, 1], [0, 4]]) put("pillar", ox, oy);
      break;
    case "cottage":
    case "hut":
      pad(12, 10, Tile.Dirt);
      k.fill(x + 1, y - 1, 3, 2, Tile.Rubble);
      k.fill(x - 4, y - 4, 8, 1, Tile.HouseWall);
      k.fill(x - 4, y - 4, 1, 5, Tile.HouseWall);
      k.fill(x + 3, y - 4, 1, 3, Tile.HouseWall);
      break;
    case "greenhouse":
      pad(11, 9, Tile.Garden);
      k.fill(x - 5, y - 4, 11, 1, Tile.Wall);
      k.fill(x - 5, y - 4, 1, 6, Tile.Wall);
      break;
    case "pump":
    case "pipe_end":
      pad(7, 7, Tile.Dirt);
      k.set(x + 2, y - 2, Tile.Rubble);
      k.fill(x - 1, y - 2, 3, 2, Tile.Wall);
      break;
    case "jetty":
    case "boat":
      pad(3, 9, Tile.Cobble);
      put("crate", 1, 2, 2, 2);
      break;
    case "wagon":
      k.fill(x - 5, y, 11, 1, Tile.Track);
      put("minecart", -1, -2, 2, 2);
      break;
    case "cart":
    case "camp":
      pad(7, 6, Tile.Dirt);
      put("crate", -2, -1, 2, 2);
      put("barrel", 1, 0, 2, 2);
      break;
    case "hollow_tree":
      k.fill(x - 2, y - 2, 5, 4, Tile.Tree);
      k.fill(x - 1, y + 1, 3, 1, Tile.Dirt);
      break;
    default:
      pad(5, 5, Tile.Dirt);
  }
  // Whatever was drawn, the place can be stood at: the mark's own cell and its neighbours are open ground.
  for (let oy = 2; oy <= 4; oy++) for (let ox = -1; ox <= 1; ox++) if (k.solid(x + ox, y + oy)) k.set(x + ox, y + oy, Tile.Dirt);
  k.mark(`poi_${n}`, x, y + 3, 1);
  k.claim(x - 7, y - 6, 15, 13);
}

function scatter(k: Kit, sk: Skeleton): void {
  const open = (x: number, y: number): boolean => {
    const t = k.get(x, y);
    return !k.isClaimed(x, y) && !k.solid(x, y) && t !== Tile.Water && t !== Tile.Road;
  };
  for (let n = 0; n < 700; n++) {
    const x = k.int(10, COUNTY_W - 11);
    const y = k.int(10, COUNTY_H - 11);
    if (!open(x, y)) continue;
    const region = sk.region[at(x >> 4, y >> 4)] as Region;
    const herbs = region === Region.Lowfields ? FIELD_HERBS : region === Region.Waters ? WATER_HERBS : WORKS_HERBS;
    k.prop({ def: "herb", cx: x, cy: y, loot: [{ item: k.pick(herbs), qty: 1 }] }, 1, 1);
  }
  for (let n = 0; n < 260; n++) {
    const x = k.int(10, COUNTY_W - 11);
    const y = k.int(10, COUNTY_H - 11);
    if (open(x, y)) k.prop({ def: "rock", cx: x, cy: y }, 1, 1);
  }
}
