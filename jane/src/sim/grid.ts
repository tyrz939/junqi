// One grid per zone. Cell = 8 px, everywhere (2020's PathTo used 32 in its
// fallback; that bug does not get a second life).
//
// Three layers live here because path, LOS, movement and projectiles all need
// the same answer to "what is in this cell":
//   tiles  u8   authored/generated terrain id (saved as seed + deltas)
//   flags  u8   derived: tile flags | prop flags (never saved)
//   occ    who stands where (never saved). Sparse: a few hundred entries in a map of
//          millions of cells, so it is a Map plus an F_OCC bit in `flags`. Hot loops (A*)
//          read the bit from the typed array and only touch the Map when it is set.

import { CELL } from "@/sim/constants";

export const F_SOLID = 1; // units cannot enter
export const F_BLOCK_LOS = 2; // sight and projectiles stop
export const F_WATER = 4; // drain/grow target; draws animated
export const F_PROP_SOLID = 8; // a solid prop covers the cell
export const F_PROP_LOS = 16; // that prop also blocks sight
export const F_INDOOR = 32; // no sky: ambient light ignores the clock
export const F_OCC = 64; // a unit stands here; ask `occupant()` who
export const F_NOPUSH = 128; // feet may cross, a pushed prop may not: the sill of a generated room

/** Bits that survive a tile change: they describe what is ON the cell, not the cell. */
const KEEP_ON_TILE_CHANGE = F_PROP_SOLID | F_PROP_LOS | F_OCC;

export const BLOCK_MOVE = F_SOLID | F_PROP_SOLID;
export const BLOCK_SIGHT = F_BLOCK_LOS | F_PROP_LOS;

export enum Tile {
  Void = 0,
  Grass = 1,
  GrassTall = 2,
  Dirt = 3,
  Road = 4,
  Water = 5,
  Sand = 6,
  Bush = 7,
  Tree = 8,
  Fence = 9,
  HouseWall = 10,
  HouseRoof = 11,
  Floor = 12,
  FloorWood = 13,
  Wall = 14,
  WallTop = 15,
  CaveFloor = 16,
  CaveWall = 17,
  TempleFloor = 18,
  TempleWall = 19,
  Moss = 20,
  DryBed = 21,
  Garden = 22,
  Rubble = 23,
  Track = 24,
  GrownPath = 25,
  Cobble = 26,
  Rail = 27,
  Cliff = 28,
  Sill = 29,
}

export const TILE_COUNT = 30;

/**
 * Flags per tile id. Water blocks feet, not sight: 2020 set `block_los` on obj_water
 * but CheckLOS and projectiles only ever tested obj_static_solid_parent, so bolts
 * crossed ponds. The shipped behaviour wins over the dead field.
 */
export const TILE_FLAGS: Uint8Array = (() => {
  const t = new Uint8Array(TILE_COUNT);
  t[Tile.Void] = F_SOLID | F_BLOCK_LOS;
  t[Tile.Water] = F_SOLID | F_WATER;
  t[Tile.Bush] = F_SOLID;
  t[Tile.Tree] = F_SOLID | F_BLOCK_LOS;
  t[Tile.Fence] = F_SOLID;
  t[Tile.HouseWall] = F_SOLID | F_BLOCK_LOS;
  t[Tile.HouseRoof] = F_SOLID | F_BLOCK_LOS;
  t[Tile.Floor] = F_INDOOR;
  t[Tile.FloorWood] = F_INDOOR;
  t[Tile.Wall] = F_SOLID | F_BLOCK_LOS | F_INDOOR;
  t[Tile.WallTop] = F_SOLID | F_BLOCK_LOS | F_INDOOR;
  t[Tile.CaveFloor] = F_INDOOR;
  t[Tile.CaveWall] = F_SOLID | F_BLOCK_LOS | F_INDOOR;
  t[Tile.TempleFloor] = F_INDOOR;
  t[Tile.TempleWall] = F_SOLID | F_BLOCK_LOS | F_INDOOR;
  t[Tile.Moss] = F_INDOOR;
  t[Tile.DryBed] = F_INDOOR;
  t[Tile.Garden] = F_INDOOR;
  t[Tile.Rubble] = F_SOLID | F_INDOOR;
  t[Tile.Track] = F_INDOOR;
  t[Tile.GrownPath] = 0;
  t[Tile.Rail] = F_SOLID;
  t[Tile.Cliff] = F_SOLID | F_BLOCK_LOS;
  // The floor just inside a generated room's mouth. Barrels never leave their room, so they
  // can never jam a corridor or be lost to the plate that needs them (DUNGEONS.md 2.1).
  t[Tile.Sill] = F_INDOOR | F_NOPUSH;
  return t;
})();

export class Grid {
  readonly w: number;
  readonly h: number;
  readonly tiles: Uint8Array;
  readonly flags: Uint8Array;
  private readonly occ = new Map<number, number>();

  constructor(w: number, h: number, tiles?: Uint8Array) {
    this.w = w;
    this.h = h;
    this.tiles = tiles ?? new Uint8Array(w * h);
    this.flags = new Uint8Array(w * h);
    if (tiles) this.refreshTileFlags();
  }

  index(cx: number, cy: number): number {
    return cy * this.w + cx;
  }

  inside(cx: number, cy: number): boolean {
    return cx >= 0 && cy >= 0 && cx < this.w && cy < this.h;
  }

  tileAt(cx: number, cy: number): number {
    return this.inside(cx, cy) ? this.tiles[cy * this.w + cx] : Tile.Void;
  }

  setTile(cx: number, cy: number, tile: number): void {
    if (!this.inside(cx, cy)) return;
    const i = cy * this.w + cx;
    this.tiles[i] = tile;
    this.flags[i] = (this.flags[i] & KEEP_ON_TILE_CHANGE) | TILE_FLAGS[tile];
  }

  fill(cx: number, cy: number, w: number, h: number, tile: number): void {
    const x0 = Math.max(0, cx);
    const y0 = Math.max(0, cy);
    const x1 = Math.min(this.w, cx + w);
    const y1 = Math.min(this.h, cy + h);
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) this.setTile(x, y, tile);
  }

  refreshTileFlags(): void {
    const { tiles, flags } = this;
    for (let i = 0; i < tiles.length; i++) {
      flags[i] = (flags[i] & KEEP_ON_TILE_CHANGE) | TILE_FLAGS[tiles[i]];
    }
  }

  clearPropFlags(): void {
    const { flags } = this;
    const keep = ~(F_PROP_SOLID | F_PROP_LOS) & 0xff;
    for (let i = 0; i < flags.length; i++) flags[i] &= keep;
  }

  /** The same, for an inclusive rect of cells. One moved crate must not cost a pass over a county of millions. */
  clearPropFlagsIn(cx0: number, cy0: number, cx1: number, cy1: number): void {
    const { flags } = this;
    const keep = ~(F_PROP_SOLID | F_PROP_LOS) & 0xff;
    const x0 = Math.max(0, cx0);
    const x1 = Math.min(this.w - 1, cx1);
    const y1 = Math.min(this.h - 1, cy1);
    for (let y = Math.max(0, cy0); y <= y1; y++) {
      for (let i = y * this.w + x0, end = y * this.w + x1; i <= end; i++) flags[i] &= keep;
    }
  }

  stampProp(cx: number, cy: number, cw: number, ch: number, blockLos: boolean): void {
    const bits = F_PROP_SOLID | (blockLos ? F_PROP_LOS : 0);
    for (let y = cy; y < cy + ch; y++) {
      for (let x = cx; x < cx + cw; x++) {
        if (this.inside(x, y)) this.flags[y * this.w + x] |= bits;
      }
    }
  }

  flagsAt(cx: number, cy: number): number {
    return this.inside(cx, cy) ? this.flags[cy * this.w + cx] : F_SOLID | F_BLOCK_LOS;
  }

  /** Terrain or prop in the way. Ignores units. */
  solid(cx: number, cy: number): boolean {
    return (this.flagsAt(cx, cy) & BLOCK_MOVE) !== 0;
  }

  /** A pushed prop may not be shoved onto this cell. Feet ignore it. */
  noPush(cx: number, cy: number): boolean {
    return (this.flagsAt(cx, cy) & F_NOPUSH) !== 0;
  }

  blocksSight(cx: number, cy: number): boolean {
    return (this.flagsAt(cx, cy) & BLOCK_SIGHT) !== 0;
  }

  /** Free for unit `self` to stand on: not solid, and not held by someone else. */
  free(cx: number, cy: number, self = 0): boolean {
    if (!this.inside(cx, cy)) return false;
    const i = cy * this.w + cx;
    if ((this.flags[i] & BLOCK_MOVE) !== 0) return false;
    if ((this.flags[i] & F_OCC) === 0) return true;
    return this.occ.get(i) === self;
  }

  /** Unit id standing on cell index `i`, or 0. */
  occupant(i: number): number {
    return (this.flags[i] & F_OCC) === 0 ? 0 : (this.occ.get(i) ?? 0);
  }

  /** Claim a cell for a unit. A cell already held by someone else stays theirs. */
  occupy(i: number, id: number): void {
    if ((this.flags[i] & F_OCC) !== 0) return;
    this.occ.set(i, id);
    this.flags[i] |= F_OCC;
  }

  /** Release a cell, only if `id` is the one holding it. */
  vacate(i: number, id: number): void {
    if (this.occ.get(i) !== id) return;
    this.occ.delete(i);
    this.flags[i] &= ~F_OCC & 0xff;
  }

  get occupiedCount(): number {
    return this.occ.size;
  }

  /** Nearest free cell in growing square rings. 2020's PathTo spiral, in the right units. */
  nearestFree(cx: number, cy: number, maxRadius: number, self = 0): { cx: number; cy: number } | null {
    if (this.free(cx, cy, self)) return { cx, cy };
    for (let r = 1; r <= maxRadius; r++) {
      for (let d = -r; d <= r; d++) {
        // top and bottom edges, then the two sides; deterministic order
        if (this.free(cx + d, cy - r, self)) return { cx: cx + d, cy: cy - r };
        if (this.free(cx + d, cy + r, self)) return { cx: cx + d, cy: cy + r };
        if (this.free(cx - r, cy + d, self)) return { cx: cx - r, cy: cy + d };
        if (this.free(cx + r, cy + d, self)) return { cx: cx + r, cy: cy + d };
      }
    }
    return null;
  }
}

export function cellOf(px: number): number {
  return Math.floor(px / CELL);
}

export function centre(c: number): number {
  return c * CELL + CELL / 2;
}
