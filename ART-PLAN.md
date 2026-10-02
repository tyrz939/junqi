# Jane: art audit and plan (2026-10-02)

Research only: no tracked file was changed. The renders are in `progress/audit/art/`. They were drawn on soft (T0), because the local `jane` has no `gpu` feature. T0 is held to T2's look (ART.md §3.1, "the tiers are one look").

## 1. Verdict

1. **Light and materials already match the best references.** Jane's light, shadow, hue-shifted ramps and night grading are up with Eastward and Sea of Stars, and so are her tufts, path edges, cut stones and icons.
2. **Charm is lost to sameness.** Every town house is the same red-tiled box. All the broadleaf trees share one lollipop silhouette, and every room is a rectangle of one floor with furniture pushed against the walls.
3. **The world is still.** Nothing on screen lives except people, fire and rain: no birds, butterflies or smoke, no foliage sway, and nobody blinks. Minish Cap and Stardew get half their warmth from this.
4. **The county is drawn in high summer, though autumn is the fixed season** (WORLD.md §2.4: "leaves on the square, the mist"). Painting autumn is the cheapest and largest single gain in mood.
5. **Dungeons and interiors lag furthest.** Walls are one cell tall, rooms have no framing, no motif repeats and no room is a payoff. Taller drawn walls, floor borders, a signature motif per dungeon and a set-piece boss room would close most of the gap.

## 2. The games picked, and why

| Game | Why it is a reference for Jane |
| --- | --- |
| **The Legend of Zelda: The Minish Cap** (Capcom/Flagship, 2004) | Required. It is the genre's best top-down town (Hyrule Town), the clearest dungeon language at 16 px tiles, and the reference for charm through oversized everyday objects |
| **Eastward** (Pixpil, 2021) | The closest match in tone and tech: modern pixel art with 3D lighting and hand-painted normal maps, which is Jane's own four-layer contract. Its towns are cosy and decayed, packed with lived-in clutter, under a dread that slowly rises (the Miasma), close to Jane's Silent Hill lean |
| **Stardew Valley** (ConcernedApe, 2016) | The model for a rural village, gardens, seasons, wind debris (petals and leaves), critters and interiors that feel lived in, all from one artist working on a budget, close to Jane's procedural budget |
| **Sea of Stars** (Sabotage, 2023) | Already Jane's look reference (ART.md §0). It is studied here for dynamic light at day and night, water, layered atmosphere, and dungeons with verticality and set pieces |
| **Hyper Light Drifter** (Heart Machine, 2016) | The model for the uncanny half: a palette per region, storytelling through ruins and bodies with no text, impressionist omission ("fill in the gaps"), light shafts, and a minimal glyph UI |

Not picked: CrossCode (its puzzle readability is relevant, but the sci-fi tiles are far from an English county), Link's Awakening DX and Alundra (their lessons are covered by Minish Cap at a higher fidelity), and Chained Echoes (overlaps with Sea of Stars).

## 3. Technique notes, per game

### 3.1 The Minish Cap

- **Scale as charm.** Shrunk to Minish size, Link meets twigs, acorns, raindrops, leaves and crystals that fill the screen. Ordinary household objects become landmarks ([TheGamer](https://www.thegamer.com/the-minish-cap-best-looking-2d-zelda/)). For Jane, this argues for a few **oversized hero objects** per place, and for treating small things (a milk bottle, a boot, a snail) as worth a sprite.
- **Hyrule Town is a "living" hub.** Aonuma said it surpassed Clock Town for feeling inhabited ([Wikipedia](https://en.wikipedia.org/wiki/The_Legend_of_Zelda:_The_Minish_Cap)). Look at the town map ([Spriters Resource, Hyrule Town](https://www.spriters-resource.com/game_boy_advance/thelegendofzeldatheminishcap/asset/6526/)):
  - Every house differs in roof colour and shape (round, gabled, striped awnings), door, signs and pots.
  - Each shop shows its trade outside: the bakery's bread sign, the café's tables, the cucco pen.
  - Wells, flower beds, fences and lamp posts break up the open ground.
  - Paths are edged with a light kerb line.
- **Buildings read by silhouette and colour first.** The roof takes about two thirds of the sprite, but it is never one monotone field: it has a ridge, a chimney with a pot, a window in the roof or a dormer, and a saturated colour that is different from its neighbours. Walls carry shutters, window boxes and a step at the door.
- **Ground rhythm.** Flat grass alternates with clusters of tall cuttable grass, flowers in groups of three to five, and bushes, leaving areas of negative space. Slynyrd's top-down tile guide states the same rule: "mix in spaces of flat green with textured areas", "negative space is your friend" ([Pixelblog 20](https://www.slynyrd.com/blog/2019/8/27/pixelblog-20-top-down-tiles)).
- **Dungeons.**
  - Thick walls two to three tiles tall frame every room. The wall top is a patterned trim and the face carries pillars, torches and statues.
  - Doors are set in framed arches, and a lock or eye statue tells you what a door is.
  - Each dungeon has one material and motif. Deepwood has wood and webs; Cave of Flames is a mine with rails and lava; Temple of Droplets has ice, opened skylights casting sunbeams, and dark lantern sections; Palace of Winds has sky seen through holes in the floor and torn carpets ([Zelda Dungeon, Palace of Winds](https://www.zeldadungeon.net/the-minish-cap-dungeons-palace-of-winds/), [Temple of Droplets](https://www.zeldadungeon.net/the-minish-cap-dungeons-temple-of-droplets/)).
  - Floors carry an inset border one tile in from the walls, and set rooms are symmetric.
- **Selective outlines.** Sprites get a dark coloured outline, while ground and walls get none. Jane already matches this rule (ART.md §3).

### 3.2 Eastward

- **Pixel art plus 3D light.** Assets are split by structure ("separate a building rooftop and the wall into two layers"), rebuilt in 3D, and given bump maps painted one by one. Fog layers and sunbeams come in at dawn and dusk ([Game Developer](https://www.gamedeveloper.com/art/eastward-s-creators-share-insights-on-making-pixel-art-adventures)). SSAO, a soft CRT blur and a LUT soften the colour ([80.lv](https://80.lv/articles/eastward-charming-chinese-pixel-art-adventure)). Jane's normal, height and emissive layers and her grade are the procedural equivalent, already built.
- **"Thousands of little touches."** Potcrock Isle and New Dam City are dense with laundry lines, pipes, pot plants, crates, posters, hanging lanterns and sleeping cats. Reviewers stress that "every place you encounter has little details that also make it unique" ([Wikipedia](https://en.wikipedia.org/wiki/Eastward_(video_game))). The density is **vertical and on the walls**, not spread across the floor.
- **Elevation.** Stairs and terraces let the player pass behind buildings, so a dense street never blocks the view ([Game Developer](https://www.gamedeveloper.com/art/eastward-s-creators-share-insights-on-making-pixel-art-adventures)).
- **Interiors.**
  - Back walls are tall (two to three tiles) and dressed with shelves, posters, clocks and windows that let in light.
  - Floors carry rugs.
  - Tables carry the clutter of a life: bowls, papers, a radio.
  - Lamps throw pools of warm light.
- **Characters.** Big-headed sprites that act through idles: John's slump, Sam's bounce, NPCs busy at tasks (cooking, fishing, sweeping).

### 3.3 Stardew Valley

- **Seasons repaint the same map.** Pelican Town has a spring, summer, fall and winter version, swapping grass hue, tree crowns, leaf litter and snow ([Spriters Resource: Pelican Town Fall](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/88626/), [Destructoid](https://www.destructoid.com/four-seasons-in-one-image-shows-how-beautiful-stardew-valleys-pixel-art-really-is/)). Autumn trees mix orange, red and yellow, with a few still green.
- **Wind debris.** On windy days spring petals and autumn leaves blow across the screen, purely for looks ([TheGamer](https://www.thegamer.com/stardew-valley-types-weather-effects/), [wiki: Weather](https://stardewvalleywiki.com/Weather)). Grass tufts sway and rustle when walked through ([Spriters Resource: Grass](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/223916/)).
- **Critters** ([Spriters Resource: Birds](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/169032/)):
  - Birds land, peck and fly off when approached.
  - Butterflies wander over the flowers.
  - Rabbits and squirrels dart away.
  - Woodpeckers sit on trunks, frogs by water, fireflies on summer nights, seagulls at the beach.
- **Every house is its owner.** The general store, the clinic, the saloon, the blacksmith's forge and the trailer each have a different footprint, roof, colour, signs and yard props. Haley's house has flower tubs; Willy's shop has nets ([wiki: Pelican Town](https://stardewvalleywiki.com/Pelican_Town)).
- **Interiors** ([Walls & Floors](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/88659/)):
  - Wallpaper on a back wall two tiles tall.
  - Windows that cast light rectangles on the floor by day.
  - Rugs, fireplaces, plants in pots, and a TV or radio.
  - Beds with patterned quilts, and kitchens with pans on the wall.

### 3.4 Sea of Stars

- **Dynamic light as a design pillar.** One engineer spent five years on the lighting, and every asset had to answer to it. Day and night switch on demand, and characters cast accurate shadows ([MobileSyrup](https://mobilesyrup.com/2023/09/11/sea-of-stars-level-design-sabotage-studio-thierry-boulanger-interview/)).
- **Living scenery.** "Animated sprites and objects, including lights, fauna, and mechanisms". Water reflects light sources and distorts what is seen through it ([Megavisions](https://www.megavisions.net/the-art-of-sea-of-stars-a-sea-of-pixels/), [ResetEra thread](https://www.resetera.com/threads/its-frankly-insane-how-good-sea-of-stars-pixel-art-is.1048128/)).
- **Dungeons.**
  - Verticality: cliff faces to climb, ledges, swimming and jumping between levels.
  - Each dungeon is a material showcase (Coral Cascades, the Clockwork Castle, the Necromancer's Lair).
  - Puzzles are read through light: sunstones, shadow pillars.
- **Animation.** Sprites are generously keyframed. Idles breathe, hair and capes trail behind the body, and small emotes play in cutscenes.

### 3.5 Hyper Light Drifter

- **Region palettes, not material palettes.** Grass is deep blue and leaves are neon red. The west is dark green grass under red skies with light green crystals ([Hardcore Gaming 101](https://www.hardcoregaming101.net/hyper-light-drifter/), [Game Developer](https://www.gamedeveloper.com/business/the-ultra-modern-stylings-of-hyper-light-drifter)). Flat colour is laid under large gradients and a vignette.
- **Pixel impressionism.** It shows "enough detail to communicate what a character is, but not too much" ([grahuba](https://grahuba.wordpress.com/2016/09/14/art-of-hyper-light-drifter/)), and leaves the eye to fill the gaps ([hookshotchargebeamrevive](https://hookshotchargebeamrevive.wordpress.com/2018/09/10/hyper-light-drifters-pixel-impressionism/)).
- **Storytelling with no text.** Titan corpses, dead robots and sacrificial altars tell the history. Sunbeams cut through the canopy, and leaves fall.
- **A minimal UI.** Health pips, an energy bar, and glyph prompts in the world's own iconography.
- **A warning from the same reviews.** Its underground areas felt "generic, seeming like they were constructed out of tiles rather than carefully designed" ([HG101](https://www.hardcoregaming101.net/hyper-light-drifter/)). That is Jane's dungeon risk exactly.

## 4. What I looked at in Jane

These were rendered with `jane sheet buildings|flora|creatures|icons|units|chrome|ui` and `jane sheet scene --backend soft`, at the art-review marks:

- **Sheets:** `progress/audit/art/buildings.png`, `flora.png`, `creatures.png`, `icons.png`, `units.png` (crop `units_crop_a.png`), `chrome.png`, and `ui/ui-sheet-*.png`. The props sheet (`jane sheet props`, 2256 x 19204) was read in sections; it was not kept because it is 173 MB.
- **County:** `town-noon.png`, `town-1840.png`, `town-2200.png`, `yard-1700.png`, `field-edge.png`, `wood-low-sun.png`, `reed-camp.png`, `works.png`.
- **Zooms:** `town-house-zoom3.png`, `field-ground-zoom3.png`, `burial-zoom3.png`.
- **Interiors:** `house-in.png`, `arms-in.png`, `church-in.png`.
- **Dungeons:** `mine.png`, `burial.png`, `museum.png`, `school.png`, `factory.png`, `pipes.png`, `library.png`.
- **Earlier shots:** `progress/2026-09-30_41_roofs/after-town-t2*.png`, `progress/2026-10-01_46_biomes/*`, `progress/2026-09-28_29_setpieces/overview-after-wgpu-burial.png`, `progress/2026-09-27_18_art-director/scenes/yard-dusk.png`.

## 5. Grades against the five games

The grade is Jane's against the best of the five in each area.

| Area | Grade | Already matches | Clearly lags | Missing entirely |
| --- | --- | --- | --- | --- |
| **Ground** | B | Tufts in clusters, path edges with a 1–2 px rim, calm turf with rare drifts, no visible grid (`field-ground-zoom3.png`). Up with Stardew | The density rhythm is uniform: marsh reeds are a carpet of noise with no quiet patches (`progress/2026-10-01_46_biomes/`), and the Works' stones sit on a near-regular grid (`works.png`). Flowers are rare single specks | Flower clusters, ferns, bracken, nettles, cow parsley, foxgloves, brambles, ivy, leaf litter, lily pads and mushroom rings in the open county |
| **Plants** | C+ | Crowns are lit as one volume, with leaf masses and a ragged edge; cut stones are excellent (`flora.png`) | All twelve broadleaf trees share one silhouette (round crown, thin straight trunk). Forests read as a carpet of identical crowns, and the field edge reads as an orchard (`field-edge.png`, `wood-low-sun.png`) | Species (oak, ash, birch, willow, hawthorn, yew, holly, beech), root flare, leaning trunks, stumps among trees, **autumn colour**, and sway |
| **Houses** | C | Courses, eaves, plinths and lit windows that glow at night (`town-2200.png`) beat Minish Cap technically | The town is one house stamped about eight times: the same red tile roof, cream plaster, window rhythm and barred-looking plank door (`town-noon.png`, `town-house-zoom3.png`). Roof courses read as barcode noise. The sprite houses (`buildings.png`) vary in material only, and the wall is a thin strip | Door colours, shutters, window boxes, porches, bay windows, dormers, house numbers, trade signs, climbing ivy or roses, milk bottles and boot scrapers, and a front garden that belongs to that house |
| **Props and variety** | B− | 267 props, all with looks, competently lit; the set pieces (carpets, junk heaps, tombs) are good | Props are placed against the walls with empty floors in the middle (`house-in.png`, `arms-in.png`). The reed camp is an empty field of mud (`reed-camp.png`). The square has a big empty middle (`town-noon.png`) | One-off "hero" objects per place, and vignettes that tell a story (Eastward and HLD) |
| **Ambient life** | D | Fire, lamp pools, rain with puddles, mist, lit windows with one flickering | — | Birds (they exist only in the audio bed, `jane-present/src/audio.rs`), butterflies, moths at lamps, a cat on a wall, chimney smoke (ART §2.8 designs it; not built), sway, falling leaves, water glints and shore foam, NPC tasks |
| **Characters** | B | Chibi proportions that read well, a six-frame walk, breathing, held things, eight facings, distinct hats and hair (`units_crop_a.png`) | One body template under every look; silhouettes differ by hat more than by build | Blinks, emotes (! ? ♪ …), expressions, dialogue portraits, idle business (sweeping, reading the paper) |
| **Palette and light** | A− | Hue-shifted ramps, coloured AO, long dusk shadows, a blue night, warm pools (`progress/2026-09-27_18_art-director/scenes/yard-dusk.png`). Matches Sea of Stars and Eastward | Interior boards are too contrasty (red streak noise, `house-in.png`). Dungeons all grade to one blue-grey (`burial.png`, `factory.png`, `pipes.png`) | **The autumn season palette**, and a grading key per dungeon |
| **Dungeons** | C− | Each dungeon has its own wall and floor pattern (`terrain/interior.rs`), furnished set pieces, and lantern darkness | The wall is one cell tall: rooms read as rectangles cut out of a void. The floor is one field edge to edge. Clutter is scattered evenly (bones on the Burial flags, `burial-zoom3.png`) | Framed doorways, floor borders and insets, symmetric set rooms, a signature motif repeated through the dungeon, a boss-room payoff, and puzzle elements that read from across the room |
| **Interiors** | D+ | The light pools are lovely | The back wall is one dark strip; the room has huge bare plank floors | Wallpaper or panelling, windows casting light, pictures, a clock, a hearth, rugs, plants, clutter on tables |
| **UI** | C+ | The icons are B+ after the icon pass, and the chrome is clean and consistent (`chrome.png`, `ui/ui-sheet-hud.png`) | A generic slate panel with nothing of Castle in it | Portraits, a theme (letter paper, a railway ticket, Julie's notebook), and ornament on the frames |

## 6. The ranked plan

The plan is ranked by charm gained for the effort. Sizes: **S** is under a day of agent work, **M** is one to three days, **L** is a week or more.

Every item keeps to the constraints:

- Everything is generated by code.
- Integer-only and deterministic: seeds from cells, ids and ticks, never the sim's RNG.
- Nothing new in the `Frame` contract unless stated. Ambient life uses the existing `SpriteCmd` and `Particle`.
- Nothing a player can touch: decor and critters have no footprint in the sim.

### Quick wins

**Q1. Autumn, the fixed season** (S–M). *Borrows: Stardew's season repaint and wind debris; HLD's falling leaves.*

- **What to do:**
  - Give three in five broadleaf trees a turned crown: beech gold, oak russet, field maple butter-yellow, and a few still green with a gold fringe. Choose by cell hash and biome; the Waters keep more green and the Works go brown.
  - Lay leaf litter under the crowns and on the square and lanes as contact-shade-style decals (clusters of two or three px in the litter's ramp, thickest on the lee side of walls).
  - Turn the hedges partly bronze, add bracken in rust on the woodland floors, and fruit the hawthorns with haws.
  - Blow leaves on wind: about 20 `Particle::Dot` per screen, coloured from the litter ramp and drifting with `Atmosphere::wind()`. Leave them out at night and in heavy rain.
- **Files:**
  - `jane-art/src/palette.rs`: the leaf ramps.
  - `jane-art/src/flora.rs`: `Bank` crowns by ramp.
  - `jane-art/src/terrain/standing.rs` and `ecotone.rs`: tree choice and litter decals.
  - `data/looks/tiles.json`.
  - `jane-present/src/atmos.rs` (or `fx.rs`): the leaf particles.
- **Caveat: the palette is at 1019 of its 1024 cap** (ART §2.7). Two new six-tone ramps need either the cap raised (a test constant; `Ix` is u16) or `leaf_olive` retoned into gold. Decide this first.
- **Before → after:** a June-green county → an October one. The square has leaves in its gutters, the wood glows gold at a low sun, and leaves drift past Jane at dusk. It also fixes a truth problem: the docs say autumn, and the pixels do not.

**Q2. House identity on the town's tile houses** (M; the largest visible gain in the town). *Borrows: Minish Cap's Hyrule Town; Stardew's every-house-is-its-owner.*

- **What to do:** give each house a seed. Have the chunk painter look up the house's west end and its eave row with `run()`, and hash that cell; that cell must be within `REACH` (4 cells). For terraces wider than that, add an optional `TileSource::building(x, y) -> Option<u32>` that the View fills from the blueprint's building rects. That is presentation-side, but it adds a method to `TileSource`. From the seed, pick:
  - **Roof:** tile, slate or thatch, in three or four ramp shades (weathered, new, mossy).
  - **Wall:** plaster in cream, pink, limewash or ochre ("Suffolk pink"), brick, or flint with brick quoins.
  - **Door:** the colour comes from the cloth ramps (racing green, oxblood, navy, black). The style is panelled with a fanlight, a cottage ledged door, or a stable door, with a step and a boot scraper.
  - **Windows:** casement, sash or bay. The rhythm is per house, not `c.wx % 3`. One house in three gets shutters in the door's colour, and one in two a window box with a flower cluster.
  - **Extras:** one house in four gets ivy, wisteria or a climbing rose up the face. Add a house number or nameplate, and chimney pots of one to three on a stack that varies in height.
  - Calm the roof's barcode, following "Clusters, not noise": fewer, longer course highlights, and a ridge cap with a lit edge.
- **Files:** `jane-art/src/terrain/hard.rs` (`house_wall`, `window`, `roof`), and `terrain/mod.rs` (`TileSource`). Use `jane-present/src/terrain.rs` and `view.rs` only if `building()` is added.
- **Before → after:** eight identical cream-and-red boxes → a street where you can say "the green door with the roses".

**Q3. Ground rhythm: busy beside quiet** (S). *Borrows: Slynyrd's negative space and Minish Cap's grass clusters.*

- **What to do:**
  - Gate the reed, tuft and stone scatter through a low-frequency mask, so they gather in patches with clear ground between. Today's density stays inside the patches and drops to a fifth outside them.
  - Replace grid-like rock spacing with jittered clusters of two to four stones and a lone outlier. If the grid comes from the sim's rubble tiles, jitter the sprite within its cell and vary its bank pick; that stays render-only.
  - Raise flower density in groups of three to seven of one species and colour, never as single specks.
- **Files:** `jane-art/src/terrain/standing.rs` and `ecotone.rs`; the `detail` column in `data/looks/tiles.json`.
- **Before → after:** the marsh's static noise (`2026-10-01_46_biomes/`) → reedbeds with open water and paths between them.

**Q4. Blinks and idle business** (S–M). *Borrows: Eastward's NPC tasks and Sea of Stars' idles.*

- **What to do:**
  - Append a `Blink` FrameId: the eyes become a one-px line, held for 3 to 4 ticks every 3 to 7 seconds, timed by `h32(unit.id)`.
  - Add `Task` loops for scheduled townsfolk: the sweeper sweeps, Mr Cobb turns a page, the milkman sets down bottles, Mrs Wakes rocks (already partly done). Each loop is two to four frames from the pose table, chosen by schedule row and presentation-only.
- **Files:** `jane-art/src/person/pose.rs`, `draw.rs` and `held.rs`; `jane-present/src/people.rs` (`pick`).
- **Before → after:** a square of statues breathing → people who are doing something.

**Q5. Calm interiors' floorboards; tone down Burial ledgers** (S). *Borrows: Stardew's and Eastward's quiet floors that let furniture read.*

- **What to do:**
  - Halve the boards' grain contrast and the red streaks, and take wear a whole board at a time.
  - Ledger stones in the Burial go from one in eleven to one in forty, gathered near the tombs.
  - Bones gather in two or three heaps per room instead of a scatter.
- **Files:** `jane-art/src/terrain/hard.rs` (`boards`, `interior_floor`) and `terrain/interior.rs`.
- **Before → after:** `house-in.png`'s shouting boards → a warm, quiet floor.

### Medium

**M1. Ambient life layer** (M; the highest charm per line of code in this plan). *Borrows: Stardew's critters, Minish Cap's and Sea of Stars' "animated fauna", ART §2.8's unbuilt smoke.*

- **Where it lives:** a new `jane-present/src/ambient.rs`. It reads the View (biome, hour, weather, tiles near the camera) and emits `SpriteCmd`s from the existing creature looks (crow, butterfly, moth, hen and bat exist; add sparrow, robin, pigeon, duck and squirrel to `creature/bird.rs` and `quad.rs`) and `Particle`s. Seeds are `h32(cell, tick / period)`. It never touches the sim and gives nothing a footprint.
- **Who appears:**
  - Ground birds peck and hop, then fly off (a two-frame flap and a fade) when Jane comes within three cells. Ducks paddle on the lake.
  - Butterflies wander over flower patches by day; moths circle lamps at night.
  - A cat sits on a wall, crows sit on fences and a robin on a spade handle.
  - Chimney smoke rises as five puffs (ART §2.8), leaning with the wind; it is suppressed in the empty houses, which is uncanny.
  - Water glints (`Dot` sparks) and foam at the shore.
  - In the Works, pigeons and dust motes.
- **Caps:** T0 gets about 12 actors, T1 about 24 and T2 about 48.
- **Before → after:** a still diorama → a place that breathes. Crows that turn to watch her are a cheap Silent Hill note.

**M2. Foliage sway and rustle** (S–M). *Borrows: Stardew's grass and HLD's foliage.*

- **What to do:**
  - Render each placed crown, bush and reed bank in three frames: rest, then 1 px of shear on the top third and 2 px on the top fifth, one way and then the other.
  - Have `jane-present` pick the frame from `(tick / period + h32(cell)) % n`, with the period shortened by the wind.
  - Add a two-frame rustle on a reed or tuft sprite when she walks through it.
  - Leave the chunk-painted tufts static; sparse placed "tall grass" sprites carry the motion.
- **Files:** `jane-art/src/flora.rs` (frames), `jane-present/src/terrain.rs` and `present.rs` (the pick). `atlas.rs` has room.
- **Before → after:** a frozen wood → a wood in a breeze, which also sells the storm.

**M3. Tree species and silhouettes** (M). *Borrows: Minish Cap's tree variants, and readability by silhouette.*

- **What to do:**
  - Give `broadleaf()` a species profile: crown width-to-height, lobe count, gap fraction and trunk lean, plus root flare (2 to 3 px of roots spreading on the ground, lit and AO'd).
  - Species: oak (wide and low, a heavy trunk), ash (airy, gaps showing the sky), silver birch (a white trunk with black bars, a light crown), weeping willow at water's edge, hawthorn (small, wind-bent, haws), yew in the churchyard (near-black), holly (dark and glossy, red berries), and beech (copper).
  - Add a stump, a fallen limb and a fern clump to the woodland bank.
  - Choose by biome and features: willow within two cells of water, yew beside graves.
- **Files:** `jane-art/src/flora.rs` (the bank goes from 46 to about 75) and `terrain/standing.rs`.
- **Before → after:** `field-edge.png`'s orchard of clones → a hedgerow and a wood you could name.

**M4. Interiors that are lived in** (M–L). *Borrows: Stardew's and Eastward's two-tile back wall and clutter.*

- **Taller back wall:** draw the north face up into the wall and void cells north of it. This is presentation-only, because those cells are already solid or outside. Give the face a dado, wallpaper or limewash per house seed, and a skirting.
- **Hang wall decor on that face:**
  - a framed picture, a clock, a calendar, a mirror and a plate rack;
  - in the Arms, the inn sign's twin and a dartboard;
  - in the church, a hymn board.
- **Light:** each window on the back wall casts a warm parallelogram on the floor by day (an existing light kind, or a floor decal), and its frame goes dark at night.
- **Rugs:** walk-over floor decals (the `carpet` set-piece painter, reused as a render-only decal) under the table and by the bed and the hearth.
- **Clutter:** tables gain a teapot, a newspaper or a bowl as `small_thing`s drawn on the table sprite by its id hash.
- **Files:**
  - `jane-art/src/terrain/hard.rs` (`wall` face height inside), `terrain/interior.rs`, and `kit/set.rs` (`carpet` as a decal).
  - `kit/furniture.rs`: tabletop variants via `vary`.
  - `jane-present/src/props.rs` and `light.rs`.
- **Before → after:** `house-in.png`'s plank ballroom → Julie's house, with a clock that has stopped.

**M5. Dungeon framing and signature motifs** (L; the core fix for dungeons). *Borrows: Minish Cap's walls, doors, borders and motif; HLD's warning about "built out of tiles".*

- **Walls:** two-cell faces, drawn up into the wall mass as in M4, with a patterned top trim per dungeon.
- **Doorways:** framed arches with a lintel and two flanking motif props.
- **Floor borders:** an inset band one cell in from every wall, with corner pieces.
- **Wear:** worn lanes between the doors, computed from the room graph in the painter.
- **One motif per dungeon, repeated in every room:**
  - **Mine:** timber sets, rails underfoot, ore glints, and a lamp hook on every second set.
  - **Burial:** candle niches, ivy, and a carved angel at every door.
  - **School:** coat pegs, radiators, high windows that throw moonlight squares, and chalk on the floors.
  - **Museum:** rope stanchions, and plinths in lines with labels.
  - **Factory:** hazard stripes, belts and gantry shadows.
  - **Pipes:** a running water channel down the middle with walkways and grates either side.
  - **Library:** shelves that make the walls; the floor is a reading-room carpet.
- **Grading:** a grade tint per dungeon (warm amber for the mine, green-grey for the Burial, sodium orange for the Factory, cold white for the School) instead of one blue for all.
- **Files:** `jane-art/src/terrain/interior.rs` and `hard.rs` (`wall`, `sill`, `interior_floor`), `kit/set.rs`, `jane-present/src/atmos.rs` (grade by zone), and DUNGEONS §2.10 for placement.
- **Before → after:** `burial.png` and `factory.png`'s blue rectangles → rooms you can place on a map from a single screenshot.

**M6. Dialogue portraits and a Castle-themed UI** (M). *Borrows: portraits from Eastward and Stardew; minimal in-world glyphs from HLD.*

- **Portraits:** render a 64 x 64 bust per person from the Person composer at 2x, with the head and shoulders and four expressions (neutral, smile, worry, shock) as eye, brow and mouth variants. These are generated, not drawn.
- **Re-skin the chrome as Castle's own paper:**
  - the dialogue box as letter paper with a deckled edge and a stamp;
  - the hotbar as a railway-ticket strip;
  - the quest pin as a pencilled note;
  - the map as an OS-style sheet with a fold.
- **Files:** `jane-art/src/chrome.rs`, `person/` (a `bust` render), `jane-present/src/ui/core.rs`, `hud.rs` and `window.rs`.
- **Before → after:** `ui/ui-sheet-hud.png`'s grey slate → a UI that belongs to Jane's letter.

**M7. Garden recipes** (M). *Borrows: Stardew's yards and Minish Cap's fenced plots.*

- **What to do:** each house's front plot, from its seed, takes one recipe and is painted as render-only decals and flora:
  - **cottage garden:** mixed flower clusters, a rose arch, a crazy-paving path;
  - **veg patch:** cabbages and canes, a water butt;
  - **tidy:** a lawn, a bird bath and a sundial;
  - **neglect:** long grass and a rusted bike.
- **Boundaries:** picket, low wall, privet or iron railings, each with a gate to match.
- **Files:** `jane-art/src/terrain/standing.rs`, `kit/growing.rs` and `kit/structure.rs`.
- **Before → after:** identical hedged strips (`town-noon.png`) → gardens that say who lives there.

### Big

**B1. Hero objects and vignettes: one-off details per place** (L; "creative variety"). *Borrows: Eastward's thousands of touches, HLD's silent storytelling, Minish Cap's oversized landmarks.*

- **Rule:** every settlement and every dungeon wing gets one **hero object** no other place has, and every ~12 x 12 cells of the open county gets at most one **vignette** of two to four render-only pieces, seeded by cell.
- **Hero objects:**
  - the square's great horse chestnut with a ring bench (conkers in autumn, Q1);
  - the war memorial with poppies;
  - the bus shelter with a faded timetable;
  - the pub's hanging sign;
  - the reed camp's upturned boat and drying nets (`reed-camp.png` is empty mud today);
  - the Works' rusted crane;
  - the Canteen's tea urn steaming.
- **Vignettes:**
  - a bike against a gate;
  - a picnic blanket left out;
  - a scarecrow in Julie's old coat;
  - a child's chalk hopscotch;
  - a single shoe on the road;
  - a ring of mushrooms;
  - a deckchair facing the School.
- **The uncanny weave:** about one in six vignettes is subtly wrong. Two cups at a picnic and nobody there; a lit window in an empty house; a swing moving with no wind. This keeps the cosy-but-uncanny tone.
- **Files:** a decor table in `data/looks/` (new looks only), `jane-art/src/kit/*` for new shapes, and placement in `jane-present` (a decor pass over the View's cells, no collision, nothing `use`-able). Anything Jane must interact with stays a sim prop, authored in `jane-world`, and is out of scope here.
- **Before → after:** competent but generic places → places with a memory, which is what an owner means by "charm".

**B2. Boss-room and set-room payoffs** (M–L). *Borrows: Minish Cap's boss arenas, Sea of Stars' set pieces, HLD's titans.*

- **Every boss room gets:**
  - a unique floor: a mosaic, a sigil, or a pattern laid concentric to the room's centre;
  - symmetric braziers or pillars;
  - a raised dais, and a wall motif scaled up (the Mine's headframe wheel, the Burial's great angel, the Factory's furnace mouth glowing, the School's clock face over the hall);
  - a grade shift when the fight begins.
- **The boss door:** wider, framed and carved, so you know it from across the map.
- **Set rooms:** one per dungeon (the Museum's great hall, the School's assembly hall with rows of chairs) drawn symmetric and lit as a stage.
- **Files:** `jane-art/src/terrain/interior.rs` (special floor painters keyed by the room's node kind), `kit/set.rs`, `jane-present/src/atmos.rs`, and the DUNGEONS §2.10 rows.
- **Before → after:** the boss rooms are the same rectangles as every other room → the room is the reward before the fight starts.

**B3. Character appeal: emotes and secondary motion** (M–L). *Borrows: Sea of Stars and Eastward.*

- **Emote bubbles:** `!`, `?`, `…`, a note and a heart, as chrome glyphs over heads. They are presentation events keyed on dialogue and schedule state.
- **Secondary motion:** hair and hem one frame behind on turns.
- **Squash on landing and recoil**, and a hurt flash with a 1-px squash.
- **Builds:** more of them in the composer (stooped, broad, child, tall and thin), so silhouettes differ in body as well as hat.
- **Files:** `jane-art/src/person/pose.rs`, `draw.rs` and `hair.rs`; `jane-present/src/people.rs` and `fx.rs`.

**B4. Water with life** (M). *Borrows: Sea of Stars' water and Minish Cap's shorelines.*

- **What to do:**
  - Animated shore foam: two to three frames along the 1-px lit shore.
  - Lily pads and reeds at margins.
  - Ripple rings under her feet in shallows (`Particle::Ring` exists).
  - Fish rises as rings.
  - A mill-race or weir with white water at the lake's outlet.
- **Files:** `jane-art/src/terrain/ground.rs`, `jane-present/src/terrain.rs` and `fx.rs`.

### Order of work

Q1 → Q3 → Q5 → M1 → Q2 → M2 → Q4 → M4 → M5 → M3 → M7 → B1 → M6 → B2 → B4 → B3.

Autumn, rhythm and calm floors are cheap and tonal. Ambient life and house identity carry the most charm per day. The interior and dungeon framing share one technique (faces drawn up into solid cells), so do them back to back.

Every item ends with ART §3.1's review: before-and-after renders at 1x and 3x on soft, gl2 and wgpu, and `tools/art-review.sh` plus a written critique.

## 7. Rules for creative variety (proposed for ART.md §3.1)

1. **No stamp twice in view.** Two buildings, gardens or rooms visible at once differ in at least three of: roof, wall, door, window rhythm, boundary and hero detail. A test can count the distinct house seeds across the town's frame.
2. **Busy and quiet.** Every textured patch of ground has quiet ground beside it. Coverage is measured per 8 x 8 cell window and must vary by a factor of three or more across a scene.
3. **One hero, one story.** Every place has one thing only it has. Every room or yard has one arrangement that implies a person (cups, a coat, a book face down).
4. **Show the season.** Every outdoor frame shows autumn somewhere: a turned crown, leaf litter or a drifting leaf.
5. **Something moves.** Every outdoor frame by day has at least one living thing that is not a person (a bird, a butterfly, smoke or sway).
6. **Dungeons are framed.** Every room has walls at least two cells tall on screen, a floor border, and its dungeon's motif at least once.

## 8. Sources

**The Minish Cap**
- [TheGamer: The Minish Cap is the best looking 2D Zelda](https://www.thegamer.com/the-minish-cap-best-looking-2d-zelda/)
- [Wikipedia: The Minish Cap](https://en.wikipedia.org/wiki/The_Legend_of_Zelda:_The_Minish_Cap)
- [Spriters Resource: Minish Cap](https://www.spriters-resource.com/game_boy_advance/thelegendofzeldatheminishcap/)
- [Spriters Resource: Hyrule Town](https://www.spriters-resource.com/game_boy_advance/thelegendofzeldatheminishcap/asset/6526/)
- [Spriters Resource: Minish Village](https://www.spriters-resource.com/game_boy_advance/thelegendofzeldatheminishcap/asset/6540/)
- [Spriters Resource: Link's House tiles](https://www.spriters-resource.com/game_boy_advance/thelegendofzeldatheminishcap/asset/6465/)
- [Zelda Dungeon: Palace of Winds](https://www.zeldadungeon.net/the-minish-cap-dungeons-palace-of-winds/)
- [Zelda Dungeon: Temple of Droplets](https://www.zeldadungeon.net/the-minish-cap-dungeons-temple-of-droplets/)
- [Zelda Wiki (Fandom): Temple of Droplets](https://zelda.fandom.com/wiki/Temple_of_Droplets)
- [Zelda Wiki (Fandom): Cave of Flames](https://zelda.fandom.com/wiki/Cave_of_Flames)

**Eastward**
- [Game Developer: Eastward's creators on pixel art adventures](https://www.gamedeveloper.com/art/eastward-s-creators-share-insights-on-making-pixel-art-adventures)
- [80.lv: Eastward](https://80.lv/articles/eastward-charming-chinese-pixel-art-adventure)
- [Wikipedia: Eastward](https://en.wikipedia.org/wiki/Eastward_(video_game))
- [Chucklefish: Introducing Eastward](https://chucklefish.org/blog/introducing-eastward-by-pixpil/)
- [Eastward media](https://eastwardgame.com/media/)

**Stardew Valley**
- [Spriters Resource: Stardew Valley](https://www.spriters-resource.com/pc_computer/stardewvalley/)
- [Spriters Resource: Pelican Town (Fall)](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/88626/)
- [Spriters Resource: Birds](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/169032/)
- [Spriters Resource: Grass](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/223916/)
- [Spriters Resource: Walls & Floors](https://www.spriters-resource.com/pc_computer/stardewvalley/asset/88659/)
- [Stardew Valley Wiki: Weather](https://stardewvalleywiki.com/Weather)
- [Stardew Valley Wiki: Pelican Town](https://stardewvalleywiki.com/Pelican_Town)
- [TheGamer: Stardew weather effects](https://www.thegamer.com/stardew-valley-types-weather-effects/)
- [Destructoid: four seasons in one image](https://www.destructoid.com/four-seasons-in-one-image-shows-how-beautiful-stardew-valleys-pixel-art-really-is/)

**Sea of Stars**
- [MobileSyrup: Sea of Stars level design interview](https://mobilesyrup.com/2023/09/11/sea-of-stars-level-design-sabotage-studio-thierry-boulanger-interview/)
- [Megavisions: The art of Sea of Stars](https://www.megavisions.net/the-art-of-sea-of-stars-a-sea-of-pixels/)
- [ResetEra: Sea of Stars' pixel art](https://www.resetera.com/threads/its-frankly-insane-how-good-sea-of-stars-pixel-art-is.1048128/)
- [Sabotage press kit](https://sabotagestudio.com/presskits/sea-of-stars/)

**Hyper Light Drifter**
- [Game Developer: The ultra-modern stylings of Hyper Light Drifter](https://www.gamedeveloper.com/business/the-ultra-modern-stylings-of-hyper-light-drifter)
- [Hardcore Gaming 101: Hyper Light Drifter](https://www.hardcoregaming101.net/hyper-light-drifter/)
- [Hyper Light Drifter's pixel impressionism](https://hookshotchargebeamrevive.wordpress.com/2018/09/10/hyper-light-drifters-pixel-impressionism/)
- [grahuba: Art of Hyper Light Drifter](https://grahuba.wordpress.com/2016/09/14/art-of-hyper-light-drifter/)
- [Space Ape: HLD UI breakdown](https://medium.com/the-space-ape-games-experience/hyper-light-drifter-ui-breakdown-c2d9cfe0a192) (blocked to the fetcher; cited from its search summary)

**Craft**
- [Slynyrd: Pixelblog 20, Top Down Tiles](https://www.slynyrd.com/blog/2019/8/27/pixelblog-20-top-down-tiles)
- [Saint11: pixel art tutorials (vegetation, wind, idle, smoke, water)](https://saint11.art/blog/pixel-art-tutorials/)

## 9. Later (owner, 2026-10-03, after the week's reset)

- **Leaves that move within the plant.** Beyond a crown's lean, individual leaves and clusters should flutter: a few loose leaves tip and flicker on the windward edge, and the odd one lets go and joins the falling leaves. Owner: "moving some leaves etc could be good too but do it in a later pass".
- **Foliage sway feedback**, being fixed in batch 3: the first sway had too few frames and every plant stepped on the same tick. The fix is 6 to 8 small steps on a faster cycle, with a phase for each plant and a travelling gust wave.
- **Rebuild sway as a draw-time bend of one static sprite** (owner, 2026-10-03: "efficient rust code that just moves or manipulates a static image. no full frames of animation unless it's ultra small and efficient"). Today each swaying sprite bakes seven leans into the atlas (batch 3). Instead, keep one sprite and add a per-sprite *bend* to the Frame's `SpriteCmd`: an integer lean at the top and a row falloff. Each renderer offsets rows as it draws: a row shift on soft, the vertex or UV skew in the gl2 and wgpu shaders. The phase stays the same (class pace, gust wave, per-plant hash). This frees the atlas frames and keeps one look across tiers. The same rule applies to any later motion (leaves, banners, laundry, water): deform or move existing pixels, and add frames only when they are tiny.

