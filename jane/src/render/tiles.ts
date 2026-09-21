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
};

/** One flat colour per tile id, for the minimap. */
export const TILE_COLORS: string[] = Array.from({ length: TILE_COUNT }, (_, t) => SW[t as Tile]?.base ?? "#000");

/** Stable per-cell hash, 0..255. Drawing must not depend on draw order or time. */
function cellHash(x: number, y: number): number {
  let h = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1);
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b);
  return (h ^ (h >>> 13)) & 255;
}

const isWallLike = (t: number): boolean =>
  t === Tile.Wall || t === Tile.CaveWall || t === Tile.TempleWall || t === Tile.Cliff || t === Tile.WallTop || t === Tile.Void;

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
      if ((h & 3) === 0) dot(h % 7, (h >> 3) % 7, sw.a);
      if ((h & 7) === 5) dot((h >> 2) % 7, (h >> 4) % 7, sw.b);
      if ((h & 63) === 9) dot(3, 3, "#f0d048");
      break;
    case Tile.GrassTall:
      dot(1 + (h % 2), 2, sw.a, 1, 3);
      dot(4 + ((h >> 2) % 2), 1, sw.a, 1, 4);
      dot(6, 4, sw.b, 1, 3);
      dot(2, 6, sw.b, 1, 2);
      break;
    case Tile.Dirt:
    case Tile.CaveFloor:
    case Tile.DryBed:
    case Tile.Sand:
      if ((h & 3) === 1) dot(h % 7, (h >> 3) % 7, sw.b);
      if ((h & 7) === 2) dot((h >> 2) % 6, (h >> 5) % 6, sw.a, 2, 1);
      break;
    case Tile.Road:
    case Tile.Cobble:
      dot(0, (h & 1) * 4, sw.b, CELL, 1);
      dot((h >> 1) % 6 + 1, 0, sw.b, 1, 4);
      dot((h >> 3) % 6 + 1, 4, sw.b, 1, 4);
      if ((h & 7) === 0) dot(2, 2, sw.a, 2, 1);
      break;
    case Tile.Water: {
      dot((h % 5) + 1, 2, sw.a, 2, 1);
      dot(((h >> 3) % 5) + 1, 6, sw.b, 2, 1);
      // Shoreline: a pale lip where water meets land to the north.
      if ((TILE_FLAGS[grid.tileAt(cx, cy - 1)] & 4) === 0) dot(0, 0, "#9cc8f0", CELL, 1);
      break;
    }
    case Tile.Bush:
      dot(1, 1, sw.a, 6, 5);
      dot(2, 2, sw.base, 3, 2);
      dot(1, 6, sw.b, 6, 1);
      break;
    case Tile.Tree: {
      const southTree = grid.tileAt(cx, cy + 1) === Tile.Tree;
      dot(0, 0, sw.base, CELL, CELL);
      dot((h % 4) + 1, (h >> 2) % 4, sw.a, 3, 2);
      dot((h >> 4) % 5, ((h >> 1) % 3) + 4, sw.b, 3, 2);
      if (!southTree) {
        dot(0, 6, sw.b, CELL, 2);
        dot(3, 5, "#6e4a2c", 2, 3);
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
      dot(0, 0, sw.b, CELL, 1);
      dot(0, 0, sw.b, 1, CELL);
      if ((h & 15) === 1) dot(3, 3, sw.a, 2, 2);
      if ((h & 31) === 7) dot(2, 5, sw.b, 4, 1);
      break;
    case Tile.FloorWood:
      dot(0, 7, sw.b, CELL, 1);
      dot((h % 6) + 1, 2, sw.a, 2, 1);
      if ((cx & 3) === 0) dot(0, 0, sw.b, 1, CELL);
      break;
    case Tile.Moss:
      dot(1, 1, sw.a, 3, 2);
      dot(4, 4, sw.b, 3, 3);
      break;
    case Tile.Garden:
      dot(0, 3, sw.b, CELL, 1);
      if ((h & 3) === 0) dot(h % 6, 5, sw.a, 2, 2);
      if ((h & 7) === 3) dot((h >> 3) % 6, 1, sw.a, 1, 2);
      break;
    case Tile.Track:
    case Tile.Rail:
      dot(0, 2, sw.a, CELL, 1);
      dot(0, 5, sw.a, CELL, 1);
      dot(1, 1, sw.b, 1, 6);
      dot(5, 1, sw.b, 1, 6);
      break;
    case Tile.Rubble:
      dot(1, 2, sw.a, 3, 3);
      dot(4, 4, sw.a, 3, 2);
      dot(2, 5, sw.b, 2, 2);
      break;
    case Tile.GrownPath:
      dot(1, 1, sw.a, 6, 6);
      dot(3, 3, sw.b, 2, 2);
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
