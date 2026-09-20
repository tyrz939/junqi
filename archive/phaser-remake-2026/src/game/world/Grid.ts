import { PATH_CELL } from "@/game/constants";

export type Tile =
  | "grass"
  | "wall"
  | "house"
  | "door"
  | "bush"
  | "wood"
  | "stone"
  | "bone"
  | "water";

export type TileFlags = { solid: boolean; blockLos: boolean };

export const TILE_FLAGS: Record<Tile, TileFlags> = {
  grass: { solid: false, blockLos: false },
  door: { solid: false, blockLos: false },
  wood: { solid: false, blockLos: false },
  stone: { solid: false, blockLos: false },
  bone: { solid: false, blockLos: false },
  bush: { solid: true, blockLos: false },
  water: { solid: true, blockLos: true },
  wall: { solid: true, blockLos: true },
  house: { solid: true, blockLos: true },
};

export class Grid {
  readonly cols: number;
  readonly rows: number;
  readonly tiles: Tile[];
  readonly extraSolid = new Set<string>();
  readonly occupants = new Map<string, string>();

  constructor(cols: number, rows: number, fill: Tile = "grass") {
    this.cols = cols;
    this.rows = rows;
    this.tiles = new Array<Tile>(cols * rows);
    this.tiles.fill(fill);
  }

  index(cx: number, cy: number): number {
    return cy * this.cols + cx;
  }

  inBounds(cx: number, cy: number): boolean {
    return cx >= 0 && cy >= 0 && cx < this.cols && cy < this.rows;
  }

  get(cx: number, cy: number): Tile {
    if (!this.inBounds(cx, cy)) return "wall";
    return this.tiles[this.index(cx, cy)];
  }

  set(cx: number, cy: number, tile: Tile): void {
    if (!this.inBounds(cx, cy)) return;
    this.tiles[this.index(cx, cy)] = tile;
  }

  flags(cx: number, cy: number): TileFlags {
    return TILE_FLAGS[this.get(cx, cy)];
  }

  solid(cx: number, cy: number, ignoreId?: string): boolean {
    if (this.extraSolid.has(Grid.cellKey(cx, cy))) return true;
    const who = this.occupants.get(Grid.cellKey(cx, cy));
    if (who && who !== ignoreId) return true;
    return this.flags(cx, cy).solid;
  }

  occupy(id: string, x: number, y: number): void {
    this.clearOccupy(id);
    const { cx, cy } = this.worldToCell(x, y);
    if (this.inBounds(cx, cy)) this.occupants.set(Grid.cellKey(cx, cy), id);
  }

  clearOccupy(id: string): void {
    for (const [key, who] of this.occupants) {
      if (who === id) this.occupants.delete(key);
    }
  }

  setExtra(cx: number, cy: number, on: boolean): void {
    const k = Grid.cellKey(cx, cy);
    if (on) this.extraSolid.add(k);
    else this.extraSolid.delete(k);
  }

  worldToCell(x: number, y: number): { cx: number; cy: number } {
    return { cx: Math.floor(x / PATH_CELL), cy: Math.floor(y / PATH_CELL) };
  }

  static cellWorld(cx: number, cy: number): { x: number; y: number } {
    return { x: (cx + 0.5) * PATH_CELL, y: (cy + 0.5) * PATH_CELL };
  }

  static cellKey(cx: number, cy: number): string {
    return `${cx},${cy}`;
  }

  blockedWorld(x: number, y: number, ignoreId?: string): boolean {
    const { cx, cy } = this.worldToCell(x, y);
    return this.solid(cx, cy, ignoreId);
  }

  blockedProjectile(x: number, y: number): boolean {
    const { cx, cy } = this.worldToCell(x, y);
    if (!this.inBounds(cx, cy)) return true;
    return this.blocksLos(cx, cy);
  }

  blockedRadius(x: number, y: number, r: number, ignoreId?: string): boolean {
    return (
      this.blockedWorld(x - r, y - r, ignoreId) ||
      this.blockedWorld(x + r, y - r, ignoreId) ||
      this.blockedWorld(x - r, y + r, ignoreId) ||
      this.blockedWorld(x + r, y + r, ignoreId) ||
      this.blockedWorld(x, y, ignoreId)
    );
  }

  blocksLos(cx: number, cy: number): boolean {
    if (this.extraSolid.has(Grid.cellKey(cx, cy))) return true;
    return this.flags(cx, cy).blockLos;
  }

  fillRect(x: number, y: number, w: number, h: number, tile: Tile): void {
    for (let cy = y; cy < y + h; cy++) {
      for (let cx = x; cx < x + w; cx++) {
        this.set(cx, cy, tile);
      }
    }
  }
}
