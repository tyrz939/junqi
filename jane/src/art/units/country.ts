// The people and animals of the open country. Two drawings of a person (bareheaded, and in a
// hat or a headscarf) dressed in different cloth, so a hamlet has a farmer, his wife, the old
// man and the woodcutter without a new drawing each. They are drawn a little shorter and
// broader than {name}, in plain working clothes, so nobody takes one of them for a friend at
// the next seat. Then the animals: a hen, a sheep, a rabbit, and the crows that follow a
// walker across a field.
//
// Frames as the base sheet: down / up / side (side faces east) and the walk alternates.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";
import { UNIT_BASE } from "@/art/unit-base";

// --- people ----------------------------------------------------------------------------------
// Palette characters the person drawing uses:
//   h H  hair (lit, shade)        s S  skin
//   a A  coat or dress            v    the front: apron, shirt or waistcoat
//   e E  trousers or skirt        T    boots
//   z Z  hat or headscarf (hatted drawing only)

const HEAD_BARE = {
  down: [
    "................",
    ".....kkkkkk.....",
    "....khhhhhHk....",
    "...khhhhhhhHk...",
    "...khhhhhhhHk...",
    "...khssssssHk...",
    "...kssksskssk...",
    "...kSssssssSk...",
    "....kSssssSk....",
  ],
  up: [
    "................",
    ".....kkkkkk.....",
    "....khhhhhHk....",
    "...khhhhhhhHk...",
    "...khhhhhhhHk...",
    "...khhhhhhhHk...",
    "...khhhhhhhHk...",
    "...kHhhhhhHHk...",
    "....kHHHHHHk....",
  ],
  side: [
    "................",
    ".....kkkkk......",
    "....khhhhhk.....",
    "...khhhhhhhk....",
    "...khhhhhhhk....",
    "...khhhhsssk....",
    "...khhhssksk....",
    "...kHhhsssssk...",
    "....kHhssSkk....",
  ],
};

const HEAD_HAT = {
  down: [
    "................",
    "......kkkk......",
    ".....kzzzZk.....",
    "..kkkzzzzzZkkk..",
    "..kZZZZZZZZZZk..",
    "...kkssssssSk...",
    "...kssksskssk...",
    "...kSssssssSk...",
    "....kSssssSk....",
  ],
  up: [
    "................",
    "......kkkk......",
    ".....kzzzZk.....",
    "..kkkzzzzzZkkk..",
    "..kZZZZZZZZZZk..",
    "...khhhhhhhHk...",
    "...khhhhhhhHk...",
    "...kHhhhhhHHk...",
    "....kHHHHHHk....",
  ],
  side: [
    "................",
    ".....kkkk.......",
    "....kzzzZk......",
    "...kzzzzzZkk....",
    "..kZZZZZZZZZk...",
    "...khhhhsssk....",
    "...khhhssksk....",
    "...kHhhsssssk...",
    "....kHhssSkk....",
  ],
};

const BODY = {
  down: [
    "....kkaaaakk....",
    "...kaavvvvaAk...",
    "..kaAavvvvaAAk..",
    "..kaAavvvvaAAk..",
    "..ksAavvvvaAsk..",
    "..kkkavvvvakkk..",
    "...kaavvvvaAk...",
    "...keeeeeeeEk...",
    "....keEkkeEk....",
    "....kTTkkTTk....",
    "....kkkkkkkk....",
  ],
  down2: [
    "....kkaaaakk....",
    "...kaavvvvaAk...",
    "..kaAavvvvaAAk..",
    "..kaAavvvvaAAk..",
    "..ksAavvvvaAsk..",
    "..kkkavvvvakkk..",
    "...kaavvvvaAk...",
    "...keeeeeeeEk...",
    "....keEkkkkk....",
    "....kTTk........",
    "....kkkk........",
  ],
  up: [
    "....kkaaaakk....",
    "...kaaaaaaaAk...",
    "..kaAaaaaaaAAk..",
    "..kaAaaaaaaAAk..",
    "..ksAaaaaaaAsk..",
    "..kkkaaaaaakkk..",
    "...kaaaaaaaAk...",
    "...keeeeeeeEk...",
    "....keEkkeEk....",
    "....kTTkkTTk....",
    "....kkkkkkkk....",
  ],
  up2: [
    "....kkaaaakk....",
    "...kaaaaaaaAk...",
    "..kaAaaaaaaAAk..",
    "..kaAaaaaaaAAk..",
    "..ksAaaaaaaAsk..",
    "..kkkaaaaaakkk..",
    "...kaaaaaaaAk...",
    "...keeeeeeeEk...",
    "....kkkkkeEk....",
    "........kTTk....",
    "........kkkk....",
  ],
  side: [
    ".....kkaaakk....",
    "....kaaaavvk....",
    "....kaAaavvk....",
    "....kaAaavvk....",
    "....kaAsSavk....",
    "....kkaaaavk....",
    "....kaaaavvk....",
    "....keeeeeEk....",
    ".....keEeEk.....",
    ".....kTTTTk.....",
    ".....kkkkkk.....",
  ],
  side2: [
    ".....kkaaakk....",
    "....kaaaavvk....",
    "....kaAaavvk....",
    "....kaAaavvk....",
    "....kaAsSavk....",
    "....kkaaaavk....",
    "....kaaaavvk....",
    "....keeeeeEk....",
    "....keEk.keEk...",
    "...kTTk..kTTk...",
    "...kkkk..kkkk...",
  ],
};

function person(head: typeof HEAD_BARE, palette: Palette): SpriteSrc {
  return {
    w: 16,
    h: 20,
    ax: 8,
    ay: 18,
    palette: { ...PAL, ...palette },
    frames: {
      down: [...head.down, ...BODY.down],
      down2: [...head.down, ...BODY.down2],
      up: [...head.up, ...BODY.up],
      up2: [...head.up, ...BODY.up2],
      side: [...head.side, ...BODY.side],
      side2: [...head.side, ...BODY.side2],
    },
  };
}

// Cloth. Plain, faded, nothing a person would be looked at for.
const SKIN = { s: "#e8b890", S: "#c08860" };
const SKIN_OLD = { s: "#dcae8c", S: "#a8785a" };
const BOOTS = { T: "#3e2c20" };

// --- animals -----------------------------------------------------------------------------------

const HEN: SpriteSrc = {
  w: 10,
  h: 10,
  ax: 5,
  ay: 9,
  palette: { ...PAL },
  frames: {
    down: ["....kk....", "...krrk...", "..kwwwwk..", "..kwkkwk..", "..kwwowk..", ".kwwwwwwk.", ".kwWwwWwk.", "..kwwwwk..", "...kyky...", "..........",],
    down2: ["..........", "....kk....", "...krrk...", "..kwwwwk..", "..kwkkwk..", "..kwwowk..", ".kwwwwwwk.", ".kwWwwWwk.", "..kwwwwk..", "...kyky...",],
    up: ["....kk....", "...krrk...", "..kwwwwk..", "..kwwwwk..", ".kwwwwwwk.", ".kwWwwWwk.", ".kWwwwwWk.", "..kWWWWk..", "...kyky...", "..........",],
    side: [".....kk...", "....krrk..", "...kwwkwk.", ".k.kwwwwok", "kWkwwwwwk.", "kWwwwWwwk.", ".kWwwWwwk.", "..kkWWkk..", "...ky.yk..", "..........",],
    side2: ["..........", ".....kk...", "....krrk..", "...kwwkwk.", ".k.kwwwwok", "kWkwwwwwk.", "kWwwwWwwk.", ".kWwwWwwk.", "..kkWWkk..", "....yky...",],
  },
};

const SHEEP: SpriteSrc = {
  w: 16,
  h: 13,
  ax: 8,
  ay: 12,
  palette: { ...PAL, x: "#2a2630" },
  frames: {
    down: [
      "....kkkkkkkk....",
      "..kkwwwWwwwWkk..",
      ".kwwWwkkkkwwWwk.",
      ".kwWwkxxxxkwWWk.",
      "kwwwkxwxxwxkWWWk",
      "kwWwkxxxxxxkWwWk",
      "kwwwWkxxxxkwWWWk",
      "kWwwwwkkkkwwWWWk",
      ".kWwWwwwWwwwWWk.",
      "..kWWWWWWWWWWk..",
      "...kxk.kk.kxk...",
      "...kxk.kk.kxk...",
      "...kkk....kkk...",
    ],
    up: [
      "....kkkkkkkk....",
      "..kkwwwWwwwWkk..",
      ".kwwWwwwwwwwWwk.",
      ".kwWwwwWwwwwWWk.",
      "kwwwwWwwwwWwWWWk",
      "kwWwwwwwWwwwWwWk",
      "kwwwWwwwwwwwWWWk",
      "kWwwwwwWwwwwWWWk",
      ".kWwWwwwWwwwWWk.",
      "..kWWWWWWWWWWk..",
      "...kxk.kk.kxk...",
      "...kxk....kxk...",
      "...kkk....kkk...",
    ],
    side: [
      "................",
      "...kkkkkkkk.....",
      ".kkwwWwwwwWkkk..",
      "kwwWwwwWwwwwkxxk",
      "kwWwwwwwwWwkxwxk",
      "kwwwWwwwwwwkxxxk",
      "kWwwwwWwwwwwkxk.",
      "kWWwwwwwwWwWk...",
      ".kWWWWWWWWWWk...",
      "..kxk.kxk.kxk...",
      "..kxk.kxk.kxk...",
      "..kkk.kkk.kkk...",
      "................",
    ],
    side2: [
      "................",
      "...kkkkkkkk.....",
      ".kkwwWwwwwWkkk..",
      "kwwWwwwWwwwwkxxk",
      "kwWwwwwwwWwkxwxk",
      "kwwwWwwwwwwkxxxk",
      "kWwwwwWwwwwwkxk.",
      "kWWwwwwwwWwWk...",
      ".kWWWWWWWWWWk...",
      ".kxk..kxk.kxk...",
      "kxk....kxk.kxk..",
      "kkk....kkk.kkk..",
      "................",
    ],
  },
};

const RABBIT: SpriteSrc = {
  w: 10,
  h: 11,
  ax: 5,
  ay: 10,
  palette: { ...PAL, a: "#9a8068", A: "#6a5646" },
  frames: {
    down: ["..k....k..", ".kak..kak.", ".kak..kak.", ".kAkkkkAk.", "..kaaaaAk.", "..kkaakAk.", "..kaWWaAk.", ".kaaaaaaAk", ".kaaaaaAAk", "..kkkkkkk.", ".........."],
    up: ["..k....k..", ".kak..kak.", ".kak..kak.", ".kAkkkkAk.", "..kaaaaAk.", "..kaaaaAk.", "..kaaaaAk.", ".kaaaaaaAk", ".kaaWWaAAk", "..kkkkkkk.", ".........."],
    side: ["...kk.....", "..kak.....", "..kAak....", "...kAkkk..", "..kaaakak.", ".kaaaaaaak", "kWaaaaaAk.", "kWaaaaAAk.", ".kkaaAAkk.", "..kkkkk...", ".........."],
    side2: ["..........", "...kk.....", "..kak.....", "..kAak....", "...kAkkkk.", "..kaaaakak", ".kaaaaaaak", "kWaaaaaAk.", "kWaaaaAAk.", ".kkk..kkk.", ".........."],
  },
};

const CROW: SpriteSrc = {
  w: 12,
  h: 10,
  ax: 6,
  ay: 9,
  palette: { ...PAL, x: "#221e2a", X: "#3c3648" },
  frames: {
    down: ["............", "....kkkk....", "...kxXXxk...", "...kxrxrk...", "....kyYk....", "..kkxXXxkk..", ".kxxXxxXxxk.", ".kXxxxxxxXk.", "..kkkkkkkk..", "....y..y...."],
    down2: ["k..........k", "kk..kkkk..kk", "kxk.kxXxk.xk", ".kxkxrxrkxk.", "..kxkyYkxk..", "...kxXXxk...", "...kxxxxk...", "....kkkk....", "....y..y....", "............"],
    up: ["............", "....kkkk....", "...kxxxXk...", "...kxxxXk...", "....kxXk....", "..kkxxxxkk..", ".kxxxxxxxxk.", ".kXxxxxxxXk.", "..kkxXXxkk..", "....k..k...."],
    side: ["............", "......kkk...", ".....kxXxk..", ".....kxrxkyk", "..kkkxxxxkk.", ".kxxxxXXxk..", "kxXxxxxxk...", ".kkkxxxxk...", "....kkkk....", ".....y.y...."],
    side2: ["...kk.......", "..kxxk.kkk..", "...kxxkxXxk.", "....kxxrxkyk", "..kkxxxxxkk.", ".kxxxxXXxk..", "kxXxxxxxk...", ".kkkkkkkk...", "............", "............"],
  },
};

const sheet: SpriteSheet = {
  // The farmer: waistcoat over a shirt, a hat against the weather.
  folk_farmer: person(HEAD_HAT, { ...SKIN, ...BOOTS, h: "#6a4a30", H: "#4a321f", z: "#b89858", Z: "#86683a", a: "#5c6a44", A: "#3c4a2c", v: "#d8ccb0", e: "#4a4034", E: "#2e2820" }),
  // His wife, or somebody's: a print dress and an apron, a headscarf.
  folk_wife: person(HEAD_HAT, { ...SKIN, ...BOOTS, h: "#8a5a38", H: "#5e3c24", z: "#9a4a48", Z: "#6a2e30", a: "#6a7aa0", A: "#465478", v: "#ece4d0", e: "#6a7aa0", E: "#465478" }),
  // An old man with nothing on his head and all day to stand at a gate.
  folk_old: person(HEAD_BARE, { ...SKIN_OLD, ...BOOTS, h: "#d8d4cc", H: "#a8a49c", a: "#5a5048", A: "#3a342e", v: "#8a7e70", e: "#4a4642", E: "#2e2c2a" }),
  // A woman in a brown coat, bareheaded, her hair pinned up.
  folk_woman: person(HEAD_BARE, { ...SKIN, ...BOOTS, h: "#4a3024", H: "#2e1c16", a: "#8a5a3a", A: "#5e3a24", v: "#d8c8a8", e: "#5a4a3a", E: "#3a2e24" }),
  // The woodcutter: a red check shirt, a knitted cap.
  folk_woodcutter: person(HEAD_HAT, { ...SKIN, ...BOOTS, h: "#5a3a24", H: "#3a2416", z: "#4a5a6a", Z: "#2e3a46", a: "#9a3a30", A: "#6a2420", v: "#c85a48", e: "#3e4a58", E: "#28303a" }),
  // Waders and oilskin: the reedcutters work the Waters' edge.
  folk_reedcutter: person(HEAD_HAT, { ...SKIN, ...BOOTS, h: "#6a5a48", H: "#4a3e30", z: "#c8a848", Z: "#8e7630", a: "#b89a40", A: "#86702c", v: "#d0b458", e: "#3a4a3a", E: "#243024" }),
  // Whoever keeps the inn: a clean apron and rolled sleeves.
  folk_keeper: person(HEAD_BARE, { ...SKIN, ...BOOTS, h: "#2e2420", H: "#1e1814", a: "#3e4658", A: "#282e3c", v: "#f0ece0", e: "#3e4658", E: "#282e3c" }),
  // A man in a grey coat who looks as if he is waiting for a bus.
  folk_man: person(HEAD_BARE, { ...SKIN, ...BOOTS, h: "#7a5a3a", H: "#54402a", a: "#6e6e74", A: "#4a4a50", v: "#a8a8ac", e: "#3e3e44", E: "#28282e" }),

  // A ruffian is the bandit without the ice: a working coat, a scarf over the face, and a stick.
  ruffian: { ...UNIT_BASE.bandit, palette: { ...UNIT_BASE.bandit.palette, d: "#6a5438", D: "#46361f", f: "#8e7650", r: "#5a5a60", R: "#3a3a40" } },

  hen: HEN,
  sheep: SHEEP,
  rabbit: RABBIT,
  crow: CROW,
};

export default sheet;
