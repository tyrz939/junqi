# PSP speed: the log

Dated hardware runs, findings and session plans, oldest first, moved out of `PLAN-PSP-SPEED.md` on 2026-11-07. Append new runs at the end, dated, and update `PSP-NOW.md` with the result. Raw data for each run is in `progress/YYYY-MM-DD_NN_hw-bench/` (local, gitignored).

## Hardware bench, 2026-10-21 (build 758dabc+, unpaced) and the sim spikes to fix

The first full bench on the owner's PSP-1000 (333 MHz). It ran with the stall-defer change off (it hung the GE at 758dabc, below), every scene unpaced (`fps:free`) and the micro-bench first.

**Micro-bench**
- **Memstick:** 15.5 MB/s sequential (sync and async alike), 12.5–13.0 MB/s for random 64 KB reads.
- **One chunk's albedo frame:** zstd -19 decodes in **15.0 ms**, lz4 HC in **2.9 ms**. A whole chunk (albedo, heights, sidecar) is about 25–30 ms with zstd and about 5 ms with lz4. **The pack is lz4 from now on** (`psp-dist.sh --codec lz4`, about 95 MB).
- **VFPU group pan:** 85.0 → 30.1 µs per block (2.82×), bit-exact.

**Scenes (unpaced; the loader was starved, so every frame drew stand-ins and the fps reads high)**

| Scene | fps | low1 | worst ms | sim worst ms | Notes |
|---|---|---|---|---|---|
| sprint | 122.9 | 40.4 | 116 | 14.1 | one bench hang (sim, see below) |
| wood | 108.6 | 53.8 | 53 | 11.1 | |
| town-night | 59.9 | 30.8 | 62 | 22.5 | |
| cellar | 73.7 | 9.4 | 359 | **129.1** | |
| mob | **11.9** | 2.6 | 516 | **104.9** | a hang: the game thread stuck in a `step` over 3 s |
| churn | 53.2 | 7.8 | **2849** | **455.8** | |
| lake | 71.6 | 49.9 | 64 | 14.4 | |

**Two hardware bugs this found (fixed in the next build):**
1. **GE hang:** Q6's deferred stall address (`jane_gu_stall_defer(true)`, moved every 6 KB) hung the GE on hardware: `sceGuSync` never came back after a sent list (night town, 758dabc). PPSSPP never showed it. It is off (`ge.rs`), and the GE is told the stall at every batch again; with it off, all 7 scenes ran. Root cause not proven. Don't turn it back on without a hardware run.
2. **Loader starvation:** the loader ran at priority 40, below the game (32). Unpaced, or when busy, the game never slept and the loader got no CPU. It now runs at 30. The painter (48) can still starve when unpaced; watch interiors and changed tiles.


## Hardware bench, 2026-10-22 (build 97a8540, stamped 5371eef9b5+): the loader fed, the real costs show

The first bench with the loader working: sync reads on its own thread at priority 30, lz4. Every scene was unpaced. Source: the owner's `dist/JANE00001/bench.txt` and `crash.txt`.

| Scene | fps | low1 | worst ms | sim worst ms | stand-in frames | reads (mean ms) | decodes (mean ms) | landed / refused |
|---|---|---|---|---|---|---|---|---|
| sprint | 79.2 | 28.6 | 83 | 16.4 | 97 / 4682 (2%) | 305 (35.5) | 191 (4.65) | 190 / 1 |
| wood | **32.7** | 17.7 | 69 | 11.3 | 0 | 298 (17.0) | 135 (4.54) | 133 / 2 |
| town-night | **31.2** | 14.0 | 152 | 21.8 | 106 / 1220 | 207 (**72.5**) | 109 (5.27) | 106 / 3 |
| cellar | 87.0 | 45.4 | 272 | 81.1 | 508 / 3516 (painter, an interior) | 0 | 0 | 0 / 1 |
| mob | **9.9** | 2.2 | 498 | **114.0** | 0 | 272 (19.4) | 153 (5.15) | 104 / **49** |
| churn | 51.4 | 7.8 | **2049** | **429.2** | 1719 / 2518 | 77 (20.1) | 62 (4.91) | 51 / 10 |
| lake | 55.4 | 41.3 | 32 | 9.5 | 0 | 9 (26.5) | 9 (5.05) | 9 / 1 |

Memory held up: free 2.2–2.9 MB, largest block 1.9–2.5 MB (it was 1.4–2.0). Micro-bench as before (lz4 2.8 ms an albedo frame, VFPU pan 2.6×).

### Learnings (noted 2026-10-22; owner: "note down, don't action yet")

**L1. The streaming works.** Stand-in frames in the open county fell from 100% to 0–2% (sprint 97 frames, all early). The fixes that did it:
1. Sync reads on the loader's own thread. `sceIoReadAsync`'s I/O thread runs at the opener's priority and starved behind an unpaced game.
2. The loader above the game's priority.
3. lz4: a whole chunk decodes in **4.5–5.3 ms** on hardware.

General lesson: on the PSP, async I/O isn't free concurrency. Give I/O its own thread and read synchronously there. Equal-priority threads don't time-slice.

**L2. The earlier 100+ fps were fake.** With stand-ins on screen the frame was cheap. Now real chunks are drawn: **wood 32.7 fps and night town 31.2 fps unpaced**, about 30 ms a frame (sprint 79, lake 55). Wood draws 592 casters and 640 sprites. This is the real gap to 60, about 2× in the dense scenes. A per-pass capture (perf overlay plus dashcam) in the wood and night town is needed to split it into CPU list, GE fill, sim and audio before choosing fixes.

**L3. A read costs 17–72 ms for about 11 KB,** against about 5 ms for a 64 KB read in the micro-bench.
- **Likely cause:** FAT seeks. Each `sceIoLseek` deep into a 97 MB file walks the cluster chain. Night town, deepest in the pack, is slowest (72 ms).
- It doesn't hurt yet, because the loader keeps up at sprint speed, but it caps throughput at about 15–50 chunks/s and will matter for teleports and first views.
- **Options:**
  - lay chunks out so neighbours are adjacent (Z-order or row blocks) and read a 2×2 or row block in one read;
  - split the pack into region files (shorter chains);
  - keep a few fds open at different regions;
  - read ahead in larger spans.
- First confirm with a micro-bench read at deep offsets against near ones.

**L4. Wasted reads.** There are about 1.6–2.2 reads per decode (sprint 305 reads, 191 decodes). Read-ahead blobs are evicted before use (`CACHE` = 6) or read for chunks no longer wanted. A bigger blob cache (each about 11 KB) or smarter read-ahead would cut I/O by about 40%.

**L5. Refused landings in the mob scene: 49 of 153.** The presenter turned down decoded chunks. The likely causes are the GE fence, or chunks no longer wanted as the camera moved, and each refusal costs a 100 ms retry. Check why before tuning.

**L6. The bench's "hangs" are load stalls, not play stalls.**
- `crash.txt`: the mob scene's hang was in **zone cellar** at frame 11277, so it happened while the bench moved from the cellar scene to the mob scene: a zone change plus a fresh county view.
- Sprint's hang is the first scene, so likely the New Game county build.
- Both fit S3 (zone changes in the tick, churn's 2.0 s worst frame and 429 ms sim).
- The bench's watchdog should tell a load from a frozen frame. Building zones off the game thread fixes the cause.

**L7. Sim spikes unchanged:** mob 114 ms, cellar 81 ms, churn 429 ms, night town 21.8 ms. Mob is 9.9 fps. S1–S4 above stand; S1 (mob) and S3 (zone changes) are the biggest.

**L8. Interiors still use the painter:** the cellar had 508 stand-in frames (14%). Baking interiors and zones (Phase 18, after world-as-data Phase D) removes the last painter use in play.

### Suggested order when the owner says go
1. A per-pass capture in the wood and night town (L2). This decides the next draw-side work: list caching, lantern block shadows, sprite and caster cost.
2. S1 mob sim, then S3 zone changes off the tick (L6, L7).
3. Pack layout and read batching (L3, L4); the refusals (L5).
4. Interiors baked (L8).


## Next session: the specific plan (written 2026-10-26)

**Goal:** one PSP build where a single Benchmark press records everything we need, then use the owner's run to start the 100-mob game logic work. Routing follows the owner's policy (memory `agent-model-routing`), with model and effort set on every spawn and at most 2–3 agents at once.

### Step 1: bench auto-capture (W0), plus the VFPU pan on

**1a. Plan** (`[opus/high]`, about 1 hour). Write down:
- **What each scene records.** The dashcam (`capture::Recorder`, `main.rs` ~l.1572; `ge.timing` is already on when a dash is running) is armed for each scene's measured window. At the scene's end it writes `capture-<scene>.bin` beside `bench.txt` (reusing the writer at ~l.2593 with a scene name in place of the next free number).
- **The scene line's summary.** Each scene line gains a compact per-part summary from that capture: means and p95 for
  - CPU: tick, sim, list, ge_build, props, lights, fx, amb and audio %;
  - GE: total, sun_shadows, water_fx, glow, ui and terrain;
  - the loader (already there).
- **Memory budget.** Captures are about 186 KB each, 7 scenes ≈ 1.3 MB on the stick. Size the dashcam's RAM to fit the largest free block (now 1.9–2.5 MB); shrink `DASH_FRAMES` in bench mode if needed.
- **`jane bench report`** (`crates/jane-cli/src/bench_scenes.rs` l.253) reads two `bench.txt` files and prints the deltas per scene and part. It also runs `jane psp-profile` on each `capture-<scene>.bin` into `progress/<date>_bench/<scene>/`.
- **VFPU group pan on by default** on the PSP (it's behind the `vfpu` word, `main.rs` l.737). It measured 2.5–2.8× on hardware with maxdiff 0. Keep a `novfpu` word to turn it off.

**1b. Critique** (`[grok/critique]` through the plugin, or the CLI if the plugin isn't installed): a second opinion on 1a.

**1c. Build** (`[grok/delegate]` medium, on its own worktree; fallback `[sonnet/medium]`). It implements 1a: PSP code in `spikes/psp-game/src/bench.rs`, `main.rs` and `perf.rs`, plus the CLI report.

**1d. Verify** (`[haiku/medium]`):
- the five gates;
- a PPSSPP scripted bench, which must write 7 scene lines with summaries and 7 captures;
- `jane bench report` on the two `bench.txt` files.

**1e. Review** (`[grok/review]` high). Merge, push main, and refresh `dist/` and `jane-psp-play`.

**Deliverable:** a PSP build. The owner presses Benchmark once and sends `JANE00001/` (`bench.txt` plus 7 captures).

### Step 2: read the owner's run (me, inline)

- `jane bench report` against `progress/2026-10-22_84_hw-bench/bench.txt` (the baseline) and the per-scene profiles.
- Write the split for wood, night town and mob into the plan: CPU parts against GE passes against sim, and which sim phase.
- Pick the draw-side items with the numbers, among them the lantern block shadows (`lamp_block_poly`), caster and sprite cost, and list size.

### Step 3: the mob game logic (S1 and the 100-mob target), started in the same session if there's time

**3a. Diagnose** (`[opus/high]`):
- host repro with `jane bench scenes --only mob --trace` and the cellar-to-mob hand-over (where the 3 s hang was);
- a per-phase sim profile: path, AI, LOS, separation, combat;
- find the unbounded loop behind the hang.
- Output: the costed list and an exact fix plan (data shapes, where the flow field lives, the LOD tiers, the spatial-hash API, the budgets), plus the hash and fixture rules. The sim is shared with the PC and co-op, so it must stay integer and deterministic.

**3b. Critique** (`[grok/critique]`): the 3a plan.

**3c. Build in two slices, not at once** (the sim is shared):
1. `[sonnet/high]`: budgets and the hang fix first, a small diff.
2. `[sonnet/high]`: the flow field, AI LOD, the spatial hash and the LOS cache, from 3a's plan.
- Add a `horde` scene (100 mixed hostiles chasing in the open county) to `SCENES` (`[sonnet/low]`).

**3d. Review** each slice: `[grok/review]` plus `[opus/high]`, because these are sim and determinism changes. Bot fixture and `jane hash` re-blessed only where behaviour is meant to change, and named.

**Gates for step 3 (hardware):**
- `horde` ≥ 60 fps unpaced, sim worst ≤ 8 ms;
- mob sim worst ≤ 4 ms;
- no hangs.

### Carried into later steps (not this session unless step 3 is done)

- Zone changes off the tick, with the memstick wait after resume and the audio muted on suspend (S3 + PSPATTERN notes).
- Pack layout and read batching, the wasted reads, the mob-scene refusals (L3–L5).
- Interiors baked (L8).
- The VFPU FDN reverb, with an A/B listen for the owner.

### Before starting

- Ask the owner whether the grok-build plugin is installed. If not, use the CLI: `timeout 7200 ~/.grok/bin/grok -p … -m grok-4.7 --reasoning-effort high --cwd <worktree> --always-approve`.
- The build folder was cleaned on 2026-10-24, so the first build is cold (about 10–20 minutes, plus about 40 s of bake).
- Run `tools/hiqos.ps1` in the background before building.

**Pace for this week** (owner, 2026-10-26: "efficient and good results even if we go slower. less agents at once"):
- One agent at a time; at most one Claude agent plus Grok.
- Grok builds and reviews wherever it fits.
- Opus high only for 1a and 3a; everything else at medium.
- Step 3's slices run one after another, never in parallel.

## Hardware bench, 2026-10-27 (W0 build, stamped 1225ba5dc3+): the split

The data is in progress/2026-10-27_85_hw-bench/ (bench.txt, captures, profiles/). Against 2026-10-22, every scene is up 3–10% in fps (the VFPU pan; audio is still 14–22% of the CPU).

**The CPU is the limit, not the GE.** Mean ms per frame, from the per-scene summaries:

| scene | fps | tick | list | ge_build | main-thread sum | GE total | audio % |
|---|---|---|---|---|---|---|---|
| wood | 35.2 | 4.0 | 10.6 | 4.1 | 18.7 | 5.1 | 16 |
| town-night | 32.1 | 6.6 | 12.5 | 4.8 | 23.9 | 9.4 | 14 |
| mob | 10.9 | 13.2 (sim 31.4) | 24.4 | 14.8 | ~84 | 15.2 | 22 |

- **The list build is the biggest single cost:** 10–12 ms in the wood and the night town, and 24 ms in mob. It is one untimed block today. Wood has 591 casters and 639 sprites, so the cost is about 15 µs (about 5000 cycles) per item.
- **ge_build** costs 4–5 ms, and 15 ms in mob.
- **The GE** has headroom everywhere except mob. Sun shadows cost a flat 2.4–2.8 ms. The night town and the cellar each have about 4.4 ms of GE time outside the named passes, likely the lamp passes.
- **The hangs are world loads on the tick, not the mob AI.** crash.txt records the 3 s stall in a `step` while the zone was still the cellar, at the cellar→mob switch. The sprint hang is the first scene's load. Churn's 2 s worst is the same cause. So "zone changes off the tick" moves up the order.
- **The mob sim** has a mean of 31 ms and a p95 of 188 ms. That is still Step 3.
- **Lake** passes (57 fps).

### Draw-side picks (in order)
- **D1. Sub-time the list build:** casters and shadow slabs, sprites, terrain, sort and the rest, as dashcam fields. Then cut the top part. Expect the per-caster shadow geometry to lead. Options:
  - cache the static casters' geometry per chunk;
  - VFPU transforms;
  - cap shadows to the nearest N, per the 100-mob note.
- **D2. ge_build:** write vertices straight into the GE list (no intermediate buffers) and skip redundant state.
- **D3. Night town:** name the unaccounted ~4.4 ms of GE time (lamps), and props at 1.6 ms (p95 4.6).
- **D4. Zone and world loads off the tick.** This fixes the hangs and the 2 s churn worst. It is the "later" item, pulled forward.
- **Rough target for 60 fps:** a main-thread budget under about 13 ms, so with audio the frame fits 16.7 ms. Wood needs list plus ge_build to come down from 14.7 to about 7 ms.

### The list split on hardware, 2026-10-27 (build 8f9f68f; data in progress/2026-10-27_86_hw-bench/)
- **Wood**, ms per frame:
  - sun-shadow casters 6.3; sprites 2.6;
  - `present.draw` 5.7 (not counted before);
  - so the main thread is about 26 ms (list 11.7, ge_build 4.2, tick 4.0, draw 5.7).
- **Night town:** lamp pools 8.0 (their casters 5.0, blocks 2.0).
- **Mob:** pools 12.3, sun shadows 5.9, sprites 3.2.
- **Emulator counters, dense wood:**
  - about 433 casters a frame reach the sun loop; about 240 of them are off screen and are culled only after their rows lookup;
  - the 193 drawn lay about 2260 band rects (about 12 each), merged only to about 1980, and each is a GE quad;
  - rows-cache misses are about 0.
- **c953740:** the rows lookup scans a compact hash array (a cache-miss guess; it changes nothing in the emulator). Hardware will tell.
- **The real fix (next): a static-caster sun-shadow cache per chunk.**
  - Casters that do not move: their merged level rects, in world space, for the current sun shear, built once per shear step.
  - Per frame, visible chunks append their rects offset by the camera.
  - Movers stay on today's path.
  - The union per level is unchanged, so the px are the same.
  - Open question: swaying trees (`bend` in the key). Either they stay per frame, or the shadow ignores the sway (a look change, so it needs an A/B).
  - Also: a cheap pre-cull before `rows_of`, and the same cache idea for still lamps' casters (already in `lamps`).
- **Hardware run, build c953740** (progress/2026-10-27_87_hw-bench/):
  - the hash lookup took wood sun casters from 6.3 to 5.5 ms;
  - wood went from 33.6 to 34.9 fps.
- **Next build: the sun pre-cull** (`sun_off_canvas`). A caster whose shadow cannot reach the canvas, by its height, depth, sway and the shear, is dropped before its rows lookup. The px are the same, which a test checks against each caster's own bands. In the emulator, wood sun casters went from 1.9 to 1.4 ms.
- **Owner's idea, 2026-10-27: one textured quad per tree shadow.** The 7 lean shapes are already kept per caster. The cost is that each lays about 12 band rects, about 2000 GE quads a frame in the wood.
  - Instead: a small T4 shadow image per (sprite, lean, sun step) in a capped cache, one quad per caster, drawn into the stencil by level (the GE's MIN/MAX blend or alpha test per level).
  - That cuts the quads about 12×, helping both `l_silh` and `ge_build`.
  - Needs RAM/VRAM (about 2 KB per image) and an A/B, since strength levels in a texture may round differently.
  - Do it after the pre-cull is measured.
- **Owner, 2026-10-27: "indoor painting is pretty slow still. that does need improving."** Interiors still paint their chunks at runtime (the county is baked). Candidates:
  - bake the interiors into the pack like the county (the "baked interiors" item);
  - or paint ahead on the loader thread while the door opens.
  - Measure the zone-entry time first (it is part of the hangs and the churn worst too: world loads on the tick).
- **2026-10-27, the return-to-county stall fixed (1497f96).** The presenter's kept county was keyed on `flags_generation`, which moves with every prop stamped (the clock rows at the hour). So nearly every return outdoors rebuilt the county's houses and dropped every kept chunk: 1.2 s emulated, 3 s on hardware. It is now keyed on `View::ground_digest` (the changed tiles and spans). Emulator: the worst tick went from 1183 to 65 ms.
- **Indoor painting, step 1 (this build).** Entering a zone other than the county raises the painter at once until its view is painted. Emulator cellar: swatches 404 → 0, worst frame 156 → 10 ms; churn swatches 2715 → 711.
- **Step 2, the real fix: bake the interiors.** One JCK1 pack per zone (`<zone>.jck`, plus `<zone>-night.jck` where windows differ), from `Baker` over a view of that zone. The loader switches packs at the door, and the presenter's `baked` follows the zone, not just the county.
- Churn's worst (about 1.3–1.6 s) is a zone built on the step when `ahead` has not got it ready. Look at that next.

## Hardware bench, 2026-10-28 (build e67fab6; data in progress/2026-10-28_88_hw-bench/): notes only, no action yet

Compared with run 87 (c953740, the hash lookup), in ms on hardware:

| scene | fps | list | sun casters | other |
|---|---|---|---|---|
| wood | 34.9 → **40.5** | 10.8 → 7.8 | 5.5 → **2.5** (the pre-cull) | sprites 2.6, draw 5.9, ge_build 4.1 unchanged |
| town-night | 32.8 → 34.3 | 12.0 → 11.4 | 1.5 → 0.8 | lamp pools still 7.7 (casters 4.8, blocks 2.0); swatch 108 → 47 |
| cellar | 89.8 → 91.9 | | | swatch 501 → **0**, worst 261 → **26** ms (the boost works on hardware). sim_worst still 77 ms (the entry tick) |
| mob | 10.0 → 12.0 | 30.9 → 25.2 | 5.1 → 3.9 | ge_build 22 → 13; sim 23.6 mean, p95 104: sim is now the wall. Its run 87 hang is gone |
| churn | 53.2 → 53.2 | | | swatch 1748 → 72, but worst **2019 → 2705 ms**, sim_worst 421 ms |
| sprint | 83.3 → 88.2 | | | still 1 hang (3 s, frame 656, zone not yet set: see below); swatch 63 → 236 |

What stands out:

1. **Churn's worst got worse (2.0 → 2.7 s), as in the emulator (1.3 → 1.6 s).** Painting is no longer the problem; the stall is a zone built on the game's step (sim_worst 421 ms) when ahead.rs isn't ready. Suspect the boost: the painter at 31 outranks the game at 32 and may starve ahead.rs and the loader (30) only indirectly, or hold a lock the step wants. Try the boost off on that build to split the two causes before baking.
2. **The sprint hang is in every run since 85 (also 86, 87).** It comes at about frame 656 with the zone still `cell 0 0 seed 0`. That is the bench's first scene loading, before the view exists. The last events are a normal ui/list/ge draw for f655, then nothing for 3 s. So the stall is outside the frame's logged spans: the first county load or the first baked reads (sprint's read_us 38 ms, baked_wait 806, the highest). It looks like a start-up stall, not a play stall, but find it: add an event before world load or seed, and before the first pack open.
3. **Sprint swatches 63 → 236.** The boost is off in the county, so this is likely the same start-up stall (the painter behind for 3 s) and not a regression. Check after (2).
4. **Town-night `read_us` 72 ms over 207 reads** (wood is 17 ms over 300). Some reads are slow there, perhaps seeks across the pack. Low priority.
5. **The cellar's entry tick is still 77 ms of sim.** It's built on entry; baking the interiors (indoor step 2) won't fix that, ahead.rs readiness will.
6. **Next big costs, unchanged in rank:** mob sim (23.6 ms mean); lamp pools at night (7.7 / 11.2 ms mob); wood `draw` 5.9 ms and sprites 2.6 ms; audio 13.5–22%; mob `water_fx` 3.0 ms on the GE.

### Next steps, in order (noted 2026-10-28; not started)

1. **Split churn's 2.7 s worst.**
   - Run the emulator churn scene with the painter entry boost off (one constant), then on. Compare worst_ms and sim_worst_ms.
   - If the boost is to blame: cap it, so it never outranks the game while a zone build is waiting. Or let it yield while ahead.rs has work.
   - Either way, log when the step builds a zone itself (ahead.rs not ready) and which zone, so the miss is visible in bench.txt.
2. **Make ahead.rs ready before the door.** Build the zone behind each door within reach a few seconds ahead. Hold the step from building one itself: wait a frame or two on a fade rather than build 400 ms inline. This also covers the cellar's 77 ms entry tick.
3. **Find the sprint start-up hang.**
   - Add crash-log events around the first world load, the seed, the county pack open and the first baked reads.
   - Re-run the hardware bench and read crash.txt.
   - If it's start-up only, move it behind the title or a loading screen, and make the bench not count it as a hang. Then recheck sprint's swatches (236).
4. **Indoor step 2: bake the interiors.** One JCK1 per zone plus `-night` variants. The loader switches pack at the door, and the presenter's `baked` follows the zone. Do this after 1–2, so the gain is measured alone.
5. **Then the ranked list:**
   - mob sim (Step 3, the 100-mob work, with a `horde` scene);
   - night lamp pools (7.7 / 11.2 ms);
   - wood `draw` 5.9 ms and sprites 2.6 ms;
   - audio 13.5–22%;
   - mob `water_fx` 3 ms on the GE;
   - ge_build;
   - the one-image-per-tree-shadow idea (needs an A/B).
6. **Low priority:** town-night's slow reads (72 ms over 207). Check pack layout and seek order.

Each step: an emulator bench before and after, then a hardware build for the owner.
