import { PATH_CELL } from "@/game/constants";
import type { Grid } from "@/game/world/Grid";

type Node = { cx: number; cy: number; g: number; f: number; px: number; py: number };

export function astar(
  grid: Grid,
  fromX: number,
  fromY: number,
  toX: number,
  toY: number,
  maxMetres = 80,
  ignoreId?: string,
): { x: number; y: number }[] | null {
  const start = grid.worldToCell(fromX, fromY);
  const goal = grid.worldToCell(toX, toY);
  if (grid.solid(goal.cx, goal.cy, ignoreId)) return null;

  const key = (c: { cx: number; cy: number }) => `${c.cx},${c.cy}`;
  const open: Node[] = [
    { ...start, g: 0, f: heur(start, goal), px: start.cx, py: start.cy },
  ];
  const came = new Map<string, string>();
  const gScore = new Map<string, number>([[key(start), 0]]);

  while (open.length) {
    open.sort((a, b) => a.f - b.f);
    const cur = open.shift()!;
    if (cur.cx === goal.cx && cur.cy === goal.cy) {
      const cells = unwind(came, key(cur));
      if (cells.length * PATH_CELL > maxMetres * PATH_CELL) return null;
      return cells.map(([cx, cy]) => ({
        x: cx * PATH_CELL + PATH_CELL / 2,
        y: cy * PATH_CELL + PATH_CELL / 2,
      }));
    }
    const dirs = [
      [1, 0],
      [-1, 0],
      [0, 1],
      [0, -1],
      [1, 1],
      [1, -1],
      [-1, 1],
      [-1, -1],
    ];
    for (const [dx, dy] of dirs) {
      const nx = cur.cx + dx;
      const ny = cur.cy + dy;
      if (grid.solid(nx, ny, ignoreId)) continue;
      if (dx !== 0 && dy !== 0 && (grid.solid(cur.cx + dx, cur.cy, ignoreId) || grid.solid(cur.cx, cur.cy + dy, ignoreId))) {
        continue;
      }
      const next = { cx: nx, cy: ny };
      const step = dx !== 0 && dy !== 0 ? 1.4 : 1;
      const tentative = (gScore.get(key(cur)) ?? 9999) + step;
      if (tentative >= (gScore.get(key(next)) ?? 9999)) continue;
      came.set(key(next), key(cur));
      gScore.set(key(next), tentative);
      open.push({
        ...next,
        g: tentative,
        f: tentative + heur(next, goal),
        px: cur.cx,
        py: cur.cy,
      });
    }
  }
  return null;
}

function heur(a: { cx: number; cy: number }, b: { cx: number; cy: number }): number {
  return Math.hypot(a.cx - b.cx, a.cy - b.cy);
}

function unwind(came: Map<string, string>, end: string): [number, number][] {
  const out: [number, number][] = [];
  let cur: string | undefined = end;
  while (cur) {
    const [cx, cy] = cur.split(",").map(Number) as [number, number];
    out.push([cx, cy]);
    cur = came.get(cur);
  }
  return out.reverse();
}
