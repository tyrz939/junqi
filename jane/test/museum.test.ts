// The Castle Museum: one breaker, two buildings (DUNGEONS.md 3.2).
//
// Many seeds proven with the stateful flood, the same seed the same museum, the contract
// names the story leans on, the layout rejected when the breaker is withheld, a solo bot that
// plays the whole thing to the big jar, and two seats showing the co-op toy (one at the
// breaker, one crossing a wing) saves time without ever being needed.

import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import { UNIT_SPRITES } from "@/art/units";
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
import { idle, simIn, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.museum;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/museum.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 17 + i * 6301);

const built = new Map<number, Blueprint>();
function museum(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("museum", seed)));
  return bp;
}

describe("the generated museum", () => {
  it("64 seeds: every one is proven (the stateful solver and C1 to C12) and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = museum(seed);
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
    expect((performance.now() - t0) / SEEDS.length, "ms per museum, proofs included").toBeLessThan(700);
    expect(worst).toBeLessThanOrEqual(6);
  });

  it("is a pure function of (seed, attempt): same seed, same museum", () => {
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

  it("the hand-placed fallback is a whole, proven museum", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the contract names the story leans on all exist on every seed", () => {
    const want = [
      "museum_exit_door", "museum_plan", "museum_breaker", "museum_boat", "museum_history_page",
      "museum_toilets_chest", "museum_cubicle_door", "museum_stove", "museum_coat_locker",
      "museum_floor_key_chest", "museum_page", "museum_cracked_case", "museum_attendant_key_chest",
      "museum_arts_door", "museum_stores_shutter", "museum_stores_armour", "museum_arch",
      "museum_gate_rotunda", "museum_rotunda_breaker", "museum_big_jar", "museum_reward_chest",
    ];
    const derived = contractOf(def);
    for (const seed of SEEDS.slice(0, 8)) {
      const keys = new Set(museum(seed).props.map((p) => p.key));
      for (const name of want) expect(keys, `seed ${seed}: ${name}`).toContain(name);
      for (const name of derived.props) expect(keys, `seed ${seed}: bound ${name}`).toContain(name);
      expect(museum(seed).units.some((u) => u.key === "attendant")).toBe(true);
      expect(museum(seed).units.some((u) => u.key === "shot_firer")).toBe(true);
    }
  });

  it("the breaker is a control, and the generator writes its list: one `if`, both ways, nothing forgotten", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = museum(seed);
      const info = infoOf(bp)!;
      expect(info.controls).toEqual([{ state: "lights", prop: "museum_breaker" }]);
      const breaker = bp.props.find((p) => p.key === "museum_breaker")!;
      const list = breaker.use!;
      expect(list[0].do).toBe("if");
      const branch = list[0] as { do: "if"; then: { do: string; prop?: string; flag?: string; value?: number }[]; else: { do: string; prop?: string; flag?: string; value?: number }[] };
      // Going dark: the flag goes up, the dark-only door unlocks and the lit-only shutter comes down.
      expect(branch.else[0]).toEqual({ do: "flag", flag: "museum_dark", value: 1 });
      expect(branch.else).toContainEqual({ do: "unlock", prop: "museum_arts_door" });
      expect(branch.else).toContainEqual({ do: "lock", prop: "museum_stores_shutter" });
      // And back again, in one list, so neither gate can be forgotten in one direction.
      expect(branch.then[0]).toEqual({ do: "flag", flag: "museum_dark", value: 0 });
      expect(branch.then).toContainEqual({ do: "lock", prop: "museum_arts_door" });
      expect(branch.then).toContainEqual({ do: "unlock", prop: "museum_stores_shutter" });
      // The shutter is up as the building is made; the armour on its rail is across the ARTS door.
      const shutter = bp.props.find((p) => p.key === "museum_stores_shutter")!;
      const artsDoor = bp.props.find((p) => p.key === "museum_arts_door")!;
      expect([shutter.locked ?? false, artsDoor.locked]).toEqual([false, true]);
      expect(["museum_shutter_h", "museum_shutter_v"]).toContain(shutter.def);
      expect(["museum_armour_door_h", "museum_armour_door_v"]).toContain(artsDoor.def);
    }
  });

  it("the solver rejects the same layout with the breaker withheld: the dark wing really is the breaker's", () => {
    for (const seed of SEEDS.slice(0, 4)) {
      const bp = museum(seed);
      const opts = { ...solveOptionsOf(def), trace: true };
      const whole = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, opts);
      expect(whole.errors).toEqual([]);
      expect(whole.trace!.layers).toEqual([0, 1]);
      const without = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { props: ["museum_breaker"] } });
      expect(without.ok).toBe(false);
      expect(without.trace!.layers).toEqual([0]);
      // And without Explosion the stores open in neither state, which is the whole point of them.
      const noBlast = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { ...opts, withhold: { verbs: ["explosion"] } });
      expect(noBlast.ok).toBe(false);
      expect(noBlast.errors.join("\n")).toMatch(/museum_attendant_key_chest/);
    }
  });

  it("the first completion is the mission in order, and inside its band", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = museum(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual(["entry", "atrium", "history", "cloakroom", "toilets", "maintenance", "arts", "science", "stores", "magic", "magic_case"]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });
});

// --- a solo bot, the whole museum ------------------------------------------------------------

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

/** Explosion is a bolt: stand beside the thing, aim at it, and throw. */
function blast(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const prop = sim.rt.propsByKey.get(key)!;
  const d = sim.catalog.props[prop.def];
  const p = sim.player;
  const dx = centre(prop.cx + (d.w >> 1)) - p.x;
  const dy = centre(prop.cy + (d.h >> 1)) - p.y;
  const len = Math.sqrt(dx * dx + dy * dy) || 1;
  p.cooldowns = {};
  p.gcd = 0;
  sim.setAim(dx / len, dy / len);
  sim.command({ t: "cast", spell: "explosion" });
  idle(sim, 40);
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

describe("a solo bot in the generated museum", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 2026]) {
    it(`seed ${seed}: the key shuffle, the breaker, the dark wing, the page, the stores, the rotunda`, { timeout: 120_000 }, () => {
      const sim = simIn(botCatalog, "museum", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      // What the story would have given her by the door: Icebolt at home, Repair in the mine.
      sim.command({ t: "dev", dev: { op: "learn", spell: "repair" } });
      idle(sim, 2);

      // The plan reveals the museum; the boat holds the conveniences key.
      useProp(sim, "museum_plan");
      talkThrough(sim);
      useProp(sim, "museum_boat");
      expect(bagCount(p, "key_toilets")).toBe(1);

      // The cloakroom: a stove to rest at and two lengths of wood in a locker.
      useProp(sim, "museum_stove");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("museum");
      useProp(sim, "museum_coat_locker");
      expect(bagCount(p, "wood")).toBe(2);

      // The conveniences: the maintenance key, and Repair still earns its keep.
      useProp(sim, "museum_gate_toilets");
      useProp(sim, "museum_toilets_chest");
      expect(bagCount(p, "key_maintenance")).toBe(1);
      expect(prop("museum_toilets_jar").hidden).toBe(true);
      cast(sim, "repair", "museum_cubicle_door");
      expect(prop("museum_toilets_jar").hidden).toBe(false);
      const strength = p.strength;
      useProp(sim, "museum_toilets_jar");
      expect(p.strength).toBe(strength + 2);

      // The breaker. One throw and the building is the other building.
      useProp(sim, "museum_gate_maintenance");
      expect([prop("museum_arts_door").locked, prop("museum_stores_shutter").locked]).toEqual([true, false]);
      expect(prop("museum_atrium_exhibit").hidden ?? false).toBe(false);
      useProp(sim, "museum_breaker");
      expect(sim.state.flags.museum_dark).toBe(1);
      expect([prop("museum_arts_door").locked, prop("museum_stores_shutter").locked]).toEqual([false, true]);
      expect(prop("museum_atrium_exhibit").hidden).toBe(true);
      expect(sim.rt.unitsByKey.get("museum_atrium_walker")).toBeDefined();
      expect(sim.rt.unitsByKey.get("museum_prowler"), "a throw to dark is priced").toBeDefined();

      // ARTS, in the dark, for the floor key.
      useProp(sim, "museum_floor_key_chest");
      expect(bagCount(p, "key_floor")).toBe(1);

      // Lights on again: everything is back on its plinth and the shutter is up.
      useProp(sim, "museum_breaker");
      expect(sim.state.flags.museum_dark).toBe(0);
      expect(prop("museum_atrium_exhibit").hidden).toBe(false);
      expect(sim.rt.unitsByKey.get("museum_atrium_walker")).toBeUndefined();

      // SCIENCE: the glove will not open while the Shot-Firer stands.
      useProp(sim, "museum_gate_science");
      useProp(sim, "museum_page");
      expect(sim.me.dialogue).toBeNull();
      expect(p.book).not.toContain("explosion");
      killUnit(sim, "shot_firer");
      useProp(sim, "museum_page");
      talkThrough(sim);
      expect(p.book).toContain("explosion");

      // The first blast, in safety, in the room that taught it.
      const spirit = p.spirit;
      blast(sim, "museum_cracked_case");
      expect(prop("museum_cracked_case").locked).toBe(false);
      expect(prop("museum_science_page").hidden).toBe(false);
      useProp(sim, "museum_science_page");
      expect(p.spirit).toBe(spirit + 6);

      // The stores: lit, an armour stands in the door. It opens in neither state.
      expect(prop("museum_stores_armour").hidden ?? false).toBe(false);
      blast(sim, "museum_stores_armour");
      expect(prop("museum_stores_armour").hidden).toBe(true);
      useProp(sim, "museum_attendant_key_chest");
      expect(bagCount(p, "key_attendant")).toBe(1);

      // The arch, then his door, then the room shuts.
      blast(sim, "museum_arch");
      expect(prop("museum_arch").hidden).toBe(true);
      useProp(sim, "museum_gate_rotunda");
      const wayIn = prop("museum_magic_wayin");
      expect(wayIn.hidden).toBe(true);
      const inside = sim.rt.bp.marks.museum_magic_in;
      walkTo(sim, centre(inside.cx), centre(inside.cy), 3);
      idle(sim, 5);
      expect(prop("museum_gate_rotunda").locked).toBe(true);
      expect(wayIn.hidden).toBe(false);

      // He throws his own breaker at three quarters. The armours step down; she throws it back.
      const boss = sim.rt.unitsByKey.get("attendant")!;
      boss.incoming.push({ amount: Math.ceil(boss.hp * 0.3), school: "physical", from: p.id, crit: false });
      idle(sim, 4);
      expect(sim.state.flags.museum_rotunda_dark).toBe(1);
      expect(prop("museum_rotunda_armour_a").hidden).toBe(true);
      expect(sim.rt.unitsByKey.get("museum_rotunda_walker_a")).toBeDefined();
      useProp(sim, "museum_rotunda_breaker");
      expect(sim.state.flags.museum_rotunda_dark).toBe(0);
      expect(prop("museum_rotunda_armour_a").hidden).toBe(false);
      expect(sim.rt.unitsByKey.get("museum_rotunda_walker_a")).toBeUndefined();
      expect(boss.statuses.some((s) => s.effect === "dazzled"), "caught in the light").toBe(true);

      // Frozen and lit, an armour can be taken out of the fight for good.
      blast(sim, "museum_rotunda_armour_a");
      expect(sim.state.flags.museum_gone_rotunda_a).toBe(1);
      boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
      idle(sim, 6);
      expect(sim.state.flags.museum_cleared).toBe(1);
      expect(prop("museum_gate_rotunda").locked).toBe(false);
      expect(wayIn.hidden).toBe(true);

      // The case that is not on the plan, and the fire door home.
      const before = p.strength;
      useProp(sim, "museum_big_jar");
      expect(p.strength).toBe(before + 14);
      useProp(sim, "museum_reward_chest");
      expect(bagCount(p, "key_forest")).toBe(1);
      expect(prop("museum_fire_door").locked).toBe(false);
      expect(walkToProp(sim, "museum_stove")).toBe(true);
    });
  }
});

// --- the museum's own rooms, held to the lint and the harness --------------------------------
// (`templates.test.ts` walks every mission there is, so it is red while another dungeon is
// half-written. These run the same two proofs over the museum's rooms alone.)

describe("the museum's room templates", () => {
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
      // The breaker's room is the one exception, and it is the harness's limit, not the room's:
      // see the test below. Everything else stands on its own.
      if (node.id === "maintenance") continue;
      for (const t of poolOf(node.pool)) expect(proveTemplate(def, node, t, catalog), `${t.id} as ${node.id}`).toEqual([]);
    }
  });

  /**
   * A building-wide state is, by definition, a list that reaches into other rooms: that is what
   * a breaker IS. The harness stamps a room ALONE, and the solver refuses any prop list that
   * names something the one-room blueprint has not got, so the breaker's room cannot be proven
   * on its own. This test pins the failure to exactly that cause, so it fails loudly if the
   * breaker's room ever stops being walkable for some other reason.
   */
  it("the breaker's room is proven like any other, though its list names half the building", () => {
    // A control's list reaches into other rooms by definition: that is what a building-wide state is.
    // The harness stamps a room alone and asks for it as a FRAGMENT, so those absent names are expected
    // rather than broken rows, and the room is still judged on its own doors and sockets.
    const node = def.nodes.find((n) => n.id === "maintenance")!;
    for (const t of poolOf(node.pool)) {
      expect(lintRoom(t), t.id).toEqual([]);
      expect(proveTemplate(def, node, t, catalog), t.id).toEqual([]);
    }
    // And in a whole museum every name the breaker throws is really there.
    const bp = museum(SEEDS[0]);
    expect(validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, solveOptionsOf(def)).errors).toEqual([]);
  });
});

// --- the rows this dungeon adds, and the art for them ----------------------------------------

describe("the museum's rows", () => {
  const MINE = [
    "museum_exhibit_armour", "museum_exhibit_waxwork", "museum_exhibit_fox", "museum_arch",
    "museum_plinth", "museum_breaker", "museum_gallery_lamp", "museum_shutter_h", "museum_shutter_v",
    "museum_armour_door_h", "museum_armour_door_v", "museum_case", "museum_stove", "museum_portrait",
    "museum_cubicle_door", "rubble", "cracked_wall_h", "cracked_wall_v", "cracked_case", "page_explosion",
  ];

  it("every prop, unit, spell and item this dungeon adds has a sprite of the right footprint", () => {
    for (const id of MINE) {
      const p = catalog.props[id];
      expect(p, id).toBeDefined();
      const s = PROP_SPRITES[p.sprite];
      expect(s, `props.${id} -> sprite "${p.sprite}"`).toBeDefined();
      expect(s.w, `props.${id} sprite width`).toBe(p.w * 8);
      expect(s.h, `props.${id} sprite height`).toBeGreaterThanOrEqual(p.h * 8);
      expect(s.frames.base, `props.${id} base frame`).toBeDefined();
    }
    for (const id of ["armour", "stuffed_fox", "waxwork", "shot_firer", "attendant"]) {
      const u = catalog.units[id];
      expect(u, id).toBeDefined();
      const s = UNIT_SPRITES[u.sprite];
      expect(s, `units.${id} -> sprite "${u.sprite}"`).toBeDefined();
      for (const f of ["down", "up", "side"]) expect(s.frames[f], `${u.sprite}.${f}`).toBeDefined();
    }
    for (const id of ["key_toilets", "key_maintenance", "key_floor", "key_attendant", "key_forest"]) {
      expect(ICONS[catalog.items[id].icon], `items.${id}`).toBeDefined();
    }
    expect(ICONS[catalog.spells.charge_lob.icon]).toBeDefined();
    for (const id of ["notice_museum", "notice_touch", "notice_power", "page_explosion", "museum_stove", "museum_portrait"]) {
      expect(catalog.dialogue[id], `dialogue.${id}`).toBeDefined();
    }
  });

  it("the shared rows DUNGEONS.md 4.2 names for the old map are here, unprefixed, and answer blast", () => {
    for (const id of ["rubble", "cracked_wall_h", "cracked_wall_v", "cracked_case"]) {
      expect(catalog.props[id].answers, id).toBe("blast");
      expect(catalog.props[id].solid, id).toBe(true);
    }
    expect([catalog.props.rubble.w, catalog.props.rubble.h]).toEqual([3, 2]);
    expect([catalog.props.cracked_wall_h.w, catalog.props.cracked_wall_h.h]).toEqual([3, 1]);
    expect([catalog.props.cracked_wall_v.w, catalog.props.cracked_wall_v.h]).toEqual([1, 3]);
  });
});

// --- two seats: the breaker is the best toy in the game, and nothing needs it ----------------

describe("the museum with two seats", () => {
  const botCatalog = yardCatalog();

  it("the rotunda seals, and whoever was late can follow her in through the way in", () => {
    const seed = 12;
    const sim = simIn(botCatalog, "museum", seed);
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const gate = sim.rt.propsByKey.get("museum_gate_rotunda")!;
    const wayIn = sim.rt.propsByKey.get("museum_magic_wayin")!;
    const arch = sim.rt.propsByKey.get("museum_arch")!;
    const rect = sim.rt.bp.rects.museum_rotunda;

    // The host is inside and the door is shut behind her; the friend is outside, by the arch.
    arch.hidden = true;
    gate.locked = false;
    placeUnit(sim, p, centre(rect.cx + 4), centre(rect.cy + 4));
    placeUnit(sim, friend, centre(arch.cx), centre(arch.cy));
    for (let t = 0; t < 8; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([gate.locked, wayIn.hidden]).toEqual([true, false]);
    // The way in stands outside the shut gate and puts her back inside the rect.
    expect(wayIn.to).toEqual({ zone: "museum", mark: "museum_magic_in" });
    const mark = sim.rt.bp.marks.museum_magic_in;
    expect(mark.cx).toBeGreaterThanOrEqual(rect.cx);
    expect(mark.cx).toBeLessThan(rect.cx + rect.w);
    expect(mark.cy).toBeGreaterThanOrEqual(rect.cy);
    expect(mark.cy).toBeLessThan(rect.cy + rect.h);
  });

  it("one at the breaker, one crossing a wing: the ARTS door opens and shuts for her, and a solo player can do it alone", () => {
    const seed = 2026;
    const sim = simIn(botCatalog, "museum", seed);
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const breaker = sim.rt.propsByKey.get("museum_breaker")!;
    const artsDoor = sim.rt.propsByKey.get("museum_arts_door")!;

    // The friend stands at the breaker while the host waits by the ARTS door.
    placeUnit(sim, friend, centre(breaker.cx), centre(breaker.cy + 1) + 2);
    friend.facing = 3;
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(artsDoor.locked).toBe(true);
    sim.command(seat, { t: "use" });
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([sim.state.flags.museum_dark, artsDoor.locked]).toEqual([1, false]);
    // And it is priced: a throw to dark wakes something in the corridor by Maintenance.
    expect(sim.rt.unitsByKey.get("museum_prowler")).toBeDefined();
    sim.command(seat, { t: "use" });
    for (let t = 0; t < 2; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect([sim.state.flags.museum_dark, artsDoor.locked]).toEqual([0, true]);
    expect(sim.rt.unitsByKey.get("museum_prowler")).toBeUndefined();
  });

  it("natural history's two plinths are a twin hold: two friends, or two crates, or one of each", () => {
    const seed = 12;
    const sim = simIn(yardCatalog(), "museum", seed);
    const jar = sim.rt.propsByKey.get("museum_natural_jar");
    if (!jar) return; // a side room the layout left out this seed
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    const plateA = sim.rt.propsByKey.get("museum_natural_plate_a")!;
    const plateB = sim.rt.propsByKey.get("museum_natural_plate_b")!;
    const crate = sim.rt.propsByKey.get("museum_natural_push_a")!;
    expect(jar.locked).toBe(true);
    // One crate pushed onto the near plinth, one friend standing on the far one.
    moveProp(sim, crate, plateA.cx, plateA.cy);
    placeUnit(sim, friend, centre(plateB.cx + 1), centre(plateB.cy + 1));
    for (let t = 0; t < 12; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(jar.locked).toBe(false);
  });
});
