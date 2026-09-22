// The open country between the set places: cottages, barns, sheds and an inn, and the small
// things that gather round people who live somewhere (a well, a woodpile, a cart, a line of
// washing), and round things that do not (a den, a heap of bones, a cold camp). Same hand as
// art/props.ts: width is footprint w*8, tall props rise UP out of their footprint, light from
// the top-left, a 1 px dark outline.
//
// The buildings are painted by a small function rather than typed out, because a county has
// hundreds of them and a hamlet of four identical cottages reads as a print run. The painter
// takes a size, a roof (thatch, slate, tile), a wall (plaster, timber frame, stone, planks),
// where the door is and how many windows, and it lays courses, joints and flecks from a fixed
// hash, so every variant is a drawing you can regenerate and diff, not a random one.
// Each building also has an "on" frame: the same house with its windows lit, which the
// renderer shows after dark (the row is `nightOnly`, with a small warm light).

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // void: a window with nobody behind it, the inside of a den
  d: "#3a3e4a", // darkest stone / cold iron
  // Thatch: straw gone grey-gold in the weather.
  a: "#c09a52",
  A: "#86683a",
  h: "#dcc27e",
  // Slate.
  j: "#5c6679",
  J: "#3b4254",
  // Plaster, limewashed and not recently.
  u: "#e2d6bc",
  U: "#b3a488",
  // Stone, for walls, wells, milestones.
  c: "#a39d90",
  C: "#6e695f",
  // Brick and barn red.
  q: "#9a4c36",
  Q: "#673026",
  z: "#8a3a2e",
  Z: "#58241e",
  // Lit window, and the warm edge of it.
  v: "#ffd878",
  V: "#d08a3a",
  // Leaf, as in art/props.ts.
  f: "#5aa84e",
  F: "#2a6838",
  // Canvas.
  O: "#bdb39a",
  I: "#857b66",
  "-": "#00000030",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

// --- a tiny canvas ------------------------------------------------------------------------

type Canvas = { w: number; h: number; px: string[][] };

function canvas(w: number, h: number): Canvas {
  return { w, h, px: Array.from({ length: h }, () => Array.from({ length: w }, () => ".")) };
}
function dot(c: Canvas, x: number, y: number, ch: string): void {
  if (x >= 0 && y >= 0 && x < c.w && y < c.h) c.px[y][x] = ch;
}
function box(c: Canvas, x: number, y: number, w: number, h: number, ch: string): void {
  for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) dot(c, i, j, ch);
}
function rows(c: Canvas): string[] {
  return c.px.map((r) => r.join(""));
}
/** A fixed hash in 0..1: flecks and gaps that are the same every time the sheet is built. */
function fleck(x: number, y: number, s: number): number {
  let h = (Math.imul(x + 1, 0x27d4eb2d) ^ Math.imul(y + 7, 0x165667b1) ^ Math.imul(s + 3, 0x9e3779b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
function swap(src: string[], map: Record<string, string>): string[] {
  return src.map((r) => Array.from(r, (ch) => map[ch] ?? ch).join(""));
}

// --- buildings ------------------------------------------------------------------------------

type Roof = "thatch" | "slate" | "tile";
type Wall = "plaster" | "timber" | "stone" | "planks";
type House = {
  /** Footprint in cells. */
  w: number;
  h: number;
  roof: Roof;
  wall: Wall;
  /** Door centre as a fraction of the width. */
  door: number;
  /** Door width in px: 8 for a cottage, 20 for a barn's double doors. */
  doorW?: number;
  windows: number;
  chimney?: number | null;
  /** A second chimney (the inn). */
  chimney2?: number;
  /** A board on a bracket beside the door. */
  sign?: boolean;
  /** Share of the footprint the roof covers, top down. */
  roofShare?: number;
  seed: number;
};

const RISE = 10;

/** Paints one building, day or night. The night version only differs in the glass. */
function house(o: House, lit: boolean): string[] {
  const pw = o.w * 8;
  const fh = o.h * 8;
  const ph = fh + RISE;
  const c = canvas(pw, ph);
  const ry0 = RISE;
  const ry1 = RISE + Math.round(fh * (o.roofShare ?? 0.56));
  // Chimneys first: the roof is painted over their feet.
  for (const at of [o.chimney, o.chimney2]) {
    if (at === null || at === undefined) continue;
    const cx = Math.round(pw * at) - 3;
    box(c, cx, 1, 7, ry0 + 6, "k");
    box(c, cx + 1, 2, 5, ry0 + 4, "q");
    for (let y = 3; y < ry0 + 5; y += 3) box(c, cx + 1, y, 5, 1, "Q");
    dot(c, cx + 1, 2, "o");
    box(c, cx, 0, 7, 2, "k");
    box(c, cx + 1, 1, 5, 1, "d");
  }
  // The roof: full width, square to the wall's eaves, softened at the two top corners.
  for (let y = ry0; y < ry1; y++) {
    for (let x = 0; x < pw; x++) {
      const ry = y - ry0;
      const edge = x === 0 || x === pw - 1 || y === ry0;
      if ((ry === 0 && (x < 2 || x > pw - 3)) || (ry === 1 && (x === 0 || x === pw - 1))) {
        dot(c, x, y, ".");
        continue;
      }
      if (edge || (ry === 1 && (x === 1 || x === pw - 2))) {
        dot(c, x, y, "k");
        continue;
      }
      const f = fleck(x, y, o.seed);
      const leftLight = x < pw * 0.3;
      const rightShade = x > pw * 0.78;
      let ch: string;
      if (o.roof === "thatch") {
        // Ridge, then courses of straw, a darker line every four rows with gaps in it.
        if (ry === 1) ch = "A";
        else if (ry === 2) ch = x % 4 === 0 ? "A" : "a";
        else if (ry === 3) ch = "A";
        else if (ry % 4 === 3) ch = f < 0.18 ? "a" : "A";
        else if (f < 0.07 || (rightShade && f < 0.3)) ch = "A";
        else if (f > (leftLight ? 0.8 : 0.93)) ch = "h";
        else ch = "a";
      } else if (o.roof === "slate") {
        const course = ry % 3 === 0;
        const joint = (x + (Math.floor(ry / 3) % 2) * 3) % 6 === 0;
        if (ry === 1) ch = "J";
        else if (course || joint) ch = "J";
        else if (f > (leftLight ? 0.82 : 0.95)) ch = "g";
        else if (rightShade && f < 0.25) ch = "J";
        else ch = "j";
      } else {
        const course = ry % 3 === 0;
        const joint = (x + (Math.floor(ry / 3) % 2) * 2) % 4 === 0;
        if (ry === 1) ch = "R";
        else if (course) ch = "R";
        else if (joint && f < 0.6) ch = "R";
        else if (f > (leftLight ? 0.8 : 0.94)) ch = "o";
        else if (rightShade && f < 0.25) ch = "R";
        else ch = "r";
      }
      dot(c, x, y, ch);
    }
  }
  // The eave: the lowest course, ragged for thatch, and the black line under it.
  const eave = o.roof === "thatch" ? "A" : o.roof === "slate" ? "J" : "R";
  for (let x = 1; x < pw - 1; x++) dot(c, x, ry1 - 2, o.roof === "thatch" && fleck(x, ry1, o.seed) < 0.35 ? "a" : eave);
  box(c, 0, ry1 - 1, pw, 1, "k");
  if (o.roof === "thatch") for (let x = 2; x < pw - 2; x += 3) dot(c, x, ry1 - 1, fleck(x, 1, o.seed) < 0.5 ? "A" : "k");

  // The wall, two pixels in from the roof's edge each side.
  const wx0 = 2;
  const wx1 = pw - 3;
  const wy0 = ry1;
  const wy1 = ph - 1;
  for (let y = wy0; y <= wy1; y++) {
    for (let x = wx0; x <= wx1; x++) {
      if (x === wx0 || x === wx1 || y === wy1) {
        dot(c, x, y, "k");
        continue;
      }
      const f = fleck(x, y, o.seed + 11);
      let ch: string;
      if (y === wy1 - 1) ch = "C"; // a course of stone at the foot of every wall
      else if (o.wall === "stone") {
        const r = y - wy0;
        const mortar = r % 4 === 3 || (x + (Math.floor(r / 4) % 2) * 3) % 6 === 0;
        ch = mortar ? "C" : f > 0.85 ? "W" : "c";
      } else if (o.wall === "planks") {
        ch = x % 4 === 0 ? "Z" : f < 0.08 ? "Z" : "z";
      } else {
        ch = f < 0.05 ? "U" : "u";
      }
      if (x >= wx1 - 2 && ch !== "C" && o.wall !== "planks") ch = o.wall === "stone" ? "C" : "U";
      dot(c, x, y, ch);
    }
  }
  // The shadow the eave throws on the wall.
  for (let x = wx0 + 1; x < wx1; x++) dot(c, x, wy0, o.wall === "planks" ? "Z" : o.wall === "stone" ? "C" : "U");
  if (o.wall === "timber") {
    // Posts at the corners and between, a sill beam and a head beam.
    for (const x of [wx0 + 1, wx1 - 1, Math.round((wx0 + wx1) / 2)]) box(c, x, wy0 + 1, 1, wy1 - wy0 - 2, "T");
    box(c, wx0 + 1, wy0 + 1, wx1 - wx0 - 1, 1, "T");
    box(c, wx0 + 1, wy1 - 2, wx1 - wx0 - 1, 1, "T");
  }

  // The door, standing on the ground.
  const dw = o.doorW ?? 8;
  const dh = Math.min(dw > 10 ? wy1 - wy0 - 2 : 13, wy1 - wy0 - 2);
  const dx = Math.max(wx0 + 2, Math.min(wx1 - dw - 1, Math.round(pw * o.door) - (dw >> 1)));
  const dy = wy1 - dh;
  box(c, dx, dy, dw, dh, "k");
  box(c, dx + 1, dy + 1, dw - 2, dh - 1, "t");
  if (dw > 10) {
    // Barn doors: two leaves, each braced with a Z.
    const half = dw >> 1;
    box(c, dx + half, dy + 1, 1, dh - 1, "k");
    for (const lx of [dx + 1, dx + half + 1]) {
      const lw = half - 1;
      box(c, lx, dy + 1, lw, 1, "T");
      box(c, lx, dy + dh - 2, lw, 1, "T");
      for (let i = 0; i < dh - 3; i++) dot(c, lx + Math.round(((lw - 1) * i) / (dh - 4)), dy + dh - 2 - i, "T");
    }
  } else {
    for (let y = dy + 1; y < dy + dh; y++) {
      dot(c, dx + 3, y, "T");
      dot(c, dx + 5, y, "T");
    }
    box(c, dx + 1, dy + 1, dw - 2, 1, "T");
    dot(c, dx + dw - 3, dy + (dh >> 1) + 1, "y");
    // A step of worn stone in front of it.
    box(c, dx - 1, wy1, dw + 2, 1, "C");
    dot(c, dx - 1, wy1, "k");
    dot(c, dx + dw, wy1, "k");
  }
  if (o.sign) {
    // A board on an iron bracket, to the left of the door.
    const bx = dx - 10;
    box(c, bx, dy - 2, 9, 1, "d");
    box(c, bx + 1, dy - 1, 1, 1, "d");
    box(c, bx + 7, dy - 1, 1, 1, "d");
    box(c, bx, dy, 9, 6, "k");
    box(c, bx + 1, dy + 1, 7, 4, "m");
    box(c, bx + 2, dy + 2, 5, 1, "T");
    box(c, bx + 2, dy + 3, 3, 1, "T");
  }

  // Windows, spread along the wall and kept off the door.
  const slots: number[] = [];
  const n = o.windows;
  for (let i = 0; i < n; i++) {
    const at = Math.round(wx0 + ((wx1 - wx0) * (i + 1)) / (n + 1)) - 3;
    if (at + 8 > dx - 2 && at < dx + dw + 2) {
      // Budge it clear of the door, to whichever side has the room.
      const left = dx - 10;
      const right = dx + dw + 3;
      slots.push(left >= wx0 + 2 && !slots.includes(left) ? left : right);
    } else slots.push(at);
  }
  for (const wx of slots) {
    if (wx < wx0 + 2 || wx + 7 > wx1 - 1) continue;
    const wy = wy0 + 3;
    box(c, wx, wy, 7, 6, "k");
    // Dark glass by day, with a pale fleck where the sky is caught in it. Lit after dark.
    box(c, wx + 1, wy + 1, 5, 4, lit ? "v" : "x");
    if (lit) {
      box(c, wx + 1, wy + 4, 5, 1, "V");
    } else {
      dot(c, wx + 1, wy + 1, "B");
      dot(c, wx + 2, wy + 1, "G");
    }
    box(c, wx + 3, wy + 1, 1, 4, "k");
    box(c, wx + 1, wy + 2, 5, 1, "k");
    box(c, wx - 1, wy + 6, 9, 1, "W");
    dot(c, wx - 1, wy + 6, "k");
    dot(c, wx + 7, wy + 6, "k");
    if (o.wall !== "planks" && fleck(wx, wy, o.seed) < 0.6) {
      // A window box: somebody still waters these.
      for (let i = 0; i < 7; i++) dot(c, wx + i, wy + 7, i % 2 === 0 ? (fleck(wx + i, 3, o.seed) < 0.5 ? "r" : "y") : "n");
    }
  }
  return rows(c);
}

function building(o: House): SpriteSrc {
  return prop(o.w * 8, o.h * 8 + RISE, o.h, { base: house(o, false), on: house(o, true) });
}

// --- small things, drawn by hand -------------------------------------------------------------

const HAYSTACK = [
  "......kkkk......",
  "....kkhhaakk....",
  "...khhhaaaaAk...",
  "..khhaaaahaaAk..",
  "..khaaahaaaaAk..",
  ".khhaaaaaaaaAAk.",
  ".khaahaaaaAaaAk.",
  "khhaaaaaaaaaaAAk",
  "khaaaaAaaahaaAAk",
  "khaahaaaaaaaAAAk",
  "kaaaaaaaaAaaAAAk",
  "kaaaaAaaaaaaAAAk",
  "kAaaaaaaaaaAAAAk",
  "kAAaaaaAaaAAAAAk",
  ".kAAAaaaaAAAAAk.",
  "..kkAAAAAAAAkk..",
  "...kkkkkkkkkk...",
  "..============..",
  "................",
  "................",
];

const HAY_CART = [
  "......kkkkkkkkk.........",
  "....kkhhaaahaaakkk......",
  "..kkhaaaahaaaaaaaAk.....",
  ".khaahaaaaaaAaaaaAAk....",
  "kkkkkkkkkkkkkkkkkkkkk...",
  "kmmmmmmmmmmmmmmmmmmtk...",
  "kmtttttttttttttttttTkkkk",
  "kTTTTTTTTTTTTTTTTTTTkttk",
  "kkkkkkkkkkkkkkkkkkkkk.kk",
  "...kkkk.......kkkk......",
  "..kmTTTk.....kmTTTk.....",
  ".kmTkkTTk...kmTkkTTk....",
  ".kTkddkTk...kTkddkTk....",
  ".kTTkkTTk...kTTkkTTk....",
  "..kTTTTk.....kTTTTk.....",
  "...kkkk.......kkkk......",
];

const CART_WRECK = [
  "........................",
  "........kkkkk...........",
  "......kkmmmmtkk.........",
  "....kkmmttttttTk........",
  "..kkmtttTTTTTTTk..kkk...",
  ".kmttTTkkkkkkkkk.kmTTk..",
  "kmtTTkk.........kmTkTTk.",
  "kTTkk...kkkk....kTkdkTk.",
  "kkk....kttTTkk..kTTkTTk.",
  "......kttTTTTTk..kTTTk..",
  ".....kkkkkkkkkkk..kkk...",
  "...kk....kk.............",
  "..kmTk..kmTk............",
  "...kk....kk.............",
  "....=======.............",
  "........................",
];

const WOODPILE = [
  "................",
  "..kkk.kkk.kkk...",
  ".kmtTkmtTkmtTk..",
  ".kttTkttTkttTkk.",
  "kkkkkkkkkkkkkmtk",
  "kmtTkmtTkmtTkttk",
  "kttTkttTkttTkkk.",
  "kkkkkkkkkkkkkmtk",
  "kmtTkmtTkmtTkttk",
  "kttTkttTkttTkTk.",
  "kkkkkkkkkkkkkkk.",
  ".=============..",
];

const STUMP = ["........", "..kkkk..", ".kmmmtk.", "kmmTmmTk", "kmmmmtTk", "kTTmTTTk", ".kTTTTk.", "..=kk=.."];
const STUMP_AXE = ["...kk...", "..kdGk..", ".kmdgtk.", "kmmkmmTk", "kmmkmtTk", "kTTkTTTk", ".kTTTTk.", "..=kk=.."];
const STUMP_OLD = ["........", "..kkk...", ".kmTtk..", "kmmTmtk.", "kmTmmTTk", "kTTTTTTk", ".kfTTTk.", "..=kk=.."];

const LOG = ["................", "..kkkkkkkkkkkk..", ".kmtttttttttTTk.", "kmTmttttTtttTmtk", "kmmTtttttttTTmmk", "kTttTTTTTTTTTTTk", ".kkkkkkkkkkkkkk.", "..============.."];

const TENT = [
  "...........kk...........",
  "..........kOIk..........",
  ".........kOOIIk.........",
  "........kOOOIIIk........",
  ".......kOOOOIIIIk.......",
  "......kOOOOkIIIIIk......",
  ".....kOOOOkxkIIIIIk.....",
  "....kOOOOkxxxkIIIIIk....",
  "...kOOOOOkxxxkIIIIIIk...",
  "..kOOOOOkxxxxxkIIIIIIk..",
  ".kOOOOOOkxxxxxkIIIIIIIk.",
  "kOOOOOOkxxxxxxxkIIIIIIIk",
  "kOOOOOOkxxxxxxxkIIIIIIIk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  ".t....................T.",
  "..====================..",
];

const BEDROLL = ["..kkkkkkkkkkkk..", ".kjjjjjjjjjjjOk.", "kjJJjjjjjjjjjOOk", "kjjjjjjJjjjjjOOk", "kJjjjjjjjjjJjOOk", ".kkkkkkkkkkkkkk.", "..============..", "................"];

const DEN = [
  "........................",
  "......kkkkkkkkkk........",
  "....kkeeeeeeeeeekkk.....",
  "...keeeeeteeeeeeeeekk...",
  "..keeteeeeeeeeteeeeeek..",
  ".keeeeeekkkkkkeeeeteeek.",
  ".keeteekxxxxxxkkeeeeeek.",
  "keeeeekxxxxxxxxxkeeteeek",
  "keeteekxxxxxxxxxkeeeeeek",
  "keeeeekxxxxxxxxxkeeeeTek",
  "kTeeeeekxxxxxxxkeeeeeTTk",
  "kTTeeeekxxxxxxxkeeeTTTTk",
  ".kTTTeeWkkkkkkkWeeTTTTk.",
  "..kkTTkWkW..kWkWkTTTkk..",
  "....kkkk.....kkkkkkk....",
  "...==================...",
  "........................",
  "........................",
];

const BONES = ["........", ".kk..kk.", "kWWkkWWk", ".kWWWWk.", ".kWWWWk.", "kWWkkWWk", ".kk..kk.", "........"];
const BONES2 = ["........", "..kkk...", ".kWWWk..", "kWkWkWk.", "kWWWWWk.", ".kWkWk.k", "..kkk.kW", "......k."];

const MILESTONE = [
  "..kkkk..",
  ".kccccK.",
  "kcWccCCk",
  "kccccCCk",
  "kckkkkCk",
  "kccccCCk",
  "kckkcCCk",
  "kccccCCk",
  "kcccCCCk",
  "kCCCCCCk",
  ".kkkkkk.",
  ".======.",
];

const SHRINE = [
  "....kkkkkkkk....",
  "..kkjjjjjjjjkk..",
  ".kjjjjJjjjjjjjk.",
  "kJJJJJJJJJJJJJJk",
  "kkkkkkkkkkkkkkkk",
  "..kcxxxxxxxxCk..",
  "..kcxxxkkxxxCk..",
  "..kcxxkWWkxxCk..",
  "..kcxxxkkxxxCk..",
  "..kcxxxhyxxxCk..",
  "..kcxxxyoxxxCk..",
  "..kcrylkkryxCk..",
  "..kCCCCCCCCCCk..",
  "..kkkkkkkkkkkk..",
  "......kcCk......",
  "......kcCk......",
  "......kcCk......",
  "......kcCk......",
  "....kkccCCkk....",
  "...kcccccCCCk...",
  "..kcccccCCCCCk..",
  "..kCCCCCCCCCCk..",
  "..kkkkkkkkkkkk..",
  "...==========...",
];

const STANDING_STONE = [
  "..kkk...",
  ".kcWck..",
  ".kcccCk.",
  "kccccCk.",
  "kcWccCk.",
  "kccccCCk",
  "kcccCCCk",
  "kccncCCk",
  "kcccCCCk",
  "kcWccCCk",
  "kccccCCk",
  "kcccCCnk",
  "kccCCCCk",
  "kcccCCCk",
  "kccncCCk",
  "kcccCCCk",
  "kccCCCCk",
  "kccccCCk",
  "knnCCnNk",
  ".kkkkkk.",
  "-======-",
  "........",
];
const STANDING_STONE_2 = [
  "........",
  "........",
  "........",
  "........",
  "...kkk..",
  "..kcWck.",
  ".kcccCk.",
  ".kccccCk",
  "kcWcccCk",
  "kccccCCk",
  "kcccCCCk",
  "kccncCCk",
  "kcccCCCk",
  "kcccCCCk",
  "kccCCCnk",
  "kcccCCCk",
  "kccCCCCk",
  "kcccCCCk",
  "knnCCnNk",
  ".kkkkkk.",
  "-======-",
  "........",
];
const STANDING_STONE_3 = [
  "........",
  "........",
  "........",
  "........",
  "........",
  "........",
  "........",
  "........",
  "........",
  "..kkkk..",
  ".kcWcck.",
  "kccccCCk",
  "kcWccCCk",
  "kcccCCCk",
  "kccncCCk",
  "kcccCCCk",
  "kccCCCnk",
  "kcccCCCk",
  "knnCCnNk",
  ".kkkkkk.",
  "-======-",
  "........",
];

const BOULDER = [
  "................",
  "................",
  "....kkkkkk......",
  "..kkWWcccckk....",
  ".kWWcccccccCk...",
  ".kWccccccccCCk..",
  "kWcccccccccCCCk.",
  "kcccccccccCCCCk.",
  "kcccccnccCCCCCCk",
  "kccccnNcCCCCCCCk",
  "kCccccCCCCCCCCdk",
  ".kCCCCCCCCCCCddk",
  "..kkCCCCCCCdddk.",
  "....kkkkkkkkkk..",
  "...==========...",
  "................",
];

const FLOWERS = ["........", ".r...y..", "rlr.yly.", ".l...l..", ".l.w.l..", "..wlw...", "...l....", "..=-=..."];
const FLOWERS2 = ["........", "...b....", "..blb.p.", "...l.plp", ".y.l..l.", "yly..l..", ".l......", ".=-=-=.."];
const FLOWERS3 = ["........", "..w.....", ".wyw..w.", "..l..wyw", "..l...l.", "..ll.l..", "...ll...", "..=-=-.."];

const FLOWERBED = [
  "................",
  ".r.y.p.r.y.w.r..",
  "rlrylplrylwlrlr.",
  ".l.l.l.l.l.l.l..",
  "kkkkkkkkkkkkkkkk",
  "keeteeeteeeeteek",
  "kcccccccccccccCk",
  "kCCCCCCCCCCCCCCk",
  ".kkkkkkkkkkkkkk.",
  "................",
];

const BEEHIVE = ["...kk...", "..khak..", ".khaaAk.", ".kaAAak.", "khaaaaAk", "kAAAAAAk", "kaaxaaAk", "kAaaaAAk", "kaAAAAAk", "kkkkkkkk", ".kTkkTk.", "..=--=.."];

const TROUGH = ["................", "kkkkkkkkkkkkkkkk", "kccccccccccccCCk", "kcbbbbbbbbbbbCCk", "kcBbbbbbbbbbBCCk", "kCCCCCCCCCCCCCdk", "kCCCCCCCCCCCCddk", "kkkkkkkkkkkkkkkk", "..k........k....", "..============.."];

const WASHING_LINE = [
  "kk....................kk",
  "kTk..................kTk",
  "kTkkkkkkkkkkkkkkkkkkkkTk",
  "kTk.kkkkk....kkkkkk..kTk",
  "kTk.kwwwk....kbbbbk..kTk",
  "kTk.kwWwk....kbBbbk..kTk",
  "kTk.kwwwk....kbbbBk..kTk",
  "kTk.kWwWk....kbbbbk..kTk",
  "kTk.kkkkk....kbBbbk..kTk",
  "kTk..........kkkkkk..kTk",
  "kTk..................kTk",
  "kTk..................kTk",
  "kTk..................kTk",
  "kTk..................kTk",
  "kTk..................kTk",
  "kTk..................kTk",
  "kkk..................kkk",
  "=-=..................=-=",
];

const PUMP = ["..kkk...", ".kdGdk..", ".kdgdkkk", ".kdgdddk", ".kdgdkkk", "kkdgdk..", "kdkgdk..", "kkdgdk..", ".kdgdk..", ".kdgdk..", "kkdgdkk.", "kdddddk.", "kkkkkkk.", ".=====.."];

const CROP = ["........", "........", "..l.l...", ".lnlnl..", ".nlnln..", "..nNn...", "..=-=...", "........"];
const CROP2 = ["....y...", "..y.Y.y.", "..Y.l.Y.", "..l.l.l.", "..l.l.l.", "..lll...", "...=-=..", "........"];
const CROP3 = ["........", "...ll...", "..lfFl..", ".lfnfFl.", ".lffFFl.", "..lFFl..", "..=--=..", "........"];

const SLAG_HEAP = [
  "........kkkk............",
  "......kkGggGkk..........",
  "....kkGgGGGGGGkk........",
  "...kGgGGdGGGGdGGkk......",
  "..kGgGGGGGGGdGGGGGkk....",
  ".kGGGGdGGGGGGGGGdGGGkk..",
  ".kGgGGGGGGdGGGGGGGGGGGk.",
  "kGGGGGGdGGGGGGdGGGGdGGGk",
  "kdGGdGGGGGGGGGGGGGGGGGdk",
  "kddGGGGGdGGGGGGGdGGGGddk",
  ".kdddGGGGGGGdGGGGGGdddk.",
  "..kkddddddddddddddddkk..",
  "....kkkkkkkkkkkkkkkk....",
  "...================.....",
  "........................",
  "........................",
];

const SLEEPERS = ["................", "kkkkkkkkkkkkkkkk", "ktTTtTTTtTTTtTTk", "kkkkkkkkkkkkkkkk", "kTtTTTtTTTTtTTTk", "kkkkkkkkkkkkkkkk", "ktTTTtTTtTTTTtTk", "kkkkkkkkkkkkkkkk"];

const GRAVESTONE = ["..kkkk..", ".kccccK.", "kcWcccCk", "kckkkcCk", "kccccCCk", "kckkcCCk", "kccccCCk", "kcccCCCk", "knCCCCnk", ".kkkkkk.", ".======."];
const GRAVESTONE2 = ["........", "...kk...", "..kcck..", ".kkcckk.", "kcccccCk", ".kkcckk.", "..kcCk..", "..kcCk..", ".kncCnk.", ".kkkkkk.", ".======."];
const GRAVESTONE3 = ["........", "........", "........", ".kkkkk..", "kcccccCk", "kcWkkcCk", "kccccCCk", "kcCCCCCk", "knCCCnNk", ".kkkkkk.", ".======."];

const FINGERPOST = [
  "......kkkk......",
  ".....kmtTk......",
  "kkkkkkmtTkkkkk..",
  "kmmmmmmtTmmmmmk.",
  "kTkkTkkmTkkTkTTk",
  "kTTTTTTtTTTTTTk.",
  "kkkkkkmtTkkkkk..",
  "..kkkkmtTkkkkkkk",
  ".kmmmmmtTmmmmmmk",
  "kTTkTkkmTkTkkTTk",
  ".kTTTTTtTTTTTTTk",
  "..kkkkmtTkkkkkkk",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  "......kmtTk.....",
  ".....kkmtTkk....",
  ".....kmmttTk....",
  ".....kkkkkkk....",
  "....=======.....",
];

const WEB = [
  "k......k.......k",
  ".W....W.W.....W.",
  "..W..W...W...W..",
  "...WWWWWWWWWW...",
  "...W.W..W..WW...",
  "..W..W..W..W.W..",
  ".W...WWWWWWW..W.",
  "WWWWWW..W..WWWWW",
  ".W...WWWWWWW..W.",
  "..W..W..W..W.W..",
  "...W.W..W..WW...",
  "...WWWWWWWWWW...",
  "..W..W...W...W..",
  ".W....W.W.....W.",
  "k......k.......k",
  "................",
];

const HEN_COOP = [
  "....kkkkkkkk....",
  "..kkttttttttkk..",
  ".kttTttttTtttTk.",
  "kTTTTTTTTTTTTTTk",
  "kkkkkkkkkkkkkkkk",
  "kmtkmtkkkkmtkmtk",
  "kmtkmtkxxkmtkmtk",
  "kmtkmtkxxkmtkmtk",
  "kmtkmtkkkkmtkmtk",
  "kTTTTTTTTTTTTTTk",
  "kkkkkkkkkkkkkkkk",
  ".k.k..W...W.k.k.",
  "..============..",
  "................",
];

/** A place's name, painted white on a board between two posts, the letters too small to read from the road. */
const NAME_BOARD = [
  "................",
  "................",
  ".kkkkkkkkkkkkkk.",
  "kuuuuuuuuuuuuuUk",
  "kuxxuxxxuxxuxuUk",
  "kuuuuuuuuuuuuuUk",
  "kuxuxxuxxxuxxuUk",
  "kuuuuuuuuuuuuuUk",
  "kUUUUUUUUUUUUUUk",
  ".kkkkkkkkkkkkkk.",
  "..kmk......kmk..",
  "..kmk......kmk..",
  "..kmk......kmk..",
  "..kmk......kmk..",
  "..kmk......kmk..",
  "..kTk......kTk..",
  "..kTk......kTk..",
  "..kkk......kkk..",
  "..===......===..",
  "................",
];

const sheet: SpriteSheet = {
  name_board: prop(16, 20, 1, { base: NAME_BOARD }),
  // Cottages: four roofs-and-walls a hamlet can mix, so no two neighbours are the same house.
  cottage_thatch: building({ w: 8, h: 6, roof: "thatch", wall: "plaster", door: 0.5, windows: 2, chimney: 0.78, seed: 1 }),
  cottage_timber: building({ w: 8, h: 6, roof: "thatch", wall: "timber", door: 0.3, windows: 2, chimney: 0.2, seed: 2 }),
  cottage_slate: building({ w: 8, h: 6, roof: "slate", wall: "stone", door: 0.62, windows: 2, chimney: 0.25, seed: 3 }),
  cottage_tile: building({ w: 9, h: 6, roof: "tile", wall: "plaster", door: 0.4, windows: 3, chimney: 0.82, seed: 4 }),
  // A long low farmhouse, a barn with doors a cart goes through, a shed, and the inn.
  farmhouse: building({ w: 10, h: 6, roof: "slate", wall: "plaster", door: 0.5, windows: 3, chimney: 0.14, chimney2: 0.86, seed: 5 }),
  barn: building({ w: 10, h: 7, roof: "slate", wall: "planks", door: 0.5, doorW: 22, windows: 0, chimney: null, roofShare: 0.5, seed: 6 }),
  shed: building({ w: 4, h: 3, roof: "slate", wall: "planks", door: 0.5, windows: 0, chimney: null, roofShare: 0.5, seed: 7 }),
  inn: building({ w: 12, h: 7, roof: "slate", wall: "stone", door: 0.55, windows: 4, chimney: 0.12, chimney2: 0.88, sign: true, seed: 8 }),
  reed_hut: building({ w: 6, h: 5, roof: "thatch", wall: "planks", door: 0.5, windows: 1, chimney: null, seed: 9 }),

  haystack: prop(16, 20, 2, { base: HAYSTACK }),
  hay_cart: prop(24, 16, 2, { base: HAY_CART }),
  cart_wreck: prop(24, 16, 2, { base: CART_WRECK }),
  woodpile: prop(16, 12, 1, { base: WOODPILE }),
  stump: prop(8, 8, 1, { base: STUMP, base2: STUMP_AXE, base3: STUMP_OLD }),
  log: prop(16, 8, 1, { base: LOG }),
  tent: prop(24, 16, 2, { base: TENT }),
  bedroll: prop(16, 8, 1, { base: BEDROLL }),
  den: prop(24, 18, 2, { base: DEN }),
  bones: prop(8, 8, 1, { base: BONES, base2: BONES2 }),
  milestone: prop(8, 12, 1, { base: MILESTONE }),
  wayside_shrine: prop(16, 24, 2, { base: SHRINE }),
  standing_stone: prop(8, 22, 1, { base: STANDING_STONE, base2: STANDING_STONE_2, base3: STANDING_STONE_3 }),
  boulder: prop(16, 16, 2, { base: BOULDER }),
  flowers: prop(8, 8, 1, { base: FLOWERS, base2: FLOWERS2, base3: FLOWERS3 }),
  flowerbed: prop(16, 10, 1, { base: FLOWERBED }),
  beehive: prop(8, 12, 1, { base: BEEHIVE }),
  trough: prop(16, 10, 1, { base: TROUGH }),
  washing_line: prop(24, 18, 1, { base: WASHING_LINE }),
  pump: prop(8, 14, 1, { base: PUMP }),
  crop: prop(8, 8, 1, { base: CROP, base2: CROP2, base3: CROP3 }),
  slag_heap: prop(24, 16, 2, { base: SLAG_HEAP }),
  sleepers: prop(16, 8, 1, { base: SLEEPERS }),
  gravestone: prop(8, 11, 1, { base: GRAVESTONE, base2: GRAVESTONE2, base3: GRAVESTONE3 }),
  fingerpost: prop(16, 24, 1, { base: FINGERPOST }),
  web: prop(16, 16, 2, { base: WEB }),
  hen_coop: prop(16, 14, 1, { base: HEN_COOP }),
  // The dead day and night versions of a cottage nobody lives in any more: the same house, no lit frame.
  cottage_empty: prop(64, 58, 6, { base: swap(house({ w: 8, h: 6, roof: "thatch", wall: "plaster", door: 0.5, windows: 2, chimney: 0.78, seed: 12 }, false), { r: "n", y: "N" }) }),
};

export default sheet;
