import { describe, expect, it } from "vitest";
import { bagAdd, bagCount } from "@/sim/inventory";
import { questReady } from "@/sim/quests";
import { moveProp, propCentre } from "@/sim/runtime";
import { NO_INPUT, Sim } from "@/sim/sim";
import type { Unit } from "@/sim/state";
import { SNAKE_FOLLOW_TICKS } from "@/sim/snake";
import { maxHp } from "@/sim/units";
import { idle, talkThrough, walkTo, walkToProp , yardCatalog } from "./bot";

const catalog = yardCatalog();

function godSim(seed: number, zone: "mine" | "burial" | "cellar"): Sim {
  const sim = Sim.newGame(catalog, seed);
  sim.command({ t: "dev", dev: { op: "god", on: true } });
  sim.command({ t: "dev", dev: { op: "tp", zone, mark: zone === "cellar" ? "stair_a" : "entry" } });
  expect(sim.me.zone).toBe(zone);
  return sim;
}

function useProp(sim: Sim, key: string, times = 1): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  for (let i = 0; i < times; i++) {
    sim.command({ t: "use" });
    idle(sim, 2);
  }
}

function lootAllDrops(sim: Sim): void {
  for (const d of [...sim.zone.drops]) {
    walkTo(sim, d.x, d.y, 5);
    sim.command({ t: "use" });
  }
}

/**
 * One unit, by hand. `dev kill` reaches 25 m through walls, and in a generated mine the next
 * room over may be the boss's: the chain must kill who it means to and nobody else.
 */
function slay(sim: Sim, u: Unit): void {
  u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 3);
}

function killAwake(sim: Sim): void {
  sim.command({ t: "dev", dev: { op: "kill" } });
  idle(sim, 3);
}

describe("cellar", () => {
  it("one iron key type opens both iron doors; the storage room wants a plain key", () => {
    const sim = godSim(11, "cellar");
    const p = sim.player;
    useProp(sim, "cellar_chest");
    expect(bagCount(p, "key_basement")).toBe(2);
    useProp(sim, "iron_door_a");
    expect(sim.rt.propsByKey.get("iron_door_a")!.solid).toBe(false);
    expect(bagCount(p, "key_basement")).toBe(1);
    useProp(sim, "storage_gate");
    expect(sim.rt.propsByKey.get("storage_gate")!.locked).toBe(true);
    useProp(sim, "rat_chest");
    useProp(sim, "storage_gate");
    useProp(sim, "storage_chest");
    expect(bagCount(p, "wood")).toBe(4);
    useProp(sim, "iron_door_b");
    useProp(sim, "stair_b");
    expect(sim.me.zone).toBe("house");
    expect(sim.state.flags["been:cellar"]).toBe(1);
  });
});

describe("mine", () => {
  it("plate -> keys -> clerk -> Headmaster -> Repair -> boss key -> vault -> Iron Knuckles", () => {
    const sim = godSim(12, "mine");
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "quest", quest: "the_mine" } });

    // The chest only opens while something sits on the plate.
    useProp(sim, "plate_chest");
    expect(bagCount(p, "key_generic")).toBe(0);
    const plate = sim.rt.propsByKey.get("plate_a")!;
    const barrel = sim.rt.propsByKey.get("plate_barrel")!;
    moveProp(sim, barrel, plate.cx, plate.cy);
    idle(sim, 12);
    expect(plate.on).toBe(true);
    useProp(sim, "plate_chest");
    expect(bagCount(p, "key_generic")).toBe(1);

    useProp(sim, "store_chest");
    expect(bagCount(p, "iron")).toBe(4);
    useProp(sim, "gate_generic_a");
    useProp(sim, "guard_chest");
    const clerk = sim.rt.unitsByKey.get("clerk")!;
    walkTo(sim, clerk.x, clerk.y, 30);
    slay(sim, clerk);
    lootAllDrops(sim);
    expect(bagCount(p, "key_mine_headmaster")).toBe(1);

    useProp(sim, "gate_generic_b");
    useProp(sim, "gate_hm");
    const hm = sim.rt.unitsByKey.get("headmaster")!;
    walkTo(sim, hm.x, hm.y, 30);
    slay(sim, hm);
    lootAllDrops(sim);
    expect(bagCount(p, "key_mine_vault")).toBe(1);

    // Repair is found down here now: in his confiscated drawer, which stayed shut while he stood.
    expect(p.book).not.toContain("repair");
    useProp(sim, "hm_drawer");
    talkThrough(sim);
    expect(p.book).toContain("repair");

    // Repair costs wood, and refuses without it. The mine is generated, so the steps may face any way.
    const steps = sim.rt.propsByKey.get("broken_steps")!;
    expect(walkToProp(sim, "broken_steps"), "walk to broken_steps").toBe(true);
    const wood = bagCount(p, "wood");
    expect(wood).toBe(2);
    sim.command({ t: "cast", spell: "repair" });
    idle(sim, 2);
    expect(steps.hidden).toBe(true);
    expect(bagCount(p, "wood")).toBe(0);

    useProp(sim, "boss_key_chest");
    useProp(sim, "gate_vault");
    useProp(sim, "vault_chest");
    expect(bagCount(p, "key_museum")).toBe(1);
    useProp(sim, "gate_boss");

    const boss = sim.rt.unitsByKey.get("iron_knuckles")!;
    walkTo(sim, boss.x, boss.y + 40, 12);
    idle(sim, 5);
    const gate = sim.rt.propsByKey.get("gate_boss")!;
    expect(gate.locked).toBe(true); // the gate dropped behind us
    expect(boss.combat).toBe("combat");
    slay(sim, boss);
    idle(sim, 5);
    expect(gate.locked).toBe(false);
    expect(questReady(sim, "the_mine")).toBe(true);
  });
});

describe("burial", () => {
  it("cold torches, the lock-in, the giant key, and a snake that obeys its clock", () => {
    const sim = godSim(13, "burial");
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "learn", spell: "icebolt" } });
    killAwake(sim);

    // Icebolt wakes the blue torches; both lit unlocks the alcove chest.
    const chest = sim.rt.propsByKey.get("torch_chest")!;
    expect(chest.locked).toBe(true);
    for (const key of ["torch_a", "torch_b"]) {
      const t = sim.rt.propsByKey.get(key)!;
      const c = propCentre(sim.catalog, t);
      walkTo(sim, c.x, c.y - 30, 4);
      p.cooldowns = {};
      p.gcd = 0;
      p.mp = 150;
      sim.setAim(0, 1);
      sim.command({ t: "cast", spell: "icebolt" });
      idle(sim, 30);
      expect(t.on, key).toBe(true);
    }
    idle(sim, 3);
    expect(chest.locked).toBe(false);

    useProp(sim, "snake_key_chest");
    useProp(sim, "gate_snake");
    const lockGate = sim.rt.propsByKey.get("gate_snake")!;
    expect(lockGate.solid).toBe(false);
    const giant = sim.rt.propsByKey.get("giant_key_chest")!;
    walkToProp(sim, "giant_key_chest");
    idle(sim, 3);
    expect(lockGate.locked).toBe(true);
    expect(sim.rt.unitsByKey.has("guard_a")).toBe(true);
    expect(giant.locked).toBe(true);
    killAwake(sim);
    idle(sim, 3);
    expect(giant.locked).toBe(false);
    expect(lockGate.locked).toBe(false);
    useProp(sim, "giant_key_chest");
    expect(bagCount(p, "key_snake_boss")).toBe(1);

    useProp(sim, "snake_gate_east");
    const snake = sim.rt.unitsByKey.get("burial_snake")!;
    walkTo(sim, snake.homeX + 40, snake.homeY, 10);
    idle(sim, 3);
    expect(snake.combat).toBe("combat");
    expect(sim.rt.propsByKey.get("snake_gate_east")!.locked).toBe(true);
    expect(snake.phase).toBe(0);
    const before = snake.segments!.slice(0, 2);
    // Phase 0 lasts 900 ticks of pursuit, then it goes home to spit rings.
    for (let t = 0; t < SNAKE_FOLLOW_TICKS + 400 && snake.phase === 0; t++) {
      const away = t % 240 < 120 ? 1 : -1;
      sim.tick({ mx: away, my: 0, sprint: false, useHeld: false, ax: 0, ay: 0 });
    }
    expect(snake.phase).toBe(1);
    expect(snake.segments!.slice(0, 2)).not.toEqual(before);
    for (let t = 0; t < 600 && sim.zone.projectiles.length === 0; t++) sim.tick(NO_INPUT);
    expect(sim.zone.projectiles.length).toBeGreaterThanOrEqual(15);

    // A hit on a tail hitbox hurts the head.
    snake.hp = maxHp(snake);
    const n = 7;
    const tx = snake.segments![n * 2];
    const ty = snake.segments![n * 2 + 1];
    sim.zone.projectiles.length = 0;
    sim.zone.projectiles.push({
      id: 999999, spell: "icebolt", from: p.id, target: 0, x: tx - 3, y: ty, vx: 1, vy: 0, left: 50,
      hit: { amount: 40, school: "frost", from: p.id, crit: false }, age: 0,
    });
    idle(sim, 4);
    expect(snake.hp).toBeLessThan(maxHp(snake));

    // After an anti-cheese reset it must pick the fight back up by itself; pillars never cause one.
    // The arena is a generated room now and is wider than the hand-built square was, so she
    // first walks back inside its aggro ring, which is what anyone who had backed off would do.
    walkTo(sim, snake.homeX + 30, snake.homeY, 12);
    snake.combat = "idle";
    snake.target = 0;
    sim.me.god = false;
    idle(sim, 12);
    sim.me.god = true;
    expect(snake.combat).toBe("combat");

    killAwake(sim);
    idle(sim, 5);
    expect(sim.state.flags.snake_down).toBe(1);
    expect(sim.rt.propsByKey.get("snake_gate_south")!.locked).toBe(false);
  });

  it("burial snakes cannot be fought, only fed", () => {
    const sim = godSim(14, "burial");
    const p = sim.player;
    const lurker = sim.rt.unitsByKey.get("lurker_a")!;
    bagAdd(sim, p, "poisoned_rat_meat", 1);
    walkTo(sim, lurker.x - 110, lurker.y, 40); // close enough that the lurker is inside the load ring
    expect(lurker.awake).toBe(true);
    lurker.combat = "idle";
    sim.zone.drops.push({ id: 424242, item: "poisoned_rat_meat", qty: 1, x: lurker.x - 30, y: lurker.y, age: 0 });
    for (let t = 0; t < 600 && lurker.alive; t++) {
      lurker.target = 0;
      lurker.combat = "idle";
      sim.tick(NO_INPUT);
    }
    expect(lurker.alive).toBe(false);
    expect(sim.zone.drops.some((d) => d.id === 424242)).toBe(false);
  });

  it("dying inside a lock-in re-opens the gate and re-arms the trap", () => {
    const sim = Sim.newGame(catalog, 15);
    sim.command({ t: "dev", dev: { op: "tp", zone: "burial", mark: "lockin_a" } });
    sim.me.lastMark = "entry"; // as if she had walked in the front way
    idle(sim, 3);
    const gate = sim.rt.propsByKey.get("gate_snake")!;
    expect(gate.locked).toBe(true);
    sim.player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 300);
    expect(sim.player.alive).toBe(true);
    expect(gate.locked).toBe(false);
    expect(sim.rt.unitsByKey.has("guard_a")).toBe(false);
    expect(sim.zone.triggers.find((t) => t.id === "burial_lockin")!.fired).toBe(false);
  });
});
