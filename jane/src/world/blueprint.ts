// A Blueprint is what a zone builder returns: terrain plus spawn rows plus named
// places. It is a pure function of (zone id, run seed), so it is never saved;
// the save holds the seed and whatever has changed since.
//
// Builders address nothing by coordinate outside themselves. Story, triggers,
// dialogue and travel refer to `key`s, `marks` and `rects` by name. That is the
// contract that lets geometry roll per seed while the story stays fixed.

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
};

/** Keys, marks and rects every seed of a zone must contain. Checked by world/validate.ts. */
export type ZoneContract = {
  units: string[];
  props: string[];
  marks: string[];
  rects: string[];
};
