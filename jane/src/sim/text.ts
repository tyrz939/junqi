// Text rows may say {name}. The player picks what she is called at New Game;
// Castle, the dog and Julie's handwriting all use it. One place does the swap so
// no row ever has "Jane" baked in.

import { NIGHT_END_HOUR, NIGHT_START_HOUR, TICKS_PER_HOUR } from "@/sim/constants";
import type { GameState } from "@/sim/state";

export const DEFAULT_NAME = "Jane";
const MAX_NAME = 16;

/** `world` is the GameState: there is one heroine and one name, however many people are playing her. */
export function expandText(world: { name: string } | null, text: string): string {
  return text.includes("{") ? text.replaceAll("{name}", world?.name ?? DEFAULT_NAME) : text;
}

/** Trim, collapse spaces, drop anything that is not a letter, digit, space, hyphen or apostrophe. */
export function cleanName(raw: string | undefined): string {
  const name = (raw ?? "")
    .replace(/[^\p{L}\p{N} '\-]/gu, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, MAX_NAME);
  return name === "" ? DEFAULT_NAME : name;
}

/** Night for the sim: 21:00 to 06:00. */
export function isNight(state: GameState): boolean {
  const hour = state.clock / TICKS_PER_HOUR;
  return hour >= NIGHT_START_HOUR || hour < NIGHT_END_HOUR;
}
