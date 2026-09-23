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
//             prop   a prop some chunk placed, by key: the open ground in front of it (a door's step)
//             place  a generated place a story claimed (world/stories.ts, data/stories), with
//                    `slot` naming what in it: a cell it kept open ("yard", "garden", "green"),
//                    one of its own props ("house", "well", "barn", "chest": for `edit`), or
//                    one of its people ("folk", "folk_2": the row's unit takes her place and
//                    her round). A story that found no place on a seed places nothing, and
//                    promises nothing: these rows are not in the contract.
//           `within`: how far from the spot it may land, in cells.
//   unit / prop / rect / mark   what to put there (any combination; they share the spot)
//   edit    instead of placing: change a prop a chunk already placed (give the car a glovebox)
//   hides   a thing lying UNDER the placed prop (which pushes): a hidden prop at the same cell,
//           shown when the prop is pushed off it (sim/under.ts), only while `when` holds. With
//           `count`, it is under ONE of them, the seed's choice: "she cannot remember which stone".
//
// Everything a row names is added to the county's contract, so a seed that cannot place
// it fails validation and is re-rolled like any other broken county. Nothing is skipped
// quietly: that is how the last attempt at this game ended up with 61 quests nobody
// could hand in.

import { hashString, rngSeed, rngU32, type RngState } from "@/sim/rng";
import { F_SOLID, Tile, TILE_FLAGS } from "@/sim/grid";
import type { Condition, Facing } from "@/sim/state";
import type { PropSpawn, Rect, UnitSpawn } from "@/world/blueprint";
import type { Chunk } from "@/world/chunks";
import type { Place } from "@/world/country";
import type { Kit } from "@/world/kit";
import { MACRO, type Skeleton } from "@/world/skeleton";

export type PlacementRow = {
  key: string;
  count?: number;
  /** With `count` in an area: stand them about the patch, each somewhere of its own, not shoulder to shoulder at its centre. */
  spread?: boolean;
  at: { mark?: string; site?: string; area?: string; poi?: string; near?: string; anchor?: string; slot?: string; place?: string; prop?: string; within?: number };
  /** The key of a prop a chunk placed, and the fields to set on it. Nothing new is placed. */
  edit?: Partial<Omit<PropSpawn, "key" | "cx" | "cy" | "def">>;
  unit?: { def: string; phase?: number; facing?: Facing };
  prop?: Omit<PropSpawn, "key" | "cx" | "cy">;
  rect?: { name: string; w: number; h: number };
  mark?: string;
  hides?: { key: string; prop: Omit<PropSpawn, "key" | "cx" | "cy">; when?: Condition[] };
  /**
   * Where the words put it: once placed, move it to the nearest open cell beside this prop (by key).
   * "A child's red glove lies by the well wall." Moving draws no dice, so the row keeps its old spot
   * in the county's stream and nothing else on the seed shifts.
   */
  beside?: string;
  /** Throw this row's own dice, not the county's (see applyPlacements). For rows added after the seeds were tuned. */
  ownDice?: boolean;
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
    if (row.hides) out.props.push(row.hides.key);
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
};

export type Stage = "chunks" | "pois" | "areas" | "places";

/**
 * Put the rows of one stage on the map. Three stages because the builder claims ground as
 * it goes: inside a chunk must happen before the chunk's box is closed to scatter, at a
 * small place after it has been dressed, in an area last, on whatever is still open.
 */
export function applyPlacements(ctx: PlaceCtx, stage: Stage, rows: readonly PlacementRow[] = PLACEMENTS): void {
  // A row with `ownDice` (and every row placed `at.prop`, which is new) throws its own dice: a stream
  // of its own, from the stage's start and its key, and the county's stream put back as it was
  // afterwards. So adding such a row, or a stone to a garden, or eleven parcels to a step, moves
  // nothing else in the county: not another row's spot, not the next farm along. The older rows keep
  // drawing on the county's stream as they always have, so no seed anybody has tuned against moves.
  const k = ctx.k;
  const start: RngState = [k.rng[0], k.rng[1], k.rng[2], k.rng[3]];
  const base = rngU32([...start] as RngState);
  for (const row of rows) {
    if (!row.ownDice && !row.at.prop) {
      applyRow(ctx, stage, row);
      continue;
    }
    const outer: RngState = [k.rng[0], k.rng[1], k.rng[2], k.rng[3]];
    const own = rngSeed(base ^ hashString(row.key), 7);
    for (let i = 0; i < 4; i++) k.rng[i] = own[i];
    try {
      applyRow(ctx, stage, row);
    } finally {
      for (let i = 0; i < 4; i++) k.rng[i] = outer[i];
    }
  }
}

function applyRow(ctx: PlaceCtx, stage: Stage, row: PlacementRow): void {
  {
    const at = row.at;
    const rowStage: Stage = at.place ? "places" : at.poi || at.anchor ? "pois" : at.area ? "areas" : "chunks";
    if (at.prop) {
      // Beside a prop another row or a chunk placed: in the first stage the prop is there by, once.
      if (!ctx.k.props.some((p) => p.key === at.prop) || ctx.k.props.some((p) => p.key === keysOf(row)[0])) return;
    } else if (rowStage !== stage) return;
    if (at.place) {
      const p = ctx.claims?.get(at.place);
      if (p) atPlace(ctx, row, p);
      return;
    }
    if (row.edit) {
      const target = ctx.k.props.find((p) => p.key === row.key);
      if (target) Object.assign(target, row.edit);
      return;
    }
    if (at.slot) {
      // Exactly there. A door belongs IN the wall, which no search for open ground would ever choose.
      const cell = ctx.chunks.map((c) => c.slots?.[at.slot!]).find((s) => s !== undefined) ?? ctx.areaSlots[at.slot];
      if (cell && row.prop) {
        const size = ctx.sizes[row.prop.def] ?? { w: 1, h: 1 };
        const top = ctx.k.prop({ ...row.prop, key: row.key, cx: cell[0], cy: cell[1] }, size.w, size.h);
        hideUnder(ctx, row, top);
      }
      return;
    }
    const anchor = anchorOf(ctx, row);
    if (!anchor) return;
    const size = row.prop ? (ctx.sizes[row.prop.def] ?? { w: 1, h: 1 }) : { w: 1, h: 1 };
    const tops: PropSpawn[] = [];
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
      if (row.prop) tops.push(ctx.k.prop({ ...row.prop, key, cx: spot.cx, cy: spot.cy }, size.w, size.h));
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
    if (row.beside) for (const t of tops) moveBeside(ctx, t, row.beside);
    if (tops.length > 0) hideUnder(ctx, row, pickTop(row, tops));
  }
}

/**
 * Put a placed thing on the nearest open cell touching a prop's footprint: not a wall, not water, not
 * under another prop. Tried in rings outward from the footprint's edge, in a fixed order, no dice.
 */
function moveBeside(ctx: PlaceCtx, t: PropSpawn, key: string): void {
  const k = ctx.k;
  const q = k.props.find((p) => p.key === key);
  if (!q) return;
  const qs = ctx.sizes[q.def] ?? { w: 1, h: 1 };
  const ts = ctx.sizes[t.def] ?? { w: 1, h: 1 };
  const taken = (x: number, y: number): boolean =>
    k.props.some((p) => {
      if (p === t) return false;
      const s = ctx.sizes[p.def] ?? { w: 1, h: 1 };
      return x < p.cx + s.w && p.cx < x + ts.w && y < p.cy + s.h && p.cy < y + ts.h;
    }) || k.units.some((u) => u.cx >= x && u.cx < x + ts.w && u.cy >= y && u.cy < y + ts.h);
  const open = (x: number, y: number): boolean => {
    for (let j = 0; j < ts.h; j++) for (let i = 0; i < ts.w; i++) if (k.solid(x + i, y + j) || k.get(x + i, y + j) === Tile.Water) return false;
    return !taken(x, y);
  };
  for (let r = 1; r <= 3; r++) {
    // The front first (below the footprint, where she stands to use it), then the sides, then behind.
    const ring: [number, number][] = [];
    for (let x = q.cx - r - ts.w + 1; x < q.cx + qs.w + r; x++) ring.push([x, q.cy + qs.h + r - 1]);
    for (let y = q.cy + qs.h + r - 2; y > q.cy - r - ts.h; y--) {
      ring.push([q.cx - r - ts.w + 1, y]);
      ring.push([q.cx + qs.w + r - 1, y]);
    }
    for (let x = q.cx - r - ts.w + 1; x < q.cx + qs.w + r; x++) ring.push([x, q.cy - r - ts.h + 1]);
    for (const [x, y] of ring) {
      if (!open(x, y)) continue;
      t.cx = x;
      t.cy = y;
      k.claim(x, y, ts.w, ts.h);
      return;
    }
  }
}

/** Which of a row's props the thing is under: a hash of where they stand, so the seed chooses and the stream is not drawn on. */
function pickTop(row: PlacementRow, tops: readonly PropSpawn[]): PropSpawn {
  if (tops.length === 1) return tops[0];
  const at = tops.map((t) => `${t.cx},${t.cy}`).join(";");
  return tops[hashString(`${row.key}:${at}`) % tops.length];
}

/** The thing a row `hides`: a hidden prop at the top prop's cell, which the top prop names as lying `under` it. */
function hideUnder(ctx: PlaceCtx, row: PlacementRow, top: PropSpawn): void {
  const h = row.hides;
  if (!h) return;
  const size = ctx.sizes[h.prop.def] ?? { w: 1, h: 1 };
  ctx.k.prop({ ...h.prop, key: h.key, cx: top.cx, cy: top.cy, hidden: true }, size.w, size.h);
  top.under = h.key;
  if (h.when) top.underWhen = h.when;
}

/**
 * Open cells at a story's place, nearest the slot first, for a row with `count`: the place was
 * claimed whole, so the kit's own "open ground" says no everywhere in it. A cell here is floor that
 * nothing solid stands on and nobody stands on, with open floor on both sides along one axis, so a
 * pushed thing can go one way and she can stand on the other.
 */
function placeCells(ctx: PlaceCtx, from: [number, number], within: number, n: number, key: string): [number, number][] {
  const k = ctx.k;
  const x0 = from[0] - within - 1;
  const y0 = from[1] - within - 1;
  const span = within * 2 + 3;
  const busy = new Uint8Array(span * span);
  const mark = (x: number, y: number): void => {
    if (x >= x0 && y >= y0 && x < x0 + span && y < y0 + span) busy[(y - y0) * span + (x - x0)] = 1;
  };
  for (const p of k.props) {
    const s = ctx.sizes[p.def] ?? { w: 1, h: 1 };
    if (p.cx > x0 + span || p.cy > y0 + span || p.cx + s.w < x0 || p.cy + s.h < y0) continue;
    for (let j = 0; j < s.h; j++) for (let i = 0; i < s.w; i++) mark(p.cx + i, p.cy + j);
  }
  for (const u of k.units) mark(u.cx, u.cy);
  const open = (x: number, y: number): boolean => {
    if (x < x0 || y < y0 || x >= x0 + span || y >= y0 + span) return false;
    return busy[(y - y0) * span + (x - x0)] === 0 && (TILE_FLAGS[k.get(x, y)] & F_SOLID) === 0;
  };
  const cells: [number, number][] = [];
  for (let y = from[1] - within; y <= from[1] + within; y++) {
    for (let x = from[0] - within; x <= from[0] + within; x++) {
      if (!open(x, y)) continue;
      if (!((open(x - 1, y) && open(x + 1, y)) || (open(x, y - 1) && open(x, y + 1)))) continue;
      cells.push([x, y]);
    }
  }
  // Nearest first, then the seed's hash; never two side by side, so each one can be pushed.
  const d = (c: [number, number]): number => Math.max(Math.abs(c[0] - from[0]), Math.abs(c[1] - from[1]));
  cells.sort((a, b) => d(a) - d(b) || hashString(`${key}:${a[0]},${a[1]}`) - hashString(`${key}:${b[0]},${b[1]}`));
  const out: [number, number][] = [];
  for (const c of cells) {
    if (out.some((o) => Math.abs(o[0] - c[0]) <= 1 && Math.abs(o[1] - c[1]) <= 1)) continue;
    out.push(c);
    if (out.length === n) break;
  }
  return out;
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
  if (row.prop && ((row.count ?? 1) > 1 || row.at.within !== undefined)) {
    // Several of a thing about the slot (the stones in a garden), or one beside what already stands on
    // it (the chair by the candles): each on open floor of its own, nearest the slot.
    const size = ctx.sizes[row.prop.def] ?? { w: 1, h: 1 };
    const keys = keysOf(row);
    const cells = placeCells(ctx, cell, row.at.within ?? 3, keys.length, row.key);
    const tops = cells.map(([x, y], n) => k.prop({ ...row.prop!, key: keys[n], cx: x, cy: y }, size.w, size.h));
    if (tops.length > 0) hideUnder(ctx, row, pickTop(row, tops));
  } else if (row.prop) {
    const size = ctx.sizes[row.prop.def] ?? { w: 1, h: 1 };
    const top = k.prop({ ...row.prop, key: row.key, cx, cy }, size.w, size.h);
    hideUnder(ctx, row, top);
  }
  if (row.unit) {
    const ux = row.prop ? cx + (ctx.sizes[row.prop.def]?.w ?? 1) : cx;
    const u = k.unit(row.key, row.unit.def, ux, cy, row.prop ? undefined : patrol);
    u.facing = row.unit.facing;
    if (row.unit.phase !== undefined && row.unit.phase > 1) u.phase = row.unit.phase;
  }
  if (row.mark) k.mark(row.mark, cx, cy + (row.prop ? (ctx.sizes[row.prop.def]?.h ?? 1) : 1), 1);
  if (row.rect) k.rect(row.rect.name, { cx: cx - (row.rect.w >> 1), cy: cy - (row.rect.h >> 1), w: row.rect.w, h: row.rect.h });
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
  if (at.prop) {
    // In front of it: the cell below its footprint, the middle of its width. A door's step.
    const q = ctx.k.props.find((x) => x.key === at.prop);
    if (!q) return null;
    const s = ctx.sizes[q.def] ?? { w: 1, h: 1 };
    return { cx: q.cx + (s.w >> 1), cy: q.cy + s.h, within: at.within ?? 3 };
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
