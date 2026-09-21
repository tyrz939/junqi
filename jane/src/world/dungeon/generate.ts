// The dungeon generator (DUNGEONS.md 2.5): authored mission, generated space. It returns an
// ordinary Blueprint through Kit, so buildZone, the solver, the save format and the renderer
// do not know a dungeon was generated.
//
//   1 choose   a template for each node from its pool, no template twice      (layout.ts)
//   2 embed    rooms onto the bay lattice, critical nodes first                (layout.ts)
//   3 route    a corridor for every edge, along the lines between bays         (layout.ts)
//   4 lock     gates, verb props, lock-ins and their way in, flag gates, state gates (and, in 5, the
//              one list per control that drives them both ways)
//   5 fill     holdings into sockets, the enemy mix by heat, dressing
//   6 name     bound things get their contract name, the rest `${zone}_${node}_${socket}`
//   7 emit     the Blueprint and the trigger rows it carries
//
// A generated name comes from node and socket, never from a coordinate or the attempt, so a
// re-rolled layout cannot hand the same jar out twice or lose one (`grow` keys on the name).

import { propFootprints, type TriggerDef } from "@/sim/catalog";
import { Tile } from "@/sim/grid";
import type { Action, ActionList, Condition, Stack } from "@/sim/state";
import { ZONE_ATTEMPTS, type Blueprint, type Rect } from "@/world/blueprint";
import { Kit } from "@/world/kit";
import { embed, embedFallback, Lanes } from "@/world/dungeon/layout";
import { templateById } from "@/world/dungeon/pools";
import { BAY_H, BAY_W, BORDER, shapeOf, type Shape } from "@/world/dungeon/room";
import type { Corridor, Door, DungeonDef, EdgeKind, Holding, Layout, LockinSpec, MissionNode, Placement, Side, Socket, StateVar } from "@/world/dungeon/types";

/** The last attempt stamps the hand-placed fallback: never throw at the player. */
export const DUNGEON_ATTEMPTS = ZONE_ATTEMPTS;

/** What a jar or a page gives when the mission does not say. John's to tune (DUNGEONS.md 3). */
const FOUND: Record<string, { stat: "strength" | "spirit"; amount: number; hide: boolean }> = {
  jar: { stat: "strength", amount: 2, hide: false },
  jar_big: { stat: "strength", amount: 14, hide: false },
  leaf_page: { stat: "spirit", amount: 2, hide: true },
};

export type RoomInfo = {
  node: MissionNode;
  shape: Shape;
  /** Zone cell of the shape's (0,0). */
  x: number;
  y: number;
  /** The whole floor, in zone cells. */
  rect: Rect;
  /** A cell to walk to that stands for "in this room". */
  centre: [number, number];
  /** Doors in use. */
  doors: string[];
};

export type EdgeLock = {
  edge: number;
  kind: EdgeKind;
  /** The prop that stands in the corridor. */
  prop: string;
};

export type LockinInfo = { node: string; gate: string; wayIn: string; mark: string; rect: string; lock: string; clear: string };

/** A prop that flips a state, and the state it flips. */
export type ControlInfo = { state: string; prop: string };

/** Everything the checks and the viewer want to know about how a blueprint was made. */
export type BuildInfo = {
  def: DungeonDef;
  layout: Layout | null;
  rooms: RoomInfo[];
  locks: EdgeLock[];
  lockins: LockinInfo[];
  controls: ControlInfo[];
  /** Why there is no dungeon here, when there is not. */
  errors: string[];
};

const INFO = new WeakMap<Blueprint, BuildInfo>();

export function infoOf(bp: Blueprint): BuildInfo | undefined {
  return INFO.get(bp);
}

let footprints: Record<string, { w: number; h: number }> | null = null;

function tileOf(name: string, where: string): Tile {
  const t = (Tile as unknown as Record<string, number>)[name];
  if (typeof t !== "number") throw new Error(`${where}: no tile called "${name}"`);
  return t as Tile;
}

const OUT: Record<Side, [number, number]> = { n: [0, -1], s: [0, 1], w: [-1, 0], e: [1, 0] };

/** The name the generator gives a socket, mark or rect of a node: its bind if it has one, else `${zone}_${node}_${local}`. */
export function localName(def: DungeonDef, node: MissionNode, local: string): string {
  return node.binds.find((b) => b.from === local)?.as ?? `${def.id}_${node.id}_${local.replace(/:/g, "_")}`;
}

export type BuildOptions = {
  /**
   * The template harness: every door of every room is opened onto a dead-end stub with a mark
   * at its end (`${zone}_${node}_door_${id}`), so a room can be solved alone from each door.
   */
  stubs?: boolean;
};

export function buildDungeon(def: DungeonDef, seed: number, attempt: number, opts: BuildOptions = {}): Blueprint {
  footprints ??= propFootprints();
  const foot = footprints;
  const zone = def.id;
  const { cols, rows } = def.lattice;
  const W = 2 * BORDER + cols * BAY_W;
  const H = 2 * BORDER + rows * BAY_H;
  const WALL = tileOf(def.tiles.wall, zone);
  const FLOOR = tileOf(def.tiles.floor, zone);
  const SILL = Tile.Sill;
  const k = new Kit(zone, W, H, seed, attempt, WALL);
  const info: BuildInfo = { def, layout: null, rooms: [], locks: [], lockins: [], controls: [], errors: [] };
  const finish = (): Blueprint => {
    const bp = k.done(def.name, def.indoor, def.ambient, attempt);
    bp.triggers = triggers;
    INFO.set(bp, info);
    return bp;
  };
  const triggers: Record<string, TriggerDef> = {};

  const layout = attempt >= DUNGEON_ATTEMPTS - 1 ? embedFallback(def) : embed(def, k);
  info.layout = layout;
  if (!layout) {
    info.errors.push("embed: no layout found");
    return finish();
  }
  const lanes = new Lanes(cols, rows);

  // --- rooms -------------------------------------------------------------------------------
  const roomOf = (id: string): RoomInfo => {
    const r = info.rooms.find((x) => x.node.id === id);
    if (!r) throw new Error(`Dungeon "${zone}": room "${id}" was never placed`);
    return r;
  };
  for (const p of layout.placements) {
    const template = templateById(p.template);
    const node = def.nodes.find((n) => n.id === p.node);
    if (!template || !node) throw new Error(`Dungeon "${zone}": placement of "${p.node}" names something that does not exist`);
    const shape = shapeOf(template, p.turn, p.mirror);
    const x = BORDER + p.bay[0] * BAY_W + shape.ox;
    const y = BORDER + p.bay[1] * BAY_H + shape.oy;
    const doors = opts.stubs ? shape.doors.map((d) => d.id) : layout.corridors.flatMap((c) => [c.a, c.b]).filter((u) => u.node === node.id).map((u) => u.door);
    const open = new Set<string>();
    for (const d of shape.doors) {
      if (!doors.includes(d.id)) continue;
      const along: [number, number] = d.side === "n" || d.side === "s" ? [1, 0] : [0, 1];
      for (let n = -1; n <= 1; n++) {
        open.add(`${d.cx + along[0] * n},${d.cy + along[1] * n}`);
        open.add(`${d.cx + along[0] * n - OUT[d.side][0]},${d.cy + along[1] * n - OUT[d.side][1]}`);
      }
    }
    for (let j = 0; j < shape.h; j++) {
      const row = shape.cells[j];
      for (let i = 0; i < shape.w; i++) {
        const ch = row[i];
        const rim = i === 0 || j === 0 || i === shape.w - 1 || j === shape.h - 1;
        let tile = WALL;
        if (ch === ".") tile = FLOOR;
        // A sill is kept only under a door that is in use; a door nobody uses is wall again.
        else if (ch === "_") tile = open.has(`${i},${j}`) ? SILL : FLOOR;
        else if (ch !== "#") tile = rim ? (open.has(`${i},${j}`) ? FLOOR : WALL) : tileOf(template.tiles[ch] ?? "", template.id);
        k.set(x + i, y + j, tile);
      }
    }
    const room = shape.rects.find((r) => r.id === "room") as Rect;
    const centreMark = shape.marks.find((m) => m.id === "centre");
    info.rooms.push({
      node,
      shape,
      x,
      y,
      rect: { cx: x + room.cx, cy: y + room.cy, w: room.w, h: room.h },
      centre: centreMark ? [x + centreMark.cx, y + centreMark.cy] : [x + (shape.w >> 1), y + (shape.h >> 1)],
      doors,
    });
  }

  // --- names -------------------------------------------------------------------------------
  const nameOf = (node: MissionNode, local: string): string => localName(def, node, local);
  /** `@socket` and `@self` inside an authored list become real names. */
  const resolve = (node: MissionNode, self: string, list: ActionList | undefined): ActionList | undefined => {
    if (!list) return undefined;
    const name = (v: string): string => (v === "@self" ? self : v.startsWith("@") ? nameOf(node, v.slice(1)) : v);
    return list.map((a): Action => {
      const b = { ...a } as Record<string, unknown>;
      for (const f of ["prop", "unit", "at", "rect", "id", "to"]) if (typeof b[f] === "string") b[f] = name(b[f] as string);
      if (Array.isArray(b.rects)) b.rects = (b.rects as string[]).map(name);
      // Lists inside lists: an `if`'s branches, what a sent unit does when it arrives.
      for (const f of ["then", "else"]) if (Array.isArray(b[f])) b[f] = resolve(node, self, b[f] as ActionList);
      if (a.do === "if") b.when = resolveWhen(node, a.when);
      return b as Action;
    });
  };
  const resolveWhen = (node: MissionNode, list: Condition[] | undefined): Condition[] | undefined =>
    list?.map((c) => (c.if === "dead" && c.unit.startsWith("@") ? { ...c, unit: nameOf(node, c.unit.slice(1)) } : c));

  // --- corridors ---------------------------------------------------------------------------
  const doorAt = (r: RoomInfo, id: string): { d: Door; x: number; y: number } => {
    const d = r.shape.doors.find((x) => x.id === id) as Door;
    return { d, x: r.x + d.cx, y: r.y + d.cy };
  };
  /** From just outside the rim to the edge of the lane: at least one cell, always. */
  const carveStub = (r: RoomInfo, doorId: string, port: number): void => {
    const { d, x, y } = doorAt(r, doorId);
    const [px, py] = lanes.cell(port);
    if (d.side === "n") k.fill(x - 1, py + 2, 3, y - (py + 2), FLOOR);
    else if (d.side === "s") k.fill(x - 1, y + 1, 3, py - 2 - y, FLOOR);
    else if (d.side === "w") k.fill(px + 2, y - 1, x - (px + 2), 3, FLOOR);
    else k.fill(x + 1, y - 1, px - 2 - x, 3, FLOOR);
  };
  for (const c of layout.corridors) {
    carveStub(roomOf(c.a.node), c.a.door, c.lane[0]);
    carveStub(roomOf(c.b.node), c.b.door, c.lane[c.lane.length - 1]);
    for (let n = 0; n < c.lane.length; n++) {
      const [ax, ay] = lanes.cell(c.lane[n]);
      k.fill(ax - 1, ay - 1, 3, 3, FLOOR);
      if (n + 1 === c.lane.length) break;
      const [bx, by] = lanes.cell(c.lane[n + 1]);
      k.fill(Math.min(ax, bx) - 1, Math.min(ay, by) - 1, Math.abs(bx - ax) + 3, Math.abs(by - ay) + 3, FLOOR);
    }
  }

  if (opts.stubs) {
    const bayOf = (r: RoomInfo): [number, number] => {
      const p = layout.placements.find((x) => x.node === r.node.id) as Placement;
      return p.bay;
    };
    for (const r of info.rooms) {
      for (const d of r.shape.doors) {
        const [bx, by] = bayOf(r);
        const port = lanes.port(bx, by, r.shape.bays, d);
        carveStub(r, d.id, port);
        const [px, py] = lanes.cell(port);
        k.fill(px - 1, py - 1, 3, 3, FLOOR);
        k.mark(`${zone}_${r.node.id}_door_${d.id}`, px, py);
      }
    }
  }

  // --- locks -------------------------------------------------------------------------------
  /** A gate in the one corridor cell that is always there: just outside the far room's door. */
  const placeGate = (c: Corridor, key: string, extra: { locked: boolean; keyTag?: string; label?: string }, rows: [string, string] = ["gate_h", "gate_v"]): void => {
    const { d, x, y } = doorAt(roomOf(c.b.node), c.b.door);
    const [ox, oy] = OUT[d.side];
    const gx = x + ox;
    const gy = y + oy;
    for (const row of rows) if (!foot[row]) throw new Error(`Dungeon "${zone}": gate row "${row}" is not a prop row`);
    if (foot[rows[0]].w !== 3 || foot[rows[0]].h !== 1 || foot[rows[1]].w !== 1 || foot[rows[1]].h !== 3) throw new Error(`Dungeon "${zone}": gate rows ${rows.join(", ")} must be 3x1 and 1x3`);
    if (d.side === "n" || d.side === "s") k.prop({ key, def: rows[0], cx: gx - 1, cy: gy, ...extra }, 3, 1);
    else k.prop({ key, def: rows[1], cx: gx, cy: gy - 1, ...extra }, 1, 3);
  };
  const stateOf = (id: string): StateVar => {
    const sv = (def.states ?? []).find((x) => x.id === id);
    if (!sv) throw new Error(`Dungeon "${zone}": no state called "${id}"`);
    if (!sv.values.includes(sv.initial)) throw new Error(`Dungeon "${zone}": state "${id}" starts as "${sv.initial}", which is not one of its values`);
    return sv;
  };
  /** Gates that stand or fall with a state: what each control's list must drive, both ways. */
  const stateGates: { state: string; is: string; gate: string }[] = [];
  /** A prop that fills the corridor: in the middle of a straight stub, or of the last lane before the far room. */
  const placeBlock = (c: Corridor, key: string, propDef: string, spawn: { needs?: Stack[]; use?: ActionList; label?: string }): void => {
    const f = foot[propDef];
    if (!f) throw new Error(`Dungeon "${zone}": edge prop "${propDef}" is not a prop row`);
    let cx: number;
    let cy: number;
    let vertical: boolean;
    if (c.lane.length === 1) {
      [cx, cy] = lanes.cell(c.lane[0]);
      const side = doorAt(roomOf(c.b.node), c.b.door).d.side;
      vertical = side === "n" || side === "s";
    } else {
      const [ax, ay] = lanes.cell(c.lane[c.lane.length - 2]);
      const [bx, by] = lanes.cell(c.lane[c.lane.length - 1]);
      cx = (ax + bx) >> 1;
      cy = (ay + by) >> 1;
      vertical = ax === bx;
    }
    if (vertical) {
      if (f.w !== 3) throw new Error(`Dungeon "${zone}": "${propDef}" is ${f.w} wide and cannot seal a corridor of 3`);
      k.prop({ key, def: propDef, cx: cx - 1, cy: cy - (f.h >> 1), ...spawn }, f.w, f.h);
    } else {
      if (f.h > 3) throw new Error(`Dungeon "${zone}": "${propDef}" is ${f.h} tall and cannot stand in a corridor of 3`);
      const x0 = cx - (f.w >> 1);
      k.prop({ key, def: propDef, cx: x0, cy: cy - 1, ...spawn }, f.w, f.h);
      // The steps are two cells tall: the corridor narrows to a neck exactly as long as they are.
      if (f.h < 3) k.fill(x0, cy - 1 + f.h, f.w, 3 - f.h, WALL);
    }
  };
  k.rect(`${zone}_all`, { cx: 0, cy: 0, w: W, h: H });
  for (const c of layout.corridors) {
    const edge = def.edges[c.edge];
    const to = roomOf(c.b.node);
    const from = roomOf(c.a.node);
    let gateKey: string | null = null;
    const kinds: EdgeKind[] = [edge.kind, ...(edge.also ?? [])];
    const gateName = (): string => {
      for (const kind of kinds) if ((kind.t === "key" || kind.t === "lockin" || kind.t === "oneway" || kind.t === "state") && kind.gateAs) return kind.gateAs;
      return `${zone}_gate_${from.node.id}_${to.node.id}`;
    };
    for (const kind of kinds) {
      if (kind.t === "key") {
        gateKey = gateName();
        placeGate(c, gateKey, { locked: true, keyTag: kind.tag, label: kind.label });
        info.locks.push({ edge: c.edge, kind, prop: gateKey });
      } else if (kind.t === "verb") {
        const key = kind.propAs ?? `${zone}_${kind.prop}_${from.node.id}_${to.node.id}`;
        const use: ActionList = [{ do: "hide", prop: key }];
        if (kind.toast) use.push({ do: "toast", text: kind.toast });
        placeBlock(c, key, kind.prop, { needs: kind.needs, use, label: kind.label });
        info.locks.push({ edge: c.edge, kind, prop: key });
      } else if (kind.t === "oneway") {
        gateKey = gateName();
        placeGate(c, gateKey, { locked: true, label: "The gate" });
        triggers[`${zone}_${from.node.id}_${to.node.id}_opens`] = {
          zone,
          rect: `${zone}_all`,
          mode: "while",
          once: true,
          when: [{ if: "flag", flag: kind.flag }],
          actions: [{ do: "unlock", prop: gateKey }],
        };
        info.locks.push({ edge: c.edge, kind, prop: gateKey });
      } else if (kind.t === "state") {
        // Passable only while the state has this value. No key fits it: the state's controls
        // drive it, and the list that does so is written below, once, for every such gate.
        const sv = stateOf(kind.var);
        if (!sv.values.includes(kind.is)) throw new Error(`Dungeon "${zone}": state "${kind.var}" has no value "${kind.is}"`);
        gateKey = gateName();
        placeGate(c, gateKey, { locked: kind.is !== sv.initial, label: kind.label }, kind.gate);
        stateGates.push({ state: kind.var, is: kind.is, gate: gateKey });
        info.locks.push({ edge: c.edge, kind, prop: gateKey });
      }
    }
    const lockin = kinds.find((x): x is LockinSpec => x.t === "lockin");
    if (lockin) {
      if (!gateKey) {
        // A lock-in with no key of its own: the gate stands open until the room seals.
        gateKey = gateName();
        placeGate(c, gateKey, { locked: false });
      }
      writeLockin(c, to, lockin, gateKey);
    }
  }

  /**
   * One function writes every lock-in, so none can forget its reset (DUNGEONS.md 2.5). The
   * way in is hidden until the room seals, so it can never be a way round a keyed gate; it is
   * how a friend who was late, or who died and walked back, follows.
   */
  function writeLockin(c: Corridor, room: RoomInfo, spec: LockinSpec, gate: string): void {
    const node = room.node;
    const { d, x, y } = doorAt(room, c.b.door);
    const [ox, oy] = OUT[d.side];
    // The rect that seals the room is the template's `inner`, or the whole floor. Keep it the
    // whole floor unless there is a reason: the clear only fires while someone stands in it, and
    // a death only re-opens the gate when nobody alive is left in it, so a rect that stops short
    // of the walls forgets whoever is at a lever by the wall. The gate stands outside the room,
    // in the corridor, so it cannot land on anyone however early the rect starts (C12).
    const inner = room.shape.rects.find((r) => r.id === "inner");
    const rect: Rect = inner ? { cx: room.x + inner.cx, cy: room.y + inner.cy, w: inner.w, h: inner.h } : room.rect;
    const rectName = nameOf(node, "inner");
    k.rect(rectName, rect);
    // Where she lands: straight in from the door, the first clear floor cell that is well inside
    // the rect (two cells, so that arriving there is arriving in the fight, not on its edge).
    let mx = x;
    let my = y;
    for (let n = 1; n < 24; n++) {
      mx = x - ox * n;
      my = y - oy * n;
      if (mx >= rect.cx + 2 && my >= rect.cy + 2 && mx < rect.cx + rect.w - 2 && my < rect.cy + rect.h - 2 && k.fits(mx, my, 1, 1)) break;
    }
    const markName = `${zone}_${node.id}_in`;
    k.mark(markName, mx, my);
    // The way in: beside the gate, on the side she is shut out on.
    const wayIn = `${zone}_${node.id}_wayin`;
    const gx = x + ox;
    const gy = y + oy;
    const vertical = d.side === "n" || d.side === "s";
    const wx = vertical ? gx - 1 : ox > 0 ? gx + 1 : gx - 2;
    const wy = vertical ? gy + oy : gy;
    k.prop({ key: wayIn, def: "way_in", cx: wx, cy: wy, hidden: true, to: { zone, mark: markName }, label: "A way in" }, 2, 1);

    const boss = node.holds.find((hd): hd is Extract<Holding, { unit: string }> => "unit" in hd && (hd.socket.startsWith("boss") || node.kind === "boss"));
    const spawned = (spec.spawn?.at ?? []).map((at, n) => ({ unit: spec.spawn?.as?.[n] ?? `${zone}_${node.id}_lockin_${n}`, at: nameOf(node, at) }));
    const foes = spec.spawn ? spawned.map((s) => s.unit) : boss ? [nameOf(node, boss.socket)] : [];
    const lock = `${zone}_${node.id}_lock`;
    const clear = `${zone}_${node.id}_clear`;
    const cleared = `${zone}_${node.id}_clear`;
    const actions: ActionList = [{ do: "lock", prop: gate }, { do: "show", prop: wayIn }];
    const reset: ActionList = [{ do: "unlock", prop: gate }, { do: "hide", prop: wayIn }];
    for (const s of spawned) {
      actions.push({ do: "spawn", unit: s.unit, def: spec.spawn?.def ?? "", at: s.at });
      reset.push({ do: "despawn", unit: s.unit });
    }
    if (!spec.spawn && boss) {
      actions.push({ do: "aggro", unit: foes[0] }, { do: "camera", mode: "lock", rect: rectName });
      reset.push({ do: "camera", mode: "follow" });
    }
    actions.push({ do: "toast", text: spec.toast ?? "The gate drops behind you." });
    const done: ActionList = [{ do: "unlock", prop: gate }, { do: "hide", prop: wayIn }, { do: "flag", flag: cleared }];
    if (spec.reward) done.push({ do: "unlock", prop: nameOf(node, spec.reward) });
    if (!spec.spawn && boss) done.push({ do: "camera", mode: "follow" });
    triggers[lock] = { zone, rect: rectName, once: true, when: [{ if: "flag", flag: cleared, not: true }, ...foes.slice(0, 1).map((u): Condition => ({ if: "dead", unit: u, not: true }))], actions, reset };
    triggers[clear] = { zone, rect: rectName, mode: "while", once: true, when: foes.map((u): Condition => ({ if: "dead", unit: u })), actions: done };
    info.lockins.push({ node: node.id, gate, wayIn, mark: markName, rect: rectName, lock, clear });
  }

  // --- fill: marks, rects, holdings --------------------------------------------------------
  for (const room of info.rooms) {
    const { node, shape } = room;
    for (const r of shape.rects) {
      const name = nameOf(node, r.id);
      if (!k.rects[name]) k.rect(name, { cx: room.x + r.cx, cy: room.y + r.cy, w: r.w, h: r.h });
    }
    for (const m of shape.marks) k.mark(nameOf(node, m.id), room.x + m.cx, room.y + m.cy);

    const held = new Set<string>();
    for (const hd of node.holds) {
      const s = shape.sockets.find((x) => x.id === hd.socket);
      if (!s) throw new Error(`Dungeon "${zone}": node "${node.id}" holds socket "${hd.socket}", which template ${shape.template.id} does not have`);
      held.add(s.id);
      const key = nameOf(node, s.id);
      if ("unit" in hd) {
        const patrol = (hd.patrol ?? []).map((pt): [number, number, number] => {
          const m = shape.marks.find((x) => x.id === pt.mark);
          if (!m) throw new Error(`Dungeon "${zone}": "${node.id}" patrols by mark "${pt.mark}", which template ${shape.template.id} does not have`);
          return [room.x + m.cx, room.y + m.cy, pt.dwell ?? 0];
        });
        k.unit(key, hd.unit, room.x + s.cx + (s.w >> 1), room.y + s.cy + (s.h >> 1), patrol.length > 1 ? patrol : undefined).phase = def.phase;
        continue;
      }
      const propDef = hd.prop ?? "chest";
      const f = foot[propDef];
      if (!f || f.w !== s.w || f.h !== s.h) throw new Error(`Dungeon "${zone}": "${propDef}" does not fit socket ${s.id} of ${shape.template.id} (${s.w}x${s.h})`);
      const guarded = (hd.guardedBy ?? []).length > 0;
      let use = "use" in hd ? resolve(node, key, hd.use) : undefined;
      const found = FOUND[propDef];
      if (found && !use) {
        use = [{ do: "grow", stat: found.stat, amount: found.amount, id: key }];
        if (found.hide) use.push({ do: "hide", prop: key });
      }
      // The wall notice is the map: reading it shows every room this seed placed. The generator
      // knows them all, so no mission has to list them (and none can list one that was dropped).
      if (propDef === "notice" && !use) use = [{ do: "reveal", rects: info.rooms.map((r) => nameOf(r.node, "room")) }];
      if ("controls" in hd && hd.controls) {
        // One generated list per control, never hand-written, or a gate is forgotten in one direction.
        const sv = stateOf(hd.controls);
        const other = sv.values[0] === sv.initial ? sv.values[1] : sv.values[0];
        const drive = (value: string): ActionList => [
          { do: "flag", flag: sv.flag, value: value === sv.initial ? 0 : 1 },
          ...stateGates.filter((g) => g.state === sv.id).map((g): Action => ({ do: g.is === value ? "unlock" : "lock", prop: g.gate })),
          ...(resolve(node, key, hd.becomes?.[value]) ?? []),
        ];
        use = [{ do: "if", when: [{ if: "flag", flag: sv.flag }], then: drive(sv.initial), else: drive(other) }, ...(use ?? [])];
        info.controls.push({ state: sv.id, prop: key });
      }
      k.prop(
        {
          key,
          def: propDef,
          cx: room.x + s.cx,
          cy: room.y + s.cy,
          locked: hd.locked || guarded || undefined,
          hidden: "hidden" in hd ? hd.hidden : undefined,
          loot: "loot" in hd ? hd.loot : undefined,
          talk: "talk" in hd ? hd.talk : undefined,
          needs: "needs" in hd ? hd.needs : undefined,
          use,
          release: "release" in hd ? resolve(node, key, hd.release) : undefined,
          to: "to" in hd ? hd.to : undefined,
          label: hd.label,
        },
        s.w,
        s.h,
      );
      if (guarded) {
        triggers[`${key}_free`] = {
          zone,
          rect: nameOf(node, "room"),
          mode: "while",
          once: true,
          when: (hd.guardedBy ?? []).map((u): Condition => ({ if: "dead", unit: u.startsWith("@") ? nameOf(node, u.slice(1)) : u })),
          actions: [{ do: "unlock", prop: key }],
        };
      }
    }
    for (const t of node.triggers ?? []) {
      triggers[`${zone}_${node.id}_${t.id}`] = {
        zone,
        rect: nameOf(node, t.rect ?? "room"),
        mode: t.mode,
        once: t.once ?? true,
        when: resolveWhen(node, t.when),
        actions: resolve(node, "", t.actions) ?? [],
        reset: resolve(node, "", t.reset),
      };
    }

    // Things to push that the mission did not name: whatever the template offers, seeded.
    for (const s of shape.sockets) {
      if (held.has(s.id) || s.kind !== "push") continue;
      const options = s.options.filter((o) => foot[o] && foot[o].w === s.w && foot[o].h === s.h);
      if (options.length > 0) k.prop({ key: nameOf(node, s.id), def: k.pick(options), cx: room.x + s.cx, cy: room.y + s.cy }, s.w, s.h);
    }

    fillEnemies(room);

    for (const s of shape.sockets) {
      if (s.kind !== "dress") continue;
      const row = def.dress[s.id.split(":")[1]];
      if (!row || !k.chance(row.chance)) continue;
      const options = row.props.filter((o) => foot[o] && foot[o].w === s.w && foot[o].h === s.h);
      if (options.length > 0) k.prop({ key: nameOf(node, s.id), def: k.pick(options), cx: room.x + s.cx, cy: room.y + s.cy }, s.w, s.h);
    }
  }

  /**
   * Heat (DUNGEONS.md 2.7): a node's enemy cost is its share of the dungeon's base heat,
   * capped by what the template says the room can hold fairly, spent on the dungeon's own
   * bestiary with a seeded mix. At most two to a spawn socket.
   */
  function fillEnemies(room: RoomInfo): void {
    const spawns = room.shape.sockets.filter((s: Socket) => s.kind === "spawn");
    let left = Math.min(Math.round(room.node.heat * def.budget.baseHeat), room.shape.template.heatMax);
    const table = Object.keys(def.budget.enemies).sort();
    const load = spawns.map(() => 0);
    let n = 0;
    while (left > 0) {
      const can = table.filter((id) => def.budget.enemies[id] <= left);
      const open = spawns.map((_, i) => i).filter((i) => load[i] < 2);
      if (can.length === 0 || open.length === 0) break;
      const id = k.pick(can);
      const at = k.pick(open);
      const s = spawns[at];
      // The second of a pair stands one cell along, if that cell is floor; otherwise on the same spot.
      const dx = load[at] === 1 && k.fits(room.x + s.cx + 1, room.y + s.cy, 1, 1) ? 1 : 0;
      k.unit(`${zone}_${room.node.id}_spawn_${n++}`, id, room.x + s.cx + dx, room.y + s.cy).phase = def.phase;
      load[at]++;
      left -= def.budget.enemies[id];
    }
  }

  return finish();
}
