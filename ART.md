# Jane, Art

How every pixel in the native build is made. Pair with `PRESENTATION.md` (what draws it, lights it and lays out the UI), `ARCHITECTURE.md` (the engine, and the `View` presentation reads, its §11), `PORT.md` (the plan: P5 is the art phase, §6.i is where render-only tiles leave the sim) and `PLAN.md` §6 (the art gaps the web build left).

**Status:** §8 steps 1, 2 and 3 are built: the palette, canvas, font, chrome and light pass; the Person composer with its looks (`data/looks/persons.json`: Jane and her seats, the town, the country folk, the villagers); and terrain and flora (`data/looks/tiles.json`, `jane_art::terrain`, `jane_art::flora`). Step 4 is built (2026-09-27): the creature plans `quadruped_mid`, `quadruped_small` and `bird` with the county's twelve animals, Julie the dog first (`data/looks/creatures.json`), and the Person `bone` skin (the skeleton). Step 5 is built: the prop kit: with the buildings, all 267 prop sprites have a look (`data/looks/props.json`, `props_rest.json`), and a test holds that none draws a stand-in; the station, the lamp road, Julie's yard, house and cellar, and the mine dress. Of step 6 the building painter (`data/looks/buildings.json`, §2.4) and the icons at 32 and 16 (`data/looks/icons.json`, §2.5) are built, and the fx tables (2026-09-27: `jane_art::fx`, an entry for every spell and effect row, §2.8); the people's attack and cast cycles are not. Of step 9 the weather's core is built (2026-09-27, `jane_art::weather`: the mist tile, the sky's hour ramp and stars, the moon, the School as a far silhouette, a treeline per region, the rain's colours; §2.8). The presenter draws all of it (`jane-present`: `people`, `creatures`, `props`). The rest is design. The TS build's generators (`jane/src/art/`) are the reference for method until P5 lands and `jane/` is archived at P10; they are not the reference for the look, which moved up on 2026-09-26 (§0).

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
  rock      every stone on the ground, cut into lit faces: boulders, piles, rubble
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
| Internal canvas | 640 x 360 (2026-10-06; was 768 x 432) | integer-scaled to the window with thin dark bars; the view is wider on a wider window, never smaller |
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
| height | 0..255 per pixel | how far the surface is above the ground in screen px: a person's head is 40, a barrel's top 12, a wall tile its wall height; clear is 0. An upright sprite's pixel stands its row's height above the feet (`Canvas::upright`), so its shadow is its silhouette thrown from the feet's row, `h / tan(elevation)` px from the sun (`light::light_upright`, the reference a backend matches); a flat thing's height is a height field over the ground (`light::light`). **The one projection** (2026-09-27): heights are true px, and a height of h is drawn `rows_up(h) = ceil(4h/5)` rows up the screen (`canvas::{rows_up, height_of_rows}`), five px for every four rows; every lit tier stands a px on the ground that many rows below it. So an upright px is `height_of_rows` of its rows over its feet, and the terrain's faces are too: a face's px is its rows over its foot as true px, what stands behind a face (a wall's top, a cliff's rock, a hedge's crown) is the face's height, and a roof starts from a storey of wall (three cells, 60 px) at its eave. A height of 4 or under is the ground's own relief, never a caster | shadow casting, water, the y-sort of tall things |

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
| `hat` | none, cap, brim, peaked, helmet, scarf, veil, cloche, panama, diving | Drawn over hair; `up` shows its back; a brim casts a small shadow on the face by height. `hat_ramp` names its cloth (the coat's when left out). **As built (step 7):** `veil` is a pale brimmed hat with a black net from its brim to the collar, a line every third column, the face a tone into shade behind it; `diving` is a sphere a px wider than the head on a bolted breastplate, with a round port on the face and one on the near side (lit from inside, emitting, when the look `emits` `glass`: the factory's sentry), a valve and the air line's union from behind |
| `skin` | skin, skin_pale, skin_dark, bone, wax, stone, metal, none | `bone` is the skeleton, `wax` the waxwork, `stone` the statue, `metal` the armour; `none` with `ghost` is the shade; `gilt` (step 7) is Goldskin, the brass ramp a tone brighter all over with a glint on the brow. **The material passes (step 7)**, after the toning and before the outline: plate (`metal` skin, or a coat in `iron`: the Foreman) is laid in lames every four rows, the gap in shade and the next plate's edge in lift with a rivet, and a closed helm has a dark sight slit across the face with the eyes' light in it and a lit ridge; stone is cracked (a line in deep with a lit lip) and grown with lichen on the shoulders and the hem; wax has a sheen on the brow and a glint on the cheek, and is otherwise exactly a person, which is the point. Only flesh bleeds: the dead of `bone`, `wax`, `stone`, `metal` and `none` lie without a pool. An undead person with `emits: ["eye"]` has a dark eye and one pinpoint of ember in it, toward the nose. Skin is never dithered. **`bone` as built** (`person::bone`, step 4): the undead are townsfolk who were outside when the county changed its mind, so the skeleton wears what is left of a coat. The head is carved into a skull (the jaw narrower than the cheekbones), with sockets in `K` and a pinpoint of ember in each when the look emits its eyes, a nasal hollow and a row of teeth over the jaw's line; no hair. The neck is two px of vertebrae, a hand three px of knuckles, a bare leg a two-px shin with a knee's knob. The coat's hem is torn into tongues by hash, open in front on a ribcage (a lit sternum, ribs in shade lines), torn at the back over the spine. A fallen skeleton lies without a pool |
| `face` | plain, glasses, beard, grim, none | Two eye pixels of `Eye` role, a nose shadow, a mouth line; `glasses` is two `k` rings with a `glint` |
| `coat` | coat, dress, gown, apron, smock, jacket, nightdress, overcoat, canvas, cardigan | `coat_ramp`; `folds` gives cloth its shade bands; the seat swap replaces this role only (§3) |
| `front` | none, apron, shirt, waistcoat, scarf, tie, braces | `front_ramp`; goes to coat on `up` |
| `legs`, `boots`, `pack`, `roll` | trousers, skirt, bare, pyjamas; boots, shoes, bare; true, false; true, false | `legs_ramp` (under a skirt, the stockings; a skirt shows as its own panel only under a coat too short to cover the knee); `boots_ramp`, leather when left out; the pack is drawn on `up`, a strap on `side`; `roll` straps a bedroll across the pack (2026-09-28) |
| `held` | none, hammer, pole, suitcase, dish, bell, lantern, billhook, broom, book, pipe, net | The composer owns the hand position per frame, so a held thing follows the swing and comes up with a wind-up; the near hand holds and west mirrors it. A lantern emits when the look `emits` `held` and keeps its light through the outline; a lantern and a suitcase swing a px a beat behind the stride; a pole's and a broom's foot is on the ground; a pipe is at the mouth, not the hand; the net is a cane, a wire hoop above the hat and a gauze bag (the Collector). **Built** (step 7) |
| `extras` | watch_chain, bell_ankle, shawl, seated, wet, stoop, keys, knuckles | Any number. `stoop`: an old back, shoulders a px forward and down, the head a px further. `shawl`: over the shoulders to a point, in the hat's ramp when she wears no hat but names one, else the front's. `watch_chain`: brass across the waistcoat; `keys`: a ring on the belt and three keys swinging a beat behind; `bell_ankle`: a brass bell at the near ankle that goes where the leg goes; `knuckles`: an iron band over each fist; `wet`: everything below the hip a tone darker. `seated` (Mrs Wakes): she sits in her own rocking chair, drawn with her since no chair stands there: its bowed back and spindles behind her (over her, from behind), its arms either side, its rockers under her feet; she faces out of it from the side too, sits two rows lower with her hands in her lap on a ball of wool and two needles, and the walk's beats rock the chair's back. Her dead frames are drawn out of it |
| `emits` | eye, glass, held | Which roles may write the emissive layer; anything else is a test failure |
| `ghost` | true, false | Ramps go to mist and the figure is a 50 % checker; height halves so its shadow is faint. No alpha anywhere in a sprite. **As built (step 7), the better-looking option:** a whole-body checker read as a screen door at 1x, so the shade is solid above the hip in `slate`, tone for tone (a cold blue-grey that the Burial's cold torches pick out), and below the hip it comes apart: two whole rows, then a 50 % checker into tongues of different lengths per column (fixed to the body, so they do not crawl), then nothing. It lays no contact shadow, its eyes keep their light, and its heights halve. `persons.rs` holds a shade to that instead of the solid body's feet, outline and spike rules |

Composition order: legs, boots, coat, front, arms, skin, face, hair, hat, held, extras (long hair behind the head, the far arm and leg, and on `side` the pack are drawn first, and the near arm last). Cloth is `polygon_cloth`: large calm areas of its base, a two-px lit edge on the light's side and the far three tenths in shade, the shoulder's turn in lift; then the painter's clusters: folds under the arms, a waist seam or a belt (two rows of leather lit on top, a brass buckle), hem folds each a line of shade with its lit ridge beside it, lapels as shapes (the near one lit, the far one in shade), a yoke, pocket flaps, an apron's bib, panel, ties and strap. Builds and cuts are silhouettes: broad shoulders square, the rest sloping; a stout belly in profile, a chest on the broad; a fitted bodice over a trapezoid skirt; a cassock to the ground; a shawl; a stoop. Heads are `ellipse_lit` with a jaw a little squarer than an egg, the face painted in four skin tones (skin is never dithered): the fringe's cast shadow on the brow, the cheek and jaw on the far side in mid, a lift on the near cheek, a two-px nose shadow, a mouth; eyes two px square under a lid line three px wide, a glint in their top corner. In profile the neck leaves the skull behind the jaw. Hair is drawn on a layer of its own, held to three tones, given a two-row highlight band arcing over the crown on the light's side and two or three strand lines, then stamped as one part so one head of hair never seams against itself. Sleeves have a lit cuff and, in profile, an elbow; hands are a 4 x 4 skin mitt with its shadow px (Jane's pass, below). Cast shades are laid as clusters with `shade` (a brim on a face, a chin on a neck, a head on a collar, the far limbs in the near ones' shade).

Every part writes its normals and its relief as it goes: relief is what stands in front of what (a sleeve two px proud of the coat). Then one finishing pass, in this order: `retone` gives each material its own few tones (cloth a light, a base and a mid, a shade where the light never reaches; skin a light, a base and a rose mid), `unchecker` and `declutter` leave the shading in clusters of two px or more, `despike` takes off any pixel with clear on three sides, `ao_contact` lays the contact shadow as a solid ellipse, `outline_sel` draws the selective outline with seams by relief, `declutter` runs again inside the line, and `upright(ay)` writes each pixel's true height above the feet (5 px a 4 rows: the head stands 40, the shoulders about 19, a hem about 7), the field a sun or a lamp casts from. No generator types an outline or a height.

**Jane's pass (2026-09-28, the owner's first playtest: "average quality").** She is on screen every frame, so the composer was taken up to her, and every person with it. What changed, each the visually better option: *the face* has its eyes set in toward the nose (`CX - 4`, `CX + 2`: at the old places they sat against the hair and the face read as a blank between two dots), a nose as a px of shade beside its lit bridge, a mouth with a lip of light under it, a jaw tapered to a softer chin; in profile the face has a line (forehead, a nose two rows proud, the lip back under it, the mouth in, the chin), where it was a flat edge whose nose the despike took off; *the hair* has a crown a row over the skull (volume), its lit side held at the ramp's lift (`HAIR`, where it was its base: dark hair read as a helmet), a sheen with a high core, a fringe that comes down in points, long locks that frame the face to the jaw then spread a px each side over the shoulders and end in points, and long hair behind her an A that spreads past her shoulders and ends in points, all a frame behind the body; *the neck* is drawn before what is worn at it (a scarf was drawn under the neck, so it read as two yellow pads), its shade under the chin a step and not two; *the scarf* is wool wound twice round the neck and tucked under the chin, two rolls with a crease between, knotted on her near side with its end hanging over the chest, lit edge and shade edge so it stays the scarf's colour, fringed; *the coat* (`coat`) is belted to mid-thigh and flares below the belt (it stopped at the hip), the belt flush in the cloth rather than seamed into a dark band, the hem folds short creases that leave the skirt calm (they ran up it as pinstripes); *hands* are a 4 x 4 mitt, so the outline leaves them a heart of skin; *the pack* is a rucksack with a flap, two straps and brass buckles, lying over her hair from behind, its straps over her shoulders in front; and a look may ask for *a bedroll* on it (`body.roll`, Jane's: a cream blanket with a red stripe near each end, rolled across the top, its end a curl on the pack from the side; from in front it is not drawn, since its ends read as pegs past her shoulders). The bedroll is the silhouette a traveller has from behind and the side, and hers alone.

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

**As built (step 7).** Five more plans and twelve anatomies (`creature::{flyer, arachnid, serpent, plant}`), every unit sprite now has a look; `machine` was not needed, because the hauler and the sentry are people (a hooded brute, an iron diver whose lit port is its lamp). `size_of(plan, anatomy)` gives the XL rows their box: the Emperor 56 x 56, the queen 48 x 48, the great flower 48 x 56, and a butterfly or a moth 36 x 36 (drawn at one and a half times: at one they did not read at 1x). Every plan keeps the anchor at `(w / 2, h - 4)`, the serpent's too (decided: its head rides the leading segment at ground level like any other foot). `flyer_insect` (butterfly, moth, Emperor; `creature::insect`, in half-px units) and `flyer_bat` hover four rows over the anchor (`hovers`), drift a px on the beat, and carry the wing beat under the walk names; the beat changes the wings' shape, not their size: spread wide and flat, raised half way (shorter, higher, the hindwing tucked), clapped up over the back (tall and narrow), and from the side face on when raised and edge on when spread. Four wings, each its own shape (a forewing with a pointed apex, a rounder hindwing), a soft volume in `belly` with a margin of `body`, a band across it in `mark`, veins, and eyespots in rings (black, `mark`, a pale centre, a pupil): a butterfly's toward the apex, a moth's and the Emperor's on every wing; a furred body with a banded abdomen; a butterfly's feelers clubbed, a moth's feathered with barbs. A bat's wings are leather on three fingers. `arachnid` (spider, queen): abdomen and cephalothorax as soft volumes, each with a gloss (a bright cluster and a glint in its upper left), eight jointed legs fanned round the body (knees clear of each other), each thigh dark with its upper edge lit in `belly` (a black spider's legs sheen brown, the queen's plum), a bright knee, a banded shin, a pale claw, every stroke two px or side-touching so the finish never takes a leg off as spikes; alternate legs lift then reach; two big eyes and a ring of small ones glowing `ember`, fangs; a spider wears a red hourglass, the queen is half as large again with a pale skull on her back, bristling, bone fangs. Dead, either lies on its back, the belly up, the legs drawn in over it in hooks. `crawler` (the lurker, 40 x 32): decided from the data (the Burial's far-spitting snake, the shape on the lake's bank, the thing under the vermin man's hatch, DUNGEONS.md's measure of a thing too strong to fight) as what lives in the dark water under all three, a cave salamander grown huge and pale as a drowned hand: a long body in a slow S, a flat wide head with a lipless mouth and two small shining eyes, pink feathery gills, four stubby splayed legs, a finned tail; it walks in a swim, flares its gills, strikes on a pink mouth, and dies on its back. `serpent_head` (the snake): the sprite is the forepart, a coil on the ground and the neck reared in an S to a flat wedge of a head with gold slit eyes, a pale scaled throat and a forked tongue; the rest of it is `snake_segments`, six lit spheres of scales from the neck's size down, which the presenter lays at every point of the sim's trail, tail first, sorting each where it lies. `plant` (cactus, flower, great flower, pumpkin): roots that shuffle it along and a top that sways a beat behind; the cactus ribbed with lit spines, a bloom and a face; the flower a ring of petals round a mouth of teeth; the great flower two rings, a maw that glows, thorned vines and blades; the pumpkin ribbed with a carved face whose eyes glow and a hop in its walk.

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

Frames: the six-frame walk and a breathe per facing (decided: six, as the people's, over four: a trot reads with weight in six, and the frame table is one table), the idle pair `Idle, Idle2` facing the viewer, `Hurt`, `Atk1..3` for what fights (a unit drawn as the sprite that is not friendly and has a spell book), `Dead`. The gait is a trot, the diagonal pairs together (near fore with far hind), stand, contact, down, pass, contact, down, the tail swinging a beat behind; a hen's step bobs her head forward. The idle pair: a dog or a cat **sits** (the haunches wide, the forelegs straight, the chest up) and on the second beat tilts its head a px and sweeps its tail; a sheep grazes, a rabbit sits up, a hen pecks, a crow cocks its head. Facing the viewer, a beast sits low in the 3/4 view: its head over its chest and forelegs, its back running away up the screen behind; walking away, its rump is lowest and its head peeks over its shoulders; its legs show two thirds of their length. Dead is the plan's own pose: a dog, a sheep or a fox on its side, a small beast on its back, a bird feet up; pallid, never emitting, at most 4 px thick.

**As built: the diagonals (2026-09-28).** Every plan also emits `DownRight .. DownRight5, DownRightB` (facing the viewer and to the right) and `UpRight .. UpRight5, UpRightB` (away and to the right), the walk and the breathe; the presenter mirrors them for the two left diagonals (PRESENTATION §1.11, `jane_present::facing::Face8`). A creature's attack and hurt stay side-on only: a creature striking on a diagonal shows `Atk1..3` and `Hurt`, mirrored to the west (decided: its blow reads from the side, and the atlas is the smaller). The derivation, by plan:

| Plan | Down-right (three-quarter front) | Up-right (three-quarter back) |
| --- | --- | --- |
| quadrupeds | the side rig turned (`quad::Turn`): every x along the body foreshortened to seven tenths about the middle of the legs and every point sheared down the screen by half (a third for the small plan) of how far forward of it it stands, so the chest and head are nearest and lowest and the rump runs back up to the left; the far pair of legs two px over and up and in the body's shade; each foot on its own patch of ground, the lowest foot of the beat on the anchor's row; the head is the front view's face turned (`head_face`: the muzzle round past the cheek, the far eye closed up on the bridge, the blaze and the nose with the muzzle, the far cheek in shade), a dog's ruff and shirt-front under the chin, her collar and tag at her throat | the same shear up the screen by a third (a quarter small; steeper read as rearing): the rump nearest and lowest, the far legs over and up to the left, the tail over the rump, the back of the head beyond the shoulders with the muzzle's end, its blaze and the nose's tip showing past the cheek (`head_back_at`), the collar where neck meets head |
| bird | the tail behind up to the left, the breast toward us, the near wing on the left, the head forward and low with the near eye, the beak down to the right, the far foot over and up | the tail nearest (a hen's fan over the back, a crow's wedge down), the near wing on the right, the head beyond with the right eye and the beak's point past the cheek |
| insects | the down view swung an eighth on the ground, the body's line to the lower right, the screen's depth squashed; the body a chain of beads, both eyes, the feelers ahead | the up view swung the other way, the head up and to the right, one eye |
| bat | the wing swinging away (the right) foreshortened and shaded, the head and eyes come round, one ear edge on | the left wing foreshortened, the snout's end past the cheek |
| arachnid | the down view's layout (bodies, hips, knees, feet) swung an eighth about the body's middle, feet kept on or over the ground; the eye cluster toward the head's lower right, the far eyes closed up | swung the other way; the far edge of the eyes past the head |
| crawler | the head down and to the right, the body back up to the left in its S (the wave across its line), near legs splayed toward us, far ones over the back in shade, both eyes | the head up and to the right, the tail nearest and riding on the ground, the right eye |
| serpent head | the neck leaning into the turn, the wedge's snout down to the right, both eyes, the nostrils at its tip | the snout up to the right, the crown's pattern toward us, the right eye |
| plant | the face toward the turn (the far eye closed up), the ribs, the bloom and the mouth coming round, the arm or leaf on the far side of the turn foreshortened and shaded | the back swung the other way; the pumpkin's carved glow at its edge |

`tests/creatures.rs::every_creature_turns_on_the_diagonals` holds that every look has them and none is its side or its front copied; the geometry test stands every standing frame, the diagonals' too, on the anchor's row.

**Julie.** The dog is Julie (STORY.md §3): dry, patient, never cute, and the heart of the game, so she is drawn to be loved at a glance. A tricolour working dog, a collie's make (black with a white muzzle, blaze and ruff, tan on her cheeks), with feathering along her belly, tan brows over dark eyes with a glint (the brows are what let a dog's face speak), one ear up but tipped over at the top and one folded, a round skull and a short deep muzzle (never a Doberman's spike and wedge), a grey chin (she is old), a white tail tip that flags when she wags, and a faded red collar with a brass tag (she keeps the key). When she stands still she sits and looks at you, and tilts her head. `tests/creatures.rs::julie_is_herself` holds these.

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
| `lamp` | post, sconce, cage, globe, gas, bulb, candle, torch, brazier, great_torch, moonbeam, sunbeam, fire, pit, grate | `mount` post, floor or wall (n, e, s, w). `on` turns glass to the lit ramp and writes it to the emissive layer; the light itself is the renderer's. A made fire (`pit`, the fieldstone ring; `grate`, the old iron grate in its stone surround; `kit/fire.rs`, 2026-10-02) has four frames, its states `[base, on, open]`: `base` cold, `open_2` laid (deadwood crosswise, unlit), `on` lit, `open` ash (its one or two last embers emit, dim); `jane_present::props::Props::fire_look` picks them |
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
- **Families built:** `sign` (post, hanging, fingerpost, name_board, notice, timetable, milestone, gravestone, headstone, standing_stone, memorial, chalk, note), `lamp` (post, gas, sconce, cage, miner, torch, brazier, great_torch, fire, signal; a cold flame takes the accent's ramp), `barrier` (door, gate and a gate edge-on, shutter, boards, drywall, crack, web, rails, sleepers, roots), `container` (chest, trunk, crate, barrel, sack, jar, churn, coffin, tin, bowl, bottles, mug, parcel), `ritual` (hatch, stairs, way_in, plate, lever, orb, manhole, ladder, summon_stone, shrine), `furniture` (table, workbench, desk, counter, altar, shelf, cabinet, case, bed, stove, bench, pew, chair, pegs, register, plinth), `structure` (well, trough, fountain, pillar, chimney, coop, woodpile, washing_line, cart, minecart, trolley, anvil, bale, pillar_box, phone_box, ticket_window, shelter, stall, pump, beehive, scarecrow, tent, hoist), and the first of `vegetation` and `debris` (apple_tree, dry_tree, stump, log, flowers, flowerbed, herb, rose, crop; boulder, rock, rubble, rockface, heap, bones). `machine` (socket, fuse_box, relay_box, call_box, breaker, generator, valve, air_pump, clock, locker) and `small_thing` (bedroll, glove, veil, slippers, boots, key, diving_helmet, hose, scarf, tangle, washing, candles, dinner, tortoise, scroll) followed on 2026-09-27, with more shapes for the others (the mine's and the Burial's mouths, broken steps and track, a bricked arch, a broken door, barn doors, wrecks, a den, a bank, dressed stones, beams of sun and moon on the floor, floor lamps, a portrait, a drawer, a planter, a footbridge, the School's bell and its rope, cage traps, a suit of armour, a waxwork, the mounted fox, drawn from the fox's own creature look). Of the families only a few `vegetation` and `debris` shapes are still step 7's.
- **The dungeons' set pieces (2026-09-28, DUNGEONS.md §2.10).** Twenty-one shapes in `kit::set`, drawn under whichever family a look names when that family has no such shape of its own (a family's own shape always wins, so no older sprite moved and the goldens only grew): `carpet` (a field, a border between guard lines with a running dot, a lozenge medallion, quarter lozenges in the corners, small figures over the field, a worn band where feet cross, fringes at the short ends, a corner turned up; a runner is the same at 6 x 2), `crate_stack` and `crate_pile` (planked crates in battens, braced, one in two stencilled, set back as they stack), `junk_heap` (a low lumpy mound of the body's material with what is in it lying on it and what sticks out of it in the trim: rock and timber, scrap iron, bones and skulls, brick and a pipe, or broken chairs over their own splinters), `shelf_fallen` (a bookcase down on its face, its end up, books under it and in front), `book_spill` (walk-over), `tool_rack` (a dark board on feet, bright tools hung on it: a pick, a shovel, a saw, rope, a hammer and spanner, a lamp), `candlestand` (an iron pricket, three unlit candles run with wax: unlit because a warm light would change the Burial's rules), `tomb_open` (the lid pushed back askew, the dark in it, a skull looking out, a hand on the rim), `lathe`, `parts_bin`, `stanchions` (brass posts and a velvet rope's sag), `bust` (marble on a fluted pedestal), `blackboard` (on its easel, a sum and a diagram in chalk), `globe`, `lab_bench` (a retort, a flask, a rack of tubes), `stool`, `pipe_stack`, `sandbags`, `fallen_trunk` (a tree down: its root plate torn up, moss on its back, bracket fungus, a sawn end in rings) and `mushrooms` (walk-over). Their rows are `data/props/sets.json` (`set_*`), their looks `data/looks/sets.json`; `jane sheet props --only set_` draws them. Nothing in a set piece emits.
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

`style` (cottage, farmhouse, barn, inn, shed, hut, steeple), `storeys` (1 to 3), `roof` (thatch, slate, tile, tin, reed), `wall` (plaster, timber, stone, brick, board, reed), `dormers`, `porch`, `lean_to`, `boarded`, `silhouette`, `rise` (above the walls: a chimney's room), `door` and `trim` ramps, `lit`. The canvas is the footprint wide and the footprint plus the walls plus `rise` tall: the front wall (a storey 28 px, a barn's 36, a shed's 22) stands on the footprint's front edge and the roof covers the footprint over it, its front slope (three fifths) in its courses facing the sun, its back slope beyond in shade (laid as the front is, joints staggered, each course's lower edge a tone up and one slate in five a tone of its own, so it never reads as a flat band), the ridge capped, a brick chimney on it. Its ends give it an outline: thatch, reed, tile and tin are hipped (each end a sloping face, the west one lit and the east in shade, its hip rafter drawn; thatch rounds over its corners), slate is gabled (a barge board down each end), and a slate cottage or farmhouse turns a gable to the road over its door, a window in it. Tile and thatch are drawn in the terrain's `roof_tile` and `thatch` ramps. Plaster weathers in long soft stains over a stone footing; timber framing is beams over it; thatch is laid in lipped courses with its reed in short streaks and a pegged ridge. Frames: `base` (windows dark, reflecting) and `on` for a lit building (windows warm, one in five dark: a room nobody is in). Heights: the wall upright from the foot; the roof `heights_by` from the eave's height to the ridge's, so it lands on the house under it.

**Decided: who owns a building.** A building that is a prop (the county's cottages, the farmhouse, the barn, the inn, a shed, the reed hut, the steeple: a footprint the sim collides) is a sprite drawn here, which the renderer y-sorts, so she walks behind its roof and in front of its wall. A building made of tiles (`HouseWall`, `HouseRoof`: the town's terraces, Julie's house) is the terrain painter's (§2.6). They share materials by ramp and one language of courses, so a cottage and a tiled house stand in one street without a seam in style; the terrain's roofs and walls are painted by the chunk painter's own routines (`terrain::hard`, on its `Painter`, not the `Canvas`), so the building painter keeps its own courses for now; when a shared `Canvas::courses` lands, both move onto it (not done at the merge of 2026-09-27: not a quick change).

### 2.5 Icon

```json
"icon": { "class": "flask", "ramp": "liquid_red", "overlay": "cork" }
```

Classes: flask and vial, key, bar, orb, stone and gem, herb, food (round, loaf, cut, pot), tool, garment, paper, misc, spell (a school disc with a mark), status (a ring with a mark). Every icon: drawn once at 32 x 32 with a dark slot, a 1-px `k` outline, a soft light from the top-left and at most 24 indices; the 16 x 16 chip is a second render at the small size, never a downscale. A glass or orb icon emits so the bar glows a little at night.

**As built (step 6, 2026-09-27).** `Look::Icon(IconLook)` in `data/looks/icons.json`, all 91 icons the items, spells and effects name: `class` (47 of them since the icon pass below, from flask to tortoise, spell and status), `ramp`, `trim` (a flask's glass, a key's ring, a status's rim; a default by class), `mark` (flame, frost, leaf, bolt, burst, fist, hammer, web, skull, shield, drop, star, thorn, heart), `glow`. `jane_art::icon` lays every shape out on a 32-unit grid and places it at the size it draws, so the 16 chip is its own render; a line is two px at the least (a one-px diagonal is a row of spikes). Rendered as two sets: variant 0 the icon at 32, variant 1 the chip at 16. Two changes from the plan above: **the slot is the chrome's** (`chrome::slot` draws it under the icon; an icon carries only its object), and **the outline is selective** like everything else (decided 2026-09-27), not all `k`. `jane sheet icons` draws them on the slot; `tests/icons.rs` holds coverage, the two sizes, the 24-colour budget and glow. **Wired 2026-09-28:** the bag, the bar and a thing lying on the ground (a `shows_loot` prop, a drop) draw these looks (`jane_present::ui::icons::look`, the 32 and its own 16 chip); the UI's old stand-in painter is only the fallback for a name with no look, which no row has.

**The icon pass (2026-09-28, after the owner's first playtest called the icons "average").** Every class was redrawn on a painter of its own (`icon::G::part`): a part is a silhouette on the 32-unit grid painted as a **material** (`Mat`) with normals by a **form** (`Form`: a soft dome, an upright or lying cylinder, a flat facet tilted its own way), light falling across the whole object from the top-left as well as across each form, and a tone of bounced light inside the far edge. The materials: **metal** hard-banded from a dark core to a specular edge and a white glint (keys, ingots, the gold ball as a mirror of the room with a sharp horizon, tools, the tin's vertical sheen); **glass** dark and clear (the slot seen through it) with a rim of refracted light on the far side, a hot streak and a glint, the liquid lit from within up to a meniscus a little above the belly's middle; **gloss** for fruit, glaze, wax and liquids; **soft** for bread, leather, wood and eggs; **cloth** that never shines; **stone** cut in facets each lit by its own tilt (a crack with its lit lip, lichen); **paper** pale with a shadowed edge and a curled corner. Details are laid over the shading last, by hand on the grid: teeth and notches, seams and stitching, a crust's scoring with each cut's lip lit, a wax seal, gills, scutes, a tweed's flecks. Parts over parts take a seam in their own dark (`sep`), so no outline is typed. The 16 chip is its own render: strokes are two px at the least, details under that are left out or drawn apart (the scissors' blades at 16 are strokes, not wedges), and one glint in three survives.

Decided in the pass: **keys** lie on the diagonal, a round bow with its hole (the slot shows through), a collar, the shank, and a bit of two teeth with a notch between, cut as the shank's perpendicular so the bit reads at 16 as well as 32; a coloured key is its colour all through, shaded as metal. The **key on the ground** (`small_thing` `key`, the sprite `tale_key`) is as big as its cell allows (a bow of 8 px with a 2 px hole, a 3 px shank so its middle keeps its light through the line, two teeth), with a glint on the bow, so it reads as a key in the grass at 1x. Mrs Bettany's spare key is `item_key_brass` in the bag; on the ground its `lost_thing` row drew `note`, a sheet of paper, which is the data's to point at `tale_key`. Three classes were added, where one class had been two things: **`moss`** (a cushion of moss in clumps on a flat stone, a few red-capped stalks), **`satchel`** (the haversack: a flap with a stitched edge, strap and brass buckle, the shoulder strap arched over; `sack` stays the tied bag of sacking) and **`linen`** (the washing: two cloths folded and stacked, the top one striped, a peg on them; `scarf` is now a scarf as worn, wrapped in a U and knotted, its ends hanging with bands and a fringe). `herb` is a root with its leaves (a tuber with rings and rootlets; the snakeroot twisted in an S); `bloom` is a rose's whorl, a lily's six points, a pansy's dark face, or five round petals with veins. A **spell** is a disc of its school's light, deep at a bevelled rim and brightest up and to the left of its heart (the bands broken in 2 px clusters, never rings), its mark white-hot with the school's brightest tone round it; a grey disc (iron, grey cloth) takes its mark dark instead, having no colour for a glow to show against. A **status** is an iron bezel round a dark face with its mark a step brighter than the face.

### 2.6 Terrain and flora

The TS painter carries as an algorithm at 16 px a cell: chamfered material edges, autotile, roofs, cliffs, fences, the water shimmer list and the canopy strips. Its seven tables become **one `TileStyle` row per tile** (and one per render-only material) in `data/looks/tiles.json`, keyed by the tile's name in snake case, fields in camel case like every table; `jane-schema` compiles them into `TILE_LOOKS` (`jane_data::tile_looks()`), a static beside `LOOKS` and the catalog and like them out of the content hash (an art edit never refuses a save), a tile or material with no row is a build error, and a `jane-art` test resolves every ramp name. **As built** (§8 step 3):

| Field | Meaning |
| --- | --- |
| `group` | ground, water, wall, roof, flora, made. Ground and water chamfer against each other; wall and roof have painters with faces, ridges and eaves; flora stands on the ground its neighbours lend it; made is a floor with a painter of its own per cell |
| `inherit` | for flora, the ground under it when no neighbour lends one; for ground, the tile it is drawn as (a grown path is grass, crops lie on garden) |
| `height`, `rise` | flat, raised (a face is drawn below its top) or canopy (a crown in the strips); `rise` in px: a ground's standing over lower ground (grass 4 over earth 2 over road 1 over water 0, which orders the rims), a wall's or roof's top, a crown's top |
| `ramp`, `accent`, `pattern` | the swatch: its material ramp, a second ramp (bark under leaf, timber on plaster), and the painter (turf, earth, gravel, setts, water, cliff, plaster, brick, roof tile, slate, thatch, slabs, boards, rail, hedge, tuft, flowers, tree, pine, fence, stone wall, and the rest; the schema's enum is the list) |
| `detail` | the sub-cell table's one number, by pattern: a density out of 16, a course pitch or a slab size in px |
| `normal` | flat, cliff (the face's slope), wall, roof (pitch by facing), water (flat; the renderer ripples it) |
| `wet` | how the tile takes rain: 0 grass and thatch stay matt, 1 earth darkens, 2 stone and plank darken and reflect the sky |
| `wallLike` | out of doors a wide block of it is a building's roof; its face shows where its south is open (walls, cliffs, the void) |
| `wild` | wild ground the speckle filter may redraw as its surroundings, and whose edges wander |

The map ink and the swatch's dark line are not fields: they are the ramp's base and deep tones.

**The dungeons' own materials (step 7, `terrain::interior`).** Each dungeon's wall and floor tile has a pattern of its own, so a frame from inside says which building it is before anything stands in it. Walls (the face a cell shows where its south is open; the top stays calm and dark): `timbered` for the mine (`cave_wall`: its rock, a timber set every third cell, a post under a cap in `wood_dark`, gold glinting in the rock), `crypt` for the Burial (`temple_wall`: ashlar in deep courses, a loculus every other cell with a skull or a bone laid in it, a lit sill under it), `ironwork` for the Factory (`works_wall`: sooted brick in stretcher bond, an iron pilaster riveted down its middle every fourth cell, a riveted beam along the top), `panelled` for the Museum (`museum_wall`: flock paper over a picture rail, moulded panels, a skirting), `pipework` for the Pipes (`pipe_wall`: coursed walling, an iron main with a flange at each joint and a stain run down from it, a copper run on brackets), `wainscot` for the School (`school_wall`: paint over a dado rail, tongue-and-groove boards, a skirting). Floors: `parquet` for the Museum (`wood_dark`, two staves to a square, each square turned), `plates` for the Factory (riveted plates two cells by one in a running bond, a row of rivets along two edges, a faint lug chequer, oil in the most trodden places), `flags` for the Burial (great flags in a running bond, about one in forty a ledger stone carved with a border and lines, gathered in a few patches since 2026-10-02), `grating` for the Pipes (a wet brick invert, standing water dark with the odd glint, a drain grate in one cell in thirty). The mine keeps `rock_floor`, its stones a px high, no more: a lantern at her feet stretched taller ones into a floor of spikes. The library keeps `floor_wood` and the generic `wall`, which the houses share; its material is its bookcases. No ramp was added.

**Render-only tiles leave the sim** (PORT §6.i). `RoofSlate`, `RoofThatch`, `BrickWall` and `Pine` are not sim tiles: a variant comes from (1) the biome and cell hash, read from the skeleton the blueprint carries, and (2) `Blueprint.paint: Vec<(Rect, Material)>`, written by the chunk stamper where an authored place wants a particular roof or wall. Kept as sim tiles with a reason: Glass, StoneWall, DeadTree, Hedge, Boardwalk, Stepping, Crops, FlowerBed.

`terrain::paint_chunk(&mut Painter, &impl TileSource, seed, cx, cy, &mut Chunk)` paints 16 x 16 cells into the renderer's chunk at 256 x 256 px (PRESENTATION, terrain chunks). `TileSource` is `size`, `tile(x, y)`, and optionally `material(x, y)` (the blueprint's paint) and `outdoor`, so the renderer's `View` and the sheet tool both feed it and `jane-art` never sees the sim. A `Chunk` holds `ChunkLayers { albedo: Vec<u32> /* 0xFFRRGGBB */, normal: Vec<[u8; 2]>, emissive: Vec<Ix> /* lit windows */, height: Vec<u8>, wet: [u8; 16 * 16] }`, its strips (one per cell row with standing things, cropped from the 88-row, 32-px-margin strip to what is drawn, each with albedo, normal, height and a mask of clear, solid and canopy for the ghost pass, dappled light and god rays), its water cells with a shimmer phase from the world cell so the shimmer of §2.8 lines up across a seam, and its caster segments (wall runs merged per straight edge, fences along their run, a square round each trunk). It is the one place indices meet RGB outside the palette module: a chunk is blitted whole and never tinted, so the albedo is resolved through the table as it is painted; the other layers stay raw for the light pass (the emissive layer too: palette indices). A chunk is a pure function of the tiles within `REACH` (4) cells of it, their paint and the seed; `chunks_touched(rect)` is what a `Tiles(rect)` event invalidates. The `Painter` holds the styles, the flora bank and all scratch: nothing is allocated once it and the `Chunk` have painted one. Its `Standing` says how the standing things are handed over: `Strips` (the default, what the sheets draw) stamps them into the strips and lists the flora in `placed` too; `Placed` (what `jane-present` uses) lists the flora only, for a renderer that draws them from its atlas and sorts them one by one, and paints fences and low walls into the ground layers with the walls, the rows either side of the chunk included so a fence's top is never cut at a seam. `PaintMap` is `Blueprint.paint` a byte a cell, for a `TileSource` over a view that hands over the rects (`View::paint`). It also hands over each px's distance to land through water (`ChunkLayers::water`, 0 on land, 1..=16 in water, from the surface map it already keeps; not in the chunk hash), which the lit tiers reflect by and darken with (PRESENTATION.md §1.8). **Tuned 2026-09-27 for the 18:40 critique:** a meadow turns to its darker tone only below `TURF_DARK` (-30, where the threshold was 24), so a dark patch is a rare hollow and the meadow's low-frequency variation reads as lighter drifts rather than camouflage; the terrain goldens were re-blessed. **Then in the whole-frame pass (same day):** a meadow never turns darker (`TURF_DARK` off) and turns lighter above `TURF_LIGHT` (250, about one px in twelve); bare ground (earth, mud, slag) keeps its key tone but for a damp hollow under `EARTH_DAMP` and a worn drift over `EARTH_WORN`; every drift's edge is a checker `RIM` wide, dry grass meets green the same way (the lightened band before it is gone); setts take wear a whole stone at a time, read at the stone's middle, one in sixteen darker and three paler; slabs, the dungeon floors and the mine's rock read wear from the broad field (`hard::wear_at`) past rare thresholds (`WEAR_DARK`, `WEAR_LIGHT`), and the rock has a stone now and then.

**Species, autumn and sway (2026-10-02, ART-PLAN Q1, Q3, M2, M3).** A tree tile is drawn as a species (`flora::Species`: oak, beech, ash, silver birch, field maple, weeping willow, hawthorn, yew, holly), each a profile (crown width to height, masses, how much sky shows through its gaps, trunk width, lean and root flare, its bark: ridged, a beech's smooth grey, a birch's white barred black) and leafed green or turned (`flora::Leafing`: a main ramp, a second ramp on some masses, or a lit rim turned first). The chunk painter chooses: a willow within two cells of water (a birch among them), a yew or a holly by a low stone wall over cobbles (a churchyard), oak, ash, hawthorn and maple for a lone or hedgerow tree, a wood in stands of one species by 8-cell blocks with holly in its understorey; about three in five broadleaves turned by the cell's hash, fewer in the Waters, more in the Works and there russet rather than gold. The wood's floor shows a fern or bracken, now and then a stump or a fallen limb; grass at a wood's edge grows ferns and bracken; shrubs go bronze in two in five; hedges turn in clumps along some stretches; fallen leaves lie thick under crowns, drift into the lee of a wall (a cell with a wall to its west or north) and blow out over the square and the lanes. The scatter (tufts, reeds, pebbles, the long grass's clumps) keeps its density only in busy patches (`terrain::busy`, an 8-cell noise) and thins to a fifth beyond them; flowers come in groups of three to seven of one colour; rubble is drawn pulled into heaps of two to four round its 3 x 3 block's centre. All of it is render choice over the same sim tiles. The presenter sways what it places (`jane-present` `Flora::sway`: rest, the top third sheared a px and the top fifth two, one way then the other, by the tick, the plant's own phase and the wind) and rustles reeds and long grass while she walks through them; the wind blows about twenty leaves across a screen by day (none at night, indoors or in heavy rain).

Flora: the 43 generators (46 since the boulders went to six; about 70 since the species) port onto `Canvas` at the new scale, their tone tables become ramps, and a crown is layered leaf masses (a darker back layer, the main ring, a front layer) each a lit sphere with a crescent of shade under it, the whole lit from the top-left with a dark core, leaves as 2 to 3 px clusters lit on the lit side and dark on the shadow side, and a ragged silhouette of leaf tips and notches; the normals are the crown's dome with each mass's sphere over it, so a tree lights as one volume. A canopy tree is stamped into its row's strip for the ghost pass.

**Stones are cut, not blown up** (2026-09-28, the owner's first playtest: "the big round rocks near some rocky cliffs are exact circles"). They were the flora bank's boulders, which the chunk painter stands on a lone cliff cell (the fringe of every crag) and drew as one `soft_ellipse`. Every stone that stands on the ground now comes from one generator, `jane_art::rock::stone`: the bank's boulders (six now, not three, so a crag's fallen stones are seldom twins: the bank is 46) and piles, and the kit's `boulder`, `rock`, `rubble`, `rockface` and `heap`. A silhouette is a ring of seven to ten hashed vertices round an ellipse, each at its own reach, one bitten in on a big stone (a notch where a piece broke away), the lower half pushed past the foot and sliced flat so it sits on the ground; a top face is the ring shrunk toward a point up and left of the middle; the band between is cut into one facet an edge. Each face is a plane with its own normal (the top tipped back to the sky, each side facing out along its edge, a hash of its own), toned by the baked light (a top in light, the west faces lifted, the east in shade, the foot a tone down) and handed to the light pass as that plane, so a lamp lights a boulder facet by facet. A big stone also carries the top's lit rim, a crease only where a lit face turns into a dark one (a crease on every edge read as a cut gem), a pit or two, a crack with its lit lip, and, on two in three, a cushion of moss draped over its crown (deepest in the middle, each column its own depth) with a rosette of lichen on the lit side; a boulder of the bank sheds a pebble (a stone of its own, five px) and has a tuft at its foot. Heights are its rows' true heights over its foot, as a plant's.

### 2.6.1 Dungeon themes: the dressing kit (2026-10-05, ART-PLAN M5 and B2)

A generated dungeon is framed and dressed by one generic painter (`jane_art::terrain::dungeon` reads the zone as rooms, `terrain::hard::dungeon` paints), from two inputs only: a **theme row** in `data/looks/dungeon_themes.json` (`jane-schema` checks it; a `jane-art` test resolves its ramps) and the **room graph** (each mission node's room rect and its kind: `boss`, `hub` as the set room, `rest`; with no graph, rooms are floor that holds a 5 x 5 square and the boss's room is where the boss stands). Nothing in the painter knows a dungeon by name: another game on this engine adds a row naming its zones and gets the same framing. All of it is drawing; no tile, collision or hash of the world moves.

What every themed dungeon gets: faces two cells tall (the solid cell over a face takes its upper half under the row's head trim), door frames (pilasters and a lintel on a north door, capped posts on the others; a boss's door two cells a side, fluted), an inset floor border a cell in from every room wall with its corners turned, worn lanes from door to door, the row's motifs in every room, the boss room's concentric floor with its raised round dais, braziers and emblem, the hub's medallion and two braziers in a pool of stage light, and the row's grade (`jane-present` `light::theme_grade`), which shifts to its fight tint while she stands in the boss's room with the boss alive.

| Field | What it is |
| --- | --- |
| `zones` | The zones that wear it (one theme a zone) |
| `trim`, `trimRamp` | The head trim: `plain`, `cap` (a timber), `dentil`, `frieze` (gilt), `cornice`, `beam` (riveted), `course` (on corbels), `rail` (a picture rail); its ramp, else the face's |
| `upper` | `carry` (the walling carried up) or `shelves` (both halves bookshelves) |
| `frame`, `carving` | The door frames' ramp; `plain`, `angel`, `lamp` or `fluted` |
| `border`, `borderRamps` | `edging`, `lozenge`, `inlay`, `stripes`, `kerb`, `line` or `pebbles`, and its two ramps (`""` the floor's) |
| `laneWear` | 1 polishes a lane a tone lighter, -1 grimes it darker, 0 none |
| `wall` | Face motifs `{ motif, every, at }`: `lamp`, `niche`, `ivy`, `painting`, `sign`, `valve`, `window`, `radiator`, `pegs`, repeated every `every` cells from `at` px |
| `floor` | Floor motifs `{ motif, every, count, ramp, gathers }`: `rails` (along the lanes), `channel`, `belt`, `gantry`, `carpet`, `moon` (under the `window`s), `heaps`, `toadstools`, `plinths`, `stanchions`, `chalk`; `gathers` names scattered prop sprites drawn into the motif instead of where they lie (never one anyone can use) |
| `boss` | `inlay` (the payoff floors' ramp), `spokes`, `setSpokes`, `braziers` (0, 2 or 4), `emblem` (`plain`, `wheel`, `headframe`, `angel`, `furnace`, `clock`, `portrait`, `rose`) and its two `emblemRamps` |
| `grade` | `tint` (multiplies the frame), `lift` (what the shadows lean to; never more red than green), `saturation` (128 as lit), `fight` (the tint in the boss's fight) |

Jane's eight rows: the mine amber, timber-capped, a lamp on every second set and rails along its lanes, a headframe over its arena; the crypt green-grey, dentilled, candle niches and ivy, an angel on every door frame, its bones heaped, a great angel; the museum gilt-friezed, paintings, plinths and stanchions, a great portrait; the library's shelves for walls on a reading-room carpet, a rose window; the works sodium, an I-beam head, hazard stripes, warning signs, belts and gantry shadows, a furnace mouth; the sewer teal, a channel down every room, valves, a great valve; the school cold white, high windows throwing moonlight on its boards, radiators, coat pegs, chalk, a clock stopped at five to nine; the wood a pebble border and fairy rings (its hedges and crowns already stand taller than two cells). The test is `jane-present/tests/dungeons.rs` (ART-PLAN §7 rule 6: every room of every dungeon on seeds 1 and 2 shows a two-cell face, a border and its motif) and `jane-art`'s `a_new_theme_row_frames_a_synthetic_dungeon` (a theme no data row has, on a made-up plan).

### 2.7 Materials

A ramp is six to eight tones in luminance order, `deep, shade, base, light, high, glint` and up to two half-steps for dither pairs; a generator asks by role and never by index. The people's ramps (skin, hair, cloth, leather, the pool: `hue::shadow_hue` lists them) are hue-shifted: each tone's lightness steps from the key, its hue turns toward a cool one in shadow (violet-blue; rose for skin, never grey-brown) and a warm one in light (yellow; peach for skin), and its saturation peaks in the midtones and falls at both ends; a grey takes a cool tint in shadow and a warm one in light. A palette test holds each of them to that. The kit's and the creatures' own materials (wood_dark, wood_pale, iron, brass, copper, glass, bone) shift the same way from step 5, listed in `hue::shadow_hue` beside the terrain's; only `stone` keeps the straight mix (the lit-sphere test's ramp). Steps 4 to 6 added **no ramp** (a creature's fur is drawn in the hair and cloth ramps, which already shift and have pallid twins): with the terrain's 37 and step 8's `turf_slag` the master palette was 1011 entries. **The cap went from 1024 to 2048 on 2026-10-02** (ART-PLAN Q1, `palette::CAP`, the CLUT's width on all three tiers): a ramp added since is laid after the pallid twins (from `LATE_BASE`, 1011), so no index before it moved (a test holds it); the autumn ramps `leaf_beech` (copper gold), `leaf_oak` (russet), `leaf_maple` (butter yellow) and `leaf_yew` (the churchyard's near-black) make it 1043. Index 1, the contact shadow, is a cool multiply (`palette::AO_TINT`, blue held up more than red) and the blit softens its crisp mask by how much of each pixel's 3 x 3 it covers (`palette::ao`), so a shadow has a soft edge and no dither. Groups:

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

About 110 ramps, deduplicated at build. **The master palette is at most 2048 entries** (`palette::CAP`, checked at compile time), laid out as ramps so a remap walks a row, with the ~25 base colours of the TS build's `art/types.ts` kept among them so the county reads as the same place. The old cap of 240 is gone; the per-sprite budgets of §3 are what keep one sprite from using the whole table.

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

**As built (2026-09-27).** `jane_art::weather`: `mist_tile(seed)`, 256 px square, 26 blobs stretched along the wind and a wisp of value noise on a 16 px lattice, wrapped so the tile is seamless (a test), stretched so a fifth of it is clear and a fifth thick, and smoothstepped; `sky(clock)`, the hour ramp's zenith, horizon and afterglow from thirteen keys; `stars(seed)`, 120, thinner near the horizon, one in eight twinkling; `moon(phase)`, a lit disc of bone cut by the terminator in eight phases, emitting; `school(band, lit)`, the School as a front silhouette (three storeys, two wings, a bell tower with an open belfry and a steeple, chimneys, the sky's light along its roof lines) in three distance bands, its windows dark or one or two lit and emitting; `treeline(region, seed)`, a 256 px strip of hashed crowns that tiles, the Works' with chimney stacks; `rain(region)` and `mist_colour(region)`. Tests: the tile is seamless and holds both thick and clear, the ramp is blue by night and noon and warm low at dusk, every far piece draws pixels, the treeline's ends meet. Not built: snow (the world is autumn: WORLD.md §2.4, the Works' winter is not in any milestone), motes and fireflies, chimney smoke as puffs (the town's smoke is a fog layer), caustics (T2 ripples its water in the shader), cloud shadows, and `jane sheet weather` and `parallax` (the frames are `jane sheet scene --weather`). God rays are the renderer's, from where the sun reaches the ground, not a mask of bands.

**Houses with owners (ART-PLAN Q2, M7, M4; 2026-10-03).** Every tile-built house in a zone is found once on entry (`terrain::houses::Houses`: a block of `HouseWall`, `HouseRoof` and `Eaves`, its door, the rows of grass in front of it) and given a seed, and the seed a `Look`: roof (tile, slate, thatch; weathered, new, mossy), wall (cream, pink, limewash or ochre plaster, Flemish brick, flint with brick quoins), door (racing green, oxblood, navy, black; panelled with a fanlight, ledged, stable; a step and a boot scraper), windows (casement, sash, bay, in a rhythm of its own; shutters on one in three, a window box on one in two), ivy or Virginia creeper, wisteria or a climbing rose on one in four, a number or a nameplate, one to three chimney pots on a stack of three heights, and a front garden (cottage, veg, tidy, neglect) with its boundary and gate (pickets, low wall, privet, railings). The painter reads it through `TileSource::house` (`terrain::hard::facade`, `terrain::standing::plot`, the garden pieces in `garden` as flora sprites); the door and chimney props are drawn as their house's variant (`kit::house_variants`). Seeds are picked in reading order so that no two houses a frame can hold share three of roof, wall, door, window rhythm, boundary and hero detail (`jane-present/tests/houses.rs`, seeds 1 to 8). A room seen from inside (`TileSource::room`: Julie's house, the Arms, St Anne's) draws its back wall two cells tall over its solid cells, papered by whose it is, hung with windows, a picture, a calendar, a mirror, a plate rack, the Arms' dartboard and sign, the hymn board and Julie's clock, stopped at five to nine; by day (`TileSource::daylight`, the lamps off) its windows lay light on the floor and the renderer repaints its chunks when the lamps turn; rugs are flat props the presenter lays by a room's tables, bed and hearth. The sprite buildings take shutters, window boxes and a climber by their seed. All drawing: no tile, collision or hash moves.

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
| Region palettes: the Lowfields are warm greens and plaster; the Waters blue-greens, reed and slate; the Works slag greys and brick | `RegionStyle { ground, wall, roof, lamp, sky, mist }` rows. **As built (step 8, `terrain::region`):** `ground`, `wall` and `roof` are ramp swaps, tone for tone, applied per cell as a chunk's albedo is resolved, by `TileSource::region` (the presenter reads `View::region_at`, in the county only: a dungeon, the forest's glades too, is built of its own stuff); the Lowfields are the palette as drawn; the Waters turn turf to `leaf_deep`, dry turf to `marsh`, earth to `mud`, hedges to `needle`, tile roofs to `slate` and thatch to `reed`; the Works turn turf, dry turf and earth to `turf_slag` (the one ramp step 8 added: 1019 entries), gravel to `ballast`, soil to `mud`, plaster to `brick`, tile and thatch to `slate`. The lamp, the sky and the mist are the weather's, in `jane-present`; the standing flora keep their own ramps |

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
| **No stamp twice in view** (ART-PLAN §7 rule 1, 2026-10-03). Two buildings a frame can hold differ in at least three of roof, wall, door, window rhythm, boundary and hero detail | `terrain::houses::Houses::fill` picks seeds under it; `jane-present/tests/houses.rs` on seeds 1 to 8 |
| **Ground varies in rare drifts** (the whole-frame pass, 2026-09-27). A ground keeps its key tone over nine tenths of it; its darker tone is a rare hollow (a few px in a hundred, none on a lawn), its lighter one a worn drift (under one in ten); three tones in near-equal share is camouflage. A drift's edge is a checker of both tones a few px wide, never a 2-px step; a stone, slab or plank takes its wear whole, read at its middle, never as a smudge across its joints | Review on `tools/art-review.sh`; the terrain goldens |
| **A floor mark darkens, it does not paint** (2026-09-27). A stain, a damp patch, a pool or a spill is the contact shade (index 1, the cool multiply), so it lies in whatever floor it is on; blots of a material's darkest tones read as holes | The props test lets a stain draw contact shade alone |
| **Warm and cool meet through grey, never mauve** (2026-09-27). A warm key and a cool fill on one surface sum to magenta when neither has enough green: lights keep the chroma their bytes name (T2's `SKY_CHROMA`), flame light leans yellow-orange rather than red, a grade's lift never has more red than green, and the dusk is gold only where the sun went down (the afterglow a dim yellow, the sky's glow amber) and blue everywhere else | Frame hue on the review set (`sheets/stats.py`-style: 18:40 and 22:00 read 240 to 260, not 280 to 330) |
| **The tiers are one look** (2026-09-27). T2 is the reference; T0's ambient keys are what T2's light gives flat ground, measured off the review set, and every tier draws the grade. A screenshot from any tier is the same hour and mood; only the fanciness differs (soft shadows, reflections, particle counts). Since 2026-09-27 every tier blooms what glows and draws T2's afterglow, T1 draws T2's lamps, fog and shafts, and T0 its lit windows | `tools/art-review.sh DIR all` and `tools/ad-contact.py`, per scene T0 / T1 / T2 |
| **Every tier casts from one list** (2026-09-27). What throws a shadow is the presenter's to say, once, in the `Frame` (its casters and its terrain blocks); every tier draws a shadow for each and for nothing else, the sun's and the nearest lamps' alike. A house throws its shadow across the square on T0 as on T2, a torch stops at the wall behind it on every tier, the afterglow's faint long shadows lie on every tier. Tiers differ in how a shadow looks (T2's penumbra, the lower tiers' dithered edge) and in how many lamps cast (T0 4, T1 8, T2 32, `shadows`), never in whether a thing has one | `jane-cli/tests/casters.rs` (a house, a post and a person at 17:00 on each tier, against T2's area); PRESENTATION.md §1.7's table |

**Decided 2026-09-27: whichever reads better wins.** The owner's rule for every look choice from here: the visually better option is the default, not the cheaper or the older one. Applied first to the outline (selective, above, over the all-`k` outline) and the walk (six frames, over four).

**Shadows are a showpiece**, not a necessity. Every opaque pixel's height is true to its shape (a head at 40, shoulders and a hem thinner, a wall its wall height, a canopy its crown), because the lit tiers cast from it: long soft sun shadows at dusk that swing with the clock, lamp shadows that fan out from every post, penumbrae that widen with distance, all tinted by the ambient (blue by day, a deeper blue at dusk) and never grey. Contact AO is a soft cool ellipse, never a checker, deep enough to read at 1x (`AO_TINT` takes two fifths off the red at its core). A shadow reads as attached at the thing's base: its root is on the foot row on every tier, and T0 and T1 lay a foot under every silhouette that the cast shadow grows out of (`PRESENTATION.md` §1.7, the grounding pass). Since 2026-09-27 every tier casts from the same list (the frame's casters and the terrain's blocks, one list): the terrain's heights make its blocks, so a wall, a roof, a hedge or a cliff drawn a px too tall throws a shadow too long on T0 as on T2, and its relief of 8 px and under (a cobble, a tuft) casts nothing anywhere (`shadow::RELIEF`).

**The art-director review.** Every generator iteration ends in its sheets viewed at 1x and at 3 to 4x, a written critique (what reads, what is muddy, where the grid shows, what looks amateur), and the worst item fixed next. The whole-frame review (PORT.md §7.1 step 6) is `tools/art-review.sh DIR [soft|gl2|wgpu|all]`: fifteen frames with every family in them (the square at 12:00, 17:00, 18:40 and 22:00 and in rain at night, a field edge, the reed camp's shore at dusk and at a misty dawn, the camp by day, a wood at a low sun, the Works, Julie's yard at dusk, the mine, the Burial and the Museum); `JANE=` renders it with another build (a before), and `tools/ad-contact.py OUT LABEL=DIR ...` lays stages or tiers side by side under labels. A family is done when every sprite in it would be defended beside the references at the same scale; the critique of its final sheets goes in the commit or the report that lands it.

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
| `down_right` (south-east; south-west mirrors it) | Three quarters on (decided 2026-09-28, the owner's playtest: she walks diagonally a lot). `down` turned: what is on her front (the face's features, lapels, buttons, a buckle, a scarf's end, a waistcoat, a badge, a skeleton's ribs) moves two px right (`Rig::turn`, `Rig::cx`), the jaw one; the near eye is a cheek in from the skull's edge, the far one by the edge, the nose's shade on the far side; the hair's near lock (the screen's left) a px wider and the far one a px narrower, tucked in under the cheek to meet the jaw; long hair behind her falls toward her back (the left); the pack peeks past her near side. The near arm (the screen's left) holds and hangs over the coat; the far arm is tucked two px in behind the body, drawn before the coat and in its shade. The legs stand a px closer, the far one a px up the screen (it stands further off) and in the near one's shade, each boot a px longer toward the right (its toe) |
| `up_right` (north-east; north-west mirrors it) | Three quarters from behind: `up` turned, the spine, the back seam and the pack two px left (her back faces the viewer's left); the crown's middle and its strands with them; on the right a sliver of cheek and jaw, two px, past the hair. The near arm is the screen's right: it holds, over the coat; the far arm (the left) is tucked in behind the body |
| the diagonals' walk, blows, casts, hurt | The side's tables read along the diagonal (`Rig::along`): a swing of `s` px is `2s/3` across the screen and `s/3` down it facing the viewer (up it facing away), so a three-px stride is two across and one down and a foot stays on the ground's row; the far arm's swing is half hidden. The attack is the side's with its reach two shorter (`ATTACK_DIAG`: a held thing at full reach stays in the frame), the cast and the hurt are the side's. Every person has `down_right`, `down_right_1 .. _5`, `down_right_b`, the same for `up_right`, and a fighter `atk_`, `cast_` and `hurt_down_right` / `_up_right` |
| walk `_1 .. _5` with the standing frame first | six frames a cycle (decided 2026-09-27: six read with weight where four read stiff), bob in px down the screen: stand (both feet down: a pass), `_1` contact (near foot 4 px out ahead, far 4 behind, leaning in a px; a coat's hem opens with the stride, its front edge carried by the leading knee), `_2` down (the weight onto it, bob +1, the far heel up), `_3` pass (feet together, the far foot lifted 2, the arms a px on their way), `_4` contact and `_5` down on the other foot. Seen from the front a contact is the forward foot a pixel lower and the hands two px apart up and down the screen, and the down lifts the back foot two. Arms swing against the legs. What hangs loose (hair ends, a hem, a scarf's tail, a skirt) reads the previous frame's bob and lean, so it lags a frame behind the body; the fold lines sway with the phase, a sixth of a turn a frame |
| breathe `_b` | the chest and shoulders 1 px up, the head 1 px up, the hem where it hung, the folds a half turn on; the renderer alternates it with the standing frame every 40 ticks |
| `atk_1..3` | wind-up (lean back 2, held thing raised), strike (lean forward 3, arm extended, the held thing at full reach), recover (lean 1); per facing |
| `cast_1..3` | hands together, hands out with a school-coloured emissive glow between them, hands down; per facing |
| `hurt` | the standing frame leant back 2 with the head down 1 and the eyes closed; the renderer flashes it |
| `on`, `open` | The family rule redraws: lid up, glass lit and emitting, lever thrown |
| `base2`, `base3` | `seed + 1`, `seed + 2` |

Creatures read the same table shape with their plan's own cycle: a trot moves diagonal pairs, a wing beat replaces the leg swing with a wing angle, a slither is an offset per segment. A row emits only the cycles its family and its rows promise: a villager who never fights has no attack and no cast, and the atlas is the smaller for it.

**Eight facings on screen** (2026-09-28). The sim keeps four facings (its rules turn on a dominant axis); the presenter shows eight (`jane_present::facing::Face8`): moving, the sector nearest the motion among the three within 45 degrees of the sim's facing, the sector she is in held at 110 % against its neighbours so a path along an edge never flickers; standing, the diagonal she walked on is kept while the sim's facing still agrees with it (she stops as she walked) and gives way to the sim's facing when she turns to face something; a motion at right angles to the sim's facing (sliding along a wall she walks into on the diagonal: the sim keeps the axis pressed first) faces the diagonal between the two; one further round (knocked back) leaves her as she stood. The three west sectors draw the east ones' frames mirrored (`FrameId::faces_east`). A look without a diagonal frame shows that diagonal from the side, so a family can gain its diagonals one at a time. `jane sheet facings <id> ...` draws looks round the compass, standing and mid-walk.

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
| `jane sheet terrain [--tile name] [--sample]` | every tile in its sixteen neighbour contexts, dry over wet, a sheet a group; `--sample`: a made-up county and interior holding every tile, dry, by day and by night. `jane sheet flora`: the bank with its normals and heights. `jane sheet county <seed> --full [--at mark or x,y] [--radius cells] [--zoom z]`: the chunk painter over a built county, its layers side by side, its albedo, and lit at five in the afternoon and at night, with the paint time a chunk. Terrain goldens: `tests/terrain_golden.txt` |
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
| Fine | 1x | 1 px | 8 x 12 | dense UI: the terminal, lists, tooltips, map labels: 80 columns by 30 lines at 640 x 360 |
| Small | 2x | 2 px | 12 x 18 | body text, HUD, dialogue: 53 columns by 20 lines at 640 x 360 (64 by 24 at the 768 x 432 the design was first laid out on) |
| Head | 3x | 3 px | 18 x 27 | headings, crits, the zone banner |
| Title | 5x | 4 px | 30 x 45 | the title |

Bold adds one to the pen; shadow is a second pass at (+1, +1) in `k` (at (+2, +2) for Head and Title); outline is a 4-way dilate in `k`. Glyphs: ASCII 32 to 126, `£`, four arrows, a bullet and a degree sign, about a hundred in all, four faces of them. `Text` has an advance per face, `measure`, greedy `wrap` on spaces, and `draw(x, y, str, Style { face, ink, shadow, outline })`.

Tests: glyphs are pairwise distinct (XOR at least 3 px on the 1x lattice); every stroke is inside the box; capitals touch rows 0 and 5; descenders alone reach rows 6 and 7; `l I 1 | ! i` are distinct, `O` and `0` are distinct (the zero is slashed), and `S 5 Z 2 B 8` are distinct; the Small face at 2x is the Fine face's strokes and nothing else.

## 7. Title, chrome, cursor, map inks

**UI at 1x** (decided): chrome is generated on the view grid (640 x 360, wider on wide screens; PRESENTATION's UI section), never on a larger grid scaled down. Chrome is flat (albedo only): the light pass does not touch it.

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
3. Terrain and flora ported; `TileStyle` rows with height, normal and wetness. The county draws, and draws lit at night. **Landed 2026-09-27** (`jane_art::terrain`, `jane_art::flora`; about 1.5 to 2 ms a chunk on a desktop, release).
4. Creature `quadruped_mid`, `quadruped_small`, `bird` with their gaits; Person `bone` skin. The first five minutes have their cast. **Built** (2026-09-27): twelve animals and the skeleton; the creatures' attack cycle with them.
5. The kit in first-walk order: `sign`, `lamp` (emissive), `barrier`, `container`, `ritual`, `furniture`, `structure`. The house, the cellar and the mine dress. **Built**: every prop sprite has a look, `machine` and `small_thing` among them.
6. Building extensions with emitting windows, the icon classes at 32 and 16, the fx tables with their emissive parts, the attack and cast cycles. **The buildings and the icons are built**, and the people's attack, cast and hurt frames per facing for every row whose book strikes or casts, with a cast's light between the hands per school (`fx::cast_glow`); the rest of the fx tables are not, and the UI does not yet draw the icons.
7. The remaining plans, held things, the rest of the kit, Head and Title faces. Every dungeon dresses. **Built:** the dungeons' walls and floors (§2.6, `terrain::interior`) and their set pieces, each dungeon's dress table swapped from the generic pillar, pile, shelf and desk to its own (`data/props/dress.json`): the mine's `pit_prop` and `ore_tub`, the Burial's `crypt_pillar`, `urns` and `tomb`, the Factory's `iron_column` and `drums`, the Pipes' `brick_pier`, the Museum's `column`, the library's and the School's `bookcase`, the School's `school_desk`. `jane sheet scene --at ZONE[:MARK]` draws a frame inside any zone. **The scatter** (DUNGEONS.md: "Dressing is the scatter"): after the mission and the templates have placed everything, each room's open floor takes the dungeon's `dress.scatter_floor` row by its chance a cell (bones, bone heaps, candle ends, stones and rubble, damp and oil and pools, papers, fallen books, broken glass, cogs, leaves and silt), and each cell under the wall's face its `dress.scatter_wall` row (cobwebs thick in the Burial, chains, tools leant in the mine, notices, portraits, moss down the Pipes' walls): hangings are 1 x 1 props whose sprite rises sixteen px up the face over them. Dressing only, by the generator's own rules (`generate::scatter`): no solid row, nothing within three cells of a doorway, nothing on a claimed cell, no two hangings side by side; seeded per node on `DunDress` with its own `b`, so no other roll moves. Their looks are `small_thing` shapes (`stain`, `papers`, `books`, `shards`, `leaves`, `cogs`, `bonepile`, `rug`, `cobweb`, `poster`, `frame`, `moss`, `tools`, `chains`), the floor marks scaled to their footprint. The Head and Title faces were already built (`font`). Also held things; every person row has a look (the tales' people, the bandit, the ruffian, the Collector, the soldiers and the Soldier, the shade, the statue, the Headmaster and Iron Knuckles, the School's masters, Caretaker and Timekeeper, the Factory's Foreman, Charge Hand, haulers and sentries, the Museum's armour, Attendant, Shot-Firer and waxworks, Goldskin). The twelve beasts and plants followed (§2.2): every unit sprite has a look, which `persons.rs` now holds.
8. The title scene, `vary`, the region ramps. **The region ramps are built** (§2.7, `terrain::region`); the title scene is the weather agent's. `vary` now runs on more of the rows that crowd: the skeleton's rags and the hauler's canvas in two colours, the soldier bearded or not, and the dungeons' bookcases, ore tubs, crypt pillars and brick piers in two renders; all within the budget of four (a row's variants must still differ by the silhouette test's measure, which is why the skeleton has two coats, not three).
9. `weather`: mist, fog, rain, storm, snow, motes, fireflies, smoke, caustics, cloud shadow, god rays; the parallax layers; `jane sheet weather` and `parallax`. The county breathes.

**Rule:** a row without a look is a build error (`jane-schema`) and a red test, never a pink square. A look that renders wrong is a redline on a sheet, and the sheet is what the owner reviews at the P5 gate (PORT §7).

## 9. Defaults taken

Recorded as defaults; the owner may flip any with a one-line edit before P5 starts. The first five are the 2026-09-26 direction and are decisions, not defaults.

| Default | Where it lands |
| --- | --- |
| 16 screen px per cell; the sim cell stays 8 units | §0, §1 |
| Internal canvas 640 x 360, integer-scaled, wider on wide screens; UI generated at 1x on that grid, flat | §0, §7, PRESENTATION's window rule |
| Four layers per sprite and tile, emitted by the primitives; a master palette of at most 1024 as ramps; budgets person 48, creature 40, prop 64, building 96, icon 24 | §1.1, §2.7, §3 |
| Six-frame walks (decided 2026-09-27), breathe, three-frame attack and cast, hurt, two dead poses; only the cycles a row promises | §4, §5 |
| Weather and parallax are generators in this crate, driven by `WORLD.md`'s weather state | §2.8 |
| Font is strokes on a 5 x 8 lattice at two working faces (Fine 1x, Small 2x) with Head and Title from the same strokes; no bitmap in source | §6 |
| Seats are coat-only swaps, albedo only | §3 |
| Per-instance `vary` is in, at most four per row, picked by unit id, materialised at boot | §3, §5 |
| The atlas disk cache is allowed and expected on a Pi: under the save directory, never shipped, never the truth | §5 |
| Procedural audio is outlined only (PRESENTATION's audio section), for PLAN M8 | not this crate |
