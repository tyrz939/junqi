// Line of sight on the cell grid. Integer DDA ("supercover"): visits every cell
// the segment touches, so a bolt cannot slip between two diagonal walls.
// Sight and projectiles share this; 2020 shared `block_los` the same way.

import { CELL } from "@/sim/constants";
import { BLOCK_SIGHT, F_BLOCK_LOS, type Grid } from "@/sim/grid";

/** True when nothing with a sight-blocking flag lies between the two pixel points. */
export function lineOfSight(grid: Grid, x0: number, y0: number, x1: number, y1: number): boolean {
  return firstBlocked(grid, x0, y0, x1, y1) === -1;
}

/** Walls only: props (pillars, gates) are ignored. The snake's anti-cheese reset asks this. */
export function lineOfSightWalls(grid: Grid, x0: number, y0: number, x1: number, y1: number): boolean {
  return firstBlocked(grid, x0, y0, x1, y1, F_BLOCK_LOS) === -1;
}

/**
 * Index of the first sight-blocking cell on the segment, or -1.
 * The start cell is never tested: a unit hugging a wall can still see out.
 */
export function firstBlocked(grid: Grid, x0: number, y0: number, x1: number, y1: number, mask = BLOCK_SIGHT): number {
  let cx = Math.floor(x0 / CELL);
  let cy = Math.floor(y0 / CELL);
  const tx = Math.floor(x1 / CELL);
  const ty = Math.floor(y1 / CELL);
  const dx = x1 - x0;
  const dy = y1 - y0;
  const stepX = dx > 0 ? 1 : -1;
  const stepY = dy > 0 ? 1 : -1;
  // Parametric distance to the next vertical / horizontal cell border.
  const invX = dx !== 0 ? 1 / Math.abs(dx) : Infinity;
  const invY = dy !== 0 ? 1 / Math.abs(dy) : Infinity;
  let tMaxX = dx !== 0 ? ((dx > 0 ? (cx + 1) * CELL - x0 : x0 - cx * CELL) * invX) : Infinity;
  let tMaxY = dy !== 0 ? ((dy > 0 ? (cy + 1) * CELL - y0 : y0 - cy * CELL) * invY) : Infinity;
  const tDeltaX = CELL * invX;
  const tDeltaY = CELL * invY;
  // Manhattan cell distance bounds the walk; no float comparison decides termination.
  let steps = Math.abs(tx - cx) + Math.abs(ty - cy);
  while (steps-- > 0) {
    if (tMaxX < tMaxY) {
      tMaxX += tDeltaX;
      cx += stepX;
    } else {
      tMaxY += tDeltaY;
      cy += stepY;
    }
    if ((grid.flagsAt(cx, cy) & mask) !== 0) return cy * grid.w + cx;
  }
  return -1;
}
