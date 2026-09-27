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
/// can reach under the chunk: a px `h` up stands `rows_up(h)` rows below it), into `out`.
///
/// The field is T2's (`scatter.wgsl`): every px of `height` over [`RELIEF`] stands on the ground
/// `rows_up(h)` rows below it and the row over that (the terrain is two rows deep), the tallest
/// winning, a px wider each side. A row's field is cut into runs whose heights lie within
/// [`BLOCK_TOLERANCE`], and a run that meets one of the same columns in the row above within it
/// grows that one down: a house's face and walls are the block of its eave, its roof a block a
/// course or two, a wall's run one block, a fence's rails a thin one.
pub fn blocks(height: &[u8], field: &mut Vec<u8>, runs: &mut Vec<Run>, out: &mut Vec<Block>) {
    out.clear();
    let side = CHUNK_PX as usize;
    field.clear();
    field.resize(side * FIELD_ROWS as usize, 0);
    for (k, &h) in height.iter().enumerate() {
        if i32::from(h) <= RELIEF {
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

/// A block over columns `x0..x1` and rows `y0..y1`, a px wider each side (T2's terrain).
fn block(x0: i16, x1: i16, y0: i16, y1: i16, height: u8) -> Block {
    Block { x0: x0 - 1, y0, x1: x1 + 1, y1, height }
}

impl Terrain {
    /// The painter over the compiled looks, its flora packed into `atlas`, and `slots` slots.
    pub fn build(atlas: &mut Atlas, slots: usize) -> Terrain {
        let mut painter = Painter::new();
        painter.set_standing(Standing::Placed);
        let flora = painter
            .bank()
            .all()
            .into_iter()
            .map(|(name, s)| {
                let (w, h) = (s.canvas.w(), s.ay);
                let look = atlas.add_canvas(&s.canvas, (s.ax as i16, s.ay as i16), h.clamp(1, 255) as u8, |_, _, t| t);
                // A tree throws its shadow from its trunk; a shrub or a stone from its spread.
                let depth = if name.contains("tree") || name.starts_with("pine") { 6 } else { w / 3 };
                // What it stands on, rows over its foot (a shrub's rim; its heights are counted
                // from there, `jane_art::flora::base`).
                let lift = (s.ay - jane_art::flora::base(&s.canvas, s.ay)).clamp(0, 255) as u8;
                Flora { look, depth: depth.clamp(3, 16) as u8, lift }
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
        self.stand(slot, layers);
    }

    /// The blocks of the chunk in `slot` from its heights as they are now (after a paint, or
    /// the swatches' relief).
    pub fn stand(&mut self, slot: u16, layers: &ChunkLayers) {
        let out = &mut self.blocks[usize::from(slot)];
        if layers.has_height() {
            blocks(&layers.height, &mut self.field, &mut self.runs, out);
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
        layers.surface.fill(0);
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
        blocks(&h, &mut field, &mut runs, &mut out);
        // The roof lands 48 rows down, on the face's foot: the house's ground is rows 107 to
        // 148, and every px of its face and roof stands on it, a px wider each side.
        assert_eq!(out, [Block { x0: 15, y0: 107, x1: 97, y1: 149, height: 60 }]);
    }
}
