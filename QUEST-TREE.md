# Jane: the Quest Tree

Every quest in the game, step by step, as the player meets it: what unlocks it, who gives it and where they stand, the exact words she reads, what she is probably thinking, where the thing actually is, what could go wrong in her head, and how she takes it back. Written 23 September 2026 after John said "quest play is confusing".

Pair with `QUESTS.md` (why each quest exists), `VOICE.md` (how the words sound) and `jane/test/quest-audit.test.ts` (the part of this document a machine checks on every seed, after every world change).

**The lesson this is built on.** When the Lost Property chain confused John, the cause was the county, not the words: the book said "by the well" and there was no well. So every step below was measured in the built county first (seeds 3 and 2026; the audit also runs 77), and where a step failed, the world was changed before the words were.

---

## 1. What the player actually sees

There are no markers, no pins and no arrows (`PLAN.md` 2.5). Everything she has is words, and this is where they are:

| Where | What it shows | Source |
| --- | --- | --- |
| **Tracker** (top right, always on) | The quest's name, then each step's `text` with a count: `A red glove, at the well on the station road 0/1`. When every step is done: `Lost Property - ready` and **`Back to the lost-property book, on the platform at Castle Halt`** | `ui/hud.ts`, `requirements[].text`, `returnTo` |
| **Quest log** (Quests tab) | The description, the steps with `[ ]` / `[x]`, and when done **`Ready to hand in: <returnTo>.`** | `ui/windows.ts` |
| **Use prompt** (above the bar) | What USE will do, **and now the name of the thing**: `[E] Read: The parish board`, `[E] Pick up: A red glove`, `[E] Knock: The farmhouse door` | `ui/hud.ts`, the prop's `label` |
| **Dialogue** | The giver's offer, its "wait" line if she asks again (the directions, repeated), its hand-in line | `data/dialogue*` |
| **Signs** | Anything she can read at a place: well plates, fingerposts, notices, notes | trees on placed props |
| **Zone banner** | The name of a place she has gone into: "Julie's House", "Goldskin Mine", "Burial Chamber" | the zone's name |
| **Toasts** | `Quest: <name>`, kill counts `Rats...: 4/6`, `Done: <name>` | `sim/quests.ts` |

Two of these are new with this document: the **`returnTo` line** (before, "ready" never said who to go back to) and the **label in the use prompt** (before, a prop's label was only ever shown in "X is locked"; she could stand at the carter's cart and not be told it was the carter's cart).

The camera shows **48 x 27 cells** (1 cell = 1 m). Half a screen is 24 across and 13 down. She walks about **6 cells a second**, 360 a minute. Walks below are **steps on foot** from the giver, measured by flooding the built county (solid ground and solid props stop her).

---

## 2. The rules the audit enforces

`jane/test/quest-audit.test.ts` builds the county (and every zone) on three seeds, derives every giver, step source and hand-in from the data (nothing is typed in as a coordinate), and holds each step to these. Thresholds are named constants at the top of the file, each with its reason.

| | Rule | Threshold |
| --- | --- | --- |
| **A1** | No id shows in player text (`lost_glove`, `poi_12`) | |
| **A2** | Every step names a landmark from the glossary (`LANDMARKS`): a place a person could look for. "Rat Meat", "The Headmaster", "Skeleton put down" fail | |
| **A3** | Every landmark a step names is **written up at the place itself**: a sign, plate, note, label, speaker name or zone banner that says that name stands within reach of it. Otherwise she cannot tell she has arrived | `POSTED_NEAR` = 2 half-screens (of a patch's edge) |
| **A4** | A step that only completes after the bell says so in the step; one that only completes by day says so in the step or the description | night words: *after the bell, after nine, at night, after dark* |
| **B1** | Something in the built world produces the step, on every seed, and it is not hidden unless something shows it | |
| **B2** | It can be walked to from the giver; any locked door on the way has a key that can be had | flood fill, arm's reach |
| **B3** | Kill steps: enough stand in the world, **and** enough stand near the place the step names | qty + `KILL_SPARE` (1) |
| **B4** | Collect steps: enough can be had (loot, drops that always drop, rewards, the bench) | |
| **C1** | The thing is **on screen from a road or path**, or close to a landmark the step names which is itself seen from a road (a small place's edge; a patch's edge; a site) | `ON_SCREEN` = 1; `NEAR_LANDMARK` = 2; `LANDMARK_SEEN_FROM_ROAD` = 1.5; `PATCH_SEEN_FROM_ROAD` = 4 half-screens |
| **C2** | The walk from the giver fits the quest's tier | hub 400, near 1500, far 2800, cross 4600 steps; `TIER` says why each far one is far |
| **D1** | Whoever takes it back can be walked to from the last step | |
| **D2** | Whoever takes it back is the giver, or is the last step itself, or is named in the quest's text; and the log says who (`returnTo`) | |
| **D3** | The walk back fits the tier | same budgets |
| **E1** | What completes a location step stands at the place the step names, not somewhere else | `NEAR_LANDMARK` |
| | Tracks the quests send her up never cross water (a dirt stroke over a river would be a ford) | |

**The words are true** (`jane/test/truth.test.ts`, added 24 September 2026 after John picked up Mrs Bettany's "key under a stone" lying in the open before she had asked). The audit proves she can find the place; this proves what the words say is there is there, the way they say it, on three seeds:

| Claim in the words | What the world holds |
| --- | --- |
| "The spare's under a stone in the garden." "There are four stones, and they shift if you lean on them." | Four pushable stones on the garden rows; the key is a hidden prop under ONE (the seed's choice), shown when that stone is pushed off it, and only once she has been asked (`hides`, `sim/under.ts`). Pushed early, the stone has earth under it; the key is there the moment she is asked |
| "My mother kept a tin under the front step." | A loose step (pushable) at the ruin's door, the tin under it, the same way |
| "By the well wall", "at the foot of the post", "in the road beside it", "by the tailboard", "on the ground by the car" | Within 3 cells of that prop (`beside`: moved there after placing, no dice drawn) |
| "Eleven parcels", "five dinner tins", "three bottles of milk on the step", "the ones on the ground" | Eleven parcels, five tins by the cold fire, the milk at No. 7's door, three windfalls under the orchard trees |
| "In the pen with the sheep", "Candles. And a chair.", "Two stones are set by it to sit on", "Teaspoons?", "a biscuit tin is put out on the step" | Sheep round the sack; a kitchen chair by the candle ends; two sitting stones by the brother's fire; teaspoons in the crows' box; the sister's door puts the tin out (`show`) rather than handing it over |
| Every thing lying on the ground | Drawn as the item it holds (`showsLoot`), with an icon of its own shape; paper only for letters |
| Signs, boards, fingerposts, milestones | Never name Julie: a roadside signpost names places, not anybody's house |

Animal and people lines were rewritten to what the sim does: sheep keep to a few yards and are gone at the bell (`dayOnly`), hens walk the same few spots in order (their patrol), rabbits never run (no flee), the ginger cat walks the south side of the square end to end. **A new line that says where something is gets a row in the truth test, or is written so it cannot be wrong.**

New placement fields that made these cheap: `hides` (a thing under a pushable prop), `at.prop` (in front of a prop), `beside`, `count`/`within` at a story place (open cells round a slot, no dice), and `ownDice` (a row added after the seeds were tuned draws its own stream, so it moves nothing else).

**The audit can fail.** One more test feeds it seven quests broken on purpose (a bare noun, an id in the text, a step pointed at the wrong place, a target nothing produces, three of a creature that stands once, a night-only step that does not say so, a "near" errand sent across the map) and checks each is caught by the right rule.

`QUEST_REPORT=1 npx vitest run test/quest-audit.test.ts` prints the whole table for every seed: step, source, bearing and walk from the giver, half-screens to the nearest road, half-screens to the named landmark, verdict.

---

## 3. What the audit found, and what changed

First run, before any fix: about thirty distinct step failures across the 26 quests, and every quest failed D2 besides. All are fixed; the audit is green on seeds 3, 2026 and 77.

| Found | Why a person would be lost | Fixed by |
| --- | --- | --- |
| No quest said who to take it back to | "Ready" and nothing else; the farmer and the book are 1,000 to 2,000 steps apart | `returnTo` on all 26 quests; tracker shows "Back to ...", log shows "Ready to hand in: ..." |
| A prop's label was never shown | She stood at the carter's cart and was not told so | Use prompt now reads `Read: The carter's cart` |
| **The Gold Mine**: "The mine mouth in the yard is open" | Wrong. It is 2020's layout. The mine is at the end of the mine road, 1,900 to 2,500 steps from the yard | Description, first step and the dog's offer now say "the end of the mine road out of Castle" |
| **The Snake Below**: "The other stair is the burial"; "Enter the burial chamber" | Wrong. There is no other stair. The chamber is off the graveyard road past Castle, 1,500 steps away, and nothing said so | Dog, description and steps now send her to the graveyard and the footpath to a stone that says DO NOT; a sign at the graveyard gate says the footpath behind it "is used" |
| Spine steps were bare nouns: "Skeleton put down", "Small Manashield Potion", "Rat Meat", "The Headmaster", "Iron Knuckles", "Find the garden", "The Snake" | A2: no place in them | Each now says where: "The bones walking Julie's fence line", "Rat meat, from the rats in Julie's cellar", "The Headmaster, down the gold mine" ... |
| **Top Field** 6 to 7 half-screens from any road on two seeds; nothing there said "Top Field" | The farmer said "up off the road" and nothing else. She would walk the road looking for it | A **farm track** (`paths.json` `top_field_track`) from the farm to the scarecrow on the rim; a fingerpost at its foot (TOP FIELD, by the farm track); a sign at the rim: "LOWFIELD FARM. TOP FIELD. Turnips." (they are not turnips) |
| **Quarry Steps** out of sight of any road on two seeds; nothing at it said its name | "The side facing this road" is not a direction | A **quarry track** from the mine yard to the gang's camp, a fingerpost at the mine end, "QUARRY STEPS." at the head of the tally slate, the Company notice names the track |
| **The allotments** out of sight of a road on one seed; nothing there said "allotments" | "Out past the last house" has as many answers as Castle has edges | A **cinder path** out of Castle to the allotments with an ALLOTMENTS fingerpost where it leaves the town; a Society sign by the shed ("Plot 9 is not let") |
| **The nurse's coat** 6 half-screens or more into the wood, off any path | "A hollow tree in the wood behind the car": a wood is all trees | **Her prints**: a one-cell trodden line from the car to the coat; the glovebox says "Follow them"; the step says "at the end of her prints" |
| Nothing at **Sallow Bottom** said Sallow Bottom; nothing at the jetty said jetty | Garden book and nurse's list both send her there by name | A parish sign at the jetty: "SALLOW BOTTOM. PARISH JETTY ... Please do not pick the white roses after dark." (the rose omen, now written where the roses are) |
| Nothing in **the Long Hedge** said Long Hedge | Three quests use the name | A waymark on Footpath No. 3 in the hedge: "PUBLIC FOOTPATH No. 3. THE LONG HEDGE." |
| The Misses Crane's step never said "Crane" | She could not tell it was theirs | The card under the door is signed "M. AND E. CRANE." |
| No. 13 and No. 15 were only "the next lamp" | Checked against No. 12, they were 3 to 6 half-screens away | Each lamp is its own landmark; each has its number on a plate (and now a label) |
| The Lost Property tin 1,540 to 1,990 steps from the book | Over the "near" budget | Deliberate: two of three lie on the road she came in by, and the third is one road on. Tier "far", reason written |
| "The crate is at the north end of the platform" | A compass word in a chunk being rebuilt | "Be on the platform when it goes; the parcel is left on the crate there" |

Not changed, on purpose: no quest text uses a compass bearing or a distance in metres. Both change per seed (`QUESTS.md` K4). Directions are always "the X road", "at the Y", "up the Z track", and the world guarantees the X, Y and Z.

---

## 4. The tree

```
NEW GAME (the platform, Castle Halt)
 |
 +-- [spine] A Letter from Julie --(stoop)--> The Thing in the Yard --(key)--> Julie's Kitchen
 |        --> Dust, Water, Pansy (note on the table) --> What Snakes Eat (dog)
 |        --> Gnox Goldskin's Mine (dog) --> The Snake Below (dog) --> [the Waters]
 |
 +-- A Lost Property (book on the platform) --> A Left Luggage --> A To Be Collected
 |                                                ^
 |                                     H If Found (haversack) hands in here
 +-- F Before the Bell (garden book, Julie's yard) --> F Rose and Stone (--> the dog)
 |
 +-- B/H Parish board, Castle square: Footpath No. 3 / Rats in the Sheds --> Plot 9
 |                                          |
 |                          H If Found (found in the hollow tree on the path)
 +-- C Farmhouse door, Lowfield Farm: Three Scarecrows --> After the Bell --> Not Turnips
 +-- E Pell's stone (last lit lamp): No. 14 --> The Lampman's Brazier (planks from the car)
 +-- D Car in the wood (glovebox): The Nurse's Round --> Her Coat --> Mrs Allen's Dressing (door in Castle square)
 +-- G Company notice (mine yard): Stood Down --> Down at Five: 4 (tally slate, gang's camp)
```

Chains are independent; only the order inside a chain is fixed. Walks below: **seed 3 / seed 2026**, steps from the giver.

---

## 5. The main line (`data/quests.json`)

### A Letter from Julie
- **Unlocked by:** New Game. **Giver:** the letter in her bag (she starts with it). **Hand-in:** arriving on Julie's stoop does it. **Pays:** nothing; opens the dog.

| Step | Where | She is thinking | Could confuse | Verdict |
| --- | --- | --- | --- | --- |
| "Auntie Julie's house, at the end of the station road" | The stoop; the station road leaves the platform and ends at Julie's gate (41 steps from where the trigger sits, 800 to 1,400 m from the platform) | "One road out of the station. Follow it." The letter says the map ends at a house and a dog | Nothing: the station has one road, and the glove and hat of Lost Property lie beside it | OK |

### The Thing in the Yard
- **Unlocked by:** talking to the dog on the step (intro). **Giver / hand-in:** the dog, Julie's step. **Pays:** the house key; opens Julie's Kitchen.

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "The bones walking Julie's fence line" | The yard skeleton, inside Julie's fence, 41 steps from the dog | "The dog said bones on her fence line. There." | Was "Skeleton put down": any skeleton anywhere counts (it still does: `kill skeleton` is by kind), but now she knows which one was meant | Fixed (A2) |

### Julie's Kitchen
- **Unlocked by:** handing in the bones. **Hand-in:** standing in the kitchen does it. Banner "Julie's House" confirms.

| "Stand in Julie's kitchen" | Through Julie's door (the key she was just given) | "The key is for the door behind the dog" | Nothing | OK |
| --- | --- | --- | --- | --- |

### Dust, Water, Pansy
- **Unlocked by:** reading the note on the kitchen table. **Hand-in:** the dog ("The dog will want to know"; returnTo "the dog, on Julie's step").

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "A Manashield potion, made at the bench in Julie's kitchen" | The bench, in the kitchen; the pansy is in the pantry chest (the note says so) | "Bench, bags open, three things" | Was "Small Manashield Potion" with no place, and the hand-in (the dog) was named nowhere | Fixed (A2, D2) |

### What Snakes Eat
- **Unlocked by:** the dog, after the kitchen. **Hand-in:** the dog.

| "Rat meat, from the rats in Julie's cellar" | The cellar, down either hatch in the kitchen (banner "Julie's Cellar") | "Under the house. The hatches." | Was "Rat Meat", and the description said "and in the mine", which is 2,000 steps off | Fixed (A2) |
| --- | --- | --- | --- | --- |

### Gnox Goldskin's Mine
- **Unlocked by:** the dog, after the rats. **Hand-in:** the dog. **Tier:** far (2,479 / 1,919 steps).

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "The Gold Mine, at the end of the mine road" | Through Castle, out along the mine road to its end; the mouth has a lamp and the GOLDSKIN MINING Co. sign | "Mine road, out of Castle, to the end" | **Was wrong**: "the mine mouth in the yard". She would have searched the yard | Fixed |
| "The Headmaster, down the gold mine" | Inside (banner "Goldskin Mine") | "Deeper in" | Was a bare name | Fixed (A2) |
| "Iron Knuckles, down the gold mine" | Inside | | Was a bare name | Fixed (A2) |

### The Snake Below
- **Unlocked by:** the dog, after the mine. **Hand-in:** the dog. **Tier:** far (1,487 / 1,663 steps).

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "The standing stone, on the footpath from the graveyard" | Through Castle, the graveyard road; the graveyard sign says the footpath behind it "is used"; the path ends at a stone that says DO NOT and the way down | "Graveyard, the path behind it, the stone" | **Was wrong**: "the other stair". There is no other stair; she would have searched the cellar | Fixed |
| "The garden, inside the burial chamber" | Inside (banner "Burial Chamber") | | Was "Find the garden" | Fixed |
| "The big snake, in its own room in the burial chamber" | Inside | | Was "The Snake" | Fixed |

---

## 6. The Lowfields side quests (`data/quests/lowfields.json`)

### Chain A. Lost Property (the book on the platform, Castle Halt)

**A1 Lost Property.** Offered from the first minute by a table labelled "Lost Property" under the platform lamp. Hand-in: the same book. Pays apples, and the unclaimed shelf beside the book then lets her take **one article** of three (a lamp stone, two bottles of water tied together, a bag of grapes): a shelf with a card, not a chest. The book's directions are the road's, not Julie's gate. Tier near.

| Step | Where (seed 3 / 2026) | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "A red glove, at the well on the station road" | Beside the station road, on screen from it (0.5 / 0.9 half-screens). 753 / 1,350 steps. The well's plate says "THE WELL ON THE STATION ROAD" and the prompt says "Pick up: A red glove" | "I walked this road. Was there a well?" Often she already has it (endowed progress) | The well used to be a cobble pad with no well (the old confusion); now a well head and a plate | OK |
| "A felt hat, at the signpost on the station road" | Beside the same road, 1,019 / 501; lies at the foot of the post (`beside`) | "The signpost I passed" | Glove and hat order differs per seed; order is free | OK |
| "A dinner tin, by the cart on the station road" | In the road beside the cart (`beside`), further along the station road | "The cart I passed" | | OK |
| **Back to** the lost-property book, on the platform at Castle Halt | 1,987 / 1,621 back | "Back where I started" | | OK |

**A2 Left Luggage.** The book's next page, once A1 is done. The trunk (label "The trunk") has stood locked on the platform since the first minute. Cross tier.

| "The carter's cart, on the mine road near the mine" | 150 to 380 m short of the mine on the mine road: 2,576 / 1,853 steps. Cart labelled "The carter's cart"; its tree says "on the mine road"; picking up the key completes it | "Mine road, near the mine: a cart" | The note ("the quarry turning") points at chain G; the quarry track is now that turning | OK |
| --- | --- | --- | --- | --- |
| "Open the trunk on the platform at Castle Halt" | The trunk. Opening it hands the quest in | "The key is for the trunk I walked past" | | OK |

**A3 To Be Collected.** The book, after A2. Completes by standing on the platform after nine.

| "The platform at Castle Halt, after the nine o'clock bell" | The platform rect, 0 steps | "Be here after nine" | She never sees the parcel arrive; that is the point, and the book says it is always collected | OK |
| --- | --- | --- | --- | --- |

### Chain B. The Allotments (the parish board, Castle square)

The board shows one notice at a time (Footpath No. 3 first, then the rats, then Plot 9). Hand-in: the board.

**B1 Rats in the Sheds.** Walk 477 / 162 steps.

| "Rats, in the allotments out past the last house" | The allotments: four rats by the Society's shed, four on the cinder path; the new ALLOTMENTS fingerpost stands where the cinder path leaves Castle | "Which last house?" is answered by the fingerpost | Any rat anywhere counts (the Society does not care where); on seed 2026 the nearest counted rat is a wild one by the allotments | Fixed (C1 on one seed, A3) |
| --- | --- | --- | --- | --- |

**B2 Plot 9.** After B1. 550 / 729 steps.

| "Whoever digs Plot 9, in the allotments, after the bell" | The stake says "PLOT 9"; the Society sign says "Plot 9 is not let"; the Tenant appears only after nine | "At night, at the stake with no name" | By day there is nobody; the wait line says so | OK |
| --- | --- | --- | --- | --- |

### Chain C. Lowfield Farm (the farmhouse door)

Giver and hand-in for all three: "The farmhouse door" (the farmer will not open it).

**C1 Three Scarecrows.** By day.

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "The scarecrow on the road by the farm gate" | 60 to 150 m from the farm on the Castle road: 152 / 80 steps | "On the road I came in by" | | OK |
| "The scarecrow in the Long Hedge, off the footpath" | In the Long Hedge; Footpath No. 3 runs through it with a waymark saying THE LONG HEDGE: 1,111 / 1,217 | "The footpath to Castle. The hedge." | "Off the footpath" is up to a screen off it; the waymark says she is in the right hedge | Fixed (A3) |
| "The scarecrow on the rim of the Top Field, up the farm track" | Up the new farm track from the farm; the track ends at the scarecrow and the TOP FIELD sign: 169 / 369 | "Track from the farm, the field with the rows" | **Was**: "up off the road", 6 to 7 half-screens from any road on two seeds, and nothing said Top Field | Fixed (C1, A3) |

**C2 After the Bell.** The same three after nine; the steps say "after the bell". Same walks. If the scarecrow omen is true the top one stands 32 to 70 m nearer the farm at night; it still counts.

**C3 Not Turnips.** Six of seven Field Pumpkins in the Top Field, in daylight: 374 / 515 steps up the track. Warned three times.

### Chain D. The Nurse's Round (the car in the wood)

The car is off the road in a wood; a signpost at a lay-by on the wood road says "A district nurse parked here and walked in". The giver is the car's glovebox (once its planks are taken).

**D1 The Nurse's Round.** Tier far. Hand-in: the glovebox.

| Step | Where | Thinking | Confuse | Verdict |
| --- | --- | --- | --- | --- |
| "Mrs Allen's cottage, on the road by Sallow Bottom" | The roadside cottage nearest Sallow Bottom; the note is signed E. Allen; the jetty sign says SALLOW BOTTOM: 1,944 / 798 | "Sallow Bottom: the water" | Nothing used to say Sallow Bottom anywhere near | Fixed (A3) |
| "Mr Pike's cottage, on the road by the Long Hedge" | 1,290 / 823; note signed PIKE | | | Fixed (A3, by the waymark) |
| "The Misses Crane, on the road under Quarry Steps" | 1,811 / 973; eleven parcels; the card is now signed M. AND E. CRANE | | The step never said their name was there | Fixed (A3) |

**D2 Her Coat.** The glovebox's second sheet.

| "Her coat, at the end of her prints into the wood behind the car" | Her prints are a trodden line from the car to the hollow tree: 192 / 209 steps. Picking up "The nurse's coat, on a hollow tree" completes it | "Follow the prints" | **Was**: "a hollow tree in the wood", 6 half-screens or more off any path, in a wood that is all trees | Fixed (C1) |
| --- | --- | --- | --- | --- |
| "Her case, on the ground by the car" | The case (label "The nurse's case"); opening it hands in and gives D3 | | | OK |

**D3 Mrs Allen's Dressing.** Given by the case, with the parcel already in hand. **Back to** Mrs Allen's sister's door, facing the fire in Castle square: 1,415 / 812 steps. The door is labelled "Mrs Allen's sister's door".

### Chain E. The Lampman (Pell's stone, by the last lit lamp)

The stone is found, not sent to: it stands by the last lit lamp of the longest Lowfields road that is not the first walk.

**E1 No. 14.** After the bell. Hand-in: the stone.

| "No. 12, the first dark lamp past Pell's stone, at night" | 30 / 33 steps; plate "No. 12" | | | OK |
| --- | --- | --- | --- | --- |
| "No. 13, the next lamp along the same road, at night" | 104 / 104 | | The audit used to measure it against No. 12 | OK |
| "No. 15, the last dark lamp on the same road, at night" | 177 / 93 | | | OK |

**E2 The Lampman's Brazier.** Two planks and a fire stone: "There are two in the car in the wood" (1,430 / 375 steps). Hand-in: the cold brazier past No. 15, which lights and becomes a fire. The car's planks are also the mine's stair wood: the decision.

### Chain F. The Garden Book (Julie's yard)

**F1 Before the Bell.** The garden book by the barrel east of the stoop. Tier far (1,613 / 1,805).

| "White water roses, below the jetty at Sallow Bottom" | Five roses round the jetty; the new parish sign there says SALLOW BOTTOM and "please do not pick the white roses after dark". The book adds: Mrs Allen's cottage is the nearest door | "Past Castle, the water, the jetty" | Nothing at the place used to say its name | Fixed (A3) |
| --- | --- | --- | --- | --- |

**F2 Rose and Stone.** Make Stone Skin at the bench, **show the dog** (returnTo says "by day": the step is empty nine to six, and the book says so at night).

### Chain G. The Company (the mine yard)

**G1 Stood Down.** The Company notice at the mine mouth. 502 / 531 steps.

| "Men working Quarry Steps, up the quarry track from the mine" | Up the new quarry track (fingerpost "QUARRY STEPS, by the quarry track") to the gang's camp and the face; six quarrymen | "The track out of the mine yard" | **Was**: "the side facing this road", out of sight of every road on two seeds | Fixed (C1, A3) |
| --- | --- | --- | --- | --- |

**G2 Down at Five: 4.** The tally slate at the camp (now headed QUARRY STEPS). "The top of Quarry Steps, up from the gang's camp": 133 / 131 steps. Hand-in: the slate.

### Chain H. The Right of Way

**H1 Footpath No. 3.** The parish board. "The stile on Footpath No. 3, outside Castle" (114 / 157), then "... by Lowfield Farm" (1,265 / 1,047). **Back to** the fingerpost at the Lowfield Farm stile (0 steps: she is standing at it).

**H2 If Found.** Found, not offered: the haversack in the hollow tree on the footpath ("Pick up: A walker's haversack, in a hollow tree"). **Back to** the lost-property book at Castle Halt: 1,945 / 1,182 steps, tier far. This is the breadcrumb west.

---

## 7. The School, in words

The School's on-screen overlay is gone; it looms through text instead. Six touches in the Lowfields, each earned by where it falls, none an opener. The School is north of every Lowfields place on every seed measured (3, 2026, 77, 5, 11), so "north" is the one bearing the text may use about it; nothing gives directions *by* it that depend on which way she is walking.

| Where | When she reads it | Line |
| --- | --- | --- |
| The well on the station road (A1, the glove) | The first evening's walk, before she knows its name | "Away north over the fields there is a big building on a hill, with one window lit. It is the only light in that direction." |
| The dog's burial offer (spine) | Late in the spine; a direction given by it | "...off the graveyard road: the one that goes north, toward the School. Do not go as far as the School." |
| The farmer's door, "After the Bell" wait line | Asked again, after he has sent her out at night | "I sit in the back kitchen, the side away from the School. You can see it from the front. I do not need to see it." |
| Mrs Allen's door, after the dressing | Only once the chain is done | "Is there a light on up at the School tonight? No, do not tell me. My sister says I am not to ask." |
| Lamp No. 15, at night (E1) | The end of the dead run, after the bell | "...The only light you can see from here is one window of the School, a long way north on its hill." |
| The parish board, once every notice is done | The idle board | "One notice has been taken down in a hurry. The corner still pinned to the board says SCHOOL." |

The red glove's name tape (it says SCHOOL and nothing else) was already there, and now sits beside the first of these.

## 8. What is still weak (judgement, not yet a failing rule)

- **Sites off the road are found by exploring.** The car in the wood and Pell's stone are givers nobody sends her to. That is intended (`QUESTS.md` K22), but a lay-by signpost is the only hint to the car. B2's roadside signposts at forks would help; a fork sign naming "the wood road" is the natural next step.
- **"The road by Sallow Bottom", "the road under Quarry Steps"** name a road by a place, not by its ends. They are true (each cottage is the roadside spot nearest its patch), but the audit only proves the cottage is near the patch, not that the road is obvious. Fingerposts at forks naming patches would close this.
- **Any rat counts** for Rats in the Sheds, and any skeleton for The Thing in the Yard. Harmless (the text now says where), but a kill step tied to a place wants its own unit row, as the pumpkins and quarrymen have.
- **The mine and burial interiors** are audited only as "inside the zone". Their own layouts are the dungeon tests' business.

## 9. Needs from other owners

- `sim/catalog.ts`: add `returnTo?: string` to `QuestDef`. The UI reads it through a local cast today.
- World (B1/B2): keep the marks `graveyard_gate`, `allotment_shed`, and the sites `farm`, `gold_mine`, `town`, `car_wood` that the new tracks and signs are placed by; keep one road from the station to Julie's, and Julie's to Castle, and the mine and graveyard roads leaving Castle (the dog's directions depend on it). The audit will say by name if any of that stops being true.

---

## 10. Tales (`data/stories/tales.json`, `data/quests/tales.json`)

*Added 24 September 2026, after John asked for "real mini stories in the world with NPCs that kind of obviously are more than the usual ... 2-3 deep, a little bit gripping, not all the same."*

Nine short stories, each with one person at it who is plainly more than a neighbour: a drawing of their own (`art/units/tales.ts`), a voice of their own, and a place that shows what happened before they say a word. Each goes two or three steps, and the second step changes what the first one meant. Seven end on a choice, and both ways leave something changed in the world: a door left open or boarded again, a person gone the next morning, a lamp lit red or green.

**What was already told, so these are not.** Before choosing, the 26 main and Lowfields quests, the 16 in Castle and the 36 country stories were listed by premise. The country stories are mostly *find my lost thing* (spectacles, bill-hook, key, scissors, clock key, ring, coat, glove, bag), *take this to someone* (windfalls, tea, kindling, letters, loaves, roses, apples), *knock and tell him his tea is ready* (Mr Leckie, Robert, the lodger), *look and report* (the ruin's candles, the bell ringing twice, the name in the stump, the memorial), *count something* (scarecrows, lanes, paces, lamps) and *clear a camp*. None of the tales below is any of those.

**How they are placed.** Every tale claims a ruin (the county has hundreds, in all three regions, near roads or reached by a laid footpath), so they never take a cottage or a farm from a country story and they land on every seed: **20 of 20 on seeds 1 to 20, all nine, with everything they put down reachable on foot** (`test/tales.test.ts` measures it). A tale's place has its own name, the same on every seed, and its own words on the board. A tale is only given a place whose kept cells she can walk to from the platform (stories.ts `walkable`, things standing counted), and its things are set down by position in the place (`at: { dx, dy, room, on }`, placements.ts) on open ground next to ground she can already reach. Nothing a tale sets down draws on the county's dice (a footpath laid to its place can still move what the country scatters afterwards, as any story's can), and on seeds 1 to 20 the tales cause no county to be re-rolled.

| | Tale | Where | Who | Hours | Tone | Verbs |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Telling the Bees | Coram's, the Lowfields | Miss Hanney, in a bee veil | day, then after the bell | tender, uneasy | knock on hives, wait for night, tie crepe |
| 2 | The Bell on His Ankle | Whinmoor Cottage, the Lowfields | Mrs Pollard in curlers; Mr Pollard in pyjamas, a bell on his ankle | after the bell | tender | follow him, read what is on the stone |
| 3 | Horace | Sundial Cottage, the Lowfields | Mr Pargeter, a panama, ninety-year-old tortoise | day | funny-sad | follow a trail of bitten dandelions, push a stone |
| 4 | Three Stones | Coker's Wall, the Lowfields | Mr Coker, a waller in a leather apron | day | dark-funny | carry stones |
| 5 | The Boarded House | Sluice Cottage, the Waters | Mr Rook, barefoot, a hammer; Mary, a voice behind boards | day, then after the bell | tense | look at the back boards, listen at the door, pull the boards |
| 6 | The Spanner | The Intake, the Waters | Mr Denholm, the Company's diver, in his helmet | day | funny-sad, uneasy | push the stone off the key, open the chest |
| 7 | The Last Rows | Tenter Cottage, the Waters | Mrs Wakes, in her rocking chair, knitting | day | eerie, kind | follow the scarf, read it, pull a row out |
| 8 | Lamp Oil / The Red Lamp | Brickfield Halt, the Works | Mr Voysey, a railwayman's cap; Mrs Voysey with her case | day, then after the bell | sad | push a barrel off a locker, light a lamp |
| 9 | Vermin / Under the Hatch | The Weighbridge, the Works | Mr Sorrell, the vermin man, his pole over his shoulder | day, then after the bell | funny, then tense | kill, then feed or fight |

The School looms in four of them, lightly and never explained: the Pollards' Tom "is up at the School"; Mrs Wakes's scarf "is not the School's register"; Horace walks north every autumn; the dead come up in two more (the man at the hives, Coker's headstones: "not since they all got up and went walking").

### 1. Telling the Bees (Coram's)

In the yard: three white hives on legs, a bee veil on a stake by the door with the net damp as if it had been out in the dew. Miss Hanney, in a veil of her own, says Mr Coram died on Friday and somebody must tell the bees or they will leave; it ought to be family, and there is none.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "Knock on the three hives at Coram's and tell them" | the three hives | knock on each (USE) | at the third, lifting the roof, comb freshly cut from the frames |
| "Whoever cuts the comb at Coram's, after the bell" | among the hives, after nine | wait for night | a man in a bee veil cutting comb without a light: "That's the name on the board." |
| "Black crepe round the three hives at Coram's" | the hives | tie crepe on each (USE, with the crepe) | the hives in black |

**Turn:** the errand for a dead man's bees is an errand for the dead man, who is still keeping them. **Choice** (Miss Hanney, after the second): *give me the crepe* (tied on, the hives change; nobody is at the hives after the bell again; his veil is taken in off the stake) or *leave him to it* (from the next morning a jar of honey stands on his step with HANNEY on the lid, and he is at the hives every night). Returns: Miss Hanney, by day.

### 2. The Bell on His Ankle (Whinmoor Cottage)

A lived-in cottage; outside the door a kitchen chair with a blanket, the ground under it worn to earth; a man's carpet slippers by the step, worn through at the toes, grass seed in them (gone from the step at night). Mrs Pollard, in curlers and a dressing gown, says her husband walks in his sleep after the bell, and she ties a bell on his ankle to hear him go.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "Where Mr Pollard walks to from Whinmoor Cottage, after the bell" | a flat stone north of the house, the Dole Stone | follow him (he goes when spoken to, and by himself at ten) | a dinner set on the stone under a cloth; he is awake: "It's for our Tom. He's up at the School." |
| "Mrs Pollard's plate, off the stone north of Whinmoor Cottage" | the Dole Stone, by day | take the plate | the plate wiped clean under the cloth |

**Turn:** he is not asleep and never was. **Choice** (Mrs Pollard): *bring the plate back* (then from that night he sits in the chair by his door and listens for the bell, and the stone is bare) or *let him go on* (from the next morning a second plate stands by the first, bread and dripping cut into soldiers).

### 3. Horace (Sundial Cottage)

The board says *Please mind the tortoise.* In the yard a tea chest on its side, straw pressed in a tortoise-sized round, the front knocked out; a flat stone beside it. Mr Pargeter has lost Horace, ninety, his father's.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "Horace, at the end of the bitten dandelions by Sundial Cottage" | four bitten dandelions north from the house, and Horace at the end | follow the trail, pick him up | "Horace, facing north" |
| "The flat stone against Horace's box at Sundial Cottage" | the front of the box | push the stone (hold USE) against the front | Horace in his box, nose to the stone |

**Turn:** he was not lost, he was going somewhere: north, every autumn. **Ending:** Horace sits in his box with his nose to the stone. "North's the other way, but he doesn't know that. Or he does, and he's being patient."

### 4. Three Stones (Coker's Wall)

A knee-high dry-stone wall running out from a ruin, going nowhere in particular, very straight; a heap of flat dressed stones by the walls; a tin mug of tea gone cold. One stone in the wall has letters on the face that meets the next: LOVING MEM.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "Three stones carried to the end of Coker's Wall" | the heap, then the gap at the end of the wall | carry each (USE to lift, USE to set down) | three set in the gap |

**Turn:** "You'll have seen what they are. Face down, they're only stone. Nobody's using them now. Not since they all got up and went walking." **Choice:** *leave them in* or *stand them back up* (three headstones stand by the heap, faces out, the second date this year; he says he'll find more).

### 5. The Boarded House (Sluice Cottage)

A cottage with planks nailed across both windows and the door. Mr Rook, in shirt-sleeves and braces, a claw hammer, barefoot; on the step a pair of men's boots full to the laces with black water. The board says *Please do not knock.* He says his Mary was out after the bell on Sunday and came back wrong.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "The boards over the back window at Sluice Cottage" | the boarded window at the side | look | every nail worked half out from the inside |
| "Mary Rook, through the boards at Sluice Cottage, after the bell" | the boarded door, after nine (given by the boots: "Ask Mary, after the bell") | knock, listen | "Look at his feet. He came in on Sunday without his boots, and he's not put them on since. He can't. They don't fit him now." |

**Turn:** each says the other came back wrong, and both have a thing to show for it. **Choice** at the door: *pull the boards off* (the door stands open a hand's width; next morning Mr Rook and his boots are gone and Mary stands in the yard: "He went off before it was light. Barefoot.") or *leave them* (next morning new planks and a tin of nails by the door; "Another night, then.").

### 6. The Spanner (the Intake)

GOLDSKIN MINING Co. DIVER'S STATION. NO BATHING. Mr Denholm, in a brass helmet and a canvas suit, sits by an air pump whose hose is dry. "You'll have to speak up. There's a good deal of glass between us." He wants his spanner out of his tool chest; the key is where he always leaves it, under the flat stone by the step.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "The key under the flat stone at the Intake" | the flat stone | push it off (hold USE); once he has asked, a small iron key where it lay (`hides`, quest-gated) | the key (a real key, drawn as one) |
| "The spanner in the tool chest at the Intake" | the chest in the ruin | unlock, open | the spanner, and a Company chit: DIVER DENHOLM TO SURFACE AND REPORT |

**Turn:** he does not want the helmet off. He wants it tightened: somebody is knocking at the intake grille, three and a rest and three, and the Company says leave it. **Choice:** *tighten the bolts* (he walks off heavy without looking back; the pump's hose runs off along the ground after him and he is not there again) or *undo the bolts* (the helmet on the ground by the step with lake water in it, and an old man bareheaded who stays: "I can hear it plainer without.").

### 7. The Last Rows (Tenter Cottage)

Mrs Wakes in her rocking chair outside her door, knitting, the scarf running out along the ground from her lap; its far end faded to no colour. She knits a row for everybody who comes up to the door.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "The newest rows of Mrs Wakes's scarf, at Tenter Cottage" | the scarf over the arm of her chair | read it | SAM ILES. ADA ILES. THE CONSTABLE. MR DUNN. The last row half done: {name} |
| "Your row, on the scarf over her chair at Tenter Cottage" | the same | tell her to finish it, or pull it out | |

**Turn:** it is a register, and you are in it. "It's not the School's register. It's only mine, and it's warmer." **Choice:** *finish it* (she gives you a short scarf with your name in the corner) or *pull it out* (a tangle of pulled wool by her chair, a gap in the scarf she will not knit over).

### 8. The Red Lamp (Brickfield Halt)

GOLDSKIN MINING Co. LIGHT RAILWAY. REQUEST STOP. A length of track, a bench, a timetable ("To stop the train, show a green lamp after the bell. A red lamp tells the driver to run through."), two dark signal lamps. Mr Voysey keeps the lamps though nothing stops. Mrs Voysey stands by the bench with her suitcase packed.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "The oil in the lamp locker at Brickfield Halt" | the lamp locker by the walls, a barrel rolled against it | push the barrel off; the locker opens | a can of paraffin, "enough to fill one signal lamp" |
| "A signal lamp lit at Brickfield Halt, after the bell" | the red lamp or the green | light one | |

**Turn:** the errand is to keep his wife from leaving. "He'll want red. Red, and it runs through. I've a ticket. It's only a single." **Choice:** light the *red* (it burns every night after; she is still by the bench: "Next Sunday, then.") or the *green* (it burns green; she is gone in the morning, and he says he heard it stop).

### 9. Under the Hatch (the Weighbridge)

Web spinners in the walls, two empty cage traps with the doors tied open, a hatch in the floor bolted from the outside, the wood round it scored from underneath. Mr Sorrell, the Company's vermin man, with his catch tied on a pole.

| Step | Where | Verb | What she finds |
| --- | --- | --- | --- |
| "Web spinners in the walls at the Weighbridge" | round the walls (five stand, four asked) | kill | |
| "The hatch in the floor at the Weighbridge, after the bell" | the hatch, after nine | feed it the scrag end he gives you, or draw the bolt | something pushing up; it is quiet, or it comes up and is put down |

**Turn:** the vermin were its supper; clearing them is what makes it come up. **Choice:** *meat down the hatch* (bolted and quiet; he goes back to spinners) or *draw the bolt* (the hatch open over a hole in wet stone with nothing in it; his pole leant on the wall: "I'll finish the week.").

### What the audit and the tests hold them to

- The quest audit (rules A to E) covers every tale quest on seeds 3, 2026 and 77, like any other; a plate's `release` list (the stone pushed off) now counts as something that shows a thing.
- `test/tales.test.ts`: the count and depth, fixed names clear of every board name, VOICE.md's hard rules, at least half turning on a physical verb; every tale landing whole and reachable on seeds 1 to 20; and every tale played end to end on seeds 3 and 2026, one branch of its choice on each, with the stone pushed, the stones carried and Mr Pollard followed, and the ending checked in the world.
