---
name: verify-junqi
description: >-
  Drive the Junqi Phaser remake in a browser (title, county, HTML console)
  and capture proof. Use when verifying play behavior, title/new/continue,
  console commands, save slot 0, bags, or zone travel — not for reading GML
  or editing docs.
---

# Verify Junqi

The user-facing app is the TypeScript remake in `junqi/`: Phaser 3 canvas + one HTML console overlay. The GameMaker tree is read-only and is not this surface.

An agent that has never seen the game must launch a **private** Vite instance, refuse anyone else's port, exercise a mapped feature the way a player would, and leave proof on disk.

## Surface

| What the player touches | What it is | Drive it how |
| --- | --- | --- |
| Title, world, HUD, bags, dialogue | Phaser canvas inside `#app` | Keys and screenshots. No ARIA. Snapshot will not see Jane, the dog, or "New game". |
| Console | DOM `#console` / `#term` / `#console-log` | Backtick to open. Fill `#term`. Read log text. This is the only stable programmatic handle. |
| Slot 0 | `localStorage` key `junqi.slot0` on **this origin** | Read after F5 or `save`. Different ports do not share the slot. |

Documented player keys (canvas, only when console is **closed**): WASD, Shift, E, Space, 1–8, I/Tab bags, Esc pause, F5 save, F2 debug, F3 grid, Enter on title.

Documented console (backtick): `give`, `god`, `tp <zone>`, `time`, `mp`, `save`, `status`, `whoami`, `inst count`, `clear_config`.

## Launch

Private instance only. Default verify origin is **port 5188**. Never start on 5173 and never drive `localhost:5173`–`5175` — those are the developer's sessions.

From repo root:

```
node .cursor/skills/verify-junqi/helpers/launch.mjs
```

Ready line: `ready pid=<n> url=http://127.0.0.1:5188/`

That helper starts `junqi/node_modules/vite/bin/vite.js` with `--port 5188 --strictPort --host 127.0.0.1` (direct node, not `npx.cmd` — detached spawn of `.cmd` fails on Windows). Override with `JUNQI_VERIFY_PORT` if 5188 is occupied **and** you will use the same env for doctor, browser, and cleanup.

Deps: `junqi/node_modules` must already exist (`cd junqi && npm install` once). First play start generates a 2000×1200 county — wait; do not treat a black canvas as failure for the first ~10s after Enter.

Teardown is Cleanup below. Leave the server up for the whole drive.

## Doctor

Read-only. Run first whenever anything looks off:

```
node .cursor/skills/verify-junqi/helpers/doctor.mjs
```

Exit 0 means: HTTP 200, `<title>Junqi · Jane</title>`, `#console` and `#term` in the HTML, and `.run/pid` is a live process we started.

Exit 1 and `oursAlive: false` means **stop**. A page on 5188 without our pid is someone else's. Do not clear their `junqi.slot0`. Do not press Enter.

In the browser after doctor: `#app canvas` exists. On a fresh origin with no slot, the canvas shows **JUNQI** and **New game**. After New Game it shows the HUD name **Jane** and a toast **Castle**.

## Drive

Harness: Cursor browser tools (`cursor-ide-browser`) plus CDP `Runtime.evaluate` for DOM and `localStorage`. No Playwright repo harness exists.

**Order:** `browser_navigate` to `http://127.0.0.1:5188/` (new tab) → `browser_lock` → work → `browser_lock` unlock when the feature is done.

**Wipe this origin before New Game** so Enter means new, not continue:

```
localStorage.removeItem("junqi.slot0"); location.reload();
```

Wait until the canvas is present and a screenshot shows `JUNQI` (BootScene paints generated tiles first; title follows).

**Title → play.** After wipe, focus the canvas and press `Enter`. Do not hunt an ARIA "New game" button. If Enter is swallowed by `#term`, click New game with `browser_mouse_click_xy` in **screenshot** pixels (not CSS). Game hit box is (92,268)–(392,318) on 1280×720. On a 1024×576 shot that center is about (194, 234). Synthetic `dispatchEvent` clicks do not reach Phaser.

**Console.** Press `` ` `` (Backquote). `#console` gets class `on` and `aria-hidden="false"`. Focus `#term`. Fill the command, press Enter. Read `#console-log` text (strip is fine; color spans are present). Esc or `` ` `` closes. While open, Phaser keys are disabled — close before WASD / I / F5.

**Observe play.** `whoami` prints `Jane@<zone>`. `status` prints `Jane  <zone>  hp …` and `beat 1/9  Castle, Sunday` on a new run, plus bag lines `julies_letter`, `apple`, `birthday_present`. `inst count` on title prints `no world (title)`; in play it prints `units` / `props`. Save those lines **before** `inst list` — the log cap is 500 and a county dump evicts them. Prefer `find dog` only while those lines are still in the log.

**Do not** prove a county verb by only calling `give` / `god` / `beat`. Those are cheats. `tp` and `save` are documented player console commands and may be the drive for those features. Screenshots prove canvas state; the log proves console state; `localStorage.junqi.slot0` proves save.

Read the feature file in `features/` for the recipe. The map lists every entry point; driving one and ignoring the others is incomplete.

## Evidence

Directory: `.cursor/skills/verify-junqi/evidence/<feature-id>/`

Keep after cleanup:

- `before.png` / `after.png` (or named shots of title vs play, closed vs open console)
- `console.txt` — `#console-log` innerText after the command
- `slot.json` — `localStorage.getItem("junqi.slot0")` parsed, when the feature mutates the slot
- `notes.md` — feature id, entry point used, URL, timestamp

Proof standards:

- Walk the player path (title button / Enter, backtick, I, F5), not `session.newRun()` from a test hook. There is no test hook.
- Capture the action and the resulting state, not only the last frame.
- Side effects: slot JSON after save; `#console` class after backtick; `whoami` after `tp`.
- A screenshot of a Phaser canvas is required for any play/title/bag claim. An ARIA snapshot of `#term` is not enough for those.

## Cleanup

```
node .cursor/skills/verify-junqi/helpers/cleanup.mjs
```

Kills the pid in `.run/pid` (Windows `taskkill /T`). Deletes `.run/`. Does **not** delete `evidence/`. Does not touch other Vite processes.

After unlock, close only the verify tab if you opened one.

## Helpers

All from repo root. `JUNQI_VERIFY_PORT` defaults to 5188.

| Command | Job |
| --- | --- |
| `node .cursor/skills/verify-junqi/helpers/launch.mjs` | Start Vite. Print `ready pid=… url=…` |
| `node .cursor/skills/verify-junqi/helpers/doctor.mjs` | JSON health. Exit 0 only if we own the port |
| `node .cursor/skills/verify-junqi/helpers/cleanup.mjs` | Stop what launch started. Keep evidence |

## Feature map

`.cursor/skills/verify-junqi/features/README.md`
