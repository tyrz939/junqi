// Which zones exist. A zone registers itself by being a file in world/zones/ that
// exports a ZoneDef; nothing central lists them.
//
// This lives apart from world/index.ts so that the COUNTY may ask the question too
// without importing it back. The county grows a door into a dungeon the moment that
// dungeon lands, and shows a blank face until then: a locked door with nothing behind
// it would be a lie, and a door into a zone that does not exist is a crash.

import type { Catalog } from "@/sim/catalog";
import type { ZoneId } from "@/sim/state";
import type { Blueprint, ZoneContract } from "@/world/blueprint";
import type { SolveState } from "@/world/validate";

export type Builder = (seed: number, attempt: number) => Blueprint;

/**
 * A zone that registers itself: a file in world/zones/ that `export const zone: ZoneDef`.
 * The five original zones are listed by hand in world/index.ts; everything newer arrives
 * this way, so adding a dungeon never means editing that file.
 */
export type ZoneDef = {
  id: ZoneId;
  build: Builder;
  contract: ZoneContract;
  /** Keys the story hands over from outside the zone (quest rewards), by `opens` tag. */
  givenKeys?: string[];
  /** Spells she is known to have at the door. Given, the solver gates every answering prop on them; left out, it gates nothing. */
  givenVerbs?: string[];
  /** Reversible mechanisms of the whole zone (a breaker, a valve), at most three. Given, the solver's flood is stateful. */
  states?: SolveState[];
  /** More to prove than the solver does (a generated dungeon's C1 to C12). Any error re-rolls the candidate. */
  check?: (bp: Blueprint, catalog: Catalog) => string[];
};

export const REGISTERED: ZoneDef[] = Object.entries(import.meta.glob("./zones/*.ts", { eager: true }) as Record<string, { zone?: ZoneDef }>)
  .sort(([a], [b]) => (a < b ? -1 : 1))
  .flatMap(([, m]) => (m.zone ? [m.zone] : []));

const REGISTERED_IDS = new Set(REGISTERED.map((z) => z.id));

/** True for a zone that registered itself. The five built-in zones are not here: they always exist. */
export function hasZone(id: string): boolean {
  return REGISTERED_IDS.has(id);
}
