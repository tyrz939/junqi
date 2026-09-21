// Icons the Burial Chamber adds. One: the thing he was holding.

import type { SpriteSheet } from "@/art/types";
import { PAL } from "@/art/types";

const sheet: SpriteSheet = {
  item_ball: {
    w: 16,
    h: 16,
    ax: 8,
    ay: 14,
    palette: { ...PAL, u: "#c8503c", U: "#7a2430", v: "#e88870" },
    frames: {
      base: [
        "................",
        "................",
        ".....kkkkk......",
        "...kkvvvuukk....",
        "..kvvvvuuuuUk...",
        "..kvvuuuuuuUk...",
        ".kvuuuuuuuuUUk..",
        ".kuuuuuuuuuUUk..",
        ".kuuuuuuuuUUUk..",
        ".kuuuuuuuUUUUk..",
        ".kUuuuuUUUUUUk..",
        "..kUuUUUUUUUk...",
        "..kUUUUUUUUk....",
        "...kkUUUUkk.....",
        ".....kkkkk......",
        "................",
      ],
    },
  },
};

export default sheet;
