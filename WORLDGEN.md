# Jane — Hybrid Worldgen

Pair with `SYSTEMS.md` (bar), `ENGINE.md` §8 (how the builders and the solver work), `DESIGN-2020.md` §4 (what 2020 *drew*), `LEARNING-SYSTEMS.md` (2020 paths). This file is how zones are grown without copying GameMaker rooms: what 2020 *placed*, the grammar taken from it, and the contract each live zone keeps.

**Rule:** read the 2020 rooms for *placement language*. Do not stamp instance x/y from the `.yy` files. Same story every seed. Geometry and clutter roll. Story objects are fixed **names** at jittered positions.

**Rule:** the whole zone exists in state; only a load ring thinks. Today's county is 640 × 384 cells (5120 × 3072 px), the size of 2020's `room_zone1`.

**Direction (September 2026, `PLAN.md`):** the county grows to about 3,600 × 2,000 cells, a ten-minute walk across, generated from a skeleton of required sites at required distances. An earlier version of this file said "do not grow the county". That was aimed at the 2026 Phaser build, which made a 2000 × 1200 map and then invented content to fill it, with no structure and no checks. **Size was never the fault. Filling space without a plan was.** The plan is `PLAN.md` §2: constraints as rows, a density budget that is tested, a seed viewer. §4 below describes the zones as they stand until milestone M2 replaces the county builder.

**Rule:** kitchen hatches are the cellar. Yard doors are the mine and the burial. The basement is not the underworld.

**Rule:** every candidate goes through the lock-and-key solver (`jane/src/world/validate.ts`). A failure is re-rolled, never shown to the player, never thrown.

§1 was audited against the room files in September 2026; corrections are marked *(corrected)*.

---

## 1. What 2020 actually placed

Parsed from `Junqi-Legacy-GM/jun7/rooms/*.yy` and instance creation code. Counts are instance families, not unique designs.

### 1.1 Rooms as size

| Room | 2020 px | Role |
| --- | --- | --- |
| `room_zone1` | 5120 × 3072 | Overworld. Two camps, fenced town, remote Auntie yard |
| `room_building2_auntie` | 512 × 512 | Kitchen / living room |
| `room_basement_auntie` | 1024 × 1024 | Domestic cellar under the house |
| `room_dungeon_goldmine` | 2048 × 2048 | Gold mine |
| `room_dungeon_burial_chamber` | 2048 × 2048 | Burial temple (densest room) |
| `room_dungeon_butterfly_forest` | 3072 × 2048 | *(Corrected)* **An empty shell**: a fully filled floor layer, an empty wall layer, four empty instance layers, `obj_room_init` and one exit back to zone1 (`door_id` 10) |
| `room_building1` | 1024 × 1024 | *(Corrected)* not empty: 10 walls, a door, an empty chest, an exit with `door_id` 3. No zone1 box has id 3, so the room is orphaned |

### 1.2 How each room was built

**zone1 (661 instances, 44 object types)**

- Tile layers `Tiles_Ground`, `Tiles_town_paths`, `Tiles_AboveGround`, `Tiles_over_top`, plus a background. Instance layers `Instances`, `Instances_lighting` (holds nothing), `Instances_map`.
- 420 fence pieces (`obj_fence1`–`8`). Lot rings, not unique art.
- House stamps: `obj_tile_house1_7x9` ×5, `house4_11x6` ×5, `house3_10x12` ×2, `house2_10x12_p1/p2`, and *(missed before)* **`house5_p1/p2/p3`** at (544–704, 992–1040): that is the Auntie house the (672, 984) zone box sits on. Stamps draw from `datafiles/maps/map1L1.csv` / `map1L2.csv` via `obj_tile_parent`.
- Minimap icons `obj_map_road` ×23, `obj_map_building` ×10, `obj_map_trees` ×7. Lamp posts ×10, trees 44, plants 34, walls 65.
- Zone exits are **thin** `obj_change_zone_box` on doors, with `goto_room` + `door_id` + `place_x/y` in creation code.
- *(Corrected)* **Nothing in the overworld is gated.** Every dungeon mouth and the town house are open. The house door is unlocked; `Key_auntie_house`, found in `obj_abandoned_car1`, is consumed with no effect.

**Auntie house (39 instances)** — twenty bench objects as furniture (`obj_bench2`–`10`) on the right wall, `obj_fruit_bowl1`, front door → zone1 (`door_id` 4), **two** basement stairs (`door_id` 5 and 6). No NPC. No orb.

**Auntie basement (102 instances)** — wall maze, 8 doors, two keyed `BASEMENT`. Chest at (232, 696) with `Key_basement_auntie` ×2. 11 reveal rects. Two stairs back up. *(Corrected)* `obj_rose_growing_zone` **does nothing**: no events, no parent, no references; it is an inert 40 × 32 rect. No enemies.

**Gold mine (598 instances)** — 216 wall, 134 lights (84 large, 50 small), 95 barrels, 41 reveal rects, 20 doors, 20 undead (13 skeletons, 7 bats), 12 chests. 2 minecarts + tracks + 3 levers + floor triggers + `obj_block_here`. Exit south (1264, 1932), `door_id` 2.

- *(Corrected)* Door key strings: `HM`, `M_VAULT`, `MINE_BOSS`, and **two doors with `key = ""` and `locked = true`, which means "opened by the generic key"**, not "unlocked". The unlocked default is the string `"NOT LOCKED"`. The burial has three such doors.
- *(Corrected)* "Floors" are cosmetic. `current_floor` is written in five places and **read nowhere**; the only effect of "floor 2" is the reveal colour. The mine does not overlap itself.
- The town dump chest at (912, 1936) in the mine is outside the walls and unreachable.
- The `MINE_BOSS` arena is empty: **no boss was ever placed**.

**Burial chamber (925 instances, 66 types)** — four corner `obj_trigger_room_lock_player_in`, snake boss at (160, 160) on `path_snake_boss` (a nine-point loop round four `block_here` pillars), Snake Key door (432, 496), Snake Boss Key door (240, 96), lily pads ~(520–600, 1184–1200), Fireball learn at **(560, 1352)**, spider wing east, 16+ chests, rats with spawn and path triggers, exit (1392, 1776) `door_id` 14. 107 `obj_undead_root`. 19 reveal rects *(corrected from "dozens")*.

- *(Corrected)* Only three **pumpkins** patrol (`path_burial_chamber_1/2/3`). Cacti are static. "Coffins" is two objects. The "snake statues" are four stationary poison-bolt units.
- *(Corrected)* The corner lock triggers are timed (a 900-frame alarm reset while you stand inside) and never released by a boss dying. `obj_boss_room_trigger` has only a Create event.

### 1.3 The overworld is two camps

| Camp | 2020 anchors (px) | What lives there |
| --- | --- | --- |
| **Town** (north-west) | "Bandits" (816, 208), (880, 208), (584, 400); patrol skeleton (992, 576) on `path0`; house door (672, 984); dump chest (328, 528); abandoned car (200, 1432) | Fenced lots, house stamps, lamps, roads, apple/grape drops, push barrels |
| **Auntie yard** (south-east) | Player start (4528, 2888) beside the dog (4616, 2864); mine door (4656, 2720); burial box (4504, 2888); butterfly box (4720, 2864); skeletons (4896, 2984), (5048, 3032); quest locations (4704, 2752), (4720, 2848) | Stoop, dungeon mouths, 1–2 undead, barrels |

A second dog at (7200, 4224), scale 0.2, is outside the room. Leftover.

The dump chest (`Gold Bar` ×4, `Rock` ×4, `Small Water` ×16, every herb ×4) is a designer give-all. It is a console command now (`give`), not loot.

The 2020 **town map** (`Sunshine.PNG`) puts Auntie's house in the west near the entrance and the mine far south-east; the room did not follow it. The live county follows the room. `DESIGN-2020.md` §4.8.

### 1.4 Critical paths, reconstructed

What a player actually had to do in 2020, from door keys, chest contents, plates and triggers. These are the placement language for the live zones.

**Basement** — stair 5 → SW room: chest (2 BASEMENT keys), 3 barrels → BASEMENT door (192, 544) → corridor with four open cells north (one furnished), the rose rect, an east leg → BASEMENT door (672, 624) → SE corridor → stair 6. A loop. Either iron door closes it.

**Mine**
1. South door, entry hall.
2. West room: chest (1096, 1760) with a generic key, **unlocked only while the plate at (1072, 1808) is held**. Push a barrel onto it.
3. North room chest: generic key + 4 Iron.
4. Generic door east → SE room: the skeleton at (1656, 1880) carries the **HM key**.
5. Generic door north → the core → cart room → the HM door (1680, 528): chest with the **vault key**.
6. **Repair** the steps (1048, 1736) with 2 Wood → cart station; a lever switches the track toward `boss_chest2` (488, 1848): the **boss key**. (The corridor is also walkable.)
7. Cart 2, levers, **Repair** the broken track with 4 Iron, ride west.
8. West: a generic-key chest, the `M_VAULT` door (the vault holds 1 Iron), the `MINE_BOSS` door onto an empty arena. No return route was found.

**Burial**
1. Arrival is *in the miniboss room*: a 1500 HP miniboss and three soldiers, all doors open.
2. Rat room → `block_here` maze → chest (568, 848): the **Snake Key**.
3. Snake Key door → `obj_trigger_lock_in_room1` locks the chest and the door and spawns four undead; kill them and `boss_chest2` gives the **Snake Boss Key**.
4. The boss room (NW) two ways: the keyed door from the north corridor, **or** with no key at all through the south door, reached by a generic door a plate also opens.
5. Fireball nook: south of a generic door, **or** shoot the flower at (568, 1416) from the south garden and its root wall dies.
6. The other corners (spider boss NE, undead boss SE, boss flower SW) are ungated.

---

## 2. Grammar: motif / pocket / connector

| Piece | Job | Seed may | Seed may not |
| --- | --- | --- | --- |
| **Motif** | A painter, not a room. Fence ring, barrel pile, torch run, house stamp, pillar maze, tree blob | Count, jitter, which variant | Touch a story key |
| **Pocket** | Authored ground with fixed keys (`dog`, `house_door`, `gate_boss`) | Offset a few cells, mirror a doorway | Omit the pocket or rename a key |
| **Connector** | Roads, corridors, clutter, wildlife | Shape, length | Cut a pocket off. The solver re-rolls if it does |

In code: `Kit` (`world/kit.ts`) is the painter. `k.claim()` marks authored ground so scatter never lands on it (no tree on the stoop). `k.spot()` finds open unclaimed footprints. One RNG stream per `(seed, zone, attempt)`, and inside the county one per step: `k.within(step, fn)` hands `fn` the dice of `(seed, zone, step, attempt)` (a road's beat, a lattice point, a macro cell's wildlife, a place's furnishings by kind and cell, a placement row by key), and the county's anonymous things are keyed by their cell, not by a count. Re-tuning one step moves only that step's output; `test/streams.test.ts` proves it. The skeleton does the same with `stepDice(seed, step, attempt)` and rank-by-hash picks (`ranked`), so ruling out one candidate (the rail took it) only moves a pick that was that candidate.

---

## 3. The contract

Every builder returns a `Blueprint`: tiles, unit and prop spawn rows, and **named** `marks` (arrival points) and `rects` (trigger areas, camera locks). Nothing outside the builder uses a coordinate.

`world/index.ts` lists, per zone, the names every seed must contain (`CONTRACTS`) and the keys the story hands over from outside (`GIVEN_KEYS`: the dog's house key).

| Zone | Size (cells) | Required units | Required props | Marks | Rects |
| --- | --- | --- | --- | --- | --- |
| `county` "Castle" | 640 × 384 | `dog` `yard_skeleton` | `house_door` `mine_door` `burial_door` | `start` `house_front` `mine_mouth` `burial_mouth` | `stoop` |
| `house` | 48 × 36 | — | `front_door` `hatch_a` `hatch_b` `bench` `ice_orb` `julies_note` `pantry_chest` | `front` `hatch_a` `hatch_b` | `kitchen` |
| `cellar` | 100 × 76 | — | `stair_a` `stair_b` `cellar_chest` `iron_door_a` `iron_door_b` `storage_gate` `storage_chest` `potion_bench` | `stair_a` `stair_b` | `cellar` |
| `mine` | 160 × 130 | `clerk` `headmaster` `iron_knuckles` | `exit_door` `plate_a` `plate_chest` `store_chest` `gate_generic_a/b` `gate_hm` `broken_steps` `boss_key_chest` `gate_vault` `vault_chest` `gate_boss` | `entry` | `mine_entry` `boss_arena` |
| `burial` | 176 × 150 | `burial_snake` `garden_flower` | `exit_door` `torch_a/b` `torch_chest` `snake_key_chest` `gate_snake` `giant_key_chest` `snake_gate_east/south` `root_a/b/c` `fire_scroll` | `entry` `lockin_a–d` | `burial_entry` `garden` `lockin_room` `snake_arena` `everywhere` |

**The solver** floods from the entrance with locked gates shut, then repeats until nothing changes: loot reachable chests (keys by `opens` tag, materials by item), open gates a held key fits, fire reachable plates / levers / cold torches, fire repairables whose `needs` are in hand, kill reachable hostiles (guaranteed drops and `onDeath` unlocks), fire `while` triggers whose rect is reached. It fails a candidate for: a missing name, a duplicate key, an unknown row, a trigger whose rect does not exist, anything spawning in a wall, an unreachable mark / required unit / required prop, or a gate that never opens. `test/world.test.ts`: five zones × 25 seeds, plus a deliberately sealed gate the solver must reject.

Pushables and carriables count as passable (they move). Fights count as won. `enter` lock-ins are ignored (they release on a kill the solver assumes).

---

## 4. The live zones

### County — `world/county.ts`

2020 language: fenced lots, house stamps, two camps, door-thin exits.

1. Grass, tall-grass blobs. A **river** north to south; the road paints over it, which is the bridge. Bolts cross water; feet do not.
2. **Auntie's yard** (south-east): a fenced lot, the house stamp, `house_door` (locked, `auntie_house`), a dirt path from the west gate to the stoop, `dog`, two barrels, an apple tree, `yard_skeleton` in the far corner in sight of the stoop. **`start` is just inside the gate**, not on the stoop: 2020 started beside the dog, the 2026 build started *inside* the letter's trigger and completed it on frame one. A short walk is the right amount.
3. **Mine mouth**: a cliff face north-east of the yard, a lamp, a sign, a dead minecart, a path from an east gap in the fence.
4. **Burial stair**: a walled graveyard south-west, coffins, two skeletons, a standing stone that says DO NOT.
5. **Town** (north-west): a main street, two cross streets, six fenced lots with a house from the 2020 size table (7×9, 11×6, 10×12 tiles of 16 px), apple trees, barrels, lamp posts, the station where the Sunday train stops, rails to the west edge.
6. **The road** between the camps, lamp posts along it, a bandit pair by the bridge, a patrolling skeleton near town.
7. **Forest** (south-west) with the abandoned car from the town map (wood, water, a fire stone). A soft pocket: it may fail to place and nothing minds.
8. Claim all authored ground, then scatter trees, 40 herbs (one loot each, gone when picked: no farm), rocks to carry. A tree line round the edge.

### House — `world/interiors.ts`

Kitchen and living room, a doorway that moves. Two hatches on one wall, both to the cellar. Stove, the ice orb beside it ("cold on purpose"), Julie's note by the table, the bench, the pantry chest (exactly one Manashield's worth: dust, water, pansy), a fruit bowl. Leftover furniture down the living-room wall, as 2020 stacked its benches. No Fireball here; it was never in the kitchen.

### Cellar — `world/interiors.ts`

2020's loop plus the rooms from its basement sketch. Room A under hatch A: the chest with two iron keys, barrels. **Iron door A** north to a long corridor. Four rooms off it: rats and a chest with a plain key; the **potion room** (bench, water, herbs); the **storage room** behind a plain-key gate (4 wood, 4 iron: what the mine's broken things want); the study (empty; 2020's sketch put the electricity orb here). A rose alcove (White Water Rose → Stone Skin). **Iron door B**, room B, hatch B.

### Mine — `world/mine.ts`

§1.4's path as a graph, with ragged rooms, barrels and torches everywhere ("carve rooms, then drown them in barrels and torches; lock three doors"). Entry → plate room (barrel + plate + chest) / store (plain key, iron) / guard room behind plain gate A (the clerk carries the HM key; a chest of wood). Plain gate B → core → HM gate → the Headmaster's office (he drops the vault key); broken steps (Repair, 2 wood) → gallery, the ornate chest (boss key); vault spur → vault (5 gold bars, the Museum key); boss gate → arena, **Iron Knuckles**, four pillars, the gate drops behind you and lifts when he falls or you do. The track and cart are set-dressing until the cart row exists.

### Burial — `world/burial.ts`

START in the centre, as the 2020 map drew it (the room put you in the miniboss's lap). West: a corridor of cold torches; an alcove with two of them and a chest that unlocks when both burn; the rat room with a pillar maze and the Snake Key. North: the Snake Key gate, the lock-in (four guards stand up at the named marks), the ornate chest with the Giant Snake Key. North-west: the snake's room, four pillars, its eight-point patrol, an east gate that takes the giant key and a south gate with no keyhole that only opens from the inside of a dead snake. East: two statues that spit and two snakes you feed, a wall stub to throw bait from, a chest. South: the garden, a pond, a cactus, a patrolling pumpkin, the flower that holds three roots shut over the last page of a book.

Unlike 2020 the boss room cannot be entered without its key. 2020's keyless south door made the whole Snake Key chain optional.

---

## 5. Topology

```
county "Castle"
  town (NW) --road, bridge-- Auntie's yard (SE)
    ├── house_door  (locked: auntie_house, from the dog)  → house:front
    │     └── hatch_a / hatch_b                           → cellar:stair_a / stair_b
    ├── mine_door                                         → mine:entry
    └── burial_door                                       → burial:entry
```

Travel is by **named mark**, both ways. You arrive in front of the door you used, because the door's `to.mark` and the far side's mark are a named pair, which is all 2020's `door_id` + `place_x/y` was.

---

## 6. Load ring

2020: `objects/distance_unload/`, ticked by `game` Step **before** units.

```
block_size            = path_cell * 2        // 16 px
dynamic_load_distance = 24 * block_size      // 384 px
dynamic_load_size     = distance * 2         // 768 px square
```

On a block change: deactivate dynamics, statics, usables and **idle** AI; activate the square. AI in combat stays awake. `instance_activate_all` before save, load, zone change and F5. A 256 px chunk index was written and commented out. *(Corrected)* the debug rectangle draws unconditionally, not only in debug.

Live (`sim/ring.ts`): same constants, same rule, re-evaluated on block change only. Sleeping means no think, no occupancy, no draw. Three differences, all deliberate: sleepers' cooldown and respawn clocks keep ticking; nothing is "activated" before a save, because a sleeping unit is an ordinary row in the state tree; and tile chunks are the renderer's business (`render/tiles.ts`, 16-cell chunks, LRU), not the sim's.

---

## 7. Do not

- Copy `.yy` instance lists into TypeScript.
- Grow the county without a skeleton, constraints and a density budget (`PLAN.md` §2). Space with nothing decided about it is how the Phaser build died.
- "Port" Butterfly Forest from the 2020 room: there is nothing in it. Build it from the 2020 *map* (`DESIGN-2020.md` §4.5): no keys, no doors, eight butterflies.
- Route the burial through the kitchen hatch.
- Generate the dump chest as loot.
- Refer to a coordinate from outside a builder.
- Throw on a bad seed.
- Add `class Kitchen extends Zone`.

---

## 8. Adding a zone

1. Add the id to `ZoneId` / `ZONE_IDS` (`sim/state.ts`). The compiler lists what else needs it.
2. Write `world/<zone>.ts`: a `Kit`, rooms, corridors three cells wide where a gate sits, props with keys for anything the story names, marks for every way in.
3. Add its `CONTRACTS` and `GIVEN_KEYS` rows and its builder in `world/index.ts`.
4. Add rows: doors on the far side (`to: { zone, mark }`), triggers, quests, dialogue.
5. `npm test`. The solver runs it on 25 seeds; the catalog test checks every quest it adds can be given and handed in.

The 2026 generators for the museum, factory, school, pipes and forest are in `archive/phaser-remake-2026/src/game/world/`. They are inventions, not ports, but the room graphs are a head start. Order: `SYSTEMS.md` §12.

---

## 9. The county as built (September 2026)

The direction note at the top of this file is now code. `jane/src/world/skeleton/` decides a 2000 × 2000 m county (3600 × 2000 until 2026-09-24) from rows (`sites.json`, `areas.json`, `pois.json`); `jane/src/world/county.ts` turns it into cells and `chunks.ts` holds the authored places it stamps. `ENGINE.md` §8.1 and §8.2 describe both; the seed viewer (`/viewer.html`) shows 24 at a time. The zone contract above is unchanged: the county still promises `dog`, `yard_skeleton`, `house_door`, `mine_door`, `burial_door`, `start`, `house_front`, `mine_mouth`, `burial_mouth` and the `stoop` rect on every seed, and the same solver still proves it.

The railway is part of the skeleton (`skeleton/rail.ts`): it comes in at the south edge along the west fence, runs through Castle Halt, turns east where the Works begin, crosses the roads on the level (a sign at each crossing) and the river on a trestle, and leaves through the east fence. Where it leaves, a fence crosses it and the rails run on through the trees to the edge of the map. It is walkable, sleepers and ballast, not a wall. Nothing small is placed on it, and the roadside beat walks it like a road, so dead signals and slag wagons stand beside it.

## Source

- Rooms: `Junqi-Legacy-GM/jun7/rooms/*` (parsed; per-room instance dumps were produced by the 2026 audit).
- Unload: `objects/distance_unload/Create_0.gml`, `Other_10.gml`; `game/Step_0.gml`; `save_room.gml`; `obj_change_zone_box`.
- House stamps: `objects/obj_tile_parent/Draw_0.gml`, `datafiles/maps/map1L1.csv`, `map1L2.csv`.
- Live: `jane/src/world/*.ts`, `jane/test/world.test.ts`, `jane/test/dungeons.test.ts`.
