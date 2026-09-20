# Jane Legacy — Systems Map

Companion to `LEARNING.md`. This is a remap of what 2020-you actually implemented, with file paths, so a remake can copy *contracts* instead of GML. The bar for the live build is `SYSTEMS.md`; how it works is `ENGINE.md`.

All paths are under `Junqi-Legacy-GM/jun7/`.

**Audited against the source, September 2026.** Counts, enums, columns, recipes, the save shape and the pipeline order were confirmed. Claims that were wrong are fixed in place and marked *(corrected)*.

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
| Terminal | `objects/terminal/` + `scripts/Terminal_Functions/` | Backtick toggle (keycode 192). Commands: `debug on\|off`, `inst count`, `clear`, `restart`, `ver`, `exit`. Ctrl+V or right-click pastes |
| Debug | `objects/debug/` | Overlay on. **F2** *(corrected)* does two things: *held*, `room_speed = 6000`; each *press* toggles `global.debug` and sets the player's run speed to 4 (on) or 1 (off) and never restores 2. **F3** held, while debug is on, draws the `mp_grid`; unit paths always draw in debug. **F5** `save_to_slot(1)` via `alarm[5] = 2`. **F6** `load_from_slot(1)` |
| Title | `objects/objStartScreen/` | Load / Options are TODO. `pressed = true` can skip the wait |
| Resolutions | `scripts/Resolution_List/` | Written, never called |

`game` Create also loads `datafiles/maps/map1L1.csv` and `map1L2.csv` into `building_grid1/2` (overworld building overlay drawn by `obj_tile_parent`, not the dungeon rooms). They are exports of `Tiled Files/untitled.tmx`, whose tileset is a purchased pack.

`game` Create builds a `gui_events` list from the OS type and **never reads it**; Step switches on `input.current_input` instead. `game` has user events 4, 5, 6, 7, 8 and 15 only, so its `event_user(0)` call is a no-op, and `obj_dialogue` has no user 0 either (it runs on its own Step and draws through user 10).

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
- AI: `aggroRange` (metres, default 10), `leashRange` (default **80**), `patrol_path`, `respawn_timer` (36000 frames), `drop_once`, `respawn_again`, `auto_regen`, `check_aggro_rng = irandom(9)`, `boundbox` (metres, default 2; range is measured between bounds), `speed_multiplier` (the only crowd-control field; only the player's movement reads it)
- Anim: `creatureState` + `SpriteSet()` 4-dir sprites + frame ranges `f_idle`, `f_walk`, …
- Bags: 24-slot `inventory_items` / `inventory_qty`, `spellbook_IDs` / `spellbook_cooldowns`, `gcd` (1.5s)
- `incoming_list`: one nested list per `damage_type`
- `dialogue_setup()` on every unit

**Tick split** (fired by `game`, not native Step)

| Event | File | Job |
| --- | --- | --- |
| User 0 | `Other_10.gml` | Alive: `CreatureStatCalc` (every tick: MP +`spirit/1000`; AI out of combat −20/60 energy), GCD/CDs, death check, AI, stop timer, anim. *(Corrected)* it only **records** `path_block_x/y`; the `mp_grid_add_cell` is in `ai_logic_combat` alone, so only units in combat block a cell |
| User 1 | `Other_11.gml` | Dead tick / respawn counter |
| User 2 | `Other_12.gml` | Animation |
| User 3 / 4 | pause / unpause | Path speed stash |
| User 5 | `Other_15.gml` | Drain incoming damage, float text, shake, aggro |
| User 6 | `Other_16.gml` | *(Corrected)* **the interact hook**: "Talked To / Being Looted". A dead unit hands its inventory to the player; a live one calls `dialogue_play`. Usables, chests and drops use user 6 the same way. The player overrides it as its main tick |

**Player extras** (`objects/obj_player/`)

- Starts `strength/spirit = 140`, walk 1 / run 2
- `SpriteSet(sprite.player)`, `creatureClass.mage`, `creatureController.player`
- Energy sprint + empty lockout
- Interact: use object in front, push/pull `"pushable"`, carry `holding_object`
- Camera follow/lock
- `itemCooldownList` of `[itemId, frames]`

**Factions:** `undead`, `beast`, `bandit`, `friendly`. `is_enemy` is simply `faction !=`, so undead and beasts fight each other and the friendly dog fights everything. `factions.bandit` is never assigned: `obj_banditParent` sets `beast`.

**Other enums:** `creatureState` attacking / casting / dead / hurt / idle / shooting / walking / running / looting. `combatState` idle / leashing / combat. `creatureClass` warrior / mage. `creatureController` player / ai. `damage_type` heal / physical / frost / fire / nature / SIZE. `WINDOW` INVENTORY / SPELLBOOK / QUESTLOG / MAP. `CAM_STATE` FOLLOW / LOCK. `DIALOGUE_MODE` STRING / OPTION. Input macros `INPUT_KEYBOARD_MOUSE` / `INPUT_CONTROLLER` / `INPUT_TOUCH` = 0 / 1 / 2.

**LOS:** `CheckLOS()` line-lists `obj_static_solid_parent` and respects `block_los`. *(Corrected)* `obj_water` sets `block_los = true` but its parent is `obj_static_parent`, which neither `CheckLOS` nor projectiles test, so **water never blocks sight or bolts**; it only blocks feet through the path grid. `CheckLOS` leaks its list when it returns false.

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

1. Dead → target validity (`is_enemy` is read *before* `instance_exists`) → alive → LOS → range (between bounds) → cooldown → GCD → MP → energy. `requires_target` is forced on for AI casters. For targeted spells `req_enemy_as_target = false` **rejects** enemy targets. `req_los` is always true except `poison_snake_boss` (via `SpellModify`)
2. Spawn `onCastObject`, set `caster`, `target`, `direction`, `range`, `event_user(0)`
3. If `valid`, pay costs, start GCD (unless immune), start that slot's cooldown, set `stop_timer = 30`, face the target, play `castAni`. Every spell's `energyCost` is 0
4. Else destroy the spell object, return `castUnsuccessful`

`PlayerCastSpell()` maps `spellErrors` to `onscreen_message`.

**Damage types:** `heal`, `physical`, `frost`, `fire`, `nature`. Units have `damage[]` and `resistances[]` arrays that are **never read** *(corrected from "barely used")*. There is no status system: see the behaviour table. All damage is `round()`ed when applied.

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

**What the spell objects actually do** (read from their events, not their names):

| Spell | mp / cd / range | Real behaviour |
| --- | --- | --- |
| `melee`, `melee_fast`, `melee_player` | 0 / 2 s (fast 0.5) / 0 | `strength/8 + irandom(strength/32)`, 5% ×2, physical, +3 energy (+9 crit). GCD-immune. **Player version hits nothing** (`target` written onto the caster) |
| `melee_stun` | 0 / 5 s / 0 | Same damage, plus `speed_multiplier = 0` on the target for 120 frames. Movement only; the victim can still cast |
| `root` | 0 / 1 s / 0 | `strength + irandom(strength)` nature. Applies **no root** |
| `frostbolt_0` (Icebolt) | 14 / 2 s / 15 m | `spirit*0.8 + irandom(spirit/8)` frost, no crit, 5 px/step, splash dmg/5 in 25 px. **No chill.** Lights wall torches |
| `frostbolt_1_AI` | 4 / 2 s / 15 m | Same object, targeted |
| `fireball_0` | 5 / 2 s / 15 m | `spirit + irandom(spirit/2)` fire, no crit, 5 px/step, splash. Learned from `obj_learn_spell_fireball` |
| `poisonbolt_0` | 1 / 1 s / 15 m | `spirit + irandom(spirit/2)`, **physical** (`dmg_type` never set), 2 px/step, no splash, no DoT |
| `poisonbolt_0_slow` | 0 / 2 s / 40 m | Same; "slow" describes the projectile |
| `poison_cactus` | 0 / 5 s / 15 m | 10 needles in a ±22.5° random fan, 4 px/step, nature |
| `poison_snake_boss` | 0 / 1 s / 15 m | Ring of 15 nature bolts 24° apart, 2 px/step, `spirit/32 + irandom(spirit/128)`, no LOS needed |
| `spawn_plant_ai` | 0 / 1 s / 15 m | A seed crawls at 1 px/step to where the player stood and becomes `obj_UndeadFlower1` |
| `webshot_ai` | 1 / 3 s / 15 m | 1 px/step, 5–10 physical, lands where the player stood and leaves `obj_spellWebOnGround` for 600 frames: 5 physical per 60 contact frames and `speed_multiplier = 0.25` for 61 |
| `webshot_group_ai` | 1 / 5 s / 15 m | Eight webshots over 120 frames, ±20° |
| `spider_melee` | 0 / 2 s / 0 | `strength + irandom(strength/3)`, then the same again at +1, +2, +3 s |
| `spider_web_wrap` | 5 / 30 s / 100 m | Pins the player for 300 frames, 5 physical per 30. May never expire (collision `timer++` cancels `timer--`; unverified) |
| `repair` | 0 / 10 s / 0 | Nearest `obj_repairable_parent` within bounds; consumes its `mats` list; invalid (free) otherwise |

Projectiles never home: they aim once on the first step and clear their target. The player's fly at the mouse.

**Objects:** `obj_spellParent` children (`obj_spellFrostbolt_0`, `obj_spellMelee`, `obj_spellSnakeBoss`, …); projectiles share `obj_spell_projectile_parent` (`homing`, `aoe_on`, range in metres, `hit_list`, a light that fades in over 5 frames).

**Melee numbers** (`obj_spellMelee/Other_10.gml`): `dmg = strength/8 + irandom(strength/32)`; 1-in-20 doubles as crit; then `DamageHP(..., physical)` and `RestoreENERGY`.

**AI pick:** first spell in the book that is affordable and off cooldown becomes `autoAttack`. Order in the book *is* priority.

---

## Items, inventory, crafting

**Data:** `scripts/ItemList/ItemList.gml`

```
itemData: name, description, icon, usable, castAni,
          onUseObject, onDropObject, maxStack, cooldown
```

`global.itemRef[? "Apple"]` → row id. Chests and recipes should use the string key, never a raw number. Forty rows. Cooldowns: Apple 5 s, Grape 20, potions 60, keys 1, Poisoned Rat Meat 3.

**Bags:** two parallel lists, 24 slots, `-1` empty. Stack into existing, then first hole. Overflow returns leftover qty. *(Corrected)* "Inventory Full" prints only when **nothing** was added; a partial overflow prints `+N`.

**Use:** needs GCD clear. Spawn `onUseObject`; if `canBeUsed`, consume 1, start the per-item cooldown and the GCD, set `stop_timer = 30`.

**Hotbar:** `game.actionBarPositions` is a grid of `ABPOS` (x1,y1,x2,y2, qty, source, id, hovered, pressed). Source `1` = item, `2` = spell.

**Craft:** `scripts/craftMap/craftMap.gml`

- 3 input slots + 1 output
- Inputs sorted by item id, concatenated *display names* become the map key. Both sides derive the names from ids, so renaming a display name is safe; renaming an `itemRef` key silently breaks the recipe
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

**Unfinished:** **all eight** potions share `obj_onUseManaPotion1` (restore 25 MP). Most materials share `obj_drop_iron`. Keys are one object each (`obj_onUseKey_*`): named keys open the `instance_nearest` door at any range if its `key` string matches; the generic key needs a door with `key == ""` within 16 px. Keys are consumed. The House Key (found in the abandoned car) matches no door. The four flower pickups give Grape; `obj_drop_key_mine_boss` gives the Headmaster key.

**The one complete item puzzle:** Rat Meat (looted from rat corpses) + Small Stranglethorn Potion → Poisoned Rat Meat; using it throws it 64 px along the aim; an idle `obj_snake_1` within 80 px paths to it and takes 10000 nature damage.

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
- Location: `quest_requirement_enter_location(questName, locationNumber)` (called from `obj_quest_location`). The requirement's `map_x`, `map_y` and `zone` fields are stored and never read

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
- Every 10 frames: `get_nearest_enemy` in `aggroRange` metres, must be alive, enemy, LOS. **Bug:** the counter starts at `irandom(9)`, is decremented, then compared `== 0`; a unit that drew 0 never scans again
- Records its leash point as its current position each idle step
- If `patrol_path` exists, assign and `path_start`
- Enter combat if LOS + in range, or already have a living `current_target`

**Combat**

- Drop target if dead, farther than `1000 + leashRange*2` m, or the unit is past `leashRange` from its leash point
- Any processed damage from a targetable source sets `current_target` if the unit has none (first-hit aggro)
- Else pick spell, `CastSpell`
- Success: face target, stop path
- `tooFar` / `notInLOS`: `PathTo` target at `run_spd`, max path length `leashRange*2` m; fail → leash
- Occupy current cell so other AI path around this unit

**Leash**

- Clear target, path to `leash_to_x/y`
- Snap when close, restore patrol position/speed, `combat_state = idle`

**PathTo(grid, x, y, speed, maxDistMetres)**

- `mp_grid_path` with diagonals
- Accept only if path length < max distance
- If blocked, 4 cardinal offsets at 4, 8 and 12 px — **bug: uses `div 32` not cell 8**

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
| 3 | `obj_reveal_regeon` | fog-of-war: x, y, image scales, `mapped`, and `cam_grab` *(corrected: a camera-capture flag, not a "minimap grab"; no instance ever sets it)* |
| 4 | `obj_door` | `locked`, `key`, `spr`, `vertical` |

**`load_room()`**

- If `obj_load_game` exists, `room_goto(current_room)` then copy quests
- Destroy all units / usables / reveals / doors (after running Create — leftover cleanup)
- Recreate from maps via `asset_get_index(obj_name)`
- Units reload at **start** position, HP / MP / energy written as **max** at save time. Unit inventory is saved and never loaded
- Player keys: transform, `alive`, `hp`, `mp`, `energy`, `currentState`, `image_index`, `scale`, `direction`, `spirit`, `strength`, `entry_id`, `gcd`, and five lists (`inventory_items`, `inventory_qty`, `itemCooldownList`, `spellbook_IDs`, `spellbook_cooldowns`)

`vars` is `json_stringify` / `json_parse` of a per-instance struct — your escape hatch for one-off state.

---

## GUI and windows

**Owned by `game`**

```
WINDOW: INVENTORY, SPELLBOOK, QUESTLOG, MAP
```

Open interface pauses + darkens. *(Corrected)* Windows cycle with **D-pad left/right** on a pad and **A / D** on the keyboard, not bumpers or arrows.

**Hit-test grids** (same `ABPOS` schema)

- `actionBarPositions` — 8 slots
- `iconPositions` — inventory
- `craftPositions` — 3 in + 1 out
- `spellbookPositions` — 4 visible, `spellbookPage` (the hit-test ignores the page: it passes the visible index)

An action bar entry for a spell stores the **spellbook position**, not the spell id.

**Logic functions** in `Game_Functions.gml`: `actionbar_logic`, `inventory_logic`, `spellbook_logic`. Each takes `(cursor_x, cursor_y, input_type)` and duplicates KBM / gamepad / touch (touch empty).

**Draw**

- `objects/game/Draw_64.gml` — GUI
- `scripts/Interface_Functions/Interface_Functions.gml` — `draw_window` 9-slice via temp surface, tooltip frame
- `GUIDrawHealthBar`, `DrawItemIcon`, `DrawSpellIcon`
- `onscreen_message` — max 3 stacked lines, 180-frame life

`mouseOnGui` blocks **right-mouse-hold** walking *(corrected: not the right stick; click-to-move is commented out)*.

Also here and undocumented before: `game/Other_15.gml` sets GUI height to 256 and re-lays every hit-test grid on resize; `draw_item_text_frame` tooltips; `DrawIconCooldown`; floating combat text that stacks itself in 24 px steps; the target ring in `game/Draw_0.gml`; the chest loot popup (`obj_item_pickup_cutscene`, 180 frames with the player in the looting state).

---

## Lighting and time

**Config:** `scr_lighting_config()` — `light_scale`, `light_color`, `light_on`, offset, `follow`, `light_sprite`.

**Parents:** static solid, static, dynamic, dynamic solid. Spells can emit too.

**Pass:** `objects/game/Draw_73.gml` (End Draw)

1. Surface = view size
2. Clear `lighting_color` = `rgb(223,223,159)`, everywhere, not driven by `time`
3. `bm_subtract` each light sprite
4. *(Corrected)* draw the surface over the view **while still in `bm_subtract`**, then reset the blend mode and free the surface. So `lighting_color` is the amount subtracted from the scene, and lights cancel it

Light sprites are `spr_light_round_32/64/128/256`. Static lights `(128,96,32)`; wall torches `(225,192,128)` and lamp posts `(246,205,64)` on the 256; bolts on the 64, scale ramping 0→1 over 5 frames (fireball `(255,127,0)`); the bat carries a red 64. Dungeon and overworld differ only in which lights are placed. **Wall torches light on frost damage**; they are only dark when `!global.debug`, and debug is on.

`lighting_on` is a bool. `game.time` starts at 19, advances `1/7200` per step (2 real minutes per game hour at 60 fps) and wraps 24. *(Corrected)* **One thing reads it:** `obj_lampost/Step_0.gml` lights lamp posts when `time > 18.5 || time < 6.5`.

Dungeons vs overworld: `is_in_dungeon()` is currently `room != room_zone1`. Do not reuse that.

---

## Rooms, doors, triggers

**Zone change:** `objects/obj_change_zone_box/` — collision with player, alarms, fade-ish Draw End.

**Doors:** `obj_door`, `obj_door_x`, `obj_door_y`. `locked` + `key`, which is a **string tag**, not an item id *(corrected)*: `"HM"`, `"M_VAULT"`, `"MINE_BOSS"`, `"BASEMENT"`, `"Snake Key"`, `"Snake Boss Key"`; `""` with `locked = true` means "opened by the generic key"; the default is `"NOT LOCKED"`. `door_setup(vertical)`. A door is a tunnel: entering its end strip auto-walks the player through on the door's own 16 px `mp_grid`; a locked door adds cells to `collision_map`.

**Keys:** one `obj_onUseKey_*` per door family (house, generic, mine headmaster/vault/boss, auntie basement, snake, snake boss).

**Triggers worth knowing**

- `obj_trigger_room_lock_player_in` — locks every door overlapping its 176×176 rect while the player is inside, on a 900-frame alarm it keeps resetting. `unlock_and_destroy` is never set: **boss death unlocks nothing**
- `obj_trigger_lock_in_room1` — the good lock-in: locks a chest and a door, spawns four undead, releases when they die
- `obj_boss_room_trigger` — Create event only. Inert
- `obj_button_1` / `obj_leaver1` — a pressure plate any in-world object holds down (so barrels work; release re-locks after 2 frames) and a lever. Neither targets an id: the lever calls `instance_nearest(obj_track_corners).change_track()`, the buttons call closures the room creation code overrides against the nearest chest or door
- `obj_trigger_snake_boss_aggro`, 12 × `obj_trigger_snake_light_on` (each lights the wall torches inside it, once)
- `obj_trigger_floor_0` / `_1` — write `current_floor`, which **nothing reads**
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
| `obj_undead_root` | Untargetable path-grid wall that lashes 5–10 nature per second at 1.5 m. 107 in the burial. Dies with the flower in its `obj_kill_root_trigger` |
| `obj_undead_lily_pad` | *(Corrected)* not a unit: an `obj_dynamic_parent`. Bolt damage within 25 px slides it 8 px away from the player over water, where it becomes a walkable raft. Never moves west (bug) |
| `obj_snake_1`, `obj_UndeadSnake`, `obj_snake_boss` | Burial snakes |
| `obj_spider1_wall`, `obj_small_spider1`, `obj_guard_spider1`, `obj_spider_boss` | Web kit |
| `obj_rat` | *(Corrected)* **beast**, not undead (`obj_banditParent`). 100 HP, wanders, 1× Rat Meat looted from the corpse, which then destroys itself |
| `obj_snake_statue_2` | Stationary turret, `poisonbolt_0`. Four in the burial |

2020 stats, for scale (player is 140/140 = 700 HP, a phase-5 test value): skeleton, bat, pumpkin, cactus, flowers, soldiers strength 3–6 (15–30 HP); rat 20; `obj_snake_1` 120/120, leash 40, `destroy_on_death`; `obj_undead_miniboss` and `obj_undead_boss` 300 (1500 HP; stun + fast melee, and very slow fast melee); `obj_spider_boss` 800/60, stationary, timer-driven (web group, then web wrap at 6 s, eggs hatch every 20 s); `obj_snake_boss` 800/800 (4000 HP). Default walk 0.5 / run 1 px per step; soldiers 0.25 / 0.5; the player 1 / 2.

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

`interaction.*` is the action layer (`objects/input/Create_0.gml:171-219`). `input_check(interaction.USE)` etc.

| Action | Keyboard | Gamepad |
| --- | --- | --- |
| USE | F | face2 (B) |
| SPRINT | Shift | shoulderr |
| SPELL1 / 2 / 3 | 1 / 2 / 3 | face1 / face3 / face4 |
| SPELL4 / 5 | 4 / 5 | shoulderlb / shoulderrb (bound, never read) |
| UP / DOWN / LEFT / RIGHT | W / S / A / D | D-pad |
| PAUSE | Esc | start |
| OPEN_INTERFACE | Tab | select |
| DRAG_ICON | F | face1 |

On keyboard the hotbar reads literal keys 1–8, not the SPELL actions. On a pad SPELL1 fires slot 1, SPELL2 slot 0, SPELL3 slot 2. Mouse: hold right to walk, right-click an icon to use it, left drags, the wheel zooms (or pages the spellbook and quest log). Left stick moves (deadzone 0.2) and drives the GUI cursor at ×10. The legacy `keys` / `inputList` grids are filled and never read.

`game` Create:

- Mobile → GUI event 8 (touch)
- Desktop → events 6 and 7 (KBM + pad)
- Console → event 6 (pad)

Player movement accepts WASD/dpad *or* analogue *or* right-mouse hold as a virtual stick (`distance/32` → strength).

Sprint drains 0.5 energy/frame; carry 0.25; pushing 0.5; every other frame, walking included, regains 0.5 *(corrected: not only idle)*. Energy starts at 0. Hit 0 → `energy_empty` until full again.

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
- Range between bounds; cast validation order (range before cooldown); `stop_timer` on cast
- Aim-based player casting, non-homing bolts, bolt splash
- Frost lights torches; bait kills the unfightable snake; flowers hold roots; plates held by pushables; Repair costs `mats`
- The snake boss's timer phases, turn clamp, body nodes and forwarding hitboxes
- Lamp posts on the clock

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
