import { createRng } from "@/game/systems/rng";
import { stampTrigger } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { coffinRow, lilyPool } from "@/game/world/motifs";
import { cellWorld, getPocket, stampPocket } from "@/game/world/stamp";
import type { EnemySpec, PropSpec, ZoneBlue } from "@/game/world/zoneTypes";

type Wing = "snake" | "garden" | "spider" | "coffin";

export function generateBurial(seed: number): ZoneBlue {
  const rng = createRng(seed ^ 0x51ed);
  const cols = 64;
  const rows = 64;
  const grid = new Grid(cols, rows, "wall");

  const hub = { x: 26, y: 26, w: 12, h: 12 };
  grid.fillRect(hub.x, hub.y, hub.w, hub.h, "bone");
  grid.fillRect(4, 30, 22, 6, "bone");
  grid.fillRect(38, 30, 22, 6, "bone");
  grid.fillRect(30, 4, 6, 22, "bone");
  grid.fillRect(30, 38, 6, 22, "bone");

  const wings: Wing[] = shuffle(["snake", "garden", "spider", "coffin"], rng);
  const slots = {
    north: { x: 22, y: 4, w: 20, h: 16 },
    east: { x: 42, y: 22, w: 18, h: 16 },
    south: { x: 22, y: 44, w: 20, h: 16 },
    west: { x: 4, y: 22, w: 18, h: 16 },
  };
  const order = ["south", "west", "east", "north"] as const;
  const placed: Record<Wing, (typeof slots)["north"]> = {
    snake: slots.north,
    garden: slots.east,
    spider: slots.west,
    coffin: slots.south,
  };
  order.forEach((dir, i) => {
    placed[wings[i]] = slots[dir];
    const s = slots[dir];
    grid.fillRect(s.x, s.y, s.w, s.h, "bone");
  });

  const south = slots.south;
  const spawn = cellWorld(south.x + 4, south.y + south.h - 3);
  const garden = placed.garden;
  const snakeRoom = placed.snake;
  const spider = placed.spider;
  const coffin = placed.coffin;

  lilyPool(grid, garden.x + 2, garden.y + 3, 8, 6);
  const gardenStamp = stampPocket(grid, getPocket("burial_garden"), garden.x + 3, garden.y + 2, rng, 1);
  coffinRow(grid, coffin.x + 2, coffin.y + 4, 4);
  const gate = stampPocket(grid, getPocket("snake_gate"), snakeRoom.x + 4, snakeRoom.y + 2, rng, 1);

  const props: PropSpec[] = [
    {
      id: "burial_exit",
      kind: "door",
      ...cellWorld(south.x + 3, south.y + south.h - 2),
      texture: "doorway",
      toZone: "county",
    },
    {
      id: "story_relic",
      kind: "chest",
      ...cellWorld(hub.x + 2, hub.y + 6),
      texture: "chest",
      loot: [{ id: "snake_key", qty: 1 }],
      actions: ["beat:relic"],
    },
    {
      id: "shrine",
      kind: "shrine",
      ...cellWorld(hub.x + 6, hub.y + 6),
      texture: "shrine",
    },
    {
      id: "boss_key_chest",
      kind: "chest",
      ...cellWorld(coffin.x + 8, coffin.y + 8),
      texture: "chest",
      loot: [{ id: "snake_boss_key", qty: 1 }],
    },
    {
      id: "snake_boss_door",
      kind: "door",
      ...cellWorld(snakeRoom.x + 10, snakeRoom.y + 14),
      texture: "doorway",
      locked: true,
      opens: "burial_boss",
    },
    ...gardenStamp.props,
    ...gate.props,
    {
      id: "burial_glow",
      kind: "chest",
      ...cellWorld(spider.x + 3, spider.y + 3),
      texture: "chest",
      loot: [
        { id: "gold_dust", qty: 3 },
        { id: "light_stone", qty: 1 },
      ],
    },
    {
      id: "burial_herbs",
      kind: "chest",
      ...cellWorld(coffin.x + 3, coffin.y + 2),
      texture: "chest",
      loot: [
        { id: "savage_snakeroot", qty: 2 },
        { id: "apple", qty: 2 },
      ],
    },
    {
      id: "burial_hub_torch_a",
      kind: "torch",
      ...cellWorld(hub.x + 2, hub.y + 2),
      texture: "torch",
    },
    {
      id: "burial_hub_torch_b",
      kind: "torch",
      ...cellWorld(hub.x + 9, hub.y + 2),
      texture: "torch",
    },
  ];

  const lockIn = gate.props.find((p) => p.id === "lock_in");
  if (!lockIn) throw new Error("Burial missing lock_in");
  const enemies: EnemySpec[] = [
    ...gate.enemies,
    ...gardenStamp.enemies,
    {
      id: "burial_spider",
      kind: "spider",
      ...cellWorld(spider.x + 6, spider.y + 6),
      patrol: [cellWorld(spider.x + 4, spider.y + 5), cellWorld(spider.x + 12, spider.y + 8)],
    },
    { id: "burial_spider_b", kind: "spider", ...cellWorld(spider.x + 10, spider.y + 4) },
    { id: "burial_rat", kind: "rat", ...cellWorld(coffin.x + 4, coffin.y + 10) },
    { id: "burial_skel", kind: "skeleton", ...cellWorld(coffin.x + 10, coffin.y + 6) },
    {
      id: "burial_plant_b",
      kind: "plant",
      ...cellWorld(garden.x + 4, garden.y + 8),
    },
    { id: "burial_flower", kind: "flower", ...cellWorld(garden.x + 10, garden.y + 4) },
    { id: "burial_cactus", kind: "cactus", ...cellWorld(coffin.x + 6, coffin.y + 8) },
  ];

  return {
    grid,
    spawn,
    required: ["story_relic", "lock_in", "shrine", "boss_gate", "snake"],
    triggers: [
      stampTrigger("into_the_burial", spawn.x, spawn.y),
      stampTrigger("burial_lock_in", lockIn.x, lockIn.y, 56, 56),
      stampTrigger("see_the_garden", gardenStamp.props[0]?.x ?? spawn.x, gardenStamp.props[0]?.y ?? spawn.y, 64, 64),
      stampTrigger("snake_room", gate.enemies[0]?.x ?? lockIn.x, gate.enemies[0]?.y ?? lockIn.y, 72, 72),
    ],
    lights: [
      { ...spawn, radius: 22 },
      { ...cellWorld(hub.x + 6, hub.y + 6), radius: 26 },
      { ...cellWorld(snakeRoom.x + 8, snakeRoom.y + 4), radius: 30 },
      { ...cellWorld(garden.x + 6, garden.y + 5), radius: 20 },
      { ...cellWorld(spider.x + 6, spider.y + 6), radius: 18 },
      { ...cellWorld(hub.x + 2, hub.y + 2), radius: 16 },
    ],
    enemies,
    props,
  };
}

function shuffle<T>(list: T[], rng: () => number): T[] {
  const out = [...list];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}
