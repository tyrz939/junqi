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
| Load | `JSON.parse`, adopt the tree, rebuild the derived runtime |
| Versioning | `SaveFile.version` + a `MIGRATIONS` table (`sim/save.ts`). Corrupt, foreign and newer saves decode to an error value, never a throw |
| Hash | `hashState` = FNV-1a of a key-sorted stringify. F2 shows it |

**Where a save goes.** Three slots. A county-sized state is several MB of JSON and `localStorage` gives an origin about five, so a slot is the same `SaveFile` text, gzipped with the platform's `CompressionStream` (`app/gzip.ts`, pure, tested in Node) into IndexedDB: database `jane`, store `saves`, key = slot, value = `{ meta, data }`. `meta` is the slot's summary (when, where, day, hour, hp, save version, bytes) kept beside the bytes, so listing slots never decompresses one. The rules live in `app/slots.ts` over two small interfaces (an async database, a sync text store), which is what lets `test/storage.test.ts` prove them with in-memory stand-ins; `app/storage.ts` is only the browser's end.

- **The menus stay synchronous.** The three summaries live in memory. `main.ts` awaits `initStorage()` before the app exists (three small reads, bounded by a timeout, never rejects), and every write and clear keeps them true.
- **Saving never costs a frame twice.** The state is stringified on the call, so the save is the tick it was asked for on; compressing and writing happen afterwards. "Saved" is toasted when the IndexedDB transaction has committed, not when it was asked for. Everything that touches one slot runs in that slot's queue: two saves land in order and the second is what remains, and an F9 straight after an F5 reads that F5.
- **Loading is asynchronous.** `App.load` holds the world still from the request until the new sim is swapped in whole, between frames. New Game or Quit to Title during the read wins; a double click is one request.
- **Fallbacks, all silent.** No IndexedDB, or it throws (private windows, blocked site data): the same text goes to `localStorage` uncompressed, as before. No `CompressionStream`: IndexedDB holds the plain text. An unreadable slot is an empty slot, never a throw on the title screen.
- **Migration.** On boot, any slot still in `localStorage` (`jane.save.N`, or `junqi.save.N` from before the rename) that decodes and is not older than the database's copy is listed at once and moved into IndexedDB through the same queue, then removed. Text that does not decode is left alone.

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
- **`Grid`** (`sim/grid.ts`): two typed arrays and a sparse map. `tiles` u8; `flags` u8 = tile flags | prop flags | an occupied bit; occupancy itself is a `Map` of a few hundred entries, read only when the bit is set, so a seven-million-cell county pays nothing for it (an `Int32Array` would be 29 MB). Flags: `SOLID`, `BLOCK_LOS`, `WATER`, `PROP_SOLID`, `PROP_LOS`, `INDOOR`, `OCC`, `NOPUSH` (a sill: feet cross it, a pushed prop does not; §8.3). **Water blocks feet, not sight**: 2020 set `block_los` on `obj_water`, but `CheckLOS` and projectiles only ever tested `obj_static_solid_parent`, so bolts crossed ponds. The shipped behaviour wins over the dead field.
- **Movement:** a 6×6 px box on the feet, axis-separated sliding against solid cells, flush to the border when blocked. Units never hard-block each other; they shape *paths* through occupancy instead, so nothing wedges in a corridor. Every awake living unit occupies its cell (2020 only marked units in combat).
- **A\*** (`sim/path.ts`): 8-way, no corner cutting, integer costs 10/14, octile heuristic. Allocation-free after construction: `g`, `from`, `stamp`, `closed` and a binary heap are typed arrays, "cleared" by bumping a generation counter. **Windowed**: the scratch covers a fixed 256 × 256 cells centred on the start, so it is ~3 MB whether the zone is a kitchen or the ten-minute county in `PLAN.md` (grid-sized scratch would be 115 MB there). A goal inside the window that cannot be reached is `null`; a goal beyond the window returns the best partial path toward it and the walker re-plans on arrival. Every search has a **node budget** (6000) and a max cost; failure means leash, as 2020's "path longer than max ⇒ fail" did. The **goal cell is always enterable** for the search, because it is normally someone's feet. At most 4 searches run per tick; the rest wait a tick.
  The 2026 build returned `null` when the goal was occupied (so nothing could chase a standing player), sorted an array on every pop, keyed nodes by string, and had no bound: an unreachable goal on a 40k-cell map cost 1.1 s.
- **LOS** (`sim/los.ts`): integer DDA supercover, every cell the segment touches, so nothing slips between diagonal walls. Sight and projectiles share it.
- **Load ring** (`sim/ring.ts`): the whole zone exists in state; a 768 px square around the player thinks. Re-evaluated on block change only. Idle AI and props outside sleep (no think, no occupancy, no draw); anything in combat stays up. Sleepers' cooldown and respawn clocks still tick: the 2026 build froze them, so things only respawned while you stood on the corpse.
- **Props by block** (`sim/runtime.ts`): the runtime buckets props by 16 × 16-cell block (128 px), keyed `by * blocksWide + bx`. A prop sits in the one bucket of its origin cell; a query reaches back by the largest footprint in `props.json` (5 × 3, the car wreck), so a prop is found from any cell it covers and nothing is listed twice. Each bucket is in ascending prop id and a merged answer is sorted by id. `zone.props` is itself in id order (props are numbered as the zone is made, `addProp` appends, nothing removes), so first-match and tie-break results are exactly those of the old pass over the whole zone; a run with the buckets replaced by a linear scan hashes the same, tick for tick. `propsNear(rt, x, y, radius)` and `propsInCells(rt, cx0, cy0, cx1, cy1)` return whole blocks, a superset, and callers still test exactly. `moveProp` is the only way a prop changes cell (push, pull, put down, dropped where she fell) and re-buckets it; a carried prop keeps the cell it was lifted from until then. Who asks: USE focus, rest and bench reach, push, Repair, frost-lit torches, plates (kept as their own short list), the ring's prop flags (it clears the last awake set and wakes what the buckets hold round each player), and the renderer, which visits only the blocks under the view plus 104 px for light pools. Solid props are stamped onto the grid per footprint (`touchProp`, flushed at housekeeping); the whole-grid re-stamp is for zone entry and load. Save, zone build, the map and the validator still read `zone.props` whole. `test/budget.test.ts` holds the tick to a 2 ms median with 3,000 units and 8,000 props asleep across the county (measured 0.16 ms; what is left is the per-unit timer, status and flush passes, about 50 ns a sleeper).

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

The county is 640 × 384 cells = 5120 × 3072 px, 2020's `room_zone1`. The 2026 build made it 2000 × 1200 and then had to fill it. The next county is 3600 × 2000 (`PLAN.md`), and it is decided before it is drawn:

### 8.1 The skeleton (`world/skeleton/`)

`buildSkeleton(seed)` decides a whole county on a **macro grid of 225 × 125 cells** (one macro cell = 16 cells = 16 m) in a fraction of a second, as plain typed arrays and lists. Pure, seeded, no trigonometry, no DOM. The game is not built from it yet; the seed viewer is.

| Layer | How |
| --- | --- |
| **Land** (`terrain.ts`) | Hashed value noise in octaves. Fixed where the story needs it: the Works are the north with one hill (the School's crown), the river runs north to south east of the middle with the Waters beyond it, the Lowfields are the south-west with foothills, one lake in the east. Free everywhere else: where the river bends, how the borders wobble. Also wetness (distance to water), slope and `rough` (the cost of laying road), each worked out once |
| **Sites + roads, together** (`index.ts`, `place.ts`, `roads.ts`) | Row by row from `sites.json`: list every macro cell the row allows (region, terrain kind, height cap, river or lake nearby, across the river from X, crow's-distance windows to what is already placed), pick one with the seed, lay its road from the site it hangs off (`roadFrom`), then **measure the row's road rules on the network as it now is**. A spot that fails is dropped and another tried (14 tries), so one awkward site costs retries, not the county. Roads are A\* over `rough` + slope, water dear (a bridge) and lake impassable, existing road nearly free, which is what makes a network with one to three bridges instead of a bundle of lines. Searches run in a corridor around their two ends first |
| **Lamps** | A road attribute. The first walk (station, Julie's, town) is lit end to end; elsewhere lamps come in runs of ~100 m and fail with distance from the town, faster east of the river, almost entirely in the Works |
| **Named patches** (`areas.json`) | About six per region, each with its own threat, placed like sites. Threat 2 and up never touches the first walk; threat 4 and up keeps 260 m from any haven. A row that does not fit a seed is skipped: patches are texture, sites are story |
| **Threat** | 0..6 per macro cell, in `PLAN.md` 2.6's order: region base, patches, +1 in a 170 m ring round every dungeon mouth, −1 on a road, 0 inside a hub's fence. Night is a rule on top (`threatAt`): +1 outside lamplight, +2 in the Works, and an unlit road loses its discount |
| **Small places** (`pois.json`) | The roads are walked first and something is set beside them every 170 to 250 m, because the density rule is about what you *see* from the road; the rest of each region's budget of 30 goes to banks and deep country |
| **Checked** | Every distance rule (by road, measured), every road site reachable from the platform, the first walk never above threat 1 by day, a bed or a fire in every region inside its own threat, 22 to 30 small places per region, no stretch of road over 900 m with nothing to see, one to three river crossings. A failing county is thrown away and the next attempt tried (40); on 1,000 seeds the mean is 2.5 attempts and the worst 13, with 10 to 18 km of road a county and 17 of the 18 patches placed on average |

### 8.2 The county (`world/county.ts`, `world/chunks.ts`)

The playable county is the skeleton turned into cells: 3600 × 2000 of them, flat typed arrays (7 MB of tiles, 7 MB of flags), built in about a third of a second and validated in about as long again.

1. **Land and water.** Every cell looks its biome up in the skeleton through a smooth warp, so the 16 m macro squares stop being squares; water is read as a smooth field, so the river has banks and sand instead of steps. Ground is a table per biome (hedged fields, thickets, outcrops, reed pools, slag) driven by one smooth value and one per-cell hash. Painted block by block: inside a macro block the smooth fields are straight lines along a row, so three bilinear samples a cell become three additions.
2. **Roads** are the skeleton's routes as four-cell strokes. A road over water is a bridge. The Burial Chamber gets a footpath from the graveyard and nothing else.
3. **Set chunks** (`chunks.ts`): the authored places, each a function of an origin that clears its box, draws itself, and returns its **gates**: walkable cells on the outside of the box that its own paths lead in from. Castle Halt, Julie's yard, the town, the farm, the mine mouth, the graveyard, the burial stair, the car in the wood, two outposts. The places the game has not built yet (Museum, library, Butterfly Forest, the statue, Factory, School) get their ground, their mass against the sky and a mark, and **no door**: a locked door with nothing behind it would be a lie.
4. **Links.** Where a road meets a chunk's box the stamp has overwritten it, so the last cell outside is joined to the chunk's nearest gate by a lane that goes *round* the box. No road can ever cut a fence or a kitchen, whatever direction the seed sends it from.
5. **Dressing.** Lamp posts stand only where the skeleton lit the road. Small places are dressed from the art that exists and leave a mark (`poi_<n>`) for the side quests and omens to be written onto. Herbs by region, rocks.
6. **Wildlife** by region, biome and threat, from the unit rows that exist. A spawn carries `phase` (the threat of the ground it stands on) and the zone scales the row by it (`PHASE_SCALE`): one `skeleton` row, not `skeleton_2`. Nothing spawns in a haven or within 48 m of the first walk.
7. **Trimmed, then judged.** The builder floods from the platform and forgets any small place or creature the flood never reached (an island of cliff is not a place). Story marks are *not* trimmed: the lock-and-key validator judges those, and a county that fails takes the seed's next valid skeleton.

**Content says where by name** (`world/placements.ts`, rows in `data/placements/*.json`). Every seed puts the Top Field, the square and the farm somewhere else, so a quest cannot hold a coordinate. A row names a mark, a site, a named patch, or a KIND of small place nearest a site (`{ poi: "scarecrow", near: "farm" }`); the builder works out the cells, in three stages because it claims ground as it goes (inside chunks, at small places, in areas). If a seed rolled no scarecrow, the nearest small place to the farm becomes one, so the quest exists on every seed. Everything a row names joins the county's contract: a seed that cannot place it is re-rolled, never quietly skipped.

**Content and zones are additive.** A table's rows are its base file plus any fragments beside it (`data/quests.json` + `data/quests/*.json`), merged in path order, a duplicate id being a boot error; art sheets merge `art/units/*.ts` and so on the same way. A zone is a file in `world/zones/` that exports a `ZoneDef` (id, builder, contract, given keys); `ZoneId` is an open string and `ZONE_IDS` (the fixed tick order) is the original five followed by the registered ones by file name. A test walks every door in every zone and checks its destination exists, which is what the closed union used to guarantee.

**A lesson worth keeping:** the first version took 1.5 to 3.5 s and got slower with each build. Nothing was wrong with the algorithm. The inner loops read imported names (`Tile.Water`, `Biome.Field`, `MACRO`) dozens of times per cell, and a module binding is a lookup every time (the test runner makes each one a getter). Copying them into locals made the painter five to twelve times faster, and the two flood fills likewise. Anything that runs per cell of the county reads locals only.

The validator re-floods only when something that blocks has changed (a gate opened, a prop hidden), not every time a herb is picked: a flood is seven million cells.

**The seed viewer** (`viewer.html`, a second Vite page, in the build) draws 24 counties at a time straight off the same `Skeleton`: land, regions or threat, day or night, walking times from the platform, and for one county every place in the order you reach it, every rule with its measured value, every road and patch. It exists because a generator is reviewed by looking at two dozen maps, not by playing two dozen games.

### 8.3 Dungeons (`world/dungeon/`)

**Authored mission, generated space, proven result.** `DUNGEONS.md` is the design; this is what is built. The Gold Mine is the first dungeon made this way: `world/mine.ts` is three lines, and `buildZone("mine", seed)` still returns an ordinary `Blueprint`, so the sim, the save format and the renderer do not know a dungeon was generated.

| Thing | Where | |
| --- | --- | --- |
| The mission | `data/dungeons/<id>.json` (`DungeonDef`, `world/dungeon/types.ts`) | Nodes in a fixed `order` (entrance, teach, key, fight, hub, rest, miniboss, bosskey, reward, boss, side), each naming a template **pool**, what its sockets **hold**, the contract names it **binds**, its `heat`, and any trigger rows of its own. Edges are `open`, `key` (a gate), `verb` (a prop that fills the corridor: the broken steps), `oneway opens_on <flag>` (a gate a flag unlocks), `sight`, and `lockin` as an `also` on another kind. Not a catalog table: the sim never reads a mission |
| Room templates | `world/dungeon/rooms/<dungeon>/*.room`, loaded raw with `import.meta.glob` (`pools.ts`) | A text grid like the art, a header above, a legend below (`room.ts` has the format). `#` wall, `.` floor, `_` sill; doors are rim characters (`n1`, `e1`; `n2` on a room of two bays); sockets are runs of a legend character (`plate:main 2x2`, `spawn`, `dress:torch`); marks; named rects. A template declares `grants` and `blocks` and is held to them |
| The lattice | `layout.ts` | Bays of 36 x 28 cells. A room fills 1 x 1 or 2 x 1 bays and its rim stays 3 cells inside them; a door is 3 cells on the middle of a bay side. **Corridors run on the lines between bays**, 3 wide, so none can break into a room. The lines are a small graph of corners and side-middles (`Lanes`); two facing doors share a middle (a straight stub), anything else is routed middle to corner to corner to middle. **A lane node belongs to one corridor**, so corridors never merge or cross, which is what keeps a lock a lock |

**The steps** (`generate.ts`, `layout.ts`), every choice from the Kit's stream for `(seed, zone, attempt)`, lists in a fixed order before any pick:

1. **choose** a template for each node from its pool, seeded, no template twice in one dungeon. One with nowhere to go at all is re-picked, up to three times.
2. **embed**, depth first. The entrance goes on an edge of the lattice. Then critical nodes in mission order, each once a neighbour of it stands (a room of more than one bay goes down as soon as it can: the big rooms are the hard ones to fit). Candidates are every free bay times every transform that offers different doors; each is joined to every placed neighbour at once, and scored: straight stubs over routed lanes, room left for the doors still owed, the boss far from the entrance, the rest room near the hub, a cycle's open end near its other end. The best three are tried in a seeded order, then the next three; a node with nowhere to go sends the search back. Four embeddings of 160 placements each are tried per attempt; they cost no cells. Side rooms go last, up to the budget, and one that does not fit is left out.
3. **route**: done during embedding, so a layout that exists is a layout whose corridors exist.
4. **lock**: a `key` or flag gate is `gate_h` / `gate_v` in the one corridor cell just outside the far room's door. A `verb` prop stands in the middle of the corridor; a 3 x 2 prop in a corridor that runs east to west gets a neck of wall exactly as long as it is. `lockin` is the macro below.
5. **fill**: holdings into sockets (a holding's prop must have its socket's footprint), bound units, then the **enemy mix**: `round(heat x baseHeat)` capped by the template's `heat`, spent on the mission's bestiary, two to a spawn socket at most, every unit at the mission's `phase`. Then dressing by the mission's table (`dress:torch` is a torch eight times in ten).
6. **name**: a bound thing gets its contract name, everything else `${zone}_${node}_${socket}` (`mine_vault_page_main`). **Never a coordinate, never the attempt**: a jar's `grow` id is its own key, so a re-rolled layout cannot hand the same jar out twice or lose one. Inside authored lists, `@socket` and `@self` are resolved to those names.
7. **emit** the Blueprint and the trigger rows it carries (`Blueprint.triggers`, below).

The last attempt (`ZONE_ATTEMPTS - 1`) stamps the mission's hand-placed `fallback` through the same steps: never throw at the player. On 1,000 seeds the mine has not needed it; the worst seed took three attempts (965 of the 1,000 took one), at about 55 ms a mine with every proof.

**The lock-in macro** (`writeLockin`): one function writes every lock-in, so none can forget its reset. For a room entered by gate G: an `enter` row on the room's rect locks G, **shows a way in**, wakes the boss (or spawns the listed units) and says so; its `reset` undoes all of it; a `while` row, when they are dead, unlocks G, hides the way in and sets `${zone}_${node}_clear`. The way in is a hidden `way_in` prop in the corridor beside G whose `to` names this zone and a mark inside the rect. Hidden until the room seals, so it is never a way round a keyed gate; it is how a friend who was late, or who died and walked back, follows. The rect is the whole floor on purpose: the clear only fires while someone stands in it, and a death only re-opens the gate when nobody alive is left in it, so a rect that stops short of the walls forgets whoever is at a lever. The gate stands outside the room, so it cannot land on anyone.

**What the engine gained for it**, each a row or a verb:

- `Blueprint.triggers`: trigger rows a builder writes itself. `zoneTriggers(catalog, bp)` (`sim/runtime.ts`) merges them with the catalog's rows for the zone; the sim looks rows up in `rt.triggers`, the validator reads the same merge and checks the blueprint's rows as the catalog checks its own (`actionRowErrors`), plus that every prop, mark and rect they name exists. An id in both is a validation error.
- A prop's `to` may name the zone she is in: she is moved to the mark, nothing reloads (`hop` in `interact.ts`).
- Tile `Sill` with flag `F_NOPUSH`: feet cross it, a pushed prop does not (`footprintFree`). It is the floor just inside every door in use, so barrels never leave their room: none can jam a corridor or be lost to the plate that needs it.
- Action `strike`: everything hostile in a named rect is hit once, with an effect if the row names one; her friends are spared unless `hitsFriends`. The blow is the puller's, so the kill and the party's penalty are hers. Effect `staggered` is a row: speed 0 and every school taken double.
- Rows: `jar`, `jar_big`, `leaf_page` (their `use` is `grow` with their own key), `way_in`, `notice`, `hoist_lever`, and the mine's `page_repair`, `broken_hoist`, `broken_cabinet`, `firstaid_stove`. A holding may be `guardedBy` units of its room: it is locked until they are dead, and the generator writes the row that unlocks it.

**The solver, extended** (`validate.ts`). `validateBlueprint` takes options and can return a trace (which flood first reached each cell, which flood opened each prop).

| | |
| --- | --- |
| `verbs` | Spells known at the door. Given, a prop that `answers` Repair, Grow or a school fires only once a spell of that kind is known, and spells are learned from `learn` rows in a reached prop's `use` list or `talk` tree. Left out, nothing is gated, which is how the older zones are still judged. The mine is solved knowing only Icebolt, so "Repair is in the drawer, behind the Headmaster" is proven, not assumed |
| `when` | A `while` row fires only when its conditions can hold: a flag some fired list set, a unit that was reached, or one a reached `enter` row would spawn onto a reached mark. A negated condition never blocks |
| hops | A `to` into the same zone is a one-way edge: its mark becomes another place the flood starts from |
| `withhold`, `shut` | Ablation. Take one thing away (a key tag, a spell, a flag, a prop's lists) or hold a gate shut, and see what is still reached |

**The checks** (`checks.ts`, run by `buildZone` through a zone's `check` hook; any error re-rolls the candidate, and the error names the check):

| | |
| --- | --- |
| C1 | Every critical node is reached, first-reached floods never decrease along `order`, and **every lock holds**: solve again without the key tag, the spell or the flag, and the far room must be unreached. C2 (no key behind its own lock) falls out |
| C3 | On the room graph, every order of spending plain keys (a tag that fits more than one lock) reaches every critical node. `lintDef` runs it on the mission alone |
| C4 | For each material, everything every sink can eat is no more than chests and certain drops supply |
| C5 | The room that teaches a verb has something to try it on, holds what that costs, and the teacher is `guardedBy` every hostile in the room |
| C6 | Each lock-in: the way in is hidden, shown by the row that locks, hidden by the clear and the reset, lands inside the rect, and is reached with the gate held shut |
| C7, C8 | The first completion is walked (critical nodes in order, real cell distances with the locks opened so far out of the way). Its length is in the mission's band, and the rest room (no heat, no spawn sockets, off the hub, something to rest at) is first reached in its band |
| C9 | The room graph has a loop; the boss, and the end of some `shortcut` edge, are within `restToBossCells` of the rest room with everything open |
| C10 | The verb's first lock and the boss gate: some cell reached in a strictly earlier flood sees the prop, on screen, walls only |
| C11 | A plate whose release re-locks something has a pushable in its room with a push path onto it, dressing included, sills respected |
| C12 | No generated row shows or locks something solid inside the rect that trips it |

Templates are proven separately (`harness.ts`): each is stamped **alone**, holding what its mission node holds, a dead-end stub on every door, in every transform it claims, and solved from each door in turn: every `grants` socket reached, every other door walked out of, and every `blocks` held with the `until` socket's lists suppressed. `lintRoom` holds the grid rules (rim, sills, nothing solid on a sill or between facing doors, spawns four cells from a door, two cells of width between doors, a push path to every plate, no lever under its own hoist, every claimed transform fits its bays).

**Adding a dungeon** is four files and no engine code, unless it needs a new verb:

1. `data/dungeons/<id>.json`: the mission (`DungeonDef`). Copy `mine.json`. `id` is the zone id. Every name the story, a quest or a test will use goes in a node's `binds` (`{ "from": "chest:vault", "as": "vault_chest", "what": "prop" }`; `from` is a socket, mark or rect id of the template) or on an edge (`gateAs`, `propAs`). Everything else is named for you.
2. `world/dungeon/rooms/<id>/*.room`: at least one template per pool the mission names, every variant of a pool with the same sockets, marks and rects the mission uses. Give every room four optional doors unless it must not have them (the entrance keeps a wall for its way out): the layout needs the freedom, and a door nobody uses is wall again.
3. `world/zones/<id>.ts`: `const d = dungeonZone("<id>"); export const zone: ZoneDef = { id: d.id, build: d.build, contract: d.contract, givenKeys: d.givenKeys, givenVerbs: d.givenVerbs, check: d.check };` The contract is derived from the binds.
4. Rows for anything new it places (`data/props/<id>.json`, `data/units/<id>.json`, `data/dialogue/<id>.json`, art in `art/props/<id>.ts`), and a door somewhere that leads to its entrance mark.

Then `test/templates.test.ts` holds its rooms to the lint and the harness with no change (it walks every mission there is), and a copy of `test/dungeon-gen.test.ts`'s first test holds its seeds. Tune `budget.critPathCells`, `restAt` and `restToBossCells` to what the first hundred seeds measure, not the other way round: a band nothing fits is twelve attempts and a fallback.

Not built yet: the stateful flood for two-state buildings (the Museum brings it), `reveal` for the wall notice, the adit (it needs a county chunk), the powder store (it needs Explosion), the template bot harness, and the dungeon page of the seed viewer.

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
- save storage (`test/storage.test.ts`): the gzip round trip on a real save and its size, then the slot rules over an in-memory database and text store: writes to one slot land in order, a read waits its turn, the `localStorage` and no-gzip fallbacks, the one-time migration (old key included, junk left alone), a corrupt slot reading as empty. The IndexedDB wrapper itself is out of a headless test's reach
- **the county** (`test/county.test.ts`): two full counties. Every mark can be walked to from the platform; the first walk is road and lit; nothing spawns in a haven, on the first walk or in a wall, and threat sets the phase; same seed, same county; and **a bot steps off the train and walks the lit road to Julie's gate in two to four minutes**
- **the county skeleton** (`test/skeleton.test.ts`): 64 seeds in the suite, 1,000 as a soak. Every row of `sites.json` holds, same seed same county, ten minutes across by road, the School above the town, patches in every region, night and lamps, bridges, speed
- **generated dungeons** (`test/templates.test.ts`, `test/dungeon-gen.test.ts`, `test/dungeon-verbs.test.ts`): every `.room` file lints clean and proves its grants and blocks alone, from every door, in every transform; 64 seeds of the mine (1,000 as a soak behind `DUNGEON_SEEDS`) pass the solver and C1 to C12 with no fallback; same seed, same mine; names survive re-rolls; the checks reject what they should; a solo bot finishes the whole mine on three seeds, drawer, cabinet, first aid, track, hoist and nook included; the sill, the way in, `strike`, blueprint triggers, the solver's verbs, `when` and hops; save v6 to v7
- determinism, save → load → continue, replay, A\* bounds, LOS, catalog integrity ("every quest can be given and handed in by some row"), art integrity, five zones × 25 seeds through the solver

The 2026 audit caught its two worst bugs in seconds with a 150-line headless harness the project never had. This is that harness, kept.

## 14. Not built

Honest list. Each is a row or a module, not a rewrite: cart on a track (`machine.kind = cart`), lily-pad rafts, Explosion / Grow / Electric spells, the Museum and every zone after it, audio bus, touch layout, options menu, res sickness, the other three burial corners and the wizard. `SYSTEMS.md` §12 has the order.
