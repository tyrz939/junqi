//! The tile painter (ART.md §2.6, PRESENTATION.md §1.6): a chunk of 16 x 16 cells painted into
//! four 256 x 256 layers, with its strips, water cells and wall casters.
//!
//! A chunk is painted in passes, the way a 2D Zelda map is built (the TS painter's algorithm, at
//! 16 px a cell):
//!
//! 1. **Ground.** Every open ground tile is a surface; a cell's surface fills it except at a
//!    convex corner, where an 8 px chamfer goes to the neighbour, so a road on a diagonal has a
//!    straight 45° edge instead of a staircase; and the edge then wobbles a px or two by noise, so
//!    no edge is ruled. Things that stand on the ground take the ground of their neighbours.
//!    Wild ground with at most one neighbour like it draws as what surrounds it.
//! 2. **Detail**, calm and by cell hash: blades, tufts, pebbles, ruts, cracks, furrows, glints.
//! 3. **Edges**, read off the per-pixel surface map: higher ground takes a rim where it drops
//!    (lit where it faces north, dark where it faces south), the lower ground a shadow under it;
//!    water a dark band under its bank and a pale lip everywhere else.
//! 4. **Hard tiles** by their own painters: walls and cliffs with a face where their south is
//!    open, roofs with ridges and eaves, house walls with windows, floors, rails, boards.
//! 5. **Standing things** (trees, shrubs, stones, fences, low walls) go into per-row strips the
//!    renderer y-sorts with units: the sim cell is one cell, the drawing is Zelda-sized.
//!
//! **Rule:** a chunk is a pure function of the tiles within [`REACH`] cells of it, their paint and
//! the seed: the same `(seed, chunk)` paints the same bytes, and a tile change reaches only the
//! chunks [`chunks_touched`] names. Variation is `h32` of the world cell and the seed, never the
//! sim's dice; noise is over world px, so two chunks agree along their seam.
//!
//! Units: px at 16 a cell ([`crate::canvas::CELL_PX`]); chunk-local px unless a name says world.

// A cell's hash is read by its low bits (`h & 3 == 0`: one cell in four), and a neighbourhood of
// surfaces is a few bytes counted by a filter: both read plainer as written.
#![allow(clippy::verbose_bit_mask, clippy::naive_bytecount)]

pub mod dungeon;
mod ecotone;
mod field;
mod ground;
mod hard;
pub mod houses;
mod interior;
pub mod region;
pub mod sheet;
mod standing;
mod style;

use jane_core::grid::{Grid, Rect};
use jane_core::hash::Fnv;
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Material, Tile};
use jane_data::TileGroup;

use crate::canvas::{Canvas, FLAT, Normal};
use crate::flora::{Bank, Ramps};
use crate::palette::{self, Ix, Ramp, Tone};

use field::Field;
use houses::{House, Houses, Room};
pub use standing::{fence_parts, mask};
pub use style::{Style, Styles, over, snake};

/// Screen px per cell.
pub const CELL: i32 = crate::canvas::CELL_PX;
/// Cells per chunk side.
pub const CHUNK_CELLS: i32 = 16;
/// Px per chunk side.
pub const CHUNK_PX: i32 = CHUNK_CELLS * CELL;
/// A strip's height: tall enough for the tallest tree above its row's foot line.
pub const STRIP_H: i32 = 88;
/// A strip's margin each side of the chunk, for crowns that overhang it.
pub const STRIP_MARGIN: i32 = 32;
/// How far below its row's cell bottom a strip reaches (a shrub's foot).
pub const STRIP_BELOW: i32 = 4;
/// How many cells beyond its chunk a tile can change what the chunk paints: a corner or an edge
/// reads 1, which tree in a wood shows a crown and where a cliff shows its face read 2, the
/// speckle filter reads a cell's neighbours' surfaces, and a standing thing's surface is the
/// ground its neighbour lends it, 1 more each.
pub const REACH: i32 = 4;

/// Cells of scratch each side of the chunk.
const M: i32 = REACH;
/// Scratch side, cells.
const GM: i32 = CHUNK_CELLS + 2 * M;
/// The surface map covers cells `-1..17`: its side in px.
const MM: i32 = (CHUNK_CELLS + 2) * CELL;
/// No surface: a cell drawn by a hard painter.
const NONE: u8 = 255;

/// Salts, one per use.
mod salt {
    /// Per-cell detail.
    pub const CELL: u32 = 0x5445_5252;
    /// The ground's broad patches.
    pub const PATCH: u32 = 0x5041_5443;
    /// The wobble of an edge, x and y.
    pub const WOBBLE_X: u32 = 0x574f_4258;
    pub const WOBBLE_Y: u32 = 0x574f_4259;
    /// Which tree, shrub or stone a cell shows.
    pub const STAND: u32 = 0x5354_414e;
    /// A water cell's shimmer phase.
    pub const WATER: u32 = 0x5741_5452;
    /// Which windows are lit.
    pub const WINDOW: u32 = 0x5749_4e44;
}

/// Where the painter reads tiles: a zone's grid through whatever holds it (the renderer's `View`,
/// a built blueprint, a sheet's made-up scene). `jane-art` never sees the sim.
pub trait TileSource {
    /// Width and height in cells.
    fn size(&self) -> (i32, i32);
    /// The tile at cell `(x, y)`; `Void` outside.
    fn tile(&self, x: i32, y: i32) -> Tile;
    /// The render-only material painted over cell `(x, y)` (`Blueprint::paint`), if any.
    fn material(&self, _x: i32, _y: i32) -> Option<Material> {
        None
    }
    /// Out of doors: a wide block of wall is a building, and a building seen from above is its roof.
    fn outdoor(&self) -> bool {
        true
    }
    /// The region under cell `(x, y)` (`jane_data::Region` order: 0 Lowfields, 1 Waters, 2
    /// Works), whose ramps it is drawn in ([`region::REGIONS`]); the Lowfields' by default.
    fn region(&self, _x: i32, _y: i32) -> u8 {
        0
    }
    /// The house whose block or front garden holds cell `(x, y)` ([`houses`], ART-PLAN Q2 and
    /// M7): what its roof, walls, door and garden are drawn as. Found once a zone over its
    /// whole block, so a house is one look across every chunk it lies in; none by default (the
    /// houses then draw as plain cream under red tile).
    fn house(&self, _x: i32, _y: i32) -> Option<House> {
        None
    }
    /// Seen from inside, whose room the zone is (ART-PLAN M4): its walls papered, its back wall
    /// hung with things, its windows laying light on the floor. None for a dungeon.
    fn room(&self) -> Option<Room> {
        None
    }
    /// A dungeon read as rooms (ART-PLAN M5, B2): its faces drawn two cells tall and framed, its
    /// floors bordered and worn, its motif in every room. None out of a dungeon.
    fn dungeon(&self) -> Option<&dungeon::Dungeon> {
        None
    }
    /// Whether it is day, for a room's windows: by day they lay light on the floor, at night
    /// they are dark. The renderer repaints a room's chunks when it turns.
    fn daylight(&self) -> bool {
        true
    }
}

/// A zone's tiles and paint, owned: what a tool or a test paints from.
#[derive(Clone, Debug)]
pub struct TileMap {
    /// The tiles.
    pub tiles: Grid<Tile>,
    /// Per cell, 0 or 1 + the material's index (`Blueprint::paint` flattened, later rects over earlier).
    pub paint: Grid<u8>,
    /// See [`TileSource::outdoor`].
    pub outdoor: bool,
    /// See [`TileSource::region`]: the blueprint's region map (empty: the Lowfields).
    pub regions: jane_core::blueprint::RegionMap,
    /// See [`TileSource::house`]: empty until [`TileMap::find_houses`].
    pub houses: Houses,
    /// See [`TileSource::room`].
    pub room: Option<Room>,
    /// See [`TileSource::daylight`].
    pub daylight: bool,
    /// See [`TileSource::dungeon`].
    pub dungeon: Option<dungeon::Dungeon>,
}

const MATERIALS: [Material; 5] =
    [Material::RoofSlate, Material::RoofThatch, Material::BrickWall, Material::Pine, Material::WildEarth];

/// A material as the byte a paint grid keeps: 0 for none.
fn code(m: Material) -> u8 {
    MATERIALS.iter().position(|&x| x == m).map_or(0, |i| i as u8 + 1)
}

/// A paint grid's byte as its material.
fn decode(k: u8) -> Option<Material> {
    k.checked_sub(1).and_then(|i| MATERIALS.get(usize::from(i)).copied())
}

/// A zone's render-only paint (`Blueprint::paint`) flattened to half a byte a cell, later rects over
/// earlier: what a renderer keeps beside a view that hands it the rects, so the painter reads a
/// cell's material in O(1). Refilled per zone without allocating once it has held the largest.
#[derive(Clone, Debug, Default)]
pub struct PaintMap {
    w: i32,
    h: i32,
    /// Two cells a byte (a code is 0 to 5), low nibble first: 2 MB for the county, not 4
    /// (PLAY-PLAN.md §7).
    k: Vec<u8>,
}

impl PaintMap {
    /// Refill for a zone of `(w, h)` cells painted `paint`.
    pub fn fill(&mut self, (w, h): (u32, u32), paint: &[(Rect, Material)]) {
        (self.w, self.h) = (w as i32, h as i32);
        self.k.clear();
        self.k.resize((w as usize * h as usize).div_ceil(2), 0);
        for &(r, m) in paint {
            let k = code(m);
            for y in r.y.max(0)..r.bottom().min(self.h) {
                for x in r.x.max(0)..r.right().min(self.w) {
                    let i = (y * self.w + x) as usize;
                    let shift = (i % 2) * 4;
                    self.k[i / 2] = (self.k[i / 2] & !(0xf << shift)) | (k << shift);
                }
            }
        }
    }

    /// The material painted over cell `(x, y)`, if any.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<Material> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        let i = (y * self.w + x) as usize;
        decode((self.k[i / 2] >> ((i % 2) * 4)) & 0xf)
    }
}

impl TileMap {
    /// Tiles with no paint.
    pub fn new(tiles: Grid<Tile>, outdoor: bool) -> TileMap {
        let paint = Grid::new(tiles.w(), tiles.h(), 0);
        TileMap {
            tiles,
            paint,
            outdoor,
            regions: jane_core::blueprint::RegionMap::default(),
            houses: Houses::default(),
            room: None,
            daylight: true,
            dungeon: None,
        }
    }

    /// A blueprint's tiles and paint.
    pub fn from_blueprint(bp: &Blueprint) -> TileMap {
        let mut m = TileMap::new(bp.tiles.clone(), !bp.indoor);
        for &(r, mat) in &bp.paint {
            m.paint.fill_rect(r, code(mat));
        }
        m.regions = bp.regions.clone();
        m
    }

    /// Paint `mat` over `r`.
    pub fn set_material(&mut self, r: Rect, mat: Material) {
        self.paint.fill_rect(r, code(mat));
    }

    /// Find its houses under world seed `seed` ([`Houses::fill`]), their doors at `doors`.
    pub fn find_houses(&mut self, seed: u32, doors: &[(i32, i32, houses::Kind)]) {
        let tiles = &self.tiles;
        let size = (tiles.w() as i32, tiles.h() as i32);
        self.houses.fill(size, |x, y| tiles.read(x, y, Tile::Void), doors, seed);
    }

    /// A blueprint's tiles and paint with its houses found under `seed`, their doors read off
    /// its props (`door` sprites; one into the Arms is the inn's, one into St Anne's the
    /// church's), and its room if it is one ([`room_of`]).
    pub fn from_blueprint_seeded(bp: &Blueprint, seed: u32) -> TileMap {
        let mut m = TileMap::from_blueprint(bp);
        let cat = jane_data::catalog();
        let doors: Vec<(i32, i32, houses::Kind)> = bp
            .props
            .iter()
            .filter(|p| cat.sprites.get(usize::from(cat.story.prop(p.def).sprite.0)) == Some(&"door"))
            .map(|p| (i32::from(p.cell.x), i32::from(p.cell.y), houses::door_kind(p.to.map(|d| d.zone))))
            .collect();
        m.find_houses(seed, &doors);
        let busy = bp.props.iter().filter(|p| !cat.story.prop(p.def).flat).flat_map(|p| {
            let d = cat.story.prop(p.def);
            (0..i32::from(d.w)).map(move |dx| (i32::from(p.cell.x) + dx, i32::from(p.cell.y)))
        });
        let tiles = &m.tiles;
        let room =
            room_of(bp.zone, seed, tiles.w() as i32, |x, y| tiles.read(x, y, Tile::Void).flags() & F_SOLID != 0, busy);
        m.room = room;
        let all_props = bp.props.iter().flat_map(|p| {
            let d = cat.story.prop(p.def);
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            (0..i32::from(d.w)).flat_map(move |dx| (0..i32::from(d.h)).map(move |dy| (x + dx, y + dy)))
        });
        let boss =
            bp.units.iter().find(|u| cat.combat.unit(u.def).boss).map(|u| (i32::from(u.cell.x), i32::from(u.cell.y)));
        let tiles = &m.tiles;
        let rect_of = |id: jane_core::ids::NameId| bp.rects.get(&jane_core::Key::Name(id)).copied();
        let size = (tiles.w() as i32, tiles.h() as i32);
        m.dungeon = dungeon_of(bp.zone, seed, size, |x, y| tiles.read(x, y, Tile::Void), all_props, rect_of, boss);
        m
    }
}

/// The dungeon a zone is read as (ART-PLAN M5, B2), if a theme in the data names it: its tiles,
/// the cells its props stand on, its room graph (each mission node's room rect, which `rect_of`
/// finds by name, and its kind) and where its boss stands.
pub fn dungeon_of(
    zone: jane_core::ids::ZoneId,
    seed: u32,
    size: (i32, i32),
    tile: impl Fn(i32, i32) -> Tile,
    props: impl Iterator<Item = (i32, i32)>,
    rect_of: impl Fn(jane_core::ids::NameId) -> Option<Rect>,
    boss: Option<(i32, i32)>,
) -> Option<dungeon::Dungeon> {
    use jane_data::MissionNodeKind as K;
    let theme = dungeon::Theme::of_zone(zone)?;
    let graph: Vec<(Rect, dungeon::Role)> = jane_data::catalog()
        .dungeons
        .mission_of(zone)
        .map(|m| {
            m.nodes
                .iter()
                .filter_map(|n| {
                    let r = n.names.iter().find_map(|t| t.rects.first().and_then(|&id| rect_of(id)))?;
                    let role = match n.kind {
                        K::Boss => dungeon::Role::Boss,
                        K::Hub => dungeon::Role::Set,
                        K::Rest => dungeon::Role::Rest,
                        _ => dungeon::Role::Plain,
                    };
                    Some((r, role))
                })
                .collect()
        })
        .unwrap_or_default();
    let seed = crate::hash::h32(seed, zone as u32, 0x4455_4e47);
    Some(dungeon::Dungeon::find(theme, size, tile, props, &graph, boss, seed))
}

/// The room a zone is seen from inside as (ART-PLAN M4), if it is one: Julie's house, the Arms
/// or St Anne's; its seed from the world's; the columns of its back walls that `props` (a tall
/// thing's footprint cells) stand against.
pub fn room_of(
    zone: jane_core::ids::ZoneId,
    seed: u32,
    w: i32,
    solid: impl Fn(i32, i32) -> bool,
    props: impl Iterator<Item = (i32, i32)>,
) -> Option<Room> {
    use jane_core::ids::ZoneId as Z;
    let kind = match zone {
        Z::House => houses::RoomKind::Julie,
        Z::Arms => houses::RoomKind::Inn,
        Z::Church => houses::RoomKind::Church,
        _ => return None,
    };
    let wall = solid;
    let mut busy = 0u128;
    for (x, y) in props {
        if (0..128).contains(&x) && (wall(x, y - 1) || wall(x, y - 2)) {
            busy |= 1 << x;
        }
    }
    let mut room = Room { kind, seed: crate::hash::h32(seed, zone as u32, 0x524f_4f4d), busy, ..Room::default() };
    room.hang(w, &wall);
    Some(room)
}

impl TileSource for TileMap {
    fn size(&self) -> (i32, i32) {
        (self.tiles.w() as i32, self.tiles.h() as i32)
    }
    fn tile(&self, x: i32, y: i32) -> Tile {
        self.tiles.read(x, y, Tile::Void)
    }
    fn material(&self, x: i32, y: i32) -> Option<Material> {
        decode(self.paint.read(x, y, 0))
    }
    fn outdoor(&self) -> bool {
        self.outdoor
    }
    fn region(&self, x: i32, y: i32) -> u8 {
        self.regions.region_at(x, y).unwrap_or(0)
    }
    fn house(&self, x: i32, y: i32) -> Option<House> {
        self.houses.at(x, y).copied()
    }
    fn room(&self) -> Option<Room> {
        self.room
    }
    fn dungeon(&self) -> Option<&dungeon::Dungeon> {
        self.dungeon.as_ref()
    }
    fn daylight(&self) -> bool {
        self.daylight
    }
}

/// A chunk's four layers (ART.md §1.1, PRESENTATION.md §1.4), each `CHUNK_PX * CHUNK_PX`,
/// row-major. The albedo is resolved through the palette as it is painted (a chunk is blitted whole
/// and never tinted); the other layers stay raw for the light pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkLayers {
    /// `0xFFRRGGBB`, every pixel opaque.
    pub albedo: Vec<u32>,
    /// Tangent-space normals (`canvas::decode`).
    pub normal: Vec<Normal>,
    /// Palette indices that shine when the ambient is low (lit windows); 0 is none.
    pub emissive: Vec<Ix>,
    /// Px above the ground plane; at least 1 everywhere.
    pub height: Vec<u8>,
    /// Per cell, row-major 16 x 16: how the tile takes rain (0 matt, 1 darkens, 2 darkens and
    /// shines), which the presenter's wetness byte scales (PRESENTATION.md §1.8).
    pub wet: [u8; (CHUNK_CELLS * CHUNK_CELLS) as usize],
    /// Per px, its distance to land through water, 1..=16, or 0 on land: where the renderer
    /// reflects and how deep it darkens (PRESENTATION.md §1.8). Not in [`Chunk::hash`]: it is
    /// the surface map the albedo already shows.
    pub water: Vec<u8>,
}

/// One row's standing things: a rect of a strip `CHUNK_PX + 2 * STRIP_MARGIN` wide and
/// [`STRIP_H`] tall whose bottom is [`STRIP_BELOW`] px under the row's cell bottom, cropped to
/// what is drawn. The renderer sorts it among units by [`Strip::base_y`] (PRESENTATION.md §1.6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Strip {
    /// The cell row in the chunk, 0..16.
    pub row: u8,
    /// Chunk-local px of the rect's top-left (x may be as low as `-STRIP_MARGIN`).
    pub x: i16,
    /// Chunk-local px of the rect's top (may be above the chunk).
    pub y: i16,
    /// Size in px.
    pub w: u16,
    /// Size in px.
    pub h: u16,
    /// Holds a canopy (a tree's crown): the ghost pass fades it when she is under it.
    pub canopy: bool,
    /// `0xFFRRGGBB`, or 0 where nothing is drawn.
    pub albedo: Vec<u32>,
    /// Normals; flat where nothing is drawn.
    pub normal: Vec<Normal>,
    /// Px above the row's foot line; 0 where nothing is drawn.
    pub height: Vec<u8>,
    /// What each pixel is ([`mask`]): clear, a solid standing thing, or canopy, for the canopy
    /// ghost and, on the lit tiers, dappled light and god rays.
    pub mask: Vec<u8>,
}

impl Strip {
    /// The chunk-local y the strip sorts at: its row's cell bottom.
    pub fn base_y(&self) -> i32 {
        (i32::from(self.row) + 1) * CELL
    }
}

/// A water cell of the chunk: where the renderer's shimmer and caustics go (PRESENTATION.md §1.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterCell {
    /// Chunk-local cell.
    pub x: u8,
    /// Chunk-local cell.
    pub y: u8,
    /// The shimmer's phase, 0..64, from the world cell: one caustic field across every seam.
    pub phase: u8,
}

/// An edge of a wall run's footprint at ground, in world canvas px, with the height it casts from
/// (PRESENTATION.md §1.7). Runs of cells are merged into one segment per straight edge in a chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CasterSeg {
    /// One end, world canvas px.
    pub a: (i32, i32),
    /// The other end.
    pub b: (i32, i32),
    /// Px it stands.
    pub height: u8,
}

/// The least height a fence's px stands at, px: over the shadows' ground (`jane_present::shadow::
/// GROUND`, 4), so a fence is never the ground its own shadow is laid on, and under the relief
/// the terrain casts from (8).
pub const FENCE_FLOOR: u8 = 5;

/// A part of a fence as its shadow sees it (PRESENTATION.md §1.7, the fence rule): a post or a
/// rail as a box on the ground, chunk-local px `[x0, x1) x [y0, y1)` of the ground it stands
/// over, its matter from `lo` px up to `hi`. A post stands from the ground; a rail floats. Built
/// from the fence's cells and their neighbours with the numbers the fence is drawn with
/// (`standing::fence`), so the shadow is the posts and the two rails as drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FencePart {
    /// The footprint's left edge, chunk-local px.
    pub x0: i16,
    /// Its top row.
    pub y0: i16,
    /// Its right edge, exclusive.
    pub x1: i16,
    /// Its bottom row, exclusive: the row under the fence's foot.
    pub y1: i16,
    /// Its matter's bottom, px up (0: from the ground).
    pub lo: u8,
    /// Its top, px up.
    pub hi: u8,
}

/// A flora sprite stood in the chunk: which one (its index in `Painter::bank().all()`), where its
/// foot is (chunk-local px) and the cell row whose strip holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    /// Index into `Bank::all()`.
    pub sprite: u16,
    /// Chunk-local px of the sprite's foot.
    pub x: i16,
    /// Chunk-local px of the sprite's foot.
    pub y: i16,
    /// The cell row it sorts with.
    pub row: u8,
}

/// A painted chunk: what the renderer's chunk cache keeps (PRESENTATION.md §1.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Chunk coordinates.
    pub cx: i32,
    /// Chunk coordinates.
    pub cy: i32,
    /// The four layers and the wetness response.
    pub layers: ChunkLayers,
    strips: Vec<Strip>,
    n_strips: usize,
    /// The water cells.
    pub water: Vec<WaterCell>,
    /// The wall runs' caster segments.
    pub casters: Vec<CasterSeg>,
    /// The flora stamped into the strips, as placements of `Painter::bank().all()` sprites: for a
    /// renderer that would rather draw trees from its atlas than keep their strips.
    pub placed: Vec<Placed>,
    /// The chunk's fences as posts and rails ([`FencePart`]): what their sun shadows are thrown
    /// from on every tier. Not in the golden (a view of the tiles, like `casters`).
    pub fences: Vec<FencePart>,
    /// Which px of the layers a fence drew ([`Standing::Placed`] only), a bit a px, row by row
    /// (`y * CHUNK_PX + x`): the sun passes them by (their shadow is [`Chunk::fences`]'), a lamp
    /// does not.
    pub fence_px: Vec<u64>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    /// An empty chunk, its buffers allocated once: paint into it again and again.
    pub fn new() -> Chunk {
        let n = (CHUNK_PX * CHUNK_PX) as usize;
        Chunk {
            cx: 0,
            cy: 0,
            layers: ChunkLayers {
                albedo: vec![0; n],
                normal: vec![FLAT; n],
                emissive: vec![Ix::CLEAR; n],
                height: vec![1; n],
                wet: [0; (CHUNK_CELLS * CHUNK_CELLS) as usize],
                water: vec![0; n],
            },
            strips: Vec::new(),
            n_strips: 0,
            water: Vec::with_capacity((CHUNK_CELLS * CHUNK_CELLS) as usize),
            casters: Vec::with_capacity(256),
            placed: Vec::with_capacity((CHUNK_CELLS * CHUNK_CELLS) as usize),
            fences: Vec::with_capacity(256),
            fence_px: vec![0; (CHUNK_PX * CHUNK_PX / 64) as usize],
        }
    }

    /// Whether a fence drew chunk-local px `(x, y)`.
    pub fn is_fence(&self, x: i32, y: i32) -> bool {
        let k = (y * CHUNK_PX + x) as usize;
        (0..CHUNK_PX).contains(&x) && (0..CHUNK_PX).contains(&y) && self.fence_px[k / 64] >> (k % 64) & 1 == 1
    }

    /// The rows with standing things in them, top row first.
    pub fn strips(&self) -> &[Strip] {
        &self.strips[..self.n_strips]
    }

    /// FNV-1a over everything painted: the chunk golden.
    pub fn hash(&self) -> u32 {
        let l = &self.layers;
        let mut f = Fnv::new().i32(self.cx).i32(self.cy);
        for a in &l.albedo {
            f = f.u32(*a);
        }
        for n in &l.normal {
            f = f.u8(n[0]).u8(n[1]);
        }
        for e in &l.emissive {
            f = f.u16(e.0);
        }
        f = f.bytes(&l.height).bytes(&l.wet);
        for s in self.strips() {
            f = f.u8(s.row).u16(s.x as u16).u16(s.y as u16).u16(s.w).u16(s.h).u8(u8::from(s.canopy));
            for a in &s.albedo {
                f = f.u32(*a);
            }
            for n in &s.normal {
                f = f.u8(n[0]).u8(n[1]);
            }
            f = f.bytes(&s.height).bytes(&s.mask);
        }
        for w in &self.water {
            f = f.u8(w.x).u8(w.y).u8(w.phase);
        }
        for t in &self.placed {
            f = f.u16(t.sprite).u16(t.x as u16).u16(t.y as u16).u8(t.row);
        }
        for c in &self.casters {
            f = f.i32(c.a.0).i32(c.a.1).i32(c.b.0).i32(c.b.1).u8(c.height);
        }
        f.finish()
    }

    fn next_strip(&mut self) -> &mut Strip {
        if self.n_strips == self.strips.len() {
            self.strips.push(Strip::default());
        }
        self.n_strips += 1;
        &mut self.strips[self.n_strips - 1]
    }
}

/// The chunks a change to the cells `r` reaches, as a rect of chunk coordinates (clip it to the
/// zone): every chunk within [`REACH`] cells of `r`. What `Event::Tiles(rect)` invalidates.
pub fn chunks_touched(r: Rect) -> Rect {
    let g = r.grow(REACH);
    let (x0, y0) = (g.x.div_euclid(CHUNK_CELLS), g.y.div_euclid(CHUNK_CELLS));
    let (x1, y1) = ((g.right() - 1).div_euclid(CHUNK_CELLS), (g.bottom() - 1).div_euclid(CHUNK_CELLS));
    Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// The chunk painter's working layers: the chunk's pixels as palette indices, before they are
/// resolved into [`ChunkLayers`].
#[derive(Clone, Debug)]
pub(crate) struct Layers {
    pub albedo: Vec<Ix>,
    pub normal: Vec<Normal>,
    pub emissive: Vec<Ix>,
    pub height: Vec<u8>,
}

impl Layers {
    fn new() -> Layers {
        let n = (CHUNK_PX * CHUNK_PX) as usize;
        Layers { albedo: vec![Ix::CLEAR; n], normal: vec![FLAT; n], emissive: vec![Ix::CLEAR; n], height: vec![1; n] }
    }

    fn clear(&mut self) {
        self.albedo.fill(Ix::CLEAR);
        self.normal.fill(FLAT);
        self.emissive.fill(Ix::CLEAR);
        self.height.fill(1);
    }

    #[inline]
    pub fn i(x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < CHUNK_PX && y < CHUNK_PX).then(|| (y * CHUNK_PX + x) as usize)
    }

    /// Write one pixel's albedo, normal and height (at least 1).
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, ix: Ix, n: Normal, z: i32) {
        if let Some(i) = Self::i(x, y) {
            self.albedo[i] = ix;
            self.normal[i] = n;
            self.height[i] = z.clamp(1, 255) as u8;
        }
    }

    /// Change a pixel's colour only.
    #[inline]
    pub fn ink(&mut self, x: i32, y: i32, ix: Ix) {
        if let Some(i) = Self::i(x, y) {
            self.albedo[i] = ix;
        }
    }

    /// The pixel's ramp and tone, if it is a ramp entry.
    #[inline]
    pub fn tone(&self, x: i32, y: i32) -> Option<(Ramp, Tone)> {
        Self::i(x, y).and_then(|i| Ramp::of(self.albedo[i]))
    }

    /// Move a pixel `steps` tones lighter (negative: darker) along its own ramp.
    #[inline]
    pub fn step(&mut self, x: i32, y: i32, steps: i32) {
        if let Some(i) = Self::i(x, y) {
            if let Some((r, t)) = Ramp::of(self.albedo[i]) {
                self.albedo[i] = r.at(t.step(steps));
            }
        }
    }

    /// Set a pixel's normal.
    #[inline]
    pub fn tilt(&mut self, x: i32, y: i32, n: Normal) {
        if let Some(i) = Self::i(x, y) {
            self.normal[i] = n;
        }
    }

    /// The pixel's height.
    #[inline]
    pub fn z(&self, x: i32, y: i32) -> i32 {
        Self::i(x, y).map_or(0, |i| i32::from(self.height[i]))
    }

    /// Make a pixel shine: `ix` goes to the emissive layer.
    #[inline]
    pub fn glow(&mut self, x: i32, y: i32, ix: Ix) {
        if let Some(i) = Self::i(x, y) {
            self.emissive[i] = ix;
        }
    }
}

/// Per-chunk scratch, reused: nothing is allocated after the first chunk.
#[derive(Clone, Debug)]
struct Scratch {
    /// Tiles for cells `-M..16+M` each way, row-major `GM x GM`.
    raw: Vec<Tile>,
    /// Their paint.
    mat: Vec<Option<Material>>,
    /// Each cell's region (`TileSource::region`).
    region: Vec<u8>,
    /// Each cell's house (`TileSource::house`).
    house: Vec<Option<House>>,
    /// Each region's share of the ground, blended across its borders.
    eco: ecotone::Ecotone,
    /// Which other wild ground each cell has near it, for the drifts across their edge.
    gmix: ecotone::GroundMix,
    /// Which surfaces drift ([`blends`]), by id; and which cells a drift reached.
    blend: [bool; 256],
    hit: Vec<bool>,
    /// The tile each cell is drawn as: its surface for ground, the ground it borrows for a
    /// standing thing, itself for a hard tile.
    paint: Vec<Tile>,
    /// The chamfer surface (a tile id), or [`NONE`] for a hard-painted cell.
    surf: Vec<u8>,
    /// The speckle filter's output.
    surf2: Vec<u8>,
    /// Cells with another surface among their eight neighbours; whether any water is in reach.
    mixed: Vec<bool>,
    water_near: bool,
    /// Water cells' distance to land in cells, 0 on land, at most 3.
    depth: Vec<u8>,
    /// The surface per px for cells `-1..17`: before and after the wobble.
    mm0: Vec<u8>,
    mm: Vec<u8>,
    /// Px to the nearest non-water px, at most 16, over the surface map.
    shore: Vec<u8>,
    /// The painted layers.
    ly: Layers,
    /// Broad patches over the chunk; the wobble over the surface map.
    patch: Field,
    /// The fields worked over the chunk: patches, the meander, the lush drifts.
    pv: Vec<i32>,
    mv: Vec<i32>,
    lv: Vec<i32>,
    fine: Field,
    /// Lush and dry grass, over many cells.
    lush: Field,
    wob_x: Field,
    wob_y: Field,
    /// One strip row being drawn, its mask, and one standing thing.
    row: Canvas,
    rowmask: Vec<u8>,
    /// Which px of the strip a fence drew (`Standing::Placed`): what `bake` marks in
    /// [`Chunk::fence_px`].
    fencerow: Vec<bool>,
    /// Where the strip canvas was last drawn: what the next row clears.
    row_bb: Rect,
    thing: Canvas,
    /// Per room of a dungeon, what the last chunk drew of its framing (`dungeon::FRAMED_*` bits).
    framed: Vec<u8>,
}

/// How a painter hands over the standing things of a chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Standing {
    /// Every row's trees, shrubs, stones, fences and low walls stamped into [`Chunk::strips`]
    /// (the sheets, and a renderer that sorts strips among its units), and listed in
    /// [`Chunk::placed`] too.
    #[default]
    Strips,
    /// No strips: the flora only as [`Chunk::placed`] (a renderer draws them from its atlas and
    /// sorts them one by one), and the fences and low walls painted into the ground layers like
    /// the walls, the tops of the row below the chunk that reach up into it included.
    Placed,
}

/// The chunk painter: the styles, the flora bank and the scratch. One per thread that paints.
#[derive(Clone, Debug)]
pub struct Painter {
    styles: Styles,
    bank: Bank,
    standing: Standing,
    s: Scratch,
    /// Per palette index: whether a region swaps it as ground, and its colour in each region.
    lut: Vec<(bool, [u32; 3])>,
    /// The chunk being painted: its first world cell, and the world seed.
    x0c: i32,
    y0c: i32,
    seed: u32,
    /// The zone seen from inside, and whether it is day (`TileSource::room`, `daylight`).
    room: Option<Room>,
    daylight: bool,
}

impl Default for Painter {
    fn default() -> Self {
        Self::new()
    }
}

impl Painter {
    /// A painter over the looks compiled into this build.
    pub fn new() -> Painter {
        Painter::with_styles(Styles::compiled())
    }

    /// A painter over `styles`.
    pub fn with_styles(styles: Styles) -> Painter {
        let ramp = |t: Tile| styles.tile(t).ramp;
        let ramps = Ramps {
            leaf: ramp(Tile::Tree),
            bark: styles.tile(Tile::Tree).accent.unwrap_or(Ramp::Bark),
            needle: styles.cell(Tile::Tree, Some(Material::Pine)).ramp,
            shrub: ramp(Tile::Bush),
            dead: ramp(Tile::DeadTree),
            stone: ramp(Tile::Rubble),
        };
        let cells = (GM * GM) as usize;
        let map = (MM * MM) as usize;
        let s = Scratch {
            raw: vec![Tile::Void; cells],
            mat: vec![None; cells],
            region: vec![0; cells],
            house: vec![None; cells],
            eco: ecotone::Ecotone::default(),
            gmix: ecotone::GroundMix::default(),
            blend: std::array::from_fn(|g| blends(&styles, g as u8)),
            hit: vec![false; (ecotone::HIT_SIDE * ecotone::HIT_SIDE) as usize],
            paint: vec![Tile::Void; cells],
            surf: vec![NONE; cells],
            surf2: vec![NONE; cells],
            depth: vec![0; cells],
            mixed: vec![true; cells],
            water_near: false,
            mm0: vec![NONE; map],
            mm: vec![NONE; map],
            shore: vec![0; map],
            ly: Layers::new(),
            patch: Field::default(),
            pv: vec![0; (CHUNK_PX * CHUNK_PX) as usize],
            mv: vec![0; (CHUNK_PX * CHUNK_PX) as usize],
            lv: vec![0; (CHUNK_PX * CHUNK_PX) as usize],
            fine: Field::default(),
            lush: Field::default(),
            wob_x: Field::default(),
            wob_y: Field::default(),
            row: Canvas::new(CHUNK_PX + 2 * STRIP_MARGIN, STRIP_H),
            rowmask: vec![0; ((CHUNK_PX + 2 * STRIP_MARGIN) * STRIP_H) as usize],
            fencerow: vec![false; ((CHUNK_PX + 2 * STRIP_MARGIN) * STRIP_H) as usize],
            row_bb: Rect::new(0, 0, CHUNK_PX + 2 * STRIP_MARGIN, STRIP_H),
            thing: Canvas::new(3 * CELL, STRIP_H),
            framed: Vec::new(),
        };
        let lut = (0..palette::LEN)
            .map(|i| {
                let ix = Ix(i as u16);
                let ground = Ramp::of(ix).is_some_and(|(r, _)| region::is_ground(r));
                (ground, [0u8, 1, 2].map(|r| pack(region::tint(r, ix))))
            })
            .collect();
        Painter {
            styles,
            bank: Bank::new(ramps),
            standing: Standing::Strips,
            s,
            lut,
            x0c: 0,
            y0c: 0,
            seed: 0,
            room: None,
            daylight: true,
        }
    }

    /// The styles it paints with.
    pub fn styles(&self) -> &Styles {
        &self.styles
    }

    /// The px of the last chunk painted that a drift between two wild grounds turned over
    /// (`ecotone::GroundMix`): chunk-local `(x, y)`, the surface (a tile id) it was and the one
    /// it took. For tests and sheets: a laid way's cells never appear here.
    pub fn drifts(&self) -> &[(i16, i16, u8, u8)] {
        &self.s.gmix.flips
    }

    /// Per room of the dungeon the last chunk was painted in, what of its framing that chunk
    /// drew (`dungeon::FRAMED_*` bits: a face two cells tall, a floor border, its motif); empty
    /// out of a dungeon. For the framing test (ART-PLAN §7 rule 6).
    pub fn framed(&self) -> &[u8] {
        &self.s.framed
    }

    /// The flora it stamps.
    pub fn bank(&self) -> &Bank {
        &self.bank
    }

    /// Hand the standing things over as `standing` from the next chunk on.
    pub fn set_standing(&mut self, standing: Standing) {
        self.standing = standing;
    }

    /// Paint chunk `(cx, cy)` of `src` under world seed `seed` into `out` (see [`paint_chunk`]).
    pub fn paint(&mut self, src: &impl TileSource, seed: u32, cx: i32, cy: i32, out: &mut Chunk) {
        let (x0, y0) = (cx * CHUNK_CELLS, cy * CHUNK_CELLS);
        (self.x0c, self.y0c, self.seed) = (x0, y0, seed);
        (self.room, self.daylight) = (src.room(), src.daylight());
        self.gather(src, x0, y0);
        self.s.eco.fill(src, x0, y0, seed);
        self.resolve(src, x0, y0);
        self.ground_mix(x0, y0, seed);
        self.surface_map(x0, y0, seed);
        ground::paint(self, x0, y0, seed);
        hard::paint(self, src, x0, y0, seed);
        standing::ground(self, x0, y0, seed);
        out.cx = cx;
        out.cy = cy;
        out.n_strips = 0;
        out.placed.clear();
        out.fence_px.fill(0);
        standing::strips(self, x0, y0, seed, out);
        self.finish(x0, y0, out);
    }

    /// Cell `(i, j)` of the scratch (chunk-local cell `i - M`, `j - M`).
    #[inline]
    fn k(i: i32, j: i32) -> usize {
        (j * GM + i) as usize
    }

    /// The scratch index of chunk-local cell `(x, y)`, which may be up to `M` outside.
    #[inline]
    fn at(x: i32, y: i32) -> usize {
        Self::k(x + M, y + M)
    }

    fn gather(&mut self, src: &impl TileSource, x0: i32, y0: i32) {
        for j in 0..GM {
            for i in 0..GM {
                let (x, y) = (x0 + i - M, y0 + j - M);
                let k = Self::k(i, j);
                let house = src.house(x, y);
                // A garden's boundary is drawn as its house's own (`standing::plot`), on the grass
                // it stands in: the sim's fence is not drawn again under it.
                let fenced = house.is_some_and(|h| h.fenced(x, y));
                self.s.raw[k] = if fenced { Tile::Grass } else { src.tile(x, y) };
                self.s.mat[k] = src.material(x, y);
                self.s.region[k] = src.region(x, y);
                self.s.house[k] = house;
            }
        }
    }

    /// Style of scratch cell `k` as drawn (its paint applied).
    #[inline]
    fn style_k(&self, k: usize) -> &Style {
        self.styles.cell(self.s.raw[k], self.s.mat[k])
    }

    /// A cliff cell with at most one cliff beside it is not a cliff; it is a boulder.
    fn lone_cliff(src: &impl TileSource, x: i32, y: i32) -> bool {
        let n = [(0, -1), (0, 1), (-1, 0), (1, 0)]
            .iter()
            .filter(|(dx, dy)| src.tile(x + dx, y + dy) == Tile::Cliff)
            .count();
        n <= 1
    }

    /// Which tile each cell is drawn as, and its chamfer surface; then the speckle filter.
    fn resolve(&mut self, src: &impl TileSource, x0: i32, y0: i32) {
        for j in 0..GM {
            for i in 0..GM {
                let k = Self::k(i, j);
                let (x, y) = (x0 + i - M, y0 + j - M);
                let t = self.s.raw[k];
                let st = *self.style_k(k);
                let lone = t == Tile::Cliff && Self::lone_cliff(src, x, y);
                let paint = match st.row.group {
                    TileGroup::Ground | TileGroup::Water => st.row.inherit.unwrap_or(t),
                    TileGroup::Flora => self.borrow(src, x, y, st.row.inherit.unwrap_or(Tile::Grass)),
                    _ if lone => self.borrow(src, x, y, Tile::Dirt),
                    // A doorway's sill is drawn as the floor it joins, no slab of its own (the
                    // owner's playtest, 2026-09-29: "normal floor continuing between rooms and
                    // halls"). It stays a sill to the sim, where nothing is pushed onto it.
                    _ if t == Tile::Sill => self.floor_of_sill(src, x, y),
                    _ => t,
                };
                let ps = self.styles.tile(paint);
                self.s.paint[k] = paint;
                self.s.surf[k] = match ps.row.group {
                    TileGroup::Ground | TileGroup::Water => paint.id(),
                    _ => NONE,
                };
            }
        }
        // Speckle: a cell of wild ground with at most one neighbour like it is noise in the
        // generator's dice, not a place, so it draws as the ground round it. Made ground and water
        // mean something and are never touched.
        let wild = |p: &Painter, g: u8| g != NONE && p.styles.id(g).row.wild;
        self.s.surf2.copy_from_slice(&self.s.surf);
        for j in 1..GM - 1 {
            for i in 1..GM - 1 {
                let k = Self::k(i, j);
                let g = self.s.surf[k];
                if !wild(self, g) {
                    continue;
                }
                let n = [
                    self.s.surf[k - GM as usize],
                    self.s.surf[k + GM as usize],
                    self.s.surf[k - 1],
                    self.s.surf[k + 1],
                ];
                if n.iter().filter(|&&c| c == g).count() > 1 {
                    continue;
                }
                let (mut best, mut best_n) = (NONE, 0);
                for &c in &n {
                    if c == g || !wild(self, c) {
                        continue;
                    }
                    let cn = n.iter().filter(|&&d| d == c).count();
                    if cn > best_n {
                        best = c;
                        best_n = cn;
                    }
                }
                if best_n >= 2 {
                    self.s.surf2[k] = best;
                    if self.styles.tile(self.s.paint[k]).row.group != TileGroup::Flora {
                        self.s.paint[k] = Tile::from_id(best).unwrap_or(Tile::Grass);
                    }
                }
            }
        }
        std::mem::swap(&mut self.s.surf, &mut self.s.surf2);
        // Flora that borrowed a speckled neighbour's ground follows it.
        for k in 0..self.s.surf.len() {
            if self.s.surf[k] != NONE {
                self.s.paint[k] = Tile::from_id(self.s.surf[k]).unwrap_or(Tile::Grass);
            }
        }
        // Which cells have another surface among their eight neighbours: only there can an edge
        // fall, so the passes that draw edges skip the rest.
        self.s.water_near = false;
        for j in 0..GM {
            for i in 0..GM {
                let k = Self::k(i, j);
                let g = self.s.surf[k];
                self.s.water_near |= g != NONE && self.styles.id(g).is_water();
                let mut mixed = i == 0 || j == 0 || i == GM - 1 || j == GM - 1;
                for dj in -1..=1 {
                    for di in -1..=1 {
                        if !mixed {
                            mixed = self.s.surf[(k as i32 + dj * GM + di) as usize] != g;
                        }
                    }
                }
                self.s.mixed[k] = mixed;
            }
        }
        // Water depth in cells, up to 3, by three passes of the four neighbours.
        for k in 0..self.s.depth.len() {
            self.s.depth[k] = u8::from(self.s.surf[k] != NONE && self.styles.id(self.s.surf[k]).is_water()) * 3;
        }
        for _ in 0..3 {
            for j in 0..GM {
                for i in 0..GM {
                    let k = Self::k(i, j);
                    if self.s.depth[k] == 0 {
                        continue;
                    }
                    let mut d = self.s.depth[k];
                    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
                        let (a, b) = (i + dx, j + dy);
                        if a >= 0 && b >= 0 && a < GM && b < GM {
                            d = d.min(self.s.depth[Self::k(a, b)] + 1);
                        }
                    }
                    self.s.depth[k] = d;
                }
            }
        }
    }

    /// The ground a standing thing at `(x, y)` stands on: its first neighbour (N, W, E, S) with
    /// open ground or a floor to lend, else `default`.
    /// The floor a sill at `(x, y)` joins: the first open neighbour, the way through first, that
    /// is no sill; else the sill itself.
    fn floor_of_sill(&self, src: &impl TileSource, x: i32, y: i32) -> Tile {
        let across = src.tile(x - 1, y) == Tile::Sill || src.tile(x + 1, y) == Tile::Sill;
        let order: [(i32, i32); 4] =
            if across { [(0, 1), (0, -1), (1, 0), (-1, 0)] } else { [(1, 0), (-1, 0), (0, 1), (0, -1)] };
        for (dx, dy) in order {
            let n = src.tile(x + dx, y + dy);
            if n == Tile::Void || n == Tile::Sill {
                continue;
            }
            let st = self.styles.cell(n, src.material(x + dx, y + dy));
            match st.row.group {
                TileGroup::Ground => return st.row.inherit.unwrap_or(n),
                TileGroup::Made if n.flags() & F_SOLID == 0 => return n,
                _ => {}
            }
        }
        Tile::Sill
    }

    fn borrow(&self, src: &impl TileSource, x: i32, y: i32, default: Tile) -> Tile {
        // Stepping stones stand in the water when the water is round them.
        let own = self.styles.cell(src.tile(x, y), src.material(x, y));
        if own.row.pattern == jane_data::TilePattern::Stepping {
            let wet = [(0, -1), (-1, 0), (1, 0), (0, 1)]
                .iter()
                .filter(|&&(dx, dy)| src.tile(x + dx, y + dy) == Tile::Water)
                .count();
            if wet >= 2 {
                return Tile::Water;
            }
        }
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            let n = src.tile(x + dx, y + dy);
            if n == Tile::Void {
                continue;
            }
            let st = self.styles.cell(n, src.material(x + dx, y + dy));
            match st.row.group {
                TileGroup::Ground => return st.row.inherit.unwrap_or(n),
                TileGroup::Made if n.flags() & F_SOLID == 0 => return n,
                _ => {}
            }
        }
        default
    }

    /// The per-px surface map for cells `-1..17`: each cell's surface, chamfered at convex
    /// corners, then its edges wobbled by noise.
    fn surface_map(&mut self, x0: i32, y0: i32, seed: u32) {
        let rise = |p: &Painter, g: u8| i32::from(p.styles.id(g).row.rise);
        for cj in 0..CHUNK_CELLS + 2 {
            for ci in 0..CHUNK_CELLS + 2 {
                let k = Self::k(ci + M - 1, cj + M - 1);
                let g = self.s.surf[k];
                let base = (cj * CELL * MM + ci * CELL) as usize;
                for py in 0..CELL as usize {
                    let r = base + py * MM as usize;
                    self.s.mm0[r..r + CELL as usize].fill(g);
                }
                if g == NONE {
                    continue;
                }
                for q in 0..4 {
                    let (dx, dy): (i32, i32) = (if q & 1 == 0 { -1 } else { 1 }, if q & 2 == 0 { -1 } else { 1 });
                    let a = self.s.surf[(k as i32 + dx) as usize];
                    let b = self.s.surf[(k as i32 + dy * GM) as usize];
                    if a == g || a == NONE || b == g || b == NONE {
                        continue;
                    }
                    let fill = if rise(self, a) <= rise(self, b) { b } else { a };
                    // A pinch: our surface goes on past the corner diagonally, the other only
                    // touches it there. The higher ground keeps its corner, the lower one's is cut,
                    // so two pools that meet at a corner stay two with a neck of land between.
                    let d = self.s.surf[(k as i32 + dx + dy * GM) as usize];
                    if d == g && rise(self, g) > rise(self, fill) {
                        continue;
                    }
                    for u in 0..CELL / 2 {
                        for v in 0..CELL / 2 - u {
                            let px = if dx < 0 { u } else { CELL - 1 - u };
                            let py = if dy < 0 { v } else { CELL - 1 - v };
                            self.s.mm0[base + (py * MM + px) as usize] = fill;
                        }
                    }
                }
            }
        }
        // The wobble: near an edge between surfaces, where either is wild, each px takes the
        // surface a px or two away along a smooth noise field.
        let (wx0, wy0) = (x0 * CELL - CELL, y0 * CELL - CELL);
        self.s.wob_x.fill(wx0, wy0, MM, MM, 3, seed ^ salt::WOBBLE_X);
        self.s.wob_y.fill(wx0, wy0, MM, MM, 3, seed ^ salt::WOBBLE_Y);
        self.s.mm.copy_from_slice(&self.s.mm0);
        for cj in 0..CHUNK_CELLS + 2 {
            for ci in 0..CHUNK_CELLS + 2 {
                let k = Self::k(ci + M - 1, cj + M - 1);
                if !self.s.mixed[k] || self.s.surf[k] == NONE {
                    continue;
                }
                for py in cj * CELL..cj * CELL + CELL {
                    for px in ci * CELL..ci * CELL + CELL {
                        let i = (py * MM + px) as usize;
                        let here = self.s.mm0[i];
                        let (wx, wy) = (wx0 + px, wy0 + py);
                        let dx = ((self.s.wob_x.at(wx, wy) * 5) >> 8) - 2;
                        let dy = ((self.s.wob_y.at(wx, wy) * 5) >> 8) - 2;
                        let (sx, sy) = (px + dx, py + dy);
                        if sx < 0 || sy < 0 || sx >= MM || sy >= MM {
                            continue;
                        }
                        let there = self.s.mm0[(sy * MM + sx) as usize];
                        if there == here || there == NONE || here == NONE {
                            continue;
                        }
                        if self.styles.id(here).row.wild || self.styles.id(there).row.wild {
                            self.s.mm[i] = there;
                        }
                    }
                }
            }
        }
        // Where two wild grounds meet, drifts of each over the other (`ecotone::GroundMix`),
        // before the edges are read off the map, so a drift has its rim like any patch; an edge
        // can then fall in any cell by one a drift reached.
        self.s.gmix.apply(&mut self.s.mm, &self.s.blend, &mut self.s.hit);
        let side = ecotone::HIT_SIDE;
        for cy in -2..side - 2 {
            for cx in -2..side - 2 {
                if !self.s.hit[((cy + 2) * side + cx + 2) as usize] {
                    continue;
                }
                for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let (x, y) = (cx + dx, cy + dy);
                    if (-M..CHUNK_CELLS + M).contains(&x) && (-M..CHUNK_CELLS + M).contains(&y) {
                        self.s.mixed[Self::at(x, y)] = true;
                    }
                }
            }
        }
        // Water's slivers: where two pools meet only at a corner the chamfers leave a thread of
        // water a px or two wide; a px of water with three or fewer water px round it goes to the
        // land beside it, and a px of land hemmed in by seven goes to water.
        if !self.s.water_near {
            self.s.shore.fill(0);
            return;
        }
        let is_water = |p: &Painter, g: u8| g != NONE && p.styles.id(g).is_water();
        self.s.mm0.copy_from_slice(&self.s.mm);
        for y in 1..MM - 1 {
            for x in 1..MM - 1 {
                if !self.s.mixed[Self::k(x / CELL + M - 1, y / CELL + M - 1)] {
                    continue;
                }
                let i = (y * MM + x) as usize;
                let here = self.s.mm0[i];
                if here == NONE {
                    continue;
                }
                let (mut wet, mut land) = (0, NONE);
                for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let n = self.s.mm0[((y + dy) * MM + x + dx) as usize];
                    if is_water(self, n) {
                        wet += 1;
                    } else if n != NONE && land == NONE {
                        land = n;
                    }
                }
                if is_water(self, here) && wet <= 3 && land != NONE {
                    self.s.mm[i] = land;
                } else if !is_water(self, here) && wet >= 7 {
                    let w = [(0, -1), (-1, 0), (1, 0), (0, 1)]
                        .iter()
                        .map(|(dx, dy)| self.s.mm0[((y + dy) * MM + x + dx) as usize])
                        .find(|&n| is_water(self, n));
                    if let Some(w) = w {
                        self.s.mm[i] = w;
                    }
                }
            }
        }
        // Px to the nearest non-water px, capped at 16 (the map's margin), so it agrees across seams.
        let water = |p: &Painter, g: u8| g != NONE && p.styles.id(g).is_water();
        for i in 0..self.s.shore.len() {
            self.s.shore[i] = if water(self, self.s.mm[i]) { 16 } else { 0 };
        }
        for pass in 0..2 {
            for n in 0..MM * MM {
                let i = if pass == 0 { n } else { MM * MM - 1 - n };
                let (x, y) = (i % MM, i / MM);
                let mut d = self.s.shore[i as usize];
                if d == 0 {
                    continue;
                }
                let s: i32 = if pass == 0 { -1 } else { 1 };
                for (dx, dy) in [(s, 0), (0, s), (s, s), (-s, s)] {
                    let (a, b) = (x + dx, y + dy);
                    if a >= 0 && b >= 0 && a < MM && b < MM {
                        d = d.min(self.s.shore[(b * MM + a) as usize].saturating_add(1));
                    }
                }
                self.s.shore[i as usize] = d;
            }
        }
    }

    /// Where two wild grounds meet, which draws as which (`ecotone::GroundMix`). Only grounds of
    /// a loose pattern blend: a garden's rows, setts and water keep their edge.
    fn ground_mix(&mut self, x0: i32, y0: i32, seed: u32) {
        let (surf, raw, mat) = (&self.s.surf, &self.s.raw, &self.s.mat);
        let (blend, styles) = (&self.s.blend, &self.styles);
        let laid = |x: i32, y: i32| {
            let k = Self::at(x, y);
            laid(styles, raw[k], mat[k], surf[k])
        };
        self.s.gmix.fill(|x, y| surf[Self::at(x, y)], |g| blend[usize::from(g)], laid, x0 * CELL, y0 * CELL, seed);
    }

    /// The surface at chunk-local px `(x, y)`, which may be up to 16 px outside the chunk.
    #[inline]
    fn surf_px(&self, x: i32, y: i32) -> u8 {
        let (a, b) = (x + CELL, y + CELL);
        if a < 0 || b < 0 || a >= MM || b >= MM {
            return NONE;
        }
        self.s.mm[(b * MM + a) as usize]
    }

    /// Px from chunk-local `(x, y)` to land, at most 16.
    #[inline]
    fn shore_px(&self, x: i32, y: i32) -> i32 {
        let (a, b) = (x + CELL, y + CELL);
        if a < 0 || b < 0 || a >= MM || b >= MM {
            return 16;
        }
        i32::from(self.s.shore[(b * MM + a) as usize])
    }

    /// Resolve the layers, and gather the chunk's water cells, casters and wetness.
    fn finish(&mut self, x0: i32, y0: i32, out: &mut Chunk) {
        let ly = &self.s.ly;
        let l = &mut out.layers;
        // Each pixel in its region's ramps (`region`); the Lowfields' as painted. A wall or a roof
        // takes its cell's region whole; the ground its share of each by the ecotone.
        let regions = &self.s.region;
        let eco = &self.s.eco;
        let lut = &self.lut;
        // A region past the table draws as the Lowfields.
        let reg = |r: u8| if r > 2 { 0 } else { usize::from(r) };
        let lowfields =
            eco.whole == Some(0) && (0..CHUNK_CELLS).all(|y| (0..CHUNK_CELLS).all(|x| regions[Self::at(x, y)] == 0));
        for (i, (dst, &ix)) in l.albedo.iter_mut().zip(&ly.albedo).enumerate() {
            let Some(&(ground, rgb)) = lut.get(usize::from(ix.0)) else {
                *dst = pack(ix);
                continue;
            };
            if lowfields {
                *dst = rgb[0];
                continue;
            }
            let (px, py) = (i as i32 % CHUNK_PX, i as i32 / CHUNK_PX);
            *dst = match eco.whole {
                Some(r) if ground => rgb[reg(r)],
                None if ground => {
                    let (a, b, t) = eco.mix(px, py);
                    let ca = rgb[reg(a)];
                    if t == 0 || a == b { ca } else { mix_rgb(ca, rgb[reg(b)], t) }
                }
                _ => rgb[reg(regions[Self::at(px / CELL, py / CELL)])],
            };
        }
        l.normal.copy_from_slice(&ly.normal);
        l.emissive.copy_from_slice(&ly.emissive);
        l.height.copy_from_slice(&ly.height);
        for y in 0..CHUNK_PX {
            let row = &mut l.water[(y * CHUNK_PX) as usize..((y + 1) * CHUNK_PX) as usize];
            let src = ((y + CELL) * MM + CELL) as usize;
            row.copy_from_slice(&self.s.shore[src..src + CHUNK_PX as usize]);
        }
        out.water.clear();
        for y in 0..CHUNK_CELLS {
            for x in 0..CHUNK_CELLS {
                let k = Self::at(x, y);
                let paint = self.styles.tile(self.s.paint[k]);
                let own = self.style_k(k);
                l.wet[(y * CHUNK_CELLS + x) as usize] = if paint.is_water() {
                    0
                } else if own.row.group == TileGroup::Flora || own.row.group == TileGroup::Ground {
                    paint.row.wet
                } else {
                    own.row.wet
                };
                if paint.is_water() && paint.row.pattern == jane_data::TilePattern::Water {
                    let phase = (crate::hash::h32((x0 + x) as u32, (y0 + y) as u32, salt::WATER) & 63) as u8;
                    out.water.push(WaterCell { x: x as u8, y: y as u8, phase });
                }
            }
        }
        standing::casters(self, x0, y0, out);
    }
}

/// Whether ground `g` drifts into the wild grounds beside it (`ecotone::GroundMix`): a wild
/// ground of a loose pattern; a garden's rows, setts and water keep their edge.
fn blends(styles: &Styles, g: u8) -> bool {
    g != NONE && {
        let st = styles.id(g);
        st.row.wild
            && !st.is_water()
            && matches!(
                st.row.pattern,
                jane_data::TilePattern::Turf
                    | jane_data::TilePattern::Earth
                    | jane_data::TilePattern::Marsh
                    | jane_data::TilePattern::Sand
                    | jane_data::TilePattern::Cracked
            )
    }
}

/// Whether a cell of tile `raw` painted `mat`, drawn as surface `surf`, is ground laid by hand,
/// which keeps its edge where wild ground drifts (`ecotone::GroundMix`): earth the land did not
/// lay (a lane, a walk, a yard, a town's ground: dirt without `Material::WildEarth`), whether the
/// cell is that dirt (even where the speckle filter drew it as the grass round it) or is drawn as
/// it (a stone by a lane borrows the lane's ground, a lone tuft of grass in a yard takes the
/// yard's); and a grown-over path.
fn laid(styles: &Styles, raw: Tile, mat: Option<Material>, surf: u8) -> bool {
    let earth = |row: &jane_data::TileStyle| row.pattern == jane_data::TilePattern::Earth;
    let own = styles.tile(raw).row;
    raw == Tile::GrownPath
        || mat != Some(Material::WildEarth)
            && (own.group == TileGroup::Ground && earth(&own) || surf != NONE && earth(&styles.id(surf).row))
}

/// A cheap hash for the per-pixel paths (a stone of a course, a cluster of an edge): one
/// multiply per field into jane-core's `mix32`. What is picked once a cell uses `h32`.
#[inline]
pub(crate) const fn fast(a: u32, b: u32, salt: u32) -> u32 {
    jane_core::hash::mix32(a.wrapping_mul(0x9e37_79b1) ^ b.wrapping_mul(0x85eb_ca77) ^ salt.wrapping_mul(0xc2b2_ae3d))
}

/// Where the ground is busy (ART-PLAN Q3, "busy beside quiet"): a slow value noise over world
/// cells `(wx, wy)`, a lattice point every eight cells eased between, 0..=255. Scatter (tufts,
/// reeds, stones, flowers) keeps its density where this is high and thins to a fifth where it is
/// low, so the ground reads as patches of growth with clear ground between them, never as an even
/// carpet. Pure in the cell and the seed: two chunks agree on a seam.
pub(crate) fn busy(wx: i32, wy: i32, seed: u32) -> i32 {
    // Stretched about the middle, so most cells are plainly in a patch or plainly out of one.
    (128 + (slow(wx, wy, seed) - 128) * 2).clamp(0, 255)
}

/// The slow noise under [`busy`], unstretched: 0..=255 about 128, rarely near either end. What
/// a rare thing (a worn board, a ledger stone's patch) is gated by.
pub(crate) fn slow(wx: i32, wy: i32, seed: u32) -> i32 {
    const SHIFT: i32 = 3;
    let (gx, gy) = (wx >> SHIFT, wy >> SHIFT);
    let (tx, ty) = (field::smooth((wx & 7) * 32 + 16), field::smooth((wy & 7) * 32 + 16));
    let v = |i: i32, j: i32| (fast(i as u32, j as u32, seed ^ 0x4255_5359) >> 24) as i32;
    let top = v(gx, gy) * (256 - tx) + v(gx + 1, gy) * tx;
    let bottom = v(gx, gy + 1) * (256 - tx) + v(gx + 1, gy + 1) * tx;
    (top * (256 - ty) + bottom * ty) >> 16
}

/// How much of a scatter's density a busy cell keeps, in 16ths: all of it in a patch, a fifth
/// outside one, eased across the patch's edge.
pub(crate) fn busy_share(wx: i32, wy: i32, seed: u32) -> u32 {
    let b = busy(wx, wy, seed);
    (3 + (b - 100).clamp(0, 80) * 13 / 80) as u32
}

/// `a` and `b` (`0xFFRRGGBB`) mixed `t` of 256 toward `b`.
#[inline]
fn mix_rgb(a: u32, b: u32, t: i32) -> u32 {
    let ch = |s: u32| {
        let (x, y) = (((a >> s) & 0xff) as i32, ((b >> s) & 0xff) as i32);
        ((x * (256 - t) + y * t) >> 8) as u32
    };
    0xff00_0000 | ch(16) << 16 | ch(8) << 8 | ch(0)
}

/// `ix` as `0xFFRRGGBB`.
pub fn pack(ix: Ix) -> u32 {
    let [r, g, b] = palette::rgb(ix);
    0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

/// Paint chunk `(cx, cy)` of `src` under world seed `seed` into `out`: the four layers, the
/// strips, the water cells, the casters and the wetness. The same painter, source, seed and chunk
/// give the same bytes; nothing is allocated once `out` and the painter have painted a chunk.
pub fn paint_chunk(painter: &mut Painter, src: &impl TileSource, seed: u32, cx: i32, cy: i32, out: &mut Chunk) {
    painter.paint(src, seed, cx, cy, out);
}
