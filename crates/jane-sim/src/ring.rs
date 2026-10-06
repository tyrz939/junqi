//! The load ring (`sim/ring.ts`, 2020's `objects/distance_unload`). The whole zone exists in
//! state; only a square round each seat thinks. Re-run when any seat in the zone crosses a
//! 16 px block, never per unit per tick.
//!
//! - **asleep:** idle units and props outside every ring: no think, no occupancy, no draw.
//! - **awake:** anything in the union of the rings, anything in combat, anything under
//!   orders, and every connected seat's body.
//!
//! Distances are measured from the middle of the block a seat stands in, not from her feet: the
//! ring only re-runs when she crosses a block, and a load re-runs it wherever she stood, so a
//! unit at the edge must answer the same both times. `Unit.awake` and the ring's key
//! (`ZoneState.ring_key`) are saved; a prop's awake bit is derived from the key.

use jane_core::num::CELL_FX;
use jane_core::{Fx, Vec2, ZoneId};

use crate::ids::{PropIx, UnitId};
use crate::runtime::ZoneRuntime;
use crate::state::{CombatState, GameState, ZoneState};
use crate::tuning::{MAX_PLAYERS, PROP_SLACK_FX, RING_BLOCK_FX, RING_RADIUS_FX};

type Blocks = [Option<(i32, i32)>; MAX_PLAYERS];

/// Where the connected seats stand, for one zone's ring: the blocks of those in it, and every
/// connected body anywhere (a body never sleeps).
#[derive(Clone, Copy, Debug, Default)]
pub struct Watchers {
    pub blocks: Blocks,
    pub bodies: [Option<UnitId>; MAX_PLAYERS],
}

impl Watchers {
    /// The seats of `state` as seen from zone `z`, whose state is passed (it may be out).
    pub fn of(state: &GameState, z: ZoneId, zone: &ZoneState) -> Self {
        let mut w = Self::default();
        for p in state.players.iter().filter(|p| p.connected) {
            w.bodies[p.seat.index()] = Some(p.unit);
            if p.zone == z {
                if let Some(u) = zone.unit(p.unit) {
                    w.blocks[p.seat.index()] =
                        Some((u.pos.x.0.div_euclid(RING_BLOCK_FX), u.pos.y.0.div_euclid(RING_BLOCK_FX)));
                }
            }
        }
        w
    }
}

fn centre(b: (i32, i32)) -> Vec2 {
    Vec2::new(Fx(b.0 * RING_BLOCK_FX + RING_BLOCK_FX / 2), Fx(b.1 * RING_BLOCK_FX + RING_BLOCK_FX / 2))
}

fn near(blocks: &Blocks, x: i32, y: i32, slack: i32) -> bool {
    let r = RING_RADIUS_FX + slack;
    blocks.iter().flatten().any(|&b| {
        let c = centre(b);
        (x - c.x.0).abs() <= r && (y - c.y.0).abs() <= r
    })
}

/// Re-run the ring if a seat crossed a block since the last run (or `force`). `scratch` is a
/// reused buffer.
pub fn step_ring(zone: &mut ZoneState, rt: &mut ZoneRuntime, w: &Watchers, force: bool, scratch: &mut Vec<PropIx>) {
    if !force && zone.ring_key == Some(w.blocks) {
        return;
    }
    zone.ring_key = Some(w.blocks);
    for u in &mut zone.units {
        if w.bodies.contains(&Some(u.id)) {
            continue;
        }
        // Something sent somewhere walks there however far from her it is.
        let awake = near(&w.blocks, u.pos.x.0, u.pos.y.0, 0)
            || matches!(u.combat, CombatState::Combat | CombatState::Evade)
            || u.order.is_some();
        if awake == u.awake {
            continue;
        }
        rt.set_awake(u, awake);
        if !awake {
            u.path = None;
        }
    }
    wake_props(zone, rt, scratch);
}

/// The props' awake bits for the ring as it last ran (`zone.ring_key`): put the old set to
/// sleep, then wake what the buckets hold round each seat. The same bits a pass over the whole
/// zone would set, without the pass. A rebuilt runtime calls this to get them back.
pub fn wake_props(zone: &ZoneState, rt: &mut ZoneRuntime, scratch: &mut Vec<PropIx>) {
    for &ix in &rt.awake_prop_list {
        rt.awake_props[ix as usize] = false;
    }
    rt.awake_prop_list.clear();
    let Some(blocks) = zone.ring_key else { return };
    let reach = RING_RADIUS_FX + PROP_SLACK_FX;
    let cell = |v: i32| v.div_euclid(CELL_FX);
    for c in blocks.iter().flatten().map(|&b| centre(b)) {
        rt.props.query(cell(c.x.0 - reach), cell(c.y.0 - reach), cell(c.x.0 + reach), cell(c.y.0 + reach), scratch);
        for &ix in scratch.iter() {
            let p = &zone.props[ix as usize];
            if rt.awake_props[ix as usize]
                || !near(&blocks, i32::from(p.cell.x) * CELL_FX, i32::from(p.cell.y) * CELL_FX, PROP_SLACK_FX)
            {
                continue;
            }
            rt.awake_props[ix as usize] = true;
            rt.awake_prop_list.push(ix);
        }
    }
    // Two seats' sets merge out of order; keep the list in id order.
    rt.awake_prop_list.sort();
}
