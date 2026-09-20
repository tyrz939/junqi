// buildSkeleton(seed): the county decided, in a few milliseconds, as data.
// Land, then the story's places, then the roads between them, then how dangerous
// each patch is, then the small things beside the road. Then every row is CHECKED
// against what was built, and a skeleton that fails is thrown away and the next
// attempt tried. The player never sees a county that breaks the story.

import areasJson from "@/data/areas.json";
import poisJson from "@/data/pois.json";
import sitesJson from "@/data/sites.json";
import { rngFloat, rngSeed } from "@/sim/rng";
import { pickCell, placeAreas, placePois, POI_BUDGET, siteCandidates } from "@/world/skeleton/place";
import { distanceToRoad, layRoad, roadDistances, route, type Land } from "@/world/skeleton/roads";
import { buildTerrain, type Terrain } from "@/world/skeleton/terrain";
import {
  at,
  inside,
  MACRO,
  metres,
  Region,
  ROAD,
  ROAD_LIT,
  SKEL_H,
  SKEL_W,
  type AreaRow,
  type Check,
  type PlacedArea,
  type PlacedSite,
  type PoiRow,
  type Road,
  type SiteRow,
  type Skeleton,
  type SkeletonRows,
} from "@/world/skeleton/types";

export * from "@/world/skeleton/types";

export const SKELETON_ROWS: SkeletonRows = {
  sites: sitesJson as unknown as SiteRow[],
  areas: areasJson as unknown as AreaRow[],
  pois: poisJson as unknown as PoiRow[],
};

export const MAX_ATTEMPTS = 40;
/** Spots tried for one site before the attempt is given up. */
const SITE_TRIES = 14;

/** A dungeon's approach is part of the dungeon: threat rises by one inside this ring (m). */
const DUNGEON_RING = 170;
/** The longest stretch of road with nothing to see from it (m). PLAN.md 2.4: "up to ~2 minutes, used on purpose". */
const LONGEST_EMPTY = 900;
const SEEN_FROM_ROAD = 120;

export function buildSkeleton(seed: number, rows: SkeletonRows = SKELETON_ROWS): Skeleton {
  let last: Skeleton | null = null;
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const s = tryBuild(seed >>> 0, attempt, rows);
    if (s) {
      if (s.ok) return s;
      last = s;
    }
  }
  if (last) return last;
  throw new Error(`Seed ${seed}: no attempt could even place the story's sites. The rows in sites.json contradict each other.`);
}

function tryBuild(seed: number, attempt: number, rows: SkeletonRows): Skeleton | null {
  const t = buildTerrain(seed, attempt);
  const rng = rngSeed(seed, 200 + attempt);
  // Sites and roads together, one row at a time: stand the site somewhere its row allows,
  // lay its road from the site it hangs off, and measure the row's road rules on the network
  // as it now is. A spot that fails is dropped and another tried, so one awkward site costs
  // a few retries, not the whole county. Row order is road order: the first walk is laid
  // first, and everything later bends toward it.
  const sites: PlacedSite[] = [];
  const site = (id: string): PlacedSite | undefined => sites.find((s) => s.id === id);
  let land: Land = { height: t.height, water: t.water, rough: t.rough, road: new Uint8Array(SKEL_W * SKEL_H) };
  const roads: Road[] = [];
  for (const row of rows.sites) {
    let candidates = siteCandidates(t, row, sites);
    const off = row.where?.offRoad;
    if (off !== undefined) {
      const d = distanceToRoad(land.road);
      candidates = candidates.filter((c) => d[c] * MACRO >= off);
    }
    const from = row.roadFrom ? site(row.roadFrom) : undefined;
    let done = false;
    for (let tries = 0; tries < SITE_TRIES && candidates.length > 0 && !done; tries++) {
      const cell = pickCell(rng, candidates);
      const here: PlacedSite = { id: row.id, name: row.name, mx: cell % SKEL_W, my: Math.floor(cell / SKEL_W), row };
      if (!from) {
        sites.push(here);
        done = true;
        break;
      }
      const trial: Land = { ...land, road: land.road.slice() };
      const cells = route(trial, from.mx, from.my, here.mx, here.my);
      if (!cells) continue;
      const laid = layRoad(trial, from.id, here.id, cells);
      let ok = true;
      for (const rule of row.dist ?? []) {
        const other = site(rule.to);
        if (rule.by !== "road" || !other) continue;
        const d = roadDistances(trial.road, other.mx, other.my)[cell];
        if ((rule.min !== undefined && d < rule.min) || (rule.max !== undefined && d > rule.max)) ok = false;
      }
      if (!ok) continue;
      land = trial;
      roads.push(laid);
      sites.push(here);
      done = true;
    }
    if (!done) return null;
  }
  const road = land.road;

  // The first walk: station to Julie's to the town. Nothing above threat 1 may touch it by day.
  const safe = new Uint8Array(SKEL_W * SKEL_H);
  for (const r of roads) {
    if (!((r.from === "station" && r.to === "julie_house") || (r.from === "julie_house" && r.to === "town"))) continue;
    for (const c of r.cells) safe[c] = 1;
  }

  const roadDist = distanceToRoad(road);
  // A site that must not be seen from a road is checked now that there are roads. Cheaper to re-roll than to repair.
  for (const s of sites) {
    const off = s.row.where?.offRoad;
    if (off !== undefined && roadDist[at(s.mx, s.my)] * MACRO < off) return null;
  }

  // The road rules are the ones most likely to fail, and they need nothing below this line.
  // Check them now, so a bad attempt costs a few milliseconds instead of the whole build.
  const early: Skeleton = {
    seed, attempt, w: SKEL_W, h: SKEL_H, height: t.height, water: t.water, region: t.region, biome: t.biome,
    road, threat: new Uint8Array(SKEL_W * SKEL_H), sites, areas: [], pois: [], roads, checks: [], ok: false,
  };
  early.checks = roadChecks(early);
  if (!early.checks.every((c) => c.ok)) return early;

  const ctx = { t, road, roadDist, sites, safeDist: distanceToRoad(safe, 1) };
  const areas = placeAreas(ctx, rows.areas, rng);
  lightLamps(seed, road, roads, site("town"), safe, t);
  const pois = placePois(ctx, roads, rows.pois, rng);
  const threat = buildThreat(t, sites, areas, road);

  const skeleton: Skeleton = {
    seed,
    attempt,
    w: SKEL_W,
    h: SKEL_H,
    height: t.height,
    water: t.water,
    region: t.region,
    biome: t.biome,
    road,
    threat,
    sites,
    areas,
    pois,
    roads,
    checks: [],
    ok: false,
  };
  skeleton.checks = validate(skeleton, safe);
  skeleton.ok = skeleton.checks.every((c) => c.ok);
  return skeleton;
}

/**
 * Lamp posts are a road attribute. The first walk is lit end to end, and so is the
 * town. Beyond that they come in runs and fail with distance: fewer east of the
 * river, almost none in the Works. Electric relights dead runs later, for good.
 */
function lightLamps(seed: number, road: Uint8Array, roads: readonly Road[], town: PlacedSite | undefined, safe: Uint8Array, t: Terrain): void {
  roads.forEach((r, n) => {
    r.cells.forEach((c, k) => {
      const x = c % SKEL_W;
      const y = Math.floor(c / SKEL_W);
      const d = town ? metres(x, y, town.mx, town.my) : 1e9;
      let p = Math.max(0, 0.75 - d / 2400);
      if (t.region[c] === Region.Waters) p *= 0.5;
      if (t.region[c] === Region.Works) p = 0.06;
      // Decided per run of six cells (about 100 m), so lamps stand in rows and go dark in rows.
      const run = rngFloat(rngSeed(seed, 9000 + n * 512 + Math.floor(k / 6)));
      if (safe[c] || d < 420 || run < p) road[c] |= ROAD_LIT;
    });
  });
}

/** Daytime threat, 0..6. Layers in PLAN.md 2.6, applied in this order. */
function buildThreat(t: Terrain, sites: readonly PlacedSite[], areas: readonly PlacedArea[], road: Uint8Array): Uint8Array {
  const threat = new Uint8Array(SKEL_W * SKEL_H);
  const school = sites.find((s) => s.id === "school");
  for (let y = 0; y < SKEL_H; y++) {
    for (let x = 0; x < SKEL_W; x++) {
      const i = at(x, y);
      const r = t.region[i] as Region;
      if (r === Region.Lowfields) threat[i] = 1;
      else if (r === Region.Waters) threat[i] = (x - t.riverX[y]) * MACRO > 900 ? 3 : 2;
      else threat[i] = school && metres(x, y, school.mx, school.my) < 700 ? 5 : 4;
    }
  }
  const stamp = (mx: number, my: number, radius: number, fn: (old: number) => number): void => {
    const reach = Math.ceil(radius / MACRO);
    for (let y = my - reach; y <= my + reach; y++) {
      for (let x = mx - reach; x <= mx + reach; x++) {
        if (inside(x, y) && metres(x, y, mx, my) <= radius) threat[at(x, y)] = fn(threat[at(x, y)]);
      }
    }
  };
  for (const a of areas) stamp(a.mx, a.my, a.row.radius, () => a.row.threat);
  for (const s of sites) if (s.row.dungeon) stamp(s.mx, s.my, DUNGEON_RING, (old) => Math.min(6, old + 1));
  // A road is the safer way, always: one less, never below one.
  for (let i = 0; i < threat.length; i++) if (road[i] & ROAD) threat[i] = Math.max(1, threat[i] - 1);
  for (const s of sites) if (s.row.hub) stamp(s.mx, s.my, s.row.hub, () => 0);
  return threat;
}

/** Night is a rule on top of the field, not a second field: +1 outside lamplight, +2 in the Works. Havens stay havens. */
export function threatAt(s: Skeleton, mx: number, my: number, night: boolean): number {
  const i = at(mx, my);
  const base = s.threat[i];
  if (!night || base === 0 || s.road[i] & ROAD_LIT) return base;
  // An unlit road loses its discount after dark, then the night is added.
  const unlitRoad = s.road[i] & ROAD ? 1 : 0;
  return Math.min(6, base + unlitRoad + (s.region[i] === Region.Works ? 2 : 1));
}

/** Distance rules and reachability: everything that depends only on sites and roads. */
function roadChecks(s: Skeleton): Check[] {
  const checks: Check[] = [];
  const check = (rule: string, ok: boolean, detail: string): void => void checks.push({ rule, ok, detail });
  const site = (id: string): PlacedSite | undefined => s.sites.find((x) => x.id === id);

  // Every road rule, measured along the network that was actually built.
  const cache = new Map<string, Float64Array>();
  const along = (a: PlacedSite, b: PlacedSite): number => {
    let d = cache.get(a.id);
    if (!d) cache.set(a.id, (d = roadDistances(s.road, a.mx, a.my)));
    return d[at(b.mx, b.my)];
  };
  for (const a of s.sites) {
    for (const rule of a.row.dist ?? []) {
      const b = site(rule.to);
      if (!b) continue;
      const d = rule.by === "road" ? along(b, a) : metres(a.mx, a.my, b.mx, b.my);
      const ok = d !== Infinity && (rule.min === undefined || d >= rule.min) && (rule.max === undefined || d <= rule.max);
      check(`${a.id} ${rule.min ?? 0}..${rule.max ?? "∞"} m from ${rule.to} by ${rule.by}`, ok, `${Math.round(d)} m`);
    }
  }

  // Every story place that should be on the network is reachable from the platform.
  const station = site("station");
  if (station) {
    const from = roadDistances(s.road, station.mx, station.my);
    for (const x of s.sites) {
      if (x.row.onRoad) check(`${x.id} is reachable by road`, from[at(x.mx, x.my)] !== Infinity, `${Math.round(from[at(x.mx, x.my)])} m from the station`);
    }
  }

  const bridges = countBridges(s);
  check("the river is crossed in one to three places", bridges >= 1 && bridges <= 3, `${bridges}`);
  return checks;
}

function validate(s: Skeleton, safe: Uint8Array): Check[] {
  const checks = roadChecks(s);
  const check = (rule: string, ok: boolean, detail: string): void => void checks.push({ rule, ok, detail });

  // The first evening is a walk, not a fight.
  let worst = 0;
  for (let i = 0; i < safe.length; i++) if (safe[i]) worst = Math.max(worst, s.threat[i]);
  check("the walk from the station to Julie's and on to town never crosses threat above 1 by day", worst <= 1, `worst ${worst}`);

  // Every region has somewhere to rest that is no more dangerous than the region itself.
  const base = [1, 3, 5];
  for (const r of [Region.Lowfields, Region.Waters, Region.Works]) {
    const rests = s.sites.filter((x) => x.row.rest && s.region[at(x.mx, x.my)] === r);
    const ok = rests.some((x) => s.threat[at(x.mx, x.my)] <= base[r]);
    check(`region ${r} has a bed or a fire inside its own threat`, ok, rests.map((x) => x.id).join(", ") || "none");
  }

  // Density: the budget, and the longest stretch of road with nothing to look at.
  for (const r of [Region.Lowfields, Region.Waters, Region.Works]) {
    const n = s.pois.filter((p) => p.region === r).length;
    check(`region ${r} has 22 to ${POI_BUDGET} points of interest`, n >= 22 && n <= POI_BUDGET, `${n}`);
  }
  let longest = 0;
  for (const r of s.roads) {
    let run = 0;
    for (const c of r.cells) {
      const x = c % SKEL_W;
      const y = Math.floor(c / SKEL_W);
      const seen =
        s.pois.some((p) => metres(x, y, p.mx, p.my) <= SEEN_FROM_ROAD) || s.sites.some((p) => metres(x, y, p.mx, p.my) <= SEEN_FROM_ROAD + 40);
      run = seen ? 0 : run + MACRO * 1.2;
      longest = Math.max(longest, run);
    }
  }
  check(`no stretch of road longer than ${LONGEST_EMPTY} m has nothing to see`, longest <= LONGEST_EMPTY, `${Math.round(longest)} m`);
  return checks;
}

/** Bridge cells that touch each other are one bridge. */
export function countBridges(s: Skeleton): number {
  const seen = new Uint8Array(s.road.length);
  let n = 0;
  for (let i = 0; i < s.road.length; i++) {
    if (!(s.road[i] & 4) || seen[i]) continue;
    n++;
    const stack = [i];
    seen[i] = 1;
    while (stack.length > 0) {
      const c = stack.pop()!;
      const cx = c % SKEL_W;
      const cy = Math.floor(c / SKEL_W);
      for (let oy = -2; oy <= 2; oy++) {
        for (let ox = -2; ox <= 2; ox++) {
          if (!inside(cx + ox, cy + oy)) continue;
          const j = at(cx + ox, cy + oy);
          if (s.road[j] & 4 && !seen[j]) {
            seen[j] = 1;
            stack.push(j);
          }
        }
      }
    }
  }
  return n;
}
