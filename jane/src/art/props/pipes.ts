// The pipes: brick, cast iron and standing water. Everything down here was made by the
// company to be worked on from below, which is why the manhole covers lift the way they do.
// Light from the top-left, outline PAL.k, the same hand as the other sheets.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // the dark of a shaft
  z: "#7e8a94", // cast iron
  Z: "#4c565e", // iron in shadow
  u: "#b0603a", // rust
  v: "#8c6a56", // brick
  V: "#5e4638", // brick in shadow
  a: "#2d5a6e", // water
  A: "#1d3c4c", // water in shadow
  d: "#9a8058", // galvanised rung
  D: "#6a5438", // rung in shadow
};

/** footH is the footprint height in 8 px cells: a tall prop rises upward out of its footprint. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // --- a manhole, seen from below: the shaft, and the cover lifted off it ------------------
  manhole: prop(16, 16, 2, {
    base: [
      "................",
      "....kkkkkkkk....",
      "..kkzzzzzzzzkk..",
      ".kzZZZZZZZZZZzk.",
      ".kzZkkkkkkkkZzk.",
      "kzZkxxxxxxxxkZzk",
      "kzZkxwxxxxwxkZzk",
      "kzZkxxxxxxxxkZzk",
      "kzZkxxxxxxxxkZzk",
      "kzZkxxxxxxxxkZzk",
      "kzZkkkkkkkkkkZzk",
      ".kzZZZZZZZZZZzk.",
      "..kzzzzzzzzzzk..",
      "...kkkkkkkkkk...",
      "................",
      "................",
    ],
  }),

  // --- the east valve: a hand wheel on a spindle -------------------------------------------
  valve: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzkkzk.",
      "kkzkkzkk",
      "kzzkkzzk",
      "kzkuukzk",
      "kzzkkzzk",
      "kkzkkzkk",
      ".kzkkzk.",
      ".kzzzzk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
    on: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzzkzk.",
      "kkkzzkkk",
      "kzkkzzkk",
      "kzzkuukz",
      "kkzzkkzk",
      "kkkzzkkk",
      ".kzkzzk.",
      ".kzzzzk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
  }),

  // --- a ladder with its rungs gone. `on` is the same ladder with the rungs back in ---------
  ladder_broken: prop(16, 24, 1, {
    base: [
      "................",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kdDk....kdDk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kddk....kddk...",
      ".kDDk....kDDk...",
      ".kkkk....kkkk...",
      "................",
      "................",
    ],
    on: [
      "................",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kdDk....kdDk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kddkddddkddk...",
      ".kddk....kddk...",
      ".kDDkddddkDDk...",
      ".kkkk....kkkk...",
      "................",
      "................",
    ],
  }),

  // --- penstocks: the plates that come down across a run ------------------------------------
  pipes_penstock_h: prop(24, 12, 1, {
    base: [
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzzzzzzzzzzzzzzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzuuzzzzzzzzuuzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzzzzzzzzzzzzzzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kZzzzzzzzzzzzzzzzzzzzzZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
    ],
  }),

  pipes_penstock_v: prop(8, 24, 3, {
    base: [
      "kkkkkkkk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kuukzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkuuZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kuukzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kkkkkkkk",
    ],
  }),

  // --- the gangers' brazier: banked, and a kettle beside it ---------------------------------
  pipes_stove: prop(16, 16, 2, {
    base: [
      "................",
      "................",
      "..kk........kk..",
      "..kzkkkkkkkkzk..",
      "..kzvvvvvvvvzk..",
      ".kkzoyyooyyozkk.",
      ".kzzyrryyrryzzk.",
      ".kzzoyyooyyozzk.",
      ".kkzvVVVVVVvzkk.",
      "..kzzzzzzzzzzk..",
      "..kZZzzzzzzZZk..",
      "..kkzkkkkkkzkk..",
      "...kzk....kzk...",
      "...kZk....kZk...",
      "...kkk....kkk...",
      "................",
    ],
  }),
};

export default sheet;
