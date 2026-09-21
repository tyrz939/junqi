// Zones: built on first visit, kept forever in state.zones. The derived runtime of
// a zone exists only while a player is in it (the scheduler in sim.ts owns that map;
// up to four people may be in four different zones). Travel is a request on the
// player, performed by the scheduler at the end of the tick, so nothing is halfway
// through iterating a unit list when the list changes.

import type { Catalog } from "@/sim/catalog";
import { CELL, PHASE_SCALE } from "@/sim/constants";
import { centre } from "@/sim/grid";
import { playerOf, playersHere, zoneTriggers, type World } from "@/sim/runtime";
import type { GameState, PlayerState, Prop, TravelRequest, Unit, ZoneId, ZoneState } from "@/sim/state";
import { createUnit, maxHp, maxMp, setAnim } from "@/sim/units";
import type { Blueprint } from "@/world/blueprint";
import { buildZone } from "@/world/index";

const blueprintCache = new Map<string, Blueprint>();

export function blueprintFor(zone: ZoneId, seed: number): Blueprint {
  const key = `${seed}:${zone}`;
  let bp = blueprintCache.get(key);
  if (!bp) {
    bp = buildZone(zone, seed);
    blueprintCache.set(key, bp);
    if (blueprintCache.size > 12) blueprintCache.delete(blueprintCache.keys().next().value as string);
  }
  return bp;
}

/**
 * Hand the cache a blueprint instead of building one. The template harness plays a single
 * room this way, as if it were the whole zone. Nothing in the game calls it.
 */
export function primeBlueprint(zone: ZoneId, seed: number, bp: Blueprint): void {
  blueprintCache.set(`${seed}:${zone}`, bp);
}

export function createZoneState(state: GameState, catalog: Catalog, bp: Blueprint): ZoneState {
  const units: Unit[] = [];
  for (const s of bp.units) {
    const u = createUnit(state, catalog, s.def, s.key, centre(s.cx), centre(s.cy), s.facing ?? 1);
    if (s.phase !== undefined && s.phase > 1) {
      // Strength is health (x5) and melee; spirit is mana and spell power. Both follow the phase.
      const m = PHASE_SCALE[Math.min(PHASE_SCALE.length - 1, s.phase)];
      u.strength = Math.round(u.strength * m);
      u.spirit = Math.round(u.spirit * m);
      u.hp = maxHp(u);
      u.mp = maxMp(u);
    }
    if (s.patrol && s.patrol.length > 1) {
      u.patrol = [];
      for (const [cx, cy] of s.patrol) u.patrol.push(centre(cx), centre(cy));
      // A third number on a waypoint is how long it stands there, in ticks. Most routes have none.
      if (s.patrol.some((p) => (p[2] ?? 0) > 0)) u.patrolDwell = s.patrol.map((p) => Math.max(0, Math.floor(p[2] ?? 0)));
    }
    units.push(u);
  }
  const props: Prop[] = bp.props.map((s) => {
    const def = catalog.props[s.def];
    if (!def) throw new Error(`Blueprint ${bp.zone}: unknown prop def "${s.def}" (${s.key})`);
    const locked = s.locked ?? false;
    return {
      id: state.nextId++,
      key: s.key,
      def: s.def,
      cx: s.cx,
      cy: s.cy,
      // A gate is a door that leads nowhere: shut while locked, open otherwise.
      solid: def.gate ? locked : def.solid,
      hidden: s.hidden ?? false,
      locked,
      used: false,
      on: s.on ?? false,
      keyTag: s.keyTag ?? "",
      to: s.to ?? null,
      loot: s.loot ? s.loot.map((l) => ({ ...l })) : null,
      use: s.use ?? null,
      release: s.release ?? null,
      needs: s.needs ?? null,
      talk: s.talk ?? "",
      label: s.label ?? "",
      nightLock: s.nightLock ?? "",
      awake: true,
    };
  });
  const triggers = Object.keys(zoneTriggers(catalog, bp)).map((id) => ({ id, fired: false, inside: false }));
  return { id: bp.zone, units, props, drops: [], projectiles: [], grounds: [], triggers, tileDeltas: [], fog: [], pendingFill: [] };
}

/** The saved state of a zone, created from its blueprint on the first visit. */
export function ensureZoneState(state: GameState, catalog: Catalog, zone: ZoneId): { zs: ZoneState; bp: Blueprint; first: boolean } {
  const bp = blueprintFor(zone, state.seed);
  let zs = state.zones[zone];
  const first = !zs;
  if (!zs) {
    zs = createZoneState(state, catalog, bp);
    state.zones[zone] = zs;
  }
  // New trigger rows added since the save was written still get a state entry.
  for (const id of Object.keys(zoneTriggers(catalog, bp))) {
    if (!zs.triggers.some((x) => x.id === id)) zs.triggers.push({ id, fired: false, inside: false });
  }
  return { zs, bp, first };
}

/** Ask to change zone. Performed at the end of the tick. Refused while carrying something. */
export function requestTravel(w: World, zone: ZoneId, mark: string, at?: TravelRequest["at"]): void {
  if (!w.actor) return;
  const body = playerOf(w);
  if (body.carrying) {
    // The Phaser build force-dropped the crate onto the player's own cell. Just say no.
    w.emit({ e: "toast", text: "I should put this down first" });
    return;
  }
  w.actor.travel = { zone, mark, at };
}

/**
 * Put an arriving player's body down in this zone: at an exact spot (waking at a bed
 * or fire) or at the named mark, on the nearest free cell. The caller has already
 * removed her from where she was and will add her here.
 */
export function placeArrival(w: World, player: PlayerState, body: Unit, req: TravelRequest): void {
  const mark = req.at
    ? { cx: Math.floor(req.at.x / CELL), cy: Math.floor(req.at.y / CELL), facing: undefined }
    : (w.rt.bp.marks[req.mark] ?? w.rt.bp.marks.start);
  if (!mark) throw new Error(`Zone ${w.zone.id} has no mark "${req.mark}" and no "start"`);
  const free = w.rt.grid.nearestFree(mark.cx, mark.cy, 8) ?? mark;
  body.x = centre(free.cx);
  body.y = centre(free.cy);
  if (mark.facing !== undefined) body.facing = mark.facing;
  body.path = null;
  body.target = 0;
  body.hold = 0;
  setAnim(body, "idle");
  player.zone = w.zone.id;
  if (!req.at) player.lastMark = req.mark;
}

// --- fog -----------------------------------------------------------------------
// One bit per 16 px block, interiors only. Lives in ZoneState so it saves for free,
// and is shared: what one of the party has seen, the map shows to all of them.

const FOG_RADIUS = 9; // blocks = 144 px, a little over half the view height

export function stampFog(w: World): void {
  if (!w.rt.bp.indoor) return;
  const fog = w.zone.fog;
  const fw = w.rt.fogW;
  for (const p of playersHere(w)) {
    const body = w.rt.units.get(p.unitId);
    if (!body) continue;
    const bx = Math.floor(body.x / (CELL * 2));
    const by = Math.floor(body.y / (CELL * 2));
    for (let y = by - FOG_RADIUS; y <= by + FOG_RADIUS; y++) {
      if (y < 0 || y >= w.rt.fogH) continue;
      for (let x = bx - FOG_RADIUS; x <= bx + FOG_RADIUS; x++) {
        if (x < 0 || x >= fw) continue;
        const dx = x - bx;
        const dy = y - by;
        if (dx * dx + dy * dy > FOG_RADIUS * FOG_RADIUS) continue;
        const bit = y * fw + x;
        const word = bit >> 5;
        while (fog.length <= word) fog.push(0);
        fog[word] |= 1 << (bit & 31);
      }
    }
  }
}

/**
 * The wall notice as the map (`reveal`): mark a rect, and the wall round it, as seen. The fog
 * is the party's, so what one of them reads off the wall all of them have on their map.
 */
export function revealRect(w: World, r: { cx: number; cy: number; w: number; h: number }): void {
  if (!w.rt.bp.indoor) return;
  const fog = w.zone.fog;
  const fw = w.rt.fogW;
  const bx0 = Math.max(0, (r.cx - 1) >> 1);
  const by0 = Math.max(0, (r.cy - 1) >> 1);
  const bx1 = Math.min(fw - 1, (r.cx + r.w) >> 1);
  const by1 = Math.min(w.rt.fogH - 1, (r.cy + r.h) >> 1);
  for (let y = by0; y <= by1; y++) {
    for (let x = bx0; x <= bx1; x++) {
      const bit = y * fw + x;
      const word = bit >> 5;
      while (fog.length <= word) fog.push(0);
      fog[word] |= 1 << (bit & 31);
    }
  }
}

export function fogSeen(fog: readonly number[], fogW: number, bx: number, by: number): boolean {
  const bit = by * fogW + bx;
  return ((fog[bit >> 5] ?? 0) & (1 << (bit & 31))) !== 0;
}
