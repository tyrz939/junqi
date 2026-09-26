//! A zone's grid (`sim/grid.ts`): tiles, the flags byte, and occupancy. Derived: the tiles are
//! the blueprint's plus the zone's deltas, the flags are tile flags | prop flags | `F_OCC`.
//!
//! **Occupancy** is a count per cell of the units standing on it (awake, alive, unhidden), kept
//! sparse in a `Lookup` with the `F_OCC` bit in the flags byte so a hot loop reads the byte and
//! touches the map only when the bit is set. The TS kept the first unit to claim a cell, which
//! made who held a shared cell depend on the order units arrived in: a rebuilt runtime could
//! disagree with the live one. A count has no history. "Occupied by someone other than me" is
//! "occupied, and not the cell I stand on" (ARCHITECTURE.md §3.3; see `path.rs`).

use jane_core::grid::Grid;
use jane_core::tile::{BLOCK_MOVE, BLOCK_SIGHT, F_BLOCK_LOS, F_NOPUSH, F_OCC, F_PROP_LOS, F_PROP_SOLID, F_SOLID};
use jane_core::{CellIx, Lookup, Rect, Tile};

/// What is outside the grid: solid, and blocks sight.
pub const OUTSIDE: u8 = F_SOLID | F_BLOCK_LOS;
const KEEP_ON_TILE_CHANGE: u8 = jane_core::tile::KEEP_ON_TILE_CHANGE;

#[derive(Clone, Debug)]
pub struct ZoneGrid {
    tiles: Grid<Tile>,
    flags: Grid<u8>,
    occ: Lookup<CellIx, u16>,
}

impl ZoneGrid {
    /// From tiles; flags are the tiles' own, with nothing stamped and nobody standing.
    pub fn new(tiles: Grid<Tile>) -> Self {
        let flags = Grid::from_vec(tiles.w(), tiles.h(), tiles.as_slice().iter().map(|t| t.flags()).collect());
        Self { tiles, flags, occ: Lookup::with_capacity(256) }
    }

    pub fn w(&self) -> u32 {
        self.tiles.w()
    }

    pub fn h(&self) -> u32 {
        self.tiles.h()
    }

    #[inline]
    pub fn inside(&self, x: i32, y: i32) -> bool {
        self.tiles.inside(x, y)
    }

    #[inline]
    pub fn ix(&self, x: i32, y: i32) -> CellIx {
        CellIx(y as u32 * self.w() + x as u32)
    }

    #[inline]
    pub fn tile_at(&self, x: i32, y: i32) -> Tile {
        self.tiles.read(x, y, Tile::Void)
    }

    #[inline]
    pub fn flags_at(&self, x: i32, y: i32) -> u8 {
        self.flags.read(x, y, OUTSIDE)
    }

    /// The flags byte of an in-grid cell by index.
    #[inline]
    pub fn flags_ix(&self, i: CellIx) -> u8 {
        *self.flags.at(i)
    }

    pub fn tiles(&self) -> &Grid<Tile> {
        &self.tiles
    }

    /// Terrain or a solid prop in the way. Ignores units.
    #[inline]
    pub fn solid(&self, x: i32, y: i32) -> bool {
        self.flags_at(x, y) & BLOCK_MOVE != 0
    }

    /// A pushed prop may not be shoved onto this cell. Feet ignore it.
    pub fn no_push(&self, x: i32, y: i32) -> bool {
        self.flags_at(x, y) & F_NOPUSH != 0
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.flags_at(x, y) & BLOCK_SIGHT != 0
    }

    /// Change a tile, keeping what stands on the cell. Outside is a no-op. The caller records
    /// the delta (`zone::set_tile`).
    pub fn set_tile(&mut self, x: i32, y: i32, t: Tile) {
        if !self.inside(x, y) {
            return;
        }
        self.tiles.set(x, y, t);
        let f = self.flags_at(x, y);
        self.flags.set(x, y, (f & KEEP_ON_TILE_CHANGE) | t.flags());
    }

    /// Clear the prop bits of a rect (clipped).
    pub fn clear_prop_flags_in(&mut self, r: Rect) {
        let Some(r) = r.intersect(self.flags.bounds()) else { return };
        let keep = !(F_PROP_SOLID | F_PROP_LOS);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if let Some(f) = self.flags.get_mut(x, y) {
                    *f &= keep;
                }
            }
        }
    }

    pub fn clear_prop_flags(&mut self) {
        let keep = !(F_PROP_SOLID | F_PROP_LOS);
        for f in self.flags.as_mut_slice() {
            *f &= keep;
        }
    }

    /// Stamp a solid prop's footprint (clipped).
    pub fn stamp_prop(&mut self, r: Rect, block_los: bool) {
        let bits = F_PROP_SOLID | if block_los { F_PROP_LOS } else { 0 };
        let Some(r) = r.intersect(self.flags.bounds()) else { return };
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if let Some(f) = self.flags.get_mut(x, y) {
                    *f |= bits;
                }
            }
        }
    }

    // --- occupancy ---------------------------------------------------------------------

    /// How many units stand on the cell.
    pub fn occupants(&self, x: i32, y: i32) -> u16 {
        if !self.inside(x, y) {
            return 0;
        }
        let i = self.ix(x, y);
        if *self.flags.at(i) & F_OCC == 0 { 0 } else { self.occ.get(&i).copied().unwrap_or(0) }
    }

    /// One more unit stands here. Outside the grid is a no-op.
    pub fn occupy(&mut self, x: i32, y: i32) {
        if !self.inside(x, y) {
            return;
        }
        let i = self.ix(x, y);
        match self.occ.get_mut(&i) {
            Some(n) => *n += 1,
            None => {
                self.occ.insert(i, 1);
            }
        }
        *self.flags.at_mut(i) |= F_OCC;
    }

    /// One fewer. A cell nobody holds stays unheld.
    pub fn vacate(&mut self, x: i32, y: i32) {
        if !self.inside(x, y) {
            return;
        }
        let i = self.ix(x, y);
        let Some(n) = self.occ.get_mut(&i) else { return };
        *n -= 1;
        if *n == 0 {
            self.occ.remove(&i);
            *self.flags.at_mut(i) &= !F_OCC;
        }
    }

    /// Cells anyone stands on.
    pub fn occupied_cells(&self) -> u32 {
        self.occ.len() as u32
    }

    /// Free to stand on for a unit whose own cell is `own` (it does not count itself): not
    /// solid, and nobody else there.
    pub fn free(&self, x: i32, y: i32, own: Option<(i32, i32)>) -> bool {
        if !self.inside(x, y) || self.solid(x, y) {
            return false;
        }
        let n = self.occupants(x, y);
        n == 0 || (n == 1 && own == Some((x, y)))
    }

    /// The nearest free cell in growing square rings (2020's `PathTo` spiral, in the right
    /// units): top and bottom edges, then the two sides, in a fixed order.
    pub fn nearest_free(&self, cx: i32, cy: i32, max_radius: i32, own: Option<(i32, i32)>) -> Option<(i32, i32)> {
        if self.free(cx, cy, own) {
            return Some((cx, cy));
        }
        for r in 1..=max_radius {
            for d in -r..=r {
                for (x, y) in [(cx + d, cy - r), (cx + d, cy + r), (cx - r, cy + d), (cx + r, cy + d)] {
                    if self.free(x, y, own) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(w: u32, h: u32) -> ZoneGrid {
        ZoneGrid::new(Grid::new(w, h, Tile::Floor))
    }

    #[test]
    fn occupancy_is_a_count_and_tile_edits_keep_it() {
        // engine.test.ts "keeps occupancy sparse, and tile edits do not evict whoever stands there",
        // with a count in place of the first holder.
        let mut g = floor(3600, 2000);
        g.occupy(1800, 1000);
        assert_eq!(g.occupied_cells(), 1);
        assert!(!g.free(1800, 1000, None));
        assert!(g.free(1800, 1000, Some((1800, 1000))));
        g.occupy(1800, 1000);
        assert!(!g.free(1800, 1000, Some((1800, 1000))), "someone else is there too");
        g.set_tile(1800, 1000, Tile::Grass);
        assert_eq!(g.occupants(1800, 1000), 2);
        g.vacate(1800, 1000);
        g.vacate(1800, 1000);
        g.vacate(1800, 1000);
        assert_eq!(g.occupants(1800, 1000), 0);
        assert_eq!(g.flags_at(1800, 1000) & F_OCC, 0);
        assert!(g.free(1800, 1000, None));
    }

    #[test]
    fn props_stamp_and_clear_and_the_outside_is_wall() {
        let mut g = floor(8, 8);
        g.stamp_prop(Rect::new(6, 6, 5, 3), true);
        assert!(g.solid(7, 7) && g.blocks_sight(6, 6));
        g.clear_prop_flags_in(Rect::new(0, 0, 7, 7));
        assert!(!g.solid(6, 6) && g.solid(7, 7));
        assert!(g.solid(-1, 0) && g.blocks_sight(8, 0));
        assert_eq!(g.tile_at(99, 0), Tile::Void);
    }

    #[test]
    fn nearest_free_walks_out_in_rings() {
        let mut g = floor(9, 9);
        g.set_tile(4, 4, Tile::Wall);
        g.occupy(4, 3);
        assert_eq!(g.nearest_free(4, 4, 3, None), Some((3, 3)));
        assert_eq!(g.nearest_free(4, 3, 0, Some((4, 3))), Some((4, 3)));
        assert_eq!(g.nearest_free(4, 4, 0, None), None);
    }
}
