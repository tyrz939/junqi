# Jane, Art

How every pixel in the native build is made. Pair with `PRESENTATION.md` (what draws it, lights it and lays out the UI), `ARCHITECTURE.md` (the engine, and the `View` presentation reads, its §11), `PORT.md` (the plan: P5 is the art phase, §6.i is where render-only tiles leave the sim) and `PLAN.md` §6 (the art gaps the web build left).

**Status:** §8 steps 1 and 2 are built: the palette, canvas, font, chrome and light pass, and the Person composer with its looks (`data/looks/persons.json`: Jane and her seats, the town, the country folk, the villagers). Step 4 is built (2026-09-27): the creature plans `quadruped_mid`, `quadruped_small` and `bird` with the county's twelve animals, Julie the dog first (`data/looks/creatures.json`), and the Person `bone` skin (the skeleton). Step 5 is built: the prop kit's seven first-walk families and the first of vegetation and debris: with the buildings, 192 of the 267 prop sprites have a look (`data/looks/props.json`); the station, the lamp road, Julie's yard, house and cellar, and the mine dress. Of step 6 the building painter (`data/looks/buildings.json`, §2.4) and the icons at 32 and 16 (`data/looks/icons.json`, §2.5) are built; the fx tables and the people's attack and cast cycles are not. The presenter draws all of it (`jane-present`: `people`, `creatures`, `props`). The rest is design. The TS build's generators (`jane/src/art/`) are the reference for method until P5 lands and `jane/` is archived at P10; they are not the reference for the look, which moved up on 2026-09-26 (§0).

Crate `jane-art`: depends on `jane-core` and `jane-data` only. No SDL. **No floats**: it sits in the lint table of the deterministic crates (PORT §3.4), so a sheet hashes the same on every target. Sphere shading, bevel normals and gradients come from integer tables. No allocation after boot beyond the atlas pages. Runs headless in tests.

**Rule: art is a function.** `render(look, seed) -> SpriteSet`. Nothing is drawn by hand; everything is described in a row and generated. The TS build did this for houses, trees, terrain, corpses, town props and icons (about 160 of its 410 sprites) and that is the part of it that holds together best. The other 250 are hand grids (`unit-base.ts` alone is 2.4k lines holding sixteen drawn sprites), with three separate people composers, three pixel-canvas helpers and about seven hash functions beside them. Here generation is the only way, and there is one of each helper. Generation also carries the whole of the new look: a generator that knows it drew a sphere can emit that sphere's normals, its height and its contact shadow for free, and a hand-drawn sprite could not.

**Rule:** presentation never touches the sim. A sprite's seed is a stable id (a prop's row id, a unit's id, a cell), never the sim's RNG, so what she sees is the same on every machine, every load and every seat.

```
jane-art/
  palette   the one RGB table, ramps of six to eight tones, region styles, map inks
  canvas    the one four-layer pixel canvas at 16 px a cell, and every drawing primitive
  hash      h32 salts over jane-core's FNV-1a and mix32
  font      stroke glyphs rasterised at boot; Fine, Small, Head and Title faces
  person    the people composer and its frame tables; bone, the skeleton
  creature  body plans and their gaits: quad (dog, sheep, cat, rat, rabbit, fox), bird (hen, crow)
  kit       the prop kit: parts, sign, lamp, barrier, container, ritual, furniture, structure, growing
  house     the building painter
  icon      icon classes, ramps, overlays
  flora     trees, bushes, reeds, crops, canopy strips
  terrain   the tile painter: TileStyle rows, autotile, chamfer, roofs, cliffs, fences, height, wetness
  corpse    dead frames from living ones
  fx        effect kinds tables and colours (the particle runtime is jane-present)
  weather   mist, rain, snow, motes, fireflies, smoke, caustics, cloud shadow, god rays; the parallax layers
  chrome    panels, buttons, bars, slots, cursors, pad glyphs, the title scene
  looks     walks the compiled LOOKS table and renders every row into the atlas
  sheet     contact sheets and the PNG encoder
```

## 0. The look

Decided 2026-09-26: modern pixel art, considerably above the SNES, nostalgic for pixel art fans but beautiful and impressive first. The references are Sea of Stars (large readable sprites, rich palettes, soft light, layered atmosphere) and Elysian Shadows (normal-mapped dynamic 2D lighting, cast shadows). What that means in numbers:

| Measure | Value | Note |
| --- | --- | --- |
| Internal canvas | 768 x 432 | integer-scaled to the window; the view is wider on a wider window, never smaller |
| Screen pixels per cell | **16** | the sim cell is unchanged at 8 units = 1 m, so 1 screen px = half a sim unit |
| A person | 32 x 40, feet at (16, 36) | |
| A prop | `w * 16` wide | never overhangs its footprint |
| An icon | 32 x 32 in the bag, 16 x 16 as a chip | |
| A tile | painted at 16 px a cell | sub-cell detail: blades, pebbles, grain, slate courses at 2 px |
| Layers per sprite and tile | four: albedo, normal, emissive, height (§1.1); walks are six frames (§4, decided 2026-09-27) | |

Still **zero hand-drawn sprites** (§1). The generators get richer primitives instead (§2.3), and every one of them knows the shape it drew well enough to light it.

---

## 1. The contract

```rust
pub fn render(look: &Look, seed: u32) -> SpriteSet

pub struct SpriteSet { w: u8, h: u8, ax: u8, ay: u8, frames: FrameMap, roles: RoleMap, emits: RoleSet }
pub struct Frame { albedo: Vec<Ix>, normal: Vec<[u8; 2]>, emissive: Vec<Ix>, height: Vec<u8> }   // all w * h; Ix is a u16 master-palette index
pub enum FrameId { Down, Down1, Down2, Down3, Down4, Down5, DownB, Up, Up1, Up2, Up3, Up4, Up5, UpB,
                   Side, Side1, Side2, Side3, Side4, Side5, SideB,
                   Atk1, Atk2, Atk3, Cast1, Cast2, Cast3, Hurt, Dead, Dead2, Base, Base2, Base3, On, Open }
pub struct RoleMap([Role; 1024])              // master-palette index -> Hair | Skin | Coat | CoatShade | Legs | Boots | Glass | Eye | ... | None
pub enum Look {
    Person(PersonLook), Creature(CreatureLook), Prop(PropLook), Building(HouseLook),
    Icon(IconLook), Flora(FloraLook), Tile(TileStyle), Weather(WeatherLook), Parallax(ParallaxLook),
    Swap { of: SpriteId, roles: Vec<(Role, RampId)> },
}
```

`Look` is a `jane-schema` struct (serde, `deny_unknown_fields`) compiled by `jane-data` into `LOOKS: &[(SpriteId, Look)]` (`jane_data::looks()`), a static beside the catalog and outside its content hash: a new coat changes no behaviour, so it never makes a save, a replay or a fixture stale. Its meaning lives here: `jane-art::looks` renders every entry, and nothing else in the game reads a look.

As built (step 2), the API the renderer and the atlas builder consume:

```rust
jane_art::looks::all() -> Result<Vec<Rendered>, String>        // every look x variant x seat
jane_art::looks::render(name) -> Result<Vec<Rendered>, String> // one sprite's sets
pub struct Rendered { sprite: SpriteId, name: &str, variant: u8, seat: u8, set: SpriteSet }  // key(): "jane@2", "villager_old#1"
jane_art::person::render(&PersonLook, seed) -> Result<SpriteSet, String>
jane_art::person::seat(&SpriteSet, seat) -> SpriteSet           // seat 0 is the set; 1..=3 swap the coat
pub struct SpriteSet { w, h, ax, ay: i32, frames: Vec<(FrameId, Canvas)>, roles: Vec<(Role, Ramp)>, emits: Vec<Role> }
SpriteSet::frame(FrameId) -> Option<&Canvas>;  SpriteSet::swap(Role, Ramp) -> SpriteSet;  SpriteSet::hash() -> u32
```

`RoleMap` became `roles: Vec<(Role, Ramp)>`: every role is drawn in a ramp of its own, so a swap by role is a remap of that ramp's tones (and, in a dead frame, of their pallid twins).

As built through step 6: `Look` is `Person | Creature | Prop | Building` (`jane-schema::model::looks`), each family in its own file under `data/looks/`; `looks::family(Family)` renders one family, and the golden keys are `unit:` for people and creatures and `prop:` for props and buildings. `FrameId` gained `Idle` and `Idle2` (a creature's idle pair, §2.2) and `Role` gained `Fur`, `Belly`, `Mark` (creatures) and `Body`, `Trim`, `Flame` (props), all appended so no older hash moved.

```rust
jane_art::creature::render(&CreatureLook, seed, attacks) -> Result<SpriteSet, String>
jane_art::kit::render(&PropLook, sprite, seed) -> Result<SpriteSet, String>    // footprint from the prop rows
jane_art::house::render(&HouseLook, sprite, seed) -> Result<SpriteSet, String>
```

| Rule | Why |
| --- | --- |
| **No pixel grids in source.** A test greps `jane-art` for three or more consecutive string literals of eight or more characters drawn from `[.a-zA-Z0-9#]` and fails on any. A second greps for `include_bytes!` and for any integer array literal over 32 elements outside `palette`, `hash` and the named lookup tables of `canvas` (Bayer, sphere normals, sine, falloff), which the test lists by name. No exception list beyond those names. No per-pixel literal art of any size, in any encoding | Mechanical, so it cannot erode; a 32 x 40 grid is as tempting as a 16 x 20 one was |
| **Pure and deterministic.** Same `Look` and seed give the same bytes in all four layers. A golden FNV hash per sprite id over all four layers lives in `tests/golden.txt`, re-blessed with `jane sheet --bless` | Unintended drift fails loudly; intended drift is a diff the owner sees |
| **Indices, never RGB.** Albedo and emissive are palette indices; only `palette` holds colour | One hand across every generator; swaps, ghosts and dead ramps are index remaps |
| **Four layers from one drawing.** A generator never writes a normal or a height by hand; the primitives emit them from the shape they drew, in drawing order | A bevel is lit as a bevel because `rect_bevel` knows it is one |
| **Seeds are stable ids**: `h32(prop.id, 0, salt)`, the unit id, cell coordinates. Never the sim RNG | Presentation may not perturb the sim; a sprite looks the same after a load and on every seat |
| **Frame vocabulary kept and grown.** The TS build's twelve names carry with their meaning (`down` is the standing frame, `base` a prop at rest, `open` over `on` over `base` is still the renderer's pick); the walk, breathe, attack, cast and hurt frames are added beside them | Renderer and tests carry; nothing that read `Down` reads anything else |
| **Anchors.** Units: feet at `(w/2, h - 4)`. Props: `(0, h - foot_h * 16)`. Icons: `(0, 0)` | The y-sort, the cast shadow and the map chart read them |

### 1.1 The four layers

| Layer | Format | Holds | Who reads it |
| --- | --- | --- | --- |
| albedo | indexed, up to 64 entries per sprite drawn from ramps of six to eight tones; 0 = clear, 1 = baked contact AO | the colour under flat light, shaded from the top-left as a base | the blit |
| normal | tangent-space RG8: `nx`, `ny` in 0..255 for -1..1; `nz` is the remainder | the surface's facing: a `rect_bevel` knows its bevel, an `ellipse_lit` its sphere, `courses` their mortar grooves; composed by drawing order, so a later shape overwrites an earlier one's normals where it covers it | the light pass |
| emissive | indexed, 0 = none | lamp glass, lit windows, orbs, spell parts, eyes at night: the pixels that shine when the ambient is low | the light pass, added after the multiply |
| height | 0..255 per pixel | how far the surface is above the ground in screen px: a person's head is 40, a barrel's top 12, a wall tile its wall height; clear is 0. An upright sprite's pixel stands its row's height above the feet (`Canvas::upright`), so its shadow is its silhouette thrown from the feet's row, `h / tan(elevation)` px from the sun (`light::light_upright`, the reference a backend matches); a flat thing's height is a height field over the ground (`light::light`) | shadow casting, water, the y-sort of tall things |

A sprite has all four at one size, always. The font and the chrome carry albedo alone and are flagged flat (normal straight up, height 0, no emissive) rather than stored four times. A test walks every sprite and checks: four layers, one size; a normal decodes to unit length within 2 of 255; emissive is non-zero only where the role is in the look's `emits` set; height is at least 1 on every opaque pixel and 0 on every clear one.

## 2. Generator families

| Family | TS build | Native |
| --- | --- | --- |
| **Person** ~85 | Jane and her seats, town, folk, villagers and tales people, bandit, soldiers, keepers, skeleton, miniboss, boss, shade, statue, armour, waxwork; three composers plus hand grids | One composer at 32 x 40. Skeleton, waxwork, statue, armour and shade are skin ramps plus a `ghost` flag |
| **Creature** ~24 | dog, rat, rabbit, cats, sheep, hens, crow, fox, butterfly, moth, emperor, bat, spiders, lurker, snake, cactus, flowers, pumpkin, factory hauler and sentry; hand grids | Nine body plans (§2.2), each with its own gait. Dead is a pose, not a drawing |
| **Prop kit** ~200 | containers, furniture, signs, lamps (seven sets by four sides), levers, plates, torches, fences, gates, shutters, webs, debris, small things, luggage; mostly generated already | One kit, twelve families (§2.3), on the richer primitives |
| **Building** ~12 | cottages, farmhouse, barn, inn, shed, reed hut, boarded cottage, shelter, steeple, coop, tent; `house()` painter | `house()` extended with style, storeys, dormers, porch, lean-to, boarded, silhouette; materials as ramp ids; windows emit |
| **Icon** 91 | templates and ramps | Classes by material ramp by overlay (§2.5), at 32 and 16 |
| **Flora** 43 | fully generated | Ported onto `Canvas` at 16 px a cell; tones become ramps; leaves as `strokes` |
| **Terrain** 51 tiles | painter, seven tables | Painter ported; the seven tables become one `TileStyle` row per tile with height, normal and wetness (§2.6) |
| **Corpse** | ten styles, some drawn | Drawn frames go; every creature poses its own `Dead` (§4.1) |
| **Effects** | kinds tables | Colours from the palette; `SCHOOL_COLOUR` lives here, once; spell parts emit |
| **Weather and parallax** | none | New (§2.8): every drop, mote, wisp and cloud shadow is generated, as is the sky |

### 2.1 Person

32 x 40, feet on (16, 36).

```json
"look": { "family": "person", "build": "slim",
  "head": { "hair": "long", "hair_ramp": "hair_dark", "hat": "none", "skin": "skin", "face": "plain" },
  "body": { "coat": "coat", "coat_ramp": "cloth_plum", "front": "scarf", "front_ramp": "cloth_mustard",
            "legs": "trousers", "legs_ramp": "cloth_brown", "boots": "boots", "pack": true },
  "held": { "item": "none" }, "extras": [], "emits": ["eye"] }
```

| Axis | Values | Notes |
| --- | --- | --- |
| `build` | slim, broad, child, stout | A per-build table of eight numbers: head y, head w, shoulder w, waist w, hip w, leg h, arm y, arm l. Every other axis and every frame table reads it |
| `hair` | short, cropped, long, bun, pigtails, bald, curlers, wet | `hair_ramp` from the hair group; drawn as `strokes(hair)` over a `soft_ellipse` skull, so it has volume and a highlight |
| `hat` | none, cap, brim, peaked, helmet, scarf, veil, cloche, panama, diving | Drawn over hair; `up` shows its back; a brim casts a small shadow on the face by height. `hat_ramp` names its cloth (the coat's when left out). `veil` and `diving` are placeholders until the tales' step |
| `skin` | skin, skin_pale, skin_dark, bone, wax, stone, metal, none | `bone` is the skeleton, `wax` the waxwork, `stone` the statue, `metal` the armour; `none` with `ghost` is the shade. Skin is never dithered. **`bone` as built** (`person::bone`, step 4): the undead are townsfolk who were outside when the county changed its mind, so the skeleton wears what is left of a coat. The head is carved into a skull (the jaw narrower than the cheekbones), with sockets in `K` and a pinpoint of ember in each when the look emits its eyes, a nasal hollow and a row of teeth over the jaw's line; no hair. The neck is two px of vertebrae, a hand three px of knuckles, a bare leg a two-px shin with a knee's knob. The coat's hem is torn into tongues by hash, open in front on a ribcage (a lit sternum, ribs in shade lines), torn at the back over the spine. A fallen skeleton lies without a pool |
| `face` | plain, glasses, beard, grim, none | Two eye pixels of `Eye` role, a nose shadow, a mouth line; `glasses` is two `k` rings with a `glint` |
| `coat` | coat, dress, gown, apron, smock, jacket, nightdress, overcoat, canvas, cardigan | `coat_ramp`; `folds` gives cloth its shade bands; the seat swap replaces this role only (§3) |
| `front` | none, apron, shirt, waistcoat, scarf, tie, braces | `front_ramp`; goes to coat on `up` |
| `legs`, `boots`, `pack` | trousers, skirt, bare, pyjamas; boots, shoes, bare; true, false | `legs_ramp` (under a skirt, the stockings; a skirt shows as its own panel only under a coat too short to cover the knee); `boots_ramp`, leather when left out; the pack is drawn on `up`, a strap on `side` |
| `held` | none, hammer, pole, suitcase, dish, bell, lantern, billhook, broom, book, pipe | The composer owns the hand position per frame, so a held thing follows the swing; a held item declares its `hand` for the mirrored side; a lantern emits. Parsed from step 2, drawn from step 7 (§8) |
| `extras` | watch_chain, bell_ankle, shawl, seated, wet, stoop | Any number. `stoop`: an old back, shoulders a px forward and down, the head a px further. `shawl`: over the shoulders to a point, in the hat's ramp when she wears no hat but names one, else the front's. The rest are drawn from step 7 |
| `emits` | eye, glass, held | Which roles may write the emissive layer; anything else is a test failure |
| `ghost` | true, false | Ramps go to mist and the figure is a 50 % checker; height halves so its shadow is faint. No alpha anywhere in a sprite |

Composition order: legs, boots, coat, front, arms, skin, face, hair, hat, held, extras (long hair behind the head, the far arm and leg, and on `side` the pack are drawn first, and the near arm last). Cloth is `polygon_cloth`: large calm areas of its base, a two-px lit edge on the light's side and the far three tenths in shade, the shoulder's turn in lift; then the painter's clusters: folds under the arms, a waist seam or a belt (two rows of leather lit on top, a brass buckle), hem folds each a line of shade with its lit ridge beside it, lapels as shapes (the near one lit, the far one in shade), a yoke, pocket flaps, an apron's bib, panel, ties and strap. Builds and cuts are silhouettes: broad shoulders square, the rest sloping; a stout belly in profile, a chest on the broad; a fitted bodice over a trapezoid skirt; a cassock to the ground; a shawl; a stoop. Heads are `ellipse_lit` with a jaw a little squarer than an egg, the face painted in four skin tones (skin is never dithered): the fringe's cast shadow on the brow, the cheek and jaw on the far side in mid, a lift on the near cheek, a two-px nose shadow, a mouth; eyes two px square under a lid line three px wide, a glint in their top corner. In profile the neck leaves the skull behind the jaw. Hair is drawn on a layer of its own, held to three tones, given a two-row highlight band arcing over the crown on the light's side and two or three strand lines, then stamped as one part so one head of hair never seams against itself. Sleeves have a lit cuff and, in profile, an elbow; hands are a 4 x 3 skin mitt with its shadow px. Cast shades are laid as clusters with `shade` (a brim on a face, a chin on a neck, a head on a collar, the far limbs in the near ones' shade).

Every part writes its normals and its relief as it goes: relief is what stands in front of what (a sleeve two px proud of the coat). Then one finishing pass, in this order: `retone` gives each material its own few tones (cloth a light, a base and a mid, a shade where the light never reaches; skin a light, a base and a rose mid), `unchecker` and `declutter` leave the shading in clusters of two px or more, `despike` takes off any pixel with clear on three sides, `ao_contact` lays the contact shadow as a solid ellipse, `outline_sel` draws the selective outline with seams by relief, `declutter` runs again inside the line, and `upright(ay)` writes each pixel's true height above the feet (5 px a 4 rows: the head stands 40, the shoulders about 19, a hem about 7), the field a sun or a lamp casts from. No generator types an outline or a height.

The build table as built (px, 32 x 40, feet on row 36): `slim` head_y 4, head_w 16, shoulder_w 12, waist_w 10, hip_w 12, leg_h 7, arm_y 22, arm_l 9. The skull is `head_w - 2` wide and 13 tall two rows under head_y, the neck two rows, the hip `36 - leg_h`. `child` (the town's four), `broad` and `stout` are first drafts in the same table; only `slim` is tuned.

### 2.2 Creature

```json
"look": { "family": "creature", "plan": "quadruped_mid", "size": "M",
  "ramps": { "body": "fur_tan", "belly": "fur_cream", "mark": "k" },
  "features": { "ears": "floppy", "tail": "long", "eyes": "dark", "snout": 2, "legs": 4 }, "emits": ["eye"] }
```

| Plan | Box | Rows | Rules | Gait |
| --- | --- | --- | --- | --- |
| `quadruped_small` | 24 x 20 | cat, rat, rabbit, fox | Body `soft_ellipse` 16 x 10 in `strokes(fur)`, head 8 x 8, ears and tail by feature, 2 x 4 leg posts. `mounted` adds a plinth (the museum) | four-frame trot, diagonal pairs |
| `quadruped_mid` | 32 x 24 | dog, sheep | Body `soft_ellipse` 22 x 12. `fleece` is five overlapping lit spheres. The dog has floppy ears and a `k` nose with a `glint` | four-frame trot |
| `bird` | 20 x 20 | hen, crow | Egg body in `strokes(feather)`, head blob, comb in `r`, beak in `y`, stick legs | two-frame step, head bob |
| `flyer_insect` | 24 x 20; XL 48 x 40 | butterfly, moth, emperor | Two wing ellipses a side with a `gradient`, a mark spot per wing. Anchor four rows below the body: it hovers | three-frame wing beat: open, half, closed |
| `flyer_bat` | 32 x 24 | bat | Body 8 x 10, wings as filled three-segment polylines | three-frame wing beat |
| `arachnid` | 32 x 32; XL 48 x 48 | spider, queen, lurker | Abdomen and cephalothorax as spheres, eight two-segment legs, two to eight eyes that emit | four frames, alternate legs advance |
| `serpent_head` | 32 x 32 on (16, 16) | snake | Wedge head by facing, `y` eyes. The body is drawn by the renderer as lit spheres along the trail, each offset by segment for the slither; `Dead` is a slack coil | slither by segment offset |
| `plant` | 32 x 40; XL 48 x 56 | cactus, flower, great_flower, pumpkin | Stem and head by feature: bloom, pad or gourd; roots spread at the foot | two-frame sway |
| `machine` | 32 x 40; 48 x 48 | hauler, sentry | Boxy iron in `rect_bevel` with `grain(iron)`, a rivet course, a glass eye that emits on `On`, tracks or legs. `Dead` is scrap | four-frame track or leg cycle; a piston |

Every plan emits `Down .. Down3`, `Up .. Up3`, `Side .. Side3`, a breathe frame per facing, `Atk1..3` if it attacks, `Hurt`, `Dead`. A plan that beats wings or slithers emits its own cycle under the walk names, so the renderer's frame table is one table.

**As built (step 4, 2026-09-27).** Three plans and eight anatomies; the rest of the table is step 7's.

```json
"dog": { "family": "creature", "plan": "quadruped_mid", "anatomy": "dog",
  "ramps": { "body": "cloth_black", "belly": "hair_white", "mark": "hair_fair" },
  "features": { "ears": "flop_one", "tail": "plume", "markings": ["tan_points", "blaze", "grizzle", "tip_white"],
                "collar": "cloth_brick" } }
```

| Axis | Values | Notes |
| --- | --- | --- |
| `plan` | quadruped_mid (32 x 28, feet on 16, 24), quadruped_small (24 x 20 on 12, 16), bird (20 x 20 on 10, 16) | The mid box is 28 tall, not 24 (decided): Julie's pricked ear and her sitting head need the rows |
| `anatomy` | dog, sheep, cat, rat, rabbit, fox, hen, crow | The build on the plan's rig: a torso of a rump and a chest joined at the waist, a neck, a skull and a muzzle, leg columns, where the tail roots; and facing the viewer, the back's line, the head's box and the body's breadth (`quad::anat`) |
| `ears` | flop, flop_one, prick, round, tall, side, none | `flop_one`: one up, one folded, a dog who has heard it all before |
| `tail` | plume, long, thin, puff, stub, brush, fan, none | A thin tail (the rat's) is bare skin |
| `markings` | tan_points, blaze, grizzle, socks, tabby, mask, tip_white, dark_face | Dyed over the shading (`Canvas::dye`), so a marking turns with the body |
| `collar` | a ramp | A collar and a brass tag |
| `ramps` | body, belly, mark | Fur is drawn in the hair and cloth ramps the people already have (hue-shifted, with pallid twins): no new ramp was needed, and the master palette keeps its room for the terrain's |

Every part is a silhouette filled as one soft volume (`Canvas::inflate`, a chamfer distance field turned into normals and a dome of height), the far legs a px behind the near ones and a tone into the body's shade, then the painter's clusters: a lit topline, a belly or a bib, markings, a face (eyes two px with a glint in their top corner, a nose in `k` with its light, tan brows over the eyes), a flank's tufts. A pelt is retoned by how dark its key is (`fur_map`): a black dog is held low so it reads black with a sheen and never goes grey; white fur keeps its shade light.

Frames: the six-frame walk and a breathe per facing (decided: six, as the people's, over four: a trot reads with weight in six, and the frame table is one table), the idle pair `Idle, Idle2` facing the viewer, `Hurt`, `Atk1..3` for what fights (a unit drawn as the sprite that is not friendly and has a spell book), `Dead`. The gait is a trot, the diagonal pairs together (near fore with far hind), stand, contact, down, pass, contact, down, the tail swinging a beat behind; a hen's step bobs her head forward. The idle pair: a dog or a cat **sits** (the haunches wide, the forelegs straight, the chest up) and on the second beat tilts its head a px and sweeps its tail; a sheep grazes, a rabbit sits up, a hen pecks, a crow cocks its head. Dead is the plan's own pose: a dog, a sheep or a fox on its side, a small beast on its back, a bird feet up; pallid, never emitting, at most 4 px thick.

**Julie.** The dog is Julie (STORY.md §3): dry, patient, never cute, and the heart of the game, so she is drawn to be loved at a glance. A black-and-tan working dog with a white blaze and bib, tan brows over dark eyes with a glint (the brows are what let a dog's face speak), one ear up and one folded, a grey chin (she is old), a white tail tip that flags when she wags, and a faded red collar with a brass tag (she keeps the key). When she stands still she sits and looks at you, and tilts her head. `tests/creatures.rs::julie_is_herself` holds these.

### 2.3 Prop kit

One `Canvas`, now four layers at 16 px a cell; the TS build's three helpers become it. Primitives:

```
fill_rect  rect_lit  rect_bevel  rect_round  ellipse  ellipse_lit  soft_ellipse  disc_lit  hline  vline  line  arc  polyline_fill
gradient(rect, ramp, dir, dither)   folds(rect, ramp, period, phase)   strokes(rect, ramp, kind: fur | feather | grass | hair, density, seed)
grain(rect, material: wood | iron | stone, seed)   courses(rect, material, seed)   ao_contact(rect, spread)
stamp  outline  shadow_ellipse  mirror_x  rotate_ccw  crop  shorten_to  remap  checker
polygon_lit(pts, ramp, curve, z)   polygon_cloth(pts, ramp, curve, z)   set_clip(rect)   shade(rect, ramp, steps)   tint(x, y, ramp, tone)   retone(ramp, map)
unchecker(ramp)   declutter(ramp)   despike   outline_sel   upright(ay)   dome_heights(max)   scale_heights   quench   bounds
```

| Primitive | Albedo | Normal | Height |
| --- | --- | --- | --- |
| `rect_lit` | highlight top and left, shade bottom and right | flat, facing up | the rect's `rise` |
| `rect_bevel`, `rect_round` | a two-tone bevel of `b` px; `rect_round` rounds the corners | the four bevel slopes, then flat; quarter-spheres at a rounded corner | rise, falling across the bevel |
| `ellipse_lit`, `soft_ellipse`, `disc_lit` | sphere shading from the top-left with a rim, `soft_ellipse` blends its edge over 2 px with ordered dither | from the sphere table by octant | a dome |
| `gradient` | a ramp walked along a direction with a 4 x 4 Bayer dither between tones | unchanged | unchanged |
| `folds` | sinusoidal shade bands across the ramp, phase by frame for a swing | the band's slope, small | unchanged |
| `strokes` | short hashed strokes of the ramp's light and shade over a base | a little lift per stroke | +1 per stroke |
| `grain` | wood ring lines, iron pitting, stone speckle, by hash | grooves for wood and stone | unchanged |
| `courses` | brick, slate, thatch, plank and stone in courses with mortar | mortar grooves | the course's relief |
| `ao_contact` | index 1 darkening under a thing where it meets what it stands on | unchanged | unchanged |
| `polygon_lit` | the polygon in hard bands of the ramp, lit as an upright cylinder | across each row from `-curve` to `+curve` in `nx`; the top rows tilt up | `lo` on the bottom row to `hi` on the top |
| `upright` | unchanged | unchanged | each row's height above the feet added to the relief |
| `outline` | `k` where a drawn pixel meets clear, `K` at an interior seam | unchanged | unchanged |

`shadow_ellipse` survives only for flat props with no height worth casting; everything else gets its shadow from height in the light pass. `courses` is the brick, slate, thatch, plank and stone logic pulled out of `house()` so a chimney, a well and a wall lay the same course. Dither is for gradients and soft edges only, never as texture noise on skin or cloth.

```json
"look": { "family": "container", "shape": "barrel", "w": 1, "h": 1, "rise": 12,
  "materials": { "body": "wood_oak", "band": "iron" }, "states": ["base"], "vary": 3 }
```

| Family | Shapes | Notes |
| --- | --- | --- |
| `container` | chest, barrel, crate, sack, jar, trunk, coffin, tin, churn, bowl, box | `open` is the lid up; a barrel is a `gradient` round its staves with `grain` |
| `furniture` | table, chair, bench, bed, shelf, desk, pew, altar, counter, rack, cabinet, cubicle, plinth, case | |
| `sign` | post, board, hanging, fingerpost, notice, name_board, gravestone, headstone, milestone, standing_stone, memorial, chalk, timetable | `text_rows` drawn as illegible `K` strokes; the words are in the dialogue, never on the sprite |
| `lamp` | post, sconce, cage, globe, gas, bulb, candle, torch, brazier, great_torch, moonbeam, sunbeam | `mount` post, floor or wall (n, e, s, w). `on` turns glass to the lit ramp and writes it to the emissive layer; the light itself is the renderer's |
| `machine` | generator, breaker, fuse_box, relay_box, valve, pump, hoist, penstock, stove, socket, call_box, phone_box, ticket_window, bell, clock, air_pump, signal | `on` lights a lamp (emissive) or turns a dial |
| `barrier` | fence, gate, wall_crack, web_wall, shutter, rails, track, sleepers, drywall, planks, hedge_gap | Fences and rails autotile from neighbours at paint time |
| `vegetation` | apple_tree, rose, herb, flowers, flowerbed, stump, log, dry_tree, bud, root_wall, vine_root, hive, hedge_seed, dandelion, tangle | Trees that stand in the draw list (flora tiles are §2.6) |
| `debris` | rubble, bones, slag, rockfall, heap, pile, wreck_car, wreck_cart, candle_ends, hollow | |
| `small_thing` | an icon class at prop scale | The same `IconLook`, drawn at 16 x 16 with contact AO |
| `ritual` | plate, lever, orb, summon_stone, way_in, shrine, mouth, arch, altar_stone, dole_stone, manhole, ladder, hatch, stairs | `on` for a lever thrown, a plate pressed, an orb lit (emissive) |
| `structure` | well, trough, fountain, pillar, chimney, coop, tent, hut, shelter, steeple, cage, hutch, scarecrow, den, cart, stall, washing_line, trolley, anvil, bar | Anything too small to be a building and too big to be furniture |

**Rule:** every routine ends `ao_contact; outline`. Width is `w * 16` by construction, so a prop can never overhang its footprint; `rise` is its height in screen px and is what the height layer tops out at. `vary: n` renders `seed + 1` and `seed + 2` for `base2` and `base3`; the renderer picks by the prop's id hash, as in the TS build.

**As built (step 5, 2026-09-27).**

```json
"lamp_post": { "family": "lamp", "shape": "post", "rise": 40,
  "materials": { "body": "iron", "trim": "brass" }, "states": ["base", "on"], "emits": ["glass"] }
```

- **The footprint comes from the prop rows**, not the look: every row naming the sprite must agree (`kit::footprint`, an error otherwise). The canvas is `w * 16` wide and `h * 16 + rise` tall; `rise` is how far the drawing stands above the footprint's back edge. The anchor is `(0, h * 16 - ...)` as §1 says; the presenter packs a prop on its foot row, as the stand-ins were.
- **Fields:** `family`, `shape` (the family's shape by name; an unknown one is a render error and a red test), `rise`, `materials` (`body`, `trim`, `accent` ramps), `states` (`base` always; `on`, `open`), `vary` (1 to 3), `mount` (`floor`, `post`, `wall_n/e/s/w`: where a wall lamp hangs), `text_rows`, `emits` (`glass`).
- **Families built:** `sign` (post, hanging, fingerpost, name_board, notice, timetable, milestone, gravestone, headstone, standing_stone, memorial, chalk, note), `lamp` (post, gas, sconce, cage, miner, torch, brazier, great_torch, fire, signal; a cold flame takes the accent's ramp), `barrier` (door, gate and a gate edge-on, shutter, boards, drywall, crack, web, rails, sleepers, roots), `container` (chest, trunk, crate, barrel, sack, jar, churn, coffin, tin, bowl, bottles, mug, parcel), `ritual` (hatch, stairs, way_in, plate, lever, orb, manhole, ladder, summon_stone, shrine), `furniture` (table, workbench, desk, counter, altar, shelf, cabinet, case, bed, stove, bench, pew, chair, pegs, register, plinth), `structure` (well, trough, fountain, pillar, chimney, coop, woodpile, washing_line, cart, minecart, trolley, anvil, bale, pillar_box, phone_box, ticket_window, shelter, stall, pump, beehive, scarecrow, tent, hoist), and the first of `vegetation` and `debris` (apple_tree, dry_tree, stump, log, flowers, flowerbed, herb, rose, crop; boulder, rock, rubble, rockface, heap, bones). `machine` and `small_thing` are step 7's.
- **Parts** (`kit::parts`), so every family lays a material one way: `planks` (a lit leading edge, a seam in shade, two short grain streaks a board, a knot in one in four), `post`, `blocks` (coursed stone, joints staggered, a lit top edge, a chip), `band` (iron with rivets), `glass` (cool and dark with a streak of sky unlit; warm, emitting and brightest in its core lit), `flame`, `writing`, `box3` (a face and a top in the 3/4 view).
- **Heights:** a face stands up row by row from the foot as a person's rows do (`upright`), a top is flat at its face's height (`lid`), a thing lying on the ground is capped at a few px (`cap_heights`).
- **Decided: what glows is light and is not lined.** A lamp's glass, a flame, a lit window keep their colour through the clean-up and the outline (a candle's flame is five px and a line would eat it). Silk (a web) is the one thing drawn unlined and undespiked: a thread a px wide.
- **New canvas primitives** (additive): `inflate` (any mask as a soft volume), `dye`, `dye_ellipse`, `dye_poly` (a marking under the shading), `lid`, `cap_heights`, `heights_by` (a roof landing on its house), `face`, `fill_normal`, `remap_emitting`, `relight`, `has`.

### 2.4 Building painter

`house(HouseLook, lit)` as in the TS build, plus:

| Field | Values |
| --- | --- |
| `style` | cottage, barn, farmhouse, inn, shed, hut, station, steeple, coop, tent |
| `storeys` | 1, 2 |
| `dormers`, `porch`, `lean_to`, `boarded` | flags |
| `silhouette` | the mass against the sky only: the title scene and the far landmark (§2.8) |
| materials | `RampId`s for wall, roof, trim and door, so a region recolours a house without a new painter |

Walls, roofs and chimneys use `courses` at 2-px slate and brick, with `grain(stone)` on a footing; the roof's normal is its pitch, the wall's is flat with mortar grooves; height is the wall height and the roof rises from it. Windows are `glass` role pixels so `lit` swaps them to the lit ramp at night and writes them to the emissive layer, warm and a little uneven by a hashed offset per window.

**As built (step 6, 2026-09-27).** `Look::Building(HouseLook)` in `data/looks/buildings.json`:

```json
"cottage_thatch": { "family": "building", "style": "cottage", "roof": "thatch", "wall": "plaster",
  "rise": 4, "porch": true, "lit": true }
```

`style` (cottage, farmhouse, barn, inn, shed, hut, steeple), `storeys` (1 to 3), `roof` (thatch, slate, tile, tin, reed), `wall` (plaster, timber, stone, brick, board, reed), `dormers`, `porch`, `lean_to`, `boarded`, `silhouette`, `rise` (above the walls: a chimney's room), `door` and `trim` ramps, `lit`. The canvas is the footprint wide and the footprint plus the walls plus `rise` tall: the front wall (a storey 28 px, a barn's 36, a shed's 22) stands on the footprint's front edge and the roof covers the footprint over it, its front slope (three fifths) in its courses facing the sun, its back slope beyond in shade, the ridge capped, a brick chimney on it. Plaster weathers in long soft stains over a stone footing; timber framing is beams over it; thatch is laid in lipped courses with its reed in short streaks and a pegged ridge. Frames: `base` (windows dark, reflecting) and `on` for a lit building (windows warm, one in five dark: a room nobody is in). Heights: the wall upright from the foot; the roof `heights_by` from the eave's height to the ridge's, so it lands on the house under it.

**Decided: who owns a building.** A building that is a prop (the county's cottages, the farmhouse, the barn, the inn, a shed, the reed hut, the steeple: a footprint the sim collides) is a sprite drawn here, which the renderer y-sorts, so she walks behind its roof and in front of its wall. A building made of tiles (`HouseWall`, `HouseRoof`: the town's terraces, Julie's house) is the terrain painter's (§2.6). They share materials by ramp and one language of courses, so a cottage and a tiled house stand in one street without a seam in style; when the terrain's `courses` lands in the canvas, the painter moves onto it.

### 2.5 Icon

```json
"icon": { "class": "flask", "ramp": "liquid_red", "overlay": "cork" }
```

Classes: flask and vial, key, bar, orb, stone and gem, herb, food (round, loaf, cut, pot), tool, garment, paper, misc, spell (a school disc with a mark), status (a ring with a mark). Every icon: drawn once at 32 x 32 with a dark slot, a 1-px `k` outline, a soft light from the top-left and at most 24 indices; the 16 x 16 chip is a second render at the small size, never a downscale. A glass or orb icon emits so the bar glows a little at night.

**As built (step 6, 2026-09-27).** `Look::Icon(IconLook)` in `data/looks/icons.json`, all 91 icons the items, spells and effects name: `class` (44 of them, from flask to tortoise, spell and status), `ramp`, `trim` (a flask's glass, a key's ring, a status's rim; a default by class), `mark` (flame, frost, leaf, bolt, burst, fist, hammer, web, skull, shield, drop, star, thorn, heart), `glow`. `jane_art::icon` lays every shape out on a 32-unit grid and places it at the size it draws, so the 16 chip is its own render; a line is two px at the least (a one-px diagonal is a row of spikes). Rendered as two sets: variant 0 the icon at 32, variant 1 the chip at 16. Two changes from the plan above: **the slot is the chrome's** (`chrome::slot` draws it under the icon; an icon carries only its object), and **the outline is selective** like everything else (decided 2026-09-27), not all `k`. `jane sheet icons` draws them on the slot; `tests/icons.rs` holds coverage, the two sizes, the 24-colour budget and glow. The UI packs and draws them (not yet wired).

### 2.6 Terrain and flora

The TS painter carries as an algorithm at 16 px a cell: chamfered material edges, autotile, roofs, cliffs, fences, the water shimmer list and the canopy strips. Its seven tables become **one `TileStyle` row per tile** in `data/looks/tiles.json`:

| Field | Meaning |
| --- | --- |
| `group` | ground, water, wall, roof, flora, made; what it autotiles against |
| `inherit` | the tile whose ground shows under a chamfered edge |
| `height` | 0 flat, 1 raised (cliff and wall faces are drawn), 2 canopy (a strip above the y-sort); with `rise` in px for the height layer |
| `swatch` | the material ramp and the per-cell hash pattern (`courses` for made things, `strokes(grass)` and pebble scatter for ground, `grain(plank)` for boards) |
| `detail` | the sub-cell table: blade density, pebble count, slate course pitch, furrow spacing, all at 2 px |
| `normal` | flat, cliff (the face's slope), wall, roof (pitch by facing), water (flat; the renderer ripples it) |
| `wet` | how the tile takes rain: 0 grass and thatch stay matt, 1 earth darkens, 2 stone and plank darken and reflect the sky |
| `wall_like`, `raised` | draws a face below its top and blocks the chamfer; its top is drawn one course up, so what stands behind it is hidden |
| `map_colour` | the ink for the map chart (PRESENTATION, the map tab) |
| `ink` | the swatch's dark line for fences, rails and furrows |

**Render-only tiles leave the sim** (PORT §6.i). `RoofSlate`, `RoofThatch`, `BrickWall` and `Pine` are not sim tiles: a variant comes from (1) the biome and cell hash, read from the skeleton the blueprint carries, and (2) `Blueprint.paint: Vec<(Rect, Material)>`, written by the chunk stamper where an authored place wants a particular roof or wall. Kept as sim tiles with a reason: Glass, StoneWall, DeadTree, Hedge, Boardwalk, Stepping, Crops, FlowerBed.

`paint_chunk(grid, cx, cy, &mut ChunkLayers, &mut Strips)` paints 16 x 16 cells into the renderer's chunk at 256 x 256 px (PRESENTATION, terrain chunks): `ChunkLayers { albedo: [u32; 256 * 256], normal: [[u8; 2]; 256 * 256], height: [u8; 256 * 256], wet: [u8; 16 * 16] }`. It is the one place indices meet RGB outside the palette module: a chunk is blitted whole and never tinted, so the albedo is resolved through the table as it is painted; the other layers stay raw for the light pass. Water cells keep a caustic phase per cell so the shimmer of §2.8 lines up across a chunk seam.

Flora: the 43 generators port onto `Canvas` at the new scale, their tone tables become ramps, leaves become `strokes(grass)` over a `soft_ellipse` crown so a tree is a lit volume rather than a flat blob, and a canopy tree emits its strip (88 rows with 32-px margins) for the ghost pass.

### 2.7 Materials

A ramp is six to eight tones in luminance order, `deep, shade, base, light, high, glint` and up to two half-steps for dither pairs; a generator asks by role and never by index. The people's ramps (skin, hair, cloth, leather, the pool: `hue::shadow_hue` lists them) are hue-shifted: each tone's lightness steps from the key, its hue turns toward a cool one in shadow (violet-blue; rose for skin, never grey-brown) and a warm one in light (yellow; peach for skin), and its saturation peaks in the midtones and falls at both ends; a grey takes a cool tint in shadow and a warm one in light. A palette test holds each of them to that. The kit's and the creatures' own materials (wood_dark, wood_pale, iron, brass, copper, glass, bone) shift the same way from step 5; the terrain's (stone, slate, brick, plaster, oak, bark, the greens) are its painter's to shift. Steps 4 to 6 added **no ramp** (a creature's fur is drawn in the hair and cloth ramps, which already shift and have pallid twins), so the master palette keeps its room below 1024 for the terrain's. Index 1, the contact shadow, is a cool multiply (`palette::AO_TINT`, blue held up more than red) and the blit softens its crisp mask by how much of each pixel's 3 x 3 it covers (`palette::ao`), so a shadow has a soft edge and no dither. Groups:

| Group | Count | Notes |
| --- | --- | --- |
| skin | 7 | skin, pale, dark, bone, wax, stone, metal; never dithered |
| hair | 6 | |
| cloth | 15 | the four seat coats first |
| wood | 4 | |
| stone | 4 | |
| metal | 5 | |
| roof | 3 | slate, thatch, tin |
| wall | 4 | plaster, brick, stone, board |
| glass and liquids | | glass and glass_lit; the flask liquids; the lit ramps are also the emissive colours |
| flora | | leaf, bark, bloom, reed, crop, by season where the county has one |
| water and sky | | the hour ramps of §2.8 |
| school | 7 x 3 + core | one triple per school (`SCHOOL_COLOUR`) |
| ui | | panel, bevel, track, ink |

About 110 ramps, deduplicated at build. **The master palette is at most 1024 entries** (a test), laid out as ramps so a remap walks a row, with the ~25 base colours of the TS build's `art/types.ts` kept among them so the county reads as the same place. The old cap of 240 is gone; the per-sprite budgets of §3 are what keep one sprite from using the whole table.

### 2.8 Atmosphere and parallax

Atmosphere is art too. Every wisp, drop and shadow is a generator in `jane-art::weather`, driven by the weather state `WORLD.md` keeps (clear, mist, rain, storm) and by area; the runtime that moves them is PRESENTATION's weather pass.

| Piece | Generated how |
| --- | --- |
| Mist tiles and fog volumes | a 512 x 512 alpha mask from 40 radial `soft_ellipse` blobs, seamless at its edges (a test); three densities by area; drifts as before; a per-area alpha ramp by height, so a hollow holds fog and a hill stands clear |
| God rays | a mask of soft parallel bands from the sun's hour angle, drawn between canopy and ground when the ambient is high and the area is wooded |
| Rain, snow | drops as 1 x 6 to 1 x 12 strokes in two greys, a splash ring of three frames, a puddle disc that reads the tile's `wet`; storm doubles the density and adds a wind lean. Snow is flakes of two sizes and a settle disc, in the Works' winter only |
| Dust motes, fireflies | 2 x 2 soft discs; fireflies emit and fade on a 64-entry table by `(tick + h32(id))` |
| Chimney smoke | five soft puffs on a rising path, each fainter, from a chimney prop's top |
| Water caustics and shimmer | a 64 x 64 seamless caustic mask scrolled two ways over water cells at the chunk's phase; a 1-px highlight per water cell as before |
| Cloud shadows | a 1024 x 1024 seamless mask of soft blobs scrolled over the ground under a sky that has cloud, multiplying the light buffer |

Parallax layers, `ParallaxLook`, drawn behind the ground where the view meets the zone's edge or the sky:

| Layer | Generated how |
| --- | --- |
| Sky | a `gradient` of the hour ramp (twelve keyframes, dithered), 120 hashed stars with a one-in-eight twinkle by tick, the moon as `disc_lit` in one of eight phases by day |
| Far treeline | two silhouettes in `K` and the region's `deep`, hashed crowns, scrolled at a quarter and a half |
| The School | `house(silhouette, storeys 3, steeple)` at the distance band's size, its windows lit and emitting at night, one flickering; north of the view outdoors, never indoors |
| Cloud layer | soft blobs on the sky, scrolled slowly, the same mask the cloud shadows read |

Tests: every weather kind and every area has an entry that draws pixels; every seamless mask matches its own opposite edges; a parallax layer for every hour keyframe renders and is not blank.

## 3. Style rules: one hand

| Rule | Enforced by |
| --- | --- |
| One palette table; everything else is indices | Crate design |
| Outline: **selective** (decided 2026-09-27). Every drawn pixel that meets clear is an outline pixel (1 screen px, half a sim unit) in the darkest tone of the adjacent material's own ramp, darkest below and away from the light, broken or lightened on lit top-left edges; `k` only where the ground needs the contrast; interior seams are the material's dark tone, `K` only between unlike materials | `outline()` runs last; test "outline closed" |
| Light from the top-left in the albedo as a base; dynamic light on top through the normals, so a sprite reads right both with the light pass off (day) and on (night, a lamp) | the lit primitives are the only fills for bodies; test "lit sphere" |
| Contact: AO baked in albedo as index 1 under a thing, plus the cast shadow the light pass draws from height | Index 1 reserved; height on every opaque pixel |
| Colour budget: person ≤ 48, creature ≤ 40, prop ≤ 64, icon ≤ 24, building ≤ 96, `k` and `K` excluded | Test |
| Dither only for gradients and soft edges; never as texture noise on skin or cloth | A test finds no 2 x 2 checker of two skin-ramp tones in any person frame |
| Silhouette at 32 x 40: head 16 wide, a 2-px neck gap, feet 4 wide; no detail under 2 px except eyes, studs and glints | Composer geometry; test |
| Contrast: a hostile body's mean luminance differs from the floor of every zone that spawns it by at least 28 of 255 | Test per unit row |
| No pure black, no pure white | Palette |
| Region palettes: the Lowfields are warm greens and plaster; the Waters blue-greens, reed and slate; the Works slag greys and brick | `RegionStyle { ground, wall, roof, lamp, sky, mist }` rows |

### 3.1 Craft: the bar (2026-09-27)

The owner's bar: the art stands out as impressive and beautiful among the best human-made pixel art games (Sea of Stars, Eastward, CrossCode, Hyper Light Drifter), not merely competent. Every generator is held to these, by test where a test can tell and by the art-director review below where it cannot.

| Rule | Held by |
| --- | --- |
| **Ramps shift hue.** Shadows step cool (blue, violet), highlights step warm (yellow, peach); saturation peaks in the midtones and falls at both ends. Skin shadows go rose or violet, never grey-brown; grass runs deep blue-green to sunlit yellow-green, never one hue | Palette test on each ramp's hue direction and saturation arc |
| **Clusters, not noise.** Texture is deliberate clusters of 2 px or more (tufts, pebbles, grain runs, courses) with a lit edge and a shadow edge; calm areas are left calm. No per-pixel speckle on any surface | Orphan-pixel count per sprite and tile (eyes, studs and glints excepted) |
| **No pillow shading, no banding.** Light comes from a direction; tone steps never run parallel to an outline | Review; a banding check where one proves reliable |
| **Pixel-perfect lines.** No doubled corners on an outline; slopes keep to 1:1, 2:1, 1:2 and their steady multiples | Jaggy check on outlines |
| **The grid never shows.** Ground carries low-frequency colour variation over many cells, organic transitions with a 1 to 2 px rim, and hashed clustered decals; at no zoom can a cell boundary be seen | Review on `jane sheet county --full` |
| **Shadows are coloured.** Contact AO and cast shadows are a cool tinted darkening of what lies under them, never grey, black or a checker | Review; the AO index's ramp |
| **Depth everywhere.** AO at the base of walls, fences, props and trunks and in inside corners; lit top edges on walls and cliffs; water banded by depth with a lit shore | Review; the height layer |
| **Night is beautiful, not dark.** Unlit ground shifts to deep blue-violet and keeps readable value; lamp pools are warm with a falloff that reads as light | Review on the night sheets |
| **Appeal at 32 x 40.** Readable faces with a glint in the eye, hair with a highlight band and strand clusters, silhouettes that say who someone is before colour does; townsfolk are different people, not recolours | Silhouette-distinct test; review |
| **Motion with weight.** Anticipation, bob and lean in the walk; secondary motion (hair, hem, scarf) a frame behind the body | Review on `jane sheet unit` |

**Decided 2026-09-27: whichever reads better wins.** The owner's rule for every look choice from here: the visually better option is the default, not the cheaper or the older one. Applied first to the outline (selective, above, over the all-`k` outline) and the walk (six frames, over four).

**Shadows are a showpiece**, not a necessity. Every opaque pixel's height is true to its shape (a head at 40, shoulders and a hem thinner, a wall its wall height, a canopy its crown), because the lit tiers cast from it: long soft sun shadows at dusk that swing with the clock, lamp shadows that fan out from every post, penumbrae that widen with distance, all tinted by the ambient (blue by day, violet at dusk) and never grey. Contact AO is a soft cool ellipse, never a checker.

**The art-director review.** Every generator iteration ends in its sheets viewed at 1x and at 3 to 4x, a written critique (what reads, what is muddy, where the grid shows, what looks amateur), and the worst item fixed next. A family is done when every sprite in it would be defended beside the references at the same scale; the critique of its final sheets goes in the commit or the report that lands it.

**Seat colours.** `jane@1`, `jane@2`, `jane@3` (teal, moss and ochre: `person::SEAT_COATS`, the TS build's `SEAT_COATS`) are swaps of the coat role and nothing else; a sprite gets them when a unit row a player controls names it (decided: seats are coat-only, hair and skin stay). Any sprite swaps by role the same way, which replaces the TS build's ~50 swap sprites. A swap touches the albedo only; normals, emissive and height are shared. Each swap gets its own atlas entry and its own corpse.

**Per-instance variation** (decided: in, kept). A person or creature row may carry bounded variants:

```json
"vary": { "hair": ["long", "bun", "short"], "coat_ramp": ["cloth_plum", "cloth_grey"] }
```

The product of the lists is capped at four variants per row (a build error above). Every variant is materialised at boot into the atlas; the renderer picks `h32(unit.id, 0, VARY) % n` and carries it as `UnitView.variant`. The sim never sees a variant, so a variant can never move a roll. Seats do not vary: the heroine is one person on every seat, and her coat says which.

## 4. Derivation rules

Every frame after `Down` is derived, so a look is written once. The composer keeps a **frame table** per cycle: for each frame, a body bob in px, a near and far arm swing, a near and far leg swing, a lean, and a `folds` phase. Every part reads the table, so a held thing, a pack and a scarf all move with the body.

| Frame | Rule |
| --- | --- |
| `up` | The skull's skin and face become hair (or the hat's back); eyes go; front and buttons become coat; the pack is drawn |
| `side` (east) | Columns left of the skull centre become hair; one eye; one nose pixel outside the skull; the torso is 12 wide; the near arm over the coat, the far arm behind. West is mirrored at draw time, and a held item declares its `hand` so the mirror puts it right; a mirrored normal has its `nx` flipped by the blit |
| walk `_1 .. _5` with the standing frame first | six frames a cycle (decided 2026-09-27: six read with weight where four read stiff), bob in px down the screen: stand (both feet down: a pass), `_1` contact (near foot 3 px out ahead, far 3 behind, leaning in a px), `_2` down (the weight onto it, bob +1, the far heel up), `_3` pass (feet together, the far foot lifted 2), `_4` contact and `_5` down on the other foot. Seen from the front a contact is the forward foot a pixel lower, and the down lifts the back foot. Arms swing against the legs. What hangs loose (hair ends, a hem, a scarf's tail, a skirt) reads the previous frame's bob and lean, so it lags a frame behind the body; the fold lines sway with the phase, a sixth of a turn a frame |
| breathe `_b` | the chest and shoulders 1 px up, the head 1 px up, the hem where it hung, the folds a half turn on; the renderer alternates it with the standing frame every 40 ticks |
| `atk_1..3` | wind-up (lean back 2, held thing raised), strike (lean forward 3, arm extended, the held thing at full reach), recover (lean 1); per facing |
| `cast_1..3` | hands together, hands out with a school-coloured emissive glow between them, hands down; per facing |
| `hurt` | the standing frame leant back 2 with the head down 1 and the eyes closed; the renderer flashes it |
| `on`, `open` | The family rule redraws: lid up, glass lit and emitting, lever thrown |
| `base2`, `base3` | `seed + 1`, `seed + 2` |

Creatures read the same table shape with their plan's own cycle: a trot moves diagonal pairs, a wing beat replaces the leg swing with a wing angle, a slither is an offset per segment. A row emits only the cycles its family and its rows promise: a villager who never fights has no attack and no cast, and the atlas is the smaller for it.

### 4.1 Dead frames

| Style | Who | How |
| --- | --- | --- |
| `fallen` | every Person | posed first, seen from above on her back (`person::FALLEN`: one arm thrown out, the other bent at her side, a knee drawn up, the coat's hem spread), the `down` frame of that pose rotated a quarter turn anticlockwise, repeated columns removed to 28 wide at most, dropped to `ay - 2`, pallor applied, lying in a pool at 38 % from the head under its middle; `Dead2` throws the other arm and draws the other knee, picked by unit id (a child's and a stout body's limbs are thrown half as far, so they lie no taller than they stood). Height becomes the body's thickness, a dome over the lying shape at most 5 px (`dome_heights`), so its cast shadow is a sliver |
| `topple`, `legs_up` | plants; small quadrupeds and birds | on the stem; on the back |
| `scrap` | machines and armour | 8 x 8 plates in two courses |
| `melt`, `wisp` | waxwork; shade | |
| `pose` | every other creature | the plan's own `Dead` |

Six muted pool pairs (step 2 has the one people need, `pool`). `pallor` darkens by 14 % and greys by 20 %; the pallid twins exist once in the palette, for the ramps a person is made of (`palette::PALLID`: skin, hair, cloth, leather), and any other ramp goes a tone darker; a dead frame never emits. Tests: a dead frame is not a living frame, is shorter, is not floating (its lowest pixel is at or below `ay - 2`), and is more than 16 px across.

## 5. Pipeline

**Boot, once, into atlases.** Every look, every variant and every swap is rendered at boot and packed, all four layers. Nothing renders from a look during play; a frame blits from finished pages.

**Budget.** The Pi 3 must hold 60 fps (owner decision; the degrade order is in PRESENTATION's performance section) and the pages are read-only and packed tight for it; at the new size and depth they live in its RAM and not its L2, and that is a cost PRESENTATION carries in its blit budget. The boot cost is paid once behind the loading screen. Pi 3 target: every look cold in under 3 s, measured by `jane bench art`. The **page cache under the save directory**, keyed by the build hash (content hash plus the `jane-art` version), is allowed and expected on a Pi: written by the game, never shipped, regenerated silently when stale, never the truth (`jane sheet` and the tests always generate), and a warm boot from it is a copy. `vary` is budgeted with the rest: a row with variants costs up to four times its frames, and a test sums looks x variants x frames x layers against the page limit.

**Atlas.** 2048 x 2048 shelf-packed page sets, one set holding four layers of one rect map: albedo 16-bit (an `Ix`: the master palette is up to 1024 entries, §2.7, so the pages are not 8-bit; PRESENTATION §1.4), normal 16-bit, emissive 16-bit, height 8-bit, seven bytes a pixel. `soft` holds the albedo pages alone, two bytes a pixel.

| Content | Frames | Pixels | Size, four layers |
| --- | ---: | ---: | ---: |
| units, with seats, variants and every cycle a row promises | ~5000 | ~6.4 M | ~45 MB |
| props | ~400 | ~0.6 M | ~4.2 MB |
| buildings | ~40 | ~0.5 M | ~3.5 MB |
| icons, 32 and 16 | ~180 | ~0.12 M | ~0.8 MB |
| flora and canopy strips | ~90 | ~0.4 M | ~2.8 MB |
| font, four faces, albedo only | ~400 glyphs | ~0.25 M | ~0.5 MB |
| chrome, albedo only | | ~0.3 M | ~0.6 MB |
| weather masks and parallax | ~120 | ~1.0 M | ~7 MB |
| total | | ~9.6 M | **~64 MB: three page sets, four at most; ~19 MB of albedo on `soft`** |

`Atlas::get(SpriteId) -> &SpriteRef { page, frames: [FrameRect; N], w, h, ax, ay, flat: bool }`; ids are interned `u16` at build; a flat sprite has albedo only. `jane-art` packs the pages; `jane-present` holds and blits them (PRESENTATION, the renderer's formats).

**Looks table.** `data/looks/*.json`, one file per family, merged in path order like every table (PORT §5.2). The key is the sprite id every row already names in `sprite` or `icon`; the value is a `Look`. **Rule:** the build (`jane-schema`) fails on a `sprite` or `icon` that names no look, and on a look no row uses. There is no default look and no pink square.

**Dev tool.** `jane sheet` in `jane-cli`:

| Command | Draws |
| --- | --- |
| `jane sheet units`, `props`, `icons`, `flora`, `chrome`, `font` | a family, every row, 1x and 4x on its real background, day-lit |
| `jane sheet creatures`, `props [--only a,b]`, `buildings` | as built: the creatures standing, from behind, in profile, sitting and dead; the kit and the buildings in every frame, what glows shown at night |
| `jane sheet unit <id>` | all frames of every cycle, every seat, every variant, the corpse, on a 4x grid, each set with a 1x strip and its west frames mirrored; `--seat N` and `--scale N` narrow and zoom it |
| `jane sheet light <id>` | the sprite lit from eight directions and from above, with the cast shadow, through the same integer light pass the renderer uses |
| `jane sheet layers <id>` | albedo, normal (as a colour ramp), emissive and height side by side at 4x; for a look, `--frame side_1` picks the frame (`down` by default), as for `light` |
| `jane sheet person --grid`, `palette` | every build by every hair and coat; the table and the ramps in luminance order |
| `jane sheet terrain` | every tile in every autotile context, dry and wet |
| `jane sheet weather <kind>` | the kind's masks and sprites, and sixty ticks of its motion over a plain ground |
| `jane sheet parallax` | every hour keyframe's sky, treeline, School and cloud, in a strip |
| `jane sheet title` | the title scene |
| `jane sheet county <seed>`, `dungeon <id> <seed>` | the real chunk painter over a built zone (the rest of these are PRESENTATION's viewer section) |
| `jane sheet --bless` | rewrites `tests/golden.txt` |

PNG through a 60-line encoder in `jane-art::sheet` (stored deflate, crc32, adler32). No image crate. The sheets are CI artefacts on every run and are never asserted; the hashes are.

**Acceptance tests.** All in `jane-art`, all headless, all run on every target.

| Test | Holds |
| --- | --- |
| coverage | every `sprite` and `icon` in the catalog has a look; every look renders every frame its family and cycles promise |
| geometry | every frame is inside its box; anchors are where §1 says; a person is 32 x 40 with feet at (16, 36); a prop is `w * 16` wide and no taller than `rise` plus its footprint; an icon is 32 and 16 |
| determinism and goldens | `render` twice gives equal bytes in all four layers; every sprite's FNV hash equals `tests/golden.txt`; the hashes are equal across the CI targets |
| layers | every sprite has all four layers at one size; every normal decodes to unit length within 2 of 255; emissive is non-zero only on declared roles; height ≥ 1 on every opaque pixel and 0 on clear |
| lit sphere | a generated `soft_ellipse` lit from eight directions through the integer light pass: the brightest quarter of its pixels lies in the light's half of the disc for every direction, and the eight results are pairwise distinct |
| outline closed | every drawn pixel meeting clear is a line: `k`, or (sel-out) its material's own dark: `deep` where it faces away from the light (below, right), no lighter than `base` on the lit side; no coloured pixel touches index 0 |
| clean clusters | in a person frame at most 5 orphans (a pixel of the sprite's own materials that no neighbour shares, inside the line; eyes, glints, buttons and buckles are studs and do not count); no pixel with clear on three sides (the profile's nose is two px so it is not one); pillow shading fails: across every row, a run of one material at least five px long with the line at both ends is no darker one px in on the left than on the right, in at least three runs of four |
| true heights | a standing frame's every pixel stands its row's height above the feet (`(ay - y) * 5 / 4`, at least 1): the head 40, a hat to 46; a lying one is at most 5, its thickness |
| colour budget | the §3 caps per family, `k` and `K` aside, over a sprite's living frames and again over its dead frames (which are in pallid twins); no 2 x 2 checker of two skin tones in any person frame |
| silhouette distinct | any two sprites of a family, standing: the XOR of their masks is at least 6 % of the union, or (one cut in other cloth: a swap, a variant, two neighbours dressed alike) at least 6 % of the union is recoloured and the recoloured pixels differ by 24 of 255 a channel on average |
| contrast | §3, per hostile unit row against every spawning zone's floor swatch |
| dead frames | §4.1's four rules for every unit row |
| no grids | §1's two greps over the crate |
| palette | at most 1024 entries; index 0 is clear and 1 is AO; every ramp is six to eight tones in luminance order; no pure black or white |
| effects, weather, parallax | every spell and effect row has an fx entry, and every entry draws pixels; the emissive parts of a cast are in the school's triple; §2.8's tests |
| vary budget | no row above four variants; the packed total fits four page sets |
| font | §6's glyph tests |

## 6. The font

**Decided: stroke-defined glyphs, rasterised at boot.** No bitmap font in source (it would be a grid) and no system font (the TS build's `fillText` is why text differed between machines).

A glyph is a list of segments with integer endpoints on a **5 x 8 lattice**: cap height 6 (rows 0 to 5), baseline on row 5, descenders on rows 6 and 7. A Bresenham pen of the face's width draws it, and at 2x and above the pen's corners are rounded by one pixel so a stroke reads as ink and not as a staircase.

```rust
'A' => &[Seg(0,5,0,1), Seg(0,1,1,0), Seg(1,0,3,0), Seg(3,0,4,1), Seg(4,1,4,5), Seg(0,3,4,3)]
```

| Face | Lattice | Pen | Cell | Where |
| --- | --- | --- | --- | --- |
| Fine | 1x | 1 px | 8 x 12 | dense UI: the terminal, lists, tooltips, map labels: 96 columns by 36 lines at 768 x 432 |
| Small | 2x | 2 px | 12 x 18 | body text, HUD, dialogue: 64 columns by 24 lines at 768 x 432, the same character grid the design was laid out on |
| Head | 3x | 3 px | 18 x 27 | headings, crits, the zone banner |
| Title | 5x | 4 px | 30 x 45 | the title |

Bold adds one to the pen; shadow is a second pass at (+1, +1) in `k` (at (+2, +2) for Head and Title); outline is a 4-way dilate in `k`. Glyphs: ASCII 32 to 126, `£`, four arrows, a bullet and a degree sign, about a hundred in all, four faces of them. `Text` has an advance per face, `measure`, greedy `wrap` on spaces, and `draw(x, y, str, Style { face, ink, shadow, outline })`.

Tests: glyphs are pairwise distinct (XOR at least 3 px on the 1x lattice); every stroke is inside the box; capitals touch rows 0 and 5; descenders alone reach rows 6 and 7; `l I 1 | ! i` are distinct, `O` and `0` are distinct (the zero is slashed), and `S 5 Z 2 B 8` are distinct; the Small face at 2x is the Fine face's strokes and nothing else.

## 7. Title, chrome, cursor, map inks

**UI at 1x** (decided): chrome is generated on the view grid (768 x 432, wider on wide screens; PRESENTATION's UI section), never on a larger grid scaled down. Chrome is flat (albedo only): the light pass does not touch it.

| Piece | Generated how |
| --- | --- |
| Title | the parallax sky of §2.8 at its night keyframe; the hill as `polyline_fill` in `K` with a `strokes(grass)` crest; the far treeline; the School as `house(silhouette, storeys 3, steeple)` at the near band with its windows emitting and one flickering; a mist tile at 40 % and a few fireflies; the title in the Title face with a `k` outline and a 2-px shadow |
| Panel, button | 85 % `panel` fill, a 1-px `k` outline, cut corners, a `rect_bevel` of 2 px in `W` top and left and `G` bottom and right; generated at size and cached by (w, h, style). A button is a panel with a pressed state one pixel down and its bevel inverted |
| Bar | track in `G`, fill as a `gradient` of a ramp, 25 % ticks, a damage lag tail |
| Slot and chip | 36 or 20 px; a 36 x 36 angle table for the conic cooldown; the GCD sweep in `W` at 30 % |
| Cursor, pad glyphs | reticle, arrow, hover brackets, grab hand; the standard mapping's buttons and sticks; all 16 x 16 |
| Map inks | `MapInk` rows in the palette: ground, water, road, rail, roof, seen, unseen smoke (PRESENTATION, the map tab) |
| Floating numbers | Small face in the school colour with an outline; crits in the Head face |

## 8. Migration order

Each step ends with something on screen or on a sheet, and with its acceptance tests green.

1. `palette`, `canvas` at 16 px a cell with the four-layer pipeline and its primitives, `hash`, `font` (Fine and Small at least), `chrome`; `jane sheet layers` and `jane sheet light`. The title and the menus draw; a lit sphere sheet is the first artefact.
2. Person `slim`, the frame tables, the six-frame walk and the breathe, the derivations, `fallen`, the seat swaps. Looks for Jane and the townsfolk.
3. Terrain and flora ported; `TileStyle` rows with height, normal and wetness. The county draws, and draws lit at night.
4. Creature `quadruped_mid`, `quadruped_small`, `bird` with their gaits; Person `bone` skin. The first five minutes have their cast. **Built** (2026-09-27): twelve animals and the skeleton; the creatures' attack cycle with them.
5. The kit in first-walk order: `sign`, `lamp` (emissive), `barrier`, `container`, `ritual`, `furniture`, `structure`. The house, the cellar and the mine dress. **Built**, with the first of vegetation and debris; `machine` and `small_thing` wait for step 7, and 75 prop sprites (the museum's exhibits, the factory's boards and sockets, the tales' small things, the forest's banks) still draw a stand-in.
6. Building extensions with emitting windows, the icon classes at 32 and 16, the fx tables with their emissive parts, the attack and cast cycles. **The buildings and the icons are built**; the fx tables and the people's attack and cast cycles are not, and the UI does not yet draw the icons.
7. The remaining plans, held things, the rest of the kit, Head and Title faces. Every dungeon dresses.
8. The title scene, `vary`, the region ramps.
9. `weather`: mist, fog, rain, storm, snow, motes, fireflies, smoke, caustics, cloud shadow, god rays; the parallax layers; `jane sheet weather` and `parallax`. The county breathes.

**Rule:** a row without a look is a build error (`jane-schema`) and a red test, never a pink square. A look that renders wrong is a redline on a sheet, and the sheet is what the owner reviews at the P5 gate (PORT §7).

## 9. Defaults taken

Recorded as defaults; the owner may flip any with a one-line edit before P5 starts. The first five are the 2026-09-26 direction and are decisions, not defaults.

| Default | Where it lands |
| --- | --- |
| 16 screen px per cell; the sim cell stays 8 units | §0, §1 |
| Internal canvas 768 x 432, integer-scaled, wider on wide screens; UI generated at 1x on that grid, flat | §0, §7, PRESENTATION's window rule |
| Four layers per sprite and tile, emitted by the primitives; a master palette of at most 1024 as ramps; budgets person 48, creature 40, prop 64, building 96, icon 24 | §1.1, §2.7, §3 |
| Six-frame walks (decided 2026-09-27), breathe, three-frame attack and cast, hurt, two dead poses; only the cycles a row promises | §4, §5 |
| Weather and parallax are generators in this crate, driven by `WORLD.md`'s weather state | §2.8 |
| Font is strokes on a 5 x 8 lattice at two working faces (Fine 1x, Small 2x) with Head and Title from the same strokes; no bitmap in source | §6 |
| Seats are coat-only swaps, albedo only | §3 |
| Per-instance `vary` is in, at most four per row, picked by unit id, materialised at boot | §3, §5 |
| The atlas disk cache is allowed and expected on a Pi: under the save directory, never shipped, never the truth | §5 |
| Procedural audio is outlined only (PRESENTATION's audio section), for PLAN M8 | not this crate |
