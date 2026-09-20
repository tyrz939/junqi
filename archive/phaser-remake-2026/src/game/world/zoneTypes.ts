import type { TriggerSpec } from "@/game/systems/triggers";
import type { ZoneId } from "@/game/types";
import type { Grid } from "@/game/world/Grid";

export type PropSpec = {
  id: string;
  kind:
    | "dog"
    | "door"
    | "chest"
    | "bench"
    | "push"
    | "broken"
    | "hatch"
    | "orb"
    | "shrine"
    | "torch"
    | "drop"
    | "trigger"
    | "sign"
    | "npc"
    | "toggle"
    | "campfire";
  x: number;
  y: number;
  texture: string;
  locked?: boolean;
  opens?: string;
  requires?: string;
  lockHint?: string;
  laterHint?: string;
  loot?: { id: string; qty: number }[];
  toZone?: ZoneId;
  learn?: string;
  talk?: string;
  actions?: string[];
};

export type EnemySpec = {
  id: string;
  kind: string;
  x: number;
  y: number;
  patrol?: { x: number; y: number }[];
  respawn?: number;
  tag?: string;
};

export type FloodSpec = {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
};

export type ZoneBlue = {
  grid: Grid;
  spawn: { x: number; y: number };
  props: PropSpec[];
  enemies: EnemySpec[];
  lights: { x: number; y: number; radius: number }[];
  triggers: TriggerSpec[];
  required: string[];
  floods?: FloodSpec[];
};
