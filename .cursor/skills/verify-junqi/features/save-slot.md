# Save slot 0

Slot 0 is one JSON blob in `localStorage` under `junqi.slot0`. The player writes it with F5 or the console `save` command, then continues from title.

## Sub-features

- `save-f5` writes the slot from play with F5.
- `save-console` writes the slot from `save` and prints `saved`.
- `save-payload` stores zone, seed, player name Jane, and bag ids.
- `save-continue` loads that slot from Continue · slot 0 or Enter on title.

## How to get to it (user POV)

- Press F5 in play with the console closed.
- Type `save` in the console.
- Return to title and choose Continue · slot 0, or press Enter when a slot exists.

## Driving it with the browser

Preconditions:

- Doctor reports `ok: true`.
- A new run is on county (title-new-run). Console is closed for F5.

- **F5 entry.** Press F5. Evaluate `localStorage.getItem("junqi.slot0")`. It is non-null JSON.
- **Console entry.** Open console, run `save`. Log contains `saved`. Slot is still present.
- **Read payload.** Parse the slot. `player.name` is Jane, `zone` is `county`, `quests.active` includes `the_letter`. Write `evidence/save-slot/slot.json`.
- **Continue.** Run `session` is not injectable. Use console `quit` only if you are proving quit; otherwise evaluate `location.reload()` after noting the slot exists, wait for title, screenshot Continue · slot 0, press Enter. `whoami` is `Jane@county` again.
- **Proof.** `evidence/save-slot/title-continue.png` shows Continue · slot 0. `notes.md` names F5 or `save`.

## Gotchas

- F5 does nothing useful while the console has Phaser keys disabled. Close it first.
- `save` on title prints `save: no world`.
- Wiping `junqi.slot0` is required before title-new-run, and destroys this proof's continue path. Save the JSON file before any wipe.
- The developer's 5173 slot is a different origin. Do not read it as this feature.
