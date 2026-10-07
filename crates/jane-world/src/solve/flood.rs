//! The flood: every state she can bring about, each from where she stood when she did.
//!
//! One layer of "seen" per combination of the state flags; without states it is the one layer,
//! so the county pays nothing for the idea. Each layer floods through `jane_core::search::flood`
//! (PORT.md §6.e: one flood), four ways, stopped by solid terrain and by what the props of that
//! layer block; which cells, not how far, so through the flood's fast path that keeps no
//! distances (`jane_core::search::fill`). A flood is the whole zone, and the county is four million cells: `run.rs` floods
//! again only when something that blocks FEET has changed.

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use jane_core::Tile;
use jane_core::grid::Grid;
use jane_core::search::{Fill, fill_words};
use jane_core::tile::F_SOLID;

use super::model::{MAX_LAYERS, Solve};
use super::passes::states;
use crate::bits::Bits;

/// The layers' "seen" and "blocked" grids, and the scratch the core flood reuses.
#[derive(Debug)]
pub(crate) struct Layers {
    w: u32,
    h: u32,
    /// Layers: `1 << states`.
    pub count: usize,
    pub seen: Vec<Bits>,
    pub blocked: Vec<Bits>,
    /// Reached in any layer; empty with one layer, which IS the any-layer.
    any: Bits,
    /// Which layers this pass's flood reached.
    pub reached: [bool; MAX_LAYERS],
    /// The pass that first reached each cell, or -1: only when traced.
    pub first_seen: Option<Vec<i16>>,
    reach: Fill,
    /// Solid terrain, a bit a cell (`y * w + x`): the zone's tiles never change in a solve.
    solid: Vec<u64>,
    /// What `blocked` of the layer last restamped was before it (the restamp's scratch).
    before: Bits,
    /// The props that last restamp stamped, by index, and those the one before it stamped.
    stamped: Vec<usize>,
    stamped_before: Vec<usize>,
    /// With one layer: the seeds of the last pass's flood, and whether it reached anything.
    last: Option<(Vec<(i32, i32)>, bool)>,
    pub floods: u32,
    pub visited: u64,
}

/// A copy without the scratch ([`Layers::keep`]).
impl Clone for Layers {
    fn clone(&self) -> Self {
        self.keep()
    }
}

impl Layers {
    pub fn new(w: u32, h: u32, count: usize, trace: bool) -> Self {
        let n = (w as usize) * (h as usize);
        Layers {
            w,
            h,
            count,
            seen: vec![Bits::new(n, false); count],
            blocked: vec![Bits::new(n, false); count],
            any: if count > 1 { Bits::new(n, false) } else { Bits::empty() },
            reached: [false; MAX_LAYERS],
            first_seen: trace.then(|| vec![-1; n]),
            reach: Fill::new(),
            solid: Vec::new(),
            before: Bits::empty(),
            stamped: Vec::new(),
            stamped_before: Vec::new(),
            last: None,
            floods: 0,
            visited: 0,
        }
    }

    /// A copy for the trail, without the scratch.
    pub fn keep(&self) -> Self {
        Layers {
            w: self.w,
            h: self.h,
            count: self.count,
            seen: self.seen.clone(),
            blocked: self.blocked.clone(),
            any: self.any.clone(),
            reached: self.reached,
            first_seen: self.first_seen.clone(),
            reach: Fill::new(),
            solid: self.solid.clone(),
            before: Bits::empty(),
            stamped: self.stamped.clone(),
            stamped_before: self.stamped_before.clone(),
            last: self.last.clone(),
            floods: self.floods,
            visited: self.visited,
        }
    }

    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h
    }

    pub fn ix(&self, x: i32, y: i32) -> usize {
        y as usize * self.w as usize + x as usize
    }

    /// Reached in any layer. Outside the zone is never reached.
    pub fn seen_any(&self, x: i32, y: i32) -> bool {
        if !self.inside(x, y) {
            return false;
        }
        let i = self.ix(x, y);
        if self.count == 1 { self.seen[0][i] } else { self.any[i] }
    }

    /// Is any cell of the inclusive box `(x0, y0, x1, y1)` set in `g`? Outside the zone is not.
    fn any_in(&self, g: &Bits, (x0, y0, x1, y1): (i32, i32, i32, i32)) -> bool {
        let (x0, x1) = (x0.max(0), x1.min(self.w as i32 - 1));
        if x0 > x1 {
            return false;
        }
        (y0.max(0)..=y1.min(self.h as i32 - 1)).any(|y| (self.ix(x0, y)..=self.ix(x1, y)).any(|i| g[i]))
    }

    /// Is any cell of the inclusive box `(x0, y0, x1, y1)` reached in layer `s`?
    pub fn touches(&self, s: usize, b: (i32, i32, i32, i32)) -> bool {
        self.any_in(&self.seen[s], b)
    }

    /// Is any cell of the inclusive box reached in any layer?
    pub fn touches_any(&self, b: (i32, i32, i32, i32)) -> bool {
        self.any_in(if self.count == 1 { &self.seen[0] } else { &self.any }, b)
    }

    pub fn count_any(&self) -> u32 {
        let g = if self.count == 1 { &self.seen[0] } else { &self.any };
        g.count_ones()
    }

    pub fn clear_any(&mut self) {
        self.any.clear_all();
    }

    /// Flood layer `s` on from `seeds`, over what it has reached already. True when it reached
    /// anything new.
    pub fn flood(&mut self, tiles: &Grid<Tile>, s: usize, seeds: &[(i32, i32)], pass: u16) -> bool {
        let (w, h) = (self.w, self.h);
        let n = w as usize * h as usize;
        if self.solid.is_empty() {
            let terrain = tiles.as_slice();
            self.solid = (0..n.div_ceil(64))
                .map(|k| {
                    let cells = &terrain[k << 6..((k + 1) << 6).min(n)];
                    cells.iter().enumerate().fold(0, |m, (j, t)| m | u64::from(t.flags() & F_SOLID != 0) << j)
                })
                .collect();
        }
        let (seen, blocked, solid) = (&self.seen[s], &self.blocked[s], &self.solid);
        let open = |i: usize| !seen[i] && !blocked[i] && solid[i >> 6] >> (i & 63) & 1 == 0;
        let starts: Vec<(i32, i32)> =
            seeds.iter().copied().filter(|&(x, y)| self.inside(x, y) && open(self.ix(x, y))).collect();
        if starts.is_empty() {
            return false;
        }
        // Sixty-four cells at a time: neither reached nor blocked nor solid (bits past the end clear).
        let word = |k: usize| {
            let tail = n - (k << 6);
            let valid = if tail >= 64 { u64::MAX } else { (1u64 << tail) - 1 };
            !(seen.word(k) | blocked.word(k) | solid[k]) & valid
        };
        fill_words(w, h, &starts, word, &mut self.reach);
        self.floods += 1;
        self.visited += u64::from(self.reach.count());
        let (seen, any, first) = (&mut self.seen[s], &mut self.any, &mut self.first_seen);
        for r in self.reach.runs() {
            let cells = r.cells(w);
            seen.fill(cells.clone(), true);
            if !any.is_empty() {
                any.fill(cells.clone(), true);
            }
            if let Some(f) = first.as_mut() {
                for c in &mut f[cells] {
                    if *c < 0 {
                        *c = pass as i16;
                    }
                }
            }
        }
        self.reached[s] = true;
        true
    }
}

impl Solve<'_> {
    /// Stamp what blocks feet in layer `s`: a gate that is locked or kept shut there, and any
    /// solid prop that cannot be pushed or carried, unless it is hidden there.
    pub(crate) fn restamp(&mut self, s: usize) {
        let mut blocked = core::mem::take(&mut self.layers.before);
        blocked.reset(self.layers.blocked[s].len());
        let mut stamped = core::mem::take(&mut self.layers.stamped_before);
        stamped.clear();
        for (i, p) in self.bp.props.iter().enumerate() {
            let def = self.def(i);
            if self.hidden_in(i, s) {
                continue;
            }
            let shut = if def.gate { self.props[i].shut || self.locked_in(i, s) } else { self.blocks_feet(i) };
            if !shut {
                continue;
            }
            stamped.push(i);
            // The front rows it stands on, as the sim stamps it (`PropDef::solid_rect`).
            for (x, y) in def.solid_rect(i32::from(p.cell.x), i32::from(p.cell.y)).cells() {
                if self.layers.inside(x, y) {
                    blocked.set(self.layers.ix(x, y), true);
                }
            }
        }
        self.layers.before = core::mem::replace(&mut self.layers.blocked[s], blocked);
        self.layers.stamped_before = core::mem::replace(&mut self.layers.stamped, stamped);
    }

    /// The cells prop `i` blocks when stamped, inside the zone, by index.
    fn footprint(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let p = &self.bp.props[i];
        let r = self.def(i).solid_rect(i32::from(p.cell.x), i32::from(p.cell.y));
        r.cells().filter(|&(x, y)| self.layers.inside(x, y)).map(|(x, y)| self.layers.ix(x, y))
    }

    /// One pass's flood: from the entrance and every same-zone door taken so far, in the layer she
    /// took it in; and from every control she can work, into the layer it leads to, starting AT
    /// the control.
    ///
    /// With one layer (no states), a pass after a gate opened usually only unblocked cells: when
    /// every cell blocked now was blocked last pass and every seed of last pass is a seed still,
    /// what this pass reaches is what the last reached and what that opens onto, so the layer
    /// floods on from the cells just unblocked beside it instead of again from the start
    /// ([`Self::flood_on`]). Anything else (a prop shown in the way) floods from the start.
    pub(crate) fn flood_all(&mut self) {
        let count = self.layers.count;
        self.layers.reached = [false; MAX_LAYERS];
        if count == 1 && self.flood_on() {
            self.reached_cells = self.layers.count_any();
            return;
        }
        for s in 0..count {
            if self.states.governed[s].is_some() {
                self.restamp(s);
                self.layers.seen[s].clear_all();
            }
        }
        self.layers.clear_any();
        let mut seeds: Vec<Vec<(i32, i32)>> = vec![Vec::new(); count];
        seeds[0].push(self.entry);
        for &(mark, s) in &self.hops {
            if let Some(m) = self.bp.marks.get(&mark) {
                seeds[s].push((i32::from(m.cell.x), i32::from(m.cell.y)));
            }
        }
        let first = (count == 1).then(|| seeds[0].clone());
        let mut dirty: VecDeque<usize> = (0..count).filter(|&s| !seeds[s].is_empty()).collect();
        while let Some(s) = dirty.pop_front() {
            let from = core::mem::take(&mut seeds[s]);
            self.layers.flood(&self.bp.tiles, s, &from, self.pass);
            states::edges(self, s, &mut seeds, &mut dirty);
        }
        self.layers.last = first.map(|f| (f, self.layers.reached[0]));
        self.reached_cells = self.layers.count_any();
    }

    /// One layer's pass flooded on from the last, if it can be: true when done. The last pass
    /// reached every cell its seeds joined over ground open then; with the seeds kept and nothing
    /// newly blocked, every cell this pass reaches beyond those is joined to them through a cell
    /// that has just been unblocked, beside one reached, so flooding from those (and from any new
    /// seed) reaches exactly what a flood from the start would.
    fn flood_on(&mut self) -> bool {
        let Some((last, reached)) = self.layers.last.take() else { return false };
        let mut seeds = vec![self.entry];
        seeds.extend(
            self.hops
                .iter()
                .filter_map(|&(mark, _)| self.bp.marks.get(&mark))
                .map(|m| (i32::from(m.cell.x), i32::from(m.cell.y))),
        );
        if !last.iter().all(|c| seeds.contains(c)) {
            return false;
        }
        self.restamp(0);
        // A cell blocked now and not before is under a prop stamped now and not before; one
        // unblocked now, under a prop stamped before and not now.
        let l = &self.layers;
        let (now, was) = (&l.stamped, &l.stamped_before);
        let newly = now.iter().filter(|i| was.binary_search(i).is_err());
        let gone: Vec<usize> = was.iter().copied().filter(|i| now.binary_search(i).is_err()).collect();
        if newly.flat_map(|&i| self.footprint(i)).any(|c| !l.before[c]) {
            // Something stands in the way that did not: from the start, as `flood_all` does.
            self.layers.seen[0].clear_all();
            self.layers.last = None;
            let from = seeds.clone();
            self.layers.flood(&self.bp.tiles, 0, &from, self.pass);
            self.layers.last = Some((seeds, self.layers.reached[0]));
            return true;
        }
        let (w, h) = (l.w as i32, l.h as i32);
        let seen = &l.seen[0];
        let mut from = seeds.clone();
        for i in gone.into_iter().flat_map(|p| self.footprint(p)) {
            if l.blocked[0][i] || seen[i] {
                continue;
            }
            let (x, y) = (i as i32 % w, i as i32 / w);
            let beside = (x > 0 && seen[i - 1])
                || (x + 1 < w && seen[i + 1])
                || (y > 0 && seen[i - w as usize])
                || (y + 1 < h && seen[i + w as usize]);
            if beside {
                from.push((x, y));
            }
        }
        let grew = self.layers.flood(&self.bp.tiles, 0, &from, self.pass);
        self.layers.reached[0] = reached || grew;
        self.layers.last = Some((seeds, self.layers.reached[0]));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&str]) -> Grid<Tile> {
        let w = rows[0].len() as u32;
        let cells =
            rows.iter().flat_map(|r| r.chars()).map(|c| if c == '#' { Tile::CaveWall } else { Tile::CaveFloor });
        Grid::from_vec(w, rows.len() as u32, cells.collect())
    }

    #[test]
    fn a_layer_floods_on_from_where_it_stopped_and_walls_and_blocks_stop_it() {
        let tiles = grid(&["..#..", "..#..", "....."]);
        let mut l = Layers::new(5, 3, 1, true);
        let i = l.ix(2, 2);
        l.blocked[0].set(i, true);
        assert!(l.flood(&tiles, 0, &[(0, 0)], 0));
        assert_eq!(l.count_any(), 6);
        assert!(!l.seen_any(3, 0));
        // Seeds already reached, in a wall, or outside do nothing.
        assert!(!l.flood(&tiles, 0, &[(1, 1), (2, 0), (-1, 0)], 1));
        l.blocked[0].set(i, false);
        assert!(l.flood(&tiles, 0, &[(2, 2)], 1));
        assert_eq!(l.count_any(), 13);
        assert_eq!(l.first_seen.as_ref().unwrap()[l.ix(4, 0)], 1);
        assert_eq!(l.first_seen.as_ref().unwrap()[l.ix(0, 0)], 0);
        assert!(l.touches(0, (4, -1, 6, 0)));
        assert!(!l.touches_any((2, 0, 2, 1)));
        assert_eq!(l.floods, 2);
    }

    #[test]
    fn with_states_any_is_the_union() {
        let tiles = grid(&["....."]);
        let mut l = Layers::new(5, 1, 2, false);
        l.blocked[0].set(2, true);
        l.flood(&tiles, 0, &[(0, 0)], 0);
        l.flood(&tiles, 1, &[(4, 0)], 0);
        assert_eq!(l.count_any(), 5);
        assert!(l.reached[0] && l.reached[1]);
    }
}
