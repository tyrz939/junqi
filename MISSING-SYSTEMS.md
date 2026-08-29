# Junqi — Missing engine systems

Pair with `SYSTEMS.md` (the bar), `WORLDGEN.md` (how the county grows), `LEARNING-SYSTEMS.md` (2020 paths).

This file is the **engine** still sitting in jun7 and not in the remake: verbs you can drop on any zone, any seed, a larger game. A new cart, lever, fog rect, or ground hazard is a **row**. It is not a new class and not a per-unit special case.

**This file is not:** one-off bosses, one puzzle in the burial, one rose in the cellar, Auntie’s lines, Fireball’s room, or a wider enemy roster. Those use the engine. They are listed in the appendix so we do not confuse them with work to do twice.

**Rule (same as `SYSTEMS.md`):** add a JSON row. Do not add a TypeScript class for a new machine, trigger, tile flag, or spell kind.

**Status** matches `SYSTEMS.md`: **IN** / **SHAPE** / **LATER** / **NEVER**. Tile flags, triggers, snapshot save, load ring, pull, energy, toggle, drain, drag, and interior fog are **IN**. Cart, floors, pad, subtract, particles, and audio are still open.

---

## What counts as engine

| Engine | One-off (appendix) |
| --- | --- |
| A non-unit mover on a track flag | The two gold-mine carts |
| A toggle that targets an id | `obj_leaver1` as a unique object |
| A floor index the grid/path/map read | Button that hard-sets floor 0 |
| Tile flags `solid` / `blockLos` | `obj_water` as its own class |
| On-enter action list | `obj_trigger_rat_path_to_snake` |
| Push and pull on one tag | A second “pullable” parent |
| Live ring + chunked tiles | — |
| Per-zone instance snapshot | — |
| Reveal rects (optional floor) | Named burial fog pieces |
| Camera follow / lock / zoom / shake | Lock used only for the snake |
| One input action layer | Touch TODO copies in every GUI |
| Animation table `kind → frames` | Importing 364 GM strips |
| Spell *kind* `ground` | `obj_spellWebOnGround` as a class |
| Energy as one field (sprint, carry, melee restore) | Melee-only special case |

If you cannot reuse it on a second floor of a second dungeon without opening `PlayScene.ts`, it is not the main list.

---

## Snapshot

The remake already has the MMO-lite core: one unit, one cast pipeline, 24+8+3/1 bags, craft by id, three quest types, dialogue runner, AI leash, keys as tags, carry, repair, learn, room lock, clock → wash, one slot, KBM, dual camera.

What 2020 still has that a **larger** game needs:

| Engine | 2020 proved it with | Remake | Bar |
| --- | --- | --- | --- |
| Non-unit mover | Minecart + track | — | LATER (row: `machine.kind = cart`) |
| Toggle | Lever / button | `kind: toggle` + action list | **IN** |
| Drain flood | Lily / wet rooms | `drain:` action on a named rect | **IN** |
| Zone floors | `current_floor` 0/1 | Flat grid | SHAPE |
| Tile flags | Water `solid` + `block_los` | `TILE_FLAGS` table | **IN** |
| Trigger list | A dozen `obj_trigger_*` | `triggers.json` + `runActions` | **IN** |
| Push **and** pull | One `"pushable"` tag | Same `push` tag | **IN** |
| Distance unload | 768 px ring | Sleep + chunked tiles | **IN** |
| Snapshot save | `save_room` instance list | `zones[id]` in `junqi.slot0` (actors, props, optional `fog`) | **IN** |
| Fog / reveal | `obj_reveal_regeon` | Interior bitmap in `fog.ts`, saved on the zone snap. County is live radar | **IN** interiors / SHAPE county |
| Camera states | FOLLOW / LOCK / zoom / shake | Follow + small shake | LATER / SHAPE |
| Action-layer input | Pad + (unfinished) touch | KBM | LATER |
| Multi-slot | `slotN.jsav` | Slot 0 | LATER |
| Animation table | SpriteSet | Facing pose | SHAPE |
| Lighting surface | `bm_subtract` | Wash + ADD | SHAPE |
| Particles | One `part_system` | 3 textures | LATER |
| Audio bus | Zone → track | Silent | LATER |
| Drag ghost | `obj_itemMove` | Bag / bar / book in `drag.ts` | **IN** |
| Ground spell kind | Web on ground | Bolt + status | SHAPE |
| Energy contract | Sprint / carry / melee restore | Sprint / idle / melee restore / carry drain | **IN** |
| Hunger / warmth | — | `survive.ts` + item `food` / `warm` | **IN** |

---

## 1. World as data

These scale because the **county** grows, not because a unit is special.

### 1.1 Machines (mover / toggle)

One table. Two kinds 2020 actually ran. More kinds later are more rows.

```
id, kind: cart | toggle, cells[] | target, onUse[]
```

**Cart (non-unit mover).** 2020: `obj_minecart` + track pieces + stop trigger. Occupies cell 8. Player rides; `reach_end()` returns control. The cart can hit the same triggers as the player (lily, lock, light, spawn). The engine is: a mover that is not a `Unit`, on tiles flagged `track`.

**Toggle.** 2020: `obj_leaver1`, `obj_button_1`. One use path: run `onUse` against a target id (door, block, light, floor).

**Remake:** `kind: toggle` runs an action list. Factory, school, pipes, and county trials prove the lever. Pipes drain is a `drain:` action on a named flood rect, not a floor index. Cart is still LATER.

**Bar:** toggle **IN**. Cart LATER.

**Do not:** `class Minecart`, `class Lever`, a third parent for buttons. The 2020 mine carts and three levers are *content rows* that prove the table.

### 1.2 Zone floors

**2020:** `obj_player.current_floor` (0 or 1). Floor triggers and reveal rects set it. Path, occupy, and minimap change with it so the mine can overlap.

**Engine:** a zone may have `floors: N`. Path, LOS, occupy, reveal, and machines read `session.floor`. A trigger or toggle sets the index. Do not fake a second zone to stack geometry.

**Remake:** one grid per zone.

**Bar:** SHAPE when a dungeon is large enough to overlap.

### 1.3 Tile flags — IN

**2020:** `obj_water` is solid and `block_los = true`. `CheckLOS()` and projectiles read `block_los` on the line. Statics default `block_los = false`.

**Engine:** every tile (and solid prop) has `solid` and `blockLos`. Water is a row that sets both. Walls set both. Grass sets neither. Bushes may be solid only. Projectiles and path already exist — they must read the flags.

**Remake:** `TILE_FLAGS` on `Grid`. `grass` neither; `bush` solid only; `water` / `wall` / `house` both. `solid()`, `blocksLos()`, `blockedProjectile()` read the table. Extra-solid props (gates, crates) also block LOS.

**Bar:** **IN**. A pond in Auntie's yard proves bolts stop on water.

### 1.4 Trigger action list — IN

**2020** implemented this as many objects (`obj_trigger_room_lock_player_in`, `obj_boss_room_trigger`, rat spawn, rat path, kill-root, snake-light, snake-aggro, `obj_block_here`, quest location). The *engine* is one thing: **on enter, run an action list**.

Dialogue already has `quest:`, `beat:`, `set:`, `handin:`. Reuse that runner.

```
id, rect | cell, when: enter | use, actions[], once?, floor?
```

Actions the larger game will keep using: `lock`, `unlock`, `solid`, `spawn`, `despawn`, `path`, `light`, `aggro`, `floor`, `learn` (already a prop), `location` (already a quest type).

**Remake:** `triggers.json` + shared `runActions` (`actions.ts`). World verbs: `lock`, `unlock`, `solid`, `spawn`, `light`, `aggro`, `location`, `learn`. One enter-rect check in `PlayScene`. Kitchen / mine / burial lock-in are rows.

**Bar:** **IN**. Do not add `talkRatSpawn()`. Rat-path-to-snake is a **row** that uses `path` + `spawn`.

### 1.5 Push and pull — IN

**2020:** one tag `"pushable"`. Walk into it → push one cell. Face 180° → pull with a spacer cell. Same occupy update (`obj_player/Other_16.gml`).

**Engine:** occupy + one tag. Direction relative to facing. Not a second object family.

**Remake:** same `push` tag. Hold E: crate in front pushes; crate behind with a spacer cell pulls (move one cell along facing).

**Bar:** **IN**.

---

## 2. Session, sleep, camera, input

These scale because the **map** grows.

### 2.1 Distance unload + chunked tiles — IN

Full contract: `WORLDGEN.md` §6.

**2020:** `distance_unload` ticks before units. On block change (16 px): sleep dynamics / statics / usables / **idle** AI; wake a 768 px square; combat stays up. Activate-all before save, load, zone change.

**Engine:** generate the whole zone, tick a ring. Chunk tiles to the same ring. Specs stay in the blueprint while sprites are down. Never persist only the ring.

**Remake:** on player block change (16 px), sleep idle AI and far props (skip tick, hide, drop occupy). Combat, locked gates, carried crates, bolts, and the speaker stay awake. Tiles draw in 16-cell chunks; far chunk textures are destroyed. F5 / zone leave wake-all, snapshot, then dirty the last block.

**Bar:** **IN**. Required before the two-camp size jump. Town must sleep in Auntie's yard once camps exist.

### 2.2 Per-zone snapshot save — IN

**2020 `save_room()`:** player map + a list of instances (units, usables, reveal rects, doors) with live xy. Load recreated from that list.

**Bugs to keep dead:** HP written as `maxhp`; load at `xstart`/`ystart`.

**Engine:** one slot schema. Per zone: actors `{id, hp, mp, x, y, …}`, props `{id, used, locked}`, reveals, floor. Wake-all, write all, sleep again.

**Remake:** same `junqi.slot0`. `zones[zoneId] = { actors: {id, hp, x, y, …}[], props: {id, used, locked, x, y}[] }`. On zone leave / F5 write the current zone from live actors (wake-all first). On enter, apply by id over the generated blueprint. Do not store `maxhp`. `dead` still keeps quest units from returning.

**Bar:** **IN**. A larger game cannot treat “left the camera” as “never existed.”

### 2.3 Fog / reveal — IN interiors

**2020:** `obj_reveal_regeon` rectangles stamp the minimap and may set floor. Dozens in mine and burial.

**Engine:** walk a cell, mark it seen, persist the bitmap with the zone snap. Floor-aware when §1.2 exists. County can stay a live radar — 2000×1200 does not need a saved fog.

**Remake:** `fog.ts` stamps an 8-cell radius on interiors and packs it onto `zones[id].fog`. County skips the bitmap. `session.visited` still names the zones you have entered.

**Bar:** **IN** for interiors. County radar is the intended leftover, not a hole.

### 2.4 Camera states — SHAPE / LATER

**2020:** `CAM_STATE.FOLLOW | LOCK`, `camera_zoom`, region clamp, `camera_shake`. One camera object.

**Engine:** `{mode: follow | lock, zoom, bounds, shake}`. Boss rooms and interiors lock; zoom is a setting; shake is a call from incoming damage. Do not grow a camera class per room.

**Remake:** follow, zoom 4, small shake.

**Bar:** lock is SHAPE (any boss pocket). Zoom / pad-driven zoom is LATER.

### 2.5 Zone travel chrome — LATER

**2020:** `obj_change_zone_box` fade, then `save_room` + `room_goto`. Pairing is `door_id` + `place_x/y`.

**Engine:** travel is already IN. Fade is polish on that verb. Keep door marks from `WORLDGEN.md`.

**Remake:** instant `PlayScene` restart.

### 2.6 Input action layer — LATER

**2020:** `input_check(interaction.USE)` walks keyboard then gamepads. GUI events 6/7/8 by device. Touch empty.

**Engine:** one action layer. Device is a source, not a second inventory or a second HUD.

**Remake:** KBM + RMB stick.

### 2.7 Slots — LATER

**2020:** `slotN.jsav`. Title Options TODO.

**Engine:** the save shape in §2.2, N slots. Slot 0 ships the county.

---

## 3. Presentation engines

These scale because **every** unit, spell, and zone uses them. They are not art one-offs.

### 3.1 Animation table — SHAPE

**2020 SpriteSet:** `kind → { s_front/back/left/right, f_idle/walk/cast/attack/shoot/hurt/dead/loot: [start, end] }`.

**Engine:** that table. New enemy = a row (or generated frames with the same keys). Walk/cast/hurt/dead are states you already have on `Unit`.

**Remake:** one generated pose per facing.

**Not the engine:** importing the 364 GM sprites. That is LATER art. The cactus `break` bug stays dead.

### 3.2 Lighting surface — SHAPE

**2020:** view-sized surface, `bm_subtract` light sprites, free each frame. Clock did **not** drive color.

**Engine:** one lighting pass. Lights are data on props/spells (`radius`, `sprite`/`color`). Clock already drives wash in the remake — keep that. Subtract (or a modern equivalent) is the missing renderer, not a per-torch object.

**Remake:** wash + ADD circles.

### 3.3 Particle system — LATER

**2020:** one `part_system`, types in `ParticleList`, projectiles emit by school.

**Engine:** `school | override → emitter`. New spell does not need a new renderer (`vfx.ts` already says this for bolts).

**Remake:** bolt / slash / spark textures.

### 3.4 Audio bus — LATER

**2020:** two tracks. The *engine* is `zone | combat | title → track`, not those two files.

**Remake:** silent.

### 3.5 Drag ghost — IN

**2020:** `obj_itemMove` with a from-enum (bar / bag / book / craft).

**Engine:** one ghost for bag / bar / book. Keys and Julie's letter refuse destroy.

**Remake:** `drag.ts` + press-and-move in `Gui` / `ActionBar`. Click-bind still works.

**Bar:** **IN**. Craft-grid drag is leftover polish, not a missing engine.

---

## 4. Combat engines (shared, not per-unit)

### 4.1 Spell kind `ground`

**2020:** lingering web on the floor, group web, wrap. Same pipeline as a bolt: validate, pay, spawn a *kind*.

**Engine:** add `ground` next to melee / bolt / self / world. Duration, school, status. JSON row. The spider kit is content that uses it.

**Remake:** `webshot_ai` is bolt + `onHit: root`.

### 4.2 Energy as one field — IN

**2020:** sprint 0.5/frame, carry 0.25, idle 0.5, empty lockout until full, melee `RestoreENERGY`. Formula and crit are already IN.

**Engine:** every cost and restore writes `unit.energy`. Carry and melee are not special objects; they are callers.

**Remake:** sprint 30/s, idle regen 30/s, carry drain 15/s, melee hit restores 16. Empty lockout until full still on the same field.

**Bar:** **IN**.

---

## 5. Already IN — do not rebuild

- Session, pause, terminal, F2/F3/F5
- Unit vitals, GCD, occupy, incoming by school, resist, `effects.json`
- Cast validate → pay → melee/bolt/self/world; AI `tryCast`
- 24 / 8 / 3+1, recipes by **id**, ground drops
- Quests kill / acquire / location, beats, dialogue runner
- AI idle / combat / leash, A* cell 8
- Keys as `opens`, chest once, push + pull, carry, repair, learn, room lock, location
- Tile flags, trigger action list, per-zone snapshot, load ring, energy callers
- Toggle, drain, Grow, interior fog, bag/bar/book drag
- Hunger / warmth, six towns, post-snake Zelda rooms
- Dual cam, HUD, four windows, RMB stick, mouseOnGui
- Clock → night wash + warmth

Bugs that stay dead: cactus SpriteSet fallthrough; CastSpell `alive` before exists; PathTo `div 32`; save HP as maxhp; `moveToX/Y` swap; recipes by display name; clock with no output.

---

## 6. Do not port

| Thing | Why |
| --- | --- |
| TweenGMS | Third party |
| `Resolution_List` | Dead |
| `obj_onUseKey_*`, `obj_bench1`–`9` | One tag / one sprite. The *anti*-pattern this file exists to avoid |
| Recipes keyed by display name | Fragile |
| WoW quest paste | Content leftover |
| Title skip, `global.debug = true` | Off |
| A class per 2020 trigger object | That is what `triggers.json` replaces |

---

## 7. What to add when (engine only)

| First (done) | Still open on the county | When a room needs the verb |
| --- | --- | --- |
| Snapshot save (live HP/xy + interior fog) **IN** | Cart on a track | Ground spell kind |
| Trigger action list **IN** | Zone floors when a mine overlaps | Particles, audio bus |
| Tile flags `solid` / `blockLos` **IN** | Camera lock as a state | Pad, extra slots |
| Toggle + drain **IN** | Lighting surface | Zone fade |
| Pull / energy / load ring **IN** | Animation table | — |
| Drag ghost **IN** | County fog bitmap (optional) | — |

Do not invent a seventh dungeon to exercise a row. Factory / school / butterfly / pipes already prove toggle, clear, Grow, drain, and Repair. Reuse those rows on the county trials.

---

## 8. File map (engine hole)

| 2020 | Engine it was | Remake hole |
| --- | --- | --- |
| `distance_unload` | Live ring | `loadRing.ts` **IN** |
| `save_room` / `load_room` | Instance snapshot | `session.zones` **IN** |
| `obj_minecart` + tracks | Machine `cart` | — |
| `obj_leaver1` / `obj_button_1` | Machine `toggle` | `kind: toggle` **IN** |
| Wet rooms / lily | Drain flood | `drain:` **IN** |
| `current_floor` | Zone floors | — |
| `obj_water` + `block_los` | Tile flags | `TILE_FLAGS` **IN** |
| `obj_trigger_*` family | Action list | `triggers.json` **IN** |
| Push + pull | One occupy tag | Same `push` tag **IN** |
| `obj_reveal_regeon` | Fog / reveal | `fog.ts` interiors **IN**; county radar |
| `CAM_STATE` + zoom | Camera states | Follow |
| `input` pad / touch | Action layer | KBM |
| `SpriteSet` | Animation table | Facing |
| Draw_73 subtract | Lighting pass | Wash + ADD |
| `ParticleList` | Emitter table | 3 textures |
| Two `.ogg` | Audio bus | — |
| `obj_itemMove` | Drag ghost | `drag.ts` **IN** |
| `obj_spellWebOnGround` | Spell kind `ground` | Bolt + status |
| Melee `RestoreENERGY` | Energy callers | Sprint / carry / melee **IN** |

---

## Appendix — 2020 one-offs (not the job)

These are **uses** of the engine, or the one permitted special-case. Do not promote them to a second AI, a pad class, or a grow class. When you need them, add a row or the single boss mover.

| 2020 thing | Uses | Notes |
| --- | --- | --- |
| Two mine carts, three levers | Machines | Content density in `WORLDGEN.md` |
| Lily pads + snake torches | Triggers + water flags + optional cart | One burial puzzle. `SYSTEMS.md` LATER |
| `obj_rose_growing_zone` | Trigger + clock | One cellar patch |
| `obj_snake_boss` / `move_snake()` | The one custom mover (`SYSTEMS.md` SHAPE) | Until then, generic AI + lock |
| Bat / pumpkin / cactus / soldiers / flowers / spider boss | Enemy **rows** | Table is as wide as 2020; do not author a wing |
| Fireball at (560, 1352) | `prop.learn` | Wrong pocket today — `WORLDGEN.md`, not a new system |
| County topology | Zone travel (IN) | Hatch = cellar; yard doors = mine / burial |
| Auntie as a speaker | Dialogue runner (IN) | A `dialogue.json` node |
| `music_dungeon1` / `bgm_butterfly1` | Audio bus | Two files, not two systems |
| Butterfly forest | Grow + authored graph | On the spine after the snake. Grow is the verb. NEVER as a spider wing |

---

## Source

- `LEARNING.md`, `LEARNING-SYSTEMS.md`, `SYSTEMS.md`, `WORLDGEN.md`
- `Junqi-Legacy-GM/jun7/objects/distance_unload`, `obj_minecart`, `obj_player/Other_16.gml`, `obj_water`, `obj_reveal_regeon`
- `Junqi-Legacy-GM/jun7/scripts/save_room`, `SpriteSet`, `ParticleList`, `Camera_functions`
- Remake: `junqi/src/game/systems/*`, `world/generate*.ts`
