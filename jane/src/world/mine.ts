// Gnox Goldskin's gold mine. The mission follows 2020's room_dungeon_goldmine as
// reconstructed from its doors, chests and levers, and the hand-drawn map beside it
// ("Headmaster", "Iron Knuckles", Vault Key, "x2 Wood" repair gate):
//
//   entry --- plate room (hold the plate down with a barrel -> generic key)
//     |  \--- guard room, behind generic gate A (the clerk carries the HM key)
//   store (generic key + iron)
//     |  generic gate B
//   core ---- HM gate ---- Headmaster's office (he drops the vault key; Repair is in his drawer)
//     |  \--- broken steps (Repair, 2 wood) --- gallery (ornate chest: boss key)
//     |  \--- first aid room (a stove: rest)     |  \--- broken track (Repair, 4 iron) --- plate room
//   boss gate                                    \--- cage (side: two plates, a page)
//   arena: Iron Knuckles, four hoists --- nook (the big jar) --- gallery, once he is rust
//   office --- vault gate --- vault (gold, the museum key)
//
// That mission is data now (data/dungeons/mine.json) and its rooms are templates
// (world/dungeon/rooms/mine/*.room). The space is generated per seed and proven before anyone
// walks into it (world/dungeon/). This file is only the mine's place in the zone list.
//
// "Language: carve rooms, then drown them in barrels and torches. Lock three doors."

import { dungeonZone } from "@/world/dungeon";

export const MINE = dungeonZone("mine");

export const buildMine = MINE.build;
