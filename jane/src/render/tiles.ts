// Terrain. Tiles are painted procedurally (hash of the cell picks the variant,
// neighbours decide wall faces and shorelines) into 16x16-cell chunk canvases.
// Chunks are built on demand, kept in a small LRU, and dropped when far away:
// "one render-texture will not survive a 5120x3072 county" (WORLDGEN.md).
// A tile change invalidates only the chunks its rect touches.

import { CELL } from "@/sim/constants";
import { F_SOLID, Tile, TILE_COUNT, TILE_FLAGS, type Grid } from "@/sim/grid";

export const CHUNK_CELLS = 16;
export const CHUNK_PX = CHUNK_CELLS * CELL;
const MAX_CHUNKS = 96;

type Swatch = { base: string; a: string; b: string };
const SW: Partial<Record<Tile, Swatch>> = {
  [Tile.Void]: { base: "#0b0a10", a: "#0b0a10", b: "#0b0a10" },
  [Tile.Grass]: { base: "#4f9a4a", a: "#5aa854", b: "#468c44" },
  [Tile.GrassTall]: { base: "#458f44", a: "#6ab85a", b: "#2f7238" },
  [Tile.Dirt]: { base: "#9a7a52", a: "#a88860", b: "#866a46" },
  [Tile.Road]: { base: "#b8a888", a: "#c8b898", b: "#9c8e72" },
  [Tile.Water]: { base: "#3a78c0", a: "#4c8cd0", b: "#2e64a8" },
  [Tile.Sand]: { base: "#d8c890", a: "#e4d6a2", b: "#c4b47c" },
  [Tile.Bush]: { base: "#3c8844", a: "#52a058", b: "#2a6838" },
  [Tile.Tree]: { base: "#2a6838", a: "#3c8844", b: "#1c4c2c" },
  [Tile.Fence]: { base: "#4f9a4a", a: "#a87848", b: "#6e4a2c" },
  [Tile.HouseWall]: { base: "#d8c8a8", a: "#e8dcc0", b: "#a89878" },
  [Tile.HouseRoof]: { base: "#a84838", a: "#c05848", b: "#7e3228" },
  [Tile.Floor]: { base: "#8c8478", a: "#989084", b: "#7c7468" },
  [Tile.FloorWood]: { base: "#b08858", a: "#bc9464", b: "#96703f" },
  [Tile.Wall]: { base: "#3a3644", a: "#5c5668", b: "#2a2632" },
  [Tile.WallTop]: { base: "#2a2632", a: "#3a3644", b: "#1e1b26" },
  [Tile.CaveFloor]: { base: "#6a5a48", a: "#766654", b: "#5a4c3c" },
  [Tile.CaveWall]: { base: "#2c2420", a: "#4a3c32", b: "#1e1814" },
  [Tile.TempleFloor]: { base: "#5a6470", a: "#66707c", b: "#4c5660" },
  [Tile.TempleWall]: { base: "#232a36", a: "#3c4656", b: "#181d26" },
  [Tile.Moss]: { base: "#4c6a58", a: "#5c7c64", b: "#3e5a4a" },
  [Tile.DryBed]: { base: "#7a7060", a: "#867c6a", b: "#6a6152" },
  [Tile.Garden]: { base: "#5a4a34", a: "#6a8a48", b: "#4a3c2a" },
  [Tile.Rubble]: { base: "#5a5048", a: "#7a7068", b: "#3e3830" },
  [Tile.Track]: { base: "#6a5a48", a: "#8a8f98", b: "#4a3626" },
  [Tile.GrownPath]: { base: "#3c8844", a: "#78c850", b: "#2a6838" },
  [Tile.Cobble]: { base: "#7c7c84", a: "#8e8e96", b: "#64646c" },
  [Tile.Rail]: { base: "#9a7a52", a: "#8a8f98", b: "#4a3626" },
  [Tile.Cliff]: { base: "#6a5c50", a: "#8a7a68", b: "#4a3e36" },
  [Tile.Sill]: { base: "#5e5040", a: "#8a7a68", b: "#3e3428" },
  [Tile.Glass]: { base: "#6f8f96", a: "#c8e4e8", b: "#4a666e" },
  [Tile.Hedge]: { base: "#2a5a3c", a: "#3f7a4c", b: "#1c3e2a" },
  [Tile.Ice]: { base: "#b4dcea", a: "#e4f6fa", b: "#8cbcd0" },
  [Tile.MuseumFloor]: { base: "#9a8e7c", a: "#ab9f8c", b: "#7e7464" },
  [Tile.MuseumWall]: { base: "#4a3e44", a: "#6a5a60", b: "#30282e" },
  [Tile.PipeFloor]: { base: "#565e5c", a: "#687270", b: "#3e4644" },
  [Tile.PipeWall]: { base: "#2c3436", a: "#48565a", b: "#1c2224" },
  [Tile.WorksFloor]: { base: "#6c6660", a: "#7e7870", b: "#524c48" },
  [Tile.WorksWall]: { base: "#3a3230", a: "#5a4c46", b: "#261f1e" },
  [Tile.SchoolFloor]: { base: "#8a6a4a", a: "#9c7c58", b: "#6a4e34" },
  [Tile.SchoolWall]: { base: "#3e4a44", a: "#5a6a60", b: "#28322e" },
};

/** One flat colour per tile id, for the minimap and for feathering one tile into another. */
export const TILE_COLORS: string[] = Array.from({ length: TILE_COUNT }, (_, t) => SW[t as Tile]?.base ?? "#000");

/**
 * Ground materials that feather into each other where they meet. Anything not on this
 * list keeps a hard edge: a wall, a floor indoors and a pane of glass all want one.
 */
const SOFT_TILES: Tile[] = [
  Tile.Grass,
  Tile.GrassTall,
  Tile.Dirt,
  Tile.Road,
  Tile.Sand,
  Tile.Water,
  Tile.Moss,
  Tile.DryBed,
  Tile.Garden,
  Tile.Rubble,
  Tile.Track,
  Tile.GrownPath,
  Tile.Cobble,
  Tile.Rail,
];
const SOFT = Uint8Array.from({ length: TILE_COUNT }, (_, t) => (SOFT_TILES.includes(t as Tile) ? 1 : 0));

/** Stable per-cell hash, 0..255. Drawing must not depend on draw order or time. */
function cellHash(x: number, y: number): number {
  let h = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1);
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b);
  return (h ^ (h >>> 13)) & 255;
}

const isWallLike = (t: number): boolean =>
  t === Tile.Wall || t === Tile.CaveWall || t === Tile.TempleWall || t === Tile.Cliff || t === Tile.WallTop || t === Tile.Void;

/** A shadow that works on any ground, because it is a darkening rather than a colour. */
const SHADE = "#00000026";
const SCUFF = "#00000018";

/**
 * Small char grids for the things that grow. "a" is the swatch's light tone, "b" its
 * dark, "c" its base and "s" a soft shadow; "." leaves the ground showing. A hash picks
 * one per cell, which is the whole point: one shape repeated is what makes a field look
 * cheap. Drawn as horizontal runs, so a full cell is a handful of fillRects, not 64.
 */
function stamp(
  ctx: CanvasRenderingContext2D,
  px: number,
  py: number,
  sw: Swatch,
  rows: readonly string[],
  ox: number,
  oy: number,
): void {
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

/** Tall grass: a clump of blades, none of them the same length, none of them centred. */
const GRASS_TUFT: readonly string[][] = [
  ["..a...", ".aa.a.", "a.a.a.", "a.cab.", ".bcab.", "..ss.."],
  ["a.....", "a..a..", "a..a.b", ".a.ab.", ".ca.b.", "..ss.."],
  ["......", ".a.a..", "aa.aab", "ac.cab", ".cccb.", "..ss.."],
  ["....a.", "...a..", "b..a..", "b.a.a.", ".ba.ab", "..sss."],
];

/** A stray blade or two, dropped somewhere else in the cell to break the grid up. */
const GRASS_BLADE: readonly string[][] = [
  ["a..", "a..", "a.b", ".ab", ".s."],
  ["..a", ".a.", ".a.", "ba.", ".s."],
  [".a.", ".a.", "ba.", "b.a", "ss."],
];

/**
 * A clump of leaves lit from the top-left. Bush, hedge and the tree canopy all use these:
 * the highlight sits in a different place in each one, so a mass of them has no grain.
 */
const LEAF: readonly string[][] = [
  ["..aaa...", ".aaaac..", "aaaccccb", "aaccccbb", ".cccccbb", "..cccbb.", "...bbb..", "....b..."],
  ["...aaa..", "..aaaac.", ".aaacccc", ".acccccc", "bccccccc", "bbcccbb.", ".bbbbb..", "..bb...."],
  [".....aa.", "...aaaa.", "..caaac.", ".ccccccb", "bcccccbb", "bbcccbb.", ".bbbb...", "..bb...."],
  ["........", "..aaa...", ".aaaac..", ".acccbb.", "..cccb..", "...bb...", "........", "........"],
  ["..aa..a.", ".aaa.aac", "aaccccc.", "acccccbb", ".ccccbb.", "b.cccb..", "bb.bbb..", ".b......"],
  ["........", ".aaaaa..", "aaaacccb", "acccccbb", ".ccccbb.", "..cbbb..", "...b....", "........"],
];

/** Moss grows in patches with nothing between them, not in a checkerboard. */
const MOSS_PATCH: readonly string[][] = [
  [".aa.....", "aaa..bb.", ".a..bbbb", "....bb..", "..aa....", ".aaa..b.", "..a..bbb", "......b."],
  ["....aa..", "..b.aaa.", ".bbb.a..", ".bb.....", "aa...bb.", "aaa.bbb.", ".a...b..", "....aa.."],
  ["..bb....", ".bbb.aa.", ".b..aaa.", ".....a..", "..aa..bb", ".aaa.bbb", "..a...b.", "bb......"],
  [".....a..", "bb..aaa.", "bbb..a..", ".b......", ".aa..bb.", "aaa.bbbb", ".a...bb.", "...aa..."],
];

/**
 * Feather a cell into its neighbours. Every material bleeds a few pixels of itself over
 * the seam, so grass meeting a road is a ragged line rather than an 8 px step. Neighbour
 * ids are read straight into locals and the tone comes from TILE_COLORS, an array, so the
 * hot path does no object lookups and allocates nothing (ENGINE.md 8.2).
 */
function feather(ctx: CanvasRenderingContext2D, grid: Grid, t: number, cx: number, cy: number, px: number, py: number, h: number): void {
  const e = CELL - 1;
  let n = grid.tileAt(cx, cy - 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px + (h & 3), py, 4, 1);
    ctx.fillRect(px + 2 + ((h >> 2) & 3), py + 1, 2, 1);
    if ((h & 64) === 0) ctx.fillRect(px + ((h >> 4) & 7), py + 2, 1, 1);
  }
  n = grid.tileAt(cx, cy + 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px + ((h >> 1) & 3), py + e, 4, 1);
    ctx.fillRect(px + 2 + ((h >> 3) & 3), py + e - 1, 2, 1);
    if ((h & 32) === 0) ctx.fillRect(px + ((h >> 5) & 7), py + e - 2, 1, 1);
  }
  n = grid.tileAt(cx - 1, cy);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px, py + (h & 3), 1, 4);
    ctx.fillRect(px + 1, py + 2 + ((h >> 4) & 3), 1, 2);
    if ((h & 16) === 0) ctx.fillRect(px + 2, py + ((h >> 2) & 7), 1, 1);
  }
  n = grid.tileAt(cx + 1, cy);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px + e, py + ((h >> 2) & 3), 1, 4);
    ctx.fillRect(px + e - 1, py + 2 + ((h >> 5) & 3), 1, 2);
    if ((h & 8) === 0) ctx.fillRect(px + e - 2, py + ((h >> 3) & 7), 1, 1);
  }
  // Corners, where three materials meet and the step would otherwise be a staircase.
  n = grid.tileAt(cx - 1, cy - 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px, py, 1 + (h & 1), 1);
  }
  n = grid.tileAt(cx + 1, cy - 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px + e - ((h >> 1) & 1), py, 1 + ((h >> 1) & 1), 1);
  }
  n = grid.tileAt(cx - 1, cy + 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px, py + e, 1 + ((h >> 2) & 1), 1);
  }
  n = grid.tileAt(cx + 1, cy + 1);
  if (n !== t && SOFT[n] === 1) {
    ctx.fillStyle = TILE_COLORS[n];
    ctx.fillRect(px + e - ((h >> 3) & 1), py + e, 1 + ((h >> 3) & 1), 1);
  }
}

function paintCell(ctx: CanvasRenderingContext2D, grid: Grid, cx: number, cy: number, px: number, py: number): void {
  const t = grid.tileAt(cx, cy) as Tile;
  const sw = SW[t] ?? SW[Tile.Void]!;
  const h = cellHash(cx, cy);
  ctx.fillStyle = sw.base;
  ctx.fillRect(px, py, CELL, CELL);
  const dot = (x: number, y: number, c: string, w = 1, hh = 1): void => {
    ctx.fillStyle = c;
    ctx.fillRect(px + x, py + y, w, hh);
  };

  if (isWallLike(t)) {
    // A wall whose south neighbour is open shows its front face; otherwise only its top.
    const southOpen = (TILE_FLAGS[grid.tileAt(cx, cy + 1)] & F_SOLID) === 0;
    if (southOpen && t !== Tile.Void) {
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
    }
    return;
  }

  switch (t) {
    case Tile.Grass:
      // A short blade rather than a stray dot: grass wants grain, not dust.
      dot(h % 6, (h >> 3) % 6, sw.a, 1, 2);
      dot((h % 6) + 1, ((h >> 3) % 6) + 1, sw.a, 1, 1);
      if ((h & 3) === 0) dot((h >> 2) % 6, ((h >> 5) % 6) + 1, sw.b, 2, 1);
      if ((h & 7) === 5) dot((h >> 1) % 7, (h >> 4) % 7, sw.b);
      if ((h & 63) === 9) {
        dot(3, 3, "#f0d048");
        dot(4, 3, "#b08828");
      }
      break;
    case Tile.GrassTall:
      stamp(ctx, px, py, sw, GRASS_TUFT[h & 3], h % 3, (h >> 2) % 3);
      // Not every cell gets the second blade, so the field thins and thickens.
      if ((h & 3) !== 0) stamp(ctx, px, py, sw, GRASS_BLADE[(h >> 5) % 3], h % 6, (h >> 4) % 4);
      break;
    case Tile.Dirt: {
      // The road's verge. Scuffed: patches of bare and trodden earth, the odd stone.
      dot(h % 6, (h >> 3) % 6, sw.a, 2 + (h & 1), 1);
      dot((h >> 2) % 6, (h >> 5) % 6, sw.b, 2, 1);
      dot((h >> 1) % 7, (h >> 4) % 7, sw.b);
      if ((h & 7) === 0) dot((h >> 3) % 6, (h >> 1) % 6, SCUFF, 3, 2);
      if ((h & 15) === 5) {
        dot(2, 4, "#c8b898", 2, 1);
        dot(2, 5, sw.b, 2, 1);
      }
      break;
    }
    case Tile.CaveFloor:
    case Tile.DryBed:
    case Tile.Sand:
      if ((h & 3) === 1) dot(h % 7, (h >> 3) % 7, sw.b);
      if ((h & 7) === 2) dot((h >> 2) % 6, (h >> 5) % 6, sw.a, 2, 1);
      break;
    case Tile.Road: {
      // Which way the road runs. The carriageway is four cells across, so only the axis
      // it travels along has road two cells out, which makes this one cheap question.
      const alongX = grid.tileAt(cx - 2, cy) === Tile.Road && grid.tileAt(cx + 2, cy) === Tile.Road;
      // How far in from the near verge. The ruts go on the second and third cell across,
      // so they run the length of the road; a rut drawn in every cell is corduroy.
      let inset = 0;
      if (alongX) while (inset < 4 && grid.tileAt(cx, cy - 1 - inset) === Tile.Road) inset++;
      else while (inset < 4 && grid.tileAt(cx - 1 - inset, cy) === Tile.Road) inset++;
      // Metalling: loose stone, a pit or two, and now and then a set stone showing through.
      dot(h % 6, (h >> 3) % 6, sw.a, 2, 1);
      dot((h >> 2) % 7, (h >> 5) % 7, sw.b);
      dot((h >> 1) % 7, (h >> 4) % 7, sw.b);
      dot((h >> 4) % 6, (h >> 2) % 6, sw.b, 2, 1);
      if ((h & 3) === 1) dot((h >> 1) % 6, (h >> 5) % 6, sw.a, 1, 2);
      if ((h & 7) === 3) {
        dot((h >> 2) % 6, (h >> 4) % 6, "#d8ccb0", 2, 1);
        dot((h >> 2) % 6, ((h >> 4) % 6) + 1, sw.b, 2, 1);
      }
      if (inset === 1 || inset === 2) {
        // The rut wanders a pixel from cell to cell, the way a rut does.
        if (alongX) {
          const ry = 3 + (h & 1);
          dot(0, ry, sw.b, CELL, 1);
          dot(0, ry, SHADE, CELL, 1);
          dot((h >> 2) % 5, ry - 1, sw.b, 3, 1);
        } else {
          const rx = 3 + (h & 1);
          dot(rx, 0, sw.b, 1, CELL);
          dot(rx, 0, SHADE, 1, CELL);
          dot(rx - 1, (h >> 2) % 5, sw.b, 1, 3);
        }
      }
      // The made edge, where the metalling stops and the verge begins. Broken by a gap,
      // so a road that wanders by a cell does not show its staircase.
      if (grid.tileAt(cx, cy - 1) !== Tile.Road) {
        dot(0, 0, sw.b, CELL, 1);
        dot(2 + (h & 3), 0, sw.a, 2, 1);
      }
      if (grid.tileAt(cx, cy + 1) !== Tile.Road) {
        dot(0, CELL - 1, sw.b, CELL, 1);
        dot(2 + ((h >> 2) & 3), CELL - 1, sw.a, 2, 1);
      }
      if (grid.tileAt(cx - 1, cy) !== Tile.Road) {
        dot(0, 0, sw.b, 1, CELL);
        dot(0, 2 + ((h >> 4) & 3), sw.a, 1, 2);
      }
      if (grid.tileAt(cx + 1, cy) !== Tile.Road) {
        dot(CELL - 1, 0, sw.b, 1, CELL);
        dot(CELL - 1, 2 + ((h >> 3) & 3), sw.a, 1, 2);
      }
      break;
    }
    case Tile.Cobble: {
      // Setts in two courses, the lower one offset half a stone. The whole pattern shifts
      // by two pixels on alternate cells as well, or the joints line up into a grid.
      const j = ((cx + cy) & 1) === 0 ? 0 : 2;
      dot(0, 3, sw.b, CELL, 1);
      dot(0, CELL - 1, sw.b, CELL, 1);
      dot(1 + j, 0, sw.b, 1, 3);
      dot(5 + j, 0, sw.b, 1, 3);
      dot(3 - j, 4, sw.b, 1, 3);
      dot(7 - j, 4, sw.b, 1, 3);
      // Every sett catches the light along its top edge.
      dot(0, 0, sw.a, 1 + j, 1);
      dot(2 + j, 0, sw.a, 3, 1);
      dot(6 + j, 0, sw.a, 2 - j, 1);
      dot(0, 4, sw.a, 3 - j, 1);
      dot(4 - j, 4, sw.a, 3, 1);
      dot(8 - j, 4, sw.a, j, 1);
      // One sett a cell is a different stone.
      if ((h & 3) === 0) dot((h & 4) === 0 ? j : 4 + j, 1, sw.b, 3, 2);
      if ((h & 7) === 5) dot(4 - j, 5, sw.a, 3, 2);
      break;
    }
    case Tile.Water: {
      dot((h % 5) + 1, 2, sw.a, 2, 1);
      dot(((h >> 3) % 5) + 1, 6, sw.b, 2, 1);
      // Shoreline: a pale lip where water meets land to the north.
      if ((TILE_FLAGS[grid.tileAt(cx, cy - 1)] & 4) === 0) dot(0, 0, "#9cc8f0", CELL, 1);
      break;
    }
    case Tile.Hedge:
    case Tile.Bush:
      stamp(ctx, px, py, sw, LEAF[h % 6], 0, 0);
      break;
    case Tile.Tree: {
      stamp(ctx, px, py, sw, LEAF[h % 6], 0, 0);
      // A wood is a mass, so it is lit along the top of its canopy and dark at the foot.
      if (grid.tileAt(cx, cy - 1) !== Tile.Tree) dot(0, 0, sw.a, CELL, 1);
      if (grid.tileAt(cx, cy + 1) !== Tile.Tree) {
        dot(0, 5, sw.b, CELL, 3);
        dot(0, 5, SHADE, CELL, 3);
        // Only about half the cells show a trunk, or the tree line is a picket fence.
        if ((h & 1) === 0) {
          const tx = 2 + (h % 3);
          dot(tx, 4, "#6e4a2c", 2, 4);
          dot(tx, 4, "#a87848", 1, 3);
        }
      }
      break;
    }
    case Tile.Fence:
      dot(0, 3, sw.a, CELL, 1);
      dot(0, 5, sw.a, CELL, 1);
      dot(3, 1, sw.b, 2, 6);
      dot(3, 1, sw.a, 1, 5);
      break;
    case Tile.HouseWall:
      dot(0, 0, sw.b, CELL, 1);
      // Windows only on the middle course of a wall strip, one every eight cells.
      if ((cx & 7) === 3 && grid.tileAt(cx, cy - 1) === Tile.HouseWall && grid.tileAt(cx, cy + 1) === Tile.HouseWall) {
        dot(2, 2, "#5a7aa8", 4, 4);
        dot(2, 2, "#1a1420", 4, 1);
        dot(4, 2, "#1a1420", 1, 4);
      }
      dot(0, 7, sw.b, CELL, 1);
      break;
    case Tile.HouseRoof:
      dot(0, (cy & 1) * 4, sw.b, CELL, 1);
      dot(((cx + cy) & 1) * 4, 0, sw.b, 1, 4);
      if ((h & 7) === 0) dot(2, 2, sw.a, 3, 1);
      break;
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
    case Tile.FloorWood:
    case Tile.SchoolFloor:
      dot(0, 7, sw.b, CELL, 1);
      dot((h % 6) + 1, 2, sw.a, 2, 1);
      if ((cx & 3) === 0) dot(0, 0, sw.b, 1, CELL);
      break;
    case Tile.Moss:
      stamp(ctx, px, py, sw, MOSS_PATCH[h & 3], 0, 0);
      break;
    case Tile.Garden:
      // Turned earth in furrows, with a seedling standing in every other one.
      dot(0, 0, sw.b, CELL, 1);
      dot(0, 4, sw.b, CELL, 1);
      dot((h >> 1) % 7, 2, SCUFF, 2, 1);
      dot((h >> 4) % 7, 6, SCUFF, 2, 1);
      if (((cx + cy) & 1) === 0) {
        const gx = 2 + (h % 3);
        dot(gx, 1, sw.a, 1, 3);
        dot(gx - 1, 2, sw.a, 1, 1);
        dot(gx + 1, 1, sw.a, 1, 1);
      } else {
        const gx = 3 + (h % 3);
        dot(gx, 5, sw.a, 1, 3);
        dot(gx + 1, 6, sw.a, 1, 1);
        dot(gx - 1, 5, sw.a, 1, 1);
      }
      break;
    case Tile.Track:
    case Tile.Rail: {
      // Rails run the way the track runs, sleepers lie across it, ballast under both.
      const alongX = grid.tileAt(cx - 1, cy) === t || grid.tileAt(cx + 1, cy) === t;
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
    case Tile.Rubble:
      dot(1, 2, sw.a, 3, 3);
      dot(4, 4, sw.a, 3, 2);
      dot(2, 5, sw.b, 2, 2);
      break;
    case Tile.GrownPath:
      dot(1, 1, sw.a, 6, 6);
      dot(3, 3, sw.b, 2, 2);
      break;
    case Tile.Glass:
      // A pane: a bright edge top and left, one long glint.
      dot(0, 0, sw.a, CELL, 1);
      dot(0, 0, sw.a, 1, CELL);
      dot(2, 5, sw.a, 1, 1);
      dot(3, 4, sw.a, 1, 1);
      dot(4, 3, sw.a, 1, 1);
      dot(0, 7, sw.b, CELL, 1);
      break;
    case Tile.Ice:
      if ((h & 3) === 0) dot(h % 5, 2, sw.a, 3, 1);
      if ((h & 7) === 5) dot((h >> 2) % 5, 5, sw.b, 2, 1);
      break;
    case Tile.Sill: {
      // A worn threshold: a lip along the side that faces the doorway, studs along the other.
      // It runs across the mouth, so it is drawn along whichever way its neighbours continue it.
      const across = grid.tileAt(cx - 1, cy) === Tile.Sill || grid.tileAt(cx + 1, cy) === Tile.Sill;
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

  if (SOFT[t] === 1) feather(ctx, grid, t, cx, cy, px, py, h);
}

export class TileCache {
  private grid: Grid | null = null;
  private readonly chunks = new Map<number, { canvas: HTMLCanvasElement; used: number }>();
  private clock = 0;
  built = 0;

  setGrid(grid: Grid): void {
    this.grid = grid;
    this.chunks.clear();
  }

  get size(): number {
    return this.chunks.size;
  }

  invalidate(cx: number, cy: number, w: number, h: number): void {
    if (!this.grid) return;
    const across = Math.ceil(this.grid.w / CHUNK_CELLS);
    // One extra cell each way: neighbours influence wall faces and shorelines.
    for (let y = Math.floor((cy - 1) / CHUNK_CELLS); y <= Math.floor((cy + h) / CHUNK_CELLS); y++) {
      for (let x = Math.floor((cx - 1) / CHUNK_CELLS); x <= Math.floor((cx + w) / CHUNK_CELLS); x++) this.chunks.delete(y * across + x);
    }
  }

  /** Draw every chunk overlapping the view rectangle (world px). */
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
      for (let x = x0; x <= x1; x++) {
        const key = y * across + x;
        let chunk = this.chunks.get(key);
        if (!chunk) {
          chunk = { canvas: this.build(grid, x, y), used: this.clock };
          this.chunks.set(key, chunk);
          this.built++;
        }
        chunk.used = this.clock;
        ctx.drawImage(chunk.canvas, x * CHUNK_PX - viewX, y * CHUNK_PX - viewY);
      }
    }
    if (this.chunks.size > MAX_CHUNKS) this.evict();
  }

  private build(grid: Grid, chunkX: number, chunkY: number): HTMLCanvasElement {
    const canvas = document.createElement("canvas");
    canvas.width = CHUNK_PX;
    canvas.height = CHUNK_PX;
    const ctx = canvas.getContext("2d")!;
    for (let j = 0; j < CHUNK_CELLS; j++) {
      for (let i = 0; i < CHUNK_CELLS; i++) paintCell(ctx, grid, chunkX * CHUNK_CELLS + i, chunkY * CHUNK_CELLS + j, i * CELL, j * CELL);
    }
    return canvas;
  }

  private evict(): void {
    const byAge = [...this.chunks.entries()].sort((a, b) => a[1].used - b[1].used);
    for (let i = 0; i < byAge.length - MAX_CHUNKS * 0.75; i++) this.chunks.delete(byAge[i][0]);
  }
}
