import towns from "@/data/towns.json";
import npcs from "@/data/npcs.json";
import { makeTrigger } from "@/game/systems/triggers";
import type { TriggerSpec } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { fenceRing, houseStamp, HOUSE_STAMPS, lotFits } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { EnemySpec, PropSpec } from "@/game/world/zoneTypes";

export type TownDef = { name: string; cx: number; cy: number; empty: number; radius: number };
export type NpcDef = { name: string; town: string; quest: string; ask: string[]; done: string[] };

const townCatalog = towns as Record<string, TownDef>;
const npcCatalog = npcs as Record<string, NpcDef>;

export function getNpc(id: string): NpcDef | undefined {
  return npcCatalog[id];
}

export function allNpcs(): [string, NpcDef][] {
  return Object.entries(npcCatalog);
}

export function stampTowns(grid: Grid, rng: () => number): {
  props: PropSpec[];
  enemies: EnemySpec[];
  triggers: TriggerSpec[];
  lights: { x: number; y: number; radius: number }[];
} {
  const props: PropSpec[] = [];
  const enemies: EnemySpec[] = [];
  const triggers: TriggerSpec[] = [];
  const lights: { x: number; y: number; radius: number }[] = [];

  for (const [townId, town] of Object.entries(townCatalog)) {
    const here = cellWorld(town.cx, town.cy);
    lights.push({ ...here, radius: 32 });
    props.push({
      id: `board_${townId}`,
      kind: "sign",
      ...cellWorld(town.cx + 1, town.cy + 1),
      texture: "sign",
      talk: `board_${townId}`,
    });
    triggers.push(
      makeTrigger(`enter_${townId}`, "enter", [`location:town_${townId}`], {
        x: here.x,
        y: here.y,
        w: 96,
        h: 96,
      }),
    );

    for (let i = 0; i < town.empty; i++) {
      const stamp = HOUSE_STAMPS[Math.floor(rng() * HOUSE_STAMPS.length)];
      const lx = town.cx + Math.floor(rng() * town.radius) - Math.floor(town.radius / 3);
      const ly = town.cy + Math.floor(rng() * town.radius) - Math.floor(town.radius / 4);
      if (!lotFits(grid, lx, ly, stamp.w + 3, stamp.h + 3)) continue;
      fenceRing(grid, lx, ly, stamp.w + 3, stamp.h + 3);
      houseStamp(grid, lx + 1, ly + 1, stamp.w, stamp.h);
    }

    const locals = allNpcs().filter(([, n]) => n.town === townId);
    locals.forEach(([id, _npc], i) => {
      const stamp = HOUSE_STAMPS[i % HOUSE_STAMPS.length];
      const lx = town.cx + 8 + (i % 2) * 16;
      const ly = town.cy + 6 + Math.floor(i / 2) * 14;
      if (lx + stamp.w >= grid.cols - 4 || ly + stamp.h >= grid.rows - 4) return;
      fenceRing(grid, lx, ly, stamp.w + 3, stamp.h + 3, 2);
      houseStamp(grid, lx + 1, ly + 1, stamp.w, stamp.h);
      const door = cellWorld(lx + 2 + Math.floor(stamp.w / 2), ly + stamp.h + 2);
      props.push({
        id: `npc_${id}`,
        kind: "npc",
        x: door.x,
        y: door.y,
        texture: "npc",
        talk: id,
      });
      if (i === 0) {
        props.push({
          id: `chest_${id}`,
          kind: "chest",
          x: door.x + 12,
          y: door.y,
          texture: "chest",
          loot: townChest(townId),
        });
      }
    });

    const fauna = townFauna(townId);
    fauna.forEach((kind, i) => {
      const ang = (i / Math.max(1, fauna.length)) * Math.PI * 2;
      const rad = 16 + (i % 3) * 7;
      let cx = town.cx + Math.round(Math.cos(ang) * rad);
      let cy = town.cy + Math.round(Math.sin(ang) * rad);
      if (grid.solid(cx, cy)) {
        cx = town.cx + 10 + (i % 5) * 3;
        cy = town.cy + 12 + Math.floor(i / 5) * 3;
      }
      enemies.push({
        id: `${townId}_${kind}_${i}`,
        kind,
        ...cellWorld(cx, cy),
        respawn: 40,
      });
    });
  }

  enemies.push(...huntPacks());
  return { props, enemies, triggers, lights };
}

function townChest(townId: string): { id: string; qty: number }[] {
  if (townId === "acreton") {
    return [
      { id: "egg", qty: 5 },
      { id: "milk", qty: 3 },
      { id: "cabbage", qty: 2 },
      { id: "butter", qty: 1 },
      { id: "peach", qty: 2 },
    ];
  }
  if (townId === "rivermill") return [{ id: "wheat", qty: 4 }, { id: "fish", qty: 2 }, { id: "flour", qty: 2 }];
  if (townId === "kiln_end") return [{ id: "coal", qty: 3 }, { id: "salt", qty: 3 }, { id: "bread", qty: 1 }];
  if (townId === "boneford") return [{ id: "bone_chip", qty: 2 }, { id: "tea_leaf", qty: 2 }];
  if (townId === "ridgegate") return [{ id: "tea_leaf", qty: 3 }, { id: "honey", qty: 2 }, { id: "seed", qty: 4 }];
  return [{ id: "cheese", qty: 2 }, { id: "bread", qty: 2 }, { id: "apple", qty: 2 }];
}

function townFauna(townId: string): string[] {
  if (townId === "rivermill") {
    return ["mill_rat", "mill_rat", "mill_rat", "mill_rat", "river_toad", "river_toad", "marsh_slime", "marsh_slime"];
  }
  if (townId === "acreton") return ["crow", "crow", "crow", "fox", "fox", "fox", "boar"];
  if (townId === "kiln_end") {
    return ["brigand", "brigand", "brigand", "miner", "miner", "ash_pumpkin", "ash_pumpkin", "kiln_golem", "overseer"];
  }
  if (townId === "boneford") {
    return [
      "ghoul",
      "ghoul",
      "grave_crow",
      "grave_crow",
      "grave_crow",
      "night_flower",
      "night_flower",
      "moth",
      "moth",
      "moth",
      "wight",
      "wight",
      "spectre",
      "captain",
      "bone_wolf",
      "bone_wolf",
      "undead_bat",
      "undead_bat",
      "undead_bat",
    ];
  }
  if (townId === "ridgegate") {
    return [
      "hermit_wolf",
      "hermit_wolf",
      "cave_bat",
      "cave_bat",
      "cave_bat",
      "frost_cactus",
      "frost_cactus",
      "hawk",
      "gold_rat",
      "gold_rat",
      "gold_rat",
    ];
  }
  return ["cutpurse", "cutpurse", "thug", "thug", "conscript", "conscript", "conscript", "conscript", "poacher"];
}

function huntPacks(): EnemySpec[] {
  const spots: { kind: string; cx: number; cy: number }[] = [
    { kind: "widow", cx: 360, cy: 520 },
    { kind: "widow", cx: 410, cy: 560 },
    { kind: "hatchling", cx: 340, cy: 470 },
    { kind: "hatchling", cx: 390, cy: 490 },
    { kind: "hatchling", cx: 430, cy: 530 },
    { kind: "hatchling", cx: 350, cy: 550 },
    { kind: "vine", cx: 300, cy: 580 },
    { kind: "vine", cx: 260, cy: 640 },
    { kind: "vine", cx: 330, cy: 700 },
    { kind: "briar", cx: 220, cy: 760 },
    { kind: "briar", cx: 280, cy: 800 },
    { kind: "lily_undead", cx: 1520, cy: 940 },
    { kind: "lily_undead", cx: 1460, cy: 900 },
    { kind: "student", cx: 270, cy: 90 },
    { kind: "student", cx: 330, cy: 100 },
    { kind: "teacher_undead", cx: 310, cy: 55 },
    { kind: "sewer_rat", cx: 1060, cy: 770 },
    { kind: "sewer_rat", cx: 1090, cy: 800 },
    { kind: "sewer_rat", cx: 1110, cy: 760 },
    { kind: "sewer_rat", cx: 1040, cy: 810 },
    { kind: "sewer_rat", cx: 1080, cy: 830 },
    { kind: "beetle", cx: 500, cy: 660 },
    { kind: "beetle", cx: 580, cy: 720 },
    { kind: "beetle", cx: 620, cy: 680 },
    { kind: "beetle", cx: 540, cy: 740 },
  ];
  return spots.map((s, i) => ({
    id: `hunt_${s.kind}_${i}`,
    kind: s.kind,
    ...cellWorld(s.cx, s.cy),
    respawn: 48,
  }));
}
