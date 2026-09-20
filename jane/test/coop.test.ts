// Co-op readiness, proven without a network: several seats in one sim.
// tick(inputs[]) + command(seat, c) is the whole surface a lockstep layer needs.

import { describe, expect, it } from "vitest";
import { seatSheets, SEAT_COATS, UNIT_SPRITES } from "@/art/units";
import { normalize } from "@/sim/angles";
import { buildCatalog, type SpellDef } from "@/sim/catalog";
import { PARTY_DEALT, PARTY_TAKEN } from "@/sim/constants";
import { cellOf, centre } from "@/sim/grid";
import { bagCount } from "@/sim/inventory";
import { costOfCells } from "@/sim/path";
import { questDone } from "@/sim/quests";
import { playReplay, Recorder } from "@/sim/replay";
import { addUnit } from "@/sim/runtime";
import { cloneState, decodeSave, encodeSave, hashState } from "@/sim/save";
import { NO_INPUT, Sim, type Command, type InputFrame } from "@/sim/sim";
import { expandText } from "@/sim/text";
import type { Unit } from "@/sim/state";
import { createUnit, maxHp, placeUnit } from "@/sim/units";
import { talkThrough as talkSolo, walkToProp } from "./bot";

const catalog = buildCatalog();

/** `who` are client tokens, not names: everyone plays the one heroine. The host opens the world first. */
function party(seed: number, who: string[], cat = catalog): Sim {
  const sim = Sim.newGame(cat, seed);
  sim.command(0, { t: "open", on: true });
  for (const w of who.slice(1)) sim.command(-1, { t: "join", who: w });
  sim.drainEvents();
  return sim;
}

function idle(sim: Sim, ticks: number): void {
  for (let i = 0; i < ticks; i++) sim.tick([]);
}

/** Walk one seat to a point in her own zone while everyone else stands still. */
function walkSeat(sim: Sim, seat: number, x: number, y: number, near = 8, maxTicks = 60 * 60): boolean {
  const view = sim.view(seat);
  let path: number[] | null = null;
  let at = 0;
  for (let t = 0; t < maxTicks; t++) {
    const p = view.player;
    if (Math.hypot(x - p.x, y - p.y) <= near) return true;
    if (!path || at >= path.length) {
      const goal = view.rt.grid.nearestFree(cellOf(x), cellOf(y), 6, p.id);
      if (!goal) return false;
      path = view.rt.path.find(cellOf(p.x), cellOf(p.y), goal.cx, goal.cy, p.id, costOfCells(4000), 400000);
      at = 0;
      if (!path) return false;
      if (path.length === 0) return true;
    }
    const cell = path[at];
    const tx = centre(cell % view.rt.grid.w);
    const ty = centre(Math.floor(cell / view.rt.grid.w));
    const d = Math.hypot(tx - p.x, ty - p.y);
    if (d < 1.5) {
      at++;
      continue;
    }
    const inputs: InputFrame[] = [NO_INPUT, NO_INPUT, NO_INPUT, NO_INPUT];
    inputs[seat] = { mx: (tx - p.x) / d, my: (ty - p.y) / d, sprint: false, useHeld: false, ax: 0, ay: 0 };
    sim.tick(inputs);
  }
  return false;
}

function talkThrough(sim: Sim, seat: number, choices: number[] = []): void {
  let n = 0;
  for (let guard = 0; guard < 200 && sim.state.players[seat].dialogue; guard++) {
    const d = sim.state.players[seat].dialogue!;
    const node = catalog.dialogue[d.tree].nodes[d.node];
    const last = d.line >= node.lines.length - 1;
    if (last && (node.options?.length ?? 0) > 0) sim.command(seat, { t: "choose", option: choices[n++] ?? 0 });
    else sim.command(seat, { t: "advance" });
  }
}

describe("seats", () => {
  it("up to four sit down, each with her own body, bags and bar", () => {
    const sim = party(7, ["host", "b", "c", "d"]);
    expect(sim.party.size()).toBe(4);
    expect(sim.state.players.map((p) => p.who)).toEqual(["host", "b", "c", "d"]);
    const ids = new Set(sim.state.players.map((p) => p.unitId));
    expect(ids.size).toBe(4);
    for (let seat = 0; seat < 4; seat++) {
      const v = sim.view(seat);
      expect(bagCount(v.player, "apple")).toBe(3);
      expect(v.me.bar[0]).toEqual({ source: "spell", id: "melee_player" });
    }
    // Nobody spawned on top of anybody.
    const cells = new Set(sim.party.units().map((u) => `${cellOf(u.x)},${cellOf(u.y)}`));
    expect(cells.size).toBe(4);
    expect(sim.command(-1, { t: "join", who: "fifth" })).toBe(-1);
    // One story: the starting quest was given once, not four times.
    expect(sim.state.quests.active.filter((q) => q.quest === "the_letter").length).toBe(1);
  });

  it("leaving parks her body; the same client gets it back with everything she owned", () => {
    const sim = party(7, ["Jane", "Mina"]);
    sim.command(1, { t: "dev", dev: { op: "give", item: "gold_bar", qty: 5 } });
    const body = sim.view(1).player.id;
    sim.command(1, { t: "leave" });
    expect(sim.party.size()).toBe(1);
    expect(sim.rt.units.has(body)).toBe(false);
    idle(sim, 30);
    expect(sim.command(-1, { t: "join", who: "Mina" })).toBe(1);
    expect(sim.view(1).player.id).toBe(body);
    expect(bagCount(sim.view(1).player, "gold_bar")).toBe(5);
    expect(sim.state.players.length).toBe(2);
  });
});

describe("the co-op penalty", () => {
  it("weakens everyone by head count, server wide, and lifts when they leave", () => {
    for (const n of [1, 2, 3, 4]) {
      const sim = party(9, ["A", "B", "C", "D"].slice(0, n));
      // Send everyone but seat 0 somewhere else entirely: the penalty does not care where they stand.
      for (let seat = 1; seat < n; seat++) sim.command(seat, { t: "dev", dev: { op: "tp", zone: "cellar", mark: "stair_a" } });
      const me = sim.player;
      const foe = createUnit(sim.state, catalog, "iron_knuckles", `dummy_${n}`, me.x + 40, me.y);
      addUnit(sim, foe);
      const foeHp = foe.hp;
      const myHp = me.hp;
      foe.incoming.push({ amount: 100, school: "frost", from: me.id, crit: false });
      me.incoming.push({ amount: 40, school: "physical", from: 0, crit: false });
      sim.tick([]);
      expect(foeHp - foe.hp, `${n} players: damage dealt`).toBe(Math.round(100 * PARTY_DEALT[n - 1]));
      expect(myHp - me.hp, `${n} players: damage taken`).toBe(Math.round(40 * PARTY_TAKEN[n - 1]));
    }
    const sim = party(9, ["A", "B"]);
    sim.command(1, { t: "leave" });
    const foe = createUnit(sim.state, catalog, "iron_knuckles", "dummy", sim.player.x + 40, sim.player.y);
    addUnit(sim, foe);
    const hp = foe.hp;
    foe.incoming.push({ amount: 100, school: "frost", from: sim.player.id, crit: false });
    sim.tick([]);
    expect(hp - foe.hp).toBe(100);
  });

  it("together is a little better than alone; alone in a party is much worse", () => {
    for (let n = 2; n <= 4; n++) {
      expect(PARTY_DEALT[n - 1] * n).toBeGreaterThan(1);
      expect(PARTY_DEALT[n - 1]).toBeLessThan(PARTY_DEALT[n - 2]);
      expect(PARTY_TAKEN[n - 1]).toBeGreaterThan(PARTY_TAKEN[n - 2]);
    }
  });
});

describe("one story, several people", () => {
  it("a friend's kill counts, and everyone is paid at the hand-in", () => {
    const sim = party(11, ["Jane", "Mina"]);
    const dog = sim.rt.unitsByKey.get("dog")!;
    expect(walkSeat(sim, 0, dog.x, dog.y, 14)).toBe(true);
    sim.command(0, { t: "use" });
    talkThrough(sim, 0, [0]);
    expect(sim.state.quests.active.some((q) => q.quest === "defeat_skeleton")).toBe(true);

    // Mina does the killing.
    const skeleton = sim.rt.unitsByKey.get("yard_skeleton")!;
    skeleton.incoming.push({ amount: 99999, school: "physical", from: sim.view(1).player.id, crit: false });
    idle(sim, 2);
    expect(skeleton.alive).toBe(false);
    expect(sim.state.players[1].stats.kills).toBe(1);

    // Jane does the talking.
    sim.command(0, { t: "use" });
    talkThrough(sim, 0, [0]);
    expect(questDone(sim, "defeat_skeleton")).toBe(true);
    expect(bagCount(sim.view(0).player, "key_auntie_house")).toBe(1);
    expect(bagCount(sim.view(1).player, "key_auntie_house")).toBe(1);
  });

  it("nothing pauses with company: one reads, the world and her friend carry on", () => {
    const sim = party(12, ["Jane", "Mina"]);
    const dog = sim.rt.unitsByKey.get("dog")!;
    walkSeat(sim, 0, dog.x, dog.y, 14);
    sim.command(0, { t: "use" });
    expect(sim.state.players[0].dialogue).not.toBeNull();
    expect(sim.frozen).toBe(false);
    const tick = sim.state.tick;
    const janeX = sim.view(0).player.x;
    const minaX = sim.view(1).player.x;
    const push: InputFrame = { mx: 1, my: 0, sprint: false, useHeld: false, ax: 0, ay: 0 };
    for (let i = 0; i < 30; i++) sim.tick([push, push]);
    expect(sim.state.tick).toBe(tick + 30);
    expect(sim.view(0).player.x).toBe(janeX); // talking: she stands still
    expect(sim.view(1).player.x).toBeGreaterThan(minaX);
    // Alone, it does pause, as it did in 2020.
    const solo = Sim.newGame(catalog, 12);
    solo.me.dialogue = { tree: "dog", node: "idle", line: 0, speaker: 0 };
    expect(solo.frozen).toBe(true);
  });

  it("what is hers goes to her; quest news goes to everyone", () => {
    const sim = party(13, ["Jane", "Mina"]);
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "house", mark: "front" } });
    sim.drainEvents();
    sim.command(0, { t: "item", item: "apple" }); // full health: "I'm not hurt", to Jane only
    sim.command(0, { t: "dev", dev: { op: "quest", quest: "the_mine" } });
    const events = sim.drainEvents();
    const jane = Sim.eventsFor(events, sim.state.players[0]).filter((e) => e.e === "toast").map((e) => (e.e === "toast" ? e.text : ""));
    const mina = Sim.eventsFor(events, sim.state.players[1]).filter((e) => e.e === "toast").map((e) => (e.e === "toast" ? e.text : ""));
    expect(jane).toContain("I'm not hurt");
    expect(mina).not.toContain("I'm not hurt");
    expect(jane.some((t) => t.startsWith("Quest:"))).toBe(true);
    expect(mina.some((t) => t.startsWith("Quest:"))).toBe(true);
  });
});

describe("splitting up", () => {
  it("every zone with someone in it ticks; an empty one stops", () => {
    const sim = party(14, ["Jane", "Mina"]);
    sim.command(0, { t: "dev", dev: { op: "tp", zone: "mine", mark: "entry" } });
    expect(sim.state.players.map((p) => p.zone)).toEqual(["mine", "county"]);

    // Something hunts Mina in the county while Jane walks in the mine.
    const mina = sim.view(1).player;
    const free = sim.view(1).rt.grid.nearestFree(cellOf(mina.x) + 8, cellOf(mina.y), 6)!;
    const hunter = createUnit(sim.state, catalog, "skeleton", "hunter", centre(free.cx), centre(free.cy));
    addUnit(sim.view(1), hunter);
    const minaHp = mina.hp;
    const janeY = sim.view(0).player.y;
    const up: InputFrame = { mx: 0, my: -1, sprint: false, useHeld: false, ax: 0, ay: 0 };
    for (let i = 0; i < 60 * 10; i++) sim.tick([up, NO_INPUT]);
    expect(sim.view(0).player.y).toBeLessThan(janeY); // the mine ticked
    expect(hunter.combat).toBe("combat"); // and so did the county
    expect(mina.hp).toBeLessThan(minaHp);

    // Mina follows Jane down. Nobody is left in the county, so it stops (and keeps its state).
    const before = hunter.animTick;
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "mine", mark: "entry" } });
    idle(sim, 30);
    expect(sim.state.players.map((p) => p.zone)).toEqual(["mine", "mine"]);
    expect(hunter.animTick).toBe(before);
    expect(sim.state.zones.county!.units.includes(hunter)).toBe(true);
  });

  it("a lock-in holds while a friend is still alive inside, and lets go when the last one falls", () => {
    const sim = party(15, ["Jane", "Mina"]);
    for (const seat of [0, 1]) {
      sim.command(seat, { t: "dev", dev: { op: "tp", zone: "burial", mark: "lockin_a" } });
      sim.state.players[seat].lastMark = "entry"; // as if they had walked in the front way
    }
    idle(sim, 3);
    const gate = sim.rt.propsByKey.get("gate_snake")!;
    expect(gate.locked).toBe(true);
    sim.command(1, { t: "dev", dev: { op: "god", on: true } });

    sim.view(0).player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 300);
    expect(sim.view(0).player.alive).toBe(true);
    expect(gate.locked).toBe(true); // Mina is still in there

    sim.command(1, { t: "dev", dev: { op: "god", on: false } });
    sim.view(1).player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 300);
    expect(gate.locked).toBe(false);
    expect(sim.rt.unitsByKey.has("guard_a")).toBe(false);
  });

  it("the night only passes when the whole party is resting", () => {
    const sim = party(16, ["Jane", "Mina"]);
    for (const seat of [0, 1]) sim.command(seat, { t: "dev", dev: { op: "tp", zone: "house", mark: "front" } });
    sim.command(0, { t: "dev", dev: { op: "time", hour: 23 } });
    const bed = sim.rt.propsByKey.get("julies_bed")!;
    const bx = bed.cx * 8 - 6;
    const by = bed.cy * 8 + 12;
    expect(walkSeat(sim, 0, bx, by, 6)).toBe(true);
    sim.command(0, { t: "use" });
    talkThrough(sim, 0, [0]);
    expect(sim.hour).toBeGreaterThan(22); // Mina is across the room: no
    expect(sim.state.rest?.zone).toBe("house"); // but Jane did rest

    expect(walkSeat(sim, 1, bx, by + 10, 8)).toBe(true);
    sim.command(0, { t: "use" });
    talkThrough(sim, 0, [0]);
    expect(sim.hour).toBeLessThan(6.2);
    expect(sim.state.day).toBe(1);
  });
});

describe("the wire format", () => {
  it("a session with two seats, a join, commands and a zone split replays to the same hash", () => {
    const seed = 2026;
    const sim = Sim.newGame(catalog, seed, "Jane");
    const rec = new Recorder(seed, "Jane");
    const send = (seat: number, c: Command): void => {
      rec.command(seat, c);
      sim.command(seat, c);
    };
    const q = (v: number): number => Math.round(v * 127) / 127;
    for (let t = 0; t < 1200; t++) {
      if (t === 90) send(0, { t: "open", on: true });
      if (t === 100) send(-1, { t: "join", who: "Mina" });
      if (t === 400) send(1, { t: "dev", dev: { op: "tp", zone: "cellar", mark: "stair_a" } });
      if (t === 700) send(0, { t: "dev", dev: { op: "give", item: "apple", qty: 2 } });
      if (t % 45 === 0) send(0, { t: "bar", slot: 0 });
      if (t % 50 === 0 && sim.party.size() > 1) send(1, { t: "bar", slot: 0 });
      if (t === 900) send(1, { t: "leave" });
      for (const p of sim.state.players) if (p.connected && p.dialogue) send(p.index, { t: "closeDialogue" });
      const a = Math.floor(t / 30) * 0.9;
      const inputs: InputFrame[] = [
        { mx: q(Math.cos(a)), my: q(Math.sin(a)), ax: q(Math.sin(a)), ay: q(Math.cos(a)), sprint: t % 200 < 60, useHeld: false },
        { mx: q(-Math.sin(a)), my: q(Math.cos(a)), ax: 0, ay: 0, sprint: false, useHeld: t % 90 < 10 },
      ];
      if (sim.frozen) continue;
      rec.frame(inputs);
      sim.tick(inputs);
    }
    const replay = rec.finish(sim);
    const again = playReplay(catalog, JSON.parse(JSON.stringify(replay)));
    expect(again.state.tick).toBe(sim.state.tick);
    expect(again.state.players.length).toBe(2);
    expect(hashState(again.state)).toBe(replay.finalHash);
    expect(maxHp(again.player)).toBe(maxHp(sim.player));
  });
});

describe("one heroine", () => {
  it("the world is closed until the host opens it, only the host can, and a save loads closed", () => {
    const sim = Sim.newGame(catalog, 21);
    expect(sim.command(-1, { t: "join", who: "b" })).toBe(-1);
    sim.command(0, { t: "open", on: true });
    expect(sim.command(-1, { t: "join", who: "b" })).toBe(1);
    expect(sim.command(1, { t: "open", on: false })).toBe(-1); // not hers to close
    expect(sim.state.open).toBe(true);

    // Closing sends nobody home. It stops anyone new.
    sim.command(0, { t: "open", on: false });
    expect(sim.party.size()).toBe(2);
    expect(sim.command(-1, { t: "join", who: "c" })).toBe(-1);

    // The penalty follows the people, not the door: she leaves, and it is a single-player game again.
    sim.command(1, { t: "leave" });
    expect(sim.party.size()).toBe(1);

    sim.command(0, { t: "open", on: true });
    const text = encodeSave(cloneState(sim.state), { zone: "", day: 0, hour: 0, hp: 0, maxhp: 0 }, new Date(0));
    const file = decodeSave(text);
    expect(file.ok).toBe(true);
    if (file.ok) expect(Sim.fromState(catalog, file.file.state).state.open).toBe(false);
  });

  it("everyone plays her under the host's name; nobody has a name of her own", () => {
    const sim = Sim.newGame(catalog, 22, "Mina");
    sim.command(0, { t: "open", on: true });
    sim.command(-1, { t: "join", who: "b" });
    expect(sim.state.name).toBe("Mina");
    expect(expandText(sim.state, "{name}.")).toBe("Mina.");
    for (const p of sim.state.players) expect("name" in p).toBe(false);
  });

  it("is told apart by the coat alone: same drawing, one palette per seat", () => {
    const sheets = seatSheets();
    const base = UNIT_SPRITES.jane;
    expect(Object.keys(sheets)).toEqual(["jane@1", "jane@2", "jane@3"]);
    for (const [seat, id] of Object.keys(sheets).entries()) {
      expect(sheets[id].frames).toBe(base.frames);
      const changed = Object.keys(base.palette).filter((ch) => sheets[id].palette[ch] !== base.palette[ch]);
      expect(changed.sort()).toEqual(["C", "c", "q"]);
      expect(sheets[id].palette.c).toBe(SEAT_COATS[seat + 1].c);
    }
    expect(new Set(SEAT_COATS.map((c) => c.c)).size).toBe(4);
    expect(SEAT_COATS[0].c).toBe(base.palette.c);
  });
});

describe("one fire", () => {
  it("whoever sits down arrives at the party's last fire, and whoever dies wakes there", () => {
    const sim = Sim.newGame(catalog, 23);
    expect(walkToProp(sim, "yard_fire")).toBe(true);
    sim.command({ t: "use" });
    talkSolo(sim);
    const fire = { ...sim.state.rest! };
    expect(fire.zone).toBe("county");

    sim.command(0, { t: "open", on: true });
    expect(sim.command(-1, { t: "join", who: "b" })).toBe(1);
    const guest = sim.view(1).player;
    expect(sim.state.players[1].zone).toBe("county");
    expect(Math.hypot(guest.x - fire.x, guest.y - fire.y)).toBeLessThan(32);

    // The guest moves the fire for everyone: she rests on the bed in the house.
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "house", mark: "front" } });
    const bed = sim.view(1).rt.propsByKey.get("julies_bed")!;
    expect(walkSeat(sim, 1, bed.cx * 8 - 6, bed.cy * 8 + 12, 6)).toBe(true);
    sim.drainEvents();
    sim.command(1, { t: "use" });
    talkThrough(sim, 1, [1]); // "Rest a moment"
    expect(sim.state.rest?.zone).toBe("house");
    // The host is in another zone and still hears it: a guest resting saves the host's world.
    expect(Sim.eventsFor(sim.drainEvents(), sim.state.players[0]).some((e) => e.e === "rest")).toBe(true);

    sim.view(0).player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 320);
    expect(sim.state.players[0].zone).toBe("house");
    expect(sim.view(0).player.alive).toBe(true);

    // Coming back is arriving too: she left in the county, she returns at the bed.
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "county", mark: "start" } });
    sim.command(1, { t: "leave" });
    expect(sim.command(-1, { t: "join", who: "b" })).toBe(1);
    expect(sim.state.players[1].zone).toBe("house");
  });
});

describe("growth is the world's, items are hers", () => {
  it("what one learns they all know, including whoever is away and whoever comes later", () => {
    const sim = party(24, ["host", "b", "c"]);
    sim.command(2, { t: "leave" });
    sim.drainEvents();
    sim.command(1, { t: "dev", dev: { op: "learn", spell: "icebolt" } });
    expect(sim.state.growth.spells).toEqual(["icebolt"]);
    for (const seat of [0, 1]) {
      expect(sim.view(seat).player.book).toContain("icebolt");
      expect(sim.view(seat).me.bar.some((s) => s?.source === "spell" && s.id === "icebolt")).toBe(true);
    }
    expect(sim.state.players[2].parked!.book).toContain("icebolt");
    // Told to everyone, once.
    const learned = sim.drainEvents().filter((e) => e.e === "learn");
    expect(learned.length).toBe(1);
    expect(learned[0].to).toBeUndefined();

    expect(sim.command(-1, { t: "join", who: "d" })).toBe(3);
    expect(sim.view(3).player.book).toContain("icebolt");
    expect(sim.view(3).me.bar.some((s) => s?.source === "spell" && s.id === "icebolt")).toBe(true);
    // Items did not follow: the newcomer has the starting kit and nothing else.
    sim.command(0, { t: "dev", dev: { op: "give", item: "gold_bar", qty: 2 } });
    expect(bagCount(sim.view(3).player, "gold_bar")).toBe(0);
  });

  it("a key never leaves with a guest: what the story needs is handed to someone staying", () => {
    const sim = party(25, ["host", "b"]);
    sim.command(1, { t: "dev", dev: { op: "give", item: "key_mine_boss", qty: 1 } });
    sim.command(1, { t: "dev", dev: { op: "give", item: "rat_meat", qty: 3 } }); // a quest asks for these
    sim.command(1, { t: "dev", dev: { op: "give", item: "gold_bar", qty: 5 } });
    expect(catalog.storyItems.has("key_mine_boss")).toBe(true);
    expect(catalog.storyItems.has("rat_meat")).toBe(true);
    expect(catalog.storyItems.has("gold_bar")).toBe(false);
    sim.command(1, { t: "leave" });
    const host = sim.view(0).player;
    const away = sim.state.players[1].parked!;
    expect(bagCount(host, "key_mine_boss")).toBe(1);
    expect(bagCount(host, "rat_meat")).toBe(3);
    expect(bagCount(host, "gold_bar")).toBe(0);
    expect(bagCount(away, "key_mine_boss")).toBe(0);
    expect(bagCount(away, "gold_bar")).toBe(5); // hers, and waiting for her
  });

  it("with no room in the bags it lands at her feet, and never ages out", () => {
    const sim = party(26, ["host", "b"]);
    const host = sim.view(0).player;
    for (let i = 0; i < host.bag!.length; i++) host.bag![i] ??= { item: "rock", qty: 1 };
    sim.command(1, { t: "dev", dev: { op: "give", item: "key_mine_vault", qty: 1 } });
    sim.command(1, { t: "leave" });
    expect(sim.zone.drops.some((d) => d.item === "key_mine_vault")).toBe(true);
    idle(sim, 60 * 200);
    expect(sim.zone.drops.some((d) => d.item === "key_mine_vault")).toBe(true);
  });
});

describe("healing a friend", () => {
  // No heal spell has been placed in the county yet; the verb is proven on a row made here.
  const cat = buildCatalog();
  const mend: SpellDef = {
    name: "Mend", description: "", icon: "apple", kind: "ally", school: "heal", mp: 0, energy: 0, range: 15,
    cooldown: 0, gcdImmune: true, needsTarget: false, needsEnemy: false, needsLos: true, anim: "cast",
    power: { stat: "spirit", div: 2, varDiv: 8 }, stop: 0,
  };
  cat.spells.mend = mend;

  function hurtPair(seed: number): { sim: Sim; me: Unit; her: Unit } {
    const sim = party(seed, ["host", "b"], cat);
    sim.command(0, { t: "dev", dev: { op: "learn", spell: "mend" } });
    const me = sim.view(0).player;
    const her = sim.view(1).player;
    me.hp = 40;
    her.hp = 40;
    sim.drainEvents();
    return { sim, me, her };
  }

  it("mouse: over her it lands on her, and the event says who cast it", () => {
    const { sim, me, her } = hurtPair(27);
    // The cursor is on her, so the aim line points at her too. `on` is what decides.
    const aim = normalize(her.x - me.x, her.y - me.y);
    sim.setAim(aim.x, aim.y, 0);
    sim.command(0, { t: "cast", spell: "mend", on: her.id });
    idle(sim, 2);
    expect(her.hp).toBeGreaterThan(40);
    expect(me.hp).toBe(40);
    const heal = sim.drainEvents().find((e) => e.e === "heal");
    expect(heal && heal.e === "heal" && heal.unit === her.id && heal.from === me.id).toBe(true);
  });

  it("mouse: over nobody, over yourself or over an enemy it lands on you, even with her dead ahead", () => {
    const { sim, me, her } = hurtPair(28);
    const aim = normalize(her.x - me.x, her.y - me.y);
    const skeleton = sim.rt.unitsByKey.get("yard_skeleton")!;
    for (const on of [0, me.id, skeleton.id]) {
      me.hp = 40;
      sim.setAim(aim.x, aim.y, 0);
      sim.command(0, { t: "cast", spell: "mend", on });
      idle(sim, 2);
      expect(me.hp, `on=${on}`).toBeGreaterThan(40);
      expect(her.hp, `on=${on}`).toBe(40);
    }
  });

  it("mouse: pointing at a friend who is out of reach fails and says so; it does not fall on you instead", () => {
    const { sim, me, her } = hurtPair(29);
    placeUnit(sim.view(1), her, her.x + 400, her.y);
    sim.command(0, { t: "cast", spell: "mend", on: her.id });
    idle(sim, 2);
    expect(her.hp).toBe(40);
    expect(me.hp).toBe(40);
    const failed = sim.drainEvents().find((e) => e.e === "castFailed");
    expect(failed && failed.e === "castFailed" && ["tooFar", "notInLOS"].includes(failed.error)).toBe(true);
  });

  it("pad: no cursor, so the friend along the right stick; stick at rest, yourself", () => {
    const { sim, me, her } = hurtPair(30);
    const aim = normalize(her.x - me.x, her.y - me.y);
    sim.setAim(aim.x, aim.y, 0);
    sim.command(0, { t: "cast", spell: "mend" });
    idle(sim, 2);
    expect(her.hp).toBeGreaterThan(40);
    expect(me.hp).toBe(40);

    sim.setAim(0, 0, 0);
    sim.command(0, { t: "cast", spell: "mend" });
    idle(sim, 2);
    expect(me.hp).toBeGreaterThan(40);
  });

  it("a friend in another zone is not a target, whatever the client says", () => {
    const { sim, me, her } = hurtPair(31);
    sim.command(1, { t: "dev", dev: { op: "tp", zone: "house", mark: "front" } });
    sim.command(0, { t: "cast", spell: "mend", on: her.id });
    idle(sim, 2);
    expect(sim.view(1).player.hp).toBe(40);
    expect(me.hp).toBeGreaterThan(40);
  });
});
