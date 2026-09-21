// The load ring (objects/distance_unload, 2020). The whole zone exists in state;
// only a 768 px square around each player thinks. Re-evaluated when any player
// crosses a 16 px block, never per unit per tick.
//
//   sleeps      idle AI and props outside every ring: no think, no occupancy, no draw
//   stays awake anyone in combat, anything under orders, anything carried, every player
//   still ticks cooldown and respawn clocks of sleepers (the Phaser build froze
//               them, so nothing respawned unless you stood on the corpse)
//
// With company the awake set is the UNION of the rings: two people at opposite
// ends of the county keep two islands of it alive, and nothing in between.
// Saves write every unit, awake or not. The ring is never what gets persisted.

import { CELL, RING_BLOCK, RING_RADIUS } from "@/sim/constants";
import { cellOf } from "@/sim/grid";
import { occupy, playersHere, propsInCells, vacate, type World } from "@/sim/runtime";

/** Props stay up 64 px beyond the ring, measured from their origin cell. */
const PROP_SLACK = 64;

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

  // A connected player's body never sleeps. Asked of the party once, not once per unit:
  // this pass is over every unit in the zone, thousands of them in the county.
  const bodies: number[] = [];
  for (const p of w.state.players) if (p.connected) bodies.push(p.unitId);

  for (const u of w.zone.units) {
    if (bodies.includes(u.id)) continue;
    // Something sent somewhere (`send`) walks there however far from her it is.
    const awake = near(u.x, u.y, 0) || u.combat === "combat" || u.order !== null;
    if (awake === u.awake) continue;
    u.awake = awake;
    if (awake) {
      if (u.alive && !u.hidden) occupy(w.rt, u);
    } else {
      vacate(w.rt, u);
      u.path = null;
    }
  }

  // Props: put the old set to sleep, then wake what the buckets hold around each
  // player. The same flags a pass over the whole zone would set, without the pass.
  const awakeProps = w.rt.awakeProps;
  for (const p of awakeProps) p.awake = false;
  awakeProps.length = 0;
  const reach = RING_RADIUS + PROP_SLACK;
  for (let i = 0; i < centres.length; i += 2) {
    const x = centres[i];
    const y = centres[i + 1];
    for (const p of propsInCells(w.rt, cellOf(x - reach), cellOf(y - reach), cellOf(x + reach), cellOf(y + reach))) {
      if (p.awake || !near(p.cx * CELL, p.cy * CELL, PROP_SLACK)) continue;
      p.awake = true;
      awakeProps.push(p);
    }
  }
}
