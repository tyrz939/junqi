// The things that stand up out of the ground and are drawn bigger than their one sim cell:
// trees, shrubs, stone piles. The sim cell stays one cell (it is what blocks feet); the sprite is
// the size a 2D Zelda would draw it, trunk on the cell and canopy over the neighbours, and it is
// y-sorted with units so she walks behind a wood's north edge and in front of its south edge.
//
// Painted procedurally, once, into small canvases: a canopy is a heap of round leaf clumps each lit
// from the top-left, with a dark crescent under every clump and an outline round the whole, which
// is how Minish Cap and Link's Awakening draw a tree. Seeded, so the same variant is the same tree
// on every machine and every run.

export type Spr = { c: HTMLCanvasElement; w: number; h: number; ax: number; ay: number };

type Tones = { o: string; d: string; m: string; l: string; h: string };

/** Leaf palettes: an ordinary wood, a drier olive one, the dark wet-wood green. */
export const LEAVES: Tones[] = [
  { o: "#132719", d: "#1f4a2d", m: "#2d6a39", l: "#468a42", h: "#6ea94f" },
  { o: "#1c2715", d: "#33482a", m: "#4a6634", l: "#688440", h: "#8ea456" },
  { o: "#0f211b", d: "#1a3a30", m: "#24543e", l: "#35704a", h: "#51905a" },
];
const PINE: Tones = { o: "#0f1f18", d: "#173a2c", m: "#22513a", l: "#316a45", h: "#4b8653" };
const BUSH: Tones[] = [
  { o: "#15301c", d: "#24562f", m: "#347a3b", l: "#4f9a47", h: "#7cbf5c" },
  { o: "#1b2d18", d: "#3a5a2a", m: "#527a34", l: "#6d9640", h: "#98b85a" },
];
const BARK = { o: "#24180f", d: "#45301f", m: "#6a4a2e", l: "#8e6a44" };
const STONE = { o: "#26221e", d: "#4c4842", m: "#6c6860", l: "#8e8a80", h: "#b4b0a4" };

function rng(seed: number): () => number {
  let s = seed >>> 0 || 1;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A tiny indexed image: colour strings per pixel, rasterised at the end. */
class Pix {
  readonly px: (string | null)[];
  constructor(
    readonly w: number,
    readonly h: number,
  ) {
    this.px = new Array(w * h).fill(null);
  }
  get(x: number, y: number): string | null {
    return x < 0 || y < 0 || x >= this.w || y >= this.h ? null : this.px[y * this.w + x];
  }
  set(x: number, y: number, c: string): void {
    if (x >= 0 && y >= 0 && x < this.w && y < this.h) this.px[y * this.w + x] = c;
  }
  /** Outline every empty pixel that touches a filled one (4-way). Pixels in `skip` rows are left alone. */
  outline(c: string, fromY = 0, toY = this.h): void {
    const add: number[] = [];
    for (let y = fromY; y < toY; y++) {
      for (let x = 0; x < this.w; x++) {
        if (this.get(x, y) !== null) continue;
        if (this.get(x - 1, y) || this.get(x + 1, y) || this.get(x, y - 1) || this.get(x, y + 1)) add.push(y * this.w + x);
      }
    }
    for (const i of add) this.px[i] = c;
  }
  canvas(): HTMLCanvasElement {
    const c = document.createElement("canvas");
    c.width = this.w;
    c.height = this.h;
    const g = c.getContext("2d")!;
    for (let y = 0; y < this.h; y++) {
      let x = 0;
      while (x < this.w) {
        const col = this.px[y * this.w + x];
        let n = 1;
        while (x + n < this.w && this.px[y * this.w + x + n] === col) n++;
        if (col) {
          g.fillStyle = col;
          g.fillRect(x, y, n, 1);
        }
        x += n;
      }
    }
    return c;
  }
}

type Puff = { x: number; y: number; r: number };

/**
 * A heap of leaf clumps inside the ellipse (cx, cy, rx, ry). Each pixel belongs to the front-most
 * clump over it (clumps are sorted top to bottom, so the lower ones are in front) and takes its tone
 * from where it sits in that clump and in the whole crown: lit top-left, dark underneath.
 */
function crown(p: Pix, cx: number, cy: number, rx: number, ry: number, n: number, t: Tones, r: () => number, rim = true, scale = 1): void {
  const puffs: Puff[] = [{ x: cx - rx * 0.1, y: cy - ry * 0.15, r: Math.min(rx, ry) * 0.62 * scale }];
  const a0 = r() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = a0 + (i / n) * Math.PI * 2 + (r() - 0.5) * 0.5;
    const k = 0.55 + r() * 0.12;
    puffs.push({ x: cx + Math.cos(a) * rx * k, y: cy + Math.sin(a) * ry * k, r: Math.min(rx, ry) * (0.4 + r() * 0.12) * scale });
  }
  puffs.sort((a, b) => a.y - b.y);
  const owner = new Int16Array(p.w * p.h).fill(-1);
  for (let i = 0; i < puffs.length; i++) {
    const q = puffs[i];
    for (let y = Math.floor(q.y - q.r - 1); y <= q.y + q.r + 1; y++) {
      for (let x = Math.floor(q.x - q.r - 1); x <= q.x + q.r + 1; x++) {
        if (x < 0 || y < 0 || x >= p.w || y >= p.h) continue;
        const dx = x + 0.5 - q.x;
        const dy = y + 0.5 - q.y;
        if (dx * dx + dy * dy <= q.r * q.r) owner[y * p.w + x] = i;
      }
    }
  }
  for (let y = 0; y < p.h; y++) {
    for (let x = 0; x < p.w; x++) {
      const i = owner[y * p.w + x];
      if (i < 0) continue;
      const q = puffs[i];
      const lx = (x + 0.5 - q.x) / q.r;
      const ly = (y + 0.5 - q.y) / q.r;
      const local = -(lx * 0.55 + ly * 0.85);
      const global = -((y - cy) / ry) * 0.5 - ((x - cx) / rx) * 0.18;
      let v = local * 0.62 + global + (r() - 0.5) * 0.16;
      // The crescent under each clump is what makes a crown read as clumps and not a disc.
      if (rim && lx * lx + ly * ly > 0.72 && ly > 0.25) v -= 0.7;
      p.set(x, y, v > 0.66 ? t.h : v > 0.22 ? t.l : v > -0.28 ? t.m : t.d);
    }
  }
}

function trunk(p: Pix, cx: number, top: number, bottom: number, w: number): void {
  const x0 = Math.round(cx - w / 2);
  for (let y = top; y <= bottom; y++) {
    const flare = y >= bottom - 1 ? (y === bottom ? 2 : 1) : 0;
    for (let x = x0 - flare; x < x0 + w + flare; x++) {
      const edgeL = x === x0 - flare;
      const edgeR = x === x0 + w + flare - 1;
      p.set(x, y, edgeL ? BARK.o : edgeR ? BARK.o : x === x0 - flare + 1 ? BARK.l : x >= x0 + w - 1 + flare - 1 ? BARK.d : BARK.m);
    }
  }
  // A knot, and the shade the crown throws down the trunk.
  p.set(x0 + 1, top + 3, BARK.d);
  for (let x = x0; x < x0 + w; x++) {
    p.set(x, top, BARK.d);
    p.set(x, top + 1, BARK.d);
  }
}

/** A broadleaf. Large: 32 x 40; medium: 24 x 32. Anchor: bottom centre of the trunk. */
export function broadleaf(seed: number, large: boolean, t: Tones): Spr {
  const r = rng(seed);
  const w = large ? 32 : 24;
  const h = large ? 40 : 32;
  const p = new Pix(w, h);
  const cx = w / 2;
  trunk(p, cx, large ? 22 : 17, h - 1, large ? 6 : 4);
  const crownP = new Pix(w, h);
  crown(crownP, cx + (r() - 0.5) * 2, large ? 14 : 11, large ? 14.5 : 10.5, large ? 12.5 : 9.5, large ? 9 : 7, t, r);
  crownP.outline(t.o);
  for (let i = 0; i < w * h; i++) if (crownP.px[i]) p.px[i] = crownP.px[i];
  // The outline under the trunk's feet is the ground's business, not the tree's.
  p.outline(BARK.o, h - 12, h);
  return { c: p.canvas(), w, h, ax: w / 2, ay: h };
}

/** A conifer: stacked drooping tiers. 22 x 40. */
export function pine(seed: number): Spr {
  const r = rng(seed);
  const w = 22;
  const h = 40;
  const p = new Pix(w, h);
  const cx = w / 2;
  trunk(p, cx, 30, h - 1, 4);
  const tiers = 4;
  const tops = [1, 8, 15, 22];
  const halfs = [4.5, 6.5, 8.5, 10.2];
  const tall = [9, 10, 10, 10];
  for (let k = 0; k < tiers; k++) {
    for (let y = 0; y < tall[k]; y++) {
      const yy = tops[k] + y;
      const f = (y + 1) / tall[k];
      const half = halfs[k] * Math.pow(f, 0.8);
      for (let x = Math.floor(cx - half); x < cx + half; x++) {
        const lx = (x + 0.5 - cx) / Math.max(1, half);
        const v = -lx * 0.7 + (1 - f) * 0.5 - (f > 0.8 ? 0.6 : 0) + (r() - 0.5) * 0.25;
        p.set(x, yy, v > 0.6 ? PINE.h : v > 0.15 ? PINE.l : v > -0.35 ? PINE.m : PINE.d);
      }
    }
  }
  p.outline(PINE.o);
  return { c: p.canvas(), w, h, ax: w / 2, ay: h };
}

/** A dead tree: a grey trunk and a few bare limbs. 22 x 34. */
export function deadTree(seed: number): Spr {
  const r = rng(seed);
  const w = 22;
  const h = 34;
  const p = new Pix(w, h);
  const cx = w / 2;
  const wood = { o: "#1e1a18", d: "#3e3834", m: "#5e5650", l: "#7e766c" };
  for (let y = 8; y < h; y++) {
    const half = y >= h - 2 ? 3 : 2;
    for (let x = Math.floor(cx - half); x < cx + half; x++) p.set(x, y, x === Math.floor(cx - half) ? wood.l : x >= cx + half - 1 ? wood.d : wood.m);
  }
  const limb = (x: number, y: number, dx: number, len: number): void => {
    let fx = x;
    let fy = y;
    for (let i = 0; i < len; i++) {
      fx += dx * (0.7 + r() * 0.6);
      fy -= 0.6 + r() * 0.7;
      p.set(Math.round(fx), Math.round(fy), wood.m);
      if (r() < 0.25 && i > 2) limb(fx, fy, -dx * 0.5 + (r() - 0.5), 3);
    }
  };
  limb(cx - 1, 16, -1, 7);
  limb(cx + 1, 13, 1, 7);
  limb(cx, 9, (r() - 0.5) * 0.8, 6);
  limb(cx - 1, 22, -1, 4);
  p.outline(wood.o);
  return { c: p.canvas(), w, h, ax: w / 2, ay: h };
}

/** A shrub on one cell, a little wider than it: 12 x 12, bottom one pixel below the cell. */
export function bush(seed: number, t: Tones, berries: boolean): Spr {
  const r = rng(seed);
  const w = 12;
  const h = 12;
  const p = new Pix(w, h);
  crown(p, 6 + (r() - 0.5) * 0.6, 6.4, 4.4, 4, 6, t, r, true, 1.45);
  if (berries) {
    for (let i = 0; i < 3; i++) {
      const x = 3 + Math.floor(r() * 6);
      const y = 3 + Math.floor(r() * 5);
      if (p.get(x, y)) {
        p.set(x, y, "#c8403c");
        p.set(x + 1, y, "#7a2430");
      }
    }
  }
  p.outline(t.o);
  return { c: p.canvas(), w, h, ax: 6, ay: 11 };
}

/** A pile of two or three stones on one cell: 12 x 11. */
export function rocks(seed: number): Spr {
  const r = rng(seed);
  const w = 12;
  const h = 11;
  const p = new Pix(w, h);
  const stones = [
    { x: 4 + r() * 1.5, y: 6.2, rx: 3.6, ry: 3 },
    { x: 7.6 + r(), y: 6.8, rx: 3, ry: 2.6 },
  ];
  if (r() < 0.6) stones.push({ x: 5.5 + r() * 2, y: 4.2, rx: 2.4, ry: 2.1 });
  stones.sort((a, b) => a.y - b.y);
  for (const s of stones) {
    for (let y = Math.floor(s.y - s.ry - 1); y <= s.y + s.ry; y++) {
      for (let x = Math.floor(s.x - s.rx - 1); x <= s.x + s.rx; x++) {
        const dx = (x + 0.5 - s.x) / s.rx;
        const dy = (y + 0.5 - s.y) / s.ry;
        const d = dx * dx + dy * dy;
        if (d > 1) continue;
        const v = -(dx * 0.6 + dy * 0.8) + (r() - 0.5) * 0.2;
        p.set(x, y, d > 0.7 && dy > 0.2 ? STONE.d : v > 0.55 ? STONE.h : v > 0.05 ? STONE.l : v > -0.45 ? STONE.m : STONE.d);
      }
    }
  }
  p.outline(STONE.o);
  return { c: p.canvas(), w, h, ax: 6, ay: 10 };
}

export const BUSHES = BUSH;

/** A boulder, standing on one cell and overhanging it: 16 x 15. */
export function boulder(seed: number): Spr {
  const r = rng(seed);
  const w = 16;
  const h = 15;
  const p = new Pix(w, h);
  const cx = 8 + (r() - 0.5);
  const cy = 8;
  const rx = 6.8;
  const ry = 5.8;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const dx = (x + 0.5 - cx) / rx;
      const dy = (y + 0.5 - cy) / ry;
      // A flattened, slightly lumpy stone.
      const d = dx * dx + dy * dy * (dy < 0 ? 1.15 : 0.9) + Math.sin(x * 1.7 + seed) * 0.05;
      if (d > 1) continue;
      const v = -(dx * 0.6 + dy * 0.8) + (r() - 0.5) * 0.18;
      p.set(x, y, dy > 0.45 && d > 0.55 ? STONE.d : v > 0.6 ? STONE.h : v > 0.1 ? STONE.l : v > -0.4 ? STONE.m : STONE.d);
    }
  }
  // A crack or two.
  let x = Math.floor(cx - 2 + r() * 4);
  for (let y = Math.floor(cy - 3); y < cy + 2; y++) {
    if (p.get(x, y)) p.set(x, y, STONE.o);
    x += r() < 0.5 ? 0 : r() < 0.5 ? -1 : 1;
  }
  p.outline(STONE.o);
  return { c: p.canvas(), w, h, ax: 8, ay: 14 };
}
