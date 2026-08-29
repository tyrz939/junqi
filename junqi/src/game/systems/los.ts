import type { Grid } from "@/game/world/Grid";

export function hasLos(grid: Grid, x0: number, y0: number, x1: number, y1: number): boolean {
  const a = grid.worldToCell(x0, y0);
  const b = grid.worldToCell(x1, y1);
  let { cx, cy } = a;
  const dx = Math.abs(b.cx - a.cx);
  const dy = Math.abs(b.cy - a.cy);
  const sx = a.cx < b.cx ? 1 : -1;
  const sy = a.cy < b.cy ? 1 : -1;
  let err = dx - dy;
  while (cx !== b.cx || cy !== b.cy) {
    if ((cx !== a.cx || cy !== a.cy) && grid.blocksLos(cx, cy)) return false;
    const e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      cx += sx;
    }
    if (e2 < dx) {
      err += dx;
      cy += sy;
    }
  }
  return true;
}
