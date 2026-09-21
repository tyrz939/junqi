// AI is an MMO loop, not a chase script (scr_ai_logic.gml):
//
//   idle    regen, patrol, aggro scan every 10 ticks (staggered), needs LOS
//   combat  drop bad targets, leash check, first affordable spell in the book,
//           tooFar / notInLOS -> path in at run speed, else stand and fight
//   leash   clear target, regen, run home, then idle
//
// The book's order IS the priority. The AI calls the same tryCast as the player.
//
// Three rows change the loop, and none of them is a class:
//   order        (the `send` verb) it walks to a point and minds nothing else until it arrives
//   sight: lit   it only notices, and only keeps, a target that stands in a prop's light
//   shunsLight   it will not step into warm light: it walks to the edge of it and waits there

import { AGGRO_PERIOD, CELL, PATHS_PER_TICK, PATH_BUDGET, PX_PER_METRE, REGEN_DIVISOR, REPATH_TICKS } from "@/sim/constants";
import { cellOf, centre } from "@/sim/grid";
import { lineOfSight } from "@/sim/los";
import { costOfCells, PATH_WINDOW } from "@/sim/path";
import { asPlayer, type World } from "@/sim/runtime";
import type { Unit } from "@/sim/state";
import { runActions } from "@/sim/actions";
import { metresBetween, resetPhases, tryCast } from "@/sim/combat";
import { LitField, litAt } from "@/sim/light";
import { speedFactor } from "@/sim/status";
import { distance, facePoint, faceVector, isEnemy, maxHp, maxMp, moveUnit, setAnim } from "@/sim/units";

/** Scratch for a search by something that shuns light. Filled just before each search; never state. */
const shunField = new LitField();

/** How far a sent unit will plan in one go, in metres. Further than that it walks in stages. */
const ORDER_PATH_METRES = 400;

/**
 * Units nobody fights (the dog, a butterfly) have no AI loop, but they can be sent somewhere
 * and they can keep a patrol: a butterfly goes from flower to flower and sits on each.
 */
export function tickNpc(w: World, u: Unit): void {
  if (u.order) {
    followOrder(w, u);
    return;
  }
  if (u.patrol) patrol(w, u, w.catalog.units[u.def].walk, false);
}

export function tickAi(w: World, u: Unit): void {
  const def = w.catalog.units[u.def];
  if (u.order) {
    followOrder(w, u);
    return;
  }
  const shy = def.shunsLight === true;
  // A boss in a later phase may have another speed.
  const run = (u.phase > 0 ? def.phases?.[u.phase - 1]?.run : undefined) ?? def.run;
  if (u.combat === "idle") {
    regen(w, u, def.autoRegen);
    if ((w.state.tick + u.thinkOffset) % AGGRO_PERIOD === 0) {
      const found = nearestEnemy(w, u, def.aggro, def.sight === "lit");
      if (found) {
        u.target = found.id;
        u.combat = "combat";
        u.path = null;
        return;
      }
    }
    if (def.bait && seekBait(w, u, def.bait, def.aggro)) return;
    patrol(w, u, def.walk, shy);
    return;
  }

  if (u.combat === "leash") {
    regen(w, u, true);
    u.target = 0;
    const d = distance(u.x, u.y, u.homeX, u.homeY);
    if (d <= Math.max(run, 1.5)) {
      moveUnit(w, u, u.homeX - u.x, u.homeY - u.y);
      u.combat = "idle";
      u.path = null;
      setAnim(u, "idle");
      return;
    }
    // Caught in the light it goes home by the straight way; otherwise it keeps to the dark.
    const round = shy && !litAt(w, u.x, u.y, true);
    const stuck = !followTo(w, u, u.homeX, u.homeY, run, def.leash * 4, round) || (round && u.path !== null && u.pathAt >= u.path.length);
    if (stuck) {
      // Cannot path home (door shut behind it, or its post is lit now): give up and stand guard here.
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
  // Light is how it sees: a target that steps into the dark is a target it no longer has.
  if (def.sight === "lit" && !litAt(w, target.x, target.y)) {
    u.target = 0;
    u.combat = "leash";
    u.path = null;
    return;
  }
  // Warm light keeps it off: standing in it, it does nothing but leave.
  if (shy && litAt(w, u.x, u.y, true)) {
    u.target = 0;
    u.combat = "leash";
    u.path = null;
    return;
  }
  if (u.stop > 0) return;
  const spell = pickSpell(w, u);
  if (!spell) {
    approach(w, u, target, run, def.leash, shy);
    return;
  }
  const result = tryCast(w, u, spell);
  if (result === "castSuccessful") {
    u.path = null;
    facePoint(u, target.x, target.y);
    return;
  }
  if (result === "tooFar" || result === "notInLOS") {
    approach(w, u, target, run, def.leash, shy);
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
  // Whole again: a boss that was let off starts its fight from the top.
  if (u.phase !== 0 && u.hp >= hp) resetPhases(w, u);
}

/**
 * Sent somewhere (`send`): walk there, minding nothing. No aggro, no leash, no patrol. On
 * arrival its list runs, with it as the subject, on behalf of whoever sent it if she is still
 * here. When the tick budget runs out it gives up where it stands and nothing happens.
 */
function followOrder(w: World, u: Unit): void {
  const o = u.order;
  if (!o) return;
  const def = w.catalog.units[u.def];
  u.target = 0;
  u.combat = "idle";
  const speed = Math.max(def.run, def.walk);
  if (--o.left <= 0 || speed <= 0) {
    u.order = null;
    u.path = null;
    if (u.anim === "walk") setAnim(u, "idle");
    return;
  }
  // Beside the mark will do: someone may be standing on it.
  if (distance(u.x, u.y, o.x, o.y) <= CELL * 1.5) {
    u.order = null;
    u.path = null;
    u.homeX = u.x;
    u.homeY = u.y;
    if (u.anim === "walk") setAnim(u, "idle");
    if (o.then && o.then.length > 0) {
      const then = o.then;
      const sender = o.seat >= 0 ? w.state.players[o.seat] : undefined;
      const here = sender && sender.connected && sender.zone === w.zone.id && w.rt.units.has(sender.unitId) ? sender : null;
      asPlayer(w, here, () => runActions(w, then, u.id));
    }
    return;
  }
  followTo(w, u, o.x, o.y, speed, ORDER_PATH_METRES, def.shunsLight === true);
}

export function nearestEnemy(w: World, u: Unit, metres: number, litOnly = false): Unit | null {
  let best: Unit | null = null;
  let bestD = metres;
  for (const other of w.zone.units) {
    if (other === u || !other.alive || !other.awake || !isEnemy(u, other)) continue;
    if (other.controller === "npc") continue;
    if (other.hidden || w.party.ofUnit(other.id)?.god) continue;
    const d = metresBetween(w, u, other);
    if (d > bestD) continue;
    if (litOnly && !litAt(w, other.x, other.y)) continue;
    if (!lineOfSight(w.rt.grid, u.x, u.y, other.x, other.y)) continue;
    bestD = d;
    best = other;
  }
  return best;
}

function approach(w: World, u: Unit, target: Unit, speed: number, leashMetres: number, shy = false): void {
  if (speed <= 0) {
    facePoint(u, target.x, target.y);
    return;
  }
  // Max path length is leash*2 metres, as in 2020. Fail -> leash.
  if (!followTo(w, u, target.x, target.y, speed, leashMetres * 2, shy)) {
    if (u.repathIn <= 0) u.combat = "leash";
  }
  // At the edge of her light with nowhere nearer to stand: it waits, and it watches her.
  if (shy && u.path !== null && u.pathAt >= u.path.length) {
    facePoint(u, target.x, target.y);
    if (u.anim === "walk") setAnim(u, "idle");
  }
}

/**
 * Walk toward a pixel goal along a cached cell path. Re-plans when the path is
 * used up, when the goal has wandered, or every REPATH_TICKS; at most
 * PATHS_PER_TICK searches run per tick across all units, the rest wait a tick.
 * Returns false only when a search ran and found nothing.
 *
 * `shy`: the walker shuns warm light. Its searches go round lit cells, and one that cannot
 * reach the goal ends at the nearest dark cell, where it stands until the next re-plan (a path
 * that is used up is not stale for it, or it would search on every tick it spent waiting).
 */
function followTo(w: World, u: Unit, gx: number, gy: number, speed: number, maxMetres: number, shy = false): boolean {
  const grid = w.rt.grid;
  u.repathIn--;
  const goalCell = grid.index(cellOf(gx), cellOf(gy));
  const used = !u.path || u.pathAt >= u.path.length;
  const stale = shy ? !u.path || u.pathGoal !== goalCell : used || u.pathGoal !== goalCell;
  if ((stale || u.repathIn <= 0) && w.rt.pathsThisTick < PATHS_PER_TICK) {
    w.rt.pathsThisTick++;
    u.repathIn = REPATH_TICKS;
    const sx = cellOf(u.x);
    const sy = cellOf(u.y);
    if (shy) {
      const half = PATH_WINDOW >> 1;
      shunField.gather(w, sx - half, sy - half, sx + half, sy + half, true);
    }
    const found = w.rt.path.find(
      sx,
      sy,
      cellOf(gx),
      cellOf(gy),
      u.id,
      costOfCells((maxMetres * PX_PER_METRE) / CELL),
      PATH_BUDGET,
      shy && shunField.count > 0 ? shunField : null,
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
    // A lamp came on across its way since it planned: stop short, and think again soon.
    if (shy && litAt(w, centre(cx), centre(cy), true)) {
      u.repathIn = Math.min(u.repathIn, 4);
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

function patrol(w: World, u: Unit, speed: number, shy: boolean): void {
  if (!u.patrol || u.patrol.length < 4 || speed <= 0) {
    if (u.anim === "walk") setAnim(u, "idle");
    return;
  }
  // Standing at a point it has reached: a butterfly on its flower, the Caretaker at a door.
  if (u.dwell > 0) {
    u.dwell--;
    if (u.anim === "walk") setAnim(u, "idle");
    return;
  }
  const n = u.patrol.length / 2;
  const at = u.patrolAt % n;
  const tx = u.patrol[at * 2];
  const ty = u.patrol[at * 2 + 1];
  if (distance(u.x, u.y, tx, ty) <= CELL) {
    u.dwell = u.patrolDwell?.[at] ?? 0;
    u.patrolAt = (at + 1) % n;
    u.path = null;
    return;
  }
  followTo(w, u, tx, ty, speed, 200, shy);
  // Home follows the patrol so a leash returns to the route, not the spawn.
  u.homeX = u.x;
  u.homeY = u.y;
}
