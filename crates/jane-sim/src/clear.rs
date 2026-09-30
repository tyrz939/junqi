//! Nothing solid ever lands on a unit (`sim/clear.ts`, DUNGEONS.md E12). Three verbs can make a
//! cell solid under someone's feet: `Show` (an exhibit back on its plinth), `Lock` (a gate
//! dropping) and a `Fill` with a solid tile (a hedge growing). A unit inside a solid cannot move
//! at all, so:
//!
//! - **show, lock:** whoever it landed on is moved to the nearest free cell she could have
//!   *walked* to: a search outward over open floor, never a ring through walls.
//! - **fill:** a solid fill waits until nobody touches its rect, then lands whole. The zone
//!   remembers what it is owed (`ZoneState.pending_fill`, saved) and looks again every tick.
//!
//! "On a unit" means its body box touches the cell, not only its feet.

use jane_core::num::CELL_FX;
use jane_core::tile::F_SOLID;
use jane_core::{Rect, Tile, Vec2};
use jane_data::PropDef;

use crate::ctx::Ctx;
use crate::event::EventKind;
use crate::ids::PropIx;
use crate::state::{Fill, Unit};
use crate::tuning::{BODY_HALF_FX, NUDGE_CELLS};
use crate::units::place_unit;
use crate::zone::set_tile;

fn box_touches(u: &Unit, r: Rect) -> bool {
    let (x, y) = (u.pos.x.0, u.pos.y.0);
    x + BODY_HALF_FX > r.x * CELL_FX
        && x - BODY_HALF_FX < r.right() * CELL_FX
        && y + BODY_HALF_FX > r.y * CELL_FX
        && y - BODY_HALF_FX < r.bottom() * CELL_FX
}

/// Does `u`'s body box touch what feet meet of a prop of `def` with its footprint's top-left at
/// `cell` (`PropDef::solid_parts`, in sixteenths of a cell)? One standing in the notch behind it
/// does not.
pub fn box_touches_feet(u: &Unit, def: &PropDef, cell: (i32, i32)) -> bool {
    let (x, y) = (u.pos.x.0, u.pos.y.0);
    let s = CELL_FX / 16;
    def.solid_parts().iter().filter(|r| r.w > 0 && r.h > 0).any(|r| {
        let (rx, ry) = (cell.0 * CELL_FX + r.x * s, cell.1 * CELL_FX + r.y * s);
        x + BODY_HALF_FX > rx
            && x - BODY_HALF_FX < rx + r.w * s
            && y + BODY_HALF_FX > ry
            && y - BODY_HALF_FX < ry + r.h * s
    })
}

/// Move unit `ix` to the nearest cell it can stand on, reached over open floor from where it is.
/// `r` is what just became solid round it: the search may pass through it and may not end in
/// it. False when there is nowhere, and it stays put.
pub fn nudge_out(cx: &mut Ctx<'_>, ix: usize, r: Rect) -> bool {
    let u = &cx.zone.units[ix];
    let (sx, sy) = u.pos.cell();
    let own = Some((sx, sy));
    let grid = &cx.rt.grid;
    let queue = &mut cx.scratch.cells;
    queue.clear();
    queue.push((sx, sy));
    let mut q = 0;
    let mut found = None;
    while q < queue.len() && (queue.len() as u32) < NUDGE_CELLS {
        let (x, y) = queue[q];
        q += 1;
        if !r.contains(x, y) && grid.free(x, y, own) {
            found = Some((x, y));
            break;
        }
        // Fixed order: east, west, south, north. It is part of determinism.
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if !grid.inside(nx, ny) || (grid.solid(nx, ny) && !r.contains(nx, ny)) || queue.contains(&(nx, ny)) {
                continue;
            }
            queue.push((nx, ny));
        }
    }
    let Some((x, y)) = found else { return false };
    let u = &mut cx.zone.units[ix];
    place_unit(cx.rt, u, Vec2::centre(x, y));
    u.hold = 0;
    true
}

/// A prop has just become solid and visible: nobody may be left standing in it.
pub fn clear_footprint(cx: &mut Ctx<'_>, prop: PropIx) {
    let p = &cx.zone.props[prop as usize];
    if !p.solid || p.hidden {
        return;
    }
    let def = cx.cat.story.prop(p.def);
    let cell = (i32::from(p.cell.x), i32::from(p.cell.y));
    let r = def.solid_rect(cell.0, cell.1);
    let mut stamped = false;
    for ix in 0..cx.zone.units.len() {
        let u = &cx.zone.units[ix];
        if !u.alive || u.hidden || !box_touches_feet(u, def, cell) {
            continue;
        }
        // The grid must show the prop before anyone looks for a free cell, or its own cells look free.
        if !stamped {
            cx.rt.flush_prop_flags(cx.zone, &mut cx.scratch.props);
            stamped = true;
        }
        nudge_out(cx, ix, r);
    }
}

/// Does any living body touch this rect?
fn rect_held(cx: &Ctx<'_>, r: Rect) -> bool {
    cx.zone.units.iter().any(|u| u.alive && !u.hidden && box_touches(u, r))
}

fn lay_tiles(cx: &mut Ctx<'_>, r: Rect, tile: Tile) {
    for y in r.y..r.bottom() {
        for x in r.x..r.right() {
            if cx.rt.grid.inside(x, y) && cx.rt.grid.tile_at(x, y) != tile {
                set_tile(cx.zone, cx.rt, cx.bp, x, y, tile);
            }
        }
    }
    cx.emit(EventKind::Tiles(r));
}

/// `Fill`. A tile feet can cross goes down at once; a solid one goes down at once if nobody is
/// touching the rect, else it is owed and [`step_pending_fill`] pays it when the rect is clear.
/// A newer fill of the same ground wins over what an older one was still owed there.
pub fn fill_rect(cx: &mut Ctx<'_>, r: Rect, tile: Tile) {
    cx.zone.pending_fill.retain(|f| f.rect != r);
    if tile.flags() & F_SOLID != 0 && rect_held(cx, r) {
        cx.zone.pending_fill.push(Fill { rect: r, tile });
        return;
    }
    lay_tiles(cx, r, tile);
}

/// Housekeeping, every tick: the fills still owed, tried again in the order they were asked for.
pub fn step_pending_fill(cx: &mut Ctx<'_>) {
    let mut i = 0;
    while i < cx.zone.pending_fill.len() {
        let f = cx.zone.pending_fill[i];
        if rect_held(cx, f.rect) {
            i += 1;
            continue;
        }
        cx.zone.pending_fill.remove(i);
        lay_tiles(cx, f.rect, f.tile);
    }
}
