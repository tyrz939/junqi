import { PATH_CELL } from "@/game/constants";
import type { Unit } from "@/game/entities/Unit";
import { hasStatus } from "@/game/systems/status";
import { biomeAt, biomeCold } from "@/game/world/biomes";
import type { ZoneId } from "@/game/types";

const HUNGER_PER_S = 0.12;
const WARM_SEEK = 8;

export function tickSurvive(
  player: Unit,
  dt: number,
  time: number,
  zone: ZoneId,
  x: number,
  y: number,
  nearFire: boolean,
): string | null {
  if (!player.alive) return null;
  const cx = Math.floor(x / PATH_CELL);
  const cy = Math.floor(y / PATH_CELL);
  const biome = zone === "county" ? biomeAt(cx, cy) : "yard";
  const cold = biomeCold(biome, time, zone) - (nearFire ? 40 : 0);
  const want = Math.max(8, 72 - cold);

  if (nearFire) player.warmth = Math.min(100, player.warmth + 22 * dt);
  else if (player.warmth > want) player.warmth = Math.max(want, player.warmth - WARM_SEEK * dt);
  else player.warmth = Math.min(want, player.warmth + 3 * dt);

  let drain = HUNGER_PER_S;
  if (player.warmth < 28) drain *= 1.6;
  if (time < 6 || time >= 21) drain *= 1.15;
  if (hasStatus(player, "well_fed")) drain *= 0.55;
  player.hunger = Math.max(0, player.hunger - drain * dt);

  starveCd = Math.max(0, starveCd - dt);
  coldCd = Math.max(0, coldCd - dt);
  if (player.hunger <= 0) {
    player.enqueueDamage(5 * dt, null, "physical");
    if (starveCd <= 0) {
      starveCd = 10;
      return "You're empty. Eat.";
    }
  }
  if (player.warmth < 22) {
    player.enqueueDamage(3 * dt, null, "frost");
    if (coldCd <= 0) {
      coldCd = 10;
      return "The county is taking heat. Fire, stone, or a roof.";
    }
  }
  return null;
}

let starveCd = 0;
let coldCd = 0;

export function resetSurvive(): void {
  starveCd = 0;
  coldCd = 0;
}

export function surviveSpeed(player: Unit): number {
  let m = 1;
  if (player.hunger < 18) m *= 0.72;
  if (player.warmth < 22) m *= 0.8;
  return m;
}


