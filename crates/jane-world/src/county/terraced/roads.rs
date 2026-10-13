//! The terraced county's roads and footpaths (PLAN-map-r4 sections 5.1 and 5.2), laid along the lines the
//! source gives (`data/world/county/routes.txt`): nothing is searched for. The metal and its verge go down
//! with a round brush as the plain county's do, but never over a face, water, a stair, a ledge or the
//! ground under a deck; a footpath is trodden dirt, under any road it meets.

use alloc::vec::Vec;
use jane_core::Tile;
use jane_core::blueprint::Span;

use super::carve::{Carved, W};
use crate::bits::Bits;
use crate::county::roads::{ROAD_WIDTH, VERGE};
use crate::county::ways::{Way, WayKind};
use crate::county::{County, Footpath};
use crate::skeleton::ROAD_LIT;

/// Ground the metal and the verge are never laid over: the land's own joints, water, the rails, the
/// stairs and stones, boards.
fn kept(t: Tile) -> bool {
    matches!(
        t,
        Tile::Cliff
            | Tile::Water
            | Tile::Waterfall
            | Tile::LedgeN
            | Tile::LedgeE
            | Tile::LedgeS
            | Tile::LedgeW
            | Tile::Void
            | Tile::Rail
            | Tile::Track
            | Tile::Stair
            | Tile::Ladder
            | Tile::Stepping
            | Tile::Boardwalk
    )
}
/// The metal and the verge of a line, a round brush along it: the metal `ROAD_WIDTH` across and a verge
/// of trodden dirt `VERGE` beyond it, never over a face, water, a stair or a ledge, boards, rails or the
/// ground under a deck.
fn lay_metal(c: &mut County<'_>, line: &[(i32, i32)], decks: &[Span]) {
    let half = (ROAD_WIDTH >> 1) + VERGE;
    // The brush's two discs, radius + a quarter, compared in sixteenths: d * 16 <= (4r + 1)^2.
    let metal = (4 * (ROAD_WIDTH >> 1) + 1).pow(2);
    let verge = (4 * half + 1).pow(2);
    for &(x, y) in line {
        for oy in -half..=half {
            for ox in -half..=half {
                let (cx, cy) = (x + ox, y + oy);
                if !c.k.inside(cx, cy) {
                    continue;
                }
                let t = c.k.get(cx, cy);
                if kept(t) || decks.iter().any(|s| s.rect.contains(cx, cy)) {
                    continue;
                }
                let d = ox * ox + oy * oy;
                if d * 16 <= metal {
                    c.k.set(cx, cy, Tile::Road);
                } else if d * 16 <= verge && t != Tile::Road {
                    c.k.set(cx, cy, Tile::Dirt);
                }
            }
        }
    }
}

/// Where the ground was water before any road (`County::before`): the carve's water, the boards over
/// it and the ground under each deck, so a crossing of any of them is a bridge to the lamps.
fn before_of(carved: &Carved) -> Bits {
    let mut before = carved.water.bits.clone();
    let w = W as usize;
    let rects = carved.planks.iter().copied().chain(carved.spans.iter().map(|s| s.rect));
    for r in rects {
        for y in r.y.max(0)..r.bottom().min(W) {
            let row = y as usize * w;
            before.fill(row + r.x.max(0) as usize..row + r.right().min(W) as usize, true);
        }
    }
    before
}

/// The roads: each of the plan's routes laid along its line from the source, with its metal and verge,
/// in the order of the skeleton's roads.
pub fn lay_roads(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let src = c.source.expect("the terraced county is laid from its source");
    let sk = c.sk;
    c.before = Some(before_of(&carved));
    for line in &src.routes.roads {
        lay_metal(c, line, &carved.spans);
        let lit = line.iter().map(|&(x, y)| sk.road.read(x >> 4, y >> 4, 0) & ROAD_LIT != 0).collect();
        c.lines.push(line.clone());
        c.lit.push(lit);
    }
    c.terraced = Some(carved);
}

/// The footpaths: the burial's from the graveyard (two cells wide), then `data/paths.json`'s at their
/// widths, each along its line from the source, as trodden dirt under any road it meets.
pub fn lay_paths(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let src = c.source.expect("the terraced county is laid from its source");
    let rows = jane_data::catalog().county.paths;
    for p in &src.routes.paths {
        match p.row {
            None => tread_line(c, &p.line, 2, &carved.spans),
            Some(row) => {
                tread_line(c, &p.line, i32::from(rows[usize::from(row)].width), &carved.spans);
                c.footpaths.push(Footpath { row: usize::from(row), line: c.lines.len() });
            }
        }
        c.lines.push(p.line.clone());
        c.lit.push(Vec::new());
    }
    c.terraced = Some(carved);
}
/// Trodden dirt `width` across along a footpath's line, under any road it meets, never over a face,
/// water, a stair or the ground under a deck; the cells trodden are kept clear of what comes after
/// (`ways::claim_trodden`), and the path is a way for `ways::clear_ways`.
fn tread_line(c: &mut County<'_>, line: &[(i32, i32)], width: i32, decks: &[Span]) {
    let half = width / 2;
    for &(x, y) in line {
        for cy in y - half..y - half + width {
            for cx in x - half..x - half + width {
                if !c.k.inside(cx, cy) || decks.iter().any(|s| s.rect.contains(cx, cy)) {
                    continue;
                }
                let t = c.k.get(cx, cy);
                if kept(t) || matches!(t, Tile::Road | Tile::Boardwalk) {
                    continue;
                }
                c.k.set(cx, cy, Tile::Dirt);
                crate::county::ways::tread(&mut c.trodden, &c.k, cx, cy);
            }
        }
    }
    c.ways.push(Way { kind: WayKind::Footpath, line: line.to_vec(), door: None });
}
