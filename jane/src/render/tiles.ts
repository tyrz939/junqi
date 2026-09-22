// Terrain. Tiles are painted procedurally into 16x16-cell chunk canvases, built on demand, kept in a
// small LRU, and dropped when far away: "one render-texture will not survive a 5120x3072 county"
// (WORLDGEN.md). A tile change invalidates only the chunks its rect touches.
//
// A chunk is painted in layers, the way a 2D Zelda map is built:
//   1. GROUND. Every walkable material (grass, earth, road, water, marsh...) is a group. A cell's
//      group fills it, except at a convex corner, where a 4 px chamfer is taken by the neighbour.
//      Two chamfers meet at the middle of every 8 px step, so a road laid on a diagonal has a
//      straight 45 degree edge instead of a staircase. Things that stand ON the ground (a tree, a
//      bush, a stone, a fence, long grass) take the ground of their neighbours, so a wood is grass
//      under its trees and a reed bed is marsh with reeds in it, not a patchwork of squares.
//   2. DETAIL. Calm: a tuft here, a pebble there, a flower once in a while.
//   3. EDGES. From a per-pixel material map: higher ground (grass) gets a dark rim where it drops
//      to lower ground (a path), the lower ground is shaded under it, water gets a dark band under
//      its bank and a pale lip everywhere else. Built walls and cliffs throw a shadow south.
//   4. HARD tiles (walls, cliffs, roofs, floors) are drawn per cell by their own painters.
//   5. STANDING things (trees, bushes, stones, fences, low walls) go into per-row strips that the
//      renderer y-sorts with units and props: the sim cell is one cell, the drawing is Zelda-sized.

import { CELL } from "@/sim/constants";
import { F_SOLID, Tile, TILE_COUNT, TILE_FLAGS, type Grid } from "@/sim/grid";
import { boulder, broadleaf, bush, BUSHES, deadTree, LEAVES, pine, rocks, type Spr } from "@/render/flora";

export const CHUNK_CELLS = 16;
export const CHUNK_PX = CHUNK_CELLS * CELL;
const MAX_CHUNKS = 96;

/** Strip canvases: a margin each side for crowns that overhang the chunk, and a row tall enough for the tallest tree. */
const STRIP_MX = 16;
export const STRIP_H = 44;
/** How far below its row's cell bottom a standing sprite may reach (a bush's foot). */
const STRIP_BELOW = 2;

// --- ground groups ------------------------------------------------------------------------------------

const G_NONE = 0;
const G_GRASS = 1;
const G_DIRT = 2;
const G_ROAD = 3;
const G_SAND = 4;
const G_WATER = 5;
const G_MOSS = 6;
const G_DRY = 7;
const G_GARDEN = 8;
const G_COBBLE = 9;
const G_ICE = 10;
const G_CLIFF = 11; // the top of a cliff: ground, but the highest there is
const G_COUNT = 12;

/** Ground group of each tile that IS ground. 0: a hard tile with its own painter, or a thing standing on ground. */
const GROUP = new Uint8Array(TILE_COUNT);
GROUP[Tile.Grass] = G_GRASS;
GROUP[Tile.GrownPath] = G_GRASS;
GROUP[Tile.Dirt] = G_DIRT;
GROUP[Tile.Road] = G_ROAD;
GROUP[Tile.Sand] = G_SAND;
GROUP[Tile.Water] = G_WATER;
GROUP[Tile.Moss] = G_MOSS;
GROUP[Tile.DryBed] = G_DRY;
GROUP[Tile.Garden] = G_GARDEN;
GROUP[Tile.Crops] = G_GARDEN;
GROUP[Tile.Cobble] = G_COBBLE;
GROUP[Tile.Ice] = G_ICE;

/** Things that stand on the ground: they take the ground of a neighbour, else this default. */
const INHERIT = new Uint8Array(TILE_COUNT);
for (const t of [Tile.GrassTall, Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Bush, Tile.Fence, Tile.StoneWall, Tile.FlowerBed, Tile.Stepping]) {
  INHERIT[t] = Tile.Grass;
}
INHERIT[Tile.Rubble] = Tile.Dirt;

/** Wild ground, which the speckle filter may redraw as its surroundings. */
const WILD = new Uint8Array(G_COUNT);
for (const g of [G_GRASS, G_MOSS, G_DRY, G_SAND, G_GARDEN, G_DIRT]) WILD[g] = 1;

/** Height, for edges: grass stands over marsh, marsh over earth, earth over the worn road, everything over water. */
const HEIGHT = Uint8Array.from({ length: G_COUNT }, (_, g) =>
  g === G_CLIFF ? 8 : g === G_GRASS ? 6 : g === G_MOSS ? 4 : g === G_WATER || g === G_ICE ? 0 : g === G_ROAD ? 1 : 2,
);

type Tone = { base: string; a: string; b: string; rim: string };
const GT: Tone[] = [];
GT[G_NONE] = { base: "#000", a: "#000", b: "#000", rim: "#000" };
GT[G_GRASS] = { base: "#4f9a4a", a: "#68b058", b: "#3f8440", rim: "#357038" };
GT[G_DIRT] = { base: "#9a7a52", a: "#ac8c62", b: "#84683f", rim: "#7a5e3c" };
GT[G_ROAD] = { base: "#bca884", a: "#cdbb98", b: "#a08e6c", rim: "#8e7c5c" };
GT[G_SAND] = { base: "#d8c890", a: "#e6d8a6", b: "#c0b07a", rim: "#b0a070" };
GT[G_WATER] = { base: "#3a74bc", a: "#5a92d6", b: "#2f62a4", rim: "#244e88" };
GT[G_MOSS] = { base: "#5a7250", a: "#6c8658", b: "#4a6044", rim: "#3f5439" };
GT[G_DRY] = { base: "#7e7362", a: "#8c826e", b: "#665c4e", rim: "#5c5446" };
GT[G_GARDEN] = { base: "#6a5238", a: "#7c6244", b: "#523e2a", rim: "#4a3826" };
GT[G_COBBLE] = { base: "#5c5c62", a: "#9a9aa0", b: "#5a5a60", rim: "#4a4a50" };
GT[G_ICE] = { base: "#b4dcea", a: "#e4f6fa", b: "#8cbcd0", rim: "#7aa8bc" };
GT[G_CLIFF] = { base: "#8a765c", a: "#a08a6c", b: "#6e5c46", rim: "#2e241a" };

type Swatch = { base: string; a: string; b: string };
/** Hard tiles and their tones. */
const SW: Partial<Record<Tile, Swatch>> = {
  [Tile.Void]: { base: "#0b0a10", a: "#0b0a10", b: "#0b0a10" },
  [Tile.Hedge]: { base: "#2a5a3c", a: "#3f7a4c", b: "#1c3e2a" },
  [Tile.HouseWall]: { base: "#dccdad", a: "#ebe0c6", b: "#ad9d7c" },
  [Tile.BrickWall]: { base: "#9c5a44", a: "#b46c52", b: "#6e3c2e" },
  [Tile.HouseRoof]: { base: "#a84838", a: "#c46050", b: "#7a3026" },
  [Tile.RoofSlate]: { base: "#56627a", a: "#6e7c96", b: "#3a4458" },
  [Tile.RoofThatch]: { base: "#b89454", a: "#d0ac68", b: "#8a6c3a" },
  [Tile.Floor]: { base: "#8c8478", a: "#989084", b: "#7c7468" },
  [Tile.FloorWood]: { base: "#b08858", a: "#bc9464", b: "#96703f" },
  [Tile.Wall]: { base: "#3a3644", a: "#5c5668", b: "#2a2632" },
  [Tile.WallTop]: { base: "#2a2632", a: "#3a3644", b: "#1e1b26" },
  [Tile.CaveFloor]: { base: "#6a5a48", a: "#766654", b: "#5a4c3c" },
  [Tile.CaveWall]: { base: "#2c2420", a: "#4a3c32", b: "#1e1814" },
  [Tile.TempleFloor]: { base: "#5a6470", a: "#66707c", b: "#4c5660" },
  [Tile.TempleWall]: { base: "#232a36", a: "#3c4656", b: "#181d26" },
  [Tile.Track]: { base: "#6a5a48", a: "#8a8f98", b: "#4a3626" },
  [Tile.Rail]: { base: "#9a7a52", a: "#8a8f98", b: "#4a3626" },
  [Tile.Cliff]: { base: "#7e6c56", a: "#957f66", b: "#65553f" },
  [Tile.Sill]: { base: "#5e5040", a: "#8a7a68", b: "#3e3428" },
  [Tile.Glass]: { base: "#6f8f96", a: "#c8e4e8", b: "#4a666e" },
  [Tile.MuseumFloor]: { base: "#9a8e7c", a: "#ab9f8c", b: "#7e7464" },
  [Tile.MuseumWall]: { base: "#4a3e44", a: "#6a5a60", b: "#30282e" },
  [Tile.PipeFloor]: { base: "#565e5c", a: "#687270", b: "#3e4644" },
  [Tile.PipeWall]: { base: "#2c3436", a: "#48565a", b: "#1c2224" },
  [Tile.WorksFloor]: { base: "#6c6660", a: "#7e7870", b: "#524c48" },
  [Tile.WorksWall]: { base: "#3a3230", a: "#5a4c46", b: "#261f1e" },
  [Tile.SchoolFloor]: { base: "#8a6a4a", a: "#9c7c58", b: "#6a4e34" },
  [Tile.SchoolWall]: { base: "#3e4a44", a: "#5a6a60", b: "#28322e" },
  [Tile.Boardwalk]: { base: "#9a7448", a: "#b48a58", b: "#6a4c2e" },
};

/** One flat colour per tile id, for the minimap. */
export const TILE_COLORS: string[] = Array.from({ length: TILE_COUNT }, (_, t) => {
  if (GROUP[t]) return GT[GROUP[t]].base;
  if (t === Tile.Tree || t === Tile.Pine) return "#2d6a39";
  if (t === Tile.DeadTree) return "#5e5650";
  if (t === Tile.Bush) return "#347a3b";
  if (t === Tile.GrassTall || t === Tile.FlowerBed || t === Tile.Stepping) return "#4a9446";
  if (t === Tile.Fence) return "#8a5e36";
  if (t === Tile.StoneWall || t === Tile.Rubble) return "#6c6860";
  return SW[t as Tile]?.base ?? "#000";
});

/** Stable per-cell hash, 0..255. Drawing must not depend on draw order or time. */
function cellHash(x: number, y: number): number {
  let h = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1);
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b);
  return (h ^ (h >>> 13)) & 255;
}

/** Smooth-ish noise 0..255 over blocks of 2^shift cells, for choosing a wood's character by area. */
function areaHash(x: number, y: number, shift: number): number {
  return cellHash((x >> shift) + 7919, (y >> shift) - 104729);
}

const WALL_LIKE = new Uint8Array(TILE_COUNT);
for (const t of [
  Tile.Wall,
  Tile.CaveWall,
  Tile.TempleWall,
  Tile.Cliff,
  Tile.WallTop,
  Tile.Void,
  Tile.MuseumWall,
  Tile.PipeWall,
  Tile.WorksWall,
  Tile.SchoolWall,
]) {
  WALL_LIKE[t] = 1;
}
const isRoof = (t: number): boolean => t === Tile.HouseRoof || t === Tile.RoofSlate || t === Tile.RoofThatch;
const isHouseWall = (t: number): boolean => t === Tile.HouseWall || t === Tile.BrickWall;
const isTree = (t: number): boolean => t === Tile.Tree || t === Tile.Pine;

/** A tile that throws a shadow on the ground south of it. */
const RAISED = Uint8Array.from({ length: TILE_COUNT }, (_, t) => (WALL_LIKE[t] === 1 || isRoof(t) || isHouseWall(t) || t === Tile.Hedge) && t !== Tile.Void ? 1 : 0);

const SHADE = "#00000026";
const SCUFF = "#00000018";

/**
 * Small char grids for the things that grow. "a" is the swatch's light tone, "b" its
 * dark, "c" its base and "s" a soft shadow; "." leaves the ground showing.
 */
function stamp(ctx: CanvasRenderingContext2D, px: number, py: number, sw: Swatch, rows: readonly string[], ox: number, oy: number): void {
  for (let y = 0; y < rows.length; y++) {
    const row = rows[y];
    let x = 0;
    while (x < row.length) {
      const ch = row.charCodeAt(x);
      let n = 1;
      while (x + n < row.length && row.charCodeAt(x + n) === ch) n++;
      if (ch !== 46) {
        ctx.fillStyle = ch === 97 ? sw.a : ch === 98 ? sw.b : ch === 115 ? SHADE : sw.base;
        ctx.fillRect(px + ox + x, py + oy + y, n, 1);
      }
      x += n;
    }
  }
}

/** Long grass: a clump of blades, none of them the same length, none of them centred. */
const GRASS_TUFT: readonly string[][] = [
  ["..a...", ".aa.a.", "a.a.a.", "a.cab.", ".bcab."],
  ["a.....", "a..a..", "a..a.b", ".a.ab.", ".ca.b."],
  ["......", ".a.a..", "aa.aab", "ac.cab", ".cccb."],
  ["....a.", "...a..", "b..a..", "b.a.a.", ".ba.ab"],
];
const TALL_GRASS: Swatch = { base: "#3f8a40", a: "#72b85c", b: "#2e6e36" };
const REEDS: Swatch = { base: "#5c7a44", a: "#8ea25a", b: "#3e5632" };

/**
 * Paving: a 16 px block of irregular flagstones, so a yard is laid stone and not graph paper. Each
 * letter is one stone; "." is a joint. Four versions of the block differ in which stones are paler or
 * darker, and the block version is picked per 16 px square, so the repeat never lines up.
 */
const FLAGS: readonly string[] = [
  "AAAAAA.BBBBB.CC.",
  "AAAAAA.BBBBB.CC.",
  "AAAAAA.BBBBB.CC.",
  "AAAAAA.BBBBB....",
  "AAAAAA.......DD.",
  "........EEEE.DD.",
  "FFFF.GG.EEEE.DD.",
  "FFFF.GG.EEEE.DD.",
  "FFFF.GG......DD.",
  "FFFF.GG.HHHHH...",
  "FFFF....HHHHH.I.",
  "....JJJ.HHHHH.I.",
  "KKK.JJJ.HHHHH.I.",
  "KKK.JJJ.......I.",
  "KKK.JJJ.LLLLL.I.",
  "................",
];
const FLAG_JOINT = "#5c5c62";
const FLAG_TONES = [
  ["#8a8a90", "#a2a2a8", "#6e6e74"],
  ["#7e7e86", "#96969e", "#64646c"],
  ["#929088", "#aaa8a0", "#76746c"],
];
let flagBlocks: HTMLCanvasElement[] | null = null;
function flagstones(): HTMLCanvasElement[] {
  if (flagBlocks) return flagBlocks;
  flagBlocks = [0, 1, 2, 3].map((v) => {
    const c = document.createElement("canvas");
    c.width = 16;
    c.height = 16;
    const g = c.getContext("2d")!;
    for (let y = 0; y < 16; y++) {
      for (let x = 0; x < 16; x++) {
        const ch = FLAGS[y][x];
        if (ch === ".") {
          g.fillStyle = FLAG_JOINT;
        } else {
          const tone = FLAG_TONES[(ch.charCodeAt(0) * 7 + v * 5) % 3];
          const above = y > 0 ? FLAGS[y - 1][x] : ".";
          const below = y < 15 ? FLAGS[y + 1][x] : ".";
          const left = x > 0 ? FLAGS[y][x - 1] : ".";
          g.fillStyle = above === "." || left === "." ? tone[1] : below === "." ? tone[2] : tone[0];
        }
        g.fillRect(x, y, 1, 1);
      }
    }
    return c;
  });
  return flagBlocks;
}

/** Moss grows in patches with nothing between them, not in a checkerboard. */
const HEDGE_LEAF: readonly string[][] = [
  ["..aaa...", ".aaaac..", "aaaccccb", "aaccccbb", ".cccccbb", "..cccbb.", "...bbb..", "....b..."],
  ["...aaa..", "..aaaac.", ".aaacccc", ".acccccc", "bccccccc", "bbcccbb.", ".bbbbb..", "..bb...."],
  [".....aa.", "...aaaa.", "..caaac.", ".ccccccb", "bcccccbb", "bbcccbb.", ".bbbb...", "..bb...."],
];

const FLOWER = ["#e8e0d0", "#f0d048", "#c8403c", "#b08cd8", "#e89048"];

// --- the sprite bank ------------------------------------------------------------------------------------

type Bank = { large: Spr[][]; med: Spr[][]; pines: Spr[]; dead: Spr[]; bushes: Spr[][]; berry: Spr[]; rocks: Spr[]; boulders: Spr[] };
let bank: Bank | null = null;
function sprites(): Bank {
  if (bank) return bank;
  const large = LEAVES.map((t, p) => [0, 1, 2, 3].map((i) => broadleaf(1000 + p * 37 + i * 101, true, t)));
  const med = LEAVES.map((t, p) => [0, 1, 2].map((i) => broadleaf(5000 + p * 41 + i * 97, false, t)));
  const pines = [0, 1, 2].map((i) => pine(9000 + i * 53));
  const dead = [0, 1].map((i) => deadTree(12000 + i * 71));
  const bushes = BUSHES.map((t, p) => [0, 1, 2, 3].map((i) => bush(15000 + p * 13 + i * 29, t, false)));
  const berry = [0, 1].map((i) => bush(17000 + i * 31, BUSHES[0], true));
  const rockSet = [0, 1, 2, 3].map((i) => rocks(19000 + i * 43));
  const boulders = [0, 1, 2].map((i) => boulder(21000 + i * 59));
  bank = { large, med, pines, dead, bushes, berry, rocks: rockSet, boulders };
  return bank;
}

// --- chunk painting --------------------------------------------------------------------------------------

/** Set per zone by the cache: out of doors, a block of wall is drawn as a building's roof. */
let outdoor = false;

/** Per-chunk scratch, reused: ground tile and group per cell with a 2-cell margin, and a pixel material map with a 1-cell margin. */
const GO = 3; // ground map margin, cells
const GM = CHUNK_CELLS + GO * 2; // ground map side, cells [-3, 19)
const MM = (CHUNK_CELLS + 2) * CELL; // material map side, pixels for cells [-1, 17)
const groundT = new Uint8Array(GM * GM);
const groundG = new Uint8Array(GM * GM);
const smoothG = new Uint8Array(GM * GM);
const rawT = new Uint8Array(GM * GM);
const mat = new Uint8Array(MM * MM);

type Standing = { y: number; draw: (g: CanvasRenderingContext2D, x: number, y: number) => void };

type Chunk = {
  canvas: HTMLCanvasElement;
  /** Standing things, one strip per cell row; null when the chunk has none. */
  strips: HTMLCanvasElement | null;
  /** Bit r set: row r's strip has something in it. */
  rows: number;
  used: number;
};

function buildChunk(grid: Grid, chunkX: number, chunkY: number): Chunk {
  const canvas = document.createElement("canvas");
  canvas.width = CHUNK_PX;
  canvas.height = CHUNK_PX;
  const ctx = canvas.getContext("2d")!;
  const x0 = chunkX * CHUNK_CELLS;
  const y0 = chunkY * CHUNK_CELLS;
  const tiles = grid.tiles;
  const gw = grid.w;
  const gh = grid.h;
  const flags = TILE_FLAGS;
  const group = GROUP;
  const inherit = INHERIT;

  // 1. Ground tile per cell, margin 2.
  for (let j = 0; j < GM; j++) {
    for (let i = 0; i < GM; i++) {
      const x = x0 + i - GO;
      const y = y0 + j - GO;
      rawT[j * GM + i] = x < 0 || y < 0 || x >= gw || y >= gh ? Tile.Void : tiles[y * gw + x];
    }
  }
  const tileAt = (x: number, y: number): number => (x < 0 || y < 0 || x >= gw || y >= gh ? Tile.Void : tiles[y * gw + x]);
  for (let j = 0; j < GM; j++) {
    for (let i = 0; i < GM; i++) {
      let t = rawT[j * GM + i];
      if (inherit[t] !== 0 || (t === Tile.Cliff && loneCliff(tileAt, x0 + i - GO, y0 + j - GO))) {
        const x = x0 + i - GO;
        const y = y0 + j - GO;
        let found = 0;
        for (let k = 0; k < 4 && found === 0; k++) {
          const n = tileAt(x + (k === 1 ? -1 : k === 2 ? 1 : 0), y + (k === 0 ? -1 : k === 3 ? 1 : 0));
          if (inherit[n] !== 0 || n === Tile.Water || n === Tile.Void) continue;
          if (group[n] !== 0 || (flags[n] & F_SOLID) === 0) found = n;
        }
        t = found !== 0 ? found : t === Tile.Cliff ? Tile.Dirt : inherit[t];
      }
      groundT[j * GM + i] = t;
      groundG[j * GM + i] = t === Tile.Cliff && !cliffFace(tileAt, x0 + i - GO, y0 + j - GO) ? G_CLIFF : group[t];
    }
  }
  // Speckle: a cell of wild ground (grass, marsh, dried mud, sand, a garden plant) with at most one
  // neighbour like it is noise in the generator's dice, not a place, so it draws as the ground round it.
  // Made ground (road, cobble) and water are never touched: they mean something.
  smoothG.set(groundG);
  for (let j = 1; j < GM - 1; j++) {
    for (let i = 1; i < GM - 1; i++) {
      const k = j * GM + i;
      const g = groundG[k];
      if (WILD[g] === 0) continue;
      const n0 = groundG[k - GM];
      const n1 = groundG[k + GM];
      const n2 = groundG[k - 1];
      const n3 = groundG[k + 1];
      if ((n0 === g ? 1 : 0) + (n1 === g ? 1 : 0) + (n2 === g ? 1 : 0) + (n3 === g ? 1 : 0) > 1) continue;
      // The commonest wild neighbour takes it.
      let best = 0;
      let bestN = 0;
      for (const c of [n0, n1, n2, n3]) {
        if (c === g || WILD[c] === 0) continue;
        const n = (n0 === c ? 1 : 0) + (n1 === c ? 1 : 0) + (n2 === c ? 1 : 0) + (n3 === c ? 1 : 0);
        if (n > bestN) {
          best = c;
          bestN = n;
        }
      }
      if (bestN >= 2) smoothG[k] = best;
    }
  }
  groundG.set(smoothG);

  // 2. Material per pixel for cells [-1, 17), with chamfered convex corners.
  for (let j = 0; j < CHUNK_CELLS + 2; j++) {
    for (let i = 0; i < CHUNK_CELLS + 2; i++) {
      const gi = (j + GO - 1) * GM + (i + GO - 1);
      const g = groundG[gi];
      const base = j * CELL * MM + i * CELL;
      for (let py = 0; py < CELL; py++) mat.fill(g, base + py * MM, base + py * MM + CELL);
      if (g === 0) continue;
      for (let q = 0; q < 4; q++) {
        const dx = (q & 1) === 0 ? -1 : 1;
        const dy = (q & 2) === 0 ? -1 : 1;
        const a = groundG[gi + dx];
        const b = groundG[gi + dy * GM];
        if (a === g || a === 0 || b === g || b === 0) continue;
        const fill = HEIGHT[a] <= HEIGHT[b] ? b : a;
        // Pixels within the chamfer: distance from the outer corner along both axes sums below 4.
        for (let u = 0; u < 4; u++) {
          for (let v = 0; v < 4 - u; v++) {
            const px = dx < 0 ? u : CELL - 1 - u;
            const py = dy < 0 ? v : CELL - 1 - v;
            mat[base + py * MM + px] = fill;
          }
        }
      }
    }
  }

  // 3. Base runs, chunk area only.
  for (let py = 0; py < CHUNK_PX; py++) {
    const row = (py + CELL) * MM + CELL;
    let x = 0;
    while (x < CHUNK_PX) {
      const m = mat[row + x];
      let n = 1;
      while (x + n < CHUNK_PX && mat[row + x + n] === m) n++;
      if (m !== 0) {
        ctx.fillStyle = GT[m].base;
        ctx.fillRect(x, py, n, 1);
      }
      x += n;
    }
  }

  // 4. Per cell: detail on ground, or the hard tile's own painter.
  for (let j = 0; j < CHUNK_CELLS; j++) {
    for (let i = 0; i < CHUNK_CELLS; i++) {
      const gi = (j + GO) * GM + (i + GO);
      const t = rawT[gi];
      const cx = x0 + i;
      const cy = y0 + j;
      if (groundG[gi] === 0) paintHard(ctx, grid, groundT[gi], cx, cy, i * CELL, j * CELL);
      else paintGround(ctx, t, groundG[gi], cx, cy, i * CELL, j * CELL);
    }
  }

  // 5. Edges from the material map, then the shade a wall, a cliff or a house throws on the ground south of it.
  paintEdges(ctx);
  ctx.fillStyle = "#00000030";
  for (let j = 0; j < CHUNK_CELLS; j++) {
    for (let i = 0; i < CHUNK_CELLS; i++) {
      const gi = (j + GO) * GM + (i + GO);
      if (groundG[gi] !== 0 && groundG[gi - GM] === 0 && RAISED[rawT[gi - GM]] === 1) ctx.fillRect(i * CELL, j * CELL, CELL, 2);
    }
  }

  // 6. Shadows on the ground under standing things, from cells two beyond the chunk so none is cut at a seam.
  const bankS = sprites();
  const standing: Standing[] = [];
  for (let j = -2; j < CHUNK_CELLS + 2; j++) {
    for (let i = -2; i < CHUNK_CELLS + 2; i++) {
      const t = rawT[(j + GO) * GM + (i + GO)];
      const inside = i >= 0 && j >= 0 && i < CHUNK_CELLS && j < CHUNK_CELLS;
      const cx = x0 + i;
      const cy = y0 + j;
      const px = i * CELL;
      const py = j * CELL;
      if (isTree(t) || t === Tile.DeadTree) {
        const pick = treeSprite(bankS, tileAt, t, groundG[(j + GO) * GM + (i + GO)], cx, cy);
        if (pick) {
          const big = pick.s.w >= 30 ? 1 : 0;
          ellipse(ctx, px + 4 + pick.ox + 2, py + CELL - 1, 7 + big * 3, 3, "#0a1a1040");
          ellipse(ctx, px + 4 + pick.ox + 1, py + CELL - 1, 4 + big * 2, 2, "#0a1a1030");
          if (inside) standing.push({ y: j, draw: (g, sx, sy) => g.drawImage(pick.s.c, sx + px + 4 + pick.ox - pick.s.ax, sy + py + CELL - pick.s.ay) });
        } else if (t !== Tile.DeadTree) {
          // Under the crowns: the floor of the wood, in shade.
          ctx.fillStyle = "#06140c38";
          ctx.fillRect(px, py, CELL, CELL);
        }
      } else if (t === Tile.Bush) {
        ellipse(ctx, px + 5, py + CELL, 5, 2, "#0a1a1038");
        if (inside) {
          const h = cellHash(cx, cy);
          const s = (h & 31) === 7 ? bankS.berry[h & 1] : bankS.bushes[areaHash(cx, cy, 4) % 5 === 0 ? 1 : 0][(h >> 2) & 3];
          standing.push({ y: j, draw: (g, sx, sy) => g.drawImage(s.c, sx + px + 4 - s.ax, sy + py + CELL + 1 - s.ay) });
        }
      } else if (t === Tile.Rubble) {
        ellipse(ctx, px + 5, py + CELL - 1, 5, 2, "#00000030");
        if (inside) {
          const s = bankS.rocks[cellHash(cx, cy) & 3];
          standing.push({ y: j, draw: (g, sx, sy) => g.drawImage(s.c, sx + px + 4 - s.ax, sy + py + CELL + 1 - s.ay) });
        }
      } else if (t === Tile.Cliff && loneCliff(tileAt, cx, cy)) {
        ellipse(ctx, px + 5, py + CELL - 1, 7, 2, "#00000034");
        if (inside) {
          const s = bankS.boulders[cellHash(cx, cy) % 3];
          standing.push({ y: j, draw: (g, sx, sy) => g.drawImage(s.c, sx + px + 4 - s.ax, sy + py + CELL + 1 - s.ay) });
        }
      } else if (inside && t === Tile.Fence) {
        standing.push({ y: j, draw: (g, sx, sy) => fence(g, tileAt, cx, cy, sx + px, sy + py) });
      } else if (inside && t === Tile.StoneWall) {
        standing.push({ y: j, draw: (g, sx, sy) => stoneWall(g, tileAt, cx, cy, sx + px, sy + py) });
      }
    }
  }

  let strips: HTMLCanvasElement | null = null;
  let rows = 0;
  if (standing.length > 0) {
    strips = document.createElement("canvas");
    strips.width = CHUNK_PX + STRIP_MX * 2;
    strips.height = CHUNK_CELLS * STRIP_H;
    const g = strips.getContext("2d")!;
    for (const s of standing) {
      // Row s.y's strip: its bottom edge is STRIP_BELOW px under the row's cell bottom.
      const top = s.y * STRIP_H;
      const originY = top + STRIP_H - STRIP_BELOW - (s.y + 1) * CELL;
      g.save();
      g.beginPath();
      g.rect(0, top, strips.width, STRIP_H);
      g.clip();
      s.draw(g, STRIP_MX, originY);
      g.restore();
      rows |= 1 << s.y;
    }
  }
  return { canvas, strips, rows, used: 0 };
}

/** A cliff cell that shows its face: open ground south of it, or of the cliff cell south of it. */
function cliffFace(tileAt: (x: number, y: number) => number, x: number, y: number): boolean {
  const s = tileAt(x, y + 1);
  if (s !== Tile.Cliff) return WALL_LIKE[s] === 0;
  const s2 = tileAt(x, y + 2);
  return s2 !== Tile.Cliff && WALL_LIKE[s2] === 0;
}

/** A cliff cell with at most one cliff beside it is not a cliff, it is a boulder. */
function loneCliff(tileAt: (x: number, y: number) => number, x: number, y: number): boolean {
  const n = (tileAt(x, y - 1) === Tile.Cliff ? 1 : 0) + (tileAt(x, y + 1) === Tile.Cliff ? 1 : 0) + (tileAt(x - 1, y) === Tile.Cliff ? 1 : 0) + (tileAt(x + 1, y) === Tile.Cliff ? 1 : 0);
  return n <= 1;
}

function ellipse(ctx: CanvasRenderingContext2D, cx: number, cy: number, rx: number, ry: number, color: string): void {
  ctx.fillStyle = color;
  for (let y = -ry; y <= ry; y++) {
    const w = Math.round(rx * Math.sqrt(Math.max(0, 1 - (y * y) / (ry * ry + 0.5))));
    if (w > 0) ctx.fillRect(cx - w, cy + y, w * 2, 1);
  }
}

/**
 * Which tree sprite a tree cell shows, if any. A wood is not a tree per cell: crowns go down on a
 * staggered three-cell lattice, the wood's south edge gets a row of its own so the trunks show, and a
 * lone tree always gets one. Cells with no crown of their own are the shaded floor under their neighbours'.
 */
function treeSprite(
  b: Bank,
  tileAt: (x: number, y: number) => number,
  t: number,
  g: number,
  cx: number,
  cy: number,
): { s: Spr; ox: number } | null {
  const h = cellHash(cx, cy);
  if (t === Tile.DeadTree) return { s: b.dead[h & 1], ox: 0 };
  const n = tileAt(cx, cy - 1);
  const s = tileAt(cx, cy + 1);
  const w = tileAt(cx - 1, cy);
  const e = tileAt(cx + 1, cy);
  const lone = !isTree(n) && !isTree(s) && !isTree(w) && !isTree(e);
  const southEdge = !isTree(s);
  const band = Math.floor(cy / 3);
  let anchor = false;
  let ox = 0;
  let large = true;
  if (lone) {
    anchor = true;
    large = (h & 3) !== 0;
  } else if (southEdge) {
    anchor = (cx & 1) === 0 || !isTree(w);
    large = (h & 1) === 0;
    ox = (h & 2) === 0 ? 0 : (h & 4) === 0 ? -1 : 1;
  } else if (cy % 3 === 2 && cx % 3 === 0) {
    anchor = true;
    if ((band & 1) === 1 && isTree(e) && isTree(tileAt(cx + 2, cy))) ox = 12;
    ox += ((h >> 3) & 3) - 1;
  }
  if (!anchor) return null;
  // Conifers on the high ground, dark crowns and the odd dead one in the wet, mixed greens elsewhere.
  if (t === Tile.Pine || (g === G_DIRT && (h & 3) !== 0) || (g === G_DRY && (h & 1) === 0)) return { s: b.pines[h % 3], ox };
  if (g === G_MOSS) {
    if ((h & 15) === 3) return { s: b.dead[h & 1], ox };
    return { s: large ? b.large[2][h & 3] : b.med[2][h % 3], ox };
  }
  const area = areaHash(cx, cy, 5);
  const pal = area < 60 ? 1 : area > 230 ? 2 : (h & 15) === 5 ? 1 : 0;
  return { s: large ? b.large[pal][(h >> 1) & 3] : b.med[pal][h % 3], ox };
}

const FENCE = { o: "#2e1e14", d: "#5a3a22", m: "#8a5e36", l: "#b08050" };

/** A wooden fence: posts and two rails along a run, a line of posts down a north-south run. */
function fence(g: CanvasRenderingContext2D, tileAt: (x: number, y: number) => number, cx: number, cy: number, px: number, py: number): void {
  const w = tileAt(cx - 1, cy) === Tile.Fence;
  const e = tileAt(cx + 1, cy) === Tile.Fence;
  const n = tileAt(cx, cy - 1) === Tile.Fence;
  const s = tileAt(cx, cy + 1) === Tile.Fence;
  const b = py + CELL; // the cell's bottom
  const r = (x: number, y: number, ww: number, hh: number, c: string): void => {
    g.fillStyle = c;
    g.fillRect(x, y, ww, hh);
  };
  if ((w || e) && !(n || s) ? true : !(n || s)) {
    // Rails, east-west, with an outline so they read against grass and road alike.
    const x0 = w ? px : px + 3;
    const x1 = e ? px + CELL : px + 5;
    for (const ry of [b - 9, b - 5]) {
      r(x0, ry - 1, x1 - x0, 1, FENCE.o);
      r(x0, ry, x1 - x0, 1, FENCE.l);
      r(x0, ry + 1, x1 - x0, 1, FENCE.m);
      r(x0, ry + 2, x1 - x0, 1, FENCE.o);
    }
    r(x0, b - 1, x1 - x0, 1, "#00000030");
  } else {
    // North-south: a rail seen from above, joining this post to the next one north.
    r(px + 3, n ? b - 16 : b - 10, 2, n ? 14 : 8, FENCE.m);
    r(px + 2, n ? b - 16 : b - 10, 1, n ? 14 : 8, FENCE.o);
    r(px + 5, n ? b - 16 : b - 10, 1, n ? 14 : 8, FENCE.o);
    r(px + 3, n ? b - 16 : b - 10, 1, n ? 14 : 8, FENCE.l);
  }
  // The post.
  r(px + 2, b - 12, 4, 12, FENCE.o);
  r(px + 3, b - 11, 2, 10, FENCE.m);
  r(px + 3, b - 11, 1, 10, FENCE.l);
  r(px + 3, b - 11, 2, 1, "#d0a070");
  r(px + 2, b, 4, 1, "#00000030");
}

const STONE = { o: "#2a2622", d: "#56524a", m: "#77726a", l: "#9a958a", h: "#bab5a8" };

/** A low dry-stone wall: a capped top that runs on into its neighbours, a face where it drops south. */
function stoneWall(g: CanvasRenderingContext2D, tileAt: (x: number, y: number) => number, cx: number, cy: number, px: number, py: number): void {
  const w = tileAt(cx - 1, cy) === Tile.StoneWall;
  const e = tileAt(cx + 1, cy) === Tile.StoneWall;
  const n = tileAt(cx, cy - 1) === Tile.StoneWall;
  const s = tileAt(cx, cy + 1) === Tile.StoneWall;
  const b = py + CELL;
  const top = n ? py - 4 : b - 11;
  const faceTop = s ? b + 4 : b - 4;
  const x0 = w ? px : px + 1;
  const x1 = e ? px + CELL : px + CELL - 1;
  const r = (x: number, y: number, ww: number, hh: number, c: string): void => {
    g.fillStyle = c;
    g.fillRect(x, y, ww, hh);
  };
  r(x0, top, x1 - x0, faceTop - top, STONE.m);
  const h = cellHash(cx, cy);
  // Capstones.
  for (let y = top + 1; y < faceTop - 1; y += 3) {
    const off = ((y + (h & 3)) & 3) - 1;
    r(x0, y, x1 - x0, 1, STONE.l);
    r(x0 + 3 + off, y, 1, 2, STONE.d);
  }
  if (!n) {
    r(x0, top, x1 - x0, 1, STONE.o);
    r(x0, top + 1, x1 - x0, 1, STONE.h);
  }
  if (!s) {
    r(x0, faceTop, x1 - x0, b - faceTop, STONE.d);
    r(x0, faceTop, x1 - x0, 1, STONE.o);
    r(x0 + 2 + (h & 1), faceTop + 1, 1, 2, STONE.o);
    r(x0 + 5, faceTop + 2, 1, 2, STONE.o);
    r(x0, b, x1 - x0, 1, "#00000038");
  }
  if (!w) r(px, top, 1, (s ? b : b) - top, STONE.o);
  if (!e) r(px + CELL - 1, top, 1, b - top, STONE.o);
}

/** Detail on a ground cell. `t` is the cell's own tile (long grass, a flower bed), `g` the ground under it. */
function paintGround(ctx: CanvasRenderingContext2D, t: number, g: number, cx: number, cy: number, px: number, py: number): void {
  const h = cellHash(cx, cy);
  const tone = GT[g];
  // Only on this cell's own material: a chamfer taken by the neighbour keeps the neighbour's look.
  const own = (x: number, y: number): boolean => mat[(py + y + CELL) * MM + px + x + CELL] === g;
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    if (!own(x, y) || !own(x + w - 1, y + hh - 1)) return;
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };
  switch (g) {
    case G_GRASS:
      // Calm, as Koholint's grass is calm: a tuft in one cell of three, a flower now and then.
      if (h % 3 === 0) {
        const x = 1 + (h >> 2) % 4;
        const y = 1 + (h >> 4) % 4;
        dot(x, y + 1, tone.b);
        dot(x + 1, y + 2, tone.b);
        dot(x + 2, y, tone.b, 1, 2);
        dot(x + 2, y, tone.a);
      } else if ((h & 15) === 4) dot((h >> 4) % 6 + 1, (h >> 2) % 6 + 1, tone.a, 1, 1);
      // Here and there a meadow patch where the flowers are thick, the way Koholint has them.
      if (t === Tile.Grass && (h & 7) === 2 && areaHash(cx, cy, 3) < 22) {
        const x = 1 + (h >> 3) % 5;
        const y = 1 + (h >> 5) % 5;
        const c = FLOWER[areaHash(cx, cy, 3) % 5];
        dot(x, y + 1, "#3a7a3a", 1, 1);
        dot(x - 1, y, c, 1, 1);
        dot(x + 1, y, c, 1, 1);
        dot(x, y - 1, c, 1, 1);
        dot(x, y, "#f0d048", 1, 1);
      } else if ((h & 127) === 17) {
        const x = 2 + (h >> 3) % 4;
        dot(x, 3, FLOWER[(h >> 1) % 5]);
        dot(x - 1, 4, "#3a7a3a", 3, 1);
      }
      break;
    case G_DIRT:
      if ((h & 3) === 0) {
        dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 1, tone.a);
        dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 2, tone.b);
      }
      if ((h & 7) === 3) dot((h >> 3) % 5 + 1, (h >> 1) % 5 + 1, tone.b, 2, 1);
      if ((h & 31) === 9) dot((h >> 2) % 5 + 1, (h >> 5) % 5 + 1, SCUFF, 3, 2);
      break;
    case G_ROAD:
      if ((h & 3) === 1) {
        dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 1, tone.a);
        dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 2, tone.b);
      }
      if ((h & 7) === 6) dot((h >> 3) % 6 + 1, (h >> 1) % 6 + 1, tone.b);
      if ((h & 15) === 10) dot((h >> 4) % 5 + 1, (h >> 2) % 5 + 1, SCUFF, 3, 1);
      break;
    case G_SAND:
      if ((h & 3) === 1) dot(h % 7, (h >> 3) % 7, tone.b);
      if ((h & 7) === 2) dot((h >> 2) % 6, (h >> 5) % 6, tone.a, 2, 1);
      break;
    case G_WATER:
      if ((h & 3) === 0) {
        const x = 1 + (h >> 2) % 4;
        const y = 2 + (h >> 4) % 4;
        dot(x, y, tone.a, 2, 1);
        dot(x + 2, y - 1, tone.a, 1, 1);
      } else if ((h & 7) === 5) dot((h >> 3) % 6 + 1, (h >> 1) % 6 + 1, tone.b, 2, 1);
      break;
    case G_MOSS:
      if ((h & 3) === 0) dot((h >> 2) % 5 + 1, (h >> 4) % 5 + 1, tone.b, 3, 2);
      else if ((h & 7) === 5) dot((h >> 3) % 5 + 1, (h >> 1) % 5 + 1, tone.a, 2, 1);
      break;
    case G_DRY: {
      if ((h & 3) === 2) {
        // Mud that has dried and split.
        const x = 1 + (h >> 2) % 4;
        const y = 1 + (h >> 4) % 4;
        dot(x, y, tone.b, 2, 1);
        dot(x + 2, y + 1, tone.b, 1, 1);
        dot(x + 3, y + 2, tone.b, 1, 1);
        dot(x + 1, y + 1, tone.a, 1, 1);
      } else if ((h & 7) === 1) dot((h >> 3) % 6 + 1, (h >> 1) % 6 + 1, tone.a);
      break;
    }
    case G_GARDEN:
      if (t === Tile.Crops) {
        // Planted rows: a furrow and a line of leaf along each.
        const lush = ["#4f8a3c", "#6aa448", "#3a6a2e"];
        for (const ry of [2, 6]) {
          dot(0, ry + 1, tone.b, CELL, 1);
          for (let k = 0; k < 2; k++) {
            const x = k * 4 + ((h >> (k + ry)) & 1);
            dot(x, ry - 1, lush[2], 3, 2);
            dot(x, ry - 1, lush[0], 2, 1);
            dot(x + 1, ry - 2, lush[1], 1, 1);
          }
        }
      } else {
        dot(0, 3, tone.b, CELL, 1);
        dot(0, 7, tone.b, CELL, 1);
        dot((h >> 1) % 6, 1, tone.a, 2, 1);
        dot((h >> 4) % 6, 5, tone.a, 2, 1);
        if (((cx + cy) & 1) === 0) {
          const gx = 2 + (h % 3);
          dot(gx, 1, "#6a9a48", 1, 2);
          dot(gx - 1, 1, "#5a8a3c", 1, 1);
          dot(gx + 1, 1, "#5a8a3c", 1, 1);
        }
      }
      break;
    case G_COBBLE: {
      const block = flagstones()[cellHash(cx >> 1, cy >> 1) & 3];
      const sx = (cx & 1) * CELL;
      const sy = (cy & 1) * CELL;
      if (own(0, 0) && own(CELL - 1, 0) && own(0, CELL - 1) && own(CELL - 1, CELL - 1)) {
        ctx.drawImage(block, sx, sy, CELL, CELL, px, py, CELL, CELL);
      } else {
        // An edge cell: only the pixels the chamfers left it.
        for (let y = 0; y < CELL; y++) for (let x = 0; x < CELL; x++) if (own(x, y)) ctx.drawImage(block, sx + x, sy + y, 1, 1, px + x, py + y, 1, 1);
      }
      break;
    }
    case G_CLIFF:
      // Weathered rock: a lit stone and its shadow, a ledge, a crack, a tuft that found a hold.
      if ((h & 3) === 0) {
        dot((h >> 2) % 5 + 1, (h >> 4) % 5 + 1, tone.a, 2, 1);
        dot((h >> 2) % 5 + 1, (h >> 4) % 5 + 2, tone.b, 2, 1);
      } else if ((h & 7) === 5) {
        dot((h >> 3) % 5 + 1, (h >> 1) % 5 + 1, tone.b, 1, 2);
        dot((h >> 3) % 5 + 2, (h >> 1) % 5 + 2, tone.b, 1, 1);
      } else if ((h & 7) === 2) {
        dot((h >> 3) % 4 + 1, (h >> 5) % 4 + 2, tone.a, 3, 1);
        dot((h >> 3) % 4 + 1, (h >> 5) % 4 + 3, tone.b, 3, 1);
      } else if ((h & 31) === 10) {
        dot(2, 3, "#5e7a44", 1, 2);
        dot(4, 3, "#5e7a44", 1, 2);
        dot(3, 4, "#5e7a44", 1, 1);
      }
      break;
    case G_ICE:
      if ((h & 3) === 0) dot(h % 5, 2, tone.a, 3, 1);
      if ((h & 7) === 5) dot((h >> 2) % 5, 5, tone.b, 2, 1);
      break;
    default:
      break;
  }

  // What stands on the ground and is not tall enough to y-sort.
  switch (t) {
    case Tile.Garden:
    case Tile.Crops: {
      if (g === G_GARDEN) break;
      // A stray plot the speckle filter took back into the grass: what grew there is still there.
      const x = px + 1 + (h & 1);
      const y = py + 1 + ((h >> 1) & 1);
      ctx.fillStyle = "#4a3826";
      ctx.fillRect(x, y + 4, 6, 1);
      ctx.fillStyle = "#2a5a2c";
      ctx.fillRect(x, y + 1, 6, 3);
      ctx.fillRect(x + 1, y, 4, 1);
      ctx.fillStyle = "#5a9a48";
      ctx.fillRect(x + 1, y + 1, 3, 2);
      ctx.fillStyle = "#8cc060";
      ctx.fillRect(x + 2, y + 1, 1, 1);
      break;
    }
    case Tile.GrassTall: {
      if ((h & 3) === 3) break;
      const sw = g === G_MOSS || g === G_DRY ? REEDS : TALL_GRASS;
      stamp(ctx, px, py, sw, GRASS_TUFT[h & 3], 1 + (h % 2), 1 + ((h >> 2) % 3));
      break;
    }
    case Tile.FlowerBed: {
      // Two or three blooms a cell, one colour to a bed so it reads as planted.
      const c = FLOWER[areaHash(cx, cy, 2) % 5];
      ctx.fillStyle = "#2f6a34";
      ctx.fillRect(px + 1, py + 5, 6, 1);
      for (let k = 0; k < 3; k++) {
        if (k === 2 && (h & 1) === 0) break;
        const fx = px + [1, 5, 3][k] + ((h >> k) & 1);
        const fy = py + [2, 3, 6][k] - ((h >> (k + 2)) & 1);
        ctx.fillStyle = "#2f6a34";
        ctx.fillRect(fx, fy + 1, 1, 2);
        ctx.fillStyle = c;
        ctx.fillRect(fx - 1, fy, 3, 1);
        ctx.fillRect(fx, fy - 1, 1, 3);
        ctx.fillStyle = "#f0d048";
        ctx.fillRect(fx, fy, 1, 1);
      }
      break;
    }
    case Tile.Stepping: {
      const ox = (h & 1) - (h >> 1 & 1);
      const oy = (h >> 2 & 1);
      ctx.fillStyle = "#3a3a36";
      ctx.fillRect(px + 1 + ox, py + 1 + oy, 6, 5);
      ctx.fillStyle = "#9a968c";
      ctx.fillRect(px + 1 + ox, py + 1 + oy, 6, 4);
      ctx.fillStyle = "#b8b4a8";
      ctx.fillRect(px + 2 + ox, py + 1 + oy, 4, 1);
      ctx.fillStyle = "#7a766c";
      ctx.fillRect(px + 2 + ox, py + 4 + oy, 4, 1);
      ctx.fillStyle = "#2a2a26";
      ctx.fillRect(px + 1 + ox, py + oy, 6, 1);
      ctx.fillRect(px + ox, py + 1 + oy, 1, 4);
      ctx.fillRect(px + 7 + ox, py + 1 + oy, 1, 4);
      ctx.fillRect(px + 1 + ox, py + 5 + oy, 6, 1);
      break;
    }
    default:
      break;
  }
}

/** Colours of edges, precomputed per group. */
const SHADOW_UNDER = "#00000030";
const WATER_DEEP = "#1e4a86";
const WATER_FOAM = "#9ccaf0";

/**
 * Edges, read off the material map. Higher ground gets a dark rim where it drops to lower ground,
 * and the lower ground takes a shadow under it (under a bank of grass, two rows on water); water
 * shows a pale lip wherever else it meets anything. A raised hard tile (a wall, a cliff, a house)
 * shades the two rows of ground under it.
 */
function paintEdges(ctx: CanvasRenderingContext2D): void {
  const height = HEIGHT;
  const tones = GT;
  for (let y = 0; y < CHUNK_PX; y++) {
    const row = (y + CELL) * MM + CELL;
    for (let x = 0; x < CHUNK_PX; x++) {
      const i = row + x;
      const m = mat[i];
      if (m === 0) continue;
      const up = mat[i - MM];
      const dn = mat[i + MM];
      const lf = mat[i - 1];
      const rt = mat[i + 1];
      const hm = height[m];
      let color: string | null = null;
      if (m === G_WATER || m === G_ICE) {
        const up2 = mat[i - MM * 2];
        if (up !== m && up !== G_ICE && up !== G_WATER) color = WATER_DEEP;
        else if (up2 !== m && up2 !== G_ICE && up2 !== G_WATER && y >= 0) color = tones[m].rim;
        else if ((dn !== m && dn !== 0) || (lf !== m && lf !== 0) || (rt !== m && rt !== 0)) color = WATER_FOAM;
      } else if (up !== 0 && up !== m && height[up] > hm) color = SHADOW_UNDER;
      else if ((up !== 0 && height[up] < hm) || (dn !== 0 && height[dn] < hm) || (lf !== 0 && height[lf] < hm) || (rt !== 0 && height[rt] < hm)) {
        color = tones[m].rim;
      }
      if (color !== null) {
        ctx.fillStyle = color;
        ctx.fillRect(x, y, 1, 1);
      }
    }
  }
}

/** Paint a hard tile: walls and cliffs, roofs and house walls, floors, boards, rails. */
function paintHard(ctx: CanvasRenderingContext2D, grid: Grid, t: number, cx: number, cy: number, px: number, py: number): void {
  const sw = SW[t as Tile] ?? SW[Tile.Void]!;
  const h = cellHash(cx, cy);
  ctx.fillStyle = sw.base;
  ctx.fillRect(px, py, CELL, CELL);
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };
  const at = (dx: number, dy: number): number => grid.tileAt(cx + dx, cy + dy);

  if (t === Tile.Cliff) {
    paintCliff(ctx, grid, cx, cy, px, py, h);
    return;
  }
  if (WALL_LIKE[t] === 1) {
    if (t === Tile.Void) return;
    // A wall whose south neighbour is open shows its front face; otherwise only its top.
    const southOpen = WALL_LIKE[at(0, 1)] === 0;
    // Out of doors a wide block of wall is a building, and a building seen from above is its roof.
    // A wall one cell thick (a ruin, a yard wall) keeps its stone cap.
    const thin = (WALL_LIKE[at(-1, 0)] === 0 && WALL_LIKE[at(1, 0)] === 0) || (WALL_LIKE[at(0, -1)] === 0 && WALL_LIKE[at(0, 1)] === 0);
    if (outdoor && !southOpen && !thin) {
      ctx.fillStyle = WORKS_ROOF.base;
      ctx.fillRect(px, py, CELL, CELL);
      paintRoof(ctx, Tile.RoofSlate, WORKS_ROOF, at, cx, cy, px, py, h, (n) => WALL_LIKE[n] === 1);
      return;
    }
    if (southOpen) {
      ctx.fillStyle = sw.a;
      ctx.fillRect(px, py, CELL, CELL);
      dot(0, 0, sw.b, CELL, 1);
      dot(0, 4, sw.b, CELL, 1);
      dot((h & 1) === 0 ? 3 : 6, 1, sw.b, 1, 3);
      dot((h & 2) === 0 ? 1 : 5, 5, sw.b, 1, 3);
      dot(0, 7, "#00000055", CELL, 1);
    } else {
      if ((h & 7) === 0) dot(h % 6, (h >> 3) % 6, sw.a, 2, 1);
      if ((h & 15) === 3) dot((h >> 2) % 6, h % 6, sw.b, 1, 2);
      // The lip of the wall's top where it meets open floor, so a block reads as a block.
      if (WALL_LIKE[at(0, -1)] === 0) dot(0, 0, sw.a, CELL, 1);
      if (WALL_LIKE[at(-1, 0)] === 0) dot(0, 0, sw.a, 1, CELL);
      if (WALL_LIKE[at(1, 0)] === 0) dot(CELL - 1, 0, sw.b, 1, CELL);
    }
    return;
  }
  if (isRoof(t)) {
    paintRoof(ctx, t, sw, at, cx, cy, px, py, h);
    return;
  }
  if (isHouseWall(t)) {
    paintHouseWall(ctx, t, sw, at, cx, cy, px, py, h);
    return;
  }

  switch (t) {
    case Tile.Hedge: {
      stamp(ctx, px, py, sw, HEDGE_LEAF[h % 3], 0, 0);
      // A clipped hedge is a block: lit along its top, a shaded face where it drops south, dark sides.
      if (at(0, -1) !== Tile.Hedge) {
        dot(0, 0, "#10241a", CELL, 1);
        dot(0, 1, "#5a9a5c", CELL, 1);
        dot(1 + (h & 3), 2, "#5a9a5c", 2, 1);
      }
      if (at(0, 1) !== Tile.Hedge) {
        dot(0, CELL - 4, "#1c3e2a", CELL, 3);
        dot((h & 3) + 1, CELL - 4, "#2a5a3c", 1, 2);
        dot(((h >> 2) & 3) + 4, CELL - 3, "#2a5a3c", 1, 2);
        dot(0, CELL - 1, "#0c1a12", CELL, 1);
      }
      if (at(-1, 0) !== Tile.Hedge) dot(0, 0, "#10241a", 1, CELL);
      if (at(1, 0) !== Tile.Hedge) dot(CELL - 1, 0, "#10241a", 1, CELL);
      break;
    }
    case Tile.Floor:
    case Tile.TempleFloor:
    case Tile.MuseumFloor:
    case Tile.PipeFloor:
    case Tile.WorksFloor:
      dot(0, 0, sw.b, CELL, 1);
      dot(0, 0, sw.b, 1, CELL);
      if ((h & 15) === 1) dot(3, 3, sw.a, 2, 2);
      if ((h & 31) === 7) dot(2, 5, sw.b, 4, 1);
      break;
    case Tile.CaveFloor:
      if ((h & 3) === 1) dot(h % 7, (h >> 3) % 7, sw.b);
      if ((h & 7) === 2) dot((h >> 2) % 6, (h >> 5) % 6, sw.a, 2, 1);
      break;
    case Tile.FloorWood:
    case Tile.SchoolFloor:
      dot(0, 7, sw.b, CELL, 1);
      dot((h % 6) + 1, 2, sw.a, 2, 1);
      if ((cx & 3) === 0) dot(0, 0, sw.b, 1, CELL);
      break;
    case Tile.Boardwalk: {
      // Planks laid across the way the walk runs, a nail head at each end.
      const alongX = at(-1, 0) === Tile.Boardwalk || at(1, 0) === Tile.Boardwalk;
      if (alongX) {
        for (let x = 0; x < CELL; x += 3) {
          dot(x, 0, sw.b, 1, CELL);
          dot(x + 1, 0, sw.a, 1, CELL);
        }
        if (at(0, -1) !== Tile.Boardwalk) dot(0, 0, "#3a2818", CELL, 1);
        if (at(0, 1) !== Tile.Boardwalk) dot(0, CELL - 1, "#2a1c10", CELL, 1);
      } else {
        for (let y = 0; y < CELL; y += 3) {
          dot(0, y, sw.b, CELL, 1);
          dot(0, y + 1, sw.a, CELL, 1);
        }
        if (at(-1, 0) !== Tile.Boardwalk) dot(0, 0, "#3a2818", 1, CELL);
        if (at(1, 0) !== Tile.Boardwalk) dot(CELL - 1, 0, "#2a1c10", 1, CELL);
      }
      break;
    }
    case Tile.Track:
    case Tile.Rail: {
      // Rails run the way the track runs, sleepers lie across it, ballast under both.
      const alongX = at(-1, 0) === t || at(1, 0) === t;
      if (alongX) {
        dot((h % 3) + 1, 0, sw.b, 2, CELL);
        dot(0, 2, sw.a, CELL, 1);
        dot(0, 3, SHADE, CELL, 1);
        dot(0, 5, sw.a, CELL, 1);
        dot(0, 6, SHADE, CELL, 1);
      } else {
        dot(0, (h % 3) + 1, sw.b, CELL, 2);
        dot(2, 0, sw.a, 1, CELL);
        dot(3, 0, SHADE, 1, CELL);
        dot(5, 0, sw.a, 1, CELL);
        dot(6, 0, SHADE, 1, CELL);
      }
      dot((h >> 3) % 7, (h >> 5) % 7, sw.base);
      break;
    }
    case Tile.Glass:
      dot(0, 0, sw.a, CELL, 1);
      dot(0, 0, sw.a, 1, CELL);
      dot(2, 5, sw.a, 1, 1);
      dot(3, 4, sw.a, 1, 1);
      dot(4, 3, sw.a, 1, 1);
      dot(0, 7, sw.b, CELL, 1);
      break;
    case Tile.Sill: {
      const across = at(-1, 0) === Tile.Sill || at(1, 0) === Tile.Sill;
      if (across) {
        dot(0, 1, sw.b, CELL, 1);
        dot(0, 6, sw.b, CELL, 1);
        dot((h & 1) === 0 ? 2 : 5, 3, sw.a, 2, 1);
      } else {
        dot(1, 0, sw.b, 1, CELL);
        dot(6, 0, sw.b, 1, CELL);
        dot(3, (h & 1) === 0 ? 2 : 5, sw.a, 1, 2);
      }
      break;
    }
    default:
      break;
  }
  // Shade on a floor under a wall.
  if (RAISED[t] === 0 && RAISED[at(0, -1)] === 1) dot(0, 0, "#00000030", CELL, 2);
}

/** The roof of a works, a library, a school: slate gone dark with soot. */
const WORKS_ROOF: Swatch = { base: "#50525e", a: "#6a6e80", b: "#2e3038" };

const CLIFF = { top: "#7e6c56", topL: "#957f66", topD: "#65553f", tuft: "#5e7a44", rimL: "#b09878", face: "#6e5848", ridgeL: "#8c7258", ridgeD: "#4c3c30", o: "#2a2018" };

/**
 * Cliffs as Link's Awakening draws them: a rocky top with a lit lip, and where the rock drops to
 * open ground a face two cells tall, ridged top to bottom, dark at its foot.
 */
function paintCliff(ctx: CanvasRenderingContext2D, grid: Grid, cx: number, cy: number, px: number, py: number, h: number): void {
  const at = (dx: number, dy: number): number => grid.tileAt(cx + dx, cy + dy);
  const open = (t: number): boolean => t !== Tile.Cliff && WALL_LIKE[t] === 0;
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };
  const lower = open(at(0, 1));
  const upper = !lower && at(0, 1) === Tile.Cliff && open(at(0, 2));
  if (lower || upper) {
    dot(0, 0, CLIFF.face, CELL, CELL);
    // Ridges run the whole face: the column decides, so the two rows line up.
    for (let x = 0; x < CELL; x++) {
      const wx = cx * CELL + x;
      const k = cellHash(wx, 77) & 7;
      if (k < 2) dot(x, 0, CLIFF.ridgeD, 1, CELL);
      else if (k === 3) dot(x, 0, CLIFF.ridgeL, 1, CELL);
    }
    if ((h & 3) === 0) dot((h >> 2) % 6 + 1, (h >> 4) % 5 + 1, CLIFF.ridgeL, 1, 2);
    if (at(0, -1) !== Tile.Cliff || !cliffFace((x, y) => grid.tileAt(x, y), cx, cy - 1)) {
      // The lip where the top breaks over into the face.
      dot(0, 0, CLIFF.o, CELL, 1);
      dot(0, 1, CLIFF.rimL, CELL, 1);
      dot(0, 2, CLIFF.topD, CELL, 1);
    }
    if (lower) {
      dot(0, CELL - 2, CLIFF.ridgeD, CELL, 1);
      dot(0, CELL - 1, CLIFF.o, CELL, 1);
    }
    if (open(at(-1, 0))) dot(0, 0, CLIFF.o, 1, CELL);
    if (open(at(1, 0))) dot(CELL - 1, 0, CLIFF.o, 1, CELL);
    return;
  }
  dot(0, 0, CLIFF.top, CELL, CELL);
  if ((h & 3) === 0) {
    dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 1, CLIFF.topL, 2, 1);
    dot((h >> 2) % 6 + 1, (h >> 4) % 6 + 2, CLIFF.topD, 2, 1);
  } else if ((h & 7) === 5) dot((h >> 3) % 6 + 1, (h >> 1) % 6 + 1, CLIFF.topD, 1, 2);
  if ((h & 31) === 12) {
    const x = 2 + (h >> 5);
    dot(x, 3, CLIFF.tuft, 1, 2);
    dot(x + 2, 3, CLIFF.tuft, 1, 2);
    dot(x + 1, 4, CLIFF.tuft, 1, 1);
  }
  if (open(at(0, -1))) {
    dot(0, 0, CLIFF.o, CELL, 1);
    dot(0, 1, CLIFF.rimL, CELL, 1);
  }
  if (open(at(-1, 0))) {
    dot(0, 0, CLIFF.o, 1, CELL);
    dot(1, 0, CLIFF.rimL, 1, CELL);
  }
  if (open(at(1, 0))) {
    dot(CELL - 1, 0, CLIFF.o, 1, CELL);
    dot(CELL - 2, 0, CLIFF.ridgeD, 1, CELL);
  }
}

/** Roofs: shingle courses (tile, slate or thatch), a ridge along the top, a dark eave along the bottom. */
function paintRoof(
  ctx: CanvasRenderingContext2D,
  t: number,
  sw: Swatch,
  at: (dx: number, dy: number) => number,
  cx: number,
  cy: number,
  px: number,
  py: number,
  h: number,
  same: (n: number) => boolean = isRoof,
): void {
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };
  for (let y = 0; y < CELL; y++) {
    const wy = cy * CELL + y;
    if (t === Tile.RoofThatch) {
      // Straw lies in long wavy bundles.
      if (wy % 3 === 2) dot(0, y, sw.b, CELL, 1);
      else if (wy % 3 === 0) {
        for (let x = 0; x < CELL; x++) if (((cx * CELL + x + (wy >> 1)) & 3) === 0) dot(x, y, sw.a);
      }
      continue;
    }
    const course = Math.floor(wy / 4);
    if (wy % 4 === 3) dot(0, y, sw.b, CELL, 1);
    else {
      const off = (course & 1) * (t === Tile.RoofSlate ? 3 : 2);
      const span = t === Tile.RoofSlate ? 6 : 4;
      for (let x = 0; x < CELL; x++) {
        const wx = cx * CELL + x + off;
        if (wx % span === 0) dot(x, y, sw.b);
        else if (wy % 4 === 0 && wx % span === 1) dot(x, y, sw.a);
      }
    }
  }
  if ((h & 15) === 6 && t !== Tile.RoofThatch) dot(2 + (h >> 4) % 4, 1 + (h >> 6) % 2, sw.a, 2, 1);
  if (!same(at(0, -1))) {
    // Ridge.
    dot(0, 0, "#1a1420", CELL, 1);
    dot(0, 1, sw.a, CELL, 2);
    dot(0, 3, sw.b, CELL, 1);
  }
  if (!same(at(0, 1))) {
    dot(0, CELL - 2, sw.b, CELL, 1);
    dot(0, CELL - 1, "#1a1420", CELL, 1);
  }
  if (!same(at(-1, 0))) {
    dot(0, 0, "#1a1420", 1, CELL);
    dot(1, 0, sw.a, 1, CELL);
  }
  if (!same(at(1, 0))) {
    dot(CELL - 1, 0, "#1a1420", 1, CELL);
    dot(CELL - 2, 0, sw.b, 1, CELL);
  }
}

/** House walls: plaster or brick, shaded under the eave, a plinth at the foot, a window every few cells. */
function paintHouseWall(ctx: CanvasRenderingContext2D, t: number, sw: Swatch, at: (dx: number, dy: number) => number, cx: number, cy: number, px: number, py: number, h: number): void {
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };
  if (t === Tile.BrickWall) {
    for (let y = 0; y < CELL; y++) {
      const wy = cy * CELL + y;
      if (wy % 3 === 2) dot(0, y, sw.b, CELL, 1);
      else for (let x = 0; x < CELL; x++) if ((cx * CELL + x + ((wy / 3) & 1) * 2) % 4 === 0) dot(x, y, sw.b);
    }
  } else if ((h & 7) === 1) dot((h >> 3) % 6 + 1, (h >> 5) % 6 + 1, sw.a, 2, 1);
  const n = at(0, -1);
  const s = at(0, 1);
  const wallN = isHouseWall(n);
  const wallS = isHouseWall(s);
  // Windows sit in the middle course of a three-course wall, every fifth cell, never at a corner.
  if (wallN && wallS && cx % 5 === 2 && isHouseWall(at(-1, 0)) && isHouseWall(at(1, 0))) {
    // A cottage window: a frame, four panes, one catching the sky, a sill, and a lintel in shadow.
    dot(0, -2, "#2a1c14", CELL, 1);
    dot(0, -1, "#3a2a1e", 1, 8);
    dot(CELL - 1, -1, "#3a2a1e", 1, 8);
    dot(1, -1, "#243450", 6, 7);
    dot(1, -1, "#4a6c9c", 3, 3);
    dot(4, -1, "#3a5a88", 2, 3);
    dot(1, 3, "#3a5a88", 3, 3);
    dot(4, 3, "#4a6c9c", 2, 3);
    dot(1, -1, "#a8c8f0", 1, 1);
    dot(2, 0, "#a8c8f0", 1, 1);
    dot(3, -1, "#6e4a2c", 1, 7);
    dot(1, 2, "#6e4a2c", 6, 1);
    dot(0, 6, "#e8dcc0", CELL, 1);
    dot(0, 7, "#00000030", CELL, 1);
  }
  if (isRoof(n)) dot(0, 0, "#00000040", CELL, 2);
  if (!wallS) {
    // Plinth.
    dot(0, CELL - 2, t === Tile.BrickWall ? "#5a3026" : "#8a7e6a", CELL, 1);
    dot(0, CELL - 1, "#1a1420", CELL, 1);
  }
  if (!isHouseWall(at(-1, 0))) {
    dot(0, 0, "#1a1420", 1, CELL);
    dot(1, 0, t === Tile.BrickWall ? sw.a : "#6e4a2c", 1, CELL);
  }
  if (!isHouseWall(at(1, 0))) {
    dot(CELL - 1, 0, "#1a1420", 1, CELL);
    dot(CELL - 2, 0, t === Tile.BrickWall ? sw.b : "#6e4a2c", 1, CELL);
  }
}

export type StripRef = { y: number; canvas: HTMLCanvasElement; sy: number; dx: number; dy: number };

export class TileCache {
  private grid: Grid | null = null;
  private readonly chunks = new Map<number, Chunk>();
  private clock = 0;
  built = 0;
  buildMs = 0;

  private outdoor = false;

  setGrid(grid: Grid, outdoorZone = false): void {
    this.grid = grid;
    this.outdoor = outdoorZone;
    this.chunks.clear();
  }

  get size(): number {
    return this.chunks.size;
  }

  invalidate(cx: number, cy: number, w: number, h: number): void {
    if (!this.grid) return;
    const across = Math.ceil(this.grid.w / CHUNK_CELLS);
    // Two extra cells each way: neighbours shape corners, edges, shadows and which tree shows a crown.
    for (let y = Math.floor((cy - 2) / CHUNK_CELLS); y <= Math.floor((cy + h + 1) / CHUNK_CELLS); y++) {
      for (let x = Math.floor((cx - 2) / CHUNK_CELLS); x <= Math.floor((cx + w + 1) / CHUNK_CELLS); x++) this.chunks.delete(y * across + x);
    }
  }

  private chunk(grid: Grid, x: number, y: number, across: number): Chunk {
    const key = y * across + x;
    let chunk = this.chunks.get(key);
    if (!chunk) {
      const t0 = performance.now();
      outdoor = this.outdoor;
      chunk = buildChunk(grid, x, y);
      this.buildMs = performance.now() - t0;
      this.chunks.set(key, chunk);
      this.built++;
    }
    chunk.used = this.clock;
    return chunk;
  }

  /** Draw the ground of every chunk overlapping the view rectangle (world px). */
  draw(ctx: CanvasRenderingContext2D, viewX: number, viewY: number, viewW: number, viewH: number): void {
    const grid = this.grid;
    if (!grid) return;
    this.clock++;
    const across = Math.ceil(grid.w / CHUNK_CELLS);
    const down = Math.ceil(grid.h / CHUNK_CELLS);
    const x0 = Math.max(0, Math.floor(viewX / CHUNK_PX));
    const y0 = Math.max(0, Math.floor(viewY / CHUNK_PX));
    const x1 = Math.min(across - 1, Math.floor((viewX + viewW) / CHUNK_PX));
    const y1 = Math.min(down - 1, Math.floor((viewY + viewH) / CHUNK_PX));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) ctx.drawImage(this.chunk(grid, x, y, across).canvas, x * CHUNK_PX - viewX, y * CHUNK_PX - viewY);
    }
    if (this.chunks.size > MAX_CHUNKS) this.evict();
  }

  /**
   * The rows of standing things (trees, bushes, stones, fences) that reach into the view, each with the
   * world y of its row's cell bottom so the renderer can sort it among units and props. Call after draw().
   */
  strips(viewX: number, viewY: number, viewW: number, viewH: number, out: StripRef[]): StripRef[] {
    out.length = 0;
    const grid = this.grid;
    if (!grid) return out;
    const across = Math.ceil(grid.w / CHUNK_CELLS);
    const down = Math.ceil(grid.h / CHUNK_CELLS);
    const x0 = Math.max(0, Math.floor((viewX - STRIP_MX) / CHUNK_PX));
    const x1 = Math.min(across - 1, Math.floor((viewX + viewW + STRIP_MX) / CHUNK_PX));
    // A row draws upward from its cell bottom, so rows up to a strip's height under the view still reach it.
    const r0 = Math.max(0, Math.floor(viewY / CELL) - 1);
    const r1 = Math.min(grid.h - 1, Math.floor((viewY + viewH + STRIP_H) / CELL));
    for (let cy = Math.floor(r0 / CHUNK_CELLS); cy <= Math.min(down - 1, Math.floor(r1 / CHUNK_CELLS)); cy++) {
      for (let cx = x0; cx <= x1; cx++) {
        const chunk = this.chunk(grid, cx, cy, across);
        if (!chunk.strips) continue;
        for (let r = 0; r < CHUNK_CELLS; r++) {
          if ((chunk.rows & (1 << r)) === 0) continue;
          const row = cy * CHUNK_CELLS + r;
          if (row < r0 || row > r1) continue;
          const bottom = (row + 1) * CELL;
          out.push({ y: bottom, canvas: chunk.strips, sy: r * STRIP_H, dx: cx * CHUNK_PX - STRIP_MX - viewX, dy: bottom + STRIP_BELOW - STRIP_H - viewY });
        }
      }
    }
    return out;
  }

  private evict(): void {
    const byAge = [...this.chunks.entries()].sort((a, b) => a[1].used - b[1].used);
    for (let i = 0; i < byAge.length - MAX_CHUNKS * 0.75; i++) this.chunks.delete(byAge[i][0]);
  }
}

/** Is she standing where a crown to the south of her would hide her? */
export function underCanopy(grid: Grid, x: number, y: number): boolean {
  const cx = Math.floor(x / CELL);
  const cy = Math.floor(y / CELL);
  for (let dy = 1; dy <= 4; dy++) {
    for (let dx = -2; dx <= 2; dx++) {
      const t = grid.tileAt(cx + dx, cy + dy);
      if (t === Tile.Tree || t === Tile.Pine) return true;
    }
  }
  return false;
}
