//! Baked console chunks (PLAN-PSP-SPEED.md §5): every chunk of the county painted offline by the
//! console's own path (a `T8` [`ChunkLayers`] and its slot's sidecar, exactly as
//! [`Terrain::land`](crate::terrain::Terrain::land) leaves them on a PSP), each compressed on its
//! own, so a console reads and decodes a chunk in a few ms instead of painting it in about 100.
//! `jane bake-chunks` writes the pack; [`Pack`] and [`Decoder`] read it (`no_std`).
//!
//! ```text
//! "JCK1"  u16 version  u8 codec (0 raw, 1 zstd, 2 lz4 block)  u8 0
//! u16 cols  u16 rows            the zone's chunks, row-major: chunk (cx, cy) is entry cy * cols + cx
//! u64 digest                    xxh3 of every chunk's raw record (albedo, height, sidecar) in entry order:
//!                               the painter's output itself, what tells a stale pack (`jane bake-chunks --check`)
//! u32 offsets[cols * rows + 1]  each chunk's blob, from the end of this table; the last is the blobs' end
//! blobs                         per chunk: u32 albedo frame bytes, u32 height frame bytes, u32 sidecar raw
//!                               bytes, then the three frames, each compressed on its own (no dictionary)
//! ```
//!
//! A raw albedo is `CHUNK_PX²` `T8` indices, px `k` at byte `k` (the console's `T8` words,
//! little-endian); a raw height is `CHUNK_PX²` px heights over each cell's base (`terrain::relative`).
//! The sidecar is in the tables' encoding (`tables`): the CLUT's entries in use, what glows, the
//! water cells, the placements, the lit windows, the blocks, the night's sockets (openings, faces,
//! ways, rails), the cells' bases and the falls.
//!
//! The bake is of a New Game's county: a chunk whose tiles or spans change in play (a gate opened,
//! a bridge mended) is the painter's again (P1c).

use alloc::vec::Vec;

use jane_art::terrain::{Face, FencePart, Opening, Placed};
use jane_sim::view::View;

use crate::atlas::{PackError, Reader};
use crate::frame::{Block, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers};
use crate::tables::{Tab, tab_struct};
use crate::terrain::{Terrain, Window};

/// The pack's magic.
pub const MAGIC: &[u8; 4] = b"JCK1";
/// Its version: raise it with any change to the format or to what a [`Baked`] holds.
pub const VERSION: u16 = 1;
/// The header's bytes before the offsets.
const HEAD: usize = 4 + 2 + 1 + 1 + 2 + 2 + 8;
/// A chunk's px.
const PX: usize = (CHUNK_PX * CHUNK_PX) as usize;

/// How a pack's frames are compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Raw,
    /// One zstd frame (RFC 8878), decoded by `ruzstd`.
    Zstd,
    /// One LZ4 block (an lz4 HC encoder's, or any), decoded by `lz4_flex`.
    Lz4,
}

impl Codec {
    pub fn tag(self) -> u8 {
        match self {
            Codec::Raw => 0,
            Codec::Zstd => 1,
            Codec::Lz4 => 2,
        }
    }

    pub fn of_tag(t: u8) -> Option<Codec> {
        [Codec::Raw, Codec::Zstd, Codec::Lz4].into_iter().find(|c| c.tag() == t)
    }
}

/// A console chunk as baked: its `T8` layers and its slot's sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Baked {
    /// `CHUNK_PX²` `T8` indices into `clut`, px `k` at byte `k`.
    pub albedo: Vec<u8>,
    /// The CLUT's entries in use, `0xAABBGGRR` (`ChunkLayers::clut`, `clut_n` long).
    pub clut: Vec<u32>,
    /// `CHUNK_PX²` px heights over each cell's base.
    pub height: Vec<u8>,
    pub glow: Vec<(u16, u32)>,
    pub water: Vec<(u8, u8, u8)>,
    pub placed: Vec<Placed>,
    pub windows: Vec<Window>,
    pub blocks: Vec<Block>,
    pub openings: Vec<Opening>,
    pub faces: Vec<Face>,
    pub ways: [u16; CHUNK_CELLS as usize],
    pub rails: Vec<FencePart>,
    pub bases: [u8; (CHUNK_CELLS * CHUNK_CELLS) as usize],
    pub falls: Vec<(u8, u8)>,
}

impl Default for Baked {
    fn default() -> Self {
        Baked {
            albedo: Vec::new(),
            clut: Vec::new(),
            height: Vec::new(),
            glow: Vec::new(),
            water: Vec::new(),
            placed: Vec::new(),
            windows: Vec::new(),
            blocks: Vec::new(),
            openings: Vec::new(),
            faces: Vec::new(),
            ways: [0; CHUNK_CELLS as usize],
            rails: Vec::new(),
            bases: [0; (CHUNK_CELLS * CHUNK_CELLS) as usize],
            falls: Vec::new(),
        }
    }
}

tab_struct!(Placed { sprite, x, y, row });
tab_struct!(Window { x, y, height, colour });
tab_struct!(Block { x0, y0, x1, y1, height, lo, fence });
tab_struct!(Opening { x, y, w, h, lit, upper });
tab_struct!(Face { x, y, plaster, eave });
tab_struct!(FencePart { x0, y0, x1, y1, lo, hi });

impl Baked {
    /// The sidecar's raw bytes (the module doc's order).
    pub fn sidecar(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(2048);
        self.clut.put(&mut o);
        self.glow.put(&mut o);
        self.water.put(&mut o);
        self.placed.put(&mut o);
        self.windows.put(&mut o);
        self.blocks.put(&mut o);
        self.openings.put(&mut o);
        self.faces.put(&mut o);
        self.ways.put(&mut o);
        self.rails.put(&mut o);
        self.bases.put(&mut o);
        self.falls.put(&mut o);
        o
    }

    /// The sidecar read back from `bytes` into `self`.
    fn read_sidecar(&mut self, bytes: &[u8]) -> Result<(), PackError> {
        let mut r = Reader::new(bytes);
        self.clut = Tab::get(&mut r)?;
        self.glow = Tab::get(&mut r)?;
        self.water = Tab::get(&mut r)?;
        self.placed = Tab::get(&mut r)?;
        self.windows = Tab::get(&mut r)?;
        self.blocks = Tab::get(&mut r)?;
        self.openings = Tab::get(&mut r)?;
        self.faces = Tab::get(&mut r)?;
        self.ways = Tab::get(&mut r)?;
        self.rails = Tab::get(&mut r)?;
        self.bases = Tab::get(&mut r)?;
        self.falls = Tab::get(&mut r)?;
        if r.left() != 0 || self.clut.len() > 256 {
            return Err(PackError("a chunk's sidecar"));
        }
        Ok(())
    }

    /// The chunk's raw record (albedo, height, sidecar), what the pack's digest hashes.
    pub fn record(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(2 * PX + 2048);
        o.extend_from_slice(&self.albedo);
        o.extend_from_slice(&self.height);
        o.extend_from_slice(&self.sidecar());
        o
    }

    /// The chunk as one pack blob, each frame compressed by `compress`.
    pub fn blob(&self, mut compress: impl FnMut(&[u8]) -> Vec<u8>) -> Vec<u8> {
        let side = self.sidecar();
        let (a, h, s) = (compress(&self.albedo), compress(&self.height), compress(&side));
        let mut o = Vec::with_capacity(12 + a.len() + h.len() + s.len());
        for n in [a.len(), h.len(), side.len()] {
            o.extend_from_slice(&(n as u32).to_le_bytes());
        }
        o.extend_from_slice(&a);
        o.extend_from_slice(&h);
        o.extend_from_slice(&s);
        o
    }

    /// Lays the chunk into a console's `T8` `layers` as [`Terrain::land`] lays a painted one.
    pub fn lay(&self, layers: &mut ChunkLayers) {
        for (w, b) in layers.albedo.iter_mut().zip(self.albedo.chunks_exact(4)) {
            *w = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        layers.clut[..self.clut.len()].copy_from_slice(&self.clut);
        layers.clut_n = self.clut.len() as u16;
        layers.height.copy_from_slice(&self.height);
        layers.glow.clear();
        layers.glow.extend_from_slice(&self.glow);
        layers.water.clear();
        layers.water.extend_from_slice(&self.water);
    }
}

/// A whole pack: its header, its blobs, a codec and a digest.
pub fn write(codec: Codec, (cols, rows): (u16, u16), digest: u64, blobs: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = blobs.iter().map(Vec::len).sum();
    let mut o = Vec::with_capacity(HEAD + 4 * (blobs.len() + 1) + total);
    o.extend_from_slice(MAGIC);
    o.extend_from_slice(&VERSION.to_le_bytes());
    o.extend_from_slice(&[codec.tag(), 0]);
    o.extend_from_slice(&cols.to_le_bytes());
    o.extend_from_slice(&rows.to_le_bytes());
    o.extend_from_slice(&digest.to_le_bytes());
    let mut at = 0u32;
    for b in blobs {
        o.extend_from_slice(&at.to_le_bytes());
        at += b.len() as u32;
    }
    o.extend_from_slice(&at.to_le_bytes());
    for b in blobs {
        o.extend_from_slice(b);
    }
    o
}

/// A pack's header read over its bytes.
#[derive(Clone, Copy, Debug)]
pub struct Pack<'a> {
    bytes: &'a [u8],
    pub codec: Codec,
    pub cols: u16,
    pub rows: u16,
    pub digest: u64,
    blobs_at: usize,
}

impl<'a> Pack<'a> {
    /// The header and offsets of a pack, checked: its magic, version, codec and lengths.
    pub fn open(bytes: &'a [u8]) -> Result<Pack<'a>, PackError> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != MAGIC {
            return Err(PackError("not a JCK1 pack"));
        }
        if r.u16()? != VERSION {
            return Err(PackError("JCK version"));
        }
        let codec = Codec::of_tag(r.u8()?).ok_or(PackError("JCK codec"))?;
        r.u8()?;
        let (cols, rows) = (r.u16()?, r.u16()?);
        let d = r.take(8)?;
        let digest = u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
        let n = usize::from(cols) * usize::from(rows);
        let blobs_at = HEAD + 4 * (n + 1);
        let pack = Pack { bytes, codec, cols, rows, digest, blobs_at };
        if bytes.len() < blobs_at || pack.offset(n) != bytes.len() - blobs_at {
            return Err(PackError("JCK length"));
        }
        Ok(pack)
    }

    fn offset(&self, i: usize) -> usize {
        let k = HEAD + 4 * i;
        u32::from_le_bytes([self.bytes[k], self.bytes[k + 1], self.bytes[k + 2], self.bytes[k + 3]]) as usize
    }

    /// Chunk `id`'s blob, if the pack holds it.
    pub fn blob(&self, id: ChunkId) -> Option<&'a [u8]> {
        if id.cx >= self.cols || id.cy >= self.rows {
            return None;
        }
        let i = usize::from(id.cy) * usize::from(self.cols) + usize::from(id.cx);
        self.bytes.get(self.blobs_at + self.offset(i)..self.blobs_at + self.offset(i + 1))
    }
}

/// A pack's header and offsets without its blobs: what a console keeps in RAM (about 36 KB for
/// the county) while the blobs stay on the stick, each read when it is wanted (P1c).
#[derive(Clone, Debug)]
pub struct Index {
    pub codec: Codec,
    pub cols: u16,
    pub rows: u16,
    pub digest: u64,
    /// Each chunk's blob from the end of the table, and the blobs' end.
    offsets: Vec<u32>,
    /// The file offset of the blobs.
    blobs_at: u32,
}

impl Index {
    /// The header's bytes (what [`head_len`](Self::head_len) reads).
    pub const HEAD: usize = HEAD;

    /// The bytes of a pack's header and offsets, from its first [`HEAD`](Self::HEAD) bytes.
    pub fn head_len(head: &[u8]) -> Result<usize, PackError> {
        let h = head.get(..HEAD).ok_or(PackError("JCK header"))?;
        if &h[..4] != MAGIC {
            return Err(PackError("not a JCK1 pack"));
        }
        let (cols, rows) = (u16::from_le_bytes([h[8], h[9]]), u16::from_le_bytes([h[10], h[11]]));
        Ok(HEAD + 4 * (usize::from(cols) * usize::from(rows) + 1))
    }

    /// The index of a pack of `file_len` bytes from its first [`head_len`](Self::head_len) bytes, checked as
    /// [`Pack::open`] checks a whole one.
    pub fn read(head: &[u8], file_len: u32) -> Result<Index, PackError> {
        let n = Index::head_len(head)?;
        let bytes = head.get(..n).ok_or(PackError("JCK index"))?;
        let mut r = Reader::new(bytes);
        r.take(4)?;
        if r.u16()? != VERSION {
            return Err(PackError("JCK version"));
        }
        let codec = Codec::of_tag(r.u8()?).ok_or(PackError("JCK codec"))?;
        r.u8()?;
        let (cols, rows) = (r.u16()?, r.u16()?);
        let d = r.take(8)?;
        let digest = u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
        let offsets: Vec<u32> =
            bytes[HEAD..].chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        let ok = offsets.windows(2).all(|w| w[0] <= w[1])
            && offsets.last().is_some_and(|&end| u64::from(end) + n as u64 == u64::from(file_len));
        if !ok {
            return Err(PackError("JCK length"));
        }
        Ok(Index { codec, cols, rows, digest, offsets, blobs_at: n as u32 })
    }

    /// Chunk `id`'s blob in the file: its offset and its bytes.
    pub fn span(&self, id: ChunkId) -> Option<(u32, u32)> {
        if id.cx >= self.cols || id.cy >= self.rows {
            return None;
        }
        let i = usize::from(id.cy) * usize::from(self.cols) + usize::from(id.cx);
        Some((self.blobs_at + self.offsets[i], self.offsets[i + 1] - self.offsets[i]))
    }

    /// The largest blob's bytes (a read buffer's size).
    pub fn most(&self) -> u32 {
        self.offsets.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0)
    }
}

/// A chunk decoded where a console lands it from (P1c): a spare slot's `T8` layers, swapped into
/// the cache's slot ([`Present::land_baked`](crate::present::Present::land_baked)) so nothing is
/// copied on the game's thread, and its sidecar (`side`'s albedo and height stay empty).
#[derive(Debug)]
pub struct Landing {
    pub id: ChunkId,
    pub layers: ChunkLayers,
    pub side: Baked,
}

impl Default for Landing {
    fn default() -> Self {
        Landing::new()
    }
}

impl Landing {
    /// About 129 KB: a slot's layers.
    pub fn new() -> Landing {
        Landing { id: ChunkId { cx: 0, cy: 0 }, layers: ChunkLayers::new_t8(), side: Baked::default() }
    }
}

/// What a console's presenter knows of its baked county (P1c): the pack's chunk grid, the chunks
/// the painter must paint after all, and what it asks the loader for.
#[derive(Debug)]
pub struct Ground {
    pub cols: u16,
    pub rows: u16,
    /// A bit a chunk: its tiles or spans are not the blueprint's now (within a job snapshot's
    /// reach), so the bake no longer shows it; read at `read_at`'s flags generation.
    dirty: Vec<u32>,
    read_at: Option<u64>,
    /// A bit a chunk: the loader could not give it (a bad read or blob); the painter's for good.
    failed: Vec<u32>,
    /// The chunks to land, most wanted first, and those to read ahead (not landed yet).
    pub wants: Vec<ChunkId>,
    pub ahead: Vec<ChunkId>,
}

/// How far a changed cell reaches into the chunks round it: a job's snapshot margin, past the
/// painter's own reach (`terrain::SNAP_MARGIN`).
const DIRTY_REACH: i32 = 8;

impl Ground {
    pub fn new(cols: u16, rows: u16) -> Ground {
        let words = (usize::from(cols) * usize::from(rows)).div_ceil(32);
        Ground {
            cols,
            rows,
            dirty: alloc::vec![0; words],
            read_at: None,
            failed: alloc::vec![0; words],
            wants: Vec::with_capacity(16),
            ahead: Vec::with_capacity(8),
        }
    }

    fn bit(&self, id: ChunkId) -> Option<usize> {
        (id.cx < self.cols && id.cy < self.rows)
            .then(|| usize::from(id.cy) * usize::from(self.cols) + usize::from(id.cx))
    }

    /// Whether the bake shows chunk `id` as the painter would now.
    pub fn holds(&self, id: ChunkId) -> bool {
        self.bit(id).is_some_and(|i| (self.dirty[i / 32] | self.failed[i / 32]) >> (i % 32) & 1 == 0)
    }

    /// The loader could not give chunk `id`.
    pub fn fail(&mut self, id: ChunkId) {
        if let Some(i) = self.bit(id) {
            self.failed[i / 32] |= 1 << (i % 32);
        }
    }

    /// The changed cells and spans read again if the flags' generation `at` is new: each marks
    /// the chunks within [`DIRTY_REACH`] cells of it.
    pub fn read(
        &mut self,
        at: u64,
        cells: impl Iterator<Item = (i32, i32)>,
        spans: impl Iterator<Item = jane_core::Rect>,
    ) {
        if self.read_at == Some(at) {
            return;
        }
        self.read_at = Some(at);
        self.dirty.fill(0);
        let rects = cells.map(|(x, y)| jane_core::Rect::new(x, y, 1, 1)).chain(spans);
        for r in rects {
            let r = r.grow(DIRTY_REACH);
            let c0 = |v: i32| v.div_euclid(CHUNK_CELLS).max(0);
            let (cx0, cy0) = (c0(r.x), c0(r.y));
            let cx1 = (r.right() - 1).div_euclid(CHUNK_CELLS).min(i32::from(self.cols) - 1);
            let cy1 = (r.bottom() - 1).div_euclid(CHUNK_CELLS).min(i32::from(self.rows) - 1);
            for cy in cy0..=cy1 {
                for cx in cx0..=cx1 {
                    let i = cy as usize * usize::from(self.cols) + cx as usize;
                    self.dirty[i / 32] |= 1 << (i % 32);
                }
            }
        }
    }
}

/// What decodes a pack's blobs: the zstd decoder's state, made once.
pub struct Decoder {
    zstd: ruzstd::decoding::FrameDecoder,
    side: Vec<u8>,
}

impl core::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Decoder")
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        Decoder { zstd: ruzstd::decoding::FrameDecoder::new(), side: Vec::new() }
    }

    /// `frame` decoded by `codec` into `out`, which it must fill exactly.
    fn frame(&mut self, codec: Codec, frame: &[u8], out: &mut [u8]) -> Result<(), PackError> {
        let n = match codec {
            Codec::Raw => {
                if frame.len() != out.len() {
                    return Err(PackError("a raw frame's length"));
                }
                out.copy_from_slice(frame);
                out.len()
            }
            Codec::Zstd => self.zstd.decode_all(frame, out).map_err(|_| PackError("a zstd frame"))?,
            Codec::Lz4 => lz4_flex::block::decompress_into(frame, out).map_err(|_| PackError("an lz4 block"))?,
        };
        if n == out.len() { Ok(()) } else { Err(PackError("a frame's length")) }
    }

    /// Blob `blob` of a `codec` pack decoded into `out`.
    pub fn decode(&mut self, codec: Codec, blob: &[u8], out: &mut Baked) -> Result<(), PackError> {
        let mut r = Reader::new(blob);
        let (a, h, side_n) = (r.len()?, r.len()?, r.len()?);
        let (fa, fh) = (r.take(a)?, r.take(h)?);
        let fs = r.take(r.left())?;
        out.albedo.resize(PX, 0);
        out.height.resize(PX, 0);
        self.frame(codec, fa, &mut out.albedo)?;
        self.frame(codec, fh, &mut out.height)?;
        let mut side = core::mem::take(&mut self.side);
        side.clear();
        side.resize(side_n, 0);
        let r = self.frame(codec, fs, &mut side).and_then(|()| out.read_sidecar(&side));
        self.side = side;
        r
    }

    /// Blob `blob` of a `codec` pack decoded into `out`'s layers as [`Baked::lay`] lays it, and
    /// its sidecar into `out.side`; `out.id` is the caller's. No buffer beyond the slot's own:
    /// the albedo's bytes are decoded into the height layer, then packed into words.
    pub fn decode_landing(&mut self, codec: Codec, blob: &[u8], out: &mut Landing) -> Result<(), PackError> {
        let mut r = Reader::new(blob);
        let (a, h, side_n) = (r.len()?, r.len()?, r.len()?);
        let (fa, fh) = (r.take(a)?, r.take(h)?);
        let fs = r.take(r.left())?;
        let l = &mut out.layers;
        if !l.is_t8() || l.height.len() != PX || l.albedo.len() * 4 != PX {
            return Err(PackError("a landing's layers"));
        }
        self.frame(codec, fa, &mut l.height)?;
        for (w, b) in l.albedo.iter_mut().zip(l.height.chunks_exact(4)) {
            *w = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        self.frame(codec, fh, &mut l.height)?;
        let mut side = core::mem::take(&mut self.side);
        side.clear();
        side.resize(side_n, 0);
        let r = self.frame(codec, fs, &mut side).and_then(|()| out.side.read_sidecar(&side));
        self.side = side;
        r?;
        let s = &mut out.side;
        l.clut[..s.clut.len()].copy_from_slice(&s.clut);
        l.clut[s.clut.len()..].fill(0);
        l.clut_n = s.clut.len() as u16;
        l.glow.clear();
        l.glow.extend_from_slice(&s.glow);
        l.water.clear();
        l.water.extend_from_slice(&s.water);
        l.surface.fill(0);
        l.fence.fill(0);
        Ok(())
    }
}

/// The console's painter on its own, painting chunks of one zone as a PSP's worker does (a job
/// run and landed into a `T8` slot: `Present::from_tables_console`), for the bake.
#[derive(Debug)]
pub struct Baker {
    terrain: Terrain,
    layers: ChunkLayers,
}

impl Baker {
    /// A console's painter over `view`'s zone, as a New Game finds it.
    pub fn new(view: &View<'_>) -> Baker {
        let mut terrain = Terrain::with_flora(jane_art::terrain::Painter::new(), Vec::new(), 1);
        terrain.release_flora_px();
        terrain.drop_normals();
        terrain.stage_jobs();
        terrain.zone(view);
        terrain.read_spans(view);
        Baker { terrain, layers: ChunkLayers::new_t8() }
    }

    /// Chunk `id` painted and landed as a console's worker and game thread do it.
    pub fn paint(&mut self, view: &View<'_>, id: ChunkId) -> Baked {
        let mut job = self.terrain.job(view, id).expect("the painter is home between chunks");
        job.run();
        let (mut work, size) = job.into_parts();
        self.terrain.land(&mut work, size, id, 0, crate::frame::CLEAR, &mut self.layers);
        self.terrain.home_again(work);
        self.terrain.baked(0, &self.layers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Baked {
        let mut b = Baked {
            albedo: (0..PX).map(|k| (k % 7) as u8).collect(),
            clut: alloc::vec![0xff10_2030, 0xfe00_00ff, 0xff40_5060, 1, 2, 3, 4],
            height: (0..PX).map(|k| (k / 300 % 5) as u8 + 1).collect(),
            glow: alloc::vec![(17, 0xffff_ee00)],
            water: alloc::vec![(1, 2, 3), (4, 5, 6)],
            placed: alloc::vec![Placed { sprite: 9, x: -3, y: 200, row: 12 }],
            windows: alloc::vec![Window { x: 40, y: 60, height: 20, colour: [1, 2, 3] }],
            blocks: alloc::vec![Block { x0: 1, y0: 2, x1: 3, y1: 4, height: 5, lo: 0, fence: true }],
            openings: alloc::vec![Opening { x: 3, y: 4, w: 5, h: 6, lit: true, upper: false }],
            faces: alloc::vec![Face { x: 7, y: 8, plaster: true, eave: true }],
            rails: alloc::vec![FencePart { x0: 1, y0: 1, x1: 9, y1: 2, lo: 3, hi: 5 }],
            falls: alloc::vec![(3, 4)],
            ..Baked::default()
        };
        b.ways[3] = 0x0f0f;
        b.bases[200] = 48;
        b
    }

    /// A pack of raw blobs reads back as written, each chunk by its coordinates; a damaged or
    /// short one is refused.
    #[test]
    fn a_raw_pack_reads_back_and_a_damaged_one_is_refused() {
        let a = sample();
        let mut b = sample();
        b.albedo[5] = 200;
        b.falls.clear();
        let blobs = [a.blob(<[u8]>::to_vec), b.blob(<[u8]>::to_vec)];
        let bytes = write(Codec::Raw, (2, 1), 0x1234, &blobs);
        let pack = Pack::open(&bytes).expect("the pack opens");
        assert_eq!((pack.codec, pack.cols, pack.rows, pack.digest), (Codec::Raw, 2, 1, 0x1234));
        let mut d = Decoder::new();
        let mut out = Baked::default();
        for (cx, want) in [(0, &a), (1, &b)] {
            d.decode(pack.codec, pack.blob(ChunkId { cx, cy: 0 }).unwrap(), &mut out).unwrap();
            assert_eq!(&out, want);
        }
        assert!(pack.blob(ChunkId { cx: 2, cy: 0 }).is_none());
        assert!(Pack::open(&bytes[..bytes.len() - 1]).is_err(), "a short pack");
        let mut bad = bytes.clone();
        bad[4] ^= 1;
        assert!(Pack::open(&bad).is_err(), "another version");
        // A blob whose sidecar is cut short.
        let mut cut = blobs[0].clone();
        cut.pop();
        assert!(d.decode(Codec::Raw, &cut, &mut out).is_err());
    }

    /// A baked chunk laid into a slot is the `T8` layers it was taken from.
    #[test]
    fn a_baked_chunk_lays_as_its_layers() {
        let b = sample();
        let mut l = ChunkLayers::new_t8();
        b.lay(&mut l);
        assert_eq!(l.clut_n, 7);
        for k in [0, 1, 2, 3, 4, 999, PX - 1] {
            assert_eq!(l.t8_abgr(k), b.clut[usize::from(b.albedo[k])]);
        }
        assert_eq!(l.height, b.height);
    }
}
