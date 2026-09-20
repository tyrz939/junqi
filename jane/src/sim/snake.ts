// The snake boss: the one permitted custom mover (SYSTEMS.md §6). Everything
// else about it is ordinary: it is a Unit, it casts through tryCast, it takes
// damage through the incoming queue. Only locomotion and the phase clock are its own.
//
// Contract, read from objects/obj_snake_boss (2020):
//   idle     steer round the patrol loop at walk speed
//   phase 0  "Follow": 900 ticks, steer at the player at run speed, turn rate
//            limited to speed*4 degrees per tick, melee-only book
//   phase 1  "Spit": steer home; once within 16 px, 300 ticks of poison rings
//   reset    if a wall cuts the line to the player, snap home at full HP (anti-cheese)
//   body     a node every 4th moving tick, 64 max; every 8th node is a hitbox
//            that forwards damage to the head
// Phases are timer-only. There are no HP thresholds; 2020 had none either.

import { CELL } from "@/sim/constants";
import { lineOfSightWalls } from "@/sim/los";
import { playersHere, type World } from "@/sim/runtime";
import type { Unit } from "@/sim/state";
import { normalize, turnToward } from "@/sim/angles";
import { metresBetween, tryCast } from "@/sim/combat";
import { clearStatuses } from "@/sim/status";
import { distance, faceVector, maxHp, maxMp, moveUnit, placeUnit, setAnim } from "@/sim/units";

export const SNAKE_FOLLOW_TICKS = 900;
export const SNAKE_SPIT_TICKS = 300;
const NODE_EVERY = 4;
const MAX_NODES = 64;
const HITBOX_EVERY = 8;

export function tickSnake(w: World, u: Unit): void {
  const def = w.catalog.units[u.def];
  if (u.combat !== "combat") {
    if (u.hp < maxHp(u)) u.hp = Math.min(maxHp(u), u.hp + maxHp(u) / 300);
    // After a reset it must be able to pick the fight back up. 2020's aggro trigger
    // re-fired while you stood in it; here the snake looks for itself, on the usual beat,
    // at whoever in the party is nearest.
    if ((w.state.tick + u.thinkOffset) % 10 === 0) {
      let prey: Unit | null = null;
      let best = def.aggro * 8;
      for (const p of playersHere(w)) {
        const body = w.rt.units.get(p.unitId);
        if (!body || !body.alive || p.god) continue;
        const d = distance(u.x, u.y, body.x, body.y);
        if (d <= best && lineOfSightWalls(w.rt.grid, u.x, u.y, body.x, body.y)) {
          best = d;
          prey = body;
        }
      }
      if (prey) {
        u.target = prey.id;
        u.combat = "combat";
        u.phase = 0;
        u.phaseTick = 0;
        return;
      }
    }
    if (u.patrol && u.patrol.length >= 4) {
      const n = u.patrol.length / 2;
      const tx = u.patrol[(u.patrolAt % n) * 2];
      const ty = u.patrol[(u.patrolAt % n) * 2 + 1];
      if (distance(u.x, u.y, tx, ty) <= CELL * 1.5) u.patrolAt = (u.patrolAt + 1) % n;
      steer(w, u, tx, ty, def.walk);
    }
    return;
  }

  const target = u.target ? w.rt.units.get(u.target) : undefined;
  if (!target || !target.alive) {
    resetSnake(w, u);
    return;
  }
  // Walls only. 2020 tested obj_wall on the line; its pillars were block_here and never counted.
  if (!lineOfSightWalls(w.rt.grid, u.x, u.y, target.x, target.y)) {
    resetSnake(w, u);
    return;
  }

  const phases = def.phases ?? [];
  const book = phases[u.phase]?.book ?? def.book;
  const speed = phases[u.phase]?.run ?? def.run;
  const touching = metresBetween(w, u, target) <= 0;

  if (u.phase === 0) {
    if (!touching) {
      steer(w, u, target.x, target.y, speed);
      u.phaseTick++;
    }
    for (const spell of book) if (tryCast(w, u, spell) === "castSuccessful") break;
    if (u.phaseTick >= SNAKE_FOLLOW_TICKS) {
      u.phase = 1;
      u.phaseTick = 0;
    }
    return;
  }

  // phase 1: go home first; the spit clock only runs once coiled there.
  if (distance(u.x, u.y, u.homeX, u.homeY) > 16) {
    steer(w, u, u.homeX, u.homeY, speed);
    return;
  }
  setAnim(u, "cast");
  for (const spell of book) if (tryCast(w, u, spell) === "castSuccessful") break;
  if (++u.phaseTick >= SNAKE_SPIT_TICKS) {
    u.phase = 0;
    u.phaseTick = 0;
  }
}

function steer(w: World, u: Unit, tx: number, ty: number, speed: number): void {
  if (speed <= 0) return;
  const want = normalize(tx - u.x, ty - u.y);
  if (want.x === 0 && want.y === 0) return;
  const h = turnToward(u.hx, u.hy, want.x, want.y, speed * 4);
  u.hx = h.x;
  u.hy = h.y;
  const moved = moveUnit(w, u, h.x * speed, h.y * speed);
  faceVector(u, h.x, h.y);
  setAnim(u, "walk");
  if (!moved || !u.segments) return;
  u.phaseStep++;
  if (u.phaseStep % NODE_EVERY !== 0) return;
  u.segments.unshift(u.x, u.y);
  if (u.segments.length > MAX_NODES * 2) u.segments.length = MAX_NODES * 2;
}

function resetSnake(w: World, u: Unit): void {
  placeUnit(w, u, u.homeX, u.homeY);
  u.hp = maxHp(u);
  u.mp = maxMp(u);
  u.target = 0;
  u.combat = "idle";
  u.phase = 0;
  u.phaseTick = 0;
  u.incoming.length = 0;
  clearStatuses(w, u);
  if (u.segments) for (let i = 0; i < u.segments.length; i += 2) {
    u.segments[i] = u.homeX;
    u.segments[i + 1] = u.homeY;
  }
  setAnim(u, "idle");
}

/**
 * Closest distance from a point to anything of this unit that can be hit: the
 * head, plus every 8th body node for segmented units. Bolts and melee both ask this.
 */
export function bodyDistance(u: Unit, x: number, y: number): number {
  let best = distance(u.x, u.y, x, y);
  if (!u.segments) return best;
  for (let n = HITBOX_EVERY - 1; n * 2 + 1 < u.segments.length; n += HITBOX_EVERY) {
    const d = distance(u.segments[n * 2], u.segments[n * 2 + 1], x, y);
    if (d < best) best = d;
  }
  return best;
}
