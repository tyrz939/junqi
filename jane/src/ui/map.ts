// Map tab. A rough chart, not a photograph: outdoors one map pixel stands for a 16x16 block of
// cells and says the most telling thing in it (a road, water, a roof, the woods). Ground she has
// not seen lies under a dark smoke that thins at the edge of where she has walked. Nothing is
// labelled; the one mark is the School, which she always knows the way to.
//
// Cheap by construction: the terrain colours are worked out once per zone, the fog is folded in
// only when the seen-bits changed (checked twice a second), and zoom and pan move a CSS box, so
// nothing is redrawn while you look at it. The player is a DOM dot with a CSS blink.

import { CELL } from "@/sim/constants";
import { Tile, TILE_COUNT } from "@/sim/grid";
import type { PlayerView as Sim } from "@/sim/sim";
import type { Ctx } from "@/ui/ctx";
import { cached, h, textOf } from "@/ui/dom";
import { packRgb, parseCssColor } from "@/ui/format";

/** Outdoors, one map pixel is this many cells on a side. */
export const OUT_STEP = 16;
/** Indoors the map is finer, but never wider or taller than this many pixels. */
const MAX_MAP_PX = 900;
/** Zoom is a whole multiple of the fit-to-window scale, so pixels stay square and crisp. */
export const ZOOMS = [1, 2, 3, 4, 6, 8] as const;
/** The zoom a map opens at outdoors: close enough to read the lanes round her. */
const OPEN_ZOOM_OUT = 2;

const UNKNOWN = packRgb(60, 50, 80);

// --- the chart: pure, no DOM ------------------------------------------------------------------

/**
 * Outdoors the chart speaks in a few inks, not in tiles: meadow, woods, heath, marsh, farmland,
 * sand, rock, water, road, roofs. Each ink claims a block by how much of it there is, times its
 * weight, so a road four cells wide still shows in a block of meadow, and so does a hamlet.
 */
const enum Ink {
  None,
  Meadow,
  Woods,
  Heath,
  Marsh,
  Farm,
  Sand,
  Rock,
  Water,
  Road,
  Roofs,
  Count,
}
/** Inks that are ground, which the chart smooths into areas. The rest are lines and marks. */
const AREA = (i: number): boolean => i >= Ink.Meadow && i <= Ink.Rock;
const INK_COLOUR: readonly number[] = [
  0,
  packRgb(108, 134, 74), // meadow
  packRgb(50, 84, 56), // woods
  packRgb(128, 108, 76), // heath
  packRgb(84, 104, 74), // marsh
  packRgb(140, 132, 76), // farmland
  packRgb(180, 158, 110), // sand
  packRgb(112, 104, 94), // rock
  packRgb(62, 98, 140), // water
  packRgb(204, 184, 140), // road
  packRgb(156, 72, 56), // roofs
];
const INK_WEIGHT: readonly number[] = [0, 1, 1, 1, 1, 1, 1, 1, 2.5, 4, 6];
const INK_OF: Uint8Array = (() => {
  const k = new Uint8Array(TILE_COUNT).fill(Ink.Meadow);
  k[Tile.Void] = Ink.None;
  const set = (ink: Ink, ts: Tile[]): void => ts.forEach((t) => (k[t] = ink));
  set(Ink.Woods, [Tile.Tree, Tile.Pine, Tile.Bush, Tile.Hedge]);
  set(Ink.Heath, [Tile.Dirt, Tile.DryBed, Tile.DeadTree, Tile.Track]); // a track is a trodden line on the heath, not a road
  set(Ink.Marsh, [Tile.Moss]);
  set(Ink.Farm, [Tile.Crops, Tile.Garden]);
  set(Ink.Sand, [Tile.Sand]);
  set(Ink.Rock, [Tile.Cliff, Tile.Rubble, Tile.Cobble, Tile.CaveWall, Tile.CaveFloor]); // cobble is stony ground as often as a street
  set(Ink.Water, [Tile.Water, Tile.Ice]);
  set(Ink.Road, [Tile.Road, Tile.Rail, Tile.Boardwalk, Tile.GrownPath]);
  set(Ink.Roofs, [Tile.HouseWall, Tile.HouseRoof, Tile.RoofSlate, Tile.RoofThatch, Tile.BrickWall, Tile.StoneWall, Tile.Floor, Tile.FloorWood]);
  return k;
})();

/** A building's footprint in cells. */
export interface BuildingRect {
  cx: number;
  cy: number;
  w: number;
  h: number;
}

/** Props big and solid enough to be a house, a barn, an inn: what the chart inks as roofs. */
export function buildingRects(props: readonly { def: string; cx: number; cy: number; hidden: boolean }[], defs: Readonly<Record<string, { w: number; h: number; solid: boolean }>>): BuildingRect[] {
  const out: BuildingRect[] = [];
  for (const p of props) {
    const d = defs[p.def];
    if (d && d.solid && !p.hidden && d.w * d.h >= 20) out.push({ cx: p.cx, cy: p.cy, w: d.w, h: d.h });
  }
  return out;
}

export function mapStep(w: number, hh: number, indoor: boolean): number {
  return indoor ? Math.max(1, Math.ceil(Math.max(w, hh) / MAX_MAP_PX)) : OUT_STEP;
}

/** Stable per-pixel hash, 0..255. */
function hash(x: number, y: number): number {
  let v = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1);
  v = Math.imul(v ^ (v >>> 15), 0x85ebca6b);
  return (v ^ (v >>> 13)) & 255;
}

function shade(c: number, k: number): number {
  const f = (v: number): number => Math.max(0, Math.min(255, Math.round(v * k)));
  return packRgb(f(c & 255), f((c >>> 8) & 255), f((c >>> 16) & 255));
}

/**
 * Colour per map pixel from the tiles. 0 = nothing there. Indoors each block is its commonest
 * tile, in chart colours (`pal`). Outdoors it is the strongest ink, and the ground inks are then
 * smoothed (a pixel of heath alone in a meadow goes to meadow) so the chart reads in areas, the
 * way someone would shade it by hand; roads, water and roofs are never smoothed away.
 */
export function terrainPixels(
  tiles: ArrayLike<number>,
  w: number,
  hh: number,
  step: number,
  pal: Uint32Array,
  indoor = false,
  buildings: readonly BuildingRect[] = [],
): Uint32Array {
  const mw = Math.ceil(w / step);
  const mh = Math.ceil(hh / step);
  const out = new Uint32Array(mw * mh);
  const n = indoor ? TILE_COUNT : Ink.Count;
  const score = new Float32Array(n);
  const ink = new Uint8Array(mw * mh);
  // Houses are props standing on grass, not tiles: their roofs are counted in by area.
  const roofs = new Float32Array(indoor ? 0 : mw * mh);
  for (const b of indoor ? [] : buildings) {
    for (let my = Math.floor(b.cy / step); my <= Math.floor((b.cy + b.h - 1) / step) && my < mh; my++) {
      const oy = Math.min(b.cy + b.h, (my + 1) * step) - Math.max(b.cy, my * step);
      for (let mx = Math.floor(b.cx / step); mx <= Math.floor((b.cx + b.w - 1) / step) && mx < mw; mx++) {
        const ox = Math.min(b.cx + b.w, (mx + 1) * step) - Math.max(b.cx, mx * step);
        if (mx >= 0 && my >= 0 && ox > 0 && oy > 0) roofs[my * mw + mx] += ox * oy;
      }
    }
  }
  for (let my = 0; my < mh; my++) {
    for (let mx = 0; mx < mw; mx++) {
      const cx0 = mx * step;
      const cy0 = my * step;
      const cx1 = Math.min(w, cx0 + step);
      const cy1 = Math.min(hh, cy0 + step);
      score.fill(0);
      let best = 0;
      let bestScore = 0;
      for (let cy = cy0; cy < cy1; cy++) {
        for (let cx = cx0; cx < cx1; cx++) {
          const t = tiles[cy * w + cx];
          if (t >= TILE_COUNT || t === Tile.Void) continue;
          const k = indoor ? t : INK_OF[t];
          const sc = (score[k] += indoor ? 1 : INK_WEIGHT[k]);
          if (sc > bestScore) {
            bestScore = sc;
            best = k;
          }
        }
      }
      if (indoor) {
        if (best > 0) out[my * mw + mx] = best < pal.length ? pal[best] : UNKNOWN;
      } else {
        const r = roofs[my * mw + mx] * INK_WEIGHT[Ink.Roofs];
        ink[my * mw + mx] = best !== Ink.None && r > bestScore ? Ink.Roofs : best;
      }
    }
  }
  if (indoor) return out;

  const counts = new Uint8Array(Ink.Count);
  for (let y = 0, i = 0; y < mh; y++) {
    for (let x = 0; x < mw; x++, i++) {
      let k = ink[i];
      if (AREA(k)) {
        counts.fill(0);
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const xx = x + dx;
            const yy = y + dy;
            if (xx < 0 || yy < 0 || xx >= mw || yy >= mh) continue;
            const o = ink[yy * mw + xx];
            if (AREA(o)) counts[o]++;
          }
        }
        let most = counts[k];
        for (let c = Ink.Meadow; c <= Ink.Rock; c++) {
          if (counts[c] > most) {
            most = counts[c];
            k = c;
          }
        }
      }
      if (k === Ink.None) continue;
      let c = INK_COLOUR[k];
      // A little hand in it: trees stippled in the woods, a darker fleck now and then elsewhere.
      const hsh = hash(x, y);
      if (k === Ink.Woods) c = shade(c, hsh < 70 ? 0.78 : hsh > 230 ? 1.12 : 1);
      else if (AREA(k)) c = shade(c, hsh < 26 ? 0.9 : 1);
      out[i] = c;
    }
  }
  return out;
}

/** How much of each map pixel's ground has been seen, 0..1, from the zone's fog bits (`fc` cells a bit). */
export function seenFraction(fog: readonly number[], fc: number, fogW: number, fogH: number, w: number, hh: number, step: number): Float32Array {
  const mw = Math.ceil(w / step);
  const mh = Math.ceil(hh / step);
  const out = new Float32Array(mw * mh);
  for (let my = 0; my < mh; my++) {
    const by0 = Math.floor((my * step) / fc);
    const by1 = Math.min(fogH - 1, Math.floor((Math.min(hh, my * step + step) - 1) / fc));
    for (let mx = 0; mx < mw; mx++) {
      const bx0 = Math.floor((mx * step) / fc);
      const bx1 = Math.min(fogW - 1, Math.floor((Math.min(w, mx * step + step) - 1) / fc));
      let seen = 0;
      let all = 0;
      for (let by = by0; by <= by1; by++) {
        for (let bx = bx0; bx <= bx1; bx++) {
          const bit = by * fogW + bx;
          all++;
          if (((fog[bit >> 5] ?? 0) & (1 << (bit & 31))) !== 0) seen++;
        }
      }
      out[my * mw + mx] = all ? seen / all : 0;
    }
  }
  return out;
}

/** Value noise over a lattice `size` pixels apart, eased between the corners, 0..1. */
function valueNoise(x: number, y: number, size: number, salt: number): number {
  const gx = Math.floor(x / size);
  const gy = Math.floor(y / size);
  const fx = x / size - gx;
  const fy = y / size - gy;
  const ex = fx * fx * (3 - 2 * fx);
  const ey = fy * fy * (3 - 2 * fy);
  const c = (i: number, j: number): number => hash(gx + i + salt, gy + j - salt) / 255;
  const top = c(0, 0) + (c(1, 0) - c(0, 0)) * ex;
  const bot = c(0, 1) + (c(1, 1) - c(0, 1)) * ex;
  return top + (bot - top) * ey;
}

/** Drifting smoke, 0..1 in five flat shades: soft banks of it, but every pixel still one colour. */
function smoke(x: number, y: number): number {
  const v = valueNoise(x, y, 9, 17) * 0.65 + valueNoise(x, y, 4, 91) * 0.35;
  return Math.round(Math.max(0, Math.min(1, (v - 0.2) / 0.6)) * 4) / 4;
}

const SMOKE_LO = [18, 15, 26];
const SMOKE_HI = [36, 31, 46];

/**
 * Fold the fog into the chart. Indoors: seen ground or nothing (the cellar's shape is the find).
 * Outdoors: seen ground shows, unseen ground is smoke with no hint of what is under it, and the
 * border between is two chunky steps of thinning smoke, so the edge is soft but the pixels are not.
 */
export function composeMap(terrain: Uint32Array, seen: Float32Array, mw: number, mh: number, indoor: boolean, out: Uint32Array): void {
  for (let y = 0, i = 0; y < mh; y++) {
    for (let x = 0; x < mw; x++, i++) {
      const t = terrain[i];
      if (t === 0) {
        out[i] = 0;
        continue;
      }
      if (indoor) {
        out[i] = seen[i] > 0 ? t : 0;
        continue;
      }
      // A 3x3 blur of the seen fraction, the centre counting double.
      let sum = seen[i] * 4;
      let wsum = 4;
      for (let dy = -1; dy <= 1; dy++) {
        const yy = y + dy;
        if (yy < 0 || yy >= mh) continue;
        for (let dx = -1; dx <= 1; dx++) {
          if (dx === 0 && dy === 0) continue;
          const xx = x + dx;
          if (xx < 0 || xx >= mw) continue;
          const k = dx === 0 || dy === 0 ? 2 : 1;
          sum += seen[yy * mw + xx] * k;
          wsum += k;
        }
      }
      const v = Math.max(seen[i], sum / wsum);
      // Four steps: smoke, thick haze, thin haze, clear. A little dither on the boundaries.
      const d = (hash(x, y) / 255 - 0.5) * 0.12;
      const lvl = v + d < 0.12 ? 0 : v + d < 0.4 ? 1 : v + d < 0.7 ? 2 : 3;
      const n = smoke(x, y);
      const sr = SMOKE_LO[0] + (SMOKE_HI[0] - SMOKE_LO[0]) * n;
      const sg = SMOKE_LO[1] + (SMOKE_HI[1] - SMOKE_LO[1]) * n;
      const sb = SMOKE_LO[2] + (SMOKE_HI[2] - SMOKE_LO[2]) * n;
      const f = lvl / 3;
      const tr = t & 255;
      const tg = (t >>> 8) & 255;
      const tb = (t >>> 16) & 255;
      out[i] = packRgb(Math.round(sr + (tr - sr) * f), Math.round(sg + (tg - sg) * f), Math.round(sb + (tb - sb) * f));
    }
  }
}

/**
 * A tile colour made to look inked on a kept chart: a little greyed, a little warmed, and cut to
 * a coarse ramp so neighbouring greens settle into a few shades.
 */
export function chartColour(r: number, g: number, b: number): number {
  const grey = (r * 3 + g * 4 + b) / 8;
  const q = (v: number, warm: number): number => Math.max(0, Math.min(255, Math.round((v * 0.72 + grey * 0.18 + warm) / 12) * 12));
  return packRgb(q(r, 14), q(g, 9), q(b, 0));
}

export function chartPalette(rgbs: readonly ([number, number, number] | null)[]): Uint32Array {
  return Uint32Array.from(rgbs, (c) => (c ? chartColour(c[0], c[1], c[2]) : UNKNOWN));
}

// --- the view: zoom and pan, pure --------------------------------------------------------------

/** `ppm` is CSS pixels per map pixel; `ox`/`oy` is where the map's top-left sits in the stage. */
export interface MapView {
  ppm: number;
  ox: number;
  oy: number;
}

/** The largest whole scale at which the whole map fits the stage. */
export function fitPpm(stageW: number, stageH: number, mw: number, mh: number): number {
  return Math.max(1, Math.floor(Math.min(stageW / mw, stageH / mh)));
}

export function clampZoomIndex(i: number): number {
  return Math.max(0, Math.min(ZOOMS.length - 1, Math.round(i)));
}

/** Keep the map on the stage: centred when it is smaller, edge to edge when it is larger. */
export function clampPan(v: MapView, stageW: number, stageH: number, mw: number, mh: number): MapView {
  const axis = (o: number, stage: number, size: number): number =>
    size <= stage ? Math.round((stage - size) / 2) : Math.round(Math.max(stage - size, Math.min(0, o)));
  return { ppm: v.ppm, ox: axis(v.ox, stageW, mw * v.ppm), oy: axis(v.oy, stageH, mh * v.ppm) };
}

/** Change scale keeping the map point under stage point (sx, sy) where it is. */
export function zoomAt(v: MapView, ppm: number, sx: number, sy: number): MapView {
  const mx = (sx - v.ox) / v.ppm;
  const my = (sy - v.oy) / v.ppm;
  return { ppm, ox: Math.round(sx - mx * ppm), oy: Math.round(sy - my * ppm) };
}

/** Put map point (mx, my) in the middle of the stage. */
export function centreOn(ppm: number, mx: number, my: number, stageW: number, stageH: number): MapView {
  return { ppm, ox: Math.round(stageW / 2 - mx * ppm), oy: Math.round(stageH / 2 - my * ppm) };
}

/** A cheap fingerprint of the seen-bits, to tell whether the smoke needs folding in again. */
function fogSig(fog: readonly number[]): number {
  let s = fog.length;
  for (let i = 0; i < fog.length; i++) s = (Math.imul(s, 31) + (fog[i] | 0)) | 0;
  return s;
}

// --- the pane -----------------------------------------------------------------------------------

export class MapPane {
  readonly el: HTMLDivElement;
  private readonly stage: HTMLDivElement;
  private readonly wrap: HTMLDivElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly school: HTMLDivElement;
  private readonly setDot: (pos: string) => void;
  private readonly setTitle: (s: string) => void;
  private readonly setBox: (box: string) => void;

  /** The zone's chart must be worked out again (new zone, tiles changed). */
  private dirty = true;
  /** The view must be put back on Jane (the map was just opened). */
  private recentre = true;
  private zoneRef: object | null = null;
  private terrain: Uint32Array = new Uint32Array(0);
  /**
   * Charts already worked out, by the zone's saved state (a loaded game brings new ones, so a
   * stale chart can never outlive its world): opening the map again costs nothing.
   */
  private charts = new WeakMap<object, Uint32Array>();
  private tilesStale = false;
  private sinceTiles = 0;
  private img: ImageData | null = null;
  private lastFog = NaN;
  private sinceFog = 0;
  private mw = 1;
  private mh = 1;
  private step = 1;
  private indoor = false;
  private zoomIdx = OPEN_ZOOM_OUT;
  private view: MapView = { ppm: 1, ox: 0, oy: 0 };
  private stageW = 0;
  private stageH = 0;
  private playerMx = 0;
  private playerMy = 0;
  private drag: { id: number; x: number; y: number; ox: number; oy: number } | null = null;
  private paletteKey = "";
  private palette: Uint32Array = new Uint32Array(0);

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-map");
    const head = h("div", "jq-map-head", this.el);
    this.setTitle = textOf(h("div", "jq-map-title", head));
    const tools = h("div", "jq-map-tools", head);
    const btn = (label: string, title: string, fn: () => void): void => {
      const b = h("div", "jq-map-btn", tools, label);
      b.title = title;
      b.addEventListener("click", fn);
    };
    btn("-", "Zoom out (-)", () => this.zoomBy(-1));
    btn("+", "Zoom in (+)", () => this.zoomBy(1));
    btn("Jane", "Back to Jane (0)", () => this.centre());

    this.stage = h("div", "jq-map-stage", this.el);
    this.wrap = h("div", "jq-map-wrap", this.stage);
    this.canvas = h("canvas", "jq-map-canvas", this.wrap);
    this.school = h("div", "jq-map-school", this.wrap);
    this.school.hidden = true;
    drawSchoolMark(h("canvas", "jq-map-school-art", this.school));
    const dot = h("div", "jq-map-dot", this.wrap);
    this.setDot = cached<string>((pos) => {
      const [x, y] = pos.split(",");
      dot.style.left = `${x}%`;
      dot.style.top = `${y}%`;
    });
    this.setBox = cached<string>((box) => {
      const [ox, oy, ww, hh] = box.split(",");
      this.wrap.style.left = `${ox}px`;
      this.wrap.style.top = `${oy}px`;
      this.wrap.style.width = `${ww}px`;
      this.wrap.style.height = `${hh}px`;
    });

    // One zoom step per notch: a trackpad's stream of small deltas is summed and spent a step at a time.
    let wheel = 0;
    this.stage.addEventListener(
      "wheel",
      (ev) => {
        ev.preventDefault();
        wheel += ev.deltaMode === 0 ? ev.deltaY : ev.deltaY * 40;
        if (Math.abs(wheel) < 50) return;
        const r = this.stage.getBoundingClientRect();
        this.zoomBy(wheel < 0 ? 1 : -1, ev.clientX - r.left, ev.clientY - r.top);
        wheel = 0;
      },
      { passive: false },
    );
    this.stage.addEventListener("pointerdown", (ev) => {
      if (ev.button !== 0) return;
      this.stage.setPointerCapture(ev.pointerId);
      this.drag = { id: ev.pointerId, x: ev.clientX, y: ev.clientY, ox: this.view.ox, oy: this.view.oy };
      this.stage.classList.add("jq-dragging");
    });
    this.stage.addEventListener("pointermove", (ev) => {
      const d = this.drag;
      if (!d || d.id !== ev.pointerId) return;
      this.setView({ ppm: this.view.ppm, ox: d.ox + ev.clientX - d.x, oy: d.oy + ev.clientY - d.y });
    });
    const end = (ev: PointerEvent): void => {
      if (this.drag?.id !== ev.pointerId) return;
      this.drag = null;
      this.stage.classList.remove("jq-dragging");
    };
    this.stage.addEventListener("pointerup", end);
    this.stage.addEventListener("pointercancel", end);
    window.addEventListener("keydown", (ev) => {
      if (!this.visible() || ev.target instanceof HTMLInputElement) return;
      if (ev.code === "Equal" || ev.code === "NumpadAdd") this.zoomBy(1);
      else if (ev.code === "Minus" || ev.code === "NumpadSubtract") this.zoomBy(-1);
      else if (ev.code === "Digit0" || ev.code === "Numpad0" || ev.code === "Home") this.centre();
      else return;
      ev.preventDefault();
    });
  }

  /** The window has just opened on this tab: fold the smoke in afresh and find her on it. */
  invalidate(): void {
    this.lastFog = NaN;
    this.recentre = true;
  }

  /** Tiles changed under the chart (a hedge grew, a wall fell). Keep the view where it is. */
  tilesChanged(): void {
    this.tilesStale = true;
  }

  /** Pad and arrow keys while the map is up: pan, and confirm steps the zoom in (then back out). */
  action(a: "up" | "down" | "left" | "right" | "confirm"): void {
    if (a === "confirm") {
      if (this.zoomIdx >= ZOOMS.length - 1) this.setZoom(0);
      else this.zoomBy(1);
      return;
    }
    const s = Math.max(this.view.ppm * 4, 24);
    const dx = a === "left" ? s : a === "right" ? -s : 0;
    const dy = a === "up" ? s : a === "down" ? -s : 0;
    this.setView({ ppm: this.view.ppm, ox: this.view.ox + dx, oy: this.view.oy + dy });
  }

  frame(sim: Sim): void {
    if (sim.zone !== this.zoneRef) {
      this.zoneRef = sim.zone;
      this.dirty = true;
      this.recentre = true;
      this.zoomIdx = sim.rt.bp.indoor ? 0 : OPEN_ZOOM_OUT;
    }
    // Changed tiles are worked in at most once a second: the county's chart is a few dozen ms.
    if (this.tilesStale && ++this.sinceTiles >= 60) {
      this.tilesStale = false;
      this.sinceTiles = 0;
      this.charts.delete(sim.zone);
      this.dirty = true;
    }
    if (this.dirty) {
      this.dirty = false;
      this.buildTerrain(sim);
      this.lastFog = NaN;
    }
    // The smoke is folded in again only when the seen-bits changed, and looked at twice a second.
    if (Number.isNaN(this.lastFog) || ++this.sinceFog >= 30) {
      this.sinceFog = 0;
      const sig = fogSig(sim.zone.fog);
      if (sig !== this.lastFog) {
        this.lastFog = sig;
        this.paint(sim);
      }
    }

    const cx = Math.floor(sim.player.x / CELL);
    const cy = Math.floor(sim.player.y / CELL);
    this.playerMx = (cx + 0.5) / this.step;
    this.playerMy = (cy + 0.5) / this.step;
    this.setDot(`${(((cx + 0.5) / (this.mw * this.step)) * 100).toFixed(2)},${(((cy + 0.5) / (this.mh * this.step)) * 100).toFixed(2)}`);

    const sw = this.stage.clientWidth;
    const sh = this.stage.clientHeight;
    if (sw > 0 && sh > 0 && (sw !== this.stageW || sh !== this.stageH || this.recentre)) {
      this.stageW = sw;
      this.stageH = sh;
      if (this.recentre) {
        this.recentre = false;
        this.centre();
      } else this.setZoom(this.zoomIdx);
    }
  }

  private visible(): boolean {
    return this.el.isConnected && !this.el.closest("[hidden]");
  }

  private zoomBy(d: number, sx = this.stageW / 2, sy = this.stageH / 2): void {
    this.setZoom(this.zoomIdx + d, sx, sy);
  }

  private setZoom(i: number, sx = this.stageW / 2, sy = this.stageH / 2): void {
    this.zoomIdx = clampZoomIndex(i);
    const ppm = fitPpm(this.stageW, this.stageH, this.mw, this.mh) * ZOOMS[this.zoomIdx];
    this.setView(zoomAt(this.view, ppm, sx, sy));
  }

  private centre(): void {
    this.zoomIdx = clampZoomIndex(this.zoomIdx);
    const ppm = fitPpm(this.stageW, this.stageH, this.mw, this.mh) * ZOOMS[this.zoomIdx];
    this.setView(centreOn(ppm, this.playerMx, this.playerMy, this.stageW, this.stageH));
  }

  private setView(v: MapView): void {
    this.view = clampPan(v, this.stageW, this.stageH, this.mw, this.mh);
    this.setBox(`${this.view.ox},${this.view.oy},${this.mw * this.view.ppm},${this.mh * this.view.ppm}`);
  }

  private buildTerrain(sim: Sim): void {
    const g = sim.rt.grid;
    const bp = sim.rt.bp;
    this.setTitle(bp.name);
    this.indoor = bp.indoor;
    this.step = mapStep(g.w, g.h, bp.indoor);
    this.mw = Math.ceil(g.w / this.step);
    this.mh = Math.ceil(g.h / this.step);
    if (this.canvas.width !== this.mw) this.canvas.width = this.mw;
    if (this.canvas.height !== this.mh) this.canvas.height = this.mh;
    let chart = this.charts.get(sim.zone);
    if (!chart) {
      const pal = this.paletteFor(this.ctx.host.tileColors());
      chart = terrainPixels(g.tiles, g.w, g.h, this.step, pal, bp.indoor, buildingRects(sim.zone.props, sim.catalog.props));
      this.charts.set(sim.zone, chart);
    }
    this.terrain = chart;
    const c2 = this.canvas.getContext("2d");
    this.img = c2 ? c2.createImageData(this.mw, this.mh) : null;

    // The School's mark, and nothing else. Outdoors only: inside, she is somewhere else.
    const school = bp.marks.school_mouth;
    this.school.hidden = !school || bp.indoor;
    if (school) {
      this.school.style.left = `${(((school.cx + 0.5) / (this.mw * this.step)) * 100).toFixed(2)}%`;
      this.school.style.top = `${(((school.cy + 0.5) / (this.mh * this.step)) * 100).toFixed(2)}%`;
    }
    this.setView(this.view);
  }

  private paint(sim: Sim): void {
    const c2 = this.canvas.getContext("2d");
    if (!c2 || !this.img) return;
    const g = sim.rt.grid;
    const seen = seenFraction(sim.zone.fog, sim.rt.fogCell, sim.rt.fogW, sim.rt.fogH, g.w, g.h, this.step);
    composeMap(this.terrain, seen, this.mw, this.mh, this.indoor, new Uint32Array(this.img.data.buffer));
    c2.putImageData(this.img, 0, 0);
  }

  private paletteFor(colors: readonly string[]): Uint32Array {
    const key = colors.join("|");
    if (key === this.paletteKey) return this.palette;
    this.paletteKey = key;
    let probe: CanvasRenderingContext2D | null = null;
    this.palette = chartPalette(
      colors.map((css) => {
        const rgb = parseCssColor(css);
        if (rgb) return rgb;
        // Named colours, hsl(), ...: let the browser resolve them.
        probe ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
        if (!probe) return null;
        probe.clearRect(0, 0, 1, 1);
        probe.fillStyle = css;
        probe.fillRect(0, 0, 1, 1);
        const d = probe.getImageData(0, 0, 1, 1).data;
        return [d[0], d[1], d[2]] as [number, number, number];
      }),
    );
    return this.palette;
  }
}

/**
 * The School's mark: nothing else on the map looks like it. A black building with a bell tower
 * and one lit window, edged in bone so it shows on smoke and on ground. It is drawn at a fixed
 * size over the chart, so it reads the same at every zoom, and it is there from the first time
 * the map is opened: she always knows which way it lies.
 */
export const SCHOOL_MARK = { w: 18, h: 14 };
function drawSchoolMark(c: HTMLCanvasElement): void {
  c.width = SCHOOL_MARK.w;
  c.height = SCHOOL_MARK.h;
  const c2 = c.getContext("2d");
  if (!c2) return;
  const parts: [number, number, number, number][] = [
    [2, 6, 14, 6], // the body
    [4, 4, 4, 2], // a taller wing
    [11, 1, 3, 5], // the bell tower
    [3, 3, 1, 1], // chimneys
    [9, 4, 1, 1],
  ];
  c2.fillStyle = "#cfc8b8";
  for (const [x, y, w, hh] of parts) c2.fillRect(x - 1, y - 1, w + 2, hh + 2);
  c2.fillStyle = "#0c0912";
  for (const [x, y, w, hh] of parts) c2.fillRect(x, y, w, hh);
  c2.fillStyle = "#f0d048";
  c2.fillRect(12, 3, 1, 1);
}
