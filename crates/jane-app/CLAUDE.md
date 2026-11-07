# jane-app

Owns the PC game binary: an SDL2 window, the fixed 60-tick loop, devices into `InputFrame`s, the title, loading and play screens, saves on disk, and backend choice. No SDL type leaves this crate: `jane-present` sees a `DeviceState` and hands back a `Frame`. Nothing depends on it.

**Map:** `src/`
- `main.rs` entry and flags; `app.rs` the game as the player meets it (title, loading, play); `game.rs` the window and the backend the probe picks (T2 `wgpu` where an adapter can draw it; see its `//!`)
- `devices.rs` the only file that knows an SDL key or button; `screen.rs`, `handle.rs` the window, the present, the wgpu surface
- `audio.rs` SDL2 audio callback at 48 kHz stereo; `saves.rs` save slots on disk; `config.rs` `config.json`
- `lan.rs` host/join through `jane-net`; `console.rs` the terminal's rows; `script.rs` `--script`; `shot.rs` PNG screenshots (F12, `--shot`); `crash.rs` `crash.log`

**Invariants:**
- Depends on all three render crates and `jane-net`/`jane-bot`; the sim stays unaware of it. Only here do wall time and devices enter, as inputs.
- Screenshots use `jane_art::sheet`'s encoder: no image crate. Workspace `unsafe_code = forbid` applies (`handle.rs` does the wgpu surface with no unsafe).
- `build.rs` runs at build.

**Test it:** no tests of its own; check with `cargo clippy -p jane-app --all-targets -- -D warnings` (build only, but the root gates run clippy workspace-wide) and a headless smoke run with `--script`. Real checks live in the crates it wires: `jane-present`, `jane-render-soft`, `jane-sim`.

**Docs:** `PRESENTATION.md` §1.3, §1.11, §3, §4, §5; `PORT.md` §7.1 step 3; `README.md` "Run the native build".
