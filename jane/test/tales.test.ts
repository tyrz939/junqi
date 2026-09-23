// The tales (QUEST-TREE.md "Tales"; data/stories/tales.json). A handful of stories told at the generated
// places, each with somebody at it who is plainly more than a neighbour, two or three steps deep, with a
// turn in the middle and, most of them, a choice at the end that leaves something changed in the world.
//
// Three kinds of proof:
//  1. The data: how many, how deep, the words kept to VOICE.md, every name its own.
//  2. The county: on seeds 1 to 20 every tale lands, and every thing it puts down stands where she can
//     walk to it. The rate is printed; the bar is every seed.
//  3. Played: each tale end to end by the headless player on two seeds, one branch of its choice on each,
//     with the physical verbs done as a person does them (the stone pushed, the stones carried, the man
//     followed after the bell), and what the ending changes checked in the world.
//
// The quest audit (quest-audit.test.ts) holds every tale quest to the same rules A to E as every other.

import { describe, expect, it } from "vitest";
import type { Catalog } from "@/sim/catalog";
import { centre, F_SOLID, TILE_FLAGS } from "@/sim/grid";
import { focusOf } from "@/sim/interact";
import { bagCount } from "@/sim/inventory";
import { questActive, questDone } from "@/sim/quests";
import { Sim } from "@/sim/sim";
import type { Unit } from "@/sim/state";
import { expandText } from "@/sim/text";
import { blueprintFor } from "@/sim/zones";
import type { Blueprint } from "@/world/blueprint";
import { allNames, STORIES } from "@/world/names";
import { PLACEMENTS } from "@/world/placements";
import talesQuests from "@/data/quests/tales.json";
import talesDialogue from "@/data/dialogue/tales.json";
import talesItems from "@/data/items/tales.json";
import talesPlacements from "@/data/placements/tales.json";
import talesStories from "@/data/stories/tales.json";
import { face, idle, talkThrough, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = yardCatalog();
const TALES = STORIES.filter((s) => s.tale);
const LONG = 900_000;
/** Seeds 1 to 20, and the three every other test plays or nobody has tuned against. */
const ALL_SEEDS = [...Array.from({ length: 20 }, (_, i) => i + 1), 2026, 77, 123];
const KINDS = ["hamlet", "farmstead", "cottage", "inn", "woodcutter", "camp", "ruin"];

// --- 1. the data ---------------------------------------------------------------------------------

describe("the tales, as data", () => {
  it("eight to twelve of them, each two or three steps deep, every one with a fixed name of its own", () => {
    expect(TALES.length).toBeGreaterThanOrEqual(8);
    expect(TALES.length).toBeLessThanOrEqual(12);
    expect(TALES.length).toBe((talesStories as unknown[]).length);
    for (const t of TALES) {
      expect(t.name, t.id).toBeTruthy();
      expect(t.quests.length, t.id).toBeGreaterThanOrEqual(1);
      expect(t.quests.length, t.id).toBeLessThanOrEqual(3);
      // Two or three steps: a quest with two steps, or two or three quests in a row.
      const steps = t.quests.reduce((n, q) => n + catalog.quests[q].requirements.length, 0);
      expect(steps, `${t.id}: steps`).toBeGreaterThanOrEqual(t.quests.length === 1 ? 1 : 2);
      for (const q of t.quests) expect(Object.keys(talesQuests)).toContain(q);
    }
    // Spread over the county: the Lowfields, the Waters and the Works.
    const regions = new Set(TALES.map((t) => t.region ?? "lowfields"));
    expect([...regions].sort()).toEqual(["lowfields", "waters", "works"]);
  });

  it("the names are their own: no two alike, none the same as or inside any name a seed paints on a board", () => {
    const names = TALES.map((t) => t.name!);
    expect(new Set(names).size).toBe(names.length);
    const boards = KINDS.flatMap((k) => allNames(k));
    for (const n of names) {
      for (const b of boards) {
        expect(b.toLowerCase().includes(n.toLowerCase()), `"${n}" is inside "${b}"`).toBe(false);
        expect(n.toLowerCase().includes(b.toLowerCase()), `"${b}" is inside "${n}"`).toBe(false);
      }
    }
  });

  it("the words keep to VOICE.md: short enough for the tracker, no dashes, nobody's name baked in", () => {
    const say = (t: string): string => expandText({ name: "Jane", seed: 3 }, t);
    for (const t of TALES) {
      for (const id of t.quests) {
        const q = catalog.quests[id] as Catalog["quests"][string] & { returnTo?: string };
        for (const r of q.requirements) {
          expect(say(r.text).length, `${id}: "${say(r.text)}"`).toBeLessThanOrEqual(70);
          expect(r.text, `${id}: a step that names no place`).toMatch(/\{place:[a-z_]+\}/);
        }
        expect(say(q.description).length, `${id}: description`).toBeLessThanOrEqual(320);
        expect(say(q.returnTo ?? "").length, `${id}: returnTo`).toBeLessThanOrEqual(70);
      }
    }
    const text = JSON.stringify([talesQuests, talesDialogue, talesItems, talesPlacements, talesStories]);
    expect(/[–—]/.test(text), "an en or em dash").toBe(false);
    expect(text.includes("Jane")).toBe(false);
    for (const m of text.matchAll(/\{place:([a-z0-9_]+)\}/g)) expect(STORIES.some((s) => s.id === m[1]), `{place:${m[1]}}`).toBe(true);
  });

  it("most turn on a thing done with the hands, not a thing fetched: pushed, carried, followed, lit, waited for after the bell", () => {
    const rows = (id: string) => PLACEMENTS.filter((r) => r.at.place === id);
    const trees = JSON.stringify(talesDialogue);
    const verbs = new Map<string, string[]>();
    for (const t of TALES) {
      const found: string[] = [];
      const defs = rows(t.id).map((r) => r.prop?.def ?? "");
      if (defs.includes("tale_stone") || defs.includes("barrel")) found.push("push");
      if (defs.includes("tale_slab")) found.push("carry");
      if (defs.some((d) => d.startsWith("tale_signal"))) found.push("light");
      if (t.quests.some((q) => catalog.quests[q].requirements.some((r) => /after the bell/.test(r.text)))) found.push("wait for night");
      if (rows(t.id).some((r) => r.unit && trees.includes(`"do":"send","unit":"${r.key}"`))) found.push("follow");
      if (defs.includes("tale_dandelion") || defs.includes("tale_scarf_run")) found.push("follow a trail");
      verbs.set(t.id, found);
    }
    const physical = [...verbs.values()].filter((v) => v.length > 0).length;
    expect(physical, JSON.stringify([...verbs])).toBeGreaterThanOrEqual(Math.ceil(TALES.length / 2));
  });
});

// --- 2. the county ----------------------------------------------------------------------------------

/** The keys a tale's rows put down (props, units, marks), whatever the seed. */
function thingsOf(id: string): { props: string[]; units: string[]; marks: string[] } {
  const out = { props: [] as string[], units: [] as string[], marks: [] as string[] };
  for (const r of PLACEMENTS) {
    if (r.at.place !== id || r.edit) continue;
    if (r.prop) out.props.push(r.key);
    if (r.hides) out.props.push(r.hides.key);
    if (r.unit) out.units.push(r.key);
    if (r.mark) out.marks.push(r.mark);
  }
  return out;
}

/** Cells she can walk to from where she starts: the ground, less every solid thing standing (hidden things stand nowhere). */
function flood(bp: Blueprint): Uint8Array {
  const w = bp.w;
  const seen = new Uint8Array(w * bp.h);
  const blocked = new Uint8Array(w * bp.h);
  for (const p of bp.props) {
    const d = catalog.props[p.def];
    if (!d?.solid || p.hidden) continue;
    for (let j = p.cy; j < p.cy + d.h; j++) for (let i = p.cx; i < p.cx + d.w; i++) blocked[j * w + i] = 1;
  }
  const start = bp.marks.start;
  const queue = new Int32Array(w * bp.h);
  let head = 0;
  let tail = 0;
  const push = (i: number): void => {
    if (seen[i] || blocked[i] || (TILE_FLAGS[bp.tiles[i]] & F_SOLID) !== 0) return;
    seen[i] = 1;
    queue[tail++] = i;
  };
  push(start.cy * w + start.cx);
  while (head < tail) {
    const i = queue[head++];
    const x = i % w;
    if (x + 1 < w) push(i + 1);
    if (x > 0) push(i - 1);
    if (i + w < seen.length) push(i + w);
    if (i >= w) push(i - w);
  }
  return seen;
}

describe("the tales, on seeds 1 to 20", () => {
  it("every tale finds its place, puts down everything it needs, and all of it can be walked to", () => {
    const landed = new Map<string, number>();
    const problems: string[] = [];
    const SEEDS = ALL_SEEDS;
    for (const seed of SEEDS) {
      const bp = blueprintFor("county", seed);
      const seen = flood(bp);
      const near = (cx: number, cy: number, w: number, h: number): boolean => {
        for (let y = cy - 1; y <= cy + h; y++) for (let x = cx - 1; x <= cx + w; x++) if (seen[y * bp.w + x]) return true;
        return false;
      };
      for (const t of TALES) {
        const rec = bp.stories?.[t.id];
        if (!rec || !("box" in rec)) {
          problems.push(`seed ${seed}: ${t.id} has no place (${rec && "skipped" in rec ? rec.skipped : "no record"})`);
          continue;
        }
        const want = thingsOf(t.id);
        let whole = true;
        for (const key of want.props) {
          const p = bp.props.find((q) => q.key === key);
          const d = p && catalog.props[p.def];
          if (!p || !d) {
            problems.push(`seed ${seed}: ${t.id}: ${key} was not put down`);
            whole = false;
          } else if (!near(p.cx, p.cy, d.w, d.h)) {
            problems.push(`seed ${seed}: ${t.id}: ${key} cannot be walked to`);
            whole = false;
          }
        }
        for (const key of want.units) {
          const u = bp.units.find((q) => q.key === key);
          if (!u || !near(u.cx, u.cy, 1, 1)) {
            problems.push(`seed ${seed}: ${t.id}: ${key} ${u ? "cannot be walked to" : "was not stood"}`);
            whole = false;
          }
        }
        for (const m of want.marks) {
          const at = bp.marks[m];
          if (!at || !seen[at.cy * bp.w + at.cx]) {
            problems.push(`seed ${seed}: ${t.id}: mark ${m} ${at ? "cannot be walked to" : "missing"}`);
            whole = false;
          }
        }
        if (whole) landed.set(t.id, (landed.get(t.id) ?? 0) + 1);
      }
    }
    const rate = TALES.map((t) => `${t.id} ${landed.get(t.id) ?? 0}/${SEEDS.length}`).join(", ");
    console.log(`tales landed whole: ${rate}`);
    expect(problems, problems.join("\n")).toEqual([]);
  }, LONG);
});

// --- 3. played ------------------------------------------------------------------------------------

function newGame(seed: number): Sim {
  const sim = Sim.newGame(catalog, seed);
  sim.command({ t: "dev", dev: { op: "god", on: true } });
  return sim;
}

/** Set the clock, having stepped away first so that nobody is kept from coming or going by being looked at. */
function at(sim: Sim, hour: number, mark: string): void {
  sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark: "start" } });
  sim.command({ t: "dev", dev: { op: "time", hour } });
  idle(sim, 70);
  sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark } });
  idle(sim, 3);
}

/** Let the clock strike an hour (the county's own rows run on the hour: data/clock/tales.json). */
function strike(sim: Sim, hour: number, mark: string): void {
  at(sim, hour - 4 / 7200, mark);
  idle(sim, 40);
}

function useProp(sim: Sim, key: string, choices: number[] = []): void {
  const prop = sim.rt.propsByKey.get(key)!;
  const aimed = (): boolean => {
    const f = focusOf(sim, sim.player);
    return f?.kind === "prop" && f.id === prop.id;
  };
  expect(walkToProp(sim, key, aimed), `walk to ${key}`).toBe(true);
  const f = focusOf(sim, sim.player);
  expect(f?.kind === "prop" && f.id === prop.id, `USE beside ${key} acts on it, not ${JSON.stringify(f)}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
  talkThrough(sim, choices);
  idle(sim, 2);
}

function talkTo(sim: Sim, key: string, choices: number[] = []): void {
  const u = sim.rt.unitsByKey.get(key);
  expect(u, `${key} is there`).toBeDefined();
  expect(u!.hidden, `${key} is not away`).toBe(false);
  let ok = false;
  for (const [ox, oy] of [[0, 10], [10, 0], [-10, 0], [0, -10], [8, 8], [-8, 8], [8, -8], [-8, -8], [0, 16], [16, 0], [-16, 0], [0, -16], [0, 0]]) {
    if (!walkTo(sim, u!.x + ox, u!.y + oy, 4)) continue;
    face(sim, u!.x, u!.y);
    const f = focusOf(sim, sim.player);
    if (f?.kind === "unit" && f.id === u!.id) {
      ok = true;
      break;
    }
  }
  expect(ok, `USE reaches ${key}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
  expect(sim.me.dialogue, `${key} answers`).not.toBeNull();
  talkThrough(sim, choices);
  idle(sim, 2);
}

/** Push a prop one cell (dx, dy), from the cell behind it, holding USE and leaning, as a person does. */
function push(sim: Sim, key: string, dx: number, dy: number): void {
  const p = sim.rt.propsByKey.get(key)!;
  const d = sim.catalog.props[p.def];
  const sx = dx > 0 ? p.cx - 1 : dx < 0 ? p.cx + d.w : p.cx;
  const sy = dy > 0 ? p.cy - 1 : dy < 0 ? p.cy + d.h : p.cy;
  expect(walkTo(sim, centre(sx), centre(sy), 2), `stand behind ${key}`).toBe(true);
  face(sim, centre(p.cx), centre(p.cy));
  const was = [p.cx, p.cy];
  for (let t = 0; t < 120 && p.cx === was[0] && p.cy === was[1]; t++) {
    sim.player.energy = 100;
    sim.tick({ mx: dx, my: dy, sprint: false, useHeld: true, ax: 0, ay: 0 });
  }
  expect([p.cx, p.cy], `${key} pushed`).toEqual([was[0] + dx, was[1] + dy]);
  // Step back off whatever it was on, and let the plates notice.
  walkTo(sim, centre(sx - dx * 2), centre(sy - dy * 2), 4);
  idle(sim, 12);
}

/** Pick up a stone from the heap (whichever is nearest to hand) and set it down on a cell, from the cell above it, facing down. */
function carry(sim: Sim, to: [number, number]): void {
  const heap = [...sim.rt.propsByKey.values()].filter((p) => p.def === "tale_slab" && !(p.cy === to[1] && Math.abs(p.cx - to[0]) <= 2));
  expect(heap.length, "a stone left on the heap").toBeGreaterThan(0);
  expect(walkToProp(sim, heap[0].key), "walk to the heap").toBe(true);
  const f = focusOf(sim, sim.player);
  const p = f?.kind === "prop" ? [...sim.rt.propsByKey.values()].find((q) => q.id === f.id) : undefined;
  expect(p?.def, "USE at the heap picks up a stone").toBe("tale_slab");
  sim.command({ t: "use" });
  idle(sim, 2);
  expect(sim.player.carrying, "carrying it").toBe(p!.id);
  expect(walkTo(sim, centre(to[0]), centre(to[1] - 1), 2), "stand above the gap").toBe(true);
  for (let t = 0; t < 3; t++) sim.tick({ mx: 0, my: 0.2, sprint: false, useHeld: false, ax: 0, ay: 0 });
  sim.command({ t: "use" });
  idle(sim, 12);
  expect([p!.cx, p!.cy], "set down in the gap").toEqual(to);
  walkTo(sim, centre(to[0]), centre(to[1] - 3), 4);
  idle(sim, 12);
}

const bag = (sim: Sim, item: string): number => bagCount(sim.player, item);
const flag = (sim: Sim, f: string): number => sim.state.flags[f] ?? 0;
const unit = (sim: Sim, key: string): Unit | undefined => sim.rt.unitsByKey.get(key);
const mark = (id: string): string => `story_${id}`;

type Branch = "a" | "b";
const PLAYS: Record<string, (sim: Sim, b: Branch) => void> = {
  tale_bees(sim, b) {
    at(sim, 10, mark("tale_bees"));
    talkTo(sim, "tale_bees_hanney");
    for (const n of [1, 2, 3]) useProp(sim, `tale_bees_hive_${n}`);
    expect(flag(sim, "been:tale_bees_told")).toBe(1);
    talkTo(sim, "tale_bees_hanney");
    expect(questDone(sim, "tale_bees_tell")).toBe(true);
    talkTo(sim, "tale_bees_hanney");
    at(sim, 22, mark("tale_bees"));
    expect(unit(sim, "tale_bees_hanney")!.hidden, "Miss Hanney is not out after the bell").toBe(true);
    talkTo(sim, "tale_bees_coram");
    at(sim, 10, mark("tale_bees"));
    talkTo(sim, "tale_bees_hanney", [b === "a" ? 0 : 1]);
    expect(questDone(sim, "tale_bees_comb")).toBe(true);
    if (b === "a") {
      for (const n of [1, 2, 3]) useProp(sim, `tale_bees_hive_${n}`);
      for (const n of [1, 2, 3]) expect(sim.rt.propsByKey.get(`tale_bees_hive_${n}`)!.on, "crepe round the hive").toBe(true);
      talkTo(sim, "tale_bees_hanney");
      expect(questDone(sim, "tale_bees_crepe")).toBe(true);
      expect(unit(sim, "tale_bees_coram"), "nobody at the hives after the bell now").toBeUndefined();
      expect(sim.rt.propsByKey.get("tale_bees_veil")!.hidden, "his veil taken in").toBe(true);
    } else {
      expect(sim.rt.propsByKey.get("tale_bees_jar")!.hidden, "no jar yet").toBe(true);
      strike(sim, 6, mark("tale_bees"));
      expect(sim.rt.propsByKey.get("tale_bees_jar")!.hidden, "a jar on the step by morning").toBe(false);
      at(sim, 22, mark("tale_bees"));
      expect(unit(sim, "tale_bees_coram")!.hidden, "and he is still at his hives").toBe(false);
    }
  },

  tale_walk(sim, b) {
    at(sim, 10, mark("tale_walk"));
    talkTo(sim, "tale_walk_wife");
    expect(questActive(sim, "tale_walk_follow")).toBeTruthy();
    at(sim, 21.5, mark("tale_walk"));
    talkTo(sim, "tale_walk_pollard");
    // He goes, and she follows him: to the stone north of the house.
    const stone = sim.rt.bp.marks.tale_walk_stone_mark;
    for (let t = 0; t < 60 * 90 && flag(sim, "tale_walk_arrived") === 0; t++) {
      const u = unit(sim, "tale_walk_pollard");
      if (u && t % 20 === 0) walkTo(sim, u.x, u.y + 16, 8, 20);
      else idle(sim, 1);
    }
    expect(flag(sim, "tale_walk_arrived"), "he got to the stone").toBe(1);
    expect(stone.cy, "the stone is north of the house").toBeLessThan(sim.rt.bp.marks[mark("tale_walk")].cy);
    expect(sim.rt.propsByKey.get("tale_walk_stone")!.on, "a dinner on the stone").toBe(true);
    // He stands at the stone: asked, he says what it is for.
    talkTo(sim, "tale_walk_pollard");
    expect(flag(sim, "been:tale_walk_seen")).toBe(1);
    at(sim, 10, mark("tale_walk"));
    talkTo(sim, "tale_walk_wife", [b === "a" ? 0 : 1]);
    expect(questDone(sim, "tale_walk_follow")).toBe(true);
    if (b === "a") {
      useProp(sim, "tale_walk_stone", [0]);
      expect(bag(sim, "tale_walk_plate")).toBe(1);
      talkTo(sim, "tale_walk_wife");
      expect(questDone(sim, "tale_walk_plate")).toBe(true);
      strike(sim, 21, mark("tale_walk"));
      expect(unit(sim, "tale_walk_pollard"), "he does not walk now").toBeUndefined();
      expect(unit(sim, "tale_walk_pollard_home"), "he sits by his door").toBeDefined();
      expect(sim.rt.propsByKey.get("tale_walk_stone")!.on, "the stone is bare").toBe(false);
    } else {
      strike(sim, 6, mark("tale_walk"));
      expect(sim.rt.propsByKey.get("tale_walk_second")!.hidden, "a second plate on the stone").toBe(false);
    }
  },

  tale_horace(sim) {
    at(sim, 10, mark("tale_horace"));
    talkTo(sim, "tale_horace_pargeter");
    const house = sim.rt.bp.marks[mark("tale_horace")];
    const found = sim.rt.propsByKey.get("tale_horace_found")!;
    expect(found.cy, "Horace is north of the house").toBeLessThan(house.cy);
    // The bitten dandelions lead from the house to him.
    for (const n of [1, 2, 3, 4]) useProp(sim, `tale_horace_bite_${n}`);
    useProp(sim, "tale_horace_found");
    expect(bag(sim, "tale_horace")).toBe(1);
    talkTo(sim, "tale_horace_pargeter");
    expect(questDone(sim, "tale_horace_find")).toBe(true);
    talkTo(sim, "tale_horace_pargeter");
    // The flat stone, pushed up against the front of the box: left, onto the bare ground there.
    push(sim, "tale_horace_stone", -1, 0);
    expect(flag(sim, "tale_horace_blocked"), "the stone is against the box").toBe(1);
    talkTo(sim, "tale_horace_pargeter");
    expect(questDone(sim, "tale_horace_box")).toBe(true);
    expect(sim.rt.propsByKey.get("tale_horace_hutch")!.on, "Horace in his box").toBe(true);
  },

  tale_wall(sim, b) {
    at(sim, 10, mark("tale_wall"));
    talkTo(sim, "tale_wall_coker");
    const first = sim.rt.propsByKey.get("tale_wall_gap_1")!;
    for (const n of [1, 2, 3]) carry(sim, [first.cx + n - 1, first.cy]);
    expect(flag(sim, "tale_wall_gap"), "three stones in the gap").toBe(3);
    talkTo(sim, "tale_wall_coker", [b === "a" ? 0 : 1]);
    expect(questDone(sim, "tale_wall_stones")).toBe(true);
    if (b === "b") {
      for (const n of [1, 2, 3]) expect(sim.rt.propsByKey.get(`tale_wall_stood_${n}`)!.hidden, "the headstone stood up").toBe(false);
      for (const n of [1, 2, 3]) expect(sim.rt.propsByKey.get(`tale_wall_slab_${n}`)!.hidden, "out of the wall").toBe(true);
    } else {
      for (const n of [1, 2, 3]) expect(sim.rt.propsByKey.get(`tale_wall_stood_${n}`)!.hidden).toBe(true);
    }
  },

  tale_boards(sim, b) {
    at(sim, 10, mark("tale_boards"));
    talkTo(sim, "tale_boards_rook");
    useProp(sim, "tale_boards_back");
    talkTo(sim, "tale_boards_rook");
    expect(questDone(sim, "tale_boards_back")).toBe(true);
    useProp(sim, "tale_boards_boots", [0]);
    expect(questActive(sim, "tale_boards_door")).toBeTruthy();
    at(sim, 22, mark("tale_boards"));
    useProp(sim, "tale_boards_house", [b === "a" ? 0 : 1]);
    expect(questDone(sim, "tale_boards_door")).toBe(true);
    strike(sim, 6, mark("tale_boards"));
    at(sim, 10, mark("tale_boards"));
    if (b === "a") {
      expect(sim.rt.propsByKey.get("tale_boards_house")!.on, "the boards are off").toBe(true);
      expect(unit(sim, "tale_boards_rook"), "he has gone").toBeUndefined();
      expect(sim.rt.propsByKey.get("tale_boards_boots")!.hidden, "and his boots").toBe(true);
      talkTo(sim, "tale_boards_mary");
    } else {
      expect(sim.rt.propsByKey.get("tale_boards_new")!.hidden, "new planks by the door").toBe(false);
      talkTo(sim, "tale_boards_rook");
    }
  },

  tale_diver(sim, b) {
    at(sim, 10, mark("tale_diver"));
    talkTo(sim, "tale_diver_denholm");
    expect(sim.rt.propsByKey.get("tale_diver_key_found")!.hidden, "the key is under the stone").toBe(true);
    // Shove it over: whichever way there is room.
    const stone = sim.rt.propsByKey.get("tale_diver_stone")!;
    const g = sim.rt.grid;
    const way = ([[1, 0], [-1, 0], [0, 1], [0, -1]] as const).find(([dx, dy]) => g.free(stone.cx + dx, stone.cy + dy, 0) && g.free(stone.cx - dx, stone.cy - dy, sim.player.id));
    expect(way, "room to push the stone").toBeDefined();
    push(sim, "tale_diver_stone", way![0], way![1]);
    expect(sim.rt.propsByKey.get("tale_diver_key_found")!.hidden, "a key where the stone was").toBe(false);
    useProp(sim, "tale_diver_key_found");
    expect(bag(sim, "tale_diver_key")).toBe(1);
    useProp(sim, "tale_diver_chest");
    useProp(sim, "tale_diver_chest");
    expect(bag(sim, "tale_spanner")).toBe(1);
    talkTo(sim, "tale_diver_denholm", [b === "a" ? 0 : 1]);
    expect(questDone(sim, "tale_diver_spanner")).toBe(true);
    if (b === "a") {
      expect(sim.rt.propsByKey.get("tale_diver_hose_1")!.hidden, "the hose runs off").toBe(false);
      idle(sim, 60 * 40);
      expect(unit(sim, "tale_diver_denholm"), "he has gone down").toBeUndefined();
    } else {
      expect(unit(sim, "tale_diver_sitting"), "he sits, bareheaded").toBeDefined();
      expect(sim.rt.propsByKey.get("tale_diver_helmet")!.hidden, "the helmet by the step").toBe(false);
    }
  },

  tale_scarf(sim, b) {
    at(sim, 10, mark("tale_scarf"));
    talkTo(sim, "tale_scarf_wakes");
    useProp(sim, "tale_scarf_lap");
    talkTo(sim, "tale_scarf_wakes");
    expect(questDone(sim, "tale_scarf_rows")).toBe(true);
    useProp(sim, "tale_scarf_lap", [b === "a" ? 0 : 1]);
    talkTo(sim, "tale_scarf_wakes");
    expect(questDone(sim, "tale_scarf_row")).toBe(true);
    if (b === "a") expect(bag(sim, "tale_scarf"), "a scarf with her name in it").toBe(1);
    else expect(sim.rt.propsByKey.get("tale_scarf_tangle")!.hidden, "the pulled row by the chair").toBe(false);
  },

  tale_halt(sim, b) {
    at(sim, 10, mark("tale_halt"));
    talkTo(sim, "tale_halt_voysey");
    push(sim, "tale_halt_barrel", 1, 0);
    expect(sim.rt.propsByKey.get("tale_halt_locker")!.locked, "the locker opens once the barrel is off it").toBe(false);
    useProp(sim, "tale_halt_locker");
    expect(bag(sim, "tale_lamp_oil")).toBe(1);
    talkTo(sim, "tale_halt_wife");
    talkTo(sim, "tale_halt_voysey");
    expect(questActive(sim, "tale_halt_lamp")).toBeTruthy();
    at(sim, 22, mark("tale_halt"));
    const lamp = b === "a" ? "tale_halt_red" : "tale_halt_green";
    useProp(sim, lamp, [0]);
    expect(sim.rt.propsByKey.get(lamp)!.on, "the lamp is lit").toBe(true);
    expect(bag(sim, "tale_lamp_oil"), "and the oil is gone into it").toBe(0);
    at(sim, 10, mark("tale_halt"));
    talkTo(sim, "tale_halt_voysey");
    expect(questDone(sim, "tale_halt_lamp")).toBe(true);
    if (b === "a") expect(unit(sim, "tale_halt_wife"), "she is still waiting").toBeDefined();
    else expect(unit(sim, "tale_halt_wife"), "she has gone").toBeUndefined();
  },

  tale_vermin(sim, b) {
    at(sim, 10, mark("tale_vermin"));
    talkTo(sim, "tale_vermin_sorrell");
    const spinners = [...sim.rt.unitsByKey.values()].filter((u) => u.def === "tale_spinner" && u.alive);
    expect(spinners.length).toBeGreaterThanOrEqual(5);
    for (const u of spinners.slice(0, 4)) {
      walkTo(sim, u.x, u.y, 20);
      u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
      idle(sim, 2);
    }
    talkTo(sim, "tale_vermin_sorrell", [0]);
    expect(questDone(sim, "tale_vermin_clear")).toBe(true);
    expect(bag(sim, "tale_scrag")).toBe(1);
    at(sim, 22, mark("tale_vermin"));
    useProp(sim, "tale_vermin_hatch", [b === "a" ? 0 : 1]);
    if (b === "b") {
      const thing = unit(sim, "tale_vermin_under");
      expect(thing, "something came up").toBeDefined();
      thing!.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
      idle(sim, 4);
      expect(flag(sim, "tale_vermin_killed")).toBe(1);
    }
    expect(flag(sim, "been:tale_vermin_fed")).toBe(1);
    at(sim, 10, mark("tale_vermin"));
    talkTo(sim, "tale_vermin_sorrell");
    expect(questDone(sim, "tale_vermin_hatch")).toBe(true);
    if (b === "b") expect(sim.rt.propsByKey.get("tale_vermin_pole")!.hidden, "his pole left on the wall").toBe(false);
  },
};

describe("the tales, played end to end", () => {
  it("every tale has a play here", () => {
    expect(Object.keys(PLAYS).sort()).toEqual(TALES.map((t) => t.id).sort());
  });
  for (const [n, seed] of ALL_SEEDS.entries()) {
    const branch: Branch = n % 2 === 0 ? "a" : "b";
    for (const t of TALES) {
      it(`seed ${seed}: ${t.id} (${branch === "a" ? "first" : "second"} way)`, () => {
        const sim = newGame(seed);
        const rec = sim.rt.bp.stories?.[t.id];
        expect(rec && "box" in rec, `${t.id} has a place on seed ${seed}`).toBe(true);
        PLAYS[t.id](sim, branch);
        for (const q of t.quests) {
          if (q === "tale_bees_crepe" && branch === "b") continue;
          if (q === "tale_walk_plate" && branch === "b") continue;
          expect(questDone(sim, q), `${q} done`).toBe(true);
        }
      }, LONG);
    }
  }
});

