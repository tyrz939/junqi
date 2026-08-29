import { getSpell } from "@/game/systems/catalog";
import { metres, tryCast } from "@/game/systems/combat";
import { hasLos } from "@/game/systems/los";
import { astar } from "@/game/systems/path";
import { speedMul } from "@/game/systems/status";
import type { Grid } from "@/game/world/Grid";
import type { Unit } from "@/game/entities/Unit";

export type AiActor = {
  unit: Unit;
  x: number;
  y: number;
  leashX: number;
  leashY: number;
  path: { x: number; y: number }[];
  patrol?: { x: number; y: number }[];
  patrolI?: number;
};

export type AiTarget = { unit: Unit; x: number; y: number };

export type AiEvent =
  | { type: "move"; x: number; y: number }
  | { type: "cast"; spellId: string };

export function tickAi(
  actor: AiActor,
  target: AiTarget | null,
  grid: Grid,
  dt: number,
): AiEvent[] {
  const events: AiEvent[] = [];
  const u = actor.unit;
  if (!u.alive) return events;
  const mul = speedMul(u);
  if (mul === 0) return events;

  const distHome = metres(Math.hypot(actor.x - actor.leashX, actor.y - actor.leashY));
  const distTarget =
    target && target.unit.alive
      ? metres(Math.hypot(actor.x - target.x, actor.y - target.y))
      : 999;
  const los = target ? hasLos(grid, actor.x, actor.y, target.x, target.y) : false;

  if (u.combatState === "leashing") {
    if (distHome < 0.4) {
      events.push({ type: "move", x: actor.leashX, y: actor.leashY });
      u.combatState = "idle";
      u.currentTargetId = null;
      actor.path = [];
      return events;
    }
    followPath(actor, grid, actor.leashX, actor.leashY, u.runSpd * mul, dt, events, 80);
    return events;
  }

  if (target && target.unit.alive && ((los && distTarget < u.aggroRange) || u.currentTargetId === target.unit.id)) {
    if (distHome > u.leashRange || distTarget > u.leashRange * 2) {
      u.combatState = "leashing";
      u.currentTargetId = null;
      return events;
    }
    u.combatState = "combat";
    u.currentTargetId = target.unit.id;
    const spellId = pickSpell(u);
    if (!spellId) return events;
    const result = tryCast(u, spellId, target.unit, distTarget, los);
    if (result === "castSuccessful") events.push({ type: "cast", spellId });
    else if (result === "tooFar" || result === "notInLOS") {
      followPath(actor, grid, target.x, target.y, u.runSpd * mul, dt, events, u.leashRange * 2);
    }
    return events;
  }

  u.combatState = "idle";
  if (u.hp < u.maxhp) u.hp = Math.min(u.maxhp, u.hp + (u.maxhp * dt) / 20);
  const patrol = actor.patrol;
  if (patrol && patrol.length > 0) {
    actor.patrolI = actor.patrolI ?? 0;
    const dest = patrol[actor.patrolI % patrol.length];
    if (Math.hypot(actor.x - dest.x, actor.y - dest.y) < 4) actor.patrolI += 1;
    else followPath(actor, grid, dest.x, dest.y, u.walkSpd * mul, dt, events, 40);
  }
  return events;
}

function pickSpell(unit: Unit): string | null {
  for (const id of unit.spellbook) {
    const spell = getSpell(id);
    if ((unit.cooldowns[id] ?? 0) > 0) continue;
    if (unit.mp < spell.mpCost) continue;
    if (unit.gcd > 0 && !spell.gcdImmune) continue;
    return id;
  }
  return unit.spellbook[0] ?? null;
}

function followPath(
  actor: AiActor,
  grid: Grid,
  tx: number,
  ty: number,
  speed: number,
  dt: number,
  events: AiEvent[],
  maxM: number,
): void {
  if (actor.path.length < 2) {
    actor.path = astar(grid, actor.x, actor.y, tx, ty, maxM, actor.unit.id) ?? [];
  }
  const next = actor.path[1] ?? actor.path[0];
  if (!next) return;
  const dx = next.x - actor.x;
  const dy = next.y - actor.y;
  const len = Math.hypot(dx, dy) || 1;
  if (len < 2) {
    actor.path.shift();
    return;
  }
  events.push({
    type: "move",
    x: actor.x + (dx / len) * speed * dt,
    y: actor.y + (dy / len) * speed * dt,
  });
}
