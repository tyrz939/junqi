// Unit sprites: the base sheet (art/unit-base.ts) plus every fragment under art/units/.
// Art is source code: one palette character per pixel, "." is transparent.
import type { SpriteSheet } from "@/art/types";
import { UNIT_BASE } from "@/art/unit-base";

export const UNIT_SPRITES: SpriteSheet = { ...UNIT_BASE };

// --- seats ---------------------------------------------------------------------------
// Co-op: four people play one heroine with one name, so the only thing that tells them
// apart is the coat. Same drawing, three palette characters swapped: c coat, C shade,
// q highlight. Seat 0 keeps the plum. Colours are chosen to stay apart in the dark and
// under the mist, and to stay off the enemy reds.
export const SEAT_COATS: readonly { c: string; C: string; q: string }[] = [
  { c: "#8e4a86", C: "#5f2c5e", q: "#b673a8" }, // plum (the original)
  { c: "#2f7f8c", C: "#1c5260", q: "#5fb4bc" }, // teal
  { c: "#6b8a3a", C: "#435a24", q: "#9cba5e" }, // moss
  { c: "#b8752e", C: "#7c4a1c", q: "#e0a458" }, // ochre
];

export function seatSprite(sprite: string, seat: number): string {
  return `${sprite}@${seat}`;
}

/** The player sheets for seats 1..3: `jane@1` and so on. Seat 0 is the sheet as drawn. */
export function seatSheets(sprite = "jane"): SpriteSheet {
  const base = UNIT_SPRITES[sprite];
  const out: SpriteSheet = {};
  for (let seat = 1; seat < SEAT_COATS.length; seat++) {
    out[seatSprite(sprite, seat)] = { ...base, palette: { ...base.palette, ...SEAT_COATS[seat] } };
  }
  return out;
}

// --- fragments -------------------------------------------------------------------------
// A dungeon's creatures, a region's props: each set may live in a file of its own under
// ./units/ and `export default` a SpriteSheet. Merged here in path order; an id drawn twice is an error.
const UNIT_SPRITES_FRAGMENTS = import.meta.glob("./units/*.ts", { eager: true, import: "default" }) as Record<string, SpriteSheet>;
for (const path of Object.keys(UNIT_SPRITES_FRAGMENTS).sort()) {
  for (const [id, sprite] of Object.entries(UNIT_SPRITES_FRAGMENTS[path])) {
    if (id in UNIT_SPRITES) throw new Error(`art: "${id}" is drawn twice (again in ${path})`);
    UNIT_SPRITES[id] = sprite;
  }
}
