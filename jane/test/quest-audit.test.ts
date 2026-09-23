// The quest audit: does a person who reads a quest know where to go, can she get there, will she
// know it when she arrives, and is it easy to take it back? QUEST-TREE.md at the repo root is the
// same audit written out by hand, step by step, with what the player is thinking at each one; this
// is the part a machine can hold the county to on every seed, after every world change.
//
// Everything here is DERIVED from the data and the built world, never typed in as coordinates:
//   - who gives a quest    every prop, unit, trigger or quest reward whose actions include `quest <id>`
//   - who takes it back    the same, for `handin <id>`
//   - where a step is      whatever produces it: `location` from a prop's tree or use, a unit's death
//                          or a trigger's rect; `kill` from the units standing in the world; `acquire`
//                          from loot, guaranteed drops, rewards and the bench
//   - what she can read    every placed prop's label, and the speaker and lines of every tree a
//                          placed prop or unit answers with. Only this counts as the world saying a name.
// The only hand-written tables are LANDMARKS (the place names quest text is allowed to use, what each
// name refers to in the world, and what a sign there must say) and TIER (how far each quest may send her).
//
// Rules, as the five questions the user asked (QUEST-TREE.md "The rules the audit enforces"):
//   A DESCRIPTIVE  every step names a landmark; every landmark it names is written up at the place itself;
//                  no ids; a step that only works at night (or by day) says so in the step.
//   B DOABLE       the target exists on every seed, is visible (or is shown by something), is reachable on
//                  foot from the giver through doors she can open, and there are enough of it.
//   C FINDABLE     the target is on screen from a road or path, or close to a landmark the step names;
//                  the walk from the giver is inside the quest's tier.
//   D HAND-IN      whoever takes it back is the giver, is the last step itself, or is named in the quest's
//                  own text; the walk back is inside the tier.
//   E NO SECRETS   what completes a step stands at the place the step names: nothing is completed by
//                  something the text never mentions, somewhere it never points.
//
// `QUEST_REPORT=1 npx vitest run test/quest-audit.test.ts` prints the whole table for every seed.

import { describe, expect, it } from "vitest";
import type { Catalog, DialogueTree } from "@/sim/catalog";
import { BLOCK_MOVE, Tile } from "@/sim/grid";
import { Sim } from "@/sim/sim";
import type { Action, ActionList, Condition } from "@/sim/state";
import { blueprintFor } from "@/sim/zones";
import { ZONE_IDS } from "@/world";
import type { Blueprint } from "@/world/blueprint";
import { countySkeleton } from "@/world/county";
import { SKEL_W } from "@/world/skeleton";
import pathsJson from "@/data/paths.json";
import lowfieldsQuests from "@/data/quests/lowfields.json";
import { DEFAULT_NAME, expandText } from "@/sim/text";
import { STORIES } from "@/world/names";
import { yardCatalog } from "./bot";

const catalog = yardCatalog();
/** Three seeds: the two the other tests play, and one nobody has tuned anything against. */
const SEEDS = [3, 2026, 77];
const LONG = 300_000;
const SIDE = new Set(Object.keys(lowfieldsQuests));

// --- thresholds -------------------------------------------------------------------------------
//
// The camera shows 48 x 27 cells (1 cell = 1 m). Half a screen is 24 across and 13 down: something that
// close to where she stands is ON SCREEN. Distances below are measured in half-screens along each axis,
// max(|dx| / 24, |dy| / 13), so "1" means "on screen from there" whichever way the screen is oriented.

const HALF_W = 24;
const HALF_H = 13;
/** On screen from the road: standing on the nearest road cell, the target is in view. */
const ON_SCREEN = 1;
/**
 * Close to a named landmark: stand at the landmark and it is on screen after one half-screen's walk.
 * Two half-screens is the most a direction like "at the well" can stretch to and still be the well's.
 */
const NEAR_LANDMARK = 2;
/**
 * A place's name is written up AT the place: a sign, a plate, a note, a label within this many
 * half-screens of it (of its edge, for a patch). Further than that and she cannot tell she has arrived.
 */
const POSTED_NEAR = 2;
/**
 * A patch of ground she is sent INTO (the Top Field, Sallow Bottom) is itself the landmark once she can
 * see it: its edge must come within this many half-screens of a road or path, or she never sees it.
 */
const PATCH_SEEN_FROM_ROAD = 4;
/**
 * A small place (a cottage, a camp, a well) is 9 to 16 cells across, so its edge is on screen from the
 * road while its middle is up to half again further off: a named small place counts as seen from the road
 * at this many half-screens.
 */
const LANDMARK_SEEN_FROM_ROAD = 1.5;

/**
 * Walking budgets, in steps (cells) on foot from the giver to the furthest step. She walks about six
 * cells a second on a road: 360 a minute. A hub errand is under a minute; a near errand a few minutes,
 * out of sight of the giver but within one stretch of road; a far one crosses a region; the one
 * cross-map chain (QUESTS.md K6) may cross the Lowfields.
 */
const WALK_BUDGET = { hub: 400, near: 1500, far: 2800, cross: 4600 } as const;
type Tier = keyof typeof WALK_BUDGET;

/**
 * How far each quest may send her. Anything not listed is "near". Every "far" or "cross" says why.
 */
const TIER: Record<string, { tier: Tier; why?: string }> = {
  the_letter: { tier: "far", why: "the first walk, station to Julie's: the road is the tutorial" },
  lost_property: { tier: "near", why: "the first two minutes: all three lie on the station road, before Julie's gate (user, 2026-09-23)" },
  left_luggage: { tier: "cross", why: "the one cross-map chain: the platform, the mine road, the platform" },
  the_nurses_round: { tier: "far", why: "three corners of the Lowfields; each note is on a road" },
  mrs_allens_dressing: { tier: "far", why: "the case in the wood back to the door in Castle square" },
  the_lampmans_brazier: { tier: "far", why: "the planks are in the car in the wood; the stone sends her for them by name" },
  before_the_bell: { tier: "far", why: "Julie's yard to Sallow Bottom, past Castle" },
  if_found: { tier: "far", why: "the Long Hedge back to Castle Halt: the breadcrumb west" },
  stood_down: { tier: "near" },
  the_mine: { tier: "far", why: "Julie's yard to the mine at the end of the mine road" },
  the_burial: { tier: "far", why: "Julie's yard to the graveyard road, the far side of Castle" },
};

/** Kill counts: more standing than asked (QUESTS.md K8), so the last one is never a wait. */
const KILL_SPARE = 1;

// --- the words a quest may use for a place ----------------------------------------------------
//
// Each row: the phrase as it may appear in a step (a regex, case-insensitive), what it refers to in the
// built world, and what a readable thing at that place must say for a player to know she has arrived.
// A step that uses a place phrase not in this list fails A2, so a new name has to be added here, with
// the sign that makes it true, before any quest can send her there.

type Ref =
  | { site: string }
  | { area: string }
  | { mark: string }
  | { prop: string }
  | { unit: string }
  | { road: [string, string] }
  | { path: string }
  | { zone: string }
  /** A generated place a story claimed on this seed (bp.stories): its footprint. */
  | { box: { cx: number; cy: number; w: number; h: number } };
type Landmark = { phrase: RegExp; ref: Ref; says: RegExp };

const LANDMARKS: Landmark[] = [
  // Castle Halt, the first walk
  { phrase: /castle halt|the platform|the station/i, ref: { site: "station" }, says: /castle halt/i },
  { phrase: /station road/i, ref: { road: ["station", "julie_house"] }, says: /station road/i },
  { phrase: /\bwell\b/i, ref: { mark: "halt_well" }, says: /\bwell\b/i },
  { phrase: /\bsignpost\b/i, ref: { mark: "halt_signpost" }, says: /signpost/i },
  { phrase: /castle road/i, ref: { road: ["julie_house", "town"] }, says: /castle road/i },
  { phrase: /the cart on the station road|under the cart/i, ref: { mark: "halt_cart" }, says: /\bcart\b/i },
  { phrase: /\btrunk\b/i, ref: { prop: "left_luggage_trunk" }, says: /trunk/i },
  { phrase: /lost[- ]property/i, ref: { prop: "lost_property_book" }, says: /lost property/i },
  // Julie's
  { phrase: /julie'?s (house|gate|yard|fence)|auntie julie/i, ref: { site: "julie_house" }, says: /julie/i },
  { phrase: /kitchen|the bench/i, ref: { zone: "house" }, says: /julie/i },
  { phrase: /cellar/i, ref: { zone: "cellar" }, says: /cellar/i },
  { phrase: /garden book/i, ref: { prop: "garden_book" }, says: /garden book/i },
  { phrase: /\bthe dog\b/i, ref: { unit: "dog" }, says: /dog/i },
  // Castle
  { phrase: /\bcastle\b(?! halt| road| square| allotment)|last house/i, ref: { site: "town" }, says: /\bcastle\b/i },
  { phrase: /castle square|the square|facing the fire/i, ref: { mark: "town_square" }, says: /parish|castle/i },
  { phrase: /parish board/i, ref: { prop: "parish_board" }, says: /parish/i },
  { phrase: /allotments?/i, ref: { area: "allotments" }, says: /allotment/i },
  { phrase: /plot 9/i, ref: { mark: "plot_nine" }, says: /plot 9/i },
  // Castle's own streets (world/chunks.ts): every door carries its number and its lane, and the lanes have name signs.
  { phrase: /churchyard|behind the church/i, ref: { mark: "churchyard" }, says: /st anne|churchyard/i },
  { phrase: /by the church|church door/i, ref: { mark: "church_door" }, says: /st anne/i },
  { phrase: /fountain/i, ref: { mark: "fountain" }, says: /fountain/i },
  { phrase: /the forge|cross lane/i, ref: { mark: "forge" }, says: /forge|cross lane/i },
  { phrase: /back lane/i, ref: { mark: "back_lane" }, says: /back lane/i },
  { phrase: /the doctor'?s|pound lane/i, ref: { mark: "doctor" }, says: /doctor|pound lane/i },
  { phrase: /arms yard|castle arms/i, ref: { mark: "arms_yard" }, says: /castle arms/i },
  { phrase: /post office/i, ref: { mark: "post_office" }, says: /post office/i },
  { phrase: /memorial/i, ref: { mark: "memorial" }, says: /memorial/i },
  { phrase: /telephone box|phone box/i, ref: { mark: "phone_box" }, says: /telephone/i },
  { phrase: /church lane|north gate/i, ref: { mark: "church_lane_top" }, says: /church lane/i },
  { phrase: /west end of the high street/i, ref: { mark: "street_west" }, says: /high street/i },
  { phrase: /east end of the high street/i, ref: { mark: "street_east" }, says: /high street/i },
  { phrase: /orchard/i, ref: { mark: "orchard" }, says: /orchard/i },
  // The farm and the fields
  { phrase: /lowfield farm|farm gate|the farm\b|farmhouse|farmer/i, ref: { site: "farm" }, says: /lowfield farm|farm/i },
  { phrase: /top field/i, ref: { area: "top_field" }, says: /top field/i },
  { phrase: /long hedge/i, ref: { area: "long_hedge" }, says: /long hedge/i },
  { phrase: /footpath no\. 3|the stile/i, ref: { path: "footpath_three" }, says: /footpath no\. 3/i },
  { phrase: /hollow tree on footpath/i, ref: { mark: "hedge_tree" }, says: /hollow tree|haversack/i },
  { phrase: /scarecrow/i, ref: { mark: "scarecrow_gate" }, says: /scarecrow/i },
  { phrase: /farm track/i, ref: { path: "top_field_track" }, says: /top field/i },
  { phrase: /quarry track/i, ref: { path: "quarry_track" }, says: /quarry steps/i },
  { phrase: /her prints/i, ref: { path: "nurses_prints" }, says: /prints/i },
  // The wood and the nurse
  { phrase: /car in the wood|the car\b|behind the car/i, ref: { site: "car_wood" }, says: /nurse|car/i },
  { phrase: /hollow tree in the wood/i, ref: { mark: "nurses_coat" }, says: /coat|hollow/i },
  { phrase: /sallow bottom/i, ref: { area: "sallow_bottom" }, says: /sallow bottom/i },
  { phrase: /\bjetty\b/i, ref: { mark: "sallow_jetty" }, says: /jetty/i },
  { phrase: /mrs allen/i, ref: { mark: "cottage_allen" }, says: /allen/i },
  { phrase: /mr pike/i, ref: { mark: "cottage_pike" }, says: /pike/i },
  { phrase: /misses crane/i, ref: { mark: "cottage_crane" }, says: /crane/i },
  // The lamps
  { phrase: /pell'?s stone/i, ref: { mark: "pell_shrine" }, says: /pell/i },
  { phrase: /no\. 12\b/i, ref: { mark: "lamp_12" }, says: /no\. 12/i },
  { phrase: /no\. 13\b/i, ref: { mark: "lamp_13" }, says: /no\. 13/i },
  { phrase: /no\. 15\b/i, ref: { mark: "lamp_15" }, says: /no\. 15/i },
  { phrase: /brazier/i, ref: { mark: "pell_brazier" }, says: /brazier/i },
  // The mine road and the quarry
  { phrase: /mine road/i, ref: { road: ["town", "gold_mine"] }, says: /mine road/i },
  { phrase: /the mine\b|gold mine|goldskin'?s mine/i, ref: { site: "gold_mine" }, says: /goldskin/i },
  { phrase: /quarry steps|cut face|foothills/i, ref: { area: "quarry_steps" }, says: /quarry steps/i },
  { phrase: /gang'?s camp/i, ref: { mark: "quarry_camp" }, says: /gang/i },
  { phrase: /down the (gold )?mine/i, ref: { zone: "mine" }, says: /goldskin/i },
  { phrase: /standing stone/i, ref: { site: "burial" }, says: /do not/i },
  { phrase: /burial chamber/i, ref: { zone: "burial" }, says: /burial chamber/i },
  { phrase: /graveyard/i, ref: { site: "graveyard" }, says: /graveyard|grave/i },
];

/**
 * The generated places' names are landmarks too, but they are the seed's, so they are not typed here:
 * every story that found a place on this seed (world/stories.ts, bp.stories) adds its place's name as a
 * phrase, the place's footprint as what it refers to, and the same name as what the board there must say.
 * A story's text says {place:<story>} and is read with the name put in (sim/text.ts), exactly as she reads it.
 */
function storyLandmarks(bp: Blueprint): Landmark[] {
  const out: Landmark[] = [];
  for (const s of Object.values(bp.stories ?? {})) {
    if (!("box" in s)) continue;
    const name = new RegExp(s.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replace(/^The /, "(?:the |The )?"), "i");
    out.push({ phrase: name, ref: { box: s.box }, says: name });
  }
  return out;
}

/** Which story each quest is told in (data/stories): a story the seed found no place for has no quests on it. */
const STORY_OF = new Map(STORIES.flatMap((s) => s.quests.map((q) => [q, s.id] as const)));

// --- the built world, as the player meets it -------------------------------------------------

type Time = "night" | "day" | undefined;
type Thing = {
  zone: string;
  cx: number;
  cy: number;
  /** "prop:key", "unit:key", "rect:name", "reward:quest", "start". */
  id: string;
  /** Everything a player can read at it: label, speaker, every line of its tree. */
  text: string;
  hidden: boolean;
  time: Time;
  /** For an item source: how many it yields (Infinity for the bench, a respawning drop). */
  qty?: number;
};

type World = {
  seed: number;
  bps: Record<string, Blueprint>;
  sim: Sim;
  /** Every source of every action a player can set off, by "quest:x", "handin:x", "location:x", "item:x", "kill:def". */
  sources: Map<string, Thing[]>;
  readable: Thing[];
  /** Keys of props some action can show. */
  shown: Set<string>;
  /** For a zone: the county door that leads (eventually) to it, and the key tags on the way. */
  doorTo: Map<string, { door: Thing; locks: string[] }>;
  sites: Map<string, { cx: number; cy: number; r: number }>;
  areas: Map<string, { cx: number; cy: number; r: number }>;
  roads: Map<string, [number, number][]>;
  paths: Map<string, [number, number][]>;
  /** Squared-screen distance field to the nearest road or path cell, sampled lazily. */
  roadCells: Uint8Array;
  items: Set<string>;
};

const flatten = (list: ActionList | undefined): Action[] => {
  const out: Action[] = [];
  for (const a of list ?? []) {
    out.push(a);
    if (a.do === "if") out.push(...flatten(a.then), ...flatten(a.else));
    if (a.do === "send" && a.then) out.push(...flatten(a.then));
  }
  return out;
};

const isNightOnly = (c: Condition): boolean => c.if === "night" && !c.not;
const isDayOnly = (c: Condition): boolean => c.if === "night" && !!c.not;

/** What a tree lets happen, node by node, with the hour it happens at (first matching start row wins). */
function treeActions(tree: DialogueTree): { actions: Action[]; time: Time; text: string }[] {
  const out: { actions: Action[]; time: Time; text: string }[] = [];
  let unconditionalNightAbove = false;
  for (const row of tree.start) {
    const when = row.when ?? [];
    const time: Time = when.some(isNightOnly) ? "night" : when.some(isDayOnly) || unconditionalNightAbove ? "day" : undefined;
    if (when.length === 1 && isNightOnly(when[0])) unconditionalNightAbove = true;
    const open = [row.node];
    const seen = new Set<string>();
    while (open.length > 0) {
      const id = open.pop()!;
      if (seen.has(id) || !tree.nodes[id]) continue;
      seen.add(id);
      const node = tree.nodes[id];
      out.push({ actions: flatten(node.actions), time, text: node.lines.join(" ") });
      if (node.goto) open.push(node.goto);
      for (const o of node.options ?? []) {
        out.push({ actions: flatten(o.actions), time, text: node.lines.join(" ") });
        if (o.goto) open.push(o.goto);
      }
    }
  }
  return out;
}

function treeText(c: Catalog, id: string | undefined, seed: number): string {
  const t = id ? c.dialogue[id] : undefined;
  if (!t) return "";
  return say(seed, [t.speaker, ...Object.values(t.nodes).flatMap((n) => [...n.lines, ...(n.options ?? []).map((o) => o.label)])].join(" "));
}

/** Text as she reads it on this seed: {name} and the story places' names put in (sim/text.ts). */
const say = (seed: number, text: string): string => expandText({ name: DEFAULT_NAME, seed }, text);

const worlds = new Map<number, World>();

function world(seed: number): World {
  const got = worlds.get(seed);
  if (got) return got;
  const c = catalog;
  const bps: Record<string, Blueprint> = {};
  for (const z of ZONE_IDS) {
    try {
      bps[z] = blueprintFor(z, seed);
    } catch {
      // A zone that will not build is world.test's to report. A quest that leans on it fails here by name.
    }
  }
  const sim = Sim.newGame(c, seed);
  const sources = new Map<string, Thing[]>();
  const readable: Thing[] = [];
  const shown = new Set<string>();
  const add = (key: string, t: Thing): void => {
    const list = sources.get(key) ?? [];
    list.push(t);
    sources.set(key, list);
  };
  const note = (t: Thing, actions: Action[], time: Time): void => {
    for (const a of actions) {
      if (a.do === "quest") add(`quest:${a.quest}`, { ...t, time });
      if (a.do === "handin") add(`handin:${a.quest}`, { ...t, time });
      if (a.do === "location") add(`location:${a.name}`, { ...t, time });
      if (a.do === "give") add(`item:${a.item}`, { ...t, time, qty: a.qty ?? 1 });
      if (a.do === "show") shown.add(a.prop);
    }
  };

  for (const [zone, bp] of Object.entries(bps)) {
    // Going through a door puts the zone's name up as a banner: that is her reading it.
    if (zone !== "county") readable.push({ zone, cx: 0, cy: 0, id: `zone:${zone}`, text: bp.name, hidden: false, time: undefined });
    for (const p of bp.props) {
      const def = c.props[p.def];
      const t: Thing = {
        zone,
        cx: p.cx,
        cy: p.cy,
        id: `prop:${p.key}`,
        text: [p.label ?? "", treeText(c, p.talk, seed)].join(" "),
        hidden: !!p.hidden,
        time: undefined,
      };
      if (t.text.trim()) readable.push(t);
      // A thing under a stone is shown by pushing the stone off it (sim/under.ts).
      if (p.under) shown.add(p.under);
      note(t, flatten(p.use), undefined);
      // A plate's other list: what happens when the stone is pushed off it (the key under the stone).
      note(t, flatten(p.release), undefined);
      if (p.talk && c.dialogue[p.talk]) for (const n of treeActions(c.dialogue[p.talk])) note(t, n.actions, n.time);
      for (const s of p.loot ?? []) add(`item:${s.item}`, { ...t, qty: s.qty });
      if (def?.bench) {
        for (const r of c.recipes) add(`item:${r.output}`, { ...t, qty: Infinity, id: `bench:${p.key}` });
      }
    }
    for (const u of bp.units) {
      const def = c.units[u.def];
      if (!def) continue;
      const time: Time = def.nightOnly ? "night" : def.dayOnly ? "day" : undefined;
      const t: Thing = { zone, cx: u.cx, cy: u.cy, id: `unit:${u.key}`, text: [def.name, treeText(c, def.talk, seed)].join(" "), hidden: false, time };
      readable.push(t);
      add(`kill:${u.def}`, t);
      note(t, flatten(def.onDeath), time);
      if (def.talk && c.dialogue[def.talk]) for (const n of treeActions(c.dialogue[def.talk])) note(t, n.actions, n.time ?? time);
      for (const l of def.loot) if (l.chance >= 1) add(`item:${l.item}`, { ...t, qty: (l.qty ?? 1) * (def.respawn > 0 ? Infinity : 1) });
    }
    for (const [tid, tr] of Object.entries(c.triggers)) {
      if (tr.zone !== zone || !bp.rects[tr.rect]) continue;
      const r = bp.rects[tr.rect];
      const when = tr.when ?? [];
      const time: Time = when.some(isNightOnly) ? "night" : when.some(isDayOnly) ? "day" : undefined;
      note({ zone, cx: r.cx + Math.floor(r.w / 2), cy: r.cy + Math.floor(r.h / 2), id: `rect:${tr.rect}(${tid})`, text: "", hidden: false, time }, flatten(tr.actions), time);
    }
  }
  // The start, and quests that pay out in quests or things: the giver of the next is whoever took the last.
  const startMark = bps.county?.marks[c.start.mark];
  const start: Thing = { zone: "county", cx: startMark?.cx ?? 0, cy: startMark?.cy ?? 0, id: "start", text: "", hidden: false, time: undefined };
  for (const q of c.start.quests) add(`quest:${q}`, start);
  for (const s of c.start.items) add(`item:${s.item}`, { ...start, qty: s.qty });
  for (let changed = true; changed; ) {
    changed = false;
    for (const [id, q] of Object.entries(c.quests)) {
      for (const h of sources.get(`handin:${id}`) ?? []) {
        for (const a of flatten(q.rewards)) {
          const key = a.do === "quest" ? `quest:${a.quest}` : a.do === "give" ? `item:${a.item}` : "";
          if (!key) continue;
          const tag = `reward:${id}@${h.id}`;
          if ((sources.get(key) ?? []).some((t) => t.id === tag)) continue;
          add(key, { ...h, id: tag, qty: a.do === "give" ? (a.qty ?? 1) : undefined });
          changed = true;
        }
      }
    }
  }

  // Zones: the county door each one is reached by, and the locks on the way.
  const doorTo = new Map<string, { door: Thing; locks: string[] }>();
  const queue: { zone: string; door: Thing | null; locks: string[] }[] = [{ zone: "county", door: null, locks: [] }];
  const done = new Set<string>();
  while (queue.length > 0) {
    const { zone, door, locks } = queue.shift()!;
    if (done.has(zone)) continue;
    done.add(zone);
    if (door) doorTo.set(zone, { door, locks });
    for (const p of bps[zone]?.props ?? []) {
      if (!p.to || done.has(p.to.zone)) continue;
      const here: Thing = door ?? { zone: "county", cx: p.cx, cy: p.cy, id: `prop:${p.key}`, text: p.label ?? "", hidden: !!p.hidden, time: undefined };
      queue.push({ zone: p.to.zone, door: here, locks: p.locked && p.keyTag ? [...locks, p.keyTag] : locks });
    }
  }

  // Sites, patches, roads and footpaths from the skeleton this county was built from.
  const bp = bps.county;
  const sk = countySkeleton(seed, bp.attempts - 1);
  const centre = (m: number): number => m * 16 + 8;
  const sites = new Map(sk.sites.map((s) => [s.id, { cx: centre(s.mx), cy: centre(s.my), r: Math.max(40, s.row.hub ?? 0) }]));
  const areas = new Map(sk.areas.map((a) => [a.id, { cx: centre(a.mx), cy: centre(a.my), r: a.row.radius }]));
  const roads = new Map<string, [number, number][]>();
  for (const r of sk.roads) roads.set(`${r.from}|${r.to}`, r.cells.map((cell) => [centre(cell % SKEL_W), centre(Math.floor(cell / SKEL_W))]));
  const paths = new Map<string, [number, number][]>();
  for (const row of pathsJson as { id: string; from: string; via: string; to: string }[]) {
    const from = sk.sites.find((s) => s.id === row.from);
    const to = sk.sites.find((s) => s.id === row.to);
    const via = sk.anchors.find((a) => a.id === row.via) ?? sk.areas.find((a) => a.id === row.via);
    if (!from || !to || !via) continue;
    const pts: [number, number][] = [
      [centre(from.mx), centre(from.my)],
      [centre(via.mx), centre(via.my) + 5],
      [centre(to.mx), centre(to.my)],
    ];
    const line: [number, number][] = [];
    for (let s = 0; s < pts.length - 1; s++) {
      const [ax, ay] = pts[s];
      const [bx, by] = pts[s + 1];
      const n = Math.max(Math.abs(bx - ax), Math.abs(by - ay));
      for (let i = 0; i <= n; i += 2) line.push([Math.round(ax + ((bx - ax) * i) / n), Math.round(ay + ((by - ay) * i) / n)]);
    }
    paths.set(row.id, line);
  }
  // Road and path cells, as a mask: what she can see "from the road".
  const roadCells = new Uint8Array(bp.w * bp.h);
  const tiles = bp.tiles;
  const road = Tile.Road;
  for (let i = 0; i < tiles.length; i++) if (tiles[i] === road) roadCells[i] = 1;
  for (const line of paths.values()) for (const [x, y] of line) if (x >= 0 && y >= 0 && x < bp.w && y < bp.h) roadCells[y * bp.w + x] = 1;
  // A footpath a story laid from the road to its place off the road (world/stories.ts) is a path like any other.
  for (const s of Object.values(bp.stories ?? {})) if ("path" in s && s.path) for (const [x, y] of s.path) roadCells[y * bp.w + x] = 1;
  // Castle's lanes are dirt and its square is cobble, not Road, but in a town every street is a street: the
  // town chunk is ~100 x 70 round its square (world/chunks.ts), so its dirt and cobble count as road.
  const sq = bp.marks.town_square;
  if (sq) {
    for (let y = Math.max(0, sq.cy - 40); y < Math.min(bp.h, sq.cy + 40); y++) {
      for (let x = Math.max(0, sq.cx - 55); x < Math.min(bp.w, sq.cx + 55); x++) {
        const t = tiles[y * bp.w + x];
        if (t === Tile.Dirt || t === Tile.Cobble) roadCells[y * bp.w + x] = 1;
      }
    }
  }

  const items = new Set([...sources.keys()].filter((k) => k.startsWith("item:")).map((k) => k.slice(5)));
  const w: World = { seed, bps, sim, sources, readable, shown, doorTo, sites, areas, roads, paths, roadCells, items };
  worlds.set(seed, w);
  return w;
}

// --- measuring -----------------------------------------------------------------------------------

/** Where a thing is, in the county: itself, or the county door it is reached by. */
function inCounty(w: World, t: Thing): Thing | null {
  if (t.zone === "county") return t;
  return w.doorTo.get(t.zone)?.door ?? null;
}

/** Half-screens between two cells: max(|dx| / 24, |dy| / 13). */
const screens = (ax: number, ay: number, bx: number, by: number): number => Math.max(Math.abs(ax - bx) / HALF_W, Math.abs(ay - by) / HALF_H);

/** Half-screens from a cell to the nearest road or path cell (capped: beyond the cap it is simply far). */
function toRoad(w: World, x: number, y: number, cap = 8): number {
  const bw = w.bps.county.w;
  const bh = w.bps.county.h;
  let best = Infinity;
  // Rows outward from her own, so the scan stops as soon as no further row could be nearer.
  for (let ady = 0; ady <= HALF_H * cap; ady++) {
    if (best <= ady / HALF_H) break;
    for (const dy of ady === 0 ? [0] : [-ady, ady]) {
      const yy = y + dy;
      if (yy < 0 || yy >= bh) continue;
      const row = yy * bw;
      for (let dx = -HALF_W * cap; dx <= HALF_W * cap; dx++) {
        const xx = x + dx;
        if (xx < 0 || xx >= bw || !w.roadCells[row + xx]) continue;
        best = Math.min(best, Math.max(Math.abs(dx) / HALF_W, ady / HALF_H));
      }
    }
  }
  return best;
}

/** Where a landmark is, as a set of sample points and a radius (a patch has a body; a road is its line). */
function refPoints(w: World, ref: Ref): { pts: [number, number][]; r: number } | null {
  const bp = w.bps.county;
  if ("site" in ref) {
    const s = w.sites.get(ref.site);
    return s ? { pts: [[s.cx, s.cy]], r: s.r } : null;
  }
  if ("area" in ref) {
    const a = w.areas.get(ref.area);
    return a ? { pts: [[a.cx, a.cy]], r: a.r } : null;
  }
  if ("mark" in ref) {
    const m = bp.marks[ref.mark];
    return m ? { pts: [[m.cx, m.cy]], r: 0 } : null;
  }
  if ("prop" in ref) {
    const p = bp.props.find((q) => q.key === ref.prop);
    return p ? { pts: [[p.cx, p.cy]], r: 0 } : null;
  }
  if ("unit" in ref) {
    const u = bp.units.find((q) => q.key === ref.unit);
    return u ? { pts: [[u.cx, u.cy]], r: 0 } : null;
  }
  if ("road" in ref) {
    const line = w.roads.get(ref.road.join("|")) ?? w.roads.get([...ref.road].reverse().join("|"));
    return line ? { pts: line, r: 0 } : null;
  }
  if ("path" in ref) {
    const line = w.paths.get(ref.path);
    return line ? { pts: line.filter((_, i) => i % 8 === 0), r: 0 } : null;
  }
  if ("box" in ref) {
    // Its rim, every few cells: "seen from the road" is its edge seen, as for any small place.
    const { cx, cy, w: bw, h: bh } = ref.box;
    const pts: [number, number][] = [];
    for (let x = cx; x < cx + bw; x += 4) pts.push([x, cy], [x, cy + bh - 1]);
    for (let y = cy; y < cy + bh; y += 4) pts.push([cx, y], [cx + bw - 1, y]);
    return { pts, r: 0 };
  }
  const d = w.doorTo.get(ref.zone)?.door;
  return d ? { pts: [[d.cx, d.cy]], r: 0 } : null;
}

/** Half-screens from a cell to a landmark (0 inside a patch or a site's hub). */
function toRef(w: World, ref: Ref, x: number, y: number): number {
  if ("box" in ref) {
    // A footprint: nothing inside it is any distance from it.
    const b = ref.box;
    return Math.max(Math.max(0, b.cx - x, x - (b.cx + b.w - 1)) / HALF_W, Math.max(0, b.cy - y, y - (b.cy + b.h - 1)) / HALF_H);
  }
  const at = refPoints(w, ref);
  if (!at) return Infinity;
  let best = Infinity;
  for (const [px, py] of at.pts) {
    const d = Math.hypot(px - x, py - y);
    if (d <= at.r) return 0;
    // Shrink by the radius along the line to the centre, then measure in half-screens.
    const k = at.r > 0 ? (d - at.r) / d : 1;
    best = Math.min(best, screens(x, y, x + (px - x) * k, y + (py - y) * k));
  }
  return best;
}

const fields = new Map<string, Uint32Array>();

/** Steps on foot from a county cell to every cell. A giver stands on a solid prop, so start from the ground round it. */
function walkField(w: World, from: Thing): Uint32Array {
  const key = `${w.seed}:${from.cx},${from.cy}`;
  const got = fields.get(key);
  if (got) return got;
  const g = w.sim.rt.grid;
  const dist = new Uint32Array(g.w * g.h).fill(0xffffffff);
  const queue = new Int32Array(g.w * g.h);
  const flags = g.flags;
  const block = BLOCK_MOVE;
  const gw = g.w;
  let head = 0;
  let tail = 0;
  for (let oy = -3; oy <= 4; oy++) {
    for (let ox = -3; ox <= 4; ox++) {
      const x = from.cx + ox;
      const y = from.cy + oy;
      if (x < 0 || y < 0 || x >= gw || y >= g.h || (flags[y * gw + x] & block) !== 0) continue;
      dist[y * gw + x] = 0;
      queue[tail++] = y * gw + x;
    }
  }
  while (head < tail) {
    const i = queue[head++];
    const d = dist[i] + 1;
    const x = i % gw;
    if (x + 1 < gw && dist[i + 1] === 0xffffffff && (flags[i + 1] & block) === 0) (dist[i + 1] = d), (queue[tail++] = i + 1);
    if (x > 0 && dist[i - 1] === 0xffffffff && (flags[i - 1] & block) === 0) (dist[i - 1] = d), (queue[tail++] = i - 1);
    if (i + gw < dist.length && dist[i + gw] === 0xffffffff && (flags[i + gw] & block) === 0) (dist[i + gw] = d), (queue[tail++] = i + gw);
    if (i >= gw && dist[i - gw] === 0xffffffff && (flags[i - gw] & block) === 0) (dist[i - gw] = d), (queue[tail++] = i - gw);
  }
  if (fields.size > 24) fields.delete(fields.keys().next().value as string);
  fields.set(key, dist);
  return dist;
}

/** The shortest walk to arm's reach of a thing, or null. */
function walk(w: World, from: Thing, to: Thing): number | null {
  const a = inCounty(w, from);
  const b = inCounty(w, to);
  if (!a || !b) return null;
  const dist = walkField(w, a);
  const g = w.sim.rt.grid;
  let best = 0xffffffff;
  for (let oy = -3; oy <= 4; oy++) {
    for (let ox = -3; ox <= 4; ox++) {
      const x = b.cx + ox;
      const y = b.cy + oy;
      if (x < 0 || y < 0 || x >= g.w || y >= g.h) continue;
      best = Math.min(best, dist[y * g.w + x]);
    }
  }
  return best === 0xffffffff ? null : best;
}

const COMPASS = ["east", "south-east", "south", "south-west", "west", "north-west", "north", "north-east"];
function bearing(a: Thing, b: Thing): string {
  const ang = Math.atan2(b.cy - a.cy, b.cx - a.cx);
  return COMPASS[(Math.round(ang / (Math.PI / 4)) + 8) % 8];
}

// --- the audit -------------------------------------------------------------------------------------

type Row = { quest: string; step: string; text: string; where: string; walk: string; road: string; lead: string; ok: string };

const NIGHT_WORDS = /after the (nine o'clock )?bell|after nine|at night|after dark/i;
const DAY_WORDS = /by day|in daylight|while it is light|before the bell|daylight/i;

/** The giver of a quest nearest to where it is used: givers and targets are paired per seed. */
function audit(seed: number, quests: Catalog["quests"] = catalog.quests): { rows: Row[]; problems: string[] } {
  const w = world(seed);
  const c = catalog;
  const rows: Row[] = [];
  const problems: string[] = [];
  const fail = (quest: string, step: string, why: string): void => {
    problems.push(`seed ${seed} ${quest} ${step}: ${why}`);
  };

  const landmarks = [...LANDMARKS, ...storyLandmarks(w.bps.county)];
  for (const [qid, raw] of Object.entries(quests)) {
    // A story the seed found no place for is not on this seed at all: nothing gives it (stories.test counts how rare that is).
    const story = STORY_OF.get(qid);
    const placed = story ? w.bps.county.stories?.[story] : undefined;
    if (placed && "skipped" in placed) {
      rows.push({ quest: qid, step: "-", text: `(no place on this seed: ${placed.skipped})`, where: "-", walk: "-", road: "-", lead: "-", ok: "skipped" });
      continue;
    }
    // The words as she reads them on this seed, with the generated places' names in.
    const q = { ...raw, description: say(seed, raw.description), returnTo: say(seed, (raw as { returnTo?: string }).returnTo ?? ""), requirements: raw.requirements.map((r) => ({ ...r, text: say(seed, r.text) })) };
    const tier = TIER[qid]?.tier ?? "near";
    const budget = WALK_BUDGET[tier];
    const givers = (w.sources.get(`quest:${qid}`) ?? []).filter((t) => inCounty(w, t));
    if (givers.length === 0) {
      fail(qid, "giver", "nothing in the built world gives it");
      continue;
    }
    // The first giver a player would meet is as good as any: take the one nearest the steps.
    const giver = givers[0];
    const handins = (w.sources.get(`handin:${qid}`) ?? []).filter((t) => inCounty(w, t));
    if (handins.length === 0) fail(qid, "hand-in", "nothing in the built world hands it in");
    let last: Thing | null = null;
    let furthest = 0;

    q.requirements.forEach((r, i) => {
      const step = `#${i + 1}`;
      const text = r.text;
      const probs: string[] = [];
      const bad = (why: string): void => {
        probs.push(why);
        fail(qid, `${step} "${text}"`, why);
      };

      // A. DESCRIPTIVE
      if (/[a-z]+_[a-z]+/i.test(text)) bad("A1 an id shows in the text");
      const named = landmarks.filter((l) => l.phrase.test(text));
      if (named.length === 0) bad("A2 names no landmark a player could look for");

      // Where the step is.
      const key = r.type === "kill" ? `kill:${r.target}` : r.type === "acquire" ? `item:${r.target}` : `location:${r.target}`;
      const targets = (w.sources.get(key) ?? []).filter((t) => !t.id.startsWith("start"));
      // B. DOABLE: exists, visible or shown, enough of it.
      if (targets.length === 0) {
        bad("B1 nothing in the world produces it");
        rows.push({ quest: qid, step, text, where: "-", walk: "-", road: "-", lead: "-", ok: probs.join("; ") });
        return;
      }
      if (targets.every((t) => t.hidden && !w.shown.has(t.id.replace(/^prop:/, "")))) bad("B1 every source is hidden and nothing shows it");
      if (r.type === "kill") {
        const n = targets.length;
        const want = r.qty > 1 ? r.qty + KILL_SPARE : r.qty;
        if (n < want) bad(`B3 ${n} standing, ${want} wanted (${r.qty} + ${KILL_SPARE} spare)`);
      }
      if (r.type === "acquire") {
        const n = targets.reduce((s, t) => s + (t.qty ?? 1), 0);
        if (n < r.qty) bad(`B4 ${n} to be had, ${r.qty} wanted`);
      }
      // The source the player would use: the nearest on foot to the giver, among those near what the text names.
      const scored = targets
        .map((t) => ({ t, steps: walk(w, giver, t) }))
        .filter((s) => s.steps !== null) as { t: Thing; steps: number }[];
      if (scored.length === 0) {
        bad("B2 no source can be walked to from the giver");
        rows.push({ quest: qid, step, text, where: targets[0].id, walk: "cut off", road: "-", lead: "-", ok: probs.join("; ") });
        return;
      }
      const leadOf = (t: Thing): number => {
        const at = inCounty(w, t)!;
        return named.length === 0 ? Infinity : Math.min(...named.map((l) => toRef(w, l.ref, at.cx, at.cy)));
      };
      // Kill and acquire rows name a place; the creatures and things that count are the ones there.
      const near = scored.filter((s) => leadOf(s.t) <= NEAR_LANDMARK);
      const pool = near.length > 0 ? near : scored;
      // Out here first: a rat in the pipes is not the one a notice about the allotments meant.
      // And a thing to be found before a thing some other quest pays out.
      // And one that is standing there before one some trigger may put there instead.
      const rank = (t: Thing): number => (t.hidden ? 4 : 0) + (t.id.startsWith("reward:") ? 2 : 0) + (t.zone !== "county" ? 1 : 0);
      pool.sort((a, b) => rank(a.t) - rank(b.t) || a.steps - b.steps);
      const best = pool[0];
      const at = inCounty(w, best.t)!;
      // Locks on the way into a zone: each needs a key that can be had.
      if (best.t.zone !== "county") {
        for (const tag of w.doorTo.get(best.t.zone)?.locks ?? []) {
          const key = Object.entries(c.items).find(([, it]) => it.opens === tag)?.[0];
          if (!key || !w.items.has(key)) bad(`B2 the way in is locked (${tag}) and no key can be had`);
        }
      }
      // Kill steps: enough of them near the place named.
      if (r.type === "kill" && named.length > 0 && r.qty > 1) {
        const there = targets.filter((t) => leadOf(t) <= NEAR_LANDMARK).length;
        if (there < r.qty) bad(`B3 only ${there} stand near what the step names; ${r.qty} wanted`);
      }

      // A3. every landmark named is written up at the place itself.
      for (const l of named) {
        const where = refPoints(w, l.ref);
        if (!where) {
          bad(`A3 "${l.phrase.source}" refers to something this seed does not have`);
          continue;
        }
        const posted = w.readable.some((t) => {
          const tc = inCounty(w, t);
          if (!tc || !l.says.test(t.text)) return false;
          // Inside a zone, only that zone's own words count; out here, only words near the place.
          if (t.zone !== "county") return "zone" in l.ref && l.ref.zone === t.zone;
          return toRef(w, l.ref, tc.cx, tc.cy) <= POSTED_NEAR;
        });
        if (!posted) bad(`A3 nothing at ${JSON.stringify(l.ref)} says ${l.says.source}: she cannot tell she has arrived`);
      }
      // A4. the hour is in the step.
      const times = new Set(targets.map((t) => t.time));
      if (times.size === 1 && times.has("night") && !NIGHT_WORDS.test(text)) bad("A4 only works after the bell and the step does not say so");
      if (times.size === 1 && times.has("day") && !DAY_WORDS.test(text) && !DAY_WORDS.test(q.description)) bad("A4 only works by day and the quest does not say so");

      // C. FINDABLE
      const road = toRoad(w, at.cx, at.cy);
      const lead = leadOf(best.t);
      const patchSeen = named.some((l) => "area" in l.ref && lead <= NEAR_LANDMARK && patchVisible(w, l.ref.area));
      const landmarkOnMap = named.some((l) => {
        if (toRef(w, l.ref, at.cx, at.cy) > NEAR_LANDMARK) return false;
        const pts = refPoints(w, l.ref);
        if (!pts) return false;
        if ("area" in l.ref) return patchVisible(w, l.ref.area);
        const seen = "site" in l.ref ? PATCH_SEEN_FROM_ROAD : LANDMARK_SEEN_FROM_ROAD;
        return pts.pts.some(([x, y]) => toRoad(w, x, y, 4) <= seen);
      });
      // A step whose source is the giver itself is in her hands already (the case gives the parcel).
      const inHand = best.t.id === giver.id;
      if (!inHand && best.t.zone === "county" && road > ON_SCREEN && !landmarkOnMap && !patchSeen)
        bad(`C1 ${road === Infinity ? "far" : road.toFixed(1)} half-screens from any road, and not beside a landmark the step names that can be seen from one`);
      if (best.steps > budget) bad(`C2 ${best.steps} steps from the giver; a ${tier} quest may ask ${budget}`);
      furthest = Math.max(furthest, best.steps);
      last = best.t;

      // E. what completes it stands where the text points.
      if (r.type === "location" && named.length > 0 && lead > NEAR_LANDMARK && best.t.zone === "county")
        bad(`E1 completed ${lead.toFixed(1)} half-screens from anything the step names`);

      const g = inCounty(w, giver)!;
      rows.push({
        quest: qid,
        step,
        text,
        where: `${best.t.id} ${bearing(g, at)}`,
        walk: String(best.steps),
        road: road === Infinity ? ">8" : road.toFixed(1),
        lead: lead === Infinity ? "-" : lead.toFixed(1),
        ok: probs.length ? probs.map((p) => p.split(" ")[0]).join(",") : "ok",
      });
    });

    // D. HAND-IN
    const lastStep = last as Thing | null; // set inside the forEach above, which the compiler cannot see
    if (handins.length > 0 && lastStep) {
      const last = lastStep;
      const home = handins.map((h) => ({ h, steps: walk(w, last, h) })).sort((a, b) => (a.steps ?? 1e9) - (b.steps ?? 1e9))[0];
      const isGiver = handins.some((h) => h.id === giver.id);
      const isLast = handins.some((h) => h.id === last.id || (inCounty(w, h)?.id === inCounty(w, last)?.id && h.zone === last.zone));
      const returnTo = (q as { returnTo?: string }).returnTo ?? "";
      const words = `${q.description} ${returnTo} ${q.requirements.map((r) => r.text).join(" ")}`;
      const namedHome = handins.some((h) => {
        const said = h.text.split(/[.:!?]/)[0].trim(); // the label or the speaker, as she would read it first
        const who = landmarks.filter((l) => l.phrase.test(words)).some((l) => {
          const hc = inCounty(w, h);
          return hc && toRef(w, l.ref, hc.cx, hc.cy) <= NEAR_LANDMARK;
        });
        return who || (said.length > 3 && words.toLowerCase().includes(said.toLowerCase()));
      });
      const step = "hand-in";
      if (!isGiver && !isLast && !namedHome) fail(qid, step, `D2 taken back by ${handins[0].id}, which is not the giver, not the last step, and not named in the quest`);
      if (!isLast && !returnTo) fail(qid, step, "D2 the log never says who to take it back to (no returnTo)");
      if (home.steps === null) fail(qid, step, `D1 ${home.h.id} cannot be walked to from the last step`);
      else if (home.steps > budget) fail(qid, step, `D3 the walk back is ${home.steps} steps; a ${tier} quest may ask ${budget}`);
      rows.push({
        quest: qid,
        step,
        text: returnTo || (isLast ? "(the last step hands it in)" : "(not said)"),
        where: home.h.id,
        walk: String(home.steps ?? "cut off"),
        road: "",
        lead: "",
        ok: isGiver ? "giver" : isLast ? "last step" : namedHome ? "named" : "-",
      });
    }
    void furthest;
  }
  return { rows, problems };
}

/** A patch she is sent into is its own landmark if its edge comes near enough a road or path to be seen. */
function patchVisible(w: World, area: string): boolean {
  const a = w.areas.get(area);
  if (!a) return false;
  // Sample the rim: the nearest point of the patch to any road.
  for (let k = 0; k < 16; k++) {
    const ang = (k / 16) * Math.PI * 2;
    if (toRoad(w, Math.round(a.cx + Math.cos(ang) * a.r), Math.round(a.cy + Math.sin(ang) * a.r), PATCH_SEEN_FROM_ROAD) <= PATCH_SEEN_FROM_ROAD) return true;
  }
  return false;
}

function table(rows: Row[]): string {
  const cols: (keyof Row)[] = ["quest", "step", "text", "where", "walk", "road", "lead", "ok"];
  const width = cols.map((k) => Math.min(k === "text" ? 60 : 40, Math.max(k.length, ...rows.map((r) => String(r[k]).length))));
  const line = (r: Record<string, string>): string => cols.map((k, n) => String(r[k]).slice(0, width[n]).padEnd(width[n])).join(" | ");
  return [line({ quest: "quest", step: "step", text: "text", where: "where (from giver)", walk: "walk", road: "road", lead: "lead", ok: "ok" }), ...rows.map(line)].join("\n");
}

describe("quest audit: a person can read it, find it, do it and take it back", () => {
  for (const seed of SEEDS) {
    it(`seed ${seed}`, () => {
      const { rows, problems } = audit(seed);
      if (process.env.QUEST_REPORT) console.log(`\n=== seed ${seed} ===\n${table(rows)}`);
      expect(problems, `\n${table(rows)}\n\n${problems.join("\n")}`).toEqual([]);
    }, LONG);
  }

  it("every step's text is short enough for the tracker and every quest says who takes it back", () => {
    // Read as she reads them: a story's {place:...} is a generated place's name (stories.test holds every seed's longest name to these).
    for (const seed of SEEDS) {
      for (const [id, q] of Object.entries(catalog.quests)) {
        for (const r of q.requirements) expect(say(seed, r.text).length, `${id}: "${say(seed, r.text)}"`).toBeLessThanOrEqual(70);
        expect(say(seed, q.description).length, `${id}: description`).toBeLessThanOrEqual(SIDE.has(id) ? 320 : 400);
        expect(say(seed, (q as { returnTo?: string }).returnTo ?? "").length, `${id}: returnTo`).toBeLessThanOrEqual(70);
      }
    }
    for (const [id, q] of Object.entries(catalog.quests)) {
      // The tracker says "Back to <returnTo>" once every step is done (ui/hud.ts), so it reads as a phrase.
      const back = (q as { returnTo?: string }).returnTo ?? "";
      expect(back.length, `${id}: returnTo`).toBeGreaterThan(0);
      expect(back, `${id}: returnTo starts lower case, it follows "Back to"`).toMatch(/^(the |[A-Z][a-z]+'s |[A-Z][a-z]+, |Pell's |Julie's |Mrs |Miss |Mr )/);
      expect(back.length, `${id}: returnTo`).toBeLessThanOrEqual(70);
    }
  });

  it("the tracks and paths the quests send her up never cross water (a stroke of dirt over a river is a ford)", () => {
    for (const seed of SEEDS) {
      const w = world(seed);
      const sk = countySkeleton(seed, w.bps.county.attempts - 1);
      for (const [id, line] of w.paths) {
        const wet = line.filter(([x, y]) => sk.water[(y >> 4) * SKEL_W + (x >> 4)]).length;
        expect(wet, `seed ${seed}: path ${id} crosses ${wet} cells of water`).toBe(0);
      }
    }
  }, LONG);

  // The audit has to be able to fail. Each of these is a quest broken in one way the user described
  // ("confusing"), run through the same checks against a real county: each must be caught, by the right rule.
  it("catches a quest that is vague, secret, impossible, or silent about the hour", () => {
    const base = catalog.quests;
    const broken = (id: string, reqs: Catalog["quests"][string]["requirements"]): Catalog["quests"] => ({ [id]: { ...base[id], requirements: reqs } });
    const caught = (quests: Catalog["quests"], rule: string): void => {
      const { problems } = audit(SEEDS[0], quests);
      expect(problems.some((p) => p.includes(rule)), `expected ${rule} in:\n${problems.join("\n")}`).toBe(true);
    };
    // A bare noun: where is "the glove"?
    caught(broken("lost_property", [{ type: "acquire", target: "lost_glove", qty: 1, text: "The glove" }]), "A2");
    // An id showing through.
    caught(broken("lost_property", [{ type: "acquire", target: "lost_glove", qty: 1, text: "A lost_glove, at the well on the station road" }]), "A1");
    // Pointed at the wrong place: the trunk is on the platform, and the step says the Long Hedge.
    caught(broken("lost_property", [{ type: "location", target: "trunk_open", qty: 1, text: "Open it, by the Long Hedge" }]), "E1");
    // Nothing produces it.
    caught(broken("lost_property", [{ type: "location", target: "nowhere_at_all", qty: 1, text: "The well on the station road" }]), "B1");
    // More than stand in the world.
    caught(broken("rats_in_the_sheds", [{ type: "kill", target: "plot_tenant", qty: 3, text: "Whoever digs Plot 9, in the allotments, after the bell" }]), "B3");
    // Night-only, and the step does not say so.
    caught(broken("plot_nine", [{ type: "location", target: "plot_tenant_down", qty: 1, text: "Whoever digs Plot 9, in the allotments" }]), "A4");
    // Too far for what it is: the mine itself, asked by a "near" errand on the platform. (It was the carter's
    // cart by the mine until the county came in to 2 km square, Sept 24; on seed 3 the cart is inside a near walk now.)
    caught(broken("to_be_collected", [{ type: "location", target: "mine", qty: 1, text: "The Gold Mine, at the end of the mine road" }]), "C2");
  }, LONG);
});
