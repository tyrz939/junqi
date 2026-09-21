// Butterfly Forest (DUNGEONS.md 3.3). No doors and nothing locked: light is the lock and a
// living thing is the key. Grow blooms buds, bridges the stream and CLOSES hedge gaps, which
// is the part that could wall her out, so there is a test here that closes every gap the zone
// has and asks the solver to finish it anyway. Plus the usual: many seeds proven, the same
// seed the same forest, a solo bot from the gate to the big jar, and a friend who follows her
// into the glade the hedge shut behind her.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { bagCount } from "@/sim/inventory";
import { centre, Tile } from "@/sim/grid";
import { propCentre } from "@/sim/runtime";
import { Sim } from "@/sim/sim";
import type { Sim as SimType } from "@/sim/sim";
import { placeUnit } from "@/sim/units";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { DUNGEONS } from "@/world/dungeon";
import { checkDungeon, contractOf, firstCompletion, lintDef } from "@/world/dungeon/checks";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { buildZone, CONTRACTS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkTo, walkToProp, walkToUnit, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.forest;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/forest.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 31 + i * 7919);

const built = new Map<number, Blueprint>();
function forest(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("forest", seed)));
  return bp;
}

/** The six glades, the two later ones, and the butterfly each holds. */
const GLADES = [
  { node: "first_glade", bud: "forest_first_bud", fly: "butterfly_1" },
  { node: "rock", bud: "forest_rock_bud", fly: "butterfly_2" },
  { node: "runner", bud: "forest_runner_bud", fly: "butterfly_3" },
  { node: "island", bud: "forest_island_bud", fly: "butterfly_4" },
  { node: "guarded", bud: "forest_guarded_bud", fly: "butterfly_5" },
  { node: "hollow", bud: "forest_hollow_bud", fly: "butterfly_6" },
];

describe("butterfly forest", () => {
  it("the mission itself is sound before any layout exists", () => {
    expect(lintDef(def, catalog)).toEqual([]);
    expect(def.indoor, "it is outdoors: the clock is the light").toBe(false);
    expect(def.tiles.wall, "hedges are the walls").toBe("Hedge");
    // No keys, no key items, no key gates: the only locks are light and a living thing.
    expect(def.givenKeys).toEqual([]);
    for (const e of def.edges) expect(e.kind.t, `${e.from} -> ${e.to}`).not.toBe("key");
  });

  it("64 seeds: every one is proven (solver and C1 to C12) inside the attempt budget, and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = forest(seed);
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
    expect((performance.now() - t0) / SEEDS.length, "ms per forest, proofs included").toBeLessThan(260);
    expect(worst).toBeLessThanOrEqual(3);
  });

  it("no self-lockout: grow EVERY hedge seed shut and the forest can still be finished", { timeout: 180_000 }, () => {
    // The generator cannot express this, so it is written against the built blueprint: take
    // every `fill` a hedge seed would do, lay it into the tiles, and solve the zone again.
    // A closed gap must only ever take a way round away, never the only way.
    let closed = 0;
    for (const seed of SEEDS.slice(0, 24)) {
      const bp = buildDungeon(def, seed, 0);
      let hedges = 0;
      for (const p of bp.props) {
        if (p.def !== "hedge_seed") continue;
        for (const a of p.use ?? []) {
          if (a.do !== "fill") continue;
          const r = bp.rects[a.rect];
          expect(r, `${p.key} fills ${a.rect}`).toBeDefined();
          expect(a.tile, "a hedge seed grows a hedge").toBe(Tile.Hedge);
          for (let y = r.cy; y < r.cy + r.h; y++) for (let x = r.cx; x < r.cx + r.w; x++) bp.tiles[y * bp.w + x] = Tile.Hedge;
          hedges++;
        }
      }
      expect(hedges, `seed ${seed}: the zone should hold five closable gaps`).toBe(5);
      closed += hedges;
      const v = validateBlueprint(bp, catalog, contractOf(def), def.givenKeys, { verbs: def.givenVerbs });
      expect(v.errors, `seed ${seed}: with every gap grown shut`).toEqual([]);
      expect(checkDungeon(bp, catalog), `seed ${seed}: with every gap grown shut`).toEqual([]);
    }
    expect(closed).toBe(24 * 5);
  });

  it("is a pure function of (seed, attempt): same seed, same forest; another seed, another forest", () => {
    const a = buildDungeon(def, 4242, 0);
    const b = buildDungeon(def, 4242, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 4243, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the contract names the story leans on all exist, on every seed", () => {
    const contract = contractOf(def);
    for (const name of ["forest_gate_out", "forest_cases", "forest_hearth", "forest_stone", "forest_big_jar", "forest_hedge_gate", ...GLADES.map((g) => g.bud)]) {
      expect(contract.props, name).toContain(name);
    }
    for (const name of ["collector", "emperor", ...GLADES.map((g) => g.fly)]) expect(contract.units, name).toContain(name);
    expect(contract.rects).toContain("forest_arena");
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = forest(seed);
      for (const name of contract.props) expect(bp.props.some((p) => p.key === name), `seed ${seed}: ${name}`).toBe(true);
      for (const name of contract.units) expect(bp.units.some((u) => u.key === name), `seed ${seed}: ${name}`).toBe(true);
      for (const name of contract.marks) expect(bp.marks[name], `seed ${seed}: ${name}`).toBeDefined();
    }
    expect(CONTRACTS.forest).toEqual(contract);
  });

  it("every butterfly is a friendly npc with a patrol that dwells, and the whole loop is inside one glade", () => {
    const bp = forest(31);
    for (const g of GLADES) {
      const u = bp.units.find((x) => x.key === g.fly)!;
      const row = catalog.units[u.def];
      expect(row.faction).toBe("friendly");
      expect(row.controller).toBe("npc");
      expect(row.talk, `${g.fly} can be talked to`).toBeDefined();
      expect(u.patrol!.length).toBeGreaterThanOrEqual(2);
      for (const pt of u.patrol!) expect(pt.length, "every waypoint dwells").toBe(3);
      const room = infoOf(bp)!.rooms.find((r) => r.node.id === g.node)!;
      for (const [x, y] of u.patrol!) {
        expect(x >= room.rect.cx && x < room.rect.cx + room.rect.w, `${g.fly} stays in ${g.node}`).toBe(true);
        expect(y >= room.rect.cy && y < room.rect.cy + room.rect.h, `${g.fly} stays in ${g.node}`).toBe(true);
      }
    }
  });

  it("day and night are two forests: the butterflies are day only, the moth and the spiders are not", () => {
    for (const n of [1, 2, 3, 4, 5, 6, 7]) expect(catalog.units[`forest_butterfly_${n}`].dayOnly, `butterfly ${n}`).toBe(true);
    expect(catalog.units.forest_moth_8.nightOnly).toBe(true);
    expect(catalog.units.forest_night_spider.nightOnly).toBe(true);
    // Sunbeams fall by day and moonbeams by night, so a different set of buds is live.
    expect(catalog.props.forest_sunbeam.dayOnly).toBe(true);
    expect(catalog.props.forest_moonbeam.nightOnly).toBe(true);
    const bp = forest(31);
    const beams = bp.props.filter((p) => p.def === "forest_sunbeam" && p.key.startsWith("forest_stone_"));
    const moons = bp.props.filter((p) => p.def === "forest_moonbeam");
    expect(beams.length, "three of the arena's buds are lit by day").toBe(3);
    expect(moons.length, "and two by night").toBe(2);
  });

  it("the hand-placed fallback is a whole, proven forest: the last attempt never throws at the player", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, CONTRACTS.forest, def.givenKeys, { verbs: def.givenVerbs }).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the first completion is the walk that was designed: the hearth off the ring, the stone last but one", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = forest(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order.slice(0, 5)).toEqual(["gate", "first_glade", "hut", "ring", "hearth"]);
      expect(walk.order.slice(-2)).toEqual(["stone", "reward"]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });

  it("the checks really reject: the lock-in, the light on the bank, and the boss key of a forest that has none", () => {
    const broken = (change: (bp: Blueprint) => void): string => {
      const bp = buildZone("forest", 31);
      change(bp);
      return checkDungeon(bp, catalog).join("\n");
    };
    expect(broken(() => {})).toBe("");
    expect(broken((bp) => (bp.props.find((p) => p.key === "forest_stone_wayin")!.hidden = false))).toMatch(/C6/);
    expect(broken((bp) => (bp.triggers!.forest_stone_lock.rect = "forest_all"))).toMatch(/C12: .*forest_hedge_gate/);
    // Take the bank away and the boss glade cannot be reached at all.
    expect(broken((bp) => (bp.props.find((p) => p.key === "forest_stone_bank")!.use = []))).toMatch(/solver: prop "forest_stone" is unreachable|C1: stone is never reached/);
  });
});

// --- a solo bot, the whole forest ------------------------------------------------------------

function useProp(sim: SimType, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

function grow(sim: SimType, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  p.cooldowns = {};
  p.gcd = 0;
  p.mp = 400;
  sim.command({ t: "cast", spell: "grow" });
  idle(sim, 3);
  expect(sim.rt.propsByKey.get(key)!.used, `${key} grew`).toBe(true);
}

/** A bolt of Explosion into a thing that answers `blast`. She has to stand back and aim. */
function blast(sim: SimType, key: string): void {
  const prop = sim.rt.propsByKey.get(key)!;
  const c = propCentre(sim.catalog, prop);
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const p = sim.player;
  for (let n = 0; n < 6 && !sim.rt.propsByKey.get(key)!.used; n++) {
    p.cooldowns = {};
    p.gcd = 0;
    p.mp = 400;
    const dx = c.x - p.x;
    const dy = c.y - p.y;
    const d = Math.sqrt(dx * dx + dy * dy) || 1;
    sim.setAim(dx / d, dy / d);
    sim.command({ t: "cast", spell: "explosion" });
    idle(sim, 40);
  }
  expect(sim.rt.propsByKey.get(key)!.used, `${key} came apart`).toBe(true);
}

function killByHand(sim: SimType, key: string): void {
  const u = sim.rt.unitsByKey.get(key)!;
  walkTo(sim, u.x, u.y, 30);
  u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 3);
  for (const d of [...sim.zone.drops]) {
    if (Math.abs(d.x - u.x) + Math.abs(d.y - u.y) > 80) continue;
    walkTo(sim, d.x, d.y, 5);
    sim.command({ t: "use" });
    idle(sim, 2);
  }
}

/** Grow the bud, wait for it to come down to the new flower, then put the net over it. */
function netIt(sim: SimType, bud: string, fly: string): void {
  grow(sim, bud);
  idle(sim, 240);
  for (let n = 0; n < 6; n++) {
    if (!sim.rt.unitsByKey.get(fly)) return;
    walkToUnit(sim, fly, 12);
    sim.command({ t: "use" });
    idle(sim, 2);
    if (sim.me.dialogue) {
      talkThrough(sim, [0]);
      idle(sim, 2);
    }
    if (!sim.rt.unitsByKey.get(fly)) return;
  }
  expect(sim.rt.unitsByKey.get(fly), `${fly} went into the net`).toBeUndefined();
}

describe("a solo bot in butterfly forest", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 2026]) {
    it(`seed ${seed}: the sign, the Collector's net, six butterflies, the stone and the Emperor`, { timeout: 300_000 }, () => {
      const sim = simIn(botCatalog, "forest", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      // Noon: the sign says sunrise to sunset, and the verbs she was given by now.
      sim.command({ t: "dev", dev: { op: "time", hour: 12 } });
      for (const spell of def.givenVerbs) sim.command({ t: "dev", dev: { op: "learn", spell } });
      idle(sim, 2);
      for (const spell of def.givenVerbs) expect(p.book).toContain(spell);

      // The council sign at the gate is the map.
      useProp(sim, "forest_gate_notice_main");
      expect(sim.me.dialogue).not.toBeNull();
      talkThrough(sim);

      // The first glade: a bud in a beam, and a butterfly she cannot keep.
      grow(sim, "forest_first_bud");
      idle(sim, 240);
      walkToUnit(sim, "butterfly_1", 12);
      sim.command({ t: "use" });
      idle(sim, 2);
      expect(sim.me.dialogue, "it sits on her sleeve and she has nothing to keep it in").not.toBeNull();
      talkThrough(sim);
      expect(bagCount(p, "butterfly")).toBe(0);

      // The hut: the cases, the wood in his tool chest, and the net he will not need.
      useProp(sim, "forest_cases");
      talkThrough(sim);
      useProp(sim, "forest_hut_chest");
      expect(bagCount(p, "wood")).toBe(2);
      expect(bagCount(p, "net")).toBe(0);
      killByHand(sim, "collector");
      expect(bagCount(p, "net"), "he was guarding the tool, not the verb").toBe(1);

      // The hearth on the ring: the party's fire moves into the middle of the forest.
      useProp(sim, "forest_hearth");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("forest");
      useProp(sim, "forest_hearth_page");

      // Butterfly one, now with the net.
      netIt(sim, "forest_first_bud", "butterfly_1");
      expect(bagCount(p, "butterfly")).toBe(1);

      // The rock and the hollow are behind Explosion; the island is across the stream.
      blast(sim, "forest_rock");
      netIt(sim, "forest_rock_bud", "butterfly_2");
      netIt(sim, "forest_runner_bud", "butterfly_3");

      expect(prop("forest_bridge").hidden, "the footbridge is still broken").toBe(false);
      const crossing = sim.rt.bp.rects.forest_crossing;
      grow(sim, "forest_vine");
      expect(sim.rt.grid.tileAt(crossing.cx, crossing.cy), "the vine is a way over the water").toBe(Tile.GrownPath);
      expect(prop("forest_bridge").hidden, "and it takes the broken rail with it").toBe(true);
      netIt(sim, "forest_island_bud", "butterfly_4");
      netIt(sim, "forest_guarded_bud", "butterfly_5");
      blast(sim, "forest_hollow_rock");
      netIt(sim, "forest_hollow_bud", "butterfly_6");
      expect(bagCount(p, "butterfly")).toBe(6);
      expect(sim.state.flags.forest_butterflies).toBe(6);

      // Eight would have given the Amulet. Six does not, and the eleventh case says so.
      useProp(sim, "forest_cases");
      talkThrough(sim);
      expect(bagCount(p, "butterfly_amulet")).toBe(0);

      // A gap in the hedge, closed for good. It opens nothing; it takes somewhere to run away.
      const gap = sim.rt.bp.rects.forest_ring_gap_a;
      expect(sim.rt.grid.tileAt(gap.cx, gap.cy)).not.toBe(Tile.Hedge);
      grow(sim, "forest_seed_ring_a");
      // A solid fill waits for its whole rect to be clear: a hedge that grew round the one cell
      // she stood on would box her in. So she steps off the ride and it lands whole.
      expect(walkToProp(sim, "forest_seed_ring_b")).toBe(true);
      idle(sim, 10);
      expect(sim.rt.grid.tileAt(gap.cx, gap.cy)).toBe(Tile.Hedge);

      // The bank up to the stone, and the hedge that closes behind her.
      grow(sim, "forest_stone_bank");
      const wayIn = prop("forest_stone_wayin");
      expect(wayIn.hidden).toBe(true);
      const inside = sim.rt.bp.marks.forest_stone_in;
      walkTo(sim, centre(inside.cx), centre(inside.cy), 4);
      idle(sim, 5);
      expect(prop("forest_hedge_gate").locked, "the hedge closes the way she came").toBe(true);
      expect(wayIn.hidden, "and shows a way back in for whoever was late").toBe(false);
      const boss = sim.rt.unitsByKey.get("emperor")!;
      expect(boss.combat).toBe("combat");

      // Five butterflies and the glade comes into bud. One bud, one feeding, one window.
      expect(prop("forest_stone_verbprop_bud1").hidden).toBe(true);
      useProp(sim, "forest_stone");
      idle(sim, 3);
      expect(sim.state.flags.forest_summoned).toBe(1);
      expect(prop("forest_stone_verbprop_bud1").hidden).toBe(false);
      let fed = false;
      for (const n of [1, 2, 3]) {
        grow(sim, `forest_stone_verbprop_bud${n}`);
        idle(sim, 300);
        if (boss.statuses.some((s) => s.effect === "staggered")) {
          fed = true;
          break;
        }
      }
      expect(fed, "it came down to feed, and that is the window").toBe(true);

      boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
      idle(sim, 6);
      expect(sim.state.flags.forest_cleared).toBe(1);
      expect(prop("forest_hedge_gate").locked).toBe(false);
      expect(wayIn.hidden).toBe(true);

      // The big jar, and a page, past the stone.
      const strength = p.strength;
      useProp(sim, "forest_big_jar");
      expect(p.strength).toBe(strength + 14);
      const spirit = p.spirit;
      useProp(sim, "forest_reward_page");
      expect(p.spirit).toBe(spirit + 6);
    });
  }
});

// --- two seats --------------------------------------------------------------------------------

function party(seed: number): SimType {
  const sim = Sim.newGame(yardCatalog(), seed);
  sim.command(0, { t: "open", on: true });
  sim.command(-1, { t: "join", who: "Mina" });
  sim.drainEvents();
  for (const seat of [0, 1]) {
    sim.command(seat, { t: "dev", dev: { op: "god", on: true } });
    sim.command(seat, { t: "dev", dev: { op: "time", hour: 12 } });
    sim.command(seat, { t: "dev", dev: { op: "tp", zone: "forest", mark: "entry" } });
    for (const spell of DUNGEONS.forest.givenVerbs) sim.command(seat, { t: "dev", dev: { op: "learn", spell } });
  }
  for (let i = 0; i < 3; i++) sim.tick([]);
  return sim;
}

describe("two seats in butterfly forest", () => {
  it("the hedge closes behind her, and the friend who was late climbs in after her", () => {
    const sim = party(31);
    sim.command(0, { t: "dev", dev: { op: "tp", zone: "forest", mark: "forest_stone_in" } });
    for (let i = 0; i < 6; i++) sim.tick([]);
    const gate = sim.rt.propsByKey.get("forest_hedge_gate")!;
    const wayIn = sim.rt.propsByKey.get("forest_stone_wayin")!;
    expect(gate.locked, "it closed behind her").toBe(true);
    expect(wayIn.hidden, "and showed the way in").toBe(false);

    const mina = sim.view(1).player;
    const c = propCentre(sim.catalog, wayIn);
    const gc = propCentre(sim.catalog, gate);
    const ax = Math.sign(c.x - gc.x) || 0;
    const ay = Math.sign(c.y - gc.y) || 1;
    placeUnit(sim, mina, c.x + ax * 10, c.y + ay * 10);
    const toward = { mx: -ax, my: -ay, sprint: false, useHeld: false, ax: 0, ay: 0 };
    for (let i = 0; i < 3; i++) sim.tick([{ mx: 0, my: 0, sprint: false, useHeld: false, ax: 0, ay: 0 }, toward]);
    sim.command(1, { t: "use" });
    for (let i = 0; i < 4; i++) sim.tick([]);
    const inner = sim.rt.bp.rects.forest_arena;
    const cx = Math.floor(mina.x / 8);
    const cy = Math.floor(mina.y / 8);
    expect(cx >= inner.cx && cx < inner.cx + inner.w && cy >= inner.cy && cy < inner.cy + inner.h, "she is in there with her").toBe(true);
  });

  it("one grows and one hits: the feeding window is opened by a friend who is nowhere near the fight", () => {
    const sim = party(31);
    sim.state.flags.forest_butterflies = 5;
    sim.command(0, { t: "dev", dev: { op: "tp", zone: "forest", mark: "forest_stone_in" } });
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "forest", mark: "forest_stone_in" } });
    for (let i = 0; i < 6; i++) sim.tick([]);
    const stone = sim.rt.propsByKey.get("forest_stone")!;
    const jane = sim.view(0).player;
    const mina = sim.view(1).player;
    const sc = propCentre(sim.catalog, stone);
    placeUnit(sim, mina, sc.x, sc.y + 12);
    for (let i = 0; i < 2; i++) sim.tick([]);
    sim.command(1, { t: "use" });
    for (let i = 0; i < 4; i++) sim.tick([]);
    const bud = sim.rt.propsByKey.get("forest_stone_verbprop_bud1")!;
    expect(bud.hidden, "the stone opened the glade for both of them").toBe(false);

    // Mina grows a bud at the far end. Jane never leaves the middle.
    const bc = propCentre(sim.catalog, bud);
    placeUnit(sim, mina, bc.x, bc.y + 10);
    for (let i = 0; i < 2; i++) sim.tick([]);
    mina.mp = 400;
    mina.cooldowns = {};
    mina.gcd = 0;
    sim.command(1, { t: "cast", spell: "grow" });
    for (let i = 0; i < 400; i++) sim.tick([]);
    const boss = sim.rt.unitsByKey.get("emperor")!;
    expect(bud.used, "she grew it from over there").toBe(true);
    expect(Math.abs(boss.x - bc.x) + Math.abs(boss.y - bc.y), "and it came to feed").toBeLessThan(90);
    expect(jane.alive && mina.alive).toBe(true);
  });
});
