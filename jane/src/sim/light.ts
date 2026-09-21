// Light that means something to the sim. Everywhere else in the county a lamp is safety; in
// the Factory it is a sentry's eye, in the Burial it is what keeps the dead off, and in
// Butterfly Forest it is where things grow. All three ask one question: is this point lit?
//
//   props only     Her own glow, a bolt in flight and a bat's red lamp are presentation. If
//                  they counted, she would light herself up for every sentry by casting.
//   one rule       `propLightShowing` is THE rule for whether a prop's light is on. The renderer
//                  asks it too, so what she sees lit and what the sim calls lit cannot drift.
//   no cache       The answer is read off the prop buckets every time it is asked. A cached
//                  light map would have to be told about every lever, every frost-lit torch,
//                  every pushed brazier and both ends of the night, and the first one forgotten
//                  would make a loaded game differ from the game that was saved.
//   cheap          A query visits the few 16-cell blocks a light could reach from, without
//                  allocating. The path finder asks once per cell per search, through `LitField`,
//                  which gathers the lights near the search window first.

import type { PropDef } from "@/sim/catalog";
import { CELL, TICKS_PER_HOUR } from "@/sim/constants";
import { PROP_BLOCK, type World, type ZoneRuntime } from "@/sim/runtime";
import type { GameState, Prop } from "@/sim/state";

/** Lamp posts burn from 18:30 to 06:30, as they did in 2020. Day-only light (a sunbeam) is the other half. */
const LAMPS_ON = 18.5 * TICKS_PER_HOUR;
const LAMPS_OFF = 6.5 * TICKS_PER_HOUR;

export function lampsLit(state: GameState): boolean {
  return state.clock > LAMPS_ON || state.clock < LAMPS_OFF;
}

/** Is this prop's light on? `lamps` is `lampsLit`, passed in so a loop asks the clock once. */
export function propLightShowing(def: PropDef, p: Prop, lamps: boolean): boolean {
  if (!def.light || p.hidden) return false;
  if (def.lightWhenOn && !p.on) return false;
  if (def.nightOnly && !lamps) return false;
  if (def.dayOnly && lamps) return false;
  return true;
}

/**
 * The radial sprite has a long soft tail: "the last third barely lifts the dark". So a point
 * counts as lit inside two thirds of the radius, which is where it looks lit.
 */
function reachSq(radius: number): number {
  return (radius * radius * 4) / 9;
}

/**
 * True if a prop's light covers the point. `warmOnly` leaves out lights marked `cold`: the
 * Burial's blue torches show what is there and keep nothing off.
 */
export function litAt(w: World, x: number, y: number, warmOnly = false): boolean {
  const rt = w.rt;
  const reach = rt.lightReach;
  if (reach <= 0) return false;
  const props = w.catalog.props;
  const lamps = lampsLit(w.state);
  const block = PROP_BLOCK;
  const cell = CELL;
  const bw = rt.propBlocksW;
  // A prop sits in the bucket of its origin cell and its light is at its middle, so reach back by a footprint too.
  const bx0 = Math.max(0, Math.floor((Math.floor((x - reach) / cell) - rt.propReachW) / block));
  const by0 = Math.max(0, Math.floor((Math.floor((y - reach) / cell) - rt.propReachH) / block));
  const bx1 = Math.min(bw - 1, Math.floor(Math.floor((x + reach) / cell) / block));
  const by1 = Math.min(rt.propBlocksH - 1, Math.floor(Math.floor((y + reach) / cell) / block));
  for (let by = by0; by <= by1; by++) {
    for (let bx = bx0; bx <= bx1; bx++) {
      const bucket = rt.propBuckets[by * bw + bx];
      if (!bucket) continue;
      for (let i = 0; i < bucket.length; i++) {
        const p = bucket[i];
        const def = props[p.def];
        const light = def.light;
        if (!light || (warmOnly && light.cold) || !propLightShowing(def, p, lamps)) continue;
        const dx = (p.cx + def.w / 2) * cell - x;
        const dy = (p.cy + def.h / 2) * cell - y;
        if (dx * dx + dy * dy <= reachSq(light.radius)) return true;
      }
    }
  }
  return false;
}

/**
 * The lights near one path search, gathered once, so the search can ask about thousands of
 * cells for the price of a short loop each. Scratch is reused; nothing here is state.
 */
export class LitField {
  private readonly xs: number[] = [];
  private readonly ys: number[] = [];
  private readonly rs: number[] = [];
  private n = 0;
  // Read per cell per search, so they are fields, not imported names (ENGINE.md 8.2).
  private readonly cell = CELL;
  private readonly half = CELL / 2;

  /** Collect every showing light that could reach into the cell rect (inclusive). */
  gather(w: World, cx0: number, cy0: number, cx1: number, cy1: number, warmOnly: boolean): void {
    this.n = 0;
    const rt: ZoneRuntime = w.rt;
    const reach = rt.lightReach;
    if (reach <= 0) return;
    const props = w.catalog.props;
    const lamps = lampsLit(w.state);
    const block = PROP_BLOCK;
    const cell = CELL;
    const pad = Math.ceil(reach / cell);
    const bw = rt.propBlocksW;
    const bx0 = Math.max(0, Math.floor((cx0 - pad - rt.propReachW) / block));
    const by0 = Math.max(0, Math.floor((cy0 - pad - rt.propReachH) / block));
    const bx1 = Math.min(bw - 1, Math.floor((cx1 + pad) / block));
    const by1 = Math.min(rt.propBlocksH - 1, Math.floor((cy1 + pad) / block));
    for (let by = by0; by <= by1; by++) {
      for (let bx = bx0; bx <= bx1; bx++) {
        const bucket = rt.propBuckets[by * bw + bx];
        if (!bucket) continue;
        for (let i = 0; i < bucket.length; i++) {
          const p = bucket[i];
          const def = props[p.def];
          const light = def.light;
          if (!light || (warmOnly && light.cold) || !propLightShowing(def, p, lamps)) continue;
          this.xs[this.n] = (p.cx + def.w / 2) * cell;
          this.ys[this.n] = (p.cy + def.h / 2) * cell;
          this.rs[this.n] = reachSq(light.radius);
          this.n++;
        }
      }
    }
  }

  get count(): number {
    return this.n;
  }

  /** Is the middle of this cell lit by anything gathered? */
  cellLit(cx: number, cy: number): boolean {
    const x = cx * this.cell + this.half;
    const y = cy * this.cell + this.half;
    for (let i = 0; i < this.n; i++) {
      const dx = this.xs[i] - x;
      const dy = this.ys[i] - y;
      if (dx * dx + dy * dy <= this.rs[i]) return true;
    }
    return false;
  }
}
