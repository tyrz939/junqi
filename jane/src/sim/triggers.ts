// Triggers: "when someone is in this named rect (and these conditions hold),
// run this action list". 2020 had a dozen obj_trigger_* objects; they were all
// this one thing. Room lock-ins, rat spawns, snake aggro, quest locations and
// torch-lighting corridors are rows in triggers.json.
//
//   mode "enter"  fires on the tick the FIRST of the party walks in
//   mode "while"  fires as soon as anyone is inside AND `when` holds
//                 (this is how "kill everything to unlock the chest" is written)
//
// With up to four players a rect is "occupied" while any living one of them is in
// it, and the actions run on behalf of whoever tripped it.
//
// Pressure plates live here too: a plate prop is pressed while any unit or
// pushable prop covers it (2020: "any obj_in_world_parent presses a button, so
// barrels work"), runs `use` on press and `release` when it clears.

import { cellOf } from "@/sim/grid";
import { asPlayer, playersHere, propsInCells, type World } from "@/sim/runtime";
import type { PlayerState, Prop } from "@/sim/state";
import { conditionsMet, runActions } from "@/sim/actions";
import type { Rect } from "@/world/blueprint";

const PLATE_PERIOD = 6;

/** First living player standing in the rect, in seat order, or null. */
export function playerInRect(w: World, rect: Rect, except: PlayerState | null = null): PlayerState | null {
  for (const p of playersHere(w)) {
    if (p === except) continue;
    const body = w.rt.units.get(p.unitId);
    if (!body || !body.alive) continue;
    const cx = cellOf(body.x);
    const cy = cellOf(body.y);
    if (cx >= rect.cx && cy >= rect.cy && cx < rect.cx + rect.w && cy < rect.cy + rect.h) return p;
  }
  return null;
}

export function stepTriggers(w: World): void {
  for (const t of w.zone.triggers) {
    const def = w.catalog.triggers[t.id];
    const rect = w.rt.bp.rects[def.rect];
    if (!rect) continue;
    const who = playerInRect(w, rect);
    const inside = who !== null;
    const entered = inside && !t.inside;
    t.inside = inside;
    if (!who || (t.fired && def.once)) continue;
    const mode = def.mode ?? "enter";
    if (mode === "enter" && !entered) continue;
    asPlayer(w, who, () => {
      if (!conditionsMet(w, def.when)) return;
      t.fired = true;
      runActions(w, def.actions, who.unitId);
    });
  }
  if (w.state.tick % PLATE_PERIOD === 0) stepPlates(w);
}

function stepPlates(w: World): void {
  // The runtime keeps the plates apart (a handful per zone, in id order), so a zone
  // without any pays nothing. Plates far from everyone still answer: a barrel left on
  // one holds its gate open from the other side of the map.
  for (const p of w.rt.plates) {
    const def = w.catalog.props[p.def];
    if (p.hidden) continue;
    const pressed = plateCovered(w, p, def.w, def.h);
    if (pressed === p.on) continue;
    p.on = pressed;
    w.emit({ e: "prop", prop: p.id, change: "switch" });
    w.emit({ e: "sfx", name: pressed ? "plateDown" : "plateUp", x: p.cx * 8, y: p.cy * 8 });
    const list = pressed ? p.use : p.release;
    // A plate has no actor: a barrel can press it. Its lists are world verbs (lock, unlock).
    if (list) runActions(w, list, 0);
  }
}

function plateCovered(w: World, plate: Prop, pw: number, ph: number): boolean {
  for (const u of w.zone.units) {
    if (!u.alive || u.hidden) continue;
    const cx = cellOf(u.x);
    const cy = cellOf(u.y);
    if (cx >= plate.cx && cy >= plate.cy && cx < plate.cx + pw && cy < plate.cy + ph) return true;
  }
  for (const other of propsInCells(w.rt, plate.cx, plate.cy, plate.cx + pw - 1, plate.cy + ph - 1)) {
    if (other === plate || other.hidden) continue;
    const def = w.catalog.props[other.def];
    if (!def.push && !def.carry) continue;
    const overlapX = other.cx < plate.cx + pw && other.cx + def.w > plate.cx;
    const overlapY = other.cy < plate.cy + ph && other.cy + def.h > plate.cy;
    if (!overlapX || !overlapY) continue;
    // In someone's arms it still has its old cell, and presses nothing.
    if (w.zone.units.some((u) => u.carrying === other.id)) continue;
    return true;
  }
  return false;
}
