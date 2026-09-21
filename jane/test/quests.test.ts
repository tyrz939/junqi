// Quests. The last attempt at this game shipped 102 quests; 61 could not be handed in and
// 33 were never offered (archive/phaser-remake-2026/POSTMORTEM.md P0.4). Two kinds of proof
// here, so that cannot happen again:
//
//  1. For EVERY quest in the catalog: something reachable gives it, something reachable hands
//     it in, every place it names is produced, every creature it names is spawned, every
//     item it names can be had. Placed props carry action lists the catalog cannot see, so
//     this builds the county and walks its props.
//  2. The Lowfields side quests (QUESTS.md Part 3), all nineteen, played end to end by the
//     headless player: offered, accepted, done, handed in, paid, and not payable twice.

import { describe, expect, it } from "vitest";
import type { Catalog, DialogueTree } from "@/sim/catalog";
import { focusOf } from "@/sim/interact";
import { bagCount } from "@/sim/inventory";
import { handIn, questActive, questDone, questReady } from "@/sim/quests";
import { Sim } from "@/sim/sim";
import type { Action, ActionList, Unit } from "@/sim/state";
import { isNight } from "@/sim/text";
import { blueprintFor } from "@/sim/zones";
import { ZONE_IDS } from "@/world";
import type { PropSpawn, UnitSpawn } from "@/world/blueprint";
import lowfieldsDialogue from "@/data/dialogue/lowfields.json";
import lowfieldsItems from "@/data/items/lowfields.json";
import lowfieldsQuests from "@/data/quests/lowfields.json";
import { idle, talkThrough, walkToProp, walkToUnit, yardCatalog } from "./bot";

const catalog = yardCatalog();
const SEEDS = [3, 2026];
const LOWFIELDS = Object.keys(lowfieldsQuests);
const LONG = 120_000;

// --- 1. the catalog, against a built world ----------------------------------------------

type Built = { props: PropSpawn[]; units: UnitSpawn[]; rects: Record<string, Set<string>>; broken: string[] };

/** Every zone of a seed, through the same cache the sim uses (a county is about a second). */
function build(seed: number): Built {
  const out: Built = { props: [], units: [], rects: {}, broken: [] };
  for (const zone of ZONE_IDS) {
    // A zone that will not build is world.test's business. A quest that leans on it fails below, by name,
    // and the message says which zone was missing and why.
    try {
      const bp = blueprintFor(zone, seed);
      out.props.push(...bp.props);
      out.units.push(...bp.units);
      out.rects[zone] = new Set(Object.keys(bp.rects));
    } catch (err) {
      out.rects[zone] = new Set();
      const why = String(err instanceof Error ? err.message : err).split(/\r?\n/)[0];
      out.broken.push(`zone "${zone}" did not build: ${why}`);
    }
  }
  return out;
}

type Reach = { given: Set<string>; handins: Set<string>; locations: Set<string>; spawned: Set<string>; items: Set<string> };

/** Everything the player can set in motion, followed to a fixed point. Conditions are not evaluated: this is about rows existing and being joined up. */
function reach(c: Catalog, world: Built): Reach {
  const lists: ActionList[] = [];
  const trees = new Set<string>();
  const spawned = new Set<string>(world.units.map((u) => u.def));
  const seenDeath = new Set<string>();
  const given = new Set<string>(c.start.quests);
  const paid = new Set<string>();
  const items = new Set<string>(c.start.items.map((s) => s.item));

  const addTree = (id: string): void => {
    const tree: DialogueTree | undefined = c.dialogue[id];
    if (!tree || trees.has(id)) return;
    trees.add(id);
    const open = tree.start.map((s) => s.node);
    const seen = new Set<string>();
    while (open.length > 0) {
      const id = open.pop()!;
      if (seen.has(id) || !tree.nodes[id]) continue;
      seen.add(id);
      const node = tree.nodes[id];
      if (node.actions) lists.push(node.actions);
      if (node.goto) open.push(node.goto);
      for (const o of node.options ?? []) {
        if (o.actions) lists.push(o.actions);
        if (o.goto) open.push(o.goto);
      }
    }
  };

  for (const p of world.props) {
    if (p.talk) addTree(p.talk);
    if (p.use) lists.push(p.use);
    if (p.release) lists.push(p.release);
    for (const s of p.loot ?? []) items.add(s.item);
  }
  for (const t of Object.values(c.triggers)) if (world.rects[t.zone]?.has(t.rect)) lists.push(t.actions);
  for (const i of Object.values(c.items)) if (i.use) lists.push(i.use);
  for (const row of c.clock) lists.push(row.actions);

  // Lists found while reading lists (a unit's death, a quest's pay, a tree an action opens) are read in turn.
  for (let n = 0; n < lists.length || spawned.size > seenDeath.size; n++) {
    for (const def of spawned) {
      if (seenDeath.has(def)) continue;
      seenDeath.add(def);
      const u = c.units[def];
      if (!u) continue;
      if (u.talk) addTree(u.talk);
      if (u.onDeath) lists.push(u.onDeath);
      for (const l of u.loot) if (l.chance >= 1) items.add(l.item);
    }
    for (const a of lists[n] ?? []) {
      if (a.do === "talk") addTree(a.tree);
      if (a.do === "spawn") spawned.add(a.def);
      if (a.do === "give") items.add(a.item);
      if (a.do === "quest") given.add(a.quest);
    }
    // A quest that can be given and handed in pays out, and its pay is a list like any other.
    const handins = new Set(lists.flatMap((l) => l.filter((a): a is Extract<Action, { do: "handin" }> => a.do === "handin").map((a) => a.quest)));
    for (const q of handins) {
      if (!given.has(q) || paid.has(q) || !c.quests[q]) continue;
      paid.add(q);
      lists.push(c.quests[q].rewards);
    }
  }
  // Recipes: an output is to be had once all its inputs are.
  for (let changed = true; changed; ) {
    changed = false;
    for (const r of c.recipes) {
      if (!items.has(r.output) && r.inputs.every((i) => items.has(i))) {
        items.add(r.output);
        changed = true;
      }
    }
  }
  const all = lists.flat();
  return {
    given,
    handins: new Set(all.filter((a): a is Extract<Action, { do: "handin" }> => a.do === "handin").map((a) => a.quest)),
    locations: new Set(all.filter((a): a is Extract<Action, { do: "location" }> => a.do === "location").map((a) => a.name)),
    spawned,
    items,
  };
}

describe("every quest in the game", () => {
  it("is given somewhere, handed in somewhere, and asks only for places, creatures and things that exist (two seeds)", () => {
    expect(Object.keys(catalog.quests).length).toBeGreaterThanOrEqual(7 + 19);
    for (const seed of SEEDS) {
      const world = build(seed);
      const r = reach(catalog, world);
      const note = world.broken.length > 0 ? ` (${world.broken.join("; ")})` : "";
      for (const [id, q] of Object.entries(catalog.quests)) {
        expect(r.given.has(id), `seed ${seed}: nothing reachable gives "${id}"${note}`).toBe(true);
        expect(r.handins.has(id), `seed ${seed}: nothing reachable hands in "${id}"${note}`).toBe(true);
        for (const req of q.requirements) {
          if (req.type === "location") expect(r.locations.has(req.target), `seed ${seed}: ${id} names the place "${req.target}" and nothing produces it${note}`).toBe(true);
          if (req.type === "kill") expect(r.spawned.has(req.target), `seed ${seed}: ${id} wants "${req.target}" killed and none is spawned${note}`).toBe(true);
          if (req.type === "acquire") expect(r.items.has(req.target), `seed ${seed}: ${id} wants "${req.target}" and it cannot be had${note}`).toBe(true);
        }
      }
    }
  }, LONG);

  it("a kill quest tied to a place has N+1 of its own creature standing there", () => {
    const world = build(SEEDS[0]);
    const count = (def: string): number => world.units.filter((u) => u.def === def).length;
    expect(count("pumpkin_top")).toBe(7);
    expect(count("quarryman")).toBe(6);
    expect(count("plot_tenant")).toBe(1);
    expect(world.units.filter((u) => u.key.startsWith("rat_shed_") || u.key.startsWith("rat_allotment_")).length).toBe(8);
    // Threat sets the phase: the Top Field and Quarry Steps play at 3, the allotments at 2.
    for (const u of world.units) {
      if (u.def === "pumpkin_top" || u.def === "quarryman") expect(u.phase, u.key).toBe(3);
      if (u.def === "plot_tenant" || u.key.startsWith("rat_shed_") || u.key.startsWith("rat_allotment_")) expect(u.phase, u.key).toBe(2);
    }
  }, LONG);

  it("in every tree a hand-in row stands above every other row, and leads to its hand-in", () => {
    // The one documented exception (QUESTS.md ASKS C): Julie's garden book shows a night-time progress
    // line for `rose_and_stone`, whose hand-in home is the dog.
    const excepted = (tree: string, quest: string): boolean => tree === "garden_book" && quest === "rose_and_stone";
    for (const [id, tree] of Object.entries(catalog.dialogue)) {
      let lastReady = -1;
      let firstOther = Infinity;
      tree.start.forEach((row, n) => {
        const ready = (row.when ?? []).filter((k) => k.if === "questReady" && !k.not);
        if (ready.some((k) => k.if === "questReady" && excepted(id, k.quest))) return;
        if (ready.length === 0) {
          firstOther = Math.min(firstOther, n);
          return;
        }
        lastReady = n;
        // From this row's node, some option or node hands that quest in.
        const quest = ready[0].if === "questReady" ? ready[0].quest : "";
        const open = [row.node];
        const seen = new Set<string>();
        let found = false;
        while (open.length > 0 && !found) {
          const nid = open.pop()!;
          if (seen.has(nid)) continue;
          seen.add(nid);
          const node = tree.nodes[nid];
          const acts = [...(node.actions ?? []), ...(node.options ?? []).flatMap((o) => o.actions ?? [])];
          found = acts.some((a) => a.do === "handin" && a.quest === quest);
          if (node.goto) open.push(node.goto);
          for (const o of node.options ?? []) if (o.goto) open.push(o.goto);
        }
        expect(found, `${id}: the row that is ready for "${quest}" never hands it in`).toBe(true);
      });
      expect(lastReady, `${id}: a questReady row sits below a row that would answer first`).toBeLessThan(firstOther);
    }
  });

  it("the Lowfields rows keep to VOICE.md and QUESTS.md K3: short, no dashes, nobody's name baked in", () => {
    expect(LOWFIELDS.length).toBe(19);
    for (const id of LOWFIELDS) expect(catalog.quests[id].description.length, `${id}: description length`).toBeLessThanOrEqual(320);
    const text = JSON.stringify([lowfieldsQuests, lowfieldsDialogue, lowfieldsItems, catalog.dialogue.dog]);
    expect(/[–—]/.test(text), "an en or em dash").toBe(false);
    expect(text.includes("Jane")).toBe(false);
  });
});

// --- 2. the Lowfields, played ------------------------------------------------------------

function newSim(seed: number): Sim {
  const sim = Sim.newGame(catalog, seed);
  sim.command({ t: "dev", dev: { op: "god", on: true } });
  day(sim);
  return sim;
}

function day(sim: Sim): void {
  sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
  expect(isNight(sim.state)).toBe(false);
}

function night(sim: Sim): void {
  sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
  expect(isNight(sim.state)).toBe(true);
}

/** `dev tp` to a mark in the county: the long walks are county.test's business, not this file's. */
function tp(sim: Sim, mark: string): void {
  expect(sim.rt.bp.marks[mark], `mark ${mark}`).toBeDefined();
  sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark } });
  idle(sim, 3);
}

/** Walk up to a prop and press USE on it, and on nothing else. */
function press(sim: Sim, key: string, mark?: string): void {
  if (mark) tp(sim, mark);
  const prop = sim.rt.propsByKey.get(key);
  expect(prop, `prop ${key}`).toBeDefined();
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  for (let n = 0; n < 6; n++) {
    const f = focusOf(sim, sim.player);
    if (f?.kind !== "drop") break;
    sim.command({ t: "use" }); // something a creature dropped is lying in the way: pick it up
    idle(sim, 1);
  }
  const f = focusOf(sim, sim.player);
  expect(f?.kind === "prop" && f.id === prop!.id, `USE beside ${key} acts on ${key} (it would act on ${JSON.stringify(f)})`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
}

/** Press USE on a talking prop, check which node answers, and click through it. */
function read(sim: Sim, key: string, node: string, choices: number[] = [], mark?: string): void {
  press(sim, key, mark);
  expect(sim.me.dialogue?.node, `${key} answers with`).toBe(node);
  talkThrough(sim, choices);
  expect(sim.me.dialogue).toBeNull();
  idle(sim, 1);
}

/** What a fight comes to: damage from her, enough of it. The creature must be there to be struck. */
function strike(sim: Sim, key: string): Unit {
  const u = sim.rt.unitsByKey.get(key);
  expect(u, `unit ${key}`).toBeDefined();
  expect(u!.hidden, `${key} is there`).toBe(false);
  u!.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
  idle(sim, 2);
  expect(u!.alive).toBe(false);
  return u!;
}

function expectDone(sim: Sim, quest: string): void {
  expect(questDone(sim, quest), `${quest} is done`).toBe(true);
  expect(questActive(sim, quest)).toBeUndefined();
  // Done is done: asking again pays nothing.
  expect(handIn(sim, quest)).toBe(false);
}

const has = (sim: Sim, item: string): number => bagCount(sim.player, item);

describe("the Lowfields side quests, played", () => {
  it("A. Lost Property: three things by reading, the carter's key across the map, the trunk, and the parcel after the bell", () => {
    const sim = newSim(SEEDS[0]);
    const p = sim.player;

    // A1. Offered from the first minute, by a book.
    read(sim, "lost_property_book", "lp_offer", [0], "start");
    expect(questActive(sim, "lost_property")).toBeDefined();
    read(sim, "lost_property_book", "lp_wait");
    press(sim, "lost_glove_drop", "halt_well");
    press(sim, "lost_hat_drop", "halt_signpost");
    expect(questReady(sim, "lost_property")).toBe(false);
    press(sim, "lost_tin_drop", "halt_cart");
    expect([has(sim, "lost_glove"), has(sim, "lost_hat"), has(sim, "lost_tin")]).toEqual([1, 1, 1]);
    expect(sim.rt.propsByKey.get("lost_tin_drop")!.hidden).toBe(true);
    const apples = has(sim, "apple");
    read(sim, "lost_property_book", "lp_in", [0], "start");
    expectDone(sim, "lost_property");
    expect([has(sim, "lost_glove"), has(sim, "lost_hat"), has(sim, "lost_tin")]).toEqual([0, 0, 0]);
    expect(has(sim, "apple")).toBe(apples + 2);
    expect(has(sim, "key_generic")).toBe(1);
    // One article from the unclaimed shelf: the plain key turns once and is the lock's.
    press(sim, "unclaimed_shelf");
    press(sim, "unclaimed_shelf");
    expect(has(sim, "key_generic")).toBe(0);
    expect(has(sim, "grape")).toBe(4);

    // A2. The trunk was there all along, and is locked.
    read(sim, "lost_property_book", "ll_offer", [0]);
    press(sim, "left_luggage_trunk");
    expect(sim.rt.propsByKey.get("left_luggage_trunk")!.locked).toBe(true);
    read(sim, "lost_property_book", "ll_wait");
    press(sim, "carters_key_drop", "carters_cart");
    read(sim, "carters_note", "read");
    expect(has(sim, "key_left_luggage")).toBe(1);
    expect(sim.state.flags["been:carters_cart"]).toBe(1);
    expect(questReady(sim, "left_luggage")).toBe(false);
    const spirit = p.spirit;
    tp(sim, "start");
    press(sim, "left_luggage_trunk"); // the key turns, and is consumed
    expect(has(sim, "key_left_luggage")).toBe(0);
    expect(questDone(sim, "left_luggage")).toBe(false);
    press(sim, "left_luggage_trunk"); // the lid: what is inside, and the hand-in
    expectDone(sim, "left_luggage");
    expect(sim.state.growth.found).toContain("page_left_luggage");
    expect(p.spirit).toBe(spirit + 3);
    expect(sim.state.growth.spirit).toBe(3);
    expect(has(sim, "gold_dust")).toBe(2);

    // A3. The same platform, after the bell. She is standing on it when the bell goes: credited without stepping off.
    read(sim, "lost_property_book", "tbc_offer", [0]);
    tp(sim, "start");
    expect(sim.state.flags["been:platform_after_nine"]).toBeUndefined();
    const parcel = sim.rt.propsByKey.get("night_parcel")!;
    expect(parcel.hidden).toBe(true);
    night(sim);
    idle(sim, 3);
    expect(sim.state.flags["been:platform_after_nine"]).toBe(1);
    expect(parcel.hidden, "she never sees it arrive").toBe(true);
    read(sim, "lost_property_book", "tbc_in", [0]);
    expectDone(sim, "to_be_collected");
    expect(has(sim, "light_stone")).toBe(2); // one from the shelf, one for checking
    read(sim, "lost_property_book", "closed");

    // Walk away and come back: it is there. Come back by day: it is not.
    tp(sim, "yard_gate");
    tp(sim, "start");
    expect(parcel.hidden).toBe(false);
    tp(sim, "yard_gate");
    day(sim);
    tp(sim, "start");
    expect(parcel.hidden).toBe(true);
    tp(sim, "yard_gate");
    night(sim);
    tp(sim, "start");
    // The decision: a page, and the platform lamp never lights again.
    press(sim, "night_parcel");
    expect(sim.state.growth.found).toContain("page_night_parcel");
    expect(p.spirit).toBe(spirit + 6);
    expect(parcel.hidden).toBe(true);
    expect(sim.rt.propsByKey.get("station_lamp")!.hidden).toBe(true);
    expect(sim.rt.propsByKey.get("station_lamp_dead")!.hidden).toBe(false);
    read(sim, "lost_property_book", "closed_taken");
    tp(sim, "yard_gate");
    tp(sim, "start");
    expect(parcel.hidden, "taken is taken").toBe(true);
    expect(sim.state.growth.found.filter((id) => id.startsWith("page_")).length).toBe(2);
  }, LONG);

  it("B. The Allotments: one notice at a time, six rats, the shed, and the tenant of Plot 9 who is only there after the bell", () => {
    const sim = newSim(SEEDS[1]);
    const p = sim.player;

    // The board shows one notice. Refuse the footpath and it moves on.
    read(sim, "parish_board", "path_offer", [1], "town_square");
    read(sim, "parish_board", "rats_offer", [0]);
    read(sim, "parish_board", "rats_wait");
    tp(sim, "allotment_shed");
    for (const key of ["rat_shed_1", "rat_shed_2", "rat_shed_3", "rat_shed_4", "rat_allotment_1"]) strike(sim, key);
    expect(questReady(sim, "rats_in_the_sheds")).toBe(false);
    strike(sim, "rat_allotment_2");
    expect(questReady(sim, "rats_in_the_sheds")).toBe(true);
    const shed = sim.rt.propsByKey.get("allotment_shed")!;
    press(sim, "allotment_shed");
    expect(shed.locked, "members only").toBe(true);
    read(sim, "parish_board", "rats_in", [0], "town_square");
    expectDone(sim, "rats_in_the_sheds");
    expect(has(sim, "key_generic")).toBe(1);
    expect(has(sim, "pansy")).toBe(2);
    press(sim, "allotment_shed", "allotment_shed");
    press(sim, "allotment_shed");
    expect(has(sim, "key_generic")).toBe(0);
    expect(has(sim, "honeylace_lily")).toBe(2);

    // Plot 9. By day there is a stake and nobody.
    read(sim, "parish_board", "plot_offer", [0], "town_square");
    const tenant = sim.rt.unitsByKey.get("plot_tenant")!;
    expect(tenant.hidden).toBe(true);
    read(sim, "plot_nine_stake", "day", [], "plot_nine_stake");
    read(sim, "parish_board", "plot_wait", [], "town_square");
    // After the bell, from far enough away that nobody sees him come.
    night(sim);
    idle(sim, 40);
    expect(tenant.hidden).toBe(false);
    read(sim, "plot_nine_stake", "night", [], "plot_nine_stake");
    const strength = p.strength;
    strike(sim, "plot_tenant");
    expect(sim.state.flags["been:plot_tenant_down"]).toBe(1);
    expect(questReady(sim, "plot_nine")).toBe(true);
    read(sim, "parish_board", "plot_in", [0], "town_square");
    expectDone(sim, "plot_nine");
    expect(sim.state.growth.found).toContain("jar_plot_nine");
    expect(p.strength).toBe(strength + 3);
    // The footpath she refused comes round again once the board has nothing else.
    read(sim, "parish_board", "board_idle");
    read(sim, "parish_board", "path_offer", [1]);
  }, LONG);

  it("B. Plot 9 survives being done out of order: the tenant put down before the notice was read", () => {
    const sim = newSim(SEEDS[0]);
    night(sim);
    idle(sim, 40);
    tp(sim, "plot_nine_stake");
    strike(sim, "plot_tenant");
    sim.state.quests.done.push("rats_in_the_sheds");
    sim.state.flags.seen_path = 1;
    read(sim, "parish_board", "plot_offer", [0], "town_square");
    read(sim, "parish_board", "plot_in", [0]);
    expectDone(sim, "plot_nine");
    expect(sim.state.growth.found).toEqual(["jar_plot_nine"]);
  }, LONG);

  it("C. Lowfield Farm: three scarecrows by day, the same three after the bell, and six things that are not turnips", () => {
    const sim = newSim(SEEDS[0]);
    const p = sim.player;
    const scarecrows = ["scarecrow_gate", "scarecrow_hedge", "scarecrow_top"];

    read(sim, "farm_door", "farmer_first", [0], "farm_door");
    read(sim, "farm_door", "farmer_wait_1");
    for (const s of scarecrows) read(sim, s, "day", [], s);
    const apples = has(sim, "apple");
    read(sim, "farm_door", "farmer_in_1", [0], "farm_door");
    expectDone(sim, "three_scarecrows");
    expect(has(sim, "apple")).toBe(apples + 4);

    // The same walk with the county's one rule applied to it: by day it does not count.
    read(sim, "farm_door", "farmer_second", [0]);
    read(sim, "scarecrow_gate", "day", [], "scarecrow_gate");
    expect(questReady(sim, "after_the_bell")).toBe(false);
    night(sim);
    for (const s of scarecrows) read(sim, s, "night", [], s);
    expect(questReady(sim, "after_the_bell")).toBe(true);
    read(sim, "farm_door", "farmer_in_2", [0], "farm_door");
    expectDone(sim, "after_the_bell");
    expect(has(sim, "potion_stoneskin")).toBe(1);
    expect(has(sim, "nasturtium")).toBe(2);

    // By day, as he said. Seven stand in the field; six are asked for.
    day(sim);
    read(sim, "farm_door", "farmer_third", [0]);
    tp(sim, "top_field");
    for (let n = 1; n <= 5; n++) strike(sim, `pumpkin_top_${n}`);
    expect(questReady(sim, "not_turnips")).toBe(false);
    strike(sim, "pumpkin_top_6");
    const strength = p.strength;
    read(sim, "farm_door", "farmer_in_3", [0], "farm_door");
    expectDone(sim, "not_turnips");
    expect(sim.state.growth.found).toContain("jar_farm");
    expect(p.strength).toBe(strength + 3);
    read(sim, "farm_door", "farmer_idle");
  }, LONG);

  it("C. if the scarecrow omen is true, the one on the Top Field stands nearer the farm after the bell, and still counts", () => {
    const sim = newSim(SEEDS[1]);
    const far = sim.rt.propsByKey.get("scarecrow_top")!;
    const near = sim.rt.propsByKey.get("scarecrow_top_near")!;
    expect([far.hidden, near.hidden]).toEqual([false, true]);
    // Both stand well inside the wide rect, so neither can be seen to change (the view is 48 x 27 cells).
    const r = sim.rt.bp.rects.scarecrow_top_wide;
    for (const s of [far, near]) {
      expect(s.cx - r.cx).toBeGreaterThan(24);
      expect(r.cx + r.w - s.cx).toBeGreaterThan(24);
      expect(s.cy - r.cy).toBeGreaterThan(14);
      expect(r.cy + r.h - s.cy).toBeGreaterThan(14);
    }
    sim.command({ t: "dev", dev: { op: "flag", flag: "omen:scarecrow_closer", value: 1 } });
    night(sim);
    tp(sim, "scarecrow_top_near");
    expect([far.hidden, near.hidden]).toEqual([true, false]);
    read(sim, "scarecrow_top_near", "night");
    expect(sim.state.flags["been:scarecrow_top_night"]).toBe(1);
    tp(sim, "yard_gate");
    day(sim);
    tp(sim, "scarecrow_top");
    expect([far.hidden, near.hidden]).toEqual([false, true]);
  }, LONG);

  it("D. The Nurse's Round: three notes on three steps, her coat behind the car, and a parcel through Mrs Allen's door", () => {
    const sim = newSim(SEEDS[1]);
    const p = sim.player;

    // The car keeps its planks; once they are taken, USE reads the glovebox.
    press(sim, "car_wreck", "nurses_case");
    expect(has(sim, "wood")).toBe(2);
    read(sim, "car_wreck", "glovebox_first", [0]);
    // Nobody is seeing anyone. Without the parcel the door has nothing to hand in.
    read(sim, "door_allen", "allen_before", [], "allen_door");
    read(sim, "note_allen", "read", [], "cottage_allen");
    read(sim, "note_pike", "read", [], "cottage_pike");
    expect(questReady(sim, "the_nurses_round")).toBe(false);
    read(sim, "note_crane", "read", [], "cottage_crane");
    // The second sheet is under the first: handing in the round opens her warning.
    read(sim, "car_wreck", "glovebox_in_1", [0, 0], "nurses_case");
    expectDone(sim, "the_nurses_round");
    expect(has(sim, "potion_manashield")).toBe(1);
    expect(questActive(sim, "her_coat")).toBeDefined();
    read(sim, "car_wreck", "glovebox_wait_2");

    const lockedCase = sim.rt.propsByKey.get("nurses_case")!;
    press(sim, "nurses_case");
    expect(lockedCase.locked).toBe(true);
    press(sim, "nurses_coat_drop", "nurses_coat");
    expect(has(sim, "key_nurses_case")).toBe(1);
    expect(sim.state.flags["been:nurses_coat"]).toBe(1);
    press(sim, "nurses_case", "nurses_case");
    expect(has(sim, "key_nurses_case")).toBe(0);
    press(sim, "nurses_case");
    expectDone(sim, "her_coat");
    expect(has(sim, "potion_lifesteal")).toBe(1);
    expect(has(sim, "nurses_parcel")).toBe(1);
    expect(questReady(sim, "mrs_allens_dressing"), "the case gives the last errand, already in hand").toBe(true);
    read(sim, "car_wreck", "glovebox_empty");

    const strength = p.strength;
    read(sim, "door_allen", "allen_in", [0], "allen_door");
    expectDone(sim, "mrs_allens_dressing");
    expect(has(sim, "nurses_parcel")).toBe(0);
    expect(sim.state.growth.found).toContain("jar_mrs_allen");
    expect(p.strength).toBe(strength + 3);
    read(sim, "door_allen", "allen_after");
    expect(p.strength).toBe(strength + 3);
  }, LONG);

  it("E. The Lampman: twelve, thirteen, fifteen after the bell; a fire lit somewhere new; and the dog, who did ask", () => {
    const sim = newSim(SEEDS[0]);
    read(sim, "pell_stone", "pell_first", [0], "pell_shrine");
    // By day there is no telling a dead lamp from a live one.
    read(sim, "lamp_12", "day", [], "lamp_12");
    expect(sim.state.flags["been:lamp_12"]).toBeUndefined();
    night(sim);
    for (const lamp of ["lamp_12", "lamp_13", "lamp_15"]) read(sim, lamp, "night", [], lamp);
    read(sim, "pell_stone", "pell_in", [1], "pell_shrine"); // "Fourteen, then"
    expectDone(sim, "number_fourteen");
    expect(sim.state.flags.said_fourteen).toBe(1);
    expect(has(sim, "light_stone")).toBe(2);

    // The brazier wants what the car holds, and what the mine's stair wants.
    read(sim, "pell_stone", "pell_second", [0]);
    const cold = sim.rt.propsByKey.get("pell_brazier_cold")!;
    const fire = sim.rt.propsByKey.get("pell_brazier")!;
    expect([cold.hidden, fire.hidden]).toEqual([false, true]);
    read(sim, "pell_brazier_cold", "brazier_empty", [], "pell_brazier");
    press(sim, "car_wreck", "nurses_case");
    expect([has(sim, "wood"), has(sim, "fire_stone")]).toEqual([2, 1]);
    read(sim, "pell_brazier_cold", "brazier_ready", [0], "pell_brazier");
    expectDone(sim, "the_lampmans_brazier");
    expect([has(sim, "wood"), has(sim, "fire_stone")]).toEqual([0, 0]);
    expect([cold.hidden, fire.hidden]).toEqual([true, false]);
    // It rests and saves like any other fire.
    read(sim, "pell_brazier", "fire");
    expect(sim.state.rest?.zone).toBe("county");
    read(sim, "pell_stone", "pell_after", [], "pell_shrine");

    // Next morning, once, below every hand-in and above the poke chain.
    sim.state.quests.done.push("defeat_skeleton"); // past the intro, as in rest.test
    day(sim);
    idle(sim, 40);
    tp(sim, "house_front");
    const talk = (): string => {
      walkToUnit(sim, "dog");
      sim.command({ t: "use" });
      const node = sim.me.dialogue!.node;
      talkThrough(sim);
      return node;
    };
    expect(talk()).toBe("lamps_counted_f");
    expect(sim.state.flags.dog_heard_lamps).toBe(1);
    expect(talk()).toBe("idle");
  }, LONG);

  it("F. The Garden Book: three roses before the bell, and a potion the dog wants to see", () => {
    const sim = newSim(SEEDS[1]);
    read(sim, "garden_book", "book_first", [0], "garden_book");
    tp(sim, "sallow_jetty");
    for (const n of [1, 2, 3]) press(sim, `sallow_rose_${n}`);
    expect(has(sim, "white_water_rose")).toBe(3);
    read(sim, "garden_book", "book_in_1", [0], "garden_book");
    expectDone(sim, "before_the_bell");
    expect([has(sim, "rock"), has(sim, "small_water")]).toEqual([2, 2]);
    expect(has(sim, "white_water_rose"), "the roses are kept: the next page needs one").toBe(3);

    read(sim, "garden_book", "book_second", [0]);
    read(sim, "garden_book", "book_wait_2");
    // Rock to stone, then stone, water and rose: the bench's business (ui.test plays it). Here it is simply made.
    sim.command({ t: "dev", dev: { op: "give", item: "potion_stoneskin", qty: 1 } });
    expect(questReady(sim, "rose_and_stone")).toBe(true);
    // The dog is not there from nine to six, so at night the book only says so. It cannot hand in.
    night(sim);
    read(sim, "garden_book", "book_in_2");
    expect(questDone(sim, "rose_and_stone")).toBe(false);
    day(sim);
    tp(sim, "yard_gate");
    idle(sim, 40);
    tp(sim, "house_front");
    walkToUnit(sim, "dog");
    sim.command({ t: "use" });
    expect(sim.me.dialogue?.node).toBe("stoneskin_done");
    talkThrough(sim, [0]);
    expectDone(sim, "rose_and_stone");
    expect(has(sim, "potion_stoneskin"), "shown, not taken").toBe(1);
    expect([has(sim, "honeylace_lily"), has(sim, "gold_dust")]).toEqual([2, 1]);
    read(sim, "garden_book", "book_after", [], "garden_book");
  }, LONG);

  it("G. The Company: five men paid off at a hatch with nobody behind it, and the one who never came down", () => {
    const sim = newSim(SEEDS[0]);
    const p = sim.player;
    // Before the gang is stood down the slate is only a slate.
    read(sim, "tally_slate", "slate_plain", [], "quarry_camp");
    read(sim, "company_notice", "company_offer", [0], "company_notice");
    for (const key of ["quarryman_a_1", "quarryman_a_2", "quarryman_a_3", "quarryman_b_1"]) strike(sim, key);
    read(sim, "company_notice", "company_wait");
    strike(sim, "quarryman_b_2");
    read(sim, "company_notice", "company_in", [0]);
    expectDone(sim, "stood_down");
    expect([has(sim, "iron"), has(sim, "wood")]).toEqual([4, 2]);
    read(sim, "company_notice", "company_after");

    read(sim, "tally_slate", "slate_offer", [0], "quarry_camp");
    expect(sim.state.flags["been:quarry_top"]).toBeUndefined();
    read(sim, "tally_slate", "slate_wait");
    tp(sim, "quarry_top");
    expect(sim.state.flags["been:quarry_top"]).toBe(1);
    read(sim, "quarry_adit", "look");
    const spirit = p.spirit;
    read(sim, "tally_slate", "slate_in", [0], "quarry_camp");
    expectDone(sim, "down_at_five");
    expect(sim.state.growth.found).toContain("page_quarry");
    expect(p.spirit).toBe(spirit + 3);
    read(sim, "tally_slate", "slate_after");
  }, LONG);

  it("H. The Right of Way: stile to stile, and a haversack that is found, not offered", () => {
    const sim = newSim(SEEDS[1]);
    read(sim, "parish_board", "path_offer", [0], "town_square");
    read(sim, "parish_board", "path_wait");
    // Told once; then the board moves on to its next notice.
    read(sim, "parish_board", "rats_offer", [1]);
    read(sim, "fingerpost_town", "fingerpost_plain", [], "hedge_stile_town");
    expect(sim.state.flags["been:hedge_stile_town"]).toBe(1);
    expect(questReady(sim, "footpath_three")).toBe(false);
    const apples = has(sim, "apple");
    read(sim, "fingerpost_farm", "fingerpost_in", [0], "hedge_stile_farm");
    expectDone(sim, "footpath_three");
    expect(has(sim, "apple")).toBe(apples + 3);
    expect(sim.state.flags.footpath_walked).toBe(1);

    // Picking it up IS the offer, and it is already in hand.
    press(sim, "haversack_drop", "hedge_tree");
    expect(questReady(sim, "if_found")).toBe(true);
    read(sim, "lost_property_book", "haversack_in", [0], "start");
    expectDone(sim, "if_found");
    expect(has(sim, "haversack")).toBe(0);
    expect([has(sim, "hemshade_root"), has(sim, "small_water")]).toEqual([3, 2]);
    read(sim, "lost_property_book", "lp_offer", [1]);
  }, LONG);
});
