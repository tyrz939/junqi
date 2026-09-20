// One lighting pass, one cached surface.
//
// 2020: allocate a view-sized surface every frame, clear it to a fixed yellow,
// bm_subtract each light sprite out of it, bm_subtract the surface off the scene,
// free it. The day clock never touched the colour; only lamp posts read it.
//
// Here: a lightmap the size of the low-res view, allocated once. Clear it to the
// ambient colour (the clock outdoors, the zone's ambient indoors), ADD each light
// as a pre-rendered radial sprite, then MULTIPLY the lightmap over the scene.
// Multiply is the well-behaved cousin of subtract: darkness scales colour instead
// of clipping it, coloured lights tint instead of washing out, and ambient = white
// is an exact no-op, so daytime costs one drawImage.

export type Light = { x: number; y: number; radius: number; color: string; intensity: number };

type Rgb = [number, number, number];

const DAY: Rgb = [255, 255, 255];
const SUNSET: Rgb = [255, 196, 150];
const DUSK: Rgb = [150, 120, 170];
const NIGHT: Rgb = [58, 66, 116];
const DAWN: Rgb = [230, 190, 190];

/** Hour -> ambient colour. Keyframes; linear between them. 17:00 start lands in the sunset. */
const KEYS: [number, Rgb][] = [
  [0, NIGHT],
  [4.5, NIGHT],
  [6, DAWN],
  [7.5, DAY],
  [16.5, DAY],
  [18, SUNSET],
  [19.5, DUSK],
  [21, NIGHT],
  [24, NIGHT],
];

export function ambientForHour(hour: number): Rgb {
  for (let i = 0; i + 1 < KEYS.length; i++) {
    const [h0, c0] = KEYS[i];
    const [h1, c1] = KEYS[i + 1];
    if (hour >= h0 && hour <= h1) {
      const k = (hour - h0) / (h1 - h0 || 1);
      return [c0[0] + (c1[0] - c0[0]) * k, c0[1] + (c1[1] - c0[1]) * k, c0[2] + (c1[2] - c0[2]) * k];
    }
  }
  return NIGHT;
}

/** Lamp posts burn from 18:30 to 06:30, the one thing 2020's clock did drive. */
export function isNight(hour: number): boolean {
  return hour > 18.5 || hour < 6.5;
}

export class Lighting {
  private readonly map = document.createElement("canvas");
  private readonly ctx = this.map.getContext("2d")!;
  private readonly sprites = new Map<string, HTMLCanvasElement>();

  resize(w: number, h: number): void {
    if (this.map.width === w && this.map.height === h) return;
    this.map.width = w;
    this.map.height = h;
  }

  /** Radial falloff sprite, cached per (radius, colour). Radii are bucketed to 4 px. */
  private sprite(radius: number, color: string): HTMLCanvasElement {
    const r = Math.max(4, Math.round(radius / 4) * 4);
    const key = `${r}|${color}`;
    let c = this.sprites.get(key);
    if (c) return c;
    c = document.createElement("canvas");
    c.width = r * 2;
    c.height = r * 2;
    const g = c.getContext("2d")!;
    const grad = g.createRadialGradient(r, r, 0, r, r, r);
    // A long soft tail: light that reaches further than it illuminates. The last third
    // barely lifts the dark, which is what makes the edge of a lamp's pool feel unsafe.
    grad.addColorStop(0, color);
    grad.addColorStop(0.25, color + "d0");
    grad.addColorStop(0.5, color + "80");
    grad.addColorStop(0.75, color + "38");
    grad.addColorStop(1, color + "00");
    g.fillStyle = grad;
    g.fillRect(0, 0, r * 2, r * 2);
    this.sprites.set(key, c);
    return c;
  }

  /**
   * `ambient` 0..255 per channel. Lights are in view pixels. When the ambient is
   * full white and there are no lights the pass is skipped entirely.
   */
  apply(target: CanvasRenderingContext2D, ambient: Rgb, lights: readonly Light[]): void {
    const flat = ambient[0] >= 254 && ambient[1] >= 254 && ambient[2] >= 254;
    if (flat) return;
    const { ctx, map } = this;
    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
    ctx.fillStyle = `rgb(${ambient[0] | 0},${ambient[1] | 0},${ambient[2] | 0})`;
    ctx.fillRect(0, 0, map.width, map.height);
    ctx.globalCompositeOperation = "lighter";
    for (const l of lights) {
      const s = this.sprite(l.radius, l.color);
      ctx.globalAlpha = Math.max(0, Math.min(1, l.intensity));
      ctx.drawImage(s, Math.round(l.x - s.width / 2), Math.round(l.y - s.height / 2));
    }
    ctx.globalAlpha = 1;
    target.globalCompositeOperation = "multiply";
    target.drawImage(map, 0, 0);
    target.globalCompositeOperation = "source-over";
  }
}
