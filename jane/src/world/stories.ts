// Stories at generated places. The open country's hamlets, farms, cottages, inns, woodcutters'
// clearings, camps and ruins are rolled per seed (world/country.ts); quests are fixed data. A story
// (data/stories/*.json) asks for "one farm in the Lowfields, quiet ground, in sight of a road", and
// this gives it exactly one, the same one every time the seed is built, never one another story
// has. Its rows in data/placements then say `at: { place: <story>, slot: "yard" }` and land in that
// farm's yard; its words say {place:<story>} and read "Hollins Farm" (world/names.ts), which is what
// the board at the farm gate says.
//
// A place off the road (a camp, a ruin, a clearing in the wood) is found by a footpath: when a story
// claims one that cannot be seen from a road, a trodden path is laid from the nearest road to it,
// with a fingerpost where it leaves the road. That is the farm track and the quarry track again
// (QUEST-TREE.md): a person walking the road sees where to turn off.
//
// A story that finds no place on a seed is skipped, with the reason written on the blueprint
// (`bp.stories`), and nothing of it is placed: no giver, so no quest that cannot be done. That should
// be rare; test/stories.test.ts counts it.

import { F_SOLID, Tile, TILE_FLAGS } from "@/sim/grid";
import { hashString } from "@/sim/rng";
import type { StoryPlace } from "@/world/blueprint";
import type { Place } from "@/world/country";
import type { Kit } from "@/world/kit";
import { PLACEMENTS, type PlacementRow } from "@/world/placements";
import { MACRO, Region, type Skeleton } from "@/world/skeleton";
import { boardText, NAMED_KINDS, nthName, STORIES, storiesOfKind, storyName, type StoryRow } from "@/world/names";

const REGION: Record<string, Region> = { lowfields: Region.Lowfields, waters: Region.Waters, works: Region.Works };

/** The opening is the letter's: nothing of a story's this close to the platform or Julie's gate (cells, centre to centre). */
const KEEP_OFF: Record<string, number> = { station: 170, julie_house: 100 };
/** Two stories' places stand at least this far apart, unless one is the other's `near`: one every now and then. */
const SPREAD = 60;
/**
 * Every story place is seen from a road: some cell of its footprint within this many half-screens
 * (24 across, 13 down) of a made road. The quest audit's "a small place's edge is on screen from the
 * road" is 1.5; this keeps a margin.
 */
const SEEN = 1.25;
/** Kinds that stand off the road by nature, and are reached by a footpath laid for the story. */
const PATHED: ReadonlySet<string> = new Set(["camp", "ruin", "woodcutter"]);
/** The longest footpath a story lays, in cells from the road to the place's edge. */
const PATH_MAX = 160;

export type Claims = {
  claims: Map<string, Place>;
  skipped: Map<string, string>;
  /** Footpaths laid to places off the road, by story: the line from the road to the place. */
  paths: Map<string, [number, number][]>;
};

/** The slots a story's rows use at its place: what a place must offer before the story can have it. */
function slotsWanted(id: string, rows: readonly PlacementRow[]): string[] {
  return [...new Set(rows.filter((r) => r.at.place === id && r.at.slot).map((r) => r.at.slot!))];
}

/** Does the place offer this slot: a kept cell, a named prop, or enough of its people ("folk", "folk_2"). */
function offers(p: Place, slot: string): boolean {
  const folk = /^folk(?:_(\d))?$/.exec(slot);
  if (folk) return p.folk.length >= Number(folk[1] ?? 1);
  if (slot === "hostiles") return p.hostiles.length > 0;
  return slot in p.slots || slot in p.things;
}

type Reach = {
  /** Half-screens from the footprint's edge to the nearest made road cell. */
  screens: number;
  /** The nearest road cell (straight line from the footprint's edge), and how far, in cells. */
  road: [number, number] | null;
  cells: number;
};

/** Road cells, binned 32 x 32, so "the nearest road" is a look in a few bins and not a sweep of a hundred thousand cells. */
type RoadBins = { cols: number; bins: Map<number, number[]> };
const BIN = 32;

function roadBins(k: Kit): RoadBins {
  const cols = Math.ceil(k.w / BIN);
  const bins = new Map<number, number[]>();
  const tiles = k.tiles;
  const road = Tile.Road;
  for (let i = 0; i < tiles.length; i++) {
    if (tiles[i] !== road) continue;
    const x = i % k.w;
    const y = (i - x) / k.w;
    const b = Math.floor(y / BIN) * cols + Math.floor(x / BIN);
    let list = bins.get(b);
    if (!list) bins.set(b, (list = []));
    list.push(i);
  }
  return { cols, bins };
}

/** How far a place is from the roads, both ways the audit and the footpath need it. */
function reach(k: Kit, rb: RoadBins, p: Place): Reach {
  const { cx, cy, w, h } = p.box;
  const R = PATH_MAX;
  let screens = Infinity;
  let cells = Infinity;
  let road: [number, number] | null = null;
  const bx0 = Math.max(0, Math.floor((cx - R) / BIN));
  const bx1 = Math.floor((cx + w + R) / BIN);
  const by0 = Math.max(0, Math.floor((cy - R) / BIN));
  const by1 = Math.floor((cy + h + R) / BIN);
  for (let by = by0; by <= by1; by++) {
    for (let bx = bx0; bx <= bx1 && bx < rb.cols; bx++) {
      for (const i of rb.bins.get(by * rb.cols + bx) ?? []) {
        const x = i % k.w;
        const y = (i - x) / k.w;
        const dx = Math.max(0, cx - x, x - (cx + w - 1));
        const dy = Math.max(0, cy - y, y - (cy + h - 1));
        const s = Math.max(dx / 24, dy / 13);
        if (s < screens) screens = s;
        const d = Math.hypot(dx, dy);
        if (d < cells) {
          cells = d;
          road = [x, y];
        }
      }
    }
  }
  return { screens, road, cells };
}

const centreOf = (p: Place): [number, number] => [p.box.cx + p.box.w / 2, p.box.cy + p.box.h / 2];

/**
 * The order stories choose in: the scarcest kind first (a seed has four farms and forty clearings,
 * so the farms are spoken for before a cottage story can crowd one out), each chain together, its
 * first place before the places that follow it. Ties keep the order of the data.
 */
function claimOrder(stories: readonly StoryRow[], places: readonly Place[]): StoryRow[] {
  const byId = new Map(stories.map((s) => [s.id, s]));
  const count = new Map<string, number>();
  for (const p of places) count.set(p.kind, (count.get(p.kind) ?? 0) + 1);
  const rootOf = (s: StoryRow): { root: StoryRow; depth: number } => {
    let depth = 0;
    let r = s;
    while (r.near && byId.has(r.near.story) && depth < 8) {
      r = byId.get(r.near.story)!;
      depth++;
    }
    return { root: r, depth };
  };
  const index = new Map(stories.map((s, n) => [s.id, n]));
  // A chain is as scarce as the scarcest place in it: a woodcutter who sends her to a cottage chooses with the cottages.
  const scarce = new Map<string, number>();
  for (const s of stories) {
    const { root } = rootOf(s);
    const n = count.get(s.kind) ?? 0;
    scarce.set(root.id, Math.min(scarce.get(root.id) ?? Infinity, n));
  }
  const key = (s: StoryRow): [number, number, number] => {
    const { root, depth } = rootOf(s);
    return [scarce.get(root.id) ?? 0, index.get(root.id)!, depth];
  };
  return [...stories].sort((a, b) => {
    const ka = key(a);
    const kb = key(b);
    return ka[0] - kb[0] || ka[1] - kb[1] || ka[2] - kb[2];
  });
}

/**
 * Give each story its place. In story order, so a chain's second half (which names its first in
 * `near`) always comes after it. Among the places that fit, the seed chooses by a hash, so the stories
 * scatter over the region instead of piling up at whatever was built first.
 */
export function claimPlaces(seed: number, k: Kit, sk: Skeleton, places: readonly Place[], rows: readonly PlacementRow[] = PLACEMENTS, stories: readonly StoryRow[] = STORIES): Claims {
  const claims = new Map<string, Place>();
  const skipped = new Map<string, string>();
  const paths = new Map<string, [number, number][]>();
  const taken = new Set<Place>();
  const defOf = new Map(k.units.map((u) => [u.key, u.def]));
  const reached = new Map<Place, Reach>();
  let bins: RoadBins | null = null;
  const sites = sk.sites.map((s) => ({ id: s.id, x: s.mx * MACRO + MACRO / 2, y: s.my * MACRO + MACRO / 2 }));
  const follows = (a: string, b: string): boolean => stories.find((s) => s.id === a)?.near?.story === b;
  /** The places a story could have now, following `near` if it follows another; `out` hears why each other was turned down. */
  const fitting = (story: StoryRow, near: Place | undefined, also: Place | null, out: (why: string) => void = () => {}): Place[] => {
    const region = REGION[story.region ?? "lowfields"];
    const [tLo, tHi] = story.threat ?? [0, 2];
    const want = [...slotsWanted(story.id, rows), "board"];
    const kinds = story.hostile?.split("|");
    const fits: Place[] = [];
    for (const p of places) {
      if (p.kind !== story.kind || p.region !== region) continue;
      if (taken.has(p) || p === also) {
        out("another story's");
        continue;
      }
      if (p.threat < tLo || p.threat > tHi) {
        out("threat");
        continue;
      }
      if (story.first && (p.first < story.first[0] || p.first > story.first[1])) {
        out("not by the first walk");
        continue;
      }
      const missing = want.find((s) => !offers(p, s));
      if (missing) {
        out(`no ${missing}`);
        continue;
      }
      if (kinds && p.hostiles.filter((key) => kinds.includes(defOf.get(key) ?? "")).length < (story.count ?? 1)) {
        out(`not ${story.count ?? 1} ${story.hostile}`);
        continue;
      }
      const [x, y] = centreOf(p);
      if (sites.some((s) => KEEP_OFF[s.id] !== undefined && Math.hypot(s.x - x, s.y - y) < KEEP_OFF[s.id])) {
        out("too near the opening");
        continue;
      }
      if (near) {
        const [nx, ny] = centreOf(near);
        if (Math.hypot(nx - x, ny - y) > story.near!.max) {
          out(`not within ${story.near!.max} of ${story.near!.story}`);
          continue;
        }
      }
      // Spread: not on top of another story, unless one follows the other.
      let crowded = false;
      for (const [id, q] of claims) {
        if (follows(story.id, id) || follows(id, story.id)) continue;
        const [qx, qy] = centreOf(q);
        if (Math.hypot(qx - x, qy - y) < SPREAD) crowded = true;
      }
      if (crowded) {
        out("crowds another story");
        continue;
      }
      let r = reached.get(p);
      if (!r) reached.set(p, (r = reach(k, (bins ??= roadBins(k)), p)));
      if (r.screens > SEEN && !(PATHED.has(p.kind) && r.road && r.cells <= PATH_MAX)) {
        out("out of sight of a road");
        continue;
      }
      fits.push(p);
    }
    return fits;
  };
  for (const story of claimOrder(stories, places)) {
    const near = story.near ? claims.get(story.near.story) : undefined;
    if (story.near && !near) {
      skipped.set(story.id, `the story it follows (${story.near.story}) has no place`);
      continue;
    }
    // Why each place of the kind was turned down, counted, for the reason on the blueprint.
    const no: Record<string, number> = {};
    let fits = fitting(story, near, null, (why) => void (no[why] = (no[why] ?? 0) + 1));
    // A chain's first place is only worth having if the places it sends her on to can be found near it.
    const followers = stories.filter((s) => s.near?.story === story.id);
    if (followers.length > 0 && fits.length > 0) {
      const ok = fits.filter((p) => followers.every((f) => fitting(f, p, p).length > 0));
      if (ok.length === 0) no[`nothing near for ${followers.map((f) => f.id).join(" and ")}`] = fits.length;
      fits = ok;
    }
    if (fits.length === 0) {
      const reasons = Object.entries(no).map(([why, n]) => `${n} ${why}`);
      skipped.set(story.id, `no ${story.kind} fits: ${reasons.length > 0 ? reasons.join(", ") : `none in the ${story.region ?? "lowfields"}`}`);
      continue;
    }
    const score = (p: Place): number => hashString(`${seed}:${story.id}:${p.n}`);
    fits.sort((a, b) => score(a) - score(b));
    const p = fits[0];
    claims.set(story.id, p);
    taken.add(p);
    const r = reached.get(p)!;
    if (r.screens > SEEN && r.road) paths.set(story.id, layPath(k, p, r.road, storyName(seed, story.id) ?? ""));
  }
  return { claims, skipped, paths };
}

/** Ground a footpath may be trodden over: grass, growth and scrub. Never water, a fence, a wall or a field in crops. */
const TREADABLE = new Uint8Array(64);
for (const t of [Tile.Grass, Tile.GrassTall, Tile.Bush, Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Moss, Tile.DryBed, Tile.Sand, Tile.Rubble]) TREADABLE[t] = 1;

/**
 * A trodden path, two cells wide, from the road straight to the nearest edge of the place, and a
 * fingerpost where it leaves the road saying where it goes. Returns the path's centre line.
 */
function layPath(k: Kit, p: Place, road: [number, number], name: string): [number, number][] {
  const { cx, cy, w, h } = p.box;
  const tx = Math.max(cx, Math.min(cx + w - 1, road[0]));
  const ty = Math.max(cy, Math.min(cy + h - 1, road[1]));
  const n = Math.max(Math.abs(tx - road[0]), Math.abs(ty - road[1]));
  const line: [number, number][] = [];
  for (let s = 0; s <= n; s++) {
    const x = Math.round(road[0] + ((tx - road[0]) * s) / Math.max(1, n));
    const y = Math.round(road[1] + ((ty - road[1]) * s) / Math.max(1, n));
    line.push([x, y]);
    for (let j = 0; j < 2; j++) {
      for (let i = 0; i < 2; i++) {
        if (TREADABLE[k.get(x + i, y + j)]) k.set(x + i, y + j, Tile.Dirt);
      }
    }
  }
  // The fingerpost: a few cells along the path from the road, to one side of it, on open ground.
  const at = line[Math.min(line.length - 1, 5)];
  const metres = Math.max(50, Math.round((n * 1.1) / 50) * 50);
  for (const [ox, oy] of [[2, 0], [-3, 0], [2, 1], [-3, 1], [0, 2], [0, -2], [3, 2], [-4, 2]] as const) {
    const x = at[0] + ox;
    const y = at[1] + oy;
    if (!k.fits(x, y, 2, 1) || (TILE_FLAGS[k.get(x, y + 1)] & F_SOLID) !== 0) continue;
    k.prop({ key: `story_post_${p.n}`, def: "fingerpost", cx: x, cy: y, label: "A fingerpost", use: [{ do: "read", text: `FOOTPATH. ${name.toUpperCase()}, ${metres} m.` }] }, 2, 1);
    break;
  }
  return line;
}

/**
 * The boards. Every hamlet, farm, inn, cottage and clearing gets its name at the edge that faces the
 * road; a camp or a ruin only when a story has claimed it. A claimed place's name is its story's (the
 * text already says it); the rest take the seed's list in build order after the stories' share.
 */
export function nameBoards(seed: number, k: Kit, places: readonly Place[], claims: ReadonlyMap<string, Place>): void {
  const storyOf = new Map<Place, string>();
  for (const [id, p] of claims) storyOf.set(p, id);
  const next = new Map<string, number>();
  for (const p of places) {
    const story = storyOf.get(p);
    if (!story && !NAMED_KINDS.has(p.kind)) continue;
    const b = p.slots.board;
    if (!b) continue;
    let name: string;
    if (story) name = storyName(seed, story) ?? "";
    else {
      const n = next.get(p.kind) ?? storiesOfKind(p.kind);
      next.set(p.kind, n + 1);
      name = nthName(seed, p.kind, n);
    }
    if (!name) continue;
    k.prop({ key: `board_${p.n}`, def: "name_board", cx: b[0], cy: b[1], label: name, use: [{ do: "read", text: boardText(p.kind, name, hashString(`${seed}:${p.n}`)) }] }, 2, 1);
  }
}

/** What the blueprint remembers of the claims, for the audit and the tests. */
export function storyRecord(seed: number, c: Claims): Record<string, StoryPlace> {
  const out: Record<string, StoryPlace> = {};
  for (const s of STORIES) {
    const p = c.claims.get(s.id);
    if (p) out[s.id] = { kind: p.kind, name: storyName(seed, s.id) ?? "", box: { ...p.box }, path: c.paths.get(s.id) };
    else out[s.id] = { skipped: c.skipped.get(s.id) ?? "not tried" };
  }
  return out;
}
