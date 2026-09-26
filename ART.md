# Jane, Art

How every pixel in the native build is made. Pair with `PRESENTATION.md` (what draws it, lights it and lays out the UI), `ARCHITECTURE.md` (the engine, and the `View` presentation reads, its §11), `PORT.md` (the plan: P5 is the art phase, §6.i is where render-only tiles leave the sim) and `PLAN.md` §6 (the art gaps the web build left).

**Status:** §8 steps 1 and 2 are built: the palette, canvas, font, chrome and light pass, and the Person composer with its looks (`data/looks/persons.json`: Jane and her seats, the town, the country folk, the villagers). The rest is design. The TS build's generators (`jane/src/art/`) are the reference for method until P5 lands and `jane/` is archived at P10; they are not the reference for the look, which moved up on 2026-09-26 (§0).

Crate `jane-art`: depends on `jane-core` and `jane-data` only. No SDL. **No floats**: it sits in the lint table of the deterministic crates (PORT §3.4), so a sheet hashes the same on every target. Sphere shading, bevel normals and gradients come from integer tables. No allocation after boot beyond the atlas pages. Runs headless in tests.

**Rule: art is a function.** `render(look, seed) -> SpriteSet`. Nothing is drawn by hand; everything is described in a row and generated. The TS build did this for houses, trees, terrain, corpses, town props and icons (about 160 of its 410 sprites) and that is the part of it that holds together best. The other 250 are hand grids (`unit-base.ts` alone is 2.4k lines holding sixteen drawn sprites), with three separate people composers, three pixel-canvas helpers and about seven hash functions beside them. Here generation is the only way, and there is one of each helper. Generation also carries the whole of the new look: a generator that knows it drew a sphere can emit that sphere's normals, its height and its contact shadow for free, and a hand-drawn sprite could not.

**Rule:** presentation never touches the sim. A sprite's seed is a stable id (a prop's row id, a unit's id, a cell), never the sim's RNG, so what she sees is the same on every machine, every load and every seat.

```
jane-art/
  palette   the one RGB table, ramps of six to eight tones, region styles, map inks
  canvas    the one four-layer pixel canvas at 16 px a cell, and every drawing primitive
  hash      h32 salts over jane-core's FNV-1a and mix32
  font      stroke glyphs rasterised at boot; Fine, Small, Head and Title faces
  person    the people composer and its frame tables
  creature  body plans and their gaits
  kit       the prop kit, twelve families
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
| Layers per sprite and tile | four: albedo, normal, emissive, height (§1.1); walks are four frames (§4) | |

Still **zero hand-drawn sprites** (§1). The generators get richer primitives instead (§2.3), and every one of them knows the shape it drew well enough to light it.

---

## 1. The contract

```rust
pub fn render(look: &Look, seed: u32) -> SpriteSet

pub struct SpriteSet { w: u8, h: u8, ax: u8, ay: u8, frames: FrameMap, roles: RoleMap, emits: RoleSet }
pub struct Frame { albedo: Vec<u8>, normal: Vec<[u8; 2]>, emissive: Vec<u8>, height: Vec<u8> }   // all w * h
pub enum FrameId { Down, Down1, Down2, Down3, DownB, Up, Up1, Up2, Up3, UpB, Side, Side1, Side2, Side3, SideB,
                   Atk1, Atk2, Atk3, Cast1, Cast2, Cast3, Hurt, Dead, Dead2, Base, Base2, Base3, On, Open }
pub struct RoleMap([Role; 256])               // albedo index -> Hair | Skin | Coat | CoatShade | Legs | Boots | Glass | Eye | ... | None
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
| height | 0..255 per pixel | how far the surface is above the ground in screen px: a person's head is 40, a barrel's top 12, a wall tile its wall height; clear is 0 | shadow casting, water, the y-sort of tall things |

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
| `skin` | skin, skin_pale, skin_dark, bone, wax, stone, metal, none | `bone` is the skeleton, `wax` the waxwork, `stone` the statue, `metal` the armour; `none` with `ghost` is the shade. Skin is never dithered |
| `face` | plain, glasses, beard, grim, none | Two eye pixels of `Eye` role, a nose shadow, a mouth line; `glasses` is two `k` rings with a `glint` |
| `coat` | coat, dress, gown, apron, smock, jacket, nightdress, overcoat, canvas, cardigan | `coat_ramp`; `folds` gives cloth its shade bands; the seat swap replaces this role only (§3) |
| `front` | none, apron, shirt, waistcoat, scarf, tie, braces | `front_ramp`; goes to coat on `up` |
| `legs`, `boots`, `pack` | trousers, skirt, bare, pyjamas; boots, shoes, bare; true, false | `legs_ramp` (under a skirt, the stockings; a skirt shows as its own panel only under a coat too short to cover the knee); `boots_ramp`, leather when left out; the pack is drawn on `up`, a strap on `side` |
| `held` | none, hammer, pole, suitcase, dish, bell, lantern, billhook, broom, book, pipe | The composer owns the hand position per frame, so a held thing follows the swing; a held item declares its `hand` for the mirrored side; a lantern emits. Parsed from step 2, drawn from step 7 (§8) |
| `extras` | watch_chain, bell_ankle, shawl, seated, wet | Any number |
| `emits` | eye, glass, held | Which roles may write the emissive layer; anything else is a test failure |
| `ghost` | true, false | Ramps go to mist and the figure is a 50 % checker; height halves so its shadow is faint. No alpha anywhere in a sprite |

Composition order: legs, boots, coat, front, arms, skin, face, hair, hat, held, extras (long hair behind the head, the far arm and leg, and on `side` the pack are drawn first, and the near arm last). Bodies, limbs and skirts are `polygon_lit`, heads `ellipse_lit` (skin is never dithered, so never `soft_ellipse`, and `unchecker` takes out a band edge that falls in a checker), cloth is `folds`, hair is drawn on a layer of its own with its tones held between `deep` and `lift` and a few `strokes(hair)`, then stamped as one part so one head of hair never seams against itself. Every one of them writes its normals and its relief as it goes: relief is what stands in front of what (a sleeve two px proud of the coat), `outline()` finds seams by relief alone, and `upright(ay)` then adds each row's height above the feet (5 px a 4 rows, so the head stands 40). `ao_contact` runs under the feet; `Canvas::outline()` is never typed by a generator.

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

### 2.3 Prop kit

One `Canvas`, now four layers at 16 px a cell; the TS build's three helpers become it. Primitives:

```
fill_rect  rect_lit  rect_bevel  rect_round  ellipse  ellipse_lit  soft_ellipse  disc_lit  hline  vline  line  arc  polyline_fill
gradient(rect, ramp, dir, dither)   folds(rect, ramp, period, phase)   strokes(rect, ramp, kind: fur | feather | grass | hair, density, seed)
grain(rect, material: wood | iron | stone, seed)   courses(rect, material, seed)   ao_contact(rect, spread)
stamp  outline  shadow_ellipse  mirror_x  rotate_ccw  crop  shorten_to  remap  checker
polygon_lit(pts, ramp, curve, z)   set_clip(rect)   upright(ay)   unchecker(ramp)   scale_heights   quench   bounds
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

### 2.5 Icon

```json
"icon": { "class": "flask", "ramp": "liquid_red", "overlay": "cork" }
```

Classes: flask and vial, key, bar, orb, stone and gem, herb, food (round, loaf, cut, pot), tool, garment, paper, misc, spell (a school disc with a mark), status (a ring with a mark). Every icon: drawn once at 32 x 32 with a dark slot, a 1-px `k` outline, a soft light from the top-left and at most 24 indices; the 16 x 16 chip is a second render at the small size, never a downscale. A glass or orb icon emits so the bar glows a little at night.

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

A ramp is six to eight tones in luminance order, `deep, shade, base, light, high, glint` and up to two half-steps for dither pairs; a generator asks by role and never by index. Groups:

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
| Outline: every drawn pixel that meets clear is `k` (1 screen px, half a sim unit); interior seams are `K`; an outline is never coloured | `outline()` runs last; test "outline closed" |
| Light from the top-left in the albedo as a base; dynamic light on top through the normals, so a sprite reads right both with the light pass off (day) and on (night, a lamp) | the lit primitives are the only fills for bodies; test "lit sphere" |
| Contact: AO baked in albedo as index 1 under a thing, plus the cast shadow the light pass draws from height | Index 1 reserved; height on every opaque pixel |
| Colour budget: person ≤ 48, creature ≤ 40, prop ≤ 64, icon ≤ 24, building ≤ 96, `k` and `K` excluded | Test |
| Dither only for gradients and soft edges; never as texture noise on skin or cloth | A test finds no 2 x 2 checker of two skin-ramp tones in any person frame |
| Silhouette at 32 x 40: head 16 wide, a 2-px neck gap, feet 4 wide; no detail under 2 px except eyes, studs and glints | Composer geometry; test |
| Contrast: a hostile body's mean luminance differs from the floor of every zone that spawns it by at least 28 of 255 | Test per unit row |
| No pure black, no pure white | Palette |
| Region palettes: the Lowfields are warm greens and plaster; the Waters blue-greens, reed and slate; the Works slag greys and brick | `RegionStyle { ground, wall, roof, lamp, sky, mist }` rows |

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
| walk `_1 _2 _3` with the standing frame first | the four poses of a walk cycle, bob in px down the screen: stand (the standing frame, both feet down: a pass), `_1` stride (near foot 3 px forward, far 3 back, bob +1), `_2` pass (feet together, the far foot lifted 2, bob 0), `_3` stride (far foot forward, bob +1); seen from the front a stride is the forward foot a pixel lower and the back one lifted a pixel. Arms swing against the legs; `folds` phase advances a quarter a frame. (Corrected 2026-09-27: the first draft's four were half a cycle, contact to up, with the sign of the bob muddled) |
| breathe `_b` | the chest and shoulders 1 px up, the head 1 px up, the coat's folds phase a half turn; the renderer alternates it with the standing frame every 40 ticks |
| `atk_1..3` | wind-up (lean back 2, held thing raised), strike (lean forward 3, arm extended, the held thing at full reach), recover (lean 1); per facing |
| `cast_1..3` | hands together, hands out with a school-coloured emissive glow between them, hands down; per facing |
| `hurt` | the standing frame leant back 2 with the head down 1 and the eyes closed; the renderer flashes it |
| `on`, `open` | The family rule redraws: lid up, glass lit and emitting, lever thrown |
| `base2`, `base3` | `seed + 1`, `seed + 2` |

Creatures read the same table shape with their plan's own cycle: a trot moves diagonal pairs, a wing beat replaces the leg swing with a wing angle, a slither is an offset per segment. A row emits only the cycles its family and its rows promise: a villager who never fights has no attack and no cast, and the atlas is the smaller for it.

### 4.1 Dead frames

| Style | Who | How |
| --- | --- | --- |
| `fallen` | every Person | `side` rotated a quarter turn anticlockwise, repeated columns removed to 28 wide at most, dropped to `ay - 2`, pallor applied, a pool at 38 % from the head; `Dead2` is the same with the near arm flung ahead of her (so it lies above the body), picked by unit id. Height falls to a seventh, the body's thickness, so its cast shadow is a sliver |
| `topple`, `legs_up` | plants; small quadrupeds and birds | on the stem; on the back |
| `scrap` | machines and armour | 8 x 8 plates in two courses |
| `melt`, `wisp` | waxwork; shade | |
| `pose` | every other creature | the plan's own `Dead` |

Six muted pool pairs (step 2 has the one people need, `pool`). `pallor` darkens by 14 % and greys by 20 %; the pallid twins exist once in the palette, for the ramps a person is made of (`palette::PALLID`: skin, hair, cloth, leather), and any other ramp goes a tone darker; a dead frame never emits. Tests: a dead frame is not a living frame, is shorter, is not floating (its lowest pixel is at or below `ay - 2`), and is more than 16 px across.

## 5. Pipeline

**Boot, once, into atlases.** Every look, every variant and every swap is rendered at boot and packed, all four layers. Nothing renders from a look during play; a frame blits from finished pages.

**Budget.** The Pi 3 must hold 60 fps (owner decision; the degrade order is in PRESENTATION's performance section) and the pages are read-only and packed tight for it; at the new size and depth they live in its RAM and not its L2, and that is a cost PRESENTATION carries in its blit budget. The boot cost is paid once behind the loading screen. Pi 3 target: every look cold in under 3 s, measured by `jane bench art`. The **page cache under the save directory**, keyed by the build hash (content hash plus the `jane-art` version), is allowed and expected on a Pi: written by the game, never shipped, regenerated silently when stale, never the truth (`jane sheet` and the tests always generate), and a warm boot from it is a copy. `vary` is budgeted with the rest: a row with variants costs up to four times its frames, and a test sums looks x variants x frames x layers against the page limit.

**Atlas.** 2048 x 2048 shelf-packed page sets, one set holding four layers of one rect map: albedo 8-bit, normal 16-bit, emissive 8-bit, height 8-bit, five bytes a pixel.

| Content | Frames | Pixels | Size, four layers |
| --- | ---: | ---: | ---: |
| units, with seats, variants and every cycle a row promises | ~5000 | ~6.4 M | ~32 MB |
| props | ~400 | ~0.6 M | ~3 MB |
| buildings | ~40 | ~0.5 M | ~2.5 MB |
| icons, 32 and 16 | ~180 | ~0.12 M | ~0.6 MB |
| flora and canopy strips | ~90 | ~0.4 M | ~2 MB |
| font, four faces, albedo only | ~400 glyphs | ~0.25 M | ~0.25 MB |
| chrome, albedo only | | ~0.3 M | ~0.3 MB |
| weather masks and parallax | ~120 | ~1.0 M | ~5 MB |
| total | | ~9.6 M | **~46 MB: three page sets, four at most** |

`Atlas::get(SpriteId) -> &SpriteRef { page, frames: [FrameRect; N], w, h, ax, ay, flat: bool }`; ids are interned `u16` at build; a flat sprite has albedo only. `jane-art` packs the pages; `jane-present` holds and blits them (PRESENTATION, the renderer's formats).

**Looks table.** `data/looks/*.json`, one file per family, merged in path order like every table (PORT §5.2). The key is the sprite id every row already names in `sprite` or `icon`; the value is a `Look`. **Rule:** the build (`jane-schema`) fails on a `sprite` or `icon` that names no look, and on a look no row uses. There is no default look and no pink square.

**Dev tool.** `jane sheet` in `jane-cli`:

| Command | Draws |
| --- | --- |
| `jane sheet units`, `props`, `icons`, `flora`, `chrome`, `font` | a family, every row, 1x and 4x on its real background, day-lit |
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
| outline closed | every drawn pixel meeting clear is `k`; no coloured pixel touches index 0 |
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
2. Person `slim`, the frame tables, the four-frame walk and the breathe, the derivations, `fallen`, the seat swaps. Looks for Jane and the townsfolk.
3. Terrain and flora ported; `TileStyle` rows with height, normal and wetness. The county draws, and draws lit at night.
4. Creature `quadruped_mid`, `quadruped_small`, `bird` with their gaits; Person `bone` skin. The first five minutes have their cast.
5. The kit in first-walk order: `sign`, `lamp` (emissive), `barrier`, `container`, `ritual`, `furniture`, `structure`. The house, the cellar and the mine dress.
6. Building extensions with emitting windows, the icon classes at 32 and 16, the fx tables with their emissive parts, the attack and cast cycles.
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
| Four-frame walks, breathe, three-frame attack and cast, hurt, two dead poses; only the cycles a row promises | §4, §5 |
| Weather and parallax are generators in this crate, driven by `WORLD.md`'s weather state | §2.8 |
| Font is strokes on a 5 x 8 lattice at two working faces (Fine 1x, Small 2x) with Head and Title from the same strokes; no bitmap in source | §6 |
| Seats are coat-only swaps, albedo only | §3 |
| Per-instance `vary` is in, at most four per row, picked by unit id, materialised at boot | §3, §5 |
| The atlas disk cache is allowed and expected on a Pi: under the save directory, never shipped, never the truth | §5 |
| Procedural audio is outlined only (PRESENTATION's audio section), for PLAN M8 | not this crate |
