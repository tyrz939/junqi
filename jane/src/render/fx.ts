// Spell effects: how a cast gathers, how a bolt travels, how it lands, what lingers, and what a
// status looks like on the unit wearing it. Every spell and every effect row has an entry here
// (test/art.test.ts): nothing falls back to a generic square.
//
// Everything is drawn with whole-pixel rectangles through a Painter, so the same code draws into
// the game's canvas and into a PNG from a node script: an effect can be
// looked at frame by frame without a browser. Colours are the game's: muted, cold, a little
// dirty. Fire is the warmest thing on screen and still not orange neon.
//
// Scale: a cell is 8 px and Jane is 18 px tall. A bolt is 3 to 5 px across, an impact about a
// cell, a blast the size of its splash (25 to 28 px). Timings are in frames at 60 Hz.

import type { School } from "@/sim/state";

export interface Painter {
  rect(x: number, y: number, w: number, h: number, color: string, alpha: number): void;
}

export type Light = { x: number; y: number; radius: number; color: string; intensity: number };

// --- looks ------------------------------------------------------------------------------------

/** The thing in flight. */
export type BoltLook = "frost" | "fire" | "venom" | "needle" | "spark" | "charge";
/** How it lands (also melee hits, keyed by spell). */
export type ImpactLook = "frost" | "fire" | "venom" | "needle" | "spark" | "blast" | "slash" | "bite" | "shock_grip" | "lash" | "none";
/** What lies on the ground afterwards, for kind=ground spells. */
export type GroundLook = "web" | "net" | "dust" | "charge";
/** What the caster's hands do. */
export type CastLook = "frost" | "fire" | "venom" | "spark" | "blast" | "throw" | "swing" | "repair" | "grow" | "none";
/** What a status looks like on the one wearing it. */
export type StatusLook =
  | "frost"
  | "burning"
  | "poison"
  | "stars"
  | "web"
  | "dust"
  | "jolt"
  | "shield"
  | "drain"
  | "glint"
  | "stone"
  | "ember_hands"
  | "frost_hands"
  | "spark_hands"
  | "thorn_hands"
  | "softened"
  /** Instant (duration 0): a single rising flicker when it lands, nothing to wear. */
  | "instant";

export type SpellFx = { cast: CastLook; bolt?: BoltLook; impact: ImpactLook; ground?: GroundLook };

/** Keyed by spell id. A new spell without an entry fails test/art.test.ts. */
export const SPELL_FX: Record<string, SpellFx> = {
  melee_player: { cast: "swing", impact: "slash" },
  melee: { cast: "swing", impact: "slash" },
  melee_fast: { cast: "swing", impact: "slash" },
  melee_stun: { cast: "swing", impact: "slash" },
  hand_bell: { cast: "throw", impact: "none", ground: "dust" },
  root_lash: { cast: "none", impact: "lash" },
  spider_bite: { cast: "none", impact: "bite" },
  icebolt: { cast: "frost", bolt: "frost", impact: "frost" },
  icebolt_ai: { cast: "frost", bolt: "frost", impact: "frost" },
  fireball: { cast: "fire", bolt: "fire", impact: "fire" },
  repair: { cast: "repair", impact: "none" },
  poisonbolt: { cast: "venom", bolt: "venom", impact: "venom" },
  poisonbolt_far: { cast: "venom", bolt: "venom", impact: "venom" },
  cactus_spray: { cast: "none", bolt: "needle", impact: "needle" },
  snake_ring: { cast: "venom", bolt: "venom", impact: "venom" },
  webshot: { cast: "throw", impact: "none", ground: "web" },
  net_throw: { cast: "throw", impact: "none", ground: "net" },
  scale_dust: { cast: "throw", impact: "none", ground: "dust" },
  charge_lob: { cast: "throw", impact: "none", ground: "charge" },
  explosion: { cast: "blast", bolt: "charge", impact: "blast" },
  grow: { cast: "grow", impact: "none" },
  spark: { cast: "spark", bolt: "spark", impact: "spark" },
  spark_ai: { cast: "spark", bolt: "spark", impact: "spark" },
  melee_shock: { cast: "swing", impact: "shock_grip" },
};

/** Keyed by effect id. A new effect without an entry fails test/art.test.ts. */
export const STATUS_FX: Record<string, StatusLook> = {
  chilled: "frost",
  burning: "burning",
  poisoned: "poison",
  spider_venom: "poison",
  stunned: "stars",
  webbed: "web",
  manashield: "shield",
  lifesteal: "drain",
  critical: "glint",
  stoneskin: "stone",
  firelash: "ember_hands",
  sparktongue: "spark_hands",
  winterbite: "frost_hands",
  stranglethorn: "thorn_hands",
  mana_25: "instant",
  staggered: "stars",
  dazzled: "stars",
  jolted: "jolt",
  dusted: "dust",
  softened: "softened",
};

// --- palette ----------------------------------------------------------------------------------

const C = {
  ice: "#a8d8ec",
  iceCore: "#e8f4fa",
  iceDeep: "#5a86a8",
  fire: "#e8803c",
  fireHot: "#f0c860",
  fireCore: "#fff0c8",
  ember: "#b84a34",
  smoke: "#3c3638",
  venom: "#86b848",
  venomDark: "#3c6030",
  venomLight: "#c8e088",
  needle: "#d8c8a0",
  spark: "#c4b8f4",
  sparkCore: "#f4f0ff",
  sparkDeep: "#6a5cb8",
  blast: "#f0c048",
  blastHot: "#fff0c0",
  debris: "#6e4a2c",
  scorch: "#1c1618",
  bone: "#e8e2d4",
  web: "#dcd8cc",
  rope: "#8a6e48",
  ropeDark: "#5a4630",
  dust: "#b8aac4",
  blood: "#6a2028",
  gold: "#f0d048",
  shield: "#7ab0e0",
  stone: "#8a8f98",
  leaf: "#78b048",
  repair: "#f0e0a0",
} as const;

/** The damage number and hit sparks, by school: kept here so hits and bolts agree. */
export const SCHOOL_COLOR: Record<School, string> = {
  heal: "#88d870",
  physical: "#ece6d8",
  frost: C.ice,
  fire: C.fire,
  nature: C.venom,
  blast: C.blast,
  shock: C.spark,
};

// --- particles --------------------------------------------------------------------------------

type Part = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Added to vy each frame (+ falls, - rises). */
  g: number;
  /** Velocity kept each frame. */
  drag: number;
  life: number;
  max: number;
  color: string;
  size: number;
  /** Colour it turns into in the last half of its life (smoke after flame), or null. */
  late: string | null;
  /** Ground marks are drawn under units and do not move. */
  ground: boolean;
};

/** A short line of pixels: a slash, a crack of lightning. Drawn for `life` frames. */
type Stroke = { pts: number[]; life: number; max: number; color: string; core: string | null };

/** An expanding ring: a blast front, a frost ring. */
type Ring = { x: number; y: number; r0: number; r1: number; life: number; max: number; color: string; squash: number };

export class Fx {
  private parts: Part[] = [];
  private strokes: Stroke[] = [];
  private rings: Ring[] = [];
  private seed: number;
  /** Lights wanted by effects this frame: the renderer collects them after `draw`. */
  readonly lights: Light[] = [];
  frame = 0;

  constructor(seed = 0x5eed) {
    this.seed = seed >>> 0 || 1;
  }

  private rnd(): number {
    this.seed = (Math.imul(this.seed, 1664525) + 1013904223) >>> 0;
    return this.seed / 4294967296;
  }

  private range(a: number, b: number): number {
    return a + (b - a) * this.rnd();
  }

  /** A new zone: nothing from the last one carries over. */
  clear(): void {
    this.parts.length = 0;
    this.strokes.length = 0;
    this.rings.length = 0;
    this.lights.length = 0;
  }

  get count(): number {
    return this.parts.length + this.strokes.length + this.rings.length;
  }

  private add(p: Partial<Part> & { x: number; y: number; color: string; life: number }): void {
    if (this.parts.length > 900) return;
    this.parts.push({ vx: 0, vy: 0, g: 0, drag: 0.94, size: 1, late: null, ground: false, max: p.life, ...p });
  }

  private spray(x: number, y: number, n: number, speed: [number, number], color: string, life: [number, number], extra: Partial<Part> = {}): void {
    for (let i = 0; i < n; i++) {
      const a = this.rnd() * Math.PI * 2;
      const s = this.range(speed[0], speed[1]);
      this.add({ x, y, vx: Math.cos(a) * s, vy: Math.sin(a) * s * 0.7, color, life: Math.round(this.range(life[0], life[1])), ...extra });
    }
  }

  private bolt(x0: number, y0: number, x1: number, y1: number, color: string, core: string | null, life: number, jag = 2): void {
    const pts: number[] = [x0, y0];
    const n = Math.max(2, Math.round(Math.hypot(x1 - x0, y1 - y0) / 3));
    for (let i = 1; i < n; i++) pts.push(x0 + ((x1 - x0) * i) / n + this.range(-jag, jag), y0 + ((y1 - y0) * i) / n + this.range(-jag, jag));
    pts.push(x1, y1);
    this.strokes.push({ pts, life, max: life, color, core });
  }

  // --- events -------------------------------------------------------------------------------

  /** A spell left someone's hands. `x, y` is the caster's feet, `facing` 0 east, 1 south, 2 west, 3 north. */
  cast(spellId: string, x: number, y: number, facing = 1): void {
    const look = SPELL_FX[spellId]?.cast ?? "none";
    const fdx = [1, 0, -1, 0][facing] ?? 0;
    const fdy = [0, 1, 0, -1][facing] ?? 1;
    // The hands are a little in front of her, at the waist.
    const hx = x + fdx * 6;
    const hy = y - 9 + fdy * 2;
    const gather = (color: string, core: string, n: number): void => {
      // Motes drawn IN to the hands: the tell that something is coming, a third of a second long.
      for (let i = 0; i < n; i++) {
        const a = (i / n) * Math.PI * 2 + this.rnd();
        const r = this.range(7, 10);
        const life = 14 + (i % 3) * 2;
        this.add({ x: hx + Math.cos(a) * r, y: hy + Math.sin(a) * r * 0.7, vx: (-Math.cos(a) * r) / life, vy: (-Math.sin(a) * r * 0.7) / life, drag: 1, color: i % 3 === 0 ? core : color, life });
      }
    };
    switch (look) {
      case "frost":
        gather(C.ice, C.iceCore, 8);
        break;
      case "fire":
        gather(C.fire, C.fireHot, 8);
        break;
      case "venom":
        gather(C.venom, C.venomLight, 6);
        break;
      case "spark":
        for (let i = 0; i < 3; i++) this.bolt(hx - 4 + i * 4, hy - 4, hx - 2 + i * 2, hy + 3, C.spark, null, 5, 1.5);
        break;
      case "blast":
        gather(C.blast, C.fireHot, 6);
        this.add({ x: hx, y: hy - 2, vy: -0.2, color: C.smoke, life: 30, size: 2 });
        break;
      case "throw":
        this.add({ x: hx, y: hy, vy: -0.6, g: 0.05, color: C.bone, life: 10 });
        break;
      case "repair": {
        // Where the hands are working, a cell ahead: a spray of bright filings and a couple of grey ones.
        const wx = x + fdx * 10;
        const wy = y - 4 + fdy * 8;
        this.spray(wx, wy, 9, [0.6, 1.3], C.repair, [10, 18], { g: 0.09 });
        this.spray(wx, wy, 4, [0.3, 0.7], C.stone, [14, 20], { g: 0.09 });
        this.strokes.push({ pts: [wx - 3, wy - 3, wx + 3, wy + 3], life: 5, max: 5, color: C.repair, core: "#ffffff" });
        this.lights.push({ x: wx, y: wy, radius: 16, color: C.repair, intensity: 0.6 });
        break;
      }
      case "grow": {
        // Green coming up out of the ground a cell ahead, as the thing in front of her is brought on.
        const wx = x + fdx * 10;
        const wy = y + fdy * 8;
        for (let i = 0; i < 12; i++) this.add({ x: wx + this.range(-6, 6), y: wy + this.range(-2, 2), vy: -this.range(0.2, 0.55), drag: 0.99, color: i % 3 ? C.leaf : C.venomLight, life: Math.round(this.range(26, 40)) });
        for (let i = 0; i < 4; i++) this.add({ x: wx - 4 + i * 3, y: wy, drag: 1, color: C.leaf, size: 2, life: 60, ground: true });
        break;
      }
      case "swing":
      case "none":
        break;
    }
    if (look === "fire" || look === "blast") this.lights.push({ x: hx, y: hy, radius: 20, color: C.fire, intensity: 0.6 });
  }

  /** A melee swing: a pale arc in front of the swinger, whether or not it connects. */
  swing(x: number, y: number, facing: number): void {
    // facing: 0 east, 1 south, 2 west, 3 north (FACING_DX/DY).
    const dx = [1, 0, -1, 0][facing] ?? 0;
    const dy = [0, 1, 0, -1][facing] ?? 1;
    const cx = x + dx * 7;
    const cy = y - 8 + dy * 6;
    const pts: number[] = [];
    // A short arc, 100 degrees, sweeping across the facing direction.
    const base = Math.atan2(dy, dx);
    for (let i = 0; i <= 6; i++) {
      const a = base - 0.9 + (i / 6) * 1.8;
      pts.push(cx + Math.cos(a) * 6 - dx * 3, cy + Math.sin(a) * 5 - dy * 3);
    }
    this.strokes.push({ pts, life: 7, max: 7, color: "#cfc8b8", core: null });
  }

  /** Where a bolt or a blow landed. */
  impact(spellId: string, x: number, y: number): void {
    const look = SPELL_FX[spellId]?.impact ?? "none";
    const cy = y - 6;
    switch (look) {
      case "frost":
        // Shards out, a thin rime ring on the ground, a little frost left on it.
        for (let i = 0; i < 8; i++) {
          const a = (i / 8) * Math.PI * 2 + this.rnd() * 0.4;
          this.add({ x, y: cy, vx: Math.cos(a) * 1.4, vy: Math.sin(a) * 1.0 - 0.2, g: 0.06, drag: 0.9, color: i % 2 ? C.iceCore : C.ice, life: 18 });
        }
        this.rings.push({ x, y, r0: 2, r1: 10, life: 14, max: 14, color: C.ice, squash: 0.55 });
        for (let i = 0; i < 6; i++) this.add({ x: x + this.range(-6, 6), y: y + this.range(-2, 2), drag: 1, color: C.iceCore, life: 50 + i * 4, ground: true });
        this.lights.push({ x, y: cy, radius: 22, color: C.ice, intensity: 0.8 });
        break;
      case "fire":
        // A filled flash that burns down from white to orange, then the ring and the embers.
        for (const [r, col, life] of [
          [4, C.fire, 12],
          [3, C.fireHot, 10],
          [2, C.fireCore, 8],
        ] as const) {
          for (let yy = -r; yy <= r; yy++) for (let xx = -r; xx <= r; xx++) if (xx * xx + yy * yy <= r * r + 1) this.add({ x: x + xx, y: cy + yy, drag: 1, color: col, life });
        }
        this.rings.push({ x, y: cy, r0: 3, r1: 8, life: 9, max: 9, color: C.fireHot, squash: 0.85 });
        this.spray(x, cy, 10, [0.6, 1.5], C.fire, [14, 24], { g: -0.03, late: C.ember, drag: 0.9 });
        for (let i = 0; i < 5; i++) this.add({ x: x + this.range(-3, 3), y: cy - 2, vy: -this.range(0.25, 0.45), drag: 0.99, color: C.smoke, size: 2, life: Math.round(this.range(30, 44)) });
        this.add({ x: x - 2, y: y - 1, drag: 1, color: C.scorch, size: 4, life: 90, ground: true });
        this.lights.push({ x, y: cy, radius: 36, color: C.fire, intensity: 1 });
        break;
      case "venom":
        // A wet splat: droplets thrown up and falling, and a green stain that lasts a moment.
        this.spray(x, cy, 7, [0.6, 1.3], C.venom, [16, 24], { g: 0.1, drag: 0.95 });
        this.add({ x: x - 2, y: y - 1, drag: 1, color: C.venomDark, size: 4, life: 70, ground: true });
        this.add({ x: x - 1, y: y - 1, drag: 1, color: C.venom, size: 2, life: 50, ground: true });
        break;
      case "needle":
        this.spray(x, cy, 2, [0.4, 0.8], C.needle, [6, 10], { g: 0.08 });
        break;
      case "spark":
        for (let i = 0; i < 4; i++) {
          const a = (i / 4) * Math.PI * 2 + this.rnd();
          this.bolt(x, cy, x + Math.cos(a) * 8, cy + Math.sin(a) * 6, C.spark, C.sparkCore, 6, 1.5);
        }
        this.spray(x, cy, 4, [0.8, 1.6], C.sparkCore, [6, 10]);
        this.lights.push({ x, y: cy, radius: 26, color: C.spark, intensity: 0.9 });
        break;
      case "blast":
        // The loud one: a flash, a front that runs out to the splash radius, earth and smoke.
        this.rings.push({ x, y: y - 3, r0: 3, r1: 26, life: 14, max: 14, color: C.blastHot, squash: 0.6 });
        this.rings.push({ x, y: y - 3, r0: 1, r1: 16, life: 10, max: 10, color: C.blast, squash: 0.6 });
        this.spray(x, y - 4, 14, [1.2, 2.6], C.debris, [18, 30], { g: 0.12, drag: 0.93 });
        this.spray(x, y - 4, 10, [0.8, 1.8], C.blast, [8, 14], { late: C.ember });
        for (let i = 0; i < 9; i++) this.add({ x: x + this.range(-10, 10), y: y - this.range(2, 8), vx: this.range(-0.2, 0.2), vy: -this.range(0.1, 0.35), drag: 0.99, color: C.smoke, size: 3, life: Math.round(this.range(50, 80)) });
        for (let i = 0; i < 5; i++) this.add({ x: x + this.range(-8, 6), y: y + this.range(-3, 1), drag: 1, color: C.scorch, size: 3, life: 180, ground: true });
        this.lights.push({ x, y: y - 4, radius: 64, color: C.blastHot, intensity: 1 });
        break;
      case "slash": {
        // A blow that lands: one hard diagonal stroke across the body, white at its heart, gone in
        // a tenth of a second, and a couple of flecks thrown off.
        const s = this.rnd() < 0.5 ? 1 : -1;
        this.strokes.push({ pts: [x - 5 * s, cy - 5, x + 5 * s, cy + 3], life: 6, max: 6, color: "#cfc8b8", core: "#ffffff" });
        this.spray(x, cy, 3, [0.6, 1.2], C.bone, [6, 10], { g: 0.1 });
        break;
      }
      case "bite": {
        // Two short fang marks closing, and a little dark blood.
        this.strokes.push({ pts: [x - 3, cy - 3, x - 2, cy + 1], life: 6, max: 6, color: C.bone, core: null });
        this.strokes.push({ pts: [x + 3, cy - 3, x + 2, cy + 1], life: 6, max: 6, color: C.bone, core: null });
        this.spray(x, cy + 2, 4, [0.4, 1.0], C.blood, [12, 18], { g: 0.1 });
        break;
      }
      case "lash": {
        // A root whips across: a dark green line with a hooked end, earth kicked up.
        this.strokes.push({ pts: [x - 7, cy + 4, x - 3, cy - 2, x + 3, cy - 3, x + 6, cy + 1], life: 7, max: 7, color: C.venomDark, core: C.debris });
        this.spray(x, y, 4, [0.3, 0.8], C.debris, [10, 16], { g: 0.1 });
        break;
      }
      case "shock_grip":
        for (let i = 0; i < 2; i++) this.bolt(x - 4, cy - 3 + i * 5, x + 4, cy - 1 + i * 4, C.spark, C.sparkCore, 5, 1.5);
        this.lights.push({ x, y: cy, radius: 18, color: C.spark, intensity: 0.7 });
        break;
      case "none":
        break;
    }
  }

  /** A damage number's worth of sparks, by school. Small: the impact already said most of it. */
  hit(school: School, x: number, y: number, crit: boolean): void {
    const color = SCHOOL_COLOR[school];
    this.spray(x, y - 6, crit ? 6 : 3, [0.5, 1.1], color, [10, 16], { g: 0.05 });
  }

  /** Someone died. What comes off them depends on what they were made of. */
  death(kind: "blood" | "oil" | "sap" | "stuffing" | "wax" | "ichor" | "bone" | "stone" | "ghost", x: number, y: number): void {
    const cy = y - 6;
    switch (kind) {
      case "ghost":
        for (let i = 0; i < 12; i++) this.add({ x: x + this.range(-6, 6), y: cy + this.range(-6, 4), vy: -this.range(0.2, 0.5), drag: 0.99, color: i % 3 ? "#9aa0b8" : "#cfd4e0", life: Math.round(this.range(30, 50)) });
        break;
      case "oil":
      case "stone":
        this.spray(x, cy, 10, [0.8, 1.8], kind === "oil" ? "#6a7078" : C.stone, [18, 28], { g: 0.12 });
        if (kind === "oil") this.spray(x, cy - 4, 5, [0.8, 1.4], C.blastHot, [6, 10]);
        for (let i = 0; i < 4; i++) this.add({ x: x + this.range(-4, 4), y: cy - 4, vy: -0.3, drag: 0.99, color: C.smoke, size: 2, life: 40 });
        break;
      case "bone":
        this.spray(x, cy, 8, [0.6, 1.4], C.bone, [16, 24], { g: 0.12 });
        break;
      case "stuffing":
      case "wax":
        this.spray(x, cy, 8, [0.5, 1.2], kind === "wax" ? "#d8c89a" : "#c8bea6", [18, 26], { g: 0.06 });
        break;
      case "sap":
      case "ichor":
        this.spray(x, cy, 8, [0.5, 1.2], kind === "sap" ? C.leaf : "#4a6038", [14, 22], { g: 0.1 });
        break;
      case "blood":
        this.spray(x, cy, 6, [0.4, 1.0], C.blood, [14, 22], { g: 0.1 });
        break;
    }
  }

  // --- things that exist in the sim ------------------------------------------------------------

  /**
   * A bolt in flight at (x, y) screen px (already lifted to hand height), moving (vx, vy) px per
   * tick. Leaves its trail in the particle list, so call once per drawn frame.
   */
  drawBolt(p: Painter, look: BoltLook, x: number, y: number, vx: number, vy: number, age: number, glow: number): void {
    const X = Math.round(x);
    const Y = Math.round(y);
    const sp = Math.hypot(vx, vy) || 1;
    const ux = vx / sp;
    const uy = vy / sp;
    switch (look) {
      case "frost": {
        // A shard: long along its flight, a bright point at the front, rime flaking off behind.
        for (let i = 0; i < 5; i++) p.rect(X - Math.round(ux * i), Y - Math.round(uy * i), 1, 1, i === 0 ? C.iceCore : i < 3 ? C.ice : C.iceDeep, 1 - i * 0.15);
        p.rect(X - 1 - Math.round(ux), Y - Math.round(uy), 3, 1, C.ice, 0.5);
        p.rect(X - Math.round(ux), Y - 1 - Math.round(uy), 1, 3, C.ice, 0.5);
        if (age % 3 === 0) this.add({ x: x - ux * 4, y: y - uy * 4, vx: this.range(-0.2, 0.2), vy: 0.15, drag: 0.98, color: C.iceCore, life: 16 });
        this.lights.push({ x, y, radius: glow, color: C.ice, intensity: 0.8 });
        break;
      }
      case "fire": {
        // A rolling ball: a size that flickers, a hot core, embers and smoke thrown off behind.
        const r = age % 6 < 3 ? 2 : 3;
        p.rect(X - r, Y - r + 1, r * 2, r * 2 - 2, C.ember, 0.9);
        p.rect(X - r + 1, Y - r, r * 2 - 2, r * 2, C.fire, 1);
        p.rect(X - 1, Y - 1, 2, 2, C.fireHot, 1);
        p.rect(X, Y - 1, 1, 1, C.fireCore, 1);
        for (let i = 0; i < 2; i++) this.add({ x: x - ux * this.range(2, 5) + this.range(-1.5, 1.5), y: y - uy * this.range(2, 5) + this.range(-1.5, 1.5), vx: this.range(-0.2, 0.2), vy: -this.range(0.05, 0.35), drag: 0.95, color: i ? C.fire : C.fireHot, late: C.ember, life: Math.round(this.range(6, 14)) });
        if (age % 4 === 0) this.add({ x: x - ux * 5, y: y - uy * 5, vy: -0.25, drag: 0.99, color: C.smoke, size: 2, life: 26 });
        this.lights.push({ x, y, radius: glow * (age % 6 < 3 ? 0.9 : 1), color: C.fire, intensity: 0.95 });
        break;
      }
      case "venom": {
        // A slow wet glob: dark rim, light top-left, and it drips.
        p.rect(X - 1, Y - 2, 3, 5, C.venomDark, 1);
        p.rect(X - 2, Y - 1, 5, 3, C.venomDark, 1);
        p.rect(X - 1, Y - 1, 3, 3, C.venom, 1);
        p.rect(X - 1, Y - 1, 1, 1, C.venomLight, 1);
        if (age % 5 === 0) this.add({ x: x + this.range(-1, 1), y: y + 2, vy: 0.2, g: 0.06, drag: 0.98, color: C.venom, life: 14 });
        if (glow) this.lights.push({ x, y, radius: glow, color: C.venom, intensity: 0.5 });
        break;
      }
      case "needle": {
        for (let i = 0; i < 4; i++) p.rect(X - Math.round(ux * i), Y - Math.round(uy * i), 1, 1, i === 0 ? C.bone : C.needle, 1 - i * 0.2);
        break;
      }
      case "spark": {
        // A crackle, not a ball: a white point and a jagged tail redrawn every frame.
        p.rect(X - 1, Y - 1, 2, 2, C.sparkCore, 1);
        let px = x;
        let py = y;
        for (let i = 0; i < 4; i++) {
          const nx = px - ux * 2.5 + this.range(-1.2, 1.2);
          const ny = py - uy * 2.5 + this.range(-1.2, 1.2);
          p.rect(Math.round(nx), Math.round(ny), 1, 1, i < 2 ? C.spark : C.sparkDeep, 1 - i * 0.2);
          px = nx;
          py = ny;
        }
        if (age % 2 === 0) p.rect(X + Math.round(this.range(-3, 3)), Y + Math.round(this.range(-3, 3)), 1, 1, C.sparkCore, 0.8);
        this.lights.push({ x, y, radius: glow * this.range(0.8, 1.1), color: C.spark, intensity: 0.9 });
        break;
      }
      case "charge": {
        // The explosion is a slow lit charge: a dark round thing with a fuse that spits.
        p.rect(X - 1, Y - 2, 3, 5, "#2a2426", 1);
        p.rect(X - 2, Y - 1, 5, 3, "#2a2426", 1);
        p.rect(X - 1, Y - 1, 1, 1, "#5a5250", 1);
        p.rect(X + 1, Y - 3, 1, 1, C.rope, 1);
        const lit = age % 4 < 2;
        p.rect(X + 1 + (lit ? 1 : 0), Y - 4, 1, 1, lit ? C.blastHot : C.blast, 1);
        if (age % 3 === 0) this.add({ x: x + 2, y: y - 4, vx: this.range(-0.5, 0.5), vy: -this.range(0.3, 0.7), g: 0.05, drag: 0.94, color: C.fireHot, life: 8 });
        this.lights.push({ x, y: y - 3, radius: glow * (lit ? 0.6 : 0.45), color: C.blast, intensity: 0.8 });
        break;
      }
    }
  }

  /**
   * Something lying on the ground. `age` and `left` in ticks since it landed and until it goes;
   * the first frames drop it from the thrower's height, the last ones fade it out.
   */
  drawGround(p: Painter, look: GroundLook, x: number, y: number, radius: number, age: number, left: number, pulse: number): void {
    const X = Math.round(x);
    const Y = Math.round(y);
    const fade = Math.min(1, left / 30);
    const land = Math.min(1, age / 8);
    if (land < 1) {
      // Still in the air: a shadow on the ground and the thing above it, falling.
      const h = Math.round((1 - land) * 16);
      p.rect(X - 2, Y, 5, 1, "#000000", 0.35);
      const col = look === "charge" ? "#2a2426" : look === "dust" ? C.dust : look === "net" ? C.rope : C.web;
      p.rect(X - 1, Y - h - 2, 3, 3, col, 1);
      return;
    }
    switch (look) {
      case "web":
      case "net": {
        const r = Math.max(4, Math.round(radius));
        const col = look === "web" ? C.web : C.rope;
        const alpha = (look === "web" ? 0.75 : 0.9) * fade;
        if (look === "web") {
          // Eight spokes and three rings of thread, flattened to lie on the ground.
          for (let s = 0; s < 8; s++) {
            const a = (s / 8) * Math.PI * 2 + 0.2;
            for (let t = 2; t <= r; t++) p.rect(X + Math.round(Math.cos(a) * t), Y + Math.round(Math.sin(a) * t * 0.6), 1, 1, col, alpha);
          }
          for (const k of [0.35, 0.65, 0.95]) {
            const rr = r * k;
            const n = Math.max(12, Math.round(rr * 4));
            for (let i = 0; i < n; i++) {
              const a = (i / n) * Math.PI * 2;
              // Threads sag between spokes: pull each point in a little away from a spoke.
              const sag = 1 - 0.12 * Math.abs(Math.sin(a * 4));
              p.rect(X + Math.round(Math.cos(a) * rr * sag), Y + Math.round(Math.sin(a) * rr * sag * 0.6), 1, 1, col, alpha * 0.8);
            }
          }
          p.rect(X - 1, Y, 2, 1, col, alpha);
        } else {
          // A net: a rope grid inside a ragged ring, knots darker where the ropes cross.
          const ry = r * 0.6;
          for (let yy = -Math.floor(ry); yy <= ry; yy++) {
            for (let xx = -r; xx <= r; xx++) {
              if ((xx / r) ** 2 + (yy / ry) ** 2 > 1) continue;
              const onX = ((xx + 40) % 4) === 0;
              const onY = ((yy + 40) % 3) === 0;
              if (onX && onY) p.rect(X + xx, Y + yy, 1, 1, C.ropeDark, alpha);
              else if (onX || onY) p.rect(X + xx, Y + yy, 1, 1, col, alpha * 0.8);
            }
          }
        }
        break;
      }
      case "dust": {
        // Scale dust hangs: a faint low haze and motes that turn slowly in it.
        const r = radius;
        const ry = r * 0.55;
        for (let yy = -Math.floor(ry); yy <= ry; yy += 2) {
          for (let xx = -Math.floor(r); xx <= r; xx += 2) {
            if ((xx / r) ** 2 + (yy / ry) ** 2 > 1) continue;
            if (((xx * 7 + yy * 13) & 3) !== 0) continue;
            p.rect(X + xx, Y + yy, 2, 2, C.dust, 0.12 * fade);
          }
        }
        for (let i = 0; i < 16; i++) {
          const a = (i / 16) * Math.PI * 2 + age * 0.012 * (i % 2 ? 1 : -1);
          const rr = r * (0.2 + ((i * 37) % 10) / 12);
          const lift = 3 + ((i * 5 + (age >> 3)) % 9);
          p.rect(X + Math.round(Math.cos(a) * rr), Y - lift + Math.round(Math.sin(a) * rr * 0.5), 1, 1, i % 4 === 0 ? "#e0d8ec" : C.dust, 0.7 * fade);
        }
        break;
      }
      case "charge": {
        // The lobbed charge burning where it lies: scorch spreading, the canister, sparks, and a
        // hot flash on every pulse (that is when it hurts).
        const r = Math.round(radius * Math.min(1, age / 20));
        const ry = r * 0.55;
        for (let yy = -Math.floor(ry); yy <= ry; yy++) {
          for (let xx = -r; xx <= r; xx++) {
            const d = (xx / r) ** 2 + (yy / ry) ** 2;
            if (d > 1 || ((xx * 3 + yy * 5 + 99) % 3) === 0) continue;
            p.rect(X + xx, Y + yy, 1, 1, C.scorch, 0.45 * fade * (1 - d * 0.5));
          }
        }
        const flash = pulse > 0 ? 1 - (age % pulse) / Math.min(pulse, 8) : 0;
        if (flash > 0) {
          const fr = Math.round(r * 0.8);
          for (let xx = -fr; xx <= fr; xx++) p.rect(X + xx, Y + Math.round(Math.sqrt(Math.max(0, 1 - (xx / fr) ** 2)) * fr * 0.5), 1, 1, C.fire, flash * 0.8 * fade);
          for (let xx = -fr; xx <= fr; xx++) p.rect(X + xx, Y - Math.round(Math.sqrt(Math.max(0, 1 - (xx / fr) ** 2)) * fr * 0.5), 1, 1, C.fireHot, flash * 0.6 * fade);
          this.lights.push({ x, y: y - 2, radius: 40, color: C.fire, intensity: flash * fade });
        }
        p.rect(X - 2, Y - 3, 4, 4, "#2a2426", fade);
        p.rect(X - 1, Y - 3, 1, 1, "#5a5250", fade);
        if (age % 2 === 0) this.add({ x: x + this.range(-2, 2), y: y - 4, vx: this.range(-0.7, 0.7), vy: -this.range(0.4, 1), g: 0.06, drag: 0.94, color: age % 4 ? C.fireHot : C.fire, life: 10 });
        if (age % 6 === 0) this.add({ x: x + this.range(-3, 3), y: y - 5, vy: -0.3, drag: 0.99, color: C.smoke, size: 2, life: 36 });
        this.lights.push({ x, y: y - 2, radius: 22, color: C.fire, intensity: 0.6 * fade });
        break;
      }
    }
  }

  /**
   * What a status looks like on its wearer: (x, y) the feet, `h` the sprite's height above
   * them, `left` ticks remaining. Small, and above or around the unit, never over the face.
   */
  drawStatus(p: Painter, look: StatusLook, x: number, y: number, h: number, t: number, left: number): void {
    const X = Math.round(x);
    const Y = Math.round(y);
    const top = Y - h;
    const fade = Math.min(1, left / 20);
    switch (look) {
      case "frost":
        // Rime at the feet, and a flake now and then.
        p.rect(X - 4, Y, 8, 1, C.ice, 0.8 * fade);
        p.rect(X - 3, Y - 1, 1, 1, C.iceCore, 0.8 * fade);
        p.rect(X + 2, Y - 1, 1, 1, C.iceCore, 0.8 * fade);
        if (t % 20 === 0) this.add({ x: X + this.range(-5, 5), y: top + 2, vy: 0.2, drag: 1, color: C.iceCore, life: 30 });
        break;
      case "burning": {
        // Three small tongues of flame licking up the body, each on its own flicker, and smoke.
        for (let i = 0; i < 3; i++) {
          const fxp = X - 5 + i * 5;
          const base = Y - 2 - ((i * 5) % 7);
          const k = (t + i * 7) % 12;
          const tall = k < 4 ? 4 : k < 8 ? 3 : 2;
          p.rect(fxp, base - tall + 1, 2, tall, C.fire, 0.9 * fade);
          p.rect(fxp, base - 1, 1, 2, C.fireHot, fade);
          if (tall === 4) p.rect(fxp + 1, base - tall, 1, 1, C.fireHot, 0.8 * fade);
        }
        if (t % 4 === 0) this.add({ x: X + this.range(-4, 4), y: Y - this.range(6, h * 0.8), vy: -this.range(0.3, 0.6), drag: 0.97, color: C.fireHot, late: C.ember, life: 14 });
        if (t % 9 === 0) this.add({ x: X + this.range(-3, 3), y: top + 2, vy: -0.3, drag: 0.99, color: C.smoke, size: 2, life: 26 });
        this.lights.push({ x, y: y - h / 2, radius: 22, color: C.fire, intensity: 0.55 * fade });
        break;
      }
      case "poison": {
        // Green bubbles rising off the head and popping, and a sick drip at the feet.
        for (let i = 0; i < 2; i++) {
          const k = (t + i * 20) % 40;
          if (k > 30) continue;
          const bx = X - 3 + i * 6 + Math.round(Math.sin((t + i * 9) * 0.2));
          const by = top + 2 - Math.round(k / 4);
          const pop = k > 26;
          if (pop) {
            p.rect(bx - 1, by, 1, 1, C.venomLight, fade);
            p.rect(bx + 2, by, 1, 1, C.venomLight, fade);
          } else {
            p.rect(bx, by, 2, 2, C.venom, 0.9 * fade);
            p.rect(bx, by, 1, 1, C.venomLight, fade);
          }
        }
        p.rect(X - 3, Y, 6, 1, C.venomDark, 0.7 * fade);
        if (t % 16 === 0) this.add({ x: X + this.range(-4, 4), y: Y - this.range(6, h * 0.6), vy: 0.1, g: 0.03, drag: 0.98, color: C.venom, life: 18 });
        break;
      }
      case "stars": {
        // Three small stars going round above the head: stunned, staggered, dazzled.
        for (let i = 0; i < 3; i++) {
          const a = t * 0.12 + (i * Math.PI * 2) / 3;
          const sx = X + Math.round(Math.cos(a) * 6);
          const sy = top - 2 + Math.round(Math.sin(a) * 2);
          p.rect(sx, sy - 1, 1, 3, C.gold, fade);
          p.rect(sx - 1, sy, 3, 1, C.gold, fade);
        }
        break;
      }
      case "web":
        // Threads across the feet.
        for (let i = -5; i <= 5; i++) p.rect(X + i, Y - 1 - (Math.abs(i) % 3 === 0 ? 1 : 0), 1, 1, C.web, 0.8 * fade);
        p.rect(X - 4, Y - 4, 1, 3, C.web, 0.6 * fade);
        p.rect(X + 3, Y - 5, 1, 4, C.web, 0.6 * fade);
        break;
      case "dust":
        for (let i = 0; i < 5; i++) {
          const a = t * 0.05 + i * 1.3;
          p.rect(X + Math.round(Math.cos(a) * 6), Y - 4 - ((i * 3 + (t >> 4)) % 10), 1, 1, C.dust, 0.8 * fade);
        }
        break;
      case "jolt":
        if (t % 4 === 0) this.bolt(X + this.range(-5, 5), top + this.range(0, h * 0.5), X + this.range(-5, 5), top + this.range(h * 0.5, h), C.spark, C.sparkCore, 3, 1.5);
        break;
      case "shield": {
        // A thin shell of blue light round her, brighter at the rim, turning.
        const n = 18;
        for (let i = 0; i < n; i++) {
          const a = (i / n) * Math.PI * 2 + t * 0.03;
          const bright = (i + (t >> 3)) % 6 === 0;
          p.rect(X + Math.round(Math.cos(a) * 9), Y - h / 2 + Math.round(Math.sin(a) * (h / 2 + 1)), 1, 1, bright ? "#d8ecff" : C.shield, (bright ? 0.9 : 0.35) * fade);
        }
        break;
      }
      case "drain":
        // Two dark red motes circling in towards her, as if she were drawing something off the air.
        for (let i = 0; i < 2; i++) {
          const k = ((t + i * 30) % 60) / 60;
          const a = k * Math.PI * 3 + i * Math.PI;
          const r = 11 * (1 - k);
          p.rect(X + Math.round(Math.cos(a) * r), Y - h / 2 + Math.round(Math.sin(a) * r * 0.8), 2, 1, "#8a2a34", fade);
          p.rect(X + Math.round(Math.cos(a) * r), Y - h / 2 + Math.round(Math.sin(a) * r * 0.8), 1, 1, "#c85060", fade);
        }
        break;
      case "glint":
        if (t % 40 < 6) {
          const gx = X + 4;
          const gy = top + 6;
          p.rect(gx, gy - 2, 1, 5, C.gold, fade);
          p.rect(gx - 2, gy, 5, 1, C.gold, fade);
          p.rect(gx, gy, 1, 1, "#fff8d8", fade);
        }
        break;
      case "stone":
        // Grey flecks over the body, as if she were going to granite.
        for (let i = 0; i < 10; i++) p.rect(X - 5 + ((i * 5) % 11), Y - 2 - ((i * 7) % (h - 5)), (i & 1) + 1, 1, i % 3 ? C.stone : "#b8bcc4", 0.8 * fade);
        break;
      case "ember_hands":
      case "frost_hands":
      case "spark_hands":
      case "thorn_hands": {
        // The weapon-hand charm: both hands glow their school's colour and shed a mote now and then.
        const col = look === "ember_hands" ? C.fireHot : look === "frost_hands" ? C.iceCore : look === "spark_hands" ? C.spark : C.leaf;
        const deep = look === "ember_hands" ? C.fire : look === "frost_hands" ? C.ice : look === "spark_hands" ? C.sparkDeep : C.venomDark;
        const on = t % 20 < 14;
        for (const sx of [-6, 5]) {
          p.rect(X + sx, Y - 8, 2, 2, deep, 0.7 * fade);
          if (on) p.rect(X + sx + (sx < 0 ? 0 : 1), Y - 8, 1, 1, col, fade);
        }
        if (t % 10 === 0) this.add({ x: X + (t % 20 ? 6 : -6), y: Y - 9, vy: look === "thorn_hands" ? 0.2 : -0.3, drag: 1, color: col, life: 14 });
        break;
      }
      case "softened":
        // Fire has opened the gilt: it runs. Gold drips down her and gathers at the feet.
        for (let i = 0; i < 3; i++) {
          const k = (t + i * 13) % 30;
          p.rect(X - 4 + i * 4, Y - h + 6 + Math.round(k / 2.5), 1, 2, C.gold, 0.9 * fade);
        }
        p.rect(X - 4, Y, 8, 1, "#b08828", 0.8 * fade);
        break;
      case "instant":
        break;
    }
  }

  // --- the particle pass ---------------------------------------------------------------------------

  /** Step everything one frame. */
  step(): void {
    this.frame++;
    for (let i = this.parts.length - 1; i >= 0; i--) {
      const q = this.parts[i];
      if (--q.life <= 0) {
        this.parts.splice(i, 1);
        continue;
      }
      if (q.ground) continue;
      q.vy += q.g;
      q.vx *= q.drag;
      q.vy *= q.drag;
      q.x += q.vx;
      q.y += q.vy;
    }
    for (let i = this.strokes.length - 1; i >= 0; i--) if (--this.strokes[i].life <= 0) this.strokes.splice(i, 1);
    for (let i = this.rings.length - 1; i >= 0; i--) if (--this.rings[i].life <= 0) this.rings.splice(i, 1);
  }

  /** Marks on the ground (scorch, stains, rime): drawn under units. */
  drawGroundMarks(p: Painter): void {
    for (const q of this.parts) {
      if (!q.ground) continue;
      const a = Math.min(1, q.life / 30);
      p.rect(Math.round(q.x), Math.round(q.y), q.size, Math.max(1, q.size >> 1), q.color, a * 0.8);
    }
  }

  /** Everything in the air: drawn over units, before the light pass. */
  draw(p: Painter): void {
    for (const r of this.rings) {
      const k = 1 - r.life / r.max;
      const rad = r.r0 + (r.r1 - r.r0) * (1 - (1 - k) * (1 - k));
      const a = r.life / r.max;
      const n = Math.max(10, Math.round(rad * 5));
      for (let i = 0; i < n; i++) {
        const t = (i / n) * Math.PI * 2;
        p.rect(Math.round(r.x + Math.cos(t) * rad), Math.round(r.y + Math.sin(t) * rad * r.squash), 1, 1, r.color, a);
      }
    }
    for (const s of this.strokes) {
      const a = s.life / s.max;
      for (let i = 0; i + 3 < s.pts.length; i += 2) {
        const x0 = s.pts[i];
        const y0 = s.pts[i + 1];
        const x1 = s.pts[i + 2];
        const y1 = s.pts[i + 3];
        const n = Math.max(1, Math.round(Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0))));
        for (let j = 0; j < n; j++) {
          const px = Math.round(x0 + ((x1 - x0) * j) / n);
          const py = Math.round(y0 + ((y1 - y0) * j) / n);
          p.rect(px, py, 1, 1, s.core && j % 2 === 0 ? s.core : s.color, a);
        }
      }
    }
    for (const q of this.parts) {
      if (q.ground) continue;
      const k = q.life / q.max;
      const color = q.late && k < 0.5 ? q.late : q.color;
      p.rect(Math.round(q.x), Math.round(q.y), q.size, q.size, color, Math.min(1, q.life / 10) * (q.size > 1 ? 0.55 : 1));
    }
  }

  /** Hand the frame's lights over and forget them. */
  takeLights(into: Light[], vx: number, vy: number): void {
    for (const l of this.lights) into.push({ ...l, x: l.x - vx, y: l.y - vy });
    this.lights.length = 0;
  }
}
