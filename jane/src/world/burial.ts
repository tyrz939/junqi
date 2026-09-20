// The burial chamber. "Theme: Cold, dark, foggy place. lots of death in here."
// START in the centre, wings off a cross, as the 2020 map drew it. This build
// carries the wings the 2020 room actually wired up:
//
//   west   blue-torch alcove (Icebolt wakes them), rat room, the Snake Key
//   north  Snake Key gate -> lock-in (the floor stands up) -> ornate chest: Giant Snake Key
//   NW     the snake's room: four pillars, a patrol loop, gates that drop
//   east   burial snakes that cannot be fought, only fed
//   south  the garden: a flower holding a root wall shut over the last page of a book
//
// The design map's other corners (spider, zombie, undead flower bosses, and the
// wizard they unlock) are later rows, not missing engine.

import { Tile } from "@/sim/grid";
import type { Blueprint, Rect } from "@/world/blueprint";
import { Kit } from "@/world/kit";

const W = 176;
const H = 150;

export function buildBurial(seed: number, attempt: number): Blueprint {
  const k = new Kit("burial", W, H, seed, attempt, Tile.TempleWall);
  const F = Tile.TempleFloor;
  const room = (r: Rect): Rect => {
    k.fill(r.cx, r.cy, r.w, r.h, F);
    k.sprinkle(r, F, Tile.Moss, 0.06);
    return r;
  };

  const start = room({ cx: 76, cy: 70, w: 24, h: 20 });
  const ratRoom = room({ cx: 14, cy: 66, w: 26, h: 28 });
  const lockin = room({ cx: 74, cy: 22, w: 28, h: 26 });
  const arena = room({ cx: 14, cy: 14, w: 34, h: 34 });
  const east = room({ cx: 130, cy: 66, w: 30, h: 28 });
  const garden = room({ cx: 96, cy: 104, w: 36, h: 24 });
  const alcove = room({ cx: 52, cy: 68, w: 12, h: 10 });

  k.fill(40, 79, 36, 3, F); // start <-> rat room (the torch corridor)
  k.fill(100, 79, 30, 3, F); // start <-> east
  k.fill(87, 48, 3, 22, F); // start <-> lock-in
  k.fill(48, 33, 26, 3, F); // lock-in <-> arena (east gate)
  k.fill(29, 48, 3, 18, F); // arena <-> rat room (south gate)
  k.fill(111, 82, 3, 22, F); // east corridor <-> garden
  k.fill(108, 128, 6, 8, F); // the nook behind the roots
  k.fill(alcove.cx + 4, alcove.cy + alcove.h, 3, 1, F); // alcove mouth onto the corridor

  k.prop({ key: "exit_door", def: "door", cx: 87, cy: 90, to: { zone: "county", mark: "burial_mouth" } }, 2, 2);
  k.mark("entry", 88, 87, 3);
  k.rect("burial_entry", start);
  k.rect("everywhere", { cx: 0, cy: 0, w: W, h: H });
  k.unit(null, "soldier", start.cx + 3, start.cy + 3);
  k.unit(null, "soldier", start.cx + 20, start.cy + 4);

  // West: two cold torches flank an alcove. Both lit -> the alcove chest unlocks.
  k.prop({ key: "torch_a", def: "torch_blue", cx: alcove.cx + 2, cy: alcove.cy + alcove.h - 1, use: [{ do: "flag", flag: "torch_a" }] }, 1, 1);
  k.prop({ key: "torch_b", def: "torch_blue", cx: alcove.cx + 9, cy: alcove.cy + alcove.h - 1, use: [{ do: "flag", flag: "torch_b" }] }, 1, 1);
  k.prop(
    {
      key: "torch_chest",
      def: "chest",
      cx: alcove.cx + 5,
      cy: alcove.cy + 1,
      locked: true,
      label: "The cold chest",
      loot: [
        { item: "potion_winterbite", qty: 1 },
        { item: "savage_snakeroot", qty: 2 },
        { item: "small_water", qty: 2 },
      ],
    },
    2,
    2,
  );
  for (let x = 44; x < 74; x += 10) k.prop({ def: "torch_blue", cx: x, cy: 79 }, 1, 1);

  // Rat room: a pillar maze, rats for meat, the Snake Key at the far side.
  for (let n = 0; n < 9; n++) {
    const s = k.spot({ cx: ratRoom.cx + 3, cy: ratRoom.cy + 3, w: ratRoom.w - 6, h: ratRoom.h - 6 }, 2, 2, 1);
    if (s) k.prop({ def: "pillar", cx: s.cx, cy: s.cy }, 2, 2);
  }
  for (let n = 0; n < 4; n++) {
    const s = k.spot(ratRoom, 1, 1, 1);
    if (s) k.unit(null, "rat", s.cx, s.cy);
  }
  k.prop({ key: "snake_key_chest", def: "chest", cx: ratRoom.cx + 1, cy: ratRoom.cy + ratRoom.h - 3, loot: [{ item: "key_snake", qty: 1 }] }, 2, 2);

  // North: the keyed gate, then the lock-in. Marks are where the floor stands up.
  k.prop({ key: "gate_snake", def: "gate_h", cx: 87, cy: 58, locked: true, keyTag: "snake", label: "The scaled door" }, 3, 1);
  k.rect("lockin_room", { cx: lockin.cx, cy: lockin.cy, w: lockin.w, h: lockin.h - 3 });
  k.mark("lockin_a", lockin.cx + 4, lockin.cy + 4);
  k.mark("lockin_b", lockin.cx + lockin.w - 5, lockin.cy + 4);
  k.mark("lockin_c", lockin.cx + 4, lockin.cy + 16);
  k.mark("lockin_d", lockin.cx + lockin.w - 5, lockin.cy + 16);
  k.prop({ key: "giant_key_chest", def: "boss_chest", cx: lockin.cx + 13, cy: lockin.cy + 1, locked: true, label: "The ornate chest", loot: [{ item: "key_snake_boss", qty: 1 }] }, 2, 2);
  k.prop({ def: "coffin", cx: lockin.cx + 2, cy: lockin.cy + 9 }, 2, 3);
  k.prop({ def: "coffin", cx: lockin.cx + lockin.w - 4, cy: lockin.cy + 9 }, 2, 3);

  // The snake's room. 2020: spawn (160,160), a nine-point loop round four pillars.
  k.prop({ key: "snake_gate_east", def: "gate_v", cx: 52, cy: 33, locked: true, keyTag: "snake_boss", label: "The giant door" }, 1, 3);
  k.prop({ key: "snake_gate_south", def: "gate_h", cx: 29, cy: 54, locked: true, label: "A door with no keyhole" }, 3, 1);
  k.rect("snake_arena", { cx: arena.cx + 2, cy: arena.cy + 2, w: arena.w - 5, h: arena.h - 4 });
  const ax = arena.cx;
  const ay = arena.cy;
  for (const [dx, dy] of [
    [9, 9],
    [23, 9],
    [9, 23],
    [23, 23],
  ]) k.prop({ def: "pillar", cx: ax + dx, cy: ay + dy }, 2, 2);
  k.unit("burial_snake", "burial_snake", ax + 17, ay + 17, [
    [ax + 6, ay + 17],
    [ax + 6, ay + 6],
    [ax + 17, ay + 5],
    [ax + 28, ay + 6],
    [ax + 28, ay + 17],
    [ax + 28, ay + 28],
    [ax + 17, ay + 29],
    [ax + 6, ay + 28],
  ]);
  for (const [dx, dy] of [
    [1, 1],
    [32, 1],
    [1, 32],
    [32, 32],
  ]) k.prop({ def: "torch_blue", cx: ax + dx, cy: ay + dy, on: true }, 1, 1);

  // East: statues that spit, and two snakes you feed instead of fight.
  k.unit("lurker_a", "lurker", east.cx + 22, east.cy + 6);
  k.unit("lurker_b", "lurker", east.cx + 22, east.cy + 20);
  k.unit(null, "statue", east.cx + 6, east.cy + 2);
  k.unit(null, "statue", east.cx + 6, east.cy + 25);
  k.prop(
    {
      key: "lurker_chest",
      def: "chest",
      cx: east.cx + 27,
      cy: east.cy + 13,
      loot: [
        { item: "gold_dust", qty: 3 },
        { item: "honeylace_lily", qty: 2 },
        { item: "potion_lifesteal", qty: 1 },
      ],
    },
    2,
    2,
  );
  // A wall stub at the mouth so bait can be thrown from cover.
  k.fill(east.cx + 8, east.cy + 10, 2, 8, Tile.TempleWall);

  // South: the garden. Pond, cactus, a patrol, and the flower that holds the roots.
  k.fill(garden.cx, garden.cy, garden.w, garden.h, Tile.Garden);
  k.fill(garden.cx + 12, garden.cy + 6, 12, 7, Tile.Water);
  k.rect("garden", garden);
  k.unit("garden_flower", "flower", 105, garden.cy + garden.h - 3);
  k.unit(null, "cactus", garden.cx + 30, garden.cy + 4);
  k.unit(null, "pumpkin", garden.cx + 4, garden.cy + 4, [
    [garden.cx + 4, garden.cy + 4],
    [garden.cx + 4, garden.cy + 18],
    [garden.cx + 10, garden.cy + 18],
    [garden.cx + 10, garden.cy + 4],
  ]);
  k.prop({ key: "root_a", def: "root_wall", cx: 108, cy: 128 }, 2, 2);
  k.prop({ key: "root_b", def: "root_wall", cx: 110, cy: 128 }, 2, 2);
  k.prop({ key: "root_c", def: "root_wall", cx: 112, cy: 128 }, 2, 2);
  k.prop({ key: "fire_scroll", def: "scroll_fire", cx: 110, cy: 133, talk: "scroll_fire" }, 2, 2);
  for (let n = 0; n < 3; n++) {
    const s = k.spot(garden, 1, 1, 1);
    if (s) k.prop({ def: "herb", cx: s.cx, cy: s.cy, loot: [{ item: k.pick(["savage_snakeroot", "night_lich_moss", "white_water_cap"]), qty: 1 }] }, 1, 1);
  }

  for (const r of [start, lockin, east]) k.torchRun(r, 10);
  k.pile(start, "barrel", 2);
  return k.done("Burial Chamber", true, 0.1, attempt);
}
