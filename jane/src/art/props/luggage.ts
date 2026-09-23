// The left-luggage trunk at Castle Halt's Lost Property desk: "One trunk, tin-lined, heavy.
// Consignor: Goldskin Mining Co." It was drawn with the treasure chest (gold and blue, a dungeon's
// prize) and read as one. A railway trunk is a dark painted box with two wooden bands over the lid
// and down the front, brass caps on the corners, a brass hasp, and the consignor's name stencilled
// across the front, gone dull. Opened, the lid and the walls are tin.
// Same hand as art/props.ts: 2 x 2 cells, the lid seen from above, the front below the seam.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#34423a", // painted canvas, railway green gone nearly black
  D: "#222c27", // its shade
  v: "#4c5e52", // where the light catches it
  u: "#9c9478", // the stencil, faded
  j: "#b8bec6", // tin
  J: "#7d848e", // tin shade
  x: "#0c0a12",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  luggage_trunk: prop(16, 18, 2, {
    base: [
      "..kkkkkkkkkkkk..",
      ".kYvvtTvvvvtTvYk",
      "kYyvtTddddtTdyYk",
      "kvddtTddddtTdddk",
      "kdddtTddddtTdddk",
      "kYydtTDDDDtTdyYk",
      "kYYDTTDDDDTTDYYk",
      "kkkkkkkkkkkkkkkk",
      "kYydtTdYYdtTdyYk",
      "kvddtTdYxdtTdddk",
      "kdddtTdYYdtTdddk",
      "kdddtTddddtTdddk",
      "kdudtTuudutTudDk",
      "kdddtTddddtTddDk",
      "kYydtTDDDDtTdyYk",
      "kYYDTTDDDDTTDYYk",
      "kkkkkkkkkkkkkkkk",
      ".==============.",
    ],
    open: [
      "..kkkkkkkkkkkk..",
      ".kYjjjjjjjjjjjYk",
      "kYjWjjjjjjjjjJYk",
      "kdjjjjjjjjjjjJdk",
      "kdjjjjjjjjjjjJdk",
      "kdJJJJJJJJJJJJdk",
      "kYYDTTDDDDTTDYYk",
      "kkkkkkkkkkkkkkkk",
      "kjxxxxxxxxxxxxjk",
      "kjxxxxxxxxxxxxjk",
      "kjJJJJJJJJJJJJjk",
      "kkkkkkkkkkkkkkkk",
      "kvddtTdYYdtTdddk",
      "kdudtTuudutTudDk",
      "kYydtTDDDDtTdyYk",
      "kYYDTTDDDDTTDYYk",
      "kkkkkkkkkkkkkkkk",
      ".==============.",
    ],
  }),
};

export default sheet;
