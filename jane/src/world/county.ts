// The county: 640 x 384 cells = 5120 x 3072 px, the size of 2020's room_zone1.
// (The 2026 Phaser build blew this up to 2000 x 1200 and then had to fill it.)
//
// Same story every seed. Two camps, as 2020 placed them: the town in the north-west,
// Auntie Julie's yard far to the south-east, a road and a river between. Required
// pockets (stoop, mine mouth, burial stair) are fixed ids at jittered positions;
// lots, clutter, herbs and wildlife roll.

import { Tile } from "@/sim/grid";
import type { Blueprint, Rect } from "@/world/blueprint";
import { Kit } from "@/world/kit";

export const COUNTY_W = 640;
export const COUNTY_H = 384;

const HOUSE_SIZES: [number, number][] = [
  [14, 18],
  [22, 12],
  [20, 24],
];
const FIELD_HERBS = ["pansy", "nasturtium", "honeylace_lily", "hemshade_root", "white_water_cap", "night_lich_moss"];

export function buildCounty(seed: number, attempt: number): Blueprint {
  const k = new Kit("county", COUNTY_W, COUNTY_H, seed, attempt, Tile.Grass);
  const all: Rect = { cx: 0, cy: 0, w: COUNTY_W, h: COUNTY_H };

  // Ground cover first, so everything later paints over it.
  k.sprinkle(all, Tile.Grass, Tile.GrassTall, 0.08);
  for (let n = 0; n < 70; n++) k.blob(k.int(12, COUNTY_W - 12), k.int(12, COUNTY_H - 12), k.int(4, 14), Tile.Grass, Tile.GrassTall);

  // River, north to south through the middle, with the road bridges cut later.
  const riverX = 318 + k.int(-10, 10);
  k.stroke(
    [
      [riverX - 30, 0],
      [riverX, 120],
      [riverX + 14, 250],
      [riverX - 8, COUNTY_H - 1],
    ],
    9,
    Tile.Water,
    2,
  );

  // --- Auntie Julie's yard (south-east camp) --------------------------------
  const yard: Rect = { cx: 492 + k.int(-6, 6), cy: 288 + k.int(-4, 4), w: 96, h: 70 };
  k.fill(yard.cx, yard.cy, yard.w, yard.h, Tile.Grass);
  k.fenceRing(yard, "w", 6);
  const hw = 26;
  const hh = 20;
  const hx = yard.cx + 40;
  const hy = yard.cy + 6;
  k.house(hx, hy, hw, hh);
  const doorX = hx + Math.floor(hw / 2) - 1;
  const doorY = hy + hh - 2;
  k.prop(
    { key: "house_door", def: "door", cx: doorX, cy: doorY, locked: true, keyTag: "auntie_house", to: { zone: "house", mark: "front" }, label: "Julie's door" },
    2,
    2,
  );
  k.fill(doorX - 3, hy + hh, 8, 5, Tile.Dirt);
  k.rect("stoop", { cx: doorX - 8, cy: hy + hh, w: 18, h: 10 });
  k.mark("house_front", doorX, hy + hh + 1, 1);
  k.unit("dog", "dog", doorX + 4, hy + hh + 3);
  k.prop({ def: "lamp_post", cx: doorX - 3, cy: hy + hh + 1 }, 1, 1);
  // A fire in the yard: the first place to rest, before the house key and its bed.
  k.prop({ key: "yard_fire", def: "campfire", cx: doorX + 12, cy: hy + hh + 6, talk: "fire" }, 2, 2);
  k.prop({ def: "barrel", cx: doorX - 6, cy: hy + hh + 1 }, 2, 2);
  k.prop({ def: "barrel", cx: doorX + 8, cy: hy + hh + 1 }, 2, 2);
  k.prop({ def: "apple_tree", cx: hx - 8, cy: hy + 10, loot: [{ item: "apple", qty: 3 }] }, 2, 2);
  // Path from the gate to the stoop; Jane arrives just inside the gate, not on the stoop:
  // the letter quest should complete after a walk, not on frame one.
  const gateY = yard.cy + Math.floor(yard.h / 2);
  k.corridor(yard.cx + 1, gateY, doorX, hy + hh + 2, 3, Tile.Dirt);
  k.mark("start", yard.cx + 5, gateY, 0);
  // The quest skeleton stands in the yard's far corner, in sight of the stoop.
  k.unit("yard_skeleton", "skeleton", yard.cx + yard.w - 14, yard.cy + yard.h - 12);

  // --- Mine mouth: a cliff face north-east of the yard -------------------------
  const mx = yard.cx + yard.w + 10;
  const my = yard.cy - 8;
  k.fill(mx - 4, my - 2, 26, 18, Tile.Dirt);
  k.fill(mx, my, 18, 8, Tile.Cliff);
  k.prop({ key: "mine_door", def: "mine_mouth", cx: mx + 7, cy: my + 6, to: { zone: "mine", mark: "entry" } }, 3, 2);
  k.mark("mine_mouth", mx + 8, my + 10, 1);
  k.prop({ key: "sign_mine", def: "sign", cx: mx + 12, cy: my + 9, talk: "sign_mine" }, 2, 1);
  k.prop({ def: "minecart", cx: mx + 2, cy: my + 11 }, 2, 2);
  k.prop({ key: "mine_fire", def: "campfire", cx: mx + 16, cy: my + 12, talk: "fire" }, 2, 2);
  k.corridor(yard.cx + yard.w - 1, gateY, mx + 8, my + 12, 3, Tile.Dirt);
  k.fill(yard.cx + yard.w - 1, gateY - 2, 1, 5, Tile.Dirt); // east gap in the fence

  // --- Burial stair: a walled graveyard south-west of the yard -----------------
  const gx = yard.cx - 58;
  const gy = yard.cy + 30;
  const grave: Rect = { cx: gx, cy: gy, w: 40, h: 30 };
  k.fill(grave.cx, grave.cy, grave.w, grave.h, Tile.Cobble);
  k.outline(grave.cx, grave.cy, grave.w, grave.h, Tile.TempleWall);
  k.fill(grave.cx + grave.w - 1, grave.cy + 12, 1, 5, Tile.Cobble); // east gate
  k.fill(gx + 16, gy + 3, 9, 6, Tile.TempleWall);
  k.prop({ key: "burial_door", def: "burial_mouth", cx: gx + 19, cy: gy + 7, to: { zone: "burial", mark: "entry" } }, 3, 2);
  k.mark("burial_mouth", gx + 20, gy + 11, 1);
  k.prop({ key: "sign_burial", def: "sign", cx: gx + 26, cy: gy + 10, talk: "sign_burial" }, 2, 1);
  for (let n = 0; n < 6; n++) {
    const s = k.spot({ cx: gx + 3, cy: gy + 12, w: grave.w - 8, h: grave.h - 16 }, 2, 3, 1);
    if (s) k.prop({ def: "coffin", cx: s.cx, cy: s.cy }, 2, 3);
  }
  k.unit(null, "skeleton", gx + 8, gy + 20);
  k.unit(null, "skeleton", gx + 30, gy + 22);
  k.corridor(grave.cx + grave.w, grave.cy + 14, yard.cx + 1, gateY, 3, Tile.Dirt);

  // --- Town (north-west camp) ------------------------------------------------
  const town: Rect = { cx: 56 + k.int(-6, 6), cy: 44 + k.int(-4, 4), w: 190, h: 120 };
  const mainY = town.cy + Math.floor(town.h / 2);
  k.fill(town.cx - 20, mainY - 2, town.w + 40, 5, Tile.Road);
  k.fill(town.cx + 60, town.cy - 4, 5, town.h + 8, Tile.Road);
  k.fill(town.cx + 126, town.cy - 4, 5, town.h + 8, Tile.Road);
  // Station: where the Sunday train stops. One train a week.
  k.fill(town.cx - 34, mainY - 8, 16, 17, Tile.Cobble);
  k.fill(0, mainY - 12, town.cx - 18, 2, Tile.Rail);
  k.prop({ key: "sign_town", def: "sign", cx: town.cx - 16, cy: mainY + 4, talk: "sign_town" }, 2, 1);
  k.prop({ key: "station_fire", def: "campfire", cx: town.cx - 28, cy: mainY + 4, talk: "fire" }, 2, 2);
  const lots: Rect[] = [];
  for (let row = 0; row < 2; row++) {
    for (let col = 0; col < 3; col++) {
      lots.push({ cx: town.cx + col * 66 + 4, cy: row === 0 ? town.cy + 2 : mainY + 6, w: 52, h: 50 });
    }
  }
  for (const lot of lots) {
    k.fill(lot.cx, lot.cy, lot.w, lot.h, Tile.Grass);
    k.fenceRing(lot, lot.cy < mainY ? "s" : "n", 5);
    const [w, h] = k.pick(HOUSE_SIZES);
    k.house(lot.cx + k.int(6, lot.w - w - 6), lot.cy + k.int(6, lot.h - h - 8), w, h);
    if (k.chance(0.6)) {
      const s = k.spot(lot, 2, 2, 1);
      if (s) k.prop({ def: "apple_tree", cx: s.cx, cy: s.cy, loot: [{ item: "apple", qty: 2 }] }, 2, 2);
    }
    if (k.chance(0.7)) k.pile({ cx: lot.cx + 2, cy: lot.cy + 2, w: lot.w - 4, h: lot.h - 4 }, "barrel", k.int(1, 2));
  }
  for (let x = town.cx - 10; x < town.cx + town.w + 10; x += 30) {
    if (k.fits(x, mainY - 4, 1, 1)) k.prop({ def: "lamp_post", cx: x, cy: mainY - 4 }, 1, 1);
  }

  // --- The road between the camps, with bridges where it meets the river --------
  const road = k.stroke(
    [
      [town.cx + town.w + 18, mainY],
      [riverX - 24, mainY + 30],
      [riverX + 30, mainY + 60],
      [yard.cx - 40, gateY - 40],
      [yard.cx - 12, gateY],
      [yard.cx + 1, gateY],
    ],
    4,
    Tile.Road,
    1,
  );
  // Lamps and a bandit pair along it; the pair waits near the bridge, as 2020's did near town.
  for (let n = 40; n < road.length - 30; n += 56) {
    const [x, y] = road[n];
    if (k.fits(x, y - 4, 1, 1)) k.prop({ def: "lamp_post", cx: x, cy: y - 4 }, 1, 1);
  }
  const ambush = road[Math.floor(road.length * 0.62)];
  k.unit("bandit_a", "bandit", ambush[0] + 3, ambush[1] - 5);
  k.unit("bandit_b", "bandit", ambush[0] - 4, ambush[1] + 6);
  const beat = road[Math.floor(road.length * 0.18)];
  k.unit("road_skeleton", "skeleton", beat[0], beat[1] + 6, [
    [beat[0], beat[1] + 6],
    [beat[0] + 26, beat[1] + 8],
    [beat[0] + 26, beat[1] + 22],
    [beat[0], beat[1] + 20],
  ]);

  // --- Forest, south-west: the abandoned car from the town map -----------------
  const forest: Rect = { cx: 30, cy: 230, w: 210, h: 130 };
  for (let n = 0; n < 60; n++) k.blob(k.int(forest.cx, forest.cx + forest.w), k.int(forest.cy, forest.cy + forest.h), k.int(3, 9), Tile.Grass, Tile.Tree);
  for (let n = 0; n < 40; n++) k.blob(k.int(forest.cx, forest.cx + forest.w), k.int(forest.cy, forest.cy + forest.h), k.int(3, 9), Tile.GrassTall, Tile.Tree);
  const carAt = k.spot({ cx: forest.cx + 60, cy: forest.cy + 40, w: 80, h: 50 }, 5, 3, 2, 200);
  if (carAt) {
    k.fill(carAt.cx - 3, carAt.cy - 3, 11, 9, Tile.Dirt);
    k.prop(
      {
        key: "car_wreck",
        def: "car_wreck",
        cx: carAt.cx,
        cy: carAt.cy,
        loot: [
          { item: "wood", qty: 2 },
          { item: "small_water", qty: 2 },
          { item: "fire_stone", qty: 1 },
        ],
      },
      5,
      3,
    );
  }

  // Authored ground is off limits to scatter: no tree ever grows on the stoop.
  k.claim(yard.cx - 2, yard.cy - 2, yard.w + 4, yard.h + 4);
  k.claim(grave.cx - 2, grave.cy - 2, grave.w + 4, grave.h + 4);
  k.claim(mx - 6, my - 4, 30, 22);
  for (const [x, y] of road) k.claim(x - 3, y - 3, 7, 7);

  // --- Scatter: trees, herbs, rocks --------------------------------------------
  for (let n = 0; n < 220; n++) {
    const x = k.int(8, COUNTY_W - 9);
    const y = k.int(8, COUNTY_H - 9);
    if (k.get(x, y) === Tile.Grass && k.fits(x - 1, y - 1, 3, 3)) k.blob(x, y, k.int(1, 3), Tile.Grass, Tile.Tree);
  }
  for (let n = 0; n < 40; n++) {
    const s = k.spot({ cx: 10, cy: 10, w: COUNTY_W - 20, h: COUNTY_H - 20 }, 1, 1, 1);
    if (s && (k.get(s.cx, s.cy) === Tile.Grass || k.get(s.cx, s.cy) === Tile.GrassTall)) {
      k.prop({ def: "herb", cx: s.cx, cy: s.cy, loot: [{ item: k.pick(FIELD_HERBS), qty: 1 }] }, 1, 1);
    }
  }
  for (let n = 0; n < 14; n++) {
    const s = k.spot({ cx: 10, cy: 10, w: COUNTY_W - 20, h: COUNTY_H - 20 }, 1, 1, 1);
    if (s) k.prop({ def: "rock", cx: s.cx, cy: s.cy }, 1, 1);
  }

  // Tree line round the edge: the county is a bowl, not a plane.
  k.fill(0, 0, COUNTY_W, 4, Tile.Tree);
  k.fill(0, COUNTY_H - 4, COUNTY_W, 4, Tile.Tree);
  k.fill(0, 0, 4, COUNTY_H, Tile.Tree);
  k.fill(COUNTY_W - 4, 0, 4, COUNTY_H, Tile.Tree);

  return k.done("Castle", false, 1, attempt);
}
