# Jane, Architecture (the Rust engine)

Crates this doc owns: `jane-core`, `jane-schema`, `jane-data`, `jane-world`, `jane-sim`, `jane-bot`, `jane-net`, `jane-cli` (including the `jane serve` subcommand, the headless host). Presentation (`jane-art`, `jane-present`, `jane-app`) is `ART.md` and `PRESENTATION.md`; this doc owns the sim-facing `View` and `Event` API they consume (§11). Pair with `ENGINE.md` (the TS record of *why* each rule exists), `PLATFORM.md`, `SYSTEMS.md` (the bar) and `PORT.md` (the plan: phases, targets, toolchain; worldgen port specifics in PORT §6).

Nothing here exists yet. This is the contract the crates are built to; a signature missing from it is proposed here first (PORT §8).

**Rule:** this file and `PORT.md` are the agreement for the engine. Code that drifts from them fixes the code or the doc first.

## 0. Rules

| Rule | Why |
| --- | --- |
| **No floats in core, schema, data, world, sim, net, bot and art.** `deny(clippy::float_arithmetic, float_cmp)`, `disallowed-types` for `f32`/`f64` newtypes and `HashMap`/`HashSet`/`RandomState`, CI grep for `\bf(32|64)\b` and `as f`. `jane-schema` may read `0.62` inside a `deserialize_with` on the host side and emit `Permille(620)`; that is the only place a float is touched | One platform-defined bit is a desync; integers cost nothing to prove and are faster on a Pi and a Pentium 4 |
| **One canonical tick.** `Sim::step(&StepInput)` is the only way time or state moves. Commands apply inside `step`, at its start, in `(seat, seq)` order | The TS applied commands between ticks, with an aim taken from a later frame |
| **Authoritative = can change a future tick. Derived = rebuildable and cannot.** Only authoritative is saved and hashed. A test drops and rebuilds every runtime mid-run; the hash stream must not move | The TS hashed path caches and `anim`; `Sim.aims` was neither saved nor hashed |
| **Iteration order is array order or `BTreeMap` order.** Hash maps only through `Lookup<K, V>` (get, insert, remove; no iteration). Sorts use unique integer keys; ties break on id | Unforgeable through the type system, not by review |
| **Ids, not strings, in the hot path.** Catalog rows are `u16` indices fixed at compile time; runtime names are `Sym(u32)` | The TS did string lookups per unit per projectile |
| **Content is compiled.** `data/**` becomes static tables; an unknown field or a dangling name is a build error | A Pi never parses JSON |
| **Typed events, no English.** The sim emits `Event` with ids; presentation owns strings, `{name}`, `{place:}` | The TS held toast text in the sim |
| **No allocation in the tick after warm-up, no `dyn` in per-unit loops, no threads in the sim** | Pi 3 and Pentium 4 are the floor |
| **Wrapping is explicit.** `wrapping_*` only in hashing and RNG; CI runs the determinism suite with overflow checks on in a release profile | A wrap on one target and a panic on another is a desync |
| **Edition 2024; the deterministic crates build on Rust 1.85** | rust9x for Windows 7 and XP (PORT §3.1) |

## 1. Crates and arrows

```
jane-core ──► jane-data ──► jane-world ──► jane-sim ──► jane-net
                 ▲                            │  ▲
jane-schema ─────┘ (build-dep)                │  └── jane-bot
                                              ▼
                       jane-art ──► jane-present ──► jane-app (SDL2)
                                              ▲
jane-cli ── sim, world, net, bot, schema, art, present (headless; `jane serve` lives here)
```

| Arrow | |
| --- | --- |
| `jane-schema → core` | Constructs the same `Catalog` structs the game runs on, so dev reload and compiled tables cannot differ |
| `jane-data → core` (lib); build-dep `schema` | `build.rs` runs the schema over `/data` and emits statics |
| `jane-world → core, data`, **never sim** | `Tile`, flags, rng, `Action` types, catalog, the zone list live in core and data |
| `jane-sim → core, data, world` | The sim calls `build_zone`; world never calls back; `story_name` stays in `jane_world::names` |
| `jane-bot → sim` | The headless player |
| `jane-net → sim, core` | Lockstep over `std::net` |
| `jane-present → sim (View, Event), world::names, data (TEXT, NAMES), art` | No SDL; reads a `View`, never `GameState` |
| `jane-app → present, sim, net` | The SDL2 binary: window, input, audio, loop, saves, config |
| `jane-cli → sim, world, net, bot, schema, art, present` | `gen`, `check`, `bench`, `replay`, `sheet`, `view`, `serve`, `soak`, `hash`, `film`. PNG through `jane-art::sheet` (ART §5), no image crate |

Import cycles inside the old sim become modules sharing one `Ctx` (§5). Dependencies allowed: `serde` and `serde_json` (schema; `serde` also for the sim's save, through `jane-core`'s optional `serde` feature), `postcard`, `lz4_flex`, `xxhash-rust` (xxh3), `indexmap`, `sdl2` (app only). Nothing else without a commit that says why (PORT §3.5).

**Rule:** `jane-world` and `jane-sim` share nothing but `jane-core` types and the `Blueprint`.

**Where things went:**

| TS | Rust |
| --- | --- |
| `text.ts expandText`, `SPELL_ERROR_TEXT`, toast literals | `jane-present::text`: tables keyed by `ToastKind`, `SpellError`, `TextId`; `TEXT[TextId]` expansion (`{name}`, `{place:}` through `jane_world::names::story_name`) |
| `cleanName` | Presentation, before `Join` is sent |
| `world/index.ts CONTRACTS, GIVEN_KEYS, ZONE_IDS`, `zones/*.ts`, `registry.ts` | `data/zones.json` → `ZONES: &[ZoneDef]`; builders matched on a `ZoneKind` enum; file order = `ZoneId` order = tick order |
| `constants.ts` tunables, `country.ts` tables | `data/tuning/{sim,skeleton,county,country,dungeon,chunks}.json` → `TUNING`. Structural consts (`CELL`, `TICK_RATE`, `RING_BLOCK`, `PATH_WINDOW`, `MAX_PLAYERS`) stay Rust `const` |
| `loop.ts` | `jane-app` (SDL loop) and `jane serve` (headless loop) |
| `terminal.ts` | `jane-present` console; every mutation is `Command::Dev(DevOp)` |
| `test/bot.ts` | `jane-bot` |
| `chunks.ts` (932 lines of authored places as code) | `data/chunks/*.chunk` and a ~200-line stamper in `jane-world::chunks` (PORT §6.f) |

## 2. Numerics (`jane_core::num`)

This section is authoritative for the types. PORT §6.b and §6.c carry the worldgen arithmetic table (which TS expression becomes which integer form, site by site).

| Type | Definition | Used for |
| --- | --- | --- |
| `Fx(i32)` | 1/256 px. `FX_ONE = 256`, `CELL_FX = 2048`, `METRE_FX = 2048`. A county is 16 000 px = 4.1 M; `i32` holds 8.4 M px | positions, speeds per tick, ranges, radii |
| `Vec2 { x, y: Fx }` | `Cell(u16, u16)`, `CellIx(u32)`; `cell(x) = x >> 11`, `centre(c) = (c << 11) + 1024` | space |
| `Milli(i32)` | 1/1000 of a point | hp, mp, energy, hits |
| `Permille(i16)` | 1/1000 | party tables, resists, effect speed, lifesteal, `hpBelow`, loot, night reach |
| `Angle(u16)` | 65 536 per turn, 0 = east, clockwise on screen | headings, aim, bolts, fans |
| `Q15(i32)` | cos and sin table entries | direction arithmetic |
| `Tick(u32)` | 2.2 years at 60 Hz | every clock |
| `Sfc32 { a, b, c, d }` | the generator from `rng.ts`, wrapping | all randomness |
| `Q16(i32)` | 16.16 | worldgen noise, terrain fields, county lattices; never in the sim |

Helpers:

```
mul_div_floor(a, b, c) -> i32      // i64 intermediate, div_euclid
mul_div_round(a, b, c) -> i32      // where the TS rounded damage; each call site says which
scale(a, Permille) -> i32
dist_sq(a: Vec2, b: Vec2) -> i64   // never a sqrt to compare
isqrt(u64) -> u32                  // Newton, exact floor; only where a length is spent
cos_q15(Angle) / sin_q15(Angle)    // 4096-entry i16 table by a >> 4
iatan2(dy, dx) -> Angle            // octant + 257-entry table
along(a: Angle, s: Fx) -> Vec2
```

**Rule:** division is floor. `mul_div_round` only where the TS rounded and the site says so.

**Rule:** angles are the only direction. There is no `normalize`. The table is 4096 entries because the finest turn in the game (the snake, 4.8° per tick) is fifty times coarser than 0.088°.

**Constants as integers.** Energy sprint, carry and regen 500, 250 and 500 milli per tick, max 100 000, push 20 000. Mp regen `+= Milli(spirit)` per tick (exact). Idle regen `max_milli / 300`. `PARTY_DEALT` and `PARTY_TAKEN` as `[Permille; 4]`. `PHASE_SCALE` as `[u8; 7]`. Night: `aggro * (10 + 4 * dark) / 10`, `leash * (10 + 6 * dark) / 10`. Effect speed, resist, lifesteal, mana shield, `hpBelow` and loot chance all `Permille`. Crit is `rng.below(one_in) == 0`. `heal` splits into `heal` (`Milli`) and `heal_pct` (`Permille`) at the schema. Idle regen is floored to the milli-point per tick, so a bar fills in a little over 300 ticks.

**Power rolls** (`SpellPower { stat, div, var_div, flat }`, `div` and `var_div` in thousandths). A spell's blow is whole points, as the TS's was (`makeHit` rounded): the fixed part `stat / div` is floored to a milli-point (`stat * 10^6 / div` milli); the random part is `irandom(floor(stat * 1000 / var_div))` whole points (GameMaker's `irandom` floors its bound); `flat` is added; a crit doubles the sum; the sum is rounded half up to a whole point (`div_round(m, 1000) * 1000`). Per blow the zone stream draws the power roll, then the crit roll. A pool's pulse rounds its roll the same way, without a crit. In the flush, damage is `blow * resist * party / 10^6` in milli (floored), less a mana shield (`absorbed = min(dmg, mp * 1000 / shield)`, the mp it costs floored), then rounded half up to a whole point; a splash is the blow over `div`, and lifesteal the damage times its permille, each rounded to a whole point; a heal lands exactly (only what is missing). Speeds in px per tick become `Fx` at compile (2 → 512, .55 → 141, 1.2 → 307); metres become `Fx` (× 2048); fan degrees become `Angle`. `BODY_HALF_FX = 768`. Light reach is `r * r * 4 / 9` in Fx². `focusOf` penalties are `Fx` with one `isqrt` per candidate inside USE reach.

**RNG.** `next_u32`, `below(n)` (Lemire multiply-shift; `n == 0` gives 0), `irandom`, `chance(Permille)`, `pick`. `seeded(seed, stream)` is a splitmix32 spread plus 12 warm-ups. Worldgen draws through `dice()` (PORT §6.a). `rngFloat` is gone.

**Worldgen in integers** (the table is PORT §6.c; the shape of it): `hash2 -> u32` read as `Q16 = h >> 16`; `smooth_q16` in `i64`; fbm weights are shifts; chamfer `u16` steps 10/14; road costs `i32` in Q8 × tenths (`len10 ∈ {10, 14}`, `rough_q8 = 179 + (fbm_q16 * 666 >> 16) + ground_q8`, slope × 56, water × 7680, road × 77, heuristic `77 * octile10`: the road rate times the octile distance in the same tenths, so it never overestimates), costs `u32` in core's one A* (a window and `cut_corners`); metres through `dist_sq` against `(m / MACRO)²`; `roadDistances` in tenths 160/226; terrain `dy * 5 / 4`, hill through `isqrt` and `smooth_q16`; the county painter's bilinear becomes Q16 per-row gradients (the DDA it already is); dungeon scores integer; `heat` through `mul_div_round`.

## 3. State model

### 3.1 Ids and names

| Kind | Type | Notes |
| --- | --- | --- |
| Content rows | `SpellId, EffectId, ItemId, UnitDefId, PropDefId, QuestId, DialogueId, TriggerId, RecipeId, TextId, StoryId, ConsequenceId, ZoneId` | `u16` newtypes in sorted-string order at build. `ZoneId` is a closed enum of 13 whose order is `zones.json` order, which is tick order |
| Content names | `NameId(u16)` | Every contract key, mark, rect, trigger rect, `location`, `grow` id, flag, and every derived `zone_node_socket` name |
| Runtime names | `Sym(u32)` | Interned at runtime, pre-seeded with `NAMES` so `Sym(n) == NameId(n)` below `NAMES.len()`. Generator-only names append; a save stores only the tail, as strings |
| Instances | `UnitId, PropId, DropId, ProjId, GroundId` | `NonZeroU32`, one monotonic counter each in `GameState.next`, never reused. Generational arenas rejected: ids must be stable across saves, and four billion per kind is unreachable |
| Seats | `Seat(u8)`, `ClientToken(u64)` | |
| Flags | `FlagKey = enum { Named(Sym), Been(Sym), Dead(Sym) }` in state (a content name's `Sym` is its `NameId`; content's own `FlagKey` names a blueprint `Key`, resolved when the verb runs) | Replaces the `been:` and `dead:` string surgery |
| Lists | `ListRef = enum { Catalog(u16), Blueprint(u16) }` | An action list is a slice, never copied per instance |

Units per zone are a `Vec<Unit>` in ascending id (spawn appends, despawn removes in place), found by binary search. Props likewise, never removed. Each zone runtime keeps `Lookup<NameId, PropIx>` and `Lookup<NameId, UnitId>`.

### 3.2 The tree

```
GameState { version, seed, frame: u32 /* step calls: the wire and replay clock */, tick: Tick /* advances only when not frozen */,
  clock, day, name: String, open, next: Counters, rng: Sfc32 /* world level: weather and ecology, §4.4 */,
  players: ArrayVec<PlayerState, 4>, zones: [Option<Box<ZoneState>>; ZONE_COUNT],
  flags: BTreeMap<FlagKey, i32>, quests { active: Vec<QuestProgress>, done: Vec<QuestId> },
  rest: Option<RestPoint>, growth { spells, strength, spirit, found: Vec<Sym> }, syms: SymTable,
  journal: Journal /* §3.7 */, weather: [WeatherState; 3] /* a sky per region, §4.6 */,
  consequences_done: BitVec /* one bit per ConsequenceId, §4.6 */, consequences_owed: Vec<(ZoneId, ConsequenceId)> /* edits waiting for their zone */,
  rumours: BTreeMap<(NameId /* person */, StoryId), Tick /* since */> }

PlayerState { seat, who, unit: UnitId, zone, last_mark: NameId, respawn_at: Option<Tick>,
  bag: Box<[Option<Stack>; 24]>, bar: [Option<BarSlot>; 8], craft: [Option<Stack>; 3] /* hers: OFF the unit */,
  dialogue: Option<Dialogue { tree, node, line, speaker, read }>, god, stats,
  travel: Option<TravelRequest>, connected, parked: Option<Box<Unit>>,
  assist: Option<Assisted { unit: UnitId, until: Tick }> /* the sticky unit, §5.4 */ }

ZoneState { id, rng: Sfc32 /* this zone's combat, loot and fan dice */, units: Vec<Unit>, props: Vec<Prop>, drops, projectiles, grounds,
  triggers: TriggerBits { fired: BitVec, inside: BitVec }, tile_deltas: BTreeMap<CellIx, Tile> /* compacted */,
  fog: Box<[u32]> /* fixed size */, pending_fill: Vec<Fill>, sleeping_due: Vec<(Tick, UnitId)>,
  wetness: u8 /* the rain ramp, §4.6 */, pressure: Vec<u16> /* per area, in the blueprint's area order, §4.6 */,
  ring_key: Option<[Option<(i32, i32)>; 4]> /* each seat's ring block when the ring last ran, §3.3 */ }

Unit { id, key: Option<Sym>, def, controller, faction, pos: Vec2, facing, strength, spirit, hp, mp, energy: Milli, energy_locked, alive,
  gcd_until, stop_until: Tick, cooldowns: Vec<(SpellId, Tick)>, item_cooldowns, synced: Tick, target: Option<UnitId>, combat, home,
  patrol: Option<Box<Patrol>>, patrol_at, dwell_until, order: Option<Box<Order { to, until, then: Option<ListRef>, seat }>>,
  path: Option<Box<PathCache>>, statuses: Vec<StatusInst { effect, until, next_pulse, from }>, died_at: Option<Tick>,
  awake, hidden, carrying: Option<PropId>, hold: u8, phase: u8, snake: Option<Box<SnakeBody { heading: Angle, phase_tick, step, trail }>> }

Prop { id, key, def, spawn: Option<u16> /* index into Blueprint.props: keyTag, to, use, release, needs, talk, label, nightLock, underWhen live THERE */,
  cell, solid, hidden, locked, used, on, loot: LootState, under_done }
```

**Left the unit:** `anim`, `animTick`, `hx`, `hy` become events plus `View::moved_this_tick` and `SnakeBody.heading`. `book` becomes derived `book_of(u)` (def plus growth for players; the phase book for AI). `bag` moves to `PlayerState.bag` ("bags are hers"; `hasItem` and `acquire` read any seat from any zone). `incoming` becomes runtime scratch (`Scratch.hits`, one queue per zone of `Hit { to, amount, school, from, crit, status }`, empty between steps). `respawn`, `deadFor`, `gcd`, `stop`, cooldown decrements and `thinkOffset` become absolute ticks; `think_offset = (id * 0x9E37) % 10`, no RNG draw at spawn. Props no longer copy lists; the blueprint is rebuilt on load from `(seed, zone)`.

### 3.3 Authoritative vs derived

**Authoritative:** everything in §3.2, including `Unit.awake` (one bit; changes only on ring re-evaluation; saved so the invariant is checkable), `PathCache` (it moves the unit next tick) and `ZoneState.ring_key` (whether the ring runs next tick depends on it, and an `awake` bit held up by combat or an order outlasts its reason until the ring runs again; a rebuilt runtime that forgot the key would run the ring when the live one did not).

**Derived** (`ZoneRuntime`; rebuilt on load and on zone entry): `Grid { tiles, flags }` = blueprint plus deltas; occupancy as a **count** per cell, `Lookup<CellIx, u16>`, plus the `F_OCC` bit (the TS kept the first unit to claim a cell, so who held a shared cell depended on arrival order and a rebuild could disagree; "occupied by someone else" is "occupied, and not my own cell", and who stands where is asked of `unit_blocks`); `prop_buckets` per 16-cell block; `awake_props` (re-derived from `ring_key` on a rebuild); `plates`; `unit_blocks` of awake, alive, unhidden units per block, one sorted `(block, id)` list; `awake_units`; `names`; `unit_names`; the merged trigger table; `dirty_rects`; `prev_pos`. The blueprints live beside the runtimes in `Sim` (`Blueprints`, one `Arc<Blueprint>` per zone), not inside them.

**Derived** (`Sim.scratch`, one per sim): one `PathScratch` (256² window, ~3 MB, shared by every zone), `LitField`, `hits`, `ops`, the event buffer.

**Rule:** a test throws every runtime away mid-run and rebuilds it; the hash stream must not move (§8 `runtime_rebuild_is_invisible`).

### 3.4 Input and commands

```
InputFrame { mv_dir: Angle, mv_mag: u8 /* 0..127 */, aim: Option<Angle>, sprint, use_held, assist: AssistProfile /* Off | Pad | Mouse */ }   // 7 bytes
StampedCommand { seat: Option<Seat> /* None = join */, seq: u16, cmd: Command }
StepInput<'a> { frames: [InputFrame; 4], commands: &'a [StampedCommand] }   // sorted (seat, seq)

enum Command { Use, Bar { slot, on }, Cast { spell, on }, Item(ItemId), BagMove, BagDestroy, CraftPut, CraftClear, CraftClearAll, CraftTake,
  Bind, Unbind, BarSwap, Advance, Choose, CloseDialogue, Join { who }, Leave, Open(bool), Dev(DevOp) }
```

The client turns move and aim into `(Angle, magnitude)` with whatever arithmetic it likes and quantises; the sim never normalises. Facing from angle by the dominant axis; an exact 45° keeps the current axis (`faceVector`). `Sim.aims` is gone: a `Cast` at frame `f` reads `frames[seat].aim` of frame `f`, so the aim is in the record. The frame carries the raw aim and the assist profile, never an assisted angle: the sim resolves assist itself at cast time (§5.4), so a replay of the recorded frames reproduces every assisted cast exactly.

### 3.5 Save

serde + `postcard` (varints, canonical) + an `lz4_flex` block, its size prepended (the block format: the frame format's checksum would pull in a second hash crate). Layout: `JANE`, a `u32` header length, the postcard header (`save_version: u16`, `content_hash: u64`, `build`, and a summary: zone, day, hour, hp, max hp) readable without decoding the body, then the body. Core types serialize through `jane-core`'s `serde` feature, which only the sim turns on; a `Tile` is saved as its id. About 150 KB compressed for a busy county.

**Content pin.** Same `content_hash`: load. Different: a typed `SaveMigration` chain if one exists, else **refuse** in release. In dev, `--allow-content-drift` remaps the `Sym` tail by string and drops gone ids with a report. Migrations are Rust functions over typed previous structs, kept twelve months. `Sym` tails are stored as strings (readable, a few KB).

`from_save` closes the world, parks every seat but 0, rebuilds blueprints and runtimes for live zones, derives the props' awake bits from the saved `ring_key`, and neither runs the ring nor steps: the saved key may lag a seat by a block (the ring runs before movement), and the next step re-runs it exactly where the unbroken game would, so a solo save continues as if never saved. Slots are owned by `jane-app` and `jane serve`; the sim only encodes bytes.

### 3.6 Hash

`Sim::hash() -> u64` is xxh3-64 of the postcard encoding streamed into the hasher: the same schema as the save, so saved and hashed cannot drift. `zone_hashes() -> [u64; ZONE_COUNT]` hashes each zone's encoding alone (a second pass, asked only after a mismatch) for desync localisation. Under 1 ms on a Pi 3, taken once a second.

### 3.7 Journal and known facts

The player's long-running context is engine state, not a presentation cache. The fog is the *seen* record; the journal is the *understood* one: what she has been, whom she has met, what she was told and whether the world bore it out.

```
Journal { entries: Vec<JournalEntry>,                       // a ring of the last N per kind (§12), oldest overwritten
          known: BTreeMap<FactKey, Known { since: Tick, how: Source }> }

JournalEntry { tick: Tick, fact: FactKey, how: Source /* what it is known by from now */, zone: ZoneId, at: Cell }
                                                    // kind() = fact.kind(): one entry per change to `known`

enum FactKey { Place(Sym), Person(Sym), Thing(ItemId | PropDefId), Claim(TextId),   // content writes NameId; a Sym
               Route(Sym, Sym) /* from, to */, Danger(Sym) /* area */, Rumour(StoryId),    // is a NameId below NAMES.len()
               Consequence(ConsequenceId) /* the state's only: the world did it, Seen */ }

enum Source { Seen, Visited, Named,          // Place
              Met, Talked, Dead,             // Person
              Held,                          // Thing (Seen shared)
              Read, Told, Confirmed, Contradicted,   // Claim: how it was heard, and what the world later did to it
              Walked,                        // Route
              AttackedIn, Fled,              // Danger
              Heard }                        // Rumour
```

| | |
| --- | --- |
| **Writers** | `Location` (Place Visited); arriving in a zone (Place Seen, the zone's name); a dialogue line's `tells` field as the line is shown (Place Named, Person Talked, Thing Seen, Claim Told, Route and Danger Told, Rumour Heard); talking to a unit with a key (Person Met); sign `Read` of a content text (Claim Read); a kill (Person Dead through `hooks::unit_died`; Danger AttackedIn once areas reach the blueprint); a chest, a pick-up, a reward or a craft (Thing Held); `Travel` and a door (Route Walked, from the mark she came in by to the mark she arrives at); the first hit taken in an area and leaving it under aggro (Danger AttackedIn, Fled); a `Consequence` firing (§4.6) that confirms or contradicts a `Claim` |
| **Readers** | `Condition::Knows { fact: FactKey }` and `Condition::Heard { claim: TextId }` (§5), so a line can say "you have seen the mill" or "you were told the bridge was out"; the quest legibility checks in `VERIFICATION.md`; the quest log's own text (`View::journal()`, `View::known(fact)`, §11); the map, which draws a named place only once it is `Known` |
| **Upgrade** | A stronger `Source` for the same key replaces a weaker one (`Seen` → `Visited` → `Named`; `Met` → `Talked` → `Dead`; `Seen` → `Held`; `AttackedIn` → `Fled`; `Read` and `Told` are equal); `since` keeps the first tick. `Confirmed` and `Contradicted` replace `Told` and `Read` and are final |

**Rule:** the journal is append-only and bounded: a ring of the last N entries per kind (§12) plus the `known` map. It is authoritative, saved and hashed. Text is never stored, only ids; `Claim(TextId)` is the line, and presentation expands it.

**Rule:** a writer emits `Event::Journal(kind)` only when something changed (a new key, or an upgrade), never for a repeat, so the shell can mark the log without diffing it.

## 4. The tick

### 4.1 Surface

```
impl Sim {
  fn new_game(seed, name) -> Sim          // builds all 13 zone blueprints up front (§9); sits the host down (Join { host }) before frame 0
  fn new_game_with(Blueprints, name) -> Sim            // over blueprints already built (tests, benches, peers share one Arc'd set)
  fn from_save(&[u8]) -> Result<Sim, SaveError>
  fn from_save_with(&[u8], Blueprints) -> Result<Sim, SaveError>
  fn step(&mut self, &StepInput) -> Stepped { ran: bool }   // ALWAYS applies commands; advances unless frozen
  fn frozen(&self) -> bool
  fn drain_events(&mut self) -> &[Event]
  fn view(&self, seat) -> Option<View<'_>>              // None for a seat not connected
  fn save(&self) -> Vec<u8>
  fn hash(&self) -> u64
  fn zone_hashes(&self) -> [u64; ZONE_COUNT]
  fn rebuild_runtimes(&mut self)                        // §8 runtime_rebuild_is_invisible
}
```

`frame` counts step calls (frozen ones too; it is bumped at the end of the step, so during step `n` it reads `n`); `tick` counts advances. Replay and the wire speak `frame`. With two or more seats `frozen` is never true. Alone, a frozen step is a run-length entry in the replay. "Frozen while one player is in dialogue" generalises to a predicate the shell may also read for menus, never a broadcast.

### 4.2 Order

```
 0 commands      (seat, seq) order, in that seat's zone ctx, actor = seat; Join / Leave / Open are world level; Dev is ordinary
 1 freeze        if frozen(): return { ran: false }
 2 clock         tick++, clock++, day rollover; clock rows fire ONCE, actor None, in the county's ctx (its runtime made for them
                 if nobody is there, dropped at 15), heard by the whole party;
                 on the hour: weather and ecology rolls from the world stream in fixed order (§4.4), before the clock rows;
                 every tick: the rain ramp of every zone state, consequences whose bit is clear and trigger is true fire
                 once, rumours start, edits owed to a zone now live land (§4.6, `living.rs`)
                 PartySnap { size, bodies: [Option<(ZoneId, UnitId, Vec2)>; 4], resting: [bool; 4] }
                 (`resting` is unfilled: `Rest { until }` asks every seat live, as the TS did, `verbs::everyone_resting`)
   for zone in ZoneId order where live (a connected seat is in it): Option::take the zone out of state.zones
 3 presence      every 30 ticks: schedule slots resolved by the clock (dayOnly / nightOnly are two-slot schedules); hide, show
                 and jump only outside 120 x 80 px of a watcher, walk by order inside it (§4.6.a)
 4 ring          on block change of any seat here: the union square; sleep and wake units and props
 5 catch-up      every awake unit's regen paid to now (a unit that woke this tick catches up; phase reset with it);
                 its missed status pulses land at step 9 as one hit (§4.3)
 6 players       seat order: a dead seat wakes at respawn_at; input, sprint and carry energy, hold-to-push, movement
                 (stun and the statuses' speed product apply)
 7 controllers   over the awake_units snapshot: ai | snake | npc (order or patrol); stunned skip. An ai or npc
                 unit under orders walks them first and minds nothing else (`orders::step_orders` until the ai lands)
 8 projectiles   fly; first sight-blocking cell or enemy body through unit_blocks; splash; school touch. Grounds pulse through unit_blocks
 9 statuses      awake living: pulses into hits
10 flush x2      the ONLY place a blow changes hp: party permille, god, mana shield, lifesteal (the second pass lands it),
                 first-hit aggro, phases, death → loot, quest hooks, onDeath. Passes run until the queue is empty
                 (two in practice; a third only when a list run by a death or phase in the second strikes again)
11 triggers      enter / while for the first living seat in the rect; plates every 6 ticks (actor None)
12 housekeeping  drops expire, corpses due on sleeping_due stand up, pending fills, prop flag re-stamp of dirty rects,
                 fog every 10
13 zone ops      apply ZoneOps: spawn (append), despawn (remove + forget_unit), wake; put the zone back; drain WorldOps
14 travel        seat order: remove, forget_unit, place, ring force, fog, presence
15 drop          runtimes of zones with nobody connected
```

The same order as `sim.ts` with four corrections: commands inside the step; clock rows once; spawn and despawn deferred; cross-zone work queued.

- **`ZoneOps { spawn, despawn, wake }`** collects what `run_actions`, `kill_unit`, `revive` and `send` did to `zone.units` mid-iteration. A spawned unit exists from step 13.
- **Controllers** (step 7, `jane_sim::{ai, npc, snake}`) run over a copy of `awake_units` taken as the step reaches them; an order comes before anything else a unit would do. Walking is one function (`ai::follow_to`): a cached path re-planned when used up, when the goal moved or every `REPATH_TICKS`, the zone's four searches a tick shared first come first served, a path let go keeps its box (no allocation once warm). A search that finds nothing is remembered as an empty path toward that goal: a chaser stands and asks again when its re-plan comes or she moves (the TS asked every tick it could, and its "fail, then leash" never fired: the failed search had just reset the clock it read). Presence (step 3, `jane_sim::presence`) never jumps a unit to its mark while either end is inside a watcher's box.
- **`forget_unit`** is the one function that clears everyone's target, drops combat to leash, and vacates occupancy and `unit_blocks`. Kill, despawn, travel and leave all call it; the TS had four copies.
- **`WorldOps`**: `PayRewards { quest, list }` (once per connected seat in her zone, actor her), `Teach`, `Grow` (patches every body, parked included), `Announce(Event)`. Drained in order after the zone is put back.
- **Party queries** read `PartySnap`, one tick stale, which is unobservable.
- **Hits** are queued per zone in `Sim.scratch` and land at that zone's step 10 of the same step: a pass lands each target's blows in the order dealt, targets in ascending id, and what a pass raises waits for the next pass (the TS let a blow raised on a later unit land in the same pass). The queue is empty between steps, so it is neither saved nor hashed; a blow dealt in a step that does not run (the console while she reads alone) or on a zone that did not tick (its last seat left this frame) is dropped.
- **Combat's seams** into other units' systems are the named functions of `jane_sim::hooks` (world spells, school touch, a bar item, quest kills, death bookkeeping, ecology on respawn, waking: what she carried, lock-ins).

### 4.3 Clocks without a sleeper pass

| Clock | Was | Is |
| --- | --- | --- |
| gcd, stop, cooldowns, dwell, order budget | decremented per tick | `*_until: Tick`, read at use |
| mp regen, idle regen | per tick, every unit | `synced: Tick` + `pay_regen(u, tick)`: elapsed × spirit, idle `max / 300 × elapsed`, clamp. Called for awake units each tick (step 5), for a sleeper on wake, and by anything reading or writing `mp` or `hp` (a cast, an effect's mana, the flush). Idle is an AI with its row's `auto_regen` while idle, any AI while leashing, never one under orders (a snake out of its fight mends hp only); the controllers do not regen themselves. Sleepers are idle by definition, so the catch-up equals the per-tick sum |
| statuses | per tick | `until`, `next_pulse`; active while `until > tick`; missed pulses queued as one hit on wake |
| respawn | per tick, every corpse | `sleeping_due` sorted `(tick, id)`; housekeeping pops `<= tick` |
| player respawn | counter | `respawn_at` |
| drops | age | `born` |
| phase reset | per tick | in `pay_regen` when hp is whole |

The `budget` test's 3 000 corpses still stand up on schedule, for a `Vec` pop instead of 3 000 branches.

### 4.4 RNG streams

`ZoneState.rng = seeded(seed ^ fnv1a(zone), 1)`: power and crit per hit, fan offsets, loot, ground pulses. `GameState.rng = seeded(seed, 1)`: the world stream, whose first use is the living world of §4.6. Worldgen: `dice()` (PORT §6.a). Entering the mine before the cellar no longer shifts a county roll; `think_offset` comes from the id.

**The world stream draws in a fixed order, once an hour**, inside the clock step when the hour turns: first the weather roll for every region in region order, then the ecology rolls for every area in `ZoneId` then area order, whether or not that zone is live (a roll for an absent zone is drawn and discarded). No other step touches `GameState.rng`, so the order the party visits zones cannot move a weather or a respawn roll (§8 `zone_order_does_not_move_rolls` covers it).

### 4.5 Seats

`Join` chooses the arrival (the party's `rest`, else the county start), reuses a parked body by token, else creates one with the start kit into `PlayerState.bag` and bar, and catches growth up. `Leave` puts the carried prop down, calls `forget_unit`, parks the body, and hands story items (`ItemDef.story`, compiled: anything with `opens` or an `acquire` target) to the lowest connected seat, overflow at her feet. `Open` is seat 0 only. Every change emits `Event::Party { connected }`.

### 4.6 Schedules, weather, ecology, consequence

The design of the living world is `WORLD.md`. This section says only what the engine provides for it: five row kinds, where their state lives, and which step reads them. All of it is clock-driven and deterministic; none of it adds a per-tick pass over sleepers.

**(a) Schedules.** A unit def may carry `schedule: [(hour_from: u8, hour_to: u8, Slot)]` with `Slot = Mark(NameId) | Patrol | Inside(NameId /* prop */) | Absent`. The presence step (§4.2 step 3) resolves the slot from the clock; `dayOnly` and `nightOnly` compile to two-slot schedules and stop being special. `Absent` and `Inside` hide the unit, `Mark` and `Patrol` show it and give it somewhere to be. **Rule: nothing teleports inside the watcher box.** A unit whose slot changed while a seat can see her gets an `Order` to the slot's mark and walks; one nobody can see is moved and re-stamped in place. `Inside(prop)` keeps the unit attached to the prop so a door can say who is behind it (`View::schedule_state`, §11).

**(b) Weather.** World state, one row per region: `WeatherState { kind: Clear | Mist | Rain | Storm, since: Tick, until: Tick }`, in region order (`GameState.weather: [WeatherState; 3]`). When the hour turns, the clock step draws two rolls per region from the world stream (§4.4), a kind roll and a length roll, against the region's row in `data/weather.json` (kind weights out of 1000 by hour band and season, a duration band in hours per kind, and the zones under that sky: every zone under exactly one); if `until` has come, the kind roll picks the kind and the length roll the hours; a roll before `until` is drawn and discarded so the stream stays in step. New Game is clear, held for `tuning/sim.json` `clearHours` (the first walk). Every zone whose state exists keeps `wetness: u8`, a ramp that climbs by `wetness.rise` every `wetness.every` while its region's sky rains or storms and the zone is not indoors, and falls by `wetness.fall` otherwise; it is stepped with the clock for every zone ever visited, live or not, so a zone left in the rain dries while nobody is there and the order of visits moves nothing. The sim reads wetness for **one rule only**: a prop whose def has a `douse` shows no light while the zone's wetness stands at or over it (`light::light_showing`, the `lit` rule of §11 `lamps_lit`; still a bed or a fire to rest at, and a light again once the ramp falls under the line), which is why wetness is authoritative. Presentation reads `weather()` and `wetness()` for everything else (mist, puddles, sound) and changes no state. The county is under the Lowfields sky as one zone; a sky by the region under her feet waits on the blueprint carrying the skeleton's region grid.

**(c) Ecology.** A blueprint lists its `areas` (`Blueprint.areas: Vec<Area { name: Key, rect: Rect }>`; the county's are the skeleton's patches as placed, each a square of its radius about its centre; other zones have none). `data/ecology.json` gives an area its `recover` and its populations `{ unit, cap: u8 /* living units of that def whose home is in the area */, weight, hold }`. `ZoneState.pressure[area]`, in the blueprint's area order, is a counter that a kill of a unit whose home lies in the area raises by its population's `weight` (`hooks::unit_died`) and the hourly ecology roll lowers by `recover / 2 + below(recover + 1)`, floor 0 (one roll per area of every blueprint, drawn whether or not the zone has ever been visited; a visited zone not live is lowered all the same). A respawn from `sleeping_due` never happens in view: while a seat's watcher box (§4.6.a) holds where the corpse lies or the home it would stand at, it is pushed back a presence beat (`presence::stands_up_unseen`, for every respawn, patch or not). Then it stands the unit up only if fewer than `cap` of its def stand in the area and the area's pressure is under the population's `hold` line (`hooks::respawn_allowed`); otherwise it is pushed back onto `sleeping_due` at the next hour. Hunting thins an area and time refills it, with no timer per corpse. Unit defs gain `hunts: [UnitDefId]` and `flees: [UnitDefId]`, read by the idle branch only (the AI's; an npc's for `flees`, so a hen runs from the fox): an idle unit with a `hunts` target inside its aggro reach (after the party, whom it always prefers) takes it as a target; one with a `flees` unit inside its leash (centre to centre) walks its leash away from the nearest, at a run, and is otherwise unchanged (its home stays, so its next leash takes it back). Neither field touches the combat branch, so a hunt only holds where the two sides are enemies (§3 `is_enemy`: one of them friendly); a `forget_unit` leashes a hunter whose target is not a seat, as it does any unit in combat.

**(d) Consequences.** `data/consequences.json` rows: `{ id, on: Flag(FlagKey, test) | QuestDone(QuestId) | Dead(NameId), zone /* county when left out */, edits: [Action], confirms | contradicts: claim? }`, where `edits` is drawn from the world verbs only: `Show`, `Hide`, `Switch`, `Spawn`, `Despawn`, `Fill`, `Send`, `Flag` (through `Send`'s `then`; the schema rejects anything else, `Lock` included, and a row whose edits are flags alone, WORLD.md §6). The clock step checks every row whose bit in `GameState.consequences_done` is clear, every tick; when the trigger is true it sets the bit, writes the journal's `Consequence(id)` fact (`JournalKind::Consequence`), emits `Event::Consequence(id)` to everyone, and runs the edits in the row's zone with `actor = None`: now if the zone is live, else queued in `GameState.consequences_owed` (saved) and run the moment the zone is next live (on arrival, step 14, before the ring and presence; or in step 2 for a zone that became live another way), so a rebuilt bridge or a relit relay run is there when she arrives. A `Consequence` may name a `Claim(TextId)` (a line some row says) it confirms or contradicts, and the journal records it (§3.7).

**(e) Rumours.** A story row may declare `spreads: { to: [NameId], after: Tick }` (content writes seconds). The story's trigger is the first hand-in of any quest it tells; from that tick each person in `to` (a unit's key, or a door's: a door speaks for whoever is behind it) is written into `GameState.rumours` at `fired + after`, and a dialogue on that person may test `Condition::SpeakerKnows(StoryId)` (§5): true once the entry's tick has come. A conversation is open when its start rules are asked, so a start rule can ask it. The player learns it as `Known::Rumour(story)` when the line with `tells` plays. Clock-driven: the clock step writes the entries in story row order, no RNG.

**Rule:** every row kind here is content, checked at build (§6), and every state field here is authoritative, saved and hashed. A consequence fires once per save, ever; the bit is the proof.

## 5. Actions and conditions

### 5.1 The verbs

```
enum Action {
  Quest(QuestId), HandIn(QuestId), Flag { key: FlagKey, op: Set(i32) | Add(i32) }, Rest { until: Option<u8> }, Grow { stat, amount, id: NameId },
  Give { item, qty }, Take { item, qty }, Learn(SpellId), Toast(TextId), Read(TextId),
  Lock(NameId), Unlock(NameId), Show(NameId), Hide(NameId), Switch { prop, on: Option<bool> },
  Spawn { key, def, at }, Despawn(NameId), Aggro(NameId), Location(NameId), Fill { rect, tile },
  Strike { rect, amount: Milli, school, effect, hits_friends }, Status(EffectId), Heal(Flat(Milli) | Pct(Permille)),
  Travel { zone, mark }, Talk(DialogueId), Throw(ItemId), Shake(u8), Camera { mode, rect },
  If { when: &[Cond], then: ListRef, els: Option<ListRef> }, Send { unit, to, then: Option<ListRef> }, Reveal(&[NameId]),
  Place { prop: PropDefId, item: ItemId } /* a prop made at the subject's feet holding one item: the light stone */ }

enum Condition { Flag { key, test: Eq | Min | NonZero }, Night, QuestActive, QuestReady, QuestDone, HasItem { item, qty }, HasSpell(SpellId), Dead(NameId),
  Knows { fact: FactKey }, Heard { claim: TextId }, SpeakerKnows(StoryId) /* §3.7, §4.6.e */ }
struct Cond { not: bool, c: Condition }
```

Exhaustive `match`, so a new verb is a compile error in `run_action`, in the solver's `each_action`, in the schema validator and in the emitter. Blueprint lists are `Vec<Action>` in `Blueprint.lists` reached by `ListRef::Blueprint(i)`: one slice per list, no per-instance copy.

### 5.2 Running a list

```
struct Ctx<'a> { cat, tuning, world: &mut WorldState /* players, flags, quests, growth, rest, clock, tick, syms, world rng */,
  zone: &mut ZoneState, rt: &mut ZoneRuntime, bp: &Blueprint, party: &PartySnap,
  actor: Option<Seat> /* by value; no asPlayer */, ops: &mut ZoneOps, wops: &mut WorldOps, events, scratch }
enum Subject { Unit(UnitId), Prop(PropId), None }

fn run_actions(cx, list: ListRef, subject)
fn run_action(cx, &Action, subject)
fn conditions_met(cx, &[Cond]) -> bool
```

**Rules:**

- Player-scoped verbs (`Give`, `Take`, `Rest`, `Travel`, `Talk`, `Read`) do nothing with `actor == None`, and a build-time check flags lists that can never have an actor (clock rows, plate lists, a poison's `onDeath`).
- `HasItem` reads the seat's bag; `HasSpell` reads growth; flags are the world's. `Knows` and `Heard` read the journal (§3.7); `SpeakerKnows` reads `GameState.rumours` against the dialogue's speaker and is false outside dialogue.
- Names resolve through `rt.names`; a miss is a no-op plus `Event::Missing` in dev.
- `Strike.from` is the subject if alive. `Send` sets `order.seat = actor`. `Subject` never changes inside a list.

### 5.3 Where names are checked

1. **Providers, at build** (`jane-schema`): every `NameId` used by any list, condition or quest must be provided by a declared source: the zone contract in `zones.json`, a dungeon `bind`/`gateAs`/`propAs`, a derived `zone_node_socket` name from every mission × pool template, a placement row key, an anchor or area id, a `.chunk` export, a story slot or thing. Zone-scoped lists must find the provider in that zone; zone-less lists (dialogue, rewards, item use) may find it anywhere.
2. **Blueprint, per build** (`jane_world::validate`): contract names exist, no duplicate keys, nothing in a wall, every blueprint list names things in this blueprint, the lock-and-key flood, C1 to C12.

A typo in `"prop": "gate_bos"` is a build error naming the file and the row.

### 5.4 Aim assist

Aim assist lives in the sim, so the pad and the mouse play the same game and a replay lands the same bolts. The frame carries `aim: Angle` (raw) and `assist: AssistProfile` (§3.4); at cast time, inside the commands step, the sim resolves the raw aim to an assisted angle and `Cast` fires along that.

```
struct Assist { cone: Angle /* half-angle */, snap: Angle, magnet: Permille, sticky_ticks: Tick, slack: Angle }   // one per profile, TUNING.assist[profile]

fn assisted_aim(cx, seat, spell, raw: Angle, profile) -> Angle
  // Off, or a spell that does not hurt what it is aimed at (anything but a bolt or a melee, and any heal): raw. Otherwise:
  // candidates: hostile units (to the caster's side; never an npc), alive, awake, unhidden, within the spell's reach of
  //   the caster between bodies (a melee's range; a bolt's range plus the caster's body), inside cone about raw
  //   (the sticky unit from PlayerState.assist is a candidate inside cone + slack while until >= tick)
  // score: |Angle::diff(raw, bearing)| as an integer, minus a small bonus for unit.target (2°) and a smaller one for the
  //   sticky unit (1°); ties by unit id
  // a bolt whose raw line passes within the spell's touch of a prop it would switch (shown, not on, answering its school),
  //   no further along than its flight plus touch and nearer the caster than the best: raw, nothing made sticky
  // best within snap: take its bearing
  // else: raw moved toward it by magnet permille of the difference (mul_div_floor)
  // none: raw. The chosen unit becomes PlayerState.assist { unit, until: tick + sticky_ticks }
```

`Angle::diff(a, b) -> i32` is the signed shortest turn, computed in `i32` and reduced to `-32 768..32 768`, with no wrapping op. Bearings come from `iatan2`; candidates come from `unit_blocks` inside the spell's range, so the search is the same box the victim search already walks.

The result is exposed as `View::assisted_aim(frame, spell) -> Option<Angle>` (§11; the `View` is already one seat's), computed from the frame the presentation is about to send and the spell on the bar, so the reticle can draw where the bolt would go; it reads the sticky unit and never sets it, and it is derived and never stored. `None` when the frame has no aim. The tuning rows are `data/tuning/sim.json` under `assist`, one row per profile, degrees and permille at the schema, `Angle` and `Permille` compiled; `Pad` is wider than `Mouse` on every field and `Off` is raw (§12 has the defaults).

**Rule:** assist never targets a friend, a prop or a corpse. Heals keep the mouse-over rule and ignore assist entirely. The mouse-over rule reads `Cast { on }`: with a cursor, the unit under it (the caster's own body for "over nobody"), and a friend there takes the heal or the cast fails and says why; `on: None` means no cursor (a pad), and a friendly spell lands on the friend nearest the aim line within 16 px of it, else on the caster.

**Rule:** the raw aim and the profile are the recorded input; the assisted angle is what `Cast` uses and what the replay reproduces. A frame never carries an assisted angle, and `Sim.aims` stays gone.

## 6. Content compile (`jane-data/build.rs`)

`data/` holds: the 20 tables plus fragments (merged in path order, duplicate id = error); `zones.json` (new: `[{ id, kind, contract, givenKeys, givenVerbs, states }]` in tick order); `dungeons/*.json`; `rooms/<dungeon>/*.room`; `chunks/*.chunk` (authored county places as data, PORT §6.f; no manifest, the directory is the list); `tuning/{sim,skeleton,county,country,dungeon,chunks}.json` (PORT §6.g); `looks/*.json` (keyed by sprite id, ART §5; a `sprite` or `icon` with no look, or a look nobody uses, fails the build); the skeleton rows; `names.json`; `bindings.json` (default key, mouse and pad bindings, PRESENTATION §4); `consequences.json` and `weather.json` (§4.6).

**Schema additions for the living world (§3.7, §4.6, §5.4):** unit defs gain `schedule` (`[{ from, to, mark | inside | patrol | absent }]`, every hour in exactly one row, never beside `dayOnly` / `nightOnly`), `hunts` and `flees`; dialogue lines gain `tells` (a list of `FactKey` writes, checked against the ids they name); story rows gain `spreads` (a story with a quest to hand in, its listeners provided names); `ecology.json` rows give an area its `recover` and its populations' `cap`, `weight` and `hold` (an area the skeleton has, a unit row that exists, each at least 1); fire props gain `douse`; `consequences.json` rows are checked like clock rows (world verbs only, more than a flag, every name provided, `on` a flag that is set somewhere, a quest that exists or a provided `dead` name, a claim some row says); `weather.json` has one row per region, in region order, every zone under one of them, every hour in exactly one band, weights that sum to 1 000 per band, a kind lasting at least an hour; `tuning/sim.json` holds `journalRing`, `wetness { every, rise, fall }` and `clearHours` (the `assist` rows per profile are still `tuning.rs` constants). A `tells` that names a place, person or story nothing provides is a build error like any other name. What names people and places by the hour (a schedule's marks and doors, a story's listeners) is interned after the chunks, so it moves no name the groups before it interned.

| Step | |
| --- | --- |
| Parse | serde into `jane-schema` raw structs, all `deny_unknown_fields`; `quest.returnTo` becomes a typed field; errors name `file:row.field` |
| Units | seconds → `Tick` (cooldown, duration, `pulse.every`, respawn, item cooldown, ground duration and pulse); metres → `Fx` (range, bounds, aggro, leash, ground radius); px → `Fx` (speed, walk, run, `splash.radius`, `touch`, `light.radius`); degrees → `Angle` (fan); fractions → `Permille` (chance, resist, effect speed, lifesteal, `hpBelow`, heat). `stop` stays ticks; `heal` splits into `heal` and `heal_pct`. The unit of every field is in `jane-schema/UNITS.md` and the struct's doc comment |
| Intern | ids in sorted-string order; a `NameId` for every name any list, condition, quest, contract, bind, placement or socket uses; a `TextId` for every English string |
| Check | everything `validateCatalog` did, plus providers (§5), `dead` unit names, flags read vs set (a flag read but never set is a build error once P1 triage has cleared the existing ones), zone ids in `travel` and `to`, marks in `doors.json` and `start.mark`, clock rows without player verbs, recipes by sorted ids, phases in falling `hpBelow`, every `.room` through `lint_room`, every mission through `lint_def` and its pool contract, every quest givable and handable by some row |
| Emit | `catalog.rs` (`pub static CATALOG: Catalog = ...` with nested `&'static` slices), `names.rs`, `text.rs`, `tuning.rs`, `zones.rs`, `rooms.rs`, `chunks.rs`, `looks.rs`; `content_hash: u64` = xxh3 of canonical JSON of everything that changes behaviour (text excluded) |
| Cost | a few hundred ms; `rerun-if-changed=data` |

**Rule:** needs only data → build time. Needs a seed → a test. Nothing checks itself at game start.

**Dev iteration.** `cargo run` after editing JSON is the loop. Feature `dev-data` makes `catalog()` load `JANE_DATA_DIR` through the same `jane-schema` code and `Box::leak`; text reloads live; a catalog reload needs a new game and says so. A test builds both ways and asserts equality (§8 `compiled_equals_dev_data`).

**Tuning.** `TUNING.party_dealt`, `.party_taken`, `.phase_scale`, `.night_aggro`, `.night_leash`, `.energy { sprint, carry, regen, push }`, `.regen_divisor`, `.gcd`, `.crit_one_in`, `.respawn_default`, `.drop_life`, `.use_reach`, `.aggro_period`, `.assist { pad, mouse: Assist }` (§5.4), `.journal_ring`, `.wetness { rise, fall }`, `.country { ... }`, plus the worldgen tables of PORT §6.g. Builders and the sim take `&Tuning`; the static is the default; the streams test flips a value in memory.

## 7. Lockstep networking (`jane-net`)

| Decision | Choice | Why |
| --- | --- | --- |
| Transport | **TCP**, `TCP_NODELAY`, length-prefixed postcard frames | Lockstep needs every input in order or it stalls anyway; on a LAN loss is nil; a UDP reliability layer is 500 lines that only pay over the internet, which is out of scope |
| Topology | Star: the host relays and paces; everyone simulates | One place decides that a frame is complete; the host may be `jane serve` on a Pi, no SDL |
| Discovery | UDP broadcast on 7777: `JANE?` → `JANE! { name, seats_used, frame, content_hash }`; join by IP too | |
| Input delay | `D = 3` frames (50 ms), host-configurable 2..6 | Below perception under a 90-tick GCD |
| Pacing | The host emits `Bundle(f)` when every seat's `Input(f)` has arrived; a client steps on `Bundle`. Missing for more than 500 ms: `Stall { seat }` to all. Missing for 10 s: the host injects `Leave`, unless the host's **wait** toggle is on, in which case the session waits | No prediction, no rollback; a slow peer stalls friends on a LAN, which is correct |
| Hash | Every 60 frames each peer sends `Hash { frame, h }`; a mismatch is `Desync { frame, seat, zone_hashes }`; every peer writes `desync-<frame>-<seat>.save`; the host offers `Resync` (snapshot the odd one back in) | `jane replay diff` localises the field |
| Late join | `Hello { proto, content_hash, build, token }` → `Welcome { seat, snapshot at frame F, delay }`; the host buffers bundles from `F + 1` until `Ready`; `Join` is stamped `F + D` | The snapshot is a save at a step boundary |
| Content check | `content_hash` and `build` must match or the join is refused, showing both | |
| Host migration | Out of scope; the session ends, guests' bodies are parked in the host's save | |
| Local play | The same `Session` over `Transport::Local` | One code path |

```
enum Msg { Hello, Welcome, Ready, Input { frame, frame_in: InputFrame, cmds }, Bundle { frame, frames: [InputFrame; 4], cmds: Vec<StampedCommand> },
  Hash { frame, h }, Desync, Stall, Resync { snapshot, frame }, Bye }
struct Session { role: Host | Guest, seat: Option<Seat>, sim: Sim, delay: u8, pending: BTreeMap<u32, Bundle>, .. }
impl Session { fn push_local(frame_in, cmds); fn try_step() -> Option<Stepped>; fn poll() }
```

A `Bundle` is a replay frame; a session log is a valid `.jrp`. `jane serve --slot 1 --open --seats 4 --delay 3 [--wait]`: loads or creates, listens, steps on bundles, saves on `Rest` and on `SIGTERM`, logs hashes and stalls, and prints one status line (tick, seats, hash). `PLATFORM.md` §4's relay is void; this section replaces it.

## 8. Determinism verification

| Test | Holds |
| --- | --- |
| `same_tape_same_hash` | Two sims on one seeded random tape, equal every 120 frames for 20 000 |
| `replay_roundtrip` | A bot session with joins, leaves, casts, dialogue and dev commands re-simulates to the same hashes |
| `save_load_continue` | Save at frames 131 and 257 of 520, load, continue; equals the unbroken run |
| `runtime_rebuild_is_invisible` | Every runtime dropped and rebuilt mid-run; hash stream unmoved |
| `zone_order_does_not_move_rolls` | Visit zones in two orders; county rolls identical |
| `awake_only_equals_everyone` | A test switch (`Sim::tick_everyone`) ticks every unit the old way (regen paid to every unit every tick, the controllers' pass over every unit asking whether it is awake); both runs settled (`Sim::settle`: every unit paid to now, which only moves `synced`), hashes equal, and a run settled only at the end equals them |
| `streams` | Carried: a change to one stage's draws moves nothing outside it |
| `same_seed_same_blueprint` | 64 seeds per zone, twice in process, across a process boundary, across the CI matrix |
| `solver_and_checks` | Every zone, every seed, through the solver and C1 to C12 |
| `budget` | 3 000 sleepers and 8 000 props: median step under 1 ms on a Pi 3, under 0.25 ms on x86_64 CI |
| the carried suites | Against `jane-bot`: sim, engine, replay, coop, verbs2, rest, dungeon-verbs, quests |
| `compiled_equals_dev_data` | Static catalog equals the `dev-data` load |
| `overflow_free` | The suite under `--profile checked` |
| **`cross_target_hash`** | `jane replay verify fixtures/*.jrp` on every target, qemu included. Worldgen hashes gate from P3; bot-session hashes from P4 (PORT §7) |

CLI: `jane replay verify | record | diff | trace`, `jane hash --seed N --frames K`, `jane save inspect`, `jane world build <zone> --seed N [--png]`, `jane data check`, `jane bench county | tick`, `jane film`.

**Playtest hooks for `VERIFICATION.md`.** Determinism says the game is the same twice; these say whether it is any good, with the same tapes.

| Hook | |
| --- | --- |
| **Player models in `jane-bot`** | The bot gains the named models of `VERIFICATION.md` §2 L3: `Reader` (follows signs and quest text literally), `Explorer` (walks everything), `Rusher` (straight lines, sprint on cooldown), `Cautious` (stays on lit roads, rests at every fire, retreats), `Co-op pair` (two seats, together and split), `Lost` (drops its objective and rebuilds it from the journal alone). A model is a policy over `View` and `View::journal()`, deterministic for a seed, and every model's tape is a `.jrp` |
| **`Sim::metrics() -> Metrics`** | Counters kept beside the state, never in it: distance walked (`Fx`), fights started and finished, deaths, ticks in dialogue, ticks lost (walked and walked back, by the fog's own path record), unknown-place lookups (a `Route` or `Place` the quest text names that is not `Known`, §3.7), casts assisted vs raw, rests. Not saved, not hashed; the `runtime_rebuild_is_invisible` test proves it |
| **`jane film`** | Runs a tape headless through `jane-present` and writes one frame per N ticks as PNG through `jane-art::sheet` (ART §5), so a run can be watched without a window |
| **`jane replay trace`** | Per-tick export of a tape: `frame, tick, seat, zone, pos, hp, target, assisted_aim, journal writes, events` as one line each, for a diff between two models or two builds |

## 9. Performance plan

**Memory.** County: 4 M tiles `u8` + 4 M flags `u8` = 8 MB; the blueprint keeps its own 4 M for delta compaction: 12 MB per live county. One shared A* scratch ~3 MB. A `Unit` is ~120 B; 3 000 of them are 360 KB. No `u128`.

**Tick.** Median **under 1 ms on a Pi 3** in a busy county with one seat; **under 4 ms worst** with four seats in four zones and 60 awake AI each. Only awake work: `awake_units` drives controllers, statuses, the flush, victim search through `unit_blocks`, plates and `clear_footprint`. Spatial: props by 16-cell block; awake units by block, updated in `move_unit`. Path: windowed A*, integer costs 10/14, budget 6 000, four searches per tick per zone, node-index tie-break. `Scratch` owns every temporary `Vec` (`clear()`, never `new()`); boxed optional unit parts are allocated on spawn only; a counting allocator asserts zero allocations across 600 steps after warm-up (a `#[global_allocator]` is an `unsafe impl`, which `unsafe_code = "forbid"` refuses in every workspace crate and its tests, so that check needs a home outside the lint: a small crate with the one exception PORT §3.4 allows. Measured out of tree for the movement-only tick, P4's first slice: 0 allocations in 100 000 steps after 600 of warm-up). Enums matched in place; no trait objects in the tick. No threads in the sim.

**Worldgen.** About 50 ms on x86_64 and about 0.5 s on a Pi 3 per county including validation (PORT §9.4 has the gates). **New Game builds all 13 zones up front** on an app thread behind a loading screen that shows the skeleton as it lands; the sim is handed finished blueprints and never builds mid-play. A blueprint disk cache (`~/.cache/jane/<content_hash>/<seed>-<zone>.bp`) is deferred to P9 and built only if the Pi misses its gate.

**Hash and save.** About 1 ms per MB.

## 10. Smells fixed / kept

**Fixed:** `Sim.aims`; clock rows per player; `asPlayer` and the circular ctx; lists mutated while iterated; the sleeper timer pass; one perturbed RNG; per-instance list copies; append-only `tileDeltas`; growable fog; anim driving logic; `book` pushed into; the bag on the body; FNV-over-JSON hash and JSON-gzip save; string keys in hot loops; the catalog built thrice; no schema check; unchecked names; the sim ↔ world cycle and module singletons; seven "nearest free cell" copies and four "clear target" copies; unit mixing (px, metres, ticks, seconds); English in the sim; linear `party.ofUnit`; the replay side channel; `frozen` before `tick++`; authored places as code; camera size baked into worldgen; `hasZone` gating; aim was raw with no assist, so the pad could not play (now deterministic assist in the sim, §5.4); the world had no memory of the player, so no line could say "you were told" (now the journal, §3.7).

**Kept, deliberately:** the `sim.ts` tick order and the double flush; windowed A* 10/14 with node tie-break; supercover LOS; the 6 × 6 body box with axis-separated sliding; occupancy shapes paths and never blocks; prop buckets by origin cell with reach-back; the load ring from block centres, with `awake` saved; the verb list, conditions, the trigger model with `reset`, the quest kinds, the dialogue shape; the solver and C1 to C12; the dungeon generator's steps and naming; terrain never saved (seed plus deltas); freeze only when alone, one heroine, the party penalty by connected count, story items handed on, one fire; string keys for units and props, as `Sym`; `dev` commands as ordinary commands.

## 11. Sim-facing API for presentation

```
struct View<'a>   // one seat, her zone, read only
  seat(), me() -> &PlayerState, body() -> &Unit, heroine() -> &str, seed(), frozen(), clock() -> (ticks_since_midnight, day)
  zone(), size(), indoor(), ambient() -> Permille, tile(cx, cy), flags(cx, cy), fog_seen(bx, by), fog_cells()
  units_in(CellRect) -> impl Iterator<UnitView { unit, prev_pos, moved, variant: u8 /* from unit.id, derived */ }>     // awake, unhidden
  props_in(CellRect) -> impl Iterator<&Prop>, prop_spawn(&Prop) -> Option<&PropSpawn>,
  light_showing(&Prop) -> Option<&Light>, lamps_lit()                          // THE rule, shared with the sim
  drops(), projectiles(), grounds(), unit_at(Vec2),
  focus() -> Option<Focus { target: FocusRef, verb: Verb /* Enter, TryTheDoor, Unlock, Open, PickUp, Craft, Read, Use, HoldToPush, Take(ItemId), Talk, PutDown, Custom(TextId) */, pushes: bool /* "(hold to push)" */ }>
  hud() -> Hud { hp, max_hp, mp, max_mp, energy, statuses, target: Option<(UnitId, Permille)> }
  dialogue() -> Option<DialogueView { speaker, lines: &[TextId], line, options: &[TextId], awaiting_choice }>
  quests() -> impl Iterator<QuestView { id, counts }>, near_bench(), near_rest(), craft_output(), book(), marks(), rects(), debug()
  assisted_aim(frame, spell) -> Option<Angle>                                  // §5.4; the reticle draws it, the sim casts along it
  tick(), frame(), is_night(), learned(), sym(&str), name(Sym), mark(Sym), rect(Sym), triggers(), list(ListRef),
  unit(UnitId), props(), sight(a, b)                                           // what jane-bot's models read (§8), all derived
  journal() -> impl Iterator<&JournalEntry>, known(fact: FactKey) -> Option<&Known>   // §3.7; the log and the map read these
  weather() -> &WeatherState /* her zone's region's */, weather_of(Region), wetness() -> u8   // §4.6; presentation's mist and puddles, no rule
  schedule_state(unit) -> Option<ScheduleState { slot, at: Mark(NameId) | Inside(PropId) | Walking(Cell) | Away }>   // so a door can say who is in

struct Event { to: Option<Seat>, in_zone: Option<ZoneId>, kind: EventKind }

enum EventKind { Toast(ToastKind), Damage { unit, from, at, amount, school, crit, absorbed }, Heal, Death { unit, def, at }, Respawn,
  Cast { unit, spell, at }, CastFailed { unit, spell, why: SpellError }, Impact { spell, school, at }, Swing { unit, at, facing },
  Status { unit, effect, on }, Loot, Learn(SpellId),
  Quest { quest, change }, Zone { zone, first }, Shake, Camera, Tiles(CellRect), Prop { prop, change }, Bag, Dialogue, PlayerDied, Rest,
  Sfx { kind, at }, Party { connected }, Journal(JournalKind), Weather { region, kind }, Consequence(ConsequenceId) }

enum ToastKind { Text(TextRef), QuestGiven(QuestId), QuestDone(QuestId), KillProgress { quest, req, n, of }, InventoryFull, TooTired, Needs { item, qty },
  NothingToRepair, NothingGrowsWithoutLight, NothingGrows, Locked { prop } /* its label and `locked_says` */, UnlockedWith(ItemId), NightLock(TextRef),
  WokeAtRest, WokeAtDoor, PartyChanged, LeftWhatMattered, PutDownFirst, ShouldKeep, NotHurt, FitsALock, ItShifts, NoRoom, NightWaits, Stronger,
  WordsStay, Learned(SpellId), Under { top, found } /* found's label */, SpellError(SpellError) }

enum SpellError { CastUnsuccessful, YouAreDead, OnCooldown, OnGcd, TooFar, NoTarget, NotEnoughMp, NotEnoughEnergy, NotInLos,
  NotValidTarget }   // a toast only for those worth saying: not the GCD, a cooldown or a plain failure
```

Routing is the TS rule: personal kinds carry `to`; zone-local kinds carry `in_zone`; party-wide kinds carry neither. `events_for(seat)` filters. Strings come from `TEXT[id]`, expanded by `jane-present::text::expand(s, heroine, seed)`.

**Rule:** `View` is the contract. `jane-present` builds its per-frame `SceneView` and `HudView` buffers (PRESENTATION §3.6) from this `View` once a frame, into reused buffers, with no reference into `GameState`; it sends `Command`s and nothing else back. Seat 1 alone and seat 4 of four are the same code. Per-instance folk variation (`vary`, ART §3) is `UnitView.variant`, derived from `unit.id` in the View so the renderer and `jane film` agree; it is never authoritative state and never saved.

## 12. Defaults taken

Owner decisions folded in above: integer-only sim and worldgen with new seeds; the same pixel look at 216 px; deterministic lockstep for four seats with a headless host; all 13 zones built at New Game; bounded `vary` variants picked by unit id; 60 fps on a Pi 3 as the floor (PRESENTATION §1.8 has the degrade order). The rest were open questions and are recorded here as defaults; any flip is a one-line edit before P0 starts.

| Question | Default |
| --- | --- |
| `stop` and `heal` at the schema | `stop` stays ticks; `heal` splits into `heal` (`Milli`) and `heal_pct` (`Permille`) |
| Content pin on load | Refuse in release; remap the `Sym` tail by string in dev (`--allow-content-drift`) |
| RNG | One stream per zone (`ZoneState.rng`); the world stream is reserved |
| `Sym` tails in saves | Strings, not hashes |
| Stall | A seat is dropped at 10 s; the host has a `--wait` toggle |
| Blueprint disk cache | Deferred to P9; built only if the Pi misses its gate |
| ARM targets | `armv7` (Pi 2, 32-bit OS, cortex-a7) in; `armv6` (Pi 1, Zero) out |
| A flag read but never set | A build error, after P1 triage clears what the data has today |
| New Game | Builds all 13 zones up front behind a loading screen that shows the skeleton |
| `jane serve` output | One status line: tick, seats, hash |
| Aim assist, `Pad` | `cone` 20°, `snap` 4°, `magnet` 350‰, `sticky_ticks` 30, `slack` 5° |
| Aim assist, `Mouse` | `cone` 8°, `snap` 2°, `magnet` 200‰, `sticky_ticks` 30, `slack` 2° |
| Aim assist, `Off` | Raw; the profile is still in the frame |
| Journal ring | 512 entries per `JournalKind`; the `known` map is unbounded (one row per fact the content can name) |
| Weather | Rolled once an hour on the hour, per region, from the world stream; `Clear` at New Game, held for `clearHours` (2) |
