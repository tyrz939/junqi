// Castle, Castle Halt and the farmstead at Zelda scale (density brief, September 2026): a town the
// size of Minish Cap's, with as much in it; a halt, not a parade ground; and the three errands the
// town gives, played on foot by the headless player from the person who asks to the person who
// thanks you. "Every screen has something to press A on" is checked by counting, not by eye.

import { describe, expect, it } from "vitest";
import { focusOf } from "@/sim/interact";
import { bagCount } from "@/sim/inventory";
import { questActive, questDone } from "@/sim/quests";
import { Sim } from "@/sim/sim";
import { buildZone } from "@/world";
import type { Blueprint } from "@/world/blueprint";
import townQuests from "@/data/quests/town.json";
import townDialogue from "@/data/dialogue/town.json";
import townItems from "@/data/items/town.json";
import { idle, talkThrough, walkToProp, walkToUnit, yardCatalog } from "./bot";

const catalog = yardCatalog();
const SEEDS = [3, 2026];
const LONG = 120_000;

/** Everything within `r` cells of a mark. */
function near(bp: Blueprint, mark: string, rx: number, ry: number) {
  const m = bp.marks[mark];
  const inside = (x: number, y: number): boolean => Math.abs(x - m.cx) <= rx && Math.abs(y - m.cy) <= ry;
  return { props: bp.props.filter((p) => inside(p.cx, p.cy)), units: bp.units.filter((u) => inside(u.cx, u.cy)) };
}

describe("Castle is a town at Zelda scale", () => {
  it("fits in 100 x 70 cells and holds twenty-five buildings, twenty-five townsfolk, and something to read or knock on at every turn", () => {
    for (const seed of SEEDS) {
      const bp = buildZone("county", seed);
      // The square sits in the middle of the town: the whole town is within 60 x 40 of it.
      const town = near(bp, "town_square", 60, 40);
      const doors = town.props.filter((p) => p.def === "door" || p.def === "door_talk");
      expect(doors.length, `${seed}: doors in Castle`).toBeGreaterThanOrEqual(25);
      const folk = town.units.filter((u) => catalog.units[u.def]?.faction === "friendly" && catalog.units[u.def].sprite.startsWith("town_") && !/cat|hen|sheep/.test(u.def));
      expect(folk.length, `${seed}: townsfolk`).toBeGreaterThanOrEqual(25);
      // Every one of them has something to say, and so does every door.
      for (const u of folk) expect(catalog.dialogue[catalog.units[u.def].talk ?? ""], u.key).toBeDefined();
      for (const d of doors) expect(d.to ?? d.talk ?? d.key === "door_allen", `${d.key} opens or answers`).toBeTruthy();
      const readable = town.props.filter((p) => p.talk && catalog.props[p.def]?.prompt);
      expect(readable.length, `${seed}: things to read, knock on or look at`).toBeGreaterThanOrEqual(45);
      // Seventy-odd things to press USE on in a hundred by seventy: one every hundred square metres.
      expect(readable.length + folk.length, `${seed}: things and people`).toBeGreaterThanOrEqual(70);
      // The text says "in the yard of the Arms": the sheet is inside the yard's rails, not out on the street.
      const sheet = bp.props.find((p) => p.key === "washing_arms")!;
      const yard = bp.marks.arms_yard;
      expect(Math.abs(sheet.cx - yard.cx) <= 7 && sheet.cy - yard.cy <= 3, `${seed}: the sheet is in the Arms yard`).toBe(true);
      // Compact: the doors of the town span no more than a hundred cells by seventy.
      const xs = doors.map((d) => d.cx);
      const ys = doors.map((d) => d.cy);
      expect(Math.max(...xs) - Math.min(...xs)).toBeLessThanOrEqual(100);
      expect(Math.max(...ys) - Math.min(...ys)).toBeLessThanOrEqual(70);
    }
  }, LONG);

  it("the Halt is a halt: platform, shelter, name board, ticket window and lamps, nobody on it, in a box a screen and a half across", () => {
    for (const seed of SEEDS) {
      const bp = buildZone("county", seed);
      const halt = near(bp, "start", 36, 20);
      for (const def of ["town_shelter", "town_station_sign", "town_ticket_window", "town_park_bench", "campfire", "sign"]) {
        expect(halt.props.some((p) => p.def === def), `${seed}: ${def} at the Halt`).toBe(true);
      }
      expect(halt.props.filter((p) => p.def === "lamp_post").length).toBeGreaterThanOrEqual(3);
      // WORLD.md §3.2: the Halt is never given a person. The woman with the case waits in Castle, by the post office.
      expect(halt.units.some((u) => u.def.startsWith("town_")), `${seed}: a person at the Halt`).toBe(false);
      expect(near(bp, "post_office", 12, 8).units.some((u) => u.key === "traveller"), `${seed}: the traveller by the post office`).toBe(true);
      const plat = bp.rects.platform;
      expect(plat.w * plat.h, "the platform is a platform, not a parade ground").toBeLessThanOrEqual(260);
    }
  }, LONG);

  it("the farm is a farmstead: house, barn, hens, sheep in a pen, a field, hay and a cart", () => {
    const bp = buildZone("county", SEEDS[0]);
    const farm = near(bp, "farm_gate", 60, 25);
    for (const def of ["town_barn_doors", "town_hen_coop", "town_trough", "town_hay_bale", "town_farm_cart"]) expect(farm.props.some((p) => p.def === def), def).toBe(true);
    expect(farm.units.filter((u) => u.def === "town_sheep").length).toBeGreaterThanOrEqual(4);
    expect(farm.units.filter((u) => u.def.startsWith("town_hen")).length).toBeGreaterThanOrEqual(2);
  }, LONG);

  it("the text keeps to VOICE.md: no dashes, nobody's name baked in, short enough to read on a sign", () => {
    const text = JSON.stringify([townQuests, townDialogue, townItems]);
    expect(/[–—]/.test(text), "an en or em dash").toBe(false);
    expect(text.includes("Jane")).toBe(false);
    for (const [id, q] of Object.entries(townQuests)) expect(q.description.length, id).toBeLessThanOrEqual(320);
    for (const [id, tree] of Object.entries(catalog.dialogue)) {
      if (!(id in townDialogue)) continue;
      for (const node of Object.values(tree.nodes)) for (const line of node.lines) expect(line.length, `${id}: ${line}`).toBeLessThanOrEqual(200);
    }
  });
});

describe("the open country's people say different things (VOICE.md, People)", () => {
  it("every country folk tree turns over at least four lines, farmer to farmer, and comes back round", () => {
    const folk = Object.values(catalog.units).filter((u) => u.sprite.startsWith("folk_") && u.talk);
    expect(folk.length).toBeGreaterThanOrEqual(8);
    for (const u of folk) {
      const tree = catalog.dialogue[u.talk!];
      const flags: Record<string, number> = {};
      const heard: string[] = [];
      // Talk to twice as many of them as there are lines, applying each node's flag rows as the sim does.
      for (let n = 0; n < Object.keys(tree.nodes).length * 2; n++) {
        const row = tree.start.find((s) => (s.when ?? []).every((c) => c.if === "flag" && (flags[c.flag] ?? 0) === (c.eq ?? 1)))!;
        heard.push(row.node);
        for (const a of tree.nodes[row.node].actions ?? []) if (a.do === "flag") flags[a.flag] = a.value ?? 1;
      }
      const distinct = new Set(heard);
      expect(distinct.size, `${u.talk}: ${heard.join(" ")}`).toBeGreaterThanOrEqual(4);
      expect(distinct.size, `${u.talk} says every line it has`).toBe(Object.keys(tree.nodes).length);
      expect(heard[distinct.size], `${u.talk} comes back round`).toBe(heard[0]);
    }
    const text = JSON.stringify(Object.fromEntries(folk.map((u) => [u.talk, catalog.dialogue[u.talk!]])));
    expect(/[–—]/.test(text)).toBe(false);
    expect(text.includes("Jane")).toBe(false);
  });
});

// --- the errands, played ------------------------------------------------------------------

function newSim(seed: number): Sim {
  const sim = Sim.newGame(catalog, seed);
  sim.command({ t: "dev", dev: { op: "god", on: true } });
  sim.command({ t: "dev", dev: { op: "time", hour: 11 } });
  return sim;
}

function tp(sim: Sim, mark: string): void {
  expect(sim.rt.bp.marks[mark], `mark ${mark}`).toBeDefined();
  sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark } });
  idle(sim, 3);
}

/** Walk up to somebody, press USE, check who answers with what, and click through. */
function ask(sim: Sim, unit: string, node: string, choices: number[] = []): void {
  // Some of them are walking (Dot along Cross Lane, the Constable the length of the street): keep after them.
  for (let tries = 0; tries < 8; tries++) {
    const u = walkToUnit(sim, unit, 12);
    if (Math.hypot(u.x - sim.player.x, u.y - sim.player.y) <= 16) break;
  }
  sim.command({ t: "use" });
  idle(sim, 2);
  expect(sim.me.dialogue?.node, `${unit} answers with`).toBe(node);
  talkThrough(sim, choices);
  idle(sim, 1);
}

/** Walk up to a prop and USE it, and it, not anything else. */
function press(sim: Sim, key: string, node?: string, choices: number[] = []): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const f = focusOf(sim, sim.player);
  expect(f?.kind === "prop" && f.id === sim.rt.propsByKey.get(key)!.id, `USE acts on ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
  if (node) expect(sim.me.dialogue?.node, `${key} answers with`).toBe(node);
  talkThrough(sim, choices);
  idle(sim, 1);
}

describe("Castle's errands, walked", () => {
  it("Sixpence: Tilly by the fountain, the cat on the newest stone behind the church, and the cat home", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "fountain");
    ask(sim, "tilly", "offer", [0]);
    expect(questActive(sim, "sixpence")).toBeDefined();
    ask(sim, "tilly", "wait");
    tp(sim, "churchyard");
    ask(sim, "sixpence", "take");
    expect(sim.rt.unitsByKey.get("sixpence")).toBeUndefined();
    tp(sim, "fountain");
    ask(sim, "tilly", "in", [0]);
    expect(questDone(sim, "sixpence")).toBe(true);
    expect(sim.rt.unitsByKey.get("sixpence_home"), "Sixpence is home, by the fountain").toBeDefined();
    ask(sim, "tilly", "after");
  }, LONG);

  it("Second Post: three letters through three doors, the Forge, No. 4 Back Lane and the Doctor's, and a key for it", () => {
    const sim = newSim(SEEDS[1]);
    tp(sim, "post_office");
    ask(sim, "miss_dray", "offer", [0]);
    expect(bagCount(sim.player, "town_letter")).toBe(3);
    tp(sim, "forge");
    press(sim, "forge_door", "post");
    press(sim, "forge_door", "plain");
    tp(sim, "back_lane");
    press(sim, "door_back_4", "post");
    tp(sim, "doctor");
    press(sim, "doctors_door", "post");
    expect(bagCount(sim.player, "town_letter")).toBe(0);
    const keys = bagCount(sim.player, "key_generic");
    tp(sim, "post_office");
    ask(sim, "miss_dray", "in", [0]);
    expect(questDone(sim, "second_post")).toBe(true);
    expect(bagCount(sim.player, "key_generic")).toBe(keys + 1);
  }, LONG);

  it("Washing Day: a pillowcase by the church, a shirt by the fountain, a sheet in the Arms yard", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "washing_line");
    ask(sim, "mrs_marsh", "offer", [0]);
    for (const [mark, key] of [
      ["church_door", "washing_church"],
      ["fountain", "washing_fountain"],
      ["arms_yard", "washing_arms"],
    ] as const) {
      tp(sim, mark);
      press(sim, key);
    }
    expect(bagCount(sim.player, "town_washing")).toBe(3);
    tp(sim, "washing_line");
    const water = bagCount(sim.player, "small_water");
    ask(sim, "mrs_marsh", "in", [0]);
    expect(questDone(sim, "washing_day")).toBe(true);
    expect(bagCount(sim.player, "town_washing")).toBe(0);
    expect(bagCount(sim.player, "small_water")).toBe(water + 2);
  }, LONG);

  it("the stalls trade: Dr Vane fills an empty vial, Mrs Crewe swaps two pansies for a nasturtium, and says so when you have none", () => {
    const sim = newSim(SEEDS[0]);
    sim.command({ t: "dev", dev: { op: "give", item: "small_empty_vial", qty: 1 } });
    sim.command({ t: "dev", dev: { op: "give", item: "pansy", qty: 2 } });
    idle(sim, 1);
    const water = bagCount(sim.player, "small_water");
    // Dr Vane is in the lane in the afternoons (units/town.json: out from twelve), as his door says.
    sim.command({ t: "dev", dev: { op: "time", hour: 14 } });
    tp(sim, "doctor");
    ask(sim, "dr_vane", "a", [0]);
    expect(bagCount(sim.player, "small_empty_vial")).toBe(0);
    expect(bagCount(sim.player, "small_water")).toBe(water + 1);
    tp(sim, "cross_lane");
    tp(sim, "town_square");
    ask(sim, "mrs_crewe", "a", [0]);
    expect(bagCount(sim.player, "nasturtium")).toBeGreaterThanOrEqual(1);
    expect(bagCount(sim.player, "pansy")).toBe(0);
    ask(sim, "mrs_crewe", "a", [0]);
    expect(bagCount(sim.player, "pansy")).toBe(0);
  }, LONG);

  it("two doors on the square open: the Castle Arms (a bed at the back) and St Anne's, and both lead back out", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "arms_front");
    press(sim, "arms_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("arms");
    ask(sim, "mrs_garland", "a", [0]);
    expect(walkToProp(sim, "arms_bed")).toBe(true);
    press(sim, "exit_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("county");
    tp(sim, "church_door");
    press(sim, "church_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("church");
    press(sim, "visitors_book", "a");
    press(sim, "exit_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("county");
  }, LONG);

  it("after the lamps the street is empty, and the Arms is not", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "town_square");
    sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
    // Nobody vanishes in front of her: she walks away, and when she comes back they have gone in.
    tp(sim, "start");
    idle(sim, 5);
    tp(sim, "town_square");
    idle(sim, 5);
    for (const key of ["tilly", "miss_dray", "mrs_marsh", "constable"]) expect(sim.rt.unitsByKey.get(key)?.hidden, key).toBe(true);
    tp(sim, "arms_front");
    press(sim, "arms_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("arms");
    expect(sim.rt.unitsByKey.get("mrs_garland")?.hidden).toBe(false);
  }, LONG);
});

// --- the town's small stories (stories push, September 2026) ---------------------------------
//
// Short errands between Castle's own people and houses, and three chains with an ending that
// changes something she can go back and see: the new stone gets its chalked name, the Forge is
// warm, No. 7 puts its empty out, Mr Dunn stops waiting by the telephone.

describe("Castle's small stories, walked", () => {
  it("Hale, three deep: the last name on the memorial, Robert's cap on the Forge peg, the cap on the new stone", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "forge");
    ask(sim, "mr_hale", "offer", [0]);
    expect(questActive(sim, "the_last_name")).toBeDefined();
    tp(sim, "memorial");
    press(sim, "memorial", "hale");
    tp(sim, "forge");
    ask(sim, "mr_hale", "name_in", [0]);
    expect(questDone(sim, "the_last_name")).toBe(true);
    ask(sim, "mr_hale", "cap_offer", [0]);
    press(sim, "forge_door", "cap");
    expect(bagCount(sim.player, "town_cap")).toBe(1);
    ask(sim, "mr_hale", "cap_in", [0]);
    expect(questDone(sim, "roberts_cap")).toBe(true);
    expect(bagCount(sim.player, "town_cap"), "he gives it back to hold").toBe(1);
    ask(sim, "mr_hale", "stone_offer", [0]);
    tp(sim, "churchyard");
    press(sim, "headstone_4", "cap");
    expect(bagCount(sim.player, "town_cap")).toBe(0);
    tp(sim, "forge");
    const fire = bagCount(sim.player, "fire_stone");
    ask(sim, "mr_hale", "stone_in", [0]);
    expect(questDone(sim, "the_new_stone")).toBe(true);
    expect(bagCount(sim.player, "fire_stone")).toBe(fire + 1);
    ask(sim, "mr_hale", "after");
    // The ending shows: the Forge is warm, and the stone has a name chalked on it.
    press(sim, "forge_door", "warm");
    tp(sim, "churchyard");
    press(sim, "headstone_4", "named");
  }, LONG);

  it("Dunn, two deep: the telephone box rings for her, and Mrs Garland in the Arms knows where Ernest is", () => {
    const sim = newSim(SEEDS[1]);
    tp(sim, "phone_box");
    ask(sim, "mr_dunn", "offer", [0]);
    press(sim, "phone_box", "ring");
    ask(sim, "mr_dunn", "hour_in", [0]);
    expect(questDone(sim, "on_the_hour")).toBe(true);
    ask(sim, "mr_dunn", "room_offer", [0]);
    tp(sim, "arms_front");
    press(sim, "arms_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("arms");
    ask(sim, "mrs_garland", "ernest");
    press(sim, "exit_door");
    idle(sim, 5);
    expect(sim.me.zone).toBe("county");
    tp(sim, "phone_box");
    ask(sim, "mr_dunn", "room_in", [0]);
    expect(questDone(sim, "the_back_room")).toBe(true);
    ask(sim, "mr_dunn", "after");
  }, LONG);

  it("Dot, two deep: three lane signs at the west end, then the painted-out one at the top of Church Lane", () => {
    const sim = newSim(SEEDS[0]);
    tp(sim, "cross_lane");
    ask(sim, "dot", "offer", [0]);
    for (const key of ["sign_back_lane", "sign_cross_lane", "sign_pound_lane"]) press(sim, key, "count");
    ask(sim, "dot", "count_in", [0]);
    expect(questDone(sim, "the_fourth_lane")).toBe(true);
    ask(sim, "dot", "lane_offer", [0]);
    tp(sim, "church_lane_top");
    press(sim, "sign_church_lane", "read");
    const keys = bagCount(sim.player, "key_generic");
    ask(sim, "dot", "lane_in", [0]);
    expect(questDone(sim, "school_lane")).toBe(true);
    expect(bagCount(sim.player, "key_generic")).toBe(keys + 1);
    ask(sim, "dot", "after");
  }, LONG);

  it("No. 7, Pound Lane, two deep: Mrs Oddie's loaf on the step, then the Milkman's bottles inside the door", () => {
    const sim = newSim(SEEDS[1]);
    // The Milkman is on his round from seven to nine, as he says; Mrs Oddie's stall is out from seven.
    sim.command({ t: "dev", dev: { op: "time", hour: 8 } });
    tp(sim, "town_square");
    ask(sim, "mrs_oddie", "offer", [0]);
    expect(bagCount(sim.player, "town_loaf")).toBe(1);
    tp(sim, "doctor");
    press(sim, "door_pound_7", "loaf");
    expect(bagCount(sim.player, "town_loaf")).toBe(0);
    tp(sim, "town_square");
    ask(sim, "mrs_oddie", "in", [0]);
    expect(questDone(sim, "two_loaves")).toBe(true);
    tp(sim, "doctor");
    ask(sim, "milkman", "offer", [0]);
    press(sim, "door_pound_7", "milk");
    ask(sim, "milkman", "in", [0]);
    expect(questDone(sim, "paid_to_sunday")).toBe(true);
    press(sim, "door_pound_7", "rinsed");
  }, LONG);

  it("the short errands: Mrs Bex's eggs, Mr Sallis's rose, the Constable's paces, Mrs Hobb's apples for Mr Tolly", () => {
    const sim = newSim(SEEDS[0]);
    // Never Any Eggs: the hen house behind Pound Lane.
    tp(sim, "town_square");
    ask(sim, "mrs_bex", "offer", [0]);
    tp(sim, "pound_hens");
    press(sim, "pound_hens", "eggs");
    ask(sim, "mrs_bex", "in", [0]);
    expect(questDone(sim, "never_any_eggs")).toBe(true);
    // White Roses: the first stone inside the churchyard gate.
    tp(sim, "street_west");
    ask(sim, "mr_sallis", "offer", [0]);
    tp(sim, "churchyard");
    press(sim, "headstone_5", "rose");
    ask(sim, "mr_sallis", "in", [0]);
    expect(questDone(sim, "white_roses")).toBe(true);
    // A Hundred and Twelve: the street end to end, lamps to lamps. Walking it before she is asked counts for nothing.
    tp(sim, "town_square");
    ask(sim, "constable", "first");
    tp(sim, "street_west");
    expect(sim.state.flags["been:street_west"]).toBeUndefined();
    tp(sim, "town_square");
    ask(sim, "constable", "offer", [0]);
    tp(sim, "street_west");
    expect(sim.state.flags["been:street_west"]).toBe(1);
    tp(sim, "street_east");
    expect(sim.state.flags["been:street_east"]).toBe(1);
    ask(sim, "constable", "in", [0]);
    expect(questDone(sim, "the_constables_paces")).toBe(true);
    // Too Red: two apples off the orchard behind No. 7 High Street, to Mr Tolly on the memorial bench.
    tp(sim, "street_east");
    ask(sim, "mrs_hobb", "offer", [0]);
    tp(sim, "orchard");
    const orchard = sim.rt.bp.marks.orchard;
    const tree = sim.rt.bp.props.filter((p) => p.def === "apple_tree" && Math.abs(p.cx - orchard.cx) < 8 && p.cy < orchard.cy).sort((a, b) => Math.abs(a.cx - orchard.cx) - Math.abs(b.cx - orchard.cx))[0];
    const apples = bagCount(sim.player, "apple");
    press(sim, tree.key!);
    expect(bagCount(sim.player, "apple")).toBeGreaterThanOrEqual(Math.max(2, apples));
    // From the square side, so she faces him and not the memorial behind him.
    tp(sim, "town_square");
    const before = bagCount(sim.player, "apple");
    ask(sim, "mr_tolly", "apples", [0]);
    expect(questDone(sim, "too_red")).toBe(true);
    expect(bagCount(sim.player, "apple")).toBe(before - 2);
    ask(sim, "mr_tolly", "after");
  }, LONG);

});
