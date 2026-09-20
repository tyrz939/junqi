// Rasterises the text-grid sprites once at boot. Each sheet becomes one canvas;
// every frame is also stored mirrored, so "side" serves both east and west
// without a per-draw transform (drawImage with a negative scale breaks pixel
// snapping on some browsers).

import type { SpriteSheet, SpriteSrc } from "@/art/types";

export type Frame = { x: number; y: number };
export type Sprite = {
  canvas: HTMLCanvasElement;
  w: number;
  h: number;
  ax: number;
  ay: number;
  frames: Record<string, Frame>;
  mirrored: Record<string, Frame>;
};

export type Atlas = Record<string, Sprite>;

function parseColor(hex: string): [number, number, number, number] {
  const h = hex.replace("#", "");
  const n = parseInt(h.length === 3 ? h.replace(/(.)/g, "$1$1") : h.slice(0, 6), 16);
  const a = h.length === 8 ? parseInt(h.slice(6, 8), 16) : 255;
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255, a];
}

function rasterise(src: SpriteSrc): Sprite {
  const names = Object.keys(src.frames);
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, src.w * names.length);
  canvas.height = src.h * 2;
  const ctx = canvas.getContext("2d")!;
  const image = ctx.createImageData(canvas.width, canvas.height);
  const colors = new Map<string, [number, number, number, number]>();
  for (const [ch, hex] of Object.entries(src.palette)) colors.set(ch, parseColor(hex));
  const frames: Record<string, Frame> = {};
  const mirrored: Record<string, Frame> = {};
  names.forEach((name, n) => {
    frames[name] = { x: n * src.w, y: 0 };
    mirrored[name] = { x: n * src.w, y: src.h };
    const rows = src.frames[name];
    for (let y = 0; y < src.h; y++) {
      const row = rows[y] ?? "";
      for (let x = 0; x < src.w; x++) {
        const c = colors.get(row[x] ?? ".");
        if (!c) continue;
        for (const [px, py] of [
          [n * src.w + x, y],
          [n * src.w + (src.w - 1 - x), src.h + y],
        ]) {
          const i = (py * canvas.width + px) * 4;
          image.data[i] = c[0];
          image.data[i + 1] = c[1];
          image.data[i + 2] = c[2];
          image.data[i + 3] = c[3];
        }
      }
    }
  });
  ctx.putImageData(image, 0, 0);
  return { canvas, w: src.w, h: src.h, ax: src.ax, ay: src.ay, frames, mirrored };
}

export function buildAtlas(...sheets: SpriteSheet[]): Atlas {
  const atlas: Atlas = {};
  for (const sheet of sheets) for (const [id, src] of Object.entries(sheet)) atlas[id] = rasterise(src);
  return atlas;
}

/** Draw a frame with its anchor on (x, y). Falls back through `fallbacks` if a frame is absent. */
export function drawSprite(
  ctx: CanvasRenderingContext2D,
  s: Sprite,
  frame: string,
  x: number,
  y: number,
  mirror = false,
  fallback = "base",
): void {
  const table = mirror ? s.mirrored : s.frames;
  const f = table[frame] ?? table[fallback] ?? table[Object.keys(table)[0]];
  if (!f) return;
  const ax = mirror ? s.w - s.ax : s.ax;
  ctx.drawImage(s.canvas, f.x, f.y, s.w, s.h, Math.round(x - ax), Math.round(y - s.ay), s.w, s.h);
}

const iconCache = new Map<string, string>();

/** 16x16 icon at 2x as a data URL, for the DOM UI. */
export function iconDataUrl(atlas: Atlas, id: string): string {
  const hit = iconCache.get(id);
  if (hit) return hit;
  const s = atlas[id];
  const canvas = document.createElement("canvas");
  canvas.width = 32;
  canvas.height = 32;
  const ctx = canvas.getContext("2d")!;
  ctx.imageSmoothingEnabled = false;
  if (s) {
    const f = s.frames.base ?? Object.values(s.frames)[0];
    ctx.drawImage(s.canvas, f.x, f.y, s.w, s.h, 0, 0, 32, 32);
  } else {
    ctx.fillStyle = "#c8403c";
    ctx.fillRect(8, 8, 16, 16);
  }
  const url = canvas.toDataURL();
  iconCache.set(id, url);
  return url;
}
