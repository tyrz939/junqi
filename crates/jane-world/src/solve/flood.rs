//! The flood: every state she can bring about, each from where she stood when she did.
//!
//! One layer of "seen" per combination of the state flags; without states it is the one layer,
//! so the county pays nothing for the idea. Each layer floods through `jane_core::search::flood`
//! (PORT.md §6.e: one flood), four ways, stopped by solid terrain and by what the props of that
//! layer block. A flood is the whole zone, and the county is four million cells: `run.rs` floods
//! again only when something that blocks FEET has changed.

use std::collections::VecDeque;

use jane_core::Tile;
use jane_core::grid::Grid;
use jane_core::search::{Conn, Reach, flood};
use jane_core::tile::F_SOLID;

use super::model::{MAX_LAYERS, Solve};
use super::passes::states;

/// The layers' "seen" and "blocked" grids, and the scratch the core flood reuses.
#[derive(Debug)]
pub(crate) struct Layers {
    w: u32,
    h: u32,
    /// Layers: `1 << states`.
    pub count: usize,
    pub seen: Vec<Vec<u8>>,
    pub blocked: Vec<Vec<u8>>,
    /// Reached in any layer; empty with one layer, which IS the any-layer.
    any: Vec<u8>,
    /// Which layers this pass's flood reached.
    pub reached: [bool; MAX_LAYERS],
    /// The pass that first reached each cell, or -1: only when traced.
    pub first_seen: Option<Vec<i16>>,
    reach: Reach,
    pub floods: u32,
    pub visited: u64,
}

impl Layers {
    pub fn new(w: u32, h: u32, count: usize, trace: bool) -> Self {
        let n = (w as usize) * (h as usize);
        Layers {
            w,
            h,
            count,
            seen: vec![vec![0; n]; count],
            blocked: vec![vec![0; n]; count],
            any: if count > 1 { vec![0; n] } else { Vec::new() },
            reached: [false; MAX_LAYERS],
            first_seen: trace.then(|| vec![-1; n]),
            reach: Reach::new(),
            floods: 0,
            visited: 0,
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
        if self.count == 1 { self.seen[0][i] != 0 } else { self.any[i] != 0 }
    }

    /// Is any cell of the inclusive box `(x0, y0, x1, y1)` reached in layer `s`?
    pub fn touches(&self, s: usize, (x0, y0, x1, y1): (i32, i32, i32, i32)) -> bool {
        let seen = &self.seen[s];
        (y0.max(0)..=y1.min(self.h as i32 - 1))
            .any(|y| (x0.max(0)..=x1.min(self.w as i32 - 1)).any(|x| seen[self.ix(x, y)] != 0))
    }

    /// Is any cell of the inclusive box reached in any layer?
    pub fn touches_any(&self, (x0, y0, x1, y1): (i32, i32, i32, i32)) -> bool {
        (y0..=y1).any(|y| (x0..=x1).any(|x| self.seen_any(x, y)))
    }

    pub fn count_any(&self) -> u32 {
        let g = if self.count == 1 { &self.seen[0] } else { &self.any };
        g.iter().filter(|&&c| c != 0).count() as u32
    }

    pub fn clear_any(&mut self) {
        self.any.fill(0);
    }

    /// Flood layer `s` on from `seeds`, over what it has reached already. True when it reached
    /// anything new.
    pub fn flood(&mut self, tiles: &Grid<Tile>, s: usize, seeds: &[(i32, i32)], pass: u16) -> bool {
        let (w, h) = (self.w, self.h);
        let terrain = tiles.as_slice();
        let seen = &self.seen[s];
        let blocked = &self.blocked[s];
        let open = |x: i32, y: i32| {
            let i = y as usize * w as usize + x as usize;
            seen[i] == 0 && blocked[i] == 0 && terrain[i].flags() & F_SOLID == 0
        };
        let starts: Vec<(i32, i32)> = seeds.iter().copied().filter(|&(x, y)| self.inside(x, y) && open(x, y)).collect();
        if starts.is_empty() {
            return false;
        }
        flood(w, h, &starts, Conn::Four, u32::MAX, open, &mut self.reach);
        self.floods += 1;
        self.visited += u64::from(self.reach.count());
        let (seen, any, first) = (&mut self.seen[s], &mut self.any, &mut self.first_seen);
        for c in self.reach.order() {
            let i = c.0 as usize;
            seen[i] = 1;
            if !any.is_empty() {
                any[i] = 1;
            }
            if let Some(f) = first.as_mut().filter(|f| f[i] < 0) {
                f[i] = pass as i16;
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
        let mut blocked = std::mem::take(&mut self.layers.blocked[s]);
        blocked.fill(0);
        for (i, p) in self.bp.props.iter().enumerate() {
            let def = self.def(i);
            if self.hidden_in(i, s) {
                continue;
            }
            let shut = if def.gate { self.props[i].shut || self.locked_in(i, s) } else { self.blocks_feet(i) };
            if !shut {
                continue;
            }
            for y in i32::from(p.cell.y)..i32::from(p.cell.y) + i32::from(def.h) {
                for x in i32::from(p.cell.x)..i32::from(p.cell.x) + i32::from(def.w) {
                    if self.layers.inside(x, y) {
                        blocked[self.layers.ix(x, y)] = 1;
                    }
                }
            }
        }
        self.layers.blocked[s] = blocked;
    }

    /// One pass's flood: from the entrance and every same-zone door taken so far, in the layer she
    /// took it in; and from every control she can work, into the layer it leads to, starting AT
    /// the control.
    pub(crate) fn flood_all(&mut self) {
        let count = self.layers.count;
        self.layers.reached = [false; MAX_LAYERS];
        for s in 0..count {
            if self.states.governed[s].is_some() {
                self.restamp(s);
                self.layers.seen[s].fill(0);
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
        let mut dirty: VecDeque<usize> = (0..count).filter(|&s| !seeds[s].is_empty()).collect();
        while let Some(s) = dirty.pop_front() {
            let from = std::mem::take(&mut seeds[s]);
            self.layers.flood(&self.bp.tiles, s, &from, self.pass);
            states::edges(self, s, &mut seeds, &mut dirty);
        }
        self.reached_cells = self.layers.count_any();
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
        l.blocked[0][i] = 1;
        assert!(l.flood(&tiles, 0, &[(0, 0)], 0));
        assert_eq!(l.count_any(), 6);
        assert!(!l.seen_any(3, 0));
        // Seeds already reached, in a wall, or outside do nothing.
        assert!(!l.flood(&tiles, 0, &[(1, 1), (2, 0), (-1, 0)], 1));
        l.blocked[0][i] = 0;
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
        l.blocked[0][2] = 1;
        l.flood(&tiles, 0, &[(0, 0)], 0);
        l.flood(&tiles, 1, &[(4, 0)], 0);
        assert_eq!(l.count_any(), 5);
        assert!(l.reached[0] && l.reached[1]);
    }
}
