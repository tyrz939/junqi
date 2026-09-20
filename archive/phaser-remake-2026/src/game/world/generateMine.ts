import { createRng } from "@/game/systems/rng";
import { stampTrigger } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { barrelPile, roomCenter, stoneTunnel, torchGrid, type RoomRect } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { PropSpec, ZoneBlue } from "@/game/world/zoneTypes";

export function generateMine(seed: number): ZoneBlue {
  const rng = createRng(seed ^ 0x51ce);
  const cols = 64;
  const rows = 44;
  const grid = new Grid(cols, rows, "wall");
  const roomCount = 5 + Math.floor(rng() * 4);
  const rooms: RoomRect[] = [];
  for (let i = 0; i < roomCount; i++) {
    rooms.push({
      x: 3 + Math.floor((i % 3) * 18) + Math.floor(rng() * 4),
      y: 4 + Math.floor(i / 3) * 13 + Math.floor(rng() * 3),
      w: 11 + Math.floor(rng() * 3),
      h: 8 + Math.floor(rng() * 2),
    });
  }
  for (const room of rooms) grid.fillRect(room.x, room.y, room.w, room.h, "stone");
  for (let i = 0; i < rooms.length - 1; i++) stoneTunnel(grid, roomCenter(rooms[i]), roomCenter(rooms[i + 1]));

  const start = rooms[0];
  const mid = rooms[Math.min(2, rooms.length - 1)];
  const vault = rooms[Math.min(3, rooms.length - 1)];
  const boss = rooms[rooms.length - 1];
  const spawn = cellWorld(roomCenter(start).cx, roomCenter(start).cy + 2);

  const props: PropSpec[] = [
    {
      id: "mine_up",
      kind: "door",
      ...cellWorld(roomCenter(start).cx, roomCenter(start).cy + 3),
      texture: "doorway",
      toZone: "county",
    },
    {
      id: "mine_hm_door",
      kind: "door",
      ...cellWorld(roomCenter(mid).cx, mid.y + 1),
      texture: "doorway",
      locked: true,
      opens: "mine_headmaster",
    },
    {
      id: "mine_vault_door",
      kind: "door",
      ...cellWorld(roomCenter(vault).cx, vault.y + 1),
      texture: "doorway",
      locked: true,
      opens: "mine_vault",
    },
    {
      id: "mine_boss_door",
      kind: "door",
      ...cellWorld(roomCenter(boss).cx, boss.y + 1),
      texture: "doorway",
      locked: true,
      opens: "mine_boss",
    },
    {
      id: "mine_hm_chest",
      kind: "chest",
      ...cellWorld(start.x + 2, roomCenter(start).cy),
      texture: "chest",
      loot: [
        { id: "key_mine_headmaster", qty: 1 },
        { id: "key_museum", qty: 1 },
        { id: "wood", qty: 2 },
      ],
    },
    {
      id: "mine_vault_key_chest",
      kind: "chest",
      ...cellWorld(mid.x + 2, roomCenter(mid).cy),
      texture: "chest",
      loot: [
        { id: "key_mine_vault", qty: 1 },
        { id: "iron", qty: 2 },
      ],
    },
    {
      id: "mine_vault",
      kind: "chest",
      ...cellWorld(roomCenter(vault).cx, roomCenter(vault).cy),
      texture: "chest",
      locked: true,
      opens: "mine_vault",
      loot: [
        { id: "gold_bar", qty: 1 },
        { id: "gold_dust", qty: 3 },
        { id: "key_mine_boss", qty: 1 },
      ],
    },
    {
      id: "mine_boss_chest",
      kind: "chest",
      ...cellWorld(roomCenter(boss).cx, roomCenter(boss).cy),
      texture: "chest",
      locked: true,
      opens: "mine_boss",
      loot: [
        { id: "gold_bar", qty: 2 },
        { id: "iron", qty: 4 },
      ],
    },
    {
      id: "mine_plaque",
      kind: "sign",
      ...cellWorld(roomCenter(start).cx - 3, roomCenter(start).cy),
      texture: "sign",
      talk: "mine_plaque",
    },
    {
      id: "mine_spark",
      kind: "orb",
      ...cellWorld(start.x + 4, start.y + 2),
      texture: "orb_ice",
      learn: "sparkbolt",
    },
    {
      id: "mine_aid",
      kind: "chest",
      ...cellWorld(start.x + 2, start.y + 2),
      texture: "chest",
      loot: [
        { id: "apple", qty: 4 },
        { id: "small_water", qty: 2 },
      ],
    },
  ];

  const lights = [{ ...cellWorld(roomCenter(start).cx, roomCenter(start).cy), radius: 28 }];
  for (let i = 0; i < rooms.length; i++) {
    const torches = torchGrid(rooms[i], `mine_torch_${i}`, 5);
    props.push(...torches.props);
    lights.push(...torches.lights);
    if (i > 0) props.push(...barrelPile({ cx: rooms[i].x + 1, cy: rooms[i].y + 1 }, `mine_barrel_${i}`, 2 + (i % 3)));
  }

  const fillChests = 4 + Math.floor(rng() * 3);
  for (let i = 0; i < fillChests; i++) {
    const room = rooms[1 + (i % Math.max(1, rooms.length - 1))];
    const pos = cellWorld(room.x + 3 + (i % 4), room.y + 3);
    props.push({
      id: `mine_ore_${i}`,
      kind: "chest",
      x: pos.x,
      y: pos.y,
      texture: "chest",
      loot: [
        { id: i % 2 === 0 ? "iron" : "wood", qty: 2 },
        { id: "coal", qty: 1 },
      ],
    });
  }

  return {
    grid,
    spawn,
    required: ["mine_up"],
    triggers: [stampTrigger("into_the_mine", spawn.x, spawn.y)],
    lights,
    enemies: [
      { id: "mine_rat_a", kind: "rat", ...cellWorld(roomCenter(mid).cx, roomCenter(mid).cy) },
      { id: "mine_rat_b", kind: "rat", ...cellWorld(roomCenter(boss).cx - 2, roomCenter(boss).cy) },
      { id: "mine_rat_c", kind: "rat", ...cellWorld(roomCenter(start).cx + 4, roomCenter(start).cy) },
      { id: "mine_rat_d", kind: "rat", ...cellWorld(roomCenter(vault).cx - 2, roomCenter(vault).cy + 2) },
      {
        id: "mine_skel_a",
        kind: "skeleton",
        ...cellWorld(roomCenter(mid).cx - 3, roomCenter(mid).cy + 2),
        patrol: [
          cellWorld(roomCenter(mid).cx - 3, roomCenter(mid).cy + 2),
          cellWorld(roomCenter(mid).cx + 3, roomCenter(mid).cy),
        ],
      },
      { id: "mine_skel_b", kind: "skeleton", ...cellWorld(roomCenter(boss).cx + 2, roomCenter(boss).cy) },
    ],
    props,
  };
}
