// The things a town is full of: a fountain, stalls, washing, benches, a post box, a telephone
// box nobody can ring, the Halt's shelter and its name board, a farm's bales and cart. Same hand
// as art/props.ts: width is footprint w * 8, tall things rise UP out of their footprint, light
// from the top-left, a 1 px dark outline, a translucent contact shadow.
//
// These are drawn with a few shape calls and then outlined by rule, not typed a pixel at a time:
// a stall is a counter, two posts and a striped awning, and the outline is wherever a shape
// meets the air. It keeps twenty props in one hand and makes a wrong pixel a wrong number.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#3a3e4a", // cold iron
  f: "#5aa84e", // leaf, lit
  F: "#2a6838", // leaf, shade
  j: "#e0685c", // bright red cloth
  u: "#d8d0e8", // pale lilac
  v: "#5a4636", // dark thatch
  z: "#8a3040", // deep red paint
  "-": "#00000030",
  "=": "#00000050",
};

class Canvas {
  readonly px: string[][];
  constructor(
    readonly w: number,
    readonly h: number,
  ) {
    this.px = Array.from({ length: h }, () => Array.from({ length: w }, () => "."));
  }
  set(x: number, y: number, c: string): this {
    if (x >= 0 && y >= 0 && x < this.w && y < this.h) this.px[y][x] = c;
    return this;
  }
  rect(x: number, y: number, w: number, h: number, c: string): this {
    for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) this.set(i, j, c);
    return this;
  }
  hline(x: number, y: number, w: number, c: string): this {
    return this.rect(x, y, w, 1, c);
  }
  vline(x: number, y: number, h: number, c: string): this {
    return this.rect(x, y, 1, h, c);
  }
  /** Filled ellipse, centre (cx, cy) on half-pixels allowed. Art is built once at boot; this is not the sim. */
  ell(cx: number, cy: number, rx: number, ry: number, c: string): this {
    for (let y = 0; y < this.h; y++) {
      for (let x = 0; x < this.w; x++) {
        const dx = (x + 0.5 - cx) / rx;
        const dy = (y + 0.5 - cy) / ry;
        if (dx * dx + dy * dy <= 1) this.set(x, y, c);
      }
    }
    return this;
  }
  /** Draw `rows` (palette strings, "." transparent) with their top-left at (x, y). */
  stamp(x: number, y: number, rows: string[]): this {
    rows.forEach((r, j) => Array.from(r).forEach((c, i) => c !== "." && this.set(x + i, y + j, c)));
    return this;
  }
  /** A 1 px outline wherever a drawn pixel meets the air. Shadows do not count as drawn. */
  outline(c = "k"): this {
    const solid = (x: number, y: number): boolean => x >= 0 && y >= 0 && x < this.w && y < this.h && !".-=".includes(this.px[y][x]) && this.px[y][x] !== c;
    const add: [number, number][] = [];
    for (let y = 0; y < this.h; y++) {
      for (let x = 0; x < this.w; x++) {
        if (!".-=".includes(this.px[y][x])) continue;
        if (solid(x - 1, y) || solid(x + 1, y) || solid(x, y - 1) || solid(x, y + 1)) add.push([x, y]);
      }
    }
    for (const [x, y] of add) this.px[y][x] = c;
    return this;
  }
  /** A soft shadow ellipse on the ground, only where nothing is drawn yet. */
  shadow(cx: number, cy: number, rx: number, ry: number): this {
    for (let y = 0; y < this.h; y++) {
      for (let x = 0; x < this.w; x++) {
        const dx = (x + 0.5 - cx) / rx;
        const dy = (y + 0.5 - cy) / ry;
        if (dx * dx + dy * dy <= 1 && this.px[y][x] === ".") this.px[y][x] = "-";
      }
    }
    return this;
  }
  rows(): string[] {
    return this.px.map((r) => r.join(""));
  }
}

/** footH: footprint height in cells. The sprite's bottom `footH * 8` rows are the footprint. */
function prop(c: Canvas, footH: number, extra: Record<string, string[]> = {}): SpriteSrc {
  return { w: c.w, h: c.h, ax: 0, ay: c.h - footH * 8, palette: PX, frames: { base: c.rows(), ...extra } };
}

// --- the square -----------------------------------------------------------------------------

function fountain(): SpriteSrc {
  const c = new Canvas(32, 32);
  c.shadow(16, 28, 15, 4);
  // The basin: a low stone ring, and the water in it.
  c.ell(16, 24, 15, 7, "g").ell(16, 23, 15, 6, "W").ell(16, 23.5, 12.5, 4.5, "B").ell(16, 23, 12, 4, "b");
  c.set(9, 22, "i").set(10, 22, "i").set(20, 24, "i").set(21, 24, "i").set(13, 25, "i");
  c.hline(4, 27, 24, "G").hline(6, 29, 20, "G");
  // The column and the upper bowl.
  c.rect(15, 9, 3, 13, "g").vline(17, 9, 13, "G").vline(15, 9, 13, "W");
  c.ell(16.5, 10, 6, 2.5, "g").ell(16.5, 9.5, 6, 2, "W").ell(16.5, 9.5, 4, 1, "b");
  // Water falling from the spout into the bowl.
  c.rect(16, 3, 1, 6, "i").set(15, 4, "b").set(17, 5, "b").set(14, 7, "i").set(18, 7, "i");
  c.set(9, 12, "i").set(24, 12, "i").vline(10, 12, 9, "b").vline(23, 12, 9, "b");
  c.outline();
  return prop(c, 3);
}

/** A market stall: a counter of goods under a striped awning on two posts. */
function stall(stripe: string, goods: string[]): SpriteSrc {
  const c = new Canvas(32, 32);
  c.shadow(16, 30, 15, 2.5);
  c.vline(2, 8, 22, "T").vline(29, 8, 22, "T");
  // Counter.
  c.rect(1, 20, 30, 9, "t").hline(1, 20, 30, "m").hline(1, 28, 30, "T");
  for (let x = 4; x < 30; x += 6) c.vline(x, 22, 6, "T");
  // Goods on the counter: whatever this stall sells, in rows.
  c.stamp(3, 16, goods);
  // Awning: stripes, and a scalloped front edge.
  c.rect(0, 2, 32, 8, "w");
  for (let x = 0; x < 32; x += 8) c.rect(x, 2, 4, 8, stripe);
  c.hline(0, 2, 32, "W");
  for (let x = 0; x < 32; x += 4) c.rect(x + 1, 10, 2, 1, x % 8 === 0 ? stripe : "w");
  c.outline();
  return prop(c, 2);
}

const APPLES = [
  "..rr.rr.ll.ll..nn.nn..yy.yy...",
  ".rrrrrrrlllllllnnnnnnyyyyyyy..",
  ".RrRrRrRlnlnlnlNnNnNnYyYyYyY..",
  "..............................",
];
const SEEDS = [
  ".tttt..mmmm..tttt..wwww..tttt.",
  "tmmmmttmyymmtmllmtwppwwtmoomt.",
  "tTTTTtTTTTTtTTTTTtWWWWWtTTTTt.",
  "..............................",
];
const LOAVES = [
  "..mmmm...mmmm...yy.yy..mmmm...",
  ".mttttm.mttttm.yyyyyy.mttttm..",
  ".tTTTTt.tTTTTt.YyYyYY.tTTTTt..",
  "..............................",
];

function washingLine(): SpriteSrc {
  const c = new Canvas(40, 24);
  c.shadow(20, 22, 19, 2);
  c.vline(1, 2, 21, "T").vline(38, 2, 21, "T").hline(0, 2, 3, "t").hline(37, 2, 3, "t");
  // The line sags a pixel in the middle.
  c.hline(2, 4, 12, "K").hline(14, 5, 12, "K").hline(26, 4, 12, "K");
  // A shirt, a sheet, a pinafore; pegs where they hang.
  c.rect(5, 5, 8, 7, "b").rect(4, 5, 2, 3, "b").rect(12, 5, 2, 3, "b").vline(9, 5, 7, "B").set(6, 5, "y").set(11, 5, "y");
  c.rect(16, 6, 9, 11, "w").vline(24, 6, 11, "W").hline(16, 16, 9, "W").set(17, 6, "y").set(23, 6, "y");
  c.rect(28, 5, 6, 3, "p").rect(27, 8, 8, 6, "p").vline(33, 5, 9, "P").set(29, 5, "y").set(32, 5, "y");
  c.outline();
  return prop(c, 1);
}

function parkBench(): SpriteSrc {
  const c = new Canvas(24, 16);
  c.shadow(12, 14.5, 11, 2);
  c.rect(2, 2, 20, 2, "t").rect(2, 5, 20, 2, "t").hline(2, 2, 20, "m").hline(2, 5, 20, "m");
  c.rect(1, 9, 22, 3, "t").hline(1, 9, 22, "m").hline(1, 11, 22, "T");
  c.vline(2, 1, 14, "d").vline(21, 1, 14, "d").vline(3, 12, 3, "d").vline(20, 12, 3, "d");
  c.outline();
  return prop(c, 1);
}

function flowerBed(): SpriteSrc {
  const c = new Canvas(24, 8);
  c.rect(0, 2, 24, 6, "e").hline(0, 7, 24, "v");
  const cols = ["r", "y", "u", "j", "w", "y", "r", "u"];
  for (let n = 0; n < 8; n++) {
    const x = 1 + n * 3;
    c.vline(x + 1, 3, 3, "n").set(x, 4, "l");
    c.set(x + 1, 2, cols[n]).set(x, 2, cols[(n + 3) % 8]).set(x + 2, 2, cols[n]).set(x + 1, 1, cols[n]);
  }
  c.outline();
  return prop(c, 1);
}

function pillarBox(): SpriteSrc {
  const c = new Canvas(8, 18);
  c.shadow(4, 16.5, 3.5, 1.5);
  c.rect(1, 3, 6, 13, "r").vline(5, 3, 13, "z").vline(1, 3, 13, "j");
  c.rect(0, 1, 8, 3, "r").hline(1, 0, 6, "r").hline(0, 3, 8, "z").hline(1, 1, 5, "j");
  c.hline(2, 5, 4, "k").rect(2, 8, 4, 3, "w").hline(2, 10, 4, "W").hline(1, 15, 6, "k");
  c.outline();
  return prop(c, 1);
}

function phoneBox(): SpriteSrc {
  const c = new Canvas(16, 36);
  c.shadow(8, 33, 8, 2.5);
  c.rect(1, 4, 14, 30, "r").vline(13, 4, 30, "z").vline(1, 4, 30, "j");
  c.rect(2, 1, 12, 3, "r").hline(3, 0, 10, "r").hline(2, 1, 11, "j");
  // TELEPHONE: a white strip with the word's shape in it.
  c.rect(3, 5, 10, 2, "w");
  for (let x = 4; x < 12; x += 2) c.set(x, 5, "k");
  // Glazing bars.
  c.rect(3, 9, 10, 18, "i");
  for (let y = 9; y < 27; y += 4) c.hline(3, y, 10, "r");
  c.vline(8, 9, 18, "r").rect(9, 14, 3, 4, "b");
  c.rect(2, 29, 12, 4, "z");
  c.outline();
  return prop(c, 2);
}

function memorial(): SpriteSrc {
  const c = new Canvas(16, 36);
  c.shadow(8, 33, 8, 2.5);
  c.rect(1, 28, 14, 6, "g").hline(1, 28, 14, "W").vline(14, 28, 6, "G");
  c.rect(3, 22, 10, 6, "g").hline(3, 22, 10, "W").vline(12, 22, 6, "G");
  c.rect(5, 6, 6, 16, "g").vline(5, 6, 16, "W").vline(10, 6, 16, "G");
  c.rect(4, 3, 8, 3, "g").hline(4, 3, 8, "W").rect(7, 0, 2, 4, "g");
  // Names, and one that is newer than the rest.
  for (let y = 9; y < 21; y += 2) c.hline(6, y, 4, "G");
  c.hline(6, 21, 3, "w");
  c.outline();
  return prop(c, 2);
}

// --- signs on brackets, one per trade --------------------------------------------------------

function hangingSign(board: string, rim: string, icon: string[]): SpriteSrc {
  const c = new Canvas(16, 26);
  c.shadow(4, 24.5, 3, 1.5);
  c.vline(3, 2, 22, "T").vline(4, 2, 22, "t").hline(3, 3, 12, "d");
  c.vline(7, 4, 2, "d").vline(13, 4, 2, "d");
  c.rect(6, 6, 9, 10, board).hline(6, 6, 9, rim).hline(6, 15, 9, rim).vline(6, 6, 10, rim).vline(14, 6, 10, rim);
  c.stamp(7, 7, icon);
  c.outline();
  return prop(c, 1);
}

const ICON_CASTLE = ["y.y.y.y", "yyyyyyy", ".yYyYy.", ".yykyy.", ".yykyy.", ".YYYYY.", "......."];
const ICON_LETTER = ["wwwwwww", "wkwwwkw", "wwkwkww", "wwwkwww", "wwwwwww", "WWWWWWW", "......."];
const ICON_SHOE = [".kkkkk.", "kk...kk", "k.....k", "k.....k", "kk...kk", ".k...k.", "......."];
const ICON_APPLE = ["...n...", "..rrr..", ".rrjrr.", ".rrrrr.", ".rrrrR.", "..RRR..", "......."];
const ICON_CROSS = ["..www..", "..www..", "wwwwwww", "wwwwwww", "..www..", "..www..", "......."];

// --- the lanes and gardens -------------------------------------------------------------------

function headstone(): SpriteSrc {
  const c = new Canvas(8, 16);
  c.shadow(4, 14.5, 3.5, 1.5);
  c.rect(1, 3, 6, 11, "g").rect(2, 2, 4, 1, "g").hline(2, 1, 4, "g");
  c.vline(1, 3, 11, "W").vline(6, 3, 11, "G").hline(2, 6, 4, "G").hline(2, 8, 4, "G").hline(2, 10, 3, "G");
  c.outline();
  return prop(c, 1);
}

function trough(): SpriteSrc {
  const c = new Canvas(24, 14);
  c.shadow(12, 12.5, 11, 2);
  c.rect(1, 3, 22, 9, "g").hline(1, 3, 22, "W").vline(22, 3, 9, "G").hline(1, 11, 22, "G");
  c.rect(3, 4, 18, 3, "b").hline(3, 4, 18, "i").set(8, 5, "i");
  c.outline();
  return prop(c, 1);
}

function hayBale(): SpriteSrc {
  const c = new Canvas(16, 18);
  c.shadow(8, 16, 8, 2);
  c.rect(1, 3, 14, 13, "y").rect(1, 3, 14, 4, "o").hline(1, 3, 14, "y");
  for (let y = 8; y < 16; y += 2) c.hline(2, y, 12, "Y");
  c.vline(5, 3, 13, "T").vline(11, 3, 13, "T");
  c.set(3, 2, "y").set(9, 2, "y").set(13, 2, "Y");
  c.outline();
  return prop(c, 2);
}

function farmCart(): SpriteSrc {
  const c = new Canvas(24, 24);
  c.shadow(12, 21.5, 11, 2.5);
  // Shafts forward, the bed, a load of sacks, one big wheel.
  c.hline(18, 13, 6, "T").hline(18, 16, 6, "T");
  c.rect(1, 9, 18, 8, "t").hline(1, 9, 18, "m").hline(1, 16, 18, "T");
  for (let x = 4; x < 18; x += 4) c.vline(x, 10, 6, "T");
  c.ell(6, 7, 4, 3, "m").ell(12, 7, 4, 3, "m").ell(9, 5, 3.5, 2.5, "t").set(5, 6, "w").set(11, 6, "w");
  c.ell(9, 18, 5, 5, "T").ell(9, 18, 3.5, 3.5, "t").ell(9, 18, 1.5, 1.5, "d");
  c.outline();
  return prop(c, 2);
}

function milkChurn(): SpriteSrc {
  const c = new Canvas(8, 14);
  c.shadow(4, 12.5, 3.5, 1.5);
  c.rect(1, 5, 6, 8, "g").vline(1, 5, 8, "W").vline(6, 5, 8, "G").rect(2, 2, 4, 3, "g").hline(2, 1, 4, "d").hline(1, 8, 6, "G");
  c.outline();
  return prop(c, 1);
}

function sacks(): SpriteSrc {
  const c = new Canvas(16, 14);
  c.shadow(8, 12.5, 7.5, 1.5);
  c.ell(4.5, 9, 4, 4, "m").ell(11.5, 9, 4, 4, "m").ell(8, 5.5, 4, 3.5, "t").set(8, 2, "T").set(4, 6, "T").set(12, 6, "T");
  c.hline(2, 11, 4, "t").hline(9, 11, 5, "t");
  c.outline();
  return prop(c, 1);
}

function anvil(): SpriteSrc {
  const c = new Canvas(16, 14);
  c.shadow(8, 12.5, 7, 1.5);
  c.rect(5, 8, 6, 5, "T").vline(5, 8, 5, "t");
  c.rect(1, 3, 14, 3, "d").rect(4, 6, 8, 2, "d").hline(3, 3, 11, "g").set(1, 4, "d").set(0, 4, "d");
  c.outline();
  return prop(c, 1);
}

function woodpile(): SpriteSrc {
  const c = new Canvas(24, 14);
  c.shadow(12, 12.5, 11, 1.5);
  for (let n = 0; n < 5; n++) c.ell(3 + n * 4.5, 10, 2.2, 2.2, "t").set(Math.round(3 + n * 4.5) - 1, 9, "m");
  for (let n = 0; n < 4; n++) c.ell(5.2 + n * 4.5, 6, 2.2, 2.2, "t").set(Math.round(5.2 + n * 4.5) - 1, 5, "m");
  for (let n = 0; n < 2; n++) c.ell(9.5 + n * 4.5, 2.5, 2.2, 2.2, "T");
  c.outline();
  return prop(c, 1);
}

function henCoop(): SpriteSrc {
  const c = new Canvas(24, 24);
  c.shadow(12, 22, 11, 2);
  c.rect(2, 9, 20, 12, "t");
  for (let y = 11; y < 21; y += 3) c.hline(2, y, 20, "T");
  // Roof: a lean-to of dark planks.
  for (let y = 0; y < 8; y++) c.hline(1 + Math.floor(y / 3), 2 + y, 22 - Math.floor(y / 3) * 2, y % 3 === 0 ? "T" : "v");
  c.rect(9, 14, 5, 7, "k").hline(9, 14, 5, "T");
  c.hline(10, 21, 4, "m").hline(11, 22, 3, "m");
  c.outline();
  return prop(c, 2);
}

function washingHeap(): SpriteSrc {
  const c = new Canvas(8, 8);
  c.shadow(4, 6.5, 4, 1.5);
  c.ell(4, 5, 3.5, 2, "w").rect(2, 3, 3, 2, "w").set(5, 4, "W").set(3, 5, "W").set(6, 5, "W").hline(2, 6, 4, "W");
  c.outline();
  return prop(c, 1);
}

// --- the Halt ------------------------------------------------------------------------------

function stationSign(): SpriteSrc {
  const c = new Canvas(32, 22);
  c.shadow(16, 20.5, 14, 1.5);
  c.vline(3, 8, 13, "d").vline(28, 8, 13, "d");
  c.rect(0, 2, 32, 8, "B").rect(1, 3, 30, 6, "w");
  // CASTLE HALT, as shapes.
  for (const x of [3, 6, 9, 12, 15, 18]) c.rect(x, 4, 2, 4, "k");
  for (const x of [22, 25, 28]) c.rect(x, 4, 2, 4, "k");
  c.outline();
  return prop(c, 1);
}

function shelter(): SpriteSrc {
  const c = new Canvas(40, 36);
  c.shadow(20, 34, 19, 2);
  // Back wall of planks, a bench along it, two posts, a pitched roof.
  c.rect(2, 12, 36, 20, "t");
  for (let x = 4; x < 38; x += 3) c.vline(x, 12, 20, "T");
  c.rect(4, 24, 32, 3, "m").hline(4, 27, 32, "T").vline(6, 27, 4, "T").vline(33, 27, 4, "T");
  c.vline(1, 10, 24, "d").vline(38, 10, 24, "d");
  for (let y = 0; y < 11; y++) c.hline(Math.max(0, 8 - y), 1 + y, 40 - Math.max(0, 8 - y) * 2, y % 2 === 0 ? "r" : "R");
  c.hline(0, 11, 40, "R");
  // A timetable pinned to the back wall.
  c.rect(28, 15, 6, 7, "w").hline(29, 17, 4, "G").hline(29, 19, 3, "G");
  c.outline();
  return prop(c, 2);
}

function ticketWindow(): SpriteSrc {
  const c = new Canvas(16, 16);
  c.rect(1, 2, 14, 12, "T").rect(3, 4, 10, 7, "i").rect(3, 8, 10, 3, "b").vline(8, 4, 7, "T");
  c.rect(1, 12, 14, 2, "m").rect(4, 0, 8, 3, "w").hline(5, 1, 6, "k");
  c.outline();
  return prop(c, 2);
}

function trolley(): SpriteSrc {
  const c = new Canvas(16, 16);
  c.shadow(8, 14.5, 7.5, 1.5);
  c.rect(1, 10, 14, 2, "d").set(3, 13, "k").set(12, 13, "k").vline(1, 3, 8, "d");
  c.rect(3, 5, 10, 5, "t").hline(3, 5, 10, "m").hline(7, 4, 3, "T");
  c.rect(5, 1, 7, 4, "z").hline(5, 1, 7, "r");
  c.outline();
  return prop(c, 1);
}

// --- on the buildings: what makes a roof a church, a barn or a home ------------------------

/** Stands on a roof ridge: a stone tower with a slate spire and a cross, rising well clear of it. */
function steeple(): SpriteSrc {
  const c = new Canvas(16, 56);
  c.rect(3, 26, 10, 30, "g").vline(3, 26, 30, "W").vline(12, 26, 30, "G");
  for (let y = 30; y < 56; y += 5) c.hline(4, y, 8, "G");
  // The belfry: an arched opening and the bell in it.
  c.rect(5, 30, 6, 8, "k").rect(6, 29, 4, 1, "k").ell(8, 35, 2, 2, "Y").set(7, 34, "y");
  c.set(7, 44, "k").set(8, 44, "k").rect(6, 45, 4, 3, "w").set(8, 46, "k").set(7, 46, "k");
  // Spire.
  for (let y = 0; y < 18; y++) {
    const half = Math.floor((y + 2) / 3);
    c.hline(8 - half, 8 + y, half * 2, y % 3 === 0 ? "d" : "G");
  }
  c.hline(2, 25, 12, "d").hline(2, 26, 12, "G");
  c.vline(8, 1, 7, "y").hline(6, 3, 5, "y");
  c.outline();
  return prop(c, 2);
}

/** A barn's double doors, set into its wall strip over the whole height of it, and chained. */
function barnDoors(): SpriteSrc {
  const c = new Canvas(32, 24);
  c.rect(0, 0, 32, 24, "T").rect(1, 1, 14, 23, "t").rect(17, 1, 14, 23, "t");
  for (let x = 3; x < 15; x += 3) c.vline(x, 1, 23, "T");
  for (let x = 19; x < 31; x += 3) c.vline(x, 1, 23, "T");
  // The X braces.
  for (let n = 0; n < 14; n++) {
    c.set(1 + n, 2 + Math.floor(n * 1.5), "m").set(14 - n, 2 + Math.floor(n * 1.5), "m");
    c.set(17 + n, 2 + Math.floor(n * 1.5), "m").set(30 - n, 2 + Math.floor(n * 1.5), "m");
  }
  c.rect(0, 10, 32, 3, "d").hline(0, 10, 32, "g");
  // The chain and its padlock, and the card on it.
  for (let x = 11; x < 21; x += 2) c.set(x, 13, "g").set(x + 1, 14, "g");
  c.rect(15, 15, 3, 3, "y").set(16, 16, "k").rect(18, 16, 4, 3, "w");
  return prop(c, 3);
}

function chimney(): SpriteSrc {
  const c = new Canvas(8, 16);
  c.rect(1, 3, 6, 13, "z").vline(1, 3, 13, "r").vline(6, 3, 13, "R");
  for (let y = 5; y < 16; y += 3) c.hline(1, y, 6, "R");
  c.rect(0, 1, 8, 3, "g").hline(0, 1, 8, "W").rect(2, 0, 4, 1, "k");
  c.outline();
  return prop(c, 1);
}

// --- inside: the Arms and the church -------------------------------------------------------

function pew(): SpriteSrc {
  const c = new Canvas(32, 14);
  c.shadow(16, 12.5, 15, 1.5);
  c.rect(1, 1, 30, 5, "T").hline(1, 1, 30, "t").rect(1, 7, 30, 3, "t").hline(1, 7, 30, "m").vline(1, 1, 12, "T").vline(30, 1, 12, "T");
  c.outline();
  return prop(c, 1);
}

function altar(): SpriteSrc {
  const c = new Canvas(32, 24);
  c.shadow(16, 22, 15, 2);
  c.rect(1, 6, 30, 15, "w").vline(30, 6, 15, "W").hline(1, 20, 30, "W");
  c.rect(12, 6, 8, 15, "z").vline(15, 8, 8, "y").hline(13, 10, 6, "y");
  c.rect(5, 1, 2, 5, "w").set(6, 0, "y").rect(25, 1, 2, 5, "w").set(26, 0, "y");
  c.outline();
  return prop(c, 2);
}

function barPumps(): SpriteSrc {
  const c = new Canvas(24, 20);
  c.rect(0, 8, 24, 12, "T").hline(0, 8, 24, "m").rect(0, 9, 24, 2, "t");
  for (const x of [5, 11, 17]) c.vline(x, 1, 8, "d").rect(x - 1, 1, 3, 2, "w").set(x, 3, "y");
  c.rect(20, 5, 3, 3, "y").rect(20, 4, 3, 1, "w");
  c.outline();
  return prop(c, 2);
}

const sheet: SpriteSheet = {
  town_fountain: fountain(),
  town_stall_greens: stall("n", APPLES),
  town_stall_seeds: stall("B", SEEDS),
  town_stall_bread: stall("r", LOAVES),
  town_washing_line: washingLine(),
  town_park_bench: parkBench(),
  town_flower_bed: flowerBed(),
  town_pillar_box: pillarBox(),
  town_phone_box: phoneBox(),
  town_memorial: memorial(),
  town_sign_arms: hangingSign("R", "t", ICON_CASTLE),
  town_sign_post: hangingSign("r", "w", ICON_LETTER),
  town_sign_forge: hangingSign("G", "d", ICON_SHOE),
  town_sign_stores: hangingSign("n", "t", ICON_APPLE),
  town_sign_doctor: hangingSign("B", "w", ICON_CROSS),
  town_headstone: headstone(),
  town_trough: trough(),
  town_hay_bale: hayBale(),
  town_farm_cart: farmCart(),
  town_milk_churn: milkChurn(),
  town_sacks: sacks(),
  town_anvil: anvil(),
  town_woodpile: woodpile(),
  town_hen_coop: henCoop(),
  town_washing_heap: washingHeap(),
  town_station_sign: stationSign(),
  town_shelter: shelter(),
  town_ticket_window: ticketWindow(),
  town_trolley: trolley(),
  town_steeple: steeple(),
  town_barn_doors: barnDoors(),
  town_chimney: chimney(),
  town_pew: pew(),
  town_altar: altar(),
  town_bar: barPumps(),
};

export default sheet;
