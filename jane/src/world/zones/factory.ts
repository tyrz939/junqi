// Goldskin Works. Semi-abandoned, which is how the council put it. The shift never clocked
// off, and it is dark inside because the foreman had the lamps taken out, run by run, and
// wrote down why (DUNGEONS.md 3.4).
//
// The one idea: LIGHT IS HOW THE MACHINES SEE YOU. Sentries and haulers have `sight: "lit"`,
// so they only notice what stands in a lamp's pool. The building is crossed twice. First in
// the dark, safely, down the vent to the generator hall; the Charge Hand carries a lamp, so
// in that hall he is the only lit thing, and the one dormant sentry will fire at him. Then
// the generator is Repaired and Sparked, every lamp in the building comes on at once, and the
// way back is the same halls awake. Electric is how she takes it back: a spark into a fuse box
// kills that circuit for good, and the shutters on it die with it. Dark is safe and shut. Lit
// is open and watched.
//
//   yard --- loading (one lamp still burning, one sentry: the lesson) --- lockers (two keys)
//   loading -- plain key --> No. 1 line (hub: a hauler on patrol, the fuse box, in the dark)
//   line --- time office (rest) ; line --- vent (the drop into the generator hall)
//   generator (the Charge Hand, the orb: SPARK, the generator, the starting board)
//   generator --- gen shutter (a dead socket: the first spark, in safety) --> back to the line
//   line -- plain key --> press hall (haulers, a call box, a patched wall) --> the office
//   press hall -- powered shutter --> stores        office -- his key, lock-in --> assembly
//   assembly (the Foreman) --> the roller door (big jar, the Glasshouse key, the way to the yard)
//
// `factory_lit` is the state and the starting board is its control, so `buildZone` solves the
// building dark and lit at once (ENGINE.md 8.3, the stateful flood).

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/index";

const d = dungeonZone("factory");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  states: d.states,
  check: d.check,
};
