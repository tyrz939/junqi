// The Gold Mine's own furniture: the Headmaster's confiscated drawer, the hoists round the
// arena, the cabinet in the pay office. Each broken thing has an `on` frame, which is the
// same thing mended: Repair switches it on and it stays where it was.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // the inside of an open drawer
  d: "#3a3e4a", // cold iron
  u: "#b0603a", // rust
};

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  page_repair: prop(16, 16, 2, {
    base: [
      "kkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmk",
      "kmttttttttttttmk",
      "kmttttttttttttmk",
      "kkkkkkkkkkkkkkkk",
      "kTttttttttttttTk",
      "kTtkkkkkkkkkktTk",
      "kTtkmmmmmmmmktTk",
      "kTtkmmmyymmmktTk",
      "kTtkmmmmmmmmktTk",
      "kTtkkkkkkkkkktTk",
      "kTttttttttttttTk",
      "kTtkkkkkkkkkktTk",
      "kTtkmmmyymmmktTk",
      "kTtkkkkkkkkkktTk",
      "kkkkkkkkkkkkkkkk",
    ],
    on: [
      "kkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmk",
      "kmttttttttttttmk",
      "kmttttttttttttmk",
      "kkkkkkkkkkkkkkkk",
      "kTttttttttttttTk",
      "kTtkkkkkkkkkktTk",
      "kTtkxxxxxxxxktTk",
      "kTtkxxwwxxxxktTk",
      "kTtkmmmmmmmmktTk",
      "kTtkmmmyymmmktTk",
      "kTtkkkkkkkkkktTk",
      "kTtkkkkkkkkkktTk",
      "kTtkmmmyymmmktTk",
      "kTtkkkkkkkkkktTk",
      "kkkkkkkkkkkkkkkk",
    ],
  }),

  broken_hoist: prop(16, 20, 2, {
    base: [
      "kkkkkkkkkkkkkkkk",
      "kdggggggggggggdk",
      "kdGGGGGGGGGGGGdk",
      "kkkkkkkkkkkkkkkk",
      ".kdk........kdk.",
      ".kdk..kuk...kdk.",
      ".kdk..kuk...kdk.",
      ".kdk...k....kdk.",
      ".kdk........kdk.",
      ".kdk........kdk.",
      ".kdk........kdk.",
      ".kdk.....k..kdk.",
      ".kdk....kuk.kdk.",
      ".kdk...kuuk.kdk.",
      ".kdk..kGGGk.kdk.",
      ".kdk.kGggGk.kdk.",
      "kkdkkkkkkkkkkdkk",
      "kdddddddddddgddk",
      "kdGGGGGGGGGGGGdk",
      "kkkkkkkkkkkkkkkk",
    ],
    on: [
      "kkkkkkkkkkkkkkkk",
      "kdggggggggggggdk",
      "kdGGGGGGGGGGGGdk",
      "kkkkkkkkkkkkkkkk",
      ".kdk..kgk...kdk.",
      ".kdk..kgk...kdk.",
      ".kdk..kgk...kdk.",
      ".kdk.kkgkk..kdk.",
      ".kdkkGGGGGk.kdk.",
      ".kdkkGggGGk.kdk.",
      ".kdkkGGGGGk.kdk.",
      ".kdk.kkkkk..kdk.",
      ".kdk........kdk.",
      ".kdk........kdk.",
      ".kdk........kdk.",
      ".kdk........kdk.",
      "kkdkkkkkkkkkkdkk",
      "kdddddddddddgddk",
      "kdGGGGGGGGGGGGdk",
      "kkkkkkkkkkkkkkkk",
    ],
  }),

  broken_cabinet: prop(24, 14, 1, {
    base: [
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kTtttttttkxxxxxxxxxxxkTk",
      "kTtmmmmmtkxxxxxxxxxxxkTk",
      "kTtmwwwmtkxxxkkkxxxxxkTk",
      "kTtmwrwmtkxxkmmmkxxxxkTk",
      "kTtmwwwmtkxkmmttmkxxxkTk",
      "kTtmmmmmtkkmmttttkxxxkTk",
      "kTtttttttkmmtttttkxxxkTk",
      "kTtttttytkkttttkkxxxxkTk",
      "kTtttttttkxkkkkxxxxxxkTk",
      "kTTTTTTTTkxxxxxxxxxxxkTk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
    ],
    on: [
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kmmmmmmmmmmmmmmmmmmmmmmk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
      "kTtttttttkktttttttttttTk",
      "kTtmmmmmtkktmmmmmmmmmtTk",
      "kTtmwwwmtkktmwwwwwwwmtTk",
      "kTtmwrwmtkktmwwwrwwwmtTk",
      "kTtmwwwmtkktmwwwwwwwmtTk",
      "kTtmmmmmtkktmmmmmmmmmtTk",
      "kTtttttttkktttttttttttTk",
      "kTtttttytkktyttttttttTTk",
      "kTtttttttkktttttttttttTk",
      "kTTTTTTTTkkTTTTTTTTTTTTk",
      "kkkkkkkkkkkkkkkkkkkkkkkk",
    ],
  }),
};

export default sheet;
