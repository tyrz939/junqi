// Item icons for things the stories hand her, so each one looks like what it is, in the bag and
// lying on the ground (a `showsLoot` prop draws its item's icon at ground scale: render/renderer.ts).
// Before these, spectacles were a glass, a bill-hook and garden scissors were an iron ingot, an egg
// was a stone and a fleece was a line of washing. 16x16, 1px near-black outline, light from the
// top-left, like every other icon.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const ICON_PAL: Palette = {
  ...PAL,
  a: "#f3ead6", // eggshell, lit
  A: "#d6c7a4", // eggshell, shade
  h: "#c9ad7a", // sacking
  H: "#8e7550", // sacking shade
  f: "#e8e2d2", // fleece
  F: "#b8b0a0", // fleece shade
  c: "#3a3e4a", // coat cloth
  C: "#262833", // coat shade
  u: "#d49a5a", // crust
  U: "#9a6232", // crust shade
  v: "#8a5a34", // tan leather
  V: "#5c3a22", // leather shade
};

function icon(rows: string[]): SpriteSrc {
  return { w: 16, h: 16, ax: 0, ay: 0, palette: ICON_PAL, frames: { base: rows } };
}

const sheet: SpriteSheet = {
  // Wire spectacles: two round lenses and a bridge, the arms folded behind.
  item_spectacles: icon([
    "................",
    "................",
    "................",
    "................",
    "..kkkk....kkkk..",
    ".kibbbk..kibbbk.",
    "kibwbbbkkibwbbbk",
    "kibbbbBk.kbbbbBk",
    "kbbbbBBk.kbbbBBk",
    ".kbBBBk...kbBBk.",
    "..kkkk.....kkkk.",
    "..G..........G..",
    "...GGGGGGGGGG...",
    "................",
    "................",
    "................",
  ]),

  // A bill-hook: a hooked blade on a short wooden handle.
  item_billhook: icon([
    "................",
    "....kkkk........",
    "...kwWggk.......",
    "..kwgk.kgk......",
    "..kgk...kgk.....",
    "...k....kgk.....",
    ".......kwgk.....",
    "......kwggk.....",
    ".....kwggGk.....",
    ".....kgggk......",
    "......kkTk......",
    "......ktTk......",
    ".....ktTk.......",
    ".....ktTk.......",
    "....ktTk........",
    "....kkk.........",
  ]),

  // A brown hen's egg.
  item_egg: icon([
    "................",
    "................",
    "......kkkk......",
    ".....kaaaAk.....",
    "....kawaaaAk....",
    "....kwaaaaAk....",
    "...kaaaaaaAAk...",
    "...kaaaaaaAAk...",
    "...kaaaaaAAAk...",
    "...kaaaaAAAAk...",
    "....kaaAAAAk....",
    "....kAAAAAAk....",
    ".....kkkkkk.....",
    "................",
    "................",
    "................",
  ]),

  // A sack of meal, tied at the neck.
  item_sack: icon([
    "................",
    "......k..k......",
    ".....khkkhk.....",
    "......kHHk......",
    ".....kkTTkk.....",
    "....khhhhhHk....",
    "...khhhhhhhHk...",
    "..khhhhhhhhHHk..",
    "..khhhHhhhhHHk..",
    "..khhhhhhhHHHk..",
    "..khhhhhhhHHHk..",
    "..khhhhhhHHHHk..",
    "..kHhhhhHHHHHk..",
    "...kHHHHHHHHk...",
    "....kkkkkkkk....",
    "................",
  ]),

  // A tuft of fleece, with a dab of the owner's red mark.
  item_fleece: icon([
    "................",
    "................",
    "................",
    ".....kkk.kk.....",
    "....kfffkffk....",
    "..kkffffffffkk..",
    ".kfffffrrfffffk.",
    ".kffffrrrffffFk.",
    "kfffffffffffFFk.",
    ".kffffffffFFFFk.",
    ".kFfffffFFFFFk..",
    "..kkFFFFFFkkk...",
    "....kkkkkk......",
    "................",
    "................",
    "................",
  ]),

  // A gold wedding ring.
  item_ring: icon([
    "................",
    "................",
    "................",
    ".....kkkkkk.....",
    "....kyyyyyYk....",
    "...kywkkkkyYk...",
    "..kyyk....kyYk..",
    "..kyk......kYk..",
    "..kyk......kYk..",
    "..kyYk....kYYk..",
    "...kyYkkkkYYk...",
    "....kYYYYYYk....",
    ".....kkkkkk.....",
    "................",
    "................",
    "................",
  ]),

  // Garden scissors, open, with wooden handles.
  item_scissors: icon([
    "................",
    "..k.........k...",
    ".kwk.......kwk..",
    "..kgk.....kgk...",
    "...kgk...kgk....",
    "....kgk.kgk.....",
    ".....kgkgk......",
    "......kGk.......",
    ".....kgkGk......",
    "....ktk.kTk.....",
    "...ktTk.kTTk....",
    "..ktTk...kTTk...",
    "..kTk.....kTk...",
    "..kkk.....kkk...",
    "................",
    "................",
  ]),

  // Three potatoes with the earth still on them.
  item_potatoes: icon([
    "................",
    "................",
    "................",
    "................",
    ".......kkkk.....",
    "......kuuuUk....",
    "..kkk.kuUuUUk...",
    ".kuuukkuuUUUk...",
    "kuuuUUkkUUUk....",
    "kuUuUUkkkkkkk...",
    "kUUUUkuuuUUUUk..",
    ".kkkkkuuUuUUUk..",
    "....kUUUUUUUk...",
    ".....kkkkkkk....",
    "....eeeeeeeeee..",
    "................",
  ]),

  // A man's good black coat, on its hanger.
  item_coat: icon([
    "................",
    ".......kk.......",
    "......k..k......",
    "....kkkkkkkk....",
    "...kccckkcccCk..",
    "..kcccckkccccCk.",
    ".kcccckwwkccccCk",
    ".kccck.kk.kcccCk",
    ".kcCck.gg.kcCCCk",
    ".kcCck.kk.kcCCk.",
    "..kCck.gg.kcCk..",
    "...kck.kk.kcCk..",
    "...kck.gg.kcCk..",
    "...kcCkkkkcCCk..",
    "...kkkkkkkkkkk..",
    "................",
  ]),

  // A boy's flat cap.
  item_cap: icon([
    "................",
    "................",
    "................",
    "................",
    "......kkkkk.....",
    "....kkggggGkk...",
    "...kgwgggggGGk..",
    "..kggggggggGGGk.",
    "..kgggggggGGGGk.",
    ".kkkkkkkkkkkkkk.",
    "kGGGGGGGk.......",
    ".kkkkkkk........",
    "................",
    "................",
    "................",
    "................",
  ]),

  // A cottage loaf.
  item_loaf: icon([
    "................",
    "................",
    "................",
    "......kkkk......",
    ".....kuuuUk.....",
    ".....kuwuUk.....",
    "....kkkUUkkk....",
    "...kuuuuuuuUk...",
    "..kuwuuuuuuUUk..",
    "..kuuuuuuuUUUk..",
    "..kuuuuuuUUUUk..",
    "..kUuuuUUUUUUk..",
    "...kUUUUUUUUk...",
    "....kkkkkkkk....",
    "................",
    "................",
  ]),

  // A man's leather glove, brown and worn at the fingers.
  item_glove_brown: icon([
    "................",
    "......k.k.......",
    ".....kvkvkk.....",
    "....kkvkvkvk....",
    "....kvkvkvkvk...",
    "....kvkvkvkvk...",
    "....kvvvvvvVk...",
    "....kvvvvvvVkk..",
    "....kvvvvvvvvVk.",
    "....kvvvvvvvVk..",
    "....kvvvvvvVVk..",
    "....kVvvvvVVk...",
    "....kkkkkkkkk...",
    "....kTTTTTTVk...",
    "....kkkkkkkkk...",
    "................",
  ]),

  // Teaspoons, none of them hers.
  item_spoons: icon([
    "................",
    "................",
    "..kk......kk....",
    ".kwgk....kwgk...",
    ".kggk....kggk...",
    "..kk......kkk...",
    "..kgk......kgk..",
    "...kgk......kgk.",
    "....kgk......kk.",
    ".....kgk.kk.....",
    "......kkkwgk....",
    ".........kggk...",
    "..........kgk...",
    "...........kgk..",
    "............kk..",
    "................",
  ]),
};

export default sheet;
