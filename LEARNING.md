# Junqi Legacy (2020) — Learning Notes

Hand-coded GameMaker Studio 2 project, folder `Junqi-Legacy-GM/jun7`. Internal name is **jun7**. Windows exe ships as **J&J RPG** (`J&J corp`, `(c) 2020`, description *"A RPG game to make smile."*). Mac/Linux still say JUN7. Quest copy calls the player **JoJo**.

The remake in `junqi/` is the live product. The player there is **Jane**; Auntie is **Julie**. This file stays 2020 history — JoJo stays here because that is what shipped.

This is a top-down 2D ARPG: overworld, interiors, three dungeon spaces, Mage-class combat, inventory, crafting, quests, lighting, and a compressed JSON save.

These notes are for **you**, six years later. They are not a roast. This is an unusually complete solo 2020 GameMaker RPG framework. The systems outran the content, which is the most useful thing the project still teaches.

---

## Snapshot

| Asset | Count |
| --- | ---: |
| Objects | 242 (231 `obj_*`) |
| Sprites | 364 |
| Scripts | 212 (a large chunk is TweenGMS) |
| `.gml` files | 1066 |
| Rooms | 9 |
| Tilesets | 13 |
| Sounds | 2 |

**World rooms**

| Room | Role |
| --- | --- |
| Room | Size | Role |
| --- | --- | --- |
| `room_start` | 1024×768 | Boot / title. Spawns `input`, `objStartScreen`, `terminal`, optional `debug` |
| `room_parent` | 1366×768 | Inherited template. `RoomCreationCode.gml` is referenced but missing on disk |
| `room_zone1` | 5120×3072 | Overworld / town |
| `room_building1` | 1024×1024 | Interior |
| `room_building2_auntie` | 512×512 | Auntie's house (fruit, craft bench) |
| `room_basement_auntie` | 1024×1024 | Auntie's basement |
| `room_dungeon_goldmine` | 2048×2048 | Gold mine |
| `room_dungeon_burial_chamber` | 2048×2048 | Burial chamber (most authored content) |
| `room_dungeon_butterfly_forest` | 3072×2048 | Butterfly forest |

**What the game actually is**

A Zelda-like overworld with WoW-adjacent systems: GCD, spellbook, action bar, leash AI, kill/acquire/location quests, potion crafting, keys, chests, day clock, subtract lighting, push/pull/carry, and slot saves. The player is a mage (`creatureClass.mage`) starting with melee, Icebolt, and Repair. The authored story cluster is **Auntie / house / basement / gold mine / burial chamber**, with undead plants, snakes, spiders, and a segmented snake boss.

---

## What 2020-you got right

These are the ideas worth keeping if you remake Junqi.

### 1. Data tables, then objects

Items and spells are not hardcoded in every chest and enemy. They live in grids plus a name/enum lookup:

- `scripts/ItemList/ItemList.gml` + `ItemAdd()`
- `scripts/SpellList/SpellList.gml` + `SpellAdd()` / `SpellModify()`

Use-behavior is a **spawned object**, not a giant switch: `onUseObject` / `onCastObject`. That is composition. It still scales.

### 2. One unit, two controllers

`obj_unitParent` is the whole combatant model: HP/MP/energy, inventory, spellbook, GCD, faction, aggro, leash, patrol, incoming damage queues, dialogue hook. Player and AI share it. `creatureController.player` vs `.ai` is a real architecture choice, not a later patch.

### 3. Deferred combat

`DamageHP()` / `RestoreHP()` do not mutate HP immediately. They enqueue `{value, from}` onto `incoming_list[damage_type]`. `obj_unitParent` User Event 5 drains the queues, applies clamp, floating text, camera shake, and first-hit aggro.

That is how you avoid same-frame order bugs. Keep this idea even if you drop `ds_list`.

### 4. Pause is a broadcast

`pause_game_before_step()` flips `game.paused` and fires user events on `obj_unitParent` and `obj_spellParent`. The `game` Step then either runs the world (`event_user(0)` on units, spells, drops, camera) or only GUI. You already treated the game as a scheduler, not a pile of independent Step events.

### 5. Distance is a unit

`GetDistanceMetres()` / `GetDistancePixels()` with `global.path_cell_size = 8`. Spell range, aggro, and leash talk in metres. Pathing talks in pixels. That split is adult engine work.

### 6. AI is an MMO loop, not a chase script

`scr_ai_logic.gml`: idle (regen + patrol + throttled aggro), combat (pick first affordable spell, walk in if too far / no LOS), leashing (run home, resume patrol). Units occupy path cells so they do not stack. Aggro is sampled every 10 frames, staggered with `irandom(9)`.

### 7. Save is a real product feature

`save_to_slot()` JSON-encodes `game.game_data`, compresses a buffer, writes `slotN.jsav`. `save_room()` snapshots player, units, usables, map reveals, doors, and quest lists. That is more than most hobby GM RPGs ever shipped.

### 8. Input was designed for more than one device

`input_check*` walks keyboard then gamepads and sets `current_input`. `game` Create builds `gui_events` from `os_mobile()` / `os_desktop()` / `os_console()`. Touch is unfinished, but the *shape* is already multi-platform.

### 9. The world has verbs beyond combat

Push/pull tagged `"pushable"` objects, pick up / put down on the 8px grid, locked doors with per-key use-objects, minecarts, lily pads that light torches, room-lock triggers, boss-room triggers, fog-of-war map reveals (`obj_reveal_regeon`). This is a place, not an arena.

### 10. You documented intent in code

`ABPOS` enum, `spellErrors`, `quest_req`, comments like "Speed hack, checking once every 10 frame", "TODO FINISH THESE". Future-you can still read what 2020-you was aiming at.

---

## What did not age well

Honest list. Most of this is "solo 2020 GameMaker," not incompetence.

### Naming never settled

In the same files you have `InventoryAdd`, `inventoryUsePosition`, `CostMP`, `RestoreENERGY`, `moveToX`, `leash_to_x`, `argument0` leftovers, and `obj_reveal_regeon`. That is not vanity. Inconsistent names make the project *feel* larger than it is, because you cannot grep one convention.

### Magic numbers instead of enums

Icon drag `from_where` is `0` action bar, `1` inventory, `2` spellbook, `3` craft. Save `type_of_obj` is `0..4`. Input device is `0/1/2` in the Step switch even though you have `INPUT_*` constants. You already knew how to write enums (`WINDOW`, `ABPOS`, `spell`, `creatureState`). Use them everywhere.

### Content is placeholders sitting on real systems

- Almost every unfinished material drops `obj_drop_iron`.
- Most potions use `obj_onUseManaPotion1`.
- Quest DB still has "Good Job JoJo", a pasted WoW Charred Vale text, and `quest_req_type.aquire`.
- Auntie's dog is the only authored dialogue, and it is a test of the dialogue *system*.
- Quest reward is `onscreen_message("TODO: GIVE REWARDS FOR QUESTS")`.
- Web spells: `TODO DEBUFF SLOW ON PLAYER`.
- Touch input: `TODO` in every GUI path.
- `global.debug = true` left on.
- Two sounds in the whole game.

The engine could carry a small campaign. The campaign was never written.

### GameMaker-era data structures

`ds_grid` / `ds_list` / `ds_map` everywhere, plus `ds_list_mark_as_list` and nested maps for save. In 2020 that was correct. Today structs + arrays (or a real data file) are simpler, leak less, and JSON better. A remake should not port the `ds_*` graph.

### Surfaces created every frame

`game` Draw End builds a lighting surface, subtracts every light sprite, draws it, then `surface_free`. `draw_window()` does the same for every window chrome. That is easy and leak-safe. It is also expensive. Cache one lighting surface and one 9-slice surface, or draw 9-slice without a surface.

### `object_check_parents` walks 200 parents

GMS has `object_is_ancestor()`. The 200-iteration climb is a tell that you were solving the language from first principles. Fine then. Delete it now.

### Room inheritance + instance creation code as content

`room_dungeon_burial_chamber` is full of `InstanceCreationCode_inst_*.gml` files, some defining `press_button()` locally. That is how you ship a dungeon in GM. It does not migrate. Any remake needs room data as data, not as 80 one-off instance scripts.

---

## Concrete bugs to remember

These are real, not style nits.

1. **Player Create swaps axes** — `moveToX = y` and `moveToY = x` in `obj_player/Create_0.gml`. Harmless while click-to-move is commented out. It is still a landmine.

2. **Missing `break` in `SpriteSet`** — `sprite.undead_cactus` falls through into `undead_soldier1`. Cactus uses soldier sprites.

3. **`CastSpell` can crash** — it reads `current_target.alive` *before* `instance_exists(current_target)`. `noone.alive` is a runtime error if a targeted spell fires with no target.

4. **`PathTo` fallback uses cell 32** — main grid is 8px (`global.path_cell_size`). The "nudge to nearest free cell" loop does `xto div 32`. Path recovery is looking at the wrong grid.

5. **Save stores unit HP as `maxhp`** — `ds_map_add(_map, "hp", maxhp)` in `save_room`. Enemies reload full. Combined with load using `xstart`/`ystart` instead of live `x`/`y`, combat state is not actually saved.

6. **`is_in_dungeon()` is a one-liner lie** — `return room != room_zone1`. Auntie's house and basement count as dungeons.

7. **Spellbook paging disagrees with itself** — controller pages by `/ 4`, mouse wheel by `/ 8`.

8. **`inventoryGetNextPosition`** — `ds_list_find_index` returns a real, never an array. The `is_array(position)` branch is dead.

9. **Day clock does not drive lighting** — `time += 1/7200` toward a 24-hour cycle, but `lighting_color` is a fixed warm yellow. The day system is a counter with no output.

10. **`CostMP` / `CostEnergy` return remaining resource, not the cost paid.** Callers ignore the return. Fine, but the names lie.

11. **`Resolution_List()` is dead.** Written, never called.

12. **Title Load / Options are `TODO`.** `objStartScreen` can skip the menu (`pressed = true`). Slot load exists; the title does not wire it.

13. **Melee formula** (`obj_spellMelee`): `strength/8 + irandom(strength/32)`, 1-in-20 double crit. Useful if you remake combat numbers.

---

## Architecture in one page

```
room_start
    -> persistent-ish controllers: game, input, collision_map, camera, terminal, debug
    -> obj_player (child of obj_unitParent)

game.Step
    pause / OS pause
    resize (user 5)
    if !paused:
        time of day
        distance_unload
        dialogue
        GUI by input device (user 6/7/8)
        zoom (user 4)
        game tick (user 0)
        units alive? user 0 : user 1
        spells, on-use items, dynamics, drops
        units: apply incoming damage (user 5)
        camera
        floating text + particles
    else if gui_open:
        GUI only

Cast / Use
    tables -> spawn obj_spell* or obj_onUse* -> event_user(0)
    costs, GCD, cooldown committed only if the object says valid

AI
    idle -> combat -> leash -> idle
    PathTo(collision_map.mp)
    occupy cell under feet
```

**Parents that matter**

- `obj_in_world_parent` → `obj_unitParent` — combatants
- `obj_friendlyParent` → `obj_player`, `obj_auntie_dog`
- `obj_undeadParent` → skeletons, snakes, spiders, plants, rats, bosses
- `obj_spellParent` — projectiles / melee windows / boss attacks
- `obj_onUseItemParent` — item scripts
- `obj_usable_parent` / `obj_chest_parent` / `obj_solid_usable_parent`
- `obj_static_solid_parent` / `obj_dynamic_solid_parent` / `obj_dynamic_parent`
- `obj_*_light_*` — lighting layers
- `obj_drop_parent`

**Third party:** TweenGMS v1.0.6 (Stephen Loney, 2017). Most of the 212 scripts are that library. Do not treat them as yours.

**Room bootstrap:** every gameplay room places `obj_room_init`, which creates `collision_map`, `distance_unload`, `game` if missing, `map_player`, then `load_room()`. Zone changes go through `obj_change_zone_box` (`save_room()` → `room_goto`).

---

## Lessons for a remake

1. **You already designed the game.** Do not start by inventing a new combat model. Port the *contracts*: item row, spell row, incoming damage, quest requirement types, room snapshot.

2. **Content was the bottleneck, not engine.** One NPC, one test quest, three dungeon rooms, two sounds. A remake wins by finishing Auntie's loop (house → basement → mine → burial chamber → snake boss → reward), not by adding a sixth system.

3. **Keys-as-objects do not scale.** `obj_onUseKey_house_auntie`, `obj_onUseKey_mine_vault`, `obj_onUseKey_BC_snake_boss`… That should be one key item with a `door_id` / tag. Same for benches: `obj_bench1` through `obj_bench9` should be one object plus a sprite.

4. **Crafting key-from-sorted-names is clever and fragile.** `craftSort()` concatenates item *display names*. Rename "Pansy" and the recipe dies. Recipes should be sorted IDs, not strings.

5. **Enums inside `obj_unitParent` Create** (`factions`, `creatureState`, …) work in GMS 2.3+ but they belong next to `init`. You already put `itemData` and `spell` in scripts. Finish that move.

6. **Keep the terminal.** `println` into an in-game console is how you debug an RPG. A remake still wants a command line (`give`, `tp`, `quest`, `god`).

7. **Do not port GML line-for-line.** Port the systems list in `LEARNING-SYSTEMS.md`. Rewrite data as structs or a spreadsheet. Lighting should be one surface. Pathfinding should use one cell size everywhere.

8. **The 2020 taste was "systems-first MMO-lite in a tiny town."** That is still a good Junqi. Auntie, dog, keys, mine, undead garden, snake temple. Lean into that, not a generic fantasy checklist.

---

## How to read the old project

Start here, in this order:

1. `scripts/init/init.gml` — almost empty; real boot is `game` Create + list scripts
2. `objects/obj_room_init/Create_0.gml` — how a room becomes a session
3. `objects/game/Create_0.gml` and `Step_0.gml`
4. `objects/obj_unitParent/Create_0.gml` and `Other_10.gml` (alive tick)
5. `scripts/ItemList`, `SpellList`, `craftMap`, `Quest_Database`
6. `scripts/Spell_Functions/CastSpell`
7. `scripts/scr_ai_logic`
8. `scripts/save_room` + `load_room` + `Save_Load_From_Slot`
9. `objects/obj_player/Other_16.gml` — movement, interact, push/carry
10. `objects/obj_auntie_dog/Other_10.gml` — the only quest/dialogue loop
11. `objects/obj_snake_boss/Create_0.gml` — the one custom boss

Ignore TweenGMS, Ease*, TGMS_*, and `__ext_*` unless you are debugging a tween.

---

## Tone check

You built, by hand, in 2020:

- a unit framework
- a spell pipeline with validation errors
- inventory + action bar + drag/drop + crafting bench
- quest log with three requirement types
- AI with leash and patrol
- 8px path grid
- subtract lighting
- day clock
- compressed saves
- keyboard + gamepad
- a debug terminal
- three dungeons and a snake that owns its own movement function

That is a **game engine with a town in it**, not a tech demo. The unfinished potions and JoJo quest text are what unfinished *content* looks like when the *engine* is already ahead.

If you rebuild Junqi, steal the systems, finish Auntie's story, and leave the `ds_list` era in this folder.
