# The Bell at Nine: the Night

The county after nine, as a second world. Pair with `STORY.md` (what is true: "By day Castle is the town that was. By night it is what it became."), `WORLD.md` §2 and §4.3 (the clock and what night does today), `VOICE.md` v2 (how the night's words are written), `PLAN.md` §2.6 (threat, the lamps, night-locked doors), `ART.md` and `PRESENTATION.md` (how it is drawn and heard) and `PORT.md` §13 (what the PSP can hold).

**Status: design and plan for the owner's review, 9 October 2026. Nothing here is built.** Every proposed change to the story is marked **STORY CHANGE** with its reason; every change to how the world is generated is marked **WORLDGEN**. The questions for the owner are §10; every one has a default, so the plan can start on the defaults.

**The owner's brief (9 October 2026).** After the bell at nine the county becomes a second world, "like Silent Hill changes, and that worked through the game, but not as extreme, half way there". All four layers change: the words, how places look, how they play, and the sound. The bell is the switch. Nights are much harsher: a place you survive and get through to a fire or a bed, still playable, with some things found only then. Gore occasionally, only with a reason, about Warcraft III; never harm to children, never self-harm. The PSP gets the whole night world within its budget. The night deepens as the story goes on and is tied to it. Worldgen and story may be reworked, shared across platforms (owner, same day).

---

## 1. The concept

At nine the School's bell rings the county onto the Company's night shift, and for nine hours Castle is the place Gnox Goldskin's Company made of it: the same streets and fields, but owned. Rust runs down the plaster from every sill. Windows that had geraniums at four are boarded, and some are bricked. The council's notices have the Company's pasted over them, in the same register. Coats hang along the railings in the square, a row of them, as on the School's pegs. Someone has chalked a register on the cobbles, five strokes at a time. The lamps that are still kept burn warm, and the ones that are not burn a cold, wrong colour that nothing is kept off by. The crickets stop. Under everything, a long way off, machinery turns. It is not Silent Hill's Otherworld. The map is the same map, the people are behind their doors, the light is never black. It is the town with its second town showing through.

**What "halfway" means, concretely.** Five rules, each one a thing a test or a review can hold:

1. **The ground plan holds.** At least 95 per cent of every zone's cells are the same tiles at night as by day. What changes in the plan is a short, authored list per place (§5.1), never a different map. A player who learned the town by day can walk it at night.
2. **Materials change, shapes do not.** The night look is the day's sprites and terrain through a night albedo LUT (rust, soot, stain, cold glass: §4.2), plus overlays laid over known geometry (boards on windows, rust runs, coats, chalk: §4.3). No sprite is redrawn as a different object.
3. **It is never dark for its own sake.** `ART.md` §3.1, "night is beautiful, not dark", still holds at every stage: unlit ground keeps readable value; the night grade darkens the mid-tones toward umber, it does not crush the darks.
4. **People keep their distance.** Nobody is turned into a monster on screen. The townsfolk are behind their doors; their knock lines are what changes. What walks is what already walks: the undead of the night shift.
5. **It is the Company's, not Hell's.** The night's whole visual and sound language comes from inside the story: iron, rust, soot, time-clocks, registers, numbered pegs, Company notices, the bell. No pentagrams, no flesh walls, no static. If an image could not have come out of the Works or the School, it does not belong in the night.

**What the night is, in the story.** **STORY CHANGE (1): the night world is the shield failing, made visible.** `STORY.md` §2.5 says Julie's shield is why "the Lowfields still look normal and the lamps still work", and §2.7 that it is failing. The night world is what shows where the shield no longer reaches: each stage of the night (§3) is the shield pulling in. The lamps that burn the wrong colour are the lamps outside it. Julie's house and yard never change, at any stage (the shield's last room), until the ending. *Why:* it makes the escalation a fact of the story rather than a difficulty dial, ties the lamps (the dog's poke chain counted fourteen going out) to Julie, and gives the observant player one more tell about the dog: the one house the night never touches is the one the dog will not stay at after nine. Nothing says so.

---

## 2. The switch

### 2.1 Dusk, before (18:30 to 21:00)

Dusk is gradual and changes **light and sound only**, never a sprite or a word: the lamps come on as built (18:30), the grade cools (`PRESENTATION.md` §1.9, as built), the town goes in street by street. From stage 2 (§3) a single low drone fades in under the evening music from 20:30 (the bed `drone`, §6), so the ear knows before the eye. Nothing boarded, nothing hung, no text changed before the bell: the bell is the switch, so there is nothing for the dusk to give away.

### 2.2 The bell (21:00): the turn

The clock row rings nine strokes from the School's door, 2.5 s apart (as built: `data/clock.json`, `EventKind::Bell`). **The turn is the first four seconds after the first stroke**, on every tier, driven by the sim's tick so every seat sees the same frame at the same tick:

| Time after the first stroke | On screen | In the ears |
| --- | --- | --- |
| 0.0 to 0.4 s | **The gutter.** Every lamp, lit window and spell glow in view dips to a fifth of itself; the ambient falls to its floor (20 per cent of the night's, never black). The kept lights do **not** gutter: Julie's yard lamp, the hubs' lamps and every fire burn on, the only warm things on the screen. Units and her target ring stay drawn (as silhouettes in the gutter's dark) so no fight is made blind | The stroke, `bell_near` / `bell_within` / `bell_far` as built; its hum partial does not decay but is held and swelled (the `turn_hold` layer, §6.2) |
| 0.4 to 1.4 s | Held dark. **Everything swaps here**: the night LUT goes in (CLUTs and chunks, §4.2), the overlays appear (§4.3), night props are shown and day props hidden (§5.3, the one exception to "nothing changes in view", §5.6), the wrong lamps are chosen | The held hum; from stage 2 a low iron slam a long way off (a shutter coming down), from stage 3 machinery spinning up under it |
| 1.4 to 4.0 s | **The light comes back from the north**, a band of returning light moving down the screen from the top edge (where the School is) to the bottom over 2.6 s. Kept lamps return warm, wrong lamps return cold. The grade eases to the stage's night grade behind the band | The `Bell` hush cue as built (6 dB under), into the night cue for the stage (§6.1) |
| strokes 2 to 9 | The night world | The remaining strokes |

It is short and unmistakable: the screen blinks out but for the warm lights, and the county comes back changed from the hill down. It teaches the night's one rule in its first second: **what stays lit in the gutter is safe**.

**When the bell does not ring.** On a Tuesday with `omen:early_bell` true, the bell rings at 20:50 and the turn comes with it (the night then begins at 20:50 for the look and the words; the sim's night, doors and spawns keep 21:00 as built, so the omen stays a sound and a look and never a rule change). Once `bell_stopped` is set (the Timekeeper down), nothing rings at nine: the music stops for eight seconds as built, and **the turn happens anyway, in silence**, slower (8 s, the gutter's dip shallower, the band of returning light slower). **STORY CHANGE (2): the night still turns without the bell.** *Why:* `STORY.md` already says "the night comes all the same"; this makes it seen, and it is the most frightening version of the switch: the county no longer needs telling.

### 2.3 Dawn, after (05:00 to 06:00)

At 05:00 first light lifts the grade (as built). At six the bell strikes six and the **reverse turn** runs: no gutter, a 3 s band of light from the **south** edge up (the hill last), the LUT and overlays leaving under it, the drone and machinery spinning down, the dawn chorus coming in behind the band. After `bell_stopped`, the same in silence. Nothing night-only remains in view at 06:00:04; a creature caught on a road walks home as built (`WORLD.md` §4.3).

### 2.4 Interiors, dungeons and the house

- **County interiors** (the Arms, the church): the bell is `bell_within`. Indoors there is no band: the room's own lamps gutter and come back (the Arms' fire does not gutter: it is a kept fire), and the room's night variant (§5.1) swaps in the dark second. Windows to the street show the outside's night after.
- **Julie's house and cellar** do not turn. The bell is heard; her lamps do not gutter; nothing in either room changes at any stage. The windows show the county turning outside.
- **Dungeons** (the mine, the Museum, the library, the forest, the pipes, the Factory, the Burial, the School) do not turn: "a dungeon's dark is its own" (`WORLD.md` §4.3), and their music does not change at nine (`PRESENTATION.md` §5.1). They hear the bell. The night changes the way to them, not them.

### 2.5 Co-op

The turn is clock-driven and the sim is lockstep, so every seat in the county sees the same turn at the same tick; a seat indoors sees its room's turn; a seat in a dungeon hears the bell and sees nothing turn. **The stage is the party's** (the world's state, §3.2), never a seat's: two players who have done different amounts of the spine still share one county and one night. A seat that joins at night joins the night as it is, with no turn.

### 2.6 Saving and loading at night

Everything the night changes in the sim is state (the latched stage, night props shown, tile deltas, the time cards, the relieved shift: §5.6), saved and hashed with the rest. The look is derived from it on load: a save made at 23:00 loads into the night world with no turn played. A save made inside the turn's four seconds (a fire's rest at 21:00:02) loads mid-turn: the turn's progress is `tick - turned_at`, derived, so a load continues it exactly. A bed sleeps to 06:00 and wakes after the dawn turn has run (§2.3): she opens her eyes on morning, as built.

---

## 3. Escalation: the four nights

### 3.1 The stages

The night deepens with the spine. Each stage is a step of the shield pulling in (§1) and is named for what the county has become:

| Stage | Name | From (the world's flag) | What it adds, over the stage before |
| --- | --- | --- | --- |
| **N1** | The town goes in | New Game | The turn (§2.2) and the grade; chalk tallies on the roads past the last lamp; the Company's notices on the far side of the river; the Long Hedge footpath open at night (§5.1). The town, the Lowfields' roads, the Halt and Julie's house look as built. Threats as built. Words: the bell's toast, knock lines (built), a handful of signs |
| **N2** | On the books | `mine_quiet` (Iron Knuckles falls; the register is countersigned A. NOONE) | The town turns too: boarded and bricked windows, rust runs, the coats on the square's railings, the Company's notices over the council's in Castle. **Wrong lamps** outside the town's fence beyond the shield's N2 line. **Company Row** opens at night (§5.1). The **time cards** glint (§5.2). The night clerk walks (§5.4). The drone bed. Night threats ×1.25 |
| **N3** | The Works come down the hill | `works_dark` (the Foreman falls; the Factory's gate lamp goes out) | The Works' smoke drifts over the Waters and the Lowfields at night; pipe runs and grates laid over the roads and walls; the School's top floor lit; the machinery bed everywhere outdoors and the "time clock" tick. Wrong lamps to the N3 line. The night shift walks out of the Works in file onto the unlit roads of every region. The Drowned Lane causeway (§5.1). The canteen's night hatch. Company barriers close two side tracks. Night threats ×1.5 |
| **N4** | The Ball is up | `burial_quiet` (Goldskin falls and the Ball comes up) | Everything at its deepest: every lamp outside the hubs and the first walk wrong; no stars (soot); every window of the School lit; crickets, owls and the town's beds silent outdoors; the music thinned to drone and the bell's echo. Night threats ×1.75 and wrong lamps draw creatures (§5.4) |

**STORY CHANGE (3): bringing the Ball up deepens the night.** `STORY.md` §2.2 says the more Goldskin took, the less the county could decide what it was, and the nights got longer. The Ball out of the hill and in her bag is the county least decided of all, so the last stretch, the Burial to the School, is the deepest night. *Why:* the darkest nights fall on the walk to the School, the walk the whole game is toward, and the dog's "it was right when it was written. It is not right now." gains a weight the player feels.

**After `bell_stopped`**, the stage stays N4 and the turn is silent (§2.2). The School's two lamps go out (as built) and its windows go dark with them: the first night in the game with no light at the top of the map.

**After an ending** the presentation closes (`STORY.md` §10). The sim keeps stepping and each ending's county keeps its rule: *hold the shield* sets the stage to N0 (no night world: "the lamps a little brighter"), *the hill* has no night at all (`night_gone`), *the train* leaves it at N4. Nothing is drawn of it, but a save made after the end loads true.

### 3.2 How the stage is kept

- **Latched at the bell.** The clock step computes the stage from the flags (`N1 + mine_quiet + works_dark + burial_quiet`, each 0 or 1, capped by the ending's rule) and writes it into `GameState.night` at the tick the night begins (the bell's first stroke, or 21:00 where no bell rings). It does not change until the next night. A boss killed at 23:00 deepens tomorrow's night, not tonight's: nothing changes in view, and the escalation is felt as "the next night was worse".
- **One field, saved and hashed:** `Night { stage: u8 /* 0 by day */, turned_at: Tick }`. `View::night()` hands it to the presentation; `Condition::NightStage { min }` hands it to content (§7.1).
- **Intensity by place.** A cell's night intensity is the stage moved by where it is, so the gradient across the map reads as the shield's reach: **Julie's house, her yard and the first walk: always 0. A hub inside its fence (Castle, the canteen, the Reedcutters' fire, the Halt): stage - 1. The Lowfields and the Waters: the stage. The Works: stage + 1** (they "were never inside the shield", `STORY.md` §10), all held to 0 to 4. Presentation reads the intensity for the look and the sound (§4, §6); the sim reads the stage for rules (§5). The intensity map is a function of the blueprint's regions and hubs (WORLDGEN, §5.6), so every machine draws the same.

---

## 4. The look

### 4.1 The language, by place

What the procedural art and the presenter can actually do, at intensity 4 (lower intensities show less: §4.4). Every item is one of: the **LUT** (§4.2), an **overlay** from the night kit (§4.3), a **lamp** change (§4.5), a **grade or fog** row (§4.6) or a **night prop** the sim shows (§5).

| Place | By day | At night (intensity 4) |
| --- | --- | --- |
| **The town** (Castle: the square, High Street, the lanes) | Plaster, flowers in boxes, lit windows | Plaster stained to umber under the LUT; rust runs from sills and gutters; one window in three boarded, one in eight bricked, one in five dark, one lit; the window boxes bare (LUT: blooms to brown). Coats along the square's railings, twelve pegs' worth. A chalk register on the cobbles by the memorial. The Company's notice pasted over the parish board's. The fountain's water dark. **Company Row** at the west end (§5.1). The square's fire burns warm (a kept fire) |
| **Farms and cottages** (the Lowfields) | Thatch, hens, a lit back kitchen | Barn doors chained with a Company tag; the scarecrows in Company overalls (a coat swap on the scarecrow's look); the hen house shut; rag strips tied on the fence posts; one window lit, the back kitchen's |
| **Roads** | Lamp runs failing eastward, milestones | Kept lamps warm with Julie's chalk ring on the post (§5.2); wrong lamps cold; chalk tallies on the road's stones every few hundred metres past the last kept lamp, the groups getting longer toward the Works; Company barriers on the two closed tracks (N3); grates set into the road at N3 |
| **Woods** (the car wood, behind the car, the Long Hedge) | Canopy light, ferns | No canopy light (night has none); cloth strips on the low branches; the car in the wood with its doors open and its lamps on, cold; fewer leaves blowing (none, as built) |
| **The Waters** (reeds, bridges, the lake, the glasshouses) | Mist at dawn, reed, slate | The fen mist darker (fog row by stage); the reed huts' doors tied shut with Company tags; the Drowned Lane's water down to a causeway (N3, §5.1); the glasshouses' panes dark under the LUT, a few cracked (overlay); the toll board's numbers chalked over |
| **The Works** (Cinder Walk, the sidings, the cooling yard, the slag mere) | Slag, pipes, smoke | Full intensity from N3: smoke thick, pipe runs over every wall, the signals lit red, the cooling yard's belt running (sound and a moving belt overlay), the canteen's hatch open with a dinner laid (N3, §5.1) |
| **Interiors** (the Arms, the church) | Warm rooms | The Arms: curtains drawn (already "kept drawn"), a coat on every peg, the back room's door ajar (N2, §5.1). The church: the hymn board's third number chalked over; the vestry open (N2, §5.1) |
| **Dungeons** | Their own | Unchanged (§2.4) |
| **The sky** | The hour ramp, stars, the moon, the School | Stars thin at N3 and are gone at N4 (the Works' soot); the moon kept (night is beautiful); the School's windows lit by stage (N1 one and a flicker, as built; N2 two; N3 the top floor; N4 every window; dark after `bell_stopped`) |

### 4.2 The night albedo LUT (all tiers)

The night's material change is **one integer function of colour**, `jane_art::palette::night_of(rgb, intensity) -> rgb`, applied to albedo **before** the light, so the lamps light rust as rust and a cold lamp shows it cold:

- greens and yellow-greens (turf, leaves, paint) toward olive and umber;
- creams, whites and pinks (plaster, limewash) toward stained umber, keeping their lightness order;
- warm metals (brass, copper, gilt) toward rust orange-brown;
- glass (unlit) toward near-black with a cold tint; lit glass is emissive and untouched;
- blues (cloth, paint, water) toward grey-umber;
- skin, hair and the people's cloth ramps **excluded** (a person is a person at night; the townsfolk are indoors anyway) and every emissive index excluded.

It is a function of the colour alone (with a role mask for people), so it needs **no new palette entries and no new pages**. Intensity 0 to 4 mixes day toward the full night colour in quarters.

| Tier | How the LUT is applied | Cost |
| --- | --- | --- |
| T2 `wgpu`, T1 `gl2` | A second CLUT texture (2048 x 1, 8 KB) per intensity, built at boot; the sprite programs read the intensity's CLUT. Chunks are repainted with `TileSource::night(x, y) -> intensity` resolving through `night_of` (the region ramp swap, `ART.md` §3, already resolves per cell at paint) | 40 KB GPU; chunks repainted under the gutter (§2.2), on the worker, two a tick, the view's 12 to 20 in under a third of a second |
| T0 `soft` | The CLUT is `[u32; 2048]`: swap the table. Chunks repainted as above | 40 KB RAM; the repaint as above |
| C2 PSP | Folded into **the palette grade** (`Ge::graded_clut`, 1 KB a page, already rebuilt per grade change): the night function is applied to each page's CLUT and each chunk's `T8` CLUT before the grade. The chunk CLUT needs its entries' roles: a chunk keeps one flag byte per CLUT entry (person/emissive or not), 256 B a slot | No VRAM. RAM: 3 KB (12 chunk slots); the graded CLUTs already held. One CLUT rebuild per page at the turn (under 0.1 ms each) |

### 4.3 The night kit: overlays

Overlays are sprites from one new prop-kit family, `night` (`data/looks/night.json`), generated like every prop (four layers, `ao_contact; outline`, the art bar's ramps and clusters), and **placed by the presenter over geometry it already knows**, never by the sim:

| Overlay | Shapes | Laid where (what the presenter already knows) |
| --- | --- | --- |
| Boards | three plank layouts, a window's size each (w 1 to 2 cells) | windows: tile houses' windows (`terrain::houses::Houses`), sprite buildings' glass rects (the `Glass` role's bounds, exposed by `house::render`) |
| Brick-up | two (coursed, rough) | windows, as above |
| Rust runs | four (short, long, forked, from a bolt) | under sills, gutters, lamp arms, railings: the faces' top rows and props' trim rows |
| Peel | three (plaster gone to brick, to lath, to stone) | plaster faces |
| Coats | four cuts, coat-swapped by the role (`Swap`), on a peg or a railing | railings, fences, pegs (the chunk's fence runs; prop anchors) |
| Hung things | an empty birdcage, a hand bell on a cord, a Company tag on a chain, cloth strips (three) | lamp arms, gate posts, low branches (flora placements) |
| Chalk | tally groups (three lengths), a time in chalk ("9"), a ring (Julie's mark, §5.2) | road and cobble cells, lamp posts, doors |
| Company paper | a notice pasted over a notice (two), a numbered card in a window | signs and boards (prop anchors), windows |
| Ironwork | a pipe run (three), a grate (two), a barrier | walls and road cells (N3) |
| Stains | two, the contact shade only (`ART.md` §3.1: a floor mark darkens, it does not paint) | under lamps, by doors, the fountain |

**Placement is deterministic and presentation-only:** each candidate socket (a window, a face, a fence run, a road cell) takes an overlay when `h32(seed, socket, kind) % 64 < density(kind, intensity)`, the kind and variant from the same hash. It is the same on every seat and every tier, it never touches the sim, and a save never stores it. It is held to `ART.md` §3.1's "no stamp twice in view": two neighbouring windows never take the same board layout. About 45 sprites, all small: one 256 x 256 page.

**Caps per frame:** T2 and T1 128 overlays, T0 96, C2 Full 96, Balanced 48, Fast 24, nearest to her first. A dropped overlay is a missing board, never a missing rule.

### 4.4 Intensity, in numbers

| Intensity | LUT mix | Windows boarded / bricked / dark | Rust, peel, stains a face | Coats, hung things, chalk | Ironwork |
| --- | --- | --- | --- | --- | --- |
| 0 | none | as built | none | none | none |
| 1 | 1/4 | 1 in 12 / none / as built | 1 in 6 faces | chalk past the last lamp only | none |
| 2 | 2/4 | 1 in 5 / 1 in 20 / 1 in 4 | 1 in 3 | coats on the square; strips in the woods | none |
| 3 | 3/4 | 1 in 4 / 1 in 10 / 1 in 4 | 1 in 2 | as 2, more | pipes and grates in the Works and on the roads into them |
| 4 | full | 1 in 3 / 1 in 8 / 1 in 5 | most | as 3 | everywhere outdoors but the town |

### 4.5 Lamps

A lamp is **kept** (warm, safe: the sim's warm light) or **wrong** (cold, unsafe). The sim decides which (§5.4); the presenter draws it: a wrong lamp's light colour is a cold green-white (`#c8eed6` at the glass, a pool of `#2e5c50`), its flicker a slow uneven pulse, its glass the `glass_cold` emissive ramp (one new ramp, eight entries), and **no moths** at it (a kept lamp keeps its moths: a particle the presenter adds). The readable rule is colour and moths, taught in the gutter (§2.2).

### 4.6 Grade, fog and sky per stage

- **Grade**: the night `Post` gains a stage term (`light::grade(region, t, intensity)`): saturation down 6 a step, the lift toward umber (`#2a1a14`, never more red than green per `ART.md` §3.1's mauve rule: the umber is checked), exposure unchanged (value kept). Every tier draws it already; the PSP's palette grade takes it for free.
- **Fog**: `data/atmosphere.json` rows gain an optional `stage` (the row shows only at or above it): `works_smoke_night` over the Waters and the Lowfields at N3 (thin, 120), `works_haze` at N4 everywhere outdoors (90); the fen mist darker at N3 (colour row by stage). One drift on T0 and the PSP as built.
- **Sky**: the stars' count by stage (120, 120, 60, 0), the School's lit windows by stage (§4.1).

### 4.7 Budgets

| | PC (T2 / T1 / T0) | PSP-1000 (C2) |
| --- | --- | --- |
| New art | the night kit, ~45 small sprites, one page (all four layers on T1 and T2); one emissive ramp (`glass_cold`, 8 palette entries: 1043 to 1051 of 2048) | the night kit as **one `T8` page**, 64 KB pixels + 1 KB CLUT in the pack; loaded into the RAM page cache at 20:50 and let go at 06:10; one VRAM page slot (of 8 to 9) while overlays are on screen, through the LRU as any page |
| LUT | 5 CLUTs, 40 KB | folded into the graded CLUTs: 0 KB; 3 KB of chunk role flags |
| Overlays | ≤ 128 / 128 / 96 quads | ≤ 96 / 48 / 24 by preset (Full / Balanced / Fast) |
| The turn | the gutter (an ambient and light scale), the band (a row-banded grade, the afterglow's per-band tables on T0, a uniform on T1 and T2), chunks repainted under the dark | the gutter as an ambient scale; the band as two gradient quads over the frame and the lightmap's clear colour; CLUTs swapped in the dark second (one rebuild a page) |
| Sound | §6.4 | §6.4 |
| **Total new RAM at night** | under 1 MB | **about 70 KB** (the kit page, the chunk flags, the sound's §6.4 share separately), held to the play's floor of 1.2 MB free (a test, §9) |

**What does not fit, and is not done:** night frames for every building and prop (a second page set: several MB on the PSP), a repaint of the PSP's chunks with night tiles (CPU the console has not got spare; the CLUT does it instead), per-pixel dissolves between two frames (a second full frame), new normal or height layers for the overlays on C2 (albedo only; its relief page is not extended).

---

## 5. Play

### 5.1 What changes, place by place

| Place | Stage | By day | At night |
| --- | --- | --- | --- |
| **The Long Hedge footpath** | N1 | A hedge with a stile at each end, the stiles overgrown and the path blocked by a fallen limb | The limb is gone (it is the night shift's way): a short cut across the Long Hedge. Mr Pike's claim ("the footpath is used at night") comes true here. Threat 2 ground, unlit |
| **Company Row** (Castle's fourth lane) | N2 | Two houses at the west end of the town with a bricked gap between them | The gap is a lane: Company Row, six terraced doors, a Company sign (G.M.Co. RESIDENTS AFTER 9), a coat on every door's peg, a time card under one door (§5.2). Dot's Fourth Lane: "I don't count that one out loud." A dead end, inside the town's fence, so threat 0 (a haven): the night's strangest place is a safe one |
| **The Arms' back room** | N2 | Ernest Dunn's door, shut; Mrs Garland leaves the tray | The door ajar. The bed made, the tray empty, the window open on the square, the telephone's receiver off its hook on the sill and the line open. His time card on the pillow. Ernest is never seen (`STORY.md` §5); the room is all there is |
| **The vestry** (the church) | N2 | Locked; the vicar has the key | Open (the vicar is in the vicarage at night). The burial book open at a spring morning: G. GOLDSKIN, and four lines under it in the same hand, the same morning, no names, only trades: keeper of snakes, gardener, housekeeper, soldier. Foreshadows the Burial's four corners (`STORY.md` §2.6) |
| **The Drowned Lane causeway** | N3 | The lane under water, a soldier patrol on its dry end | The water down a cell (the tide of the night shift's pumps): a causeway across to Mother's Garden, the long way round cut in half. Threat 4. A jar of Julie's (strength) at its far end, night only |
| **The canteen hatch** | N3 | The hatch open six to half six; five tins at noon | The shutter up at nine, one dinner laid, the rota on the hatch with a line ruled for the night shift: 9 o'c., and the name space blank, the ink still wet. Nobody behind the hatch. (Her name is first written in the time book, `STORY.md` §8; this does not write it) |
| **The Works' time clock** | N3 | A dead clock by the Factory's gate (`machine` `clock`) | Ticking. It takes a time card and stamps it OUT (§5.2) |
| **Two Company barriers** | N3 | Two side tracks open (seeded, never a story road, never the first walk) | ROAD CLOSED BY ORDER OF THE COMPANY across each, solid. The validator proves every site stays reachable at night by another way (§5.6) |
| **The mine** | N2 | (After `mine_quiet` the door is never barred at night again, as built) | Unchanged inside |
| **Julie's house, cellar, yard** | all | Hers | Unchanged. The yard lamp is kept at every stage |

Night-locked doors stay as built (`PLAN.md` §2.6: the shop, the farms, some cottages; never the church, never the Arms; the Works' wicket opens for the night shift). From N3 the Halfway House's door (which takes her in after the bell) is the only inn outside Castle; it stays so.

### 5.2 Night-only finds, and what they are for

| Find | Stage | Where | What it is for |
| --- | --- | --- | --- |
| **Time cards** (8) | N2 | Glinting at night only (as `night_glint` is), each where its man or woman was at nine on the night of the bell: TACE (the top of Church Lane), DUNN, E. (the Arms' back room), E.M., NURSE (by her car), the carter (where the horse would not go), PELL, A. (by the last lit lamp, the fourteenth), the tollkeeper (the sunk boat), the fifth reedcutter (the empty hook), BREEN'S RELIEF (Hut One). Adults only: no child's name is on a time card, ever | **STORY CHANGE (4): the night shift can be relieved, one card at a time.** Stamped OUT at the Works' time clock at night (N3), each card relieves one of the night shift: one `night_soldier` on Cinder Walk stands down for good (a consequence: it walks to the canteen and does not come out) and the Company pays a wage packet (a gold-leaf page: spirit). Shown to the person who waits for them, by day (Mrs Tace, Walter Dunn), it changes one line, once, and confirms nothing. *Why:* "Not Relieved" becomes something she can do a little of, by night, which is the only time it can be done; it pays the side threads (`STORY.md` §5.1) without touching the endings, where the shift still walks in two of three |
| **Julie's chalk rings** | N2 | On the post of every kept lamp, visible at night only | Navigation: the kept lamps are marked in Julie's hand, the hand from the letter. The shield's edge, in chalk. At N4 the rings on the lamps that went wrong are smudged. Never explained |
| **The burial book** | N2 | The vestry (§5.1) | Foreshadows the Burial; the vicar reacts by day, once, behind a question she chooses to ask |
| **Julie's jar on the causeway** | N3 | The Drowned Lane's far end (§5.1) | Growth (strength), the night's reward for the night's risk |
| **The three night glints** | N1 | The car in the wood, the still pool, the Bellfield (as built) | As built: a potion and something useful each |

Each find is a night find because the story says it could only be: the cards are the Timekeeper's, and he keeps nights; the chalk is Julie's work on the shield, which only shows where the shield is failing; the vestry is open because the vicar is in. "Some things only found then" is a reason to go out, never a requirement: **nothing on the spine is night-only** (a test, §9).

### 5.3 Threats

As built at N1 (`WORLD.md` §4.3): the night shift up at the road edges and the rough ground (`night_skeleton`, `night_hound`, `night_soldier` in the Works), `NIGHT_AGGRO` 150, `NIGHT_HIT` 130, +1 threat outside lamplight (+2 in the Works). Each stage adds:

| Stage | Numbers | New behaviour | Hit |
| --- | --- | --- | --- |
| N1 | as built | as built | `NIGHT_HIT` 130 |
| N2 | night spawn candidates ×1.25 | **The night clerk** (`night_clerk`, the `skeleton_clerk` row's night look: a cold lamp in its hand, a moving wrong light): walks a road's unlit stretch counting; when it sees her it rings a hand bell (a 0.8 s tell, as the Hand-Bell's), and every night creature within 20 m takes her as its target. Kill it first or go round | 140 |
| N3 | ×1.5 | **The night shift walks out**: the cooling yard's soldiers walk Cinder Walk (as built) and two more files of three walk the unlit roads of the Waters and the Lowfields, in file, 21:00 to 05:00, stopping at a kept lamp's edge and looking (never stepping in). The Black Dog out of the Works two nights in three (it was one in three) | 150 |
| N4 | ×1.75 | **Wrong lamps draw them**: a wrong lamp's light is a spawn point for the night shift (the candidates include it) and a night creature idles toward the nearest wrong lamp; the Bellfield's "everything the School lets out" | 160 |

The aggro cap (10 m by night, `PLAN.md` §2.6) and every leash rule stand. Night threat is felt, not shown (no numbers). The phase table is unchanged: a night creature is the phase its ground gives it, harder only by `NIGHT_HIT` and its numbers. **Still playable:** the bots' night-exposure gate (§9) holds a Reader able to cross each region at night from fire to fire at every stage; deaths per night hour at N4 at most twice N1's.

### 5.4 Safe lights, fires and beds

- **The rule, one sentence: warm is safe.** In a kept lamp's or a fire's warm light nothing of the night changes (as built). A wrong lamp gives light to see by and no safety.
- **Kept lamps** are the hubs' lamps, the first walk's lamps (the station road), Julie's yard lamp, and the lamps inside the shield's line for the stage. **WORLDGEN:** the blueprint ranks every lamp by road distance from Julie's gate and stores the stage at which each goes wrong (`Blueprint.shield: Vec<(PropIx, u8)>`; the N2 line at 700 m by road, N3 at 400 m, N4 everything but the hubs and the first walk). The east road's omen (`lamp_east` dark at 22:00) is kept as its own rule.
- **Fires never go wrong.** Every kept fire is warm at every stage, and every region keeps one inside its base threat (as built, `county_fires.rs`). A made fire (`PLAY-PLAN.md` §2.2) is warm and safe: at N4 the player who carries wood carries safety.
- **Beds** as built: Julie's, the Arms, the Halfway House. A bed sleeps to six. The harsh night is always skippable from a bed, and that is the point: the night is a place you choose to be.
- **The dog's absence.** As built: not there from nine to six once the key is given, nobody sees it go, no line explains it. Nothing is added to the empty step at any stage. The tell is the house the night never touches (§1). **The Black Dog** (`night_hound`) never comes within the yard's fence or on the station road, and keeps a silhouette and colour that cannot be mistaken for Julie's dog (a test on the two looks' silhouettes; §10 asks the owner whether to keep the name).

### 5.5 What the night does to people

Townsfolk are `Inside` after the bell (as built); what changes is what their doors say. Knock lines exist and take night variants by stage (§7). The Arms' regulars and Mrs Garland are awake all night and say night lines from N2. Nobody walks but Mr Pollard (to the Dole Stone at ten, as built) and the Tenant of Plot 9. **No townsperson is ever shown changed.**

### 5.6 Determinism: worldgen versus runtime

**Worldgen (a pure function of the seed, in the blueprint; WORLDGEN):**
- the shield's ranks for every lamp (§5.4);
- every night prop as an ordinary prop with a `night` field (`{ "stage": 2 }`: shown from the bell to six at or above that stage): the time cards (8 of a candidate list per seed, by the rows' constraints, as stories place things), the barriers (2 of the eligible side tracks), the causeway's fill rect, Company Row's doors and its gap's tiles (an authored night layer of the town chunk: `Chunk.night: [(rect, tiles)]`, stamped as tile deltas at the turn and reverted at six), the back room's and the vestry's door hours;
- the night spawn candidates for all four stages at once, each with the stage it starts at (so a stage is a filter, never a re-roll);
- **the validator, per stage**: every required site reachable at night by a route that crosses no barrier, a kept fire within 2 minutes' walk of every road cell, nothing of the night inside a hub's fence or on the first walk, Julie's house and yard untouched, the spine needs no night-only thing.

**Runtime (state, saved and hashed):** `GameState.night` (the latched stage and the tick it turned); the night props' `hidden` bits, set by the clock rows at the turn as `night_glint` is today (`data/clock/night.json`); the tile deltas of Company Row and the causeway while they stand (owed to a zone nobody is in, as consequences are, and landed on arrival); the time cards taken and stamped (flags) and the shift relieved (consequences, once each); the presence of the night spawns.

**Derived, never saved:** which lamps are wrong (the blueprint's ranks against the latched stage), the intensity map, the overlays, the LUT, the turn's progress.

**The one exception to "nothing changes in view".** Presence (`ARCHITECTURE.md` §4.6.a) never shows or hides a thing inside a watcher's box. In the turn's dark second (ticks 24 to 84 after the first stroke) it may, for night props and night spawns only: the gutter is the county's blink, and the bell is the one moment the county is allowed to change in front of her. Outside that second the rule stands, and at dawn the night's things leave as built (walking home, out of sight).

**Zones on demand.** A zone nobody is in turns when it is next live: its night props and tile deltas land on arrival (as owed consequence edits do), with no turn played. A dungeon is never turned.

---

## 6. Sound

### 6.1 Ambience and score by stage

| Stage (intensity) | Beds (outdoors) | Score |
| --- | --- | --- |
| Day | as built | the region's day song |
| Intensity 1 | as built at night: crickets, owls, wind, the Works' hum louder after nine | the region's night song, as built |
| 2 | crickets halved; `drone` 40 (a filtered-noise bed under a held D2, the bell's hum, beating slowly); owls every 60 to 180 s (were 30 to 90) | the night song with its `deep` section in the loop: the melody dropped every other loop, the drift track and the bell's minor tierce in the pad |
| 3 | crickets gone outside the town; `machinery` 50 (slow iron, a press a long way off, a belt) and the `clock` tick (a time clock's escapement, one a second, outdoors everywhere); the drone 60 | the region's **shift** arrangement: the same song's chords and patterns on drone, low strings and iron, the theme's head once a loop on a far choir |
| 4 | silence where there was life: no crickets, no owls, no birds at dawn until the dawn turn; wind 40; the drone 80; machinery 30; a single far bell stroke at seeded gaps of 90 to 240 s (the Timekeeper's rope, `STORY.md` §8), stopped once `bell_stopped` is set | the shift arrangement thinned to its drone and the bell's echo; after `bell_stopped` the drone alone |

Indoors the outdoor beds are walled off as built (rain on the roof, the Arms' fire); the drone reaches every interior at half level but Julie's house, where it is never heard. Dungeons as built.

### 6.2 The turn's sting

`turn_<stage>`, a sound effect (never ducked), at the first stroke, layered over the bell:

- `turn_hold` (every stage): the stroke's hum partial held and swelled for 1.4 s, then let go into the room.
- N2 adds `shutter`: a low iron slam a long way off, with the long reverb tail, at 0.6 s.
- N3 adds `spin_up`: machinery coming up to speed under the band of light, 1.5 to 4 s.
- N4 adds a choir's open fifth on D, every window on the hill, as the band reaches the bottom of the screen.
- The silent turn (after `bell_stopped`): no stroke and no hold; the `shutter` at a third of its level at 2 s; nothing else.
- Dawn: `dawn_turn`, the machinery spinning down and the first bird under the band.

### 6.3 What changes in `jane-audio` and `jane-present::audio`

- `MusicCue::Zone(zone, region, night: bool)` becomes `night: u8` (the intensity); `MusicCue::zone` still normalises a dungeon's away. The cue table reads `View::night()` beside `bell_stopped`.
- **New beds** (live generators, `jane-audio`): `drone`, `machinery`, `clock`. **New patches** (`data/audio/sfx.json`): `turn_hold`, `shutter`, `spin_up`, `turn_choir`, `dawn_turn`. **Songs**: a `deep` section added to `lowfields_night`, `waters_night` and `works_night`; three new `*_shift` songs written from the same patterns (`data/audio/songs`).
- Checks as built (`tests/music.rs`): loudness within 2 dB, keys (the shift songs in their night song's key), no clicks, balance (the drone must not take a cue past 70 per cent of its power under 150 Hz: the drone is a bed, mixed under), and the turn heard in `jane sheet audio`'s `nine` scene at each stage.

### 6.4 The PSP's tracker budget

The tracker module is 0.81 MB today. The night adds three shift songs (patterns only: a few KB, reusing the bank's instruments), the five turn patches rendered into the bank (about 60 KB at 22.05 kHz mono, the bell's own sample reused pitched for the hold), and the three beds as short seamless loops (drone 32 KB, machinery 48 KB, clock 8 KB). **About 160 KB in all; the module under 1 MB.** If the play's RAM floor (§9) is under threat, the beds go first (the drone kept, made from the bell sample), then the shift songs fall back to the night songs' `deep` sections.

---

## 7. Words

### 7.1 The mechanism

Text already chooses by the hour: a dialogue tree's `start` rules take conditions (`{"if": "night"}`, `hours`, `weekday`; a sign is a tree with a speaker such as "Stake", `data/dialogue/lowfields.json` `plot_nine_stake`), and knock lines have night nodes. The night needs three small additions:

1. **`Condition::NightStage { min }`** (content: `{"if": "nightStage", "min": 2}`): true while it is night and the latched stage is at least `min`. Start rules list the deepest variant first, as they do now.
2. **Text rows with variants**, for the text that is not a dialogue tree (a `Read(TextId)` of a notice or a note, a toast, an examine line):
   ```json
   "notice_police_west": {
     "text": "CASTLE CONSTABULARY. Doors are not answered after nine. The Castle Arms will take you in till it is light.",
     "night": [
       { "min": 2, "text": "CASTLE CONSTABULARY. Doors are not answered after nine. The Castle Arms will take you in till it is light. (Under it, in pencil: and out again at six.)" }
     ]
   }
   ```
   Compiled to separate `TextId`s (`notice_police_west#n2`) with a link to the day row, so a variant is its own claim in the journal (`ARCHITECTURE.md` §3.7: it was read at night, and the day text was not). `Read` resolves the variant when it is read, from the sim's state, so every seat reads the same.
3. **Toasts and loading lines by stage**: the bell's toast as a variant row (`clock.json` keeps one action, the text varies), and a loading-line set per stage.

### 7.2 What gets a night variant

| Kind | Which | Roughly |
| --- | --- | --- |
| Signs and notices | Every sign in a place that turns at N2 or deeper: the parish board, the police notice, the Company's mine sign is **frozen** (`VOICE.md` §12.11) and is not varied; street name signs; shop cards; the Halt's timetable | 40 rows |
| Knock lines | Every numbered door (they have night nodes already): a stage-3 variant for one in three | 30 |
| People's night lines | The Arms (Mrs Garland, Mr Cobb, Mr Quill, Mr Ennis, the constable, Mr Ince, the woman with the case), the Halfway House, the watchmen's huts, Mrs Fludd at the fire | 25 |
| Jane's examine lines | Every prop or building with an examine line in a turned place; the night props (§5.1, §5.2) | 60 |
| Toasts | The bell (by stage), the silent nine, six o'clock, entering Company Row, stamping a card | 12 |
| Loading lines | A night set, a few per stage | 12 |
| The diary | **No variants.** The journal stamps each entry's hour (`VOICE.md` §6.5), so an entry written at night already says so | 0 |

About 180 rows: a sixth of the game's text, written in the coming VOICE v2 rewrite (§7.4), not after it.

### 7.3 Rules for writing them (for `VOICE.md` v2)

1. **The same text, one thing wrong.** A night variant keeps its day text's form, register and most of its words, and changes one thing: a number, a tense, a name, an addition in another hand. "Same sign, subtly wrong." A test holds it (§9: at least half its words are its day text's).
2. **Escalate by stage, not by volume.** N1 a detail; N2 the Company's hand over the council's, in the same register (`VOICE.md` §4: notices are where the register never changes when the subject does); N3 the text addressed to the night shift; N4 the text that has stopped expecting anyone to read it.
3. **Jane is a beat behind, and she was here this afternoon.** Her night examine lines lean on the day she remembers: "Boards across the window. The nails are rusted in. There was a geranium on this sill at four." She counts ("Eleven coats. I count them again and it's eleven."). She never says "wrong", "creepy", "changed" or "different" (`VOICE.md` §5.8).
4. **Never explain the night.** No line says what the night is, why the town turns, or what the shield is. Not the dog, not a notice, not the diary.
5. **Never confirm.** A night variant does not say whether its day text was true (`VOICE.md` §2.8). The time card shown to Mrs Tace changes one line once; it never says "he is dead".
6. **No `{name}` in the night's signs.** Her name is first written in the time book (`STORY.md` §8); nothing at night writes it before.
7. **People who are inside speak through doors.** Knock lines are short; the night ones shorter, and a third of them are only a sound ("The bolt goes across. Nobody says anything.").

Samples, in the VOICE v2 voice, for the owner's ear:

| Where | By day | At night |
| --- | --- | --- |
| Parish board (N2) | "PARISH OF CASTLE. Whist drive, Thursday, in the hall. Bring a plate." | "PARISH OF CASTLE. Whist drive, Thursday, in the hall. Bring a plate." Over it, pasted at the corners: "GOLDSKIN MINING Co. All persons on the books will report at nine. Persons not on the books will be put on them." |
| The fountain (Jane, N2) | "Clear enough to see the pennies. Eleven. Somebody's thrown in a button." | "Too dark to see the pennies. Eleven, and the button. I'm counting them from this afternoon." |
| The square's railings (Jane, N2) | "Iron railings, painted green a long time ago." | "A coat on every spike. Twelve. They're dry. It's been raining since eight." |
| A wrong lamp (Jane, N2) | "A lamp on a post. The glass is clean." | "It's lit, and nothing's come to it. Not one moth." |
| A kept lamp (Jane, N2) | "A lamp on a post. The glass is clean." | "A ring in chalk round the post, at the height of my hand. It's the hand from my letter." |
| No. 7, Pound Lane, knock (N3) | "No answer. Three bottles on the step." | "No answer. Four bottles on the step. The milk doesn't come at night." |
| Company Row sign (N2) | (not there by day) | "COMPANY ROW. G.M.Co. Residents after 9 only. Lights out at 10." |
| The bell's toast, N1 / N3 / silent | "A school bell, a long way off. Nine o'clock." | N3: "Nine o'clock. Up the hill, a shutter comes down." Silent: "Nine o'clock. No bell." (as built) |

### 7.4 The coming text rewrite

The VOICE v2 rewrite (`progress/2026-10-09_65_voice/`) should write each night variant **beside its day row in the same commit**, so the two are judged together; the night pass is a column in the rewrite's tracker, not a second rewrite. The tests (§9) land before the first rows, so a variant that drifts from its day text, writes `{name}`, or uses a banned word fails at once.

---

## 8. Gore

**The rule (owner, 9 October 2026; `VOICE.md` §12.5):** occasionally, only with an explicit reason the player can find, about Warcraft III: a dark red stain, a carcass, a glove; never lingering, never animated, never close up. **Never** harm to a child, a child's body, a child-sized anything bloodied, a stained coat on a peg; never self-harm, and **nothing hanging that could read as a person** (the night's hung things are coats on pegs at chest height, cages, bells and strips of cloth; no rope ends in a loop, nothing human-sized hangs from a height).

Where, in the whole game, and why:

| Where | When | What is shown | The reason she can find |
| --- | --- | --- | --- |
| Lowfield Farm's sheep pen | The morning after a night the Black Dog was out in the Lowfields and the pen gate was left open (the Farmer's count) | One sheep down in the pen, a dark stain under it, the crows on the fence; gone by noon | The Farmer's line ("You'd tell me if it was four") becomes a count of sheep; the hound's tracks through the gate. Ecology: hunting changes numbers, and numbers are seen (`WORLD.md` §4.2) |
| The cooling yard's belt (the Works) | Night, N3 and deeper | The belt running with nobody at it; a dark stain along a run of its rollers; one glove caught in them | The foreman's diary, three pages earlier: "If the office asks, he was never on." The man whose card he took out of the rack |
| The Burial | As built | The open tomb, the skull looking out, the hand on the rim (`ART.md` §2.3) | Goldskin's four favourites, buried the same morning |

That is all of it. **No gore in the mine or the School** (the children's places), none in the town, none on people, none at night for its own sake. Combat stays as built: the undead come apart dry (bone, rust, dust), creatures fall in their dead pose; no blood from blows (§10 asks whether living creatures should leave a small stain).

---

## 9. Implementation plan

Six rounds, each shippable on its own: after each, the game builds, the slow tier is green, the PSP boots and plays, and the night is whole at whatever depth it has reached. The order puts the sim's state first (so every later round reads one field), the switch second (so the night is felt before it is dressed), and the words last but planned from the start.

| Round | Work | Crates / data | Owner | Size |
| --- | --- | --- | --- | --- |
| **R1. The stage** | `GameState.night` latched at the turn; `Condition::NightStage`; `View::night()`; text rows with `night` variants compiled to linked `TextId`s; light `warm` (the blueprint's shield ranks against the stage) read by the safe-light rule and the spawn exclusion; the presence exception for the dark second; SAVE_VERSION bump | `jane-sim`, `jane-schema`, `jane-data`; `data/clock.json` | heavy | M |
| **R2. The switch** | The gutter, the band and the dawn turn in the presenter (all tiers and C2); the grade's stage term; the cue table's `night: u8`; the drone, machinery and clock beds; the turn patches; the silent turn | `jane-present` (light, atmos, audio), `jane-render-*` (the band on T0's row tables, a uniform on T1 and T2, two quads on C2), `jane-audio`; `data/audio` | heavy | M |
| **R3. The look** | `palette::night_of` and the role mask; the CLUTs per intensity (PC), the chunk repaint under the gutter, the PSP's graded CLUT fold and chunk role flags; the `night` kit family (~45 looks) to the art bar with the critique loop; the presenter's overlay placement (windows, faces, fences, flora, road cells) and caps; `glass_cold`; fog rows by stage; the sky by stage; the PSP bake's night page | `jane-art`, `jane-present`, `jane-render-psp`, `jane-cli` (bake, sheets); `data/looks/night.json`, `data/atmosphere.json` | heavy (art), light (sheets) | L |
| **R4. The places** | WORLDGEN: the shield ranks, night props with `night.stage`, Company Row's night layer in the town chunk, the causeway, the barriers, the stage-tagged spawn candidates, the per-stage validator; the clock rows that turn them; the time cards and the Works' time clock; the relief consequences; the night clerk and the N3 files; `NIGHT_HIT` by stage; blueprint fixtures re-blessed | `jane-world`, `jane-sim` (life, ai), `jane-data`; `data/chunks/town.chunk`, `data/props`, `data/placements`, `data/units`, `data/consequences.json`, `data/clock/night.json` | heavy | L |
| **R5. The words** | The ~180 night variants, in the VOICE v2 rewrite, beside their day rows; the bell's toasts by stage; the night loading lines; the voice tests for variants | `data/dialogue/*`, `data/clock*`; `jane-data/tests/voice.rs` | heavy (writing), owner's ear | M to L |
| **R6. Tuning, gore, proof** | The two gore moments; the Black Dog's look and bounds; bot night-exposure runs at every stage; the art-review frames of every stage (§9.2); PSP captures at N4 (the square at 22:00 in rain, the lake, Cinder Walk); the degrade presets' caps | `jane-bot`, `jane-art`, `jane-present`; `progress/` | heavy, owner's PSP | M |

R2 can begin while R1 is in review (it reads the stage through a stub that is always N1); R3 and R4 can run in parallel worktrees after R1 merges; R5 needs only R1's variants and runs with the VOICE rewrite.

### 9.1 Tests

- **Determinism**: `night_stage_latches_at_the_turn_and_only_there` (a boss killed at 23:00 moves the stage at the next nine, not now); `a_night_slept_is_a_night_idled` extended across a turn and a stage change; a replay tape across the turn at each stage hashes the same twice and on two seats in lockstep; `zone_order_does_not_move_rolls` with night spawns at N3; `runtime_rebuild_is_invisible` mid-turn; save and load at 21:00:02 (mid-turn) and at 23:00 at N4 round-trip `decode(save(s)) == s`.
- **Worldgen**: blueprint hash fixtures moved once in R4 and held; the per-stage validator on 64 seeds (every site reachable at night, a kept fire within two minutes of every road cell, hubs and the first walk untouched, Julie's house and yard untouched, the spine needs nothing night-only, the barriers never cut a story road); the town chunk's night layer stamps inside its rect.
- **The look**: the Frame at each stage at every tier (and C2) holds no pass the tier cannot draw and draws in `soft` without a panic; overlays within caps; no overlay twice in view on neighbouring windows; `night_of` leaves people's ramps and every emissive index alone; the umber lift never more red than green; golden frames of the square at 22:00 at N1 and N4.
- **The PSP**: free RAM in play at N4 at the square at 22:00 in rain at least 1.2 MB (and the map open at least 0.6); VRAM page slots unchanged in number; the night page under 66.5 KB; the state hash at the square at 22:00 the same under Full, Balanced and Fast at N4 (as built for the day).
- **Sound**: the new beds and patches pass `tests/music.rs` (loudness, keys, clicks, balance); the turn's sting for each stage in `jane sheet audio`'s `nine` scene; the tracker module under 1 MB.
- **Words**: every night variant links to a day row; at least half its words are its day text's; no `{name}` in a night sign; the banned list of `VOICE.md` §10 and §5.8; the frozen rows (§7.2) have no variants.
- **Gore**: rows tagged `gore` exist only at the three places of §8; no `gore` tag in the mine, the School or any zone a child's look is placed in.
- **Play**: the Reader and the Rusher on seeds 1 to 5 at every stage: night exposure (time outside warm light), deaths per night hour at N4 at most twice N1's, every region crossed at night fire to fire; the time cards found and stamped by a bot on one seed.

### 9.2 Art review

At each round from R3, `tools/art-review.sh` gains the night frames: the square, the High Street, Company Row, the Long Hedge, the reed camp, Cinder Walk and the Arms, each at 22:00 at N1, N2, N3 and N4, and the turn as a film strip (`jane sheet scene` over the turn's ticks). The critique loop is capped at three passes per family per round; what still falls short of `ART.md` §3.1 after three is reported, not polished forever.

### 9.3 Risks

| Risk | Answer |
| --- | --- |
| The PSP's RAM (1.4 to 2.8 MB free in play) | 70 KB of art and 160 KB of sound at night; a test holds the floor; the beds and the shift songs are the first cuts (§6.4) |
| The night reads as murk, not beauty | Value kept by the grade (exposure unchanged), the LUT shifts hue not lightness order; the review frames at each stage; T2 the reference, every tier one look |
| The night is tedious or unfair | Beds skip it; nothing on the spine is night-only; the bots' exposure and death gates per stage; warm-is-safe taught in the first second |
| Stamping tiles at the turn (Company Row, the causeway) moves saves and the blueprint hash | One SAVE_VERSION bump in R1 and one fixture re-bless in R4, each in its own commit with the owner told |
| The presence exception lets something pop in view | Only night props and night spawns, only in the dark second; a test that nothing else is shown or hidden in a watcher's box then |
| The Black Dog read as Julie's dog | Bounds (never in the yard or on the station road), a distinct silhouette test, and §10 Q8 |
| The text doubles | A sixth of the rows, written beside their day rows in the VOICE rewrite, not a second pass |
| Combat in the gutter's dark | Units and her ring drawn as silhouettes through the dark second; the ambient floor at 20 per cent |

**Rough size:** R1 M, R2 M, R3 L, R4 L, R5 M to L, R6 M. With three or four agents in parallel after R1, about six working rounds.

---

**Owner request (9 October 2026, for R3):** at night, water reflects the stars and the moon. The sky's night stars and a moon (not drawn on T0 today) are mirrored in still water, rippled by the reflection band, dimmer under the soot at N4, on every tier including C2 within budget.

## 10. Decided (owner, 9 October 2026)

The owner approved the design. All twelve questions are settled on their defaults; the owner chose 2, 3, 4, 6, 7, 9 and 10 explicitly:

1. The stages start at the mine (N2), the Factory (N3) and the Burial (N4).
2. **STORY CHANGE (1) accepted:** the night world is Julie's shield failing, and her house never turns.
3. **STORY CHANGE (3) accepted:** bringing the Ball up deepens the night.
4. **STORY CHANGE (4) accepted:** eight time cards relieve the night shift, one man at a time.
5. The turn as designed: the gutter, then the light comes back from the hill down, in about four seconds.
6. **STORY CHANGE (2) accepted:** after the Timekeeper falls, the night still turns, in silence.
7. Company Row is accepted.
8. The Black Dog stays, kept away from the yard and the station road, and unmistakable in shape.
9. Gore is limited to the three placed moments, with no blood in combat.
10. Dungeons do not turn.
11. The presence exception, in the turn's dark second only.
12. About 180 night variants, written beside their day rows in the VOICE v2 rewrite.

`STORY.md` takes the four story changes in round R1.

---

## Concept frames

Rendered for this document from the build at `32ac4c4` with `jane sheet scene` (seed 1), then mocked with an image script (a post-process, not game code), in `progress/2026-10-09_66_night/` (local, in this worktree):

- `square-day-night-world.png`: the town square at 14:00, at 22:00 as built (in rain, the Rusher's run: the townsfolk in, the square's fire doused), and a **mock** of 22:00 at N2: the night grade toward umber, three windows boarded and one bricked, rust run down the plaster, the two gas lamps gone wrong (cold), coats along the railings, a register chalked by the memorial, the fountain dark, and the square's fire lit warm as the one safe light. The mock's overlays are rough stand-ins for the night kit (§4.3), which would be generated to the art bar.
- `road-day-night-world.png`: a lamp on a county road, the same three ways (the mock: the grade, a wrong lamp, cloth strips on the branches, chalk on the road).
