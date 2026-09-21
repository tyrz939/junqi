// The small engine verbs a generated dungeon leans on (DUNGEONS.md 4.7), each proven in the
// generated mine, where they are used:
//   E2  trigger rows carried by the blueprint, in the sim and in the validator
//   E3  a prop whose `to` names the zone she is in: moved to the mark, nothing reloaded
//   E4  the Sill: feet cross it, a pushed prop does not
//   E6  `strike`: the first hazard verb
// and what the solver learned: verbs, `when`, one-way hops, ablation. And the save migration
// that drops a mine saved from the hand-built layout.

import { describe, expect, it } from "vitest";
import { runActions } from "@/sim/actions";
import { buildCatalog } from "@/sim/catalog";
import { SAVE_VERSION } from "@/sim/constants";
import { centre, Tile } from "@/sim/grid";
import { moveProp, removeUnit } from "@/sim/runtime";
import { decodeSave, encodeSave } from "@/sim/save";
import { NO_INPUT, Sim } from "@/sim/sim";
import type { Facing } from "@/sim/state";
import { placeUnit } from "@/sim/units";
import type { Blueprint } from "@/world/blueprint";
import { buildZone, CONTRACTS } from "@/world/index";
import { validateBlueprint } from "@/world/validate";
import { idle, simIn, talkThrough, walkTo, yardCatalog } from "./bot";

const catalog = yardCatalog();

function inRect(sim: Sim, rect: string): boolean {
  const r = sim.rt.bp.rects[rect];
  const cx = Math.floor(sim.player.x / 8);
  const cy = Math.floor(sim.player.y / 8);
  return cx >= r.cx && cy >= r.cy && cx < r.cx + r.w && cy < r.cy + r.h;
}

describe("E4: the sill", () => {
  it("a pushed barrel stops at the mouth of its room; feet walk straight over", () => {
    const sim = simIn(catalog, "mine", 12);
    const p = sim.player;
    const grid = sim.rt.grid;
    // Nobody else in the mine: a rat standing in the way would refuse the push for its own reasons.
    for (const u of [...sim.zone.units]) if (u !== p) removeUnit(sim, u);
    // A sill of the plate room: the middle cell of three, and which way is into the room.
    const room = sim.rt.bp.rects.mine_plate_room;
    let sx = -1;
    let sy = -1;
    let ix = 0;
    let iy = 0;
    for (let y = room.cy; y < room.cy + room.h && sx < 0; y++) {
      for (let x = room.cx; x < room.cx + room.w && sx < 0; x++) {
        if (grid.tileAt(x, y) !== Tile.Sill) continue;
        const across = grid.tileAt(x - 1, y) === Tile.Sill && grid.tileAt(x + 1, y) === Tile.Sill;
        const along = grid.tileAt(x, y - 1) === Tile.Sill && grid.tileAt(x, y + 1) === Tile.Sill;
        if (!across && !along) continue;
        [sx, sy] = [x, y];
        if (across) iy = y === room.cy ? 1 : -1;
        else ix = x === room.cx ? 1 : -1;
      }
    }
    expect(sx, "the plate room has a door in use, and a sill inside it").toBeGreaterThan(0);
    expect(grid.noPush(sx, sy)).toBe(true);
    expect(grid.solid(sx, sy)).toBe(false);

    // The barrel two cells inside the sill, her behind it, pushing at the door.
    const barrel = sim.rt.propsByKey.get("plate_barrel")!;
    const bx = ix !== 0 ? (ix > 0 ? sx + 2 : sx - 3) : sx;
    const by = iy !== 0 ? (iy > 0 ? sy + 2 : sy - 3) : sy;
    moveProp(sim, barrel, bx, by);
    const hx = ix !== 0 ? sx + 4 * ix : sx;
    const hy = iy !== 0 ? sy + 4 * iy : sy;
    placeUnit(sim, p, centre(hx), centre(hy));
    p.facing = (ix > 0 ? 2 : ix < 0 ? 0 : iy > 0 ? 3 : 1) as Facing;
    idle(sim, 2);
    const push = (): void => {
      for (let t = 0; t < 40; t++) sim.tick({ mx: -ix, my: -iy, sprint: false, useHeld: true, ax: 0, ay: 0 });
    };
    push();
    expect([barrel.cx, barrel.cy], "one cell toward the door: nothing in the way yet").toEqual([bx - ix, by - iy]);
    push();
    push();
    expect([barrel.cx, barrel.cy], "and no further: the next cell is the sill").toEqual([bx - ix, by - iy]);

    // She walks out over it all the same (round the barrel: the mouth is three cells wide).
    moveProp(sim, barrel, bx + 3 * ix + (iy !== 0 ? 5 : 0), by + 3 * iy + (ix !== 0 ? 5 : 0));
    idle(sim, 2);
    expect(walkTo(sim, centre(sx - 3 * ix), centre(sy - 3 * iy), 4)).toBe(true);
  });
});

describe("E3 and E2: the way in to a room that has sealed", () => {
  it("is hidden until the gate drops, takes her to the mark inside without reloading the zone, and hides again when the room is won", () => {
    const sim = simIn(catalog, "mine", 12);
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    const p = sim.player;
    const gate = sim.rt.propsByKey.get("gate_boss")!;
    const wayIn = sim.rt.propsByKey.get("mine_arena_wayin")!;
    const inside = sim.rt.bp.marks.mine_arena_in;
    // The rows that do this were written by the generator and ride on the blueprint.
    expect(sim.rt.bp.triggers!.mine_arena_lock.zone).toBe("mine");
    expect(sim.zone.triggers.map((t) => t.id)).toEqual(expect.arrayContaining(["into_mine", "mine_arena_lock", "mine_arena_clear"]));
    expect(sim.catalog.triggers.mine_arena_lock).toBeUndefined();

    // Standing on it before the room seals, USE finds nothing: it cannot be a way round the key.
    expect(wayIn.hidden).toBe(true);
    placeUnit(sim, p, centre(wayIn.cx), centre(wayIn.cy));
    sim.command({ t: "use" });
    idle(sim, 2);
    expect(inRect(sim, "boss_arena")).toBe(false);

    // Someone goes in; the gate drops; the way in shows.
    placeUnit(sim, p, centre(inside.cx), centre(inside.cy));
    idle(sim, 3);
    expect(gate.locked).toBe(true);
    expect(wayIn.hidden).toBe(false);

    // Shut out (as a friend who was late would be): climb in.
    const rt = sim.rt;
    const zone = sim.zone;
    placeUnit(sim, p, centre(wayIn.cx), centre(wayIn.cy));
    idle(sim, 1);
    expect(inRect(sim, "boss_arena")).toBe(false);
    sim.command({ t: "use" });
    idle(sim, 2);
    expect(inRect(sim, "boss_arena")).toBe(true);
    expect(sim.me.zone).toBe("mine");
    expect(sim.me.travel).toBeNull();
    expect(sim.rt).toBe(rt);
    expect(sim.zone).toBe(zone);

    const boss = sim.rt.unitsByKey.get("iron_knuckles")!;
    boss.incoming.push({ amount: 1e6, school: "physical", from: p.id, crit: false });
    idle(sim, 5);
    expect(gate.locked).toBe(false);
    expect(wayIn.hidden).toBe(true);
  });

  it("dying inside a generated lock-in re-opens the gate, hides the way in and re-arms the trap", () => {
    const sim = simIn(catalog, "mine", 15);
    const inside = sim.rt.bp.marks.mine_arena_in;
    placeUnit(sim, sim.player, centre(inside.cx), centre(inside.cy));
    idle(sim, 3);
    const gate = sim.rt.propsByKey.get("gate_boss")!;
    const wayIn = sim.rt.propsByKey.get("mine_arena_wayin")!;
    expect(gate.locked).toBe(true);
    expect(wayIn.hidden).toBe(false);
    sim.player.incoming.push({ amount: 99999, school: "physical", from: 0, crit: false });
    idle(sim, 300);
    expect(sim.player.alive).toBe(true);
    expect(inRect(sim, "mine_entry")).toBe(true);
    // gate_boss is keyed: open again means back to how she found it, which is what `unlock` on a gate does.
    expect(gate.solid).toBe(false);
    expect(wayIn.hidden).toBe(true);
    expect(sim.zone.triggers.find((t) => t.id === "mine_arena_lock")!.fired).toBe(false);
  });

  it("the validator reads the blueprint's rows too: a clash with the catalog, a prop that is not there, a bad row", () => {
    const solve = (change: (bp: Blueprint) => void): string => {
      const bp = buildZone("mine", 12);
      change(bp);
      return validateBlueprint(bp, buildCatalog(), CONTRACTS.mine, []).errors.join("\n");
    };
    expect(solve(() => {})).toBe("");
    expect(solve((bp) => (bp.triggers!.into_mine = { zone: "mine", rect: "mine_entry", once: true, actions: [] }))).toMatch(/"into_mine" is in the blueprint and in the catalog/);
    expect(solve((bp) => bp.triggers!.mine_arena_lock.actions.push({ do: "lock", prop: "no_such_gate" }))).toMatch(/no prop "no_such_gate"/);
    expect(solve((bp) => bp.triggers!.mine_arena_lock.actions.push({ do: "learn", spell: "no_such_spell" }))).toMatch(/unknown spell/);
    expect(solve((bp) => (bp.triggers!.mine_arena_lock.rect = "nowhere"))).toMatch(/no rect "nowhere"/);
    expect(solve((bp) => (bp.props.find((p) => p.key === "mine_arena_wayin")!.to = { zone: "mine", mark: "nowhere" }))).toMatch(/leads to mark "nowhere"/);
  });
});

describe("E6: strike", () => {
  it("hits what is hostile in the rect, once, spares her unless told not to, staggers, and the kill is the puller's", () => {
    const sim = simIn(catalog, "mine", 12);
    const p = sim.player;
    const boss = sim.rt.unitsByKey.get("iron_knuckles")!;
    const clerk = sim.rt.unitsByKey.get("clerk")!;
    const drop = sim.rt.bp.rects.mine_arena_drop_1;
    const at = (dx: number, dy: number): [number, number] => [centre(drop.cx + dx), centre(drop.cy + dy)];
    placeUnit(sim, boss, ...at(2, 2));
    placeUnit(sim, p, ...at(3, 3));
    const strike = (extra: object, actor: number): void => {
      runActions(sim, [{ do: "strike", rect: "mine_arena_drop_1", amount: 200, school: "physical", effect: "staggered", ...extra }], actor);
      sim.tick(NO_INPUT);
    };
    const bossHp = boss.hp;
    const clerkHp = clerk.hp;
    const myHp = p.hp;
    strike({}, p.id);
    expect(boss.hp).toBe(bossHp - 200);
    expect(boss.statuses.some((s) => s.effect === "staggered")).toBe(true);
    expect(clerk.hp, "outside the rect").toBe(clerkHp);
    expect(p.hp, "her friends are spared").toBe(myHp);
    // Staggered: cannot move, takes double.
    strike({}, p.id);
    expect(boss.hp).toBe(bossHp - 600);
    strike({ hitsFriends: true, amount: 10 }, 0);
    expect(p.hp).toBeLessThan(myHp);
    // The blow that kills him is hers: the quest counts it.
    sim.command({ t: "dev", dev: { op: "quest", quest: "the_mine" } });
    const kills = sim.me.stats.kills;
    strike({}, p.id);
    expect(boss.alive).toBe(false);
    expect(sim.me.stats.kills).toBe(kills + 1);
    expect(sim.state.quests.active.find((q) => q.quest === "the_mine")!.counts[2]).toBe(1);
  });

  it("a strike row is checked at boot like any other: a school, an amount, a rect, a real effect", () => {
    const c = buildCatalog();
    c.triggers.bad = { zone: "mine", rect: "mine_entry", once: true, actions: [{ do: "strike", rect: "", amount: 0, school: "heal", effect: "no_such_effect" }] };
    // validateCatalog is what buildCatalog throws from; ask it directly.
    return import("@/sim/catalog").then(({ validateCatalog }) => {
      const errors = validateCatalog(c).join("\n");
      expect(errors).toMatch(/strike needs a rect, an amount and a damage school/);
      expect(errors).toMatch(/unknown effect "no_such_effect"/);
    });
  });
});

describe("what the solver learned", () => {
  const solverCatalog = buildCatalog();
  const bp = buildZone("mine", 12);
  const solve = (opts: Parameters<typeof validateBlueprint>[4]) => validateBlueprint(bp, solverCatalog, CONTRACTS.mine, [], { verbs: ["icebolt"], trace: true, ...opts });
  const reached = (v: ReturnType<typeof solve>, rect: string): boolean => {
    const r = bp.rects[rect];
    for (let y = r.cy; y < r.cy + r.h; y++) for (let x = r.cx; x < r.cx + r.w; x++) if (v.trace!.firstSeen[y * bp.w + x] >= 0) return true;
    return false;
  };

  it("verbs: Repair is learned from the drawer's page, and nothing that answers Repair fires without it", () => {
    const whole = solve({});
    expect(whole.errors).toEqual([]);
    expect(whole.trace!.verbs).toContain("repair");
    expect(whole.trace!.firedAt.broken_steps).toBeGreaterThanOrEqual(whole.trace!.firedAt.hm_drawer);
    const without = solve({ withhold: { verbs: ["repair"] } });
    expect(without.ok).toBe(false);
    expect(reached(without, "mine_gallery_room")).toBe(false);
    expect(without.trace!.firedAt.hm_cabinet).toBeUndefined();
    // Left ungated (no `verbs`), a broken thing mends itself, which is how the older zones are still judged.
    expect(validateBlueprint(bp, solverCatalog, CONTRACTS.mine, []).errors).toEqual([]);
  });

  it("when: a `while` row fires only if its conditions can hold: no flag, no nook; no dead Headmaster, no drawer", () => {
    expect(reached(solve({}), "mine_nook_room")).toBe(true);
    expect(reached(solve({ withhold: { flags: ["mine_cleared"] } }), "mine_nook_room")).toBe(false);
    // The Headmaster stands behind his own gate: keep it shut and the drawer never opens.
    const shutOut = solve({ shut: ["gate_hm"] });
    expect(shutOut.trace!.dead).not.toContain("headmaster");
    expect(shutOut.trace!.firedAt.hm_drawer).toBeUndefined();
    expect(shutOut.trace!.verbs).not.toContain("repair");
  });

  it("a `to` into the same zone is a one-way edge: with the way in showing, the arena is reached over a gate that never opens", () => {
    const open: Blueprint = { ...bp, props: bp.props.map((q) => (q.key === "mine_arena_wayin" ? { ...q, hidden: false } : q)) };
    const over = validateBlueprint(open, solverCatalog, CONTRACTS.mine, [], { verbs: ["icebolt"], trace: true, shut: ["gate_boss"] });
    const m = bp.marks.mine_arena_in;
    expect(over.trace!.firstSeen[m.cy * bp.w + m.cx]).toBeGreaterThanOrEqual(0);
    const hidden = solve({ shut: ["gate_boss"] });
    expect(hidden.trace!.firstSeen[m.cy * bp.w + m.cx]).toBe(-1);
  });
});

describe("rows", () => {
  it("the drawer's page is read once; after that it says so, and teaches nothing twice", () => {
    const sim = simIn(catalog, "mine", 12);
    sim.command({ t: "dev", dev: { op: "god", on: true } });
    const drawer = sim.rt.propsByKey.get("hm_drawer")!;
    const hm = sim.rt.unitsByKey.get("headmaster")!;
    expect(drawer.locked).toBe(true);
    placeUnit(sim, sim.player, hm.x + 12, hm.y);
    hm.incoming.push({ amount: 1e6, school: "physical", from: sim.player.id, crit: false });
    idle(sim, 4);
    expect(drawer.locked).toBe(false);
    // A player the dog already taught (the dog still does, for now) loses nothing by looking.
    sim.command({ t: "dev", dev: { op: "learn", spell: "repair" } });
    placeUnit(sim, sim.player, centre(drawer.cx) - 8, centre(drawer.cy + 2) + 2);
    sim.command({ t: "use" });
    expect(sim.me.dialogue?.node).toBe("spent");
    talkThrough(sim);
    expect(sim.state.growth.spells.filter((s) => s === "repair").length).toBe(1);
  });
});

describe("saves", () => {
  it("v6 -> v7: a mine saved from the hand-built layout is dropped, and whoever saved inside it walks back in from the mouth", () => {
    const sim = Sim.newGame(catalog, 21);
    sim.command({ t: "dev", dev: { op: "tp", zone: "mine", mark: "entry" } });
    idle(sim, 2);
    expect(sim.me.zone).toBe("mine");
    sim.state.rest = { zone: "mine", x: sim.player.x, y: sim.player.y };
    const file = JSON.parse(encodeSave(sim.prepareSave(), { zone: "mine", day: 0, hour: 17, hp: 1, maxhp: 1 }, new Date(0)));
    file.version = 6;
    file.state.version = 6;
    const decoded = decodeSave(JSON.stringify(file));
    expect(decoded.ok).toBe(true);
    if (!decoded.ok) return;
    const state = decoded.file.state;
    expect(state.version).toBe(SAVE_VERSION);
    expect(state.zones.mine).toBeUndefined();
    expect(state.players[0].zone).toBe("county");
    expect(state.rest).toBeNull();
    const again = Sim.fromState(catalog, state);
    idle(again, 2);
    const mouth = again.rt.bp.marks.mine_mouth;
    expect(again.me.zone).toBe("county");
    expect(Math.abs(again.player.x - centre(mouth.cx)) + Math.abs(again.player.y - centre(mouth.cy))).toBeLessThan(80);
    expect(bagCountOf(again)).toBe(bagCountOf(sim));
  });
});

function bagCountOf(sim: Sim): number {
  return (sim.player.bag ?? []).reduce((n, s) => n + (s ? s.qty : 0), 0);
}
