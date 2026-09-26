// The Museum. Limestone, four wings off a round hall, opening hours ten to four.
//
// One breaker, two buildings (DUNGEONS.md 3.2). With the lights on it is a museum: the
// exhibits are solid props on their plinths and one of them stands in the door of the stores.
// With them off the plinths are empty, the exhibits walk, and the power shutters come down.
// So some doors open only in the light, some only in the dark, and the stores opens in
// neither: Explosion is the answer to that one, and a lit exhibit blown up is gone for good.
//
//   entry --- atrium (hub: the arch, ARTS, SCIENCE, the cloakroom)
//     \--- history (the boat: conveniences key) --- toilets (maintenance key; a damaged door)
//                                                \--- natural history (two plinths, a jar)
//   atrium --- cloakroom (rest) --- maintenance (THE BREAKER)
//   atrium -- lights dark --> arts (the floor key, on an empty plinth)
//   atrium -- floor key --> science (the Shot-Firer, the page, a cracked case)
//   science -- lights lit AND Explosion --> stores (the Attendant's key)
//   atrium -- Explosion through the arch, then his key --> the rotunda (lock-in)
//   rotunda --> the case that is not on the plan, and the fire door back to the cloakroom
//
// The mission is data (data/dungeons/museum.json) and its rooms are templates
// (world/dungeon/rooms/museum/*.room). `states` is what makes buildZone solve it in both
// buildings at once (ENGINE.md 8.3, the stateful flood).

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/index";

const d = dungeonZone("museum");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  states: d.states,
  check: d.check,
};
