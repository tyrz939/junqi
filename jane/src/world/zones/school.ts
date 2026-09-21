// Castle School, on the crown of the hill, with one window lit (DUNGEONS.md 3.6). The last
// dungeon, and it asks for all six verbs at once: it teaches nothing and forgives nothing.
//
// The building follows the timetable. The bell rope in the hall is the state `period`
// (lessons / break): in lesson time the classroom doors stand open and there is a master
// walking in each corridor; at break those doors shut, three others open, and the corridors
// are empty. That is one mechanism (ENGINE.md 8.3, the stateful flood) and it is the whole hub.
//
// Night is the other axis and it is NOT a state flag: it is the clock. The sick bay has two
// beds. The near one sleeps to six, as beds do. The far one is labelled YOU WILL BE WOKEN AT
// THE BELL and sleeps to twenty-one hundred, so the bed is the lever that makes it night, and
// because a bed only turns the clock when the whole party is resting, it is a regroup beat in
// co-op for nothing. The Caretaker walks the corridors at night only.
//
//   boiler (the stair up out of the Burial; the yard door) --- hall (THE BELL ROPE, the timetable)
//   hall --- sick bay (rest: two beds)
//   hall --- the long corridor --- Woodwork (lessons) / Chemistry (lessons) / Physics (break)
//                              \-- lost property (break)
//   hall --- the top corridor  --- Botany (lessons) / Domestic Science (break) / the ice house (break)
//                              \-- the staff room
//   six stopped clocks AND the Caretaker's key, rung on the rope --> the tower (lock-in)
//   the tower --- the top room: the register, the big jar, and the back stair to the sick bay
//
// The mission is data (data/dungeons/school.json), its rooms are templates
// (world/dungeon/rooms/school/*.room), and `states` is what makes buildZone solve it in both
// buildings at once. The children are never shown anywhere in it: coats, desks, chalk and a
// register, and that is all (STORY.md 7.5).

import { dungeonZone } from "@/world/dungeon";
import type { ZoneDef } from "@/world/index";

const d = dungeonZone("school");

export const zone: ZoneDef = {
  id: d.id,
  build: d.build,
  contract: d.contract,
  givenKeys: d.givenKeys,
  givenVerbs: d.givenVerbs,
  states: d.states,
  check: d.check,
};
