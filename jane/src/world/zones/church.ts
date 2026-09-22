// St Anne's, on the square in Castle. Pews, the altar with its two candles that do not burn
// down, the visitors' book with Julie's hand in it, and the vicar, who is always in.
// Built in world/interiors.ts; registered here.

import { buildChurch } from "@/world/interiors";
import type { ZoneDef } from "@/world/registry";

export const zone: ZoneDef = {
  id: "church",
  build: buildChurch,
  contract: { units: ["vicar"], props: ["exit_door", "visitors_book"], marks: ["entry"], rects: ["nave"] },
};
