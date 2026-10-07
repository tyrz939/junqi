//! The roads and the footpaths. The skeleton's routes become strokes through the centres of their
//! macro cells, a lane of metal with a trodden verge each side laid with a round brush; a road
//! over water is a bridge of planks. The burial footpath and `data/paths.json`'s footpaths are
//! laid the same way, centre to centre, so the chunks overwrite their ends and the link lanes
//! bring them round to a gate. Where a footpath meets or crosses a road, the road keeps its metal.

use alloc::vec::Vec;
use jane_core::{Sfc32, Tile};
use jane_data::Via;

use super::{County, Footpath, centre};
use crate::kit::stroke_line;
use crate::skeleton::{ROAD_LIT, RoadEnd, SKEL_H, SKEL_W};
use crate::steps::Step;

/// The metal, in cells. A lane, not a trunk road: two people pass, a cart takes all of it.
pub const ROAD_WIDTH: i32 = 3;
/// Trodden verge each side of it.
pub const VERGE: i32 = 1;

/// A road's end as a dice row: a site by its row, the railway as -1.
fn end_code(e: RoadEnd) -> i32 {
    match e {
        RoadEnd::Site(i) => i32::from(i),
        RoadEnd::Rail => -1,
    }
}

/// The roads: the skeleton's, stroked, with verges and bridges. Refines skeleton data, so it
/// throws the skeleton's attempt, one stream per road by its two ends.
pub fn lay_roads(c: &mut County<'_>) {
    let sk = c.sk;
    c.before = Some(c.k.tiles().clone());
    let half = (ROAD_WIDTH >> 1) + VERGE;
    // The brush's two discs, radius + a quarter, compared in sixteenths: d * 16 <= (4r + 1)^2.
    let metal = (4 * (ROAD_WIDTH >> 1) + 1).pow(2);
    let verge = (4 * half + 1).pow(2);
    let first = c.lines.len();
    for r in &sk.roads {
        let pts: Vec<(i32, i32)> = r.cells.iter().map(|&(x, y)| (centre(x), centre(y))).collect();
        let mut rng = c.k.dice_at(Step::CountyRoad, sk.attempt, end_code(r.from), end_code(r.to));
        // The centre line first, then the metal and the verge painted round it with a ROUND
        // brush: a square brush drawn along a diagonal makes a road half as wide again as the same
        // road running straight. The verge is laid along the road's own line, so it never wanders
        // off it, and never over water: a bridge is the road and nothing else.
        let line = c.k.stroke(&mut rng, &pts, 1, Tile::Road, 1);
        c.k.round_brush(&line, half, |t, d| {
            if d * 16 <= metal {
                Some(Tile::Road)
            } else if d * 16 <= verge && t != Tile::Road && t != Tile::Water {
                Some(Tile::Dirt)
            } else {
                None
            }
        });
        let lit = line
            .iter()
            .map(|&(x, y)| sk.road.read((x >> 4).min(SKEL_W - 1), (y >> 4).min(SKEL_H - 1), 0) & ROAD_LIT != 0)
            .collect();
        c.lines.push(line);
        c.lit.push(lit);
    }
    // Where a road crosses water it is a bridge, and a bridge is a deck of planks, not a stripe
    // of road on the river.
    let before = c.before.as_ref().expect("set above");
    let tiles = c.k.tiles_mut();
    for line in &c.lines[first..] {
        for &(x, y) in line {
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let (cx, cy) = (x + ox, y + oy);
                    if before.read(cx, cy, Tile::Void) == Tile::Water && tiles.read(cx, cy, Tile::Void) == Tile::Road {
                        tiles.set(cx, cy, Tile::Boardwalk);
                    }
                }
            }
        }
    }
}

/// The footpaths: the burial's from the graveyard (the Burial Chamber is off the road on purpose,
/// and a footpath from the graveyard finds it, and only that), then `data/paths.json`'s, site to
/// site by way of a named small place or patch. New ground, not skeleton data: the county's
/// attempt, one stream per path (the burial's row 0, a footpath's its row plus one).
pub fn lay_paths(c: &mut County<'_>) {
    let sk = c.sk;
    let site = |id: &str| sk.sites.iter().find(|s| s.def.id == id);
    if let (Some(grave), Some(burial)) = (site("graveyard"), site("burial")) {
        let mut rng = c.k.dice(Step::CountyPath, 0, 0);
        let pts = [(centre(grave.mx), centre(grave.my)), (centre(burial.mx), centre(burial.my))];
        let line = footpath(c, &mut rng, &pts, 2);
        c.lines.push(line);
        c.lit.push(Vec::new());
    }
    for (row, p) in jane_data::catalog().county.paths.iter().enumerate() {
        let via = match p.via {
            Via::Anchor(i) => sk.anchors.iter().find(|a| a.row == i).map(|a| (a.mx, a.my)),
            Via::Area(i) => sk.area(i).map(|a| (a.mx, a.my)),
        };
        let (Some(from), Some(to), Some(via)) =
            (sk.sites.get(usize::from(p.from)), sk.sites.get(usize::from(p.to)), via)
        else {
            continue;
        };
        let mut rng = c.k.dice(Step::CountyPath, row as i32 + 1, 0);
        let pts =
            [(centre(from.mx), centre(from.my)), (centre(via.0), centre(via.1) + 5), (centre(to.mx), centre(to.my))];
        let line = footpath(c, &mut rng, &pts, i32::from(p.width));
        c.footpaths.push(Footpath { row, line: c.lines.len() });
        c.lines.push(line);
        c.lit.push(Vec::new());
    }
}

/// A footpath's stroke: trodden dirt `width` wide, wobbling up to two cells off its course, laid
/// under any road it meets (the TypeScript laid it over, and the path from the graveyard wiped
/// the end of the graveyard's road).
fn footpath(c: &mut County<'_>, rng: &mut Sfc32, pts: &[(i32, i32)], width: i32) -> Vec<(i32, i32)> {
    let line = stroke_line(rng, pts, 2);
    c.k.square_brush(&line, width, |t| (!matches!(t, Tile::Road | Tile::Boardwalk)).then_some(Tile::Dirt));
    let half = width / 2;
    for &(x, y) in &line {
        for cy in y - half..y - half + width {
            for cx in x - half..x - half + width {
                if c.k.inside(cx, cy) && c.k.get(cx, cy) == Tile::Dirt {
                    super::ways::tread(&mut c.trodden, &c.k, cx, cy);
                }
            }
        }
    }
    c.ways.push(super::ways::Way { kind: super::ways::WayKind::Footpath, line: line.clone(), door: None });
    line
}
