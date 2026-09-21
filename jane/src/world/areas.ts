// Named patches that are more than a threat number. Most patches in areas.json are
// only that: the ground is whatever the biome made it and the danger is in who lives
// there. A few are PLACES, and a quest stands on them, so they are drawn: the
// allotments are plots, the Top Field is planted, Quarry Steps has a face.
//
// Drawn straight after the land and BEFORE the roads and chunks, so a road that
// crosses one simply crosses it. Each leaves the marks and rects its quests use.

import { Tile } from "@/sim/grid";
import type { Kit } from "@/world/kit";

/** May return SLOTS: exact cells offered to content placed by name (the adit's rock belongs against the face). */
type Dress = (k: Kit, cx: number, cy: number, radius: number) => Record<string, [number, number]> | void;

/** Rows of fenced plots either side of a cinder path. Plot nine is the one nobody lets. */
const allotments: Dress = (k, cx, cy) => {
  const w = 58;
  const h = 40;
  const x0 = cx - Math.floor(w / 2);
  const y0 = cy - Math.floor(h / 2);
  k.fill(x0, y0, w, h, Tile.Grass);
  const pathY = y0 + Math.floor(h / 2) - 1;
  k.fill(x0 - 4, pathY, w + 8, 3, Tile.Dirt);
  let n = 1;
  for (const top of [true, false]) {
    for (let col = 0; col < 5; col++, n++) {
      const px = x0 + 2 + col * 11;
      const py = top ? y0 + 2 : pathY + 5;
      const ph = top ? pathY - 2 - py : y0 + h - 2 - py;
      k.fill(px, py, 9, ph, Tile.Garden);
      // A fence on three sides; the side on the path is open, and plots are walkable, so nothing is ever shut in.
      for (let i = 0; i < 9; i++) k.set(px + i, top ? py - 1 : py + ph, Tile.Fence);
      for (let j = 0; j < ph; j += 2) {
        k.set(px - 1, py + j, Tile.Fence);
      }
      if (n === 9) {
        k.rect("plot_nine", { cx: px, cy: py, w: 9, h: ph });
        k.mark("plot_nine", px + 4, py + Math.floor(ph / 2), 1);
        k.mark("plot_nine_stake", px + 4, top ? py + ph + 1 : py - 2, 1);
      }
    }
  }
  k.mark("allotment_shed", x0 + w - 4, pathY + 1, 0);
};

/** A planted field: drills of turned earth with grass between. What grows in it is not turnips. */
const topField: Dress = (k, cx, cy, radius) => {
  const r = Math.floor(radius * 0.55);
  for (let y = cy - r; y <= cy + r; y++) {
    for (let x = cx - r; x <= cx + r; x++) {
      if ((x - cx) * (x - cx) + (y - cy) * (y - cy) > r * r) continue;
      const t = k.get(x, y);
      if (t === Tile.Water || t === Tile.Sand) continue;
      k.set(x, y, (y - cy + r) % 4 < 2 ? Tile.Garden : Tile.Grass);
    }
  }
  k.mark("top_field", cx, cy, 1);
};

/** A worked face with a ledge in front of it, and an adit somebody has put a rock across. */
const quarrySteps: Dress = (k, cx, cy) => {
  k.fill(cx - 16, cy - 6, 33, 16, Tile.Dirt);
  k.fill(cx - 14, cy - 12, 29, 7, Tile.Cliff);
  // The steps: two lower ledges either side, so the face reads as cut, not fallen.
  k.fill(cx - 22, cy - 8, 7, 4, Tile.Cliff);
  k.fill(cx + 16, cy - 8, 7, 4, Tile.Cliff);
  k.rect("quarry_top", { cx: cx - 5, cy: cy - 5, w: 11, h: 9 });
  k.mark("quarry_top", cx, cy, 3);
  k.mark("quarry_adit", cx, cy - 2, 3);
  // The rock across the adit stands flush under the face: three cells wide, two deep.
  return { quarry_adit: [cx - 1, cy - 5] };
};

export const AREA_DRESS: Record<string, Dress> = {
  allotments,
  top_field: topField,
  quarry_steps: quarrySteps,
};
