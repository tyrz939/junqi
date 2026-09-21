// The Castle Pipes. Brick culvert, laid by the company to carry water down from the hill
// and something else back up. Phase 3, and the approach to the Factory rather than a
// dungeon of its own (DUNGEONS.md 3.4): no verb, no boss, and its reward is a road.
//
// One valve, two runs. The east run is dry as she finds it and the west run is under water;
// turn the valve in the valve house and the two swap, for good and back again. So one
// penstock stands open in each state and never both, which is the Museum's breaker in a
// smaller building.
//
//   sump (the grate down from the town) --- north run (the first manhole) --- junction
//   junction (hub: the plate, the barrel, the jar in the niche) --- chamber (rest, a brazier)
//   junction --- valve house (THE VALVE)
//   junction -- east valve dry --> east run (a manhole, a jar on a shelf that floods)
//   junction -- east valve flooded --> west run (a manhole, the outfall key)
//   west run -- outfall key --> outfall (the gear, a page, the Works yard manhole)
//   outfall -- the gear --> chamber, a grating that stays up: the short way back
//
// Every ladder Repaired from below (iron x1) shows a manhole that stays open, so the pipes
// become a covered road between the town, the Museum's stair and the Works yard in a county
// with no fast travel. The four manholes lead to county marks `manhole_1` to `manhole_4`.

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/index";

const d = dungeonZone("pipes");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  states: d.states,
  check: d.check,
};
