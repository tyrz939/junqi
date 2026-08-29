import { LOAD_BLOCK, LOAD_RING_PX, PATH_CELL, TILE_CHUNK_CELLS } from "@/game/constants";

export type Block = { x: number; y: number };

export function worldBlock(x: number, y: number): Block {
  return { x: Math.floor(x / LOAD_BLOCK), y: Math.floor(y / LOAD_BLOCK) };
}

export function sameBlock(a: Block | null, b: Block): boolean {
  return Boolean(a && a.x === b.x && a.y === b.y);
}

export function inLoadRing(x: number, y: number, px: number, py: number): boolean {
  return Math.abs(x - px) <= LOAD_RING_PX && Math.abs(y - py) <= LOAD_RING_PX;
}

export function chunkInRing(chunkCx: number, chunkCy: number, px: number, py: number): boolean {
  const x0 = chunkCx * TILE_CHUNK_CELLS * PATH_CELL;
  const y0 = chunkCy * TILE_CHUNK_CELLS * PATH_CELL;
  const x1 = x0 + TILE_CHUNK_CELLS * PATH_CELL;
  const y1 = y0 + TILE_CHUNK_CELLS * PATH_CELL;
  return x0 < px + LOAD_RING_PX && x1 > px - LOAD_RING_PX && y0 < py + LOAD_RING_PX && y1 > py - LOAD_RING_PX;
}
