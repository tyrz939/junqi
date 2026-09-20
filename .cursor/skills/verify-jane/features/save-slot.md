# Save slots

Three slots in `localStorage`: `jane.save.0`, `.1`, `.2`. F5 writes slot 1 (index 0), F9 reads it. The pause menu (Esc) has Save and Load for all three; the title has Load and Continue.

A save is `{ format: "jane-save", version, savedAt, summary: { zone, day, hour, hp, maxhp }, state }` where `state` is the sim's whole `GameState` tree, not a copy of selected fields.

## Sub-features

- `save-gated` F5 away from a bed or fire toasts "You can only save at a bed or a fire" and writes nothing.
- `save-rest` E on a fire (or Julie's bed) rests, heals, and writes the run's slot by itself; F5 there works too.
- `save-shape` the slot parses, `format` is `jane-save`, `state.seed` and `state.zone` match the live sim.
- `save-live-values` let a skeleton hit Jane, save, reload the page, Continue: HP is the damaged value, not max; cooldowns, statuses, ground drops and opened chests persist. (2020 wrote maxhp; the 2026 Phaser build forgot cooldowns.)
- `save-menu` Esc → Save → slot 2 writes `jane.save.1`; the title's Load shows its zone, day and time.
- `save-corrupt` set `jane.save.2` to `"{not json"`: the title shows that slot as empty and nothing throws.

## Recipe

1. New run. Walk. Press F5. Capture the toast and `slot.json`.
2. Reload the page. Title: Continue is enabled. Click it.
3. Compare `jane.sim().state.tick`, player x/y and hp with the values recorded before the reload.
