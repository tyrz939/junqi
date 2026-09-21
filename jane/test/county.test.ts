// The playable county, rasterised from the skeleton. The skeleton is proven on many
// seeds (skeleton.test.ts); a full county is seven million cells, so it is proven on a
// few. What is checked here is what the skeleton cannot know: that the cells join up.

import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { Sim } from "@/sim/sim";
import { walkTo } from "./bot";
import { F_SOLID, TILE_FLAGS, Tile } from "@/sim/grid";
import { hashString } from "@/sim/rng";
import type { Blueprint } from "@/world/blueprint";
import { buildZone } from "@/world";
import { buildCounty, COUNTY_H, COUNTY_W, countySkeleton } from "@/world/county";
import { placementContract, type PlacementRow } from "@/world/placements";
import { at, MACRO } from "@/world/skeleton";

const SEEDS = [3, 2026];
const built = new Map<number, { bp: Blueprint; ms: number }>();
function county(seed: number): Blueprint {
  let b = built.get(seed);
  if (!b) {
    const t0 = performance.now();
    const bp = buildZone("county", seed);
    built.set(seed, (b = { bp, ms: performance.now() - t0 }));
  }
  return b.bp;
}

/** Every cell a walker can reach from a mark, 8-way, no corner cutting: the same rule the path finder uses. */
function reachable(bp: Blueprint, from: string): Uint8Array {
  const open = (x: number, y: number): boolean => x >= 0 && y >= 0 && x < bp.w && y < bp.h && (TILE_FLAGS[bp.tiles[y * bp.w + x]] & F_SOLID) === 0;
  const seen = new Uint8Array(bp.w * bp.h);
  const start = bp.marks[from];
  const stack = [start.cy * bp.w + start.cx];
  seen[stack[0]] = 1;
  while (stack.length > 0) {
    const c = stack.pop()!;
    const x = c % bp.w;
    const y = (c - x) / bp.w;
    for (let oy = -1; oy <= 1; oy++) {
      for (let ox = -1; ox <= 1; ox++) {
        if ((ox === 0 && oy === 0) || !open(x + ox, y + oy)) continue;
        if (ox !== 0 && oy !== 0 && (!open(x + ox, y) || !open(x, y + oy))) continue;
        const n = (y + oy) * bp.w + x + ox;
        if (!seen[n]) {
          seen[n] = 1;
          stack.push(n);
        }
      }
    }
  }
  return seen;
}

describe("the county", () => {
  it("is 3600 x 2000, carries the story's names, and builds in a second or two", () => {
    for (const seed of SEEDS) {
      const bp = county(seed);
      expect([bp.w, bp.h]).toEqual([COUNTY_W, COUNTY_H]);
      for (const m of ["start", "yard_gate", "house_front", "town_square", "mine_mouth", "burial_mouth", "graveyard_gate", "farm_gate", "school_mouth", "museum_mouth"]) {
        expect(bp.marks[m], `${seed}: mark ${m}`).toBeDefined();
      }
      for (const p of ["house_door", "mine_door", "burial_door", "yard_fire", "station_fire", "town_fire", "farm_fire", "reed_fire", "canteen_fire", "car_wreck", "sign_mine", "sign_town"]) {
        expect(bp.props.some((x) => x.key === p), `${seed}: prop ${p}`).toBe(true);
      }
      expect(built.get(seed)!.ms, "build + validation, ms").toBeLessThan(6000);
    }
  });

  it("every story place can be walked to from the platform", () => {
    for (const seed of SEEDS) {
      const bp = county(seed);
      const seen = reachable(bp, "start");
      for (const [name, m] of Object.entries(bp.marks)) {
        expect(seen[m.cy * bp.w + m.cx], `${seed}: ${name} at ${m.cx},${m.cy}`).toBe(1);
      }
      // The car is off every road and has no mark: somewhere beside it must be reachable.
      const car = bp.props.find((p) => p.key === "car_wreck")!;
      let beside = 0;
      for (let ox = -2; ox <= 6; ox++) for (let oy = -2; oy <= 4; oy++) beside += seen[(car.cy + oy) * bp.w + car.cx + ox];
      expect(beside).toBeGreaterThan(0);
    }
  });

  it("the first walk is a road all the way, and it is lit", () => {
    const bp = county(SEEDS[0]);
    const sk = countySkeleton(SEEDS[0], bp.attempts - 1);
    const first = sk.roads.find((r) => r.from === "station")!;
    let road = 0;
    for (const c of first.cells) {
      const x = (c % sk.w) * MACRO + MACRO / 2;
      const y = Math.floor(c / sk.w) * MACRO + MACRO / 2;
      let here = false;
      for (let oy = -6; oy <= 6 && !here; oy++) for (let ox = -6; ox <= 6; ox++) if (bp.tiles[(y + oy) * bp.w + x + ox] === Tile.Road) here = true;
      if (here) road++;
    }
    expect(road / first.cells.length).toBeGreaterThan(0.85);
    const lamps = bp.props.filter((p) => p.def === "lamp_post").length;
    expect(lamps).toBeGreaterThan(60);
  });

  it("nothing is spawned in a haven, on the first walk, or inside something solid; threat sets the phase", () => {
    const bp = county(SEEDS[0]);
    const sk = countySkeleton(SEEDS[0], bp.attempts - 1);
    const wild = bp.units.filter((u) => u.phase !== undefined);
    expect(wild.length).toBeGreaterThan(300);
    expect(wild.length).toBeLessThan(1500);
    for (const u of wild) {
      const threat = sk.threat[at(u.cx >> 4, u.cy >> 4)];
      expect(threat).toBeGreaterThan(0);
      expect(u.phase).toBe(threat);
      expect((TILE_FLAGS[bp.tiles[u.cy * bp.w + u.cx]] & F_SOLID) === 0).toBe(true);
    }
    // Danger is a map: the Works are not populated like the Lowfields.
    const phases = new Set(wild.map((u) => u.phase));
    expect(phases.has(1) && phases.has(4)).toBe(true);
  });

  it("is the same county for the same seed", () => {
    const a = county(SEEDS[1]);
    const b = buildZone("county", SEEDS[1]);
    const sum = (bp: Blueprint): number => {
      let h = 0x811c9dc5;
      for (let i = 0; i < bp.tiles.length; i += 7) h = Math.imul(h ^ bp.tiles[i], 0x01000193) >>> 0;
      return h ^ hashString(JSON.stringify([bp.units, bp.props, bp.marks]));
    };
    expect(sum(b)).toBe(sum(a));
  });
});

describe("the first evening", () => {
  it("she steps off the train and can walk the lit road to Julie's gate in two to four minutes", () => {
    const seed = SEEDS[0];
    const sim = Sim.newGame(buildCatalog(), seed);
    const bp = county(seed);
    const sk = countySkeleton(seed, bp.attempts - 1);
    expect(Math.abs(sim.player.x / 8 - bp.marks.start.cx)).toBeLessThan(6);
    const road = sk.roads.find((r) => r.from === "station" && r.to === "julie_house")!;
    const t0 = sim.state.tick;
    // Follow the road the way a person would: from one stretch of it to the next.
    for (let n = 4; n < road.cells.length - 3; n += 4) {
      const c = road.cells[n];
      const x = (c % sk.w) * MACRO * 8 + 64;
      const y = Math.floor(c / sk.w) * MACRO * 8 + 64;
      expect(walkTo(sim, x, y, 40, 60 * 60), `stretch ${n} of ${road.cells.length}`).toBe(true);
    }
    const gate = bp.marks.yard_gate;
    expect(walkTo(sim, gate.cx * 8 + 4, gate.cy * 8 + 4, 12, 60 * 90)).toBe(true);
    const minutes = (sim.state.tick - t0) / 3600;
    expect(minutes).toBeGreaterThan(1.5);
    expect(minutes).toBeLessThan(4.5);
    expect(sim.player.alive).toBe(true);
    // And the dog is where it always is.
    const dog = sim.rt.unitsByKey.get("dog")!;
    expect(Math.hypot(dog.x - sim.player.x, dog.y - sim.player.y)).toBeLessThan(80 * 8);
  });
});

describe("placements: content says where by name", () => {
  const rows: PlacementRow[] = [
    { key: "t_board", at: { mark: "town_square" }, prop: { def: "sign", talk: "sign_town" } },
    { key: "t_keeper", at: { site: "farm" }, unit: { def: "dog" } },
    { key: "t_pumpkin", count: 5, at: { area: "top_field" }, unit: { def: "pumpkin" } },
    { key: "t_pocket", at: { poi: "scarecrow", near: "farm" }, prop: { def: "chest", loot: [{ item: "apple", qty: 1 }] }, rect: { name: "t_scarecrow_field", w: 24, h: 24 }, mark: "t_scarecrow" },
    { key: "t_pocket_again", at: { poi: "scarecrow", near: "farm" }, prop: { def: "sign", talk: "sign_town" } },
  ];

  it("resolves marks, sites, areas and kinds of small place on every seed, and promises them in the contract", () => {
    expect(placementContract(rows)).toEqual({
      units: ["t_keeper", "t_pumpkin_1", "t_pumpkin_2", "t_pumpkin_3", "t_pumpkin_4", "t_pumpkin_5"],
      props: ["t_board", "t_pocket", "t_pocket_again"],
      marks: ["t_scarecrow"],
      rects: ["t_scarecrow_field"],
    });
    for (const seed of [3, 11]) {
      const bp = buildCounty(seed, 0, rows);
      const sk = countySkeleton(seed, 0);
      const prop = (key: string) => bp.props.find((p) => p.key === key)!;
      const unit = (key: string) => bp.units.find((u) => u.key === key)!;
      const near = (a: { cx: number; cy: number }, b: { cx: number; cy: number }, d: number): boolean => Math.hypot(a.cx - b.cx, a.cy - b.cy) <= d;

      expect(near(prop("t_board"), bp.marks.town_square, 14), `${seed}: board by the square`).toBe(true);
      const farm = sk.sites.find((s) => s.id === "farm")!;
      expect(near(unit("t_keeper"), { cx: farm.mx * MACRO + 8, cy: farm.my * MACRO + 8 }, 50), `${seed}: keeper at the farm`).toBe(true);

      const field = sk.areas.find((a) => a.id === "top_field");
      if (field) {
        for (let n = 1; n <= 5; n++) {
          const u = unit(`t_pumpkin_${n}`);
          expect(near(u, { cx: field.mx * MACRO + 8, cy: field.my * MACRO + 8 }, field.row.radius), `${seed}: pumpkin ${n} in the Top Field`).toBe(true);
          expect(u.phase, "a creature plays at the threat of its ground").toBe(3);
        }
      }

      // Both rows share one scarecrow; it is the nearest small place to the farm that is (or was made) one.
      expect(near(prop("t_pocket"), prop("t_pocket_again"), 14), `${seed}: one scarecrow, two things at it`).toBe(true);
      const r = bp.rects.t_scarecrow_field;
      expect(prop("t_pocket").cx).toBeGreaterThanOrEqual(r.cx);
      expect(prop("t_pocket").cx).toBeLessThan(r.cx + r.w);
      expect(bp.marks.t_scarecrow).toBeDefined();
      // Nothing placed is inside a wall.
      for (const key of ["t_board", "t_pocket", "t_pocket_again"]) expect((TILE_FLAGS[bp.tiles[prop(key).cy * bp.w + prop(key).cx]] & F_SOLID) === 0, key).toBe(true);
    }
  });
});
