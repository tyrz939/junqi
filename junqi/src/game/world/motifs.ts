import type { Grid, Tile } from "@/game/world/Grid";
import { cellWorld } from "@/game/world/stamp";
import type { PropSpec } from "@/game/world/zoneTypes";

export const HOUSE_STAMPS = [
  { w: 7, h: 9 },
  { w: 11, h: 6 },
  { w: 10, h: 12 },
] as const;

export function paintBorder(grid: Grid, tile: Tile = "wall"): void {
  grid.fillRect(0, 0, grid.cols, 1, tile);
  grid.fillRect(0, grid.rows - 1, grid.cols, 1, tile);
  grid.fillRect(0, 0, 1, grid.rows, tile);
  grid.fillRect(grid.cols - 1, 0, 1, grid.rows, tile);
}

export function fenceRing(grid: Grid, x: number, y: number, w: number, h: number, gap = 2): void {
  grid.fillRect(x, y, w, 1, "wall");
  grid.fillRect(x, y + h - 1, w, 1, "wall");
  grid.fillRect(x, y, 1, h, "wall");
  grid.fillRect(x + w - 1, y, 1, h, "wall");
  const gx = x + Math.floor((w - gap) / 2);
  grid.fillRect(gx, y + h - 1, gap, 1, "grass");
}

export function houseStamp(grid: Grid, x: number, y: number, w: number, h: number): void {
  grid.fillRect(x, y, w, h, "house");
  const door = x + Math.floor(w / 2) - 1;
  grid.fillRect(door, y + h - 1, 2, 1, "door");
}

export type RoomRect = { x: number; y: number; w: number; h: number };

export function roomCenter(room: RoomRect): { cx: number; cy: number } {
  return { cx: room.x + Math.floor(room.w / 2), cy: room.y + Math.floor(room.h / 2) };
}

export function stoneTunnel(grid: Grid, a: { cx: number; cy: number }, b: { cx: number; cy: number }): void {
  let x = a.cx;
  let y = a.cy;
  while (x !== b.cx) {
    grid.set(x, y, "stone");
    x += Math.sign(b.cx - x);
  }
  while (y !== b.cy) {
    grid.set(x, y, "stone");
    y += Math.sign(b.cy - y);
  }
}

export function lilyPool(grid: Grid, x: number, y: number, w: number, h: number): void {
  grid.fillRect(x, y, w, h, "water");
  grid.fillRect(x + 1, y + Math.floor(h / 2), w - 2, 1, "grass");
}

export function coffinRow(grid: Grid, x: number, y: number, count: number): void {
  for (let i = 0; i < count; i++) grid.fillRect(x + i * 3, y, 2, 3, "bone");
}

export function wiggleRoad(
  grid: Grid,
  a: { cx: number; cy: number },
  b: { cx: number; cy: number },
  rng: () => number,
  width = 3,
): { cx: number; cy: number }[] {
  return wiggleStroke(grid, a, b, rng, width, "stone");
}

export function wiggleStroke(
  grid: Grid,
  a: { cx: number; cy: number },
  b: { cx: number; cy: number },
  rng: () => number,
  width: number,
  tile: Tile,
): { cx: number; cy: number }[] {
  const path: { cx: number; cy: number }[] = [];
  let x = a.cx;
  let y = a.cy;
  let wobble = 0;
  const budget = Math.abs(b.cx - a.cx) + Math.abs(b.cy - a.cy) + 8000;
  let steps = 0;
  while ((x !== b.cx || y !== b.cy) && steps < budget) {
    steps += 1;
    paintDisk(grid, x, y, width, tile);
    if (steps % 8 === 0) path.push({ cx: x, cy: y });
    if (rng() < 0.12) wobble = Math.floor(rng() * 5) - 2;
    const dx = Math.sign(b.cx - x);
    const dy = Math.sign(b.cy - y);
    if (Math.abs(b.cx - x) > Math.abs(b.cy - y)) {
      x += dx;
      y = clamp(y + (rng() < 0.35 ? wobble : 0), 2, grid.rows - 3);
    } else if (dy !== 0) {
      y += dy;
      x = clamp(x + (rng() < 0.35 ? wobble : 0), 2, grid.cols - 3);
    } else {
      x += dx || (rng() < 0.5 ? 1 : -1);
    }
  }
  paintDisk(grid, b.cx, b.cy, width, tile);
  path.push({ cx: b.cx, cy: b.cy });
  return path;
}

export function barrelPile(
  origin: { cx: number; cy: number },
  idPrefix: string,
  count: number,
): PropSpec[] {
  const props: PropSpec[] = [];
  for (let i = 0; i < count; i++) {
    const pos = cellWorld(origin.cx + (i % 2), origin.cy + Math.floor(i / 2));
    props.push({ id: `${idPrefix}_${i}`, kind: "push", x: pos.x, y: pos.y, texture: "crate" });
  }
  return props;
}

export function torchGrid(
  room: { x: number; y: number; w: number; h: number },
  idPrefix: string,
  step = 6,
): { props: PropSpec[]; lights: { x: number; y: number; radius: number }[] } {
  const props: PropSpec[] = [];
  const lights: { x: number; y: number; radius: number }[] = [];
  let n = 0;
  for (let x = room.x + 1; x < room.x + room.w - 1; x += step) {
    for (const y of [room.y + 2, room.y + room.h - 3]) {
      const pos = cellWorld(x, y);
      props.push({ id: `${idPrefix}_${n}`, kind: "torch", x: pos.x, y: pos.y, texture: "torch" });
      lights.push({ x: pos.x, y: pos.y, radius: 22 });
      n += 1;
    }
  }
  return { props, lights };
}

export function furnitureWall(
  origin: { cx: number; cy: number },
  count: number,
  idPrefix: string,
): PropSpec[] {
  const props: PropSpec[] = [];
  for (let i = 0; i < count; i++) {
    const pos = cellWorld(origin.cx + i * 2, origin.cy);
    props.push({ id: `${idPrefix}_${i}`, kind: "bench", x: pos.x, y: pos.y, texture: "bench" });
  }
  return props;
}

export function lampAlong(
  path: { cx: number; cy: number }[],
  every: number,
): { x: number; y: number; radius: number }[] {
  const lights: { x: number; y: number; radius: number }[] = [];
  for (let i = 0; i < path.length; i += every) {
    const p = cellWorld(path[i].cx, path[i].cy - 2);
    lights.push({ x: p.x, y: p.y, radius: 26 });
  }
  return lights;
}

function paintDisk(grid: Grid, cx: number, cy: number, width: number, tile: Tile): void {
  const r = Math.floor(width / 2);
  for (let dy = -r; dy <= r; dy++) {
    for (let dx = -r; dx <= r; dx++) {
      if (!grid.inBounds(cx + dx, cy + dy)) continue;
      const cur = grid.get(cx + dx, cy + dy);
      if (cur === "wall" || cur === "house" || cur === "door") continue;
      if (tile === "water" && cur === "stone") continue;
      grid.set(cx + dx, cy + dy, tile);
    }
  }
}

export function lotFits(grid: Grid, x: number, y: number, w: number, h: number): boolean {
  if (x < 4 || y < 4 || x + w >= grid.cols - 4 || y + h >= grid.rows - 4) return false;
  let road = 0;
  for (let cy = y; cy < y + h; cy++) {
    for (let cx = x; cx < x + w; cx++) {
      const t = grid.get(cx, cy);
      if (t === "house" || t === "wall" || t === "water") return false;
      if (t === "stone") road += 1;
    }
  }
  return road < w * h * 0.3;
}

export function scatterTile(
  grid: Grid,
  x: number,
  y: number,
  w: number,
  h: number,
  tile: Tile,
  count: number,
  rng: () => number,
): void {
  for (let i = 0; i < count; i++) {
    const cx = x + Math.floor(rng() * w);
    const cy = y + Math.floor(rng() * h);
    if (!grid.inBounds(cx, cy)) continue;
    if (grid.get(cx, cy) !== "grass") continue;
    grid.set(cx, cy, tile);
  }
}

function clamp(n: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, n));
}
