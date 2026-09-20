// The load ring (objects/distance_unload, 2020). The whole zone exists in state;
// only a 768 px square around each player thinks. Re-evaluated when any player
// crosses a 16 px block, never per unit per tick.
//
//   sleeps      idle AI and props outside every ring: no think, no occupancy, no draw
//   stays awake anyone in combat, anything carried, every player
//   still ticks cooldown and respawn clocks of sleepers (the Phaser build froze
//               them, so nothing respawned unless you stood on the corpse)
//
// With company the awake set is the UNION of the rings: two people at opposite
// ends of the county keep two islands of it alive, and nothing in between.
// Saves write every unit, awake or not. The ring is never what gets persisted.

import { CELL, RING_BLOCK, RING_RADIUS } from "@/sim/constants";
import { occupy, playersHere, vacate, type World } from "@/sim/runtime";

export function stepRing(w: World, force = false): void {
  const centres: number[] = [];
  let key = "";
  for (const p of playersHere(w)) {
    const body = w.rt.units.get(p.unitId);
    if (!body) continue;
    centres.push(body.x, body.y);
    key += `${Math.floor(body.x / RING_BLOCK)},${Math.floor(body.y / RING_BLOCK)};`;
  }
  if (!force && key === w.rt.ringKey) return;
  w.rt.ringKey = key;

  const near = (x: number, y: number, slack: number): boolean => {
    for (let i = 0; i < centres.length; i += 2) {
      if (Math.abs(x - centres[i]) <= RING_RADIUS + slack && Math.abs(y - centres[i + 1]) <= RING_RADIUS + slack) return true;
    }
    return false;
  };

  for (const u of w.zone.units) {
    if (w.party.ofUnit(u.id)) continue;
    const awake = near(u.x, u.y, 0) || u.combat === "combat";
    if (awake === u.awake) continue;
    u.awake = awake;
    if (awake) {
      if (u.alive && !u.hidden) occupy(w.rt, u);
    } else {
      vacate(w.rt, u);
      u.path = null;
    }
  }
  for (const p of w.zone.props) p.awake = near(p.cx * CELL, p.cy * CELL, 64);
}
