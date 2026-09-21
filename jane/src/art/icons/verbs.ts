// Spell icons for the three verbs the later dungeons teach (DUNGEONS.md 4.4): Explosion,
// Grow and Spark. 16x16, 1px near-black outline, bright fills, light from the top-left,
// like every other icon. Inks beyond the shared palette are the ones icons.ts already uses.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const ICON_PAL: Palette = {
  ...PAL,
  f: "#fff4a8", // pale yellow / glow
  q: "#d8a8f0", // light purple
};

function icon(rows: string[]): SpriteSrc {
  return { w: 16, h: 16, ax: 0, ay: 0, palette: ICON_PAL, frames: { base: rows } };
}

const sheet: SpriteSheet = {
  // A shot-firer's charge: a round black powder charge, a fuse, and the fuse is lit.
  spell_blast: icon([
    "............f...",
    "..........kfyf..",
    ".........kyfoy..",
    ".........kt.f...",
    "........ktk.....",
    ".....kkktkk.....",
    "...kkGGtGGGkk...",
    "..kGggGGGGGGKk..",
    ".kGgwgGGGGGGKKk.",
    ".kGggGGGGGGKKKk.",
    ".kGGGGGGGGGKKKk.",
    ".kGGGGGGGGKKKKk.",
    "..kGGGGGKKKKKk..",
    "...kkKKKKKKkk...",
    ".....kkkkkk.....",
    "................",
  ]),

  // A shoot coming up out of turned earth, two leaves, the light on the left one.
  spell_grow: icon([
    "................",
    "......kk........",
    ".....klnk..kkk..",
    "....kllnk.klnnk.",
    "...kllnnkklnnNk.",
    "...klnnNkknnNk..",
    "....knNkknNNk...",
    ".....kkknkkk....",
    ".......knk......",
    ".......knk......",
    "......kknkk.....",
    "...kkkeenetkkk..",
    "..keetteeeteeek.",
    ".keeeeeteeeeeTk.",
    ".kkkkkkkkkkkkkk.",
    "................",
  ]),

  // A spark between two contacts: one jagged line, white at the core.
  spell_shock: icon([
    "................",
    ".kkk............",
    "kgWgk...kk......",
    "kgggk..kqk......",
    ".kkkkkkqfk......",
    "....kqqfkk......",
    "...kqffkkkkk....",
    "...kffwwwfqk....",
    "....kkkkwfqk....",
    "......kwfqk.....",
    ".....kwfqkkkkk..",
    ".....kfqk.kgggk.",
    ".....kqk..kgWgk.",
    "......k....kkk..",
    "................",
    "................",
  ]),
};

export default sheet;
