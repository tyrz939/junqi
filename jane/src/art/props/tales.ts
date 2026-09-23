// The tales' things (data/props/tales.json, QUEST-TREE.md "Tales"): what stands at the places of the few
// people in the county who are more than neighbours, and shows their story before they speak. Same hand as
// art/props.ts and art/props/country.ts: width is footprint w*8, tall things rise UP out of their footprint,
// light from the top-left, a 1 px dark outline. A thing is drawn as what it is: the key is a key, the
// stone is a stone, the boots have water in them, because the words say so.
//
// Frames: "base"; "on" where a thing changes (crepe tied round a hive, a dinner set on the Dole Stone,
// Horace in his box, a lamp lit, a hatch opened, boards pulled off a door); "open" once a locker is emptied.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";
import country from "@/art/props/country";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // a hole, a slot, the inside of something
  d: "#3a3e4a", // cold iron
  c: "#a39d90", // stone
  C: "#6e695f",
  u: "#e2d6bc", // plaster
  U: "#b3a488",
  a: "#c09a52", // straw
  A: "#86683a",
  h: "#dcc27e",
  O: "#bdb39a", // canvas
  I: "#857b66",
  Z: "#26222c", // black crepe
  q: "#8a5a3a", // carpet slipper, leather
  Q: "#5e3a24",
  v: "#ffd878", // lit
  V: "#d08a3a",
  j: "#4f7a5a", // painted locker green
  J: "#34543e",
  "-": "#00000030",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  for (const [name, rows] of Object.entries(frames)) {
    if (rows.length !== h || rows.some((r) => r.length !== w)) throw new Error(`tales art: frame ${name} is not ${w}x${h}`);
  }
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

/** A copy of `rows` with `patch` laid over it at (x, y); "." in the patch leaves what was there. */
function over(rows: readonly string[], x: number, y: number, patch: readonly string[]): string[] {
  const out = rows.map((r) => [...r]);
  patch.forEach((line, j) => {
    [...line].forEach((ch, i) => {
      if (ch !== "." && out[y + j] && out[y + j][x + i] !== undefined) out[y + j][x + i] = ch;
    });
  });
  return out.map((r) => r.join(""));
}

// --- Coram's --------------------------------------------------------------------------------------

// A white box hive on legs under a tin roof, bees at the slot. "on": black crepe round it, knotted at the back.
const HIVE = [
  "..kkkk..",
  ".kggggk.",
  "kgggggGk",
  "kkkkkkkk",
  ".kwwwWk.",
  ".kwwwWky",
  ".kWWWWk.",
  ".kwwwWk.",
  "ykwwwWk.",
  ".kkxxkk.",
  ".kT..Tk.",
  ".-....-.",
];
const HIVE_CREPE = over(HIVE, 0, 5, [".kZZZZkZ", ".kZZZZZZ", ".kwwwWkZ"]);

// A wide hat and a darned net veil hung on a stake. By night the nail at the top of the stake is bare.
const VEIL = [
  "........",
  "..kkkk..",
  ".kOOOOk.",
  "kkOOOOkk",
  "kOOOOOIk",
  "kkkkkkkk",
  ".kgWgWk.",
  ".kWgWgk.",
  ".kgWgWk.",
  "..kWgk..",
  "...kTk..",
  "...kTk..",
  "...kTk..",
  "...kTk..",
  "..kTTTk.",
  "..-----.",
];
const VEIL_BARE = over(
  VEIL.map((r, y) => (y < 9 ? "........" : r)),
  0,
  8,
  ["...kk...", "...kgk.."],
);

const JAR = [
  "........",
  "..kkkk..",
  "..kwwk..",
  ".kkkkkk.",
  ".koyyok.",
  ".kyyyYk.",
  ".kyYYYk.",
  "..kkkk..",
];

// --- Whinmoor Cottage and the Dole Stone ----------------------------------------------------------

// A kitchen chair with a red blanket folded on the seat, on ground worn to earth.
const CHAIR = [
  ".kk..kk.",
  ".kt..tk.",
  ".kkkkkk.",
  ".ktttTk.",
  ".kkkkkk.",
  ".kt..tk.",
  "kkkkkkkk",
  "krRrRrRk",
  "kRrRrRrk",
  "kkkkkkkk",
  "ekT..Tke",
  ".ekeeke.",
];

// A man's carpet slippers, worn through at the toes.
const SLIPPERS = [
  "........",
  ".kk..kk.",
  "kqqkkqqk",
  "kqQkkqQk",
  "kqQkkqQk",
  "kxQkkxQk",
  ".kk..kk.",
  "........",
];

// A flat stone on two others, waist high. "on": a dinner plate under a cloth, a blue rim showing.
const DOLE = [
  "................",
  "................",
  "................",
  ".kkkkkkkkkkkkkk.",
  "kcccccccccccccck",
  "kccCccccccCccCck",
  "kCCCCCCCCCCCCCCk",
  "kkkkkkkkkkkkkkkk",
  ".kcCk......kcCk.",
  ".kcCk......kcCk.",
  ".kCCk......kCCk.",
  ".kkkk......kkkk.",
];
const DOLE_DINNER = over(DOLE, 3, 1, ["..kkkkk...", ".kwwwwWk..", "kBwwwwWWBk", "kkBBBBBBkk"]);

// The second plate, on a stone of its own.
const DINNER = [
  "........",
  "..kkkk..",
  ".kwwwWk.",
  "kBwwWWBk",
  "kkBBBBkk",
  ".kcccck.",
  "kccccCCk",
  "kcCCCCCk",
  "kkkkkkkk",
  "........",
];

// --- Sundial Cottage ------------------------------------------------------------------------------

// A tea chest on its side, straw inside, the front knocked out. "on": Horace, nose to the front.
const HUTCH = [
  "................",
  "................",
  "................",
  ".kkkkkkkkkkkkkk.",
  ".kgmmmmmmmmmmgk.",
  ".kmmtmmmmmtmmmk.",
  ".kgmmmmmmmmmmgk.",
  ".kkkkkkkkkkkkkk.",
  ".kgkxxxxxxxxkgk.",
  ".kmkxahxxhaxkmk.",
  ".kmkahaahahakmk.",
  ".kmkhaahaahakmk.",
  ".kmkaahahaahkmk.",
  ".kmkhahaahahkmk.",
  ".kgkkkkkkkkkkgk.",
  ".kkkkkkkkkkkkkk.",
  "..============..",
  "................",
];
const HUTCH_HORACE = over(HUTCH, 4, 9, ["..kkkk..", ".kmTmTk.", "kkTmTmkk", ".kmTmTk.", "..kssk.."]);

// Bare damp earth where a stone lay, worm casts in it.
const HOLLOW = [
  "........",
  "..eeee..",
  ".eTeeTe.",
  ".eeeeee.",
  ".eTeeee.",
  ".eeeeTe.",
  "..eeee..",
  "........",
];

// A flat grey stone: pushed, not lifted.
const STONE = [
  "........",
  "..kkkkk.",
  ".kccccck",
  "kcwcccCk",
  "kcccccCk",
  "kCcccCCk",
  ".kCCCCk.",
  "..kkkk..",
];

// A dandelion rosette with the flower bitten off, the stalk short.
const DANDELION = [
  "........",
  "...kk...",
  "..kWk.k.",
  ".klkNklk",
  "kllnNllk",
  ".klNnlk.",
  "kllkkllk",
  ".kk..kk.",
];

// Horace, from above, head to the north.
const TORTOISE = [
  "...kk...",
  "..kssk..",
  "skkkkkks",
  ".kmTmTk.",
  ".kTmTmk.",
  ".kmTmTk.",
  "skkkkkks",
  "...kk...",
];

// --- Coker's Wall -----------------------------------------------------------------------------------

// A flat stone dressed square. Face down, it is only stone.
const SLAB = [
  "........",
  "kkkkkkkk",
  "kWWWWWck",
  "kWccccCk",
  "kWccccCk",
  "kcCCCCCk",
  "kkkkkkkk",
  "........",
];

// A knee-high dry-stone wall, three yards of it.
const DRYWALL = [
  "........................",
  "........................",
  ".kkkkkkkkkkkkkkkkkkkkkk.",
  "kcCkcckCcckcCkccCkcckcCk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  "kcckCcckccCkcckcCcckcCck",
  "kCckccCkcCckcckCckccCkck",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  "kccCkcckCcckccCkcckCccck",
  "kCcckccCkcckCcckcCckccCk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  ".======================.",
];

// The end of the wall: a scraped footing where the next stones go.
const GAP = [
  "........",
  "........",
  ".eeeeee.",
  "eCeeceee",
  "eeeeeeCe",
  ".eeeeee.",
  "........",
  "........",
];

const MUG = [
  "........",
  "........",
  ".kkkkk..",
  ".kTTTkk.",
  ".kgggkgk",
  ".kgggkgk",
  ".kGGGkk.",
  "..kkk...",
];

// --- Sluice Cottage ---------------------------------------------------------------------------------

// The roofless cottage of a ruin, boarded: planks across both windows and three across the door, nailed.
// "on": the boards off, and the door open a hand's width.
const EMPTY = country.cottage_empty.frames.base;
const PLANK = (n: number): string => `k${"m".repeat(n)}k`;
const NAILED = (n: number): string => `k${"TgTTTT".repeat(4).slice(0, n)}k`;
const BOARDED = over(EMPTY, 15, 40, [PLANK(10), NAILED(10), "", PLANK(10), NAILED(10)].map((r) => (r === "" ? "." : r)));
const BOARDED_2 = over(BOARDED, 35, 40, [PLANK(10), NAILED(10), ".", PLANK(10), NAILED(10)]);
const BOARDED_3 = over(BOARDED_2, 25, 46, [PLANK(10), NAILED(10), ".", PLANK(10), NAILED(10), ".", PLANK(10), NAILED(10)]);
const UNBOARDED = over(EMPTY, 28, 46, ["xx", "xx", "xx", "xx", "xx", "xx", "xx", "xx", "xx", "xx"]);

// Boots on the step, full to the laces with black water.
const BOOTS = [
  "........",
  "kkk.kkk.",
  "kxk.kxk.",
  "kek.kek.",
  "kek.kek.",
  "keekkeek",
  "kkkkkkkk",
  "........",
];

// A window at the back, boarded over from outside, the nails worked half out.
const BACKBOARDS = [
  "................",
  ".kkkkkkkkkkkkkk.",
  ".kUuuuuuuuuuuUk.",
  ".kUkkkkkkkkkkUk.",
  ".kUkmmmmmmmmkUk.",
  ".kUkTTgTTTgTkUk.",
  ".kUkmmmmmmmmkUk.",
  ".kUkTgTTTTgTkUk.",
  ".kUkmmmmmmmmkUk.",
  ".kUkTTgTTgTTkUk.",
  ".kUkkkkkkkkkkUk.",
  ".kUuuuuuuuuuuUk.",
  ".kCCCCCCCCCCCCk.",
  ".kkkkkkkkkkkkkk.",
];

// A stack of planks, and a tin of nails beside it.
const PLANKS = [
  "................",
  "................",
  ".kkkkkkkkkk.....",
  ".kmmmmmmmmk.kkk.",
  ".kkkkkkkkkk.kgk.",
  ".ktTttTttTk.kdk.",
  ".kkkkkkkkkk.kgk.",
  ".kmmmmmmmmk.kkk.",
  ".kkkkkkkkkk.....",
  "..========......",
];

// --- The Intake -------------------------------------------------------------------------------------

// A small iron key, on the ground.
const KEY = [
  "........",
  "........",
  ".kkk....",
  "kgGgkkkk",
  "kG.Ggggk",
  "kgGgkkgk",
  ".kkk..kk",
  "........",
];

// A diver's air pump in a wooden case: two wheels, two handles, brass unions, a coil of hose.
const PUMP = [
  "..kk......kk....",
  ".kddk....kddk...",
  "kdkkdk..kdkkdk..",
  "kdkkdk..kdkkdk..",
  ".kddk....kddk...",
  "kkkkkkkkkkkkkkk.",
  "kmmmmmmmmmmmmmk.",
  "ktTtttTttttTtmk.",
  "ktyktttttkyttmkk",
  "ktttTttttTtttmkG",
  "ktTtttTttttTtmkG",
  "kkkkkkkkkkkkkkkk",
  ".kk.........kk..",
  ".=============..",
];

// His helmet, faceplate open, a little lake water in the bottom.
const HELMET = [
  "..kkkk..",
  ".kyyyYk.",
  "kyykkYYk",
  "kykxxkYk",
  "kykbbkYk",
  "kyykkYYk",
  "kYyyyYYk",
  "kkYYYYkk",
  ".kkkkkk.",
  ".------.",
];

// Rubber hose along the ground.
const HOSE = [
  "................",
  "................",
  "................",
  "kk....kkk.......",
  "GGkkkkGGGkkkkkkk",
  "kkGGGGkkkGGGGGGG",
  "..kkkk...kkkkkkk",
  "................",
];

// --- Tenter Cottage ---------------------------------------------------------------------------------

// The newest rows, over the arm of the chair and down to the ground: needles still in, a darker name row.
const SCARF_END = [
  ".g....g.",
  "..g..g..",
  ".krrrrk.",
  ".kRRRRk.",
  ".krrrrk.",
  ".kRPPRk.",
  ".krrrrk.",
  ".kkkkkk.",
];

// A run of the scarf along the ground, rows of knitting and the darker name rows.
const SCARF_RUN = [
  "................",
  "................",
  "................",
  "kkkkkkkkkkkkkkkk",
  "rrRrrPrrRrrrPrrR",
  "rRrrRPrRrrRrPrRr",
  "kkkkkkkkkkkkkkkk",
  "................",
];

// A tangle of pulled wool, still crimped.
const TANGLE = [
  "........",
  "..r.r...",
  ".r.r.rr.",
  "r.rRr..r",
  ".rR.Rr.r",
  "r.r.r.r.",
  ".r.rr.r.",
  "........",
];

// --- Brickfield Halt -------------------------------------------------------------------------------

// A length of the Company's light railway: two rails on sleepers, rust on the rails.
const RAILS = [
  "........................",
  "gGgGgGgGgGgGgGgGgGgGgGgG",
  ".TT...TT...TT...TT...TT.",
  ".tT...tT...tT...tT...tT.",
  ".TT...TT...TT...TT...TT.",
  "gGgGgGgGgGgGgGgGgGgGgGgG",
  ".TT...TT...TT...TT...TT.",
  "........................",
];

// The timetable, on two posts: a printed sheet in a frame.
const TIMETABLE = [
  ".kkkkkkkkkkkkkk.",
  ".kTTTTTTTTTTTTk.",
  ".kTwwwwwwwwwwTk.",
  ".kTwGGGGGGGwwTk.",
  ".kTwwwwwwwwwwTk.",
  ".kTwGGGwGGGGwTk.",
  ".kTwGGGGwGGGwTk.",
  ".kTwGGwGGGGwwTk.",
  ".kTwwwwwwwwwwTk.",
  ".kTTTTTTTTTTTTk.",
  ".kkkkkkkkkkkkkk.",
  "..kTk......kTk..",
  "..kTk......kTk..",
  "..kTk......kTk..",
  "..kTk......kTk..",
  "..===......===..",
];

/** A signal lamp on a post: `glass` dark, and lit in the "on" frame. */
function signal(dark: string, hot: string, lit: string): Record<string, string[]> {
  const post = (lens: string, lens2: string): string[] => [
    "..kkkk..",
    ".kddddk.",
    ".kdkkdk.",
    `.kk${lens}${lens}kk.`,
    `.kk${lens}${lens2}kk.`,
    ".kddddk.",
    "..kddk..",
    ...Array.from({ length: 15 }, () => "...kdk.."),
    "..kddk..",
    "..====..",
  ];
  return { base: post(dark, dark), on: post(hot, lit) };
}

// The lamp locker: tall, green paint, a hasp. "open": the doors back and the shelf empty.
const LOCKER = [
  "................",
  "................",
  "................",
  "................",
  ".kkkkkkkkkkkkkk.",
  ".kJJJJJJJJJJJJk.",
  ".kjjjjjkjjjjjjk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjdkdjjjjJk.",
  ".kjjjjdkdjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kjjjjjkjjjjjJk.",
  ".kJJJJJkJJJJJJk.",
  ".kkkkkkkkkkkkkk.",
  ".kTk........kTk.",
  ".kkk........kkk.",
  "..============..",
];
const LOCKER_OPEN = over(LOCKER, 3, 6, [
  "xxxxxxxxxx",
  "xxxxxxxxxx",
  "kkkkkkkkkk",
  "xxxxxxxxxx",
  "xxxxxxxxxx",
  "kkkkkkkkkk",
  "xxxxxxxxxx",
  "xxxxxxxxxx",
  "kkkkkkkkkk",
  "xxxxxxxxxx",
  "xxxxxxxxxx",
]);

// --- The Weighbridge --------------------------------------------------------------------------------

// A hatch in the floor, bolted on the outside. "on": open, and nothing but dark under it.
const HATCH = [
  "................",
  ".kkkkkkkkkkkkkk.",
  ".kmmmmmmmmmmmmk.",
  ".ktTttTttTttTtk.",
  ".kkkkkkkkkkkkkk.",
  ".kmmmmmmmmmmmmk.",
  ".ktTttTkkkkTttk.",
  ".kkkkkkdddgkkkk.",
  ".kmmmmmkkkkmmmk.",
  ".ktTttTttTttTtk.",
  ".kkkkkkkkkkkkkk.",
  ".kmmmmmmmmmmmmk.",
  ".ktTttTttTttTtk.",
  ".kkkkkkkkkkkkkk.",
  "................",
  "................",
];
const HATCH_OPEN = [
  "................",
  ".kkkkkkkkkkkkkk.",
  ".kCxxxxxxxxxxCk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kxxxxxxxxxxxxk.",
  ".kCxxxxxxxxxxCk.",
  ".kkkkkkkkkkkkkk.",
  "..kmmmmmmmmmk...",
  "..kkkkkkkkkkk...",
  "................",
];

// Cage traps, empty, the doors tied open.
const CAGE = [
  "........",
  "........",
  "kkkkkkkk",
  "kg.g.g.k",
  "k.g.g.gk",
  "kg.g.g.k",
  "k.g.g.gk",
  "kkkkkkkk",
  ".r....r.",
  "........",
];

// Mr Sorrell's pole, leant on the wall, with nothing tied to it.
const POLE = [
  "......k.",
  ".....kt.",
  ".....kt.",
  "....kt..",
  "....kt..",
  "....kt..",
  "...kt...",
  "...kt...",
  "...kt...",
  "..kt....",
  "..kt....",
  "..kt....",
  "..kt....",
  ".kt.....",
  ".kt.....",
  ".kt.....",
  ".kt.....",
  "kt......",
  "kt......",
  "kt......",
  "kt......",
  "=.......",
];

const sheet: SpriteSheet = {
  tale_hive: prop(8, 12, 1, { base: HIVE, on: HIVE_CREPE }),
  tale_veil: prop(8, 16, 1, { base: VEIL, on: VEIL_BARE }),
  tale_jar: prop(8, 8, 1, { base: JAR }),
  tale_chair: prop(8, 12, 1, { base: CHAIR }),
  tale_slippers: prop(8, 8, 1, { base: SLIPPERS, on: SLIPPERS.map(() => "........") }),
  tale_dole_stone: prop(16, 12, 1, { base: DOLE, on: DOLE_DINNER }),
  tale_dinner: prop(8, 10, 1, { base: DINNER }),
  tale_hutch: prop(16, 18, 2, { base: HUTCH, on: HUTCH_HORACE }),
  tale_hollow: prop(8, 8, 1, { base: HOLLOW, on: HOLLOW }),
  tale_stone: prop(8, 8, 1, { base: STONE }),
  tale_dandelion: prop(8, 8, 1, { base: DANDELION }),
  tale_tortoise: prop(8, 8, 1, { base: TORTOISE }),
  tale_slab: prop(8, 8, 1, { base: SLAB }),
  tale_drywall: prop(24, 12, 1, { base: DRYWALL }),
  tale_gap: prop(8, 8, 1, { base: GAP, on: GAP }),
  tale_mug: prop(8, 8, 1, { base: MUG }),
  tale_cottage_boarded: prop(64, 58, 6, { base: BOARDED_3, on: UNBOARDED }),
  tale_boots: prop(8, 8, 1, { base: BOOTS }),
  tale_backboards: prop(16, 14, 1, { base: BACKBOARDS }),
  tale_planks: prop(16, 10, 1, { base: PLANKS }),
  tale_key: prop(8, 8, 1, { base: KEY }),
  tale_air_pump: prop(16, 14, 1, { base: PUMP }),
  tale_helmet: prop(8, 10, 1, { base: HELMET }),
  tale_hose: prop(16, 8, 1, { base: HOSE }),
  tale_scarf_end: prop(8, 8, 1, { base: SCARF_END }),
  tale_scarf_run: prop(16, 8, 1, { base: SCARF_RUN }),
  tale_tangle: prop(8, 8, 1, { base: TANGLE }),
  tale_rails: prop(24, 8, 1, { base: RAILS }),
  tale_timetable: prop(16, 16, 1, { base: TIMETABLE }),
  tale_signal_red: prop(8, 24, 1, signal("R", "r", "o")),
  tale_signal_green: prop(8, 24, 1, signal("N", "l", "y")),
  tale_locker: prop(16, 22, 2, { base: LOCKER, open: LOCKER_OPEN }),
  tale_hatch: prop(16, 16, 2, { base: HATCH, on: HATCH_OPEN }),
  tale_cage: prop(8, 10, 1, { base: CAGE }),
  tale_pole: prop(8, 22, 1, { base: POLE }),
};

export default sheet;
