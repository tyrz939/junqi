//! C10: the tease. The verb's first lock and the boss gate are seen before they can be opened:
//! some cell she stood on in an earlier flood has the prop on screen with no wall between.
//!
//! Only a verb she finds INSIDE can be teased: the lock is meant to be seen, wondered at, and
//! opened later. A verb she already had at the door is opened in the same flood that reaches it,
//! so there is nothing to tease and nothing to prove (every lock in a return visit is of that kind).

use alloc::format;
use alloc::vec::Vec;
use jane_core::blueprint::PropSpawn;
use jane_core::tile::F_BLOCK_LOS;
use jane_core::{Tile, view};
use jane_data::{MissionEdgeKind, MissionNodeKind};

use super::{Check, Ctx, Fault};

/// Half a view each way, less a margin: what is surely on screen around her (`jane_core::view`).
const SEE_W: i32 = view::HALF_W_CELLS as i32 - 2;
const SEE_H: i32 = view::HALF_H_CELLS as i32 - 1;

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let m = c.m;
    let mut teases = Vec::new();
    let first_verb = c.info.locks.iter().find(|l| {
        matches!(l.kind, MissionEdgeKind::Verb { verb, .. } if !m.given_verbs.contains(&verb))
            && !m.edges[l.edge].shortcut
    });
    teases.extend(first_verb.map(|l| l.prop));
    let boss = c.info.rooms.iter().find(|r| m.nodes[r.node].kind == MissionNodeKind::Boss);
    if let Some(boss) = boss {
        let gate =
            c.info.locks.iter().find(|l| {
                usize::from(m.edges[l.edge].to) == boss.node && !matches!(l.kind, MissionEdgeKind::Verb { .. })
            });
        teases.extend(gate.map(|l| l.prop));
    }
    let mut out = Vec::new();
    for key in teases {
        let (Some(p), Some(opened_at)) = (c.prop(key), c.base.info.fired(key)) else { continue };
        if !seen_before(c, p, opened_at) {
            out.push(Fault::new(Check::C10, format!("{} is not seen before it can be opened", c.name(key))));
        }
    }
    out
}

/// Is there a cell she stood on in an earlier flood from which the prop is on screen with no
/// wall between?
pub fn seen_before(c: &Ctx<'_>, p: &PropSpawn, opened_at: u16) -> bool {
    let bp = c.bp;
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    let d = c.cat.story.prop(p.def);
    let px = i32::from(p.cell.x) + i32::from(d.w >> 1);
    let py = i32::from(p.cell.y) + i32::from(d.h >> 1);
    // An integer line walk, cell to cell. Walls only: a gate is what she is looking at.
    let clear = |x0: i32, y0: i32| {
        let (mut x, mut y) = (x0, y0);
        let (dx, dy) = ((px - x).abs(), (py - y).abs());
        let sx = if x < px { 1 } else { -1 };
        let sy = if y < py { 1 } else { -1 };
        let mut err = dx - dy;
        while x != px || y != py {
            if bp.tiles.read(x, y, Tile::Void).flags() & F_BLOCK_LOS != 0 {
                return false;
            }
            let e2 = err * 2;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
        true
    };
    for y in (py - SEE_H).max(0)..=(py + SEE_H).min(h - 1) {
        for x in (px - SEE_W).max(0)..=(px + SEE_W).min(w - 1) {
            // Strictly earlier: a cell first reached in the flood that opened it may have been
            // reached with the key already in hand, and that is an open door, not a tease.
            let Some(seen) = c.base.info.first_seen_at(x, y) else { continue };
            if seen >= opened_at {
                continue;
            }
            if clear(x, y) {
                return true;
            }
        }
    }
    false
}
