import { COUNTY_COLS, COUNTY_ROWS } from "@/game/constants";
import { stampTrigger } from "@/game/systems/triggers";
import { createRng } from "@/game/systems/rng";
import { Grid } from "@/game/world/Grid";
import {
  fenceRing,
  houseStamp,
  HOUSE_STAMPS,
  lotFits,
  paintBorder,
  scatterTile,
  wiggleRoad,
  wiggleStroke,
} from "@/game/world/motifs";
import { stampTowns } from "@/game/world/generateTowns";
import { stampCamps, stampTrials } from "@/game/world/generateTrials";
import { stampWaysides } from "@/game/world/generateWaysides";
import { cellWorld, getPocket, stampPocket, stampPocketSoft } from "@/game/world/stamp";
import type { EnemySpec, PropSpec, ZoneBlue } from "@/game/world/zoneTypes";

export function generateTownYard(seed: number): ZoneBlue {
  const rng = createRng(seed);
  const grid = new Grid(COUNTY_COLS, COUNTY_ROWS, "grass");
  paintBorder(grid);

  const town = { cx: 180, cy: 160 };
  const auntie = { cx: 1780, cy: 1040 };
  const museum = { cx: 720, cy: 400 };
  const farm = { cx: 540, cy: 640 };
  const factory = { cx: 1120, cy: 680 };
  const rivermill = { cx: 760, cy: 360 };
  const acreton = { cx: 560, cy: 700 };
  const kiln = { cx: 1160, cy: 640 };
  const boneford = { cx: 1460, cy: 860 };
  const ridgegate = { cx: 1720, cy: 380 };

  const roads = [
    wiggleRoad(grid, town, auntie, rng, 3),
    wiggleRoad(grid, town, ridgegate, rng, 3),
    wiggleRoad(grid, town, museum, rng, 2),
    wiggleRoad(grid, museum, factory, rng, 2),
    wiggleRoad(grid, town, { cx: 300, cy: 70 }, rng, 2),
    wiggleRoad(grid, town, rivermill, rng, 2),
    wiggleRoad(grid, rivermill, acreton, rng, 2),
    wiggleRoad(grid, acreton, factory, rng, 2),
    wiggleRoad(grid, factory, boneford, rng, 2),
    wiggleRoad(grid, rivermill, kiln, rng, 2),
    wiggleRoad(grid, ridgegate, kiln, rng, 2),
    wiggleRoad(grid, ridgegate, boneford, rng, 2),
  ];
  const road = roads[0];
  const river = wiggleStroke(grid, { cx: 40, cy: 280 }, { cx: 1550, cy: 1080 }, rng, 4, "water");

  scatterTile(grid, 220, 520, 180, 220, "bush", 90, rng);
  scatterTile(grid, 40, 40, 80, 60, "bush", 18, rng);
  scatterTile(grid, 1680, 200, 280, 700, "stone", 140, rng);
  scatterTile(grid, 1360, 780, 280, 280, "bone", 80, rng);
  scatterTile(grid, 440, 540, 220, 220, "bush", 50, rng);
  scatterTile(grid, 980, 580, 280, 220, "stone", 40, rng);
  scatterTile(grid, 200, 200, 400, 180, "bush", 70, rng);
  scatterTile(grid, 800, 80, 700, 180, "bush", 50, rng);
  scatterTile(grid, 400, 380, 280, 220, "bush", 55, rng);
  scatterTile(grid, 800, 400, 400, 180, "stone", 36, rng);
  scatterTile(grid, 1200, 200, 400, 240, "stone", 50, rng);
  scatterTile(grid, 900, 720, 400, 260, "stone", 40, rng);
  scatterTile(grid, 200, 700, 350, 280, "bush", 80, rng);
  scatterTile(grid, 600, 900, 500, 180, "bush", 40, rng);
  scatterTile(grid, 1500, 480, 260, 180, "stone", 32, rng);

  const lotCount = 5 + Math.floor(rng() * 5);
  for (let i = 0; i < lotCount; i++) {
    const stamp = HOUSE_STAMPS[Math.floor(rng() * HOUSE_STAMPS.length)];
    const lx = town.cx + Math.floor(rng() * 110) - 20;
    const ly = town.cy + 8 + Math.floor(rng() * 80);
    if (!lotFits(grid, lx, ly, stamp.w + 4, stamp.h + 4)) continue;
    fenceRing(grid, lx, ly, stamp.w + 4, stamp.h + 4);
    houseStamp(grid, lx + 2, ly + 1, stamp.w, stamp.h);
  }

  grid.fillRect(auntie.cx - 14, auntie.cy + 8, 5, 4, "water");

  const required = [
    { id: "stoop", cx: auntie.cx - 4, cy: auntie.cy - 4, jitter: 2 },
    { id: "mine_mouth", cx: auntie.cx - 28, cy: auntie.cy - 18, jitter: 2 },
    { id: "burial_mouth", cx: auntie.cx + 18, cy: auntie.cy - 10, jitter: 2 },
    { id: "factory_front", cx: factory.cx, cy: factory.cy, jitter: 3 },
    { id: "school_front", cx: 300, cy: 70, jitter: 2 },
    { id: "butterfly_front", cx: 380, cy: 500, jitter: 4 },
  ];
  const optional = [
    { id: "train_station", cx: 48, cy: 48, jitter: 2 },
    { id: "shop_front", cx: 130, cy: 210, jitter: 3 },
    { id: "gym_front", cx: 220, cy: 70, jitter: 3 },
    { id: "library_front", cx: 340, cy: 160, jitter: 3 },
    { id: "museum_front", cx: museum.cx, cy: museum.cy, jitter: 3 },
    { id: "farm_front", cx: farm.cx, cy: farm.cy, jitter: 4 },
    { id: "abandoned_front", cx: 260, cy: 720, jitter: 4 },
    { id: "pipes_grate", cx: 1080, cy: 780, jitter: 4 },
    { id: "graveyard_front", cx: 1480, cy: 880, jitter: 4 },
    { id: "car_wreck", cx: 90, cy: 420, jitter: 3 },
    { id: "cave_mouth", cx: 1880, cy: 860, jitter: 3 },
    { id: "picnic_spot", cx: auntie.cx - 40, cy: auntie.cy - 8, jitter: 3 },
    { id: "hedge_notice", cx: town.cx - 20, cy: town.cy + 40, jitter: 2 },
  ];

  const props: PropSpec[] = [];
  const enemies: EnemySpec[] = [];
  let mineOrigin = { cx: auntie.cx - 28, cy: auntie.cy - 18 };

  for (const place of required) {
    const stamped = stampPocket(grid, getPocket(place.id), place.cx, place.cy, rng, place.jitter);
    props.push(...stamped.props);
    enemies.push(...stamped.enemies);
    if (place.id === "mine_mouth") mineOrigin = stamped.origin;
  }
  for (const place of optional) {
    const stamped = stampPocketSoft(grid, getPocket(place.id), place.cx, place.cy, rng, place.jitter);
    if (!stamped) continue;
    props.push(...stamped.props);
    enemies.push(...stamped.enemies);
  }

  const riverSpot = river[Math.floor(river.length * 0.45)];
  if (riverSpot) {
    const board = stampPocketSoft(grid, getPocket("river_board"), riverSpot.cx + 6, riverSpot.cy - 4, rng, 3);
    if (board) props.push(...board.props);
  }

  const rim = sampleRim(grid, rng, 110);
  for (const cell of rim) {
    if (grid.get(cell.cx, cell.cy) !== "grass") continue;
    grid.set(cell.cx, cell.cy, "bush");
  }

  const herbIds = [
    "pansy",
    "nasturtium",
    "honeylace_lily",
    "white_water_cap",
    "night_lich_moss",
    "berry",
    "mushroom",
    "carrot",
    "onion",
    "potato",
    "wheat",
    "honey",
    "tea_leaf",
    "pear",
    "plum",
    "seed",
    "cabbage",
  ];
  const settlements = stampTowns(grid, rng);
  const trials = stampTrials(grid, rng);
  const camps = stampCamps(grid, rng);
  const ways = stampWaysides(grid, roads, rng);
  props.push(...settlements.props, ...trials.props, ...camps.props, ...ways.props);
  enemies.push(...settlements.enemies, ...trials.enemies, ...camps.enemies, ...ways.enemies);

  const herbs = [
    ...sampleOpen(grid, rng, 18, town.cx, town.cy, 200, 160),
    ...sampleOpen(grid, rng, 42, 40, 40, grid.cols - 80, grid.rows - 80),
  ];
  herbs.forEach((h, i) => {
    props.push({
      id: `herb_${i}`,
      kind: "drop",
      x: h.x,
      y: h.y,
      texture: "drop",
      loot: [{ id: herbIds[i % herbIds.length], qty: 1 }],
    });
  });

  const banditA = road[Math.floor(road.length * 0.22)] ?? town;
  const banditB = road[Math.floor(road.length * 0.28)] ?? town;
  const banditC = road[Math.floor(road.length * 0.55)] ?? town;
  const skel = road[Math.floor(road.length * 0.18)] ?? town;
  const yardA = { cx: auntie.cx + 16, cy: auntie.cy + 14 };
  const yardB = { cx: auntie.cx + 28, cy: auntie.cy + 8 };

  enemies.push(
    { id: "town_bandit_a", kind: "bandit", ...cellWorld(banditA.cx, banditA.cy) },
    { id: "town_bandit_b", kind: "bandit", ...cellWorld(banditA.cx + 3, banditA.cy + 1) },
    { id: "road_bandit_a", kind: "bandit", ...cellWorld(banditB.cx, banditB.cy) },
    { id: "road_bandit_b", kind: "bandit", ...cellWorld(banditB.cx + 2, banditB.cy - 1) },
    { id: "mid_bandit_a", kind: "bandit", ...cellWorld(banditC.cx, banditC.cy) },
    { id: "mid_bandit_b", kind: "bandit", ...cellWorld(banditC.cx - 2, banditC.cy + 2) },
    {
      id: "skeleton",
      kind: "skeleton",
      ...cellWorld(yardA.cx, yardA.cy),
      patrol: [cellWorld(yardA.cx, yardA.cy), cellWorld(yardA.cx + 8, yardA.cy + 4)],
    },
    { id: "yard_skel_b", kind: "skeleton", ...cellWorld(yardB.cx, yardB.cy) },
    {
      id: "road_skel",
      kind: "skeleton",
      ...cellWorld(skel.cx + 4, skel.cy + 2),
      patrol: [cellWorld(skel.cx + 4, skel.cy + 2), cellWorld(skel.cx + 12, skel.cy + 6)],
    },
    { id: "forest_wolf_a", kind: "wolf", ...cellWorld(320, 620) },
    { id: "forest_wolf_b", kind: "wolf", ...cellWorld(410, 740) },
    { id: "farm_crow_a", kind: "crow", ...cellWorld(560, 600) },
    { id: "farm_crow_b", kind: "crow", ...cellWorld(600, 680) },
    { id: "mine_ridge_bat_a", kind: "bat", ...cellWorld(1720, 900) },
    { id: "mine_ridge_bat_b", kind: "bat", ...cellWorld(1840, 820) },
    { id: "river_slime_a", kind: "slime", ...cellWorld(900, 500) },
    { id: "river_slime_b", kind: "slime", ...cellWorld(1100, 720) },
    { id: "grave_flower_a", kind: "flower", ...cellWorld(1500, 920) },
    { id: "grave_pumpkin_a", kind: "pumpkin", ...cellWorld(1580, 860) },
    { id: "ridge_wolf_c", kind: "hermit_wolf", ...cellWorld(1600, 320), respawn: 55 },
    { id: "ridge_wolf_d", kind: "hermit_wolf", ...cellWorld(1840, 440), respawn: 55 },
    { id: "kiln_brigand_a", kind: "brigand", ...cellWorld(1080, 600), respawn: 50 },
    { id: "kiln_brigand_b", kind: "brigand", ...cellWorld(1220, 700), respawn: 50 },
    { id: "mill_toad_a", kind: "river_toad", ...cellWorld(820, 400), respawn: 40 },
    { id: "mill_toad_b", kind: "marsh_slime", ...cellWorld(700, 300), respawn: 40 },
    { id: "acre_boar_a", kind: "boar", ...cellWorld(500, 760), respawn: 48 },
    { id: "acre_fox_a", kind: "fox", ...cellWorld(620, 640), respawn: 40 },
    { id: "forest_boar_a", kind: "boar", ...cellWorld(280, 860), respawn: 48 },
    { id: "forest_fox_a", kind: "fox", ...cellWorld(180, 700), respawn: 40 },
    { id: "bone_ghoul_a", kind: "ghoul", ...cellWorld(1380, 900), respawn: 50 },
    { id: "bone_ghoul_b", kind: "wight", ...cellWorld(1520, 800), respawn: 60 },
  );

  const dog = props.find((p) => p.id === "dog");
  const spawn = dog ? { x: dog.x, y: dog.y + 24 } : cellWorld(auntie.cx, auntie.cy + 8);
  const mineDoor = props.find((p) => p.id === "mine_mouth");
  const scarecrow = props.find((p) => p.id === "scarecrow");
  const factoryDoor = props.find((p) => p.id === "factory_door");
  const schoolDoor = props.find((p) => p.id === "school_door");
  const forestDoor = props.find((p) => p.id === "butterfly_mouth");

  return {
    grid,
    spawn,
    required: ["house_door", "dog", "skeleton", "factory_door", "school_door", "butterfly_mouth"],
    floods: trials.floods,
    triggers: [
      stampTrigger("arrive_stoop", spawn.x, spawn.y, 64, 64),
      stampTrigger("see_mine_mouth", mineDoor?.x ?? spawn.x, mineDoor?.y ?? spawn.y, 56, 56),
      ...(scarecrow ? [stampTrigger("into_the_farm", scarecrow.x, scarecrow.y, 56, 56)] : []),
      ...(factoryDoor ? [stampTrigger("mouth_factory", factoryDoor.x, factoryDoor.y, 48, 48)] : []),
      ...(schoolDoor ? [stampTrigger("mouth_school", schoolDoor.x, schoolDoor.y, 48, 48)] : []),
      ...(forestDoor ? [stampTrigger("mouth_forest", forestDoor.x, forestDoor.y, 48, 48)] : []),
      ...settlements.triggers,
      ...trials.triggers,
    ],
    lights: [
      { x: spawn.x, y: spawn.y, radius: 18 },
      { ...cellWorld(mineOrigin.cx + 4, mineOrigin.cy + 2), radius: 24 },
      ...settlements.lights,
      ...camps.lights,
      ...trials.lights,
      ...ways.lights,
    ],
    enemies,
    props,
  };
}

function sampleRim(grid: Grid, rng: () => number, count: number): { cx: number; cy: number }[] {
  const out: { cx: number; cy: number }[] = [];
  const margin = 18;
  for (let i = 0; i < count; i++) {
    const edge = Math.floor(rng() * 4);
    if (edge === 0) out.push({ cx: 2 + Math.floor(rng() * (grid.cols - 4)), cy: 2 + Math.floor(rng() * margin) });
    else if (edge === 1) out.push({ cx: 2 + Math.floor(rng() * (grid.cols - 4)), cy: grid.rows - 3 - Math.floor(rng() * margin) });
    else if (edge === 2) out.push({ cx: 2 + Math.floor(rng() * margin), cy: 2 + Math.floor(rng() * (grid.rows - 4)) });
    else out.push({ cx: grid.cols - 3 - Math.floor(rng() * margin), cy: 2 + Math.floor(rng() * (grid.rows - 4)) });
  }
  return out;
}

function sampleOpen(
  grid: Grid,
  rng: () => number,
  count: number,
  x: number,
  y: number,
  w: number,
  h: number,
): { x: number; y: number }[] {
  const found: { x: number; y: number }[] = [];
  for (let i = 0; i < count * 8 && found.length < count; i++) {
    const cx = x + Math.floor(rng() * w);
    const cy = y + Math.floor(rng() * h);
    if (grid.solid(cx, cy)) continue;
    found.push(cellWorld(cx, cy));
  }
  return found;
}
