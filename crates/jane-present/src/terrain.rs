//! The terrain painter behind the chunk cache (PRESENTATION.md §1.6): `jane_art::terrain` reads
//! the zone through the `View` and paints a chunk's four layers into its slot; its trees, shrubs
//! and stones come back as placements the scene draws from the atlas, sorted among the units.
//! Fences and low walls are painted into the ground with the walls (`Standing::Placed`).

use jane_art::terrain::{self, Chunk, PaintMap, Painter, Placed, Standing, TileSource};
use jane_core::{Material, Tile};
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::frame::{Block, CELL, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers, SURFACE_OUTSIDE, rows_up};
use crate::shadow::RELIEF;

/// A zone as the painter reads it: the view's tiles, the paint kept beside them.
struct ViewTiles<'v, 'a> {
    view: &'v View<'a>,
    paint: &'v PaintMap,
}

impl TileSource for ViewTiles<'_, '_> {
    fn size(&self) -> (i32, i32) {
        let (w, h) = self.view.size();
        (w as i32, h as i32)
    }
    fn tile(&self, x: i32, y: i32) -> Tile {
        self.view.tile(x, y)
    }
    fn material(&self, x: i32, y: i32) -> Option<Material> {
        self.paint.get(x, y)
    }
    fn outdoor(&self) -> bool {
        !self.view.indoor()
    }
    fn region(&self, x: i32, y: i32) -> u8 {
        // Only the county's ground takes its region's ramps: a dungeon (the forest's glades
        // too) is built of its own stuff.
        if self.view.zone() != jane_core::ids::ZoneId::County {
            return 0;
        }
        match self.view.region_at(x, y) {
            jane_data::Region::Lowfields => 0,
            jane_data::Region::Waters => 1,
            jane_data::Region::Works => 2,
        }
    }
}

/// A flora sprite as the atlas holds it, with what its shadow is thrown as.
#[derive(Clone, Copy, Debug)]
pub struct Flora {
    pub look: RefId,
    /// Its sway (ART-PLAN M2): the rest frame, then its top sheared a px (two at the very top)
    /// one way and then the other. A thing that does not sway has its look three times.
    pub sway: [RefId; 3],
    /// Whether it rustles when she walks through it (reeds, long grass).
    pub rustles: bool,
    /// How deep it is across the ground, px (a trunk is thin, a shrub is its spread).
    pub depth: u8,
    /// How many rows over its foot the row it stands on is: its caster's foot, so every tier
    /// throws its shadow from where its heights are counted.
    pub lift: u8,
}

/// The painter, its scratch chunk, the zone's paint, and each slot's placements and blocks.
#[derive(Debug)]
pub struct Terrain {
    painter: Painter,
    chunk: Chunk,
    paint: PaintMap,
    flora: Vec<Flora>,
    placed: Vec<Vec<Placed>>,
    /// Each slot's blocks, chunk-local px (`blocks`).
    blocks: Vec<Vec<Block>>,
    /// The field `blocks` builds in.
    field: Vec<u8>,
    runs: Vec<Run>,
    /// Each slot's lit windows as lights (`windows`), chunk-local px.
    windows: Vec<Vec<Window>>,
}

/// A lit window as a light (PRESENTATION.md §1.7, 2026-09-28): the ground a few px in front of
/// the face it is set in, chunk-local px, how high it is, and its glow's colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub x: i16,
    pub y: i16,
    pub height: u8,
    pub colour: [u8; 3],
}

/// Glowing px of the terrain nearer than this to one another, px, are one window's.
const WINDOW_JOIN: i32 = 4;
/// How far in front of its face a window's light stands, px.
const WINDOW_OUT: i32 = 6;

/// A window's glowing px as they are gathered: `(x0, y0, x1, y1, n, r, g, b, h)`, its box, how
/// many, and the sums of their colour and height.
type Glow = (i32, i32, i32, i32, u32, u32, u32, u32, u32);

/// The lit windows of a chunk's layers (`emissive` master-palette indices, `height`): its glowing
/// px on a face (over the terrain's relief), gathered into windows, each a light standing on the
/// ground in front of its face at its middle's height.
pub fn windows(emissive: &[jane_art::palette::Ix], height: &[u8], out: &mut Vec<Window>) {
    out.clear();
    let mut acc: Vec<Glow> = Vec::new();
    let side = CHUNK_PX as usize;
    for (k, ix) in emissive.iter().enumerate() {
        let h = height.get(k).copied().unwrap_or(0);
        if !ix.is_opaque() || i32::from(h) <= RELIEF {
            continue;
        }
        let (x, y) = ((k % side) as i32, (k / side) as i32);
        let [r, g, b] = jane_art::palette::rgb(*ix);
        let near = |w: &Glow| {
            x >= w.0 - WINDOW_JOIN && x <= w.2 + WINDOW_JOIN && y >= w.1 - WINDOW_JOIN && y <= w.3 + WINDOW_JOIN
        };
        match acc.iter_mut().find(|w| near(w)) {
            Some(w) => {
                *w = (
                    w.0.min(x),
                    w.1.min(y),
                    w.2.max(x),
                    w.3.max(y),
                    w.4 + 1,
                    w.5 + u32::from(r),
                    w.6 + u32::from(g),
                    w.7 + u32::from(b),
                    w.8 + u32::from(h),
                );
            }
            None => acc.push((x, y, x, y, 1, u32::from(r), u32::from(g), u32::from(b), u32::from(h))),
        }
    }
    for (x0, y0, x1, y1, n, r, g, b, h) in acc {
        // A few px of glow is a spark, not a window.
        if n < 4 {
            continue;
        }
        let height = (h / n).clamp(1, 255) as i32;
        let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
        out.push(Window {
            x: cx as i16,
            y: (cy + rows_up(height) + WINDOW_OUT) as i16,
            height: height as u8,
            colour: [(r / n) as u8, (g / n) as u8, (b / n) as u8],
        });
    }
}

/// Placements a chunk holds at most: one a cell.
const PLACED: usize = (CHUNK_CELLS * CHUNK_CELLS) as usize;
/// Blocks a slot reserves (a street of houses and fences is a few hundred).
const BLOCKS: usize = 1024;
/// Rows of a chunk's field: its own, and those under it its tallest px stand on.
const FIELD_ROWS: i32 = CHUNK_PX + rows_up(255) + 1;
/// How far apart two px's heights may be and still stand in one block, px: a hedge's crown, a
/// roof's course, a cliff's broken top. The block stands the tallest of them.
pub const BLOCK_TOLERANCE: u8 = 3;

/// A rect of the field still growing down the rows: `(x0, x1, y0, lo, hi)`.
pub type Run = (i16, i16, i16, u8, u8);

/// What the terrain of one chunk stands on the ground, as [`Block`]s in chunk-local px (a block
/// can reach under the chunk: a px `h` up stands `rows_up(h)` rows below it), into `out`. The px
/// a fence drew (`fence`, a bit a px, row by row: `jane_art::terrain::Chunk::fence_px`; empty
/// for none) are left out: a fence's blocks are its parts ([`fence_blocks`]).
///
/// The field is T2's (`scatter.wgsl`): every px of `height` over [`RELIEF`] stands on the ground
/// `rows_up(h)` rows below it and the row over that (the terrain is two rows deep), the tallest
/// winning, a px wider each side. A row's field is cut into runs whose heights lie within
/// [`BLOCK_TOLERANCE`], and a run that meets one of the same columns in the row above within it
/// grows that one down: a house's face and walls are the block of its eave, its roof a block a
/// course or two, a wall's run one block.
pub fn blocks(height: &[u8], fence: &[u64], field: &mut Vec<u8>, runs: &mut Vec<Run>, out: &mut Vec<Block>) {
    out.clear();
    let side = CHUNK_PX as usize;
    field.clear();
    field.resize(side * FIELD_ROWS as usize, 0);
    for (k, &h) in height.iter().enumerate() {
        if i32::from(h) <= RELIEF || fence.get(k / 64).is_some_and(|w| w >> (k % 64) & 1 == 1) {
            continue;
        }
        let (x, y) = (k % side, k / side);
        let gy = y + rows_up(i32::from(h)) as usize;
        for r in [gy - 1, gy] {
            let f = &mut field[r * side + x];
            *f = (*f).max(h);
        }
    }
    runs.clear();
    let mut open = 0;
    for r in 0..FIELD_ROWS {
        let row = &field[r as usize * side..(r as usize + 1) * side];
        let r = r as i16;
        // This row's runs, appended after the open rects; the open ones not grown are closed.
        let first = runs.len();
        let mut x = 0;
        while x < side {
            let h = row[x];
            if h == 0 {
                x += 1;
                continue;
            }
            let (x0, mut lo, mut hi) = (x, h, h);
            x += 1;
            while x < side && row[x] > 0 && hi.max(row[x]) - lo.min(row[x]) <= BLOCK_TOLERANCE {
                (lo, hi) = (lo.min(row[x]), hi.max(row[x]));
                x += 1;
            }
            runs.push((x0 as i16, x as i16, r, lo, hi));
        }
        // Grow each open rect by the run of its columns, or close it.
        let mut kept = 0;
        for i in 0..open {
            let (x0, x1, y0, lo, hi) = runs[i];
            let grown = (first..runs.len()).find(|&j| {
                let (a, b, _, l, h) = runs[j];
                a == x0 && b == x1 && h > 0 && hi.max(h) - lo.min(l) <= BLOCK_TOLERANCE
            });
            match grown {
                Some(j) => {
                    let (_, _, _, l, h) = runs[j];
                    runs[j].4 = 0;
                    runs[kept] = (x0, x1, y0, lo.min(l), hi.max(h));
                    kept += 1;
                }
                None => out.push(block(x0, x1, y0, r, hi)),
            }
        }
        // The runs that grew nothing open rects of their own.
        let mut n = kept;
        for j in first..runs.len() {
            if runs[j].4 > 0 {
                runs[n] = runs[j];
                n += 1;
            }
        }
        runs.truncate(n);
        open = n;
    }
    for &(x0, x1, y0, _, hi) in &runs[..open] {
        out.push(block(x0, x1, y0, FIELD_ROWS as i16, hi));
    }
}

/// A chunk's fence as blocks (the fence rule, PRESENTATION.md §1.7): each post and rail as the
/// painter drew it (`jane_art::terrain::Chunk::fences`), a post from the ground, a rail from its
/// underside, appended to `out`.
pub fn fence_blocks(parts: &[jane_art::terrain::FencePart], out: &mut Vec<Block>) {
    out.extend(parts.iter().map(|f| Block {
        x0: f.x0,
        y0: f.y0,
        x1: f.x1,
        y1: f.y1,
        height: f.hi,
        lo: f.lo,
        fence: true,
    }));
}

/// The widest row of what a canvas draws, px.
fn drawn_width(c: &jane_art::Canvas) -> i32 {
    (0..c.h())
        .map(|y| {
            let xs: Vec<i32> = (0..c.w()).filter(|&x| c.get(x, y).is_opaque()).collect();
            xs.last().map_or(0, |l| l - xs[0] + 1)
        })
        .max()
        .unwrap_or(0)
}

/// A sway frame of a flora sprite (ART-PLAN M2): its rows from the top of what is drawn down a
/// fifth of its height pushed two px toward `dir`, the rows down to a third one px, the rest at
/// rest, so the crown leans over a trunk that stays put. Two px wider each side than the sprite,
/// its anchor moved with it.
fn sheared(atlas: &mut Atlas, c: &jane_art::Canvas, (ax, ay): (i32, i32), dir: i32) -> RefId {
    let top = (0..c.h()).find(|&y| (0..c.w()).any(|x| c.get(x, y).is_opaque())).unwrap_or(0);
    let tall = (ay - top).max(1);
    let shift = |y: i32| {
        if y < top + tall / 5 {
            2 * dir
        } else if y < top + tall / 3 {
            dir
        } else {
            0
        }
    };
    let (w, h) = ((c.w() + 4) as u16, c.h() as u16);
    atlas.add_texels(w, h, (ax as i16 + 2, ay as i16), ay.clamp(1, 255) as u8, |x, y| {
        crate::atlas::Texel::of(c, x - 2 - shift(y), y)
    })
}

/// A block over columns `x0..x1` and rows `y0..y1`, a px wider each side (T2's terrain).
fn block(x0: i16, x1: i16, y0: i16, y1: i16, height: u8) -> Block {
    Block { x0: x0 - 1, y0, x1: x1 + 1, y1, height, lo: 0, fence: false }
}

impl Terrain {
    /// The painter over the compiled looks, its flora packed into `atlas`, and `slots` slots.
    pub fn build(atlas: &mut Atlas, slots: usize) -> Terrain {
        let mut painter = Painter::new();
        painter.set_standing(Standing::Placed);
        let bank = painter.bank();
        let flora = bank
            .all()
            .into_iter()
            .enumerate()
            .map(|(i, (name, s))| {
                let (w, h) = (s.canvas.w(), s.ay);
                let look = atlas.add_canvas(&s.canvas, (s.ax as i16, s.ay as i16), h.clamp(1, 255) as u8, |_, _, t| t);
                let kind = bank.kind_of(i as u16);
                let sway = if kind.sways() {
                    [look, sheared(atlas, &s.canvas, (s.ax, s.ay), -1), sheared(atlas, &s.canvas, (s.ax, s.ay), 1)]
                } else {
                    [look; 3]
                };
                let rustles = matches!(kind, jane_art::flora::Kind::Reeds | jane_art::flora::Kind::Grass);
                // A tree throws its shadow from its trunk; a shrub or a stone from its spread,
                // as deep as it is drawn wide (round, seen from above): a row of bushes planted
                // down the screen, a cell apart, is a hedge in the field as one across it is
                // (at a third of the canvas's width, light passed between them one way and not
                // the other, 2026-09-28).
                let wide = drawn_width(&s.canvas);
                let depth = if name.contains("tree") || name.starts_with("pine") { 6 } else { wide.max(w / 3) };
                // What it stands on, rows over its foot (a shrub's rim; its heights are counted
                // from there, `jane_art::flora::base`).
                let lift = (s.ay - jane_art::flora::base(&s.canvas, s.ay)).clamp(0, 255) as u8;
                Flora { look, sway, rustles, depth: depth.clamp(3, 16) as u8, lift }
            })
            .collect();
        Terrain {
            painter,
            chunk: Chunk::new(),
            paint: PaintMap::default(),
            flora,
            placed: (0..slots).map(|_| Vec::with_capacity(PLACED)).collect(),
            blocks: (0..slots).map(|_| Vec::with_capacity(BLOCKS)).collect(),
            field: Vec::with_capacity((CHUNK_PX * FIELD_ROWS) as usize),
            runs: Vec::with_capacity(512),
            windows: (0..slots).map(|_| Vec::new()).collect(),
        }
    }

    /// A new zone: its paint read once.
    pub fn zone(&mut self, view: &View<'_>) {
        self.paint.fill(view.size(), view.paint());
    }

    /// Paints chunk `id` properly into `layers`, the chunk in `slot`. Cells outside the zone
    /// take `outside`, as the swatches do.
    pub fn paint(&mut self, view: &View<'_>, id: ChunkId, slot: u16, outside: u32, layers: &mut ChunkLayers) {
        let src = ViewTiles { view, paint: &self.paint };
        let (cx, cy) = (i32::from(id.cx), i32::from(id.cy));
        terrain::paint_chunk(&mut self.painter, &src, view.seed(), cx, cy, &mut self.chunk);
        let c = &self.chunk.layers;
        layers.albedo.copy_from_slice(&c.albedo);
        let lit = layers.lit();
        if layers.has_height() {
            layers.height.copy_from_slice(&c.height);
        }
        layers.glow.clear();
        if !lit {
            // T0 keeps what glows sparse (§1.3 `glow`): its lit windows, up to the reserve.
            let room = layers.glow.capacity();
            let glowing = c.emissive.iter().enumerate().filter(|(_, ix)| ix.is_opaque());
            layers.glow.extend(glowing.take(room).map(|(k, &ix)| (k as u16, terrain::pack(ix))));
        }
        if lit {
            layers.normal.copy_from_slice(&c.normal);
            layers.fence.copy_from_slice(&self.chunk.fence_px);
            for (e, &ix) in layers.emissive.iter_mut().zip(&c.emissive) {
                *e = if ix.is_opaque() { terrain::pack(ix) } else { 0 };
            }
            // What the ground is to the rain and the water (§1.8): the px's distance to land
            // through water, and how its cell takes rain.
            for (k, s) in layers.surface.iter_mut().enumerate() {
                let (x, y) = (k as i32 % CHUNK_PX, k as i32 / CHUNK_PX);
                let wet = c.wet[(y / CELL * CHUNK_CELLS + x / CELL) as usize].min(3);
                *s = c.water[k].min(16) * 4 + wet;
            }
        }
        layers.water.clear();
        layers.water.extend(self.chunk.water.iter().map(|w| (w.x, w.y, w.phase)));
        // Beyond the zone's edge: the frame's clear, flat.
        let (w, h) = src.size();
        let (x0, y0) = (cx * CHUNK_CELLS, cy * CHUNK_CELLS);
        if x0 + CHUNK_CELLS > w || y0 + CHUNK_CELLS > h {
            let side = CHUNK_PX as usize;
            for y in 0..CHUNK_PX {
                let row = y as usize * side;
                let inside_y = y0 + y / CELL < h;
                let x_in = if inside_y { ((w - x0).clamp(0, CHUNK_CELLS) * CELL) as usize } else { 0 };
                layers.albedo[row + x_in..row + side].fill(outside);
                if layers.has_height() {
                    layers.height[row + x_in..row + side].fill(0);
                }
                if lit {
                    layers.normal[row + x_in..row + side].fill([128, 128]);
                    layers.emissive[row + x_in..row + side].fill(0);
                    layers.surface[row + x_in..row + side].fill(SURFACE_OUTSIDE);
                }
            }
            layers.water.retain(|&(x, y, _)| x0 + i32::from(x) < w && y0 + i32::from(y) < h);
            layers.glow.retain(|&(k, _)| {
                let (x, y) = (i32::from(k) % CHUNK_PX, i32::from(k) / CHUNK_PX);
                x0 + x / CELL < w && y0 + y / CELL < h
            });
        }
        let placed = &mut self.placed[usize::from(slot)];
        placed.clear();
        placed.extend_from_slice(&self.chunk.placed);
        windows(&c.emissive, &c.height, &mut self.windows[usize::from(slot)]);
        let (zw, zh) = (w * CELL - x0 * CELL, h * CELL - y0 * CELL);
        self.windows[usize::from(slot)].retain(|wd| i32::from(wd.x) < zw && i32::from(wd.y) < zh + CELL);
        let out = &mut self.blocks[usize::from(slot)];
        if layers.has_height() {
            blocks(&layers.height, &self.chunk.fence_px, &mut self.field, &mut self.runs, out);
            fence_blocks(&self.chunk.fences, out);
        } else {
            out.clear();
        }
    }

    /// The lit windows of the chunk in `slot`, chunk-local px.
    pub fn windows(&self, slot: u16) -> &[Window] {
        &self.windows[usize::from(slot)]
    }

    /// The blocks of the chunk in `slot` from the swatches' relief (a painted chunk's are made
    /// as it is painted, its fence apart).
    pub fn stand(&mut self, slot: u16, layers: &ChunkLayers) {
        let out = &mut self.blocks[usize::from(slot)];
        if layers.has_height() {
            blocks(&layers.height, &[], &mut self.field, &mut self.runs, out);
        } else {
            out.clear();
        }
    }

    /// What the chunk in `slot` stands, chunk-local px.
    pub fn blocks(&self, slot: u16) -> &[Block] {
        &self.blocks[usize::from(slot)]
    }

    /// A slot painted roughly: it stands nothing, and its ground is dry land until the painter
    /// reaches it.
    pub fn swatched(&mut self, slot: u16, layers: &mut ChunkLayers) {
        self.placed[usize::from(slot)].clear();
        self.windows[usize::from(slot)].clear();
        layers.surface.fill(0);
        layers.fence.fill(0);
        layers.water.clear();
        layers.glow.clear();
    }

    /// What the chunk in `slot` stands: foot px in the chunk, and the sprite.
    pub fn placed(&self, slot: u16) -> &[Placed] {
        &self.placed[usize::from(slot)]
    }

    /// Flora sprite `i` (`Placed::sprite`) as the atlas holds it.
    pub fn flora(&self, i: u16) -> Flora {
        self.flora[usize::from(i)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::height_of_rows;

    #[test]
    fn each_lit_window_is_a_light_in_front_of_its_face_and_a_spark_is_none() {
        // Two windows 4 x 5 on a face 20 px up, 24 px apart, and one glowing px on the ground.
        let side = CHUNK_PX as usize;
        let (mut e, mut h) = (vec![jane_art::palette::Ix::CLEAR; side * side], vec![1u8; side * side]);
        let glass = jane_art::palette::Ramp::GlassLit.at(jane_art::palette::Tone::Light);
        for x0 in [40, 64] {
            for y in 100..105 {
                for x in x0..x0 + 4 {
                    (e[y * side + x], h[y * side + x]) = (glass, 20);
                }
            }
        }
        e[200 * side + 10] = glass;
        let mut out = Vec::new();
        windows(&e, &h, &mut out);
        assert_eq!(out.len(), 2, "{out:?}");
        for (w, x0) in out.iter().zip([40, 64]) {
            assert_eq!((w.x, w.height), (x0 + 1, 20));
            // On the ground in front of the face: its middle's rows down, and out.
            assert_eq!(i32::from(w.y), 102 + rows_up(20) + WINDOW_OUT);
        }
    }

    #[test]
    fn a_house_stands_on_its_footprint_as_one_block_and_a_cobble_as_none() {
        // A roof 40 rows deep at 60 px over a face 48 rows tall (a storey) whose foot is row 148,
        // columns 16..96; a cobble 5 px high and a kerb 8 px high beside it.
        let side = CHUNK_PX as usize;
        let mut h = vec![1u8; side * side];
        for y in 60..148 {
            for x in 16..96 {
                h[y * side + x] = if y < 100 { 60 } else { height_of_rows(148 - y as i32).max(1) as u8 };
            }
        }
        h[200 * side + 150] = 5;
        h[200 * side + 160] = 8;
        let (mut field, mut runs, mut out) = (Vec::new(), Vec::new(), Vec::new());
        blocks(&h, &[], &mut field, &mut runs, &mut out);
        // The roof lands 48 rows down, on the face's foot: the house's ground is rows 107 to
        // 148, and every px of its face and roof stands on it, a px wider each side.
        assert_eq!(out, [Block { x0: 15, y0: 107, x1: 97, y1: 149, height: 60, lo: 0, fence: false }]);
    }

    #[test]
    fn a_fence_stands_as_its_parts_and_a_hedge_as_a_block() {
        // A post-and-rail fence across the chunk, its foot on row 99: rails on rows 82..85 and
        // 90..93, a post 6 px wide down to the foot, every px of it marked a fence's; and a hedge
        // cell to the right, a crown at 9 px over a face of 7 rows.
        let side = CHUNK_PX as usize;
        let mut h = vec![1u8; side * side];
        let mut fence = vec![0u64; side * side / 64];
        let mut mark = |h: &mut [u8], x: usize, y: usize, v: u8| {
            h[y * side + x] = v;
            fence[(y * side + x) / 64] |= 1 << ((y * side + x) % 64);
        };
        for x in 10..120 {
            for y in [82, 83, 84, 90, 91, 92] {
                mark(&mut h, x, y, height_of_rows(99 - y as i32) as u8);
            }
        }
        for x in 60..66 {
            for y in 76..100 {
                mark(&mut h, x, y, height_of_rows(99 - y as i32).max(5) as u8);
            }
        }
        for y in 140..160 {
            for x in 140..160 {
                h[y * side + x] = if y < 153 { 9 } else { height_of_rows(160 - y as i32).max(1) as u8 };
            }
        }
        let (mut field, mut runs, mut out) = (Vec::new(), Vec::new(), Vec::new());
        blocks(&h, &fence, &mut field, &mut runs, &mut out);
        // The fence's px stand in no block of the heights: its parts are its blocks.
        assert!(out.iter().all(|b| b.x0 >= 139), "{out:?}");
        let hedge = out.iter().find(|b| b.x0 <= 150 && 150 < b.x1 && b.y0 <= 158 && 158 < b.y1).copied().unwrap();
        assert_eq!((hedge.height, hedge.lo, hedge.fence), (9, 0, false));
        assert!(crate::shadow::spills(&hedge));
        let rail = jane_art::terrain::FencePart { x0: 10, y0: 98, x1: 26, y1: 100, lo: 7, hi: 11 };
        fence_blocks(&[rail], &mut out);
        let b = *out.last().unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1, b.lo, b.height, b.fence), (10, 98, 26, 100, 7, 11, true));
        assert!(crate::shadow::spills(&b));
    }
}
