import pockets from "@/data/pockets.json";
import { beatSpine } from "@/game/systems/catalog";
import type { ZoneId } from "@/game/types";
import { Grid, type Tile } from "@/game/world/Grid";
import type { EnemySpec, PropSpec, ZoneBlue } from "@/game/world/zoneTypes";

export type PocketTile = { dx: number; dy: number; w?: number; h?: number; tile: Tile };

export type PocketProp = Omit<PropSpec, "x" | "y"> & { dx: number; dy: number };

export type PocketEnemy = Omit<EnemySpec, "x" | "y" | "patrol"> & {
  dx: number;
  dy: number;
  patrol?: { dx: number; dy: number }[];
};

export type PocketDef = {
  id: string;
  zone: string;
  w: number;
  h: number;
  tiles?: PocketTile[];
  props?: PocketProp[];
  enemies?: PocketEnemy[];
};

export type StampResult = {
  origin: { cx: number; cy: number };
  props: PropSpec[];
  enemies: EnemySpec[];
};

const catalog = pockets as Record<string, PocketDef>;

const MUST: Record<ZoneId, string[]> = {
  county: ["house_door", "dog", "skeleton", "factory_door", "school_door", "butterfly_mouth"],
  house: ["bench", "hatch_a"],
  dungeon: ["cellar_gate"],
  mine: ["mine_up"],
  burial: ["story_relic", "lock_in", "shrine", "boss_gate", "snake"],
  abandoned: ["abandoned_up"],
  museum: ["museum_up"],
  graveyard: ["graveyard_up"],
  factory: ["factory_up"],
  school: ["school_up"],
  butterfly: ["butterfly_up"],
  pipes: ["pipes_up"],
};

export function getPocket(id: string): PocketDef {
  const row = catalog[id];
  if (!row) throw new Error(`Unknown pocket: ${id}`);
  return row;
}

export function cellWorld(cx: number, cy: number): { x: number; y: number } {
  return Grid.cellWorld(cx, cy);
}

export function stampPocket(
  grid: Grid,
  pocket: PocketDef,
  cx: number,
  cy: number,
  rng: () => number,
  jitter = 2,
): StampResult {
  let origin = { cx, cy };
  let last = "";
  for (let attempt = 0; attempt < 12; attempt++) {
    const jx = attempt === 0 ? 0 : Math.floor(rng() * (jitter * 2 + 1)) - jitter;
    const jy = attempt === 0 ? 0 : Math.floor(rng() * (jitter * 2 + 1)) - jitter;
    origin = { cx: cx + jx, cy: cy + jy };
    if (fits(grid, origin.cx, origin.cy, pocket.w, pocket.h)) {
      paint(grid, pocket, origin);
      return {
        origin,
        props: (pocket.props ?? []).map((p) => placeProp(p, origin)),
        enemies: (pocket.enemies ?? []).map((e) => placeEnemy(e, origin)),
      };
    }
    last = `${pocket.id} at ${origin.cx},${origin.cy}`;
  }
  throw new Error(`Pocket would not fit: ${last || pocket.id}`);
}

export function stampPocketSoft(
  grid: Grid,
  pocket: PocketDef,
  cx: number,
  cy: number,
  rng: () => number,
  jitter = 2,
): StampResult | null {
  try {
    return stampPocket(grid, pocket, cx, cy, rng, jitter);
  } catch {
    return null;
  }
}

export function assertRequired(zone: ZoneId, blue: ZoneBlue): void {
  const have = new Set<string>([...blue.props.map((p) => p.id), ...blue.enemies.map((e) => e.id)]);
  for (const id of MUST[zone]) {
    if (!have.has(id)) throw new Error(`Zone ${zone} missing beat object ${id}`);
  }
  for (const beat of beatSpine) {
    for (const id of beat.mustPlace) {
      if (!MUST[zone].includes(id)) continue;
      if (!have.has(id)) throw new Error(`Beat ${beat.id} missing ${id} in ${zone}`);
    }
  }
}

function fits(grid: Grid, cx: number, cy: number, w: number, h: number): boolean {
  return cx >= 1 && cy >= 1 && cx + w < grid.cols - 1 && cy + h < grid.rows - 1;
}

function paint(grid: Grid, pocket: PocketDef, origin: { cx: number; cy: number }): void {
  for (const tile of pocket.tiles ?? []) {
    grid.fillRect(origin.cx + tile.dx, origin.cy + tile.dy, tile.w ?? 1, tile.h ?? 1, tile.tile);
  }
}

function placeProp(spec: PocketProp, origin: { cx: number; cy: number }): PropSpec {
  const { dx, dy, ...rest } = spec;
  const pos = cellWorld(origin.cx + dx, origin.cy + dy);
  return { ...rest, x: pos.x, y: pos.y };
}

function placeEnemy(spec: PocketEnemy, origin: { cx: number; cy: number }): EnemySpec {
  const { dx, dy, patrol, ...rest } = spec;
  const pos = cellWorld(origin.cx + dx, origin.cy + dy);
  return {
    ...rest,
    x: pos.x,
    y: pos.y,
    patrol: patrol?.map((p) => cellWorld(origin.cx + p.dx, origin.cy + p.dy)),
  };
}
