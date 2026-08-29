# Junqi — Hybrid Worldgen Plan

Pair with `SYSTEMS.md` (bar), `MISSING-SYSTEMS.md` (what is still out), `LEARNING-SYSTEMS.md` (2020 paths). This file is how we grow the county without copying GameMaker rooms.

**Rule:** read the 2020 rooms for *placement language*. Do not stamp instance x/y from the `.yy` files. Same story every seed. Geometry and clutter roll. Story beats stamp as small pockets.

**Rule:** generate the whole zone, tick only a load ring. Gen and sleep are one system. The county is 2000×1200 cells (16000×9600 px). Do not go back to a 72×48 pocket.

**Rule:** kitchen hatch is the cellar. Yard doors are the mine and the burial. The basement is not the underworld.

**Rule:** Jane, same story every seed. Factory / school / butterfly / pipes are **authored graphs** (show the lock, teach a verb, tell a wall-story). Motifs fill. Do not `carve()` a paragraph.

---

## 1. What 2020 actually placed

Parsed from `Junqi-Legacy-GM/jun7/rooms/*.yy` and instance creation code. Counts are instance families, not “unique designs.”

### 1.1 Rooms as size

| Room | 2020 px | Role |
| --- | --- | --- |
| `room_zone1` | 5120 × 3072 | Overworld. Two camps, fenced town, remote Auntie yard |
| `room_building2_auntie` | 512 × 512 | Kitchen / living room |
| `room_basement_auntie` | 1024 × 1024 | Domestic cellar under the house |
| `room_dungeon_goldmine` | 2048 × 2048 | Gold mine |
| `room_dungeon_burial_chamber` | 2048 × 2048 | Burial temple (densest room) |
| `room_dungeon_butterfly_forest` | 3072 × 2048 | Exists. Not on the story spine. Do not remake to “use” spiders |
| `room_building1` | 1024 × 1024 | Empty test interior. Ignore |

The remake county is 2000 × 1200 cells (16000 × 9600 px at cell 8). That is larger than 2020 zone1 on each axis. Town sits northwest. Auntie sits southeast. An early remake mashed both camps into 72 × 48 — that pocket is gone.

### 1.2 How each room was built

**zone1 (661 instances, 44 object types)**

- Tile layers: `Tiles_Ground`, `Tiles_town_paths`, `Tiles_AboveGround`, `Tiles_over_top`, plus a background.
- Instance layers: `Instances`, `Instances_lighting`, `Instances_map`.
- ~420 fence pieces (`obj_fence1`–`8`). These are lot rings, not unique art.
- House stamps: `obj_tile_house1_7x9` (5), `obj_tile_house4_11x6` (5), `obj_tile_house3_10x12` (2), `obj_tile_house2_10x12_p1/p2`. Each stamp draws from `maps/map1L1.csv` / `map1L2.csv` via `obj_tile_parent` (16×16 tile grid on a surface).
- Minimap icons: `obj_map_road` (23), `obj_map_building` (10), `obj_map_trees` (7).
- Lamp posts (10), trees (~44), plants (~34), walls (65).
- Zone exits are **thin** `obj_change_zone_box` on doors, with `goto_room` + `door_id` + `place_x/y` in creation code.

Do not place 400 fences one by one. Draw a fence ring around a lot.

**Auntie house (39 instances)**

- Twenty bench objects used as furniture (`obj_bench2`–`10`), clustered on the right wall (~328–392, 160–304).
- `obj_fruit_bowl1`.
- Front door → `room_zone1` (`door_id` 4).
- **Two** basement stairs → `room_basement_auntie` (`door_id` 5 and 6).
- No Auntie NPC. No Fireball orb.

**Auntie basement (102 instances)**

- Wall maze, 8 doors, some keyed `BASEMENT`.
- Chest at (232, 696) with `Key_basement_auntie` ×2.
- `obj_rose_growing_zone`.
- Benches as furniture again.
- 11 `obj_reveal_regeon` rects (map fog).
- Stairs back up (two zone boxes).

This is a house undercroft, not the path to the snake.

**Gold mine (598 instances)**

- 216 wall, **134 lights**, **95 barrels**, 41 reveal, 20 doors, 20 undead, 12 chests.
- Keys on doors: `HM`, `M_VAULT`, `MINE_BOSS` (empty string = unlocked).
- Chests scatter Iron / Wood / `Key_generic`. One dump chest: 10 generic keys, 20 wood, 20 iron. Vault key in a chest. Boss key in `obj_boss_chest2`.
- 2 minecarts + tracks / corners / ramp / broken steps + 3 levers + floor triggers (`obj_trigger_floor_0` / `_1`) + `obj_block_here`.
- Exit south (1264, 1932) back to zone1 (`door_id` 2).

Language: carve rooms, then drown them in barrels and torches. Lock three doors.

**Burial chamber (925 instances, 66 types)**

- Four corner `obj_trigger_room_lock_player_in`.
- Snake boss at (160, 160) with `patrol_path = path_snake_boss`.
- Snake Key door (432, 496). Snake Boss Key door (240, 96).
- Lily pads ~ (520–600, 1184–1200). Fireball learn at **(560, 1352)** — in the garden, not the kitchen.
- Pumpkin / cactus patrols (`path_burial_chamber_1/2/3`).
- Spider wing east (~1100–1500): wall spiders, eggs, guards.
- Coffins, snake statues, 16+ chests (apples, stranglethorn, keys).
- Rats + `obj_trigger_rat_path_to_snake` + `obj_trigger_rat_spawn_trigger`.
- Exit back to zone1 (1392, 1776), `door_id` 14.

Language: a cross with four wings. Each wing has a motif. Boss and garden are pockets.

### 1.3 The overworld is two camps

2020 did not make one yard. Town sits northwest. Auntie’s yard sits far southeast. Grass and a road sit between.

| Camp | 2020 anchors (px) | What lives there |
| --- | --- | --- |
| **Town** | Bandits (816, 208) and (880, 208); extra bandit (584, 400); patrol skeleton (992, 576) on `path0`; Auntie house door (672, 984); dump chest (328, 528); abandoned car (200, 1432) | Fenced lots, house stamps, lamps, roads, apple/grape drops, push barrels |
| **Auntie yard** | Dog (4616, 2864); mine door (4656, 2720) / zone box (4656, 2707); burial zone (4504, 2888); butterfly zone (4720, 2864); skeletons (4896, 2984) and (5048, 3032); quest locations (4704, 2752) and (4720, 2848) | Stoop, dungeon mouths, 1–2 undead, barrels |

A second dog instance at (7200, 4224) is **outside** the 5120×3072 room. Leftover. Do not remake it.

An early remake mashed both camps into one 72×48 box with the house in the middle. That is why it read as a combat sandbox. The live county is two camps plus six towns, a river, and the later mouths.

The town dump chest (`Gold Bar` ×4, `Rock` ×4, `Small Water` ×16, every herb ×4) is a designer give-all. Keep that as a terminal command, not a generated loot table.

---

## 2. Grammar: motif / pocket / connector

| Piece | Job | Seed may | Seed may not |
| --- | --- | --- | --- |
| **Motif** | A painter, not a room. Fence ring, barrel pile, torch every N cells, door grid, lily pool, coffin row, house stamp from a size table | Count, jitter, which variant | Story prop ids |
| **Pocket** | 8–16 cells. Authored recipe or a tight suggestion. Fixed ids (`dog`, `house_door`, `mine_vault`) | Rotate, offset ±2 cells | Omit the pocket or rename the id |
| **Connector** | The ~95%. Road wiggle, lot shapes, extra dungeon rooms, tunnels | Shape, length, extra rooms | Drop a pocket if the connector fails — retry the seed |

`beats.json` already invalidates a seed if a required beat object is missing. Pockets are how those objects get into the world.

---

## 3. Story pockets

Handmade, or a tight suggestion the generator must satisfy. 8–16 cells.

| Pocket | Must contain | May jitter | Do not randomize |
| --- | --- | --- | --- |
| **Auntie stoop** | `dog`, locked house door (`opens: auntie_house`), 2 barrels | Facing, offset ±2 | Dog id, door tag |
| **Kitchen** | Fruit, craft bench, **two** hatches on one wall | Which wall, leftover furniture | Hatch destinations (cellar, not burial) |
| **Cellar key room** | 1 keyed door (`auntie_basement`), 1 chest (basement keys), rose patch | Room size 8–12 | Key tag |
| **Mine mouth** | Exit to county, lamp, wreck | Left/right of door | `toZone: county` |
| **Mine locks** | HM door, vault door, boss door, vault chest | Order along the spine | Which key opens which |
| **Burial garden** | 2 lily pads, Fireball orb (`learn: fireball_0`), 1 plant | Pool shape | The learn id |
| **Snake gate** | Snake-key door, lock trigger, empty arena 10–14 cells | Arena size | Boss id + lock-in |

2020 Fireball is in the burial garden. The kitchen orb is Icebolt (Jane's first crack). Do not put Fireball in the kitchen.

Butterfly forest is on the spine after the snake. Grow is the verb. Do not remake it as a spider wing.

---

## 4. Per-zone fill

### County

**2020 language:** fenced lots, house stamps, two camps, door-thin exits.

**Generate (`generateTownYard.ts`)**

1. Town NW `(180, 160)`. Auntie SE `(1780, 1040)`. Roads wiggle between them and the six towns.
2. River stroke. Biomes from `biomes.ts` (cold at night / mountain / burial).
3. Around Cross: lots, fence rings, house stamps from the size table (`7×9`, `11×6`, `10×12`).
4. `stampTowns` — Cross, Rivermill, Acreton, Kiln End, Boneford, Ridgegate (`towns.json`).
5. `stampTrials` / `stampCamps` / `stampWaysides` — stone rings, restock fires, road clutter.
6. Bandits in pairs on the road. Yard undead. Rim wildlife from `enemies-more.json`.
7. Herb scatter from a kitchen table. Not the dump chest.

**Keep authored (required pockets):** stoop, mine mouth, burial mouth, factory / school / butterfly fronts.

**Soft pockets (may skip):** train, shop, pipes grate, graveyard, farm, abandoned, picnic, car wreck. A skip is silent — do not treat a missing grate as a failed seed.

### House

**2020 language:** furniture-dense, fruit, two stairs down.

**Generate:** one box 16–22 cells. Stamp leftover benches as wall furniture. One fruit bowl.

**Keep authored:** craft bench, two hatches to cellar, front door to county. No Fireball.

### Cellar (`generateCellar`, `ZoneId` `dungeon`)

**2020 language:** domestic, keyed inners, roses.

**Generate:** 2–3 rooms off a hall. Scatter barrels. One rose patch.

**Keep authored:** stairs up, basement-key door + chest.

**Drop from this zone:** snake key, burial door, “relic.” Those belong in the yard / burial.

Generator is `generateCellar`. `ZoneId` stays `dungeon` so slot 0 does not break.

### Mine

**2020 language:** lights + barrels + locked doors + carts.

**Generate:** 5–8 carved rooms. Torch motif on walls. Barrel piles in corners. 4–6 iron/wood chests from a table. Optional one cart on a 6-cell track (machine row — see `MISSING-SYSTEMS.md`).

**Keep authored:** three keyed doors + vault/boss chests + exit to county.

### Burial

**2020 language:** four wings, keys, garden, snake, spiders.

**Generate:** cross hallway. Assign wings from a table: snake / garden / spider / coffin. Fill each from a motif + one patrol path.

**Keep authored:** garden pocket, snake-key door, boss-key door, lock-ins, snake boss.

---

## 5. Topology (do this before more random rooms)

```
county
  Cross --roads-- Rivermill / Acreton / Kiln End / Boneford / Ridgegate
  auntie yard
    ├── house door → kitchen
    │                 └── hatch ×2 → cellar (domestic)
    ├── mine door → mine (HM / vault / boss)
    ├── burial door → burial (garden, wings, snake)
    ├── factory door → factory (lever, lock-in, Repair → pipes)
    ├── school door → school (desks, detention, bell)
    └── butterfly mouth → forest (Grow, north fight)
```

Neighbour interiors (abandoned / museum / graveyard) sit on soft pockets. Kitchen hatch = cellar. Yard doors = mine and burial. Factory / school / forest are the second map after the snake, not a second spine.

Zone boxes stay **thin** and sit on the door, with a paired `door_id` / entry mark so you come out in front of the door, not in a wall.

---

## 6. Distance unload (load ring)

2020: `objects/distance_unload/`. Spawned by `obj_room_init`. Ticks in `game` Step **before** units (`event_user(0)`).

### 6.1 What shipped

```
block_size            = path_cell * 2        // 16 px
dynamic_load_distance = 24 * block_size      // 384 px ≈ 24 m
dynamic_load_size     = distance * 2         // 768 px square
```

When the player’s `x div block_size` or `y div block_size` changes:

1. `instance_deactivate` dynamics, statics, usables.
2. Deactivate **idle** AI only (`controller == ai && combat_state == idle`).
3. `instance_activate_region` the square around the player.
4. Store the new block.

Combat AI stays awake even if far. That is the MMO rule: a fight does not freeze because you stepped one cell.

Save, load, and zone change call `instance_activate_all` first. `save_room` dirties `player_block_prev_x = -1` so the ring rebuilds next tick. F5 also activates all.

A **256 px chunk index** was written in Create and commented out. The live ring is what actually ran. Steal the ring, not `instance_deactivate`.

Debug Draw End draws the rectangle.

### 6.2 Remake contract

| Rule | Do this |
| --- | --- |
| When | Player block changes. Do not test every actor every frame |
| Sleep | Skip AI tick, hide sprite, drop occupy. Keep the spec in the zone blueprint |
| Stay awake | Player, locked target, anyone in combat, carried prop, live bolts, current speaker, the pocket you are standing in |
| Tiles | Chunk to the same ring. One Phaser render-texture will not survive a 5120×3072 county |
| Save | Wake all, write live HP/xy for the **whole** zone, then sleep again. Never persist only the ring |
| Pockets | Ids stay in data while sprites are down. `beats.json` still sees them |

Ring size: ~24 metres. Camps on the 2000×1200 county are far enough that town sleeps in Auntie’s yard.

Status in `SYSTEMS.md`: **IN**.

---

## 7. What the remake does today

| File | Now |
| --- | --- |
| `generateTownYard.ts` | 2000×1200 county. Town NW, Auntie SE, river, biomes, required + soft pockets |
| `generateTowns.ts` | Six towns from `towns.json`. Boards and doorstep NPCs from `npcs.json` |
| `generateTrials.ts` | Stone rings: push, lever, clear, drain, Grow |
| `generateWaysides.ts` | Road clutter on the wiggle strokes |
| `generateHouse.ts` | Kitchen pocket, two hatches to cellar, Icebolt orb, Julie's note. No Fireball |
| `generateCellar.ts` | Domestic 2–3 rooms, basement key, no snake key / burial door (`ZoneId` still `dungeon`) |
| `generateMine.ts` | 5–8 rooms, torch/barrel fill, HM / vault / boss locks, rats, patrol |
| `generateBurial.ts` | Cross + four wings, garden Fireball, snake gate, exit to county |
| `generatePlaces.ts` | Abandoned house, museum (keyed MAGIC wing), graveyard |
| `generateDungeons.ts` | Factory / school / butterfly / pipes. Authored graphs. Toggle, clear, Grow, drain, Repair |
| Survival | `survive.ts`. Hunger / warmth on food and stones. Night, mountain, burial run cold. Campfires |
| Fog | Interiors stamp a saved bitmap. County is live radar |
| Tiles / sleep | Chunked to the load ring. Idle/far sleep on block change |

---

## 8. Data shape (do not invent a class per pocket)

Suggested, still rows:

```
pockets.json   id, zone, size, tiles[], props[], enemies[], facing
motifs.ts      fenceRing, barrelPile, torchGrid, houseStamp, lilyPool, coffinRow
triggers.json  onEnter → action list   (spawn, lock, light, aggro)  — see MISSING-SYSTEMS.md
```

A designer adds a pocket row. `PlayScene` does not grow a `if (orb)` branch.

House stamp sizes from 2020, as a table: `7×9`, `11×6`, `10×12`. Paint from a small tile atlas or generated brick/plank, not from the CSV port.

---

## 9. Build order

| Step | Ship | Proof |
| --- | --- | --- |
| 1 | Pocket stamper + motif painters | **Done.** Same pocket ids survive two seeds |
| 1b | Load ring (~24 m) | **Done.** Town sleeps in Auntie yard |
| 2 | County: two camps + road + stoop / mine / burial | **Done.** Then grown to 2000×1200 + six towns |
| 3 | Kitchen + cellar split | **Done.** Icebolt in the kitchen. Hatch ≠ burial |
| 4 | Mine fill + three keyed doors | **Done** |
| 5 | Burial cross + garden + snake gate | **Done.** Fireball at the lilies |
| 6 | Factory / school / butterfly / pipes | **Done.** One verb each. `still_turning` is the dog hand-in after all three mouths, not the first door |
| 7 | Towns, trials, camps, hunger / warmth | **Done.** Thin coat, not JaneCraft |

Still open: cart, zone floors, subtract lighting, audio, snake sine-body. Do not invent a seventh dungeon to exercise a leftover.

---

## 10. Do not

- Copy `.yy` instance lists into TypeScript.
- Port butterfly forest to “use” spiders. Grow is the verb. The north fight is a lock-in, not a wing.
- Generate the town dump chest as normal loot.
- Route burial through the kitchen hatch.
- Keep one render-texture as the county grows.
- Persist only the awake ring.
- Add a `class Kitchen extends Zone`.

---

## Source

- Rooms: `Junqi-Legacy-GM/jun7/rooms/room_zone1`, `room_building2_auntie`, `room_basement_auntie`, `room_dungeon_goldmine`, `room_dungeon_burial_chamber`.
- Unload: `objects/distance_unload/Create_0.gml`, `Other_10.gml`; `game/Step_0.gml`; `save_room.gml`; `obj_change_zone_box`.
- House stamps: `objects/obj_tile_parent/Draw_0.gml`, `maps/map1L1.csv`, `map1L2.csv`.
- Zone pairing: instance creation `goto_room` / `door_id` on change-zone boxes.
- Remake today: `junqi/src/game/world/generate*.ts`.
