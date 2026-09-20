// Art is source code: every sprite is a grid of palette characters. No imported
// sheets (the 2020 project's art was purchased packs that cannot ship in a public
// repo), no binary assets, diffs you can read. The atlas builder (render/atlas.ts)
// rasterises these once at boot into one canvas.

/** One character per pixel. "." is always transparent. Every other char must be in the palette. */
export type Palette = Record<string, string>;

export type SpriteSrc = {
  /** Pixel size of every frame. */
  w: number;
  h: number;
  /**
   * Anchor: the pixel inside the frame that sits on the entity's position.
   * Units: the point between the feet (usually w/2, h-2).
   * Props: the top-left of the footprint (usually 0, h - footprintHeightPx), so
   * tall props rise upward out of their footprint.
   */
  ax: number;
  ay: number;
  palette: Palette;
  /** Frame name -> `h` strings of exactly `w` characters. */
  frames: Record<string, string[]>;
};

/**
 * Frame names the renderer looks for.
 *
 * Units:  "down" "up" "side" (side faces EAST; west is mirrored at draw time)
 *         optional walk alternates "down2" "up2" "side2", optional "dead"
 * Props:  "base", optional "on" (lever pulled / torch lit / plate pressed),
 *         optional "open" (chest looted, gate unlocked is simply not drawn)
 * Icons:  "base"
 */
export type SpriteSheet = Record<string, SpriteSrc>;

/** Shared 16-colour-ish palette so the whole game reads as one hand. Sheets may extend it. */
export const PAL: Palette = {
  k: "#1a1420", // outline / near black
  K: "#2e2838", // soft outline
  w: "#f4f0e6", // white
  W: "#cfc8b8", // off white / bone
  g: "#8a8f98", // grey
  G: "#555a66", // dark grey
  s: "#e8b890", // skin
  S: "#c08860", // skin shade
  r: "#c8403c", // red
  R: "#7a2430", // dark red
  o: "#e08838", // orange
  y: "#f0d048", // yellow
  Y: "#b08828", // gold shade
  l: "#78c850", // light green
  n: "#3c8844", // green
  N: "#22503a", // dark green
  b: "#58a8e8", // light blue
  B: "#3060b0", // blue
  i: "#a8e0f8", // ice
  p: "#a868c8", // purple
  P: "#5c3878", // dark purple
  t: "#a87848", // tan / wood
  T: "#6e4a2c", // dark wood
  m: "#c89868", // light wood
  e: "#4a3626", // earth
};
