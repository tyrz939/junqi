# Jane, Presentation

How the native build is seen, heard and driven: the scene contract and the three render backends behind it, lighting and shadows, atmosphere, effects, the immediate-mode UI and its view buffers, input and rebinding, the audio hooks, and the viewer, sheet, film and bench tools. Pair with `ARCHITECTURE.md` (the engine; its §11 is the `View` and `Event` API this file consumes; its §5 is where aim assist lives), `ART.md` (every pixel this file draws is generated there, in four layers), `PORT.md` (phases P6, P7 and P9; the crate map in §4; the perf targets in §9.4, which the per-tier gates of §1.12 here refine), `WORLD.md` (the weather state the atmosphere reads), `VERIFICATION.md` (its L7, human review artefacts, is what `jane sheet scene` and `jane film` feed) and `ENGINE.md` §9 to §12 (the TS record of the renderer, input and UI that carry).

**Changed 2026-09-26.** The single software renderer at 384 x 216 became one scene contract with three backends, a 768 x 432 canvas, four-layer sprites with normal-mapped lighting and cast shadows, atmosphere per area, and aim assist in the sim. The old software design is now the `soft` backend, not the game.

Five crates.

| Crate | Kind | Holds | Depends on | GPU |
| --- | --- | --- | --- | --- |
| `jane-present` | library | `Frame` and `Backend`, atlas, scene, chunks, light, atmosphere, fx, camera, present, view consumption, `ui/`, input mapping, text, the audio trait, bench and film loops | `jane-art`, `jane-sim` (`View`, `Event`, `Command`), `jane-world::names`, `jane-data` (`TEXT`, `NAMES`), `jane-core::view` | none |
| `jane-render-soft` | library | T0: CPU framebuffer, blit, multiply lightmap, mist tile, nearest upscale, read-back | `jane-present` | none |
| `jane-render-gl2` | library | T1: OpenGL 2.1 / GLES 2.0 through `glow`; GLSL 1.20 / ES 1.00; shadow geometry; sharp bilinear | `jane-present`, `glow` | GL 2.1 or GLES 2 |
| `jane-render-wgpu` | library | T2: Vulkan, DX12, Metal or GLES 3 through `wgpu`; WGSL; soft shadows, bloom, grading, water | `jane-present`, `wgpu` | Vulkan 1.1 class |
| `jane-app` | binary | SDL2 window, the probe that picks a backend, audio device, the loop, save slots, `config.json` | the four above, `jane-sim`, `jane-net`, `sdl2` | picks one at boot |

`jane view`, `jane sheet`, `jane film` and `jane bench` are `jane-cli` subcommands (§6) built on `jane-present` and `jane-render-soft` with no window; a `gpu` cargo feature, off by default, adds the other two backends for local use.

```
jane-app → jane-render-{soft,gl2,wgpu} → jane-present → jane-art, jane-sim (View, Event), jane-world::names, jane-data (TEXT, NAMES)

jane-present/src/
  frame     Frame, Pass, the draw lists, Tier, the Features table (§1.1, §1.3)
  backend   trait Backend, Caps, the read-back contract (§1.2)
  atlas     pages from jane-art at boot: albedo, normal, emissive, height; SpriteRef lookup (§1.4)
  scene     draw list, y-sort, snapshots and interpolation
  chunks    terrain chunk cache, four layers a chunk (§1.6)
  light     light list, sun and moon by the clock, ambient by hour and area, shadow casters (§1.7)
  atmos     fog volumes, weather, parallax layers, grading rows (§1.9)
  fx        particles, strokes, rings; event-driven (§2)
  camera    follow, lock, shake, clamp (§1.10)
  present   Present: tick(), draw(alpha) -> &Frame; FrameStats
  view      ViewBuffers built from jane_sim::view::View (§3.6)
  ui/       core, widgets, focus, drag, map, title, loading, console (§3)
  input     device → InputFrame + Edge queue; bindings; the assist profile (§4)
  text      TEXT[TextId], expand, toast and error tables (§3.7)
  audio     AudioBus trait, cue table, NullBus (§5)
  bench     headless frame loop through any backend; pixel counting on soft (§1.12)
  film      jane film: N ticks from a save to PNG frames (§6)

jane-render-soft/src/  fb, blit, lightmap, mist, upscale, readback  |  gl2/src/  context, shaders, shadowgeom, targets, upscale  |  wgpu/src/  device, pipelines, shadows, post, water, readback
```

**Rule:** nothing in `jane-present` reads `GameState`. It reads `jane_sim::view::View` and `Event`, builds its own buffers (§3.6) and sends `Command`s through the app. It never writes state.

**Rule:** `jane-present` is headless and knows no GPU API. It produces a `Frame`; a backend draws it. Every function in it runs in a test with no window; `jane sheet scene` (§6) renders a whole frame that way through `soft`.

**Rule:** floats are allowed in every crate on this page and never decide anything the sim reads. Move and aim are quantised before they become an `InputFrame` (§4). The `soft` pixel path is integer, so its frames are byte-identical across targets.

**Rule:** every presentation timer counts ticks, never frames or milliseconds (§1.11).

**Rule:** the `Frame` is a pure function of `View`, presenter state and tier. Two machines at the same tier with the same save and the same ticks produce the same `Frame`; `soft` turns the same `Frame` into the same pixels on every target.

**The canvas.** The internal pixel canvas is **768 x 432**: 48 x 27 cells at **16 screen px per cell**. The sim cell is unchanged (8 sim units, 1 m; `jane-core::view` still says `CELL_PX = 8` for everything the sim and worldgen read); the render scale is 2, so a sim position `Fx` becomes a canvas position by one shift. People are 32 x 40. Every worldgen and sim constant is what it was.

**The window.** No bars. `s = win_h / 432` (a real number); the canvas is `ceil(win_w / s)` by 432 pixels, so it is always 432 tall and a wider window shows more county, never a smaller picture. Where `s` is an integer (1536 x 864, 2304 x 1296, 3072 x 1728, 3840 x 2160 at 5x) the upscale is a nearest blit; where it is not (1920 x 1080 is 2.5x) T1 and T2 use a **sharp bilinear** shader (nearest inside a texel, bilinear over the texel's last output pixel, so every pixel is square and no edge swims) and T0 uses nearest with an integer-window toggle in Controls. A 21:9 monitor sees 64 cells wide of the same 16-px cells. Every guarantee that mentions the view (the LOS box, the density test, the half-screens of 24 x 13, the roadside anchor rule in `ENGINE.md` §8.2) is stated for 48 x 27 in `jane-core::view` (`PORT.md` §6.h) and holds for wider, because wider only adds what is seen. The sim never learns the window size.

## 1. Scene and backends

### 1.1 The Frame

`present.draw(alpha, canvas)` fills one `Frame` a frame, into reused buffers, and hands a reference to the backend. Nothing in it names a texture, a shader or a pixel format.

The `Frame` owns one flat `Vec` per kind of command, and a pass names a `Span` (start, len) of one of them rather than holding a slice of its own: one allocation per list for the life of the presenter, and no lifetime on the `Frame`. The painted terrain chunks travel in the `Frame` too (`layers`, the chunk cache's slots), so a backend reads everything it draws from the frame it is handed.

```
pub struct Frame { tier: Tier, canvas: (u16, u16), camera: (i32, i32) /* the view's top-left in the zone, canvas px, interpolated */,
                   clear: u32, passes: Vec<Pass>, chunks: Vec<ChunkCmd>, sprites: Vec<SpriteCmd>, layers: Vec<ChunkLayers> }
pub struct Span { start: u32, len: u32 }                      // frame.chunks_in(s), frame.sprites_in(s)
pub enum Pass {                                                // landed (P6 steps 1 and 2)
  Terrain { chunks: Span },                                    // ChunkCmd { id: ChunkId { cx, cy }, generation: u32, x, y: i32, slot: u16 }
  Sprites { layer: Depth, cmds: Span },                        // already in draw order; Standing is y-sorted
  Lights { ambient: [u8; 3] },                                 // the ambient alone until the lightmap (PORT.md §7.1 step 5)
}
pub struct SpriteCmd { page: u8, src: Src { x, y, w, h: u16 }, x: i16, y: i16, flags: Flags { mirror, tint: None | Flash(u8) | Ghost(u8) }, height_px: u8 }
pub struct ChunkLayers { albedo: Vec<u32> }                    // resolved; normal, emissive and height land with the chunk painter
pub enum Depth { Sky, FarLandmark, FarTreeline, Ground, Standing, Canopy, NearFog, Weather, Ui }
```

Every command is in canvas coordinates already (the camera taken off); `camera` is there for parallax. The passes still to come keep the same shape, a `Span` into a list the `Frame` owns (`points: Span` into `lights`, `casters: Span`, `parts: Span`, `cmds: Span` into `ui`), or plain values:

```
  Sky { bands: Span, stars: Span, moon: Option<Moon> },
  Parallax { layer: Depth, factor: Q8, sprites: Span },                                // far landmark, far treeline
  Lights { ambient: Rgb, sun: Option<Directional>, points: Span, casters: Span },
  Fog { volumes: Span /* FogVolume { rect, density: u8, colour: Rgb, drift: (i16, i16) } */ },
  Weather { kind: Clear | Mist | Rain | Storm, intensity: u8, wind: i8 },
  Particles { parts: Span },
  Ui { cmds: Span },
pub struct Light { pos: (i32, i32), height: u8, colour: Rgb, radius: u16, falloff: Falloff, casts: bool, kind: Point | Spot { dir: Angle, cone: u8 } }
pub struct Caster { segs: Span, height: u8 }                                           // footprint edges at ground
pub struct Post { grade: GradeId /* region x hour */, tint: Rgb, bloom: u8, exposure: u8 }
```

Passes are in draw order and a backend draws them in that order; `Pass::needs()` is the tier its `Features` row asks, and a backend below the tier a pass needs draws what its `Features` row says (§1.3), never something of its own. The `Vec`s inside are reserved once and cleared, never dropped: no allocation after the second frame (§1.12).

### 1.2 The Backend trait

```
pub trait Backend {
  fn caps(&self) -> Caps;                                  // tier, max lights, max casters, max texture, has_readback, has_stencil
  fn upload_atlas(&mut self, pages: &AtlasPages);          // albedo, normal, emissive, height; once at boot, again on a CLUT change
  fn draw(&mut self, frame: &Frame);                       // caches chunks by (id, gen); uploads on a miss
  fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16);   // the canvas as 0xAARRGGBB; sheets, film, bench
}
```

| Backend | Tier | Runs on | Draws |
| --- | --- | --- | --- |
| `jane-render-soft` | T0 | any CPU; the fallback when no GPU or no driver; `jane serve`, tests and CI | the previous software design: CLUT blits into a `u32` framebuffer, the multiply lightmap, one mist tile, nearest upscale. Full 768 x 432, or **half res** 384 x 216 (a 2:1 decimated atlas built at boot; every `Frame` coordinate shifted down by one) for the Pentium 4 |
| `jane-render-gl2` | T1 | any GPU from about 2006 (GeForce 6, 7, 8; Radeon X and HD 2000; Intel GMA 950 and up where the driver allows) and Raspberry Pi 2, 3 and 4 by GLES 2 through Mesa V3D and VC4 | normal-mapped lighting in a fragment shader, hard cast shadows from 2D shadow geometry, fog layers, particles, tint. No post beyond tint |
| `jane-render-wgpu` | T2 | modern PCs and Pi 4 and 5 by Vulkan 1.1 (V3DV) | everything T1 plus soft shadows, many lights, bloom, colour grading, screen-space water reflection, higher particle caps |

**Rule:** a backend is a function of the `Frame` and its own caches. It reads no view, no event and no tuning row; everything it needs to draw is in the `Frame` or was uploaded.

**Rule:** `soft` always builds, on every target, with no feature flag. It is what CI renders and what a machine with no GPU plays.

**Rule:** a backend that cannot draw a pass at its tier draws the row below it in the `Features` table; it never skips a pass silently and never invents one.

### 1.3 Tiers and the Features ladder

`jane-app` probes at boot: request a `wgpu` adapter that reports Vulkan 1.1, DX12, Metal or GLES 3 with the limits T2 needs (T2); else create a GL 2.1 or GLES 2 context through SDL and check for framebuffer objects, eight texture units and a fragment shader (T1); else `soft` (T0). `config.json` under `[present]` carries `backend = auto | soft | gl2 | wgpu` and one key per `Features` row. The probe's result and the rows in force are printed on the F2 overlay and by `jane bench`.

Every visual feature is a row. A row names the tier it needs, its default per tier and its `config.json` key, so T0 and T1 degrade by rule, never by accident.

| Feature | Needs | T0 | T1 | T2 | Key |
| --- | --- | --- | --- | --- | --- |
| Multiply lightmap, additive discs, ambient | T0 | on | replaced by shader lighting | replaced | none |
| Normal-mapped lighting (N dot L with light height) | T1 | no | on | on | `normal_light` |
| Emissive layer (lamps, windows, orbs, spells glow unlit) | T0 | as albedo | on | on | none |
| Hard cast shadows from shadow geometry | T1 | no | on, 8 casting lights | on | `shadows` |
| Soft shadows, penumbra by distance | T2 | no | no | on | `soft_shadows` |
| Sun and moon as a directional caster | T1 | ambient only | on | on | `sun_shadows` |
| Silhouette sun shadows: each unit's and prop's albedo mask sheared along the sun by its height, tinted by the ambient, soft-edged by one dither step (decided 2026-09-27: shadows are a showpiece on every tier) | T0 | on | on | replaced by soft shadow maps | `silhouettes` |
| Light count on screen | | 16 | 32 | 128 | `max_lights` |
| Fog volumes per area | T0 | one drift tile | layered, drifting | layered, drifting | `fog` |
| God rays through canopy | T2 | no | no | on | `god_rays` |
| Wetness specular after rain | T1 | no | on | on | `wet` |
| Water | T0 | shimmer pixel | shimmer and refraction | reflection and refraction | `water` |
| Bloom on emissive | T2 | no | no | on | `bloom` |
| Colour grading per region and hour | T0 | CLUT tint | tint | 3D LUT | `grade` |
| Particle pool; weather particles (rain, storm) a third of it | | 900 | 2000 | 8000 | `max_particles` |
| Sharp bilinear upscale | T1 | nearest | on | on | `sharp` |
| Half-res canvas; frame skip | T0 | options | frame skip only | frame skip only | `half_res`, `frame_skip` |

**Rule:** a row's tier is the truth. A test builds the `Frame` for a night in the town at every tier and asserts that no pass a tier cannot draw is present in that tier's `Frame`, and that every pass present is drawn by `soft` without a panic.

### 1.4 Formats and atlases

| Buffer | Format | Size at 768 x 432 | Why |
| --- | --- | --- | --- |
| `soft` framebuffer | `u32` `0xAARRGGBB`, one `Vec<u32>` | 1.3 MB (332 KB at half res) | linear sweeps; SDL streaming texture takes it as is; the light pass is a per-channel multiply |
| Albedo pages | 16-bit master-palette indices (`jane_art::palette::Ix`) into one CLUT `[u32; 1024]`, 2048 x 2048 | 8 MB a page; the whole set ~19 MB, the only pages `soft` holds (`ART.md`, the pipeline section) | one lookup; coat swaps, ghosts and dead ramps are free; on the GPU the CLUT is a 1024 x 1 texture and an albedo texel is two bytes (`LUMINANCE_ALPHA` on GLES 2), the index `lo + 256 * hi` |
| Normal, emissive, height pages | RG8 tangent-space; 16-bit emissive index (an `Ix`, 0 is dark); 8-bit height; the albedo's layout | 5 bytes a texel together | generated by `jane-art` from the shape primitives a look used, never painted; lamps, lit windows, orbs and spells glow unlit; height gives shadow length and water (a person is 40, a barrel its `rise`) |
| Terrain chunks | four layers, 256 x 256 px: albedo `u32` resolved, normal RG8, emissive 16-bit `Ix`, height 8-bit; plus strips | 576 KB a chunk on a GPU, 256 KB on `soft`, and the strips (0 on open ground, up to about 1.1 MB in a dense wood: or none, drawing `placed` flora from the atlas); LRU 48 | the ground is the biggest draw (§1.6) |
| `soft` light buffer | `[u16; 3]` per cell at 1/4 resolution (192 x 108) | 124 KB | §1.7 |
| T1 light and shadow targets | RGB8 at 1/2 resolution; one 8-bit shadow mask | 400 KB and 83 KB | §1.7 |
| Mist tile | 8-bit alpha 256 x 256 | 64 KB | §1.9 |
| UI panels | `u32`, cached by `(w, h, style)` | small | `ART.md`, the chrome section |

**Why 16-bit.** The master palette outgrew 256 entries at `ART.md` §8 step 1 (27 fixed and 40 ramps of 8 is 347; the ceiling is 1024, `ART.md` §2.7). A CLUT per page of 256 would work only while every page's sprites used 254 colours between them, and a coat swap would become a remap per page; a `u16` index is one table for every page, costs one more byte a texel, and keeps the CLUT (4 KB) in L1 on `soft`.

CLUT index 0 is clear, index 1 is the contact shadow (`dst x 0.75`, by shifts); every other index is opaque. There is no per-pixel alpha in an albedo sprite: index 1 or a checker stands in for it. On a GPU the four pages of one layout are bound together, so one sprite quad reads all four in one fragment shader; on `soft` the normal, emissive and height pages are never uploaded.

**Sprite blit on `soft`.**

```
fb.blit(&page, src: Rect, x, y, Flags { mirror: bool, tint: None | Flash(a) | Ghost(a) })
```

A mirrored frame walks its source columns backwards (no mirrored copy in the atlas). The inner loop is `0 → skip; 1 → dst * 0.75 by shifts; else clut[i]`; tints are a two-multiply 8-bit lerp on packed channel pairs. Scalar code that autovectorises on SSE2 and NEON; no intrinsics. A 4 x 4 Bayer matrix dithers gradients only: sky, mist alpha, the low bits of light. Never on a sprite. On T1 and T2 a mirrored quad flips its texture coordinates and its normal's x.

### 1.5 The frame order

The pass order of `ENGINE.md` §9 with the atmosphere layers between:

| Pass | Depth | T1, Pi 4, night, town | T0, Pi 3, night, full res |
| --- | --- | ---: | ---: |
| clear (only the strip outside the zone at its edges); sky bands, stars, moon; far landmark and far treeline by parallax | Sky, FarLandmark, FarTreeline | 0.2 ms | 0.3 ms |
| chunks (≤ 12 on screen); water; flat props, ground marks and spells, drops | Ground | 0.8 ms | 1.8 ms |
| **draw list**: props, units, strips, carried things, projectiles, y-sorted (~300 quads) | Standing | 0.5 ms | 2.4 ms |
| canopy ghost, airborne particles | Canopy | 0.2 ms | 0.6 ms |
| shadow geometry, 8 casting lights | | 2.0 ms | none |
| lighting (multiply on T0; shader on T1) | | 2.5 ms | 3.6 ms |
| fog volumes, weather; indoor fog | NearFog, Weather | 1.6 ms | 2.3 ms |
| tint or grade; overlays: bars, float text, brackets, reticle, UI, F3 | Ui | 0.7 ms | 1.3 ms |
| upscale and present | | 0.6 ms | 1.5 ms |
| **total** | | **~9 ms; budget 12 ms** | **~14 ms; 30 fps or half res** |

**Draw list without allocation.** `DrawCmd { y, key, sprite: SpriteCmd }` in a `Vec` reserved to 4096 once; a counting sort on `y` (the feet, canvas px) into one bucket per canvas row over the view and 96 px round it (anything further is culled before it is pushed), then each bucket put in `key` order by insertion, since a row holds a handful: linear, no allocation, and the same order for the same set however it was pushed. `key` is a prop's id, or a unit's id with the top bit set, so on one row a prop draws under the unit standing at its foot. The sorted list is the `Sprites { Standing }` pass as it stands; flat props go through a second list into `Sprites { Ground }`; no backend sorts.

### 1.6 Terrain chunks

`ChunkCache` as `ENGINE.md` §9 describes, at 2x: 16 x 16 cells painted by `jane_art::terrain::paint_chunk` into 256 x 256 px, LRU 48, invalidated per rect on `Event::Tiles(rect)`, dropped whole on zone change. A chunk is four layers (albedo resolved through the CLUT as it is painted, since a chunk is never tinted; normal; emissive; height) plus its strips (88 rows with 32-px margins, for y-sorting tall tiles against units) and its water cells. A backend caches by `(id, gen)` and uploads on a miss; `soft` keeps only the albedo.

**The painter (landed, `ART.md` §8 step 3).** `jane_art::terrain::paint_chunk(&mut painter, &src, seed, cx, cy, &mut chunk)`, `src` any `TileSource` (the presenter implements it over the `View`: `size`, `tile`, and `material` from the blueprint's paint when it has it). A `Chunk` is `layers` (`ChunkLayers { albedo: Vec<u32> 0xFFRRGGBB, normal, emissive: Vec<Ix>, height, wet: [u8; 256] }`), `strips()` (one per cell row with standing things, cropped to what is drawn, `x`/`y` chunk-local, sorted at `base_y()`, with a mask of clear, solid and canopy), `placed` (the same flora as sprite placements, for a renderer that draws trees from its atlas instead), `water` (cells with a shimmer phase) and `casters` (world canvas px, merged per straight edge). `terrain::chunks_touched(rect)` is what `Event::Tiles(rect)` invalidates (a tile reaches 4 cells). Measured 1.5 to 2 ms a chunk on the owner's desktop in release, about 2.5 ms in a dense wood; a Pi is the P9 measurement.

About 8 ms a chunk on a Pi 3 and 4 ms on a Pi 4 at 2x, so chunks are painted on the worker thread `jane-app` owns (the loading-screen thread), one ahead in the camera's heading, and a frame never waits: a chunk not yet painted draws its region's flat ground swatch for the frames it takes. At most two land in one frame.

**As built (P6 step 2).** Until the painter lands, `jane_present::stand_in::paint_chunk` paints each cell as its tile's flat swatch, on the presenter's thread in `tick()`, every chunk under the view and 64 px round it (a swatch chunk is a fill, not 8 ms). The 48 slots' layers are made when the presenter is (12 MB on `soft`) and live in the `Frame`'s `layers`, so walking into new ground never allocates; a `ChunkCmd` names its slot and its `generation`, which is bumped by every paint.

### 1.7 Lighting and shadows

The reference is the Elysian Shadows look: a pixel-art county whose lamps throw real pools with real shadows and whose walls catch light from the side they face.

**Sources, all from the view.** Prop lights through `View::light_showing(&Prop)` (THE rule, `ENGINE.md` §6, shared with the sim so what she sees lit is what a sentry sees lit); unit glows; her lantern, radius 136 canvas px, when ambient is below 750 permille; effect lights from `fx`; spells. Every light carries colour, radius, falloff (a 256-entry LUT on T0, the same curve in the shader), height above ground, a cast-shadow flag and a kind (point, or spot with a direction and cone: the sentry's eye, the lighthouse). Flicker from a 64-entry table indexed by `(tick + h32(id)) * rate`.

**The sun and moon** are one directional light whose angle follows the clock: azimuth east to west over the day, elevation from the hour, so at 17:00 when Jane steps off the train the shadows are long and point east, and by 18:30 when the lamps come on the sun's share is gone. The moon is the same light at night, dim and blue, with a phase from the day. Directional shadows are cast by every caster on screen at once, which is why T1 caps casting *point* lights and not the sun.

**Ambient** by hour from the clock's keyframes outdoors; a zone's `ambient` indoors, blue-tinted; **per-area overrides** from the skeleton's areas the blueprint carries (the Works darker at noon under their soot; the lake shore brighter under open sky), each a row with a colour and a permille per hour band. Day is free on T0: at ambient ≥ 254 the multiply is an exact no-op and the pass is skipped.

**Occluders.** Props with `rise`, units (a person is 40 tall), wall runs (from `TileStyle.height` and `wall_like`, merged into segments per chunk when the chunk is painted) and canopy trees. A `Caster` is its footprint's edges at ground with a height; casters are gathered once a frame inside the light reach (208 canvas px past the view) and shared by every light.

| Tier | Lighting | Shadows |
| --- | --- | --- |
| T0 `soft` | the previous design: a light buffer at 1/4 resolution cleared to ambient, every light **added** as `colour * LUT[(d2 * 255) / r2] * intensity` over its disc, upsampled bilinear (or nearest plus Bayer under `light_steps`) and multiplied `dst = (dst * L) >> 8`. No normals; emissive texels are drawn as albedo at full brightness | contact shadows only (index 1) |
| T1 `gl2` | per light, a quad over its disc; the fragment shader reads albedo, normal and emissive, computes N dot L with the light's height as z, applies the falloff and the shadow mask, and adds into a half-res light target; ambient and sun first; one multiply pass at the end; emissive added unlit | **hard**: for each casting light and each caster in its disc, every footprint edge facing away from the light is extruded to the disc's rim scaled by `height`, drawn into the shadow mask with the stencil where the context has one and by alpha accumulation into a small FBO where it has not; the light's quad reads the mask. 8 casting lights, then the nearest 8 by rule |
| T2 `wgpu` | the same terms in WGSL over a tiled light list (128 lights, culled per 64 x 64 tile), one pass | **soft**: a 1D polar shadow map per casting light from the same casters, sampled with a penumbra that widens with distance from the occluder; the sun as a directional map over the view. 32 casting lights |

**Rule:** what is lit is decided by the view (`light_showing`), the tier decides only how it looks. A lamp the sim says is lit is lit on every tier; a lamp the sim says is dark casts nothing anywhere.

### 1.8 Materials: wetness and water

**Wet after rain.** Every tile carries a per-cell `wetness` byte in the presenter (never the sim), set to 255 while `WORLD.md`'s weather state is rain or storm, decaying by the tuning row after it clears, faster under roof and canopy, never on water. On T1 and T2 the fragment shader adds a specular term from wetness and the light's height along the tile's normal, so a wet road glints under a lamp; on T0 nothing changes, and that is the row.

**Water.** T0: one 1-px highlight per water cell at a hashed offset, moving by tick (the shimmer). T1: the shimmer plus refraction, the water texel's normal offsetting the sample of what lies under it. T2: **screen-space reflection**, standing things above the water line reflected downward into water cells by their height layer, darkened by depth, rippled by the same normal; the School's silhouette in the lake at dusk is the reason it exists.

### 1.9 Atmosphere layers and grading

The `Frame` is layered by depth, front to back as the table reads:

| Depth | Holds | Parallax | Tier |
| --- | --- | --- | --- |
| `Sky` | bands by hour, stars, moon; from the title painter (`ART.md`, the chrome section) | fixed | T0 |
| `FarLandmark` | the School as `house(silhouette)` at its distance band when it is north of the view outdoors (`PLAN.md` §2.2); one lit window; never indoors | 1/8 | T0 |
| `FarTreeline` | a hashed treeline strip per region | 1/4 | T0 |
| `Ground` | chunks, water, flat props, ground marks, drops | 1 | T0 |
| `Standing` | the y-sorted draw list | 1 | T0 |
| `Canopy` | canopy strips ghosted, airborne particles | 1 | T0 |
| `NearFog` | fog volumes | 1 and drifting | T0 (one tile) |
| `Weather` | rain, storm, mist particles; lightning as a one-frame ambient jump | 1, wind-slanted | T0 |
| `Ui` | §3 | fixed | T0 |

**Fog volumes per area.** Each area of the skeleton owns a `FogVolume` row: density by hour band and by weather (clear, mist, rain, storm; `WORLD.md`), a colour from the region's palette, a drift. The Waters breathe at dawn; the Works hold their smoke all day; a dungeon's `ambient` row may carry its own volume (the mine's dust). On T1 and T2 volumes are drawn as two drifting layers of the 256 x 256 mist tile (26 radial blobs, built at boot) per volume, the near one over `Standing`, the far one under it; on T0 one drift of one tile at the strongest density in view. Indoor fog is black 32-px blocks after the light pass wherever `fog_seen` is clear, every tier.

**God rays** (T2): a radial blur from the sun's screen position through the canopy mask, only under `Canopy` tiles, only when the sun is low and the weather is clear or mist.

**Colour grading** per region and hour: a `Grade` row per region by hour band. T2 applies it as a 3D LUT baked at boot from the row (the Lowfields warm and green-gold, the Waters cold blue-green, the Works sodium and soot); T1 applies the row's tint (one multiply and one add, the only post T1 has); T0 applies the same tint through a CLUT swap for the frame, which is free.

### 1.10 Camera and interpolation

The camera advances **per tick** in sim units: `cam += (target - cam) * 0.18`, shake `*= 0.86` with sine-table offsets, lock to a named rect on `Event::Camera`, clamp to the zone, centre a zone smaller than the canvas. A frame interpolates the camera between its last two tick positions by `alpha`, as it does units, and shifts to canvas px last.

Units: `snapshot()` before each sim tick into a flat `Vec<(UnitId, Fx, Fx)>` sorted by id; a frame lerps the current position against the snapshot; a jump over 40 sim px snaps (travel, respawn, a hop). Moving props the same way. `View::units_in` already hands back `prev_pos` and `moved`, so the snapshot is one copy. Parallax layers take the interpolated camera times their factor.

**As built.** The camera is integer, in `Fx`: the follow is `(target - cam) * 46 >> 8` (0.18), the shake decays by `220 >> 8` (0.86) and wobbles from a 16-step sine table by tick, so two machines at the same tick frame the same view. The target puts her middle (feet less 10 sim px) at the canvas centre; a lock holds a room whole with 24 sim px round it, or keeps her inside it with that pad when it is bigger than the view. The snapshot is the presenter's own: each tick a unit's position becomes the next tick's `prev` (a unit not seen before takes the view's `prev_pos`), so a tick in which the sim did not run leaves everything still instead of replaying the last step. `tick()` frames for the canvas of the last `draw` (`Present::set_canvas` before the first).

### 1.11 Fixed-tick presentation

`present.tick()` runs once per accumulated tick whether or not the sim ran (pause, dialogue, menus), so toasts fade and cursors blink while the world is frozen; `present.draw(alpha)` is pure. A 144 Hz display, a 30 fps Pi 3 and a T2 desktop show the same animation at the same speed.

| Timer | Rule |
| --- | --- |
| Walk frame | `(anim_ticks / 9) & 1` |
| Lunge, lift | the TS offsets over the cast's own ticks, unchanged, at 2x |
| Hurt | `hurt_until = tick + 8`; flash 50 % for the first 4 ticks |
| Float text | lives 60 ticks; rises 2 px every 3 ticks; school colour, outlined (`ART.md`, the chrome section) |
| Camera, shake | per tick (§1.10) |
| Particles | step per tick; drawn at `x + vx * alpha` |
| Mist drift, shimmer, flicker, bob, wetness decay, sun angle, title stars | by tick |
| Lightning | ambient to full for 2 ticks, once per storm roll from the presenter's `Lcg` |
| Dialogue reveal | 3 characters per tick |
| Toasts | 180 ticks; fade over the last 24; three at most, alike ones merged |
| Cooldown sweep, GCD sweep | no timer: drawn from `cooldown_frac` and `gcd_frac` of the view |
| Cursor blink, focus ring | 30 ticks on, 30 off |

**Rule:** `present.tick()` is the only place a presentation counter changes. `draw` reads `tick` and `alpha` and nothing else that moves.

### 1.12 Performance gates, degrade ladder, measurement

Gates, stated per tier; each is a p99 frame time in `jane bench` on the machine named, release build, night, town, 8 shadow-casting lights unless the row says shadows off. They refine the single frame row of `PORT.md` §9.4.

| Tier | Machine | Canvas | Gate |
| --- | --- | --- | ---: |
| T2 `wgpu` | modern PC, 4K output (5x) | 768 x 432 | 60 fps with headroom: p99 < 6 ms |
| T1 `gl2` | Raspberry Pi 4, GLES 2 | 768 x 432 | 60 fps: p99 < 12 ms |
| T1 `gl2` | ancient PC, GeForce 7 class, shadows off | 768 x 432 | 60 fps: p99 < 12 ms |
| T0 `soft` | Pentium 4 | 384 x 216 half res | 60 fps: p99 < 12 ms |
| T0 `soft` | Pentium 4 | 768 x 432 | 30 fps: p99 < 25 ms |
| T1 `gl2` | Raspberry Pi 3, shadows off | 768 x 432 | best effort; recorded, not a gate |

Pentium 4: `-C target-cpu=pentium4`, no `u64` in an inner loop; where SDL has no accelerated backend the upscale is ours, row duplication into the window surface.

**Degrade ladder.** The `Features` rows (§1.3) come off in this order when P9 measures over a gate on a target, and the gate is measured again after each step. They are tuning keys in `config.json` under `[present]`, defaults per target written by `jane bench`, never `cfg` in the code.

| Step | Row off | Saves (T1, Pi 4, night) | Costs |
| --- | --- | ---: | --- |
| 1 | `soft_shadows` → hard; then `god_rays`, `bloom` (T2 only) | | penumbrae; the canopy light |
| 2 | `max_lights` halved; `shadows` to 4 casting lights | ~1.0 ms | fewer pools throw shadows |
| 3 | `fog` to one layer | ~0.8 ms | the county breathes less |
| 4 | `shadows = off` | ~1.0 ms | lamps light without throwing |
| 5 | `max_particles` halved | ~0.3 ms | thinner rain |
| 6 | `normal_light = off`: the T0 lightmap in the shader | ~1.5 ms | flat pools |
| 7 | `half_res` (T0 only) | half the frame | the 2x detail |
| 8 | `frame_skip = 1`: draw every other tick, 30 fps | half the frame | last resort; the sim still steps at 60 and the catch-up cap in `jane-app` still holds |

**Measurement.** `FrameStats` per pass (sky, parallax, chunks, water, list, fx, shadows, light, fog, weather, grade, ui, upscale); F2 prints p50 and p99 over 120 frames, the backend, the tier and the rows in force. `jane bench --save <slot> --frames 600 [--night] [--wide] [--backend b] [--tier t]` runs the same loop headless and prints the table per tier it can reach on the machine, plus **pixels written per frame** on `soft`. CI asserts the deterministic proxies on every target through `soft`: pixels ≤ 6 x canvas area at night, draw list ≤ 1500 in town, lights in the `Frame` ≤ the tier's `max_lights`, casters ≤ 256, chunks landed per frame ≤ 2, and no allocation after the second frame (a counting allocator in the test; a `#[global_allocator]` is an `unsafe impl`, which the workspace's `unsafe_code = "forbid"` refuses in tests too, so until the one-exception crate of `PORT.md` §3.4 exists the in-tree test holds every `Frame` list and the framebuffer to its address and capacity, and the counting allocator runs out of tree: 0 allocations in `tick` + `draw` + `soft` over 2400 frames after the second, idle, walking and idle, on 2026-09-27).

**Rule:** a number in these tables is replaced by a measured one at P6 and enforced at P9; a guess never gates.

## 2. Effects

The 30-odd effect kinds of the TS `fx.ts` carry as tables (`ART.md`, the pipeline section: every spell and effect row has an fx entry that draws pixels).

```
trait Painter { fn rect(&mut self, x, y, w, h, colour: u8, alpha: u8); fn px(&mut self, x, y, colour: u8); fn light(&mut self, l: Light) }
struct Fx { parts: Vec<Part> /* cap by tier */, strokes: [Stroke; 64], rings: [Ring; 32], rng: Lcg, lights: Vec<FxLight> }
impl Fx {
  fn on_event(&mut self, e: &Event, v: &ViewBuffers)      // cast, impact, death, status, swing, sfx resolve here
  fn tick(&mut self)
  fn draw_ground(&self, p: &mut impl Painter, alpha)      // under the draw list
  fn draw_air(&self, p: &mut impl Painter, alpha)         // above it
  fn draw_status(&self, p, unit: &UnitView, alpha)
  fn draw_bolt(&self, p, bolt: &BoltView, alpha)
}
```

Event-driven: a cast or a death resolves at event time, not a frame later. Steps per tick, drawn at `pos + vel * alpha` in canvas px. Fixed pools sized by the tier's `max_particles` row; overflow drops the oldest. `Lcg` reseeded per zone from `h32(seed, zone)`, so the same fight looks the same twice and nothing in it touches the sim's dice. Colours are palette indices; an effect with a glow emits an `FxLight` into the `Lights` pass and an emissive index into its particles, so a bolt lights the wall it passes on T1 and blooms on T2. `Painter` is implemented by the `Frame`'s particle list and by the sheet tool (`jane sheet fx <spell>`, §6).

Tests: every spell and effect row has an entry with the parts its kind needs; every entry draws pixels; ten seconds at 60 casts a second never exceeds a pool at any tier.

## 3. UI

Immediate mode into the `Ui` pass at 1x on the canvas. **The design grid is the canvas** (768 x 432 at 16:9, wider on a wider window; never a second layer at another scale): the Small face is the stroke font at 2x cell, 12 x 18, which gives 64 columns by 24 rows at 16:9, and widgets anchor to edges, so a wider window spreads the HUD and leaves a centred window centred. Head is 24 x 36, Title 30 x 48 (`ART.md`, the font section).

### 3.1 Core

```
pub struct Ui { atlas, font, input: UiInput, focus: Focus, hot: WidgetId, active: WidgetId,
                drops: Vec<(Rect, DropTarget)>, drag: Option<Drag>, tooltip: Option<Tip>, tick: u32, cmds: Vec<UiCmd> }
impl Ui {
  fn begin(&mut self, input: &UiInput, tick: u32); fn end(&mut self) -> (&[UiOut], &[UiCmd]);
  fn panel(&mut self, r: Rect, style: PanelStyle); fn label(&mut self, x, y, text: &str, style: Style);
  fn wrapped(&mut self, r: Rect, text: &str, cols: u8) -> u8 /* lines */;
  fn button(&mut self, id, r, label) -> bool;
  fn slot(&mut self, id, r, view: &SlotView, target: DropTarget) -> SlotOut;
  fn bar(&mut self, r, frac: u8, lag: u8, ramp: RampId);
  fn chip(&mut self, r, icon: SpriteId, count: u16);
  fn list(&mut self, id, r, rows: usize, row_h) -> ListOut;
  fn text_field(&mut self, id, r, buf: &mut FixedStr<16>) -> FieldOut;
  fn popover(&mut self, id, at: (i16, i16), items: &[TextId]) -> Option<usize>;
}
enum UiOut { Command(jane_sim::Command), Intent(AppIntent) }
enum AppIntent { NewGame { name: FixedStr<16> }, Continue, Load(u8), Save(u8), ToTitle, Quit, Resume, Rebind(Binding), Assist(AssistProfile), Backend(BackendChoice), Host, Join(Addr), Console(ConsoleLine) }
```

`WidgetId = h32(file_line_hash, index)`. The app drains `UiOut` each frame: a `Command` goes to the session (local or lockstep), an `Intent` to the app. **The UI never calls into the sim.** Layers, top one eats input: debug, terminal, title, pause, window, dialogue, HUD and world. Text wraps greedily on spaces; a dialogue line's unrevealed tail is laid out invisibly so the box never reflows as it types. `UiCmd`s are panel, glyph and sprite draws in canvas px; every backend draws them the same way, unlit, above every other pass.

### 3.2 Widget list

| Screen | Holds |
| --- | --- |
| Title | The chrome scene (`ART.md`, the chrome section) at the canvas size, drawn through the `Sky` and `FarLandmark` passes so the School's window flickers and its silhouette sits in the lake on T2. New Game (name field, 16 characters, `clean_name` before `Join`), Continue, Load, Controls, Host, Join, Quit |
| Loading | New Game builds all 13 zones up front (`PORT.md` §9.4: under 5 s on a Pi 3, under half a second on a modern x86-64), on a thread the app owns, and the screen shows the skeleton forming: land, then river and lake, then roads and rail, then sites, lamps and patches, then one mark per zone as its blueprint lands, drawn with the same card painter as `jane view` (§6) at 2x, a stage name in Small face beneath. Continue and Load show the same screen while the county is rebuilt from seed and deltas. Input is ignored until the last zone; there is no cancel |
| Load, Continue | Three slot rows from the app's save summaries (zone, day, hour, hp, when), read from the save header without decoding (`ARCHITECTURE.md` §3.5). An unreadable slot is an empty slot |
| Controls | One row per action: label, key, mouse, pad. Press-to-rebind captures the next press; conflicts are shown in `R`, never refused; Reset restores `data/bindings.json`. An Aim Assist row (Off, Pad, Mouse) per seat. A Video row: backend (auto, soft, gl2, wgpu), integer window on T0, and one toggle per `Features` row the tier has |
| Pause | Resume, Save (enabled only when `MeView.can_save`, in reach of a bed or a fire), Load, Controls, Quit to Title. Solo it freezes the sim through the app; with company nothing freezes (`ENGINE.md` §4) |
| HUD | Vitals top-left (hp, mp, energy bars with damage lag); up to 8 status chips with a conic sweep; target or boss frame top-centre; zone name, clock, day and a sun or moon top-right; quest tracker of at most 4 lines on the right; the prompt above the bar (`[E] Read: Notice`, key or pad glyph from the bindings table); Bag, Book, Quests, Map and Menu buttons, pointer only; the zone banner; the death veil with the respawn count; toasts; the 8-slot bar with cooldown and GCD sweeps, a fail flash and slot labels; the reticle at the assisted aim (§4) |
| Window | Tabs: Inventory (8 x 3 bag, the craft strip when `at_bench`, the stats card), Spellbook, Quests, Map (§3.5). Centred 640 x 380; the stats card drops below the bag on a narrow canvas. Pauses the sim solo, never with company |
| Popover | Use, Move, Destroy on a slot; destroying outside the bag asks once |
| Tooltip | After 20 ticks under the pointer or the focus ring; 40-column wrap; name, count, what it does |
| Dialogue | Speaker name, the line revealed at 3 characters a tick, at most 2 options, a "more" glyph when a line continues, device hints for advance and choose |
| Terminal | Backquote; the top third; a ring of 400 lines and a history of 100; tab completion over the console rows of `ENGINE.md` §12; every line becomes `AppIntent::Console`, and every mutation it can cause is a `Command::Dev` (`ARCHITECTURE.md` §1) |
| Host, Join | Host: slot, open or closed, seats, input delay. Join: the broadcast list plus an address field; refused joins show both content hashes (`ARCHITECTURE.md` §7). P8 |
| Debug | F2: fps, frame p50 and p99 per pass, backend and tier, the `Features` rows in force, tick µs, awake and total units, live and built chunks, lights and casters in the `Frame`, path searches, dropped ticks, state hash. F3: solid cells, occupancy, unit paths, trigger rects, caster segments, the load ring |

### 3.3 Focus and navigation

```
struct Focus { region: Bag | Craft | Bar | List | None, index: u8 }
impl Focus { fn move_cursor(dir); fn home(); fn cycle_tab(); fn cursor_target() -> DropTarget }
```

The last device used decides whether the focus ring is drawn and which hints show; the UI-mode key map swap of the TS input layer carries (a window open turns move keys into cursor keys and USE into confirm). `DragPayload` (a bag slot, a spell, a bar slot) meets `DropTarget` (bag, craft, bar, bar background, window, outside).

### 3.4 Drag and drop

```
struct Drag { payload: DragPayload, origin: DropTarget, start: (i16, i16), moved: bool, ghost: SpriteId }
```

Arm on pointer down; start once the pointer has moved 4 canvas px; the ghost follows at 70 %; every widget pushes its `(rect, DropTarget)` in the frame; the last hit under the pointer wins on release; outside the window asks to destroy; the result is one `bagMove`, `craftPut` or `barSet` command. On a pad, confirm picks up and confirm puts down at `cursor_target`. Right click, or a second tap, opens the popover.

### 3.5 Map tab

The chart algorithm of the TS map tab carries whole, on `u32` buffers. Outdoors 1 chart px = 4 x 4 cells (the 2000 x 2000 county is 500 x 500); weighted inks (ground 1, water 2.5, road and rail 4, roofs 6, a solid prop's area counting as roof at 20 cells and over); a 3 x 3 majority on ground inks; woods stippled; roads, water and roofs never smoothed away. Indoors the commonest tile per block. Ink colours are the `MapInk` rows of the palette (`ART.md`, the chrome section).

`MapChart { terrain, composed, seen }` per zone. `terrain` is painted once per zone from the blueprint; `composed` is recomposed only when the fog bits change (checked every 30 ticks): unseen ground is a four-step smoke of two-octave value noise, dithered; unseen indoors is nothing. Zoom `[fit, 1, 2, 3, 4, 6]`, nearest-neighbour blit, drag or stick to pan, `0` to recentre, the School's mark, a blinking dot for her and one per seat. The map is a `UiCmd` like any panel and is never lit.

### 3.6 The view buffers

`jane_sim::view::View` (`ARCHITECTURE.md` §11) is the sim's contract: one seat, her zone, read only, with the rules the sim also uses (`light_showing`, `focus`, `near_bench`, `craft_output`, `hud`, `assisted_aim`) behind it. `jane-present` walks it once per frame and fills buffers the app owns:

```
fn build(view: &jane_sim::view::View, events: &[Event], out: &mut ViewBuffers)
```

The `Vec`s inside are reserved on the first frame and cleared, never dropped, after it: **no allocation after the first frame** (asserted by the counting allocator of §1.12). No reference into `GameState` survives the call; strings are `&'static str` from `NAMES` and `TEXT` or small fixed buffers; the only thing that leaves the UI is a `Command`.

```
pub struct ViewBuffers { tick: u32, seed: u32, zone: ZoneView, me: MeView, hud: HudView, window: WindowView,
                         dialogue: Option<DialogueView>, scene: SceneView }
ZoneView { id: ZoneId, name: &'static str, w_cells, h_cells: u16, indoor: bool, ambient: u8, rects: &[NamedRect],
           areas: &[AreaView { rect, ambient: Option<AmbientRow>, fog: Option<FogRow>, grade: GradeId }],
           weather: Clear | Mist | Rain | Storm, grid: GridRef /* read-only tiles and flags */, fog: FogRef }
MeView { unit_id, seat: u8, x, y: Fx, facing, dead: bool, respawn_ticks: u32, can_save: bool, at_bench: bool,
         clock_ticks: u32 /* since midnight */, day: u16, night: bool, aim: Option<Angle> /* View::assisted_aim */ }
HudView { hp, hp_max, mp, mp_max, en, en_max: Milli,
          statuses: [Option<StatusChip { icon, ticks_left, total }>; 8],
          target: Option<TargetFrame { name, hp_frac: u8, boss: bool }>, boss: bool,
          zone_name: &'static str, clock_text: FixedStr<8>,
          tracker: [Option<QuestLine { title, progress_text, ready }>; 4],
          prompt: Option<Prompt { verb: Verb, label: &'static str }>,
          bar: [BarSlotView { icon, count, cooldown_frac, gcd_frac: u8, usable: bool, source }; 8],
          banner: Option<TextId>, toasts: [Option<Toast { kind: ToastKind, until }>; 3], party_size: u8 }
WindowView { bag: [SlotView { icon, count, name, usable }; 24],
             craft: CraftView { inputs: [SlotView; 3], output: Option<SlotView>, enabled: bool },
             stats: StatsCard, book: Vec<SpellRow { id, name, icon, cost, range, cooldown_text, bound_slot }>,
             quests: Vec<QuestRow { id, title, state, lines }>,
             map: MapSource { zone, step, buildings, seen_version } }
DialogueView { speaker: &'static str, text: &str /* expanded */, options: [Option<&str>; 2], choosing: bool, more: bool }
SceneView { units: Vec<UnitView { id, x, y: Fx, prev_x, prev_y, facing, sprite: SpriteId, variant: u8,
                                  anim_ticks: u32, action: Idle | Walk | Attack(t) | Cast(t) | Dead,
                                  hp_frac: u8, hostile: bool, friend_seat: Option<u8>, statuses, glow: Option<Light>,
                                  carrying: Option<SpriteId>, is_me: bool, height: u8 }>,
            snake: Option<SnakeView { head, trail }>,
            props: Vec<PropView { id, sprite, x, y, frame: u8 /* open > on > base, base2, base3 by id hash */,
                                  flat: bool, light: Option<Light>, loot_icon: Option<SpriteId>, height: u8 }>,
            drops: Vec<DropView>, projectiles: Vec<BoltView { id, x, y, vx, vy, spell }>,
            grounds: Vec<GroundView { spell, x, y, r, ticks_left, total }>,
            walls: Vec<Seg> /* caster segments of the chunks in reach */,
            camera_lock: Option<Rect>, hover_friend: Option<UnitId>, lights_ambient: [u8; 3], sun: Directional }
```

`UnitView.variant` is the bounded folk variation of `ART.md` (the style rules): `h32(unit.id, 0, VARY) % n` over a row's `vary` set, `n ≤ 4`, materialised at boot; a seat's coat swap is separate and is the only thing that tells seats apart. `props` covers only the 16-cell blocks under the canvas plus 208 canvas px, the light reach; `walls` the same. `MeView.clock_ticks` is ticks since midnight; the clock text, the sun or moon glyph and the sun's angle are worked out here, not in the sim. `weather` is read from the view, which reads `WORLD.md`'s weather state; the presenter never rolls weather.

**Rule:** a rule the sim reads (what is lit, what USE would do, whether she may save, where the assisted aim points) is read through `View`, never re-derived in `jane-present`. The view is tested in the sim's suite; the buffers are tested here for shape only.

### 3.7 Text

The sim has no English (`ARCHITECTURE.md` §0). `jane-data` compiles every string in the content into `TEXT: [&str; N]` indexed by `TextId`; `jane-present::text` owns it and everything done to it.

```
fn expand(s: &str, heroine: &str, seed: u32, out: &mut FixedStr<256>)   // {name} → heroine; {place:kind} → jane_world::names::story_name(seed, kind)
fn toast(kind: &ToastKind, heroine, seed) -> &str                        // one row per ToastKind variant
fn spell_error(e: SpellError) -> &'static str
fn verb(v: Verb) -> &'static str                                          // Enter, Unlock, Open, PickUp, Craft, Read, Use, HoldToPush, Take, Talk, PutDown, Custom(TextId)
fn clean_name(raw: &str) -> FixedStr<16>                                  // before Join
```

Toasts arrive as `Event::Toast(ToastKind)`; the table here turns `KillProgress { quest, req, n, of }` into "Rats 3 of 5" and `Needs { item, qty }` into "Needs wood x2". Numbers are formatted by integer helpers; nothing here calls a float formatter. **Rule:** a `ToastKind` variant or a `TextId` without a row is a build error, not a blank toast.

## 4. Input

`jane-app` turns SDL2 events into device state; `jane-present::input` turns device state into two outputs and nothing downstream knows which device it was (`ENGINE.md` §10):

```
InputFrame { mv_dir: Angle, mv_mag: u8 /* 0..127 */, aim: Option<Angle>, sprint: bool, use_held: bool, assist: AssistProfile }   // ARCHITECTURE.md §3.4 plus one byte, one per tick
enum AssistProfile { Off, Pad, Mouse }
enum Edge { Ui(UiAction), Game(GameAction) }                                                                                     // queued on press
```

**Aim assist lives in the sim** (`ARCHITECTURE.md` §5), not here, so it is deterministic and replays: the client sends the raw aim and the profile, and the sim's tuning rows per profile set the cone, the magnetism and the stickiness (the partial assist of a console FPS: a pull toward a hostile inside the cone, a hold on the one it has, never a snap). This section says only that. The reticle draws the assisted aim the view reports, `View::assisted_aim(seat)`, carried as `MeView.aim`, so what she sees is where the bolt goes and the reticle is never a frame ahead of the sim. The profile is a Controls row per seat (`AppIntent::Assist`), defaulting to `Pad` when the last device was a pad and `Off` for a mouse, and a change is a `Command`, so every seat and every replay knows it.

Move and aim vectors are turned into `(Angle, magnitude)` with floats here and quantised; the sim never normalises. Keyboard by `Scancode`; mouse from window px to canvas px through the window's `s`, aim from the chest (12 canvas px above the feet); right button held walks toward the cursor with strength `dist / 64`, the 2020 virtual stick; left button is bar slot 1 unless a widget is hot; wheel zooms the map or scrolls a list. `SDL_GameController` with the standard mapping: left stick or D-pad move, right stick aim, A X Y LB RB bar 1 to 5, B use, RT sprint, View bags, Menu pause. Text entry through `SDL_StartTextInput`; the held set is cleared while text is captured. The reticle is drawn and the OS cursor hidden while the mouse aims.

**Bindings are data.** `data/bindings.json` is compiled in with the rest of the content (`ARCHITECTURE.md` §6): `Binding { action, keys: [Scancode; 2], mouse: Option<Button>, pad: Option<PadInput> }`. User overrides live in `config.json` beside the saves, one row per changed action, applied over the compiled table at boot. The Controls screen rebinds by press-capture; conflicts are shown, not refused; the prompt's glyphs and every hint read the same table, so a rebound key is never shown wrong.

**Rule:** `jane-present` never sees an SDL type. The app hands it `DeviceState { keys: BitSet<512>, mouse, pad }` and reads back `InputFrame` and `Edge`s; the same struct is what a test fills.

**Rule:** nothing in `input` bends an aim. The raw angle leaves this crate; the sim bends it; the view reports where it went.

## 5. Audio hooks

```
trait AudioBus { fn music(&mut self, cue: MusicCue); fn sfx(&mut self, kind: SfxKind, at: (Fx, Fx), listener: (Fx, Fx)); fn tick(&mut self) }
enum MusicCue { Title, Zone(ZoneId, night: bool), Combat, Dead, Silence }
struct NullBus;
```

The cue table lives in `jane-present::audio`: title on the title screen; `Zone` on entry and at the night boundary; a hostile in combat with her for 2 s → `Combat`, 6 s after the last → back to the zone cue; `Event::PlayerDied` → `Dead`. `Event::Sfx { kind, at }` passes through attenuated over 24 cells from the listener. Weather is an `Sfx` loop the cue table starts and stops from the view's weather state. `jane-app` owns the SDL audio device and implements the trait; `NullBus` is what `jane serve`, the tests and the first release use.

Procedural audio is outlined for `PLAN.md` M8 only and nothing before it depends on it: `SfxPatch { wave, pitch: (start, end), decay_ms, duty, vibrato }` rendered into buffers at boot, music as a tick-driven sequencer over the same voices. No audio file ships (`DESIGN-2020.md` §6).

## 6. Native viewer, sheets, film and bench

All are `jane-cli` subcommands drawing through `jane-render-soft` into an unshown canvas (or any backend with the `gpu` feature and `--backend`), reading back with `Backend::read_back` and writing PNG through the 60-line encoder in `jane-art::sheet` (no image crate, `PORT.md` §3.5).

| Command | Does |
| --- | --- |
| `jane view [--seed n] [--out dir]` | 24 skeleton cards and one county's detail at 5x with layers (land, regions or threat, day or night, walking times, every rule with its measured value, roads, rail, patches, anchors in gold), tables in the Small face (so it is also a font test). Never rasterises the full county. With `--out` it writes the cards as PNG; otherwise it opens a window through the `window` cargo feature of `jane-cli`, off by default so a Pi host builds `jane serve` with no SDL |
| `jane sheet county <seed> [--night] [--layer l]` | the skeleton card at 1x |
| `jane sheet county <seed> --full [--at mark] [--radius cells]` | the real chunk painter over a window of the county, all four layers side by side |
| `jane sheet dungeon <id> <seed>` | the dungeon's floor with names |
| `jane sheet scene --save <slot> [--tick n] [--night] [--tier t] [--backend b]` | one whole frame, headless, lighting, atmosphere and UI included; at T0 through `soft` it is the frame CI attaches; at T1 or T2 locally it is the frame the owner reviews. **As built** (no save slots yet): `jane sheet scene [--seed n] [--minutes m \| --ticks t] [--model reader\|rusher] [--night \| --hour h] [--wide] [--out file.png \| dir]` plays the seed from New Game with a player model as `jane play` does, the presenter ticking beside it every frame, sets the clock if asked, and writes one T0 frame (768 x 432, or 1008 x 432 with `--wide`) to `sheets/` |
| `jane film --save <slot> --ticks N --out dir/ [--every k] [--tier t] [--backend b]` | N ticks from a save, one frame per tick at `alpha = 1`, a PNG per frame (or every k-th), the `FrameStats` table beside them; through `soft` at T0 on CI, any backend locally. The same tool over a trace range (`--trace`, `--from`, `--to`, `--clips`) is the film clip of `VERIFICATION.md` L7. Frames and scene sheets are how a lamp coming on at 18:30, a shadow swinging with the sun or the rain starting is looked at, not asserted |
| `jane sheet fx <spell>` | the effect's parts over 60 ticks, one column per tick, with its lights |
| `jane sheet units\|props\|icons\|flora\|chrome\|font`, `unit <id>`, `title` | `ART.md`, the pipeline section; a unit sheet shows all four layers |
| `jane bench --save <slot> --frames 600 [--night] [--wide] [--backend b] [--tier t]` | §1.12; reports per tier the machine can reach |

Sheets and films are artefacts on every CI run, never asserted (`PORT.md` §3.6); the `soft` frames are byte-identical across targets, so a film diff between two commits is a review tool, not a gate. A generator is reviewed by looking at two dozen maps, not by playing two dozen games (`ENGINE.md` §8.2); a renderer is reviewed by looking at a film of the first evening.

## 7. Defaults taken

Each is a one-line edit if the owner flips it before P6.

| Question | Default | Where it bites |
| --- | --- | --- |
| Backends | One `Frame` contract; `soft`, `gl2`, `wgpu` as three crates; `jane-app` probes wgpu, then gl2, then soft; `backend` in `config.json` overrides | preamble, §1.2, §1.3 |
| Canvas | 768 x 432, 16 canvas px per cell, render scale 2; the sim cell and every sim and worldgen constant unchanged | preamble |
| Upscale | Integer where the height divides; else sharp bilinear on T1 and T2 and nearest on T0 with an integer-window toggle | preamble |
| Window | Wider canvas at the same height, no bars; the 48 x 27 guarantees hold for wider | preamble |
| Sprites | Four layers from `jane-art`: albedo (indexed), normal (RG8), emissive, height; people 32 x 40; 2048 x 2048 pages; T0 uploads albedo only | §1.4 |
| Lighting and shadows | Point and spot lights with height and a cast flag; sun and moon directional by the clock; ambient by hour, zone and area; occluders are props, units, wall runs and canopy with height; T0 multiply lightmap and contact shadows, T1 normal-mapped with hard shadows from 8 casting lights, T2 tiled with soft shadows from 32 | §1.7 |
| Materials and atmosphere | Per-cell wetness in the presenter after rain, specular on T1 and T2; water shimmer on T0, refraction on T1, reflection on T2; fog volumes per area by hour and weather; nine depth layers; god rays T2 only; grading per region and hour as tint on T0 and T1 and a LUT on T2; weather read from the view (`WORLD.md`), never rolled here | §1.8, §1.9 |
| Features | Every visual feature is a `Features` row with a tier and a `config.json` key; the degrade ladder is the order they come off | §1.3, §1.12 |
| Gates | T2 modern PC 60 at 4K with headroom; T1 Pi 4 60 at 768 x 432 with 8 casting lights; T1 GeForce 7 class 60 with shadows off; T0 Pentium 4 60 at half res or 30 at full; Pi 3 best effort at T1, not a gate | §1.12 |
| Timers | Ticks, never frames; the `Frame` is a pure function of `View`, presenter state and tier | §1.11 |
| Font | Stroke-defined glyphs on the 5 x 8 lattice, rasterised at boot; Small at 2x cell, 12 x 18; 64 x 24 characters at 16:9 | §3 |
| UI scale | 1x on the canvas grid; the window 640 x 380; widget list unchanged | §3 |
| Seats and folk | Seats are a coat swap only; bounded `vary` variants, at most 4 a row, picked by unit id | §3.6 |
| Aim assist | In the sim; `InputFrame.assist` carries the profile (Off, Pad, Mouse); the reticle draws `View::assisted_aim` | §4 |
| Verification | `jane sheet scene` and `jane film` through `soft` on CI, any backend locally; artefacts, never asserted; `jane bench` per tier | §6 |
| Atlas cache | Allowed on disk under the save directory, keyed by build hash, all four layers together; written, never shipped | boot |
| Build order | Window first (`PORT.md` §7.1): §1 on `soft` and a window before the art is done; chunks as flat swatches (§1.6) and step-1 demo sprites stand in; the prompt, vitals and dialogue box of §3.2 come with P6, the rest of §3 with P7 | §1.6, §3.2 |
| New Game | All 13 zones built up front behind the loading screen; no zone is built on first entry | §3.2 |
| Audio and bindings | `NullBus` ships and procedural audio is outlined for `PLAN.md` M8 only; `data/bindings.json` compiled in, overrides in `config.json` beside the saves | §4, §5 |
