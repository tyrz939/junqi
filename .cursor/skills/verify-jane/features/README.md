# Jane verification map

This directory is the maintained source for verifying the live build in `jane/`. Read this index, then the matching feature file. Engine logic is proven headless by `npm test`; these recipes cover what only a browser can show.

## Baseline preconditions

- Launch with `node .cursor/skills/verify-jane/helpers/launch.mjs`.
- Doctor with `node .cursor/skills/verify-jane/helpers/doctor.mjs` and require `ok: true` at `http://127.0.0.1:5188/`.
- Drive only that origin. Never `5173`–`5175`.
- Wipe `jane.save.0`, `jane.save.1`, `jane.save.2` on the verify origin before a New Game recipe.
- The world is a canvas (`#world`). Everything else is DOM under `#ui` with `jq-*` classes: real buttons, real text.
- `window.jane.sim()` is a read-only inspection handle. Read state with it. Never play with it.
- A background tab does not tick (`requestAnimationFrame` throttling). Confirm `jane.sim().state.tick` advances before driving.

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Prefer DOM reads (`.jq-zone-name`, `.jq-toast`, `.jq-term-log`) and `localStorage` over canvas coordinates.
- For a reproducible world: open the console and run `seed 20260920`.
- Console cheats set a scene. The claim must come from a key, a click or a drag.
- Close the console before sending world keys. While its input has focus the game ignores the keyboard.

## Features

| File | Covers |
| --- | --- |
| `title-new-run.md` | Title screen, New Game, Continue, Controls, the starting kit |
| `console.md` | Backtick console, rows, determinism checks (`hash`, `replay verify`) |
| `save-slot.md` | F5 / F9, the three slots, the save file shape, load from title |
| `bags.md` | The tabbed window: inventory, craft row, spellbook, quests, map, drag and drop |
| `zone-travel.md` | Doors by named mark, the locked house door, interiors, fog, lighting |
