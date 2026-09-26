//! What stands in while the art is missing (PORT.md §7.1). Delete this module when the looks
//! land: the chunk painter (`jane_art::terrain::paint_chunk`, ART.md §8 step 3) replaces
//! [`paint_chunk`], and the person, creature and prop looks replace [`StandIns`].
//!
//! - A chunk draws each cell as its tile's flat swatch (PRESENTATION.md §1.6's fallback). For the
//!   lit tiers it also gets a relief the light can find: walls rise as faces to the south, roofs
//!   are pitched east-west, a wood's canopy is lumpy and tall, hedges and fences stand.
//! - A unit draws the step-1 demo `ball`, its plum cloth swapped per kind (her, another seat,
//!   four folk variants, hostile, the dead): a coat swap is an index remap (ART.md §1). Its
//!   height stands it up: each px as high as it is above her feet, as a person's will be.
//! - A prop draws the demo sprite nearest its size, at 1x or half size; a prop with a light
//!   draws the `lamp`.
//!
//! All of it is drawn by `jane-present`, never by `jane-art`. [`tile_rgb`] outlives the rest:
//! `jane view` and `jane play --snap` colour their maps with it, so it moves when this goes.

use jane_art::canvas::normal;
use jane_art::demo;
use jane_art::palette::{Ix, Ramp};
use jane_core::Tile;

use crate::atlas::{Atlas, RefId, Texel};
use crate::frame::{CELL, CHUNK_CELLS, CHUNK_PX, ChunkId, ChunkLayers};

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
        T::HouseRoof => [130, 60, 50],
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
    debug_assert_eq!(layers.albedo.len(), side * side);
    let lit = layers.lit();
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
                layers.albedo[row * side + x0..row * side + x0 + CELL as usize].fill(c);
            }
            if !lit {
                continue;
            }
            let t = |dx: i32, dy: i32| if inside(cx + dx, cy + dy) { tile(cx + dx, cy + dy) } else { Tile::Void };
            for y in 0..CELL {
                for x in 0..CELL {
                    let k = ((j * CELL + y) as usize) * side + (i * CELL + x) as usize;
                    let (n, z) = if inside(cx, cy) { relief(&t, (cx, cy), x, y) } else { ([128, 128], 0) };
                    layers.normal[k] = n;
                    layers.height[k] = z;
                    layers.emissive[k] = 0;
                }
            }
        }
    }
}

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
            // Cells of wall below this one: 0 for the foot of a wall, one more a cell up.
            let below = (1..=2).take_while(|&k| wall_like(t(0, k))).count() as i32;
            if below >= 2 {
                // The wall's top, seen from above: flat and high.
                (normal(0, -20), 40)
            } else {
                let z = below * CELL + (CELL - y);
                (normal(0, 96), z.clamp(1, 40) as u8)
            }
        }
        T::HouseRoof => {
            // The run of roof in this column: pitched along it, the ridge in the middle.
            let up = (1..=12).take_while(|&k| t(0, -k) == T::HouseRoof).count() as i32;
            let down = (1..=12).take_while(|&k| t(0, k) == T::HouseRoof).count() as i32;
            let len = (up + down + 1) * CELL;
            let r = up * CELL + y;
            let mid = len / 2;
            let off = (r - mid) * 127 / mid.max(1);
            let rise = 14 - 14 * (r - mid).abs() / mid.max(1);
            (normal(0, off.clamp(-80, 80)), (30 + rise) as u8)
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
        T::Crops | T::GrassTall => {
            // Rows of growth: ridges running east-west, 4 px apart.
            let ny = [-50, -20, 20, 50][(y & 3) as usize];
            (normal(0, ny), 2)
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
        for name in PROP_DEMOS {
            let c = demo::sprite(name).expect("a demo sprite");
            let (w, h) = (c.w(), c.h());
            props.push((w, h, atlas.add_canvas(&c, (0, h as i16), h as u8, |_, y, t| standing(t, y, h as i16))));
            let hh = (h / 2) as i16;
            props.push((w / 2, h / 2, atlas.add_canvas_half(&c, (0, hh), hh as u8, |_, y, t| standing(t, y, hh))));
        }
        let lamp = demo::lamp();
        let (w, h) = (lamp.w(), lamp.h());
        let hh = (h / 2) as i16;
        let lamps = [
            (w, h, atlas.add_canvas(&lamp, (0, h as i16), h as u8, |_, y, t| standing(t, y, h as i16))),
            (w / 2, h / 2, atlas.add_canvas_half(&lamp, (0, hh), hh as u8, |_, y, t| standing(t, y, hh))),
        ];
        StandIns { units, props, lamps }
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

    /// A prop's stand-in: the demo nearest its box (the footprint, `w x h` cells, plus a cell
    /// of rise unless it is flat), or the lamp nearest it for a prop with a light.
    pub fn prop(&self, w_cells: u8, h_cells: u8, flat: bool, lit: bool) -> RefId {
        let want = (i32::from(w_cells) * CELL, i32::from(h_cells) * CELL + if flat { 0 } else { CELL });
        let near = |&&(w, h, _): &&(i32, i32, RefId)| (w - want.0).abs() + (h - want.1).abs();
        let pick = if lit { self.lamps.iter().min_by_key(near) } else { self.props.iter().min_by_key(near) };
        pick.expect("stand-ins are packed").2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
