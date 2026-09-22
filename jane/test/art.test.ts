import { describe, expect, it } from "vitest";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import type { SpriteSheet } from "@/art/types";
import { UNIT_SPRITES } from "@/art/units";
import { TILE_COLORS } from "@/render/tiles";
import { buildCatalog } from "@/sim/catalog";
import { F_BLOCK_LOS, F_SOLID, Tile, TILE_COUNT, TILE_FLAGS } from "@/sim/grid";

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

  it("every tile id has a minimap colour, and the dressing tiles stop what they should", () => {
    expect(TILE_COLORS.length).toBe(TILE_COUNT);
    for (let t = 0; t < TILE_COUNT; t++) expect(TILE_COLORS[t], `tile ${t} colour`).toMatch(/^#[0-9a-f]{3,8}$/i);
    const solid = (t: Tile): boolean => (TILE_FLAGS[t] & F_SOLID) !== 0;
    const blind = (t: Tile): boolean => (TILE_FLAGS[t] & F_BLOCK_LOS) !== 0;
    for (const t of [Tile.FlowerBed, Tile.Stepping, Tile.Crops, Tile.Boardwalk]) expect(solid(t), `${Tile[t]} is walkable`).toBe(false);
    expect([solid(Tile.StoneWall), blind(Tile.StoneWall)]).toEqual([true, false]);
    for (const t of [Tile.RoofSlate, Tile.RoofThatch, Tile.BrickWall]) expect([solid(t), blind(t)]).toEqual([true, true]);
    expect(TILE_FLAGS[Tile.Pine]).toBe(TILE_FLAGS[Tile.Tree]);
    expect([solid(Tile.DeadTree), blind(Tile.DeadTree)]).toEqual([true, false]);
  });

  it("the well and the cart have drawings of their own, two cells wide", () => {
    for (const id of ["well", "cart"]) expect(PROP_SPRITES[id]?.w, id).toBe(16);
  });
});
