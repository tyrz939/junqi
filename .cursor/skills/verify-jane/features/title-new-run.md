# Title and new run

The title is DOM: wordmark **PROJECT JANE**, tagline "Castle, Sunday.", a menu (New Game, Continue, Load, Controls) and the build string in a corner.

## Sub-features

- `title-shows` the title renders on a wiped origin; Continue is disabled ("No saves yet").
- `title-new` New Game opens the name step ("The ticket is made out to:"); Enter boards the train. The HUD shows the chosen name.
- `title-controls` Controls lists keyboard and gamepad bindings.
- `title-continue` after a save exists, Continue loads the most recent slot.
- `new-run-kit` HUD shows Jane 150/150/100, zone Castle, 17:00 Day 1, tracker "A Letter from Julie"; bar slot 1 melee, slot 7 Apple ×3, slot 8 Julie's Letter.

## Recipe

1. Wipe the three `jane.save.N` keys, reload. Screenshot: `before.png`.
2. Click New Game. Wait until `jane.sim()` is non-null and `state.tick` is advancing.
3. Read `.jq-zone-name` (Castle) and the vitals. Screenshot: `after.png`.
4. `state.json`: `{ seed, zone, clock, quests, bar }` from `jane.sim().state`. `clock` must be `17 * 7200`.
5. Jane must be standing on open ground inside the yard gate with the house in view, and the letter quest must **not** be complete yet (it completes on reaching the stoop).

## Known not-bugs

- The first frame after New Game drops a few ticks while the county (640 × 384 cells) generates. F2 shows the count.
