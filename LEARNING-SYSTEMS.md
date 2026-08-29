# Junqi Legacy — Systems Map

Companion to `LEARNING.md`. This is a remap of what 2020-you actually implemented, with file paths, so a remake can copy *contracts* instead of GML. The remake bar lives in `SYSTEMS.md`. Paths below stay under `Junqi-Legacy-GM/jun7/`.

All paths are under `Junqi-Legacy-GM/jun7/`.

---

## Boot and session

| Piece | Where | Contract |
| --- | --- | --- |
| Globals | `scripts/init/init.gml` | `global.debug`, `global.name`, `global.os_current`, `global.path_cell_size = 8` |
| Controller | `objects/game/` | Persistent session. Owns pause, GUI, quest lists, `game_data`, `player` ref, lighting flag, day `time` |
| Room boot | `objects/obj_room_init/Create_0.gml` | Spawns `collision_map`, `distance_unload`, `game` if missing, `map_player`, then `load_room()` |
| Zone change | `objects/obj_change_zone_box/` | `save_room()` then `room_goto` |
| Input | `objects/input/` + `scripts/Input_Functions/` | `input_check / pressed / released / analogue`. Sets `current_input` |
| Collision | `objects/collision_map/Create_0.gml` | `mp_grid` sized to room, cell 8. Adds static, dynamic, usable solids, water |
| Camera | `objects/obj_camera/` + `scripts/Camera_functions/` | Follow / lock. Player has `CAM_STATE` |
| Particles | `scripts/ParticleList/ParticleList.gml` | One `part_system`, manual update from `game` Step, several `part_type`s |
| Terminal | `objects/terminal/` + `scripts/Terminal_Functions/` | Backtick toggle. Commands: `debug`, `inst count`, `clear`, `restart`, `ver`, `exit` |
| Debug | `objects/debug/` | Overlay on. F2 = 60 ↔ 6000 fps. F3 draws the path grid. F5/F6 on `game` |
| Title | `objects/objStartScreen/` | Load / Options are TODO. `pressed = true` can skip the wait |
| Resolutions | `scripts/Resolution_List/` | Written, never called |

`game` Create also loads `maps/map1L1.csv` and `map1L2.csv` into `building_grid1/2` (overworld building overlay, not the dungeon rooms).

Windows product: **J&J RPG**, company **J&J corp**, `(c) 2020`. Mac/Linux display name is still JUN7.

---

## Units

**Chain:** `obj_in_world_parent` → `obj_unitParent` → (`obj_friendlyParent` | `obj_undeadParent`) → concrete units.

**Object:** `objects/obj_unitParent/`

**Create sets**

- Vitals: `hp`, `maxhp`, `mp`, `maxmp`, `energy`, `maxenergy`
- Stats: `strength`, `spirit` → `CreatureStatCalc()` (`maxhp = strength * 5`, `maxmp = spirit * 5`)
- Motion: `walk_spd`, `run_spd`, `spd`, `myPath`, `moving`, `stop_timer`
- Combat: `faction`, `class`, `controller`, `combat_state`, `current_target`, `autoAttack`
- AI: `aggroRange` (metres), `leashRange`, `patrol_path`, `respawn_timer` (36000 frames), `drop_once`, `respawn_again`, `auto_regen`
- Anim: `creatureState` + `SpriteSet()` 4-dir sprites + frame ranges `f_idle`, `f_walk`, …
- Bags: 24-slot `inventory_items` / `inventory_qty`, `spellbook_IDs` / `spellbook_cooldowns`, `gcd` (1.5s)
- `incoming_list`: one nested list per `damage_type`
- `dialogue_setup()` on every unit

**Tick split** (fired by `game`, not native Step)

| Event | File | Job |
| --- | --- | --- |
| User 0 | `Other_10.gml` | Alive: stats, GCD/CDs, death check, AI, stop timer, anim, occupy path cell |
| User 1 | `Other_11.gml` | Dead tick / respawn counter |
| User 2 | `Other_12.gml` | Animation |
| User 3 / 4 | pause / unpause | Path speed stash |
| User 5 | `Other_15.gml` | Drain incoming damage, float text, shake, aggro |
| User 6 | `Other_16.gml` | Extra (player overrides a lot) |

**Player extras** (`objects/obj_player/`)

- Starts `strength/spirit = 140`, walk 1 / run 2
- `SpriteSet(sprite.player)`, `creatureClass.mage`, `creatureController.player`
- Energy sprint + empty lockout
- Interact: use object in front, push/pull `"pushable"`, carry `holding_object`
- Camera follow/lock
- `itemCooldownList` of `[itemId, frames]`

**Factions:** `undead`, `beast`, `bandit`, `friendly`. `is_enemy` is simply `faction !=`.

**LOS:** `CheckLOS()` line-lists `obj_static_solid_parent` and respects `block_los`.

---

## Combat / spells

**Data:** `scripts/SpellList/SpellList.gml`

```
spellData: name, description, icon, mpCost, energyCost, range,
           castAni, onCastObject, cooldown, gcd_immune,
           requires_target, req_enemy_as_target, req_los
```

`spell` enum is the authoring ID. `global.spellRef[? spell.frostbolt_0]` → row index in `global.spellList`.

**Pipeline:** `CastSpell(spellbook_position)` in `scripts/Spell_Functions/Spell_Functions.gml`

1. Dead / target / enemy-or-not / alive / LOS / range / cooldown / GCD / MP / energy
2. Spawn `onCastObject`, set `caster`, `target`, `direction`, `range`, `event_user(0)`
3. If `valid`, pay costs, start GCD (unless immune), start that slot's cooldown, play `castAni`
4. Else destroy the spell object, return `castUnsuccessful`

`PlayerCastSpell()` maps `spellErrors` to `onscreen_message`.

**Damage types:** `heal`, `physical`, `frost`, `fire`, `nature`. Units have `damage[]` and `resistances[]` arrays (sized, barely used).

**Known spells**

| Enum | Role |
| --- | --- |
| `melee`, `melee_fast`, `melee_stun`, `melee_player` | Melee windows |
| `frostbolt_0`, `frostbolt_1_AI` | Icebolt |
| `fireball_0` | Fireball (not given at start) |
| `repair` | World interact / repair |
| `root`, `spawn_plant_ai` | Plant / root attacks |
| `poisonbolt_0`, `poisonbolt_0_slow`, `poison_cactus`, `poison_snake_boss` | Burial chamber kit |
| `webshot_ai`, `webshot_group_ai`, `spider_melee`, `spider_web_wrap` | Spider kit |

**Objects:** `obj_spellParent` children (`obj_spellFrostbolt_0`, `obj_spellMelee`, `obj_spellSnakeBoss`, …).

**Melee numbers** (`obj_spellMelee/Other_10.gml`): `dmg = strength/8 + irandom(strength/32)`; 1-in-20 doubles as crit; then `DamageHP(..., physical)` and `RestoreENERGY`.

**AI pick:** first spell in the book that is affordable and off cooldown becomes `autoAttack`. Order in the book *is* priority.

---

## Items, inventory, crafting

**Data:** `scripts/ItemList/ItemList.gml`

```
itemData: name, description, icon, usable, castAni,
          onUseObject, onDropObject, maxStack, cooldown
```

`global.itemRef[? "Apple"]` → row id. Chests and recipes should use the string key, never a raw number.

**Bags:** two parallel lists, 24 slots, `-1` empty. Stack into existing, then first hole. Overflow returns leftover qty and prints "Inventory Full".

**Use:** spawn `onUseObject`; if `canBeUsed`, consume 1, start item cooldown list + GCD.

**Hotbar:** `game.actionBarPositions` is a grid of `ABPOS` (x1,y1,x2,y2, qty, source, id, hovered, pressed). Source `1` = item, `2` = spell.

**Craft:** `scripts/craftMap/craftMap.gml`

- 3 input slots + 1 output
- Inputs sorted by item id, concatenated *display names* become the map key
- `craftAdd(r1, r2, r3, resultName, qty)`
- Taking the output calls `InventoryAdd` then `inventoryRemoveItem` once per input slot (removes 1 each)

**Recipes that exist**

| Inputs | Output |
| --- | --- |
| Gold Dust + Small Water + Pansy | Small Manashield Potion |
| Gold Dust + Small Water + Nasturtium | Small Life Steal Potion |
| Gold Dust + Small Water + Honeylace Lily | Small Critical Potion |
| Stone + Small Water + White Water Rose | Small Stone Skin Potion |
| Small Water + Hemshade Root | Small Firelash Potion |
| Small Water + White Water Cap | Small Sparktongue Potion |
| Small Water + Night Lich Moss | Small Winterbite Potion |
| Small Water + Savage Snakeroot | Small Stranglethorn Potion |
| Gold Bar | Gold Dust x4 |
| Rock | Stone |
| Small Stranglethorn Potion + Rat Meat | Poisoned Rat Meat |

**Unfinished:** most potions share `obj_onUseManaPotion1`. Most materials share `obj_drop_iron`. Keys are one object each (`obj_onUseKey_*`).

---

## Quests and dialogue

**Authoring:** `scripts/Quest_Database/Quest_Database.gml`

```
quest_create(refName)
quest_description / quest_completion
quest_requirement_add(quest, type, object_id, text, qty, map_x, map_y, zone)
```

**Types:** `kill`, `aquire` (typo), `location`.

**Runtime:** `game.current_quest_list` is a list of lists: `[questId, prog0, prog1, ...]`. Completions go to `quest_completed_list`.

**Hooks**

- Kill: `quest_requirement_slay_unit(object_index)` from unit death if `quest_unit`
- Item: `quest_requirement_acquire_item(item)` from `InventoryAdd` on the player
- Location: `quest_requirement_enter_location(questName, locationNumber)` (must be called from a trigger)

**UI:** `questlog_logic` + `draw_quest_log` in `Quest_Functions.gml`.

**Dialogue:** `scripts/Dialogue_Functions/`

- Unit owns `dialogue_list`
- `dialogue_add(text)` or `dialogue_add_option_2(prompt, a, b)`
- `dialogue_play(x, y)` spawns `obj_dialogue`
- NPC script is a state machine on `dialogue_number` + last option

**Only authored loop:** `objects/obj_auntie_dog/Other_10.gml` — test text → give "Defeat Skeleton" → check log → hand-in → missing reward.

Database quests are placeholders. "Reclaiming the Charred Vale" is pasted WoW text.

---

## AI and pathfinding

**Scripts:** `scripts/scr_ai_logic/scr_ai_logic.gml`, `scripts/Pathfinding_Functions/Pathfinding_Functions.gml`

**States:** `combatState.idle | leashing | combat`

**Idle**

- Regen ~`max/300` per tick if `auto_regen`
- Every 10 frames: `get_nearest_enemy` in `aggroRange` metres, must be alive, enemy, LOS
- If `patrol_path` exists, assign and `path_start`
- Enter combat if LOS + in range, or already have a living `current_target`

**Combat**

- Drop target if dead, absurdly far, or past `leashRange` from leash point
- Else pick spell, `CastSpell`
- Success: face target, stop path
- `tooFar` / `notInLOS`: `PathTo` target; fail → leash
- Occupy current cell so other AI path around this unit

**Leash**

- Clear target, path to `leash_to_x/y`
- Snap when close, restore patrol position/speed, `combat_state = idle`

**PathTo(grid, x, y, speed, maxDistMetres)**

- `mp_grid_path` with diagonals
- Accept only if path length < max distance
- If blocked, spiral 4-way at 4px / 4 steps — **bug: uses `div 32` not cell 8**

`collision_map` rebuilds from parent solid objects. Dynamic carry/push must add/clear cells themselves.

---

## Save / load

**Slot I/O:** `scripts/Save_Load_From_Slot/Save_Load_From_Slot.gml`

```
slotN.jsav = compress( json_encode(game.game_data) )
```

**`save_room()`** writes into `game.game_data`:

| Key | Contents |
| --- | --- |
| `room_get_name(room)` | List of instance snapshots |
| `current_quest_list`, `quest_completed_list` | Quest progress |
| `current_room` | Room asset id |
| `player` | Nested map: transform, vitals, stats, inventory, spellbook, `vars` JSON |

Instance `type_of_obj`:

| Type | Parent | Extra |
| --- | --- | --- |
| 1 | `obj_unitParent` except player | stats, patrol, leash, respawn, inventory |
| 2 | `obj_usable_parent` | `used`; chests also `loot`/`qty` |
| 3 | `obj_reveal_regeon` | fog-of-war / minimap grab |
| 4 | `obj_door` | `locked`, `key`, `spr`, `vertical` |

**`load_room()`**

- If `obj_load_game` exists, `room_goto(current_room)` then copy quests
- Destroy all units / usables / reveals / doors (after running Create — leftover cleanup)
- Recreate from maps via `asset_get_index(obj_name)`
- Units reload at **start** position, HP written as **maxhp** at save time

`vars` is `json_stringify` / `json_parse` of a per-instance struct — your escape hatch for one-off state.

---

## GUI and windows

**Owned by `game`**

```
WINDOW: INVENTORY, SPELLBOOK, QUESTLOG, MAP
```

Open interface pauses + darkens. Bumper/arrows cycle windows.

**Hit-test grids** (same `ABPOS` schema)

- `actionBarPositions` — 8 slots
- `iconPositions` — inventory
- `craftPositions` — 3 in + 1 out
- `spellbookPositions` — 4 visible, `spellbookPage`

**Logic functions** in `Game_Functions.gml`: `actionbar_logic`, `inventory_logic`, `spellbook_logic`. Each takes `(cursor_x, cursor_y, input_type)` and duplicates KBM / gamepad / touch (touch empty).

**Draw**

- `objects/game/Draw_64.gml` — GUI
- `scripts/Interface_Functions/Interface_Functions.gml` — `draw_window` 9-slice via temp surface, tooltip frame
- `GUIDrawHealthBar`, `DrawItemIcon`, `DrawSpellIcon`
- `onscreen_message` — max 3 stacked lines, 180-frame life

`mouseOnGui` blocks world click-to-move / right-stick walk.

---

## Lighting and time

**Config:** `scr_lighting_config()` — `light_scale`, `light_color`, `light_on`, offset, `follow`, `light_sprite`.

**Parents:** static solid, static, dynamic, dynamic solid. Spells can emit too.

**Pass:** `objects/game/Draw_73.gml` (End Draw)

1. Surface = view size
2. Clear `lighting_color` (warm yellow, not driven by `time`)
3. `bm_subtract` each light sprite
4. Draw surface over the view, free surface

`lighting_on` is a bool. `game.time` advances `1/7200` per step (~2 real minutes per game hour at 60 fps) and wraps 24. **Nothing reads it for lighting.**

Dungeons vs overworld: `is_in_dungeon()` is currently `room != room_zone1`. Do not reuse that.

---

## Rooms, doors, triggers

**Zone change:** `objects/obj_change_zone_box/` — collision with player, alarms, fade-ish Draw End.

**Doors:** `obj_door`, `obj_door_x`, locked + `key` item id, `door_setup(vertical)`.

**Keys:** one `obj_onUseKey_*` per door family (house, generic, mine headmaster/vault/boss, auntie basement, snake, snake boss).

**Triggers worth knowing**

- `obj_trigger_room_lock_player_in`
- `obj_boss_room_trigger`
- `obj_trigger_rat_spawn_trigger`, `obj_trigger_rat_path_to_snake`
- `obj_kill_root_trigger`
- `obj_learn_spell_fireball`
- `obj_room_init` (per-room setup via creation code)

**Burial chamber** is the densest room: lots of instance creation code, paths `path_burial_chamber_*`, `path_snake_boss`, statues, coffins, lily pads, snake keys.

**Snake boss:** `obj_snake_boss` — custom `move_snake()`, body list, sine wiggle, phases, dummy object cleaned on create. Not the generic AI mover.

**Enemy roster** (mostly `obj_undeadParent`, faction `undead`)

| Object | Notes |
| --- | --- |
| `obj_UndeadTest` | Skeleton, quest kill target |
| `obj_BanditTest` | Overworld placeholder |
| `obj_UndeadBat`, `obj_UndeadPumpkin`, `obj_UndeadCactus` | Field undead |
| `obj_UndeadFlower1/2`, `obj_UndeadBossFlower1` | Plant casters |
| `obj_undead_soldier1/2/3`, `obj_undead_miniboss`, `obj_undead_boss` | Humanoid undead |
| `obj_undead_root`, `obj_undead_lily_pad` | Hazards |
| `obj_snake_1`, `obj_UndeadSnake`, `obj_snake_boss` | Burial snakes |
| `obj_spider1_wall`, `obj_small_spider1`, `obj_guard_spider1`, `obj_spider_boss` | Web kit |
| `obj_rat` | Mine; drops Rat Meat |

---

## Sprites / animation

`scripts/SpriteSet/SpriteSet.gml` — enum `sprite.*` then a huge switch assigning:

```
s_front, s_back, s_left, s_right
f_attack, f_cast, f_dead, f_hurt, f_idle, f_shoot, f_walk, f_loot
```

Each value is `[startFrame, endFrame]` on those directional strips.

**Roster:** player, auntie dog, skeleton, test, bat, undead pumpkin/cactus/soldiers/mini/boss, snakes, roots, flowers, spiders, rat.

**Bug:** `undead_cactus` missing `break` → becomes soldier.

A remake should be a table: `sprite_id → {dirs, frames}` not a 400-line switch.

---

## Input map (as designed)

`interaction.*` (defined on the input object, not repeated here) is the action layer. `input_check(interaction.USE)` etc.

`game` Create:

- Mobile → GUI event 8 (touch)
- Desktop → events 6 and 7 (KBM + pad)
- Console → event 6 (pad)

Player movement accepts WASD/dpad *or* analogue *or* right-mouse hold as a virtual stick (`distance/32` → strength).

Sprint drains 0.5 energy/frame; carry drains 0.25; idle regen 0.5. Hit 0 → `energy_empty` until full again.

---

## What to port vs rewrite

**Port the contract**

- Item row, spell row, quest requirement row
- Incoming damage queue
- Spell validation error enum
- Slot save shape (player + per-room instance list)
- AI idle / combat / leash
- Metre/pixel split
- Action bar source + id
- Crafting as an unordered 3-ingredient recipe (but key by IDs)

**Rewrite**

- `ds_*` → structs / arrays / files
- `object_check_parents` → ancestor test
- Per-frame lighting surface → cached
- `draw_window` temp surface → 9-slice draw
- Per-key / per-bench objects → data
- Instance creation code dungeons → scene files
- SpriteSet switch → table
- Touch `TODO`s → one input path
- Quest copy / potion onUse stubs → real content

**Do not port**

- TweenGMS v1.0.6 scripts (`extensions/TweenGMS/`, Stephen Loney 2017)
- `__health`, `__score`, `__background_colour` compat leftovers
- WoW quest strings
- `global.debug = true` as default
