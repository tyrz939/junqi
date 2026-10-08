# Jane: Port

The plan for the native rewrite. Pair with `PLAN.md` (the game), `ARCHITECTURE.md` (the engine), `ART.md` and `PRESENTATION.md` (how it looks and is driven), `WORLD.md` (the living county), `EXPERIENCE.md` (what happens, minute by minute) and `VERIFICATION.md` (how every claim is proven). This file owns: repo layout, toolchain, targets, the crate map, phases and gates, agent parallelisation, the test strategy, the worldgen port, the data pipeline, and the docs list.

**Rule:** this file is the agreement for the port, as `PLAN.md` is for the game. Work that drifts from it fixes the work or this file first.

**Rule:** no content on an untested engine. Every phase ends in something that runs and a gate a test or a number decides. `archive/phaser-remake-2026/POSTMORTEM.md` is why.

**Rule:** not a 1:1 port. Algorithms and content contracts carry. Architecture is fixed where the audit found it wrong. Seeds do not match the TypeScript and nothing is kept compatible with it.

**Rule (2026-09-27):** the TypeScript build is frozen and deprecated. Nothing in `jane/` is changed, fixed or kept in step with `/data`, the docs or the Rust from here; a `/data` change that breaks it stands. It is read for method where a doc points at it, and moved to the archive at P10.

**Rule:** this is a plan; a claim that something runs is a bug in this file until a phase gate (§7) says otherwise. *(2026-09-26: P0's workspace and CI exist, P1 is under way, P2's skeleton (land, sites, roads, rail, patches, lamps, small places, anchors, threat, checks) is ported; README's "Where it stands" is the record.)*

## 1. What is decided

| | |
| --- | --- |
| Language | Rust, stable, edition 2024, std only, no nightly features |
| Targets | `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc` and `i686-unknown-linux-gnu` (SSE2 baseline, Pentium 4), `aarch64-unknown-linux-gnu` (Pi 3/4), `armv7-unknown-linux-gnueabihf` (Pi 2 and 32-bit Pi OS). Windows 7 and XP later through `*-win7-windows-msvc` (tier 3, `build-std`) and rust9x. One dedicated build per target |
| Numbers | Integer only in sim and worldgen: fixed point, integer noise, squared-distance compares. Floats are a compile error in `core`, `schema`, `data`, `world`, `sim`, `net`, `bot` and `art` (§3.4). Sim positions are `Fx(i32)` at 1/256 px; worldgen noise and fields are `Q16`. `ARCHITECTURE.md` §2 owns every numeric type |
| Fidelity | New seeds. The port is proven by invariants (the carried tests, §9), never by hashes against the TypeScript |
| Play | Deterministic lockstep, 4 seats, LAN over `std::net`. A headless host (`jane serve`) runs on a Pi |
| Content | JSON, hand-edited, compiled and validated by `build.rs` into static Rust data. No JSON parsing at runtime |
| Look | *(Changed 2026-09-26)* Modern pixel art well above SNES, nostalgic but beautiful and impressive first (Sea of Stars class). Internal canvas 768 x 432: the same 48 x 27 cells at 16 screen px per cell, so every worldgen and sim constant stands; people 32 x 40. Normal-mapped dynamic 2D lighting with cast shadows (Elysian Shadows), emissive, per-area fog and atmosphere layers, parallax depth, weather, post on the top tier. All art still procedural: zero hand-drawn grids; the generators emit albedo, normal, emissive and height (`ART.md`) |
| Rendering | *(Changed 2026-09-26)* Three backends behind one scene contract (`PRESENTATION.md` §1): `soft` (T0, CPU, always builds, headless sheets and CI, fallback), `gl2` (T1, OpenGL 2.1 / GLES 2: ancient PCs from about 2006 and every Pi), `wgpu` (T2, Vulkan / DX12 / Metal: modern PCs, Pi 4 and 5). Every visual feature is a row with the tier it needs; lower tiers degrade by rule. SDL2 for window, input, audio |
| Window | The view widens at integer scale to fill the window, no bars; fractional scales use a sharp-bilinear shader on T1/T2. Guarantees (what a screen shows, half-screen distances, the density rule) are stated for 16:9 and hold for anything wider |
| Zone build | New Game builds all 13 zones up front behind a loading screen that shows the skeleton forming |
| Folk variation | Bounded `vary` variants on a look, at most 4 per row, picked by unit id. Seats stay coat-only swaps |
| Hardware floor | *(Changed 2026-09-26)* Three classes, each a gate: a modern PC at T2; an ancient PC (SSE2 CPU, GL 2.1 GPU) at T1; a Raspberry Pi 4 at T1 with shadows. 60 fps must hold in every class at its tier; the `Features` ladder (`PRESENTATION.md` §1.12) is the degrade order and 30 fps is never a pass. Pi 3 and the Pentium 4 at T0 are best effort, recorded, not gates |
| Aim | *(Added 2026-09-26)* Partial aim assist for spells, console-FPS style, in the sim and deterministic, per input profile (`ARCHITECTURE.md` §5.4) |
| Verification | *(Added 2026-09-26)* The proof of play goes 10 to 100 times deeper than the TypeScript suite: player-model bots, experience metrics with bands, truth and cohesion audits across zones, living-world audits, dossiers and film per seed (`VERIFICATION.md`). `EXPERIENCE.md` is what is proven; a claim without a check is a wish |
| World | *(Added 2026-09-26)* The county breathes: schedules, weather, ecology, consequences and rumour as engine state and data (`WORLD.md`, `ARCHITECTURE.md` §4.6); the player's long-running context is a journal the world can read (`ARCHITECTURE.md` §3.7) |
| Method | This planning pass writes documents. The owner executes with parallel agents in worktrees, one crate or module each (§8) |

## 2. Repo layout

```
/                          Cargo.toml (workspace), rust-toolchain.toml, .cargo/config.toml, Cross.toml, clippy.toml
/crates/jane-*/            the fifteen crates (§4)
/data/                     content, moved out of the TS tree; hand-edited; read by build.rs
   *.json, <table>/*.json  the 20 tables, base file plus fragments
   zones.json              the 13 zones in tick order: id, kind, contract, given keys and verbs, states (§5.4)
   dungeons/*.json         missions
   rooms/<dungeon>/*.room  109 templates (moved from jane/src/world/dungeon/rooms)
   chunks/*.chunk          authored county places, now data (§6.f)
   looks/*.json            one look per sprite id and icon id (§5.3, ART.md §5)
   tuning/*.json           tables that were constants in code (§6.g); aim-assist profiles
   weather.json, atmosphere.json, ecology.json, consequences.json   the living world (WORLD.md §10)
   bindings.json           default key, mouse and pad bindings (PRESENTATION.md §4)
   audio/                  sfx.json, instruments.json, songs/*.json: every sound as numbers (PRESENTATION.md §5)
/tests/fixtures/           replays (.jrp), hash files
/tools/                    ci scripts, toolchain notes (rust9x, win7 build-std), pi setup, cross Dockerfiles
/.github/workflows/
/archive/phaser-remake-2026/
/archive/web-remake-2026/  jane/ moved here in P10; POSTMORTEM.md written in P0
/*.md                      root docs (§2.2)
```

**Rule:** content lives at `/data`, outside every crate. A crate reads it only through `jane-data`'s build script or the `jane-schema` library. Presentation data outside the content hash is read by its own crate's build script, which checks it the same way: `bindings.json` by `jane-present`'s, `audio/` by `jane-audio`'s.

**Rule:** `jane/` stays in place, buildable, until P10. It is the reference while the port is proven against invariants. Nothing in it is edited after P0 except doc banners.

### 2.1 The archive note

`archive/web-remake-2026/POSTMORTEM.md`, written in P0, in the style of the Phaser one. Sections: what it proved (headless deterministic sim, the test suite plays the game, lockstep-ready state tree, skeleton-first worldgen with rows as constraints, the lock-and-key solver and C1-C12, templates proven alone, 623 tests); what it got wrong and the port fixes (§6 smells, float determinism never proven across engines, six seed schemes, `hasZone` coupling, god functions, hidden module state, camera size in worldgen); what carries verbatim (data, room templates, test list, the writing); what is dropped (DOM UI, browser storage, save migrations v1-8, art grids, Vite glue). A numbers table: lines per directory, test count, generation times.

### 2.2 Root docs after the port

| Doc | Fate | Edit |
| --- | --- | --- |
| `PLAN.md` | Design truth | Banner: "the build is planned in PORT.md; §4 is the TS record". §8 milestones point at PORT.md phases |
| `STORY.md`, `VOICE.md`, `QUESTS.md`, `QUEST-TREE.md`, `DESIGN-2020.md`, `LEARNING*.md`, `OLD-NOTES.md`, `MISSING-SYSTEMS.md` | Design truth and history | None |
| `SYSTEMS.md` | The bar. Stays | Banner. Every row goes back to SHAPE until a Rust test names it (Rule 0). §11 file map becomes "TS record"; new §11 lists crates |
| `ENGINE.md` | TS record | Banner: superseded by `ARCHITECTURE.md`. Keep: best description of the sim's rules |
| `PLATFORM.md` | Partly stale | Banner. §1, §2, §5-7 stand. §3 (browser/PWA/Electron/Tauri) and §4 (WebSocket relay) are void; pointer to `ARCHITECTURE.md` §7 |
| `WORLDGEN.md` | Design truth, stale facts | Banner. Fix: county is 2000 x 2000; §3 sizes (mine 160 x 128, burial 232 x 184, W = 16 + cols x 36, H = 16 + rows x 28); §8 "adding a zone" for the crate layout |
| `DUNGEONS.md` | Design truth, stale facts | Banner. Fix: bay margin (rim 3 inside 36 x 28), templates path (`data/rooms/`), `oneway` edge kind, sight tiles, entrance rule |
| `README.md` | Rewritten (§10) | |
| `PORT.md`, `ARCHITECTURE.md`, `ART.md`, `PRESENTATION.md`, `WORLD.md`, `EXPERIENCE.md`, `VERIFICATION.md` | New | |

## 3. Toolchain and targets

### 3.1 Pinning

`rust-toolchain.toml`: current stable pinned (bump deliberately, own commit), `components = ["clippy", "rustfmt"]`, the six targets. Edition 2024.

**Rule:** no nightly, no `build-std` in the main workspace. Windows 7/XP builds use their own toolchain files under `tools/win-legacy/`, built by the owner.

**Rule:** the eight float-free crates (`core`, `schema`, `data`, `world`, `sim`, `net`, `bot`, `art`) use only language and std features that exist in Rust 1.85, so a rust9x fork still builds them; CI does a second `cargo check` on pinned 1.85 for those eight.

### 3.2 Per-target flags (`.cargo/config.toml`)

| Target | rustflags | Notes |
| --- | --- | --- |
| `x86_64-*` | `-C target-cpu=x86-64` | Baseline, no AVX |
| `i686-*` | `-C target-cpu=pentium4 -C target-feature=+sse2` | |
| `aarch64-unknown-linux-gnu` | `-C target-cpu=cortex-a53` | Pi 4 is the gate (Cortex-A72 runs A53 code; V3D GPU at GLES 3.1 / Vulkan 1.1); Pi 3 and 5 run it |
| `armv7-unknown-linux-gnueabihf` | `-C target-cpu=cortex-a7 -C target-feature=+neon,+vfp4` | Pi 2 and 32-bit Pi OS; armv6 (Pi 1, Zero) out |

`Cross.toml` for ARM: `cross` images with SDL2 and Mesa dev packages (Dockerfile under `tools/cross/`). x86 targets use the `sdl2` crate's `bundled` feature; the Pi uses the distro's SDL2 and Mesa (V3D for GLES 2/3 and Vulkan). GPU classes per target: modern x86-64 at T2 (`wgpu` over Vulkan or DX12), ancient x86 at T1 (`gl2` over OpenGL 2.1 drivers of the 2006 era, or T0 where there is no driver), Pi 4 at T1 by default and T2 where V3DV is present, Pi 2/3 at T1 through GLES 2.

### 3.3 Profiles

```
[profile.dev]      opt-level = 1, overflow-checks = true, debug-assertions = true
[profile.release]  opt-level = 3, lto = "fat", codegen-units = 1, panic = "abort", overflow-checks = false, strip = true
[profile.checked]  inherits = "release", overflow-checks = true, debug-assertions = true, panic = "unwind"
[profile.dev.package."*"] opt-level = 3
```

**Rule:** hashes and RNG use `wrapping_*` explicitly. Everything else is plain arithmetic, so `checked` turns a real overflow into a panic instead of a silent desync.

### 3.4 Lints

| Crate group | Lints |
| --- | --- |
| all | `unsafe_code = "forbid"` (the exceptions are §4's unsafe column; as built, one: `jane-render-gl2` is `deny` and its `src/gl.rs` alone allows it), `clippy::all`, pedantic where sane, `rust_2018_idioms`, `missing_debug_implementations` |
| `core`, `schema`, `data`, `world`, `sim`, `net`, `bot`, `art` | `clippy::float_arithmetic = "deny"`, `clippy::float_cmp = "deny"`, `clippy::disallowed_types` (HashMap, HashSet, RandomState, Instant, rand; Rc, RefCell, `Arc<Mutex>` in `sim` and `world`), `clippy::disallowed_methods` (f32/f64 conversions, `sort_unstable*`, `std::env::var`) |
| same eight | CI grep gate: `rg '\bf(32\|64)\b' crates/jane-{core,schema,data,world,sim,net,bot,art}/src` returns nothing, and so does `rg 'as f'` |

`schema` has one exception: the build-side reader of JSON numbers, `crates/jane-schema/src/compile/fraction.rs` (a `Num` type raw structs hold, read only through unit methods that emit `Tick`, `Permille`, `Fx`, `Angle` or `Q16`). It is the only file an f64 is named in, it runs on the host at build time, it carries a local `allow` with the reason, and the float gate skips it by path. `IndexMap`/`IndexSet` and `BTreeMap` are the allowed maps in the float-free crates; hash maps only through a `Lookup<K, V>` that cannot be iterated (`ARCHITECTURE.md` §0).

### 3.5 Dependency policy

**Rule:** a dependency is added by a commit that says why; "it saves a day" is not enough in the eight float-free crates.

| Crate | Where | Why |
| --- | --- | --- |
| `serde`, `serde_json` | `jane-schema`; `serde` alone also in `sim` and behind `jane-core`'s optional `serde` feature (only the sim turns it on) | The schema is serde structs; the save and the hash are one serde encoding of the state (`ARCHITECTURE.md` §3.5), whose core types derive it |
| `postcard` | `sim` (save), `net` (wire) | Compact, canonical, integer-friendly |
| `lz4_flex` | `sim` (save frame) | A busy county's save is ~150 KB compressed (`ARCHITECTURE.md` §3.5) |
| `xxhash-rust` (xxh3) | `sim` (state hash), `schema` (content hash) | One hash for saves, desync checks and the content pin (`ARCHITECTURE.md` §3.6) |
| `indexmap` | `core` re-exports | Ordered maps |
| `sdl2` | `app`; `jane-render-gl2` (and so `jane-cli` behind its off-by-default `gpu` feature) | Window, input, audio, present; the GL context for `gl2` (made in `jane-render-gl2`, so the entry-point loader never crosses a crate boundary, and a hidden window's for `jane sheet` and `jane bench`) and the window handle for `wgpu` |
| `glow` | `jane-render-gl2` only | OpenGL 2.1 / GLES 2.0 bindings, no C code, the same crate on the Pi and a 2006 PC (0.17, the version wgpu's GL backend already builds). Every call is an `unsafe fn`: see §4's unsafe column |
| `wgpu` | `jane-render-wgpu` only | Vulkan, DX12, Metal and GLES 3 behind one API; the only large dependency in the workspace, and it never touches the eight float-free crates |
| `raw-window-handle` | `app`, the two GPU backends | The window handle contract between SDL2 and the backends |

No `png` crate: `jane-art::sheet` carries its own encoder (uncompressed deflate, ~60 lines; `ART.md` §5), so sheets hash the same on every target and `jane-cli` stays free of C code. Out: tokio, async, rand, glam/nalgebra, bevy, image, crossbeam, anyhow in libs, log/tracing in sim.

### 3.6 CI

| Job | Runner | Does |
| --- | --- | --- |
| `check` | ubuntu | fmt, clippy `-D warnings`, float grep, `cargo check` on 1.85 for the eight float-free crates, `jane check` on `/data` |
| `build-psp` | ubuntu | `jane-core`, `jane-schema`, `jane-data`, `jane-world`, `jane-sim`, `jane-art` and `jane-present` built `--no-default-features` (`no_std` plus `alloc`) for `mipsel-sony-psp` with `-Zbuild-std=core,alloc,panic_abort` on a date-pinned nightly, so std cannot creep back (§13.9). The one nightly in CI; the workspace itself stays stable |
| `test-linux` | ubuntu | The fast tier, `cargo test --workspace` (SEEDS=64), on every push; the slow tier, `cargo test --release --workspace -- --ignored` (the crawls, the bands, the story, VERIFICATION.md §6), before a merge; `cargo test --profile checked -p jane-world -p jane-sim` (SEEDS=16) |
| `test-windows` | windows | tests for x86_64 msvc; release build i686 msvc |
| `build-i686-linux` | ubuntu | release build; `jane gen --hash` under `linux32` |
| `build-arm` | ubuntu | `cross build --release` aarch64 + armv7; `cross run ... gen --zones all --seeds 1..16 --hash` under qemu-user |
| `determinism` | needs above | diff `hashes-<target>.txt` and `bot-hash-<target>.txt` across every target; fail on any difference |
| `soak` | weekly | SEEDS=1000 skeleton, county 64, every dungeon 1000; seed sheets as artefacts |
| `bench` | self-hosted Pi 4, an ancient PC and a modern PC nightly, when online | `jane bench --json --tier` vs `tools/perf/thresholds.json` per class, and the late game (`cargo test --release -p jane-cli --test late_game -- --ignored`, `jane bench sim --gate`, §9.4). Until those runners exist the owner benches by hand before each gate and commits the JSON |
| `play` | ubuntu, every merge | `VERIFICATION.md` L0 to L2 in full, L3 on 8 seeds x 2 models; nightly L3 to L6 on 64 seeds x all models; weekly the soak (1000 seeds L1-L2, 256 seeds L3-L6). The sweep report diffs `EXPERIENCE.md`'s claims |

Artefacts on every `test-linux`: `sheets/county-<seed>.png` (8 seeds), dungeon sheets, art sheets (albedo, normal, emissive, height side by side), `jane film` clips of trace highlights and the per-seed dossiers from the `play` job. Never asserted, always attached.

## 4. Crate map

An arrow reads "is used by".

```
jane-core --> jane-data --> jane-world --> jane-sim --> jane-bot
   |             |                           |   |
   |             +--> jane-art               |   +--> jane-net
   |                     |                   v
   +---------------------+-----------> jane-present ----------> jane-app (SDL2)
                                          |  (Frame)              ^
                                          +--> jane-render-soft --+   T0: CPU, headless, fallback
                                          +--> jane-render-gl2  --+   T1: OpenGL 2.1 / GLES 2 (glow)
                                          +--> jane-render-wgpu --+   T2: Vulkan / DX12 / Metal (wgpu)

jane-schema (lib) <-- jane-data/build.rs (build-dep)
jane-cli <-- schema, world, sim, net, bot, art, present, render-soft     no SDL
   jane gen | check | bench | replay | sheet | film | play | sweep | dossier | audit | view | serve | soak | hash | save inspect
```

| Crate | One line | Depends on | Unsafe | Floats |
| --- | --- | --- | --- | --- |
| `jane-core` | Rng (sfc32 + splitmix seed), `dice()`, FNV-1a and `mix32`, the numeric types (`Fx(i32)` 1/256 px, `Milli`, `Permille`, `Angle(u16)`, `Tick`, `Q16`; `ARCHITECTURE.md` §2), `Grid<T>`, `Rect`, `Cell`, ids (`newtype u16`), `View` constants, `Blueprint`, the `Action` and `Condition` enums, one flood/BFS, one chamfer, one A*, one weighted pick, sort helpers with total keys | none | forbid | none |
| `jane-schema` | The schema: serde structs with `deny_unknown_fields`, table merge, interning, cross-ref and provider validation, room and chunk parsing and lint, look validation, codegen to Rust source, the content hash | core, serde, serde_json, xxhash-rust | forbid | none (build-side f64 only inside `deserialize_with`, §3.4) |
| `jane-data` | `build.rs` runs `jane-schema` over `/data`, emits `catalog.rs`, `names.rs`, `text.rs`, `tuning.rs`, `zones.rs`, `rooms.rs`, `chunks.rs`, `looks.rs` statics; typed accessors by id (`ARCHITECTURE.md` §6). *(Built 2026-09-26: one `catalog.rs`; the catalog's types are `jane-schema`'s `model`, which builds without serde, so data depends on schema's model and runs its `compile` feature from `build.rs`)* | core, schema (model); build-dep schema (compile) | forbid | none |
| `jane-world` | Skeleton, county, interiors, dungeons, placements, stories, names, solver, checks. `build_zone(zone, seed) -> Blueprint` | core, data | forbid | none |
| `jane-sim` | State tree, tick, actions, path, ai, combat, interact, inventory, quests, dialogue, triggers, clock, light, ring, save, replay, hash, `view::View` and `Event` (`ARCHITECTURE.md` §11) | core, data, world, postcard, lz4_flex, xxhash-rust | forbid | none |
| `jane-bot` | Headless player over `jane-sim`: walks the first five minutes and every dungeon, records the replay fixtures, drives the bot-session hash gate. What `test/bot.ts` was, as a crate the suite and `jane-cli` share | core, data, sim | forbid | none |
| `jane-net` | Lockstep: input frames, commands, join by snapshot, hash compare, desync reports, stall, timeouts, discovery. `std::net` TCP polled from the caller's loop (a guest builds the county on a thread). Host/guest `Session` over a `Link` trait (`ARCHITECTURE.md` §7) | core, data, sim, postcard, serde | forbid | none |
| `jane-art` | Procedural sprite, tile, font, icon, chrome and weather generators from `looks` rows, each emitting albedo, normal, emissive and height; palettes and ramps; atlas packing; contact sheets with its own PNG encoder (`ART.md`) | core, data | forbid | none (so sheets hash the same on every target) |
| `jane-present` | The scene: builds the backend-agnostic `Frame` (passes and draw lists: terrain, sprite batches, lights and shadow casters, fog volumes, parallax layers, particles, post settings, UI) from `jane_sim::view::View` and presenter state (PRESENTATION.md §1); fx runtime, camera, immediate-mode ui, input mapping, `text` (English expansion of `TextId`, `{name}`, `{place:}`); the `Backend` trait and the `Features` tier table. Reads views and events, never writes state; emits `Command`s only. No SDL, no GPU; headless in tests | core, data, sim (view, events), world (names), art | forbid | allowed, discouraged |
| `jane-audio` | *(Built 2026-09-27)* Every sound, made in code (`PRESENTATION.md` §5): FM, additive tables, Karplus-Strong strings, struck modes and filtered noise under click-free envelopes; `data/audio/sfx.json` patches rendered into buffers at boot; live ambient beds; the music sequencer on the game's tick over `data/audio/songs`; the mixer with one shared reverb and a limiter; WAV writing and the analysis the tests listen with. `build.rs` checks `data/audio` through the crate's own model (outside the content hash, like `bindings.json`). Owns no device | serde, serde_json | forbid | allowed |
| `jane-render-soft` | T0 backend: CPU rasteriser into a `u32` framebuffer, half or full res, the multiply lightmap, no normals or shadows. Always builds; `jane sheet`, `jane film` and CI draw through it | present | deny, one measured `allow` for the blit loop if the Pentium 4 demands it | allowed |
| `jane-render-gl2` | T1 backend: OpenGL 2.1 / GLES 2.0 through `glow`, GLSL 1.20 / ES 1.00; normal-mapped lights in a fragment shader, hard cast shadows by extruded occluder geometry, fog layers, particles, sharp-bilinear upscale | present, core, glow, sdl2 | deny: `glow`'s calls are all `unsafe fn`, so `src/gl.rs` alone carries `#![allow(unsafe_code)]` and wraps the calls T1 makes in safe ones that check every length handed to the driver; the crate copies the workspace's lint table with `unsafe_code = "deny"` (Cargo cannot override one inherited lint). Decided 2026-09-27: GL from Rust has no safe path (SDL's loader returns raw pointers, and every binding crate's calls are unsafe), and this is the exception the column always allowed | allowed |
| `jane-render-wgpu` | T2 backend: Vulkan / DX12 / Metal / GLES 3 through `wgpu`; everything T1 plus soft shadows, many lights, bloom, colour grading, water reflection, higher particle caps | present, wgpu, raw-window-handle | forbid | allowed |
| `jane-app` | SDL2 binary: window, input events, audio device (SDL2's callback at 48 kHz running `jane-audio`'s engine; silence when there is none), loop, threads, save files, config; probes the GPU, picks the backend and tier (override in `config.json`), and hands the `Frame` to it | everything | forbid | allowed |
| `jane-cli` | `jane gen`, `check`, `bench`, `replay verify|record|diff`, `sheet`, `view`, `serve`, `soak`, `hash`, `save inspect`, `world build`, `audio render|check|list`. `jane serve` is the headless lockstep host: no SDL, no sound, one status line (tick, seats, hash) | schema, world, sim, net, bot, art, present, audio | forbid | allowed |

**Rule:** `core` and `data` land first and change rarely; a change goes through integration before anyone builds on it.

**Rule:** `world` and `sim` share nothing but `core` types and the `Blueprint`. `world` never calls `sim`.

**Rule:** the eight float-free crates build for every target with no `cfg(target_*)`.

**Rule:** `jane_sim::view::View` is the contract between the engine and everything that draws. `jane-present` reads nothing else from the sim.

**Rule:** `Frame` is the contract between the scene and every backend. A backend draws a `Frame` and nothing else; a visual feature that one backend cannot draw is a `Features` row with a tier, never a `cfg` or a branch on the backend's name inside `jane-present`.

## 5. Data pipeline

### 5.1 Schema

One serde struct per table with `deny_unknown_fields`, required fields required, enums for every string union. That struct is the schema. Unknown key, typo in `do`, a sprite with no look = build error with file, row key, field. JSON floats (`ambient: 0.16`, chances) parse into `Permille` or `Q16` via `deserialize_with`; seconds into `Tick`, metres and px into `Fx`, degrees into `Angle` (`ARCHITECTURE.md` §6 has the unit of every field).

### 5.2 Merge and order

Base + fragments sorted by path, as now. Duplicate id = error. Row order is data (placements/stories contest spots); compiled arrays keep it and the build prints it.

### 5.3 Validation moved to build time

| Check | Now | Port |
| --- | --- | --- |
| Cross-file ids | partly catalog, partly solver, partly tests; `returnTo` untyped; `{place:}` unknown left verbatim | All resolved to `u16` at build; unknown = error; `returnTo` typed; flags: every read has a write and vice versa (a read with no write is an error: it was a warning until P1's triage, which found one, the scarecrow omen, now set by `data/omens.json`) |
| Names | none | Providers (`ARCHITECTURE.md` §5): every `NameId` a list, condition or quest uses must be declared by a zone contract in `zones.json`, a mission `bind`, a derived `zone_node_socket`, a placement row, an anchor or area id, a `.chunk` export or a story slot. Zone-scoped lists find the provider in that zone |
| Zone contract names | `CONTRACTS` mutated at import | Static per zone (`zones.json` rows + mission `binds` + placement promises), emitted as `static CONTRACT_<ZONE>`; every referenced name must be in some contract. Per-seed keeping is the runtime solver's job |
| Doors | `hasZone` gate, dead `icehouse` | Every `to.zone` is one of 13; dead row removed in P1 |
| Room templates | parsed/linted at runtime | Parsed, transformed, linted (rules 1-6) at build; harness proof stays a runtime test |
| Chunks | code in `chunks.ts` | `.chunk` files parsed and linted at build (§6.f); their exported marks and rects are name providers |
| Missions | `lintDef` runtime | Build: node order monotone, pools have templates with the sockets, `binds.from` exists, edges valid, states ≤ 3, fallback stamps |
| Names/stories | tests | Build: unique per pool, mechanical VOICE rules, story ids unique, place kinds exist |
| Looks | none | `data/looks/*.json`, keyed by sprite id and icon id (struct in `jane-schema`, meaning in `jane-art`; `ART.md` §5). Every sprite or icon a unit, prop, tile material, item, spell or status row names has a look, and every look is named by some row: either miss is a build error. `vary` lists are bounded at 4. `jane sheet` renders every look |

**Rule:** needs only data → build time. Needs a seed → a test. Nothing runs at game start.

### 5.4 Interning and ids

Every id string → `u16` newtype (`ItemId`, `PropDefId`, `UnitDefId`, `QuestId`, `NodeId`, `TriggerId`, `ZoneId`, `MarkName`, `RectName`, `KeyTag`, `Flag`, `SocketName`, `PoolId`, `TemplateId`, `PlacementKey`, `StoryId`, `TextId`, `SpriteId`). `static NAMES: [&str; N]` for console/debug/saves/errors. `ZoneId` is a closed enum of 13 whose order is `data/zones.json`'s order, which is the tick order. Runtime names (generated `zone_node_socket` keys) are `Sym(u32)`, pre-seeded so `Sym(n) == NameId(n)` below `NAMES.len()`; a save stores only the tail, as strings. Saves store `u16` ids plus the content hash: in release a differing hash refuses the load with both hashes shown unless a typed migration exists; in dev (`--allow-content-drift`) it remaps by name and drops gone names with a note (`ARCHITECTURE.md` §3.5).

### 5.5 Codegen

Plain Rust statics (not a blob): ~1.1 MB JSON → 2-3 MB source, a few hundred KB of `.rodata`; cached via `rerun-if-changed=../../data`. A feature `dev-data` loads `JANE_DATA_DIR` through the same `jane-schema` code at startup, and a test asserts the two paths build the same `Catalog`.

### 5.6 Dev loop

`cargo build` is the reload. `jane check` runs the schema in < 1 s with no codegen. `jane view --seeds 1..24 --out sheets/`. `jane sheet props`.

## 6. Worldgen port

### 6.a One seed scheme

```
dice(seed: u32, zone: ZoneId, step: Step, attempt: u8, a: i32, b: i32) -> Rng
  key = fnv1a(zone as u16, step as u16, attempt, a, b)   // little-endian bytes, no strings
  Rng::new(seed ^ mix32(key))                              // sfc32, splitmix seed, 12 warm-ups
```

`Step` is a closed enum in `jane-world/src/steps.rs` (SkelTerrain, SkelSite, SkelRoad, SkelRail, SkelAreas, SkelLamps, SkelPoi, SkelAnchor, CountyLand, CountyRoad, CountyPath, CountyChunk, CountyDoor, CountySmall, CountyPlaceRow, CountyStory, CountyNamePool, CountyHerbs, CountyRocks, CountyWild, DunChoose, DunEmbed, DunFill, DunLights, DunDress, ...). The six TS patterns become this one call. Rank-by-hash picks use `mix32(fnv1a(step, id, n))`.

**Rule:** every stage draws only from its own `dice()`; per-row/cell stages call `dice()` per row/cell.

**Rule:** a county stage that refines skeleton data (land, water, biomes, lamps) takes the skeleton's attempt; a stage that dresses takes the county's. Fixes `paintLand` (no attempt) and lamps (attempt 0).

The streams test carries.

### 6.b Integer noise and fields

`jane-core::noise`, in `Q16` (16.16, `ARCHITECTURE.md` §2): `lattice(ix, iy, salt) -> Q16` (`mix32(fnv1a(ix, iy, salt)) >> 16`), `smooth(t)` (i64 intermediates), `lerp`, `value(x, y, salt)`, `fbm(x, y, salt, octaves, gain_shift)` with weights as shifts. Terrain fields and county lattices become `Grid<Q16>`; thresholds (`.4`, `.5`) become `26214`, `32768` in `data/tuning/county.json`. One `core::chamfer` (3-4 weights, `u16` steps 10/14, integer cell units on the 4-cell grid). The county painter's bilinear sampling becomes per-row `Q16` gradients: the DDA it already is.

**Rule:** noise output is a pure function of `(x, y, salt)`.

### 6.c Arithmetic that moves

| TS | Port |
| --- | --- |
| `Math.hypot(dx,dy) <= r` (8) | `dx*dx + dy*dy <= r*r` in i64; `core::dist_sq` |
| `**` (4) | integer squares / `Q16` multiply with i64 intermediates |
| `Math.round` (48) | `core::div_round` (half away from zero) or `div_floor`; each site says which |
| `toFixed(1)` | integer formatting helper in `world::text` |
| `rngRange(72.5, 82.5)` | `rng.range(725, 825)` in tenths |
| f64 heap ties | `BinaryHeap<(Reverse<u32 cost>, Reverse<u32 seq>, Cell)>` |
| f32 stores + f64 math | `Q16` everywhere in fields; i64 inside one expression |
| road costs | `i32` in Q8 × tenths, heap key `(cost: i64, cell: u32)` (`ARCHITECTURE.md` §2) |

### 6.d Order dependence

| TS | Port |
| --- | --- |
| `Object.entries` weighted pick | `core::pick_weighted(&[(T, u32)], &mut Rng)` over an ordered slice from data |
| `Object.keys(BUILDERS)` | `ZoneId` enum order (`zones.json`) |
| `Object.keys(k.marks)` in cutThrough | `IndexMap` insertion order |
| `starts[0]` fallback | solver takes explicit `entrances: &[MarkName]` from the contract |
| Trigger iteration by merge order | sorted by `TriggerId` at build |
| Glob+sort path order | build-time order emitted into statics |
| 6 stability-dependent sorts | `sort_by_key` with total key ending in id; `sort_unstable` disallowed |

**Rule:** a comparator returns `Equal` only for the same element.

### 6.e One of each

`jane-core`: one `flood(grid, passable, starts, budget) -> Reach` (replaces ~9), one `chamfer`, one `astar` (budget + window), one hash pair. Hidden module state gone: `lastSkeleton` returned and passed; three footprint sources → one `Claims` grid on the `Kit`; `country` cached ctx → `CountryCtx` passed; name pools built inside the county build; `INFO`/`onFoot` WeakMaps → fields of `Built { blueprint, info }`; `catalogForValidation` → `jane-data` statics.

### 6.f Authored chunks as data

`data/chunks/*.chunk`, the `.room` format with a chunk header (`id`, `box`, `anchor`, `face`, `gates`, `slots`, `grid`, `legend` with tile/prop/mark/rect/fill entries). Stamper `world::chunks` (~200 lines) replaces 932. Procedural bits (town lot fill, yard tree) are `fill` legend entries backed by `data/tuning/chunks.json`; a `.chunk` is a pure grid plus its `fill` table and carries no other procedural ops. The marks and rects a chunk exports are name providers at build (§5.3), so a quest that names `house_door` is checked against the chunk that draws it.

**Rule:** a chunk grid never references a coordinate outside its box; gates and slots are the only things the county reads from it.

*(Data side built: the sixteen places of `chunks.ts` are `data/chunks/*.chunk`, a tile `grid` plus optional `things`, `names` and `claims` layers, one legend, and a header of `id`, `box`, `anchor`, `pin`, `face`, `gates`, `slots`, `around`; the grammar is `jane-schema/src/compile/tables/chunks/parse.rs` and `data/chunks/README.md`. `Catalog::chunks` holds them laid out and linted; the graveyard's coffins are the one fill. The station's `halt_approach`, the one rect that reaches past a box, is an `around` header line. The stamper is next.)*

### 6.g Tuning to data

`data/tuning/skeleton.json`, `county.json`, `country.json`, `dungeon.json`, `chunks.json` (and `sim.json` for the sim's tables, `ARCHITECTURE.md` §6): every constant the audit listed, integers or `Q16`. Builders take `&Tuning`; the static is the default; the streams test flips a value in memory. *So far:* `chunks.json`, `sim.json`, and `country.json`'s item lists (the herbs the county scatters, what its roadside chests hold and its orchards bear, by region: the catalog's `county.furnishing`, which an item's `replaceable` flag also counts). Structural constants (`CELL`, `TICK_RATE`, `RING_BLOCK`, `PATH_WINDOW`, `MAX_PLAYERS`) stay Rust `const`.

### 6.h Camera size in one place

`jane-core::view`: `VIEW_W_CELLS = 48`, `VIEW_H_CELLS = 27` from `FRAME_H_PX = 216`, `CELL_PX = 8`, `MIN_ASPECT = 16:9`. Half-screens, LOS box and the density test read these. A wider window shows more cells (§1), so every guarantee is a guarantee at 48 x 27 and holds when more is shown.

**Rule:** no literal 48, 27, 24, 13, 22 or 12 in `jane-world`.

### 6.i Render-only tiles

`RoofSlate`, `RoofThatch`, `BrickWall`, `Pine` leave the sim `Tile` enum. Variants come from (1) biome + cell hash read by the renderer from the skeleton the blueprint carries, (2) `Blueprint.paint: Vec<(Rect, Material)>` written by the chunk stamper. Kept with a reason: Glass, StoneWall, DeadTree, Hedge, Boardwalk, Stepping, Crops, FlowerBed.

### 6.j Retry and cost

Attempts stay (40/12/12). Added `GenStats { attempts, floods, cells_visited, ms }`, floored by tests: skeleton mean attempts ≤ 3, p99 ≤ 13 over 1000 seeds; county attempt > 0 on ≤ 2 % of 64; dungeon fallback 0 of 1000. Only then make C1/C6 re-solve incremental. A blueprint disk cache (`~/.cache/jane/<content_hash>/<seed>-<zone>.bp`) is deferred to P9 and written only if the Pi misses the New Game gate in §9.4.

**Rule:** measure before optimising the solver.

### 6.k The solver as a module

```
jane-world/src/solve/  model.rs flood.rs passes/{loot,gates,mechanisms,repairs,kills,triggers,hops,ifs,states}.rs run.rs ablate.rs report.rs
jane-world/src/dungeon/checks/  mod.rs c01_locks.rs c03_plain_keys.rs c04_sinks.rs c05_teacher.rs c06_lockin.rs c07_rest.rs c08_crit_len.rs c09_cycle.rs c10_seen.rs c11_plates.rs c12_trigger_solid.rs
```

Passes share `fn(&mut Solve) -> Changed`, each unit-tested. `BuildInfo` is a field. Stateful flood ≤ 3 states as layers.

### 6.l No `hasZone`

All 13 zones exist statically. `doors.json` always applies. `icehouse` row goes.

### 6.m Port order and acceptance per stage (invariants, no goldens)

| # | Stage | From | TS lines | Acceptance |
| --- | --- | --- | ---: | --- |
| 1 | core: rng, hash, numerics, noise, grid, flood, chamfer, astar, pick, sort keys, view | `sim/rng.ts` + scattered | ~400 | sfc32 known-answer vectors from TS (integers match); flood = reference BFS; chamfer within 1; noise range/continuity; `dice()` distinct per step and (a, b) |
| 2 | Skeleton terrain | `skeleton/terrain.ts` | 208 | River touches N and S; hill max in the Works; one lake east of river; region area bands; sheet |
| 3 | Sites, roads, early checks | `place.ts`, `roads.ts`, `index.ts` | ~700 | 1000 seeds: every sites row holds by road; 1-3 bridges; every road site reachable; attempt floors; speed |
| 4 | Rail, areas, lamps, POIs, anchors, threat, final checks | `rail.ts`, `anchors.ts`, rest | ~600 | Carry `skeleton.test.ts` whole |
| 5 | County land, roads, paths, rail raster, fingerposts | county stages 1-6, 8, 9 | ~500 | Road within 3 cells of every skeleton road cell; water under road = Boardwalk; rail 4-connected edge to edge; PNG |
| 6 | Chunks as data, link lanes, doors | `chunks.ts`, stages 7, 10, 11 | 932 → ~200 + 15 files | Every contract mark/rect exists; every gate connects; door far mark exists; flood from `start` reaches every gate |
| 7 | Furnishers, placements, small places, signposts | `country.ts`, `placements.ts`, `areas.ts` | 2609 | Carry `density`, `harshness`, `town`, `county` |
| 8 | Stories, names, boards | `stories.ts`, `names.ts` | 554 | Carry `stories`, `tales`, `truth`, `locked-words`, `quest-audit` (seeded parts) |
| 9 | Scatter, life, cutThrough, dropUnreachable, done | stages 19-22 | ~250 | Carry `county` reachability, first walk lit, same seed same county, streams |
| 10 | Interiors | `interiors.ts` | 260 | Solver passes house, cellar, arms, church on 64 seeds |
| 11 | Templates compile, lint, transforms | `room.ts`, `pools.ts` | 640 | Build-time lint on 109; carry `templates` |
| 12 | Dungeon layout | `layout.ts` | 494 | 64 seeds/dungeon embed without fallback; corridors never cross; rooms in bays |
| 13 | Lock, lights, fill, name, emit | `generate.ts`, `lights.ts` | 764 | Names stable across attempts; light rules; heat within cap |
| 14 | Solver | `validate.ts` | 677 | Every zone every seed (64 suite, 1000 soak); sealed gate rejected; per-pass unit tests |
| 15 | Checks C1-C12 | `checks.ts` | 700 | Each check rejects its bad case; all pass 64 seeds/dungeon; designed order |
| 16 | Harness | `harness.ts` | 85 | Every template proves grants/blocks alone from every door in every transform |
| 17 | Bot plays through | needs sim and bot (P4) | | Per-dungeon bot tests; the five-minute first walk |

Stages 1-9 and 10-16 are two independent worktree tracks once `core` exists.

## 7. Phases and gates

| Phase | What | Crates | Parallel units | Gate | Size |
| --- | --- | --- | --- | --- | ---: |
| **P0** Docs, archive note, workspace, CI | The docs; POSTMORTEM; empty crates; toolchain/config/Cross/lints; CI green on empty crates; `/data` moved with a path alias so the TS still runs | all (empty) | docs 4; workspace + CI 1 | `cargo build` all six targets in CI; docs merged | ~0 |
| **P1** Core, schema, data | `jane-core` with tests; `jane-schema` for all tables + rooms + missions + chunks + tuning + looks + `zones.json` + `bindings.json`; `jane-data` build.rs; `jane check`. Surfaces data bugs | core, schema, data, cli | core 1; schema 1 per table group (5); cli 1 | Every JSON compiles with zero unknown fields; every cross-ref and provider resolves; `jane check` < 1 s; core unit tests; `compiled_equals_dev_data` | core ~1.5k, schema ~2.5k |
| **P2** Skeleton, county, viewer | §6.m 2-9; `jane view` PNGs; `jane bench` for gen | world, cli | skeleton 1-2; county paint 1; chunks 1; furnishers 2; stories 1; scatter+cut 1 | Skeleton tests 1000 seeds; county 64; density/harshness floors; streams; owner looks at 24 sheets; gen ms recorded | ~6.5k → ~6k + data |
| **P3** Dungeons, solver, templates | §6.m 10-16; dungeon sheets; `jane gen --zones all --hash` | world, cli | templates 1; layout 1; lock+lights+fill 1; solver 1-2; checks 2; harness 1; interiors 1 | Every zone every seed through solver; C1-C12 64 seeds; fallback 0 of 1000; each check rejects; template proofs; `hashes-x86_64.txt == hashes-aarch64.txt` (first cross-target gate) | ~3.5k → ~4k |
| **P4** Sim and bot | `ARCHITECTURE.md`'s sim including the journal (§3.7), aim assist (§5.4), schedules, weather, ecology, consequences and rumour (§4.6); `jane-bot` with the first two player models (Reader, Rusher); `jane replay verify`; `Sim::metrics()` | sim, bot, cli | ~8 per sim module; bot 1; living-world rows 1 | Carry `sim`, `engine`, `replay`, `coop`, `verbs2`, `rest`, `dungeon-verbs`, `budget`; bot plays the first five minutes and every dungeon on 3 seeds; save/load hash-equal; replay hash-equal; `bot-hash-<target>.txt` equal across targets (`ARCHITECTURE.md` §8 `cross_target_hash`); assist replays exactly; tick µs recorded | ~7k → ~9k |
| **P4b** Verification stack | `VERIFICATION.md` L3 to L7: all player models, the `Trace`, experience metrics and the first bands, truth and cohesion audits over the journal, living-world audits, dossiers; `jane play`, `sweep`, `dossier`, `audit`; the `play` CI job; `EXPERIENCE.md`'s first hour tagged and passing | bot, sim, cli | models 2; metrics 1; audits 2; dossier 1 | The first-hour claims in `EXPERIENCE.md` pass on 64 seeds x all models; the sweep report is a CI artefact; a deliberately broken quest text fails L4 legibility by name | new ~4k |
| **P5** Art | `ART.md` generators, looks, font, chrome, weather; four layers per sprite; six-frame walks, four-frame creature gaits; `jane sheet` incl. `layers` and `light`; `vary` variants | art, cli | 4-6 per family; layers 1; weather 1 | Every look renders with all four layers; sheets on CI; owner redlines; no sprite without a look and no look unused; ≤ 4 variants per `vary`; every look built into atlases inside its §9.4 row (`ART.md` §5) | new ~6-8k |
| **P6** Scene and the soft backend, playable solo | `PRESENTATION.md` §1 in `jane-present` (the `Frame`, camera, fx, `Features`) and `jane-render-soft`; `jane-app`; `jane film`; New Game builds all 13 zones behind a loading screen that draws the skeleton; the P7 slice of §7.1 (keyboard move, use and bar; the prompt, the vitals, the dialogue box) | present, render-soft, app | scene 2; soft 2; app 1; slice 1 | The owner plays the first hour at T0; frame ms recorded against §9.4; `jane film` produces the L7 clips; lints hold | ~3.5k → ~4k; soft ~2k; app ~1k |
| **P6b** T1: `gl2`, lights, shadows, atmosphere | `jane-render-gl2`; normal-mapped lighting, hard shadows, fog volumes, parallax, weather, sharp-bilinear upscale; the `Features` ladder; a Pi 4 and an ancient PC on the desk | render-gl2, present | shaders 1; shadows 1; atmosphere 1; Pi bring-up 1 | 60 fps on a Pi 4 and on the ancient PC at 768 x 432, night, town, per §9.4; the same `Frame` draws on T0 and T1 with only `Features`-row differences (a pixel diff of the albedo pass is empty) | new ~3k |
| **P6c** T2: `wgpu` and post | `jane-render-wgpu`; soft shadows, many lights, bloom, grading, water reflection | render-wgpu | 2 | 60 fps at 4K output on a modern PC with headroom; T1 and T2 agree on everything but the T2 rows; owner signs the look off against `ART.md` §3 on the same 24 seeds as the sheets | new ~2.5k |
| **P7** UI and input | `PRESENTATION.md` §3-4 in `jane-present::ui` and `::input`; `bindings.json`; keyboard, mouse, pad; Controls screen; the assist profile flag | present (ui, input), app | 3-4 | Every README console row works; owner plays with a pad and with a mouse, assist on and off; UI has no sim writes except `Command`s | ~3.5k → ~3.5k |
| **P8** Lockstep LAN | `jane-net`; host/join screens; `jane serve` | net, present (ui), cli | protocol+host 1; guest+join 1; UI 1 | Two machines, four seats, an hour without desync; hash every 60 frames; a Pi hosting headless through `jane serve`; stall and rejoin over loopback | new ~1.5k *(built 2026-09-27 ahead of P7: ~2.4k plus tests; §7.2)* |
| **P9** Targets and perf | Pi 4 aarch64 and armv7 on hardware (Pi 3 recorded); i686 Linux/Windows on an ancient PC; Win 7; XP if it builds; thresholds enforced per class and tier | app, backends, cli | 1 per target; 1 profiling per backend | §9.4 thresholds green in all three classes at their tier; 60 fps by the `Features` ladder, 30 fps is not a pass; RSS under ceiling | fixes |
| **P10** Archive | `jane/` → `archive/web-remake-2026/`; banners; SYSTEMS.md rows re-marked | docs | 1 | `git grep jane/src` in root docs finds only archive notes | 0 |

**Rule:** a phase is not entered until the previous gate is green on CI. Two tracks at once only where the table says (P2 and P3 after P1; P4b beside P5; P5 beside P3/P4; P6 beside P5, window first, §7.1; P6b and P6c after P6, in either order).

**Rule:** P4 does not start content. The bot tests are the content until P6; from P4b the `EXPERIENCE.md` claims are the content's specification and grow with every phase.

**Rule:** a degrade flag (a `Features` row: shadows, soft shadows, fog volumes, bilinear light, post, half-res) is a documented tuning row with its tier, never a hidden `cfg`. The gate in P9 says which rows were on in each class.

### 7.1 Window first (decided 2026-09-27)

P6 enters beside P5, not after it. On 2026-09-27 P5 stands at `ART.md` §8 step 1 (palette, canvas, font, chrome, the lit sphere); waiting for steps 2 and 3 would keep the game headless for a phase. So the window comes first and the art replaces what stands in for it as each step lands.

| Step | Work | Seen by |
| --- | --- | --- |
| 1 | `jane-present`: `Frame` and `Pass`, the camera with tick interpolation, the counting-sort draw list, `tick()` and `draw(alpha)`, fed by `View` | headless tests |
| 2 | `jane-render-soft`: framebuffer, CLUT blit, chunks as flat swatches, nearest upscale, `read_back`; `jane sheet scene` | a PNG of a real frame |
| 3 | `jane-app`: SDL2 window, the fixed 60-tick loop, keyboard to `InputFrame`, the `soft` backend | the owner walks the county |
| 4 | The P7 slice: keyboard move, use and bar slots; the prompt, the vitals, the dialogue box. **Done 2026-09-27, and most of the rest of P7 with it** (`PRESENTATION.md` §3 as built: title, loading, HUD, window, menus, terminal, Controls, F2 and F3; P7's gate, the owner playing with a pad and a mouse, is still to come) | quests are played |
| 5 | The T0 lightmap and the chunk painter | night is night |
| beside 1 to 5 | `ART.md` §8 steps 2 and 3 (person, terrain), then on in order | the placeholders go |
| 6 | The art-director pass over whole frames (town at dusk, a field edge, the lakeshore, a wood): palette, outline and shading tuned across every family at once (`ART.md` §3.1) | frames that hold up beside the references |
| 7 | P6c before P6b (decided 2026-09-27): `wgpu` at T2 first, on the owner's desk, for the normal-mapped light and the soft shadows the look is built for; `gl2` after | the Elysian Shadows half of the look |

**Stand-ins while art is missing.** A chunk with no painter draws its region's flat ground swatch (`PRESENTATION.md` §1.6 already allows it for the frames a chunk takes to paint). A unit or prop with no look draws the nearest step-1 demo sprite of its size. Both are drawn by `jane-present`, never by `jane-art`, and both go when the step that replaces them lands; `ART.md`'s rule that a row without a look is a build error applies from the P5 gate, not before.

**SDL2 on a Windows or Linux x86 desk** is the `sdl2` crate's `bundled` and `static-link` features (§12), which build SDL from source and need CMake on the path; the Pi keeps the distro package.

### 7.2 P8 as built (2026-09-27)

Built beside P7 (`ARCHITECTURE.md` §7 has the protocol and the hooks). Decided that day (owner): LAN only, player-hosted first (`jane-app --host` plays seat 0, others `--join`), `jane serve` secondary, the transport a trait.

| Gate item | State |
| --- | --- |
| Hash every 60 frames | Done: every peer, with its last three saves kept for a desync's report (the hash point, the first step that differs when both re-simulate, the parts that differ) |
| Stall and rejoin over loopback | Done, in tests with a driven clock: shown at 500 ms, dropped at 10 s, the wait toggle; a dropped guest back in her own seat with her bags, caught up to the host's hash |
| Four seats, an hour without desync | On one machine, over TCP: `jane serve` (release) and four `jane join` processes, two Readers and two Rushers, 216 000 frames (an hour at 60 Hz, ~1 000 frames lost to stalls while the machine also built and tested): 14 398 hash checks agreed, 0 differed, no seat dropped, all five ending on one hash. Two machines not yet run |
| A Pi hosting headless through `jane serve` | `jane serve` built and soaked on x86_64; not yet run on a Pi |
| Measured | In one process (`jane-net/tests/loopback.rs`): four seats, 18 900 frames, 927 hash checks agreed. Two `jane-app` windows on one machine, host and guest: both at frame 1200 on one hash, 16 checks agreed, each drawing both players in their own coats. `jane serve` with two bot guests over localhost for five minutes: 18 000 frames, 598 checks agreed, 0 differed |

Not built: internet play, NAT traversal and relays (LAN only, by decision); host migration; a session recorded from a loaded save (only New Game sessions are tapes); saving on `SIGTERM` (std has no signal handling; `jane serve` saves on every rest and at `--ticks`). The Host and Join screens, Open to LAN, the table plate, the stall banner and the terminal's `join` and `leave` were built on the UI unit's widgets after it merged (`PRESENTATION.md` §3.2); a scripted two-window run through the title's Host and Join held 25 hash checks, 0 differed.

## 8. Agent parallelisation

| Rule | |
| --- | --- |
| One crate or module per agent | One directory under `crates/` plus its own tests; two agents never own the same file |
| Shared crates land first | `core` and `data` merged and tagged before P2 branches; changes to them are small same-day branches; live agents rebase |
| Interface first | The pseudo-signatures in the docs are the contract; a missing signature is proposed in the doc first |
| Every agent adds tests | No tests, no merge; name the carried TS test in the commit |
| Short-lived branches | 1-2 days; integration branch `integration`; `main` takes it at each gate |
| Integrate daily | Owner or an integration agent merges daily, runs the suite, fixes conflicts there; commit style stays |
| Merge order | Leaf first; within world: core helpers, skeleton, county paint, chunks, furnishers, stories, scatter; templates, layout, generate, solver, checks, harness |
| No cross-branch deps | A's type lands in integration first, or B stubs behind a trait |
| Formatting is not a diff | `cargo fmt` pre-commit hook; `rustfmt.toml` committed |
| Determinism is everyone's | `cargo test --profile checked` on the crate before merge |

## 9. Test strategy

### 9.1 Layers

Unit (`#[cfg(test)]`), rule tests (`tests/*.rs`, one per carried TS file, naming its source), seeded sweeps (`SEEDS` env: 64 default, 256 CI for skeleton and mine, 1000 weekly soak), bot (`crates/jane-bot/tests/`), determinism (`crates/jane-sim/tests/determinism.rs` + CI cross-target), perf (`jane bench` JSON vs thresholds), sheets (artefacts, never asserted). These are `VERIFICATION.md`'s layers L0 to L2 plus the determinism gates; L3 to L7 (player models, experience metrics, truth and cohesion, the living world, dossiers and film) are that document's and land in P4b.

### 9.2 What carries, by TS file

skeleton → `world/tests/skeleton.rs`; county → `county.rs`; world → `solver.rs`; dungeon-gen + per-dungeon + dungeons + dungeon-dressing → `dungeon_<id>.rs` + `dungeon_gen.rs` (bot parts → `bot/tests/play_<id>.rs`); templates → `templates.rs` (lint moves to build); streams → `streams.rs`; stories/tales/truth/locked-words/town/quest-audit → `content_*.rs` + schema unit tests (mechanical word rules to build time); density/harshness → floors; sim/engine/replay/coop/verbs2/rest/dungeon-verbs/budget/quests → `sim/tests/*.rs` (`budget` becomes bench threshold + smoke); map → `present/tests/map.rs`; storage/ui/art/save migrations → dropped, replaced by save round trip, UI command tests, `jane sheet` and `ART.md`'s audit tests.

### 9.3 Determinism gates

1. `hash(build_zone(z, s))` all 13 zones, seeds 1..64, twice in one process, equal. 2. Same in a fresh process vs a file. 3. Same on every CI target, byte-equal. 4. Bot session five minutes, hash every 600 ticks, replay equal, every target equal. 5. Save at T, load, tick to T + 6000, equals uninterrupted. 6. All under `--profile checked`. `ARCHITECTURE.md` §8 names each test.

### 9.4 Perf targets (release, single thread for gen/sim)

Three machine classes, each a gate at its tier. Ancient PC means an SSE2 CPU of the Pentium 4 era with a 2006-class GPU on an OpenGL 2.1 driver (GeForce 7, Radeon X1000, Intel GMA 950 where the driver allows).

| Metric | Modern PC (T2) | Ancient PC (T1) | Pi 4 (T1) |
| --- | ---: | ---: | ---: |
| Skeleton (one valid attempt) | < 10 ms | < 80 ms | < 60 ms |
| County build + solver | < 300 ms | < 2.5 s | < 2 s |
| One dungeon build + solver + C1-C12 | < 15 ms | < 120 ms | < 100 ms |
| New Game, all 13 zones, behind the loading screen | < 500 ms | < 4 s | < 3.5 s |
| Art into atlases at boot (four layers) | < 150 ms | < 1 s | < 800 ms |
| Median tick, county, TS budget test's sleepers | < 100 µs | < 600 µs | < 400 µs |
| p99 tick | < 500 µs | < 2 ms | < 1.5 ms |
| Frame, 768 x 432, town, night, 8 shadow lights | < 2 ms at 4K output | < 12 ms, shadows off | < 12 ms, hard shadows on |
| RSS in play | < 256 MB | < 128 MB | < 192 MB |
| Binary (app with its backends) | < 24 MB | < 12 MB | < 16 MB |

Measured on the modern desk, 27 September 2026 (`jane play --profile`, a Reader's story run on seeds 1 and 7, two game hours, release): the tick's median 1.3 to 2 µs and p99 30 to 38 µs, units awake about 10 of 400 to 2 300; the slowest step under 0.4 ms (a controller's search). Before the county's runtime was kept when nobody is in it, a clock row run while she was indoors built it for one step and dropped it again, 29 ms, and walking back out of any door built it again, 41 ms: stalls a frame cannot hide (`Sim::drop_empty`). The bot's own think is 4 to 5 µs a frame.

Recorded, not gated: T0 `soft` on the Pentium 4 (60 fps at 384 x 216 or 30 at 768 x 432, the multiply lightmap only) and a Pi 3 at T1 with shadows off. i686 takes the ancient PC column; armv7 takes the Pi 4 column with 20 % slack. Enforced from P9; recorded before.

**The tick in the late game** *(recorded 2026-09-27, modern PC, release)*. The two tick rows are held over a long session as well as a short one: `jane bench sim` plays a model from New Game (the Reader, 120 minutes by default) and times ten-minute buckets of play, the step, the bot's thinking and the tape's hash apart, beside units, props, the journal, path searches and nodes, the bot's plans, events and live runtimes. `tools/perf/thresholds.json`'s `sim` gates the last quarter of the frames: the median tick (100 µs) and the p99 tick (500 µs) above, and a whole frame of 200 µs (5,000 frames/s, step, bot and hash) so that a player model that slows down late fails too. `crates/jane-cli/tests/late_game.rs` runs it on seed 7 (`--gate`); it is ignored, run in release by hand before a gate and by the `bench` job once its runners exist.

| Reader, 120 min | frames/s, whole run | frames/s, last 30 min | step mean / p99, last 30 min (µs) | step p99, worst 10 min (µs) | bot, last 30 min (µs a frame) |
| --- | ---: | ---: | ---: | ---: | ---: |
| seed 7, before | 5,274 | 2,233 | 10 / 72 | 72 | 436 |
| seed 7, after | 19,386 | 13,477 | 4 / 35 | 35 | 69 |
| seed 1, before / after | 68,412 / 92,598 | 61,597 / 87,794 | 7 / 128, 4 / 118 | 128 / 118 | 7 / 5 |
| seed 2, before / after | 67,759 / 92,440 | 69,300 / 102,837 | 7 / 124, 5 / 120 | 124 / 121 | 5 / 3 |

The late-game slowdown was the Reader's, not the sim's: with nothing left it could reach (seed 7 from minute 80), it chose again every frame, and each choice walked the county's 11,000 props asking what each was and how far it lay, a square root apiece: 430 µs a frame, 97 % of it. The sim's own growth was plates walking the zone's 3,700 units once a plate every sixth tick, half the county's step. Both now give the same answers for less (the same hashes, every session above and the bot-session fixture). What does not grow: units, props, events, the journal (143 entries at two hours), fog. What remains, by size: the bot's own paths (a 320-cell window and 120,000 nodes every 120 frames, the Reader's road cost weakening the heuristic: 30 to 50 µs a frame while walking, 10 to 25 ms at worst; the A* runs at 65 to 70 ns a node, and neither a four-ary heap nor an open list bucketed by `f` made it faster); the tape's hash every 600 frames (0.5 to 1 ms, growing with the zones made); in a dungeon on this branch, the county's runtime built for each clock row and dropped again (5 to 7 ms, eight times a game day; the story branch keeps the county's runtime, which ends it). The story branch, which plays further, spends its worst ticks on the dog's orders: a search of the whole 6,000-node budget every 20 ticks toward a mark it cannot reach (400 µs each, the p99 near 500 µs); routed to it, with its museum tactic (20 to 50 µs a frame of the bot's).

**Rule:** 60 fps holds in every class at its tier, at night in the town. If the frame row fails, the `Features` ladder (`PRESENTATION.md` §1.12) is walked in order: soft shadows to hard; then post off; then fog volumes to one layer; then shadows off; then bilinear light to flat steps; then half-res. 30 fps is never a pass. The P9 gate records which rows were on in each class.

## 10. Docs written in this pass (order)

1 `PORT.md` (this). 2 `ARCHITECTURE.md`. 3 `ART.md`. 4 `PRESENTATION.md`. 5 `README.md` (what it is; run it: `cargo run --release -p jane-app`, `cargo test`, `jane check`, `jane view`; targets; the first hour; controls; this repo table; where it stands; the seed viewer as `jane view`). 6 Banners (ENGINE, PLATFORM, WORLDGEN, DUNGEONS, SYSTEMS, PLAN with §2.2 fixes). 7 `archive/web-remake-2026/POSTMORTEM.md`. 8 `VERIFICATION.md` (the layers L0 to L7, the `Trace`, bands, the CI shape). 9 `WORLD.md` (time, people, ecology, weather and atmosphere, consequence, rumour, cohesion, the story spine). 10 `EXPERIENCE.md` (the format, the first hour in full with check tags, the later hours as headers, the long-running context).

**Rule:** `EXPERIENCE.md` grows with every content phase and is never behind the content: a place, quest or person that is not in it is not shipped.

**Rule:** no doc describes code that does not exist as if it did.

## 11. Risks

| Risk | Mitigation |
| --- | --- |
| Agent Rust grows `Rc<RefCell>` or clones per tick | `ARCHITECTURE.md` §3 fixes the arena-and-id design; `disallowed_types` for Rc, RefCell, `Arc<Mutex>` in sim and world |
| Integer worldgen looks worse | `jane view` sheets in P2 before furnishers; noise and thresholds are tuning data |
| Pi perf short | Measure in P2 on hardware; painter profiled first; loading screen shows the skeleton; the §9.4 degrade order before any redesign |
| Data compile surfaces hundreds of errors in P1 | Its purpose; a default look per family in one commit, refined in P5 |
| Scope creep | Gates; a system needs a row here first |
| Cross-target hash differences (`usize`, endianness, `as`) | No `usize` in state or blueprints; hash over explicit LE bytes; `checked` profile; CI diff from P3 |
| Old Windows toolchains vs `sdl2` | The eight float-free crates and `jane-cli` have no SDL; the XP app is a stretch goal, never a gate |
| Suites take minutes | `SEEDS`; soak weekly; `cargo nextest` allowed |
| P2 and P3 both change `Blueprint` | `Blueprint` in `core`, frozen at P1; a change is a `core` branch first |
| Worktrees diverge | §8 |
| Three backends triple the presentation work and drift apart | One `Frame` contract and one `Features` table; `soft` is built first and is the reference every other backend is pixel-diffed against on the albedo pass; a feature exists in `jane-present` once and in each backend as a shader or a loop, never as logic |
| 2006-era GL drivers lie about their limits | The `gl2` backend uses GLSL 1.20 / ES 1.00 with no extensions beyond `OES_standard_derivatives`; every shader is compiled in CI against Mesa's software `llvmpipe` at GL 2.1; caps are probed and any failure drops to T0, which always works |
| The verification stack costs more than the game | Layers run at different cadences (§3.6 `play`); bands start wide and tighten against the owner's own traces; a metric that never fails in a month is demoted to nightly |
| Console ports (PSP, Xbox, Dreamcast) force a memory diet late | §13: the sim budget is Dreamcast's 6 MB and held from now, hash-proven; measure the real peak first; Rust-on-SH4 spike before Dreamcast is scheduled |
| The look outruns the hardware floor | Every visual feature has a tier row from the day it is designed (`PRESENTATION.md` §1); the ancient PC and the Pi 4 are on the desk from P6b, not P9 |

## 12. Defaults taken

Each is a one-line edit to flip before P0 starts.

| Default | |
| --- | --- |
| SDL2 on the Pi | Distro package; x86 targets use the `sdl2` crate's `bundled` and `static-link` features (CMake on the build machine, no DLL beside the exe) |
| `armv7` | Cortex-A7 (Pi 2, and Pi 3/4 on a 32-bit OS); armv6 (Pi 1, Zero) out |
| Legacy Windows | Windows 7 before XP; XP is never a gate |
| `.chunk` files | Pure grids plus a `fill` table; no procedural ops |
| `jane serve` output | One status line: tick, seats, hash |
| Repository name | No rename; the GitHub repo stays Junqi |
| Benching | By hand before each gate until self-hosted Pi 4, ancient PC and modern PC runners exist |
| Compile-time units | `stop` stays ticks; `heal` splits into `heal` and `heal_pct` at the schema |
| Content pin | Refuse in release, remap by name in dev |
| RNG | Per-zone stream for combat, loot and fans; one reserved world stream |
| `Sym` tails in saves | Strings, not hashes |
| Stall | A seat is dropped after 10 s; the host has a "wait" toggle |
| Blueprint disk cache | Deferred to P9; written only if the Pi misses the New Game row in §9.4 |
| A flag read but never set | A build error (a warning until P1's triage; the omens now set theirs, `data/omens.json`) |
| Canvas | 768 x 432 internal, 16 screen px per cell, people 32 x 40; the sim cell stays 8 units = 1 m (`ART.md` §1) |
| Backends and tiers | `soft` T0 always; `gl2` T1 through `glow` at GLSL 1.20 / ES 1.00; `wgpu` T2; probed at boot, overridable in `config.json`; `soft` is the pixel-diff reference for the albedo pass |
| Sprite layers | Albedo, normal, emissive, height from every generator; the `soft` backend ignores normal and height |
| Light | Normal-mapped point, spot and directional (sun and moon by the clock); hard shadows on T1, soft on T2; `soft` keeps the multiply lightmap |
| Atmosphere | Fog and mist volumes per skeleton area from `data/atmosphere.json`, weather from `data/weather.json`; god rays and reflections T2 only |
| UI grid | The 768 x 432 canvas; Small face 12 x 18, a dense 8 x 12 face for lists |
| Aim assist | In the sim per profile; Pad cone 20°, snap 4°, magnet 350‰, sticky 30 ticks; Mouse cone 8°, snap 2°, magnet 200‰; Off = raw (`ARCHITECTURE.md` §5.4) |
| Journal | Append-only, ring of 512 per kind plus the `known` map; ids only, never text (`ARCHITECTURE.md` §3.7) |
| Weather | Rolled hourly per region from the world stream; no storm on the first walk (`WORLD.md` §5) |
| Player models | Reader, Explorer, Rusher, Cautious, Co-op pair, Lost (`VERIFICATION.md` §2 L3); the Reader and the Rusher first |
| Atlas cache on disk | Allowed: written under the save dir, keyed by build hash, never shipped |
| Procedural audio | *(Built 2026-09-27)* `jane-audio`, all synthesised, no file and no sample table over 32 entries; the cue table in `jane-present::audio`; `NullBus` for `jane serve` and the tests (`PRESENTATION.md` §5) |
| Font | Stroke-defined glyphs on a 5 x 8 lattice, rasterised at boot at two sizes; title and heading faces from the same strokes (`ART.md` §6) |
| Seats | Coat-only swaps by role (`ART.md` §3) |
| Pi 3 and Pentium 4 | Recorded at T0 / T1 with shadows off, never a gate |

## 13. Console targets and memory tiers (planned 2026-10-08)

**Status: a plan, not code.** Nothing here is built. It sets budgets now so the sim and the bake are shaped for small machines before they set hard. It amends §1 *Targets*, *Rendering* and *Hardware floor* for consoles only; PC and Pi rows stand unchanged.

**Order:** PSP first (shipping target), then Xbox (original, 64 MB), then Dreamcast. Dreamcast sets the **sim** budget (smallest RAM, slowest CPU); PSP sets the **art** budget (smallest VRAM, smallest screen). Both are held from now, whichever ships first.

### 13.1 The machines

| | CPU | RAM | VRAM | Sound RAM | Screen | Toolchain |
| --- | --- | --- | --- | --- | --- | --- |
| PSP | 333 MHz MIPS, single-precision FPU | 32 MB (about 24 usable on a PSP-1000) | 2 MB (the framebuffers' too) | n/a | 480 x 272 | `rust-psp`, `mipsel-sony-psp`, nightly and `build-std`; PPSSPP to test |
| Xbox (original) | 733 MHz x86 | 64 MB shared | shared | n/a | 640 x 480 and up | `nxdk` (C); Rust experimental |
| Dreamcast | 200 MHz SH-4 | 16 MB | 8 MB | 2 MB | 640 x 480 | KallistiOS (C); no official Rust target |

**Toolchain note:** §1 says stable Rust, std only, no nightly. That holds for every PC and Pi target. The PSP build needs nightly and `build-std`; it is the one exception and lives in its own `.cargo/config.toml` target block, never the workspace default. The sim crates stay `no_std` plus `alloc` capable so Dreamcast and Xbox can reach them through a C-callable library if Rust does not reach those targets.

### 13.2 Budgets (held from now, on every target including PC)

| Budget | Number | Source | Check |
| --- | ---: | --- | --- |
| Sim resident heap, console form (county, units, scratch, journal, events) | **7 MB** (PSP-1000); **6 MB** the Dreamcast goal | PSP-1000, Dreamcast | `tests/heap_budget.rs` asserts 7 MB (`packed_sim_new_game`) |
| World build peak, console form (New Game, behind the loading screen) | **9 MB** | PSP-1000 | `tests/heap_budget.rs` asserts it (`packed_build_peak`) |
| Tick, busy county, four seats | **under 8 ms on a 200 MHz class CPU** (extrapolated from the Pi 3 numbers in ARCHITECTURE §9) | Dreamcast | bench ratio against the Pi 3 row |
| Resident art | **3 MB RAM plus the 2 MB VRAM** at the PSP tier (the VRAM holds the two framebuffers, 1.1 MB at `8888`, and a page cache in the rest; corrected 2026-10-08, the PSP has 2 MB of VRAM, not 4) | PSP | atlas size test per tier |
| Music | Tracker patterns plus one shared sample bank **under 1.5 MB**; on the PSP-1000 **0.81 MB held** (2026-10-08, §13.4: the play heap has 1.4 to 2.4 MB free) | Dreamcast sound RAM | `tests/tracker.rs` holds what the mixer holds under 0.9 MB |
| Allocations per tick after warm-up | **0** (already a rule, ARCHITECTURE §9) | all | counting allocator |

Measured (2026-10-08, §13.3 has the tables and the instrument): the county build peaks at 15.3 MB after diet phase 1 (51.8 before). After phase 2 the sim at New Game is 16.2 MB on PC (21.0 before) and **11.5 MB in the console form** (blueprints packed, 5.3 MB of it); on PPSSPP the replay peaks at 10.5 MB (19.6 before). **After phase 3 the console form builds at 8.3 MB peak and its sim is 6.7 MB at New Game** (host, requested bytes); on PPSSPP as a PSP-1000, build peak 8.7 MB, replay peak 6.7 MB. **With zones built on demand (§13.3, phase 4) the console sim is 6.2 MB at New Game**; a zone's entry peaks at 8.3 MB (under the build's 9). The art figures (ART §5: the full four-layer atlas about 64 MB, units 45 MB) are still unmeasured.

**The PSP-1000's memory** (2026-10-08, phase 3; PPSSPP headless with `PPSSPP_PSP1000=1`, PSP-1000 model, `spikes/psp-sim`). The user partition is 24 MB; with the program loaded, the largest free block PPSSPP reports is **20.4 MB** (`sceKernelMaxFreeMemSize`). The build and the sim are never resident together: New Game builds behind the loading screen, then the sim runs over what was built (the build's peak includes the blueprints the sim keeps).

| MB | During the New Game build | During play | Source |
| --- | ---: | ---: | --- |
| Program, loaded, with its stacks (`psp-sim.prx` is 4.4 MB on disk: the whole sim stack; the renderer will add to it) | 3.6 | 3.6 | measured: 24 less the 20.4 free |
| World build (peak: the county on its canvas, packed before its solve) | 8.7 | | measured, PPSSPP |
| Sim (blueprints packed, state, runtimes, A\* scratch, journal; the replay's peak) | | 6.7 | measured, PPSSPP |
| Art cache (§13.2's 3 MB RAM, twice for play) | 3 | 6 | budget (the loading screen's art only during the build) |
| Audio (tracker module and sample bank, §13.2) | | 0.81 | measured (made once the world is built: 805 878 B and the thread's 32 KB stack) |
| **Headroom** | **7.2** | **6.2** | of 24 |

Allocator overhead and fragmentation come out of the headroom (the counts are requested bytes). With the full 6 MB of art held during the build as well, the headroom then is 4.2 MB.

### 13.3 The sim diet (gameplay-neutral, hash-proven)

Every item must leave the state hash and replay tapes byte-identical; a diet that changes a hash is a gameplay change and is refused.

**The instrument (2026-10-08):** `jane bench heap [--seed N] [--ticks N] [--json]` (`crates/jane-cli/src/heap.rs`) counts requested bytes through `jane-cli`'s `cap` allocator (the one `#[global_allocator]` the lints allow; no unsafe of ours), as §13.10's spike did: live and peak per county stage and per zone (a ballast lifts the live count to `cap`'s unresettable peak, so each stage's own high-water mark reads exactly), each blueprint retained and by field (a lone clone of each field), and the sim after New Game and idle ticks. Decimal MB, 64-bit host. The gate is `crates/jane-cli/tests/heap_budget.rs` (slow tier): build peak, blueprints and sim at New Game held about 5% over the last measure, and the build under the 20 MB target; ratchet it down with each diet step.

**Measured, seed 1, before phase 1** (peak while each stage ran, over the live bytes before it; seeds 2 to 4 within a few per cent):

| Rank | Stage | +peak MB | What it was |
| ---: | --- | ---: | --- |
| 1 | `gardens` | 20.7 | four county `Vec<bool>` planes (busy, blocked, flood before and after) and two floods' runs |
| 2 | `ways` | 20.0 | a `u32` prop index for each of 4 M cells (16 MB) plus a copy of `trodden` |
| 3 | `solve` (county) | 17.9 | the solver's `seen` and `blocked` planes a byte a cell, the restamp scratch, the trail's copies, the flood's runs |
| 4 | `stories` | 13.1 | the stopping and walkable planes (bytes), a flood's runs |
| 5 | `cut_through` | 12.7 | the same flood planes; `reached` kept for the next stage |
| 6 | `perimeters` | 9.0 | the keep and blocked planes |
| | `County::new` (in `skeleton`) | 16.2 | tiles 4 MB, claims, `trodden`, `wild_earth` 4 MB each, held the whole build |
| | `roads` | 4.1 retained | the ground before the roads (a tile grid), held to the end |

Build peak **51.8 MB** (§13.10's PSP figure, 52.0, agrees), the thirteen blueprints **11.5 MB**, the sim at New Game **22.5 MB** (blueprints included).

**Phase 1, done (2026-10-08), each its own commit, world hash fixture and sim goldens unchanged:**

1. County flag planes a bit a cell (`jane_world::bits::Bits`): claims, `trodden`, `wild_earth`, `reached`, `ground`, the flood planes, perimeters' keep, gardens' busy. **49 -> 32 MB.**
2. `ways`: the prop-feet plane sparse (`BTreeMap` by cell). **-> 26 MB.**
3. The solver's `seen`, `blocked`, `any` and restamp scratch as `Bits`; the 64-cell word read straight from them. **-> 23 MB.**
4. The build drops each plane once no later stage reads it (`County::release_after`): the ground before the roads, the walkable plane, the road distance fields. **-> 19 MB.**
5. Finished blueprints shrink to fit (`Blueprint::shrink_to_fit` in `build_zone_with`). Blueprints **11.5 -> 10.0 MB.**
6. Whole-county floods keep no runs (`Fill::bits_only`, `into_seen`); the solver ORs the fill's bitset into its layers. **-> 16.3 MB.**
7. `County::done` lets the earth planes go before the paint grows; the paint reserves exactly. **-> 15.3 MB.**

**After phase 1, seed 1:** build peak **15.3 MB** (seeds 2 to 4: 16.3, 15.5, 16.0), now `drop_unreachable` plus `done` (+2.6 over 12.7 live) and `gardens` (+2.5); the county's live floor while it builds is about 12.5 MB, most of it the growing blueprint. Blueprints **10.0 MB** retained: the county 8.6 (tiles 4.0, paint 2.3, props 1.3, local names 0.6, units 0.2), the other twelve 1.4. Sim at New Game **21.0 MB**: the blueprints plus about 11 MB of sim (a flags plane per zone, 4.2 MB, the copied `Prop` and `Unit` state, the per-zone lookups and buckets, the A\* window 1 MB plus its heap).

**Cross-check:** §13.10's host (`psp-sim-host verify`) on the dieted crates replays the tape to the same final hash, `520a733ef4dcf12c`, with build peak 15.9 MB (was 52.0) and replay peak 21.5 MB (was 23.1).

**Against §13.2:** the build target (under 20 MB) is met. The resident target (6 MB) is not: blueprints alone are 10 MB. That takes the structural items below.

**Phase 2 (structural, still hash-neutral), in order of bytes:**

1. **Zones built on demand.** Keep the seed; build a zone's blueprint when a seat enters it, drop it when none is there (the twelve small zones build in well under 2 s emulated, §13.10). Saves 1.4 MB now and lets the build stream. The sim already reads blueprints through `Blueprints::get`; the work is lifetime, not logic.
2. **The county's tiles compressed.** The stages are global, so a chunk cannot be regenerated alone from the seed; the practical form is a **compressed tile plane** (row runs or a small per-chunk palette), decoded a chunk at a time into an LRU near the seats. Expect 4 MB -> under 1 MB.
3. **Paint as runs on the tile plane.** 2.3 MB of `(Rect, Material)` is mostly one-row `WildEarth` runs; store them as `(y: u16, x0: u16, x1: u16, Material)` (7 B) or as a material bit plane, and have the hash encode the same bytes it does now.
4. **The sim's flags plane as an overlay.** `ZoneGrid::flags` is the tiles' own flags plus stamps; keep a sparse stamp map over `tile.flags()` (stamps are thousands, cells millions). 4.2 MB -> tens of KB; the read is one lookup more on the hot path, so bench it.
5. **Compact `PropSpawn` and local names.** Box the rare fields (`to`, `night_lock`, `under_when`, `label`), intern the place-names (`county_rock_812_40`) as `(kind, x, y)` and print them on demand. About 1.5 MB.
6. **Compact A\* nodes** (16 B now; `from` fits `u16` in a 256 x 256 window, the two generations one word): 1 MB -> 0.5 MB. Paths unchanged.
7. **No `usize` in state, no floats, no random hasher** (already rules: §1, §11). The `no_std` plus `alloc` build of the sim crates is in CI (`build-psp`).

**Phase 2, done (2026-10-08), each its own commit; world hash fixture, sim hash, replay and save goldens and the bot fixture unchanged:**

1. **The sim's flags plane paged** (item 4). `ZoneGrid` reads the base tiles' own flags; a 64-cell run gets a page only where a stamp, a body or a changed tile differs, and gives it back when the run is the tiles' again. Zones under 256 K cells keep a byte a cell (one read, and small). Runtime grids **4.61 -> 1.16 MB**. A randomised test holds the pages to the byte plane.
2. **A\* nodes 16 -> 8 bytes** (item 6): `g` plus one mark (generation, closed bit, the step it came by; the parent is one step back). Path scratch **1.58 -> 1.06 MB**.
3. **Packed blueprints** (items 2 and 3): `jane_core::plane::Plane`, the chunk API below; `Blueprint::pack` folds tiles and paint into it and leaves `tiles` hollow (its size only) and `paint` empty; `Blueprints::packed()` is the console form. The sim reads tiles through `Blueprint::tile` / `tile_in`; a bot tape verifies to every hash over packed blueprints (`jane-bot/tests/determinism.rs`). County tiles **4.00 -> 1.48 MB**, paint **2.33 -> 0.39 MB**. PC keeps the grids, because `jane-art`'s `TileMap::from_blueprint` clones `bp.tiles` and walks `bp.paint` (not edited here; moving it onto the chunk API is the step that lets PC drop them too). `jane_world::hash` refuses a packed blueprint: hash as built, then pack.
4. **The name tail** (part of item 5): `SymTable` kept each generated place name twice (a `String` in the tail, another as a `BTreeMap` key); now one string end to end plus an open-addressed index. It still serialises as the `Vec<String>` it was. Names interned **1.6 -> 0.57 MB**.

**The chunk API** (`crates/jane-core/src/plane.rs`; what a console renderer streams):

- A chunk is **16 x 16 cells** (`plane::CHUNK`); chunk `(cx, cy)` covers cells `cx * 16 ..`, `cy * 16 ..`; the plane's last row and column of chunks are clipped by its edge.
- Each chunk is coded alone: a palette of 1, 2, 4 or 16 values and an index of 0, 1, 2 or 4 bits a cell, or 8 bits raw for more than 16 values. A `u32` descriptor per chunk holds the width code and the offset of its bytes.
- **A cell:** `Plane::get(x, y)` / `read(x, y, outside)` / `at(i)`, O(1), no cache (descriptor, palette, index: three loads). The sim reads tiles this way; the hot flags read never touches it while a page holds the cell.
- **A chunk:** `Plane::chunk(cx, cy, &mut [u8; 256]) -> bool` decodes one whole into the caller's buffer. The renderer owns the cache: about the chunks a screen and a margin cover (a 48 x 27 cell camera is 4 x 3 chunks, 6 x 5 with a margin: 30 x 512 bytes for tiles and paint, 15 KB). Nothing in the plane grows or caches.
- What is packed: `Blueprint::packed: Option<Box<Packed { tiles, paint }>>`, tiles as `Tile::id`, paint as one material a cell (0 none, else `Material` + 1; the last rect over a cell wins, which is all a drawing of the paint reads). `View::packed()` hands both to a presenter.
- Built without a whole grid (phase 3): `Plane::pack_bands` takes a band of 16 rows at a time (how `Blueprint::pack` lays the paint), `Plane::pack_chunks` a chunk at a time (how the console county's canvas packs its tiles).
- Read-only: built once, never written. Play's changes are the zone's tile deltas over it, as over a grid.

**After phase 2, seed 1** (`jane bench heap`; requested bytes, 64-bit host):

| | Before phase 2 | PC (grids kept) | Console form (packed) |
| --- | ---: | ---: | ---: |
| Blueprints, all 13 | 9.95 | 9.95 | **5.31** |
| of them: county tiles, paint | 4.00, 2.33 | 4.00, 2.33 | 1.48, 0.39 |
| Sim state (zones' rows, names, journal) | 3.41 | 2.36 | 2.36 |
| of it: names interned, units, props | about 1.6, 1.04, 0.74 | 0.57, 1.04, 0.74 | 0.57, 1.04, 0.74 |
| Runtime grids (flags, parts, bodies) | 4.61 | 1.16 | 1.16 |
| Path scratch (A\* window and heap) | 1.58 | 1.06 | 1.06 |
| **Sim at New Game (blueprints included)** | **20.99** | **16.17** | **11.52** |
| Sim peak over 600 idle ticks | 21.30 | 16.21 | 11.56 |
| Build peak | 15.30 | 15.30 | 15.30 |

The rest of the sim past those rows (about 1.6 MB) is the runtimes' lookups, prop buckets and unit blocks.

**On PPSSPP** (`spikes/psp-sim`, now packing each blueprint as it lands): the tape replays to **`520a733ef4dcf12c` at tick 3 600**, all 60 hashes checked; build peak 14.7 MB (unchanged; the county's worldgen), live after the build **4.7 MB** (32-bit), replay peak **10.5 MB** (was 19.6), replay 5.4 s emulated (was 5.6).

**Tick** (`jane bench sim --model rusher --seeds 1,2 --minutes 30`, run interleaved before and after): the step's p50 1 us, p99 20 to 24 us outdoors and 110 to 111 us in the museum, mean 1 to 4 us: the same before and after, within run-to-run noise; the bot's own time is a few per cent up (it reads the flags through the `View`). The gate `tests/heap_budget.rs` now holds the PC sim and the packed form too.

**Phase 3, done (2026-10-08), each its own commit; world hash fixture, sim hash, replay and save goldens, the bot fixture and every jane-present, jane-art and jane-render-soft test unchanged:**

1. **Rare fields out of line** (`jane_core::rare::Rare<T>`, a box made only when a field is set, read and written through `Deref` so no reader changed): `PropSpawn` 128 -> 32 bytes (`PropRare`: `key_tag`, `to`, `loot`, `use_list`, `release`, `needs`, `talk`, `label`, `night_lock`, `under`, `under_when`; one row in eight sets any), the state's `Unit` 272 -> about 100 (`UnitRare`: cooldowns, statuses, feel, order, snake, seated, `died_at`, `carrying`), `Prop` 72 -> 32 (`PropMore`: loot, night, regrow, `burns_until`). The save writes each field in its old place (`UnitOut`/`UnitIn`, `PropWire`), so its bytes and the state hash are as before. Blueprint props **2.18 -> 0.83 MB**, zone units **1.04 -> 0.49**, props **0.74 -> 0.33**.
2. **Names end to end** (`jane_core::Names`: one string plus ends, shared on clone): a blueprint's `local_names` (**0.88 -> 0.36 MB**), the builder's index over them (`NameIndex`, no second copy as map keys), and the sim's `SymTable`, which keeps a zone's names as a run of that very list (less those already interned) instead of copying them (**0.57 -> 0.21 MB**). The hash feeds `Names` as the `Vec<String>` it was.
3. **The runtime**: feet masks interned (11 300 county cells held 32 bytes each; now a number into a few dozen masks), prop buckets one block-ordered list (15 625 `Vec`s were 0.4 MB), the A\* heap reserved at 4 096 entries (it was 65 536, never used; it grows if a search needs it). Runtime grids **1.16 -> 0.71 MB**, buckets **0.42 -> 0.10**, path scratch **1.06 -> 0.57**.
4. **The console build** (`jane_world::build_zone_packed_with`, `Blueprints::build_packed_with`; equal to the PC build packed, `tests/packed_build.rs`): the county is laid on a **chunked tile canvas** (`jane_world::canvas`, 16 x 16 chunks coded by how many kinds each holds: inline, two bits, four, or a byte; widened on write, compacted after the stages that write all over it; 1.5 to 1.9 MB where the grid was 4 MB), packed straight from it as it finishes (its paint packed a band at a time with the wild earth laid over it a cell at a time, never the 2 MB of earth rects), and judged by the solver packed (the solver reads tiles through `Blueprint::tile`). PC keeps the grid.
5. **The county build's planes**: the ground before the roads kept as a water bit plane (4 MB -> 0.5), a tale's ground on foot as bits (the flood's distances were 1.3 MB), the edges' keep and blocked planes per patch box (1 MB), whole-county floods through `Fill::direct` and a 48 KB word cache (no 0.5 MB copy of the open plane), the scanline stack as cell indices, and the builder's lists grown by a quarter, not double.

**After phase 3, seed 1** (`jane bench heap`; requested bytes, 64-bit host):

| | After phase 2 | PC (grids) | Console form (built packed) |
| --- | ---: | ---: | ---: |
| Build peak | 15.30 | 11.85 | **8.26** |
| Blueprints, all 13 | 9.95 (5.31 packed) | 8.21 | **3.58** |
| Sim state (zones' rows, names, journal) | 2.36 | 1.04 | 1.04 |
| of it: names interned, units, props | 0.57, 1.04, 0.74 | 0.21, 0.49, 0.33 | 0.21, 0.49, 0.33 |
| Runtime grids; prop buckets; lookups | 1.16; about 0.4; about 0.3 | 0.71; 0.10; 0.28 | 0.71; 0.10; 0.28 |
| Path scratch (A\* window and heap) | 1.06 | 0.57 | 0.57 |
| **Sim at New Game (blueprints included)** | 16.17 (11.52 packed) | 11.32 | **6.68** |
| Sim peak over 600 idle ticks | 16.21 (11.56) | 11.37 | 6.73 |

The console build's highest stages: `gardens` 8.26, `stories` 8.01, `country` 7.71 (`jane bench heap --county` shows what each part of the county holds after each stage). The gate (`tests/heap_budget.rs`) now asserts §13.2's 9 MB build and 7 MB sim for the console form and holds every row about 5% over these.

**On PPSSPP as a PSP-1000** (`spikes/psp-sim`, the county built packed; `PPSSPP_PSP1000=1` sets the model in this headless build): the tape replays to **`520a733ef4dcf12c` at tick 3 600**, all 60 hashes checked; largest free block at start 20.4 MB; build peak **8.7 MB** (was 14.7), live after the build 3.5 MB, replay peak **6.7 MB** (was 10.5).

**What it cost:** the canvas's reads and writes make the console county build about half as slow again: 0.60 to 0.73 s on the host against PC's 0.39 to 0.53, and **about 66 s emulated on PPSSPP** (was 39). PC's own build is unchanged within noise (`county_build_solve` 404 -> 426 to 437 ms median, interleaved). **Tick:** `jane bench sim --model rusher --seeds 1,2 --minutes 30`, three interleaved runs: last-quarter p50 1 us before and after, p99 10 to 12 us -> 10 us (seed 1) and 111 to 112 -> 112 (seed 2).

**Phase 4, done (2026-10-08): zones built on demand.** World hash fixture, every sim hash, replay and save golden, the bot fixture and the story and spine bots unchanged (they play every zone held, as before); the bot fixture's file, two 30-minute story runs and their tapes are also held over on-demand sets, PC and packed (`jane-bot/tests/zones_on_demand.rs`).

- **`Blueprints::on_demand_with(seed, source, report)`** builds the county alone. A zone is built when the sim first needs it (`Sim::ensure_zone` / `ensure_runtime`: a seat walks in, a save names it) and let go at step 15 once no seat is in it (`Sim::release_blueprints`); the county is never let go (its clock rows). `Blueprints::build` and the rest still hold all thirteen and never let go (tests, tools, a guest).
- **The seam:** `trait ZoneSource { fn zone(&self, seed, zone, report) -> Result<Blueprint, BuildError> }`; `Build { packed, load: Option<fn(seed, zone) -> Option<Blueprint>> }` asks `load` (a per-zone cache, to come) before it builds. Whatever it gives must be what the build gives.
- **The digest** (`ZoneDigest`, made from the blueprint the first time the zone is held; a few hundred bytes plus its spawn rows): the zone's `Names` (shared with the name table), `indoor`, its row counts, and, made when the blueprint is let go while the zone has a state, each spawn row's first instance hashed (xxh3-64 of its save encoding, for that `SpawnBase`). `ZoneForm::of_in` reads the blueprint if held, else the digest: an instance is its row exactly when it hashes as the row's first instance did, so the save and the hash are the same bytes (a 64-bit collision, about 2^-64 a row compared, is the only way they could differ). `sym_runs` skips a zone never built: each zone has a name of its own (tested on seeds 1 to 3), so a zone never interned matches no run.
- **Only the county has areas or regions** (asserted when a zone is held, tested on seeds 1 to 3): the world's rolls draw for the county's areas alone, and a zone not held is under its own sky (`living::region_in_zone`); the ramp reads `indoor` from the digest.
- **Readers of another zone:** `Blueprints::get` is held zones only (the county, a seat's zone); `Blueprints::fetch` gives any zone, built for the call if not held and not kept. `route::place_of_step` (the quest compass; `jane-present` caches each step's place), the bot's crawl and audit, and the app's console travel fetch. A save loads over an on-demand set by holding the zones it names, then letting go of those nobody is in.
- **Ahead of her:** `Sim::zones_ahead(seat, r)` (the zones behind doors within `r` cells, not held) and `Sim::offer_blueprint(bp)` (held until a seat has been in it, or 600 steps): a shell's loader thread can build the zone behind a door before she steps through it. Tested invisible (offered every few seconds through the story runs, every hash the same). The PSP spike does not do it yet.
- **Users:** jane-app's New Game and Load (PC form) and the PSP spike's builder (`spikes/psp-game/src/main.rs`, `build`: `on_demand_with` with `Build { packed: true }` in place of `build_packed_with`; nothing else there changed).

**Measured, seed 1** (`jane bench heap`, the console form; requested bytes, 64-bit host):

| | All thirteen held | On demand |
| --- | ---: | ---: |
| Build (New Game) peak | 8.26 | 8.27 (the county's own) |
| Sim at New Game (blueprints included) | 6.68 | **6.18** |
| Sim after 600 idle ticks, peak | 6.73 | 6.24 |
| New Game, wall time on the host | 1.16 to 1.20 s (all 13 built) | 1.01 to 1.11 s (the county, the sim made) |
| A zone entered: the step's time on the host; the heap's peak in it | not measured | 0.1 ms (house, cellar, arms, church) to 32 ms (factory); peak 6.3 to **8.33** MB (museum) |
| Back in the county after entering and leaving all twelve | | 6.95 (the zones' states, kept as before) |

The gate (`tests/heap_budget.rs`) holds these about 5% over and the entry peak under the 9 MB build target.

**On PPSSPP as a PSP-1000** (`spikes/psp-game`, `script.txt` `300 12 63 county:town_square seed:1`, then `300 22 63 burial:entry seed:1`; interleaved with the same program building all thirteen): New Game's build **52.1 s emulated against 59.0** (the twelve small zones were 6.9 s of it); live after the build **4.63 MB against 5.04**; in play 15.9 MB against 16.3; the play's peak 16.5 against 16.9. Entering the Burial from the county: the step that builds it **1.32 s emulated** (36 ms with it held), the heap's peak 16.6 MB (16.7 held), nothing short. **That stall is the cost:** a step of 1.3 s at a dungeon's door wants the zone built ahead on the builder thread (`zones_ahead`, `offer_blueprint`) or a loading card at the door; the PSP shell's to do (its owner's). The small zones on the host: house, cellar, arms, church under 0.2 ms; library 2 ms; mine 6; forest, pipes 10 to 13; burial, museum, school 12 to 21; factory 17 to 32.

Against §13.2: the console sim is 6.2 MB at New Game, not yet the Dreamcast's 6 MB (nor the 5 MB hoped for): what is left at New Game is the county (its blueprint about 2.9 MB packed, its runtime, its rows) and the scratch, which on-demand zones do not touch.

**Left:**

1. **The PSP shell builds ahead:** at a door, the zone on the builder thread (or a loading card), so no step stalls 1.3 s (above).
2. **The console build's time**: the canvas reads in stories, country and the edges; a decoded-chunk cache near the writer or wider pools for the busiest chunks would win back most of it.
3. **PC drops its grids** once `jane-art` reads the chunk API (`TileMap` over a `Plane`): not contained (`TileMap` owns the mutable tile and paint grids its painter and houses read a cell at a time), so left for the renderer's side.
4. **The flags pages** (0.4 MB of 64-cell runs, a third of it stamps of a prop or two): square pages would halve them.

Against §13.2: the console form builds at 8.3 MB and runs at 6.7 MB (6.2 with zones on demand, phase 4), inside the PSP-1000's 24 MB with the art and the audio beside it (§13.2's table), not yet the Dreamcast's 6 MB sim.

### 13.4 The bake (compile-time, all targets)

Generators run once on a dev machine and write a **target-neutral canonical pack** (indexed RGBA sprites with all four layers, note and sample data). **Per-target packers** then convert it. The runtime on every target only loads.

| Target | Art | Music |
| --- | --- | --- |
| PC and Pi | Today's atlas path (§9.4 cold boot stays valid; the bake is an optional cache, as the page cache is now) | Procedural synthesis, as built |
| PSP | Swizzled 8-bit (or 4-bit) paletted pages, albedo only, paged per zone | Tracker module plus shared bank |
| Xbox | Swizzled pages, albedo plus emissive, paged per zone | Tracker module plus shared bank |
| Dreamcast | PVR twiddled, VQ or paletted, albedo only, paged per zone | Tracker module plus shared bank in sound RAM; the AICA's 64 voices carry the channels |

Rules: the bake is deterministic (same seed and generator version, same bytes, hashed in CI); outputs are cached by input hash; nothing is generated at runtime on a console. Music is **MOD-style by decision (2026-10-08)**: sequenced patterns plus samples, never streamed audio, so the file is kilobytes and the same module plays on every target.

Existing rule (ART-PLAN §3, 2026-10-03) is the art side of this: **motion is a draw-time deform of one static sprite**, not baked frames, unless the frames are tiny. Features built as draw-time code carry to consoles; baked frames and big caches do not.

**As built (2026-10-08): `jane bake [--out DIR] [--target psp] [--force]`** (`crates/jane-cli/src/bake.rs`, `bake_psp.rs`; host only, `std`, integer only; reads the generators, changes nothing they draw). Default out `target/bake/`. Skipped when `DIR/stamp.txt` names this build (the FNV of the running `jane` executable, which holds the generators and the looks); `--force` bakes again. 1.7 s in release.

- **Canonical pack** `canonical.jbk` plus `manifest.tsv` (one line a frame). Holds every look (`looks::all()`: units with every variant, seat and frame, props, buildings, icons at 32 and 16), the flora bank, every terrain style in its sixteen neighbour contexts (the centre cell and what the tile stands above it, the rest clear) and every glyph of the four faces, regular and bold (flat, `Ix::INK`). Key: category, sprite (a `SpriteId`; bank index, style index or code point otherwise), variant and seat (one byte, high and low nibble), frame (`FrameId as u8`; the mask; face * 2 + bold). Layout, little-endian: `"JBK1"`, `u32` palette length, `u32` master length, `u32` items; the palette as RGB triples; per item `{u8 cat, u16 sprite, u8 frame, u8 vs, u8 flat, u16 w, h, i16 ax, ay}`; then per item the albedo (`u16` a px) and unless flat the normal (`[u8; 2]`), emissive (`u16`) and height (`u8`). A terrain colour the master palette lacks would be appended after it; today there are none (the chunk painter writes only master colours), so the palette is the master's 1107. No seed: 9 767 frames, 74.4 MB, hash `1cc26bdbb108b81b`; `bake::tests::same_build_same_bytes` builds it twice and compares.
- **Not in it yet:** the fx, weather, parallax and far-landmark sprites, the chrome's sweeps and marks, the stand-ins (the PSP pack takes the presenter's own atlas for those it packs as refs, below; the presenter's UI page, with its marks and sweeps, is not in the PSP pack yet: the font and icons are, as canonical items). And terrain is samples, not the game's terrain: the county is painted a chunk at a time from the seed by `terrain::Painter`, which cannot be baked seed-free (see §13.11).
- **PSP pack** `jane-psp.jpk` (`JPK2` since 2026-10-08; was `JPK1`): albedo only, from the canonical pack **and the presenter's own atlas** (below). Each frame trimmed to its drawn rect (anchor moved with it), identical frames stored once, then shelf-packed one sprite set (sprite, variant, seat) at a time in key order within its category, frame by frame (tallest first) onto the oldest of the category's (for units, the sprite's) four newest pages whose CLUT can still take the set's colours **exactly**, else a new page. Pages are 256 x 256 (a frame over 256 gets a page of its own, at most 512; the last page of a run is cut to the next power of two it uses), `GU_PSM_T8` with one 256-entry CLUT each: 0 clear, 1 the contact shadow (`0x58301010` ABGR, alpha blending where the PC multiplies), 2 to 255 the page's colours. Only a set that alone draws more than 254 colours forces a quantised page: integer median cut, each entry the drawn colour nearest its box's mean, so a CLUT never holds a colour the art did not draw; **no page needs it today**. CLUT in `GU_PSM_8888`: the 1 KB a page is noise beside the pixels, it keeps the master palette's 24-bit colours exactly where 5551 would requantise every ramp, and the contact shadow needs a partial alpha. Pixels swizzled for the GE: 16-byte by 8-row blocks, row-major blocks.

- **The presenter's atlas** `presenter.jat` (`JAT1`, 2026-10-08): `jane bake` builds `Present::new(T0)` and writes its own atlas (`Atlas::to_pack`: every page in every layer it has, the sparse glow, the mist, the CLUT, and the sprite table in `RefId` order, each ref with its **bake key** where it was packed from a look: people, creatures and their lantern sets, props, buildings, flora). `Atlas::from_pack` reads it back (`no_std`): `atlas::tests::the_atlas_from_the_pack_is_the_atlas_from_the_generators` holds it equal to the generators' atlas at T0 and T1, every layer, ref and key. 8 652 refs (8 034 keyed), 5 pages, 34.2 MB at T0, hash `2a2d737678646613`. The PSP packer reads it: a keyed ref must draw exactly the canonical item's px (else the bake fails: **the presenter and the packs cannot drift**), a keyed ref the canonical pack lacks (a person's lantern set, seat bit 3) joins its sprite's group, and an unkeyed one (stand-ins, the sky, the cues, critters, cast glows, the house variants, the canopy clusters) becomes a `scene` sprite keyed by its `RefId`; one over the GE's 512 is left out (its ref draws nothing on C2). **Still to do for a console boot:** `Present::new` builds its module tables (each look's frames, glass rows, flora kinds, the UI page's glyph metrics) beside the atlas by running the generators; a console needs those tables in the pack too, so the presenter can boot from it without generating. The atlas half is done.
- **Page groups:** a unit's pages hold only that unit (its `SpriteId`: every variant, seat, frame and lantern set), so a zone loads the units it spawns and no others; every other category is one group. Groups are contiguous runs of pages.

`JPK2`, little-endian (the PSP is), no parsing library needed:

```
header, 48 bytes   [u8; 4] "JPK2"  u16 version (2)  u16 page_count  u32 sprite_count
                   u32 page_table_off (48)  u32 sprite_table_off  u32 data_off (64-aligned)  u64 canonical pack hash
                   u32 group_table_off  u16 group_count  u16 0  u32 ref_table_off  u32 ref_count
page, 16 bytes     u32 offset (64-aligned: 1024 bytes of CLUT, then the w * h swizzled pixels)  u16 w (power of two, = TBW)
                   u16 h (power of two)  u8 category  u8 psm (5 = T8)  u8 clut psm (3 = 8888)  u8 flags (bit 0 swizzled, bit 1 quantised)
                   u32 pixel bytes
sprite, 20 bytes   u8 category  u8 frame  u16 sprite  u8 variant << 4 | seat  u8 0  u16 page (0xFFFF: draws nothing)
                   u16 u, v, w, h  i16 ax, ay (the anchor, from the trimmed rect's top-left)
                   sorted by (category, sprite, variant and seat, frame): a binary search finds a frame
group, 8 bytes     u8 category  u8 0  u16 sprite (a unit's SpriteId; 0xFFFF: the whole category)  u16 first page  u16 pages
ref, 8 bytes       per presenter RefId, in order: u32 sprite index (0xFFFFFFFF: draws nothing on C2)
                   i16 ax, ay (the presenter's anchor, from the trimmed rect's top-left; a prop's is its footprint's foot)
```

Categories: 0 terrain, 1 flora, 2 units, 3 props, 4 buildings, 5 icons, 6 font, 7 scene (`jane_present::atlas::cat`).

**Measured, the PSP pack** (2026-10-08, `JPK2`: 15 273 344 bytes, hash `cd412a4a9cf327ef`; was `JPK1` 7 628 352; bytes are CLUTs plus pixels; *rects* is the trimmed frames' share of the page area, *drawn* their opaque px):

| Category | Frames | Unique | Pages | Bytes | Rects | Drawn |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| terrain (samples) | 848 | 688 | 5 | 332 800 | 79% | 75% |
| flora | 92 | 92 | 3 | 199 680 | 68% | 36% |
| units (grouped by sprite; with the lantern sets) | 7 625 | 7 212 | 148 | 8 073 216 | 65% | 42% |
| props | 441 | 401 | 6 | 399 360 | 71% | 54% |
| buildings | 19 | 19 | 9 | 566 272 | 58% | 57% |
| icons | 186 | 186 | 2 | 133 120 | 65% | 43% |
| font | 880 | 849 | 5 | 275 456 | 72% | 35% |
| scene (the presenter's own) | 616 | 454 | 56 | 5 005 312 | 45% | 9% |
| **total** | | | **234** | **14 985 216** | | **249% of 6 MB** |

Against §13.2 (3 MB RAM plus the 2 MB VRAM, *resident*): the file grew (units 5.5 to 8.1 MB, since a sprite's last page is no longer shared, and the presenter's own sprites, 5.0 MB, are new), but it lives on the Memory Stick; what must fit is what a place keeps resident, and now a zone's units are page groups it can load alone (about 55 KB a unit sprite on average). **The scene category is the next lever:** 9% drawn, its sky, treeline and landmark sprites are wide and mostly clear, a page each; cut them into strips, or draw them at run time on C2, before it ships. In order of bytes, the art cuts (all **open, the owner's call**, §13.7; none changes the PC art): **diagonal facings cut on C2** (14 of 50 frames a set, about 1.5 MB), **walks of four frames, not six** (about 0.6 MB after the diagonals go), **seats as CLUT swaps** (a seat is a ramp remap, so seat 1 to 3 is the seat-0 pixels with a second CLUT: 249 frames, about 0.2 MB), **the font in `T4`** (one ink: half its 0.28 MB).

**The sound, as built (2026-10-08): the tracker (option (b)), decided by measurement.** The choice was (a) the PC's synth (`jane-audio`'s voices, beds and reverb) ported to integers and run live on the PSP, or (b) the module the owner chose: the synth rendered once at the bake into patterns and a shared bank, mixed in integers on the console. Measured on the same busy load (the combat cue, three beds, an effect every 10 blocks, 512-frame blocks at 22.05 kHz) on the host: the PC's synth 318 us a block, the tracker 41.6 us, **7.6 times cheaper**, before the synth's floats are ported at all; the tracker on PPSSPP takes 13% of a PSP-1000 (below), so the synth would take about all of it. (b) it is; the same patterns play on every console.

- **The module** `jane-psp.jau` (`jane bake --target psp`, `jane_audio::bake`, `std`, 21 s with the rest of the bake; `jane_audio::tracker`, `no_std` and integer): a `JAU1` prefix, a postcard header (every song's tracks, sections, chords resolved to semitones, patterns a varint a step, forms; every instrument's zones, envelope, velocity curve and each note's level; every effect and bed), the resident frames, then the far frames. Samples are the PC's own voices, patches and beds rendered at 48 kHz, resampled (windowed sinc) to the lowest of 11 025, 16 000 or 22 050 Hz that keeps 99% of the energy (instruments and longer effects at 16 kHz at most, beds and effects past 1.5 s at 11 kHz, short effects to 22 kHz), stored as **the PlayStation's own 4-bit ADPCM** (VAG: 28 frames in 15 bytes, five two-tap predictors, a shift a block; IMA's single step lost 10 to 15 dB SNR on bright strings and reeds), 8-bit mu-law where VAG falls under 18 dB (2 of 172). An instrument has a sample every 12 semitones (a choir's formants every 2); a held voice is its sustain looped (its attack and decay drawn by the player from the PC envelope), a struck one whole for 0.45 s and then a flattened loop of its tail falling at the energy-matched rate. Each zone's level, each note's level and each instrument's velocity curve (louder notes are brighter on the PC) are **measured at the bake** against the PC voice, so the bank plays at the PC's level. **1 372 636 bytes; held in RAM 805 878**: patterns 51 923, instruments 275 828 (83 zones), effects 212 195 resident (bells and thunder tail-looped), beds 188 967 (2 to 4 s seamless loops; a wide bed plays twice, half a loop apart, a side each), the far slot, the room's lines and the mixing buses. **643 723 bytes stay on the Memory Stick** (the lesson cues, a quest's, rest, waking, save, boots, the train): read into the one far slot (49 KB) on the game's thread when asked for, the PC's lazy effects' rule (PLAY-PLAN.md §7).
- **The player** (`tracker::Player`) is `seq::Player` in integers, draw for draw on the same xorshift: **the same notes from the same seed as the PC** (`the_tracker_plays_the_pcs_notes_from_the_same_seed`: every song, two seeds, track, pitch and chromatic equal, timing within the humanising). The mixer: at most 24 music voices (the quietest stolen in 8 ms; the PC lets 48 sound, the rest are tails 30 dB down), 16 effects, the ten beds; each voice decoded a run ahead, linearly interpolated, mixed mono into one of 72 place-and-send groups (each group panned and sent once); a voice from an 11 kHz sample mixed at half rate; the PC's eight-line room at half rate; a look-ahead limiter at the PC's ceiling. Integers only (`#![deny(clippy::float_arithmetic)]`), no allocation after it is made.
- **Against the PC** (`crates/jane-audio/tests/tracker.rs`, 24 s of each cue, each side's gated loudness and chroma): all 19 cues **within 0.6 dB** of the PC's loudness, chroma alike 0.97 to 0.999; every effect within 2.5 dB (the bright ones lose what lies over 11 kHz) and every bed within 1 dB.
- **On the PSP** (`crates/jane-audio-psp`; the spike calls `jane_audio_psp::psp::start(dirs, seed, scripted)` where the world is made and `sound.tick(&view, &events, &present)` after each stepped tick): the cue table's asks become commands on a lock-free ring (four 32-bit atomics a slot; the MIPS has no 64-bit ones); an audio thread at priority 16 (the game's is 32) mixes 512 frames into the next of four 64-aligned buffers and hands them to sceAudio's sample-rate converter at 22.05 kHz, blocking. PPSSPP headless as a PSP-1000, the town in the rain 28 s and the square at 20:00 into the storm and the bell at nine (70 s): **no underrun**; the mixer **2.6% idle, 9 to 13% busy** (3.0 ms a 23 ms block with 20 voices, 15 voice-frames a frame; PPSSPP's clock is about an instruction a cycle: a million multiply-adds read 24 ms); fps in play **58.6 against 59.0** without the module (the frame had headroom there); free RAM in play 1.66 MB against 2.42 MB without, the largest block 0.72 MB against 1.45 MB. A scripted run writes `psp-audio.wav` and `psp-cmds.txt` beside it; `jane audio replay psp-cmds.txt --wav psp-audio.wav` plays the same commands through the host's tracker (**bit for bit the PSP's**: 0 of 3 251 200 samples differ) and the PC's synth: loudness -29.4 against -28.9 dB, chroma 0.990 (`progress/2026-10-08_59_psp-audio/`, local).
- **Open:** the title and loading screens are silent (the sound starts with the world: its 0.8 MB does not fit beside a New Game's build); a held world does not duck the music yet (`Sound::bus.set_held`); one far slot, so a second far effect stops the first.
### 13.5 Console tiers

Added to the `Features` ladder (`PRESENTATION.md` §1.12) as rows below `soft`; a console tier is a bake and renderer setting, never a fork of the game. Gameplay tests (bots, hashes, replays) run unchanged across tiers.

| | Xbox (`C1`) | PSP (`C2`) | Dreamcast (`C3`) |
| --- | --- | --- | --- |
| Albedo | Paletted, per-page palettes | 8-bit paletted pages, 256 per page | VQ or 8-bit paletted pages |
| Normal, height | Kept if the budget allows | **Normals kept** as baked `T4` relief pages lit by a per-frame CLUT (owner, 2026-10-08, §13.12); height cut | **Cut** |
| Emissive | Kept | **Kept** as a glow CLUT a page, added over the light, and halos (owner, 2026-10-08, §13.12) | Cut; glow sprites at draw time |
| Lighting | Normal-mapped, as T1 | Multiply lightmap (the `soft` method), on the GE (its own 128 x 128 target) | Multiply lightmap |
| Shadows | Hard, as T1 | **The sun's silhouettes and every light's shadows kept** (T0's bands and slabs, the GE's stencil and its own lightmap target; still lights' static shadows cached as textures; owner, 2026-10-08, §13.12). Limits: the lamps' shadows at a quarter size (soft-edged), no feather on the sun's, raised terrain neither takes nor casts a lamp's shadow on itself, a cached pool lags what is set down near it by up to 30 frames | Blob only |
| Master palette | Per-page | Per-page 256 (the 1024 master is cut) | Per-page 256 |
| Sprite variety | As PC | Trimmed variants and cycles, by test | Trimmed further |
| Weather and parallax | Kept, thinned | Reduced overlays | A few cheap overlays |
| Ambient particles | Kept | Fewer | Fewer |
| Canvas | 640 x 480 class | 480 x 272 at the native 16 px a cell, a 30 x 17-cell view (decided for now, 2026-10-08); 10 px a cell (48 x 27) still open, 13.7 | open (13.7) |
| Frame rate | 60 | 60 wanted, 30 allowed | 30 allowed |
| 4-seat LAN | Yes | Yes (ad hoc or infrastructure) | **Cut or later** (broadband adapter only) |
| Saves | Memory unit or disk | Memory Stick | VMU, tight; saves are small (ARCHITECTURE §3.5) |

**Hardware floor amendment:** §1's "30 fps is never a pass" applies to PC and Pi. A console may pass at 30 where this table says so; nothing else relaxes.

### 13.6 What never changes

Worldgen, sim, AI, combat, quests, saves, determinism, the replay and hash checks, the county's size and density, the story spine, the art direction and tone. A console may draw less of it, never play less of it. The seeded county does not shrink; only how much is resident at once.

### 13.7 Open decisions

**Decided 2026-10-08 (the owner, for the PSP renderer groundwork):**

1. **Terrain:** the integer terrain painter (`jane_art::terrain::Painter`) runs on the console, a chunk at a time, from the seed: terrain cannot be baked seed-free, so this is the one exception to "nothing generated on a console". The pack carries only what is seed-free. `jane-art` builds `no_std` plus `alloc` for the PSP (CI `build-psp`).
2. **Units:** pages are loaded per zone (only the sets a zone spawns; `JPK2`'s page groups, §13.4), with **no art cuts**. The cuts in §13.4 (diagonal facings, four-frame walks, seats as CLUT swaps, the font in `T4`) stay **open** for the owner.
3. **Canvas:** native 16 px a cell, 480 x 272, a 30 x 17-cell view on the PSP for now (the 10 px question below stays open).

**Decided 2026-10-08 (the owner, for the first playable build, §13.12):**

4. **The target is the PSP-1000:** 32 MB, about 24 MB for a homebrew, and 2 MB of VRAM (this section said 4 MB in places: corrected). Textures may sit in main RAM for the GE, a small VRAM cache for hot pages; a zone's page groups streamed from the pack, never the whole pack.
5. **The PSP keeps its lighting:** normal relief, the sun's cast shadows and what glows, by the GE's means (§13.5's C2 column changed; §13.12 says how).

**Still open:**

- **C2 art cuts** (§13.4): diagonal facings, four-frame walks, seats as CLUT swaps, the font in `T4`. Each trims the PSP pack, none touches the PC art; the owner's call, by test.
- **PSP canvas:** 480 x 270 at 10 px a cell keeps the 48 x 27 camera (§6.h) and every sim constant, at 0.625 of the PC art scale (so the bake must re-render sprites, not downscale). The alternative, 30 x 17 cells at 16 px, shows less world and breaks the half-screen and density guarantees. Recommended: 10 px a cell. **Checked 2026-10-08: the generators cannot render at another cell size cheaply.** Procedural is not scale-free here: the person is drawn on a fixed 32 x 40 frame with its build, poses, hair and faces in literal px (`person::W`, `H`, `build.rs`), `canvas::CELL_PX = 16` is a constant read by 15 files (the kit's footprints, the house painter, the terrain patterns, flora), and the pixel-art rules (outlines, clusters, two-px noses, the colour budget) are tuned at that size. A 10 px set is a second art pass per family (people, creatures, kit, houses, terrain, flora; icons and font stay, being UI): a scale parameter threaded through every generator, re-tuned shapes at 20 x 25, its own goldens and the §3.1 critique loop. Estimate: weeks, at the art bar, and a real cost to the PC art if done carelessly. So the first PSP pack is at the native **16 px a cell, a 30 x 17-cell view at 480 x 272**, and the 10 px decision stays open.
- **Dreamcast canvas:** 640 x 480 is 4:3. Letterbox 16:9 or show extra rows; decide when C3 starts.
- **Rust on SH4 and original Xbox:** spike before either port is scheduled. Fallback: the sim as a static library called from C.
- **Whether `C1` keeps normals.** Decided by the Xbox's real fill rate at the time.
- **Platform interface:** keep it to a handful of calls (video, input, audio, net, file) so a C shell can host the Rust sim.

### 13.8 PSP spike result (2026-10-08)

`spikes/psp-smoke/` (its own `[workspace]`, nightly via its `rust-toolchain.toml`, not part of the main build) is a `no_std`, integer-only, allocation-free loop of 100 000 xorshift steps that writes `SMOKE hash=... ticks=...` to fd 1. **Measured:** nightly Rust 1.101 plus `cargo-psp` 0.2.10 builds a working `.prx` and `EBOOT.PBP` with no `pspdev` and no WSL; PPSSPP's headless runner executes it and prints `hash=201fbe426b480fc3`, **identical to the same loop computed on x86_64**. So integer determinism holds across the MIPS PSP core and the PC for this loop. It proves the toolchain and the hash method, not the sim, memory or speed.

Reproduce (Windows):

1. `cargo install cargo-psp`; `rustup toolchain install nightly -c rust-src`.
2. `cd spikes/psp-smoke && cargo +nightly psp --release`.
3. PPSSPP headless: clone `hrydgard/ppsspp` with submodules to a directory outside the repo, check each submodule out at its **pinned** commit (a `--depth 1` clone leaves them empty or off-pin), build `SPIRV-Cross` then `PPSSPPHeadless` with `msbuild Windows\PPSSPP.sln /p:Configuration=Release /p:Platform=x64 /t:PPSSPPHeadless`.
4. `PPSSPPHeadless.exe target/mipsel-sony-psp/release/psp-smoke.prx --root=. --timeout=30` (a `.prx` or `.elf`, not the `EBOOT.PBP`; the program must end with `sceKernelExitGame`; `dprintln!` is not captured, a `sceIoWrite` to fd 1 is).

Next spikes, in order: (1) build the real `jane-core` and `jane-sim` for `mipsel-sony-psp` and report what fails (std use, `HashMap`, floats, `usize` in state); (2) run a short replay tape on PPSSPP and compare its state hash with the PC's; (3) measure peak heap and tick time under the emulator.

### 13.9 What breaks building the real crates for the PSP (2026-10-08)

Built with `cargo +nightly build --target mipsel-sony-psp -Zbuild-std=...`, target dir outside the repo. Nothing in `crates/` was changed; the `jane-core` experiment ran on a scratch copy.

| Finding | Detail |
| --- | --- |
| **`std` does not build for the PSP target** | `-Zbuild-std=...,std` fails inside the standard library (no allocator, no sync primitives, no io error for this OS). The crates must be `no_std` plus `alloc` for the PSP; the PSP shell supplies the allocator and I/O through `rust-psp`. This amends �1 "std only": PC and Pi keep `std`, the float-free crates gain a `std` default feature and a `no_std` build |
| **`jane-core` is nearly there** | After mechanical edits on a scratch copy (`#![no_std]`, `extern crate alloc`, `std::` paths to `core::` or `alloc::`, `HashMap` to `hashbrown`) it **compiles for the PSP**. The one real change: `IndexMap` has no default hasher without `std`, so it takes `BuildHasherDefault<FnvHasher>` (the crate already has `FnvHasher`) and `new()` becomes `default()` |
| Floats | **None** in `core`, `schema`, `data`, `world` or `sim` (0 `f32` or `f64` hits). The PSP is single-float only, so this matters and holds |
| `usize` | 79 in `core`, 524 in `world`, 245 in `sim`, 171 in `schema`. It is 32 bits on the PSP and 64 on PC. Most is indexing, but any `usize` that reaches state, a hash, or arithmetic that can exceed 32 bits is a cross-target hash risk (already �11); audit with the `checked` profile |
| `AtomicU64` | `jane-sim/src/grid.rs:76`. The PSP target has no 64-bit atomics. Replace with a counter owned by the sim, or `AtomicU32` |
| `OnceLock` | 8 uses in `sim` (`combat`, `hooks`, `light`, `quests`, `replay`) and 1 in `world` (`county/country/defs.rs`). Not in `alloc`. Replace with `once_cell::race` or `spin::Once`, or build the value at start-up |
| `Arc` | `jane-sim/src/blueprints.rs`, `grid.rs`, `runtime.rs`. `alloc::sync::Arc` needs atomic pointers, which the PSP target has at 32 bits; verify at link time, else `Rc` (the sim has no threads) |
| `Mutex` and `fs` | Only in `jane-data` (dev-data loader, off by default) and `jane-schema` (the `compile` feature, host only). The game links neither on a console, so they stay `std` |
| Other `std::` | Everything else counted is a re-export of `core` or `alloc` (`mem`, `fmt`, `cmp`, `ops`, `BTreeMap`, `VecDeque`, `BinaryHeap`, `array::from_fn`, `error::Error`) |
| Dependencies | `postcard`, `lz4_flex`, `xxhash-rust`, `serde` and `indexmap` all support `no_std` plus `alloc`; their features in the workspace manifest currently ask for `std` and must become optional |

**Status (2026-10-08, later): done for the sim stack.** `jane-core`, `jane-schema` (model), `jane-data`, `jane-world` and `jane-sim` all build `no_std` plus `alloc` for `mipsel-sony-psp` (CI job `build-psp`), with a default-on `std` feature so PC and Pi are unchanged. Host proof: 67 `jane-sim` and `jane-world` suites pass, and the world hash fixture and replay goldens did not change. Fixes made: `OnceLock` to `once_cell::race::OnceBox`, `AtomicU64` to a `target_has_atomic = "64"` split (never hashed or saved), `IndexMap` on the FNV hasher. **Not yet proven:** the sim *running* on PSP or PPSSPP (a replay-tape hash against the PC's), and its peak heap and tick time there. Those are the next spikes.

**Size of the job (as estimated before the pass):** `core` is done in principle (about 3 800 lines, one real change). `schema` (19 800 lines), `data`, `world` (24 200) and `sim` (22 100) need the same mechanical pass plus the `AtomicU64`, `OnceLock` and `Arc` fixes above. **Estimate: days, not weeks, and it is the same change Dreamcast and Xbox need.** Gate it with a CI job that builds each float-free crate for `mipsel-sony-psp` with `build-std=core,alloc`, so `std` cannot creep back.

### 13.10 The real sim on PPSSPP: a replay tape (2026-10-08)

`spikes/psp-sim/` links the real `jane-sim`, `jane-world`, `jane-data` and `jane-core` (`no_std`, by path) into a PSP `.prx` (4.09 MB, `EBOOT.PBP` the same), embeds a PC-recorded tape (`tape.jrp`, 15 664 bytes: seed 1, the rusher bot for 3 600 frames, three kills, a hash every 60 ticks) and replays it with `replay::verify_tape`. `spikes/psp-sim/host/` records the tape and prints the PC's numbers. Nothing in `crates/` changed.

**Result: no replay hash yet; the county does not fit in PSP memory.** The PC final hash is `520a733ef4dcf12c` at tick 3 600; the PSP never reaches the replay.

| Measured | PC (x86_64) | PSP (PPSSPP, `-j`) |
| --- | ---: | ---: |
| Tape decode, content hash | `99eb71a3579ff97a` | `99eb71a3579ff97a` (match) |
| The twelve small zones' blueprint hashes (`jane_world::hash`) | | **all twelve match** the PC's |
| Worst small zone, build peak | | 3.3 MB (museum); each under 2 s emulated |
| County build | peak **52.0 MB**, resident after 9.5 MB | **out of memory** in stage `gardens`: a 4 000 000 byte allocation with 46.9 MB live, 6.6 MB free in pieces (largest 3.4 MB) |
| All 13 blueprints resident | 11.7 MB | not reached |
| Replay peak (blueprints, state, scratch, journal) | **23.1 MB** | not reached |

What that means:

- **Determinism holds so far.** Postcard, lz4, xxh3 and all of worldgen for twelve zones give byte-identical hashes on 32-bit MIPS and 64-bit x86, so `usize` width, endianness and iteration order have not bitten yet. The county, the sim step and the state hash are still unproven on the PSP.
- **Memory is the blocker, and it is far over §13.2.** The county's worldgen peak (about 50 MB at 32 bits) does not fit a PSP-2000's 54 MB user partition (what PPSSPP gives), let alone a PSP-1000's 24. The running sim needs 23 MB on PC against the **6 MB** budget. On the county, `stories` to `gardens` add 16 MB live (`perimeters` peaks 37.6 MB). §13.3's diet (packed and chunked county, no resident blueprint copy, compact A\*) is required before the sim can run on any console; a spike that only proves the replay hash could instead load baked blueprints (needs `serde` on `jane_core::Blueprint` and its parts).
- **Allocator:** rust-psp's own allocator takes one kernel block per allocation; a kernel-block-per-allocation run crashed PPSSPP (segfault, likely the block table) part way through the county. The spike uses `psp` with `stub-only` and its own allocator: a 3 MB `talc` pool for allocations under 64 KB, a kernel block each for bigger ones. One whole-partition `talc` heap failed at the same allocation. A peak counter wraps both (requested bytes, not overhead).
- **Speed (a hint only; PPSSPP's clock is not a PSP's):** wall time under 2 s for the whole run with the JIT (`-j`); without `-j` the default core is far slower (the first run sat 10 minutes in the county). Emulated time for the twelve small zones together is about 8 s, so a full New Game build would be tens of seconds on the device as built.
- **Workarounds:** `stub-only` drops rust-psp's `memcpy` family, so `.cargo/config.toml` sets `build-std-features = ["compiler-builtins-mem"]`; it also drops `module!` (which calls the missing `catch_unwind`), so the module header is spelled out in `main.rs`; its panic handler is a spin loop, so a panic is a `TIMEOUT` after the last `SIM` line (the `Cargo.toml` notes how to patch a local rust-psp copy to print it, as used for the numbers above).

Reproduce (Windows, from the repo root; target dirs outside the repo):

```
CARGO_TARGET_DIR=C:/Users/kille/tools/jane-psp-target/host cargo build --release --manifest-path spikes/psp-sim/host/Cargo.toml
C:/Users/kille/tools/jane-psp-target/host/release/psp-sim-host.exe record spikes/psp-sim/tape.jrp 3600 1   # only to re-record
C:/Users/kille/tools/jane-psp-target/host/release/psp-sim-host.exe verify spikes/psp-sim/tape.jrp          # PC hashes and peaks
cd spikes/psp-sim && CARGO_TARGET_DIR=C:/Users/kille/tools/jane-psp-target/sim cargo +nightly psp --release
PPSSPPHeadless.exe C:/Users/kille/tools/jane-psp-target/sim/mipsel-sony-psp/release/psp-sim.prx -j --timeout=300
```

The tape is tied to the content hash; a content change makes `Tape::decode` refuse it, so re-record then.


**Update 2026-10-08, after memory diet phase 1 (13.3): the replay hash matches on the PSP.** PPSSPP headless (`-j`, `--graphics=software`) builds all 13 zones for seed 1 (build peak **14.7 MB**, was out of memory at 46.9 MB), replays the 3 600-tick tape with all 60 checkpoint hashes checked, and ends on `520a733ef4dcf12c`, **identical to x86_64**. Replay peak 19.6 MB. Emulated time: county build about 39 s, the whole build about 55 s, the 3 600-tick replay about 5.6 s (about 1.5 ms a tick); an emulator's clock is a hint, not a PSP measurement. The spike then draws the county to the 480 x 272 framebuffer (whole county priority-sampled on the left, a 1:1 crop of the busiest town on the right); `--screenshot-save=<png>` captures it (`progress/2026-10-08_52_psp-county.png`, local only).

### 13.11 The PSP renderer and the `Frame` (survey, 2026-10-08)

**Can `jane-present` build `no_std` plus `alloc`?** Yes, by the same mechanical pass §13.9 made on the sim, with three real items. It is 30 500 lines; its `std::` uses are `core`/`alloc` re-exports (`mem`, `fmt::Write`, `BTreeSet`, `VecDeque`, `cmp`, `Arc`, `cell::Cell`, `str`) bar one `OnceLock` (`fx.rs`, to `once_cell::race` as the sim did), a `println!` and an `eprintln!` (`present.rs`, `cues.rs`), and `build.rs` (host side, fine). No `HashMap`, no `Instant`. Floats: 54 `f32`/`f64` in two files only, `input.rs` (stick and pointer positions, aim thresholds; `.round()`, `.abs()`) and `audio.rs` (gains, a pan; `.sin()`, `.cos()`, `.sqrt()`); `core` lacks those methods without `std`, so `libm` or integer rewrites, and the PSP's FPU is single precision, so the 12 `f64`s go. Dependencies: `jane-core`, `-data`, `-sim`, `-world` (already `no_std`) and **`jane-art`**, which is float-free but not yet `no_std` (the same pass again).

**The three real items:**

1. **Boot renders the art.** `Present::new` builds the atlas by running the generators (`people.rs`, `props.rs`, `creatures.rs`, `terrain.rs`, `ui/art.rs`, `ambient.rs`, `cues.rs`, `stand_in.rs`), interning a `RefId` per frame in its own order and deriving per-sprite facts from the four layers (`SpriteRef::height`, `top`, `base`, `burn`, flora sway classes). On a console nothing is generated (§13.4), so the presenter needs an atlas *source*: rendered (PC, as now) or loaded from a pack. The pack must then be keyed the way the presenter asks (its `RefId` order), and carry those derived facts.
2. **Terrain is painted at run time from the seed** (`chunks.rs` over `terrain::Painter`, 256 x 256 px a chunk, `u32` albedo). It cannot be baked seed-free, and a tile bank (what the bake holds now) loses the painter's work across cells. Either the painter runs on the PSP (integer, so it ports; its output is all master colours, so a chunk can be `T8` through a by-area CLUT at 64 KB) or the rule "nothing generated on a console" gains this exception. **Decided 2026-10-08: the painter runs on the PSP** (§13.7); `jane-art` builds `no_std` for it.
3. **`SpriteCmd` names a PC page and rect** (`page: u8`, `src`), resolved by the presenter against the PC atlas's 2048 pages, and the T0 passes assume the PC's layers (silhouettes from albedo masks, the glow list).

**Recommendation: the same `Frame`; the PSP is a fourth backend (`C2`, below `soft`).** A `Frame` holds no texture or format (§1.1 of PRESENTATION), and every pass a C2 backend cannot draw has a `Features` row below it already (no normals, no T2 post; silhouettes and the lightmap are drawable on the GE as sheared quads and a multiply blend). What has to change is upstream of the `Frame`, not the contract: the presenter takes its atlas from the pack, so `SpriteCmd::{page, src}` already name the PSP's pages and trimmed rects, and a C2 `Features` row set leaves out what C2 cuts. A reduced `Frame` would fork the presenter, the one thing §13.5 forbids. Concretely, next: (a) the bake records the presenter's own atlas (each `RefId` with its `SpriteRef`) rather than walking the generators beside it, so the two cannot drift; (b) `jane-present` gains an `Atlas::from_pack`, a `no_std` build behind a default `std` feature, and C2 rows; (c) a `jane-render-psp` backend in a spike workspace, as §13.10's.

**Done (2026-10-08), each its own commit, PC output unchanged (every jane-art, jane-present and jane-render-soft test passes as it was):**

- **`jane-art` builds `no_std` plus `alloc`** for `mipsel-sony-psp`, the whole crate (the terrain painter and every generator; none needed `std` past `core` and `alloc` paths and the prelude's `Vec`, `String`, `vec!`, `format!`). Still float-free.
- **`jane-present` builds `no_std` plus `alloc`.** `fx`'s `OnceLock` is `once_cell::race::OnceBox`; the `println!` and `eprintln!` were in tests. **The floats stay `std`-only, with integer forms beside them**, because rewriting them would change what the PC does at the edges: the mouse is in fractional canvas px when the window's scale is not whole (`Fit`, `to_canvas`, `reticle_at`, `canvas_to_world`: PC only), the PC mixer takes `f32` gains and pans (`Volumes::gains`, `place`), and the cue table measured its distances in `f32` cells (`hypot`, `sqrt`, `round`, `sin`, `cos`, which `core` lacks). Without `std`: canvas px are `i32` (`input::Px`), stick tilt is Q15, the stick and aim deadzones squared integers, the move vector through `iatan2` and `isqrt`; distances are compared squared in Fx, gains and pans are Q12, the fire's level by `isqrt` in half cells, the owl's bearing by the sim's sine table. No float on the `no_std` path; no float added.
- **(a) and (b):** `Atlas::to_pack` and `Atlas::from_pack` (`JAT1`), the bake writes `presenter.jat` and packs the PSP's sprites from it with the drift check (§13.4); `Features::c2()` (PRESENTATION §1.12). Both done since: the module tables (`JPT1`) and `jane-render-psp` with `spikes/psp-game`, §13.12.

### 13.12 First playable PSP build (2026-10-08)

**What runs.** `spikes/psp-game` (its own workspace, nightly, like the other spikes) on PPSSPP set to the **PSP-1000** (24 MB user partition): it boots, reads the presenter's tables (`present.jpt`) and the PSP pack's tables (`jane-psp.jpk`, its pages stay in the file) from beside the program (`argv[0]`'s directory, else `host0:/`, else `ms0:/PSP/GAME/jane/`), builds all thirteen zones from seed 1 and packs each blueprint as it lands (§13.3; or reads them back from the stick's blueprint cache, §13.13), starts New Game, travels her to the town square on the first tick (New Game wakes her at the farm on the county's west edge, a thousand cells from Castle; a dev travel, said plainly), and then runs: the pad (analog stick or d-pad to walk, Cross to use, R to sprint) to the sim at its 60 ticks a second, `Present::tick` per tick, `Present::draw` per frame at 480 x 272, `jane-render-psp` on the GE at the vblank. Every two seconds it prints fps, ticks a second, each part's microseconds, quads, page loads, live and peak heap and the kernel's free memory to fd 1. A `script.txt` beside it (`ticks [hour [effects]]`) walks her along a fixed path through the town instead of the pad and exits at that tick (the headless screenshot); `effects` is `jane_render_psp::list::fx` bits, to measure each.

**How it is built (all of it in this branch):**

- `Present::tables` / `Present::from_tables` (`JPT1`, `jane-present/src/tables.rs`): the atlas's sprite table without px and each module's table (looks' frames, glass rows, flora kinds and bends, the sky's and cues' sprites, critters, the UI page's glyph metrics); the presenter boots running no generator but the terrain painter's own. `jane bake` writes `present.jpt` (368 KB). Tests hold its frames to the generators' presenter's.
- `Present::from_tables_console(tier, tables, slots)`: a fixed chunk-slot count, no casting band in the paint-ahead, each chunk's albedo **`T8` over a CLUT of its own** (`Frame::t8`, `ChunkLayers::new_t8`: a chunk draws 30 to 140 colours, so lossless), the painter's flora px let go (`Painter::release_flora_px`: `Placed` reads only sizes), lists reserving a 480 x 272 view's worth. Presenter 15.4 -> 5.4 MB (12 slots). A test holds its shown chunks and sprites to the PC's.
- **Deferred chunk painting** (`set_deferred_paint`, `take_paint_job`, `land`; `terrain::PaintJob` owns the painter and a snapshot of the zone round the chunk): a chunk takes the painter about 100 ms on the PSP, so the spike runs it on a thread below the game's priority in the time the game waits for the vblank and the GE. Without it every new chunk was a 100 to 400 ms hitch.
- `against_walls` keeps its answers while no chunk is painted (it was 70% of a tick on the PSP; the same answers on PC).
- `jane-render-psp` (`no_std` plus `alloc`; `ge.rs` the one unsafe module, PSP only): `Lister` turns the `Frame` into quads (sprites resolved from the presenter's atlas rects to the pack's trimmed `T8` rects, mirrored, bent rows as strips, clipped; chunks drawn where they lie as `T8`; the terrain laid back over a sprite whose feet it hides; the lighting below); `cache` decides RAM pages (an LRU under a byte budget, loaded from the file on a miss) and VRAM slots. In CI's `build-psp` matrix.
- **The lighting is kept** (owner, 2026-10-08, §13.5, §13.7): the bake (`JPK2` version 3) ships each page's normals as a `T4` page of 16 directions (`normals::quantize`, one quantiser for bake and GE) and a glow CLUT per page (an entry glows when every texel of it on the page glows the one colour). On the GE: **relief** (the normal page through a per-frame sun CLUT, doubled multiply; subtle, as PC T2's is on sprites); **the sun's silhouettes** (`Features::c2` `silhouettes` on; `jane_present::shadow`'s bands for casters, rows kept between frames; the terrain's blocks as one swept polygon each; laid with the stencil: cleared, raised terrain marked from the chunks' height layers, strongest first, so each px is shaded once, as `soft`'s mask); **the lamps and their shadows** (`soft`'s quarter-size lightmap drawn on the GE into its own 128 x 128 target: cleared to the ambient, each pool a soft disc of `soft`'s falloff; for the two lamps the presenter lets cast, their shadows (casters' slabs in bands of eight rows, blocks' sides turned from the lamp) marked in the target's stencil and the pool added outside them, its bounce inside; the frame multiplied by the target's colour, stretched bilinear); **glow** (each page again through its glow CLUT, added over the light; the chunks' lit-window px as runs) and a **halo** over each light (T0's bloom, gathered); the **grade** as a doubled multiply (exposure and tint) and a lift toward the darks (`src * (1 - dst)`).
- Memory on the spike's side: the world before the presenter, the worker thread made at boot (its stack), `opt-level = "s"` (jane-art at 3), big buffers 64-aligned for the GE, `memsize.py` setting `MEMSIZE=1` for a PSP-2000's EBOOT.

**Measured (PPSSPP headless, `-j`, PSP-1000 model; emulated time, a hint, not hardware):**

| | |
| --- | --- |
| Boot | presenter from tables 0.37 s; world 39 s (the county 27 s); first frame paints the view's chunks 3.2 s |
| fps, town by day, walking, every effect on | 52 to 62 (the square, the gardens, past the church); tick 2.5 to 3.9 ms, sim 0.4 to 0.5 ms, the lister 2.3 to 5.7 ms, the GE 0.6 to 1.1 ms; a frame's worst 4 to 9 ms |
| fps, the town at 22:00, lamp shadows on | 57 to 62 at the square, 45 to 60 in the gardens (fences: the lister 7.9 ms, the GE 2.1 ms); with every light casting (below): 58 to 61 at the square (lister 3.5 ms), 56 to 61 walking the gardens, 60 in the Arms (lister 2.2 ms) |
| Effects' cost (the lister, the square and the gardens) | relief about 0.15 ms of GE; silhouettes 1.5 to 2.6 ms (was 4.7 before the polygons); glow and halos under 0.2 ms; the lightmap's pools on the GE (was 3 to 8 ms on the CPU); lamp shadows about 2 ms CPU and up to 0.7 ms GE a casting lamp |
| User memory at boot | 20.7 MB free (the program 4.57 MB, two thread stacks) |
| Heap (requested bytes) | world build peak 15.0 MB; sim after the build 10.5 MB; with the presenter 16.6 MB; running 18.4 to 18.7 MB (peak 18.8); 0.25 MB left at the least |
| Art in RAM | pages 0.95 to 1.6 MB (budget 1.5 MB, albedo and normals); chunks 1.8 MB (12 `T8` slots with their heights); lightmap 64 KB |
| VRAM (2 MB) | two `8888` framebuffers 1.11 MB; the lightmap's target 140 KB (256 x 140 `8888`, half the canvas); the lamp cache 256 KB (16 pools of 128 x 128 `T8`); 8 page slots of 66.5 KB, 0.53 MB; total 2.05 MB of 2.10 (13 page slots before the lamp cache, 9 with the 64 KB target) |

**Run it (Windows, from the repo root; target dirs outside the repo):**

```
cargo build --release -p jane-cli
target/release/jane bake --target psp --out <dir>                      # present.jpt, jane-psp.jpk (20 MB)
cd spikes/psp-game && CARGO_TARGET_DIR=<t> cargo +nightly psp --release   # <t>/mipsel-sony-psp/release/EBOOT.PBP, psp-game.prx
python -I spikes/psp-game/memsize.py <t>/mipsel-sony-psp/release/EBOOT.PBP
# GUI: a folder PSP/GAME/jane/ on PPSSPP's memstick holding EBOOT.PBP, present.jpt and jane-psp.jpk
#      (no script.txt); System > PSP model: PSP-1000 for the 24 MB target.
# Headless: <dir> holding psp-game.prx, present.jpt, jane-psp.jpk and script.txt ("650 10" walks 650 ticks at 10:00)
PPSSPPHeadless.exe <dir>/psp-game.prx --root=<dir> -j --timeout=900 --screenshot-save=<png>
```

PPSSPPHeadless takes no model (it hard-codes the PSP-2000 and ignores `--appendconfig`); the local build at `C:\Users\kille\tools\ppsspp-src` has a one-line patch in `headless/Headless.cpp` (`PPSSPP_PSP1000` set in the environment picks `PSP_MODEL_FAT`). A loose PBP without `MEMSIZE=1` gets the same 24 MB on any model.

**Known gaps:** (the HUD and every screen: done since, §13.13); `Tint::Seen` is half alpha, not the checker; feet behind the terrain are a patch of the terrain laid back over the sprite (good on roofs and fences, coarse at a sprite's edge); lamp shadows soft-edged at a quarter size; a moving caster's lamp shadow is hard-edged; bloom is halos only; silhouettes' edges are not feathered. Page groups are loaded a page at a time as a frame names them (LRU), not a zone's groups at once. **Memory is the risk:** 0.25 MB left on a PSP-1000 in the town; another zone's units or a busier frame may not fit until the sim's diet phase 3 (§13.3, in progress elsewhere) frees its share. PC behaviour is unchanged throughout (the PC presenter, `soft`, gl2 and wgpu draw the same bytes; every console path is behind `from_tables_console`).

**Every light casts (second pass, 2026-10-08).** The console presenter lets every light cast and C2 draws 16. `jane_render_psp::lamps` caches each still light's pool with its static shadows: a light is cached once it stood where it was the frame before and nothing moving holds it; its key is its cell, height and radius; what stands still in its square (blocks, and casters whose sprite is not a unit's or the scene's) is hashed order-free by cell and counted. A slot is (re)built when more or other things stand round it (fewer is the band's culling and keeps it), no sooner than 30 frames after its last build, one build a frame: `jane_present::shadow`'s slabs (casters on coarse rows, blocks' sides) are scanline-filled into a grid of 4, 8 or 16 px cells (by radius) and each texel is the falloff, times the bounce in shadow. The 64 x 64 `T8` textures (4 KB) live in VRAM after the lightmap's target (64 slots, 256 KB; 9 page slots of 66.5 KB left, from 13), uploaded through the uncached mirror; the LRU takes the slot unused longest. Per frame a cached light is one bilinear quad, or its stencil marks of what moves and two quads. Measured: the square 9 lights cached, 9 builds in all, lister 5.9 -> 3.5 ms; the gardens 13 to 18 held, about 50 builds in 420 ticks; the Arms 5 held, 7 builds. Cost: about 0.1 ms a cached light, 0.15 ms a moving caster in a light's reach, a whole light cast each frame (the lantern) 1 to 2 ms among fences. Held at most 18 at once (72 KB), so 256 KB is ample; no RAM is spent but the 4 KB upload buffer and a 6 KB relief CLUT buffer. **Lamp relief:** a sprite a light reaches takes a CLUT toward that light (`normals::clut_lamp`). **Memory after diet phase 3 (merged):** user free at boot 20.6 MB; heap peak 14.5 MB (the world build 12.0 MB); 2.6 to 4.3 MB free in play (the gardens by day the least).

| Scene (PSP-1000 model, every effect on) | fps |
| --- | --- |
| The square, 22:00 | 58 to 61 |
| The gardens walk, 22:00 | 56 to 61 (was 28 to 60 before the cache) |
| The gardens walk, 12:00 | 55 to 60 |
| The Arms (inside), 22:00 | 60 (46 the second after arriving) |

**Shadow resolution and strength (third pass, 2026-10-08; owner: "point-light shadows look faint and low-res").** The lightmap target is half the canvas (2 px cells, 256 x 140 `8888`, declared 256 square, only the rows drawn read), the lamp cache 16 pools of 128 x 128 (texels 2, 4 or 8 px by the light's reach), so a cached shadow's edge is 1 to 2 px soft and a moving caster's is cast at the same 2 px. Two causes of "faint" were not resolution: a crate level with a lamp threw only two edge-on lines (`row_slabs` casts a caster's front and back faces; C2 now adds its sides, `shadow::side_slabs`), and indoors `soft`'s own lamp shadows are faint (the Arms' ambient is high). **Darkness vs PC soft** (same mark, hour and tick, she stands; the light a shadow keeps over the blocks PC shadows): the square 22:00 PC 0.884, C2 0.877 before and 0.874 after, mean error a block 0.0163 -> 0.0141, 92% of PC's shadowed blocks shaded on C2; the Arms 23:00 PC 0.912, C2 0.916 before and after, error 0.0084 -> 0.0073. **Perf:** a carried light (her lantern) casts what moves and its 24 nearest still casters a frame, rows two and four times coarser past half its reach and past it; rows wholly over a light are dropped (no area); a light inside a caster's footprint casts none from it; the sun's bands are kept per caster. The quarry camp at 22:00 went 20 fps (with the half-res target) -> 57 to 60. Sway is not the cost there (measured: on 31.1, off 32.2). **Swatches in the tour** (the blocky "water", shots 04 to 08): an in-zone teleport left the whole view as the stand-in's swatches until the painter's thread caught up (a chunk is about 200 ms of its wall time); a view over half swatches is now painted at once, as a zone's first view is, and while any on-screen chunk waits the spike gives the painter a vblank more (30 fps a moment). **Pell's brazier** is a quest prop, unlit until she lights it: dark at 22:00 on PC too.

| Tour, 22:00 unless said, walking 300 ticks (PSP-1000) | fps (last 2 s; the window before) |
| --- | --- |
| The square | 62 (54) |
| The square, 02:00 | 62 (55) |
| The square, 19:00 | 62 (53) |
| Pell's brazier | 59 (48) |
| Quarry camp | 58 (45; 32 in the tour before) |
| Sallow jetty, 21:00 | 59 (49) |
| lamp_12 | 60 (52) |
| Halt well, 20:00 | 62 (46) |
| The Arms, 23:00 | 61 (60) |
| The church | 61 (60) |
| The house | 63 (60) |
| The burial | 62 (60) |
| The mine | 61 (57) |
| The cellar | 62 (60) |

Rebuilds with 16 slots: the burial and the mine hold all 16 and build 51 and 66 over 300 ticks (one a frame at most), the square 46. lamp_12's 111 ms tick in the tour is the sim's (reported, not touched).

Screenshots, every iteration with a line each: `progress/2026-10-08_53_psp-game/`, the second pass `progress/2026-10-08_54_psp-lights/`, the third `progress/2026-10-08_56_psp-shadow-res/` (local).

**The atmosphere (fourth pass, 2026-10-08).** C2 draws every atmosphere pass a T0 frame holds, as `soft` draws it, in `jane_render_psp::list::atmos` and `grade` (each its own commit; PC output unchanged). Compared against `soft` at the spike's own sim state: `jane sheet scene --psp` plays the spike's start (the travel and the clock on tick 0, then idle ticks, 480 x 272) and prints the state hash the spike prints when done, so both frames are of one state (the townsfolk an earlier comparison lacked on the PSP were the PC sheet's other state, not a PSP fault: at one hash both draw the same units).

- [x] **Grade** as `soft`'s, term for term: the frame read back as `T32` a channel a pass through `soft`'s tables (exposure, T2's shoulder, tint, lift; at dusk one set a 32-column band with the afterglow's multiply and add), each pass writing its own channel alone (the pixel mask); the **saturation** a mix with the frame's luma, summed at half size in the lightmap's target from three channel CLUTs (`src * (1 - s) + dst * s` down, a reverse subtract and a doubled multiply up); the dusk's **far pull** a smooth strip. Mean RGB, the square 22:00: PSP 91/83/99, soft 90/82/99, wgpu 94/83/100 (was 99/91/103: brighter and less blue); 19:00 97/81/97 against 97/81/97; 18:00 119/86/88 against 122/87/89. 3 KB of CLUTs (48 KB at dusk), no VRAM; three to five full-canvas GE passes.
- [x] **Water**: `soft`'s shimmer, a 2-px glint a cell at its places and strengths (checked px for px). Reflections stay T2's (and T1's); C2 has no surface layer, so no puddles (the `wet` row is off, as on T0).
- [x] **Weather**: the rain, a storm's and their splashes and ripples are particles (below); the flash, the darker and cooler light and the greyer grade come in the frame's ambient and `Post`. Rain at the square 22:00 67/71/94 against soft 67/71/95, a storm 66/69/94 against 66/70/95. `script.txt` takes `clear`, `mist`, `rain` or `storm` (`script_weather` in the spike: the sky held as `jane sheet scene --weather` holds it).
- [x] **Fog**: one drift of the mist tile at the strongest volume (`T8` through a per-frame CLUT of `soft`'s weights, repeating), faded over the volume's edge by its vertices' alpha; the reeds at 06:00 in mist 119/115/101 against soft 120/116/102 (107/105/93 without). `Ge::set_mist` takes the presenter's tile once: 64 KB RAM.
- [x] **Sky and far things**: the backdrop's rows past the zone's top edge as smooth strips (`sky_at`, a column every 32 px), its stars, the far landmark and the treeline cut at the horizon. In the county the camera keeps to the zone, so as on T0 it shows only where a view passes an edge (a test holds it); in water it is T2's.
- [x] **Particles**: a streak a smooth-shaded GE line from its head's alpha to clear, a ring twelve lines squashed to half height, a dot a quad, a glow a soft disc at `soft`'s falloff (4 KB). The fire's sparks and smoke, the reeds' motes, the cues' glints checked against soft.
- Not done: the moon (T0 does not draw it either); T2's reflections, puddles and light shafts; the fog's `top` (T0 ignores it too). The GE's state is now reset at the end of each list (pixel mask, blend, stencil, wrap) and the last texture's filter undone before the next is bound (a bilinear texture after another had drawn nearest).
- Cost: no VRAM; RAM 64 KB (mist) + 4 KB (spot) + 3 to 48 KB (CLUTs) + 1 KB (fog CLUT). Free in play 3.1 to 4.7 MB. The lister and the GE's CPU time moved under 0.1 ms; the GE's fill (PPSSPP does not time it) is three to five full-canvas passes for the grade, a canvas for the fog when there is one. Each pass can be left out (`Lister::atmos_off`, `atmos_fx` bits) for a bench or the degrade ladder; no scene needed it.

| Tour, walking 300 ticks (PSP-1000), last 2 s (the window before) | Clear | Rain |
| --- | --- | --- |
| The square 02:00 / 19:00 / 22:00 | 62 / 62 / 62 | 61 / 62 / 62 |
| Pell's brazier | 59 (48) | 59 (48) |
| Quarry camp | 58 (47) | 60 (41) |
| Sallow jetty 21:00 | 59 (50) | 59 (50) |
| lamp_12 | 60 (53) | 58 (51) |
| Halt well 20:00 | 62 (46) | 62 (47) |
| The Arms, church, house, burial, mine, cellar | 61, 61, 63, 62, 61, 62 | the same (indoors) |

Screenshots: `progress/2026-10-08_58_psp-atmos/` (local), a line each in its README.

**The higher tiers' water (fifth pass, 2026-10-08; owner: "the PC's higher-tier water on the PSP too").** As close to T1 (`jane-render-gl2`'s compose) as the GE goes, compared against gl2 at one state hash (`jane sheet scene --psp --backend gl2`); `jane_render_psp::list::water`, `list::shafts`, `water`.

- **What the ground is:** a console chunk has no surface layer, so its CLUT's alpha carries it (`frame::T8_WATER`, `T8_WET`, `T8_SHINE`: water, ground that darkens in rain, ground that shines; its colours split by it), the chunk drawn opaque. The GE marks the water and the wet ground in the stencil through the chunk's own CLUT, inside the water's bounds (or the view in rain), over the light; what is drawn over them (a lily pad, her feet) is unmarked from its own quads. The painter caches its last colours' indices (a chunk's runs are shorter with the marks: without it the square at 19:00 fell to 40 fps).
- [x] **Reflections:** the sky backdrop laid down the screen at T1's `0.62 y`, at T1's share and tint; the far landmark and treeline in it (hazed); T1's crest and trough rows (a 16-px run a row, troughs batched after crests); standing things (people, creatures, the ducks, tall props; not a footprint such as a fountain's basin) mirrored at their foot from their own pages, in two-row bands each shifted by T1's ripple, tinted toward the water. The lake at noon (eel_path_camp) 120 luma against gl2's 126 and soft's 102.
- [x] **Lamp glints:** each lamp's light as fading soft discs run down from under it, strongest where the water begins (T1's Blinn glint toward the 3/4 eye), at night only, on the water, the puddles and the wet ground.
- [x] **Puddles and wet ground in rain:** wet ground darker (T1's `1 - 0.4 wet`, a matt cell's 0.55 of it) with the sky's sheen; puddles where a 256 x 256 noise tile (T1's two value noises, its rim's dither baked in; 64 KB RAM) lies over the rain's edge, on wet ground at its foot, mirroring the sky, the far things and what stands as the water does. The console presenter keeps the ground's wetness in the frame (`Atmos::wet`; T0's `wet` row stays off). The square at noon in rain 95 luma against gl2's 98.
- [x] **Light shafts:** the stencil's shade read back as `T32` and summed at half size from six points up each column along the ray, added over the shade in the sun's light at gl2's strength, while the sun is under 30 degrees in clear air or mist (the presenter's rule for T1's `Rays`, which a console frame does not carry).
- Cost (PSP-1000, standing, the lister and the GE's CPU time; PPSSPP does not time the GE's fill): the lake +1.7 ms and +0.7 ms (the crest rows 1.2 ms of it, 2.7 before the table sine and the batching; 31 fps at the first try); the lamp glints under 0.05 ms; puddles at the square +0.7 and +0.8 ms; shafts under 0.05 ms CPU, six quarter-canvas and one full-canvas GE pass. RAM: 64 KB (noise) + 1 KB; no VRAM. Free in play 2.8 MB at the least. Switches: `atmos_fx::REFLECT`, `STREAKS`, `PUDDLES`, `SHAFTS`; the spike's `atmos=N` script word.
- Not as T1: no refraction of the water's own colour, no depth term (the painted water carries its depth), reflections are of what stands, unlit by the lamps round it (the flat light's share), and the puddles' wet-ground sheen is a display-value add. Two of the four ducks the PC draws at eel_path_camp are not in the console's frame at all (the presenter's, not the water's; open).

| Water and tour (PSP-1000), walking 300 ticks, last 2 s | Clear | Rain |
| --- | --- | --- |
| eel_path_camp 12:00 / 21:00 | 60 / 59 | 60 / 60 |
| lake_statue_mouth 12:00 / 21:00 | 61 / 60 | 60 / 60 |
| reed_end_landing 12:00 / 06:00 | 62 / 63 | 62 / 62 |
| sallow_jetty 21:00 | 59 | 60 |
| The tour (14 scenes) | 58 to 63 | 56 to 62 (lamp_12 56) |

Screenshots: `progress/2026-10-08_60_psp-water/` (local), a line each in its README.

### 13.13 PSP feature-complete (checklist, 2026-10-08)

The PSP plays the whole game the PC plays (§13.6): every screen, every verb on its pad, the real game flow and the same saves. Owners: **UI** (this round's UI and flow agent), **atmos** (the atmosphere agent: sky, landmarks, water, particles, fog, weather, grade), **audio** (the audio agent), **later** (a later round). Status: done, part, open.

| Feature (PC) | PSP | Owner | Status |
| --- | --- | --- | --- |
| Title (backdrop, New Game, Continue, Load, Controls, Quit) | the PC's title, `TitleInfo::console`: no Host or Join, no name field (no keyboard; she is Jane; the PSP's on-screen keyboard: later) | UI | done |
| New Game | a clock seed (re-rolled while unproven, as the PC), the county built on a thread while the loading screen tells it; she wakes at the farm as on PC; the dev travel only behind a script | UI | done |
| Continue, Load | the slot's county read back from the stick's blueprint cache (or rebuilt) from its note's seed, the save laid over it (`Sim::from_save_with`) | UI | done |
| Loading screen | the PC's train window and lines, compact layout; the lantern follows the build's stages (about 60 s emulated); a Load whose seed is kept says "Reading the county" (about 0.5 s) | UI | done |
| Blueprint cache (zones kept on the stick, below) | `ms0:/PSP/SAVEDATA/JANE00001/cache/<seed>-<zone>.bp`, the last four seeds | perf | done |
| Saves (beds, fires; Save in reach of rest; the rest's autosave; overwrite asks) | `ms0:/PSP/SAVEDATA/JANE00001/slotN.jane`, the sim's bytes as the PC writes them, and `slotN.txt` (the note: seed, place, clock, quest, tracker, the map's ink and pins); temp file then rename | UI | done (sceIo; the savedata utility's icon and dialog: later) |
| Options (Controls) | a console page: the pad map from the bindings' pad column as the PSP plays it, the three volumes, Back | UI (volumes heard: audio) | done |
| Display page (`Features` rows) | not shown: C2's rows are fixed | later | open |
| HUD: vitals, chips, target frame, sky plate, tracker, bar, prompt, toasts, banner, death veil | the PC's, compact plates and tracker on 480 x 272; the PSP's buttons drawn on the bar and the prompt | UI | done (death veil and chips not seen on the PSP yet) |
| Tracker's way and bearing lines, named places' banners | off on the console (`ViewBuffers::no_ways`): the roads are 1 MB and a walk a 16 MB grid over the county (`jane_sim::route::Roads::walk`), which hung the first PSP play | later (a bounded or sparse walk in `jane-sim`) | open |
| Quest marks over heads, emotes, the fight ring, cast bars | the PC's (`ui::marks`, `ui::fight`) | UI | done |
| Action bar 1 to 5 | ×, □, △, d-pad up, d-pad down | UI | done |
| Action bar 6 to 8 | no button (as the PC's pad): cast from the bag's spell list or moved down | later | open |
| Targeting (Tab, Shift-Tab), soft target | d-pad right and left cycle the foes in front; the soft target as PC | UI | done |
| Free aim (Ctrl, right stick) | no right stick: casts go to the target or along her facing (`AssistProfile::Pad`) | later | open |
| Click to select, click to move | no pointer: none | — | n/a |
| Casting, melee auto-attack | the sim's, unchanged; the cast bar is the PC's | UI | done |
| Use, talk, push and pull (held) | ○ (held: push) | UI | done |
| Sprint, hop | R, L | UI | done |
| Bag, stores, crafting (drag and drop) | SELECT opens the bag; the d-pad walks the slots, × picks up and puts down, × twice opens Use / Put on the bar / Destroy; □ and △ put across at a cupboard (compact two panels); the bench strip beside the grid | UI | done (cupboard and bench not seen on the PSP yet) |
| Book, quest log, map | L and R step the window's tabs; × puts the lit spell on the bar, tracks the lit quest; the map's chart at 256 px a side on a console (8 cells a px; 1 MB less) and let go when the map closes | UI | done (map zoom and pan hints still name the wheel and the mouse) |
| Dialogue and choices | d-pad and ×, ○ to leave | UI | done |
| Pause (Resume, Save, Load, Controls, Quit to Title) | START; START first lets go of a target, as Esc | UI | done |
| Death and waking | the sim's; the veil as PC | UI | done |
| Lessons (a spell learned, a jar, a page) | the PC's cards | UI | done |
| Quick save, quick load (F5, F9) | keys only on PC; the pause menu's Save and Load | — | n/a |
| Console, debug overlays, speed keys | dev keys: none on the PSP | — | n/a |
| Co-op (4 seats, ad hoc) | not yet: `jane-net` needs a PSP transport | later | open |
| Music and effects | | audio | open (audio agent) |
| Sky, landmarks, water, particles, fog, weather, colour grade | `soft`'s, and T1's water: reflections, lamp glints, puddles and wet ground, light shafts (§13.12, the fourth and fifth passes) | atmos | done |

**The pad (decided here, shown on the Controls page).** The PSP has one stick and twelve buttons; the bindings' pad column names the PC's standard mapping, so the console routes each PSP button onto it and draws each hint as the PSP button (`PadStyle::Psp`), one table for both (`jane-present` `input::PadStyle`, `spikes/psp-game` `shell::route`):

| PSP | In play | In a screen |
| --- | --- | --- |
| Stick | walk | step (one a lean) |
| × (A) | bar 1 | choose |
| ○ (B) | use, talk; held, push | back |
| □ (X) | bar 2 | put across (a cupboard) |
| △ (Y) | bar 3 | put all away |
| L (LT) | hop | tab left (LB) |
| R (RT) | sprint | tab right (RB) |
| d-pad ← → (LB, RB) | the foe before, the next foe | step |
| d-pad ↑ ↓ (LS, RS) | bar 4, bar 5 | step |
| SELECT (View) | the bag (then L and R to the book, the log, the map) | close the bag |
| START (Menu) | pause (first lets go of a target) | back |

**The UI on the GE.** The UI page's pictures (`UiArt::rects`: every glyph of the four faces, the sweeps, the marks, the icons at 32 and 16) are `ui` sprites in the PSP pack (category 8, keyed by their place in that list; 1 184 pictures, most the font's and the icons' frames already packed, so 2 new pages, 84 KB). `jane_render_psp::ui` turns the frame's `UiCmd`s into quads: a sprite from its pack page and trimmed rect, an `ink` through an all-white CLUT times the ink's colour, fills flat, the UI's images (`Frame::ui_images`: the title's backdrop, the loading card, the map's chart) as `8888` textures in RAM converted when their generation moves, clips applied in integer px. UI pages share the page cache (RAM LRU, VRAM slots) with the world's; no VRAM is set aside (45 KB was free). Layouts: the presenter's own where they fit 480 x 272, a compact branch (`Ui::compact`, a canvas under 290 px tall, so never on PC) where they did not: the HUD's plates and tracker, the loading lines, the pause menu, the slot cards, the title's rows, the console's Controls page. `jane sheet ui` draws any screen at a console's canvas and pad (`--canvas 480x272 --pad psp`).

**Measured (PSP-1000 model, PPSSPPHeadless, seed 1, 2026-10-08).** Boot to title 0.1 s; the title 56 fps, 11.9 MB of user RAM free; New Game or Load: the county on the builder thread 60 s emulated (25 s before only in the log; now the loading screen draws at 30 fps meanwhile, its lantern following the stages), heap peak 10.3 MB in the build and 15.8 MB as the presenter is made; play 54 to 62 fps by day and at 22:00 (lamp_12, the square, the gardens) with the UI on (its quads 0.2 to 0.4 ms of the CPU, the GE's share under 0.3 ms); **user RAM free in play 1.4 to 1.6 MB** (heap 15.0 to 15.4 MB), with the map open 0.78 MB (the 2.6 to 4.3 MB of §13.12 less the UI's buffers, the view buffers and the GE's UI pages). Free in menus as in play: the world is held, not let go. The presenter's tick: at most 1.1 ms walking the county at night (it was 46 to 111 ms: a view more than half swatches was painted on the tick whenever the painter's thread fell behind a walk; now only a view that leapt, a teleport, is), the sim's step at most 2.4 ms, the view buffers' 0.3 ms; the first tick of a world is 3 s (its first view painted and her place found) and the log keeps each part's worst (`sim_worst`, `tick_worst`, `bufs_worst`). VRAM unchanged: the UI's pages share the page slots; its images (title 512 x 272, loading card, map chart) are `8888` textures in RAM, one copy each (`Ui::move_images`: the frame's px let go once the GE has them). Screenshots: `progress/2026-10-08_57_psp-ui/` (local).

**The blueprint cache (2026-10-08; ARCHITECTURE §9's deferred disk cache, built for the PSP).** A built zone is kept on the stick and read back by Continue, Load, and a New Game on a seed built before, instead of the minute's build. `jane_sim::zone_cache`: one file a zone, `<seed>-<zone>.bp`, so a zone built on demand (§13.3, phase 2 item 1) can be read alone; the packed blueprint (only the console form) as postcard writes its serde form (jane-core's `serde` feature now covers every blueprint type: `Grid`, `Plane`, `Names` and `Rare` by hand, each checked on read so no read can leave its bytes), after a 48-byte header: `JBPC`, the format (`zone_cache::FORMAT`, bumped when a kept type's serde form changes), zone, seed, `jane_world::SOURCE_STAMP` (a hash of the sources a blueprint is a function of: jane-world, jane-core, jane-data, jane-schema, `data/`; made by jane-world's `build.rs`, line ends dropped), the content hash, the blueprint's own hash (`jane_world::hash::hash_packed`: every field as `hash` feeds it, the tiles and paint as their packed planes; `hash` and the world fixture unchanged), the body's length and xxh3. Any field that differs, a body of another length or sum, a body that does not decode or decodes to another zone, or a decoded blueprint of another hash: refused, the zone built and written again. Written as it is encoded (two passes: the first only counts and sums; 16 KB at a time to a temporary file, then a rename), right after each zone is built, so no encoding is held whole; the last four seeds read or built are kept (`seeds.txt`), an older seed's zones removed. The spike's side is `county_cache.rs` (the sceIo store) and one call in `build()`; PC does not use it (PC keeps the grids, which this cache does not hold, and builds in half a second).

| PSP-1000, PPSSPPHeadless, seed 1 | Cold (built, then written) | Warm (read back) |
| --- | ---: | ---: |
| The county's thirteen zones, emulated | 59.9 to 61.1 s | **0.52 to 0.58 s** |
| Heap peak while it runs (requested bytes, with the title's 1.5 MB live) | 10.3 MB | **7.1 MB** |
| Live after (the blueprints) | 5.04 MB | 5.03 MB |
| State hash after 60 ticks, New Game / Continue of slot 1 | `cf7cb73231108d0d` / `d2023075969a987e` | the same |

On the stick: **2.76 MB a seed** (the county 2.51 MB; four seeds 11 MB). The read holds one zone's file beside the blueprint it decodes (a plane's bytes read at exactly their length, every list shrunk to fit), so it peaks under the build. The program grew 138 KB (5.36 -> 5.50 MB `.prx`: the serde code for the blueprint's types), out of play's free RAM. Tests: `jane-sim/tests/zone_cache.rs` (every zone read back equal to the build, hash for hash, seed 1 fast and seeds 2 to 4 slow; every header field, the body, a truncation, another zone's and another seed's file refused; a refused zone rebuilt and kept again).

**Scripted runs.** `script.txt` keeps its old words (`ticks [hour [effects]] [zone:mark] [still]`) and gains `press:<button>@t<frame>` or `@p<tick>` (a press of six frames; `t` counts frames outside play, `p` ticks of play), `hold:<button>@p<a>-p<b>`, `shot@t<n>` or `@p<n>` (the screen to `shot-*.bmp` beside the script) `new` (New Game at once; a script with no title presses does this), `seed:N` (New Game's seed), `stick:<degrees>@p<a>-p<b>` (the stick leaned) and `q` for the second play session's ticks (after Quit to Title and a load). Buttons: `cross circle square triangle l r up down left right select start`.

**The name (2026-10-08).** The game is **The Bell at Nine** (John: "Rename it to the bell at nine"). What a player sees changed: the title's lockup (THE over BELL in the Title face, a rule, AT NINE; the old tagline "The bell rings at nine" dropped, since the name says it), the PC window's title (`jane-app` `GAME_NAME`), the PSP's XMB title (`spikes/psp-game/Psp.toml`, cargo-psp's PARAM.SFO `TITLE`; `memsize.py` keeps it), and `tools/dist.sh` (`dist/TheBellAtNine.zip` holding `The Bell at Nine/The Bell at Nine.exe` and its README). What did not, on purpose: the heroine (`{name}`, Jane by default), the crates and the repository, every internal id, the save folder (`%APPDATA%\Jane`, `~/Library/Application Support/Jane`, `$XDG_DATA_HOME/jane`) and the Memory Stick's `ms0:/PSP/SAVEDATA/JANE00001/`, so every save made before the rename still loads. The saves are plain files, not the savedata utility's, so there is no savedata display title to change yet; when the utility's dialog comes (the Saves row above), its title is The Bell at Nine and its id stays JANE00001.

### Still open

- Where Host and Join sit on the title screen (`PLAN.md` §10); decided in P8's UI unit.
- Whether the XP app build is attempted at all after P9, or the eight float-free crates and `jane-cli` are the whole XP story.
