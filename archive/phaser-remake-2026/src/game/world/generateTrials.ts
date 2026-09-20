import { makeTrigger } from "@/game/systems/triggers";
import type { TriggerSpec } from "@/game/systems/triggers";
import { Grid } from "@/game/world/Grid";
import { barrelPile, fenceRing } from "@/game/world/motifs";
import { cellWorld } from "@/game/world/stamp";
import type { EnemySpec, FloodSpec, PropSpec } from "@/game/world/zoneTypes";

export type TrialPack = {
  props: PropSpec[];
  enemies: EnemySpec[];
  triggers: TriggerSpec[];
  floods: FloodSpec[];
  lights: { x: number; y: number; radius: number }[];
};

const SPOTS: { cx: number; cy: number }[] = [
  { cx: 420, cy: 280 },
  { cx: 620, cy: 180 },
  { cx: 860, cy: 260 },
  { cx: 980, cy: 420 },
  { cx: 240, cy: 380 },
  { cx: 500, cy: 480 },
  { cx: 760, cy: 560 },
  { cx: 1080, cy: 500 },
  { cx: 1280, cy: 360 },
  { cx: 1440, cy: 520 },
  { cx: 360, cy: 820 },
  { cx: 680, cy: 780 },
  { cx: 920, cy: 900 },
  { cx: 1200, cy: 820 },
  { cx: 1560, cy: 700 },
  { cx: 1700, cy: 500 },
  { cx: 1880, cy: 640 },
  { cx: 80, cy: 700 },
  { cx: 1600, cy: 200 },
  { cx: 1320, cy: 160 },
  { cx: 200, cy: 980 },
  { cx: 800, cy: 1040 },
  { cx: 340, cy: 220 },
  { cx: 900, cy: 180 },
  { cx: 1040, cy: 320 },
  { cx: 1400, cy: 280 },
  { cx: 1840, cy: 280 },
  { cx: 250, cy: 520 },
  { cx: 430, cy: 900 },
  { cx: 640, cy: 980 },
  { cx: 880, cy: 720 },
  { cx: 1320, cy: 640 },
  { cx: 1600, cy: 920 },
  { cx: 1900, cy: 780 },
  { cx: 100, cy: 400 },
  { cx: 1100, cy: 980 },
  { cx: 1500, cy: 120 },
  { cx: 720, cy: 480 },
];

type Verb = "push" | "lever" | "clear" | "drain" | "grow";

export function stampTrials(grid: Grid, rng: () => number): TrialPack {
  const pack: TrialPack = { props: [], enemies: [], triggers: [], floods: [], lights: [] };
  let n = 0;
  const verbs: Verb[] = ["push", "lever", "clear", "drain", "grow"];
  for (const spot of SPOTS) {
    if (n >= 28) break;
    if (!fits(grid, spot.cx, spot.cy, 16, 14)) continue;
    const verb = verbs[n % verbs.length];
    paintPad(grid, spot.cx, spot.cy, 16, 14);
    if (verb === "push") pushTrial(pack, n, spot.cx, spot.cy);
    else if (verb === "lever") leverTrial(pack, n, spot.cx, spot.cy);
    else if (verb === "clear") clearTrial(pack, n, spot.cx, spot.cy);
    else if (verb === "drain") drainTrial(pack, n, spot.cx, spot.cy, grid);
    else growTrial(pack, n, spot.cx, spot.cy, grid);
    pack.lights.push({ ...cellWorld(spot.cx + 8, spot.cy + 7), radius: 20 });
    n += 1;
  }
  void rng;
  return pack;
}

function fits(grid: Grid, x: number, y: number, w: number, h: number): boolean {
  if (x < 4 || y < 4 || x + w >= grid.cols - 4 || y + h >= grid.rows - 4) return false;
  let grass = 0;
  for (let cy = y; cy < y + h; cy++) {
    for (let cx = x; cx < x + w; cx++) {
      const t = grid.get(cx, cy);
      if (t === "house" || t === "wall") return false;
      if (t === "grass" || t === "stone") grass += 1;
    }
  }
  return grass > (w * h) * 0.55;
}

function paintPad(grid: Grid, x: number, y: number, w: number, h: number): void {
  fenceRing(grid, x, y, w, h, 2);
  grid.fillRect(x + 1, y + 1, w - 2, h - 2, "stone");
}

function markChest(id: string, x: number, y: number, extra: { id: string; qty: number }[] = []): PropSpec {
  return {
    id,
    kind: "chest",
    ...cellWorld(x, y),
    texture: "chest",
    loot: [{ id: "julies_mark", qty: 1 }, ...extra],
    actions: [`set:trial:${id}`, "note:A mark Julie would have left."],
  };
}

function pushTrial(pack: TrialPack, n: number, x: number, y: number): void {
  pack.props.push(...barrelPile({ cx: x + 3, cy: y + 4 }, `trial${n}_crate`, 6));
  pack.props.push(markChest(`trial_${n}`, x + 12, y + 6, [{ id: "apple", qty: 2 }]));
  pack.props.push({
    id: `trial${n}_sign`,
    kind: "sign",
    ...cellWorld(x + 2, y + 2),
    texture: "sign",
    talk: "trial_push",
  });
}

function leverTrial(pack: TrialPack, n: number, x: number, y: number): void {
  pack.props.push({
    id: `trial${n}_gate`,
    kind: "door",
    ...cellWorld(x + 8, y + 4),
    texture: "doorway",
    locked: true,
    lockHint: "The mark is behind a vote.",
  });
  pack.props.push({
    id: `trial${n}_lever`,
    kind: "toggle",
    ...cellWorld(x + 3, y + 10),
    texture: "lever",
    actions: [`unlock:trial${n}_gate`, "note:The gate forgets itself."],
  });
  pack.props.push(markChest(`trial_${n}`, x + 12, y + 3, [{ id: "small_water", qty: 1 }]));
  pack.props.push({
    id: `trial${n}_sign`,
    kind: "sign",
    ...cellWorld(x + 2, y + 2),
    texture: "sign",
    talk: "trial_lever",
  });
}

function clearTrial(pack: TrialPack, n: number, x: number, y: number): void {
  const a = `trial${n}_a`;
  const b = `trial${n}_b`;
  pack.enemies.push(
    { id: a, kind: "skeleton", ...cellWorld(x + 6, y + 6), respawn: 0 },
    { id: b, kind: "bandit", ...cellWorld(x + 10, y + 8), respawn: 0 },
  );
  pack.props.push({
    id: `trial${n}_gate`,
    kind: "door",
    ...cellWorld(x + 8, y + 3),
    texture: "doorway",
    locked: true,
    lockHint: "They have to go down first.",
  });
  pack.props.push(markChest(`trial_${n}`, x + 8, y + 2, [{ id: "bone_chip", qty: 1 }]));
  pack.triggers.push(
    makeTrigger(`trial${n}_slam`, "enter", [`lock:trial${n}_gate`, "note:The ring closes."], {
      x: cellWorld(x + 8, y + 7).x,
      y: cellWorld(x + 8, y + 7).y,
      w: 64,
      h: 64,
    }),
    makeTrigger(`trial${n}_clear`, "clear", [`unlock:trial${n}_gate`, "note:The ring opens."], { watch: [a, b] }),
  );
}

function drainTrial(pack: TrialPack, n: number, x: number, y: number, grid: Grid): void {
  const flood = { id: `trial_flood_${n}`, x: x + 4, y: y + 3, w: 10, h: 8 };
  grid.fillRect(flood.x, flood.y, flood.w, flood.h, "water");
  grid.fillRect(x + 8, y + 6, 2, 2, "stone");
  pack.floods.push(flood);
  pack.props.push({
    id: `trial${n}_lever`,
    kind: "toggle",
    ...cellWorld(x + 2, y + 11),
    texture: "lever",
    actions: [`drain:trial_flood_${n}`, "note:The water takes a job elsewhere."],
  });
  pack.props.push(markChest(`trial_${n}`, x + 8, y + 6, [{ id: "night_lich_moss", qty: 1 }]));
  pack.props.push({
    id: `trial${n}_sign`,
    kind: "sign",
    ...cellWorld(x + 2, y + 2),
    texture: "sign",
    talk: "trial_drain",
  });
}

function growTrial(pack: TrialPack, n: number, x: number, y: number, grid: Grid): void {
  grid.fillRect(x + 3, y + 3, 11, 8, "water");
  grid.fillRect(x + 11, y + 6, 2, 2, "grass");
  pack.props.push(markChest(`trial_${n}`, x + 12, y + 6, [{ id: "honeylace_lily", qty: 1 }]));
  pack.props.push({
    id: `trial${n}_sign`,
    kind: "sign",
    ...cellWorld(x + 2, y + 2),
    texture: "sign",
    talk: "trial_grow",
  });
}

export function stampCamps(grid: Grid, rng: () => number): TrialPack {
  const pack: TrialPack = { props: [], enemies: [], triggers: [], floods: [], lights: [] };
  const spots = [
    { cx: 280, cy: 240 },
    { cx: 700, cy: 340 },
    { cx: 1100, cy: 280 },
    { cx: 900, cy: 700 },
    { cx: 1500, cy: 640 },
    { cx: 480, cy: 900 },
    { cx: 1650, cy: 360 },
    { cx: 130, cy: 560 },
    { cx: 400, cy: 200 },
    { cx: 600, cy: 300 },
    { cx: 850, cy: 450 },
    { cx: 1000, cy: 200 },
    { cx: 1250, cy: 480 },
    { cx: 1400, cy: 720 },
    { cx: 1580, cy: 400 },
    { cx: 1750, cy: 600 },
    { cx: 320, cy: 800 },
    { cx: 720, cy: 880 },
    { cx: 1100, cy: 860 },
    { cx: 1900, cy: 500 },
    { cx: 200, cy: 300 },
    { cx: 480, cy: 500 },
    { cx: 980, cy: 980 },
    { cx: 1680, cy: 780 },
  ];
  let i = 0;
  for (const s of spots) {
    if (i >= 16) break;
    if (!fits(grid, s.cx, s.cy, 12, 12)) continue;
    fenceRing(grid, s.cx, s.cy, 12, 12, 2);
    pack.props.push({
      id: `camp${i}_fire`,
      kind: "campfire",
      ...cellWorld(s.cx + 6, s.cy + 6),
      texture: "campfire",
    });
    pack.props.push({
      id: `camp${i}_chest`,
      kind: "chest",
      ...cellWorld(s.cx + 9, s.cy + 3),
      texture: "chest",
      loot: [
        { id: "apple", qty: 2 },
        { id: rng() < 0.5 ? "gold_dust" : "wood", qty: 1 },
      ],
    });
    pack.enemies.push(
      { id: `camp${i}_a`, kind: i % 3 === 0 ? "wolf" : i % 2 === 0 ? "bandit" : "skeleton", ...cellWorld(s.cx + 3, s.cy + 4), respawn: 50 },
      { id: `camp${i}_b`, kind: i % 3 === 0 ? "crow" : i % 2 === 0 ? "bandit" : "soldier", ...cellWorld(s.cx + 8, s.cy + 8), respawn: 50 },
      { id: `camp${i}_c`, kind: "rat", ...cellWorld(s.cx + 5, s.cy + 9), respawn: 40 },
    );
    pack.lights.push({ ...cellWorld(s.cx + 6, s.cy + 6), radius: 28 });
    i += 1;
  }
  return pack;
}
