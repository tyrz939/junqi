// Layer 1: the land. Height, the river and its lake, the three regions, biomes.
// Pure function of the seed. No trigonometry: noise is hashed lattice values,
// smoothed, summed in octaves.

import { rngRange, rngSeed, type RngState } from "@/sim/rng";
import { at, Biome, inside, Region, SKEL_H, SKEL_W } from "@/world/skeleton/types";

function hash2(seed: number, x: number, y: number): number {
  let h = (seed ^ Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

const smooth = (t: number): number => t * t * (3 - 2 * t);

/** Value noise in 0..1 at a given lattice period (macro cells). */
function noise(seed: number, x: number, y: number, period: number): number {
  const fx = x / period;
  const fy = y / period;
  const x0 = Math.floor(fx);
  const y0 = Math.floor(fy);
  const tx = smooth(fx - x0);
  const ty = smooth(fy - y0);
  const a = hash2(seed, x0, y0);
  const b = hash2(seed, x0 + 1, y0);
  const c = hash2(seed, x0, y0 + 1);
  const d = hash2(seed, x0 + 1, y0 + 1);
  return a + (b - a) * tx + (c - a) * ty + (a - b - c + d) * tx * ty;
}

export function fbm(seed: number, x: number, y: number, period: number, octaves = 3): number {
  let sum = 0;
  let amp = 1;
  let total = 0;
  for (let o = 0; o < octaves; o++) {
    sum += noise(seed + o * 7919, x, y, Math.max(2, period / (1 << o))) * amp;
    total += amp;
    amp *= 0.5;
  }
  return sum / total;
}

export type Terrain = {
  height: Uint8Array;
  water: Uint8Array;
  region: Uint8Array;
  biome: Uint8Array;
  /** Distance to the nearest water, in macro cells. */
  wet: Float32Array;
  /** Steepest height step to any neighbour. Asked of every cell for every row, so it is worked out once. */
  slope: Uint8Array;
  /** Cost of laying a road across this cell, about 0.7 to 4. */
  rough: Float32Array;
  /** Column of the river at each row: the Waters lie east of it. */
  riverX: Int16Array;
  /** The crown of the hill, where the School goes. */
  crown: { mx: number; my: number };
  lake: { mx: number; my: number; r: number };
};

/**
 * The county's shape is fixed where the story needs it and free everywhere else:
 *   the Works are the north (the hill; the School on its crown, seen from the station),
 *   the river runs north to south somewhere east of the middle, the Waters beyond it,
 *   the Lowfields are the south-west, with foothills along their southern edge.
 * Where the river bends, where the borders wobble, how high the hill is: the seed's.
 */
export function buildTerrain(seed: number, attempt: number): Terrain {
  const rng: RngState = rngSeed(seed, 100 + attempt);
  const n = SKEL_W * SKEL_H;
  const height = new Uint8Array(n);
  const water = new Uint8Array(n);
  const region = new Uint8Array(n);
  const biome = new Uint8Array(n);
  const s = (seed ^ Math.imul(attempt + 1, 0x9e3779b1)) >>> 0;

  // The river: a wandering column, top to bottom, about 60 to 66% of the way across. On the square county
  // (Sept 24) it sits a little further east than it did, because the Lowfields hold the town, the first walk
  // and every story place, and its sideways wander is scaled to the narrower map.
  const riverX = new Int16Array(SKEL_H);
  const riverBase = rngRange(rng, SKEL_W * 0.58, SKEL_W * 0.66);
  const lean = rngRange(rng, -8, 8); // drifts east or west as it goes south
  for (let y = 0; y < SKEL_H; y++) {
    const t = y / (SKEL_H - 1);
    const wander = (fbm(s + 11, 0, y, 38, 2) - 0.5) * 36 + (fbm(s + 12, 0, y, 11, 1) - 0.5) * 8;
    riverX[y] = Math.round(Math.min(SKEL_W * 0.74, Math.max(SKEL_W * 0.5, riverBase + lean * (t - 0.5) + wander)));
  }

  // The hill: one crown in the north, a little west of the river so it stands over the town.
  const crown = { mx: Math.round(rngRange(rng, SKEL_W * 0.3, SKEL_W * 0.48)), my: Math.round(rngRange(rng, 9, 17)) };
  const hillR = rngRange(rng, 38, 48);
  // The Works' southern border, wobbling around 34% of the height; the foothills along the
  // south-west. Both depend on the column only, so they are worked out once per column.
  const worksEdges = new Float32Array(SKEL_W);
  const footEdges = new Float32Array(SKEL_W);
  for (let x = 0; x < SKEL_W; x++) {
    worksEdges[x] = SKEL_H * 0.34 + (fbm(s + 21, x, 0, 44, 3) - 0.5) * 44;
    footEdges[x] = SKEL_H * 0.8 + (fbm(s + 31, x, 0, 30, 3) - 0.5) * 34;
  }
  const worksEdge = (x: number): number => worksEdges[x];
  const footEdge = (x: number): number => footEdges[x];

  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const i = at(x, y);
      let h = 60 + fbm(s + 1, x, y, 48, 4) * 50;
      const dx = x - crown.mx;
      const dy = (y - crown.my) * 1.25;
      const d = Math.sqrt(dx * dx + dy * dy);
      if (d < hillR) h += smooth(1 - d / hillR) * 120;
      const foot = y - footEdge(x);
      if (foot > 0 && x < riverX[y] - 6) h += Math.min(70, foot * 5);
      const river = Math.abs(x - riverX[y]);
      if (river < 14) h -= smooth(1 - river / 14) * 38; // the valley
      height[i] = Math.max(0, Math.min(255, Math.round(h)));
      region[i] = y < worksEdge(x) ? Region.Works : x > riverX[y] ? Region.Waters : Region.Lowfields;
    }
  }

  // Water: the river (two or three cells wide, wider in the south), then the lake in the Waters.
  for (let y = 0; y < SKEL_H; y++) {
    const half = y > SKEL_H * 0.55 ? 1 : 0;
    for (let x = riverX[y] - 1; x <= riverX[y] + half; x++) if (inside(x, y)) water[at(x, y)] = 1;
    // Keep the river continuous where the column jumps sideways between rows.
    if (y > 0) {
      const a = Math.min(riverX[y - 1], riverX[y]);
      const b = Math.max(riverX[y - 1], riverX[y]);
      for (let x = a; x <= b; x++) water[at(x, y)] = 1;
    }
  }
  const lake = {
    // Clear of the east fence by its own width: on a square county 92% of the width put its far shore off the map.
    mx: Math.round(rngRange(rng, SKEL_W * 0.78, SKEL_W * 0.86)),
    my: Math.round(rngRange(rng, SKEL_H * 0.5, SKEL_H * 0.78)),
    r: Math.round(rngRange(rng, 7, 10)),
  };
  for (let y = lake.my - lake.r - 3; y <= lake.my + lake.r + 3; y++) {
    for (let x = lake.mx - lake.r - 3; x <= lake.mx + lake.r + 3; x++) {
      if (!inside(x, y)) continue;
      const dx = x - lake.mx;
      const dy = (y - lake.my) * 1.3;
      const edge = lake.r + (fbm(s + 41, x, y, 6, 2) - 0.5) * 5;
      // 2 = the lake. A road may bridge the river (1), dearly; nothing crosses the lake.
      if (Math.sqrt(dx * dx + dy * dy) < edge) water[at(x, y)] = 2;
    }
  }

  // How wet the ground is: distance to water, by two passes of a chamfer sweep.
  const wet = new Float32Array(n).fill(999);
  for (let i = 0; i < n; i++) if (water[i]) wet[i] = 0;
  for (let y = 0; y < SKEL_H; y++) for (let x = 0; x < SKEL_W; x++) relax(wet, x, y, -1);
  for (let y = SKEL_H - 1; y >= 0; y--) for (let x = SKEL_W - 1; x >= 0; x--) relax(wet, x, y, 1);

  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const i = at(x, y);
      const patch = fbm(s + 51, x, y, 14, 3);
      const big = fbm(s + 52, x, y, 34, 2);
      const r = region[i] as Region;
      if (r === Region.Lowfields) {
        const foot = y - footEdge(x);
        biome[i] = foot > 0 ? (patch > 0.62 ? Biome.Wood : Biome.Foothill) : big > 0.6 ? Biome.Wood : patch > 0.52 ? Biome.Hedge : Biome.Field;
      } else if (r === Region.Waters) {
        biome[i] = wet[i] < 3 ? Biome.Reed : wet[i] < 7 && patch > 0.45 ? Biome.Marsh : big > 0.58 ? Biome.WetWood : patch > 0.6 ? Biome.Garden : Biome.Marsh;
      } else {
        biome[i] = height[i] > 150 ? Biome.Hill : big > 0.55 ? Biome.Yard : Biome.Slag;
      }
    }
  }
  // How hard the ground is to put a road across. Without this every road is a ruler line with
  // one right-angle bend; with it they find their way round woods and wet ground, and wander
  // about a quarter further than the crow, which is what the county's size was worked out from.
  const rough = new Float32Array(n);
  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const i = at(x, y);
      const b = biome[i] as Biome;
      const ground = b === Biome.WetWood ? 1.2 : b === Biome.Reed ? 1.0 : b === Biome.Wood ? 0.9 : b === Biome.Marsh ? 0.7 : b === Biome.Hill ? 0.5 : 0;
      rough[i] = 0.7 + fbm(s + 61, x, y, 9, 2) * 2.6 + ground;
    }
  }

  const slope = new Uint8Array(n);
  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const h = height[at(x, y)];
      let worst = 0;
      for (let oy = -1; oy <= 1; oy++) {
        for (let ox = -1; ox <= 1; ox++) {
          if (inside(x + ox, y + oy)) worst = Math.max(worst, Math.abs(height[at(x + ox, y + oy)] - h));
        }
      }
      slope[at(x, y)] = worst;
    }
  }
  return { height, water, region, biome, wet, slope, rough, riverX, crown, lake };
}

/** One cell of a chamfer sweep. Unrolled: it runs for every cell of the map, twice, and must not allocate. */
function relax(wet: Float32Array, x: number, y: number, dir: 1 | -1): void {
  let best = wet[at(x, y)];
  if (inside(x + dir, y)) best = Math.min(best, wet[at(x + dir, y)] + 1);
  if (inside(x, y + dir)) best = Math.min(best, wet[at(x, y + dir)] + 1);
  if (inside(x + dir, y + dir)) best = Math.min(best, wet[at(x + dir, y + dir)] + 1.4);
  if (inside(x - dir, y + dir)) best = Math.min(best, wet[at(x - dir, y + dir)] + 1.4);
  wet[at(x, y)] = best;
}
