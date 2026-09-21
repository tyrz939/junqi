// The ruined library. Four rooms, nothing in them that is alive, and the only page left in
// the book (PLAN.md; DUNGEONS.md 3.3). It is where Grow is learned, and it says so about
// itself in one line: nothing comes up in the dark. The planter under the hole in the roof
// is the first thing to try it on, in the same room, with the light already on it.
//
//   entry ---- stacks (the plate under the library steps: a pressed page) ---- nook (a fire)
//     \                  \
//      \------------------ shelf: the last book, the dry planter, a jar behind the vine
//
// The way back from the shelf to the entry is the short way round, which is the loop.

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/registry";

const d = dungeonZone("library");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  check: d.check,
};
