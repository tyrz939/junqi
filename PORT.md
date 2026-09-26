# Jane: Port

The plan for the native rewrite. Pair with `PLAN.md` (the game), `ARCHITECTURE.md` (the engine), `ART.md` and `PRESENTATION.md` (how it looks and is driven), `WORLD.md` (the living county), `EXPERIENCE.md` (what happens, minute by minute) and `VERIFICATION.md` (how every claim is proven). This file owns: repo layout, toolchain, targets, the crate map, phases and gates, agent parallelisation, the test strategy, the worldgen port, the data pipeline, and the docs list.

**Rule:** this file is the agreement for the port, as `PLAN.md` is for the game. Work that drifts from it fixes the work or this file first.

**Rule:** no content on an untested engine. Every phase ends in something that runs and a gate a test or a number decides. `archive/phaser-remake-2026/POSTMORTEM.md` is why.

**Rule:** not a 1:1 port. Algorithms and content contracts carry. Architecture is fixed where the audit found it wrong. Seeds do not match the TypeScript and nothing is kept compatible with it.

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
/crates/jane-*/            the fourteen crates (§4)
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
/tests/fixtures/           replays (.jrp), hash files
/tools/                    ci scripts, toolchain notes (rust9x, win7 build-std), pi setup, cross Dockerfiles
/.github/workflows/
/archive/phaser-remake-2026/
/archive/web-remake-2026/  jane/ moved here in P10; POSTMORTEM.md written in P0
/*.md                      root docs (§2.2)
```

**Rule:** content lives at `/data`, outside every crate. A crate reads it only through `jane-data`'s build script or the `jane-schema` library.

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
| all | `unsafe_code = "forbid"` (one exception, §4), `clippy::all`, pedantic where sane, `rust_2018_idioms`, `missing_debug_implementations` |
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
| `sdl2` | `app` only | Window, input, audio, present; the GL context for `gl2` and the window handle for `wgpu` |
| `glow` | `jane-render-gl2` only | OpenGL 2.1 / GLES 2.0 bindings, no C code, the same crate on the Pi and a 2006 PC |
| `wgpu` | `jane-render-wgpu` only | Vulkan, DX12, Metal and GLES 3 behind one API; the only large dependency in the workspace, and it never touches the eight float-free crates |
| `raw-window-handle` | `app`, the two GPU backends | The window handle contract between SDL2 and the backends |

No `png` crate: `jane-art::sheet` carries its own encoder (uncompressed deflate, ~60 lines; `ART.md` §5), so sheets hash the same on every target and `jane-cli` stays free of C code. Out: tokio, async, rand, glam/nalgebra, bevy, image, crossbeam, anyhow in libs, log/tracing in sim.

### 3.6 CI

| Job | Runner | Does |
| --- | --- | --- |
| `check` | ubuntu | fmt, clippy `-D warnings`, float grep, `cargo check` on 1.85 for the eight float-free crates, `jane check` on `/data` |
| `test-linux` | ubuntu | `cargo test --workspace` (SEEDS=64); `cargo test --profile checked -p jane-world -p jane-sim` (SEEDS=16) |
| `test-windows` | windows | tests for x86_64 msvc; release build i686 msvc |
| `build-i686-linux` | ubuntu | release build; `jane gen --hash` under `linux32` |
| `build-arm` | ubuntu | `cross build --release` aarch64 + armv7; `cross run ... gen --zones all --seeds 1..16 --hash` under qemu-user |
| `determinism` | needs above | diff `hashes-<target>.txt` and `bot-hash-<target>.txt` across every target; fail on any difference |
| `soak` | weekly | SEEDS=1000 skeleton, county 64, every dungeon 1000; seed sheets as artefacts |
| `bench` | self-hosted Pi 4, an ancient PC and a modern PC nightly, when online | `jane bench --json --tier` vs `tools/perf/thresholds.json` per class. Until those runners exist the owner benches by hand before each gate and commits the JSON |
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
| `jane-net` | Lockstep: input frames, commands, join by snapshot, hash compare, stall, timeouts. `std::net` TCP, one thread. Host/guest `Session` over a `Transport` (`ARCHITECTURE.md` §7) | core, sim, postcard | forbid | none |
| `jane-art` | Procedural sprite, tile, font, icon, chrome and weather generators from `looks` rows, each emitting albedo, normal, emissive and height; palettes and ramps; atlas packing; contact sheets with its own PNG encoder (`ART.md`) | core, data | forbid | none (so sheets hash the same on every target) |
| `jane-present` | The scene: builds the backend-agnostic `Frame` (passes and draw lists: terrain, sprite batches, lights and shadow casters, fog volumes, parallax layers, particles, post settings, UI) from `jane_sim::view::View` and presenter state (PRESENTATION.md §1); fx runtime, camera, immediate-mode ui, input mapping, `text` (English expansion of `TextId`, `{name}`, `{place:}`); the `Backend` trait and the `Features` tier table. Reads views and events, never writes state; emits `Command`s only. No SDL, no GPU; headless in tests | core, data, sim (view, events), world (names), art | forbid | allowed, discouraged |
| `jane-render-soft` | T0 backend: CPU rasteriser into a `u32` framebuffer, half or full res, the multiply lightmap, no normals or shadows. Always builds; `jane sheet`, `jane film` and CI draw through it | present | deny, one measured `allow` for the blit loop if the Pentium 4 demands it | allowed |
| `jane-render-gl2` | T1 backend: OpenGL 2.1 / GLES 2.0 through `glow`, GLSL 1.20 / ES 1.00; normal-mapped lights in a fragment shader, hard cast shadows by extruded occluder geometry, fog layers, particles, sharp-bilinear upscale | present, glow, raw-window-handle | deny (the GL calls are `glow`'s safe API) | allowed |
| `jane-render-wgpu` | T2 backend: Vulkan / DX12 / Metal / GLES 3 through `wgpu`; everything T1 plus soft shadows, many lights, bloom, colour grading, water reflection, higher particle caps | present, wgpu, raw-window-handle | forbid | allowed |
| `jane-app` | SDL2 binary: window, input events, audio device, loop, threads, save files, config; probes the GPU, picks the backend and tier (override in `config.json`), and hands the `Frame` to it | everything | forbid | allowed |
| `jane-cli` | `jane gen`, `check`, `bench`, `replay verify|record|diff`, `sheet`, `view`, `serve`, `soak`, `hash`, `save inspect`, `world build`. `jane serve` is the headless lockstep host: no SDL, one status line (tick, seats, hash) | schema, world, sim, net, bot, art, present | forbid | allowed |

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
| **P5** Art | `ART.md` generators, looks, font, chrome, weather; four layers per sprite; 4-frame cycles; `jane sheet` incl. `layers` and `light`; `vary` variants | art, cli | 4-6 per family; layers 1; weather 1 | Every look renders with all four layers; sheets on CI; owner redlines; no sprite without a look and no look unused; ≤ 4 variants per `vary`; every look built into atlases inside its §9.4 row (`ART.md` §5) | new ~6-8k |
| **P6** Scene and the soft backend, playable solo | `PRESENTATION.md` §1 in `jane-present` (the `Frame`, camera, fx, `Features`) and `jane-render-soft`; `jane-app`; `jane film`; New Game builds all 13 zones behind a loading screen that draws the skeleton | present, render-soft, app | scene 2; soft 2; app 1 | The owner plays the first hour at T0; frame ms recorded against §9.4; `jane film` produces the L7 clips; lints hold | ~3.5k → ~4k; soft ~2k; app ~1k |
| **P6b** T1: `gl2`, lights, shadows, atmosphere | `jane-render-gl2`; normal-mapped lighting, hard shadows, fog volumes, parallax, weather, sharp-bilinear upscale; the `Features` ladder; a Pi 4 and an ancient PC on the desk | render-gl2, present | shaders 1; shadows 1; atmosphere 1; Pi bring-up 1 | 60 fps on a Pi 4 and on the ancient PC at 768 x 432, night, town, per §9.4; the same `Frame` draws on T0 and T1 with only `Features`-row differences (a pixel diff of the albedo pass is empty) | new ~3k |
| **P6c** T2: `wgpu` and post | `jane-render-wgpu`; soft shadows, many lights, bloom, grading, water reflection | render-wgpu | 2 | 60 fps at 4K output on a modern PC with headroom; T1 and T2 agree on everything but the T2 rows; owner signs the look off against `ART.md` §3 on the same 24 seeds as the sheets | new ~2.5k |
| **P7** UI and input | `PRESENTATION.md` §3-4 in `jane-present::ui` and `::input`; `bindings.json`; keyboard, mouse, pad; Controls screen; the assist profile flag | present (ui, input), app | 3-4 | Every README console row works; owner plays with a pad and with a mouse, assist on and off; UI has no sim writes except `Command`s | ~3.5k → ~3.5k |
| **P8** Lockstep LAN | `jane-net`; host/join screens; `jane serve` | net, present (ui), cli | protocol+host 1; guest+join 1; UI 1 | Two machines, four seats, an hour without desync; hash every 60 frames; a Pi hosting headless through `jane serve`; stall and rejoin over loopback | new ~1.5k |
| **P9** Targets and perf | Pi 4 aarch64 and armv7 on hardware (Pi 3 recorded); i686 Linux/Windows on an ancient PC; Win 7; XP if it builds; thresholds enforced per class and tier | app, backends, cli | 1 per target; 1 profiling per backend | §9.4 thresholds green in all three classes at their tier; 60 fps by the `Features` ladder, 30 fps is not a pass; RSS under ceiling | fixes |
| **P10** Archive | `jane/` → `archive/web-remake-2026/`; banners; SYSTEMS.md rows re-marked | docs | 1 | `git grep jane/src` in root docs finds only archive notes | 0 |

**Rule:** a phase is not entered until the previous gate is green on CI. Two tracks at once only where the table says (P2 and P3 after P1; P4b beside P5; P5 beside P3/P4; P6b and P6c after P6, in either order).

**Rule:** P4 does not start content. The bot tests are the content until P6; from P4b the `EXPERIENCE.md` claims are the content's specification and grow with every phase.

**Rule:** a degrade flag (a `Features` row: shadows, soft shadows, fog volumes, bilinear light, post, half-res) is a documented tuning row with its tier, never a hidden `cfg`. The gate in P9 says which rows were on in each class.

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

Recorded, not gated: T0 `soft` on the Pentium 4 (60 fps at 384 x 216 or 30 at 768 x 432, the multiply lightmap only) and a Pi 3 at T1 with shadows off. i686 takes the ancient PC column; armv7 takes the Pi 4 column with 20 % slack. Enforced from P9; recorded before.

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
| The look outruns the hardware floor | Every visual feature has a tier row from the day it is designed (`PRESENTATION.md` §1); the ancient PC and the Pi 4 are on the desk from P6b, not P9 |

## 12. Defaults taken

Each is a one-line edit to flip before P0 starts.

| Default | |
| --- | --- |
| SDL2 on the Pi | Distro package; x86 targets use the `sdl2` crate's `bundled` feature |
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
| Procedural audio | Outlined for `PLAN.md` M8 only; a `NullBus` ships until then |
| Font | Stroke-defined glyphs on a 5 x 8 lattice, rasterised at boot at two sizes; title and heading faces from the same strokes (`ART.md` §6) |
| Seats | Coat-only swaps by role (`ART.md` §3) |
| Pi 3 and Pentium 4 | Recorded at T0 / T1 with shadows off, never a gate |

### Still open

- Where Host and Join sit on the title screen (`PLAN.md` §10); decided in P8's UI unit.
- Whether the XP app build is attempted at all after P9, or the eight float-free crates and `jane-cli` are the whole XP story.
