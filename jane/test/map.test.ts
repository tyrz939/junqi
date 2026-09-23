// The map: the county's explored record (sim, saved), and the chart drawn from it (pure UI maths).
import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { CELL, FOG_CELLS_OUT } from "@/sim/constants";
import { Tile, TILE_COUNT } from "@/sim/grid";
import { cloneState, decodeSave, encodeSave, hashState } from "@/sim/save";
import { NO_INPUT, Sim } from "@/sim/sim";
import { fogSeen } from "@/sim/zones";
import {
  buildingRects,
  centreOn,
  clampPan,
  clampZoomIndex,
  composeMap,
  fitPpm,
  mapStep,
  OUT_STEP,
  seenFraction,
  terrainPixels,
  zoomAt,
  ZOOMS,
} from "@/ui/map";
import { packRgb } from "@/ui/format";

const catalog = buildCatalog();

function blockOf(sim: Sim): { bx: number; by: number } {
  const k = CELL * sim.rt.fogCell;
  return { bx: Math.floor(sim.player.x / k), by: Math.floor(sim.player.y / k) };
}

describe("the county's explored record", () => {
  it("is a coarse bitset stamped round her as she goes, and nowhere else", () => {
    const sim = Sim.newGame(catalog, 4242);
    expect(sim.zone.id).toBe("county");
    expect(sim.rt.fogCell).toBe(FOG_CELLS_OUT);
    expect(sim.rt.fogW).toBe(Math.ceil(sim.rt.grid.w / FOG_CELLS_OUT));
    for (let t = 0; t < 12; t++) sim.tick(NO_INPUT);
    const { bx, by } = blockOf(sim);
    expect(fogSeen(sim.zone.fog, sim.rt.fogW, bx, by)).toBe(true);
    expect(fogSeen(sim.zone.fog, sim.rt.fogW, Math.min(sim.rt.fogW - 1, bx + 3), by)).toBe(true); // what she can see
    expect(fogSeen(sim.zone.fog, sim.rt.fogW, Math.min(sim.rt.fogW - 1, bx + 12), by)).toBe(false); // what she cannot
    // The whole county's record is small enough to save without thought.
    expect(sim.zone.fog.length).toBeLessThanOrEqual(Math.ceil((sim.rt.fogW * sim.rt.fogH) / 32));

    // Walking east marks new ground.
    const before = sim.zone.fog.reduce((n, w) => n + popcount(w), 0);
    for (let t = 0; t < 240; t++) sim.tick({ ...NO_INPUT, mx: 1, sprint: true });
    const after = sim.zone.fog.reduce((n, w) => n + popcount(w), 0);
    expect(after).toBeGreaterThan(before);
  });

  it("saves and loads, and a resumed game stamps the same bits as one that never stopped", () => {
    const walk = (t: number) => ({ ...NO_INPUT, mx: t < 150 ? 1 : 0, my: t < 150 ? 0 : 1, sprint: true });
    const straight = Sim.newGame(catalog, 99);
    for (let t = 0; t < 300; t++) straight.tick(walk(t));
    const first = Sim.newGame(catalog, 99);
    for (let t = 0; t < 140; t++) first.tick(walk(t));
    const text = encodeSave(cloneState(first.state), { zone: "county", day: 0, hour: 8, hp: 1, maxhp: 1 }, new Date(0));
    const decoded = decodeSave(text);
    expect(decoded.ok).toBe(true);
    if (!decoded.ok) return;
    expect(decoded.file.state.zones.county?.fog).toEqual(first.state.zones.county?.fog);
    const resumed = Sim.fromState(catalog, decoded.file.state);
    for (let t = 140; t < 300; t++) resumed.tick(walk(t));
    expect(resumed.state.zones.county?.fog).toEqual(straight.state.zones.county?.fog);
    expect(hashState(resumed.state)).toBe(hashState(straight.state));
  });
});

function popcount(v: number): number {
  let n = 0;
  for (let x = v >>> 0; x; x &= x - 1) n++;
  return n;
}

describe("the chart", () => {
  const pal = Uint32Array.from({ length: TILE_COUNT }, (_, i) => packRgb(i * 5, 100, 50));

  /** A 64x32-cell field: grass, a road across it, a pond, one house. */
  function field(): { tiles: Uint8Array; w: number; h: number } {
    const w = 64;
    const h = 32;
    const tiles = new Uint8Array(w * h).fill(Tile.Grass);
    for (let x = 0; x < w; x++) for (let y = 14; y < 18; y++) tiles[y * w + x] = Tile.Road;
    for (let x = 40; x < 56; x++) for (let y = 0; y < 8; y++) tiles[y * w + x] = Tile.Water;
    return { tiles, w, h };
  }

  it("is very low resolution out of doors, and inks roads, water and houses", () => {
    const { tiles, w, h } = field();
    expect(mapStep(3600, 2000, false)).toBe(OUT_STEP);
    expect(OUT_STEP).toBeGreaterThanOrEqual(8);
    const step = 8;
    const house = buildingRects([{ def: "cottage", cx: 2, cy: 2, hidden: false }, { def: "crate", cx: 20, cy: 2, hidden: false }], {
      cottage: { w: 8, h: 6, solid: true },
      crate: { w: 2, h: 2, solid: true },
    });
    expect(house).toEqual([{ cx: 2, cy: 2, w: 8, h: 6 }]); // a crate is not a house
    const t = terrainPixels(tiles, w, h, step, pal, false, house);
    const mw = w / step;
    expect(t.length).toBe(mw * (h / step));
    const at = (mx: number, my: number): number => t[my * mw + mx];
    const road = at(3, 1 + 1); // cells 24..31 x 16..23: the road's lower half
    const grass = at(3, 3);
    const pond = at(5, 0);
    const roofs = at(0, 0);
    expect(new Set([road, grass, pond, roofs]).size).toBe(4);
    for (let mx = 0; mx < mw; mx++) expect(at(mx, 1)).toBe(road); // the road runs the width unbroken
  });

  it("puts nothing on the ground but the ground: seen is terrain, unseen is smoke with no hint beneath", () => {
    const { tiles, w, h } = field();
    const step = 8;
    const mw = w / step;
    const mh = h / step;
    const t = terrainPixels(tiles, w, h, step, pal, false);
    const out = new Uint32Array(mw * mh);
    composeMap(t, new Float32Array(mw * mh).fill(1), mw, mh, false, out);
    expect(Array.from(out)).toEqual(Array.from(t)); // no way-out squares, no labels, no icons

    // Unseen: two different countries give the same smoke.
    const other = new Uint32Array(t.length).fill(packRgb(250, 10, 10));
    const a = new Uint32Array(mw * mh);
    const b = new Uint32Array(mw * mh);
    composeMap(t, new Float32Array(mw * mh), mw, mh, false, a);
    composeMap(other, new Float32Array(mw * mh), mw, mh, false, b);
    expect(Array.from(a)).toEqual(Array.from(b));
    for (const v of a) expect((v & 255) + ((v >>> 8) & 255) + ((v >>> 16) & 255)).toBeLessThan(150); // dark
  });

  it("reads the explored fraction off the fog bits at any block size", () => {
    // Fog of 8 cells a bit over a 32x16-cell zone: 4x2 bits. Mark the top-left and bottom-right.
    const fogW = 4;
    const fog = [(1 << 0) | (1 << 7)];
    const s8 = seenFraction(fog, 8, fogW, 2, 32, 16, 8);
    expect(Array.from(s8)).toEqual([1, 0, 0, 0, 0, 0, 0, 1]);
    const s16 = seenFraction(fog, 8, fogW, 2, 32, 16, 16);
    expect(Array.from(s16)).toEqual([0.25, 0.25]);
  });

  it("clamps zoom to its ladder and keeps pixels whole", () => {
    expect(clampZoomIndex(-5)).toBe(0);
    expect(clampZoomIndex(99)).toBe(ZOOMS.length - 1);
    expect(clampZoomIndex(1.4)).toBe(1);
    expect(ZOOMS.every((z) => Number.isInteger(z))).toBe(true);
    expect(fitPpm(1200, 600, 225, 125)).toBe(4);
    expect(fitPpm(100, 100, 225, 125)).toBe(1); // never below a pixel a pixel
  });

  it("zooms about the cursor and never pans the chart off the stage", () => {
    const v = { ppm: 4, ox: -100, oy: -50 };
    const z = zoomAt(v, 8, 300, 200);
    // The map point under the cursor stays under it.
    expect((300 - z.ox) / z.ppm).toBeCloseTo((300 - v.ox) / v.ppm, 5);
    expect((200 - z.oy) / z.ppm).toBeCloseTo((200 - v.oy) / v.ppm, 5);

    const stageW = 600;
    const stageH = 300;
    // Dragged far past the edge: pulled back so the map still fills the stage.
    const far = clampPan({ ppm: 8, ox: 5000, oy: -99999 }, stageW, stageH, 225, 125);
    expect(far.ox).toBe(0);
    expect(far.oy).toBe(stageH - 125 * 8);
    // Smaller than the stage: centred, whatever the drag.
    const small = clampPan({ ppm: 1, ox: 400, oy: -40 }, stageW, stageH, 225, 125);
    expect(small).toEqual({ ppm: 1, ox: Math.round((stageW - 225) / 2), oy: Math.round((stageH - 125) / 2) });
    // Centring on Jane puts her in the middle.
    const c = centreOn(8, 100, 60, stageW, stageH);
    expect(c.ox + 100 * 8).toBe(stageW / 2);
    expect(c.oy + 60 * 8).toBe(stageH / 2);
  });
});
