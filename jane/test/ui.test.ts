// UI helpers that need no DOM: formatting, sweep maths, the pad focus model.
import { describe, expect, it } from "vitest";
import {
  barSlotSig,
  clockText,
  costText,
  durationText,
  firstEmpty,
  isNight,
  latestSlot,
  lootText,
  packRgb,
  parseCssColor,
  slotSummary,
  stackSig,
  statusTime,
  stepIndex,
  sweepPercent,
  uiScale,
  useKeyLabel,
} from "@/ui/format";
import { cursorTarget, cycleTab, homeCursor, moveCursor, parseDropTarget, type Cursor, type NavLayout } from "@/ui/nav";

describe("ui format", () => {
  it("scales in whole steps of the 360-line design", () => {
    expect(uiScale(640, 360)).toBe(1);
    expect(uiScale(1280, 719)).toBe(1);
    expect(uiScale(1280, 720)).toBe(2);
    expect(uiScale(1920, 1080)).toBe(3);
    expect(uiScale(3840, 2160)).toBe(6);
    expect(uiScale(200, 100)).toBe(1);
    expect(uiScale(420, 900)).toBe(1); // portrait phone: width wins
  });

  it("prints the clock as HH:MM", () => {
    expect(clockText(0)).toBe("00:00");
    expect(clockText(17)).toBe("17:00");
    expect(clockText(6.5)).toBe("06:30");
    expect(clockText(23 + 59 / 60)).toBe("23:59");
    expect(clockText(24)).toBe("00:00");
    expect(isNight(12)).toBe(false);
    expect(isNight(18.5)).toBe(true);
    expect(isNight(3)).toBe(true);
  });

  it("formats tick durations", () => {
    expect(statusTime(1)).toBe("1");
    expect(statusTime(60)).toBe("1");
    expect(statusTime(61)).toBe("2");
    expect(statusTime(60 * 150)).toBe("3m");
    expect(durationText(90)).toBe("1.5 s");
    expect(durationText(300)).toBe("5 s");
    expect(durationText(60 * 180)).toBe("3 min");
  });

  it("sweeps from 0 (just used) to 100 (ready) in half-percent steps", () => {
    expect(sweepPercent(0, 90)).toBe(100);
    expect(sweepPercent(90, 90)).toBe(0);
    expect(sweepPercent(45, 90)).toBe(50);
    expect(sweepPercent(5, 0)).toBe(100);
    expect(sweepPercent(200, 90)).toBe(0);
    const v = sweepPercent(31, 90);
    expect(v * 2).toBe(Math.round(v * 2));
  });

  it("builds signatures that change exactly when the slot should repaint", () => {
    expect(stackSig(null)).toBe("");
    expect(stackSig({ item: "apple", qty: 3 })).toBe("apple*3");
    expect(barSlotSig(null, 0)).toBe("");
    expect(barSlotSig({ source: "spell", id: "icebolt" }, 9)).toBe(barSlotSig({ source: "spell", id: "icebolt" }, 0));
    expect(barSlotSig({ source: "item", id: "apple" }, 2)).not.toBe(barSlotSig({ source: "item", id: "apple" }, 1));
    expect(firstEmpty([{ source: "spell", id: "a" }, null, null])).toBe(1);
    expect(firstEmpty([{ source: "spell", id: "a" }])).toBe(-1);
  });

  it("describes costs, loot and save slots", () => {
    expect(costText({ mp: 0, energy: 0 })).toBe("Free");
    expect(costText({ mp: 12, energy: 0 })).toBe("12 MP");
    expect(costText({ mp: 5, energy: 20 })).toBe("5 MP + 20 EN");
    expect(lootText("Apple", 2)).toBe("+2 Apple");
    expect(slotSummary(null)).toBe("Empty");
    expect(slotSummary({ slot: 0, savedAt: "2026-01-01T00:00:00Z", zone: "Castle County", day: 0, hour: 17.5, hp: 49.2, maxhp: 100 })).toBe(
      "Castle County - Day 1, 17:30 - HP 50/100",
    );
  });

  it("finds the newest save", () => {
    const mk = (slot: number, savedAt: string) => ({ slot, savedAt, zone: "z", day: 0, hour: 0, hp: 1, maxhp: 1 });
    expect(latestSlot([null, null, null])).toBe(-1);
    expect(latestSlot([mk(0, "2026-01-01T10:00:00Z"), null, mk(2, "2026-03-01T10:00:00Z")])).toBe(2);
    expect(latestSlot([mk(0, "2026-05-01T10:00:00Z"), mk(1, "2026-03-01T10:00:00Z"), null])).toBe(0);
    expect(latestSlot([null, mk(1, "not a date"), null])).toBe(1);
  });

  it("steps menu focus over disabled rows, wrapping", () => {
    expect(stepIndex([false, true, false], 0, 1)).toBe(2);
    expect(stepIndex([false, true, false], 2, 1)).toBe(0);
    expect(stepIndex([false, true, false], 0, -1)).toBe(2);
    expect(stepIndex([true, true, false], 2, 1)).toBe(2);
    expect(stepIndex([], 0, 1)).toBe(0);
  });

  it("parses the colour forms the minimap palette may get", () => {
    expect(parseCssColor("#fff")).toEqual([255, 255, 255]);
    expect(parseCssColor("#3a5f2b")).toEqual([0x3a, 0x5f, 0x2b]);
    expect(parseCssColor("#3a5f2bff")).toEqual([0x3a, 0x5f, 0x2b]);
    expect(parseCssColor("rgb(1, 2, 3)")).toEqual([1, 2, 3]);
    expect(parseCssColor("rgba(10 20 30 / 0.5)")).toEqual([10, 20, 30]);
    expect(parseCssColor("rebeccapurple")).toBeNull();
    expect(packRgb(0x11, 0x22, 0x33)).toBe(0xff332211);
  });

  it("reads the USE key off the bindings table", () => {
    expect(useKeyLabel([{ action: "Move", keys: "W A S D" }, { action: "Use / Talk / hold: Push, Pull", keys: "E  F" }])).toBe("E");
    expect(useKeyLabel([{ action: "Move", keys: "W" }])).toBe("E");
  });
});

describe("ui nav", () => {
  const inv = (craft: boolean): NavLayout => ({ tab: "inventory", craft, listLen: 0 });
  const walk = (c: Cursor, dirs: ("up" | "down" | "left" | "right")[], l: NavLayout): Cursor => dirs.reduce((at, d) => moveCursor(at, d, l), c);

  it("moves inside the bag grid and stops at its edges", () => {
    expect(moveCursor({ region: "bag", index: 0 }, "left", inv(false))).toEqual({ region: "bag", index: 0 });
    expect(moveCursor({ region: "bag", index: 0 }, "up", inv(false))).toEqual({ region: "bag", index: 0 });
    expect(moveCursor({ region: "bag", index: 7 }, "right", inv(false))).toEqual({ region: "bag", index: 7 });
    expect(moveCursor({ region: "bag", index: 3 }, "down", inv(false))).toEqual({ region: "bag", index: 11 });
  });

  it("goes bag -> craft -> bar at a bench, bag -> bar away from one, and back", () => {
    expect(walk({ region: "bag", index: 21 }, ["down"], inv(true))).toEqual({ region: "craft", index: 2 });
    expect(walk({ region: "bag", index: 23 }, ["down"], inv(true))).toEqual({ region: "craft", index: 3 });
    expect(walk({ region: "bag", index: 21 }, ["down", "down"], inv(true))).toEqual({ region: "bar", index: 4 });
    expect(walk({ region: "bag", index: 21 }, ["down"], inv(false))).toEqual({ region: "bar", index: 5 });
    expect(walk({ region: "bar", index: 5 }, ["up"], inv(false))).toEqual({ region: "bag", index: 21 });
    expect(walk({ region: "bar", index: 5 }, ["up"], inv(true))).toEqual({ region: "craft", index: 2 });
    expect(walk({ region: "bar", index: 0 }, ["down", "left"], inv(true))).toEqual({ region: "bar", index: 0 });
  });

  it("every cell of the inventory tab is reachable from home", () => {
    for (const craft of [false, true]) {
      const l = inv(craft);
      const seen = new Set<string>();
      const queue: Cursor[] = [homeCursor(l)];
      while (queue.length > 0) {
        const c = queue.pop() as Cursor;
        const k = `${c.region}:${c.index}`;
        if (seen.has(k)) continue;
        seen.add(k);
        for (const d of ["up", "down", "left", "right"] as const) queue.push(moveCursor(c, d, l));
      }
      expect(seen.size).toBe(24 + 8 + (craft ? 4 : 0));
    }
  });

  it("walks the spellbook list and falls through to the bar", () => {
    const l: NavLayout = { tab: "book", craft: false, listLen: 3 };
    expect(homeCursor(l)).toEqual({ region: "list", index: 0 });
    expect(homeCursor({ ...l, listLen: 0 })).toEqual({ region: "bar", index: 0 });
    expect(walk({ region: "list", index: 0 }, ["up"], l)).toEqual({ region: "list", index: 0 });
    expect(walk({ region: "list", index: 0 }, ["down", "down", "down"], l)).toEqual({ region: "bar", index: 0 });
    expect(walk({ region: "bar", index: 4 }, ["up"], l)).toEqual({ region: "list", index: 2 });
    const q: NavLayout = { tab: "quests", craft: false, listLen: 2 };
    expect(walk({ region: "list", index: 1 }, ["down"], q)).toEqual({ region: "list", index: 1 });
    expect(moveCursor(homeCursor({ tab: "map", craft: false, listLen: 0 }), "down", q).region).toBe("none");
  });

  it("cycles tabs both ways", () => {
    expect(cycleTab("inventory", -1)).toBe("map");
    expect(cycleTab("map", 1)).toBe("inventory");
    expect(cycleTab("book", 1)).toBe("quests");
  });

  it("maps cursors and data-drop attributes onto the same targets", () => {
    expect(cursorTarget({ region: "bag", index: 5 })).toEqual(parseDropTarget("bag:5"));
    expect(cursorTarget({ region: "craft", index: 1 })).toEqual(parseDropTarget("craft:1"));
    expect(cursorTarget({ region: "craft", index: 3 })).toEqual({ kind: "window" });
    expect(cursorTarget({ region: "bar", index: 7 })).toEqual(parseDropTarget("bar:7"));
    expect(parseDropTarget("barbg")).toEqual({ kind: "barbg" });
    expect(parseDropTarget("window")).toEqual({ kind: "window" });
    expect(parseDropTarget("outside")).toEqual({ kind: "outside" });
    expect(parseDropTarget(null)).toEqual({ kind: "outside" });
    expect(parseDropTarget("bag:x")).toEqual({ kind: "outside" });
  });
});
