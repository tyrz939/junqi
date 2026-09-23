// Each step of the county's build throws dice of its own (Kit.within, stepDice): the rail, each road's
// beat, each point of the places' lattice, each macro cell's wildlife, each placement row. This is the
// proof. Build a seed, change ONE step, build it again, and everything the change did not touch is where
// it was: same key, same thing, same cell. Before Sept 24, 2026 every step drew on one stream, and a
// rail with gentler bends moved a cottage in the Lowfields a kilometre away and broke a quest.

import { afterEach, describe, expect, it } from "vitest";
import type { Blueprint } from "@/world/blueprint";
import { buildCounty, forgetCountySkeletons, RAIL_SHAPE } from "@/world/county";
import { PLACEMENTS, placementContract, type PlacementRow } from "@/world/placements";
import { buildSkeleton, MACRO } from "@/world/skeleton";
import { RAIL_COST } from "@/world/skeleton/rail";

const SEED = 2026;
/** Around what a change touched, how far its knock-on may reach (cells): a place moved off the line claims its new ground. */
const REACH = 64;

type Thing = { key: string; def: string; cx: number; cy: number };

/** Macro cells within REACH of any cell whose tile differs, or of any thing only one build has. */
function touched(a: Blueprint, b: Blueprint, extra: Thing[] = []): Uint8Array {
  const mw = Math.ceil(a.w / MACRO);
  const mh = Math.ceil(a.h / MACRO);
  const hit = new Uint8Array(mw * mh);
  for (let i = 0; i < a.tiles.length; i++) if (a.tiles[i] !== b.tiles[i]) hit[Math.floor(i / a.w / MACRO) * mw + Math.floor((i % a.w) / MACRO)] = 1;
  for (const t of extra) hit[Math.floor(t.cy / MACRO) * mw + Math.floor(t.cx / MACRO)] = 1;
  const r = Math.ceil(REACH / MACRO);
  const out = new Uint8Array(mw * mh);
  for (let y = 0; y < mh; y++) {
    for (let x = 0; x < mw; x++) {
      if (!hit[y * mw + x]) continue;
      for (let oy = -r; oy <= r; oy++) for (let ox = -r; ox <= r; ox++) if (x + ox >= 0 && y + oy >= 0 && x + ox < mw && y + oy < mh) out[(y + oy) * mw + x + ox] = 1;
    }
  }
  return out;
}

const sig = (t: Thing): string => `${t.key}|${t.def}|${t.cx},${t.cy}`;

/** Everything of `a` outside the touched ground, missing or moved in `b` (and the other way about). */
function moved(a: Blueprint, b: Blueprint, area: Uint8Array): { outside: number; moved: string[] } {
  const mw = Math.ceil(a.w / MACRO);
  const inside = (t: Thing): boolean => area[Math.floor(t.cy / MACRO) * mw + Math.floor(t.cx / MACRO)] === 1;
  const all = (bp: Blueprint): Thing[] => [...bp.props, ...bp.units];
  const inB = new Set(all(b).map(sig));
  const inA = new Set(all(a).map(sig));
  const out: string[] = [];
  let outside = 0;
  for (const t of all(a)) {
    if (inside(t)) continue;
    outside++;
    if (!inB.has(sig(t))) out.push(`gone: ${sig(t)}`);
  }
  for (const t of all(b)) if (!inside(t) && !inA.has(sig(t))) out.push(`new: ${sig(t)}`);
  return { outside, moved: out };
}

/** Things one build has and the other has not, by signature. */
function differ(a: Blueprint, b: Blueprint): Thing[] {
  const inB = new Set([...b.props, ...b.units].map(sig));
  const inA = new Set([...a.props, ...a.units].map(sig));
  return [...[...a.props, ...a.units].filter((t) => !inB.has(sig(t))), ...[...b.props, ...b.units].filter((t) => !inA.has(sig(t)))];
}

const base = buildCounty(SEED, 0);

describe("each step of the county throws its own dice", () => {
  afterEach(() => {
    RAIL_SHAPE.bend = 32;
  });

  it("re-laying the rail's bends moves nothing away from the line", () => {
    RAIL_SHAPE.bend = 12;
    const b = buildCounty(SEED, 0);
    const area = touched(base, b);
    const share = area.reduce((n, v) => n + v, 0) / area.length;
    // The change was real, and it was local.
    expect(differ(base, b).length, "the rail change moved something").toBeGreaterThan(0);
    expect(share, "share of the county the rail change touched").toBeLessThan(0.2);
    const m = moved(base, b, area);
    expect(m.outside).toBeGreaterThan(5000);
    expect(m.moved, "things away from the line that moved").toEqual([]);
    // The stories found the same places, and the quests' rows stand where they stood.
    expect(b.stories).toEqual(base.stories);
    const promised = placementContract();
    for (const key of [...promised.props, ...promised.units]) {
      const t0 = [...base.props, ...base.units].find((t) => t.key === key);
      const t1 = [...b.props, ...b.units].find((t) => t.key === key);
      expect(t1 && sig(t1), key).toBe(t0 && sig(t0));
    }
    // Wildlife: every creature of the first build off the touched ground is in the second, on its cell.
    const mw = Math.ceil(base.w / MACRO);
    const wild = base.units.filter((u) => u.phase !== undefined && area[Math.floor(u.cy / MACRO) * mw + Math.floor(u.cx / MACRO)] === 0);
    expect(wild.length).toBeGreaterThan(100);
    const there = new Set(b.units.map(sig));
    expect(wild.filter((u) => !there.has(sig(u))).map(sig)).toEqual([]);
  });

  it("one more placement row moves nothing but what it put down", () => {
    // Three crates spread about a patch: the row throws dice for where each one goes, early in the build,
    // before the places, the scatter and the wildlife. On one shared stream that reshuffled all of them.
    const row: PlacementRow = { key: "zz_stream_probe", count: 3, spread: true, at: { area: "top_field", within: 30 }, prop: { def: "crate" } };
    const b = buildCounty(SEED, 0, [...PLACEMENTS, row]);
    const probes = b.props.filter((p) => p.key.startsWith(row.key));
    expect(probes.length, "the new row placed its crates").toBe(3);
    // The crates, and nothing else in the county but what wanted their cells: a creature whose spot in its
    // own macro cell a crate now stands on, say. Nothing a macro cell or more from any crate.
    const d = differ(base, b).filter((t) => !t.key.startsWith(row.key));
    const far = d.filter((t) => probes.every((p) => Math.max(Math.abs(p.cx - t.cx), Math.abs(p.cy - t.cy)) > 2 * MACRO));
    expect(far.map(sig)).toEqual([]);
    expect(d.length, "things the crates displaced").toBeLessThan(4);
    expect(b.stories).toEqual(base.stories);
  });

  it("re-routing the rail in the skeleton moves the small places along it, and nothing else", () => {
    const a = buildSkeleton(SEED);
    RAIL_COST.turn = 0.5;
    try {
      const b = buildSkeleton(SEED);
      expect(b.attempt, "same attempt, so the same terrain and sites").toBe(a.attempt);
      expect(b.rail).not.toEqual(a.rail);
      expect(b.sites).toEqual(a.sites);
      expect(b.roads).toEqual(a.roads);
      expect(b.areas).toEqual(a.areas);
      // The places the story needs by name (Mrs Allen's cottage, the scarecrows, the lamps) stand where they stood.
      expect(b.anchors).toEqual(a.anchors);
      // The rolled small places: those well away from both lines are the same places, of the same kind.
      const near = (p: { mx: number; my: number }): boolean => [...a.rail, ...b.rail].some((c) => Math.abs((c % a.w) - p.mx) <= 8 && Math.abs(Math.floor(c / a.w) - p.my) <= 8);
      const key = (p: { kind: string; mx: number; my: number }): string => `${p.kind}@${p.mx},${p.my}`;
      const far = a.pois.filter((p) => !near(p));
      expect(far.length).toBeGreaterThan(40);
      const inB = new Set(b.pois.map(key));
      expect(far.filter((p) => !inB.has(key(p))).map(key)).toEqual([]);

      // And the county built on it: the stories' places, and everything away from the two lines, unmoved.
      forgetCountySkeletons();
      const county = buildCounty(SEED, 0);
      const area = touched(base, county);
      const m = moved(base, county, area);
      expect(m.outside).toBeGreaterThan(5000);
      expect(m.moved, "things away from the line that moved").toEqual([]);
      expect(county.stories).toEqual(base.stories);
    } finally {
      RAIL_COST.turn = 6;
      forgetCountySkeletons();
    }
  });
});
