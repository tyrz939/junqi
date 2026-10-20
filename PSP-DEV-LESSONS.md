# PSP homebrew: lessons from a real port

A practical guide to PSP homebrew development. It is drawn from porting *The Bell at Nine* (repo `Junqi`: a Rust, integer-only, deterministic pixel-art RPG) to a PSP-1000, from October 2026. The rules come first and the project's numbers serve as examples. Use it for any PSP project, in Rust or C.

**How sure each number is:**
- **[hw]**: measured on a real PSP-1000 (the owner's unit, 5.50 Prome-4 CFW).
- **[emu]**: measured on PPSSPP. That proves correctness, not speed.
- **[host]**: measured on the PC.
- **[est]**: estimated or reasoned, not measured.
- **[unverified]**: believed true, but not checked by this project.
- No tag: well-known platform fact, or a rule this project learned.

---

## Contents

1. Hardware cheat sheet
2. Toolchain and build (Rust, cargo-psp, no_std)
3. Emulator vs hardware: how to measure
4. CPU performance rules
5. Memory
6. GE and rendering rules
7. Audio and the Media Engine
8. I/O, streaming and compression
9. Threads, timing and frame pacing
10. Debugging on the device
11. Testing strategy
12. CFW and kernel mode
13. Day-one checklist for a new PSP project
14. Pitfalls we hit (symptom, cause, fix)
15. Where to look in this repo

---

## 1. Hardware cheat sheet

| Part | Fact | Notes |
|---|---|---|
| CPU | Allegrex, MIPS32-class, **in-order, single-issue**, 1 to 333 MHz | **Default 222 MHz CPU / 111 MHz bus.** Homebrew can set 333/166 (`scePowerSetClockFrequency(333, 333, 166)`) |
| Caches | 16 KB I-cache and 16 KB D-cache, **no L2** | Cache line usually given as 64 B [unverified]. A miss goes to slow main RAM: scattered reads are what hurt most |
| Scratchpad | 16 KB at `0x00010000`, as fast as the cache | Free to use; this project hasn't used it yet |
| FPU | **Single precision only.** `f64` is soft-float (slow) | Also has the VFPU (128 float registers, SIMD). Works from Rust through rust-psp's `vfpu_asm!` [emu]: see #32 and PLAN-PSP-SPEED.md "VFPU: where it pays" |
| Missing ops | No 64-bit divide in hardware | `i64`/`u64` `/` and `%` call `__divdi3` and friends (100+ cycles). A 32x32→64 multiply (`mult`) is one instruction |
| Atomics | **32-bit only** | No `AtomicU64` |
| Media Engine (ME) | A second MIPS core with its own 2 MB eDRAM | Firmware uses it for codecs (sceAtrac, sceMp3). **Your own ME code needs kernel mode (CFW)**, and PPSSPP doesn't run it (§7, §12) |
| RAM | PSP-1000: 32 MB, **24 MB user partition**. PSP-2000 and later: 64 MB, about 52 MB user with `MEMSIZE=1` in PARAM.SFO | PSP-1000 also has a **4 MB "volatile" block** (`sceKernelVolatileMemLock`) [unverified on our unit] |
| VRAM | **2 MB** eDRAM, framebuffers included | Not 4 MB: this project got it wrong once |
| GE (GPU) | Fixed-function: textured quads, triangles, lines; blend, stencil, CLUTs; a display list it runs while you write it | 8 KB texture cache (16 B x 8-row blocks). It can texture from main RAM too, sharing the bus with the CPU |
| Screen | 480 x 272, framebuffer stride 512 px, 60 Hz vblank | One 8888 buffer = 512 x 272 x 4 = 557 KB; two = 1.11 MB |
| Textures | Power-of-two sides, at most 512. Formats T4, T8 (CLUT), 5650, 5551, 4444, 8888, DXT | Swizzled textures read faster |
| Audio | sceAudio channels, plus a sample-rate-converter (SRC) channel; SAS (sceSasCore) voice library | §7 |
| Storage | Memory Stick (`ms0:`); `host0:` under PPSSPP or psplink | Slow and seek-heavy; a file handle may go stale after sleep |

**How far below a PC.** Expect **50 to 150 times slower** than a modern desktop core for the same Rust code:
- [hw/host] The terrain painter: about 2 ms a chunk on the host, about 100 ms on the PSP (50x).
- [hw/host] The integer audio mixer: 32 µs a block on the host, about 4.2 ms on the PSP (130x).

The reasons stack: a 333 MHz clock, in-order single issue (no overlapping of independent work), small caches with no L2, and slow RAM. Pointer chasing and scattered reads (hash maps, trees, sparse grids, per-pixel table lookups) sit at the high end of that range. Streaming copies sit at the low end.

---

## 2. Toolchain and build (Rust, cargo-psp, no_std)

### 2.1 What works

- **Rust:** nightly, plus `rust-src`, plus `cargo-psp` (0.2.10 used) and the `psp` crate (rust-psp 0.3.x).
  - Install: `cargo install cargo-psp`, then `rustup toolchain install nightly -c rust-src`.
  - Build: `cargo +nightly psp --release`. It gives a `.prx` and an `EBOOT.PBP`, with **no pspdev SDK and no WSL** needed on Windows.
- **Target:** `mipsel-sony-psp`, using `build-std` (`core`, `alloc`).
  - **`std` does not build** for it: no allocator, no sync, no io.
  - Every crate the console links must be `no_std + alloc`.
- **Pattern that keeps PC builds unchanged:** each shared crate gets a **default-on `std` feature** plus a `no_std` build.
  - The console build sets `default-features = false`.
  - **A CI job builds every console crate for `mipsel-sony-psp` with `build-std=core,alloc`,** so `std` can't creep back in. This is the most valuable gate in the port.
- **Keep the console build out of the main workspace.** Put it in its own `[workspace]` with its own `rust-toolchain.toml` (nightly) and `.cargo/config.toml`. Then PC and CI stay on stable.
- **Release profile used:**
  - `opt-level = "s"` overall, because RAM holds the program too;
  - opt 3 on the hot crates (painter, renderer, mixer) and opt 2 on the presenter;
  - `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`.
  - The program was 4.4 to 5.5 MB as a `.prx`; that size comes straight out of the 24 MB.
- **`Psp.toml`:** sets the XMB title (PARAM.SFO).
- **`MEMSIZE=1`:** this project's own `memsize.py` patches it into the EBOOT, giving a PSP-2000+ its larger partition.
  - A loose PBP without it gets 24 MB on any model.

### 2.2 Porting the code to no_std: what broke

| Problem | Fix |
|---|---|
| `std::` paths | Most are re-exports: switch to `core::`/`alloc::` |
| `HashMap` | Use `hashbrown`, or better, a `BTreeMap` or sorted `Vec`. A default `HashMap` seeds its hasher randomly, which breaks determinism anyway |
| `IndexMap` has no default hasher without `std` | `BuildHasherDefault<FnvHasher>`; `new()` becomes `default()` |
| `OnceLock` | `once_cell::race::OnceBox`, or build the value at start-up |
| `AtomicU64` | Split on `cfg(target_has_atomic = "64")`, or use `AtomicU32`. Keep it out of anything hashed or saved |
| `Arc` | Works at 32 bits; check at link time. If there are no threads, use `Rc` |
| Float maths (`sin`, `sqrt`, `round`) | `core` lacks these methods without `std`. Use integer forms: Q12/Q15 fixed point, `isqrt`, `iatan2`, sine tables. `libm` is the other choice. Avoid `f64` entirely |
| `usize` | 32 bits on PSP, 64 on PC. Any `usize` that reaches state, a hash or a save is a cross-target bug waiting to happen. Use fixed-width types there |
| Dependencies | `postcard`, `lz4_flex`, `xxhash-rust`, `serde`, `indexmap`, `talc`, `hashbrown` all work `no_std`. Turn off their `std` features in the workspace manifest |

### 2.3 Gotchas with cargo-psp and rust-psp

- **`stub-only`** (rust-psp's syscall stubs without its runtime) is what you want with your own allocator. But it drops:
  - **`memcpy`/`memset`:** add `[unstable] build-std-features = ["compiler-builtins-mem"]` to `.cargo/config.toml`;
  - **`module!`:** it calls the missing `catch_unwind`, so write the module header by hand;
  - **a useful panic handler:** its handler spins forever. Vendor rust-psp (`[patch.crates-io]`) and make the handler call yours (§10).
- **rust-psp's default allocator takes one kernel block per allocation.** That crashed PPSSPP part-way through a big build (the block table, probably). Use your own (§5).
- **Unoptimised builds can fail to link:** big functions overflow MIPS's 16-bit branch offsets. CI builds the console crates at `opt-level = 1`.
- **The target spec says `"cpu": "mips2"`.** So LLVM **never emits the Allegrex's extra instructions:** `madd`/`msub`, `min`/`max`, `clz`, `movz`/`movn`, `ext`/`ins`, `seb`/`seh`.
  - To get them: inline `asm!` in the few hottest loops,
  - or try `-C target-cpu=mips32r2` and scan the ELF for opcodes the Allegrex lacks (a spike planned here, not yet done).
- **`sceGuSignal` in rust-psp puts the signal id where the GE reads the behaviour.** The list stops and you get a black frame. Write the SIGNAL and END commands by hand.
- **`sceGuGetMemory` checks nothing.** A display list that outgrows its buffer silently overwrites the GU's own statics, and crashes later somewhere unrelated. See §6.6 and §14.
- **cargo-psp names the EBOOT after the crate** once `Psp.toml` sets a title (`psp-game.EBOOT.PBP`). A stale `EBOOT.PBP` next to it once shipped an old build.
  - The dist script takes the named file and **checks that the fresh `.prx` is inside the zip.**
  - Zip with forward slashes: PowerShell's `Compress-Archive` writes backslashes, which some unzip tools reject.

---

## 3. Emulator vs hardware: how to measure

### 3.1 PPSSPP is a correctness tool, not a stopwatch

- **CPU time on PPSSPP (JIT) is not hardware time.** PPSSPP doesn't model the Allegrex's pipeline, cache misses or RAM latency. It runs about one instruction a cycle with no stalls.
- How much it understates depends on the work:

| Work | Emulated vs real PSP-1000 |
|---|---|
| Streaming integer compute (the terrain painter) | About **3.2x** faster on PPSSPP: it lands 3.5 chunks a second, the hardware 1.1 [emu/hw] |
| A whole game frame (presenter, lister, sim) | **6 to 8x** faster on PPSSPP [hw] |
| Scattered-memory code (table lookups per px, tree walks) | Worse still. A stand-in painter "a few ms" on PPSSPP took about **20 ms** on hardware [emu/hw] |

- **The GE is effectively instant on PPSSPP.** `sceGuSync` returned in 11 µs. So **GE fill cost is invisible** in the emulator.
  - The first hardware run showed a 51 ms GE wait at 11.6 fps, in scenes PPSSPP ran at 60 [hw].
- **PPSSPP emulates neither the CPU data cache nor the GE's view of it.** Cache-coherency bugs (§6.7) never show there.
- **PPSSPP keeps VRAM and file handles across sleep;** hardware may not. It also stubs out `sceKernelRegisterDefaultExceptionHandler`.
- **PPSSPPHeadless hard-codes the PSP-2000** (about 52 MB) and ignores `--appendconfig`.
  - This project patched `headless/Headless.cpp` so `PPSSPP_PSP1000=1` in the environment picks the PSP-1000 (24 MB).
  - **Always test memory as a PSP-1000.**

### 3.2 What PPSSPP *is* good for

- Proving determinism: state hashes match the PC bit for bit (§11).
- Memory: `sceKernelMaxFreeMemSize` and `sceKernelTotalFreeMemSize` behave like the hardware's partition.
- Crashes: it reports CPU faults with a call stack, which the game can't on hardware.
- Scripted walk tests and headless screenshots, in CI-like loops.
- Its interpreter (no `-j`) checks alignment; the JIT (`-j`) is fast enough for long runs.
- Headless runs:
  - `PPSSPPHeadless.exe game.prx --root=<dir> -j --timeout=900 --screenshot-save=x.png`.
  - Pass a `.prx`, not the EBOOT. The program must end with `sceKernelExitGame`.
  - Text written with `sceIoWrite` to fd 1 is captured; `dprintln!` is not.
- Building PPSSPP: clone with submodules, each at its **pinned** commit (a `--depth 1` clone leaves them empty), build SPIRV-Cross, then `msbuild ... /t:PPSSPPHeadless`.

### 3.3 Calibrate, then trust only the hardware

1. **Put a frame profiler in the game from day one** (§10.3): CPU time by part, GE time by pass, counts (quads, binds, CLUT loads, uploads), and free memory. Let the owner save a capture with a button combination.
2. **Read hardware captures to get scale factors,** per kind of work. This project's are above.
3. **Add a handicap to the emulator for the work that matters most.**
   - Here: `slowpaint:330` makes the painter thread spin for 230% more of its own time, so PPSSPP paints as slowly as the hardware.
   - The walk test runs with it, so stand-in ground bugs showed up on PC before the owner saw them.
4. **Budgets are set in hardware milliseconds.** An emulator number is a hint that something got faster or slower, never "it fits".
5. **Measure at the clock you will ship.** This project ran at 333/166. The owner's goal is 60 fps at the **default 222/111** (about 0.67x the CPU and bus, which also slows the GE's memory traffic), so the bench needs a 222 MHz case.

---

## 4. CPU performance rules

- **Count cycles per item, not per frame.**
  - 333 MHz at 60 fps is about 5.5 M cycles a frame, and audio and the OS take some of that.
  - A per-pixel cost of 300 cycles over a 256 x 256 chunk is 20 M cycles (60 ms).
- **Kill 64-bit divides and remainders on hot paths.** On `mips2` they are `__divdi3`/`__udivdi3`/`__moddi3`/`__umoddi3` calls, 100+ cycles each.
  - Use `i32` where the range is proven (`debug_assert!` plus a comment).
  - A constant divisor becomes a reciprocal multiply plus a shift, but only where it is exact.
  - A 32x32→64 multiply is cheap; keep it.
  - **Example:** `isqrt(u64)` was Newton with a u64 divide each step, about 700 cycles. The fix is a divide-free bit-by-bit root with a u32 fast path. It is exact, so results can't change.
  - **Example:** one i64 divide per pixel in the ecotone painter cost about 20 ms a chunk [est].
  - **Example:** the lamp shadow projection used 64-bit divides and roots. Moving it to 32 bits where it fits (with a test holding it to the 64-bit version) took the frame's 3.5 ms to 1.5 ms [emu].
- **Guard it:** disassemble the PSP ELF (`llvm-objdump` from `rustup component add llvm-tools`). Fail the build if a hot-list function calls a 64-bit divide helper, and ratchet the total count of call sites downward.
  - This project's scan found 170 sites (`div64-scan.txt`). The guard is planned, not built yet.
- **No floats in the sim** (determinism across targets), and no `f64` anywhere on the PSP.
- **Memory layout beats arithmetic.**
  - Read rows and bands, not cells through an abstraction. The console county build halved its time from that alone: 65 s to 33 s emulated, byte-identical output.
  - Replace `BTreeSet`s and hash sets on hot paths with bit planes.
  - Use byte tables in place of branches.
  - Keep loops free of bounds checks and divides.
- **Cache answers that change rarely.** Recompute on an event, not every tick.
  - Prop quest marks were rebuilt per prop per tick (0.8 ms). They are now re-asked every 8 to 32 ticks, and at once on an event that can change them (0.05 ms).
  - Tiles read neighbour by neighbour went into a per-tick window (0.8 to 0.4 ms).
- **Bound worst cases, not averages.**
  - A mob's path searches got a hard per-tick node budget: 2,048 nodes, shared, deterministic, and a search cut short keeps its old path.
  - The worst sim step went from 302 ms to 56 ms [hw]. The worst tick went from 12,002 nodes to 2,049.
  - **One 300 ms frame is what players remember.**
- **Never do a big job in one frame.** Spread it across frames, put it on a worker, or do it under a loading screen (§9). A view first painted on the first play tick was a 2.3 s freeze; it moved under the loading scene.
- **Per-crate `opt-level`:** opt 3 on hot crates, `s` elsewhere, buys speed without a big binary.
- **Profile on hardware before optimising.** The guesses were wrong several times:
  - "the mixer is slow because of the reverb": it was the voices, at 73%;
  - "the sun pass is caster-bound": it was fill.

---

## 5. Memory

### 5.1 The budget (PSP-1000)

| | MB | Source |
|---|---:|---|
| User partition | 24 | |
| Program loaded, with its stacks (`.prx` 4.4 to 5.5 MB) | 3.6 to 4.6 | [emu] |
| Largest free block at boot | 20.4 to 20.7 | [emu] |
| Free in play, the whole game on | **about 3.2** (largest block about 2.7) | [hw] `perf.txt` |
| Free with the map open | 2.3 (largest 1.8) | [emu] |

**Where the rest went** [emu]:
- sim 6.2 to 6.7 MB;
- art pages in RAM 1.25 MB;
- terrain chunk slots 1.8 MB;
- audio 0.81 MB;
- presenter about 5.4 MB;
- display lists 2 x 256 KB;
- the dashcam ring 178 KB.

### 5.2 Rules

- **Budget RAM per system from day one** and gate it on the PC. Here `tests/heap_budget.rs` holds the console form: a build peak of 9 MB and a sim of 7 MB, about 5% over the last measure, ratcheted down with each cut.
  - The tool behind it: a counting `#[global_allocator]` on the host that reports live and peak bytes per stage.
- **Design for "never resident together."** The world build and the running sim each fit; both at once don't. Build behind the loading screen, then free the build's scratch.
- **The data diet that took the world from 52 MB to 6 MB**, with every hash unchanged:
  - flag planes as bits, not bytes or bools;
  - sparse maps for sparse data;
  - drop each plane once no later stage reads it;
  - `shrink_to_fit` finished data;
  - rare fields boxed out of line (128 B rows down to 32 B);
  - strings interned once;
  - grids packed as 16 x 16 chunks with a per-chunk palette (1/2/4/8 bits);
  - paged overlays instead of full planes;
  - compact pathfinding nodes (16 B to 8 B);
  - zones built or loaded on demand.
- **Fragmentation is the real limit, not total free memory.**
  - The first county build failed on **one 4 MB allocation with 6.6 MB free in pieces** (largest 3.4 MB) [emu].
  - Watch `sceKernelMaxFreeMemSize` (largest block), not just the total.
  - Avoid single huge allocations. The first PSP play hung on a **16 MB walk grid** over the county (the route lines; turned off on console).
- **Allocator that worked:**
  - a **`talc` pool** (5 MB, claimed once at boot from the low end) for allocations under 64 KB;
  - **one kernel block each** (`sceKernelAllocPartitionMemory`, high end) for bigger ones, 64-aligned so the GE can read them;
  - each falls back to the other;
  - a counter of live and peak bytes, and of failures, on both.
  - Small allocations of 1 KB or more are 16-aligned, so GE-read buffers can be read in place.
- **The allocator isn't thread-safe,** and a spin lock never yields to a lower-priority thread. So once threads run, every alloc and free holds the scheduler (`sceKernelSuspendDispatchThread` / `Resume`).
  - Better still: workers allocate nothing in steady state.
- **Make thread stacks at boot,** while a big enough block still exists.
  - Size them from measurement (`sceKernelGetThreadStackFreeSize`): the main stack used 35 KB, so 128 KB; the painter used 3 KB, so 32 KB.
  - A one-off job (the world builder, 384 KB of stack) gets a thread made for it and freed after.
  - A background job starts only if the largest block holds its stack plus 512 KB.
- **Fail soft.** An infallible `Vec` growth that fails calls `handle_alloc_error`, which panics.
  - On hot render paths use `try_reserve` and **drop work with a counter** rather than crash.
  - The renderer's quad buffers cap at 11,264 quads, the sun's pass at 7,168, and polygons at 1,024 a frame.
- **Squeeze-test it.** A `ballast:KB` script word holds back kernel blocks to run short on purpose; `ballast:auto2048` allocates until 2 MB remain. Run the heaviest scene under it.
- **Spare RAM:**
  - the **16 KB scratchpad** (mixer state, decoder tables; split ranges per thread);
  - the PSP-1000's **volatile 4 MB** (`sceKernelVolatileMemLock`, user mode) [unverified: lock and sleep/resume behaviour still to test on hardware]. Good for caches you can lose.

---

## 6. GE and rendering rules

### 6.1 VRAM is 2 MB: plan it like a memory map

What this project fits:

| Region | Size |
|---|---|
| Two 8888 framebuffers, 512 x 272 | 1.11 MB |
| Lightmap render target, 256 x 140, 8888 | 140 KB |
| Lamp pool cache, 16 x 128² T8 | 256 KB |
| 8 page slots, each a 256² T8 plus a 1 KB CLUT (66.5 KB) | 0.53 MB |
| **Total** | **2.05 of 2.10 MB** |

- Everything else is drawn **from main RAM:** big pages, terrain chunks, UI images. The GE can texture from RAM, more slowly, and it shares the bus.
- **No depth buffer:** draw in order. Through-mode 16-bit integer vertices.
- **Keep 8888 if you need stencil.** On the PSP **the stencil lives in the framebuffer's alpha**:
  - 5650 has none;
  - 5551 has 1 bit;
  - 4444 has 4 bits but 12-bit colour, which bands the dark ramps;
  - 16-bit would halve fill bandwidth, but only if your effects need no multi-bit stencil.

### 6.2 Fill rate is the GE's budget

- **The costly passes are the ones that touch every pixel:** full-screen quads, read-backs of the frame, stencil clears, and large unclipped quads.
- [hw] The sun-shadow pass was **4 to 5 ms, 43% of the GE**, even at day with no lamp work.
  - Cause: a full-canvas quad clearing the stencil, plus **one 256 x 256 mark quad per terrain chunk, not clipped to the screen**: 390k to 590k px rasterised against a 130k px canvas.
  - Fix [plan]:
    - clip to the canvas and to the cells that have relief (83% are flat);
    - clear the stencil with `sceGuClear`'s stencil bit, nearly free, instead of a quad.
- **Rough fill estimate:** about 150 Mpx/s for a textured read-modify-write pass [est]. The whole screen is 130k px, so about 0.9 ms a full-screen pass.
- **Sum your full-screen passes per frame and keep the count small.** Fold work into CLUTs (6.4) instead of extra passes.
- **Read-backs in slices:** a 480-px-wide quad reading a 32-bit texture fetches each 16 x 8 texture-cache block once per row.
  - Cut it into slices 16 texels wide and each block serves all 8 rows: the SDK's "blit in slices", about 3x faster per its sample [unverified on hw for this project].
- **Bound effect passes by what they cover.** The water's masked passes use the water's bounding box, which is most of the screen by a diagonal lake. Use row spans instead.
- **Many tiny quads cost too:** per-primitive setup on the GE, and command bytes and CPU time to emit.
  - The water's crests were one 1-px-tall quad per run per row.
  - Use a texture, or CLUT cycling, instead.

### 6.3 Textures, binds and CLUTs

- **Paletted pages:** T8 256 x 256 pages, each with its own 256-entry CLUT in 8888.
  - The CLUT is 1 KB beside 64 KB of pixels.
  - 8888 keeps 24-bit colours exact, where 5551 would requantise every ramp, and allows a partial alpha (the contact shadow).
  - Pack so each page's sprites share a CLUT exactly; fall back to integer median cut only when a set exceeds 254 colours (none did).
- **Swizzle textures at bake time** (16-byte x 8-row blocks, blocks row-major). Terrain chunks read unswizzled from RAM are a known cost here.
- **Binds and CLUT loads cost CPU and GE.** Each page bind reloads its 1 KB CLUT.
  - Typical [hw]: 84 to 172 binds and 110 to 190 CLUT loads a frame.
  - At one dense place the y-sort interleaved pages and **binds went from about 200 to 1,600** (nearly one per batch).
  - Fixes:
    - skip a rebind when the key repeats;
    - sort by page within overlap-free windows (sprites whose rects don't overlap may swap order);
    - bake sprites that appear together onto shared pages.
- **Uploading to VRAM:** a CPU copy of 66 KB through the uncached mirror mid-list costs about 1 ms [est].
  - Use `sceGuCopyImage` from the already written-back RAM page, so the GE moves it [plan].
  - Spread uploads out: at most one a frame, after the flip.
- **Texture state leaks between draws:** a bilinear texture drew nearest after another. Reset the GE state (filter, blend, stencil, pixel mask, wrap) at the end of each list, and set the filter per bind.

### 6.4 CLUT tricks (cheap, and they don't cost fill)

- **Night and colour grading folded into CLUTs:** grade each page's CLUT once per grade change (1 KB a page) and the vertex colours as drawn.
  - This replaced three full-screen 32-bit read-backs plus four saturation passes with one flat pass.
- **Tags in CLUT alpha:** water, wet ground and shine pixels carry an alpha tag in the chunk's CLUT. Drawing the chunk through a masking mode marks the stencil only where the tag is, with no extra layer.
- **Lighting by CLUT:**
  - normals as a T4 page of 16 directions, lit by a per-frame "sun CLUT";
  - a per-page glow CLUT added over the light;
  - a per-sprite CLUT toward a nearby lamp.
- **Palette cycling** for animated water [plan]: cycle CLUT entries instead of drawing animated geometry.
- **Seat or team colours as CLUT swaps** rather than more pixels [proposed].

### 6.5 Render targets, stencil and passes

- **An offscreen render target** in VRAM for the lightmap, at half resolution:
  - clear to the ambient light;
  - add pools as soft discs;
  - mark shadows in its stencil;
  - multiply over the frame, stretched bilinear.
- **Cache static work as small textures:** each still lamp's pool and its static shadows as a 64 x 64 T8 texture (4 KB), rebuilt at most one a frame. A lamp then costs one quad, about 0.1 ms.
- **Give each stencil value one meaning per pass and write the meanings down.**
  - Bug here: the light shafts read stencil value 1 as "shade", but raised terrain was also marked 1. On hardware, where the painter lagged and stand-in ground raised every cell, the shafts lit the whole screen.
  - Fix: raised terrain is 2 and shade is 1.
- **Pixel mask** (`sceGuPixelMask`) lets you write only alpha (stencil) or only one colour channel.

### 6.6 The display list

- **Size it, plan it, and guard it.** The list is a static buffer (256 KB here), and `sceGuGetMemory` doesn't check the end.
  - **The v0.4.x hardware crash was a list overflow:** a lantern in a wood of about 2,000 blocks wanted about 8,100 shadow polygons, a 589 to 629 KB list.
  - It overwrote the GU's statics, and faulted later at an unrelated address.
- **Fix:** estimate the frame's list cost before writing a byte, never under. If it won't fit, drop passes **least-missed first** (faint shadow bands, then glow, then fog), never terrain, sprites or light. Test the room before every batch, and count the frames that didn't fit.
- **Two lists, pipelined:** the GE draws frame N while the CPU builds N+1 (§9.3).
- **Call lists** (`sceGuCallList`) for static segments that don't change frame to frame [plan]: they cut the CPU's list-building time (3 to 6.5 ms a frame [hw]).
- **Per-pass GE timing:** put a SIGNAL (behaviour *continue*) and an END at each pass boundary. The GE's signal handler stamps the time. Pass time is one signal to the next, read a frame late.
  - Turn this on only while profiling; it adds list bytes.

### 6.7 The CPU data cache and the GE

The GE reads RAM, not the CPU's write-back data cache. PPSSPP models none of this, so get it right by rule:
- write the list and its vertices through the **uncached mirror** (rust-psp's `sceGuStart`/`sceGuGetMemory` do);
- do **one `sceKernelDcacheWritebackAll` before `sceGuStart`** for everything the CPU wrote (pages read from the stick, landed chunks, CLUTs, UI images);
- for anything written while the list runs, **write back by range** before its command, and only into buffers no earlier command of that list reads;
- write VRAM through the uncached mirror, never while the list may read it;
- **flush the texture cache** at each list's start, after each VRAM upload, and before reading a render target as a texture;
- **free or rewrite nothing the list reads until it finishes.** With pipelining, keep freed pages in a "grave" until the GE is done, and **fence** buffers the in-flight list reads;
- the CPU reads GE-written memory only through the uncached mirror;
- write the audio buffer back before handing it to the audio DMA.

### 6.8 Rendering design rules

- **Keep one renderer contract across platforms.** The PSP is one more backend for the same frame description, with a feature tier (C2) that leaves passes out. Don't fork the game logic for it.
- **No runtime generation on the console:** bake art offline into console-ready formats (§8). Motion is a draw-time deform of one static sprite, not baked frames.
- **Compare against the PC at one state.** Run the PC renderer at the console's resolution at the same state hash, and compare mean RGB and the pixels. The look was checked this way to within 1 to 2 levels.

---

## 7. Audio and the Media Engine

### 7.1 What was built

- An integer tracker: patterns plus a sample bank, rendered at the bake from the PC's synth.
  - It was chosen over porting the synth by measurement: the tracker was 7.6x cheaper on the host before the synth's floats were even ported.
- **Samples in VAG** (PlayStation 4-bit ADPCM: 28 samples in 16 bytes, five predictors, a shift per block). IMA ADPCM lost 10 to 15 dB SNR on bright instruments.
  - Rates by content: 11/16/22 kHz.
  - 0.81 MB resident; long rare cues stay on the stick and stream into one slot.
- **Mixer:**
  - 24 music voices, 16 effects, 10 beds;
  - 22.05 kHz, mono groups with pan and send;
  - half-rate where the sample allows;
  - integer reverb and limiter;
  - no allocation after start.
- **Output:**
  - an audio thread at priority 16 (above the game's 32);
  - 512-frame blocks into four 64-aligned buffers;
  - the **SRC channel at 22.05 kHz**, blocking.
- **Commands** come from the game on a lock-free ring of 32-bit atomics (no 64-bit atomics on MIPS).
- **Proof:** a scripted run writes its command log and its output WAV. The host tracker replays the log **bit for bit** (0 of 3.25 M samples differ).

### 7.2 What it costs

- **12 to 22% of the CPU on hardware** [hw]: 20% in captures, 13.5% in a perf log.
  - PPSSPP showed 9 to 13%.
  - Per frame at 60 fps that is about 3.4 ms of 16.7.
- **Where it goes** [host profile]:
  - voices (ADPCM decode, interpolation, gain) 73%;
  - groups 8%;
  - reverb and limiter 17%.
- About **0.7% of the CPU per live voice;** typically 17 to 19 voices are live, 29 at most.
- **Audio spikes after a hitch:** after a long frame, the thread mixes the backlog (40 to 75 ms). Fix the hitches, and keep sound latency in mind: 512-frame blocks plus 3 queued is about 90 ms.

### 7.3 Options to get it down (ranked)

1. **Optimise the mixer** (bit-exact):
   - no bounds checks in the inner loop;
   - Allegrex `madd`/`min`/`max` via `asm!`;
   - a bigger inner chunk (32 to 64 frames);
   - cull voices under about -54 dB;
   - keep the hottest short samples as PCM;
   - voice state in the scratchpad.
   - Expected 20% down to 10 to 12% [est].
2. **SAS (`sceSasCore`):** the firmware's 32-voice ADPCM mixer with ADSR, pitch and volume, mixed into your buffer with `__sceSasCoreWithMix`.
   - VAG is its native format.
   - rust-psp has no bindings: add the NIDs, and load the module with `sceUtilityLoadModule`.
   - **[unverified] whether SAS mixes on the ME or the Allegrex:** no source was found either way, so measure it on hardware.
   - Its envelopes and reverb differ from yours, so output isn't bit-exact (the owner accepted that).
   - PPSSPP emulates it, but only hardware timing means anything.
3. **Firmware codecs on the ME in user mode:** `sceMp3`/`sceAtrac` decode on the Media Engine.
   - This is pre-rendered music: cheap on the CPU, but loses the tracker's dynamic transitions.
   - Here it is the last resort.
4. **Your own code on the ME** (§12): run the whole mixer there. This needs kernel mode (CFW), and **PPSSPP doesn't emulate it,** so you test only on hardware.
   - Expected about 20% down to about 0.5% [est].
   - Keep a user-mode fallback and a watchdog that switches back.
   - Prior art: "Unlocking the PSP's dual core setup", wololo.net, June 2026; PPSSPP's Media Engine notes.
5. **Don't:** mix music at 11 kHz (audibly dull), or pre-mix beds (you lose independent fades).

---

## 8. I/O, streaming and compression

### 8.1 Bake everything that is fixed

- **If the world is fixed, generate it offline and ship the result.** A PC-built world (a 1-minute PSP build cold [emu]) loads in 0.4 to 0.6 s from a cache [emu].
- Run generators once on the PC. Write a canonical pack, then per-target packers (swizzled T8 pages for the PSP). The console only loads.
- The bake is deterministic and hashed. **The PC atlas and the console pack are checked against each other at bake time,** so they can't drift.
- **Can a tile atlas replace baked terrain? Measure, don't assume.** This county's painted ground:
  - 75% of 16 x 16 cells unique;
  - 39% of 8 x 8 blocks unique;
  - 215k 3x3 tile-context classes.
  - So a tile atlas couldn't reproduce the look. Baked per-chunk pixels can, exactly.

### 8.2 Choose the codec by ratio *and* PSP decode time

Measured on 1,024 real 256 x 256 T8 chunks (independent frames, one per chunk) [host]; PSP times are [est]:

| Codec | KB/chunk (indices) | Host decode | PSP decode | Verdict |
|---|---:|---:|---|---|
| lz4 | 13.9 | 37 µs | about 1 ms | Fast, 2.3x bigger |
| **zstd -19** | **6.04** | 54 µs | **about 2 to 3 ms (C), 3 to 5 ms (`ruzstd`)** | **Picked** |
| zstd -22 | 5.99 | 59 µs | same | No real gain |
| zstd + 112 KB trained dictionary | 6.07 | 68 µs | | No gain |
| zstd, neighbour chunk as prefix | 6.04 | | | No gain, and loses random access |
| zlib -9 | 6.54 | 113 µs | | |
| bz2 -9 | 6.47 | 684 µs | | Too slow |
| lzma -9e | 5.69 | 390 µs | about 15 to 30 ms | 6% smaller for 10x the decode |
| Custom context model + static rANS | 5.13 | 775 to 1,020 µs | about 30 to 60 ms | 12% smaller for 20x the decode |

- **Lesson:** per-pixel entropy decoding on a 333 MHz in-order core is tens of ms. LZ-family decoders sit near memcpy speed.
  - With heavy encode on the PC and cheap decode on the PSP, zstd at high levels was within 12% of the best ratio at about LZ speed.
  - Decoder state is about 100 KB, allocated once.
  - Heights compressed separately (0.6 KB); CLUTs don't compress (194 B).
- **Budget decode against demand.**
  - A sprint brings about 2.3 chunks a second on screen: about 7 ms of decode a second, under 0.15 ms a frame.
  - A teleport wants about 12 chunks at once: plan for the burst, nearest first.

### 8.3 I/O rules

- **Never do synchronous `sceIo` on the game thread in play.**
  - Today's sync page loads are suspected of preempting audio. Use a loader thread and `sceIoReadAsync` [plan].
- **Saves:**
  - write a temp file, then rename;
  - serialize into a buffer on the game thread, and write from a writer thread [plan].
- **Every file read reopens and retries once on failure.** Handles can go stale after sleep and resume.
- **A disk cache with strict validation:** header plus format version plus source stamp plus content hash plus the object's own hash plus length plus xxh3. Anything off, and the entry is refused and rebuilt.
  - Write while encoding (16 KB at a time; a first pass only counts and sums), so nothing is held whole.
- **Search paths:** `argv[0]`'s directory, then `host0:/`, then `ms0:/PSP/GAME/<name>/`. That runs the same way on PPSSPP, psplink and the stick.

---

## 9. Threads, timing and frame pacing

### 9.1 One core: threads don't add throughput

- **All your threads share ONE CPU core** (the ME aside). A worker thread only gets the time the game thread sleeps: the vblank wait and the GE wait.
  - **Threading can hide latency; it can't close a throughput gap.**
  - [hw] The painter landed 1.1 chunks a second; a sprint needed 2.1 to 2.3. No look-ahead fixes a producer half as fast as the demand.
  - The fixes were:
    - make the stand-in look like the ground (sampled real cells, dithered edges);
    - then bake the chunks offline (§8).
- **Priorities** (a lower number runs first):

| Thread | Priority |
|---|---|
| Watchdog | 15 |
| Audio | 16 |
| Game | 32 |
| Background builder or zone loader | 40 |
| Painter | 48 |

- **Starvation guard:** if the game never sleeps (every frame late), a low-priority worker never runs. After 90 frames starved, raise the painter above the game (priority 31) for one job, which bounds the stall at one job (about 100 ms).
- **Hand jobs off without locks:** an atomic pointer pair (TODO/DONE) and a semaphore. The worker owns the job until it posts it back.
  - Stage the landing on the worker, so the game thread only swaps a pointer. The chunk landing went from 26 ms to 4 ms on the game thread [emu].

### 9.2 Sim and frame rate

- **Fixed-rate deterministic sim** (60 Hz here), decoupled from the frame rate. The presenter interpolates the camera.
  - At 30 fps that is about 2 ticks a frame; at 60, one.
  - Per-tick costs double when the frame rate halves. Per-frame costs (draw, list, GE) don't.
- **Step ticks from the display's vblank count, not the wall clock** sampled at the loop top (which jitters by about 5 ms). Clamp at 4 ticks a frame so one hitch doesn't snowball.
- **Locked 30 beats an unstable 60.** A frame that sometimes makes 16.7 ms and sometimes doesn't judders; a steady 30 doesn't.
  - The owner's final bar is 60 on every frame, with about 40% headroom: about 10 ms CPU and 10 ms GE in the busiest scene.

### 9.3 Pipelining and pacing

- **Pipeline the GE:** two display lists. `draw` sends frame N and returns at once. The next `draw` first waits for N (`settle`), shows it, then writes into the other list.
  - Frame time becomes max(CPU, GE) instead of CPU + GE + vblank. The first hardware reading was 28 + 40 + 10 ms serialised [hw].
- **Pace to "the vblank after the last flip, plus N",** not "N vblanks from now."
  - **The microstutter bug:** frames that did 20 ms of work still took 50 ms at locked 30, because the pacer missed a flip it could have made and waited a whole extra vblank.
  - Count missed flips from the display's vcount at each flip, and log them.
- **Give a starving worker an extra vblank** only when the frame has measured room (the last 8 frames' work plus 2 ms under a vblank).
- **Set the clock at boot and again after resume, and read it back:** `scePowerSetClockFrequency(333, 333, 166)`; if short, set CPU and bus separately. Show what is in effect on the overlay.

### 9.4 Sleep and resume

Hardware sleep can lose VRAM contents and invalidate file handles [hw symptom: garbled text after resume]. Register a power callback, and on resume:
- mark every VRAM slot and cached texture empty, and re-upload from RAM;
- set the GE base state and display again;
- reopen files;
- set the clock again;
- re-reserve the audio channel if output fails.

Test it on PPSSPP by simulating sleep (spoil the slots, close handles), because PPSSPP itself keeps them.

---

## 10. Debugging on the device

### 10.1 Crash report (`crash.txt`)

- **A panic is your only crash hook in user mode.**
  - `handle_alloc_error` ends in the panic handler too, so OOM is caught.
  - Vendor rust-psp so its `stub-only` panic handler calls yours.
- **The handler allocates nothing:** format into a static buffer, write with `sceIo*`, then `sceKernelExitGame`.
- **Contents:**
  - panic file and line, and return address;
  - build commit and dirty flag;
  - live and peak heap, free and largest block, allocation failures (and the last failed size);
  - zone, cell, clock and seed;
  - **a ring of the last 32 stage events** ("list", "ge draw", "sound", "ahead"...).
- **There's no user-mode CPU exception handler.**
  - `sceKernelRegisterDefaultExceptionHandler` needs an import rust-psp doesn't have, and PPSSPP stubs it.
  - So a bad load or store on hardware is a hang or a reset, with no report.
  - On PPSSPP the emulator prints the fault and its call stack. Use that: reproduce hardware faults in the emulator under the hardware's conditions (heap squeeze, list caps, slow painter).

### 10.2 Freeze report (`freeze.txt`)

- **A watchdog thread** at the highest priority (15) wakes once a second. If no frame has begun for 8 s, it writes `freeze.txt`, allocating nothing:
  - place, the last stage events;
  - **every thread's status, wait type and object, priority, stack left, and CPU clocks run, sampled twice a second apart.** A spinning thread ran the whole second; a deadlocked one ran none and waits.
- **No `freeze.txt` after a freeze means every thread stopped:** a CPU fault halted the PSP.
- The benchmark has its own watchdog: a 3 s stall writes `bench.txt` and `crash.txt`, and after ten times that it exits so the owner isn't stuck at a frozen screen.

### 10.3 Performance overlay and captures (for real hardware)

- **Overlay (L + R + SELECT):**
  - fps and worst frame;
  - CPU and bus clocks read back;
  - ms by part (sim, presenter, draw, UI, lister, list build, GE wait, vblank wait), audio share;
  - free and largest block;
  - VRAM slots and uploads, RAM page hits and failed loads;
  - painter backlog, missed flips, list-full frames;
  - page 2: GE ms by pass.
  - It is appended to `perf.txt` every 2 s (capped at 256 KB), with **a run header** (commit, settings, seed), so a log can be tied to a build.
  - Off, it costs a flag test; on, about 0.3 ms.
- **Dashcam (L + R + START):**
  - every frame always goes into a ring of the last 600 frames (178 KB, reserved at boot, about 10 µs a frame);
  - the press saves the 10 s before plus the 5 s after to `capture-N.bin`;
  - about 60 fields a frame (CPU parts, 17 GE passes, counts), plus **suspect events** (stand-ins on screen, uploads, cache evictions, fences, resumes, audio underruns) to match against a glitch the player saw.
  - **The format is self-describing** (field names inside), so old captures still decode.
- **Screenshot (L + R + SQUARE):** writes `screen-N.bmp`.
- **Host decoder:** `jane psp-profile capture.bin` gives:
  - mean fps, late share, missed vblanks;
  - mean, p50, p95 and max per field and pass;
  - the ten worst frames with their top CPU parts and GE passes;
  - an HTML report with stacked charts against 16.7 and 33.3 ms.
- **Reserve diagnostic buffers up front,** never at the moment of failure: memory is lowest exactly when you need the report.

### 10.4 Make the owner's testing cheap

- **A built-in benchmark row on the title screen:** six scripted scenes (sprint, wood, town at night, cellar, mob, churn) on a fresh unsaved game, one line each in `bench.txt`. The tester gets comparable numbers without playing.
- **Button combinations for every capture tool,** listed on the Controls page.
- **Ship a dist zip** that unzips to `PSP/GAME/<name>/`, built and checked by one script.

---

## 11. Testing strategy

- **Determinism is the backbone.**
  - The sim is integer-only, with no floats, no `usize` in state, no random hasher, and the same RNG everywhere.
  - The **state hash matches the PC bit for bit** on PPSSPP. This project's smoke test, a 3,600-tick replay tape (`520a733ef4dcf12c`, 60 checkpoints) and New Game and Continue hashes all matched.
  - **Do this first**, before any rendering. It proved postcard, lz4, xxh3 and the whole worldgen identical on 32-bit MIPS and 64-bit x86.
- **Replay tapes:** record on the PC (bot input, a hash every 60 ticks), embed or load them on the console, and verify every checkpoint. A tape is tied to the content hash, so re-record it when content changes.
- **Scripted runs:** a `script.txt` beside the program drives the real game.
  - Presses, holds, stick angles, teleports, weather, hour, effects on and off;
  - `ballast:`, `listcap:`, `slowpaint:`, `nopaint`, `nopipe`, `sleep@`/`resume@`/`stale@`;
  - screenshots at a tick.
  - **A script must step exactly one tick per frame.** Otherwise a slow frame (a zone built on the step) feeds different input ticks, and cold and warm runs diverge though the sim is fine.
- **Walk tests on PPSSPP** (`tools/psp-walk-test.py`), cases as a PSP-1000:
  - calm, heavy, heavy without painting, squeeze, list cap, run, and run in town;
  - with `slowpaint:330`;
  - each prints one table: fps, missed flips, stand-ins, list size, memory, and PPSSPP's own CPU fault report.
- **Same-state visual compare:** the PC renders the console's start state at 480 x 272 and prints the same state hash. Compare pixels or mean RGB. Keep PPSSPP frame dumps byte-identical across optimisations that claim "same pixels".
- **Host tests for the console renderer's pure parts:** the lister, list guard, CLUT grading and quad hashes run on the PC.
- **Gates for every PSP change:**
  - full workspace tests;
  - the `no_std` PSP build in CI;
  - the dist zip check;
  - heap budgets;
  - the world and sim hash fixtures unchanged;
  - the walk test.
  - Cap emulator iteration loops (about 4 runs) to avoid churning.
- **Small, verified steps.** Each diet item or optimisation is its own commit with every hash unchanged. **A change that moves a hash is a gameplay change and is refused** unless it is meant.

---

## 12. CFW and kernel mode

- **User mode (plain homebrew) gives you:** the Allegrex, the GE, sceAudio, SAS, the firmware codecs (which use the ME internally), the volatile 4 MB [unverified], the scratchpad, and power and clock control.
- **Kernel mode (CFW: the owner runs 5.50 Prome-4 on a PSP-1000) adds:**
  - running your own code on the Media Engine (a second 333 MHz core with its own eDRAM);
  - a real CPU exception handler (registering a default exception handler with EPC, BadVAddr and cause) [unverified for this toolchain].
- **Rules for ME work:**
  - shared state in **uncached** memory: command and result rings, mailboxes;
  - the main CPU writes commands; the ME writes results and its own heartbeat counter;
  - a watchdog falls back to the main CPU path;
  - the ME never touches anything the sim reads (determinism stays testable on PC);
  - build the ME code as a flat `no_std` blob, bit-exact with a host build.
- **PPSSPP doesn't run custom ME code.** Every ME feature must have a user-mode path that is the default and the one PPSSPP tests. The ME path is tested on hardware only, off the critical path.
- **Good ME candidates:** the audio mixer, and pure jobs with a frame of latency. Not the sim. Not anything you can't test on the PC.

---

## 13. Day-one checklist for a new PSP project

1. **Pick the floor model: the PSP-1000** (24 MB user, 2 MB VRAM, 222 MHz default). Budget against it.
2. **Set up the toolchain:** nightly plus cargo-psp in its own workspace; `stub-only`, `compiler-builtins-mem`, a hand-written module header, and a vendored rust-psp with your panic hook.
3. **Make shared crates `no_std + alloc`** with a default `std` feature. **CI-build them for `mipsel-sony-psp`** on every push.
4. **Rules from the start:**
   - integer-only and deterministic logic;
   - no `f64`;
   - no `usize` in state;
   - no `HashMap` iteration order in anything observable;
   - no 64-bit atomics.
5. **Write a smoke test that hashes on both PC and PPSSPP.** Then a replay tape with checkpoints.
6. **Build PPSSPPHeadless locally,** patched for the PSP-1000 model. Script headless runs with screenshots.
7. **Write your own allocator:** a pool for small allocations, kernel blocks for big ones, counters for live, peak, failures and largest free block. Make thread stacks at boot.
8. **Budgets as tests:** heap per system, program size, VRAM map, fps per scene, measured in hardware ms.
9. **Diagnostics before features:**
   - `crash.txt` from the panic hook;
   - a `freeze.txt` watchdog;
   - a perf overlay plus `perf.txt` with a run header;
   - a dashcam capture plus a host decoder;
   - a screenshot button.
10. **An on-device benchmark menu** with fixed scenes, so a tester can produce numbers without playing.
11. **Get hardware numbers in week one.** Derive emulator-to-hardware scale factors per kind of work, and add handicaps (like `slowpaint`) to the emulator tests.
12. **Bake everything fixed** on the PC: swizzled T8 pages, CLUTs, compressed chunks (zstd), audio as VAG. Nothing generated at runtime.
13. **Plan VRAM as a memory map,** with framebuffer format chosen by stencil needs. Count full-screen passes. Guard the display list.
14. **Follow the GE/data-cache coherency rules** (§6.7) from the first textured quad.
15. **Pace from vblank counts, pipeline the GE,** and run the sim at a fixed rate.
16. **Keep heavy work off the game thread:** a loader thread with async I/O, a worker for decode, a loading screen for anything over a frame.
17. **Handle sleep and resume:** a power callback, re-upload VRAM, reopen files, set the clock again.
18. **Write a dist script** that stages `PSP/GAME/<name>/` and verifies the zip holds the fresh build.
19. **Hunt 64-bit divides** with an objdump guard on hot functions.
20. **Work in small verified steps:** measure on hardware, change one thing, keep hashes identical, measure again.

---

## 14. Pitfalls we hit (symptom → cause → fix)

| # | Symptom | Cause | Fix |
|---:|---|---|---|
| 1 | `-Zbuild-std=std` fails for `mipsel-sony-psp` | No `std` for this OS | `no_std + alloc` crates with a default `std` feature; CI gate |
| 2 | Link errors for `memcpy`/`memset` | `stub-only` drops rust-psp's | `build-std-features = ["compiler-builtins-mem"]` |
| 3 | `module!` won't compile with `stub-only` | It calls the missing `catch_unwind` | Write the module header by hand |
| 4 | A panic shows as a hang or a headless TIMEOUT | rust-psp's `stub-only` panic handler spins | Vendor rust-psp; its handler calls yours, which writes `crash.txt` and exits |
| 5 | PPSSPP segfaults mid-build | rust-psp's allocator uses one kernel block per allocation | Own allocator: talc pool plus kernel blocks for 64 KB and up |
| 6 | CI link failure on an unoptimised build | Huge functions overflow MIPS 16-bit branch offsets | Build console crates at `opt-level = 1` or higher |
| 7 | Memory looks fine on PPSSPP, fails on a PSP-1000 | PPSSPPHeadless always emulates a PSP-2000 (about 52 MB) | Patch it to read `PPSSPP_PSP1000=1`; test at 24 MB |
| 8 | `dprintln!` output missing in headless runs | Not captured | `sceIoWrite` to fd 1 |
| 9 | An old build shipped to the tester | A stale `EBOOT.PBP` beside cargo-psp's named one | The dist script takes the named file and checks the `.prx` inside the zip |
| 10 | Zip won't extract on some tools | `Compress-Archive` wrote backslashes | Zip with forward slashes |
| 11 | World build out of memory with 6.6 MB "free" | One 4 MB allocation, largest free block 3.4 MB (fragmentation) | Bit planes, packed chunked grids, planes freed early; watch the largest block |
| 12 | First play hung | A 16 MB walk grid over the county for route lines | Off on console; bounded or sparse walks only |
| 13 | **Hardware crash in dense woods (SIGSEGV at 0x10080008, or in bind_page)** | **Display list overflow:** about 8,100 shadow polygons (589 to 629 KB) into a 256 KB list; `sceGuGetMemory` doesn't check; it overwrote the GU's statics | Plan the frame's list cost first; shed passes least-missed first; test room per batch; bounded quad buffers with `try_reserve` |
| 14 | Runs at about 29 fps "locked 30" but microstutters | The pacer aimed at "2 vblanks from now", so frames with 20 ms of work took 50 ms | Pace from the display's vblank count since the last flip; count missed flips |
| 15 | 100 to 400 ms hitches walking into new ground | Terrain chunk painted synchronously (about 100 ms each on hardware) | Painter on a low-priority worker; later baked chunks |
| 16 | Ground shown as flat blocks on hardware, fine on PPSSPP | Painter about 3.3x slower on hardware; demand 2.1 to 2.3 chunks/s, supply 1.1 | `slowpaint:330` in the emulator tests; stand-ins built from real sampled cells; bake |
| 17 | Painter stuck behind; swatches for seconds | The game never sleeps, so a priority-48 worker never runs | Boost to priority 31 after 90 starved frames, for one job |
| 18 | A pale wash over the whole screen for a second | Light shafts read stencil 1 as shade, but raised terrain was also 1, and stand-ins raised every cell | One meaning per stencil value: raised 2, shade 1 |
| 19 | Text and props garbled after sleep and resume | VRAM contents and file handles not kept on hardware | Power callback: re-upload slots, reopen files, reset GE, display and clock |
| 20 | Black frame when adding GE timing | rust-psp's `sceGuSignal` writes the id where the behaviour goes (END or JUMP) | Hand-written SIGNAL plus END commands |
| 21 | A texture drawn nearest when it should be bilinear | GE state left over from the previous texture | Reset GE state at list end; set the filter per bind |
| 22 | Scripted runs end on different hashes, cold vs warm | A slow frame caught up 4 ticks with one frame's stick input | Scripts step exactly one tick per frame |
| 23 | 2.3 s freeze right after the loading screen | First-view setup done in the first play tick | Do it under the loading scene |
| 24 | 56 to 302 ms sim spikes in mob fights | Unbounded path searches in one tick | A hard, deterministic per-tick node budget |
| 25 | A dense spot drops to about 23 fps | Sort order interleaved pages: binds about 200 to 1,600 | Sort by page within overlap-free windows; co-locate sprites on pages |
| 26 | GE takes 4 to 5 ms for shadows in daylight | Unclipped 256² stencil marks per chunk plus a full-screen clear quad | Clip to the screen and relief cells; clear via `sceGuClear` |
| 27 | Emulator says 60 fps; hardware 11.6 | PPSSPP doesn't time GE fill and understates CPU 6 to 8x | Hardware captures; per-pass GE signals; pipelined lists |
| 28 | Freeze leaves no record | No user-mode exception handler; a CPU fault halts all threads | Watchdog `freeze.txt` (none written means fault); reproduce on PPSSPP for the stack |
| 29 | Audio spikes to 40 to 75 ms after a hitch | The mixer catches up on the backlog | Remove the hitches; smaller blocks; async I/O so sync reads don't preempt |
| 30 | Possible GE glitches no emulator shows | CPU data cache vs GE coherency | Follow §6.7 by rule; fence buffers in flight |
| 31 | Dialogue "can't continue" on the pad | Hint named ○ (back on a pad) for "next" | × confirms, ○ backs out, on every screen; hints drawn as the PSP's buttons |
| 32 | (Avoided) VFPU code that would fault or quietly lose its values | `lv.q`/`sv.q` need 16-byte addresses; only `ThreadAttributes::VFPU` threads may run VFPU code, and rust-psp's `cosf`/`sinf`/`fminf`/`fmaxf` are VFPU code under libm's names; `vfpu_asm!` is `.set noreorder`/`noat` (no delay slots filled); the caller crate needs `asm_experimental_arch` | Aligned buffer types (`#[repr(align(16))]`) with a pointer check and an integer fallback; the VFPU attribute on any thread that may touch floats; one `asm!` per quad inside a Rust loop; long-lived values outside matrix 0. The VFPU is a win only where data is wide, regular and 32-bit: not gathers, not `i16`/`u8` data, not serial recurrences |
**Note on the v0.4.1 crash.** The session brief mentioned a 16 MB allocation failure crashing v0.4.1. The repo's records (commits `4d2f877d` and `8619d1ab`, `progress/PLAN-psp-v041.md` §E) say the v0.4.x hardware crash was **reproduced as the display-list overflow** (#13). The 16 MB item on record is the route-walk grid that hung the first PSP play (#12). If there was a separate 16 MB failure, it isn't written down here: **[unverified]**.

---

## 15. Where to look in this repo

| Topic | Path |
|---|---|
| The console plan, budgets, memory diet, bake, tiers, spikes, every measured round | `PORT.md` §13 (about l.550 to 1255) |
| 60 fps plan rev 2: GE fill, water, streaming, compression, audio options, ME, owner answers | `progress/PLAN-psp-streaming.md` |
| Hardware measurements: codecs, cell uniqueness, mixer profile, 64-bit divide scan, capture decodes | `progress/2026-10-18_82_hw-run/` |
| Stutter and crash analysis; v0.4.1 plan | `progress/NEXT-2026-10-10.md`, `progress/PLAN-psp-v041.md` |
| The game shell: allocator, threads, priorities, painter worker, clock, sleep, scripts | `spikes/psp-game/src/main.rs` |
| `crash.txt`, `freeze.txt`, watchdog | `spikes/psp-game/src/diag.rs` |
| On-device benchmark | `spikes/psp-game/src/bench.rs`, `crates/jane-present/src/bench.rs` |
| Perf overlay and `perf.txt` | `spikes/psp-game/src/perf.rs` |
| Zone built ahead on a thread | `spikes/psp-game/src/ahead.rs`, `county_cache.rs` |
| GE glue: VRAM map, slots, CLUTs, coherency rules, list guard, pipelining, signals | `crates/jane-render-psp/src/ge.rs` |
| Frame to quads, passes, list budget | `crates/jane-render-psp/src/list.rs` (and `list/guard.rs`, `list/water.rs`) |
| Capture format (`JCAP`) | `crates/jane-render-psp/src/capture.rs`; decoder `jane psp-profile` in `crates/jane-cli` |
| Lamp cache, lightmap, normals, grade | `crates/jane-render-psp/src/lamps.rs`, `light.rs`, `normals.rs`, `grade.rs` |
| Audio on the PSP (ring, SRC thread) and tracker | `crates/jane-audio-psp`, `crates/jane-audio/src/tracker.rs` |
| Bake and PSP packer (JPK2, swizzle, CLUT packing) | `crates/jane-cli/src/bake.rs`, `bake_psp.rs` |
| Heap budget gate | `crates/jane-cli/tests/heap_budget.rs` |
| Dist zip and walk tests | `tools/psp-dist.sh`, `tools/psp-walk-test.py` |
| First spikes (toolchain smoke, sim replay) | `spikes/psp-smoke/`, `spikes/psp-sim/` |
| History | `git log --oneline -i --grep=PSP` (descriptive messages; e.g. `8619d1ab` list guard, `b2a67915` pacing, `9bb14c57` stand-ins, `b1499501` watchdog, `26d694e2` GE timing and pipelining, `d214066c` cache audit, `aec47ab4` sleep and resume) |
