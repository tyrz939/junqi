import { getSpell } from "@/game/systems/catalog";
import type { DamageType } from "@/game/types";

export type SpellKind = "melee" | "bolt" | "self";

export type SpellVfx = {
  kind: SpellKind;
  school: DamageType;
  color: number;
  speed: number;
  radius: number;
  cone: number;
  texture: string;
};

const SCHOOL: Record<string, { color: number; texture: string; school: DamageType }> = {
  frost: { color: 0x9ad4ff, texture: "bolt_frost", school: "frost" },
  fire: { color: 0xff7744, texture: "bolt_fire", school: "fire" },
  nature: { color: 0x88cc44, texture: "bolt_nature", school: "nature" },
  physical: { color: 0xe8d8b0, texture: "slash", school: "physical" },
  heal: { color: 0x88dd88, texture: "bolt_heal", school: "heal" },
};

const OVERRIDE: Partial<Record<string, Partial<SpellVfx>>> = {
  repair: { kind: "self", color: 0xc4a060, texture: "spark" },
  grow: { kind: "self", color: 0x66aa44, texture: "spark" },
  lesser_heal: { kind: "self", color: 0x88dd88, texture: "spark" },
  greater_heal: { kind: "self", color: 0x88dd88, texture: "spark" },
  flash_heal: { kind: "self", color: 0xa8f0a8, texture: "spark" },
  ward: { kind: "self", color: 0x9ad4ff, texture: "spark" },
  barrier: { kind: "self", color: 0xc4a878, texture: "spark" },
  haste: { kind: "self", color: 0x88cc44, texture: "spark" },
  melee_player: { kind: "melee", cone: 58, radius: 14 },
};

export function spellVfx(id: string): SpellVfx {
  const spell = getSpell(id);
  const schoolKey = spell.school ?? (spell.castAni === "attacking" ? "physical" : "frost");
  const look = SCHOOL[schoolKey] ?? SCHOOL.frost;
  const kind: SpellKind =
    spell.castAni === "attacking" ? "melee" : spell.range > 0 ? "bolt" : "self";
  return {
    kind,
    school: look.school,
    color: look.color,
    speed: 260,
    radius: kind === "melee" ? 12 : 7,
    cone: 55,
    texture: look.texture,
    ...OVERRIDE[id],
  };
}

export function facingDir(deg: number): "e" | "s" | "w" | "n" {
  const a = ((deg % 360) + 360) % 360;
  if (a >= 315 || a < 45) return "e";
  if (a < 135) return "s";
  if (a < 225) return "w";
  return "n";
}

export function faceTexture(base: string, deg: number): string {
  return `${base}_${facingDir(deg)}`;
}

export function angleBetween(ax: number, ay: number, bx: number, by: number): number {
  return (Math.atan2(by - ay, bx - ax) * 180) / Math.PI;
}

export function angleDelta(a: number, b: number): number {
  let d = ((a - b + 180) % 360) - 180;
  if (d < -180) d += 360;
  return Math.abs(d);
}
