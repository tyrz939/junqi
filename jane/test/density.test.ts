// Is the county full? (The density brief of 2026-09-23: "things mostly everywhere, with paths
// kind of safe.") A screen is what the camera shows at once, 48 x 27 cells. The county is cut
// into screens and each one is asked whether there is anything on it to look at or deal with:
// a unit, or a prop that is not a herb, a loose rock or a lamp. The table is printed on every
// run; the expectations under it are the floor.
//
//   open country    fewer than one screen in ten of the land she can reach is empty
//   the first walk  no screen the station road or the town road crosses is empty
//   in view         from almost every stretch of the first walk a place or a camp is in sight

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import type { Blueprint } from "@/world/blueprint";
import { buildZone } from "@/world";
import { countySkeleton } from "@/world/county";
import { MACRO, SKEL_W } from "@/world/skeleton";

const SEEDS = [3, 2026];
const SW = 48;
const SH = 27;
const MINOR = new Set(["herb", "rock", "lamp_post", "lamp_run"]);
const catalog = buildCatalog();

/** Every cell she can walk to from the platform, four ways, over tiles alone. */
function reach(bp: Blueprint): Uint8Array {
  const w = bp.w;
  const seen = new Uint8Array(w * bp.h);
  const queue = new Int32Array(w * bp.h);
  const flags = TILE_FLAGS;
  const solid = F_SOLID;
  const tiles = bp.tiles;
  let head = 0;
  let tail = 0;
  const push = (i: number): void => {
    if (seen[i] || (flags[tiles[i]] & solid) !== 0) return;
    seen[i] = 1;
    queue[tail++] = i;
  };
  push(bp.marks.start.cy * w + bp.marks.start.cx);
  while (head < tail) {
    const i = queue[head++];
    const x = i % w;
    if (x + 1 < w) push(i + 1);
    if (x > 0) push(i - 1);
    if (i + w < seen.length) push(i + w);
    if (i >= w) push(i - w);
  }
  return seen;
}

type Survey = {
  seed: number;
  screens: number;
  land: number;
  empty: number;
  firstScreens: number;
  firstEmpty: number;
  inView: number;
  props: number;
  units: number;
  hostile: number;
  folk: number;
  byDef: Record<string, number>;
};

function survey(seed: number): Survey {
  const bp = buildZone("county", seed);
  const sk = countySkeleton(seed, bp.attempts - 1);
  const gw = Math.floor(bp.w / SW);
  const gh = Math.floor(bp.h / SH);
  const things = new Uint16Array(gw * gh);
  const places: [number, number][] = [];
  const byDef: Record<string, number> = {};
  const screenOf = (x: number, y: number): number => {
    const sx = Math.floor(x / SW);
    const sy = Math.floor(y / SH);
    return sx < gw && sy < gh ? sy * gw + sx : -1;
  };
  for (const p of bp.props) {
    byDef[p.def] = (byDef[p.def] ?? 0) + 1;
    if (MINOR.has(p.def)) continue;
    const i = screenOf(p.cx, p.cy);
    if (i >= 0) things[i]++;
    places.push([p.cx, p.cy]);
  }
  let hostile = 0;
  let folk = 0;
  for (const u of bp.units) {
    const i = screenOf(u.cx, u.cy);
    if (i >= 0) things[i]++;
    if (catalog.units[u.def].faction === "friendly") folk++;
    else hostile++;
    places.push([u.cx, u.cy]);
  }
  // Land she can reach: a screen at least a quarter of which she can walk on.
  const seen = reach(bp);
  let land = 0;
  let empty = 0;
  for (let sy = 0; sy < gh; sy++) {
    for (let sx = 0; sx < gw; sx++) {
      let n = 0;
      for (let y = sy * SH; y < sy * SH + SH; y += 3) for (let x = sx * SW; x < sx * SW + SW; x += 3) n += seen[y * bp.w + x];
      if (n < (SW / 3) * (SH / 3) * 0.25) continue;
      land++;
      if (things[sy * gw + sx] === 0) empty++;
    }
  }
  // The first walk: every screen its two roads cross, and whether something stands within a screen of each stretch.
  const first = new Set<number>();
  let stretches = 0;
  let inView = 0;
  for (const r of sk.roads) {
    if (!((r.from === "station" && r.to === "julie_house") || (r.from === "julie_house" && r.to === "town"))) continue;
    for (const c of r.cells) {
      const x = (c % SKEL_W) * MACRO + MACRO / 2;
      const y = Math.floor(c / SKEL_W) * MACRO + MACRO / 2;
      first.add(screenOf(x, y));
      stretches++;
      if (places.some(([px, py]) => Math.abs(px - x) <= SW / 2 + 6 && Math.abs(py - y) <= SH / 2 + 4)) inView++;
    }
  }
  // A screen that is mostly a set place (a town, the station, Julie's yard) is the place's own business, not the country's.
  const inSite = (i: number): boolean => {
    const x = (i % gw) * SW + SW / 2;
    const y = Math.floor(i / gw) * SH + SH / 2;
    return Object.entries(bp.rects).some(([name, r]) => name.startsWith("site_") && x >= r.cx && y >= r.cy && x < r.cx + r.w && y < r.cy + r.h);
  };
  let firstEmpty = 0;
  for (const i of first) if (i >= 0 && things[i] === 0 && !inSite(i)) firstEmpty++;
  return {
    seed,
    screens: gw * gh,
    land,
    empty,
    firstScreens: first.size,
    firstEmpty,
    inView: inView / Math.max(1, stretches),
    props: bp.props.length,
    units: bp.units.length,
    hostile,
    folk,
    byDef,
  };
}

const col = (cells: (string | number)[]): string => cells.map((c) => String(c).padStart(11)).join("");

describe("density", () => {
  it("the county is full: something on nearly every screen, every screen of the first walk, a place always in view", { timeout: 120000 }, () => {
    const out = SEEDS.map(survey);
    const lines = [col(["seed", "screens", "land", "empty", "empty %", "walk scr", "walk empty", "in view %", "props", "hostile", "folk"])];
    for (const s of out) {
      lines.push(col([s.seed, s.screens, s.land, s.empty, ((100 * s.empty) / s.land).toFixed(1), s.firstScreens, s.firstEmpty, (100 * s.inView).toFixed(0), s.props, s.hostile, s.folk]));
    }
    console.log(`\nper-screen content, ${SW} x ${SH} cells a screen\n${lines.join("\n")}`);
    const top = Object.entries(out[0].byDef)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 40)
      .map(([k, v]) => `${k} ${v}`)
      .join(", ");
    console.log(`seed ${out[0].seed}, props by kind: ${top}`);
    for (const s of out) {
      expect(s.empty / s.land, `seed ${s.seed}: ${s.empty} of ${s.land} reachable land screens are empty`).toBeLessThan(0.1);
      expect(s.firstEmpty, `seed ${s.seed}: screens of the first walk with nothing on them`).toBe(0);
      expect(s.inView, `seed ${s.seed}: stretches of the first walk with nothing in view`).toBeGreaterThan(0.95);
      // It is lived in, and it is not safe.
      expect(s.folk).toBeGreaterThan(150);
      // Per screen, not a head count: 2500 on the 3.6 km county was 0.46 a screen of land; the 2 km square one
      // (Sept 24) is held to more than that, 0.7, which is about what the old one had (0.79).
      expect(s.hostile / s.land).toBeGreaterThan(0.7);
      for (const def of ["cottage_thatch", "farmhouse", "barn", "well", "fingerpost", "milestone", "tent", "den"]) expect(s.byDef[def] ?? 0, `seed ${s.seed}: no ${def}`).toBeGreaterThan(0);
    }
  });
});
