# Jane — MMO-lite Systems Alignment

The production bar for the live build in `jane/`. Pair with `ENGINE.md` (how it works), `LEARNING.md` and `LEARNING-SYSTEMS.md` (what 2020's code did), `DESIGN-2020.md` (what 2020 designed), `WORLDGEN.md` (zones), `MISSING-SYSTEMS.md` (2020 engine not yet carried). This file is what we hold the build against: **depth, scope, complexity, functionality**.

Jane is not a theme-park MMO and not a combat sandbox. It is an **MMO-lite for one to four**: WoW-shaped verbs in a Zelda-sized county, played alone or with up to three friends on a LAN (`PLATFORM.md`). Single-player is a party of one, not a separate path. The player is **Jane**. The product is Julie's county: dog / house / cellar / mine / burial / snake. Systems exist so that every later place is rows, not classes.

This file was rewritten in September 2026 when the Phaser remake was archived. The previous version marked forty-odd rows **IN**; an audit found a dozen of them were partial, stubbed or false (`archive/phaser-remake-2026/POSTMORTEM.md`). Hence the first rule.

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

Test names are from `jane/test/`: **E** `engine`, **S** `sim`, **D** `dungeons`, **W** `world`, **R** `replay`, **A** `art`, **T** `rest`, **C** `coop`.

---

## 1. Session / boot / time

| Contract | 2020 | Live | Bar |
| --- | --- | --- | --- |
| Persistent session | `game` object | one `GameState` tree (`sim/state.ts`) | **IN** — S: save → load → continue equals never having saved |
| Fixed step | 60 fps frames | integer ticks at 1/60 s; accumulator + interpolation (`app/loop.ts`) | **IN** — E: seconds → ticks; S, R |
| Determinism | — | seeded sfc32 in state, no runtime trig, quantised input | **IN** — S: hash per 120 ticks; R |
| Slot save | compressed `slotN.jsav` | `localStorage`, 3 slots, versioned `SaveFile` + migrations | **IN** — S: garbage and newer saves decode to errors |
| Replay | — | input runs + tick-stamped commands, re-simulated | **IN** — R |
| Day clock | `time += 1/7200`, lit lamp posts only | same rate; starts **17:00** (`Story.docx`); drives ambient light and lamp posts | **IN** (rate, start: S) / **SHAPE** (light is presentation) |
| Pause | broadcast user events | the scheduler is not called; dialogue freezes from inside | **IN** — R covers frozen ticks |
| Terminal | backtick | `` ` `` + `app/terminal.ts`, a table of rows; cheats are sim commands | **SHAPE** |
| Title / load / controls | TODO | title, 3 slots, controls table | **SHAPE** |
| Debug | F2 / F3 / F5 / F6 | F2 overlay (fps, sim ms, draw ms, awake, chunks, paths, hash), F3 grid, F5 / F9 | **SHAPE** |
| Distance unload | `distance_unload`, 768 px | `sim/ring.ts`; sleepers still tick clocks | **IN** — D: bait test needs the lurker inside the ring |
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
| Resist per school (unit row × status rows) | **SHAPE** |
| Status list as `effects.json` rows read by every unit | **IN** — D (chilled, poisoned via play); catalog E |
| First-hit aggro; aggro scan every 10 ticks, staggered without 2020's dead-counter bug | **IN** — S: "enemies chase a player who is standing still" |
| Respawn timer (ticks while asleep too); `respawn: 0` stays dead | **SHAPE** |
| Player death: stand back up at the last arrival mark after 4 s; lock-ins reset | **IN** — D: "dying inside a lock-in re-opens the gate and re-arms the trap" |
| Dialogue hook: unit row `talk` | **IN** — S golden path |
| Patrol waypoints; home follows the patrol | **SHAPE** |
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
| Bolt fan / ring (`count`, `fan`), splash, carried light | **IN** — D: snake ring ≥ 15 bolts |
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
| Every quest can be given and handed in by some row | **IN** — E |
| Dialogue tree: first-match `start` rules, lines, ≤ 2 options, actions, goto | **IN** — S golden path |
| Hand-in rules sit **above** state rules in every tree (2026's dog hid two hand-ins behind a flag) | **SHAPE** — convention; the E quest test catches the worst case |
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
| Snake boss: the one custom mover. Turn clamp, 900 / 300 tick phases, reset on broken LOS, trail nodes, forwarding tail hitboxes | **IN** — D |

## 7. World verbs

| Verb | Bar |
| --- | --- |
| Zone travel by named mark; refuses while carrying | **IN** — S, D |
| Locked door + key tag; gates (solid while locked) | **IN** — D |
| Chest loot once; keeps what did not fit | **IN** — D |
| Push / pull: hold USE 30 ticks, 20 energy, one cell | **SHAPE** — code; the tests place the barrel rather than push it |
| Carry / put down in front, never on your own cell | **SHAPE** |
| Pressure plate held by a unit **or a pushable**; `release` re-locks | **IN** — D: mine |
| Repair: world-kind spell, prop `answers: "repair"`, consumes `needs`, refuses without | **IN** — D: mine |
| School touch: frost wakes `torch_blue` | **IN** — D: burial |
| Learn-spell prop (a dialogue tree with a `learn` action) | **IN** — S golden path |
| Lever / toggle (`use` list, `on` state) | **SHAPE** — no lever is placed yet |
| Trigger rects: `enter` / `while`, conditions, actions, `reset` on player death | **IN** — D: lock-in, arena, torches |
| Location | **IN** — S, D |
| Fog: one bit per 16 px block, interiors, in zone state | **SHAPE** |
| Tile edits as `tileDeltas` (`fill` action) | **SHAPE** — nothing uses it yet; it is what Grow and drain will be |
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
  → mine (dog teaches Repair): plate + barrel, plain keys, clerk (HM key), Headmaster (vault key),
       broken steps (Repair, 2 wood), ornate chest (boss key), vault (gold, Museum key), Iron Knuckles
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

## 11. File map (live)

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
| USE, push / pull / carry, Repair, school touch | `sim/interact.ts` |
| Verbs and conditions | `sim/actions.ts` |
| Bags, craft | `sim/inventory.ts` |
| Quests / dialogue / triggers + plates | `sim/quests.ts` `dialogue.ts` `triggers.ts` |
| Load ring / zones, travel, fog | `sim/ring.ts` `zones.ts` |
| Save codec, hash / replay | `sim/save.ts` `replay.ts` |
| Zones + solver | `world/county.ts` `interiors.ts` `mine.ts` `burial.ts` `kit.ts` `validate.ts` `index.ts` |
| Art | `art/units.ts` `props.ts` `icons.ts` |
| Renderer | `render/renderer.ts` `tiles.ts` `lighting.ts` `atlas.ts` |
| Input | `input/input.ts` |
| UI | `ui/host.ts` (the contract) + `ui/*` |
| Shell | `app/app.ts` `loop.ts` `storage.ts` `terminal.ts` |
| Tests | `jane/test/` |

2020 GML is **read-only** and stays out of the repo. Steal contracts. Do not port line-for-line.

## 12. Intentionally later, in order

**The order of work is now `PLAN.md` §8** (engine at scale → skeleton generator and seed viewer → the Lowfields → dungeon generator → omens and the text pass → the Waters → the Works). The list below is the engine-side backlog those milestones draw from.

1. **A browser smoke test** (boot, new game, walk, open bags) so presentation rows can start earning **IN**.
2. **Push / pull / carry under test**, then a lever placed somewhere real.
3. **Museum** — the next 2020 zone; key shuffle, light switches, a door Repair mends. Its key is already in the mine vault.
4. **Explosion** (mine reward in the 2020 map) and **Grow** (`fill` action + `answers: "grow"`), then **Butterfly Forest** as 2020 drew it: eight butterflies, no doors.
5. **Cart on a track** (`machine.kind = cart`), lily-pad rafts.
6. **Pipes → Factory** with the Electric spell; the orb waits in the cellar study.
7. The other three burial corners and the wizard; **School**.
8. Audio bus; touch layout; options; res sickness on death.

Phase numbers move up one row of the balance sheet per dungeon. Do not pull from this list until the rows above it stay green.
