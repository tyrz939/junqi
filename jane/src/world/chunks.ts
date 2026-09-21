// Set chunks: the authored places the skeleton positions. "Consistent where it
// counts": the stoop, the mine mouth, the graveyard and the town are the same
// places on every seed; where they stand, and the road that finds them, are not.
//
// A chunk is a function of an origin. It clears its box, draws itself, and says
// where its GATES are: walkable cells on the outside of its box that its own paths
// lead in from. The county joins each arriving road to the nearest gate, going
// round the box, never through it, so no road ever cuts a fence or a kitchen.
//
// (PLAN.md meant these to become data, text grids like the art. They are code for
// now because every one of them already existed as code; the contract above is
// what the data format will have to keep.)

import { Tile } from "@/sim/grid";
import type { Rect } from "@/world/blueprint";
import type { Kit } from "@/world/kit";

export type Gate = [number, number];
/** `slots`: exact cells the chunk offers to content placed by name: where a door goes in a wall it built. */
export type Chunk = { id: string; box: Rect; gates: Gate[]; slots?: Record<string, [number, number]> };
type Build = (k: Kit, ox: number, oy: number) => Chunk;

const HOUSE_SIZES: [number, number][] = [
  [14, 18],
  [22, 12],
  [20, 24],
];

function clear(k: Kit, box: Rect, ground: Tile = Tile.Grass): void {
  k.fill(box.cx, box.cy, box.w, box.h, ground);
}

/** A box centred on the origin, kept inside the map with room for the ring road round it. */
function boxAt(k: Kit, ox: number, oy: number, w: number, h: number): Rect {
  const cx = Math.max(8, Math.min(k.w - w - 8, ox - Math.floor(w / 2)));
  const cy = Math.max(8, Math.min(k.h - h - 8, oy - Math.floor(h / 2)));
  return { cx, cy, w, h };
}

/** The middle of each side, one cell outside the box. */
function sideGates(box: Rect): { n: Gate; s: Gate; w: Gate; e: Gate } {
  const mx = box.cx + Math.floor(box.w / 2);
  const my = box.cy + Math.floor(box.h / 2);
  return { n: [mx, box.cy - 1], s: [mx, box.cy + box.h], w: [box.cx - 1, my], e: [box.cx + box.w, my] };
}

/** Castle Halt: where the Sunday train stops. One train a week. New Game stands her on the platform. */
export const station: Build = (k, _ox, oy) => {
  const box: Rect = { cx: 4, cy: Math.max(8, Math.min(k.h - 48, oy - 20)), w: 44, h: 40 };
  const my = box.cy + 20;
  clear(k, box);
  k.fill(4, box.cy - 40, 3, box.h + 80, Tile.Rail);
  k.fill(8, my - 9, 18, 19, Tile.Cobble);
  k.fill(26, my - 1, box.w - 22, 3, Tile.Dirt);
  k.mark("start", 16, my, 0);
  k.prop({ key: "sign_town", def: "sign", cx: 28, cy: my + 3, talk: "sign_town" }, 2, 1);
  k.prop({ key: "station_fire", def: "campfire", cx: 12, cy: my + 5, talk: "fire" }, 2, 2);
  k.prop({ key: "station_lamp", def: "lamp_post", cx: 25, cy: my - 4 }, 1, 1);
  k.prop({ key: "station_crate", def: "crate", cx: 10, cy: my - 7 }, 2, 2);
  k.rect("platform", { cx: 8, cy: my - 9, w: 18, h: 19 });
  // Wide on purpose: whatever changes on the platform changes while she is too far away to see it.
  k.rect("halt_approach", { cx: 0, cy: box.cy - 30, w: box.w + 34, h: box.h + 60 });
  k.mark("lost_property", 20, my - 6, 1);
  return { id: "station", box, gates: [[box.cx + box.w, my]], slots: { station_lamp_dead: [25, my - 4], night_parcel: [12, my - 5] } };
};

/** Auntie Julie's yard: the fence, the house, the stoop, the dog, the thing in the far corner. */
export const julieYard: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 96, 70);
  clear(k, box);
  k.outline(box.cx, box.cy, box.w, box.h, Tile.Fence);
  const hw = 26;
  const hh = 20;
  const hx = box.cx + 40;
  const hy = box.cy + 6;
  k.house(hx, hy, hw, hh);
  const doorX = hx + Math.floor(hw / 2) - 1;
  const doorY = hy + hh - 2;
  k.prop(
    { key: "house_door", def: "door", cx: doorX, cy: doorY, locked: true, keyTag: "auntie_house", to: { zone: "house", mark: "front" }, label: "Julie's door" },
    2,
    2,
  );
  const gateY = box.cy + Math.floor(box.h / 2);
  const stoopY = hy + hh + 2;
  // One lane across the yard under the stoop, gate to gate; the door's own path drops onto it.
  k.fill(box.cx, gateY - 1, box.w, 3, Tile.Dirt);
  k.fill(doorX - 3, hy + hh, 8, 5, Tile.Dirt);
  k.fill(doorX, stoopY, 3, gateY - stoopY + 1, Tile.Dirt);
  // And one down to the south gate, so whichever way the road comes there is a way in.
  const southX = box.cx + 20;
  k.fill(southX - 1, gateY, 3, box.cy + box.h - gateY, Tile.Dirt);
  k.rect("stoop", { cx: doorX - 8, cy: hy + hh, w: 18, h: 10 });
  k.mark("house_front", doorX, hy + hh + 1, 1);
  // Just inside the west gate: where the old small county began. Tests and the console still use it.
  k.mark("yard_gate", box.cx + 5, gateY, 0);
  k.unit("dog", "dog", doorX + 4, hy + hh + 3);
  k.prop({ def: "lamp_post", cx: doorX - 3, cy: hy + hh + 1 }, 1, 1);
  // A fire in the yard: the first place to rest, before the house key and its bed.
  k.prop({ key: "yard_fire", def: "campfire", cx: doorX + 12, cy: hy + hh + 6, talk: "fire" }, 2, 2);
  k.prop({ def: "barrel", cx: doorX - 6, cy: hy + hh + 1 }, 2, 2);
  k.prop({ def: "barrel", cx: doorX + 8, cy: hy + hh + 1 }, 2, 2);
  k.prop({ def: "apple_tree", cx: hx - 8, cy: hy + 10, loot: [{ item: "apple", qty: 3 }] }, 2, 2);
  // The quest skeleton stands in the yard's far corner, in sight of the stoop.
  k.unit("yard_skeleton", "skeleton", box.cx + box.w - 14, box.cy + box.h - 12);
  k.mark("garden_book", doorX + 11, hy + hh + 2, 1);
  return {
    id: "julie_house",
    box,
    gates: [
      [box.cx - 1, gateY],
      [box.cx + box.w, gateY],
      [southX, box.cy + box.h],
    ],
  };
};

/** Castle: a main street, two cross streets, six fenced lots, lamps. A haven. */
export const town: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 206, 124);
  clear(k, box);
  const mainY = box.cy + Math.floor(box.h / 2);
  const streetA = box.cx + 68;
  const streetB = box.cx + 136;
  k.fill(box.cx, mainY - 2, box.w, 5, Tile.Road);
  k.fill(streetA - 2, box.cy, 5, box.h, Tile.Road);
  k.fill(streetB - 2, box.cy, 5, box.h, Tile.Road);
  let allenDoor: [number, number] = [box.cx, box.cy];
  for (let row = 0; row < 2; row++) {
    for (let col = 0; col < 3; col++) {
      const lot: Rect = { cx: box.cx + col * 68 + 6, cy: row === 0 ? box.cy + 4 : mainY + 6, w: 54, h: 52 };
      k.fenceRing(lot, row === 0 ? "s" : "n", 5);
      const [w, h] = k.pick(HOUSE_SIZES);
      const hx = lot.cx + k.int(6, lot.w - w - 6);
      const hy = lot.cy + k.int(6, lot.h - h - 8);
      k.house(hx, hy, w, h);
      // The house north of the square, whose wall faces the fire: somebody still lives behind that door.
      if (row === 0 && col === 1) {
        allenDoor = [hx + Math.floor(w / 2) - 1, hy + h - 2];
        k.fill(allenDoor[0], hy + h, 2, lot.cy + lot.h - (hy + h), Tile.Dirt);
        k.mark("allen_door", allenDoor[0], hy + h + 1, 3);
      }
      if (k.chance(0.6)) {
        const s = k.spot(lot, 2, 2, 1);
        if (s) k.prop({ def: "apple_tree", cx: s.cx, cy: s.cy, loot: [{ item: "apple", qty: 2 }] }, 2, 2);
      }
      if (k.chance(0.7)) k.pile({ cx: lot.cx + 2, cy: lot.cy + 2, w: lot.w - 4, h: lot.h - 4 }, "barrel", k.int(1, 2));
      // The gap in each lot's fence opens onto the main street.
      const gapX = lot.cx + Math.floor(lot.w / 2);
      k.fill(gapX - 2, row === 0 ? lot.cy + lot.h : mainY + 3, 5, row === 0 ? mainY - 2 - (lot.cy + lot.h) : lot.cy - (mainY + 3), Tile.Dirt);
    }
  }
  for (let x = box.cx + 8; x < box.cx + box.w - 4; x += 30) {
    if (k.fits(x, mainY - 4, 1, 1)) k.prop({ def: "lamp_post", cx: x, cy: mainY - 4 }, 1, 1);
  }
  k.mark("town_square", streetA + 6, mainY, 1);
  k.prop({ key: "town_fire", def: "campfire", cx: streetA + 8, cy: mainY + 4, talk: "fire" }, 2, 2);
  return {
    id: "town",
    box,
    gates: [
      [box.cx - 1, mainY],
      [box.cx + box.w, mainY],
      [streetA, box.cy - 1],
      [streetA, box.cy + box.h],
      [streetB, box.cy - 1],
      [streetB, box.cy + box.h],
    ],
    slots: { allen_door: allenDoor },
  };
};

/** The Gold Mine: a cliff face, a mouth in it, a sign that is not sure you can leave. */
export const mineMouth: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 30, 24);
  clear(k, box, Tile.Dirt);
  const mx = box.cx + 6;
  const my = box.cy + 2;
  k.fill(mx, my, 18, 8, Tile.Cliff);
  k.prop({ key: "mine_door", def: "mine_mouth", cx: mx + 7, cy: my + 6, to: { zone: "mine", mark: "entry" } }, 3, 2);
  k.mark("mine_mouth", mx + 8, my + 10, 1);
  k.prop({ key: "sign_mine", def: "sign", cx: mx + 12, cy: my + 9, talk: "sign_mine" }, 2, 1);
  k.prop({ def: "minecart", cx: mx + 2, cy: my + 11 }, 2, 2);
  k.prop({ key: "mine_fire", def: "campfire", cx: mx + 16, cy: my + 14, talk: "fire" }, 2, 2);
  // The Company's pay hatch: a stub of wall with a gap in it, and nobody behind the gap.
  k.fill(box.cx + 1, my + 12, 1, 2, Tile.Wall);
  k.fill(box.cx + 3, my + 12, 1, 2, Tile.Wall);
  k.mark("company_notice", box.cx + 2, my + 15, 3);
  const g = sideGates(box);
  return { id: "gold_mine", box, gates: [g.s, g.w, g.e] };
};

/** The graveyard: walled, cobbled, tenanted. The Burial Chamber's stair is NOT here; it is off in the dark nearby. */
export const graveyard: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 44, 32);
  clear(k, box, Tile.Cobble);
  k.outline(box.cx, box.cy, box.w, box.h, Tile.TempleWall);
  const my = box.cy + Math.floor(box.h / 2);
  k.fill(box.cx, my - 2, 1, 5, Tile.Cobble);
  k.fill(box.cx + box.w - 1, my - 2, 1, 5, Tile.Cobble);
  k.mark("graveyard_gate", box.cx + 3, my, 0);
  for (let n = 0; n < 7; n++) {
    const s = k.spot({ cx: box.cx + 4, cy: box.cy + 3, w: box.w - 8, h: box.h - 6 }, 2, 3, 1);
    if (s && Math.abs(s.cy + 1 - my) > 4) k.prop({ def: "coffin", cx: s.cx, cy: s.cy }, 2, 3);
  }
  k.unit(null, "skeleton", box.cx + 10, box.cy + 7);
  k.unit(null, "skeleton", box.cx + box.w - 12, box.cy + box.h - 8);
  const g = sideGates(box);
  return { id: "graveyard", box, gates: [g.w, g.e] };
};

/** The stair down: a block of temple wall in open ground, reached by a footpath, never by a road. */
export const burialStair: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 19, 16);
  clear(k, box, Tile.Cobble);
  const bx = box.cx + 5;
  const by = box.cy + 1;
  k.fill(bx, by, 9, 6, Tile.TempleWall);
  k.prop({ key: "burial_door", def: "burial_mouth", cx: bx + 3, cy: by + 4, to: { zone: "burial", mark: "entry" } }, 3, 2);
  k.mark("burial_mouth", bx + 4, by + 8, 1);
  k.prop({ key: "sign_burial", def: "sign", cx: bx + 10, cy: by + 7, talk: "sign_burial" }, 2, 1);
  const g = sideGates(box);
  return { id: "burial", box, gates: [g.s, g.w, g.e] };
};

/** Lowfield Farm: an outpost. A house, a fenced plot, a fire. */
export const farm: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 64, 44);
  clear(k, box);
  const my = box.cy + Math.floor(box.h / 2);
  k.fill(box.cx, my - 1, box.w, 3, Tile.Dirt);
  k.house(box.cx + 6, box.cy + 3, 22, 12);
  const plot: Rect = { cx: box.cx + 34, cy: box.cy + 2, w: 26, h: 16 };
  k.fill(plot.cx, plot.cy, plot.w, plot.h, Tile.Garden);
  k.fenceRing(plot, "s", 4);
  k.fill(plot.cx + Math.floor(plot.w / 2) - 2, plot.cy + plot.h, 4, my - 1 - (plot.cy + plot.h), Tile.Dirt);
  k.prop({ key: "farm_fire", def: "campfire", cx: box.cx + 14, cy: my + 5, talk: "fire" }, 2, 2);
  k.pile({ cx: box.cx + 30, cy: my + 4, w: 24, h: 12 }, "barrel", 2);
  k.prop({ def: "lamp_post", cx: box.cx + 30, cy: my - 3 }, 1, 1);
  k.mark("farm_gate", box.cx + 3, my, 0);
  k.fill(box.cx + 15, box.cy + 15, 4, my - 1 - (box.cy + 15), Tile.Dirt);
  k.mark("farm_door", box.cx + 16, box.cy + 16, 3);
  const g = sideGates(box);
  // The farmhouse is 22 x 12 at (+6, +3): its wall strip is the bottom three rows, and the door sits in it.
  return { id: "farm", box, gates: [g.w, g.e], slots: { farm_door: [box.cx + 16, box.cy + 13] } };
};

/** The wood with the car in it. No road comes here. */
export const carWood: Build = (k, ox, oy) => {
  const box = boxAt(k, ox, oy, 15, 11);
  clear(k, box, Tile.Dirt);
  k.prop(
    {
      key: "car_wreck",
      def: "car_wreck",
      cx: box.cx + 5,
      cy: box.cy + 4,
      loot: [
        { item: "wood", qty: 2 },
        { item: "small_water", qty: 2 },
        { item: "fire_stone", qty: 1 },
      ],
    },
    5,
    3,
  );
  k.mark("car_wood", box.cx + 7, box.cy + 8, 3);
  const g = sideGates(box);
  return { id: "car_wood", box, gates: [g.n, g.s, g.w, g.e] };
};

/** A small outpost: a clearing, a fire, something to sit on. The Reedcutters' fire and the works canteen. */
function outpost(id: string, fireKey: string, ground: Tile, roofed: boolean): Build {
  return (k, ox, oy) => {
    const box = boxAt(k, ox, oy, 34, 26);
    clear(k, box, ground);
    const my = box.cy + Math.floor(box.h / 2);
    if (roofed) k.house(box.cx + 5, box.cy + 2, 22, 12);
    k.prop({ key: fireKey, def: "campfire", cx: box.cx + 16, cy: my + 4, talk: "fire" }, 2, 2);
    k.prop({ def: "crate", cx: box.cx + 6, cy: my + 5 }, 2, 2);
    k.prop({ def: "barrel", cx: box.cx + 24, cy: my + 6 }, 2, 2);
    k.mark(`${id}_gate`, box.cx + 3, my + 1, 0);
    const g = sideGates(box);
    return { id, box, gates: [g.s, g.w, g.e] };
  };
}

/**
 * Places the story reaches later and the game has not built yet. Each gets its ground,
 * its shape against the sky and a mark, and no door: a locked door with nothing behind
 * it would be a lie. They are here so the county is already the right shape, and so the
 * School is already there to be looked at.
 */
function landmark(id: string, w: number, h: number, ground: Tile, wall: Tile, mass: [number, number]): Build {
  return (k, ox, oy) => {
    const box = boxAt(k, ox, oy, w, h);
    clear(k, box, ground);
    const [bw, bh] = mass;
    const bx = box.cx + Math.floor((w - bw) / 2);
    k.fill(bx, box.cy + 1, bw, bh, wall);
    k.mark(`${id}_mouth`, box.cx + Math.floor(w / 2), box.cy + bh + 4, 1);
    const g = sideGates(box);
    return { id, box, gates: [g.s, g.w, g.e] };
  };
}

export const CHUNKS: Record<string, Build> = {
  station,
  julie_house: julieYard,
  town,
  farm,
  car_wood: carWood,
  gold_mine: mineMouth,
  graveyard,
  burial: burialStair,
  reed_camp: outpost("reed_camp", "reed_fire", Tile.Dirt, false),
  canteen: outpost("canteen", "canteen_fire", Tile.Cobble, true),
  museum: landmark("museum", 40, 30, Tile.Cobble, Tile.TempleWall, [28, 14]),
  library: landmark("library", 32, 26, Tile.Dirt, Tile.Wall, [20, 10]),
  butterfly_forest: landmark("butterfly_forest", 30, 24, Tile.Garden, Tile.Tree, [22, 8]),
  lake_statue: landmark("lake_statue", 20, 18, Tile.Cobble, Tile.TempleWall, [4, 4]),
  factory: landmark("factory", 56, 40, Tile.Cobble, Tile.Wall, [44, 22]),
  school: landmark("school", 60, 44, Tile.Cobble, Tile.Wall, [48, 26]),
};
