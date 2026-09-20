// Pure focus model for the tabbed window: where the pad/keyboard cursor can sit
// and where a direction takes it. Also the drag payload / drop target vocabulary
// shared by pointer drags and pad "pick up, put down".
//
// Spatial layout the rules mirror (inventory tab):
//
//   bag    8 x 3 grid
//   craft  [in0][in1][in2] -> [out]        (only while at a bench)
//   bar    8 slots, bottom of the screen
//
// Spellbook tab: a vertical list, then the bar. Quests tab: a list. Map: nothing.

export type TabId = "inventory" | "book" | "quests" | "map";
export const TABS: readonly TabId[] = ["inventory", "book", "quests", "map"];

export type Region = "bag" | "craft" | "bar" | "list" | "none";
export type Cursor = { region: Region; index: number };
export type Dir = "up" | "down" | "left" | "right";
export type NavLayout = { tab: TabId; craft: boolean; listLen: number };

export const BAG_COLS = 8;
export const BAG_ROWS = 3;
export const BAR_COUNT = 8;
/** Craft cells 0..2 are inputs, 3 is the output. */
export const CRAFT_CELLS = 4;

export type DragPayload = { kind: "bag"; slot: number } | { kind: "spell"; id: string } | { kind: "bar"; slot: number };

export type DropTarget =
  | { kind: "bag"; slot: number }
  | { kind: "craft"; slot: number }
  | { kind: "bar"; slot: number }
  | { kind: "barbg" }
  | { kind: "window" }
  | { kind: "outside" };

/** `data-drop` attribute -> target. "bag:3", "craft:1", "bar:0", "barbg", "window"; anything else is outside. */
export function parseDropTarget(attr: string | null | undefined): DropTarget {
  if (!attr) return { kind: "outside" };
  const [kind, n] = attr.split(":");
  const slot = Number(n);
  if ((kind === "bag" || kind === "craft" || kind === "bar") && Number.isInteger(slot) && slot >= 0) return { kind, slot };
  if (kind === "barbg" || kind === "window") return { kind };
  return { kind: "outside" };
}

export function cycleTab(tab: TabId, dir: 1 | -1): TabId {
  const i = TABS.indexOf(tab);
  return TABS[(i + dir + TABS.length) % TABS.length];
}

export function homeCursor(l: NavLayout): Cursor {
  if (l.tab === "inventory") return { region: "bag", index: 0 };
  if (l.tab === "book") return l.listLen > 0 ? { region: "list", index: 0 } : { region: "bar", index: 0 };
  if (l.tab === "quests") return l.listLen > 0 ? { region: "list", index: 0 } : { region: "none", index: 0 };
  return { region: "none", index: 0 };
}

const clamp = (n: number, lo: number, hi: number): number => (n < lo ? lo : n > hi ? hi : n);

export function moveCursor(c: Cursor, dir: Dir, l: NavLayout): Cursor {
  const dx = dir === "left" ? -1 : dir === "right" ? 1 : 0;
  const dy = dir === "up" ? -1 : dir === "down" ? 1 : 0;
  switch (c.region) {
    case "bag": {
      const col = c.index % BAG_COLS;
      const row = Math.floor(c.index / BAG_COLS);
      if (dx !== 0) return { region: "bag", index: row * BAG_COLS + clamp(col + dx, 0, BAG_COLS - 1) };
      const nr = row + dy;
      if (nr < 0) return c;
      if (nr < BAG_ROWS) return { region: "bag", index: nr * BAG_COLS + col };
      return l.craft ? { region: "craft", index: Math.min(CRAFT_CELLS - 1, col >> 1) } : { region: "bar", index: col };
    }
    case "craft": {
      if (dx !== 0) return { region: "craft", index: clamp(c.index + dx, 0, CRAFT_CELLS - 1) };
      if (dy < 0) return { region: "bag", index: (BAG_ROWS - 1) * BAG_COLS + c.index * 2 };
      return { region: "bar", index: c.index * 2 };
    }
    case "bar": {
      if (dx !== 0) return { region: "bar", index: clamp(c.index + dx, 0, BAR_COUNT - 1) };
      if (dy > 0) return c;
      if (l.tab === "inventory") {
        return l.craft
          ? { region: "craft", index: Math.min(CRAFT_CELLS - 1, c.index >> 1) }
          : { region: "bag", index: (BAG_ROWS - 1) * BAG_COLS + c.index };
      }
      if (l.tab === "book" && l.listLen > 0) return { region: "list", index: l.listLen - 1 };
      return c;
    }
    case "list": {
      if (dy === 0 || l.listLen === 0) return c;
      const n = c.index + dy;
      if (n >= l.listLen) return l.tab === "book" ? { region: "bar", index: 0 } : c;
      return { region: "list", index: Math.max(0, n) };
    }
    default:
      return c;
  }
}

/** Where a pad "put down" lands, in the same vocabulary a pointer drop uses. */
export function cursorTarget(c: Cursor): DropTarget {
  if (c.region === "bag") return { kind: "bag", slot: c.index };
  if (c.region === "bar") return { kind: "bar", slot: c.index };
  if (c.region === "craft" && c.index < CRAFT_CELLS - 1) return { kind: "craft", slot: c.index };
  return { kind: "window" };
}
