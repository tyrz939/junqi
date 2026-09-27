//! The terrain painter behind the chunk cache (PRESENTATION.md §1.6): `jane_art::terrain` reads
//! the zone through the `View` and paints a chunk's four layers into its slot; its trees, shrubs
//! and stones come back as placements the scene draws from the atlas, sorted among the units.
//! Fences and low walls are painted into the ground with the walls (`Standing::Placed`).

use jane_art::terrain::{self, Chunk, PaintMap, Painter, Placed, Standing, TileSource};
use jane_core::{Material, Tile};
use jane_sim::view::View;

use crate::atlas::{Atlas, RefId};
use crate::frame::{CELL, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers};

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
}

/// The painter, its scratch chunk, the zone's paint, and each slot's placements.
#[derive(Debug)]
pub struct Terrain {
    painter: Painter,
    chunk: Chunk,
    paint: PaintMap,
    flora: Vec<Flora>,
    placed: Vec<Vec<Placed>>,
}

/// Placements a chunk holds at most: one a cell.
const PLACED: usize = (CHUNK_CELLS * CHUNK_CELLS) as usize;

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
                Flora { look, depth: depth.clamp(3, 16) as u8 }
            })
            .collect();
        Terrain {
            painter,
            chunk: Chunk::new(),
            paint: PaintMap::default(),
            flora,
            placed: (0..slots).map(|_| Vec::with_capacity(PLACED)).collect(),
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
        if lit {
            layers.normal.copy_from_slice(&c.normal);
            layers.height.copy_from_slice(&c.height);
            for (e, &ix) in layers.emissive.iter_mut().zip(&c.emissive) {
                *e = if ix.is_opaque() { terrain::pack(ix) } else { 0 };
            }
        }
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
                if lit {
                    layers.normal[row + x_in..row + side].fill([128, 128]);
                    layers.height[row + x_in..row + side].fill(0);
                    layers.emissive[row + x_in..row + side].fill(0);
                }
            }
        }
        let placed = &mut self.placed[usize::from(slot)];
        placed.clear();
        placed.extend_from_slice(&self.chunk.placed);
    }

    /// A slot painted roughly: it stands nothing.
    pub fn swatched(&mut self, slot: u16) {
        self.placed[usize::from(slot)].clear();
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
