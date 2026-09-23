// The county: 2000 x 2000 cells, 2 km square, five minutes across by road. (3.6 km by 2 km until
// Sept 24, 2026: the walks were too long for what was on them. The height stayed; the width came in.)
// (2020's room_zone1 was 640 x 384. PLAN.md 2.1 is why this one is not.)
//
// It is built FROM the skeleton (world/skeleton): that decided, on a coarse grid,
// where the river runs, where the story's places stand, which roads join them, which
// lamps still work and how dangerous each patch is. This file turns that into cells:
//
//   1 land and water   every cell, from the skeleton's biome and water, edges softened
//   2 roads            the skeleton's routes as strokes; a road over water is a bridge
//   3 set chunks       the authored places, stamped where the skeleton put them
//   4 links            every road joined to a gate of every chunk it meets, going round
//   5 dressing         lamps where the skeleton lit them, small places, herbs, rocks
//   6 wildlife         by region and by threat; nothing on the first walk, nothing in a haven
//
// Same seed, same county. The story's names (dog, house_door, stoop, mine_mouth...)
// are the contract; every coordinate is the seed's.

import { F_SOLID, Tile, TILE_FLAGS } from "@/sim/grid";
import type { ActionList } from "@/sim/state";
import { hashString } from "@/sim/rng";
import type { Blueprint, Rect } from "@/world/blueprint";
import { CHUNKS, type Chunk, type Gate } from "@/world/chunks";
import doorsJson from "@/data/doors.json";
import pathsJson from "@/data/paths.json";
import { AREA_DRESS } from "@/world/areas";
import { Kit } from "@/world/kit";
import { applyPlacements, claimPois, PLACEMENTS, type PlaceCtx, type PlacementRow, type PoiSpot } from "@/world/placements";
import { propFootprints } from "@/sim/catalog";
import { hasZone } from "@/world/registry";
import { at, Biome, buildSkeleton, COUNTY_H, COUNTY_W, MACRO, Region, ROAD_LIT, SKEL_H, SKEL_W, type Skeleton } from "@/world/skeleton";
import { compass, countryPlaces, distanceWords, furnishCountry, type Country } from "@/world/country";
import { claimPlaces, nameBoards, storyRecord } from "@/world/stories";

export { COUNTY_H, COUNTY_W };

/** A footpath: from one site to another by way of a named small place or patch. `marks` name its two ends. */
type PathRow = { id: string; from: string; via: string; to: string; width: number; marks: [string, string] };
const PATHS = pathsJson as unknown as PathRow[];

/**
 * A way into a dungeon. Either `chunk` (set into that landmark's face) or `near` (stood on open
 * ground beside that site). `mark` also leaves a county mark here, which is
 * where the dungeon's own way out arrives; `fromBelow` makes it a mark only, for a way that opens
 * from the other side: a manhole lifts from the pipes, never from the street.
 */
type DoorRow = { zone: string; chunk?: string; near?: string; def?: string; key: string; label: string; keyTag?: string; nightLock?: string; mark?: string; fromBelow?: boolean };
const DOORS = doorsJson as unknown as DoorRow[];

/** The metal, in cells. A lane, not a trunk road: two people pass, a cart takes all of it. */
const ROAD_WIDTH = 3;
/** Trodden verge each side of it. */
const VERGE = 1;
const FIELD_HERBS = ["pansy", "nasturtium", "honeylace_lily", "hemshade_root"];
const WATER_HERBS = ["white_water_cap", "white_water_rose", "honeylace_lily", "night_lich_moss"];
const WORKS_HERBS = ["night_lich_moss", "savage_snakeroot", "hemshade_root"];

let footprints: Record<string, { w: number; h: number }> | null = null;

// One skeleton per seed is plenty to remember: a county is asked for once a game, and again only when validation re-rolls it.
let lastSkeleton: { key: string; list: Skeleton[] } | null = null;

/** The `attempt`-th valid skeleton of a seed. Attempt 0 is the county the seed viewer shows. */
export function countySkeleton(seed: number, attempt: number): Skeleton {
  const key = String(seed >>> 0);
  if (!lastSkeleton || lastSkeleton.key !== key) lastSkeleton = { key, list: [] };
  const list = lastSkeleton.list;
  while (list.length <= attempt) {
    const from = list.length === 0 ? 0 : list[list.length - 1].attempt + 1;
    list.push(buildSkeleton(seed, undefined, from));
  }
  return list[attempt];
}

export function buildCounty(seed: number, attempt: number, rows: readonly PlacementRow[] = PLACEMENTS): Blueprint {
  const sk = countySkeleton(seed, attempt);
  const k = new Kit("county", COUNTY_W, COUNTY_H, seed, attempt, Tile.Grass);
  paintLand(k, sk, seed);
  // Patches that are places, not only a threat number (the allotments, the planted field, the quarry face).
  // Before the roads and chunks, so a road that crosses one simply crosses it.
  const areaSlots: Record<string, [number, number]> = {};
  for (const a of sk.areas) Object.assign(areaSlots, AREA_DRESS[a.id]?.(k, a.mx * MACRO + MACRO / 2, a.my * MACRO + MACRO / 2, a.row.radius) ?? {});

  // Tree line round the edge: the county is a bowl, not a plane.
  k.fill(0, 0, COUNTY_W, 4, Tile.Tree);
  k.fill(0, COUNTY_H - 4, COUNTY_W, 4, Tile.Tree);
  k.fill(0, 0, 4, COUNTY_H, Tile.Tree);
  k.fill(COUNTY_W - 4, 0, 4, COUNTY_H, Tile.Tree);

  // --- 2 roads ------------------------------------------------------------------
  const centre = (m: number): number => m * MACRO + MACRO / 2;
  const lines: [number, number][][] = [];
  const lit: boolean[][] = [];
  // The ground as it was before any road: where a road crosses water, it is a bridge.
  const before = k.tiles.slice();
  const half = (ROAD_WIDTH >> 1) + VERGE;
  for (const r of sk.roads) {
    const pts = r.cells.map((c): [number, number] => [centre(c % SKEL_W), centre(Math.floor(c / SKEL_W))]);
    // The centre line first, then the metal and the verge painted round it with a ROUND brush: a
    // square brush drawn along a diagonal makes a road half as wide again as the same road running
    // straight, and a lane that swells at every bend reads as a mistake.
    const line = k.stroke(pts, 1, Tile.Road, 1);
    const metal = ((ROAD_WIDTH >> 1) + 0.25) ** 2;
    const verge = (half + 0.25) ** 2;
    // A verge along every road, a cell each side of the metal, laid along the road's OWN line so it
    // never wanders off it. Grass does not meet a made road edge on: there is always a band of
    // trodden dirt, and drawing it is what stops the road reading as a stripe laid over a field.
    // Not over water: a bridge is the road and nothing else.
    for (const [x, y] of line) {
      for (let oy = -half; oy <= half; oy++) {
        for (let ox = -half; ox <= half; ox++) {
          const d = ox * ox + oy * oy;
          if (d > verge) continue;
          const t = k.get(x + ox, y + oy);
          if (d <= metal) k.set(x + ox, y + oy, Tile.Road);
          else if (t !== Tile.Road && t !== Tile.Water) k.set(x + ox, y + oy, Tile.Dirt);
        }
      }
    }
    lines.push(line);
    lit.push(line.map(([x, y]) => (sk.road[at(Math.min(SKEL_W - 1, x >> 4), Math.min(SKEL_H - 1, y >> 4))] & ROAD_LIT) !== 0));
  }
  // Where a road crosses water it is a bridge, and a bridge is a deck of planks, not a stripe of road on the river.
  for (const line of lines) {
    for (const [x, y] of line) {
      for (let oy = -1; oy <= 1; oy++) {
        for (let ox = -1; ox <= 1; ox++) {
          const i = (y + oy) * COUNTY_W + x + ox;
          if (before[i] === Tile.Water && k.tiles[i] === Tile.Road) k.tiles[i] = Tile.Boardwalk;
        }
      }
    }
  }
  // The Burial Chamber is off the road on purpose. A footpath from the graveyard finds it, and only that.
  const grave = sk.sites.find((s) => s.id === "graveyard");
  const burial = sk.sites.find((s) => s.id === "burial");
  if (grave && burial) {
    lines.push(k.stroke([[centre(grave.mx), centre(grave.my)], [centre(burial.mx), centre(burial.my)]], 2, Tile.Dirt, 2));
    lit.push([]);
  }
  // Footpaths (data/paths.json): site to site by way of a named small place. Laid like roads, centre to
  // centre, so the chunks overwrite their ends and the links below bring them round to a gate.
  const footpaths: { row: PathRow; line: [number, number][] }[] = [];
  for (const row of PATHS) {
    const from = sk.sites.find((s) => s.id === row.from);
    const to = sk.sites.find((s) => s.id === row.to);
    const via = sk.anchors.find((a) => a.id === row.via) ?? sk.areas.find((a) => a.id === row.via);
    if (!from || !to || !via) continue;
    const line = k.stroke(
      [
        [centre(from.mx), centre(from.my)],
        [centre(via.mx), centre(via.my) + 5],
        [centre(to.mx), centre(to.my)],
      ],
      row.width,
      Tile.Dirt,
      2,
    );
    lines.push(line);
    lit.push([]);
    footpaths.push({ row, line });
  }

  // --- 3 set chunks, 4 links ------------------------------------------------------
  const chunks: Chunk[] = [];
  for (const s of sk.sites) {
    const build = CHUNKS[s.id];
    if (build) chunks.push(build(k, centre(s.mx), centre(s.my)));
  }
  // Each set place's ground, by name, so a measure of the open country can tell it from a town.
  for (const c of chunks) if (!k.rects[`site_${c.id}`]) k.rect(`site_${c.id}`, c.box);
  for (const line of lines) for (const c of chunks) linkRoad(k, line, c);
  // The railway: through the halt and on, off the map at both ends (skeleton/rail.ts). After the roads,
  // so where they meet the road keeps its metal (a level crossing), and after the chunks, so the halt's
  // own platform rails stay as they were drawn.
  layRailway(k, sk);
  // A footpath's two ends get a mark each, a little way out from the chunk it leaves: where the fingerpost stands.
  for (const { row, line } of footpaths) {
    const outside = (p: readonly [number, number]): boolean => chunks.every((c) => p[0] < c.box.cx - 8 || p[1] < c.box.cy - 8 || p[0] >= c.box.cx + c.box.w + 8 || p[1] >= c.box.cy + c.box.h + 8);
    const a = line.findIndex(outside);
    const b = line.length - 1 - [...line].reverse().findIndex(outside);
    if (a < 0 || b <= a) continue;
    const pa = line[Math.min(b, a + 24)];
    const pb = line[Math.max(a, b - 24)];
    const ends = [
      [row.marks[0], pa, row.to],
      [row.marks[1], pb, row.from],
    ] as const;
    for (const [name, p, toward] of ends) {
      clearing(k, p[0], p[1]);
      k.mark(name, p[0], p[1], 1);
      // A path end the quests already furnish (data/placements: its own post or sign, saying more) gets no
      // second post from here: two fingerposts and a sign at one stile read as clutter, not as care.
      if (rows.some((r) => (r.at as { mark?: string }).mark === name)) continue;
      // A fingerpost where the footpath leaves the road, saying where it goes. Beside the stile, not on it.
      const there = sk.sites.find((s) => s.id === toward);
      const metres = Math.round((Math.abs(b - a) * 1.1) / 50) * 50;
      for (const [ox, oy] of [[2, -1], [-3, -1], [2, 1], [-3, 1]] as const) {
        if (!there || !k.fits(p[0] + ox, p[1] + oy, 2, 1)) continue;
        k.prop({ def: "fingerpost", cx: p[0] + ox, cy: p[1] + oy, label: "A fingerpost", use: [{ do: "read", text: `FOOTPATH. ${there.name.toUpperCase()}, ${metres < 1000 ? `${metres} m` : `${(metres / 1000).toFixed(1)} km`}.` }] }, 2, 1);
        break;
      }
    }
  }

  // Content placed by name (data/placements): what goes inside a chunk goes in now, while its open ground is still open.
  const pois: PoiSpot[] = sk.pois.map((p) => ({ x: centre(p.mx), y: centre(p.my), kind: p.kind, anchor: p.anchor }));
  const place: PlaceCtx = {
    k,
    areaSlots,
    sk,
    chunks,
    pois,
    claimed: claimPois(sk, pois, rows),
    sizes: (footprints ??= propFootprints()),
    threatAt: (cx, cy) => sk.threat[at(Math.min(SKEL_W - 1, cx >> 4), Math.min(SKEL_H - 1, cy >> 4))],
  };
  // Doors into the dungeons: one per landmark, set into its face, but ONLY for a zone that
  // exists (world/registry.ts). The county grows a door the day its dungeon lands; until then
  // the face is blank. Nothing else has to change when a dungeon is added.
  footprints ??= propFootprints();
  for (const d of DOORS) {
    if (!hasZone(d.zone)) continue;
    const chunk = chunks.find((c) => c.id === (d.chunk ?? d.near));
    // Either set into a landmark's face, or stood on open ground beside a place (a grate in the
    // town, a manhole cover on the road): the pipes have no building of their own.
    let cell = d.chunk ? chunk?.slots?.[`${d.chunk}_door`] : undefined;
    let size = { w: 2, h: 2 };
    if (!d.chunk && chunk) {
      size = footprints![d.def ?? "door"] ?? size;
      // Room for the cover and for somebody to stand in front of it: open ground three deep below it.
      const spot = k.spot({ cx: chunk.box.cx - 14, cy: chunk.box.cy - 14, w: chunk.box.w + 28, h: chunk.box.h + 28 }, size.w, size.h + 3, 1, 200);
      cell = spot ? [spot.cx, spot.cy] : undefined;
    }
    if (!cell) continue;
    k.prop(
      {
        key: d.key,
        def: d.def ?? "door",
        cx: cell[0],
        cy: cell[1],
        locked: d.keyTag !== undefined,
        keyTag: d.keyTag,
        to: d.fromBelow ? undefined : { zone: d.zone, mark: "entry" },
        label: d.label,
        nightLock: d.nightLock,
      },
      size.w,
      size.h,
    );
    // A mark the dungeon's own door comes back out at. The way down opens from below (a manhole
    // lifts from the pipes side), so this end is a mark first and a door only when the row says so.
    if (d.mark) k.mark(d.mark, cell[0], cell[1] + size.h, 1);
    // The ground in front of a way in is the way in: nothing of the country's is built across it.
    k.claim(cell[0] - 1, cell[1] + size.h, size.w + 2, 4);
  }

  applyPlacements(place, "chunks", rows);
  for (const c of chunks) k.claim(c.box.cx - 6, c.box.cy - 6, c.box.w + 12, c.box.h + 12);

  // --- 5 dressing -------------------------------------------------------------------
  // The roads' furniture first, while their margins are open: lamps on one side at an even step,
  // a lamp at each end of a bridge, a fingerpost at every fork, milestones (world/country.ts).
  const country: Country = { k, sk, chunks, lines, lit, before, sizes: (footprints ??= propFootprints()) };
  furnishCountry(country, "roads");
  for (const line of lines) for (const [x, y] of line) k.claim(x - 3, y - 3, 7, 7);
  // Small places: dressed as the kind the seed rolled, or the kind a placement row needed them to be.
  // The Factory's reward, out here where it counts: the longest dark stretches of road get a relay
  // box at the head and a run of dead lamps along them. Spark the box and that road is lit for good,
  // which is the difference between a road at night and a road at night you can see along. She walks
  // past every one of them long before she can do anything about them.
  if (hasZone("factory")) relayRuns(k, lines, lit);

  pois.forEach((p, n) => smallPlace(k, p.kind, p.x, p.y, p.anchor ?? `poi_${n}`));
  // A signpost says something. The seed's own roadside posts were drawn with nothing on them, and a sign
  // you cannot read is worse than no sign: it names the two nearest places, which way, and how far.
  for (const p of k.props) {
    if (p.def !== "signpost" || p.talk || p.use) continue;
    // Somebody's house is not a place a signpost names: Julie's no more than anybody else's.
    const near = sk.sites
      .filter((s) => s.id !== "julie_house")
      .map((s) => ({ s, dx: centre(s.mx) - p.cx, dy: centre(s.my) - p.cy }))
      .map((o) => ({ ...o, m: Math.hypot(o.dx, o.dy) }))
      .sort((a, b) => a.m - b.m)
      .slice(0, 2);
    if (near.length === 0) continue;
    p.label ??= "A signpost";
    p.use = [{ do: "read", text: `${near.map((o) => `${o.s.name.toUpperCase()}, ${compass(o.dx, o.dy)}, ${distanceWords(o.m)}`).join(". ")}.` }];
  }
  // A place the story needs is known by its name: a mark (made above) and a rect of the same name round it.
  for (const p of pois) if (p.anchor && k.marks[p.anchor]) k.rect(p.anchor, { cx: p.x - 6, cy: p.y - 4, w: 13, h: 11 });
  applyPlacements(place, "pois", rows);
  for (const p of pois) k.claim(p.x - 7, p.y - 6, 15, 13);
  // What the quests put in the named patches goes down before the country fills up round it.
  applyPlacements(place, "areas", rows);
  // Everything else a county has in it: field edges, hamlets, farms, camps, dens, ruins, ponds.
  furnishCountry(country, "places");
  // The stories told at those places (world/stories.ts): each claims one, the boards go up with the
  // places' names on them, and each story's rows go into the place it claimed.
  const built = countryPlaces(country);
  const claimed = claimPlaces(seed, k, sk, built, rows);
  nameBoards(seed, k, built, claimed.claims);
  // Each story place by name: its footprint as a rect, and a mark on the ground in front of its board (where
  // `dev tp` puts a tester, and where a person reading the board stands).
  for (const [id, p] of claimed.claims) {
    k.rect(`story_${id}`, { ...p.box });
    const b = p.slots.board;
    if (b) k.mark(`story_${id}`, b[0], b[1] + 1);
  }
  applyPlacements({ ...place, claims: claimed.claims }, "places", rows);
  scatter(k, sk);

  // --- 6 wildlife ---------------------------------------------------------------------
  // By region, biome and threat, kept back from the roads; then anything left empty gets something.
  furnishCountry(country, "life");

  cutThrough(k);
  dropUnreachable(k);
  const bp = k.done("Castle", false, 1, attempt);
  bp.stories = storyRecord(seed, claimed);
  return bp;
}

/**
 * A named place (a story's ruin, a cottage the quests send her to) that the wood has closed round is
 * not lost: somebody cut a way to it. Flood from the platform; for each named mark the flood never
 * reached, find the shortest way out to ground it did reach, going through trees, scrub and rubble
 * but never water, a wall or a fence, and clear that way to a trodden path. The square county (Sept
 * 24) puts more of its places between a wood and the river, where this used to throw the county away.
 */
function cutThrough(k: Kit): void {
  const start = k.marks.start;
  if (!start) return;
  const w = k.w;
  const n = w * k.h;
  const flags = TILE_FLAGS;
  const tiles = k.tiles;
  const open = (i: number): boolean => (flags[tiles[i]] & F_SOLID) === 0;
  const CUT = new Uint8Array(64);
  for (const t of [Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Bush, Tile.Hedge, Tile.Rubble]) CUT[t] = 1;
  const seen = new Uint8Array(n);
  const queue = new Int32Array(n);
  // Unrolled, as in dropUnreachable: this runs over every cell of the county.
  let tail = 0;
  const push = (j: number): void => {
    if (seen[j] || !open(j)) return;
    seen[j] = 1;
    queue[tail++] = j;
  };
  const flood = (from: number): void => {
    let head = 0;
    tail = 0;
    push(from);
    while (head < tail) {
      const i = queue[head++];
      const x = i % w;
      if (x + 1 < w) push(i + 1);
      if (x > 0) push(i - 1);
      if (i + w < n) push(i + w);
      if (i >= w) push(i - w);
    }
  };
  flood(start.cy * w + start.cx);
  const prev = new Int32Array(n);
  for (const name of Object.keys(k.marks)) {
    if (name.startsWith("poi_")) continue;
    const m = k.marks[name];
    const from = m.cy * w + m.cx;
    if (seen[from] || !open(from)) continue;
    // Breadth first out from the mark, over open ground and anything an axe can clear, to the reached country.
    prev.fill(-1);
    let head = 0;
    let end = 0;
    prev[from] = from;
    queue[end++] = from;
    let found = -1;
    const step = (i: number, j: number): void => {
      if (found >= 0 || prev[j] >= 0 || !(open(j) || CUT[tiles[j]])) return;
      prev[j] = i;
      if (seen[j]) found = j;
      else queue[end++] = j;
    };
    // Bounded: a way out is a few dozen cells of thicket, never a search of half the county.
    while (head < end && found < 0 && end < 400000) {
      const i = queue[head++];
      const x = i % w;
      if (x + 1 < w) step(i, i + 1);
      if (x > 0) step(i, i - 1);
      if (i + w < n) step(i, i + w);
      if (i >= w) step(i, i - w);
    }
    if (found < 0) continue;
    for (let i = found; i !== from; i = prev[i]) if (!open(i)) tiles[i] = Tile.Dirt;
    flood(from);
  }
}

/**
 * A small place on an island of cliff or in a ring of water is not a place. Flood from the
 * platform, the way the validator will, and forget any `poi_` mark the flood never reached.
 * Story marks are left for the validator to judge: if one of those is cut off the county is
 * wrong and must be re-rolled, not quietly trimmed.
 */
function dropUnreachable(k: Kit): void {
  const start = k.marks.start;
  if (!start) return;
  const w = k.w;
  const seen = new Uint8Array(w * k.h);
  const queue = new Int32Array(w * k.h);
  let head = 0;
  let tail = 0;
  // Locals, for the same reason as the painter's: this runs once per cell of the county.
  const flags = TILE_FLAGS;
  const solid = F_SOLID;
  const tiles = k.tiles;
  const push = (i: number): void => {
    if (seen[i] || (flags[tiles[i]] & solid) !== 0) return;
    seen[i] = 1;
    queue[tail++] = i;
  };
  push(start.cy * w + start.cx);
  while (head < tail) {
    const i = queue[head++];
    const x = i % w;
    if (x + 1 < w) push(i + 1);
    if (x > 0) push(i - 1);
    if (i + w < seen.length) push(i + w);
    if (i >= w) push(i - w);
  }
  for (const name of Object.keys(k.marks)) {
    const m = k.marks[name];
    if (name.startsWith("poi_") && !seen[m.cy * w + m.cx]) delete k.marks[name];
  }
  // The same goes for anything alive: a creature walled in where nobody can come is only a cost.
  for (let n = k.units.length - 1; n >= 0; n--) {
    const u = k.units[n];
    if (u.phase !== undefined && !seen[u.cy * w + u.cx]) k.units.splice(n, 1);
  }
}

// Imported names are copied into locals for the painter below. It touches them dozens of times a
// cell, seven million cells; through a module binding (and the test runner wraps every one in
// a getter) that is most of the build. Plain constants cost nothing.
const T = { ...Tile } as typeof Tile;
const B = {
  Field: Biome.Field,
  Hedge: Biome.Hedge,
  Wood: Biome.Wood,
  Foothill: Biome.Foothill,
  Reed: Biome.Reed,
  Marsh: Biome.Marsh,
  WetWood: Biome.WetWood,
  Garden: Biome.Garden,
  Slag: Biome.Slag,
  Yard: Biome.Yard,
  Hill: Biome.Hill,
} as const;
const M = MACRO;
const SW = SKEL_W;
const SH = SKEL_H;
const CW = COUNTY_W;

// --- land ----------------------------------------------------------------------------

function hash01(seed: number, x: number, y: number): number {
  let h = (seed ^ Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** A lattice of hashed values; `sample` reads it smoothly. Coarse noise for seven million cells without a noise call per cell. */
function lattice(seed: number, w: number, h: number): Float32Array {
  const out = new Float32Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) out[y * w + x] = hash01(seed, x, y);
  return out;
}

/**
 * Every cell's ground. The skeleton's macro cells are 16 m squares; looked up through a
 * smooth warp they stop being squares, and water is read as a smooth field so the river
 * has banks instead of steps.
 *
 * Seven million cells, so it is written block by block: inside one macro block the three
 * smooth fields (warp x, warp y, clumps) are straight lines along a row, which turns three
 * bilinear samples a cell into three additions.
 */
function paintLand(k: Kit, sk: Skeleton, seed: number): void {
  const s = (seed ^ hashString("county-land")) >>> 0;
  const LW = SW + 1;
  const warpX = lattice(s + 1, LW, SH + 1);
  const warpY = lattice(s + 2, LW, SH + 1);
  const clumps = lattice(s + 3, LW, SH + 1);
  const wet = new Float32Array(SW * SH);
  // Only blocks with water within two macro cells need the water field at all.
  const nearWater = new Uint8Array(SW * SH);
  for (let my = 0; my < SH; my++) {
    for (let mx = 0; mx < SW; mx++) {
      if (!sk.water[at(mx, my)]) continue;
      wet[at(mx, my)] = 1;
      for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) if (mx + ox >= 0 && my + oy >= 0 && mx + ox < SW && my + oy < SH) nearWater[at(mx + ox, my + oy)] = 1;
    }
  }
  const tiles = k.tiles;
  const biomes = sk.biome;
  const W = 20; // how far the warp may push a lookup, in cells, end to end
  // One macro block at a time, as its own small function: called 28,000 times it is compiled
  // once and stays compiled, where one seven-million-turn loop ran mostly in the slow tier.
  const paintBlock = (mx: number, my: number): void => {
    {
      const n = my * LW + mx;
      const watery = nearWater[my * SW + mx] === 1;
      for (let j = 0; j < M; j++) {
        const ty = j / M;
        const y = my * M + j;
        const xl = warpX[n] + (warpX[n + LW] - warpX[n]) * ty;
        const xs = (warpX[n + 1] + (warpX[n + LW + 1] - warpX[n + 1]) * ty - xl) / M;
        const yl = warpY[n] + (warpY[n + LW] - warpY[n]) * ty;
        const ys = (warpY[n + 1] + (warpY[n + LW + 1] - warpY[n + 1]) * ty - yl) / M;
        const cl = clumps[n] + (clumps[n + LW] - clumps[n]) * ty;
        const cs = (clumps[n + 1] + (clumps[n + LW + 1] - clumps[n + 1]) * ty - cl) / M;
        let row = y * CW + mx * M;
        for (let i = 0; i < M; i++, row++) {
          const x = mx * M + i;
          let u = (x + (xl + xs * i - 0.5) * W) / M - 0.5;
          let v = (y + (yl + ys * i - 0.5) * W) / M - 0.5;
          if (u < 0) u = 0;
          else if (u > SW - 1) u = SW - 1;
          if (v < 0) v = 0;
          else if (v > SH - 1) v = SH - 1;
          if (watery) {
            const u0 = u >= SW - 1 ? SW - 2 : u | 0;
            const v0 = v >= SH - 1 ? SH - 2 : v | 0;
            const tx = u - u0;
            const tv = v - v0;
            const wi = v0 * SW + u0;
            const water = wet[wi] * (1 - tx) * (1 - tv) + wet[wi + 1] * tx * (1 - tv) + wet[wi + SW] * (1 - tx) * tv + wet[wi + SW + 1] * tx * tv;
            if (water >= 0.4) {
              tiles[row] = water >= 0.5 ? T.Water : T.Sand;
              continue;
            }
          }
          const r = hash01(s, x, y);
          const biome = biomes[((v + 0.5) | 0) * SW + ((u + 0.5) | 0)] as Biome;
          // A little of the per-cell value on the smooth one, so thickets have ragged edges.
          tiles[row] = ground(biome, cl + cs * i + (r - 0.5) * 0.12, r, x, y, s);
        }
      }
    }
  };
  for (let my = 0; my < SH; my++) for (let mx = 0; mx < SW; mx++) paintBlock(mx, my);
}

/** `clump` is smooth (thickets, outcrops, pools), `r` is per cell (the odd tree, the long grass). */
function ground(biome: Biome, clump: number, r: number, x: number, y: number, s: number): Tile {
  switch (biome) {
    case B.Field:
      return r < 0.004 ? T.Tree : r < 0.012 ? T.Bush : r < 0.1 ? T.GrassTall : T.Grass;
    case B.Hedge: {
      // Hedged fields: lines of bush on a loose grid, with gaps where a gate once was.
      const line = x % 34 === 0 || y % 30 === 0;
      if (line && hash01(s + 7, Math.floor(x / 9), Math.floor(y / 9)) > 0.3) return T.Bush;
      return r < 0.006 ? T.Tree : r < 0.16 ? T.GrassTall : T.Grass;
    }
    case B.Wood:
      return clump > 0.8 ? T.Pine : clump > 0.66 || r < 0.05 ? T.Tree : r < 0.4 ? T.GrassTall : T.Grass;
    case B.Foothill:
      return clump > 0.74 ? T.Cliff : r < 0.02 ? T.Pine : clump < 0.3 ? T.Dirt : r < 0.2 ? T.GrassTall : T.Grass;
    case B.Reed:
      // Patches of reed and of moss, not a salt-and-pepper of both: the smooth field decides.
      return clump > 0.78 ? T.Water : clump > 0.47 ? T.GrassTall : T.Moss;
    case B.Marsh:
      return clump > 0.76 ? T.Water : clump < 0.22 ? T.DryBed : r < 0.3 ? T.GrassTall : r < 0.34 ? T.Bush : T.Moss;
    case B.WetWood:
      return clump > 0.68 || r < 0.05 ? T.Tree : r < 0.5 ? T.Moss : T.GrassTall;
    case B.Garden:
      return clump > 0.7 ? T.Bush : r < 0.03 ? T.Rubble : clump > 0.44 ? T.Garden : T.Grass;
    case B.Slag:
      // Rubble is solid: a scatter of it is cover, a carpet of it is a maze.
      return clump > 0.78 ? T.Cliff : r < 0.05 ? T.Rubble : clump < 0.3 ? T.DryBed : T.Dirt;
    case B.Yard:
      // The yards' old hardstandings, in strips. (These were lengths of track, forty cells long and ending
      // in the grass for no reason; the Works' railway is one line now, laid whole, and it goes somewhere.)
      return y % 46 < 2 && hash01(s + 9, Math.floor(x / 40), Math.floor(y / 46)) > 0.5 ? T.Cobble : r < 0.04 ? T.Rubble : clump > 0.6 ? T.Cobble : T.Dirt;
    case B.Hill:
      return clump > 0.72 ? T.Cliff : r < 0.04 ? T.Rubble : clump < 0.35 ? T.Grass : T.Dirt;
    default:
      return T.Grass;
  }
}

// --- the railway -----------------------------------------------------------------------

/** Up the west fence the line runs at this column: the halt's platform rails (chunks.ts station) are at 4 to 6. */
const RAIL_WEST_X = 5;
/** The tree line round the county is this deep; the boundary fence crosses the line at its inner edge. */
const EDGE = 4;

/**
 * The line in cells: one width of sleepers and rails, walkable, with a shoulder of ballast either
 * side and the growth cut back a cell beyond that. Where a road crosses it the road keeps its
 * metal and a sign stands at the crossing; where it crosses water it goes over on a trestle (the
 * track, a deck of planks either side, a rail of fence at the edges). At each end, where it leaves
 * the county through the trees, a fence crosses it: the rails run on under it and out of sight.
 */
/** The widest a bend of the line is drawn (cells from the corner to where the curve begins). */
const RAIL_BEND = 32;

/**
 * The skeleton's line is square to the grid (skeleton/rail.ts): straights and right-angle corners.
 * Keep only the corners, and round each one into a curve (a quadratic from where the bend begins,
 * through the corner's pull, to where it ends) as wide as the straights either side allow. A bend
 * that would swing over the lake is drawn tighter.
 */
function railCurves(pts: readonly [number, number][], sk: Skeleton): [number, number][] {
  const sign = (a: [number, number], b: [number, number]): string => `${Math.sign(b[0] - a[0])},${Math.sign(b[1] - a[1])}`;
  const corners: [number, number][] = [pts[0]];
  for (let i = 1; i + 1 < pts.length; i++) if (sign(pts[i - 1], pts[i]) !== sign(pts[i], pts[i + 1])) corners.push(pts[i]);
  corners.push(pts[pts.length - 1]);
  const len = (a: [number, number], b: [number, number]): number => Math.abs(b[0] - a[0]) + Math.abs(b[1] - a[1]);
  const out: [number, number][] = [corners[0]];
  for (let i = 1; i + 1 < corners.length; i++) {
    const [cx, cy] = corners[i];
    const a = corners[i - 1];
    const b = corners[i + 1];
    const la = len(a, corners[i]);
    const lb = len(corners[i], b);
    const ia = [Math.sign(cx - a[0]), Math.sign(cy - a[1])];
    const ob = [Math.sign(b[0] - cx), Math.sign(b[1] - cy)];
    const curve = (r: number): [number, number][] => {
      const p: [number, number][] = [];
      for (let s = 0; s <= 2 * r; s++) {
        const t = s / (2 * r);
        // B(t) = (1-t)^2 P0 + 2t(1-t) C + t^2 P2, with P0 = C - r*in and P2 = C + r*out.
        const x = cx - (1 - t) * (1 - t) * r * ia[0] + t * t * r * ob[0];
        const y = cy - (1 - t) * (1 - t) * r * ia[1] + t * t * r * ob[1];
        p.push([Math.round(x), Math.round(y)]);
      }
      return p;
    };
    let r = Math.min(RAIL_BEND, Math.floor(la / 2), Math.floor(lb / 2));
    while (r > 2 && curve(r).some(([x, y]) => sk.water[at(x >> 4, y >> 4)] === 2)) r >>= 1;
    out.push(...(r > 0 ? curve(r) : [corners[i]]));
  }
  out.push(corners[corners.length - 1]);
  return out;
}

function layRailway(k: Kit, sk: Skeleton): void {
  if (sk.rail.length < 2) return;
  const pts: [number, number][] = sk.rail.map((c) => {
    const mx = c % SKEL_W;
    const my = Math.floor(c / SKEL_W);
    return [mx === 0 ? RAIL_WEST_X : mx * MACRO + MACRO / 2, my * MACRO + MACRO / 2];
  });
  // Out to the map's own edge at both ends.
  const reach = (p: [number, number]): [number, number] => {
    if (p[1] >= COUNTY_H - MACRO) return [p[0], COUNTY_H - 1];
    if (p[1] < MACRO) return [p[0], 0];
    if (p[0] >= COUNTY_W - MACRO) return [COUNTY_W - 1, p[1]];
    return [0, p[1]];
  };
  pts.unshift(reach(pts[0]));
  pts.push(reach(pts[pts.length - 1]));
  const path = railCurves(pts, sk);
  // The centre line, four-connected, so every sleeper has a neighbour along the way the line runs.
  const line: [number, number][] = [];
  line.push(path[0]);
  for (let n = 0; n + 1 < path.length; n++) {
    const [ax, ay] = path[n];
    const [bx, by] = path[n + 1];
    const steps = Math.max(Math.abs(bx - ax), Math.abs(by - ay));
    for (let s = 1; s <= steps; s++) {
      const x = Math.round(ax + ((bx - ax) * s) / steps);
      const y = Math.round(ay + ((by - ay) * s) / steps);
      const [px, py] = line[line.length - 1];
      // A diagonal step becomes two square ones.
      if (x !== px && y !== py) line.push([x, py]);
      line.push([x, y]);
    }
  }
  const inner = (x: number, y: number): boolean => x >= EDGE && y >= EDGE && x < COUNTY_W - EDGE && y < COUNTY_H - EDGE;
  const centre = new Set(line.map(([x, y]) => y * COUNTY_W + x));
  // Anything small already standing where the line goes gives way to it (a scatter from a patch's dressing).
  // Nothing with a name is ever this near the line: sites keep 96 m off and small places a macro cell.
  for (let n = k.props.length - 1; n >= 0; n--) {
    const p = k.props[n];
    if (!p.key.startsWith("county_")) continue;
    let hit = false;
    for (let oy = -1; oy <= 2 && !hit; oy++) for (let ox = -1; ox <= 2 && !hit; ox++) hit = centre.has((p.cy + oy) * COUNTY_W + p.cx + ox);
    if (hit) k.props.splice(n, 1);
  }
  const GROWTH = new Set<Tile>([Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Bush, Tile.Cliff, Tile.Rubble, Tile.Hedge, Tile.Fence, Tile.Wall]);
  let crossing = false;
  let signs = 0;
  line.forEach(([x, y], i) => {
    const here = k.get(x, y);
    // A level crossing: the road keeps its metal, and a sign stands at the first crossing of each road.
    if (here === Tile.Road || here === Tile.Boardwalk) {
      if (!crossing) {
        const [px, py] = line[Math.max(0, i - 3)];
        for (const [ox, oy] of [[2, 0], [-3, 0], [0, 2], [0, -2]] as const) {
          if (!k.fits(px + ox, py + oy, 2, 1) || centre.has((py + oy) * COUNTY_W + px + ox)) continue;
          k.prop({ key: `rail_crossing_${signs++}`, def: "sign", cx: px + ox, cy: py + oy, label: "A crossing sign", use: [{ do: "read", text: "RAILWAY CROSSING. STOP, LOOK AND LISTEN." }] }, 2, 1);
          break;
        }
      }
      crossing = true;
      return;
    }
    crossing = false;
    if (here === Tile.Rail) return; // the halt's own platform rails
    const wet = here === Tile.Water;
    k.set(x, y, Tile.Track);
    for (let oy = -2; oy <= 2; oy++) {
      for (let ox = -2; ox <= 2; ox++) {
        const cx = x + ox;
        const cy = y + oy;
        if ((ox === 0 && oy === 0) || !inner(cx, cy) || centre.has(cy * COUNTY_W + cx)) continue;
        const t = k.get(cx, cy);
        if (t === Tile.Road || t === Tile.Boardwalk || t === Tile.Track || t === Tile.Rail) continue;
        const near = Math.max(Math.abs(ox), Math.abs(oy)) === 1;
        if (t === Tile.Water) {
          // The trestle: planks either side of the rails, and a rail of fence along its edges.
          if (near || wet) k.set(cx, cy, near ? Tile.Boardwalk : Tile.Fence);
        } else if (near) k.set(cx, cy, Tile.Dirt);
        else if (GROWTH.has(t)) k.set(cx, cy, sk.region[at(cx >> 4, cy >> 4)] === Region.Works ? Tile.Dirt : Tile.Grass);
      }
    }
  });
  // Where it leaves: a fence across the cutting at the inner edge of the trees, the rails running on beyond it.
  for (const end of [line[0], line[line.length - 1]]) {
    const vertical = end[1] === 0 || end[1] === COUNTY_H - 1;
    // `d` counts in from the map's edge: the trees are d 0 to EDGE - 1, and the fence stands on the last of them.
    const cell = (d: number, o: number): [number, number] =>
      vertical ? [end[0] + o, end[1] === 0 ? d : COUNTY_H - 1 - d] : [end[0] === 0 ? d : COUNTY_W - 1 - d, end[1] + o];
    for (let o = -3; o <= 3; o++) k.set(...cell(EDGE - 1, o), Tile.Fence);
    // Beyond the fence the trees stand back from the line, so the rails are seen to go on.
    for (let d = 0; d < EDGE - 1; d++) {
      k.set(...cell(d, 0), Tile.Track);
      k.set(...cell(d, -1), Tile.Dirt);
      k.set(...cell(d, 1), Tile.Dirt);
    }
  }
  // The line is the line: nothing the country builds later stands on it.
  for (const [x, y] of line) k.claim(x - 2, y - 2, 5, 5);
}

// --- links ---------------------------------------------------------------------------

const RING = 4;

/**
 * Where a road crosses into a chunk's box, the stamp has overwritten it. Join the last
 * cell outside to the chunk's nearest gate by a lane that goes ROUND the box, so it can
 * never cut a fence, a wall or a kitchen.
 */
function linkRoad(k: Kit, line: readonly [number, number][], c: Chunk): void {
  const b = c.box;
  const within = (x: number, y: number): boolean => x >= b.cx - 2 && y >= b.cy - 2 && x < b.cx + b.w + 2 && y < b.cy + b.h + 2;
  let was = line.length > 0 && within(line[0][0], line[0][1]);
  for (let i = 1; i < line.length; i++) {
    const now = within(line[i][0], line[i][1]);
    if (now !== was) connect(k, now ? line[i - 1] : line[i], c);
    was = now;
  }
}

function connect(k: Kit, p: readonly [number, number], c: Chunk): void {
  let gate: Gate = c.gates[0];
  let best = Infinity;
  for (const g of c.gates) {
    const d = Math.abs(g[0] - p[0]) + Math.abs(g[1] - p[1]);
    if (d < best) {
      best = d;
      gate = g;
    }
  }
  const ring: Rect = { cx: Math.max(6, c.box.cx - RING), cy: Math.max(6, c.box.cy - RING), w: 0, h: 0 };
  ring.w = Math.min(k.w - 7, c.box.cx + c.box.w + RING - 1) - ring.cx + 1;
  ring.h = Math.min(k.h - 7, c.box.cy + c.box.h + RING - 1) - ring.cy + 1;
  const path = perimeter(ring);
  const nearest = (q: readonly [number, number]): number => {
    let at = 0;
    let d = Infinity;
    path.forEach(([x, y], n) => {
      const dd = Math.abs(x - q[0]) + Math.abs(y - q[1]);
      if (dd < d) {
        d = dd;
        at = n;
      }
    });
    return at;
  };
  const a = nearest(p);
  const g = nearest(gate);
  const lane = (x: number, y: number): void => k.fill(x - 1, y - 1, 3, 3, Tile.Dirt);
  const forward = (g - a + path.length) % path.length;
  const step = forward <= path.length - forward ? 1 : -1;
  for (let n = a; ; n = (n + step + path.length) % path.length) {
    lane(path[n][0], path[n][1]);
    if (n === g) break;
  }
  straight(p, path[a], lane);
  straight(path[g], gate, lane);
}

function perimeter(r: Rect): [number, number][] {
  const out: [number, number][] = [];
  for (let x = r.cx; x < r.cx + r.w; x++) out.push([x, r.cy]);
  for (let y = r.cy + 1; y < r.cy + r.h; y++) out.push([r.cx + r.w - 1, y]);
  for (let x = r.cx + r.w - 2; x >= r.cx; x--) out.push([x, r.cy + r.h - 1]);
  for (let y = r.cy + r.h - 2; y > r.cy; y--) out.push([r.cx, y]);
  return out;
}

function straight(a: readonly [number, number], b: readonly [number, number], lane: (x: number, y: number) => void): void {
  const steps = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
  for (let n = 0; n <= steps; n++) {
    const t = steps === 0 ? 0 : n / steps;
    lane(Math.round(a[0] + (b[0] - a[0]) * t), Math.round(a[1] + (b[1] - a[1]) * t));
  }
}

// --- dressing --------------------------------------------------------------------------

/**
 * The skeleton's small places, dressed with what the art already has. They are shapes to
 * walk past, and named marks (`poi_<n>`) for the side quests and omens that will be written
 * onto them; nothing here talks yet.
 */
function smallPlace(k: Kit, kind: string, x: number, y: number, name: string): void {
  // A rolled place that landed on a road or in water is simply dropped. A place the story NEEDS is not:
  // it keeps its mark (on ground made open for it) and goes undressed, because a path runs through it.
  if (k.isClaimed(x, y) || k.get(x, y) === Tile.Water) {
    if (name.startsWith("poi_")) return;
    kind = "none";
  }
  /**
   * The ground a small place stands on. Not a square: a square of cobble in a field reads as a tile,
   * not as somewhere people have been. This is a blob with a ragged edge and a scuffed apron of dirt
   * around it, which is what a hundred years of feet leave, and it is most of what makes the thing
   * look like a place at all from the road.
   */
  const pad = (w: number, h: number, t: Tile): void => {
    const rx = w / 2;
    const ry = h / 2;
    // Integer bounds. An odd width gives a half-integer radius, and a loop from -4.5 hands `set` a
    // fractional index, which a typed array drops on the floor without a word: every pad painted
    // nothing at all and every test still passed, because no test looks at the ground.
    const jn = Math.ceil(ry) + 2;
    const in_ = Math.ceil(rx) + 2;
    for (let j = -jn; j <= jn; j++) {
      for (let i = -in_; i <= in_; i++) {
        const d = (i * i) / (rx * rx) + (j * j) / (ry * ry);
        const worn = k.get(x + i, y + j);
        if (worn === Tile.Water || worn === Tile.Road) continue;
        // The middle is the surface, the rim is broken, and outside it the grass is worn to dirt.
        if (d <= 0.7) k.set(x + i, y + j, t);
        else if (d <= 1 && k.chance(0.7)) k.set(x + i, y + j, t);
        else if (d <= 1.9 && k.chance(0.45) && worn !== t) k.set(x + i, y + j, Tile.Dirt);
      }
    }
  };
  /**
   * Structure: walls, fences, furrows. Never over a road, over water, or over ground a set chunk owns,
   * because these footprints are now big enough to reach all three, and a fence across the lane is worse
   * than no fence at all.
   */
  const lay = (lx: number, ly: number, w: number, h: number, t: Tile): void => {
    for (let j = ly; j < ly + h; j++) {
      for (let i = lx; i < lx + w; i++) {
        const was = k.get(i, j);
        if (was === Tile.Road || was === Tile.Water || k.isClaimed(i, j)) continue;
        k.set(i, j, t);
      }
    }
  };
  // A place the story needs gets its GROUND here and its THINGS from data/placements, where they can carry
  // a key, a dialogue tree and loot. Only the seed's own rolled places are furnished by this function.
  const rolled = name.startsWith("poi_");
  const put = (def: string, ox: number, oy: number, w = 1, h = 1): void => {
    if (rolled && k.fits(x + ox, y + oy, w, h)) k.prop({ def, cx: x + ox, cy: y + oy }, w, h);
  };
  /**
   * The outskirt. A place is not only the thing in the middle of it: it is the thing, and then the
   * ring of stuff that gathered around the thing because people kept coming back. Without this a
   * roadside place is a prop on a mat, and you walk past it without reading it as anywhere at all.
   */
  const outskirt = (r: number, t: Tile, n: number): void => {
    // Only a rolled place gets an outskirt. An anchor is ground the story has already claimed: the quests
    // set five parcels round the crane cottage and a haversack in the hedge tree, and they need the room.
    if (!rolled) return;
    const blocks = (TILE_FLAGS[t] & F_SOLID) !== 0;
    for (let a = 0; a < n; a++) {
      const ox = k.roll(2 * r + 1) - r;
      const oy = k.roll(2 * r + 1) - r;
      if (Math.abs(ox) + Math.abs(oy) < r - 1) continue;
      const cx = x + ox;
      const cy = y + oy;
      const was = k.get(cx, cy);
      if (was !== Tile.Grass && was !== Tile.Dirt) continue;
      // A bush is solid. Scattered singly it is something to walk round; in a line it is a wall, and a
      // wall drawn by accident out here can shut a story place off the map and force the county to re-roll.
      // So a blocker only ever goes down with clear ground on all four sides: a hedge cannot grow itself.
      if (blocks && (k.solid(cx - 1, cy) || k.solid(cx + 1, cy) || k.solid(cx, cy - 1) || k.solid(cx, cy + 1))) continue;
      k.set(cx, cy, t);
    }
  };
  switch (kind) {
    case "none":
      // A bare spot the story needs (a lamp post will stand here, not a shrine): a mark and nothing else.
      break;
    case "well":
      // A well is where the village that is not here any more used to come. Cobble worn by buckets,
      // a trough, and a low wall on the windward side that somebody built and nobody finished.
      pad(9, 9, Tile.Cobble);
      k.set(x, y, Tile.Water);
      lay(x - 4, y - 1, 1, 4, Tile.Wall);
      put("well_head", 0, -1);
      put("barrel", 2, 2, 2, 2);
      outskirt(6, Tile.Bush, 14);
      break;
    case "shrine":
    case "statue":
      pad(9, 7, Tile.Cobble);
      put("pillar", 0, 0);
      put("pillar", -3, 1);
      put("pillar", 3, 1);
      lay(x - 1, y + 2, 3, 1, Tile.Rubble);
      outskirt(6, Tile.Bush, 12);
      break;
    case "scarecrow":
      // Not a post in a field: a field, with the post in it. The furrows are what you see first.
      pad(13, 9, Tile.Dirt);
      for (let r = -3; r <= 3; r += 2) lay(x - 6, y + r, 13, 1, Tile.Garden);
      put("scarecrow", 0, 0);
      outskirt(8, Tile.GrassTall, 18);
      break;
    case "signpost":
      // The commonest thing on any road in the county, and until now the one that drew nothing at all.
      // A fingerpost, the passing place worn into the verge beside it, and a bench of sorts.
      pad(9, 7, Tile.Dirt);
      // Two courses, not one. A single cell of setts is seen edge on and reads as a row of rungs.
      lay(x - 3, y + 2, 7, 2, Tile.Cobble);
      put("signpost", 0, 0);
      put("crate", -3, 1, 2, 2);
      outskirt(6, Tile.Bush, 12);
      break;
    case "signal":
      pad(7, 7, Tile.Cobble);
      lay(x - 3, y + 3, 7, 1, Tile.Rail);
      put("pillar", 0, 0);
      put("signpost", 2, 1);
      outskirt(6, Tile.Bush, 10);
      break;
    case "stones":
      pad(13, 13, Tile.Dirt);
      for (const [ox, oy] of [[-4, -4], [4, -4], [-5, 1], [5, 1], [0, 5], [-2, -5], [3, 4]]) put("pillar", ox, oy);
      outskirt(9, Tile.GrassTall, 22);
      break;
    case "cottage":
    case "hut":
      // A roofless house still has its garden wall, its yard, and the black ring where the fire was.
      pad(16, 13, Tile.Dirt);
      lay(x + 1, y - 1, 3, 2, Tile.Rubble);
      lay(x - 4, y - 4, 8, 1, Tile.HouseWall);
      lay(x - 4, y - 4, 1, 5, Tile.HouseWall);
      lay(x + 3, y - 4, 1, 3, Tile.HouseWall);
      if (rolled) {
        lay(x - 7, y + 4, 14, 1, Tile.Fence);
        lay(x - 7, y - 2, 1, 7, Tile.Fence);
        // The gate. A garden wall with no way through is a pen, and the mark stands on the far side of it.
        lay(x - 1, y + 4, 3, 1, Tile.Dirt);
      }
      lay(x - 6, y + 1, 5, 2, Tile.Garden);
      put("campfire_cold", 5, 1);
      put("crate", -2, 3, 2, 2);
      outskirt(9, Tile.Bush, 16);
      break;
    case "greenhouse":
      pad(15, 11, Tile.Garden);
      lay(x - 5, y - 4, 11, 1, Tile.Wall);
      lay(x - 5, y - 4, 1, 6, Tile.Wall);
      lay(x + 5, y - 4, 1, 4, Tile.Glass);
      for (let r = -2; r <= 3; r += 2) lay(x - 6, y + r, 12, 1, Tile.Dirt);
      put("barrel", -4, 4, 2, 2);
      outskirt(8, Tile.Bush, 14);
      break;
    case "pump":
    case "pipe_end":
      pad(11, 9, Tile.Dirt);
      k.set(x + 2, y - 2, Tile.Rubble);
      lay(x - 1, y - 2, 3, 2, Tile.Wall);
      lay(x - 5, y + 2, 11, 2, Tile.Cobble);
      put("barrel", 4, -1, 2, 2);
      put("crate", -4, -1, 2, 2);
      outskirt(7, Tile.Rubble, 12);
      break;
    case "jetty":
    case "boat":
      pad(5, 13, Tile.Cobble);
      lay(x - 1, y - 6, 3, 4, Tile.FloorWood);
      put("crate", 1, 2, 2, 2);
      put("barrel", -2, 4, 2, 2);
      outskirt(7, Tile.GrassTall, 14);
      break;
    case "wagon":
      lay(x - 8, y, 17, 1, Tile.Track);
      if (rolled) lay(x - 8, y - 1, 17, 1, Tile.Rail);
      pad(9, 5, Tile.Dirt);
      put("minecart", -1, -2, 2, 2);
      put("crate", 4, 2, 2, 2);
      outskirt(7, Tile.Bush, 12);
      break;
    case "cart":
    case "camp":
      // Somebody stopped here for a night. The ring of stones is cold and the cart never went on.
      pad(11, 9, Tile.Dirt);
      put("road_cart", 3, -2, 2, 2);
      put("campfire_cold", 0, 0);
      put("crate", -3, -1, 2, 2);
      put("barrel", -1, 3, 2, 2);
      outskirt(7, Tile.Bush, 14);
      break;
    case "hollow_tree":
      // Thicket first, then the one tree you can get inside. The wood has to be thick to be worth a gap.
      outskirt(8, Tile.Tree, 26);
      // An anchor keeps the old small tree: the hedge tree has a haversack hung in it, and a thicket
      // seven deep leaves nowhere to hang it.
      if (rolled) lay(x - 3, y - 3, 7, 6, Tile.Tree);
      else lay(x - 2, y - 2, 5, 4, Tile.Tree);
      lay(x - 1, y - 1, 3, 3, Tile.Dirt);
      lay(x - 1, y + 1, 3, 3, Tile.Dirt);
      break;
    default:
      pad(7, 7, Tile.Dirt);
      outskirt(6, Tile.Bush, 10);
  }
  // Whatever was drawn, the place can be stood at: the mark's own cell and its neighbours are open ground.
  // The path out matters as much as the standing room. These footprints are wide enough to close a ring
  // round their own mark -- a thicket, a fence and a wall meeting by chance -- and a mark nobody can walk
  // to fails the solver and throws the whole county away. So a three-wide way is opened south from the
  // mark, past the last of the dressing, every time. It costs a few cells of the shape and it means the
  // shape can never be a trap.
  for (let oy = 2; oy <= 14; oy++) {
    for (let ox = -1; ox <= 1; ox++) {
      const cy = y + oy;
      if (k.get(x + ox, cy) === Tile.Water || k.isClaimed(x + ox, cy)) continue;
      if (k.solid(x + ox, cy)) k.set(x + ox, cy, Tile.Dirt);
    }
  }
  // A place the story needs also gets room: a note, a lamp post, a scarecrow will be set down beside the mark,
  // and the text says "at the well", not "somewhere in the thicket near the well".
  if (!rolled) clearing(k, x, y + 3);
  k.mark(name, x, y + 3, 1);
}

/** Open ground round a named spot: trees, bushes, outcrops and rubble give way; water and buildings do not. */
function clearing(k: Kit, x: number, y: number): void {
  for (let oy = -5; oy <= 5; oy++) {
    for (let ox = -7; ox <= 7; ox++) {
      const t = k.get(x + ox, y + oy);
      if (t === Tile.Tree || t === Tile.Pine || t === Tile.DeadTree || t === Tile.Bush || t === Tile.Cliff || t === Tile.Rubble) k.set(x + ox, y + oy, Tile.Grass);
    }
  }
}

function scatter(k: Kit, sk: Skeleton): void {
  const open = (x: number, y: number): boolean => {
    const t = k.get(x, y);
    return !k.isClaimed(x, y) && !k.solid(x, y) && t !== Tile.Water && t !== Tile.Road;
  };
  for (let n = 0; n < 700; n++) {
    const x = k.int(10, COUNTY_W - 11);
    const y = k.int(10, COUNTY_H - 11);
    if (!open(x, y)) continue;
    const region = sk.region[at(x >> 4, y >> 4)] as Region;
    const herbs = region === Region.Lowfields ? FIELD_HERBS : region === Region.Waters ? WATER_HERBS : WORKS_HERBS;
    k.prop({ def: "herb", cx: x, cy: y, loot: [{ item: k.pick(herbs), qty: 1 }] }, 1, 1);
  }
  for (let n = 0; n < 260; n++) {
    const x = k.int(10, COUNTY_W - 11);
    const y = k.int(10, COUNTY_H - 11);
    if (open(x, y)) k.prop({ def: "rock", cx: x, cy: y }, 1, 1);
  }
}

/**
 * Dead lamp runs, and the relay box at the head of each. The skeleton decides which stretches of
 * road still work (lamps fail with distance from the town, and almost entirely in the Works); this
 * finds the longest stretches that do NOT, and furnishes them: a box she will walk past many times
 * and can do nothing with, and the lamps it feeds, standing dark at the usual spacing.
 *
 * Sparking a box switches its own run on and sets `lamps_<n>`, which is a flag the threat field can
 * read later: a lit road keeps its discount after dark. This is the largest single thing any verb
 * gives back to the county, and it is the verb that gives it, not loot.
 */
function relayRuns(k: Kit, lines: readonly (readonly [number, number])[][], lit: readonly boolean[][]): void {
  const SPACING = 26;
  const LEAST = SPACING * 4;
  const RUNS = 3;
  // Every unlit stretch of every road, longest first; ties on where it starts, so the seed decides nothing here.
  const runs: { line: number; from: number; to: number }[] = [];
  lines.forEach((line, n) => {
    const on = lit[n];
    if (on.length !== line.length) return;
    let from = -1;
    for (let i = 0; i <= line.length; i++) {
      const dark = i < line.length && !on[i];
      if (dark && from < 0) from = i;
      if (!dark && from >= 0) {
        if (i - from >= LEAST) runs.push({ line: n, from, to: i - 1 });
        from = -1;
      }
    }
  });
  runs.sort((a, b) => b.to - b.from - (a.to - a.from) || a.line - b.line || a.from - b.from);

  // The box goes down FIRST, and it is tried at several points along the head of the run: the lit end
  // of a dark stretch is usually the edge of a town or a yard, where the ground is already spoken for.
  // Only once a box stands does its run get its lamps, so a county never carries a promise nothing keeps.
  let placed = 0;
  for (const run of runs) {
    if (placed >= RUNS) break;
    const line = lines[run.line];
    const beside = (i: number, def: string, key: string): boolean => {
      if (i < 0 || i >= line.length) return false;
      const [x, y] = line[i];
      for (const dy of [-4, 4, -5, 5, -6, 6]) {
        if (k.solid(x, y + dy) || k.isClaimed(x, y + dy)) continue;
        const t = k.get(x, y + dy);
        if (t === Tile.Water || t === Tile.Road) continue;
        k.prop({ key, def, cx: x, cy: y + dy }, 1, 1);
        return true;
      }
      return false;
    };
    const boxKey = `relay_${placed}`;
    let at = -1;
    for (let i = run.from; i <= Math.min(run.to - SPACING, run.from + SPACING) && at < 0; i += 4) {
      if (beside(i, "relay_box", boxKey)) at = i;
    }
    if (at < 0) continue;
    const lamps: string[] = [];
    for (let i = at + SPACING; i < run.to - 2; i += SPACING) {
      const key = `lamp_run_${placed}_${lamps.length}`;
      if (beside(i, "lamp_run", key)) lamps.push(key);
    }
    const box = k.props.find((p) => p.key === boxKey)!;
    if (lamps.length < 3) {
      // Not a run, just a gap. Take the box back out rather than leave a switch for three lamps.
      for (const key of [boxKey, ...lamps]) {
        const idx = k.props.findIndex((p) => p.key === key);
        if (idx >= 0) k.props.splice(idx, 1);
      }
      continue;
    }
    const use: ActionList = lamps.map((key) => ({ do: "switch", prop: key, on: true }));
    use.push({ do: "flag", flag: `lamps_${placed}`, value: 1 });
    use.push({ do: "toast", text: "It takes, and the next one takes, and it goes away down the road ahead of you." });
    box.use = use;
    box.label = "A relay box";
    placed++;
  }
}
