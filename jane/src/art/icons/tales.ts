// Item icons for the tales (data/items/tales.json). 16x16, 1px near-black outline, light from the
// top-left, like every other icon.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const ICON_PAL: Palette = {
  ...PAL,
  Z: "#26222c", // black crepe
  z: "#403a48",
  q: "#8a5a34", // tin, rust
  Q: "#5e3a24",
  c: "#b8b4ac", // steel
  C: "#7a7670",
};

function icon(rows: string[]): SpriteSrc {
  if (rows.length !== 16 || rows.some((r) => r.length !== 16)) throw new Error("tales icon is not 16x16");
  return { w: 16, h: 16, ax: 0, ay: 0, palette: ICON_PAL, frames: { base: rows } };
}

const sheet: SpriteSheet = {
  // A jar of comb honey with a paper lid tied on.
  item_honey: icon([
    "................",
    ".....kkkkkk.....",
    "....kwwwwwWk....",
    "....kkkkkkkk....",
    ".....kgggGk.....",
    "....kkkkkkkk....",
    "...koyyyyyyok...",
    "...kyyYyyyyYk...",
    "...kyyyyYyyYk...",
    "...kyYyyyyyYk...",
    "...kyyyyYyyYk...",
    "...koyyyyyyok...",
    "...kYYYYYYYYk...",
    "....kkkkkkkk....",
    "................",
    "................",
  ]),
  // A strip of black crepe, folded over on itself.
  item_crepe: icon([
    "................",
    "................",
    "..kkkkkkkkkk....",
    ".kzZzZzZzZzZk...",
    ".kZzZzZzZzZzZk..",
    "..kkkkkkkkkkZk..",
    "...........kZk..",
    "..kkkkkkkkkkZk..",
    ".kZzZzZzZzZzZk..",
    ".kzZzZzZzZzZk...",
    "..kkkkkkkkkk....",
    "..kZk...........",
    "..kZZk..........",
    "...kkk..........",
    "................",
    "................",
  ]),
  // A dinner plate with a blue rim, the knife and fork laid across it.
  item_plate: icon([
    "................",
    "................",
    ".....kkkkkk.....",
    "...kkBBBBBBkk...",
    "..kBBwwwwwwBBk..",
    "..kBwwwwwgwwBk..",
    ".kBwwwwwgwwwwBk.",
    ".kBwgwwgwwwwwBk.",
    ".kBwwgggwwwwwBk.",
    ".kBwwwgwwwwwWBk.",
    "..kBwwgwwwwWBk..",
    "..kBBwwwwWWBBk..",
    "...kkBBBBBBkk...",
    ".....kkkkkk.....",
    "................",
    "................",
  ]),
  // Horace: a tortoise from the side, his head out, looking the way he always looks.
  item_tortoise: icon([
    "................",
    "................",
    "................",
    "......kkkkk.....",
    "....kkmTmTmkk...",
    "...kmTmTmTmTmk..",
    "..kTmTmTmTmTmTk.",
    "..kmTmTmTmTmTmkk",
    ".kkkkkkkkkkkkkSk",
    "kSsk.kSk..kSskSk",
    "kSsk.kSk..kSsk.k",
    ".kk...k....kk...",
    "................",
    "................",
    "................",
    "................",
  ]),
  // A diver's spanner: a long steel handle and a wide jaw for wing nuts.
  item_spanner: icon([
    "................",
    ".kkk............",
    "kcck............",
    "kc.kk...........",
    "kcc.ck..........",
    ".kkcCck.........",
    "...kcCck........",
    "....kcCck.......",
    ".....kcCck......",
    "......kcCck.....",
    ".......kcCck....",
    "........kcCck...",
    ".........kcCkk..",
    "..........kCCCk.",
    "...........kkk..",
    "................",
  ]),
  // A short scarf in odd ends of wool, a darker row with a name in it.
  item_scarf: icon([
    "................",
    "..kkkkkkkk......",
    "..krrrrrRk......",
    "..kRrRrRrk......",
    "..kPPPPPPk......",
    "..krrrrrRk......",
    "..kBbBbBbk......",
    "..krrrrrRkkkkk..",
    "..kRrRrRrrrrRk..",
    "..kkkkkkkRrRrk..",
    "........kPPPPk..",
    "........krrrRk..",
    "........krkrkk..",
    "........kr.r.k..",
    "........kk.k.k..",
    "................",
  ]),
  // A can of paraffin with a screw cap and a handle.
  item_oil_can: icon([
    "................",
    "......kkk.......",
    "....kkkgkk......",
    "...kcCkkkCck....",
    "...kc.....ck....",
    "..kkkkkkkkkkk...",
    "..kqqqqqqqqQk...",
    "..kqwwwwwwqQk...",
    "..kqwrrrrwqQk...",
    "..kqwwwwwwqQk...",
    "..kqqqqqqqqQk...",
    "..kqqqqqqqqQk...",
    "..kQQQQQQQQQk...",
    "...kkkkkkkkk....",
    "................",
    "................",
  ]),
};

export default sheet;
