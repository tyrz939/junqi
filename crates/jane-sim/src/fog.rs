//! Fog (`sim/zones.ts stampFog`): seen-bits in `ZoneState.fog`, one per 2-cell block indoors and
//! per 8-cell block out, shared by the party. Interiors black out what is unseen; outdoors the
//! ground is always drawn and the bits only decide what the map shows.

use jane_core::Vec2;
use jane_core::num::CELL_FX;

use crate::runtime::FogGeom;
use crate::tuning::{FOG_RADIUS_IN, FOG_RADIUS_OUT};

/// Mark the disc round `pos` as seen.
pub fn stamp_fog(fog: &mut [u32], geom: FogGeom, indoor: bool, pos: Vec2) {
    let r = if indoor { FOG_RADIUS_IN } else { FOG_RADIUS_OUT };
    // The coarse outdoor disc is rounded out a little.
    let lim = if indoor { r * r } else { r * r + r };
    let size = CELL_FX * geom.cells as i32;
    let (bx, by) = (pos.x.0.div_euclid(size), pos.y.0.div_euclid(size));
    for y in (by - r).max(0)..=(by + r).min(geom.h as i32 - 1) {
        for x in (bx - r).max(0)..=(bx + r).min(geom.w as i32 - 1) {
            let (dx, dy) = (x - bx, y - by);
            if dx * dx + dy * dy > lim {
                continue;
            }
            let bit = y as u32 * geom.w + x as u32;
            if let Some(w) = fog.get_mut((bit >> 5) as usize) {
                *w |= 1 << (bit & 31);
            }
        }
    }
}

/// Has the fog block `(bx, by)` been seen?
pub fn fog_seen(fog: &[u32], geom: FogGeom, bx: i32, by: i32) -> bool {
    if bx < 0 || by < 0 || bx as u32 >= geom.w || by as u32 >= geom.h {
        return false;
    }
    let bit = by as u32 * geom.w + bx as u32;
    fog.get((bit >> 5) as usize).is_some_and(|w| w & (1 << (bit & 31)) != 0)
}
