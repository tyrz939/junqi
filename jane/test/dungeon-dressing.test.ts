// Dungeon dressing that makes sense (Sept 24, after playing: "go over torch (and other item)
// positions in dungeons to make sure they make sense. on walls probably better than floor").
//
// Lamps are put up by rule (world/dungeon/lights.ts), in the wall cell, facing the floor they
// light; rooms marked dark keep only their own light; the Museum's lamps go out with its
// breaker. Templates keep their furniture against walls and out of doorways (lint rule 6).

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS, Tile } from "@/sim/grid";
import { DUNGEONS } from "@/world/dungeon";
import { infoOf } from "@/world/dungeon/generate";
import { lampsAlong } from "@/world/dungeon/lights";
import { brokenRooms, TEMPLATES } from "@/world/dungeon/pools";
import { lintRoom } from "@/world/dungeon/room";
import { buildZone } from "@/world/index";

const catalog = buildCatalog();
const INTO: Record<string, [number, number]> = { n: [0, 1], s: [0, -1], w: [1, 0], e: [-1, 0] };

describe("lamps along a wall", () => {
  it("spread evenly, half a gap from a corner, and one clear cell from a doorway", () => {
    expect(lampsAlong(25, 9, false, false)).toEqual([4, 12, 20]);
    const flanked = lampsAlong(20, 9, true, true);
    expect(flanked[0]).toBe(1);
    expect(flanked[flanked.length - 1]).toBe(18);
    expect(lampsAlong(1, 9, false, false)).toEqual([]);
    expect(lampsAlong(2, 9, true, false)).toEqual([1]);
  });
});

describe("every room template", () => {
  it("parses, and lints clean: furniture against walls, doorways clear, no lamp on the floor", () => {
    expect(brokenRooms()).toEqual([]);
    for (const t of TEMPLATES) expect(lintRoom(t), t.id).toEqual([]);
  });
});

describe("the lamps of a generated dungeon", () => {
  for (const [id, def] of Object.entries(DUNGEONS)) {
    const plan = def.lights;
    if (!plan) continue;
    it(`${id}: every lamp hangs in a wall cell facing open floor, every lit room has some, and a dark room has none`, () => {
      for (const seed of [1, 2, 3]) {
        const bp = buildZone(id as never, seed);
        const info = infoOf(bp)!;
        const wall = (Tile as unknown as Record<string, number>)[def.tiles.wall];
        const lamps = bp.props.filter((p) => p.def.startsWith(`${plan.prop}_`));
        expect(lamps.length, `${id} seed ${seed}`).toBeGreaterThan(info.rooms.length);
        for (const p of lamps) {
          const side = p.def.slice(plan.prop.length + 1);
          const [dx, dy] = INTO[side];
          expect(bp.tiles[p.cy * bp.w + p.cx], `${p.key} is in a wall`).toBe(wall);
          expect(TILE_FLAGS[bp.tiles[(p.cy + dy) * bp.w + p.cx + dx]] & F_SOLID, `${p.key} faces open floor`).toBe(0);
          expect(catalog.props[p.def].solid, `${p.def} stands in nobody's way`).toBe(false);
        }
        // No lamp hangs over a door or a way out, or in the wall cell beside one: the pair flanks it a cell clear.
        const doors = bp.props.filter((p) => p.to || p.def === "door");
        expect(doors.length, `${id} seed ${seed} has a way out`).toBeGreaterThan(0);
        for (const d of doors) {
          const f = catalog.props[d.def];
          for (const p of lamps) {
            const dx = Math.max(d.cx - p.cx, p.cx - (d.cx + f.w - 1), 0);
            const dy = Math.max(d.cy - p.cy, p.cy - (d.cy + f.h - 1), 0);
            expect(Math.max(dx, dy), `${p.key} hangs clear of ${d.key}`).toBeGreaterThan(1);
          }
        }
        for (const r of info.rooms) {
          const mine = lamps.filter((p) => new RegExp(`^${id}_${r.node.id}_lamp_\\d+$`).test(p.key)).length;
          if (r.node.dark) expect(mine, `${id} ${r.node.id} is dark on purpose`).toBe(0);
          else expect(mine, `${id} ${r.node.id} is lit`).toBeGreaterThanOrEqual(4);
        }
        // No standing torch is dressing any more: what stands on the floor was put there by the mission.
        expect(bp.props.filter((p) => p.def === "torch" && !def.nodes.some((n) => p.key.startsWith(`${id}_${n.id}_prop_`) || n.binds.some((b) => b.as === p.key)))).toEqual([]);
      }
    });
  }

  it("the Museum's wall lamps are on the breaker: the list that puts the building out puts them out too", () => {
    const bp = buildZone("museum", 1);
    const lamps = bp.props.filter((p) => p.def.startsWith("museum_wall_lamp_"));
    expect(lamps.length).toBeGreaterThan(0);
    for (const p of lamps) expect(p.on, p.key).toBe(true);
    const breaker = bp.props.find((p) => infoOf(bp)!.controls.some((c) => c.prop === p.key))!;
    const text = JSON.stringify(breaker.use);
    for (const p of lamps) expect(text, p.key).toContain(`{"do":"switch","prop":"${p.key}","on":false}`);
  });

  it("the Works' cages are empty and the Burial's torches are cold: neither changes who sees her or who keeps off", () => {
    for (const s of ["n", "s", "e", "w"]) {
      expect(catalog.props[`lamp_cage_${s}`].light).toBeUndefined();
      expect(catalog.props[`cold_sconce_${s}`].light?.cold).toBe(true);
    }
  });
});
