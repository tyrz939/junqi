// Butterfly Forest (DUNGEONS.md 3.3). Outdoors, hedges for walls, and no keys: light is the
// lock and a living thing is the key. Grow does three things here, and one of them takes a way
// away instead of opening one.
//
//   gate -- first glade (bud 1) -- the hut (the Collector, the net) -- the ring
//     \______ a dry bank, grown, is the short way back from the hut ______/
//   ring: the hearth (rest), the rock (Explosion), the runner, the island, the stone
//   island -- the guarded glade;  runner -- the hollow;  stone -- the reward
//   two glades are left for a later visit, behind Fire and behind Electric
//
// Every hedge seed in the zone can be grown shut and the dungeon is still finishable: that is
// proven seed by seed in test/forest.test.ts, because the generator cannot say it.

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/registry";

const d = dungeonZone("forest");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  check: d.check,
};
