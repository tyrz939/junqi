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
//           `within`: how far from the anchor it may land, in cells.
//   unit / prop / rect / mark   what to put there (any combination; they share the spot)
//
// Everything a row names is added to the county's contract, so a seed that cannot place
// it fails validation and is re-rolled like any other broken county. Nothing is skipped
// quietly: that is how the last attempt at this game ended up with 61 quests nobody
// could hand in.

import type { Facing } from "@/sim/state";
import type { PropSpawn, Rect } from "@/world/blueprint";
import type { Chunk } from "@/world/chunks";
import type { Kit } from "@/world/kit";
import { MACRO, type Skeleton } from "@/world/skeleton";

export type PlacementRow = {
  key: string;
  count?: number;
  at: { mark?: string; site?: string; area?: string; poi?: string; near?: string; within?: number };
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

export type PoiSpot = { x: number; y: number; kind: string };

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
  sk: Skeleton;
  chunks: readonly Chunk[];
  pois: readonly PoiSpot[];
  claimed: ReadonlyMap<string, number>;
  /** Footprint of each prop def, in cells. */
  sizes: Readonly<Record<string, { w: number; h: number }>>;
  /** Threat of the ground under a cell, for a placed creature's phase. */
  threatAt: (cx: number, cy: number) => number;
};

export type Stage = "chunks" | "pois" | "areas";

/**
 * Put the rows of one stage on the map. Three stages because the builder claims ground as
 * it goes: inside a chunk must happen before the chunk's box is closed to scatter, at a
 * small place after it has been dressed, in an area last, on whatever is still open.
 */
export function applyPlacements(ctx: PlaceCtx, stage: Stage, rows: readonly PlacementRow[] = PLACEMENTS): void {
  for (const row of rows) {
    const at = row.at;
    const rowStage: Stage = at.poi ? "pois" : at.area ? "areas" : "chunks";
    if (rowStage !== stage) continue;
    const anchor = anchorOf(ctx, row);
    if (!anchor) continue;
    const size = row.prop ? (ctx.sizes[row.prop.def] ?? { w: 1, h: 1 }) : { w: 1, h: 1 };
    for (const key of keysOf(row)) {
      const spot = openSpot(ctx.k, anchor, size.w, size.h);
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
        const r: Rect = { cx: spot.cx - Math.floor(row.rect.w / 2), cy: spot.cy - Math.floor(row.rect.h / 2), w: row.rect.w, h: row.rect.h };
        ctx.k.rect(row.rect.name, r);
      }
      if (row.mark && key === keysOf(row)[0]) ctx.k.mark(row.mark, spot.cx, spot.cy + size.h, 1);
    }
  }
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
