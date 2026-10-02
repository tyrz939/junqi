# Jane: the World

The living county. Written 26 September 2026 from John's direction: *"the world needs to breathe and live and be cohesive in a global context."* Pair with `PLAN.md` (the agreement), `STORY.md` (what is true), `VOICE.md` (how Castle talks), `QUESTS.md` and `QUEST-TREE.md` (what she is asked), `EXPERIENCE.md` (what she meets, minute by minute), `ARCHITECTURE.md` §3.7 and §4.6 (the engine hooks this doc leans on: **journal and known facts, schedules, weather, ecology, consequences, rumours**) and `VERIFICATION.md` (the checks: L5 truth and cohesion, L6 the living world).

**Status: design for the Rust build.** Where a row or a check is named, it is the row or check that must exist for the section to be true. Proposals are marked *proposed*; John decides them.

**Built so far (the engine, P4, `jane_sim::living`), each with a small row of shipped content:** §5.2's weather table as `data/weather.json` (a sky per region, the first walk held clear; in the county the sky over her is the region under her feet, by the skeleton's region grid); the rain ramp, one per region, and the douse rule (a campfire's light goes out at `douse` 160 in its own region's rain and comes back when that ramp falls under it, not at six); `data/ecology.json` for the allotments, the Top Field and Quarry Steps (pressure, `cap`, `hold`, recovery every ten game minutes); `data/consequences.json` with B1's "rats at four, not eight", Julie's Kitchen (her door night-locked, a lock she holds the key to) and the mine going quiet when Iron Knuckles falls (a spawn by the Company notice that waits for her to come back up); a bed's night lived through (§2.1); Mr Cobb's hours as a schedule (§3.1: in the Arms after the bell, the yard in the morning, his seat by day); one rumour (Mr Ames's spectacles, heard at No. 3, Pound Lane half a day later, the door speaking for whoever is behind it). *Built 26 September 2026, the story pass:* **the town's news** (a consequence row's `spreads`, §7.2: the six spine bosses and the allotment sheds reach the people who would hear first, in order), the county's edits for each act (§6), the bell stopping when the Timekeeper falls (§2.1), people who move when something has happened (a schedule row's `flag`, §3), and the three endings (`STORY.md` §10). The rest of this document is still design.

---

## 1. The rule

**Rule: everything in the world is somewhere, doing something, for a reason the player could find out.** Nothing exists only when it is looked at.

The state tree keeps every zone whether or not anyone stands in it (`ARCHITECTURE.md`). That is the floor. This document is about what *fills* that state over time: where a person is at four in the afternoon, what the rats have eaten, which road is wet, which lamp went out last night and who noticed.

Four tests, added to `PLAN.md`'s cohesion test for anything that moves:

1. **It has a where.** At any hour a row can say where it is and the map could show it.
2. **It has a why.** Its presence follows from something she can read, hear or see.
3. **It leaves a trace.** When it changes the county, the change is visible afterwards, and the journal holds it.
4. **It is the same for everyone.** Two seats in co-op, or one seat an hour later, meet the same world.

---

## 2. Time

### 2.1 The clock

One game hour is two real minutes. A day is 48 real minutes; the night is 18 of them.

| Clock | Real minutes after arrival | What happens, everywhere |
| ---: | ---: | --- |
| 17:00 | 0 | She steps off the Sunday train. Sunset begins |
| 18:00 | 2 | Church bell, evensong. The town only; it is not *the* bell |
| 18:30 | 3 | Lamps come on where lamps work. The School's window is already lit |
| 20:50 | 7.7 | Tuesdays, the bell *if that omen is true* (`omen:early_bell`): the same bell, ten minutes early, and not again at nine. The night keeps its hour |
| 21:00 | 8 | **The bell.** (Once the Timekeeper is down: "Nine o'clock. No bell.", and the night all the same; `bell_stopped`, `nine_silent`.) Night. `nightLock` doors shut. The dog is not on the step. The night shift is up on the roads' unlit edges and the rough ground (§4.3); the lamps are the safe line. The Works' wicket opens for the night shift |
| 22:00 | 10 | The east road's lamps go out *if that omen is true* (`omen:east_lamps`: the road from Castle over the river to the Museum, whose lamps are a row of their own, `lamp_east`; lit again for the evening at 18:00). The bridge's two lamps and the Museum's are not the run |
| 05:00 | 24 | First light. Fen mist thickest. The mine's door, if barred (`omen:mine_no_exit`), is open again |
| 06:00 | 26 | **The bell again.** (Once the Timekeeper is down: "Six o'clock, and no bell.") Day. Doors open. The dog is on the step (or at the top of Church Lane, the School's morning). The night shift is gone |
| 07:00 | 28 | Milk round (the Milkman, seven to nine). Farms turn out |
| 12:00 | 38 | Noon: the statue in the lake faces the way it faces by day |
| 16:00 | 46 | Second post. The last safe hour to start a long walk |

**Rule: the bell is heard in every zone, including dungeons, at nine and at six.** It is the one signal the whole county shares, and the one clock text may rely on, give or take the early bell: on a seed where Mrs Fenn is right it rings at ten to nine on a Tuesday, and the night still comes at nine. The train's whistle on a Sunday at five is heard the same way, everywhere. **Clock rows keep an hour and a minute on a ten-minute mark, and a weekday may be asked** (`clock.json`'s `minute`, the `weekday` condition; day 0 of the engine, WORLD's day 1, is a Sunday).

**Rule: a bed sleeps the clock to 06:00 and no other hour,** and only two seats written to say otherwise sleep to another: the School's far bed (its card: woken at the bell) and the bench on the Museum's steps (until ten, when the doors open). A fire rests but does not sleep. Nothing else moves the clock. **The night she sleeps is lived:** the weather turns on every hour of it, the ground wets and dries, the patches refill every ten minutes, what was due to stand up stands up, and what the county was going to do it does, exactly as if she had sat up all night by the bed. Only nobody walks about while she sleeps (`ARCHITECTURE.md` §4.6.f).

### 2.2 The shape of a day

| Hour | The town (Castle) | The Halt | The farms and cottages | The Waters | The Works |
| --- | --- | --- | --- | --- | --- |
| 06 to 07 | Bell. Doors unlock. The sweeper takes the leaves off the square | Fire burning. Nobody | Hens out. The farmer at the pump | Reedcutters at the fire, tea | The canteen hatch opens |
| 07 to 09 | Milk round, High Street then Pound Lane. No. 7's bottles stay | | Sheep turned out. Woodcutters to the clearings | Cutters go into the beds | Nobody. The rota says day shift; nobody comes |
| 09 to 12 | Shops open. Mr Cobb takes his seat outside the Arms | | Field work. The farmer on the track | Cutting. Hut doors open. The Museum opens at ten | Watchmen's huts empty by day |
| 12 to 14 | Dinner. Miss Orme sits where she can see the Doctor's | | Doors shut for dinner (knock lines change) | Cutters at the fire | Five tins put out at the hatch. Nobody comes |
| 14 to 16 | Afternoon. Children out until tea | | | Cutters back in | |
| 16 to 17 | Second post at four. Tea. Children in | | Hens in. Sheep penned | The Museum shuts at four. Cutters out of the water by five | |
| 17 to 18:30 | Evening. People at doors, talking about the lamps | The train (Sundays only) | The farmer's last look at the scarecrows | Cutters stack the day's reed | The hatch closes |
| 18:30 to 21 | Lamps. People go in, street by street, north end first | | Doors barred, "at nine, everybody along here does" | Fire built up. Nobody past the landing | Hut fires lit by whoever is there |
| 21 to 06 | Bell. Streets empty. Windows lit; some go dark at midnight. The Arms lit till it is light | Fire. The parcel, some nights | Dark. One window at Whinmoor. Mr Pollard walks at ten | Mist. Something on the far bank | The night shift on Cinder Walk. The Factory's lights on |

**Rule: a person absent from her usual place is somewhere else on the map, and a schedule slot says where.** "Not in" is a location (`Inside(prop)` or `Mark`), not an absence; `Absent` is reserved for the dog and the sheep.

### 2.3 The calendar

Days count from arrival. Arrival is a Sunday, day 1.

| Day | What is different |
| ---: | --- |
| 1, Sunday | Arrival at 17:00. Julie's bed unslept. The trunk on the platform |
| 2, Monday | The first morning. The blanket line. The town hears of "the girl off the Sunday train" (§7) |
| 3, Tuesday | Mrs Fenn's day: the bell goes early *if that omen is true* (20:50, every Tuesday, `omen:early_bell`). The sweeper finds salt again |
| 4, Wednesday | Market. The farmer in Castle. Whatever she has done by now is known at the Arms |
| 7, Saturday | The town shuts early. Mr Cobb does not sit out |
| 8, Sunday | **The train.** 17:00: a whistle as it comes in and another as it goes, and the Sunday sacks on the platform until Monday morning. The Lost Property dates fall due. From the choice on, any Sunday the train stops: she signals at the name board and is on the platform at five, and it stands there till ten past (`train_in`; `STORY.md` §10) |
| 15, 22, ... | Every Sunday, the train. "Does not always stop" (pencilled on the timetable at the Halt) is an omen: when true (`omen:train_through`), it runs through one Sunday in three, the first after she came (day 8) and every third after (29, 50, ...): one whistle, and no sacks |

**Rule: nothing in the calendar is required by the spine.** A day-keyed event is texture, a rumour trigger, or an omen's payoff. A player who sleeps through everything loses nothing she needs.

### 2.4 Seasons *(later, optional)*

Autumn is the fixed season: the tortoise walks north, leaves on the square, the mist. A slow drift over 28 days (the palette one step colder, mist weight up by a tenth a week, first frost on day 21) is a season column in `weather.json`, not a system. Not in any milestone yet.

---

## 3. People

**Rule: every named person has a home, a work place, a route between them, a schedule row, and something they know.** Nobody is a vending machine. The rows: `schedule` (a slot per hour band: `Mark`, `Patrol`, `Inside`, `Absent`), `tells` (a line that teaches a fact), `spreads` (who hears of a story, after how long; §7).

Where a person stands is a place the generator guarantees (a site, an anchor, a chunk key, a story place); the route is the road or path between two of those. A person met off their route is a bug or a story.

### 3.1 The town: Castle

The town is a set chunk: the square (fountain, memorial, Mr Tolly's bench, the parish board, the fire), High Street, Church Lane up to the church and churchyard, Cross Lane (the Forge), Back Lane (the orchard behind), Pound Lane (the Doctor's, No. 7 with the milk), the Castle Arms, Castle Stores, the post office, the green behind Pound Lane. The names are the dialogue rows' names; the streets are the chunk's.

| Who | Home (their `Inside` slot after the bell) | By day | Route | Tells (true when) |
| --- | --- | --- | --- | --- |
| Mr Cobb | The Castle Arms, till it is light | The Arms yard from six; his seat outside the Arms from nine to the bell | The yard to his seat, on foot | "I sit out here till the bell. Mrs Garland likes the yard kept" (always). "After nine she'll let you in, but she likes you to stay till it's light. Most do" |
| Mrs Garland | The Castle Arms | Behind the bar, all hours; door open till it is light | None | Who has been in. Ernest Dunn's whereabouts |
| Mr Dunn (Walter) | No. 9, Pound Lane | By the telephone box on the square, waiting for his brother to ring on the hour; after the Back Room, watching the Arms door | None | His brother Ernest is chapel and has never been in a public house (the telephone says otherwise) |
| Ernest Dunn | The Arms' back room, since the spring, "the night of the bell" (a date the town shares, §8) | Never seen. His tray comes back empty | None | Through Mrs Garland and his brother. "He's been ringing me from across the square" |
| Mrs Crewe | No. 2, Cross Lane | Her seed stall | None | Julie had "the lot" off her (pansies, nasturtiums, lilies) and never said what for |
| The Milkman | No. 4, Back Lane, by the dairy yard | His round, 07:00 to 09:00, behind Pound Lane ("I'm round from seven, and the milk's on every step by nine"); in the rest of the day | The round, in order | No. 7's three bottles (always: the bottles are placed). Which doors took milk (from day 2) |
| Miss Dray | The post office | Outside it from 09:00 to 18:00, waiting for the second post at four (the door: OPEN NINE TILL HALF PAST FIVE) | None | The three mis-numbered letters. The Sunday sacks (day 8) |
| Mrs Tace | No. 6, Cross Lane: the door on the latch "for somebody else" | Her step; from the first night with no bell, the top of Church Lane (`church_lane_top`, a row with `flag: nine_silent`), to see them come down | Step, or the lane up to the north gate | The School's window, "always the same one lit", from the top of Church Lane (always: a fixed light) |
| Mrs Fenn | No. 1, Back Lane, where a clock strikes the quarter twice | Her door | None | "In before the lamps." The bell going early on Tuesday, checked against her kitchen clock (she says it from day 4 on every seed; it rang at ten to nine on the Tuesdays of a third of seeds, `omen:early_bell`) |
| Tilly | No. 1, Pound Lane | The square | Home to the fountain | Sixpence in the churchyard. After: at the lamps he sits in her window facing the hill; by day he sits by the fountain with his back to it (`sixpence_home`) |
| Mr Tolly | No. 3, High Street | His bench in the square | Door to bench | The memorial's last name, cut sharper (always) |
| Dr Vane | The Doctor's, Pound Lane | Out in the lane by his door from 12:00 to 20:00 ("in the lane most afternoons"); the surgery never opens | None | Nobody has been ill since the spring. He fills a vial from the tap. After Counted Out, told the cases had trades on them: Julie had his list of everyone he signed fit |
| Miss Orme | No. 3, Pound Lane, next door to the Doctor's | The bench by the Doctor's with Mr Lyle, 13:00 to 19:00 ("since lunch"; he walks her home at the lamps) | Door to bench | What Mr Lyle says about the bell, and that she has heard both |
| Mr Lyle | A room at the Arms | Beside Miss Orme, 13:00 to 19:00 | The Arms to the bench | By report, through Miss Orme: "the bell at nine is the church." Wrong, checkably: the church rings at six, for evensong |
| Mr Hale | Over the Forge, Cross Lane | The Forge door, open a crack | None | Robert. HALE, R. on the memorial (the name is cut) |
| The constable, Mr Ince | The Castle Arms, after the bell | The constable walks the High Street end to end (from the evening the Museum's lamp is first lit, he stands at the east end from six till the bell, looking at it); Mr Ince reads Sunday's paper | The street | A hundred and twelve paces, then more. The piece on page four about the east road's lamps |
| Mrs Hobb, Mrs Marsh, Mrs Oddie, Mrs Wick, Mrs Bex, Mr Pound, Mr Sallis, the sweeper, Nell, Dot, Robin | Their doors: Mrs Hobb No. 7 and Mr Sallis No. 1, High Street (the orchard behind, and the card IN THE GARDEN); Mrs Marsh No. 5, High Street (ROUND AT THE LINE); Mrs Oddie No. 3, Mrs Wick No. 5 and Mrs Bex No. 6, Back Lane; Mr Pound over Castle Stores; the sweeper and Dot No. 13, Pound Lane; Nell and Robin No. 9, Cross Lane (the two voices and their mum) | Doors, steps, stalls and the square by day; in by nine | Door to the square and back | One true thing and one false thing each (`STORY.md` §5.1). The sweeper's salt. The street "takes longer" after four |
| A Woman with a Case | A room at the Arms, since Sunday | By the post office, where the letters go on Sunday's train | None | On the Sunday she came, whether the train has been; from the first morning, "Is it Sunday?" and "I have been here since Sunday. It has not been Sunday since" (a clock row counts mornings) |
| Mr Quill, Mr Ennis | The Castle Arms, all hours | The tap room | None | The Arms' regulars |
| The ginger cat | Nowhere: it keeps no hours | Cross Lane along the south side of the square, lamp to lamp, sitting at each end as long as it sat last time | End to end | Its line is its route (the chunk's waypoints: 34 to 69, 900 and 600 ticks) |

*Built 26 September 2026:* every townsperson above has a schedule row, `Inside` their door when they go in (the dog alone is `Absent`). The hours, out / in: Mr Cobb the yard 06, his seat 09, the Arms 21; the sweeper and the constable 06 to 21; Mr Dunn and Mrs Tace 07 to 21; Mr Sallis, Mr Hale 07 to 19; Mrs Hobb 07 to 20; Mrs Marsh 07 to 18; Mrs Oddie 07 to 17; the Milkman 07 to 09; Mr Tolly 08 to 20; Mrs Wick 08 to 19; Mrs Fenn, Mr Pound, Mrs Crewe, the children (Tilly, Nell, Robin, Dot) 08 to 18; Miss Dray, the woman with the case 09 to 18; Mr Ince 09 to 20; Mrs Bex 09 to 17; Dr Vane 12 to 20; Miss Orme and Mr Lyle 13 to 19. **Nobody is out after the bell.**

**The hours move a little.** A row's `vary` moves its hours by up to that many minutes either way, in fives, one draw a day from `Step::SimHours` keyed by the person (Miss Orme and Mr Lyle draw together, `varyWith`, so he still walks her home). Twenty minutes for most, fifteen where a line pins the hour closer (Mrs Fenn, Mr Dunn, Mrs Tace, the Milkman), none for the sweeper and the constable, who keep the bell. It is never saved: the draw is the seed's, the day's and the person's. It never crosses a bell: from nine to six everybody keeps the world's hour, so "in by nine" is always true, "since lunch" is 12:40 to 13:20, and "at the lamps" (18:30) is 18:40 to 19:20. Clock minutes land with the omens' work; until then the offset moves the clock the row is read at, in ticks. No door's knock line says anything its tenant contradicts. The Halt has nobody (§3.2): the woman with the case waits in Castle.

**The doors.** A numbered door is a person even when nobody is named: `No. 3, Pound Lane` has a knock line by day, another after the bell, a `nightLock`, and an `Inside` slot that says who is behind it.

### 3.2 The Halt

**Rule: the Halt is never given a person.** The office is not staffed; the book, the trunk, the timetable and the crate tell it. The train is on the west fence on Sundays at 17:00, and the crate says whether it stopped. It is the first place she learns that in Castle the paperwork has outlived the people.

### 3.3 The farms and the open country

| Who | Home | By day | Route | Tells |
| --- | --- | --- | --- | --- |
| The Farmer (Lowfield Farm) | The farmhouse, behind the door | The pump 06:00; the farm track 09:00 to 12:00; Castle on Wednesday; the scarecrows at 17:00 | House, track, rim of the Top Field, back | The scarecrow count. The Top Field's turnips (they are not). The back kitchen faces away from the School |
| Mrs Allen | Her cottage by Sallow Bottom | Cottage by day; her sister's in Castle after 16:00 | Cottage to Castle square, by road | The nurse came on a Tuesday. The dressing. "Do not tell me about the light" |
| Mr Pike | Pike's cottage by the Long Hedge | In | None | The footpath is used at night |
| The Misses Crane | Crane's cottage under Quarry Steps | In; eleven parcels on the step | None | The parcels came by the carter. The carter stopped coming |
| The nurse, the carter, A. Pell | Nowhere. Her car, coat and case; his cart, note and key; a stone by the last lit lamp | | | Her round; where the horse would not go; fourteen lamps |
| The Tenant of Plot 9 | The allotments, after nine | | The shed to Plot 9 | Nothing. He digs |
| Country farmers, wives, old men, grey coats, woodcutters, the innkeeper | Farmsteads, hamlets, cottages, clearings, the Halfway House | Fences, plots, stumps, the trough, by the hour above | Home to field to road's edge | The turned-over lines (`VOICE.md` §People rule 6): the weather, the bell, the barn card, the creatures at the road's edge, the crows |
| The 27 country-story households (Ames to Ruck) | `stories/lowfields.json` places | As their story says | | Their own lost thing; who lives next door; the ruin's candles |
| Miss Hanney, the Pollards, Mr Pargeter, Mr Coker | Coram's, Whinmoor, Sundial Cottage, Coker's Wall | As their tale says (`QUEST-TREE.md` §10) | Mr Pollard: the Dole Stone at 22:00 | The bees, Tom at the School, north, "since they all got up" |

### 3.4 The Waters

| Who | Home | By day | Route | Tells |
| --- | --- | --- | --- | --- |
| The Reedcutters (four coats, one hook empty) | The Reedcutters' fire | The beds 07:00 to 17:00; the fire otherwise | Fire, landing, beds | The eels. The fifth coat. The far bank after dark |
| The tollkeeper | Absent. The toll board, the sunk boat | | | Who crossed, and how many times |
| Mr Rook and Mary | Sluice Cottage | He outside barefoot, she behind boards | None | Each other. Sunday |
| Mr Denholm | The Intake | The pump, in his helmet | None | The knocking at the grille, "three and a rest and three" |
| Mrs Wakes | Tenter Cottage | Her chair | None | Everyone who came to the door, in wool |
| A Reedcutter (country) | Reed huts | The hut, the landing | | The water line. What the reeds hide |

### 3.5 The Works

| Who | Home | By day | Route | Tells |
| --- | --- | --- | --- | --- |
| Whoever is behind the canteen hatch | The canteen | The hatch 06:00 to 18:30; five tins at noon | None seen | The night shift. The rota. Who has not come for dinner |
| The Voyseys | Brickfield Halt | He at the lamps; she by the bench with her case | None | Red and green. Next Sunday |
| Mr Sorrell | The Weighbridge | Round the walls with his pole | The walls to the hatch | The vermin. What they were for |
| The foreman | Nowhere. His diary, three pages | | | What the Works ran on |
| The night shift | The cooling yard, since the shift ended | | Cinder Walk after the bell (§4.4) | Nothing. They are what was outside |
| The Headmaster, the clerk, Iron Knuckles | The mine, since the children went down | | Inside only | The register. The keys. The pay |

### 3.6 The dog

| Hour | Slot | Tells |
| --- | --- | --- |
| 06:00 to 21:00 | `Mark` `dogs_step`: Julie's step, the left-hand board | The spine. The poke chain. "It is in your log. Check it" |
| 16:00 to 21:00, while Counted Out is in her log | `Mark` `dog_museum`: the Museum's steps | Takes the forest back, nearer than the step. Its offer and the quest's returnTo say so |
| 18:00 to 21:00, while Not Relieved or Under the Stone is in her log | `Mark` `dog_graveyard`: inside the graveyard gate | Takes the Factory and the Burial back |
| 06:00 to 12:00, while The Bell at Nine is in her log | `Mark` `dog_church_lane`: the top of Church Lane, by the north gate | Takes the School back, the morning after |
| 06:00 to 21:00, the Sunday she signals the train (`flag: train_signalled`) | `Mark` `dog_halt`: the platform at Castle Halt | Sees her off (`STORY.md` §10) |
| Every hour, once the Ball is back in the hill (`flag: night_gone`) | `Absent` | Gone with the night |
| 21:00 to 06:00, once the key is given | `Absent`. Nobody sees it leave (the presence rule: nothing hides in view) | Nothing. The step is empty; the house is not. On the first night it waits on the step whatever the hour |

Outside a meeting's hours the dog is on the step, and never in two places. Each meeting is a schedule row with `while` (built 26 September 2026); the spine test walks there and finds it.

**Rule: the dog is the one exception to §1.** Its night slot is `Absent` with no mark, and it has no `spreads` row ever. Nobody in Castle has seen Julie and the dog together; no `tells` row may say otherwise.

---

## 4. Creatures and ecology

`ecology.json`: per area (or terrain band), a population per unit def with a `cap`, a `recover` amount every ten game minutes, a `hold` line, a day place and a night place, and `hunts` / `flees`. Threat is the field from `PLAN.md` §2.6; ecology decides *what* is in a pocket, threat decides what it costs.

### 4.1 Populations by area

| Region | Area (threat) | By day | After the bell | Prey |
| --- | --- | --- | --- | --- |
| Lowfields | Roads, fields (1) | Rabbits, hens, sheep, crows | Skeletons at the road's edge, never on a lit road | Rabbits, hens |
| | Allotments (2) | Rats x8 | Rats, the Tenant | Seed, hens' eggs |
| | Long Hedge (2) | Crows, rats, one bandit camp | Skeletons | Rabbits |
| | Sallow Bottom (2) | Rats, marsh birds | Bats. A soldier in Julie's cellar *if the rose omen is true and a rose is picked after dark*, here or there | |
| | Top Field (3) | Pumpkins x7 | Pumpkins, and the scarecrow closer *if true* | Nothing. They eat the field |
| | Behind the car (3) | Spiders, crows | Spiders, bats | Rabbits |
| | Quarry Steps (3) | Quarrymen x6, in gang order | Quarrymen, bats from the adit | |
| | Story camps | Ruffians x3, or skeleton, crow, spider x3 | Same, awake | What they took |
| Waters | Tollhouse Reach (2) | Rats, eels below the bridge | Bats | |
| | Eel Beds (3) | Marsh rats x7, eels | Marsh rats, something in the water | Eels take the rats' young; the rats take the cutters' bait |
| | Glasshouse Row (3) | Flowers x5, spiders | Flowers open | |
| | Drowned Lane (4) | Soldiers x4 *on the road* | Soldiers x6 | |
| | Mother's Garden (4) | Flowers, the statue | Flowers, the statue turned | |
| | Still Pool (4) | Nothing on the bank | On the lake's bank by the statue: something that is not a person *if the lake omen is true* (`omen:nobody_drowned`; it fights); a man wet to the collar if not, who says nobody ever drowned in it | |
| Works | Cinder Walk (3) | Crows, one soldier at each dead signal | The night shift walks the road (§4.4) | |
| | Chapel Rise (5) | Skeletons, tomb guards | Same, more; the four markers' keepers | |
| | Sidings (5) | The pay clerk, soldiers | Same | |
| | Cooling Yard (5) | Soldiers x7: the night shift | Empty: they are on Cinder Walk | |
| | Slag Mere (5) | Cactus x7, bats | Same, bats hunting | Bats take the mere's flies |
| | Bellfield (6) | Nothing by day | Everything the School lets out | |
| All | Hubs (0) | Cats, hens, the sweeper's robin | Cats. The ginger cat walks the square at any hour | |

**Rule: hostiles come on sight.** A hostile comes for her the moment she is within its aggro and in its sight, by day as by night, on the gentlest ground as on the hardest, in the county and in every dungeon; the night lengthens its reach, it does not wake it. What leaves her be is a row with no aggro (rabbits, sheep, hens, butterflies, the county's people), not an hour. The owner decided this on 2026-09-29, overriding the earlier *by day the Lowfields' own ground leaves her be*: his playtests met that rule as "enemies don't aggro unless hit first" and wanted the county harsher (`PLAN.md` §2.6, *Day*; `jane-sim/tests/ai.rs` `by_day_the_gentlest_ground_comes_for_her_on_sight`, `a_skeleton_in_the_mine_comes_on_sight_by_day`).

**Rule: a creature notices her from on the screen, and less as she outgrows it** *(the owner, 2026-09-30: "aggro range is too big; WoW-like")*. WoW's 20 yards at her level, a yard less a level over it and never under 5, scaled to a view of 48 × 27 cells (13.5 m to the top edge): most rows notice her at 8 to 10 m, the big and the elite at 11 to 13 m, and nothing past 14 m, by day or night. Her growth is her level: her strength and spirit against the creature's phase (60 a phase-table step, her New Game sum), so the Lowfields' phase-1 things notice her at their row's reach at New Game and at 4 m by the end, while a deep phase notices her from further while she is behind it. The night adds a quarter, however dark. Bosses keep their arena's rows; rows with no aggro notice nobody (`PLAN.md` §2.6 *Aggro*; `jane_sim::ai::aggro_reach`). A rooted thing (a flower, a cactus, a statue) lets go of a fight once she is half again 14 m from it (21 m; or half again its own aggro if longer), not half again its now shorter aggro: shot at from bolt range, it stays in the fight rather than letting go and mending whole between her bolts.

| Row | Aggro before (m) | After (m) |
| --- | ---: | ---: |
| skeleton, quarryman, statue, cactus, wall_spider, tale_spinner, the Pryor sitter | 16 | 9 |
| night_skeleton / night_soldier | 18 / 20 | 10 / 11 |
| skeleton_guard, bandit | 16 | 10 |
| soldier, rose_soldier | 18 | 10 |
| skeleton_clerk, plot_tenant | 14 | 8 |
| bat, ruffian, the Hurst ruffian | 14 | 9 |
| spider | 14 | 8 |
| lake_shape | 14 | 10 |
| pumpkin, pumpkin_top | 12 | 8 |
| shade, forest_night_spider | 12 | 9 |
| crow | 11 | 9 |
| sentry, master | 11 | 10 |
| flower | 10 | 8 |
| hauler | 9 | 11 |
| tale_under | 24 | 13 |
| yard_bones (new) | | 5 |
| rat, stuffed_fox, armour, waxwork, lurker; every boss | as they were | as they were |
| At night | × 1.4, × 1.8 in the Works (29 m) | × 1.25, and never past 14 m |

**Julie's yard** *(the owner, 2026-09-30)*. The yard skeleton ran at her before she reached the dog, so the dog's quest had nothing left to fight and the one skeleton was a chase on the step. Now seven `yard_bones` (a skeleton that notices her at 5 m, leash 40 m, back in 600 s) stand about the yard: two in it (the keyed `yard_skeleton` in the far corner, the hand-in's consequence takes it off for good, and one by the east fence), and five scattered 6 cells and more outside the fence within 90 cells of the yard's middle, 16 cells and more from the station road, off every road's metal and out of every set place's box (`life::yard_bones`, `Step::CountyYard`). She walks up to them; the dog's quest (`defeat_skeleton`, now a kill of `yard_bones`) always has one to hand.

**Rule: a kill quest's patch holds N+1 of its target within five cells of the patch's centre by day, and refills within ten game minutes of being allowed to** (`QUESTS.md` K8). That is `cap` N+1, a `hold` line one kill never reaches (so each of the N she was asked for stands up again at the next ten-minute mark after it falls, out of her sight; a unit's own `respawn` is only for creatures no patch's row names), and an ecology that looks at a held patch every ten game minutes: once the pressure is under the line, whatever was due stands up at the next ten-minute mark. `recover` is a rate per ten minutes, not a timer per corpse; a patch cleared to the last is quieter for hours.

### 4.2 Prey and predators

| Eats | Is eaten by (`hunts`) | What the player sees |
| --- | --- | --- |
| Rabbits (grass) | Crows, spiders | Rabbits thin near a spider patch; crows gather where rabbits die |
| Hens (seed) | Rats, the Tenant (eggs) | Fewer eggs at the honesty box when the rats are up |
| Rats (seed, eggs, bait) | Eels, cats, the vermin man | A rat pile at a hatch |
| Eels (rats' young, bait) | Reedcutters | Cutting orders slow when the eels are down |
| Crows (carrion) | Nothing | Crows follow her so far and turn back at the same spot (the farmer's line): the spot is the edge of a threat pocket |
| Bats (flies) | Nothing | Over water at dusk; over the mere all night |
| Undead | Nothing. They do not eat | They stand. That is the horror |

**Rule: hunting changes numbers, and numbers are seen.** Kill every rabbit in a field and the crows leave it; the field is quieter for a day; the rabbits are back in three. Kill the rats at the allotments and the hens lay: the honesty box has eggs tomorrow. Nothing is farmed, nothing is ground; there is no reward beyond what is seen.

### 4.3 What night does

| At the bell | Where | What |
| --- | --- | --- |
| Come up | The field edge of every road but the first walk, every 260 to 400 points; the rough ground (threat 3 and over, 2.5 to 5.5 per cent of macro cells) | **Built:** the night shift (`night_skeleton`, and `night_soldier` in the Works), `nightOnly` and `shunsLight`, placed by the county (`life::night_shift`, `Step::CountyNight`). They walk a stretch of road edge and stop at a lamp's light |
| Step on | Unlit roads only | On a lit road they stand at the lamp's edge and look |
| Turn | Named things | The statue in the lake. The scarecrow *if true*. The Tenant. Mr Pollard |
| Shut | `nightLock` doors | The shop, the farms, some cottages. Never the church, never the Arms. The constable says so, and a police notice at the west end of the High Street: "Doors are not answered after nine. The Castle Arms will take you in till it is light." Opened: the Works' wicket, for the night shift |
| Bar | The mine's front door *if the mine omen is true* | From outside, until five |
| Leak | Dungeon rings (§4.4) | |
| Go | The dog, sheep, hens; the reedcutters off the landing | To their slot. The dog's says nothing |

At six everything reverses, in the same order backwards, over a game hour. A creature caught on a road at 06:30 walks back; she can watch it go. Nothing hides or shows in view.

### 4.4 What the dungeons leak

One thing per region walks a road at night, announced by a sign first (`QUESTS.md` K7).

| Dungeon | Leaks | Where | Sign |
| --- | --- | --- | --- |
| The Gold Mine | Skeletons, two or three | The mine road inside the mine's ring, 21:00 to 05:00 | "No exit either, some nights." The carter's note |
| The Museum | Nothing walks. The wing's lights go on and off | Seen from the bridge | The toll board |
| Butterfly Forest | Eight butterflies over the road, harmless | The forest road | They are the sign |
| The pipes and the Factory | **The night shift**: the cooling yard's soldiers walk Cinder Walk to the canteen and back, in file, 21:00 to 06:00 | Cinder Walk, the whole length | The rota: "night shift to be relieved at six. Not relieved." |
| The Burial Chamber | Nothing leaves. The ground fog does (§5.3) | The graveyard and Chapel Rise | DO NOT |
| The School | Nothing, until the last act. Then the Bellfield | | The bell |

**Rule: what leaks stays in its ring and goes home at six.** A leaked thing never reaches a hub's fence, and never stands on the walk from the station to Julie's house.

### 4.5 Recovery

**Rule: nothing respawns on a timer. Populations recover toward `cap` at a `recover` rate per area, every ten game minutes, and the rate is a row.** A creature a patch's row names stands up at the next ten-minute mark after it falls, when the ecology looks, if the patch is under its line; one no row names yet keeps its row's `respawn` until its patch is written. A patch she cleared is quieter for a day; a patch whose reason she removed (the rats' food, the camp's fire, the shift relieved) stays cleared through a consequence (§6). Bosses and named creatures never return (`respawn: 0`). Nothing stands up in view.

### 4.6 Food that comes back

*(Decided 2026-09-29.)* **Rule: the county's food returns every few days; nothing else does.** An apple tree or a windfall she empties (a prop def with `regrow`) holds what its spawn row held again `regrow.days` game days later (3), give or take half a day by the prop's own id, so an orchard comes back a tree at a time and not in one minute. Julie's larders fill again on the same clock with a few staples, not with what they first held (`regrow.restock` in `data/tuning/sim.json`: the pantry chest two vials of water and two bunches of grapes, the fruit bowl two apples; the potion makings do not come back). The clock starts when the source is emptied and runs through a bed's night; a zone nobody is in fills up the moment she walks in. It is the world's, so a tree one seat stripped is bare for the others. Small amounts, spread out: food is findable, never a farm. Herbs are makings, not food, and do not return; a dungeon's chest is as it was designed. `jane-sim` `regrow.rs`, saved as `Prop::regrow`.

---

## 5. Weather and atmosphere

### 5.1 Weather kinds

The engine names four kinds: `Clear`, `Mist`, `Rain`, `Storm` (`ARCHITECTURE.md` §4.6 b). Overcast, drizzle and wind are *proposed* presentation shades of those four, not kinds of their own. A kind is drawn when the hour turns, from the seed and the region's row, and lasts its duration band; a seed's Tuesday morning is the same for every player and every replay.

| Kind | Sight | Roads (wetness) | Fires | People | Creatures |
| --- | --- | --- | --- | --- | --- |
| Clear | Full | Dry | Burn | Out | As the hour says |
| Mist | Half; lamp light reaches a third as far; the School's window shows only from high ground | Dry | Burn | Out, fewer lines about the lamps | Night creatures come up an hour early |
| Rain | Two-thirds | Wet: the ramp climbs; unpaved tracks slow her by a tenth; fords deepen a cell | A kept fire goes out *as light* when wetness crosses its `douse`, and is still a rest point; relit at six. A made fire (PLAY-PLAN.md §2.2) goes out for good at the next ten-minute mark, unless a tent or a shed stands within three cells of it; its place still wakes her | In. Knock lines only | Rats and rabbits in. Undead unmoved |
| Storm | Half; lightning shows the whole screen for a frame | Wet. A trunk down across one path per storm (pushable) | As rain | In, doors barred. The reedcutters off the water | Bats grounded. Electric lamps flicker |

**Rule: weather never takes away a *kept* rest point, a key, a road or a quest target.** It costs time, light and company. A fire she made is hers to lose to the rain (decision D6, 2026-10-02); the county's own fires (the hubs, the hearths, the dungeons' stoves, a fire a quest lit) are never put out, and every region keeps one inside its base threat (`crates/jane-world/tests/county_fires.rs`). A trunk across the footpath leaves the road open. No storm on the first walk; mist never hides a lit lamp inside its reach.

### 5.2 Weights by region and hour band

Shares out of ten. *Proposed*; John tunes by the seed viewer's weather column.

| Region | Band | Clear | Mist | Rain | Storm |
| --- | --- | ---: | ---: | ---: | ---: |
| Lowfields | 06 to 12 | 7 | 1 | 2 | 0 |
| | 12 to 18:30 | 8 | 0 | 2 | 0 |
| | 18:30 to 21 | 6 | 2 | 2 | 0 |
| | 21 to 06 | 5 | 3 | 2 | 0 |
| Waters | 06 to 12 | 3 | 5 | 2 | 0 |
| | 12 to 18:30 | 6 | 2 | 2 | 0 |
| | 18:30 to 21 | 3 | 4 | 2 | 1 |
| | 21 to 06 | 1 | 6 | 2 | 1 |
| Works | 06 to 12 | 6 | 2 | 2 | 0 |
| | 12 to 18:30 | 7 | 1 | 2 | 0 |
| | 18:30 to 21 | 5 | 2 | 2 | 1 |
| | 21 to 06 | 4 | 2 | 2 | 2 |

**Rule: the sky follows the region under her feet.** The county is three skies, drawn apart by the rows above: the sky that rains on a cell, and that she sees, is the sky of the skeleton's region at that cell (Lowfields, Waters, Works), so she can cross the river out of the rain and into mist. Each region shades its own kinds (mist in the Waters is fog; in the Works it is smoke). A zone off the county is under its row's sky (`weather.json`'s zone lists). The first walk's band is clear on every seed.

### 5.3 Atmosphere layers

`atmosphere.json`: a layer is a fog volume the renderer draws from data (`PRESENTATION.md`, lighting), keyed to the skeleton's sites, areas, terrain bands and poi kinds so it lands where the place is on every seed. Each row: where, hours, the weather it needs, colour, density, height (ground or full), drift, and what it does to sight. Presentation only; it changes no state.

| Layer | Keyed to | Hours | Needs | Look | Does |
| --- | --- | --- | --- | --- | --- |
| Fen mist | Terrain `bank`; areas `sallow_bottom`, `eel_beds`, `still_pool`, `tollhouse_reach`; poi `jetty`, reed huts | 19:00 to 08:00 | Any | White, waist-high, drifting off the water | Sight halved at ground level. Things in it show head and shoulders |
| Lake haze | Site `lake_statue`; area `still_pool` | 05:00 to 09:00 | Clear | Pale, full height, thin | The statue is a shape until she is on the bank |
| Works smoke | Sites `factory`, `canteen`; areas `cooling_yard`, `sidings`, `slag_mere` | All hours, thicker after the bell | Any | Grey-brown, full height, drifting east | Lamps have haloes. Darker over threat 5 |
| Canopy light | Terrain `wood`; sites `car_wood`, `butterfly_forest`; area `behind_the_car` | Day | Clear | Shafts, moving | Bright cells and dark cells; creatures stand in the dark ones |
| Ground fog | Site `graveyard`; area `chapel_rise`; the burial's mouth | 21:00 to 06:00 | Any but storm | Grey, ankle-high, still | Hides the ground: the footpath's stones show only up close. Leaks a screen down the graveyard road |
| Hill cloud | Site `school`; area `bellfield` | 21:00 to 06:00, and in mist | Any | Low cloud on the crown; the window lit inside it | A light without a building. By day, a building |
| Town smoke | Site `town` | 06:00 to 09:00, 16:00 to 19:00 | Any but rain | Chimney threads | A lit chimney is a person home |

**Rule: a layer is where its key is, on every seed, and nowhere else.** No layer stands on a hub's fire. The fen mist stops at the last `bank` cell plus a screen's drift. A layer that hides a quest target leaves the target's landmark (the sign, the post, the plate) visible from the road.

**Rule: atmosphere is felt threat, not a filter.** Where a layer is thickest the threat is highest, so a player who reads the mist reads the map. The seed viewer paints layers beside threat so the two can be compared.

---

## 6. Consequence

**Rule: every quest outcome changes the world, visibly and for good, and the journal records the change.** `consequences.json`: per quest (or tale branch, or a named death), the world edits that run once: `Show`, `Hide`, `Lock`, `Unlock`, `Switch`, `Spawn`, `Despawn`, `Fill`, `Send`, `Flag`, and optionally a `Claim` the edit confirms or contradicts. The log never says what it meant (`VOICE.md` rule 5); the county shows it.

All *proposed* unless `QUESTS.md` already states the consequence.

| Quest | Consequence at hand-in | Seen where |
| --- | --- | --- |
| The Thing in the Yard | The fence line clear; skeletons never spawn inside Julie's fence again | The yard |
| Julie's Kitchen | The house is a hub: threat 0, `nightLock` on the door with her key. *Shipped:* `house_kept`, the door night-locked once she has seen the kitchen: from nine to six it answers only a key that fits it, and hers is bound (the door out never locks, so nobody is shut in) | The door |
| Dust, Water, Pansy; Under the House | The bench lit; the cellar's rats at two for good; the roses grow back | The kitchen, the cellar |
| The Gold Mine | *Built* (`mine_quiet`, on Iron Knuckles' death): someone sits under the Company notice at the mouth; the mine's door is never barred at night again; the town hears (§7.2). *Proposed:* the notice overpainted OPEN; the mine's skeletons stop walking the road | The mine road; the Arms |
| A1, A2 Lost Property, Left Luggage | Three lines filled; the shelf open, one article gone; the trunk stands open | The platform |
| A3 To Be Collected | Parcel taken: the platform lamp never lights again. Left: the crate empty at six | The platform, every night |
| B1, B2 The allotments | Rats at four, not eight; the hens lay; Plot 9 dug over, the stake gone, "Plot 9 is let" | The allotments, the farm |
| C1, C2, C3 The farm | The door opens a hand's width by day; the top scarecrow taken down next morning (or not); the Top Field ploughed, threat 3 to 2 | The farm, the Top Field |
| D1, D2, D3 The nurse | Three cards: "The nurse has been"; her prints grown over in three days; Mrs Allen home by day, her sister's door has one voice | Three cottages, the wood, Castle |
| E1, E2 The lampman | Plates 12, 13, 15 cleaned; a stump where 14 would be; **a fire in the east Lowfields**, threat -1 within a screen | The dead run |
| F1, F2 The garden book | Five roses picked, three back by day 3; the dog says its line and nothing else | Sallow Bottom, the step |
| G1, G2 The Company | The face quiet; the notice's tally chalked; the slate wiped and "4" rewritten; the adit rock in the journal for Explosion | Quarry Steps |
| H1, H2 The right of way | The path's grass trodden wider; the haversack on the unclaimed shelf | The Long Hedge, the platform |
| W1 to W3 The toll | The tollhouse fire lit; the board reads PAID; the keeper's box open | Tollhouse Reach |
| W4 to W6 The reeds | The cutters back in the beds; hut doors open; the fifth coat on its hook, wet | The Reedcutters' fire |
| W7 to W9 The garden | Rare herbs in Julie's garden; the statue faces the other way by day too | Julie's yard, Mother's Garden |
| W10, W11 The library | The library fire lit; three books shelved with Julie's margins | The portico |
| W12, W13 The lane | A fire at the far end; the council sign turned: DIVERSION | Drowned Lane |
| W14, W15 The lake | Three names crossed out on the shrine; the statue's day face fixed | The lake |
| K1, K2 The rota, the dinners | The hut fire on Cinder Walk lit; three tins at the signals; the night shift stops at them | Cinder Walk |
| K3 Relief | The cooling yard empty. **The night shift no longer walks Cinder Walk.** The rota: RELIEVED | The whole Works after dark |
| K4 Signals at Danger | Three signals at danger: the train does not stop *from now*, whatever the omen said | The Halt, next Sunday |
| K5, K6 The diary, the last wagon | The Factory side door open; the sidings quiet | The sidings |
| K7 to K9 The graveyard | Three graves tended; roses on the Rise; the four markers named in the journal | Chapel Rise |
| K10, K11 The pipes | A hatch open: a shortcut into the Works; Slag Mere quiet | The Works |
| K12 The Bell Rope | The bell rings *twice* at nine from now: the School's, and one from under the Bellfield | Everywhere |
| K13 No. 14, Again | The dead run lit for good, threat -1 at night on that road, permanently; a new post where 14 stood | The Lowfields |
| K14 The Adit | The rock gone; the adit open | Quarry Steps |
| Tales 1 to 9 | As `QUEST-TREE.md` §10 states per branch: the hives in black or a honey jar; the chair or a second plate; the boots gone or new planks; the red lamp or the green | Each tale's place, every day after |
| The Museum; Butterfly Forest; the Factory (the spine, `QUEST-TREE.md` §5) | *Built*, each on its boss's death: the lamp over the Museum's west end lit at night (`wing_lit`, `museum_wing_lamp`); the two butterflies on the road by the forest gate gone (`forest_quiet`); the lamp at the Factory's gate out for good (`works_dark`, `factory_lamp`) | The Waters, the Works |
| Under the Stone (the Burial) | *Built* (`burial_quiet`, on Goldskin's death): the graveyard's two skeletons gone; Goldskin said by name in the town. *Proposed:* the ground fog gone (a presentation layer); the four markers' keepers up | The graveyard; the Arms |
| The Bell at Nine (the School) | *Built* (`bell_stopped`, on the Timekeeper's death): the bell stops, at nine and at six and on Tuesdays; the School's two lamps out; Mrs Tace up at the top of Church Lane | Everywhere; the School's front |
| Yours to Say | The ending chosen, and the game closes (`STORY.md` §10) | The study, the mine, the Halt |

**Rule: a consequence is a set of world edits that runs once and lands in every zone it names, never a flag alone.** A quest whose only consequence is a flag fails the cohesion test.

---

## 7. Rumour and knowledge

### 7.1 Facts

The **journal** (`ARCHITECTURE.md` §3.7) holds what she has understood: entries (read, heard, seen, a consequence) and known facts keyed as `Place`, `Person`, `Thing`, `Claim`, `Route`, `Danger`, `Rumour`, each with when and how she learned it. It is the reference: **dialogue may only assume what the journal holds.**

| Key | Example | Written when |
| --- | --- | --- |
| `Place` | Julie's house; the Top Field | A banner, a sign, a line that names it |
| `Person` | Mr Cobb; the Headmaster | First met, or first named by a teller |
| `Thing` | The trunk; the adit rock | Read, opened, or named |
| `Claim` | "The lamps on the east road go out at ten" | Read at a sign; later *seen* by a consequence that confirms or contradicts it |
| `Route` | Castle to the mine, by the mine road | Walked, or given by a teller |
| `Danger` | The Top Field, after the bell | Felt (a hit there), or warned in words |
| `Rumour` | "The mine's open again" | A `tells` line played by someone whose `spreads` has delivered it |

**Rule: no text confirms a claim** (`VOICE.md` rule 5). The journal records a claim as *read at* and, later, *seen*. The truth of an omen is a seed roll no row can read into a line.

### 7.2 How facts travel

A story row declares `spreads: { to: [person], after: <days> }`. From the tick the story's own trigger fires, each person named hears of it after the delay, and their line may test whether the speaker knows. The routes below are the intent behind each `to` list; a route is real only when the two people share a schedule place.

| From | To | Via | Delay |
| --- | --- | --- | --- |
| Her arrival | Mr Cobb | Seen from his seat | Same evening if she passed before 18:00; else day 2 |
| Mr Cobb | Mrs Garland, the Milkman | The Arms yard, the round | Next morning |
| Mrs Garland | Everyone who drinks | The bar | Same day |
| The Milkman | Every door on the round | The step | Next morning |
| Her deed at the mine | The Misses Crane | Their window on the mine road | Next day |
| **Built: the town's news** | A consequence row's `spreads`, in the order they hear it (`STORY.md` §11) | The mine: Mr Cobb (who sees who comes up the street) 3 h, Mrs Garland 12 h, the constable and Mr Ince 18 h, the Milkman 24 h, No. 3, Pound Lane and Mr Hale 28 h. The Museum's lamp: the constable (from the east end of the street at dusk) 24 h, Mr Ince and Mrs Garland 30 h. The Works dark: the sweeper 24 h, the constable 28 h, Mrs Tace 30 h. Goldskin: the vicar (from the church door) 12 h, Mr Tolly and Mrs Garland 24 h, Mr Sallis 28 h. The sheds: the Milkman 12 h, Mrs Bex 24 h. The bell's silence is heard by everyone at once, at the first nine it does not ring | Each line said once |
| The Cranes | Mrs Allen | Nobody direct. The farmer at market, Wednesday | Up to 3 days |
| The reedcutters | The town | Nobody crosses the bridge. The Waters hear nothing from the town and tell it nothing | Never, unless she carries it (a `tells` with her as source) |
| The canteen hatch | Nobody | | Never. The Works are cut off |

**Rule: rumour is slower than she is.** She can always outrun a fact. Arriving before the news of herself is the ordinary case and a source of unease ("You'll be the one off the Sunday train. We heard Tuesday.").

**Rule: a person tells only what their `tells` row holds and their `spreads` has delivered.** A line that mentions a deed the journal does not hold, or an event that has not happened on this seed, is a bug (L5).

### 7.3 Omens per seed

About a third of omens are true per seed (`PLAN.md` §5), rolled at New Game from the world stream, one roll per row of `data/omens.json` in row order, with constraints (no two lethal ones of a region stacked; none between the station and Julie's, where no row claims anything; a few per region is the rows' job). *Built:* eight omens, each true on about a third of seeds, at least two a region, in this row order (the first three were shipped first, so adding the rest moved none of their rolls). The Lowfields: the scarecrow closer (C2, C3); no exit from the mine some nights (the front door barred from nine to five, the adit the way out); the white roses after dark (lethal: a soldier in Julie's cellar's study). The Waters: the east road's lamps out at ten (the lighting notice by its first lamp says so; the run is its own lamp row); the bridge counts who crosses (the toll board at the east bridge; the third time she steps onto it in a night its two lamps go out together until the evening, which costs light and nothing else); nobody has drowned in the lake (lethal; the council's notice by the statue; the first time she comes near, the lake's night figure is decided: something that is not a person, or a man who is wet to the collar). The Works: the early bell (Tuesdays at ten to nine; Mrs Fenn, Mr Treloar and the others say it on every seed); the train that does not always stop (one Sunday in three, one whistle and no sacks). The train's is the railway's, heard from anywhere and seen at the Halt a week after she came, never on the first walk; nothing else true stands between the station and Julie's. The Headmaster's bell in the mine is still *proposed*. The county then *does* what a true omen says at the hour it says. People's lines are the same either way (Mrs Fenn checked her clock and then checked the clock, on every seed from day 4). Confirmation is by observation only, through a consequence that names the claim; contradiction is the same: the county fails to do the thing, and the journal records *seen: the lamps burned all night*.

### 7.4 The long-running context

A player who comes back after an hour away, or a guest who sits down at the party's fire, reconstructs what is going on from three things and nothing else:

| Source | Holds |
| --- | --- |
| **The journal** | Quests (with `returnTo`), facts by key with where and when, people met (last thing said, where they stand), claims and what she saw of them, a day line per day ("Day 3, Tuesday. Rested at the Reedcutters' fire. The bell went before nine.") |
| **The map** | What she has seen, fogged beyond it; every fire she has rested at; every sign she has read as a dot she can re-read |
| **The people** | Whoever is nearest with a `tells` row that assumes only the journal: the dog by day, the parish board, the lost-property book, the rota, the order slate. Each says the next thing, not the whole story |

**Rule: the journal is the world's, not the seat's** (`PLATFORM.md` §2: quests, flags and growth belong to the world). A guest reads the same journal the host does.

---

## 8. Global cohesion

**Rule: one name means one thing everywhere.** A place, a person, a bell, a train, a company has one name in every row, sign, line and journal entry, and the seed viewer and the journal use that name.

**Rule: distances in text match the county.** Text names relations the generator guarantees (`QUESTS.md` K4); "an hour's walk" is wrong on a county five minutes wide. Text says "the end of the mine road", "across the river", "at the top of the town", never minutes or metres.

### 8.1 The three economies, one history

| | The Lowfields | The Waters | The Works |
| --- | --- | --- | --- |
| Made or grew | Turnips, eggs, wool, apples; thatch bought from the Waters | Reed, eels, garden flowers, the Museum's visitors | Gold, then whatever the Factory made from it; slag; the light railway |
| Sold where | Castle market, Wednesday; the Sunday train | Castle, over the tollhouse bridge | Everywhere, by rail. The trunk on the platform is the last consignment |
| Owed to | The Company: the mine's wages paid the town's rents | The Company: it bought the water rights (the Intake, the diver) | The Company, entirely |
| What went wrong | The mine closed; the paperwork stayed | The gardens were tended once; the tollkeeper stopped counting | The shift never ended |
| The School | The children went up the hill to it; the Headmaster took them down the mine | Mrs Wakes's scarf is "not the School's register" | On the crown, over everything; the Bellfield is its field |

### 8.2 The cohesion table

Cross-region references that must agree. Each row is an L5 cohesion check: the same fact in two places has one value.

| Reference | Said where | Must agree with |
| --- | --- | --- |
| The bell at nine | The clock row; every farmer; Mrs Fenn; Mr Lyle (wrong, on purpose); the rota; the Bell Rope | One bell, the School's, heard everywhere. The church rings at six |
| The Sunday train | The letter; Miss Dray; the lost-property dates; the Voyseys' timetable; K4; the ending | Day 1 and every seventh day, 17:00, the west fence; "does not always stop" is an omen |
| Fourteen lamps | The dog's poke chain; A. Pell's stone; E1; K13 | The dead run has 12, 13, 15 and a gap. The dog's number is the dog's |
| The night of the bell | Mr Dunn; Mrs Tace; the memorial's sharp name; the Rooks' Sunday | One date, in the spring, before she came; the undead date from it |
| Gnox Goldskin | The mine notice; the trunk's consignor; the diver's station; the light railway; the Museum's portrait; the foreman's diary; the Burial | One man, one company, one sphere. Never seen before the Burial |
| The nurse; the carter | Her car and three cards and Mrs Allen and K7; his cart, note, the Cranes' parcels, the trunk's key | One woman, one Tuesday, one coat; one man, one horse, the quarry turning |
| The Headmaster | The mine; the register; the Pollards' Tom; Mrs Wakes's scarf; the School | One man, in the mine, with the register; the School is empty of him |
| White roses | Julie's cellar; Sallow Bottom's jetty sign; K8; the rose omen | One kind of rose, in two places; the omen is about picking after dark |
| The statue | The lake; W9; W15; Mother's Garden | Two statues (the lake's, the garden's), each with its own turn; never confused in a line |
| North | Every School line | The School is north of every Lowfields place on every seed; "north" is the only bearing text may use |

### 8.3 Where things are kept

A county that hands her keys she must not throw away and gold bars nobody buys gives her somewhere to put them down. **Cupboards** (a prop row with `store`, `jane_sim::store`) keep 24 slots each; what is in one belongs to the world and the whole party, saved with it. Anything goes in, keys and a quest's things included; what is in a cupboard is not in her bag, so nothing that asks her to *hold* a thing sees it there. They stand where a person would keep things, never in a dungeon:

| Where | Which | Stands |
| --- | --- | --- |
| Julie's house, the kitchen | Julie's dresser (`julies_dresser`) | Against the north wall between the two hatches, plates on its rack |
| Julie's house, the front room | Her cupboard (`julies_cupboard`) | Against the north wall by the bed, where the second shelf stood |
| Julie's cellar, room A | A cupboard (`cellar_cupboard`) | On the north wall, left of the way in from the iron door |
| The Castle Arms | A lodger's cupboard (`arms_cupboard`) | On the landing behind the partition, in the corner under it |
| Castle Halt | Left Luggage (`left_luggage_locker`), a green locker | By the lost property, within 12 of its mark (`data/placements/lowfields.json`) |

---

## 9. Story, structured

`STORY.md`'s truths stand (§1 to §6 there). This section proposes a **spine of acts** across the six dungeons, what the county knows at each boundary, and how generated side stories hang off it. All *proposed*.

### 9.1 The acts

| Act | Dungeon | She learns to do | The county knows at the boundary | The world changes |
| --- | --- | --- | --- | --- |
| 0 Arrival | The road, the house, the cellar | Walk, read, craft, Icebolt | "Someone came on the Sunday train" (day 2, the town) | The house is a hub |
| 1 The Mine | The Gold Mine | Repair | Mr Cobb saw her come back down the mine road; the Arms by that night, the milk round the next morning | Someone under the Company notice; the mine's door never barred; the Museum key |
| 2 The Waters | The Museum, the ruined library, Butterfly Forest | Explosion, Grow | The constable sees the Museum's lamp from the east end of the street. Nothing else: the Waters do not talk to the town; she carries the forest to Dr Vane or nobody knows | The wing's lamp lit; the forest road's butterflies gone; the Amulet |
| 3 The Works | The pipes, the Factory | Electric | The sweeper: no soot on the square; Mrs Tace | The Factory's lamp out; the dead run relit (K13); the Works darker |
| 4 The Burial | The Burial Chamber | Fire | "Goldskin" said aloud in the town for the first time: the vicar, then Mr Tolly and the Arms | The graveyard's skeletons gone |
| 5 The School | The School | Everything | The bell does not ring, and everybody heard it not ring | The School's lamps out; Mrs Tace at the top of Church Lane; Yours to Say |
| 6 The choice | The study, the mine's vault, or the Halt | Carry it | Nothing: the game closes | `STORY.md` §10 |

*The reference build had the Burial second, after the mine, and the data kept that order until 26 September 2026; `PLAN.md` §2.2 moves it fifth, and the data now follows the plan: the dog offers each act's place in this order (`QUEST-TREE.md` §5), and the Burial's stair stays locked until the Factory's key.*

### 9.2 What each act boundary leaves in the journal

| Boundary | Facts (read or seen) | Quests | People |
| --- | --- | --- | --- |
| After 0 | Julie expected her; the bell is at nine; the dog is not on the step after nine; the map "was right"; the mine is at the end of the mine road out of Castle | The mine offered | The dog |
| After 1 | The Headmaster keeps a register in the mine; the Company shipped gold; the Museum is across the river; the vault's brass key is the Museum's | The Museum offered | The Cranes; the carter, by his note |
| After 2 | What the gold was for (the MAGIC wing); where the well were sent; Grow, from the last page | The Factory offered, by the pipes | The reedcutters; the tollkeeper's tally; the dog, one slip |
| After 3 | The Works ran on the same gold; the foreman's diary; the four markers; the Chairman's Key opens the stair under the Hoar Stone | The Burial offered, via the graveyard footpath | The canteen hatch; the Voyseys |
| After 4 | Goldskin is the wizard; the Ball; the shield was Julie's | The School offered | Nobody new. The town says his name |
| After 5 | Who rings the bell; the Ball is hers to say | Yours to Say | The dog, at the top of Church Lane |
| After 6 | The choice made (`the_end`) | None | The dog, or not |

### 9.3 Side stories on the spine

| Kind | Count | Hangs where | Rule |
| --- | --- | --- | --- |
| Region side quests (`QUESTS.md` Part 2) | 19 + 15 + 14 | Off the hub, outpost and patches of the act's region | Never required by the spine. Each chain ends in growth, a fire, a key or materials |
| Country stories | 27 households | Anywhere in the region's open country | *Find, take, knock, look, count, clear.* Never the spine's nouns |
| Tales | 9 | Ruins, three per region | Two or three deep, a turn, a choice; the choice's consequence stands every day after |
| Omens | About a third true | Signs, notes, stones, rumours, per region | Fair; never on the first walk |
| The dog's chain | 8 pokes | The step, by day | Slips twice in the game, both behind a question she asks: the forest, and the choice (`STORY.md` §3) |

**Built 1: the act's news.** Each act's boss starts the town's news (§7.2, §9.1's column; `STORY.md` §11). It is the one place the county acknowledges her, and it travels at rumour speed, so a fast player hears it after she has done the next thing.

**Proposed 2: the Waters' silence is the Waters' idea.** Nobody there talks to the town; the tollkeeper stopped counting. She is the only carrier: telling Mrs Garland what the wing showed is a `tells` with her as source, and the only way the town learns act 2. Optional, never required.

**Proposed 3: the second bell.** From K12 the bell rings twice at nine: the School's and one from under the Bellfield. That is the Works' act boundary made audible everywhere, and the School's last act begins with the two bells falling out of time.

---

## 10. What the data needs

### 10.1 Rows

Shapes follow `ARCHITECTURE.md` §4.6; this table says what each row must hold for this document to be true.

| Row or file | Shape | Holds |
| --- | --- | --- |
| `schedule` (on a unit def) | `[(hour_from, hour_to, Mark(place) / Patrol / Inside(prop) / Absent)]` | §2.2, §3. Every hour covered; `Absent` only for the dog, sheep and hens |
| `tells` (on a dialogue line) | The fact it teaches (`FactKey`), and the line's conditions on the journal | §3, §7. What they say and what it writes |
| `spreads` (on a story row) | `{ to: [person], after: <ticks> }` | §7.2. Who hears, after how long |
| `claim` (on a text row) | `{ kind, subject, ... }` | §7.3, `VERIFICATION.md` L5. Every line that says where, how many or when |
| `consequences.json` | `{ id, on: Flag / QuestDone / Dead, edits: [world verbs], claim? }` | §6. One row per quest, tale branch and named death |
| `weather.json` | `{ region: { band: { Clear, Mist, Rain, Storm: weight }, season? } }` plus a duration band per kind | §5.1, §5.2 |
| `atmosphere.json` | `{ id, keyedTo: { site / area / terrain / poi }, hours, needs, colour, density, height, drift, sight }` | §5.3 |
| `ecology.json` (area spawn rows) | `{ unit, cap, recover, hold, day: <place>, night: <place> }`; on unit defs `hunts`, `flees` | §4 |
| `leaks` (on a dungeon row) | `{ unit, count, ring, hours, sign }` | §4.4 |
| `calendar` (clock rows) | `{ day or weekday, hour, actions }` | §2.3 |

### 10.2 The checks

`VERIFICATION.md` L5 (truth and cohesion) and L6 (the living world) hold this document, with L1 for what must exist on every seed. Named here so each section has its check.

| Section | Layer | Check |
| --- | --- | --- |
| §1 the rule | L6 schedules | Every named unit has a slot for every hour; nobody teleports in view; `Absent` only where §10.1 allows |
| §2 time | L6 the bell, night rule | The bell at 21:00 and 06:00 in every zone on every seed; a bed sets 06:00; `nightLock` doors lock on it and open at six |
| §2.3 calendar | L6, L1 | Day-keyed rows fire on their day; no spine quest depends on any |
| §3 people | L6 schedules | Every person at their slot each hour, walking the route between; the route is on road or path |
| §3 people | L5 claims | Every `tells` line names only things placed within a screen of the speaker or true everywhere |
| §4 ecology | L6 ecology, havens | Counts at 06:00 and 21:00 match the rows within one; kill one and it stands again at the next ten-minute mark it is not watched; a held patch refills within ten game minutes of falling under its line; nothing leaked crosses a hub fence or the first walk |
| §4.3 night | L6 presence | At 21:00 every night-only unit is up within a game hour; by 07:00 none remain; none hide or show in view |
| §5 weather | L6 weather | Same kind for the same seed, day and hour on every replay; no storm on the first walk; mist never hides a lit lamp inside its reach; no kind removes a kept rest point, key, road or target |
| §5.3 atmosphere | L1, L6 | Every layer's key resolves on every seed; no layer covers a hub fire; every quest landmark under a layer is visible from its road |
| §6 consequence | L6 consequences | Every quest and tale branch has a row with at least one world edit; the edit is present after hand-in, after a save and load, and in a zone entered later |
| §7 rumour | L5 knows, L6 rumours | No line assumes a fact the journal lacks at that tick on any trace; a rumour is offered only after its event and within a day |
| §7.3 omens | L5 omen | No text row reads an omen's truth; the journal's *seen* entry appears only through a consequence |
| §7.4 context | L3 the Lost, L5 knows | From any save, the journal plus the nearest teller yields the next `returnTo` or offer within one conversation |
| §8 cohesion | L5 cohesion | One name, one thing; no text contains minutes or metres; "north" is the only bearing |
| §9 story | L5, L6 | After each act's hand-in the journal holds §9.2's facts; the act rumour reaches the Arms within its delay |
