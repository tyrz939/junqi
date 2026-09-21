// Goldskin Works: the things the shift left running. All four are palette swaps of the base
// sheet (DUNGEONS.md 4.3 asks for new art; the shapes that exist already say the right thing,
// and a swap keeps one hand across the game). Import UNIT_BASE, never `@/art/units`: that file
// merges these fragments and the cycle leaves the sheet undefined.

import type { SpriteSheet } from "@/art/types";
import { UNIT_BASE } from "@/art/unit-base";

const sheet: SpriteSheet = {
  // A gantry sentry: painted steel with one lens, and nothing to say about the dark.
  factory_sentry: {
    ...UNIT_BASE.statue,
    palette: {
      ...UNIT_BASE.statue.palette,
      w: "#c8d4dc",
      W: "#8c98a2",
      g: "#8f9aa4",
      G: "#515b64",
      y: "#bfe4ff",
      Y: "#6aa8cc",
    },
  },

  // A hauler: a low steel trolley with an amber lamp on top, and a load it never put down.
  factory_hauler: {
    ...UNIT_BASE.lurker,
    palette: {
      ...UNIT_BASE.lurker.palette,
      v: "#7a7f86",
      V: "#4e535a",
      z: "#2f343a",
      c: "#e0a830",
      y: "#e08838",
    },
  },

  // The Charge Hand: the shift supervisor, in a long works coat, with a lamp in one hand.
  factory_charge_hand: {
    ...UNIT_BASE.miniboss,
    palette: {
      ...UNIT_BASE.miniboss.palette,
      v: "#39414d",
      K: "#2c3440",
      y: "#ffd070",
      Y: "#c89828",
      w: "#dcd2c0",
      W: "#a89e8c",
    },
  },

  // The Foreman: plated, and lit from the inside where the plate does not quite meet.
  factory_foreman: {
    ...UNIT_BASE.boss,
    palette: {
      ...UNIT_BASE.boss.palette,
      g: "#6f7a84",
      G: "#434c55",
      u: "#8a5a3a",
      U: "#563526",
      o: "#7fc8ff",
      y: "#bfe4ff",
      W: "#c9d4dc",
    },
  },
};

export default sheet;
