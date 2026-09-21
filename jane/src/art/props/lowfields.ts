// Props the Lowfields side quests need (QUESTS.md Part 3). Same hand as art/props.ts:
// width is footprint w*8, tall props rise UP out of their footprint, light from the
// top-left, 1px dark outline.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#3a3e4a", // darkest stone / cold iron
  // Contact shadow, translucent so a thing sits on whatever ground it is standing on.
  "-": "#00000030",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // The parish board: two posts, a framed board, three papers, the middle one pinned over the others.
  notice_board: prop(24, 20, 1, {
    base: [
      ".kkkkkkkkkkkkkkkkkkkkkk.",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kmttttttttttttttttttttTk",
      "kmtwwwwtttrwwwwtttwwwtTk",
      "kmtwGGwttwwwwwwwttwGwtTk",
      "kmtwwwwttwGGGGwwttwwwtTk",
      "kmtwGGwttwwwwwwwttwGwtTk",
      "kmtwwWwttwGGGwwwttwwWtTk",
      "kmtWWWWttwwwwwwWttWWWtTk",
      "kmtttttttWWWWWWWttttttTk",
      "kmttttttttttttttttttttTk",
      "kTTTTTTTTTTTTTTTTTTTTTTk",
      ".kkkkkkkkkkkkkkkkkkkkkk.",
      "...ktTk..........ktTk...",
      "...ktTk..........ktTk...",
      "...ktTk..........ktTk...",
      "...ktTk..........ktTk...",
      "...ktTk..........ktTk...",
      "..kktTkk........kktTkk..",
      "..kkkkkk........kkkkkk..",
    ],
  }),

  // A coat on a cross of sticks, a sack for a head, straw out of the top of the sack and
  // out of both cuffs. At one cell wide it was a dark post with a head on it: the whole
  // shape of a scarecrow is the arms, so the row is now two cells wide (data/props/
  // lowfields.json) and the crossbar goes from edge to edge. One eye. Buttons down the
  // front, the top one done up. The post carries on down through the coat to the field.
  scarecrow: prop(16, 28, 1, {
    base: [
      "......y.y.......",
      ".....kkkkkk.....",
      ".....kmmttk.....",
      ".....kmkttk.....",
      ".....kmmttk.....",
      ".....kmtttk.....",
      ".....kttTTk.....",
      "......ktTk......",
      ".kkkkkkkkkkkkkk.",
      "kyGGGGGGGGGKKKyk",
      "kyGGGGGGGGGKKKyk",
      ".kkkkkGGGGkkkkk.",
      ".ky..kGGGGk..yk.",
      "....kGGGGGKk....",
      "....kGkGGGKk....",
      "....kGGGGGKk....",
      "....kGyGGGKk....",
      "....kGGGGGKk....",
      "....kGkGGKKk....",
      "....kGGGKKKk....",
      "....kkyGKkkk....",
      "......ktTk......",
      "......ktTk......",
      "......ktTk......",
      "......ktTk......",
      "......ktTk......",
      ".....kkeekk.....",
      "....--====--....",
    ],
  }),

  // The lampman's brazier before anyone has laid it: the campfire's ring of stones, swept out.
  campfire_cold: prop(16, 16, 2, {
    base: [
      "................",
      "................",
      "................",
      "................",
      "................",
      "................",
      "................",
      "................",
      "..kkkkkkkkkkkk..",
      ".kWgkdkKKKkdkWgk",
      "kWgGkddKKKKdkgGk",
      "kgGGkkkkkkkkkgGk",
      ".kkkWggkkWggkkk.",
      "...kgGGkkgGGk...",
      "....kkk..kkk....",
      "................",
    ],
  }),

  // The rock across the adit on Quarry Steps. Too big to shift and too neat to have fallen.
  adit_rock: prop(24, 20, 2, {
    base: [
      "........kkkkkkkk........",
      "......kkWWWWWgggkk......",
      "....kkWWWWWWgggggGkk....",
      "...kWWWWWggggggggGGGk...",
      "..kWWWWgggggggggGGGGGk..",
      ".kWWWgggggkgggggGGGGGGk.",
      ".kWWggggggkggggGGGGGGGk.",
      "kWWgggggggGkgggGGGGGGGGk",
      "kWggggggggGGkgGGGGGGGGGk",
      "kWgggggggGGGGkGGGGGGGGdk",
      "kggggggkgGGGGGGGGGGGGddk",
      "kgggggkgGGGGGGGGGGGGdddk",
      "kggggGGGGGGGGGGGGGGddddk",
      "kgggGGGGGGGGGGGGGGdddddk",
      "kgGGGGGGGGGGGGGGGddddddk",
      ".kGGGGGGGGGGGGGdddddddk.",
      ".kGGGGGGGGGGGdddddddddk.",
      "..kkGGGGGGdddddddddkkk..",
      "....kkkkddddddddkkkk....",
      "........kkkkkkkk........",
    ],
  }),
};

export default sheet;
