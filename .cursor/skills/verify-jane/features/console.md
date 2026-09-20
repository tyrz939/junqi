# Console

Backtick opens a drop-down console (`.jq-term-input`, `.jq-term-log`). It pauses the world. Escape or backtick closes it. Rows live in `jane/src/app/terminal.ts`.

## Sub-features

- `console-toggle` backtick opens and closes; the world stops ticking while open.
- `console-help` `help` lists every row with usage.
- `console-give` `give apple 5` raises the bar's apple count by 5.
- `console-tp` `tp house front` changes `.jq-zone-name` to Julie's House and prints the zone's marks.
- `console-time` `time 22` makes the county dark; lamp posts light.
- `console-hash` `hash` prints tick and state hash.
- `console-replay` `replay verify` prints `MATCH`.
- `console-unknown` an unknown row prints `Unknown command`, never throws.

## Recipe

1. New run on `seed 20260920`. Open the console. `console.txt` starts here.
2. Run `help`, `pos`, `inst count`, `give apple 5`, `time 22`, `hash`.
3. Close the console, walk a few seconds, reopen, run `replay verify`. Require `MATCH`. This proves the cheats went through the sim's command path.
4. Screenshot the county at 22:00: lamp posts lit, Jane carrying a small light.
