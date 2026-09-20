// The seam between the app shell and the DOM UI. The UI never reaches past this:
// it reads sim state, sends commands, and asks the host for anything that needs
// storage, the renderer or the clock. The app never reaches into the UI's DOM.
//
// Why DOM and not canvas: text stays crisp at any scale, layout is CSS, pointer
// events hit the UI before the world for free (2020's `mouseOnGui` check), drag
// and drop is one pointer path for mouse and touch, and none of it costs a draw
// call in the world renderer.

import type { Catalog } from "@/sim/catalog";
import type { SimEvent } from "@/sim/events";
// The UI is built against ONE SEAT's view of the world, never the whole sim: the same
// code serves seat 0 alone and seat 3 of four on a LAN (PLATFORM.md).
import type { Command, PlayerView as Sim } from "@/sim/sim";

export type SlotInfo = {
  slot: number;
  savedAt: string;
  zone: string;
  day: number;
  hour: number;
  hp: number;
  maxhp: number;
};

/** Actions the input layer routes to the UI. Same names for keyboard and pad. */
export type UiAction =
  | "bags"
  | "book"
  | "quests"
  | "map"
  | "pause"
  | "terminal"
  | "debug"
  | "confirm"
  | "cancel"
  | "left"
  | "right"
  | "up"
  | "down"
  | "tabLeft"
  | "tabRight";

export interface UiHost {
  readonly catalog: Catalog;
  /** null while on the title screen. */
  sim(): Sim | null;
  /** Forward a command to the sim (the app records it for replays). No-op on the title. */
  command(c: Command): void;
  /** `name` is what the player called her; blank means Jane. */
  newGame(seed?: number, name?: string): void;
  /** True when a bed or a fire is in reach. The game can only be saved there. */
  canSave(): boolean;
  /** Always length 3; null = empty slot. */
  slots(): (SlotInfo | null)[];
  save(slot: number): boolean;
  load(slot: number): boolean;
  toTitle(): void;
  /** data: URL of a 16x16 icon drawn at 2x, for <img> / background-image. */
  iconUrl(icon: string): string;
  /** CSS colour per tile id, for the minimap. */
  tileColors(): readonly string[];
  /** Run one terminal line; returns output lines. */
  terminal(line: string): string[];
  /**
   * Button labels for whichever device was touched last ("E" / "Esc" / "W S" on a
   * keyboard, "A" / "B" / "D-pad" on a pad), for on-screen hints.
   */
  hints(): { confirm: string; cancel: string; choose: string; use: string };
  /** Rows for the controls screen. */
  bindings(): { action: string; keys: string; pad: string }[];
  /** Multi-line text for the F2 overlay. */
  debugText(): string;
  /** Build string for the title screen. */
  readonly version: string;
}

export interface UiApi {
  /** Called once per rendered frame with the sim events drained since the last call. */
  frame(events: SimEvent[]): void;
  /** Returns true when the UI consumed the action. */
  action(a: UiAction): boolean;
  /** True while a window that pauses the world is open (title, bags, pause menu, terminal). */
  blocksWorld(): boolean;
  /** True while a text field has focus: the input layer must not read game keys. */
  capturesKeys(): boolean;
  /** Show a toast from outside the sim (save written, load failed). */
  toast(text: string): void;
}
