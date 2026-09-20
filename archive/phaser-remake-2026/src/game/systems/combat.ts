import { GCD_SECONDS, PATH_CELL } from "@/game/constants";
import { getSpell } from "@/game/systems/catalog";
import type { Unit } from "@/game/entities/Unit";
import type { DamageType, SpellError } from "@/game/types";

export function metres(pixels: number): number {
  return pixels / PATH_CELL;
}

export function tryCast(
  caster: Unit,
  spellId: string,
  target: Unit | null,
  distMetres: number,
  inLos = true,
): SpellError {
  const spell = getSpell(spellId);
  if (!caster.alive) return "youAreDead";
  if ((caster.cooldowns[spellId] ?? 0) > 0) return "onCooldown";
  if (caster.gcd > 0 && !spell.gcdImmune) return "onGCD";
  if (caster.mp < spell.mpCost) return "notEnoughMP";
  if (caster.energy < spell.energyCost) return "notEnoughEnergy";

  if (spell.requiresTarget || caster.controller === "ai") {
    if (!target) return "noTarget";
    if (!target.alive) return "notValidTarget";
    if (spell.reqEnemyAsTarget && !caster.isEnemy(target)) return "notValidTarget";
    if (!spell.reqEnemyAsTarget && caster.isEnemy(target)) return "notValidTarget";
    if (spell.reqLos && !inLos) return "notInLOS";
    const reach = spell.range > 0 ? spell.range : spell.castAni === "attacking" ? 1.2 : 0;
    if (distMetres > reach) return "tooFar";
  }

  caster.mp -= spell.mpCost;
  caster.energy -= spell.energyCost;
  caster.cooldowns[spellId] = spell.cooldown;
  if (!spell.gcdImmune) caster.gcd = GCD_SECONDS;
  caster.stopTimer = 0.35;
  caster.creatureState = spell.castAni;
  return "castSuccessful";
}

export function meleeDamage(strength: number, critBonus = false): { amount: number; crit: boolean } {
  let amount = strength / 8 + Math.floor(Math.random() * (strength / 32));
  const crit = Math.floor(Math.random() * 20) === 0 || critBonus;
  if (crit) amount *= 2;
  return { amount: Math.round(amount), crit };
}

export function schoolDamage(
  spirit: number,
  school: DamageType,
  critBonus = false,
): { amount: number; crit: boolean; type: DamageType } {
  let amount = spirit / 10 + Math.floor(Math.random() * (spirit / 40));
  if (school === "fire") amount *= 1.1;
  let crit = Math.floor(Math.random() * 20) === 0 || critBonus;
  if (crit) amount *= 2;
  return { amount: Math.round(amount), crit, type: school };
}

export function spellErrorText(error: SpellError): string | null {
  switch (error) {
    case "noTarget":
      return "You need a target";
    case "notEnoughEnergy":
      return "Not enough energy";
    case "notEnoughMP":
      return "Not enough mana";
    case "notInLOS":
      return "Not in line of sight";
    case "notValidTarget":
      return "I can't cast at that";
    case "tooFar":
      return "Too far";
    case "youAreDead":
      return "I can't do that while dead";
    case "onCooldown":
    case "onGCD":
      return null;
    default:
      return null;
  }
}
