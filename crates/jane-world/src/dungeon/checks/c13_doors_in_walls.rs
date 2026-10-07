//! C13: every door stands in a wall, the right way round. A gate, a shutter, a door out of the
//! zone: whatever shuts a way stands in a gap of a wall, with wall on both its flanks, so it
//! reads as a doorway and not as a thing left on the floor (the owner's playtest, 2026-09-29:
//! "an inner mine door stands on open ground with no wall around it"). A hatch or a way in is
//! flat on the floor and is no door here.
//!
//! The footprint says how it is drawn (`jane-art` `kit::barrier`: edge-on when taller than wide),
//! so it says which wall it belongs in. Wider than tall, or square (the 2x2 door out, drawn face
//! on), it stands in a wall that runs west to east, a north or a south wall: wall west and east
//! of it on its foot row, and on every row when it is one cell deep. Taller than wide (a gate
//! across a west or east door, drawn edge on) it stands in a wall that runs north to south: wall
//! north and south of it on every column. A face-on door in a side wall looked like a south door
//! stood on an east wall.

use alloc::format;
use alloc::vec::Vec;
use jane_core::Blueprint;
use jane_core::grid::Rect;
use jane_core::tile::{F_SOLID, Tile};

use super::{Check, Ctx, Fault};

/// Wall: a tile nothing walks through, the zone's edge included.
fn wall(bp: &Blueprint, x: i32, y: i32) -> bool {
    bp.tiles.read(x, y, Tile::Void).flags() & F_SOLID != 0
}

/// Whether a door with this footprint stands in a wall gap the way it is drawn.
pub fn in_wall(bp: &Blueprint, r: Rect) -> bool {
    let across = |y: i32| wall(bp, r.x - 1, y) && wall(bp, r.right(), y);
    let along = |x: i32| wall(bp, x, r.y - 1) && wall(bp, x, r.bottom());
    if r.h > r.w {
        (r.x..r.right()).all(along)
    } else if r.h == 1 {
        across(r.y)
    } else {
        across(r.bottom() - 1)
    }
}

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let (bp, cat) = (c.bp, c.cat);
    let mut out = Vec::new();
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        if !d.solid || !(d.gate || p.to.is_some()) {
            continue;
        }
        let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
        if !in_wall(bp, r) {
            out.push(Fault::new(
                Check::C13,
                format!(
                    "{} ({}) at {},{} does not stand in a wall gap {}",
                    c.name(p.key),
                    d.id,
                    r.x,
                    r.y,
                    if r.h > r.w { "running north to south" } else { "running west to east" }
                ),
            ));
        }
    }
    out
}
