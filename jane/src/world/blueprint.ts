// A Blueprint is what a zone builder returns: terrain plus spawn rows plus named
// places. It is a pure function of (zone id, run seed), so it is never saved;
// the save holds the seed and whatever has changed since.
//
// Builders address nothing by coordinate outside themselves. Story, triggers,
// dialogue and travel refer to `key`s, `marks` and `rects` by name. That is the
// contract that lets geometry roll per seed while the story stays fixed.

import type { TriggerDef } from "@/sim/catalog";
import type { ActionList, Facing, Stack, ZoneId } from "@/sim/state";

export type Rect = { cx: number; cy: number; w: number; h: number };
export type Mark = { cx: number; cy: number; facing?: Facing };

export type UnitSpawn = {
  key: string;
  def: string;
  cx: number;
  cy: number;
  facing?: Facing;
  /** Patrol waypoints in cells. */
  patrol?: [number, number][];
  /**
   * The threat (1..6) of the ground it stands on: the phase of 2020's balance sheet it
   * plays at. One `skeleton` row, scaled where it is spawned, instead of skeleton_2, skeleton_3.
   */
  phase?: number;
};

export type PropSpawn = {
  key: string;
  def: string;
  cx: number;
  cy: number;
  locked?: boolean;
  keyTag?: string;
  hidden?: boolean;
  on?: boolean;
  to?: { zone: ZoneId; mark: string };
  loot?: Stack[];
  use?: ActionList;
  /** Pressure plates: runs when the last thing steps off. */
  release?: ActionList;
  /** Materials a world verb (Repair) consumes from the caster's bag. */
  needs?: Stack[];
  talk?: string;
  label?: string;
  /**
   * This door does not open after dark, and this is what it says instead. A creative
   * choice door by door, never a rule for a whole town. Put it on the OUTSIDE door only:
   * nobody is ever shut in.
   */
  nightLock?: string;
};

export type Blueprint = {
  zone: ZoneId;
  name: string;
  w: number;
  h: number;
  tiles: Uint8Array;
  units: UnitSpawn[];
  props: PropSpawn[];
  marks: Record<string, Mark>;
  rects: Record<string, Rect>;
  /** Interiors ignore the day clock and keep a fog bitmap. */
  indoor: boolean;
  /** Ambient light for interiors, 0 (black) .. 1 (full). */
  ambient: number;
  /** Generation attempts used; >1 means a candidate failed validation and was re-rolled. */
  attempts: number;
  /**
   * Trigger rows the builder wrote itself: a generated lock-in, a gate that opens on a flag.
   * Merged with the catalog's rows for this zone (`zoneTriggers` in sim/runtime.ts), in the
   * sim and in the validator alike. An id the catalog already has is a validation error.
   */
  triggers?: Record<string, TriggerDef>;
};

/**
 * Candidates buildZone rolls for one zone and one seed before it gives up. A generated
 * dungeon spends the last of them on its hand-placed fallback, so it never does give up.
 */
export const ZONE_ATTEMPTS = 12;

/** Keys, marks and rects every seed of a zone must contain. Checked by world/validate.ts. */
export type ZoneContract = {
  units: string[];
  props: string[];
  marks: string[];
  rects: string[];
};
