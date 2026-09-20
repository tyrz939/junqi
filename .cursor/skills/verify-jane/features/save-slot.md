# Save slots

Three slots in IndexedDB: database `jane`, store `saves`, keys `0`, `1`, `2`, each `{ meta, data }` with `data` the gzipped save JSON (`ENGINE.md` §3). F5 writes slot 1 (index 0), F9 reads it. The pause menu (Esc) has Save and Load for all three; the title has Load and Continue. Writes and reads are asynchronous: wait for the "Saved to slot N" / "Loaded slot N" toast before reading anything. `localStorage` (`jane.save.N`, plain JSON) is only the fallback when IndexedDB is blocked, and where older saves wait to be migrated at boot.

A save is `{ format: "jane-save", version, savedAt, summary: { zone, day, hour, hp, maxhp }, state }` where `state` is the sim's whole `GameState` tree, not a copy of selected fields.

Read slot N as parsed JSON (page console, read-only):

```js
const rec = await new Promise((ok, no) => { const r = indexedDB.open("jane"); r.onerror = no; r.onsuccess = () => { const g = r.result.transaction("saves").objectStore("saves").get(N); g.onsuccess = () => ok(g.result); g.onerror = no; }; });
const slot = JSON.parse(typeof rec.data === "string" ? rec.data : await new Response(new Blob([rec.data]).stream().pipeThrough(new DecompressionStream("gzip"))).text());
```

## Sub-features

- `save-gated` F5 away from a bed or fire toasts "You can only save at a bed or a fire" and writes nothing.
- `save-rest` E on a fire (or Julie's bed) rests, heals, and writes the run's slot by itself; F5 there works too.
- `save-shape` the slot parses, `format` is `jane-save`, `state.seed` and `state.zone` match the live sim.
- `save-live-values` let a skeleton hit Jane, save, reload the page, Continue: HP is the damaged value, not max; cooldowns, statuses, ground drops and opened chests persist. (2020 wrote maxhp; the 2026 Phaser build forgot cooldowns.)
- `save-menu` Esc → Save → slot 2 writes key `1`; the row refreshes when the write commits; the title's Load shows its zone, day and time.
- `save-corrupt` set `localStorage` `jane.save.2` to `"{not json"` and reload: the title shows that slot as empty, nothing throws, and the junk is left where it is. Same for an IndexedDB row whose `data` is not gzip: listed, but loading it toasts "That save could not be loaded" and the slot then reads empty.
- `save-migrate` with IndexedDB empty, put a valid save text in `localStorage` `jane.save.0` and reload: the title lists it at once; moments later it is in IndexedDB key `0` and gone from `localStorage`.

## Recipe

1. New run. Walk. Press F5. Capture the toast and `slot.json`.
2. Reload the page. Title: Continue is enabled. Click it.
3. Compare `jane.sim().state.tick`, player x/y and hp with the values recorded before the reload.
