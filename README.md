# Junqi

This is a buggy mess. The verbs work. The first five minutes do not. New Game starts at 19:00 on a mountain tile, the dog is a 16px blob, and the county is 2000 by 1200 cells. You spawn on the stoop. Talk to the dog first. Do not treat this as a finished game.

A Phaser remake of a 2020 GameMaker ARPG. You play Jane. She arrives in Castle on a Sunday train. She does not start a witch.

The 2020 GameMaker tree is not in this repo. If you still have it, keep it local as `Junqi-Legacy-GM/`. Git ignores that folder.

## Run it

You need Node 18 or newer.

```bash
cd junqi
npm install
npm run dev
```

Open the URL Vite prints. Click **New game**.

The canvas is 1280 by 720. Art is drawn in code. There is no imported GameMaker sprite sheet.

## First hour

New Game puts Jane on Auntie Julie's stoop at 19:00. The dog is next to you. The letter quest completes while you stand there.

1. Press **E** on the dog. Choose **I'll do it**.
2. Kill a skeleton in the yard. **Space** is melee. A kill before you accept the quest does not count.
3. Talk to the dog again. Choose **I'll take it**. That hand-in gives the house key. **Not now** leaves you without it.
4. Press **E** on the house door.
5. Press **E** on the ice orb in the kitchen. You learn Icebolt.

Kitchen hatches go to the cellar. Yard mouths go to the mine and the burial. Stand in the kitchen before you walk into both underworlds. Those enter triggers fire once.

**F5** writes slot 0 to `localStorage` as `junqi.slot0`.

## Controls

These match the title card and `PlayScene`.

| Input | Action |
| --- | --- |
| WASD or arrows | move |
| Right mouse | walk toward the cursor |
| Left mouse | target |
| Shift | sprint. Sprint spends energy |
| E | use, talk, or hold to push a crate |
| Space | melee |
| 1 through 8 | action bar |
| I or Tab | open bags. Arrows cycle inventory, spellbook, quest, and map |
| Q, K, M | quest, spellbook, map while bags are open |
| Esc | close bags, or pause |
| F5 | save slot 0 |
| F3 | path grid |
| F2 | debug overlay |
| ` | console |

You start with melee, Repair, three apples, Julie's letter, and a birthday present.

Icebolt is the kitchen orb. Fireball is the burial garden orb. Grow is the east garden in the butterfly forest.

## After the snake

The county still turns. Factory, school, butterfly forest, and pipes are authored rooms. Each one shows a lock, then teaches one verb.

| Place | You see first | You learn | You come back for |
| --- | --- | --- | --- |
| Factory | A locked line | Lever in the crates, then a lock-in on the floor | Repair the shop wreck. That opens the pipes hatch |
| School | A glass case in the hall | Push desks for the office key. East class is a fight | The office bell opens the case you walked past |
| Butterfly forest | A chest in the water | Grow in the east garden makes a floor | North fight, then the amulet |
| Pipes | A chest in a lake | Drain lever in the dry west | Walk |

Other doors on the map include the house in the trees, the museum, and the graveyard.

Towns are Castle Cross, Rivermill, Acreton, Kiln End, Boneford, and Ridgegate. The county is 2000 by 1200 cells. You spawn at the house. You do not start a map trek.

## Console

Press `` ` ``. Type `help`.

Useful commands include `give apple 5`, `god`, `tp factory`, `time 14`, `save`, `mp`, and `hp`.

## This repo

`junqi/` is the Vite + TypeScript game. Phaser is 3.88.2.

`SYSTEMS.md` is the production bar for the remake.

`WORLDGEN.md` is how the county and the rooms are built.

`LEARNING.md` is 2020 history. That file still uses JoJo. The remake player is Jane.

`MISSING-SYSTEMS.md` lists 2020 contracts that are not in the remake.
