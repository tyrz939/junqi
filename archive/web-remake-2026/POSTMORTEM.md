# Web remake (2026) — post-mortem

This folder will hold the Vite + TypeScript remake (`jane/` at the repo root until `PORT.md` phase P10 moves it here). It is being replaced by the native Rust build (`PORT.md`, `ARCHITECTURE.md`, `ART.md`, `PRESENTATION.md`). Written 2026-09-26, while the build is still the reference and every number below can be checked by running it.

Unlike the Phaser post-mortem this is not a list of things that did not work. The build works. It is archived because the owner wants the game native, integer-exact on every machine from a Raspberry Pi to a Pentium 4, and drawn entirely by code, and none of those three is a browser build. What follows is what it proved, what it got wrong, what carries, and what is dropped, so the Rust build starts from its lessons and not from zero.

## Numbers

| | |
| --- | --- |
| `src/sim` | 31 files, 7.0k lines |
| `src/world` (incl. 109 `.room` templates) | 17.7k lines |
| `src/art` | 12k lines, ~410 sprites (109 units, 267 props, 91 icons) |
| `src/render` | 3.3k lines |
| `src/ui` | 3.6k lines TypeScript + 1.5k CSS |
| `src/app`, `src/input` | 1.3k lines |
| `src/data` | ~1.1 MB JSON: items 99, units 149, props 283, quests 95, dialogue 305, spells 23, effects 20, triggers 23, placements 224, stories 47, dungeons 8 |
| `test/` | 36 files, 12.7k lines, 623 tests, about two minutes |
| County | 2000 × 2000 cells, built and validated in "a second or two" |
| Runtime dependencies | none; bundle ~280 kB |

## What it proved

- **A headless, deterministic sim that the tests play.** Fixed 60 Hz ticks, a seeded RNG inside the state, no runtime trigonometry, input quantised to 1/127, one serialisable state tree, `hashState`, and a replay test that re-simulates a session to the same hash. `test/bot.ts` walks, talks, pushes and fights with nothing a person could not press.
- **A lockstep-ready state tree.** `players[]` up to four, join and leave as commands in the same stream as everything else, every zone with a player in it ticking, the party penalty by connected count, one heroine, one fire. Twenty-odd co-op tests with two to four seats in one sim. No networking was ever written; the sim was shaped for it and that shape carries.
- **Skeleton-first worldgen with rows as constraints.** Story places as `sites.json` rows with distance rules, solved site by site with its road; areas, POIs and anchors placed by rank; a threat map; checks on every seed (first walk safe and lit, a rest per region, no empty stretch over 900 m). A seed viewer that showed 24 counties at a time and let the owner say yes.
- **The lock-and-key solver and C1 to C12.** Every zone on every seed floods from the entrance and must reach every contract name; dungeons are authored as mission graphs, generated on a bay lattice from room templates, and rejected by name when a check fails. 1,000-seed soaks with zero fallbacks. Templates proven alone from every door in every transform; each check shown to reject a blueprint broken for it.
- **Content as rows, verbs as one enum.** 29 actions and 8 conditions drive dialogue, props, triggers, quests, items, deaths, phases and clock rows. A new verb is one case and the compiler lists the rest.
- **Streams independence.** Changing one generation step moves nothing outside the ground it touches. This is the test that made agent-parallel worldgen work.
- **Art as source.** No image, font or audio file ever entered the repo. About 160 of the 410 sprites were already generated (houses, trees, terrain, corpses, town props, icon templates), and those are the parts that held together best.

## What it got wrong, and what the port fixes

| Wrong | Fix |
| --- | --- |
| Floats in decisions: positions, hp fractions, `Math.sqrt` distances, party multipliers; and in worldgen `Math.hypot` (8 sites), `**` (4), `Math.round` .5 (48), `toFixed`, `Float32` stores mixed with `f64` maths. Determinism across JS engines was planned to be proven and never was | Integer-only sim and worldgen (`ARCHITECTURE.md` §2, `PORT.md` §6.b–c). Floats are a compile error in those crates |
| Six seed-derivation patterns (`stepDice`, the Kit main stream, `within` twice, `rankBase` strings, `paintLand` with no attempt, lamps with attempt 0) | One `dice(seed, zone, step, attempt, a, b)` (`PORT.md` §6.a) |
| `Sim.aims` outside `GameState`: not saved, not hashed; commands applied between ticks with the aim from a later frame, so a replay could diverge above 60 Hz | Aim in the input frame; commands applied inside the step (`ARCHITECTURE.md` §0, §3.4) |
| Clock rows ran once per connected player, docs said once | Once, actor none |
| One `state.rng` shared by combat, loot and `createUnit`, so the order zones were first visited moved combat rolls | Per-zone streams; `think_offset` from the id |
| `sim` imported `world` and `world` imported `sim`; UI and renderer read `sim.rt` and `sim.zone` and called sim functions | One-way crate graph; a `View` API (`ARCHITECTURE.md` §1, §11) |
| Per-tick timer, status and flush passes over every unit in a live zone, thousands of sleepers included | Absolute ticks and a catch-up on wake (`ARCHITECTURE.md` §4.3) |
| Runtime indexes aliasing state objects; `asPlayer` dynamic scoping; lists mutated while iterated; prop instances copying action lists | Arenas with typed ids, `Ctx` with an explicit actor, deferred `ZoneOps`, `ListRef` |
| God functions: `buildCounty` (22 stages), `validateBlueprint` (one 580-line closure), `buildDungeon` | Stages with one stream each; the solver as passes (`PORT.md` §6.k) |
| Hidden module state: `lastSkeleton`, three footprint sources, `country.ts`'s cached context, name pools, `INFO` and `onFoot` WeakMaps, `CONTRACTS` mutated at import | Values returned and passed; contracts in `data/zones.json` |
| Order dependence: `Object.entries` weighted picks, `Object.keys` for tick order and `cutThrough`, glob-and-sort path order, six sorts leaning on stability | Ordered arrays from data, a closed `ZoneId` enum, total comparators (`PORT.md` §6.d) |
| Authored county chunks as 932 lines of code; tuning tables as code; the camera size baked into worldgen; render-only tiles in the sim enum; `hasZone` gating with a dead `icehouse` door row | `.chunk` files, `data/tuning/`, one `view` module, `Blueprint.paint`, all thirteen zones always present |
| No schema check on content (unknown fields passed; `quest.returnTo` untyped); flags never cross-checked; names resolved at boot or never | `deny_unknown_fields`, providers, build errors (`ARCHITECTURE.md` §6) |
| Three people composers, three pixel-canvas helpers, seven hash functions, `SCHOOL_COLOR` twice with different values; three quarters of the pixels typed by hand | One composer, one canvas, one hash, one palette; no grids in source (`ART.md`) |
| Presentation timers per frame, so effects, camera and mist ran faster on a 144 Hz display; text by system font | Every presentation timer counts ticks; a stroke font (`PRESENTATION.md` §1.7, `ART.md` §6) |
| Stale docs: the county size, dungeon sizes, the bay margin, the templates path | Corrected in place on 2026-09-26 |

## What carries verbatim

- Every content row under `src/data/`, the 109 room templates, the mission graphs, `sites.json`, `areas.json`, `pois.json`, `anchors.json`, `paths.json`, `doors.json`, `names.json`, the placements and stories.
- The writing: `STORY.md`, `VOICE.md`, `QUESTS.md`, `QUEST-TREE.md`, every line of dialogue and every sign.
- The test list, as design rules: same seed same hash, replay, save and load equality, streams, the skeleton checks, county reachability, the solver on every zone on every seed, C1 to C12, template proofs, density and harshness floors, the quest and truth audits, the co-op rules, the verbs.
- The algorithms: sfc32, the windowed A*, supercover LOS, the 6 × 6 body box, the load ring from block centres, the cast pipeline and its validation order, the double flush, the trigger model with `reset`, the solver's fixpoint, the dungeon layout search, the terrain painter, the flora generators, the corpse styles, the effect kinds, the map chart inks.

## What is dropped

- The DOM and CSS UI, IndexedDB and localStorage storage, the gzip-JSON save format and its eight migrations.
- The hand-typed sprite grids: about 250 sprites, replaced by generators.
- Vite glue: `import.meta.glob` in six places, the seed viewer HTML (rebuilt as `jane view`).
- Browser-era platform plans: PWA, Electron, a WebSocket relay (`PLATFORM.md` §3, §4).
- Seeds. A Rust seed builds a different county from the same number, on purpose.

## The lesson

The Phaser build died of content on an untested engine. This build never did: the tests played the game from the first week and the engine was ahead of the content the whole way. The lesson it adds is narrower and worth stating: **a deterministic engine that is only deterministic on one JavaScript engine has not been proven deterministic**, and a build that cannot run on the machines you want it on cannot be made to by patching. Both are fixed by choosing the platform first. `PORT.md` does.
