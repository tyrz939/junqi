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
  // A child's mitten, red, with a knitted cuff. The old one was a rectangle with a
  // right-angled thumb; this one is a rounded mitt with the thumb set low, a lit edge
  // down the left and a ribbed cuff you can read as knitting.
  item_glove: icon([
    "................",
    "......kkkk......",
    ".....karrk......",
    "....karrrrk.....",
    "...karrrrrRk....",
    "...karrrrrRk....",
    "...karrrrrRk....",
    "...karrrrrRkkk..",
    "...karrrrrrrarRk",
    "...karrrrrrrrrRk",
    "...karrrrrrrrRk.",
    "...krrrrrrrRRk..",
    "...kkkkkkkkkkk..",
    "...kWwWwWwWWgk..",
    "...kwWwWwWwWgk..",
    "...kkkkkkkkkkk..",
  ]),

  // A man's grey trilby, from the side. It was a flat grey dome with a black stripe; it
  // now turns from light to shade across the crown and the band is felt-brown, not black.
  // A crown crease was tried and cut: six pixels of crown is not enough to put one in.
  item_hat: icon([
    "................",
    "................",
    ".....kkkkkk.....",
    "....kwgggggk....",
    "...kwgggggGGk...",
    "...kwgggggGGk...",
    "...kwggggGGGk...",
    "...kwgggGGGGk...",
    "...kTTTTTTTTk...",
    ".kkkTTTTTTTTkkk.",
    "kwggggggggggggGk",
    "kwgggggggggGGGGk",
    ".kGGGGGGGGGGGGk.",
    "..kkkkkkkkkkkk..",
    "................",
    "................",
  ]),

  // A carter's dinner tin: oval, lidded, with a wire handle that is fixed to the tin
  // rather than hovering above it, and ends that turn away from the light.
  item_tin: icon([
    "................",
    "................",
    ".....kkkkkk.....",
    "....k......k....",
    "...k........k...",
    "..kk........kk..",
    ".kkkkkkkkkkkkkk.",
    "kwwWWWWWWWWWggGk",
    "kwWWWWWWWWWWgggk",
    "kkkkkkkkkkkkkkkk",
    "kwWggggggggggGGk",
    "kwgggggggggggGGk",
    "kwgggggggggggGGk",
    "kWgggggggggggGGk",
    ".kGGGGGGGGGGGGk.",
    "..kkkkkkkkkkkk..",
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
