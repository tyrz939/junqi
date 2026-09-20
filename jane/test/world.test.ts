import { describe, expect, it } from "vitest";
import { buildCatalog } from "@/sim/catalog";
import { F_SOLID, TILE_FLAGS } from "@/sim/grid";
import { ZONE_IDS } from "@/sim/state";
import { BUILDERS, buildZone, CONTRACTS, GIVEN_KEYS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";

const catalog = buildCatalog();
const SEEDS = Array.from({ length: 25 }, (_, i) => 1000 + i * 7919);

describe("worldgen", () => {
  for (const zone of ZONE_IDS) {
    it(`${zone}: every seed passes the lock-and-key solver`, () => {
      for (const seed of SEEDS) {
        const bp = buildZone(zone, seed);
        const v = validateBlueprint(bp, catalog, CONTRACTS[zone], GIVEN_KEYS[zone]);
        expect(v.errors, `seed ${seed}`).toEqual([]);
        expect(bp.attempts, `seed ${seed} needed re-rolls`).toBeLessThanOrEqual(3);
      }
    });
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

  it("different seeds give different counties with the same story keys", () => {
    const a = buildZone("county", 1);
    const b = buildZone("county", 2);
    expect(Buffer.from(a.tiles).equals(Buffer.from(b.tiles))).toBe(false);
    for (const key of CONTRACTS.county.props) {
      expect(a.props.some((p) => p.key === key)).toBe(true);
      expect(b.props.some((p) => p.key === key)).toBe(true);
    }
  });

  it("Jane arrives on open ground, in sight of the house, not already on the stoop", () => {
    for (const seed of SEEDS) {
      const bp = buildZone("county", seed);
      const s = bp.marks.start;
      expect(TILE_FLAGS[bp.tiles[s.cy * bp.w + s.cx]] & F_SOLID).toBe(0);
      const stoop = bp.rects.stoop;
      const inside = s.cx >= stoop.cx && s.cy >= stoop.cy && s.cx < stoop.cx + stoop.w && s.cy < stoop.cy + stoop.h;
      expect(inside).toBe(false);
      const dog = bp.units.find((u) => u.key === "dog")!;
      expect(Math.hypot(dog.cx - s.cx, dog.cy - s.cy)).toBeLessThan(80);
    }
  });

  it("the solver really rejects a sealed gate", () => {
    const bp = buildZone("mine", 5);
    const broken = { ...bp, props: bp.props.map((p) => (p.key === "gate_boss" ? { ...p, keyTag: "no_such_key" } : p)) };
    const v = validateBlueprint(broken, catalog, CONTRACTS.mine, []);
    expect(v.ok).toBe(false);
    expect(v.errors.join(" ")).toMatch(/gate_boss|iron_knuckles/);
  });
});
