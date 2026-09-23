// How each thing lies once it is dead. A dead thing has to read at a glance as dead AND as what
// it was: people fall on their backs, small animals go feet up, machines come apart, a shade
// thins out, a plant keels over. Most bodies are made here from the unit's own drawings,
// so a villager in a red coat is a villager in a red coat on the ground, in her own colours,
// without a hand-drawn frame per person. A sheet that draws its own "dead" keeps it, and may
// still ask for a pool under it.
//
// Every unit sprite must appear in CORPSES or draw a "dead" frame of its own (test/art.test.ts):
// there is no silent fallback to a standing drawing.

import type { Palette, SpriteSrc } from "@/art/types";

export type CorpseStyle =
  /** Rotated from "side" onto its back, head west, a dark pool under the chest. People. */
  | "fallen"
  /** The whole standing drawing tipped over onto its side. Plants on a stem. */
  | "topple"
  /** "down" upside down on the ground, wings open and still. The Emperor. */
  | "flat"
  /** Drawn by hand in CorpseSpec.frame, in the sprite's own palette. Small animals. */
  | "drawn"
  /** "side" turned belly up, feet in the air, a small pool. Birds, sheep, rats, spiders. */
  | "legsUp"
  /** Fallen, then thinned to a third of itself, no blood, a few pale motes. Shades. */
  | "wisp"
  /** The standing drawing broken into plates and laid in a heap, with an oil stain. Machines, armour. */
  | "scrap"
  /** Wax: the standing drawing slumped into a puddle of itself. */
  | "melt"
  /** Keeps its own hand-drawn "dead" frame, and puts a pool under it. */
  | "hand+pool"
  /** Keeps its own hand-drawn "dead" frame exactly as drawn (bones, rubble, ash). */
  | "hand";

export type CorpseSpec = {
  style: CorpseStyle;
  pool?: "blood" | "oil" | "sap" | "stuffing" | "wax" | "ichor";
  /** "drawn": the body, by hand, in the sprite's own palette characters (the frame's size). */
  frame?: string[];
};

// Small animals are too small for a rule to lay them down well; these are drawn. Each lies on
// its side, head east, eye shut (a k where the eye was), legs out from under it.

/** Cat, 12 x 10, feet on row 9. Tail out flat behind it. */
const CAT_DEAD = [
  "............",
  "............",
  "............",
  "............",
  "............",
  ".........k.k",
  "..kkkkkkkckk",
  "kkcccccccckk",
  "kccCccccCcck",
  ".kkckkckkkk.",
];

/** Rabbit, 10 x 11, feet on row 10. Ears laid back along the ground. */
const RABBIT_DEAD = [
  "..........",
  "..........",
  "..........",
  "..........",
  "..........",
  "....kkk...",
  "..kkAAAkk.",
  ".kaaaaaaak",
  "kWaaaaakak",
  "kWaaaAAaak",
  ".kkkkkkkk.",
];

/** The mounted fox, 14 x 10: off its stand and on its side, the stitching showing. */
const FOX_DEAD = [
  "..............",
  "..............",
  "..............",
  "............k.",
  "...........kfk",
  ".kkkkkkkkkkffk",
  "kfffffffffffkk",
  "kFffffffffffFk",
  ".kFkkFkkFkkFk.",
  "..kk.kk.kk.kk.",
];

/** Butterfly and moth, 12 x 10: wings shut, on its side on the ground, feet in the air. */
const WINGS_DEAD = (f: string, F: string, b: string): string[] => [
  "............",
  "............",
  "............",
  "............",
  "............",
  "............",
  "......k.k...",
  "..kkkkkkkk..",
  `.k${f}${F}${f}${f}${f}k${b}${b}k.`,
  "..kkkkkkkk..",
];

/**
 * The burial snake's head, 16 x 16 on (8, 8). Alive, the renderer strings its body out behind it;
 * dead, the body is not drawn, so the head brings a slack coil of its own, pale belly up.
 */
const SNAKE_DEAD = [
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "...........kkk..",
  ".kkkk.....knnnk.",
  "knnnnk...knkNnnk",
  "kccccnk.knccckk.",
  ".kkkcnnknccck...",
  "....kccccck.....",
  ".....kkkkk......",
  "................",
  "................",
  "................",
];

/** The bat, 16 x 12, down out of the air: on its back, wings spread limp, feet up. */
const BAT_DEAD = [
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "......k..k......",
  ".kkk.kPPPPk.kkk.",
  "kpPPkPPwwPPkPPpk",
  ".kkkkkkkkkkkkkk.",
];

/** The spider queen, 16 x 16: over on her back, the red belly up, legs curled in over it. */
const QUEEN_DEAD = [
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "...k.k....k.k...",
  "..kk.kk..kk.kk..",
  ".kkkkkkkkkkkkkk.",
  "kaarrrrrrrrrraak",
  "kaarrrrrrrrrraak",
  ".kkaaaaaaaaaakk.",
  "...kkkkkkkkkk...",
];

/** A hen, 10 x 10: on her side, feet stuck up, the head down with the comb on the ground. */
const HEN_DEAD = [
  "..........",
  "..........",
  "..........",
  "..........",
  "..........",
  "...y.y....",
  "..kykyk...",
  ".kwwwwwkrk",
  "kWwwWwwkwk",
  ".kkWWkkkk.",
];

/** A crow, 12 x 10: on its back in the furrow, feet curled up, the beak on the ground. */
const CROW_DEAD = [
  "............",
  "............",
  "............",
  "............",
  "............",
  "....y.y.....",
  "...kykyk....",
  ".kkxxxxxkkk.",
  "kxXxxXxxxkyy",
  ".kkkkkkkkk..",
];

/** A sheep, 16 wide, `h` tall: over on its side, four stiff legs out, the face (`x`) on the grass. */
const SHEEP_DEAD = (x: string, h: number): string[] => [
  ...Array<string>(h - 6).fill("................"),
  `...k${x}k.k${x}k.k${x}k..`,
  `...k${x}k.k${x}k.k${x}k..`,
  ".kkkkkkkkkkkk...",
  `kwwWwwwWwwwwk${x}${x}k`,
  `kWwwwwWwwwwWk${x}kk`,
  ".kkkkkkkkkkkk...",
];

/** Pools: [edge, core]. Muted, near black, never a bright red: this is not that kind of game. */
const POOLS: Record<NonNullable<CorpseSpec["pool"]>, [string, string]> = {
  blood: ["#4a1c22b0", "#2c0e14d8"],
  ichor: ["#2c3a28b0", "#18221ad8"],
  oil: ["#24242ab0", "#121216e0"],
  sap: ["#3a4a24a0", "#26321ac8"],
  stuffing: ["#b8ae96c0", "#8c826cd0"],
  wax: ["#d8c89ae0", "#b8a478f0"],
};

/**
 * Which sprite lies how. Keyed by SPRITE id (units.json `sprite`), not unit id: the five
 * rows that share the skeleton share its bones.
 */
export const CORPSES: Record<string, CorpseSpec> = {
  // --- hand-drawn, kept ---
  jane: { style: "hand+pool", pool: "blood" },
  dog: { style: "hand+pool", pool: "blood" },
  rat: { style: "hand+pool", pool: "blood" },
  bandit: { style: "hand+pool", pool: "blood" },
  ruffian: { style: "hand+pool", pool: "blood" },
  school_master: { style: "hand+pool", pool: "blood" },
  soldier: { style: "hand+pool", pool: "ichor" },
  skeleton: { style: "hand" },
  miniboss: { style: "hand" },
  school_caretaker: { style: "hand+pool", pool: "blood" },
  factory_charge_hand: { style: "hand+pool", pool: "oil" },
  boss: { style: "hand" },
  school_ringer: { style: "hand" },
  factory_foreman: { style: "hand+pool", pool: "oil" },
  statue: { style: "hand" },
  factory_sentry: { style: "hand+pool", pool: "oil" },
  pumpkin: { style: "hand+pool", pool: "sap" },
  cactus: { style: "hand+pool", pool: "sap" },
  flower: { style: "hand+pool", pool: "sap" },
  spider: { style: "hand+pool", pool: "ichor" },
  bat: { style: "drawn", pool: "blood", frame: BAT_DEAD },
  lurker: { style: "hand+pool", pool: "ichor" },
  snake: { style: "drawn", pool: "ichor", frame: SNAKE_DEAD },

  // --- machines ---
  factory_hauler: { style: "scrap", pool: "oil" },
  museum_armour: { style: "scrap" },

  // --- the museum's other exhibits ---
  museum_waxwork: { style: "melt", pool: "wax" },
  museum_fox: { style: "drawn", pool: "stuffing", frame: FOX_DEAD },
  museum_shot_firer: { style: "fallen", pool: "stuffing" },
  museum_attendant: { style: "fallen", pool: "stuffing" },

  // --- the burial chamber ---
  shade: { style: "wisp" },
  the_soldier: { style: "fallen", pool: "ichor" },
  goldskin: { style: "fallen", pool: "ichor" },
  great_flower: { style: "topple", pool: "sap" },
  spider_queen: { style: "drawn", pool: "ichor", frame: QUEEN_DEAD },

  // --- the forest ---
  butterfly: { style: "drawn", frame: WINGS_DEAD("f", "F", "y") },
  moth: { style: "drawn", frame: WINGS_DEAD("c", "C", "W") },
  emperor: { style: "topple", pool: "ichor" },
  collector: { style: "fallen", pool: "blood" },

  // --- the country's animals ---
  hen: { style: "drawn", pool: "blood", frame: HEN_DEAD },
  town_hen: { style: "drawn", pool: "blood", frame: HEN_DEAD },
  town_hen_brown: { style: "drawn", pool: "blood", frame: HEN_DEAD },
  crow: { style: "drawn", pool: "blood", frame: CROW_DEAD },
  rabbit: { style: "drawn", pool: "blood", frame: RABBIT_DEAD },
  sheep: { style: "drawn", pool: "blood", frame: SHEEP_DEAD("x", 13) },
  town_sheep: { style: "drawn", pool: "blood", frame: SHEEP_DEAD("G", 12) },
  town_cat_black: { style: "drawn", pool: "blood", frame: CAT_DEAD },
  town_cat_ginger: { style: "drawn", pool: "blood", frame: CAT_DEAD },
};

/** Every sprite id not listed above whose drawing is a standing person: the villagers, the townsfolk. */
export const PERSON_CORPSE: CorpseSpec = { style: "fallen", pool: "blood" };

export type Remains = NonNullable<CorpseSpec["pool"]> | "bone" | "stone" | "ghost";

/** What comes off a thing at the moment it dies (render/fx.ts `death`): the same stuff its pool is made of. */
export function remainsOf(sprite: string): Remains {
  const spec = CORPSES[sprite] ?? PERSON_CORPSE;
  if (spec.style === "wisp") return "ghost";
  if (spec.pool) return spec.pool;
  if (spec.style === "scrap") return "stone";
  if (spec.style === "drawn") return "ichor";
  return sprite === "skeleton" || sprite === "miniboss" ? "bone" : "stone";
}

// ------------------------------------------------------------------------------------------

type Grid = string[][];

const SPARE = "0123456789!#$%&*+=?@^~:;<>/|-_()[]{}',\"`\\";

function toGrid(rows: string[]): Grid {
  return rows.map((r) => r.split(""));
}

function blank(w: number, h: number): Grid {
  return Array.from({ length: h }, () => Array<string>(w).fill("."));
}

function bbox(g: Grid): { x0: number; y0: number; x1: number; y1: number } {
  let x0 = Infinity;
  let y0 = Infinity;
  let x1 = -1;
  let y1 = -1;
  g.forEach((row, y) =>
    row.forEach((c, x) => {
      if (c === ".") return;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }),
  );
  return { x0, y0, x1, y1 };
}

function crop(g: Grid): Grid {
  const b = bbox(g);
  if (b.x1 < 0) return [["."]];
  return g.slice(b.y0, b.y1 + 1).map((r) => r.slice(b.x0, b.x1 + 1));
}

/** Rotate a quarter turn anticlockwise: what faced east now faces up, the head goes west. */
function rotateCCW(g: Grid): Grid {
  const h = g.length;
  const w = g[0].length;
  const out = blank(h, w);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) out[w - 1 - x][y] = g[y][x];
  return out;
}

/**
 * Shorten a lying body along x until it fits `max`: drop the columns that repeat their
 * neighbour, from the legs end first (a drawing's legs are its most repetitive rows).
 */
function shorten(g: Grid, max: number): Grid {
  let cols = g[0].map((_, x) => g.map((r) => r[x]));
  while (cols.length > max) {
    let drop = -1;
    // Legs are the east half once rotated; look there for a repeated column first, then anywhere.
    for (let x = cols.length - 2; x > cols.length / 2 && drop < 0; x--) if (cols[x].join("") === cols[x + 1].join("")) drop = x;
    for (let x = cols.length - 2; x > 0 && drop < 0; x--) if (cols[x].join("") === cols[x - 1].join("")) drop = x;
    if (drop < 0) drop = Math.floor(cols.length * 0.7);
    cols.splice(drop, 1);
  }
  return g.map((_, y) => cols.map((c) => c[y]));
}

/**
 * A person on her back, head west, face up, arms at her sides, boots east: the shape Jane's own
 * dead frame has. Roles, filled from the person's own standing drawing:
 *   1 hair or hat   2 skin   4 coat   5 coat shade   6 legs   7 boots   k outline
 */
const LYING: string[] = [
  "..kkk..kkk......",
  ".k111kk442k.kk..",
  "k1122kk44kkk77k.",
  "k1k22k5444666kk.",
  "k1122k5444kkkk..",
  "k1k22k544466677k",
  ".k112kk4442kkkk.",
  "..kkk.kkkkk.....",
];

function mode(counts: Map<string, number>, not: Set<string>): string | undefined {
  let best: string | undefined;
  let n = 0;
  for (const [c, k] of counts) if (!not.has(c) && k > n) [best, n] = [c, k];
  return best;
}

function countRows(g: Grid, y0: number, y1: number): Map<string, number> {
  const m = new Map<string, number>();
  for (let y = Math.max(0, y0); y <= Math.min(g.length - 1, y1); y++) for (const c of g[y]) if (c !== ".") m.set(c, (m.get(c) ?? 0) + 1);
  return m;
}

/** Read a standing person's colours off her "down" drawing: which palette char is hair, skin, coat... */
function personRoles(down: Grid): Record<string, string> {
  const b = bbox(down);
  const out = new Set([".", "k", "K"]);
  const top = mode(countRows(down, b.y0 + 1, b.y0 + 3), out) ?? "k";
  const skin = mode(countRows(down, b.y0 + 4, b.y0 + 7), new Set([...out, top])) ?? top;
  const coatRows = countRows(down, b.y0 + 9, b.y0 + 13);
  const coat = mode(coatRows, new Set([...out, skin])) ?? top;
  const shade = mode(coatRows, new Set([...out, skin, coat])) ?? coat;
  const legs = mode(countRows(down, b.y1 - 4, b.y1 - 2), new Set([...out, skin])) ?? coat;
  const boots = mode(countRows(down, b.y1 - 1, b.y1 - 1), new Set([...out])) ?? legs;
  return { k: "k", "1": top, "2": skin, "4": coat, "5": shade, "6": legs, "7": boots };
}

function flipV(g: Grid): Grid {
  return g.slice().reverse().map((r) => r.slice());
}

/** Paste `src` onto `dst` with its bottom row on `bottom` and centred on `cx`. */
function paste(dst: Grid, src: Grid, cx: number, bottom: number): { x: number; y: number } {
  const w = src[0].length;
  const h = src.length;
  const x0 = Math.max(0, Math.min(dst[0].length - w, Math.round(cx - w / 2)));
  const y0 = Math.max(0, bottom - h + 1);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const c = src[y][x];
    if (c !== "." && dst[y0 + y]?.[x0 + x] !== undefined) dst[y0 + y][x0 + x] = c;
  }
  return { x: x0, y: y0 };
}

/** A soft pool on the ground under whatever is already there: edge ring, darker middle. */
function pool(g: Grid, cx: number, cy: number, rx: number, ry: number, edge: string, core: string): void {
  for (let y = Math.floor(cy - ry); y <= Math.ceil(cy + ry); y++) {
    for (let x = Math.floor(cx - rx); x <= Math.ceil(cx + rx); x++) {
      if (!g[y] || g[y][x] === undefined || g[y][x] !== ".") continue;
      const d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2;
      if (d <= 0.45) g[y][x] = core;
      else if (d <= 1) g[y][x] = edge;
    }
  }
}

function hexToRgba(hex: string): [number, number, number, number] {
  const h = hex.replace("#", "");
  const n = parseInt(h.length === 3 ? h.replace(/(.)/g, "$1$1") : h.slice(0, 6), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, h.length === 8 ? parseInt(h.slice(6, 8), 16) : 255];
}

function rgbaToHex(c: [number, number, number, number]): string {
  const p = (v: number): string => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return `#${p(c[0])}${p(c[1])}${p(c[2])}${c[3] >= 255 ? "" : p(c[3])}`;
}

/** Dead colour: a little darker and a little greyer than the living one. */
function pallor(hex: string, fade = 1): string {
  const [r, g, b, a] = hexToRgba(hex);
  const grey = (r + g + b) / 3;
  if (fade < 1) {
    // A shade going out: what colour it had goes to the grey-blue of the mist, and thins.
    const mist = [150, 160, 180];
    return rgbaToHex([r * 0.35 + mist[0] * 0.65, g * 0.35 + mist[1] * 0.65, b * 0.35 + mist[2] * 0.65, a * fade]);
  }
  const k = 0.86;
  return rgbaToHex([(r * 0.8 + grey * 0.2) * k, (g * 0.8 + grey * 0.2) * k, (b * 0.8 + grey * 0.2) * k, a]);
}

/**
 * Recolour a dead frame with fresh palette characters so the living frames keep their colours.
 * Returns the new palette entries; mutates the grid.
 */
function recolour(g: Grid, palette: Palette, taken: Set<string>, fade = 1): Palette {
  const map = new Map<string, string>();
  const add: Palette = {};
  for (const row of g) {
    for (let x = 0; x < row.length; x++) {
      const c = row[x];
      if (c === "." || !(c in palette)) continue;
      let n = map.get(c);
      if (!n) {
        n = [...SPARE].find((s) => !taken.has(s));
        if (!n) throw new Error("corpse: out of spare palette characters");
        taken.add(n);
        map.set(c, n);
        add[n] = pallor(palette[c], fade);
      }
      row[x] = n;
    }
  }
  return add;
}

function spare(taken: Set<string>): string {
  const n = [...SPARE].find((s) => !taken.has(s));
  if (!n) throw new Error("corpse: out of spare palette characters");
  taken.add(n);
  return n;
}

/** Break a standing drawing into plates and lay them in a heap on the ground. */
function scrapHeap(src: Grid, w: number, h: number, ground: number): Grid {
  const body = crop(src);
  const bh = body.length;
  const bw = body[0].length;
  const out = blank(w, h);
  // Plates of 4 x 4 (edge plates smaller), in reading order.
  const plates: Grid[] = [];
  for (let y = 0; y < bh; y += 4) for (let x = 0; x < bw; x += 4) {
    const p = crop(body.slice(y, y + 4).map((r) => r.slice(x, x + 4)));
    if (p.length > 1 || p[0].length > 1 || p[0][0] !== ".") if (bbox(p).x1 >= 0) plates.push(p);
  }
  // Heaviest first, into a low heap: two courses, the lower one wider, each plate outlined by its own pixels.
  plates.sort((a, b) => b.length * b[0].length - a.length * a[0].length);
  const courses = [
    { y: ground, x: Math.max(0, Math.floor((w - Math.min(w, plates.length * 3)) / 2)) },
    { y: ground - 3, x: Math.floor(w / 2) - 5 },
  ];
  plates.forEach((p, i) => {
    const course = courses[i % 5 < 3 ? 0 : 1];
    const at = paste(out, i % 2 === 1 ? p.map((r) => r.slice().reverse()) : p, course.x + p[0].length / 2, course.y);
    course.x = at.x + p[0].length + (i % 3 === 0 ? 1 : 0) - 1;
    if (course.x > w - 3) course.x = 1 + (i % 4);
  });
  return out;
}

/**
 * The sprite with a "dead" frame for its style. Pure: returns a new SpriteSrc, never mutates
 * the one it was given (palette swaps share frame tables with the sheet they came from).
 */
export function withCorpse(src: SpriteSrc, spec: CorpseSpec): SpriteSrc {
  const palette: Palette = { ...src.palette };
  const taken = new Set(Object.keys(palette));
  taken.add(".");
  const ground = src.ay;
  let g: Grid;
  let body: { x0: number; y0: number; x1: number; y1: number } | null = null;

  switch (spec.style) {
    case "hand":
    case "hand+pool": {
      if (!src.frames.dead) throw new Error(`corpse: "${spec.style}" needs a drawn dead frame`);
      g = toGrid(src.frames.dead);
      break;
    }
    case "fallen":
    case "wisp": {
      const roles = personRoles(toGrid(src.frames.down));
      const lying = LYING.map((r) => [...r].map((c) => (c === "." ? "." : roles[c])));
      g = blank(src.w, src.h);
      const at = paste(g, lying, src.ax, ground - 1);
      body = { x0: at.x, y0: at.y, x1: at.x + lying[0].length - 1, y1: at.y + lying.length - 1 };
      break;
    }
    case "topple": {
      const lying = shorten(rotateCCW(crop(toGrid(src.frames.side))), src.w);
      g = blank(src.w, src.h);
      const at = paste(g, lying, src.ax, ground);
      body = { x0: at.x, y0: at.y, x1: at.x + lying[0].length - 1, y1: at.y + lying.length - 1 };
      break;
    }
    case "flat": {
      // On its back on the ground, wings open and still: the "down" drawing upside down, no longer hovering.
      const spread = flipV(crop(toGrid(src.frames.down)));
      g = blank(src.w, src.h);
      const at = paste(g, spread, src.ax, src.h - 1);
      body = { x0: at.x, y0: at.y, x1: at.x + spread[0].length - 1, y1: at.y + spread.length - 1 };
      break;
    }
    case "drawn": {
      const rows = spec.frame;
      if (!rows || rows.length !== src.h || rows.some((r) => r.length !== src.w)) throw new Error(`corpse: a drawn body must be ${src.w} x ${src.h}`);
      g = toGrid(rows);
      break;
    }
    case "legsUp": {
      const up = flipV(crop(toGrid(src.frames.side)));
      g = blank(src.w, src.h);
      // Flyers hover (their anchor sits below the drawing); a dead one is on the ground, one row lower.
      const at = paste(g, up, src.ax, Math.min(src.h - 1, ground));
      body = { x0: at.x, y0: at.y, x1: at.x + up[0].length - 1, y1: at.y + up.length - 1 };
      break;
    }
    case "scrap": {
      g = scrapHeap(toGrid(src.frames.down), src.w, src.h, ground);
      break;
    }
    case "melt": {
      // Keep the top of the head and the face, lose the middle, spread the rest into a puddle.
      const stand = crop(toGrid(src.frames.down));
      const head = stand.slice(0, Math.min(6, stand.length));
      g = blank(src.w, src.h);
      const slump = head.concat(stand.slice(stand.length - 2));
      paste(g, slump, src.ax, ground - 1);
      break;
    }
  }

  const fade = spec.style === "wisp" ? 0.4 : 1;
  const b = body ?? (() => {
    const bb = bbox(g);
    return bb;
  })();
  if (spec.style !== "hand" && spec.style !== "hand+pool") Object.assign(palette, recolour(g, src.palette, taken, fade));

  if (spec.pool) {
    const [edge, core] = POOLS[spec.pool];
    const e = spare(taken);
    const c = spare(taken);
    palette[e] = edge;
    palette[c] = core;
    const bw = b.x1 - b.x0 + 1;
    // Under the chest: a third of the way from the head (west) end for the fallen, the middle for the rest.
    const cx = spec.style === "fallen" || spec.style === "wisp" ? b.x0 + bw * 0.38 : b.x0 + bw / 2;
    const rx = Math.max(2.5, Math.min(7, bw * 0.42));
    pool(g, cx, Math.min(src.h - 1, b.y1 + 0.5), rx, spec.style === "melt" ? 1.6 : 1.4, e, c);
  }

  if (spec.style === "wisp") {
    const m = spare(taken);
    palette[m] = "#cfd4e080";
    for (const [dx, dy] of [
      [2, -3],
      [6, -5],
      [10, -2],
    ]) {
      const x = b.x0 + dx;
      const y = b.y0 + dy;
      if (g[y]?.[x] === ".") g[y][x] = m;
    }
  }

  return { ...src, palette, frames: { ...src.frames, dead: g.map((r) => r.join("")) } };
}
