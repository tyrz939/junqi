import towns from "@/data/towns.json";
import { biomeAt, type Biome } from "@/game/world/biomes";
import { Grid } from "@/game/world/Grid";
import { barrelPile, fenceRing, houseStamp, HOUSE_STAMPS } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { TownDef } from "@/game/world/generateTowns";
import type { EnemySpec, PropSpec } from "@/game/world/zoneTypes";

/**
 * Path samples every 8 cells. Walk is ~7.25 cells/s. Viewport ~40 cells.
 * Lamps every 8 samples ≈ 9s (OoT posts). Places every 16 samples ≈ 18s
 * (WoW-zone camp / pack / shrine). Not LttP-every-screen, not korok spam.
 */
const LAMP_EVERY = 8;
const PLACE_EVERY = 16;
const PLACE_GAP = 22;

export type WaysidePack = {
  props: PropSpec[];
  enemies: EnemySpec[];
  lights: { x: number; y: number; radius: number }[];
};

type Cell = { cx: number; cy: number };

const townCatalog = towns as Record<string, TownDef>;

const HUBS: { cx: number; cy: number; r: number }[] = [
  ...Object.values(townCatalog).map((t) => ({ cx: t.cx, cy: t.cy, r: t.radius + 8 })),
  { cx: 1780, cy: 1040, r: 42 },
  { cx: 300, cy: 70, r: 22 },
  { cx: 1120, cy: 680, r: 26 },
  { cx: 380, cy: 500, r: 20 },
];

const HERBS = [
  "pansy",
  "nasturtium",
  "honeylace_lily",
  "berry",
  "mushroom",
  "carrot",
  "onion",
  "potato",
  "wheat",
  "honey",
  "tea_leaf",
  "pear",
  "plum",
  "cabbage",
  "seed",
];

export function stampWaysides(grid: Grid, roads: Cell[][], rng: () => number): WaysidePack {
  const pack: WaysidePack = { props: [], enemies: [], lights: [] };
  const claimed: Cell[] = [];
  let lamps = 0;
  let places = 0;

  roads.forEach((path, roadI) => {
    const start = 4;
    const end = path.length - 4;
    for (let i = start; i < end; i++) {
      const p = path[i];
      if (nearHub(p.cx, p.cy)) continue;
      if (i % LAMP_EVERY === 0) placeLamp(pack, grid, p, `${roadI}_${lamps++}`);
      if (i % PLACE_EVERY !== 0) continue;
      if (tooClose(claimed, p, PLACE_GAP)) continue;
      const kind = places % 5;
      const ok =
        kind === 0
          ? placeCamp(pack, grid, rng, places, p)
          : kind === 1
            ? placePack(pack, grid, rng, places, p)
            : kind === 2
              ? placeShrine(pack, grid, rng, places, p)
              : kind === 3
                ? placeRuin(pack, grid, rng, places, p)
                : placeWild(pack, grid, rng, places, p);
      if (ok) {
        claimed.push(p);
        places += 1;
      }
    }
  });

  return pack;
}

function placeLamp(pack: WaysidePack, grid: Grid, p: Cell, id: string): void {
  const site = offsetGrass(grid, p, 2);
  if (!site) return;
  const pos = cellWorld(site.cx, site.cy);
  pack.props.push({
    id: `lamp_${id}`,
    kind: "torch",
    ...pos,
    texture: "torch",
  });
  pack.lights.push({ x: pos.x, y: pos.y, radius: 24 });
}

function placeCamp(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const at = beside(grid, p, rng, 12, 12);
  if (!at) return false;
  fenceRing(grid, at.cx, at.cy, 12, 12, 2);
  pack.props.push({
    id: `waycamp${n}_fire`,
    kind: "campfire",
    ...cellWorld(at.cx + 6, at.cy + 6),
    texture: "campfire",
  });
  pack.props.push({
    id: `waycamp${n}_chest`,
    kind: "chest",
    ...cellWorld(at.cx + 9, at.cy + 3),
    texture: "chest",
    loot: [
      { id: "apple", qty: 2 },
      { id: rng() < 0.5 ? "bread" : "wood", qty: 1 },
    ],
  });
  pack.props.push({
    id: `waycamp${n}_sign`,
    kind: "sign",
    ...cellWorld(at.cx + 2, at.cy + 2),
    texture: "sign",
    talk: "wayside_camp",
  });
  const fauna = biomeFauna(biomeAt(at.cx, at.cy), "camp");
  pack.enemies.push(
    { id: `waycamp${n}_a`, kind: fauna[0], ...cellWorld(at.cx + 3, at.cy + 4), respawn: 50 },
    { id: `waycamp${n}_b`, kind: fauna[1], ...cellWorld(at.cx + 8, at.cy + 8), respawn: 50 },
  );
  pack.lights.push({ ...cellWorld(at.cx + 6, at.cy + 6), radius: 28 });
  return true;
}

function placePack(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const at = beside(grid, p, rng, 6, 6);
  if (!at) return false;
  const fauna = biomeFauna(biomeAt(at.cx, at.cy), "pack");
  pack.enemies.push(
    { id: `waypoint${n}_a`, kind: fauna[0], ...cellWorld(at.cx + 1, at.cy + 1), respawn: 48 },
    { id: `waypoint${n}_b`, kind: fauna[1], ...cellWorld(at.cx + 4, at.cy + 3), respawn: 48 },
    { id: `waypoint${n}_c`, kind: fauna[2] ?? fauna[0], ...cellWorld(at.cx + 2, at.cy + 5), respawn: 40 },
  );
  if (rng() < 0.45) {
    pack.props.push({
      id: `waypoint${n}_crate`,
      kind: "push",
      ...cellWorld(at.cx, at.cy + 2),
      texture: "crate",
    });
  }
  return true;
}

function placeShrine(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const at = beside(grid, p, rng, 5, 5);
  if (!at) return false;
  grid.fillRect(at.cx + 1, at.cy + 1, 3, 3, "stone");
  const mile = n % 2 === 0;
  pack.props.push({
    id: `wayshrine${n}`,
    kind: mile ? "sign" : "shrine",
    ...cellWorld(at.cx + 2, at.cy + 2),
    texture: mile ? "sign" : "shrine",
    talk: mile ? "wayside_mile" : "wayside_shrine",
  });
  if (rng() < 0.5) {
    pack.props.push({
      id: `wayshrine${n}_drop`,
      kind: "drop",
      ...cellWorld(at.cx + 3, at.cy + 3),
      texture: "drop",
      loot: [{ id: HERBS[n % HERBS.length], qty: 1 }],
    });
  }
  pack.lights.push({ ...cellWorld(at.cx + 2, at.cy + 2), radius: 18 });
  return true;
}

function placeRuin(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const stamp = HOUSE_STAMPS[n % HOUSE_STAMPS.length];
  const w = stamp.w + 3;
  const h = stamp.h + 3;
  const at = beside(grid, p, rng, w, h);
  if (!at) {
    return placeWreck(pack, grid, rng, n, p);
  }
  fenceRing(grid, at.cx, at.cy, w, h, 2);
  houseStamp(grid, at.cx + 1, at.cy + 1, stamp.w, stamp.h);
  pack.props.push({
    id: `wayruin${n}_sign`,
    kind: "sign",
    ...cellWorld(at.cx + Math.floor(w / 2), at.cy + h),
    texture: "sign",
    talk: "wayside_ruin",
  });
  if (rng() < 0.6) {
    pack.props.push({
      id: `wayruin${n}_chest`,
      kind: "chest",
      ...cellWorld(at.cx + Math.floor(w / 2) + 1, at.cy + h),
      texture: "chest",
      loot: [
        { id: rng() < 0.5 ? "cheese" : "gold_dust", qty: 1 },
        { id: "wood", qty: 2 },
      ],
    });
  }
  pack.props.push(...barrelPile({ cx: at.cx + w - 3, cy: at.cy + h }, `wayruin${n}_crate`, 3));
  return true;
}

function placeWreck(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const at = beside(grid, p, rng, 5, 4);
  if (!at) return false;
  pack.props.push({
    id: `waywreck${n}`,
    kind: "broken",
    ...cellWorld(at.cx + 2, at.cy + 1),
    texture: "broken",
  });
  pack.props.push({
    id: `waywreck${n}_sign`,
    kind: "sign",
    ...cellWorld(at.cx, at.cy),
    texture: "sign",
    talk: "wayside_wreck",
  });
  pack.props.push(...barrelPile({ cx: at.cx + 1, cy: at.cy + 2 }, `waywreck${n}_crate`, 2));
  if (rng() < 0.4) {
    pack.enemies.push({
      id: `waywreck${n}_rat`,
      kind: "rat",
      ...cellWorld(at.cx + 3, at.cy + 2),
      respawn: 36,
    });
  }
  return true;
}

function placeWild(pack: WaysidePack, grid: Grid, rng: () => number, n: number, p: Cell): boolean {
  const at = beside(grid, p, rng, 6, 6);
  if (!at) return false;
  const fauna = biomeFauna(biomeAt(at.cx, at.cy), "wild");
  pack.enemies.push(
    { id: `waywild${n}_a`, kind: fauna[0], ...cellWorld(at.cx + 1, at.cy + 2), respawn: 42 },
    { id: `waywild${n}_b`, kind: fauna[1], ...cellWorld(at.cx + 4, at.cy + 4), respawn: 42 },
  );
  pack.props.push({
    id: `waywild${n}_herb`,
    kind: "drop",
    ...cellWorld(at.cx + 2, at.cy + 1),
    texture: "drop",
    loot: [{ id: HERBS[(n + 3) % HERBS.length], qty: 1 }],
  });
  if (rng() < 0.4) {
    pack.props.push({
      id: `waywild${n}_bench`,
      kind: "bench",
      ...cellWorld(at.cx, at.cy),
      texture: "bench",
    });
  }
  return true;
}

function biomeFauna(biome: Biome, role: "camp" | "pack" | "wild"): string[] {
  if (biome === "forest") return role === "wild" ? ["fox", "boar", "crow"] : ["wolf", "bandit", "crow"];
  if (biome === "farm") return ["crow", "fox", "boar"];
  if (biome === "grave") return role === "wild" ? ["moth", "grave_crow", "ghoul"] : ["ghoul", "skeleton", "grave_crow"];
  if (biome === "mountain") return ["hermit_wolf", "cave_bat", "frost_cactus"];
  if (biome === "works") return ["brigand", "thug", "miner"];
  if (biome === "yard") return ["skeleton", "bat", "rat"];
  if (biome === "town") return ["cutpurse", "thug", "rat"];
  if (role === "wild") return ["marsh_slime", "river_toad", "mill_rat"];
  return ["bandit", "marsh_slime", "crow"];
}

function beside(grid: Grid, p: Cell, rng: () => number, w: number, h: number): Cell | null {
  const dist = rng() < 0.5 ? 5 : 8;
  const dirs = [
    [dist, 0],
    [-dist, 0],
    [0, dist],
    [0, -dist],
    [dist, dist],
    [-dist, dist],
    [dist, -dist],
    [-dist, -dist],
  ];
  const rot = Math.floor(rng() * dirs.length);
  for (let i = 0; i < dirs.length; i++) {
    const [dx, dy] = dirs[(i + rot) % dirs.length];
    const cx = p.cx + dx;
    const cy = p.cy + dy;
    if (padOpen(grid, cx, cy, w, h)) return { cx, cy };
  }
  return null;
}

function offsetGrass(grid: Grid, p: Cell, dist: number): Cell | null {
  for (const [dx, dy] of [
    [0, -dist],
    [dist, 0],
    [-dist, 0],
    [0, dist],
    [dist, -dist],
    [-dist, -dist],
  ]) {
    const cx = p.cx + dx;
    const cy = p.cy + dy;
    if (!grid.inBounds(cx, cy)) continue;
    if (grid.get(cx, cy) === "grass") return { cx, cy };
  }
  return null;
}

function padOpen(grid: Grid, x: number, y: number, w: number, h: number): boolean {
  if (x < 4 || y < 4 || x + w >= grid.cols - 4 || y + h >= grid.rows - 4) return false;
  if (nearHub(x + Math.floor(w / 2), y + Math.floor(h / 2))) return false;
  let open = 0;
  for (let cy = y; cy < y + h; cy++) {
    for (let cx = x; cx < x + w; cx++) {
      const t = grid.get(cx, cy);
      if (t === "house" || t === "wall") return false;
      if (t === "grass" || t === "stone" || t === "bone") open += 1;
    }
  }
  return open > w * h * 0.55;
}

function nearHub(cx: number, cy: number): boolean {
  return HUBS.some((h) => Math.abs(cx - h.cx) < h.r && Math.abs(cy - h.cy) < h.r);
}

function tooClose(claimed: Cell[], p: Cell, gap: number): boolean {
  return claimed.some((c) => Math.abs(c.cx - p.cx) + Math.abs(c.cy - p.cy) < gap);
}
