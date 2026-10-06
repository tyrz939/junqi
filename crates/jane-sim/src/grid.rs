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
use std::sync::Arc;

use jane_core::{Blueprint, CellIx, Lookup, Rect, Tile};

/// What is outside the grid: solid, and blocks sight.
pub const OUTSIDE: u8 = F_SOLID | F_BLOCK_LOS;
const KEEP_ON_TILE_CHANGE: u8 = jane_core::tile::KEEP_ON_TILE_CHANGE;

/// What feet meet in one cell (`ZoneGrid::feet_meet`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meets<'a> {
    Open,
    /// Terrain, the grid's edge, or a prop solid to its cell's edges.
    Whole,
    /// Props' feet over part of it: a row of sixteen bits (bit `i` = sixteenth `i` from the
    /// west) for each sixteenth of the cell from the north.
    Part(&'a [u16; 16]),
}

/// How far into a cliff's north lip cell feet may come, in sixteenths: under a body's height (a
/// box of `BODY_HALF_FX` each way, six), so a body in the lip always stands partly in the row
/// north of it and the lip opens no way past anything there: what connects is what the cells
/// said (the softlock proofs, the paths and the solver read the cells).
pub const CLIFF_LIP_OPEN: usize = 5;
const _: () = assert!((CLIFF_LIP_OPEN as i32) * (jane_core::num::CELL_FX / 16) < 2 * crate::tuning::BODY_HALF_FX);

/// A cliff's north lip cell to feet: open for its first [`CLIFF_LIP_OPEN`] rows, solid below.
static CLIFF_LIP: [u16; 16] = {
    let mut m = [u16::MAX; 16];
    let mut r = 0;
    while r < CLIFF_LIP_OPEN {
        m[r] = 0;
        r += 1;
    }
    m
};

/// Where a grid's tiles come from: its own (a test's grid), or the blueprint's, shared and never
/// copied (PLAY-PLAN.md §7: the county's 2000 x 2000 tiles were held twice).
#[derive(Clone, Debug)]
enum Base {
    Own(Grid<Tile>),
    Blueprint(Arc<Blueprint>),
}

#[derive(Clone, Debug)]
pub struct ZoneGrid {
    base: Base,
    /// The tiles changed since the blueprint (a cleared hedge, a filled pit), over a shared
    /// `base`: sparse, and empty in most zones (a read skips it then).
    changed: Lookup<CellIx, Tile>,
    flags: Grid<u8>,
    occ: Lookup<CellIx, u16>,
    /// The cells a solid prop stamps but whose feet (`PropDef::solid_parts`) cover only part of:
    /// what of each is solid to feet, in sixteenths. A cell stamped with no entry is solid whole.
    /// Paths, sight and the solver read the cell; only a moving body reads this.
    parts: Lookup<CellIx, [u16; 16]>,
    /// Which version of the flags this is (see [`ZoneGrid::generation`]).
    generation: u64,
}

/// Generations handed out, process wide: a grid made afresh (a runtime rebuilt, a zone streamed
/// in) never shares one with the grid it replaces.
static GENERATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_generation() -> u64 {
    GENERATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl ZoneGrid {
    /// A number that changes whenever a tile or a prop's stamp changes what the flags say (not
    /// occupancy): a reader that derived something from the flags (the bot's plan over blocks, its
    /// flood of where she can reach) knows it still holds while this is the same. Not state: never
    /// hashed, never saved, and no rule of the sim reads it.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// From tiles; flags are the tiles' own, with nothing stamped and nobody standing.
    pub fn new(tiles: Grid<Tile>) -> Self {
        let flags = Grid::from_vec(tiles.w(), tiles.h(), tiles.as_slice().iter().map(|t| t.flags()).collect());
        Self {
            base: Base::Own(tiles),
            changed: Lookup::new(),
            flags,
            occ: Lookup::with_capacity(256),
            parts: Lookup::with_capacity(256),
            generation: next_generation(),
        }
    }

    /// Over a blueprint's tiles, shared, with `deltas` (the zone's changed tiles) laid over them:
    /// the same grid [`new`](Self::new) makes of the blueprint's tiles with the deltas written in.
    pub fn over(bp: &Arc<Blueprint>, deltas: impl IntoIterator<Item = (CellIx, Tile)>) -> Self {
        let tiles = &bp.tiles;
        let flags = Grid::from_vec(tiles.w(), tiles.h(), tiles.as_slice().iter().map(|t| t.flags()).collect());
        let mut g = Self {
            base: Base::Blueprint(Arc::clone(bp)),
            changed: Lookup::new(),
            flags,
            occ: Lookup::with_capacity(256),
            parts: Lookup::with_capacity(256),
            generation: next_generation(),
        };
        for (i, t) in deltas {
            let (x, y) = ((i.0 % g.w()) as i32, (i.0 / g.w()) as i32);
            g.set_tile(x, y, t);
        }
        g
    }

    fn base(&self) -> &Grid<Tile> {
        match &self.base {
            Base::Own(g) => g,
            Base::Blueprint(bp) => &bp.tiles,
        }
    }

    /// What feet meet in cell `(x, y)`: the whole cell (terrain, outside, a prop solid to its
    /// edges), part of it (props' feet), or nothing.
    #[inline]
    pub fn feet_meet(&self, x: i32, y: i32) -> Meets<'_> {
        if !self.inside(x, y) {
            return Meets::Whole;
        }
        let i = self.ix(x, y);
        let f = *self.flags.at(i);
        if f & F_SOLID != 0 {
            // Raised ground's north lip: the rock is drawn from the cell's top edge, so feet come
            // up to it, a little past the edge (the owner's playtest, 2026-10-07: she stopped
            // short of a cliff from the north).
            if f & F_PROP_SOLID == 0 && self.tile_at(x, y) == Tile::Cliff && self.tile_at(x, y - 1) != Tile::Cliff {
                Meets::Part(&CLIFF_LIP)
            } else {
                Meets::Whole
            }
        } else if f & F_PROP_SOLID != 0 {
            self.parts.get(&i).map_or(Meets::Whole, Meets::Part)
        } else {
            Meets::Open
        }
    }

    pub fn w(&self) -> u32 {
        self.flags.w()
    }

    /// The grid's own heap (`Sim::mem`): the flags by cell, and the tiles where it holds them
    /// (its own, or the changed few over a blueprint's).
    pub fn heap_bytes(&self) -> usize {
        let n = self.flags.w() as usize * self.flags.h() as usize;
        let own = match &self.base {
            Base::Own(g) => std::mem::size_of_val(g.as_slice()),
            Base::Blueprint(_) => self.changed.len() * 16,
        };
        n + own
    }

    pub fn h(&self) -> u32 {
        self.flags.h()
    }

    #[inline]
    pub fn inside(&self, x: i32, y: i32) -> bool {
        self.flags.inside(x, y)
    }

    #[inline]
    pub fn ix(&self, x: i32, y: i32) -> CellIx {
        CellIx(y as u32 * self.w() + x as u32)
    }

    #[inline]
    pub fn tile_at(&self, x: i32, y: i32) -> Tile {
        if !self.changed.is_empty()
            && self.inside(x, y)
            && let Some(&t) = self.changed.get(&self.ix(x, y))
        {
            return t;
        }
        self.base().read(x, y, Tile::Void)
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
        match &mut self.base {
            Base::Own(g) => g.set(x, y, t),
            Base::Blueprint(bp) => {
                let i = CellIx(y as u32 * bp.tiles.w() + x as u32);
                if bp.tiles.read(x, y, Tile::Void) == t {
                    self.changed.remove(&i);
                } else {
                    self.changed.insert(i, t);
                }
            }
        }
        let f = self.flags_at(x, y);
        self.flags.set(x, y, (f & KEEP_ON_TILE_CHANGE) | t.flags());
        self.generation = next_generation();
    }

    /// Clear the prop bits of a rect (clipped).
    pub fn clear_prop_flags_in(&mut self, r: Rect) {
        let Some(r) = r.intersect(self.flags.bounds()) else { return };
        self.generation = next_generation();
        let keep = !(F_PROP_SOLID | F_PROP_LOS);
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if let Some(f) = self.flags.get_mut(x, y) {
                    *f &= keep;
                }
                let i = self.ix(x, y);
                self.parts.remove(&i);
            }
        }
    }

    pub fn clear_prop_flags(&mut self) {
        self.generation = next_generation();
        let keep = !(F_PROP_SOLID | F_PROP_LOS);
        for f in self.flags.as_mut_slice() {
            *f &= keep;
        }
        self.parts.clear();
    }

    /// Stamp a solid prop's footprint (clipped), solid to feet whole.
    pub fn stamp_prop(&mut self, r: Rect, block_los: bool) {
        let bits = F_PROP_SOLID | if block_los { F_PROP_LOS } else { 0 };
        let Some(r) = r.intersect(self.flags.bounds()) else { return };
        self.generation = next_generation();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if let Some(f) = self.flags.get_mut(x, y) {
                    *f |= bits;
                }
                let i = self.ix(x, y);
                self.parts.remove(&i);
            }
        }
    }

    /// Stamp a solid prop's footprint (clipped) whose feet are `parts`: rects in sixteenths of a
    /// cell on the grid (a cell's `x * 16`). The cells are solid as [`ZoneGrid::stamp_prop`]'s
    /// are; feet meet only the parts. A cell another prop already holds whole stays whole.
    pub fn stamp_prop_parts(&mut self, r: Rect, block_los: bool, parts: &[Rect; 3]) {
        let bits = F_PROP_SOLID | if block_los { F_PROP_LOS } else { 0 };
        let Some(r) = r.intersect(self.flags.bounds()) else { return };
        self.generation = next_generation();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let i = self.ix(x, y);
                let f = self.flags.at_mut(i);
                let whole = *f & F_PROP_SOLID != 0 && !self.parts.contains(&i);
                *f |= bits;
                if whole {
                    continue;
                }
                let mut m = self.parts.get(&i).copied().unwrap_or([0; 16]);
                let cell = Rect::new(x * 16, y * 16, 16, 16);
                for p in parts.iter().filter_map(|p| p.intersect(cell)) {
                    let row = (((1u32 << p.w) - 1) << (p.x - cell.x)) as u16;
                    for sy in p.y..p.bottom() {
                        m[(sy - cell.y) as usize] |= row;
                    }
                }
                self.parts.insert(i, m);
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
        Self::nearest(cx, cy, max_radius, |x, y| self.free(x, y, own))
    }

    /// [`nearest_free`](Self::nearest_free) for a unit put into the world (a spawn, a respawn): a
    /// free cell with an open cell beside it, so it is never put down boxed in.
    pub fn nearest_roomy(&self, cx: i32, cy: i32, max_radius: i32) -> Option<(i32, i32)> {
        Self::nearest(cx, cy, max_radius, |x, y| {
            self.free(x, y, None)
                && [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().any(|(dx, dy)| !self.solid(x + dx, y + dy))
        })
    }

    fn nearest(cx: i32, cy: i32, max_radius: i32, ok: impl Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
        if ok(cx, cy) {
            return Some((cx, cy));
        }
        for r in 1..=max_radius {
            for d in -r..=r {
                for (x, y) in [(cx + d, cy - r), (cx + d, cy + r), (cx - r, cy + d), (cx + r, cy + d)] {
                    if ok(x, y) {
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
        // A free cell walled in on four sides is no place to put a unit down.
        let mut b = floor(9, 9);
        for (x, y) in [(4, 3), (5, 4), (4, 5), (3, 4)] {
            b.set_tile(x, y, Tile::Wall);
        }
        assert_eq!(b.nearest_free(4, 4, 2, None), Some((4, 4)));
        assert_ne!(b.nearest_roomy(4, 4, 2), Some((4, 4)));
    }
}
