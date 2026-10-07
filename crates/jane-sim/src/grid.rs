//! A zone's grid (`sim/grid.ts`): tiles, the flags byte, and occupancy. Derived: the tiles are
//! the blueprint's plus the zone's deltas, the flags are tile flags | prop flags | `F_OCC`.
//!
//! **Occupancy** is a count per cell of the units standing on it (awake, alive, unhidden), kept
//! sparse in a `Lookup` with the `F_OCC` bit in the flags byte so a hot loop reads the byte and
//! touches the map only when the bit is set. The TS kept the first unit to claim a cell, which
//! made who held a shared cell depend on the order units arrived in: a rebuilt runtime could
//! disagree with the live one. A count has no history. "Occupied by someone other than me" is
//! "occupied, and not the cell I stand on" (ARCHITECTURE.md §3.3; see `path.rs`).

use alloc::sync::Arc;
use jane_core::grid::Grid;
use jane_core::tile::{BLOCK_MOVE, BLOCK_SIGHT, F_BLOCK_LOS, F_NOPUSH, F_OCC, F_PROP_LOS, F_PROP_SOLID, F_SOLID};

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
/// copied (PLAY-PLAN.md §7: the county's 2000 x 2000 tiles were held twice). Never written: a
/// changed tile is in `ZoneGrid::changed`.
#[derive(Clone, Debug)]
enum Base {
    Own(Grid<Tile>),
    Blueprint(Arc<Blueprint>),
}

impl Base {
    fn tiles(&self) -> &Grid<Tile> {
        match self {
            Base::Own(g) => g,
            Base::Blueprint(bp) => &bp.tiles,
        }
    }
}

/// Cells to a page of [`Flags`]: a run of this many cells in index order.
const RUN: usize = 64;
const RUN_SHIFT: u32 = RUN.trailing_zeros();

/// The most cells a grid holds its flags a byte a cell for (256 KB); a bigger one is paged. Every
/// zone but the county is under it.
const DENSE_MAX: usize = 1 << 18;

/// The flags byte of every cell, held as the base tiles' own flags plus pages where something
/// differs (PORT.md §13.3, phase 2): a prop's stamp, someone standing, a changed tile. A page is
/// [`RUN`] cells in index order, written whole when its first cell differs and let go when its
/// last cell is the base's again, so the plane costs what is stamped, not what is there (the
/// county's 4 M bytes were a byte a cell). A small zone keeps a byte a cell ([`DENSE_MAX`]): it
/// costs little there and reads in one step.
#[derive(Clone, Debug)]
struct Flags {
    w: u32,
    h: u32,
    /// A small zone's flags, a byte a cell; empty when paged.
    dense: Vec<u8>,
    /// The page of each run of cells; 0: none, the base tiles' flags.
    table: Vec<u16>,
    /// Pages by id; page 0 is never used.
    pool: Vec<[u8; RUN]>,
    /// Cells of each page that differ from the base's flags; a page at 0 is let go.
    differ: Vec<u8>,
    /// Pages let go, to use again.
    free: Vec<u16>,
}

impl Flags {
    /// Over `tiles`: a byte a cell up to `dense_max` cells, else paged.
    fn over(tiles: &Grid<Tile>, dense_max: usize) -> Self {
        let n = tiles.w() as usize * tiles.h() as usize;
        if n <= dense_max {
            let dense = tiles.as_slice().iter().map(|t| t.flags()).collect();
            return Self {
                w: tiles.w(),
                h: tiles.h(),
                dense,
                table: Vec::new(),
                pool: Vec::new(),
                differ: Vec::new(),
                free: Vec::new(),
            };
        }
        let mut pool = Vec::with_capacity(256);
        pool.push([0; RUN]);
        Self {
            w: tiles.w(),
            h: tiles.h(),
            dense: Vec::new(),
            table: alloc::vec![0; n.div_ceil(RUN)],
            pool,
            differ: alloc::vec![0; 1],
            free: Vec::with_capacity(256),
        }
    }

    /// The flags byte of cell `i` (in the grid), `base` the base tiles.
    #[inline]
    fn at(&self, base: &[Tile], i: usize) -> u8 {
        if !self.dense.is_empty() {
            return self.dense[i];
        }
        match self.table[i >> RUN_SHIFT] {
            0 => base[i].flags(),
            p => self.pool[usize::from(p)][i & (RUN - 1)],
        }
    }

    /// Write cell `i`'s flags byte.
    fn set(&mut self, base: &[Tile], i: usize, f: u8) {
        if !self.dense.is_empty() {
            self.dense[i] = f;
            return;
        }
        let run = i >> RUN_SHIFT;
        let mut p = self.table[run];
        if p == 0 {
            if base[i].flags() == f {
                return;
            }
            p = self.page(base, run);
        }
        let pu = usize::from(p);
        let cell = &mut self.pool[pu][i & (RUN - 1)];
        let was = *cell != base[i].flags();
        *cell = f;
        let now = f != base[i].flags();
        match (was, now) {
            (false, true) => self.differ[pu] += 1,
            (true, false) => {
                self.differ[pu] -= 1;
                if self.differ[pu] == 0 {
                    self.table[run] = 0;
                    self.free.push(p);
                }
            }
            _ => {}
        }
    }

    /// A page for run `run`, filled with the base's flags.
    fn page(&mut self, base: &[Tile], run: usize) -> u16 {
        let lo = run << RUN_SHIFT;
        let mut fresh = [0u8; RUN];
        for (k, f) in fresh.iter_mut().enumerate() {
            *f = base.get(lo + k).map_or(0, |t| t.flags());
        }
        let p = match self.free.pop() {
            Some(p) => {
                self.pool[usize::from(p)] = fresh;
                p
            }
            None => {
                // A page id is a u16: the county's 2000 x 2000 is 62 500 runs, all of which fit.
                assert!(self.pool.len() < usize::from(u16::MAX), "more pages of flags than a u16 names");
                self.pool.push(fresh);
                self.differ.push(0);
                (self.pool.len() - 1) as u16
            }
        };
        self.differ[usize::from(p)] = 0;
        self.table[run] = p;
        p
    }

    /// Clear `bits` in every cell (the base's flags never hold them).
    fn clear_all(&mut self, base: &[Tile], bits: u8) {
        for f in &mut self.dense {
            *f &= !bits;
        }
        for run in 0..self.table.len() {
            if self.table[run] == 0 {
                continue;
            }
            let lo = run << RUN_SHIFT;
            for i in lo..(lo + RUN).min(base.len()) {
                let p = usize::from(self.table[run]);
                if p == 0 {
                    break;
                }
                let f = self.pool[p][i & (RUN - 1)];
                if f & bits != 0 {
                    self.set(base, i, f & !bits);
                }
            }
        }
    }

    fn heap_bytes(&self) -> usize {
        self.dense.capacity() + self.table.capacity() * 2 + self.pool.capacity() * RUN + self.differ.capacity() + self.free.capacity() * 2
    }

    /// Pages held now.
    fn pages(&self) -> usize {
        self.pool.len().saturating_sub(1 + self.free.len())
    }
}

#[derive(Clone, Debug)]
pub struct ZoneGrid {
    base: Base,
    /// The tiles changed since the base (a cleared hedge, a filled pit): sparse, and empty in
    /// most zones (a read skips it then).
    changed: Lookup<CellIx, Tile>,
    flags: Flags,
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
#[cfg(target_has_atomic = "64")]
static GENERATIONS: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1);

#[cfg(target_has_atomic = "64")]
fn next_generation() -> u64 {
    GENERATIONS.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
}

/// The same counter where the target has no 64-bit atomics (the PSP, PORT.md §13.9). It wraps
/// after four billion grids, where a reader could take a fresh grid for one it saw then; no rule of
/// the sim reads a generation, so nothing it steps, saves or hashes can differ.
#[cfg(not(target_has_atomic = "64"))]
static GENERATIONS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);

#[cfg(not(target_has_atomic = "64"))]
fn next_generation() -> u64 {
    u64::from(GENERATIONS.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
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
        Self {
            flags: Flags::over(&tiles, DENSE_MAX),
            base: Base::Own(tiles),
            changed: Lookup::new(),
            occ: Lookup::with_capacity(256),
            parts: Lookup::with_capacity(256),
            generation: next_generation(),
        }
    }

    /// Over a blueprint's tiles, shared, with `deltas` (the zone's changed tiles) laid over them:
    /// the same grid [`new`](Self::new) makes of the blueprint's tiles with the deltas written in.
    pub fn over(bp: &Arc<Blueprint>, deltas: impl IntoIterator<Item = (CellIx, Tile)>) -> Self {
        let mut g = Self {
            flags: Flags::over(&bp.tiles, DENSE_MAX),
            base: Base::Blueprint(Arc::clone(bp)),
            changed: Lookup::new(),
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

    /// The flags byte of in-grid cell `i`.
    #[inline]
    fn flag(&self, i: CellIx) -> u8 {
        self.flags.at(self.base.tiles().as_slice(), i.0 as usize)
    }

    /// Write the flags byte of in-grid cell `i`.
    #[inline]
    fn put(&mut self, i: CellIx, f: u8) {
        self.flags.set(self.base.tiles().as_slice(), i.0 as usize, f);
    }

    /// What feet meet in cell `(x, y)`: the whole cell (terrain, outside, a prop solid to its
    /// edges), part of it (props' feet), or nothing.
    #[inline]
    pub fn feet_meet(&self, x: i32, y: i32) -> Meets<'_> {
        if !self.inside(x, y) {
            return Meets::Whole;
        }
        let i = self.ix(x, y);
        let f = self.flag(i);
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
        self.flags.w
    }

    /// The grid's own heap (`Sim::mem`): the flags' pages and table, and the tiles where it holds
    /// them (its own, or the changed few over a blueprint's).
    pub fn heap_bytes(&self) -> usize {
        let own = match &self.base {
            Base::Own(g) => core::mem::size_of_val(g.as_slice()),
            Base::Blueprint(_) => 0,
        };
        self.flags.heap_bytes() + own + self.changed.len() * 16
    }

    /// Pages of flags held: runs of cells where a stamp, a body or a changed tile makes the flags
    /// differ from the tiles' own (`jane bench heap`).
    pub fn flag_pages(&self) -> usize {
        self.flags.pages()
    }

    pub fn h(&self) -> u32 {
        self.flags.h
    }

    #[inline]
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.flags.w && (y as u32) < self.flags.h
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
        self.base.tiles().read(x, y, Tile::Void)
    }

    #[inline]
    pub fn flags_at(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) { self.flag(self.ix(x, y)) } else { OUTSIDE }
    }

    /// The flags byte of an in-grid cell by index.
    #[inline]
    pub fn flags_ix(&self, i: CellIx) -> u8 {
        self.flag(i)
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
        let i = self.ix(x, y);
        if self.base.tiles().read(x, y, Tile::Void) == t {
            self.changed.remove(&i);
        } else {
            self.changed.insert(i, t);
        }
        let f = self.flag(i);
        self.put(i, (f & KEEP_ON_TILE_CHANGE) | t.flags());
        self.generation = next_generation();
    }

    /// Clear the prop bits of a rect (clipped).
    pub fn clear_prop_flags_in(&mut self, r: Rect) {
        let Some(r) = r.intersect(self.bounds()) else { return };
        self.generation = next_generation();
        let bits = F_PROP_SOLID | F_PROP_LOS;
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let i = self.ix(x, y);
                let f = self.flag(i);
                if f & bits != 0 {
                    self.put(i, f & !bits);
                }
                self.parts.remove(&i);
            }
        }
    }

    pub fn clear_prop_flags(&mut self) {
        self.generation = next_generation();
        let base = match &self.base {
            Base::Own(g) => g.as_slice(),
            Base::Blueprint(bp) => bp.tiles.as_slice(),
        };
        self.flags.clear_all(base, F_PROP_SOLID | F_PROP_LOS);
        self.parts.clear();
    }

    fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.flags.w as i32, self.flags.h as i32)
    }

    /// Stamp a solid prop's footprint (clipped), solid to feet whole.
    pub fn stamp_prop(&mut self, r: Rect, block_los: bool) {
        let bits = F_PROP_SOLID | if block_los { F_PROP_LOS } else { 0 };
        let Some(r) = r.intersect(self.bounds()) else { return };
        self.generation = next_generation();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let i = self.ix(x, y);
                let f = self.flag(i);
                self.put(i, f | bits);
                self.parts.remove(&i);
            }
        }
    }

    /// Stamp a solid prop's footprint (clipped) whose feet are `parts`: rects in sixteenths of a
    /// cell on the grid (a cell's `x * 16`). The cells are solid as [`ZoneGrid::stamp_prop`]'s
    /// are; feet meet only the parts. A cell another prop already holds whole stays whole.
    pub fn stamp_prop_parts(&mut self, r: Rect, block_los: bool, parts: &[Rect; 3]) {
        let bits = F_PROP_SOLID | if block_los { F_PROP_LOS } else { 0 };
        let Some(r) = r.intersect(self.bounds()) else { return };
        self.generation = next_generation();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let i = self.ix(x, y);
                let f = self.flag(i);
                let whole = f & F_PROP_SOLID != 0 && !self.parts.contains(&i);
                self.put(i, f | bits);
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
        if self.flag(i) & F_OCC == 0 { 0 } else { self.occ.get(&i).copied().unwrap_or(0) }
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
                let f = self.flag(i);
                self.put(i, f | F_OCC);
            }
        }
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
            let f = self.flag(i);
            self.put(i, f & !F_OCC);
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

    /// The paged flags read what a byte a cell read (the plane before PORT.md §13.3's phase 2),
    /// through every write, and hold no page once nothing differs from the tiles.
    #[test]
    fn paged_flags_read_as_a_byte_a_cell() {
        let (w, h) = (93u32, 71u32);
        let mut s = 0x9e37_79b9u32;
        let mut next = |n: u32| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s % n
        };
        let tiles: Vec<Tile> = (0..w * h).map(|_| Tile::ALL[next(Tile::ALL.len() as u32) as usize]).collect();
        let mut g = ZoneGrid::new(Grid::from_vec(w, h, tiles.clone()));
        g.flags = Flags::over(g.base.tiles(), 0);
        let mut want: Vec<u8> = tiles.iter().map(|t| t.flags()).collect();
        let mut occ = alloc::vec![0u16; (w * h) as usize];
        let at = |x: i32, y: i32| (y as u32 * w + x as u32) as usize;
        for step in 0..4000 {
            let (x, y) = (next(w) as i32, next(h) as i32);
            match next(6) {
                0 => {
                    let r = Rect::new(x - 1, y - 1, next(9) as i32 + 1, next(5) as i32 + 1);
                    let los = next(2) == 0;
                    g.stamp_prop(r, los);
                    if let Some(r) = r.intersect(Rect::new(0, 0, w as i32, h as i32)) {
                        for (cx, cy) in r.cells() {
                            want[at(cx, cy)] |= F_PROP_SOLID | if los { F_PROP_LOS } else { 0 };
                        }
                    }
                }
                1 => {
                    let r = Rect::new(x - 2, y - 2, next(12) as i32, next(12) as i32);
                    g.clear_prop_flags_in(r);
                    if let Some(r) = r.intersect(Rect::new(0, 0, w as i32, h as i32)) {
                        for (cx, cy) in r.cells() {
                            want[at(cx, cy)] &= !(F_PROP_SOLID | F_PROP_LOS);
                        }
                    }
                }
                2 => {
                    g.occupy(x, y);
                    occ[at(x, y)] += 1;
                    want[at(x, y)] |= F_OCC;
                }
                3 => {
                    g.vacate(x, y);
                    let n = &mut occ[at(x, y)];
                    if *n > 0 {
                        *n -= 1;
                        if *n == 0 {
                            want[at(x, y)] &= !F_OCC;
                        }
                    }
                }
                4 => {
                    let t = Tile::ALL[next(Tile::ALL.len() as u32) as usize];
                    g.set_tile(x, y, t);
                    want[at(x, y)] = (want[at(x, y)] & KEEP_ON_TILE_CHANGE) | t.flags();
                }
                _ if step % 500 == 0 => {
                    g.clear_prop_flags();
                    for f in &mut want {
                        *f &= !(F_PROP_SOLID | F_PROP_LOS);
                    }
                }
                _ => {}
            }
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    assert_eq!(g.flags_at(x, y), want[at(x, y)], "step {step} at {x},{y}");
                }
            }
        }
        // Everything undone: no page is held.
        g.clear_prop_flags();
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                while g.occupants(x, y) > 0 {
                    g.vacate(x, y);
                }
                g.set_tile(x, y, tiles[at(x, y)]);
            }
        }
        assert_eq!(g.flag_pages(), 0);
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
