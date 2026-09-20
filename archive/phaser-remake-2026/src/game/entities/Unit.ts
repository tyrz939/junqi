import { ENERGY_MAX, INVENTORY_SIZE } from "@/game/constants";
import { mitigate, tickStatuses, type StatusInst } from "@/game/systems/status";
import type {
  CombatState,
  Controller,
  CreatureClass,
  CreatureState,
  DamageType,
  Faction,
  IncomingHit,
} from "@/game/types";

export type UnitInit = {
  id: string;
  name: string;
  faction: Faction;
  class: CreatureClass;
  controller: Controller;
  strength: number;
  spirit: number;
  walkSpd: number;
  runSpd: number;
  spells: string[];
};

export class Unit {
  readonly id: string;
  name: string;
  faction: Faction;
  class: CreatureClass;
  controller: Controller;
  strength: number;
  spirit: number;
  walkSpd: number;
  runSpd: number;
  maxhp = 1;
  hp = 1;
  maxmp = 1;
  mp = 1;
  maxenergy = ENERGY_MAX;
  energy = ENERGY_MAX;
  energyEmpty = false;
  alive = true;
  gcd = 0;
  stopTimer = 0;
  combatState: CombatState = "idle";
  creatureState: CreatureState = "idle";
  facing = 270;
  currentTargetId: string | null = null;
  spellbook: string[];
  cooldowns: Record<string, number> = {};
  incoming: IncomingHit[] = [];
  inventorySize = INVENTORY_SIZE;
  inventory: { id: string | null; qty: number }[];
  itemCooldowns: { id: string; t: number }[] = [];
  aggroRange = 10;
  leashRange = 24;
  enemyKind = "";
  flags: Record<string, number> = {};
  statuses: StatusInst[] = [];
  resist: Partial<Record<DamageType, number>> = {};
  dropped = false;
  respawn = 0;
  deadFor = 0;
  questUnit = false;
  hunger = 80;
  warmth = 70;

  constructor(init: UnitInit) {
    this.id = init.id;
    this.name = init.name;
    this.faction = init.faction;
    this.class = init.class;
    this.controller = init.controller;
    this.strength = init.strength;
    this.spirit = init.spirit;
    this.walkSpd = init.walkSpd;
    this.runSpd = init.runSpd;
    this.spellbook = [...init.spells];
    this.inventory = Array.from({ length: this.inventorySize }, () => ({
      id: null,
      qty: 0,
    }));
    this.recalc();
    this.hp = this.maxhp;
    this.mp = this.maxmp;
  }

  recalc(): void {
    this.maxhp = this.strength * 5;
    this.maxmp = this.spirit * 5;
  }

  isEnemy(other: Unit): boolean {
    return other.faction !== this.faction;
  }

  enqueueDamage(amount: number, fromId: string | null, type: DamageType): void {
    this.incoming.push({ amount, fromId, type });
  }

  flushIncoming(): IncomingHit[] {
    const hits = this.incoming;
    this.incoming = [];
    const applied: IncomingHit[] = [];
    for (const hit of hits) {
      if (hit.type === "heal") {
        this.hp = Math.min(this.maxhp, this.hp + Math.abs(hit.amount));
        applied.push(hit);
      } else {
        let amount = mitigate(this, hit.amount, hit.type);
        const shield = this.statuses.find((s) => s.id === "manashield");
        if (shield && this.mp > 0) {
          const soak = Math.min(this.mp, amount);
          this.mp -= soak;
          amount -= soak;
        }
        this.hp = Math.max(0, this.hp - amount);
        applied.push({ ...hit, amount });
      }
      if (this.hp <= 0) {
        this.alive = false;
        this.creatureState = "dead";
        this.mp = 0;
        this.energy = 0;
        break;
      }
    }
    return applied;
  }

  tickTimers(dt: number): void {
    if (this.gcd > 0) this.gcd = Math.max(0, this.gcd - dt);
    if (this.stopTimer > 0) this.stopTimer = Math.max(0, this.stopTimer - dt);
    for (const id of Object.keys(this.cooldowns)) {
      this.cooldowns[id] = Math.max(0, this.cooldowns[id] - dt);
    }
    if (this.alive && this.mp < this.maxmp) {
      this.mp = Math.min(this.maxmp, this.mp + this.spirit * 0.06 * dt);
    }
    this.itemCooldowns = this.itemCooldowns
      .map((c) => ({ ...c, t: c.t - dt }))
      .filter((c) => c.t > 0);
    if (this.alive) {
      for (const dot of tickStatuses(this, dt)) this.incoming.push(dot);
    }
    if (!this.alive && this.respawn > 0 && !this.questUnit) this.deadFor += dt;
  }

  snapshot(): Record<string, unknown> {
    return {
      hp: this.hp,
      mp: this.mp,
      energy: this.energy,
      energyEmpty: this.energyEmpty,
      strength: this.strength,
      spirit: this.spirit,
      spellbook: this.spellbook,
      inventory: this.inventory,
      gcd: this.gcd,
      facing: this.facing,
      alive: this.alive,
      statuses: this.statuses,
      hunger: this.hunger,
      warmth: this.warmth,
    };
  }

  restore(data: Record<string, unknown>): void {
    this.hp = Number(data.hp ?? this.hp);
    this.mp = Number(data.mp ?? this.mp);
    this.energy = Number(data.energy ?? this.energy);
    this.energyEmpty = Boolean(data.energyEmpty);
    this.strength = Number(data.strength ?? this.strength);
    this.spirit = Number(data.spirit ?? this.spirit);
    this.recalc();
    if (Array.isArray(data.spellbook)) this.spellbook = data.spellbook as string[];
    if (Array.isArray(data.inventory)) {
      this.inventory = data.inventory as { id: string | null; qty: number }[];
    }
    this.gcd = Number(data.gcd ?? 0);
    this.facing = Number(data.facing ?? this.facing);
    this.alive = data.alive !== false;
    if (Array.isArray(data.statuses)) this.statuses = data.statuses as StatusInst[];
    if (data.hunger != null) this.hunger = Number(data.hunger);
    if (data.warmth != null) this.warmth = Number(data.warmth);
  }

  revive(x = 0, y = 0): { x: number; y: number } {
    this.alive = true;
    this.creatureState = "idle";
    this.combatState = "idle";
    this.hp = this.maxhp;
    this.mp = this.maxmp;
    this.energy = this.maxenergy;
    this.energyEmpty = false;
    this.deadFor = 0;
    this.incoming = [];
    return { x, y };
  }

  spendEnergy(amount: number): void {
    this.energy = Math.max(0, this.energy - amount);
    if (this.energy === 0) this.energyEmpty = true;
  }

  regenEnergy(amount: number): void {
    this.energy = Math.min(this.maxenergy, this.energy + amount);
    if (this.energy === this.maxenergy) this.energyEmpty = false;
  }
}
