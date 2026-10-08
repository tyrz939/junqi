//! A packed plane of bytes in square chunks (PORT.md §13.3, phase 2): a zone's tiles, or its paint,
//! held at a few bits a cell where a byte a cell held 4 MB for the county.
//!
//! **The chunk API** (the sim reads cells; a renderer streams chunks):
//!
//! - A chunk is [`CHUNK`] x [`CHUNK`] cells; chunk `(cx, cy)` covers cells `cx * CHUNK ..` and
//!   `cy * CHUNK ..`. The last row and column of chunks are clipped by the plane's edge.
//! - Each chunk is coded alone: its distinct values (a palette of 1, 2, 4 or 16, or none) and an
//!   index of 0, 1, 2, 4 or 8 bits a cell. A cell reads in O(1) with no cache ([`Plane::get`]); a
//!   chunk decodes whole into the reader's own buffer ([`Plane::chunk`]), so a renderer keeps
//!   its own small cache of decoded chunks near the camera and nothing here grows.
//! - Read-only: built once from a full grid ([`Plane::pack`]) and never written. What changes in
//!   play (a cleared hedge) is the zone's deltas over it, as over a blueprint's tiles.

use alloc::vec::Vec;

/// Cells to a chunk's edge.
pub const CHUNK: u32 = 16;
const CELLS: usize = (CHUNK * CHUNK) as usize;
const SHIFT: u32 = CHUNK.trailing_zeros();

/// Index bits by code (a descriptor's top three bits).
const BITS: [u32; 5] = [0, 1, 2, 4, 8];
const OFFSET_MASK: u32 = (1 << 29) - 1;

/// A `w x h` plane of bytes, packed by chunk.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Plane {
    w: u32,
    h: u32,
    /// Chunks across.
    cw: u32,
    /// Per chunk, row-major: the code of its index width (top 3 bits) and where its bytes start in
    /// `data` (the rest). At the start: the palette, `1 << bits` bytes (none at 8 bits), then
    /// the index, row-major over the full chunk, low bits first.
    desc: Vec<u32>,
    data: Vec<u8>,
}

impl Plane {
    /// Pack `cells` (`w x h`, row-major).
    pub fn pack(w: u32, h: u32, cells: &[u8]) -> Plane {
        assert_eq!(cells.len(), w as usize * h as usize, "plane size");
        Self::pack_by(w, h, |i| cells[i])
    }

    /// Pack the `w x h` plane whose cell of row-major index `i` is `cell(i)`, with no copy of it.
    pub fn pack_by(w: u32, h: u32, cell: impl Fn(usize) -> u8) -> Plane {
        let cw = w.div_ceil(CHUNK);
        let ch = h.div_ceil(CHUNK);
        let mut desc = Vec::with_capacity((cw * ch) as usize);
        let mut data = Vec::new();
        let mut chunk = [0u8; CELLS];
        for cy in 0..ch {
            for cx in 0..cw {
                // The chunk, its cells off the plane's edge read as its first cell.
                let first = cell((cy * CHUNK) as usize * w as usize + (cx * CHUNK) as usize);
                for (k, c) in chunk.iter_mut().enumerate() {
                    let (x, y) = (cx * CHUNK + k as u32 % CHUNK, cy * CHUNK + k as u32 / CHUNK);
                    *c = if x < w && y < h { cell(y as usize * w as usize + x as usize) } else { first };
                }
                encode(&chunk, &mut desc, &mut data);
            }
        }
        desc.shrink_to_fit();
        data.shrink_to_fit();
        Plane { w, h, cw, desc, data }
    }

    /// Pack a `w x h` plane a chunk at a time: `chunk(cx, cy, cells)` writes chunk `(cx, cy)`
    /// row-major ([`CHUNK`] cells a row); its cells past the plane's edge are then read as its first,
    /// as [`Plane::pack_by`] reads them. For a source held in the same chunks (a builder's canvas).
    pub fn pack_chunks(w: u32, h: u32, mut chunk: impl FnMut(u32, u32, &mut [u8; CELLS])) -> Plane {
        let cw = w.div_ceil(CHUNK);
        let ch = h.div_ceil(CHUNK);
        let mut desc = Vec::with_capacity((cw * ch) as usize);
        let mut data = Vec::new();
        let mut cells = [0u8; CELLS];
        for cy in 0..ch {
            for cx in 0..cw {
                chunk(cx, cy, &mut cells);
                // Only a chunk the plane's edge clips has cells past it.
                if (cx + 1) * CHUNK > w || (cy + 1) * CHUNK > h {
                    let first = cells[0];
                    for (k, c) in cells.iter_mut().enumerate() {
                        if cx * CHUNK + k as u32 % CHUNK >= w || cy * CHUNK + k as u32 / CHUNK >= h {
                            *c = first;
                        }
                    }
                }
                encode(&cells, &mut desc, &mut data);
            }
        }
        desc.shrink_to_fit();
        data.shrink_to_fit();
        Plane { w, h, cw, desc, data }
    }

    /// Pack a `w x h` plane a band of [`CHUNK`] rows at a time: `band(y0, rows, cells)` writes rows
    /// `y0 .. y0 + rows` into `cells` (`w` a row, row-major, zeroed first). Holds one band, never
    /// the whole plane: what a console packs paint with (PORT.md §13.3, phase 3).
    pub fn pack_bands(w: u32, h: u32, mut band: impl FnMut(u32, u32, &mut [u8])) -> Plane {
        let cw = w.div_ceil(CHUNK);
        let ch = h.div_ceil(CHUNK);
        let mut desc = Vec::with_capacity((cw * ch) as usize);
        let mut data = Vec::new();
        let mut rows = alloc::vec![0u8; w as usize * CHUNK as usize];
        let mut chunk = [0u8; CELLS];
        for cy in 0..ch {
            let y0 = cy * CHUNK;
            let n = CHUNK.min(h - y0);
            rows.fill(0);
            band(y0, n, &mut rows[..(w * n) as usize]);
            for cx in 0..cw {
                let x0 = (cx * CHUNK) as usize;
                if (cx + 1) * CHUNK <= w && n == CHUNK {
                    // A whole chunk: a row's stretch at a time.
                    for (y, out) in chunk.chunks_exact_mut(CHUNK as usize).enumerate() {
                        let at = y * w as usize + x0;
                        out.copy_from_slice(&rows[at..at + CHUNK as usize]);
                    }
                } else {
                    let first = rows[x0];
                    for (k, c) in chunk.iter_mut().enumerate() {
                        let (x, y) = (cx * CHUNK + k as u32 % CHUNK, k as u32 / CHUNK);
                        *c = if x < w && y < n { rows[(y * w + x) as usize] } else { first };
                    }
                }
                encode(&chunk, &mut desc, &mut data);
            }
        }
        desc.shrink_to_fit();
        data.shrink_to_fit();
        Plane { w, h, cw, desc, data }
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

    /// Chunks across and down.
    pub const fn chunks(&self) -> (u32, u32) {
        (self.w.div_ceil(CHUNK), self.h.div_ceil(CHUNK))
    }

    /// The byte at in-plane cell `(x, y)`.
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> u8 {
        debug_assert!(x < self.w && y < self.h, "({x}, {y}) outside {}x{}", self.w, self.h);
        let d = self.desc[((y >> SHIFT) * self.cw + (x >> SHIFT)) as usize];
        let at = (d & OFFSET_MASK) as usize;
        let bits = BITS[(d >> 29) as usize];
        let k = ((y & (CHUNK - 1)) << SHIFT | (x & (CHUNK - 1))) as usize;
        match bits {
            0 => self.data[at],
            8 => self.data[at + k],
            b => {
                let bit = k * b as usize;
                let ix = (self.data[at + (1 << b) + bit / 8] >> (bit % 8)) & ((1 << b) - 1);
                self.data[at + usize::from(ix)]
            }
        }
    }

    /// The bytes of `out.len()` cells of row `y` from `x0`, all inside the plane: a chunk's
    /// stretch at a time, so a whole row costs a descriptor a chunk, not a cell.
    pub fn row(&self, x0: u32, y: u32, out: &mut [u8]) {
        debug_assert!(y < self.h && x0 as usize + out.len() <= self.w as usize, "row {y} from {x0}");
        let ky = ((y & (CHUNK - 1)) << SHIFT) as usize;
        let (mut x, mut at) = (x0, 0usize);
        while at < out.len() {
            let kx = (x & (CHUNK - 1)) as usize;
            let len = (CHUNK as usize - kx).min(out.len() - at);
            let span = &mut out[at..at + len];
            let d = self.desc[((y >> SHIFT) * self.cw + (x >> SHIFT)) as usize];
            let base = (d & OFFSET_MASK) as usize;
            let k0 = ky | kx;
            match BITS[(d >> 29) as usize] as usize {
                0 => span.fill(self.data[base]),
                8 => span.copy_from_slice(&self.data[base + k0..base + k0 + len]),
                b => {
                    let (pal, index) = (&self.data[base..base + (1 << b)], base + (1 << b));
                    for (t, o) in span.iter_mut().enumerate() {
                        let bit = (k0 + t) * b;
                        *o = pal[usize::from((self.data[index + bit / 8] >> (bit % 8)) & ((1 << b) - 1))];
                    }
                }
            }
            at += len;
            x += len as u32;
        }
    }

    /// The byte at cell index `i` (row-major).
    #[inline]
    pub fn at(&self, i: u32) -> u8 {
        self.get(i % self.w, i / self.w)
    }

    /// The byte at `(x, y)`, or `outside`.
    #[inline]
    pub fn read(&self, x: i32, y: i32, outside: u8) -> u8 {
        if self.inside(x, y) { self.get(x as u32, y as u32) } else { outside }
    }

    /// Chunk `(cx, cy)` decoded into `out`, row-major, [`CHUNK`] cells a row; cells past the
    /// plane's edge read as the chunk's first. `false` (and `out` untouched) off the plane.
    pub fn chunk(&self, cx: u32, cy: u32, out: &mut [u8; CELLS]) -> bool {
        let (cw, ch) = self.chunks();
        if cx >= cw || cy >= ch {
            return false;
        }
        let d = self.desc[(cy * cw + cx) as usize];
        let at = (d & OFFSET_MASK) as usize;
        let bits = BITS[(d >> 29) as usize] as usize;
        match bits {
            0 => out.fill(self.data[at]),
            8 => out.copy_from_slice(&self.data[at..at + CELLS]),
            b => {
                for (k, o) in out.iter_mut().enumerate() {
                    let bit = k * b;
                    let ix = (self.data[at + (1 << b) + bit / 8] >> (bit % 8)) & ((1 << b) - 1);
                    *o = self.data[at + usize::from(ix)];
                }
            }
        }
        true
    }

    /// Every cell, row-major: the plane unpacked.
    pub fn unpack(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.w as usize * self.h as usize);
        for y in 0..self.h {
            for x in 0..self.w {
                out.push(self.get(x, y));
            }
        }
        out
    }

    /// The plane as held: each chunk's descriptor and the bytes they point into. Equal planes
    /// hold equal bytes (packing is a function of the cells), so a hash may read these.
    pub fn raw(&self) -> (&[u32], &[u8]) {
        (&self.desc, &self.data)
    }

    /// Bytes held.
    pub fn heap_bytes(&self) -> usize {
        self.desc.capacity() * 4 + self.data.capacity()
    }
}

/// The bytes chunk `code` (a descriptor's top three bits) holds: its palette and its index.
#[cfg(feature = "serde")]
const fn chunk_bytes(code: usize) -> usize {
    match BITS[code] {
        0 => 1,
        8 => CELLS,
        b => (1 << b) + CELLS * b as usize / 8,
    }
}

/// As `(w, h, descriptors, bytes)`, the plane exactly as held. Read back only if every chunk's
/// descriptor names a width and bytes the plane has, so every read stays in it.
#[cfg(feature = "serde")]
impl serde::Serialize for Plane {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (self.w, self.h, &self.desc, BytesRef(&self.data)).serialize(s)
    }
}

/// The plane's bytes as one byte string (postcard: a length, then the bytes, as a list of bytes
/// would be), read back at exactly their length: a list grows by doubling past serde's cautious
/// first guess, which cost the county's 1.5 MB another half megabyte.
#[cfg(feature = "serde")]
struct Bytes(Vec<u8>);

#[cfg(feature = "serde")]
struct BytesRef<'a>(&'a [u8]);

#[cfg(feature = "serde")]
impl serde::Serialize for BytesRef<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(self.0)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Bytes {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Bytes;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a plane's bytes")
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Bytes, E> {
                Ok(Bytes(v.to_vec()))
            }
            fn visit_byte_buf<E: serde::de::Error>(self, v: Vec<u8>) -> Result<Bytes, E> {
                Ok(Bytes(v))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Bytes, A::Error> {
                let mut v = Vec::new();
                while let Some(b) = seq.next_element()? {
                    v.push(b);
                }
                v.shrink_to_fit();
                Ok(Bytes(v))
            }
        }
        d.deserialize_bytes(V)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Plane {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (w, h, desc, Bytes(data)) = <(u32, u32, Vec<u32>, Bytes)>::deserialize(d)?;
        let (cw, ch) = (w.div_ceil(CHUNK), h.div_ceil(CHUNK));
        let fits = desc.len() as u64 == u64::from(cw) * u64::from(ch)
            && desc.iter().all(|&d| {
                let code = (d >> 29) as usize;
                code < BITS.len() && (d & OFFSET_MASK) as usize + chunk_bytes(code) <= data.len()
            });
        if !fits {
            return Err(serde::de::Error::custom("a plane's chunks do not fit it"));
        }
        Ok(Plane { w, h, cw, desc, data })
    }
}

/// One chunk coded onto the plane's descriptors and bytes.
fn encode(chunk: &[u8; CELLS], desc: &mut Vec<u32>, data: &mut Vec<u8>) {
    // The palette in order of first appearance, and each value's place in it by a table (`slot`),
    // where a scan of the palette for every cell was the cost.
    let mut palette = [0u8; CELLS];
    let mut len = 0usize;
    let mut slot = [u16::MAX; 256];
    for &c in chunk {
        if slot[usize::from(c)] == u16::MAX {
            slot[usize::from(c)] = len as u16;
            palette[len] = c;
            len += 1;
        }
    }
    let palette = &palette[..len];
    let code = match palette.len() {
        1 => 0,
        2 => 1,
        3..=4 => 2,
        5..=16 => 3,
        _ => 4,
    };
    let bits = BITS[code];
    let at = data.len() as u32;
    assert!(at <= OFFSET_MASK, "a plane past 512 MB");
    desc.push((code as u32) << 29 | at);
    if bits == 8 {
        data.extend_from_slice(chunk);
        return;
    }
    let n = 1usize << bits;
    let mut pal = [0u8; 16];
    pal[..palette.len()].copy_from_slice(palette);
    for p in &mut pal[palette.len()..n] {
        *p = palette[0];
    }
    data.extend_from_slice(&pal[..n]);
    if bits == 0 {
        return;
    }
    let start = data.len();
    data.resize(start + CELLS * bits as usize / 8, 0);
    for (k, c) in chunk.iter().enumerate() {
        let ix = slot[usize::from(*c)] as u8;
        let bit = k * bits as usize;
        data[start + bit / 8] |= ix << (bit % 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plane_reads_back_what_it_packed_cell_and_chunk() {
        let (w, h) = (CHUNK * 5 + 3, CHUNK * 3 + 7);
        let mut s = 0x1234_5678u32;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s
        };
        // Chunks of 1, 2, 3, 5 and 200 values, by row of chunks.
        let cells: Vec<u8> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let r = next();
                match (y / CHUNK + x / CHUNK) % 5 {
                    0 => 7,
                    1 => (r % 2) as u8 * 9,
                    2 => (r % 3) as u8,
                    3 => (r % 5) as u8 + 40,
                    _ => r as u8,
                }
            })
            .collect();
        let p = Plane::pack(w, h, &cells);
        assert_eq!(p.unpack(), cells);
        assert_eq!(p.read(-1, 0, 99), 99);
        assert_eq!(p.read(0, h as i32, 99), 99);
        let (cw, ch) = p.chunks();
        let mut buf = [0u8; CELLS];
        for cy in 0..ch {
            for cx in 0..cw {
                assert!(p.chunk(cx, cy, &mut buf));
                for (k, &b) in buf.iter().enumerate() {
                    let (x, y) = (cx * CHUNK + k as u32 % CHUNK, cy * CHUNK + k as u32 / CHUNK);
                    if x < w && y < h {
                        assert_eq!(b, cells[(y * w + x) as usize], "chunk {cx},{cy} cell {k}");
                    }
                }
            }
        }
        assert!(!p.chunk(cw, 0, &mut buf));
        let mut row = alloc::vec![0u8; w as usize];
        for y in 0..h {
            p.row(0, y, &mut row);
            assert_eq!(row[..], cells[(y * w) as usize..((y + 1) * w) as usize], "row {y}");
            p.row(5, y, &mut row[..30]);
            assert_eq!(row[..30], cells[(y * w + 5) as usize..(y * w + 35) as usize], "row {y} from 5");
        }
        assert!(p.heap_bytes() < cells.len());
        let banded = Plane::pack_bands(w, h, |y0, rows, out| {
            out.copy_from_slice(&cells[(y0 * w) as usize..((y0 + rows) * w) as usize]);
        });
        assert_eq!(banded, p, "packed a band at a time, the same plane");
    }
}
