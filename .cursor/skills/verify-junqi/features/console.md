# Console

The backtick console is a real HTML overlay. The player types world and shell commands, reads a log, and closes with Escape. Phaser movement is off while it is open.

## Sub-features

- `console-open` reveals `#console` with class `on` and `aria-hidden="false"`.
- `console-whoami` prints `Jane@<zone>` in play.
- `console-status` prints vitals, beat line, and bag ids.
- `console-inst-title` on title prints `no world (title)`.
- `console-uname` prints `JUNQI/web 0.1.0`.
- `console-close` hides the overlay and returns keys to the canvas.

## How to get to it (user POV)

- Press `` ` `` (or `~`) from title or play.
- Click the `#term` field once the overlay is open.
- Press Escape or `` ` `` to close.

## Driving it with the browser

Preconditions:

- Doctor reports `ok: true`.
- For whoami/status/inst-in-play: a new run is already on county (see title-new-run).
- For inst-on-title: wipe and stay on title (do not press Enter).

- **Open.** Press Backquote. `#console` has class `on`. Head text includes `JUNQI  ·  console`.
- **Focus.** Click or tab to `#term` if the caret is not there.
- **uname.** Fill `#term` with `uname`, press Enter. Log contains `JUNQI/web 0.1.0`.
- **whoami (play).** Fill `whoami`, Enter. Log contains `Jane@county` on a new run.
- **status (play).** Fill `status`, Enter. Log contains `Jane  county`, `hp 700/700`, `beat 1/9  Castle, Sunday`, `julies_letter`.
- **inst on title.** From title only, fill `inst count`, Enter. Log contains `no world (title)`.
- **Close.** Press Escape. `#console` lacks class `on` and `aria-hidden` is `true`.
- **Proof.** Screenshot open overlay `evidence/console/open.png`. Save `#console-log` innerText to `evidence/console/console.txt`.

## Gotchas

- Backquote is captured on `window` in the capture phase. Do not type it into `#term` as a command.
- While open, WASD / I / F5 do not reach Phaser.
- `status` on title still prints the default Jane unit; `inst count` is what tells you there is no world.
- Color markup is stripped in the visible log; assert the plain words.
- The log keeps 500 lines. `inst list` on county fills it with herbs and lamps. Capture `whoami` / `status` first.
