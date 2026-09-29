# Jane: Verification

The verification stack for the Rust build. Pair with `EXPERIENCE.md` (what happens, minute by minute, and what she knows at each point; written alongside this), `PLAN.md` (the game), `PORT.md` §9 (the test strategy this extends), `ARCHITECTURE.md` §8 and §11 (determinism, `View` and `Event`), `QUEST-TREE.md` §2 (the audit and the truth rules), `DUNGEONS.md` §2.6 (C1 to C13) and `SYSTEMS.md` Rule 0.

Written 26 September 2026 from the owner's direction: *"verification layers of gameplay itself probably need to go 10-100x deeper to nail a truly fun generated game. The world needs to breathe and live and be cohesive in a global context. The whole experience needs to be distilled into highly detailed documents and carefully verified against what will actually happen in the game world. Gameplay and the player's long-running context of what's happening is very much key."*

**Rule:** this is the design of what the Rust build's tests, bots and tools prove, and only what is marked **Built**, with its date and where it lives, exists. A claim that a layer runs is a bug in this file until `PORT.md` §7's gate for it is green. Built so far: the six player models of L3 (not every rule of each), the trace (§3.1), L4's metrics from it, L5's audit of the quest words and the omens against the built world, `jane sweep`, `jane dossier` and `jane audit` (§3.2), §4.1's bands as a test, and §4.3.

---

## 1. Purpose and the one rule

A hand-made game is tested by playing it. A generated game cannot be: there are more seeds than there will ever be play sessions, and the seed the owner plays proves nothing about the next one. So:

**Rule:** a generated game is only as good as what is proven about every seed.

"Fun" is not a number, so it is taken apart into properties that are, each with a band, each measured on a sweep of seeds, each printed as a table a person can read. `EXPERIENCE.md` says what the game is like; every claim in it names the check that holds it (§5). `SYSTEMS.md` Rule 0 (IN means a test names it) is the same rule one level down, for systems. This file applies it to the experience.

What the Phaser build taught (`archive/phaser-remake-2026/POSTMORTEM.md`): 102 quests, 61 with no hand-in, a first five minutes that did not work, and a docs table that said IN for a dozen things that were not. A 150-line headless harness found the two worst bugs in seconds and the project never had one. What the TS build taught (`QUEST-TREE.md` §3): when a chain confused the owner the cause was the county, not the words; the audit found thirty step failures the tests had not, because no test asked *can a person find this from the text alone*. Each layer below is one more version of that question.

---

## 2. The layers

| Layer | Proves | Runs | Cost | Fails on |
| --- | --- | --- | --- | --- |
| **L0** Data | The content is well formed and every reference resolves | Build time (`jane-data/build.rs`, `jane check`) | < 1 s | A build error naming file, row, field |
| **L1** Invariants | Every seed builds a world that can be finished | Seeded sweep (`SEEDS`) | ms per zone | Seed, zone, check name |
| **L2** Structure | The world is the size, density and difficulty `PLAN.md` says | Seeded sweep, tables | s per seed | Seed, region, metric, value, band |
| **L3** Playthroughs | A modelled player gets through it | Bot over `View`, traces | min per seed-model | Seed, model, tick, the objective it was on |
| **L4** Experience | What it was like, in numbers, by model | From traces | s per trace | Seed, model, tick, metric, band, a film clip |
| **L5** Truth and cohesion | Everything she can read or hear is true of the world it names, across zones, and assumes only what she knows | Static per seed, plus traces | s per seed | Seed, text row, the claim, what the world holds |
| **L6** Living world | People, creatures, weather and consequences behave like a place | Long traces, sampled hourly | min per seed | Seed, tick, the rule, who or what broke it |
| **L7** Human review | The owner can review a seed without playing it | CI artefacts | none to run | The owner's redlines |

Every layer prints its table on every run, in the TS suite's habit: the expectations under the table are the floor, the table is what a person reads.

### L0 Data (build time)

`ARCHITECTURE.md` §6 and `PORT.md` §5.3 own the compile; this layer is its audit view.

| Check | What |
| --- | --- |
| Schema | Every table a serde struct, `deny_unknown_fields`, units typed |
| Cross-refs | Every id in every list, condition, quest, door, recipe and look resolves; every flag read has a write |
| Providers | Every `NameId` a list or quest names is provided by a zone contract, bind, socket, placement, chunk export or story slot |
| Text rules | `VOICE.md`'s mechanical rules on every English string: no fantasy diction (the "Do not write" list as a word list), no ellipsis, no compass bearing or metre count in quest text (`QUESTS.md` K4), no id in player text (A1), a landmark in every step (A2), night and day words present where the step is gated (A4), lines at most two sentences, `{name}` and `{place:}` well formed |
| `look` coverage | Every sprite and icon a row names has a look; no look is unused; `vary` at most 4 |
| Claims registry | Every text row that makes a claim about the world carries a `claim` (§L5); a claim kind the checker does not know is a build error |

**Rule:** needs only data, build time; needs a seed, a test. Nothing checks itself at game start.

### L1 Invariants (seeded)

Carried from `PORT.md` §6.m and §9.3; the floor under everything else.

| Check | Sweep |
| --- | --- |
| Skeleton: every `sites.json` row holds by road; regions, river, hill, rail | 1000 seeds |
| County reachability: every site, gate, door mark and story place from the platform on foot | 64 in the suite, 1000 weekly |
| The first walk never crosses threat above 1 by day; a fire inside threat ≤ base in every region | 64 |
| Solver on every zone every seed; sealed gate rejected; passes unit-tested | 64, 1000 weekly |
| C1 to C13 on every dungeon; each check rejects its bad case | 64 per dungeon, 1000 weekly |
| Template proofs: every `.room` grants and blocks alone, every transform, every door | all 109 |
| Streams: a change to one stage's draws moves nothing outside it | 64 |
| Determinism gates 1 to 6 of `PORT.md` §9.3, cross-target byte equality | CI matrix |
| Generation cost floors: attempts, fallbacks, ms | 1000 |

### L2 Structure metrics (seeded, tables)

Every number is per region and per seed and is printed as a table with its band from `PLAN.md`. Bands carry from the TS `density` and `harshness` floors where those exist.

| Metric | Band | Source |
| --- | --- | --- |
| Hubs per region | 1 town or equivalent + 2 outposts | `PLAN.md` §2.4 |
| Side quests per region, chain length | 15 to 20, chains of 2 to 4 | §2.4 |
| Enemy families per region | 6 to 8, each with a home terrain | §2.4 |
| Points of interest per region | 25 to 35 | §2.4 |
| Reachable land screens with nothing on them | < 10 %; first walk 0 | `density` |
| Something in view from the first walk | > 95 % of stretches | `density` |
| Something visible from any road, walking | one per 20 to 30 s | §2.4; measured along every road |
| Something interactive from any road | one per 60 to 90 s | §2.4 |
| Deliberate empty stretch | ≤ 2 min, and only before a site or dungeon ring | §2.4 |
| Road to field creature density | road < 0.6 × field | `harshness` |
| Works to Lowfields density | > 1.6 × | `harshness` |
| Threat curve along the spine road | monotone by region base; every pocket above base has a road bending round it and is seen from that road | §2.6 |
| Dungeon approach | every cell of a dungeon mouth's ring at most the phase of the rooms behind it (built: `no_dungeon_approach_is_harder_than_its_first_room`) | §2.6 |
| Upgrade rule | by the time the crit path reaches threat N, phase N−1's upgrades are reachable without crossing N | §2.6 |
| Site distances by road | inside each `sites.json` band | §2.3 |
| Dungeon crit path | inside `budget.critPathCells` (C8); rest room first reached at 40 to 60 % (C7) | `DUNGEONS.md` §2.6 |
| Key and lock depth | every key at most 2 rooms off the crit path; every locked thing seen before its key by a tease (C10) | §2.6 |
| Hub choices | from every hub, at least 3 distinct quest directions open at once | new |
| Dead-end payoff | every dead end longer than a screen ends in a jar, a page, a chest, a sign or a view of a site | new |
| Omens | about a third true per seed; ≥ 2 per region; no two lethal ones stacked; none true on the first walk (built: the roll, its rate and the lethal rule, `omens.rs`; eight rows, at least two a region, each with a test on and off) | §5 |

**Rule:** a structure metric is measured on the built blueprint, never on a playthrough. If it needs a player, it belongs in L4.

### L3 Bot playthroughs with player models

The TS bot (`jane/test/bot.ts`) walks to a point with the sim's pathfinder, walks to a prop or unit *by key*, pushes, faces, clicks through a dialogue taking given choices, fights with melee, and can start a game inside a zone at a mark. It proves the map is walkable and a dungeon finishable. It cannot get lost, because it reads `propsByKey`, and it cannot be confused, because it is told where to go.

`jane-bot` keeps that as a **harness** (for dungeon and template proofs) and adds **player models**: policies over `View` and the journal that press only what a person could press.

| Model | Policy | What it proves |
| --- | --- | --- |
| **The Reader** | Takes every quest offered. Reads every sign in focus. Walks to what the text names, by matching landmark names against signs it has read and banners it has seen; if a name is unknown, walks roads reading signs until one names it. Never leaves a road without a text reason. Rests when the journal says a bed or fire is near and hp < 50 % | Quest legibility: the text is enough |
| **The Explorer** | Frontier walk over fog: goes to the nearest unseen screen edge, reads everything, fights what aggroes, picks up what it can. Quests are incidental | Coverage: what is off the path, and whether it pays |
| **The Rusher** | Spine only. Beelines each objective over ground it has seen, sprints, skips side text, never rests until a lock-in or death forces it | The floor: the least a player can do and still get through |
| **The Cautious** | The Reader, plus: retreats at 50 % hp, rests at every fire passed, sleeps every night at a bed, never enters a pocket whose threat it has felt before without a new verb | Harshness is fair to care |
| **The Co-op pair** | Two seats at the 62 / 115 penalty. *Together*: Reader leads, the second follows and heals. *Split*: a Reader and a Rusher on different objectives in different zones, regrouping at the fire on a death | The penalty is survivable together and punishing apart; no lock-in traps one |
| **The Lost** | The Reader, but every 20 real minutes it drops its objective and its own map and rebuilds both from the journal alone (quest log, `returnTo`, what it has read) | The journal is enough to come back after a week away |

Common rules:

- A model sees `View` (`ARCHITECTURE.md` §11): units and props in the camera rect, focus and its verb, hud, dialogue, quests, fog, and `events_for(seat)`. It never reads `GameState`, a blueprint, a prop key or a mark. A lint holds `jane-bot` to `jane_sim::view` and `Command`.
- A model paths over cells it has seen, plus the visible run of a road ahead. It remembers the ground it walked; it does not know the ground it has not.
- A model's **journal** is what a person would have: every `Read`, every dialogue line advanced, every zone banner, every quest change, every toast, with the tick. It is the sim's own journal (`ARCHITECTURE.md` §3.7: authoritative, saved, hashed, and what dialogue conditions read), reached through `View::journal()` and `View::known(fact)`; the model keeps nothing of its own beyond its decisions. The Quests tab is one view of it, the models another, and `jane dossier` a third, all reading the same record, so "what she knows" cannot differ between the game, the bot and the report.
- A model records a **decision** each time it changes objective: which quest, which landmark name, which journal entry told it. That is how a failure says *why* it was walking there.
- Sweeps are seeds × models × hours. Output is a `Trace` (§3), deterministic: same seed, model and build give the same trace, hashed every 600 ticks, equal on every target.

**Rule:** a model may be dumber than a person. It may not be better informed.

**Built (28 September 2026)**, `jane_bot::Model` (`crates/jane-bot/src/lib.rs`, `story.rs`, `lost.rs`, `pair.rs`; the pair's table in `crates/jane-cli/src/pair.rs`). Each new model is the Reader with one thing changed, so the Reader's and the Rusher's decisions are untouched (the bot-session fixture holds). Decided without the owner, and so written down: where a model departs from the table above, it is here.

| Model | As built | Not built yet |
| --- | --- | --- |
| Explorer | Walks to the nearest fog block not yet charted that borders charted ground (`lost::fog_frontier`), at a 30-cell handicap so whatever is within reach comes first; reads, opens and fights as the Reader; a quest's thing only within 48 cells; the county only (from a house or dungeon it walks back out) | It stays out of doors at night with no bed of its own: its deaths are the night's |
| Cautious | Backs off a fight she is losing at half her health, not a third (`Ctx::flee_below`); sits at any fire within 40 cells when under 95 %; sleeps at Julie's bed every night once the house is open, as the Reader does | "Never enters a pocket whose threat it has felt without a new verb": the Reader's tolls on the ground where she died stand in for it |
| Lost | Knows *where* a thing is only once it has been inside the 48 × 27 camera (`lost::Eyes`): a quest's thing, a person, a creature, a trigger's ground, a door into a dungeon. Until then she looks from the step's words. She **reads the signs she passes** (any with words within 10 cells of her, out of doors: a fingerpost at a fork, a roadside post to a place, a footpath's post), and each place one names with a way and a distance ("THE GOLD MINE, NORTH-EAST, 850 m.") is a lead (`lost::Lead`): the road cell nearest where it says the place is. First the lead whose whole name the words say (every word of it: "Hazel Spinney" is not "Poplar Spinney"), the longest name and then the newest read, which she walks to along the roads, turned by any newer sign read on the way or by the thing coming on screen; a lead walked to its end without the thing on screen is spent. Then a fingerpost or signpost she has had on screen within 32 cells and not read, which she goes to and reads. Then (as before the signs were read) the thing she has had on screen whose name, label or lettering shares most words with them (a well for "at the well on the station road"), not yet stood by; then the newest road leaving the screen toward ground never on it (she follows a road to its end before turning back to a fork); then the edge of the map; never within 64 cells of where she died. **Twenty minutes looking for one step and she is told** (the Reader's reading takes over): that is the step failing "from the text alone" (the sweep of 29 September 2026, seeds 1 to 8: 114 give-ups over 39 steps before she read the signs, 65 over 31 after). Every twenty minutes she forgets her screen memory and her objective, and recalls from the journal what stands where each entry was learned (a sign read, a book, a person met) | In a dungeon she plays the Reader's crawl (a dungeon's legibility is C1 to C13's), save that with nothing else to do she walks first to the unseen ground nearest a door whose own name, or a board or notice within 5 cells of it, shares a word with the step's (`crawl::room_words`: a wind kept, the dungeon's own name and "hall", "room" dropped). She does not go through that door before another: which hop comes first is the dungeon's puzzle, and a hop may not come back. A notice that says where a room is without standing by its door is not read for the way. Fires and beds she finds as the Reader does |
| Co-op pair | Host and guest over `jane-net`'s in-memory links, lockstep, the party's penalty. *Together*: a Reader leads (seat 0); seat 1 keeps within 8 cells of her, through the door she took, and otherwise plays as the Reader beside her. *Split*: a Reader and a Rusher, each on its own | Healing (no heal is on her bar); regrouping at the fire after a death; a lock-in followed into |

All models still see the whole zone for fighting, doors (the Lost's looking aside), fires and beds; the lint that holds `jane-bot` to `View` is P4b's.

### L4 Experience metrics from traces

Computed from `.jtr` files, per seed and model, printed as tables with bands per model. Times are real minutes at 60 ticks a second; walking is at 6 cells a second on foot (`QUEST-TREE.md` §1).

| Metric | Definition |
| --- | --- |
| Time to first fight, first death, first rest | Ticks to the first `Damage` to her, first `PlayerDied`, first `Rest` |
| Deaths per hour, by threat | `PlayerDied` per real hour, keyed by the threat of the cell she died on |
| Walk to play ratio | Ticks moving with no objective event in the last 30 s, over ticks in dialogue, fighting, reading or using |
| Backtracking distance | Cells walked over ground walked in the previous ten minutes, not on the way to a hand-in |
| Time lost | Ticks walking *away* from every live objective at once (distance to each rising) |
| Quest legibility | For the Reader: real minutes from taking a step to standing at its place, over the audit's walk for that step (`QUEST-TREE.md` §2, C2 budgets); and whether it got there from the text at all |
| Pacing curve | Events per real minute (fights, reads, pickups, quest changes), by hour of play; the dread, release, dread shape (`DUNGEONS.md` §2.7) inside a dungeon |
| Resource curves | hp, mp, potions, keys, energy sampled every 60 ticks; the minimum and how long it stayed there |
| Nothing-to-see stretches | Runs of ≥ 60 s walking with no unit, no readable, no pickup and no site edge in view; where, how long |
| Night exposure | Ticks outside lamplight between the bell and six; how many of them in the Works |
| Co-op split penalty encounters | Fights fought by one seat alone at 62 %; how many were lost; how far from the other seat |
| Journal size | Entries by kind by hour: what a person is being asked to hold in her head |

A failure names the seed, the model, the tick, the metric and its band, and a `jane film` clip (§3) of the thirty seconds round it.

**Rule:** every L4 metric is defined on the trace alone. Anyone with the `.jtr` and this table can recompute it.

**Built (28 September 2026)**, `jane_bot::experience::measure(trace, seat)`, printed by `jane play --trace` and gathered by `jane sweep`:

| Metric | As built |
| --- | --- |
| Time to each goal | Each quest's given, ready and done, in real minutes (frames: a night slept is not play) |
| Time to first sighting | For each step (and each "Back to ..."), from its quest given to the first `Sight`: the harness knows where the thing is and writes when it first stands on her screen; the model does not know it |
| Time not knowing what to do | Frames alive with no objective, and frames on a search objective (the Lost looking from the words), by hour |
| Backtracking | Cells walked into an 8-cell block walked in the previous ten minutes, not on the way to a hand-in, over cells walked |
| Deaths | Each with what last hurt her, the zone, the region, the cell and the frame: by cause and by hour of play |
| Empty walks | The owner's "the world feels empty", held against `PLAN.md` §2.4 (something visible every 20 to 30 s; a deliberate empty stretch at most two minutes). Runs of 30 s or more walking out of doors (not talking, not fighting) **over ground the map had not charted** (three walking seconds charting nothing end a run: nothing is new on ground walked before, and the first sweep's "emptiest walks" were bots pacing old ground), measured two ways: **nothing new** on screen (no prop with a verb, no creature or person, no patch's edge not seen before), and **no new landmark** (nothing a person would remember the walk by: a thing with words or a label, a door, a fire, a bed, a bench, a person who talks, a patch's edge; herbs, rocks and rabbits do not count). The row above's nothing-to-see (nothing at all on screen) is marked *bare*. Not for a pair: the map is the party's |
| Night exposure | Samples out of doors at night outside any light, before the first morning |
| Walk to play, pacing, journal size | Samples walking over samples talking or fighting; kills, loot and quest changes by hour; journal writes by kind |

Not built: time lost as "walking away from every objective", legibility against the audit's walk budgets (the TS audit's walks are not in the Rust build), resource curves as a table, co-op split-penalty fights (deaths apart are counted), film clips.

### L5 Truth and cohesion

The TS `truth.test.ts` proves a dozen hand-listed claims on three seeds (the key under one of four stones, eleven parcels, milk on the step). This layer makes that the rule for every claim.

**Claims are data.** A text row that says something about the world carries a `claim` field: `{ kind, subject, ... }`. The build (L0) refuses a claim kind it cannot check. Per seed, the checker holds each against the built world; the time-dependent ones against a sim run.

| Claim kind | Example | Checked against |
| --- | --- | --- |
| `beside` | "by the well wall" | within 3 cells of that prop |
| `count` | "eleven parcels" | exactly that many of that thing at that place |
| `at` | "the tin under the front step" | hidden under that pushable, shown when it is pushed, only once asked |
| `behind` | "the wood and iron the mine will want" is in the locked storage room | that room holds those items, that door is the one the key opens |
| `name` | a sign says TOP FIELD | a place of that name within `POSTED_NEAR` (A3) |
| `direction` | "the road that goes north, toward the School" | true on this seed's skeleton |
| `hours` | "the dog is not on the step after nine" | a sim run from 20:50 to 21:10 |
| `promise` | "It is always collected" | the parcel is gone by morning on every trace that stood there |
| `omen` | "No exit either, some nights" | true at the seeded rate; when true, the solver proves the second way out; never on the first walk |
| `moves` | "they shift if you lean on them" | the prop row is pushable and the prompt says so |

**Cohesion across zones (global context).** The county is one place, and a name is one thing everywhere:

| Check | What |
| --- | --- |
| One name, one thing | A landmark name used in the town, in a dialogue, on a sign in the Works and in a quest step resolves to the same `NameId` and stands where each says |
| Rumours are events | A rumour row names an event id; the event happened before the rumour can be heard (a flag, a death, a choice) or is scheduled to (a clock row); a rumour of a thing that never happens on this seed is never offered |
| Tales change the world | Each tale's ending state (the hives in black, the lamp burning green, Mr Rook gone) is what the world shows the next morning, and what anyone who mentions it afterwards says |
| The School | Every line that gives its bearing says north, and it is north of the speaker on this seed |
| Region belonging | Every prop, enemy and material in a region is on that region's table (`PLAN.md` cohesion test 2) |

**What she knows.** The journal is the reference for "what the player knows". A dialogue line, a quest step or a sign that assumes knowledge is checked two ways: statically, every path through the dialogue and quest graph to that line passes a write of the names it uses (a line may introduce a name; then it is the write); and on every trace, at the tick the line was shown, its names were in that seat's journal. A line that assumes what she cannot have been told fails, naming the line and the missing entry. `VOICE.md` Rule 5 is checked the same way in reverse: no journal write ever records whether an omen was true.

**Rule:** a new line that says where or how many or when gets a `claim`, or is written so it cannot be wrong (`QUEST-TREE.md` §2).

**Built (28 September 2026)**, before the claims registry: `jane_bot::audit` reads the English, `jane audit --seed N` prints it, `tests/audit.rs` holds it on seeds 1 to 3 and the sweep reports seeds 1 to 8. For every quest step and every "Back to ...":

- **Where the thing is**: every instance of what completes it on the seed (things holding it, a trigger's rect or a thing that marks the place, units that must fall or that talk it done, whoever takes it back and the marks their hours put them at), in the county or in the dungeon or house the words name;
- **What the words name**: every phrase after a preposition, held against what the county built and a person can see: a prop's name and label, a door's zone, the speaker and the capitals of what a sign says ("CASTLE HALT"), a story place's name, a patch, a site's ground. Not the layout's rects and marks, which nobody sees: an anchor a quest names counts only when something stands at it (the 2026 web build's bug). A name (a capital word) is answered only by something called that; a kind ("the well") by anything of the kind;
- **Reach** by preposition: by, at, beside 10 cells (the truth test's 3, plus a prop's size and a placement's spread); in, on, inside 40; near 48 (A3's two half-screens); from, past, up, along 150 (a route leads from it); a named place's edge allowed 16. A way ("the station road", "the farm track") wants a road or path within 12 cells;
- **Verdicts**: ok; FAR (built, but not within reach); UNBUILT (nothing in the county is called that); OFF-ROAD (more than a half-screen from any road or path, and so is the landmark named: C1); UNPOSTED (a place's own name the words use is built, but written on nothing she can see within 48 cells of the thing: A3); NOTHING (no instance on the seed). Each step also says whether a place's name it uses is written anywhere away from the place by a road (`pointed`: a fingerpost, a board naming where a way goes), which is how the Reader of L3 finds a name it does not know. `tests/audit.rs` enforces no NOTHING and no UNBUILT on seeds 1 to 3, and that it catches a well taken away from the glove; FAR and OFF-ROAD are recorded (§4.4);
- **Omens**: which are true per seed (`jane_sim::omens::is_true`, marked only in the tools), where each claim's key sentence is posted or said, and never two lethal of a region.

Not built: claims as data, the knowledge check, the cohesion table.

### L6 Living-world audits

Long traces (a Cautious and an Explorer for eight real hours each) sampled every game hour.

| Audit | Rule | Sampled |
| --- | --- | --- |
| Schedules | Nobody teleports in view: a unit seen at tick t and t + 30 moved at most its speed × 30. Every named person is somewhere plausible each hour (at home, on patrol, at their place, or gone for a stated reason) | hourly, every named unit |
| Presence | Day-only and night-only things hide and show never within 120 × 80 px of a watcher (`ARCHITECTURE.md` §4.2 step 3) | every tick in view |
| Ecology | Populations recover on their respawn rows; a pocket the player thins stays thin for the respawn time and not longer, and one she clears refills within ten game minutes of falling under its `hold` line; nothing respawns in view; `respawn: 0` stays dead | counts by area every ten game minutes |
| Weather | When weather lands (`PLAN.md` §6): bands per region (mist in the Waters, clear on the first walk); no storm on the first walk; mist never hides a lit lamp inside its reach | hourly |
| Consequences | Every quest outcome visibly changes something (a prop, a person, a door, a light) and the change is journaled; both branches of every choice | per quest on the trace |
| Rumours after the event | A rumour of an event is offered only after its tick, and within a day of it | per rumour |
| Night rule | Between the bell and six: lit roads hold −1, unlit roads lose it, the Works +2, and the sim's `lamps_lit()` agrees with the renderer's | per night |
| The dog after nine | Not on the step from the bell to six on every seed; the step's `talk` is empty then; back by six | per night |
| The bell | Rings at nine on every seed, once, heard in every zone; nightLock doors lock on it and open at six | per night |
| Havens | Nothing spawns or follows inside a hub fence; a rest room has no spawn sockets and nothing paths into it | hourly |

### L7 Human review artefacts

Produced by CI on every merge and nightly; never asserted.

| Artefact | What |
| --- | --- |
| Seed sheets | 24 counties side by side with sites, roads, threat and distances (`jane view`); one dungeon × 24 seeds with node ids, locks, crit path, attempts and failed check names |
| Contact sheets | Every look at 1× and 6× on its real background (`jane sheet`) |
| Film clips | `jane film` renders a trace range through `jane-present` headless: a strip of frames, 4 a second, with the hud, the journal's last line and the model's current decision under each. Clipped automatically at: first fight, first death, each dungeon entry, each rest, each L4 failure |
| Walkthrough dossier | `jane dossier` writes a seed's playthrough as prose from the trace and the journal: what she saw, when, what she was told and by whom, what she did, what was true and what was not (the omens, marked only here, never in the game). One per seed per model. The owner reads the Reader's dossier for seed N instead of playing seed N |
| Claims report | L5's table for a seed: every claim, its kind, pass or fail, what the world held |
| The EXPERIENCE.md report | §5: every claim in `EXPERIENCE.md` with pass and fail per seed and model |

---

## 3. The `Trace` and the `jane` commands

### 3.1 `.jtr`

postcard in an lz4 frame, like a save (`ARCHITECTURE.md` §3.5), so it is small and canonical.

```
Header  { magic JTRC, version, content_hash, build, seed, model, seats, hours, started_clock }
Record  { tick: Tick, seat: Option<Seat>, kind }
  Pos        { zone, pos: Vec2, facing, moving }            every 60 ticks per seat, and on every zone change
  Event      { Event }                                      every sim Event, as routed
  Journal    { entry: Read | Heard | Banner | Quest | Toast, text: TextId, speaker, place }
  Decision   { objective: Quest(QuestId, step) | Explore(screen) | Rest | Regroup | None, because: JournalIx }
  Sample     { hp, mp, energy, potions, keys, threat_here, lit_here, in_view: { units, readables, pickups, sites } }   every 60 ticks
  Hash       { sim_hash }                                   every 600 ticks
Footer  { ticks, records, metrics: [(MetricId, i32)] }
```

**Rule:** a trace is a pure function of `(content_hash, seed, model, seats, hours)`. Two runs differ or the build is wrong; every target must produce the same bytes.

**Built (28 September 2026)**, `jane_sim::trace` (the format and the `Observer`) and `jane_bot::run::Session` (a model's side). The observer holds only `&Sim` after each step, so a traced run steps and hashes as an untraced one (`tests/trace.rs`, which also holds that the same session writes the same bytes and that L4 recomputes from them). As built, against the sketch above: `Pos` is written on each zone change and the cell rides in every `Sample`; `Event` is the subset a metric reads (`trace::Ev`: zone, quest changes, hurt with the row of what hurt her, kills, deaths with the region, rest, learn, loot, consequences, the School's bell); a `Journal` record carries its words, not a `TextId`; a `Decision` carries the words it acts on (the step's text, the "Back to ..."), not a journal index; `Sight` is new (a step's thing first on screen, and how far from her); a `Sample` counts what is new on screen as well as what is there, and how much of the map was charted since the last; the footer's metrics are left to whoever reads the trace. Not sampled: the threat under her (it is the skeleton's, not the View's).

### 3.2 Commands (`jane-cli`)

| Command | Does |
| --- | --- |
| `jane play --model reader --seed N --hours 2 --trace out.jtr` | One model, one seed, headless; writes the trace and prints the L4 table for it |
| `jane play --model pair:split --seed N --hours 2` | Two seats over `Transport::Local` |
| `jane sweep --models all --seeds 1..64 --hours 1 --report` | Every model on every seed in parallel; L4 and L6 tables; the `EXPERIENCE.md` claims report; exit code on any band failure |
| `jane dossier --trace out.jtr` | The walkthrough dossier as Markdown |
| `jane film --trace out.jtr --from T --to T2` and `--clips` | Frame strips; `--clips` renders every automatic clip |
| `jane audit truth --seed N` | L5's claims on the built world and the sim runs the time-dependent ones need |
| `jane audit cohesion --seed N` | L5's cross-zone table and the knowledge check over the dialogue graph |
| `jane audit world --seed N --hours 8` | L6 on a long Cautious and Explorer pair |
| `jane audit experience --seeds 1..8` | Only the `EXPERIENCE.md` report |
| `jane metrics --seed N` | L2 tables for one seed |

All headless, no SDL, in `jane-cli`, so a Pi can run a sweep overnight.

**Built (28 September 2026)**, as they are called now:

| Command | Does |
| --- | --- |
| `jane play --model reader\|rusher\|explorer\|cautious\|lost --seed N --minutes M --trace OUT.jtr` | One model, one seed, from New Game; writes the trace and prints its L4 table (`--l4` prints the table alone) |
| `jane sweep --seeds 1..8 --models all [--minutes 2400] [--short 120] [--out DIR]` | Every model (the five and `pair:together`, `pair:split`) on every seed, in parallel; the story models to an ending or `--minutes`, the Explorer and the pairs `--short`; each trace to `target/sweep/`, and the report to `DIR/README.md` (default `sheets/sweep/`): the quests the Lost could not find, ranked (every step given up on, and a total), with L5's verdict and why; the emptiest walks; deaths by killer, place and hour; every run's L4 row; §4.1's rows per seed; L5's findings and the omens; the pairs. `--report-only` writes the report again from the traces, playing nothing (L4 is a function of the trace) |
| `jane dossier <seed> [--models reader,lost] [--minutes M]` | One seed's walkthrough to `sheets/dossier/seed-N/README.md`: the omens (true or not, marked only here), each model's L4 table, timeline, deaths, what it looked for and for how long, a `--snap` picture every 30 minutes and whenever the Lost gives up, and the seed's L5 findings |
| `jane audit --seed N [--all] [--quest ID]` | L5 on the built world: the steps whose words do not hold (or every step), and the omens; `--quest` reads one quest's steps phrase by phrase |

Not built: `film`, `audit cohesion`, `audit world`, `audit experience`, `metrics`; a sweep's exit code on a band (the bands are the test's, §4.1).

---

## 4. Bands

Starting values, to be tuned against play (§8). Every band is a named row in `data/tuning/verify.json`, never a literal in a test, and a change to one is its own commit with the play session that justified it.

### 4.1 The first hour, on every seed

Clock: New Game at 17:00; a game hour is two real minutes; lamps at 18:30 (three real minutes in); the bell at 21:00 (eight minutes in).

| Claim | Model | Band | Layer |
| --- | --- | --- | --- |
| Julie's gate reached by the station road | Reader | 2 to 4 min | L4 time to place |
| Julie's gate reached | Rusher | 1.5 to 3 min | L4 |
| The lamps come on while she is still on the road, or in the yard | all | 18:30 falls inside the first walk or the yard | L4 + L6 |
| The first fight is the yard skeleton | Reader, Cautious | first `Damage` from `skeleton` inside Julie's fence | L4 |
| No death on the first walk | Reader, Cautious, Rusher | 0 deaths before the stoop; health lost < 15 | L4, `harshness` |
| Nothing lethal on the first walk | all seeds | no threat > 1 cell on the road by day | L1 |
| The first walk is never empty | all seeds | 0 empty screens; > 95 % of stretches with something in view | L2 |
| The house key from the dog, the kitchen entered | Reader | by 8 min | L4 |
| A bed or fire is reachable before the bell | Reader | first rest available (journal knows a bed) before 21:00 | L4 + L5 `hours` |
| The first rest taken | Cautious | before 21:00 | L4 |
| Icebolt learned in the kitchen | Reader | by 12 min | L4 |
| The cellar's rats and the meat | Reader | by 20 min; ≤ 1 death | L4 |
| The dog is gone at nine | all seeds | absent 21:00 to 06:00 | L6 |
| Walk to play ratio, first hour | Reader | 1.0 to 2.5 | L4 |
| Time lost, first hour | Reader | < 6 min; Lost < 12 min | L4 |
| Quest legibility, Lowfields | Reader | reaches the step's place inside 1.5 × the audit's walk on ≥ 95 % of steps; 100 % from the text alone | L4 |
| Night exposure, first night | Reader | < 3 min outside lamplight | L4 |
| Nothing-to-see stretches, first hour | Explorer | none over 2 min; ≤ 3 over 1 min | L4 |
| The School is in every first-hour dossier | all | a lit window north, seen or read, before the bell | L5 + L7 |

### 4.2 A dungeon, on every seed

| Claim | Model | Band | Layer |
| --- | --- | --- | --- |
| Crit path length | built | inside `budget.critPathCells` (C8) | L1 |
| Time on the crit path | Rusher | crit cells ÷ 6 per second, × 1.2 to 2.0 | L4 |
| Time to finish | Reader | Rusher × 1.5 to 3 | L4 |
| Deaths per attempt | Rusher 1 to 4; Reader 0 to 2; Cautious 0 to 1 | | L4 |
| Finishes at the dungeon's phase | Reader, Cautious | 100 % of seeds; Rusher ≥ 90 % | L3 |
| The rest room found before the mini-boss | Reader, Cautious | on ≥ 95 % of seeds; at 40 to 60 % of the crit path (C7) | L4, L1 |
| The verb's first use is in safety | all | no `Damage` in the teach room (C5) | L4 |
| The pacing curve has the shape | Reader | events per minute: rises to the mini-boss, near 0 at the rest room, dips after the verb, highest before the boss | L4 |
| A lock-in can be followed into | Pair, split | the second seat reaches the first inside 60 s of the lock (C6) | L3 |
| Lock-in at 38 % | one seat of four | the template bot test's solo room is won; a cost-8 room is not required | `DUNGEONS.md` §2.7 |
| Every jar and page reachable solo | Reader | 100 % on the Explorer's full walk | L3 |
| A dead end pays | Explorer | every dead end over a screen long yields something | L2 |

### 4.3 The whole story, on every seed

*Built 27 September 2026* (`crates/jane-bot/tests/story.rs`). The Reader from New Game through every act of the spine to the ending its choice policy names (`Ctx::ending`; `jane play --ending hold|hill|train`), the three in turn over the seeds.

| Claim | Model | Band | Layer |
| --- | --- | --- | --- |
| The ending it chose is reached | Reader | seeds 1 to 5; each of the three on at least one seed; inside ten game hours of play (nights slept are not play) | L3 |
| Every act of the spine is done | Reader | the twelve quests from A Letter from Julie to Yours to Say | L3 |
| Nothing the spine needs stays set aside | Reader | a step set aside (`STUCK: <quest> step n`) is done within 90 game minutes | L3 |
| The town notices each act | all seeds | STORY.md §11's rows have fired by the end: `mine_quiet`, `wing_lit`, `forest_quiet`, `works_dark`, `burial_quiet`, `bell_stopped` | L6 |
| A story is a tape | Reader | the run's tape replays to every hash and ends on the run's own | L3 |
| The three endings from the choice | Reader | from Yours to Say (the console's start), each where STORY.md §10 puts it, closing the game with its `the_end` and taking the Ball; the train on a Sunday it stops | L3 |

**Where it stands (28 September 2026, `worktree-agent-ae995c5da38174afe`).** The band holds: `the_reader_reaches_an_ending_on_seeds_1_to_5` passes (seed 1 holds the shield, 2 and 5 put the Ball back in the hill, 3 takes the Sunday train, 4 holds the shield; all twelve spine quests on every seed, the town's six notices fired). From New Game, `jane play --minutes 2400 --deaths`, both models end the game on seeds 1 to 5 and 7: the Reader holding the shield at 321 to 560 minutes of the log's clock with 7 to 33 deaths a run, the Rusher putting the Ball back at 222 to 427 minutes with 10 to 32. The Burial, the wall a day earlier (40 to 204 deaths a run, never finished), now costs 1 to 6 deaths a seed. Every dungeon finishes under the dungeon test (the Burial also on seeds 4, 5, 7 and 11), and every act from the console.

What the deaths were, and what they are now (`--deaths` records the zone, the cell, the killer, her most health, her apples and potions, the hour and what she was doing):

| Seed | Before (27 Sept, `d1a6bcb`) | Now | Most of them now |
| --- | --- | --- | --- |
| 1 | 137+ by minute 1811, locked out of the Burial | 11, holds the shield at 511 | the Burial's Soldier and cactus, 2 each |
| 2 | 194, beaten in the Burial (spiders 57, lurkers 53, the snake 31) | 7, at 366 | the Ringer 4 |
| 3 | 117, held the shield at 798 (the School's guards 54, the Ringer 24) | 33, at 560 | the county's wall spiders 7, the Ringer 7, the Museum's attendant 6 |
| 4 | 40, then idle from minute 294 to the end | 24, at 472 | the Museum's attendant 8 |
| 5 | 108, beaten in the Burial (lurkers 51, cactus 32) | 14, at 377 | the county 7 |
| 7 | 204, beaten in the Burial (spiders 146) | 11, at 322 | the Ringer 3 |

Nearly all of it was the bot, not the world: she came to the Burial with one bait, not two (the "reached, and it did not count" rule set the brewing aside, and a full bag threw the makings out), so the second lurker kept its chest, the glasshouse key and the ways round shut; she ate her apples on the trash and met the Spider with none, or kept every apple and fell in the rat room with thirteen; she stood in Goldskin's corner; she stepped from one safe cell to the next out of a cactus's fire for hours. And three loops that looked like play: a craft whose output could not come off the bench counted as made; two plans over the county's blocks walked her back and forth for thirty hours; the Seam's near side was across a wall, so she went out of the Mine and in again every tick. The Burial's numbers were not softened (DUNGEONS.md §3.5 says why each is fair).

What is left: most deaths out of doors are with no apple in the bag (the county's apples do not come again, and the pantry holds none: a renewable food is a content decision, not taken here); the Museum's attendant and the School's Ringer are the dearest keepers now; a bag fills with what destroy refuses (the keys of places done, twenty gold bars), and only a hand-in empties it.

The whole-story runs are the slow tier (§6; `#[ignore = "slow: …"]`, a minute or more of release time a seed): `cargo test --release -p jane-bot --test story -- --ignored`. The three endings from the choice run in the fast tier, always.

How the story bot plays it (`crates/jane-bot/src/story.rs`): a quest step in a dungeon is played whole by the crawl (in by its door, through its locks and verbs to its boss, out again); a door that keeps hours is come back to when it opens; out of doors from eight in the evening she goes home to Julie's bed and sleeps till six (the first thing the county teaches); whoever takes a quest back and keeps hours is looked for where the hours put them; a door shut from outside is gone round by the zone she has seen a door from, or the one the log names; a far goal is planned over 16-cell blocks first (`coarse.rs`); a bag nearly full throws out what can be found again. To look at an act without playing the ones before, `jane play --from ACT[+]` puts a new game at it (for looking, never a tape), and `--explain`, `--profile` and `--show-prop KEY` say what the bot holds, where the time goes and what the ground round a prop is.

---

## 5. `EXPERIENCE.md` and this file

`EXPERIENCE.md` is the specification of what happens: minute by minute for the first hour, hour by hour for the game, and at each point what she knows (the journal she should have), what she has been told, which of it is true, what she can see from where she stands, and what she is probably thinking. It is `QUEST-TREE.md` §5 to §10 widened to the whole experience and written before the world, not after.

Every claim in it is tagged with the layer and metric that holds it:

```
By 18:30 the lamps have come on and she is on the road or in the yard.       [L4:time_to_place gate] [L6:lamps]
The dog's first line names the bones on the fence line, and there they are.   [L5:claim beside] [L4:first_fight]
She has been told about the School once, by the well, and not its name.       [L5:knows school] [L7:dossier]
```

`jane sweep --report` parses the tags, runs or looks up each check, and prints `EXPERIENCE.md`'s claims in order with pass and fail per seed and model. That report is the artefact the owner reads at a phase gate.

**Rule:** a claim without a check is a wish; a check without a claim is noise. Each file lists the other's orphans at the bottom of the report, and either is a failure at a gate.

**Rule:** `EXPERIENCE.md` is written in the player's tense and the world's words; this file is written in the harness's. Neither describes code.

---

## 6. CI shape

**Two tiers of `cargo test`.** The suite is cut by wall time, not by layer, so that the run a change is checked against on every save stays short:

| Tier | Command | What runs | Time | When |
| --- | --- | --- | --- | --- |
| Fast | `cargo test --workspace` | Unit tests and short checks: L0; the L1 and L2 sweeps (`SEEDS`, 64 by default, every one under fifteen seconds); the bot's first minutes, replay and save-form round trips, the L5 audit, the trace, the three endings from the choice; art, present and render checks | about three minutes in a dev build | Every change, before every push |
| Slow | `cargo test --release --workspace -- --ignored` | Everything marked `#[ignore = "slow: …"]`: every dungeon crawl on three seeds (§4.2), the first hour's bands (§4.1, L4), the whole story on five seeds (§4.3), the two-hour late-game budget, the three-process `serve`, the T1 albedo gate, the timing prints | about five minutes in release | Before a merge, and nightly |

**Rule:** a test that plays more than a few game minutes, or takes more than about twenty seconds in a dev build, is `#[ignore = "slow: <why>"]`; the fast tier never waits on a whole game. `-- --include-ignored` runs both tiers in one go.

| When | Layers | Seeds × models | Time | Blocks |
| --- | --- | --- | --- | --- |
| Every merge | Both tiers: L0; L1 (64 seeds); L2 tables and floors; L3 Reader and Rusher on 8 seeds for 1 hour each; L4 on those traces; L5 static on 8 seeds | 8 × 2 | < 10 min | The merge |
| Nightly | L3 to L6 on 64 seeds × all models (1 hour each; Cautious and Explorer 8 hours on 8 of them); every film clip; every dossier; the `EXPERIENCE.md` report | 64 × 7 | hours, on the Pi rack if it exists | The next day's work, by the owner reading the report |
| Weekly soak | L1 and L2 on 1000 seeds; L3 to L6 on 256 seeds × all models; L1 on every dungeon 1000 seeds | 256 × 7 | a weekend | A phase gate |
| Cross-target | Trace bytes equal across x86_64, i686, aarch64, armv7 for 4 seeds × Reader | | with `determinism` | The merge, from P4 |

What blocks each `PORT.md` §7 gate:

| Gate | Needs green |
| --- | --- |
| P1 | L0 whole, the claims registry parsing |
| P2 | L1 skeleton and county; L2 world tables; L5 `truth` and `cohesion` static; seed sheets |
| P3 | L1 dungeons and templates; L2 dungeon bands |
| P4 | L3 Reader and Rusher on 8 seeds, first five minutes and every dungeon; trace determinism; L4 tables printed (bands recorded, not enforced) |
| P6 | The owner plays the first hour on a seed and the Reader's dossier for that seed matches the owner's notes (§8); L4 first-hour bands enforced |
| P7 | The Pair models over `Transport::Local`; the co-op bands |
| P8 | The Pair over the wire, an hour without desync, traces equal |
| M3 to M7 (content, on the Rust build) | The nightly green for a week on the region's seeds before the owner is asked to play it |

**Rule:** a band is enforced only after it has been recorded on 64 seeds and read by the owner. Recorded first, enforced second, tuned from play third.

---

## 7. What carries, what is new

| From the TS build | Carries as | Notes |
| --- | --- | --- |
| `quest-audit.test.ts` (A1 to E1, walk budgets, the seven broken quests) | L2 quest rows and L0 text rules; the broken-quest test carries whole | The audit proves she *can* find it; L4 proves the Reader *does* |
| `truth.test.ts` (beside, counts, hidden until asked, looks like what it is, signs name no house) | L5 as claim kinds, over every text row instead of a dozen | |
| `harshness.test.ts` (the duel table, the county crossing table, on and off road, first walk safe) | L2 floors and L4's harness models (health held up, the instrument that does not die) | The arena duel stays: it is the one place a number is forced |
| `density.test.ts` (empty screens, first walk, in view) | L2 floors | |
| `bot.ts` (walk, prop, push, talk, fight, start in a zone) | The `jane-bot` harness, for template and dungeon proofs | Not a model: it reads keys |
| `tales.test.ts`, `stories`, `locked-words`, `town` | L1 reachability and L5 claims | |
| `PORT.md` §9.3 determinism gates | L1 | Traces added as gate 7 |
| The seed viewer | L7 seed sheets | |

| New | Where |
| --- | --- |
| Player models over `View` and a journal | L3 |
| The trace format and `jane play`, `sweep`, `dossier`, `film`, `audit` | §3 |
| Experience metrics with bands per model | L4 |
| Journal-based legibility (from the text alone) and the Lost model | L3, L4 |
| Claims as data, over every text row | L0, L5 |
| Cohesion across zones: one name one thing, rumours are events, tales change the world | L5 |
| Knowledge: a line assumes only what the journal holds | L5 |
| Living-world audits: schedules, ecology, weather, consequences, the night rule, the dog | L6 |
| Dossiers and film clips | L7 |
| The `EXPERIENCE.md` report and the orphan rule | §5 |
| Bands as tuning data with a play session behind each change | §4, §8 |

---

## 8. Risks

| Risk | What it looks like | Mitigation |
| --- | --- | --- |
| A metric rewards the wrong thing | Walk-to-play tuned down by scattering junk; "nothing to see" satisfied by a rock | L2's interactive rule counts only things with a verb or a claim; the Explorer's dead-end payoff wants a jar, a page, a chest, a sign or a view; the owner reads dossiers, not tables, at gates |
| Bots better than people | The Reader never misreads; it matches names exactly | The Lost model forgets on purpose; landmark matching is by the words on the sign, not the `NameId`; a legibility band is set from the owner's own time on the step, not the Reader's |
| Bots worse than people | The Rusher dies where a person would step back | Bands per model, not one band; the Cautious exists so a harsh number is checked against a careful player too; a band both the Rusher and the Cautious fail is a world bug, one only the Rusher fails is a Rusher bug |
| Bands tuned to the bots | The numbers settle where the models happen to land | **The owner's play sessions are recorded as traces** (the app writes a `.jtr` on request); bands come from those traces first and the models are held to them, never the reverse; a band that no owner trace supports is marked *provisional* in the table |
| The dossier is not the experience | Prose from a trace misses what a screen felt like | Film clips beside the dossier at every highlight; the P6 gate compares the dossier to the owner's notes from the same seed and any mismatch is a dossier bug first |
| Claims rot | A line is rewritten and its `claim` is not | L0 refuses a claim whose subject no longer exists; L5 fails a text row that names a place, count or hour and carries no claim |
| Sweeps take too long | Nightly runs into the morning | Seeds are the dial (`SEEDS`), hours the other; every-merge stays under ten minutes; a Pi rack runs the nightly if the build machine is busy; nothing here touches the sim's performance rows |
| The stack proves the game is correct and it is still not fun | Every band green, the owner bored | That is the case the bands cannot catch and the reason the owner plays every milestone (`PLAN.md` §7). When it happens the session is recorded, the dossier is read against it, and the property that was missing becomes a new metric with a band |

**Rule:** a band is changed by a play session, not by a failing sweep.
