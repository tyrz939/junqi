# Jane — Engine

How the live build in `jane/` works, and why each choice was made. Pair with `SYSTEMS.md` (the bar), `LEARNING.md` (what 2020 proved), `archive/phaser-remake-2026/POSTMORTEM.md` (what the last attempt got wrong).

Web. TypeScript, Vite, Vitest. **Zero runtime dependencies**: no Phaser, no framework. The production bundle is the game's own code.

```
jane/src/
  sim/      headless, deterministic. No DOM, no clock, no Math.random. ~25 files
  world/    zone builders + the lock-and-key validator. Pure functions of (zone, seed)
  data/     every content row, JSON
  art/      every sprite and icon, as palette-character grids in TypeScript
  render/   Canvas2D: atlas, chunked tiles, one multiply lightmap
  input/    one action layer: keyboard, mouse, gamepad
  ui/       DOM: HUD, windows, dialogue, menus, console
  app/      the shell: fixed-step loop, save slots, console commands, replay
jane/test/ the sim plays itself in Node
```

The dependency arrow only points one way: `app → {ui, render, input} → sim → data`. `world` depends on `sim/grid` and `sim/rng` and nothing else. Nothing in `sim/` or `world/` imports from anywhere else, which is why the whole game runs in a test.

---

## 1. Time

One tick is 1/60 s. **All sim time is integer ticks.** 2020 was a 60 fps frame-stepped GameMaker loop and every number in it was "per frame" (`gcd_timer*60`, `respawn_timer = 36000`, energy 0.5 per step). Those numbers port exactly and never drift. JSON rows speak seconds; `catalog.ts` converts to ticks once at boot.

`app/loop.ts` is a fixed-step accumulator. Real time fills the accumulator; the sim drains it in whole ticks; the renderer gets `alpha` (how far into the next tick) and interpolates positions between the last two ticks. A 144 Hz display and a throttled tab run the same game. Catch-up is capped at 5 ticks a frame, the rest are dropped and counted (F2 shows the count), so a stall cannot spiral.

The 2026 Phaser build stepped with a variable `dt`, tested only endpoints, and let a hitch frame carry a bolt through an 8 px wall.

## 2. Determinism

Same seed + same inputs ⇒ same state, bit for bit, tick for tick.

- **RNG:** `sim/rng.ts`, sfc32 with splitmix32 seeding. State is four numbers inside `GameState`, so it saves and resumes mid-sequence. Worldgen uses a separate stream per `(seed, zone, attempt)`, so adding an `rng` call in the mine cannot move a tree in the county. The sim never calls `Math.random`.
- **No runtime trigonometry.** `sin`/`cos`/`atan2` may differ in their last bit between JS engines, and one bit is a desync. Facing is a 4-way enum. Aim and headings are unit vectors. Turning is "rotate this vector by a whole-degree table entry" (`sim/angles.ts`); the table is built once and rounded to 1e-9. `sqrt` and the four operators are exact in IEEE 754. The snake's ±3°/tick steering and the 15-bolt ring both run on the table.
- **Input is quantised.** Move and aim vectors snap to 1/127 before the sim sees them.
- **Iteration order is array order.** No `Map`/`Set` iteration decides an outcome. The A\* heap breaks ties on cell index.
- **Proof:** `test/sim.test.ts` runs two sims on one input tape and compares `hashState` every 120 ticks; `test/replay.test.ts` records a session (input runs + tick-stamped commands, run-length encoded), re-simulates it from scratch and compares hashes. In the browser: console `replay verify`.

## 3. State

`sim/state.ts`. **The whole game is one JSON-able tree**: `GameState → zones[id] → { units, props, drops, projectiles, grounds, triggers, tileDeltas, fog }`. No classes, no `Map`s, no object references (ids only), no functions.

What that buys:

| | |
| --- | --- |
| Save | `JSON.stringify(state)`. There is no second schema to forget a field in. 2020 wrote `maxhp` where it meant `hp` and reloaded units at `xstart`; the 2026 build forgot cooldowns, statuses, Grow edits and ground loot. None of those bugs has anywhere to live |
| Load | adopt the tree, rebuild the derived runtime |
| Versioning | `SaveFile.version` + a `MIGRATIONS` table (`sim/save.ts`). Corrupt, foreign and newer saves decode to an error value, never a throw |
| Hash | `hashState` = FNV-1a of a key-sorted stringify. F2 shows it |

**Players are rows too.** `state.players[]` holds up to four `PlayerState`s (`MAX_PLAYERS`): seat, `who` (a client token, never shown), body id, zone, bar, craft row, open conversation, respawn clock, stats, pending travel, `connected`, and `parked` (her body, kept while she is away; the same token gets it back). **The line is: bags and bar are hers, everything else is the world's.** On the state itself: `name` (one heroine, the host's choice, shared by all), `quests`, `flags`, `rest` (the last bed or fire anyone used; where everyone wakes and where everyone arrives), `growth` (what has been learned; found upgrades later) and `open` (whether anyone else may sit down; a save loads closed). Single-player is a party of one, not a separate path. Save v4: `MIGRATIONS[2]` moved the old single-player fields into `players[0]`, `MIGRATIONS[3]` moved name, rest and learning from the person to the world.

**Derived data is never saved.** `sim/runtime.ts` holds, for each live zone: the `Grid` (tiles from seed + `tileDeltas`, flags, occupancy), the `PathFinder` scratch, id and key lookup maps. It is rebuilt on zone entry and on load, and throwing it away must never change behaviour. Terrain is not saved at all: a zone is its seed plus the list of cells that changed (`tileDeltas`).

Saving reads the tree as it stands. 2020 had to `instance_activate_all` first because deactivated instances were invisible to `with`; here a sleeping unit is an ordinary row. A save cannot perturb a replay.

## 4. The tick

2020's `game` object was already a scheduler firing user events in a fixed order. `Sim.tick` is that, written down (`sim/sim.ts`):

```
 1 clock          day time advances; clock rows fire (once, for the whole party)
   then, for every zone with a connected player in it, in fixed order:
 2 ring           sleep / wake, only when a player crosses a 16 px block; the ring is the union of theirs
 3 timers         GCD, cooldowns, stop, anim, MP regen; respawn clocks of sleepers too
 4 players        each seat's input: movement, sprint / carry energy, hold-to-push
 5 controllers    AI think + move; the snake's own mover
 6 projectiles    bolts fly; ground effects pulse
 7 statuses       DoT / HoT pulses queue hits
 8 flush          the ONLY place hp changes. deaths, loot, quest hooks
 9 triggers       enter / while rects; pressure plates
10 housekeeping   drop ageing, prop flags, fog
11 travel         each player's pending zone change happens last, never mid-iteration
```

**A `World` is one zone plus, optionally, the player being acted for.** The scheduler keeps a `ZoneCtx` per live zone and drops it when the last player leaves (the zone's state stays in the tree). Systems never ask "where is the player"; they are handed a `World`. Anything done on someone's behalf (her command, the trigger she tripped, the kill she landed) runs inside `asPlayer(w, p, fn)`, and `w.party` answers the party-wide questions (`size`, `ofUnit`, `units`, `everyoneResting`, `each`). Plates and clock rows run with no actor at all, and the type system makes that case explicit (`w.actor` is nullable; `playerOf(w)` throws if code that needs one is called without).

`Sim.tick(inputs[])` takes one input frame per seat; `Sim.command(seat, c)` one command. **Sitting down and getting up are commands** (`join`, `leave`), so a session where people come and go is still one deterministic stream. Replay format v3 records exactly that (plus the heroine's name in the header, since the name is part of the world) (frames carry every seat; commands carry their seat), and it is the future wire format: lockstep is this stream, sent over a socket.

The shell never holds the whole sim. `sim.view(seat)` is a `PlayerView`: her body, her zone, her runtime, `frozen` from her side. UI and renderer are written against it, so seat 1 alone and seat 4 of four are the same code. Events carry routing (`to` a seat, `inZone`); `Sim.eventsFor(events, me)` is the filter, so her toasts, loot and screen shake are hers, a zone's sounds belong to whoever is in it, and quest news reaches everyone.

**Leaving hands on what the story needs.** `catalog.storyItems` is derived at boot (every item with `opens`, every `acquire` target). When a seat stands up those stacks go to a staying player, host first, overflow at her feet; story items on the ground never age out. **Learning is taught to the table** (`teach` in `actions.ts`): it goes into `state.growth`, into every body connected or parked, and onto the first free bar slot of every seat; `join` catches a newcomer up.

**Friendly spells are mouse-over.** `bar` and `cast` commands may carry `on`: the unit under the cursor when the button went down (0 for nobody; absent when there is no cursor, i.e. a pad). The hover is presentation, so the client resolves it (`Renderer.hoveredFriend`); it travels in the command, so it is recorded and replays; the sim trusts none of it (`chooseFriend` in `combat.ts`: a living party member, in this zone, in range, in sight, or the cast fails with the ordinary error). Without `on` the friend nearest the aim line is used.

**Numbers are per seat.** `damage` and `heal` events carry `from`; the renderer floats a number only when this seat's unit dealt or took it. Sparks and enemy bars are for everyone.

**The party penalty** lives in one place, the flush (step 8): a player's dealt damage × `PARTY_DEALT[n]`, damage she takes × `PARTY_TAKEN[n]`, where n is how many are *connected*, not how many are near. Healing is untouched. The world is never rebalanced (`PLATFORM.md` §2).

Discrete actions are `Command`s (`use`, `bar`, `bagMove`, `craftTake`, `choose`, `dev`, …) applied between ticks. They also work while the world is frozen (bag open, dialogue), which keeps inventory and dialogue input inside the deterministic record. The console's cheats are commands too, so a session that used `god` still replays.

**Pause is not a broadcast any more.** Nothing is told to pause; the scheduler simply is not called. Dialogue freezes the sim from inside (`sim.frozen`); windows and menus are the app's decision. **With more than one player connected nothing freezes at all**: she stands and reads, her bag is open, and the county carries on around her.

## 5. Space

- **Cell = 8 px, everywhere.** Path, collision, occupancy, props, triggers, fog blocks (2 cells), ring blocks (2 cells). 2020's `PathTo` fallback used 32.
- **Metre = 8 px.** Combat speaks metres, movement speaks pixels. Range is measured **between bounds** (`GetDistanceBetweenBounds`): centre distance minus both bodies, so range 0 means touching, which is how every 2020 melee row is written. `bounds` is a unit row field.
- **`Grid`** (`sim/grid.ts`): two typed arrays and a sparse map. `tiles` u8; `flags` u8 = tile flags | prop flags | an occupied bit; occupancy itself is a `Map` of a few hundred entries, read only when the bit is set, so a seven-million-cell county pays nothing for it (an `Int32Array` would be 29 MB). Flags: `SOLID`, `BLOCK_LOS`, `WATER`, `PROP_SOLID`, `PROP_LOS`, `INDOOR`, `OCC`. **Water blocks feet, not sight**: 2020 set `block_los` on `obj_water`, but `CheckLOS` and projectiles only ever tested `obj_static_solid_parent`, so bolts crossed ponds. The shipped behaviour wins over the dead field.
- **Movement:** a 6×6 px box on the feet, axis-separated sliding against solid cells, flush to the border when blocked. Units never hard-block each other; they shape *paths* through occupancy instead, so nothing wedges in a corridor. Every awake living unit occupies its cell (2020 only marked units in combat).
- **A\*** (`sim/path.ts`): 8-way, no corner cutting, integer costs 10/14, octile heuristic. Allocation-free after construction: `g`, `from`, `stamp`, `closed` and a binary heap are typed arrays, "cleared" by bumping a generation counter. **Windowed**: the scratch covers a fixed 256 × 256 cells centred on the start, so it is ~3 MB whether the zone is a kitchen or the ten-minute county in `PLAN.md` (grid-sized scratch would be 115 MB there). A goal inside the window that cannot be reached is `null`; a goal beyond the window returns the best partial path toward it and the walker re-plans on arrival. Every search has a **node budget** (6000) and a max cost; failure means leash, as 2020's "path longer than max ⇒ fail" did. The **goal cell is always enterable** for the search, because it is normally someone's feet. At most 4 searches run per tick; the rest wait a tick.
  The 2026 build returned `null` when the goal was occupied (so nothing could chase a standing player), sorted an array on every pop, keyed nodes by string, and had no bound: an unreachable goal on a 40k-cell map cost 1.1 s.
- **LOS** (`sim/los.ts`): integer DDA supercover, every cell the segment touches, so nothing slips between diagonal walls. Sight and projectiles share it.
- **Load ring** (`sim/ring.ts`): the whole zone exists in state; a 768 px square around the player thinks. Re-evaluated on block change only. Idle AI and props outside sleep (no think, no occupancy, no draw); anything in combat stays up. Sleepers' cooldown and respawn clocks still tick: the 2026 build froze them, so things only respawned while you stood on the corpse.

## 6. Units and combat

One `Unit` shape. `controller` is a field: `player | ai | npc | snake`.

**The cast pipeline** (`sim/combat.ts`), same function for Jane and a rat:

1. validate, in 2020's order: dead → stunned → target → enemy → LOS → **range → cooldown** → GCD → MP → energy. Range before cooldown matters: an AI told "onCooldown" stands still, an AI told "tooFar" keeps walking. The 2026 build had them reversed and melee mobs froze for 2 s after every swing.
2. spawn a **kind**, never a class: `melee | bolt | self | world | ground`
3. pay costs only if the kind says the cast was valid (a Repair with nothing to repair costs nothing)
4. damage goes to the victim's `incoming` queue
5. `flushIncoming` applies it later in the tick: resist (unit row × status rows), manashield, clamp, lifesteal back to the source, mana-on-hit, first-hit aggro, on-hit status, death

The target is resolved to a live unit or `null` before anything is read from it (2020 read `.faction` before `instance_exists`).

**Aim, not click-target.** Jane's bolts fly along the aim vector (cursor or right stick). Melee is forgiving on purpose: it takes the nearest enemy in reach, preferring the half-plane she faces, and turns her toward what it hit (a browser run with the cursor parked the wrong way whiffed thirty swings before this rule). AI units have no aim and cast at `target`. This is what 2020's last build did (every assignment to the player's `current_target` is commented out; the pad layout says "Right stick: Aim"). Bolts do not home, so a 2 px/tick poison bolt can be walked around. Bolt rows can fan (`count`, `fan`; 360 = ring), splash (`splash.radius/div`, 2020's dmg/5 in 25 px) and carry a light.

**Statuses** are `effects.json` rows read by every unit: `speed`, `stun`, `pulse`, `resist`, `manaShield`, `lifesteal`, `critOneIn`, `onMelee`, `manaOnHit`, instant `heal`/`mana`. 2020 had one `speed_multiplier` that only the player's movement read, so AI could not be slowed. The 2026 build keyed behaviour by effect **id** in TypeScript, so "a new status is a row" was false.

**AI** (`sim/ai.ts`): idle (regen max/300 per tick, patrol, aggro scan every 10 ticks, LOS required) → combat (first affordable spell in the book; the book's order is the priority; path in on `tooFar`/`notInLOS`) → leash. The scan is staggered with `(tick + offset) % 10`; 2020 drew `irandom(9)` once and compared a decrementing counter to 0, so a unit that drew 0 never scanned again.
`bait` is a unit row field: an idle unit walks to a drop of that item and dies on it. That is 2020's unkillable burial snake and its poisoned rat meat, as data.

**The snake boss** (`sim/snake.ts`) is the one custom mover: heading turn-limited to speed×4 degrees per tick, 900 ticks of pursuit, then home and 300 ticks of 15-bolt rings, full reset if a wall cuts the line to the player, a trail node every fourth moving tick (64 max), and every eighth node is a hitbox that forwards damage to the head. Timer phases only; 2020 had no HP thresholds either. Everything else about it is an ordinary unit.

## 7. World verbs

One verb list (`Action` union in `state.ts`, one exhaustive `switch` in `actions.ts`) speaks for dialogue options, prop use, triggers, quest rewards, item use, unit death and the console. Rows are structured objects (`{ "do": "unlock", "prop": "gate_boss" }`), checked at boot, and an unhandled verb is a **compile error**. The 2026 build used `"learn:foo"` strings, parsed at run time, silently ignoring what it did not know.

**Props are capability rows, not classes.** A door is `solid + locked + to`. A chest is `loot`. A gate is `gate` (solid while locked). A crate is `push`. A rock is `carry`. A torch is `light`; a cold torch is `answers: "frost" + lightWhenOn`. Broken steps are `answers: "repair"` with `needs: [wood ×2]`. A pressure plate is `plate` with `use` and `release` lists, pressed by any unit **or pushable** standing on it (2020: "any obj_in_world_parent presses a button, so barrels work").

- **USE** is one entry point (2020's B button: Use / Talk / Push / Pull). Tap: drop → talk → unlock / enter → loot → carry → `use[]`. Hold against a pushable while walking: push one cell after 30 ticks, 20 energy; walk away while holding: pull, stepping back first.
- **Keys** are an `opens` tag on the item and a `keyTag` on the prop. One use path. Consumed, as in 2020, unless `bound`.
- **Triggers** (`triggers.json`): a named rect, `enter` or `while` mode, conditions, an action list, and an optional **`reset`** list. When the player dies after a trigger fired, its reset runs and it re-arms, so a lock-in can never leave a respawned player outside a shut gate.
- **Quests:** `kill`, `acquire`, `location`, nothing else. `acquire` reads the bag live. `location` reads a persistent "been here" flag, so a place visited before the quest was taken still counts (the 2026 build lost those forever). Rewards are action lists; a reward that does not fit the bag lands at your feet.
- **Drops, chests, fog, tile edits** all live in `ZoneState`, so they survive travel and saves. A chest keeps what did not fit.

## 8. Worldgen

`world/`. A builder is a pure function `(seed, attempt) → Blueprint` = tiles + unit/prop spawn rows + **named** marks and rects. Story, triggers, dialogue and travel refer to names (`house_door`, `stoop`, `lockin_a`), never coordinates. That is the whole contract that lets geometry roll while the story holds.

`world/validate.ts` is a **lock-and-key solver**, the machine version of 2020's Dungeon Graph kit: flood from the entrance, collect keys from reachable chests and guaranteed drops, open what they fit, fire reachable levers, plates, repairs (if the materials are obtainable) and kill-triggers, repeat to a fixed point. A zone passes when its `ZoneContract` exists, nothing spawns in a wall, and every required thing is reached and every gate can open. A failed candidate is **re-rolled with the next attempt number**, never thrown at the player. `test/world.test.ts` holds five zones × 25 seeds to that, plus "Jane arrives on open ground, in sight of the dog, not already standing on the quest trigger".

The county is 640 × 384 cells = 5120 × 3072 px, 2020's `room_zone1`. The 2026 build made it 2000 × 1200 and then had to fill it.

## 9. Rendering

Canvas2D into a **low-resolution framebuffer**, upscaled by an integer factor with `image-rendering: pixelated`. The view is ~216 px tall (2020's camera was 256) and as wide as the window's aspect makes it, as 2020's was. At 1080p that is 5×. The UI is DOM at native resolution, so "zoomed world, crisp UI" needs no second camera.

- **Art is source.** `art/*.ts` are palette-character grids; `render/atlas.ts` rasterises them once, plus a mirrored copy so west is not a transform. No image, font or audio file ships (see `DESIGN-2020.md` §6 for why nothing from 2020 can).
- **Tiles** (`render/tiles.ts`): painted procedurally per cell (a stable hash picks the variant; neighbours decide wall faces and shorelines) into 16×16-cell chunk canvases, built on demand, LRU-evicted, invalidated per rect on a `tiles` event. About a dozen chunks are visible at once.
- **One lighting pass, one cached surface** (`render/lighting.ts`). 2020 allocated a view-sized surface every frame, cleared it to a fixed yellow, `bm_subtract`ed each light out of it, `bm_subtract`ed the result off the scene and freed it; the clock never touched the colour. Here: clear the cached lightmap to the ambient colour (the clock outdoors, the zone's ambient indoors), **add** each light as a cached radial sprite, **multiply** the lightmap over the scene. Multiply scales colour instead of clipping it, coloured lights tint, and ambient = white is an exact no-op, so daytime costs nothing. Lights are prop / unit / spell row data. Lamp posts burn 18:30–06:30, the one thing 2020's clock did drive.
- **Order:** chunks → flat props → grounds → drops → y-sorted props and units → projectiles → particles → **light** → fog → unlit overlays (health bars, floating numbers, reticle) → F3 debug.
- **Camera:** eased follow, clamped to the zone; `camera` action locks it to a named rect for boss rooms (2020 declared `CAM_STATE.LOCK` and never set it); additive decaying shake from a `shake` event.
- The renderer reads state and events. It never writes to the sim.

## 10. Input

`input/input.ts`. Devices fill two outputs and nothing downstream knows which device it was:

- **held** → `InputFrame` (move vector, sprint, use-held, aim), sampled once per display frame
- **edges** → `GameAction | UiAction`, queued on press

Keyboard + mouse and the 2020 pad layout (left stick / D-pad move, right stick aim, A X Y LB RB = bar 1–5, B use, RT sprint, View bags, Menu pause) both work. Holding the right mouse button walks toward the cursor with strength distance/32, 2020's virtual stick. 2020 had this shape and then triplicated every window's logic per device with the touch copy left as `TODO`; here there is one UI path and Pointer Events cover touch.

## 11. UI

DOM, behind one interface (`ui/host.ts`). It reads state, sends commands, asks the host for storage / icons / the console. It builds once and patches what changed. Text is crisp at any scale; pointer events reach widgets before the world for free, which is all 2020's `mouseOnGui` was; drag and drop is one Pointer Events path.

## 12. Debugging

`` ` `` console (`app/terminal.ts`, a table of rows: `help give god tp time hp mp learn quest flag kill spawn save load seed hash pos inst speed replay ver title party open close join leave`). `open` then `join` sits an idle second body down at the party's fire, which is how to feel the party penalty before there is a network. **F2** overlay: fps, sim ms, draw ms, awake/total units, live/built chunks, path searches and average nodes, dropped ticks, state hash. **F3**: solid cells, occupancy, unit paths, trigger rects, the load ring. `speed 4` is 2020's hold-F2 fast-forward.

## 13. Tests

`npm test`, headless, ~3 s. A bot (`test/bot.ts`) that can only hold a direction, press USE, press a bar slot and aim:

- plays **the first five minutes**: letter → dog → skeleton → key → kitchen → Icebolt → back out
- runs the **cellar, mine and burial chains**: both iron doors on one key type, the plate and the barrel, clerk → Headmaster → Repair (and its refusal without wood) → boss key → vault → the arena gate dropping and lifting; cold torches lit by Icebolt, the lock-in and its chest, the snake's 900-tick clock, its ring, a tail hit hurting the head, bait killing a snake that cannot be fought, a death inside a lock-in re-arming it
- **co-op** (`test/coop.test.ts`): four seats and a refused fifth, a closed world refusing everyone, leave and rejoin by client token, one name, one coat per seat, arriving and waking at the shared fire, a guest's rest reaching the host, shared learning (the away and the late included), keys handed on and never ageing out, heals by mouse-over and by stick (her, yourself, out of reach, another zone), the penalty by head count wherever they stand, a shared kill paying everyone, nothing pausing with company, event routing, two zones ticking at once and an empty one stopping, a lock-in holding while a friend lives, the night waiting for everyone, a two-seat replay
- beds and fires, the name, save migration v1 → v3, the dog's poke chain and its absence after dark (`test/rest.test.ts`)
- determinism, save → load → continue, replay, A\* bounds, LOS, catalog integrity ("every quest can be given and handed in by some row"), art integrity, five zones × 25 seeds through the solver

The 2026 audit caught its two worst bugs in seconds with a 150-line headless harness the project never had. This is that harness, kept.

## 14. Not built

Honest list. Each is a row or a module, not a rewrite: cart on a track (`machine.kind = cart`), lily-pad rafts, Explosion / Grow / Electric spells, the Museum and every zone after it, audio bus, touch layout, options menu, res sickness, the other three burial corners and the wizard. `SYSTEMS.md` §12 has the order.
