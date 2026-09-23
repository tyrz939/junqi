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
import doorsJson from "@/data/doors.json";
import { hasZone } from "@/world/registry";

const DOORS = doorsJson as { zone: string; key: string; mark?: string; fromBelow?: boolean }[];
import { buildCounty, COUNTY_H, COUNTY_W, countySkeleton } from "@/world/county";
import { placementContract, type PlacementRow } from "@/world/placements";
import { at, MACRO, SKELETON_ROWS } from "@/world/skeleton";

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
  it("is 2000 x 2000, carries the story's names, and builds in a second or two", () => {
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
    // The density brief (2026-09-23): thousands, not hundreds. Mobs everywhere off the road, like a WoW zone.
    // (2500 to 9000 on the 3.6 x 2 km county. It is 2 km square since Sept 24, so the same per screen is 1400 to
    // 5000; the floor is set above that, because the square county carries more a screen than the old one did.)
    expect(wild.length).toBeGreaterThan(2000);
    expect(wild.length).toBeLessThan(5000);
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

  // The bug this exists for: `pad()` computed its bounds from a half-width, so an odd-sized place
  // looped from -4.5 and handed `set` a fractional index, which a typed array discards in silence.
  // Every small place in the county was drawing no ground at all, the well the lost-property book
  // names was a prop standing in open grass, and the whole suite stayed green because nothing here
  // had ever looked at the ground under a place. It does now.
  it("a small place has made ground under it, not just a prop standing in a field", () => {
    const made = new Set([Tile.Cobble, Tile.Dirt, Tile.Garden, Tile.Track, Tile.FloorWood, Tile.Rubble]);
    for (const seed of SEEDS) {
      const bp = county(seed);
      // The anchors the side quests name by name, and what the player is told is there.
      for (const id of ["halt_well", "halt_signpost", "halt_cart", "carters_cart"]) {
        const m = bp.marks[id];
        expect(m, `${id} on seed ${seed}`).toBeTruthy();
        // The place is drawn round the mark; the mark itself stands three cells south of its middle.
        let ground = 0;
        for (let j = -7; j <= 4; j++) {
          for (let i = -6; i <= 6; i++) {
            const t = bp.tiles[(m.cy + j) * bp.w + (m.cx + i)] as Tile;
            if (made.has(t)) ground++;
          }
        }
        expect(ground, `${id} on seed ${seed} stands on ${ground} cells of made ground`).toBeGreaterThan(24);
      }
    }
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
  // Two to four minutes on the 3.6 km county. The user, Sept 24: "walking distances can be a little shorter".
  // The station road is 520 to 850 m now, a minute or two, still a walk with things beside it.
  it("she steps off the train and can walk the lit road to Julie's gate in one to three minutes", () => {
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
    expect(minutes).toBeGreaterThan(1);
    expect(minutes).toBeLessThan(3);
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

describe("the places the story needs", () => {
  it("every anchor, dressed area, footpath end and chunk slot exists on every seed, by name", () => {
    for (const seed of SEEDS) {
      const bp = county(seed);
      const sk = countySkeleton(seed, bp.attempts - 1);
      expect(sk.anchors.map((a) => a.id)).toEqual(SKELETON_ROWS.anchors!.map((r) => r.id));
      for (const a of sk.anchors) {
        expect(bp.marks[a.id], `${seed}: mark ${a.id}`).toBeDefined();
        expect(bp.rects[a.id], `${seed}: rect ${a.id}`).toBeDefined();
      }
      for (const m of ["plot_nine", "plot_nine_stake", "allotment_shed", "top_field", "quarry_top", "quarry_adit", "hedge_stile_town", "hedge_stile_farm", "lost_property", "garden_book", "farm_door", "allen_door", "company_notice"]) {
        expect(bp.marks[m], `${seed}: mark ${m}`).toBeDefined();
      }
      for (const r of ["platform", "halt_approach", "plot_nine", "quarry_top", "stoop"]) expect(bp.rects[r], `${seed}: rect ${r}`).toBeDefined();
      // The first walk's anchors really are on the first walk: within a short step of its road.
      const first = sk.roads.find((r) => r.from === "station")!;
      const well = sk.anchors.find((a) => a.id === "halt_well")!;
      expect(Math.min(...first.cells.map((c) => Math.hypot((c % sk.w) - well.mx, Math.floor(c / sk.w) - well.my)))).toBeLessThan(3.5);
      // The lamps Pell counted stand in the dark.
      const lamp = sk.anchors.find((a) => a.id === "lamp_13")!;
      let lit = 0;
      for (let oy = -2; oy <= 2; oy++) for (let ox = -2; ox <= 2; ox++) lit += sk.road[at(lamp.mx + ox, lamp.my + oy)] & 2 ? 1 : 0;
      expect(lit, `${seed}: lamp_13 stands on a dark stretch (another road may pass close by)`).toBeLessThanOrEqual(3);
    }
  });
});

describe("doors into the dungeons", () => {
  it("a landmark grows its door the day its dungeon lands, and shows a blank face until then", () => {
    const bp = county(SEEDS[0]);
    for (const d of DOORS) {
      const door = bp.props.find((p) => p.key === d.key);
      expect(Boolean(door), `${d.key} exists iff zone ${d.zone} does`).toBe(hasZone(d.zone));
      if (!door) continue;
      // A way that opens from below (a manhole) is a mark and a cover, not a door down.
      expect(door.to).toEqual(d.fromBelow ? undefined : { zone: d.zone, mark: "entry" });
      if (d.mark) expect(bp.marks[d.mark], d.mark).toBeDefined();
      // Set into the face, with open ground in front of it to walk up to.
      expect(TILE_FLAGS[bp.tiles[(door.cy + 3) * bp.w + door.cx]] & F_SOLID).toBe(0);
    }
  });
});

describe("what Electric gives back to the county", () => {
  it("the longest dark roads get a relay box and a run of dead lamps, and sparking the box lights them for good", () => {
    // On every seed, not most: this is the Factory's largest reward, and a county without it is a
    // county where finding Electric changes nothing out of doors.
    for (const seed of [...SEEDS, 11, 77, 404]) {
      const bp = county(seed);
      const boxes = bp.props.filter((p) => p.def === "relay_box");
      const lamps = bp.props.filter((p) => p.def === "lamp_run");
      expect(boxes.length, `${seed}: relay boxes`).toBeGreaterThanOrEqual(1);
      expect(lamps.length, `${seed}: dead lamps`).toBeGreaterThanOrEqual(3 * boxes.length);
      for (const box of boxes) {
        // Every lamp the box promises is really out there, and it says so once it is done.
        const switched = (box.use ?? []).filter((a) => a.do === "switch").map((a) => (a.do === "switch" ? a.prop : ""));
        expect(switched.length).toBeGreaterThanOrEqual(3);
        for (const key of switched) expect(bp.props.some((p) => p.key === key), `${seed}: ${key}`).toBe(true);
        expect((box.use ?? []).some((a) => a.do === "flag" && a.flag.startsWith("lamps_"))).toBe(true);
      }
      // They stand dark: a lamp only lights once its box is sparked.
      for (const l of lamps) expect(l.on ?? false).toBe(false);
    }
  });
});
