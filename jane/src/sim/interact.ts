// The player's USE verb and the world verbs behind it. 2020's B button was
// "Use / Talk / Push / Pull" and so is this: one entry point, capability rows
// on the prop decide what happens.
//
//   tap USE    pick up drop | talk | unlock / open / travel | loot | carry | use[]
//   hold USE   against a `push` prop: push it one cell (or pull, moving away)
//   Repair     a world-kind spell: nearest prop that `answers: "repair"`, pays `needs`
//   Icebolt    any frost bolt lights props that `answers: "frost"` (2020's blue torches)

import { CELL, PX_PER_METRE, USE_REACH } from "@/sim/constants";
import { cellOf } from "@/sim/grid";
import { playerOf, propCentre, refreshPropFlags, type World } from "@/sim/runtime";
import { FACING_DX, FACING_DY, type Prop, type School, type Unit } from "@/sim/state";
import { runActions } from "@/sim/actions";
import { startDialogue } from "@/sim/dialogue";
import { bagAdd, bagCount, bagRemove } from "@/sim/inventory";
import { nearestDrop, pickUp } from "@/sim/loot";
import { requestTravel } from "@/sim/zones";
import { distance, isEnemy, moveUnit, spendEnergy } from "@/sim/units";

/** 2020: hold USE for 30 frames with 20 energy in the tank; the push costs those 20. */
export const PUSH_HOLD_TICKS = 30;
export const PUSH_ENERGY = 20;
const TALK_REACH = 20;

export type Focus =
  | { kind: "drop"; id: number; prompt: string }
  | { kind: "unit"; id: number; prompt: string }
  | { kind: "prop"; id: number; prompt: string }
  | null;

function propDistance(w: World, u: Unit, p: Prop): number {
  const def = w.catalog.props[p.def];
  // Distance from the unit to the prop's rectangle, not its centre: big props are easy to reach.
  const x0 = p.cx * CELL;
  const y0 = p.cy * CELL;
  const x1 = x0 + def.w * CELL;
  const y1 = y0 + def.h * CELL;
  const dx = u.x < x0 ? x0 - u.x : u.x > x1 ? u.x - x1 : 0;
  const dy = u.y < y0 ? y0 - u.y : u.y > y1 ? u.y - y1 : 0;
  return Math.sqrt(dx * dx + dy * dy);
}

function interactable(w: World, p: Prop): boolean {
  if (p.hidden) return false;
  const def = w.catalog.props[p.def];
  if (p.locked || p.to) return true;
  if (p.loot && !p.used) return true;
  if (p.talk) return true;
  if (def.carry || def.bench) return true;
  if (p.use && !(def.once && p.used) && !def.answers) return true;
  return false;
}

function promptFor(w: World, p: Prop): string {
  const def = w.catalog.props[p.def];
  if (p.locked) return "Unlock";
  if (p.to) return def.prompt ?? "Enter";
  if (p.loot && !p.used) return "Open";
  if (def.carry) return "Pick up";
  if (def.bench) return "Craft";
  if (p.talk) return def.prompt ?? "Read";
  return def.prompt ?? "Use";
}

/** What USE would act on right now. Pure; the HUD calls it every frame for the prompt and highlight. */
export function focusOf(w: World, u: Unit): Focus {
  if (u.carrying) return { kind: "prop", id: u.carrying, prompt: "Put down" };
  const drop = nearestDrop(w, u);
  if (drop) return { kind: "drop", id: drop.id, prompt: `Take ${w.catalog.items[drop.item].name}` };
  const fx = FACING_DX[u.facing];
  const fy = FACING_DY[u.facing];
  let best: Focus = null;
  let bestScore = Infinity;
  for (const other of w.zone.units) {
    if (other === u || !other.alive || !other.awake || other.hidden || isEnemy(u, other)) continue;
    if (!w.catalog.units[other.def].talk) continue;
    const d = distance(u.x, u.y, other.x, other.y);
    if (d > TALK_REACH) continue;
    const behind = (other.x - u.x) * fx + (other.y - u.y) * fy < 0 ? 8 : 0;
    if (d + behind < bestScore) {
      bestScore = d + behind;
      best = { kind: "unit", id: other.id, prompt: "Talk" };
    }
  }
  for (const p of w.zone.props) {
    if (!p.awake || !interactable(w, p)) continue;
    const d = propDistance(w, u, p);
    if (d > USE_REACH) continue;
    const c = propCentre(w.catalog, p);
    const behind = (c.x - u.x) * fx + (c.y - u.y) * fy < 0 ? 8 : 0;
    if (d + behind < bestScore) {
      bestScore = d + behind;
      best = { kind: "prop", id: p.id, prompt: promptFor(w, p) };
    }
  }
  return best;
}

export function use(w: World, u: Unit): void {
  if (!u.alive) return;
  if (u.carrying) {
    putDown(w, u);
    return;
  }
  const f = focusOf(w, u);
  if (!f) return;
  if (f.kind === "drop") {
    const d = w.zone.drops.find((x) => x.id === f.id);
    if (d) pickUp(w, u, d);
    return;
  }
  if (f.kind === "unit") {
    const other = w.rt.units.get(f.id);
    const tree = other ? w.catalog.units[other.def].talk : undefined;
    if (other && tree) startDialogue(w, tree, other.id);
    return;
  }
  const p = w.rt.props.get(f.id);
  if (p) useProp(w, u, p);
}

function useProp(w: World, u: Unit, p: Prop): void {
  const def = w.catalog.props[p.def];
  if (p.locked) {
    const key = p.keyTag ? findKey(w, u, p.keyTag) : null;
    if (!key) {
      w.emit({ e: "toast", text: p.label ? `${p.label} is locked` : "Locked" });
      w.emit({ e: "sfx", name: "locked", x: u.x, y: u.y });
      return;
    }
    // One use path for every key. 2020 consumed the key; so do we, except bound ones.
    if (!w.catalog.items[key].bound) bagRemove(w, u, key, 1);
    p.locked = false;
    if (def.gate) {
      p.solid = false;
      w.rt.propFlagsDirty = true;
    }
    w.emit({ e: "prop", prop: p.id, change: "unlock" });
    w.emit({ e: "toast", text: `Unlocked with ${w.catalog.items[key].name}` });
    return;
  }
  if (p.to) {
    requestTravel(w, p.to.zone, p.to.mark);
    return;
  }
  if (p.loot && !p.used) {
    const left: typeof p.loot = [];
    for (const s of p.loot) {
      const rest = bagAdd(w, u, s.item, s.qty);
      if (rest < s.qty) w.emit({ e: "loot", item: s.item, qty: s.qty - rest });
      if (rest > 0) left.push({ item: s.item, qty: rest });
    }
    if (left.length > 0) {
      p.loot = left; // chest keeps what did not fit
      w.emit({ e: "toast", text: "Inventory full" });
    } else {
      p.loot = null;
      p.used = true;
      if (def.hideWhenUsed) {
        p.hidden = true;
        w.rt.propFlagsDirty = true;
      }
      if (p.use) runActions(w, p.use, u.id);
    }
    w.emit({ e: "prop", prop: p.id, change: "open" });
    return;
  }
  if (def.carry) {
    if (u.energyLocked || u.energy < PUSH_ENERGY) {
      w.emit({ e: "toast", text: "Too tired" });
      return;
    }
    u.carrying = p.id;
    p.solid = false;
    w.rt.propFlagsDirty = true;
    w.emit({ e: "prop", prop: p.id, change: "use" });
    return;
  }
  if (p.talk) {
    startDialogue(w, p.talk, -p.id);
    return;
  }
  if (p.use && !(def.once && p.used)) {
    p.used = true;
    p.on = !p.on;
    w.emit({ e: "prop", prop: p.id, change: "switch" });
    runActions(w, p.use, u.id);
    return;
  }
  if (def.bench) w.emit({ e: "prop", prop: p.id, change: "use" });
}

function findKey(w: World, u: Unit, tag: string): string | null {
  if (!u.bag) return null;
  for (const s of u.bag) if (s && w.catalog.items[s.item].opens === tag) return s.item;
  return null;
}

/** A bed or a fire within reach? The game can only be saved there. */
export function nearRest(w: World): boolean {
  const u = playerOf(w);
  for (const p of w.zone.props) {
    if (!p.hidden && w.catalog.props[p.def].rest && propDistance(w, u, p) <= USE_REACH * 2) return true;
  }
  return false;
}

/** Any bench within reach? The bag window shows the craft grid only then. */
export function nearBench(w: World): boolean {
  const u = playerOf(w);
  for (const p of w.zone.props) {
    if (w.catalog.props[p.def].bench && propDistance(w, u, p) <= USE_REACH * 2) return true;
  }
  return false;
}

// --- carry -----------------------------------------------------------------

function footprintFree(w: World, p: Prop, cx: number, cy: number, self: number): boolean {
  const def = w.catalog.props[p.def];
  const wasSolid = p.solid;
  p.solid = false;
  refreshPropFlags(w.catalog, w.rt, w.zone);
  let ok = true;
  for (let y = cy; y < cy + def.h && ok; y++) {
    for (let x = cx; x < cx + def.w && ok; x++) ok = w.rt.grid.free(x, y, self);
  }
  p.solid = wasSolid;
  refreshPropFlags(w.catalog, w.rt, w.zone);
  return ok;
}

function putDown(w: World, u: Unit): void {
  const p = w.rt.props.get(u.carrying);
  if (!p) {
    u.carrying = 0;
    return;
  }
  const def = w.catalog.props[p.def];
  const fx = FACING_DX[u.facing];
  const fy = FACING_DY[u.facing];
  // The cell block directly in front of the feet, never the player's own cell.
  const ucx = cellOf(u.x);
  const ucy = cellOf(u.y);
  const cx = fx > 0 ? ucx + 1 : fx < 0 ? ucx - def.w : ucx - Math.floor(def.w / 2);
  const cy = fy > 0 ? ucy + 1 : fy < 0 ? ucy - def.h : ucy - Math.floor(def.h / 2);
  if (!footprintFree(w, p, cx, cy, 0)) {
    w.emit({ e: "toast", text: "No room to put it down" });
    return;
  }
  p.cx = cx;
  p.cy = cy;
  p.solid = def.solid;
  u.carrying = 0;
  w.rt.propFlagsDirty = true;
  w.emit({ e: "prop", prop: p.id, change: "push" });
}

// --- push / pull -----------------------------------------------------------

/**
 * Called every tick while USE is held. `mx,my` is the movement intent, so
 * "holding USE and walking away" reads as a pull. Returns true while the
 * player is braced against something (movement is suppressed by the caller).
 */
export function holdUse(w: World, u: Unit, mx: number, my: number): boolean {
  if (u.carrying || !u.alive) {
    u.hold = 0;
    return false;
  }
  const fx = FACING_DX[u.facing];
  const fy = FACING_DY[u.facing];
  const p = pushableAhead(w, u);
  if (!p) {
    u.hold = 0;
    return false;
  }
  const along = mx * fx + my * fy;
  if (along === 0) {
    u.hold = 0;
    return true;
  }
  if (u.energyLocked || u.energy < PUSH_ENERGY) {
    if (u.hold === 0) w.emit({ e: "toast", text: "Too tired" });
    u.hold = 1;
    return true;
  }
  if (++u.hold < PUSH_HOLD_TICKS) return true;
  u.hold = 0;
  const dir = along > 0 ? 1 : -1;
  if (dir < 0) {
    // Pull: the player steps back one cell first, then the prop follows into the gap.
    const ox = u.x;
    const oy = u.y;
    moveUnit(w, u, -fx * CELL, -fy * CELL);
    if (distance(ox, oy, u.x, u.y) < CELL - 0.5) {
      moveUnit(w, u, ox - u.x, oy - u.y);
      return true;
    }
  }
  const nx = p.cx + fx * dir;
  const ny = p.cy + fy * dir;
  if (!footprintFree(w, p, nx, ny, 0)) {
    if (dir < 0) moveUnit(w, u, fx * CELL, fy * CELL);
    return true;
  }
  p.cx = nx;
  p.cy = ny;
  w.rt.propFlagsDirty = true;
  spendEnergy(u, PUSH_ENERGY);
  if (dir > 0) moveUnit(w, u, fx * CELL, fy * CELL);
  w.emit({ e: "prop", prop: p.id, change: "push" });
  w.emit({ e: "sfx", name: "push", x: u.x, y: u.y });
  return true;
}

function pushableAhead(w: World, u: Unit): Prop | null {
  const fx = FACING_DX[u.facing];
  const fy = FACING_DY[u.facing];
  const px = u.x + fx * (CELL - 1);
  const py = u.y + fy * (CELL - 1);
  const cx = cellOf(px);
  const cy = cellOf(py);
  for (const p of w.zone.props) {
    if (p.hidden || !p.awake || !p.solid) continue;
    const def = w.catalog.props[p.def];
    if (!def.push) continue;
    if (cx >= p.cx && cx < p.cx + def.w && cy >= p.cy && cy < p.cy + def.h) return p;
  }
  return null;
}

// --- world verbs -------------------------------------------------------------

/** Repair / Grow: nearest answering prop within 2 m. Invalid casts cost nothing, as in 2020. */
export function worldVerb(w: World, caster: Unit, verb: "repair" | "grow"): boolean {
  let best: Prop | null = null;
  let bestD = 2 * PX_PER_METRE;
  for (const p of w.zone.props) {
    if (p.hidden || p.used || w.catalog.props[p.def].answers !== verb) continue;
    const d = propDistance(w, caster, p);
    if (d <= bestD) {
      bestD = d;
      best = p;
    }
  }
  if (!best) {
    w.emit({ e: "toast", text: verb === "repair" ? "Nothing here to repair" : "Nothing here will grow" });
    return false;
  }
  for (const need of best.needs ?? []) {
    if (bagCount(caster, need.item) < need.qty) {
      w.emit({ e: "toast", text: `Needs ${need.qty}x ${w.catalog.items[need.item].name}` });
      return false;
    }
  }
  for (const need of best.needs ?? []) bagRemove(w, caster, need.item, need.qty);
  best.used = true;
  best.on = true;
  if (best.use) runActions(w, best.use, caster.id);
  w.emit({ e: "prop", prop: best.id, change: "use" });
  return true;
}

/** A bolt of `school` ended at (x,y): props that answer to that school switch on. */
export function schoolTouch(w: World, school: School, x: number, y: number, from: number): void {
  for (const p of w.zone.props) {
    if (p.hidden || p.on) continue;
    const def = w.catalog.props[p.def];
    if (def.answers !== school) continue;
    const c = propCentre(w.catalog, p);
    if (distance(x, y, c.x, c.y) > 14) continue;
    p.on = true;
    p.used = true;
    if (p.use) runActions(w, p.use, from);
    w.emit({ e: "prop", prop: p.id, change: "switch" });
  }
}
