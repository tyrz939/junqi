# jane-cli

Owns `jane` (alias `cargo jane`), the headless tool: every subcommand is a function of its arguments and `/data`. Nothing depends on it. It is the glue for gens, hashes, bakes, bots, sheets, benches and the headless host.

**Map:** `src/` (one file per subcommand family; `main.rs` dispatches)
- `gen.rs`, `hash_cmd.rs`, `view.rs` / `view/`, `mapsheet.rs` worldgen runs, state hashes, seed viewer, county review sheet
- `play.rs`, `sweep.rs`, `pair.rs` bots: `play` and `replay` (incl. `--fixture`, `--snap`), the L4 sweep, a co-op pair over lockstep
- `sheet_cmd.rs`, `sheet_terrain.rs`, `scene.rs`, `ui_sheet.rs`, `layers.rs`, `strip.rs`, `snap.rs` contact sheets and headless frames as PNGs
- `bake.rs`, `bake_psp.rs`, `bake_chunks.rs`, `bake_world.rs` canonical art pack, PSP pack, baked chunks, baked county
- `bench*.rs`, `heap.rs`, `mem.rs`, `psp_profile.rs` benches, heap and memory budgets, PSP capture reader
- `serve.rs` the headless lockstep host; `audio_cmd.rs`, `audio_replay.rs` audio sheets and PSP audio replay

**Invariants:**
- Enables `jane-schema`'s `compile` feature (the `check` command). `jane-render-gl2` and `-wgpu` are optional behind the `gpu` feature; it links `jane-render-soft` and `jane-render-psp` always.
- The bot-hash fixture is rewritten by `jane play --fixture tests/fixtures/bot-hash-x86_64.txt`; the gate check-outs the file afterwards (root gates).

**Test it:** `cargo test --release -p jane-cli` (`casters`, `heap_budget`, `mem_budget`, `late_game`, `serve`; ignored by default: `heap_budget`, `late_game`, `mem_budget`, `serve` (three processes over TCP, about 15 s): `-- --ignored`). Run the command you changed directly, for example `cargo run -q --release -p jane-cli -- gen --zones all --hash`.
Dependents: none, but a change to a subcommand that bakes (`bake*`) alters what the PSP loads, so run `bash tools/psp-dist.sh`.

**Docs:** `README.md` "Run the native build"; `VERIFICATION.md` §3.2; `PORT.md` §13.4; `PRESENTATION.md` §6.
