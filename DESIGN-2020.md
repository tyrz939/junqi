# Jane — The 2020 design folder

Everything that sits beside the GameMaker project in `Junqi-Legacy-GM/` and is not code: two Word docs, five spreadsheets, six annotated dungeon maps, a town map, a controller layout, a dungeon-graph icon kit, Tiled files, and 2.2 GB of art packs. Audited September 2026. None of the other docs had read this folder; `OLD-NOTES.md` covers the Trello boards only.

Pair with `LEARNING.md` (what the 2020 code ran), `OLD-NOTES.md` (what 2020 planned on Trello), `SYSTEMS.md` (the bar for the live build).

**Read this first:** the documents are thin. The three `.docx` files total about 600 words. The five spreadsheets hold about 30 rows between them, with no formulas. The real design content is in the annotated PNG maps. Where this file says *inference*, the source does not say it.

---

## 1. The story, as written

`Story.docx` (June 2020) is the only narrative document. It is four paragraphs and three bullets.

- The protagonist is **Jane**. "JoJo" appears in no design document; it exists only in 2020 quest strings in code. The live build's "Jane" is the original name, not a rename.
- The aunt is **"aunty Julie"**.
- On her **20th birthday** a letter appears on Jane's desk, "very old like it came from 18th century".
- The letter, verbatim: *"Dear Jane, Happy Birthday, I am your aunty Julie, can you come to visit me next week. I have already draw a map for you in case you cant find it. BTW, Enjoy your birthday! Julie"*
- Julie "is her only family member in the world. She wants to know about her parents and other family."
- The town is **"Town named Castle"**. Twelve hours by train. "Only one train per week on Sunday." She packs clothes and "a small present for her aunty".
- In the tunnel she "saw lots colourful things floating everywhere… she seems can hear the little things were singing something about Day and night".
- **She arrives at 5 pm**, stops "watching the beautiful sunset", and the house is "10mins away". "She couldn't send message out and no signals so she has to follow the map."
- "She doesn't know real journey just started."
- Key points: *Auntie sent letter, her only clue / Puzzle why does phone have no signal / Must find aunties house on map.*

Nothing after the arrival is written. There is no antagonist, no ending and no second act in any document.

**What the live build takes from this:** the letter's content, not its wording. The 2020 text reads as poor English, not as eerie, so the live letter is proper English with one uncertain line ("I have drawn you a map. It was right when I drew it."); see `VOICE.md`. New Game starts at **17:00**, not 2020's 19:00, so the first thing the clock does is a sunset and the lamp posts come on three real minutes later. The present is in the bag. The phone has no signal.

The only later story text is on the Burial Chamber map (§4.6): an "Evil wizard boss", a "magic ball" that "can help put up the shield" and "help teleport people who are well to butterfly forest". *Inference:* a town under threat, a protective shield, Butterfly Forest as refuge. The map itself says "something we need to decide on".

---

## 2. `design doc.docx` is a different game

One page, titled **JaneCraft**, March–June 2020. It is a survival-crafting pitch:

> Jane is a witch. A kind gentle witch. Except she does not know that yet. […] You will have to gather necessities for survival like food and water. […] Be careful at night-time as everything is not as it seems in the world. Something is still out there. Watching you.

Features listed: gathering, "Magic Crafting system", "Magic Combat Spells, costing certain reagents to use", "Building system for shelter", "Fire for cooking, boiling water, and alchemy", day/night, weather (rain, snow, temperature), "Dungeons will be platformer gameplay", "World will be top down, RTS style controls".

By the time the dungeon maps were drawn (autumn 2020) the platformer idea was gone: every map is a top-down Zelda floor plan. JaneCraft is where the elements column on the Trello dream board came from. It is not the game 2020 built and it is not the bar.

**Keep:** "A kind gentle witch. Except she does not know that yet." That sentence is why the live build starts Jane with no spells and teaches Icebolt in the kitchen. **Keep:** the tone line about night. **Leave:** hunger, thirst, shelter, reagent-cost spells, weather. The 2026 Phaser build added hunger and warmth (`survive.ts`); the audit found it did 0 damage at any frame rate above 10 fps and shook the screen forever. It is not in the live build.

---

## 3. Numbers

### 3.1 `health and damage.xlsx` — the only balance model

A plain six-row table. No formulas, no level curve, no stat scaling.

| Phase | Player HP | Enemy HP | Boss HP | Damage (average) |
| --- | ---: | ---: | ---: | --- |
| 1 (Before Mine) | 150 | 100 | 800 | 10–30 |
| 2 (Start Museum) | 300 | 200 | 1500 | 30–60 |
| 3 (Start BF) | 400 | 300 | 2000 | 50–80 |
| 4 (Start Factory) | 500 | 400 | 3000 | 70–110 |
| 5 (Start BC) | 700 | 600 | 4000 | 100–140 |
| 6 (End School) | 1000 | 800 | 5000 | 160–240 |

The sheet does not say whose damage the last column is.

**This explains the 2020 code.** `obj_player` has strength and spirit 140, so 700 HP: phase 5. The snake boss has 4000 HP: phase 5. Icebolt at spirit 140 hits for 112–129 and Fireball for 140–210: phases 5 and 6. The 2020 build was left tuned for testing the Burial Chamber, which the table places **fifth**. Nobody should copy those numbers into an opening.

**The live build uses phase 1** for the whole 2020-built spine: Jane has strength and spirit 30 (150 HP, 150 MP), yard enemies sit at 40–100 HP, bosses at 800, hits land for 10–30. The 2020 *formulas* are kept (`strength/8 + irandom(strength/32)`, `spirit*0.8 + irandom(spirit/8)`, 1-in-20 double); only the stats moved. When a later dungeon lands, its rows move up a phase. That is what the table is for.

### 3.2 `Crafting.xlsx`

Three material slots. Mat3 is "Small Water Vial" for every potion.

| Mat1 | Mat2 | Potion |
| --- | --- | --- |
| Gold Dust | Pansy | Small Manashield Potion |
| Gold Dust | Nasturtium | Small Life Steal Potion |
| Gold Dust | Honeylace Lily | Small Critical Potion |
| Stone | White Water Rose | Small Stone Skin Potion |
| — | Hemshade Root | Small Firelash Potion |
| — | White Water Cap | Small Sparktongue Potion |
| — | Night Lich Moss | Small Winterbite Potion |
| — | Savage Snakeroot | Small Stranglethorn Potion |

Plus Gold Bar → Gold Dust and Rocks → Stone. This matches `craftMap.gml` exactly.

**The sheet has no effect column.** What each potion does is written nowhere in 2020, and in code all eight share `obj_onUseManaPotion1` and restore 25 MP. The only design statement is the production-board card: "Potion on-hit toys (cast heals you, taking hits restores mana)". The live build's effects are therefore new, chosen to be those toys, and each is one `effects.json` row:

| Potion | Live effect |
| --- | --- |
| Manashield | 20 s: damage is paid from mana first |
| Life Steal | 20 s: 30% of damage dealt returns as health |
| Critical | 20 s: one hit in three is critical |
| Stone Skin | 20 s: physical damage halved |
| Firelash | 30 s: melee adds fire and sets burning |
| Sparktongue | 30 s: taking a hit restores mana |
| Winterbite | 30 s: melee adds frost and chills |
| Stranglethorn | 30 s: melee adds nature and poisons. Also the ingredient for Poisoned Rat Meat, which is its real 2020 job |

Every potion is "Small". No larger tier is listed anywhere.

### 3.3 The other sheets

- `Quest IDs.xlsx`: one row, `0 | ALL THE THINGS | Test quest`. There is no ID scheme.
- `teleport IDs.xlsx`: five entries — 4 Auntie House front door, (5, 6) Auntie Basement left / right, 10 Butterfly Forest, 14 Burial Ground. IDs only; no pairing table. They match the `door_id`s in the rooms.
- `credits.xlsx`: two entries (§6).

---

## 4. The dungeons, as drawn

### 4.0 Method

`Dungeon Graph Making Tools/` is a Zelda **lock-and-key dependency graph** icon kit: diamonds are things you acquire or activate (small key, key item, boss key, switch A–E), squares are things that block you (locked door, item obstacle, boss door, barrier A–E), ENTRANCE at one end and BOSS at the other. `Dungeon 1.psd` (July 2020) flattens to the same image as the 2017 sample, so no real graph was ever authored with it. The actual dungeons were drawn as MS-Paint floor plans with a shared legend (bolts for keys and spells, red stars for puzzles, four-point stars for bosses), modelled on the ripped Minish Cap maps in `reference material/`.

**The live build turns that kit into code.** `jane/src/world/validate.ts` is the graph: it floods each generated zone, collects keys from chests and guaranteed drops, opens what they fit, fires levers, plates, repairs and kill-triggers, and repeats until nothing changes. A seed whose boss door can never open is re-rolled before the player sees it.

### 4.1 Order (`dungeon progression.png`)

```
Mine  --[Explosion Spell, Repair, KEY for Museum]-->  Museum  --[Key to BF]-->  Butterfly Forest
                                                                                 [Grow Plant Spell, Butterfly Amulet]
Repair ----------> Underground Pipe System --(Sewer)--> Factory
Explosion Spell -> (Air Vent) ------------------------> Factory --> Grave Yard Tunnels --> School
```

The health table agrees: Mine, Museum, Butterfly Forest, Factory, Burial Chamber, School. **School is the finale and has no map.**

This contradicts the order every other doc assumed. `SYSTEMS.md` §9 ran mine → burial → snake, then factory / school / butterfly / pipes as a "second map". 2020 intended the **Burial Chamber fifth**, and the snake is one of four corner minibosses under an "Evil wizard". 2020 *built* the mine and the burial first because those were the rooms it was testing. Both facts are true; the live build carries the rooms 2020 built, at phase-1 numbers, and treats the drawn order as the roadmap (§7).

Two gates were already honoured by accident: the Museum key is found in the mine (the live vault chest holds `key_museum`), and **Repair is a mine reward, not a starting spell** (the dog teaches it when it sends you down; 2020 code had it in the starting book).

### 4.2 Gold Mine

Two floors, a minecart track with a switch and a "Minecart Ramp Jump", "Control Room", "First Aid Room", "Kitchen". Legend: Repair Spell — **"Requires material to repair (iron, wood, gold)"**; generic key; "Headmaster's Key"; "Vault Key"; key "Gnox Goldskin"; Explosion Spell; wood; iron; track switch; optional "5x Gold Bar". Mini-bosses **"Iron Knuckles"** and **"Headmaster"**. Repair gates "x4 Iron" (track), "x1 Wood", "x2 Wood". The boss is unnamed, behind the Gnox Goldskin door, with the Explosion Spell drawn inside its room. Heart and gem outlines in three rooms are not in the legend (*inference:* upgrades).

Live: Headmaster drops the vault key, the ornate chest behind the "x2 Wood" steps holds the Large Mine Key ("stamped GNOX GOLDSKIN"), the vault holds five gold bars, Iron Knuckles is the arena. Explosion and the cart are later rows.

### 4.3 Museum

"Lime Stone building". Wings Science / Magic / Arts / History, MEN and WOMEN toilets. Exit Key, Floor 1 and Floor 2 keys, Light Switch, "Damaged door, need to repair with wood", Maintenance Key (in the WOMEN toilet), "Key to toilets" (inside a boat exhibit in History), **"Key for Butterfly Forest"** (Magic wing), "To Sewer". No enemies or boss are marked. It is a key-shuffle building, not a combat dungeon.

### 4.4 Underground Pipe System

A black pipe network drawn over the town's road map, eight red arrows, no text. *Inference:* manholes. It is a connector between places, entered with Repair, leading to the Factory by "Sewer".

### 4.5 Butterfly Forest

"No keys, No doors, Forest dungeon". Needs the Museum key and a **"Net for catching butterflies"**. Eight butterflies, each its own puzzle: blow up a rock (Explosion), burn a tree so it drops seeds, scare off animals, dig through a cave, chase a fast one, grow purple flowers to attract one, grow a vine to a higher level, cross water ("maybe repair boat?"). "boss summon location 5 BUTTERFLIES". "8 BUTTERFLIES FOUND Amulet Collected". The Grow spell is "Found in ruined library only page left in the book". "gold stash x10".

The 2020 room `room_dungeon_butterfly_forest` is an **empty shell**: a filled floor layer, a room init and one exit. The 2026 Phaser build's "learn Grow, north fight, amulet" was an invention, not a port.

### 4.6 Factory

Entry by air vent or sewer. "Book worker diary explaining events in factory". **Electric spell**: "used to store and release electricity and control robots/machinery". "Lights come on when power on". "avoid robot los puzzle". "Drop in to genorator room through vent". Large robot mini-bosses: "Can use eletric spell to control these and smash the door, otherwise they're too powerful". The electricity orb is in **Auntie's basement study** (`auntie basement.PNG`), which also shows a "Potion making Room", a "storage room locked", and two iron doors on one iron key ("need open one of the two doors only").

Live cellar: potion room with a bench, locked storage room (the wood and iron the mine wants), two iron doors on one key type, a study with nothing in it yet.

### 4.7 Burial Chamber

"Underground Burial Chamber". START in the centre, four corner bosses: **Snake, Spider, Undead Flower, Zombie**. "Evil wizard boss unlocked after kill 4 mini boss". Theme, verbatim: **"Cold, dark, foggy place. lots of death in here"**. Elements: blue flames, ice area, ghostly wind, dead dry trees, "Black scary birdy", undead spiders and snakes, poison areas, ghosts, zombies, undead flowers, "Lots of ruined/broken things". Props: "10x Glow Dust - for lighting", cactus, "fire breathing pumpkins", **"Large Torch can push"**, "Flame Spell". Reward: the magic ball (§1).

`Burial Chamber doc.docx` (January 2021) is an unfinished walkthrough of the "Undead Plant Area": kill plants with collected **plant poison**, avoid an unkillable patroller in the main corridor, fetch a "Large Seed for later use", chests of poison, and a note that breaks off mid-sentence: "…the person buried here loved nature and was a ."

Live: the blue flames are `torch_blue` props that only light on frost damage (2020 code did this too), the scroll in the garden finishes the broken sentence, the burial snakes that cannot be fought are fed poisoned meat (2020 code). The other three corners and the wizard are later rows.

### 4.8 The town (`Sunshine.PNG`)

A hand-drawn map over the road network in `map.png`. West: **Entrance** (far west), **Aunties House**, shop streets. A diagonal river NW→SE with a bridge and a walk bridge, a statue on a roundabout, a lake. North: supermarket, gym, library, school with playground, theme park. South-west: forest with the **abandoned house** and **"Abandond Car"**, farm, farm house, "shed for cows", **"Grave Yard + Dungeon 'Burial Chamber'"**. South-east: **Museum** in four quadrants, **"Semi abandon Factory"**, mountain "water come out from", cave, **gold Mine**.

The 2020 room did not follow this: it put Auntie's yard far south-east with the mine and burial mouths beside the stoop. The live county follows the room (two camps, a road, a river) and keeps the map's forest car.

---

## 5. Controls (`xbox controller layout.jpg`, November 2020)

| Pad | Action |
| --- | --- |
| Left stick, D-pad | Movement |
| **Right stick** | **Aim** |
| A / X / Y | Spell 1 / 2 / 3 |
| LB / RB | Spell 4 / 5 |
| B | Use / Talk / Push / Pull |
| RT | Sprint |
| View | Inventory / Spellbook / Map |
| Menu | Pause |

Twin-stick aim, five spell buttons. This is the layout in `jane/src/input/input.ts`. It also settles a question `OLD-NOTES.md` got wrong: see §7.

---

## 6. Assets and licensing

`credits.xlsx` lists two things: a bat sprite (bagzie, OGA-BY 3.0) and the font m5x7 (Daniel Linssen, free with attribution).

`Assets/` is about 6,940 files and 2.2 GB, almost all **purchased packs that cannot ship in a public repository**: Time Fantasy characters and animals (finalbossblues), Daniel Thomas "2D Hand Painted" tilesets (the Tiled `Town.tsx` depends on one), gamedeveloperstudio.com smoke effects. Several packs have no licence file at all (UI Window Pack, Spellpack 1, Lava Cave, three icon packs, controller icons). Fonts: dogica (SIL OFL, attribution), Pixeled, Lucida Sans (commercial). Three music files with no licence information. `reference material/` is ripped Nintendo and PopCap art.

`Assets/Characters/Dungeon Burial Ground/` holds 131 sprites that look hand-made (cactus, mouse, a pumpkin named "napkin", spiders and a spider boss, undead flower boss, zombies, a snake head). Authorship is stated nowhere.

**Rule:** nothing from `Junqi-Legacy-GM/` enters the repo. That folder stays git-ignored. The live build's art is source code: palette-character grids in `jane/src/art/`, rasterised at boot. It ships no image, font or audio file, and needs no credit line.

---

## 7. What this folder changes

| Believed before | The folder says | Live build |
| --- | --- | --- |
| Player was JoJo; Jane is a remake rename | Jane in every document. JoJo is a code string | Jane |
| New Game at 19:00 (2020 code) | Arrives 5 pm at sunset | 17:00 |
| Burial and the snake end the first act | Burial is dungeon **5 of 6**; the snake is one of four corner bosses under a wizard | Burial is built (2020 built it), at phase-1 numbers, and the quest text does not call the snake an ending |
| Repair is a starting spell | Repair is found in the mine and costs iron / wood / gold | Dog teaches it with the mine quest; every Repair has a `needs` list |
| Post-snake order: factory, school, butterfly, pipes | Mine → Museum → Butterfly Forest → (Pipes →) Factory → Burial → School | Roadmap order. The next zone is the **Museum**; its key is already in the mine vault |
| 2020 kept locked-target combat; direction aim was a card "do not silently flip" | The pad layout says **Right stick: Aim**, and the final 2020 code has every player-target assignment commented out: bolts flew at the mouse | Aim-based. Loudly, not silently: see `OLD-NOTES.md` "Gameplay fork" |
| 2020 player numbers are the numbers | They are phase-5 test values | Phase 1 |
| The 364 GameMaker sprites are a later art import | They are purchased packs | Never. Art is code |
| Potions have designed effects | No effect is written anywhere | New effect rows, built on the one design hint |

**Still undecided in 2020, and still undecided:** who the wizard is, why the ball matters, where Julie went, what the tunnel vision was, what the School is. Those are content. The engine does not wait for them.

---

## Source

`Junqi-Legacy-GM/`: `Story.docx`, `design doc.docx`, `Crafting.xlsx`, `Quest IDs.xlsx`, `health and damage.xlsx`, `teleport IDs.xlsx`, `credits.xlsx`, `map.png`, `Sunshine.PNG`, `xbox controller layout.jpg`, `Dungeons/**`, `Dungeon Graph Making Tools/**`, `Tiled Files/**`, `Assets/**`, `reference material/**`. Handwriting marked "[?]" in the audit notes was not read with confidence ("gov office", "fishing").
