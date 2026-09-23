// Placements: how content gets INTO the generated county without knowing where
// anything is. A quest needs "six pumpkins in the Top Field", "a notice board on the
// town square", "the scarecrow nearest the farm, with something in its pocket". Every
// seed puts the Top Field, the square and the farm somewhere else, so content says
// WHERE BY NAME and the county builder works out the cells.
//
// Rows live in data/placements/*.json (one file per region or quest line). Each row:
//
//   key     what the story calls it. One thing: the key itself. `count` > 1: key_1, key_2...
//   at      where, by name (exactly one of):
//             mark   a named mark (a chunk's: town_square, farm_gate, yard_gate...)
//             site   a story site: somewhere open in or beside its chunk
//             area   a named patch from areas.json: somewhere open inside its radius
//             poi    a KIND of small place from pois.json, with `near` (a site id): the
//                    nearest one of that kind. If this seed has none, the nearest small
//                    place of any kind BECOMES one, so the quest exists on every seed.
//             anchor a small place the skeleton guarantees BY NAME (data/anchors.json)
//             slot   an exact cell a set chunk offers (a door's place in a wall). The
//                    thing goes exactly there, solid ground or not.
//             place  a generated place a story claimed (world/stories.ts, data/stories), with
//                    `slot` naming what in it: a cell it kept open ("yard", "garden", "green"),
//                    one of its own props ("house", "well", "barn", "chest": for `edit`), or
//                    one of its people ("folk", "folk_2": the row's unit takes her place and
//                    her round). A story that found no place on a seed places nothing, and
//                    promises nothing: these rows are not in the contract.
//           `within`: how far from the spot it may land, in cells.
//   unit / prop / rect / mark   what to put there (any combination; they share the spot)
//   edit    instead of placing: change a prop a chunk already placed (give the car a glovebox)
//
// Everything a row names is added to the county's contract, so a seed that cannot place
// it fails validation and is re-rolled like any other broken county. Nothing is skipped
// quietly: that is how the last attempt at this game ended up with 61 quests nobody
// could hand in.

import type { Facing } from "@/sim/state";
import type { PropSpawn, Rect, UnitSpawn } from "@/world/blueprint";
import type { Chunk } from "@/world/chunks";
import type { Place } from "@/world/country";
import type { Kit } from "@/world/kit";
import { MACRO, type Skeleton } from "@/world/skeleton";
import { Tile } from "@/sim/grid";

export type PlacementRow = {
  key: string;
  count?: number;
  /** With `count` in an area: stand them about the patch, each somewhere of its own, not shoulder to shoulder at its centre. */
  spread?: boolean;
  at: {
    mark?: string;
    site?: string;
    area?: string;
    poi?: string;
    near?: string;
    anchor?: string;
    slot?: string;
    place?: string;
    within?: number;
    /**
     * At a story's place, by where in it rather than by a kept cell: cells from the place's top-left
     * (a tale dresses its place the way a set is dressed: the chair here, the drag marks there). The
     * thing goes on the nearest open ground within `within` cells (default 3); growth and rubble under
     * it are cleared, water and walls are not.
     */
    dx?: number;
    dy?: number;
    /** At a story's place, on exactly the cell of a thing an earlier row put there (by key), moved by dx, dy: the key under the stone. */
    on?: string;
    /** With dx, dy: the open ground to find, [w, h, ox, oy], and where in it this row's thing goes. */
    room?: [number, number, number, number];
  };
  /**
   * The key of a prop a chunk placed, and the fields to set on it. Nothing new is placed. A `def` may be
   * changed only for one of the same footprint (a tale boards up a roofless cottage it has claimed), and
   * a `key` given so that a tale's words can name the thing (switch the house, show the boards).
   */
  edit?: Partial<Omit<PropSpawn, "cx" | "cy">>;
  unit?: { def: string; phase?: number; facing?: Facing };
  prop?: Omit<PropSpawn, "key" | "cx" | "cy">;
  rect?: { name: string; w: number; h: number };
  mark?: string;
};

const FILES = import.meta.glob("../data/placements/*.json", { eager: true, import: "default" }) as Record<string, PlacementRow[]>;

/** Every placement row, in file then row order. Order is part of the seed's meaning: it decides who gets a contested spot. */
export const PLACEMENTS: PlacementRow[] = Object.keys(FILES)
  .sort()
  .flatMap((path) => FILES[path]);

/** Names the rows promise, for the zone contract. */
export function placementContract(rows: readonly PlacementRow[] = PLACEMENTS): { units: string[]; props: string[]; marks: string[]; rects: string[] } {
  const out = { units: [] as string[], props: [] as string[], marks: [] as string[], rects: [] as string[] };
  for (const row of rows) {
    if (row.edit) continue; // the chunk already promised that prop
    if (row.at.place) continue; // a story's: there when the seed found it a place, and only then
    for (const key of keysOf(row)) {
      if (row.unit) out.units.push(key);
      if (row.prop) out.props.push(key);
    }
    if (row.mark) out.marks.push(row.mark);
    if (row.rect) out.rects.push(row.rect.name);
  }
  return out;
}

function keysOf(row: PlacementRow): string[] {
  const n = row.count ?? 1;
  return n === 1 ? [row.key] : Array.from({ length: n }, (_, i) => `${row.key}_${i + 1}`);
}

export type PoiSpot = { x: number; y: number; kind: string; anchor?: string };

/**
 * Step one, before the small places are dressed: decide which small place each `poi`
 * row gets, and turn it into the kind the row needs if the seed did not roll one.
 * Returns row key -> index into `pois`. Mutates `pois[n].kind`.
 */
export function claimPois(sk: Skeleton, pois: PoiSpot[], rows: readonly PlacementRow[] = PLACEMENTS): Map<string, number> {
  const taken = new Map<number, string>(); // poi index -> the kind it was claimed as
  const out = new Map<string, number>();
  for (const row of rows) {
    const kind = row.at.poi;
    if (!kind) continue;
    const near = sk.sites.find((s) => s.id === row.at.near) ?? sk.sites.find((s) => s.id === "town");
    if (!near) continue;
    const nx = near.mx * MACRO + MACRO / 2;
    const ny = near.my * MACRO + MACRO / 2;
    let best = -1;
    let bestD = Infinity;
    // A place already claimed as this kind may be shared (two quests about the same scarecrow).
    // Otherwise prefer a free place of the right kind; failing that, any free place, which is re-dressed.
    for (const wantKind of [true, false]) {
      pois.forEach((p, n) => {
        const claimedAs = taken.get(n);
        if (claimedAs !== undefined && claimedAs !== kind) return;
        if (wantKind && p.kind !== kind) return;
        const d = (p.x - nx) * (p.x - nx) + (p.y - ny) * (p.y - ny);
        if (d < bestD) {
          bestD = d;
          best = n;
        }
      });
      if (best >= 0) break;
    }
    if (best < 0) continue; // no small places at all: the contract check will fail this county
    pois[best].kind = kind;
    taken.set(best, kind);
    out.set(row.key, best);
  }
  return out;
}

export type PlaceCtx = {
  k: Kit;
  /** Exact cells offered by dressed areas (world/areas.ts), beside the chunks' own. */
  areaSlots: Readonly<Record<string, [number, number]>>;
  sk: Skeleton;
  chunks: readonly Chunk[];
  pois: readonly PoiSpot[];
  claimed: ReadonlyMap<string, number>;
  /** Footprint of each prop def, in cells. */
  sizes: Readonly<Record<string, { w: number; h: number }>>;
  /** Threat of the ground under a cell, for a placed creature's phase. */
  threatAt: (cx: number, cy: number) => number;
  /** Story id -> the generated place it claimed (world/stories.ts). Filled before the "places" stage. */
  claims?: ReadonlyMap<string, Place>;
  /** Ground she can walk to (world/stories.ts): a tale's things are set down only where she can reach them. */
  ground?: Uint8Array;
};

export type Stage = "chunks" | "pois" | "areas" | "places";

/**
 * Put the rows of one stage on the map. Three stages because the builder claims ground as
 * it goes: inside a chunk must happen before the chunk's box is closed to scatter, at a
 * small place after it has been dressed, in an area last, on whatever is still open.
 */
export function applyPlacements(ctx: PlaceCtx, stage: Stage, rows: readonly PlacementRow[] = PLACEMENTS): void {
  for (const row of rows) {
    const at = row.at;
    const rowStage: Stage = at.place ? "places" : at.poi || at.anchor ? "pois" : at.area ? "areas" : "chunks";
    if (rowStage !== stage) continue;
    if (at.place) {
      const p = ctx.claims?.get(at.place);
      if (p) atPlace(ctx, row, p);
      continue;
    }
    if (row.edit) {
      const target = ctx.k.props.find((p) => p.key === row.key);
      if (target) Object.assign(target, row.edit);
      continue;
    }
    if (at.slot) {
      // Exactly there. A door belongs IN the wall, which no search for open ground would ever choose.
      const cell = ctx.chunks.map((c) => c.slots?.[at.slot!]).find((s) => s !== undefined) ?? ctx.areaSlots[at.slot];
      if (cell && row.prop) {
        const size = ctx.sizes[row.prop.def] ?? { w: 1, h: 1 };
        ctx.k.prop({ ...row.prop, key: row.key, cx: cell[0], cy: cell[1] }, size.w, size.h);
      }
      continue;
    }
    const anchor = anchorOf(ctx, row);
    if (!anchor) continue;
    const size = row.prop ? (ctx.sizes[row.prop.def] ?? { w: 1, h: 1 }) : { w: 1, h: 1 };
    for (const key of keysOf(row)) {
      // Spread: a point of its own somewhere in the patch, then the nearest open ground to THAT.
      let from: Anchor = anchor;
      if (row.spread) {
        // Inside the patch's circle, not its square: the corners of the square are somebody else's ground
        // (and somebody else's threat). Small search radius, so it stays where it was thrown.
        const reach = Math.floor(anchor.within * 0.8);
        let ox = 0;
        let oy = 0;
        for (let tries = 0; tries < 8; tries++) {
          ox = ctx.k.int(-reach, reach);
          oy = ctx.k.int(-reach, reach);
          if (ox * ox + oy * oy <= reach * reach) break;
          ox = oy = 0;
        }
        from = { cx: anchor.cx + ox, cy: anchor.cy + oy, within: 6 };
      }
      const spot = openSpot(ctx.k, from, size.w, size.h) ?? (row.spread ? openSpot(ctx.k, anchor, size.w, size.h) : null);
      if (!spot) break;
      if (row.prop) ctx.k.prop({ ...row.prop, key, cx: spot.cx, cy: spot.cy }, size.w, size.h);
      if (row.unit) {
        // Beside the prop if there is one, on the spot if not.
        const ux = row.prop ? spot.cx + size.w : spot.cx;
        const u = ctx.k.unit(key, row.unit.def, ux, spot.cy);
        u.facing = row.unit.facing;
        const phase = row.unit.phase ?? ctx.threatAt(ux, spot.cy);
        if (phase > 1) u.phase = phase;
      }
      if (row.rect && key === keysOf(row)[0]) {
        // Centred on the NAMED place, not on wherever the thing found room: the text says "at the scarecrow".
        const r: Rect = { cx: anchor.cx - Math.floor(row.rect.w / 2), cy: anchor.cy - Math.floor(row.rect.h / 2), w: row.rect.w, h: row.rect.h };
        ctx.k.rect(row.rect.name, r);
      }
      if (row.mark && key === keysOf(row)[0]) ctx.k.mark(row.mark, spot.cx, spot.cy + size.h, 1);
    }
  }
}

/**
 * A row at a story's place. The place was built and claimed whole, so nothing here searches for
 * open ground: a kept cell is exactly where the thing goes, a named prop is edited in place, and a
 * resident is replaced where she stands by the story's own person, who walks her round.
 */
function atPlace(ctx: PlaceCtx, row: PlacementRow, p: Place): void {
  const k = ctx.k;
  const slot = row.at.slot ?? "";
  if (row.edit) {
    const key = p.things[slot];
    const target = key ? k.props.find((q) => q.key === key) : undefined;
    if (target) Object.assign(target, row.edit);
    return;
  }
  if (slot === "hostiles") {
    // A camp that is a story's: what stands round its fire is the story's own creature (one def per
    // camp, so a kill only counts here, and nothing comes back once it is cleared).
    if (!row.unit) return;
    for (const key of p.hostiles) {
      const u = k.units.find((x) => x.key === key);
      if (u) u.def = row.unit.def;
    }
    return;
  }
  let cell: [number, number] | undefined = p.slots[slot];
  let patrol: UnitSpawn["patrol"];
  if (row.at.on !== undefined) {
    const under = k.props.find((q) => q.key === row.at.on);
    if (!under) return;
    cell = [under.cx + (row.at.dx ?? 0), under.cy + (row.at.dy ?? 0)];
  } else if (row.at.dx !== undefined || row.at.dy !== undefined) {
    const size = row.prop ? (ctx.sizes[row.prop.def] ?? { w: 1, h: 1 }) : { w: 1, h: 1 };
    // `room`: open ground for the whole of a small scene (the box, the stone and somewhere to stand
    // to push it), with this row's thing at (ox, oy) in it; later rows sit `on` it.
    const [rw, rh, ox, oy] = row.at.room ?? [size.w, size.h, 0, 0];
    const spot = openAt(k, ctx.sizes, ctx.ground, p.box.cx + (row.at.dx ?? 0), p.box.cy + (row.at.dy ?? 0), rw, rh, row.at.within ?? 3);
    if (!spot) return;
    cell = [spot[0] + ox, spot[1] + oy];
  }
  const folk = /^folk(?:_(\d))?$/.exec(slot);
  if (folk) {
    const key = p.folk[Number(folk[1] ?? 1) - 1];
    const n = k.units.findIndex((u) => u.key === key);
    if (n < 0) return;
    const was = k.units[n];
    k.units.splice(n, 1);
    cell = [was.cx, was.cy];
    patrol = was.patrol;
  }
  if (!cell) {
    // Beside one of the place's props: the open cell below it.
    const key = p.things[slot];
    const q = key ? k.props.find((x) => x.key === key) : undefined;
    if (!q) return;
    cell = [q.cx, q.cy + (ctx.sizes[q.def]?.h ?? 1)];
  }
  const [cx, cy] = cell;
  if (row.prop) {
    const size = ctx.sizes[row.prop.def] ?? { w: 1, h: 1 };
    k.prop({ ...row.prop, key: row.key, cx, cy }, size.w, size.h);
  }
  if (row.unit) {
    const ux = row.prop ? cx + (ctx.sizes[row.prop.def]?.w ?? 1) : cx;
    const u = k.unit(row.key, row.unit.def, ux, cy, row.prop ? undefined : patrol);
    u.facing = row.unit.facing;
    if (row.unit.phase !== undefined && row.unit.phase > 1) u.phase = row.unit.phase;
  }
  // Below a thing, below a person; a mark on its own is where it was put.
  if (row.mark) k.mark(row.mark, cx, cy + (row.prop ? (ctx.sizes[row.prop.def]?.h ?? 1) : row.unit ? 1 : 0), 1);
  if (row.rect) k.rect(row.rect.name, { cx: cx - (row.rect.w >> 1), cy: cy - (row.rect.h >> 1), w: row.rect.w, h: row.rect.h });
}

/** Growth and rubble a tale may clear from under what it sets down. Water, walls and fences stay. */
const SOFT_GROUND = new Set<number>([Tile.Bush, Tile.GrassTall, Tile.Tree, Tile.Pine, Tile.DeadTree, Tile.Rubble]);

/**
 * The nearest footprint to (x, y), in rings out to `within`, whose ground is walkable or only growth,
 * and on which nothing already stands. Inside a built place every cell is claimed, so this looks at the
 * ground and the things themselves rather than the claim map. It draws no randomness: a tale's dressing
 * moves nothing else in the county.
 */
function openAt(k: Kit, sizes: PlaceCtx["sizes"], ground: Uint8Array | undefined, x: number, y: number, w: number, h: number, within: number): [number, number] | null {
  const R = within + 16;
  const near = k.props.filter((q) => Math.abs(q.cx - x) <= R && Math.abs(q.cy - y) <= R);
  const people = k.units.filter((u) => Math.abs(u.cx - x) <= R && Math.abs(u.cy - y) <= R);
  const marks = Object.values(k.marks).filter((m) => Math.abs(m.cx - x) <= R && Math.abs(m.cy - y) <= R);
  const free = (cx: number, cy: number): boolean => {
    for (let j = cy; j < cy + h; j++) {
      for (let i = cx; i < cx + w; i++) {
        if (i < 0 || j < 0 || i >= k.w || j >= k.h) return false;
        const t = k.get(i, j);
        if (k.solid(i, j) && !SOFT_GROUND.has(t)) return false;
        if (t === Tile.Water || t === Tile.Road) return false;
      }
    }
    for (const q of near) {
      const s = sizes[q.def] ?? { w: 1, h: 1 };
      if (q.cx < cx + w && q.cx + s.w > cx && q.cy < cy + h && q.cy + s.h > cy) return false;
    }
    if (people.some((u) => u.cx >= cx && u.cx < cx + w && u.cy >= cy && u.cy < cy + h)) return false;
    // Not on a mark, nor hard beside one: that is where somebody stands (in front of a board, at a door).
    if (marks.some((m) => m.cx >= cx - 1 && m.cx <= cx + w && m.cy >= cy - 1 && m.cy <= cy + h)) return false;
    if (!ground) return true;
    // Somewhere she can already walk to touches it: the ground under it, or beside it.
    for (let j = cy - 1; j <= cy + h; j++) {
      for (let i = cx - 1; i <= cx + w; i++) {
        const corner = (i === cx - 1 || i === cx + w) && (j === cy - 1 || j === cy + h);
        if (!corner && i >= 0 && j >= 0 && i < k.w && j < k.h && ground[j * k.w + i]) return true;
      }
    }
    return false;
  };
  for (let r = 0; r <= within; r++) {
    for (let oy = -r; oy <= r; oy++) {
      for (let ox = -r; ox <= r; ox++) {
        if (Math.max(Math.abs(ox), Math.abs(oy)) !== r || !free(x + ox, y + oy)) continue;
        for (let j = y + oy; j < y + oy + h; j++) for (let i = x + ox; i < x + ox + w; i++) if (SOFT_GROUND.has(k.get(i, j))) k.set(i, j, Tile.Dirt);
        return [x + ox, y + oy];
      }
    }
  }
  return null;
}

type Anchor = { cx: number; cy: number; within: number };

function anchorOf(ctx: PlaceCtx, row: PlacementRow): Anchor | null {
  const at = row.at;
  if (at.mark) {
    const m = ctx.k.marks[at.mark];
    return m ? { cx: m.cx, cy: m.cy, within: at.within ?? 8 } : null;
  }
  if (at.site) {
    const c = ctx.chunks.find((x) => x.id === at.site);
    if (!c) return null;
    return { cx: c.box.cx + Math.floor(c.box.w / 2), cy: c.box.cy + Math.floor(c.box.h / 2), within: at.within ?? Math.floor(Math.max(c.box.w, c.box.h) / 2) + 4 };
  }
  if (at.area) {
    const a = ctx.sk.areas.find((x) => x.id === at.area);
    if (!a) return null;
    return { cx: a.mx * MACRO + MACRO / 2, cy: a.my * MACRO + MACRO / 2, within: at.within ?? Math.floor(a.row.radius * 0.8) };
  }
  if (at.anchor) {
    const p = ctx.pois.find((x) => x.anchor === at.anchor);
    return p ? { cx: p.x, cy: p.y + 3, within: at.within ?? 6 } : null;
  }
  if (at.poi) {
    const n = ctx.claimed.get(row.key);
    if (n === undefined) return null;
    return { cx: ctx.pois[n].x, cy: ctx.pois[n].y + 3, within: at.within ?? 5 };
  }
  return null;
}

/** An open, unclaimed footprint near the anchor, tried outward in rings so a thing lands as close as it can. Seeded. */
function openSpot(k: Kit, a: Anchor, w: number, h: number): { cx: number; cy: number } | null {
  for (let r = 1; r <= a.within; r = Math.ceil(r * 1.6)) {
    const spot = k.spot({ cx: a.cx - r, cy: a.cy - r, w: r * 2 + 1, h: r * 2 + 1 }, w, h, 1, 24);
    if (spot) return spot;
  }
  return null;
}
