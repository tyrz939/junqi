// AI is an MMO loop, not a chase script (scr_ai_logic.gml):
//
//   idle    regen, patrol, aggro scan every 10 ticks (staggered), needs LOS
//   combat  drop bad targets, leash check, first affordable spell in the book,
//           tooFar / notInLOS -> path in at run speed, else stand and fight
//   leash   clear target, regen, run home, then idle
//
// The book's order IS the priority. The AI calls the same tryCast as the player.

import { AGGRO_PERIOD, CELL, PATHS_PER_TICK, PATH_BUDGET, PX_PER_METRE, REGEN_DIVISOR, REPATH_TICKS } from "@/sim/constants";
import { cellOf, centre } from "@/sim/grid";
import { lineOfSight } from "@/sim/los";
import { costOfCells } from "@/sim/path";
import type { World } from "@/sim/runtime";
import type { Unit } from "@/sim/state";
import { metresBetween, tryCast } from "@/sim/combat";
import { speedFactor } from "@/sim/status";
import { distance, facePoint, faceVector, isEnemy, maxHp, maxMp, moveUnit, setAnim } from "@/sim/units";

export function tickAi(w: World, u: Unit): void {
  const def = w.catalog.units[u.def];
  if (u.combat === "idle") {
    regen(w, u, def.autoRegen);
    if ((w.state.tick + u.thinkOffset) % AGGRO_PERIOD === 0) {
      const found = nearestEnemy(w, u, def.aggro);
      if (found) {
        u.target = found.id;
        u.combat = "combat";
        u.path = null;
        return;
      }
    }
    if (def.bait && seekBait(w, u, def.bait, def.aggro)) return;
    patrol(w, u, def.walk);
    return;
  }

  if (u.combat === "leash") {
    regen(w, u, true);
    u.target = 0;
    const d = distance(u.x, u.y, u.homeX, u.homeY);
    if (d <= Math.max(def.run, 1.5)) {
      moveUnit(w, u, u.homeX - u.x, u.homeY - u.y);
      u.combat = "idle";
      u.path = null;
      setAnim(u, "idle");
      return;
    }
    if (!followTo(w, u, u.homeX, u.homeY, def.run, def.leash * 4)) {
      // Cannot path home (door shut behind it): give up and stand guard here.
      u.homeX = u.x;
      u.homeY = u.y;
      u.combat = "idle";
    }
    return;
  }

  // combat
  const target = u.target ? w.rt.units.get(u.target) : undefined;
  if (!target || !target.alive || !isEnemy(u, target)) {
    u.target = 0;
    u.combat = "leash";
    return;
  }
  if (distance(u.x, u.y, u.homeX, u.homeY) / PX_PER_METRE > def.leash) {
    u.combat = "leash";
    u.path = null;
    return;
  }
  if (u.stop > 0) return;
  const spell = pickSpell(w, u);
  if (!spell) {
    approach(w, u, target, def.run, def.leash);
    return;
  }
  const result = tryCast(w, u, spell);
  if (result === "castSuccessful") {
    u.path = null;
    facePoint(u, target.x, target.y);
    return;
  }
  if (result === "tooFar" || result === "notInLOS") {
    approach(w, u, target, def.run, def.leash);
    return;
  }
  // onCooldown / onGCD / no mana: hold position, keep facing the fight.
  facePoint(u, target.x, target.y);
  if (u.anim === "walk") setAnim(u, "idle");
}

/** First spell in the book that is off cooldown and affordable. Order is priority. */
function pickSpell(w: World, u: Unit): string | null {
  for (const id of u.book) {
    const s = w.catalog.spells[id];
    if ((u.cooldowns[id] ?? 0) > 0) continue;
    if (u.mp < s.mp || u.energy < s.energy) continue;
    if (u.gcd > 0 && !s.gcdImmune) continue;
    return id;
  }
  // Everything is cooling down: report the first one so range/LOS still drive movement.
  return u.book[0] ?? null;
}

function regen(w: World, u: Unit, on: boolean): void {
  if (!on || !u.alive) return;
  const hp = maxHp(u);
  const mp = maxMp(u);
  if (u.hp < hp) u.hp = Math.min(hp, u.hp + hp / REGEN_DIVISOR);
  if (u.mp < mp) u.mp = Math.min(mp, u.mp + mp / REGEN_DIVISOR);
  void w;
}

export function nearestEnemy(w: World, u: Unit, metres: number): Unit | null {
  let best: Unit | null = null;
  let bestD = metres;
  for (const other of w.zone.units) {
    if (other === u || !other.alive || !other.awake || !isEnemy(u, other)) continue;
    if (other.controller === "npc") continue;
    if (other.hidden || w.party.ofUnit(other.id)?.god) continue;
    const d = metresBetween(w, u, other);
    if (d > bestD) continue;
    if (!lineOfSight(w.rt.grid, u.x, u.y, other.x, other.y)) continue;
    bestD = d;
    best = other;
  }
  return best;
}

function approach(w: World, u: Unit, target: Unit, speed: number, leashMetres: number): void {
  if (speed <= 0) {
    facePoint(u, target.x, target.y);
    return;
  }
  // Max path length is leash*2 metres, as in 2020. Fail -> leash.
  if (!followTo(w, u, target.x, target.y, speed, leashMetres * 2)) {
    if (u.repathIn <= 0) u.combat = "leash";
  }
}

/**
 * Walk toward a pixel goal along a cached cell path. Re-plans when the path is
 * used up, when the goal has wandered, or every REPATH_TICKS; at most
 * PATHS_PER_TICK searches run per tick across all units, the rest wait a tick.
 * Returns false only when a search ran and found nothing.
 */
function followTo(w: World, u: Unit, gx: number, gy: number, speed: number, maxMetres: number): boolean {
  const grid = w.rt.grid;
  u.repathIn--;
  const goalCell = grid.index(cellOf(gx), cellOf(gy));
  const stale = !u.path || u.pathAt >= u.path.length || u.pathGoal !== goalCell;
  if ((stale || u.repathIn <= 0) && w.rt.pathsThisTick < PATHS_PER_TICK) {
    w.rt.pathsThisTick++;
    u.repathIn = REPATH_TICKS;
    const found = w.rt.path.find(
      cellOf(u.x),
      cellOf(u.y),
      cellOf(gx),
      cellOf(gy),
      u.id,
      costOfCells((maxMetres * PX_PER_METRE) / CELL),
      PATH_BUDGET,
    );
    if (!found) {
      u.path = null;
      return false;
    }
    u.path = found;
    u.pathGoal = goalCell;
    u.pathAt = 0;
  }
  if (!u.path || u.pathAt >= u.path.length) return true;

  const f = speedFactor(w, u);
  let budget = speed * f;
  if (budget <= 0) return true;
  while (budget > 0 && u.pathAt < u.path.length) {
    const cell = u.path[u.pathAt];
    const cx = cell % grid.w;
    const cy = (cell - cx) / grid.w;
    // Held by someone else. If it is the goal, that is the target's own feet: stop
    // beside it. Otherwise someone stepped in since planning: wait, re-plan soon.
    if (!grid.free(cx, cy, u.id)) {
      if (u.pathAt < u.path.length - 1) u.repathIn = Math.min(u.repathIn, 4);
      break;
    }
    const tx = centre(cx);
    const ty = centre(cy);
    const d = distance(u.x, u.y, tx, ty);
    if (d <= budget) {
      moveUnit(w, u, tx - u.x, ty - u.y);
      budget -= d;
      u.pathAt++;
    } else {
      const dx = ((tx - u.x) / d) * budget;
      const dy = ((ty - u.y) / d) * budget;
      faceVector(u, dx, dy);
      moveUnit(w, u, dx, dy);
      budget = 0;
    }
  }
  setAnim(u, "walk");
  return true;
}

/**
 * The 2020 burial snakes could not be fought (600 HP, 120-180 a bolt) but would
 * walk to poisoned rat meat and die on it. `bait` makes that a row, not a class.
 */
function seekBait(w: World, u: Unit, item: string, metres: number): boolean {
  for (let i = 0; i < w.zone.drops.length; i++) {
    const d = w.zone.drops[i];
    if (d.item !== item) continue;
    const dist = distance(u.x, u.y, d.x, d.y);
    if (dist > metres * PX_PER_METRE) continue;
    if (!lineOfSight(w.rt.grid, u.x, u.y, d.x, d.y)) continue;
    if (dist <= 16) {
      w.zone.drops.splice(i, 1);
      u.incoming.push({ amount: 10000, school: "nature", from: 0, crit: false });
      return true;
    }
    followTo(w, u, d.x, d.y, Math.max(0.5, w.catalog.units[u.def].walk), metres * 2);
    return true;
  }
  return false;
}

function patrol(w: World, u: Unit, speed: number): void {
  if (!u.patrol || u.patrol.length < 4 || speed <= 0) {
    if (u.anim === "walk") setAnim(u, "idle");
    return;
  }
  const n = u.patrol.length / 2;
  const tx = u.patrol[(u.patrolAt % n) * 2];
  const ty = u.patrol[(u.patrolAt % n) * 2 + 1];
  if (distance(u.x, u.y, tx, ty) <= CELL) {
    u.patrolAt = (u.patrolAt + 1) % n;
    u.path = null;
    return;
  }
  followTo(w, u, tx, ty, speed, 200);
  // Home follows the patrol so a leash returns to the route, not the spawn.
  u.homeX = u.x;
  u.homeY = u.y;
}
