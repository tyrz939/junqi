---
name: verify-jane
description: >-
  Drive the live Jane build in a browser (title, world canvas, DOM UI, console,
  save slots) and capture proof. Use when verifying play behaviour, title / new /
  continue, console rows, save slots, bags, dialogue or zone travel. Not for
  reading GML or editing docs. For engine logic, run `npm test` first: the sim is
  headless and the tests play the game.
---

# Verify Jane

The user-facing app is `jane/`: a low-resolution world `<canvas id="world">` with a **DOM** UI in `<div id="ui">` on top. No Phaser. The archived Phaser build in `archive/phaser-remake-2026/` is not this surface, and neither is the GameMaker tree.

**Try `npm test` first.** Anything that is sim logic (combat, quests, keys, triggers, worldgen, saves, replay) is proven headless in `jane/test/` in about three seconds. The browser is for what tests cannot see: that it boots, draws, takes input and that the UI does what it says.

## Surface

| What the player touches | What it is | Drive it how |
| --- | --- | --- |
| World | `#world` canvas, integer-scaled | Keys, mouse, screenshots |
| Title, HUD, windows, dialogue, menus, toasts | DOM under `#ui`, classes `jq-*` | Real clicks and ARIA/DOM reads. `.jq-title`, `.jq-tab`, `.jq-toast`, `.jq-zone-name`, `.jq-speaker` |
| Console | DOM: `.jq-term-input`, `.jq-term-log` | `` ` `` opens it. Type, Enter, read the log |
| Save slots | IndexedDB database `jane`, store `saves`, keys `0` … `2` on **this origin**: `{ meta, data }`, `data` = gzipped save JSON. (`localStorage` `jane.save.N` only as the fallback when IndexedDB is blocked, and as old saves waiting to be migrated) | Read after the "Saved to slot N" toast (the write is asynchronous). `meta` is the summary; the unzipped JSON has `format: "jane-save"`, `version`, `summary`, `state`. Snippet in `features/save-slot.md` |
| Inspection handle | `window.jane` (the `App`) | `jane.sim()` is the live sim: `jane.sim().state`, `.player`, `.zone`, `.rt.unitsByKey`. `jane.debugText()` is the F2 text. **Read with it. Do not play with it** |

Keys (world, when no window is open): WASD / arrows move, Shift sprint, E or F use (hold to push), Space / left click / 1 bar slot 1, 2–8 bar, I or Tab bags, K book, J quests, M map, Esc pause, F5 / F9 quick save / load slot 1, F2 overlay, F3 path grid, `` ` `` console.

Console rows: `help give god tp time hp mp learn quest flag kill spawn save load seed hash pos inst speed replay ver title`.

## Launch

Private instance only. Default verify origin is **port 5188**. Never start on 5173 and never drive `localhost:5173`–`5175`: those are the developer's sessions.

```
node .cursor/skills/verify-jane/helpers/launch.mjs
```

Ready line: `ready pid=<n> url=http://127.0.0.1:5188/`. Deps: `jane/node_modules` must exist (`cd jane && npm install` once). Override the port with `JANE_VERIFY_PORT`, and use the same value for doctor, browser and cleanup.

## Doctor

```
node .cursor/skills/verify-jane/helpers/doctor.mjs
```

Exit 0 means: HTTP 200, `<title>Project Jane</title>`, `id="world"` and `id="ui"` in the HTML, and `.run/pid` is a live process we started. Exit 1 with `oursAlive: false` means **stop**: a page on 5188 without our pid is someone else's. Do not clear their slots.

## Drive

1. Navigate to `http://127.0.0.1:5188/`. The title shows **PROJECT JANE**, "Castle, Sunday.", and a menu: New Game, Continue, Load, Controls.
2. For a clean run wipe this origin first: `indexedDB.deleteDatabase("jane"); for (let i = 0; i < 3; i++) localStorage.removeItem("jane.save." + i); location.reload();`
3. Click **New Game**, type a name (or keep Jane), press Enter. For a reproducible world use the console afterwards: `seed 20260920` starts a new run on that seed.
4. Expect: HUD name **Jane**, 150/150 HP and MP, zone **Castle**, clock **17:00 Day 1**, tracker "A Letter from Julie", bar slot 1 melee, slot 7 apples ×3, slot 8 the letter.

**Background tabs do not tick.** `requestAnimationFrame` is throttled in an unfocused or headless tab, so the sim may sit at tick 0 while screenshots still work. Check `jane.sim().state.tick` twice, a second apart. If it is stuck, bring the tab to the front. Do not "fix" it by calling private methods and then claiming a player path.

**Do not prove a verb with a cheat.** `give`, `god`, `kill`, `tp`, `learn` are console rows for setting a scene. The claim itself must come from the player path: a key, a click, a drag.

**Determinism is checkable in the page:** `replay verify` re-simulates the session from New Game and prints `MATCH` or `DIVERGED`. `hash` prints the state hash. After any console cheat the replay still matches, because cheats are sim commands.

Read the feature file in `features/` for each recipe.

## Evidence

Directory: `.cursor/skills/verify-jane/evidence/<feature-id>/`

- `before.png` / `after.png`
- `console.txt` — `.jq-term-log` innerText
- `slot.json` — the slot's unzipped, parsed save (IndexedDB `jane` / `saves`), when the feature writes one
- `state.json` — the few fields of `jane.sim().state` the claim is about
- `notes.md` — feature id, entry point, URL, seed, timestamp

A screenshot is required for any claim about the world canvas. A DOM read is enough for a claim about the UI.

## Cleanup

```
node .cursor/skills/verify-jane/helpers/cleanup.mjs
```

Kills the pid in `.run/pid`, deletes `.run/`, keeps `evidence/`. Close only the tab you opened.

## Feature map

`.cursor/skills/verify-jane/features/README.md`
