// The ruined library (DUNGEONS.md 3.3, "Where Grow is learned"). Four rooms, nothing alive in
// them, one page left in the book, and a dry planter under the hole in the roof to try it on
// before anything asks for it. The tests hold it to that: many seeds proven, the same seed the
// same library, a solo bot that reads the page and grows the planter, and a second seat who
// can stand on the plate instead of pushing the steps onto it.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { centre, Tile } from "@/sim/grid";
import { moveProp, propCentre } from "@/sim/runtime";
import { Sim } from "@/sim/sim";
import type { Sim as SimType } from "@/sim/sim";
import { placeUnit } from "@/sim/units";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { DUNGEONS } from "@/world/dungeon";
import { checkDungeon, contractOf, firstCompletion, lintDef } from "@/world/dungeon/checks";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { buildZone, CONTRACTS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.library;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/library.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 31 + i * 7919);

const built = new Map<number, Blueprint>();
function library(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("library", seed)));
  return bp;
}

describe("the ruined library", () => {
  it("the mission itself is sound before any layout exists", () => {
    expect(lintDef(def, catalog)).toEqual([]);
  });

  it("64 seeds: every one is proven (solver and C1 to C12) inside the attempt budget, and none needs the fallback", { timeout: 120_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = library(seed);
      const info = infoOf(bp)!;
      expect(info.layout!.fallback, `seed ${seed} fell back to the hand-placed layout`).toBe(false);
      expect(bp.attempts, `seed ${seed}`).toBeLessThan(ZONE_ATTEMPTS);
      worst = Math.max(worst, bp.attempts);
      expect(checkDungeon(bp, catalog), `seed ${seed}`).toEqual([]);
      for (const n of def.nodes) expect(info.rooms.some((r) => r.node.id === n.id), `seed ${seed}: ${n.id}`).toBe(true);
      // Nothing is alive in here. That is the whole point of the room the verb is learned in.
      expect(bp.units.length, `seed ${seed}`).toBe(0);
    }
    expect((performance.now() - t0) / SEEDS.length, "ms per library, proofs included").toBeLessThan(120);
    expect(worst).toBeLessThanOrEqual(2);
  });

  it("is a pure function of (seed, attempt): same seed, same library; another seed, another library", () => {
    const a = buildDungeon(def, 4242, 0);
    const b = buildDungeon(def, 4242, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.marks).toEqual(b.marks);
    expect(a.rects).toEqual(b.rects);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 4243, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the contract names the story leans on all exist, on every seed", () => {
    const contract = contractOf(def);
    for (const name of ["library_door_out", "library_book", "library_planter", "library_vine_jar", "library_page", "library_fire", "library_plate", "library_steps"]) {
      expect(contract.props, name).toContain(name);
    }
    expect(contract.marks).toContain("entry");
    expect(contract.rects).toContain("library_vine");
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = library(seed);
      for (const name of contract.props) expect(bp.props.some((p) => p.key === name), `seed ${seed}: ${name}`).toBe(true);
      for (const name of contract.marks) expect(bp.marks[name], `seed ${seed}: ${name}`).toBeDefined();
      expect(CONTRACTS.library).toEqual(contract);
    }
  });

  it("layouts really differ: over the seeds the four rooms stand in many different bays", () => {
    const shelves = new Set<string>();
    const nooks = new Set<string>();
    for (const seed of SEEDS.slice(0, 32)) {
      const layout = infoOf(library(seed))!.layout!;
      shelves.add(String(layout.placements.find((p) => p.node === "shelf")!.bay));
      nooks.add(String(layout.placements.find((p) => p.node === "nook")!.bay));
    }
    expect(shelves.size).toBeGreaterThanOrEqual(4);
    expect(nooks.size).toBeGreaterThanOrEqual(4);
  });

  it("the hand-placed fallback is a whole, proven library: the last attempt never throws at the player", () => {
    const bp = buildDungeon(def, 99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, CONTRACTS.library, def.givenKeys, { verbs: def.givenVerbs }).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
  });

  it("the first completion is the walk that was designed: the fire before the last book, and the way back is short", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = library(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual(["entry", "stacks", "nook", "shelf"]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });

  it("C5 really rejects: take the light away from the planter and the room no longer teaches Grow in safety", () => {
    const bp = buildZone("library", 31);
    // The planter is the first use, and it is the first use because it answers the verb. Take
    // the row's answer away and nothing in the room can be tried.
    const cat = buildCatalog();
    cat.props.library_planter = { ...cat.props.library_planter, answers: undefined };
    expect(checkDungeon(bp, cat).join("\n")).toMatch(/C5: shelf teaches grow and has nothing to try it on/);
  });
});

// --- a solo bot, the whole library ---------------------------------------------------------

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
  idle(sim, 2);
  expect(sim.rt.propsByKey.get(key)!.used, `${key} grew`).toBe(true);
}

describe("a solo bot in the ruined library", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 2026]) {
    it(`seed ${seed}: the notice, the fire, the last book, the planter, the jar behind the vine and the pressed page`, { timeout: 60_000 }, () => {
      const sim = simIn(botCatalog, "library", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      idle(sim, 2);

      // The notice is the map: a held notice with no use of its own reveals every room.
      useProp(sim, "library_entry_notice_main");
      expect(sim.me.dialogue).not.toBeNull();
      talkThrough(sim);

      // The reading room fire: the party's fire moves in here.
      useProp(sim, "library_fire");
      talkThrough(sim);
      expect(sim.state.rest!.zone).toBe("library");

      // Grow is not hers until she reads the page, and the solver is held to that too.
      expect(p.book).not.toContain("grow");
      useProp(sim, "library_book");
      talkThrough(sim);
      expect(p.book).toContain("grow");

      // The safe first use, in the same room, standing in the light that comes through the roof.
      expect(prop("library_vine_jar").hidden).toBe(true);
      const vine = sim.rt.bp.rects.library_vine;
      expect(sim.rt.grid.tileAt(vine.cx, vine.cy)).not.toBe(Tile.GrownPath);
      grow(sim, "library_planter");
      expect(sim.rt.grid.tileAt(vine.cx, vine.cy)).toBe(Tile.GrownPath);
      expect(prop("library_vine_jar").hidden).toBe(false);
      const strength = p.strength;
      useProp(sim, "library_vine_jar");
      expect(p.strength).toBe(strength + 2);
      useProp(sim, "library_vine_jar");
      expect(p.strength, "a jar is found once, whoever asks").toBe(strength + 2);

      // Nothing here will grow in the dark: the hole in the roof is the whole rule.
      expect(sim.catalog.props.library_planter.answers).toBe("grow");
      expect(sim.catalog.props.library_beam.light).toBeDefined();

      // The steps on the plate hold the pressed page down. Alone, she pushes them.
      expect(prop("library_page").locked).toBe(true);
      moveProp(sim, prop("library_steps"), prop("library_plate").cx, prop("library_plate").cy);
      idle(sim, 12);
      expect(prop("library_page").locked).toBe(false);
      const spirit = p.spirit;
      useProp(sim, "library_page");
      expect(p.spirit).toBe(spirit + 2);

      // And the short way back to the door she came in by.
      expect(walkToProp(sim, "library_door_out")).toBe(true);
    });
  }
});

// --- two seats -------------------------------------------------------------------------------

function party(seed: number): SimType {
  const sim = Sim.newGame(yardCatalog(), seed);
  sim.command(0, { t: "open", on: true });
  sim.command(-1, { t: "join", who: "Mina" });
  sim.drainEvents();
  for (const seat of [0, 1]) {
    sim.command(seat, { t: "dev", dev: { op: "god", on: true } });
    sim.command(seat, { t: "dev", dev: { op: "tp", zone: "library", mark: "entry" } });
  }
  for (let i = 0; i < 3; i++) sim.tick([]);
  return sim;
}

describe("two seats in the ruined library", () => {
  it("plate or friend: a second pair of boots holds the page down, and the steps never have to be pushed", () => {
    const sim = party(31);
    const plate = sim.rt.propsByKey.get("library_plate")!;
    const page = sim.rt.propsByKey.get("library_page")!;
    const steps = sim.rt.propsByKey.get("library_steps")!;
    const where = { cx: steps.cx, cy: steps.cy };
    expect(page.locked, "it starts held shut").toBe(true);

    const mina = sim.view(1).player;
    const c = propCentre(sim.catalog, plate);
    placeUnit(sim, mina, c.x, c.y);
    for (let i = 0; i < 6; i++) sim.tick([]);
    expect(page.locked, "she is standing on it").toBe(false);
    expect({ cx: steps.cx, cy: steps.cy }, "nobody had to push anything").toEqual(where);

    // And it is a moment, not a state: step off and the page is the plate's again.
    placeUnit(sim, mina, centre(plate.cx) + 200, centre(plate.cy) + 200);
    for (let i = 0; i < 6; i++) sim.tick([]);
    expect(page.locked).toBe(true);
  });
});
