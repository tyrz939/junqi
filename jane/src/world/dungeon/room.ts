// Room templates: a text grid, like the art. One character per cell, a header above, a
// legend below (DUNGEONS.md 2.3). This file parses one, turns and mirrors it, and lints it.
//
//   id      mine.plate.a
//   pool    mine.plate
//   bays    1x1
//   turn    0 180 mirror
//   doors   n1 optional, e1 optional
//   needs   -
//   grants  chest:reward
//   blocks  chest:reward until plate:main
//   coop    plate_or_friend
//   heat    3
//   rect    inner 3 3 20 12        (optional, any number; "room" is always the whole floor)
//   grid
//   ######nnn######
//   #.....___.....#
//   ...
//   legend
//   n door n1
//   P plate:main 2x2
//   B push:main 2x2 barrel | crate
//   r spawn
//   m mark boss
//   : tile Track
//
// `#` wall, `.` floor and `_` sill are built in. One legend entry per line. A socket with a
// name (`plate:main`) is one rectangle; a socket without one (`spawn`) may appear many times
// and is numbered in reading order (`spawn:0`, `spawn:1`).
//
// A template is an INTERFACE: variants of a pool differ in their grid and never in their
// doors' required members, needs, grants or blocks.

import type { CoopTag, Door, RoomTemplate, Side, Socket, SocketKind, TemplateMark, TemplateRect, Turn } from "@/world/dungeon/types";

// --- the lattice a template must fit -------------------------------------------------------

/** A bay, in cells. Corridors run on the lines between bays and never through a room. */
export const BAY_W = 36;
export const BAY_H = 28;
/** Wall kept round the whole lattice, so a corridor can go round the outside. */
export const BORDER = 8;
/** A mouth is three cells, centred on this cell of its bay side. */
export const MOUTH_X = 18;
export const MOUTH_Y = 14;
/** A room's rim stays inside these cells of its bay, which keeps a wall between it and any corridor. */
const RIM_LO = 3;
const RIM_HI_X = 33;
const RIM_HI_Y = 25;

const SOCKET_KINDS: readonly SocketKind[] = [
  "chest", "plate", "push", "carry", "lever", "verbprop", "rest", "jar", "page", "notice", "spawn", "boss", "unit", "dress", "prop", "exit",
];
const COOP_TAGS: readonly CoopTag[] = ["plate_or_friend", "twin_hold", "lure_and_lever", "carry_relay", "guard_the_pusher"];
const SIDES: readonly Side[] = ["n", "e", "s", "w"];

/** Does something stand here that feet cannot cross? Dressing is solid unless it is a torch on the wall. */
export function socketIsSolid(s: Socket): boolean {
  if (s.kind === "plate" || s.kind === "page" || s.kind === "spawn" || s.kind === "boss" || s.kind === "unit") return false;
  if (s.kind === "dress") return !s.id.startsWith("dress:torch");
  return true;
}

// --- parse -------------------------------------------------------------------------------

type LegendEntry =
  | { t: "door"; id: string }
  | { t: "tile"; tile: string }
  | { t: "mark"; id: string }
  | { t: "socket"; kind: SocketKind; name: string | null; w: number; h: number; options: string[] };

export function parseRoom(text: string, file = "?"): RoomTemplate {
  const fail = (msg: string): never => {
    throw new Error(`${file}: ${msg}`);
  };
  const lines = text.replace(/\r/g, "").split("\n");
  const head: Record<string, string[]> = {};
  const grid: string[] = [];
  const legendLines: string[] = [];
  let part: "head" | "grid" | "legend" = "head";
  for (const raw of lines) {
    const line = raw.replace(/\s+$/, "");
    if (part === "head") {
      if (line.trim() === "" || line.trim().startsWith("//")) continue;
      if (line.trim() === "grid") {
        part = "grid";
        continue;
      }
      const m = /^(\w+)\s+(.*)$/.exec(line.trim());
      if (!m) fail(`cannot read header line "${line}"`);
      else (head[m[1]] ??= []).push(m[2].trim());
    } else if (part === "grid") {
      if (line.trim() === "legend") part = "legend";
      else if (line.trim() !== "") grid.push(line);
    } else if (line.trim() !== "" && !line.trim().startsWith("//")) legendLines.push(line.trim());
  }
  const one = (key: string): string => head[key]?.[0] ?? fail(`no "${key}" line`);
  if (grid.length < 5) fail("no grid");
  const gw = grid[0].length;
  for (const [y, row] of grid.entries()) if (row.length !== gw) fail(`grid row ${y} is ${row.length} wide, row 0 is ${gw}`);

  const legend = new Map<string, LegendEntry>();
  for (const l of legendLines) {
    const m = /^(\S)\s+(.*)$/.exec(l);
    if (!m) {
      fail(`cannot read legend line "${l}"`);
      continue;
    }
    const ch = m[1];
    if (ch === "#" || ch === "." || ch === "_") fail(`legend may not redefine "${ch}"`);
    if (legend.has(ch)) fail(`legend defines "${ch}" twice`);
    const words = m[2].trim().split(/\s+/);
    if (words[0] === "door") legend.set(ch, { t: "door", id: words[1] ?? fail(`door "${ch}" has no id`) });
    else if (words[0] === "tile") legend.set(ch, { t: "tile", tile: words[1] ?? fail(`tile "${ch}" has no name`) });
    else if (words[0] === "mark") legend.set(ch, { t: "mark", id: words[1] ?? fail(`mark "${ch}" has no id`) });
    else {
      const [kind, name] = words[0].split(":");
      if (!SOCKET_KINDS.includes(kind as SocketKind)) fail(`legend "${ch}": unknown socket kind "${kind}"`);
      let w = 1;
      let h = 1;
      let rest = words.slice(1);
      const size = /^(\d+)x(\d+)$/.exec(rest[0] ?? "");
      if (size) {
        w = Number(size[1]);
        h = Number(size[2]);
        rest = rest.slice(1);
      }
      const options = rest.join(" ").split("|").map((s) => s.trim()).filter((s) => s !== "");
      legend.set(ch, { t: "socket", kind: kind as SocketKind, name: name ?? null, w, h, options });
    }
  }

  // Doors, as declared in the header, then found on the rim.
  const bays = /^(\d+)x(\d+)$/.exec(one("bays"));
  if (!bays) fail(`bays "${one("bays")}" is not WxH`);
  const bayCols = Number(bays?.[1]);
  const bayRows = Number(bays?.[2]);
  const declared = new Map<string, boolean>();
  for (const d of one("doors").split(",").map((s) => s.trim()).filter((s) => s !== "")) {
    const m = /^([nesw]\d+)\s+(required|optional)$/.exec(d);
    if (!m) fail(`cannot read door "${d}"`);
    else declared.set(m[1], m[2] === "required");
  }
  const doors: Door[] = [];
  const gh = grid.length;
  for (const [ch, e] of legend) {
    if (e.t !== "door") continue;
    const cells: [number, number][] = [];
    for (let y = 0; y < gh; y++) for (let x = 0; x < gw; x++) if (grid[y][x] === ch) cells.push([x, y]);
    if (cells.length !== 3) fail(`door ${e.id} is ${cells.length} cells, not 3`);
    const side = e.id[0] as Side;
    if (!SIDES.includes(side)) fail(`door id "${e.id}" does not start with n, e, s or w`);
    if (!declared.has(e.id)) fail(`door ${e.id} is drawn and not declared`);
    doors.push({ id: e.id, side, bay: Number(e.id.slice(1)) - 1, required: declared.get(e.id) ?? false, cx: cells[1][0], cy: cells[1][1] });
  }
  for (const id of declared.keys()) if (!doors.some((d) => d.id === id)) fail(`door ${id} is declared and not drawn`);
  doors.sort((a, b) => (a.id < b.id ? -1 : 1));

  // Sockets: every connected run of a socket character is one socket.
  const sockets: Socket[] = [];
  const marks: TemplateMark[] = [];
  const tiles: Record<string, string> = {};
  const taken = new Uint8Array(gw * gh);
  const counters: Record<string, number> = {};
  for (let y = 0; y < gh; y++) {
    for (let x = 0; x < gw; x++) {
      const ch = grid[y][x];
      if (ch === "#" || ch === "." || ch === "_" || taken[y * gw + x]) continue;
      const e = legend.get(ch);
      if (!e) {
        fail(`grid (${x},${y}): "${ch}" is not in the legend`);
        continue;
      }
      if (e.t === "door") continue;
      if (e.t === "tile") {
        tiles[ch] = e.tile;
        continue;
      }
      if (e.t === "mark") {
        if (marks.some((m) => m.id === e.id)) fail(`mark "${e.id}" is drawn twice`);
        marks.push({ id: e.id, cx: x, cy: y });
        continue;
      }
      // The rectangle this run fills, by reading order: x is its left edge, y its top.
      let w = 0;
      while (x + w < gw && grid[y][x + w] === ch && !taken[y * gw + x + w] && w < e.w) w++;
      let h = 0;
      while (y + h < gh && grid[y + h][x] === ch && h < e.h) h++;
      if (w !== e.w || h !== e.h) fail(`socket "${ch}" at (${x},${y}) is ${w}x${h}, the legend says ${e.w}x${e.h}`);
      for (let j = y; j < y + h; j++) {
        for (let i = x; i < x + w; i++) {
          if (grid[j][i] !== ch) fail(`socket "${ch}" at (${x},${y}) is not a full ${e.w}x${e.h} rectangle`);
          taken[j * gw + i] = 1;
        }
      }
      let id: string;
      if (e.name !== null && e.kind === "dress") {
        // Dressing is named for what it is (`dress:torch`) and there are many of each.
        const n = counters[`dress:${e.name}`] ?? 0;
        counters[`dress:${e.name}`] = n + 1;
        id = `dress:${e.name}:${n}`;
      } else if (e.name !== null) {
        id = `${e.kind}:${e.name}`;
        if (sockets.some((s) => s.id === id)) fail(`socket "${id}" is drawn twice; only unnamed sockets repeat`);
      } else {
        const n = counters[e.kind] ?? 0;
        counters[e.kind] = n + 1;
        id = `${e.kind}:${n}`;
      }
      sockets.push({ id, kind: e.kind, cx: x, cy: y, w, h, options: e.options });
    }
  }

  const rects: TemplateRect[] = [{ id: "room", cx: 1, cy: 1, w: gw - 2, h: gh - 2 }];
  for (const r of head.rect ?? []) {
    const m = /^(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)$/.exec(r);
    if (!m) fail(`cannot read rect "${r}"`);
    else rects.push({ id: m[1], cx: Number(m[2]), cy: Number(m[3]), w: Number(m[4]), h: Number(m[5]) });
  }

  const turnWords = one("turn").split(/\s+/);
  const turns = turnWords.filter((t) => t !== "mirror").map((t) => Number(t) as Turn);
  for (const t of turns) if (![0, 90, 180, 270].includes(t)) fail(`turn "${t}" is not 0, 90, 180 or 270`);
  if (!turns.includes(0)) fail("turn must include 0");

  const needs: RoomTemplate["needs"] = { verbs: [], items: [] };
  for (const n of (head.needs?.[0] ?? "-").split(",").map((s) => s.trim())) {
    if (n === "-" || n === "") continue;
    const m = /^(verb|item)\s+(\S+)(?:\s+(\d+))?$/.exec(n);
    if (!m) fail(`cannot read need "${n}"`);
    else if (m[1] === "verb") needs.verbs.push(m[2]);
    else needs.items.push({ item: m[2], qty: Number(m[3] ?? 1) });
  }
  const list = (key: string): string[] =>
    (head[key] ?? []).flatMap((v) => v.split(",")).map((s) => s.trim()).filter((s) => s !== "" && s !== "-");
  const blocks = list("blocks").map((b) => {
    const m = /^(\S+)\s+until\s+(\S+)$/.exec(b);
    if (!m) return fail(`cannot read block "${b}"`);
    return { what: m[1], until: m[2] };
  });
  const coop = list("coop") as CoopTag[];
  for (const c of coop) if (!COOP_TAGS.includes(c)) fail(`unknown coop tag "${c}"`);

  return {
    id: one("id"),
    pool: one("pool"),
    bays: [bayCols, bayRows],
    turns,
    mirror: turnWords.includes("mirror"),
    doors,
    sockets,
    marks,
    rects,
    needs,
    grants: list("grants"),
    blocks,
    coop,
    heatMax: Number(head.heat?.[0] ?? 0),
    grid,
    tiles,
  };
}

// --- turn and mirror ---------------------------------------------------------------------

/** A template as it will be stamped: turned, mirrored, and told where in its bays it sits. */
export type Shape = {
  template: RoomTemplate;
  turn: Turn;
  mirror: boolean;
  w: number;
  h: number;
  bays: [number, number];
  /** Rows of characters. Sockets and marks are floor here; they live in the lists. */
  cells: string[];
  doors: Door[];
  sockets: Socket[];
  marks: TemplateMark[];
  rects: TemplateRect[];
  /** Offset of the grid inside its bay group, chosen so every door lies on its bay's mouth line. */
  ox: number;
  oy: number;
  /** False when this transform does not fit its bays, or its doors do not line up. */
  fits: boolean;
};

const CW: Record<Side, Side> = { n: "e", e: "s", s: "w", w: "n" };

export function shapeOf(t: RoomTemplate, turn: Turn, mirror: boolean): Shape {
  let w = t.grid[0].length;
  let h = t.grid.length;
  let bays: [number, number] = [t.bays[0], t.bays[1]];
  // Points first: a function from template coordinates to shape coordinates, built up step by step.
  type Pt = (x: number, y: number) => [number, number];
  let at: Pt = (x, y) => [x, y];
  let doors = t.doors.map((d) => ({ ...d }));
  if (mirror) {
    const w0 = w;
    const cols = bays[0];
    at = (x, y) => [w0 - 1 - x, y];
    doors = doors.map((d) => ({ ...d, side: d.side === "e" ? "w" : d.side === "w" ? "e" : d.side, bay: d.side === "n" || d.side === "s" ? cols - 1 - d.bay : d.bay }));
  }
  for (let q = 0; q < turn / 90; q++) {
    const prev = at;
    const h0 = h;
    const rows = bays[1];
    at = (x, y) => {
      const [px, py] = prev(x, y);
      return [h0 - 1 - py, px];
    };
    doors = doors.map((d) => ({ ...d, side: CW[d.side], bay: d.side === "e" || d.side === "w" ? rows - 1 - d.bay : d.bay }));
    [w, h] = [h, w];
    bays = [bays[1], bays[0]];
  }
  const box = (r: { cx: number; cy: number; w: number; h: number }): { cx: number; cy: number; w: number; h: number } => {
    const [ax, ay] = at(r.cx, r.cy);
    const [bx, by] = at(r.cx + r.w - 1, r.cy + r.h - 1);
    return { cx: Math.min(ax, bx), cy: Math.min(ay, by), w: Math.abs(bx - ax) + 1, h: Math.abs(by - ay) + 1 };
  };
  const rows: string[][] = Array.from({ length: h }, () => Array.from({ length: w }, () => "#"));
  const socketAt = new Set<string>();
  for (const s of t.sockets) for (let j = 0; j < s.h; j++) for (let i = 0; i < s.w; i++) socketAt.add(`${s.cx + i},${s.cy + j}`);
  for (const m of t.marks) socketAt.add(`${m.cx},${m.cy}`);
  for (let y = 0; y < t.grid.length; y++) {
    for (let x = 0; x < t.grid[0].length; x++) {
      const [nx, ny] = at(x, y);
      rows[ny][nx] = socketAt.has(`${x},${y}`) ? "." : t.grid[y][x];
    }
  }
  // In id order, so that two shapes offering the same doors are joined up the same way.
  const outDoors: Door[] = doors
    .map((d) => {
      const [cx, cy] = at(d.cx, d.cy);
      return { ...d, id: `${d.side}${d.bay + 1}`, cx, cy };
    })
    .sort((a, b) => (a.id < b.id ? -1 : 1));
  const shape: Shape = {
    template: t,
    turn,
    mirror,
    w,
    h,
    bays,
    cells: rows.map((r) => r.join("")),
    doors: outDoors,
    sockets: t.sockets.map((s) => ({ ...s, ...box(s) })),
    marks: t.marks.map((m) => {
      const [cx, cy] = at(m.cx, m.cy);
      return { id: m.id, cx, cy };
    }),
    rects: t.rects.map((r) => ({ id: r.id, ...box(r) })),
    ox: 0,
    oy: 0,
    fits: true,
  };

  // Where in its bays: every n/s door fixes x, every e/w door fixes y, and they must agree.
  let ox: number | null = null;
  let oy: number | null = null;
  for (const d of outDoors) {
    if (d.side === "n" || d.side === "s") {
      const want = d.bay * BAY_W + MOUTH_X - d.cx;
      if (ox !== null && ox !== want) shape.fits = false;
      ox = want;
      if (d.cy !== (d.side === "n" ? 0 : h - 1)) shape.fits = false;
    } else {
      const want = d.bay * BAY_H + MOUTH_Y - d.cy;
      if (oy !== null && oy !== want) shape.fits = false;
      oy = want;
      if (d.cx !== (d.side === "w" ? 0 : w - 1)) shape.fits = false;
    }
    if (d.bay < 0 || d.bay >= (d.side === "n" || d.side === "s" ? bays[0] : bays[1])) shape.fits = false;
  }
  shape.ox = ox ?? Math.floor((bays[0] * BAY_W + 1 - w) / 2);
  shape.oy = oy ?? Math.floor((bays[1] * BAY_H + 1 - h) / 2);
  if (shape.ox < RIM_LO || shape.ox + w - 1 > (bays[0] - 1) * BAY_W + RIM_HI_X) shape.fits = false;
  if (shape.oy < RIM_LO || shape.oy + h - 1 > (bays[1] - 1) * BAY_H + RIM_HI_Y) shape.fits = false;
  // A socket that is not square cannot be turned on its side: the prop it holds has one footprint.
  if (turn === 90 || turn === 270) for (const s of t.sockets) if (s.w !== s.h) shape.fits = false;
  return shape;
}

/** Every transform the template allows, in a fixed order. */
export function shapesOf(t: RoomTemplate): Shape[] {
  const out: Shape[] = [];
  for (const turn of t.turns) {
    out.push(shapeOf(t, turn, false));
    if (t.mirror) out.push(shapeOf(t, turn, true));
  }
  return out;
}

// --- lint --------------------------------------------------------------------------------

const INWARD: Record<Side, [number, number]> = { n: [0, 1], s: [0, -1], w: [1, 0], e: [-1, 0] };

/** The six rules of DUNGEONS.md 2.3, and the few the mine added. Empty means clean. */
export function lintRoom(t: RoomTemplate): string[] {
  const errors: string[] = [];
  const gw = t.grid[0].length;
  const gh = t.grid.length;
  const at = (x: number, y: number): string => (x < 0 || y < 0 || x >= gw || y >= gh ? "#" : t.grid[y][x]);
  const doorCells = new Set<string>();
  for (const d of t.doors) {
    const along: [number, number] = d.side === "n" || d.side === "s" ? [1, 0] : [0, 1];
    for (let n = -1; n <= 1; n++) doorCells.add(`${d.cx + along[0] * n},${d.cy + along[1] * n}`);
  }

  // 1. The rim is wall except at declared doors.
  for (let y = 0; y < gh; y++) {
    for (let x = 0; x < gw; x++) {
      const rim = x === 0 || y === 0 || x === gw - 1 || y === gh - 1;
      const ch = at(x, y);
      if (rim && ch !== "#" && !doorCells.has(`${x},${y}`)) errors.push(`rule 1: rim cell (${x},${y}) is "${ch}", not wall or a door`);
      if (!rim && doorCells.has(`${x},${y}`)) errors.push(`rule 1: door cell (${x},${y}) is not on the rim`);
    }
  }
  // A sill inside every door; none anywhere else.
  const sills = new Set<string>();
  for (const d of t.doors) {
    const along: [number, number] = d.side === "n" || d.side === "s" ? [1, 0] : [0, 1];
    const [ix, iy] = INWARD[d.side];
    for (let n = -1; n <= 1; n++) {
      const x = d.cx + along[0] * n + ix;
      const y = d.cy + along[1] * n + iy;
      sills.add(`${x},${y}`);
      if (at(x, y) !== "_") errors.push(`door ${d.id}: cell (${x},${y}) just inside it is "${at(x, y)}", not a sill`);
    }
  }
  for (let y = 0; y < gh; y++) for (let x = 0; x < gw; x++) if (at(x, y) === "_" && !sills.has(`${x},${y}`)) errors.push(`sill at (${x},${y}) is not inside a door`);

  // What stands where, for the path rules.
  const SOLID = 1;
  const PUSH = 2;
  const stand = new Uint8Array(gw * gh);
  for (const s of t.sockets) {
    const v = s.kind === "push" ? PUSH : socketIsSolid(s) ? SOLID : 0;
    if (v) for (let j = 0; j < s.h; j++) for (let i = 0; i < s.w; i++) stand[(s.cy + j) * gw + s.cx + i] = v;
  }
  const floorTile = (x: number, y: number): boolean => {
    const ch = at(x, y);
    // Extra tiles are taken as walkable for the path rules unless they are plainly not.
    return ch !== "#" && !(t.tiles[ch] && /Wall|Water|Rail|Rubble|Fence|Glass|Hedge|Cliff|Void/.test(t.tiles[ch]));
  };

  // 2. Sockets on floor, clear of sills and of the straight line between facing doors.
  for (const s of t.sockets) {
    for (let j = 0; j < s.h; j++) {
      for (let i = 0; i < s.w; i++) {
        const x = s.cx + i;
        const y = s.cy + j;
        if (x <= 0 || y <= 0 || x >= gw - 1 || y >= gh - 1) errors.push(`rule 2: socket ${s.id} touches the rim at (${x},${y})`);
        if (!socketIsSolid(s)) continue;
        for (const [dx, dy] of [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]]) {
          if (at(x + dx, y + dy) === "_") errors.push(`rule 2: solid socket ${s.id} touches the sill at (${x + dx},${y + dy})`);
        }
      }
    }
  }
  for (const a of t.doors) {
    for (const b of t.doors) {
      if (a.id >= b.id) continue;
      const facing = (a.side === "n" && b.side === "s") || (a.side === "s" && b.side === "n") || (a.side === "e" && b.side === "w") || (a.side === "w" && b.side === "e");
      if (!facing) continue;
      const vertical = a.side === "n" || a.side === "s";
      if (vertical ? a.cx !== b.cx : a.cy !== b.cy) continue;
      for (const s of t.sockets) {
        if (!socketIsSolid(s) || s.kind === "push") continue;
        const hit = vertical ? s.cx <= a.cx + 1 && s.cx + s.w - 1 >= a.cx - 1 : s.cy <= a.cy + 1 && s.cy + s.h - 1 >= a.cy - 1;
        if (hit) errors.push(`rule 2: solid socket ${s.id} stands in the line between doors ${a.id} and ${b.id}`);
      }
    }
  }

  // 3. Nobody is hit in a doorway.
  for (const s of t.sockets) {
    if (s.kind !== "spawn" && s.kind !== "boss" && s.kind !== "unit") continue;
    for (const d of t.doors) {
      const dist = Math.max(Math.abs(s.cx - d.cx), Math.abs(s.cy - d.cy));
      if (dist < 4) errors.push(`rule 3: ${s.id} is ${dist} cells from door ${d.id}, under 4`);
    }
  }

  // 4. Two cells of clear width between every pair of doors, push props where they start.
  if (t.doors.length > 1) {
    const wide = (x: number, y: number): boolean => {
      for (let j = 0; j < 2; j++) for (let i = 0; i < 2; i++) if (!floorTile(x + i, y + j) || stand[(y + j) * gw + x + i] !== 0) return false;
      return true;
    };
    const startOf = (d: Door): [number, number] => {
      // The 2 x 2 block whose far corner is the door's centre cell, reaching into the room.
      const [ix, iy] = INWARD[d.side];
      return [Math.min(d.cx, d.cx + ix), Math.min(d.cy, d.cy + iy)];
    };
    const seen = new Uint8Array(gw * gh);
    const queue: [number, number][] = [startOf(t.doors[0])];
    if (wide(queue[0][0], queue[0][1])) seen[queue[0][1] * gw + queue[0][0]] = 1;
    else queue.length = 0;
    for (let n = 0; n < queue.length; n++) {
      const [x, y] = queue[n];
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= gw - 1 || ny >= gh - 1 || seen[ny * gw + nx] || !wide(nx, ny)) continue;
        seen[ny * gw + nx] = 1;
        queue.push([nx, ny]);
      }
    }
    for (const d of t.doors) {
      const [x, y] = startOf(d);
      if (!seen[y * gw + x]) errors.push(`rule 4: no path two cells wide from door ${t.doors[0].id} to door ${d.id}`);
    }
  }

  // Every socket can be walked up to from every door.
  if (t.doors.length > 0) {
    const seen = new Uint8Array(gw * gh);
    const queue: [number, number][] = [[t.doors[0].cx, t.doors[0].cy]];
    seen[queue[0][1] * gw + queue[0][0]] = 1;
    for (let n = 0; n < queue.length; n++) {
      const [x, y] = queue[n];
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= gw || ny >= gh || seen[ny * gw + nx]) continue;
        // A push prop is in the way only until it is pushed.
        if (!floorTile(nx, ny) || stand[ny * gw + nx] === SOLID) continue;
        seen[ny * gw + nx] = 1;
        queue.push([nx, ny]);
      }
    }
    for (const s of t.sockets) {
      let near = false;
      for (let j = -1; j <= s.h && !near; j++) for (let i = -1; i <= s.w && !near; i++) near = seen[(s.cy + j) * gw + s.cx + i] === 1;
      if (!near) errors.push(`socket ${s.id} cannot be walked up to`);
    }
    for (const m of t.marks) if (!seen[m.cy * gw + m.cx]) errors.push(`mark ${m.id} cannot be walked to`);
  }

  // 5. A plate has a push prop in the room, and the prop can be pushed onto it.
  const plates = t.sockets.filter((s) => s.kind === "plate");
  const pushes = t.sockets.filter((s) => s.kind === "push");
  for (const plate of plates) {
    const ok = pushes.some((p) => pushPath(gw, gh, p, plate, (x, y) => floorTile(x, y) && at(x, y) !== "_" && (stand[y * gw + x] === 0 || inside(p, x, y))));
    if (!ok) errors.push(`rule 5: nothing in the room can be pushed onto ${plate.id}`);
  }
  if (pushes.length < plates.length) errors.push(`rule 5: ${plates.length} plates and only ${pushes.length} things to push`);

  // A hoist must not drop on whoever pulls its lever.
  for (const r of t.rects) {
    if (!r.id.startsWith("drop")) continue;
    for (const s of t.sockets) {
      if (s.kind !== "lever") continue;
      if (s.cx >= r.cx - 1 && s.cy >= r.cy - 1 && s.cx + s.w <= r.cx + r.w + 1 && s.cy + s.h <= r.cy + r.h + 1) errors.push(`${s.id} stands in or beside ${r.id}: the puller would be under it`);
    }
  }
  for (const r of t.rects) {
    if (r.cx < 1 || r.cy < 1 || r.cx + r.w > gw - 1 || r.cy + r.h > gh - 1) errors.push(`rect ${r.id} reaches the rim`);
  }
  for (const g of t.grants) if (!t.sockets.some((s) => s.id === g)) errors.push(`grants names "${g}", which is not a socket`);
  for (const b of t.blocks) {
    if (!t.sockets.some((s) => s.id === b.what)) errors.push(`blocks names "${b.what}", which is not a socket`);
    if (!t.sockets.some((s) => s.id === b.until)) errors.push(`blocks names "${b.until}", which is not a socket`);
  }

  // 6. Dressing stands where somebody would have put it. A pile, a shelf or a cot is against a
  // wall (a shelf with its back to it); nothing dressed stands within two cells of a doorway;
  // and no lamp is dressing at all, because lamps go on the walls by rule (lights.ts).
  for (const s of t.sockets) {
    if (s.kind !== "dress") continue;
    const name = s.id.split(":")[1];
    if (name === "torch") errors.push(`rule 6: ${s.id} is a lamp on the floor; lamps hang on walls by rule (lights.ts)`);
    const row = (y: number): boolean => Array.from({ length: s.w }, (_, i) => at(s.cx + i, y) === "#").every(Boolean);
    const col = (x: number): boolean => Array.from({ length: s.h }, (_, j) => at(x, s.cy + j) === "#").every(Boolean);
    const backed = s.w >= s.h ? row(s.cy - 1) || row(s.cy + s.h) : col(s.cx - 1) || col(s.cx + s.w);
    let touches = false;
    for (let j = 0; j < s.h; j++) touches ||= at(s.cx - 1, s.cy + j) === "#" || at(s.cx + s.w, s.cy + j) === "#";
    for (let i = 0; i < s.w; i++) touches ||= at(s.cx + i, s.cy - 1) === "#" || at(s.cx + i, s.cy + s.h) === "#";
    if (name === "shelf" && !backed) errors.push(`rule 6: ${s.id} at (${s.cx},${s.cy}) does not have its back to a wall`);
    else if ((name === "pile" || name === "cot") && !touches) errors.push(`rule 6: ${s.id} at (${s.cx},${s.cy}) stands out in the room, not against a wall`);
    if (!socketIsSolid(s)) continue;
    for (const key of sills) {
      const [sx, sy] = key.split(",").map(Number);
      const dx = Math.max(s.cx - sx, 0, sx - (s.cx + s.w - 1));
      const dy = Math.max(s.cy - sy, 0, sy - (s.cy + s.h - 1));
      if (Math.max(dx, dy) <= 2) {
        errors.push(`rule 6: ${s.id} at (${s.cx},${s.cy}) crowds the doorway at (${sx},${sy})`);
        break;
      }
    }
  }

  // Every transform it claims must fit its bays with its doors on the mouth lines.
  for (const s of shapesOf(t)) if (!s.fits) errors.push(`turn ${s.turn}${s.mirror ? " mirrored" : ""} does not fit ${s.bays[0]}x${s.bays[1]} bays with its doors on the mouth lines`);
  return errors;
}

function inside(s: { cx: number; cy: number; w: number; h: number }, x: number, y: number): boolean {
  return x >= s.cx && y >= s.cy && x < s.cx + s.w && y < s.cy + s.h;
}

/**
 * Can the footprint `from` be pushed until it overlaps `to`? A flood of the footprint's
 * position; each step needs the new footprint clear and one clear cell behind it for whoever
 * is pushing. It does not prove she can walk round to get behind it. `clear` must count the
 * prop's own starting cells as clear.
 */
export function pushPath(
  gw: number,
  gh: number,
  from: { cx: number; cy: number; w: number; h: number },
  to: { cx: number; cy: number; w: number; h: number },
  clear: (x: number, y: number) => boolean,
): boolean {
  const { w, h } = from;
  const fits = (x: number, y: number): boolean => {
    for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) if (x + i < 0 || y + j < 0 || x + i >= gw || y + j >= gh || !clear(x + i, y + j)) return false;
    return true;
  };
  const overlaps = (x: number, y: number): boolean => x < to.cx + to.w && x + w > to.cx && y < to.cy + to.h && y + h > to.cy;
  const seen = new Set<number>([from.cy * gw + from.cx]);
  const queue: [number, number][] = [[from.cx, from.cy]];
  for (let n = 0; n < queue.length; n++) {
    const [x, y] = queue[n];
    if (overlaps(x, y)) return true;
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      const nx = x + dx;
      const ny = y + dy;
      if (seen.has(ny * gw + nx) || !fits(nx, ny)) continue;
      // Somewhere to stand on the side she pushes from.
      let behind = false;
      if (dx !== 0) for (let j = 0; j < h && !behind; j++) behind = clear(dx > 0 ? x - 1 : x + w, y + j);
      else for (let i = 0; i < w && !behind; i++) behind = clear(x + i, dy > 0 ? y - 1 : y + h);
      if (!behind) continue;
      seen.add(ny * gw + nx);
      queue.push([nx, ny]);
    }
  }
  return false;
}
