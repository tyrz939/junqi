// Item icons for the Lowfields side quests (QUESTS.md ASKS A). 16x16, 1px near-black
// outline, bright fills, light from the top-left, like every other icon.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const ICON_PAL: Palette = {
  ...PAL,
  a: "#f09080", // light red: the lit edge of the wool
};

function icon(rows: string[]): SpriteSrc {
  return { w: 16, h: 16, ax: 0, ay: 0, palette: ICON_PAL, frames: { base: rows } };
}

const sheet: SpriteSheet = {
  // A child's mitten, red, with a knitted cuff.
  item_glove: icon([
    "................",
    "................",
    "......kkkk......",
    ".....krrrrk.....",
    "....karrrrrk....",
    "....karrrrRk....",
    "....karrrrRk.kk.",
    "....karrrrRkkrRk",
    "....karrrrrrrrRk",
    "....krrrrrrrrRk.",
    "....krrrrrrRRk..",
    ".....krrrrRRk...",
    ".....kkkkkkkk...",
    ".....kWWwWWgk...",
    ".....kWgWgWgk...",
    ".....kkkkkkkk...",
  ]),

  // A man's grey trilby, from the side.
  item_hat: icon([
    "................",
    "................",
    "................",
    ".....kkkkkk.....",
    "....kggggggk....",
    "...kgWgggggGk...",
    "...kgWggggGGk...",
    "...kgggggGGGk...",
    "...kggggGGGGk...",
    "...kKKKKKKKKk...",
    ".kkkKKKKKKKKkkk.",
    "kgggggggggggGGGk",
    "kWggggggggGGGGk.",
    ".kkGGGGGGGGkkk..",
    "...kkkkkkkk.....",
    "................",
  ]),

  // A carter's dinner tin: oval, lidded, with a wire handle.
  item_tin: icon([
    "................",
    "................",
    ".....kkkkkk.....",
    "....k......k....",
    "...k........k...",
    "...k........k...",
    "..kkkkkkkkkkkk..",
    ".kwwWWWWWWWWggk.",
    "kwWWWWWWWWWWgggk",
    "kkkkkkkkkkkkkkkk",
    "kWWggggggggggGGk",
    "kWgggggggggggGGk",
    "kWgggggggggggGGk",
    ".kggggggggggGGk.",
    "..kkkkkkkkkkkk..",
    "................",
  ]),

  // A walker's canvas haversack with its one strap.
  item_haversack: icon([
    "................",
    "....kkkkkk......",
    "...kTk...kTk....",
    "..kTk.....kTk...",
    "..kTk......kTk..",
    "..kkkkkkkkkkkk..",
    ".kmmmmmmmmmmttk.",
    ".kmttttttttttTk.",
    ".kmtttkyyktttTk.",
    ".kkkkkkyykkkkkk.",
    ".kmmmmkkkkmmttk.",
    ".kmmmmmmmmmtttk.",
    ".kmmmmmmmmmtttk.",
    ".kmtmmmmmmttTTk.",
    ".kkttttttttTTkk.",
    "..kkkkkkkkkkkk..",
  ]),
};

export default sheet;
