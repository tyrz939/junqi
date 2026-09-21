// Butterfly Forest's three inventory icons: the net he was carrying, one of the eight, and
// what the eight become when the last case has a label on it.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  f: "#f4a0c0", // wing
  F: "#c06890", // wing shade
  q: "#fff0bc", // gauze
};

function icon(frames: Record<string, string[]>): SpriteSrc {
  return { w: 16, h: 16, ax: 8, ay: 14, palette: PX, frames };
}

const sheet: SpriteSheet = {
  item_net: icon({
    base: [
      "................",
      "....kkkkkk......",
      "..kkqqqqqqkk....",
      ".kqqqqqqqqqqk...",
      ".kqqwqqwqqwqk...",
      ".kqqqqqqqqqqk...",
      ".kqwqqwqqwqqk...",
      "..kqqqqqqqqk....",
      "...kqqqqqqk.....",
      "....kkqqkk......",
      ".....kkkk.......",
      "......ktk.......",
      ".......ktk......",
      "........ktk.....",
      ".........kTk....",
      "..........kk....",
    ],
  }),

  item_butterfly: icon({
    base: [
      "................",
      "................",
      "..kk........kk..",
      ".kffk......kffk.",
      "kfFffk....kffFfk",
      "kffffkkkkkfffffk",
      "kfFfkkyykkfFffk.",
      "kfffkkyykkffffk.",
      ".kffkkyykkfffk..",
      "..kkkkkkkkkkk...",
      "....kkyykk......",
      "......kk........",
      ".....k..k.......",
      "....k....k......",
      "................",
      "................",
    ],
  }),

  item_amulet: icon({
    base: [
      "................",
      "......kkkk......",
      ".....kYYYYk.....",
      "....kYkkkkYk....",
      "...kYkiiiikYk...",
      "..kYkifffikYk...",
      "..kYkiffFikYk...",
      "..kYkikkkikYk...",
      "..kYkifffikYk...",
      "..kYkifFfikYk...",
      "...kYkiiiikYk...",
      "....kYkkkkYk....",
      ".....kYYYYk.....",
      "......kkkk......",
      "................",
      "................",
    ],
  }),
};

export default sheet;
