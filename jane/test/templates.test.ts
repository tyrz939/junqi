// Room templates (DUNGEONS.md 2.3, 2.4): every .room file parses, obeys the lint rules in
// every transform it claims, keeps its pool's contract, and proves its own promises when it
// is stamped alone and solved from each of its doors.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { DUNGEONS } from "@/world/dungeon";
import { lintDef } from "@/world/dungeon/checks";
import { proveTemplate } from "@/world/dungeon/harness";
import { poolOf, TEMPLATES } from "@/world/dungeon/pools";
import { lintRoom, parseRoom, shapeOf, shapesOf } from "@/world/dungeon/room";

const catalog = buildCatalog();
const defs = Object.values(DUNGEONS);

/** A small room that obeys every rule; the tests below break it one rule at a time. */
const GOOD = `
id      test.good
pool    test.good
bays    1x1
turn    0 180 mirror
doors   n1 optional, s1 optional
needs   -
grants  chest:reward
blocks  chest:reward until plate:main
heat    2
grid
#######nnn#######
#......___......#
#...............#
#..PP.......CC..#
#..PP.......CC..#
#...............#
#...BB..........#
#...BB....r.....#
#...............#
#...............#
#......___......#
#######sss#######
legend
n door n1
s door s1
P plate:main 2x2
C chest:reward 2x2
B push:main 2x2 barrel | crate
r spawn
`;

function broken(change: (text: string) => string): string[] {
  return lintRoom(parseRoom(change(GOOD), "test"));
}

describe("room templates", () => {
  it("every .room file parses and is clean under the lint, in every transform it claims", () => {
    expect(TEMPLATES.length).toBeGreaterThanOrEqual(12);
    for (const t of TEMPLATES) {
      expect(lintRoom(t), t.id).toEqual([]);
      expect(shapesOf(t).filter((s) => s.fits).length, `${t.id} fits in no transform`).toBeGreaterThan(0);
    }
  });

  it("the lint catches each rule it claims to", () => {
    expect(lintRoom(parseRoom(GOOD, "test"))).toEqual([]);
    // 1: a hole in the rim that is not a door
    expect(broken((t) => t.replace("#...............#\n#..PP", "................#\n#..PP")).join(" ")).toMatch(/rule 1/);
    // a door with no sill inside it
    expect(broken((t) => t.replace("#......___......#\n#...............#\n#..PP", "#...............#\n#...............#\n#..PP")).join(" ")).toMatch(/not a sill/);
    // 2: something solid on the line between two facing doors
    expect(broken((t) => t.replace("#..PP.......CC..#\n#..PP.......CC..#", "#..PP..CC.......#\n#..PP..CC.......#")).join(" ")).toMatch(/rule 2/);
    // 3: a spawn socket in a doorway
    expect(broken((t) => t.replace("#...............#\n#..PP", "#.......r.......#\n#..PP")).join(" ")).toMatch(/rule 3/);
    // 5: a plate and nothing to push onto it
    expect(broken((t) => t.replace(/BB/g, "..").replace("B push:main 2x2 barrel | crate\n", "")).join(" ")).toMatch(/rule 5/);
    // 5: the thing to push is walled off from the plate
    expect(broken((t) => t.replace("#...............#\n#...BB", "#################\n#...BB")).join(" ")).toMatch(/rule 5/);
    // 6: a letter that is not in the legend is a parse error, not a surprise
    expect(() => parseRoom(GOOD.replace("#...BB....r.....#", "#...BB....r..Z..#"), "test")).toThrow(/not in the legend/);
    // a transform that does not fit its bay: a room as wide as a bay cannot be turned on its side
    const wide = TEMPLATES.find((t) => t.grid[0].length > 25 && t.bays[0] === 1)!;
    expect(lintRoom({ ...wide, turns: [0, 90] }).join(" ")).toMatch(/does not fit/);
  });

  it("turning and mirroring keep every socket on the floor and every door on the rim", () => {
    for (const t of TEMPLATES) {
      for (const s of shapesOf(t)) {
        for (const d of s.doors) {
          const onRim = d.side === "n" ? d.cy === 0 : d.side === "s" ? d.cy === s.h - 1 : d.side === "w" ? d.cx === 0 : d.cx === s.w - 1;
          expect(onRim, `${t.id} turn ${s.turn} door ${d.id}`).toBe(true);
        }
        for (const k of s.sockets) {
          for (let j = 0; j < k.h; j++) for (let i = 0; i < k.w; i++) expect(s.cells[k.cy + j][k.cx + i], `${t.id} turn ${s.turn} ${k.id}`).toBe(".");
        }
      }
      // Four quarter turns are the template again.
      const round = shapeOf(t, 0, false);
      expect(round.cells.map((row, y) => row.length === t.grid[y].length).every(Boolean)).toBe(true);
    }
  });

  it("variants of a pool differ in their grid and never in their contract; every pool a mission names exists", () => {
    for (const def of defs) {
      expect(lintDef(def, catalog), def.id).toEqual([]);
      for (const node of def.nodes) {
        const pool = poolOf(node.pool);
        expect(pool.length, `${def.id}: pool ${node.pool} is empty`).toBeGreaterThan(0);
        const face = (n: number): string => {
          const t = pool[n];
          return JSON.stringify([t.bays, t.doors.filter((d) => d.required).map((d) => d.id), t.needs, [...t.grants].sort(), t.blocks]);
        };
        for (let n = 1; n < pool.length; n++) expect(face(n), `${pool[n].id} against ${pool[0].id}`).toBe(face(0));
        for (const t of pool) {
          for (const h of node.holds) expect(t.sockets.some((s) => s.id === h.socket), `${t.id} has no socket ${h.socket} for node ${node.id}`).toBe(true);
          for (const b of node.binds) {
            const found = t.sockets.some((s) => s.id === b.from) || t.marks.some((m) => m.id === b.from) || t.rects.some((r) => r.id === b.from);
            expect(found, `${t.id} has nothing called ${b.from} for node ${node.id}`).toBe(true);
          }
          // Enough doors for every edge the node has, whatever the layout asks of it.
          const edges = def.edges.filter((e) => e.kind.t !== "sight" && (e.from === node.id || e.to === node.id)).length;
          expect(t.doors.length, `${t.id}: ${edges} edges`).toBeGreaterThanOrEqual(edges);
        }
      }
    }
  });

  it("every template proves its grants and its blocks, alone, from every door, in every transform", () => {
    for (const def of defs) {
      for (const node of def.nodes) {
        for (const t of poolOf(node.pool)) expect(proveTemplate(def, node, t, catalog), `${t.id} as ${node.id}`).toEqual([]);
      }
    }
  });

  it("the harness really catches a variant that opens a way round its own lock", () => {
    const def = DUNGEONS.mine;
    const node = def.nodes.find((n) => n.id === "plate")!;
    const t = poolOf("mine.plate")[0];
    // The same room, but the mission forgets to lock the chest: `chest:reward until plate:main` no longer holds.
    const careless = { ...node, holds: node.holds.map((h) => (h.socket === "chest:reward" ? { ...h, locked: false } : h)) };
    expect(proveTemplate(def, careless, t, catalog).join(" ")).toMatch(/opens without plate:main/);
  });
});
