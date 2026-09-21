// Props the Lowfields side quests need (QUESTS.md Part 3). Same hand as art/props.ts:
// width is footprint w*8, tall props rise UP out of their footprint, light from the
// top-left, 1px dark outline.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#3a3e4a", // darkest stone / cold iron
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

  // A coat on a cross of sticks, a sack for a head. The head sits a little to one side of the
  // pole, the way a head does when it is listening. One eye. The top button is done up.
  scarecrow: prop(8, 28, 1, {
    base: [
      "...kkkk.",
      "..kmmmtk",
      "..kmkmtk",
      "..kmmttk",
      "..kmtttk",
      "..ktttTk",
      "...kTTk.",
      "kkkkkkkk",
      "kTGGGKTk",
      "kkGGGKkk",
      ".kGkGKk.",
      ".kGGGKk.",
      ".kGkGKk.",
      ".kGGGKk.",
      ".kGkGKk.",
      ".kGGKKk.",
      ".kGGKKk.",
      ".kGKGKk.",
      ".kkGkKk.",
      "..kkkk..",
      "..ktTk..",
      "..ktTk..",
      "..ktTk..",
      "..ktTk..",
      "..ktTk..",
      "..ktTk..",
      ".kkeekk.",
      ".kkkkkk.",
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
