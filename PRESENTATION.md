# Jane, Presentation

How the native build is seen, heard and driven: the scene contract and the three render backends behind it, lighting and shadows, atmosphere, effects, the immediate-mode UI and its view buffers, input and rebinding, the audio (its hooks, the cue table, the synth and the score), and the viewer, sheet, film and bench tools. Pair with `ARCHITECTURE.md` (the engine; its §11 is the `View` and `Event` API this file consumes; its §5 is where aim assist lives), `ART.md` (every pixel this file draws is generated there, in four layers), `PORT.md` (phases P6, P7 and P9; the crate map in §4; the perf targets in §9.4, which the per-tier gates of §1.12 here refine), `WORLD.md` (the weather state the atmosphere reads), `VERIFICATION.md` (its L7, human review artefacts, is what `jane sheet scene` and `jane film` feed) and `ENGINE.md` §9 to §12 (the TS record of the renderer, input and UI that carry).

**Changed 2026-09-26.** The single software renderer at 384 x 216 became one scene contract with three backends, a 768 x 432 canvas, four-layer sprites with normal-mapped lighting and cast shadows, atmosphere per area, and aim assist in the sim. The old software design is now the `soft` backend, not the game.

Six crates.

| Crate | Kind | Holds | Depends on | GPU |
| --- | --- | --- | --- | --- |
| `jane-present` | library | `Frame` and `Backend`, atlas, scene, chunks, light, atmosphere, fx, camera, present, view consumption, `ui/`, input mapping, text, the audio trait, bench and film loops | `jane-art`, `jane-sim` (`View`, `Event`, `Command`), `jane-world::names`, `jane-data` (`TEXT`, `NAMES`), `jane-core::view` | none |
| `jane-render-soft` | library | T0: CPU framebuffer, blit, multiply lightmap, mist tile, nearest upscale, read-back | `jane-present` | none |
| `jane-render-gl2` | library | T1: OpenGL 2.1 / GLES 2.0 through `glow`; GLSL 1.20 / ES 1.00; shadow geometry; sharp bilinear | `jane-present`, `glow`, `sdl2` (its context) | GL 2.1 or GLES 2 |
| `jane-render-wgpu` | library | T2: Vulkan, DX12, Metal or GLES 3 through `wgpu`; WGSL; soft shadows, bloom, grading, water | `jane-present`, `wgpu` | Vulkan 1.1 class |
| `jane-audio` | library | every sound made in code: voices, patches rendered at boot, beds, the music sequencer, the mixer, WAV and analysis (§5) | `serde`, `serde_json` | none |
| `jane-app` | binary | SDL2 window, the probe that picks a backend, audio device, the loop, save slots, `config.json` | the five above, `jane-sim`, `jane-net`, `sdl2` | picks one at boot |

`jane view`, `jane sheet`, `jane film` and `jane bench` are `jane-cli` subcommands (§6) built on `jane-present` and `jane-render-soft` with no window; a `gpu` cargo feature, off by default, adds the other two backends for local use.

```
jane-app → jane-render-{soft,gl2,wgpu} → jane-present → jane-art, jane-sim (View, Event), jane-world::names, jane-data (TEXT, NAMES)
jane-app → jane-audio                      (the synth; jane-present never sees it, only its own AudioBus)

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
  audio     AudioBus trait, MusicCue, SfxKind, Bed, the cue table (Soundtrack), place, NullBus (§5)
  bench     headless frame loop through any backend; pixel counting on soft (§1.12)
  film      jane film: N ticks from a save to PNG frames (§6)

jane-render-soft/src/  fb, blit, lightmap, mist, upscale, readback  |  gl2/src/  gl (the one unsafe module: glow behind safe calls), sdl (the context), shaders, prep (Frame to vertex lists, albedo runs, silhouette spans, shadow geometry), ui, lib (targets, passes, present, read-back)  |  wgpu/src/  gpu (device, layouts), prep (Frame to instance lists, lights, tiles), lib (pipelines, targets, shadows, post, present, read-back), shaders/{common, gbuffer, scatter, light, post}.wgsl
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

Every command is in canvas coordinates already (the camera taken off); `camera` is there for parallax. The atmosphere's passes (landed 2026-09-27, §1.8, §1.9, §2) keep the same shape, a `Span` into a list the `Frame` owns, or plain values; the frame also carries `tick`, the presentation tick every drift, shimmer and twinkle is timed by:

```
  Sky(SkyLook { zenith, horizon, glow: Rgb, glow_x: i16, glow_amount, stars: u8, star_list: Span, moon: Option<Moon>, zone: (i32, i32, i32, i32), tick }),
  Parallax { layer: Depth, factor: u8 /* of 256: FarLandmark 32, FarTreeline 64 */, sprites: Span },   // placed on the backdrop: y from the horizon
  Water { cells: Span },                                          // WaterCmd { x, y, phase }: T0's shimmer; T2 reads the chunks' surface layer
  Weather(Atmos { kind: Clear | Mist | Rain | Storm, rain, mist: u8, wind: i8, flash: u8, wet: u8 }),
  Fog { volumes: Span /* FogVolume { rect, edge, density: u8, colour: Rgb, top: u8 } */, drift: (i16, i16) },
  Rays { strength: u8 },                                          // T2
  Particles { layer: Depth /* Ground, Canopy, Weather */, parts: Span },  // Particle { x, y, shape: Streak | Dot | Ring | Glow, colour, alpha, glow, height }
  Ui { cmds: Span },
```

The order a frame holds them: `Sky`, `Parallax` (the moon and the School, then the treeline), `Terrain`, `Water`, the ground sprites, the silhouettes, the standing sprites, `Weather`, below T2 the particles that do not glow (so the lightmap lights the rain), `Lights`, the ground particles, `Fog`, `Rays`, the air's particles, the rain, `Post`, `Ui`. A chunk's layers carry two more for the lit tiers: `surface`, a byte a px (`water * 4 + wet`: the distance to land through water, 1..=16, and how the ground takes rain; 254 beyond the zone, where the sky shows), and `water`, the chunk's water cells for every tier.

T1's shadow geometry is built from what a `Caster` already carries (its sprite's rows, its foot, height and depth; §1.7), so no `segs` joined it: wall runs and roofs in the chunks still throw no point-light shadow on T1 (a gap: they would need the chunks' caster segments in the `Frame`).

Passes are in draw order and a backend draws them in that order; `Pass::needs()` is the tier its `Features` row asks, and a backend below the tier a pass needs draws what its `Features` row says (§1.3), never something of its own. The `Vec`s inside are reserved once and cleared, never dropped: no allocation after the second frame (§1.12).

### 1.2 The Backend trait

```
pub trait Backend {
  fn caps(&self) -> Caps;                                  // tier, max lights, max casters, max texture, has_readback, has_stencil
  fn upload_atlas(&mut self, pages: &AtlasPages);          // albedo, normal, emissive, height; once at boot, again on a CLUT change
  fn draw(&mut self, frame: &Frame);                       // caches chunks by (id, gen); uploads on a miss
  fn read_back(&mut self, out: &mut Vec<u32>) -> (u16, u16);   // the canvas as 0xAARRGGBB; sheets, film, bench
  fn stats(&self) -> Option<FrameStats> { None }           // frame times per pass (§1.12); soft, gl2 and wgpu all give them
}
```

`Caps` also names the backend (`soft`, `gl2`, `wgpu`) for the title bar, F2 and `jane bench`. `AtlasPages` carries each page's four layers (`Page { w, h, albedo: Vec<u16>, normal: Vec<[u8; 2]>, emissive: Vec<u16>, height: Vec<u8> }`); a presenter at T0 packs the albedo alone and `soft` keeps only that.

| Backend | Tier | Runs on | Draws |
| --- | --- | --- | --- |
| `jane-render-soft` | T0 | any CPU; the fallback when no GPU or no driver; `jane serve`, tests and CI | the previous software design: CLUT blits into a `u32` framebuffer, the multiply lightmap, one mist tile, nearest upscale. Full 768 x 432, or **half res** 384 x 216 (a 2:1 decimated atlas built at boot; every `Frame` coordinate shifted down by one) for the Pentium 4 |
| `jane-render-gl2` | T1 | any GPU from about 2006 (GeForce 6, 7, 8; Radeon X and HD 2000; Intel GMA 950 and up where the driver allows) and Raspberry Pi 2, 3 and 4 by GLES 2 through Mesa V3D and VC4 | normal-mapped lighting in a fragment shader, hard cast shadows from 2D shadow geometry, fog layers, particles, tint. No post beyond tint. **As built** (P6b): glow 0.17, one GLSL source for 1.20 and ES 1.00; an albedo pass that is `soft`'s frame before its light to the byte (tested on GL 2.1 and GLES), normal and emissive passes, point-light shadow masks, a light target, the compose, the Ui pass, sharp bilinear in `present`, `read_back` of the canvas (§1.7) |
| `jane-render-wgpu` | T2 | modern PCs and Pi 4 and 5 by Vulkan 1.1 (V3DV) | everything T1 plus soft shadows, many lights, bloom, colour grading, screen-space water reflection, higher particle caps. **As built** (P6c): wgpu 30, WGSL; a G-buffer, a height field and one light pass with soft shadows traced through it (§1.7), bloom, the grade, sharp-bilinear upscale in `present`, `read_back` of the graded canvas |

**Rule:** a backend is a function of the `Frame` and its own caches. It reads no view, no event and no tuning row; everything it needs to draw is in the `Frame` or was uploaded.

**Rule:** `soft` always builds, on every target, with no feature flag. It is what CI renders and what a machine with no GPU plays.

**Rule:** a backend that cannot draw a pass at its tier draws the row below it in the `Features` table; it never skips a pass silently and never invents one.

### 1.3 Tiers and the Features ladder

`jane-app` probes at boot: request a `wgpu` adapter that reports Vulkan 1.1, DX12, Metal or GLES 3 with the limits T2 needs (T2); else create a GL 2.1 or GLES 2 context through SDL and check for framebuffer objects, eight texture units and a fragment shader (T1); else `soft` (T0). **As built:** `jane-app --backend auto|soft|gl2|wgpu`, `auto` by default, asks wgpu (with no window yet) for a high-performance adapter whose downlevel capabilities are WebGPU-compliant (compute, storage buffers in the fragment stage) and, when there is one, opens a plain window and makes its surface; else it opens a window made for OpenGL (`jane_render_gl2::attributes` set first, so X11 picks a GL visual), asks SDL for an OpenGL 2.1 compatibility context and then an OpenGL ES 2.0 one, and takes `gl2` if the context has framebuffer objects (core, `ARB_` or `EXT_framebuffer_object`; a missing entry point is looked up again with the `EXT`, `ARB` and `OES` suffixes), eight texture units, 2048-px textures and compiles every T1 shader; else it draws with `soft` on that window. It prints why at each step it passes, and `--backend wgpu` or `--backend gl2` fails loudly instead. Where wgpu has an adapter but not a surface on the window, the plain window is hidden (wgpu's handle token keeps it) and a GL one opened. The SDL window lends wgpu its handles through `raw-window-handle` with no unsafe code: wgpu wants a `Send + Sync` handle source and SDL's `Window` is neither, so a zero-sized token lends the handles of a clone of the game window leaked once for the life of the process and kept in a thread-local (`jane-app/src/handle.rs`). The title bar names the backend, the adapter, the present mode and the GPU frame times. **Presenting (2026-09-27):** the window presents by `Mailbox` where the surface offers it and by `Fifo` where it does not (`Immediate` first with vsync off), and under `Mailbox` the loop paces its frames to the display's refresh itself, so the GPU draws only frames that are shown. Why: under `Fifo` on a Vulkan window that is not in front, `get_current_texture` waited about 260 ms a frame (3 fps); the GPU, idle between frames, clocked down and its light pass read 10.5 ms instead of 1.2, and the old catch-up cap of 5 ticks a frame dropped 1877 ticks in 15 s. The loop now never drops a tick for a slow frame: a frame runs every tick it owes (at most 50 ms of tick work, the rest on the next frame) and drops only past a second behind, and a minimised or hidden window is not drawn at all while the clock ticks on. `config.json` under `[present]` carries `backend = auto | soft | gl2 | wgpu` and one key per `Features` row. The probe's result and the rows in force are printed on the F2 overlay and by `jane bench`.

Every visual feature is a row. A row names the tier it needs, its default per tier and its `config.json` key, so T0 and T1 degrade by rule, never by accident.

| Feature | Needs | T0 | T1 | T2 | Key |
| --- | --- | --- | --- | --- | --- |
| Multiply lightmap, additive discs, ambient | T0 | on (built: a quarter-size buffer, cubic falloff, a pool at most half again as bright, weaker by day) | replaced by shader lighting (built: the same cubic pool and day rule, per px) | replaced | none |
| Normal-mapped lighting (N dot L with light height) | T1 | no | on (built; off: every surface faces up) | on (built) | `normal_light` |
| Emissive layer (lamps, windows, orbs, spells glow unlit) | T0 | as albedo | on (built) | on | none |
| Hard cast shadows from shadow geometry | T1 | no | on, 8 casting lights (built: casters' rows projected from the lamp, §1.7; chunks' walls do not cast) | on | `shadows` |
| Soft shadows, penumbra by distance | T2 | no | no | on (built: traced through the height field, §1.7) | `soft_shadows` |
| Sun and moon as a directional caster | T1 | ambient only | on (built: its shadows are the silhouettes; N dot L gives the relief) | on (built) | `sun_shadows` |
| Silhouette sun shadows: each unit's and prop's albedo mask sheared along the sun by its height, tinted by the ambient, soft-edged by one dither step (decided 2026-09-27: shadows are a showpiece on every tier) | T0 | on (built) | on (built: `soft`'s integers, so the same bytes) | replaced by the traced soft shadows | `silhouettes` |
| Light count on screen (casting: T1 8, T2 32) | | 16 | 32 (built: one quad each, one draw) | 128 (built: culled per 32 x 32 tile) | `max_lights` |
| Weather: rain and storm particles, mist, lightning, the dimmer and cooler light and grade (landed 2026-09-27, §1.9) | T0 | on (built: the rain under the lightmap, a little of its own light; the flash in the ambient) | the `Frame` carries it; gl2 draws none of it yet (a gap) | on (built: each drop lit by its tile's lamps) | `weather` |
| Fog volumes per area | T0 | one drift tile (built: the strongest in view, none thinner than 20 of 255) | layered, drifting (the `Fog` pass is in the `Frame`; gl2 does not draw it yet) | layered, drifting (built: two layers of the mist tile, the far one on the ground only, lit by the fill, the sun, the afterglow and the lamps it stands in) | `fog` |
| God rays through canopy | T2 | no | no | on (built: the air lit where the low sun reaches the ground past it, against shade; faint in clear air, strong in mist) | `god_rays` |
| Wetness specular after rain | T1 | no | on (in the `Frame` as `Atmos::wet` and the chunks' `surface`; gl2 does not draw it yet) | on (built: wet ground darker, a glint of each lamp and a sheen of the sky) | `wet` |
| Water | T0 | shimmer pixel (built: a 2-px glint a cell, walking by tick) | shimmer and refraction (gl2 does not draw it yet) | reflection and refraction (built: standing things by the height layer, else the sky backdrop and its far things, rippled, darkened by depth) | `water` |
| The sky, the far landmark, the far treeline (§1.9) | T0 | on (built: beyond the zone's north edge) | not drawn by gl2 yet | on (built: a backdrop texture, seen beyond the zone and in every water px) | `sky` |
| Bloom on emissive | T2 | no | no | on (built) | `bloom` |
| Colour grading per region and hour | T0 | CLUT tint (not yet) | tint (built: the presenter puts `Post` in a T1 frame too, its tint and lift alone and the rest `Post::NONE`, so `Pass::needs` is T1 for it; the compose multiplies and lifts) | 3D LUT; built as the same terms in the shader (exposure, a soft shoulder, saturation, tint, a coloured lift) | `grade` |
| Particle pool; weather particles (rain, storm) a third of it | | 900 (built) | 2000 (the `Particles` pass is in the `Frame`; gl2 does not draw it yet) | 8000 (built) | `max_particles` |
| Sharp bilinear upscale | T1 | nearest | on (built; nearest where the scale is whole) | on (built) | `sharp` |
| Half-res canvas; frame skip | T0 | options | frame skip only | frame skip only | `half_res`, `frame_skip` |

**Rule:** a row's tier is the truth. A test builds the `Frame` for a night in the town at every tier and asserts that no pass a tier cannot draw is present in that tier's `Frame`, and that every pass present is drawn by `soft` without a panic.

**As built (the atmosphere's rows, 2026-09-27).** `jane_present::Features` holds `weather`, `fog`, `water`, `wet`, `god_rays`, `sky` and `max_particles`, `Features::of(tier)` their defaults and `Features::set(tier, key, value)` a row by its `config.json` key, held to what the tier can draw (`god_rays` only at T2, `wet` above T0, `max_particles` no higher than the tier's). They live on the presenter's atmosphere (`Present::atmos_mut().features`) and gate what they name each tick. `jane sheet scene` and `jane bench frames` set them with `--rows fog=off,god_rays=off`; `jane-app` does not read them from `config.json` yet (a gap for the app's owner).

### 1.4 Formats and atlases

| Buffer | Format | Size at 768 x 432 | Why |
| --- | --- | --- | --- |
| `soft` framebuffer | `u32` `0xAARRGGBB`, one `Vec<u32>` | 1.3 MB (332 KB at half res) | linear sweeps; SDL streaming texture takes it as is; the light pass is a per-channel multiply |
| Albedo pages | 16-bit master-palette indices (`jane_art::palette::Ix`) into one CLUT `[u32; 1024]`, 2048 x 2048 | 8 MB a page; the whole set ~19 MB, the only pages `soft` holds (`ART.md`, the pipeline section) | one lookup; coat swaps, ghosts and dead ramps are free; on the GPU the CLUT is a 1024 x 1 texture and an albedo texel is two bytes (`LUMINANCE_ALPHA` on GLES 2), the index `lo + 256 * hi`. **As built**: pages are packed at most `atlas::PAGE_SIDE` (2048) on a side, a new page begun when one is full, whatever the backend, so the `Frame` is the same on every machine and tier; 2048 is what a Pi 2 or 3's VC4 takes and what `gl2` requires, and `Caps::max_texture` reports each backend's own limit (`soft` none, `gl2` `GL_MAX_TEXTURE_SIZE`, `wgpu` its device's) |
| Normal, emissive, height pages | RG8 tangent-space; 16-bit emissive index (an `Ix`, 0 is dark); 8-bit height; the albedo's layout (on T2: `Rg8Unorm`, `R16Uint`, `R8Unorm` array textures beside the `R16Uint` albedo, the CLUT a 1024 x 1 `Rgba8UnormSrgb`) | 5 bytes a texel together | generated by `jane-art` from the shape primitives a look used, never painted; lamps, lit windows, orbs and spells glow unlit; height gives shadow length and water (a person is 40, a barrel its `rise`) |
| Terrain chunks | four layers, 256 x 256 px: albedo `u32` resolved, normal RG8, emissive 16-bit `Ix`, height 8-bit; plus strips | 576 KB a chunk on a GPU, 256 KB on `soft`, and the strips (0 on open ground, up to about 1.1 MB in a dense wood: or none, drawing `placed` flora from the atlas); LRU 48 | the ground is the biggest draw (§1.6) |
| `soft` light buffer | `[u16; 3]` per cell at 1/4 resolution (192 x 108) | 124 KB | §1.7 |
| T1 light and shadow targets | RGB8 at 1/2 resolution; one 8-bit shadow mask. **As built**: RGBA8 light target at full size on desktop GL, half each way on GLES and tile GPUs (`Rows::half_light`), holding display values halved (0 to 2); two RGBA8 masks at the light target's size, a channel a casting light; plus the albedo, normal-and-height, emissive, silhouette-mask, snapshot and canvas targets at canvas size | 1.3 MB (full) or 330 KB (half) and 2 x the same | §1.7 |
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

**Sources, all from the view.** Prop lights through `View::light_showing(&Prop)` (THE rule, `ENGINE.md` §6, shared with the sim so what she sees lit is what a sentry sees lit); unit glows; her lantern, radius 136 canvas px, when ambient is below 750 permille; effect lights from `fx`; spells. Every light carries colour, radius, falloff (a 256-entry LUT on T0, the same curve in the shader), height above ground, a cast-shadow flag and a kind (point, or spot with a direction and cone: the sentry's eye, the lighthouse). Flicker from a 64-entry table indexed by `(tick + h32(id)) * rate`. **A light is never shadowed by its own prop** (2026-09-27): every `Light` carries `clear`, the px round its ground point that throw no shadow on it (a lamp post's 14, a fire's 22, half its footprint and 6), and T2's march stops that far short of it; before, a lamp's cap and a fire's flames stood taller than the light they hold and threw a hard wedge, or swallowed the pool whole. A standing prop's light shines from its glass but never lower than 28 px (`FLAME_HEIGHT`), since a light at a fire's 14 px only grazes the ground; and a deep orange flame is held up to a lamp's luminance before its gain on T2 (`MIN_LUMA`), or green grass swallows its pool. The data's radii are unchanged, so `light_showing` and the sentries see what they saw.

**The sun and moon** are one directional light whose angle follows the clock: azimuth east to west over the day, elevation from the hour, so at 17:00 when Jane steps off the train the shadows are long and point east, and by 18:30 when the lamps come on the sun's share is gone. The moon is the same light at night, dim and blue, with a phase from the day. Directional shadows are cast by every caster on screen at once, which is why T1 caps casting *point* lights and not the sun.

**Ambient** by hour from the clock's keyframes outdoors; a zone's `ambient` indoors, blue-tinted; **per-area overrides** from the skeleton's areas the blueprint carries (the Works darker at noon under their soot; the lake shore brighter under open sky), each a row with a colour and a permille per hour band. Day is free on T0: at ambient ≥ 254 the multiply is an exact no-op and the pass is skipped.

**Occluders.** Props with `rise`, units (a person is 40 tall), wall runs (from `TileStyle.height` and `wall_like`, merged into segments per chunk when the chunk is painted) and canopy trees. A `Caster` is its footprint's edges at ground with a height; casters are gathered once a frame inside the light reach (208 canvas px past the view) and shared by every light.

| Tier | Lighting | Shadows |
| --- | --- | --- |
| T0 `soft` | the previous design: a light buffer at 1/4 resolution cleared to ambient, every light **added** as `colour * LUT[(d2 * 255) / r2] * intensity` over its disc, upsampled bilinear (or nearest plus Bayer under `light_steps`) and multiplied `dst = (dst * L) >> 8`. No normals; emissive texels are drawn as albedo at full brightness. **As built**: the LUT is `(1 - d2/r2)^3`, a pool lifts a pixel to at most 1.5x, its gain falls by day, and flame light leans warm as on T2 | the contact shadow (index 1) and the **silhouettes** of the sun or moon: each caster's rows laid on the ground `h * cot(elevation)` px away from the sun, stretched to meet the next row's and as thick as the caster is deep, gathered in a coverage mask so two never darken twice, fading toward the tip, multiplied toward the sky's `shade`, the edge px at five eighths and a ring outside at three eighths through the 4 x 4 Bayer. Integer |
| T1 `gl2` | per light, a quad over its disc; the fragment shader reads albedo, normal and emissive, computes N dot L with the light's height as z, applies the falloff and the shadow mask, and adds into a half-res light target; ambient and sun first; one multiply pass at the end; emissive added unlit. **As built** (below) | **hard**: for each casting light and each caster in its disc, every footprint edge facing away from the light is extruded to the disc's rim scaled by `height`, drawn into the shadow mask with the stencil where the context has one and by alpha accumulation into a small FBO where it has not; the light's quad reads the mask. 8 casting lights, then the nearest 8 by rule. **As built**: the caster's rows, not its footprint's edges (below) |
| T2 `wgpu` | the same terms in WGSL over a tiled light list (128 lights, culled per 32 x 32 tile on the CPU, 64 a tile at most), one pass: the fill, the sun (N dot L over its elevation's sine, so its colour is its light on flat ground, capped at 2.5x on a face turned to a low sun), each point light by N dot L with its height as z and a windowed inverse square, emissive added unlit; the height field's ambient occlusion darkens the ground at the foot of what stands | **soft**: traced through the height field (below) from every lit pixel toward the sun or moon and each of the 32 casting lights, the penumbra widening with the distance from what casts it, tinted by the fill |

**T1 as built (P6b, 2026-09-27).** `jane-render-gl2`, glow 0.17 over an SDL2 context, one GLSL source for 1.20 and ES 1.00 (`#version` and a float precision put on per dialect; no bit operations, no integer textures, no const arrays, loops of constant bound). Every target keeps canvas row 0 first, so a read-back needs no flip; only the window's upscale turns it up. A frame:

1. **Albedo.** The passes in order into an RGBA8 target: chunks from an 8 x 6 atlas of 256 px slots (cached by `(slot, generation)`, forgotten on an atlas upload, since a new presenter numbers its generations again), then each sprite pass through the CLUT (a 1024 x 1 RGBA8 texture; albedo and emissive pages `LUMINANCE_ALPHA`, the index `lo + 256 * hi`; normal and height one RGBA8 page; pages are at most 2048 on a side, and one taller than the context's limit, should one come, is laid in strips side by side), the contact shadow by its 3 x 3 cover inside the sprite's rect, flash and ghost by `soft`'s two-multiply lerp, and between the ground and the standing things the **silhouettes**: the spans of `soft`'s `silhouette::cast`, built on the CPU from the casters' rows with the same integers, MAX-blended into a canvas-sized mask, then applied from a snapshot of the albedo by `soft`'s `apply` (edge five eighths, the ring three eighths on the dither's odd squares). All of it is integer arithmetic in floats (`floor((a + 0.5) / b)` for a division, bytes read as `floor(v * 255 + 0.5)`), so in the **exact** mode the target is `soft`'s framebuffer before its lightmap, byte for byte: the texels that read what is under them (the contact shadow, a ghost) read a snapshot, and the sprites are drawn in runs no reader overlaps, the readers' rects copied into the snapshot before each run (`glCopyTexSubImage2D`). The **fast** mode, the default on GLES and on tile GPUs (VC4, V3D, Mali, Adreno, PowerVR, where every copy out of a target flushes it), draws as T2 does: each pass's contact shadows first by a multiply, then its opaque runs and its ghosts blended; at most one level off on about a thousand px, and a nearer thing's contact shadow no longer darkens the feet behind it. The test `crates/jane-render-gl2/tests/albedo.rs` draws the town at 22:00, 17:00 (with silhouettes), noon and 19:00 on seeds 1 to 3 through desktop GL and GLES and asserts the exact albedo equals `soft`'s frame with its `Lights` pass removed, pixel for pixel.
2. **Normal and height, then emissive**, the same chunks and non-ghost sprites into two more RGBA8 targets (`(nx, ny, height, depth)`, a mirrored sprite's `nx` flipped; the emissive index's CLUT colour). GLES 2 has no multiple render targets, so these are passes, not attachments.
3. **Point-light shadows** for the nearest 8 casting lights (`shadows`): each caster within the light's reach, except the one holding it (her lantern, 14 px from her feet), is its sprite's rows from the foot up, runs of equal rows merged, each run a vertical slab at the front and at the back of the footprint projected from the light onto the ground (`t = d * h_light / (h_light - z)`; a row at or above the light reaches the rim plus 24 px and stops). Each corner carries how high the shadow reaches over that ground point, the ray over the caster's top, `h_light + (H - h_light) * t / d`, and the quads are MAX-blended into one channel of two RGBA8 masks at the light target's size (four lights a mask, two passes a frame).
4. **Light.** A light target holding display values halved (0 to 2). The sky first: the fill and the sun or moon by N dot L over its elevation's sine, summed in linear light, **exposed so flat ground is as bright as T0's `ambient` and keeps the sky's colour** (fill and sun scaled together by `luma(ambient) / luma(fill + sun on flat ground)`: the dusk is the fill's violet, not the keyframes' orange), the relief eased toward flat by 0.6 so a face turned away keeps two fifths of the sun (pixel art at 0 goes to mud). Then every point light (up to `max_lights`) as one quad over its disc and 72 px above it, in one draw, added: its colour times 0.95 by T0's day rule (`(300 - ambient) / 220`, capped 1) and warm lean, by T0's cubic falloff across the ground, by N dot L relative to flat ground (wrap 0.4, at most twice the ground's), by a spot's cone, and dark where the px is lower than its mask says the shadow reaches (a px `h` up is found on the ground `h` rows lower, and a standing thing's face half its depth nearer the viewer, so it never falls in its own shadow).
5. **Compose** into the canvas: albedo times light plus emissive, then the tint and lift of the frame's `Post` pass (its T1 grade). **Ui** over it (`ui.rs`): one quad per command, straight alpha, runs sharing a clip, image and page in one draw; a UI sprite's contact-shadow texel is black at a quarter of its coverage, as on T2.
6. **Present**: sharp bilinear to the window (nearest where the scale is whole or `sharp` is off), or to an offscreen target of the output's size for the bench.

**Why this shadow and not the height field or the stencil.** The height field (T2) needs a scatter of every lifted px to its ground point, which GL 2.1 and GLES 2 can only do as a point per px with a texture read in the vertex shader (none on VC4, float-only on a GeForce 7), and then a march of 30 or more reads per lit px per light: several ms on a Pi 4 at half size. Footprint boxes (the plan) make every person a rectangle and every tree a trunk-wide wall. The rows are what the silhouettes already use: a tree's shadow from a lamp is its canopy's shape thrown long, a person's is her outline, and the CPU work is a few hundred quads. A MAX-blended height mask rather than the stencil keeps what the stencil cannot, how high the shadow stands (so a person in a barrel's lamp shadow is dark to her knees, not to her hair), packs four lights to a target, and needs only `GL_MAX` (core since GL 1.4; `EXT_blend_minmax` on GLES 2, on every Pi); without it the masks and silhouette spans are written last-wins. Limits: chunks' walls, roofs and fences throw no point-light shadow (no segments in the `Frame`), and the edge is hard.

**Unsafe.** Every `glow` call is an `unsafe fn` (a C API: the context must be current on the calling thread, and a pointer handed to the driver must be as long as the call says), so the workspace's `forbid` cannot hold in this crate: `jane-render-gl2` is `deny`, as `PORT.md` §4 lists it, and `src/gl.rs` alone allows unsafe code, lending the rest of the crate safe calls that check every length they hand the driver, point vertex attributes into buffers by offset only, and are neither `Send` nor `Sync`. The context is made in the same crate (`sdl.rs`) from SDL's safe `gl_get_proc_address`, so no raw loader crosses a crate boundary; `jane-app` and `jane-cli` stay `forbid`.

**The T2 shadow technique: a height field seen from above, traced with a clearance penumbra.** The G-buffer (the canvas and a 64 px guard band round it, so a caster just off screen still casts) holds each pixel's albedo, normal, emissive, height and depth. In the 3/4 view a pixel `h` px up at `(x, y)` stands on the ground at `(x, y + h)`, so a compute pass stands every lifted pixel on its ground point in a buffer the canvas's size, the tallest winning, a px wider each side and as many rows deep as the caster is: an upright sprite's columns land on its feet and become a thin wall as tall and as shaped as it is, and a roof lands on the house under it. The light pass then marches from each lit pixel (from the front of its body when it stands) toward each light across that field, the ray rising as the light is high, and keeps `min(k * clearance / t)`: `t` px from the pixel, how far the ray clears the field there, `k` the light's distance over its size (or `1 / tan` of the sun's spread, 2 to 5 degrees, widest at dusk). Steps are 1 px near and up to 3 far, a long step reading the tallest of four texels so a thin post is never stepped past, a short one bilinear so a penumbra has no stairs. **Why this and not 1D polar maps or SDFs:** it casts from what the art drew (every px's true height, `ART.md` §1.1) rather than from footprint segments, so a person's shadow is her silhouette, a canopy's is lumpy and a fence throws a comb; the sun is one more light, not a directional map to place; the penumbra comes free and widens with distance as a real one does; the terrain casts with no geometry of its own; and it is one compute pass and one fragment pass at canvas resolution, independent of the output's 4K. Its limits: what is hidden behind something else on screen does not cast, and nothing off the guard band casts in.

**Rule:** what is lit is decided by the view (`light_showing`), the tier decides only how it looks. A lamp the sim says is lit is lit on every tier; a lamp the sim says is dark casts nothing anywhere, and its glass is dark.

### 1.8 Materials: wetness and water

**Wet after rain.** Every tile carries a per-cell `wetness` byte in the presenter (never the sim), set to 255 while `WORLD.md`'s weather state is rain or storm, decaying by the tuning row after it clears, faster under roof and canopy, never on water. On T1 and T2 the fragment shader adds a specular term from wetness and the light's height along the tile's normal, so a wet road glints under a lamp; on T0 nothing changes, and that is the row.

**Water.** T0: one 1-px highlight per water cell at a hashed offset, moving by tick (the shimmer). T1: the shimmer plus refraction, the water texel's normal offsetting the sample of what lies under it. T2: **screen-space reflection**, standing things above the water line reflected downward into water cells by their height layer, darkened by depth, rippled by the same normal; the School's silhouette in the lake at dusk is the reason it exists.

**As built (2026-09-27).** *Wetness* is the sim's rain ramp under her feet (`View::wetness`, which already climbs in rain and falls after it, WORLD.md §5.1) times each px's own response, which the terrain painter gives per cell (`TileStyle.wet`: 0 matt, 1 darkens, 2 darkens and shines) and the chunk carries in its `surface` byte; water is never wet, and indoors nothing is. The presenter keeps no ramp of its own: the sim's is the truth the douse rule reads, so the road dries when the fire relights. On T2 wet ground is up to 40 % darker, reflects a sheen of the sky's fill, and glints under every lamp by a Blinn term toward the 3/4 eye, so a lamp's reflection runs long down a wet road toward her; the same term makes water glint always. T0 draws nothing for it, which is its row. *Water* on T2 is a full-canvas pass after the light: a px of water (its `surface` distance to land) looks for a px `2h` above it that stands `h` tall and is not water, up to 72 px, and takes its lit colour; where nothing stands it takes the sky backdrop (§1.9) laid down the screen, its horizon along the top edge (row `0.62 y`), so the School and the treeline hang upside down in whatever water lies toward the top of the view and the colour runs from the horizon's to the zenith's as the eye comes nearer (a shore-relative horizon was tried first: every spit of land cut a seam in it). The reflection is rippled a px by two swells and the wind (roughened by the rain), banded by a crest and a trough row that wander with the swell, laid over the water's own lit colour refracted half as far, at 42 % in the shallows to 74 % in the deep, and darkened by depth. Beyond the zone's north edge the same pass shows the sky itself. T0: the shimmer is a 2-px glint a cell, 40 of every 64 steps, walking from the cell's phase. Measured: 0.11 ms at dawn with a lake filling half the view.

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

**As built (2026-09-27): the weather.** `jane_present::atmos` reads the sky over her from the view (`View::weather`: rolled by the sim hourly per region, clear on the first walk; this module never rolls) and eases each turn in over ten seconds: rain to 40000 of 65535 (a haze of 7000 with it), a storm to full (12000 of haze), mist to 52000; indoors the sky is shut out. The wind is a breath by day, a push in rain and a gale in a storm, gusting on a slow sine by tick; the rain leans by it and the mist tile drifts with it. Lightning strikes in a storm's full dark every six to eighteen seconds from the atmosphere's own `Lcg` (a second on its heels one time in four): the flash is full for two ticks and gone in five more, and lifts the ambient (T0) and the fill (T2) toward white. The light changes with the sky (`Atmosphere::light`): cloud takes up to seven eighths of the sun's colour and softens its shadows, rain darkens the flat light by up to 28 % and cools and greys the fill, mist lets the fill up toward the flat light (it comes from everywhere); the grade loses saturation (26 in rain, 22 in mist), cools its tint in rain and lifts its shadows toward the mist's colour. *Rain* is a third of the particle pool: drops fall 110 to 190 px onto ground points in and round the view at 9 to 13 px a tick, each a stroke as long as a tick and a third of its fall, its length hashed so the rain is not a comb, and ends in a splash (a ring of 0 to 2 px over 5 ticks) or, on water, a ripple (1 to 7 px over 22); on T2 each is lit by the fill and the lamps of its tile, so the rain shows where the lamps are; on T0 it goes under the lightmap for the same reason, with a little light of its own.

**As built: the sky and the far things.** The backdrop is the sky's hour ramp (`jane_art::weather::sky`, thirteen keys: deep blue at night, rose at dawn, blue by day, gold then rose then violet at dusk), greyed by cloud, with the afterglow low over the sun's bearing, 120 hashed stars from half past eight to five (one in eight twinkling, cloud hiding them) and the moon in its phase (eight, from the day) crossing east to west over the night. On it, the School as a silhouette at its bearing (x across the canvas by `dx / dy`, a third of the width at 45 degrees; the band by distance: 120, 160 or 200 px wide), one window lit from 18:30 and a second flickering, only in the county and only while it is north of the view; and a treeline strip per region at a quarter of the camera, the Works' with chimney stacks. On T2 it is rendered to a texture the canvas's width and 256 rows (row `y` is `y` px up, so it is already the mirror water wants), the far things hazed toward the sky by their distance and lit by the sky's own light, their windows glowing; it shows beyond the zone's north edge and in the water. T0 draws it only beyond the zone's north edge, dithered row to row. In play the camera keeps to the zone, so the sky is seen mostly in water.

**As built: fog.** Volumes are the data's layers (`data/atmosphere.json`, WORLD.md §5.3), resolved on entering a zone to the areas and rects they name (`View::areas`, `View::rects`), a region's over the whole view while she stands under its sky outdoors, a zone's over the whole zone; each eases in and out with its hours and skies, and fades over its spread past its rect. Beside them, the weather's mist over the whole view, and at dusk and dawn a thin haze the afterglow lights. T2 draws them in one pass after the water: per px the volumes' summed density (a ground fog thinning to nothing by its `top`, so a thing taller shows its head and shoulders), a far layer of the mist tile on the ground only and a near one over everything, larger and faster, as banks and lanes (the tile squared over a thin even haze); the fog is lit by the fill, the sun's share, the afterglow and each lamp of its tile unshadowed (a lamp in mist has a halo). T0 draws one drift of the tile at the strongest volume, blended by a table, and nothing for a volume thinner than 20 of 255. *Light shafts* (T2): over each px the air up the column to 48 px, lit where the sun's ray past it reaches the ground (the light pass writes where the sun reaches), shown against shade; faint in clear air, strong in mist; only when the sun is under 30 degrees and the sky clear or misty.

**As built: dusk.** Round sunset and dawn the grade warms the side of the view toward the sun's bearing and leans its far (top) edge toward the horizon's colour, the afterglow's strength by the clock; with the thin haze this is what keeps an 18:40 frame from being flat violet away from the lamps.

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
| Lightning | ambient to full for 2 ticks, once per storm roll from the presenter's `Lcg` (built: in a storm's full dark, a strike every 6 to 18 s from the atmosphere's `Lcg`, one in four doubled; full for 2 ticks, gone in 5 more) |
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
| 8 | `frame_skip = 1`: draw every other tick, 30 fps | half the frame | last resort; the sim still steps at 60 and the catch-up in `jane-app` (a second behind at most, 50 ms of ticks a frame) still holds |

**Measurement.** `FrameStats` per pass (sky, parallax, chunks, water, list, fx, shadows, light, fog, weather, grade, ui, upscale); F2 prints p50 and p99 over 120 frames, the backend, the tier and the rows in force. `jane bench --save <slot> --frames 600 [--night] [--wide] [--backend b] [--tier t]` runs the same loop headless and prints the table per tier it can reach on the machine, plus **pixels written per frame** on `soft`. CI asserts the deterministic proxies on every target through `soft`: pixels ≤ 6 x canvas area at night, draw list ≤ 1500 in town, lights in the `Frame` ≤ the tier's `max_lights`, casters ≤ 256, chunks landed per frame ≤ 2, and no allocation after the second frame (a counting allocator in the test; a `#[global_allocator]` is an `unsafe impl`, which the workspace's `unsafe_code = "forbid"` refuses in tests too, so until the one-exception crate of `PORT.md` §3.4 exists the in-tree test holds every `Frame` list and the framebuffer to its address and capacity, and the counting allocator runs out of tree: 0 allocations in `tick` + `draw` + `soft` over 2400 frames after the second, idle, walking and idle, on 2026-09-27).

**Measured (2026-09-27, T2).** `jane bench frames --backend wgpu` on the owner's desk (RTX 3060, Vulkan), the town at 22:00 with 7 lights, 768 x 432 upscaled to an offscreen 3840 x 2160: the GPU's own clock p50 1.84 ms, p99 2.40 ms (the light pass 1.6 ms of it, the height field 0.02, the G-buffer 0.04, bloom and grade 0.05, the upscale 0.12); the whole frame to its last pixel p50 2.2, p99 2.8 ms; at 17:00 p99 2.9 ms; at 21:9 (1008 x 432 to 5040 x 2160) p99 3.1 ms. The gate is p99 < 6 ms. `soft` on the same machine: 1.5 ms p50, 1.8 p99 at 768 x 432, the silhouettes 0.5 and the lightmap 0.9 of it.

**Measured with the atmosphere (2026-09-27, T2, same desk, 4K output).** The town at 22:00, clear: GPU p50 1.73, p99 1.75 ms, the whole frame p99 2.21 ms (the sky 0.01, the water 0.01, the fog 0.00, the grade 0.04). In a storm (2666 rain parts, the haze, the wet ground): GPU p99 1.81 ms, whole frame p99 2.46 ms (fx 0.01, fog 0.02, weather 0.01). At the reeds at 06:00 in mist, a lake filling half the view: GPU p99 2.48 ms, whole frame p99 2.92 ms (water 0.11, fog 0.03, the light 2.0 of it). The gate is p99 < 6 ms: the atmosphere costs about 0.1 to 0.2 ms of it. `soft` at the reeds in mist: 1.42 ms p50, 2.50 p99, the fog 0.43 of it.

**Measured (2026-09-27, T1).** `jane bench frames --backend gl2 --frames 600` on the same desk (RTX 3060, an OpenGL 2.1 compatibility context the driver gives as 4.6, GLSL 1.20; `GL_TIME_ELAPSED` per section, read four frames late), the town at 22:00 with 6 lights and 42 casters, 768 x 432 upscaled to an offscreen 3840 x 2160, 21 draw calls:

| Rows | GPU clock p50 / p99 | Whole frame to last px p50 / p99 |
| --- | ---: | ---: |
| T1 defaults on desktop GL (8 casting, full-size light target, exact albedo, normals) | 0.24 / 0.24 ms (list 0.08, light 0.03, shadows 0.01, compose 0.01, upscale 0.11) | 0.52 / 0.63 ms |
| `shadows = off` (the GeForce 7 gate's rows) | 0.23 / 0.23 ms | 0.43 / 0.64 ms |
| half-size light target, fast albedo (the Pi's defaults) | 0.21 / 0.21 ms | 0.46 / 0.58 ms |
| the same through a GLES context (GLSL ES 1.00; no timer queries there) | CPU 0.10 / 0.13 ms | 0.45 / 0.56 ms |
| shadows off, half light, fast, `normal_light = off` | 0.20 / 0.20 ms | 0.38 / 0.50 ms |

At 17:00 (the silhouettes, one light) 0.45 / 0.60 ms whole; at 21:9 (1008 x 432 to 4K) 0.53 / 0.72 ms. One run with the machine building in four other worktrees had a p99 of 14.6 ms from the scheduler, not the GPU; the table is from quiet runs. `soft` on the same bench: 2.1 / 4.2 ms. What this does not say: a 3060 is some fifty times a GeForce 7800 in fill, so the two T1 gates stay unmeasured until the ancient PC and the Pi 4 are on the desk. The work a frame asks of them, as built: about 4 million px shaded at 1080p output (the albedo, normal and emissive passes over the canvas and its sprites, the light target, the compose, the upscale), 3 to 12 texture reads each, and one copy per contact-shadowed sprite in the exact mode (which is why the fast mode is the default on tile GPUs).

**`FrameStats`** (`jane_present::backend`) is what F2 and `jane bench` read: the whole frame's last time, p50 and p99 over 120 frames; the same per pass in `[u32; 13]` arrays keyed by `StatPass` (sky, parallax, chunks, water, list, fx, shadows, light, fog, weather, grade, ui, upscale; a pass a backend does not split reads 0); whether the clock is the GPU's; the last frame's draw calls, lights, casters and px written (`soft`). `wgpu` writes timestamp queries at every pass boundary and reads them a frame or two late, the upscale in `present` with its own; without `TIMESTAMP_QUERY` it keeps the CPU's encode and submit. `soft` times each pass on the CPU. `gl2` times seven sections (chunks, list with the normal and emissive passes, shadows, light, grade for the compose, ui, upscale) with `GL_TIME_ELAPSED` queries (`EXT_timer_query` or `ARB_timer_query`, desktop GL only) in a ring of four frames, and without them the CPU's time in each section.

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

**As built (2026-09-27).** The tables are `jane_art::fx`: an entry for each of the 24 spell rows (cast, bolt, impact, ground: the TS build's looks) and the 20 effect rows (how a status is worn), each a `Recipe` of emissions (how many, what shape, which of the look's three colours and what it turns to, from where: the point, a gather ring moving in, a scatter; speed, spread, rise, gravity, drag, life, glow, a ground mark or not) and the light it throws; `jane_art::fx::Spark` is one part and its integer step, shared by the game and the test that every entry draws pixels (and that no entry names a row that is not there). The pool is `jane_present::fx::Fx`: a cast, swing, impact, death or status resolves into its recipe at event time, at the caster's hands, the blow's point, the body; a status sheds every few ticks on its wearer while it is on; a bolt in the view's projectiles sheds its trail along each tick's whole path and draws a head (a glow, a core and a streak back along its flight) that carries a light, so a bolt lights the wall it passes on T2 (casting, with a clear of 6 px) and blooms; a pool on the ground bursts as it appears. Parts are `Frame` particles: T2 lights each by its tile's lamps and adds what glows unlit and into the bloom; below T2 the parts that do not glow go under the lightmap and those that glow over it, their unlit share multiplied by the ambient. The fx take two thirds of `max_particles` and the rain one third, the oldest dropped first; the `Lcg` is reseeded per zone from `h32(seed, zone)`. Not built: `jane sheet fx <spell>` (the recipes can be run by `jane sheet scene --cast SPELL[:TICKS]`, which casts east and draws the frame so many ticks later), and the `Painter` trait the design names (the frame's particle list is the one painter).

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
| Host, Join | **As built (P8, `ui/lan.rs`).** Over the title, in the menus' window panel and gold heading, choices as tab buttons (left and right step the lit row). **Host**: the world (New, or Slot 1 to 3 where a slot holds a save, its zone and day beneath), the door (Open, Closed), seats (2, 3, 4, with the four coats beside, lit up to the count), input delay (2 to 6 frames, in ms beside), if one stalls (Drop at 10 s, Wait), the port (digits, 1024 and up; Join finds it on any port); Host and Back. Host builds or loads the world behind the loading card and opens it to the LAN once built; a rest by anyone at the table writes the host's slot. **Join**: the hosts on this network (a UDP ask every second: name, address, seats taken; a host this build cannot join is greyed and says so), an address field (the last joined offered again), and beneath it what a join is doing (knocking, building the host's county, waiting for the world) or why it ended; a refusal shows both content hashes (or both builds). Join becomes Cancel while knocking. **Pause** gains "Open to LAN" alone, "Hosting on port N" (greyed) when hosting, and "The host keeps this table" (greyed) when joined, its foot saying the world is the host's and anyone's rest saves it there; while someone else sits at the table a line over it says the world does not stop. **HUD** at a table: under the vitals a plate of the four coats, lit for each seat taken, hers underlined, with "hosting" or "joined"; a stall banner in the top third while the table waits (the awaited coats, how long, what happens at 10 s or that the host waits); toasts as coats sit down and get up ("The teal coat sat down", "Two at the table: each of us is weaker", "Alone again: as strong as ever"), read from the sim so every machine says the same; a desync's report to the terminal and a toast. A guest cannot save (the world is the host's) |
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

## 5. Audio

**Built 2026-09-27.** Every sound in the game is made by code: no audio file ships (`DESIGN-2020.md` §6), and no sample table in the source or the data holds more than 32 numbers (the art's "no grids" rule, heard). The palette is one voice throughout: soft FM, additive tables with a little unison, filtered noise and plucked strings under gentle envelopes, in one small shared room, never a raw chiptune beep unless the thing is meant to beep (the UI's ticks are soft wooden FM, not square waves). Three pieces:

```
jane-present::audio   the hooks and the cue table: View + Event -> music cues, placed sounds, beds   (no samples)
jane-audio            the synth: voices, patches rendered at boot, beds, the sequencer, the mixer    (no device)
jane-app/src/audio.rs the SDL2 device: a callback at 48 kHz stereo running jane-audio's Engine      (the bus)
```

### 5.1 The hooks

```
trait AudioBus { fn music(&mut self, cue: MusicCue); fn sfx(&mut self, kind: SfxKind, at: At, listener: At);
                 fn bed(&mut self, bed: Bed, level: u8); fn tick(&mut self) }
enum MusicCue { Title, Zone(ZoneId, Region, night: bool), Bell, Combat, Dead, Silence }
struct NullBus;   // jane serve, the tests
struct Soundtrack // the cue table: title(bus) on the title and loading screens, tick(view, events, bus) each play tick
```

`Zone` carries the region because the county is three places: a dungeon's cue normalises its region and night away (`MusicCue::zone`), so a dungeon's music does not change at nine. `bed` is the one addition to the outline: the loops are not sound effects, they are levels the bus fades to. The table reads the world through `Sense::of(view)`, a plain struct, so every rule is tested without a county (`audio.rs` tests); the view gained `flag(name)` and `weekday()`, read-only, for the bell (`jane-sim/src/view.rs`).

**Music.** Title on the title and loading screens. The zone's cue on entry and at the night boundary. A hostile fighting her (in combat, targeting her, within 24 cells; or a blow either way) for **120 ticks** turns it to `Combat`; **360 ticks** after the last it turns back; a skirmish shorter than 2 s never turns it. `PlayerDied` (or her body down) is `Dead`, which plays once and leaves the silence until she wakes. While the bell strikes, `Bell`. `the_end` is `Silence`. The fades are a table (`fades`): into a fight 0.7 s out and 0.3 s in, out of one 2.5 s each way, a fall cuts in 0.3 s, the bell hushes over 2.5 s.

**Effects.** Every `Event::Sfx`, swing, impact (by school), hurt, crit, heal, death, cast, failed cast, status, loot, quest given and done, learned spell, journal, rest, respawn, prop change (open, use, unlock, lock, switch) and door (a zone change) becomes an `SfxKind` at its place. `place(at, listener)` fades it over **24 cells** (the square of the remaining distance, so it reaches nothing smoothly), pans it by the offset across (full at 12 cells, 85 per cent wide) and sends more of it to the room the further it is. Her footsteps keep time with the walk cycle (§1.11): one as she steps off, on the drawing's first contact, then one every 18 ticks while she keeps moving (`STEP_TICKS`, half of six frames of six ticks), so each lands where a foot comes down; by distance they would patter, since she crosses seven cells a second. Each falls on the surface under her: grass (grass, garden, crops, moss, grown path), road (dirt, road, track, sand, rubble, rail), cobble (cobble, stepping stones and every stone floor), wood (floorboards, the boardwalk) and water, which is also soft ground whose wetness is at 150 or over (`WET_STEPS`), so a walk after rain splashes. An owl calls at night and a crow over the Lowfields by clear day, every 30 to 90 s, 10 to 18 cells off in a seeded direction; a storm thunders every 12 to 37 s. Julie's dog barks when it comes out on the step in the morning or something fights near it, whines when it goes for the night while she is within twelve cells, and pants when she comes to its side (at most every 20 s). The UI makes a soft tick for each step through a menu, a small sound for a screen opened or closed and a choice made, and a chime for a save.

**At the table** (LAN co-op, `PLATFORM.md`): each machine hears its own seat. The cue table ticks on the session's view of this machine's seat, so the music is her zone's, her fight's and her fall's; everything the other seats do in her zone (their swings, casts, hits, deaths, a door they push) is placed where it happens, like any other sound; their footsteps are not played (hers only). Alone, a pause or an open window holds the world, and the music and beds step back to a third of their level until it goes on (`Cmd::Duck`); with company nothing holds and nothing ducks. The music's variation follows the county's seed, so every machine at a table hears the same score.

**Beds.** Rain from the view's weather (170, a storm 255) outdoors, on the roof indoors (150, 220) and not at all underground; wind by region (Works 95, Lowfields 70, Waters 55; a clear night +30, rain +50, a storm 235, mist 40); birds by the clock outside the Works (a dawn chorus 05 to 08, a quiet day, a full evening 17 to 21, a third of it in rain; the Forest too); crickets from 20:00 to 04:00 in the dry (fewer in the Works); the lake in the Waters; the hum in the Works (louder after nine), the Factory and the pipes; drips in the mine, the pipes, the cellar and the Burial; a fire in the Arms and at home, and any lit campfire, brazier or stove within 8 cells, louder as she comes to it (a doused one is silent); a clock at home and in the Library and Museum. Levels are 0 to 255 and every bed swells over about two seconds.

### 5.2 The bell at nine

The bell is a struck tower bell: eleven modes after the partials of an English church bell (the hum an octave under the strike note, the prime, the **minor-third tierce** that makes a bell sound sad, the quint, the nominal, and the high partials that make the clang), each a pair a few tenths of a hertz apart so it beats as a real bell hums, each dying at its own rate, hum longest; the strike note is D (146.8 Hz, the hum D2), the key of the title and of the bell's own cue. `jane sheet audio` draws it (`sheets/audio/sfx-bell_near.png`): the beating is the curved lines through the partials.

It **strikes the hour**: nine strikes at 21:00 and six at 06:00, 2.5 s apart (a hand-rung bell, a stroke and the swing back), heard in every zone (`WORLD.md` §2.1). Across the open county it is `bell_far` (the high partials gone with the distance); indoors and underground it is `bell_within`, heard through walls and earth; where its ringer stands within hearing, it is `bell_near` from where he is. While it strikes, the music is `Bell`: a D minor hush six decibels under the matched loudness, a choir on the open fifth, the low strings answering with the bell's falling triad, and then the night's cue. A bed's sleep that jumps the clock past six still rings six on waking. On a Tuesday where `omen:early_bell` is true it rings at 20:50 and not again at nine. Once the Timekeeper is down (`bell_stopped`) it never rings again: at nine the music stops for eight seconds where the bell used to be, and at six nothing. **The ringer rings it** (`STORY.md` §8): in the School each of his phases is a strike of `bell_near` where he stands ("The bell goes"), and the School's own music tolls the same bell every other bar. The church at six (18:00, evensong) is a smaller, brighter bell, five quick strikes, heard only in the town (the Lowfields, the church, the Arms, the house); the Sunday train whistles at 17:02, a chord of three pipes a long way off, everywhere.

### 5.3 The cues

The score (`data/audio/songs`, 19 songs). All are matched to **-21 dBFS gated loudness** within 2 dB (`jane_audio::LOUDNESS`), so the player never reaches for the volume when the place changes; tempos are whole ticks a step, so the music keeps the game's clock. The theme's first phrase (up a fifth, lean on the sixth, fall to the second) runs through the whole county.

| Cue | Song | Key, time | Instruments | Mood |
| --- | --- | --- | --- | --- |
| Title | `title` | D minor, 4/4, 67 bpm | felt piano (or celesta), celesta echo, strings, harp, soft bass | The theme: a slow air falling from the fifth like the bell, lifting once into F and coming home unresolved |
| Lowfields by day | `lowfields_day` | G major, 6/8 | flute (or clarinet), fingerpicked guitar (or harp), warm pad, plucked bass, celesta | The town that was: the theme in the major, lilting, turning wistful through E minor |
| Lowfields by night | `lowfields_night` | E minor, slow | felt piano alone, dark pad, bowed glass (or choir), low strings, a wandering harp | After the bell: the theme's first notes in the dark, and once a loop the Neapolitan F that does not belong |
| Waters by day | `waters_day` | F Lydian | clarinet (or flute), flowing harp, glass, warm pad, celesta drops | Reeds and mist over still water, the raised fourth hanging in the air |
| Waters by night | `waters_night` | F# minor, a crawl | choir humming, glass, drone, a far clarinet, harp drops | Fog, something on the far bank, a phrase that never finishes |
| Works by day | `works_day` | C minor | harmonium hymn (or clarinet), staccato string wheel, low strings, frame drum, iron | The shift nobody comes to: a wheel turning, the works band's hymn |
| Works by night | `works_night` | C Phrygian | low-drum heartbeat, dark pad, low strings rocking C to D flat, sub, choir, iron | The night shift on Cinder Walk |
| House, Cellar | `home` | F major, 3/4 | felt piano, celesta (or small bell), piano waltz, strings, bass | Julie's lullaby: warm until the minor four turns it, a room someone is not in |
| Arms | `arms` | D major, 3/4 | harmonium (or flute), guitar oom-pah, plucked bass, strings | The inn lit till it is light: a slow waltz a little out of its time |
| Church | `church` | D minor, 4/4 hymn | organ, organ pedal, choir, treble organ | The hymn, closing each verse on a major chord it does not believe |
| Mine, Pipes | `deep` | A minor | drone, dark pad, harp drips, far choir, low strings | Under the ground: a drone that does not change for a long time, water falling in the dark |
| Burial | `burial` | E Phrygian dirge | low drum on the one, choir, low strings, small bell, drone | Goldskin's four favourites: a dirge leaning on the flat second |
| Forest | `forest` | E Lydian | flute, celesta wings (or harp), choir, glass, bass | The coloured things singing about day and night: beautiful and slightly wrong |
| Library, Museum | `halls` | B minor | harpsichord in two parts, warm pad, small bell | Cases and dust: an invention walking the circle of fifths, a little too careful |
| Factory | `factory` | C minor | low drums limping, iron off the beat, staccato riff, low strings, sub, dark pad | A machine climbing to a leading note that never gets its tonic |
| School | `school` | D minor, 3/4 | music box whose spring is going, the tower bell every other bar, choir, drone, celesta | Forty-one names marked present: a counting tune |
| Combat | `combat` | D minor, 112 bpm | low drums in threes and twos, frame drum, brushes, staccato ostinato, low and high strings, dark pad, sub | Something has hold of her: the bell's falling triad over a run, pressing to the leading note |
| Dead | `dead` (once) | D minor, slow | celesta, felt piano, strings, low strings | The theme's first phrase slowed and alone, stopping on the second degree as if it forgot the rest |
| Bell | `bell` (once) | D minor | drone, dark pad, choir, low strings | The hush under the strikes, 6 dB under the others (`under` in its row) |

**Seeded variation.** The county's seed picks each track's instrument from its alternatives and each song's form (the order of its sections) and patterns; every loop redraws its form and patterns from the seed and the loop's number, the notes marked `~` sound on some loops and not others, and a `drift` track (the night harps, the drips, the iron in the Works) wanders its chord's tones at seeded gaps. The same county always plays the same evening; another county another (`tests/music.rs`).

### 5.4 The synth (`jane-audio`)

Floats throughout (it is presentation, and nothing in it reads or feeds the sim); 48 kHz stereo, any rate the device gives.

- **Voices.** Two-operator FM with a second modulator for the tine or chiff, each index falling on its own clock, louder notes brighter (felt piano, celesta, music box, bass, glass, tick). Additive tables: harmonic amplitudes summed into a single cycle at boot, one table per harmonic count so a high note never aliases, up to four unison voices spread in pitch and across the stereo, optional breath noise and formant bands (strings, pads, choir, flute, clarinet, organ, harmonium, drone). Karplus-Strong strings with an all-pass tuning the fraction of a sample and a comb for where the string is plucked (harp, guitar, harpsichord, plucked bass), in tune to within 6 cents. Struck modes as decaying complex phasors, pairs beating (the tower bell, a small bell, iron). A pitched drum (a sine falling from above its pitch, a noise click), tuned like a timpano to the song's tonic. Filtered noise (brushes). Each through a state-variable filter (Simper's) with key tracking, a velocity and envelope sweep and a slow wobble, and an ADSR whose attack is a raised curve and whose release starts from wherever the attack had got to, so no note clicks on or off.
- **Sound effects** (`data/audio/sfx.json`, 53 patches): the outline's `SfxPatch { wave, pitch: (start, end), decay_ms, duty, vibrato }` grown into up to four layers, each a sine, triangle, band-limited saw or pulse, white or pink noise, FM, a plucked string or the tower bell, with an attack, a hold, a filter sweep, tremolo, a delay and a little figure of notes; rendered at boot into mono buffers (about 0.3 s for all of them), with up to four variants each a few cents apart so a walk is never the same step twice.
- **Beds**: made live, each a small generator (decorrelated noise through moving filters, and grains: drops, chirps, drips, pops, ticks), so they never loop audibly.
- **The sequencer** runs on ticks: a step is a whole number of ticks (a tick is 800 samples at 48 kHz), notes land on their sample with a few milliseconds of seeded humanising, chords are voiced near the track's octave so progressions move by the nearest step, and at most 48 voices sound at once (the oldest fades out in 8 ms).
- **The mix**: music, effects and beds each with its volume (smoothed, square-law from the 0 to 100 settings), one shared reverb (an eight-line feedback delay network under a Householder matrix, 2.4 s, each line darkening as it dies, after a pre-delay and two diffusers), a DC blocker and a 4 ms look-ahead limiter at -0.6 dBFS.
- **The pattern language** (`pattern.rs`): one token a step, like a tracker's column. Melody digits are degrees of the key; bass and arp digits are degrees above the chord's root (1, 3, 5, 7, 8), so an arpeggio follows the progression without being written again; `-` holds, `.` rests, `'` and `,` move an octave, `#` and `b` alter, `!` and `?` accent and soften, `~` is a maybe; chords are one token a bar (`b7`, `4m`, `5M7`, `1s4`, `6/7` for two in a bar). `build.rs` reads every file through the crate's own model and refuses the build on a pattern that does not fit its section, a missing instrument, or a list over its bound; the data sits outside the content hash, as `bindings.json` does.

### 5.5 The device

`jane-app` opens SDL2's audio (the `sdl2` crate's own subsystem, no other crate) at 48 kHz stereo with 1024-sample buffers and a callback that owns the `Engine`; the bus sends it commands over a channel, so neither side waits. With no device or no audio subsystem it prints one line and is silent; the game plays on. `jane serve` has no sound at all. The volumes are `config.json`'s `volume` (`{ "master": 80, "music": 70, "sfx": 80 }`, 0 to 100; effects cover the beds) and a row on the Controls screen: minus, value and plus for each, in steps of ten; with the keys, the row's left and right turn the lit one and confirm moves to the next. `jane serve`, the tests and a headless run use `NullBus`.

### 5.6 How it is checked

Nobody on the build can hear, so it is judged by numbers (`jane-audio/src/analysis.rs`) and by eye:

- `tests/music.rs`: every cue within 2 dB of its loudness, under the ceiling, no DC, **no clicks even 6 dB up** (a second difference far sharper than its neighbourhood's), heard in the key it was written in (a chroma against Krumhansl's key profiles; a mode may be heard as itself or its parent major or that major's relative minor), every note it played in its scale unless the pattern asked otherwise, every pitched instrument in tune at A3, every sound effect starting and ending at rest, no cue with more than 70 per cent of its power under 150 Hz (`analysis::balance`, which `jane audio check` prints for each: the score is warm and dark by choice, never drowned in its drums); a county's music the same every time and another's different; a cue change that fades to true silence. The first run found two real faults: letting go of a note in its attack stepped from the curve to the ramp (a click at every slow pad's chord change), and a low D's energy split between C# and Eb in too coarse a spectrum.
- Unit tests beside each piece: the polynomial sine, the envelope's edges, the string's tuning, the limiter's ceiling, the reverb's decay, the bell's inharmonic partials, every bed's level, the pattern and chord grammar; and in `jane-present`, the cue table's timings (2 s, 6 s), the bell's nine and six, the early bell and the silent one, the ringer's strikes, the fall, the fades over 24 cells, the stride and the surfaces, and the beds by sky, hour and place.
- `jane audio render <sfx:|bed:|inst:|song:|scene:name> [--png]` writes a WAV (and a waveform over a log-frequency spectrogram) under `sheets/audio/`; `jane audio check` prints each cue's loudness, peak, clicks and the key it is heard in against the key written, and the gain that would match it; `jane sheet audio` renders every patch, bed, song and three scenes (`nine`: dusk in the Lowfields, the bell and the night coming in; `six`; `combat`: a walk interrupted and given back).

## 6. Native viewer, sheets, film and bench

All are `jane-cli` subcommands drawing through `jane-render-soft` into an unshown canvas (or any backend with the `gpu` feature and `--backend`), reading back with `Backend::read_back` and writing PNG through the 60-line encoder in `jane-art::sheet` (no image crate, `PORT.md` §3.5).

| Command | Does |
| --- | --- |
| `jane view [--seed n] [--out dir]` | 24 skeleton cards and one county's detail at 5x with layers (land, regions or threat, day or night, walking times, every rule with its measured value, roads, rail, patches, anchors in gold), tables in the Small face (so it is also a font test). Never rasterises the full county. With `--out` it writes the cards as PNG; otherwise it opens a window through the `window` cargo feature of `jane-cli`, off by default so a Pi host builds `jane serve` with no SDL |
| `jane sheet county <seed> [--night] [--layer l]` | the skeleton card at 1x |
| `jane sheet county <seed> --full [--at mark] [--radius cells]` | the real chunk painter over a window of the county, all four layers side by side |
| `jane sheet dungeon <id> <seed>` | the dungeon's floor with names |
| `jane sheet scene --save <slot> [--tick n] [--night] [--tier t] [--backend b]` | one whole frame, headless, lighting, atmosphere and UI included; at T0 through `soft` it is the frame CI attaches; at T1 or T2 locally it is the frame the owner reviews. **As built** (no save slots yet): `jane sheet scene [--seed n] [--minutes m \| --ticks t] [--model reader\|rusher] [--night \| --hour h] [--wide] [--out file.png \| dir]` plays the seed from New Game with a player model as `jane play` does, the presenter ticking beside it every frame, sets the clock if asked (`--hour 18:40` lets the minutes pass with the world idle), and writes one frame (768 x 432, or 1008 x 432 with `--wide`) to `sheets/`: T0 through `soft`, or T2 through `wgpu` with `--backend wgpu` when `jane-cli` is built with its `gpu` feature. For the atmosphere's review (2026-09-27): `--at MARK` or `--at ZONE:MARK` travels there first, `--weather clear|mist|rain|storm` holds the sky (the ground wet as after an hour of rain), `--cast SPELL[:TICKS]` learns a spell and casts it east, the frame drawn so many ticks later, `--rows key=value,...` sets `Features` rows, and `--crop X,Y,W,H` with `--zoom Z` writes a close look at 3 or 4x; `tools/shots.sh` and `tools/at.sh` render the standard hours and a list of marks |
| `jane film --save <slot> --ticks N --out dir/ [--every k] [--tier t] [--backend b]` | N ticks from a save, one frame per tick at `alpha = 1`, a PNG per frame (or every k-th), the `FrameStats` table beside them; through `soft` at T0 on CI, any backend locally. The same tool over a trace range (`--trace`, `--from`, `--to`, `--clips`) is the film clip of `VERIFICATION.md` L7. Frames and scene sheets are how a lamp coming on at 18:30, a shadow swinging with the sun or the rain starting is looked at, not asserted |
| `jane sheet fx <spell>` | the effect's parts over 60 ticks, one column per tick, with its lights |
| `jane sheet ui [screen ...]` | **Built 2026-09-27.** The UI headless through the presenter and `soft` in states play rarely shows at once: `hud` (a fight's chips, target, three toasts, a flash, the lag), `dead`, `choice`, `tooltip`, `popover`, `drag`, `pause`; PNGs to `sheets/`. The UI's review sheet, as `sheet scene` is the world's |
| `jane sheet audio`, `jane audio render\|check\|list` | **Built 2026-09-27.** Every sound effect, bed, song and scene to WAV under `sheets/audio/`, songs and scenes drawn as a waveform over a spectrogram; `check` measures each cue (§5.6). The audio's review sheet |
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
| Audio and bindings | *(Built 2026-09-27)* `jane-audio` makes every sound; `data/audio` and `data/bindings.json` are checked and compiled in by their crates' build scripts (outside the content hash); overrides and volumes in `config.json` beside the saves | §4, §5 |
| The bell's rules | Read from the view in the presentation (the clock, `bell_stopped`, `omen:early_bell`, the weekday), mirroring `data/clock.json`; a sim `Bell` event would need a sound action in the schema's action vocabulary (the story's crates), so it waits for that. If the clock rows change, `Soundtrack::clock` must follow | §5.2 |
| The bell's count | It strikes the hour: nine at nine, six at six, 2.5 s apart; the church at six is a different, smaller bell | §5.2 |
| `MusicCue::Zone` | Carries the region, since the county is three places; a dungeon's cue ignores region and hour | §5.1 |
| Loudness | Every cue at -21 dBFS gated, within 2 dB; the bell's hush 6 dB under | §5.3 |
| Volumes | Master 80, music 70, effects 80 (beds follow effects), square law; a row on the Controls screen, not a screen of its own | §5.5 |
| Audio thread | SDL2's callback owning the engine, commands over a channel; floats; 1024-sample buffers | §5.5 |
| Saves and config | Slot files and `config.json` in `%APPDATA%\Jane`, `~/Library/Application Support/Jane` or `$XDG_DATA_HOME/jane`; beside the exe when a file called `portable` is there; `--data-dir` overrides | §3.2 |
| The window's height | 350, not 380: the HUD's bar stays visible below it as a drop target | §3.2 |
| The quest tab's name | Log (`VOICE.md` rule 8: no "quest" in the game's words) | §3.2 |
| The title | A backdrop painted once into a UI image with what moves drawn over it each frame, not the `Sky` and `FarLandmark` passes; the passes can take it over when §2.8's parallax lands | §3.2 |
| UI pointer | The OS cursor hidden; the UI draws the arrow, the hand and the reticle | §3.1, §4 |
| Stand-in icons | Drawn from the icon's name in `jane-present::ui::icons` until `ART.md` §2.5 lands | §3.1 |
