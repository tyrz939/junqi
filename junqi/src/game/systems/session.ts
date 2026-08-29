import {
  ACTION_BAR_SIZE,
  PLAYER_SPIRIT,
  PLAYER_STRENGTH,
  RUN_SPEED,
  SAVE_KEY,
  WALK_SPEED,
} from "@/game/constants";
import { Unit } from "@/game/entities/Unit";
import { BeatTracker } from "@/game/systems/beats";
import { inventoryAdd } from "@/game/systems/inventory";
import { QuestLog } from "@/game/systems/quests";
import { randomSeed } from "@/game/systems/rng";
import { resetSurvive } from "@/game/systems/survive";
import type { ActionBarSlot, GuiWindow, ZoneId } from "@/game/types";

export type ZoneActorSnap = {
  id: string;
  hp: number;
  mp?: number;
  x: number;
  y: number;
  alive: boolean;
  facing?: number;
  deadFor?: number;
};

export type ZonePropSnap = {
  id: string;
  used?: boolean;
  locked?: boolean;
  x: number;
  y: number;
};

export type ZoneSnap = {
  actors: ZoneActorSnap[];
  props: ZonePropSnap[];
  fog?: string;
};

export type WorldInspect = {
  zone: ZoneId;
  player: { x: number; y: number };
  actors: {
    id: string;
    kind: string;
    x: number;
    y: number;
    hp: number;
    maxhp: number;
    asleep: boolean;
    alive: boolean;
  }[];
  props: { id: string; kind: string; x: number; y: number }[];
  bolts: number;
  chunks: number;
  asleep: number;
};

export class GameSession {
  seed = 0;
  time = 19;
  zone: ZoneId = "county";
  entry = { x: 0, y: 0 };
  marks: Partial<Record<ZoneId, { x: number; y: number }>> = {};
  player = makePlayer();
  beats = new BeatTracker();
  quests = new QuestLog();
  actionBar: ActionBarSlot[] = Array.from({ length: ACTION_BAR_SIZE }, () => null);
  flags: Record<string, boolean | number | string> = {};
  dead = new Set<string>();
  zones: Partial<Record<ZoneId, ZoneSnap>> = {};
  god = false;
  paused = false;
  guiOpen = false;
  window: GuiWindow = "inventory";
  craft: (string | null)[] = [null, null, null];
  dogStep = 0;
  terminalOpen = false;
  started = false;
  toast = "";
  held: ActionBarSlot | null = null;
  carrying: string | null = null;
  debug = false;
  visited: ZoneId[] = [];
  talking = false;
  spellbookPage = 0;
  wantSave = false;
  wantRestart = false;
  wantQuit = false;
  pendingZone: ZoneId | null = null;
  inspectWorld: (() => WorldInspect) | null = null;

  newRun(seed?: number): void {
    this.seed = seed ?? randomSeed();
    this.time = 19;
    this.zone = "county";
    this.entry = { x: 0, y: 0 };
    this.marks = {};
    this.player = makePlayer();
    this.beats = new BeatTracker();
    this.quests = new QuestLog();
    this.actionBar = [
      { source: "spell", id: "melee_player" },
      { source: "spell", id: "repair" },
      { source: "item", id: "apple" },
      { source: "item", id: "julies_letter" },
      null,
      null,
      null,
      null,
    ];
    this.flags = {};
    this.dead = new Set();
    this.zones = {};
    this.god = false;
    this.paused = false;
    this.guiOpen = false;
    this.window = "inventory";
    this.craft = [null, null, null];
    this.dogStep = 0;
    this.held = null;
    this.carrying = null;
    this.debug = false;
    this.visited = [];
    this.talking = false;
    this.spellbookPage = 0;
    this.wantSave = false;
    this.wantRestart = false;
    this.wantQuit = false;
    this.pendingZone = null;
    this.started = true;
    inventoryAdd(this.player, "apple", 3);
    inventoryAdd(this.player, "julies_letter", 1);
    inventoryAdd(this.player, "birthday_present", 1);
    this.quests.acquire("the_letter");
    resetSurvive();
  }

  toTitle(wipe: boolean): void {
    this.wantRestart = false;
    this.wantQuit = false;
    this.wantSave = false;
    this.pendingZone = null;
    this.inspectWorld = null;
    this.terminalOpen = false;
    this.started = false;
    this.guiOpen = false;
    this.talking = false;
    this.paused = false;
    if (!wipe) return;
    this.seed = 0;
    this.time = 19;
    this.zone = "county";
    this.entry = { x: 0, y: 0 };
    this.marks = {};
    this.player = makePlayer();
    this.beats = new BeatTracker();
    this.quests = new QuestLog();
    this.actionBar = Array.from({ length: ACTION_BAR_SIZE }, () => null);
    this.flags = {};
    this.dead = new Set();
    this.zones = {};
    this.god = false;
    this.visited = [];
    this.carrying = null;
    this.craft = [null, null, null];
    this.dogStep = 0;
    this.held = null;
    this.debug = false;
    this.spellbookPage = 0;
  }

  worldFrozen(): boolean {
    return this.paused || this.guiOpen || this.terminalOpen || this.talking;
  }

  markHere(x: number, y: number): void {
    this.marks[this.zone] = { x, y };
  }

  flag(id: string): boolean {
    return Boolean(this.flags[id]);
  }

  setFlag(id: string, value: boolean | number | string = true): void {
    this.flags[id] = value;
  }

  writeZone(id: ZoneId, snap: ZoneSnap): void {
    this.zones[id] = snap;
  }

  readZone(id: ZoneId): ZoneSnap | undefined {
    return this.zones[id];
  }

  save(): void {
    const payload = {
      seed: this.seed,
      time: this.time,
      zone: this.zone,
      entry: this.entry,
      marks: this.marks,
      player: this.player.snapshot(),
      beats: this.beats.list(),
      quests: this.quests.snapshot(),
      actionBar: this.actionBar,
      flags: this.flags,
      dead: [...this.dead],
      zones: this.zones,
      dogStep: this.dogStep,
      visited: this.visited,
      carrying: this.carrying,
      craft: this.craft,
      spellbookPage: this.spellbookPage,
    };
    localStorage.setItem(SAVE_KEY, JSON.stringify(payload));
  }

  hasSave(): boolean {
    return Boolean(localStorage.getItem(SAVE_KEY));
  }

  load(): boolean {
    const raw = localStorage.getItem(SAVE_KEY);
    if (!raw) return false;
    const data = JSON.parse(raw) as Record<string, unknown>;
    this.newRun(Number(data.seed));
    this.time = Number(data.time ?? 19);
    this.zone = (data.zone as ZoneId) ?? "county";
    this.entry = (data.entry as { x: number; y: number }) ?? this.entry;
    this.marks = (data.marks as GameSession["marks"]) ?? {};
    this.player.restore((data.player as Record<string, unknown>) ?? {});
    this.beats.restore((data.beats as string[]) ?? []);
    this.quests.restore(
      (data.quests as { active: never[]; completed: string[] }) ?? { active: [], completed: [] },
    );
    if (Array.isArray(data.actionBar)) this.actionBar = data.actionBar as ActionBarSlot[];
    this.flags = (data.flags as GameSession["flags"]) ?? {};
    this.dead = new Set((data.dead as string[]) ?? []);
    this.zones = (data.zones as GameSession["zones"]) ?? {};
    this.dogStep = Number(data.dogStep ?? 0);
    this.visited = (data.visited as ZoneId[]) ?? [];
    this.carrying = (data.carrying as string | null) ?? null;
    if (Array.isArray(data.craft)) this.craft = data.craft as (string | null)[];
    this.spellbookPage = Number(data.spellbookPage ?? 0);
    this.started = true;
    return true;
  }
}

function makePlayer(): Unit {
  return new Unit({
    id: "player",
    name: "Jane",
    faction: "friendly",
    class: "mage",
    controller: "player",
    strength: PLAYER_STRENGTH,
    spirit: PLAYER_SPIRIT,
    walkSpd: WALK_SPEED,
    runSpd: RUN_SPEED,
    spells: ["melee_player", "repair"],
  });
}

export const session = new GameSession();
