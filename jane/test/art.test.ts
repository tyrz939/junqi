import { describe, expect, it } from "vitest";
import { CORPSES, remainsOf } from "@/art/corpse";
import { ICONS } from "@/art/icons";
import { PROP_SPRITES } from "@/art/props";
import type { SpriteSheet } from "@/art/types";
import { UNIT_SPRITES } from "@/art/units";
import { Fx, SPELL_FX, STATUS_FX } from "@/render/fx";
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

  it("every unit has a real dead frame: lower than it stood, on the ground, not a standing drawing", () => {
    const ink = (rows: string[]): { top: number; bottom: number; n: number } => {
      let top = Infinity;
      let bottom = -1;
      let n = 0;
      rows.forEach((r, y) => {
        for (const c of r) if (c !== ".") {
          top = Math.min(top, y);
          bottom = Math.max(bottom, y);
          n++;
        }
      });
      return { top, bottom, n };
    };
    const sprites = new Set(Object.values(catalog.units).map((u) => u.sprite));
    for (const id of sprites) {
      const s = UNIT_SPRITES[id];
      const dead = s.frames.dead;
      expect(dead, `${id} has no dead frame`).toBeDefined();
      for (const f of ["down", "up", "side"]) expect(dead.join(""), `${id}: dead is the "${f}" drawing`).not.toBe(s.frames[f].join(""));
      const d = ink(dead);
      const stand = ink(s.frames.down);
      expect(d.n, `${id}: dead frame is empty`).toBeGreaterThan(8);
      // Lying down: shorter than standing (a body on the ground, not a figure).
      expect(d.bottom - d.top, `${id}: dead is as tall as alive`).toBeLessThan(stand.bottom - stand.top);
      // On the ground: nothing floats. Its lowest pixel is at or below the feet, give or take one.
      expect(d.bottom, `${id}: dead body floats`).toBeGreaterThanOrEqual(Math.min(s.ay, s.h - 1) - 1);
    }
  });

  it("every corpse rule names a sprite that exists, and remains are known for every unit", () => {
    for (const id of Object.keys(CORPSES)) expect(UNIT_SPRITES[id], `corpse.ts lists "${id}", which is not drawn`).toBeDefined();
    for (const u of Object.values(catalog.units)) expect(["blood", "oil", "sap", "stuffing", "wax", "ichor", "bone", "stone", "ghost"]).toContain(remainsOf(u.sprite));
  });

  it("every spell and every effect has a look of its own (render/fx.ts), with the parts its kind needs", () => {
    for (const [id, s] of Object.entries(catalog.spells)) {
      const fx = SPELL_FX[id];
      expect(fx, `spells.${id} has no entry in SPELL_FX`).toBeDefined();
      if (s.kind === "bolt") expect(fx.bolt, `spells.${id} is a bolt with nothing to draw in flight`).toBeDefined();
      if (s.kind === "ground") expect(fx.ground, `spells.${id} lies on the ground with nothing to draw there`).toBeDefined();
      if (s.kind === "bolt" || s.kind === "melee") expect(fx.impact, `spells.${id} lands with no impact`).not.toBe("none");
      if (s.kind === "world") expect(fx.cast, `spells.${id} is a world verb with no cast`).not.toBe("none");
    }
    for (const id of Object.keys(SPELL_FX)) expect(catalog.spells[id], `SPELL_FX.${id} is not a spell`).toBeDefined();
    for (const [id, e] of Object.entries(catalog.effects)) {
      const look = STATUS_FX[id];
      expect(look, `effects.${id} has no entry in STATUS_FX`).toBeDefined();
      // Only an instant effect may have nothing to wear.
      if (look === "instant") expect(e.duration, `effects.${id} lasts, so it needs a look`).toBe(0);
    }
    for (const id of Object.keys(STATUS_FX)) expect(catalog.effects[id], `STATUS_FX.${id} is not an effect`).toBeDefined();
  });

  it("every look draws something: bolts, grounds and statuses put pixels down, impacts and casts leave particles", () => {
    const fx = new Fx(7);
    let px = 0;
    const p = { rect: (): void => void px++ };
    const looks = new Set(Object.values(SPELL_FX));
    for (const look of looks) {
      if (look.bolt) {
        px = 0;
        fx.drawBolt(p, look.bolt, 50, 50, 3, 0, 4, 20);
        expect(px, `bolt ${look.bolt}`).toBeGreaterThan(2);
      }
      if (look.ground) {
        px = 0;
        fx.drawGround(p, look.ground, 50, 50, 12, 30, 60, 30);
        expect(px, `ground ${look.ground}`).toBeGreaterThan(10);
      }
    }
    for (const id of Object.keys(SPELL_FX)) {
      fx.clear();
      fx.impact(id, 50, 50);
      fx.cast(id, 50, 50, 0);
      if (SPELL_FX[id].impact !== "none" || !["none", "swing"].includes(SPELL_FX[id].cast)) expect(fx.count, `spell ${id} leaves nothing`).toBeGreaterThan(0);
    }
    for (const [id, look] of Object.entries(STATUS_FX)) {
      if (look === "instant") continue;
      fx.clear();
      px = 0;
      for (let t = 0; t < 60; t++) fx.drawStatus(p, look, 50, 50, 18, t, 200);
      expect(px + fx.count, `status ${id} shows nothing`).toBeGreaterThan(0);
    }
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
