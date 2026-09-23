// The whole game is this tree. It is plain JSON-able data: no classes, no Maps,
// no references between objects (ids only). That is what makes the three hard
// things cheap:
//   save      = JSON.stringify(state)            (one schema, not a second one)
//   determinism = hash(state) after N ticks is a function of (seed, inputs)
//   headless  = the sim needs nothing from the browser
//
// Anything derived (grid flags, occupancy, path scratch, spatial buckets) lives
// in ZoneRuntime (runtime.ts) and is rebuilt from this tree, never saved.

import type { RngState } from "@/sim/rng";

/**
 * A zone's name. It was a closed union of five; it is a string now, because a dungeon is
 * a file dropped into world/zones/ and the list of zones is whatever world/index.ts finds
 * (ZONE_IDS lives there). Misspelt ids are caught where they are used: travel to an
 * unknown zone is a boot error in the catalog's checks and in the blueprint validator.
 */
export type ZoneId = string;

export type Faction = "undead" | "beast" | "bandit" | "friendly";
/** `blast` is Explosion's school and `shock` is Electric's: each is also what a prop may `answers` to. */
export type School = "heal" | "physical" | "frost" | "fire" | "nature" | "blast" | "shock";
export const SCHOOLS: readonly School[] = ["heal", "physical", "frost", "fire", "nature", "blast", "shock"];

/** Who drives the unit. One Unit shape; the controller is a field, as in 2020. */
export type Controller = "player" | "ai" | "npc" | "snake";
export type CombatState = "idle" | "combat" | "leash";
export type Anim = "idle" | "walk" | "attack" | "cast" | "hurt" | "dead";

/** 0 east, 1 south, 2 west, 3 north. The sim never needs trigonometry. */
export type Facing = 0 | 1 | 2 | 3;
export const FACING_DX: readonly number[] = [1, 0, -1, 0];
export const FACING_DY: readonly number[] = [0, 1, 0, -1];

export type Stack = { item: string; qty: number };

export type Hit = {
  amount: number;
  school: School;
  from: number; // unit id, 0 = the world
  crit: boolean;
  status?: string; // effect id applied if the hit lands
};

export type StatusInst = {
  effect: string;
  left: number; // ticks
  nextTick: number; // ticks until the next periodic pulse (dot/hot)
  from: number;
  pool: number; // absorb remaining, 0 when unused
};

export type Unit = {
  id: number;
  /** Stable blueprint key ("dog", "yard_skeleton"). Story, triggers and saves address units by key. */
  key: string;
  def: string; // row in units.json
  controller: Controller;
  faction: Faction;
  x: number;
  y: number;
  facing: Facing;
  strength: number;
  spirit: number;
  hp: number;
  mp: number;
  energy: number;
  energyLocked: boolean;
  alive: boolean;
  anim: Anim;
  animTick: number;
  gcd: number;
  stop: number; // ticks rooted in place after a cast ("stop_timer")
  cooldowns: Record<string, number>;
  itemCooldowns: Record<string, number>;
  book: string[];
  bag: (Stack | null)[] | null; // only units that carry things pay for 24 slots
  target: number;
  combat: CombatState;
  homeX: number;
  homeY: number;
  patrol: number[] | null; // flat [x0,y0,x1,y1,...] in px
  patrolAt: number;
  /** Ticks to stand at each patrol point before walking on, one per point. Null = never waits. */
  patrolDwell: number[] | null;
  /** Ticks left to stand at the point it has just reached. */
  dwell: number;
  /** Somewhere it has been sent (`send`). While it has one it walks there and minds nothing else. */
  order: UnitOrder | null;
  path: number[] | null; // cell indices
  /** Cell the current path was planned toward. A partial path does not end on it. */
  pathGoal: number;
  pathAt: number;
  repathIn: number;
  thinkOffset: number; // staggers the aggro scan
  incoming: Hit[];
  statuses: StatusInst[];
  deadFor: number;
  respawn: number; // ticks; 0 = stays dead
  awake: boolean;
  /** Not in the world right now (the dog after dark). Not drawn, not talked to, not targeted. */
  hidden: boolean;
  carrying: number; // prop id, 0 = hands free
  hold: number; // ticks USE has been held against a pushable
  segments: number[] | null; // snake body: flat trail [x,y,...], newest first
  /** Heading unit vector. Only steered movers (the snake) turn it gradually. */
  hx: number;
  hy: number;
  phase: number;
  phaseTick: number;
  phaseStep: number;
};

export type PropLight = { radius: number; color: string; flicker: number };

/**
 * A unit told to walk somewhere by the `send` verb. It lives on the unit, so a save taken
 * mid-walk loads into the same walk. `seat` is whoever caused it (-1 for nobody: a clock
 * row, a plate), and `then` runs on her behalf, with the unit as its subject, on arrival.
 */
export type UnitOrder = { x: number; y: number; left: number; then: ActionList | null; seat: number };

/**
 * Props are capability rows, not classes. A door is `solid + locked + to`.
 * A chest is `solid + loot`. A lever is `use`. A crate is `solid + push`.
 */
export type Prop = {
  id: number;
  key: string;
  def: string; // row in props.json
  cx: number;
  cy: number;
  solid: boolean;
  hidden: boolean;
  locked: boolean;
  used: boolean;
  on: boolean; // lever / light / lily state
  keyTag: string; // item `opens` tag needed while locked
  to: { zone: ZoneId; mark: string } | null;
  loot: Stack[] | null;
  use: ActionList | null;
  /** Pressure plates: runs when the last thing steps off. */
  release: ActionList | null;
  /** Materials a world verb (Repair) consumes. */
  needs: Stack[] | null;
  talk: string;
  label: string;
  /** Said instead of opening, between nine and six. "" = an ordinary door. */
  nightLock: string;
  awake: boolean;
};

export type Drop = { id: number; item: string; qty: number; x: number; y: number; age: number };

export type Projectile = {
  id: number;
  spell: string;
  from: number;
  target: number; // 0 = fired along a direction
  x: number;
  y: number;
  vx: number; // px per tick
  vy: number;
  left: number; // px of range left
  hit: Hit;
  age: number;
};

export type GroundEffect = {
  id: number;
  spell: string;
  from: number;
  faction: Faction;
  x: number;
  y: number;
  radius: number; // px
  left: number; // ticks
  nextTick: number;
};

export type TriggerState = { id: string; fired: boolean; inside: boolean };

export type ZoneState = {
  id: ZoneId;
  units: Unit[];
  props: Prop[];
  drops: Drop[];
  projectiles: Projectile[];
  grounds: GroundEffect[];
  triggers: TriggerState[];
  /** Tile changes since generation: [cellIndex, tile, ...]. Terrain itself is seed-derived. */
  tileDeltas: number[];
  /** Seen-bits, one per 16 px block, packed 32 to a number. Interiors black out the unseen; outdoors it is the map's record of where she has been. */
  fog: number[];
  /**
   * Solid `fill`s that could not land because somebody was standing in the rect:
   * [cx, cy, w, h, tile, ...]. Tried again every tick until the rect is clear, so a hedge never
   * grows through a person, or round one.
   */
  pendingFill: number[];
};

export type QuestProgress = { quest: string; counts: number[] };

export type BarSlot = { source: "item" | "spell"; id: string } | null;

export type DialogueState = {
  tree: string;
  node: string;
  line: number;
  speaker: number; // unit id or -prop id
  /** A thing read once, whose words are its own (a milestone, a fingerpost): shown as one line, no tree. */
  text?: string;
} | null;

/** Up to four people play one world (PLATFORM.md). */
export const MAX_PLAYERS = 4;

export type TravelRequest = { zone: ZoneId; mark: string; at?: { x: number; y: number } };

/**
 * One person at the table. What is *hers* lives here: her body, her bags (on the
 * unit), her bar, her conversation. What belongs to the *world* stays on GameState:
 * the heroine's name, quests, flags, what has been learned, the last place anyone rested.
 */
export type PlayerState = {
  /** Seat, 0..3. Stable for the life of the world; inputs and commands are addressed to it. It is also her coat colour. */
  index: number;
  /** Who sits here: a token the client keeps, never shown. It is how a returning guest gets her own bags back. */
  who: string;
  unitId: number;
  /** The zone her unit is in. Every zone with a connected player in it ticks. */
  zone: ZoneId;
  /** Arrival mark last used; she wakes here if she has never rested. */
  lastMark: string;
  /** Ticks until she stands back up. 0 = alive. */
  respawnIn: number;
  bar: BarSlot[];
  craft: (Stack | null)[];
  dialogue: DialogueState;
  god: boolean;
  stats: { kills: number; deaths: number; casts: number };
  /** Set by requestTravel, performed by the scheduler at the end of the tick. Null between ticks. */
  travel: TravelRequest | null;
  /** False after she leaves. Her unit waits in `parked` with everything she owned. */
  connected: boolean;
  parked: Unit | null;
};

export type GameState = {
  version: number;
  seed: number;
  tick: number;
  /** Ticks since midnight. */
  clock: number;
  day: number;
  rng: RngState;
  nextId: number;
  /**
   * The heroine. The host chooses it at New Game and everyone who joins plays her:
   * there is one name in this world, and text rows say {name}.
   */
  name: string;
  /** Whether anyone else may sit down. The host opens and closes the world; a save always loads closed. */
  open: boolean;
  players: PlayerState[];
  zones: Partial<Record<ZoneId, ZoneState>>;
  /** World-wide: one story, one log. */
  flags: Record<string, number>;
  quests: { active: QuestProgress[]; done: string[] };
  /**
   * The last bed or fire *anyone* rested at. Dying wakes you here, and so does sitting
   * down: a friend who joins arrives at the party's fire, not at the far end of the county.
   */
  rest: RestPoint | null;
  /**
   * Growth belongs to the world, so everyone at the table is as far along as everyone
   * else. Found upgrades go here as they are built. Items stay personal.
   */
  growth: {
    spells: string[];
    /** Strength and spirit found so far, on top of the heroine's row. Julie's jars; gold-leaf pages. */
    strength: number;
    spirit: number;
    /** Ids of the upgrades already taken. An upgrade is a thing in the world, found once, by whoever gets there. */
    found: string[];
  };
};

export type RestPoint = { zone: ZoneId; x: number; y: number };

// ---------------------------------------------------------------------------
// Actions and conditions: the one verb list shared by dialogue options, prop
// use, triggers, quest rewards, unit death and the terminal. Adding a verb is
// one case in actions.ts; adding content is a row.

export type Action =
  | { do: "quest"; quest: string }
  | { do: "handin"; quest: string }
  | { do: "flag"; flag: string; value?: number; add?: number }
  | { do: "rest"; until?: number }
  /** Growth by finding (PLAN.md 2.6). `id` names the jar or the page: taking it twice does nothing, whoever asks. */
  | { do: "grow"; stat: "strength" | "spirit"; amount: number; id: string }
  | { do: "give"; item: string; qty?: number }
  | { do: "take"; item: string; qty?: number }
  | { do: "learn"; spell: string }
  | { do: "toast"; text: string }
  /** Words on a thing, shown in the reading box rather than a toast, so they can be read and not missed. */
  | { do: "read"; text: string }
  | { do: "lock"; prop: string }
  | { do: "unlock"; prop: string }
  | { do: "show"; prop: string }
  | { do: "hide"; prop: string }
  | { do: "switch"; prop: string; on?: boolean }
  | { do: "spawn"; unit: string; def: string; at: string }
  | { do: "despawn"; unit: string }
  | { do: "aggro"; unit: string }
  | { do: "location"; name: string }
  | { do: "fill"; rect: string; tile: number }
  /**
   * The first hazard verb: everything hostile standing in a named rect is hit, once. A hoist
   * dropped on a boss, a floor grid, any trap. Her friends are spared unless the row says otherwise.
   */
  | { do: "strike"; rect: string; amount: number; school: School; effect?: string; hitsFriends?: boolean }
  | { do: "status"; effect: string }
  | { do: "heal"; amount: number }
  | { do: "travel"; zone: ZoneId; mark: string }
  | { do: "talk"; tree: string }
  | { do: "throw"; item: string }
  | { do: "shake"; amount: number }
  | { do: "camera"; mode: "follow" | "lock"; rect?: string }
  /** A list that depends on the world: the breaker that goes dark if it is lit and lit if it is dark. */
  | { do: "if"; when: Condition[]; then: ActionList; else?: ActionList }
  /**
   * Walk a unit (by key) to a mark, minding nothing on the way, then run `then` with that unit as
   * the subject. It gives up after a tick budget and then does nothing.
   */
  | { do: "send"; unit: string; to: string; then?: ActionList }
  /** The wall notice as the map: set the fog's seen-bits over these named rects of the actor's zone. */
  | { do: "reveal"; rects: string[] };

export type ActionList = Action[];

export type Condition = (
  | { if: "flag"; flag: string; eq?: number; min?: number }
  | { if: "night" }
  | { if: "questActive"; quest: string }
  | { if: "questReady"; quest: string }
  | { if: "questDone"; quest: string }
  | { if: "hasItem"; item: string; qty?: number }
  | { if: "knows"; spell: string }
  | { if: "dead"; unit: string }
) & { not?: boolean };
