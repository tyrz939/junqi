//! The terrain painter behind the chunk cache (PRESENTATION.md §1.6): `jane_art::terrain` reads
//! the zone through the `View` and paints a chunk's four layers into its slot; its trees, shrubs
//! and stones come back as placements the scene draws from the atlas, sorted among the units.
//! Fences and low walls are painted into the ground with the walls (`Standing::Placed`).

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use jane_art::terrain::dungeon::Dungeon;
use jane_art::terrain::houses::{self, House, Houses, Room};
use jane_art::terrain::{self, Chunk, PaintMap, Painter, Placed, Standing, TileSource};
use jane_core::{Material, Tile};
use jane_sim::view::View;

use crate::atlas::{Atlas, Key, RefId, cat};
use crate::frame::{Block, CELL, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers, SURFACE_OUTSIDE, rows_up};
use crate::shadow::RELIEF;

/// A packed paint plane's materials (`jane_core::Packed::paint`): a cell's value less one is the
/// `Material`'s discriminant.
const PACKED_PAINT: [Material; 5] =
    [Material::RoofSlate, Material::RoofThatch, Material::BrickWall, Material::Pine, Material::WildEarth];
const _: () = {
    let mut i = 0;
    while i < PACKED_PAINT.len() {
        assert!(PACKED_PAINT[i] as usize == i, "PACKED_PAINT out of the Material's order");
        i += 1;
    }
};

/// A zone as the painter reads it: the view's tiles, the paint kept beside them.
struct ViewTiles<'v, 'a> {
    view: &'v View<'a>,
    paint: &'v PaintMap,
    houses: &'v Houses,
    room: Option<Room>,
    dungeon: Option<&'v Dungeon>,
    daylight: bool,
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
        // Over packed blueprints (a console's, PORT.md §13.3) the paint is read from its plane,
        // a cell at a time, and no map is kept beside it.
        if let Some(p) = self.view.packed() {
            let (w, h) = self.view.size();
            if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
                return None;
            }
            return PACKED_PAINT.get(usize::from(p.paint.get(x as u32, y as u32)).checked_sub(1)?).copied();
        }
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
    fn house(&self, x: i32, y: i32) -> Option<House> {
        self.houses.at(x, y).copied()
    }
    fn room(&self) -> Option<Room> {
        self.room
    }
    fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon
    }
    fn daylight(&self) -> bool {
        self.daylight
    }
}

/// A flora sprite as the atlas holds it, with what its shadow is thrown as.
#[derive(Clone, Copy, Debug)]
pub struct Flora {
    pub look: RefId,
    /// Its sway (ART-PLAN §9): its one look bent at draw time, `lean` set to the sway's level
    /// (`-3..=3`, [`SWAY_REACH`]), so a level more moves its top a px and the rows under it in a
    /// soft spread (the owner, 2026-10-03: "efficient rust code that just moves or manipulates a
    /// static image"). A crown leans two px at most and only over its upper half; reeds three,
    /// to the foot. [`Bend::NONE`](crate::frame::Bend::NONE) for a thing that does not sway.
    pub bend: crate::frame::Bend,
    /// How it sways: [`SwayClass`].
    pub class: SwayClass,
    /// A broadleaf crown's loose leaf clusters (ART-PLAN §9), three on each edge, west first:
    /// tiny copies of its own texels the wind tips and turns at draw time.
    pub leaves: [Cluster; LEAF_CLUSTERS],
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
    /// The painter and its scratch chunk; away while a [`PaintJob`] has them.
    work: Option<Box<Work>>,
    paint: PaintMap,
    /// The zone's houses (ART-PLAN Q2), its room seen from inside (M4), and whether it is day.
    houses: Houses,
    room: Option<Room>,
    /// The zone read as a dungeon's rooms (ART-PLAN M5, B2), if a theme names it.
    dungeon: Option<Arc<Dungeon>>,
    daylight: bool,
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

/// A sway's levels each way: leans `-3..=3`, a reed's top a px a level.
pub const SWAY_REACH: i32 = 3;

/// Leaf clusters a crown has, both edges.
pub const LEAF_CLUSTERS: usize = 6;

/// A loose leaf cluster on a crown's edge (ART-PLAN §9): where it is in the crown's sprite, its
/// 2 x 2 texels as they are and a turn lighter (a leaf showing its pale underside), and its
/// colour for the leaf that lets go. `side` 0 is no cluster.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cluster {
    /// Its top-left, px from the sprite's top-left.
    pub x: i16,
    pub y: i16,
    /// The edge it is on: -1 west, 1 east, 0 none.
    pub side: i8,
    pub plain: RefId,
    pub lit: RefId,
    pub colour: [u8; 3],
}

/// A crown's leaf clusters: on each edge, three rows spread down its upper part, the outermost
/// leaf texels there copied 2 x 2 into the atlas (four texels each, plain and turned light).
fn clusters(atlas: &mut Atlas, c: &jane_art::Canvas, (top, bend): (i32, i32)) -> [Cluster; LEAF_CLUSTERS] {
    use jane_art::palette::Ramp;
    let leaf = |x: i32, y: i32| {
        let ix = c.get(x, y);
        ix.is_opaque() && Ramp::of(ix).is_some_and(|(r, _)| r.name().starts_with("leaf"))
    };
    let mut out = [Cluster::default(); LEAF_CLUSTERS];
    for (s, side) in [-1i32, 1].into_iter().enumerate() {
        // The rows whose outermost px on this edge is leaf, and that px's column.
        let edge: Vec<(i32, i32)> = (top + 2..bend - 1)
            .filter_map(|y| {
                let mut xs = 0..c.w();
                let x = if side < 0 {
                    xs.find(|&x| c.get(x, y).is_opaque())
                } else {
                    xs.rfind(|&x| c.get(x, y).is_opaque())
                }?;
                leaf(x, y).then_some((x, y))
            })
            .collect();
        if edge.len() < 3 {
            continue;
        }
        for k in 0..3 {
            let (ex, ey) = edge[edge.len() * (2 * k + 1) / 6];
            // The cluster reaches in from the edge.
            let x0 = if side < 0 { ex } else { ex - 1 };
            let texel = |x: i32, y: i32, turn: i32| {
                let (tx, ty) = (x0 + x, ey + y);
                if !leaf(tx, ty) {
                    return crate::atlas::Texel::CLEAR;
                }
                let mut t = crate::atlas::Texel::of(c, tx, ty);
                if let Some((r, tone)) = Ramp::of(t.albedo) {
                    t.albedo = r.at(tone.step(turn));
                }
                t
            };
            let h = (c.height_at(ex, ey)).max(1);
            let plain = atlas.add_texels(2, 2, (0, 0), h, |x, y| texel(x, y, 0));
            let lit = atlas.add_texels(2, 2, (0, 0), h, |x, y| texel(x, y, 2));
            let colour = jane_art::palette::rgb(c.get(ex, ey));
            out[s * 3 + k] = Cluster { x: x0 as i16, y: ey as i16, side: side as i8, plain, lit, colour };
        }
    }
    out
}

/// How a flora sprite sways: its lean's reach and its own pace (`Present::draw` reads the pace).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwayClass {
    /// Not at all (a stone, a dead tree, a garden's boundary).
    Still,
    /// A crown: two px at most at its top, over its upper half; slow.
    Crown,
    /// A shrub, a fern, bracken: two px at its top, down to its foot; brisker.
    Bush,
    /// Reeds and long grass: three px at the tips, down to the foot; quickest.
    Reed,
}

impl SwayClass {
    /// Every class, in its discriminant's order.
    pub const ALL: [SwayClass; 4] = [SwayClass::Still, SwayClass::Crown, SwayClass::Bush, SwayClass::Reed];

    /// Ticks for one sway to and fro in a breeze: about 1.5 to 3 s (the owner, 2026-10-03).
    pub const fn period(self) -> u32 {
        match self {
            SwayClass::Still | SwayClass::Crown => 170,
            SwayClass::Bush => 130,
            SwayClass::Reed => 96,
        }
    }
}

/// A flora sprite's bend (ART-PLAN §9), its lean left to the draw: none at and under the row
/// where it starts (a crown's middle, a shrub's or a reed's foot), and `reach` px at the top of
/// what is drawn at a level of [`SWAY_REACH`], so one level to the next moves the top a px or
/// less and the rows under it in a soft spread. With the top row drawn and where it bends.
fn bend_of(c: &jane_art::Canvas, ay: i32, class: SwayClass) -> (crate::frame::Bend, (i32, i32)) {
    let top = (0..c.h()).find(|&y| (0..c.w()).any(|x| c.get(x, y).is_opaque())).unwrap_or(0);
    let tall = (ay - top).max(1);
    let (from, reach) = match class {
        SwayClass::Still => return (crate::frame::Bend::NONE, (top, ay)),
        SwayClass::Crown => (top + tall * 11 / 20, 2),
        SwayClass::Bush => (ay, 2),
        SwayClass::Reed => (ay, 3),
    };
    // A level of 3 leans the top `reach` px: the whole level at `3 * rows / reach` rows up.
    let rows = (from - top).max(1);
    let span = (SWAY_REACH * rows * 2 + reach) / (2 * reach);
    debug_assert!(from <= 255 && span <= 255, "a flora sprite too tall to bend");
    let bend = crate::frame::Bend { lean: 0, from: from.clamp(0, 255) as u8, span: span.clamp(1, 255) as u8 };
    (bend, (top, from))
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
                atlas.key_next(Key { cat: cat::FLORA, sprite: i as u16, vs: 0, frame: 0 });
                let look = atlas.add_canvas(&s.canvas, (s.ax as i16, s.ay as i16), h.clamp(1, 255) as u8, |_, _, t| t);
                let kind = bank.kind_of(i as u16);
                let rustles = matches!(kind, jane_art::flora::Kind::Reeds | jane_art::flora::Kind::Grass);
                let class = if !kind.sways() {
                    SwayClass::Still
                } else if rustles {
                    SwayClass::Reed
                } else if kind.is_tree() {
                    SwayClass::Crown
                } else {
                    SwayClass::Bush
                };
                let (bend, crown) = bend_of(&s.canvas, s.ay, class);
                // A broadleaf's loose leaves; a pine's needles hold.
                let leaves = if matches!(kind, jane_art::flora::Kind::Green(_) | jane_art::flora::Kind::Turned(_)) {
                    clusters(atlas, &s.canvas, crown)
                } else {
                    [Cluster::default(); LEAF_CLUSTERS]
                };
                // A tree throws its shadow from its trunk; a shrub or a stone from its spread,
                // as deep as it is drawn wide (round, seen from above): a row of bushes planted
                // down the screen, a cell apart, is a hedge in the field as one across it is
                // (at a third of the canvas's width, light passed between them one way and not
                // the other, 2026-09-28).
                let wide = drawn_width(&s.canvas);
                // A garden's boundary is a line a few px deep, not a block as deep as it is long
                // (ART-PLAN M7); what stands in a garden is round.
                let depth = match kind {
                    jane_art::flora::Kind::Garden(g) if g.is_boundary() => 3,
                    jane_art::flora::Kind::Garden(_) => (wide / 2).max(4),
                    _ if name.contains("tree") || name.starts_with("pine") => 6,
                    _ => wide.max(w / 3),
                };
                // What it stands on, rows over its foot (a shrub's rim; its heights are counted
                // from there, `jane_art::flora::base`).
                let lift = (s.ay - jane_art::flora::base(&s.canvas, s.ay)).clamp(0, 255) as u8;
                Flora { look, bend, class, leaves, rustles, depth: depth.clamp(3, 16) as u8, lift }
            })
            .collect();
        Terrain::with_flora(painter, flora, slots)
    }

    /// The painter, with its flora already packed (`flora`, from the presenter's tables on a
    /// console), and `slots` slots.
    pub fn with_flora(mut painter: Painter, flora: Vec<Flora>, slots: usize) -> Terrain {
        painter.set_standing(Standing::Placed);
        Terrain {
            work: Some(Box::new(Work { painter, chunk: Chunk::new() })),
            paint: PaintMap::default(),
            houses: Houses::default(),
            room: None,
            dungeon: None,
            daylight: true,
            flora,
            placed: (0..slots).map(|_| Vec::with_capacity(PLACED)).collect(),
            blocks: (0..slots).map(|_| Vec::with_capacity(BLOCKS)).collect(),
            field: Vec::with_capacity((CHUNK_PX * FIELD_ROWS) as usize),
            runs: Vec::with_capacity(512),
            windows: (0..slots).map(|_| Vec::new()).collect(),
        }
    }

    /// A new zone: its paint read once, its houses found and seeded, its room if it is one.
    pub fn zone(&mut self, view: &View<'_>) {
        match view.packed() {
            // Read from the packed plane as the painter asks (`ViewTiles::material`).
            Some(_) => self.paint.fill((0, 0), &[]),
            None => self.paint.fill(view.size(), view.paint()),
        }
        let cat = jane_data::catalog();
        let (w, h) = view.size();
        let all = jane_core::Rect::new(0, 0, w as i32, h as i32);
        let is_door =
            |p: &jane_sim::state::Prop| cat.sprites.get(usize::from(cat.story.prop(p.def).sprite.0)) == Some(&"door");
        let doors: Vec<(i32, i32, houses::Kind)> = view
            .props_in(all)
            .filter(|p| is_door(p))
            .map(|p| {
                let to = view.prop_spawn(p).and_then(|s| s.to).map(|d| d.zone);
                (i32::from(p.cell.x), i32::from(p.cell.y), houses::door_kind(to))
            })
            .collect();
        self.houses.fill((w as i32, h as i32), |x, y| view.tile(x, y), &doors, view.seed());
        let tall = view.props_in(all).filter(|p| !cat.story.prop(p.def).flat).flat_map(|p| {
            let d = cat.story.prop(p.def);
            (0..i32::from(d.w)).map(move |dx| (i32::from(p.cell.x) + dx, i32::from(p.cell.y)))
        });
        self.room = terrain::room_of(
            view.zone(),
            view.seed(),
            w as i32,
            |x, y| view.tile(x, y).flags() & jane_core::tile::F_SOLID != 0,
            tall.collect::<Vec<_>>().into_iter(),
        );
        let feet: Vec<(i32, i32)> = view
            .props_in(all)
            .flat_map(|p| {
                let d = cat.story.prop(p.def);
                let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
                (0..i32::from(d.w)).flat_map(move |dx| (0..i32::from(d.h)).map(move |dy| (x + dx, y + dy)))
            })
            .collect();
        self.dungeon = terrain::dungeon_of(
            view.zone(),
            view.seed(),
            (w as i32, h as i32),
            |x, y| view.tile(x, y),
            feet.into_iter(),
            |id| view.rect(view.key_sym(jane_core::Key::Name(id))),
            None,
        )
        .map(Arc::new);
    }

    /// The zone read as a dungeon's rooms, if it is one.
    pub fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon.as_deref()
    }

    /// The zone's houses, for the props drawn on them (a door's paint, a chimney's pots).
    pub fn houses(&self) -> &Houses {
        &self.houses
    }

    /// The zone's room, if it is one.
    pub fn room(&self) -> Option<Room> {
        self.room
    }

    /// Whether it is day for a room's windows; true when it has just turned, so a room's chunks
    /// are painted again.
    pub fn set_daylight(&mut self, day: bool) -> bool {
        let turned = self.daylight != day;
        self.daylight = day;
        turned && self.room.is_some()
    }

    /// Paints chunk `id` properly into `layers`, the chunk in `slot`. Cells outside the zone
    /// take `outside`, as the swatches do.
    pub fn paint(&mut self, view: &View<'_>, id: ChunkId, slot: u16, outside: u32, layers: &mut ChunkLayers) {
        let src = ViewTiles {
            view,
            paint: &self.paint,
            houses: &self.houses,
            room: self.room,
            dungeon: self.dungeon.as_deref(),
            daylight: self.daylight,
        };
        let (cx, cy) = (i32::from(id.cx), i32::from(id.cy));
        // The painter is home unless a job has it, and the presenter paints nothing then.
        let Some(mut work) = self.work.take() else { return };
        terrain::paint_chunk(&mut work.painter, &src, view.seed(), cx, cy, &mut work.chunk);
        self.land(&work.chunk, view.size(), id, slot, outside, layers);
        self.work = Some(work);
    }

    /// Whether the painter is home (no [`PaintJob`] has it).
    pub fn home(&self) -> bool {
        self.work.is_some()
    }

    /// A job painting chunk `id` away from the view: the painter goes with it, and the zone round
    /// the chunk as a snapshot, so it may run while the sim steps on (a console's worker, PORT.md
    /// §13.12). `None` while another job has the painter.
    pub fn job(&mut self, view: &View<'_>, id: ChunkId) -> Option<PaintJob> {
        let work = self.work.take()?;
        let (x0, y0) = (i32::from(id.cx) * CHUNK_CELLS - SNAP_MARGIN, i32::from(id.cy) * CHUNK_CELLS - SNAP_MARGIN);
        let side = CHUNK_CELLS + 2 * SNAP_MARGIN;
        let src = ViewTiles {
            view,
            paint: &self.paint,
            houses: &self.houses,
            room: self.room,
            dungeon: self.dungeon.as_deref(),
            daylight: self.daylight,
        };
        let n = (side * side) as usize;
        let (mut tiles, mut mat, mut house) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        for y in y0..y0 + side {
            for x in x0..x0 + side {
                tiles.push(src.tile(x, y));
                mat.push(src.material(x, y));
                house.push(src.house(x, y));
            }
        }
        let snap = Snap {
            bp: Arc::clone(view.blueprints().get(view.zone())),
            county: view.zone() == jane_core::ids::ZoneId::County,
            size: src.size(),
            outdoor: src.outdoor(),
            room: self.room,
            dungeon: self.dungeon.clone(),
            daylight: self.daylight,
            x0,
            y0,
            side,
            tiles,
            mat,
            house,
        };
        Some(PaintJob { id, seed: view.seed(), size: view.size(), work, src: snap })
    }

    /// The painter back from a job (its chunk landed or not).
    pub fn home_again(&mut self, work: Box<Work>) {
        self.work = Some(work);
    }

    /// Lays a painted chunk `id` into `layers`, the chunk in `slot`, for a zone of `(w, h)`
    /// cells. Cells outside the zone take `outside`, as the swatches do.
    pub fn land(
        &mut self,
        chunk: &Chunk,
        (w, h): (u32, u32),
        id: ChunkId,
        slot: u16,
        outside: u32,
        layers: &mut ChunkLayers,
    ) {
        let (cx, cy) = (i32::from(id.cx), i32::from(id.cy));
        let c = &chunk.layers;
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
            layers.fence.copy_from_slice(&chunk.fence_px);
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
        layers.water.extend(chunk.water.iter().map(|w| (w.x, w.y, w.phase)));
        // Beyond the zone's edge: the frame's clear, flat.
        let (w, h) = (w as i32, h as i32);
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
        placed.extend_from_slice(&chunk.placed);
        windows(&c.emissive, &c.height, &mut self.windows[usize::from(slot)]);
        let (zw, zh) = (w * CELL - x0 * CELL, h * CELL - y0 * CELL);
        self.windows[usize::from(slot)].retain(|wd| i32::from(wd.x) < zw && i32::from(wd.y) < zh + CELL);
        let out = &mut self.blocks[usize::from(slot)];
        if layers.has_height() {
            blocks(&layers.height, &chunk.fence_px, &mut self.field, &mut self.runs, out);
            fence_blocks(&chunk.fences, out);
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
    /// Every flora sprite as packed, in the bank's order (the presenter's tables).
    pub fn all_flora(&self) -> &[Flora] {
        &self.flora
    }

    pub fn flora(&self, i: u16) -> Flora {
        self.flora[usize::from(i)]
    }
}

/// Cells round a chunk a [`PaintJob`]'s snapshot holds: past the painter's reach
/// (`terrain::REACH`) and the neighbour it reads beyond it.
const SNAP_MARGIN: i32 = 8;

/// The painter and its scratch chunk, which a [`PaintJob`] takes away and gives back.
#[derive(Debug)]
pub struct Work {
    painter: Painter,
    chunk: Chunk,
}

/// A chunk to paint away from the presenter (PORT.md §13.12): on a console, a worker thread
/// paints while the sim and the frames go on, and the presenter lands it when it comes back.
/// It owns everything it reads; [`run`](Self::run) is the painter's whole work.
#[derive(Debug)]
pub struct PaintJob {
    pub id: ChunkId,
    seed: u32,
    size: (u32, u32),
    work: Box<Work>,
    src: Snap,
}

impl PaintJob {
    /// Paints the chunk (the slow part: about 100 ms on a PSP).
    pub fn run(&mut self) {
        let (cx, cy) = (i32::from(self.id.cx), i32::from(self.id.cy));
        terrain::paint_chunk(&mut self.work.painter, &self.src, self.seed, cx, cy, &mut self.work.chunk);
    }

    /// The painted chunk, the zone's size, and the painter to give back.
    pub fn into_parts(self) -> (Box<Work>, (u32, u32)) {
        (self.work, self.size)
    }
}

impl Work {
    /// The chunk last painted.
    pub fn chunk(&self) -> &Chunk {
        &self.chunk
    }
}

/// The zone round one chunk as the painter reads it, copied out of the view: tiles, paint and
/// houses for the chunk and [`SNAP_MARGIN`] cells round it, the regions from the blueprint (its
/// region map is the runtime's), the zone's room, dungeon and daylight.
#[derive(Debug)]
struct Snap {
    bp: Arc<jane_core::Blueprint>,
    county: bool,
    size: (i32, i32),
    outdoor: bool,
    room: Option<Room>,
    dungeon: Option<Arc<Dungeon>>,
    daylight: bool,
    x0: i32,
    y0: i32,
    side: i32,
    tiles: Vec<Tile>,
    mat: Vec<Option<Material>>,
    house: Vec<Option<House>>,
}

impl Snap {
    #[inline]
    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let (i, j) = (x - self.x0, y - self.y0);
        (i >= 0 && j >= 0 && i < self.side && j < self.side).then(|| (j * self.side + i) as usize)
    }
}

impl TileSource for Snap {
    fn size(&self) -> (i32, i32) {
        self.size
    }
    fn tile(&self, x: i32, y: i32) -> Tile {
        // Past the window: the blueprint's own (the painter never reads so far).
        self.at(x, y).map_or_else(|| self.bp.tile(x, y), |k| self.tiles[k])
    }
    fn material(&self, x: i32, y: i32) -> Option<Material> {
        self.at(x, y).and_then(|k| self.mat[k])
    }
    fn outdoor(&self) -> bool {
        self.outdoor
    }
    fn region(&self, x: i32, y: i32) -> u8 {
        if !self.county {
            return 0;
        }
        match jane_sim::living::region_at(&self.bp, x, y) {
            jane_data::Region::Lowfields => 0,
            jane_data::Region::Waters => 1,
            jane_data::Region::Works => 2,
        }
    }
    fn house(&self, x: i32, y: i32) -> Option<House> {
        self.at(x, y).and_then(|k| self.house[k])
    }
    fn room(&self) -> Option<Room> {
        self.room
    }
    fn dungeon(&self) -> Option<&Dungeon> {
        self.dungeon.as_deref()
    }
    fn daylight(&self) -> bool {
        self.daylight
    }
}

crate::tables::tab_struct!(Flora { look, bend, class, leaves, rustles, depth, lift });
crate::tables::tab_struct!(Cluster { x, y, side, plain, lit, colour });
crate::tables::tab_enum!(SwayClass, SwayClass::ALL);

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
