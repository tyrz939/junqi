// Seeded RNG. The sim never calls Math.random.
//
// sfc32: 32-bit state x4, passes PractRand, only uses integer ops that are
// exact in every JS engine. State is four plain numbers so it serialises into
// the save with the rest of GameState and resumes the identical sequence.

export type RngState = [number, number, number, number];

export function rngSeed(seed: number, stream = 0): RngState {
  // splitmix32 to spread a small seed over the whole state.
  let h = (seed ^ Math.imul(stream + 1, 0x9e3779b9)) >>> 0;
  const next = (): number => {
    h = (h + 0x9e3779b9) >>> 0;
    let z = h;
    z = Math.imul(z ^ (z >>> 16), 0x21f0aaad);
    z = Math.imul(z ^ (z >>> 15), 0x735a2d97);
    return (z ^ (z >>> 15)) >>> 0;
  };
  const s: RngState = [next(), next(), next(), next()];
  for (let i = 0; i < 12; i++) rngU32(s);
  return s;
}

export function rngU32(s: RngState): number {
  const t = (((s[0] + s[1]) >>> 0) + s[3]) >>> 0;
  s[3] = (s[3] + 1) >>> 0;
  s[0] = (s[1] ^ (s[1] >>> 9)) >>> 0;
  s[1] = (s[2] + (s[2] << 3)) >>> 0;
  s[2] = ((s[2] << 21) | (s[2] >>> 11)) >>> 0;
  s[2] = (s[2] + t) >>> 0;
  return t;
}

/** Float in [0, 1). 32 bits of entropy; exact in IEEE doubles. */
export function rngFloat(s: RngState): number {
  return rngU32(s) / 4294967296;
}

/** GameMaker `irandom(n)`: integer in [0, n] inclusive. n is floored; n < 0 gives 0. */
export function irandom(s: RngState, n: number): number {
  const m = Math.floor(n);
  if (m <= 0) return 0;
  return Math.floor(rngFloat(s) * (m + 1));
}

/** Integer in [lo, hi] inclusive. */
export function rngRange(s: RngState, lo: number, hi: number): number {
  return lo + irandom(s, hi - lo);
}

export function rngChance(s: RngState, p: number): boolean {
  return rngFloat(s) < p;
}

export function rngPick<T>(s: RngState, list: readonly T[]): T {
  return list[irandom(s, list.length - 1)];
}

/** FNV-1a. Used to derive a per-zone stream from the run seed, and for state hashes. */
export function hashString(text: string, h = 0x811c9dc5): number {
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/**
 * The dice of one named step of a build, from (seed, step, attempt): `stepDice(seed, "county:rail", 3)`.
 * Each step of world generation throws its own, so re-tuning one step (a rail's bends, one more
 * placement row, a thinner scatter) moves nothing drawn under any other name.
 */
export function stepDice(seed: number, step: string, attempt = 0): RngState {
  return rngSeed(hashString(`${seed >>> 0}:${step}`), attempt);
}
