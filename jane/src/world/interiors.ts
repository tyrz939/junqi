// Julie's house and the cellar under it. "The hatches are a cellar, Jane. Not a
// dungeon." 2020's room_building2_auntie had two basement stairs on one wall and
// furniture down the other; room_basement_auntie was a keyed loop with no
// enemies, two iron doors that take the same key, and a rose patch.

import { Tile } from "@/sim/grid";
import type { Blueprint, Rect } from "@/world/blueprint";
import { Kit } from "@/world/kit";

/**
 * A cottage's inside: a kitchen and a front room, 28 m by 20 of floor between them. It was 44 by
 * 30 until Sept 24: Julie is one more person on the station
 * road, and her house is the size of her neighbours'. Everything the story uses is still here.
 */
export function buildHouse(seed: number, attempt: number): Blueprint {
  const k = new Kit("house", 34, 25, seed, attempt, Tile.Wall);
  const kitchen: Rect = { cx: 2, cy: 2, w: 15, h: 20 };
  const living: Rect = { cx: 19, cy: 2, w: 13, h: 20 };
  k.fill(kitchen.cx, kitchen.cy, kitchen.w, kitchen.h, Tile.FloorWood);
  k.fill(living.cx, living.cy, living.w, living.h, Tile.FloorWood);
  const doorway = 8 + k.int(0, 6);
  k.fill(17, doorway, 2, 5, Tile.FloorWood);
  k.rect("kitchen", kitchen);

  // Front door, in the living room's south wall.
  k.prop({ key: "front_door", def: "door", cx: 25, cy: 22, to: { zone: "county", mark: "house_front" } }, 2, 2);
  k.mark("front", 25, 20, 3);

  // Two hatches on one wall, both to the cellar.
  k.prop({ key: "hatch_a", def: "hatch", cx: 4, cy: 4, to: { zone: "cellar", mark: "stair_a" } }, 2, 2);
  k.prop({ key: "hatch_b", def: "hatch", cx: 9, cy: 4, to: { zone: "cellar", mark: "stair_b" } }, 2, 2);
  k.mark("hatch_a", 5, 7, 1);
  k.mark("hatch_b", 10, 7, 1);

  k.prop({ def: "stove", cx: 12, cy: 2 }, 2, 2);
  k.prop({ key: "ice_orb", def: "orb_ice", cx: 15, cy: 2, talk: "orb_ice" }, 2, 2);
  k.prop({ def: "table", cx: 6, cy: 11 }, 3, 2);
  k.prop({ key: "julies_note", def: "note", cx: 10, cy: 12, talk: "julies_note" }, 1, 1);
  k.prop({ key: "bench", def: "bench", cx: 3, cy: 17 }, 3, 2);
  k.prop(
    {
      key: "pantry_chest",
      def: "chest",
      cx: 9,
      cy: 19,
      loot: [
        { item: "gold_dust", qty: 2 },
        { item: "small_water", qty: 3 },
        { item: "pansy", qty: 2 },
      ],
    },
    2,
    2,
  );
  k.prop({ key: "fruit_bowl", def: "fruit_bowl", cx: 13, cy: 15, loot: [{ item: "apple", qty: 4 }] }, 2, 1);
  k.prop({ def: "shelf", cx: 12, cy: 20 }, 3, 1);

  // Front room: leftover furniture along the wall, as 2020 stacked its benches.
  k.prop({ key: "julies_bed", def: "bed", cx: 29, cy: 3, talk: "bed" }, 2, 3);
  k.prop({ def: "shelf", cx: 21, cy: 2 }, 3, 1);
  k.prop({ def: "shelf", cx: 25, cy: 2 }, 3, 1);
  k.prop({ def: "table", cx: 21 + k.int(0, 4), cy: 10 + k.int(0, 3) }, 3, 2);
  k.prop({ def: "torch", cx: 20, cy: 3 }, 1, 1);
  k.prop({ def: "torch", cx: 3, cy: 10 }, 1, 1);
  k.prop({ def: "torch", cx: 30, cy: 19 }, 1, 1);
  return k.done("Julie's House", true, 0.72, attempt);
}

const CELLAR_W = 100;
const CELLAR_H = 76;

export function buildCellar(seed: number, attempt: number): Blueprint {
  const k = new Kit("cellar", CELLAR_W, CELLAR_H, seed, attempt, Tile.Wall);
  const F = Tile.Floor;

  // Room A, south-west: where hatch A comes down. The iron key is here.
  const a: Rect = { cx: 4, cy: 50, w: 22, h: 20 };
  k.fill(a.cx, a.cy, a.w, a.h, F);
  k.prop({ key: "stair_a", def: "stairs", cx: 6, cy: 66, to: { zone: "house", mark: "hatch_a" } }, 2, 2);
  k.mark("stair_a", 10, 66, 0);
  k.prop({ key: "cellar_chest", def: "chest", cx: 20, cy: 52, loot: [{ item: "key_basement", qty: 2 }] }, 2, 2);
  k.pile({ cx: a.cx + 1, cy: a.cy + 8, w: 10, h: 6 }, "barrel", 3);

  // North out of room A through the first iron door, up to the long corridor.
  k.fill(13, 40, 3, 10, F);
  k.prop({ key: "iron_door_a", def: "gate_h", cx: 13, cy: 45, locked: true, keyTag: "basement", label: "The iron door" }, 3, 1);
  k.fill(6, 37, 85, 3, F);

  // Four rooms north of the corridor.
  const rooms: Rect[] = [6, 26, 46, 66].map((x) => ({ cx: x, cy: 16 + k.int(0, 3), w: 16, h: 14 }));
  rooms.forEach((r) => {
    k.fill(r.cx, r.cy, r.w, r.h, F);
    k.fill(r.cx + 6, r.cy + r.h, 3, 37 - (r.cy + r.h), F);
  });
  const [ratRoom, potionRoom, storage, study] = rooms;

  for (let n = 0; n < 3; n++) {
    const s = k.spot(ratRoom, 1, 1, 1);
    if (s) k.unit(null, "rat", s.cx, s.cy);
  }
  k.prop(
    {
      key: "rat_chest",
      def: "chest",
      cx: ratRoom.cx + 1,
      cy: ratRoom.cy + 1,
      loot: [
        { item: "key_generic", qty: 1 },
        { item: "small_empty_vial", qty: 2 },
      ],
    },
    2,
    2,
  );

  // "Potion making Room" from the 2020 basement sketch.
  k.prop({ key: "potion_bench", def: "bench", cx: potionRoom.cx + 2, cy: potionRoom.cy + 1 }, 3, 2);
  k.prop(
    {
      key: "potion_chest",
      def: "chest",
      cx: potionRoom.cx + 12,
      cy: potionRoom.cy + 1,
      loot: [
        { item: "small_water", qty: 4 },
        { item: "hemshade_root", qty: 1 },
        { item: "night_lich_moss", qty: 1 },
        { item: "stone", qty: 1 },
      ],
    },
    2,
    2,
  );
  k.prop({ def: "shelf", cx: potionRoom.cx + 6, cy: potionRoom.cy }, 3, 1);

  // "storage room locked": wood and iron, which the mine's broken things will want.
  k.prop({ key: "storage_gate", def: "gate_h", cx: storage.cx + 6, cy: storage.cy + storage.h + 1, locked: true, keyTag: "generic", label: "The storage room" }, 3, 1);
  k.prop(
    {
      key: "storage_chest",
      def: "chest",
      cx: storage.cx + 7,
      cy: storage.cy + 1,
      loot: [
        { item: "wood", qty: 4 },
        { item: "iron", qty: 4 },
      ],
    },
    2,
    2,
  );
  k.pile({ cx: storage.cx + 1, cy: storage.cy + 5, w: storage.w - 2, h: 5 }, "crate", 3);

  // The study. 2020's sketch put an electricity orb here; that spell is a later door.
  k.prop({ def: "table", cx: study.cx + 6, cy: study.cy + 4 }, 3, 2);
  k.prop({ def: "shelf", cx: study.cx + 2, cy: study.cy }, 3, 1);
  k.prop({ def: "shelf", cx: study.cx + 10, cy: study.cy }, 3, 1);
  const lone = k.spot(study, 1, 1, 1);
  if (lone) k.unit(null, "rat", lone.cx, lone.cy);

  // Rose alcove south of the corridor. White Water Rose goes into Stone Skin.
  const alcove: Rect = { cx: 40, cy: 40, w: 12, h: 8 };
  k.fill(alcove.cx, alcove.cy, alcove.w, alcove.h, Tile.Garden);
  for (let n = 0; n < 3; n++) {
    const s = k.spot({ cx: alcove.cx + 1, cy: alcove.cy + 2, w: alcove.w - 2, h: alcove.h - 3 }, 1, 1, 0);
    if (s) k.prop({ def: "rose", cx: s.cx, cy: s.cy, loot: [{ item: "white_water_rose", qty: 1 }] }, 1, 1);
  }

  // East leg: the other iron door, then room B and hatch B. Either door closes the loop.
  k.fill(88, 40, 3, 20, F);
  k.prop({ key: "iron_door_b", def: "gate_h", cx: 88, cy: 48, locked: true, keyTag: "basement", label: "The other iron door" }, 3, 1);
  const b: Rect = { cx: 74, cy: 58, w: 20, h: 14 };
  k.fill(b.cx, b.cy, b.w, b.h, F);
  k.prop({ key: "stair_b", def: "stairs", cx: 90, cy: 68, to: { zone: "house", mark: "hatch_b" } }, 2, 2);
  k.mark("stair_b", 87, 68, 2);
  k.pile({ cx: b.cx + 1, cy: b.cy + 1, w: 8, h: 6 }, "barrel", 2);

  for (const r of [a, b, ...rooms]) k.torchRun(r, 9);
  for (let x = 10; x < 88; x += 12) k.prop({ def: "torch", cx: x, cy: 37 }, 1, 1);
  k.rect("cellar", { cx: 0, cy: 0, w: CELLAR_W, h: CELLAR_H });
  return k.done("Julie's Cellar", true, 0.3, attempt);
}

// --- Castle: the two doors on the square that open ------------------------------------------
//
// The Castle Arms and St Anne's. Small rooms, full: a Zelda house is a counter, a fire, a
// table and somebody to talk to. Both are the same on every seed. They register themselves
// from world/zones/arms.ts and world/zones/church.ts.

/** The Castle Arms: the tap room, the bar, a fire, the regulars, and a room at the back with a bed. */
export function buildArms(seed: number, attempt: number): Blueprint {
  const k = new Kit("arms", 34, 25, seed, attempt, Tile.Wall);
  k.fill(2, 2, 30, 20, Tile.FloorWood);
  // The back room: a partition with a doorway in it.
  k.fill(23, 2, 1, 8, Tile.Wall);
  k.fill(23, 10, 9, 1, Tile.Wall);
  k.fill(27, 10, 2, 1, Tile.FloorWood);
  k.rect("tap_room", { cx: 2, cy: 2, w: 21, h: 20 });

  k.prop({ key: "exit_door", def: "door", cx: 15, cy: 22, to: { zone: "county", mark: "arms_front" }, label: "The street" }, 2, 2);
  k.mark("entry", 15, 20, 3);

  // The bar along the north wall, the shelves behind it, the landlady in front of it.
  k.prop({ def: "shelf", cx: 4, cy: 2 }, 3, 1);
  k.prop({ def: "shelf", cx: 8, cy: 2 }, 3, 1);
  k.prop({ key: "arms_bar", def: "town_bar", cx: 4, cy: 4 }, 3, 2);
  k.prop({ def: "town_bar", cx: 7, cy: 4 }, 3, 2);
  k.prop({ def: "town_bar", cx: 10, cy: 4 }, 3, 2);
  k.unit("mrs_garland", "town_landlady", 8, 8).facing = 1;
  k.prop({ key: "arms_notice", def: "sign", cx: 14, cy: 2, talk: "arms_notice" }, 2, 1);
  k.prop({ def: "barrel", cx: 18, cy: 2 }, 2, 2);
  k.prop({ def: "barrel", cx: 20, cy: 2 }, 2, 2);
  k.prop({ def: "barrel", cx: 19, cy: 5, loot: [{ item: "small_water", qty: 1 }] }, 2, 2);

  // The fire, and four tables, two of them taken.
  k.prop({ key: "arms_stove", def: "stove", cx: 2, cy: 11 }, 2, 2);
  k.prop({ def: "table", cx: 7, cy: 12 }, 3, 2);
  k.prop({ def: "table", cx: 14, cy: 12 }, 3, 2);
  k.prop({ def: "table", cx: 7, cy: 17 }, 3, 2);
  k.prop({ def: "table", cx: 14, cy: 17 }, 3, 2);
  k.unit("mr_quill", "town_regular", 10, 13).facing = 2;
  k.unit("mr_ennis", "town_regular_b", 17, 18).facing = 2;
  k.prop({ def: "torch", cx: 2, cy: 3 }, 1, 1);
  k.prop({ def: "torch", cx: 22, cy: 15 }, 1, 1);
  k.prop({ def: "torch", cx: 2, cy: 20 }, 1, 1);

  // The room at the back. The door locks from her side; nobody comes in.
  k.prop({ key: "arms_bed", def: "bed", cx: 29, cy: 3, talk: "arms_bed" }, 2, 3);
  k.prop({ def: "shelf", cx: 25, cy: 2 }, 3, 1);
  k.prop({ def: "torch", cx: 24, cy: 8 }, 1, 1);
  // Beyond the partition, the rest of the house: kegs and the landing.
  k.prop({ def: "crate", cx: 25, cy: 14 }, 2, 2);
  k.prop({ def: "barrel", cx: 28, cy: 14 }, 2, 2);
  k.prop({ def: "barrel", cx: 28, cy: 18 }, 2, 2);
  return k.done("The Castle Arms", true, 0.7, attempt);
}

/** St Anne's: pews either side of an aisle, the altar and its candles, the book by the door, and the vicar. */
export function buildChurch(seed: number, attempt: number): Blueprint {
  const k = new Kit("church", 22, 36, seed, attempt, Tile.TempleWall);
  k.fill(2, 2, 18, 30, Tile.TempleFloor);
  k.rect("nave", { cx: 2, cy: 2, w: 18, h: 30 });
  k.prop({ key: "exit_door", def: "door", cx: 10, cy: 32, to: { zone: "county", mark: "church_door" }, label: "The square" }, 2, 2);
  k.mark("entry", 10, 30, 3);

  k.prop({ key: "altar", def: "town_altar", cx: 9, cy: 3, talk: "altar" }, 4, 2);
  k.prop({ def: "torch", cx: 5, cy: 3 }, 1, 1);
  k.prop({ def: "torch", cx: 16, cy: 3 }, 1, 1);
  k.unit("vicar", "town_vicar", 10, 7).facing = 1;
  for (let y = 10; y <= 25; y += 3) {
    k.prop({ def: "town_pew", cx: 3, cy: y }, 4, 1);
    k.prop({ def: "town_pew", cx: 15, cy: y }, 4, 1);
  }
  k.prop({ def: "torch", cx: 2, cy: 17 }, 1, 1);
  k.prop({ def: "torch", cx: 19, cy: 17 }, 1, 1);
  k.prop({ def: "table", cx: 15, cy: 28 }, 3, 2);
  k.prop({ key: "visitors_book", def: "note", cx: 14, cy: 29, talk: "visitors_book" }, 1, 1);
  k.prop({ def: "shelf", cx: 3, cy: 30 }, 3, 1);
  return k.done("St Anne's", true, 0.55, attempt);
}
