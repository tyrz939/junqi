// World renderer: Canvas2D into a low-resolution framebuffer, upscaled by an
// integer factor with image-rendering: pixelated. The view is ~216 px tall (2020's
// camera was 256); its width follows the window's aspect, as 2020's did. The UI is
// DOM at native resolution, so "zoomed world, crisp UI" needs no second camera.
//
// The renderer reads sim state and never writes it. Positions are interpolated
// between the last two ticks (`alpha`), so a 144 Hz display is smooth while the
// sim stays at 60.
//
// Frame:  tile chunks -> flat props -> grounds -> drops -> y-sorted props/units
//         -> projectiles -> particles -> LIGHT (multiply) -> fog -> unlit overlays
//         (health bars, floating text, reticle) -> debug

import type { SpriteSheet } from "@/art/types";
import { SEAT_COATS, seatSprite } from "@/art/units";
import { CELL, RING_RADIUS } from "@/sim/constants";
import type { SimEvent } from "@/sim/events";
import { BLOCK_MOVE, cellOf } from "@/sim/grid";
import { lampsLit, propLightShowing } from "@/sim/light";
import { propsInCells } from "@/sim/runtime";
// The renderer draws ONE SEAT's view: her zone, her camera. Four clients, four cameras.
import type { PlayerView as Sim } from "@/sim/sim";
import { FACING_DX, FACING_DY, type Prop, type School, type Unit } from "@/sim/state";
import { maxHp } from "@/sim/units";
import { fogSeen } from "@/sim/zones";
import { buildAtlas, drawSprite, iconDataUrl, type Atlas } from "@/render/atlas";
import { ambientForHour, Lighting, type Light } from "@/render/lighting";
import { TileCache } from "@/render/tiles";

const TARGET_VIEW_H = 216;

/**
 * How far outside the view a prop is still visited, in px from its origin cell: the
 * widest light in props.json (the lamp post, 96) and a little more, so a pool of light
 * reaches the screen before its post does. The tallest sprite is well inside that.
 */
const PROP_VIEW_MARGIN = 104;

const SCHOOL_COLOR: Record<School, string> = {
  heal: "#78e860",
  physical: "#f4f0e6",
  frost: "#a8e0f8",
  fire: "#ff9040",
  nature: "#98d848",
  blast: "#f6cd40",
  shock: "#c8b8ff",
};

type FloatText = { x: number; y: number; text: string; color: string; age: number; big: boolean };
type Particle = { x: number; y: number; vx: number; vy: number; life: number; color: string };
type Drawable = { y: number; draw: () => void };

export class Renderer {
  readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly atlas: Atlas;
  private readonly tiles = new TileCache();
  private readonly lighting = new Lighting();
  private scale = 4;
  viewW = 384;
  viewH = TARGET_VIEW_H;
  private camX = 0;
  private camY = 0;
  private camReady = false;
  private shake = 0;
  private lockRect = "";
  private zoneKey = "";
  private readonly prev = new Map<number, { x: number; y: number }>();
  private readonly propShown = new Map<number, { x: number; y: number }>();
  /** Reused every frame: the props in the blocks under the view. */
  private readonly propsInView: Prop[] = [];
  private readonly texts: FloatText[] = [];
  private readonly particles: Particle[] = [];
  private frameNo = 0;
  showGrid = false;
  /** Cursor in canvas client pixels; null when the mouse is not the active aim device. */
  cursor: { x: number; y: number } | null = null;
  frameMs = 0;

  constructor(canvas: HTMLCanvasElement, ...sheets: SpriteSheet[]) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d", { alpha: false })!;
    this.atlas = buildAtlas(...sheets);
    this.resize();
  }

  iconUrl(id: string): string {
    return iconDataUrl(this.atlas, id);
  }

  /** Integer scale so every world pixel is a whole number of screen pixels. */
  resize(): void {
    const w = Math.max(320, window.innerWidth);
    const h = Math.max(180, window.innerHeight);
    this.scale = Math.max(1, Math.round(h / TARGET_VIEW_H));
    this.viewW = Math.ceil(w / this.scale);
    this.viewH = Math.ceil(h / this.scale);
    this.canvas.width = this.viewW;
    this.canvas.height = this.viewH;
    this.canvas.style.width = `${this.viewW * this.scale}px`;
    this.canvas.style.height = `${this.viewH * this.scale}px`;
    this.ctx.imageSmoothingEnabled = false;
    this.lighting.resize(this.viewW, this.viewH);
  }

  /** Client pixels -> world pixels. */
  screenToWorld(clientX: number, clientY: number): { x: number; y: number } {
    const r = this.canvas.getBoundingClientRect();
    return { x: (clientX - r.left) / this.scale + this.camX, y: (clientY - r.top) / this.scale + this.camY };
  }

  /** Call just before sim.tick(): remembers where everything was, for interpolation. */
  snapshot(sim: Sim): void {
    for (const u of sim.zone.units) {
      const p = this.prev.get(u.id);
      if (p) {
        p.x = u.x;
        p.y = u.y;
      } else this.prev.set(u.id, { x: u.x, y: u.y });
    }
  }

  /**
   * `mine` is this seat's unit. Numbers are a feeling, as in classic WoW, and they are
   * yours: what you dealt, what you took, what you healed. A friend's fight shows its
   * sparks and the enemy's bar, never her arithmetic.
   */
  handle(events: readonly SimEvent[], mine: number): void {
    for (const ev of events) {
      switch (ev.e) {
        case "damage":
          if ((ev.unit === mine || ev.from === mine) && (ev.amount > 0 || ev.absorbed > 0)) {
            this.text(ev.x, ev.y - 14, ev.amount > 0 ? String(ev.amount) : "absorb", ev.crit ? "#f0d048" : SCHOOL_COLOR[ev.school], ev.crit);
          }
          this.burst(ev.x, ev.y - 6, SCHOOL_COLOR[ev.school], ev.crit ? 8 : 4);
          break;
        case "heal":
          if (ev.unit === mine || ev.from === mine) this.text(ev.x, ev.y - 14, `+${ev.amount}`, SCHOOL_COLOR.heal, false);
          break;
        case "death":
          this.burst(ev.x, ev.y - 6, "#a868c8", 14);
          break;
        case "impact":
          this.burst(ev.x, ev.y, SCHOOL_COLOR[ev.school], 6);
          break;
        case "shake":
          this.shake = Math.min(6, this.shake + ev.amount);
          break;
        case "camera":
          this.lockRect = ev.mode === "lock" ? ev.rect : "";
          break;
        case "tiles":
          this.tiles.invalidate(ev.cx, ev.cy, ev.w, ev.h);
          break;
        case "zone":
          this.camReady = false;
          this.lockRect = "";
          break;
        default:
          break;
      }
    }
  }

  private text(x: number, y: number, text: string, color: string, big: boolean): void {
    // Stack in 8 px steps so simultaneous hits stay legible (2020 stacked at 24).
    let yy = y;
    for (const t of this.texts) if (t.age < 12 && Math.abs(t.x - x) < 16 && Math.abs(t.y - yy) < 8) yy -= 8;
    this.texts.push({ x, y: yy, text, color, age: 0, big });
  }

  private burst(x: number, y: number, color: string, n: number): void {
    for (let i = 0; i < n; i++) {
      const a = (i / n) * Math.PI * 2 + this.frameNo;
      const s = 0.4 + ((i * 37) % 10) / 12;
      this.particles.push({ x, y, vx: Math.cos(a) * s, vy: Math.sin(a) * s - 0.3, life: 18 + ((i * 13) % 12), color });
    }
  }

  draw(sim: Sim, alpha: number): void {
    const t0 = performance.now();
    this.frameNo++;
    const ctx = this.ctx;
    const bp = sim.rt.bp;
    const key = `${sim.state.seed}:${sim.me.zone}`;
    if (key !== this.zoneKey) {
      this.zoneKey = key;
      this.tiles.setGrid(sim.rt.grid);
      this.prev.clear();
      this.propShown.clear();
      this.camReady = false;
    }
    const lerp = (u: Unit): { x: number; y: number } => {
      const p = this.prev.get(u.id);
      if (!p) return { x: u.x, y: u.y };
      const dx = u.x - p.x;
      const dy = u.y - p.y;
      if (dx * dx + dy * dy > 40 * 40) return { x: u.x, y: u.y }; // teleport: do not smear
      return { x: p.x + dx * alpha, y: p.y + dy * alpha };
    };

    // --- camera: follow with easing, or hold a locked room ---------------------
    const player = sim.player;
    const pp = lerp(player);
    let tx = pp.x - this.viewW / 2;
    let ty = pp.y - 10 - this.viewH / 2;
    const lock = this.lockRect ? bp.rects[this.lockRect] : undefined;
    if (lock) {
      const lx = lock.cx * CELL;
      const ly = lock.cy * CELL;
      const lw = lock.w * CELL;
      const lh = lock.h * CELL;
      const pad = 24;
      tx = lw + pad * 2 <= this.viewW ? lx + lw / 2 - this.viewW / 2 : Math.max(lx - pad, Math.min(lx + lw + pad - this.viewW, tx));
      ty = lh + pad * 2 <= this.viewH ? ly + lh / 2 - this.viewH / 2 : Math.max(ly - pad, Math.min(ly + lh + pad - this.viewH, ty));
    }
    const worldW = bp.w * CELL;
    const worldH = bp.h * CELL;
    tx = worldW <= this.viewW ? (worldW - this.viewW) / 2 : Math.max(0, Math.min(worldW - this.viewW, tx));
    ty = worldH <= this.viewH ? (worldH - this.viewH) / 2 : Math.max(0, Math.min(worldH - this.viewH, ty));
    if (!this.camReady) {
      this.camX = tx;
      this.camY = ty;
      this.camReady = true;
    } else {
      this.camX += (tx - this.camX) * 0.18;
      this.camY += (ty - this.camY) * 0.18;
    }
    this.shake *= 0.86;
    const sx = this.shake > 0.2 ? Math.round(Math.sin(this.frameNo * 1.7) * this.shake) : 0;
    const sy = this.shake > 0.2 ? Math.round(Math.cos(this.frameNo * 2.3) * this.shake) : 0;
    const vx = Math.round(this.camX) + sx;
    const vy = Math.round(this.camY) + sy;

    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
    ctx.fillStyle = "#0b0a10";
    ctx.fillRect(0, 0, this.viewW, this.viewH);
    this.tiles.draw(ctx, vx, vy, this.viewW, this.viewH);
    this.drawWater(sim, vx, vy);

    const inView = (x: number, y: number, m: number): boolean =>
      x > vx - m && y > vy - m && x < vx + this.viewW + m && y < vy + this.viewH + m;
    const lights: Light[] = [];
    // Whether a prop's light is on is the sim's rule (sim/light.ts): what she sees lit is what a sentry sees lit.
    const night = lampsLit(sim.state);
    const amb = bp.indoor ? bp.ambient * 255 : Math.min(...ambientForHour(sim.hour));
    const darkness = 1 - amb / 255;
    const flick = (seed: number, amount: number): number => 1 - amount * (0.5 + 0.5 * Math.sin(this.frameNo * 0.23 + seed * 1.7));

    // --- props: ease display position toward the cell so pushes slide --------
    // Only the blocks under the view are visited, never the zone's whole prop list.
    const sorted: Drawable[] = [];
    // What she carries still has the cell it was lifted from, which may be off screen by now.
    const held = player.carrying ? sim.rt.props.get(player.carrying) : undefined;
    const carried: Prop | null = held && !held.hidden ? held : null;
    const m = PROP_VIEW_MARGIN;
    for (const p of propsInCells(sim.rt, cellOf(vx - m), cellOf(vy - m), cellOf(vx + this.viewW + m), cellOf(vy + this.viewH + m), this.propsInView)) {
      if (p.hidden || !p.awake || p === carried) continue;
      const def = sim.catalog.props[p.def];
      const gx = p.cx * CELL;
      const gy = p.cy * CELL;
      let shown = this.propShown.get(p.id);
      if (!shown) {
        shown = { x: gx, y: gy };
        this.propShown.set(p.id, shown);
      }
      shown.x += (gx - shown.x) * 0.25;
      shown.y += (gy - shown.y) * 0.25;
      if (Math.abs(gx - shown.x) < 0.3) shown.x = gx;
      if (Math.abs(gy - shown.y) < 0.3) shown.y = gy;
      if (!inView(gx, gy, m)) continue;
      const sprite = this.atlas[def.sprite];
      if (def.light && propLightShowing(def, p, night)) {
        lights.push({
          x: shown.x + (def.w * CELL) / 2 - vx,
          y: shown.y + (def.h * CELL) / 2 - 6 - vy,
          radius: def.light.radius,
          color: def.light.color,
          intensity: flick(p.id, def.light.flicker),
        });
      }
      if (def.gate && !p.solid) continue; // an open gate is a doorway
      // "base" is one drawing, and a hillside carries hundreds of herbs, so drawing it every time is
      // what makes a meadow look printed. Where the art offers "base2"/"base3" the prop picks one by its
      // own id, so the same plant is the same plant every frame and its neighbour is a different one.
      let rest = "base";
      if (sprite && sprite.frames.base2) {
        const n = sprite.frames.base3 ? 3 : 2;
        const pick = (p.id * 2654435761) >>> 0;
        rest = ["base", "base2", "base3"][pick % n];
      }
      const frame = p.used && p.loot === null && sprite?.frames.open ? "open" : (p.on || (def.nightOnly && night)) && sprite?.frames.on ? "on" : rest;
      const sxp = shown.x;
      const syp = shown.y;
      const paint = (): void => {
        if (sprite) drawSprite(ctx, sprite, frame, sxp - vx, syp - vy);
        else {
          ctx.fillStyle = "#c8403c";
          ctx.fillRect(sxp - vx, syp - vy, def.w * CELL, def.h * CELL);
        }
      };
      if (def.flat) paint();
      else sorted.push({ y: shown.y + def.h * CELL, draw: paint });
    }

    // --- ground effects and drops ------------------------------------------------
    for (const g of sim.zone.grounds) {
      if (!inView(g.x, g.y, 40)) continue;
      ctx.globalAlpha = Math.min(0.6, g.left / 60);
      ctx.strokeStyle = "#f4f0e6";
      ctx.lineWidth = 1;
      for (let r = g.radius; r > 2; r -= 4) {
        ctx.beginPath();
        ctx.arc(Math.round(g.x - vx), Math.round(g.y - vy), r, 0, Math.PI * 2);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    }
    for (const d of sim.zone.drops) {
      if (!inView(d.x, d.y, 16)) continue;
      const icon = this.atlas[sim.catalog.items[d.item].icon];
      const bob = Math.round(Math.sin(this.frameNo * 0.1 + d.id) * 1.5);
      sorted.push({
        y: d.y,
        draw: () => {
          ctx.fillStyle = "#00000050";
          ctx.fillRect(Math.round(d.x - vx) - 4, Math.round(d.y - vy) + 2, 8, 2);
          if (icon) ctx.drawImage(icon.canvas, 0, 0, 16, 16, Math.round(d.x - vx) - 6, Math.round(d.y - vy) - 12 + bob, 12, 12);
        },
      });
    }

    // --- units -----------------------------------------------------------------
    for (const u of sim.zone.units) {
      if (!u.awake || u.hidden) continue;
      const pos = lerp(u);
      if (!inView(pos.x, pos.y, 220)) continue;
      const def = sim.catalog.units[u.def];
      if (def.glow && u.alive) lights.push({ x: pos.x - vx, y: pos.y - 8 - vy, radius: def.glow.radius, color: def.glow.color, intensity: 0.8 });
      // Jane carries a little light of her own, but only once it is dark enough to need it.
      if (u.id === player.id && darkness > 0.25) lights.push({ x: pos.x - vx, y: pos.y - 8 - vy, radius: 68, color: "#8a7a68", intensity: Math.min(0.9, darkness * 1.4) });
      sorted.push({ y: pos.y, draw: () => this.drawUnit(sim, u, pos.x - vx, pos.y - vy) });
      if (u.segments && u.alive) sorted.push({ y: pos.y - 1, draw: () => this.drawSnakeBody(u, vx, vy) });
    }

    sorted.sort((a, b) => a.y - b.y);
    for (const d of sorted) d.draw();
    if (carried) {
      const sprite = this.atlas[sim.catalog.props[carried.def].sprite];
      if (sprite) drawSprite(ctx, sprite, "base", pp.x - vx - sprite.w / 2, pp.y - vy - 26);
    }

    // --- projectiles -------------------------------------------------------------
    for (const p of sim.zone.projectiles) {
      const x = p.x - p.vx * (1 - alpha) - vx;
      const y = p.y - p.vy * (1 - alpha) - vy - 6;
      const spell = sim.catalog.spells[p.spell];
      const color = SCHOOL_COLOR[spell.school];
      ctx.fillStyle = color;
      ctx.globalAlpha = 0.35;
      ctx.fillRect(Math.round(x - p.vx * 2) - 1, Math.round(y - p.vy * 2) - 1, 3, 3);
      ctx.globalAlpha = 0.6;
      ctx.fillRect(Math.round(x - p.vx) - 1, Math.round(y - p.vy) - 1, 3, 3);
      ctx.globalAlpha = 1;
      ctx.fillRect(Math.round(x) - 2, Math.round(y) - 2, 4, 4);
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(Math.round(x) - 1, Math.round(y) - 1, 2, 2);
      if (spell.glow) lights.push({ x, y, radius: spell.glow * Math.min(1, p.age / 5), color, intensity: 0.9 });
    }

    for (let i = this.particles.length - 1; i >= 0; i--) {
      const p = this.particles[i];
      p.x += p.vx;
      p.y += p.vy;
      p.vy += 0.03;
      if (--p.life <= 0) {
        this.particles.splice(i, 1);
        continue;
      }
      ctx.globalAlpha = Math.min(1, p.life / 10);
      ctx.fillStyle = p.color;
      ctx.fillRect(Math.round(p.x - vx), Math.round(p.y - vy), 1, 1);
    }
    ctx.globalAlpha = 1;

    // --- fog: drawn BEFORE the light pass, so lamps glow in it and the dark swallows it ---
    const fogDensity = bp.indoor ? (bp.ambient <= 0.12 ? 0.22 : 0) : 0.06 + darkness * 0.34;
    if (fogDensity > 0.01) this.drawMist(vx, vy, fogDensity);

    // --- light -----------------------------------------------------------------
    const a = bp.indoor ? Math.round(255 * bp.ambient) : 0;
    const ambient: [number, number, number] = bp.indoor ? [a * 0.9, a * 0.95, Math.min(255, a * 1.15)] : ambientForHour(sim.hour);
    this.lighting.apply(ctx, ambient, lights);

    if (bp.indoor) this.drawFog(sim, vx, vy);
    else this.drawSchool(bp.marks.school_mouth, vx, vy, darkness, sim.state.tick);

    // --- unlit overlays ----------------------------------------------------------
    for (const u of sim.zone.units) {
      if (!u.awake || !u.alive || u.id === player.id || u.controller === "npc") continue;
      const hp = maxHp(u);
      if (u.hp >= hp && u.combat !== "combat") continue;
      const pos = lerp(u);
      if (!inView(pos.x, pos.y, 20)) continue;
      const sprite = this.atlas[sim.catalog.units[u.def].sprite];
      const top = pos.y - vy - (sprite ? sprite.ay : 18) - 4;
      const w = sim.catalog.units[u.def].boss ? 24 : 14;
      ctx.fillStyle = "#1a1420";
      ctx.fillRect(Math.round(pos.x - vx - w / 2) - 1, Math.round(top) - 1, w + 2, 4);
      ctx.fillStyle = "#c8403c";
      ctx.fillRect(Math.round(pos.x - vx - w / 2), Math.round(top), Math.max(0, Math.round((w * u.hp) / hp)), 2);
    }
    ctx.font = "8px ui-monospace, Consolas, monospace";
    ctx.textAlign = "center";
    for (let i = this.texts.length - 1; i >= 0; i--) {
      const t = this.texts[i];
      if (++t.age > 54) {
        this.texts.splice(i, 1);
        continue;
      }
      const x = Math.round(t.x - vx);
      const y = Math.round(t.y - vy - t.age * 0.35);
      ctx.globalAlpha = t.age > 40 ? (54 - t.age) / 14 : 1;
      ctx.font = t.big ? "bold 10px ui-monospace, Consolas, monospace" : "8px ui-monospace, Consolas, monospace";
      ctx.fillStyle = "#1a1420";
      ctx.fillText(t.text, x + 1, y + 1);
      ctx.fillStyle = t.color;
      ctx.fillText(t.text, x, y);
    }
    ctx.globalAlpha = 1;

    // The friend under the cursor: corner brackets at her feet, in her coat's colour. It is
    // who a friendly spell would land on, and the only targeting UI the game has.
    const hovered = player.alive ? sim.rt.units.get(this.hoveredFriend(sim)) : undefined;
    if (hovered && hovered.id !== player.id) {
      const pos = lerp(hovered);
      const x = Math.round(pos.x - vx);
      const y = Math.round(pos.y - vy);
      ctx.fillStyle = SEAT_COATS[(sim.party.ofUnit(hovered.id)?.index ?? 0) % SEAT_COATS.length].q;
      for (const sx of [-1, 1]) {
        for (const sy of [-1, 1]) {
          ctx.fillRect(x + sx * 8 - (sx > 0 ? 2 : 0), y - 8 + sy * 10, 3, 1);
          ctx.fillRect(x + sx * 8, y - 8 + sy * 10 - (sy > 0 ? 2 : 0), 1, 3);
        }
      }
    }

    if (this.cursor && player.alive) {
      const c = this.screenToWorld(this.cursor.x, this.cursor.y);
      const x = Math.round(c.x - vx);
      const y = Math.round(c.y - vy);
      ctx.fillStyle = "#f4f0e6";
      ctx.fillRect(x - 3, y, 2, 1);
      ctx.fillRect(x + 2, y, 2, 1);
      ctx.fillRect(x, y - 3, 1, 2);
      ctx.fillRect(x, y + 2, 1, 2);
    }

    if (this.showGrid) this.drawDebug(sim, vx, vy);
    this.frameMs = performance.now() - t0;
  }

  /** The living party member whose sprite is under the cursor (herself included), or 0. */
  hoveredFriend(sim: Sim): number {
    if (!this.cursor) return 0;
    const c = this.screenToWorld(this.cursor.x, this.cursor.y);
    let best = 0;
    let bestD = Infinity;
    for (const u of sim.party.units()) {
      if (!u.alive || u.hidden || sim.rt.units.get(u.id) !== u) continue;
      // The sprite's box, a little generous: she is moving and so is the hand.
      const dx = Math.abs(c.x - u.x);
      const dy = c.y - (u.y - 8);
      if (dx > 9 || Math.abs(dy) > 12) continue;
      const d = dx + Math.abs(dy);
      if (d < bestD) {
        best = u.id;
        bestD = d;
      }
    }
    return best;
  }

  private drawUnit(sim: Sim, u: Unit, x: number, y: number): void {
    const ctx = this.ctx;
    const def = sim.catalog.units[u.def];
    // Four of them, one heroine, one name: they are told apart by the coat, and the coat is the seat.
    const seat = u.controller === "player" ? (sim.party.ofUnit(u.id)?.index ?? 0) : 0;
    const sprite = (seat > 0 ? this.atlas[seatSprite(def.sprite, seat)] : undefined) ?? this.atlas[def.sprite];
    if (!sprite) {
      ctx.fillStyle = u.alive ? "#c8403c" : "#555a66";
      ctx.fillRect(Math.round(x) - 5, Math.round(y) - 14, 10, 14);
      return;
    }
    if (!u.alive) {
      ctx.globalAlpha = 0.75;
      drawSprite(ctx, sprite, "dead", x, y, false, "down");
      ctx.globalAlpha = 1;
      return;
    }
    // The contact shadow. It was one hard 10x3 rectangle, the same under a rat and under a boss, and
    // against the drawn ground cover a square of flat black under everything alive is what makes units
    // read as stickers laid on the field. Three rows, sized to the sprite, give a round soft-ended blot
    // for the cost of two more fillRects and no path and no allocation.
    const sw = Math.max(4, Math.min(13, (sprite.w >> 1) - 1));
    const sx = Math.round(x);
    const sy = Math.round(y);
    ctx.fillStyle = "#0000004a";
    ctx.fillRect(sx - sw + 1, sy - 2, sw * 2 - 2, 1);
    ctx.fillRect(sx - sw, sy - 1, sw * 2, 2);
    ctx.fillRect(sx - sw + 1, sy + 1, sw * 2 - 2, 1);
    const dir = u.facing === 1 ? "down" : u.facing === 3 ? "up" : "side";
    const step = u.anim === "walk" && Math.floor(u.animTick / 9) % 2 === 1;
    let ox = 0;
    let oy = 0;
    if (u.anim === "attack" && u.animTick < 9) {
      ox = FACING_DX[u.facing] * 3;
      oy = FACING_DY[u.facing] * 3;
    } else if (u.anim === "cast" && u.animTick < 14) oy = -1;
    else if (u.anim === "hurt" && u.animTick < 8) ox = u.animTick % 2 === 0 ? 1 : -1;
    const frame = step ? `${dir}2` : dir;
    drawSprite(ctx, sprite, frame, x + ox, y + oy, u.facing === 2, dir);
    if (u.anim === "hurt" && u.animTick < 5) {
      ctx.globalCompositeOperation = "lighter";
      ctx.globalAlpha = 0.5;
      drawSprite(ctx, sprite, frame, x + ox, y + oy, u.facing === 2, dir);
      ctx.globalAlpha = 1;
      ctx.globalCompositeOperation = "source-over";
    }
    for (const s of u.statuses) {
      const e = sim.catalog.effects[s.effect];
      if (e.speed !== undefined && e.speed < 1) {
        ctx.fillStyle = e.speed === 0 ? "#f0d048" : "#a8e0f8";
        ctx.fillRect(Math.round(x) - 4, Math.round(y) + 1, 8, 1);
      }
    }
  }

  /** The snake's body: discs along the recorded trail, tail first, tapering over the last half. */
  private drawSnakeBody(u: Unit, vx: number, vy: number): void {
    const seg = u.segments;
    if (!seg) return;
    const ctx = this.ctx;
    const n = seg.length / 2;
    for (let i = n - 1; i >= 1; i--) {
      const taper = i > n / 2 ? 1 - (i - n / 2) / (n / 2) : 1;
      const r = Math.max(1.5, 6 * taper);
      const wig = Math.sin(this.frameNo * 0.12 + i * 0.5) * 1.5;
      const x = Math.round(seg[i * 2] - vx + wig * u.hy);
      const y = Math.round(seg[i * 2 + 1] - vy - 4 - wig * u.hx);
      ctx.fillStyle = "#1a1420";
      ctx.beginPath();
      ctx.arc(x, y, r + 1, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = i % 8 === 7 ? "#b08828" : i % 2 === 0 ? "#3c8844" : "#22503a";
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  private mistTex: HTMLCanvasElement | null = null;

  /**
   * Ground mist. One 256 px tile of soft blobs, wrapped, drawn twice at different
   * drifts and anchored to the world so it slides past as Jane walks. Thin by day,
   * thick by night and in the burial ("Cold, dark, foggy place").
   */
  /**
   * The School looms (PLAN.md 2.2). A top-down view has no horizon, so the far landmark is
   * drawn where the horizon would be: along the top edge of the screen, a dark mass with
   * one lit window, whenever the School lies north of what she can see. It slides sideways
   * as she walks east or west of it, grows as she gets nearer, and is gone once the real
   * building is on screen. Unlit, so it is the one thing the night never hides.
   */
  private drawSchool(mark: { cx: number; cy: number } | undefined, vx: number, vy: number, darkness: number, tick: number): void {
    if (!mark) return;
    const sx = mark.cx * CELL - vx;
    const north = vy - mark.cy * CELL; // px the School lies above the top of the view
    if (north < 40) return;
    const ctx = this.ctx;
    // Nearer is larger and further in from the side; ten minutes away it is a thumbnail at the screen's edge.
    const near = Math.max(0, Math.min(1, 1 - north / 14000));
    const size = 0.9 + near * 1.1;
    // How much of her sideways offset shows. Little: it is far away, and it must never slide under the HUD.
    const pull = 0.015 + near * 0.1;
    const x = Math.round(this.viewW / 2 + Math.max(-this.viewW * 0.2, Math.min(this.viewW * 0.2, (sx - this.viewW / 2) * pull)));
    const w = Math.round(46 * size);
    const h = Math.round(18 * size);
    ctx.globalAlpha = Math.min(1, north / 160) * (0.72 + darkness * 0.24);
    ctx.fillStyle = "#0c0912";
    // Body, a taller wing, two chimneys, the bell tower.
    ctx.fillRect(x - w / 2, 0, w, Math.round(h * 0.62));
    ctx.fillRect(x - w / 2 + Math.round(w * 0.1), 0, Math.round(w * 0.3), Math.round(h * 0.8));
    ctx.fillRect(x + Math.round(w * 0.22), 0, Math.round(w * 0.16), h);
    ctx.fillRect(x - Math.round(w * 0.42), 0, Math.max(1, Math.round(w * 0.05)), Math.round(h * 0.95));
    ctx.fillRect(x - Math.round(w * 0.02), 0, Math.max(1, Math.round(w * 0.05)), Math.round(h * 0.9));
    // The window. It is never not lit. A slow, uneven breath, from the tick so it is the same for everyone.
    const breath = 0.78 + 0.22 * Math.abs(((tick >> 3) % 40) - 20) / 20;
    ctx.globalAlpha = Math.min(1, north / 160) * breath;
    ctx.fillStyle = "#f0d048";
    const px = Math.max(1, Math.round(size));
    ctx.fillRect(x + Math.round(w * 0.28), Math.round(h * 0.55), px, px + (size > 1 ? 1 : 0));
    ctx.globalAlpha = 1;
  }

  private drawMist(vx: number, vy: number, density: number): void {
    const size = 256;
    if (!this.mistTex) {
      const c = document.createElement("canvas");
      c.width = size;
      c.height = size;
      const g = c.getContext("2d")!;
      let seed = 0x2f6e2b1;
      const rnd = (): number => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
      for (let n = 0; n < 26; n++) {
        const x = rnd() * size;
        const y = rnd() * size;
        const r = 22 + rnd() * 46;
        // Draw each blob nine times so the tile wraps without a seam.
        for (const ox of [-size, 0, size]) {
          for (const oy of [-size, 0, size]) {
            const grad = g.createRadialGradient(x + ox, y + oy, 0, x + ox, y + oy, r);
            grad.addColorStop(0, "rgba(206,212,224,0.20)");
            grad.addColorStop(1, "rgba(206,212,224,0)");
            g.fillStyle = grad;
            g.fillRect(x + ox - r, y + oy - r, r * 2, r * 2);
          }
        }
      }
      this.mistTex = c;
    }
    const ctx = this.ctx;
    const t = this.frameNo;
    ctx.globalAlpha = Math.min(1, density * 2.2);
    for (const [speedX, speedY, shift] of [
      [0.11, 0.03, 0],
      [-0.06, 0.05, 97],
    ]) {
      const ox = (((-vx + t * speedX + shift) % size) + size) % size;
      const oy = (((-vy + t * speedY + shift * 2) % size) + size) % size;
      for (let y = oy - size; y < this.viewH; y += size) {
        for (let x = ox - size; x < this.viewW; x += size) ctx.drawImage(this.mistTex, Math.round(x), Math.round(y));
      }
    }
    ctx.globalAlpha = 1;
  }

  /** Two-frame shimmer on visible water, without rebuilding any chunk. */
  private drawWater(sim: Sim, vx: number, vy: number): void {
    const grid = sim.rt.grid;
    const phase = Math.floor(this.frameNo / 24);
    const x0 = Math.max(0, Math.floor(vx / CELL));
    const y0 = Math.max(0, Math.floor(vy / CELL));
    const x1 = Math.min(grid.w - 1, Math.floor((vx + this.viewW) / CELL));
    const y1 = Math.min(grid.h - 1, Math.floor((vy + this.viewH) / CELL));
    this.ctx.fillStyle = "#9cc8f0";
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        if ((grid.flags[y * grid.w + x] & 4) === 0) continue;
        if (((x * 7 + y * 13 + phase) & 7) !== 0) continue;
        this.ctx.fillRect(x * CELL - vx + ((x + phase) % 5), y * CELL - vy + ((y * 3 + phase) % 6), 2, 1);
      }
    }
  }

  private drawFog(sim: Sim, vx: number, vy: number): void {
    const block = CELL * 2;
    const ctx = this.ctx;
    ctx.fillStyle = "#07060b";
    const x0 = Math.max(0, Math.floor(vx / block));
    const y0 = Math.max(0, Math.floor(vy / block));
    const x1 = Math.min(sim.rt.fogW - 1, Math.floor((vx + this.viewW) / block));
    const y1 = Math.min(sim.rt.fogH - 1, Math.floor((vy + this.viewH) / block));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        if (!fogSeen(sim.zone.fog, sim.rt.fogW, x, y)) ctx.fillRect(x * block - vx, y * block - vy, block, block);
      }
    }
  }

  /** F3: solid cells, occupancy, the load ring, paths, trigger rects. */
  private drawDebug(sim: Sim, vx: number, vy: number): void {
    const ctx = this.ctx;
    const grid = sim.rt.grid;
    const x0 = Math.max(0, Math.floor(vx / CELL));
    const y0 = Math.max(0, Math.floor(vy / CELL));
    const x1 = Math.min(grid.w - 1, Math.floor((vx + this.viewW) / CELL));
    const y1 = Math.min(grid.h - 1, Math.floor((vy + this.viewH) / CELL));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const i = y * grid.w + x;
        if ((grid.flags[i] & BLOCK_MOVE) !== 0) {
          ctx.fillStyle = "#ff303050";
          ctx.fillRect(x * CELL - vx, y * CELL - vy, CELL, CELL);
        }
        if (grid.occupant(i) !== 0) {
          ctx.fillStyle = "#f0d04880";
          ctx.fillRect(x * CELL - vx + 1, y * CELL - vy + 1, CELL - 2, CELL - 2);
        }
      }
    }
    ctx.strokeStyle = "#58a8e8";
    ctx.lineWidth = 1;
    for (const u of sim.zone.units) {
      if (!u.path || !u.awake) continue;
      ctx.beginPath();
      ctx.moveTo(u.x - vx, u.y - vy);
      for (let i = u.pathAt; i < u.path.length; i++) {
        const c = u.path[i];
        ctx.lineTo((c % grid.w) * CELL + 4 - vx, Math.floor(c / grid.w) * CELL + 4 - vy);
      }
      ctx.stroke();
    }
    ctx.strokeStyle = "#78c850";
    for (const [, r] of Object.entries(sim.rt.bp.rects)) {
      if (r.w >= grid.w) continue;
      ctx.strokeRect(r.cx * CELL - vx + 0.5, r.cy * CELL - vy + 0.5, r.w * CELL, r.h * CELL);
    }
    const p = sim.player;
    ctx.strokeStyle = "#f4f0e6";
    ctx.strokeRect(p.x - RING_RADIUS - vx + 0.5, p.y - RING_RADIUS - vy + 0.5, RING_RADIUS * 2, RING_RADIUS * 2);
  }

  get stats(): { chunks: number; chunksBuilt: number; particles: number; scale: number } {
    return { chunks: this.tiles.size, chunksBuilt: this.tiles.built, particles: this.particles.length, scale: this.scale };
  }
}
