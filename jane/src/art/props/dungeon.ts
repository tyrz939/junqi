// Props every generated dungeon shares: the jars and pages that growth is found in, and
// the way in that appears beside a gate when a room seals. Same hand as art/props.ts:
// top-down 3/4, light from the top-left, 1 px dark outline, the sprite as wide as its footprint.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // the dark inside a jar, the gap under a gate
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  jar: prop(8, 10, 1, {
    base: [
      "..kkkk..",
      ".kmmmmk.",
      "..kttk..",
      ".kttttk.",
      "kttmmttk",
      "ktmttttk",
      "kttttTtk",
      "kttttTTk",
      ".kTTTTk.",
      "..kkkk..",
    ],
    on: [
      "........",
      "........",
      "..kkkk..",
      ".kxxxxk.",
      "kttxxttk",
      "ktmttttk",
      "kttttTtk",
      "kttttTTk",
      ".kTTTTk.",
      "..kkkk..",
    ],
  }),

  jar_big: prop(16, 18, 2, {
    base: [
      "....kkkkkkkk....",
      "...kmmmmmmmmk...",
      "...kttttttttk...",
      "....kkttttkk....",
      "...kttttttttk...",
      "..kttmmmmttttk..",
      ".kttmmttttttttk.",
      ".ktmmtttttttTtk.",
      "kttmttttttttTTtk",
      "kttmttyyyytttTtk",
      "ktttttyYYytttTtk",
      "kttttttyyttttTtk",
      "kttttttttttTTTtk",
      ".ktttttttttTTtk.",
      ".kTttttttTTTTTk.",
      "..kTTTTTTTTTTk..",
      "...kkkkkkkkkk...",
      "................",
    ],
    on: [
      "................",
      "................",
      "................",
      "....kkkkkkkk....",
      "...kxxxxxxxxk...",
      "..kttxxxxxxttk..",
      ".kttmmttttttttk.",
      ".ktmmtttttttTtk.",
      "kttmttttttttTTtk",
      "kttmttyyyytttTtk",
      "ktttttyYYytttTtk",
      "kttttttyyttttTtk",
      "kttttttttttTTTtk",
      ".ktttttttttTTtk.",
      ".kTttttttTTTTTk.",
      "..kTTTTTTTTTTk..",
      "...kkkkkkkkkk...",
      "................",
    ],
  }),

  leaf_page: prop(8, 8, 1, {
    base: [
      "........",
      ".kkkkkk.",
      ".kyyywk.",
      ".kyYyyk.",
      ".kyyYyk.",
      ".kwyyyk.",
      ".kkkkkk.",
      "........",
    ],
  }),

  way_in: prop(16, 8, 1, {
    base: [
      "kkkkkkkkkkkkkkkk",
      "kTxxxxxxxxxxxxTk",
      "kTxxxxxxxxxxxxTk",
      "kTxxkxxxxxxkxxTk",
      "kTmmmmmmmmmmmmTk",
      "kTttttttttttttTk",
      "kkkkkkkkkkkkkkkk",
      "................",
    ],
  }),
};

export default sheet;
