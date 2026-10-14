//! What stands in while the art is missing (PORT.md §7.1). Delete this module when the looks
//! land: the chunk painter (`jane_art::terrain::paint_chunk`, ART.md §8 step 3) replaces
//! [`paint_chunk`], and the person, creature and prop looks replace [`StandIns`].
//!
//! - A chunk draws each cell as its tile's flat swatch (PRESENTATION.md §1.6's fallback). For the
//!   lit tiers it also gets a relief the light can find: walls rise as faces to the south, roofs
//!   are pitched east-west, a wood's canopy is lumpy and tall, hedges and fences stand.
//! - A unit whose sprite has no look yet (`people` draws those that do: Jane and the townsfolk
//!   from ART.md §8 step 2) draws the step-1 demo `ball`, its plum cloth swapped per kind (her, another seat,
//!   four folk variants, hostile, the dead): a coat swap is an index remap (ART.md §1). Its
//!   height stands it up: each px as high as it is above her feet, as a person's will be.
//! - A prop draws the demo sprite nearest its size, at 1x or half size; a prop with a light
//!   draws the `lamp`.
//!
//! All of it is drawn by `jane-present`, never by `jane-art`. [`tile_rgb`] outlives the rest:
//! `jane view` and `jane play --snap` colour their maps with it, so it moves when this goes.

use alloc::vec::Vec;
use jane_art::canvas::normal;
use jane_art::demo;
use jane_art::palette::{Ix, Ramp};
use jane_core::Tile;

use crate::atlas::{Atlas, RefId, Texel};
use crate::frame::{CELL, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers, height_of_rows};

/// A tile's flat swatch (the colours `jane view` has drawn the county in since P2).
pub fn tile_rgb(t: Tile) -> [u8; 3] {
    use jane_core::Tile as T;
    match t {
        T::Void => [0, 0, 0],
        T::Grass => [118, 164, 84],
        T::GrassTall => [92, 140, 66],
        T::Dirt => [150, 122, 84],
        T::Road => [96, 92, 90],
        T::Water => [52, 96, 170],
        T::Sand => [206, 190, 140],
        T::Bush => [60, 110, 50],
        T::Tree => [34, 84, 40],
        T::Fence => [120, 84, 52],
        T::HouseWall => [150, 110, 90],
        T::HouseRoof | T::Eaves => [130, 60, 50],
        T::Floor | T::FloorWood => [170, 140, 100],
        T::Wall | T::WallTop | T::StoneWall => [110, 106, 100],
        T::Moss => [104, 140, 80],
        T::DryBed => [160, 148, 110],
        T::Garden => [120, 100, 60],
        T::Rubble => [120, 110, 100],
        T::Track => [70, 56, 44],
        T::GrownPath => [130, 140, 90],
        T::Cobble => [150, 146, 136],
        T::Rail => [40, 40, 44],
        T::Cliff => [96, 90, 84],
        T::Hedge => [46, 96, 46],
        T::Boardwalk => [176, 130, 70],
        T::DeadTree => [90, 80, 60],
        T::Glass => [170, 200, 220],
        T::FlowerBed => [200, 120, 160],
        T::Stepping => [140, 140, 150],
        T::Crops => [190, 170, 80],
        T::Ice => [210, 230, 240],
        T::CaveFloor => [120, 108, 92],
        T::CaveWall => [52, 46, 44],
        T::TempleFloor => [176, 170, 156],
        T::TempleWall => [70, 66, 80],
        // The rest are dungeon tiles: walls dark, water blue, floors a stone grey.
        _ => {
            let f = t.flags();
            if f & jane_core::tile::F_WATER != 0 {
                [60, 100, 170]
            } else if f & jane_core::tile::F_SOLID != 0 {
                [64, 58, 62]
            } else {
                [140, 132, 118]
            }
        }
    }
}

/// Paints chunk `id` as flat swatches into `layers` (`CHUNK_PX` square), reading the ground
/// through `tile(cx, cy)`. Cells outside the zone (`w x h` cells) take `outside`. The albedo is
/// the same at every tier; where the layers carry them (T1 and T2), the normal and height get
/// the stand-in relief ([`relief`]).
pub fn paint_chunk(
    id: ChunkId,
    (w, h): (u32, u32),
    outside: u32,
    tile: impl Fn(i32, i32) -> Tile,
    layers: &mut ChunkLayers,
) {
    let side = CHUNK_PX as usize;
    debug_assert!(layers.is_t8() || layers.albedo.len() == side * side);
    if layers.is_t8() {
        layers.clut_n = 0;
    }
    let (lit, heights) = (layers.lit(), layers.has_height());
    let inside = |cx: i32, cy: i32| cx >= 0 && cy >= 0 && cx < w as i32 && cy < h as i32;
    for j in 0..CHUNK_CELLS {
        for i in 0..CHUNK_CELLS {
            let (cx, cy) = (i32::from(id.cx) * CHUNK_CELLS + i, i32::from(id.cy) * CHUNK_CELLS + j);
            let c = if inside(cx, cy) {
                let [r, g, b] = tile_rgb(tile(cx, cy));
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            } else {
                outside
            };
            let x0 = (i * CELL) as usize;
            for y in 0..CELL as usize {
                let row = (j * CELL) as usize + y;
                layers.fill_albedo(row * side + x0, CELL as usize, c);
            }
            if !heights {
                continue;
            }
            let t = |dx: i32, dy: i32| if inside(cx + dx, cy + dy) { tile(cx + dx, cy + dy) } else { Tile::Void };
            for y in 0..CELL {
                for x in 0..CELL {
                    let k = ((j * CELL + y) as usize) * side + (i * CELL + x) as usize;
                    let (n, z) = if inside(cx, cy) { relief(&t, (cx, cy), x, y) } else { ([128, 128], 0) };
                    layers.height[k] = z;
                    if lit {
                        layers.normal[k] = n;
                        layers.emissive[k] = 0;
                    }
                }
            }
        }
    }
}

/// [`paint_chunk`], softened for a console's `T8` chunk (PORT.md §13.13, the real PSP: a painter
/// that falls behind a walk showed squares of flat colour): each cell's colour runs into its
/// neighbours' over six px by an ordered dither (no colour a chunk did not have), and a sparse
/// speckle a shade darker breaks the flat. Heights as [`paint_chunk`]'s. Any other layer is
/// painted as [`paint_chunk`] paints it.
pub fn paint_chunk_soft(
    id: ChunkId,
    cells: (u32, u32),
    outside: u32,
    tile: impl Fn(i32, i32) -> Tile,
    layers: &mut ChunkLayers,
) {
    // The cells across a chunk and a ring round it; a 4 x 4 ordered dither's thresholds (0..16);
    // the px from an edge a neighbour's colour runs over.
    const N: usize = RING;
    paint_chunk(id, cells, outside, &tile, layers);
    if !layers.is_t8() {
        return;
    }
    // The cells' colours and a ring of their neighbours, as CLUT entries, and each a shade darker.
    let inside = |cx: i32, cy: i32| cx >= 0 && cy >= 0 && cx < cells.0 as i32 && cy < cells.1 as i32;
    let mut ix = [0u8; N * N];
    let mut dark = [0u8; N * N];
    for j in 0..N as i32 {
        for i in 0..N as i32 {
            let (cx, cy) = (i32::from(id.cx) * CHUNK_CELLS + i - 1, i32::from(id.cy) * CHUNK_CELLS + j - 1);
            let c = if inside(cx, cy) {
                let [r, g, b] = tile_rgb(tile(cx, cy));
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            } else {
                outside
            };
            let d = c & 0xff00_0000 | (0..3).fold(0, |a, s| a | (((c >> (8 * s)) & 0xff) * 7 / 8) << (8 * s));
            let k = j as usize * N + i as usize;
            ix[k] = layers.t8_index(c);
            dark[k] = layers.t8_index(d);
        }
    }
    let side = CHUNK_PX as usize;
    let cell = CELL as usize;
    for y in 0..side {
        let (j, ly) = (y / cell + 1, y % cell);
        for x in 0..side {
            let (i, lx) = (x / cell + 1, x % cell);
            let b = BAYER[(y & 3) * 4 + (x & 3)];
            // The share toward the neighbour across the nearer edge: 8 of 16 at it, none at RUN.
            let share = |d: usize| if d < RUN { ((RUN - d) * 8 / RUN) as u8 } else { 0 };
            let mut k = j * N + i;
            if lx < RUN && b < share(lx) {
                k -= 1;
            } else if lx >= cell - RUN && b < share(cell - 1 - lx) {
                k += 1;
            }
            if ly < RUN && BAYER[(x & 3) * 4 + (y & 3)] < share(ly) {
                k -= N;
            } else if ly >= cell - RUN && BAYER[(x & 3) * 4 + (y & 3)] < share(cell - 1 - ly) {
                k += N;
            }
            // A speckle one px in about twenty, placed by a hash of the px (the same each paint).
            let h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77);
            let e = if h.wrapping_mul(0x2545_f491) >> 27 == 0 { dark[k] } else { ix[k] };
            layers.t8_set(y * side + x, e);
        }
    }
}

/// The cells across a chunk and a ring round it (the soft swatches' neighbour tables).
const RING: usize = CHUNK_CELLS as usize + 2;
/// A 4 x 4 ordered dither's thresholds (0..16), and the px from a cell's edge a neighbour's colour
/// runs over (the soft swatches').
const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const RUN: usize = 6;

/// For each px of a cell (`ly * CELL + lx`), the step in the [`RING`] square from the cell's own
/// entry to the one whose colour the soft dither shows there (-19..=19: a neighbour across the
/// nearer edge, either axis or both). `CELL` is a multiple of 4, so the dither's phase is the
/// px's in its cell, and one table serves every cell of every chunk.
const SOFT_STEP: [i8; (CELL * CELL) as usize] = {
    const fn share(d: usize) -> u8 {
        if d < RUN { ((RUN - d) * 8 / RUN) as u8 } else { 0 }
    }
    let cell = CELL as usize;
    let mut t = [0i8; (CELL * CELL) as usize];
    let mut ly = 0;
    while ly < cell {
        let mut lx = 0;
        while lx < cell {
            let bx = BAYER[(ly & 3) * 4 + (lx & 3)];
            let by = BAYER[(lx & 3) * 4 + (ly & 3)];
            let mut d = 0i32;
            if lx < RUN && bx < share(lx) {
                d -= 1;
            } else if lx >= cell - RUN && bx < share(cell - 1 - lx) {
                d += 1;
            }
            if ly < RUN && by < share(ly) {
                d -= RING as i32;
            } else if ly >= cell - RUN && by < share(cell - 1 - ly) {
                d += RING as i32;
            }
            t[ly * cell + lx] = d as i8;
            lx += 1;
        }
        ly += 1;
    }
    t
};

/// `x * K` for the speckle's hash, per column.
const SPECKLE_X: [u32; CHUNK_PX as usize] = {
    let mut t = [0u32; CHUNK_PX as usize];
    let mut x = 0;
    while x < t.len() {
        t[x] = (x as u32).wrapping_mul(0x9e37_79b1);
        x += 1;
    }
    t
};

/// [`paint_chunk_soft`] for a console's `T8` chunk, without the relief: what stands in while the
/// painter is behind (PLAN-psp-v041 B). The albedo is the soft swatches' to the byte (the same
/// CLUT, the same dither, the same speckle); the height is all 0 and the chunk stands nothing,
/// so it costs a few ms on a PSP where the relief and the blocks over it cost 105 ms and put a
/// dome on every tree. The caller clears the blocks ([`crate::terrain::Terrain::stand_flat`]).
pub fn paint_chunk_flat_t8(
    id: ChunkId,
    cells: (u32, u32),
    outside: u32,
    tile: impl Fn(i32, i32) -> Tile,
    layers: &mut ChunkLayers,
) {
    debug_assert!(layers.is_t8());
    let inside = |cx: i32, cy: i32| cx >= 0 && cy >= 0 && cx < cells.0 as i32 && cy < cells.1 as i32;
    // Each cell's colour, with a ring of neighbours', one tile lookup apiece.
    let mut argb = [0u32; RING * RING];
    for j in 0..RING {
        for i in 0..RING {
            let (cx, cy) = (id.cx as i32 * CHUNK_CELLS + i as i32 - 1, id.cy as i32 * CHUNK_CELLS + j as i32 - 1);
            argb[j * RING + i] = if inside(cx, cy) {
                let [r, g, b] = tile_rgb(tile(cx, cy));
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            } else {
                outside
            };
        }
    }
    // The CLUT in the order the soft pass leaves it: the chunk's own cells first (the flat pass),
    // then each cell of the ring with its darker twin.
    layers.clut_n = 0;
    for j in 1..RING - 1 {
        for i in 1..RING - 1 {
            layers.t8_index(argb[j * RING + i]);
        }
    }
    let mut ix = [0u8; RING * RING];
    let mut dark = [0u8; RING * RING];
    for (k, &c) in argb.iter().enumerate() {
        let d = c & 0xff00_0000 | (0..3).fold(0, |a, s| a | (((c >> (8 * s)) & 0xff) * 7 / 8) << (8 * s));
        ix[k] = layers.t8_index(c);
        dark[k] = layers.t8_index(d);
    }
    // Four px a word. A px takes its cell's neighbour's colour by `SOFT_STEP`, and a darker shade
    // for the speckle, one px in about thirty-two, by a hash of the px.
    let (side, cell) = (CHUNK_PX as usize, CELL as usize);
    for y in 0..side {
        let (j, ly) = (y / cell + 1, y % cell);
        let yh = (y as u32).wrapping_mul(0x85eb_ca77);
        let row = &mut layers.albedo[y * side / 4..(y + 1) * side / 4];
        for (w, out) in row.iter_mut().enumerate() {
            let x = w * 4;
            let (i, lx) = (x / cell + 1, x % cell);
            let base = j * RING + i;
            let steps = &SOFT_STEP[ly * cell + lx..ly * cell + lx + 4];
            let mut word = 0u32;
            for q in 0..4 {
                let k = (base as isize + steps[q] as isize) as usize;
                let h = SPECKLE_X[x + q] ^ yh;
                let e = if h.wrapping_mul(0x2545_f491) >> 27 == 0 { dark[k] } else { ix[k] };
                word |= u32::from(e) << (8 * q);
            }
            *out = word;
        }
    }
    layers.height.fill(0);
    layers.water.clear();
    layers.glow.clear();
}

/// Cells the painter has painted, a few of each tile, that a console's stand-in draws its own
/// cells with while the painter is behind (the real PSP, v0.5.0: a run outpaced it, about one
/// chunk a second there against three, and the ground went to flat blocks of colour). Taken from
/// chunks as they land in the zone, so they are its ground as it looks there and then; each is a
/// cell whose eight neighbours are its own tile, so it holds the tile's texture and no edge.
/// [`paint_chunk_sampled_t8`] tiles them; a tile with none yet keeps its flat swatch.
#[derive(Debug)]
pub struct Samples {
    /// [`SAMPLES`] cells of `CELL x CELL` px each, each px an entry of its sample's palette.
    px: Vec<u8>,
    /// Each sample's palette: the [`PAL`] colours its cell showed most, as their chunk's CLUT held
    /// them (`0xAABBGGRR`, the alpha the water and rain tags).
    pal: Vec<[u32; PAL]>,
    /// Each sample's tile (`Tile::id`), or [`FREE`].
    owner: Vec<u8>,
    /// Each sample's neighbours of its own tile, of eight: 8 is the tile's texture alone, fewer
    /// has its edges in it (a road a cell or two wide never has 8).
    score: Vec<u8>,
    /// Each tile's samples, by id, [`FREE`] past the last.
    of: Vec<[u8; PER_TILE]>,
    /// Each tile's next sample to replace once it has [`PER_TILE`].
    turn: Vec<u8>,
}

/// Cells a [`Samples`] holds (about 33 KB in all).
pub const SAMPLES: usize = 96;
/// Colours a sample keeps: a chunk's CLUT holds 256, and a stand-in may wear a few dozen samples.
/// Past that a colour takes the CLUT's nearest, a search a PSP spent 70 ms a chunk on when each
/// sample brought all its own colours.
const PAL: usize = 16;
/// Cells it keeps of one tile, to pick among so a field of one tile is not one cell repeated.
const PER_TILE: usize = 4;
/// A cell is sampled with at least this many of its eight neighbours its own tile: a fence or a
/// path a cell wide has two.
const MIN_ALIKE: u8 = 2;
/// No sample, no tile (an id no tile has: `Tile`'s run from 0 and are far fewer).
const FREE: u8 = u8::MAX;
const CELL_AREA: usize = (CELL * CELL) as usize;

impl Default for Samples {
    fn default() -> Samples {
        Samples::new()
    }
}

impl Samples {
    pub fn new() -> Samples {
        Samples {
            px: alloc::vec![0; SAMPLES * CELL_AREA],
            pal: alloc::vec![[0; PAL]; SAMPLES],
            owner: alloc::vec![FREE; SAMPLES],
            score: alloc::vec![0; SAMPLES],
            of: alloc::vec![[FREE; PER_TILE]; 256],
            turn: alloc::vec![0; 256],
        }
    }

    /// Forgets them all (a new zone, or its chunks painted again for the light).
    pub fn clear(&mut self) {
        self.owner.fill(FREE);
        self.of.fill([FREE; PER_TILE]);
        self.turn.fill(0);
    }

    /// Samples held now.
    pub fn held(&self) -> usize {
        self.owner.iter().filter(|&&o| o != FREE).count()
    }

    fn count(&self, t: u8) -> usize {
        self.of[usize::from(t)].iter().take_while(|&&s| s != FREE).count()
    }

    /// The sample a new cell of tile `t` with `score` neighbours alike is written into: a free
    /// one, else, once it has [`PER_TILE`], its own worst (in turn among equals) if that is no
    /// better, else the last of the tile with the most. `None`: it keeps what it has.
    fn slot_for(&mut self, t: u8, score: u8) -> Option<usize> {
        let ti = usize::from(t);
        if self.count(t) == PER_TILE {
            let turn = usize::from(self.turn[ti]);
            let k = (0..PER_TILE)
                .map(|k| (k + turn) % PER_TILE)
                .min_by_key(|&k| self.score[usize::from(self.of[ti][k])])
                .unwrap_or(0);
            let s = usize::from(self.of[ti][k]);
            if self.score[s] > score {
                return None;
            }
            self.turn[ti] = ((k + 1) % PER_TILE) as u8;
            return Some(s);
        }
        let s = match self.owner.iter().position(|&o| o == FREE) {
            Some(s) => s,
            None => {
                let rich = (0..=255u8).max_by_key(|&u| (self.count(u), u)).unwrap_or(0);
                let m = self.count(rich);
                let s = self.of[usize::from(rich)][m - 1];
                self.of[usize::from(rich)][m - 1] = FREE;
                usize::from(s)
            }
        };
        let n = self.count(t);
        self.owner[s] = t;
        self.of[ti][n] = s as u8;
        Some(s)
    }

    /// From a chunk the painter has just laid into `layers` (a console's `T8` one): for each tile
    /// in it, the cell with the most of its eight neighbours the same tile (at least
    /// [`MIN_ALIKE`]), the first found from a place the chunk picks, so the cells kept move
    /// about. One new cell a tile a chunk, kept if it is no worse than the tile's worst.
    pub fn harvest(&mut self, id: ChunkId, cells: (u32, u32), tile: impl Fn(i32, i32) -> Tile, layers: &ChunkLayers) {
        if !layers.is_t8() {
            return;
        }
        let inside = |cx: i32, cy: i32| cx >= 0 && cy >= 0 && cx < cells.0 as i32 && cy < cells.1 as i32;
        let mut ring = [FREE; RING * RING];
        for j in 0..RING {
            for i in 0..RING {
                let (cx, cy) =
                    (i32::from(id.cx) * CHUNK_CELLS + i as i32 - 1, i32::from(id.cy) * CHUNK_CELLS + j as i32 - 1);
                if inside(cx, cy) {
                    ring[j * RING + i] = tile(cx, cy).id();
                }
            }
        }
        // Each tile's best cell: (neighbours alike + 1, cell, its colours), 0 for none. Of the
        // cells as alike, the calmest (the fewest colours: a flower or a stone in a sample shows
        // as a grid when it is tiled).
        let mut best = [(0u8, 0u8, 0u16); 256];
        let colours = |c: usize| {
            let (x0, y0, side) = (
                (c % CHUNK_CELLS as usize) * CELL as usize,
                (c / CHUNK_CELLS as usize) * CELL as usize,
                CHUNK_PX as usize,
            );
            let mut seen = [0u64; 4];
            for p in 0..CELL_AREA {
                let k = (y0 + p / CELL as usize) * side + x0 + p % CELL as usize;
                let ix = (layers.albedo[k / 4] >> ((k % 4) * 8)) as u8;
                seen[usize::from(ix >> 6)] |= 1 << (ix & 63);
            }
            seen.iter().map(|w| w.count_ones() as u16).sum::<u16>()
        };
        let n = (CHUNK_CELLS * CHUNK_CELLS) as usize;
        let start = (u32::from(id.cx).wrapping_mul(0x9e37_79b1) ^ u32::from(id.cy).wrapping_mul(0x85eb_ca77)) >> 24;
        for c in 0..n {
            // Every cell once, from `start`, by a stride prime to 256.
            let c = (start as usize + c * 37) % n;
            let (i, j) = (c % CHUNK_CELLS as usize + 1, c / CHUNK_CELLS as usize + 1);
            let me = ring[j * RING + i];
            if me == FREE {
                continue;
            }
            let alike = (0..9).filter(|&d| d != 4 && ring[(j + d / 3 - 1) * RING + i + d % 3 - 1] == me).count() as u8;
            let b = &mut best[usize::from(me)];
            if alike < MIN_ALIKE || alike + 1 < b.0 {
                continue;
            }
            let n = colours(c);
            if alike + 1 > b.0 || n < b.2 {
                *b = (alike + 1, c as u8, n);
            }
        }
        for (t, &(score, c, _)) in best.iter().enumerate().filter(|b| b.1.0 > 0) {
            if let Some(s) = self.slot_for(t as u8, score - 1) {
                self.score[s] = score - 1;
                let (i, j) = (usize::from(c) % CHUNK_CELLS as usize, usize::from(c) / CHUNK_CELLS as usize);
                self.take(s, layers, i * CELL as usize, j * CELL as usize);
            }
        }
    }

    /// The cell at chunk px `(x0, y0)` of `layers` as sample `s`: its [`PAL`] commonest colours,
    /// and each px as the nearest of them (its own, mostly).
    fn take(&mut self, s: usize, layers: &ChunkLayers, x0: usize, y0: usize) {
        let (side, cell) = (CHUNK_PX as usize, CELL as usize);
        // Its colours as CLUT entries (a cell shows a few dozen at most), and how often.
        let (mut seen, mut n) = ([(0u8, 0u16); CELL_AREA], 0);
        let mut which = [0u8; CELL_AREA];
        for (p, w) in which.iter_mut().enumerate() {
            let k = (y0 + p / cell) * side + x0 + p % cell;
            let ix = (layers.albedo[k / 4] >> ((k % 4) * 8)) as u8;
            let at = seen[..n].iter().position(|e| e.0 == ix).unwrap_or_else(|| {
                seen[n] = (ix, 0);
                n += 1;
                n - 1
            });
            seen[at].1 += 1;
            *w = at as u8;
        }
        // The commonest first; the palette is them, padded with the first.
        let mut order = [0u8; CELL_AREA];
        for (k, o) in order.iter_mut().enumerate().take(n) {
            *o = k as u8;
        }
        order[..n].sort_unstable_by_key(|&k| (core::cmp::Reverse(seen[usize::from(k)].1), k));
        let colour = |k: u8| layers.clut[usize::from(seen[usize::from(k)].0)];
        let kept = n.min(PAL);
        for e in 0..PAL {
            self.pal[s][e] = colour(order[e.min(kept - 1)]);
        }
        // Each colour to its palette entry: its own, or the nearest kept.
        let mut to = [0u8; CELL_AREA];
        for (r, &k) in order[..n].iter().enumerate() {
            to[usize::from(k)] = if r < PAL {
                r as u8
            } else {
                let c = colour(k);
                (0..kept).min_by_key(|&e| distance(self.pal[s][e], c)).unwrap_or(0) as u8
            };
        }
        for p in 0..CELL_AREA {
            self.px[s * CELL_AREA + p] = to[usize::from(which[p])];
        }
    }

    /// A sample of tile `t` for cell `(cx, cy)`: one of its own, picked by the cell's patch.
    fn pick(&self, t: Tile, cx: i32, cy: i32) -> u8 {
        let n = self.count(t.id());
        if n == 0 {
            return FREE;
        }
        // By patches of 8 x 8 cells: samples from other places differ in tone (the painter's
        // drifts of dry and lush grass), and picked cell by cell they made a checker.
        let (px, py) = (cx >> 3, cy >> 3);
        let h = (px as u32).wrapping_mul(0x9e37_79b1) ^ (py as u32).wrapping_mul(0x85eb_ca77);
        self.of[usize::from(t.id())][(h.wrapping_mul(0x2545_f491) >> 16) as usize % n]
    }
}

/// Tiles the painter draws as the ground round them with something standing on it (a tuft, a
/// bush, a tree; the standing comes with the painter's own paint): a stand-in may draw their
/// cells as that ground.
fn stands_on_ground(t: Tile) -> bool {
    matches!(t, Tile::GrassTall | Tile::Bush | Tile::Tree | Tile::DeadTree | Tile::Moss)
}

/// How far apart two tiles' flat swatches are.
fn rgb_distance(a: Tile, b: Tile) -> u32 {
    let (a, b) = (tile_rgb(a), tile_rgb(b));
    (0..3).map(|c| u32::from(a[c].abs_diff(b[c]))).sum()
}

/// Two tiles near enough in colour that one's cells may stand in for the other's (grass and
/// tall grass, a floor and a wooden one; not grass and earth, nor earth and road).
fn near(a: Tile, b: Tile) -> bool {
    rgb_distance(a, b) <= 80
}

/// How far apart two CLUT colours are: their channels', and far for a tag of another kind.
fn distance(a: u32, b: u32) -> u32 {
    let ch = |s: u32| ((a >> s) & 0xff).abs_diff((b >> s) & 0xff);
    ch(0) + ch(8) + ch(16) + if a >> 24 == b >> 24 { 0 } else { 1024 }
}

/// The samples a sampled stand-in wears at most, each in its own colours or a tile's.
const WORN: usize = 128;

/// [`paint_chunk_flat_t8`] with each cell that has a [`Samples`] cell of its tile drawn as that
/// cell (the painter's own texture for the tile, tiled), its edges run into its neighbours' by
/// the soft swatches' dither. A tile with none yet borrows another's: the ground round it that
/// it stands on, or a tile near its colour, or else the commonest sampled tile's texture in its
/// own colour (the first field of crops, the first pond). Returns the cells inside the zone left
/// flat, which only a chunk with no samples at all has. With no samples it paints what
/// [`paint_chunk_flat_t8`] paints.
pub fn paint_chunk_sampled_t8(
    id: ChunkId,
    cells: (u32, u32),
    outside: u32,
    tile: impl Fn(i32, i32) -> Tile,
    samples: &mut Samples,
    layers: &mut ChunkLayers,
) -> u16 {
    debug_assert!(layers.is_t8());
    let inside = |cx: i32, cy: i32| cx >= 0 && cy >= 0 && cx < cells.0 as i32 && cy < cells.1 as i32;
    // Each cell's colour and sample, with a ring of neighbours', one tile lookup apiece.
    let mut argb = [0u32; RING * RING];
    let mut src = [FREE; RING * RING];
    let mut tid = [FREE; RING * RING];
    // A cell's sample worn in its own tile's colour (its tile), else `FREE`.
    let mut tint = [FREE; RING * RING];
    // A cell's sample mirrored left to right (by the cell), so a field of one sample is no grid.
    let mut flip = [false; RING * RING];
    // The tile whose texture a tile with nothing near it wears: the most sampled.
    let donor =
        (0..=254u8).filter(|&t| samples.count(t) > 0).max_by_key(|&t| (samples.count(t), core::cmp::Reverse(t)));
    let at =
        |i: usize, j: usize| (id.cx as i32 * CHUNK_CELLS + i as i32 - 1, id.cy as i32 * CHUNK_CELLS + j as i32 - 1);
    for j in 0..RING {
        for i in 0..RING {
            let (cx, cy) = at(i, j);
            let k = j * RING + i;
            argb[k] = if inside(cx, cy) {
                let t = tile(cx, cy);
                tid[k] = t.id();
                src[k] = samples.pick(t, cx, cy);
                flip[k] = ((cx as u32).wrapping_mul(0x2c1b_3c6d) ^ (cy as u32).wrapping_mul(0x297a_2d39)) >> 31 != 0;
                let [r, g, b] = tile_rgb(t);
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            } else {
                outside
            };
        }
    }
    // A cell of a tile not sampled yet borrows a sampled tile's cells: the commonest round it
    // that it may stand on (as the painter draws a lone tuft of wild ground, or a tree's foot,
    // as the ground round it), else the sampled tile nearest its colour, if near.
    let mut flat = 0u16;
    for j in 0..RING {
        for i in 0..RING {
            let k = j * RING + i;
            if src[k] != FREE || tid[k] == FREE {
                continue;
            }
            let Some(me) = Tile::from_id(tid[k]) else { continue };
            let mut best = (0u8, FREE);
            let mut votes = [(FREE, 0u8); 8];
            for dj in 0..3 {
                for di in 0..3 {
                    let (ii, jj) = ((i + di).wrapping_sub(1), (j + dj).wrapping_sub(1));
                    if ii >= RING || jj >= RING || (di, dj) == (1, 1) {
                        continue;
                    }
                    let t = tid[jj * RING + ii];
                    let Some(other) = Tile::from_id(t).filter(|&o| samples.count(o.id()) > 0) else { continue };
                    if !(stands_on_ground(me) || near(me, other)) {
                        continue;
                    }
                    let v = votes.iter().position(|v| v.0 == t || v.0 == FREE).unwrap_or(7);
                    votes[v] = (t, votes[v].1 + 1);
                    if votes[v].1 > best.0 {
                        best = (votes[v].1, t);
                    }
                }
            }
            let by = if best.1 == FREE {
                (0..=254u8)
                    .filter(|&t| samples.count(t) > 0)
                    .filter_map(Tile::from_id)
                    .filter(|&o| near(me, o))
                    .min_by_key(|&o| rgb_distance(me, o))
            } else {
                Tile::from_id(best.1)
            };
            let (cx, cy) = at(i, j);
            if let Some(by) = by {
                src[k] = samples.pick(by, cx, cy);
            } else if let Some(d) = donor.and_then(Tile::from_id) {
                src[k] = samples.pick(d, cx, cy);
                tint[k] = tid[k];
            }
            let ring = i == 0 || j == 0 || i == RING - 1 || j == RING - 1;
            flat += u16::from(src[k] == FREE && !ring);
        }
    }
    // The CLUT as the flat swatch begins it, then each sampled colour as it is met.
    layers.clut_n = 0;
    for j in 1..RING - 1 {
        for i in 1..RING - 1 {
            layers.t8_index(argb[j * RING + i]);
        }
    }
    let mut ix = [0u8; RING * RING];
    let mut dark = [0u8; RING * RING];
    for (k, &c) in argb.iter().enumerate() {
        let d = c & 0xff00_0000 | (0..3).fold(0, |a, s| a | (((c >> (8 * s)) & 0xff) * 7 / 8) << (8 * s));
        ix[k] = layers.t8_index(c);
        dark[k] = layers.t8_index(d);
    }
    // Each sample the chunk wears (in a tint or not), its palette as this chunk's CLUT indices,
    // once; a cell's `wear` is its entry. Past `WORN` the rest are flat.
    let mut worn = [(FREE, FREE, [0u8; PAL]); WORN];
    let (mut n_worn, mut wear) = (0, [0u8; RING * RING]);
    for k in 0..RING * RING {
        if src[k] == FREE {
            continue;
        }
        let key = (src[k], tint[k]);
        if let Some(w) = worn[..n_worn].iter().position(|w| (w.0, w.1) == key) {
            wear[k] = w as u8;
            continue;
        }
        if n_worn == WORN {
            src[k] = FREE;
            continue;
        }
        let sk = usize::from(src[k]);
        let mut map = [0u8; PAL];
        let lum = |c: u32| ((c & 0xff) * 77 + ((c >> 8) & 0xff) * 150 + ((c >> 16) & 0xff) * 29) >> 8;
        // Its mean light, which the tile's own colour takes; a px half as light again at most.
        let base = (samples.px[sk * CELL_AREA..(sk + 1) * CELL_AREA]
            .iter()
            .map(|&e| lum(samples.pal[sk][usize::from(e)]))
            .sum::<u32>()
            / CELL_AREA as u32)
            .max(1);
        for (e, m) in map.iter_mut().enumerate() {
            let c = samples.pal[sk][e];
            // To `0xAARRGGBB` for the CLUT's own lookup: as it is, or its light in the tile's colour.
            let argb = match Tile::from_id(tint[k]) {
                None => c & 0xff00_ff00 | (c >> 16) & 0xff | (c & 0xff) << 16,
                Some(t) => {
                    let (rgb, l) = (tile_rgb(t), lum(c).min(base * 3 / 2));
                    let ch = |v: u8| (u32::from(v) * l / base).min(255);
                    0xff00_0000 | ch(rgb[0]) << 16 | ch(rgb[1]) << 8 | ch(rgb[2])
                }
            };
            *m = layers.t8_index(argb);
        }
        worn[n_worn] = (src[k], tint[k], map);
        wear[k] = n_worn as u8;
        n_worn += 1;
    }
    let (side, cell) = (CHUNK_PX as usize, CELL as usize);
    for y in 0..side {
        let (j, ly) = (y / cell + 1, y % cell);
        let yh = (y as u32).wrapping_mul(0x85eb_ca77);
        for w in 0..side / 4 {
            let x = w * 4;
            let (i, lx) = (x / cell + 1, x % cell);
            let base = j * RING + i;
            let steps = &SOFT_STEP[ly * cell + lx..ly * cell + lx + 4];
            let mut word = 0u32;
            for q in 0..4 {
                let k = (base as isize + steps[q] as isize) as usize;
                let e = if src[k] == FREE {
                    let h = SPECKLE_X[x + q] ^ yh;
                    if h.wrapping_mul(0x2545_f491) >> 27 == 0 { dark[k] } else { ix[k] }
                } else {
                    let sk = usize::from(src[k]);
                    let col = if flip[k] { cell - 1 - (lx + q) } else { lx + q };
                    worn[usize::from(wear[k])].2[usize::from(samples.px[sk * CELL_AREA + ly * cell + col])]
                };
                word |= u32::from(e) << (8 * q);
            }
            layers.albedo[y * side / 4 + w] = word;
        }
    }
    layers.height.fill(0);
    layers.water.clear();
    layers.glow.clear();
    flat
}

/// How tall a wall's face stands, rows: three cells (its height is `height_of_rows` of that).
const WALL: i32 = 48;

/// Stands up walls and faces.
fn wall_like(t: Tile) -> bool {
    use jane_core::Tile as T;
    matches!(t, T::HouseWall | T::Wall | T::WallTop | T::StoneWall | T::TempleWall | T::CaveWall | T::Cliff)
}

/// The stand-in relief of pixel `(x, y)` of cell `(cx, cy)`, where `t(dx, dy)` is the tile `dx, dy`
/// cells away (`t(0, 0)` the cell's own): `(normal, height px)`. Faces look south (the 3/4
/// view sees a wall's south face), a face's px as high as it stands above the wall's foot.
fn relief(t: &impl Fn(i32, i32) -> Tile, (cx, cy): (i32, i32), x: i32, y: i32) -> ([u8; 2], u8) {
    use jane_core::Tile as T;
    let here = t(0, 0);
    let lump = |top: i32, lo: i32| {
        // A dome over each 2 x 2 cells: the canopy lumps, a hedge's rounded top.
        let (bx, by) = ((cx & 1) * CELL + x - CELL, (cy & 1) * CELL + y - CELL);
        let d2 = bx * bx + by * by;
        let z = (top - (top - lo) * d2 / (CELL * CELL)).clamp(lo, top);
        (normal(bx * 127 / 18, by * 127 / 18), z as u8)
    };
    match here {
        _ if wall_like(here) => {
            // A wall is a face rising from the row it stands on: each px as high as it is above
            // that row (true px, `height_of_rows`), up to WALL rows; past that, the wall's
            // thickness seen from above, flat.
            let below = (1..=8).take_while(|&k| wall_like(t(0, k))).count() as i32;
            let z = below * CELL + (CELL - y);
            if z > WALL {
                (normal(0, -12), height_of_rows(WALL) as u8)
            } else {
                (normal(0, 96), height_of_rows(z).max(1) as u8)
            }
        }
        T::HouseRoof | T::Eaves => {
            // The run of roof in this column: pitched along it, the ridge in the middle.
            let up = (1..=12).take_while(|&k| t(0, -k).is_roof()).count() as i32;
            let down = (1..=12).take_while(|&k| t(0, k).is_roof()).count() as i32;
            let len = (up + down + 1) * CELL;
            let r = up * CELL + y;
            let mid = len / 2;
            let off = (r - mid) * 127 / mid.max(1);
            // The eaves sit on the wall's face under the roof, the ridge 14 px above them.
            let wall = (1..=3).take_while(|&k| wall_like(t(0, down + k))).count() as i32;
            let eaves = height_of_rows((wall * CELL).clamp(12, WALL));
            let rise = 14 - 14 * (r - mid).abs() / mid.max(1);
            (normal(0, off.clamp(-80, 80)), (eaves + rise) as u8)
        }
        T::Tree => lump(44, 30),
        T::DeadTree => lump(30, 20),
        T::Bush | T::Hedge => lump(14, 6),
        T::Fence => {
            if y >= 6 {
                (normal(0, 90), (CELL - y + 4).clamp(1, 14) as u8)
            } else {
                ([128, 128], 0)
            }
        }
        _ => ([128, 128], 0),
    }
}

/// Who a unit is, as far as its stand-in shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Me,
    /// Another seat.
    Seat,
    /// A friendly unit of no seat, by its `vary` variant (0..4).
    Folk(u8),
    Hostile,
    Dead,
}

/// The step-1 demo sprites a prop picks from, at 1x and half size: the ball is the units' and
/// the lamp the lit props'.
const PROP_DEMOS: [&str; 3] = ["panel", "sphere", "still"];

/// The unit's coat swap, the ball's plum cloth to this ramp, by [`UnitKind`]: her, a seat, the
/// four folk variants, hostile, dead.
const COATS: [Ramp; 8] = [
    Ramp::ClothPlum,
    Ramp::ClothBlue,
    Ramp::ClothMustard,
    Ramp::ClothGreen,
    Ramp::ClothBrown,
    Ramp::WoodPale,
    Ramp::ClothRed,
    Ramp::ClothGrey,
];

/// The stand-in sprites, packed into the atlas at boot.
#[derive(Debug)]
pub struct StandIns {
    /// The ball in each coat of [`COATS`].
    units: [RefId; 8],
    /// `(w, h, id)` of every prop candidate that is not a lamp, 1x and half.
    props: Vec<(i32, i32, RefId)>,
    /// The lamp at 1x and half.
    lamps: [(i32, i32, RefId); 2],
    /// The same lamps with their glass dark: a lamp the view says is out does not glow.
    dark: [RefId; 2],
    /// How high the lamps' glass glows above their foot, px: where their light shines from.
    glass: [u8; 2],
}

/// How high the middle of a canvas's glowing px stands above `foot`, px.
fn glow_height(c: &jane_art::Canvas, foot: i16) -> u8 {
    let rows: Vec<i32> = (0..c.h()).filter(|&y| (0..c.w()).any(|x| c.emissive_at(x, y) != Ix::CLEAR)).collect();
    let mid = if rows.is_empty() { i32::from(foot) / 2 } else { rows.iter().sum::<i32>() / rows.len() as i32 };
    (i32::from(foot) - mid).clamp(1, 255) as u8
}

/// The row a stand-in stands on: one below its lowest opaque px (the demo sprites float in
/// their boxes; a person's feet are her anchor).
fn foot_of(c: &jane_art::Canvas) -> i16 {
    (0..c.h()).rev().find(|&y| (0..c.w()).any(|x| c.get(x, y).is_opaque())).map_or(c.h(), |y| y + 1) as i16
}

/// A stand-in texel stood up: its height is how far it is above the foot row `foot` (a person
/// is as tall as she is drawn), at least 1 where it is opaque; the contact shadow and clear
/// keep 0.
fn standing(t: Texel, y: i32, foot: i16) -> Texel {
    if t.albedo.is_opaque() {
        Texel { height: (i32::from(foot) - y).clamp(1, 255) as u8, ..t }
    } else {
        Texel { height: 0, ..t }
    }
}

/// `ix` with ramp `from` swapped for `to`, tone for tone.
fn swap(ix: Ix, from: Ramp, to: Ramp) -> Ix {
    match Ramp::of(ix) {
        Some((r, tone)) if r == from => to.at(tone),
        _ => ix,
    }
}

impl StandIns {
    /// Packs every stand-in into `atlas`.
    pub fn build(atlas: &mut Atlas) -> StandIns {
        let ball = demo::ball();
        // Feet at h - 4, the contact shadow's middle (ART.md §1: units anchor by the feet).
        let feet = ((ball.w() / 2) as i16, (ball.h() - 4) as i16);
        let units = COATS.map(|coat| {
            atlas.add_canvas(&ball, feet, 40, |_, y, t| Texel {
                albedo: swap(t.albedo, Ramp::ClothPlum, coat),
                ..standing(t, y, feet.1)
            })
        });
        let mut props = Vec::new();
        // A prop's anchor is the row it stands on (its foot), from its box's top-left.
        let mut pack = |c: &jane_art::Canvas, glass: bool| {
            let (w, h, f) = (c.w(), c.h(), foot_of(c));
            let hf = f / 2;
            let lit = |t: Texel| if glass { t } else { Texel { emissive: Ix::CLEAR, ..t } };
            (
                (w, h, atlas.add_canvas(c, (0, f), f as u8, |_, y, t| lit(standing(t, y, f)))),
                (w / 2, h / 2, atlas.add_canvas_half(c, (0, hf), hf as u8, |_, y, t| lit(standing(t, y, hf)))),
            )
        };
        for name in PROP_DEMOS {
            let (full, half) = pack(&demo::sprite(name).expect("a demo sprite"), true);
            props.extend([full, half]);
        }
        let lamp = demo::lamp();
        let (full, half) = pack(&lamp, true);
        let (dark_full, dark_half) = pack(&lamp, false);
        let g = glow_height(&lamp, foot_of(&lamp));
        StandIns { units, props, lamps: [full, half], dark: [dark_full.2, dark_half.2], glass: [g, g / 2] }
    }

    /// A unit's stand-in.
    pub fn unit(&self, kind: UnitKind) -> RefId {
        self.units[match kind {
            UnitKind::Me => 0,
            UnitKind::Seat => 1,
            UnitKind::Folk(v) => 2 + usize::from(v % 4),
            UnitKind::Hostile => 6,
            UnitKind::Dead => 7,
        }]
    }

    /// How high a lamp stand-in's glass glows above its foot, px; `None` for any other.
    pub fn glass(&self, id: RefId) -> Option<u8> {
        (0..2).find(|&i| self.lamps[i].2 == id || self.dark[i] == id).map(|i| self.glass[i])
    }

    /// The stand-in drawn for `id` when its light is out: a lamp's dark twin, else itself.
    pub fn unlit(&self, id: RefId) -> RefId {
        self.lamps.iter().position(|l| l.2 == id).map_or(id, |i| self.dark[i])
    }

    /// A prop's stand-in: the demo nearest its box (the footprint, `w x h` cells, plus a cell
    /// of rise unless it is flat), or the lamp nearest it for a prop with a light.
    pub fn prop(&self, w_cells: u8, h_cells: u8, flat: bool, lit: bool) -> RefId {
        let want = (i32::from(w_cells) * CELL, i32::from(h_cells) * CELL + if flat { 0 } else { CELL });
        let near = |&&(w, h, _): &&(i32, i32, RefId)| (w - want.0).abs() + (h - want.1).abs();
        let pick = if lit { self.lamps.iter().min_by_key(near) } else { self.props.iter().min_by_key(near) };
        pick.expect("stand-ins are packed").2
    }
}

crate::tables::tab_struct!(StandIns { units, props, lamps, dark, glass });

#[cfg(test)]
mod tests {
    use super::*;

    /// A painted chunk as the painter might land it, as a `T8` chunk: each cell the same 16
    /// colours in 4 x 4 blocks (by the px's place in its cell), tinted by the chunk.
    fn landed(id: ChunkId) -> ChunkLayers {
        let mut l = ChunkLayers::new_t8();
        l.set_albedo(|k| {
            let (x, y) = (k as i32 % CHUNK_PX, k as i32 / CHUNK_PX);
            0xff00_0000
                | (((x % CELL / 4) as u32 * 64) << 16)
                | (((y % CELL / 4) as u32 * 64) << 8)
                | (u32::from(id.cx) * 7)
        });
        l
    }

    #[test]
    fn with_no_samples_the_sampled_stand_in_is_the_flat_one() {
        let road = |x: i32, _: i32| if x % 7 == 3 { Tile::Road } else { Tile::Grass };
        for (id, cells) in [(ChunkId { cx: 2, cy: 3 }, (200, 200)), (ChunkId { cx: 1, cy: 1 }, (20, 20))] {
            let (mut flat, mut sampled) = (ChunkLayers::new_t8(), ChunkLayers::new_t8());
            paint_chunk_flat_t8(id, cells, 0xff00_0001, road, &mut flat);
            let n = paint_chunk_sampled_t8(id, cells, 0xff00_0001, road, &mut Samples::new(), &mut sampled);
            assert_eq!(sampled.albedo, flat.albedo);
            assert_eq!(sampled.clut[..usize::from(flat.clut_n)], flat.clut[..usize::from(flat.clut_n)]);
            let inside = (0..256).filter(|c| {
                let (x, y) = (i32::from(id.cx) * 16 + c % 16, i32::from(id.cy) * 16 + c / 16);
                x < cells.0 as i32 && y < cells.1 as i32
            });
            assert_eq!(usize::from(n), inside.count(), "{id:?}: every cell inside is flat");
        }
    }

    #[test]
    fn a_landed_chunk_lends_its_cells_and_a_stand_in_wears_them() {
        // Grass with a road a cell wide (two of its neighbours road: enough), and in the chunk
        // stood in for, a pond and a field of sand, neither ever landed.
        let ground = |x: i32, y: i32| match (x, y) {
            (40, _) => Tile::Road,
            (33..=36, 50..=53) => Tile::Water,
            (_, 60..) => Tile::Sand,
            _ => Tile::Grass,
        };
        let (from, cells) = (ChunkId { cx: 2, cy: 2 }, (200, 200));
        let l = landed(from);
        let mut s = Samples::new();
        s.harvest(from, cells, ground, &l);
        assert_eq!(s.held(), 2, "a cell of grass and one of the road");
        // Another chunk of that ground, stood in for: the grass and the road are the landed
        // cells (in each cell's middle, past the edges' dither); nothing is flat.
        let to = ChunkId { cx: 2, cy: 3 };
        let mut out = ChunkLayers::new_t8();
        let n = paint_chunk_sampled_t8(to, cells, 0, ground, &mut s, &mut out);
        assert_eq!(n, 0, "no cell is flat");
        let px = |l: &ChunkLayers, x: i32, y: i32| l.t8_abgr((y * CHUNK_PX + x) as usize);
        for (x, y) in [(8, 8), (9, 7), (24 + 4 * 16, 8 + 7 * 16), (8 * 16 + 8, 8)] {
            let want = [px(&l, x % CELL, y % CELL), px(&l, CELL - 1 - x % CELL, y % CELL)];
            assert!(want.contains(&px(&out, x, y)), "({x}, {y}) is the sample's px (or mirrored)");
        }
        // The pond (cells 1..=4, rows 2..=5) wears the grass's texture in the water's colour:
        // bluer than green, and not one flat colour.
        let pond: Vec<u32> = (0..4).map(|d| px(&out, 2 * 16 + 4 + d * 3, 3 * 16 + 8)).collect();
        assert!(pond.iter().all(|&c| (c >> 16) & 0xff > (c >> 8) & 0xff), "{pond:x?}");
        assert!(pond.windows(2).any(|w| w[0] != w[1]), "{pond:x?}");
        // As the sand (rows 12 on) does in its own.
        let sand = px(&out, 5 * 16 + 8, 14 * 16 + 8);
        assert!(sand & 0xff > (sand >> 16) & 0xff, "{sand:x}: redder than blue");
    }

    #[test]
    fn samples_hold_a_few_of_each_tile_and_no_more_than_their_room() {
        let mut s = Samples::new();
        let cells = (4000, 4000);
        // Many chunks of one tile each, of more tiles than the room holds four of.
        for (n, &t) in Tile::ALL.iter().cycle().take(600).enumerate() {
            let id = ChunkId { cx: (n % 200) as u16, cy: (n / 200) as u16 };
            s.harvest(id, cells, |_, _| t, &landed(id));
            assert!(s.held() <= SAMPLES);
        }
        assert_eq!(s.held(), SAMPLES);
        assert!(Tile::ALL.iter().all(|&t| s.count(t.id()) <= PER_TILE));
        s.clear();
        assert_eq!(s.held(), 0);
    }

    #[test]
    fn a_chunk_is_its_tiles_swatches() {
        let mut layers = ChunkLayers::new(crate::Tier::T0);
        // A 20 x 20 zone of grass with one road cell at (17, 18): chunk (1, 1) holds it.
        let tile = |x: i32, y: i32| if (x, y) == (17, 18) { Tile::Road } else { Tile::Grass };
        paint_chunk(ChunkId { cx: 1, cy: 1 }, (20, 20), 0xff00_0001, tile, &mut layers);
        let px = &layers.albedo;
        let at = |x: i32, y: i32| px[(y * CHUNK_PX + x) as usize];
        let [r, g, b] = tile_rgb(Tile::Road);
        assert_eq!(at(16 + 5, 32 + 15), 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b));
        assert_ne!(at(5, 5), at(16 + 5, 32 + 15));
        // Cells past the zone's edge (x or y >= 20) are outside.
        assert_eq!(at(4 * 16, 0), 0xff00_0001);
        assert_eq!(at(0, 4 * 16), 0xff00_0001);
    }

    #[test]
    fn a_consoles_soft_swatches_blend_their_edges_with_their_own_colours() {
        // Grass with a road column at x = 20 (chunk 1's cell 4): soft and flat agree mid-cell,
        // the edge px take the neighbour's colour (or a speckle) and nothing else; heights match.
        let tile = |x: i32, _: i32| if x == 20 { Tile::Road } else { Tile::Grass };
        let id = ChunkId { cx: 1, cy: 0 };
        let (mut flat, mut soft) = (ChunkLayers::new_t8(), ChunkLayers::new_t8());
        paint_chunk(id, (40, 40), 0xff00_0001, tile, &mut flat);
        paint_chunk_soft(id, (40, 40), 0xff00_0001, tile, &mut soft);
        assert_eq!(flat.height, soft.height);
        let argb = |l: &ChunkLayers, x: usize, y: usize| {
            let k = y * CHUNK_PX as usize + x;
            let c = l.clut[((l.albedo[k / 4] >> ((k % 4) * 8)) & 0xff) as usize];
            c & 0xff00_ff00 | (c >> 16) & 0xff | (c & 0xff) << 16
        };
        let rgb = |t: Tile| {
            let [r, g, b] = tile_rgb(t);
            0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
        };
        let dark = |c: u32| c & 0xff00_0000 | (0..3).fold(0, |a, s| a | (((c >> (8 * s)) & 0xff) * 7 / 8) << (8 * s));
        let (road, grass) = (rgb(Tile::Road), rgb(Tile::Grass));
        // Row 0 runs into the zone's edge above it (outside).
        let allowed = [road, grass, dark(road), dark(grass), 0xff00_0001, dark(0xff00_0001)];
        let mut mixed = 0;
        for y in 0..CHUNK_PX as usize {
            for x in 0..CHUNK_PX as usize {
                let c = argb(&soft, x, y);
                assert!(allowed.contains(&c), "{x},{y}: {c:08x}");
                mixed += usize::from(x / 16 == 3 && c == road);
            }
            // Mid-column, under the zone's top row, the road's own colour or its speckle.
            assert!(y < 16 || [road, dark(road)].contains(&argb(&soft, 4 * 16 + 8, y)));
        }
        assert!(mixed > 0, "the road runs into the grass beside it");
        let mut again = ChunkLayers::new_t8();
        paint_chunk_soft(id, (40, 40), 0xff00_0001, tile, &mut again);
        assert_eq!(again.albedo, soft.albedo, "the same each paint");
    }

    type Ground<'a> = &'a dyn Fn(i32, i32) -> Tile;

    /// A county's worth of ground by hash: grass and road, a wood, walls and a roof, water.
    fn mixed(x: i32, y: i32) -> Tile {
        let h = (x as u32).wrapping_mul(0x9e37_79b1) ^ (y as u32).wrapping_mul(0x85eb_ca77);
        let h = h.wrapping_mul(0x2545_f491) >> 24;
        match (x, y) {
            (20..=27, 40..=44) => Tile::HouseWall,
            (20..=27, 36..=39) => Tile::HouseRoof,
            (_, 70..=72) => Tile::Road,
            _ => match h % 16 {
                0..=5 => Tile::Tree,
                6 | 7 => Tile::Bush,
                8 => Tile::Water,
                9 => Tile::Fence,
                10 => Tile::Dirt,
                _ => Tile::Grass,
            },
        }
    }

    #[test]
    fn the_flat_swatch_is_the_soft_swatchs_albedo_with_no_relief() {
        // County ground, a wood, a wall's edge and the zone's edge (cells past 20 x 20 are outside).
        let ground = |x: i32, _: i32| if x % 7 == 3 { Tile::Road } else { Tile::Grass };
        let wood = |x: i32, y: i32| if (x * 3 + y) % 5 == 0 { Tile::Grass } else { Tile::Tree };
        let cases: [(ChunkId, (u32, u32), Ground<'_>); 4] = [
            (ChunkId { cx: 2, cy: 3 }, (200, 200), &ground),
            (ChunkId { cx: 0, cy: 1 }, (200, 200), &wood),
            (ChunkId { cx: 1, cy: 2 }, (200, 200), &mixed),
            (ChunkId { cx: 1, cy: 1 }, (20, 20), &mixed),
        ];
        for (id, cells, tile) in cases {
            let (mut soft, mut flat) = (ChunkLayers::new_t8(), ChunkLayers::new_t8());
            // A slot reused: stale heights, water and glow from the chunk before.
            flat.height.fill(9);
            flat.water.push((1, 2, 3));
            flat.glow.push((1, 2));
            paint_chunk_soft(id, cells, 0xff00_0001, tile, &mut soft);
            paint_chunk_flat_t8(id, cells, 0xff00_0001, tile, &mut flat);
            assert_eq!(flat.albedo, soft.albedo, "{id:?}: the indices");
            assert_eq!(flat.clut_n, soft.clut_n, "{id:?}: the CLUT's length");
            let n = usize::from(soft.clut_n);
            assert_eq!(flat.clut[..n], soft.clut[..n], "{id:?}: the CLUT");
            assert!(flat.height.iter().all(|&z| z == 0), "{id:?}: flat");
            assert!(flat.water.is_empty() && flat.glow.is_empty());
        }
        // The relief the soft pass paints is real where there are trees and walls.
        let (mut soft, mut flat) = (ChunkLayers::new_t8(), ChunkLayers::new_t8());
        paint_chunk_soft(ChunkId { cx: 0, cy: 1 }, (200, 200), 0, wood, &mut soft);
        paint_chunk_flat_t8(ChunkId { cx: 0, cy: 1 }, (200, 200), 0, wood, &mut flat);
        assert!(soft.height.iter().any(|&z| z > 0));
        assert!(flat.height.iter().all(|&z| z == 0));
    }

    /// `cargo test -p jane-present --release stand_in_cost -- --ignored --nocapture`: the host's
    /// proxy for the PSP's 105 ms to a few ms.
    #[test]
    #[ignore = "a timing, printed"]
    fn stand_in_cost() {
        let id = ChunkId { cx: 1, cy: 2 };
        let mut l = ChunkLayers::new_t8();
        let time = |f: &mut dyn FnMut()| {
            f();
            let t = std::time::Instant::now();
            for _ in 0..40 {
                f();
            }
            t.elapsed().as_secs_f64() * 1000.0 / 40.0
        };
        // The old path: the soft swatches with their relief, and the blocks over the relief.
        let (mut field, mut runs, mut out) = (Vec::new(), Vec::new(), Vec::new());
        let old = time(&mut || {
            paint_chunk_soft(id, (200, 200), 0, mixed, &mut l);
            crate::terrain::blocks(&l.height, &[], &[], &[], &mut field, &mut runs, &mut out);
            core::hint::black_box((&l, &out));
        });
        let new = time(&mut || {
            paint_chunk_flat_t8(id, (200, 200), 0, mixed, &mut l);
            core::hint::black_box(&l);
        });
        std::println!("soft {old:.3} ms, flat {new:.3} ms, {:.1}x", old / new);
        // The sampled one, every tile of the chunk sampled (each sample of many colours), and a
        // harvest.
        let blocks = |x: i32, y: i32| {
            [Tile::Grass, Tile::Road, Tile::Tree, Tile::Dirt, Tile::Water][((x / 4 + y / 5) % 5) as usize]
        };
        let mut s = Samples::new();
        for n in 0..8u16 {
            let from = ChunkId { cx: 5 + n, cy: n };
            s.harvest(from, (200, 200), blocks, &landed(from));
        }
        let from = ChunkId { cx: 9, cy: 9 };
        let land = landed(from);
        let harvest = time(&mut || s.harvest(from, (200, 200), blocks, &land));
        let sampled = time(&mut || {
            paint_chunk_sampled_t8(id, (200, 200), 0, blocks, &mut s, &mut l);
            core::hint::black_box(&l);
        });
        std::println!("sampled {sampled:.3} ms ({} samples), harvest {harvest:.3} ms", s.held());
    }

    #[test]
    fn coats_tell_kinds_apart_and_props_pick_by_size() {
        let mut a = Atlas::new();
        let s = StandIns::build(&mut a);
        let kinds =
            [UnitKind::Me, UnitKind::Seat, UnitKind::Folk(0), UnitKind::Folk(3), UnitKind::Hostile, UnitKind::Dead];
        for (i, &k) in kinds.iter().enumerate() {
            for &l in &kinds[i + 1..] {
                assert_ne!(s.unit(k), s.unit(l), "{k:?} and {l:?} share a coat");
            }
        }
        let small = *a.get(s.prop(1, 1, true, false));
        let big = *a.get(s.prop(4, 2, false, false));
        assert!(small.src.w < big.src.w, "{small:?} {big:?}");
        let lamp = *a.get(s.prop(1, 1, false, true));
        assert_eq!((lamp.src.w, lamp.src.h), (16, 24));
    }

    #[test]
    fn the_lit_tiers_get_relief_and_the_same_albedo() {
        // A wall two cells tall at x = 3 with its foot on row 5; a roof over rows 0 to 3.
        let tile = |x: i32, y: i32| match (x, y) {
            (3, 4 | 5) => Tile::HouseWall,
            (3, 0..=3) => Tile::HouseRoof,
            _ => Tile::Grass,
        };
        let id = ChunkId { cx: 0, cy: 0 };
        let (mut t0, mut t2) = (ChunkLayers::new(crate::Tier::T0), ChunkLayers::new(crate::Tier::T2));
        paint_chunk(id, (16, 16), 0, tile, &mut t0);
        paint_chunk(id, (16, 16), 0, tile, &mut t2);
        assert_eq!(t0.albedo, t2.albedo);
        assert!(!t0.lit() && t2.lit());
        // T0 carries the heights too, the same: its silhouettes climb what T2's field stands.
        assert_eq!(t0.height, t2.height);
        let z = |x: i32, y: i32| t2.height[(y * CHUNK_PX + x) as usize];
        // The wall's face rises from its foot; grass lies flat.
        assert!(z(3 * 16 + 8, 5 * 16 + 15) < z(3 * 16 + 8, 5 * 16 + 1));
        assert!(z(3 * 16 + 8, 4 * 16 + 1) > z(3 * 16 + 8, 5 * 16 + 1));
        assert_eq!(z(0, 0), 0);
        // The roof's two pitches face north and south.
        let ny = |x: i32, y: i32| t2.normal[(y * CHUNK_PX + x) as usize][1];
        assert!(ny(3 * 16 + 8, 2) < 128 && ny(3 * 16 + 8, 60) > 128);
    }

    #[test]
    fn a_standing_stand_in_is_as_high_as_it_is_above_its_feet() {
        let mut a = Atlas::with_layers(true);
        let s = StandIns::build(&mut a);
        let r = *a.get(s.unit(UnitKind::Me));
        let page = &a.pages.pages[0];
        let col = usize::from(r.src.x) + 16;
        let top =
            (0..r.src.h).find(|&y| page.albedo[usize::from(r.src.y + y) * usize::from(page.w) + col] > 1).unwrap();
        let z = page.height[usize::from(r.src.y + top) * usize::from(page.w) + col];
        assert_eq!(i32::from(z), i32::from(r.ay) - i32::from(top));
    }
}
