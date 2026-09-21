// Castle School: the building follows the timetable (DUNGEONS.md 3.6).
//
// Many seeds proven with the stateful flood over `period`, the same seed the same school, the
// contract names the story leans on, the layout rejected when the bell rope is withheld and
// again when the Caretaker's key is, a solo bot that plays the whole thing to the register,
// and two seats showing that the rope and the two beds are toys and never requirements.

import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import { UNIT_SPRITES } from "@/art/units";
import { buildCatalog } from "@/sim/catalog";
import { CELL, TICKS_PER_HOUR } from "@/sim/constants";
import { cellOf, centre } from "@/sim/grid";
import { bagCount } from "@/sim/inventory";
import { propCentre } from "@/sim/runtime";
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
import { idle, simIn, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.school;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/school.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 17 + i * 6301);

const built = new Map<number, Blueprint>();
function school(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("school", seed)));
  return bp;
}

describe("the generated school", () => {
  it("64 seeds: every one is proven (the stateful solver and C1 to C12) and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = school(seed);
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
    expect((performance.now() - t0) / SEEDS.length, "ms per school, proofs included").toBeLessThan(400);
    expect(worst).toBeLessThanOrEqual(4);
  });

  it("is a pure function of (seed, attempt): same seed, same school", () => {
    const a = buildDungeon(def, 4242, 0);
    const b = buildDungeon(def, 4242, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.rects).toEqual(b.rects);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 4243, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the hand-placed fallback is a whole, proven school", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the contract names the story leans on all exist on every seed, and so do both arrival marks", () => {
    const want = [
      "school_exit_door", "school_timetable", "school_bell_rope", "school_hall_pegs",
      "school_bed", "school_bell_bed", "school_grate", "school_sickbay_chest",
      "school_clock_woodwork", "school_clock_chemistry", "school_clock_botany",
      "school_clock_physics", "school_clock_domestic", "school_clock_ice",
      "school_woodwork_steps", "school_chemistry_case", "school_botany_vine",
      "school_physics_fuse", "school_domestic_range", "school_ice_torch_a", "school_ice_torch_b",
      "school_door_woodwork", "school_door_physics", "school_tower_door", "school_top_door", "school_back_stair",
      "school_belfry_gear", "school_belfry_fuse", "school_belfry_case",
      "school_register_book", "school_big_jar", "school_reward_chest",
    ];
    const derived = contractOf(def);
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = school(seed);
      const keys = new Set(bp.props.map((p) => p.key));
      for (const name of want) expect(keys, `seed ${seed}: ${name}`).toContain(name);
      for (const name of derived.props) expect(keys, `seed ${seed}: bound ${name}`).toContain(name);
      // `entry` is the county's door; `boiler` is where the Burial Chamber's stair comes up.
      expect(bp.marks.entry, `seed ${seed}: entry`).toBeDefined();
      expect(bp.marks.boiler, `seed ${seed}: boiler`).toBeDefined();
      expect(bp.units.some((u) => u.key === "caretaker")).toBe(true);
      expect(bp.units.some((u) => u.key === "ringer")).toBe(true);
    }
  });

  it("the bell rope is a control, and the generator writes its list: one `if`, both periods, nothing forgotten", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = school(seed);
      expect(infoOf(bp)!.controls).toEqual([{ state: "period", prop: "school_bell_rope" }]);
      const rope = bp.props.find((p) => p.key === "school_bell_rope")!;
      const list = rope.use!;
      expect(list[0].do).toBe("if");
      const branch = list[0] as { do: "if"; then: { do: string; prop?: string; flag?: string; value?: number }[]; else: { do: string; prop?: string; flag?: string; value?: number }[] };
      // To break: the flag goes up, the break-only doors open and the lesson doors shut.
      expect(branch.else[0]).toEqual({ do: "flag", flag: "school_break", value: 1 });
      expect(branch.else).toContainEqual({ do: "unlock", prop: "school_door_physics" });
      expect(branch.else).toContainEqual({ do: "lock", prop: "school_door_woodwork" });
      // And back to lessons in the same list, so neither door can be forgotten in one direction.
      expect(branch.then[0]).toEqual({ do: "flag", flag: "school_break", value: 0 });
      expect(branch.then).toContainEqual({ do: "lock", prop: "school_door_physics" });
      expect(branch.then).toContainEqual({ do: "unlock", prop: "school_door_woodwork" });
      // The building is made in lesson time: the classrooms are open and the yard is not.
      expect(bp.props.find((p) => p.key === "school_door_woodwork")!.locked ?? false).toBe(false);
      expect(bp.props.find((p) => p.key === "school_door_physics")!.locked).toBe(true);
      // The author's own list runs after the generated one: that is what rings nine.
      expect(list[1].do).toBe("if");
    }
  });

  it("the solver rejects the layout with the bell rope withheld, and again with the Caretaker's key", () => {
    for (const seed of SEEDS.slice(0, 4)) {
      const bp = school(seed);
      const opts = { ...solveOptionsOf(def), trace: true };
      const whole = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, opts);
      expect(whole.errors).toEqual([]);
      expect(whole.trace!.layers).toEqual([0, 1]);
      // No rope, no break: three lessons, the tower and the ending are all on the other side of it.
      const noRope = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { props: ["school_bell_rope"] } });
      expect(noRope.ok).toBe(false);
      expect(noRope.trace!.layers).toEqual([0]);
      expect(noRope.errors.join("\n")).toMatch(/school_clock_physics|school_physics_fuse/);
      // The six clocks are not enough on their own: the rope will not ring nine without his key.
      const noKey = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { keys: ["school_tower"] } });
      expect(noKey.ok).toBe(false);
      expect(noKey.errors.join("\n")).toMatch(/school_tower_door|ringer|school_register_book/);
    }
  });

  it("the first completion is the mission in order, and inside its band", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = school(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual([
        "boiler", "hall", "sick_bay", "corridor", "top_corridor",
        "woodwork", "chemistry", "botany", "physics", "domestic", "ice_house",
        "tower", "top_room",
      ]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });
});

// --- a solo bot, the whole school --------------------------------------------------------------

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

/** A world verb: stand beside the thing and cast. */
function cast(sim: Sim, spell: string, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  p.cooldowns = {};
  p.gcd = 0;
  p.mp = 400;
  sim.command({ t: "cast", spell });
  idle(sim, 2);
}

/**
 * A bolt: aim at the thing's middle and throw. A bolt dies on the first sight-blocking cell,
 * so she tries one side, and if nothing catches she walks round to the next, as a person would.
 */
function bolt(sim: Sim, spell: string, key: string): void {
  const prop = sim.rt.propsByKey.get(key)!;
  const d = sim.catalog.props[prop.def];
  const c = propCentre(sim.catalog, prop);
  const spots: [number, number][] = [
    [c.x, (prop.cy + d.h) * CELL + 6],
    [c.x, prop.cy * CELL - 6],
    [prop.cx * CELL - 6, c.y],
    [(prop.cx + d.w) * CELL + 6, c.y],
  ];
  for (const [x, y] of spots) {
    if (sim.rt.grid.solid(cellOf(x), cellOf(y))) continue;
    if (!walkTo(sim, x, y, 5)) continue;
    const me = sim.player;
    me.cooldowns = {};
    me.gcd = 0;
    me.mp = 400;
    const dx = c.x - me.x;
    const dy = c.y - me.y;
    const len = Math.hypot(dx, dy) || 1;
    sim.setAim(dx / len, dy / len);
    sim.command({ t: "cast", spell });
    idle(sim, 60);
    const now = sim.rt.propsByKey.get(key)!;
    if (now.on || now.hidden) return;
  }
  throw new Error(`nothing she did reached ${key} with ${spell}`);
}

function killUnit(sim: Sim, key: string): void {
  const u = sim.rt.unitsByKey.get(key)!;
  walkTo(sim, u.x, u.y, 40);
  u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 3);
  for (const drop of [...sim.zone.drops]) {
    if (Math.abs(drop.x - u.x) + Math.abs(drop.y - u.y) > 60) continue;
    walkTo(sim, drop.x, drop.y, 5);
    sim.command({ t: "use" });
  }
}

describe("a solo bot in the generated school", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 2026]) {
    it(`seed ${seed}: six lessons in two periods, a night in the sick bay, the Caretaker, the belfry`, { timeout: 180_000 }, () => {
      const sim = simIn(botCatalog, "school", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      // Every verb is hers already: this is the last dungeon and it teaches nothing.
      for (const spell of def.givenVerbs) sim.command({ t: "dev", dev: { op: "learn", spell } });
      sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
      idle(sim, 2);

      // The timetable is the map. The caretaker's bin by the boiler has the wood in it.
      useProp(sim, "school_timetable");
      talkThrough(sim);
      useProp(sim, "school_boiler_chest");
      expect(bagCount(p, "wood")).toBe(2);

      // The sick bay: the near bed sleeps to six, as beds do.
      useProp(sim, "school_sickbay_chest");
      expect(bagCount(p, "wood")).toBe(4);
      useProp(sim, "school_bed");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("school");
      expect(Math.floor(sim.state.clock / TICKS_PER_HOUR)).toBe(6);

      // Lesson time, which is how the building is made: three classrooms are open.
      expect(sim.state.flags.school_break ?? 0).toBe(0);
      expect(prop("school_door_woodwork").locked ?? false).toBe(false);
      expect(prop("school_door_physics").locked).toBe(true);

      // Woodwork: the benches across the flooded floor, and the first clock.
      expect(prop("school_clock_woodwork").hidden).toBe(true);
      cast(sim, "repair", "school_woodwork_steps");
      expect(prop("school_clock_woodwork").hidden).toBe(false);
      expect(bagCount(p, "wood")).toBe(2);
      useProp(sim, "school_clock_woodwork");
      talkThrough(sim);
      expect(sim.state.flags.school_lesson_woodwork).toBe(1);

      // Chemistry: the fume cupboard.
      bolt(sim, "explosion", "school_chemistry_case");
      expect(prop("school_chemistry_case").hidden).toBe(true);
      useProp(sim, "school_clock_chemistry");
      talkThrough(sim);
      expect(sim.state.flags.school_lesson_chemistry).toBe(1);

      // Botany: the vine across the gap, in the glasshouse, and only while the sun is on it.
      sim.command({ t: "dev", dev: { op: "time", hour: 11 } });
      idle(sim, 2);
      cast(sim, "grow", "school_botany_vine");
      expect(prop("school_clock_botany").hidden).toBe(false);
      useProp(sim, "school_clock_botany");
      talkThrough(sim);
      expect(sim.state.flags.school_lesson_botany).toBe(1);

      // The rope. One pull and it is break: those three shut and three others open.
      useProp(sim, "school_bell_rope");
      expect(sim.state.flags.school_break).toBe(1);
      expect(prop("school_door_woodwork").locked).toBe(true);
      expect(prop("school_door_physics").locked).toBe(false);
      expect(sim.rt.unitsByKey.get("school_corridor_master_a")).toBeUndefined();

      // Physics, Domestic Science, and the ice house: the other half of the day.
      bolt(sim, "spark", "school_physics_fuse");
      expect(prop("school_physics_lamp_a").hidden).toBe(false);
      useProp(sim, "school_clock_physics");
      talkThrough(sim);
      bolt(sim, "fireball", "school_domestic_range");
      useProp(sim, "school_clock_domestic");
      talkThrough(sim);
      expect(prop("school_clock_ice").locked).toBe(true);
      bolt(sim, "icebolt", "school_ice_torch_a");
      bolt(sim, "icebolt", "school_ice_torch_b");
      idle(sim, 4);
      expect(sim.state.flags.school_ice_a).toBe(1);
      expect(prop("school_clock_ice").locked).toBe(false);
      useProp(sim, "school_clock_ice");
      talkThrough(sim);
      for (const n of ["physics", "domestic", "ice"]) expect(sim.state.flags[`school_lesson_${n}`], n).toBe(1);

      // Six clocks and the rope still will not ring nine: the tower wants his key as well.
      useProp(sim, "school_bell_rope");
      expect(sim.state.flags.school_tower_open ?? 0).toBe(0);
      expect(prop("school_tower_door").locked).toBe(true);

      // The far bed is the lever. YOU WILL BE WOKEN AT THE BELL: it sleeps to nine at night.
      useProp(sim, "school_bell_bed");
      talkThrough(sim);
      expect(Math.floor(sim.state.clock / TICKS_PER_HOUR)).toBe(21);

      // He walks the corridor at night, and he is carrying every key in the building.
      idle(sim, 30);
      expect(sim.rt.unitsByKey.get("caretaker")!.hidden ?? false).toBe(false);
      killUnit(sim, "caretaker");
      expect(bagCount(p, "key_tower")).toBe(1);

      // Nine.
      useProp(sim, "school_bell_rope");
      expect(sim.state.flags.school_tower_open).toBe(1);
      idle(sim, 4);
      expect(prop("school_tower_door").locked).toBe(false);

      // The stair seals behind her, and whoever was late has a way in.
      const wayIn = prop("school_tower_wayin");
      expect(wayIn.hidden).toBe(true);
      const inside = sim.rt.bp.marks.school_tower_in;
      walkTo(sim, centre(inside.cx), centre(inside.cy), 3);
      idle(sim, 6);
      expect(prop("school_tower_door").locked).toBe(true);
      expect(wayIn.hidden).toBe(false);

      // The belfry. Every toll turns it over, and every verb has one job up here.
      const boss = sim.rt.unitsByKey.get("ringer")!;
      expect(prop("school_belfry_brazier_a").on).toBe(true);
      // He takes half of everything physical, so this is the thirty per cent that starts phase one.
      boss.incoming.push({ amount: Math.ceil(boss.hp * 0.6), school: "physical", from: p.id, crit: false });
      idle(sim, 6);
      expect(prop("school_belfry_brazier_a").on, "the first toll puts the lamps out").toBe(false);
      expect(prop("school_belfry_bud_a").hidden).toBe(false);
      // Electric puts the lamps back on their own circuit; Fire would do it one at a time.
      bolt(sim, "spark", "school_belfry_fuse");
      expect(prop("school_belfry_brazier_a").on).toBe(true);
      // Grow closes the window; Repair winds the gear and the room hits back.
      cast(sim, "grow", "school_belfry_bud_a");
      const before = boss.hp;
      cast(sim, "repair", "school_belfry_gear");
      idle(sim, 3);
      expect(boss.hp).toBeLessThan(before);
      // Explosion is the cupboard, which is a box of potions and not a lock.
      bolt(sim, "explosion", "school_belfry_case");
      expect(prop("school_belfry_chest").hidden).toBe(false);

      boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
      idle(sim, 8);
      expect(sim.state.flags.school_cleared).toBe(1);
      expect(prop("school_tower_door").locked).toBe(false);
      expect(wayIn.hidden).toBe(true);

      // The top room: the register, the big jar, and the back stair to the sick bay.
      const strength = p.strength;
      useProp(sim, "school_big_jar");
      expect(p.strength).toBe(strength + 14);
      useProp(sim, "school_register_book");
      talkThrough(sim);
      useProp(sim, "school_reward_chest");
      expect(bagCount(p, "gold_bar")).toBeGreaterThanOrEqual(6);
      expect(prop("school_back_stair").locked).toBe(false);
      expect(walkToProp(sim, "school_bed")).toBe(true);
    });
  }
});

// --- the school's own rooms, held to the lint and the harness -----------------------------------
// (`templates.test.ts` walks every mission there is, so it is red while another dungeon is
// half-written. These run the same two proofs over the school's rooms alone.)

describe("the school's room templates", () => {
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
      // The hall is the one exception, and it is the harness's limit, not the room's: a control's
      // list reaches into every corridor in the building by definition. See the test below.
      if (node.id === "hall") continue;
      for (const t of poolOf(node.pool)) expect(proveTemplate(def, node, t, catalog), `${t.id} as ${node.id}`).toEqual([]);
    }
  });

  it("the hall is proven like any other room, though the rope's list names half the building", () => {
    const node = def.nodes.find((n) => n.id === "hall")!;
    for (const t of poolOf(node.pool)) {
      expect(lintRoom(t), t.id).toEqual([]);
      expect(proveTemplate(def, node, t, catalog), t.id).toEqual([]);
    }
    const bp = school(SEEDS[0]);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
  });
});

// --- the rows this dungeon adds, and the art for them --------------------------------------------

describe("the school's rows", () => {
  const MINE = ["bell_rope", "clock_stopped", "sickbay_bed_bell", "school_fuse", "school_register", "school_bell", "school_pegs"];

  it("every prop, unit and item this dungeon adds has a sprite of the right footprint", () => {
    for (const id of MINE) {
      const d = catalog.props[id];
      expect(d, id).toBeDefined();
      const s = PROP_SPRITES[d.sprite];
      expect(s, `props.${id} -> sprite "${d.sprite}"`).toBeDefined();
      expect(s.w, `props.${id} sprite width`).toBe(d.w * 8);
      expect(s.h, `props.${id} sprite height`).toBeGreaterThanOrEqual(d.h * 8);
      expect(s.frames.base, `props.${id} base frame`).toBeDefined();
    }
    for (const id of ["master", "caretaker", "ringer"]) {
      const u = catalog.units[id];
      expect(u, id).toBeDefined();
      const s = UNIT_SPRITES[u.sprite];
      expect(s, `units.${id} -> sprite "${u.sprite}"`).toBeDefined();
      for (const f of ["down", "up", "side"]) expect(s.frames[f], `${u.sprite}.${f}`).toBeDefined();
    }
    expect(ICONS[catalog.items.key_tower.icon]).toBeDefined();
    for (const id of ["notice_school", "school_boiler", "school_pegs", "school_clock", "school_bed_day", "school_bed_bell", "school_register"]) {
      expect(catalog.dialogue[id], `dialogue.${id}`).toBeDefined();
    }
  });

  it("the three rows DUNGEONS.md 4.2 names for the School are as it names them", () => {
    expect([catalog.props.sickbay_bed_bell.w, catalog.props.sickbay_bed_bell.h]).toEqual([2, 3]);
    expect(catalog.props.sickbay_bed_bell.rest).toBe(true);
    expect(catalog.props.clock_stopped.once).toBe(true);
    expect(catalog.props.school_fuse.answers).toBe("shock");
    // The near bed sleeps to six as every bed does; the far one is woken at the bell.
    const untilOf = (tree: string): number | undefined => {
      for (const n of Object.values(catalog.dialogue[tree].nodes)) {
        for (const a of n.actions ?? []) if (a.do === "rest") return a.until;
      }
      return undefined;
    };
    expect(untilOf("school_bed_day")).toBe(6);
    expect(untilOf("school_bed_bell")).toBe(21);
  });

  it("the Caretaker is a night-only mini-boss who carries the tower key, and the Ringer is 5,000 HP at phase 6", () => {
    const c = catalog.units.caretaker;
    expect(c.nightOnly).toBe(true);
    expect(c.boss).toBe(true);
    expect(c.loot).toContainEqual({ item: "key_tower", qty: 1, chance: 1 });
    expect(catalog.items.key_tower.opens).toBe("school_tower");
    // 125 strength x 5 hp x the phase-6 scale of 8.
    expect(catalog.units.ringer.strength * 5 * 8).toBe(5000);
    expect(catalog.units.ringer.phases!.length).toBe(3);
    for (const ph of catalog.units.ringer.phases!) expect(ph.onEnter!.length).toBeGreaterThan(0);
  });

  it("no row in this dungeon shows a child, and none bakes the heroine's name in", () => {
    const text = JSON.stringify([
      catalog.dialogue.notice_school, catalog.dialogue.school_boiler, catalog.dialogue.school_pegs,
      catalog.dialogue.school_clock, catalog.dialogue.school_bed_day, catalog.dialogue.school_bed_bell,
      catalog.dialogue.school_register,
    ]);
    expect(text).not.toMatch(/Jane/);
    expect(text).not.toMatch(/\bchild|children|pupil|boy\b|girl\b/i);
  });
});

// --- two seats: the rope and the beds are toys, and nothing in here needs a second player ---------

describe("the school with two seats", () => {
  const botCatalog = yardCatalog();

  it("one on the rope, one in the corridor: the classroom doors open and shut for her, and a solo player can do it alone", () => {
    const sim = simIn(botCatalog, "school", 2026);
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const rope = sim.rt.propsByKey.get("school_bell_rope")!;
    const woodwork = sim.rt.propsByKey.get("school_door_woodwork")!;
    const physics = sim.rt.propsByKey.get("school_door_physics")!;

    placeUnit(sim, friend, centre(rope.cx), centre(rope.cy + 1) + 2);
    friend.facing = 3;
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([woodwork.locked ?? false, physics.locked]).toEqual([false, true]);
    sim.command(seat, { t: "use" });
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([sim.state.flags.school_break, woodwork.locked, physics.locked ?? false]).toEqual([1, true, false]);
    // And it is priced: the masters go off duty at break and come back on at the next bell.
    expect(sim.rt.unitsByKey.get("school_top_master_a")).toBeUndefined();
    sim.command(seat, { t: "use" });
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([sim.state.flags.school_break, woodwork.locked ?? false, physics.locked]).toEqual([0, false, true]);
    expect(sim.rt.unitsByKey.get("school_top_master_a")).toBeDefined();
  });

  it("the tower stair seals, and whoever was late can follow her up through the way in", () => {
    const sim = simIn(botCatalog, "school", 12);
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const gate = sim.rt.propsByKey.get("school_tower_door")!;
    const wayIn = sim.rt.propsByKey.get("school_tower_wayin")!;
    const hall = sim.rt.bp.rects.school_hall;
    const rect = sim.rt.bp.rects.school_tower;

    // The host is up the stair with the door shut behind her; the friend is still in the hall.
    gate.locked = false;
    placeUnit(sim, p, centre(rect.cx + 6), centre(rect.cy + 6));
    placeUnit(sim, friend, centre(hall.cx + 4), centre(hall.cy + 4));
    for (let t = 0; t < 8; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([gate.locked, wayIn.hidden]).toEqual([true, false]);
    // The way in stands outside the shut door and puts whoever climbs it back inside the belfry.
    expect(wayIn.to).toEqual({ zone: "school", mark: "school_tower_in" });
    const mark = sim.rt.bp.marks.school_tower_in;
    expect(mark.cx).toBeGreaterThanOrEqual(rect.cx);
    expect(mark.cx).toBeLessThan(rect.cx + rect.w);
    expect(mark.cy).toBeGreaterThanOrEqual(rect.cy);
    expect(mark.cy).toBeLessThan(rect.cy + rect.h);
  });

  it("the night only passes when the whole party is lying down, which is what the card on the far bed says", () => {
    const sim = simIn(botCatalog, "school", 12);
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const far = sim.rt.propsByKey.get("school_bell_bed")!;
    const near = sim.rt.propsByKey.get("school_bed")!;

    // The friend is somewhere else in the building, so the clock does not move.
    const hall = sim.rt.bp.rects.school_hall;
    placeUnit(sim, friend, centre(hall.cx + 4), centre(hall.cy + 4));
    walkTo(sim, centre(far.cx + 1), centre(far.cy + 3), 8);
    sim.command({ t: "use" });
    talkThrough(sim);
    for (let t = 0; t < 4; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(Math.floor(sim.state.clock / TICKS_PER_HOUR), "one awake, so the night does not pass").toBe(10);
    expect(p.hp, "she is mended all the same").toBe(p.strength * 5);

    // The friend lies down in the other bed and the night passes for both of them.
    placeUnit(sim, friend, centre(near.cx + 1), centre(near.cy + 3));
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    sim.command(seat, { t: "use" });
    talkThrough(sim);
    sim.command({ t: "use" });
    talkThrough(sim);
    for (let t = 0; t < 4; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(Math.floor(sim.state.clock / TICKS_PER_HOUR)).toBe(21);
  });
});
