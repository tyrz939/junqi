# Zone travel

The player changes zone at doors, or types `tp <zone>` in the console. A new run can reach Julie's kitchen. The toast reads Julie's kitchen. `whoami` becomes `Jane@house`.

## Sub-features

- `travel-tp-house` console-travels to house.
- `travel-toast` shows Julie's kitchen.
- `travel-whoami` prints `Jane@house`.
- `travel-inst` no longer says `no world (title)` after the restart.

## How to get to it (user POV)

- Walk to the house door on the stoop, unlock with the house key, press E. (Key is a quest reward — not on a fresh run.)
- Type `tp house` in the console (documented). Zones: county, house, dungeon, burial, mine, abandoned, museum, graveyard, factory, school, butterfly, pipes.

## Driving it with the browser

Preconditions:

- Doctor reports `ok: true`.
- A new run is on county. `whoami` is `Jane@county`.

- **Door entry.** Skip on a fresh run unless you have already handed in the skeleton and hold the house key. If you do, close console, walk to the stoop door, press E, wait for the scene restart.
- **Console entry.** Open console, run `tp house`. Log contains `tp house`. PlayScene restarts. Wait through generate.
- **Confirm.** `whoami` is `Jane@house`. `status` contains `house`. Close console. Screenshot `evidence/zone-travel/house.png` shows toast or map title Julie's kitchen.
- **Proof.** `evidence/zone-travel/console.txt` and the house screenshot. `notes.md` names `tp house` or the door.

## Gotchas

- `tp` on title prints `tp: no world`.
- `tp farm` is not a ZoneId. Valid kitchen id is `house`. Cellar is `dungeon`.
- Scene restart drops the console closed. Open it again after the load.
- Carry is dropped on travel. Do not hold a crate across this proof.
- Walking the locked door without the key is not a fail of `tp house`.
