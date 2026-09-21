import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import { ZONE_IDS } from "@/world";
import { BUILDERS, buildZone, CONTRACTS, GIVEN_KEYS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";

const catalog = buildCatalog();
const SEEDS = Array.from({ length: 25 }, (_, i) => 1000 + i * 7919);

describe("worldgen", () => {
  for (const zone of ZONE_IDS) {
    it(`${zone}: every seed passes the lock-and-key solver`, () => {
      // A county is seven million cells and about a second; the skeleton it is built from is
      // proven on many seeds in skeleton.test.ts, so the full thing is proven on a few.
      for (const seed of zone === "county" ? SEEDS.slice(0, 4) : SEEDS) {
        const bp = buildZone(zone, seed);
        const v = validateBlueprint(bp, catalog, CONTRACTS[zone], GIVEN_KEYS[zone]);
        expect(v.errors, `seed ${seed}`).toEqual([]);
        expect(bp.attempts, `seed ${seed} needed re-rolls`).toBeLessThanOrEqual(3);
      }
      // Four counties, each built and then solved a second time: more than the default five seconds allows.
    }, 60_000);
  }

  it("is a pure function of (zone, seed)", () => {
    for (const zone of ZONE_IDS) {
      const a = BUILDERS[zone](4242, 0);
      const b = BUILDERS[zone](4242, 0);
      expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(true);
      expect(a.props).toEqual(b.props);
      expect(a.units).toEqual(b.units);
    }
  });

  it("every door leads to a zone that exists, and to a mark that zone has", () => {
    // Zone ids are open strings now (a dungeon is a file dropped into world/zones/), so the
    // compiler no longer catches a misspelt destination. This does, for every zone there is.
    const built = new Map(ZONE_IDS.map((z) => [z, buildZone(z, 1000)]));
    for (const [zone, bp] of built) {
      for (const p of bp.props) {
        if (!p.to) continue;
        const dest = built.get(p.to.zone);
        expect(dest, `${zone}: prop ${p.key} leads to unknown zone "${p.to.zone}"`).toBeDefined();
        expect(dest!.marks[p.to.mark], `${zone}: prop ${p.key} leads to ${p.to.zone}, which has no mark "${p.to.mark}"`).toBeDefined();
      }
    }
  });

  it("different seeds give different counties with the same story keys", () => {
    const a = buildZone("county", 1);
    const b = buildZone("county", 2);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(false);
    for (const key of CONTRACTS.county.props) {
      expect(a.props.some((p) => p.key === key)).toBe(true);
      expect(b.props.some((p) => p.key === key)).toBe(true);
    }
  });

  it("Jane arrives on the platform at the county's edge, a real walk from the house; the yard gate is in sight of the dog", () => {
    for (const seed of SEEDS.slice(0, 4)) {
      const bp = buildZone("county", seed);
      const s = bp.marks.start;
      expect(TILE_FLAGS[bp.tiles[s.cy * bp.w + s.cx]] & F_SOLID).toBe(0);
      expect(s.cx).toBeLessThan(40);
      const dog = bp.units.find((u) => u.key === "dog")!;
      // Story.docx: she arrives at five and walks. Two to three minutes by road is 900 to 1350 m;
      // the crow's distance is shorter, and never trivial.
      expect(Math.hypot(dog.cx - s.cx, dog.cy - s.cy)).toBeGreaterThan(450);
      const gate = bp.marks.yard_gate;
      const stoop = bp.rects.stoop;
      const inside = gate.cx >= stoop.cx && gate.cy >= stoop.cy && gate.cx < stoop.cx + stoop.w && gate.cy < stoop.cy + stoop.h;
      expect(inside).toBe(false);
      expect(Math.hypot(dog.cx - gate.cx, dog.cy - gate.cy)).toBeLessThan(80);
    }
  }, 60_000); // four counties

  it("the solver really rejects a sealed gate", () => {
    const bp = buildZone("mine", 5);
    const broken = { ...bp, props: bp.props.map((p) => (p.key === "gate_boss" ? { ...p, keyTag: "no_such_key" } : p)) };
    const v = validateBlueprint(broken, catalog, CONTRACTS.mine, []);
    expect(v.ok).toBe(false);
    expect(v.errors.join(" ")).toMatch(/gate_boss|iron_knuckles/);
  });
});
