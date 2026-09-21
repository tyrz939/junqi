// Castle School's staff. All three are somebody already in the game wearing different
// colours, which is the cheapest honest way to say "the same building, later": the Master is
// the bandit in a gown, the Caretaker is the mine's figure with a lamp on him, and the Ringer
// is the big one with the warmth taken out.
//
// A fragment may not import `@/art/units` (that module MERGES the fragments, so it would be a
// cycle and the sheet would be undefined at the moment this ran). `@/art/unit-base` is the
// base sheet on its own, and palette swaps of it are legitimate art (DUNGEONS.md 4.3).

import type { Palette, SpriteSheet } from "@/art/types";
import { PAL } from "@/art/types";
import { UNIT_BASE } from "@/art/unit-base";

/** The bandit's olive coat becomes a master's gown: black serge, chalk on the sleeves. */
const GOWN: Palette = { ...PAL, d: "#2b2a34", D: "#17161d", f: "#4a4856" };

/** The mine's figure, but lit: a caretaker walks his round behind a lamp. */
const LAMP: Palette = { ...PAL, v: "#6a5a3c", w: "#ffe8b0", W: "#c8a868", y: "#ffd070" };

/** The big one, with the warmth taken out of him. */
const COLD: Palette = { ...PAL, u: "#5a6472", U: "#333c48", s: "#c8cbd4", S: "#98a0ae" };

const sheet: SpriteSheet = {
  school_master: { ...UNIT_BASE.bandit, palette: GOWN },
  school_caretaker: { ...UNIT_BASE.miniboss, palette: LAMP },
  school_ringer: { ...UNIT_BASE.boss, palette: COLD },
};

export default sheet;
