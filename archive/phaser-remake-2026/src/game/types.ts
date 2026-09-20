export type Faction = "undead" | "beast" | "bandit" | "friendly";
export type CreatureClass = "warrior" | "mage";
export type Controller = "player" | "ai";
export type CombatState = "idle" | "leashing" | "combat";
export type CreatureState =
  | "attacking"
  | "casting"
  | "dead"
  | "hurt"
  | "idle"
  | "shooting"
  | "walking";
export type DamageType = "heal" | "physical" | "frost" | "fire" | "nature";

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

export type ItemDef = {
  name: string;
  description: string;
  usable: boolean;
  maxStack: number;
  cooldown: number;
  effect?: string;
  opens?: string;
  food?: number;
  warm?: number;
};

export type LootRoll = {
  id: string;
  qty: number;
  chance: number;
};

export type StatusKind = "flag" | "absorb" | "slow" | "root" | "dot" | "heal";

export type EffectDef = {
  kind: StatusKind;
  duration?: number;
  amount?: number;
  tick?: number;
  school?: DamageType;
};

export type SpellDef = {
  name: string;
  description: string;
  mpCost: number;
  energyCost: number;
  range: number;
  castAni: CreatureState;
  cooldown: number;
  gcdImmune: boolean;
  requiresTarget: boolean;
  reqEnemyAsTarget: boolean;
  reqLos: boolean;
  school?: DamageType;
  onHit?: string;
};

export type RecipeDef = {
  inputs: string[];
  output: string;
  qty: number;
};

export type BeatDef = {
  id: string;
  title: string;
  mustPlace: string[];
  unlock: string;
};

export type EnemyDef = {
  name: string;
  faction: Faction;
  class: CreatureClass;
  strength: number;
  spirit: number;
  walkSpd: number;
  runSpd: number;
  aggroRange: number;
  leashRange: number;
  spells: string[];
  questUnit?: boolean;
  respawn?: number;
  loot?: LootRoll[];
  resist?: Partial<Record<DamageType, number>>;
  scale?: number;
  onDeath?: string[];
  sprite?: string;
};

export type IncomingHit = {
  amount: number;
  fromId: string | null;
  type: DamageType;
};

export type QuestReqType = "kill" | "acquire" | "location";

export type QuestRequirement = {
  type: QuestReqType;
  target: string;
  qty: number;
  text: string;
};

export type QuestDef = {
  name: string;
  description: string;
  completion: string;
  requirements: QuestRequirement[];
  rewards?: { id: string; qty: number }[];
};

export type ActionBarSlot = {
  source: "item" | "spell";
  id: string;
} | null;

export type ZoneId =
  | "county"
  | "house"
  | "dungeon"
  | "burial"
  | "mine"
  | "abandoned"
  | "museum"
  | "graveyard"
  | "factory"
  | "school"
  | "butterfly"
  | "pipes";
export type GuiWindow = "inventory" | "spellbook" | "quest" | "map";
