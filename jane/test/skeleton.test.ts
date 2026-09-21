// The county skeleton: 200 seeds, every row checked on every one of them.
// This is the cheap half of worldgen testing (PLAN.md 4): the skeleton is a few
// milliseconds, so it can be proven on hundreds of seeds; full rasterisation is
// proven on a handful.

import { describe, expect, it } from "vitest";
import { buildSkeleton, countBridges, MAX_ATTEMPTS, Region, ROAD, ROAD_LIT, SKELETON_ROWS, threatAt, at, type Skeleton } from "@/world/skeleton";

// 64 seeds keep the suite quick (a county is ~140 ms). For a soak: SKELETON_SEEDS=1000 npx vitest run test/skeleton.test.ts
const COUNT = Number(process.env.SKELETON_SEEDS ?? 64);
const SEEDS = Array.from({ length: COUNT }, (_, i) => (i * 2654435761) >>> 0);
const all: Skeleton[] = SEEDS.map((seed) => buildSkeleton(seed));

describe("the county skeleton", () => {
  it("satisfies every row of sites.json on every seed, within the attempt budget", () => {
    const failed = all.filter((s) => !s.ok);
    const why = failed.slice(0, 5).map((s) => `${s.seed}: ${s.checks.filter((c) => !c.ok).map((c) => `${c.rule} (${c.detail})`).join("; ")}`);
    expect(why).toEqual([]);
    expect(Math.max(...all.map((s) => s.attempt))).toBeLessThan(MAX_ATTEMPTS);
    if (process.env.SKELETON_SEEDS) {
      const tries = all.map((s) => s.attempt + 1);
      const km = all.map((s) => s.roads.reduce((n, r) => n + r.metres, 0) / 1000);
      const mean = (list: number[]): string => (list.reduce((a, b) => a + b, 0) / list.length).toFixed(2);
      console.log(`SOAK ${all.length} seeds: attempts mean ${mean(tries)} worst ${Math.max(...tries)}; road km mean ${mean(km)} min ${Math.min(...km).toFixed(1)} max ${Math.max(...km).toFixed(1)}; patches mean ${mean(all.map((s) => s.areas.length))} of ${SKELETON_ROWS.areas.length}`);
    }
  });

  it("is the same county for the same seed, and a different one for a different seed", () => {
    const again = buildSkeleton(SEEDS[7]);
    expect(again.sites.map((s) => [s.id, s.mx, s.my])).toEqual(all[7].sites.map((s) => [s.id, s.mx, s.my]));
    expect(Array.from(again.road)).toEqual(Array.from(all[7].road));
    expect(Array.from(again.threat)).toEqual(Array.from(all[7].threat));
    const spots = new Set(all.map((s) => s.sites.map((x) => `${x.mx},${x.my}`).join("|")));
    expect(spots.size).toBe(all.length);
  });

  it("places every story site, in its region", () => {
    for (const s of all) {
      expect(s.sites.map((x) => x.id)).toEqual(SKELETON_ROWS.sites.map((r) => r.id));
      for (const x of s.sites) expect(["lowfields", "waters", "works"][s.region[at(x.mx, x.my)]]).toBe(x.row.region);
    }
  });

  it("is ten minutes across by road: the far shore is at least 3.6 km from the platform", () => {
    for (const s of all) {
      const c = s.checks.find((k) => k.rule.startsWith("lake_statue 3600"));
      expect(c?.ok, `${s.seed} ${c?.detail}`).toBe(true);
    }
  });

  it("puts the School on high ground north of the town, where it can loom", () => {
    for (const s of all) {
      const school = s.sites.find((x) => x.id === "school")!;
      const town = s.sites.find((x) => x.id === "town")!;
      expect(school.my).toBeLessThan(town.my);
      expect(s.height[at(school.mx, school.my)]).toBeGreaterThan(s.height[at(town.mx, town.my)] + 40);
    }
  });

  it("danger is a map, not a gradient: every region has patches above and below its base", () => {
    for (const s of all) {
      for (const r of [Region.Lowfields, Region.Waters, Region.Works]) {
        const here = s.areas.filter((a) => s.region[at(a.mx, a.my)] === r);
        expect(here.length, `seed ${s.seed} region ${r}`).toBeGreaterThanOrEqual(3);
      }
      // A haven is a haven, and the first walk is lit.
      const town = s.sites.find((x) => x.id === "town")!;
      expect(s.threat[at(town.mx, town.my)]).toBe(0);
      const first = s.roads.find((r) => r.from === "station")!;
      for (const c of first.cells) expect(s.road[c] & ROAD_LIT).toBeTruthy();
    }
  });

  it("night raises everything outside lamplight, more in the Works, and never a haven", () => {
    const s = all[3];
    const town = s.sites.find((x) => x.id === "town")!;
    expect(threatAt(s, town.mx, town.my, true)).toBe(0);
    let lit = -1;
    let dark = -1;
    for (let i = 0; i < s.road.length; i++) {
      if (!(s.road[i] & ROAD) || s.threat[i] === 0) continue;
      if (s.road[i] & ROAD_LIT && lit < 0) lit = i;
      if (!(s.road[i] & ROAD_LIT) && dark < 0) dark = i;
    }
    expect(threatAt(s, lit % s.w, Math.floor(lit / s.w), true)).toBe(s.threat[lit]);
    expect(threatAt(s, dark % s.w, Math.floor(dark / s.w), true)).toBeGreaterThan(s.threat[dark]);
  });

  it("crosses the river in few places, and is quick enough to run on a title screen", () => {
    for (const s of all) expect(countBridges(s)).toBeGreaterThanOrEqual(1);
    const t0 = performance.now();
    for (let i = 0; i < 10; i++) buildSkeleton(9000 + i);
    // Generous: the suite runs this beside the full-county tests, which keep every core busy.
    expect((performance.now() - t0) / 10).toBeLessThan(900);
  });
});
