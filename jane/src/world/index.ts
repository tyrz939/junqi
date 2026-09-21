// Zone registry. buildZone is a pure function of (zone, seed): it rolls a
// candidate, validates it with the lock-and-key solver, and re-rolls on failure.
// The seed the player sees never changes; only the per-zone attempt counter does.

import { buildCatalog, type Catalog } from "@/sim/catalog";
import type { ZoneId } from "@/sim/state";
import { ZONE_ATTEMPTS, type Blueprint, type ZoneContract } from "@/world/blueprint";
import { buildBurial } from "@/world/burial";
import { buildCounty } from "@/world/county";
import { buildCellar, buildHouse } from "@/world/interiors";
import { MINE } from "@/world/mine";
import { placementContract } from "@/world/placements";
import { REGISTERED } from "@/world/registry";
import { validateBlueprint, type SolveState } from "@/world/validate";

export type { Builder, ZoneDef } from "@/world/registry";
import type { Builder, ZoneDef } from "@/world/registry";

export const BUILDERS: Record<ZoneId, Builder> = {
  county: buildCounty,
  house: buildHouse,
  cellar: buildCellar,
  mine: MINE.build,
  burial: buildBurial,
};

/** What every seed of a zone must contain. Story, triggers and travel lean on these names. */
export const CONTRACTS: Record<ZoneId, ZoneContract> = {
  county: {
    units: ["dog", "yard_skeleton"],
    props: ["house_door", "mine_door", "adit_door", "burial_door"],
    marks: ["start", "house_front", "mine_mouth", "mine_adit", "burial_mouth"],
    rects: ["stoop", "mine_yard"],
  },
  house: {
    units: [],
    props: ["front_door", "hatch_a", "hatch_b", "bench", "ice_orb", "julies_note", "pantry_chest"],
    marks: ["front", "hatch_a", "hatch_b"],
    rects: ["kitchen"],
  },
  cellar: {
    units: [],
    props: ["stair_a", "stair_b", "cellar_chest", "iron_door_a", "iron_door_b", "storage_gate", "storage_chest", "potion_bench"],
    marks: ["stair_a", "stair_b"],
    rects: ["cellar"],
  },
  mine: {
    units: ["clerk", "headmaster", "iron_knuckles"],
    props: [
      "exit_door",
      "plate_a",
      "plate_chest",
      "store_chest",
      "gate_generic_a",
      "gate_generic_b",
      "gate_hm",
      "broken_steps",
      "boss_key_chest",
      "adit_door",
      "gate_vault",
      "vault_chest",
      "gate_boss",
    ],
    marks: ["entry", "adit"],
    rects: ["mine_entry", "boss_arena"],
  },
  burial: {
    units: ["burial_snake", "garden_flower"],
    props: [
      "exit_door",
      "torch_a",
      "torch_b",
      "torch_chest",
      "snake_key_chest",
      "gate_snake",
      "giant_key_chest",
      "snake_gate_east",
      "snake_gate_south",
      "root_a",
      "root_b",
      "root_c",
      "fire_scroll",
    ],
    marks: ["entry", "lockin_a", "lockin_b", "lockin_c", "lockin_d"],
    rects: ["burial_entry", "garden", "lockin_room", "snake_arena", "everywhere"],
  },
};

/** Keys the story hands over from outside the zone (quest rewards), by `opens` tag. */
export const GIVEN_KEYS: Record<ZoneId, string[]> = {
  county: ["auntie_house"],
  house: [],
  cellar: [],
  mine: [],
  burial: [],
};

/** Spells known at the door, for the zones that are proven against them. */
export const GIVEN_VERBS: Record<ZoneId, string[] | undefined> = { mine: MINE.givenVerbs };

/** Reversible mechanisms, by zone, for the stateful flood. None of the original five has any. */
export const STATES: Record<ZoneId, SolveState[] | undefined> = {};

/** Checks beyond the solver's, by zone. The mine is generated, and must be the mine that was designed. */
export const CHECKS: Record<ZoneId, ZoneDef["check"]> = { mine: MINE.check };

// What data/placements promises is part of the county's contract: a seed that cannot place a
// quest's scarecrow is a broken county and is re-rolled, exactly like one with no stoop.
{
  const promised = placementContract();
  const c = CONTRACTS.county;
  c.units = [...c.units, ...promised.units];
  c.props = [...c.props, ...promised.props];
  c.marks = [...c.marks, ...promised.marks];
  c.rects = [...c.rects, ...promised.rects];
}

for (const z of REGISTERED) {
  if (BUILDERS[z.id]) throw new Error(`Zone "${z.id}" is registered twice`);
  BUILDERS[z.id] = z.build;
  CONTRACTS[z.id] = z.contract;
  GIVEN_KEYS[z.id] = z.givenKeys ?? [];
  GIVEN_VERBS[z.id] = z.givenVerbs;
  STATES[z.id] = z.states;
  CHECKS[z.id] = z.check;
}

/** Every zone, in the order the scheduler ticks them: the original five, then the rest by file name. Fixed, so it can never decide an outcome. */
export const ZONE_IDS: readonly ZoneId[] = Object.keys(BUILDERS);

const MAX_ATTEMPTS = ZONE_ATTEMPTS;
let catalogForValidation: Catalog | null = null;

export function buildZone(zone: ZoneId, seed: number): Blueprint {
  if (!BUILDERS[zone]) throw new Error(`Unknown zone "${zone}". Zones: ${ZONE_IDS.join(" ")}`);
  catalogForValidation ??= buildCatalog();
  let lastErrors: string[] = [];
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const bp = BUILDERS[zone](seed, attempt);
    const v = validateBlueprint(bp, catalogForValidation, CONTRACTS[zone], GIVEN_KEYS[zone], { verbs: GIVEN_VERBS[zone], states: STATES[zone] });
    const errors = v.ok ? (CHECKS[zone]?.(bp, catalogForValidation) ?? []) : v.errors;
    if (errors.length === 0) return bp;
    lastErrors = errors;
  }
  throw new Error(`Zone "${zone}" failed validation ${MAX_ATTEMPTS} times for seed ${seed}:\n  ${lastErrors.join("\n  ")}`);
}
