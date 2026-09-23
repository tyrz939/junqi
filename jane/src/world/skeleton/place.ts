// Layers 1b, 3 and 4: where things go. Sites are solved from their rows
// (sites.json), then sub-areas (areas.json), then a budgeted scatter of minor
// points of interest (pois.json). Everything is "list the cells that satisfy the
// row, pick one with the seed": no hill-climbing, nothing that can wander.

import { hashString, rngFloat, stepDice, type RngState } from "@/sim/rng";
import type { Terrain } from "@/world/skeleton/terrain";
import {
  at,
  Biome,
  inside,
  MACRO,
  metres,
  REGION_IDS,
  ROAD,
  SKEL_H,
  SKEL_W,
  type AreaRow,
  type PlacedArea,
  type PlacedPoi,
  type PlacedSite,
  type PoiRow,
  type Region,
  type SiteRow,
} from "@/world/skeleton/types";

const pick = <T>(rng: RngState, list: readonly T[]): T => list[Math.min(list.length - 1, Math.floor(rngFloat(rng) * list.length))];

/** The base of a named ranking (`ranked`, `rankOf`): one per (seed, step, attempt). */
export const rankBase = (seed: number, step: string, attempt: number): number => hashString(`${seed >>> 0}:${step}:${attempt}`);

/** A cell's place in a named ranking: a hash of the ranking's base and the cell, 32 bits. */
export function rankOf(base: number, cell: number): number {
  let h = (base ^ Math.imul(cell + 1, 0x9e3779b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return (h ^ (h >>> 16)) >>> 0;
}

/**
 * The seed's pick of one cell, by rank rather than by position in the list: the cell that ranks first.
 * Rule out any other cell (the rail took it, a new row filled it) and the pick stays where it was, where
 * a dice roll into the list would land somewhere else entirely. Ties go to the lower cell.
 */
export function ranked(base: number, cells: readonly number[]): number {
  let best = cells[0];
  let top = rankOf(base, best);
  for (let n = 1; n < cells.length; n++) {
    const r = rankOf(base, cells[n]);
    if (r < top || (r === top && cells[n] < best)) {
      top = r;
      best = cells[n];
    }
  }
  return best;
}

/** How far a patch's edge may lie from the nearest road and still be seen from it (m): about a half-screen and a half. */
const PATCH_SEEN = 40;
/** Extra metres a patch of threat 4 or more keeps from a haven's fence. */
const HAVEN_MARGIN = 260;
/** Sites keep at least this far apart unless a row says otherwise. */
const SITE_SPACING = 160;
/** A road rule is aimed at with the crow's distance: roads wander about this much. */
const WANDER_MIN = 1.08;
const WANDER_MAX = 1.55;

const slopeAt = (t: Terrain, x: number, y: number): number => t.slope[at(x, y)];

export function terrainFits(t: Terrain, kind: string | undefined, x: number, y: number): boolean {
  const i = at(x, y);
  if (t.water[i]) return false;
  switch (kind) {
    case undefined:
      return true;
    case "flat":
      return slopeAt(t, x, y) <= 7 && t.wet[i] >= 2;
    case "bank":
      return t.wet[i] >= 2 && t.wet[i] <= 5;
    case "wood":
      return t.biome[i] === Biome.Wood || t.biome[i] === Biome.WetWood;
    case "foothill":
      return t.biome[i] === Biome.Foothill && slopeAt(t, x, y) <= 14;
    case "crown":
      return metres(x, y, t.crown.mx, t.crown.my) <= 5 * MACRO && slopeAt(t, x, y) <= 16;
    default:
      return false;
  }
}

/** Every macro cell this row could stand on, given what is already placed. Array order, so the seed's pick is stable. */
export function siteCandidates(t: Terrain, row: SiteRow, placed: readonly PlacedSite[]): number[] {
  const byId = new Map(placed.map((p) => [p.id, p]));
  {
    const region = REGION_IDS.indexOf(row.region) as Region;
    const where = row.where ?? {};
    const candidates: number[] = [];
    const edge = where.edge;
    // What each placed site allows, as squared macro distances, worked out once per row and not once per cell.
    // A road rule is aimed at with the crow's distance (roads wander); the real road is measured afterwards.
    const limits = placed.map((p) => {
      const rule = row.dist?.find((r) => r.to === p.id);
      let min = SITE_SPACING;
      let max = Infinity;
      if (rule) {
        min = (rule.min ?? 0) / (rule.by === "road" ? WANDER_MAX : 1);
        max = (rule.max ?? Infinity) / (rule.by === "road" ? WANDER_MIN : 1);
      }
      return { x: p.mx, y: p.my, min2: (min / MACRO) ** 2, max2: (max / MACRO) ** 2 };
    });
    for (let y = 3; y < SKEL_H - 3; y++) {
      for (let x = edge === "west" ? 0 : 3; x < (edge === "east" ? SKEL_W : SKEL_W - 3); x++) {
        if (edge === "west" && x !== 0) continue;
        if (edge === "east" && x !== SKEL_W - 1) continue;
        const i = at(x, y);
        if (t.region[i] !== region || !terrainFits(t, where.terrain, x, y)) continue;
        if (where.maxHeight !== undefined && t.height[i] > where.maxHeight) continue;
        if (where.riverWithin !== undefined &&Math.abs(x - t.riverX[y]) * MACRO > where.riverWithin) continue;
        if (where.lakeWithin !== undefined && metres(x, y, t.lake.mx, t.lake.my) > t.lake.r * MACRO + where.lakeWithin) continue;
        if (where.acrossRiverFrom) {
          const other = byId.get(where.acrossRiverFrom);
          if (other && x > t.riverX[y] === other.mx > t.riverX[other.my]) continue;
        }
        let ok = true;
        for (const l of limits) {
          const d2 = (x - l.x) * (x - l.x) + (y - l.y) * (y - l.y);
          if (d2 < l.min2 || d2 > l.max2) {
            ok = false;
            break;
          }
        }
        if (ok) candidates.push(i);
      }
    }
    return candidates;
  }
}

export const pickCell = (rng: RngState, cells: readonly number[]): number => pick(rng, cells);

/** `safeDist`: macro cells to the first walk (station, Julie's, town). Nothing above threat 1 may reach it. */
/** `nearRail`: the railway's macro cells and those beside them (skeleton/rail.ts). Nothing small stands on the line. */
export type PlaceCtx = { t: Terrain; road: Uint8Array; roadDist: Float32Array; sites: PlacedSite[]; safeDist: Float32Array; nearRail?: Uint8Array };

/**
 * Sub-areas: named patches with their own danger, in every region (PLAN.md 2.6).
 * A row that cannot be placed on this seed is skipped, not fatal: areas are texture,
 * sites are story.
 */
export function placeAreas(ctx: PlaceCtx, rows: readonly AreaRow[], seed: number, attempt: number): PlacedArea[] {
  const out: PlacedArea[] = [];
  for (const row of rows) {
    const region = REGION_IDS.indexOf(row.region) as Region;
    const where = row.where ?? {};
    const near = where.near ? ctx.sites.find((s) => s.id === where.near) : undefined;
    if (where.near && !near) continue;
    const reach = Math.ceil(row.radius / MACRO);
    const candidates: number[] = [];
    // A patch never swallows a haven, a dangerous one never sits on a story door that is not its
    // anchor, and two patches do not overlap. Squared macro distances, once per row.
    const keepOut = [
      // The worst patches (4 and up) also keep a long field's width from any haven: you should see them coming.
      ...ctx.sites
        .filter((s) => s !== near)
        .map((s) => ({ x: s.mx, y: s.my, r: s.row.hub ? s.row.hub + row.radius + 30 + (row.threat >= 4 ? HAVEN_MARGIN : 0) : row.radius * 0.6 })),
      ...out.map((a) => ({ x: a.mx, y: a.my, r: (a.row.radius + row.radius) * 0.9 })),
    ].map((k) => ({ x: k.x, y: k.y, r2: (k.r / MACRO) ** 2 }));
    for (let y = reach; y < SKEL_H - reach; y++) {
      for (let x = reach; x < SKEL_W - reach; x++) {
        const i = at(x, y);
        if (ctx.t.region[i] !== region || !terrainFits(ctx.t, where.terrain, x, y)) continue;
        const rd = ctx.roadDist[i] * MACRO;
        if (where.onRoad && rd > 0) continue;
        if (where.offRoad !== undefined && rd < where.offRoad) continue;
        if (near) {
          const d = metres(x, y, near.mx, near.my);
          if (d < (where.nearMin ?? 0) || d > (where.nearMax ?? 1e9)) continue;
        }
        if (row.threat > 1 && ctx.safeDist[i] <= reach + 2) continue;
        let ok = true;
        for (const k of keepOut) {
          if ((x - k.x) * (x - k.x) + (y - k.y) * (y - k.y) < k.r2) {
            ok = false;
            break;
          }
        }
        if (ok) candidates.push(i);
      }
    }
    if (candidates.length === 0) continue;
    // A patch is somewhere she is sent by name, so its edge is seen from a road (a couple of half-screens
    // off it at most) wherever the row leaves room for that; only where it does not may it lie out of sight.
    const seen = candidates.filter((i) => ctx.roadDist[i] * MACRO <= row.radius + PATCH_SEEN);
    // Each patch ranks the ground on its own: a cell ruled out moves the patch only if it was the patch's pick.
    const cell = ranked(rankBase(seed, `skel:area:${row.id}`, attempt), seen.length > 0 ? seen : candidates);
    out.push({ id: row.id, name: row.name, mx: cell % SKEL_W, my: Math.floor(cell / SKEL_W), row });
  }
  return out;
}

/** Per region. PLAN.md 2.4: 25 to 35 points of interest. */
export const POI_BUDGET = 38;
// Near enough that a road is never bare for long, far enough that two places never read as one.
const POI_SPACING = 90;
/** "Something visible from the road every 20 to 30 seconds of walking": 150 to 225 m at 7.5 m/s. */
/** The three sites the first walk joins. Its roads get twice the beat. */
const FIRST_WALK = new Set(["station", "julie_house", "town"]);
const ROADSIDE_EVERY_MIN = 170;
const ROADSIDE_EVERY_MAX = 250;

/**
 * Minor places. The roads are walked first and something is set beside them at a
 * steady beat, because the density rule is about what you SEE from the road; then
 * the rest of each region's budget goes deep and along the banks, for whoever leaves it.
 */
export function placePois(ctx: PlaceCtx, roads: readonly { cells: number[]; from?: string; to?: string }[], rows: readonly PoiRow[], seed: number, attempt: number): PlacedPoi[] {
  const out: PlacedPoi[] = [];
  // Nothing here shares dice with anything else. What kind a place is, is its cell's rank; each road's
  // beat and its spots are that road's own ranking; the deep scatter throws its own stream. So the rail
  // re-laid, or one more road, moves the small places along it and leaves the rest where they stood.
  const kindBase = rankBase(seed, "skel:poi:kind", attempt);
  const unit = (base: number, n: number): number => rankOf(base, n) / 4294967296;
  const count = [0, 0, 0];
  const free = (x: number, y: number): boolean => {
    if (!inside(x, y) || x < 2 || y < 2 || x > SKEL_W - 3 || y > SKEL_H - 3) return false;
    const i = at(x, y);
    if (ctx.t.water[i] || ctx.road[i] & ROAD || ctx.nearRail?.[i]) return false;
    for (const s of ctx.sites) if (metres(x, y, s.mx, s.my) < Math.max(110, (s.row.hub ?? 0) + 40)) return false;
    for (const p of out) if (metres(x, y, p.mx, p.my) < POI_SPACING) return false;
    return true;
  };
  const kindFor = (region: Region, where: PoiRow["where"], x: number, y: number): PoiRow | null => {
    const fits = rows.filter((r) => r.regions.includes(REGION_IDS[region]) && (r.where === where || r.where === "any"));
    if (fits.length === 0) return null;
    let roll = unit(kindBase, at(x, y)) * fits.reduce((n, r) => n + r.weight, 0);
    for (const r of fits) if ((roll -= r.weight) <= 0) return r;
    return fits[fits.length - 1];
  };
  const add = (x: number, y: number, where: PoiRow["where"]): boolean => {
    const region = ctx.t.region[at(x, y)] as Region;
    if (count[region] >= POI_BUDGET) return false;
    const row = kindFor(region, where, x, y);
    if (!row) return false;
    out.push({ kind: row.kind, name: row.name, mx: x, my: y, region });
    count[region]++;
    return true;
  };

  for (const road of roads) {
    // The first walk is the establishing shot: the station to Julie's to the town, two or three
    // minutes on a lit road with nothing dangerous on it. If THAT stretch is empty, the county reads
    // as empty however full the rest of it is, so it gets something to look at about twice as often.
    const first = FIRST_WALK.has(road.from ?? "") && FIRST_WALK.has(road.to ?? "");
    const min = first ? ROADSIDE_EVERY_MIN * 0.5 : ROADSIDE_EVERY_MIN;
    const max = first ? ROADSIDE_EVERY_MAX * 0.5 : ROADSIDE_EVERY_MAX;
    const name = road.from === "rail" ? "rail" : `${road.from}>${road.to}`;
    const beat = rankBase(seed, `skel:poi:beat:${name}`, attempt);
    const side = rankBase(seed, `skel:poi:side:${name}`, attempt);
    let since = 0;
    let next = min + unit(beat, 0) * (max - min);
    for (let k = 1; k < road.cells.length; k++) {
      since += MACRO * 1.2;
      if (since < next) continue;
      const c = road.cells[k];
      const cx = c % SKEL_W;
      const cy = Math.floor(c / SKEL_W);
      // One or two cells off the road, either side, seeded.
      const spots: [number, number][] = [];
      for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) if (Math.max(Math.abs(ox), Math.abs(oy)) >= 1 && free(cx + ox, cy + oy)) spots.push([cx + ox, cy + oy]);
      if (spots.length === 0) continue;
      const cell = ranked(side, spots.map(([x, y]) => at(x, y)));
      const x = cell % SKEL_W;
      const y = Math.floor(cell / SKEL_W);
      if (add(x, y, "roadside")) {
        since = 0;
        next = min + unit(beat, k) * (max - min);
      }
    }
  }

  // The rest of the budget: banks, then deep country. Bounded tries, so a cramped region ends short, not in a loop.
  const rng = stepDice(seed, "skel:poi:deep", attempt);
  for (let tries = 0; tries < 4000 && (count[0] < POI_BUDGET || count[1] < POI_BUDGET || count[2] < POI_BUDGET); tries++) {
    const x = 2 + Math.floor(rngFloat(rng) * (SKEL_W - 4));
    const y = 2 + Math.floor(rngFloat(rng) * (SKEL_H - 4));
    if (!free(x, y)) continue;
    const i = at(x, y);
    const bank = ctx.t.wet[i] >= 1.5 && ctx.t.wet[i] <= 4;
    const deep = ctx.roadDist[i] * MACRO >= 130;
    if (bank) add(x, y, "bank");
    else if (deep) add(x, y, "deep");
  }
  return out;
}
