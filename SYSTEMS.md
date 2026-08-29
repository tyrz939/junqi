# Junqi — MMO-lite Systems Alignment

Production bar for the TypeScript remake. Pair with `LEARNING.md` (why), `LEARNING-SYSTEMS.md` (2020 paths), `WORLDGEN.md` (hybrid county), and `MISSING-SYSTEMS.md` (2020 contracts not in the remake). This file is what we hold the build against: **depth, scope, complexity, functionality**.

Junqi is not a theme-park MMO and not a combat sandbox. It is a **single-player MMO-lite**: WoW-shaped verbs in a Zelda-sized county. The player is **Jane**. The product is Julie's county: dog / house / cellar / mine / burial / snake. After the snake the county still turns. Factory, school, butterfly forest, and pipes are **Zelda dungeons**: each shows a lock first, teaches one verb, and tells Julie's story in the walls. Systems exist so those dungeons are rows (toggle, clear, grow, drain, repair), not classes.

---

## The bar

| Axis | In | Out |
| --- | --- | --- |
| **Depth** | A verb has validation, cost, cooldown, a failure string, and a data row | A unique class per potion, key, or enemy |
| **Scope** | Every 2020 *contract* that the campaign can touch | TweenGMS, unused resolution list, pasted WoW quest copy |
| **Complexity** | Shared unit, shared spell pipeline, shared AI loop, shared save shape | Parallel player-only combat or a second inventory |
| **Functionality** | The player can *do* the 2020 verbs in this county | Feature-complete gamepad/touch, imported GM art, full snake sine-body |

**Rule:** add a JSON row. Do not add a TypeScript class for a new item, spell, recipe, quest, enemy, status, or dialogue node.

**Rule:** content stays thin. If a system cannot be proven on Jane's loop, it is not done — it is a stub. Factory / school / butterfly / pipes already exist. Do not invent a seventh dungeon to "use" a system.

**Rule:** 2020 bugs stay dead. Recipes key by id. Day clock drives lighting. Path cell is 8 everywhere. Save stores live HP. Cast never reads a missing target.

---

## Status key

| Mark | Meaning |
| --- | --- |
| **IN** | Contract lives in the remake and is wired to play |
| **SHAPE** | Types/data exist; a later pass fills art or a boss special-case |
| **LATER** | Real 2020 idea, not needed to ship the county |
| **NEVER** | Do not port |

---

## 1. Session / boot / time

| Contract | 2020 | Remake | Bar |
| --- | --- | --- | --- |
| Persistent session | `game` object | `session.ts` | **IN** — seed, zone, player, beats, quests, bar, flags, dead, zone snaps, visited, carrying, craft |
| Slot save | compressed `slotN.jsav` | `localStorage` `junqi.slot0` | **IN** — live HP + hunger/warmth on the player snap; per-zone actor/prop xy + optional interior `fog` |
| Day clock | `time += 1/7200`, unused | `HOUR_SECONDS`, drives night wash + warmth | **IN** |
| Pause broadcast | user events | `session.worldFrozen()` = paused / GUI / terminal / dialogue | **IN** |
| Terminal | backtick | `` ` `` + `termCommands.ts` / `termFs.ts` / `TermOverlay.ts` | **IN** — `give` `god` `tp` `time` `quest` `beat` `save` `mp` `hp` `dump` `effect` `set` `status` plus a fake shell. Add a row. Do not grow a second language |
| Title load | TODO | New / Load slot 0 | **IN** |
| Debug overlay | F2/F3/F5 | F2 overlay, F3 grid, F5 save | **IN** |
| Distance unload | far-instance sleep | live ring + chunked tiles | **IN** — sleep idle/far on block change; wake-all before F5 / zone leave |
| Resolutions list | dead | FIT 1280×720 | **NEVER** |

---

## 2. Unit (the MMO actor)

One `Unit`. Player and AI share it. Controller is a field.

| Field | Bar |
| --- | --- |
| HP / MP = str×5 / spi×5 | **IN** |
| Energy + empty lockout | **IN** — sprint, idle regen, melee restore, carry drain |
| GCD 1.5s, per-spell CD, `stopTimer` | **IN** |
| 24-slot bag, spellbook, facing, target id | **IN** |
| Faction / class / combatState / creatureState | **IN** |
| Incoming damage queue by school | **IN** |
| Resist table per school | **IN** |
| Status list (duration, kind, tick) | **IN** — data in `effects.json` |
| Auto-attack = first affordable book/bar spell | **IN** — player melee while targeted; AI first book spell |
| Respawn timer | **IN** — `session.dead` keeps quest units out; field trash returns. `dropOnce` was removed |
| Dialogue hook | **IN** — unit/prop id → `dialogue.json` |
| Patrol waypoints | **IN** — optional on enemy spec |
| Occupy path cell | **IN** — others path around you |

Do not split `Player` / `Enemy` classes. Add fields on `Unit` or rows on the enemy def.

---

## 3. Combat / spells

Pipeline is the product:

1. Validate (dead, target, enemy, LOS, range, CD, GCD, MP, energy)
2. Pay costs only on success
3. Spawn a *kind* (melee / bolt / self / world), not a class
4. On hit: school damage + optional `onHit` status
5. Drain incoming later this tick

| Contract | Bar |
| --- | --- |
| Spell row in `spells.json` | **IN** |
| Error enum + toast map | **IN** |
| Schools: heal, physical, frost, fire, nature | **IN** |
| Melee cone + locked target | **IN** |
| Stepped bolts, die on wall/unit | **IN** |
| VFX from school / override table | **IN** — new spell does not need a new renderer |
| `onHit` status id | **IN** |
| World verb (`repair`) | **IN** — one special id, not a family of objects |
| AI uses the same `tryCast` | **IN** — AI may bolt, not only physical melee |
| Crit 1-in-20; melee `str/8 + rng(str/32)` | **IN** |
| Manashield / stoneskin / lifesteal / firelash / winterbite / sparktongue / stranglethorn | **IN** as statuses |
| Slow / root / poison DoT / wet | **IN** |
| Resist on flush | **IN** |

**Scalability:** a new spell is a JSON row + optional `vfx.ts` override. A new status is an `effects.json` row.

---

## 4. Items / bags / craft / bar

| Contract | Bar |
| --- | --- |
| Item row (`items.json`) | **IN** |
| Stack, leftover, "Inventory full" | **IN** |
| Use → consume, item CD, GCD | **IN** |
| Keys are a tag (`opens`), one use path | **IN** |
| Action bar: 8 slots, `{ source, id }` | **IN** |
| Click / RMB use, 1–8, bind from bag/book | **IN** |
| Craft: 3 in + 1 out, sorted **ids** | **IN** |
| 2020 recipe list | **IN** |
| Ground drops | **IN** — death rolls `loot[]`, E to pick up |
| Drag ghost (`obj_itemMove`) | **IN** — bag / bar / book drag in `drag.ts`. Keys and Julie's letter refuse destroy |
| Hunger / warmth | **IN** — `survive.ts`. Food and `warm` on item rows. Night and mountain take heat; campfire / stone give it back |

---

## 5. Quests / dialogue / beats

Three quest types only: `kill`, `acquire`, `location`.

| Contract | Bar |
| --- | --- |
| Quest row + progress array | **IN** |
| Slay / inventory sync / enter location | **IN** |
| Hand-in + **rewards[]** | **IN** — no more hardcoded key grant |
| Story spine (`beats.json`) | **IN** — seed invalid if a required beat object is missing. `other_doors` fires on first factory / school / forest enter. `still_turning` is the dog hand-in after all three |
| Dialogue list + 2-option nodes | **IN** — data, not `talkDog()` branches |
| Speakers | **IN** — dog plus town boards / doorstep NPCs from `npcs.json` + `towns.json`. Same runner |
| Quest log window | **IN** |
| WoW paste / "Good Job JoJo" | **NEVER** |

---

## 6. AI / path

MMO loop, not a chase script.

```
idle (regen, patrol, aggro+LOS every ~0.16s)
  → combat (pick first affordable spell, walk if tooFar / notInLOS)
  → leash (home, then idle)
```

| Contract | Bar |
| --- | --- |
| Idle / combat / leash | **IN** |
| Aggro metres, leash metres | **IN** |
| A* cell 8, diagonals, max path metres | **IN** |
| Occupy cell | **IN** |
| Patrol path | **IN** |
| First-in-book priority | **IN** |

Snake **boss** is the one allowed custom mover. Until the sine-body lands it still uses this loop plus room-lock + phase spell list. That is **SHAPE**, not a second AI.

---

## 7. World verbs

A place, not an arena.

| Verb | Bar |
| --- | --- |
| Zone travel + marks | **IN** — 12 `ZoneId`s. Hatch = cellar. Yard doors = mine / burial. Later mouths = factory / school / butterfly / pipes |
| Locked door + key tag | **IN** — E unlocks a keyed door in place when it has no `toZone` |
| Chest loot once (flag) | **IN** |
| Push / pull on 8px grid | **IN** — same `push` tag; face 180° + spacer pulls |
| Carry / put down | **IN** — E pick up, E put down, occupy while held; drain energy. Dropped on zone travel |
| Repair world object | **IN** |
| Learn-spell prop | **IN** — `learn:` action, not `if (orb)` |
| Toggle / lever | **IN** — `kind: toggle` runs an action list. Factory, school, trials |
| Drain flood | **IN** — `drain:` action on a named rect. Pipes + county trials |
| Grow | **IN** — spell row. Butterfly forest + grow trials |
| Room lock / boss lock | **IN** — `triggers.json` row (`lock` / `unlock`), not a PlayScene branch |
| Location trigger | **IN** — enter-rect + `location:` action |
| Reveal / fog | **IN** — interiors stamp a saved bitmap (`fog.ts`). County stays live radar |
| Hunger / warmth | **IN** — thin coat on food and fire stones, not JaneCraft |
| Towns / NPCs | **IN** — six towns, boards, doorstep speakers. `towns.json` + `npcs.json` |
| Gold mine as its own zone | **IN** — same generator family as the basement, different loot |
| Lily pad / torch puzzle | **LATER** — burial set-dressing, not a new system |
| Cart on a track | **LATER** — machine row, not a class |
| Distance-based instance sleep | **IN** — 768 px ring; far idle AI / props skip tick; tile chunks |

---

## 8. GUI / camera / input

| Contract | Bar |
| --- | --- |
| Dual camera: zoomed world, crisp UI | **IN** |
| HUD vitals, target, stacked toasts (max 3) | **IN** |
| Windows: inventory, spellbook, quest, map | **IN** — `BagPanel` `BookPanel` `QuestLog` `MiniMap` |
| `mouseOnGui` blocks world walk / target | **IN** |
| Tooltips on bar + bag hover | **IN** |
| RMB virtual stick | **IN** |
| KBM | **IN** |
| Gamepad / touch | **LATER** — keep `input` as one action layer when it lands |
| Camera lock states | **LATER** |
| Subtract lighting surface | **SHAPE** — night wash + glows; true subtract is art pass |
| Particle system | **LATER** |

---

## 9. Content vs system

The campaign that *proves* the systems. Do not grow this list until the verbs above stay green. Do not add a `ZoneId` so a speaker has somewhere to stand.

```
Jane + letter (Sunday train)
  → yard + dog (dialogue, quest give/hand-in)
  → skeleton (kill, loot, respawn-exempt)
  → house key (quest reward)
  → kitchen (location, chest, bench, learn Icebolt — she does not start a witch)
  → cellar (domestic, basement key)
  → gold mine (HM / vault / boss locks)
  → burial (garden Fireball, wings, snake)
  → snake down
  → factory (see the gate, lever in crates, clear the line, Repair the grate)
  → school (hall twice, desks, detention, office key, bell opens the case you passed)
  → butterfly forest (see the island, learn Grow, north fight, amulet)
  → pipes (see the wet chest, drain lever, walk)
```

County dressing (shopkeep, scarecrow, car, picnic, farm acre, graveyard, abandoned house, museum keyed wing) stays on the overworld or as neighbour interiors. Do not add a seventh dungeon to exercise a verb. The four post-snake interiors *are* the verb. County **trials** (stone rings: push, lever, clear, drain, Grow) and **camps** (restock + campfire) are how the overworld gets hours. Hunger / warmth is a thin coat on food and fire stones, not JaneCraft.

Enemy *rows* may include bandit / plant / spider so the table is as wide as 2020. Spawn them where the county already has a hole. Do not author a spider wing.

---

## 10. Scalability checklist (use in review)

A change is on-bar if:

1. A designer can add the thing in JSON without opening `PlayScene.ts`
2. Player and AI share the validation / cost / incoming path
3. Failure is an enum string, not a silent no-op (except GCD/CD, which stay quiet like 2020)
4. Save/load does not invent a second schema
5. Metres for combat, pixels for path, cell = 8
6. Story spine objects are still present after a seed roll

A change is off-bar if:

- New `class Fireball extends Spell`
- New `obj_onUseKey_*` equivalent
- Recipe keyed by display name
- Quest progress that is not kill / acquire / location
- A second HUD that does not use the UI camera
- Combat that writes `hp` in the same call that queues damage

---

## 11. File map (remake)

| Concern | Live here |
| --- | --- |
| Rows | `junqi/src/data/*.json` — catalog also merges `spells-more` `enemies-more` `quests-more` |
| Towns / NPCs | `towns.json` `npcs.json` |
| Catalog | `junqi/src/game/systems/catalog.ts` |
| Unit | `junqi/src/game/entities/Unit.ts` |
| Cast / numbers | `junqi/src/game/systems/combat.ts` |
| Status | `junqi/src/game/systems/status.ts` + `effects.json` |
| Survive | `survive.ts` |
| Dialogue | `junqi/src/game/systems/dialogue.ts` + `dialogue.json` |
| Actions / triggers | `actions.ts` `triggers.ts` + `triggers.json` (`drain:` included) |
| Load ring | `loadRing.ts` |
| Loot | `junqi/src/game/systems/loot.ts` |
| AI / path / LOS | `ai.ts` `path.ts` `los.ts` |
| Quests / beats | `quests.ts` `beats.ts` — quest window is `ui/QuestLog.ts` |
| Session | `session.ts` |
| Terminal | `termCommands.ts` `termLog.ts` `termFs.ts` `ui/TermOverlay.ts` |
| Zones | `world/generate*.ts` `buildZone.ts` `motifs.ts` `stamp.ts` `biomes.ts` + `pockets.json` |
| County fill | `generateTownYard.ts` `generateTowns.ts` `generateTrials.ts` `generateWaysides.ts` |
| Interiors | `generateHouse.ts` `generateCellar.ts` `generateMine.ts` `generateBurial.ts` `generatePlaces.ts` `generateDungeons.ts` |
| Play compose | `scenes/PlayScene.ts` |
| UI | `ui/Hud.ts` `Gui.ts` `ActionBar.ts` `DialogueBox.ts` `BagPanel.ts` `BookPanel.ts` `MiniMap.ts` `drag.ts` `fog.ts` |

2020 GML is **read-only**. Steal contracts. Do not port line-for-line.

---

## 12. Intentionally later

These are real, and we will lie if we mark them IN:

- Segmented snake boss mover
- Gamepad + touch GUI paths
- Imported GameMaker sprites / SpriteSet table
- True subtract lighting surface
- Cart on a track (`machine.kind = cart`)
- Zone floors (`current_floor` 0/1)
- Full burial instance-script set-dressing
- Particle library
- Audio bus
- Multi-slot save
- Ground loot in the zone snap (drops still die on zone leave)

Ship the county with the **IN** column. Pull from this later list only when Jane's loop is playable and crisp. Toggle, drain, drag, interior fog, hunger/warmth, and the four post-snake rooms are IN.
