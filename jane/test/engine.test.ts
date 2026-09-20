import { describe, expect, it } from "vitest";
import { buildCatalog, recipeKey, validateCatalog } from "@/sim/catalog";
import { Grid, Tile } from "@/sim/grid";
import { lineOfSight } from "@/sim/los";
import { costOfCells, PathFinder } from "@/sim/path";
import { irandom, rngSeed } from "@/sim/rng";
import { rotate, turnToward } from "@/sim/angles";

describe("catalog", () => {
  it("has no dangling ids, bad rows or duplicate recipes", () => {
    const c = buildCatalog();
    expect(validateCatalog(c)).toEqual([]);
  });

  it("keys recipes by sorted item ids, not display names or order", () => {
    const c = buildCatalog();
    expect(recipeKey(["small_water", "pansy", "gold_dust"])).toBe(recipeKey(["gold_dust", "small_water", "pansy"]));
    expect(c.recipeIndex[recipeKey(["pansy", "small_water", "gold_dust"])].output).toBe("potion_manashield");
  });

  it("converts seconds to integer ticks once", () => {
    const c = buildCatalog();
    expect(c.spells.icebolt.cooldown).toBe(120);
    expect(c.items.apple.cooldown).toBe(300);
    expect(c.units.skeleton.respawn).toBe(36000);
  });

  it("every quest can be given and handed in by some row", () => {
    const c = buildCatalog();
    const text = JSON.stringify([c.dialogue, c.triggers, c.quests, c.start, c.units, c.items]);
    for (const id of Object.keys(c.quests)) {
      expect(text.includes(`"do":"quest","quest":"${id}"`) || c.start.quests.includes(id), `${id} is never offered`).toBe(true);
      expect(text.includes(`"do":"handin","quest":"${id}"`), `${id} can never be handed in`).toBe(true);
    }
  });
});

describe("rng", () => {
  it("is a pure function of the seed", () => {
    const a = rngSeed(1234, 1);
    const b = rngSeed(1234, 1);
    const seqA = Array.from({ length: 50 }, () => irandom(a, 1000));
    const seqB = Array.from({ length: 50 }, () => irandom(b, 1000));
    expect(seqA).toEqual(seqB);
    expect(new Set(seqA).size).toBeGreaterThan(30);
  });

  it("irandom is inclusive like GameMaker", () => {
    const s = rngSeed(7);
    const seen = new Set<number>();
    for (let i = 0; i < 400; i++) seen.add(irandom(s, 3));
    expect([...seen].sort()).toEqual([0, 1, 2, 3]);
    expect(irandom(s, 0.9)).toBe(0);
  });
});

describe("angles", () => {
  it("rotates without runtime trig and turns by a clamped amount", () => {
    const r = rotate(1, 0, 90);
    expect(r.x).toBeCloseTo(0, 6);
    expect(r.y).toBeCloseTo(1, 6);
    const t = turnToward(1, 0, 0, 1, 3);
    expect(t.y).toBeGreaterThan(0);
    expect(t.y).toBeLessThan(0.06);
    const snap = turnToward(1, 0, 0.9999, 0.0141, 3);
    expect(snap.x).toBeCloseTo(0.9999, 4);
  });
});

function walled(): Grid {
  const g = new Grid(64, 64);
  g.fill(0, 0, 64, 64, Tile.Floor);
  g.fill(30, 0, 1, 50, Tile.Wall); // wall with a gap at the bottom
  return g;
}

describe("pathfinding", () => {
  it("routes round a wall and never cuts a corner", () => {
    const g = walled();
    const p = new PathFinder(g);
    const path = p.find(10, 10, 50, 10, 1, costOfCells(400));
    expect(path).not.toBeNull();
    for (const cell of path!) expect(g.solid(cell % 64, Math.floor(cell / 64))).toBe(false);
    expect(path![path!.length - 1]).toBe(10 * 64 + 50);
    expect(path!.some((c) => Math.floor(c / 64) >= 50)).toBe(true);
  });

  it("reaches a goal cell that another unit is standing on", () => {
    // The 2026 Phaser build returned null here, so nothing could chase a standing player.
    const g = walled();
    g.occupy(10 * 64 + 20, 99);
    const p = new PathFinder(g);
    expect(p.find(10, 10, 20, 10, 1, costOfCells(100))).not.toBeNull();
  });

  it("paths around other units' cells", () => {
    const g = new Grid(16, 5);
    g.fill(0, 0, 16, 5, Tile.Floor);
    g.fill(0, 0, 16, 1, Tile.Wall);
    g.fill(0, 4, 16, 1, Tile.Wall);
    g.occupy(2 * 16 + 8, 7);
    const path = new PathFinder(g).find(2, 2, 14, 2, 1, costOfCells(100))!;
    expect(path.includes(2 * 16 + 8)).toBe(false);
  });

  it("gives up inside its budget when the goal is sealed off", () => {
    const g = new Grid(400, 400);
    g.fill(0, 0, 400, 400, Tile.Floor);
    g.fill(100, 100, 9, 1, Tile.Wall);
    g.fill(100, 108, 9, 1, Tile.Wall);
    g.fill(100, 100, 1, 9, Tile.Wall);
    g.fill(108, 100, 1, 9, Tile.Wall);
    const p = new PathFinder(g);
    const t0 = performance.now();
    expect(p.find(10, 10, 104, 104, 1, costOfCells(2000), 6000)).toBeNull();
    expect(performance.now() - t0).toBeLessThan(50);
    expect(p.stats.expanded).toBeLessThanOrEqual(6001);
  });

  it("uses constant memory and still paths anywhere in a ten-minute county", () => {
    // PLAN.md: 3600 x 2000 cells. The first A* sized its scratch to the grid: 115 MB here.
    const g = new Grid(3600, 2000);
    g.fill(0, 0, 3600, 2000, Tile.Floor);
    g.fill(3400, 1700, 1, 200, Tile.Wall);
    const p = new PathFinder(g);
    expect(p.scratchBytes).toBeLessThan(4 * 1024 * 1024);
    // Far corner of the map, well away from the window origin at (0,0): indices must remap.
    const path = p.find(3390, 1900, 3410, 1900, 1, costOfCells(600))!;
    expect(path).not.toBeNull();
    expect(path[path.length - 1]).toBe(1900 * 3600 + 3410);
    for (const c of path) expect(g.solid(c % 3600, Math.floor(c / 3600))).toBe(false);
    expect(path.some((c) => Math.floor(c / 3600) < 1700 || Math.floor(c / 3600) >= 1900)).toBe(true);
  });

  it("returns a partial path toward a goal beyond the search window", () => {
    const g = new Grid(2000, 400);
    g.fill(0, 0, 2000, 400, Tile.Floor);
    const p = new PathFinder(g);
    const path = p.find(100, 200, 1500, 200, 1, costOfCells(5000), 60000)!;
    expect(path).not.toBeNull();
    const end = path[path.length - 1];
    expect(end % 2000).toBeGreaterThan(180); // it got meaningfully closer
    expect(end % 2000).toBeLessThan(1500); // and did not pretend to arrive
    expect(p.stats.partial).toBe(1);
  });

  it("keeps occupancy sparse, and tile edits do not evict whoever stands there", () => {
    const g = new Grid(3600, 2000);
    g.fill(0, 0, 3600, 2000, Tile.Floor);
    const i = 1000 * 3600 + 1800;
    g.occupy(i, 42);
    expect(g.occupiedCount).toBe(1);
    expect(g.free(1800, 1000, 0)).toBe(false);
    expect(g.free(1800, 1000, 42)).toBe(true);
    g.occupy(i, 43); // already held: stays 42's
    expect(g.occupant(i)).toBe(42);
    g.setTile(1800, 1000, Tile.Grass);
    expect(g.occupant(i)).toBe(42);
    g.vacate(i, 43); // not the holder: no effect
    expect(g.occupant(i)).toBe(42);
    g.vacate(i, 42);
    expect(g.occupant(i)).toBe(0);
    expect(g.free(1800, 1000, 0)).toBe(true);
  });

  it("respects max path length", () => {
    const g = walled();
    expect(new PathFinder(g).find(10, 10, 50, 10, 1, costOfCells(45))).toBeNull();
  });
});

describe("line of sight", () => {
  it("is blocked by walls and open across water", () => {
    const g = new Grid(32, 8);
    g.fill(0, 0, 32, 8, Tile.Floor);
    g.fill(10, 0, 1, 8, Tile.Wall);
    g.fill(20, 0, 2, 8, Tile.Water);
    expect(lineOfSight(g, 4 * 8, 20, 9 * 8, 22)).toBe(true);
    expect(lineOfSight(g, 4 * 8, 20, 14 * 8, 22)).toBe(false);
    // Bolts crossed ponds in 2020; water blocks feet, not sight.
    expect(lineOfSight(g, 14 * 8, 20, 28 * 8, 30)).toBe(true);
    expect(g.solid(20, 3)).toBe(true);
  });

  it("cannot slip between diagonal walls", () => {
    const g = new Grid(8, 8);
    g.fill(0, 0, 8, 8, Tile.Floor);
    g.setTile(3, 4, Tile.Wall);
    g.setTile(4, 3, Tile.Wall);
    expect(lineOfSight(g, 3 * 8 + 1, 3 * 8 + 1, 4 * 8 + 7, 4 * 8 + 7)).toBe(false);
  });
});
