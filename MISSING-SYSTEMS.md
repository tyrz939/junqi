# Jane — Missing engine systems

Pair with `SYSTEMS.md` (the bar), `ENGINE.md` (the live engine), `WORLDGEN.md` (zones), `LEARNING-SYSTEMS.md` (2020 paths).

This file is the **engine** that 2020's jun7 had, or wanted, and the live build does not carry yet: verbs you can drop on any zone, any seed, a larger game. A new cart, lever, fog rect or ground hazard is a **row**. It is not a new class and not a per-unit special case.

**This file is not** one-off bosses, one puzzle in the burial, Auntie's lines, or a wider enemy roster. Those *use* the engine; they are in the appendix so nobody does them twice.

**Rule (same as `SYSTEMS.md`):** add a row. **Rule 0 applies here too:** nothing below is **IN** unless a test names it.

Rewritten September 2026 after two audits. The first found that several things this file said 2020 "proved" were never wired up in 2020 at all; those are marked *(corrected)*. The second found that several things the previous version marked IN in the Phaser remake were partial or inert (`archive/phaser-remake-2026/POSTMORTEM.md`).

---

## What counts as engine

| Engine | One-off (appendix) |
| --- | --- |
| A non-unit mover on a track flag | The two gold-mine carts |
| A plate / lever that runs an action list | `obj_leaver1` calling `instance_nearest(obj_track_corners)` |
| Tile flags `solid` / `blockLos` / `water` | `obj_water` as its own class |
| On-enter / while action list with a death reset | `obj_trigger_rat_path_to_snake` |
| Push, pull, carry on capability flags | A second "pullable" parent |
| A school that switches a prop on | Wall torches that happen to read frost damage |
| A `bait` field | `obj_snake_1`'s hand-written idle branch |
| Load ring over a whole-zone state | — |
| Camera follow / lock-to-rect / shake | A lock that 2020 declared and never set |
| One input action layer | Touch `TODO` copies in every GUI function |
| Animation table `kind → frames` | Importing 364 purchased sprites |
| Spell kinds `bolt` (fan, ring, splash) and `ground` | `obj_spellWebOnGround`, `obj_spellCactus`, `obj_spellSnakeBoss` as classes |

If you cannot reuse it on a second floor of a second dungeon without opening `sim/`, it is not the main list.

---

## Snapshot

| Engine | What 2020 actually had | Live | Bar |
| --- | --- | --- | --- |
| Fixed-step deterministic sim, replay | 60 fps frames; no determinism | `sim/`, `test/replay` | **IN** |
| One state tree = the save | `save_room` copied fields (and got `hp` wrong) | `GameState` | **IN** |
| Tile flags | *(Corrected)* `obj_water.block_los = true` was **never read**: `CheckLOS` and projectiles only test `obj_static_solid_parent`. Water blocked feet only | `TILE_FLAGS`; water is solid, not sight-blocking | **IN** |
| Trigger action list | A dozen `obj_trigger_*` | `triggers.json`: `enter` / `while`, conditions, `reset` | **IN** |
| Pressure plate | `obj_button_1`: any in-world object holds it; release re-locks | `plate` prop flag, `use` / `release` lists | **IN** |
| Lever / toggle | *(Corrected)* no target id: nearest track corner, or closures overridden in room creation code | prop `use` list + `on` state | **SHAPE** — nothing placed |
| Keys | door `key` string tag (good); one object per key (bad); named keys open the nearest door at **any range** | `opens` / `keyTag`; you must be at the door | **IN** |
| Repair with materials | `obj_repairable_parent.mats` | `world` spell kind, `answers`, `needs` | **IN** |
| School switches a prop | frost lights wall torches | `answers: <school>` + `lightWhenOn` | **IN** |
| Bait | `obj_snake_1` + Poisoned Rat Meat | unit `bait` field, item `throw` action | **IN** |
| Push / pull / carry | hold USE 30 frames, 20 energy, spacer cell; `"pickup"` tag | `push` / `carry` flags; carry refuses zone travel | **SHAPE** |
| Distance unload | 768 px ring | `sim/ring.ts`; sleepers' clocks tick | **IN** |
| Fog / reveal | reveal rects fill the minimap buffer | one bit per 16 px block in zone state | **SHAPE** |
| Tile edits that save | — | `tileDeltas` + `fill` action | **SHAPE** — unused until Grow / drain |
| Spell kinds | one object per spell | `melee bolt self world ground`; bolt `count` / `fan` / `splash` | **IN** (bolt, melee, world) / **SHAPE** (ground, self) |
| Statuses | *(Corrected)* none: `speed_multiplier`, player only. `damage[]` and `resistances[]` never read | effect rows read by every unit | **IN** |
| Camera | *(Corrected)* follow tween only. `CAM_STATE.LOCK` is an empty case; `cam_grab` is never set; zoom unclamped | eased follow, lock to named rect, shake | **SHAPE** |
| Input action layer | `interaction.*`, pad partly wired, touch `TODO` | KBM + the 2020 pad layout, one path | **SHAPE** |
| Multi-slot save | `slotN.jsav`; title Load was `TODO` | 3 slots, versioned, title + pause menus | **SHAPE** |
| Lighting | two `bm_subtract` passes on a surface allocated every frame; fixed colour | cached lightmap, add, multiply; clock-driven ambient | **SHAPE** |
| Particles | one `part_system`, 7 types | pooled squares by school | **SHAPE** |
| Animation table | `SpriteSet` switch (with the cactus fall-through) | frame names per sprite: `down up side` + `2` + `dead` | **SHAPE** |
| Non-unit mover (cart) | `obj_minecart` + tracks + levers | — | **LATER** |
| Zone floors | *(Corrected)* **did not exist.** `current_floor` is written in five places and read nowhere | — | **NEVER**, until a zone truly overlaps itself |
| Lily-pad raft | bolt damage slides a pad over water; it becomes walkable (never west: a bug) | — | **LATER** |
| Zone travel fade | 30 frames out, 30 in | instant | **LATER** |
| Audio bus | *(Corrected)* **2020 is silent**: two `.ogg` files, the only play call commented out | silent | **Built in the Rust build** (2026-09-27, `PRESENTATION.md` §5) |
| Touch layout | `TODO` | Pointer Events reach the UI; no on-screen sticks | **LATER** |
| Hunger / warmth | never in jun7 (it is JaneCraft, the March 2020 pitch) | — | **NEVER** |

---

## 1. Still missing: world as data

### 1.1 Machines (mover)

```
id, kind: cart, cells[] (tiles flagged Track), speed, onArrive[]
```

**2020:** `obj_minecart` follows a GameMaker path; the rider is teleported with the cart each step, so it is the *player* who touches triggers on the way. *(Corrected)* the cart itself only has one registered collision (`obj_minecart_stop_trigger`, which crashes every cart); the other `Collision_obj_minecart.gml` files are orphans missing from the event list. The cart is not in the path grid. A crash respawns it on a fixed path.

**Engine:** a mover that is not a `Unit`, on cells flagged `Track`, carrying the player, stopping at a marked cell and running an action list. Levers switch a junction. The 2020 design map adds a "Minecart Ramp Jump" and track sections that want Repair (4 iron). **Do not:** `class Minecart`.

The live mine paints `Track` tiles and parks a dead cart on them. That is set-dressing. **Bar: LATER.**

### 1.2 Toggle

The row exists (any prop with a `use` list flips `on` and runs it). What is missing is a placed lever and a test. 2020's levers had no target id and cannot be copied; the live contract is "`use` names the prop keys it acts on", which the solver already understands. **Bar: SHAPE** until the first lever earns a test.

### 1.3 Tile edits: Grow, drain, Explosion

`fill` writes a named rect to a tile and records it in `tileDeltas`, so it saves and replays, and the renderer rebuilds only the chunks the rect touches. Grow (2020 map: "Walk up to another level by making a vine grow", "Make purple flowers grow") is `answers: "grow"` + `fill` with `GrownPath`. Explosion ("destroy some rock") is the same with `Rubble → floor`. The 2026 build wrote the grid directly and lost the edit on reload. **Bar: SHAPE** until a zone uses it.

### 1.4 Raft

2020's lily pad: any bolt damage within 25 px slides it 8 px away from the player, only onto water and not onto another pad; it then clears its path rect, so you can stand on it. In the live engine that is a `push`-like prop that is non-solid over `WATER` cells and answers to any school. **Bar: LATER.**

---

## 2. Still missing: session, camera, input

- **Browser smoke test.** Presentation cannot earn IN without one. First on `SYSTEMS.md` §12.
- **Travel fade.** Polish on a verb that is IN.
- **Touch.** The UI is pointer-driven already. What is missing is a virtual stick and buttons that feed the same `InputFrame`. No second HUD.
- **Options** (rebinding, cursor, volume). 2020's was `TODO`.
- **Death.** 2020's production board wanted "Die → graveyard respawn + res sickness". Live: you stand back up at the mark you came in by, lock-ins reset. Res sickness would be one `effects.json` row applied in `revivePlayer`.
- **Zoom.** 2020's wheel zoom had its clamp commented out. The live view is a fixed ~216 px tall at an integer scale. A zoom setting would be a second integer.

## 3. Still missing: presentation

- **Audio bus:** `zone | combat | title → track`, `event → sfx`. *(Built in the Rust build, 2026-09-27: `jane-present::audio` listens to every one of them and `jane-audio` makes the sound, `PRESENTATION.md` §5. The TypeScript build stays silent.)*
- **Animation:** attack, cast and hurt are offsets and flashes on the walk frames. A real table would add frame names, not code.
- **Particles:** squares. An emitter table keyed by school is the 2020 shape (`ParticleList`, 7 types; every poison bolt reused the frost particles).
- **Square room lights** (production board): a light row with a rect instead of a radius.

## 4. Still missing: combat

- **`ground` and `self` kinds under test.** Webshot (`ground`) and a self-buff spell exist as rows; no test casts them.
- **AoE placed by aim** ("AOE from where?", production board): a `ground` row cast at the aim point within range rather than at a target. One field.
- **Boss-width health bar, first-time loot description, stats in the inventory window** (production board): UI rows.

---

## 5. Already IN — do not rebuild

Fixed-step deterministic sim, replay, one-tree saves with versions and migrations. One unit, one cast pipeline with 2020's validation order, incoming queue, effect rows, aim-based casting, fan / ring / splash bolts. 24 / 8 / 3+1, recipes by id, drops in zone state, rewards that never vanish. Kill / acquire / location with live `acquire` and persistent `location`. Dialogue trees. AI idle / combat / leash on a budgeted typed-array A\*. Keys as tags, gates, chests, plates, Repair with materials, frost torches, bait, triggers with reset, the segmented snake. Load ring. Five validated zones. Art as source.

Bugs that stay dead: `SpriteSet` cactus fall-through; `CastSpell` reading a missing target; `PathTo` `div 32`; save writing `maxhp` and loading at `xstart`; `moveToX/Y` swap; recipes by display name; player melee that hits nothing; the aggro counter that skips zero; paths that cannot reach an occupied goal; cooldown checked before range; rewards lost to a full bag; once-triggers that lose progress; saves that forget cooldowns and tile edits.

## 6. Do not port

| Thing | Why |
| --- | --- |
| TweenGMS | Third party |
| `Resolution_List` | Dead |
| `obj_onUseKey_*`, `obj_bench1`–`10` | One tag / one sprite. The anti-pattern this file exists to avoid |
| `current_floor` | It never did anything |
| `obj_trigger_room_lock_player_in` | A timed lock nobody can release. Use `enter` + `while` + `reset` |
| `obj_rose_growing_zone`, `obj_boss_room_trigger`, `plant_trigger` | Inert in 2020 |
| Named keys that open the nearest door at any range | You should be standing at the door |
| A class per 2020 trigger object | That is what `triggers.json` replaces |
| Anything in `Junqi-Legacy-GM/Assets/` | Purchased packs and ripped reference art (`DESIGN-2020.md` §6) |
| JaneCraft survival | A different game |

---

## Appendix — 2020 one-offs (uses of the engine, not the engine)

| 2020 thing | Uses | Notes |
| --- | --- | --- |
| Two mine carts, three levers | Machines | Content density. `WORLDGEN.md` §1.4 has the route |
| Lily pads round the key-chest island | Raft | One burial puzzle. Solution unverified even in 2020 |
| 12 `obj_trigger_snake_light_on` | Trigger + `switch` | Torches that light as you walk the west corridor |
| `obj_snake_boss` / `move_snake()` | The one custom mover | **IN** (`sim/snake.ts`) |
| Spider boss (eggs hatch every 20 s, web wrap at 6 s), undead boss, boss flower (seeds that crawl and bloom) | Unit rows + `ground` / `spawn` | The other three burial corners |
| Bat / pumpkin / cactus / soldiers / flowers / statues | Enemy rows | **IN** as rows; placed where the zones have holes |
| Fireball at (560, 1352) | A dialogue tree with `learn` | **IN**, in the garden |
| Auntie as a speaker | Dialogue | She is absent. That is the story |
| `music_dungeon1` / `bgm_butterfly1` | Audio bus | Two unlicensed files, never played |

---

## Source

- `LEARNING.md`, `LEARNING-SYSTEMS.md`, `WORLDGEN.md`, `DESIGN-2020.md`, `ENGINE.md`
- `Junqi-Legacy-GM/jun7/objects/`: `distance_unload`, `obj_minecart`, `obj_player/Other_16.gml`, `obj_water`, `obj_reveal_regeon`, `obj_button_1`, `obj_leaver1`, `obj_camera`, `obj_light_wall_torch_parent`, `obj_snake_1`, `obj_undead_lily_pad`
- `Junqi-Legacy-GM/jun7/scripts/`: `save_room`, `SpriteSet`, `ParticleList`, `Camera_functions`, `Unit_Functions`
- Live: `jane/src/sim/*`, `jane/src/world/*`, `jane/test/*`
