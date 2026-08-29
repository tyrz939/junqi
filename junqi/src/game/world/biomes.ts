export type Biome = "town" | "farm" | "forest" | "grave" | "mountain" | "works" | "river" | "yard";

export function biomeAt(cx: number, cy: number): Biome {
  if (cx > 1680) return "mountain";
  if (cx > 1360 && cy > 760) return "grave";
  if (cx > 980 && cx < 1320 && cy > 560 && cy < 860) return "works";
  if (cx < 420 && cy < 320) return "town";
  if (cx > 420 && cx < 720 && cy > 520 && cy < 820) return "farm";
  if (cx < 560 && cy > 480) return "forest";
  if (cx > 1600 && cy > 880) return "yard";
  return "river";
}

export function biomeCold(biome: Biome, time: number, zone: string): number {
  let cold = 0;
  if (time < 6 || time >= 20) cold += 18;
  if (biome === "mountain") cold += 22;
  if (biome === "grave") cold += 10;
  if (zone === "burial" || zone === "dungeon") cold += 16;
  if (zone === "mine" || zone === "pipes") cold += 8;
  if (zone === "factory") cold -= 12;
  if (zone === "house") cold -= 10;
  return cold;
}
