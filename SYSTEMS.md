# Jane — MMO-lite Systems Alignment

The production bar for the live build in `jane/`. Pair with `ENGINE.md` (how it works), `LEARNING.md` and `LEARNING-SYSTEMS.md` (what 2020's code did), `DESIGN-2020.md` (what 2020 designed), `WORLDGEN.md` (zones), `MISSING-SYSTEMS.md` (2020 engine not yet carried). This file is what we hold the build against: **depth, scope, complexity, functionality**.

Jane is not a theme-park MMO and not a combat sandbox. It is an **MMO-lite for one to four**: WoW-shaped verbs in a Zelda-sized county, played alone or with up to three friends on a LAN (`PLATFORM.md`). Single-player is a party of one, not a separate path. The player is **Jane**. The product is Julie's county: dog / house / cellar / mine / burial / snake. Systems exist so that every later place is rows, not classes.

This file was rewritten in September 2026 when the Phaser remake was archived. The previous version marked forty-odd rows **IN**; an audit found a dozen of them were partial, stubbed or false (`archive/phaser-remake-2026/POSTMORTEM.md`). Hence the first rule.

> **The bar now applies to the Rust build (`PORT.md`).** By Rule 0, every **IN** below is IN *for the TypeScript build*; for the native build every row is **SHAPE** until a Rust test names it, and the row is re-marked as each `PORT.md` phase gate passes. The contracts in the rows do not change; the file map in §11 is the TypeScript build's record, and `PORT.md` §4 is the crate map that replaces it. Rule 1's "one case in `actions.ts`" becomes one variant of `Action` (`ARCHITECTURE.md` §5); Rule 2's boot error becomes a build error (`ARCHITECTURE.md` §6); Rule 5's list gains "no floats" (`ARCHITECTURE.md` §0).

---

## The bar

| Axis | In | Out |
| --- | --- | --- |
| **Depth** | A verb has validation, cost, cooldown, a failure string, and a data row | A unique class per potion, key, or enemy |
| **Scope** | Every 2020 *contract* that the campaign can touch | TweenGMS, the unused resolution list, pasted WoW quest copy, JaneCraft survival |
| **Complexity** | Shared unit, shared spell pipeline, shared AI loop, one state tree | Parallel player-only combat, a second inventory, a second save schema |
| **Functionality** | The player can *do* the 2020 verbs in this county | Imported GameMaker art, touch layout, audio |

**Rule 0: IN means a test names it.** A row is **IN** only if something in `jane/test/` would fail without it. Anything else is **SHAPE** at best. A status column nobody can run is a wish list.

**Rule 1: add a row.** Do not add a TypeScript class for a new item, spell, recipe, quest, enemy, status, prop, trigger or dialogue node. A new *verb* is one case in `actions.ts`; the compiler lists every place that must handle it.

**Rule 2: a bad row is a boot error.** The catalog validates every cross-reference at start-up and reports them all. Nothing is parsed at run time; nothing unknown is silently ignored.

**Rule 3: content follows the plan, and nothing new starts until the last thing has been played.** The game is now meant to be large (`PLAN.md`: a ten-minute county, three regions, six dungeons), so "thin" is no longer the rule; *ordered* is. If a system cannot be proven on something Jane can do today, it is a stub. Do not add a place so a row has somewhere to stand. Every addition passes the cohesion test in `PLAN.md` §1.

**Rule 4: 2020 bugs stay dead**, and so do 2026's. Recipes key by id. The clock drives light. Path cell is 8 everywhere. The goal cell of a path is enterable. Saves are the state, not a copy of it. Cast never reads a missing target. Range is checked before cooldown. Rewards never vanish into a full bag. A lock-in undoes itself when you die in it.

**Rule 5: the sim is headless and deterministic.** Nothing under `sim/` or `world/` touches the DOM, the wall clock, `Math.random` or runtime trigonometry. If a change makes `test/replay.test.ts` fail, the change is wrong.

---

## Status key

| Mark | Meaning |
| --- | --- |
| **IN** | In the live build, and a test would fail without it |
| **SHAPE** | Code or data exists; no test holds it, or presentation is provisional |
| **LATER** | A real 2020 idea, not needed to ship the county. A row or a module when it lands |
| **NEVER** | Do not port |

Test names are from `jane/test/`: **E** `engine`, **S** `sim`, **D** `dungeons`, **W** `world`, **R** `replay`, **A** `art`, **T** `rest`, **C** `coop`, **G** `storage`, **B** `budget`, **K** `skeleton`, **Y** `county`, **Q** `quests`, **L** `templates`, **N** `dungeon-gen`, **V** `dungeon-verbs`, **X** `verbs2`, **M** `museum`, **U** `burial`, **I** `library`, **F** `forest`, **P** `pipes`, **FA** `factory`, **SC** `school`.

---

## 1. Session / boot / time

| Contract | 2020 | Live | Bar |
| --- | --- | --- | --- |
| Persistent session | `game` object | one `GameState` tree (`sim/state.ts`) | **IN** — S: save → load → continue equals never having saved |
| Fixed step | 60 fps frames | integer ticks at 1/60 s; accumulator + interpolation (`app/loop.ts`) | **IN** — E: seconds → ticks; S, R |
| Determinism | — | seeded sfc32 in state, no runtime trig, quantised input | **IN** — S: hash per 120 ticks; R |
| Slot save | compressed `slotN.jsav` | 3 slots, versioned `SaveFile` + migrations, gzipped (`app/gzip.ts`) into IndexedDB; slot rules in `app/slots.ts` (sync summary cache, per-slot write queue, `localStorage` fallback and one-time migration) | **IN** — S: garbage and newer saves decode to errors; G: gzip round trip on a real save, write order, fallbacks, migration, corrupt slot reads empty / **SHAPE** the IndexedDB wrapper itself (`app/storage.ts`): no test can reach it |
| Replay | — | input runs + tick-stamped commands, re-simulated | **IN** — R |
| Day clock | `time += 1/7200`, lit lamp posts only | same rate; starts **17:00** (`Story.docx`); drives ambient light and lamp posts | **IN** (rate, start: S) / **SHAPE** (light is presentation) |
| Pause | broadcast user events | the scheduler is not called; dialogue freezes from inside | **IN** — R covers frozen ticks |
| Terminal | backtick | `` ` `` + `app/terminal.ts`, a table of rows; cheats are sim commands | **SHAPE** |
| Title / load / controls | TODO | title, 3 slots, controls table | **SHAPE** |
| Debug | F2 / F3 / F5 / F6 | F2 overlay (fps, sim ms, draw ms, awake, chunks, paths, hash), F3 grid, F5 / F9 | **SHAPE** |
| Distance unload | `distance_unload`, 768 px | `sim/ring.ts`; sleepers still tick clocks | **IN** — D: bait test needs the lurker inside the ring |
| Prop buckets | `instance_place` / collision lists | `sim/runtime.ts`: props bucketed by 16 x 16-cell block, queries answer in id order (`propsNear`, `propsInCells`); `moveProp` is the only way a prop changes cell; USE, push, plates, the ring, prop flags and the renderer ask the buckets, never the zone | **IN** — B: every prop a whole-zone pass finds is found, in id order, moved props included; local re-stamp equals a full one |
| Tick budget | — | 3,000 units and 8,000 props asleep across the county, median tick under 2 ms (measured: 0.16 ms) | **IN** — B: sleepers stay asleep, still respawn on schedule, the yard skeleton still aggroes |
| Resolutions list | dead | integer-scaled low-res framebuffer | **NEVER** |

### 1b. Party (co-op-ready; no network yet)

2020 was built MMO-shaped for this and never got there.

| Contract | Live | Bar |
| --- | --- | --- |
| Seats | `state.players[]`, four at most; a fifth `join` is refused | **IN** — C |
| Join / leave | commands in the recorded stream; leaving parks the body, the same client token gets it back | **IN** — C |
| Open / closed | a world is single-player until seat 0 opens it; closing sends nobody home; a save loads closed | **IN** — C |
| One heroine | one name on the state, the host's; no player has her own | **IN** — C, T |
| Coat per seat | same drawing, palette swap of `c C q`: plum, teal, moss, ochre. No name labels | **IN** (the sheets) — C / **SHAPE** (the draw) |
| One fire | `state.rest`: everyone wakes there, everyone arrives there, anyone can move it, nobody is called back | **IN** — C, T |
| A guest's rest saves | the `rest` event reaches every seat in every zone | **IN** — C |
| Shared growth | `state.growth`: one learns, all know, the away and the late included. Items stay personal | **IN** — C |
| Story items survive a disconnect | keys and `acquire` targets are handed to a staying player; overflow drops at her feet and never ages out | **IN** — C |
| Own numbers only | `damage` / `heal` carry `from`; the renderer floats only this seat's | **IN** (the field) — C / **SHAPE** (the filter) |
| Heal a friend | spell kind `ally`. Mouse: the friend under the cursor (`on` in the command), else yourself; out of reach fails loudly. Pad: the friend along the right stick, else yourself | **IN** — C (five tests), on a row made by the test. No heal spell is placed yet |
| Hover brackets | the friend under the cursor is marked at her feet in her coat colour | **SHAPE** |
| Per-seat input | `tick(inputs[])`, `command(seat, c)`; one seat's keys never move another | **IN** — C |
| Split party | every zone with a connected player ticks; an empty zone stops and keeps its state | **IN** — C |
| Party penalty | by head count *connected*, server-wide: deal 62 / 46 / 38%, take 115 / 130 / 145%. The world is never rebalanced | **IN** — C |
| Shared story | quests, flags, kills count for all; hand-in rewards pay everyone | **IN** — C |
| Chest and ground loot | first come: it belongs to whoever opens or picks it up. Decided | **SHAPE** — a consequence of the code, no test |
| Nothing pauses with company | dialogue and windows freeze the world only for a party of one | **IN** — C |
| Lock-ins | hold while anyone is alive inside; reset when the last one falls | **IN** — C |
| Sleeping the night | only when the whole party is resting | **IN** — C |
| Event routing | `to` a seat, `inZone`, or everyone; `Sim.eventsFor` | **IN** — C |
| Multi-seat replay | format v3, seats in frames and commands, name in the header; hashes match | **IN** — C, R |
| One seat's view | UI and renderer take a `PlayerView`, never the sim | **SHAPE** |
| Console | `open`, `close`, `party`, `join [who]`, `leave <seat>` | **SHAPE** |
| Network | host relay + lockstep client | **LATER** — `PLATFORM.md` §4 |

## 2. Unit (the MMO actor)

One `Unit` shape. `controller: player | ai | npc | snake`. Do not split Player / Enemy.

| Field | Bar |
| --- | --- |
| HP / MP = str×5 / spi×5; MP regen spirit/1000 per tick; no player HP regen | **IN** — S |
| Energy: sprint −0.5, carry −0.25, else +0.5; empty locks until full; melee refunds 3 (9 on crit) | **IN** (S tape exercises it) |
| GCD 90 ticks, per-spell cooldown, `stop` on cast | **IN** — S |
| 24-slot bag, spellbook, 4-way facing, aim vector | **IN** — S |
| Incoming damage queue; hp changes only at the flush | **IN** — S: "queues damage and only changes hp at the flush" |
| Resist per school (unit row × status rows); schools are `heal physical frost fire nature blast shock`; an effect may strip the row's resists (`noResist`: `softened`) or land only on what is weak to a school (`onlyIfWeak`: `jolted`) | **IN** — X: a plated machine takes a fifth of a blow, all of it when softened, and one and a half times a spark either way; the spark jolts it and not a skeleton |
| Status list as `effects.json` rows read by every unit | **IN** — D (chilled, poisoned via play); catalog E |
| First-hit aggro; aggro scan every 10 ticks, staggered without 2020's dead-counter bug | **IN** — S: "enemies chase a player who is standing still" |
| Respawn timer (ticks while asleep too); `respawn: 0` stays dead | **SHAPE** |
| Player death: stand back up at the last arrival mark after 4 s; lock-ins reset | **IN** — D: "dying inside a lock-in re-opens the gate and re-arms the trap" |
| Dialogue hook: unit row `talk` | **IN** — S golden path |
| Patrol waypoints; home follows the patrol; a waypoint may carry a **dwell** in ticks (third number in `UnitSpawn.patrol`, `Unit.patrolDwell`); `npc` units keep a patrol too | **IN** — X: a rat and a friendly `npc` stand 120 ticks at the point that has a dwell and walk on from the one that has none; a generated holding's `patrol` becomes cells and dwells |
| Boss phases by health: `phases[n].hpBelow`, the phase's `book` and `run`, and `onEnter` run once with the boss as subject, for whoever struck; whole again, it starts over | **IN** — X: two thresholds, one blow through both, the reset |
| Occupy path cell (every awake living unit, not only in combat) | **IN** — E: "paths around other units' cells" |
| Range between bounds (`bounds` row field) | **IN** — S |

## 3. Combat / spells

The pipeline is the product: validate → spawn a kind → pay only if valid → queue → flush.

| Contract | Bar |
| --- | --- |
| Spell row: kind, school, costs, range, cooldown, `power {stat, div, varDiv, flat}`, effect | **IN** — E catalog |
| Error enum + toast map; GCD / cooldown stay quiet | **IN** — S |
| Validation order: range **before** cooldown | **IN** — S: "checks range before cooldown" |
| Missing target is an error value, not a crash | **IN** — S: "survives a target that no longer exists" |
| Failed cast costs nothing | **IN** — S |
| Kinds: `melee`, `bolt`, `self`, `world`, `ground` | **IN** (melee, bolt, world: S, D) / **SHAPE** (self, ground) |
| Aim-based player casting; AI casts at target; bolts do not home | **IN** — D: torches lit by an aimed Icebolt |
| Bolt fan / ring (`count`, `fan`), splash, carried light | **IN** — D: snake ring ≥ 15 bolts; X: Explosion's splash is the whole blow (`div` 1) |
| Spell `touch` (px): how near a prop's middle a bolt must end to switch on what answers its school; 14 when left out | **IN** — X: a blast finds the middle of a 5 x 3 pile from its corner, and with the old fixed 14 it does not |
| Player verbs as rows: `explosion` (bolt, blast, splash, touch 28), `grow` (world), `spark` (bolt, shock, `jolted`); effects `dazzled`, `jolted`, `dusted`, `softened` | **IN** — X (the rows and what they do), A (their icons) |
| Bolts die on the first sight-blocking cell or enemy body; water does not block them | **IN** — E: LOS |
| Crit 1-in-20 ×2; melee `str/8 + irandom(str/32)`; Icebolt `spi*0.8 + irandom(spi/8)`; Fireball `spi + irandom(spi/2)` | **IN** as rows |
| AI uses the same `tryCast`; first affordable spell in the book | **IN** — S, D |
| Potions as effect rows (manashield, lifesteal, critical, stoneskin, firelash, sparktongue, winterbite, stranglethorn) | **SHAPE** — rows exist and validate; no test drinks one |
| Numbers follow the 2020 balance sheet, phase 1 | **SHAPE** — see `DESIGN-2020.md` §3.1 |

**Scalability:** a new spell is a JSON row. A new status is an `effects.json` row and needs no TypeScript, because no behaviour is keyed by effect id.

## 4. Items / bags / craft / bar

| Contract | Bar |
| --- | --- |
| Item row; stack, leftover returned | **IN** — S |
| Use → consume, item cooldown, GCD, `stop` | **SHAPE** |
| Keys are an `opens` tag; one use path; consumed unless `bound` | **IN** — D: cellar |
| Action bar: 8 slots `{source, id}`; bind, unbind, swap; learning fills the first free slot | **IN** — S golden path |
| Craft: 3 in + 1 out, **sorted ids**, inputs consumed only if the output fits | **IN** — S, E |
| The 2020 recipe list (11) | **IN** — E |
| Crafting needs a bench in reach | **SHAPE** |
| Ground drops live in zone state (survive travel and saves); story drops never expire | **IN** — D: keys looted from drops |
| Rewards that do not fit land at your feet | **IN** — S: "never loses a reward when the bag is full" |
| Bound items refuse destroy | **SHAPE** |
| Hunger / warmth | **NEVER** — JaneCraft, not Jane. The 2026 version was inert |

## 5. Quests / dialogue

Three quest types only: `kill`, `acquire`, `location`.

| Contract | Bar |
| --- | --- |
| Quest row + progress; rewards are an action list | **IN** — S |
| `acquire` reads the bag live; `location` reads a persistent flag, so early visits count | **IN** — S golden path (the letter completes on arrival) |
| Every quest can be given and handed in by some row | **IN** — E (the rows, placements included), Q (a built county: dialogue, triggers, item use, unit deaths and the action lists on placed props, followed to a fixed point, on two seeds) |
| Every `location` a quest names is produced by something reachable; every `kill` target is spawned; every `acquire` target can be had (loot on a placed prop, a guaranteed drop, a recipe, a `give`) | **IN** — Q |
| Dialogue tree: first-match `start` rules, lines, ≤ 2 options, actions, goto | **IN** — S golden path |
| Hand-in rules sit **above** state rules in every tree (2026's dog hid two hand-ins behind a flag), and every `questReady` row leads to its hand-in. One documented exception: `garden_book`'s night line for `rose_and_stone`, whose hand-in home is the dog | **IN** — Q |
| The Lowfields side quests (`QUESTS.md` Part 3): 19 quests in 8 chains, all rows in `data/*/lowfields.json` fragments, placed by name on every seed (`data/placements/lowfields.json`), things as givers (a book, a board, a door, a stone, a glovebox, a slate) | **IN** — Q plays all 19 end to end: offered, accepted, done, handed in, paid, and not payable twice |
| Quest growth: jars and pages arrive through `grow` with an id, once per world however many are paid | **IN** — Q (three jars, three pages), C (the verb) |
| After the bell: `night` rows in trees, a `nightOnly` creature, and show/hide behind wide rects so nothing is seen to change (the parcel, the scarecrow omen) | **IN** — Q |
| A quest survives being done out of order (place first, item first, giver last; the Tenant put down before the notice is read) | **IN** — Q |
| Story beats as a separate system | **NEVER** — flags and quests already say it. The 2026 `beats.json` failed silently out of order |
| WoW paste / "Good Job JoJo" | **NEVER** |

## 6. AI / path

```
idle (regen, patrol, bait, aggro+LOS every 10 ticks)
  → combat (first affordable spell; path in on tooFar / notInLOS; leash past range)
  → leash (clear target, regen, run home) → idle
```

| Contract | Bar |
| --- | --- |
| Idle / combat / leash | **IN** — S |
| A\*: cell 8, 8-way, no corner cutting, typed-array scratch, node budget, max length, enterable goal | **IN** — E (five tests, including a 50 ms bound on a sealed goal) |
| ≤ 4 searches per tick; re-plan every 20 ticks or when the goal moves | **SHAPE** |
| LOS: integer DDA supercover | **IN** — E |
| `bait` row field (the unfightable burial snake) | **IN** — D |
| `litAt(w, x, y)`: is this point inside two thirds of a showing prop light's radius. Prop lights only (never her glow, a bolt or a unit's), read off the prop buckets, no cache; one rule (`propLightShowing`) shared with the renderer: `lightWhenOn`, `nightOnly` (18:30 to 06:30), `dayOnly` (the rest), hidden | **IN** — X; B still holds the tick budget |
| `sight: "lit"` (unit row): notices and keeps only a target standing in light | **IN** — X: blind in the dark beside a skeleton that is not, sees her under a lamp, loses her when it goes out, takes the lit one of two players |
| `shunsLight` (unit row): paths round warm light (`light.cold` does not count), stops at the nearest dark cell and waits, leaves if caught in it | **IN** — X: 600 ticks at the edge without once standing in the light or touching her, at a re-plan every 20 ticks; comes in when the lamp goes out |
| `send` orders (`Unit.order`): walk to a mark minding nothing, run a list on arrival as the sender, give up on a tick budget; kept awake outside the ring; saved | **IN** — X: past a player inside its aggro; a walk with no way is given up; a save mid-walk loads into the same walk; the sender of two players is paid |
| Snake boss: the one custom mover. Turn clamp, 900 / 300 tick phases, reset on broken LOS, trail nodes, forwarding tail hitboxes | **IN** — D |

## 7. World verbs

| Verb | Bar |
| --- | --- |
| Zone travel by named mark; refuses while carrying | **IN** — S, D |
| Doors that are not answered after dark: `nightLock` on the placed prop, a line said instead of opening, outside doors only so nobody is shut in | **IN** — T. No door in the county carries it yet; it is a choice made door by door |
| Locked door + key tag; gates (solid while locked) | **IN** — D |
| Chest loot once; keeps what did not fit | **IN** — D |
| Push / pull: hold USE 30 ticks, 20 energy, one cell | **IN** (push) — V: a bot holds USE against a barrel and it moves one cell / **SHAPE** (pull) |
| Sill: tile flag `F_NOPUSH`. Feet cross it, a pushed prop does not; the floor just inside every generated door, so barrels never leave their room | **IN** — V: the second push, onto the sill, is refused; she walks out over it |
| Carry / put down in front, never on your own cell | **SHAPE** |
| Pressure plate held by a unit **or a pushable**; `release` re-locks | **IN** — D: mine |
| Repair: world-kind spell, prop `answers: "repair"`, consumes `needs`, refuses without | **IN** — D: mine |
| Grow: world-kind spell, prop `answers: "grow"`, and only in light (`litAt`); in the dark it costs nothing and says so | **IN** — X |
| School touch: frost wakes `torch_blue` | **IN** — D: burial |
| Learn-spell prop (a dialogue tree with a `learn` action) | **IN** — S golden path |
| Lever / toggle (`use` list, `on` state) | **IN** — N: the hoist lever, hidden until its hoist is mended, pulled once |
| `if` (conditions, `then`, `else`): a list that depends on the world, asked as whoever is acting; checked at boot all the way down | **IN** — X: both branches, `hasItem` differing between two players, a bad row inside a branch |
| `send` (a unit by key, to a mark, `then`) | **IN** — X (see AI) |
| `reveal` (named rects into the fog, the party's); a prop that is read may also run its `use` (the wall notice) | **IN** — X: the mine's notice shows every room this seed placed and no corridor |
| `show`, `lock` and a solid `fill` never land on a unit: shown and locked things stand her aside (a walk outward over open floor, never through a wall); a solid fill waits, saved, until its whole rect is clear | **IN** — X: an exhibit against a wall, the big door on two units, a hedge that does not grow through her or round her and lands after a save and a step away |
| A prop row's own `prompt` wins over "Open" for something holding loot | **IN** — X: Gather, Pick up, Open |
| Console `kill`: what she can see within 25 m, never the hidden, never through a wall | **IN** — X |
| `strike`: everything hostile in a named rect is hit once, optional effect, friends spared unless `hitsFriends`; the blow (kill, quest count, party penalty) is the puller's | **IN** — V; N: a mended hoist dropped on Iron Knuckles, 200 and `staggered` |
| A prop's `to` may name the zone she is in: moved to the mark, nothing reloaded (way in, vent, drop) | **IN** — V: same runtime, same zone state, no travel pending |
| Growth props: `jar`, `jar_big`, `leaf_page`; `use` is `grow` with the prop's own key as the id | **IN** — N: the cabinet jar gives once and only once; the vault page hides itself |
| Trigger rects: `enter` / `while`, conditions, actions, `reset` on player death | **IN** — D: lock-in, arena, torches |
| `Blueprint.triggers`: rows a builder writes, merged with the catalog's for the zone in the sim and in the validator; checked like catalog rows; an id in both is an error | **IN** — V: the mine's arena rows ride on the blueprint and re-arm on death; the validator refuses a clash, a missing prop, a bad row |
| Location | **IN** — S, D |
| Fog: one bit per 16 px block, interiors, in zone state | **IN** (the bits: X, through `reveal`) / **SHAPE** (the draw) |
| Tile edits as `tileDeltas` (`fill` action) | **IN** — X: solid and walkable fills, across a save. No placed content uses it yet; it is what Grow and drain will be |
| Lily-pad rafts, cart on a track | **LATER** |

## 8. GUI / camera / input

| Contract | Bar |
| --- | --- |
| Low-res world framebuffer, integer scale; DOM UI at native resolution | **SHAPE** |
| HUD vitals, statuses, target / boss bar, toasts (max 3), interact prompt, quest tracker | **SHAPE** |
| Windows: inventory + craft, spellbook, quests, map; drag between bag / bar / book / craft | **SHAPE** |
| Dialogue box, pause menu, title, console, F2 overlay | **SHAPE** |
| One action layer: keyboard + mouse, and the 2020 pad layout | **SHAPE** |
| Camera: eased follow, zone clamp, lock to a named rect, decaying shake | **SHAPE** |
| One lighting pass: cached lightmap, add lights, multiply | **SHAPE** |
| Art as source: palette-grid sprites and icons for every row | **IN** — A |
| Touch layout, options menu, audio bus | **LATER** |

Presentation is **SHAPE** by definition until a browser test exists. That is honest, not modest: the sim is what is proven.

## 9. Content vs system

The campaign that proves the systems. It is what 2020 **built**, at phase-1 numbers.

```
Jane + letter (Sunday train, 17:00, sunset)
  → walk in the gate → the stoop (location) → dog (dialogue, quest give / hand-in)
  → yard skeleton (kill) → house key (quest reward)
  → kitchen (location): note (quest), pantry, bench (craft Manashield), ice orb (learn Icebolt)
  → cellar: iron key ×2, two iron doors, rats (meat), plain key, storage (wood, iron), roses, potion room
  → dog: rats → snakeroot → Stranglethorn → Poisoned Rat Meat
  → mine (the dog points; Repair is in the Headmaster's drawer): plate + barrel, plain keys, clerk (HM key),
       Headmaster (vault key), broken steps (Repair, 2 wood), ornate chest (boss key), the adit,
       vault (gold, Museum key), Iron Knuckles
  → burial: cold torches (Icebolt), rat maze (Snake Key), lock-in (Giant Snake Key),
       garden (flower holds the roots; the last page: Fireball), snakes you feed, the Snake
  → dog: "There is a museum across the river with a wing marked MAGIC, and you are holding the key to it."
```

Enemy rows may be as wide as 2020 (bat, rat, bandit, soldier, pumpkin, cactus, flower, statue, spiders). Spawn them where a zone already has a hole.

## 10. Scalability checklist (use in review)

A change is on-bar if:

1. A designer can add the thing as a row without opening `sim/`
2. Player and AI share the validation / cost / incoming path
3. Failure is an enum or a boot error, never a silent no-op (GCD and cooldown stay quiet, like 2020)
4. It lives in the state tree, so it saves and replays without anyone remembering it
5. Metres for combat, pixels for movement, cell = 8, ticks for time
6. Every seed of every zone still passes the solver
7. A test names it

A change is off-bar if:

- New `class Fireball extends Spell`, new `obj_onUseKey_*` equivalent
- Behaviour keyed by an effect, item or prop **id** in TypeScript
- A recipe keyed by display name; a quest type that is not kill / acquire / location
- An action as a string to be parsed at run time
- `Math.random`, `Date.now`, `Math.sin` or a DOM type under `sim/` or `world/`
- State on a sprite, a scene, a singleton, or anywhere but `GameState`
- Combat that writes `hp` in the same call that queues damage

## 11. File map (TypeScript build, record; the Rust crate map is `PORT.md` §4)

| Concern | Lives here |
| --- | --- |
| Rows | `jane/src/data/` — `spells effects items units props recipes quests dialogue triggers start` `.json` |
| Row types + boot validation | `sim/catalog.ts` |
| State tree, Action / Condition unions | `sim/state.ts` |
| Scheduler, commands, dev ops | `sim/sim.ts` |
| Derived runtime (grid, path scratch, lookups) | `sim/runtime.ts` |
| Unit, movement | `sim/units.ts` |
| Cast / bolts / grounds / flush / death | `sim/combat.ts` |
| Status | `sim/status.ts` |
| AI / path / LOS / angles | `sim/ai.ts` `path.ts` `los.ts` `angles.ts` |
| Snake | `sim/snake.ts` |
| USE, push / pull / carry, Repair, Grow, school touch | `sim/interact.ts` |
| Light the sim can see (`litAt`, the one rule for a showing prop light) | `sim/light.ts` |
| Nothing solid lands on a unit (nudge, pending fills) | `sim/clear.ts` |
| Verbs and conditions | `sim/actions.ts` |
| Bags, craft | `sim/inventory.ts` |
| Quests / dialogue / triggers + plates | `sim/quests.ts` `dialogue.ts` `triggers.ts` |
| Load ring / zones, travel, fog | `sim/ring.ts` `zones.ts` |
| Save codec, hash / replay | `sim/save.ts` `replay.ts` |
| Zones + solver | `world/county.ts` `interiors.ts` `mine.ts` `burial.ts` `kit.ts` `validate.ts` `index.ts` |
| Generated dungeons | `world/dungeon/` (`types.ts` `room.ts` `pools.ts` `layout.ts` `generate.ts` `checks.ts` `harness.ts` `index.ts`), missions in `data/dungeons/*.json`, rooms in `world/dungeon/rooms/<dungeon>/*.room` |
| Art | `art/units.ts` `props.ts` `icons.ts` |
| Renderer | `render/renderer.ts` `tiles.ts` `lighting.ts` `atlas.ts` |
| Input | `input/input.ts` |
| UI | `ui/host.ts` (the contract) + `ui/*` |
| Shell | `app/app.ts` `loop.ts` `storage.ts` `terminal.ts` |
| Tests | `jane/test/` |

2020 GML is **read-only** and stays out of the repo. Steal contracts. Do not port line-for-line.

## 11b. The county skeleton, and the county built from it

| Contract | Live | Bar |
| --- | --- | --- |
| Story places are rows with rules (`sites.json`): region, terrain, distances by road or by line, across the river, off the road | solved site by site with its road, re-tried locally, re-rolled if it cannot hold | **IN** — K, 64 seeds (1,000 as a soak) |
| Roads join the story and merge into one network; one to three bridges | A\* over roughness and slope | **IN** — K |
| Five minutes across by road | the far shore is ≥ 2.2 km from the platform (3.6 km until the county went square, 2026-09-24) | **IN** — K |
| The School stands over the town | crown site, town capped in height | **IN** — K |
| Danger is a map: region base, named patches (`areas.json`), dungeon rings, roads, havens | `threat` 0..6 per macro cell | **IN** — K |
| Night and lamps change the map | `threatAt(s, x, y, night)` | **IN** — K |
| The first walk is safe and lit | checked on every seed | **IN** — K |
| Density: 22 to 30 small places per region, nothing-to-see stretches ≤ 900 m | roadside beat, then banks and deep country | **IN** — K |
| Seed viewer | `viewer.html` | **SHAPE** — hand-checked |
| The playable county is built from the skeleton: 2000 × 2000, roads, bridges, set chunks joined by their gates | `world/county.ts`, `world/chunks.ts` | **IN** — Y: every mark reachable from the platform on two seeds; W: the lock-and-key solver on four |
| New Game starts on the platform; Julie's gate is two to four minutes by the lit road | a bot walks it | **IN** — Y |
| Wildlife by region, biome and threat; a spawn's `phase` scales its row | `PHASE_SCALE` | **IN** — Y |
| Nothing spawns in a haven, on the first walk, or in a wall | | **IN** — Y |
| Later dungeons stand as shapes with a mark and no door | | **IN** — Y (marks) |
| Map window: one pixel per block of cells past 900 across; roads and water win the block | `ui/map.ts` | **SHAPE** |
| Places the story needs, guaranteed by name: anchors with marks and rects, required patches, footpath ends, chunk and area slots | `skeleton/anchors.ts`, `areas.ts`, `data/anchors.json`, `data/paths.json` | **IN** — Y, K |
| Content placed by name: mark, site, patch (optionally spread), kind of small place, anchor, slot, edit | `world/placements.ts` | **IN** — Y, Q |
| The School as a far landmark along the top of the screen | `render/renderer.ts` `drawSchool` | **SHAPE** — hand-checked from the platform and the town |
| Set chunks as data, not code | — | **LATER** |

## 11c. Generated dungeons (`DUNGEONS.md`, `ENGINE.md` §8.3)

| Contract | Live | Bar |
| --- | --- | --- |
| Room templates | `.room` text grids in pools (`world/dungeon/rooms/<dungeon>/`), parsed, turned and mirrored by `room.ts` | **IN** — L: every file parses; doors stay on the rim and sockets on the floor in every transform |
| Template lint | rim, sills, nothing solid on a sill or between facing doors, spawns 4 cells from a door, 2 cells of width between doors, a push path to every plate, no lever under its own hoist, every claimed transform fits its bays | **IN** — L: clean on every file; each rule shown to catch a room broken for it |
| Pool contract | variants differ in grid, never in required doors, needs, grants or blocks; every socket, mark and rect a mission names exists in every variant | **IN** — L |
| Template harness | the room alone, holding what its node holds, a stub on every door: grants reached and every door left from every door; blocks hold by ablation | **IN** — L: all 23 mine templates; a careless mission that unlocks the plate chest is caught |
| Mission lint | grants follow from holdings; a sight edge has a corridor; C3 on the mission alone | **IN** — L |
| The generator | choose, embed on the bay lattice (depth first, scored), route on the lines between bays, lock, fill, name, emit | **IN** — N: 64 seeds (1,000 as a soak), zero fallbacks, at most 3 attempts on the 1,000; same seed same mine; hub and boss room in many different bays |
| Fallback embedding | the last attempt stamps the mission's hand-placed layout through the same steps | **IN** — N: it is a whole mine and passes every check |
| Names | bound names from the mission; the rest `${zone}_${node}_${socket}`, never a coordinate or the attempt | **IN** — N: every `grow` id is its prop's key, and the set is the same across attempts, seeds and the fallback; the derived contract contains the hand-written one |
| Heat | `round(heat x baseHeat)` capped by the template, spent on the mission's bestiary, two to a socket, every unit at the mission's phase; rest rooms empty | **IN** — N |
| Lock-in macro | one function: lock, show the way in, wake or spawn, toast; reset; clear. The way in is hidden until the room seals | **IN** — V: hidden, shown, climbed, hidden again; a death re-arms it. N: C6 |
| Solver: verbs, `when`, hops, ablation | `validateBlueprint` options `verbs`, `withhold`, `shut`, `entry`, `trace` | **IN** — V: no Repair, no gallery; no flag, no nook; Headmaster shut away, no drawer; the arena reached over a shut gate only when the way in shows |
| Solver: `if` | `then` once its conditions can hold, `else` if they did not when the list first ran; a `then` that cannot run yet is kept for anything that can be worked again, and not for a lever that works once | **IN** — X |
| Solver: floods only when feet can go somewhere new | a gate opening or a blocking prop shown or hidden floods again; a chest unlocking, loot, a kill, a flag go round the settle loop. County validation, seed 3: 723 ms to 118 ms | **IN** — X: a chain of locked chests is one flood, a gate is two |
| The stateful flood | `validateBlueprint` option `states` (at most three two-valued flags; unset = how the zone starts). Nodes are (cell, state); a control (a prop whose `use` sets a state flag) reached in one state is an edge to the state its list leads to, entered at the control; what controls lock, unlock, show and hide is how the states differ; everything else stays monotone. A state that looks different by two routes is an error | **IN** — X: accepted with the breaker, refused without it, and refused when the breaker is behind a door only the other state opens |
| `state` edges and controls | mission `states`, edge `{ t: "state", var, is }` (a keyless gate, shut in the starting state unless `is` is it), holding `controls` (+ `becomes`): the generator writes the one `if` list that flips the flag and drives every such gate both ways; grant `{ state }`; C1, C3 and C8 know the kind | **IN** — X, on a test dungeon that is in nobody's data folder |
| The adit | a door in the gallery (behind Repair) to the county mark `mine_adit`, round the hill from the mouth; the county's side is barred until the gallery has been stood in, then a way back in | **IN** — X; W: both doors lead to marks that exist |
| Checks C1 to C12 | `world/dungeon/checks.ts`, run by `buildZone` through the zone's `check` hook; a failing candidate is re-rolled and the error names the check | **IN** — N: every seed passes, and each of C1 and C3 to C12 rejects by name a blueprint or a mission broken for it (C2 falls out of C1) |
| The Museum, generated | `data/dungeons/museum.json`: one breaker driving a dark-only gate and a light-only one, the stores behind an exhibit that answers blast, 2020's key shuffle kept whole, the Shot-Firer, the Attendant throwing the breaker at 75/50/25% | **IN** — M: 64 seeds (1,000 as a soak), zero fallbacks; the solver refusing the same layout with the breaker withheld; a solo bot to the big jar; two seats at the breaker |
| The library and Butterfly Forest, generated | `data/dungeons/library.json` (four rooms, nothing alive, Grow taught and tried in one room) and `forest.json` (outdoors, `indoor: false`, not one key, light as the lock; Grow blooms buds, bridges a stream and closes hedge gaps) | **IN** — I, F: 64 seeds each, zero fallbacks; **every closable hedge gap shut and the dungeon still finishes**; a solo bot nets six butterflies and beats the Emperor |
| The Burial Chamber, generated | `data/dungeons/burial.json` at phase 5: the built content re-hosted and bound to the same names, four corners in the player's order, shades, the great torch, Goldskin | **IN** — U: 64 seeds, zero fallbacks; the old snake chain still played by the bot (D); a shade refusing warm light; Goldskin softened by fire and beatable without it |
| The pipes, the Factory and the School, generated | `data/dungeons/{pipes,factory,school}.json`: one valve and two runs; light as the rule the machines see by; a timetable on a bell rope | **IN** — P, FA, SC: 64 seeds each (256 as a soak), zero fallbacks, the solver refusing each layout with its control withheld, a solo bot through each |
| What a verb gives back to the county | doors that appear with their zone, the mine's adit, and relay boxes on the longest dark roads that light a whole run for good | **IN** — Y |
| A builder's loot is checked | an item that does not exist throws when she opens the chest, so the validator names it instead | **IN** — W |
| The Gold Mine, generated | mission in `data/dungeons/mine.json`: Repair in the Headmaster's drawer (shut while he stands) with a safe first use in the same room, First Aid off the hub, the track shortcut on the store's iron, four hoists, the nook and its gates, the cage side room; two variants per pool, one arena | **IN** — D: the old chain, now reading the drawer. N: a solo bot does all of it on three seeds |
| Save v7 | a mine saved from the hand-built layout is dropped; whoever saved in it is put at the mine mouth | **IN** — V |
| Save v8 | `Unit.order`, `patrolDwell`, `dwell`; `ZoneState.pendingFill` | **IN** — X: a v7 file loads, gains them, and plays on to the same hash |
| Co-op tags on templates (`plate_or_friend`, `twin_hold`, `lure_and_lever`) | parsed and kept; nothing reads them | **SHAPE** — the two-bot tests of `DUNGEONS.md` 2.8 are not written |
| Template bot harness (a bot plays each room in each transform) | — | **LATER** |
| The powder store, the dungeon viewer page | — | **LATER** — the powder store wants a `rubble` row that answers `blast` (the Museum's); the spell is in |

## 12. Intentionally later, in order

**The order of work is now `PLAN.md` §8** (engine at scale → skeleton generator and seed viewer → the Lowfields → dungeon generator → omens and the text pass → the Waters → the Works). The list below is the engine-side backlog those milestones draw from.

1. **A browser smoke test** (boot, new game, walk, open bags) so presentation rows can start earning **IN**.
2. **Push / pull / carry under test**, then a lever placed somewhere real.
3. **Museum** — the next 2020 zone; key shuffle, light switches, a door Repair mends. Its key is already in the mine vault.
4. **Explosion** and **Grow** are spell rows now, with the engine under them (X); what is left is the things that answer them, then **Butterfly Forest** as 2020 drew it: eight butterflies, no doors.
5. **Cart on a track** (`machine.kind = cart`), lily-pad rafts.
6. **Pipes → Factory** with the Electric spell (`spark` is a row; `sight: "lit"` is in); the orb waits in the cellar study.
7. The other three burial corners and the wizard; **School**.
8. Audio bus; touch layout; options; res sickness on death.

Phase numbers move up one row of the balance sheet per dungeon. Do not pull from this list until the rows above it stay green.
