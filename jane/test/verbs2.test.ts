// The engine capabilities the five dungeons after the mine all lean on (DUNGEONS.md 4.7), each
// with a test that fails without it:
//   E1   action `if`, in the runner, the row checks and the solver
//        the STATEFUL FLOOD, the `state` edge and the generated control, on a test dungeon
//   E7   schools blast and shock, `touch`, Explosion, Grow, Spark, and four effects
//   E8   litAt, `sight: "lit"`, `shunsLight`, day-only light, Grow wants light
//   E9   action `send`, saved mid-walk
//   E10  phases[n].onEnter
//   E11  a patrol point with a dwell
//   E12  show, lock and a solid fill never land on a unit
//   E13  action `reveal`, and the mine's wall notice
// and the small fixes that waited for the sim to be free: the prompt a row asks for, `dev kill`
// through walls, the solver's needless floods, the dog's pointer, the mine's adit.
//
// Test rows (a lamp, a sentry, a shade, a boss) are put into the catalog here and nowhere else.
// The test dungeon's mission and its one room of its own are written below: it is in nobody's
// data folder, so it is not a zone and the game cannot reach it.

import { describe, expect, it } from "vitest";
import { conditionsMet, runActions } from "@/sim/actions";
import { actionRowErrors, buildCatalog, validateCatalog, type Catalog, type PropDef, type UnitDef } from "@/sim/catalog";
import { SAVE_VERSION } from "@/sim/constants";
import { tryCast } from "@/sim/combat";
import { cellOf, centre, Tile } from "@/sim/grid";
import { focusOf } from "@/sim/interact";
import { lampsLit, litAt } from "@/sim/light";
import { lineOfSight } from "@/sim/los";
import { addProp, addUnit, asPlayer, removeUnit } from "@/sim/runtime";
import { cloneState, decodeSave, encodeSave, hashState } from "@/sim/save";
import { NO_INPUT, Sim } from "@/sim/sim";
import { SCHOOLS, type Action, type Prop, type Unit } from "@/sim/state";
import { resistFactor, speedFactor } from "@/sim/status";
import { createUnit, distance, maxHp, placeUnit } from "@/sim/units";
import { createZoneState, fogSeen, primeBlueprint } from "@/sim/zones";
import type { Blueprint, PropSpawn } from "@/world/blueprint";
import { checkDungeon, contractOf, lintDef, solveOptionsOf } from "@/world/dungeon/checks";
import { buildDungeon, infoOf } from "@/world/dungeon/generate";
import { addTemplates } from "@/world/dungeon/pools";
import { lintRoom, parseRoom } from "@/world/dungeon/room";
import type { DungeonDef, MissionEdge } from "@/world/dungeon/types";
import { buildZone } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkTo, yardCatalog } from "./bot";

// --- test rows -----------------------------------------------------------------------------

function testCatalog(): Catalog {
  const c = yardCatalog();
  const lamp: PropDef = { name: "Test Lamp", sprite: "torch", w: 1, h: 1, solid: false, blockLos: false, lightWhenOn: true, light: { radius: 60, color: "#ffffff", flicker: 0 } };
  c.props.test_lamp = lamp;
  c.props.test_cold_lamp = { ...lamp, light: { radius: 60, color: "#6aa8ff", flicker: 0, cold: true } };
  c.props.test_night_lamp = { ...lamp, lightWhenOn: false, nightOnly: true };
  c.props.test_sunbeam = { ...lamp, lightWhenOn: false, dayOnly: true, flat: true };
  c.props.test_bud = { name: "Bud", sprite: "herb", w: 1, h: 1, solid: false, blockLos: false, flat: true, answers: "grow" };
  c.props.test_rubble = { name: "Rubble", sprite: "car_wreck", w: 5, h: 3, solid: true, blockLos: true, answers: "blast" };
  c.props.test_exhibit = { name: "Exhibit", sprite: "barrel", w: 2, h: 2, solid: true, blockLos: false };
  const skeleton = c.units.skeleton;
  c.units.test_sentry = { ...structuredClone(skeleton), sight: "lit" };
  c.units.test_shade = { ...structuredClone(skeleton), shunsLight: true };
  c.units.test_machine = { ...structuredClone(skeleton), resist: { shock: -0.5, physical: 0.8 } };
  const boss: UnitDef = {
    ...structuredClone(skeleton),
    strength: 200,
    autoRegen: true,
    book: ["melee"],
    phases: [
      { hpBelow: 0.75, book: ["melee_fast"], run: 2, onEnter: [{ do: "flag", flag: "test_boss_phases", add: 1 }, { do: "status", effect: "stoneskin" }] },
      { hpBelow: 0.5, book: ["melee_stun"], onEnter: [{ do: "flag", flag: "test_boss_phases", add: 10 }] },
    ],
  };
  c.units.test_boss = boss;
  expect(validateCatalog(c), "the test rows are rows like any other").toEqual([]);
  return c;
}

const catalog = testCatalog();

function putProp(sim: Sim, def: string, key: string, cx: number, cy: number, extra: Partial<Prop> = {}): Prop {
  const d = sim.catalog.props[def];
  const p: Prop = {
    id: sim.state.nextId++, key, def, cx, cy, solid: d.gate ? false : d.solid, hidden: false, locked: false, used: false, on: false,
    keyTag: "", to: null, loot: null, use: null, release: null, needs: null, talk: "", label: "", nightLock: "", awake: true, ...extra,
  };
  addProp(sim, p);
  return p;
}

function putUnit(sim: Sim, def: string, key: string, cx: number, cy: number): Unit {
  const u = createUnit(sim.state, sim.catalog, def, key, centre(cx), centre(cy));
  addUnit(sim, u);
  return u;
}

/**
 * The mine's shaft hall with nobody in it and every light out: a big dark room to put things
 * in. Returns a sim and a cell in the middle of the hall with open floor round it.
 */
function quietHall(seed = 12): { sim: Sim; cx: number; cy: number } {
  const sim = simIn(catalog, "mine", seed);
  for (const u of [...sim.zone.units]) if (u !== sim.player) removeUnit(sim, u);
  for (const p of sim.zone.props) if (sim.catalog.props[p.def].light) p.hidden = true;
  const r = sim.rt.bp.rects.mine_core_room;
  // A clear patch of 25 x 9 cells, found the same way every time.
  for (let cy = r.cy + 5; cy < r.cy + r.h - 5; cy++) {
    for (let cx = r.cx + 13; cx < r.cx + r.w - 13; cx++) {
      let clear = true;
      for (let y = cy - 4; y <= cy + 4 && clear; y++) for (let x = cx - 12; x <= cx + 12 && clear; x++) clear = sim.rt.grid.free(x, y);
      if (clear) {
        placeUnit(sim, sim.player, centre(cx), centre(cy));
        idle(sim, 2);
        return { sim, cx, cy };
      }
    }
  }
  throw new Error("no clear patch in the shaft hall");
}

/** A blueprint a few cells across, drawn from rows of text: `#` wall, anything else floor. */
function tinyBlueprint(rows: string[], props: PropSpawn[], marks: Record<string, [number, number]>): Blueprint {
  const w = rows[0].length;
  const h = rows.length;
  const tiles = new Uint8Array(w * h);
  rows.forEach((row, y) => Array.from(row).forEach((ch, x) => (tiles[y * w + x] = ch === "#" ? Tile.Wall : Tile.Floor)));
  return {
    zone: "test_tiny", name: "Tiny", w, h, tiles, units: [], props, indoor: true, ambient: 0.5, attempts: 1, rects: {},
    marks: Object.fromEntries(Object.entries(marks).map(([k, [cx, cy]]) => [k, { cx, cy }])),
  };
}

const NOTHING = { units: [], props: [], marks: [], rects: [] };

// --- E1 ------------------------------------------------------------------------------------

describe("E1: if", () => {
  it("runs `then` when its conditions hold and `else` when they do not, for whoever is acting", () => {
    const { sim } = quietHall();
    const list: Action[] = [
      { do: "if", when: [{ if: "flag", flag: "test_power" }], then: [{ do: "flag", flag: "test_then", add: 1 }], else: [{ do: "flag", flag: "test_else", add: 1 }] },
    ];
    runActions(sim, list, sim.player.id);
    expect([sim.state.flags.test_then ?? 0, sim.state.flags.test_else ?? 0]).toEqual([0, 1]);
    sim.state.flags.test_power = 1;
    runActions(sim, list, sim.player.id);
    expect([sim.state.flags.test_then, sim.state.flags.test_else]).toEqual([1, 1]);
    // No `else`: nothing happens, and nothing throws.
    runActions(sim, [{ do: "if", when: [{ if: "flag", flag: "no_such_flag" }], then: [{ do: "flag", flag: "test_then", add: 1 }] }], sim.player.id);
    expect(sim.state.flags.test_then).toBe(1);

    // `hasItem` is hers, not the party's: the same list does two things for two people.
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    sim.command(0, { t: "dev", dev: { op: "give", item: "wood", qty: 1 } });
    const holding: Action[] = [{ do: "if", when: [{ if: "hasItem", item: "wood" }], then: [{ do: "flag", flag: "test_has", add: 1 }], else: [{ do: "flag", flag: "test_has_not", add: 1 }] }];
    asPlayer(sim, sim.state.players[0], () => runActions(sim, holding, sim.state.players[0].unitId));
    asPlayer(sim, sim.state.players[seat], () => runActions(sim, holding, sim.state.players[seat].unitId));
    expect([sim.state.flags.test_has, sim.state.flags.test_has_not]).toEqual([1, 1]);
    expect(asPlayer(sim, sim.state.players[seat], () => conditionsMet(sim, [{ if: "hasItem", item: "wood" }]))).toBe(false);
  });

  it("is checked at boot all the way down: a bad row inside a branch is still a bad row", () => {
    const c = buildCatalog();
    expect(actionRowErrors(c, "here", [{ do: "if", when: [{ if: "flag", flag: "x" }], then: [{ do: "learn", spell: "icebolt" }] }])).toEqual([]);
    const nested: Action[] = [{ do: "if", when: [{ if: "knows", spell: "no_such_spell" }], then: [{ do: "give", item: "no_such_item" }], else: [{ do: "send", unit: "x", to: "y", then: [{ do: "status", effect: "no_such_effect" }] }] }];
    const errors = actionRowErrors(c, "here", nested).join("\n");
    expect(errors).toMatch(/unknown spell "no_such_spell"/);
    expect(errors).toMatch(/unknown item "no_such_item"/);
    expect(errors).toMatch(/unknown effect "no_such_effect"/);
    expect(actionRowErrors(c, "here", [{ do: "if", when: [], then: [] }]).join("\n")).toMatch(/if needs a "when" and a "then"/);
  });

  it("the solver reads both branches: a `then` waits for its flag, and a lever that works once does not get a second pull", () => {
    const c = buildCatalog();
    const room = ["#############", "#...........#", "#...........#", "#...........#", "#############"];
    const gate: PropSpawn = { key: "g", def: "gate_v", cx: 8, cy: 1, locked: true };
    const opens: Action[] = [{ do: "if", when: [{ if: "flag", flag: "test_power" }], then: [{ do: "unlock", prop: "g" }], else: [{ do: "toast", text: "Nothing happens." }] }];
    const sw = (def: string): PropSpawn => ({ key: "sw", def, cx: 4, cy: 1, use: opens });
    const power: PropSpawn = { key: "power", def: "lever", cx: 6, cy: 1, use: [{ do: "flag", flag: "test_power" }] };
    const marks: Record<string, [number, number]> = { start: [1, 2], far: [10, 2] };
    const solve = (props: PropSpawn[], withhold?: string[]) => validateBlueprint(tinyBlueprint(room, props, marks), c, NOTHING, [], { withhold: { props: withhold } }).errors;

    // The switch is reached before the power is on. It can be pulled again, so its `then` is kept and runs when the flag is set.
    expect(solve([gate, sw("lever"), power])).toEqual([]);
    expect(solve([gate, sw("lever"), power], ["power"])).toEqual(['mark "far" is unreachable']);
    // `else` alone opens nothing.
    expect(solve([gate, sw("lever")])).toEqual(['mark "far" is unreachable']);
    // A lever that works once (the hoist's) was spent on the `else`: the proof must not assume a second pull.
    expect(solve([gate, sw("hoist_lever"), power])).toEqual(['mark "far" is unreachable']);
    // `else` is a branch too: what it unlocks is unlocked.
    const backwards: PropSpawn = { key: "sw", def: "lever", cx: 4, cy: 1, use: [{ do: "if", when: [{ if: "flag", flag: "never_set" }], then: [], else: [{ do: "unlock", prop: "g" }] }] };
    expect(solve([gate, backwards])).toEqual([]);
  });
});

// --- the stateful flood ----------------------------------------------------------------------

const SWITCH_ROOM = `
id      lab.switch.a
pool    lab.switch
bays    1x1
turn    0 180 mirror
doors   n1 optional, s1 optional, w1 optional, e1 optional
needs   -
grants  lever:main
blocks  -
coop    -
heat    0
grid
###########nnn###########
#..........___..........#
#.......................#
#.......................#
#....L..................#
#.......................#
#...............p...q...#
#.......................#
w_....................._e
w_....................._e
w_....................._e
#.......................#
#.......................#
#.................u.....#
#.......................#
#.......................#
#.......................#
#..........___..........#
###########sss###########
legend
n door n1
s door s1
w door w1
e door e1
L lever:main
u unit:moth
p mark perch1
q mark perch2
`;
const switchRoom = parseRoom(SWITCH_ROOM, "test/lab.switch.a");
addTemplates([switchRoom]);

/**
 * A building of three rooms and one breaker. The vault's door is passable only when the lights
 * are on, and they start off. `toSwitch` is how the room with the breaker is reached.
 */
function lab(id: string, toSwitch: MissionEdge["kind"]): DungeonDef {
  return {
    id, name: "Test Lab", phase: 1, tiles: { floor: "CaveFloor", wall: "CaveWall", alt: [] }, indoor: true, ambient: 0.3,
    lattice: { cols: 3, rows: 3 }, givenVerbs: [], givenKeys: [],
    states: [{ id: "lights", values: ["lit", "dark"], initial: "dark", flag: "lab_lit" }],
    nodes: [
      {
        id: "entry", kind: "entrance", order: 0, critical: true, pool: "mine.entry", heat: 0, demands: [], grants: [],
        holds: [{ socket: "exit:main", prop: "door", to: { zone: "county", mark: "mine_mouth" } }],
        binds: [{ from: "entry", as: "entry", what: "mark" }],
      },
      {
        id: "switch", kind: "puzzle", order: 1, critical: true, pool: "lab.switch", heat: 0, demands: [], grants: [{ state: "lights" }],
        holds: [
          { socket: "lever:main", prop: "lever", controls: "lights", becomes: { lit: [{ do: "toast", text: "The lights come on." }], dark: [{ do: "toast", text: "The lights go out." }] } },
          // Something harmless on a round of two perches, resting on the first: a holding may carry a patrol.
          { socket: "unit:moth", unit: "dog", patrol: [{ mark: "perch1", dwell: 90 }, { mark: "perch2" }] },
        ],
        binds: [{ from: "lever:main", as: "lab_breaker", what: "prop" }],
      },
      {
        id: "vault", kind: "reward", order: 2, critical: true, pool: "mine.store", heat: 0, demands: [], grants: [],
        holds: [{ socket: "chest:main", loot: [{ item: "gold_bar", qty: 1 }] }],
        binds: [{ from: "chest:main", as: "lab_chest", what: "prop" }, { from: "room", as: "lab_vault", what: "rect" }],
      },
    ],
    edges: [
      { from: "entry", to: "switch", kind: toSwitch },
      { from: "entry", to: "vault", kind: { t: "state", var: "lights", is: "lit", gateAs: "lab_shutter" } },
    ],
    budget: { sideRooms: [0, 0], critPathCells: [0, 99999], restToBossCells: 99999, baseHeat: 0, enemies: {} },
    dress: {},
    fallback: [
      { node: "entry", template: "mine.entry.a", bay: [1, 2], turn: 0, mirror: false },
      { node: "switch", template: "lab.switch.a", bay: [0, 2], turn: 0, mirror: false },
      { node: "vault", template: "mine.store.a", bay: [1, 1], turn: 0, mirror: false },
    ],
  };
}

function buildLab(def: DungeonDef, seed: number): Blueprint {
  for (let attempt = 0; attempt < 12; attempt++) {
    const bp = buildDungeon(def, seed, attempt);
    if ((infoOf(bp)?.errors ?? ["?"]).length === 0) return bp;
  }
  throw new Error("the test lab would not lay out");
}

describe("the stateful flood: a building with a breaker", () => {
  const c = buildCatalog();
  const open = lab("lab", { t: "open" });

  it("the test room is a room like any other, and the mission lints clean", () => {
    expect(lintRoom(switchRoom)).toEqual([]);
    expect(lintDef(open, c)).toEqual([]);
    const noControl = structuredClone(open);
    noControl.nodes[1].holds = [];
    noControl.nodes[1].grants = [];
    expect(lintDef(noControl, c).join("\n")).toMatch(/state "lights" has no control/);
  });

  it("generator: a `state` edge is a gate nobody has a key for, shut in the state the building starts in, and the control's list drives it both ways", () => {
    for (const seed of [1, 2, 3, 4, 5]) {
      const bp = buildLab(open, seed);
      const shutter = bp.props.find((p) => p.key === "lab_shutter")!;
      expect(["gate_h", "gate_v"]).toContain(shutter.def);
      expect([shutter.locked, shutter.keyTag]).toEqual([true, undefined]);
      const breaker = bp.props.find((p) => p.key === "lab_breaker")!;
      expect(breaker.use).toEqual([
        {
          do: "if",
          when: [{ if: "flag", flag: "lab_lit" }],
          then: [{ do: "flag", flag: "lab_lit", value: 0 }, { do: "lock", prop: "lab_shutter" }, { do: "toast", text: "The lights go out." }],
          else: [{ do: "flag", flag: "lab_lit", value: 1 }, { do: "unlock", prop: "lab_shutter" }, { do: "toast", text: "The lights come on." }],
        },
      ]);
      expect(infoOf(bp)!.controls).toEqual([{ state: "lights", prop: "lab_breaker" }]);
      // E11 through the generator: the holding's patrol is the template's marks, in zone cells, with their dwell.
      const moth = bp.units.find((u) => u.key === "lab_switch_unit_moth")!;
      expect(moth.patrol).toEqual([[bp.marks.lab_switch_perch1.cx, bp.marks.lab_switch_perch1.cy, 90], [bp.marks.lab_switch_perch2.cx, bp.marks.lab_switch_perch2.cy, 0]]);
    }
  });

  it("solver: accepts it, reaches both states, and rejects the same layout with the control withheld", () => {
    for (const seed of [1, 2, 3]) {
      const bp = buildLab(open, seed);
      const opts = { ...solveOptionsOf(open), trace: true };
      const whole = validateBlueprint(bp, c, contractOf(open), [], opts);
      expect(whole.errors).toEqual([]);
      expect(whole.trace!.layers).toEqual([0, 1]);
      const without = validateBlueprint(bp, c, contractOf(open), [], { ...opts, withhold: { props: ["lab_breaker"] } });
      expect(without.ok).toBe(false);
      expect(without.errors.join("\n")).toMatch(/prop "lab_chest" is unreachable/);
      expect(without.trace!.layers).toEqual([0]);
      // And C1 proves the lock the same way, by name.
      expect(checkDungeon(bp, c).filter((e) => e.startsWith("C1") || e.startsWith("solver"))).toEqual([]);
    }
  });

  it("solver: you are where you stood when you pulled it. A breaker behind a dark-only door leaves her shut in with the lights on", () => {
    const trap = lab("lab", { t: "state", var: "lights", is: "dark", gateAs: "lab_dark_door" });
    const bp = buildLab(trap, 1);
    const v = validateBlueprint(bp, c, contractOf(trap), [], { ...solveOptionsOf(trap), trace: true });
    // Both states are reached, and every cell of the vault's corridor is open in one of them:
    // a flood that started each state from the front door would call this finished.
    expect(v.trace!.layers).toEqual([0, 1]);
    expect(v.errors.join("\n")).toMatch(/prop "lab_chest" is unreachable/);
    const vault = bp.rects.lab_vault;
    expect(v.trace!.firstSeen[(vault.cy + 2) * bp.w + vault.cx + 2]).toBe(-1);
  });

  it("a state that looks different depending on how it was reached is refused by name", () => {
    const bp = buildLab(open, 1);
    const breaker = bp.props.find((p) => p.key === "lab_breaker")!;
    const lopsided: Blueprint = {
      ...bp,
      props: bp.props.map((p) => (p === breaker ? { ...p, use: [{ do: "if", when: [{ if: "flag", flag: "lab_lit" }], then: [{ do: "flag", flag: "lab_lit", value: 0 }], else: [{ do: "flag", flag: "lab_lit", value: 1 }, { do: "unlock", prop: "lab_shutter" }] }] } : p)),
    };
    expect(validateBlueprint(lopsided, c, contractOf(open), [], solveOptionsOf(open)).errors.join("\n")).toMatch(/"lab_shutter" is left differently/);
  });

  it("in play: the shutter is down, the breaker lifts it, the breaker drops it again, and it never drops on her (E12)", () => {
    // Played under the mine's name, since only registered zones tick. The rows are the lab's.
    const played = lab("mine", { t: "open" });
    const seed = 990001;
    primeBlueprint("mine", seed, buildLab(played, 2));
    const sim = simIn(catalog, "mine", seed);
    const p = sim.player;
    const shutter = sim.rt.propsByKey.get("lab_shutter")!;
    const vault = sim.rt.bp.rects.lab_vault;
    const intoVault = (): boolean => walkTo(sim, centre(vault.cx + 3), centre(vault.cy + 3), 6, 60 * 40);
    expect([shutter.locked, shutter.solid]).toEqual([true, true]);
    expect(intoVault()).toBe(false);

    const pull = (): void => {
      const lever = sim.rt.propsByKey.get("lab_breaker")!;
      placeUnit(sim, p, centre(lever.cx), centre(lever.cy + 1) + 2);
      p.facing = 3;
      idle(sim, 1);
      sim.command({ t: "use" });
      idle(sim, 2);
    };
    pull();
    expect(sim.state.flags.lab_lit).toBe(1);
    expect([shutter.locked, shutter.solid]).toEqual([false, false]);
    expect(intoVault()).toBe(true);

    // Dark again, with someone standing in the doorway: the shutter comes down and she is beside it, not in it.
    const friend = putUnit(sim, "dog", "test_friend", shutter.cx, shutter.cy);
    pull();
    expect(sim.state.flags.lab_lit).toBe(0);
    expect([shutter.locked, shutter.solid]).toEqual([true, true]);
    const def = sim.catalog.props[shutter.def];
    const inGate = (u: Unit): boolean => cellOf(u.x) >= shutter.cx && cellOf(u.x) < shutter.cx + def.w && cellOf(u.y) >= shutter.cy && cellOf(u.y) < shutter.cy + def.h;
    expect(inGate(friend)).toBe(false);
    expect(sim.rt.grid.solid(cellOf(friend.x), cellOf(friend.y))).toBe(false);
    expect(distance(friend.x, friend.y, centre(shutter.cx), centre(shutter.cy))).toBeLessThan(24);
  });
});

// --- E7 --------------------------------------------------------------------------------------

describe("E7: blast, shock, touch, and the three spells", () => {
  it("the schools exist, the rows are in, and staggered doubles them too", () => {
    expect(SCHOOLS).toEqual(expect.arrayContaining(["blast", "shock"]));
    const c = buildCatalog();
    expect([c.spells.explosion.school, c.spells.explosion.touch, c.spells.explosion.splash]).toEqual(["blast", 28, { radius: 28, div: 1 }]);
    expect([c.spells.spark.school, c.spells.spark.effect]).toEqual(["shock", "jolted"]);
    expect([c.spells.grow.kind, c.spells.grow.world]).toEqual(["world", "grow"]);
    expect(c.effects.staggered.resist).toMatchObject({ blast: 2, shock: 2 });
    for (const id of ["dazzled", "jolted", "dusted", "softened"]) expect(c.effects[id], id).toBeDefined();
  });

  it("Explosion: everything near the burst takes all of it, and its touch reaches the middle of a pile an ordinary bolt would miss", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "learn", spell: "explosion" } });
    // Two skeletons a cell apart, six cells east of her.
    const a = putUnit(sim, "skeleton", "test_a", cx + 6, cy);
    const b = putUnit(sim, "skeleton", "test_b", cx + 7, cy + 1);
    a.strength = b.strength = 400;
    a.hp = b.hp = maxHp(a);
    sim.setAim(1, 0);
    expect(tryCast(sim, p, "explosion", { x: 1, y: 0 })).toBe("castSuccessful");
    idle(sim, 60);
    expect(maxHp(a) - a.hp).toBeGreaterThan(0);
    expect(maxHp(b) - b.hp, "the splash is the whole blow, not a fifth of it").toBe(maxHp(a) - a.hp);

    // A 5 x 3 pile, hit on the end of its long side: 20 px from its middle. Blast's touch is 28.
    removeUnit(sim, a);
    removeUnit(sim, b);
    const rubble = putProp(sim, "test_rubble", "test_rubble", cx - 2, cy + 3, { use: [{ do: "hide", prop: "test_rubble" }] });
    idle(sim, 1);
    const shoot = (): void => {
      placeUnit(sim, p, centre(cx - 2), centre(cy));
      p.cooldowns = {};
      p.gcd = 0;
      p.mp = 999;
      expect(tryCast(sim, p, "explosion", { x: 0, y: 1 })).toBe("castSuccessful");
      idle(sim, 40);
    };
    const touch = sim.catalog.spells.explosion.touch;
    sim.catalog.spells.explosion.touch = undefined;
    shoot();
    expect(rubble.hidden, "with the old fixed 14 px the blast does not find it").toBe(false);
    sim.catalog.spells.explosion.touch = touch;
    shoot();
    expect(rubble.hidden).toBe(true);
  });

  it("Spark jolts what is weak to shock and nothing else; softened takes resists away and leaves weaknesses", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "learn", spell: "spark" } });
    const machine = putUnit(sim, "test_machine", "test_machine", cx + 5, cy);
    machine.strength = 400;
    machine.hp = maxHp(machine);
    expect(tryCast(sim, p, "spark", { x: 1, y: 0 })).toBe("castSuccessful");
    idle(sim, 12);
    expect(machine.hp).toBeLessThan(maxHp(machine));
    expect(machine.statuses.map((s) => s.effect)).toContain("jolted");
    idle(sim, 40);
    expect(machine.statuses.map((s) => s.effect), "half a second").not.toContain("jolted");

    const plain = putUnit(sim, "skeleton", "test_plain", cx + 5, cy + 2);
    plain.strength = 400;
    plain.hp = maxHp(plain);
    placeUnit(sim, p, centre(cx), centre(cy + 2));
    p.cooldowns = {};
    p.gcd = 0;
    expect(tryCast(sim, p, "spark", { x: 1, y: 0 })).toBe("castSuccessful");
    idle(sim, 12);
    expect(plain.hp).toBeLessThan(maxHp(plain));
    expect(plain.statuses.map((s) => s.effect)).not.toContain("jolted");

    expect(resistFactor(sim, machine, "physical")).toBeCloseTo(0.2);
    expect(resistFactor(sim, machine, "shock")).toBeCloseTo(1.5);
    runActions(sim, [{ do: "status", effect: "softened" }], machine.id);
    expect(resistFactor(sim, machine, "physical")).toBe(1);
    expect(resistFactor(sim, machine, "shock")).toBeCloseTo(1.5);
    runActions(sim, [{ do: "status", effect: "dusted" }], plain.id);
    expect(speedFactor(sim, plain)).toBe(0.5);
    runActions(sim, [{ do: "status", effect: "dazzled" }], plain.id);
    expect(speedFactor(sim, plain)).toBe(0);
  });
});

// --- E8 --------------------------------------------------------------------------------------

describe("E8: light the sim can see", () => {
  it("litAt: prop lights only, inside two thirds of the radius, by the same rule the renderer draws them with", () => {
    const { sim, cx, cy } = quietHall();
    const x = centre(cx + 6);
    const y = centre(cy);
    expect(litAt(sim, x, y)).toBe(false);
    const lamp = putProp(sim, "test_lamp", "test_lamp", cx + 6, cy);
    expect(litAt(sim, x, y), "it is off").toBe(false);
    lamp.on = true;
    expect(litAt(sim, x, y)).toBe(true);
    expect(litAt(sim, x + 39, y)).toBe(true);
    expect(litAt(sim, x + 45, y), "the last third of a light barely lifts the dark").toBe(false);
    lamp.hidden = true;
    expect(litAt(sim, x, y)).toBe(false);
    lamp.hidden = false;
    lamp.on = false;

    // Not her, not a bolt, not a bat with a red lamp: a sentry must not see her because she cast.
    putUnit(sim, "bat", "test_bat", cx + 6, cy + 1).controller = "npc";
    sim.command({ t: "dev", dev: { op: "learn", spell: "icebolt" } });
    tryCast(sim, sim.player, "icebolt", { x: 1, y: 0 });
    idle(sim, 3);
    expect(sim.zone.projectiles.length).toBe(1);
    expect(litAt(sim, sim.zone.projectiles[0].x, sim.zone.projectiles[0].y)).toBe(false);
    expect(litAt(sim, sim.player.x, sim.player.y)).toBe(false);
    expect(litAt(sim, x, y + 8)).toBe(false);

    // Cold light is light, except to what only minds the warm kind.
    putProp(sim, "test_cold_lamp", "test_cold", cx + 6, cy).on = true;
    expect([litAt(sim, x, y), litAt(sim, x, y, true)]).toEqual([true, false]);
  });

  it("a lamp post lights the ground from 18:30 to 06:30, and a sunbeam the rest of the day", () => {
    const { sim, cx, cy } = quietHall();
    putProp(sim, "test_night_lamp", "test_post", cx + 4, cy);
    putProp(sim, "test_sunbeam", "test_beam", cx - 8, cy);
    const post: [number, number] = [centre(cx + 4), centre(cy) + 8];
    const beam: [number, number] = [centre(cx - 8), centre(cy) + 8];
    sim.command({ t: "dev", dev: { op: "time", hour: 12 } });
    expect([lampsLit(sim.state), litAt(sim, ...post), litAt(sim, ...beam)]).toEqual([false, false, true]);
    sim.command({ t: "dev", dev: { op: "time", hour: 19 } });
    expect([lampsLit(sim.state), litAt(sim, ...post), litAt(sim, ...beam)]).toEqual([true, true, false]);
    sim.command({ t: "dev", dev: { op: "time", hour: 6.25 } });
    expect(litAt(sim, ...post)).toBe(true);
    const c = buildCatalog();
    c.props.lamp_post.dayOnly = true;
    expect(validateCatalog(c).join("\n")).toMatch(/cannot be both nightOnly and dayOnly/);
  });

  it('sight: "lit". It does not see her in the dark, sees her in the light, loses her when the lamp goes out, and of two people takes the lit one', () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    const sentry = putUnit(sim, "test_sentry", "test_sentry", cx + 7, cy);
    const plain = putUnit(sim, "skeleton", "test_plain", cx + 7, cy + 3);
    idle(sim, 40);
    expect(plain.target, "an ordinary skeleton at the same distance has her at once").toBe(p.id);
    expect([sentry.combat, sentry.target]).toEqual(["idle", 0]);
    removeUnit(sim, plain);
    const lamp = putProp(sim, "test_lamp", "test_lamp", cx, cy - 1);
    lamp.on = true;
    idle(sim, 12);
    expect([sentry.combat, sentry.target]).toEqual(["combat", p.id]);
    lamp.on = false;
    idle(sim, 2);
    expect(sentry.target).toBe(0);
    idle(sim, 40);
    expect(sentry.target, "and it does not find her again in the dark").toBe(0);

    // Two of them: the nearer one in the dark, the further one under the lamp.
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.rt.units.get(sim.state.players[seat].unitId)!;
    placeUnit(sim, sentry, centre(cx + 7), centre(cy));
    sentry.homeX = sentry.x;
    sentry.homeY = sentry.y;
    sentry.combat = "idle";
    placeUnit(sim, friend, centre(cx + 5), centre(cy + 4));
    placeUnit(sim, p, centre(cx), centre(cy));
    lamp.on = true;
    expect([litAt(sim, p.x, p.y), litAt(sim, friend.x, friend.y)]).toEqual([true, false]);
    for (let t = 0; t < 12; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(sentry.target).toBe(p.id);
  });

  it("shunsLight: it comes for her, stops at the edge of the warm light and waits there; when the lamp goes out it comes in", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    const lamp = putProp(sim, "test_lamp", "test_lamp", cx, cy - 1);
    lamp.on = true;
    const shade = putUnit(sim, "test_shade", "test_shade", cx + 10, cy);
    const hp = p.hp;
    let searches = 0;
    for (let t = 0; t < 600; t++) {
      sim.tick(NO_INPUT);
      expect(litAt(sim, shade.x, shade.y, true), `tick ${t}: it never stands in the light`).toBe(false);
      if (t === 300) searches = sim.rt.path.stats.searches;
    }
    expect(shade.target).toBe(p.id);
    expect(p.hp).toBe(hp);
    // As near as the dark lets it: just outside the 40 px that a 60 px lamp lights.
    const fromLamp = distance(shade.x, shade.y, centre(lamp.cx), centre(lamp.cy));
    expect(fromLamp).toBeGreaterThan(40);
    expect(fromLamp).toBeLessThan(56);
    expect(sim.rt.path.stats.searches - searches, "waiting costs a re-plan every 20 ticks, not a search a tick").toBeLessThan(25);

    // A cold light does not hold it; nor does no light.
    lamp.on = false;
    putProp(sim, "test_cold_lamp", "test_cold", cx, cy - 1).on = true;
    idle(sim, 300);
    expect(p.hp).toBeLessThan(hp);

    // Caught in the light when a lamp comes on, it does nothing but leave.
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    lamp.on = true;
    const before = p.hp;
    idle(sim, 240);
    expect(litAt(sim, shade.x, shade.y, true)).toBe(false);
    expect(p.hp).toBe(before);
  });

  it("Grow, end to end: nothing grows in the dark, the cast costs nothing then, and in the light the bud's list runs", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "learn", spell: "grow" } });
    const bud = putProp(sim, "test_bud", "test_bud", cx + 1, cy, { use: [{ do: "flag", flag: "test_bloomed" }] });
    const lamp = putProp(sim, "test_lamp", "test_lamp", cx + 2, cy - 1);
    idle(sim, 1);
    const mp = p.mp;
    sim.drainEvents();
    expect(tryCast(sim, p, "grow")).toBe("castUnsuccessful");
    expect(sim.drainEvents().some((e) => e.e === "toast" && e.text === "Nothing grows without light")).toBe(true);
    expect([p.mp, bud.used, sim.state.flags.test_bloomed ?? 0]).toEqual([mp, false, 0]);
    lamp.on = true;
    expect(tryCast(sim, p, "grow")).toBe("castSuccessful");
    expect([bud.used, sim.state.flags.test_bloomed]).toEqual([true, 1]);
    expect(p.mp).toBe(mp - sim.catalog.spells.grow.mp);
    // It bloomed once.
    p.cooldowns = {};
    p.gcd = 0;
    expect(tryCast(sim, p, "grow")).toBe("castUnsuccessful");
  });
});

// --- E9 --------------------------------------------------------------------------------------

describe("E9: send", () => {
  const order = (then: Action[]): Action[] => [{ do: "send", unit: "test_walker", to: "test_there", then }];

  it("it walks to the mark past someone it would otherwise attack, then its list runs with it as the subject", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    sim.rt.bp.marks.test_there = { cx: cx + 10, cy };
    // She stands between it and the mark, well inside its aggro.
    const walker = putUnit(sim, "skeleton", "test_walker", cx - 10, cy);
    runActions(sim, order([{ do: "flag", flag: "test_arrived" }, { do: "status", effect: "stunned" }]), p.id);
    expect(walker.order).not.toBeNull();
    const hp = p.hp;
    let ticks = 0;
    while (walker.order && ticks++ < 2000) sim.tick(NO_INPUT);
    expect(sim.state.flags.test_arrived).toBe(1);
    expect(distance(walker.x, walker.y, centre(cx + 10), centre(cy))).toBeLessThanOrEqual(12);
    expect(walker.statuses.map((s) => s.effect), "`then` lands on the one that was sent").toContain("stunned");
    expect(p.statuses.length).toBe(0);
    expect(p.hp, "it minded nothing on the way").toBe(hp);
    expect([walker.homeX, walker.homeY], "and it stays where it stops").toEqual([walker.x, walker.y]);
  });

  it("a walk it cannot finish is given up after its budget, and then nothing happens", () => {
    const { sim, cx, cy } = quietHall();
    // The mine's entry is behind a locked gate from here: there is no way.
    const walker = putUnit(sim, "skeleton", "test_walker", cx - 6, cy);
    const vault = sim.rt.bp.rects.mine_vault_room;
    sim.rt.bp.marks.test_there = { cx: vault.cx + 3, cy: vault.cy + 3 };
    placeUnit(sim, sim.player, centre(cx + 12), centre(cy + 3));
    runActions(sim, order([{ do: "flag", flag: "test_arrived" }]), sim.player.id);
    const budget = walker.order!.left;
    expect(budget).toBeGreaterThan(600);
    for (let t = 0; t <= budget && walker.order; t++) sim.tick(NO_INPUT);
    expect(walker.order).toBeNull();
    expect(sim.state.flags.test_arrived ?? 0).toBe(0);
    // Nobody of the party, nothing dead, nothing that cannot walk.
    runActions(sim, [{ do: "send", unit: "player_0", to: "test_there" }], sim.player.id);
    expect(sim.player.order).toBeNull();
  });

  it("a save taken mid-walk loads into the same walk, and the list still runs for whoever sent it", () => {
    const { sim, cx, cy } = quietHall();
    sim.command(0, { t: "open", on: true });
    const seat = sim.command(-1, { t: "join", who: "friend" });
    const friend = sim.state.players[seat];
    sim.rt.bp.marks.test_there = { cx: cx + 10, cy: cy + 2 };
    const walker = putUnit(sim, "rat", "test_walker", cx - 8, cy + 2);
    asPlayer(sim, friend, () => runActions(sim, order([{ do: "give", item: "gold_bar", qty: 2 }]), friend.unitId));
    expect(walker.order!.seat).toBe(seat);
    for (let t = 0; t < 60; t++) sim.tick([NO_INPUT, NO_INPUT]);
    expect(walker.order).not.toBeNull();

    // The save is the host's world: seat 0 sits back down alone. Both worlds are made that way, then run on.
    const solo = (): Sim => Sim.fromState(catalog, cloneState(sim.prepareSave()));
    const a = solo();
    const b = solo();
    const saved = decodeSave(encodeSave(cloneState(sim.prepareSave()), { zone: "mine", day: 0, hour: 17, hp: 1, maxhp: 1 }, new Date(0)));
    expect(saved.ok).toBe(true);
    for (let t = 0; t < 900; t++) {
      a.tick(NO_INPUT);
      b.tick(NO_INPUT);
    }
    expect(hashState(a.state)).toBe(hashState(b.state));
    const arrived = a.rt.unitsByKey.get("test_walker")!;
    expect(arrived.order).toBeNull();
    expect(distance(arrived.x, arrived.y, centre(cx + 10), centre(cy + 2))).toBeLessThanOrEqual(12);

    // In the world where she stayed, the gold is hers, not the host's.
    for (let t = 0; t < 900 && walker.order; t++) sim.tick([NO_INPUT, NO_INPUT]);
    const bag = (seatNo: number): number => (sim.rt.units.get(sim.state.players[seatNo].unitId)!.bag ?? []).reduce((n, s) => n + (s && s.item === "gold_bar" ? s.qty : 0), 0);
    expect([bag(0), bag(seat)]).toEqual([0, 2]);
  });
});

// --- E10 -------------------------------------------------------------------------------------

describe("E10: phases[n].onEnter", () => {
  it("runs once as the boss crosses into each phase, takes the phase's book, and starts over when it is whole again", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    const boss = putUnit(sim, "test_boss", "test_boss", cx + 4, cy);
    const hurt = (fraction: number): void => {
      boss.incoming.push({ amount: Math.round(maxHp(boss) * fraction), school: "physical", from: p.id, crit: false });
      sim.tick(NO_INPUT);
    };
    hurt(0.1);
    expect([boss.phase, sim.state.flags.test_boss_phases ?? 0, boss.book]).toEqual([0, 0, ["melee"]]);
    hurt(0.2);
    expect([boss.phase, sim.state.flags.test_boss_phases, boss.book]).toEqual([1, 1, ["melee_fast"]]);
    expect(boss.statuses.map((s) => s.effect), "the boss is the subject of its own list").toContain("stoneskin");
    hurt(0.05);
    expect(sim.state.flags.test_boss_phases, "once").toBe(1);
    // Its own list gave it Stone Skin: half of this lands.
    hurt(0.4);
    expect([boss.phase, sim.state.flags.test_boss_phases, boss.book]).toEqual([2, 11, ["melee_stun"]]);

    // One blow through two thresholds runs both lists, in order.
    const other = putUnit(sim, "test_boss", "test_boss_2", cx + 4, cy + 3);
    sim.state.flags.test_boss_phases = 0;
    other.incoming.push({ amount: Math.round(maxHp(other) * 0.6), school: "physical", from: 0, crit: false });
    sim.tick(NO_INPUT);
    expect([other.phase, sim.state.flags.test_boss_phases]).toEqual([2, 11]);

    // Let off, it mends, and the fight starts from the top.
    boss.combat = "idle";
    boss.target = 0;
    boss.hp = maxHp(boss) - 1;
    placeUnit(sim, p, centre(cx - 12), centre(cy));
    p.hidden = true;
    idle(sim, 30);
    expect([boss.phase, boss.book]).toEqual([0, ["melee"]]);
  });
});

// --- E11 -------------------------------------------------------------------------------------

describe("E11: a patrol point with a dwell", () => {
  it("a third number on a waypoint is ticks to stand there; a blueprint without one is what it always was", () => {
    const bp = tinyBlueprint(["#########", "#.......#", "#########"], [], { start: [1, 1] });
    bp.units.push({ key: "a", def: "rat", cx: 1, cy: 1, patrol: [[1, 1, 90], [6, 1]] }, { key: "b", def: "rat", cx: 2, cy: 1, patrol: [[1, 1], [6, 1]] });
    const sim = Sim.newGame(catalog, 5);
    const zs = createZoneState(sim.state, catalog, bp);
    expect([zs.units[0].patrolDwell, zs.units[0].dwell]).toEqual([[90, 0], 0]);
    expect(zs.units[1].patrolDwell).toBeNull();
    expect(zs.units[1].patrol).toEqual([centre(1), centre(1), centre(6), centre(1)]);
  });

  it("it stands for its dwell at the point that has one and walks straight on from the point that has none; a butterfly does it too", () => {
    const { sim, cx, cy } = quietHall();
    sim.player.hidden = true; // nothing to chase: this is about the round
    for (const [def, key, row] of [["rat", "test_rat", 0], ["dog", "test_butterfly", 3]] as const) {
      const u = putUnit(sim, def, key, cx - 8, cy + row);
      u.patrol = [centre(cx - 8), centre(cy + row), centre(cx - 2), centre(cy + row)];
      u.patrolDwell = [0, 120];
      let stood = 0;
      let longest = 0;
      let reached = false;
      for (let t = 0; t < 1200; t++) {
        const x = u.x;
        sim.tick(NO_INPUT);
        if (distance(u.x, u.y, centre(cx - 2), centre(cy + row)) <= 8) reached = true;
        stood = u.x === x && reached ? stood + 1 : 0;
        longest = Math.max(longest, stood);
      }
      expect(reached, key).toBe(true);
      expect(longest, `${key} waits at the far point`).toBeGreaterThanOrEqual(120);
      expect(longest, `${key} and then goes`).toBeLessThan(200);
      expect(u.patrolAt, key).toBeGreaterThanOrEqual(0);
    }
  });
});

// --- E12 -------------------------------------------------------------------------------------

describe("E12: nothing solid lands on a unit", () => {
  const inside = (u: Unit, r: { cx: number; cy: number; w: number; h: number }): boolean =>
    u.x + 3 > r.cx * 8 && u.x - 3 < (r.cx + r.w) * 8 && u.y + 3 > r.cy * 8 && u.y - 3 < (r.cy + r.h) * 8;

  it("show: an exhibit back on its plinth stands her beside it, on her side of the wall, and she can walk away", () => {
    const { sim } = quietHall();
    const p = sim.player;
    // Hard against the hall's north wall: the nearest free cell by a ring would be through it.
    const hall = sim.rt.bp.rects.mine_core_room;
    let at: [number, number] | null = null;
    for (let x = hall.cx + 2; x < hall.cx + hall.w - 4 && !at; x++) {
      const clear = [0, 1, 2].every((dy) => [-1, 0, 1, 2].every((dx) => sim.rt.grid.free(x + dx, hall.cy + dy, p.id)));
      if (clear && sim.rt.grid.solid(x, hall.cy - 1) && sim.rt.grid.solid(x + 1, hall.cy - 1)) at = [x, hall.cy];
    }
    expect(at).not.toBeNull();
    const [ex, ey] = at!;
    const exhibit = putProp(sim, "test_exhibit", "test_exhibit", ex, ey, { hidden: true });
    placeUnit(sim, p, centre(ex) + 3, centre(ey) + 3);
    idle(sim, 2);
    runActions(sim, [{ do: "show", prop: "test_exhibit" }], 0);
    expect(exhibit.hidden).toBe(false);
    expect(inside(p, { cx: ex, cy: ey, w: 2, h: 2 })).toBe(false);
    expect(sim.rt.grid.solid(cellOf(p.x), cellOf(p.y))).toBe(false);
    expect(cellOf(p.y), "inside the hall still").toBeGreaterThanOrEqual(hall.cy);
    expect(distance(p.x, p.y, centre(ex), centre(ey))).toBeLessThan(28);
    idle(sim, 2);
    expect(walkTo(sim, centre(ex + 6), centre(ey + 4), 6, 600)).toBe(true);
  });

  it("lock: the big door drops with a rat under it and a friend half under it, and neither is in it afterwards", () => {
    const sim = simIn(catalog, "mine", 12);
    const gate = sim.rt.propsByKey.get("gate_boss")!;
    const def = sim.catalog.props[gate.def];
    runActions(sim, [{ do: "unlock", prop: "gate_boss" }], 0);
    idle(sim, 1);
    const rat = putUnit(sim, "rat", "test_rat", gate.cx, gate.cy);
    const dog = putUnit(sim, "dog", "test_dog", gate.cx + def.w - 1, gate.cy + def.h - 1);
    runActions(sim, [{ do: "lock", prop: "gate_boss" }], 0);
    const box = { cx: gate.cx, cy: gate.cy, w: def.w, h: def.h };
    for (const u of [rat, dog]) {
      expect(inside(u, box), u.key).toBe(false);
      expect(sim.rt.grid.solid(cellOf(u.x), cellOf(u.y)), u.key).toBe(false);
    }
    expect(gate.solid).toBe(true);
    idle(sim, 1);
    expect(sim.rt.grid.solid(gate.cx, gate.cy)).toBe(true);
  });

  it("fill: a hedge does not grow through her, or round her. It waits until she has stepped out, through a save if need be", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    const r = { cx: cx - 1, cy: cy - 1, w: 3, h: 3 };
    sim.rt.bp.rects.test_hedge = r;
    runActions(sim, [{ do: "fill", rect: "test_hedge", tile: Tile.CaveWall }], p.id);
    expect(sim.zone.pendingFill).toEqual([r.cx, r.cy, 3, 3, Tile.CaveWall]);
    idle(sim, 30);
    expect(sim.rt.grid.solid(cx + 1, cy), "not one cell of it, or she would be standing in a box").toBe(false);
    // A rect nobody is in fills at once; one that lets feet through never waits.
    sim.rt.bp.rects.test_far = { cx: cx + 6, cy: cy - 1, w: 2, h: 2 };
    runActions(sim, [{ do: "fill", rect: "test_far", tile: Tile.CaveWall }, { do: "fill", rect: "test_hedge", tile: Tile.Track }], p.id);
    expect(sim.rt.grid.solid(cx + 6, cy)).toBe(true);
    expect(sim.rt.grid.tileAt(cx, cy)).toBe(Tile.Track);
    expect(sim.zone.pendingFill, "and a newer fill of the same ground replaces what the older one was owed").toEqual([]);

    runActions(sim, [{ do: "fill", rect: "test_hedge", tile: Tile.CaveWall }], p.id);
    const loaded = Sim.fromState(catalog, cloneState(sim.prepareSave()));
    expect(loaded.zone.pendingFill.length).toBe(5);
    expect(walkTo(loaded, centre(cx + 3), centre(cy + 3), 4, 600)).toBe(true);
    idle(loaded, 2);
    expect(loaded.zone.pendingFill).toEqual([]);
    for (let y = r.cy; y < r.cy + 3; y++) for (let x = r.cx; x < r.cx + 3; x++) expect(loaded.rt.grid.solid(x, y)).toBe(true);
    expect(loaded.rt.grid.solid(cellOf(loaded.player.x), cellOf(loaded.player.y))).toBe(false);
  });
});

// --- E13 -------------------------------------------------------------------------------------

describe("E13: reveal", () => {
  it("the mine's wall notice is the map: reading it shows every room this seed placed, and no corridor she has not walked", () => {
    const sim = simIn(catalog, "mine", 12);
    const bp = sim.rt.bp;
    const notice = bp.props.find((p) => p.def === "notice")!;
    const rooms = infoOf(buildZone("mine", 12))!.rooms.map((r) => r.node.id);
    const reveal = notice.use![0] as Extract<Action, { do: "reveal" }>;
    expect(notice.use!.length).toBe(1);
    expect(reveal.do).toBe("reveal");
    expect(reveal.rects).toEqual(expect.arrayContaining(["mine_entry", "mine_arena_room", "mine_vault_room"]));
    expect(reveal.rects.length).toBe(rooms.length);
    const seen = (rect: string): boolean => {
      const r = bp.rects[rect];
      return fogSeen(sim.zone.fog, sim.rt.fogW, (r.cx + (r.w >> 1)) >> 1, (r.cy + (r.h >> 1)) >> 1);
    };
    idle(sim, 12);
    expect([seen("mine_entry"), seen("mine_vault_room"), seen("boss_arena")]).toEqual([true, false, false]);
    const prop = sim.rt.propsByKey.get(notice.key)!;
    placeUnit(sim, sim.player, centre(prop.cx), centre(prop.cy + 1) + 2);
    sim.player.facing = 3;
    idle(sim, 1);
    expect(focusOf(sim, sim.player)).toMatchObject({ kind: "prop", id: prop.id, prompt: "Read" });
    sim.command({ t: "use" });
    expect(sim.me.dialogue?.tree, "it is still a notice").toBe("notice_mine");
    talkThrough(sim);
    expect([seen("mine_vault_room"), seen("boss_arena"), seen("mine_nook_room")]).toEqual([true, true, true]);
    // The broken steps stand in a corridor, and a plan of the rooms does not show it.
    const steps = sim.rt.propsByKey.get("broken_steps")!;
    expect(fogSeen(sim.zone.fog, sim.rt.fogW, steps.cx >> 1, steps.cy >> 1)).toBe(false);
  });

  it("a rect that is not there is a validation error in a blueprint's own lists, and a no-op at run time", () => {
    const bp = buildZone("mine", 12);
    const bad: Blueprint = { ...bp, props: bp.props.map((p) => (p.def === "notice" ? { ...p, use: [{ do: "reveal", rects: ["mine_entry", "nowhere"] }] } : p)) };
    expect(validateBlueprint(bad, buildCatalog(), NOTHING, []).errors.join("\n")).toMatch(/no rect "nowhere"/);
    const sim = simIn(catalog, "mine", 12);
    expect(() => runActions(sim, [{ do: "reveal", rects: ["nowhere"] }], sim.player.id)).not.toThrow();
  });
});

// --- the small fixes -------------------------------------------------------------------------

describe("small fixes", () => {
  it("a prop row's own prompt wins over 'Open': a herb is gathered, a lost thing is picked up, a chest is still opened", () => {
    const { sim, cx, cy } = quietHall();
    const loot = [{ item: "apple", qty: 1 }];
    for (const [def, prompt] of [["herb", "Gather"], ["lost_thing", "Pick up"], ["chest", "Open"]] as const) {
      const p = putProp(sim, def, `test_${def}`, cx + 1, cy, { loot: structuredClone(loot) });
      sim.player.facing = 0;
      idle(sim, 1);
      expect(focusOf(sim, sim.player), def).toMatchObject({ kind: "prop", id: p.id, prompt });
      p.hidden = true;
      p.solid = false;
      idle(sim, 1);
    }
  });

  it("dev kill takes what she can see within 25 m: not what is behind a wall, and not what is not there", () => {
    const { sim, cx, cy } = quietHall();
    const p = sim.player;
    const seenOne = putUnit(sim, "rat", "test_seen", cx + 5, cy);
    const hiddenOne = putUnit(sim, "rat", "test_hidden", cx + 5, cy + 2);
    hiddenOne.hidden = true;
    // The nearest floor within reach that a wall hides from her.
    let behind: Unit | null = null;
    for (let r = 4; r < 24 && !behind; r++) {
      for (let x = cx - r; x <= cx + r && !behind; x++) {
        for (const y of [cy - r, cy + r]) {
          if (sim.rt.grid.free(x, y) && !lineOfSight(sim.rt.grid, p.x, p.y, centre(x), centre(y))) behind = putUnit(sim, "rat", "test_behind", x, y);
          if (behind) break;
        }
      }
    }
    expect(behind, "somewhere within 25 m is out of sight").not.toBeNull();
    expect(Math.abs(behind!.x - p.x) <= 200 && Math.abs(behind!.y - p.y) <= 200).toBe(true);
    sim.command({ t: "dev", dev: { op: "kill" } });
    idle(sim, 2);
    expect([seenOne.alive, hiddenOne.alive, behind!.alive]).toEqual([false, true, true]);
  });

  it("the solver floods again for a gate and not for a chest", () => {
    const c = buildCatalog();
    const room = ["#############", "#...........#", "#...........#", "#...........#", "#############"];
    const marks: Record<string, [number, number]> = { start: [1, 2] };
    const key: PropSpawn = { key: "k", def: "chest", cx: 2, cy: 1, loot: [{ item: "key_generic", qty: 2 }] };
    const chest: PropSpawn = { key: "c", def: "chest", cx: 5, cy: 1, locked: true, keyTag: "generic", loot: [{ item: "key_mine_boss", qty: 1 }] };
    const big: PropSpawn = { key: "big", def: "chest", cx: 10, cy: 1, locked: true, keyTag: "mine_boss", loot: [{ item: "gold_bar", qty: 1 }] };
    const passes = (props: PropSpawn[]) => {
      const v = validateBlueprint(tinyBlueprint(room, props, marks), c, NOTHING, [], { trace: true });
      expect(v.errors).toEqual([]);
      return [v.trace!.passes, Object.keys(v.trace!.firedAt).sort()];
    };
    // A chain of locked chests is settled on the one flood: nothing about where she can walk changed.
    expect(passes([key, chest, big])).toEqual([1, ["big", "c", "k"]]);
    const gate: PropSpawn = { key: "g", def: "gate_v", cx: 8, cy: 1, locked: true, keyTag: "generic" };
    expect(passes([key, chest, big, gate])).toEqual([2, ["big", "c", "g", "k"]]);
  });

  it("the dog points at the drawer and teaches nothing: Repair is in the mine", () => {
    const c = buildCatalog();
    const dog = c.dialogue.dog;
    const taught: string[] = [];
    for (const node of Object.values(dog.nodes)) {
      for (const list of [node.actions ?? [], ...(node.options ?? []).map((o) => o.actions ?? [])]) for (const a of list) if (a.do === "learn") taught.push(a.spell);
    }
    expect(taught).not.toContain("repair");
    const text = dog.nodes.offer_mine.lines.join(" ");
    expect(text).toMatch(/Headmaster/);
    // No name baked in, no long dash, and the dog does not say "I" about this (VOICE.md, STORY.md 3).
    expect(text).not.toMatch(/Repair|Jane|—|\bI\b/);
    expect(dog.nodes.offer_mine.options![0].actions).toEqual([{ do: "quest", quest: "the_mine" }, { do: "flag", flag: "offered_mine" }]);
  });

  it("the adit: a door in the gallery, behind Repair, that comes out round the hill from the mine mouth; once found it opens from outside too", () => {
    const c = buildCatalog();
    const mine = buildZone("mine", 12);
    const adit = mine.props.find((p) => p.key === "adit_door")!;
    expect(adit.to).toEqual({ zone: "county", mark: "mine_adit" });
    const gallery = mine.rects.mine_gallery_room;
    expect(adit.cx >= gallery.cx && adit.cx < gallery.cx + gallery.w && adit.cy >= gallery.cy && adit.cy < gallery.cy + gallery.h).toBe(true);
    const near = (t: NonNullable<ReturnType<typeof validateBlueprint>["trace"]>): boolean => {
      for (let y = adit.cy - 1; y <= adit.cy + 2; y++) for (let x = adit.cx - 1; x <= adit.cx + 2; x++) if (t.firstSeen[y * mine.w + x] >= 0) return true;
      return false;
    };
    const solve = (withhold?: { verbs: string[] }) => validateBlueprint(mine, c, NOTHING, [], { verbs: ["icebolt"], trace: true, withhold }).trace!;
    expect(near(solve())).toBe(true);
    expect(near(solve({ verbs: ["repair"] })), "no Repair, no adit").toBe(false);

    // In play: out by the adit, and the door she came out of is barred until the gallery has been stood in.
    const sim = Sim.newGame(catalog, 3);
    const county = sim.rt.bp;
    const out = county.marks.mine_adit;
    const mouth = county.marks.mine_mouth;
    expect(Math.abs(out.cx - mouth.cx) + Math.abs(out.cy - mouth.cy)).toBeLessThan(30);
    const door = sim.rt.propsByKey.get("adit_door")!;
    expect([door.locked, door.keyTag, door.to]).toEqual([true, "", { zone: "mine", mark: "adit" }]);
    sim.command({ t: "dev", dev: { op: "tp", zone: "mine", mark: "adit" } });
    idle(sim, 3);
    expect(sim.state.flags.mine_adit_open, "she has stood in the gallery").toBe(1);
    const inner = sim.rt.propsByKey.get("adit_door")!;
    placeUnit(sim, sim.player, centre(inner.cx), centre(inner.cy + 2) + 2);
    sim.player.facing = 3;
    idle(sim, 1);
    sim.command({ t: "use" });
    idle(sim, 3);
    expect(sim.me.zone).toBe("county");
    expect(Math.abs(cellOf(sim.player.x) - out.cx) + Math.abs(cellOf(sim.player.y) - out.cy)).toBeLessThan(6);
    expect(sim.rt.propsByKey.get("adit_door")!.locked, "and now it opens from this side").toBe(false);
    expect(walkTo(sim, centre(mouth.cx), centre(mouth.cy), 8, 60 * 30)).toBe(true);
  });
});

// --- saves -------------------------------------------------------------------------------------

describe("saves", () => {
  it("v7 -> v8: units gain their orders and their dwell, zones gain what a fill is owed, and the game goes on", () => {
    const sim = simIn(catalog, "mine", 12);
    idle(sim, 5);
    const file = JSON.parse(encodeSave(sim.prepareSave(), { zone: "mine", day: 0, hour: 17, hp: 1, maxhp: 1 }, new Date(0)));
    file.version = 7;
    file.state.version = 7;
    for (const zone of Object.values(file.state.zones) as { units: Record<string, unknown>[]; pendingFill?: number[] }[]) {
      delete zone.pendingFill;
      for (const u of zone.units) {
        delete u.order;
        delete u.dwell;
        delete u.patrolDwell;
      }
    }
    const decoded = decodeSave(JSON.stringify(file));
    expect(decoded.ok).toBe(true);
    if (!decoded.ok) return;
    expect(decoded.file.state.version).toBe(SAVE_VERSION);
    expect(SAVE_VERSION).toBeGreaterThanOrEqual(8);
    const again = Sim.fromState(catalog, decoded.file.state);
    expect(again.zone.pendingFill).toEqual([]);
    expect(again.zone.units.every((u) => u.order === null && u.dwell === 0 && u.patrolDwell === null)).toBe(true);
    idle(again, 120);
    expect(hashState(again.state)).toBe(hashState((idle(sim, 120), sim.state)));
  });
});
