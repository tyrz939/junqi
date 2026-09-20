# Project Jane

*Working title; the game gets its real name later. The folder and package are `jane`.*

A web remake of a 2020 GameMaker ARPG. You play Jane. On her twentieth birthday a letter a hundred years old asks her to a town called Castle, twelve hours away on the Sunday train, to meet an aunt she has never met. She arrives at five, at sunset, with no signal. Julie is not home. The dog is.

She does not start a witch.

This is the second rebuild. The first (Phaser, 2026) is archived in `archive/phaser-remake-2026/` with a post-mortem; its README began "This is a buggy mess" and was right. This one was written from the ground up around an engine that can prove what it does: the simulation is headless and deterministic, and **the test suite plays the game**.

## Run it

Node 18 or newer.

```bash
cd jane
npm install
npm run dev      # open the URL Vite prints, click New Game
npm test         # 97 tests, a few seconds, no browser
npm run build    # type-check + production bundle (~280 kB, no runtime dependencies)
```

Art is source code (palette-character grids rasterised at boot). The repo ships no image, font or audio file.

## The first hour

New Game puts Jane just inside Julie's gate at 17:00. The lamp posts come on at 18:30, three real minutes later.

1. Walk up to the house. Reaching the stoop completes the letter.
2. **E** on the dog. Take the quest. **Space** (or left click) swings. Put the yard skeleton down. A kill before you accept does not count; that is 2020's rule.
3. Back to the dog for the house key. **E** on the door twice: unlock, enter.
4. In the kitchen: read Julie's note, open the pantry chest, stand at the bench, open your bags (**I**), drag dust + water + pansy into the craft row, take the potion. Touch the orb by the stove. You learn Icebolt. Aim with the mouse.
5. The two hatches are the cellar. It has two iron doors and one kind of iron key, rats, roses, and a locked storage room with the wood and iron the mine will want.
6. The dog sends you for rat meat, then to the mine (and teaches Repair), then to the burial.

Things worth knowing: Repair costs what the thing is made of. A pressure plate stays down under a barrel. The blue torches only wake for cold. The small snakes in the burial cannot be fought; they can be fed. If the big snake loses sight of you it starts again. If you die you wake at the last bed or fire you rested at, however far away that is, and any door that locked behind you is open again. The dog is not on the step after nine. A bell tells you when that is.

## Controls

| | Keyboard + mouse | Gamepad (the 2020 layout) |
| --- | --- | --- |
| Move | WASD / arrows, or hold right mouse | Left stick / D-pad |
| Aim | Mouse | Right stick |
| Sprint (spends energy) | Shift | RT |
| Use / talk; hold to push, hold and back away to pull | E or F | B |
| Bar slot 1 | Space, left click, 1 | A |
| Bar slots 2–5 / 6–8 | 2–5 / 6–8 | X, Y, LB, RB |
| Bags / spellbook / quests / map | I or Tab / K / J / M | View |
| Pause, back | Esc | Menu |
| Save / load | F5 / F9, **only within reach of a bed or a fire**. Resting at one (E) saves by itself | |
| Console | `` ` `` — type `help` | |
| Debug overlay / path grid | F2 / F3 | |

Console rows worth knowing: `give apple 5`, `god`, `tp burial entry`, `time 22`, `kill`, `speed 4`, `hash`, `replay verify`. For co-op before there is a network: `open`, `join`, `party`, `leave 2`.

## This repo

| | |
| --- | --- |
| `jane/` | The game. Vite + TypeScript, zero runtime dependencies |
| `PLAN.md` | Where it is going: a ten-minute seeded county, three regions, generated dungeons with fixed challenges, signs that are true a third of the time |
| `PLATFORM.md` | How people will play: browser, installable app, LAN co-op by deterministic lockstep, and what the sim must change first |
| `STORY.md`, `VOICE.md` | What is true in Castle, and how Castle talks. Drafts for review |
| `ENGINE.md` | How it works, and why: fixed step, determinism, the state tree, the scheduler, A\*, the cast pipeline, the solver, the lightmap |
| `SYSTEMS.md` | The production bar. A row is **IN** only when a test names it |
| `WORLDGEN.md` | What 2020 placed, the zone contract, the lock-and-key solver |
| `MISSING-SYSTEMS.md` | 2020 engine not yet carried: carts, rafts, tile-edit spells, audio |
| `LEARNING.md`, `LEARNING-SYSTEMS.md` | What the 2020 GameMaker code actually did, audited against its source |
| `DESIGN-2020.md` | What 2020 *designed*: the story doc, the balance sheet, six dungeon maps, the pad layout, licensing |
| `OLD-NOTES.md` | The 2020–21 Trello boards |
| `archive/phaser-remake-2026/` | The previous attempt and its `POSTMORTEM.md` |
| `.cursor/skills/verify-jane/` | How an agent drives the running game in a browser and leaves proof |

The 2020 GameMaker project and its design folder are not in this repo and never will be: most of its art is purchased packs. If you have it, keep it beside this file as `Junqi-Legacy-GM/`. Git ignores that folder.

## Where it stands

Built and tested: the engine, and the spine 2020 built (county, house, cellar, gold mine, burial chamber) at the opening numbers from 2020's own balance sheet. Presentation (renderer, UI, input) is written and hand-checked but has no automated browser test yet, so `SYSTEMS.md` marks it SHAPE, not IN.

Not built: everything 2020 drew after the mine. The order is on paper (`DESIGN-2020.md` §4.1): **Museum**, Butterfly Forest, the pipes and the Factory, the rest of the Burial Chamber, the School. The Museum key is already in the mine vault.
