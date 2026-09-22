// Stories at the generated places (world/stories.ts, data/stories). The open country's hamlets, farms,
// cottages, inns, clearings, camps and ruins are rolled per seed and the stories are fixed data, so the
// proof is in three parts:
//
//  1. The data holds together: every story's quests exist, every {place:...} names a story, every name any
//     seed could give a place is unique and fits the tracker, the words keep to VOICE.md's two hard rules.
//  2. On several seeds every story finds a place, or is skipped with the reason written on the blueprint,
//     and that is rare; no two stories share a place; each place has its board; a few stand on the first
//     walk and none crowds the opening.
//  3. A sample of them is PLAYED, end to end, by the headless player: walked to, asked, done, walked back,
//     handed in, paid. Chains are played link by link, the camps are fought, the night step is done at night.
//
// The quest audit (quest-audit.test.ts) holds every placed story to the same five rules as every other quest.

import { describe, expect, it } from "vitest";
import type { Catalog } from "@/sim/catalog";
import { bagCount } from "@/sim/inventory";
import { focusOf } from "@/sim/interact";
import { questDone, requirementCount } from "@/sim/quests";
import { Sim } from "@/sim/sim";
import type { Action, ActionList, Unit } from "@/sim/state";
import { expandText, isNight } from "@/sim/text";
import { blueprintFor } from "@/sim/zones";
import type { Blueprint, StoryPlace } from "@/world/blueprint";
import { countySkeleton } from "@/world/county";
import { allNames, STORIES } from "@/world/names";
import { PLACEMENTS } from "@/world/placements";
import { MACRO, SKEL_W } from "@/world/skeleton";
import storyQuests from "@/data/quests/country.json";
import storyDialogue from "@/data/dialogue/stories.json";
import storyItems from "@/data/items/stories.json";
import { face, idle, walkTo, walkToProp, yardCatalog } from "./bot";

const catalog = yardCatalog();
/** The two seeds every other test plays, and two nobody has tuned anything against. */
const SEEDS = [3, 2026, 77, 11];
/** Seeds whose every quest other tests require to be given (quests.test.ts): no story may be skipped on these. */
const EVERY_STORY = [3, 2026];
const LONG = 600_000;
const KINDS = ["hamlet", "farmstead", "cottage", "inn", "woodcutter", "camp", "ruin"];
const PLACE = /\{place:([a-z0-9_]+)\}/g;
const byId = new Map(STORIES.map((s) => [s.id, s]));

const placed = (bp: Blueprint): [string, Extract<StoryPlace, { box: unknown }>][] =>
  Object.entries(bp.stories ?? {}).filter((e): e is [string, Extract<StoryPlace, { box: unknown }>] => "box" in e[1]);

// --- 1. the data ---------------------------------------------------------------------------

describe("the stories, as data", () => {
  it("every story's quests exist, every quest is in one story, every {place:...} is a story", () => {
    const seen = new Set<string>();
    for (const s of STORIES) {
      expect(KINDS, `${s.id}: kind`).toContain(s.kind);
      if (s.near) expect(byId.has(s.near.story), `${s.id} follows ${s.near.story}`).toBe(true);
      for (const q of s.quests) {
        expect(catalog.quests[q], `${s.id}: quest ${q}`).toBeDefined();
        expect(seen.has(q), `${q} is in two stories`).toBe(false);
        seen.add(q);
      }
      // What it puts at its place is in data/placements; a story with no rows at all is a place nobody uses.
      expect(PLACEMENTS.some((r) => r.at.place === s.id), `${s.id}: no placement rows`).toBe(true);
    }
    const text = JSON.stringify([storyQuests, storyDialogue, storyItems]);
    for (const m of text.matchAll(PLACE)) expect(byId.has(m[1]), `{place:${m[1]}}`).toBe(true);
    for (const r of PLACEMENTS) if (r.at.place) expect(byId.has(r.at.place), `placement ${r.key} at ${r.at.place}`).toBe(true);
  });

  it("there are thirty-odd of them, most short, some chains, some that send her to a camp", () => {
    const told = STORIES.filter((s) => s.quests.length > 0);
    const quests = told.reduce((n, s) => n + s.quests.length, 0);
    const chains = told.filter((s) => s.quests.length >= 2);
    const camps = told.filter((s) => STORIES.some((h) => h.near?.story === s.id && h.kind === "camp"));
    expect(told.length).toBeGreaterThanOrEqual(25);
    expect(STORIES.length).toBeGreaterThanOrEqual(30);
    expect(STORIES.length).toBeLessThanOrEqual(40);
    expect(quests).toBeGreaterThanOrEqual(30);
    expect(chains.length).toBeGreaterThanOrEqual(6);
    expect(chains.length).toBeLessThanOrEqual(8);
    expect(chains.every((s) => s.quests.length <= 3)).toBe(true);
    expect(camps.length).toBeGreaterThanOrEqual(3);
  });

  it("every name a seed could paint on a board is its own, and no name hides inside another", () => {
    const all: string[] = [];
    for (const kind of KINDS) {
      const list = allNames(kind);
      expect(list.length, kind).toBeGreaterThan(0);
      // Every inn is the Halfway House (the innkeepers say so in data/dialogue/country.json). Every other kind has a list.
      if (kind !== "inn") expect(new Set(list).size, `${kind}: duplicate names`).toBe(list.length);
      all.push(...new Set(list));
    }
    for (const a of all) for (const b of all) if (a !== b) expect(b.toLowerCase().includes(a.toLowerCase()), `"${a}" is inside "${b}"`).toBe(false);
    // More names than the busiest seed has places of the kind a story needs: stories draw from the front of the list.
    for (const kind of KINDS) if (kind !== "inn") expect(allNames(kind).length, kind).toBeGreaterThanOrEqual(STORIES.filter((s) => s.kind === kind).length + 10);
  });

  it("the words fit the tracker with the longest name any seed could put in, and keep to VOICE.md", () => {
    const longest = new Map(KINDS.map((k) => [k, allNames(k).reduce((a, b) => (b.length > a.length ? b : a), "")]));
    const worst = (t: string): string => t.replace(PLACE, (_, id: string) => longest.get(byId.get(id)!.kind)!);
    for (const s of STORIES) {
      for (const id of s.quests) {
        const q = catalog.quests[id] as Catalog["quests"][string] & { returnTo?: string };
        for (const r of q.requirements) {
          expect(worst(r.text).length, `${id}: "${worst(r.text)}"`).toBeLessThanOrEqual(70);
          // Every step names the place it happens at: that name is the landmark the quest audit holds it to.
          expect(r.text, `${id}: a step that names no place`).toMatch(PLACE);
        }
        expect(worst(q.description).length, `${id}: description`).toBeLessThanOrEqual(320);
        expect(worst(q.returnTo ?? "").length, `${id}: returnTo`).toBeLessThanOrEqual(70);
      }
    }
    const text = JSON.stringify([storyQuests, storyDialogue, storyItems]);
    expect(/[–—]/.test(text), "an en or em dash").toBe(false);
    expect(text.includes("Jane")).toBe(false);
    // The name goes in where the text says {place:...}, and a pub's "The" is lower case mid-sentence.
    const seed = 3;
    const inn = STORIES.find((s) => s.kind === "inn");
    if (inn) {
      expect(expandText({ name: "Mina", seed }, `At {place:${inn.id}}.`)).toBe("At the Halfway House.");
      expect(expandText({ name: "Mina", seed }, `{place:${inn.id}}. {name}.`)).toBe("The Halfway House. Mina.");
    }
  });
});

// --- 2. on the seeds -------------------------------------------------------------------------

describe("the stories, placed", () => {
  it("every story finds a place, or says why not, and that is rare; no two share one; each has its board", () => {
    let skips = 0;
    let tries = 0;
    for (const seed of SEEDS) {
      const bp = blueprintFor("county", seed);
      const stories = bp.stories ?? {};
      for (const s of STORIES) {
        const p = stories[s.id];
        expect(p, `seed ${seed}: ${s.id} has no record`).toBeDefined();
        tries++;
        if ("skipped" in p) {
          skips++;
          expect(p.skipped.length, `seed ${seed}: ${s.id} skipped without a reason`).toBeGreaterThan(10);
          expect(EVERY_STORY.includes(seed), `seed ${seed}: ${s.id} skipped (${p.skipped}), and quests.test plays every quest on this seed`).toBe(false);
          continue;
        }
        // The board at the place says its name, and the place is the kind the story asked for.
        expect(p.kind).toBe(s.kind);
        const b = p.box;
        const board = bp.props.find((q) => q.def === "name_board" && q.label === p.name && q.cx >= b.cx && q.cy >= b.cy && q.cx < b.cx + b.w && q.cy < b.cy + b.h);
        expect(board, `seed ${seed}: ${s.id}: no board saying ${p.name} at its place`).toBeDefined();
        expect(bp.marks[`story_${s.id}`], `seed ${seed}: ${s.id}: mark`).toBeDefined();
      }
      // Never shared.
      const boxes = placed(bp).map(([, p]) => `${p.box.cx},${p.box.cy}`);
      expect(new Set(boxes).size, `seed ${seed}: two stories share a place`).toBe(boxes.length);
    }
    expect(skips / tries, `${skips} of ${tries} story places skipped`).toBeLessThanOrEqual(0.05);
  }, LONG);

  it("a few stand on the first walk, and none crowds the opening", () => {
    for (const seed of SEEDS) {
      const bp = blueprintFor("county", seed);
      const sk = countySkeleton(seed, bp.attempts - 1);
      const centre = (m: number): number => m * MACRO + MACRO / 2;
      const first = sk.roads
        .filter((r) => (r.from === "station" && r.to === "julie_house") || (r.from === "julie_house" && r.to === "town"))
        .flatMap((r) => r.cells.map((c): [number, number] => [centre(c % SKEL_W), centre(Math.floor(c / SKEL_W))]));
      const station = sk.sites.find((s) => s.id === "station")!;
      const near = placed(bp).filter(([, p]) => first.some(([x, y]) => Math.max(0, p.box.cx - x, x - (p.box.cx + p.box.w)) ** 2 + Math.max(0, p.box.cy - y, y - (p.box.cy + p.box.h)) ** 2 < 70 * 70));
      expect(near.length, `seed ${seed}: stories by the first walk: ${near.map(([id]) => id).join(" ")}`).toBeGreaterThanOrEqual(3);
      for (const [id, p] of placed(bp)) {
        const d = Math.hypot(p.box.cx + p.box.w / 2 - centre(station.mx), p.box.cy + p.box.h / 2 - centre(station.my));
        expect(d, `seed ${seed}: ${id} is on the platform's doorstep`).toBeGreaterThanOrEqual(150);
      }
    }
  }, LONG);
});

// --- 3. played ---------------------------------------------------------------------------------

/** Click through whatever is open, taking an option that does something for the quest when there is one. */
function talk(sim: Sim): void {
  for (let guard = 0; sim.me.dialogue && guard < 60; guard++) {
    const d = sim.me.dialogue;
    const node = sim.catalog.dialogue[d.tree]?.nodes[d.node];
    const options = node?.options ?? [];
    if (node && d.line >= node.lines.length - 1 && options.length > 0) {
      const pick = options.findIndex((o) => (o.actions ?? []).some((a) => a.do === "quest" || a.do === "handin"));
      sim.command({ t: "choose", option: Math.max(0, pick) });
    } else sim.command({ t: "advance" });
  }
  idle(sim, 2);
}

const acts = (list: ActionList | undefined): Action[] => (list ?? []).flatMap((a) => (a.do === "if" ? [a, ...acts(a.then), ...acts(a.else)] : [a]));
function treeActs(tree: string | undefined): Action[] {
  const t = tree ? catalog.dialogue[tree] : undefined;
  if (!t) return [];
  return Object.values(t.nodes).flatMap((n) => [...acts(n.actions), ...(n.options ?? []).flatMap((o) => acts(o.actions))]);
}

/** Stand at the story's own mark (in front of its board) and walk the rest. */
function goTo(sim: Sim, mark: string): void {
  sim.command({ t: "dev", dev: { op: "tp", zone: "county", mark } });
  idle(sim, 3);
}

function useProp(sim: Sim, key: string): void {
  expect(walkToProp(sim, key), `walk to ${key}`).toBe(true);
  const prop = sim.rt.propsByKey.get(key)!;
  const f = focusOf(sim, sim.player);
  expect(f?.kind === "prop" && f.id === prop.id, `USE beside ${key} acts on ${key}, not ${JSON.stringify(f)}`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
  talk(sim);
}

/**
 * Walk up to a person and talk. A person stands in front of her own house, and a person trying to talk to
 * her steps round until it is her USE reaches and not the door: so does the bot, one side at a time.
 */
function talkTo(sim: Sim, u: Unit): void {
  let ok = false;
  for (const [ox, oy] of [[0, 10], [10, 0], [-10, 0], [0, -10], [0, 0]]) {
    if (!walkTo(sim, u.x + ox, u.y + oy, 4)) continue;
    face(sim, u.x, u.y);
    const f = focusOf(sim, sim.player);
    if (f?.kind === "unit" && f.id === u.id) {
      ok = true;
      break;
    }
  }
  expect(ok, `USE reaches ${u.key} from some side (it is at ${Math.round(u.x / 8)},${Math.round(u.y / 8)}; she is at ${Math.round(sim.player.x / 8)},${Math.round(sim.player.y / 8)})`).toBe(true);
  sim.command({ t: "use" });
  idle(sim, 2);
  expect(sim.me.dialogue, `${u.key} answers`).not.toBeNull();
  talk(sim);
}

/** Whoever gives the quest: a person (by their tree) or a thing. */
function giverOf(sim: Sim, quest: string): { unit?: Unit; prop?: string } {
  for (const u of sim.rt.unitsByKey.values()) if (treeActs(catalog.units[u.def]?.talk).some((a) => a.do === "quest" && a.quest === quest)) return { unit: u };
  const p = sim.rt.bp.props.find((q) => treeActs(q.talk).some((a) => a.do === "quest" && a.quest === quest));
  return { prop: p?.key };
}

function doStep(sim: Sim, r: Catalog["quests"][string]["requirements"][number]): void {
  const bp = sim.rt.bp;
  if (r.type === "kill") {
    const standing = [...sim.rt.unitsByKey.values()].filter((u) => u.def === r.target && u.alive);
    expect(standing.length, `${r.target} standing`).toBeGreaterThanOrEqual(r.qty);
    for (const u of standing.slice(0, r.qty)) {
      walkTo(sim, u.x, u.y, 20);
      u.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
      idle(sim, 2);
      expect(u.alive, `${u.key} down`).toBe(false);
    }
    return;
  }
  const does = (p: (typeof bp.props)[number]): boolean => {
    const all = [...acts(p.use), ...treeActs(p.talk)];
    if (r.type === "location") return all.some((a) => a.do === "location" && a.name === r.target);
    return (p.loot ?? []).some((s) => s.item === r.target) || all.some((a) => a.do === "give" && a.item === r.target);
  };
  const p = bp.props.find(does);
  expect(p, `something that does "${r.text}"`).toBeDefined();
  useProp(sim, p!.key);
}

function play(sim: Sim, storyId: string): string[] {
  const s = byId.get(storyId)!;
  const done: string[] = [];
  for (const quest of s.quests) {
    const q = catalog.quests[quest];
    sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
    goTo(sim, `story_${storyId}`);
    const giver = giverOf(sim, quest);
    expect(giver.unit ?? giver.prop, `${quest}: a giver stands in the world`).toBeDefined();
    if (giver.unit) talkTo(sim, giver.unit);
    else useProp(sim, giver.prop!);
    expect(sim.state.quests.active.some((a) => a.quest === quest), `${quest} was offered and taken`).toBe(true);
    for (const r of q.requirements) {
      // A step that says "after the bell" is done after the bell.
      if (/after the (nine o'clock )?bell|after nine|after dark/i.test(r.text)) sim.command({ t: "dev", dev: { op: "time", hour: 22 } });
      expect(isNight(sim.state) === /after the (nine o'clock )?bell|after nine|after dark/i.test(r.text)).toBe(true);
      doStep(sim, r);
      const i = q.requirements.indexOf(r);
      const prog = sim.state.quests.active.find((a) => a.quest === quest)!;
      expect(requirementCount(sim, q, prog, i), `${quest}: "${r.text}" done`).toBe(r.qty);
      sim.command({ t: "dev", dev: { op: "time", hour: 10 } });
    }
    // Back to whoever gave it: walked, not teleported, from the last step.
    const before = catalog.quests[quest].rewards.filter((a) => a.do === "give").map((a) => (a.do === "give" ? a.item : ""));
    const had = new Map(before.map((item) => [item, bagOf(sim, item)]));
    const home = giverOf(sim, quest);
    if (home.unit) talkTo(sim, home.unit);
    else useProp(sim, home.prop!);
    expect(questDone(sim, quest), `${quest} handed in`).toBe(true);
    for (const item of before) expect(bagOf(sim, item), `${quest} paid ${item}`).toBeGreaterThan(had.get(item)!);
    done.push(quest);
  }
  return done;
}

const bagOf = (sim: Sim, item: string): number => bagCount(sim.player, item);

/** Stories played on each seed: every kind of step and giver the batch uses, a chain three deep, both camps' kinds of work, the night step. */
const SAMPLE: Record<number, string[]> = {
  3: ["ames", "leckie", "hurst", "vosper", "treloar"],
  2026: ["rudd", "farrant", "hackett", "gunn", "venn"],
};

describe("the stories, played end to end", () => {
  for (const [seedText, ids] of Object.entries(SAMPLE)) {
    const seed = Number(seedText);
    for (const id of ids) {
      it(`seed ${seed}: ${id}`, () => {
        const bp = blueprintFor("county", seed);
        const rec = bp.stories?.[id];
        expect(rec && "box" in rec, `seed ${seed}: ${id} has a place`).toBe(true);
        const sim = Sim.newGame(catalog, seed);
        sim.command({ t: "dev", dev: { op: "god", on: true } });
        const done = play(sim, id);
        expect(done).toEqual(byId.get(id)!.quests);
      }, LONG);
    }
  }
});
