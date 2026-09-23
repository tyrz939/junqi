// Things the words promise and the world now has: the stones in Mrs Bettany's garden (one of them
// has her key under it), the loose front step at the Hacketts' (the tin), the gang's five dinner
// tins, the candle ends and the empty chair, the pencil marks on the door frame, the chalk X, the
// ladder that stands up by itself. Same hand as art/props.ts: width is footprint w*8, tall things
// rise UP out of their footprint, light from the top-left, a 1 px dark outline.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  c: "#a39d90", // stone
  C: "#6e695f", // stone shade
  M: "#6f8a4a", // moss
  a: "#e9dfc4", // tallow
  A: "#b9ab86", // tallow shade
  x: "#0c0a12",
  "-": "#00000030",
  "=": "#00000050",
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // A garden stone the size of a loaf, sat low in the earth. Three, so four in a bed are four stones.
  garden_stone: prop(8, 8, 1, {
    base: [
      "........",
      "........",
      "..kkkk..",
      ".kWccCk.",
      "kWcccCCk",
      "kcccCCCk",
      ".kkkkkk.",
      ".======.",
    ],
    base2: [
      "........",
      "........",
      "........",
      ".kkkkk..",
      "kWcMcCk.",
      "kccCCCCk",
      ".kkkkkk.",
      "..=====.",
    ],
    base3: [
      "........",
      "...kkk..",
      "..kWcCk.",
      ".kWccCCk",
      ".kcMcCCk",
      ".kcCCCk.",
      "..kkkk..",
      "..====..",
    ],
  }),

  // The front step: one flag of stone, worn hollow in the middle, standing a little proud of the ground.
  step_stone: prop(8, 8, 1, {
    base: [
      "........",
      "kkkkkkkk",
      "kWcccccC",
      "kcCcccCC",
      "kccCCccC",
      "kCCCCCCC",
      "kkkkkkkk",
      "========",
    ],
  }),

  // A dinner tin with its lid on, set down in a row with the others.
  dinner_tin: prop(8, 8, 1, {
    base: [
      "........",
      "........",
      ".kkkkkk.",
      "kwWWWWgk",
      "kgggggGk",
      "kWggggGk",
      ".kkkkkk.",
      ".------.",
    ],
  }),

  // Candle ends burnt down in saucers, three to a cell. No flame: it is by day that she looks.
  candle_ends: prop(8, 8, 1, {
    base: [
      "........",
      "..k.....",
      ".kak..k.",
      ".kAk.kak",
      "kWWWkkAk",
      ".kkkkWWW",
      ".k.a..kk",
      "..kWk...",
    ],
  }),

  // The door frame's last upright, with the heights pencilled up it.
  pencil_marks: prop(8, 20, 1, {
    base: [
      "..kkkk..",
      "..ktTk..",
      "..ktTk..",
      "..kkTk..",
      "..ktTk..",
      "..kKkk..",
      "..ktTk..",
      "..ktTk..",
      "..kKkk..",
      "..ktTk..",
      "..ktTk..",
      "..kkTk..",
      "..ktTk..",
      "..ktTk..",
      "..kKkk..",
      "..ktTk..",
      "..ktTk..",
      ".kktTkk.",
      ".kmtTTk.",
      ".======.",
    ],
  }),

  // An X in chalk on a flat stone, gone over more than once.
  chalk_mark: prop(8, 8, 1, {
    base: [
      "........",
      ".kkkkkk.",
      "kcwcccwC",
      "kccwcwCC",
      "kcccwcCC",
      "kccwcwCC",
      "kCwCCCwC",
      ".kkkkkk.",
    ],
  }),

  // A ladder stood up on its own feet, tarred black, with SAYER in white on the second rung.
  leaning_ladder: prop(8, 28, 1, {
    base: [
      ".k....k.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".KwwwwK.",
      ".K....K.",
      ".K....K.",
      ".KkkkkK.",
      ".K....K.",
      ".K....K.",
      ".k....k.",
      ".=....=.",
    ],
  }),

  // A parcel in brown paper and string, the nurse's wrapping.
  parcel: prop(8, 8, 1, {
    base: [
      "........",
      ".kkkkkk.",
      "kmmtmmmk",
      "kmmtmmTk",
      "kttttttk",
      "kmmtmTTk",
      ".kkkkkk.",
      ".------.",
    ],
  }),

  // Three pints on the step, one for each morning. The oldest has not turned.
  milk_bottles: prop(8, 8, 1, {
    base: [
      ".k..k.k.",
      "kykkykyk",
      "kwkkwkwk",
      "kwWkwWwW",
      "kwWkwWwW",
      "kwWkwWkW",
      "kkkkkkkk",
      "========",
    ],
    // One empty bottle, rinsed and put out for the Milkman, once the milk is in and he is paid.
    on: [
      "........",
      "...kk...",
      "...kk...",
      "..kiik..",
      "..kiik..",
      "..kiBk..",
      "..kkkk..",
      "..====..",
    ],
  }),

  // The unclaimed shelf: two boards on brackets, with what is on them. A lamp stone, two bottles tied
  // at the necks, a paper bag of grapes, and a card. Nothing locked about it.
  unclaimed_shelf: prop(16, 24, 2, {
    base: [
      "................",
      "kkkkkkkkkkkkkkkk",
      "kTttttttttttttTk",
      "kT............Tk",
      "kT.kk....kk...Tk",
      "kT.kik..kwWk..Tk",
      "kTkyiyk.kwWk..Tk",
      "kTkiiik.kwWk..Tk",
      "kkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmk",
      "kTttttttttttttTk",
      "kT............Tk",
      "kT.kkkk..kkkk.Tk",
      "kTkmmmmk.kwwwkTk",
      "kTkmpmpk.kwGwkTk",
      "kTkmmmmk.kkkkkTk",
      "kkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmk",
      "kTttttttttttttTk",
      "kT............Tk",
      "kT............Tk",
      "kT............Tk",
      "kkk..........kkk",
      "===..........===",
    ],
  }),

  // A kitchen chair, facing the doorway, with nothing on it. The seat is clean.
  kitchen_chair: prop(8, 14, 1, {
    base: [
      "..kkkk..",
      ".kttttk.",
      ".ktkktk.",
      ".ktkktk.",
      ".ktkktk.",
      "kkkkkkkk",
      "kWttttTk",
      "kttttTTk",
      "kkkkkkkk",
      ".kTk.kTk",
      ".kTk.kTk",
      ".kTk.kTk",
      ".kk...kk",
      ".=.....=",
    ],
  }),
};

export default sheet;
