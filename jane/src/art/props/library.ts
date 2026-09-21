// The ruined library's three pieces of furniture: the book with one page left in it, the
// dry planter under the hole in the roof, and the light that comes down through the hole.
// The planter has an `on` frame, which is the same planter with something growing out of it.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // between the boards
  q: "#d8cba4", // sunlight on dust
  Q: "#efe6c4", // the bright of it
  v: "#6a4a30", // dry soil
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // A book open on a stand. The block of pages is thin, because they have been cut out.
  page_grow: prop(16, 16, 2, {
    base: [
      "................",
      "................",
      "......kkkk......",
      ".....kWWWWk.....",
      "kkkkkkWWWWkkkkkk",
      "kTWWWWWyyWWWWWTk",
      "kTWWWWWyyWWWWWTk",
      "kTWWWWkkkkWWWWTk",
      "kTWWWWWWWWWWWWTk",
      "kkTTkkkkkkkkTTkk",
      "..kTk......kTk..",
      "..kTk......kTk..",
      "..kTk......kTk..",
      "..kTk......kTk..",
      "..kTk......kTk..",
      "..kkk......kkk..",
    ],
  }),

  // A long wooden planter. Dry: cracked soil, one dead stick. `on`: it has come up.
  library_planter: prop(16, 16, 2, {
    base: [
      "................",
      "................",
      "................",
      "........k.......",
      ".......kTk......",
      "......kT.k......",
      "kkkkkkkTkkkkkkkk",
      "kTvvvvvvvvvvvvTk",
      "kTvxvvvxvvvxvvTk",
      "kTvvvvvvvvvvvvTk",
      "kmmmmmmmmmmmmmmk",
      "kTmmmmmmmmmmmmTk",
      "kTmmmmmmmmmmmmTk",
      "kTmmmmmmmmmmmmTk",
      "kTTTTTTTTTTTTTTk",
      "kkkkkkkkkkkkkkkk",
    ],
    on: [
      "................",
      "...k..kkk..k....",
      "..klk.klk.klk...",
      "...knkklkkknk...",
      "....knklkknk....",
      "......knkk......",
      "kkkkkkknkkkkkkkk",
      "kTvvvvvnvvvvvvTk",
      "kTvvlvvnvvlvvvTk",
      "kTvvvvvvvvvvvvTk",
      "kmmmmmmmmmmmmmmk",
      "kTmmmmmmmmmmmmTk",
      "kTmmmmmmmmmmmmTk",
      "kTmmmmmmmmmmmmTk",
      "kTTTTTTTTTTTTTTk",
      "kkkkkkkkkkkkkkkk",
    ],
  }),

  // Daylight on the floorboards, with the shadow of the broken rafters across it.
  library_beam: prop(16, 16, 2, {
    base: [
      "....qqqqqqqq....",
      "..qqQQQQQQQQqq..",
      ".qQQQQQQQQQQQQq.",
      "qQQQQQQQQQQQQQQq",
      "qQQQQQQQQQQQQQQq",
      "qQQQkkQQQQQQQQQq",
      "qQQQQkkQQQQQQQQq",
      "qQQQQQkkQQQQQQQq",
      "qQQQQQQkkQQQQQQq",
      "qQQQQQQQkkQQQQQq",
      "qQQQQQQQQkkQQQQq",
      "qQQQQQQQQQkkQQQq",
      ".qQQQQQQQQQQQQq.",
      "..qqQQQQQQQQqq..",
      "....qqqqqqqq....",
      "................",
    ],
  }),
};

export default sheet;
