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

jane-render-soft/src/  fb, blit, lightmap, mist, upscale, readback  |  gl2/src/  context, shaders, shadowgeom, targets, upscale  |  wgpu/src/  gpu (device, layouts), prep (Frame to instance lists, lights, tiles), lib (pipelines, targets, shadows, post, present, read-back), shaders/{common, gbuffer, scatter, light, post}.wgsl
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
                   clear: u32, passes: Vec<Pass>, chunks: Vec<ChunkCmd>, sprites: Vec<SpriteCmd>, layers: Vec<ChunkLayers>,
                   lights: Vec<Light>, casters: Vec<Caster> }
pub struct Span { start: u32, len: u32 }                      // frame.chunks_in(s), sprites_in, lights_in, casters_in
pub type Rgb = [u8; 3];                                        // 255 is full
pub enum Pass {                                                // landed (P6 steps 1, 2 and 5; P6c)
  Terrain { chunks: Span },                                    // ChunkCmd { id: ChunkId { cx, cy }, generation: u32, x, y: i32, slot: u16 }
  Sprites { layer: Depth, cmds: Span },                        // already in draw order; Standing is y-sorted
  Silhouettes { sun: Directional, shade: Rgb, casters: Span }, // T0 and T1 only, between Ground and Standing (§1.3 `silhouettes`)
  Lights { ambient: Rgb, fill: Rgb, sun: Option<Directional>, points: Span, casters: Span },
  Post(Post),                                                  // T2 only, last
}
pub struct Directional { azimuth: Angle /* toward it, 0 east, a quarter turn south */, elevation: Angle, colour: Rgb /* on flat ground */, spread: u16 /* angular radius */ }
pub struct Light { pos: (i32, i32) /* its ground point */, height: u8, colour: Rgb /* flicker applied */, radius: u16, size: u8, casts: bool,
                   kind: Point | Spot { dir: Angle, cone: Angle } }
pub struct Caster { sprite: u32 /* index into sprites: its albedo is the silhouette */, foot: (i16, i16), height: u8, depth: u8 }
pub struct Post { tint: Rgb, lift: Rgb, saturation: u8 /* 128 is as lit */, bloom: u8, exposure: u8 /* 128 is 1 */ }
pub struct SpriteCmd { page: u8, src: Src { x, y, w, h: u16 }, x: i16, y: i16, flags: Flags { mirror, tint: None | Flash(u8) | Ghost(u8) }, height_px: u8 }
pub struct ChunkLayers { albedo: Vec<u32>, normal: Vec<[u8; 2]>, emissive: Vec<u32>, height: Vec<u8> }   // T0 fills the albedo alone
pub enum Depth { Sky, FarLandmark, FarTreeline, Ground, Standing, Canopy, NearFog, Weather, Ui }
```

**The light pass carries both halves of the light.** `ambient` is the flat light T0 multiplies by (the clock's keyframes, as before). `fill` is the sky's own light, what a surface in shadow is lit by on T1 and T2: blue by day, violet at dusk, a deep blue-violet at night that keeps its value; indoors the zone's `ambient`, blue-tinted. `sun` is the sun or the moon, its colour the light it throws on flat ground, so `fill + sun.colour` is roughly `ambient` and T0, which has no directional light, loses nothing by folding it in. All of it is `jane_present::light::sky(clock, day, indoor, permille, region)`, integer, so the sun's angle a T0 frame shears its silhouettes by is the same bytes on every target. On T0 the pass is still left out at full ambient; on T1 and T2 it is always there, since noon has a sun.

**Sources.** Every prop the view says is showing (`View::light_showing`), standing on the row its sprite stands on at the height its glass glows (a building's lit windows on its front, 16 px up); every unit glow of the catalog; her lantern (radius 136, at her side the way she faces, 20 px up) when the flat light is below 750 permille. Flicker is `light::flicker`, a 64-step table walked ten steps a second from a place set by the id. The lights whose reach touches the canvas are kept, the nearest to its middle first, up to the tier's `max_lights` (16, 32, 128); T1 lets the nearest 8 cast and T2 the nearest 32.

**Casters** are every unit standing and every prop not flat: its sprite (the silhouette), its foot (a unit's feet, a prop's lowest drawn row), its height and its depth across the ground (a person 5 px, a prop a quarter of its footprint, 4 to 12). Wall runs, roofs and canopy cast from the chunks' height layer on T2 and not at all on T0.

Every command is in canvas coordinates already (the camera taken off); `camera` is there for parallax. The passes still to come keep the same shape, a `Span` into a list the `Frame` owns (`parts: Span`, `cmds: Span` into `ui`), or plain values:

```
  Sky { bands: Span, stars: Span, moon: Option<Moon> },
  Parallax { layer: Depth, factor: Q8, sprites: Span },                                // far landmark, far treeline
  Fog { volumes: Span /* FogVolume { rect, density: u8, colour: Rgb, drift: (i16, i16) } */ },
  Weather { kind: Clear | Mist | Rain | Storm, intensity: u8, wind: i8 },
  Particles { parts: Span },
  Ui { cmds: Span },
```

T1's shadow geometry wants footprint edges; a `Caster` carries the foot, the depth and the sprite's width, which is its box, and `segs` join it when `gl2` lands.

Passes are in draw order and a backend draws them in that order; `Pass::needs()` is the tier its `Features` row asks, and a backend below the tier a pass needs draws what its `Features` row says (§1.3), never something of its own. The `Vec`s inside are reserved once and cleared, never dropped: no allocation after the second frame (§1.12).

### 1.2 The Backend trait

```
pub trait Backend {
  fn caps(&self) -> Caps;                                  // tier, max lights, max casters, max texture, has_readback, has_stencil
  fn upload_atlas(&mut self, pages: &AtlasPages);          // albedo, normal, emissive, height; once at boot, again on a CLUT change
  fn draw(&mut self, frame: &Frame);                       // caches chunks by (id, gen); uploads on a miss
  fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16);   // the canvas as 0xAARRGGBB; sheets, film, bench
  fn stats(&self) -> Option<FrameStats> { None }           // frame times per pass (§1.12); soft and wgpu both give them
}
```

`Caps` also names the backend (`soft`, `wgpu`) for the title bar, F2 and `jane bench`. `AtlasPages` carries each page's four layers (`Page { w, h, albedo: Vec<u16>, normal: Vec<[u8; 2]>, emissive: Vec<u16>, height: Vec<u8> }`); a presenter at T0 packs the albedo alone and `soft` keeps only that.

| Backend | Tier | Runs on | Draws |
| --- | --- | --- | --- |
| `jane-render-soft` | T0 | any CPU; the fallback when no GPU or no driver; `jane serve`, tests and CI | the previous software design: CLUT blits into a `u32` framebuffer, the multiply lightmap, one mist tile, nearest upscale. Full 768 x 432, or **half res** 384 x 216 (a 2:1 decimated atlas built at boot; every `Frame` coordinate shifted down by one) for the Pentium 4 |
| `jane-render-gl2` | T1 | any GPU from about 2006 (GeForce 6, 7, 8; Radeon X and HD 2000; Intel GMA 950 and up where the driver allows) and Raspberry Pi 2, 3 and 4 by GLES 2 through Mesa V3D and VC4 | normal-mapped lighting in a fragment shader, hard cast shadows from 2D shadow geometry, fog layers, particles, tint. No post beyond tint |
| `jane-render-wgpu` | T2 | modern PCs and Pi 4 and 5 by Vulkan 1.1 (V3DV) | everything T1 plus soft shadows, many lights, bloom, colour grading, screen-space water reflection, higher particle caps. **As built** (P6c): wgpu 30, WGSL; a G-buffer, a height field and one light pass with soft shadows traced through it (§1.7), bloom, the grade, sharp-bilinear upscale in `present`, `read_back` of the graded canvas |

**Rule:** a backend is a function of the `Frame` and its own caches. It reads no view, no event and no tuning row; everything it needs to draw is in the `Frame` or was uploaded.

**Rule:** `soft` always builds, on every target, with no feature flag. It is what CI renders and what a machine with no GPU plays.

**Rule:** a backend that cannot draw a pass at its tier draws the row below it in the `Features` table; it never skips a pass silently and never invents one.

### 1.3 Tiers and the Features ladder

`jane-app` probes at boot: request a `wgpu` adapter that reports Vulkan 1.1, DX12, Metal or GLES 3 with the limits T2 needs (T2); else create a GL 2.1 or GLES 2 context through SDL and check for framebuffer objects, eight texture units and a fragment shader (T1); else `soft` (T0). **As built:** `jane-app --backend auto|soft|wgpu`, `auto` by default, asks wgpu for a high-performance adapter compatible with the window's surface whose downlevel capabilities are WebGPU-compliant (compute, storage buffers in the fragment stage); if there is none it prints why and draws with `soft`, and `--backend wgpu` fails loudly instead. `gl2` is not in the probe until it exists. The SDL window lends wgpu its handles through `raw-window-handle` with no unsafe code: wgpu wants a `Send + Sync` handle source and SDL's `Window` is neither, so a zero-sized token lends the handles of a clone of the game window leaked once for the life of the process and kept in a thread-local (`jane-app/src/handle.rs`). The title bar names the backend, the adapter and the GPU frame times. `config.json` under `[present]` carries `backend = auto | soft | gl2 | wgpu` and one key per `Features` row. The probe's result and the rows in force are printed on the F2 overlay and by `jane bench`.

Every visual feature is a row. A row names the tier it needs, its default per tier and its `config.json` key, so T0 and T1 degrade by rule, never by accident.

| Feature | Needs | T0 | T1 | T2 | Key |
| --- | --- | --- | --- | --- | --- |
| Multiply lightmap, additive discs, ambient | T0 | on (built: a quarter-size buffer, cubic falloff, a pool at most half again as bright, weaker by day) | replaced by shader lighting | replaced | none |
| Normal-mapped lighting (N dot L with light height) | T1 | no | on | on (built) | `normal_light` |
| Emissive layer (lamps, windows, orbs, spells glow unlit) | T0 | as albedo | on | on | none |
| Hard cast shadows from shadow geometry | T1 | no | on, 8 casting lights | on | `shadows` |
| Soft shadows, penumbra by distance | T2 | no | no | on (built: traced through the height field, §1.7) | `soft_shadows` |
| Sun and moon as a directional caster | T1 | ambient only | on | on (built) | `sun_shadows` |
| Silhouette sun shadows: each unit's and prop's albedo mask sheared along the sun by its height, tinted by the ambient, soft-edged by one dither step (decided 2026-09-27: shadows are a showpiece on every tier) | T0 | on (built) | on | replaced by the traced soft shadows | `silhouettes` |
| Light count on screen (casting: T1 8, T2 32) | | 16 | 32 | 128 (built: culled per 32 x 32 tile) | `max_lights` |
| Fog volumes per area | T0 | one drift tile | layered, drifting | layered, drifting | `fog` |
| God rays through canopy | T2 | no | no | on | `god_rays` |
| Wetness specular after rain | T1 | no | on | on | `wet` |
| Water | T0 | shimmer pixel | shimmer and refraction | reflection and refraction | `water` |
| Bloom on emissive | T2 | no | no | on (built) | `bloom` |
| Colour grading per region and hour | T0 | CLUT tint (not yet) | tint | 3D LUT; built as the same terms in the shader (exposure, a soft shoulder, saturation, tint, a coloured lift) | `grade` |
| Particle pool; weather particles (rain, storm) a third of it | | 900 | 2000 | 8000 | `max_particles` |
| Sharp bilinear upscale | T1 | nearest | on | on (built) | `sharp` |
| Half-res canvas; frame skip | T0 | options | frame skip only | frame skip only | `half_res`, `frame_skip` |

**Rule:** a row's tier is the truth. A test builds the `Frame` for a night in the town at every tier and asserts that no pass a tier cannot draw is present in that tier's `Frame`, and that every pass present is drawn by `soft` without a panic.

### 1.4 Formats and atlases

| Buffer | Format | Size at 768 x 432 | Why |
| --- | --- | --- | --- |
| `soft` framebuffer | `u32` `0xAARRGGBB`, one `Vec<u32>` | 1.3 MB (332 KB at half res) | linear sweeps; SDL streaming texture takes it as is; the light pass is a per-channel multiply |
| Albedo pages | 16-bit master-palette indices (`jane_art::palette::Ix`) into one CLUT `[u32; 1024]`, 2048 x 2048 | 8 MB a page; the whole set ~19 MB, the only pages `soft` holds (`ART.md`, the pipeline section) | one lookup; coat swaps, ghosts and dead ramps are free; on the GPU the CLUT is a 1024 x 1 texture and an albedo texel is two bytes (`LUMINANCE_ALPHA` on GLES 2), the index `lo + 256 * hi` |
| Normal, emissive, height pages | RG8 tangent-space; 16-bit emissive index (an `Ix`, 0 is dark); 8-bit height; the albedo's layout (on T2: `Rg8Unorm`, `R16Uint`, `R8Unorm` array textures beside the `R16Uint` albedo, the CLUT a 1024 x 1 `Rgba8UnormSrgb`) | 5 bytes a texel together | generated by `jane-art` from the shape primitives a look used, never painted; lamps, lit windows, orbs and spells glow unlit; height gives shadow length and water (a person is 40, a barrel its `rise`) |
| Terrain chunks | four layers, 256 x 256 px: albedo `u32` resolved, normal RG8, emissive 16-bit `Ix`, height 8-bit; plus strips | 576 KB a chunk on a GPU, 256 KB on `soft`, and the strips (0 on open ground, up to about 1.1 MB in a dense wood: or none, drawing `placed` flora from the atlas); LRU 48 | the ground is the biggest draw (§1.6) |
| `soft` light buffer | `[u16; 3]` per cell at 1/4 resolution (192 x 108) | 124 KB | §1.7 |
| T1 light and shadow targets | RGB8 at 1/2 resolution; one 8-bit shadow mask | 400 KB and 83 KB | §1.7 |
| Mist tile | 8-bit alpha 256 x 256 | 64 KB | §1.9 |
| UI panels | `u32`, cached by `(w, h, style)` | small | `ART.md`, the chrome section |

**Why 16-bit.** The master palette outgrew 256 entries at `ART.md` §8 step 1 (27 fixed and 40 ramps of 8 is 347; the ceiling is 1024, `ART.md` §2.7). A CLUT per page of 256 would work only while every page's sprites used 254 colours between them, and a coat swap would become a remap per page; a `u16` index is one table for every page, costs one more byte a texel, and keeps the CLUT (4 KB) in L1 on `soft`.

CLUT index 0 is clear, index 1 is the contact shadow, a cool multiply of what is under it (`jane_art::palette::AO_TINT`, about `dst x (0.65, 0.67, 0.80)`), softened at its edge by how much of each pixel's 3 x 3 the index-1 mask covers (`palette::ao`; `ART.md` §2.7); every other index is opaque. There is no per-pixel alpha in an albedo sprite: index 1 or a checker stands in for it. On a GPU the four pages of one layout are bound together, so one sprite quad reads all four in one fragment shader; on `soft` the normal, emissive and height pages are never uploaded.

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

**As built (P6 step 2, the painter wired 2026-09-27).** `jane_present::terrain` runs the painter on the presenter's thread in `tick()`, over a `TileSource` on the `View` (`tile`, `size`, `indoor`, and `material` from `View::paint` kept a byte a cell per zone), for every chunk under the view and 64 px round it, and copies the albedo into the slot, and at T1 and T2 the normal, height and emissive (resolved to `0xAARRGGBB`, 0 where nothing glows), so the wgpu tier lights the ground and its walls. The painter runs in `Standing::Placed`: its 43 flora sprites are packed into the atlas once at boot, each chunk's placements are kept per slot, and `draw` puts them into the y-sorted standing list by their feet with a silhouette `Caster` (a tree's depth is its trunk's, a shrub's or a stone's its spread); fences and low walls are in the ground with the walls. At most two chunks land in a tick (`LAND_PER_TICK`), on screen and nearest the middle first; a chunk on screen with nothing to show yet takes its flat swatches (`stand_in::paint_chunk`) until the painter reaches it, a stale one keeps drawing what it had, and the tick a zone is entered paints everything it shows outright (the load, not a frame in play). `Event::Tiles(rect)` invalidates the rect grown by `REACH`. A caster *segment* has no `Frame` type: T2 traces walls through the height layer, and the chunk's `casters` stay in `jane-art` until a tier wants them. The 48 slots' layers are made when the presenter is (12 MB on `soft`, which gets the albedo alone; 34 MB with the four layers of T1 and T2) and live in the `Frame`'s `layers`, so walking into new ground never allocates; a `ChunkCmd` names its slot and its `generation`, which is bumped by every paint. Measured on the owner's desktop in release, a bot walking the county: the worst `tick()` 5.5 ms (two chunks landing), the zone's first tick 17 ms, the app at its 144 Hz vsync with a 2 ms `soft` draw. For the lit tiers the stand-in swatch paints a relief with it: a wall is a face rising from the row it stands on (three cells at most) and looking south, a roof is pitched east-west with its eaves on that face, a wood's canopy is lumpy and 30 to 44 px tall, hedges and fences stand.

### 1.7 Lighting and shadows

The reference is the Elysian Shadows look: a pixel-art county whose lamps throw real pools with real shadows and whose walls catch light from the side they face.

**Sources, all from the view.** Prop lights through `View::light_showing(&Prop)` (THE rule, `ENGINE.md` §6, shared with the sim so what she sees lit is what a sentry sees lit); unit glows; her lantern, radius 136 canvas px, when ambient is below 750 permille; effect lights from `fx`; spells. Every light carries colour, radius, falloff (a 256-entry LUT on T0, the same curve in the shader), height above ground, a cast-shadow flag and a kind (point, or spot with a direction and cone: the sentry's eye, the lighthouse). Flicker from a 64-entry table indexed by `(tick + h32(id)) * rate`.

**The sun and moon** are one directional light whose angle follows the clock: azimuth east to west over the day, elevation from the hour, so at 17:00 when Jane steps off the train the shadows are long and point east, and by 18:30 when the lamps come on the sun's share is gone. The moon is the same light at night, dim and blue, with a phase from the day. Directional shadows are cast by every caster on screen at once, which is why T1 caps casting *point* lights and not the sun.

**Ambient** by hour from the clock's keyframes outdoors; a zone's `ambient` indoors, blue-tinted; **per-area overrides** from the skeleton's areas the blueprint carries (the Works darker at noon under their soot; the lake shore brighter under open sky), each a row with a colour and a permille per hour band. Day is free on T0: at ambient ≥ 254 the multiply is an exact no-op and the pass is skipped.

**Occluders.** Props with `rise`, units (a person is 40 tall), wall runs (from `TileStyle.height` and `wall_like`, merged into segments per chunk when the chunk is painted) and canopy trees. A `Caster` is its footprint's edges at ground with a height; casters are gathered once a frame inside the light reach (208 canvas px past the view) and shared by every light.

| Tier | Lighting | Shadows |
| --- | --- | --- |
| T0 `soft` | the previous design: a light buffer at 1/4 resolution cleared to ambient, every light **added** as `colour * LUT[(d2 * 255) / r2] * intensity` over its disc, upsampled bilinear (or nearest plus Bayer under `light_steps`) and multiplied `dst = (dst * L) >> 8`. No normals; emissive texels are drawn as albedo at full brightness. **As built**: the LUT is `(1 - d2/r2)^3`, a pool lifts a pixel to at most 1.5x, its gain falls by day, and flame light leans warm as on T2 | the contact shadow (index 1) and the **silhouettes** of the sun or moon: each caster's rows laid on the ground `h * cot(elevation)` px away from the sun, stretched to meet the next row's and as thick as the caster is deep, gathered in a coverage mask so two never darken twice, fading toward the tip, multiplied toward the sky's `shade`, the edge px at five eighths and a ring outside at three eighths through the 4 x 4 Bayer. Integer |
| T1 `gl2` | per light, a quad over its disc; the fragment shader reads albedo, normal and emissive, computes N dot L with the light's height as z, applies the falloff and the shadow mask, and adds into a half-res light target; ambient and sun first; one multiply pass at the end; emissive added unlit | **hard**: for each casting light and each caster in its disc, every footprint edge facing away from the light is extruded to the disc's rim scaled by `height`, drawn into the shadow mask with the stencil where the context has one and by alpha accumulation into a small FBO where it has not; the light's quad reads the mask. 8 casting lights, then the nearest 8 by rule |
| T2 `wgpu` | the same terms in WGSL over a tiled light list (128 lights, culled per 32 x 32 tile on the CPU, 64 a tile at most), one pass: the fill, the sun (N dot L over its elevation's sine, so its colour is its light on flat ground, capped at 2.5x on a face turned to a low sun), each point light by N dot L with its height as z and a windowed inverse square, emissive added unlit; the height field's ambient occlusion darkens the ground at the foot of what stands | **soft**: traced through the height field (below) from every lit pixel toward the sun or moon and each of the 32 casting lights, the penumbra widening with the distance from what casts it, tinted by the fill |

**The T2 shadow technique: a height field seen from above, traced with a clearance penumbra.** The G-buffer (the canvas and a 64 px guard band round it, so a caster just off screen still casts) holds each pixel's albedo, normal, emissive, height and depth. In the 3/4 view a pixel `h` px up at `(x, y)` stands on the ground at `(x, y + h)`, so a compute pass stands every lifted pixel on its ground point in a buffer the canvas's size, the tallest winning, a px wider each side and as many rows deep as the caster is: an upright sprite's columns land on its feet and become a thin wall as tall and as shaped as it is, and a roof lands on the house under it. The light pass then marches from each lit pixel (from the front of its body when it stands) toward each light across that field, the ray rising as the light is high, and keeps `min(k * clearance / t)`: `t` px from the pixel, how far the ray clears the field there, `k` the light's distance over its size (or `1 / tan` of the sun's spread, 2 to 5 degrees, widest at dusk). Steps are 1 px near and up to 3 far, a long step reading the tallest of four texels so a thin post is never stepped past, a short one bilinear so a penumbra has no stairs. **Why this and not 1D polar maps or SDFs:** it casts from what the art drew (every px's true height, `ART.md` §1.1) rather than from footprint segments, so a person's shadow is her silhouette, a canopy's is lumpy and a fence throws a comb; the sun is one more light, not a directional map to place; the penumbra comes free and widens with distance as a real one does; the terrain casts with no geometry of its own; and it is one compute pass and one fragment pass at canvas resolution, independent of the output's 4K. Its limits: what is hidden behind something else on screen does not cast, and nothing off the guard band casts in.

**Rule:** what is lit is decided by the view (`light_showing`), the tier decides only how it looks. A lamp the sim says is lit is lit on every tier; a lamp the sim says is dark casts nothing anywhere, and its glass is dark.

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
| Walk frame | a person: `1 + (anim_ticks - 1) / 6` of the six-frame cycle, mod 6 (`jane_present::people::pick`); standing, the standing frame and the breathe in turn every 40 ticks, out of step by unit id; a stand-in: a 1-px bob, `(anim_ticks / 9) & 1` |
| Creature frame | as a person's walk and breathe (`jane_present::creatures::pick`); stood still 90 ticks, the idle pair (the dog and the cats sit and look at you, a hen pecks), each beat 48 ticks, facing the viewer and never mirrored; on a `Cast` event, the attack's three beats, 4 ticks each, for a creature that fights; dead, its dead pose |
| Prop frame | `open` (looted or used) over `on` (thrown, pressed, or a light the view says shows) over a base picked by the prop's id (`jane_present::props`); a lit prop's light shines from the height its glass glows at; a building's `on` frame lights its windows |
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

**Measured (2026-09-27, T2).** `jane bench frames --backend wgpu` on the owner's desk (RTX 3060, Vulkan), the town at 22:00 with 7 lights, 768 x 432 upscaled to an offscreen 3840 x 2160: the GPU's own clock p50 1.84 ms, p99 2.40 ms (the light pass 1.6 ms of it, the height field 0.02, the G-buffer 0.04, bloom and grade 0.05, the upscale 0.12); the whole frame to its last pixel p50 2.2, p99 2.8 ms; at 17:00 p99 2.9 ms; at 21:9 (1008 x 432 to 5040 x 2160) p99 3.1 ms. The gate is p99 < 6 ms. `soft` on the same machine: 1.5 ms p50, 1.8 p99 at 768 x 432, the silhouettes 0.5 and the lightmap 0.9 of it.

**`FrameStats`** (`jane_present::backend`) is what F2 and `jane bench` read: the whole frame's last time, p50 and p99 over 120 frames; the same per pass in `[u32; 13]` arrays keyed by `StatPass` (sky, parallax, chunks, water, list, fx, shadows, light, fog, weather, grade, ui, upscale; a pass a backend does not split reads 0); whether the clock is the GPU's; the last frame's draw calls, lights, casters and px written (`soft`). `wgpu` writes timestamp queries at every pass boundary and reads them a frame or two late, the upscale in `present` with its own; without `TIMESTAMP_QUERY` it keeps the CPU's encode and submit. `soft` times each pass on the CPU.

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

Immediate mode into the `Ui` pass at 1x on the canvas. **The design grid is the canvas** (768 x 432 at 16:9, wider on a wider window; never a second layer at another scale): the Small face is the stroke font at 2x cell, 12 x 18, which gives 64 columns by 24 rows at 16:9, and widgets anchor to edges, so a wider window spreads the HUD and leaves a centred window centred. Head is 18 x 27, Title 30 x 45 (`ART.md`, the font section). Dense text (lists, tooltips, the tracker, the terminal, the overlays) is the Fine face, 8 x 12.

**Built 2026-09-27** (the whole of this section; what differs from the first design is said where it differs). Everything lives in `jane-present::ui` (`core`, `cmd`, `art`, `icons`, `style`, `hud`, `dialogue`, `menus`, `window`, `map`, `title`, `loading`, `console`, `controls`, `perf`, `world`), `jane-present::view` and `::text`, and in `jane-app` (`app.rs` the scenes and the loop, `saves.rs`, `config.rs`, `console.rs` the terminal's rows, `script.rs`).

### 3.1 Core

**The `Ui` pass** is a list in the `Frame`, `Frame::ui: Vec<UiCmd>`, drawn after every pass (after `Post` on T2), unlit, never graded, bloomed or fogged, plus `Frame::ui_images: Vec<UiImage>`, the UI's run-time pictures by slot. The contract every backend keeps is the module doc of `jane-present::ui::cmd`:

```
enum UiCmd {
  Fill   { dst: Rect, argb: u32 },                                        // one colour, blended by its alpha
  Sprite { page: u8, src: Src, dst: Rect, ink: u16, alpha: u8, mirror: bool }, // atlas texels through the CLUT; ink != 0 recolours every opaque texel
  Image  { slot: u16, src: Src, dst: Rect, alpha: u8 },                    // Frame::ui_images[slot], 0xAARRGGBB, its own alpha times `alpha`
  Clip(Rect),                                                              // a scissor for every later command
}
struct UiImage { generation: u32, w: u16, h: u16, argb: Vec<u32> }         // a backend re-uploads on a new (slot, generation)
```

Rects are canvas px (`i16`). Texel 0 is clear, texel 1 (the contact shadow) darkens to three quarters, any other texel is its CLUT colour. A sprite whose `dst` differs from its `src` stretches nearest by integer division (`src.x + dx * src.w / dst.w`). Blending is `under + (colour - under) * a / 255` **in the stored sRGB bytes**; `soft` is the reference (`jane-render-soft/src/ui.rs`), and `wgpu` matches it by drawing through a plain (non-sRGB) view of the canvas (`jane-render-wgpu/src/ui.rs`, `shaders/ui.wgsl`, checked by a headless read-back test). `gl2` draws nothing of it yet.

**The UI page.** Every glyph of the four faces (regular and bold, packed as one-bit masks drawn with an ink), the cooldown sweeps (48 steps, 36 and 20 px), the marks (arrow, reticle, hand, sun, moon, the pad's buttons, diamond, skull, lock and a few glyphs) and the item, spell and status icons are packed at boot into one atlas page the presenter appends last (`ui::art`, `Present::ui_art`), so a backend that uploads `Present::atlas()` has it. The icons are **stand-ins** drawn from their names by `ui::icons` (a brass key, a red flask, a frost medallion) until `ART.md` §2.5's icon family lands; nothing else knows.

**Chrome** is composed of fills, not generated canvases, so a panel of any size costs a handful of commands: a banded two-tone body (`ui_panel` mid to shade, 90 % to 96 %), a `k` rim with its corners cut, a lit inner top edge and a shaded bottom, gold studs and a gold hairline on a window. Colours are palette indices throughout (`ui::style`: text, quiet, dim, gold, and good, warn and bad for the overlays), so the chrome shifts hue as the world does.

```
pub struct Ui { art, input: UiInput, tick, canvas, interactive, nav, cmds, out, hot, active, drops, drag, tooltip, popover, images, reticle, … }
impl Ui {
  fn begin(&mut self, input: UiInput, tick: u32, canvas: (u16, u16)); fn finish(&mut self, frame: &mut Frame);
  fn panel(r, PanelStyle /* Hud | Window | Tip | Debug */); fn text(x, y, &str, Ink) -> x; fn wrapped(r, &str, Ink) -> lines;
  fn wrapped_reveal(r, &str, Ink, shown) -> lines;       // the whole text laid out; only `shown` chars drawn
  fn button(id, r, label, ButtonKind /* Menu | Chip | Tab */, enabled, focused) -> bool;
  fn slot(id, r, &SlotView, drag: Option<DragPayload>, target: DropTarget, focused) -> SlotOut;
  fn bar(r, frac, lag, Ramp); fn well(r, lit); fn rule(x0, x1, y, Ix); fn focus_ring(r);
  fn text_field(r, &mut String, max, Ink) -> FieldOut; fn popover(id, &[&str]) -> Option<usize>; fn tip(id, r, |s| …);
  fn image_mut(slot, w, h) -> &mut UiImage; fn command(Command); fn intent(AppIntent); fn take_drop() -> Option<(DragPayload, DropTarget)>;
}
enum UiOut { Command(jane_sim::Command), Intent(AppIntent) }
enum AppIntent { NewGame { name }, Continue, Load(u8), Save(u8), LoadMenu, SaveMenu, ToTitle, Quit, Resume, Pause, Controls, Back,
                 Rebind { row, col }, ResetBindings, Assist(AssistProfile), Console(String), OpenWindow(u8), CloseWindow }
```

`WidgetId = wid(name, index)` (FNV-1a). `UiInput` is device-free (pointer, the primary button held, pressed and released, right and middle presses, the wheel, the navigation presses, typed text, keys and pad inputs pressed this frame, whether the pad was last); the app fills it, a test fills it by hand. The app drains `UiOut` each frame: a `Command` goes to the session stamped like any press, an `Intent` to the app. **The UI never calls into the sim.** Layers, bottom to top: F3's world overlay, HUD, dialogue, window, the menus (pause, slots, confirm, Controls), the terminal, F2 and the top line; the app sets `Ui::interactive` for the top one only. The pointer over any panel drawn this frame or the last is the UI's (`wants_pointer`): a press there never reaches the world. Text wraps greedily on spaces (an explicit newline breaks); a dialogue line's unrevealed tail is laid out invisibly so the box never reflows as it types. The OS cursor is hidden and the UI draws its own: the reticle in play (§4), a pixel arrow over the UI, a hand while dragging.

### 3.2 Widget list

| Screen | Holds |
| --- | --- |
| Title | **As built** (a departure recorded here): the backdrop is painted once per canvas size into a UI image, not drawn through the `Sky` and `FarLandmark` passes: the night sky in dithered bands from indigo to a low ember glow, stars, the moon with a halo, the far treeline, the School on its hill (block, wing, bell tower with an open arch, two rows of lit windows), its light drawn down the lake in broken streaks, reeds on the near bank. Over it each frame: one window flickering, mist bands drifting at their own speeds, fireflies pulsing. JANE in the Title face, bold, gold lit from above in three bands with a `k` outline and shadow; "The bell rings at nine" under a gold rule. New Game opens a name field (16 characters, `clean_name`) with Begin and Back; Continue and Load are live only when a slot holds a save. Host and Join (P8) open their screens below |
| Loading | New Game builds on a thread the app owns; the loader first sends the skeleton (`jane_world::skeleton::skeleton(seed)`), which becomes a card at 2x (land by biome and height, river and lake, roads (lit ones gold) and rail, small places, patches as dotted rings, sites as diamonds, dungeon mouths red), each layer swept in top to bottom under a lamp-coloured line, 20 ticks a stage, then one mark per zone; the stage's name in the Small face and five stage dots beneath. Play starts when the build is done and the stages have shown (about two seconds). Load and Continue show the same card when `config.json` knows the slot's seed (written with the save); else the card is blank and the stages still run. Input is ignored; there is no cancel |
| Load, Save | Three wells from the slots' headers without decoding (`jane_sim::save::read_header`): the zone, "Day 3, 21:00", "HP 34 of 40", how long ago, and "latest" on the newest. Empty or unreadable is "Empty". Loading an empty slot is not offered; saving over a full one is one pick |
| Controls | One row per action (28, scrolled): the label, two keys, the mouse button, the pad input. Pick a cell and press: a key for the key columns, a button for the mouse, a pad input for the pad; Backspace clears; Esc keeps it. A clash shows in red with who else has it, never refused. Reset all restores `data/bindings.json`. Beneath: the aim assist (Auto, Off, Pad, Mouse) and the backend (auto, soft, wgpu, taking effect on the next start). All kept in `config.json`. Not built: the integer-window toggle and the per-`Features` toggles (no `Features` table exists yet) |
| Pause | Resume, Save (live only when `MeView.can_save`, and saying so beneath: "Save by a bed or a fire"), Load, Controls, Quit to Title (which asks). The world dims under it. Solo it holds the sim (the app stops stepping it); with company nothing would hold (not reachable before P8) |
| HUD | Vitals top-left: her name, hp as numbers, hp, mp and energy bars (a lit lip, a shaded foot, quarter ticks) with a damage-lag tail in the bar's own pale tone that holds 30 ticks and falls, the hp bar pulsing under a quarter; up to 8 status chips under the plate with their sweep, a red underline on the harmful. The target frame top-centre while she and a hostile trade blows (600 ticks), a skull on a boss. The zone's name (the county by region: The Lowfields, The Waters, The Works), the clock, the day and a sun or moon top-right, with the hour as a mark along the plate's foot. The tracker: the first four quests, each at its first unfinished step ("Back to …" in gold when ready), on a dark band with a gold edge. The prompt above the bar: the Use key's cap (or the pad's button) from the bindings table, then "Verb: Label", "(hold to push)" when it also moves. Bag, Book, Log, Map and Menu chips bottom-left, pointer only. The zone banner in the Head face with gold rules growing out of it, fading, "Night" beneath after dark. The death veil: a cold violet wash deepening to the edges, "{name} fell", "Waking in N" with a skull. Toasts above the prompt: up to three, 180 ticks, rising in and fading out, the same words merged with "x2", gold for a gain, red for a refusal. The 8-slot bar: 36 px wells, the icon, the stack count, the key or pad label, the cooldown as a dark conic sweep and the GCD as a pale one, a gold flash on a cast and a red one on a refusal. Under an open window the plates it covers step aside; the bar stays, as a place to drop |
| Window | One panel, 640 x 350 (**not 380**: the bar below stays visible and a drop target), four tabs: **Bag** (the 8 x 3 bag; the craft strip, three inputs, the output and Make, when `at_bench`, else a line saying a bench is wanted; the stats card: strength, spirit, health and mana of their maxes, the day, falls, and a tally), **Book** (what she casts: her row's own book and what the party has learned, each draggable to the bar, its key shown when bound; the lit one whole on the right), **Log** (the quests, active then done, ◆ ready and ✓ done; the lit one's text and its steps with their counts), **Map** (§3.5). The tab is called Log, not Quests: `VOICE.md` rule 8. Holds the world solo |
| Popover | On a bag slot (right click, or a press on a pad): Use (when usable), Put on the bar, Destroy. Destroy asks once |
| Tooltip | After 20 ticks under the pointer; 40 columns; the name in gold, then what it costs, when it can be used again and what it does |
| Dialogue | Speaker on a plate over the box's top edge; the line revealed at 3 characters a tick; at most 2 options with their number caps, lit by pointer or keys; ▼ bobbing while another line follows, ■ at the last; the device's hint ("E Next", "E Close", "1, 2 or E Choose"). A press mid-line shows the rest; the next sends `Advance`, an option `Choose` (what `jane-bot` sends), at most one command a line; back (Esc, B) sends `CloseDialogue` |
| Terminal | Backquote; slides over the top third; a ring of 400 lines (long ones wrapped), a history of 100 (up, down), page keys and the wheel scroll, Tab completes a row and then its argument (items, spells, units, quests, zones). Every line becomes `AppIntent::Console`; `jane-app/src/console.rs` runs it, and every mutation is a `Command::Dev` (`ARCHITECTURE.md` §1). The rows are `ENGINE.md` §12's less `replay` (headless: `jane replay`), plus `clear`. `join [token]` sits an idle body down in an open world and `leave <seat>` gets a seat up (a guest's is hung up on): the host's, or alone; a guest is told only the host seats people |
| Host, Join | **As built (P8, `ui/lan.rs`).** Over the title, in the menus' window panel and gold heading, choices as tab buttons (left and right step the lit row). **Host**: the world (New, or Slot 1 to 3 where a slot holds a save, its zone and day beneath), the door (Open, Closed), seats (2, 3, 4, with the four coats beside, lit up to the count), input delay (2 to 6 frames, in ms beside), if one stalls (Drop at 10 s, Wait); the port others dial; Host and Back. Host builds or loads the world behind the loading card and opens it to the LAN once built; a rest by anyone at the table writes the host's slot. **Join**: the hosts on this network (a UDP ask every second: name, address, seats taken; a host this build cannot join is greyed and says so), an address field (the last joined offered again), and beneath it what a join is doing (knocking, building the host's county, waiting for the world) or why it ended; a refusal shows both content hashes (or both builds). Join becomes Cancel while knocking. **Pause** gains "Open to LAN" alone and "Hosting on port N" (greyed) when hosting, and none when joined; with company a line over it says the world does not stop. **HUD** at a table: under the vitals a plate of the four coats, lit for each seat taken, hers underlined, with "hosting" or "joined"; a stall banner in the top third while the table waits (the awaited coats, how long, what happens at 10 s or that the host waits); toasts as coats sit down and get up ("The teal coat sat down", "Two at the table: each of us is weaker", "Alone again: as strong as ever"), read from the sim so every machine says the same; a desync's report to the terminal and a toast. A guest cannot save (the world is the host's) |
| Debug | **F2** cycles off, compact, full. Compact, bottom right: fps, the last frame's ms, p50 and p99 over 120 frames, the sim's tick µs, tier and backend, dropped ticks; the border amber near the 16.7 ms budget and red over it. Full, top right: the last 240 frames as stacked bars by stage (sim step, presenter tick, frame and UI build, backend draw, the wait for the display) with budget lines at 16.7 and 8.3 ms and a red tick over each hitch; the sim's tick graph with p50, p99 and max; the per-pass table (sky, parallax, chunks, water, list, fx, shadows, light, fog, weather, grade, ui, upscale: p50 and p99 from `Backend::stats`, the GPU's clock where it has one, "-" where a backend does not split a pass); units awake of all, path searches a tick and nodes each, events a tick, and the per-phase times of `Sim::metrics()` ("n/a" until the sim has it); the frame's list, lights, casters, chunks live and painted, UI commands, atlas pages, draw calls and px written; the last hitches over 20 ms with the stage that dominated and their tick; the passes in force. **F3** draws over the world, under the UI: solid, prop-solid, water, sight-blocking and occupied cells; chunk borders with their generation and slot; units with id, row and state, their path, their target, aggro and leash rings; triggers and plates with their names and whether fired; lights with their reach and height, casters, the sun's way; unseen fog blocks; named rects; props with id, row and state; the zone's edge and the camera's lock. While it is up 1–9 toggle the layers (the legend says which are on), and the pointer over a unit, prop or tile opens its fields. Either shows a top line: seed, zone, tick, the clock, the state hash (taken every half second). **F6** holds the world, and steps it one tick a press while held; **F7** a quarter speed, **F8** four times; the sim stays deterministic, only the ticks a frame change |

### 3.3 Focus and navigation

Each screen keeps its own focus (a menu's row, the bag's slot, the book's and the log's line, the Controls cell). The last device decides whether the ring is drawn (`Ui::nav`: a pad, or the keys in a screen) and which hints show; a menu's lit row shows whatever the device, since its rows follow the pointer. The UI-mode key map swap of the TS input layer carries: with a screen up the move keys are cursor keys, Use and Enter confirm, Esc and the pad's B go back, LB and RB change tab.

### 3.4 Drag and drop

```
struct Drag { payload: DragPayload /* Bag | Craft | Bar | Spell */, origin: DropTarget, start: (i32, i32), moved: bool, ghost: Option<SpriteId> }
enum DropTarget { Bag(u8), Craft(u8), Bar(u8), BarBackground, Window, Outside }
```

Arm on press over a slot that holds something; start once the pointer has moved 4 canvas px; the ghost follows at 70 %; every widget pushes its `(rect, DropTarget)` in the frame and the last hit of the previous frame's under the pointer wins on release; a press and release without the move is a click. The result is one command: `BagMove`, `CraftPut`, `Bind` (bag or book to the bar), `BarSwap`, `Unbind` (the bar off itself), `CraftClear`; a bag thing dropped outside every panel asks "Destroy the …?" and sends `BagDestroy`. On keys or a pad, confirm picks a bag slot up and confirm puts it down where the ring is. Right click, or a press on a pad, opens the popover.

### 3.5 Map tab

The chart algorithm of the TS map tab carries whole, on `u32` buffers (`ui::map`). Outdoors 1 chart px = 4 x 4 cells (the 2000 x 2000 county is 500 x 500); weighted inks (ground 1, water 2.5, road and rail 4, roofs 6, a solid prop of 20 cells and over counting as roof by its area); a 3 x 3 majority on ground inks; woods stippled; roads, water and roofs never smoothed away. Indoors the commonest tile per block, greyed and warmed onto a coarse ramp. Ink colours are palette ramps (grass, cloth green, reed, moss, mustard, plaster, stone, water, brick; `MapInk` rows are not yet in the palette).

`MapChart { terrain, seen, composed }` per zone, painted once per zone from the view's tiles and props; the seen quarters are read every 30 ticks (`View::seen`, one look per chart px outdoors, where a fog block holds four) and `composed` is recomposed only when they changed: unseen ground is a four-step smoke of two-octave value noise, its border two dithered steps of thinning haze; unseen indoors is nothing. Zoom fit, 1, 2, 3, 4, 6 by the wheel or +/-, nearest; drag or the arrow keys pan; 0 recentres on her. The School's mark is a gold star at the mark whose name holds "school"; she is a gold dot that blinks. The chart is a `UiCmd::Image` and never lit.

### 3.6 The view buffers

`jane_sim::view::View` (`ARCHITECTURE.md` §11) is the sim's contract. `jane-present::view::ViewBuffers` is walked from it once a **tick** (not a frame: the toasts of every tick land, and a held world still fades them), with that tick's events, into what the HUD, the dialogue box and the window draw:

```
pub struct ViewBuffers { tick, seed, heroine: String, me: MeView, hud: HudView, window: WindowView, dialogue: Option<DialogueView> }
MeView { dead, respawn_ticks, can_save /* View::near_rest */, at_bench /* View::near_bench */, frozen, the_end }
HudView { hp, mp, en: Gauge { now, max, frac, lag }, statuses: Vec<StatusChip>, target: Option<TargetFrame>, zone, zone_name,
          clock: String, clock_ticks, day, night, tracker: Vec<QuestLine>, prompt: Option<Prompt { verb, label, hold }>,
          bar: [SlotData; 8], banner: Option<(&str, since)>, toasts: Vec<Toast { text, tone, born, count }>, party }
WindowView { bag: Vec<SlotData> /* 24 */, craft: [SlotData; 3], craft_out /* View::craft_output */, at_bench, stats: StatsCard,
             book: Vec<SpellRow { id, bound }>, quests: Vec<QuestRow { title, body, steps, ready, done }> }
DialogueView { speaker, text /* expanded */, options, choosing, more, key /* a new key restarts the reveal */ }
```

The scene's share (units, props, lights, the sky) is the presenter's own records (§1.11), not a `SceneView`. Strings and vectors are cleared and refilled, not dropped; a new toast is the one allocation of a steady frame. No reference into `GameState` survives the call; the only thing that leaves the UI is a `Command`. `View` gained read-only accessors for it: `text(TextRef)` (a zone's generated words), `party()`, `seen(cx, cy)` and `fog_block()`.

**Rule:** a rule the sim reads (what is lit, what USE would do, whether she may save, the bench, what the craft row makes, where the assisted aim points) is read through `View`, never re-derived in `jane-present`. The view is tested in the sim's suite; the buffers are tested here for shape and timing (the lag holds then falls, toasts merge and age, a cooldown rounds up, a new game reads).

### 3.7 Text

The sim has no English (`ARCHITECTURE.md` §0). `jane-present::text` owns every string the UI adds and everything done to content's:

```
fn expand(s: &str, heroine: &str, seed: u32, out: &mut String)   // {name} → heroine; {place:<story>} → jane_world::names::story_name(seed, id)
fn say(v: &View, r: TextRef, out)                                // a content or a zone's own text, expanded
fn toast(v: &View, kind: &ToastKind, out) -> Tone                // one row per ToastKind: Plain, Good or Refused
fn spell_error(e: SpellError) -> Option<&str>                    // None for the quiet ones (GCD, cooldown)
fn verb(v: Verb) -> &str                                         // Enter, Try the door, Unlock, Open, Pick up, Craft, Read, Use, Hold to push, Take, Talk, Put down, Custom(TextId)
fn zone_name(zone, region) -> &str; fn clock(ticks, out); fn span(ticks, out); fn clean_name(raw) -> String
```

`KillProgress` reads "Rats 3 of 5"; `Needs` reads "Needs wood x2"; the words are written to `VOICE.md` (plain, first person where she speaks: "My bag is full", "I should keep that"). Numbers are formatted by integer helpers; nothing here calls a float formatter. **Rule:** a `ToastKind`, `Verb` or `SpellError` variant without a row is a build error: every match is exhaustive.

## 4. Input

`jane-app` turns SDL2 events into device state; `jane-present::input` turns device state into two outputs and nothing downstream knows which device it was (`ENGINE.md` §10):

```
InputFrame { mv_dir: Angle, mv_mag: u8 /* 0..127 */, aim: Option<Angle>, sprint: bool, use_held: bool, assist: AssistProfile }   // ARCHITECTURE.md §3.4 plus one byte, one per tick
enum AssistProfile { Off, Pad, Mouse }
enum Edge { Ui(UiAction), Game(GameAction) }                                                                                     // queued on press
```

**Aim assist lives in the sim** (`ARCHITECTURE.md` §5), not here, so it is deterministic and replays: the client sends the raw aim and the profile, and the sim's tuning rows per profile set the cone, the magnetism and the stickiness. The reticle draws the assisted aim the view reports: the app asks `View::assisted_aim(frame, spell)` for the frame it is about to send and the first spell on her bar, and draws the reticle along it at the cursor's distance from her chest, with a faint dot at the cursor itself when the assist has pulled away from it; what she sees is where the bolt goes. The profile is a Controls row (`None`, the default, is `Pad` while the pad aims and `Off` for a mouse).

Move and aim vectors are turned into `(Angle, magnitude)` with floats here and quantised; the sim never normalises. Keyboard by `Scancode`; mouse from window px to canvas px through the window's `s`, aim from the chest (12 canvas px above the feet); right button held walks toward the cursor with strength `dist / 64`, the 2020 virtual stick; left button is bar slot 1 unless the pointer is over the UI; the wheel zooms the map, scrolls the terminal and the Controls list. `SDL_GameController` with the standard mapping: left stick or D-pad move, right stick aim, A X Y LB RB bar 1 to 5, B use, RT sprint, View bags, Menu pause. Text entry through `SDL_StartTextInput` while a text field or the terminal has the keyboard (`Mode::Text`: only Esc and backquote act). The OS cursor is hidden; the UI draws the pointer.

**Bindings are data.** `data/bindings.json` (one row per action: `{ "action": "use", "keys": ["E", "F"], "mouse": null, "pad": "B" }`) is compiled in by **`jane-present`'s build script** against one table of names (`src/input_names.rs`: actions with their Controls labels, key caps to scancodes, pad inputs, mouse buttons), which `config.json`'s overrides read too; an unknown name or an action without a row is a build error. It is presentation, not content, so it sits outside `jane-data`'s catalog and its content hash (decided 2026-09-27). User overrides live in `config.json` beside the saves, one row per changed action with only the changed columns (`"none"` unbinds a pad or mouse column), applied over the compiled table at boot into the `Bindings` in force (`Input::bindings`). The Controls screen rebinds by press-capture; conflicts are shown, not refused; the prompt's glyph, the bar's labels and every hint read the same table, so a rebound key is never shown wrong. F6, F7 and F8 (hold and step, a quarter, four times) are rows like any other.

**Rule:** `jane-present` never sees an SDL type. The app hands it `DeviceState { keys: BitSet<512>, mouse, pad }` and a `UiInput`, and reads back `InputFrame`, `Edge`s and `UiOut`s; the same structs are what a test fills.

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
| `jane sheet scene --save <slot> [--tick n] [--night] [--tier t] [--backend b]` | one whole frame, headless, lighting, atmosphere and UI included; at T0 through `soft` it is the frame CI attaches; at T1 or T2 locally it is the frame the owner reviews. **As built** (no save slots yet): `jane sheet scene [--seed n] [--minutes m \| --ticks t] [--model reader\|rusher] [--night \| --hour h] [--wide] [--out file.png \| dir]` plays the seed from New Game with a player model as `jane play` does, the presenter ticking beside it every frame, sets the clock if asked (`--hour 18:40` lets the minutes pass with the world idle), and writes one frame (768 x 432, or 1008 x 432 with `--wide`) to `sheets/`: T0 through `soft`, or T2 through `wgpu` with `--backend wgpu` when `jane-cli` is built with its `gpu` feature |
| `jane film --save <slot> --ticks N --out dir/ [--every k] [--tier t] [--backend b]` | N ticks from a save, one frame per tick at `alpha = 1`, a PNG per frame (or every k-th), the `FrameStats` table beside them; through `soft` at T0 on CI, any backend locally. The same tool over a trace range (`--trace`, `--from`, `--to`, `--clips`) is the film clip of `VERIFICATION.md` L7. Frames and scene sheets are how a lamp coming on at 18:30, a shadow swinging with the sun or the rain starting is looked at, not asserted |
| `jane sheet fx <spell>` | the effect's parts over 60 ticks, one column per tick, with its lights |
| `jane sheet ui [screen ...]` | **Built 2026-09-27.** The UI headless through the presenter and `soft` in states play rarely shows at once: `hud` (a fight's chips, target, three toasts, a flash, the lag), `dead`, `choice`, `tooltip`, `popover`, `drag`, `pause`; PNGs to `sheets/`. The UI's review sheet, as `sheet scene` is the world's |
| `jane sheet units\|props\|icons\|flora\|chrome\|font`, `unit <id>`, `title` | `ART.md`, the pipeline section; a unit sheet shows all four layers |
| `jane bench --save <slot> --frames 600 [--night] [--wide] [--backend b] [--tier t]` | §1.12; reports per tier the machine can reach. **As built**: `jane bench frames [--backend soft\|wgpu] [--frames n] [--seed n] [--ticks t] [--hour h] [--wide] [--output WxH]` plays to a frame, then times n there: the `Frame` built, the backend's draw, the whole frame to its last pixel at the output size (wgpu upscales to an offscreen target, 3840 x 2160 by default) and the `FrameStats` table |

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
| Lighting and shadows | Point and spot lights with height and a cast flag; sun and moon directional by the clock; ambient by hour, zone and area; occluders are props, units, wall runs and canopy with height; T0 multiply lightmap, contact shadows and sun silhouettes, T1 normal-mapped with hard shadows from 8 casting lights, T2 tiled with soft shadows from 32 traced through a height field | §1.7 |
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
| Audio and bindings | `NullBus` ships and procedural audio is outlined for `PLAN.md` M8 only; `data/bindings.json` compiled in by `jane-present`'s build script (outside the content hash), overrides in `config.json` beside the saves | §4, §5 |
| Saves and config | Slot files and `config.json` in `%APPDATA%\Jane`, `~/Library/Application Support/Jane` or `$XDG_DATA_HOME/jane`; beside the exe when a file called `portable` is there; `--data-dir` overrides | §3.2 |
| The window's height | 350, not 380: the HUD's bar stays visible below it as a drop target | §3.2 |
| The quest tab's name | Log (`VOICE.md` rule 8: no "quest" in the game's words) | §3.2 |
| The title | A backdrop painted once into a UI image with what moves drawn over it each frame, not the `Sky` and `FarLandmark` passes; the passes can take it over when §2.8's parallax lands | §3.2 |
| UI pointer | The OS cursor hidden; the UI draws the arrow, the hand and the reticle | §3.1, §4 |
| Stand-in icons | Drawn from the icon's name in `jane-present::ui::icons` until `ART.md` §2.5 lands | §3.1 |
