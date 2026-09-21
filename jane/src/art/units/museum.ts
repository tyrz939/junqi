// The Museum's exhibits, once they are down off their plinths.
//
// Every one of them was made to look like a person, so they share one body: a standing figure,
// bare-headed or capped, and the palette says what it is made of. Steel with nothing behind
// the visor, candle wax, the Company's khaki, council navy. The mounted fox is its own small
// thing. Light from the top-left, outline PAL.k, side faces east.
//
// (A fragment cannot palette-swap a sprite out of art/units.ts: that file imports its
// fragments before it assigns its own sheet, so the base is not there yet. The shared body
// below is this file's own.)

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

/** q face or visor, Q its shade, U body, u its highlight, G cap, y trim. */
type Body = Record<string, string[]>;

const BARE: Body = {
  down: [
    "................",
    ".....kkkkkk.....",
    "....kqqqqqqk....",
    "...kqqqqqqqqk...",
    "...kqqkqqkqqk...",
    "...kqqkqqkqqk...",
    "...kqqqqqqqqk...",
    "...kqqqQQqqqk...",
    "...kQqqqqqqQk...",
    "....kQQQQQQk....",
    "..kkkuUUUUukkk..",
    ".kquuUUUUUUuukq.",
    ".kquuUUUUUUuukq.",
    ".kqkuUUyyUUukkq.",
    ".kqk.UUUUUU.kkq.",
    "..k..UUUUUU..k..",
    ".....kUUkUk.....",
    ".....kUUkUk.....",
    ".....kkk.kk.....",
    "................",
  ],
  up: [
    "................",
    ".....kkkkkk.....",
    "....kqqqqqqk....",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kQQQQQQQQk...",
    "....kQQQQQQk....",
    "..kkkuUUUUukkk..",
    ".kquuUUUUUUuukq.",
    ".kquuUUUUUUuukq.",
    ".kqkuUUUUUUukkq.",
    ".kqk.UUUUUU.kkq.",
    "..k..UUUUUU..k..",
    ".....kUUkUk.....",
    ".....kUUkUk.....",
    ".....kkk.kk.....",
    "................",
  ],
  side: [
    "................",
    "......kkkk......",
    ".....kqqqqk.....",
    ".....kqqqqqk....",
    ".....kqqkqqk....",
    ".....kqqqqqk....",
    ".....kqqqqk.....",
    ".....kQqqqk.....",
    "......kQQk......",
    ".....kkUUkk.....",
    "....kuUUUUuk....",
    "....kuUUUUuk....",
    "...kquUUyUuqk...",
    "...kq.UUUU.qk...",
    "....k.UUUU.k....",
    "......UUUU......",
    "......kUUk......",
    "......kUUk......",
    "......kkkk......",
    "................",
  ],
};

/** The same body under a peaked cap: a miner's, an attendant's. */
const CAPPED: Body = {
  down: [
    "................",
    "....kkkkkkkk....",
    "...kGGGGGGGGk...",
    "...kGGGGGGGGk...",
    "...kqqkqqkqqk...",
    "...kqqkqqkqqk...",
    "...kqqqqqqqqk...",
    "...kqqqQQqqqk...",
    "...kQqqqqqqQk...",
    "....kQQQQQQk....",
    "..kkkuUUUUukkk..",
    ".kquuUUUUUUuukq.",
    ".kquuUUUUUUuukq.",
    ".kqkuUUyyUUukkq.",
    ".kqk.UUUUUU.kkq.",
    "..k..UUUUUU..k..",
    ".....kUUkUk.....",
    ".....kUUkUk.....",
    ".....kkk.kk.....",
    "................",
  ],
  up: [
    "................",
    "....kkkkkkkk....",
    "...kGGGGGGGGk...",
    "...kGGGGGGGGk...",
    "...kGGGGGGGGk...",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kqqqqqqqqk...",
    "...kQQQQQQQQk...",
    "....kQQQQQQk....",
    "..kkkuUUUUukkk..",
    ".kquuUUUUUUuukq.",
    ".kquuUUUUUUuukq.",
    ".kqkuUUUUUUukkq.",
    ".kqk.UUUUUU.kkq.",
    "..k..UUUUUU..k..",
    ".....kUUkUk.....",
    ".....kUUkUk.....",
    ".....kkk.kk.....",
    "................",
  ],
  side: [
    "................",
    "....kkkkkkk.....",
    "...kGGGGGGGk....",
    "....kGGGGGGGk...",
    ".....kqqkqqk....",
    ".....kqqqqqk....",
    ".....kqqqqk.....",
    ".....kQqqqk.....",
    "......kQQk......",
    ".....kkUUkk.....",
    "....kuUUUUuk....",
    "....kuUUUUuk....",
    "...kquUUyUuqk...",
    "...kq.UUUU.qk...",
    "....k.UUUU.k....",
    "......UUUU......",
    "......kUUk......",
    "......kUUk......",
    "......kkkk......",
    "................",
  ],
};

function figure(body: Body, extra: Palette): SpriteSrc {
  return { w: 16, h: 20, ax: 8, ay: 18, palette: { ...PAL, ...extra }, frames: body };
}

const sheet: SpriteSheet = {
  // An empty suit of armour: steel, brass at the belt, and nothing at all behind the visor.
  museum_armour: figure(BARE, { q: "#20242c", Q: "#14171d", U: "#6a707c", u: "#9aa2ae", y: "#c8a038" }),

  // A waxwork: the same figure a little warmer, and softening.
  museum_waxwork: figure(BARE, { q: "#e8d8b0", Q: "#b8a478", U: "#8a7a5c", u: "#b09a74", y: "#d8c088" }),

  // The Shot-Firer: the Company's khaki, presented holding a charge.
  museum_shot_firer: figure(CAPPED, { q: "#e8d8b0", Q: "#b8a478", U: "#8a7a4a", u: "#b8a478", G: "#5a4e2e", y: "#c8403c" }),

  // The Attendant: council navy, a peaked cap, and a great many keys at the belt.
  museum_attendant: figure(CAPPED, { q: "#d8c8a8", Q: "#a89070", U: "#2c3448", u: "#465270", G: "#1a2030", y: "#f0d048" }),

  // A mounted fox, with the glass eyes left in.
  museum_fox: {
    w: 14,
    h: 10,
    ax: 7,
    ay: 8,
    palette: { ...PAL, f: "#c87a3c", F: "#8a4a22" },
    frames: {
      down: [
        "..kk......kk..",
        "..kfk....kfk..",
        ".kffkkkkkkffk.",
        ".kffffffffffk.",
        "kfwffffffffwfk",
        "kffffffffffffk",
        ".kFFkFFkFFkFk.",
        ".kFk.kFk.kFk..",
        "..k...k...k...",
        "..............",
      ],
      up: [
        "..kk......kk..",
        "..kfk....kfk..",
        ".kffkkkkkkffk.",
        ".kffffffffffk.",
        "kffffffffffffk",
        "kfFFFFFFFFFFfk",
        ".kFFkFFkFFkFk.",
        ".kFk.kFk.kFk..",
        "..k...k...k...",
        "..............",
      ],
      side: [
        "...........kk.",
        "..........kffk",
        ".kkkkkkkkkkffk",
        "kfffffffffffwk",
        "kffffffffffffk",
        "kFffffffffffFk",
        ".kFkkFkkFkkFk.",
        ".kFk.kFk.kFk..",
        "..k...k...k...",
        "..............",
      ],
    },
  },
};

export default sheet;
