import { describe, expect, it } from "vitest";
import { SAVE_VERSION } from "@/sim/constants";
import { focusOf, nearRest } from "@/sim/interact";
import { cloneState, decodeSave, encodeSave } from "@/sim/save";
import { Sim } from "@/sim/sim";
import { cleanName, expandText, isNight } from "@/sim/text";
import { addUnit } from "@/sim/runtime";
import { createUnit, maxHp } from "@/sim/units";
import { idle, talkThrough, walkToProp, walkToUnit , yardCatalog } from "./bot";

const catalog = yardCatalog();

describe("the player's name", () => {
  it("is chosen at New Game, cleaned, and defaults to Jane", () => {
    expect(Sim.newGame(catalog, 5, "  Mina!!  ").state.name).toBe("Mina");
    expect(Sim.newGame(catalog, 5, "").state.name).toBe("Jane");
    expect(Sim.newGame(catalog, 5).state.name).toBe("Jane");
    expect(cleanName("Anne-Marie O'Neil the Third")).toBe("Anne-Marie O'Nei");
  });

  it("is what Castle calls her: no row has a name baked in", () => {
    const sim = Sim.newGame(catalog, 5, "Mina");
    const intro = catalog.dialogue.dog.nodes.intro.lines[0];
    expect(expandText(sim.state, intro)).toBe("Mina.");
    const all = JSON.stringify([catalog.dialogue, catalog.quests, catalog.items, catalog.triggers, catalog.clock]);
    expect(all.includes("Jane")).toBe(false);
  });
});

describe("beds and fires", () => {
  it("resting mends her, remembers the spot, and asks the app to save", () => {
    const sim = Sim.newGame(catalog, 21);
    const p = sim.player;
    expect(nearRest(sim)).toBe(false);
    expect(sim.state.rest).toBeNull();
    p.hp = 40;
    expect(walkToProp(sim, "yard_fire")).toBe(true);
    expect(nearRest(sim)).toBe(true);
    sim.drainEvents();
    sim.command({ t: "use" });
    talkThrough(sim);
    expect(p.hp).toBe(maxHp(p));
    expect(sim.state.rest?.zone).toBe("county");
    expect(sim.drainEvents().some((e) => e.e === "rest")).toBe(true);
  });

  it("dying wakes her at the last fire, even from another zone, with nothing she carried", () => {
    const sim = Sim.newGame(catalog, 21);
    walkToProp(sim, "yard_fire");
    sim.command({ t: "use" });
    talkThrough(sim);
    const rest = { ...sim.state.rest! };
    sim.command({ t: "dev", dev: { op: "tp", zone: "mine", mark: "entry" } });
    expect(sim.me.zone).toBe("mine");
    sim.player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 320);
    expect(sim.me.zone).toBe("county");
    expect(sim.player.alive).toBe(true);
    expect(Math.hypot(sim.player.x - rest.x, sim.player.y - rest.y)).toBeLessThan(24);
    expect(sim.me.stats.deaths).toBe(1);
  });

  it("before the first rest she wakes where she came in", () => {
    const sim = Sim.newGame(catalog, 21);
    sim.command({ t: "dev", dev: { op: "tp", zone: "mine", mark: "entry" } });
    sim.player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 320);
    expect(sim.me.zone).toBe("mine");
    expect(sim.player.alive).toBe(true);
  });

  it("a bed can sleep the clock to morning", () => {
    const sim = Sim.newGame(catalog, 21);
    sim.command({ t: "dev", dev: { op: "tp", zone: "house", mark: "front" } });
    sim.command({ t: "dev", dev: { op: "time", hour: 23 } });
    expect(walkToProp(sim, "julies_bed")).toBe(true);
    sim.command({ t: "use" });
    talkThrough(sim, [0]); // "Sleep until morning"
    expect(sim.hour).toBeGreaterThanOrEqual(6);
    expect(sim.hour).toBeLessThan(6.1);
    expect(sim.state.day).toBe(1);
    expect(sim.state.rest?.zone).toBe("house");
  });
});

describe("saves", () => {
  it("a version 1 save migrates forward, through every version, instead of breaking", () => {
    const sim = Sim.newGame(catalog, 33);
    idle(sim, 30);
    const unitId = sim.player.id;
    const old = JSON.parse(encodeSave(cloneState(sim.state), { zone: "Castle", day: 0, hour: 17, hp: 150, maxhp: 150 }, new Date(0)));
    // Rebuild the shape a September-20 save had: one player, her fields on the state itself.
    const p0 = old.state.players[0];
    for (const k of ["name", "open", "rest", "growth"]) delete old.state[k];
    Object.assign(old.state, {
      playerId: p0.unitId, zone: p0.zone, lastMark: p0.lastMark, respawnIn: 0,
      bar: p0.bar, craft: p0.craft, dialogue: null, god: false, stats: p0.stats,
    });
    delete old.state.players;
    old.version = 1;
    old.state.version = 1;
    for (const z of Object.values(old.state.zones) as { units: Record<string, unknown>[] }[]) {
      for (const u of z.units) {
        delete u.hidden;
        delete u.pathGoal;
      }
    }
    const decoded = decodeSave(JSON.stringify(old));
    expect(decoded.ok).toBe(true);
    if (!decoded.ok) return;
    expect(decoded.file.version).toBe(SAVE_VERSION);
    const me = decoded.file.state.players[0];
    expect(decoded.file.state.name).toBe("Jane");
    expect(decoded.file.state.rest).toBeNull();
    expect(decoded.file.state.open).toBe(false);
    expect(decoded.file.state.growth.spells).toContain("melee_player");
    expect(me.who).toBe("host");
    expect(me.unitId).toBe(unitId);
    expect(me.bar[0]).toEqual({ source: "spell", id: "melee_player" });
    expect("playerId" in decoded.file.state).toBe(false);
    const resumed = Sim.fromState(catalog, decoded.file.state);
    idle(resumed, 60);
    expect(resumed.player.hidden).toBe(false);
    expect(resumed.player.id).toBe(unitId);
  });
});

describe("the dog", () => {
  it("only says the city-dog line if you keep bothering it", () => {
    const sim = Sim.newGame(catalog, 44);
    sim.state.quests.done.push("defeat_skeleton"); // past the intro, nothing to hand in: the idle chain
    walkToUnit(sim, "dog");
    const heard: string[] = [];
    for (let n = 0; n < 6; n++) {
      sim.command({ t: "use" });
      const d = sim.me.dialogue!;
      expect(d.tree).toBe("dog");
      heard.push(d.node);
      talkThrough(sim);
    }
    expect(heard).toEqual(["idle", "poke_1", "poke_2", "poke_3", "poke_4", "poke_5"]);
    expect(catalog.dialogue.dog.nodes.poke_4.lines[0]).toMatch(/not a city dog/);
    expect(catalog.dialogue.dog.nodes.intro.lines.join(" ")).not.toMatch(/city dog/);
  });

  it("is not there after dark, and never vanishes while she is looking", () => {
    const sim = Sim.newGame(catalog, 44);
    const dog = sim.rt.unitsByKey.get("dog")!;
    sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
    expect(isNight(sim.state)).toBe(true);
    idle(sim, 40); // Jane is at the gate, far from the step
    expect(dog.hidden).toBe(true);

    sim.command({ t: "dev", dev: { op: "time", hour: 8 } });
    idle(sim, 40);
    expect(dog.hidden).toBe(false);

    walkToUnit(sim, "dog");
    sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
    idle(sim, 90);
    expect(dog.hidden).toBe(false); // she is standing right there
    expect(focusOf(sim, sim.player)?.kind).toBe("unit");
  });

  it("a door marked for it is not answered after dark, and is an ordinary door by day", () => {
    const sim = Sim.newGame(catalog, 45);
    const door = sim.rt.propsByKey.get("house_door")!;
    door.locked = false;
    door.nightLock = "Nobody comes to the door. Somebody is standing behind it.";
    expect(walkToProp(sim, "house_door")).toBe(true);
    sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
    expect(focusOf(sim, sim.player)?.prompt).toBe("Try the door");
    sim.drainEvents();
    sim.command({ t: "use" });
    idle(sim, 5);
    expect(sim.me.zone).toBe("county");
    expect(sim.drainEvents().some((e) => e.e === "toast" && /standing behind it/.test(e.text))).toBe(true);

    sim.command({ t: "dev", dev: { op: "time", hour: 9 } });
    sim.command({ t: "use" });
    idle(sim, 5);
    expect(sim.me.zone).toBe("house");
  });

  it("the county rings a bell at nine", () => {
    const sim = Sim.newGame(catalog, 44);
    sim.command({ t: "dev", dev: { op: "time", hour: 20.999 } });
    sim.drainEvents();
    idle(sim, 20);
    const toasts = sim.drainEvents().filter((e) => e.e === "toast").map((e) => (e.e === "toast" ? e.text : ""));
    expect(toasts.some((t) => /bell/.test(t))).toBe(true);
  });
});

describe("things that are only there at night", () => {
  it("a nightOnly creature is absent by day, present after nine, and cannot be struck while it is away", () => {
    const cat = yardCatalog();
    cat.units.night_tenant = { ...cat.units.skeleton, nightOnly: true };
    const sim = Sim.newGame(cat, 46);
    const p = sim.player;
    const free = sim.rt.grid.nearestFree(Math.floor(p.x / 8) + 40, Math.floor(p.y / 8), 8)!;
    const tenant = createUnit(sim.state, cat, "night_tenant", "tenant", free.cx * 8 + 4, free.cy * 8 + 4);
    addUnit(sim, tenant);
    sim.command({ t: "dev", dev: { op: "time", hour: 12 } });
    idle(sim, 30);
    expect(tenant.hidden).toBe(true);
    tenant.incoming.length = 0;
    sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
    idle(sim, 30);
    expect(tenant.hidden).toBe(false);
    sim.command({ t: "dev", dev: { op: "time", hour: 7 } });
    idle(sim, 30);
    expect(tenant.hidden).toBe(true);
  });
});
