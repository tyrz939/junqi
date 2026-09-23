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
import type { Facing } from "@/sim/state";
import type { PropSpawn, Rect, UnitSpawn } from "@/world/blueprint";
import type { Kit } from "@/world/kit";

export type Gate = [number, number];
/** `slots`: exact cells the chunk offers to content placed by name: where a door goes in a wall it built. */
export type Chunk = { id: string; box: Rect; gates: Gate[]; slots?: Record<string, [number, number]> };
type Build = (k: Kit, ox: number, oy: number) => Chunk;

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

/**
 * Castle Halt: where the Sunday train stops. One train a week. New Game stands her on the platform.
 * A halt, not a parade ground: a platform along the line with a shelter, a name board, a bench and
 * two lamps; the ticket office nobody staffs, with the lost-property things in front of it; a
 * yard with a fire and a sign; a path out to the road.
 *
 *   rail | platform | fence | office (ticket window)
 *        | shelter  |       |   lost property
 *        | crate    |  ====== path to the road ======> gate
 *        | bench    |       |   fire     sign    lamps
 *        | board    |       |   churns, sacks
 */
export const station: Build = (k, _ox, oy) => {
  const box: Rect = { cx: 4, cy: Math.max(8, Math.min(k.h - 42, oy - 17)), w: 36, h: 34 };
  const my = box.cy + 17;
  clear(k, box);
  const L = new Local(k, 0, my); // x as the county has it (the line is at x 4), y from the middle of the platform
  // The platform's own length of line. The line goes on beyond it both ways, to the county's edge (county.ts layRailway).
  k.fill(4, box.cy, 3, box.h, Tile.Rail);
  L.f(7, -14, 9, 28, Tile.Cobble);
  L.f(16, -14, 1, 28, Tile.Fence);
  L.f(16, -2, 1, 5, Tile.Cobble);
  L.f(16, -1, box.cx + box.w - 16, 3, Tile.Dirt);
  // The path stays a path: nothing placed by name later (the lost-property things) may stand on it.
  k.claim(16, my - 1, box.cx + box.w - 16, 3);
  // The yard's edges: a fence along the top and the bottom, open to the road.
  L.f(17, -17, 22, 1, Tile.Fence);
  L.f(17, 16, 22, 1, Tile.Fence);

  // The platform, north to south.
  L.p("town_shelter", 9, -12);
  L.p("sign", 14, -13, { key: "timetable", talk: "timetable", label: "Timetable" });
  k.prop({ key: "station_lamp", def: "lamp_post", cx: 14, cy: my - 8 }, 1, 1);
  k.prop({ key: "station_crate", def: "crate", cx: 8, cy: my - 7 }, 2, 2);
  L.mark("start", 11, 0, 0);
  L.p("town_park_bench", 8, 4);
  L.p("lamp_post", 14, 6);
  L.p("town_station_sign", 9, 8, { key: "station_board", talk: "station_board" });
  L.p("town_trolley", 12, 11);
  L.u("traveller", "town_traveller", 9, 11, 0);
  L.rect("platform", 7, -14, 9, 28);

  // The office: nobody behind the glass. What was lost on the road is out in front of it.
  L.house(18, -16, 8, 8, { key: "ticket_window", talk: "ticket_window", label: "Ticket Window" }, 3, "town_ticket_window");
  L.p("town_park_bench", 18, -4);
  L.p("town_flower_bed", 23, -4);
  L.p("town_park_bench", 29, -15);
  L.p("town_flower_bed", 33, -15);

  // The yard, across the path: an open forecourt where the lost-property things stand, a fire, the sign.
  L.mark("lost_property", 22, 5, 3);
  k.prop({ key: "station_fire", def: "campfire", cx: 31, cy: my + 8, talk: "fire" }, 2, 2);
  k.prop({ key: "sign_town", def: "sign", cx: 32, cy: my + 3, talk: "sign_town" }, 2, 1);
  L.p("lamp_post", 36, -3);
  L.p("lamp_post", 36, 3);
  L.p("town_milk_churn", 35, 12);
  L.p("town_milk_churn", 36, 12);
  L.p("town_milk_churn", 35, 14);
  L.p("town_sacks", 31, 14);
  L.p("crate", 18, 14);

  // Wide on purpose: whatever changes on the platform changes while she is too far away to see it.
  k.rect("halt_approach", { cx: 0, cy: box.cy - 30, w: box.w + 34, h: box.h + 60 });
  return { id: "station", box, gates: [[box.cx + box.w, my]], slots: { station_lamp_dead: [14, my - 8], night_parcel: [9, my - 5] } };
};

/**
 * Auntie Julie's: the fence, the house, the stoop, the dog, the thing in the far corner.
 * One more cottage on the station road, the size of the others in the county (country.ts
 * builds them 22 x 16 round an 8 x 6 house; this one has a yard for the dog, the hens and
 * the thing in the corner, so it is 42 x 28, about a farmstead's ground: it was 64 x 48). A vegetable plot, two apple
 * trees, a hen house, the washing line, the water butt and the wood against the wall; one
 * lamp by the door; a lane across the yard gate to gate, and one down to the south.
 *
 *    0  plot  apple  [ house    ] wood             skeleton
 *    9  plot         lamp stoop flowers  book    fire
 *   15  gate ====== lane ================================ gate
 *   20  apple    |south lane|      washing       hens
 */
export const julieYard: Build = (k, ox, oy) => {
  const W = 42;
  const H = 28;
  const box = boxAt(k, ox, oy, W, H);
  clear(k, box);
  k.outline(box.cx, box.cy, W, H, Tile.Fence);
  const L = new Local(k, box.cx, box.cy);
  const hx = 14;
  const hy = 2;
  const hw = 10;
  const hh = 7;
  const doorX = hx + Math.floor(hw / 2) - 1;
  k.house(box.cx + hx, box.cy + hy, hw, hh);
  L.p("town_chimney", hx + 2, hy);
  L.p("door", doorX, hy + hh - 2, { key: "house_door", locked: true, keyTag: "auntie_house", to: { zone: "house", mark: "front" }, label: "Julie's door" });
  const gateY = 15;
  // One lane across the yard under the stoop, gate to gate; the door's own path drops onto it.
  L.f(0, gateY - 1, W, 3, Tile.Dirt);
  L.f(doorX - 2, hy + hh, 6, 2, Tile.Dirt);
  L.f(doorX, hy + hh + 2, 2, gateY - (hy + hh + 2), Tile.Dirt);
  // And one down to the south gate, so whichever way the road comes there is a way in.
  const southX = 8;
  L.f(southX - 1, gateY, 3, H - gateY, Tile.Dirt);
  L.rect("stoop", doorX - 6, hy + hh, 14, 7);
  L.mark("house_front", doorX, hy + hh + 1, 1);
  // Just inside the west gate: where the old small county began. Tests and the console still use it.
  L.mark("yard_gate", 3, gateY, 0);
  L.u("dog", "dog", doorX + 4, hy + hh + 2);
  L.p("lamp_post", doorX - 3, hy + hh);
  L.p("town_flower_bed", doorX + 4, hy + hh);
  // A fire in the yard: the first place to rest, before the house key and its bed.
  L.p("campfire", 31, 9, { key: "yard_fire", talk: "fire" });
  // Against the house: the water butt and the wood.
  L.p("barrel", hx + hw + 1, hy + 4);
  L.p("town_woodpile", hx + hw + 1, hy + 1);
  // Beside the water butt, as the quest says ("by the barrel in her yard"), and clear of the dog on the step.
  L.mark("garden_book", hx + hw + 3, hy + hh, 1);
  // West of the house: the vegetable plot, fenced, and the apple trees.
  L.fence(2, 2, 9, 8, [
    [6, 9],
    [7, 9],
  ]);
  L.f(3, 3, 7, 6, Tile.Garden);
  L.f(6, 3, 2, 7, Tile.Dirt);
  L.p("apple_tree", 11, 3, { loot: [{ item: "apple", qty: 3 }] });
  L.p("apple_tree", 3, 20, { loot: [{ item: "apple", qty: 2 }] });
  // Over the lane: the washing line, the hen house and its hens.
  L.p("town_washing_line", 22, 21);
  L.p("town_hen_coop", 33, 22, { key: "julies_hens", talk: "hen_coop" });
  // Julie's hens keep still. (A unit walking at the edge of the load ring can wake on one side of a
  // save and not the other: sim/ring.ts measures from her exact position, not her block. Reported.)
  L.u(null, "town_hen", 31, 25, 1);
  L.u(null, "town_hen_brown", 37, 25, 2);
  // The quest skeleton stands in the yard's far corner, in sight of the stoop, and out of reach of anyone
  // who only comes in at the west gate and stands about (test/budget.test.ts strolls there).
  L.u("yard_skeleton", "skeleton", W - 2, 1);
  return {
    id: "julie_house",
    box,
    gates: [
      [box.cx - 1, box.cy + gateY],
      [box.cx + W, box.cy + gateY],
      [box.cx + southX, box.cy + H],
    ],
  };
};

// --- drawing in a chunk's own coordinates -----------------------------------------------

/** Footprints of the props the chunks stand about, so a call cannot claim the wrong ground. */
const FOOT: Record<string, [number, number]> = {
  lamp_post: [1, 1],
  lamp_dead: [1, 1],
  barrel: [2, 2],
  crate: [2, 2],
  chest: [2, 2],
  apple_tree: [2, 2],
  campfire: [2, 2],
  sign: [2, 1],
  table: [3, 2],
  door: [2, 2],
  door_talk: [2, 2],
  notice_board: [3, 1],
  minecart: [2, 2],
  coffin: [2, 3],
  note: [1, 1],
  town_fountain: [4, 3],
  town_stall: [4, 2],
  town_stall_seeds: [4, 2],
  town_stall_bread: [4, 2],
  town_washing_line: [5, 1],
  town_park_bench: [3, 1],
  town_flower_bed: [3, 1],
  town_pillar_box: [1, 1],
  town_phone_box: [2, 2],
  town_memorial: [2, 2],
  town_sign_arms: [2, 1],
  town_sign_post: [2, 1],
  town_sign_forge: [2, 1],
  town_sign_stores: [2, 1],
  town_sign_doctor: [2, 1],
  town_headstone: [1, 1],
  town_trough: [3, 1],
  town_hay_bale: [2, 2],
  town_farm_cart: [3, 2],
  town_milk_churn: [1, 1],
  town_sacks: [2, 1],
  town_anvil: [2, 1],
  town_woodpile: [3, 1],
  town_hen_coop: [3, 2],
  town_station_sign: [4, 1],
  town_shelter: [5, 2],
  town_ticket_window: [2, 2],
  town_trolley: [2, 1],
  town_steeple: [2, 2],
  town_barn_doors: [4, 3],
  town_chimney: [1, 1],
};

type Way = [number, number] | [number, number, number];
type Extra = Omit<PropSpawn, "def" | "cx" | "cy" | "key"> & { key?: string };

/** A chunk draws in its own coordinates: (0, 0) is the top-left of its box. */
class Local {
  constructor(
    readonly k: Kit,
    readonly X: number,
    readonly Y: number,
  ) {}
  f(x: number, y: number, w: number, h: number, t: Tile): void {
    this.k.fill(this.X + x, this.Y + y, w, h, t);
  }
  p(def: string, x: number, y: number, extra: Extra = {}): PropSpawn {
    const [w, h] = FOOT[def] ?? [1, 1];
    return this.k.prop({ ...extra, def, cx: this.X + x, cy: this.Y + y }, w, h);
  }
  /** A person or an animal. A route is waypoints (and ticks to stand at each), in chunk cells. */
  u(key: string | null, def: string, x: number, y: number, facing: Facing = 1, route?: Way[]): UnitSpawn {
    const patrol = route?.map((q): Way => (q.length === 3 ? [this.X + q[0], this.Y + q[1], q[2]] : [this.X + q[0], this.Y + q[1]]));
    const s = this.k.unit(key, def, this.X + x, this.Y + y, patrol);
    s.facing = facing;
    return s;
  }
  mark(name: string, x: number, y: number, facing?: Facing): void {
    this.k.mark(name, this.X + x, this.Y + y, facing);
  }
  rect(name: string, x: number, y: number, w: number, h: number): Rect {
    return this.k.rect(name, { cx: this.X + x, cy: this.Y + y, w, h });
  }
  /** Absolute cell of a chunk cell. */
  at(x: number, y: number): [number, number] {
    return [this.X + x, this.Y + y];
  }
  /**
   * A house: roof over a wall strip, and a door in the middle of the wall (or `door` cells in).
   * The door is whatever `spec` says; `null` leaves the hole for someone else's row (a slot).
   * Returns the door's cell and the step in front of it.
   */
  house(x: number, y: number, w: number, h: number, spec: Extra | null, door = Math.floor(w / 2) - 1, def = "door_talk"): { door: [number, number]; step: [number, number] } {
    this.k.house(this.X + x, this.Y + y, w, h);
    // A chimney on every home-sized roof, at one end or the other, so a row of houses is not one house.
    if (w <= 12) this.k.prop({ def: "town_chimney", cx: this.X + x + ((x >> 1) % 2 === 0 ? 2 : w - 3), cy: this.Y + y }, 1, 1);
    const d: [number, number] = [this.X + x + door, this.Y + y + h - 2];
    if (spec) this.k.prop({ ...spec, def, cx: d[0], cy: d[1] }, 2, 2);
    return { door: d, step: [d[0], d[1] + 2] };
  }
  /**
   * A fence round a rect, with gaps at the listed chunk cells (the gap takes `ground`). What is
   * fenced is somebody's, so it is claimed (with a row either side): nothing placed later by name,
   * a grate or a lost glove, lands in a garden or against its rails. `open` leaves the inside free.
   */
  fence(x: number, y: number, w: number, h: number, gaps: [number, number][] = [], ground: Tile = Tile.Dirt, open = false): void {
    this.k.outline(this.X + x, this.Y + y, w, h, Tile.Fence);
    for (const [gx, gy] of gaps) this.k.set(this.X + gx, this.Y + gy, ground);
    if (!open) this.k.claim(this.X + x - 1, this.Y + y - 2, w + 2, h + 3);
  }
}

/**
 * Castle. Zelda scale: a town you can cross in thirty seconds that is full the whole way.
 * One high street through a cobbled square; a back lane behind each row of houses; alleys
 * between them. Twenty-six buildings, three of them with a way in or a reason to knock; a
 * fountain, a memorial, stalls, gardens, washing, and the people who live here, out of doors
 * until the lamps. The door facing the fire in the square is Mrs Allen's sister's.
 *
 *   0 ........churchyard.|lane|..........................orchard
 *  10 back lane ---------|    |------------- back lane ----------
 *  14 houses  | church   |    | Allen  PO    the Arms    house
 *  23 gardens |       [  the square: fountain, fire, board ]  pub yard
 *  33 ====================== high street ============================
 *  37 houses   forge      [  stalls        phone box    ]  stores
 *  47 ------------------------- cross lane -----------------------
 *  50 houses          green   doctor's   |lane|  houses
 *  60 ------------------------- pound lane -----------------------
 *  62 plots, hens, woodpiles              |lane|  churns
 */
export const town: Build = (k, ox, oy) => {
  const W = 100;
  const H = 70;
  const box = boxAt(k, ox, oy, W, H);
  clear(k, box);
  const L = new Local(k, box.cx, box.cy);
  const MAIN = 34;

  // --- streets ---------------------------------------------------------------------------
  L.f(0, MAIN - 1, W, 3, Tile.Road);
  L.f(34, 23, 36, 24, Tile.Cobble); // the square
  L.f(47, 0, 3, 23, Tile.Dirt); // Church Lane, north to the gate
  L.f(47, 47, 3, 23, Tile.Dirt); // and south
  L.f(1, 10, 32, 2, Tile.Dirt); // Back Lane, west
  L.f(50, 10, 49, 2, Tile.Dirt); // and east
  L.f(1, 47, 98, 2, Tile.Dirt); // Cross Lane
  L.f(1, 60, 98, 2, Tile.Dirt); // Pound Lane
  // Alleys between the houses, lane to street.
  for (const [x, y0, y1] of [
    [11, 10, 33],
    [32, 10, 33],
    [60, 10, 23],
    [86, 10, 33],
    [11, 36, 60],
    [23, 36, 60],
    [60, 47, 60],
    [70, 36, 47],
    [82, 36, 60],
  ] as const) {
    L.f(x, y0, 2, y1 - y0, Tile.Dirt);
  }

  // --- north: Back Lane, the church, the square's north side ------------------------------
  const backLane: [number, number, string][] = [
    [2, 9, "No. 1, Back Lane"],
    [13, 8, "No. 2, Back Lane"],
    [23, 9, "No. 3, Back Lane"],
    [52, 9, "No. 4, Back Lane"],
    [63, 9, "No. 5, Back Lane"],
    [75, 9, "No. 6, Back Lane"],
  ];
  const backTalk = ["door_back_1", "door_back_2", "door_back_3", "door_back_4", "door_back_5", "door_back_6"];
  backLane.forEach(([x, w, label], n) => L.house(x, 2, w, 8, { key: `door_back_${n + 1}`, talk: backTalk[n], label }));
  L.mark("back_lane", 30, 10, 0);
  L.p("sign", 1, 12, { key: "sign_back_lane", talk: "sign_back_lane" });
  // The top of Church Lane, by the north gate: the one lane in town whose name is painted out.
  L.p("sign", 50, 3, { key: "sign_church_lane", talk: "sign_church_lane" });
  L.mark("church_lane_top", 48, 4, 0);
  // An orchard where No. 7 would have been.
  for (const [x, y] of [
    [87, 1],
    [91, 2],
    [95, 1],
    [89, 6],
    [93, 6],
  ] as const) {
    L.p("apple_tree", x, y, { loot: [{ item: "apple", qty: 2 }] });
  }
  L.mark("orchard", 91, 8, 1);

  // The church, its yard behind it, and the cat that is not allowed in there either.
  L.house(34, 8, 12, 14, { key: "church_door", label: "St Anne's", to: { zone: "church", mark: "entry" } }, 5, "door");
  L.mark("church_door", 39, 23, 1);
  L.p("town_steeple", 39, 10);
  L.p("town_flower_bed", 35, 22);
  L.p("town_flower_bed", 42, 22);
  L.fence(34, 0, 13, 7, [
    [46, 2],
    [46, 3],
    [46, 4],
  ]);
  L.f(35, 1, 11, 5, Tile.Grass);
  [36, 38, 40, 42, 44].forEach((x, n) => L.p("town_headstone", x, 1, { key: `headstone_${n + 1}`, talk: n === 3 ? "headstone_new" : n === 4 ? "headstone_sallis" : "headstone" }));
  [36, 38, 40].forEach((x) => L.p("town_headstone", x, 4, { talk: "headstone" }));
  L.mark("churchyard", 42, 4, 1);
  L.u("sixpence", "town_cat_sixpence", 44, 4, 2);

  // Mrs Allen's sister. Her door faces the fire; the door itself is a row in data/placements.
  const allen = L.house(51, 14, 9, 8, null, 3);
  L.mark("allen_door", allen.door[0] - L.X, 23, 3);
  L.p("town_flower_bed", 51, 22);
  L.p("town_flower_bed", 57, 22);
  // The post office, with its box outside, and Miss Dray, who is waiting for the second post.
  L.house(62, 14, 9, 8, { key: "post_office_door", talk: "post_office_door", label: "The Post Office" }, 4);
  L.p("town_sign_post", 68, 22, { key: "sign_post_office", talk: "sign_post_office" });
  L.p("town_pillar_box", 64, 22, { key: "pillar_box", talk: "pillar_box" });
  L.mark("post_office", 66, 23, 3);
  L.u("miss_dray", "town_postmistress", 63, 24, 1);

  // The Castle Arms: the one door on the street that opens, and a yard of tables in front of it.
  L.house(72, 12, 14, 10, { key: "arms_door", label: "The Castle Arms", to: { zone: "arms", mark: "entry" } }, 6, "door");
  L.p("town_sign_arms", 81, 22, { key: "sign_arms", talk: "sign_arms" });
  L.p("town_chimney", 74, 12);
  L.p("town_chimney", 83, 12);
  L.f(72, 22, 14, 10, Tile.Dirt);
  L.fence(
    71,
    22,
    15,
    10,
    [
      [78, 31],
      [79, 31],
    ],
    Tile.Dirt,
    true,
  );
  // Open for the washing to blow into, but not along the rails at the bottom.
  k.claim(L.X + 71, L.Y + 28, 15, 4);
  L.f(72, 22, 14, 1, Tile.Dirt);
  // Barrels against the wall, two tables out in the yard with a bench each, and room between.
  L.p("barrel", 72, 22, { key: "arms_barrel", loot: [{ item: "small_water", qty: 2 }] });
  L.p("barrel", 84, 22);
  L.p("table", 73, 27);
  L.p("table", 82, 27);
  L.p("town_park_bench", 73, 29);
  L.p("town_park_bench", 82, 29);
  L.mark("arms_front", 78, 23, 1);
  L.mark("arms_yard", 78, 26, 1);
  L.u("mr_cobb", "town_drinker", 80, 29, 2);

  // No. 7, High Street, and its garden: the orchard's owner.
  L.house(88, 14, 10, 8, { key: "door_high_7", talk: "door_high_7", label: "No. 7, High Street" }, 4);
  L.fence(88, 23, 10, 9, [
    [92, 23],
    [93, 23],
    [92, 31],
    [93, 31],
  ]);
  L.f(92, 22, 2, 11, Tile.Dirt);
  L.p("town_flower_bed", 89, 24);
  L.p("town_flower_bed", 94, 24);
  L.p("apple_tree", 89, 27, { loot: [{ item: "apple", qty: 2 }] });
  L.p("town_park_bench", 94, 29);
  L.u("mrs_hobb", "town_waterer", 90, 26, 3);

  // High Street, north side, west of the church: three houses behind their front gardens.
  const high: [number, number, number, string, string][] = [
    [2, 9, 3, "No. 1, High Street", "door_high_1"],
    [13, 9, 3, "No. 3, High Street", "door_high_3"],
    [23, 9, 1, "No. 5, High Street", "door_high_5"],
  ];
  for (const [x, w, door, label, talk] of high) {
    const h = L.house(x, 14, w, 8, { key: talk, talk, label }, door);
    const dx = h.door[0] - L.X;
    L.fence(x, 23, w, 9, [
      [dx, 23],
      [dx + 1, 23],
      [dx, 31],
      [dx + 1, 31],
    ]);
    L.f(dx, 22, 2, 11, Tile.Dirt);
  }
  // No. 1: the gardener's. Beds and a tree.
  L.p("town_flower_bed", 7, 24);
  L.p("town_flower_bed", 7, 29);
  L.p("apple_tree", 3, 27, { loot: [{ item: "apple", qty: 3 }] });
  L.u("mr_sallis", "town_gardener", 8, 26, 2, [
    [8, 26, 400],
    [8, 27, 300],
  ]);
  // No. 3: a bench and a water butt.
  L.p("town_park_bench", 18, 29);
  L.p("barrel", 14, 24);
  L.p("town_flower_bed", 18, 24);
  // No. 5: Mrs Marsh's washing, what is left of it.
  L.p("town_washing_line", 26, 25, { key: "marsh_line" });
  L.p("town_flower_bed", 27, 29);
  L.mark("washing_line", 28, 27, 1);
  L.u("mrs_marsh", "town_washerwoman", 30, 27, 2);

  // --- the square ------------------------------------------------------------------------
  L.p("town_fountain", 41, 26, { key: "fountain", talk: "fountain" });
  L.mark("fountain", 43, 30, 3);
  L.u("tilly", "town_tilly", 40, 30, 1);
  L.u("robin", "town_robin", 37, 31, 1, [
    [37, 25, 60],
    [37, 31, 0],
    [45, 31, 90],
    [46, 25, 0],
  ]);
  L.u("nell", "town_nell", 46, 25, 1, [
    [46, 25, 60],
    [45, 31, 0],
    [37, 31, 90],
    [37, 25, 0],
  ]);
  // The fire, straight out from Mrs Allen's sister's door, so the note is right about it.
  const fireX = allen.door[0] - L.X;
  L.p("campfire", fireX, 26, { key: "town_fire", talk: "fire" });
  L.mark("town_square", 52, 30, 1);
  L.p("town_memorial", 63, 26, { key: "memorial", talk: "memorial" });
  L.mark("memorial", 63, 24, 1);
  L.p("town_park_bench", 60, 30);
  L.u("mr_tolly", "town_old_man", 64, 30, 2);
  L.u("mrs_fenn", "town_gossip_a", 66, 36, 0);
  L.u("mrs_wick", "town_gossip_b", 68, 36, 2);
  // Lamps: one at each corner of the square, and a pair at each end of the street.
  for (const [x, y] of [
    [34, 23],
    [69, 23],
    [34, 46],
    [69, 46],
    [1, 32],
    [1, 36],
    [98, 32],
    [98, 36],
    [46, 7],
    [50, 7],
    [46, 68],
    [50, 68],
  ] as const) {
    L.p("lamp_post", x, y);
  }
  // Market: three stalls, and the people who keep them standing at their ends.
  L.p("town_stall", 36, 39, { key: "stall_greens", talk: "stall_greens" });
  L.u("mr_pound", "town_grocer", 40, 41, 1);
  L.p("town_stall_bread", 41, 39 + 0, { key: "stall_bread", talk: "stall_bread" });
  L.u("mrs_oddie", "town_baker", 45, 41, 1);
  L.p("town_stall_seeds", 52, 39, { key: "stall_seeds", talk: "stall_seeds" });
  L.u("mrs_crewe", "town_seedwoman", 56, 41, 1);
  L.u("mrs_bex", "town_shopper", 38, 43, 1, [
    [38, 43, 240],
    [44, 43, 180],
    [54, 43, 240],
    [44, 44, 0],
  ]);
  L.p("town_phone_box", 64, 39, { key: "phone_box", talk: "phone_box" });
  L.u("mr_dunn", "town_caller", 63, 42, 0);
  L.mark("phone_box", 67, 40, 2);
  L.p("town_flower_bed", 35, 45);
  L.p("town_flower_bed", 59, 45);
  L.u("sweeper", "town_sweeper", 36, 37, 1, [
    [36, 37, 300],
    [67, 37, 0],
    [67, 45, 300],
    [36, 45, 0],
  ]);
  // Each end of the High Street, between its pair of lamps: the Constable's walk.
  L.rect("street_west", 0, MAIN - 4, 5, 9);
  L.mark("street_west", 3, MAIN, 0);
  L.rect("street_east", W - 5, MAIN - 4, 5, 9);
  L.mark("street_east", W - 4, MAIN, 2);
  L.u("constable", "town_constable", 20, MAIN + 1, 0, [
    [4, MAIN + 1, 600],
    [95, MAIN + 1, 600],
  ]);

  // --- south of the street: Cross Lane ---------------------------------------------------
  L.house(2, 37, 9, 8, { key: "door_cross_2", talk: "door_cross_2", label: "No. 2, Cross Lane" });
  L.house(13, 37, 10, 8, { key: "forge_door", talk: "forge_door", label: "The Forge" }, 4);
  L.p("town_sign_forge", 20, 45, { key: "sign_forge", talk: "sign_forge" });
  L.p("town_anvil", 14, 45);
  L.u("mr_hale", "town_smith", 16, 46, 1);
  L.mark("forge", 19, 47, 3);
  L.mark("cross_lane", 40, 48, 0);
  L.mark("pound_lane", 30, 61, 0);
  L.house(25, 37, 8, 8, { key: "door_cross_6", talk: "door_cross_6", label: "No. 6, Cross Lane" });
  L.u("mrs_tace", "town_widow", 30, 46, 2);
  L.house(72, 37, 10, 8, { key: "stores_door", talk: "stores_door", label: "Castle Stores" }, 4);
  L.p("town_sign_stores", 79, 45, { key: "sign_stores", talk: "sign_stores" });
  L.p("crate", 73, 45);
  L.house(84, 37, 8, 8, { key: "door_cross_9", talk: "door_cross_9", label: "No. 9, Cross Lane" });
  L.p("town_woodpile", 94, 44);
  L.p("town_flower_bed", 94, 38);

  // --- Pound Lane ------------------------------------------------------------------------
  L.house(2, 50, 9, 8, { key: "door_pound_1", talk: "door_pound_1", label: "No. 1, Pound Lane" });
  L.house(13, 50, 9, 8, { key: "door_pound_3", talk: "door_pound_3", label: "No. 3, Pound Lane" });
  // The green: benches under a tree, and the pair who have been on them since lunch.
  L.p("apple_tree", 28, 51, { loot: [{ item: "apple", qty: 2 }] });
  L.p("town_park_bench", 25, 55);
  L.p("town_park_bench", 30, 55);
  L.p("town_flower_bed", 26, 58);
  L.u("mr_lyle", "town_courting_a", 25, 57, 3);
  L.u("miss_orme", "town_courting_b", 27, 57, 3);
  L.u("mr_ince", "town_reader", 33, 55, 2);
  L.u("dot", "town_dot", 4, 48, 0, [
    [4, 48, 120],
    [30, 48, 200],
  ]);
  L.house(36, 50, 10, 8, { key: "doctors_door", talk: "doctors_door", label: "The Doctor's" }, 4);
  L.p("town_sign_doctor", 43, 58, { key: "sign_doctor", talk: "sign_doctor" });
  L.mark("doctor", 41, 59, 3);
  L.u("dr_vane", "town_doctor", 38, 59, 1);
  L.house(51, 50, 9, 8, { key: "door_pound_7", talk: "door_pound_7", label: "No. 7, Pound Lane" });
  L.house(63, 50, 9, 8, { key: "door_pound_9", talk: "door_pound_9", label: "No. 9, Pound Lane" });
  L.house(74, 50, 8, 8, { key: "door_pound_11", talk: "door_pound_11", label: "No. 11, Pound Lane" });
  L.house(86, 50, 10, 8, { key: "door_pound_13", talk: "door_pound_13", label: "No. 13, Pound Lane" });
  L.p("sign", 1, 58, { key: "sign_pound_lane", talk: "sign_pound_lane" });
  L.p("sign", 1, 45, { key: "sign_cross_lane", talk: "sign_cross_lane" });

  // --- behind Pound Lane: plots, hens, the milk ------------------------------------------
  L.fence(2, 63, 19, 6, [
    [10, 63],
    [11, 63],
  ]);
  L.f(3, 64, 17, 4, Tile.Garden);
  L.f(10, 64, 2, 4, Tile.Dirt);
  L.p("town_hen_coop", 24, 63, { key: "pound_hens", talk: "pound_hen_coop" });
  L.mark("pound_hens", 25, 61, 1);
  L.u(null, "town_hen", 28, 65, 1, [
    [28, 65, 120],
    [33, 66, 200],
    [30, 64, 100],
  ]);
  L.u(null, "town_hen_brown", 32, 64, 2, [
    [32, 64, 160],
    [27, 66, 120],
  ]);
  L.u(null, "town_hen", 30, 67, 0, [
    [30, 67, 200],
    [35, 65, 140],
  ]);
  L.p("town_woodpile", 38, 64);
  L.p("town_woodpile", 38, 67);
  L.p("town_milk_churn", 52, 63);
  L.p("town_milk_churn", 53, 63);
  L.p("town_milk_churn", 52, 65);
  L.u("milkman", "town_milkman", 55, 64, 2, [
    [55, 64, 600],
    [55, 49, 0],
    [66, 49, 300],
    [55, 49, 0],
  ]);
  L.p("town_sacks", 60, 64);
  L.p("barrel", 64, 64);
  L.p("town_hay_bale", 70, 63);
  L.fence(78, 63, 20, 6, [
    [87, 63],
    [88, 63],
  ]);
  L.f(79, 64, 18, 4, Tile.Garden);
  L.f(87, 64, 2, 4, Tile.Dirt);
  L.u("town_cat", "town_cat_ginger", 34, 46, 1, [
    [34, 48, 900],
    [58, 48, 600],
  ]);

  return {
    id: "town",
    box,
    gates: [
      [box.cx - 1, box.cy + MAIN],
      [box.cx + W, box.cy + MAIN],
      [box.cx + 48, box.cy - 1],
      [box.cx + 48, box.cy + H],
    ],
    // Where Mrs Marsh's washing came down: by the church door, by the fountain, inside the Arms yard.
    slots: { allen_door: allen.door, washing_church: L.at(45, 24), washing_fountain: L.at(46, 28), washing_arms: L.at(80, 25) },
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
  // The adit: a second way out, round the east side of the hill, let into the cliff's flank. It is
  // barred from the inside. Once somebody has stood in the gallery it leads from (which is behind
  // Repair), a row in data/triggers unbars it, and it is a way back in for good. A sign that says
  // there is no exit some nights needs a second exit to be fair (PLAN.md 5).
  k.prop({ key: "adit_door", def: "door", cx: mx + 16, cy: my + 3, locked: true, to: { zone: "mine", mark: "adit" }, label: "The adit" }, 2, 2);
  k.mark("mine_adit", mx + 19, my + 4, 0);
  k.rect("mine_yard", box);
  // The Company's pay hatch: a stub of wall with a gap in it, and nobody behind the gap.
  k.fill(box.cx + 1, my + 12, 1, 2, Tile.Wall);
  k.fill(box.cx + 3, my + 12, 1, 2, Tile.Wall);
  k.mark("company_notice", box.cx + 2, my + 15, 3);
  // The Company kept a yard: lamps either side of the mouth, a second cart, pit props, stores.
  k.prop({ def: "lamp_post", cx: mx + 5, cy: my + 8 }, 1, 1);
  k.prop({ def: "lamp_post", cx: mx + 11, cy: my + 8 }, 1, 1);
  k.prop({ def: "minecart", cx: mx + 4, cy: my + 15 }, 2, 2);
  k.prop({ def: "town_woodpile", cx: mx + 10, cy: my + 18 }, 3, 1);
  k.prop({ def: "crate", cx: mx + 20, cy: my + 10 }, 2, 2);
  k.prop({ def: "barrel", cx: mx + 20, cy: my + 13 }, 2, 2);
  k.prop({ def: "town_sacks", cx: mx + 19, cy: my + 18 }, 2, 1);
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
  // Stones in rows, the way a parish lays them out, and lamps at both gates.
  for (let x = box.cx + 5; x < box.cx + box.w - 5; x += 3) {
    k.prop({ def: "town_headstone", cx: x, cy: box.cy + 2, talk: "headstone" }, 1, 1);
    k.prop({ def: "town_headstone", cx: x, cy: box.cy + box.h - 4, talk: "headstone" }, 1, 1);
  }
  for (const [x, y] of [
    [box.cx + 2, my - 3],
    [box.cx + 2, my + 3],
    [box.cx + box.w - 3, my - 3],
    [box.cx + box.w - 3, my + 3],
  ] as const) {
    k.prop({ def: "lamp_post", cx: x, cy: y }, 1, 1);
  }
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

/**
 * Lowfield Farm: a farmstead. The farmhouse (whose door talks and does not open), the barn (which
 * is chained), the hens, a pen of sheep, the drilled field across the lane, the hay and the cart.
 * The farmer does not come out; everything else is out in the open where it can be looked at.
 */
export const farm: Build = (k, ox, oy) => {
  const W = 60;
  const H = 44;
  const box = boxAt(k, ox, oy, W, H);
  clear(k, box);
  const L = new Local(k, box.cx, box.cy);
  const my = Math.floor(H / 2);
  L.f(0, my - 1, W, 3, Tile.Dirt);
  // The farmhouse: 14 x 10, its door in the wall strip. The door itself is a row in data/placements.
  const house = L.house(5, 3, 14, 10, null, 5);
  L.f(10, 13, 2, my - 1 - 13, Tile.Dirt);
  L.mark("farm_door", 10, 14, 3);
  L.p("town_flower_bed", 6, 13);
  L.p("town_flower_bed", 13, 13);
  L.p("sign", 2, my + 2, { key: "sign_farm", talk: "sign_farm" });
  L.mark("farm_gate", 3, my, 0);
  L.p("lamp_post", 1, my - 3);
  L.p("town_milk_churn", 5, my - 4);
  L.p("town_milk_churn", 6, my - 4);
  L.p("campfire", 20, 15, { key: "farm_fire", talk: "fire" });
  // The barn, and its doors, which are the one thing here that asks you not to.
  L.house(23, 2, 16, 12, null);
  L.p("town_barn_doors", 29, 11, { key: "barn_door", talk: "barn_door", label: "The barn" });
  L.f(30, 14, 2, my - 1 - 14, Tile.Dirt);
  L.p("town_hay_bale", 24, 15);
  L.p("town_hay_bale", 35, 15);
  L.p("lamp_post", 33, my - 3);
  // Hens by the house, sheep in the pen.
  L.p("town_hen_coop", 41, 2, { key: "farm_hens", talk: "hen_coop" });
  L.u(null, "town_hen", 45, 4, 1, [
    [45, 4, 140],
    [50, 5, 200],
    [47, 6, 90],
  ]);
  L.u(null, "town_hen_brown", 51, 3, 2, [
    [51, 3, 180],
    [46, 5, 120],
  ]);
  L.fence(41, 8, 17, 11);
  L.p("town_trough", 43, 16);
  (
    [
      [45, 11, 50, 13],
      [52, 10, 48, 15],
      [54, 14, 51, 11],
      [47, 14, 44, 10],
    ] as const
  ).forEach(([x, y, x2, y2]) =>
    L.u(null, "town_sheep", x, y, 0, [
      [x, y, 400],
      [x2, y2, 300],
    ]),
  );
  // Across the lane: the field in drills, the hay, the cart.
  L.fence(3, my + 3, 26, 15, [
    [15, my + 3],
    [16, my + 3],
  ]);
  for (let y = my + 4; y < my + 17; y++) L.f(4, y, 24, 1, (y - my) % 2 === 0 ? Tile.Garden : Tile.Grass);
  L.f(15, my + 2, 2, 15, Tile.Dirt);
  L.p("town_hay_bale", 33, my + 4);
  L.p("town_hay_bale", 36, my + 4);
  L.p("town_hay_bale", 33, my + 7);
  L.p("town_farm_cart", 40, my + 4);
  L.p("town_sacks", 45, my + 5);
  L.p("barrel", 49, my + 4);
  L.p("town_woodpile", 33, my + 12);
  L.p("apple_tree", 44, my + 11, { loot: [{ item: "apple", qty: 2 }] });
  L.p("apple_tree", 50, my + 13, { loot: [{ item: "apple", qty: 2 }] });
  const g = sideGates(box);
  return { id: "farm", box, gates: [g.w, g.e], slots: { farm_door: house.door } };
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
    // Somewhere to sit by the fire, and the things a camp or a canteen keeps about it.
    k.prop({ def: "town_park_bench", cx: box.cx + 12, cy: my + 8 }, 3, 1);
    k.prop({ def: "town_park_bench", cx: box.cx + 19, cy: my + 8 }, 3, 1);
    k.prop({ def: "lamp_post", cx: box.cx + 2, cy: my - 2 }, 1, 1);
    if (roofed) {
      k.prop({ def: "table", cx: box.cx + 26, cy: my + 1 }, 3, 2);
      k.prop({ def: "town_milk_churn", cx: box.cx + 29, cy: box.cy + 3 }, 1, 1);
      k.prop({ def: "town_sacks", cx: box.cx + 29, cy: box.cy + 6 }, 2, 1);
    } else {
      k.prop({ def: "town_woodpile", cx: box.cx + 6, cy: box.cy + 4 }, 3, 1);
      k.prop({ def: "town_washing_line", cx: box.cx + 14, cy: box.cy + 3 }, 5, 1);
      k.prop({ def: "town_sacks", cx: box.cx + 24, cy: box.cy + 5 }, 2, 1);
      k.prop({ def: "town_hay_bale", cx: box.cx + 27, cy: box.cy + 8 }, 2, 2);
    }
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
    const doorX = box.cx + Math.floor(w / 2) - 1;
    k.mark(`${id}_mouth`, box.cx + Math.floor(w / 2), box.cy + bh + 4, 1);
    // A way up to the face, so whatever door is set into it can be walked to.
    k.fill(doorX, box.cy + bh + 1, 2, 4, Tile.Dirt);
    // Lamps either side of the way up, and a bench to look at it from, where there is room.
    for (const [x, y, def, fw] of [
      [doorX - 3, box.cy + bh + 2, "lamp_post", 1],
      [doorX + 4, box.cy + bh + 2, "lamp_post", 1],
      [box.cx + 2, box.cy + h - 3, "town_park_bench", 3],
    ] as const) {
      if (k.fits(x, y, fw, 1)) k.prop({ def, cx: x, cy: y }, fw, 1);
    }
    const g = sideGates(box);
    // The slot in its face. A door is set here once the place behind it exists (data/doors.json);
    // until then the face is blank, because a locked door with nothing behind it would be a lie.
    return { id, box, gates: [g.s, g.w, g.e], slots: { [`${id}_door`]: [doorX, box.cy + bh - 1] } };
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
