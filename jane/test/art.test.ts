import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import type { SpriteSheet } from "@/art/types";
import { UNIT_SPRITES } from "@/art/units";
import { buildCatalog } from "@/sim/catalog";

const catalog = buildCatalog();

function checkSheet(name: string, sheet: SpriteSheet, requiredFrames: string[]): void {
  for (const [id, s] of Object.entries(sheet)) {
    for (const f of requiredFrames) expect(s.frames[f], `${name}.${id} is missing frame "${f}"`).toBeDefined();
    for (const [fname, rows] of Object.entries(s.frames)) {
      expect(rows.length, `${name}.${id}.${fname} row count`).toBe(s.h);
      rows.forEach((row, y) => {
        expect(row.length, `${name}.${id}.${fname} row ${y} width`).toBe(s.w);
        for (const ch of row) {
          if (ch !== ".") expect(s.palette[ch], `${name}.${id}.${fname} row ${y}: char "${ch}" not in palette`).toBeDefined();
        }
      });
    }
    expect(s.ax).toBeGreaterThanOrEqual(0);
    expect(s.ax).toBeLessThanOrEqual(s.w);
    expect(s.ay).toBeGreaterThanOrEqual(0);
    expect(s.ay).toBeLessThanOrEqual(s.h);
  }
}

describe("art", () => {
  it("unit sprites are well formed and cover every unit row", () => {
    checkSheet("units", UNIT_SPRITES, ["down", "up", "side"]);
    for (const [id, u] of Object.entries(catalog.units)) expect(UNIT_SPRITES[u.sprite], `units.${id} -> sprite "${u.sprite}"`).toBeDefined();
  });

  it("prop sprites are well formed, cover every prop row, and are as wide as their footprint", () => {
    checkSheet("props", PROP_SPRITES, ["base"]);
    for (const [id, p] of Object.entries(catalog.props)) {
      const s = PROP_SPRITES[p.sprite];
      expect(s, `props.${id} -> sprite "${p.sprite}"`).toBeDefined();
      expect(s.w, `props.${id} sprite width`).toBe(p.w * 8);
      expect(s.h, `props.${id} sprite height`).toBeGreaterThanOrEqual(p.h * 8);
    }
  });

  it("icons are 16x16 and cover every item, spell and effect row", () => {
    checkSheet("icons", ICONS, ["base"]);
    for (const s of Object.values(ICONS)) expect([s.w, s.h]).toEqual([16, 16]);
    for (const [id, i] of Object.entries(catalog.items)) expect(ICONS[i.icon], `items.${id} -> icon "${i.icon}"`).toBeDefined();
    for (const [id, s] of Object.entries(catalog.spells)) expect(ICONS[s.icon], `spells.${id} -> icon "${s.icon}"`).toBeDefined();
    for (const [id, e] of Object.entries(catalog.effects)) expect(ICONS[e.icon], `effects.${id} -> icon "${e.icon}"`).toBeDefined();
  });
});
