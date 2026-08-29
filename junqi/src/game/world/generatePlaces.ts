import { createRng } from "@/game/systems/rng";
import { stampTrigger } from "@/game/systems/triggers";
import { Grid, type Tile } from "@/game/world/Grid";
import { barrelPile, paintBorder, roomCenter, scatterTile, stoneTunnel, torchGrid, type RoomRect } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { EnemySpec, PropSpec, ZoneBlue } from "@/game/world/zoneTypes";
import type { ZoneId } from "@/game/types";

function carve(
  seed: number,
  salt: number,
  cols: number,
  rows: number,
  fill: Tile,
  floor: Tile,
  roomCount: number,
): { grid: Grid; rooms: RoomRect[]; rng: () => number } {
  const rng = createRng(seed ^ salt);
  const grid = new Grid(cols, rows, fill);
  paintBorder(grid);
  const rooms: RoomRect[] = [];
  for (let i = 0; i < roomCount; i++) {
    rooms.push({
      x: 3 + Math.floor((i % 3) * Math.max(8, Math.floor(cols / 3) - 4)) + Math.floor(rng() * 3),
      y: 3 + Math.floor(i / 3) * Math.max(7, Math.floor(rows / 3)) + Math.floor(rng() * 2),
      w: 8 + Math.floor(rng() * 4),
      h: 6 + Math.floor(rng() * 3),
    });
  }
  for (const room of rooms) {
    if (room.x + room.w >= cols - 2) room.w = Math.max(6, cols - 3 - room.x);
    if (room.y + room.h >= rows - 2) room.h = Math.max(5, rows - 3 - room.y);
    grid.fillRect(room.x, room.y, room.w, room.h, floor);
  }
  for (let i = 0; i < rooms.length - 1; i++) stoneTunnel(grid, roomCenter(rooms[i]), roomCenter(rooms[i + 1]));
  return { grid, rooms, rng };
}

function exitDoor(id: string, zone: ZoneId, x: number, y: number): PropSpec {
  return {
    id,
    kind: "door",
    x,
    y,
    texture: "doorway",
    toZone: zone,
  };
}

export function generateAbandoned(seed: number): ZoneBlue {
  const { grid, rooms, rng } = carve(seed, 0xa11d, 28, 22, "wall", "wood", 2);
  const start = rooms[0];
  const spawn = cellWorld(roomCenter(start).cx, roomCenter(start).cy + 2);
  const far = rooms[rooms.length - 1];
  scatterTile(grid, 2, 2, 24, 18, "bush", 8, rng);
  const props: PropSpec[] = [
    exitDoor("abandoned_up", "county", spawn.x, cellWorld(roomCenter(start).cx, start.y + start.h - 1).y),
    {
      id: "abandoned_note",
      kind: "sign",
      ...cellWorld(roomCenter(far).cx, roomCenter(far).cy),
      texture: "sign",
      talk: "abandoned_note",
    },
    {
      id: "abandoned_tin",
      kind: "chest",
      ...cellWorld(far.x + 2, far.y + 2),
      texture: "chest",
      loot: [
        { id: "night_view_glass", qty: 1 },
        { id: "wood", qty: 2 },
      ],
    },
    ...barrelPile({ cx: start.x + 1, cy: start.y + 1 }, "abandoned_crate", 2),
  ];
  return {
    grid,
    spawn,
    required: ["abandoned_up"],
    triggers: [stampTrigger("into_abandoned", spawn.x, spawn.y)],
    lights: [{ ...spawn, radius: 22 }],
    enemies: rng() < 0.7 ? [{ id: "abandoned_root", kind: "plant", ...cellWorld(roomCenter(far).cx - 2, roomCenter(far).cy) }] : [],
    props,
  };
}

export function generateMuseum(seed: number): ZoneBlue {
  const { grid, rooms } = carve(seed, 0x5e11, 40, 28, "wall", "stone", 4);
  const start = rooms[0];
  const spawn = cellWorld(roomCenter(start).cx, roomCenter(start).cy + 2);
  const magic = rooms[rooms.length - 1];
  const props: PropSpec[] = [
    exitDoor("museum_up", "county", spawn.x, cellWorld(roomCenter(start).cx, start.y + start.h - 1).y),
    {
      id: "museum_plaque_in",
      kind: "sign",
      ...cellWorld(roomCenter(start).cx - 2, roomCenter(start).cy),
      texture: "sign",
      talk: "museum_plaque",
    },
    {
      id: "museum_inner",
      kind: "door",
      ...cellWorld(magic.x, roomCenter(magic).cy),
      texture: "doorway",
      locked: true,
      opens: "museum_inner",
    },
    {
      id: "museum_magic",
      kind: "sign",
      ...cellWorld(roomCenter(magic).cx, roomCenter(magic).cy),
      texture: "sign",
      talk: "museum_magic",
    },
    {
      id: "museum_case",
      kind: "chest",
      ...cellWorld(magic.x + 3, magic.y + 2),
      texture: "chest",
      loot: [
        { id: "gold_dust", qty: 2 },
        { id: "light_stone", qty: 1 },
      ],
    },
  ];
  const lights = rooms.flatMap((room, i) => torchGrid(room, `museum_torch_${i}`, 6).lights);
  return {
    grid,
    spawn,
    required: ["museum_up"],
    triggers: [stampTrigger("into_the_museum", spawn.x, spawn.y)],
    lights,
    enemies: [{ id: "museum_spirit", kind: "plant", ...cellWorld(roomCenter(rooms[1]).cx, roomCenter(rooms[1]).cy) }],
    props,
  };
}

export function generateGraveyard(seed: number): ZoneBlue {
  const rng = createRng(seed ^ 0x6a7e);
  const cols = 36;
  const rows = 28;
  const grid = new Grid(cols, rows, "grass");
  paintBorder(grid, "bone");
  grid.fillRect(4, 4, 28, 20, "bone");
  scatterTile(grid, 4, 4, 28, 20, "bush", 12, rng);
  const spawn = cellWorld(18, 24);
  grid.fillRect(17, 27, 2, 1, "door");
  const props: PropSpec[] = [
    exitDoor("graveyard_up", "county", spawn.x, cellWorld(18, 27).y),
    { id: "grave_warn_in", kind: "sign", ...cellWorld(8, 20), texture: "sign", talk: "grave_warn" },
    { id: "grave_julie", kind: "sign", ...cellWorld(18, 12), texture: "shrine", talk: "grave_julie" },
    { id: "grave_extra", kind: "sign", ...cellWorld(26, 8), texture: "sign", talk: "wizard_card" },
  ];
  const enemies: EnemySpec[] = [
    { id: "grave_skel_a", kind: "skeleton", ...cellWorld(10, 10) },
    { id: "grave_skel_b", kind: "skeleton", ...cellWorld(24, 16) },
  ];
  return {
    grid,
    spawn,
    required: ["graveyard_up"],
    triggers: [stampTrigger("into_graveyard", spawn.x, spawn.y)],
    lights: [{ ...spawn, radius: 20 }, { ...cellWorld(18, 12), radius: 16 }],
    enemies,
    props,
  };
}
