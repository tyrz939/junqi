// The county's skeleton: everything about a seed that can be decided on a coarse
// grid, in milliseconds, before a single tile is drawn. Regions, the river, the
// hill, where the story's places are, the roads between them, how dangerous each
// patch is. PLAN.md 2.3.
//
// One macro cell is 16 x 16 cells. A cell is 8 px and a metre is 8 px, so a macro
// cell is 16 m across and the county (225 x 125 macro) is 3600 m by 2000 m.

export const MACRO = 16; // cells per macro cell, and therefore metres
export const SKEL_W = 225;
export const SKEL_H = 125;
export const COUNTY_W = SKEL_W * MACRO; // 3600 cells
export const COUNTY_H = SKEL_H * MACRO; // 2000 cells

export const enum Region {
  Lowfields = 0,
  Waters = 1,
  Works = 2,
}
export const REGION_NAMES = ["The Lowfields", "The Waters", "The Works"] as const;
export type RegionId = "lowfields" | "waters" | "works";
export const REGION_IDS: readonly RegionId[] = ["lowfields", "waters", "works"];

export const enum Biome {
  Field = 0,
  Hedge = 1,
  Wood = 2,
  Foothill = 3,
  Reed = 4,
  Marsh = 5,
  WetWood = 6,
  Garden = 7,
  Slag = 8,
  Yard = 9,
  Hill = 10,
  Town = 11,
}
export const BIOME_NAMES = ["field", "hedge", "wood", "foothill", "reed", "marsh", "wetwood", "garden", "slag", "yard", "hill", "town"] as const;

export const ROAD = 1;
export const ROAD_LIT = 2;
export const ROAD_BRIDGE = 4;

/** `by: "road"` is checked after the roads exist; placement aims for it using the crow's distance. */
export type DistanceRule = { to: string; min?: number; max?: number; by: "road" | "line" };

export type SiteRow = {
  id: string;
  name: string;
  region: RegionId;
  /** Story places are joined to the road network. `false`: deliberately off it (the burial, the car). */
  onRoad: boolean;
  /** A dungeon mouth raises threat in a ring around itself: the approach is part of the dungeon. */
  dungeon?: boolean;
  /** A bed or a fire the player can rest at. The distance between two of these is the length of a run. */
  rest?: boolean;
  /** Threat 0 inside this radius (m): nothing spawns, nothing follows you in. */
  hub?: number;
  where?: {
    edge?: "west" | "east" | "north" | "south";
    /** foothill: the rising ground. crown: the highest ground in the region. bank: beside water. flat, wood, dry. */
    terrain?: "foothill" | "crown" | "bank" | "flat" | "wood";
    acrossRiverFrom?: string;
    /** No higher than this (0..255). The town stays off the hill, so the School can stand over it. */
    maxHeight?: number;
    /** Within this many metres of the river (the town is a river town). */
    riverWithin?: number;
    /** Within this many metres of the lake's shore. */
    lakeWithin?: number;
    /** At least this far (m) from any road: not seen from one. */
    offRoad?: number;
  };
  dist?: DistanceRule[];
  /** The site this one hangs off: its road is laid from there when it is placed. Must be an earlier row. */
  roadFrom?: string;
};

export type AreaRow = {
  id: string;
  name: string;
  region: RegionId;
  /** The phase of the 2020 balance sheet this patch plays at, 0..6. 0 is a haven. */
  threat: number;
  /** Metres. */
  radius: number;
  rest?: boolean;
  /** A quest is written against this patch: a county without it is re-rolled. Most patches are texture and may be skipped. */
  required?: boolean;
  where?: { near?: string; nearMax?: number; nearMin?: number; terrain?: "wood" | "bank" | "flat" | "foothill" | "crown"; offRoad?: number; onRoad?: boolean };
};

export type PoiRow = {
  kind: string;
  name: string;
  regions: RegionId[];
  weight: number;
  where: "roadside" | "deep" | "bank" | "any";
};

export type Placed = { id: string; name: string; mx: number; my: number };
export type PlacedSite = Placed & { row: SiteRow };
export type PlacedArea = Placed & { row: AreaRow };
/** `anchor`: this small place is one the story needs (skeleton/anchors.ts), and this is its name. */
export type PlacedPoi = { kind: string; name: string; mx: number; my: number; region: Region; anchor?: string };

export type Road = { from: string; to: string; cells: number[]; metres: number };

export type Check = { rule: string; ok: boolean; detail: string };

export type Skeleton = {
  seed: number;
  /** Re-rolls it took. A skeleton that cannot satisfy its rows is never shown; the next attempt is tried. */
  attempt: number;
  w: number;
  h: number;
  height: Uint8Array;
  water: Uint8Array;
  region: Uint8Array;
  biome: Uint8Array;
  road: Uint8Array;
  /** Daytime threat, 0..6. Night is a rule applied on top (threatAt). */
  threat: Uint8Array;
  sites: PlacedSite[];
  areas: PlacedArea[];
  pois: PlacedPoi[];
  /** Small places the story needs, by name. Each is also in `pois`. */
  anchors: { id: string; kind: string; mx: number; my: number }[];
  roads: Road[];
  checks: Check[];
  ok: boolean;
};

export type SkeletonRows = { sites: SiteRow[]; areas: AreaRow[]; pois: PoiRow[]; anchors?: import("@/world/skeleton/anchors").AnchorRow[] };

export const at = (mx: number, my: number): number => my * SKEL_W + mx;
export const inside = (mx: number, my: number): boolean => mx >= 0 && my >= 0 && mx < SKEL_W && my < SKEL_H;
/** Crow's distance between two macro cells, in metres. sqrt is exact in IEEE 754; no trig anywhere in worldgen. */
export const metres = (ax: number, ay: number, bx: number, by: number): number => Math.sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by)) * MACRO;
