// The cast pipeline is the product (SYSTEMS.md §3):
//   1. validate   dead, stunned, cooldown, GCD, MP, energy, target, enemy, LOS, range
//   2. spawn a KIND (melee / bolt / self / world / ground), never a class
//   3. pay        only if the kind says the cast was valid
//   4. queue      damage goes to `incoming`, never straight to hp
//   5. flush      later in the same tick, in one place (flushIncoming)
// Player and AI call the same tryCast. Failure is an enum, not a silent no-op.
//
// Order matters and is a fix: 2020's CastSpell read `current_target.faction`
// before `instance_exists(current_target)`. Here the target is resolved to a
// live Unit (or null) before anything is read from it.

import type { Power, SpellDef } from "@/sim/catalog";
import { CELL, CRIT_ONE_IN, GCD_TICKS, PARTY_DEALT, PARTY_TAKEN, PX_PER_METRE } from "@/sim/constants";
import { firstBlocked, lineOfSight } from "@/sim/los";
import { irandom } from "@/sim/rng";
import { asPlayer, vacate, type World } from "@/sim/runtime";
import { FACING_DX, FACING_DY, type Hit, type Projectile, type Unit } from "@/sim/state";
import {
  applyEffect,
  clearStatuses,
  defenceMods,
  isStunned,
  offenceMods,
  resistFactor,
} from "@/sim/status";
import { distance, facePoint, faceVector, isEnemy, maxHp, maxMp, restoreEnergy, setAnim } from "@/sim/units";
import { schoolTouch, worldVerb } from "@/sim/interact";
import { bodyDistance } from "@/sim/snake";
import { normalize, rotate } from "@/sim/angles";
import { onUnitKilled } from "@/sim/quests";
import { runActions } from "@/sim/actions";
import { rollLoot } from "@/sim/loot";

export type SpellError =
  | "castSuccessful"
  | "castUnsuccessful"
  | "youAreDead"
  | "onCooldown"
  | "onGCD"
  | "tooFar"
  | "noTarget"
  | "notEnoughMP"
  | "notEnoughEnergy"
  | "notInLOS"
  | "notValidTarget";

/** Toast text per error. GCD and cooldown stay quiet, as `PlayerCastSpell` did. */
export const SPELL_ERROR_TEXT: Record<SpellError, string | null> = {
  castSuccessful: null,
  castUnsuccessful: null,
  youAreDead: "I can't do that while dead",
  onCooldown: null,
  onGCD: null,
  tooFar: "Too far",
  noTarget: "I need a target",
  notEnoughMP: "Not enough mana",
  notEnoughEnergy: "Not enough energy",
  notInLOS: "Not in line of sight",
  notValidTarget: "I can't cast at that",
};

/**
 * `GetDistanceBetweenBounds`: centre distance in metres minus both bodies.
 * Range 0 therefore means "touching", which is how every 2020 melee row is written.
 */
export function metresBetween(w: World, a: Unit, b: Unit): number {
  const bounds = w.catalog.units[a.def].bounds + w.catalog.units[b.def].bounds;
  return Math.max(0, distance(a.x, a.y, b.x, b.y) / PX_PER_METRE - bounds);
}

export type Aim = { x: number; y: number };

export function rollPower(w: World, caster: Unit, power: Power): number {
  const stat = power.stat === "strength" ? caster.strength : caster.spirit;
  return stat / power.div + irandom(w.state.rng, stat / power.varDiv) + (power.flat ?? 0);
}

function rollCrit(w: World, oneIn: number): boolean {
  return irandom(w.state.rng, oneIn - 1) === 0;
}

/**
 * `aim` is a unit vector from the player's cursor / right stick. 2020's last
 * build had already dropped click-targeting for the player (every assignment to
 * the player's current_target is commented out; bolts flew at the mouse). AI
 * units have no aim and cast at `target`.
 *
 * Check order is 2020's: dead, target, LOS, range, THEN cooldown, GCD, MP, energy.
 * Range before cooldown matters: an AI that hears "onCooldown" stands still, an
 * AI that hears "tooFar" keeps walking.
 */
export function tryCast(w: World, caster: Unit, spellId: string, aim: Aim | null = null, on?: number): SpellError {
  const spell = w.catalog.spells[spellId];
  if (!spell) return "castUnsuccessful";
  if (!caster.alive) return "youAreDead";
  if (isStunned(w, caster)) return "castUnsuccessful";

  let target: Unit | null = caster.target ? (w.rt.units.get(caster.target) ?? null) : null;
  if (target && !target.alive) target = null;
  if (spell.needsTarget) {
    if (!target) return "noTarget";
    if (spell.needsEnemy !== isEnemy(caster, target)) return "notValidTarget";
    if (spell.needsLos && !lineOfSight(w.rt.grid, caster.x, caster.y, target.x, target.y)) return "notInLOS";
    if (metresBetween(w, caster, target) > spell.range) return "tooFar";
  } else if (target && spell.needsEnemy && !isEnemy(caster, target)) {
    target = null;
  }
  // Friendly spells choose who before they check what it costs, like every other target check.
  let friend: Unit | null = null;
  if (spell.kind === "ally") {
    const chosen = chooseFriend(w, caster, spell, aim, target, on);
    if (typeof chosen === "string") return chosen;
    friend = chosen;
  }
  if ((caster.cooldowns[spellId] ?? 0) > 0) return "onCooldown";
  if (caster.gcd > 0 && !spell.gcdImmune) return "onGCD";
  if (caster.mp < spell.mp) return "notEnoughMP";
  if (caster.energy < spell.energy) return "notEnoughEnergy";
  if (aim) {
    faceVector(caster, aim.x, aim.y);
    target = null;
  }

  let valid = true;
  switch (spell.kind) {
    case "melee":
      castMelee(w, caster, spellId, spell, target);
      break;
    case "bolt":
      castBolt(w, caster, spellId, spell, target, aim);
      break;
    case "self":
      if (spell.effect) applyEffect(w, caster, spell.effect, caster.id);
      break;
    case "ally": {
      const to = friend ?? caster;
      if (spell.power) to.incoming.push(makeHit(w, caster, spell));
      if (spell.effect) applyEffect(w, to, spell.effect, caster.id);
      break;
    }
    case "ground":
      castGround(w, caster, spellId, spell, target);
      break;
    case "world":
      valid = worldVerb(w, caster, spell.world ?? "repair");
      break;
  }
  if (!valid) return "castUnsuccessful";

  caster.mp -= spell.mp;
  caster.energy -= spell.energy;
  if (spell.cooldown > 0) caster.cooldowns[spellId] = spell.cooldown;
  if (!spell.gcdImmune) caster.gcd = GCD_TICKS;
  caster.stop = Math.max(caster.stop, spell.stop);
  setAnim(caster, spell.anim);
  if (target) facePoint(caster, target.x, target.y);
  const castBy = w.party.ofUnit(caster.id);
  if (castBy) castBy.stats.casts++;
  w.emit({ e: "cast", unit: caster.id, spell: spellId, x: caster.x, y: caster.y });
  return "castSuccessful";
}

/** How far off the aim line a friend may stand and still be the one you meant, px. Generous: she is moving. */
const ALLY_AIM_SLACK = 16;

/**
 * Who a friendly spell lands on. One button, no target frame, no modifier key:
 *
 *   mouse    `on` is the unit under the cursor when the button was pressed (0 = nobody).
 *            Over a friend: her. She must be in range and in sight, and if she is not the
 *            cast FAILS and says why; it does not quietly fall on the caster, because the
 *            player pointed at someone and meant it. Over yourself, over an enemy, over
 *            grass: yourself.
 *   pad      no cursor, so `on` is absent: the friend nearest the right stick's line,
 *            or yourself with the stick at rest or nobody that way.
 *   AI       its (friendly) target, else itself.
 *
 * Null means the caster. It never fails for want of a friend, so it is the same spell alone.
 */
function chooseFriend(w: World, caster: Unit, spell: SpellDef, aim: Aim | null, target: Unit | null, on: number | undefined): Unit | null | SpellError {
  if (on !== undefined) {
    if (on === 0 || on === caster.id) return null;
    const u = w.rt.units.get(on);
    if (!u || !u.alive || u.hidden || !w.party.ofUnit(u.id)) return null;
    if (spell.needsLos && !lineOfSight(w.rt.grid, caster.x, caster.y, u.x, u.y)) return "notInLOS";
    if (metresBetween(w, caster, u) > spell.range) return "tooFar";
    return u;
  }
  if (aim) return friendAlong(w, caster, spell, aim);
  return target && !isEnemy(caster, target) ? target : null;
}

/** The living party member closest to the aim ray, in range and (if the spell asks) in sight. */
function friendAlong(w: World, caster: Unit, spell: SpellDef, aim: Aim): Unit | null {
  let best: Unit | null = null;
  let bestOff = ALLY_AIM_SLACK + 1;
  for (const u of w.party.units()) {
    if (u.id === caster.id || !u.alive || u.hidden || w.rt.units.get(u.id) !== u) continue;
    const dx = u.x - caster.x;
    const dy = u.y - caster.y;
    const along = dx * aim.x + dy * aim.y;
    if (along <= 0 || metresBetween(w, caster, u) > spell.range) continue;
    const off = Math.abs(dx * aim.y - dy * aim.x);
    if (off >= bestOff) continue;
    if (spell.needsLos && !lineOfSight(w.rt.grid, caster.x, caster.y, u.x, u.y)) continue;
    best = u;
    bestOff = off;
  }
  return best;
}

function makeHit(w: World, caster: Unit, spell: SpellDef): Hit {
  const mods = offenceMods(w, caster);
  let amount = spell.power ? rollPower(w, caster, spell.power) : 0;
  const crit = rollCrit(w, mods.critOneIn > 0 ? mods.critOneIn : CRIT_ONE_IN);
  if (crit) amount *= 2;
  return { amount: Math.round(amount), school: spell.school, from: caster.id, crit, status: spell.effect };
}

/**
 * Melee: the locked target if it is in reach; else the nearest enemy in reach,
 * preferring the facing half-plane. Melee is forgiving on purpose: a parked cursor
 * or a centred stick should not make thirty swings whiff at a skeleton chewing on
 * your back, so anything behind still counts, it just loses ties to what you face.
 * A swing that finds nobody is still a swing.
 * (2020 wrote the search result onto the caster and could DamageHP(noone).)
 */
function castMelee(w: World, caster: Unit, spellId: string, spell: SpellDef, locked: Unit | null): void {
  let victim: Unit | null = null;
  if (locked && isEnemy(caster, locked) && metresBetween(w, caster, locked) <= spell.range) victim = locked;
  if (!victim) {
    let best = Infinity;
    const fx = FACING_DX[caster.facing];
    const fy = FACING_DY[caster.facing];
    for (const u of w.zone.units) {
      if (!u.alive || !u.awake || u.hidden || u === caster || !isEnemy(caster, u) || u.controller === "npc") continue;
      const d = metresBetween(w, caster, u);
      if (d > spell.range) continue;
      const behind = (u.x - caster.x) * fx + (u.y - caster.y) * fy < -CELL / 2;
      const score = d + (behind ? 1000 : 0);
      if (score >= best) continue;
      if (!lineOfSight(w.rt.grid, caster.x, caster.y, u.x, u.y)) continue;
      best = score;
      victim = u;
    }
  }
  w.emit({ e: "swing", unit: caster.id, x: caster.x, y: caster.y, facing: caster.facing });
  if (!victim) return;
  facePoint(caster, victim.x, victim.y);
  const hit = makeHit(w, caster, spell);
  victim.incoming.push(hit);
  for (const extra of offenceMods(w, caster).onMelee) {
    if (!extra) continue;
    victim.incoming.push({ amount: extra.amount, school: extra.school, from: caster.id, crit: false, status: extra.effect });
  }
  if (spell.restoreEnergy) restoreEnergy(caster, spell.restoreEnergy * (hit.crit ? 3 : 1));
  w.emit({ e: "impact", spell: spellId, school: spell.school, x: victim.x, y: victim.y });
}

function castBolt(
  w: World,
  caster: Unit,
  spellId: string,
  spell: SpellDef,
  target: Unit | null,
  aim: Aim | null,
): void {
  let dx = FACING_DX[caster.facing];
  let dy = FACING_DY[caster.facing];
  if (aim) {
    dx = aim.x;
    dy = aim.y;
  } else if (target) {
    const n = normalize(target.x - caster.x, target.y - caster.y);
    if (n.x !== 0 || n.y !== 0) {
      dx = n.x;
      dy = n.y;
    }
  }
  const speed = spell.speed ?? 2;
  const count = Math.max(1, spell.count ?? 1);
  const fan = spell.fan ?? 0;
  for (let i = 0; i < count; i++) {
    let vx = dx;
    let vy = dy;
    if (count > 1 || fan > 0) {
      // Ring: evenly spaced, first bolt on the aim line. Fan: random inside the arc (2020's cactus).
      const offset = fan >= 360 ? (i * 360) / count : irandom(w.state.rng, fan) - fan / 2;
      const r = rotate(dx, dy, offset);
      vx = r.x;
      vy = r.y;
    }
    const p: Projectile = {
      id: w.state.nextId++,
      spell: spellId,
      from: caster.id,
      target: 0,
      x: caster.x + vx * 4,
      y: caster.y + vy * 4,
      vx: vx * speed,
      vy: vy * speed,
      left: (spell.range + w.catalog.units[caster.def].bounds * 2) * PX_PER_METRE,
      hit: makeHit(w, caster, spell),
      age: 0,
    };
    w.zone.projectiles.push(p);
  }
}

function castGround(w: World, caster: Unit, spellId: string, spell: SpellDef, target: Unit | null): void {
  const at = target ?? caster;
  w.zone.grounds.push({
    id: w.state.nextId++,
    spell: spellId,
    from: caster.id,
    faction: caster.faction,
    x: at.x,
    y: at.y,
    radius: (spell.radius ?? 1) * PX_PER_METRE,
    left: spell.duration ?? 60,
    nextTick: 1,
  });
}

/**
 * Bolts fly straight. 2020 aimed once and never homed, which is what makes a
 * slow poison bolt dodgeable. They die on the first sight-blocking cell, the
 * first enemy body, or when their range runs out.
 */
export function stepProjectiles(w: World): void {
  const list = w.zone.projectiles;
  for (let i = list.length - 1; i >= 0; i--) {
    const p = list[i];
    const speed = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
    const nx = p.x + p.vx;
    const ny = p.y + p.vy;
    let dead = firstBlocked(w.rt.grid, p.x, p.y, nx, ny) !== -1;
    p.x = nx;
    p.y = ny;
    p.left -= speed;
    p.age++;
    const caster = w.rt.units.get(p.from);
    let victim: Unit | null = null;
    if (!dead) {
      for (const u of w.zone.units) {
        if (!u.alive || !u.awake || u.hidden || u.id === p.from || u.controller === "npc") continue;
        if (caster ? !isEnemy(caster, u) : false) continue;
        if (bodyDistance(u, p.x, p.y) > w.catalog.units[u.def].bounds * PX_PER_METRE) continue;
        victim = u;
        break;
      }
    }
    if (victim) {
      victim.incoming.push(p.hit);
      dead = true;
    }
    if (dead || p.left <= 0) {
      const spell = w.catalog.spells[p.spell];
      if (spell.splash && p.hit.amount > 0) {
        const amount = Math.round(p.hit.amount / spell.splash.div);
        for (const u of w.zone.units) {
          if (u === victim || !u.alive || !u.awake || u.hidden || u.id === p.from || u.controller === "npc") continue;
          if (caster ? !isEnemy(caster, u) : false) continue;
          if (bodyDistance(u, p.x, p.y) > spell.splash.radius) continue;
          if (amount > 0) u.incoming.push({ amount, school: p.hit.school, from: p.from, crit: false });
        }
      }
      schoolTouch(w, spell.school, p.x, p.y, p.from, spell.touch);
      w.emit({ e: "impact", spell: p.spell, school: spell.school, x: p.x, y: p.y });
      list.splice(i, 1);
    }
  }
}

/** Ground kind: a lingering area that pulses its effect/damage on enemies inside it. */
export function stepGrounds(w: World): void {
  const list = w.zone.grounds;
  for (let i = list.length - 1; i >= 0; i--) {
    const g = list[i];
    const spell = w.catalog.spells[g.spell];
    if (--g.nextTick <= 0) {
      g.nextTick = spell.pulse ?? 30;
      const caster = w.rt.units.get(g.from);
      for (const u of w.zone.units) {
        if (!u.alive || !u.awake || u.hidden || u.faction === g.faction || u.controller === "npc") continue;
        if (distance(g.x, g.y, u.x, u.y) > g.radius) continue;
        const amount = spell.power && caster ? Math.round(rollPower(w, caster, spell.power)) : 0;
        if (amount > 0 || spell.effect) {
          u.incoming.push({ amount, school: spell.school, from: g.from, crit: false, status: spell.effect });
        }
      }
    }
    if (--g.left <= 0) list.splice(i, 1);
  }
}

/**
 * The only place hp changes. Runs after every controller, spell and status has
 * had its turn this tick, so same-tick ordering can never decide who dies.
 */
export function flushIncoming(w: World, u: Unit): void {
  if (u.incoming.length === 0) return;
  const hits = u.incoming;
  u.incoming = [];
  if (!u.alive) return;
  const owner = w.party.ofUnit(u.id) ?? null;
  // The co-op penalty: set by how many are connected, not by who is standing here.
  const seats = Math.min(PARTY_DEALT.length, Math.max(1, w.party.size())) - 1;
  for (const hit of hits) {
    if (hit.school === "heal") {
      const before = u.hp;
      u.hp = Math.min(maxHp(u), u.hp + hit.amount);
      if (u.hp > before) w.emit({ e: "heal", unit: u.id, from: hit.from, x: u.x, y: u.y, amount: Math.round(u.hp - before) });
      continue;
    }
    let amount = hit.amount * resistFactor(w, u, hit.school);
    if (owner) amount *= PARTY_TAKEN[seats];
    else if (hit.from && w.party.ofUnit(hit.from)) amount *= PARTY_DEALT[seats];
    if (owner?.god) amount = 0;
    const def = defenceMods(w, u);
    let absorbed = 0;
    if (def.manaShield > 0 && amount > 0 && u.mp > 0) {
      absorbed = Math.min(amount, u.mp / def.manaShield);
      u.mp -= absorbed * def.manaShield;
      amount -= absorbed;
    }
    amount = Math.round(amount);
    u.hp = Math.max(0, u.hp - amount);
    if (def.manaOnHit > 0) u.mp = Math.min(maxMp(u), u.mp + def.manaOnHit);
    w.emit({
      e: "damage",
      unit: u.id,
      from: hit.from,
      x: u.x,
      y: u.y,
      amount,
      school: hit.school,
      crit: hit.crit,
      absorbed: Math.round(absorbed),
    });
    // Only the one who was hit feels it.
    if (owner && amount > 0) asPlayer(w, owner, () => w.emit({ e: "shake", amount: Math.min(4, 1 + amount / 40) }));
    const source = hit.from ? w.rt.units.get(hit.from) : undefined;
    if (source && source.alive && amount > 0) {
      const steal = offenceMods(w, source).lifesteal;
      if (steal > 0) source.incoming.push({ amount: Math.round(amount * steal), school: "heal", from: source.id, crit: false });
    }
    // First hit pulls aggro, even from outside the aggro radius.
    if (source && source.alive && u.controller !== "player" && u.controller !== "npc" && u.combat !== "combat") {
      if (isEnemy(u, source)) {
        u.target = source.id;
        u.combat = "combat";
        u.awake = true;
      }
    }
    if (hit.status && u.hp > 0) applyEffect(w, u, hit.status, hit.from);
    if (u.hp > 0 && amount > 0 && u.anim === "idle") setAnim(u, "hurt");
    if (u.hp > 0 && amount > 0 && u.controller === "ai") enterPhases(w, u, source ?? null);
    if (u.hp <= 0) {
      killUnit(w, u, source ?? null);
      break;
    }
  }
}

/**
 * A boss crosses into its next phase when its health falls to that row's `hpBelow` of full
 * (a row with `hpBelow: 1` is entered by the first blow that hurts): it takes that
 * phase's book and speed, and the phase's `onEnter` list runs once, with the boss as the
 * subject, on behalf of whoever landed the blow (nobody, if poison did). Health thresholds
 * only, and only for the ordinary AI: the snake's phases are its own clock (sim/snake.ts).
 * One blow may cross two thresholds; both lists run, in order.
 */
function enterPhases(w: World, u: Unit, source: Unit | null): void {
  const phases = w.catalog.units[u.def].phases;
  if (!phases) return;
  const fraction = u.hp / maxHp(u);
  // `phase` counts the rows entered so far: 0 is the unit as its own row describes it.
  while (u.phase < phases.length && fraction <= phases[u.phase].hpBelow) {
    const row = phases[u.phase];
    u.phase++;
    u.phaseTick = 0;
    u.book = [...row.book];
    if (row.onEnter && row.onEnter.length > 0) {
      const list = row.onEnter;
      const by = source ? (w.party.ofUnit(source.id) ?? null) : null;
      asPlayer(w, by, () => runActions(w, list, u.id));
    }
  }
}

/**
 * All its health back (it leashed and mended, or it respawned): the fight starts again from
 * the top, and the phase lists will run again when it is brought down again.
 */
export function resetPhases(w: World, u: Unit): void {
  if (u.phase === 0) return;
  const def = w.catalog.units[u.def];
  if (!def.phases || u.controller !== "ai") return;
  u.phase = 0;
  u.phaseTick = 0;
  u.book = [...def.book];
}

export function killUnit(w: World, u: Unit, killer: Unit | null): void {
  u.alive = false;
  u.order = null;
  u.dwell = 0;
  u.hp = 0;
  u.mp = 0;
  u.energy = 0;
  u.target = 0;
  u.path = null;
  u.combat = "idle";
  u.deadFor = 0;
  u.incoming.length = 0;
  clearStatuses(w, u);
  setAnim(u, "dead");
  vacate(w.rt, u);
  w.emit({ e: "death", unit: u.id, def: u.def, x: u.x, y: u.y });
  for (const other of w.zone.units) if (other.target === u.id) other.target = 0;
  const owner = w.party.ofUnit(u.id);
  if (owner) {
    owner.stats.deaths++;
    owner.dialogue = null;
    asPlayer(w, owner, () => w.emit({ e: "playerDied" }));
    return;
  }
  // A kill by anyone in the party counts for the party's quests.
  const slayer = killer ? (w.party.ofUnit(killer.id) ?? null) : null;
  if (slayer) {
    slayer.stats.kills++;
    onUnitKilled(w, u);
  }
  rollLoot(w, u);
  const def = w.catalog.units[u.def];
  if (def.onDeath) asPlayer(w, slayer, () => runActions(w, def.onDeath ?? [], u.id));
}
