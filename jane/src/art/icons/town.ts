// Item icons for Castle's errands. 16x16, 1 px outline, light from the top-left.

import type { SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

function icon(rows: string[]): SpriteSrc {
  return { w: 16, h: 16, ax: 0, ay: 0, palette: PAL, frames: { base: rows } };
}

const sheet: SpriteSheet = {
  // A shirt off somebody's line, folded once, with a wooden peg still on the collar.
  item_washing: icon([
    "................",
    "......kkkk......",
    "....kkkttkkk....",
    "...kbbkTTkbbk...",
    "..kbbbbkkbbbBk..",
    ".kbbbbbbbbbbBBk.",
    ".kbbkbbbbbbkBBk.",
    ".kkkkbbbbbbkkkk.",
    "....kbbbbbBk....",
    "....kbbbbbBk....",
    "....kbiibbBk....",
    "....kbbbbbBk....",
    "....kbbbbbBk....",
    "....kBBBBBBk....",
    "....kkkkkkkk....",
    "................",
  ]),
};

export default sheet;
