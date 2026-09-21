// The Burial Chamber's dead. Three of them are the same figure in different colours, which is
// the point: the shade is the soldier with the light taken out of him, and Goldskin is the
// soldier with gold poured over him. Light from the top-left, outline PAL.k, as everywhere.
//
// (A fragment cannot read UNIT_SPRITES: the eager glob in art/units.ts makes this module a
// static import of that one, so the sheet is not built yet when this runs. The frames a
// palette swap would have borrowed are therefore written out once, here, and shared.)

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

/** a/b head, c/d face, e/f body, g feet. */
const FIGURE: Record<string, string[]> = {
  down: [
    "................",
    ".....kkkkkk.....",
    "....kaaaaaak....",
    "...kaabbbbaak...",
    "...kabccccbak...",
    "..kabcdccdcbak..",
    "...kbccccccbk...",
    "...kkcccccckk...",
    "....kkeeeekk....",
    "..kkfeeeeeefkk..",
    "..kffeeeeeeffk..",
    "..kffeeeeeeffk..",
    "..kfkeeeeeekfk..",
    "..kkkeeeeeekkk..",
    "....keeeeeek....",
    "....keekkeek....",
    "....keekkeek....",
    "....kggkkggk....",
    "....kkkkkkkk....",
    "................",
  ],
  up: [
    "................",
    ".....kkkkkk.....",
    "....kaaaaaak....",
    "...kaabbbbaak...",
    "...kabbbbbbak...",
    "..kabbbbbbbbak..",
    "...kbbbbbbbbk...",
    "...kkbbbbbbkk...",
    "....kkeeeekk....",
    "..kkfeeeeeefkk..",
    "..kffeeeeeeffk..",
    "..kffeeeeeeffk..",
    "..kfkeeeeeekfk..",
    "..kkkeeeeeekkk..",
    "....keeeeeek....",
    "....keekkeek....",
    "....keekkeek....",
    "....kggkkggk....",
    "....kkkkkkkk....",
    "................",
  ],
  side: [
    "................",
    ".....kkkkk......",
    "....kaaaaak.....",
    "...kaabbbbk.....",
    "...kabccck......",
    "..kabcddck......",
    "...kbcccck......",
    "...kkccckk......",
    "....kkeekk......",
    "..kkfeeeeekk....",
    "..kffeeeeeek....",
    "..kffeeeeeek....",
    "...kfeeeeeek....",
    "...kkeeeeeek....",
    "....keeeeek.....",
    "....keeekk......",
    "....keek........",
    "....kggk........",
    "....kkkk........",
    "................",
  ],
};

const BLOOM: Record<string, string[]> = {
  down: [
    "................",
    "......kkk.......",
    "....kkaaakk.....",
    "...kaabbbaak....",
    "..kaabcccbaak...",
    "..kabcdddcbak...",
    "..kabcdwdcbak...",
    "..kabcdddcbak...",
    "..kaabcccbaak...",
    "...kaabbbaak....",
    "....kkaaakk.....",
    "......kek.......",
    "......kek.......",
    ".....kkekk......",
    "....kneeenk.....",
    "...knneeennk....",
    "..knnneeennnk...",
    "..knnnnnnnnnk...",
    "...kkkkkkkkk....",
    "................",
  ],
  up: [
    "................",
    "......kkk.......",
    "....kkbbbkk.....",
    "...kbbnnnbbk....",
    "..kbbnnnnnbbk...",
    "..kbnnnnnnnbk...",
    "..kbnnnNnnnbk...",
    "..kbnnnnnnnbk...",
    "..kbbnnnnnbbk...",
    "...kbbnnnbbk....",
    "....kkbbbkk.....",
    "......kek.......",
    "......kek.......",
    ".....kkekk......",
    "....kneeenk.....",
    "...knneeennk....",
    "..knnneeennnk...",
    "..knnnnnnnnnk...",
    "...kkkkkkkkk....",
    "................",
  ],
  side: [
    "................",
    ".....kkk........",
    "...kkaaakk......",
    "..kaabbbaak.....",
    "..kabcccbak.....",
    "..kbcdddcbk.....",
    "..kbcdwdcbk.....",
    "..kbcdddcbk.....",
    "..kabcccbak.....",
    "..kaabbbaak.....",
    "...kkaaakk......",
    ".....kek........",
    ".....kek........",
    "....kkekk.......",
    "...kneeenk......",
    "..knneeennk.....",
    ".knnneeennnk....",
    ".knnnnnnnnnk....",
    "..kkkkkkkkk.....",
    "................",
  ],
};

const SPIDER: Record<string, string[]> = {
  down: [
    "................",
    "..k..........k..",
    "..kk........kk..",
    "...kk......kk...",
    "....kk....kk....",
    "..kkkkkkkkkkkk..",
    ".kkaaaaaaaaaakk.",
    ".kaarrrrrrrraak.",
    ".kaarwrrrrwraak.",
    ".kaarrrrrrrraak.",
    "..kaarrrrrrak...",
    "...kkaaaakk.....",
    "..kk......kk....",
    ".kk........kk...",
    ".k..........k...",
    "................",
  ],
  up: [
    "................",
    "..k..........k..",
    "..kk........kk..",
    "...kk......kk...",
    "....kk....kk....",
    "..kkkkkkkkkkkk..",
    ".kkaaaaaaaaaakk.",
    ".kaaaaaaaaaaaak.",
    ".kaaarraarraaak.",
    ".kaaaaaaaaaaaak.",
    "..kaaaaaaaaak...",
    "...kkaaaakk.....",
    "..kk......kk....",
    ".kk........kk...",
    ".k..........k...",
    "................",
  ],
  side: [
    "................",
    "..k.......k.....",
    "..kk.....kk.....",
    "...kk...kk......",
    "....kk.kk.......",
    "..kkkkkkkkkk....",
    ".kkaaaaaaaakk...",
    ".kaarrrrrrwak...",
    ".kaarrrrrrwak...",
    ".kaarrrrrraak...",
    "..kaarrrraak....",
    "...kkaaaakk.....",
    "..kk....kk......",
    ".kk......kk.....",
    ".k........k.....",
    "................",
  ],
};

function figure(palette: Palette, frames = FIGURE, h = 20): SpriteSrc {
  return { w: 16, h, ax: 8, ay: h - 2, palette: { ...PAL, ...palette }, frames };
}

const sheet: SpriteSheet = {
  // Two tones and an outline. Whatever it was wearing, the light does not reach it.
  shade: figure({ a: "#3a3448", b: "#241f30", c: "#2e2a3c", d: "#181422", e: "#221d2e", f: "#3a3448", g: "#181422" }),

  // Parade dress, kept up: steel, a red tabard, boots that were polished this morning.
  the_soldier: figure({ a: "#c2ccd6", b: "#6c7680", c: "#cfc8b8", d: "#8a8f98", e: "#7a2430", f: "#c8403c", g: "#4a3626" }),

  // Gold over everything, including the face. It is not thick.
  goldskin: figure({ a: "#f0d048", b: "#b08828", c: "#f4e0a0", d: "#b08828", e: "#5c3878", f: "#a868c8", g: "#b08828" }),

  // Rooted. The petals are the only part that moves.
  great_flower: figure({ a: "#e8708c", b: "#a83c5c", c: "#f0d048", d: "#b08828", e: "#3c8844", n: "#22503a", N: "#78c850", w: "#f4f0e6" }, BLOOM),

  spider_queen: figure({ a: "#4a3050", r: "#7a3c30", w: "#f0d048" }, SPIDER, 16),
};

export default sheet;
