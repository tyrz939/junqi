// The scheduler. 2020's `game` object already treated the game as a scheduler
// firing user events in a fixed order rather than a pile of independent Steps.
// This is that idea with the order written down and nothing implicit:
//
//   tick(inputs)                    one InputFrame per seat
//     1  clock                      day time advances; whatever the county does on the hour
//     for every zone with a player in it, in ZONE_IDS order:
//     2  ring                       sleep / wake on 16 px block change (union of every player's ring)
//     3  timers                     GCD, cooldowns, stop, anim, MP regen, respawn clocks (sleepers too)
//     4  players                    each one's movement, sprint/carry energy, hold-to-push, in seat order
//     5  controllers                AI think + move, snake mover
//     6  projectiles, grounds       bolts fly, webs pulse
//     7  statuses                   DoT / HoT pulses queue hits
//     8  flush                      the ONLY place hp changes; deaths, loot, quest hooks
//     9  triggers, plates           enter / while rects, pressure plates
//    10  drops, fog, prop flags     housekeeping
//    11  travel                     pending zone changes, in seat order, performed last
//
// CO-OP READY (PLATFORM.md). There is no "the player" and no "the zone": up to four
// people share a world and may stand in four different zones, each of which ticks.
// Discrete actions arrive as commands addressed to a seat. tick(inputs) + command(seat, c)
// is the entire surface a network layer needs, and it is exactly what the replay
// recorder already writes: lockstep co-op is a replay being written by several hands.
//
// Alone, the world holds still while she talks, as in 2020. With company nothing pauses.

import type { Catalog } from "@/sim/catalog";
import {
  BAR_SLOTS,
  CRAFT_INPUTS,
  ENERGY_CARRY,
  ENERGY_REGEN,
  ENERGY_SPRINT,
  SAVE_VERSION,
  TICKS_PER_DAY,
  TICKS_PER_HOUR,
} from "@/sim/constants";
import type { SimEvent } from "@/sim/events";
import { cellOf, centre } from "@/sim/grid";
import { lineOfSight } from "@/sim/los";
import { rngSeed } from "@/sim/rng";
import {
  addUnit,
  asPlayer,
  buildRuntime,
  flushPropFlags,
  moveProp,
  occupy,
  removeUnit,
  vacate,
  type Party,
  type World,
  type ZoneRuntime,
} from "@/sim/runtime";
import { cleanName, expandText, isNight } from "@/sim/text";
import { MAX_PLAYERS, type Facing, type GameState, type PlayerState, type RestPoint, type Unit, type ZoneId, type ZoneState } from "@/sim/state";
import { bindLearned, runActions, teach } from "@/sim/actions";
import { tickAi, tickNpc } from "@/sim/ai";
import { nudgeOut, stepPendingFill } from "@/sim/clear";
import { normalize } from "@/sim/angles";
import { flushIncoming, resetPhases, SPELL_ERROR_TEXT, stepGrounds, stepProjectiles, tryCast, type Aim } from "@/sim/combat";
import { advanceDialogue, chooseOption, closeDialogue } from "@/sim/dialogue";
import { holdUse, nearRest, use } from "@/sim/interact";
import { bagAdd, bagDestroy, bagMove, craftClear, craftClearAll, craftPut, craftTake, useItem } from "@/sim/inventory";
import { spawnDrop, stepDrops } from "@/sim/loot";
import { giveQuest } from "@/sim/quests";
import { stepRing } from "@/sim/ring";
import { tickSnake } from "@/sim/snake";
import { isStunned, speedFactor, tickStatuses } from "@/sim/status";
import { playerInRect, stepTriggers } from "@/sim/triggers";
import { createUnit, faceVector, maxHp, maxMp, moveUnit, placeUnit, restoreEnergy, setAnim, spendEnergy } from "@/sim/units";
import { ZONE_IDS } from "@/world";
import { ensureZoneState, placeArrival, requestTravel, stampFog } from "@/sim/zones";

/** Held inputs, sampled once per tick. Vectors are quantised to 1/127 so a recording is exact. */
export type InputFrame = {
  mx: number;
  my: number;
  sprint: boolean;
  useHeld: boolean;
  /** Aim direction from the cursor or right stick; (0,0) means "use facing". */
  ax: number;
  ay: number;
};

export const NO_INPUT: InputFrame = { mx: 0, my: 0, sprint: false, useHeld: false, ax: 0, ay: 0 };

export type DevOp =
  | { op: "give"; item: string; qty: number }
  | { op: "god"; on: boolean }
  | { op: "tp"; zone: ZoneId; mark: string }
  | { op: "time"; hour: number }
  | { op: "hp"; value: number }
  | { op: "mp"; value: number }
  | { op: "learn"; spell: string }
  | { op: "quest"; quest: string }
  | { op: "flag"; flag: string; value: number }
  | { op: "kill" }
  | { op: "spawn"; def: string };

export type Command =
  | { t: "use" }
  /** `on`: the unit under the cursor when it was pressed, 0 for nobody, absent without a cursor. Friendly spells read it. */
  | { t: "bar"; slot: number; on?: number }
  | { t: "cast"; spell: string; on?: number }
  | { t: "item"; item: string }
  | { t: "bagMove"; from: number; to: number }
  | { t: "bagDestroy"; slot: number }
  | { t: "craftPut"; bag: number; slot: number }
  | { t: "craftClear"; slot: number }
  | { t: "craftClearAll" }
  | { t: "craftTake" }
  | { t: "bind"; slot: number; source: "item" | "spell"; id: string }
  | { t: "unbind"; slot: number }
  | { t: "barSwap"; a: number; b: number }
  | { t: "advance" }
  | { t: "choose"; option: number }
  | { t: "closeDialogue" }
  /**
   * Someone sits down. Addressed to seat -1; the seat is assigned in order. `who` is a
   * token her client keeps (never shown, never a name: everyone plays the one heroine).
   * Refused unless the host has opened the world.
   */
  | { t: "join"; who: string }
  /** Someone gets up. Her body and what she owned wait for her; what the story needs is handed on. */
  | { t: "leave" }
  /** Seat 0 only: let others sit down, or stop letting them. */
  | { t: "open"; on: boolean }
  | { t: "dev"; dev: DevOp };

/** The `who` of the person whose machine holds the world. */
export const HOST = "host";

/** New games begin at 17:00. Story.docx: "When she arrived the town it was already 5pm". */
export const START_HOUR = 17;

/** Presentation events that belong to one person, not to everyone who can see the spot. */
const PERSONAL = new Set<SimEvent["e"]>(["toast", "castFailed", "loot", "learn", "bag", "dialogue", "playerDied", "shake", "camera"]);

export function newGameState(seed: number, name?: string): GameState {
  return {
    version: SAVE_VERSION,
    seed: seed >>> 0,
    tick: 0,
    clock: START_HOUR * TICKS_PER_HOUR,
    day: 0,
    rng: rngSeed(seed >>> 0, 1),
    nextId: 1,
    name: cleanName(name),
    open: false,
    players: [],
    zones: {},
    flags: {},
    quests: { active: [], done: [] },
    rest: null,
    growth: { spells: [], strength: 0, spirit: 0, found: [] },
  };
}

/** One live zone: the World every system function sees. `actor` is set around player-scoped calls. */
class ZoneCtx implements World {
  actor: PlayerState | null = null;
  constructor(
    private readonly sim: Sim,
    public zone: ZoneState,
    public rt: ZoneRuntime,
  ) {}
  get catalog(): Catalog {
    return this.sim.catalog;
  }
  get state(): GameState {
    return this.sim.state;
  }
  get party(): Party {
    return this.sim.party;
  }
  emit(ev: SimEvent, all = false): void {
    this.sim.emitFrom(this, ev, all);
  }
}

/**
 * What one seat sees: her zone, acting as her. The UI and the renderer are built
 * against this, so they work the same for seat 0 alone and seat 3 of four. It is a
 * World, so the read-only helpers (focusOf, nearBench, craftOutput, currentNode,
 * requirementCount) take it directly.
 */
export class PlayerView implements World {
  constructor(
    private readonly sim: Sim,
    readonly index: number,
  ) {}
  get catalog(): Catalog {
    return this.sim.catalog;
  }
  get state(): GameState {
    return this.sim.state;
  }
  get party(): Party {
    return this.sim.party;
  }
  get me(): PlayerState {
    return this.sim.state.players[this.index];
  }
  get actor(): PlayerState {
    return this.me;
  }
  set actor(_p: PlayerState | null) {
    /* a view always acts as its own seat */
  }
  get zone(): ZoneState {
    return this.sim.ctxOf(this.me.zone).zone;
  }
  get rt(): ZoneRuntime {
    return this.sim.ctxOf(this.me.zone).rt;
  }
  get player(): Unit {
    const u = this.rt.units.get(this.me.unitId);
    if (!u) throw new Error(`Seat ${this.index} has no body in ${this.me.zone}`);
    return u;
  }
  get hour(): number {
    return this.sim.hour;
  }
  get frozen(): boolean {
    return this.sim.frozen;
  }
  get lastTickMs(): number {
    return this.sim.lastTickMs;
  }
  emit(): void {
    /* views are for reading */
  }
}

export class Sim implements World {
  readonly catalog: Catalog;
  state: GameState;
  private readonly live = new Map<ZoneId, ZoneCtx>();
  private readonly views: PlayerView[] = [];
  private events: SimEvent[] = [];
  private readonly aims: (Aim | null)[] = [];
  /** Wall-clock cost of the last tick in ms, for the debug overlay. Not state. */
  lastTickMs = 0;

  readonly party: Party = {
    size: () => this.state.players.reduce((n, p) => n + (p.connected ? 1 : 0), 0),
    ofUnit: (unitId) => this.state.players.find((p) => p.connected && p.unitId === unitId),
    units: () => {
      const out: Unit[] = [];
      for (const p of this.state.players) {
        const u = p.connected ? this.live.get(p.zone)?.rt.units.get(p.unitId) : undefined;
        if (u) out.push(u);
      }
      return out;
    },
    bodies: () => {
      const out: Unit[] = [];
      for (const p of this.state.players) {
        const u = p.parked ?? this.live.get(p.zone)?.rt.units.get(p.unitId);
        if (u) out.push(u);
      }
      return out;
    },
    everyoneResting: () => {
      for (const p of this.state.players) {
        if (!p.connected) continue;
        const ctx = this.ctxOf(p.zone);
        if (!asPlayer(ctx, p, () => nearRest(ctx))) return false;
      }
      return true;
    },
    each: (fn) => {
      for (const p of this.state.players) {
        if (!p.connected) continue;
        const ctx = this.ctxOf(p.zone);
        asPlayer(ctx, p, () => fn(ctx, p));
      }
    },
  };

  private constructor(catalog: Catalog, state: GameState) {
    this.catalog = catalog;
    this.state = state;
  }

  /** `name` is the heroine's, chosen by the host. Everyone who joins later plays her under it. */
  static newGame(catalog: Catalog, seed: number, name?: string): Sim {
    const sim = new Sim(catalog, newGameState(seed, name));
    sim.command(-1, { t: "join", who: HOST });
    return sim;
  }

  /**
   * `state` is adopted, not copied. Pass a clone if the caller keeps using it.
   * A save is the host's world: seat 0 sits back down, everyone else's body waits for
   * her to come back, and the world loads closed (and so without the party penalty)
   * until the host opens it again.
   */
  static fromState(catalog: Catalog, state: GameState): Sim {
    const sim = new Sim(catalog, state);
    state.open = false;
    for (const p of state.players) {
      if (p.index === 0 || !p.connected) continue;
      const zs = state.zones[p.zone];
      const at = zs ? zs.units.findIndex((u) => u.id === p.unitId) : -1;
      if (zs && at >= 0) p.parked = zs.units.splice(at, 1)[0];
      p.connected = false;
      p.dialogue = null;
      p.travel = null;
    }
    for (const p of state.players) if (p.connected) sim.ctxOf(p.zone);
    for (const ctx of sim.live.values()) stepRing(ctx, true);
    return sim;
  }

  // --- zones ---------------------------------------------------------------------

  /** The live context of a zone, built on demand. Zones with nobody in them are dropped after each tick. */
  ctxOf(zone: ZoneId): ZoneCtx {
    let ctx = this.live.get(zone);
    if (!ctx) {
      const { zs, bp } = ensureZoneState(this.state, this.catalog, zone);
      ctx = new ZoneCtx(this, zs, buildRuntime(this.catalog, bp, zs));
      this.live.set(zone, ctx);
    }
    return ctx;
  }

  private dropEmptyZones(): void {
    for (const zone of [...this.live.keys()]) {
      if (!this.state.players.some((p) => p.connected && p.zone === zone)) this.live.delete(zone);
    }
  }

  // --- events --------------------------------------------------------------------

  emitFrom(ctx: World, ev: SimEvent, all: boolean): void {
    if (ev.e === "toast") ev.text = expandText(this.state, ev.text);
    if (!all) {
      ev.inZone = ctx.zone.id;
      if (ctx.actor && PERSONAL.has(ev.e)) ev.to = ctx.actor.index;
    }
    this.events.push(ev);
  }

  /** Everything since the last drain. Solo callers and tests use this. */
  drainEvents(): SimEvent[] {
    const out = this.events;
    this.events = [];
    return out;
  }

  /** What one seat should see and hear: her own, her zone's, and the party's. Call after drainEvents. */
  static eventsFor(events: readonly SimEvent[], me: PlayerState): SimEvent[] {
    return events.filter((ev) => (ev.to === undefined || ev.to === me.index) && (ev.inZone === undefined || ev.inZone === me.zone));
  }

  // --- solo conveniences: seat 0 -------------------------------------------------

  view(index = 0): PlayerView {
    return (this.views[index] ??= new PlayerView(this, index));
  }
  get me(): PlayerState {
    return this.state.players[0];
  }
  get player(): Unit {
    return this.view(0).player;
  }
  get zone(): ZoneState {
    return this.ctxOf(this.me.zone).zone;
  }
  get rt(): ZoneRuntime {
    return this.ctxOf(this.me.zone).rt;
  }
  /**
   * The Sim is itself a World for seat 0 in her zone, so solo callers and tests can
   * hand it straight to a system function: `tryCast(sim, sim.player, "icebolt")`.
   */
  private actorOverride: PlayerState | null | undefined;
  get actor(): PlayerState | null {
    return this.actorOverride === undefined ? this.me : this.actorOverride;
  }
  set actor(p: PlayerState | null) {
    this.actorOverride = p === this.me ? undefined : p;
  }
  emit(ev: SimEvent, all = false): void {
    this.emitFrom(this, ev, all);
  }

  /** Alone, the world holds still while she talks. With company, nothing pauses. */
  get frozen(): boolean {
    let talking = false;
    let n = 0;
    for (const p of this.state.players) {
      if (!p.connected) continue;
      n++;
      talking = p.dialogue !== null;
    }
    return n === 1 && talking;
  }

  /** Hour of day as a float, 0..24. */
  get hour(): number {
    return this.state.clock / TICKS_PER_HOUR;
  }

  /**
   * The state to write. 2020 had to instance_activate_all before saving because
   * deactivated instances were invisible to `with`; here sleepers are ordinary rows
   * in the same arrays, so saving reads the tree as it stands and changes nothing.
   */
  prepareSave(): GameState {
    return this.state;
  }

  // --- the tick --------------------------------------------------------------------

  tick(input: InputFrame | readonly InputFrame[]): void {
    const t0 = typeof performance !== "undefined" ? performance.now() : 0;
    const s = this.state;
    if (this.frozen) return;
    const inputs: readonly InputFrame[] = Array.isArray(input) ? input : [input as InputFrame];
    s.tick++;

    // 1 clock, and whatever the county does on the hour (the bell at nine): everyone hears it
    if (++s.clock >= TICKS_PER_DAY) {
      s.clock = 0;
      s.day++;
    }
    if (s.clock % TICKS_PER_HOUR === 0) {
      const hour = s.clock / TICKS_PER_HOUR;
      for (const row of this.catalog.clock) {
        if (row.hour === hour) this.party.each((w, p) => runActions(w, row.actions, p.unitId));
      }
    }

    for (const zoneId of ZONE_IDS) {
      const w = this.live.get(zoneId);
      if (!w || !s.players.some((p) => p.connected && p.zone === zoneId)) continue;
      w.rt.pathsThisTick = 0;
      if (s.tick % 30 === 0) this.stepDayOnly(w);
      // 2 ring
      stepRing(w);
      // 3 timers
      for (const u of w.zone.units) this.tickTimers(w, u);
      // 4 players, in seat order
      for (const p of s.players) {
        if (!p.connected || p.zone !== zoneId) continue;
        const frame = inputs[p.index] ?? NO_INPUT;
        this.aims[p.index] = frame.ax !== 0 || frame.ay !== 0 ? normalize(frame.ax, frame.ay) : null;
        asPlayer(w, p, () => this.tickPlayer(w, p, frame));
      }
      // 5 controllers
      for (const u of w.zone.units) {
        if (!u.alive || !u.awake || u.hidden || isStunned(w, u)) continue;
        if (u.controller === "ai") tickAi(w, u);
        else if (u.controller === "snake") tickSnake(w, u);
        // Nobody fights these, but they may be sent somewhere, and a butterfly keeps its round of flowers.
        else if (u.controller === "npc" && (u.order !== null || u.patrol !== null)) tickNpc(w, u);
      }
      // 6 projectiles, grounds
      stepProjectiles(w);
      stepGrounds(w);
      // 7 statuses
      for (const u of w.zone.units) if (u.alive) tickStatuses(w, u);
      // 8 flush. Twice: lifesteal heals queued by the first pass land this tick too.
      for (const u of w.zone.units) flushIncoming(w, u);
      for (const u of w.zone.units) flushIncoming(w, u);
      // 9 triggers
      stepTriggers(w);
      // 10 housekeeping
      stepDrops(w);
      flushPropFlags(this.catalog, w.rt, w.zone);
      stepPendingFill(w);
      if (s.tick % 10 === 0) stampFog(w);
    }

    // 11 travel, in seat order
    for (const p of s.players) if (p.connected && p.travel) this.performTravel(p);
    this.dropEmptyZones();
    if (t0) this.lastTickMs = performance.now() - t0;
  }

  /**
   * `dayOnly` units are simply not there between 21:00 and 06:00; `nightOnly` ones are not
   * there the rest of the time. Nobody sees either go, or come.
   */
  private stepDayOnly(w: ZoneCtx, force = false): void {
    const night = isNight(this.state);
    const watchers = force ? [] : this.party.units();
    const hour = Math.floor(this.state.clock / TICKS_PER_HOUR) % 24;
    const inSpan = (h: number, from: number, to: number): boolean => (from < to ? h >= from && h < to : from > to ? h >= from || h < to : true);
    for (const u of w.zone.units) {
      const def = this.catalog.units[u.def];
      if (!def.dayOnly && !def.nightOnly && !def.schedule) continue;
      const q = this.state.quests;
      const holds = (r: { while?: string; after?: string }): boolean =>
        r.while ? q.active.some((p) => p.quest === r.while) : r.after ? q.done.includes(r.after) : true;
      const rows = def.schedule ? [...def.schedule.filter((r) => r.while || r.after), ...def.schedule.filter((r) => !r.while && !r.after)] : undefined;
      const slot = rows?.find((r) => inSpan(hour, r.from, r.to) && holds(r));
      if (def.schedule && !slot) continue;
      const away = slot
        ? !!(slot.inside || slot.absent)
        : def.dayOnly
          ? night && (!def.dayOnlyAfter || this.state.quests.done.includes(def.dayOnlyAfter))
          : !night;
      if (u.hidden === away) continue;
      // Never vanish or appear while any of them is looking straight at it.
      if (watchers.some((p) => w.rt.units.has(p.id) && Math.abs(u.x - p.x) < 120 && Math.abs(u.y - p.y) < 80)) continue;
      u.hidden = away;
      if (away) vacate(w.rt, u);
      else {
        // The world went on without it: if something solid stands where it stood, it comes back beside it.
        if (u.alive && w.rt.grid.solid(cellOf(u.x), cellOf(u.y))) nudgeOut(w, u, cellOf(u.x), cellOf(u.y), 1, 1);
        if (u.alive && u.awake) occupy(w.rt, u);
      }
    }
  }

  private tickTimers(w: ZoneCtx, u: Unit): void {
    if (u.gcd > 0) u.gcd--;
    if (u.stop > 0) u.stop--;
    for (const k in u.cooldowns) if (u.cooldowns[k] > 0 && --u.cooldowns[k] === 0) delete u.cooldowns[k];
    for (const k in u.itemCooldowns) if (u.itemCooldowns[k] > 0 && --u.itemCooldowns[k] === 0) delete u.itemCooldowns[k];
    u.animTick++;
    if (u.alive) {
      // RestoreMP(spirit / 1000) per step, every living unit.
      if (u.mp < maxMp(u)) u.mp = Math.min(maxMp(u), u.mp + u.spirit / 1000);
      if ((u.anim === "attack" || u.anim === "cast" || u.anim === "hurt") && u.animTick > 24) setAnim(u, "idle");
      return;
    }
    const owner = this.party.ofUnit(u.id);
    if (owner) {
      if (owner.respawnIn > 0 && --owner.respawnIn === 0) asPlayer(w, owner, () => this.revivePlayer(w, owner, u));
      return;
    }
    if (u.respawn > 0 && ++u.deadFor >= u.respawn) this.respawn(w, u);
  }

  private respawn(w: ZoneCtx, u: Unit): void {
    u.alive = true;
    u.hp = maxHp(u);
    u.mp = maxMp(u);
    u.deadFor = 0;
    u.combat = "idle";
    u.target = 0;
    resetPhases(w, u);
    setAnim(u, "idle");
    const free = w.rt.grid.nearestFree(Math.floor(u.homeX / 8), Math.floor(u.homeY / 8), 6, u.id);
    placeUnit(w, u, free ? centre(free.cx) : u.homeX, free ? centre(free.cy) : u.homeY);
    w.emit({ e: "respawn", unit: u.id });
  }

  /**
   * Death is harsh on purpose: you wake at the last bed or fire you rested at, however
   * far that is. Before the first rest there is only the door you came in by.
   */
  private revivePlayer(w: ZoneCtx, player: PlayerState, u: Unit): void {
    const mark = w.rt.bp.marks[player.lastMark] ?? w.rt.bp.marks.start;
    // Whatever she was carrying stays where she fell.
    const carried = u.carrying ? w.rt.props.get(u.carrying) : undefined;
    if (carried) {
      const def = this.catalog.props[carried.def];
      const spot = w.rt.grid.nearestFree(Math.floor(u.x / 8), Math.floor(u.y / 8), 6) ?? { cx: carried.cx, cy: carried.cy };
      moveProp(w, carried, spot.cx, spot.cy);
      carried.solid = def.solid;
    }
    u.carrying = 0;
    u.alive = true;
    u.hp = maxHp(u);
    u.mp = maxMp(u);
    u.energy = 100;
    u.energyLocked = false;
    setAnim(u, "idle");

    // Lock-ins undo themselves and re-arm, so a death never leaves a gate shut in your face.
    // With company they hold for as long as someone is still alive inside.
    for (const t of w.zone.triggers) {
      const def = w.rt.triggers[t.id];
      const rect = def ? w.rt.bp.rects[def.rect] : undefined;
      if (!def || !t.fired || !def.reset || !rect) continue;
      if (rect.w < w.rt.grid.w && playerInRect(w, rect, player)) continue;
      runActions(w, def.reset, u.id);
      t.fired = false;
      t.inside = false;
    }
    for (const other of w.zone.units) {
      if (other.target === u.id) other.target = 0;
      if (other.combat === "combat" && !this.party.ofUnit(other.target)) other.combat = "leash";
    }

    // The party's fire, not hers: whoever rested last moved it for everyone.
    const rest = this.state.rest;
    if (rest && rest.zone !== w.zone.id) {
      // Another zone: travel happens at the end of this tick, like every other zone change.
      player.travel = { zone: rest.zone, mark: "start", at: { x: rest.x, y: rest.y } };
    } else if (rest) {
      placeUnit(w, u, rest.x, rest.y);
    } else {
      const free = w.rt.grid.nearestFree(mark.cx, mark.cy, 8, u.id) ?? mark;
      placeUnit(w, u, centre(free.cx), centre(free.cy));
    }
    w.emit({ e: "respawn", unit: u.id });
    w.emit({ e: "toast", text: rest ? "I woke where I last rested" : "I woke where I came in" });
  }

  private tickPlayer(w: ZoneCtx, player: PlayerState, input: InputFrame): void {
    const u = w.rt.units.get(player.unitId);
    if (!u) return;
    if (!u.alive) {
      if (player.respawnIn === 0) player.respawnIn = 240;
      return;
    }
    // With company the world does not stop for a conversation, but she does.
    const busy = player.dialogue !== null;
    const stunned = isStunned(w, u);
    let mx = busy ? 0 : input.mx;
    let my = busy ? 0 : input.my;
    const len = Math.sqrt(mx * mx + my * my);
    if (len > 1) {
      mx /= len;
      my /= len;
    }
    const wantsMove = len > 0.05 && u.stop === 0 && !stunned;
    const held = input.useHeld && !busy;
    const braced = held && !stunned ? holdUse(w, u, Math.sign(Math.round(mx)), Math.sign(Math.round(my))) : false;
    if (!held) u.hold = 0;

    let sprinting = false;
    if (wantsMove && !braced) {
      const def = this.catalog.units[u.def];
      sprinting = input.sprint && !u.energyLocked && u.energy > 0 && u.carrying === 0;
      const speed = (sprinting ? def.run : def.walk) * speedFactor(w, u) * (player.god ? 2 : 1);
      faceVector(u, mx, my);
      moveUnit(w, u, mx * speed, my * speed);
      if (u.anim === "idle" || u.anim === "walk") setAnim(u, "walk");
    } else if (u.anim === "walk") {
      setAnim(u, "idle");
    }
    // Energy: sprint -0.5, carry -0.25, anything else +0.5 (walking included), per tick.
    if (sprinting) spendEnergy(u, ENERGY_SPRINT);
    else if (u.carrying) {
      spendEnergy(u, ENERGY_CARRY);
      if (u.energy === 0) use(w, u); // arms give out: put it down
    } else restoreEnergy(u, ENERGY_REGEN);
  }

  private performTravel(player: PlayerState): void {
    const req = player.travel;
    player.travel = null;
    if (!req) return;
    const from = this.ctxOf(player.zone);
    const body = from.rt.units.get(player.unitId);
    if (!body) return;
    removeUnit(from, body);
    // What is in flight does not outlive the room, once the last of the party has left it.
    const othersStay = this.state.players.some((p) => p !== player && p.connected && p.zone === player.zone);
    if (!othersStay) {
      from.zone.projectiles.length = 0;
      from.zone.grounds.length = 0;
    }
    for (const u of from.zone.units) {
      if (u.target === body.id) u.target = 0;
      if (u.combat === "combat" && !this.party.ofUnit(u.target)) u.combat = "leash";
    }
    const first = !this.state.zones[req.zone];
    const to = this.ctxOf(req.zone);
    asPlayer(to, player, () => {
      placeArrival(to, player, body, req);
      addUnit(to, body);
      stepRing(to, true);
      stampFog(to);
      this.stepDayOnly(to, true); // arriving somewhere at night: it was already gone
      to.emit({ e: "zone", zone: req.zone, name: to.rt.bp.name, first });
    });
  }

  // --- commands ----------------------------------------------------------------------

  /** Aim for the next cast when it is issued between ticks (the app sets this from the cursor). */
  setAim(ax: number, ay: number, seat = 0): void {
    this.aims[seat] = ax !== 0 || ay !== 0 ? normalize(ax, ay) : null;
  }

  /**
   * Discrete actions, addressed to a seat. Safe while frozen or paused; never advances
   * time. `command(c)` is seat 0, for solo callers.
   */
  command(c: Command): number;
  command(seat: number, c: Command): number;
  command(a: number | Command, b?: Command): number {
    const seat = typeof a === "number" ? a : 0;
    const c = typeof a === "number" ? (b as Command) : a;
    if (c.t === "join") return this.join(c.who);
    const player = this.state.players[seat];
    if (!player || !player.connected) return -1;
    if (c.t === "open") {
      // The host's decision alone. Closing does not send anyone home; it stops anyone new sitting down.
      if (seat !== 0) return -1;
      this.state.open = c.on;
      return seat;
    }
    if (c.t === "leave") {
      this.leave(player);
      return seat;
    }
    const w = this.ctxOf(player.zone);
    asPlayer(w, player, () => this.run(w, player, c));
    return seat;
  }

  private run(w: ZoneCtx, player: PlayerState, c: Exclude<Command, { t: "join" } | { t: "leave" } | { t: "open" }>): void {
    const u = w.rt.units.get(player.unitId);
    if (!u) return;
    switch (c.t) {
      case "use":
        if (player.dialogue) advanceDialogue(w);
        else use(w, u);
        return;
      case "bar": {
        const slot = player.bar[c.slot];
        if (!slot) return;
        if (slot.source === "spell") this.playerCast(w, player, u, slot.id, c.on);
        else useItem(w, u, slot.id);
        return;
      }
      case "cast":
        this.playerCast(w, player, u, c.spell, c.on);
        return;
      case "item":
        useItem(w, u, c.item);
        return;
      case "bagMove":
        bagMove(w, u, c.from, c.to);
        return;
      case "bagDestroy":
        bagDestroy(w, u, c.slot);
        return;
      case "craftPut":
        craftPut(w, u, c.bag, c.slot);
        return;
      case "craftClear":
        craftClear(w, u, c.slot);
        return;
      case "craftClearAll":
        craftClearAll(w, u);
        return;
      case "craftTake":
        craftTake(w, u);
        return;
      case "bind":
        if (c.slot >= 0 && c.slot < BAR_SLOTS) player.bar[c.slot] = { source: c.source, id: c.id };
        return;
      case "unbind":
        if (c.slot >= 0 && c.slot < BAR_SLOTS) player.bar[c.slot] = null;
        return;
      case "barSwap": {
        const bar = player.bar;
        if (c.a < 0 || c.b < 0 || c.a >= BAR_SLOTS || c.b >= BAR_SLOTS) return;
        [bar[c.a], bar[c.b]] = [bar[c.b], bar[c.a]];
        return;
      }
      case "advance":
        advanceDialogue(w);
        return;
      case "choose":
        chooseOption(w, c.option);
        return;
      case "closeDialogue":
        closeDialogue(w);
        return;
      case "dev":
        this.dev(w, player, u, c.dev);
        return;
      default: {
        const never: never = c;
        throw new Error(`Unhandled command ${JSON.stringify(never)}`);
      }
    }
  }

  private playerCast(w: ZoneCtx, player: PlayerState, u: Unit, spell: string, on?: number): void {
    if (player.dialogue) return;
    if (!u.book.includes(spell)) return;
    const result = tryCast(w, u, spell, this.aims[player.index] ?? null, on);
    if (result === "castSuccessful") return;
    w.emit({ e: "castFailed", unit: u.id, spell, error: result });
    const text = SPELL_ERROR_TEXT[result];
    if (text) w.emit({ e: "toast", text });
  }

  // --- seats ---------------------------------------------------------------------------

  /**
   * Sit down. Everyone arrives at the party's last fire (the county's start before
   * anyone has rested): no fast travel, but nobody joins a friend from ten minutes away
   * either. A `who` that has played here before gets her own body and bags back; anyone
   * else gets the starting kit. Either way she knows what the world has learned, because
   * growth is the world's. Returns the seat, or -1: the world is closed, or all four are taken.
   */
  private join(who: string): number {
    const s = this.state;
    if (s.players.length > 0 && !s.open) return -1;
    const back = s.players.find((p) => !p.connected && p.who === who && p.parked);
    if (!back && s.players.length >= MAX_PLAYERS) return -1;

    const at = this.arrival();
    const w = this.ctxOf(at.zone);
    const free = w.rt.grid.nearestFree(Math.floor(at.x / 8), Math.floor(at.y / 8), 8) ?? { cx: Math.floor(at.x / 8), cy: Math.floor(at.y / 8) };
    let player: PlayerState;
    let body: Unit;
    if (back && back.parked) {
      player = back;
      body = back.parked;
      back.parked = null;
      back.connected = true;
      back.respawnIn = 0;
      body.x = centre(free.cx);
      body.y = centre(free.cy);
      if (!body.alive) {
        body.alive = true;
        body.hp = maxHp(body);
        setAnim(body, "idle");
      }
    } else {
      body = createUnit(s, this.catalog, "jane", `player_${s.players.length}`, centre(free.cx), centre(free.cy), at.facing);
      player = {
        index: s.players.length,
        who,
        unitId: body.id,
        zone: at.zone,
        lastMark: "start",
        respawnIn: 0,
        bar: Array.from({ length: BAR_SLOTS }, () => null),
        craft: Array.from({ length: CRAFT_INPUTS }, () => null),
        dialogue: null,
        god: false,
        stats: { kills: 0, deaths: 0, casts: 0 },
        travel: null,
        connected: true,
        parked: null,
      };
      s.players.push(player);
    }
    const fresh = player !== back;
    player.zone = at.zone;
    player.lastMark = "start";
    addUnit(w, body);
    asPlayer(w, player, () => {
      if (fresh) {
        for (const item of this.catalog.start.items) bagAdd(w, body, item.item, item.qty);
        this.catalog.start.bar.forEach((slot, i) => {
          if (slot) player.bar[i] = slot;
        });
        for (const q of this.catalog.start.quests) giveQuest(w, q);
      }
      // Catch up: whatever the table learned while she was away, or before she ever came.
      if (fresh) {
        body.strength += s.growth.strength;
        body.spirit += s.growth.spirit;
        body.hp = maxHp(body);
        body.mp = maxMp(body);
      }
      for (const spell of s.growth.spells) {
        if (!body.book.includes(spell)) body.book.push(spell);
        bindLearned(player, spell);
      }
      stepRing(w, true);
      w.emit({ e: "zone", zone: at.zone, name: w.rt.bp.name, first: player.index === 0 && fresh });
    });
    this.announceParty();
    return player.index;
  }

  /** Where anyone who sits down appears: the party's last bed or fire, else where the story starts. */
  private arrival(): RestPoint & { facing: Facing } {
    if (this.state.rest) return { ...this.state.rest, facing: 1 };
    const marks = this.ctxOf("county").rt.bp.marks;
    const start = marks[this.catalog.start.mark] ?? marks.start;
    return { zone: "county", x: centre(start.cx), y: centre(start.cy), facing: (start.facing ?? 1) as Facing };
  }

  /**
   * Get up. Her body leaves the world and waits with what she owned. What the story
   * cannot go on without (keys, quest items) does not wait: it is handed to someone who
   * is staying, so a needed thing can never walk out of the door with a guest.
   */
  private leave(player: PlayerState): void {
    const w = this.ctxOf(player.zone);
    const body = w.rt.units.get(player.unitId);
    if (body) {
      if (body.carrying) asPlayer(w, player, () => use(w, body));
      for (const u of w.zone.units) if (u.target === body.id) u.target = 0;
      removeUnit(w, body);
      body.incoming.length = 0;
      player.parked = body;
    }
    player.connected = false;
    player.dialogue = null;
    player.travel = null;
    if (body) this.handOn(body);
    this.dropEmptyZones();
    this.announceParty();
  }

  /**
   * Into a staying player's bags, not onto the ground where the leaver stood: that
   * could be behind a lock-in, or ten minutes' walk away with no fast travel. Lowest
   * seat first (the host, whose world it is). What does not fit lands at her feet, and
   * story items on the ground never age out. With nobody left, she keeps them.
   */
  private handOn(leaver: Unit): void {
    const heir = this.state.players.find((p) => p.connected);
    if (!heir || !leaver.bag) return;
    const w = this.ctxOf(heir.zone);
    const to = w.rt.units.get(heir.unitId);
    if (!to) return;
    asPlayer(w, heir, () => {
      let any = false;
      for (let i = 0; i < leaver.bag!.length; i++) {
        const stack = leaver.bag![i];
        if (!stack || !this.catalog.storyItems.has(stack.item)) continue;
        leaver.bag![i] = null;
        const left = bagAdd(w, to, stack.item, stack.qty);
        if (left < stack.qty) w.emit({ e: "loot", item: stack.item, qty: stack.qty - left });
        if (left > 0) spawnDrop(w, stack.item, left, to.x, to.y);
        any = true;
      }
      if (any) w.emit({ e: "toast", text: "She left what mattered with you" });
    });
  }

  /** The penalty follows the head count, so say so whenever it changes. */
  private announceParty(): void {
    const n = this.party.size();
    if (n <= 1 && this.state.players.length <= 1) return;
    const text =
      n <= 1
        ? "You are alone again. The county stops weighing you."
        : `There are ${["", "", "two", "three", "four"][n]} of you now. The county has noticed, and each of you is weaker for it. Stay together.`;
    const any = this.state.players.find((p) => p.connected);
    if (any) this.ctxOf(any.zone).emit({ e: "toast", text }, true);
  }

  private dev(w: ZoneCtx, player: PlayerState, u: Unit, d: DevOp): void {
    switch (d.op) {
      case "give":
        if (this.catalog.items[d.item]) bagAdd(w, u, d.item, d.qty);
        return;
      case "god":
        player.god = d.on;
        return;
      case "tp":
        requestTravel(w, d.zone, d.mark);
        if (player.travel) this.performTravel(player);
        this.dropEmptyZones();
        return;
      case "time":
        this.state.clock = Math.floor((((d.hour % 24) + 24) % 24) * TICKS_PER_HOUR);
        return;
      case "hp":
        u.hp = Math.max(1, Math.min(maxHp(u), d.value));
        return;
      case "mp":
        u.mp = Math.max(0, Math.min(maxMp(u), d.value));
        return;
      case "learn":
        teach(w, d.spell);
        return;
      case "quest":
        if (this.catalog.quests[d.quest]) giveQuest(w, d.quest);
        return;
      case "flag":
        this.state.flags[d.flag] = d.value;
        return;
      case "kill":
        // Everything hostile within 25 m that she can see: roughly the screen, never the next room
        // through a wall, and never something that is not there (the hidden are not in the world).
        for (const other of w.zone.units) {
          const near = Math.abs(other.x - u.x) <= 200 && Math.abs(other.y - u.y) <= 200;
          if (!near || !other.alive || !other.awake || other.hidden || other.faction === u.faction) continue;
          if (!lineOfSight(w.rt.grid, u.x, u.y, other.x, other.y)) continue;
          other.incoming.push({ amount: 1e6, school: "physical", from: u.id, crit: false });
        }
        return;
      case "spawn": {
        if (!this.catalog.units[d.def]) return;
        const free = w.rt.grid.nearestFree(Math.floor(u.x / 8) + 3, Math.floor(u.y / 8), 6);
        if (free) addUnit(w, createUnit(this.state, this.catalog, d.def, "", centre(free.cx), centre(free.cy)));
        return;
      }
    }
  }
}
