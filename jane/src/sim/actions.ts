// The one verb runner. Dialogue options, prop use, triggers, quest rewards,
// item use, unit death and the terminal all speak this list. The switch is
// exhaustive: adding a verb to the Action union without handling it here is a
// compile error, not a silently ignored string.

import { cellOf, centre } from "@/sim/grid";
import { lineOfSight } from "@/sim/los";
import { addUnit, playerOf, playersHere, removeUnit, touchProp, type World } from "@/sim/runtime";
import { FACING_DX, FACING_DY, type Action, type ActionList, type Condition, type PlayerState, type Prop } from "@/sim/state";
import { bagAdd, bagCount, bagRemove } from "@/sim/inventory";
import { giveQuest, handIn, onLocation, questActive, questDone, questReady } from "@/sim/quests";
import { applyEffect } from "@/sim/status";
import { createUnit, maxHp, maxMp } from "@/sim/units";
import { ENERGY_MAX, TICKS_PER_HOUR } from "@/sim/constants";
import { isNight } from "@/sim/text";
import { startDialogue } from "@/sim/dialogue";
import { spawnDrop } from "@/sim/loot";
import { requestTravel } from "@/sim/zones";

export function runActions(w: World, list: ActionList, actor: number): void {
  for (const a of list) runAction(w, a, actor);
}

function findProp(w: World, key: string): Prop | undefined {
  return w.rt.propsByKey.get(key);
}

/**
 * `subject` is the unit the verb lands on (who is healed, who throws). `w.actor` is the
 * player it is done on behalf of, and may be null: the bell at nine has no actor, nor
 * does a unit dying of poison. Player-scoped verbs (give, learn, rest...) do nothing then.
 */
export function runAction(w: World, a: Action, subject: number): void {
  const me = w.actor;
  const player = me && w.rt.units.has(me.unitId) ? playerOf(w) : null;
  const actor = subject;
  switch (a.do) {
    case "quest":
      giveQuest(w, a.quest);
      return;
    case "handin":
      handIn(w, a.quest);
      return;
    case "flag":
      w.state.flags[a.flag] = a.add !== undefined ? (w.state.flags[a.flag] ?? 0) + a.add : (a.value ?? 1);
      return;
    case "rest": {
      // A bed or a fire: mend, remember the spot, let the app write the save.
      // `until` sleeps the clock forward to that hour (a bed); without it no time passes (a fire).
      if (!me || !player) return;
      player.hp = maxHp(player);
      player.mp = maxMp(player);
      player.energy = ENERGY_MAX;
      player.energyLocked = false;
      // The spot is the party's: whoever dies next, or sits down next, wakes here.
      w.state.rest = { zone: w.zone.id, x: player.x, y: player.y };
      if (a.until !== undefined) {
        // The clock is everyone's. The night only passes when the whole party is resting.
        if (w.party.everyoneResting()) {
          const target = Math.floor(a.until * TICKS_PER_HOUR);
          if (w.state.clock >= target) w.state.day++;
          w.state.clock = target;
        } else {
          w.emit({ e: "toast", text: "The night will not pass until everyone is resting" });
        }
      }
      // To everyone: the world lives on the host's machine, and a guest's rest saves it too.
      w.emit({ e: "rest" }, true);
      return;
    }
    case "give": {
      if (!player) return;
      const qty = a.qty ?? 1;
      const left = bagAdd(w, player, a.item, qty);
      if (left < qty) w.emit({ e: "loot", item: a.item, qty: qty - left });
      // Never lose a reward: what does not fit lands at the player's feet.
      if (left > 0) {
        spawnDrop(w, a.item, left, player.x, player.y);
        w.emit({ e: "toast", text: "Inventory full" });
      }
      return;
    }
    case "take":
      if (player) bagRemove(w, player, a.item, a.qty ?? 1);
      return;
    case "learn":
      if (me) teach(w, a.spell);
      return;
    case "grow":
      grow(w, a.stat, a.amount, a.id);
      return;
    case "toast":
      w.emit({ e: "toast", text: a.text });
      return;
    case "lock":
    case "unlock": {
      const p = findProp(w, a.prop);
      if (!p) return;
      p.locked = a.do === "lock";
      if (w.catalog.props[p.def].gate) {
        p.solid = p.locked;
        touchProp(w, p);
      }
      w.emit({ e: "prop", prop: p.id, change: a.do });
      return;
    }
    case "show":
    case "hide": {
      const p = findProp(w, a.prop);
      if (!p) return;
      p.hidden = a.do === "hide";
      touchProp(w, p);
      w.emit({ e: "prop", prop: p.id, change: a.do });
      return;
    }
    case "switch": {
      const p = findProp(w, a.prop);
      if (!p) return;
      p.on = a.on ?? !p.on;
      w.emit({ e: "prop", prop: p.id, change: "switch" });
      return;
    }
    case "spawn": {
      if (w.rt.unitsByKey.has(a.unit)) return;
      const mark = w.rt.bp.marks[a.at];
      if (!mark) return;
      const free = w.rt.grid.nearestFree(mark.cx, mark.cy, 6) ?? mark;
      addUnit(w, createUnit(w.state, w.catalog, a.def, a.unit, centre(free.cx), centre(free.cy), mark.facing ?? 1));
      return;
    }
    case "despawn": {
      const u = w.rt.unitsByKey.get(a.unit);
      if (u && !w.party.ofUnit(u.id)) removeUnit(w, u);
      return;
    }
    case "aggro": {
      const u = w.rt.unitsByKey.get(a.unit);
      const victim = player ?? playersHere(w).map((p) => w.rt.units.get(p.unitId)).find((x) => x?.alive);
      if (u && u.alive && victim) {
        u.target = victim.id;
        u.combat = "combat";
        u.awake = true;
      }
      return;
    }
    case "location":
      onLocation(w, a.name);
      return;
    case "fill": {
      const r = w.rt.bp.rects[a.rect];
      if (!r) return;
      for (let y = r.cy; y < r.cy + r.h; y++) {
        for (let x = r.cx; x < r.cx + r.w; x++) {
          if (!w.rt.grid.inside(x, y) || w.rt.grid.tileAt(x, y) === a.tile) continue;
          w.rt.grid.setTile(x, y, a.tile);
          w.zone.tileDeltas.push(w.rt.grid.index(x, y), a.tile);
        }
      }
      w.emit({ e: "tiles", cx: r.cx, cy: r.cy, w: r.w, h: r.h });
      return;
    }
    case "strike": {
      const r = w.rt.bp.rects[a.rect];
      if (!r) return;
      // Whoever pulled the lever struck the blow: the kill is hers, and so is the party's penalty.
      // A trap nobody set (a trigger, the clock) strikes as the world, from nobody.
      const puller = w.rt.units.get(actor);
      const from = puller && puller.alive ? puller.id : 0;
      for (const u of w.zone.units) {
        if (!u.alive || u.hidden) continue;
        if (u.faction === "friendly" && !a.hitsFriends) continue;
        const cx = cellOf(u.x);
        const cy = cellOf(u.y);
        if (cx < r.cx || cy < r.cy || cx >= r.cx + r.w || cy >= r.cy + r.h) continue;
        u.incoming.push({ amount: a.amount, school: a.school, from, crit: false, status: a.effect });
      }
      w.emit({ e: "sfx", name: "strike", x: centre(r.cx + (r.w >> 1)), y: centre(r.cy + (r.h >> 1)) });
      w.emit({ e: "shake", amount: 3 });
      return;
    }
    case "status": {
      const u = w.rt.units.get(actor) ?? player;
      if (u) applyEffect(w, u, a.effect, actor);
      return;
    }
    case "heal": {
      const u = w.rt.units.get(actor) ?? player;
      if (!u) return;
      // Below 1 is a fraction of max HP: the 2020 apple healed 25% while its tooltip said 25.
      const amount = a.amount <= 1 ? Math.round(maxHp(u) * a.amount) : a.amount;
      u.incoming.push({ amount, school: "heal", from: u.id, crit: false });
      return;
    }
    case "travel":
      if (me) requestTravel(w, a.zone, a.mark);
      return;
    case "talk":
      if (me) startDialogue(w, a.tree, actor);
      return;
    case "throw": {
      // Bait lands three cells ahead, or at the feet if a wall is in the way.
      const u = w.rt.units.get(actor) ?? player;
      if (!u) return;
      const tx = u.x + FACING_DX[u.facing] * 24;
      const ty = u.y + FACING_DY[u.facing] * 24;
      const open = lineOfSight(w.rt.grid, u.x, u.y, tx, ty) && !w.rt.grid.solid(cellOf(tx), cellOf(ty));
      spawnDrop(w, a.item, 1, open ? tx : u.x, open ? ty : u.y);
      return;
    }
    case "shake":
      w.emit({ e: "shake", amount: a.amount });
      return;
    case "camera":
      w.emit({ e: "camera", mode: a.mode, rect: a.rect ?? "" });
      return;
    default: {
      const never: never = a;
      throw new Error(`Unhandled action ${JSON.stringify(never)}`);
    }
  }
}

/**
 * Growth is the world's, not hers: one of them touches the orb and all of them know
 * Icebolt, including whoever is away from the table and whoever sits down next month.
 */
export function teach(w: World, spell: string): boolean {
  const known = w.state.growth.spells;
  if (!w.catalog.spells[spell] || known.includes(spell)) return false;
  known.push(spell);
  for (const body of w.party.bodies()) if (!body.book.includes(spell)) body.book.push(spell);
  for (const p of w.state.players) bindLearned(p, spell);
  w.emit({ e: "learn", spell }, true);
  w.emit({ e: "toast", text: `Learned ${w.catalog.spells[spell].name}` }, true);
  return true;
}

/**
 * Growth by finding. There is no XP: she is stronger because she found one of Julie's jars,
 * and so is everyone at the table, the away and the late included (as with `teach`). The id
 * is the jar itself. Quest rewards run once per player, so without it a party of four would
 * eat one jar four times; with it, the second, third and fourth asks are no-ops, and every
 * upgrade in the county can be counted.
 */
export function grow(w: World, stat: "strength" | "spirit", amount: number, id: string): boolean {
  const g = w.state.growth;
  if (g.found.includes(id)) return false;
  g.found.push(id);
  g[stat] += amount;
  for (const body of w.party.bodies()) {
    body[stat] += amount;
    // The new health is hers at once: finding a jar at 10 HP should feel like finding a jar.
    if (body.alive) {
      if (stat === "strength") body.hp = Math.min(maxHp(body), body.hp + amount * 5);
      else body.mp = Math.min(maxMp(body), body.mp + amount * 5);
    }
  }
  w.emit({ e: "toast", text: stat === "strength" ? "You are a little stronger than you were" : "The words stay with you" }, true);
  return true;
}

/** A newly known spell takes the first empty bar slot, once. */
export function bindLearned(p: PlayerState, spell: string): void {
  if (p.bar.some((s) => s !== null && s.source === "spell" && s.id === spell)) return;
  const free = p.bar.findIndex((s) => s === null);
  if (free >= 0) p.bar[free] = { source: "spell", id: spell };
}

export function conditionsMet(w: World, list: Condition[] | undefined): boolean {
  if (!list) return true;
  for (const c of list) if (!conditionMet(w, c)) return false;
  return true;
}

function conditionMet(w: World, c: Condition): boolean {
  const r = conditionHolds(w, c);
  return c.not ? !r : r;
}

function conditionHolds(w: World, c: Condition): boolean {
  switch (c.if) {
    case "flag": {
      const v = w.state.flags[c.flag] ?? 0;
      if (c.eq !== undefined) return v === c.eq;
      if (c.min !== undefined) return v >= c.min;
      return v !== 0;
    }
    case "night":
      return isNight(w.state);
    case "questActive":
      return questActive(w, c.quest) !== undefined;
    case "questReady":
      return questReady(w, c.quest);
    case "questDone":
      return questDone(w, c.quest);
    // Hers, not the party's: what she is holding. False with no actor.
    case "hasItem":
      return w.actor !== null && bagCount(playerOf(w), c.item) >= (c.qty ?? 1);
    // The world's: what any of them has learned, all of them know.
    case "knows":
      return w.state.growth.spells.includes(c.spell) || (w.actor !== null && playerOf(w).book.includes(c.spell));
    case "dead": {
      const u = w.rt.unitsByKey.get(c.unit);
      return u ? !u.alive : (w.state.flags[`dead:${c.unit}`] ?? 0) !== 0;
    }
    default: {
      const never: never = c;
      throw new Error(`Unhandled condition ${JSON.stringify(never)}`);
    }
  }
}
