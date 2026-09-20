// Zone registry. buildZone is a pure function of (zone, seed): it rolls a
// candidate, validates it with the lock-and-key solver, and re-rolls on failure.
// The seed the player sees never changes; only the per-zone attempt counter does.

import { buildCatalog, type Catalog } from "@/sim/catalog";
import type { ZoneId } from "@/sim/state";
import type { Blueprint, ZoneContract } from "@/world/blueprint";
import { buildBurial } from "@/world/burial";
import { buildCounty } from "@/world/county";
import { buildCellar, buildHouse } from "@/world/interiors";
import { buildMine } from "@/world/mine";
import { validateBlueprint } from "@/world/validate";

type Builder = (seed: number, attempt: number) => Blueprint;

export const BUILDERS: Record<ZoneId, Builder> = {
  county: buildCounty,
  house: buildHouse,
  cellar: buildCellar,
  mine: buildMine,
  burial: buildBurial,
};

/** What every seed of a zone must contain. Story, triggers and travel lean on these names. */
export const CONTRACTS: Record<ZoneId, ZoneContract> = {
  county: {
    units: ["dog", "yard_skeleton"],
    props: ["house_door", "mine_door", "burial_door"],
    marks: ["start", "house_front", "mine_mouth", "burial_mouth"],
    rects: ["stoop"],
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
      "gate_vault",
      "vault_chest",
      "gate_boss",
    ],
    marks: ["entry"],
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

const MAX_ATTEMPTS = 12;
let catalogForValidation: Catalog | null = null;

export function buildZone(zone: ZoneId, seed: number): Blueprint {
  catalogForValidation ??= buildCatalog();
  let lastErrors: string[] = [];
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    const bp = BUILDERS[zone](seed, attempt);
    const v = validateBlueprint(bp, catalogForValidation, CONTRACTS[zone], GIVEN_KEYS[zone]);
    if (v.ok) return bp;
    lastErrors = v.errors;
  }
  throw new Error(`Zone "${zone}" failed validation ${MAX_ATTEMPTS} times for seed ${seed}:\n  ${lastErrors.join("\n  ")}`);
}
