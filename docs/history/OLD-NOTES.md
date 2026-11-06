# Jane — Old notes (Trello)

Trello / Kanban screenshots recovered 2026. Broad overview, not a second spec.

Pair with `LEARNING.md` (what 2020 actually ran), `LEARNING-SYSTEMS.md` (paths), `DESIGN-2020.md` (the design *folder*: story doc, balance sheet, dungeon maps, pad layout), `SYSTEMS.md` (the bar), `WORLDGEN.md` (zones), `MISSING-SYSTEMS.md` (engine still out).

This file is the Trello boards only. It was written before anyone read the design folder or audited the code, and two of its conclusions were wrong; they are corrected below and marked.

**These cards are not a backlog.** Most of this never shipped. The boards are how 2020-you *thought* before and during jun7. Steal taste. Do not grow a sixth dungeon because a due date said March 2021.

---

## What you are looking at

Three boards, two minds.

| Board | Colour | When it feels like | What it is |
| --- | --- | --- | --- |
| **Jojo Witch** | green | earliest | Zelda-craft dream. Open world, elements, many dungeons, BotW / Wind Waker notes. Working title **BBB Jojo Witch**. Player **JoJo**. |
| **Old notes** | green, tiny | same era | Story spine in four cards. Music and two overworld quests. |
| **Production** | orange-brown | ~Jan–Mar 2021 | The jun7 build board. Technical todos, UI, controls, the two dungeons that actually got rooms. |

The dream board wanted Breath of the Wild. The orange board was already an MMO-lite in a county. The live build follows the orange board's *product* (Jane / Julie's house / dog / mine / burial / snake). The other places on the green board are not "dressing": they are the rest of the game, in an order 2020 wrote down (`DESIGN-2020.md` §4.1). They are simply not built yet.

The working title says **Jojo**, and 2020's test quests say "Good Job JoJo", but `Story.docx` (June 2020) says **Jane** throughout. Jane is the original name.

Dates on dungeon cards (production board): Gold Mine ~13–15 Jan 2021, Burial Chamber ~16–26 Jan, Museum 3 Feb, Factory 14 Feb, Underground Pipes ~20–23 Feb, Butterfly Valley 3 Mar, School ~12–17 Mar. Targeting-aim card due **4 Mar 2021**. Treat dates as "we hoped," not "we shipped."

---

## Story that was always the game

The small green board, almost the whole plot:

1. Background story (brief)
2. At the beginning of the game
3. Set Auntie's house
4. Set underground dungeon of Auntie's house

That is still the remake spine. House first. Cellar under the house. Mine and burial as yard mouths, not a theme-park list.

Jojo Witch also had: intro cutscene, "at the start of game," first level, dungeon. Same shape, more cards.

**Auntie drift.** Dream board: "Auntie is a doggy." 2020 split them — Auntie is the house and the absent person; the dog is the only authored speaker. Keep the split. The dog *is* the test that proved dialogue + quest + hand-in (production board, 4/4, done).

Other dream characters: evil spirit, rats, a red-hair four-dir sheet, a green-hat four-dir sheet, generic NPC. Rats and undead made it. The witch-look sheets are art intent, not a second player class.

---

## Two dungeon lists

Dream board and production board name the same places. Only two got a 2020 room that mattered.

| Place | Dream name | 2020 room | Remake |
| --- | --- | --- | --- |
| Gold mine | Gold Mining Cave | `room_dungeon_goldmine` | **IN** — yard door |
| Burial | Burial Chamber at grave yard | `room_dungeon_burial_chamber` | **IN** — yard door |
| Auntie basement | underground basement / basement in aunties house | `room_basement_auntie` | **IN** — kitchen hatch (cellar, not the underworld) |
| Butterfly | Butterfly Forest / Valley | `room_dungeon_butterfly_forest` exists but is an **empty shell** | Dungeon 3. 2020 drew it: no keys, no doors, eight butterflies, Grow, a net, an amulet |
| Museum | Museum | — | **Dungeon 2, and next.** 2020 drew it: a key shuffle across four wings; its key is in the mine, and the forest's key is in its Magic wing |
| Factory | Factory | — | Dungeon 4. 2020 drew it: break in by vent or sewer, an Electric spell that runs robots, a worker's diary |
| School | School | — | **The finale.** Never drawn |
| Pipes | Underground pipe system | — | The connector to the factory, entered with Repair |

*(Corrected.)* The previous version of this table described "verbs" for these places (drain the east, push desks, hall you walk twice). Those were inventions of the 2026 Phaser build, not 2020 ideas. What 2020 drew is in `DESIGN-2020.md` §4.
| Theme-park record | Record in music player's house | — | not the county |

Production **Overworld** column was empty. Zone1 still got built. The empty list means they did not plan the town on Trello; they placed it in the room.

### Map language (dream attachments)

Gold mine legend (as readable): key / door / gate, start, finish / level exit, mine cart track, optional boss, boss, item box. Cover boxes. Pivot.

Burial legend (as readable): START, final boss, mini boss, items, chest, locked gate, boss room. Also: check-and-clear, furnishings, stairs, area-of-effect / interactive / loot, path of pivot / iron cart / moving object.

That is `WORLDGEN.md` placement language: beats and tags, not stamped x/y. Cart + track is the machine row in `MISSING-SYSTEMS.md` (LATER). Do not rebuild those maps as rooms.

Production **Doing** already knew the mine spell had to matter on the final boss. Remake: garden Fireball is that spell. Keep it.

Production also: "fall from 2nd floor to first floor to access minecart." That is zone floors (`MISSING-SYSTEMS.md` SHAPE), not a new dungeon.

---

## Elements (dream crafting)

The green **elements** column is why `items.json` has wood / iron / gold / water / stones / herbs. The *recipes* in 2020 are a slice. The *fantasy* was BotW cooking with a county accent.

| Card | Notes | Remake |
| --- | --- | --- |
| Wood | Build / fix boat, house, box. Burn to coal | Wood + Repair exist. Boat does not |
| Fire stone | Light wood. 10 + coal + magic → fireball | Item row exists. Fireball is a learn-prop, not a 10-stone craft |
| Coal | Heat, special weapons, puzzle tools | Item row |
| Gold | Magic for almost every spell; keys locked by evils | Gold bar / dust; keys are tags, not gold |
| Water | Only in rain, with a gold pot. Medicine and poison | `small_water` is a vial. No rain gatherer |
| Energy bar/drink | 1.5× spell power | Energy is a unit field (sprint / carry / melee). Not a drink |
| Wind | Wind bell; move things except in water | Push/pull/carry. No wind spell |
| Night view glass | See in the dark | Item row. Lighting is wash + ADD |
| Light stone | Circle of light like a torch | Item row. Lights are prop data |
| Iron | Physical damage (knife / sword) | Item row. Player is still a mage |
| Poisoned plants | "chemical weapon" magic | Herbs + poison status |
| Healing plants | Combine with gold + water → potion | Craft-by-id potions. Gold is not in every recipe |

**Do not** add rain-pot, wind bell, shrink spell, or "gold in every recipe" to ship the county. If an element already has an item id, a recipe row is enough.

---

## Quest ideas that stayed cards

Dream **Quests**: butterflies, forest, talk with evil spirits, find the way to talk to animals (the dog), ghost family puzzle (nature view), help animals, collect elements for spells, freeze-enemy puzzle, boat fix, spell to move a big item, tools for gold mining, shrink spell.

Tiny board: fish in the river for a reward; walk up to a tree, get a quest to heal trees.

Production's only *systems test* that was marked done: **friendly NPC, dialogue, give quest, hand it in.** That is the dog. Three quest types in the remake (kill / acquire / location) are the engine. These titles are content rows you may never write.

---

## Production wish list (orange board)

Useful because it is closer to the engine you actually wrote.

### Technical

- Potion on-hit toys (cast heals you, taking hits restores mana) — this became `onHit` statuses (`firelash`, `lifesteal`, manashield…). Fun potions were the point.
- First-time loot description (what it is / how to use)
- Controller: spell *direction* and x,y targeting
- Large area spells (spikes from the ground) — `MISSING-SYSTEMS.md` spell kind `ground`
- Fog texture — reveal rects are SHAPE
- Square lights for dungeon / cave rooms
- Solid colour smoke puffs on damage — particles LATER
- Point-in-rectangle for mouse-on-GUI — remake has `mouseOnGui`
- Draw a shield around town
- Die → graveyard respawn + res sickness — live: you stand back up at the mark you came in by and lock-ins reset. Res sickness would be one effect row
- Inventory window shows stats
- Options: choose cursor type
- Boss-width health bar

### Controls / UI

Keyboard (tagged), controller, touch (empty), button list. Matches `LEARNING-SYSTEMS.md`: pad existed, touch was TODO. Remake: KBM now, pad/touch LATER, one action layer.

Quest system, dialogue, main / options / load / start menus. Title New / Load slot 0 is IN. Options menu was TODO in 2020 and stays LATER.

### Gameplay fork (flipped, and not silently)

**URGENT (4 Mar 2021): remove targeting, make direction-based spell aim.**

*(Corrected.)* The previous version of this section said "2020 and the remake kept locked target + melee cone + bolt at target… Do not swap the combat model to finish an old card." **That was wrong about 2020.** The card was done:

- In the final jun7 source every assignment to the player's `current_target` is commented out (`obj_player/Other_16.gml:219-273`).
- Player projectiles fly along `spell_direction`, the direction to the mouse. They never home.
- No player spell has `requires_target`.
- The pad layout dated **November 2020** already says "Right stick: Aim" (`xbox controller layout.jpg`).
- Only AI units use `current_target`.

So click-targeting was the 2026 Phaser build's departure, not 2020's model. The live build is **aim-based**: bolts fly at the cursor or along the right stick, melee takes the nearest enemy in the facing half-plane, AI casts at its target. It is still the MMO-lite (GCD, cooldowns, a spellbook, an action bar, leash AI); what changed is who does the aiming. The HUD still shows a target frame, for whoever you last hit.

**AOE from where?** — still the right question. Ground kind is the engine answer, not a new class.

**Doing:** hp/mana numbers from red/blue effects; mine spell on final boss; dungeon work; save/load + persistent zones; "improve game play feel." Persistent zones are `session.zones` (IN). Feel is not a system.

---

## Zelda notes (what they were stealing)

**Breath of the Wild:** goal at the start; more possibility; enemy setting; story and conversation; elements and their functions; how you get food / weapons / tools; skills (power, exploring); character; buildings; world (surroundings and places).

**Wind Waker (personal):** more characters → more talk; story and conversation; world setting; *simple* elements; goal is very clear; buildings not special but good enough.

The remake already took the useful half: a clear first goal (the dog), simple elements as item rows, buildings that are stamps, a small world that feels like a place. "Set much more possibility" is how BotW ate calendars. Jane stays a county.

---

## Also on the dream board (not engine)

- Music / marketing lists. 2020 shipped two `.ogg`. Audio bus is LATER. Marketing is not this repo.
- Jane to-do: Drawing 3/4.
- Handwritten notebook photo — unread here; the story-line cards cover the same beat.

---

## How to use this file

| Take | Leave |
| --- | --- |
| Auntie house → cellar → mine → burial, then factory / school / forest / pipes as Zelda graphs | A seventh dungeon, or `carve()` plus a sign |
| Dog as the systems test | Jojo Witch as a second game |
| Elements as item / recipe rows | Rain pot, wind bell, shrink, gold-in-every-spell |
| Ground AOE, fog, square room lights, first-loot text | Direction-aim rewrite of combat |
| Toggle as a row; cart / floor later | `class Lever` |
| "Buildings not special but good enough" | A spider wing bolted on to “use” spiders |

If a card is already an **IN** row in `SYSTEMS.md`, it is done and a test holds it. If it is in `MISSING-SYSTEMS.md`, it waits on the engine table. If it is a place, check `DESIGN-2020.md` §4 before calling it flavour: 2020 may have drawn it.

Source: Trello screenshots (Jojo Witch, old notes, 2021 production). Card bodies behind attachments were not transcribed.
