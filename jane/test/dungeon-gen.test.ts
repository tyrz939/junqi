// The dungeon generator (DUNGEONS.md 2.5 to 2.9), held to the Gold Mine: many seeds, zero
// fallbacks, the same seed the same mine, names that survive re-rolls, checks that really
// reject, and a solo bot that finishes the whole thing, new content included.

import { describe, expect, it } from "vitest";
import { bagCount } from "@/sim/inventory";
import { questReady } from "@/sim/quests";
import { moveProp } from "@/sim/runtime";
import type { Sim } from "@/sim/sim";
import { placeUnit } from "@/sim/units";
import { buildCatalog } from "@/sim/catalog";
import { centre, Tile } from "@/sim/grid";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { DUNGEONS } from "@/world/dungeon";
import { checkDungeon, contractOf, firstCompletion } from "@/world/dungeon/checks";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { buildZone, CONTRACTS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.mine;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/dungeon-gen.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 31 + i * 7919);

/** Built once per seed for the whole file: every test below reads, none of them writes. */
const built = new Map<number, Blueprint>();
function mine(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("mine", seed)));
  return bp;
}

describe("the generated mine", () => {
  it("64 seeds: every one is proven (solver and C1 to C12) inside the attempt budget, and none needs the fallback", { timeout: 120_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = mine(seed);
      const info = infoOf(bp)!;
      expect(info.layout!.fallback, `seed ${seed} fell back to the hand-placed layout`).toBe(false);
      expect(bp.attempts, `seed ${seed}`).toBeLessThan(ZONE_ATTEMPTS);
      worst = Math.max(worst, bp.attempts);
      expect(checkDungeon(bp, catalog), `seed ${seed}`).toEqual([]);
      // No room twice, every critical node placed, side rooms inside their budget.
      const used = info.layout!.placements.map((p) => p.template);
      expect(new Set(used).size).toBe(used.length);
      for (const n of def.nodes) if (n.critical) expect(info.rooms.some((r) => r.node.id === n.id), `seed ${seed}: ${n.id}`).toBe(true);
      const sides = info.rooms.filter((r) => !r.node.critical).length;
      expect(sides).toBeGreaterThanOrEqual(def.budget.sideRooms[0]);
      expect(sides).toBeLessThanOrEqual(def.budget.sideRooms[1]);
    }
    // A dungeon is small. If this creeps up, profile: hot loops must read locals (ENGINE.md 8).
    // A mine costs about 55 ms alone. This is a guard against an order of magnitude, not a benchmark:
    // the suite runs its files in parallel, so the figure here is whatever the machine had left over.
    expect((performance.now() - t0) / SEEDS.length, "ms per mine, proofs included").toBeLessThan(500);
    expect(worst).toBeLessThanOrEqual(4);
  });

  it("is a pure function of (seed, attempt): same seed, same mine; another seed, another mine", () => {
    const a = buildDungeon(def, 4242, 0);
    const b = buildDungeon(def, 4242, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.rects).toEqual(b.rects);
    expect(a.triggers).toEqual(b.triggers);
    const c = buildDungeon(def, 4243, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(c.tiles))).toBe(false);
    const d = buildDungeon(def, 4242, 1);
    expect(Buffer.from(a.tiles).equals(Buffer.from(d.tiles))).toBe(false);
  });

  it("layouts really differ: over the seeds, the hub and the boss room stand in many different bays", () => {
    const hubs = new Set<string>();
    const bosses = new Set<string>();
    for (const seed of SEEDS.slice(0, 32)) {
      const layout = infoOf(mine(seed))!.layout!;
      hubs.add(String(layout.placements.find((p) => p.node === "core")!.bay));
      bosses.add(String(layout.placements.find((p) => p.node === "arena")!.bay));
    }
    expect(hubs.size).toBeGreaterThanOrEqual(4);
    expect(bosses.size).toBeGreaterThanOrEqual(4);
  });

  it("the hand-placed fallback is a whole, proven mine: the last attempt never throws at the player", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, CONTRACTS.mine, def.givenKeys, { verbs: def.givenVerbs }).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
    // Every critical node, and the same whatever the seed.
    expect(infoOf(bp)!.rooms.map((r) => r.node.id).sort()).toEqual(def.nodes.filter((n) => n.critical).map((n) => n.id).sort());
    const other = buildDungeon(def, 7, ZONE_ATTEMPTS - 1);
    expect(infoOf(other)!.layout).toEqual(infoOf(bp)!.layout);
    expect(Buffer.from(other.tiles).equals(Buffer.from(bp.tiles))).toBe(true);
  });

  it("the contract derived from the mission's binds contains every name the story already leans on", () => {
    const derived = contractOf(def);
    for (const what of ["units", "props", "marks", "rects"] as const) {
      for (const name of CONTRACTS.mine[what]) expect(derived[what], `${what}: ${name}`).toContain(name);
    }
  });

  it("generated names come from node and socket: a jar's id is its key, and no re-roll or layout can change it", () => {
    const found = (bp: Blueprint): string[] =>
      bp.props.flatMap((p) => (p.use ?? []).flatMap((a) => (a.do === "grow" ? [`${p.key}=${a.id}`] : []))).sort();
    const a = found(buildDungeon(def, 5, 0));
    expect(a.length).toBeGreaterThanOrEqual(3);
    for (const pair of a) expect(pair.split("=")[0]).toBe(pair.split("=")[1]);
    // The same jars whatever the attempt and whatever the seed (the cage is a side room and may be left out).
    const always = (list: string[]): string[] => list.filter((x) => !x.startsWith("mine_cage"));
    expect(always(found(buildDungeon(def, 5, 3)))).toEqual(always(a));
    expect(always(found(buildDungeon(def, 77777, 0)))).toEqual(always(a));
    expect(always(found(buildDungeon(def, 5, ZONE_ATTEMPTS - 1)))).toEqual(always(a));
    expect(a).toContain("cabinet_jar=cabinet_jar");
    expect(a).toContain("mine_big_jar=mine_big_jar");
  });

  it("heat: every unit is at the dungeon's phase, a room's enemies cost no more than its share, and rest rooms are empty", () => {
    for (const seed of SEEDS.slice(0, 16)) {
      const bp = mine(seed);
      for (const u of bp.units) expect(u.phase).toBe(def.phase);
      for (const room of infoOf(bp)!.rooms) {
        const inside = bp.units.filter((u) => u.key.startsWith(`mine_${room.node.id}_spawn_`));
        const cost = inside.reduce((n, u) => n + def.budget.enemies[u.def], 0);
        const share = Math.min(Math.round(room.node.heat * def.budget.baseHeat), room.shape.template.heatMax);
        expect(cost, `${room.node.id}`).toBeLessThanOrEqual(share);
        // Within 15% of the share whenever the sockets can hold it: costs of 1 exist, so it is exact here.
        if (room.shape.sockets.filter((s) => s.kind === "spawn").length * 2 >= share) expect(cost, `${room.node.id}`).toBe(share);
        if (room.node.kind === "rest" || room.node.heat === 0) expect(inside.length).toBe(0);
      }
    }
  });

  it("the checks really reject, each by name: C1 and C3 to C12", () => {
    const broken = (change: (bp: Blueprint) => void): string => {
      const bp = buildZone("mine", 31);
      change(bp);
      return checkDungeon(bp, catalog).join("\n");
    };
    const prop = (bp: Blueprint, key: string) => bp.props.find((p) => p.key === key)!;
    expect(broken(() => {})).toBe("");
    expect(broken((bp) => (prop(bp, "gate_generic_b").locked = false))).toMatch(/C1: .*store -> core does not hold/);
    expect(broken((bp) => (prop(bp, "broken_steps").hidden = true))).toMatch(/C1: .*core -> gallery does not hold/);
    expect(broken((bp) => (prop(bp, "mine_arena_wayin").hidden = false))).toMatch(/C6/);
    expect(broken((bp) => (prop(bp, "mine_office_chest_wood").loot = []))).toMatch(/C4: .*wood[\s\S]*C5: hm_cabinet wants 1 wood/);
    expect(
      broken((bp) => {
        delete bp.triggers!.hm_drawer_free;
        prop(bp, "hm_drawer").locked = false;
      }),
    ).toMatch(/C5: repair can be learned in office while headmaster is still up/);
    expect(broken((bp) => (bp.triggers!.mine_arena_lock.rect = "mine_all"))).toMatch(/C12: .*gate_boss/);
    expect(
      broken((bp) => {
        for (const p of bp.props) if (catalog.props[p.def].push) p.def = "chest";
      }),
    ).toMatch(/C11: nothing can be pushed onto plate_a/);

    // The checks that read the mission: the same blueprint, judged against a mission that asks for something else.
    const against = (change: (d: typeof def) => void): string => {
      const bp = buildZone("mine", 31);
      const other = structuredClone(def);
      change(other);
      infoOf(bp)!.def = other;
      return checkDungeon(bp, catalog).join("\n");
    };
    expect(against(() => {})).toBe("");
    // One plain key for two plain locks: spend it on the guard room and the hub is lost.
    expect(against((d) => (d.nodes.find((n) => n.id === "store")!.holds = []))).toMatch(/C3: opening plain locks in the order \[entry-guard\] strands her/);
    expect(against((d) => (d.budget.restAt = [0.9, 1]))).toMatch(/C7: the rest room is first reached/);
    expect(against((d) => (d.budget.critPathCells = [10, 20]))).toMatch(/C8: the first completion is \d+ cells/);
    expect(against((d) => (d.budget.restToBossCells = 10))).toMatch(/C9: the boss is \d+ cells from the rest room/);
    expect(against((d) => (d.edges = d.edges.map((e) => ({ ...e, shortcut: false }))))).toMatch(/C9: no shortcut was placed/);
    // Handed the boss key at the door, she opens the big door the first time she sees it: no tease.
    expect(against((d) => (d.givenKeys = ["mine_boss"]))).toMatch(/C10: gate_boss is not seen before it can be opened/);
  });

  it("the first completion is the walk that was designed: the rest room before the Headmaster, the boss last but one", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = mine(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual(["entry", "plate", "store", "guard", "core", "firstaid", "office", "gallery", "vault", "arena", "nook"]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });
});

// --- a solo bot, the whole mine ------------------------------------------------------------

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

function repair(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  p.cooldowns = {};
  p.gcd = 0;
  sim.command({ t: "cast", spell: "repair" });
  idle(sim, 2);
  expect(sim.rt.propsByKey.get(key)!.used, `${key} mended`).toBe(true);
}

function killAndLoot(sim: Sim, unit: string): void {
  const u = sim.rt.unitsByKey.get(unit)!;
  walkTo(sim, u.x, u.y, 30);
  // By hand, not `dev kill`: that reaches 25 m through walls, and the next bay may be the boss room.
  u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 3);
  // Only what fell here. Bats and rats fight each other all over the mine, and what they drop is somewhere else.
  for (const d of [...sim.zone.drops]) {
    if (Math.abs(d.x - u.x) + Math.abs(d.y - u.y) > 60) continue;
    walkTo(sim, d.x, d.y, 5);
    sim.command({ t: "use" });
  }
}

describe("a solo bot in the generated mine", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 2026, 90210]) {
    it(`seed ${seed}: the whole mission, the drawer, the cabinet, first aid, the track, the hoists, the nook`, { timeout: 60_000 }, () => {
      const sim = simIn(botCatalog, "mine", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      sim.command({ t: "dev", dev: { op: "quest", quest: "the_mine" } });
      idle(sim, 2);
      expect(sim.state.flags["been:mine"]).toBe(1);

      // The plate, the two plain keys, the clerk.
      useProp(sim, "plate_chest");
      expect(bagCount(p, "key_generic")).toBe(0);
      moveProp(sim, prop("plate_barrel"), prop("plate_a").cx, prop("plate_a").cy);
      idle(sim, 12);
      useProp(sim, "plate_chest");
      useProp(sim, "store_chest");
      expect(bagCount(p, "key_generic")).toBe(2);
      useProp(sim, "gate_generic_a");
      useProp(sim, "guard_chest");
      killAndLoot(sim, "clerk");
      expect(bagCount(p, "key_mine_headmaster")).toBe(1);
      useProp(sim, "gate_generic_b");

      // First aid: a stove off the hub. Resting there moves the party's fire into the mine.
      useProp(sim, "firstaid_stove");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("mine");
      const stove = prop("firstaid_stove");
      expect(Math.abs(sim.state.rest!.x - centre(stove.cx)) + Math.abs(sim.state.rest!.y - centre(stove.cy))).toBeLessThan(40);

      // The drawer stays shut while the Headmaster stands. Then: Repair, and a safe first use in the same room.
      useProp(sim, "gate_hm");
      useProp(sim, "hm_drawer");
      expect(sim.me.dialogue).toBeNull();
      expect(p.book).not.toContain("repair");
      killAndLoot(sim, "headmaster");
      expect(bagCount(p, "key_mine_vault")).toBe(1);
      useProp(sim, "hm_drawer");
      talkThrough(sim);
      expect(p.book).toContain("repair");
      useProp(sim, "mine_office_chest_wood");
      expect(prop("cabinet_jar").hidden).toBe(true);
      repair(sim, "hm_cabinet");
      expect(prop("cabinet_jar").hidden).toBe(false);
      const strength = p.strength;
      useProp(sim, "cabinet_jar");
      expect(p.strength).toBe(strength + 2);
      expect(sim.state.growth.found).toContain("cabinet_jar");
      useProp(sim, "cabinet_jar");
      expect(p.strength).toBe(strength + 2);

      // The steps, the boss key, and the track: the long loop closes with the iron from the store.
      expect(bagCount(p, "wood")).toBe(2);
      repair(sim, "broken_steps");
      expect(prop("broken_steps").hidden).toBe(true);
      useProp(sim, "boss_key_chest");
      expect(bagCount(p, "iron")).toBe(4);
      repair(sim, "broken_track");
      expect(prop("broken_track").hidden).toBe(true);
      expect(bagCount(p, "iron")).toBe(0);

      // The vault: the Museum key, and a page.
      useProp(sim, "gate_vault");
      useProp(sim, "vault_chest");
      expect(bagCount(p, "key_museum")).toBe(1);
      const spirit = p.spirit;
      useProp(sim, "mine_vault_page_main");
      expect(p.spirit).toBe(spirit + 6);
      expect(prop("mine_vault_page_main").hidden).toBe(true);

      // The arena seals, and shows its way in. A hoist mended and pulled with him under it.
      useProp(sim, "gate_boss");
      const wayIn = prop("mine_arena_wayin");
      expect(wayIn.hidden).toBe(true);
      const inside = sim.rt.bp.marks.mine_arena_in;
      walkTo(sim, centre(inside.cx), centre(inside.cy), 3);
      idle(sim, 5);
      expect(prop("gate_boss").locked).toBe(true);
      expect(wayIn.hidden).toBe(false);
      const boss = sim.rt.unitsByKey.get("iron_knuckles")!;
      expect(boss.combat).toBe("combat");
      useProp(sim, "arena_chest");
      expect(prop("mine_arena_lever_hoist1").hidden).toBe(true);
      repair(sim, "mine_arena_verbprop_hoist1");
      expect(prop("mine_arena_lever_hoist1").hidden).toBe(false);
      const drop = sim.rt.bp.rects.mine_arena_drop_1;
      placeUnit(sim, boss, centre(drop.cx + 2), centre(drop.cy + 2));
      const hp = boss.hp;
      useProp(sim, "mine_arena_lever_hoist1");
      idle(sim, 2);
      expect(boss.hp).toBe(hp - 200);
      expect(boss.statuses.some((s) => s.effect === "staggered")).toBe(true);

      boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
      idle(sim, 5);
      expect(prop("gate_boss").locked).toBe(false);
      expect(wayIn.hidden).toBe(true);
      expect(sim.state.flags.mine_cleared).toBe(1);
      expect(questReady(sim, "the_mine")).toBe(true);

      // The nook opens on his death, and its far gate is the short way back to the gallery.
      const before = p.strength;
      useProp(sim, "mine_big_jar");
      expect(p.strength).toBe(before + 14);
      expect(prop("mine_gate_nook_gallery").locked).toBe(false);
      expect(walkToProp(sim, "boss_key_chest")).toBe(true);
      expect(sim.rt.grid.tileAt(0, 0)).toBe(Tile.CaveWall);
    });
  }
});
