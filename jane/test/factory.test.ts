// Goldskin Works: light is how the machines see you (DUNGEONS.md 3.4).
//
// Many seeds proven with the stateful flood, the same seed the same works, the contract names
// the story leans on, the layout refused when the starting board is withheld and when Spark is,
// a solo bot that crosses the building dark, finds the verb, puts the power on and crosses it
// again lit, and two seats showing the lock-in can be followed and the grids spare a friend.

import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import { UNIT_SPRITES } from "@/art/units";
import { buildCatalog } from "@/sim/catalog";
import { centre } from "@/sim/grid";
import { bagCount } from "@/sim/inventory";
import type { Sim } from "@/sim/sim";
import { NO_INPUT } from "@/sim/sim";
import { propCentre } from "@/sim/runtime";
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
import { idle, simIn, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.factory;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/factory.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 13 + i * 7919);

const built = new Map<number, Blueprint>();
function factory(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("factory", seed)));
  return bp;
}

describe("the generated works", () => {
  it("64 seeds: every one is proven (the stateful solver and C1 to C12) and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = factory(seed);
      const info = infoOf(bp)!;
      expect(info.layout!.fallback, `seed ${seed} fell back to the hand-placed layout`).toBe(false);
      expect(bp.attempts, `seed ${seed}`).toBeLessThan(ZONE_ATTEMPTS);
      worst = Math.max(worst, bp.attempts);
      expect(checkDungeon(bp, catalog), `seed ${seed}`).toEqual([]);
      const used = info.layout!.placements.map((p) => p.template);
      expect(new Set(used).size).toBe(used.length);
      for (const n of def.nodes) if (n.critical) expect(info.rooms.some((r) => r.node.id === n.id), `seed ${seed}: ${n.id}`).toBe(true);
      const sides = info.rooms.filter((r) => !r.node.critical).length;
      expect(sides).toBe(1);
    }
    expect((performance.now() - t0) / SEEDS.length, "ms per works, proofs included").toBeLessThan(1500);
    expect(worst).toBeLessThanOrEqual(6);
  });

  it("is a pure function of (seed, attempt): same seed, same works", () => {
    const a = buildDungeon(def, 3131, 0);
    const b = buildDungeon(def, 3131, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.rects).toEqual(b.rects);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 3132, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the hand-placed fallback is a whole, proven works", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the contract names the story leans on all exist on every seed", () => {
    const want = [
      "factory_exit_door", "factory_plan", "factory_loading_lamp", "factory_light_notice",
      "factory_lockers_chest", "factory_line_lamp_a", "factory_line_lamp_b", "factory_line_fuse",
      "factory_stove", "factory_rack", "factory_vent", "factory_orb", "factory_generator",
      "factory_board", "factory_gen_socket", "factory_gen_lamp", "factory_call_box",
      "factory_weak_wall", "factory_press_lamp", "factory_foreman_key_chest",
      "factory_assembly_lamp_a", "factory_assembly_lamp_b", "factory_grid_a",
      "factory_big_jar", "factory_reward_chest", "factory_roller_socket",
      "factory_gate_line", "factory_gate_press", "factory_stores_shutter", "factory_press_gate",
      "factory_gate_assembly", "factory_gen_shutter", "factory_roller_door",
    ];
    const derived = contractOf(def);
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = factory(seed);
      const keys = new Set(bp.props.map((p) => p.key));
      for (const name of want) expect(keys, `seed ${seed}: ${name}`).toContain(name);
      for (const name of derived.props) expect(keys, `seed ${seed}: bound ${name}`).toContain(name);
      for (const u of ["charge_hand", "foreman", "factory_line_hauler", "factory_press_hauler"]) {
        expect(bp.units.some((x) => x.key === u), `seed ${seed}: ${u}`).toBe(true);
      }
      // The vent is a one-way drop into the generator hall: a `to` that names this very zone.
      const vent = bp.props.find((p) => p.key === "factory_vent")!;
      expect(vent.to).toEqual({ zone: "factory", mark: "factory_gen_in" });
    }
  });

  it("the starting board is the control, and the generator writes its list: one `if`, both ways", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = factory(seed);
      expect(infoOf(bp)!.controls).toEqual([{ state: "factory_lit", prop: "factory_board" }]);
      const list = bp.props.find((p) => p.key === "factory_board")!.use!;
      expect(list[0].do).toBe("if");
      const branch = list[0] as { do: "if"; then: { do: string; prop?: string; flag?: string; value?: number; on?: boolean }[]; else: typeof list };
      // The building starts dark: the first spark is the `else` branch, which lights it.
      expect(branch.else[0]).toEqual({ do: "flag", flag: "factory_power", value: 1 });
      expect(branch.else).toContainEqual({ do: "unlock", prop: "factory_stores_shutter" });
      expect(branch.else).toContainEqual({ do: "switch", prop: "factory_line_lamp_a", on: true });
      expect(branch.else).toContainEqual({ do: "switch", prop: "factory_assembly_lamp_b", on: true });
      expect(branch.then).toContainEqual({ do: "lock", prop: "factory_stores_shutter" });
      expect(branch.then).toContainEqual({ do: "switch", prop: "factory_line_lamp_a", on: false });
      // Every lamp but the loading bay's is out as the building is made; the shutter is down.
      for (const key of ["factory_line_lamp_a", "factory_line_lamp_b", "factory_gen_lamp", "factory_press_lamp", "factory_assembly_lamp_a", "factory_assembly_lamp_b"]) {
        expect(bp.props.find((p) => p.key === key)!.on ?? false, key).toBe(false);
      }
      expect(bp.props.find((p) => p.key === "factory_loading_lamp")!.on).toBe(true);
      expect(bp.props.find((p) => p.key === "factory_bench_lamp")!.on).toBe(true);
      expect(bp.props.find((p) => p.key === "factory_stores_shutter")!.locked).toBe(true);
      expect(["factory_shutter_h", "factory_shutter_v"]).toContain(bp.props.find((p) => p.key === "factory_stores_shutter")!.def);
    }
  });

  it("the solver rejects the layout without the board, without Spark, and without the call box", () => {
    for (const seed of SEEDS.slice(0, 4)) {
      const bp = factory(seed);
      const opts = { ...solveOptionsOf(def), trace: true };
      const whole = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, opts);
      expect(whole.errors).toEqual([]);
      expect(whole.trace!.layers).toEqual([0, 1]);
      // The stores are behind a shutter that only a started generator lifts.
      const noBoard = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { props: ["factory_board"] } });
      expect(noBoard.trace!.layers).toEqual([0]);
      // Spark is found inside, and everything after it needs it.
      const noSpark = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { verbs: ["spark"] } });
      expect(noSpark.ok).toBe(false);
      expect(noSpark.errors.join("\n")).toMatch(/factory_reward_chest|factory_big_jar/);
      // And the office is behind a wall a hauler has to be called through.
      const noWall = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { flags: ["factory_wall_down"] } });
      expect(noWall.ok).toBe(false);
    }
  });

  it("the first completion is the mission in order, and inside its band", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = factory(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual([
        "yard", "loading", "lockers", "line_a", "time_office", "vent",
        "generator", "gen_shutter", "press_hall", "office", "assembly", "roller_door",
      ]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });
});

// --- a solo bot, the whole works, dark and then lit --------------------------------------------

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

/**
 * Spark is a bolt: stand beside the thing, aim at its middle, and throw. A bolt dies on what
 * blocks sight, so try each side of the footprint until the thing answers.
 */
function spark(sim: Sim, key: string, done?: () => boolean): void {
  const prop = sim.rt.propsByKey.get(key)!;
  const d = sim.catalog.props[prop.def];
  const landed = done ?? ((): boolean => prop.on === true);
  const c = propCentre(sim.catalog, prop);
  const spots: [number, number][] = [
    [c.x, (prop.cy + d.h) * 8 + 6],
    [c.x, prop.cy * 8 - 6],
    [prop.cx * 8 - 6, c.y],
    [(prop.cx + d.w) * 8 + 6, c.y],
  ];
  for (const [x, y] of spots) {
    if (landed()) return;
    if (!walkTo(sim, x, y, 5)) continue;
    const p = sim.player;
    const dx = c.x - p.x;
    const dy = c.y - p.y;
    const len = Math.sqrt(dx * dx + dy * dy) || 1;
    p.cooldowns = {};
    p.gcd = 0;
    p.mp = 999;
    sim.setAim(dx / len, dy / len);
    sim.command({ t: "cast", spell: "spark" });
    idle(sim, 30);
  }
  expect(landed(), `spark ${key}`).toBe(true);
}

/** The rect a floor grid stands in, whatever the generator called it. */
function dropRect(sim: Sim, gridKey: string): { cx: number; cy: number; w: number; h: number } {
  const grid = sim.rt.propsByKey.get(gridKey)!;
  const found = Object.entries(sim.rt.bp.rects).find(
    ([id, r]) => id.includes("drop") && grid.cx >= r.cx && grid.cx < r.cx + r.w && grid.cy >= r.cy && grid.cy < r.cy + r.h,
  );
  expect(found, `no drop rect round ${gridKey}`).toBeDefined();
  return found![1];
}

function cast(sim: Sim, spell: string, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  p.cooldowns = {};
  p.gcd = 0;
  sim.command({ t: "cast", spell });
  idle(sim, 2);
}

function killUnit(sim: Sim, key: string): void {
  const u = sim.rt.unitsByKey.get(key)!;
  walkTo(sim, u.x, u.y, 40);
  u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 4);
}

describe("a solo bot in the generated works", () => {
  const botCatalog = yardCatalog();
  for (const seed of [13, 2026]) {
    it(`seed ${seed}: the dark crossing, the orb, the power, and the lit crossing back`, { timeout: 180_000 }, () => {
      const sim = simIn(botCatalog, "factory", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      for (const spell of def.givenVerbs) sim.command({ t: "dev", dev: { op: "learn", spell } });
      idle(sim, 2);

      // The plan on the wall, and the lesson under the one lamp still burning.
      useProp(sim, "factory_plan");
      talkThrough(sim);
      expect(prop("factory_loading_lamp").on).toBe(true);
      useProp(sim, "factory_light_notice");
      talkThrough(sim);

      // The lockers: two plain keys and four bars of iron.
      useProp(sim, "factory_lockers_chest");
      expect(bagCount(p, "key_generic")).toBe(2);
      expect(bagCount(p, "iron")).toBe(4);

      // No. 1 line, in the dark, with a hauler walking it.
      useProp(sim, "factory_gate_line");
      expect(prop("factory_line_lamp_a").on ?? false).toBe(false);

      // The time office: a stove, and the cards all stamped in.
      useProp(sim, "factory_stove");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("factory");
      useProp(sim, "factory_rack");
      talkThrough(sim);

      // The vent: a one-way drop that puts her on the generator hall floor.
      const hall = sim.rt.bp.rects.factory_gen_hall;
      useProp(sim, "factory_vent");
      idle(sim, 4);
      expect(sim.rt.grid.w > 0 && p.x >= hall.cx * 8 && p.x < (hall.cx + hall.w) * 8).toBe(true);
      expect(p.y >= hall.cy * 8 && p.y < (hall.cy + hall.h) * 8).toBe(true);

      // The orb will not open while the Charge Hand and his sentry are up.
      useProp(sim, "factory_orb");
      expect(sim.me.dialogue).toBeNull();
      expect(p.book).not.toContain("spark");
      killUnit(sim, "charge_hand");
      killUnit(sim, "factory_gen_sentry");
      useProp(sim, "factory_orb");
      talkThrough(sim);
      expect(p.book).toContain("spark");

      // Repair the generator with the iron, and the starting board wakes.
      expect(prop("factory_board").hidden).toBe(true);
      cast(sim, "repair", "factory_generator");
      expect(prop("factory_board").hidden).toBe(false);

      // One spark, and every lamp in the building comes on at once.
      expect(sim.state.flags.factory_power ?? 0).toBe(0);
      spark(sim, "factory_board");
      expect(sim.state.flags.factory_power).toBe(1);
      for (const key of ["factory_line_lamp_a", "factory_gen_lamp", "factory_press_lamp", "factory_assembly_lamp_a"]) {
        expect(prop(key).on, key).toBe(true);
      }
      expect(prop("factory_stores_shutter").locked).toBe(false);

      // The dead socket by the shutter: the first spark outside the hall, in safety.
      spark(sim, "factory_gen_socket");
      expect(sim.state.flags.factory_gen_open).toBe(1);
      expect(prop("factory_gen_shutter").locked).toBe(false);

      // The press hall: spark the call box and a hauler walks a new wall down.
      useProp(sim, "factory_gate_press");
      expect(prop("factory_weak_wall").hidden ?? false).toBe(false);
      spark(sim, "factory_call_box");
      for (let t = 0; t < 200 && (sim.state.flags.factory_wall_down ?? 0) === 0; t++) idle(sim, 30);
      expect(sim.state.flags.factory_wall_down, "the hauler reached the wall").toBe(1);
      expect(prop("factory_weak_wall").hidden).toBe(true);
      expect(prop("factory_press_gate").locked).toBe(false);

      // The office: his key, his diary, a page.
      useProp(sim, "factory_foreman_key_chest");
      expect(bagCount(p, "key_foreman")).toBe(1);
      expect(bagCount(p, "foremans_diary")).toBe(1);
      const spirit = p.spirit;
      useProp(sim, "factory_office_page");
      expect(p.spirit).toBe(spirit + 6);

      // The assembly hall shuts behind her.
      useProp(sim, "factory_gate_assembly");
      const wayIn = prop("factory_assembly_wayin");
      const arena = sim.rt.bp.rects.factory_assembly;
      walkTo(sim, centre(arena.cx + 4), centre(arena.cy + 4), 6);
      idle(sim, 6);
      expect([prop("factory_gate_assembly").locked, wayIn.hidden]).toEqual([true, false]);

      // A fuse box takes a quarter of the hall out, and he puts it back at two thirds.
      const boss = sim.rt.unitsByKey.get("foreman")!;
      spark(sim, "factory_assembly_fuse_a", () => prop("factory_assembly_lamp_a").on === false);
      expect(prop("factory_assembly_lamp_a").on).toBe(false);
      // He is plated: physical lands at three tenths, and the verb this place gave her at half again.
      boss.incoming.push({ amount: Math.ceil(boss.hp * 0.3), school: "shock", from: p.id, crit: false });
      idle(sim, 4);
      expect(prop("factory_assembly_lamp_a").on, "he resets every fuse at once").toBe(true);

      // And the floor grid does what the verb is for: walk him onto it and throw one spark.
      const grid = prop("factory_grid_a");
      const drop = dropRect(sim, "factory_grid_a");
      placeUnit(sim, boss, centre(drop.cx + (drop.w >> 1)), centre(drop.cy + (drop.h >> 1)));
      const c = propCentre(sim.catalog, grid);
      placeUnit(sim, p, c.x, c.y + 20);
      const before = boss.hp;
      p.cooldowns = {};
      p.gcd = 0;
      p.mp = 999;
      sim.setAim(0, -1);
      sim.command({ t: "cast", spell: "spark" });
      idle(sim, 12);
      expect(grid.used, "the spark reached the socket").toBe(true);
      expect(boss.hp, "the grid takes him").toBeLessThan(before);
      expect(boss.statuses.some((x) => x.effect === "jolted"), "and stops him for half a second").toBe(true);

      boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
      idle(sim, 8);
      // The clear is a `while` row on the arena: it fires while somebody is standing in it.
      walkTo(sim, centre(arena.cx + (arena.w >> 1)), centre(arena.cy + (arena.h >> 1)), 8);
      idle(sim, 10);
      expect(sim.state.flags.factory_cleared).toBe(1);
      expect(prop("factory_gate_assembly").locked).toBe(false);
      expect(wayIn.hidden).toBe(true);

      // The roller door: the big jar, the Glasshouse key, and the yard open for good.
      const strength = p.strength;
      useProp(sim, "factory_big_jar");
      expect(p.strength).toBeGreaterThan(strength);
      useProp(sim, "factory_reward_chest");
      expect(bagCount(p, "key_burial")).toBe(1);
      spark(sim, "factory_roller_socket");
      expect(sim.state.flags.factory_roller_open).toBe(1);
      expect(prop("factory_roller_door").locked).toBe(false);
    });
  }
});

// --- the works' own rooms, held to the lint and the harness -------------------------------------

describe("the works' room templates", () => {
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
      for (const t of poolOf(node.pool)) expect(proveTemplate(def, node, t, catalog), `${t.id} as ${node.id}`).toEqual([]);
    }
  });
});

// --- the rows this dungeon adds, and the art for them -------------------------------------------

describe("the works' rows", () => {
  const PROPS = [
    "socket_dead", "fuse_box", "call_box", "grid_socket", "generator_cold", "broken_generator",
    "works_lamp", "weak_wall", "relay_box", "orb_spark", "factory_shutter_h", "factory_shutter_v",
    "factory_stove", "factory_rack",
  ];

  it("every prop, unit, spell and item this dungeon adds has a sprite of the right footprint", () => {
    for (const id of PROPS) {
      const d = catalog.props[id];
      expect(d, id).toBeDefined();
      const s = PROP_SPRITES[d.sprite];
      expect(s, `props.${id} -> sprite "${d.sprite}"`).toBeDefined();
      expect(s.w, `props.${id} sprite width`).toBe(d.w * 8);
      expect(s.h, `props.${id} sprite height`).toBeGreaterThanOrEqual(d.h * 8);
      expect(s.frames.base, `props.${id} base frame`).toBeDefined();
    }
    for (const id of ["sentry", "hauler", "charge_hand", "foreman"]) {
      const u = catalog.units[id];
      expect(u, id).toBeDefined();
      const s = UNIT_SPRITES[u.sprite];
      expect(s, `units.${id} -> sprite "${u.sprite}"`).toBeDefined();
      for (const f of ["down", "up", "side"]) expect(s.frames[f], `${u.sprite}.${f}`).toBeDefined();
    }
    for (const id of ["key_foreman", "foremans_diary"]) expect(ICONS[catalog.items[id].icon], `items.${id}`).toBeDefined();
    expect(ICONS[catalog.spells.spark_ai.icon]).toBeDefined();
    for (const id of ["notice_factory", "notice_light", "notice_cards", "notice_press", "factory_stove", "orb_spark", "foremans_diary"]) {
      expect(catalog.dialogue[id], `dialogue.${id}`).toBeDefined();
    }
  });

  it("the things that answer a spark answer it, and the machines only see what is lit", () => {
    for (const id of ["socket_dead", "fuse_box", "call_box", "grid_socket", "generator_cold", "relay_box"]) {
      expect(catalog.props[id].answers, id).toBe("shock");
    }
    expect(catalog.props.broken_generator.answers).toBe("repair");
    expect(catalog.props.works_lamp.lightWhenOn).toBe(true);
    expect(catalog.units.sentry.sight).toBe("lit");
    expect(catalog.units.hauler.sight).toBe("lit");
    // The Foreman is plated against everything she owns and weak to the one verb this place gives.
    expect(catalog.units.foreman.resist!.shock).toBeLessThan(0);
    for (const school of ["physical", "frost", "fire", "blast"] as const) {
      expect(catalog.units.foreman.resist![school], school).toBeLessThan(1);
    }
  });
});

// --- two seats ----------------------------------------------------------------------------------

describe("the works with two seats", () => {
  const botCatalog = yardCatalog();

  it("the assembly hall seals, and whoever was late can follow her in through the way in", () => {
    const sim = simIn(botCatalog, "factory", 13);
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const gate = sim.rt.propsByKey.get("factory_gate_assembly")!;
    const wayIn = sim.rt.propsByKey.get("factory_assembly_wayin")!;
    const rect = sim.rt.bp.rects.factory_assembly;

    gate.locked = false;
    placeUnit(sim, p, centre(rect.cx + 4), centre(rect.cy + 4));
    placeUnit(sim, friend, centre(gate.cx), centre(gate.cy) + 24);
    for (let t = 0; t < 8; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([gate.locked, wayIn.hidden]).toEqual([true, false]);
    expect(wayIn.to).toEqual({ zone: "factory", mark: "factory_assembly_in" });
    const mark = sim.rt.bp.marks.factory_assembly_in;
    expect(mark.cx).toBeGreaterThanOrEqual(rect.cx);
    expect(mark.cx).toBeLessThan(rect.cx + rect.w);
    expect(mark.cy).toBeGreaterThanOrEqual(rect.cy);
    expect(mark.cy).toBeLessThan(rect.cy + rect.h);
  });

  it("lure and socket: the grid takes the Foreman and spares the friend standing on it", () => {
    const sim = simIn(botCatalog, "factory", 2026);
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command({ t: "dev", dev: { op: "learn", spell: "spark" } });
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const boss = sim.rt.unitsByKey.get("foreman")!;
    const grid = sim.rt.propsByKey.get("factory_grid_a")!;
    const drop = dropRect(sim, "factory_grid_a");

    // One draws him onto the grid and stands on it with him; the other throws the spark.
    placeUnit(sim, boss, centre(drop.cx + (drop.w >> 1)), centre(drop.cy + (drop.h >> 1)));
    placeUnit(sim, friend, centre(drop.cx + (drop.w >> 1)) + 10, centre(drop.cy + (drop.h >> 1)));
    const c = propCentre(sim.catalog, grid);
    placeUnit(sim, p, c.x, c.y + 20);
    const bossHp = boss.hp;
    const friendHp = friend.hp;
    p.cooldowns = {};
    p.gcd = 0;
    p.mp = 999;
    sim.setAim(0, -1);
    sim.command({ t: "cast", spell: "spark" });
    for (let t = 0; t < 40; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(grid.used, "the spark reached the socket").toBe(true);
    expect(boss.hp, "the grid took him").toBeLessThan(bossHp);
    expect(friend.hp, "and spared her friend standing on it").toBe(friendHp);
  });

  it("a sentry only notices what stands in a lamp's light, and a solo player can simply stay out of it", () => {
    const sim = simIn(botCatalog, "factory", 13);
    const p = sim.player;
    const sentry = sim.rt.unitsByKey.get("factory_gen_sentry")!;
    const lamp = sim.rt.propsByKey.get("factory_bench_lamp")!;
    const hand = sim.rt.unitsByKey.get("charge_hand")!;
    expect(lamp.on).toBe(true);
    hand.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
    for (let t = 0; t < 4; t++) sim.tick(NO_INPUT);

    const reset = (): void => {
      sentry.target = 0;
      sentry.combat = "idle";
      sentry.path = null;
    };
    // Eighty pixels from it, well inside its reach, and standing in the dark: it never sees her.
    placeUnit(sim, sentry, centre(lamp.cx) - 40, centre(lamp.cy));
    placeUnit(sim, p, centre(lamp.cx) - 120, centre(lamp.cy));
    reset();
    for (let t = 0; t < 40; t++) sim.tick(NO_INPUT);
    expect(sentry.target ?? 0, "she is in the dark").toBe(0);
    // One step into the pool of the bench lamp and it has her.
    placeUnit(sim, p, centre(lamp.cx) - 16, centre(lamp.cy));
    reset();
    for (let t = 0; t < 40; t++) sim.tick(NO_INPUT);
    expect(sentry.target, "she is standing in it").toBe(p.id);
  });
});
