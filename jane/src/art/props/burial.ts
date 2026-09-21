// The Burial Chamber's own furniture. Two kinds of flame: the braziers and the great torch
// are warm (they keep things off), and the blue torches that were already drawn are cold
// (they show what is there). The webs and the dead hearth are what Fire opens.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  d: "#c08a3a", // bronze
  D: "#7a5426", // bronze shade
  f: "#ff8a30", // flame
  F: "#ffd060", // flame, hot
  x: "#0c0a12", // a cold hollow
  v: "#cfe0ff", // web
  V: "#8fa4c0", // web, shaded
  z: "#3a3344", // dead ash
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  brazier: prop(8, 16, 1, {
    base: [
      "........",
      "........",
      "........",
      "...kk...",
      "..kzzk..",
      "..kzzk..",
      "...kk...",
      "...kk...",
      "..kDDk..",
      ".kdDDdk.",
      ".kdDDdk.",
      "kdDddDdk",
      "kdddddDk",
      "kkDDDDkk",
      ".kkkkkk.",
      "........",
    ],
    on: [
      "...ff...",
      "..fFFf..",
      ".fFFFFf.",
      ".fFffFf.",
      "..fFFf..",
      "..kfFk..",
      "...kk...",
      "...kk...",
      "..kDDk..",
      ".kdDDdk.",
      ".kdDDdk.",
      "kdDddDdk",
      "kdddddDk",
      "kkDDDDkk",
      ".kkkkkk.",
      "........",
    ],
  }),

  great_torch: prop(16, 24, 2, {
    base: [
      "......ff........",
      ".....fFFf.......",
      "....fFFFFf......",
      "...fFFffFFf.....",
      "....fFFFFf......",
      ".....kfFk.......",
      "....kdDDdk......",
      "....kdDDdk......",
      "....kkDDkk......",
      ".....kddk.......",
      ".....kddk.......",
      ".....kddk.......",
      ".....kddk.......",
      ".....kddk.......",
      "....kdDDdk......",
      "...kdDddDdk.....",
      "..kdDddddDdk....",
      "..kdddddddDk....",
      "..kDDDDDDDDk....",
      "..kkkkkkkkkk....",
      "...kzzzzzzk.....",
      "...kzzzzzzk.....",
      "...kkkkkkkk.....",
      "................",
    ],
  }),

  web_wall_h: prop(24, 16, 2, {
    base: [
      "kvk..vk..kv..kv..kv..kvk",
      "kVvk.Vvk.vV.kvV.kvV.kVvk",
      "k.vVkv.Vkv.Vkv.Vkv.Vv.Vk",
      "kv.vVv..vV..vV..vV..vVvk",
      "kvVv.VvVv.Vv.Vv.Vv.Vv.vk",
      "kv.Vv.vv.Vvv.Vv.Vvv.Vv.k",
      "kvVv.Vv.Vv.Vv.Vv.Vv.Vvvk",
      "kv.Vv.Vv.Vv.Vv.Vv.Vv.Vvk",
      "kvVv.Vv.Vvv.Vv.Vvv.Vv.vk",
      "kv.Vv.Vv.Vv.Vv.Vv.Vv.Vvk",
      "kvVv.Vv.Vv.Vv.Vv.Vv.Vvvk",
      "kv.Vv.vv.Vvv.Vv.Vvv.Vv.k",
      "kvVv.VvVv.Vv.Vv.Vv.Vv.vk",
      "kv.vVv..vV..vV..vV..vVvk",
      "k.vVkv.Vkv.Vkv.Vkv.Vv.Vk",
      "kvk..vk..kv..kv..kv..kvk",
    ],
  }),

  web_wall_v: prop(8, 24, 3, {
    base: [
      "kvk..kvk",
      "kVvk.vVk",
      "kv.VkV.k",
      "kvVv.Vvk",
      "kv.Vv.vk",
      "kvVv.Vvk",
      "kv.Vv.vk",
      "kvVvvVvk",
      "kv.Vv.vk",
      "kvVv.Vvk",
      "kv.Vv.vk",
      "kvVvvVvk",
      "kv.Vv.vk",
      "kvVv.Vvk",
      "kv.Vv.vk",
      "kvVvvVvk",
      "kv.Vv.vk",
      "kvVv.Vvk",
      "kv.Vv.vk",
      "kvVv.Vvk",
      "kv.VkV.k",
      "kVvk.vVk",
      "kvk..kvk",
      "kvk..kvk",
    ],
  }),

  cold_hearth: prop(16, 16, 2, {
    base: [
      "................",
      "................",
      "....kkkkkk......",
      "..kkGggggGkk....",
      ".kGgxxxxxxgGk...",
      ".kgxxzzzzxxgk...",
      "kGxxzzzzzzxxGk..",
      "kgxzzxxxxzzxgk..",
      "kgxzzxxxxzzxgk..",
      "kGxxzzzzzzxxGk..",
      ".kgxxzzzzxxgk...",
      ".kGgxxxxxxgGk...",
      "..kkGggggGkk....",
      "....kkkkkk......",
      "................",
      "................",
    ],
    on: [
      "................",
      "......ff........",
      "....kkfFfk......",
      "..kkGfFFFGkk....",
      ".kGgfFFfFFgGk...",
      ".kgfFfffffFgk...",
      "kGffFffffFffGk..",
      "kgfFffFFffFfgk..",
      "kgffFFffFFffgk..",
      "kGffffFFffffGk..",
      ".kgffffffffgk...",
      ".kGgffffffgGk...",
      "..kkGggggGkk....",
      "....kkkkkk......",
      "................",
      "................",
    ],
  }),
};

export default sheet;
