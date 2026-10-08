//! The tiles of a big zone while it is built (PORT.md §13.3, phase 3): the county's four million
//! cells held a chunk at a time, each [`CHUNK`] x [`CHUNK`] chunk coded by how many kinds of tile
//! it holds: one (inline), up to four (two bits a cell), up to sixteen (four), or more (a byte).
//! A chunk is widened the first time a write needs it and never narrowed. The county is about
//! 1.5 MB this way where a byte a cell was 4 MB, and reads every cell the same.
//!
//! Every read and write is a cell's; the build's whole-map floods ask [`Canvas::solid_word`].

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use jane_core::tile::F_SOLID;
use jane_core::{Grid, Plane, Rect, Tile};

/// Cells to a chunk's edge.
const CHUNK: u32 = 16;
const SHIFT: u32 = CHUNK.trailing_zeros();
const CELLS: usize = (CHUNK * CHUNK) as usize;

/// A chunk descriptor's kind (top two bits) and value (the rest: a tile id, or an entry).
const KIND: u32 = 3 << 30;
const VALUE: u32 = !KIND;
const ONE: u32 = 0;
const TWO: u32 = 1 << 30;
const FOUR: u32 = 2 << 30;
const RAW: u32 = 3 << 30;

/// An unused palette slot (no tile has this id).
const NONE: u8 = u8::MAX;

/// Whether the tile of each id stops feet (an id no tile has reads as `Tile::Void`).
static SOLID: [bool; 256] = {
    let mut a = [false; 256];
    let mut i = 0;
    while i < 256 {
        let t = match Tile::from_id(i as u8) {
            Some(t) => t,
            None => Tile::Void,
        };
        a[i] = t.flags() & F_SOLID != 0;
        i += 1;
    }
    a
};

/// Entries a block of a [`Pool`] holds.
const BLOCK: usize = 128;

/// Entries of `N` bytes in blocks of [`BLOCK`], freed ones reused: it grows a block at a time and
/// never copies itself (a pool of a megabyte grown by copying held two for a moment).
#[derive(Clone, Debug)]
struct Pool<const N: usize> {
    blocks: Vec<Box<[[u8; N]]>>,
    len: u32,
    free: Vec<u32>,
}

impl<const N: usize> Pool<N> {
    const fn new() -> Self {
        Self { blocks: Vec::new(), len: 0, free: Vec::new() }
    }

    fn alloc(&mut self) -> u32 {
        if let Some(e) = self.free.pop() {
            return e;
        }
        if self.len as usize == self.blocks.len() * BLOCK {
            self.blocks.push(vec![[0; N]; BLOCK].into_boxed_slice());
        }
        self.len += 1;
        self.len - 1
    }

    #[inline]
    fn get(&self, e: u32) -> &[u8; N] {
        &self.blocks[e as usize / BLOCK][e as usize % BLOCK]
    }

    #[inline]
    fn get_mut(&mut self, e: u32) -> &mut [u8; N] {
        &mut self.blocks[e as usize / BLOCK][e as usize % BLOCK]
    }

    fn release(&mut self, e: u32) {
        self.free.push(e);
    }

    fn heap_bytes(&self) -> usize {
        self.blocks.len() * BLOCK * N + self.blocks.capacity() * 16 + self.free.capacity() * 4
    }
}

/// A `w x h` plane of tiles in chunks; see the module docs.
#[derive(Clone, Debug)]
pub struct Canvas {
    w: u32,
    h: u32,
    /// Chunks across.
    cw: u32,
    desc: Vec<u32>,
    /// Up to four kinds: four palette bytes, then two bits a cell (64 bytes).
    two: Pool<68>,
    /// Up to sixteen: sixteen palette bytes, then four bits a cell (128 bytes).
    four: Pool<144>,
    raw: Pool<256>,
}

impl Canvas {
    /// `w x h` cells of `fill`.
    pub fn new(w: u32, h: u32, fill: Tile) -> Self {
        let cw = w.div_ceil(CHUNK);
        let n = (cw * h.div_ceil(CHUNK)) as usize;
        Self {
            w,
            h,
            cw,
            desc: vec![ONE | u32::from(fill.id()); n],
            two: Pool::new(),
            four: Pool::new(),
            raw: Pool::new(),
        }
    }

    pub const fn w(&self) -> u32 {
        self.w
    }

    pub const fn h(&self) -> u32 {
        self.h
    }

    pub const fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h
    }

    /// The chunk of in-plane `(x, y)` and the cell's place in it.
    #[inline]
    fn at(&self, x: u32, y: u32) -> (usize, usize) {
        (((y >> SHIFT) * self.cw + (x >> SHIFT)) as usize, ((y & (CHUNK - 1)) << SHIFT | (x & (CHUNK - 1))) as usize)
    }

    /// The tile id at in-plane `(x, y)`.
    #[inline]
    fn id(&self, x: u32, y: u32) -> u8 {
        let (c, k) = self.at(x, y);
        let d = self.desc[c];
        let v = d & VALUE;
        match d & KIND {
            ONE => v as u8,
            TWO => {
                let e = self.two.get(v);
                e[usize::from((e[4 + (k >> 2)] >> ((k & 3) * 2)) & 3)]
            }
            FOUR => {
                let e = self.four.get(v);
                e[usize::from((e[16 + (k >> 1)] >> ((k & 1) * 4)) & 15)]
            }
            _ => self.raw.get(v)[k],
        }
    }

    /// The tile at `(x, y)`; `Tile::Void` outside.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Tile {
        if !self.inside(x, y) {
            return Tile::Void;
        }
        Tile::from_id(self.id(x as u32, y as u32)).unwrap_or(Tile::Void)
    }

    /// Whether the tile at `(x, y)` stops feet (outside does, as `Tile::Void`).
    #[inline]
    pub fn solid(&self, x: i32, y: i32) -> bool {
        if !self.inside(x, y) {
            return SOLID[usize::from(Tile::Void.id())];
        }
        SOLID[usize::from(self.id(x as u32, y as u32))]
    }

    /// The tile at in-plane cell index `i` (`y * w + x`).
    #[inline]
    pub fn get_ix(&self, i: usize) -> Tile {
        let (x, y) = ((i % self.w as usize) as u32, (i / self.w as usize) as u32);
        Tile::from_id(self.id(x, y)).unwrap_or(Tile::Void)
    }

    /// Set the tile at `(x, y)`; outside is a no-op.
    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        if !self.inside(x, y) {
            return;
        }
        let (c, k) = self.at(x as u32, y as u32);
        let id = t.id();
        loop {
            let d = self.desc[c];
            let v = d & VALUE;
            match d & KIND {
                ONE => {
                    if v as u8 == id {
                        return;
                    }
                    let e = self.two.alloc();
                    let s = self.two.get_mut(e);
                    *s = [0; 68];
                    s[..4].copy_from_slice(&[v as u8, NONE, NONE, NONE]);
                    self.desc[c] = TWO | e;
                }
                TWO => {
                    let s = self.two.get_mut(v);
                    match slot(&mut s[..4], id) {
                        Some(ix) => {
                            let (b, sh) = (4 + (k >> 2), (k & 3) * 2);
                            s[b] = (s[b] & !(3 << sh)) | ((ix as u8) << sh);
                            return;
                        }
                        None => self.widen_two(c, v),
                    }
                }
                FOUR => {
                    let s = self.four.get_mut(v);
                    match slot(&mut s[..16], id) {
                        Some(ix) => {
                            let (b, sh) = (16 + (k >> 1), (k & 1) * 4);
                            s[b] = (s[b] & !(15 << sh)) | ((ix as u8) << sh);
                            return;
                        }
                        None => self.widen_four(c, v),
                    }
                }
                _ => {
                    self.raw.get_mut(v)[k] = id;
                    return;
                }
            }
        }
    }

    /// Chunk `c` (two bits, entry `e`) as four bits.
    fn widen_two(&mut self, c: usize, e: u32) {
        let old = *self.two.get(e);
        self.two.release(e);
        let n = self.four.alloc();
        let s = self.four.get_mut(n);
        *s = [0; 144];
        s[..16].fill(NONE);
        s[..4].copy_from_slice(&old[..4]);
        for k in 0..CELLS {
            let ix = (old[4 + (k >> 2)] >> ((k & 3) * 2)) & 3;
            s[16 + (k >> 1)] |= ix << ((k & 1) * 4);
        }
        self.desc[c] = FOUR | n;
    }

    /// Chunk `c` (four bits, entry `e`) as a byte a cell.
    fn widen_four(&mut self, c: usize, e: u32) {
        let old = *self.four.get(e);
        self.four.release(e);
        let n = self.raw.alloc();
        let s = self.raw.get_mut(n);
        for (k, b) in s.iter_mut().enumerate() {
            *b = old[usize::from((old[16 + (k >> 1)] >> ((k & 1) * 4)) & 15)];
        }
        self.desc[c] = RAW | n;
    }

    /// Set the tile at cell index `i`.
    #[inline]
    pub fn set_ix(&mut self, i: usize, t: Tile) {
        let (x, y) = ((i % self.w as usize) as i32, (i / self.w as usize) as i32);
        self.set(x, y, t);
    }

    /// Every cell of `r` inside the plane.
    pub fn fill_rect(&mut self, r: Rect, t: Tile) {
        if let Some(r) = r.intersect(Rect::new(0, 0, self.w as i32, self.h as i32)) {
            for (x, y) in r.cells() {
                self.set(x, y, t);
            }
        }
    }

    /// Cells `64 k .. 64 k + 63` (by index) that stop feet (`F_SOLID`), cell `64 k + j` at bit `j`;
    /// bits past the last cell clear.
    pub fn solid_word(&self, k: usize) -> u64 {
        let n = self.w as usize * self.h as usize;
        let base = k << 6;
        let count = (n - base).min(64) as u32;
        let (mut x, mut y) = ((base % self.w as usize) as u32, (base / self.w as usize) as u32);
        let (mut m, mut j) = (0u64, 0u32);
        // A chunk's stretch of the row at a time.
        while j < count {
            let kx = x & (CHUNK - 1);
            let len = (CHUNK - kx).min(self.w - x).min(count - j);
            m |= self.row_solid(x, y, len) << j;
            j += len;
            x += len;
            if x == self.w {
                x = 0;
                y += 1;
            }
        }
        m
    }

    /// The solid bits of `len` cells from `(x, y)` along its row, all in one chunk.
    #[inline]
    fn row_solid(&self, x: u32, y: u32, len: u32) -> u64 {
        let (c, k0) = self.at(x, y);
        let d = self.desc[c];
        let v = d & VALUE;
        let all = (1u64 << len) - 1;
        let solid = |id: u8| SOLID[usize::from(id)];
        match d & KIND {
            ONE => {
                if solid(v as u8) {
                    all
                } else {
                    0
                }
            }
            TWO => {
                let e = self.two.get(v);
                let pal = (0..4).fold(0u8, |m, i| m | u8::from(e[i] != NONE && solid(e[i])) << i);
                if pal == 0 {
                    return 0;
                }
                (0..len as usize).fold(0u64, |m, t| {
                    let k = k0 + t;
                    let ix = (e[4 + (k >> 2)] >> ((k & 3) * 2)) & 3;
                    m | u64::from(pal >> ix & 1) << t
                })
            }
            FOUR => {
                let e = self.four.get(v);
                let pal = (0..16).fold(0u16, |m, i| m | u16::from(e[i] != NONE && solid(e[i])) << i);
                if pal == 0 {
                    return 0;
                }
                (0..len as usize).fold(0u64, |m, t| {
                    let k = k0 + t;
                    let ix = (e[16 + (k >> 1)] >> ((k & 1) * 4)) & 15;
                    m | u64::from(pal >> ix & 1) << t
                })
            }
            _ => {
                let e = self.raw.get(v);
                (0..len as usize).fold(0u64, |m, t| m | u64::from(solid(e[k0 + t])) << t)
            }
        }
    }

    /// The tiles as a grid, a byte a cell.
    pub fn to_grid(&self) -> Grid<Tile> {
        let mut cells = Vec::with_capacity(self.w as usize * self.h as usize);
        for y in 0..self.h {
            for x in 0..self.w {
                cells.push(Tile::from_id(self.id(x, y)).unwrap_or(Tile::Void));
            }
        }
        Grid::from_vec(self.w, self.h, cells)
    }

    /// The tiles packed (`jane_core::plane`), as `Blueprint::pack` packs a grid of them: a chunk
    /// decoded whole at a time (the canvas's chunks are the plane's).
    pub fn pack(&self) -> Plane {
        Plane::pack_chunks(self.w, self.h, |cx, cy, out| self.decode(cx, cy, out))
    }

    /// Chunk `(cx, cy)` decoded, row-major (cells past the plane's edge as the codes leave them).
    fn decode(&self, cx: u32, cy: u32, out: &mut [u8; CELLS]) {
        let d = self.desc[(cy * self.cw + cx) as usize];
        let v = d & VALUE;
        match d & KIND {
            ONE => out.fill(v as u8),
            TWO => {
                let e = self.two.get(v);
                for (k, o) in out.iter_mut().enumerate() {
                    *o = e[usize::from((e[4 + (k >> 2)] >> ((k & 3) * 2)) & 3)];
                }
            }
            FOUR => {
                let e = self.four.get(v);
                for (k, o) in out.iter_mut().enumerate() {
                    *o = e[usize::from((e[16 + (k >> 1)] >> ((k & 1) * 4)) & 15)];
                }
            }
            _ => out.copy_from_slice(self.raw.get(v)),
        }
    }

    /// Every chunk's bytes moved together, the slots that widening left empty and the pools'
    /// spare room let go: a canvas after a stage that wrote all over it (the land) holds what its
    /// chunks need and no more. Reads the same.
    pub fn compact(&mut self) {
        let (mut two, mut four, mut raw) = (Pool::<68>::new(), Pool::<144>::new(), Pool::<256>::new());
        for d in &mut self.desc {
            let v = *d & VALUE;
            *d = match *d & KIND {
                TWO => {
                    let e = two.alloc();
                    *two.get_mut(e) = *self.two.get(v);
                    TWO | e
                }
                FOUR => {
                    let e = four.alloc();
                    *four.get_mut(e) = *self.four.get(v);
                    FOUR | e
                }
                RAW => {
                    let e = raw.alloc();
                    *raw.get_mut(e) = *self.raw.get(v);
                    RAW | e
                }
                _ => *d,
            };
        }
        (self.two, self.four, self.raw) = (two, four, raw);
    }

    /// Bytes held.
    pub fn heap_bytes(&self) -> usize {
        self.desc.capacity() * 4 + self.two.heap_bytes() + self.four.heap_bytes() + self.raw.heap_bytes()
    }
}

/// The last words a flood asked of [`Canvas::solid_word`], by word: a scanline fill asks the rows
/// about the one it is on again and again, so a few thousand words (48 KB) answer it nearly
/// always, where a copy of every word was half a megabyte.
#[derive(Clone, Debug)]
pub struct WordCache {
    /// Word number plus one; 0 empty.
    tags: Vec<u32>,
    words: Vec<u64>,
}

impl Default for WordCache {
    fn default() -> Self {
        Self { tags: vec![0; 4096], words: vec![0; 4096] }
    }
}

impl WordCache {
    /// Word `k`, from the cache or from `f`.
    #[inline]
    pub fn get(&mut self, k: usize, f: impl FnOnce(usize) -> u64) -> u64 {
        let s = k & (self.tags.len() - 1);
        if self.tags[s] != k as u32 + 1 {
            self.tags[s] = k as u32 + 1;
            self.words[s] = f(k);
        }
        self.words[s]
    }
}

/// The palette slot of `id` in `pal`, taking the first empty one if it is new; `None` if full.
fn slot(pal: &mut [u8], id: u8) -> Option<usize> {
    for (i, p) in pal.iter_mut().enumerate() {
        if *p == id {
            return Some(i);
        }
        if *p == NONE {
            *p = id;
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every write through every widening reads back as a grid of bytes would.
    #[test]
    fn a_canvas_reads_back_what_a_grid_would() {
        let (w, h) = (CHUNK * 3 + 5, CHUNK * 2 + 9);
        let mut c = Canvas::new(w, h, Tile::Grass);
        let mut g = Grid::new(w, h, Tile::Grass);
        let kinds: Vec<Tile> = (0..=255u8).filter_map(Tile::from_id).collect();
        let mut s = 0x2545_f491u32;
        for round in 0..20_000u32 {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let (x, y) = ((s % (w + 4)) as i32 - 2, ((s >> 8) % (h + 4)) as i32 - 2);
            // Few kinds at first, then many: chunks widen step by step.
            let spread = 2 + (round as usize / 1000).min(kinds.len() - 2);
            let t = kinds[(s >> 16) as usize % spread];
            c.set(x, y, t);
            g.set(x, y, t);
        }
        c.fill_rect(Rect::new(3, 4, 20, 2), Tile::Water);
        g.fill_rect(Rect::new(3, 4, 20, 2), Tile::Water);
        assert_eq!(c.to_grid(), g);
        let before = c.heap_bytes();
        c.compact();
        assert!(c.heap_bytes() <= before);
        assert_eq!(c.to_grid(), g, "compacted, the same tiles");
        c.set(1, 1, Tile::Road);
        g.set(1, 1, Tile::Road);
        assert_eq!(c.to_grid(), g, "and it still widens and writes");
        for y in -1..=h as i32 {
            for x in -1..=w as i32 {
                assert_eq!(c.get(x, y), g.read(x, y, Tile::Void), "({x}, {y})");
            }
        }
        let n = (w * h) as usize;
        for k in 0..n.div_ceil(64) {
            let want = (k << 6..((k + 1) << 6).min(n))
                .enumerate()
                .fold(0u64, |m, (j, i)| m | u64::from(g.as_slice()[i].flags() & F_SOLID != 0) << j);
            assert_eq!(c.solid_word(k), want);
        }
        assert_eq!(c.pack(), Plane::pack_by(w, h, |i| g.as_slice()[i].id()));
    }
}
