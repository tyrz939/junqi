// The Burial Chamber, re-hosted on the dungeon generator (DUNGEONS.md 3.5). Many seeds, zero
// fallbacks, the contract the story already leans on, and then the one idea in play: cold
// light shows what is there, warm light keeps it off. The snake's corner is a set piece and
// its own proof is still test/dungeons.test.ts, unchanged.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { cellOf, centre } from "@/sim/grid";
import { bagCount } from "@/sim/inventory";
import { litAt } from "@/sim/light";
import { propCentre } from "@/sim/runtime";
import type { World } from "@/sim/runtime";
import { CELL } from "@/sim/constants";
import { NO_INPUT, Sim, type InputFrame } from "@/sim/sim";
import type { Unit } from "@/sim/state";
import { maxHp, placeUnit } from "@/sim/units";
import { ZONE_ATTEMPTS, type Blueprint } from "@/world/blueprint";
import { BURIAL } from "@/world/burial";
import { DUNGEONS } from "@/world/dungeon";
import { checkDungeon, firstCompletion } from "@/world/dungeon/checks";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { buildZone, CONTRACTS } from "@/world/index";
import { hasZone } from "@/world/registry";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = buildCatalog();
const def = DUNGEONS.burial;
/** 64 in the suite; `DUNGEON_SEEDS=1000 npx vitest run test/burial.test.ts` is the soak. */
const SEEDS = Array.from({ length: Number(process.env.DUNGEON_SEEDS ?? 64) }, (_, i) => 31 + i * 7919);

const built = new Map<number, Blueprint>();
function burial(seed: number): Blueprint {
  let bp = built.get(seed);
  if (!bp) built.set(seed, (bp = buildZone("burial", seed)));
  return bp;
}

describe("the generated burial chamber", () => {
  it("64 seeds: every one is proven (solver and C1 to C12) inside the attempt budget, and none needs the fallback", { timeout: 180_000 }, () => {
    const t0 = performance.now();
    let worst = 0;
    for (const seed of SEEDS) {
      const bp = burial(seed);
      const info = infoOf(bp)!;
      expect(info.layout!.fallback, `seed ${seed} fell back to the hand-placed layout`).toBe(false);
      expect(bp.attempts, `seed ${seed}`).toBeLessThan(ZONE_ATTEMPTS);
      worst = Math.max(worst, bp.attempts);
      expect(checkDungeon(bp, catalog), `seed ${seed}`).toEqual([]);
      const used = info.layout!.placements.map((p) => p.template);
      expect(new Set(used).size).toBe(used.length);
      for (const n of def.nodes) if (n.critical) expect(info.rooms.some((r) => r.node.id === n.id), `seed ${seed}: ${n.id}`).toBe(true);
    }
    expect((performance.now() - t0) / SEEDS.length, "ms per burial, proofs included").toBeLessThan(700);
    expect(worst).toBeLessThanOrEqual(5);
  });

  it("is a pure function of (seed, attempt): same seed, same chamber", () => {
    const a = buildDungeon(def, 4242, 0);
    const b = buildDungeon(def, 4242, 0);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
    expect(a.props).toEqual(b.props);
    expect(a.units).toEqual(b.units);
    expect(a.marks).toEqual(b.marks);
    expect(a.triggers).toEqual(b.triggers);
    expect(Buffer.from(buildDungeon(def, 4243, 0).tiles).equals(Buffer.from(a.tiles))).toBe(false);
  });

  it("the contract derived from the mission's binds contains every name the story already leans on", () => {
    for (const what of ["units", "props", "marks", "rects"] as const) {
      for (const name of CONTRACTS.burial[what]) expect(BURIAL.contract[what], `${what}: ${name}`).toContain(name);
    }
    // The story's own trigger rows say `everywhere`; the generator calls the whole map `<zone>_all`.
    expect(burial(31).rects.everywhere).toEqual({ cx: 0, cy: 0, w: burial(31).w, h: burial(31).h });
  });

  it("the first completion is the walk that was designed: the fire before the corners, Goldskin last but one", () => {
    for (const seed of SEEDS.slice(0, 6)) {
      const bp = burial(seed);
      const walk = firstCompletion(bp, catalog, infoOf(bp)!);
      expect(walk.errors).toEqual([]);
      expect(walk.order).toEqual([
        "entry", "hall", "alcove", "east", "rat", "garden", "orchard", "vigil",
        "lockin", "glasshouse", "snake", "web", "nursery", "dark", "parade", "vault", "stair",
      ]);
      expect(walk.cells).toBeGreaterThanOrEqual(def.budget.critPathCells[0]);
      expect(walk.cells).toBeLessThanOrEqual(def.budget.critPathCells[1]);
    }
  });

  it("layouts really differ: the hall and the vault stand in many different bays", () => {
    const halls = new Set<string>();
    const vaults = new Set<string>();
    for (const seed of SEEDS.slice(0, 32)) {
      const layout = infoOf(burial(seed))!.layout!;
      halls.add(String(layout.placements.find((p) => p.node === "hall")!.bay));
      vaults.add(String(layout.placements.find((p) => p.node === "vault")!.bay));
    }
    expect(halls.size).toBeGreaterThanOrEqual(4);
    expect(vaults.size).toBeGreaterThanOrEqual(4);
  });

  it("every unit is at phase 5, and the rest room and the teach room are empty", () => {
    for (const seed of SEEDS.slice(0, 8)) {
      const bp = burial(seed);
      for (const u of bp.units) expect(u.phase).toBe(5);
      for (const room of infoOf(bp)!.rooms) {
        if (room.node.heat !== 0) continue;
        expect(bp.units.filter((u) => u.key.startsWith(`burial_${room.node.id}_spawn_`)).length, room.node.id).toBe(0);
      }
    }
  });

  it("the hand-placed fallback is a whole, proven chamber: the last attempt never throws at the player", () => {
    const bp = BURIAL.build(99, ZONE_ATTEMPTS - 1);
    expect(infoOf(bp)!.layout!.fallback).toBe(true);
    expect(validateBlueprint(bp, catalog, BURIAL.contract, def.givenKeys, { verbs: def.givenVerbs }).errors).toEqual([]);
    expect(checkDungeon(bp, catalog)).toEqual([]);
    expect(infoOf(bp)!.rooms.map((r) => r.node.id).sort()).toEqual(def.nodes.filter((n) => n.critical).map((n) => n.id).sort());
    const other = BURIAL.build(7, ZONE_ATTEMPTS - 1);
    expect(Buffer.from(other.tiles).equals(Buffer.from(bp.tiles))).toBe(true);
  });

  it("the stair up is the way into the School, and it leads there only once the School is built", () => {
    // A door into a zone that does not exist is a crash rather than a mystery, so the zone file
    // takes the destination off until the School lands. The door has no lock: the room it stands
    // in opens when Goldskin is down, and that is the only way to it.
    const stair = burial(31).props.find((p) => p.key === "stair_up")!;
    expect(stair.locked ?? false).toBe(false);
    expect(stair.keyTag).toBeUndefined();
    expect(stair.to).toEqual(hasZone("school") ? { zone: "school", mark: "boiler" } : undefined);
  });
});

// --- in play: cold light, warm light --------------------------------------------------------

function world(sim: Sim): World {
  return sim as unknown as World;
}

function lit(sim: Sim, x: number, y: number, warmOnly = false): boolean {
  return litAt(world(sim), x, y, warmOnly);
}

/**
 * Aim at a prop's middle and cast. A bolt dies on the first sight-blocking cell, so a thing
 * that answers fire is lit by standing so that the wall behind it stops the bolt: she tries a
 * side, and if nothing catches she walks round to the next one, as a person would.
 */
function burn(sim: Sim, key: string, spell = "fireball"): void {
  const p = sim.rt.propsByKey.get(key)!;
  const d = sim.catalog.props[p.def];
  const c = propCentre(sim.catalog, p);
  const spots: [number, number][] = [
    [c.x, (p.cy + d.h) * CELL + 6],
    [c.x, p.cy * CELL - 6],
    [p.cx * CELL - 6, c.y],
    [(p.cx + d.w) * CELL + 6, c.y],
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
    idle(sim, 30);
    const now = sim.rt.propsByKey.get(key)!;
    if (now.on || now.hidden) return;
  }
  throw new Error(`nothing she did lit ${key}`);
}

/** Stand in the middle of a named room: the zones are big and these tests are about one room. */
function stand(sim: Sim, rect: string): void {
  const r = sim.rt.bp.rects[rect];
  placeUnit(sim, sim.player, centre(r.cx + (r.w >> 1)), centre(r.cy + (r.h >> 1)));
  idle(sim, 2);
}

function godIn(seed: number, cat = yardCatalog()): Sim {
  const sim = simIn(cat, "burial", seed);
  sim.command({ t: "dev", dev: { op: "god", on: true } });
  sim.command({ t: "dev", dev: { op: "learn", spell: "icebolt" } });
  idle(sim, 2);
  return sim;
}

describe("cold light shows what is there, warm light keeps it off", () => {
  it("a brazier is warm and a blue torch is cold: only one of them counts against a shade", () => {
    const sim = godIn(71);
    sim.command({ t: "dev", dev: { op: "learn", spell: "fireball" } });
    const brazier = sim.rt.propsByKey.get("dark_brazier_a")!;
    const bc = propCentre(sim.catalog, brazier);
    stand(sim, "burial_dark");
    expect(brazier.on).toBe(false);
    expect(lit(sim, bc.x, bc.y)).toBe(false);
    burn(sim, "dark_brazier_a");
    expect(brazier.on).toBe(true);
    expect(lit(sim, bc.x, bc.y)).toBe(true);
    expect(lit(sim, bc.x, bc.y, true), "a brazier is warm").toBe(true);

    // The blue torches answer frost, and their light is marked cold.
    const cold = sim.rt.propsByKey.get("torch_a")!;
    const cc = propCentre(sim.catalog, cold);
    stand(sim, "burial_alcove");
    burn(sim, "torch_a", "icebolt");
    expect(cold.on).toBe(true);
    expect(lit(sim, cc.x, cc.y), "cold light is light").toBe(true);
    expect(lit(sim, cc.x, cc.y, true), "cold light is not warm").toBe(false);
  });

  it("a shade will not step into warm light: it comes to the edge of it and waits", () => {
    const sim = godIn(71);
    sim.command({ t: "dev", dev: { op: "learn", spell: "fireball" } });
    const brazier = sim.rt.propsByKey.get("dark_brazier_a")!;
    const bc = propCentre(sim.catalog, brazier);
    stand(sim, "burial_dark");
    burn(sim, "dark_brazier_a");
    expect(brazier.on).toBe(true);

    // Her, in the light. It, in the dark, twenty cells off, and set on her.
    // Beside the brazier, on open floor: stood on its footprint she would be a goal nothing can path to.
    const floor = sim.rt.grid.nearestFree(cellOf(bc.x) + 1, cellOf(bc.y) + 2, 4, sim.player.id)!;
    placeUnit(sim, sim.player, centre(floor.cx), centre(floor.cy));
    const shade = sim.rt.unitsByKey.get("dark_shade_a")!;
    placeUnit(sim, shade, bc.x + 170, bc.y);
    shade.homeX = shade.x;
    shade.homeY = shade.y;
    // Nothing notices a god, and the county's creatures no longer hunt each other (sim/units.ts
    // isEnemy), so she comes out of god mode and it is set on her: the question is the light.
    sim.command({ t: "dev", dev: { op: "god", on: false } });
    shade.target = sim.player.id;
    shade.combat = "combat";
    const startedAt = Math.hypot(shade.x - sim.player.x, shade.y - sim.player.y);
    expect(lit(sim, sim.player.x, sim.player.y, true), "she is standing in it").toBe(true);
    let everInLight = false;
    for (let t = 0; t < 600; t++) {
      sim.tick(NO_INPUT);
      if (lit(sim, shade.x, shade.y, true)) everInLight = true;
    }
    expect(everInLight, "a shade never stands in warm light").toBe(false);    expect(Math.hypot(shade.x - sim.player.x, shade.y - sim.player.y)).toBeLessThan(startedAt);
    expect(shade.alive).toBe(true);
  });

  it("the great torch is a room she pushes: a solo bot walks it up the dark hall and the light goes with it", () => {
    const sim = godIn(71);
    stand(sim, "burial_dark");
    const torch = sim.rt.propsByKey.get("great_torch")!;
    expect(sim.catalog.props[torch.def].push).toBe(true);
    const from = { cx: torch.cx, cy: torch.cy };
    // She stands against its west face and walks east, holding USE. Thirty ticks and twenty
    // energy a cell: this is the push a person makes, not a test moving a prop.
    let moved = 0;
    for (let cell = 0; cell < 4; cell++) {
      const p = sim.rt.propsByKey.get("great_torch")!;
      placeUnit(sim, sim.player, p.cx * CELL - 5, centre(p.cy) + 4);
      sim.player.energy = 100;
      const was = p.cx;
      const hold: InputFrame = { mx: 1, my: 0, sprint: false, useHeld: true, ax: 0, ay: 0 };
      for (let t = 0; t < 90 && p.cx === was; t++) sim.tick(hold);
      if (p.cx > was) moved++;
    }
    const now = sim.rt.propsByKey.get("great_torch")!;
    expect(moved, "four cells of pushing").toBe(4);
    expect(now.cx).toBe(from.cx + 4);
    expect(now.cy).toBe(from.cy);
    // The safe room moved with her: warm light where it is now, none where it was.
    const c = propCentre(sim.catalog, now);
    expect(lit(sim, c.x, c.y, true)).toBe(true);
    expect(lit(sim, centre(from.cx) - 40, centre(from.cy), true)).toBe(false);
  });
});

// --- Goldskin -------------------------------------------------------------------------------

describe("Goldskin", () => {
  function inTheVault(seed: number): { sim: Sim; boss: Unit } {
    const sim = godIn(seed);
    sim.command({ t: "dev", dev: { op: "learn", spell: "fireball" } });
    sim.command({ t: "dev", dev: { op: "tp", zone: "burial", mark: "goldskin_shade_a" } });
    idle(sim, 5);
    return { sim, boss: sim.rt.unitsByKey.get("goldskin")! };
  }

  it("is 4,000 at phase 5, gilded against everything, and still comes apart without fire", () => {
    const { sim, boss } = inTheVault(93);
    expect(maxHp(boss)).toBe(4020);
    const hp = boss.hp;
    boss.incoming.push({ amount: 500, school: "physical", from: sim.player.id, crit: false });
    idle(sim, 2);
    // resist 0.8: a fifth of it lands, and a fifth of it is not nothing.
    expect(hp - boss.hp).toBe(100);
  });

  it("fire opens him: a brazier lit in his hall softens him for six seconds", () => {
    const { sim, boss } = inTheVault(93);
    expect(boss.statuses.some((s) => s.effect === "softened")).toBe(false);
    burn(sim, "vault_brazier_a");
    expect(sim.rt.propsByKey.get("vault_brazier_a")!.on).toBe(true);
    expect(boss.statuses.some((s) => s.effect === "softened"), "the gold runs").toBe(true);
    const hp = boss.hp;
    boss.incoming.push({ amount: 500, school: "physical", from: sim.player.id, crit: false });
    idle(sim, 2);
    expect(hp - boss.hp).toBe(500);
  });

  it("puts every flame out at each phase, and stands things up in the dark", () => {
    const { sim, boss } = inTheVault(93);
    burn(sim, "vault_brazier_a");
    expect(sim.rt.propsByKey.get("vault_brazier_a")!.on).toBe(true);
    boss.incoming.push({ amount: maxHp(boss) * 0.3, school: "physical", from: sim.player.id, crit: false });
    idle(sim, 3);
    expect(boss.phase).toBeGreaterThanOrEqual(1);
    for (const key of ["vault_brazier_a", "vault_brazier_b", "vault_brazier_c", "vault_brazier_d"]) {
      expect(sim.rt.propsByKey.get(key)!.on, key).toBe(false);
    }
    expect(sim.rt.unitsByKey.has("burial_gold_a"), "something stands up").toBe(true);
    // And a brazier that is off answers a fireball again.
    burn(sim, "vault_brazier_b");
    expect(sim.rt.propsByKey.get("vault_brazier_b")!.on).toBe(true);
  });
});

// --- a solo bot, the whole chamber -----------------------------------------------------------

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

function slay(sim: Sim, key: string, loot = true): void {
  const u = sim.rt.unitsByKey.get(key);
  expect(u, `unit ${key}`).toBeTruthy();
  const unit = u as Unit;
  walkTo(sim, unit.x, unit.y, 40);
  unit.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 4);
  expect(unit.alive, key).toBe(false);
  if (!loot) return;
  for (const d of [...sim.zone.drops]) {
    if (Math.abs(d.x - unit.x) + Math.abs(d.y - unit.y) > 80) continue;
    walkTo(sim, d.x, d.y, 5);
    sim.command({ t: "use" });
  }
}

describe("a solo bot in the generated burial chamber", () => {
  const botCatalog = yardCatalog();
  for (const seed of [12, 90210]) {
    it(`seed ${seed}: the cold torches, the snake, Fire, both webs, the great torch, the seal, Goldskin`, { timeout: 180_000 }, () => {
      const sim = simIn(botCatalog, "burial", seed);
      const p = sim.player;
      const prop = (key: string) => sim.rt.propsByKey.get(key)!;
      sim.command({ t: "dev", dev: { op: "god", on: true } });
      sim.command({ t: "dev", dev: { op: "learn", spell: "icebolt" } });
      sim.command({ t: "dev", dev: { op: "quest", quest: "the_burial" } });
      idle(sim, 2);
      expect(sim.state.flags["been:burial"]).toBe(1);

      // The wall notice is the map.
      useProp(sim, "burial_entry_notice_main");
      talkThrough(sim);

      // West: the cold torches only wake for frost, and both lit unlocks the chest.
      expect(prop("torch_chest").locked).toBe(true);
      burn(sim, "torch_a", "icebolt");
      burn(sim, "torch_b", "icebolt");
      idle(sim, 3);
      expect(prop("torch_a").on && prop("torch_b").on).toBe(true);
      expect(prop("torch_chest").locked).toBe(false);
      useProp(sim, "torch_chest");
      useProp(sim, "snake_key_chest");
      expect(bagCount(p, "key_snake")).toBe(1);

      // The scaled door, the lock-in, the giant key.
      useProp(sim, "gate_snake");
      expect(walkToProp(sim, "giant_key_chest")).toBe(true);
      idle(sim, 3);
      expect(prop("gate_snake").locked, "the floor stood up behind her").toBe(true);
      for (const g of ["guard_a", "guard_b", "guard_c", "guard_d"]) slay(sim, g, false);
      idle(sim, 3);
      expect(prop("giant_key_chest").locked).toBe(false);
      useProp(sim, "giant_key_chest");
      expect(bagCount(p, "key_snake_boss")).toBe(1);

      // The snake keeps its own room, and its south door has no keyhole until it is still.
      useProp(sim, "snake_gate_east");
      expect(prop("snake_gate_south").locked).toBe(true);
      slay(sim, "burial_snake", false);
      idle(sim, 5);
      expect(sim.state.flags.snake_down).toBe(1);
      expect(prop("snake_gate_south").locked).toBe(false);

      // East: the small snakes are fed, not fought, so she leaves them and takes the chest.
      useProp(sim, "lurker_chest");
      expect(bagCount(p, "key_burial")).toBe(1);

      // The garden: the flower holds the roots shut over the scroll.
      expect(prop("root_a").hidden).toBe(false);
      expect(prop("fire_scroll").locked).toBe(true);
      slay(sim, "garden_flower", false);
      idle(sim, 5);
      expect(prop("root_a").hidden).toBe(true);
      expect(prop("fire_scroll").locked).toBe(false);
      expect(p.book).not.toContain("fireball");
      useProp(sim, "fire_scroll");
      talkThrough(sim);
      expect(p.book).toContain("fireball");

      // The orchard: the first thing Fire is good for is a fire.
      expect(prop("orchard_fire").hidden).toBe(true);
      burn(sim, "cold_hearth");
      expect(prop("cold_hearth").hidden).toBe(true);
      expect(prop("orchard_fire").hidden).toBe(false);
      const strength = p.strength;
      useProp(sim, "burial_orchard_jar_main");
      expect(p.strength).toBe(strength + 2);

      // The glasshouse: a lock-in, and half a jar.
      useProp(sim, "gate_glasshouse");
      expect(walkToProp(sim, "glasshouse_jar")).toBe(true);
      idle(sim, 3);
      expect(prop("gate_glasshouse").locked).toBe(true);
      expect(prop("burial_glasshouse_wayin").hidden, "the way in is shown while it is shut").toBe(false);
      slay(sim, "great_flower", false);
      idle(sim, 5);
      expect(sim.state.flags.flower_down).toBe(1);
      expect(prop("gate_glasshouse").locked).toBe(false);
      const half = p.strength;
      useProp(sim, "glasshouse_jar");
      expect(p.strength).toBe(half + 7);

      // North: the web across the passage, and the Spider behind it.
      expect(prop("web_wall").hidden).toBe(false);
      burn(sim, "web_wall");
      expect(prop("web_wall").hidden).toBe(true);
      burn(sim, "web_alcove");
      expect(prop("web_alcove").hidden).toBe(true);
      useProp(sim, "burial_web_page_main");
      slay(sim, "spider_queen", false);
      idle(sim, 5);
      expect(sim.state.flags.spider_down).toBe(1);

      // South: warm light opens the passage, and the great torch is how she crosses the hall.
      expect(prop("gate_dark").locked).toBe(true);
      burn(sim, "hall_brazier");
      idle(sim, 3);
      expect(sim.state.flags.burial_south).toBe(1);
      expect(prop("gate_dark").locked).toBe(false);
      const torch = prop("great_torch");
      const was = torch.cx;
      placeUnit(sim, p, torch.cx * CELL - 5, centre(torch.cy) + 4);
      p.energy = 100;
      const hold: InputFrame = { mx: 1, my: 0, sprint: false, useHeld: true, ax: 0, ay: 0 };
      for (let t = 0; t < 90 && torch.cx === was; t++) sim.tick(hold);
      expect(torch.cx, "she pushed it a cell").toBe(was + 1);
      burn(sim, "dark_brazier_a");
      expect(prop("dark_brazier_a").on).toBe(true);
      slay(sim, "the_soldier", false);
      idle(sim, 5);
      expect(sim.state.flags.soldier_down).toBe(1);

      // The seal: four names, and four flames that light as she walks back into the hall.
      expect(prop("gate_wizard").locked).toBe(true);
      expect(prop("hall_flame_a").on).toBe(false);
      useProp(sim, "wizard_seal");
      idle(sim, 3);
      expect(prop("hall_flame_a").on, "a flame in each corner, and nobody lit them").toBe(true);
      expect(sim.state.flags.burial_four).toBe(1);
      expect(prop("gate_wizard").locked).toBe(false);

      // The vault seals behind her. Fire opens him; she could do it without.
      useProp(sim, "goldskin_stone");
      talkThrough(sim);
      idle(sim, 3);
      expect(prop("gate_wizard").locked).toBe(true);
      const boss = sim.rt.unitsByKey.get("goldskin")!;
      expect(boss.combat).toBe("combat");
      burn(sim, "vault_brazier_a");
      expect(boss.statuses.some((s) => s.effect === "softened")).toBe(true);
      slay(sim, "goldskin", false);
      idle(sim, 5);
      expect(sim.state.flags.burial_cleared).toBe(1);
      expect(prop("gate_wizard").locked).toBe(false);

      // The Ball, the big jar, and a stair that goes up, into the School's boiler room.
      const before = p.strength;
      useProp(sim, "burial_big_jar");
      expect(p.strength).toBe(before + 14);
      useProp(sim, "ball_chest");
      expect(bagCount(p, "the_ball")).toBe(1);
      expect(prop("stair_up").locked ?? false, "the stair up is open once he is down").toBe(false);
      expect(prop("stair_up").to).toEqual(hasZone("school") ? { zone: "school", mark: "boiler" } : undefined);
      expect(sim.me.zone).toBe("burial");
      // The way back: the stair comes out at the fire.
      expect(prop("gate_back").locked).toBe(false);
      expect(walkToProp(sim, "vigil_fire")).toBe(true);
    });
  }
});



// --- two seats ------------------------------------------------------------------------------

/** `who` are client tokens, not names: everyone plays the one heroine. The host opens the world. */
function party(seed: number): Sim {
  const sim = Sim.newGame(yardCatalog(), seed);
  sim.command(0, { t: "open", on: true });
  sim.command(-1, { t: "join", who: "Mina" });
  sim.drainEvents();
  for (const seat of [0, 1]) {
    sim.command(seat, { t: "dev", dev: { op: "god", on: true } });
    sim.command(seat, { t: "dev", dev: { op: "tp", zone: "burial", mark: "entry" } });
  }
  for (let i = 0; i < 3; i++) sim.tick([]);
  return sim;
}

describe("two seats in the burial chamber", () => {
  it("guard the pusher: one walks the great torch up the hall and the other stands in its light", () => {
    const sim = party(71);
    const torch = sim.rt.propsByKey.get("great_torch")!;
    const jane = sim.view(0).player;
    const mina = sim.view(1).player;
    placeUnit(sim, jane, torch.cx * CELL - 5, centre(torch.cy) + 4);
    placeUnit(sim, mina, (torch.cx + 2) * CELL + 10, centre(torch.cy) + 4);
    jane.energy = 100;
    const was = torch.cx;
    const hold: InputFrame = { mx: 1, my: 0, sprint: false, useHeld: true, ax: 0, ay: 0 };
    for (let t = 0; t < 90 && torch.cx === was; t++) sim.tick([hold, NO_INPUT]);
    expect(torch.cx, "one of them pushed").toBe(was + 1);
    // The moving safe room: both of them are inside it, and the one who did nothing is safest.
    expect(lit(sim, jane.x, jane.y, true)).toBe(true);
    expect(lit(sim, mina.x, mina.y, true)).toBe(true);
    const shade = sim.rt.unitsByKey.get("dark_shade_a")!;
    placeUnit(sim, shade, (torch.cx + 2) * CELL + 150, centre(torch.cy));
    shade.homeX = shade.x;
    shade.homeY = shade.y;
    let everInLight = false;
    for (let t = 0; t < 400; t++) {
      sim.tick([]);
      if (lit(sim, shade.x, shade.y, true)) everInLight = true;
    }
    expect(everInLight, "it waits at the edge of the light").toBe(false);
    expect(mina.alive && jane.alive).toBe(true);
  });

  it("a lock-in shows a way in, and the friend who was late climbs in after her", () => {
    const sim = party(71);
    sim.command(0, { t: "dev", dev: { op: "tp", zone: "burial", mark: "flower_seed_a" } });
    for (let i = 0; i < 5; i++) sim.tick([]);
    const gate = sim.rt.propsByKey.get("gate_glasshouse")!;
    const wayIn = sim.rt.propsByKey.get("burial_glasshouse_wayin")!;
    expect(gate.locked, "it sealed behind her").toBe(true);
    expect(wayIn.hidden, "and showed the way in").toBe(false);

    const mina = sim.view(1).player;
    const c = propCentre(sim.catalog, wayIn);
    // It stands beside the gate she cannot open, so she comes at it from the other side.
    const gc = propCentre(sim.catalog, gate);
    const ax = Math.sign(c.x - gc.x) || 0;
    const ay = Math.sign(c.y - gc.y) || 1;
    placeUnit(sim, mina, c.x + ax * 10, c.y + ay * 10);
    const toward: InputFrame = { mx: -ax, my: -ay, sprint: false, useHeld: false, ax: 0, ay: 0 };
    for (let i = 0; i < 3; i++) sim.tick([NO_INPUT, toward, NO_INPUT, NO_INPUT]);
    sim.command(1, { t: "use" });
    for (let i = 0; i < 4; i++) sim.tick([]);
    const inner = sim.rt.bp.rects.burial_glasshouse_inner;
    const cx = Math.floor(mina.x / CELL);
    const cy = Math.floor(mina.y / CELL);
    expect(cx >= inner.cx && cx < inner.cx + inner.w && cy >= inner.cy && cy < inner.cy + inner.h, "she is in there with her").toBe(true);

    // And when the room is clear the way in is gone again: it is never a way round the lock.
    const boss = sim.rt.unitsByKey.get("great_flower")!;
    boss.incoming.push({ amount: 1e6, school: "physical", from: sim.view(0).player.id, crit: false });
    for (let i = 0; i < 6; i++) sim.tick([]);
    expect(gate.locked).toBe(false);
    expect(wayIn.hidden).toBe(true);
  });
});




