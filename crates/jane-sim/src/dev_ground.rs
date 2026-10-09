//! Dev grounds (MAP.md §9 R3): the hand-built test maps of R1 and R2 ([`crate::height::terraces`],
//! [`crate::span::viaduct`]) dressed as places, so a frame shows real cliffs, stairs, ledges,
//! a waterfall and bridges with her on them before the terraced county is carved (R4). A sheet's
//! and a console spike's tool (`jane sheet scene --ground`, the PSP script's `ground:`), never a
//! zone of the game: [`stand_in`] puts one in the county's place for a frame.
//!
//! The dressing is the test map's own cells and levels, then: a region map (so each region's
//! faces and decks are seen), roads and lanes on the joins' lines, trees, shrubs and stones on
//! open ground, water (a stream over a waterfall into a pool on the terraces; a canal beside the
//! towpath under the viaduct), lamps by the joins, and on the viaduct ground a lane in a cutting
//! with an arch over it (a third span). Every join, ledge and span of the test map is kept as it
//! is; nothing is laid on a face, a join or a landing.

use alloc::vec::Vec;

use jane_core::blueprint::{PropSpawn, RegionMap, Span};
use jane_core::{Blueprint, Cell, Plane, Rect, Tile, ZoneId};

/// Which dev ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// R1's terraces: a plateau's long face across three regions, a ledge, a stair, a ladder, a
    /// ramp, a rise, and a waterfall.
    Terraces,
    /// R2's viaduct: a valley under a stone viaduct and a broken footbridge, and an arch over a
    /// lane in a cutting.
    Viaduct,
}

impl Ground {
    /// By its name (`terraces`, `viaduct`).
    pub fn parse(s: &str) -> Option<Ground> {
        match s {
            "terraces" => Some(Ground::Terraces),
            "viaduct" => Some(Ground::Viaduct),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Ground::Terraces => "terraces",
            Ground::Viaduct => "viaduct",
        }
    }
}

/// The dressed ground in `zone`'s blueprint.
pub fn blueprint(g: Ground, zone: ZoneId) -> Blueprint {
    match g {
        Ground::Terraces => terraces(zone),
        Ground::Viaduct => viaduct(zone),
    }
}

/// `bps` with the county's blueprint the dressed ground `g` (seed and every other zone kept).
pub fn stand_in(mut bps: crate::Blueprints, g: Ground) -> crate::Blueprints {
    bps.replace_county(blueprint(g, ZoneId::County));
    bps
}

/// A level plane worked on as bytes.
struct Levels {
    w: i32,
    v: Vec<u8>,
}

impl Levels {
    fn of(bp: &Blueprint) -> Levels {
        let w = bp.w() as i32;
        let v = bp.level.as_ref().map_or_else(|| alloc::vec![1; (bp.w() * bp.h()) as usize], Plane::unpack);
        Levels { w, v }
    }
    fn set(&mut self, r: Rect, l: u8) {
        for (x, y) in r.cells() {
            self.v[(y * self.w + x) as usize] = l;
        }
    }
    fn pack(self, bp: &mut Blueprint) {
        bp.level = Some(Plane::pack(bp.w(), bp.h(), &self.v));
    }
}

fn fill(bp: &mut Blueprint, r: Rect, t: Tile) {
    bp.tiles.fill_rect(r, t);
}

/// Tile `t` on every cell of `cells` that is plain grass now (never a face, a join or a road).
fn scatter(bp: &mut Blueprint, cells: &[(i32, i32)], t: Tile) {
    for &(x, y) in cells {
        if bp.tile(x, y) == Tile::Grass {
            fill(bp, Rect::new(x, y, 1, 1), t);
        }
    }
}

/// A lamp post at each cell (the catalog's `lamp_post`), named `dev_lamp_<i>`.
fn lamps(bp: &mut Blueprint, at: &[(u16, u16)]) {
    let Some(def) = jane_data::catalog().story.prop_id("lamp_post") else { return };
    for (i, &(x, y)) in at.iter().enumerate() {
        let key = bp.local(&alloc::format!("dev_lamp_{i}"));
        let mut p = PropSpawn::new(key, def, Cell::new(x, y));
        p.on = true;
        bp.props.push(p);
    }
}

/// The terraces dressed: the Lowfields west, the Waters in the middle, the Works east, so the one
/// long face shows each region's walling; the ramp's road runs on along the foot and up onto the
/// plateau; a stream crosses the plateau and falls over the face into a pool; a copse, shrubs
/// and stones; lamps at the stair, the ramp and the ladder.
fn terraces(zone: ZoneId) -> Blueprint {
    use crate::height::terraces::{H, RAMP, STAIR, W};
    let mut bp = crate::height::terraces::blueprint(zone);
    // Regions by thirds, on a map of 16-cell squares.
    let mut regions = RegionMap::new(16, (W / 16) as u16, (H / 16) as u16, 0);
    for my in 0..(H / 16) as u16 {
        for mx in 0..(W / 16) as u16 {
            regions.set(
                mx,
                my,
                if mx < 2 {
                    0
                } else if mx < 4 {
                    1
                } else {
                    2
                },
            );
        }
    }
    bp.regions = regions;
    // The ramp's road: on along the foot west, and up on the plateau north then west.
    fill(&mut bp, Rect::new(RAMP.x, RAMP.bottom(), RAMP.w, 4), Tile::Road);
    fill(&mut bp, Rect::new(4, 20, RAMP.right() - 4, 3), Tile::Road);
    fill(&mut bp, Rect::new(RAMP.x + 1, 4, 4, RAMP.y - 4), Tile::Road);
    fill(&mut bp, Rect::new(52, 4, RAMP.x - 51, 3), Tile::Road);
    // The stair's path down to the road.
    fill(&mut bp, Rect::new(STAIR.x, STAIR.bottom(), STAIR.w, 20 - STAIR.bottom()), Tile::Dirt);
    // A stream over the plateau (level 2), over the face (a waterfall, in the Waters), into a pool
    // at the foot (level 1) that drains east under nothing (the pool's edge is its bank).
    let (fx, fw) = (40, 3);
    fill(&mut bp, Rect::new(fx - 3, 2, fw + 6, 3), Tile::Water);
    fill(&mut bp, Rect::new(fx, 5, fw, 9), Tile::Water);
    fill(&mut bp, Rect::new(fx, 14, fw, 2), Tile::Waterfall);
    fill(&mut bp, Rect::new(fx - 2, 16, fw + 4, 3), Tile::Water);
    fill(&mut bp, Rect::new(fx - 1, 19, fw + 2, 1), Tile::Water);
    // A copse on the plateau in the Lowfields; shrubs along the lip; stones at the foot.
    let mut copse = Vec::new();
    for (i, (x, y)) in Rect::new(18, 3, 12, 7).cells().enumerate() {
        let h = jane_core::hash::mix32((x as u32) << 16 ^ y as u32 ^ 0x7e57);
        if h % 3 == 0 && i % 2 == 0 {
            copse.push((x, y));
        }
    }
    scatter(&mut bp, &copse, Tile::Tree);
    scatter(&mut bp, &[(4, 10), (5, 11), (26, 12), (33, 12), (50, 11), (56, 12), (70, 9), (76, 11)], Tile::Bush);
    scatter(&mut bp, &[(66, 3), (72, 6), (78, 2)], Tile::DeadTree);
    scatter(&mut bp, &[(3, 17), (19, 16), (34, 17), (47, 16), (58, 16), (66, 17), (74, 16), (86, 17)], Tile::Rubble);
    scatter(&mut bp, &[(12, 26), (14, 30), (62, 30), (70, 34), (80, 28), (8, 36), (20, 40), (54, 38)], Tile::Bush);
    scatter(&mut bp, &[(6, 32), (22, 33), (44, 42), (60, 44), (88, 40)], Tile::Tree);
    // Lamps: the stair's foot, the ramp's top and foot, the ladder's foot, the pool.
    lamps(&mut bp, &[(79, 17), (87, 11), (87, 19), (59, 17), (45, 19), (30, 18)]);
    bp
}

/// The viaduct dressed: the valley in the Waters (mud and stone banks), the terraces in the
/// Lowfields; a canal beside the towpath with a landing at the rim stair; a lane down a cutting
/// from the north terrace to the towpath with an arch over it (a third span, along x); trees,
/// shrubs and lamps.
fn viaduct(zone: ZoneId) -> Blueprint {
    use crate::span::viaduct::{FACE, H, RIM, RIM_STAIR, TOWPATH, VIADUCT, W};
    let mut bp = crate::span::viaduct::blueprint(zone);
    let mut lv = Levels::of(&bp);
    let mut regions = RegionMap::new(8, (W / 8) as u16, (H / 8) as u16, 0);
    for mx in 0..(W / 8) as u16 {
        for my in 2..4 {
            regions.set(mx, my, 1);
        }
    }
    bp.regions = regions;
    // The canal: the towpath's south half, but a landing at the rim stair.
    let canal = Rect::new(TOWPATH.x, TOWPATH.y + 4, TOWPATH.w, TOWPATH.h - 4);
    fill(&mut bp, canal, Tile::Water);
    fill(&mut bp, Rect::new(RIM_STAIR.x - 2, canal.y, RIM_STAIR.w + 4, canal.h), Tile::Road);
    // Under the viaduct the towpath stays; its piers stand in the canal (the abutments are the
    // rect's end rows, already cliff).
    // The cutting: a lane at level 0 from the towpath north into the terrace, rimmed both sides,
    // its head a face; an arch (a span along x) carries the terrace's path over it.
    let (cx, cw, top) = (20, 4, 6);
    lv.set(Rect::new(cx, top, cw, FACE.y - top), 0);
    fill(&mut bp, Rect::new(cx, top, cw, FACE.bottom() - top), Tile::Road);
    lv.set(Rect::new(cx, FACE.y, cw, 2), 0);
    for x in [cx - 1, cx + cw] {
        fill(&mut bp, Rect::new(x, top, 1, FACE.y - top), Tile::Cliff);
    }
    // The cutting's head: a face two cells, the terrace's level.
    fill(&mut bp, Rect::new(cx - 1, top - 2, cw + 2, 2), Tile::Cliff);
    lv.set(Rect::new(cx - 1, top - 2, cw + 2, 2), 1);
    let arch = Rect::new(cx - 1, 10, cw + 2, 3);
    bp.spans.push(Span { rect: arch, along_x: true, deck_level: 1, broken: false });
    // The terrace's path over the arch, east-west.
    fill(&mut bp, Rect::new(4, arch.y, cx - 5, arch.h), Tile::Road);
    fill(&mut bp, Rect::new(arch.right(), arch.y, VIADUCT.x - arch.right(), arch.h), Tile::Road);
    // The viaduct's road on north and south, so it reads as a way.
    fill(&mut bp, Rect::new(VIADUCT.x, 2, VIADUCT.w, FACE.y - 2), Tile::Road);
    fill(&mut bp, Rect::new(VIADUCT.x, RIM.bottom(), VIADUCT.w, H as i32 - 2 - RIM.bottom()), Tile::Road);
    // Trees and shrubs on the terraces; reeds would be the canal's (its bank is drawn).
    scatter(&mut bp, &[(6, 4), (9, 6), (40, 4), (44, 7), (52, 5), (57, 9), (60, 14), (8, 14)], Tile::Tree);
    scatter(&mut bp, &[(4, 15), (14, 16), (26, 15), (37, 16), (42, 15), (50, 16), (58, 16)], Tile::Bush);
    scatter(&mut bp, &[(6, 34), (12, 38), (22, 42), (40, 33), (48, 40), (58, 36), (26, 34)], Tile::Tree);
    scatter(&mut bp, &[(10, 30), (18, 31), (36, 30), (44, 31), (54, 32), (60, 30)], Tile::Bush);
    lamps(&mut bp, &[(29, 16), (34, 16), (29, 30), (34, 30), (51, 29), (25, 9)]);
    lv.pack(&mut bp);
    bp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_span_of_each_dev_ground_is_well_formed_and_she_starts_on_open_ground() {
        for g in [Ground::Terraces, Ground::Viaduct] {
            let bp = blueprint(g, ZoneId::County);
            let levels = bp.level.clone().expect("levels");
            let grid = crate::grid::ZoneGrid::with_levels(bp.tiles.clone(), levels).with_spans(bp.spans.clone());
            for s in &bp.spans {
                assert!(crate::span::well_formed(&grid, s), "{}: {s:?}", g.name());
            }
            let start = bp.marks.values().next().expect("a start mark").cell;
            assert!(!grid.solid(i32::from(start.x), i32::from(start.y)), "{}", g.name());
        }
    }
}
