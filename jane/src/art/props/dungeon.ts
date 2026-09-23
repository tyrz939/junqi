// Props every generated dungeon shares: the jars and pages that growth is found in, and
// the way in that appears beside a gate when a room seals. Same hand as art/props.ts:
// top-down 3/4, light from the top-left, 1 px dark outline, the sprite as wide as its footprint.

import type { Palette, SpriteSheet, SpriteSrc } from "@/art/types";
import { PAL } from "@/art/types";

const PX: Palette = {
  ...PAL,
  x: "#0c0a12", // the dark inside a jar, the gap under a gate
  h: "#fff4b0", // flame, a lit bulb's core
  d: "#3a3e4a", // cold iron: brackets, cages
};

// --- lamps on walls ------------------------------------------------------------------------
// A dungeon's lamps hang on its walls (world/dungeon/lights.ts), in the wall cell itself. Each
// kind is one small head drawing, and the bracket is drawn four ways, for the wall it hangs on:
//   _n  on the face of the north wall, the face she sees: a plate, an arm, the head above it
//   _s  on the south wall, which she sees the top of: the head stands on the lip, seen from behind
//   _w  on the west wall: an arm out of the wall's edge, the head hung at the end of it
//   _e  the same, the other way
// Every drawing is 8 x 16 and anchored like any 1 x 1 prop: the bottom 8 rows are the wall cell.

type Head = string[];

function wallLamp(head: Head, side: "n" | "s" | "w" | "e"): string[] {
  const rows = Array.from({ length: 16 }, () => Array.from({ length: 8 }, () => "."));
  const hw = head[0].length;
  const hh = head.length;
  const paste = (grid: string[], x0: number, y0: number): void => {
    grid.forEach((row, j) => {
      for (let i = 0; i < row.length; i++) if (row[i] !== "." && y0 + j >= 0 && y0 + j < 16 && x0 + i >= 0 && x0 + i < 8) rows[y0 + j][x0 + i] = row[i];
    });
  };
  if (side === "n") {
    paste(["..kddk..", "..kGGk..", "...kk..."], 0, 12);
    paste(head, (8 - hw) >> 1, 12 - hh);
  } else if (side === "s") {
    paste(["..kGGk..", "...kk..."], 0, 9);
    paste(head, (8 - hw) >> 1, 9 - hh);
  } else {
    // Drawn for the west wall, then turned round for the east.
    const top = 12 - hh;
    paste(["kddk"], 0, top + 1);
    paste(["kd"], 0, top + 2);
    paste(head, 8 - hw, top);
    if (side === "e") for (const r of rows) r.reverse();
  }
  return rows.map((r) => r.join(""));
}

function lampSet(id: string, base: Head, on?: Head): SpriteSheet {
  const out: SpriteSheet = {};
  for (const side of ["n", "s", "w", "e"] as const) {
    const frames: Record<string, string[]> = { base: wallLamp(base, side) };
    if (on) frames.on = wallLamp(on, side);
    out[`${id}_${side}`] = { w: 8, h: 16, ax: 0, ay: 8, palette: PX, frames };
  }
  return out;
}

/** The Gold Mine: a miner's lantern on an iron hook, which is where a miner leaves it. */
const MINE_LAMP: Head = ["..k..", ".kdk.", "kdGdk", "kyhyk", "kyhyk", "kdGdk", ".kkk."];
/** The Burial: a cup of blue fire. They burn cold if they burn at all. */
const COLD_TORCH: Head = [".kk.", "kbBk", "kbik", "kiwk", ".kk.", "kGGk", ".kk."];
/** The Works: the cage is still on the wall. The foreman had the bulb taken out and wrote down why. */
const LAMP_CAGE: Head = [".kkk.", "kdxdk", "kxdxk", "kdxdk", "kxdxk", ".kkk."];
/** The Library: a brass plate and a candle. */
const CANDLE: Head = ["..y..", ".yhy.", "..y..", "..w..", "..W..", ".kYk.", "kYYYk", ".kkk."];
/** The Museum: an electric globe on a brass arm, on the same breaker as the galleries. */
const GLOBE_OFF: Head = [".kkk.", "kWgWk", "kgWgk", "kWgWk", ".kkk.", "..Y..", ".kYk."];
const GLOBE_ON: Head = [".kkk.", "kwhwk", "khwhk", "kwhwk", ".kkk.", "..Y..", ".kYk."];
/** Castle School: a gas lamp in a glass chimney, turned low. */
const GAS_LAMP: Head = ["..k..", ".kyk.", "kyhyk", "kyhyk", "kWyWk", ".kkk.", "..G..", ".kGk."];
/** The Pipes: a bulb in a cage, the kind that is meant to get wet. */
const CAGED_BULB: Head = [".kkk.", "kdydk", "kyhyk", "kdydk", ".kkk."];

/** footH is the footprint height in 8 px cells. */
function prop(w: number, h: number, footH: number, frames: Record<string, string[]>): SpriteSrc {
  return { w, h, ax: 0, ay: h - footH * 8, palette: PX, frames };
}

const sheet: SpriteSheet = {
  jar: prop(8, 10, 1, {
    base: [
      "..kkkk..",
      ".kmmmmk.",
      "..kttk..",
      ".kttttk.",
      "kttmmttk",
      "ktmttttk",
      "kttttTtk",
      "kttttTTk",
      ".kTTTTk.",
      "..kkkk..",
    ],
    on: [
      "........",
      "........",
      "..kkkk..",
      ".kxxxxk.",
      "kttxxttk",
      "ktmttttk",
      "kttttTtk",
      "kttttTTk",
      ".kTTTTk.",
      "..kkkk..",
    ],
  }),

  jar_big: prop(16, 18, 2, {
    base: [
      "....kkkkkkkk....",
      "...kmmmmmmmmk...",
      "...kttttttttk...",
      "....kkttttkk....",
      "...kttttttttk...",
      "..kttmmmmttttk..",
      ".kttmmttttttttk.",
      ".ktmmtttttttTtk.",
      "kttmttttttttTTtk",
      "kttmttyyyytttTtk",
      "ktttttyYYytttTtk",
      "kttttttyyttttTtk",
      "kttttttttttTTTtk",
      ".ktttttttttTTtk.",
      ".kTttttttTTTTTk.",
      "..kTTTTTTTTTTk..",
      "...kkkkkkkkkk...",
      "................",
    ],
    on: [
      "................",
      "................",
      "................",
      "....kkkkkkkk....",
      "...kxxxxxxxxk...",
      "..kttxxxxxxttk..",
      ".kttmmttttttttk.",
      ".ktmmtttttttTtk.",
      "kttmttttttttTTtk",
      "kttmttyyyytttTtk",
      "ktttttyYYytttTtk",
      "kttttttyyttttTtk",
      "kttttttttttTTTtk",
      ".ktttttttttTTtk.",
      ".kTttttttTTTTTk.",
      "..kTTTTTTTTTTk..",
      "...kkkkkkkkkk...",
      "................",
    ],
  }),

  leaf_page: prop(8, 8, 1, {
    base: [
      "........",
      ".kkkkkk.",
      ".kyyywk.",
      ".kyYyyk.",
      ".kyyYyk.",
      ".kwyyyk.",
      ".kkkkkk.",
      "........",
    ],
  }),

  way_in: prop(16, 8, 1, {
    base: [
      "kkkkkkkkkkkkkkkk",
      "kTxxxxxxxxxxxxTk",
      "kTxxxxxxxxxxxxTk",
      "kTxxkxxxxxxkxxTk",
      "kTmmmmmmmmmmmmTk",
      "kTttttttttttttTk",
      "kkkkkkkkkkkkkkkk",
      "................",
    ],
  }),

  ...lampSet("mine_lamp", MINE_LAMP),
  ...lampSet("cold_sconce", COLD_TORCH),
  ...lampSet("lamp_cage", LAMP_CAGE),
  ...lampSet("candle_sconce", CANDLE),
  ...lampSet("museum_wall_lamp", GLOBE_OFF, GLOBE_ON),
  ...lampSet("gas_lamp", GAS_LAMP),
  ...lampSet("caged_bulb", CAGED_BULB),
};

export default sheet;
