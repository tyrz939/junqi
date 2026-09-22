// The Castle Arms, on the square in Castle: the one door on the high street that opens.
// A tap room, a bar, a fire, two regulars and the landlady, and a bed in the room at the back.
// Built in world/interiors.ts beside Julie's house; registered here so the county grows the
// door the moment the room exists.

import { buildArms } from "@/world/interiors";
import type { ZoneDef } from "@/world/registry";

export const zone: ZoneDef = {
  id: "arms",
  build: buildArms,
  contract: { units: ["mrs_garland"], props: ["exit_door", "arms_bed"], marks: ["entry"], rects: ["tap_room"] },
};
