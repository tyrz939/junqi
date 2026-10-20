# PSP: 60 fps at the full look, fast world streaming, and fitting the hardware (rev 2, 18 Oct 2026)

This is a plan only. No game code has changed.

The owner asked for three things:
- streaming that is "much faster";
- the Media Engine used "for sound and ANYTHING else we can offload";
- in rev 2, **60 fps on the PSP at the full look**, with no degraded settings: every gap is closed by optimisation. Then "lightening fast … fly even in the small details": **about 10 ms or less of CPU and of GE in the busiest scene** (§0).

**Owner answers built in**
- **60 fps target:** 16.7 ms a frame, for the CPU and for the GE.
- **No fallback graphics settings.** A look change is allowed only where an A/B shows it reads at least as well (ART.md §3.1, "whichever reads better wins").
- **Strong compression:** heavy encode on the PC, cheap decode on the PSP.
- **Art seed pinned:** approved, so the county looks the same in every game.
- **Music stays on the tracker if at all possible,** with exact transitions. Pre-rendered audio is the last resort, waived for the PSP only (DESIGN-2020 §6).
- **The owner's PSP is on 5.50 Prome-4 CFW,** so kernel-mode and Media Engine code are available. They will test the ME work on hardware, as long as it is off the critical path. Every ME path keeps a user-mode fallback.

**Evidence**, all in `tools/psp-measure/ (scripts; capture decodes in progress/2026-10-18_82_hw-run/)`:
- `key.txt` and `decode{0,3,5,6}/stdout.txt` (the owner's PSP-1000 run, commit `39445fe`); `dist/JANE00001/perf.txt` (lakeside, t=700 s).
- Host measurements:

| Script | Output | What it measures |
|---|---|---|
| `cellstat.rs` | `cellstat-*.txt` | The whole county painted |
| `chunkcodec.rs` | `chunkcodec-out.txt` | Dump plus the domain coder |
| `codecs.py` | `codecs-out.txt` | zstd, lzma, zlib, bz2 on 1,024 real console chunks |
| `mixprof.rs` | `mixprof-out.txt` | Tracker cost and voice counts |
| — | `div64-scan.txt` | 64-bit divide sites |

The `.rs` files run when copied into `crates/jane-cli/examples/`, with `lz4_flex` as a dev-dep for `cellstat`. They are throwaway: never commit them.

---

## 0. The target: fly, not just 60

The owner: "i want it lightening fast, not just on target for fps. it should fly even in the small details."

So the bar is **headroom**: **at most about 10 ms of CPU and 10 ms of GE in the busiest scene** (night rain in town). That is 60 fps with about 40% to spare. There are two stages:

| Stage | CPU, busiest scene | GE, busiest scene | Reached by |
|---|---|---|---|
| A: 60 fps safe | ≤ 15 ms | ≤ 15 ms | §10 items 1–17 (expected 13.7 / 9.0 ms) |
| **B: fly** | **≤ 10 ms** | **≤ 10 ms** | Stage A, plus the ME mixer (−1.7), list ≤ 3 (static lamps fully cached, §6 L2), ge_build ≤ 1.5 (call lists for every static segment), presenter tick ≤ 1.2, draw + ui ≤ 1.4, W5 or W2 water. Expected about 10 ms CPU and about 7.5 ms GE |

### 0.1 The small details

Each has a target, measured on the owner's unit.

| Detail | Today (source) | Target | How |
|---|---|---|---|
| **UI pass** (GE `ui` 1.0–1.5 ms a frame; CPU quads 0.2–0.4 ms, PORT.md l.1214) | 1.0–1.5 ms GE | **≤ 0.4 ms GE** | One T4 font page (an open PORT.md item, half the bytes, one bind); glyphs batched per page; the HUD's quads kept as a GE call list while the UI is unchanged; no per-frame CLUT reload for UI pages |
| **Menus and pause** (the world is held but still drawn) | Full frame cost | **≤ 3 ms CPU, ≤ 1 ms GE** | On opening, the last world frame is kept as a texture: the framebuffer, copied once with `sceGuCopyImage` into the free lightmap/lamp VRAM while the world is held. Draw that as one quad plus the menu. Opening and closing within 1 frame |
| **Map screen** | **Opening is a 2.5 s frame** (chart built at once, PORT.md l.1152) | **Opens in 1 frame; pans and zooms at 60** | The world is fixed: **bake the county chart at build** into the jpk (T8 pages). At runtime only the fog of war (a 1-bit mask, 96×96 or cell-level) and the pins are drawn. Pan/zoom as GE quads |
| **Boot to title** | 0.1 s emulated; hardware not measured | **≤ 1.5 s on hardware** | Title first. The jpk tables and the jau's 806 KB resident part are read async behind the title; audio starts when it's in |
| **Load / New Game to play** | "Reading the county" about 0.5 s emulated; **then a 2.34 s first-tick freeze** (NEXT-2026-10-10 l.186) | **≤ 1 s, no freeze** | `county.bp` read async. The first tick's work (activation, packing, her place, the first view) is done **under the loading screen** |
| **First-frame priming** | First view painted on the first tick (3 s, PORT.md l.1214) | **Frame one of play is a normal frame; no hitch in the first 5 s** | Under the loading screen: decode the first view's baked chunks (12 × 3 ms), load the view's atlas pages, warm the lamp cache and the call lists |
| **Zone transitions** (doors) | 12–60 ms warm, up to 1.3 s cold; the first view outside the county painted at about 1 chunk/s | **≤ 1 frame of stall inside the fade; full detail at fade-in** | §9: baked zones, prefetched by the ahead logic; the fade covers ≤ 150 ms |
| **Teleport / fast travel** | Up to seconds of stand-ins | **Full detail ≤ 100 ms after arrival** | Baked chunks (§5), nearest first, micro stand-ins for at most a few frames |
| **Save** | Not measured | **≤ 1 frame on the game thread** | Serialize (postcard + lz4, already) into a buffer. A writer thread does `sceIoWrite` and the rename; the game keeps running. Show "Saved" when the thread reports |
| **Input latency** (press → photon) | Pipelined 30 fps: about 2.5–3 frames (≈ 85–100 ms; estimate) | **≤ 2 frames at 60 (≤ 33 ms)** | Read the pad as late as possible before the step; the pipelined GE adds one frame. Measure by filming a debug "flash on press" at 240 fps |
| **Sound latency** (event → speaker) | Ring drained per 512-frame block (23 ms), plus up to 3 queued blocks: **≤ about 90 ms** | **≤ 35 ms** | 256-frame blocks, 2 to 3 queued. The per-block overhead is small once `CHUNK` is 64 (§7) |
| **Spikes** | 50–140 ms frames (streaming, lamp builds, uploads, page loads) | **No frame over 20 ms in the bench scenes** | Q1–Q7, §5, §6 L2 |

---

## 1. The 60 fps budget

### 1.1 Tick rate

`jane_core::num::TICK_RATE` = **60**. At 30 fps, `n_ticks` ≈ 2 a frame.

- **At 60 fps it is 1 tick a frame,** so the per-tick work becomes per-frame work at half today's per-frame cost.
  - The per-tick work is `sim`, and the presenter's `tick` and `bufs`: `spikes/psp-game/src/main.rs` l.1813 sums `shell.times` per stepped tick.
- **The per-frame work stays the same.** That is `draw`, `ui`, `list`, `ge_build`, and the GE, which redraws every frame.
- **Audio is per second,** so its share of a 16.7 ms frame stays the same percentage.
- The sim stays at 60 Hz. Changing it would move every timing and the bot fixture.
- The presenter already interpolates the camera. At 60/60 there is nothing between ticks to interpolate, and no "presenter faster than the sim" mode is needed.

### 1.2 How the CPU and the GE overlap

Pacing is `Locked30 pipelined`: the GE draws frame N while the CPU builds N+1. At 60 fps:
- **CPU ≤ 16.7 ms and GE ≤ 16.7 ms, each on its own,** with about 10% headroom, because a frame that misses by 0.1 ms costs a whole vblank. So the targets are **15 ms CPU and 15 ms GE**.
- The GE texturing from RAM (chunks, RAM-held pages) shares the bus with the CPU. That is one more reason to keep fill low.

### 1.3 Per scene: now, at 60 fps pacing, and after this plan

"Now @60" puts today's measured costs into a 60 fps frame: per-tick costs halved, per-frame costs unchanged, audio at its measured share of 16.7 ms.

| ms | Day county (cap 0) now @60 | → after | Night town (cap 3) now @60 | → after | Night rain (cap 6) now @60 | → after | Lakeside (perf t=700) now @60 | → after |
|---|---|---|---|---|---|---|---|---|
| sim (1 tick) | 1.0 | 1.0 | 0.8 | 0.8 | 1.1 | 1.1 | 1.0 | 1.0 |
| presenter tick + bufs | 3.3 (mean; p50 1.9) | 1.4 | 2.2 | 1.5 | 4.1 | 1.9 | 1.8 | 1.3 |
| draw + ui | 2.1 | 1.6 | 2.5 | 1.9 | 2.9 | 2.1 | 2.3 | 1.7 |
| list | 3.1 | 2.0 | 11.6 | 4.0 | 9.6 | 4.0 | 5.3 | 3.0 |
| ge_build | 2.9 | 1.5 | 5.6 | 2.5 | 5.6 | 2.5 | 6.5 | 2.5 |
| audio (share × 16.7) | 3.4 (20%) | 1.8 (11%) | 3.3 | 1.8 | 2.2 | 1.8 | 2.4 | 1.8 |
| streaming / worker | (spikes 20–90) | 0.3 | — | 0.3 | — | 0.3 | — | 0.3 |
| **CPU total** | **15.8** | **9.6** | **26.0** | **12.8** | **25.5** | **13.7** | **19.3** | **11.6** |
| CPU with the ME mixer (§7.4) | | 7.9 | | 11.1 | | 12.0 | | 9.9 |
| **GE total** | **7.0** | **3.4** | **9.5** | **5.6** | **18.8** | **9.0** | **15.8** | **6.6** |

**GE after**, in detail:
- **Sun shadows:** 4.0–5.0 → about 1.3 (§3).
- **water_fx:** lake 8.1 → about 3.0; rain 9.7 → about 3.5 (§4).
- Everything else unchanged: lightmap 1.7–2.2, ui 1.0–1.4, terrain about 0.5.

**The coordinator's guesses, corrected:**
- **CPU:** day 8–10 is about right (9.6, or 7.9 with the ME). Night town comes to 12.8, or 11.1 with the ME. **Night rain (13.7) is the tightest scene,** not night town.
- **GE:** about 3.5 to 6.5 ms in ordinary scenes (lower than guessed, once the shadow fill fix is in). The lake is 15.8 ms before the water fix, not 13, and about 6.6 after. Night rain is 18.8 ms before, the worst GE scene, and about 9 after.

### 1.4 Per item: costs and how to reach the target

| Item | Today per 60 fps frame | Target | How | Effort | Risk |
|---|---|---|---|---|---|
| sim | 0.8–1.1; worst ticks 6–12; mob worst tick **56 ms** (commit d503ad3d) | ≤ 1.2; worst ≤ 8 | Use the sim's phase timers (`metrics().phase_ns`, logged per window). Find the 56 ms tick's phase. Spread it across ticks without changing results. Remove the 64-bit divides (§2 Q5). **The bot fixture hash must not move** | M | Med |
| presenter tick | 1.9–4.1 | ≤ 1.9 | `t_props` cached (§2 Q3); `t_ambient` (1.5 ms by day in cap 0) profiled the same way; paint landing as a slot swap (§5) | S–M | Low |
| draw + ui | 2.1–2.9 | ≤ 2.1 | Profile first. Cache UI quads while the UI is unchanged | S–M | Low |
| list | 3.1–11.6 | ≤ 4 | §6: cached static shadows, lamp work and patches | M–L | Med |
| ge_build | 2.9–6.5 | ≤ 2.5 | §6 L3, static call lists; fewer binds (§2 Q6); water quads cut (§4) | M | Med |
| audio | 20% = 3.4 | 11% = 1.8 (then about 0.1 with the ME) | §7 | M | Low (opt.) / High (ME) |
| streaming | 20–90 ms spikes | 0.3 average, no spikes | §5: baked chunks with zstd | L | Low |
| GE sun shadows | 4.0–5.0 | ≤ 1.5 | §3 | S | Low |
| GE water_fx | 8.1–9.7 | ≤ 3.5 | §4 | M | Med |

### 1.5 What might still miss 60, and how it gets closed

Graphics settings are not an option here.

1. **Night rain in town:** CPU 13.7, GE 9.0. The tightest scene.
   - If it misses, the next levers are the lamp lightmap work (cache static lamps fully) and the ME mixer.
2. **Sim spikes in mobs** (a 56 ms tick).
   - Closed by sim profiling and spreading work across ticks inside the same results: path work already has a budget, so find what else spikes.
   - Until then a mob spawn may drop frames.
3. **Teleports and zone entries:** about 12 chunks wanted at once.
   - Streaming gives 2 to 3 ms of decode each. The view fills in about 50 ms of worker time across frames, with micro stand-ins meanwhile (§2 Q2).
4. **Audio spikes** (§2 Q7).

---

## 2. Quick wins now

These are ordered by saving per unit of effort. Each was checked against the code.

| # | What | Checked in code | Expected saving | Size | Who |
|---|---|---|---|---|---|
| **Q0** | **Measure first:** GE sub-pass marks inside sun shadows (clear, raised, polys, bands) and water_fx; list sub-timers; audio per block (worst block µs, blocks a frame); sim phase worst | GE passes are one mark each today (`capture::pass`). `list.rs` has `pass_mark`; the tracker has `prof[4]` laps, not captured | Enables everything below | S | Sonnet |
| **Q1** | **Sun shadow fill** (§3): clip the raised marks to the canvas and to the cells with relief; replace the stencil clear quad with the frame's `sceGuClear` stencil bit | §3 | **GE −2.5 to −3.5 ms** in every outdoor scene | S | Opus (GE) |
| **Q2** | **Stand-ins:** one sampled stand-in a tick, the rest a GE micro texture | `present.rs` ~l.2363: every on-screen missing chunk gets `paint_chunk_sampled_t8` in the same tick (about 20 ms each on hardware) | t_paint max 50–97 ms → about 20 ms; the micro texture brings it to about 0 | S (cap); M (micro) | Sonnet; Opus |
| **Q3** | **t_props** | `present.rs` `read_props` (l.1626) rebuilds every visible prop's `PropRec` each sim tick: catalog and string lookups (`gathers.contains`, `cat.sprites` names), `light_showing` + `wrong_lamp`, house and tile lookups, rugs, quest marks; then `stand_on_tops`, O(props × tops) | Cache `PropRec` per prop id. Recompute only on prop state change (on/used/loot/solid/carry) or on a lamp or night-stage change. Night 1.1–2.8 ms a frame at 30 fps → about 0.3 | S–M | Sonnet |
| **Q4** | **Page uploads** | `ge.rs` `bind_page`: CPU `copy_nonoverlapping` of up to 66 KB to VRAM through the uncached mirror, mid-list. Cap 6 has about 100 uploads in 600 frames, on 35–55 ms frames | Use `sceGuCopyImage` (`vendor/psp/src/sys/gu.rs` l.2768) from the RAM page, already written back: the GE moves it. CPU −1 ms an upload (estimate: uncached stores at 50–100 MB/s); about 0.2 ms average, spikes −2 to −5 ms | S | Sonnet, Opus review |
| **Q5** | **32-bit maths audit** | 170 sites with 64-bit divide, remainder or `isqrt` (`div64-scan.txt`). On a `mips2` target an i64/u64 divide is `__divdi3`/`__udivdi3` (100+ cycles); 32×32→64 `mult` is cheap. Hot ones below | Exact 32-bit replacements | S–M | Sonnet/Grok |
| **Q6** | **Binds and CLUT loads** | `ge.rs` already skips a rebind when the batch key repeats. Binds (84–172 a frame) come from y-sorted sprites interleaving pages; every page bind reloads its own 1 KB CLUT (110–190 loads) | (a) Reorder within overlap-free windows: two sprites whose rects don't overlap may swap, so sort by page inside each window, done in the lister and testable on the PC. (b) The bake packs co-occurring sprites (a scene's props, a unit's frames) onto shared pages. About GE −0.3 to 0.8, CPU −0.3 | M | Opus |
| **Q7** | **Audio spikes** (`audio` max 44 ms in cap 5; frames of 100 ms show about 20 ms) | The host replay finds no slow block caused by a command (`mixprof-out.txt`: the slow blocks are OS noise). `F::AUDIO` is mixing time summed per frame | Q0's per-block capture tells one slow block from catch-up after a stall. Suspects: preemption by kernel IO threads during the game's sync `sceIoRead` (page loads); the far-effect load path (`jane-audio-psp` `load_far`, under `sceKernelSuspendDispatchThread`); a long frame holding 2 to 4 blocks. Move IO to async (§8.9); then re-capture | S | Sonnet |
| **Q8** | **Bug: a horizontal fence shades itself in her lamp** (owner report) | See below | — | S–M | Opus |

**Q5's hot sites**
- **Painter (runtime fallback only after §5):** `jane-art/src/terrain/ecotone.rs:159`, an i64 divide **per px** in ecotone chunks. About 65k × 100+ cycles is about 20 ms a chunk on hardware.
  - `terrain/hard/dungeon.rs:784,796,1411,1422` and `terrain/dungeon.rs:548-550`: `isqrt(u64)` and i64 divides per px or per cell in dungeon floors.
- **Lister and presenter, every frame:** `jane-present/src/shadow.rs:350,491-521,1105`, divides and `isqrt` per lamp slab (`n_slabs` 150–770 at night, so about 0.3 to 0.6 ms).
- **Sim** (cheap but per unit per tick):
  - `walk.rs:259-269` and `ai.rs:1048-1058,1442`: `isqrt(u64)` plus two i64 divides per moving unit;
  - `combat.rs:89,129-130`.
- **`jane_core::num::isqrt` itself:** a Newton loop with a **u64 divide per iteration**, about 5 iterations, so about 700 cycles.
  - Fix: a divide-free bit-by-bit root, plus a u32 fast path when `n < 2^32`. It is exact, so the sim results are unchanged by construction.
- **Tracker:** its u64 work is multiplies and shifts once per 32-frame chunk, so it is fine.

**Q5's fix rules**
- Use i32 where the range is proven: add `debug_assert!`s and keep the proof in a comment.
- A divide by a constant becomes a multiply by its reciprocal plus a shift, only where exact.
- Keep 64-bit off the hot paths.

**Q5's guard**
- A test script disassembles the PSP ELF (`llvm-objdump` from `rustup component add llvm-tools`).
- It fails if any function on a hot list (`isqrt`, `shadow::*slabs*`, `walk::*`, `ai::step*`, `Mixer::*`, `Lister::*`) calls `__divdi3`, `__udivdi3`, `__moddi3` or `__umoddi3`.
- It also ratchets the total number of call sites.
- Every sim edit is checked with `jane hash` and the bot fixture: **unchanged, or the edit is wrong.**

**Q8, where the fence self-shadow most likely comes from**
- Fences are painted into the ground px (`jane_art::terrain` `Standing::Placed`; `Chunk::fence_px` marks those px).
- Their shadows come from their parts as `Block`s (`terrain::fence_blocks`). A lamp throws them with `jane_present::shadow::block_slabs` (shadow.rs l.677). Its doc says a fence part "also throws its underside … and its top".
- For an E-W rail with the lamp to the south, that slab lands north of the rail's foot. That is exactly the screen rows the rail's own painted px occupy, so the fence darkens itself.
- **The sun avoids this:** reach 1, plus T2's fence bits let the sun "pass them". But `Chunk::fence_px` says "the sun passes them by …, a lamp does not". So the lamp self-shadow is in the shared slab code and **likely shows on the PC tiers too**.
  - The PSP has no fence layer at all: `ChunkLayers::new_t8` keeps none. So the PSP could not mask it even for the sun.
- **Check:** `jane sheet scene` at night beside an E-W fence with her lamp, on soft/gl2/wgpu, and a PSP shot.
- **Fix direction (later):**
  - start a fence part's lamp slab beyond its own drawn rows (reach 1, as the sun's);
  - or mask fence px out of lamp shadows. On a console that is a free CLUT alpha tag (`T8_FENCE`, like `T8_WATER`), marked into the stencil before the lamp shadows.

**Done (2026-10-19): Q0's GE part, Q1, Q4, Q8**
- **Q0:** sub-passes `sun_shadows.{clear,raised,polys,bands}` and `water_fx.{clear,wet,marks,sky,crests,mirror,streaks}` (`capture::pass`, `Lister::sub_marks`). The parent's time still holds them; `jane psp-profile` lists them and does not stack them twice. They are signalled only when timed and are never read by the list guard.
- **Q1:** each chunk's relief cells (height over `GROUND`) are read once per generation as rects of cells, at most 24, else one span a row. The raised marks are clipped to them and to the canvas. The stencil clear is the frame's `sceGuClear` stencil bit whenever nothing before the pass touches the stencil; the GE keeps the alpha unwritten until then (`sceGuPixelMask(0xff000000)`). The band cap counts as before. PPSSPP shots are byte-identical (town 17:22, wood 12:00, rain), and a host test proves the clipped marks set the same stencil px as whole chunks. PPSSPP does not time fill, so the saving needs a hardware capture.
- **Q4:** `bind_page` uploads with `sceGuCopyImage` (512-px rows of 32-bit, then the rest), then `sceGuTexSync` and `sceGuTexFlush`. A page not 16-byte aligned falls back to the CPU copy. Shots stay identical across frames with uploads.
- **Q8:** this was PSP-only. T0 and T1 weigh the lamp shadow's reach (1) against the height layer, so fence px (≥ `FENCE_FLOOR`) never took it. The PSP lightmap multiplies every px. Now a fence's lamp shadow is cleared off the px its fence draws over (`shadow::fence_drawn`: its columns, from the foot up `rows_up(FENCE_TOP)`) before the other casters' shadows are laid. This applies to her lantern's stencil marks and to the cached lamp textures.

**Done (2026-10-20): Q6, and §6's list and GE build work (phase row 13)**
- **Binds and CLUT loads (Q6):**
  - A page binds through one CLUT: the graded one, its glow CLUT or the UI's white one, not its own and then that one. A graded CLUT already made for the frame's grade is not copied again.
  - `clut_load` skips the CLUT the list loaded last (nothing a list reads is rewritten after its first load).
  - The untextured kinds (flat quads, polygons, lines, flat strips) are one texture state, so a lamp's stencil clear and its marks take one bind.
  - **Q6a:** the sprites' quads are grouped by texture where no two that touch swap (`list/order.rs`; a host test holds every touching pair to its order). The particles and glows were tried too: 2 to 4 binds for twice the cost, so they are off (`order:N` in a script names the passes).
- **GE build:** the stall address moves every 6 KB of list, not at each `sceGuGetMemory` and draw (two syscalls a batch; a two-function patch to the vendored rust-psp). Vertices are written to the uncached list as whole words.
- **List:**
  - **L4:** a patch scans only the chunk's relief cells.
  - **L1 (part):** a block's sun shadow is its swept rect's hull laid from its six edges, not a sort of eight points (a 200,000-case host test against the sort). Town at night: `silh_blocks` 0.81 to 0.25 ms on PPSSPP.
  - What stands still round the lights is gathered only when a light is looked up.
- **Not done:**
  - **L3 (call lists).** Through-mode vertices are screen px, so any camera move rewrites them; only an exact replay is possible. It was tried on the UI and dropped: in play its quads change about every other frame, and when they did not, its pages had moved between VRAM slots.
  - **L2** is the lamp cache already. Her lantern's block shadows (`lamp_block_poly`, about 1.1 ms a frame on PPSSPP in town at night, about 95 blocks) are the largest single piece of the night list left.
- **PPSSPP, the bench places (tpf:2, audio off, means over 600 frames):**

| Place | binds | CLUT loads | ge_build ms | list ms |
|---|---|---|---|---|
| Town 21:30 | 110 → 101 | 137 → 42 | 1.55 → 1.25 | 3.86 → 3.59 |
| Wood 12:00 | 95 → 74 | 146 → 36 | 1.67 → 1.24 | 3.75 → 3.81 |
| Lake 15:00 | 106 → 87 | 142 → 45 | 1.76 → 1.30 | 2.10 → 2.15 |
| Wood, relief on (audio on) | 373 → 200 | 548 → 162 | 4.01 → 1.97 | 4.18 → 4.43 |

- The shots are byte-identical: 3 per place, also with relief on.
- PPSSPP doesn't time the GE or syscalls as the hardware does, so the gates (night list ≤ 4 ms, ge_build ≤ 2.5 ms) need a hardware capture.

---

## 3. Sun shadows (4.0–5.0 ms GE, 43–71% of the GE in every outdoor scene)

**How it works** (`jane-render-psp/src/list.rs` `silhouettes`, about l.1942–2096; GE modes in `ge.rs` about l.1473–1530):
1. `StencilClear`: a **full-canvas quad** writing stencil 0 under `sceGuPixelMask(0x00ffffff)`. That is a read-modify-write of every px.
2. `RaisedMark`: **one quad per placed chunk, the whole 256×256 chunk, not clipped to the canvas** (`(cx, cy, cx+CHUNK_PX, cy+CHUNK_PX)`, l.2060). The chunk's height is textured as T8 through a CLUT mask, and raised px are written to the stencil.
   - With 6 to 9 chunks placed, that is 390k to 590k px rasterised against a 130k px canvas.
3. `ShadowBand`: the casters' polygons, then the band rects at up to 16 strength levels, strongest first. Each multiplies where the stencil is clear and sets it.
   - **There is no separate full-screen "shade" pass:** the bands shade as they mark.

**What the data says**
- Cap 0 (day): 25 casters, **0 lamp slabs**, and still **4.96 ms**.
- Night caps: `n_slabs` 145–770 are *lamp* slabs, in the lightmap pass, not the sun's.
- So the sun pass cost is **fill**, not casters.
- Estimate at about 150 Mpx/s for a textured read-modify-write: clear about 0.9 ms, raised marks about 2.5 to 3.5 ms, bands about 0.5 to 1 ms. That adds to about 4.5, which matches.
- Q0 adds sub-pass marks to confirm before anything else changes.

**Options, ranked**

| Option | GE saving | Effort | Look |
|---|---|---|---|
| **A. Clip `RaisedMark` to the canvas,** with UVs (the water pass's `chunk()` helper already does this), and **only over the cells with relief.** 83% of cells have flat height (`cellstat-county.txt`). Per chunk, a list of relief rects comes from the bake (§5), or from `Terrain::land` today | **−2 to −3 ms** | S | **Identical** |
| **B. Drop the clear quad.** Keep the alpha (stencil) channel untouched by everything drawn before the shadows: `sceGuPixelMask(0xff000000)`, or alpha-preserving blend states. Let the frame's opening `sceGuClear` clear the stencil, which costs about nothing in clear mode. Audit every draw before the pass for alpha writes | −0.8 | S–M | Identical |
| C. Skip the pass when every band would be under strength 16 (deep dusk, overcast). It already returns when `shear` is None | −4.5 in those scenes only | S | Identical (it skips only the invisible) |
| D. Cache static shadows per chunk at quantised sun steps: CPU-rasterised T4 masks in RAM, 32 KB a chunk; only moving casters drawn live | GE −0.5 more after A+B (one masked quad per chunk replaces the bands); CPU list −1 to 2 ms (§6 L1) | L | Identical if quantised to the shear's Q8 step |
| E. 16 levels → 4 | About −0.2 (fewer overlapping rects) | S | **Worse:** the long dusk shadows' falloff bands. **Not recommended** |
| F. Half-resolution mask, scaled up | −1 to −1.5 after A+B | M | Softer, 2-px edges. **Changes the look:** only on an A/B win |
| G. Fold into the lightmap multiply (draw the shadows into the 256×140 lightmap target) | Removes the band pass: −0.5 to −1 | M | Half-resolution shadows, as F. **A/B only** |

**Recommended first step:** Q0's sub-pass marks, then **A + B** in one change.
- Expected: 4.5 to 5.0 → **about 1.3 to 1.8 ms**, identical pixels.
- Gate: a PPSSPP frame dump byte-identical before and after, plus a hardware capture.
- Then D, as part of the list work (§6).

---

## 4. water_fx (8.1 ms by the lake, 9.7 ms in rain): the GE blocker at 60

**What it draws** (`jane-render-psp/src/list/water.rs` `Lister::water_fx`). Every masked pass covers **the bounds**: the bounding box of every water cell in view, or **the whole canvas in rain** (`pools`).
1. `StencilClear` over the bounds.
2. **Rain only:**
   - wet and shine marks per chunk, through the chunk's CLUT tags;
   - two multiply passes and two add passes over the bounds;
   - a puddle noise mark;
   - the puddle multiply and a sky mirror.
3. **Water:** the `T8_WATER` mark per chunk; then **every sprite quad drawn so far that overlaps the bounds is drawn again** to unmark the stencil (l.205–218).
4. `sky_mirror`: 16-row strips over the bounds, alpha blend.
5. `crests`: **one 1-px-tall quad per run per row**, up to 272 rows × runs. The troughs follow as a second batch.
6. `far_mirror`: the parallax things mirrored.
7. `mirror`: every standing sprite over the water, re-drawn flipped in **2-row bands, each its own quad** (`BAND = 2`).
8. `streaks`: lamp glints.

**Where the 8 ms goes (estimate; Q0 measures it).** By the lake, the bounds are most of the canvas: about 5 full-bounds fills (clear, mark, sky, crests and troughs, mirrors) plus the unmark redraw and thousands of small quads. Small quads pay per-primitive setup on the GE and command bytes on the CPU (lakeside `ge_build` is 6.5 ms).

**Options**

| Option | Saving | Effort | Look |
|---|---|---|---|
| **W1. Tight bounds:** mask passes over the water cells' merged row spans per chunk, not one box. A diagonal lake fills a big box | −25 to −40% of the pass | S–M | Identical |
| **W2. Crests as a texture:** one masked pass of a small scrolling crest/trough texture (a T8 noise band, like the puddle noise `Tex::Noise` already used), replacing hundreds of 1-row quads | −1 to −2 GE, −1 to −2 CPU (`ge_build`) | M | Near-identical; the rows' motion matches if the texture is built from the same `isin` terms. A/B |
| **W3. Mirrors in 4-row bands,** where the ripple shift is equal across adjacent bands (merge bands with the same `dx`) | −0.5 to −1 GE/CPU | S | Identical when merged only where equal |
| **W4. Mark and sky in one pass:** draw the water mark quad with the sky gradient as vertex colour, using a texture function that keeps alpha from the chunk CLUT's water tag. Removes a full-bounds pass | −1 | M | Identical (verify the GE texture-function combination) |
| **W5. Crest phase in the CLUT:** bake water px into 8 row-phase indices (8 × about 5 water colours; checked against the 256-entry limit per chunk, max 211 used today). Animate the crests by **cycling those CLUT entries**, which the per-frame CLUT grading already rewrites (`ge.rs` ~l.727) | Crests at zero GE fill: −1 to −2 | M (bake + CLUT) | Very close; moving bands as CLUT cycling. A/B against W2 |
| **W6. Unmark without redrawing:** draw the water fx *before* the standing sprites, so nothing needs unmarking. Needs care with the lightmap order (T1 draws the water over the light) | −0.5 to −1 | M | Must match T1 (PC parity) |
| W7. Half-resolution reflections into an offscreen target | −2 to −3 | M | Softer reflections. **A/B only** |
| — Animate at a lower rate | **No GE saving** (the GE redraws every frame); CPU only | — | — |

**Recommended:** W1 + W3 first (S, identical). Then W2 or W5 (A/B), then W4.
- Expected: lake 8.1 → **about 3.0 ms**; rain 9.7 → **about 3.5 ms**.
- Lakeside GE total 15.8 → about 6.6; night rain 18.8 → about 9.0.

---

## 5. Ground streaming: baked chunks, strongly compressed

### 5.1 Why bake

Measured on the whole county (`cellstat-county.txt`):
- 75% of 16×16 cells are unique (1.77 M of 2.36 M);
- 39% of 8×8 blocks are unique;
- 215k 3×3 tile-context classes.

So a GE tile atlas can't reproduce the look: "The grid never shows" (ART.md §3.1). **Baking the console's own paint output offline is pixel-identical.**
- It replaces about 100 ms of PSP CPU a chunk with a 2 to 3 ms decode.
- It removes stand-ins in normal walking, and the 4 ms landing copy (slot swap).
- What a console chunk carries, who reads it and the PSP path are unchanged from rev 1: `ChunkLayers::new_t8`, `Terrain::land` (terrain.rs l.922), the sidecar (placed, blocks, windows, sockets, bases, falls, water, glow), and the loader-thread / worker-decode / slot-swap design.
- The **art seed pin** is approved. `view.seed()` currently feeds the painter, `Houses::fill`, `room_of` and `dungeon_of`, and is replaced by a constant per zone. It is presenter-only, so `state_hash` and the bot fixture are unchanged.

### 5.2 Compression on 1,024 real console chunks

Sample: every 3rd chunk each way. Test set disjoint from any training set. Indices use each chunk's own frequency-sorted palette; heights are made relative (`terrain::relative`).

| Codec (per chunk, independent) | idx KB | height KB | Host decode µs (idx) | PSP decode estimate | County MB (+0.2 CLUT, +0.5 sidecar) |
|---|---|---|---|---|---|
| lz4 (block; today's save codec) | 13.9 | 2.2 | 37 | about 0.7–1.5 ms | **about 157** |
| zstd -3 | 7.82 | — | 43 | — | — |
| **zstd -19** | **6.04** | 0.60 | 54 | **about 2–3 ms (C), 3–5 ms (pure Rust `ruzstd`)** | **about 66** |
| zstd -22 (ultra) | 5.99 | 0.60 | 59 | same | about 66 |
| zstd -22 + 112 KB trained dictionary | 6.07 | 0.66 | 68 | — | No gain |
| zstd -22, a neighbour chunk as prefix | 6.04 | — | — | — | No gain, and breaks random access |
| zstd, idx + height in one frame | 6.65 joint | | | | No gain over separate (6.59) |
| neighbour-match transform + zstd -22 | 6.44 | — | 53 + pass | — | Worse than plain zstd |
| zlib -9 | 6.54 | — | 113 | — | — |
| bz2 -9 | 6.47 | — | 684 | Too slow | — |
| lzma -9e (lc0 lp0 pb0) | **5.69** | 0.63 | 390 | about 15–30 ms | about 62 |
| **Domain coder** (`chunkcodec.rs`): each px coded as "equals left / up / up-right / up-left" or a literal; 32 contexts; static rANS trained on the PC | **5.13** | **0.50** | 775–1020 | about 30–60 ms | **about 58** |
| Order-1 (left, up) entropy bound (rev 1) | 3.6–4.7 | — | — | — | Bound only |

**PSP scale factors.** The painter runs about 50× slower on the PSP than on the host (2 ms vs about 100 ms). The tracker runs about 130× slower (32 µs vs about 4.2 ms a block). Memcpy-heavy LZ decoding sits nearer the low end. Q0 and P0e measure the real numbers with a chunk decoded 100× on the owner's unit.

**Pick: zstd -19 to -22, one independent frame per layer per chunk, no dictionary.**
- About **7.3 KB a chunk, about 66 MB for the county** (lz4 is 157).
- About **2 to 3 ms of worker decode** with the reference C decoder, compiled decode-only. It needs a C cross-compile for `mipsel-sony-psp`; otherwise use `ruzstd` (`no_std` + alloc) at about 3 to 5 ms.
- Decoder state is about 100 KB, allocated once.
- **Why not the strongest:** LZMA saves 4 MB (6%) for about 10× the decode. The domain coder saves 8 MB (12%) for about 20× the decode: per-px entropy decoding on a 333 MHz in-order core costs tens of ms. The county's 2.3 chunks/s sprint could afford even that (about 0.5 ms a frame on average), but a teleport's 12-chunk burst would take about half a second. zstd's sizes are within 12% of the best measured, at about LZ speed.
- **Decode budget:** ≤ 3 ms a chunk on the worker. That is about 7 ms per second of sprint, under 0.15 ms a frame at 60.
- **CLUT:** 194 B raw; zstd doesn't help. Optionally store CLUT entries as u16 indices into a 663-colour county table, which saves about 100 B a chunk (0.9 MB).
- **Sidecar:** about 1 KB postcard, about 0.5 KB after zstd (estimate; measure at the bake).

**P1b measured (2026-10-20, `jane bake-chunks`, all 9,216 county chunks, console path):**
- zstd -19: **66.9 MB** (a chunk: 6.4 KB albedo, 0.60 KB height, 0.40 KB sidecar of 0.81 KB raw). Baked in 55 s on 6 threads.
- lz4 HC 12 (`--codec lz4`): **95.0 MB** (8.8 / 1.2 / 0.5 KB). Baked in 30 s.
- Every chunk decodes (`ruzstd`, `lz4_flex`) bit for bit to the paint (`bake-chunks --verify`, 4 to 6 s).
- The pack is not committed: `tools/psp-dist.sh` paints the county (about 4 to 6 s) and bakes again only when its digest moved. A committed key (`assets/world/county.jck.key`, the digest of about 70 probe chunks) fails the slow tier when the art changes without `bake-chunks --key`.
- Found on the way: a chunk landed in the tick (a zone's first view, an edge chunk) tags wet ground in the CLUT's alpha by its heights before they are made relative, so on raised ground its alpha differs from the worker's (its colours are the same). The bake follows the worker.

**P1c done (2026-10-20): the PSP reads the pack.**
- `county.jck` ships in the zip (stored, not deflated). At boot the game reads its index (36 KB) and starts a loader thread (priority 40: below the game, above the painter).
- The presenter asks (`Present::baked_wants`): the band's chunks not landed, then up to 6 read ahead (beyond the slots, or a chunk further along her way). The loader reads blobs with `sceIoReadAsync` into a 6-entry blob cache, decodes one at a time (`Decoder::decode_landing`) into a spare slot's layers, and posts it. The game swaps it into the cache's slot (`Present::land_baked`): no read, decode or 128 KB copy on its thread.
- The painter keeps every chunk within 8 cells of a tile or span that is not the blueprint's (`View::tile_deltas`, `spans_changed`), any chunk the loader failed on, and every other zone. A baked chunk takes no stand-in ahead and no upgrade; one on screen and not landed still gets the stand-in.
- The console band ranks off-screen chunks by their gap to the view (a gap it moves away from counts 4×), not by distance to a point ahead: a turn's side row was being left out and stood in.
- Not done: the GE micro-texture stand-in (Q2 M part). With the bake it is rarely needed.
- Still open: the first view of a New Game or Load is still painted on the tick, so its chunks keep the tick path's CLUT alpha on raised ground. Baked chunks match the worker path.
- **Loader RAM: about 410 KB** (index 36 KB, blob cache 6 × 24 KB, spare slot 130 KB, ruzstd state, 48 KB stack). Kernel free in play went from 3.30 to 2.63 MB.
- **PPSSPP `run` (sprint, slowpaint 330), nobake vs baked:**

  | | nobake | baked |
  |---|---|---|
  | swatches | 133 | 0 |
  | stand-ins on screen after settling | — | 0 |
  | missed flips | 5.1% | 1.0% |
  | t_paint p95 / max | 5.9 / 25.5 ms | 2.4 / 3.9 ms |
  | tick p95 / max | 6.9 / 27.5 ms | 3.9 / 5.5 ms |

  Decode wall time is about 38 ms a chunk on PPSSPP, but that includes being preempted by the game. The hardware number needs a capture.

**All zones once baked** (Phase D of PLAN-world-as-data):
- 12 non-county zones (`ZONE_COUNT` = 13).
- Interiors are 1 to 4 chunks each, with two daylight variants for rooms. The large zones (mine, burial, school, museum, factory) are about 20 to 150 chunks each.
- Estimate about 600 to 1,000 chunks in total, so **5 to 8 MB.**
- **About 75 MB for the whole world.**
- Measure at the Phase D bake.

### 5.3 RAM and VRAM

Unchanged from rev 1:
- VRAM is full (8888 framebuffers 1.06 MB, lightmap, 16 lamp pools, 8 page slots). Chunks stay in RAM, 12 slots at 129 KB each.
- The bake adds a 54 KB index, one spare slot, 48 KB of IO buffers and about 100 KB of zstd state.
- It frees the stand-in samples, and the painter's scratch when idle.
- A compressed cache costs about 7 KB a chunk: 1 MB holds about 140 chunks.

**The PC keeps its painter,** with the pinned seed. It paints a chunk in 2 ms, and T1/T2 carry layers a T8 bake doesn't.

---

## 6. CPU display list (list 3–12 ms, ge_build 3–6.5 ms)

Unchanged in substance from rev 1. `Lister::build_with` clears and rebuilds every quad every frame.

1. **L0:** sub-timers (Q0).
2. **L1:** cache static shadow polygons per chunk by `shadow::shear` (Q8 pair); with §3 D, as masks.
3. **L2:** cache static lamps' slabs and lightmap polys per lamp; only units are recomputed. `ev_lamp_build` frames run 48 to 66 ms.
4. **L3:** emit static segments once into GE call lists (`sceGuCallList`) with a camera offset.
5. **L4:** the patch scan only over relief cells, which come from the bake.

Plus §4's water quad cuts and §2 Q6's bind reduction.

**Targets:** night list about 4 ms; ge_build about 2.5. The gate is `QuadBuf` hashes unchanged on static scenes.

---

## 7. Audio: tracker-based, exact transitions, low CPU

### 7.1 Facts and measurements

**On hardware:** the tracker takes 13 to 20% of the CPU: 5 to 7 ms a 33 ms frame, or 3.3 ms a 60 fps frame.
- That is mixing 512 frames at **22.05 kHz** into one SRC channel (`jane-audio-psp/src/psp.rs` `RATE`).
- **The mix rate is already 22.05 kHz,** not 44.1. 11 kHz samples are already mixed at half rate. Instruments are 16 kHz samples.

**Host replay of the owner's two PSP command logs** (`mixprof-out.txt`; town in rain 24 s, square into the storm and the bell 77 s):

| Section | µs a block | Share |
|---|---|---|
| **voices** (VAG decode, interpolation, gain) | 21–24 | **73%** |
| groups (72 pan/send buses) | 2.1–2.6 | 8% |
| out (reverb, DC, limiter) | 5.0–5.6 | 17% |
| sequence | 0.3 | 1% |

- Total about 32 µs a block on the host, about 4.2 ms on hardware (×130).
- **Voices alive per block: mean 17 to 19, p95 24, max 29.** The per-voice cost on hardware is about 0.17 ms a block, about **0.7% of the CPU per voice**.
- The sequencer is on the audio sample clock, quantised to 1/60 s ticks (`tracker.rs` `step_frames`). The transitions are fade pairs (`jane-present/src/audio.rs` `fades`). Both stay exact under every option below.

### 7.2 Options, costed (% of the PSP CPU; today about 20% on hardware)

| # | Option | Check | Saving | Effort | Notes |
|---|---|---|---|---|---|
| 1 | **Inner loop without bounds checks;** raw pointers; Allegrex `madd`, `min`/`max` through inline `asm!` (the target is `mips2`, so LLVM never emits them, §8.8) | `Mixer::chunk` (tracker.rs l.1722) indexes `v.dec[..]`, `groups[..]`, `hgroups[..]` per frame | −20 to −30% of voices, so **−3 to −4 pts** | S–M | Bit-exact (same integers). Host tests stay |
| 2 | **Cull inaudible voices:** raise the floor from `env < 16` to about −54 dB under the voice's owner level; tails of struck notes and sleeping beds | l.1832 | −15 to −25% of voices, **−2 to −4 pts** | S | Inaudible by construction (a listening check) |
| 3 | **Bigger chunk:** `CHUNK` 32 → 64. Envelope steps go from 1.45 ms to 2.9 ms at 22 kHz | l.33 | −5 to −8% of voices, **−1 pt** | S | Check for zipper noise on fast attacks |
| 4 | **Hot short samples kept as PCM** (footsteps, hits). PCM is 3.5× ADPCM; 150 KB ADPCM becomes 525 KB | — | −10 to −15% of voices, **−1.5 to −2 pts** | S | Needs the volatile 4 MB (§8.6) or a RAM trim |
| 5 | **Cheaper reverb:** 8 lines → 6, or optimised code at the same 8 | `Room::run2` | −2 to −4 pts | S | Fewer lines changes the tail: listen. Prefer code optimisation |
| 6 | **Simpler limiter** | `mix_out` already uses shifts for a full block | < 1 pt | S | Not worth it |
| 7 | **Pre-mix beds** per area × weather | Beds fade by game state independently (`bed_level`) | −2 to −3 pts | M | **Loses independent bed fades:** not recommended |
| 8 | Mix music at 11 kHz | Instruments carry up to 8 kHz | −3 pts | S | **Audible dulling:** no |
| — | **1 + 2 + 3 + 4** | | **20% → about 10 to 12%** (1.7 to 2.0 ms a 60 fps frame) | | Pure optimisation, bit-identical except the cull |
| 9 | **sceSasCore** for the music voices (24 ≤ its 32), our sequencer driving key-on, pitch, ADSR and volume per grain (256 at 44.1 kHz = 5.8 ms), mixed with `__sceSasCoreWithMix` into our buffer | **No bindings in `vendor/psp`**: add NIDs and load via `sceUtilityLoadModule(AV_SASCORE)`. **Unverified: whether SAS mixes on the ME or the Allegrex.** I found no source either way. PPSSPP emulates it (testable), but its cost is meaningful only on hardware | If off-CPU: **music −60 to 70% of the mixer → about 4 to 5% total.** If on the Allegrex: about neutral | M | VAG is native (our samples are VAG already). **The sound changes:** SAS's ADSR shapes, interpolation and reverb differ from our Room. Music would no longer bit-match the host replay. Note-on timing quantised to the grain (≤ 5.8 ms; humanising is already larger) |
| 10 | **The tracker on the ME** (CFW), side track | §8: whole `Mixer` as a flat `no_std` mipsel blob; command ring and PCM ring in uncached memory; watchdog falls back to the main-CPU mixer | **About 20% → about 0.5%** (the main thread only outputs blocks) | L | Bit-exact with the host. Testable only on the owner's unit. Prior art: m-cid's 2026 ME work, ["unlocking the PSP's dual core setup"](https://wololo.net/2026/06/16/unlocking-the-psps-dual-core-setup/); [PPSSPP's ME notes](https://www.ppsspp.org/docs/development/ppsspp-internals/media-engine) |
| 11 | Pre-rendered music (MP3/AT3 on the ME codec) | — | — | M | **Last resort only** |

**Coordinator's guesses, corrected:**
- Mixing at 22 kHz is already done.
- Options 1 + 2 + 3 (+4) bring audio to **about 10 to 12%**, not 6 to 8%: voices dominate and are already half-rate where possible.
- SAS on top gives about 4 to 5% *if* it runs off the main CPU (unverified).
- The ME gives about 0.5%.

**Recommendation**
- Do 1, 2 and 3 now (S–M, Sonnet with Opus review), then 4 when RAM allows. The 60 fps budget then holds at 1.8 ms.
- Spike SAS only to *measure where it runs* (S, hardware): 24 voices for 10 s, timing `__sceSasCore` on the main thread.
- The ME tracker is a side track (Opus), off the critical path.
- Pre-rendered music is not planned.

---

## 8. Fit the hardware

Ranked by value. Each item was checked against the code.

| Rank | Item | State today | What to do | Saving | Effort |
|---|---|---|---|---|---|
| 1 | **Fill-rate discipline on the GE** (clip and sparsify full-screen passes) | Raised marks are unclipped; there is a clear quad; water bounds are boxes (§3, §4) | §3 A+B, §4 W1/W3/W4 | GE −4 to −7 ms | S–M |
| 2 | **Async IO and GE-side copies** (items 9 and 4) | All `sceIo` is sync on the game thread (`main.rs` `File::read_at`); `sceIoReadAsync` is unused. Uploads are CPU copies | Loader thread with async reads (pages and chunks); `sceGuCopyImage` uploads | Spikes gone; −0.2 ms average | S–M |
| 3 | **Static GE call lists** (item 3) | Not done; everything is rebuilt each frame | §6 L3 | ge_build −1 to −2 | M |
| 4 | **CLUT tricks** (item 2) | **Done:** night and grade are folded into the chunk and sprite CLUTs (`Frame::night`, graded CLUTs in `ge.rs` ~l.727); water, wet and shine are tagged in CLUT alpha. **Not done:** water animation | §4 W5: crest phase as CLUT cycling | GE −1 to −2 at the water | M |
| 5 | **Allegrex ISA** (item 8) | The target spec is `"cpu": "mips2"` (`rustc --print target-spec-json --target mipsel-sony-psp`), so LLVM never emits the Allegrex's `madd`/`msub`, `min`/`max`, `clz`, `movz`/`movn`, `ext`/`ins`, `seb`/`seh` | Spike: build with `-C target-cpu=mips32r2` and scan the ELF for opcodes the Allegrex lacks (e.g. MIPS32 `mul`, trap `teq`, 64-bit FPU). Else inline `asm!` in the 4 or 5 hottest functions (mixer inner loop, isqrt `clz`, lz4/zstd copy loops) | 5–15% on integer hot loops (estimate) | S spike |
| 6 | **The 16 KB scratchpad at 0x00010000** (item 6) | Unused | Mixer voice states and the active group buffers (72 × 64 × 4 B is too big; the about 8 active groups fit), or the zstd decode tables. Split between threads by fixed ranges | Mixer −10 to 20% (fewer misses; estimate) | S–M |
| 7 | **Texture layout** (item 4) | Atlas pages are **swizzled** at bake (`bake_psp.rs`); normals are T4 swizzled. **Chunks are not swizzled** (bound with swizzle 0 from RAM); 8 VRAM slots LRU | Pre-swizzle baked chunks. The CPU's per-px reads (`Lister::patch`) then unswizzle the address. Font in T4 (PORT.md open item) | Terrain pass about −0.2 GE | S |
| 8 | **Cache-friendly layout** (item 7) | Voices are an AoS `Vec` with `retain_mut`; quads are a 20 B AoS | Profile-led after Q0. `PropRec` cache (Q3) is the first instance | Unknown until profiled | — |
| 9 | **60 fps presenter with the sim at its own rate** (item 10) | Sim 60 Hz, presenter interpolates; at 60 fps it is 1 tick a frame | Nothing to build. Switch pacing to Locked60 when §1's budgets pass | — | S |
| 10 | **Fold light into the draw** (item 1): Gouraud per-vertex light on a coarse ground grid | The lightmap is already a 256×140 half-resolution target with one multiply; `light_mul` 0.1, `glow` 0.08, `grade` 0.01 (CLUT), saturation 0 ms. The lightmap pass itself is 1.7–2.2 ms at night, mostly lamp pools and lamp shadows | Vertex light can't carry lamp shadows (a showpiece, ART.md §3.1) and costs CPU per vertex (15 lights × about 540 vertices). The remaining passes are already almost free | **Not recommended** | — |
| 11 | **Framebuffer format** (item 5) | 8888, two buffers of 512×272 (`ge.rs` l.211, l.419) | 5650 would halve fill bandwidth and free 557 KB of VRAM, but **has no alpha, so no stencil** (shadows and water use multi-bit stencil marks). 5551 has 1 bit; 4444 has 4 bits but 12-bit colour bands the night ramps and sky; dithering breaks pixel art | **Keep 8888** | — |
| 12 | **VFPU** for presenter-only maths | The lister is integer and bit-tested against the PC ("what ge.rs sends is exactly this list") | Only inside `ge.rs` emission, if anything ever needs float. Never in sim or lister | — | — |

**PC consistency:**
- Items 1 to 4 and 6 to 8 are PSP backend internals with identical output, so the PC renderers don't need to follow.
- §4 W5 (CLUT-cycled crests), if adopted, changes the PSP water's look slightly against T1. An A/B decides.
- The fence fix (Q8) belongs in the shared slab code, so all tiers get it.

**Media Engine, beyond audio (CFW).**
- **Nothing else is worth it on the critical path.** After the bake there is nothing to paint.
- The sim must stay deterministic and testable on the PC.
- The lister is cheap after §6, and is a pure function testable on the PC.
- The ME is for the mixer (§7 #10) and later, if needed, pure presenter jobs with a one-frame latency: sun shadow polys for moving casters, lamp slabs.
- ME sync design, as in rev 1:
  - uncached shared rings and mailboxes;
  - the main CPU writes commands; the ME writes results and its own counter;
  - a watchdog falls back to the main CPU;
  - nothing the sim reads.
- **Kernel-mode requirement:** the owner's Prome-4 provides it. The user-mode path is the default everywhere else.

**More RAM: the volatile 4 MB** (`sceKernelVolatileMemLock`, user mode, PSP-1000).
- Uses: the compressed chunk cache, PCM hot samples (§7 #4), far effects resident.
- Verify the lock, and sleep/resume, on the owner's unit.

---

## 9. Zone streaming

Unchanged from rev 1. Once Phase D bakes every zone:
- blueprints load in tens of ms;
- chunks are baked (about 5 to 8 MB with zstd);
- the ahead build becomes a prefetch of the `.bp` and the arrival chunks into the compressed cache;
- on entering, the worker decodes the view nearest first. A 3×2 view at 2 to 3 ms each is ready in about 20 ms.

**Gate:** the churn bench, first view under 100 ms, no runtime build.

---

## 10. Phases

**Gates**
- **PC:** the slow tier, `tools/psp-walk-test.py` with `slowpaint 330`, and the `QuadBuf` hash tests.
- **Hardware:** bench scenes (`crates/jane-present/src/bench.rs` `SCENES`: sprint, wood, town-night, cellar, mob, churn), plus captures decoded with `jane psp-profile`, plus a lakeside scene. **Add a `lake` scene to `SCENES`** (S).

| Order | Phase | Effort | Risk | Who | Gate (expected hardware) |
|---|---|---|---|---|---|
| 1 | **Q0** instrumentation (GE sub-passes, list sub-timers, audio per block, sim phase worst) and **P0e** (memstick read MB/s, zstd and lz4 decode ms on hardware) | S | Low | Sonnet | Numbers written into this file |
| 2 | **Q1** sun shadow fill (§3 A+B) | S | Low | Opus | sun_shadows ≤ 1.8 ms; frame dump identical |
| 3 | **Q2** stand-in cap | S | Low | Sonnet | t_paint max ≤ 25 ms |
| 4 | **Q3** `PropRec` cache | S–M | Low | Sonnet | t_props ≤ 0.4 ms a frame at night |
| 5 | **Q4** `sceGuCopyImage` uploads | S | Low | Sonnet + Opus review | Upload frames no slower than neighbours |
| 6 | **Q5** 32-bit audit and objdump guard | S–M | Low (sim checked by hash) | Sonnet/Grok | Guard green; `jane hash` and bot fixture unchanged |
| 7 | **Q7** audio spike diagnosis, **Q8** fence bug | S, S–M | Low | Sonnet, Opus | Cause found; fence shots on every tier |
| 8 | **Audio 1+2+3** (§7) | S–M | Low | Sonnet, Opus review | audio ≤ 12% on hardware; host bit-exact except the cull |
| 9 | **Water W1+W3** | S–M | Low | Opus | lake water_fx ≤ 5 ms |
| 10 | **P1a** art seed pin | S | Low | Sonnet | hash and fixture unchanged; goldens re-blessed once |
| 11 | **P1b** `jane bake-chunks` (zstd -19, console path, stale test) | M | Low | Opus | Bit-identical to the console paint |
| 12 | **P1c** PSP loader thread, async reads, worker zstd decode, slot-swap landing, prefetch, micro stand-in fallback (Q2 M part) | L | Med | Opus | sprint: 0 stand-ins after the first view; t_paint p95 < 3, max < 8; tick −4 ms |
| 13 | **Q6** binds and CLUT loads; **§6 L1–L4** list caching and call lists | M–L | Med | Opus | night list ≤ 4, ge_build ≤ 2.5; hashes unchanged |
| 14 | **Water W2/W5 (A/B) + W4** | M | Med | Opus | lake water_fx ≤ 3 ms; rain ≤ 3.5 |
| 15 | **Allegrex ISA spike** + scratchpad | S, S–M | Med | Opus | Opcode scan clean; mixer −10%+ |
| 16 | **Sim spikes** (mob 56 ms tick) | M | Med | Opus | worst tick ≤ 8 ms; hash unchanged |
| 17 | **Locked60** pacing on | S | Low | Sonnet | All bench scenes ≥ 59 fps mean, missed vblanks < 2%; night rain is the gate scene |
| 18 | **Zones baked** (after world-as-data Phase D) | M | Low | Opus | churn first view < 100 ms |
| 19 | **Volatile 4 MB** + PCM hot samples (§7 #4) | S | Med (hardware only) | Sonnet | Lock works; audio −1.5 pts |
| side | **ME tracker** (CFW, watchdog fallback) | L | High (hardware-only tests) | Opus | audio ≤ 0.5%; PPSSPP uses the user path |
| side | **SAS spike** (measure only) | S | Low | Sonnet | Where SAS runs, and its main-CPU µs |

Items 1 to 9 are the quick-win block: about two weeks of agent time, mostly S, and they deliver most of the GE gap and the streaming spikes' worst. Items 10 to 12 are the streaming rework. Items 13 to 17 close the 60 fps CPU gap.

**Expected after item 17:** CPU 9.6 / 12.8 / 13.7 / 11.6 ms and GE 3.4 / 5.6 / 9.0 / 6.6 ms (day / night town / night rain / lake). That is stage A: 60 fps with about 15% headroom in the worst scene.

**Stage B, "fly"** (about 10 ms CPU and about 7.5 ms GE in night rain), after stage A:

| Order | Phase | Effort | Who |
|---|---|---|---|
| 20 | ME mixer in use on the owner's unit (side track lands) | — | Opus |
| 21 | §6 L2 fully: every static lamp's slabs, lightmap polys and lamp-cache textures persistent | M | Opus |
| 22 | Call lists for every static segment (terrain, cached shadows, static props, HUD) | M | Opus |
| 23 | Presenter tick and draw profiled down: `t_ambient`, `t_fx`, sprite collection | M | Sonnet then Opus |
| 24 | §0.1 small details: baked map chart, held-world menus, async boot and save, first-tick priming, 256-frame audio blocks, the T4 font | S each | Sonnet (most), Opus (menus' framebuffer copy) |

**Gate for stage B:** every bench scene's p95 frame ≤ 10 ms CPU and ≤ 10 ms GE, with no frame over 20 ms, plus each §0.1 target met on hardware.

---

## 11. Open questions for the owner

1. **Look-changing optimisations under A/B.** May half-resolution water reflections (§4 W7) or CLUT-cycled crests (§4 W5) be tried as A/Bs, kept only if they read as well? Everything in the main path is pixel-identical without them, but they would buy GE headroom in night rain.
2. **What "60 fps" means.** Is every frame at 60 in the bench scenes the bar? Or are rare drops on teleports and big mob spawns acceptable while those spikes are worked down (§1.5)?
3. **SAS, if it proves to run off the main CPU.** On the PSP, music would then use SAS's own envelopes and reverb, close to the PC but not bit-identical. Is that acceptable, or does the ME tracker (bit-exact) remain the only offload you want?

## Owner answers (2026-10-18, after rev 2)

1. **Look-changing options** (half-res reflections, palette-cycled crests, fewer shadow levels and the like) are acceptable. Show side-by-side comparisons, as shots or GIFs at the same moment, before choosing.
2. **60 fps is the minimum, on every frame, including teleports and mob spawns.**
   - The ideal is 60 fps at **222 MHz**: CPU 222, bus 111, the PSP's default clock. Today it runs at 333/166.
   - Budget and measure everything against 222 MHz as the stretch target. That is about 0.67x the CPU and bus speed, which also slows the GE's memory traffic.
   - Add a 222 MHz case to the bench and to psp-walk-test.
3. **SAS's slightly different sound is fine.** Fast is the goal; promote the SAS spike from "measure only" to a real candidate.

## Water option W-mesh: distortion grid (owner question, 2026-10-18)

Replace today's water passes with one distortion mesh. Today they are the stencil over the bbox of all water, the sprites redrawn to unmark them, and thousands of crest and reflection quads.

- **Reflection.**
  - Copy the screen strip above the waterline into a texture, using a GE copy within VRAM (`sceGuCopyImage`).
  - Draw it mirrored over the water through a coarse grid, a vertex every 8–16 px, emitted only over water cells.
  - Per frame, the CPU offsets the grid's UVs from a sine table: a few hundred vertices, well under 1 ms.
- **Ripples and crests.** Use the same grid, with a scrolling wave texture or a CLUT cycle on the T8_WATER indices. No per-crest quads.
- **Shallows.** Use the same trick on the ground under shallow water, for refraction.
- **Expected cost [est].** GE water about 8 ms → about 1.5–3 ms, rain included. Measure on hardware.
- **Look.** Point sampling, offsets snapped to whole pixels, and optionally the wobble updated every 2–4 frames for a hand-animated feel, to keep the pixel art crisp.
  - This changes the look, so show the owner side-by-side comparisons against today's water before choosing.
  - Compare it with W2 and W5.

## More hacks not yet costed (owner question, 2026-10-18)

To size and rank in the next plan pass:

1. **Scrolling backdrop buffer.**
   - Compose the static world once into an off-screen VRAM buffer of screen size plus a margin: ground, static shadows, static light.
   - Each frame, blit it at the scroll offset, compose only the newly exposed strips, then draw the dynamic layers.
   - Recompose it in slices across frames as the sun steps.
   - The VRAM budget is the risk: about 2 MB, with the 8888 framebuffers and the chunk slots.
   - This is potentially the largest GE win.
2. **HUD and text cache.** Render the HUD, hotbar and quest text to a texture only when they change; pre-render common strings. The UI pass is about 1.5 ms now, target about 0.
3. **Amortise and stagger slow work.** Lamp flicker, sky, ambient, far props and lightmap refresh run every 2nd or 4th frame, in fixed slots, so no frame takes all of them.
4. **I-cache-aware builds.** The I-cache is 16 KB.
   - Hot code at opt-level 3, cold code at "s", with `#[cold]`/`#[inline(never)]` on rare paths.
   - Fat LTO with codegen-units = 1.
   - Research PGO for mipsel-psp.
5. **Rust hot-loop overheads.** Bounds checks, allocation during a frame and Vec growth. Use per-frame arenas, and audited `get_unchecked` only in measured hot loops. Confirm overflow checks are off in release.
6. **Baked per-caster shadow sprites per sun step,** instead of shearing rows into bands each time.
7. **AI level of detail.** Units far from every seat think every N ticks, or sleep.
   - This changes the sim on every platform: re-bless the bot fixture, and check quest pacing and fairness.
8. **Fewer syscalls per frame.** Batch timer, semaphore and audio calls; don't poll the perf timers when the overlay is off.
9. **Vertex formats.** Make sure every draw uses through-mode 2D with i16 positions, u8 UVs where possible, and two-vertex GU_SPRITES, to shrink the list and its bandwidth.

Items 1 and 3 reshape the frame into "blit the backdrop, then draw what moved". That shape is the likely path to 60 fps at 222 MHz.

### More, round 2 (owner: "any more? note those", 2026-10-18)

10. **Shared flow field for chasers.**
    - One distance-to-target map (Dijkstra/BFS from her cell) per N ticks, shared by every hostile chasing her, instead of a path search per hostile. 16 chasers cost about 1.
    - This is a sim change on every platform: re-bless the bot fixture and check the chase feel. The mob scene is the test.
11. **Baked navigation data.** The fixed world allows region and portal graphs and route tables built offline, so searches are shorter and seldom fail.
12. **The ME as a second core for the sim.**
    - The sim is deterministic integer code on the same ISA, so it gives identical results on the ME.
    - Pipeline it: the ME steps tick N+1 while the main CPU presents tick N. Main-thread CPU roughly doubles.
    - Needs kernel mode/CFW and hardware-only testing. Coherence goes through uncached shared memory and explicit writeback/invalidate.
    - An extreme side track, after the ME-mixer track.
    - zstd chunk decode is another ME candidate.
13. **Lamp flicker and pulse by CLUT intensity,** not a lightmap recompute.
14. **Poll input late,** just before each sim tick: up to a frame less input latency.
15. **The GE or DMA clears big buffers,** not CPU memset loops.
16. **Lookup tables in the 16 KB scratchpad:** sine, palette ramps, mixer tables.
17. **Fixed pools and arenas** for particles, fx and path nodes: no fragmentation, free allocation.
18. **Particles stepped at 30 Hz** with interpolated positions; one GU_SPRITES batch.
19. **Saves compressed and written on a worker thread,** so saving never hitches.
20. **Door prefetch:** load and decompress the next zone into RAM while she stands near its door, so the transition is instant.

## The line (owner, 2026-10-18)

"Note to do what's sensible. No hacks for using me when cpu has so much potential here to be fine. That may be the line we draw."

- **Dropped: custom Media Engine code.** That covers the ME mixer track, the sim on the ME (item 12), and zstd decode on the ME. It needs no kernel mode and no CFW.
- **Kept: the firmware's own services,** sceSasCore and the hardware audio channels, which are ordinary user-mode APIs, if they measure well. The bar is that they are sensible and stay testable in PPSSPP.
- **The main path:** get the speed from the main CPU and the GE with the sensible items in this plan. Weigh every hack by its gain against its complexity and risk, and prefer the plain one.

## Hardware features not yet planned (2026-10-19)

All are user mode and emulated by PPSSPP, so they sit inside the owner's "sensible" line.

1. **VFPU via rust-psp `vfpu_asm!`.** It has 128 float regs as 4-wide vectors or 4×4 matrices; vsin/vcos/vexp2/vrcp/vrsq; vi2s/vi2uc saturating packs; vt5650/vt4444/vt5551 colour packs.
   - **Uses:**
     - **The mixer inner loop:** 4 samples an op, with the clip built into the pack. A strong cut to the 10–12% audio after the rev 2 work.
     - **Water-mesh UV offsets and particles:** 4 at a time.
     - **Light falloff and colour packing for uploads.**
   - **Presentation only.** The sim stays integer and deterministic.
2. **GE morphing** (up to 8 keyframes per vertex): foliage sway, flags, water swell at zero CPU.
3. **GE skinning** (8 bones): bend trees or cloth.
4. **Texture matrix and UV generation:** projected or scrolled textures, such as **cloud shadows drifting over the ground**, mist and caustics, with no CPU per pixel.
5. **GE bounding-box test:** the GE skips list segments that are off-screen, so culling is free on the CPU side.
6. **Colour test:** discard by colour range, for cut-out masks.
7. **Uncached-pointer writes** (address | 0x4000_0000) for data the GE reads, so the 16 KB D-cache isn't polluted. Mind the coherency rules.
8. **sceAudioSRCChReserve, the hardware sample-rate-conversion channel.** It upsamples our 22.05 kHz output for free. **Already in use** (`jane-audio-psp/src/psp.rs` reserves it at 22.05 kHz; checked 2026-10-20).
9. **Hardware channel volume and pan** for fades and crossfades.
10. **Allegrex instructions** (madd/msub, min/max, clz, ext/ins, wsbh, movz/movn): already a spike in rev 2, via inline asm or a target change.

**Best value:** 1 (the mixer), 8, 4 (cloud shadows) and 5. For 1, see the next section: the VFPU fits the mixer's bus stages, not its voices.

## VFPU: where it pays (research pass, 2026-10-20)

**Brief:** the VFPU is idle; where would it buy real speed? Presentation, audio and PSP-only code may use it and differ slightly from the PC (owner-approved). **The sim stays integer** (saves, lockstep, the bot hash). User mode only: no kernel, no ME code.

**Short answer:** it pays where the work is wide, regular and already in lines of 16 bytes: the mixer's bus stages, the lamps' shadow projection and any future float-heavy effect. It does **not** fix the two biggest CPU items: the mixer's voices (about 73% of the audio) and the display list (3–12 ms). Those are bound by gathers, branches and memory, and §6 and §7.2 stay their plan.

### What the VFPU is good and bad at, here

- **Good:** 4-wide float arithmetic, one instruction a quad. `vi2f.q`/`vf2id.q` convert four ints with a free power-of-two scale (Q12 in or out at no cost), `vdot.q`/`vfad.q` sum across a quad, `vrcp`/`vrsq` stand in for the Allegrex's ~36-cycle `div` and for `isqrt`. `vi2s`/`vi2uc` pack with saturation (a free clamp to i16 or u8).
- **Bad:**
  - **No gather.** Each lane from a different address is an `lw` + `mtv` (or one `lv.s`) per lane.
  - **No 8- or 16-bit loads.** `lv.s`/`lv.q` take 32-bit words only. Our samples (`i16`), texels (`u8`) and vertices (`i16`) need converting first, or a `vs2i`-style unpack of packed pairs.
  - **No integer arithmetic.** Integer data costs a `vi2f` in and a `vf2i` out. One instruction a quad each, so cheap only if several ops sit between.
  - **`lv.q`/`sv.q` fault on an address off a 16-byte line.** `ulv.q` exists but is two loads and is not trusted on every firmware. Our `Vec`s are 4- or 8-aligned, so buffers must be made aligned (`#[repr(align(16))]`).
  - **Serial recurrences don't vectorise:** VAG ADPCM (`s = f(old, older)`), the DC filters and the one-pole damps over time.
- **Exactness:** an f32 holds integers to 2^24. Q12 products of 16-bit audio reach 2^29, so a frame can come out one unit off the integer path (−96 dB). Fine for sound and pixels; **never for the sim**.
- **Context:** only threads made with `ThreadAttributes::VFPU` may use it, and theirs is the only VFPU state saved on a switch (128 floats, 512 B). The audio thread switches about 43 times a second: under 0.1% of the CPU.

### Ranked candidates

Hardware costs are from `progress/2026-10-18_82_hw-run/` and §1.3, §7.1. "Gain" is the expected saving on hardware, with the reasoning. Only #1 is measured so far.

| # | Candidate | Cost on hardware today | Expected VFPU gain, and why | Effort | Risk | Look / sound change |
|---:|---|---|---|---|---|---|
| 1 | **Mixer: group pan and send** (`tracker.rs` `block`, the "groups" lap; **the spike**) | 8% of the mixer: about 1.3–1.6% of the CPU | About 20 VFPU instructions per 4 frames against about 19 integer per frame (`lw`, `mult`, `mflo`, `sra`, `add`, `sw` times three buses). **3–4× on the stage, about −1 pt CPU.** Buses already 16-aligned (`Lane`) | S (done) | Low | ±1 unit on rare frames (−96 dB) |
| 2 | **Mixer: out stage** (`Room::run2` reverb, `mix_out` master, DC, limiter) | 17% of the mixer: about 3 pts | The reverb's 8 lines are two quads: damp (`vsub`/`vscl`/`vadd`), gains (`vmul`), the cross-mix sum (`vfad`), `wl`/`wr` as two `vdot.q` with fixed sign vectors. The lines' taps are 8 `lv.s` + 8 `sv.s` on the integer buffer, so no conversion pass. The limiter's last loop is 4 samples an op, with `vi2s` as the clamp. **−1.5 to −2 pts.** The pre-delay, diffusers and DC stay scalar (serial) | M | Low–Med: a float FDN must stay stable (gains < 1; listen to the tail) | The reverb's tail not bit-equal to the PC's (inaudible by design) |
| 3 | **Lamp block shadows** (`list.rs` `lamp_block_poly`) | About 1.1 ms a frame in town on PPSSPP, likely more on hardware | 8 corners × 2 integer `div` (about 36 cycles each) plus `i64` compares: about 600+ cycles a block. VFPU: one `vrcp` per height (2 a block), corners 4 to a quad (`vsub`, `vscl`, `vadd`, `vf2id` with the cell's power of two as its scale), `vrsq` for the over-reach case. **Projection about 5–10×.** But **an integer reciprocal hoisted per height** (2 divides a block, Q16 multiply) gets most of it with no asm: **do that first**, then VFPU only if the profile still shows it. §6 L2 caches static lamps, which shrinks it again | S (integer) / M (VFPU) | Low | A shadow edge a cell off now and then |
| 4 | **Mixer: voices** (`Mixer::chunk`, `add_run`, `vag_run`) | 73% of the mixer: about 11–15 pts, **the big one** | **Poor fit.** Each voice resamples with a data-dependent fetch (gather), from `i16` (no 16-bit VFPU load), and the VAG decode is a serial recurrence. 4 outputs a quad still need 8 scalar loads and `mtv`s: about break-even with the scalar code. Only the beds at their own rate (`add_run`, contiguous) fit, and only if `dec` is stored as `i32`. **About 0 to −1 pt.** Keep §7.2 #1–#4 (bounds-check-free loop, Allegrex `madd`/`min`/`max`, culling, bigger chunks) | L | Med | — |
| 5 | **Lightmap pools** (`light.rs` `LightMap::pools`; check it still runs per frame on the PSP, since lamps are mostly cached GE pools now) | A `u32` divide + LUT per lit cell: about 1–2 ms for 10 lamps, if it runs | Integer first: a per-light reciprocal of `r²` removes the divide. VFPU: 4 cells a quad with the cubic falloff computed (no LUT gather), but the totals are AoS `[u32; 3]` and need an SoA rewrite. **Integer fix about −50%; VFPU a further −30% or so** | S / M | Low | ±1 in a light level |
| 6 | **Sun and lamp slabs** (`shadow.rs` `project`, `row_slabs`, `block_bands`) | Part of list 3–12 ms | Divides and `isqrt` per slab corner (`vrcp`/`vrsq` fit), but most of the work is integer bookkeeping and emission, and §6 L1/L2 caching removes most of it per frame. **Small after caching** | M | Low | A cell now and then |
| 7 | **Vertex emission** (`ge.rs`, `list.rs` quads) | ge_build 3–6.5 ms | Mostly copying `i16`/`u16`/`u32` fields into vertex memory: store-bound, and the data is not in VFPU registers to begin with (an `mtv` per word). §6 L3 static call lists and §HW 7 uncached writes are the wins. **About 0** | — | — | — |
| 8 | **Water mesh** (W-mesh grid, if built), **particles and rain** (`fx`) | Tens of µs today | `vsin`/`vcos` 4 vertices at a time, `vi2s` into `i16` vertices. Real, but on a few hundred vertices a frame it's **under 0.1 ms**. GE morphing (§HW 2) costs zero CPU. Worth it only if counts reach thousands | S–M | Low | Slight shape change |
| 9 | **CLUT grading, night tables** (`grade.rs`) | Per palette change, not per frame | Table lookups (gathers) over 256 entries. **No fit, no need** | — | — | — |
| 10 | **Chunk decode** (zstd, lz4), **the painter** | Off the frame (worker, bake) | Byte-serial entropy decode and per-pixel table lookups. **No fit** | — | — | — |
| — | **The sim** | — | **Forbidden:** float rounding breaks the bot hash, saves and lockstep | — | — | — |

**Total realistic VFPU gain:**
- About **−2.5 to −3 pts of CPU** in the mixer (#1 + #2): about 0.4–0.5 ms a 60 fps frame.
- Up to about 1 ms a frame in lamp-lit towns (#3), mostly reachable in integers.

**Recommendation:** do #1 and #2 after §7.2 #1–#3. Hold #3 to "integer first". Keep the VFPU in mind for future float-heavy effects. It does not replace §6 or §7.2.

### The spike (built)

- **What:** `crates/jane-audio-psp/src/vfpu.rs` `pan`: the group pan and send, four frames a quad (`lv.q`, `vi2f.q`, `vscl.q`, `vadd.q`, `vf2id.q`, `sv.q`), hooked in through `Mixer::pan` (`tracker.rs` `Pan`/`pan_int`). `jane-audio` stays safe code, and bit-identical on PC.
- **The mixer's buses are now `Lane<N>`** (`#[repr(align(16))]`). Any slice off a line falls back to `pan_int`.
- **In play it is on only with the script word `vfpu`** (`VFPU_PAN`, read by the audio thread every buffer). The audio thread now has `ThreadAttributes::VFPU`.
- **Bench:** `micro vfpu pan …` in `bench.txt` (`micro.rs`, after the decode lines). It runs 2,000 mixer blocks of 8 groups × 64 frames (4 of them sent), integer then VFPU, and prints µs a block, the speed-up and the largest difference. The script word `vfpubench` prints the same line at boot without running the whole bench.
- **PPSSPP result (correctness; the timings are the emulator's, not the hardware's):**
  - `vfpubench`: `int_us_block 43.058 vfpu_us_block 21.213 speedup 2.02 maxdiff 0`. Bit-identical on the bench's loud data.
  - In play (Castle at 20:00, 600 ticks, `vfpu` against none): the audio log's `groups` lap fell from 171–197 to 93–107 µs a 512-frame buffer. No underruns, no fault.
  - The two runs' captured WAVs are **sample-identical for the first 11.8 s** (260,000 stereo frames). After that they part where a command lands a buffer later. Two integer runs part the same way at 15.3 s: that is emulator timing, not the pan.
  - **Hardware still to measure:** the owner's next Benchmark run prints the line.
- **For the owner's hardware run:** the Benchmark row's `bench.txt` now ends with the `micro vfpu pan` line. Its `int_us_block` × the blocks a second (22,050 / 64 ≈ 345) is the stage's real cost; `vfpu_us_block` is the spike's.

### Pitfalls learned

- **`vfpu_asm!` wraps each block in `.set noreorder` and `.set noat`.** No branch delay slot is filled for you, and no pseudo-instruction may need `$at`. Loop in Rust around one `asm!` per quad, or fill every delay slot by hand.
- **The macro expands to `core::arch::asm!` in the calling crate.** That crate needs `#![feature(asm_experimental_arch)]`, cfg'd to `target_os = "psp"`.
- **Operands:** registers by position or name (`{0}`, `{m}`), never by register name (`out("t0")` doesn't parse; use `"$8"`). The base of `lv.q`/`sv.q` must be an `in(reg)` pointer.
- **VFPU registers are invisible to LLVM.** Values loaded in one `asm!` survive to the next, which is how the gains are loaded once a call. Only VFPU code you call can clobber them: rust-psp's own `cosf`/`sinf`/`fminf`/`fmaxf` (`vendor/psp/src/math`) use `S000`/`S001`. Keep long-lived values out of matrix 0 (the spike uses `S700`–`S702`).
- **rust-psp's math exports VFPU code under plain libm names.** A thread **without** `ThreadAttributes::VFPU` that reaches `f32::sin`/`cos`/`min`/`max` through them faults (coprocessor unusable). Today the painter and the loader are integer only. Keep them so, or give them the attribute.
- **Workspace `unsafe_code = "forbid"`:** `jane-audio` and `jane-present` can't hold `asm!`. They take a function pointer (`Mixer::pan`), and the PSP crate (`unsafe_code = "deny"`, `#[allow]` on a PSP-only module) supplies the VFPU one. `jane-render-psp` is already `deny` and can take a PSP-only `vfpu` module the same way.
- **16-byte alignment is the caller's job.** Make the buffers aligned types (`Lane`), check the pointers at entry and fall back to the integer path. Never `ulv.q`.
- **Floor vs `>> 12`:** use `vf2id` (round down), not `vf2iz` (towards zero) or `vf2in` (nearest), to match an arithmetic shift on negatives.
