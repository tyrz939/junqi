# Jane Legacy (2020) — Learning Notes

Hand-coded GameMaker Studio 2 project, folder `Junqi-Legacy-GM/jun7`. Internal name is **jun7**. Windows exe ships as **J&J RPG** (`J&J corp`, `(c) 2020`, description *"A RPG game to make smile."*). Mac/Linux still say JUN7. Quest copy in code calls the player **JoJo**.

The build in `jane/` is the live product (`ENGINE.md`). The player there is **Jane**; Auntie is **Julie**. Those are the **2020 names**: `Story.docx` says Jane and "aunty Julie" throughout, and "JoJo" appears in no design document, only in test quest strings (`DESIGN-2020.md` §1). This file stays 2020 *code* history, so JoJo stays in the quotes.

**Audited against the source, September 2026.** Every checkable claim below was verified in the GML. Where an earlier version of this file was wrong it has been corrected in place and marked *(corrected)*. The audit also found mechanics and bugs this file never mentioned; they are in the last two sections.

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
| Sounds | 2 (neither is ever played: the one `audio_play` call is commented out) |
| Paths | 14 |

About 35 of the 212 scripts are the project's own; the rest are TweenGMS.

**World rooms**

| Room | Size | Role |
| --- | --- | --- |
| `room_start` | 1024×768 | Boot / title. Places `objStart`, which creates `input`, `objStartScreen`, `terminal`, `debug`, then destroys itself |
| `room_parent` | 1366×768 | Inherited template (only 4 rooms inherit it). `RoomCreationCode.gml` is referenced but missing on disk; `room_zone1` and `room_dungeon_goldmine` reference missing creation code too |
| `room_zone1` | 5120×3072 | Overworld / town |
| `room_building1` | 1024×1024 | Orphan test interior: 10 walls, a door, an empty chest, an exit with `door_id` 3 that no zone1 box pairs with |
| `room_building2_auntie` | 512×512 | Auntie's house (fruit, craft bench) |
| `room_basement_auntie` | 1024×1024 | Auntie's basement |
| `room_dungeon_goldmine` | 2048×2048 | Gold mine |
| `room_dungeon_burial_chamber` | 2048×2048 | Burial chamber (most authored content) |
| `room_dungeon_butterfly_forest` | 3072×2048 | **Empty shell** *(corrected)*: a filled floor layer, a room init and one exit. Nothing else was ever placed |

**What the game actually is**

A Zelda-like overworld with WoW-adjacent systems: GCD, spellbook, action bar, leash AI, kill/acquire/location quests, potion crafting, keys, chests, day clock, subtract lighting, push/pull/carry, and slot saves. The player is a mage (`creatureClass.mage`) starting with melee, Icebolt, and Repair, with strength and spirit 140 (700 HP). Those are **end-game test numbers**: the 2020 balance sheet puts 700 HP at phase 5 of 6, "Start Burial Chamber", and 150 HP at the start (`DESIGN-2020.md` §3.1). The authored story cluster is **Auntie / house / basement / gold mine / burial chamber**, with undead plants, snakes, spiders, and a segmented snake boss.

---

## What 2020-you got right

These are the ideas worth keeping if you remake Jane.

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

`scr_ai_logic.gml`: idle (regen + patrol + throttled aggro), combat (pick first affordable spell, walk in if too far / no LOS), leashing (run home, resume patrol). Units **in combat** mark their path cell so others route round them *(corrected: idle and patrolling units only record the cell, they never block it)*. Aggro is sampled every 10 frames, staggered with `irandom(9)` — and that stagger has a bug, see below.

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
- Web spells carry the comment `TODO DEBUFF SLOW ON PLAYER`, but the slow **is** implemented *(corrected)*: `obj_spellWebOnGround` sets `speed_multiplier = 0.25` for 61 frames.
- Touch input: six `TODO`s (five in `Game_Functions.gml`, one in `Quest_Functions.gml`) and an empty touch event *(corrected: not "every GUI path")*.
- `global.debug = true` left on. Wall torches are only dark when debug is off, so nobody ever saw them dark.
- Two sounds in the project and the game is **silent**: the only play call is commented out.
- All eight potions are one object that restores 25 MP. No potion effect is written down anywhere, in code or in the design folder.
- The Apple says "Restore 25HP" and heals 25% of max HP (175 for that 700 HP player).
- The four flower pickups all give Grape. `obj_drop_key_mine_boss` gives the Headmaster key. The House Key is consumed and does nothing; the house door is not locked.

The engine could carry a small campaign. The campaign was never written.

### GameMaker-era data structures

`ds_grid` / `ds_list` / `ds_map` everywhere, plus `ds_list_mark_as_list` and nested maps for save. In 2020 that was correct. Today structs + arrays (or a real data file) are simpler, leak less, and JSON better. A remake should not port the `ds_*` graph.

### Surfaces created every frame

`game` Draw End builds a lighting surface, subtracts every light sprite, draws it, then `surface_free`. `draw_window()` does the same for every window chrome. That is easy and leak-safe. It is also expensive. Cache one lighting surface and one 9-slice surface, or draw 9-slice without a surface.

### `object_check_parents` walks 200 parents

GMS has `object_is_ancestor()`. The 200-iteration climb is a tell that you were solving the language from first principles. Fine then. Delete it now.

### Room inheritance + instance creation code as content

`room_dungeon_burial_chamber` and `room_dungeon_goldmine` each carry **64** `InstanceCreationCode_inst_*.gml` files *(corrected from "80")*, three of them defining `press_button()` locally. That is how you ship a dungeon in GM. It does not migrate. Any remake needs room data as data, not as 128 one-off instance scripts.

---

## Concrete bugs to remember

These are real, not style nits.

1. **Player Create swaps axes** — `moveToX = y` and `moveToY = x` in `obj_player/Create_0.gml`. Harmless while click-to-move is commented out. It is still a landmine.

2. **Missing `break` in `SpriteSet`** — `sprite.undead_cactus` falls through into `undead_soldier1`. Cactus uses soldier sprites.

3. **`CastSpell` can crash** — the first dereference is `is_enemy(current_target)`, which reads `.faction` *(corrected: not `.alive`, which comes two lines later)*, and `instance_exists` is checked after both. `spellErrors.noTarget` is therefore unreachable. It is latent for the player, because no player spell has `requires_target` and every assignment to the player's `current_target` is commented out; an AI caster always runs the block.

4. **`PathTo` fallback uses cell 32** — main grid is 8px (`global.path_cell_size`). The "nudge to nearest free cell" loop does `xto div 32`. Path recovery is looking at the wrong grid. It tries three rings (4, 8, 12 px) *(corrected from "4 steps")* and only runs when `mp_grid_path` fails.

5. **Save stores unit HP as `maxhp`** — `ds_map_add(_map, "hp", maxhp)` in `save_room`. Enemies reload full. Combined with load using `xstart`/`ystart` instead of live `x`/`y`, combat state is not actually saved.

6. **`is_in_dungeon()` is a one-liner lie** — `return room != room_zone1`. Auntie's house and basement count as dungeons.

7. **Spellbook paging disagrees with itself** — controller pages by `/ 4`, mouse wheel by `/ 8`.

8. **`inventoryGetNextPosition`** — `ds_list_find_index` returns a real, never an array. The `is_array(position)` branch is dead.

9. **Day clock barely drives anything** — `time += 1/7200` toward a 24-hour cycle, starting at 19, but `lighting_color` is a fixed `rgb(223,223,159)`. *(Corrected: the clock has exactly one consumer.)* `obj_lampost/Step_0.gml` lights lamp posts when `time > 18.5 || time < 6.5`. There is no day/night tint.

10. **`CostMP` / `CostEnergy` return remaining resource, not the cost paid.** Callers ignore the return. Fine, but the names lie.

11. **`Resolution_List()` is dead.** Written, never called.

12. **Title Load / Options are `TODO`.** `objStartScreen` can skip the menu (`pressed = true`). Slot load exists; the title does not wire it.

13. **Melee formula** (`obj_spellMelee`): `strength/8 + irandom(strength/32)`, 1-in-20 double crit; a hit refunds 3 energy, a crit 9. Useful if you remake combat numbers.

**Found by the 2026 audit, not in earlier versions of this list:**

14. **The player's melee does no damage.** `obj_spellMelee/Other_10.gml` does `with caster { target = get_nearest_enemy(x, y, 16) }`, which writes the *caster's* `target`, not the spell's. With no `current_target` (the player never has one) it calls `DamageHP(noone, …)`. AI melee works.
15. **Some units never aggro.** `check_aggro_rng = irandom(9)` can be 0. The Step does `check_aggro_rng--` and then tests `== 0`, so a unit that drew 0 goes to −1 and never scans for a target again.
16. **No statuses, no resistances.** `damage[]` and `resistances[]` are never read. Icebolt does not chill, "Root" does not root, poison bolts never set `dmg_type` and land as physical. "slow" in a spell name means a slow projectile. The only crowd control is `speed_multiplier`, which only the player's movement reads, so AI cannot be slowed or stunned.
17. **Web wrap may never expire** *(unverified in-engine)*: its User Event does `timer--` and its collision event, which runs later the same step while the player overlaps it, does `timer++`.
18. **Zoom is unclamped.** The clamp to `camera_min`/`camera_max` sits inside a comment; the wheel moves `cameraHeight` by 64 with no floor.
19. **`CheckLOS` leaks its `ds_list`** on the false path. Unit inventory is saved and never loaded. Quest lists are written with `ds_map_add_list` every save without deleting the old entry *(save correctness unverified at runtime)*.
20. **Spellbook hit-test ignores the page**: `PlayerCastSpell(i)` and `drag_icon(2, i)` take the visible slot index.
21. **Lily pads never move west.** The 180° case tests `bbox_right - 1`, which hits the pad itself.
22. **`obj_banditParent` sets `faction = factions.beast`.** `factions.bandit` is assigned nowhere, so "bandits" and rats also fight the undead.

---

## Architecture in one page

```
room_start
    -> persistent-ish controllers: game, input, collision_map, camera, terminal, debug
    -> obj_player (child of obj_unitParent)

game.Step                                  (verified order)
    PAUSE pressed? toggle pause, close GUI
    OS paused? force pause
    apply deferred pause_next_frame
    resize (user 5)
    if !paused:
        time of day
        distance_unload (user 0)           <- before units
        obj_dialogue user 0                (no such event: no-op; dialogue runs on its own Step)
        GUI by input.current_input         (KBM user 7, pad user 6, touch user 8)
        if paused exit                     (GUI may have paused)
        zoom (user 4)
        game user 0                        (no such event: no-op)
        units alive? user 0 : user 1
        spells, on-use items, dynamics, drops (user 0)
        units: drain incoming damage (user 5)
        camera (user 0)
        floating text, part_system_update
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

## Mechanics this file never mentioned

Found by the 2026 audit. Each is a contract worth knowing about; several are better ideas than anything in the lists above.

- **The player aims; it does not target.** Every assignment to the player's `current_target` is commented out (`obj_player/Other_16.gml:219-273`). Player projectiles fly along `spell_direction`, the direction to the mouse; the pad layout says "Right stick: Aim". The Trello card "URGENT: remove targeting, make direction-based spell aim" (4 March 2021) was **done**. Only AI units use `current_target`. Projectiles never home (`homing = false`): they aim once, which is what makes a 2 px/step poison bolt dodgeable.
- **Bolts splash.** `obj_spell_projectile_parent`: the full hit on the body it touches, plus dmg/5 to each enemy inside a 25 px circle along the flight path (`aoe_on`). Icebolt and Fireball cannot crit; five other spell objects can (5%, ×2).
- **Range is measured between bounds.** `GetDistanceBetweenBounds` = `point_distance/8 − (boundbox_a + boundbox_b)`, default `boundbox` 2 m. "Range 0" melee therefore connects at ≤ 32 px centre distance. One metre is 8 px.
- **Frost lights torches.** `obj_light_wall_torch_parent/Other_24.gml`: a wall torch switches on when it takes frost damage. Icebolt is a light switch. The burial's design note says "Blue flames".
- **The snakes you cannot fight, you feed.** `obj_snake_1` has 600 HP and spits 120–180 a bolt from 40 m. Idle, it paths to any Poisoned Rat Meat drop within 80 px and takes 10000 nature damage on reaching it. Rats drop meat; Stranglethorn potion + Rat Meat crafts the bait; using it throws it 64 px along the aim. It is the only complete puzzle loop in the project and no doc had noticed it.
- **Flowers hold root walls shut.** 107 `obj_undead_root` are untargetable path-grid walls (bolts pass over them). `obj_kill_root_trigger` destroys the roots in its rect once the flower inside it is dead.
- **Pressure plates take anything.** `obj_button_1` is pressed by any `obj_in_world_parent`, so a pushed barrel holds it. Release re-locks after 2 frames. The mine's first chest is only open while the plate is down.
- **Repair costs materials.** `obj_spellRepair` finds the nearest `obj_repairable_parent`, needs the player within bounds and holding every entry of its `mats` list (broken track: 4 Iron; broken steps: 2 Wood), consumes them, and is `valid = false` (no cost, no cooldown) otherwise.
- **Push is a hold, not a bump.** Hold USE hard against a `"pushable"` for 30 frames with ≥ 20 energy; the object then slides 8 px at 0.2 px/frame with the player, draining 0.5 energy/frame (20 per cell). Input 180° from facing pulls, and the free-rect check adds one spacer cell behind the player. Carry is a separate `"pickup"` tag with a grid-snapped put-down.
- **Energy:** starts at 0. Sprint −0.5, carry −0.25, push −0.5, anything else (walking included) +0.5 per frame. `energy_empty` locks sprint / push / pickup until back to 100. No spell costs energy. A melee hit refunds 3.
- **Regen:** every living unit regains `spirit/1000` MP per frame. The player has **no HP regen at all**. AI out of combat heals `max/300` per frame (full in 5 s).
- **A cast roots you:** `stop_timer = 30` frames on every cast and every item use; item use also needs GCD clear and starts it.
- **Validation order** in `CastSpell`: dead → target validity → LOS → range → cooldown → GCD → MP → energy. Range before cooldown is what keeps an AI walking while its swing cools down.
- **Interact highlight:** the nearest usable / lootable / talkable thing within a 5 m scan gets `light_up`. User Event 6 is the interact hook on units (talk, or loot a corpse), usables, chests and drops.
- **Corpses are looted, not exploded.** A dead unit's inventory moves to the player on interact. Drops live 18000 frames. There is no random loot: Rat Meat, and one skeleton carrying a key.
- **Room lock-ins are timed, not boss-driven.** `obj_trigger_room_lock_player_in` locks every door overlapping its 176×176 rect while the player is inside, on a 900-frame alarm it keeps resetting; `unlock_and_destroy` is never set, so a boss dying unlocks nothing. `obj_trigger_lock_in_room1` is the good one: it locks a chest and a door, spawns four undead, and releases when they are dead.
- **The snake boss** alternates on timers, not HP: 900 steps chasing at 0.75 px/step with a ±3°/step turn clamp, then home and 300 steps of 15-bolt rings; it **rebuilds itself at full HP** if a wall crosses the line to the player; a body node every 4th moving step (64 max), every 8th node a dummy hitbox that forwards damage to the head; drawn as a tapering tri-strip into a fixed 192² surface, so it only renders in its own room.
- **Doors are tunnels.** Entering a door's end strip auto-walks the player through on the door's private 16 px `mp_grid`.
- **Zone travel** fades 30 frames out, `save_room`, `room_goto`, 30 frames in while paused; the arrival box with `entry_id == door_id` places the player.
- **Map:** `map_player` keeps an RGBA buffer at one pixel per 8 px cell. A reveal rect's first touch fills its bbox, leaving wall cells dark. The overworld is pre-revealed.
- **Camera:** follow is a 20-frame EaseOutQuad tween. A reveal region with `cam_grab` would capture the camera (40-frame tween, pad 128×64), but no instance sets `cam_grab` and `CAM_STATE.LOCK` is an empty case. Shake is `(2,2)` on every hit dealt or taken, plus `(2,3)` when taken.
- **Action bar entries for spells store the spellbook position**, not the spell id.
- **View:** 455 × 256 (`cameraHeight` 256, width follows the window aspect); GUI height 256.

---

## Lessons for a remake

1. **You already designed the game.** Do not start by inventing a new combat model. Port the *contracts*: item row, spell row, incoming damage, quest requirement types, room snapshot.

2. **Content was the bottleneck, not engine.** One NPC, one test quest, two dungeon rooms that mattered, no sound. A remake wins by finishing Auntie's loop (house → basement → mine → burial chamber → snake boss → reward), not by adding a sixth system. *Know what that loop is:* it is what 2020 **built**, not the order 2020 **designed** (mine → museum → butterfly forest → factory → burial → school; `DESIGN-2020.md` §4.1). The 2026 Phaser remake then proved the opposite failure: 102 quests, 52 enemies and twelve zones on an engine nobody had tested (`archive/phaser-remake-2026/POSTMORTEM.md`).

3. **Keys-as-objects do not scale.** `obj_onUseKey_house_auntie`, `obj_onUseKey_mine_vault`, `obj_onUseKey_BC_snake_boss`… The *door* side was already right: `obj_door.key` is a string tag (`"HM"`, `"M_VAULT"`, `"MINE_BOSS"`, `"BASEMENT"`, `"Snake Key"`, `"Snake Boss Key"`, `""` = opened by the generic key, `"NOT LOCKED"` = the default). Only the key side is one object per key, and each named key tests `instance_nearest` door at **any range**. That should be one key item with an `opens` tag. Same for benches: `obj_bench1` through `obj_bench9` should be one object plus a sprite.

4. **Crafting key-from-sorted-names is clever and fragile.** `craftSort()` concatenates item *display names*. *(Corrected: renaming a display name does **not** break a recipe, because both the table and the runtime lookup derive names from item ids and stay in sync. Renaming the `itemRef` key is what breaks it, silently: the ingredient resolves to `undefined`.)* Recipes should be sorted IDs, not strings.

5. **Enums inside `obj_unitParent` Create** (`factions`, `creatureState`, …) work in GMS 2.3+ but they belong next to `init`. You already put `itemData` and `spell` in scripts. Finish that move.

6. **Keep the terminal.** `println` into an in-game console is how you debug an RPG. A remake still wants a command line (`give`, `tp`, `quest`, `god`).

7. **Do not port GML line-for-line.** Port the systems list in `LEARNING-SYSTEMS.md`. Rewrite data as structs or a spreadsheet. Lighting should be one surface. Pathfinding should use one cell size everywhere.

8. **The 2020 taste was "systems-first MMO-lite in a tiny town."** That is still a good Jane. Auntie, dog, keys, mine, undead garden, snake temple. Lean into that, not a generic fantasy checklist.

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

If you rebuild Jane, steal the systems, finish Auntie's story, and leave the `ds_list` era in this folder.

That rebuild exists now: `jane/`, described in `ENGINE.md`. It took the contracts in this file, the puzzle loops the audit dug up (frost torches, fed snakes, held plates, flower-held roots, Repair that costs wood), and none of the GML.
