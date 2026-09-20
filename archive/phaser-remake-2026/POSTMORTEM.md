# Phaser remake (2026) — post-mortem

This folder is the Vite + TypeScript + Phaser 3.88 remake, archived September 2026 and replaced by the ground-up build in `junqi/` (`ENGINE.md`). It still builds (`npm install && npm run dev`). It is kept for its content rows and its generators, not as a base.

Its own README opened with "This is a buggy mess. The verbs work. The first five minutes do not." This file says why, with evidence, so the next build does not repeat it. Paths are relative to `src/game/` unless they start with `data/`.

The audit was read-only. It also bundled the Phaser-free modules and ran them in Node on seed 12345; that 150-line harness found the two worst bugs in seconds. The project never had one. **That is the first lesson.**

Build: `tsc --noEmit` clean under strict, 84 modules, one 1,753 kB chunk (424 kB gzip) because all of Phaser ships. No tests.

---

## P0 — why the first five minutes did not work

1. **Enemies could not path to a standing player.** `path.ts:17` returned `null` when the goal cell was solid; `Grid.solid` counted occupants (`world/Grid.ts:64-68`); the goal was the player's own occupied cell. Chase only worked on the frames the player crossed a cell border. Paths were only recomputed when exhausted (`ai.ts:108`), and a `null` re-ran A\* every frame.
2. **Cast validation made eight enemy kinds inert.** `combat.ts:24-28` forced the target block for AI and returned `notValidTarget` whenever `reqEnemyAsTarget` was false, which every player-style bolt row had. `pickSpell` kept choosing that first spell forever. Slime, pumpkin, vine, briar, ash_pumpkin, frost_cactus, poacher, marsh_slime: 33 of 271 county spawns never cast and never moved. `onCooldown` was checked before `tooFar`, so melee mobs froze for their 2 s cooldown after every swing.
3. **Survival was cosmetic and shook the screen forever.** Starve and cold damage were `5*dt` and `3*dt`; `mitigate` rounded them to 0 above 10 fps; the zero hit still counted as a hit, so the camera shook and the sprite flashed every frame. Warmth could not fall below 32. Spawn read as "mountain" because `cx > 1680` (`biomes.ts:4`) was tested before the yard rule, and the stoop was at cx 1780.
4. **Dog dialogue order made two hand-ins unreachable.** `data/dialogue.json` is first-match and `flag:knows_ice` sat above two `questMet:` rows. Touching the ice orb (README step 5) locked both out. `julies_marks` was only offered inside that node and could never complete.
5. **Once-triggers lost progress permanently.** `location:` only credited quests active at that instant; `beats.complete` silently returned false out of order; the trigger then marked itself fired anyway. Entering the burial before the kitchen dropped two beats and every later beat failed for good.
6. **Quest rewards were lost on a full bag** (`quests.ts:77` ignored the leftover). The house key could vanish: a real softlock. ("Not now" on the dog's reward was *not* a softlock; the node re-offered.)
7. **Grow edits were not saved.** Save on a grown tile, load inside solid water.
8. **A dropped crate could land on the player's own cell** (`forceDrop`), including on zone travel.
9. **A\* had no bound.** `maxMetres` was checked after reaching the goal. Open set re-sorted on every pop, string keys in Maps, no closed set, `dirs` reallocated per node. Unreachable goal: 32 ms at 2.5k cells, 1,156 ms at 40k. Quadratic. The county was 2.4M cells.
10. **The clock started at dusk** (19:00, from 2020's code): the night wash arrived two minutes in and lasted eighteen. `the_letter` completed on frame 1 because the spawn sat inside its own trigger.

## Engine fundamentals

- **Time:** variable `dt`, no clamp, no sub-step. Movement and bolts tested only endpoints; a hitch could tunnel a 260 px/s bolt through an 8 px wall. `SYSTEMS.md` called them "stepped bolts".
- **Determinism:** none. Combat and loot called `Math.random`. Worldgen threaded one RNG stream through every generator, so any added call shifted everything after it.
- **Headless:** about 4.3k lines were Phaser-free, but `Unit` had no x/y; position lived in `sprite.x`. Movement, bolts, triggers, carry and the load ring existed only inside `PlayScene.ts` (1,562 lines).
- **State was smeared.** Lock state in three places (`prop.locked`, `grid.extraSolid`, an `open:` flag). "Used" in three. `session` mixed save data with UI latches, each field hand-listed five times. No save version; a migration hack lived in gameplay code.
- **Not saved:** cooldowns, bolts, enemy statuses, aggro, paths, `light:` action lights, ground drops. Herb drops came back on every zone change: an infinite farm. `load()` had no try/catch; a failed seed threw instead of re-rolling.
- **Load ring:** sleepers did not tick timers, so mobs only respawned while you stood within 384 px of the corpse.
- **Spawns inside solids:** `fits()` checked bounds, not overlap. Seven on seed 12345; two were in unseeded interiors and broken on every run.
- **No damage field on spells.** All 50 rows differed only in cost, cooldown, range, school, `onHit`.

## Docs that said IN and were not

Hunger/warmth (inert). "AI uses the same tryCast" (broken for 12 kinds). "A new status is an `effects.json` row" (false: manashield, stoneskin, wet, haste, lifesteal, firelash, winterbite, stranglethorn, sparktongue, well_fed were all keyed by id in TypeScript; the `absorb` and `flag` kinds were never read). "Repair is one special id" (`repair`, `grow`, `abandoned_car`, `boss_gate` were hardcoded in `PlayScene`). "Max path metres" (not a bound). "Dialogue is data" (town boards were a TS table; NPC trees were synthesised in code). Of **102 quests, 61 had no hand-in anywhere and 33 were never offered.** Unknown action strings were silently ignored.

**Lesson:** a status column nobody can run is a wish list. The live build's `SYSTEMS.md` marks a row IN only when a test names it.

## Scope, against "content stays thin"

| Area | Lines |
| --- | ---: |
| Terminal and fake shell (`ls cd cat tree alias lua uname whoami`) | 1,503 (14% of the TypeScript) |
| Six towns, trials, waysides | 824 |
| Four post-snake dungeons + museum, graveyard, abandoned house | 641 |
| Hand-plotted pixels in `BootScene` | 581 |
| Minimap + fog | 261 |
| Survive + biomes (inert) | 94 |

96 items, 50 spells, 52 enemies on 11 sprite sheets, 102 quests, 271 county enemies, 537 props, 200 lights. The README's own five steps used about six rows. The 2020 lesson ("the systems outran the content") had inverted: **content outran the systems, and neither was tested.**

## Worth keeping (and kept)

- One `Unit` for player and AI; `tryCast` → `SpellError` → toast.
- The incoming-damage queue with a flush phase.
- The action-list idea and a single port between data and scene. It needed a parser and boot-time validation; the live build made it a typed union instead.
- `tickAi` returning events rather than touching sprites.
- Seeded pocket stamps with stable ids; per-zone snapshot keyed by id. `assertRequired` should have re-rolled, not thrown.
- The load-ring box tests and 16-cell tile chunks.
- Fog bit-packing.
- `items.json`, `recipes.json` (sorted-id match), `effects.json` names, the `QuestDef` and `EnemyDef` shapes, `{source, id}` bar slots.
- The writing. The dog's voice ("I am not a city dog.") carries over.
- Strict tsconfig, two-dependency footprint.

## Worth mining later

`world/generateDungeons.ts` (factory, school, butterfly, pipes) and `generatePlaces.ts` (museum, graveyard, abandoned house) are 640 lines of authored room graphs with one verb each (toggle, clear, Grow, drain, Repair). They are inventions rather than ports (2020's butterfly room is an empty shell; `DESIGN-2020.md` §4 has what 2020 actually drew), but the layouts are a head start. Port them as blueprints through the validator, in the 2020 order: **Museum first.**
