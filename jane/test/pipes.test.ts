// The Castle Pipes: one valve, two runs, and four manholes that stay open (DUNGEONS.md 3.4).
//
// Many seeds proven with the stateful flood, the same seed the same culvert, the contract
// names the story leans on, the layout refused when the valve is withheld, a solo bot that
// walks the whole thing and puts every ladder back, and two seats showing the junction's
// plate is faster with a friend and never needs one.

import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import { buildCatalog } from "@/sim/catalog";
import { centre } from "@/sim/grid";
import { bagCount } from "@/sim/inventory";
import { moveProp } from "@/sim/runtime";
import type { Sim } from "@/sim/sim";
import { NO_INPUT } from "@/sim/sim";
import { placeUnit } from "@/sim/units";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { DUNGEONS } from "@/world/dungeon";
import { checkDungeon, contractOf, firstCompletion, lintDef, solveOptionsOf } from "@/world/dungeon/checks";
import { proveTemplate } from "@/world/dungeon/harness";
import { poolOf } from "@/world/dungeon/pools";
import { lintRoom } from "@/world/dungeon/room";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { buildZone } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.pipes;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/pipes.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 29 + i * 5417);

const built = new Map<number, Blueprint>();
function pipes(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("pipes", seed)));
  return bp;
}

describe("the generated pipes", () => {
  it("64 seeds: every one is proven (the stateful solver and C1 to C12) and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = pipes(seed);
      const info = infoOf(bp)!;
      expect(info.layout!.fallback, `seed ${seed} fell back to the hand-placed layout`).toBe(false);
      expect(bp.attempts, `seed ${seed}`).toBeLessThan(ZONE_ATTEMPTS);
      worst = Math.max(worst, bp.attempts);
      expect(checkDungeon(bp, catalog), `seed ${seed}`).toEqual([]);
      const used = info.layout!.placements.map((p) => p.template);
      expect(new Set(used).size).toBe(used.length);
      for (const n of def.nodes) if (n.critical) expect(info.rooms.some((r) => r.node.id === n.id), `seed ${seed}: ${n.id}`).toBe(true);
      const sides = info.rooms.filter((r) => !r.node.critical).length;
      expect(sides).toBeGreaterThanOrEqual(def.budget.sideRooms[0]);
      expect(sides).toBeLessThanOrEqual(def.budget.sideRooms[1]);
    }
    expect((performance.now() - t0) / SEEDS.length, "ms per culvert, proofs included").toBeLessThan(400);
    expect(worst).toBeLessThanOrEqual(6);
  });

  it("is a pure function of (seed, attempt): same seed, same culvert", () => {
    const a = buildDungeon(def, 771, 0);
    const b = buildDungeon(def, 771, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.rects).toEqual(b.rects);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 772, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the hand-placed fallback is a whole, proven culvert", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the contract names the story leans on all exist on every seed, and the four manholes lead to the county", () => {
    const want = [
      "pipes_exit_door", "pipes_plan", "pipes_north_ladder", "pipes_manhole_1", "pipes_junction_plate",
      "pipes_junction_jar", "pipes_stove", "pipes_valve", "pipes_east_ladder", "pipes_manhole_2",
      "pipes_west_chest", "pipes_manhole_3", "pipes_outfall_gear", "pipes_outfall_page", "pipes_manhole_4",
      "pipes_east_penstock", "pipes_west_penstock", "pipes_gate_outfall", "pipes_grating",
    ];
    const derived = contractOf(def);
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = pipes(seed);
      const keys = new Set(bp.props.map((p) => p.key));
      for (const name of want) expect(keys, `seed ${seed}: ${name}`).toContain(name);
      for (const name of derived.props) expect(keys, `seed ${seed}: bound ${name}`).toContain(name);
      for (let n = 1; n <= 4; n++) {
        const hole = bp.props.find((p) => p.key === `pipes_manhole_${n}`)!;
        expect(hole.hidden, `manhole ${n} starts shut`).toBe(true);
        expect(hole.to).toEqual({ zone: "county", mark: `manhole_${n}` });
      }
    }
  });

  it("the valve is a control, and the generator writes its list: one `if`, both ways, nothing forgotten", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = pipes(seed);
      expect(infoOf(bp)!.controls).toEqual([{ state: "east_valve", prop: "pipes_valve" }]);
      const list = bp.props.find((p) => p.key === "pipes_valve")!.use!;
      expect(list[0].do).toBe("if");
      const branch = list[0] as { do: "if"; then: { do: string; prop?: string; flag?: string; value?: number }[]; else: { do: string; prop?: string; flag?: string; value?: number }[] };
      // Turning it to flooded: the west penstock lifts and the east one drops.
      expect(branch.else[0]).toEqual({ do: "flag", flag: "pipes_flooded", value: 1 });
      expect(branch.else).toContainEqual({ do: "unlock", prop: "pipes_west_penstock" });
      expect(branch.else).toContainEqual({ do: "lock", prop: "pipes_east_penstock" });
      // And back again, in one list, so neither run can be forgotten in one direction.
      expect(branch.then[0]).toEqual({ do: "flag", flag: "pipes_flooded", value: 0 });
      expect(branch.then).toContainEqual({ do: "lock", prop: "pipes_west_penstock" });
      expect(branch.then).toContainEqual({ do: "unlock", prop: "pipes_east_penstock" });
      // As the culvert is made: east open, west under water.
      const east = bp.props.find((p) => p.key === "pipes_east_penstock")!;
      const west = bp.props.find((p) => p.key === "pipes_west_penstock")!;
      expect([east.locked ?? false, west.locked]).toEqual([false, true]);
      expect(["pipes_penstock_h", "pipes_penstock_v"]).toContain(west.def);
    }
  });

  it("the solver rejects the same layout with the valve withheld: the west run really is the valve's", () => {
    for (const seed of SEEDS.slice(0, 4)) {
      const opts = { ...solveOptionsOf(def), trace: true };
      const bp = pipes(seed);
      const whole = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, opts);
      expect(whole.errors).toEqual([]);
      expect(whole.trace!.layers).toEqual([0, 1]);
      const without = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { props: ["pipes_valve"] } });
      expect(without.ok).toBe(false);
      expect(without.trace!.layers).toEqual([0]);
      // And without Repair not one manhole opens, which is the whole point of them.
      const noRepair = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { verbs: ["repair"] } });
      expect(noRepair.trace!.firedAt.pipes_north_ladder).toBeUndefined();
    }
  });

  it("the first completion is the mission in order, and inside its band", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const walk = firstCompletion(pipes(seed), catalog, infoOf(pipes(seed))!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual(["sump", "north_run", "junction", "chamber", "valve_house", "east_run", "west_run", "outfall"]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });
});

// --- a solo bot, the whole culvert -----------------------------------------------------------

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

function cast(sim: Sim, spell: string, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  p.cooldowns = {};
  p.gcd = 0;
  sim.command({ t: "cast", spell });
  idle(sim, 2);
}

describe("a solo bot in the generated pipes", () => {
  const botCatalog = yardCatalog();
  for (const seed of [5, 4242]) {
    it(`seed ${seed}: the plan, the first ladder, the brazier, the valve, both runs, the outfall`, { timeout: 120_000 }, () => {
      const sim = simIn(botCatalog, "pipes", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      // What the story has given her by now: Icebolt at home, Repair in the mine,
      // Explosion in the museum, Grow in the forest.
      for (const spell of def.givenVerbs) sim.command({ t: "dev", dev: { op: "learn", spell } });
      idle(sim, 2);

      // The plan on the wall is the map of the runs.
      useProp(sim, "pipes_plan");
      talkThrough(sim);

      // The north run: iron in a box, and the first ladder put back for good.
      useProp(sim, "pipes_north_chest");
      expect(bagCount(p, "iron")).toBeGreaterThanOrEqual(2);
      expect(prop("pipes_manhole_1").hidden).toBe(true);
      cast(sim, "repair", "pipes_north_ladder");
      expect(prop("pipes_manhole_1").hidden).toBe(false);
      expect(prop("pipes_manhole_1").to).toEqual({ zone: "county", mark: "manhole_1" });

      // The chamber off the junction: a brazier, and where she wakes if it goes badly.
      useProp(sim, "pipes_stove");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("pipes");
      useProp(sim, "pipes_chamber_chest");

      // The east run is dry as she finds it. The second ladder, and a jar on the silt shelf.
      expect(prop("pipes_east_penstock").locked ?? false).toBe(false);
      useProp(sim, "pipes_east_chest");
      cast(sim, "repair", "pipes_east_ladder");
      expect(prop("pipes_manhole_2").hidden).toBe(false);

      // The valve. One turn and the west run is the dry one.
      useProp(sim, "pipes_valve_chest");
      expect(prop("pipes_west_penstock").locked).toBe(true);
      useProp(sim, "pipes_valve");
      expect(sim.state.flags.pipes_flooded).toBe(1);
      expect([prop("pipes_west_penstock").locked, prop("pipes_east_penstock").locked]).toEqual([false, true]);

      // The west run: the outfall key and the third ladder.
      useProp(sim, "pipes_west_chest");
      expect(bagCount(p, "key_outfall")).toBe(1);
      cast(sim, "repair", "pipes_west_ladder");
      expect(prop("pipes_manhole_3").hidden).toBe(false);

      // The outfall: the gear lifts the grating home, a page, and the Works yard manhole.
      useProp(sim, "pipes_gate_outfall");
      expect(prop("pipes_grating").locked).toBe(true);
      useProp(sim, "pipes_outfall_gear");
      expect(sim.state.flags.pipes_outfall_open).toBe(1);
      expect(prop("pipes_grating").locked).toBe(false);
      const spirit = p.spirit;
      useProp(sim, "pipes_outfall_page");
      expect(p.spirit).toBe(spirit + 6);
      useProp(sim, "pipes_outfall_chest");
      cast(sim, "repair", "pipes_outfall_ladder");
      expect(prop("pipes_manhole_4").hidden).toBe(false);
      expect(prop("pipes_manhole_4").to).toEqual({ zone: "county", mark: "manhole_4" });

      // And the short way back to the fire is open.
      expect(walkToProp(sim, "pipes_stove")).toBe(true);
    });
  }
});

// --- the pipes' own rooms, held to the lint and the harness ----------------------------------

describe("the pipes' room templates", () => {
  it("the mission lints clean and every pool it names exists, with a door for every edge", () => {
    expect(lintDef(def, catalog)).toEqual([]);
    for (const node of def.nodes) {
      const pool = poolOf(node.pool);
      expect(pool.length, `pool ${node.pool} is empty`).toBeGreaterThan(0);
      for (const t of pool) {
        expect(lintRoom(t), t.id).toEqual([]);
        for (const h of node.holds) expect(t.sockets.some((s) => s.id === h.socket), `${t.id} has no socket ${h.socket}`).toBe(true);
        for (const b of node.binds) {
          const found = t.sockets.some((s) => s.id === b.from) || t.marks.some((m) => m.id === b.from) || t.rects.some((r) => r.id === b.from);
          expect(found, `${t.id} has nothing called ${b.from}`).toBe(true);
        }
        const edges = def.edges.filter((e) => e.kind.t !== "sight" && (e.from === node.id || e.to === node.id)).length;
        expect(t.doors.length, `${t.id}: ${edges} edges`).toBeGreaterThanOrEqual(edges);
      }
    }
  });

  it("every template proves its grants and its blocks, alone, from every door, in every transform", () => {
    for (const node of def.nodes) {
      // The valve's room is the one exception: a control's list reaches into other rooms by
      // definition, and the harness stamps a room alone (see the museum's breaker).
      for (const t of poolOf(node.pool)) expect(proveTemplate(def, node, t, catalog), `${t.id} as ${node.id}`).toEqual([]);
    }
  });
});

// --- the rows this zone adds, and the art for them -------------------------------------------

describe("the pipes' rows", () => {
  it("every prop this zone adds has a sprite of the right footprint", () => {
    for (const id of ["manhole", "valve", "ladder_broken", "pipes_penstock_h", "pipes_penstock_v", "pipes_stove"]) {
      const d = catalog.props[id];
      expect(d, id).toBeDefined();
      const s = PROP_SPRITES[d.sprite];
      expect(s, `props.${id} -> sprite "${d.sprite}"`).toBeDefined();
      expect(s.w, `props.${id} sprite width`).toBe(d.w * 8);
      expect(s.h, `props.${id} sprite height`).toBeGreaterThanOrEqual(d.h * 8);
      expect(s.frames.base, `props.${id} base frame`).toBeDefined();
    }
    expect(catalog.props.ladder_broken.answers).toBe("repair");
    expect(ICONS[catalog.items.key_outfall.icon]).toBeDefined();
    for (const id of ["notice_pipes", "notice_junction", "notice_valve", "pipes_stove"]) {
      expect(catalog.dialogue[id], `dialogue.${id}`).toBeDefined();
    }
  });
});

// --- two seats: the junction's plate is faster with a friend and never needs one --------------

describe("the pipes with two seats", () => {
  it("the jar in the niche: a friend on the plate, or the barrel, or nothing at all", () => {
    const sim = simIn(yardCatalog(), "pipes", 5);
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const jar = sim.rt.propsByKey.get("pipes_junction_jar")!;
    const plate = sim.rt.propsByKey.get("pipes_junction_plate")!;
    const barrel = sim.rt.propsByKey.get("pipes_junction_barrel")!;
    expect(jar.locked).toBe(true);

    // A friend standing on it does it at once, and steps off again.
    placeUnit(sim, friend, centre(plate.cx + 1), centre(plate.cy + 1));
    for (let t = 0; t < 8; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(jar.locked).toBe(false);
    placeUnit(sim, friend, centre(plate.cx + 8), centre(plate.cy + 8));
    for (let t = 0; t < 8; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(jar.locked).toBe(true);

    // And the barrel does the same job for one player, which is the rule.
    moveProp(sim, barrel, plate.cx, plate.cy);
    for (let t = 0; t < 12; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(jar.locked).toBe(false);
  });
});
