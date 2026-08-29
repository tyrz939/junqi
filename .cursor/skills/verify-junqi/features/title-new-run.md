# Title and new run

Title starts a disposable run. After a wiped slot, New game (or Enter) puts Jane on Castle at Julie's stoop with the Sunday letter already in the bag.

## Sub-features

- `title-paint` shows JUNQI and New game on a wiped origin.
- `title-continue-empty` shows Continue · empty when `junqi.slot0` is missing.
- `title-enter-new` starts play from Enter when there is no slot.
- `title-click-new` starts play from the New game hit box.
- `play-jane` shows Jane on Castle with beat Castle, Sunday.

## How to get to it (user POV)

- Open `http://127.0.0.1:5188/` (verify instance).
- Click New game.
- Press Enter on title when the slot is empty.

## Driving it with the browser

Preconditions:

- Doctor reports `ok: true` at `http://127.0.0.1:5188/`.
- This tab is the verify origin, not 5173.

- **Wipe slot.** Evaluate `localStorage.removeItem("junqi.slot0"); location.reload();`. After reload the canvas is title, not play.
- **Title shot.** Screenshot `evidence/title-new-run/title.png`. The frame reads JUNQI and Jane · Sunday train. Continue reads empty.
- **Open from keyboard.** Press Enter. Wait until `#console` can answer play queries. Do not treat a dark frame in the first seconds as failure.
- **Confirm play.** Open the console with Backquote, run `whoami`, then `status`, then `inst list`. Log contains `Jane@county`, `Jane  county`, `beat 1/9  Castle, Sunday`, and an actor line `dog`.
- **Play shot.** Close the console (Escape). Screenshot `evidence/title-new-run/play.png`. HUD name is Jane. A toast may still say Castle.
- **Click entry (if proving it).** Wipe and reload again. `browser_mouse_click_xy` uses **screenshot** pixels, not CSS. On a FIT-filled canvas the New game center is about (194, 234) when the shot is 1024×576 (game 242,293 on 1280×720). Confirm the tool's viewport mapping before a second try. Same `whoami` result as Enter.
- **Proof.** Write `evidence/title-new-run/console.txt` from `#console-log` and `notes.md` naming the entry used (`Enter` or `New game` click).

## Gotchas

- Enter continues slot 0 if a save exists. Wipe first or you will load last night's run.
- Phaser title buttons have no accessible name. Snapshot will not list New game.
- Synthetic `dispatchEvent` clicks are not trusted; Phaser ignores them. Use the browser click tool in screenshot space, or Enter after focusing the canvas.
- County generate blocks the first play frame. Poll `whoami` until it is `Jane@county`. Do not run `inst list` first — the console keeps 500 lines and a county dump erases whoami.
- 5173 may already show a mid-run Jane. That is not this feature.
