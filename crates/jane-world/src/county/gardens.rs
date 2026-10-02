//! Front gardens fenced (ART-PLAN M7 made true): every house of the county's tiles with a door in
//! its front and a garden two or three rows deep in front of it (`jane_core::garden::front`) gets
//! a fence along the garden's front row, with a two-cell gap under the door for its gate. The
//! painter draws the house's own boundary (pickets, a low wall, privet, railings) on exactly
//! these cells and its gate in the gap, so what she walks into is what she sees.
//!
//! A fence goes down only on plain garden ground nothing else wants: never on a way, a road, a
//! path, a door's step, a thing, a unit's cell or its patrol, or a mark; and a garden whose gate
//! would open onto something solid is left open. Then the flood from `start` is taken again,
//! and the garden nearest to any ground she could reach before and cannot now is left open,
//! until she reaches all she did. No dice.

use jane_core::Rect;
use jane_core::garden::{self, PLOT_ROWS};

use super::County;
use super::finish::{blocked_by_props, from_start};
use super::ways::trodden_at;

/// What a fence stands on.
const FENCE: jane_core::Tile = jane_core::Tile::Fence;

/// A garden fenced: its front row's cells, and the gate's west cell.
struct Laid {
    row: Rect,
    gate: i32,
}

impl Laid {
    /// The cells the fence stands on.
    fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (self.row.x..self.row.right()).filter(|&x| x != self.gate && x != self.gate + 1).map(|x| (x, self.row.y))
    }
}

/// The `gardens` stage.
pub fn fence_gardens(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let (w, h) = (c.k.w(), c.k.h());
    let bp = c.k.blueprint();
    let at = |x: i32, y: i32| (y * w + x) as usize;
    // Every cell something else wants: a thing's footprint, a unit and its patrol, a mark.
    let mut busy = vec![false; (w * h) as usize];
    let mut want = |x: i32, y: i32| {
        if c.k.inside(x, y) {
            busy[at(x, y)] = true;
        }
    };
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        for dy in 0..i32::from(d.h).max(1) {
            for dx in 0..i32::from(d.w).max(1) {
                want(x + dx, y + dy);
            }
        }
    }
    for u in &bp.units {
        want(i32::from(u.cell.x), i32::from(u.cell.y));
        for wp in &u.patrol {
            want(i32::from(wp.cell.x), i32::from(wp.cell.y));
        }
    }
    for m in bp.marks.values() {
        want(i32::from(m.cell.x), i32::from(m.cell.y));
    }
    // The doors as the painter finds them: a `door` sprite's west cell and row.
    let doors: Vec<(i32, i32)> = bp
        .props
        .iter()
        .filter(|p| cat.sprites.get(usize::from(cat.story.prop(p.def).sprite.0)) == Some(&"door"))
        .map(|p| (i32::from(p.cell.x), i32::from(p.cell.y)))
        .collect();
    let blocked = blocked_by_props(&c.k);
    let open = |x: i32, y: i32| c.k.inside(x, y) && !c.k.solid(x, y) && !blocked[at(x, y)];
    let mut laid = Vec::new();
    for b in garden::blocks((w, h), |x, y| c.k.get(x, y)) {
        let rect = b.rect;
        let Some(&(door, _)) = doors.iter().find(|d| d.1 >= b.eave && rect.contains(d.0, d.1)) else { continue };
        let (rows, fenced) = garden::front(rect, h, |x, y| c.k.get(x, y));
        if fenced || rows < 2 || rect.w < 4 || rect.w > 64 {
            continue;
        }
        // The garden as the painter will find it once fenced: the same rows, the last of them
        // its boundary (`front` reads two-thirds grass before it and half fence on it).
        let row = Rect::new(rect.x, rect.bottom() + rows - 1, rect.w, 1);
        let gate = garden::gate_x(Rect::new(rect.x, rect.bottom(), rect.w, rows), Some(door));
        let l = Laid { row, gate };
        let plain =
            l.cells().all(|(x, y)| garden::plot_ground(c.k.get(x, y)) && !busy[at(x, y)] && !trodden_at(c, x, y));
        // The gate open, the way out of it open, and the path from the door to it.
        let through = (rect.bottom()..=row.y + 1).all(|y| open(gate, y) || open(gate + 1, y));
        if plain && through {
            laid.push(l);
        }
    }
    if laid.is_empty() {
        return;
    }
    let before = from_start(&c.k, &blocked);
    let was: Vec<Vec<(i32, i32, jane_core::Tile)>> =
        laid.iter().map(|l| l.cells().map(|(x, y)| (x, y, c.k.get(x, y))).collect()).collect();
    for l in &laid {
        for (x, y) in l.cells() {
            c.k.set(x, y, FENCE);
        }
    }
    let Some(before) = before else { return };
    // Nothing she could reach before is shut away: the garden nearest each cell lost is opened
    // again, one at a time, until none is.
    let mut kept = vec![true; laid.len()];
    loop {
        let Some(after) = from_start(&c.k, &blocked) else { return };
        // (The fences' own cells are not lost: nobody stands in a fence.)
        let tiles = c.k.blueprint().tiles.as_slice();
        let Some(lost) = (0..before.len()).find(|&i| before[i] && !after[i] && tiles[i] != FENCE) else { return };
        let (lx, ly) = (lost as i32 % w, lost as i32 / w);
        let near = |l: &Laid| {
            let dx = (l.row.x - lx).max(lx - (l.row.right() - 1)).max(0);
            let dy = (l.row.y - PLOT_ROWS - ly).max(ly - l.row.y).max(0);
            dx.max(dy)
        };
        let Some(i) = (0..laid.len()).filter(|&i| kept[i]).min_by_key(|&i| (near(&laid[i]), i)) else { return };
        kept[i] = false;
        for &(x, y, t) in &was[i] {
            c.k.set(x, y, t);
        }
    }
}
