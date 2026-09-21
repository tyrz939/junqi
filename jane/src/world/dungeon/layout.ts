// From graph to bays (DUNGEONS.md 2.5, steps 1 to 3). Nothing here touches a cell: a layout
// is which template fills which node, in which bays, turned how, and which lane each
// corridor runs along. Cells come later, in generate.ts.
//
// The lattice is a grid of bays. Corridors run on the LINES between bays, three cells wide,
// and a room's rim keeps its distance from them, so a corridor can never break into a room.
// The lines make a small graph:
//
//        corner ---- mid ---- corner        a `mid` is the middle of one bay side: the port
//          |                    |           of the door on that side (of either bay)
//         mid       bay        mid          a `corner` is where lines cross
//          |                    |
//        corner ---- mid ---- corner
//
// Two doors that face each other across a line share a port: a straight stub. Anything else
// (a cycle closer, a shortcut) is routed mid to corner to corner to mid. A lane node belongs
// to one corridor only, so corridors never merge or cross, which is what keeps a lock a lock.
//
// Every choice comes from the Kit's stream, lists are in a fixed order before any pick, and
// nothing iterates a Map or a Set.

import type { Kit } from "@/world/kit";
import { BAY_H, BAY_W, BORDER, MOUTH_X, MOUTH_Y, shapesOf, type Shape } from "@/world/dungeon/room";
import { poolOf, templateById } from "@/world/dungeon/pools";
import type { Corridor, Door, DungeonDef, Layout, MissionNode, Placement } from "@/world/dungeon/types";

/** Whole embeddings tried per attempt before the attempt is given up. They cost no cells. */
const EMBED_TRIES = 4;
/** Rooms put down, in all, by one embedding before it is given up; and how many places are tried for one room. */
const EMBED_BUDGET = 160;
const EMBED_WIDTH = 6;

export class Lanes {
  readonly cols: number;
  readonly rows: number;
  readonly count: number;
  readonly used: Uint8Array;
  private readonly corners: number;
  private readonly hmids: number;
  private readonly links: number[][];

  constructor(cols: number, rows: number) {
    this.cols = cols;
    this.rows = rows;
    this.corners = (cols + 1) * (rows + 1);
    this.hmids = cols * (rows + 1);
    this.count = this.corners + this.hmids + (cols + 1) * rows;
    this.used = new Uint8Array(this.count);
    this.links = Array.from({ length: this.count }, () => []);
    const link = (a: number, b: number): void => {
      this.links[a].push(b);
      this.links[b].push(a);
    };
    for (let j = 0; j <= rows; j++) for (let i = 0; i < cols; i++) {
      link(this.hmid(i, j), this.corner(i, j));
      link(this.hmid(i, j), this.corner(i + 1, j));
    }
    for (let j = 0; j < rows; j++) for (let i = 0; i <= cols; i++) {
      link(this.vmid(i, j), this.corner(i, j));
      link(this.vmid(i, j), this.corner(i, j + 1));
    }
  }

  corner(i: number, j: number): number {
    return j * (this.cols + 1) + i;
  }
  /** Middle of the horizontal line above bay row j, over bay column i. */
  hmid(i: number, j: number): number {
    return this.corners + j * this.cols + i;
  }
  /** Middle of the vertical line left of bay column i, beside bay row j. */
  vmid(i: number, j: number): number {
    return this.corners + this.hmids + j * (this.cols + 1) + i;
  }

  /** Centre cell of a lane node, in zone cells. */
  cell(n: number): [number, number] {
    if (n < this.corners) return [BORDER + (n % (this.cols + 1)) * BAY_W, BORDER + Math.floor(n / (this.cols + 1)) * BAY_H];
    if (n < this.corners + this.hmids) {
      const m = n - this.corners;
      return [BORDER + (m % this.cols) * BAY_W + MOUTH_X, BORDER + Math.floor(m / this.cols) * BAY_H];
    }
    const m = n - this.corners - this.hmids;
    return [BORDER + (m % (this.cols + 1)) * BAY_W, BORDER + Math.floor(m / (this.cols + 1)) * BAY_H + MOUTH_Y];
  }

  /** The port a door opens onto, for a room whose first bay is (bx, by). */
  port(bx: number, by: number, bays: [number, number], d: Door): number {
    if (d.side === "n") return this.hmid(bx + d.bay, by);
    if (d.side === "s") return this.hmid(bx + d.bay, by + bays[1]);
    if (d.side === "w") return this.vmid(bx, by + d.bay);
    return this.vmid(bx + bays[0], by + d.bay);
  }

  /** Lines that run through the inside of a room of more than one bay carry nothing. */
  blockInside(bx: number, by: number, bays: [number, number], value: number): void {
    for (let j = by; j < by + bays[1]; j++) for (let i = bx + 1; i < bx + bays[0]; i++) this.used[this.vmid(i, j)] = value;
    for (let j = by + 1; j < by + bays[1]; j++) for (let i = bx; i < bx + bays[0]; i++) this.used[this.hmid(i, j)] = value;
    for (let j = by + 1; j < by + bays[1]; j++) for (let i = bx + 1; i < bx + bays[0]; i++) this.used[this.corner(i, j)] = value;
  }

  /** A corridor already runs where the inside of this room would be. */
  insideTaken(bx: number, by: number, bays: [number, number]): boolean {
    for (let j = by; j < by + bays[1]; j++) for (let i = bx + 1; i < bx + bays[0]; i++) if (this.used[this.vmid(i, j)]) return true;
    for (let j = by + 1; j < by + bays[1]; j++) for (let i = bx; i < bx + bays[0]; i++) if (this.used[this.hmid(i, j)]) return true;
    for (let j = by + 1; j < by + bays[1]; j++) for (let i = bx + 1; i < bx + bays[0]; i++) if (this.used[this.corner(i, j)]) return true;
    return false;
  }

  /** Breadth-first from a port over free lane nodes. `prev[n]` is -1 where it never got. */
  flood(from: number): { dist: Int16Array; prev: Int16Array } {
    const dist = new Int16Array(this.count).fill(-1);
    const prev = new Int16Array(this.count).fill(-1);
    if (this.used[from]) return { dist, prev };
    const queue = [from];
    dist[from] = 0;
    for (let q = 0; q < queue.length; q++) {
      const n = queue[q];
      for (const m of this.links[n]) {
        if (dist[m] >= 0 || this.used[m]) continue;
        dist[m] = dist[n] + 1;
        prev[m] = n;
        queue.push(m);
      }
    }
    return { dist, prev };
  }
}

/** Floods from a port, by port, for as long as the lanes do not change. */
type FloodCache = ({ dist: Int16Array; prev: Int16Array } | undefined)[];

type Placed = { node: MissionNode; shape: Shape; bx: number; by: number; usedDoors: string[] };

type State = {
  def: DungeonDef;
  lanes: Lanes;
  owner: Int8Array; // bay -> index into `placed`, or -1
  placed: Placed[];
  corridors: Corridor[];
  /** Worked out once per embedding, because the search asks thousands of times: */
  /** node id -> the edges that need a corridor from it (a `sight` edge rides on another edge's). */
  edgesOf: Record<string, number[]>;
  /** node id -> how many of those lead to a critical node. */
  criticalEdges: Record<string, string[]>;
  /** node id -> where it stands, while it stands. */
  where: Record<string, Placed | undefined>;
};

function newState(def: DungeonDef): State {
  const edgesOf: Record<string, number[]> = {};
  const criticalEdges: Record<string, string[]> = {};
  for (const n of def.nodes) {
    edgesOf[n.id] = [];
    criticalEdges[n.id] = [];
  }
  def.edges.forEach((e, i) => {
    if (e.kind.t === "sight") return;
    edgesOf[e.from].push(i);
    edgesOf[e.to].push(i);
    if (def.nodes.find((n) => n.id === e.to)?.critical) criticalEdges[e.from].push(e.to);
    if (def.nodes.find((n) => n.id === e.from)?.critical) criticalEdges[e.to].push(e.from);
  });
  return { def, lanes: new Lanes(def.lattice.cols, def.lattice.rows), owner: new Int8Array(def.lattice.cols * def.lattice.rows).fill(-1), placed: [], corridors: [], edgesOf, criticalEdges, where: {} };
}

function placedOf(s: State, id: string): Placed | undefined {
  return s.where[id];
}

/**
 * Join a newly placed room to every placed neighbour. For each edge, the pair of free
 * doors with the shortest lane between them; a tie goes to the earlier door. Returns the
 * corridors made (already marked on the lanes), or null if some edge could not be joined,
 * in which case the lanes are left as they were.
 */
function connect(s: State, me: Placed, floods?: FloodCache): Corridor[] | null {
  const made: Corridor[] = [];
  const undo = (): null => {
    disconnect(s, me, made);
    return null;
  };
  for (const ei of s.edgesOf[me.node.id]) {
    const e = s.def.edges[ei];
    const other = placedOf(s, e.from === me.node.id ? e.to : e.from);
    if (!other || other === me) continue;
    let best: { mine: Door; theirs: Door; lane: number[] } | null = null;
    for (const mine of me.shape.doors) {
      if (me.usedDoors.includes(mine.id)) continue;
      const from = s.lanes.port(me.bx, me.by, me.shape.bays, mine);
      // The first corridor of a one-bay room is routed over lanes nobody has touched since the
      // cache was started, so a flood from the same port is the same flood.
      const cached = floods !== undefined && made.length === 0 && me.shape.bays[0] * me.shape.bays[1] === 1;
      const { dist, prev } = cached ? (floods[from] ??= s.lanes.flood(from)) : s.lanes.flood(from);
      for (const theirs of other.shape.doors) {
        if (other.usedDoors.includes(theirs.id)) continue;
        const to = s.lanes.port(other.bx, other.by, other.shape.bays, theirs);
        if (dist[to] < 0) continue;
        if (best && best.lane.length <= dist[to] + 1) continue;
        const lane: number[] = [];
        for (let n = to; n >= 0; n = prev[n]) lane.push(n);
        lane.reverse();
        best = { mine, theirs, lane };
      }
    }
    if (!best) return undo();
    for (const n of best.lane) s.lanes.used[n] = 1;
    me.usedDoors.push(best.mine.id);
    other.usedDoors.push(best.theirs.id);
    const mineFirst = e.from === me.node.id;
    const a = { node: me.node.id, door: best.mine.id };
    const b = { node: other.node.id, door: best.theirs.id };
    made.push({ edge: ei, a: mineFirst ? a : b, b: mineFirst ? b : a, lane: mineFirst ? best.lane : [...best.lane].reverse() });
  }
  return made;
}

/** Take back what `connect` did for one room: free its lanes and the doors it used on its neighbours. */
function disconnect(s: State, me: Placed, made: Corridor[]): void {
  for (const c of made) for (const n of c.lane) s.lanes.used[n] = 0;
  for (const c of made) {
    const other = placedOf(s, c.a.node === me.node.id ? c.b.node : c.a.node);
    const theirs = c.a.node === me.node.id ? c.b.door : c.a.door;
    if (other) other.usedDoors.splice(other.usedDoors.indexOf(theirs), 1);
  }
  me.usedDoors.length = 0;
}

/** Edges of a placed node that lead to a critical node not placed yet. */
function pendingOf(s: State, p: Placed): number {
  let n = 0;
  for (const other of s.criticalEdges[p.node.id]) if (!s.where[other]) n++;
  return n;
}

/** A placed room that still owes doors to rooms not yet placed must have them, on free ports. */
function starved(s: State): boolean {
  for (const p of s.placed) {
    const owes = pendingOf(s, p);
    if (owes === 0) continue;
    let free = 0;
    for (const d of p.shape.doors) {
      if (!p.usedDoors.includes(d.id) && !s.lanes.used[s.lanes.port(p.bx, p.by, p.shape.bays, d)]) free++;
    }
    if (free < owes) return true;
  }
  return false;
}

function bayFree(s: State, bx: number, by: number, bays: [number, number]): boolean {
  if (bx < 0 || by < 0 || bx + bays[0] > s.lanes.cols || by + bays[1] > s.lanes.rows) return false;
  for (let j = by; j < by + bays[1]; j++) for (let i = bx; i < bx + bays[0]; i++) if (s.owner[j * s.lanes.cols + i] >= 0) return false;
  // A room of more than one bay swallows the lines between its bays, so nothing may be running on them.
  return !s.lanes.insideTaken(bx, by, bays);
}

function put(s: State, p: Placed): void {
  const index = s.placed.length;
  s.placed.push(p);
  s.where[p.node.id] = p;
  for (let j = p.by; j < p.by + p.shape.bays[1]; j++) for (let i = p.bx; i < p.bx + p.shape.bays[0]; i++) s.owner[j * s.lanes.cols + i] = index;
  s.lanes.blockInside(p.bx, p.by, p.shape.bays, 1);
}

function take(s: State): void {
  const p = s.placed.pop() as Placed;
  s.where[p.node.id] = undefined;
  for (let j = p.by; j < p.by + p.shape.bays[1]; j++) for (let i = p.bx; i < p.bx + p.shape.bays[0]; i++) s.owner[j * s.lanes.cols + i] = -1;
  s.lanes.blockInside(p.bx, p.by, p.shape.bays, 0);
}

function bayDistance(a: Placed, bx: number, by: number): number {
  return Math.abs(a.bx - bx) + Math.abs(a.by - by);
}

type Candidate = { shapes: Shape[]; bx: number; by: number; score: number };

/** The bay a door looks into, which may be off the lattice. */
function facingBay(p: Placed, d: Door): [number, number] {
  if (d.side === "n") return [p.bx + d.bay, p.by - 1];
  if (d.side === "s") return [p.bx + d.bay, p.by + p.shape.bays[1]];
  if (d.side === "w") return [p.bx - 1, p.by + d.bay];
  return [p.bx + p.shape.bays[0], p.by + d.bay];
}

/**
 * Room for what is still to come: for every placed room that owes doors to rooms not yet
 * placed, how many of its free doors look straight into a free bay. A door that does not
 * can still be used, by a corridor that goes round; those are what fill the lanes up.
 */
function roomToGrow(s: State): number {
  let score = 0;
  for (const p of s.placed) {
    const owes = pendingOf(s, p);
    if (owes === 0) continue;
    let straight = 0;
    for (const d of p.shape.doors) {
      if (p.usedDoors.includes(d.id) || s.lanes.used[s.lanes.port(p.bx, p.by, p.shape.bays, d)]) continue;
      const [fx, fy] = facingBay(p, d);
      if (bayFree(s, fx, fy, [1, 1])) straight++;
    }
    score += 2 * Math.min(straight, owes) - 4 * Math.max(0, owes - straight);
  }
  return score;
}

/** Shapes that offer the same doors are the same shape to the layout: judge one, then pick among them. */
function byDoors(shapes: Shape[]): Shape[][] {
  const groups: { key: string; shapes: Shape[] }[] = [];
  for (const shape of shapes) {
    const key = `${shape.bays[0]}x${shape.bays[1]}:${shape.doors.map((d) => `${d.id}${d.required ? "!" : ""}`).sort().join(",")}`;
    const g = groups.find((x) => x.key === key);
    if (g) g.shapes.push(shape);
    else groups.push({ key, shapes: [shape] });
  }
  return groups.map((g) => g.shapes);
}

function candidatesFor(s: State, node: MissionNode, shapes: Shape[], first: boolean): Candidate[] {
  const out: Candidate[] = [];
  const { cols, rows } = s.lanes;
  const entrance = s.placed[0];
  const hub = s.placed.find((p) => p.node.kind === "hub");
  const floods: FloodCache = [];
  for (const group of byDoors(shapes)) {
    const shape = group[0];
    for (let by = 0; by + shape.bays[1] <= rows; by++) {
      for (let bx = 0; bx + shape.bays[0] <= cols; bx++) {
        if (!bayFree(s, bx, by, shape.bays)) continue;
        // The way in from outside is on an edge of the lattice, as a mine mouth is on the side of its hill.
        if (first && bx > 0 && by > 0 && bx + shape.bays[0] < cols && by + shape.bays[1] < rows) continue;
        const me: Placed = { node, shape, bx, by, usedDoors: [] };
        put(s, me);
        const made = connect(s, me, floods);
        if (made) {
          let ok = !starved(s);
          // Every door the template insists on must be spoken for by the time its node has all its edges.
          if (ok && pendingOf(s, me) === 0) ok = shape.doors.every((d) => !d.required || me.usedDoors.includes(d.id));
          if (ok) {
            let score = roomToGrow(s);
            for (const c of made) score -= 8 * (c.lane.length - 1);
            if (node.kind === "boss" && entrance) score += 2 * bayDistance(entrance, bx, by);
            if (node.kind === "rest" && hub) score -= 2 * bayDistance(hub, bx, by);
            // The rule Gungeon uses: when this room is one end of a cycle still open, sit near the other end.
            for (const e of s.def.edges) {
              const via = e.from === node.id ? e.to : e.to === node.id ? e.from : null;
              if (!via || placedOf(s, via) || e.kind.t === "sight") continue;
              for (const f of s.def.edges) {
                const far = f.from === via ? f.to : f.to === via ? f.from : null;
                const farPlaced = far && far !== node.id ? placedOf(s, far) : undefined;
                if (farPlaced && farPlaced !== me) score -= Math.max(0, bayDistance(farPlaced, bx, by) - 2);
              }
            }
            out.push({ shapes: group, bx, by, score });
          }
          disconnect(s, me, made);
        }
        take(s);
      }
    }
  }
  return out;
}

/**
 * Critical nodes by `order`, each only once a neighbour of it stands; then the side rooms.
 * One exception to the order: a room of more than one bay goes down as soon as a neighbour
 * of it stands. The big rooms are the hard ones to fit, and a lattice that is already full of
 * small rooms has nowhere left for a boss arena; placed early, they shape everything else.
 */
function placingOrder(def: DungeonDef): MissionNode[] {
  const critical = def.nodes.filter((n) => n.critical).sort((a, b) => a.order - b.order || def.nodes.indexOf(a) - def.nodes.indexOf(b));
  const big = (n: MissionNode): boolean => poolOf(n.pool).some((t) => t.bays[0] * t.bays[1] > 1);
  const out: MissionNode[] = [];
  const rest = [...critical];
  const joined = (n: MissionNode): boolean =>
    def.edges.some((e) => e.kind.t !== "sight" && ((e.from === n.id && out.some((o) => o.id === e.to)) || (e.to === n.id && out.some((o) => o.id === e.from))));
  while (rest.length > 0) {
    let at = out.length === 0 ? 0 : rest.findIndex((n) => big(n) && joined(n));
    if (at < 0) at = rest.findIndex(joined);
    if (at < 0) at = 0;
    out.push(rest.splice(at, 1)[0]);
  }
  return [...out, ...def.nodes.filter((n) => !n.critical)];
}

function shuffled<T>(k: Kit, list: readonly T[]): T[] {
  const out = [...list];
  for (let i = out.length - 1; i > 0; i--) {
    const j = k.roll(i);
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

/**
 * One embedding: place the nodes in order, depth first. At each node the candidates are
 * scored and the best three are tried in a seeded order, then the rest by score; a node
 * with nowhere to go sends the search back to re-place the one before it (Gungeon backs up
 * and re-picks too). `budget` bounds the whole search, so a hopeless start is given up.
 */
function tryEmbed(def: DungeonDef, k: Kit): Layout | null {
  const s = newState(def);
  const usedTemplates: string[] = [];
  const dropped: string[] = [];
  // Side rooms are placed until the budget's upper end is met; one that does not fit is left out,
  // and the layout only fails if fewer than the lower end found a place.
  const sideMax = def.budget.sideRooms[1];
  const order = placingOrder(def);
  let sides = 0;
  let budget = EMBED_BUDGET;

  const place = (at: number): boolean => {
    if (at === order.length) return sides >= def.budget.sideRooms[0];
    const node = order[at];
    if (!node.critical && sides >= sideMax) {
      dropped.push(node.id);
      if (place(at + 1)) return true;
      dropped.pop();
      return false;
    }
    // Choose: a template of the pool not used yet in this dungeon (no room twice), seeded. One
    // that has nowhere to go at all is re-picked, up to three times; one that had somewhere to
    // go and led nowhere is not, or every dead end would be explored once per variant.
    const pool = shuffled(k, poolOf(node.pool).filter((t) => !usedTemplates.includes(t.id))).slice(0, 3);
    for (const template of pool) {
      const found = candidatesFor(s, node, shapesOf(template).filter((x) => x.fits), s.placed.length === 0);
      if (found.length === 0) continue;
      // The sort is total: score, then position, then the turn of the first shape.
      found.sort((a, b) => b.score - a.score || a.by - b.by || a.bx - b.bx || a.shapes[0].turn - b.shapes[0].turn || Number(a.shapes[0].mirror) - Number(b.shapes[0].mirror));
      const tries = [...shuffled(k, found.slice(0, 3)), ...found.slice(3, EMBED_WIDTH)];
      for (const pick of tries) {
        if (budget-- <= 0) return false;
        const me: Placed = { node, shape: k.pick(pick.shapes), bx: pick.bx, by: pick.by, usedDoors: [] };
        put(s, me);
        const made = connect(s, me);
        if (!made) throw new Error("a candidate that connected once must connect again");
        const before = s.corridors.length;
        s.corridors.push(...made);
        usedTemplates.push(template.id);
        if (!node.critical) sides++;
        if (place(at + 1)) return true;
        if (!node.critical) sides--;
        usedTemplates.pop();
        s.corridors.length = before;
        disconnect(s, me, made);
        take(s);
      }
      break;
    }
    // A side room that does not fit this seed is left out. A critical one sends the search back.
    if (node.critical) return false;
    dropped.push(node.id);
    if (place(at + 1)) return true;
    dropped.pop();
    return false;
  };
  return place(0) ? finish(s, dropped, false) : null;
}

function finish(s: State, dropped: string[], fallback: boolean): Layout {
  const placements: Placement[] = s.placed.map((p) => ({ node: p.node.id, template: p.shape.template.id, bay: [p.bx, p.by], turn: p.shape.turn, mirror: p.shape.mirror }));
  return { placements, corridors: [...s.corridors].sort((a, b) => a.edge - b.edge), dropped, fallback };
}

export function embed(def: DungeonDef, k: Kit): Layout | null {
  for (let t = 0; t < EMBED_TRIES; t++) {
    const layout = tryEmbed(def, k);
    if (layout) return layout;
  }
  return null;
}

/** The hand-placed embedding: the same joining of doors, with nothing left to chance. Throws if the author got it wrong. */
export function embedFallback(def: DungeonDef): Layout {
  const s = newState(def);
  for (const node of placingOrder(def)) {
    const row = def.fallback.find((f) => f.node === node.id);
    if (!row) {
      if (node.critical) throw new Error(`Dungeon "${def.id}": the fallback does not place "${node.id}"`);
      continue;
    }
    const template = templateById(row.template);
    const shape = template ? shapesOf(template).find((x) => x.turn === row.turn && x.mirror === row.mirror && x.fits) : undefined;
    if (!shape || !bayFree(s, row.bay[0], row.bay[1], shape.bays)) throw new Error(`Dungeon "${def.id}": fallback row for "${node.id}" does not fit`);
    const me: Placed = { node, shape, bx: row.bay[0], by: row.bay[1], usedDoors: [] };
    put(s, me);
    const made = connect(s, me);
    if (!made) throw new Error(`Dungeon "${def.id}": fallback cannot join "${node.id}" to its neighbours`);
    s.corridors.push(...made);
  }
  const dropped = def.nodes.filter((n) => !placedOf(s, n.id)).map((n) => n.id);
  return finish(s, dropped, true);
}
