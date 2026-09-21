// The Burial Chamber. "Theme: Cold, dark, foggy place. lots of death in here."
//
// One idea: COLD LIGHT SHOWS WHAT IS THERE, WARM LIGHT KEEPS IT OFF. The blue torches are
// cold (`light.cold`): they answer frost and show what was hidden. Braziers and the great
// torch are warm: the dead of this place (`shade`, `shunsLight`) will not step into them.
//
//   hall     the square hall: four passages, four flames, the seal, and the fire off it
//   west     the cold alcove, the rat room, the scaled door, the lock-in, the snake (built)
//   east     the lurkers that are fed not fought, the garden, the burnt scroll: FIRE
//   south    the dark hall and the great torch, then the parade ground and the Soldier
//   north    the webs, then the nursery and the Spider
//   back     the seal turns on four names, and Goldskin is behind it. Then a stair that
//            goes UP, into the School's boiler room.
//
// The mission is data now (data/dungeons/burial.json), its rooms are templates
// (world/dungeon/rooms/burial/*.room) and the space is generated per seed and proven before
// anyone walks into it. What was hand-built here in September (the cold torches, the lock-in
// and its chest, the snake's clock, the bait, the root wall and the scroll) is the same
// content, re-hosted on the generator at phase 5 and bound to the same names.
//
// Two lines are not the generator's: the story's own trigger rows for this zone
// (data/triggers.json) name a rect `everywhere`, which the generator calls `<zone>_all`.
// The zone file aliases it rather than edit a base data file.

import type { Blueprint } from "@/world/blueprint";
import { dungeonZone } from "@/world/dungeon";
import { hasZone } from "@/world/registry";

const d = dungeonZone("burial");

export const BURIAL = {
  ...d,
  contract: { ...d.contract, rects: [...d.contract.rects, "everywhere"] },
  build: (seed: number, attempt: number): Blueprint => {
    const bp = d.build(seed, attempt);
    bp.rects.everywhere = { cx: 0, cy: 0, w: bp.w, h: bp.h };
    // The stair goes up into the School's boiler room. Until the School is built it goes nowhere:
    // it is a locked door with no keyhole either way, and a door into a zone that does not exist
    // is a crash rather than a mystery.
    if (!hasZone("school")) {
      const stair = bp.props.find((p) => p.key === "stair_up");
      if (stair) delete stair.to;
    }
    return bp;
  },
};

export const buildBurial = BURIAL.build;
