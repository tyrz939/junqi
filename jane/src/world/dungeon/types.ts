// The shapes a generated dungeon is written in (DUNGEONS.md 2.2). Three kinds of data:
//
//   DungeonDef     data/dungeons/<id>.json. The mission: the fixed order of challenges, their
//                  locks and keys, and the contract names the story leans on. Authored whole.
//   RoomTemplate   world/dungeon/rooms/<dungeon>/*.room. A text grid with named sockets, in a
//                  pool. The mission only ever talks to the pool.
//   Layout         which template fills which node, where, and how the corridors run. The
//                  only part that is generated, and it is thrown away if it cannot be proven.

import type { ActionList, Condition, Stack, ZoneId } from "@/sim/state";

/** A spell id. A verb is what a dungeon teaches: the thing that re-opens rooms already walked through. */
export type Verb = string;

export type NodeKind =
  | "entrance" | "teach" | "fight" | "puzzle" | "key" | "hub" | "rest"
  | "miniboss" | "verb" | "bosskey" | "boss" | "reward" | "exit" | "side";

/**
 * What goes into one of a template's sockets. Names inside action lists may be written
 * `@<socket id>` (a prop, unit, mark or rect of the same node) and `@self`; the generator
 * resolves them to the bound or generated name.
 */
export type Holding =
  | { socket: string; loot: Stack[]; prop?: string; locked?: boolean; label?: string; guardedBy?: string[] }
  | { socket: string; unit: string }
  | {
      socket: string;
      prop: string;
      talk?: string;
      needs?: Stack[];
      use?: ActionList;
      release?: ActionList;
      locked?: boolean;
      hidden?: boolean;
      label?: string;
      to?: { zone: ZoneId; mark: string };
      /** Locked until these units of the same node are dead (`@socket` or a contract name). The generator writes the trigger. */
      guardedBy?: string[];
    };

export type Bind = { from: string; as: string; what: "prop" | "unit" | "mark" | "rect" };

export type Grant =
  | { key: string } // an `opens` tag
  | { verb: Verb }
  | { item: string; qty: number }
  | { flag: string };

/** A trigger a node carries with it. `rect` is a template rect id ("room" by default); names resolve as in holdings. */
export type NodeTrigger = {
  id: string;
  rect?: string;
  mode?: "enter" | "while";
  once?: boolean;
  when?: Condition[];
  actions: ActionList;
  reset?: ActionList;
};

export type MissionNode = {
  /** Also the key prefix for everything anonymous in it: `mine_office_jar_main`. */
  id: string;
  kind: NodeKind;
  /** Place in the fixed challenge order. Side nodes take their parent's. */
  order: number;
  critical: boolean;
  pool: string;
  holds: Holding[];
  binds: Bind[];
  /** Verbs this node cannot be finished without. */
  demands: Verb[];
  /** What finishing it gives the mission. Documentation that is checked: each must follow from `holds`. */
  grants: Grant[];
  /** 0 .. 1.3, share of budget.baseHeat. 0 = no enemies ever. */
  heat: number;
  triggers?: NodeTrigger[];
};

export type LockinSpec = {
  t: "lockin";
  gateAs?: string;
  /** Units stood up when the room seals. Without it the room's bound boss is the fight. */
  spawn?: { def: string; at: string[]; as?: string[] };
  toast?: string;
  /** Socket of a locked chest the clear opens. */
  reward?: string;
};

export type EdgeKind =
  | { t: "open" }
  | { t: "key"; tag: string; gateAs?: string; label?: string }
  | { t: "verb"; verb: Verb; prop: string; needs?: Stack[]; propAs?: string; label?: string; toast?: string }
  | LockinSpec
  | { t: "oneway"; how: "opens_on"; flag: string; gateAs?: string }
  | { t: "sight" };

export type MissionEdge = { from: string; to: string; kind: EdgeKind; shortcut?: boolean; also?: EdgeKind[] };

export type Turn = 0 | 90 | 180 | 270;
export type Placement = { node: string; template: string; bay: [number, number]; turn: Turn; mirror: boolean };

export type DungeonDef = {
  id: ZoneId;
  name: string;
  /** DESIGN-2020 3.1. Becomes UnitSpawn.phase on every unit. */
  phase: number;
  /** Tile NAMES (`Tile` enum keys), so the JSON reads. */
  tiles: { floor: string; wall: string; alt: string[] };
  indoor: boolean;
  ambient: number;
  lattice: { cols: number; rows: number };
  /** Spells known on arrival: proven by earlier dungeons. */
  givenVerbs: Verb[];
  /** `opens` tags handed over outside. */
  givenKeys: string[];
  nodes: MissionNode[];
  edges: MissionEdge[];
  budget: {
    sideRooms: [number, number];
    /** Walking length of the first completion, in cells. */
    critPathCells: [number, number];
    /** Where along that walk the rest room is first reached, as fractions. DUNGEONS.md asks for 0.4 to 0.6. */
    restAt?: [number, number];
    /** Most cells from the rest room to the boss, and to where a shortcut comes out, with everything open. */
    restToBossCells: number;
    /** Enemy cost points for a node with heat 1. */
    baseHeat: number;
    /** units.json row -> cost. The dungeon's whole bestiary. */
    enemies: Record<string, number>;
  };
  /** Dressing a `dress:<name>` socket may become, by name: prop defs to pick from, and how often it is there at all. */
  dress: Record<string, { props: string[]; chance: number }>;
  /** One hand-placed embedding, used if every attempt fails. */
  fallback: Placement[];
};

// --- templates ---------------------------------------------------------------------------

export type Side = "n" | "e" | "s" | "w";

export type SocketKind =
  | "chest" | "plate" | "push" | "carry" | "lever" | "verbprop" | "rest"
  | "jar" | "page" | "notice" | "spawn" | "boss" | "unit" | "dress" | "prop" | "exit";

export type CoopTag = "plate_or_friend" | "twin_hold" | "lure_and_lever" | "carry_relay" | "guard_the_pusher";

export type Door = { id: string; side: Side; /** 0-based bay along that side. */ bay: number; required: boolean; /** Centre cell of the three, in grid coordinates. */ cx: number; cy: number };
export type Socket = { id: string; kind: SocketKind; cx: number; cy: number; w: number; h: number; options: string[] };
export type TemplateMark = { id: string; cx: number; cy: number };
export type TemplateRect = { id: string; cx: number; cy: number; w: number; h: number };

export type RoomTemplate = {
  id: string;
  pool: string;
  bays: [number, number];
  turns: Turn[];
  mirror: boolean;
  doors: Door[];
  sockets: Socket[];
  marks: TemplateMark[];
  rects: TemplateRect[];
  needs: { verbs: Verb[]; items: Stack[] };
  /** Socket ids she can then reach. */
  grants: string[];
  /** Negative guarantees, proven by ablation: `what` stays shut with `until`'s list suppressed. */
  blocks: { what: string; until: string }[];
  coop: CoopTag[];
  heatMax: number;
  /** One string per row, the wall rim included. Characters are the closed set in room.ts. */
  grid: string[];
  /** Extra tiles by grid character, from the legend (`:` Track). */
  tiles: Record<string, string>;
};

// --- layout ------------------------------------------------------------------------------

/** One end of a corridor: a door of a placed room. */
export type DoorUse = { node: string; door: string };

export type Corridor = {
  edge: number; // index into def.edges
  a: DoorUse;
  b: DoorUse;
  /** Lane graph node ids from a's port to b's port. One entry when the two doors face each other. */
  lane: number[];
};

export type Layout = {
  placements: Placement[];
  corridors: Corridor[];
  /** Side nodes that did not fit this seed. */
  dropped: string[];
  fallback: boolean;
};
