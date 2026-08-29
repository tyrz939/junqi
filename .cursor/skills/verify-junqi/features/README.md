# Junqi verification map

This directory is the maintained source for verifying the TypeScript remake. Read this index, then the matching feature file.

## Baseline preconditions

- Launch with `node .cursor/skills/verify-junqi/helpers/launch.mjs`.
- Doctor with `node .cursor/skills/verify-junqi/helpers/doctor.mjs` and require `ok: true` at `http://127.0.0.1:5188/`.
- Drive only that origin. Never `5173`–`5175`.
- Wipe `localStorage.junqi.slot0` on the verify origin before a New Game recipe.
- World is a Phaser canvas. Bags, title buttons, Jane, and the dog have no ARIA names.
- The HTML console (`#term`, `#console-log`) is the stable handle for typed commands.
- Close the console before sending canvas keys (I, F5, WASD, Esc).

## Driving conventions

- Start every recipe from the baseline unless its preconditions say otherwise.
- Prefer `#term`, `#console`, `#console-log`, and `localStorage.junqi.slot0` over canvas coordinates.
- Treat console command names as literal (`whoami`, `inst count`, `tp house`, `save`).
- After Enter on a wiped title, wait for play. County generation is slow. `inst count` saying `no world (title)` means you are still on title.
- Restore nothing in the developer's slot. This origin is disposable. Do not delete proof files.

## Proof and skip reporting

- Canvas claims need a screenshot that shows JUNQI, Jane, Bag tabs, or a zone toast.
- Console claims need `#console-log` text saved to `evidence/<id>/console.txt`.
- Save claims need parsed `junqi.slot0` in `evidence/<id>/slot.json`.
- Record the feature id and entry point in `evidence/<id>/notes.md`.
- An unreachable entry point is a fail for that entry, not a pass via another path.

## Feature entry contract

Each feature file starts with an H1 and one paragraph. Then exactly four H2s: `Sub-features`, `How to get to it (user POV)`, `Driving it with the browser`, `Gotchas`.

## Features

- [Title and new run](./title-new-run.md) — wipe slot, title, Enter / New game, Jane on Castle.
- [Console](./console.md) — backtick overlay, whoami, status, inst, uname.
- [Save slot 0](./save-slot.md) — F5 or `save`, then Continue.
- [Bags](./bags.md) — I / Tab sheet, Bag tab, starting letter.
- [Zone travel](./zone-travel.md) — `tp house`, kitchen title, whoami.
