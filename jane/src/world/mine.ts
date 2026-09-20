// Gnox Goldskin's gold mine. Topology follows 2020's room_dungeon_goldmine as
// reconstructed from its doors, chests and levers, and the hand-drawn map beside
// it ("Headmaster", "Iron Knuckles", Vault Key, "x2 Wood" repair gate):
//
//   entry --- plate room (hold the plate down with a barrel -> generic key)
//     |  \--- guard room, behind generic gate A (the clerk carries the HM key)
//   store (generic key + iron)
//     |  generic gate B
//   core ---- HM gate ---- Headmaster's office (he drops the vault key)
//     |  \--- broken steps (Repair, 2 wood) --- gallery (ornate chest: boss key)
//     |------ vault gate --- vault (gold, the museum key)
//   boss gate
//   arena: Iron Knuckles
//
// "Language: carve rooms, then drown them in barrels and torches. Lock three doors."

import { Tile } from "@/sim/grid";
import type { Blueprint, Rect } from "@/world/blueprint";
import { Kit } from "@/world/kit";

const W = 160;
const H = 130;

export function buildMine(seed: number, attempt: number): Blueprint {
  const k = new Kit("mine", W, H, seed, attempt, Tile.CaveWall);
  const F = Tile.CaveFloor;
  const room = (r: Rect): Rect => {
    // Ragged edges: shave random corners so rooms read as dug, not drawn.
    k.fill(r.cx, r.cy, r.w, r.h, F);
    for (const [x, y] of [
      [r.cx, r.cy],
      [r.cx + r.w - 3, r.cy],
      [r.cx, r.cy + r.h - 3],
      [r.cx + r.w - 3, r.cy + r.h - 3],
    ]) {
      if (k.chance(0.7)) k.fill(x, y, 3, 3, Tile.CaveWall), k.fill(x + (x === r.cx ? 1 : 0), y + (y === r.cy ? 1 : 0), 2, 2, F);
    }
    return r;
  };

  const entry = room({ cx: 66, cy: 104, w: 28, h: 18 });
  const plateRoom = room({ cx: 24, cy: 100, w: 26, h: 20 });
  const guard = room({ cx: 106, cy: 100, w: 30, h: 22 });
  const store = room({ cx: 68, cy: 78, w: 24, h: 16 });
  const core = room({ cx: 60, cy: 48, w: 40, h: 24 });
  const office = room({ cx: 110, cy: 48, w: 30, h: 24 });
  const gallery = room({ cx: 14, cy: 52, w: 20, h: 20 });
  const vault = room({ cx: 26, cy: 26, w: 28, h: 18 });
  const arena = room({ cx: 62, cy: 6, w: 38, h: 24 });

  // Corridors. Every gate sits on a 3-cell corridor so gate_h / gate_v seal it exactly.
  k.fill(50, 111, 16, 3, F); // entry <-> plate room
  k.fill(94, 111, 12, 3, F); // entry <-> guard room
  k.fill(79, 94, 3, 10, F); // entry <-> store
  k.fill(79, 72, 3, 6, F); // store <-> core
  k.fill(100, 59, 10, 3, F); // core <-> office
  k.fill(34, 59, 26, 2, F); // core <-> gallery, two cells tall: the steps fill it
  k.fill(79, 30, 3, 18, F); // core <-> arena
  k.fill(54, 39, 25, 3, F); // vault spur

  k.prop({ key: "exit_door", def: "door", cx: 79, cy: 122, to: { zone: "county", mark: "mine_mouth" } }, 2, 2);
  k.mark("entry", 80, 119, 3);
  k.rect("mine_entry", entry);

  // Plate room. 2020: "any obj_in_world_parent presses a button, so barrels work";
  // stepping off re-locked the chest, so the barrel is the answer.
  k.prop(
    {
      key: "plate_a",
      def: "plate",
      cx: plateRoom.cx + 6,
      cy: plateRoom.cy + 6,
      use: [{ do: "unlock", prop: "plate_chest" }],
      release: [{ do: "lock", prop: "plate_chest" }],
    },
    2,
    2,
  );
  k.prop({ key: "plate_chest", def: "chest", cx: plateRoom.cx + 20, cy: plateRoom.cy + 3, locked: true, loot: [{ item: "key_generic", qty: 1 }], label: "The chest" }, 2, 2);
  k.prop({ key: "plate_barrel", def: "barrel", cx: plateRoom.cx + 12, cy: plateRoom.cy + 12 }, 2, 2);

  k.prop(
    {
      key: "store_chest",
      def: "chest",
      cx: store.cx + 10,
      cy: store.cy + 1,
      loot: [
        { item: "key_generic", qty: 1 },
        { item: "iron", qty: 4 },
      ],
    },
    2,
    2,
  );

  k.prop({ key: "gate_generic_a", def: "gate_v", cx: 100, cy: 111, locked: true, keyTag: "generic" }, 1, 3);
  k.prop({ key: "gate_generic_b", def: "gate_h", cx: 79, cy: 75, locked: true, keyTag: "generic" }, 3, 1);
  k.prop({ key: "gate_hm", def: "gate_v", cx: 104, cy: 59, locked: true, keyTag: "mine_headmaster", label: "The Headmaster's door" }, 1, 3);
  k.prop({ key: "gate_vault", def: "gate_v", cx: 56, cy: 39, locked: true, keyTag: "mine_vault", label: "The vault" }, 1, 3);
  k.prop({ key: "gate_boss", def: "gate_h", cx: 79, cy: 34, locked: true, keyTag: "mine_boss", label: "The big door" }, 3, 1);

  k.unit("clerk", "skeleton_clerk", guard.cx + 20, guard.cy + 10);
  k.unit(null, "skeleton", guard.cx + 8, guard.cy + 6);
  k.unit(null, "skeleton", guard.cx + 10, guard.cy + 16);
  k.prop({ key: "guard_chest", def: "chest", cx: guard.cx + 26, cy: guard.cy + 2, loot: [{ item: "wood", qty: 2 }] }, 2, 2);

  k.unit("headmaster", "headmaster", office.cx + 18, office.cy + 12);
  k.prop({ def: "table", cx: office.cx + 20, cy: office.cy + 4 }, 3, 2);
  k.prop({ def: "shelf", cx: office.cx + 8, cy: office.cy }, 3, 1);
  k.prop({ key: "office_chest", def: "chest", cx: office.cx + 26, cy: office.cy + 20, loot: [{ item: "apple", qty: 2 }, { item: "coal", qty: 2 }] }, 2, 2);

  k.prop(
    {
      key: "broken_steps",
      def: "broken_steps",
      cx: 44,
      cy: 59,
      needs: [{ item: "wood", qty: 2 }],
      use: [{ do: "hide", prop: "broken_steps" }, { do: "toast", text: "The steps hold." }],
      label: "Broken steps",
    },
    3,
    2,
  );
  k.prop({ key: "boss_key_chest", def: "boss_chest", cx: gallery.cx + 3, cy: gallery.cy + 9, loot: [{ item: "key_mine_boss", qty: 1 }] }, 2, 2);

  k.prop(
    {
      key: "vault_chest",
      def: "boss_chest",
      cx: vault.cx + 4,
      cy: vault.cy + 8,
      loot: [
        { item: "gold_bar", qty: 5 },
        { item: "key_museum", qty: 1 },
      ],
    },
    2,
    2,
  );

  k.unit("iron_knuckles", "iron_knuckles", arena.cx + 19, arena.cy + 8);
  // The arena rect stops short of the gate so it never drops on Jane's head.
  k.rect("boss_arena", { cx: arena.cx, cy: arena.cy, w: arena.w, h: arena.h - 2 });
  for (const [dx, dy] of [
    [8, 6],
    [28, 6],
    [8, 16],
    [28, 16],
  ]) k.prop({ def: "pillar", cx: arena.cx + dx, cy: arena.cy + dy }, 2, 2);

  // Core: cart track set-dressing. The cart itself is a LATER machine row.
  k.fill(core.cx + 4, core.cy + 18, core.w - 8, 1, Tile.Track);
  k.prop({ def: "minecart", cx: core.cx + 8, cy: core.cy + 16 }, 2, 2);

  // Wildlife, then the 2020 look: barrels and torches everywhere.
  const dens: [Rect, string, number][] = [
    [store, "bat", 2],
    [core, "rat", 3],
    [core, "bat", 2],
    [gallery, "bat", 3],
    [plateRoom, "rat", 2],
    [vault, "skeleton", 2],
    [entry, "rat", 1],
  ];
  for (const [r, def, n] of dens) {
    for (let i = 0; i < n; i++) {
      const s = k.spot(r, 1, 1, 2);
      if (s) k.unit(null, def, s.cx, s.cy);
    }
  }
  for (const r of [entry, guard, store, core, office, gallery, vault]) k.pile(r, "barrel", k.int(1, 3));
  for (const r of [entry, plateRoom, guard, store, core, office, gallery, vault, arena]) k.torchRun(r, 8);
  return k.done("Goldskin Mine", true, 0.16, attempt);
}
