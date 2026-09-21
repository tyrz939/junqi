// The county is about to hold thousands of units and props. A tick must cost what is
// AWAKE around the party, not what exists: this test fills the far side of the county
// with sleepers and holds the scheduler to a budget while Jane walks about at home.

import { describe, expect, it } from "vitest";
import { CELL, RING_RADIUS } from "@/sim/constants";
import { centre, F_PROP_LOS, F_PROP_SOLID } from "@/sim/grid";
import { focusOf } from "@/sim/interact";
import { addProp, addUnit, moveProp, propsInCells, propsNear } from "@/sim/runtime";
import { Sim, type InputFrame } from "@/sim/sim";
import type { Prop, Unit } from "@/sim/state";
import { createUnit } from "@/sim/units";
import { walkTo , yardCatalog } from "./bot";

const catalog = yardCatalog();

/** Nothing extra within this many px of her, on either axis. The ring is 384. */
const KEEP_CLEAR = 1200;
const EXTRA_UNITS = 3000;
const EXTRA_PROPS = 8000;
const BUDGET_MS = 2;

const UNIT_DEFS = ["skeleton", "rat", "bat", "bandit", "spider"];
// Small and large, solid and flat, lit and unlit, pushable and carried: every capability the index serves.
const PROP_DEFS = ["rock", "crate", "torch", "lamp_post", "herb", "apple_tree", "car_wreck", "sign"];

/** What createZoneState makes of a bare blueprint row. Asleep: the ring would have left it so. */
function makeProp(sim: Sim, defId: string, cx: number, cy: number): Prop {
  const def = catalog.props[defId];
  return {
    id: sim.state.nextId++,
    key: "",
    def: defId,
    cx,
    cy,
    solid: def.gate ? false : def.solid,
    hidden: false,
    locked: false,
    used: false,
    on: false,
    keyTag: "",
    to: null,
    loot: null,
    use: null,
    release: null,
    needs: null,
    talk: "",
    label: "",
    nightLock: "",
    awake: false,
  };
}

/** A lattice over the whole county, every third cell, leaving a wide hole around her. Terrain is ignored. */
function fillCounty(sim: Sim): { units: Unit[]; props: Prop[] } {
  const p = sim.player;
  const grid = sim.rt.grid;
  const units: Unit[] = [];
  const props: Prop[] = [];
  let i = 0;
  for (let cy = 2; cy < grid.h - 4; cy += 3) {
    for (let cx = 2; cx < grid.w - 6; cx += 3) {
      if (Math.abs(centre(cx) - p.x) < KEEP_CLEAR && Math.abs(centre(cy) - p.y) < KEEP_CLEAR) continue;
      if (i++ % 11 < 3) {
        if (units.length >= EXTRA_UNITS) continue;
        const u = createUnit(sim.state, catalog, UNIT_DEFS[units.length % UNIT_DEFS.length], "", centre(cx), centre(cy));
        u.awake = false;
        // One in seven is a corpse on a short clock: sleepers must still respawn on schedule.
        if (units.length % 7 === 0) {
          u.alive = false;
          u.hp = 0;
          u.respawn = 300;
        }
        addUnit(sim, u);
        units.push(u);
      } else if (props.length < EXTRA_PROPS) {
        const prop = makeProp(sim, PROP_DEFS[props.length % PROP_DEFS.length], cx, cy);
        addProp(sim, prop);
        props.push(prop);
      }
    }
  }
  return { units, props };
}

/** Walk a square, sprinting half the time: she crosses ring blocks all the way round. */
function stroll(t: number): InputFrame {
  const phase = Math.floor(t / 100) % 4;
  return { mx: [1, 0, -1, 0][phase], my: [0, 1, 0, -1][phase], sprint: t % 200 < 100, useHeld: false, ax: 0, ay: 0 };
}

function median(times: number[]): number {
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

describe("tick budget", () => {
  it(`${EXTRA_UNITS} units and ${EXTRA_PROPS} props asleep across the county: median tick under ${BUDGET_MS} ms`, () => {
    const sim = Sim.newGame(catalog, 2026);
    expect(sim.me.zone).toBe("county");
    const before = { units: sim.zone.units.length, props: sim.zone.props.length };
    const extra = fillCounty(sim);
    expect(extra.units.length).toBe(EXTRA_UNITS);
    expect(extra.props.length).toBe(EXTRA_PROPS);
    expect(sim.zone.units.length).toBe(before.units + EXTRA_UNITS);
    expect(sim.zone.props.length).toBe(before.props + EXTRA_PROPS);
    expect(KEEP_CLEAR).toBeGreaterThan(RING_RADIUS * 2);

    const x0 = sim.player.x;
    const y0 = sim.player.y;
    let far = 0;
    const times: number[] = [];
    for (let t = 0; t < 600; t++) {
      const frame = stroll(t);
      const a = performance.now();
      sim.tick(frame);
      times.push(performance.now() - a);
      far = Math.max(far, Math.abs(sim.player.x - x0) + Math.abs(sim.player.y - y0));
    }
    // The median, not the mean: a GC pause or the JIT warming up is not the scheduler's doing.
    const mid = median(times);
    const mean = times.reduce((a, b) => a + b, 0) / times.length;
    const p90 = [...times].sort((a, b) => a - b)[Math.floor(times.length * 0.9)];
    console.log(`tick budget: median ${mid.toFixed(4)} ms, mean ${mean.toFixed(4)} ms, p90 ${p90.toFixed(4)} ms, worst ${Math.max(...times).toFixed(3)} ms`);
    expect(mid).toBeLessThan(BUDGET_MS);

    // She walked, and the ring followed her without waking the far side of the county.
    expect(far).toBeGreaterThan(64);
    expect(extra.units.every((u) => !u.awake)).toBe(true);
    expect(extra.props.every((p) => !p.awake)).toBe(true);
    expect(sim.rt.grid.occupiedCount).toBeLessThan(before.units + 1);
    // Sleepers' clocks ran all the same: every corpse on a 300-tick clock is up again.
    const corpses = extra.units.filter((u) => u.respawn === 300);
    expect(corpses.length).toBeGreaterThan(400);
    expect(corpses.every((u) => u.alive && u.deadFor === 0)).toBe(true);
    // Solid sleepers are still walls.
    const wreck = extra.props.find((p) => p.def === "car_wreck")!;
    expect(sim.rt.grid.flagsAt(wreck.cx + 4, wreck.cy + 2) & F_PROP_SOLID).toBe(F_PROP_SOLID);

    // And home is as it was: the skeleton in the yard still takes exception to her.
    const skeleton = sim.rt.unitsByKey.get("yard_skeleton")!;
    expect(skeleton.combat).toBe("idle");
    walkTo(sim, skeleton.x, skeleton.y, 20);
    expect(skeleton.combat).toBe("combat");
    expect(skeleton.target).toBe(sim.player.id);
  });

  it("the buckets find what a pass over the whole zone finds, in id order", () => {
    const sim = Sim.newGame(catalog, 7);
    const extra = fillCounty(sim);
    const rt = sim.rt;
    // A cheap fixed sequence; the test must not borrow the sim's rng.
    let seed = 12345;
    const next = (n: number): number => {
      seed = (seed * 1103515245 + 12345) & 0x7fffffff;
      return seed % n;
    };
    const touching = (cx0: number, cy0: number, cx1: number, cy1: number): Prop[] =>
      sim.zone.props.filter((p) => {
        const def = catalog.props[p.def];
        return p.cx <= cx1 && p.cy <= cy1 && p.cx + def.w - 1 >= cx0 && p.cy + def.h - 1 >= cy0;
      });
    const check = (cx0: number, cy0: number, cx1: number, cy1: number): void => {
      const found = propsInCells(rt, cx0, cy0, cx1, cy1);
      for (let i = 1; i < found.length; i++) expect(found[i].id).toBeGreaterThan(found[i - 1].id);
      const ids = new Set(found.map((p) => p.id));
      for (const p of touching(cx0, cy0, cx1, cy1)) expect(ids.has(p.id), `prop ${p.id} (${p.def}) at ${p.cx},${p.cy}`).toBe(true);
      // A superset, but a local one: never the zone.
      expect(found.length).toBeLessThan(600);
    };
    for (let i = 0; i < 200; i++) {
      const cx = next(rt.grid.w);
      const cy = next(rt.grid.h);
      check(cx, cy, cx + next(24), cy + next(24));
    }
    // A single cell under the far corner of the widest footprint still finds it.
    const wreck = extra.props.find((p) => p.def === "car_wreck" && p.cx % 16 > 11)!;
    expect(propsInCells(rt, wreck.cx + 4, wreck.cy + 2, wreck.cx + 4, wreck.cy + 2)).toContain(wreck);

    // Moved props are found where they went and not where they were.
    for (let i = 0; i < 300; i++) {
      const p = extra.props[next(extra.props.length)];
      const from = { cx: p.cx, cy: p.cy };
      moveProp(sim, p, next(rt.grid.w - 6), next(rt.grid.h - 4));
      expect(propsInCells(rt, p.cx, p.cy, p.cx, p.cy)).toContain(p);
      if (Math.abs(p.cx - from.cx) > 40 || Math.abs(p.cy - from.cy) > 40) {
        expect(propsInCells(rt, from.cx, from.cy, from.cx, from.cy)).not.toContain(p);
      }
    }
    for (let i = 0; i < 100; i++) {
      const cx = next(rt.grid.w);
      const cy = next(rt.grid.h);
      check(cx, cy, cx + next(24), cy + next(24));
    }
    // Every prop is in exactly one bucket.
    let held = 0;
    for (const bucket of rt.propBuckets) held += bucket ? bucket.length : 0;
    expect(held).toBe(sim.zone.props.length);

    // After the end-of-tick flush the grid's prop flags equal a full re-stamp.
    sim.tick(stroll(0));
    const propBits = (): Uint8Array => rt.grid.flags.map((f) => f & (F_PROP_SOLID | F_PROP_LOS));
    const local = propBits();
    rt.propFlagsDirty = true;
    sim.tick(stroll(1));
    expect(Buffer.compare(Buffer.from(local), Buffer.from(propBits()))).toBe(0);

    // USE still finds the letterbox-sized things at her feet, and only those.
    const rock = makeProp(sim, "rock", Math.floor(sim.player.x / CELL) + 1, Math.floor(sim.player.y / CELL));
    rock.awake = true;
    addProp(sim, rock);
    expect(propsNear(rt, sim.player.x, sim.player.y, 12)).toContain(rock);
    const f = focusOf(sim, sim.player);
    expect(f && f.kind === "prop" ? f.id : 0).toBe(rock.id);
  });
});
