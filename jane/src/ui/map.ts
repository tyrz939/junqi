// Minimap tab. One pixel per cell, written once into ImageData when the window
// opens (the county is 640x384 cells: fine once, wasteful per frame), then
// scaled by CSS with `image-rendering: pixelated`. The player is a DOM dot with
// a CSS blink, so nothing is redrawn while you look at it.

import { CELL } from "@/sim/constants";
import { Tile } from "@/sim/grid";
import type { PlayerView as Sim } from "@/sim/sim";
import { fogSeen } from "@/sim/zones";
import type { Ctx } from "@/ui/ctx";
import { cached, h, textOf, u } from "@/ui/dom";
import { packRgb, parseCssColor } from "@/ui/format";

/** Room for the image inside the pane, in design pixels. */
const FIT_W = 304;
const FIT_H = 150;
/** The map image is never wider or taller than this many pixels. */
const MAX_MAP_PX = 900;

const UNKNOWN = packRgb(60, 50, 80);

export class MapPane {
  readonly el: HTMLDivElement;
  private readonly wrap: HTMLDivElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly setDot: (pos: string) => void;
  private readonly setTitle: (s: string) => void;
  private dirty = true;
  private paletteKey = "";
  private palette: Uint32Array = new Uint32Array(0);

  constructor(private readonly ctx: Ctx) {
    this.el = h("div", "jq-map");
    this.setTitle = textOf(h("div", "jq-map-title", this.el));
    const stage = h("div", "jq-map-stage", this.el);
    this.wrap = h("div", "jq-map-wrap", stage);
    this.canvas = h("canvas", "jq-map-canvas", this.wrap);
    const dot = h("div", "jq-map-dot", this.wrap);
    this.setDot = cached<string>((pos) => {
      const [x, y] = pos.split(",");
      dot.style.left = `${x}%`;
      dot.style.top = `${y}%`;
    });
  }

  invalidate(): void {
    this.dirty = true;
  }

  frame(sim: Sim): void {
    if (this.dirty) {
      this.dirty = false;
      this.build(sim);
    }
    const g = sim.rt.grid;
    const cx = Math.floor(sim.player.x / CELL);
    const cy = Math.floor(sim.player.y / CELL);
    this.setDot(`${(((cx + 0.5) / g.w) * 100).toFixed(2)},${(((cy + 0.5) / g.h) * 100).toFixed(2)}`);
  }

  private build(sim: Sim): void {
    const g = sim.rt.grid;
    const w = g.w;
    const hh = g.h;
    this.setTitle(sim.rt.bp.name);
    const scale = Math.min(FIT_W / w, FIT_H / hh);
    this.wrap.style.width = u(w * scale);
    this.wrap.style.height = u(hh * scale);
    // One pixel a cell is right for a cellar and 29 MB of image for the county. Past 900 cells
    // across, one pixel stands for a block of cells, and says the most useful thing in it.
    const step = Math.max(1, Math.ceil(Math.max(w, hh) / MAX_MAP_PX));
    const mw = Math.ceil(w / step);
    const mh = Math.ceil(hh / step);
    if (this.canvas.width !== mw) this.canvas.width = mw;
    if (this.canvas.height !== mh) this.canvas.height = mh;
    const c2 = this.canvas.getContext("2d");
    if (!c2) return;

    const pal = this.paletteFor(this.ctx.host.tileColors());
    const img = c2.createImageData(mw, mh);
    const px = new Uint32Array(img.data.buffer);
    const tiles = g.tiles;
    const indoor = sim.rt.bp.indoor;
    const fog = sim.zone.fog;
    const fogW = sim.rt.fogW;
    for (let my = 0, i = 0; my < mh; my++) {
      for (let mx = 0; mx < mw; mx++, i++) {
        const cx = mx * step;
        const cy = my * step;
        if (indoor && !fogSeen(fog, fogW, cx >> 1, cy >> 1)) continue; // stays transparent: unexplored
        let t = tiles[cy * w + cx];
        if (step > 1) {
          // A road four cells wide must not vanish between samples, nor a river bank flicker:
          // paved ground wins the block, then water, then whatever the corner was.
          let best = 0;
          for (let oy = 0; oy < step && cy + oy < hh; oy++) {
            for (let ox = 0; ox < step && cx + ox < w; ox++) {
              const here = tiles[(cy + oy) * w + cx + ox];
              const rank = here === Tile.Road || here === Tile.Cobble || here === Tile.Rail ? 3 : here === Tile.Water ? 2 : 0;
              if (rank > best) {
                best = rank;
                t = here;
              }
            }
          }
        }
        px[i] = t < pal.length ? pal[t] : UNKNOWN;
      }
    }
    c2.putImageData(img, 0, 0);

    // Ways out. Sized in cells so they stay readable on the county and do not swamp a cellar.
    const m = Math.max(0, Math.round(mw / 160));
    for (const p of sim.zone.props) {
      if (!p.to || p.hidden) continue;
      if (indoor && !fogSeen(fog, fogW, p.cx >> 1, p.cy >> 1)) continue;
      const x = Math.floor(p.cx / step);
      const y = Math.floor(p.cy / step);
      c2.fillStyle = "#120e18";
      c2.fillRect(x - m - 1, y - m - 1, 2 * m + 3, 2 * m + 3);
      c2.fillStyle = "#e6b450";
      c2.fillRect(x - m, y - m, 2 * m + 1, 2 * m + 1);
    }
  }

  private paletteFor(colors: readonly string[]): Uint32Array {
    const key = colors.join("|");
    if (key === this.paletteKey) return this.palette;
    this.paletteKey = key;
    const out = new Uint32Array(colors.length);
    let probe: CanvasRenderingContext2D | null = null;
    colors.forEach((css, i) => {
      let rgb = parseCssColor(css);
      if (!rgb) {
        // Named colours, hsl(), ...: let the browser resolve them.
        probe ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
        if (probe) {
          probe.clearRect(0, 0, 1, 1);
          probe.fillStyle = css;
          probe.fillRect(0, 0, 1, 1);
          const d = probe.getImageData(0, 0, 1, 1).data;
          rgb = [d[0], d[1], d[2]];
        }
      }
      out[i] = rgb ? packRgb(rgb[0], rgb[1], rgb[2]) : UNKNOWN;
    });
    this.palette = out;
    return out;
  }
}
