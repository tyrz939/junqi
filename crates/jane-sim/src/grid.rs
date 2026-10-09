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
use alloc::vec::Vec;
use jane_core::grid::Grid;
use jane_core::tile::{BLOCK_MOVE, BLOCK_SIGHT, F_BLOCK_LOS, F_NOPUSH, F_OCC, F_PROP_LOS, F_PROP_SOLID, F_SOLID};

use jane_core::blueprint::{Span, SpanIx};
use jane_core::plane::CHUNK;
use jane_core::tile::FLAT_LEVEL;
use jane_core::{Blueprint, CellIx, Lookup, Plane, Rect, Tile};

use crate::regions::Regions;

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
    /// The tile at in-grid cell `(x, y)`, index `i`.
    #[inline]
    fn tile(&self, x: u32, y: u32, i: usize) -> Tile {
        match self {
            Base::Own(g) => g.as_slice()[i],
            Base::Blueprint(bp) => bp.tile_in(x, y, i),
        }
    }

    /// The tile at in-grid cell index `i` of a grid `w` wide.
    #[inline]
    fn tile_ix(&self, w: u32, i: usize) -> Tile {
        self.tile(i as u32 % w, i as u32 / w, i)
    }

    /// The tile at `(x, y)`, `Tile::Void` outside.
    fn read(&self, x: i32, y: i32) -> Tile {
        match self {
            Base::Own(g) => g.read(x, y, Tile::Void),
            Base::Blueprint(bp) => bp.tile(x, y),
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
    /// Over `base`, `w x h`: a byte a cell up to `dense_max` cells, else paged.
    fn over(base: &Base, w: u32, h: u32, dense_max: usize) -> Self {
        let n = w as usize * h as usize;
        if n <= dense_max {
            let dense = (0..n).map(|i| base.tile_ix(w, i).flags()).collect();
            return Self { w, h, dense, table: Vec::new(), pool: Vec::new(), differ: Vec::new(), free: Vec::new() };
        }
        let mut pool = Vec::with_capacity(256);
        pool.push([0; RUN]);
        Self {
            w,
            h,
            dense: Vec::new(),
            table: alloc::vec![0; n.div_ceil(RUN)],
            pool,
            differ: alloc::vec![0; 1],
            free: Vec::with_capacity(256),
        }
    }

    /// The flags byte of in-grid cell `(x, y)`, index `i`, over `base`.
    #[inline]
    fn at(&self, base: &Base, x: u32, y: u32, i: usize) -> u8 {
        if !self.dense.is_empty() {
            return self.dense[i];
        }
        match self.table[i >> RUN_SHIFT] {
            0 => base.tile(x, y, i).flags(),
            p => self.pool[usize::from(p)][i & (RUN - 1)],
        }
    }

    /// Write cell `i`'s flags byte.
    fn set(&mut self, base: &Base, i: usize, f: u8) {
        if !self.dense.is_empty() {
            self.dense[i] = f;
            return;
        }
        let run = i >> RUN_SHIFT;
        let mut p = self.table[run];
        let own = base.tile_ix(self.w, i).flags();
        if p == 0 {
            if own == f {
                return;
            }
            p = self.page(base, run);
        }
        let pu = usize::from(p);
        let cell = &mut self.pool[pu][i & (RUN - 1)];
        let was = *cell != own;
        *cell = f;
        let now = f != own;
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
    fn page(&mut self, base: &Base, run: usize) -> u16 {
        let lo = run << RUN_SHIFT;
        let n = self.w as usize * self.h as usize;
        let mut fresh = [0u8; RUN];
        for (k, f) in fresh.iter_mut().enumerate() {
            if lo + k < n {
                *f = base.tile_ix(self.w, lo + k).flags();
            }
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
    fn clear_all(&mut self, base: &Base, bits: u8) {
        for f in &mut self.dense {
            *f &= !bits;
        }
        let n = self.w as usize * self.h as usize;
        for run in 0..self.table.len() {
            if self.table[run] == 0 {
                continue;
            }
            let lo = run << RUN_SHIFT;
            for i in lo..(lo + RUN).min(n) {
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
        self.dense.capacity()
            + self.table.capacity() * 2
            + self.pool.capacity() * RUN
            + self.differ.capacity()
            + self.free.capacity() * 2
    }

    /// Pages held now.
    fn pages(&self) -> usize {
        self.pool.len().saturating_sub(1 + self.free.len())
    }
}

/// Masks of what of a cell is solid to feet, in sixteenths, each held once (PORT.md §13.3,
/// phase 3): a cell names its mask by number, where it held the 32 bytes.
#[derive(Clone, Debug, Default)]
struct Masks {
    list: Vec<[u16; 16]>,
    index: Lookup<[u16; 16], u16>,
}

impl Masks {
    #[inline]
    fn get(&self, k: u16) -> &[u16; 16] {
        &self.list[usize::from(k)]
    }

    /// The number of mask `m`, made if it is new.
    fn id(&mut self, m: [u16; 16]) -> u16 {
        if let Some(&k) = self.index.get(&m) {
            return k;
        }
        assert!(self.list.len() < usize::from(u16::MAX), "more masks of feet than a u16 names");
        let k = self.list.len() as u16;
        self.list.push(m);
        self.index.insert(m, k);
        k
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
    parts: Lookup<CellIx, u16>,
    /// The distinct masks `parts` names (a few dozen shapes over thousands of cells).
    masks: Masks,
    /// Which version of the flags this is (see [`ZoneGrid::generation`]).
    generation: u64,
    /// A test grid's own level plane ([`ZoneGrid::with_levels`]); a blueprint's is read through it.
    own_level: Option<alloc::boxed::Box<Plane>>,
    /// The level plane's chunks where the ground's level changes, in them or beside them (MAP.md
    /// §3.2's fast path), a bit a chunk; empty for a zone without levels, whose every rule is
    /// the flat one.
    edges: Vec<u64>,
    /// The spans (MAP.md §2.5): the blueprint's, or a test grid's ([`ZoneGrid::with_spans`]).
    spans: Vec<Span>,
    /// Which spans are broken now (the zone's `spans_broken`), a bit each.
    broken: u64,
    /// `(block, span)` for each 16-cell block a span's rect, grown by a cell, touches; sorted.
    span_blocks: Vec<(u32, SpanIx)>,
    /// The level regions (`regions.rs`): built with a grid that has levels, again when a barrier
    /// tile or a span's state changes.
    regions: Option<alloc::boxed::Box<Regions>>,
}

/// A bit for each chunk of `p` whose cells hold more than one level or that touches (eight ways)
/// a chunk at another level: where a sight line needs the levels.
fn level_edges(p: &Plane) -> Vec<u64> {
    let (cw, ch) = p.chunks();
    let mut bits = alloc::vec![0u64; (cw as usize * ch as usize).div_ceil(64).max(1)];
    for cy in 0..ch {
        for cx in 0..cw {
            let me = p.chunk_uniform(cx, cy);
            let edge = me.is_none()
                || (cy.saturating_sub(1)..(cy + 2).min(ch))
                    .any(|y| (cx.saturating_sub(1)..(cx + 2).min(cw)).any(|x| p.chunk_uniform(x, y) != me));
            if edge {
                let k = (cy * cw + cx) as usize;
                bits[k / 64] |= 1 << (k % 64);
            }
        }
    }
    bits
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
        let (w, h) = (tiles.w(), tiles.h());
        let base = Base::Own(tiles);
        Self {
            flags: Flags::over(&base, w, h, DENSE_MAX),
            base,
            changed: Lookup::new(),
            occ: Lookup::with_capacity(256),
            parts: Lookup::with_capacity(256),
            masks: Masks::default(),
            generation: next_generation(),
            own_level: None,
            edges: Vec::new(),
            spans: Vec::new(),
            broken: 0,
            span_blocks: Vec::new(),
            regions: None,
        }
    }

    /// From tiles over ground of `levels` (MAP.md §2.2): a hand-built grid with height, for tests.
    pub fn with_levels(tiles: Grid<Tile>, levels: Plane) -> Self {
        assert_eq!((levels.w(), levels.h()), (tiles.w(), tiles.h()), "the levels are the tiles' size");
        let mut g = Self::new(tiles);
        g.edges = level_edges(&levels);
        g.own_level = Some(alloc::boxed::Box::new(levels));
        g.rebuild_regions();
        g
    }

    /// With these spans (whole or broken as each says): a hand-built grid's, for tests.
    pub fn with_spans(mut self, spans: alloc::vec::Vec<Span>) -> Self {
        self.set_spans(spans);
        self
    }

    fn set_spans(&mut self, spans: Vec<Span>) {
        assert!(spans.len() <= jane_core::blueprint::SPANS_MAX, "too many spans");
        self.broken = spans.iter().enumerate().fold(0, |b, (i, s)| b | u64::from(s.broken) << i);
        let bw = self.w().div_ceil(CHUNK);
        self.span_blocks.clear();
        for (i, s) in spans.iter().enumerate() {
            let Some(r) = s.rect.grow(1).intersect(self.bounds()) else { continue };
            for by in (r.y as u32 / CHUNK)..=((r.bottom() - 1) as u32 / CHUNK) {
                for bx in (r.x as u32 / CHUNK)..=((r.right() - 1) as u32 / CHUNK) {
                    self.span_blocks.push((by * bw + bx, i as SpanIx));
                }
            }
        }
        self.span_blocks.sort_by_key(|&k| k);
        self.spans = spans;
        self.rebuild_regions();
    }

    /// Build the level regions afresh (a grid with levels), or drop them (one without).
    fn rebuild_regions(&mut self) {
        self.regions = if self.has_levels() { Regions::build(self).map(alloc::boxed::Box::new) } else { None };
    }

    /// Over a blueprint's tiles, shared, with `deltas` (the zone's changed tiles) laid over them:
    /// the same grid [`new`](Self::new) makes of the blueprint's tiles with the deltas written in.
    pub fn over(bp: &Arc<Blueprint>, deltas: impl IntoIterator<Item = (CellIx, Tile)>) -> Self {
        let base = Base::Blueprint(Arc::clone(bp));
        let mut g = Self {
            flags: Flags::over(&base, bp.w(), bp.h(), DENSE_MAX),
            base,
            changed: Lookup::new(),
            occ: Lookup::with_capacity(256),
            parts: Lookup::with_capacity(256),
            masks: Masks::default(),
            generation: next_generation(),
            own_level: None,
            edges: bp.level.as_ref().map_or_else(Vec::new, level_edges),
            spans: Vec::new(),
            broken: 0,
            span_blocks: Vec::new(),
            regions: None,
        };
        for (i, t) in deltas {
            let (x, y) = ((i.0 % g.w()) as i32, (i.0 / g.w()) as i32);
            g.set_tile(x, y, t);
        }
        if bp.spans.is_empty() {
            g.rebuild_regions();
        } else {
            g.set_spans(bp.spans.clone());
        }
        g
    }

    /// The flags byte of in-grid cell `i`.
    #[inline]
    fn flag(&self, i: CellIx) -> u8 {
        self.flags.at(&self.base, i.0 % self.flags.w, i.0 / self.flags.w, i.0 as usize)
    }

    /// The flags byte of in-grid cell `(x, y)`.
    #[inline]
    fn flag_xy(&self, x: i32, y: i32) -> u8 {
        let i = y as u32 * self.flags.w + x as u32;
        self.flags.at(&self.base, x as u32, y as u32, i as usize)
    }

    /// Write the flags byte of in-grid cell `i`.
    #[inline]
    fn put(&mut self, i: CellIx, f: u8) {
        self.flags.set(&self.base, i.0 as usize, f);
    }

    /// What feet meet in cell `(x, y)`: the whole cell (terrain, outside, a prop solid to its
    /// edges), part of it (props' feet), or nothing.
    #[inline]
    pub fn feet_meet(&self, x: i32, y: i32) -> Meets<'_> {
        if !self.inside(x, y) {
            return Meets::Whole;
        }
        let f = self.flag_xy(x, y);
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
            self.parts.get(&self.ix(x, y)).map_or(Meets::Whole, |&k| Meets::Part(self.masks.get(k)))
        } else {
            Meets::Open
        }
    }

    pub fn w(&self) -> u32 {
        self.flags.w
    }

    // --- height (MAP.md §2, §3) --------------------------------------------------------------

    /// Does this zone have levels at all? A zone without them (every zone built so far) never
    /// asks anything below, and every rule of the sim is the flat one there.
    #[inline]
    pub fn has_levels(&self) -> bool {
        !self.edges.is_empty()
    }

    fn level_plane(&self) -> Option<&Plane> {
        match &self.base {
            Base::Blueprint(bp) => bp.level.as_ref(),
            Base::Own(_) => self.own_level.as_deref(),
        }
    }

    /// The level of the ground at `(x, y)`, 0 to 3: [`FLAT_LEVEL`] in a zone without levels and
    /// outside the grid.
    #[inline]
    pub fn level_at(&self, x: i32, y: i32) -> u8 {
        if self.edges.is_empty() {
            return FLAT_LEVEL;
        }
        self.level_plane().map_or(FLAT_LEVEL, |p| p.read(x, y, FLAT_LEVEL))
    }

    /// Is every chunk the cell box `a..=b` touches one level, with no change of level in or
    /// beside it? Then the ground is the same under the whole box, and sight across it is the
    /// flat rule (MAP.md §3.2's fast path). True in a zone without levels.
    pub fn flat_between(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        if self.edges.is_empty() {
            return true;
        }
        let last = |n: u32| (n.max(1) - 1) as i32;
        let cw = self.w().div_ceil(CHUNK);
        let s = CHUNK.trailing_zeros();
        let cx0 = a.0.min(b.0).clamp(0, last(self.w())) >> s;
        let cx1 = a.0.max(b.0).clamp(0, last(self.w())) >> s;
        let cy0 = a.1.min(b.1).clamp(0, last(self.h())) >> s;
        let cy1 = a.1.max(b.1).clamp(0, last(self.h())) >> s;
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                let k = (cy as u32 * cw + cx as u32) as usize;
                if self.edges[k / 64] >> (k % 64) & 1 != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Is `(x, y)` in a chunk where the level changes, in it or beside it? False in a zone without
    /// levels.
    pub fn level_changes_near(&self, x: i32, y: i32) -> bool {
        if self.edges.is_empty() || !self.inside(x, y) {
            return false;
        }
        let k = ((y as u32 / CHUNK) * self.w().div_ceil(CHUNK) + x as u32 / CHUNK) as usize;
        self.edges[k / 64] >> (k % 64) & 1 != 0
    }

    /// The level regions (`regions.rs`), in a zone with levels.
    #[inline]
    pub fn regions(&self) -> Option<&Regions> {
        self.regions.as_deref()
    }

    // --- spans (MAP.md §2.5) ---------------------------------------------------------------

    /// Does this zone have spans at all? A zone without them never asks anything below.
    #[inline]
    pub fn has_spans(&self) -> bool {
        !self.spans.is_empty()
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Is span `i` whole (it has a deck)?
    #[inline]
    pub fn span_whole(&self, i: SpanIx) -> bool {
        usize::from(i) < self.spans.len() && self.broken >> i & 1 == 0
    }

    /// The broken spans, a bit each.
    pub fn spans_broken(&self) -> u64 {
        self.broken
    }

    /// Break span `i` or make it whole (the zone's state says which; `zone::set_span_broken`).
    pub fn set_span_broken(&mut self, i: SpanIx, broken: bool) {
        let bit = 1u64 << i;
        let next = if broken { self.broken | bit } else { self.broken & !bit };
        if next == self.broken {
            return;
        }
        self.broken = next;
        if let Some(mut r) = self.regions.take() {
            r.portals(self);
            self.regions = Some(r);
        }
    }

    /// Every span listed in the block of `(x, y)` (its rect grown by a cell touches the block).
    #[inline]
    fn spans_in_block(&self, x: i32, y: i32) -> impl Iterator<Item = SpanIx> + '_ {
        let b = if self.spans.is_empty() || !self.inside(x, y) {
            u32::MAX
        } else {
            (y as u32 / CHUNK) * self.w().div_ceil(CHUNK) + x as u32 / CHUNK
        };
        let from = self.span_blocks.partition_point(|&(k, _)| k < b);
        self.span_blocks[from..].iter().take_while(move |&&(k, _)| k == b).map(|&(_, i)| i)
    }

    /// The whole span whose deck lies over `(x, y)`, if any.
    #[inline]
    pub fn deck_at(&self, x: i32, y: i32) -> Option<SpanIx> {
        self.spans_in_block(x, y).find(|&i| self.span_whole(i) && self.spans[usize::from(i)].rect.contains(x, y))
    }

    /// The whole span one of whose ends `(x, y)` is, with ground at its deck's level: where feet
    /// step onto it. With which end.
    #[inline]
    pub fn deck_end_at(&self, x: i32, y: i32) -> Option<(SpanIx, usize)> {
        self.spans_in_block(x, y).find_map(|i| {
            let s = &self.spans[usize::from(i)];
            let e = s.end_of(x, y)?;
            (self.span_whole(i) && self.level_at(x, y) == s.deck_level).then_some((i, e))
        })
    }

    /// Does the cell box `a..=b` touch a span (its rect grown by a cell)? False in a zone without.
    pub fn span_near(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        if self.spans.is_empty() {
            return false;
        }
        let r = Rect::new(a.0.min(b.0), a.1.min(b.1), (a.0 - b.0).abs() + 1, (a.1 - b.1).abs() + 1);
        self.spans.iter().any(|s| s.rect.grow(1).overlaps(r))
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
        self.base.read(x, y)
    }

    #[inline]
    pub fn flags_at(&self, x: i32, y: i32) -> u8 {
        if self.inside(x, y) { self.flag_xy(x, y) } else { OUTSIDE }
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
        let was = if self.regions.is_some() { self.tile_at(x, y) } else { t };
        if self.base.read(x, y) == t {
            self.changed.remove(&i);
        } else {
            self.changed.insert(i, t);
        }
        let f = self.flag(i);
        self.put(i, (f & KEEP_ON_TILE_CHANGE) | t.flags());
        self.generation = next_generation();
        // A face that opens (Grow's vines) or closes changes the level regions: built again.
        if self.regions.is_some() && crate::regions::barrier(was) != crate::regions::barrier(t) {
            self.rebuild_regions();
        }
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
        self.flags.clear_all(&self.base, F_PROP_SOLID | F_PROP_LOS);
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
                let mut m = self.parts.get(&i).map_or([0; 16], |&k| *self.masks.get(k));
                let cell = Rect::new(x * 16, y * 16, 16, 16);
                for p in parts.iter().filter_map(|p| p.intersect(cell)) {
                    let row = (((1u32 << p.w) - 1) << (p.x - cell.x)) as u16;
                    for sy in p.y..p.bottom() {
                        m[(sy - cell.y) as usize] |= row;
                    }
                }
                let k = self.masks.id(m);
                self.parts.insert(i, k);
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

    /// How many units stand on the deck over the cell (MAP.md §2.5): kept apart from the ground's
    /// count, under the cell's index with [`CellIx::DECK_BIT`], and never in the flags byte, so the
    /// ground under a deck is not held by anyone on it.
    pub fn deck_occupants(&self, x: i32, y: i32) -> u16 {
        if !self.inside(x, y) || self.spans.is_empty() {
            return 0;
        }
        self.occ.get(&CellIx(self.ix(x, y).0 | CellIx::DECK_BIT)).copied().unwrap_or(0)
    }

    /// One more unit stands on the cell, on its deck if `deck`.
    pub fn occupy_on(&mut self, x: i32, y: i32, deck: bool) {
        if !deck {
            return self.occupy(x, y);
        }
        if self.inside(x, y) {
            let i = CellIx(self.ix(x, y).0 | CellIx::DECK_BIT);
            match self.occ.get_mut(&i) {
                Some(n) => *n += 1,
                None => {
                    self.occ.insert(i, 1);
                }
            }
        }
    }

    /// One fewer, on its deck if `deck`.
    pub fn vacate_on(&mut self, x: i32, y: i32, deck: bool) {
        if !deck {
            return self.vacate(x, y);
        }
        if !self.inside(x, y) {
            return;
        }
        let i = CellIx(self.ix(x, y).0 | CellIx::DECK_BIT);
        let Some(n) = self.occ.get_mut(&i) else { return };
        *n -= 1;
        if *n == 0 {
            self.occ.remove(&i);
        }
    }

    /// [`free`](Self::free) on a layer: on a deck, nobody else on the deck over the cell.
    pub fn free_on(&self, x: i32, y: i32, deck: bool, own: Option<(i32, i32)>) -> bool {
        if !deck {
            return self.free(x, y, own);
        }
        let n = self.deck_occupants(x, y);
        n == 0 || (n == 1 && own == Some((x, y)))
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
        g.flags = Flags::over(&g.base, w, h, 0);
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
