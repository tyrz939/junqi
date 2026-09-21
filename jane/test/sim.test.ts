import { describe, expect, it } from "vitest";
import { GCD_TICKS } from "@/sim/constants";
import { tryCast } from "@/sim/combat";
import { centre } from "@/sim/grid";
import { bagAdd, bagCount, craftOutput } from "@/sim/inventory";
import { questDone } from "@/sim/quests";
import { addUnit } from "@/sim/runtime";
import { cloneState, decodeSave, encodeSave, hashState } from "@/sim/save";
import { NO_INPUT, Sim, type InputFrame } from "@/sim/sim";
import { createUnit, maxHp } from "@/sim/units";
import { fight, idle, talkThrough, walkTo, walkToProp, walkToUnit , yardCatalog } from "./bot";

const catalog = yardCatalog();

/** A scripted, seed-independent input tape: wander, sprint, mash the bar. */
function tape(t: number): InputFrame {
  const phase = Math.floor(t / 45) % 8;
  const dirs = [
    [1, 0],
    [1, 1],
    [0, 1],
    [-1, 1],
    [-1, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
  ];
  const [mx, my] = dirs[phase];
  return { mx, my, sprint: t % 200 < 80, useHeld: t % 97 < 20, ax: my, ay: -mx };
}

function run(sim: Sim, from: number, to: number): void {
  for (let t = from; t < to; t++) {
    if (t % 30 === 0) sim.command({ t: "bar", slot: 0 });
    if (t % 111 === 0) sim.command({ t: "use" });
    if (sim.me.dialogue) sim.command({ t: "closeDialogue" });
    sim.tick(tape(t));
  }
}

describe("determinism", () => {
  it("same seed + same inputs = same state hash, tick for tick", () => {
    const a = Sim.newGame(catalog, 777);
    const b = Sim.newGame(catalog, 777);
    for (let chunk = 0; chunk < 10; chunk++) {
      run(a, chunk * 120, (chunk + 1) * 120);
      run(b, chunk * 120, (chunk + 1) * 120);
      expect(hashState(a.state)).toBe(hashState(b.state));
    }
  });

  it("different seeds diverge", () => {
    const a = Sim.newGame(catalog, 1);
    const b = Sim.newGame(catalog, 2);
    expect(hashState(a.state)).not.toBe(hashState(b.state));
  });

  it("save -> load -> continue equals never having saved", () => {
    const straight = Sim.newGame(catalog, 4321);
    run(straight, 0, 900);

    const first = Sim.newGame(catalog, 4321);
    run(first, 0, 400);
    const text = encodeSave(cloneState(first.state), { zone: "county", day: 0, hour: 17, hp: 1, maxhp: 1 }, new Date(0));
    const decoded = decodeSave(text);
    expect(decoded.ok).toBe(true);
    if (!decoded.ok) return;
    const resumed = Sim.fromState(catalog, decoded.file.state);
    run(resumed, 400, 900);
    expect(hashState(resumed.state)).toBe(hashState(straight.state));
  });

  it("rejects garbage and newer saves without throwing", () => {
    expect(decodeSave("{not json").ok).toBe(false);
    expect(decodeSave('{"format":"jane-save","version":999,"state":{}}').ok).toBe(false);
    // Saves written before the rename still open.
    const renamed = JSON.parse(encodeSave(cloneState(Sim.newGame(catalog, 3).state), { zone: "", day: 0, hour: 0, hp: 0, maxhp: 0 }, new Date(0)));
    renamed.format = "junqi-save";
    expect(decodeSave(JSON.stringify(renamed)).ok).toBe(true);
    expect(decodeSave('{"hello":1}').ok).toBe(false);
  });
});

describe("cast pipeline", () => {
  it("queues damage and only changes hp at the flush", () => {
    const sim = Sim.newGame(catalog, 9);
    const p = sim.player;
    const foe = createUnit(sim.state, catalog, "skeleton", "t_foe", p.x + 12, p.y);
    addUnit(sim, foe);
    const before = foe.hp;
    sim.setAim(1, 0);
    expect(tryCast(sim, p, "melee_player", { x: 1, y: 0 })).toBe("castSuccessful");
    expect(foe.hp).toBe(before);
    expect(foe.incoming.length).toBe(1);
    sim.tick(NO_INPUT);
    expect(foe.hp).toBeLessThan(before);
    expect(foe.incoming.length).toBe(0);
  });

  it("melee is forgiving: aimed the wrong way, it still finds the enemy in reach", () => {
    const sim = Sim.newGame(catalog, 9);
    const p = sim.player;
    const foe = createUnit(sim.state, catalog, "skeleton", "t_behind", p.x - 12, p.y);
    addUnit(sim, foe);
    expect(tryCast(sim, p, "melee_player", { x: 1, y: 0 })).toBe("castSuccessful"); // aiming east, foe is west
    expect(foe.incoming.length).toBe(1);
    expect(p.facing).toBe(2); // and she turns to face what she hit
  });

  it("checks range before cooldown, so a cooling-down AI keeps walking", () => {
    const sim = Sim.newGame(catalog, 9);
    const p = sim.player;
    const foe = createUnit(sim.state, catalog, "skeleton", "t_far", p.x + 60, p.y);
    addUnit(sim, foe);
    foe.target = p.id;
    foe.cooldowns.melee = 100;
    expect(tryCast(sim, foe, "melee")).toBe("tooFar");
  });

  it("survives a target that no longer exists", () => {
    // 2020 read current_target.faction before instance_exists(current_target).
    const sim = Sim.newGame(catalog, 9);
    const foe = createUnit(sim.state, catalog, "skeleton", "t_ghost", sim.player.x + 10, sim.player.y);
    addUnit(sim, foe);
    foe.target = 987654;
    expect(tryCast(sim, foe, "melee")).toBe("noTarget");
  });

  it("charges nothing for a failed cast and starts the GCD on success", () => {
    const sim = Sim.newGame(catalog, 9);
    const p = sim.player;
    p.book.push("icebolt", "repair");
    const mp = p.mp;
    expect(tryCast(sim, p, "repair")).toBe("castUnsuccessful");
    expect(p.gcd).toBe(0);
    expect(tryCast(sim, p, "icebolt", { x: 1, y: 0 })).toBe("castSuccessful");
    expect(p.mp).toBe(mp - 14);
    expect(p.gcd).toBe(GCD_TICKS);
    expect(tryCast(sim, p, "icebolt", { x: 1, y: 0 })).toBe("onCooldown");
    expect(sim.zone.projectiles.length).toBe(1);
  });

  it("enemies chase a player who is standing still, and hit them", () => {
    const sim = Sim.newGame(catalog, 9);
    const p = sim.player;
    const free = sim.rt.grid.nearestFree(Math.floor(p.x / 8) + 9, Math.floor(p.y / 8), 6)!;
    const foe = createUnit(sim.state, catalog, "skeleton", "t_chaser", centre(free.cx), centre(free.cy));
    addUnit(sim, foe);
    const hp = p.hp;
    idle(sim, 60 * 12);
    expect(foe.combat).toBe("combat");
    expect(p.hp).toBeLessThan(hp);
  });
});

describe("bags and crafting", () => {
  it("never loses a reward when the bag is full", () => {
    const sim = Sim.newGame(catalog, 3);
    const p = sim.player;
    for (let i = 0; i < 40; i++) bagAdd(sim, p, "gold_bar", 16);
    expect(p.bag!.every((s) => s !== null)).toBe(true);
    const drops = sim.zone.drops.length;
    sim.command({ t: "dev", dev: { op: "quest", quest: "defeat_skeleton" } });
    const q = sim.state.quests.active.find((x) => x.quest === "defeat_skeleton")!;
    q.counts[0] = 1;
    sim.command({ t: "dev", dev: { op: "flag", flag: "x", value: 1 } });
    // hand in through the same action list the dog uses
    const lastLine = catalog.dialogue.dog.nodes.reward.lines.length - 1;
    sim.me.dialogue = { tree: "dog", node: "reward", line: lastLine, speaker: 0 };
    sim.command({ t: "choose", option: 0 });
    expect(questDone(sim, "defeat_skeleton")).toBe(true);
    expect(sim.zone.drops.length).toBe(drops + 1);
    expect(sim.zone.drops[sim.zone.drops.length - 1].item).toBe("key_auntie_house");
  });

  it("crafts by sorted ids and only consumes inputs when the output fits", () => {
    const sim = Sim.newGame(catalog, 3);
    const p = sim.player;
    bagAdd(sim, p, "pansy", 1);
    bagAdd(sim, p, "small_water", 1);
    bagAdd(sim, p, "gold_dust", 1);
    const slotOf = (item: string): number => p.bag!.findIndex((s) => s?.item === item);
    sim.command({ t: "craftPut", bag: slotOf("pansy"), slot: 0 });
    sim.command({ t: "craftPut", bag: slotOf("small_water"), slot: 1 });
    sim.command({ t: "craftPut", bag: slotOf("gold_dust"), slot: 2 });
    expect(craftOutput(sim)).toEqual({ item: "potion_manashield", qty: 1 });
    sim.command({ t: "craftTake" });
    expect(bagCount(p, "potion_manashield")).toBe(1);
    expect(bagCount(p, "pansy")).toBe(0);
    expect(sim.me.craft.every((s) => s === null)).toBe(true);
  });
});

describe("the first five minutes, played headless", () => {
  it("letter -> dog -> skeleton -> key -> kitchen -> Icebolt", () => {
    const sim = Sim.newGame(catalog, 20260920);
    const p = sim.player;
    expect(sim.hour).toBe(17);
    expect(questDone(sim, "the_letter")).toBe(false);

    // Walk up to the dog. Reaching the stoop completes the letter.
    walkToUnit(sim, "dog");
    expect(questDone(sim, "the_letter")).toBe(true);

    sim.command({ t: "use" });
    expect(sim.me.dialogue?.tree).toBe("dog");
    talkThrough(sim, [0]);
    expect(sim.state.quests.active.some((q) => q.quest === "defeat_skeleton")).toBe(true);

    // The door is locked until the dog says so.
    expect(walkToProp(sim, "house_door")).toBe(true);
    sim.command({ t: "use" });
    expect(sim.me.zone).toBe("county");

    const skeleton = sim.rt.unitsByKey.get("yard_skeleton")!;
    walkTo(sim, skeleton.x, skeleton.y, 20);
    expect(fight(sim, skeleton)).toBe(true);
    expect(p.alive).toBe(true);

    walkToUnit(sim, "dog");
    sim.command({ t: "use" });
    talkThrough(sim, [0]);
    expect(questDone(sim, "defeat_skeleton")).toBe(true);
    expect(bagCount(p, "key_auntie_house")).toBe(1);

    expect(walkToProp(sim, "house_door")).toBe(true);
    sim.command({ t: "use" }); // unlock
    sim.command({ t: "use" }); // enter
    idle(sim, 2);
    expect(sim.me.zone).toBe("house");

    expect(walkToProp(sim, "ice_orb")).toBe(true);
    expect(questDone(sim, "see_the_kitchen")).toBe(true);
    sim.command({ t: "use" });
    talkThrough(sim);
    expect(p.book).toContain("icebolt");
    expect(sim.me.bar.some((s) => s?.id === "icebolt")).toBe(true);

    // And back out the front door to where we came in.
    expect(walkToProp(sim, "front_door")).toBe(true);
    sim.command({ t: "use" });
    idle(sim, 2);
    expect(sim.me.zone).toBe("county");
    expect(p.hp).toBeLessThanOrEqual(maxHp(p));
  });
});
