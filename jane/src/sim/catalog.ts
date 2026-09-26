// Catalog: every content row the sim reads, typed, validated once at boot.
//
// Rule carried over from SYSTEMS.md: a new item, spell, unit, effect, recipe,
// quest, prop, dialogue node or trigger is a JSON row. No class per thing.
//
// Rule added by this build: a bad row is a boot error, not a runtime surprise.
// `validateCatalog` checks every cross-reference (spell -> effect, unit -> spell,
// quest -> item, action -> anything) and reports all of them at once.

import { TICK_RATE } from "@/sim/constants";
import type { Action, ActionList, Anim, BarSlot, Condition, Controller, Faction, School, Stack } from "@/sim/state";
import { SCHOOLS } from "@/sim/state";

import clockJson from "@/data/clock.json";
import dialogueJson from "@/data/dialogue.json";
import effectsJson from "@/data/effects.json";
import itemsJson from "@/data/items.json";
import propsJson from "@/data/props.json";
import questsJson from "@/data/quests.json";
import recipesJson from "@/data/recipes.json";
import spellsJson from "@/data/spells.json";
import startJson from "@/data/start.json";
import triggersJson from "@/data/triggers.json";
import unitsJson from "@/data/units.json";

/** `ally`: lands on the friend nearest the aim line, or on the caster when nobody is there. Heals are this kind. */
export type SpellKind = "melee" | "bolt" | "self" | "ally" | "world" | "ground";

/** amount = stat/div + irandom(stat/varDiv) + flat. 2020 melee: strength/8 + irandom(strength/32). */
export type Power = { stat: "strength" | "spirit"; div: number; varDiv: number; flat?: number };

export type SpellDef = {
  name: string;
  description: string;
  icon: string;
  kind: SpellKind;
  school: School;
  mp: number;
  energy: number;
  /** Metres. Melee reach for kind=melee. */
  range: number;
  /** Seconds in JSON, ticks after load. */
  cooldown: number;
  gcdImmune: boolean;
  needsTarget: boolean;
  needsEnemy: boolean;
  needsLos: boolean;
  anim: Anim;
  power?: Power;
  /** Bolt speed, px per tick. */
  speed?: number;
  /** Bolts per cast and the arc they cover in degrees (360 = ring, evenly spaced). */
  count?: number;
  fan?: number;
  /** 2020 bolts splashed hit/div on other enemies within `radius` px of the impact. */
  splash?: { radius: number; div: number };
  /** Effect applied to whoever the spell lands on (or the caster, for kind=self). */
  effect?: string;
  /** Energy handed back to the caster when a melee lands (`RestoreENERGY`). */
  restoreEnergy?: number;
  /** Ground: radius in metres, lifetime and pulse in seconds (ticks after load). */
  radius?: number;
  duration?: number;
  pulse?: number;
  /** World verbs: what the spell does to the prop in front of the caster. */
  world?: "repair" | "grow";
  /** Ticks the caster is held still after casting. */
  stop: number;
  /** Light carried by the projectile, px. */
  glow?: number;
  /**
   * How close to a prop's middle the bolt must end to switch on a prop that answers its school,
   * px. 14 when left out: an icebolt has to find the torch. A blast is wider (28), because it
   * must reach the middle of a 3 x 2 pile of rubble from whichever side it lands on.
   */
  touch?: number;
};

export type EffectDef = {
  name: string;
  icon: string;
  /** Seconds in JSON, ticks after load. 0 = instant. */
  duration: number;
  harmful: boolean;
  /** Movement multiplier, 0..1. 0 roots. */
  speed?: number;
  /** Cannot act at all. */
  stun?: boolean;
  /** Periodic pulse: amount per pulse, seconds between pulses. Heal school heals. */
  pulse?: { amount: number; every: number; school: School };
  /** Instant heal/mana on apply. */
  heal?: number;
  mana?: number;
  /** Incoming damage multiplier per school (0.5 = half). */
  resist?: Partial<Record<School, number>>;
  /** Damage is paid from MP first, at this MP-per-damage rate. */
  manaShield?: number;
  /** Fraction of damage dealt returned as health. */
  lifesteal?: number;
  /** Extra 1-in-N crit roll replaced by this (2 = every other hit). */
  critOneIn?: number;
  /** Melee hits add this. */
  onMelee?: { school: School; amount: number; effect?: string };
  /** Taking a hit restores this much MP (2020 production board: "taking hits restores mana"). */
  manaOnHit?: number;
  /**
   * Lands only on a unit whose own row is weak to this school (resist below 0). `jolted` is
   * this: a spark stops a machine for half a second and does nothing to a rat.
   */
  onlyIfWeak?: School;
  /** While it lasts, the unit row's own resists count for nothing (weaknesses stay). Fire opens Goldskin. */
  noResist?: boolean;
};

export type LootRoll = { item: string; qty: number; chance: number };

export type UnitDef = {
  name: string;
  faction: Faction;
  controller: Controller;
  strength: number;
  spirit: number;
  /** px per tick. 2020 player: walk 1, run 2. */
  walk: number;
  run: number;
  /** Metres. */
  aggro: number;
  leash: number;
  /** Body radius in metres ("boundbox"). Range is measured between bounds. */
  bounds: number;
  book: string[];
  /** Seconds in JSON, ticks after load. 0 = never. */
  respawn: number;
  autoRegen: boolean;
  loot: LootRoll[];
  resist?: Partial<Record<School, number>>;
  sprite: string;
  onDeath?: ActionList;
  talk?: string;
  boss?: boolean;
  /** Light the unit carries (the 2020 bat wore a red one). */
  glow?: { radius: number; color: string };
  /** Present only between 06:00 and 21:00. The dog is never seen after dark. */
  dayOnly?: boolean;
  /**
   * `dayOnly` holds only once this quest is done. The dog keeps Julie's hours, but not on the night
   * {name} first arrives: a player who dawdles at the Halt and reaches the step after nine must still
   * find someone on it, or the story has nobody to start it.
   */
  dayOnlyAfter?: string;
  /** The mirror: not there between 06:00 and 21:00. Never seen arriving or leaving. */
  nightOnly?: boolean;
  /**
   * Where it is by the hour (WORLD.md §3): one slot per span, `from` up to `to`, wrapping midnight.
   * This build keeps only in or out: `inside` and `absent` are away, `mark` and `patrol` are here
   * (the Rust build also walks a unit to its mark).
   */
  schedule?: { from: number; to: number; mark?: string; inside?: string; patrol?: boolean; absent?: boolean }[];
  /** Item id this unit cannot resist: while idle it walks to a drop of it and dies there. */
  bait?: string;
  /** Snake controller only: body segment count and spacing in px. */
  body?: { segments: number; spacing: number };
  /**
   * Spell lists by phase for bosses, in falling order of `hpBelow` (a fraction of full health).
   * A row is entered when health falls to it: the unit takes its book (and `run`), and `onEnter`
   * runs once, with the boss as the subject, on behalf of whoever landed the blow: the Attendant
   * throws the breaker at 75%. Until the first row is entered the unit is its own row. A boss
   * that gets all its health back (a leash, a respawn) starts again, and its lists will run again.
   * (The snake reads this table by its own clock: sim/snake.ts.)
   */
  phases?: { hpBelow: number; book: string[]; run?: number; onEnter?: ActionList }[];
  /** `lit`: it only notices, and only keeps, a target standing in a prop's light. The Factory's sentries. */
  sight?: "lit";
  /** It will not step into warm light: it walks to the edge of it and waits there. The Burial's shades. */
  shunsLight?: boolean;
};

export type ItemDef = {
  name: string;
  description: string;
  icon: string;
  maxStack: number;
  usable: boolean;
  /** Seconds in JSON, ticks after load. */
  cooldown: number;
  /** Runs on the user. Consumes one unless `keep`. */
  use?: ActionList;
  keep?: boolean;
  /** Key tag: unlocks any prop whose keyTag matches. One use path for every key. */
  opens?: string;
  /** Story items: cannot be destroyed or dropped. */
  bound?: boolean;
};

export type RecipeDef = { inputs: string[]; output: string; qty: number };

export type QuestReq = { type: "kill" | "acquire" | "location"; target: string; qty: number; text: string };
export type QuestDef = {
  name: string;
  description: string;
  completion: string;
  requirements: QuestReq[];
  rewards: ActionList;
};

export type PropDef = {
  name: string;
  sprite: string;
  /** Footprint in cells. */
  w: number;
  h: number;
  solid: boolean;
  blockLos: boolean;
  push?: boolean;
  carry?: boolean;
  bench?: boolean;
  /**
   * What switches this prop on: a world verb (Repair, Grow) or a damage school
   * (2020's wall torches lit on frost damage). Its `use` list runs when it lands.
   */
  answers?: "repair" | "grow" | School;
  /** `use` runs only the first time. */
  once?: boolean;
  /** A door that leads nowhere: solid while locked, open once unlocked. */
  gate?: boolean;
  /** Pressure plate: `use` when a unit or pushable covers it, `release` when clear. */
  plate?: boolean;
  /** Light shows only while the prop is `on`. */
  lightWhenOn?: boolean;
  /** Light shows only between 18:30 and 06:30, like the 2020 lamp posts. */
  nightOnly?: boolean;
  /** Light shows only between 06:30 and 18:30: a glade the sun comes down into. The other half of `nightOnly`. */
  dayOnly?: boolean;
  /** A bed or a fire. The game can only be saved within reach of one, and dying wakes you at the last one used. */
  rest?: boolean;
  /** Gathered things vanish once looted. */
  hideWhenUsed?: boolean;
  /**
   * `cold`: light that shows what is there and keeps nothing off (the Burial's blue torches).
   * Something that `shunsLight` walks straight through it. Everything else about it is light.
   */
  light?: { radius: number; color: string; flicker: number; cold?: boolean };
  /** A thing on the ground that is drawn as what it holds: its first loot item's icon, at ground scale. The sprite is only for when it holds nothing. */
  showsLoot?: boolean;
  /** Draw order bias: floor decals draw under units. */
  flat?: boolean;
  prompt?: string;
  /**
   * What is said of it, after its label, when it is tried while locked: "is locked" unless set. A
   * page held down by a mechanism "is held fast", a jar behind a grille "is out of reach": only a
   * thing with a lock is locked.
   */
  lockedSays?: string;
};

export type DialogueOption = { label: string; goto?: string; actions?: ActionList };
export type DialogueNode = { lines: string[]; options?: DialogueOption[]; actions?: ActionList; goto?: string };
export type DialogueTree = {
  speaker: string;
  start: { when?: Condition[]; node: string }[];
  nodes: Record<string, DialogueNode>;
};

export type TriggerDef = {
  zone: string;
  /** Named rect in the zone blueprint. */
  rect: string;
  /** "enter" fires on walking in; "while" fires once inside AND `when` holds. Default enter. */
  mode?: "enter" | "while";
  once: boolean;
  when?: Condition[];
  actions: ActionList;
  /**
   * Undo list, run when the player dies after this trigger fired; the trigger then
   * re-arms. Without it a lock-in would leave the respawned player outside a shut gate.
   */
  reset?: ActionList;
};

/** Something the whole county does at an hour of the day: the bell at nine. */
export type ClockDef = { hour: number; actions: ActionList };

/** `mark`: where in the county New Game stands her. The station; tests move it to Julie's gate. */
export type StartDef = { items: Stack[]; bar: BarSlot[]; quests: string[]; mark: string };

export type Catalog = {
  spells: Record<string, SpellDef>;
  effects: Record<string, EffectDef>;
  items: Record<string, ItemDef>;
  units: Record<string, UnitDef>;
  recipes: RecipeDef[];
  recipeIndex: Record<string, RecipeDef>;
  quests: Record<string, QuestDef>;
  props: Record<string, PropDef>;
  dialogue: Record<string, DialogueTree>;
  triggers: Record<string, TriggerDef>;
  clock: ClockDef[];
  start: StartDef;
  /**
   * Derived: what the story cannot go on without. Keys, and anything a quest asks the
   * party to acquire. These never age out on the ground, and when the person holding
   * one leaves the table it is handed to someone who is staying.
   */
  storyItems: Set<string>;
};

const seconds = (s: number): number => Math.round(s * TICK_RATE);

/** Recipes key by sorted item ids. 2020 keyed by display name; renaming Pansy killed the recipe. */
export function recipeKey(inputs: readonly string[]): string {
  return [...inputs].sort().join("+");
}

/**
 * Content comes in one base file per table (data/quests.json) plus any number of
 * FRAGMENTS beside it (data/quests/lowfields.json, data/units/museum.json...). A
 * fragment is the same shape as its base and is merged in, in path order, so a region's
 * quests or a dungeon's creatures live in a file of their own and two people can add
 * content without touching the same file. An id defined twice is a boot error.
 */
const FRAGMENTS = import.meta.glob("../data/*/*.json", { eager: true, import: "default" }) as Record<string, unknown>;

function fragmentsOf(table: string): { path: string; rows: unknown }[] {
  return Object.keys(FRAGMENTS)
    .filter((p) => p.split("/").at(-2) === table)
    .sort()
    .map((path) => ({ path, rows: FRAGMENTS[path] }));
}

function mergedTable<T>(table: string, base: unknown): Record<string, T> {
  const out = structuredClone(base) as Record<string, T>;
  for (const { path, rows } of fragmentsOf(table)) {
    for (const [id, row] of Object.entries(structuredClone(rows) as Record<string, T>)) {
      if (id in out) throw new Error(`${table}: "${id}" is defined twice (again in ${path})`);
      out[id] = row;
    }
  }
  return out;
}

function mergedList<T>(table: string, base: unknown): T[] {
  const out = structuredClone(base) as T[];
  for (const { rows } of fragmentsOf(table)) out.push(...(structuredClone(rows) as T[]));
  return out;
}

/** Footprint of every prop def, in cells. The county builder needs it to place props by name without building a whole catalog. */
export function propFootprints(): Record<string, { w: number; h: number }> {
  const out: Record<string, { w: number; h: number }> = {};
  for (const [id, p] of Object.entries(mergedTable<PropDef>("props", propsJson))) out[id] = { w: p.w, h: p.h };
  return out;
}

export function buildCatalog(): Catalog {
  const spells = mergedTable<SpellDef>("spells", spellsJson);
  for (const s of Object.values(spells)) {
    s.cooldown = seconds(s.cooldown);
    if (s.duration !== undefined) s.duration = seconds(s.duration);
    if (s.pulse !== undefined) s.pulse = seconds(s.pulse);
    s.stop = s.stop ?? 0;
  }
  const effects = mergedTable<EffectDef>("effects", effectsJson);
  for (const e of Object.values(effects)) {
    e.duration = seconds(e.duration);
    if (e.pulse) e.pulse.every = Math.max(1, seconds(e.pulse.every));
  }
  const items = mergedTable<ItemDef>("items", itemsJson);
  for (const i of Object.values(items)) i.cooldown = seconds(i.cooldown);
  const units = mergedTable<UnitDef>("units", unitsJson);
  for (const u of Object.values(units)) {
    u.respawn = seconds(u.respawn);
    u.bounds = u.bounds ?? 1;
  }
  const recipes = mergedList<RecipeDef>("recipes", recipesJson);
  const recipeIndex: Record<string, RecipeDef> = {};
  for (const r of recipes) recipeIndex[recipeKey(r.inputs)] = r;

  const catalog: Catalog = {
    spells,
    effects,
    items,
    units,
    recipes,
    recipeIndex,
    quests: mergedTable<QuestDef>("quests", questsJson),
    props: mergedTable<PropDef>("props", propsJson),
    dialogue: mergedTable<DialogueTree>("dialogue", dialogueJson),
    triggers: mergedTable<TriggerDef>("triggers", triggersJson),
    clock: mergedList<ClockDef>("clock", clockJson),
    start: structuredClone(startJson) as unknown as StartDef,
    storyItems: new Set<string>(),
  };
  for (const [id, item] of Object.entries(items)) if (item.opens) catalog.storyItems.add(id);
  for (const q of Object.values(catalog.quests)) {
    for (const r of q.requirements) if (r.type === "acquire") catalog.storyItems.add(r.target);
  }
  const errors = validateCatalog(catalog);
  if (errors.length > 0) throw new Error(`Catalog has ${errors.length} error(s):\n  ${errors.join("\n  ")}`);
  return catalog;
}

type Need = (ok: boolean, msg: string) => void;
const isSchool = (s: unknown): boolean => SCHOOLS.includes(s as School);

/**
 * Every action in a list, and in the lists inside it (`if`'s branches, what a sent unit does
 * on arrival), in written order. Anything that reads lists for what they COULD do (the solver,
 * the dungeon checks, the quest tests) walks them with this, so a `learn` under an `if` is still found.
 */
export function eachAction(list: ActionList | undefined | null, fn: (a: Action) => void): void {
  for (const a of list ?? []) {
    fn(a);
    if (a.do === "if") {
      eachAction(a.then, fn);
      eachAction(a.else, fn);
    } else if (a.do === "send") eachAction(a.then, fn);
  }
}

function checkAction(c: Catalog, need: Need, where: string, a: Action): void {
  switch (a.do) {
    case "quest":
    case "handin":
      need(a.quest in c.quests, `${where}: unknown quest "${a.quest}"`);
      break;
    case "give":
    case "take":
      need(a.item in c.items, `${where}: unknown item "${a.item}"`);
      break;
    case "learn":
      need(a.spell in c.spells, `${where}: unknown spell "${a.spell}"`);
      break;
    case "status":
      need(a.effect in c.effects, `${where}: unknown effect "${a.effect}"`);
      break;
    case "spawn":
      need(a.def in c.units, `${where}: unknown unit def "${a.def}"`);
      break;
    case "talk":
      need(a.tree in c.dialogue, `${where}: unknown dialogue "${a.tree}"`);
      break;
    case "throw":
      need(a.item in c.items, `${where}: unknown item "${a.item}"`);
      break;
    case "strike":
      need(isSchool(a.school) && a.school !== "heal" && a.amount > 0 && typeof a.rect === "string" && a.rect !== "", `${where}: strike needs a rect, an amount and a damage school`);
      if (a.effect) need(a.effect in c.effects, `${where}: unknown effect "${a.effect}"`);
      break;
    case "grow":
      need((a.stat === "strength" || a.stat === "spirit") && a.amount > 0 && typeof a.id === "string" && a.id !== "", `${where}: grow needs a stat, an amount and the id of the thing found`);
      break;
    case "if":
      need(Array.isArray(a.when) && a.when.length > 0 && Array.isArray(a.then), `${where}: if needs a "when" and a "then"`);
      need(a.else === undefined || Array.isArray(a.else), `${where}: if: "else" must be a list`);
      checkConditionRows(c, need, where, Array.isArray(a.when) ? a.when : []);
      for (const b of Array.isArray(a.then) ? a.then : []) checkAction(c, need, `${where} (then)`, b);
      for (const b of Array.isArray(a.else) ? a.else : []) checkAction(c, need, `${where} (else)`, b);
      break;
    case "send":
      need(typeof a.unit === "string" && a.unit !== "" && typeof a.to === "string" && a.to !== "", `${where}: send needs a unit and a mark to send it to`);
      need(a.then === undefined || Array.isArray(a.then), `${where}: send: "then" must be a list`);
      for (const b of Array.isArray(a.then) ? a.then : []) checkAction(c, need, `${where} (then)`, b);
      break;
    case "reveal":
      need(Array.isArray(a.rects) && a.rects.length > 0 && a.rects.every((r) => typeof r === "string" && r !== ""), `${where}: reveal needs a list of rects`);
      break;
    default:
      break;
  }
}

function checkConditionRows(c: Catalog, need: Need, where: string, list: Condition[] | undefined): void {
  for (const k of list ?? []) {
    if (k.if === "questActive" || k.if === "questReady" || k.if === "questDone") {
      need(k.quest in c.quests, `${where}: unknown quest "${k.quest}"`);
    } else if (k.if === "hasItem") {
      need(k.item in c.items, `${where}: unknown item "${k.item}"`);
    } else if (k.if === "knows") {
      need(k.spell in c.spells, `${where}: unknown spell "${k.spell}"`);
    }
  }
}

/**
 * The same row checks for lists that do not live in the catalog: the triggers a blueprint
 * carries and the `use` lists of generated props (world/validate.ts asks). Empty means clean.
 */
export function actionRowErrors(c: Catalog, where: string, list: ActionList | undefined, when?: Condition[]): string[] {
  const errors: string[] = [];
  const need: Need = (ok, msg) => {
    if (!ok) errors.push(msg);
  };
  for (const a of list ?? []) checkAction(c, need, where, a);
  checkConditionRows(c, need, where, when);
  return errors;
}

/** Every dangling id, wrong type and duplicate recipe, as readable strings. Empty means clean. */
export function validateCatalog(c: Catalog): string[] {
  const errors: string[] = [];
  const need: Need = (ok, msg) => {
    if (!ok) errors.push(msg);
  };

  const checkActions = (where: string, list: ActionList | undefined): void => {
    for (const a of list ?? []) checkAction(c, need, where, a);
  };
  const checkConditions = (where: string, list: Condition[] | undefined): void => checkConditionRows(c, need, where, list);

  for (const [id, s] of Object.entries(c.spells)) {
    const at = `spells.${id}`;
    need(["melee", "bolt", "self", "ally", "world", "ground"].includes(s.kind), `${at}: bad kind "${s.kind}"`);
    need(isSchool(s.school), `${at}: bad school "${s.school}"`);
    need(s.range >= 0 && s.mp >= 0 && s.energy >= 0 && s.cooldown >= 0, `${at}: negative number`);
    if (s.effect) need(s.effect in c.effects, `${at}: unknown effect "${s.effect}"`);
    if (s.kind === "bolt") need((s.speed ?? 0) > 0, `${at}: bolt needs speed`);
    if (s.kind === "ground") need((s.radius ?? 0) > 0 && (s.duration ?? 0) > 0, `${at}: ground needs radius and duration`);
    if (s.kind === "world") need(s.world === "repair" || s.world === "grow", `${at}: world spell needs a verb`);
    if (s.touch !== undefined) need(s.kind === "bolt" && s.touch > 0, `${at}: touch is a bolt's, in px`);
    if (s.kind === "melee" || s.kind === "bolt") need(!!s.power, `${at}: needs power`);
    if (s.power) need(s.power.div > 0 && s.power.varDiv > 0, `${at}: power divisors must be > 0`);
  }
  for (const [id, e] of Object.entries(c.effects)) {
    const at = `effects.${id}`;
    if (e.pulse) need(isSchool(e.pulse.school), `${at}: bad pulse school`);
    if (e.onMelee) {
      need(isSchool(e.onMelee.school), `${at}: bad onMelee school`);
      if (e.onMelee.effect) need(e.onMelee.effect in c.effects, `${at}: unknown onMelee effect`);
    }
    for (const k of Object.keys(e.resist ?? {})) need(isSchool(k), `${at}: bad resist school "${k}"`);
    if (e.onlyIfWeak !== undefined) need(isSchool(e.onlyIfWeak) && e.onlyIfWeak !== "heal", `${at}: bad onlyIfWeak school "${e.onlyIfWeak}"`);
  }
  for (const [id, i] of Object.entries(c.items)) {
    const at = `items.${id}`;
    need(i.maxStack >= 1, `${at}: maxStack < 1`);
    checkActions(at, i.use);
    need(!i.usable || !!i.use || !!i.opens, `${at}: usable but has neither use[] nor opens`);
  }
  for (const [id, u] of Object.entries(c.units)) {
    const at = `units.${id}`;
    need(u.strength > 0, `${at}: strength must be > 0`);
    // What bites looks only at the party's bodies for a target (sim/ai.ts nearestEnemy): a friendly row that fights would be invisible to it.
    need(u.faction !== "friendly" || u.controller === "npc" || u.controller === "player", `${at}: a friendly unit is a person or the player (npc or player controller)`);
    for (const s of u.book) need(s in c.spells, `${at}: unknown spell "${s}"`);
    for (const p of u.phases ?? []) for (const s of p.book) need(s in c.spells, `${at}: unknown phase spell "${s}"`);
    (u.phases ?? []).forEach((p, n) => {
      need(p.hpBelow > 0 && p.hpBelow <= 1, `${at}: phase ${n}: hpBelow is a fraction of full health`);
      checkActions(`${at}.phases[${n}].onEnter`, p.onEnter);
    });
    need(u.sight === undefined || u.sight === "lit", `${at}: sight is "lit" or nothing`);
    for (const l of u.loot) need(l.item in c.items, `${at}: unknown loot "${l.item}"`);
    for (const k of Object.keys(u.resist ?? {})) need(isSchool(k), `${at}: bad resist school "${k}"`);
    if (u.talk) need(u.talk in c.dialogue, `${at}: unknown dialogue "${u.talk}"`);
    if (u.controller === "snake") need(!!u.body, `${at}: snake controller needs body`);
    if (u.bait) need(u.bait in c.items, `${at}: unknown bait "${u.bait}"`);
    checkActions(at, u.onDeath);
  }
  const seen = new Set<string>();
  c.recipes.forEach((r, n) => {
    const at = `recipes[${n}]`;
    need(r.inputs.length >= 1 && r.inputs.length <= 3, `${at}: 1..3 inputs`);
    for (const i of r.inputs) need(i in c.items, `${at}: unknown input "${i}"`);
    need(r.output in c.items, `${at}: unknown output "${r.output}"`);
    const key = recipeKey(r.inputs);
    need(!seen.has(key), `${at}: duplicate recipe ${key}`);
    seen.add(key);
  });
  for (const [id, q] of Object.entries(c.quests)) {
    const at = `quests.${id}`;
    need(q.requirements.length > 0, `${at}: no requirements`);
    for (const r of q.requirements) {
      if (r.type === "kill") need(r.target in c.units, `${at}: unknown kill target "${r.target}"`);
      if (r.type === "acquire") need(r.target in c.items, `${at}: unknown item "${r.target}"`);
      need(r.qty >= 1, `${at}: qty < 1`);
    }
    checkActions(at, q.rewards);
  }
  for (const [id, t] of Object.entries(c.dialogue)) {
    const at = `dialogue.${id}`;
    need(t.start.length > 0, `${at}: no start`);
    need(!t.start[t.start.length - 1]?.when, `${at}: last start entry must be unconditional`);
    for (const s of t.start) {
      need(s.node in t.nodes, `${at}: start -> unknown node "${s.node}"`);
      checkConditions(at, s.when);
    }
    for (const [nid, node] of Object.entries(t.nodes)) {
      need(node.lines.length > 0, `${at}.${nid}: no lines`);
      need((node.options?.length ?? 0) <= 2, `${at}.${nid}: at most 2 options`);
      if (node.goto) need(node.goto in t.nodes, `${at}.${nid}: goto unknown node "${node.goto}"`);
      checkActions(`${at}.${nid}`, node.actions);
      for (const o of node.options ?? []) {
        if (o.goto) need(o.goto in t.nodes, `${at}.${nid}: option -> unknown node "${o.goto}"`);
        checkActions(`${at}.${nid}`, o.actions);
      }
    }
  }
  for (const [id, t] of Object.entries(c.triggers)) {
    checkConditions(`triggers.${id}`, t.when);
    checkActions(`triggers.${id}`, t.actions);
    checkActions(`triggers.${id}.reset`, t.reset);
  }
  for (const [id, p] of Object.entries(c.props)) {
    need(p.w >= 1 && p.h >= 1, `props.${id}: footprint < 1`);
    if (p.answers !== undefined) need(p.answers === "repair" || p.answers === "grow" || (isSchool(p.answers) && p.answers !== "heal"), `props.${id}: answers "${p.answers}" is not a world verb or a damage school`);
    if (p.lightWhenOn || p.nightOnly || p.dayOnly) need(!!p.light, `props.${id}: light flag without a light`);
    need(!(p.nightOnly && p.dayOnly), `props.${id}: a light cannot be both nightOnly and dayOnly`);
  }
  c.clock.forEach((row, n) => {
    need(row.hour >= 0 && row.hour < 24, `clock[${n}]: hour out of range`);
    checkActions(`clock[${n}]`, row.actions);
  });
  for (const s of c.start.items) need(s.item in c.items, `start: unknown item "${s.item}"`);
  for (const q of c.start.quests) need(q in c.quests, `start: unknown quest "${q}"`);
  for (const b of c.start.bar) {
    if (b) need(b.source === "spell" ? b.id in c.spells : b.id in c.items, `start: unknown bar id "${b.id}"`);
  }
  return errors;
}
