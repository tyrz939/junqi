//! What stands in while the art is missing (PORT.md §7.1). Delete this module when the looks
//! land: the chunk painter (`jane_art::terrain::paint_chunk`, ART.md §8 step 3) replaces
//! [`paint_chunk`], and the person, creature and prop looks replace [`StandIns`].
//!
//! - A chunk draws each cell as its tile's flat swatch (PRESENTATION.md §1.6's fallback).
//! - A unit draws the step-1 demo `ball`, its plum cloth swapped per kind (her, another seat,
//!   four folk variants, hostile, the dead): a coat swap is an index remap (ART.md §1).
//! - A prop draws the demo sprite nearest its size, at 1x or half size; a prop with a light
//!   draws the `lamp`.
//!
//! All of it is drawn by `jane-present`, never by `jane-art`. [`tile_rgb`] outlives the rest:
//! `jane view` and `jane play --snap` colour their maps with it, so it moves when this goes.

use jane_art::demo;
use jane_art::palette::{Ix, Ramp};
use jane_core::Tile;

use crate::atlas::{Atlas, RefId};
use crate::frame::{CELL, CHUNK_CELLS, CHUNK_PX, ChunkId};

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

/// Paints chunk `id` as flat swatches into `px` (`CHUNK_PX` square, `0xAARRGGBB`), reading the
/// ground through `tile(cx, cy)`. Cells outside the zone (`w x h` cells) take `outside`.
pub fn paint_chunk(id: ChunkId, (w, h): (u32, u32), outside: u32, tile: impl Fn(i32, i32) -> Tile, px: &mut [u32]) {
    let side = CHUNK_PX as usize;
    debug_assert_eq!(px.len(), side * side);
    for j in 0..CHUNK_CELLS {
        for i in 0..CHUNK_CELLS {
            let (cx, cy) = (i32::from(id.cx) * CHUNK_CELLS + i, i32::from(id.cy) * CHUNK_CELLS + j);
            let c = if cx < w as i32 && cy < h as i32 {
                let [r, g, b] = tile_rgb(tile(cx, cy));
                0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            } else {
                outside
            };
            let x0 = (i * CELL) as usize;
            for y in 0..CELL as usize {
                let row = (j * CELL) as usize + y;
                px[row * side + x0..row * side + x0 + CELL as usize].fill(c);
            }
        }
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
        let units = COATS.map(|coat| atlas.add_canvas(&ball, feet, 40, |ix| swap(ix, Ramp::ClothPlum, coat)));
        let mut props = Vec::new();
        for name in PROP_DEMOS {
            let c = demo::sprite(name).expect("a demo sprite");
            let (w, h) = (c.w(), c.h());
            props.push((w, h, atlas.add_canvas(&c, (0, h as i16), 16, |ix| ix)));
            props.push((w / 2, h / 2, atlas.add_canvas_half(&c, (0, (h / 2) as i16), 8)));
        }
        let lamp = demo::lamp();
        let (w, h) = (lamp.w(), lamp.h());
        let lamps = [
            (w, h, atlas.add_canvas(&lamp, (0, h as i16), 48, |ix| ix)),
            (w / 2, h / 2, atlas.add_canvas_half(&lamp, (0, (h / 2) as i16), 24)),
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
        let mut px = vec![0u32; (CHUNK_PX * CHUNK_PX) as usize];
        // A 20 x 20 zone of grass with one road cell at (17, 18): chunk (1, 1) holds it.
        let tile = |x: i32, y: i32| if (x, y) == (17, 18) { Tile::Road } else { Tile::Grass };
        paint_chunk(ChunkId { cx: 1, cy: 1 }, (20, 20), 0xff00_0001, tile, &mut px);
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
}
