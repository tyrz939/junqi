// Text rows may say {name}. The player picks what she is called at New Game;
// Castle, the dog and Julie's handwriting all use it. One place does the swap so
// no row ever has "Jane" baked in.

import { NIGHT_END_HOUR, NIGHT_START_HOUR, TICKS_PER_HOUR } from "@/sim/constants";
import type { GameState } from "@/sim/state";
import { storyName } from "@/world/names";

export const DEFAULT_NAME = "Jane";
const MAX_NAME = 16;

/**
 * `world` is the GameState: there is one heroine and one name, however many people are playing her.
 *
 * A story told at a generated place (world/names.ts) says {place:<story>} wherever it names the
 * place, and this puts in what the board there says on this seed: "the hen house at Hollins Farm".
 * A pub's "The" goes lower case in the middle of a sentence: "at the Plough", "The Plough is shut."
 */
export function expandText(world: { name: string; seed?: number } | null, text: string): string {
  if (!text.includes("{")) return text;
  let out = text.replaceAll("{name}", world?.name ?? DEFAULT_NAME);
  if (out.includes("{place:")) {
    const seed = world?.seed ?? 0;
    out = out.replace(/\{place:([a-z0-9_]+)\}/g, (whole, id: string, at: number) => {
      const name = storyName(seed, id);
      if (!name) return whole;
      const before = out.slice(0, at).trimEnd();
      const opens = before === "" || /[.!?:"]$/.test(before);
      return !opens && name.startsWith("The ") ? `the ${name.slice(4)}` : name;
    });
  }
  return out;
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
