// Castle School's own furniture. An ordinary institution: a rope by the stair, clocks in
// wooden cases, pegs with the coats still on them, two beds in the sick bay, a fuse board and
// a bell. Nothing here is a person and nothing here is a child, which is the point.
//
// Light from the top-left, outline PAL.k, same hand as the other sheets. Things that answer a
// verb get an `on` frame, which is what they look like afterwards; the clock's `open` frame is
// the clock stopped (the renderer draws `open` once a `once` prop has been used).

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#6a707c", // painted steel: fuse board, bell frame
  D: "#3f4550", // steel in shadow
  u: "#c08a3a", // bell bronze
  U: "#7a5426", // bronze shade
  v: "#d8cfc0", // whitewash, enamel, bed linen
  V: "#a89c88", // whitewash in shadow
  q: "#3a5a7c", // school serge: the coats
  Q: "#24384e", // serge in shadow
  f: "#ffe07a", // a lit filament
  z: "#0c0a12", // the inside of a case
};

/** footH is the footprint height in 8 px cells: a tall prop rises upward out of its footprint. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  // The rope in the hall. It goes up out of the frame, which is where the bell is.
  bell_rope: prop(8, 16, 1, {
    base: [
      "...WW...",
      "...WW...",
      "...WW...",
      "...WW...",
      "...WW...",
      "...WW...",
      "...WW...",
      "...WW...",
      "..kWWk..",
      "..kwwk..",
      "..kwwk..",
      "..kWWk..",
      "..kWWk..",
      "...kk...",
      "...kk...",
      "........",
    ],
    on: [
      "..WW....",
      "..WW....",
      "..WW....",
      "...WW...",
      "...WW...",
      "...WW...",
      "....WW..",
      "....WW..",
      "...kWWk.",
      "...kwwk.",
      "...kwwk.",
      "...kWWk.",
      "...kWWk.",
      "....kk..",
      "....kk..",
      "........",
    ],
  }),

  // A school clock in a wooden case. `open` is the same clock with the hands stopped.
  clock_stopped: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kTTTTk.",
      ".kTwwTk.",
      "kTwWWwTk",
      "kTwWkWTk",
      "kTwWkWTk",
      "kTwWkkWk",
      "kTwWWWTk",
      "kTwwwwTk",
      ".kTTTTk.",
      "..kTTk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
    open: [
      "........",
      "..kkkk..",
      ".kTTTTk.",
      ".kTvvTk.",
      "kTvVVvTk",
      "kTvVkVTk",
      "kTvkkVTk",
      "kTvVVVTk",
      "kTvVVVTk",
      "kTvvvvTk",
      ".kTTTTk.",
      "..kTTk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
  }),

  // The far bed in the sick bay. A card is pinned over the head of it.
  sickbay_bed_bell: prop(16, 24, 3, {
    base: [
      "................",
      "................",
      "....kkkkkk......",
      "....kwwwwk......",
      "....kwkwwk......",
      "....kkkkkk......",
      "................",
      "..kkkkkkkkkkkk..",
      "..kWWWWWWWWWWk..",
      "..kWvvvvvvvvWk..",
      "..kWvvvvvvvvWk..",
      "..kvvvvvvvvvvk..",
      "..kvvvvvvvvvvk..",
      "..kVvvvvvvvvVk..",
      "..kVVvvvvvvVVk..",
      "..kVVVvvvvVVVk..",
      "..kVVVVVVVVVVk..",
      "..kTTTTTTTTTTk..",
      "..kTkkkkkkkkTk..",
      "..kTk......kTk..",
      "..kkk......kkk..",
      "..kTk......kTk..",
      "..kkk......kkk..",
      "................",
    ],
  }),

  // A board of dead sockets. `on` is the board with the lamps drawing off it.
  school_fuse: prop(8, 16, 1, {
    base: [
      "........",
      "........",
      "..kkkk..",
      ".kdddDk.",
      ".kdzzDk.",
      ".kdzzDk.",
      ".kdddDk.",
      ".kdzzDk.",
      ".kdzzDk.",
      ".kdddDk.",
      ".kDDDDk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
    on: [
      "........",
      "........",
      "..kkkk..",
      ".kdddDk.",
      ".kdffDk.",
      ".kdffDk.",
      ".kdddDk.",
      ".kdffDk.",
      ".kdffDk.",
      ".kdddDk.",
      ".kDDDDk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
    ],
  }),

  // The register, open on the desk in the top room.
  school_register: prop(16, 16, 1, {
    base: [
      "................",
      "................",
      "................",
      "................",
      "................",
      "..kkkkkkkkkkkk..",
      ".kwwwwwwkwwwwwk.",
      "kwWkkkWwkwWkkWwk",
      "kwkkkkkWkWkkkkwk",
      "kwWkkkWwkwWkkWwk",
      "kwkkkkkWkWkkkkwk",
      "kwWWWWWwkwWWWWwk",
      ".kWWWWWWkWWWWWk.",
      "..kkkkkkkkkkkk..",
      "................",
      "................",
    ],
  }),

  // The bell. It is a bell, in a frame, and it is not ringing.
  school_bell: prop(16, 24, 2, {
    base: [
      "................",
      "......kkkk......",
      ".....kdDDdk.....",
      "....kd....dk....",
      "....kd....dk....",
      "......kuuk......",
      ".....kuuuuk.....",
      "....kuuuuuuk....",
      "....kuuuuuUk....",
      "...kuuuuuuUUk...",
      "...kuuuuuuUUk...",
      "..kuuuuuuuUUUk..",
      "..kuuuuuuuUUUk..",
      "..kuuuuuuuUUUk..",
      ".kuuuuuuuuUUUUk.",
      ".kUUUUUUUUUUUUk.",
      "..kkkkkkkkkkkk..",
      "......kUUk......",
      "......kUUk......",
      "....kkkkkkkk....",
      "...kDDDDDDDDk...",
      "...kDDDDDDDDk...",
      "....kkkkkkkk....",
      "................",
    ],
  }),

  // Sixty pegs, numbered to sixty. The coats are on their own numbers and they are buttoned.
  school_pegs: prop(16, 16, 1, {
    base: [
      "................",
      "kkkkkkkkkkkkkkkk",
      "kTTTTTTTTTTTTTTk",
      "kkkkkkkkkkkkkkkk",
      ".k.k.k.k.k.k.k..",
      "kqkkqkkqkkqkkqk.",
      "kqqkqqkqqkqqkqq.",
      "kqqkqqkqqkqqkqq.",
      "kqQkqQkqQkqQkqQ.",
      "kqQkqQkqQkqQkqQ.",
      "kQQkQQkQQkQQkQQ.",
      "kQQkQQkQQkQQkQQ.",
      ".kk.kk.kk.kk.kk.",
      "................",
      "................",
      "................",
    ],
  }),
};

export default sheet;
