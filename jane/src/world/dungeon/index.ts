// Generated dungeons, by id. A dungeon is a mission in data/dungeons/<id>.json, room
// templates in rooms/<id>/*.room, and nothing else: `dungeonZone("museum")` is everything
// world/index.ts (or a file in world/zones/) needs to register it.
//
// data/dungeons is not a catalog table: the sim never reads a mission, only the blueprint
// built from it.

import type { Catalog } from "@/sim/catalog";
import type { Blueprint, ZoneContract } from "@/world/blueprint";
import { checkDungeon, contractOf } from "@/world/dungeon/checks";
import { buildDungeon } from "@/world/dungeon/generate";
import type { DungeonDef } from "@/world/dungeon/types";

const FILES = import.meta.glob("../../data/dungeons/*.json", { eager: true, import: "default" }) as Record<string, DungeonDef>;

export const DUNGEONS: Record<string, DungeonDef> = {};
for (const path of Object.keys(FILES).sort()) {
  const def = FILES[path];
  if (DUNGEONS[def.id]) throw new Error(`Dungeon "${def.id}" is defined twice (again in ${path})`);
  DUNGEONS[def.id] = def;
}

export type DungeonZone = {
  id: string;
  build: (seed: number, attempt: number) => Blueprint;
  /** Derived from the mission's binds: every name the story may lean on. */
  contract: ZoneContract;
  givenKeys: string[];
  /** Spells the player is known to have at the door. The solver holds the dungeon to it. */
  givenVerbs: string[];
  /** Checks C1 to C12. Empty means the blueprint is the dungeon that was designed. */
  check: (bp: Blueprint, catalog: Catalog) => string[];
};

export function dungeonZone(id: string): DungeonZone {
  const def = DUNGEONS[id];
  if (!def) throw new Error(`No dungeon "${id}" in data/dungeons`);
  return {
    id,
    build: (seed, attempt) => buildDungeon(def, seed, attempt),
    contract: contractOf(def),
    givenKeys: def.givenKeys,
    givenVerbs: def.givenVerbs,
    check: checkDungeon,
  };
}
