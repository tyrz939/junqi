//! Grids, cells and rects. Sizes are `u32`, never `usize`, in anything that is saved or hashed.

use alloc::vec;
use alloc::vec::Vec;

/// A cell of a zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cell {
    pub x: u16,
    pub y: u16,
}

impl Cell {
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

/// A flat cell index, `y * w + x`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellIx(pub u32);

/// A rect of cells: `x..x + w`, `y..y + h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub const fn overlaps(self, o: Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// Grown by `n` cells on every side (shrunk for negative `n`).
    pub const fn grow(self, n: i32) -> Rect {
        Rect { x: self.x - n, y: self.y - n, w: self.w + 2 * n, h: self.h + 2 * n }
    }

    /// The overlap, or `None`.
    pub fn intersect(self, o: Rect) -> Option<Rect> {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        (r > x && b > y).then(|| Rect::new(x, y, r - x, b - y))
    }

    pub const fn area(self) -> i64 {
        self.w as i64 * self.h as i64
    }

    /// The centre cell (floored).
    pub const fn centre(self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// Every cell, row by row.
    pub fn cells(self) -> impl Iterator<Item = (i32, i32)> {
        (self.y..self.bottom()).flat_map(move |y| (self.x..self.right()).map(move |x| (x, y)))
    }
}

/// A dense `w x h` grid, row-major.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Grid<T> {
    w: u32,
    h: u32,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    pub fn new(w: u32, h: u32, fill: T) -> Self {
        Self { w, h, cells: vec![fill; (w as usize) * (h as usize)] }
    }

    pub fn fill(&mut self, v: T) {
        self.cells.fill(v);
    }

    /// Set every cell of `r` that is inside the grid.
    pub fn fill_rect(&mut self, r: Rect, v: T) {
        if let Some(r) = r.intersect(self.bounds()) {
            for y in r.y..r.bottom() {
                let row = self.ix(r.x as u32, y as u32);
                self.cells[row..row + r.w as usize].fill(v.clone());
            }
        }
    }
}

impl<T> Grid<T> {
    pub fn from_vec(w: u32, h: u32, cells: Vec<T>) -> Self {
        assert_eq!(cells.len(), (w as usize) * (h as usize), "grid size");
        Self { w, h, cells }
    }

    pub const fn w(&self) -> u32 {
        self.w
    }

    pub const fn h(&self) -> u32 {
        self.h
    }

    pub const fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.w as i32, self.h as i32)
    }

    pub const fn len(&self) -> u32 {
        self.w * self.h
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub const fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h
    }

    /// Index of an in-bounds cell. Panics in debug for a cell outside.
    #[inline]
    pub fn ix(&self, x: u32, y: u32) -> usize {
        debug_assert!(x < self.w && y < self.h, "({x}, {y}) outside {}x{}", self.w, self.h);
        y as usize * self.w as usize + x as usize
    }

    pub fn cell_ix(&self, x: u32, y: u32) -> CellIx {
        CellIx(y * self.w + x)
    }

    pub fn xy(&self, i: CellIx) -> (u32, u32) {
        (i.0 % self.w, i.0 / self.w)
    }

    /// The cell at `(x, y)`, or `None` outside.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<&T> {
        if self.inside(x, y) { Some(&self.cells[self.ix(x as u32, y as u32)]) } else { None }
    }

    #[inline]
    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut T> {
        if self.inside(x, y) {
            let i = self.ix(x as u32, y as u32);
            Some(&mut self.cells[i])
        } else {
            None
        }
    }

    /// Set a cell; outside the grid is a no-op.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: T) {
        if let Some(c) = self.get_mut(x, y) {
            *c = v;
        }
    }

    #[inline]
    pub fn at(&self, i: CellIx) -> &T {
        &self.cells[i.0 as usize]
    }

    #[inline]
    pub fn at_mut(&mut self, i: CellIx) -> &mut T {
        &mut self.cells[i.0 as usize]
    }

    pub fn as_slice(&self) -> &[T] {
        &self.cells
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.cells
    }

    pub fn into_vec(self) -> Vec<T> {
        self.cells
    }
}

impl<T: Copy> Grid<T> {
    /// The cell at `(x, y)`, or `outside`.
    #[inline]
    pub fn read(&self, x: i32, y: i32, outside: T) -> T {
        self.get(x, y).copied().unwrap_or(outside)
    }
}

/// The 4 orthogonal steps, in the fixed order every search uses: E, S, W, N.
pub const DIRS4: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// The 8 steps: E, S, W, N, then SE, SW, NW, NE.
pub const DIRS8: [(i32, i32); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_ops() {
        let a = Rect::new(0, 0, 4, 4);
        let b = Rect::new(2, 2, 4, 4);
        assert!(a.overlaps(b));
        assert_eq!(a.intersect(b), Some(Rect::new(2, 2, 2, 2)));
        assert_eq!(a.intersect(Rect::new(4, 0, 2, 2)), None);
        assert!(!a.overlaps(Rect::new(4, 0, 2, 2)));
        assert_eq!(a.grow(1), Rect::new(-1, -1, 6, 6));
        assert_eq!(a.cells().count(), 16);
        assert!(a.contains(3, 3) && !a.contains(4, 3));
    }

    #[test]
    fn grid_bounds_and_fill() {
        let mut g = Grid::new(5, 3, 0u8);
        assert_eq!(g.len(), 15);
        g.fill_rect(Rect::new(-2, 1, 4, 10), 7);
        assert_eq!(g.read(0, 1, 9), 7);
        assert_eq!(g.read(1, 2, 9), 7);
        assert_eq!(g.read(2, 1, 9), 0);
        assert_eq!(g.read(-1, 0, 9), 9);
        g.set(99, 99, 1);
        let i = g.cell_ix(4, 2);
        assert_eq!(g.xy(i), (4, 2));
    }
}
