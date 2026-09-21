// Anchors: small places the STORY needs, as opposed to the ones the seed happens to
// roll. "The well on the road from the station", "the scarecrow at the farm gate",
// "the shrine beside the last lamp that still works". A quest is written against a
// name, and `poi_<n>` means something different on every seed, so these get names,
// constraints and a guarantee: a county that cannot place them all is re-rolled, the
// same as one that cannot place the Museum.
//
// Rows are data/anchors.json. An anchor becomes a small place of its `kind` (it is
// dressed like any other, and counts toward the region's budget), and the county gives
// it a mark and a rect named after it.

import { rngFloat, type RngState } from "@/sim/rng";
import type { Terrain } from "@/world/skeleton/terrain";
import { at, inside, metres, REGION_IDS, ROAD, ROAD_LIT, SKEL_H, SKEL_W, type PlacedArea, type PlacedPoi, type PlacedSite, type Region, type RegionId, type Road } from "@/world/skeleton/types";

export type AnchorRow = {
  id: string;
  /** A kind from pois.json, or "none" for a bare spot (a lamp post stands there, not a shrine). */
  kind: string;
  name?: string;
  region?: RegionId;
  where: {
    /** Beside the road between these two sites. */
    road?: [string, string];
    /** Crow's metres to a site. */
    dist?: { to: string; min?: number; max?: number };
    /** Inside this named patch. */
    area?: string;
    /** On the rim of this patch, on the side nearest that site. */
    rim?: { area: string; toward: string };
    /** Beside any road, as near this patch as a roadside can be. */
    nearArea?: string;
    /**
     * Beside the last lamp that works on the longest Lowfields road that is not the first
     * walk. The builder MAKES it so: that lamp is lit and the next 350 m are dark.
     */
    lastLamp?: boolean;
    /** Along the same road as that anchor, this many macro cells further into the dark. */
    after?: { anchor: string; steps: number };
    /** At least this far from another anchor. */
    apart?: { from: string; min: number };
    /** Within this band of another anchor (metres). */
    nearAnchor?: { anchor: string; min: number; max: number };
    /** Of what is left, the few nearest this site. */
    nearest?: string;
  };
};

export type PlacedAnchor = { id: string; kind: string; mx: number; my: number };

export type AnchorCtx = {
  t: Terrain;
  road: Uint8Array;
  roads: readonly Road[];
  sites: readonly PlacedSite[];
  areas: readonly PlacedArea[];
  pois: PlacedPoi[];
  /** The first walk (station, Julie's, town). Its lamps are never put out, whatever else shares its cells. */
  safe: Uint8Array;
};

/** Anchors keep this far apart unless one is placed `after` another. */
const APART = 48;
const DARK_RUN = 22; // macro cells: about 350 m

const pick = <T>(rng: RngState, list: readonly T[]): T => list[Math.min(list.length - 1, Math.floor(rngFloat(rng) * list.length))];

/** Solve the rows in order. Returns null if any cannot be placed; the caller re-rolls the county. */
export function placeAnchors(ctx: AnchorCtx, rows: readonly AnchorRow[], rng: RngState): PlacedAnchor[] | null {
  const out: PlacedAnchor[] = [];
  const along = new Map<string, { road: Road; index: number }>();
  const roadOf = (a: string, b: string): Road | undefined => ctx.roads.find((r) => (r.from === a && r.to === b) || (r.from === b && r.to === a));
  const site = (id: string): PlacedSite | undefined => ctx.sites.find((s) => s.id === id);
  const area = (id: string): PlacedArea | undefined => ctx.areas.find((a) => a.id === id);

  // Worked out once for the whole map: the rows below ask these of every cell, several times over.
  const openMask = new Uint8Array(SKEL_W * SKEL_H);
  const roadside = new Uint8Array(SKEL_W * SKEL_H);
  for (let y = 2; y < SKEL_H - 2; y++) {
    for (let x = 2; x < SKEL_W - 2; x++) {
      const i = at(x, y);
      if (ctx.road[i] & ROAD) {
        for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) roadside[at(x + ox, y + oy)] = 1;
        continue;
      }
      if (ctx.t.water[i]) continue;
      let clear = true;
      // Never inside a set chunk: the chunk would clear it away.
      for (const s of ctx.sites) {
        if (metres(x, y, s.mx, s.my) < Math.max(56, (s.row.hub ?? 0) * 0.9)) {
          clear = false;
          break;
        }
      }
      if (clear) openMask[i] = 1;
    }
  }
  const open = (x: number, y: number): boolean => inside(x, y) && openMask[at(x, y)] === 1;
  /** Open cells one or two macro cells off a road cell. */
  const beside = (cell: number): number[] => {
    const cx = cell % SKEL_W;
    const cy = Math.floor(cell / SKEL_W);
    const found: number[] = [];
    for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) if ((ox !== 0 || oy !== 0) && open(cx + ox, cy + oy)) found.push(at(cx + ox, cy + oy));
    return found;
  };

  for (const row of rows) {
    const w = row.where;
    const region = REGION_IDS.indexOf(row.region ?? "lowfields") as Region;
    let candidates: number[] = [];

    if (w.after) {
      const from = along.get(w.after.anchor);
      if (!from) return null;
      const index = Math.min(from.road.cells.length - 2, from.index + w.after.steps);
      candidates = beside(from.road.cells[index]).slice(0, 8);
      along.set(row.id, { road: from.road, index });
    } else if (w.lastLamp) {
      // The longest Lowfields road that is not the first walk. Roads run parent to child, which is away from the town.
      const roads = ctx.roads
        .filter((r) => r.from !== "station" && !(r.from === "julie_house" && r.to === "town"))
        .filter((r) => r.cells.filter((c) => ctx.t.region[c] === region).length > r.cells.length * 0.7)
        .sort((a, b) => b.cells.length - a.cells.length);
      const road = roads[0];
      if (!road || road.cells.length < 12) return null;
      // Roads merge, so this one may share its first stretch with the first walk, whose lamps are never
      // put out. Start the dark where the next few cells (where the counted lamps stand) are this road's own.
      let index = road.cells.length >= DARK_RUN + 10 ? Math.floor(road.cells.length * 0.4) : 3;
      const shared = (k: number): boolean => road.cells.slice(k + 1, k + 10).some((c) => ctx.safe[c] === 1);
      while (index < road.cells.length - 10 && shared(index)) index++;
      if (shared(index)) return null;
      for (let k = 0; k < road.cells.length; k++) {
        if (k === index) ctx.road[road.cells[k]] |= ROAD_LIT;
        else if (k > index && k <= index + DARK_RUN && !ctx.safe[road.cells[k]]) ctx.road[road.cells[k]] &= ~ROAD_LIT;
      }
      candidates = beside(road.cells[index]);
      along.set(row.id, { road, index });
    } else {
      const road = w.road ? roadOf(w.road[0], w.road[1]) : undefined;
      if (w.road && !road) return null;
      const inArea = w.area ? area(w.area) : undefined;
      if (w.area && !inArea) return null;
      const rimArea = w.rim ? area(w.rim.area) : undefined;
      const rimSite = w.rim ? site(w.rim.toward) : undefined;
      if (w.rim && (!rimArea || !rimSite)) return null;
      const nearArea = w.nearArea ? area(w.nearArea) : undefined;
      if (w.nearArea && !nearArea) return null;
      const distTo = w.dist ? site(w.dist.to) : undefined;
      const nearA = w.nearAnchor ? out.find((a) => a.id === w.nearAnchor!.anchor) : undefined;
      if (w.nearAnchor && !nearA) return null;

      const pool = road ? [...new Set(road.cells.flatMap(beside))].sort((a, b) => a - b) : null;
      const cells: number[] = [];
      const scan = (i: number): void => {
        const x = i % SKEL_W;
        const y = Math.floor(i / SKEL_W);
        if (!open(x, y)) return;
        if (!row.where.nearArea && !w.rim && ctx.t.region[i] !== region) return;
        if (distTo) {
          const d = metres(x, y, distTo.mx, distTo.my);
          if (d < (w.dist!.min ?? 0) || d > (w.dist!.max ?? Infinity)) return;
        }
        if (nearA) {
          const d = metres(x, y, nearA.mx, nearA.my);
          if (d < w.nearAnchor!.min || d > w.nearAnchor!.max) return;
        }
        if (inArea && metres(x, y, inArea.mx, inArea.my) > inArea.row.radius * 0.7) return;
        if (rimArea) {
          const d = metres(x, y, rimArea.mx, rimArea.my);
          if (d < rimArea.row.radius - 26 || d > rimArea.row.radius + 10) return;
        }
        if (nearArea && !roadside[i]) return;
        cells.push(i);
      };
      if (pool) pool.forEach(scan);
      else for (let i = 0; i < SKEL_W * SKEL_H; i++) scan(i);
      candidates = cells;

      // "Nearest" rules keep the best few rather than any: the rim toward a site, the roadside nearest a patch.
      const nearestTo = (tx: number, ty: number, keep: number): void => {
        candidates = [...candidates].sort((a, b) => metres(a % SKEL_W, Math.floor(a / SKEL_W), tx, ty) - metres(b % SKEL_W, Math.floor(b / SKEL_W), tx, ty) || a - b).slice(0, keep);
      };
      if (rimSite) nearestTo(rimSite.mx, rimSite.my, 6);
      const nearestSite = w.nearest ? site(w.nearest) : undefined;
      if (nearestSite) nearestTo(nearestSite.mx, nearestSite.my, 3);
      if (nearArea) {
        nearestTo(nearArea.mx, nearArea.my, 6);
      }
    }

    // Spacing: from every other anchor, and the row's own `apart`.
    candidates = candidates.filter((i) => {
      const x = i % SKEL_W;
      const y = Math.floor(i / SKEL_W);
      for (const a of out) {
        const need = w.apart && w.apart.from === a.id ? w.apart.min : w.after || w.nearAnchor ? 0 : APART;
        if (metres(x, y, a.mx, a.my) < need) return false;
      }
      return true;
    });
    if (candidates.length === 0) return null;
    const cell = pick(rng, candidates);
    out.push({ id: row.id, kind: row.kind, mx: cell % SKEL_W, my: Math.floor(cell / SKEL_W) });
  }

  // An anchor IS a small place. Whatever the seed rolled within 100 m of one gives way to it.
  for (const a of out) {
    for (let n = ctx.pois.length - 1; n >= 0; n--) if (metres(a.mx, a.my, ctx.pois[n].mx, ctx.pois[n].my) < 100) ctx.pois.splice(n, 1);
  }
  for (const a of out) {
    const row = rows.find((r) => r.id === a.id)!;
    ctx.pois.push({ kind: a.kind, name: row.name ?? a.id, mx: a.mx, my: a.my, region: ctx.t.region[at(a.mx, a.my)] as Region, anchor: a.id });
  }
  return out;
}
