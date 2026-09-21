// Goldskin Works: painted steel, brick and the things that answer a spark. Everything with a
// window in it is dark until it has current, so the same grid serves both frames with the
// window character swapped. Light from the top-left, outline PAL.k.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // a dead window
  c: "#bfe4ff", // current
  C: "#5c90b8", // current in shadow
  z: "#7e8a94", // painted steel
  Z: "#4c565e", // steel in shadow
  u: "#b0603a", // rust, brass, a blown fuse
  v: "#8c7c6c", // works brick
  V: "#5e5246", // brick in shadow
};

/** footH is the footprint height in 8 px cells: a tall prop rises upward out of its footprint. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

/** The generator, three cells square. `win` is what shows through its inspection window. */
const MACHINE = [
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  "kzzzzzzzzzzzzzzzzzzzzzzk",
  "kzZZZZZZZZZZZZZZZZZZZZzk",
  "kzZkkkkkkkkkkkkkkkkkkZzk",
  "kzZkzzzzzzzzzzzzzzzzkZzk",
  "kzZkzcczzcczzcczzcczkZzk",
  "kzZkzzzzzzzzzzzzzzzzkZzk",
  "kzZkkkkkkkkkkkkkkkkkkZzk",
  "kzZZZZZZZZZZZZZZZZZZZZzk",
  "kzzzzzzzzzzzzzzzzzzzzzzk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  "kzzzzzzzzzzzzzzzzzzzzzzk",
  "kzzkkkkkkzzzzkkkkkkzzzzk",
  "kzzkuuuukzzzzkuuuukzzzzk",
  "kzzkuuuukzzzzkuuuukzzzzk",
  "kzzkkkkkkzzzzkkkkkkzzzzk",
  "kzzzzzzzzzzzzzzzzzzzzzzk",
  "kZZZZZZZZZZZZZZZZZZZZZZk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
  "kZzzzzzzzzzzzzzzzzzzzzZk",
  "kZzuuzzzzzzzzzzzzzzuuzZk",
  "kZzzzzzzzzzzzzzzzzzzzzZk",
  "kZZZZZZZZZZZZZZZZZZZZZZk",
  "kkkkkkkkkkkkkkkkkkkkkkkk",
];

const machine = (win: string, bolt: string): string[] => MACHINE.map((r) => r.split("c").join(win).split("u").join(bolt));

const sheet: SpriteSheet = {
  // --- the generator, seized: `on` is the same machine with the bearing freed ---------------
  broken_generator: prop(24, 24, 3, {
    base: machine("x", "u"),
    on: machine("x", "z"),
  }),

  // --- the starting board: dark until it is sparked, and then it is the building -------------
  generator_cold: prop(24, 24, 3, {
    base: machine("x", "z"),
    on: machine("c", "z"),
  }),

  // --- a works lamp on its bracket ----------------------------------------------------------
  works_lamp: prop(8, 16, 1, {
    base: [
      "........",
      "...kk...",
      "...kk...",
      "..kkkk..",
      ".kzzzzk.",
      ".kZZZZk.",
      "..kGGk..",
      "...kk...",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
    on: [
      "........",
      "...kk...",
      "...kk...",
      "..kkkk..",
      ".kzzzzk.",
      ".kZyyZk.",
      "..kyyk..",
      "...ky...",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  }),

  // --- a dead socket: nothing at all until she puts something in it -------------------------
  socket_dead: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzxxzk.",
      ".kzxxzk.",
      ".kzzzzk.",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
    on: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzcczk.",
      ".kzcczk.",
      ".kzzzzk.",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  }),

  // --- a fuse box. `on` is blown, and it stays blown ----------------------------------------
  fuse_box: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzuuzk.",
      ".kzuuzk.",
      ".kzzzzk.",
      ".kzwwzk.",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
    on: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzxxzk.",
      ".kzxxzk.",
      ".kzzzzk.",
      ".kzWWzk.",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  }),

  // --- a call box on a post -----------------------------------------------------------------
  call_box: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzrrzk.",
      ".kzzzzk.",
      ".kzwwzk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
    on: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzcczk.",
      ".kzzzzk.",
      ".kzwwzk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  }),

  // --- a relay box at the head of a county lamp run ------------------------------------------
  relay_box: prop(8, 16, 1, {
    base: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzxxzk.",
      ".kzzzzk.",
      ".kZuuZk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
    on: [
      "........",
      "..kkkk..",
      ".kzzzzk.",
      ".kzcczk.",
      ".kzzzzk.",
      ".kZyyZk.",
      "..kkkk..",
      "...kk...",
      "...kk...",
      "..kkkk..",
      "........",
      "........",
      "........",
      "........",
      "........",
      "........",
    ],
  }),

  // --- a floor grid with a socket in it ------------------------------------------------------
  grid_socket: prop(8, 8, 1, {
    base: [
      "kkkkkkkk",
      "kzkzkzzk",
      "kkkkkkkk",
      "kzkzkzzk",
      "kkkkkkkk",
      "kzkzkzzk",
      "kkkkkkkk",
      "kZZZZZZk",
    ],
    on: [
      "kkkkkkkk",
      "kckckcck",
      "kkkkkkkk",
      "kckckcck",
      "kkkkkkkk",
      "kckckcck",
      "kkkkkkkk",
      "kCCCCCCk",
    ],
  }),

  // --- the orb on its kitchen stand ----------------------------------------------------------
  orb_spark: prop(16, 16, 2, {
    base: [
      "................",
      ".....kkkkk......",
      "....kcCCCck.....",
      "...kcCwwwCck....",
      "...kcCwwwCck....",
      "...kcCCCCCck....",
      "....kcCCCck.....",
      ".....kkkkk......",
      "......kkk.......",
      "......ktk.......",
      "......ktk.......",
      ".....kkTkk......",
      "....kTTTTTk.....",
      "...kTTTTTTTk....",
      "...kkkkkkkkk....",
      "................",
    ],
    on: [
      "................",
      ".....kkkkk......",
      "....kzZZZzk.....",
      "...kzZWWWZzk....",
      "...kzZWWWZzk....",
      "...kzZZZZZzk....",
      "....kzZZZzk.....",
      ".....kkkkk......",
      "......kkk.......",
      "......ktk.......",
      "......ktk.......",
      ".....kkTkk......",
      "....kTTTTTk.....",
      "...kTTTTTTTk....",
      "...kkkkkkkkk....",
      "................",
    ],
  }),

  // --- a patched wall: two courses of new brick where a door was ------------------------------
  weak_wall: prop(24, 16, 2, {
    base: [
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kvvvvkvvvvkvvvvkvvvvkvvk",
      "kvvvvkvvvvkvvvvkvvvvkvvk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kvvkvvvvkuuuukvvvvkvvvvk",
      "kvvkvvvvkuuuukvvvvkvvvvk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kvvvvkvvvvkuuuukvvvvkvvk",
      "kvvvvkvvvvkuuuukvvvvkvvk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kvvkvvvvkuuuukvvvvkvvvvk",
      "kvvkvvvvkuuuukvvvvkvvvvk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kVVVVkVVVVkVVVVkVVVVkVVk",
      "kVVVVkVVVVkVVVVkVVVVkVVk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
    ],
  }),

  // --- roller shutters: the gates a powered circuit drives -------------------------------------
  factory_shutter_h: prop(24, 12, 1, {
    base: [
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzzzzzzzzzzzzzzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzzzzzzzzzzzzzzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kzzzzuuzzzzzzzzuuzzzzzzk",
      "kZZZZZZZZZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kZzzzzzzzzzzzzzzzzzzzzZk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
    ],
  }),

  factory_shutter_v: prop(8, 24, 3, {
    base: [
      "kkkkkkkk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kuukzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkuuZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kuukzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kzzkzzZk",
      "kkkkkkkk",
    ],
  }),

  // --- the time office stove ------------------------------------------------------------------
  factory_stove: prop(16, 16, 2, {
    base: [
      "................",
      ".....kkkkkk.....",
      ".....kzzzzk.....",
      "kkkkkkkkkkkkkkkk",
      "kzzzzzzzzzzzzzzk",
      "kzkkkkkkkkkkkkzk",
      "kzkoyyoooooookzk",
      "kzkyrryyoooookzk",
      "kzkoyyoooooookzk",
      "kzkkkkkkkkkkkkzk",
      "kzzzzzzzzzzzzzzk",
      "kZZZZZZZZZZZZZZk",
      "kkkkkkkkkkkkkkkk",
      ".kZk........kZk.",
      ".kZk........kZk.",
      ".kkk........kkk.",
    ],
  }),

  // --- the card rack, sixty numbered slots and every card stamped in ---------------------------
  factory_rack: prop(24, 16, 1, {
    base: [
      "........................",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kTttttttttttttttttttttTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTttttttttttttttttttttTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTkwkwkwkwkwkwkwkwkwkkTk",
      "kTttttttttttttttttttttTk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "........................",
      "........................",
      "........................",
      "........................",
    ],
  }),
};

export default sheet;
