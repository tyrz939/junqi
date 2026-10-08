//! A live zone's derived data (ARCHITECTURE.md §3.3, `sim/runtime.ts`): rebuilt from the
//! blueprint and the zone's state on entry and on load, dropped when nobody is in the zone.
//! Throwing it away and rebuilding it never changes a tick (§8 `runtime_rebuild_is_invisible`).
//!
//! **Presence.** A unit is *present* when it is awake, alive and not hidden. A present unit is
//! *in*: it occupies its cell and is listed in `unit_blocks`. [`ZoneRuntime::enter`] and
//! [`ZoneRuntime::leave`] are the only ways in and out, and the runtime remembers where it put
//! each unit, so `leave` is idempotent and needs nothing of the unit but its id. Call `leave`
//! before clearing one of the three bits and `enter` after setting them; a move through
//! `units::move_unit` or `units::place_unit` keeps the rest.

use alloc::vec;
use alloc::vec::Vec;

use jane_core::blueprint::{Mark, Trigger};
use jane_core::num::CELL_SHIFT;
use jane_core::{Blueprint, CellIx, Lookup, Rect, Sym, TriggerId, Vec2, ZoneId};

use crate::grid::ZoneGrid;
use crate::ids::{PropIx, UnitId};
use crate::state::{Unit, ZoneState};
use crate::sym::{SymTable, of_key};
use crate::tuning::{BLOCK_CELLS, FOG_CELLS_IN, FOG_CELLS_OUT};

/// Where a row of the merged trigger table came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerSource {
    Catalog(TriggerId),
    /// Index into `Blueprint.triggers`.
    Blueprint(u16),
}

/// A row of a zone's trigger table: the catalog's rows for the zone in id order, then the
/// blueprint's in its order, a blueprint row whose name a catalog row already has left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneTrigger {
    pub source: TriggerSource,
    pub rect: Sym,
    pub trigger: Trigger,
}

/// The merged table, in the order `TriggerBits` indexes (`runtime.ts zoneTriggers`).
pub fn zone_triggers(bp: &Blueprint, locals: &[Sym]) -> Vec<ZoneTrigger> {
    let cat = jane_data::catalog();
    let mut out: Vec<ZoneTrigger> = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    for (id, def) in cat.story.triggers_in(bp.zone) {
        ids.push(def.id);
        out.push(ZoneTrigger {
            source: TriggerSource::Catalog(id),
            rect: of_key(def.trigger.rect, locals),
            trigger: def.trigger,
        });
    }
    for (i, (key, t)) in bp.triggers.iter().enumerate() {
        let name = match *key {
            jane_core::Key::Name(n) => cat.name(n),
            jane_core::Key::Local(l) => &bp.local_names[l as usize],
        };
        if ids.contains(&name) {
            continue;
        }
        out.push(ZoneTrigger { source: TriggerSource::Blueprint(i as u16), rect: of_key(t.rect, locals), trigger: *t });
    }
    out
}

/// A blueprint's local names as syms, interning any not seen yet. Called once when the zone's
/// state is made, so the syms are in the save; a rebuild finds them all already there.
pub fn intern_locals(syms: &mut SymTable, bp: &Blueprint) -> Vec<Sym> {
    syms.intern_all(&bp.local_names)
}

/// A rebuild's view of the same: every local is already interned.
pub fn find_locals(syms: &SymTable, bp: &Blueprint) -> Vec<Sym> {
    bp.local_names
        .iter()
        .map(|n| syms.find(n).unwrap_or_else(|| panic!("zone {:?}: local name {n:?} was never interned", bp.zone)))
        .collect()
}

/// Blocks of [`BLOCK_CELLS`] a side over a `w x h` grid.
#[derive(Clone, Copy, Debug)]
pub struct Blocks {
    pub w: u32,
    pub h: u32,
}

impl Blocks {
    pub fn over(w: u32, h: u32) -> Self {
        let b = BLOCK_CELLS as u32;
        Self { w: w.div_ceil(b).max(1), h: h.div_ceil(b).max(1) }
    }

    /// The block of a cell, clamped to the grid's blocks (a thing off the edge is in the edge block).
    pub fn of_cell(self, cx: i32, cy: i32) -> u32 {
        let bx = cx.div_euclid(BLOCK_CELLS).clamp(0, self.w as i32 - 1) as u32;
        let by = cy.div_euclid(BLOCK_CELLS).clamp(0, self.h as i32 - 1) as u32;
        by * self.w + bx
    }

    pub fn of_pos(self, p: Vec2) -> u32 {
        self.of_cell(p.x.cell(), p.y.cell())
    }

    /// Inclusive block range over an inclusive cell rect, clamped.
    pub fn range(self, cx0: i32, cy0: i32, cx1: i32, cy1: i32) -> (u32, u32, u32, u32) {
        let c = |v: i32, n: u32| v.div_euclid(BLOCK_CELLS).clamp(0, n as i32 - 1) as u32;
        (c(cx0, self.w), c(cy0, self.h), c(cx1, self.w), c(cy1, self.h))
    }
}

/// Props by the block of their origin cell, each bucket in ascending index (= id). A query
/// reaches back by the largest footprint in the catalog, so a prop is found from any cell it
/// covers and nothing is listed twice (`runtime.ts`, "props by block").
///
/// Held as one list, block by block (PORT.md §13.3, phase 3): `starts[b]..starts[b + 1]` of
/// `ixs` is block `b`'s bucket. The county's 15 625 blocks were a `Vec` each; a prop moving
/// block (a push) shifts the list, which is rare beside the queries.
#[derive(Clone, Debug)]
pub struct PropBuckets {
    pub blocks: Blocks,
    starts: Vec<u32>,
    ixs: Vec<PropIx>,
    reach_w: i32,
    reach_h: i32,
}

impl PropBuckets {
    fn new(blocks: Blocks) -> Self {
        let cat = jane_data::catalog();
        let reach_w = cat.story.props.iter().map(|p| i32::from(p.w) - 1).max().unwrap_or(0);
        let reach_h = cat.story.props.iter().map(|p| i32::from(p.h) - 1).max().unwrap_or(0);
        Self { blocks, starts: vec![0; (blocks.w * blocks.h) as usize + 1], ixs: Vec::new(), reach_w, reach_h }
    }

    /// Block `b`'s bucket, ascending.
    #[inline]
    fn bucket(&self, b: usize) -> &[PropIx] {
        &self.ixs[self.starts[b] as usize..self.starts[b + 1] as usize]
    }

    /// Every prop into its block at once, `(block, ix)` in any order: what a zone's runtime is
    /// built with (one pass, where inserting one by one shifted the list each time).
    fn fill(&mut self, mut all: Vec<(u32, PropIx)>) {
        all.sort_unstable();
        self.starts.iter_mut().for_each(|s| *s = 0);
        for &(b, _) in &all {
            self.starts[b as usize + 1] += 1;
        }
        for b in 1..self.starts.len() {
            self.starts[b] += self.starts[b - 1];
        }
        self.ixs = all.into_iter().map(|(_, ix)| ix).collect();
    }

    pub fn insert(&mut self, block: u32, ix: PropIx) {
        let b = block as usize;
        let at = self.starts[b] as usize + self.bucket(b).partition_point(|&x| x < ix);
        self.ixs.insert(at, ix);
        self.starts[b + 1..].iter_mut().for_each(|s| *s += 1);
    }

    pub fn remove(&mut self, block: u32, ix: PropIx) {
        let b = block as usize;
        if let Ok(k) = self.bucket(b).binary_search(&ix) {
            self.ixs.remove(self.starts[b] as usize + k);
            self.starts[b + 1..].iter_mut().for_each(|s| *s -= 1);
        }
    }

    /// Every prop whose footprint may touch the inclusive cell rect, in ascending index, into
    /// `out` (emptied first). A superset: callers test exactly.
    pub fn query(&self, cx0: i32, cy0: i32, cx1: i32, cy1: i32, out: &mut Vec<PropIx>) {
        out.clear();
        let (bx0, by0, bx1, by1) = self.blocks.range(cx0 - self.reach_w, cy0 - self.reach_h, cx1, cy1);
        let mut buckets = 0;
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let b = self.bucket((by * self.blocks.w + bx) as usize);
                if !b.is_empty() {
                    out.extend_from_slice(b);
                    buckets += 1;
                }
            }
        }
        // Each bucket is sorted; two or more together are not. Indices are unique.
        if buckets > 1 {
            out.sort();
        }
    }

    /// Does `f` hold for any prop whose footprint may touch the inclusive cell rect? Visits in
    /// bucket order, stops at the first yes, needs no buffer (the light question, `light.rs`).
    pub fn any_in(&self, cx0: i32, cy0: i32, cx1: i32, cy1: i32, mut f: impl FnMut(PropIx) -> bool) -> bool {
        let (bx0, by0, bx1, by1) = self.blocks.range(cx0 - self.reach_w, cy0 - self.reach_h, cx1, cy1);
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                for &ix in self.bucket((by * self.blocks.w + bx) as usize) {
                    if f(ix) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Present units by block, as one sorted list of `(block, id)`: a query is a binary search per
/// block row, a move is a remove and an insert, and nothing allocates once the list has grown
/// to the most units ever present at once.
#[derive(Clone, Debug)]
pub struct UnitBlocks {
    pub blocks: Blocks,
    entries: Vec<(u32, UnitId)>,
}

impl UnitBlocks {
    fn new(blocks: Blocks) -> Self {
        Self { blocks, entries: Vec::with_capacity(256) }
    }

    pub fn insert(&mut self, block: u32, id: UnitId) {
        let at = self.entries.partition_point(|&e| e < (block, id));
        self.entries.insert(at, (block, id));
    }

    pub fn remove(&mut self, block: u32, id: UnitId) {
        if let Ok(at) = self.entries.binary_search(&(block, id)) {
            self.entries.remove(at);
        }
    }

    pub fn len(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Present units in the blocks over an inclusive cell rect, block by block and by id within
    /// a block, into `out` (emptied first). A superset: callers test exactly.
    pub fn query(&self, cx0: i32, cy0: i32, cx1: i32, cy1: i32, out: &mut Vec<UnitId>) {
        out.clear();
        let (bx0, by0, bx1, by1) = self.blocks.range(cx0, cy0, cx1, cy1);
        for by in by0..=by1 {
            let lo = by * self.blocks.w + bx0;
            let hi = by * self.blocks.w + bx1;
            let start = self.entries.partition_point(|&(b, _)| b < lo);
            for &(b, id) in &self.entries[start..] {
                if b > hi {
                    break;
                }
                out.push(id);
            }
        }
    }
}

/// The fog's geometry: cells per bit, and bits across and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FogGeom {
    pub cells: u32,
    pub w: u32,
    pub h: u32,
}

impl FogGeom {
    pub fn of(bp: &Blueprint) -> Self {
        let cells = if bp.indoor { FOG_CELLS_IN } else { FOG_CELLS_OUT };
        Self { cells, w: bp.w().div_ceil(cells), h: bp.h().div_ceil(cells) }
    }

    pub fn words(self) -> u32 {
        (self.w * self.h).div_ceil(32)
    }
}

#[derive(Debug)]
pub struct ZoneRuntime {
    pub zone: ZoneId,
    pub grid: ZoneGrid,
    /// `Key::Local(i)` of this zone's blueprint is `locals[i]`.
    pub locals: Vec<Sym>,
    pub marks: Lookup<Sym, Mark>,
    pub rects: Lookup<Sym, Rect>,
    /// Props by key.
    pub names: Lookup<Sym, PropIx>,
    /// Units by key.
    pub unit_names: Lookup<Sym, UnitId>,
    pub props: PropBuckets,
    /// One bit per prop: inside some seat's ring. The ring clears the last set and sets the new.
    pub awake_props: Vec<bool>,
    pub awake_prop_list: Vec<PropIx>,
    /// Pressure plates, in id order.
    pub plates: Vec<PropIx>,
    /// Every unit whose `awake` bit is set, ascending: the controllers' snapshot (step 7).
    pub awake_units: Vec<UnitId>,
    pub unit_blocks: UnitBlocks,
    /// Units in (occupying and listed), and the position they were registered at.
    entered: Lookup<UnitId, Vec2>,
    pub triggers: Vec<ZoneTrigger>,
    /// Everything changed: re-stamp every solid prop at housekeeping.
    pub props_dirty_all: bool,
    /// Something small changed: cell rects to re-stamp at housekeeping.
    pub props_dirty: Vec<Rect>,
    /// Where each awake unit stood when the players step began, ascending id (`View`'s
    /// `prev_pos`, `moved`).
    pub prev_pos: Vec<(UnitId, Vec2)>,
    pub paths_this_tick: u8,
    pub fog: FogGeom,
    /// The blueprint's region map (the county's; empty elsewhere): whose sky and ramp a cell is
    /// under (`region_at`).
    pub regions: jane_core::RegionMap,
    /// The soonest `Prop::regrow` of the zone's props (`regrow.rs`), so housekeeping looks at
    /// the props only when some food is due back.
    pub regrow_next: Option<jane_core::Tick>,
    /// The blueprint's sanctuary (a generated dungeon's rest rooms and thresholds out): where
    /// nothing follows her (`ai::fight`, PLAN.md §2.6 *Leash*).
    pub sanctuary: Vec<Rect>,
}

impl ZoneRuntime {
    /// The region cell `(x, y)` lies in (§4.6.b): whose sky rains on it, whose ramp wets it.
    pub fn region_at(&self, x: i32, y: i32) -> jane_data::Region {
        crate::living::region_in(&self.regions, self.zone, x, y)
    }

    /// Is cell `(x, y)` sanctuary: a dungeon's rest room or the threshold of a way out?
    pub fn in_sanctuary(&self, (x, y): (i32, i32)) -> bool {
        self.sanctuary.iter().any(|r| r.contains(x, y))
    }

    /// Build from the blueprint and the zone's state. `locals` are the blueprint's local names
    /// as syms (all interned when the zone's state was made).
    pub fn build(bp: &alloc::sync::Arc<Blueprint>, zone: &ZoneState, locals: Vec<Sym>) -> Self {
        let cat = jane_data::catalog();
        // The blueprint's tiles, shared, under the zone's changed ones (PLAY-PLAN.md §7).
        let grid = ZoneGrid::over(bp, zone.tile_deltas.iter().map(|(&i, &t)| (i, t)));
        let blocks = Blocks::over(bp.w(), bp.h());
        let mut marks = Lookup::with_capacity(bp.marks.len());
        for (&k, &m) in &bp.marks {
            marks.insert(of_key(k, &locals), m);
        }
        let mut rects = Lookup::with_capacity(bp.rects.len());
        for (&k, &r) in &bp.rects {
            rects.insert(of_key(k, &locals), r);
        }
        let mut rt = Self {
            zone: bp.zone,
            grid,
            marks,
            rects,
            names: Lookup::with_capacity(zone.props.len()),
            unit_names: Lookup::with_capacity(zone.units.len()),
            props: PropBuckets::new(blocks),
            awake_props: vec![false; zone.props.len()],
            awake_prop_list: Vec::new(),
            plates: Vec::new(),
            awake_units: Vec::new(),
            unit_blocks: UnitBlocks::new(blocks),
            entered: Lookup::with_capacity(256),
            triggers: zone_triggers(bp, &locals),
            locals,
            props_dirty_all: false,
            props_dirty: Vec::new(),
            prev_pos: Vec::new(),
            paths_this_tick: 0,
            fog: FogGeom::of(bp),
            regions: bp.regions.clone(),
            regrow_next: crate::regrow::soonest(zone),
            sanctuary: bp.sanctuary.clone(),
        };
        let mut placed = Vec::with_capacity(zone.props.len());
        for (i, p) in zone.props.iter().enumerate() {
            rt.names.insert(p.key, i as PropIx);
            placed.push((blocks.of_cell(i32::from(p.cell.x), i32::from(p.cell.y)), i as PropIx));
            if cat.story.prop(p.def).plate {
                rt.plates.push(i as PropIx);
            }
        }
        rt.props.fill(placed);
        rt.restamp_all(zone);
        for u in &zone.units {
            if let Some(k) = u.key {
                rt.unit_names.insert(k, u.id);
            }
            if u.awake {
                rt.awake_units.push(u.id);
            }
            rt.enter(u);
        }
        rt
    }

    fn index_prop(&mut self, ix: PropIx, p: &crate::state::Prop) {
        self.names.insert(p.key, ix);
        let b = self.props.blocks.of_cell(i32::from(p.cell.x), i32::from(p.cell.y));
        self.props.insert(b, ix);
    }

    /// A prop appended to the zone at runtime (its index is the new last).
    pub fn add_prop(&mut self, zone: &ZoneState, ix: PropIx) {
        let p = &zone.props[ix as usize];
        self.index_prop(ix, p);
        self.awake_props.push(false);
        if jane_data::catalog().story.prop(p.def).plate {
            self.plates.push(ix);
        }
        self.touch_prop(zone, ix);
    }

    // --- units ---------------------------------------------------------------------------

    pub const fn present(u: &Unit) -> bool {
        u.awake && u.alive && !u.hidden
    }

    /// Occupy the unit's cell and list it by block, if it is present and not already in.
    pub fn enter(&mut self, u: &Unit) {
        if Self::present(u) && !self.entered.contains(&u.id) {
            let (cx, cy) = u.pos.cell();
            self.grid.occupy(cx, cy);
            self.unit_blocks.insert(self.unit_blocks.blocks.of_pos(u.pos), u.id);
            self.entered.insert(u.id, u.pos);
        }
    }

    /// The reverse of [`enter`](Self::enter), from where it entered; a unit not in is a no-op.
    pub fn leave(&mut self, id: UnitId) {
        if let Some(at) = self.entered.remove(&id) {
            let (cx, cy) = at.cell();
            self.grid.vacate(cx, cy);
            self.unit_blocks.remove(self.unit_blocks.blocks.of_pos(at), id);
        }
    }

    /// Is the unit in (occupying, listed by block)?
    pub fn is_in(&self, id: UnitId) -> bool {
        self.entered.contains(&id)
    }

    /// A unit moved to where it now is; if it is in, its cell and block follow.
    pub fn moved(&mut self, u: &Unit) {
        let Some(from) = self.entered.get_mut(&u.id) else { return };
        let from = core::mem::replace(from, u.pos);
        let (ox, oy) = from.cell();
        let (nx, ny) = u.pos.cell();
        if (ox, oy) != (nx, ny) {
            self.grid.vacate(ox, oy);
            self.grid.occupy(nx, ny);
        }
        let (bo, bn) = (self.unit_blocks.blocks.of_pos(from), self.unit_blocks.blocks.of_pos(u.pos));
        if bo != bn {
            self.unit_blocks.remove(bo, u.id);
            self.unit_blocks.insert(bn, u.id);
        }
    }

    /// Set a unit's `awake` bit, keeping presence and the awake list true.
    pub fn set_awake(&mut self, u: &mut Unit, awake: bool) {
        if u.awake == awake {
            return;
        }
        self.leave(u.id);
        u.awake = awake;
        match self.awake_units.binary_search(&u.id) {
            Ok(at) if !awake => {
                self.awake_units.remove(at);
            }
            Err(at) if awake => self.awake_units.insert(at, u.id),
            _ => {}
        }
        self.enter(u);
    }

    /// A unit joins the zone (spawned, arrived, sat down). It must already be in `zone.units`.
    pub fn add_unit(&mut self, u: &Unit) {
        if let Some(k) = u.key {
            self.unit_names.insert(k, u.id);
        }
        if u.awake {
            if let Err(at) = self.awake_units.binary_search(&u.id) {
                self.awake_units.insert(at, u.id);
            }
        }
        self.enter(u);
    }

    /// A unit leaves the zone. `forget_unit` (ctx.rs) clears everyone's hold on it first.
    pub fn remove_unit(&mut self, u: &Unit) {
        self.leave(u.id);
        if let Some(k) = u.key {
            if self.unit_names.get(&k) == Some(&u.id) {
                self.unit_names.remove(&k);
            }
        }
        if let Ok(at) = self.awake_units.binary_search(&u.id) {
            self.awake_units.remove(at);
        }
    }

    // --- prop flags ----------------------------------------------------------------------

    fn footprint(p: &crate::state::Prop) -> Rect {
        let def = jane_data::catalog().story.prop(p.def);
        Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(def.w), i32::from(def.h))
    }

    /// `solid` or `hidden` changed on this prop: its footprint is re-stamped at housekeeping.
    pub fn touch_prop(&mut self, zone: &ZoneState, ix: PropIx) {
        self.props_dirty.push(Self::footprint(&zone.props[ix as usize]));
    }

    /// The only way a prop changes cell (push, pull, put down): re-bucketed, and both its
    /// footprints re-stamped at housekeeping (`runtime.ts moveProp`).
    pub fn move_prop(&mut self, zone: &mut ZoneState, ix: PropIx, to: jane_core::Cell) {
        self.touch_prop(zone, ix);
        let p = &mut zone.props[ix as usize];
        let from = self.props.blocks.of_cell(i32::from(p.cell.x), i32::from(p.cell.y));
        let into = self.props.blocks.of_cell(i32::from(to.x), i32::from(to.y));
        p.cell = to;
        if from != into {
            self.props.remove(from, ix);
            self.props.insert(into, ix);
        }
        self.touch_prop(zone, ix);
    }

    /// Re-stamp the prop flags over one rect of cells from the props as they stand
    /// (`runtime.ts restampCells`).
    pub fn restamp_cells(&mut self, zone: &ZoneState, r: Rect, scratch: &mut Vec<PropIx>) {
        self.grid.clear_prop_flags_in(r);
        self.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, scratch);
        for &ix in scratch.iter() {
            let p = &zone.props[ix as usize];
            if p.solid && !p.hidden {
                stamp(&mut self.grid, p);
            }
        }
    }

    /// Re-stamp every solid prop: zone entry, load, and whoever set `props_dirty_all`.
    pub fn restamp_all(&mut self, zone: &ZoneState) {
        self.grid.clear_prop_flags();
        for p in &zone.props {
            if p.solid && !p.hidden {
                stamp(&mut self.grid, p);
            }
        }
        self.props_dirty_all = false;
        self.props_dirty.clear();
    }

    /// Housekeeping: bring the prop flags up to date with what changed since the last call.
    /// `scratch` is a reused buffer.
    pub fn flush_prop_flags(&mut self, zone: &ZoneState, scratch: &mut Vec<PropIx>) {
        if self.props_dirty_all {
            self.restamp_all(zone);
            return;
        }
        let mut i = 0;
        while i < self.props_dirty.len() {
            let r = self.props_dirty[i];
            i += 1;
            self.grid.clear_prop_flags_in(r);
            self.props.query(r.x, r.y, r.right() - 1, r.bottom() - 1, scratch);
            for &ix in scratch.iter() {
                let p = &zone.props[ix as usize];
                if p.solid && !p.hidden {
                    stamp(&mut self.grid, p);
                }
            }
        }
        self.props_dirty.clear();
    }

    /// May this zone run another path search this tick? Counts it if so (four per tick per
    /// zone, `path::PATHS_PER_TICK`; a walker refused keeps its old path and asks next tick).
    pub fn take_path_search(&mut self) -> bool {
        if self.paths_this_tick >= crate::path::PATHS_PER_TICK {
            return false;
        }
        self.paths_this_tick += 1;
        true
    }

    /// The cell of a blueprint mark by sym.
    pub fn mark(&self, s: Sym) -> Option<Mark> {
        self.marks.get(&s).copied()
    }
}

/// Stamp a solid prop: its `base` rows (`PropDef::solid_rect`) for paths, sight and the solver,
/// and where its look meets the ground (`PropDef::solid_parts`) for feet.
fn stamp(grid: &mut ZoneGrid, p: &crate::state::Prop) {
    let def = jane_data::catalog().story.prop(p.def);
    let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
    let cells = def.solid_rect(x, y);
    if def.feet.is_none() {
        grid.stamp_prop(cells, def.block_los);
        return;
    }
    let parts = def.solid_parts().map(|r| Rect::new(r.x + x * 16, r.y + y * 16, r.w, r.h));
    grid.stamp_prop_parts(cells, def.block_los, &parts);
}

/// The cell of a position, as a flat index into a `w`-wide grid.
pub const fn cell_ix(p: Vec2, w: u32) -> CellIx {
    CellIx(((p.y.0 >> CELL_SHIFT) as u32) * w + (p.x.0 >> CELL_SHIFT) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_blocks_query_by_rows_in_block_then_id_order() {
        let mut b = UnitBlocks::new(Blocks::over(64, 64));
        let id = |n| UnitId::new(n).unwrap();
        b.insert(0, id(5));
        b.insert(0, id(2));
        b.insert(1, id(1));
        b.insert(4, id(9));
        let mut out = Vec::new();
        b.query(0, 0, 31, 15, &mut out);
        assert_eq!(out, [id(2), id(5), id(1)]);
        b.query(0, 16, 63, 31, &mut out);
        assert_eq!(out, [id(9)]);
        b.remove(0, id(2));
        b.query(0, 0, 0, 0, &mut out);
        assert_eq!(out, [id(5)]);
    }

    #[test]
    fn blocks_clamp_to_the_edge() {
        let b = Blocks::over(40, 20);
        assert_eq!((b.w, b.h), (3, 2));
        assert_eq!(b.of_cell(-5, -5), 0);
        assert_eq!(b.of_cell(99, 99), 5);
        assert_eq!(b.range(-100, 0, 1000, 5), (0, 0, 2, 0));
    }
}
