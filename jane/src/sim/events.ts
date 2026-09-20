// What the sim tells the outside. Presentation (floating text, shake, particles,
// sound, toasts, UI refresh) listens; it never reaches into the sim to find out
// what happened. Events are not state: they are drained every frame and never saved.

import type { School, ZoneId } from "@/sim/state";
import type { SpellError } from "@/sim/combat";

/**
 * Routing, stamped by the sim as the event is emitted. With up to four people in up
 * to four zones, a client shows an event only if it is hers or happened where she is:
 *   `to`      the seat it belongs to (a toast, her screen shake). Absent = not personal.
 *   `inZone`  where it happened. Absent = the whole party (quest news, the bell).
 */
export type EventRoute = { to?: number; inZone?: ZoneId };

export type SimEvent = EventRoute & SimEventBody;

type SimEventBody =
  | { e: "toast"; text: string }
  /** `from` is the unit that did it, 0 for the world. A client shows the number only if she dealt it or took it. */
  | { e: "damage"; unit: number; from: number; x: number; y: number; amount: number; school: School; crit: boolean; absorbed: number }
  | { e: "heal"; unit: number; from: number; x: number; y: number; amount: number }
  | { e: "death"; unit: number; def: string; x: number; y: number }
  | { e: "respawn"; unit: number }
  | { e: "cast"; unit: number; spell: string; x: number; y: number }
  | { e: "castFailed"; unit: number; spell: string; error: SpellError }
  | { e: "impact"; spell: string; school: School; x: number; y: number }
  | { e: "swing"; unit: number; x: number; y: number; facing: number }
  | { e: "status"; unit: number; effect: string; on: boolean }
  | { e: "loot"; item: string; qty: number }
  | { e: "learn"; spell: string }
  | { e: "quest"; quest: string; change: "given" | "progress" | "ready" | "done" }
  | { e: "zone"; zone: ZoneId; name: string; first: boolean }
  | { e: "shake"; amount: number }
  | { e: "camera"; mode: "follow" | "lock"; rect: string }
  | { e: "tiles"; cx: number; cy: number; w: number; h: number }
  | { e: "prop"; prop: number; change: "open" | "unlock" | "lock" | "use" | "push" | "show" | "hide" | "switch" }
  | { e: "bag" }
  | { e: "dialogue" }
  | { e: "playerDied" }
  | { e: "rest" }
  | { e: "sfx"; name: string; x: number; y: number };
