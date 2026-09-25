//! Anchors: small places the story needs, as opposed to the ones the seed happens to roll. "The
//! well on the road from the station", "the scarecrow at the farm gate", "the shrine beside the last
//! lamp that still works". A quest is written against a name, so these get names, constraints and a
//! guarantee: a county that cannot place them all is re-rolled, the same as one that cannot place
//! the Museum. Carries `jane/src/world/skeleton/anchors.ts`.
//!
//! An anchor becomes a small place of its kind (dressed like any other; it sits on top of the
//! region's rolled budget), and the county gives it a mark and a rect named after it.

use std::cmp::Reverse;

use jane_core::NameId;
use jane_core::grid::Grid;
use jane_data::{AnchorDef, Region};

use super::place::{PlacedArea, PlacedPoi, PlacedSite};
use super::rank::{rank_base, ranked};
use super::roads::{ROAD, ROAD_LIT, Road, RoadEnd};
use super::terrain::Terrain;
use super::types::{SKEL_H, SKEL_W, Water, cell_of, inside, metres_sq, nearer_than, xy_of};
use crate::steps::Step;

/// A small place the story needs, where it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedAnchor {
    /// Its row in the anchor table.
    pub row: u8,
    /// A kind of small place, or `None` for a bare spot.
    pub kind: Option<NameId>,
    pub mx: i32,
    pub my: i32,
}

/// What the anchors read of the county built so far, and the two things they change: the lamps
/// along the road the last lamp is on, and the small places (an anchor displaces rolled ones).
#[derive(Debug)]
pub struct AnchorCtx<'a> {
    pub t: &'a Terrain,
    pub road: &'a mut Grid<u8>,
    pub roads: &'a [Road],
    pub sites: &'a [PlacedSite],
    pub areas: &'a [PlacedArea],
    pub pois: &'a mut Vec<PlacedPoi>,
    /// The first walk (station, Julie's, town). Its lamps are never put out, whatever else shares its cells.
    pub safe: &'a Grid<bool>,
    /// The railway and the cells beside it: an anchor never stands on the line.
    pub near_rail: &'a Grid<bool>,
    /// The roads `lastLamp` never picks: the first walk's (station to Julie's, Julie's to town).
    pub first_walk: [(RoadEnd, RoadEnd); 2],
}

/// Anchors keep this far apart unless one is placed `after` another (m).
const APART: i64 = 48;
/// Macro cells of dark after the last lamp: about 350 m.
const DARK_RUN: usize = 22;

/// Where an anchor that walks a road is along it: which road, and which cell of it.
#[derive(Clone, Copy, Debug)]
struct Along {
    road: usize,
    index: usize,
}

/// Solve the rows in order. `None` if any cannot be placed: the caller re-rolls the county.
pub fn place_anchors(ctx: &mut AnchorCtx<'_>, rows: &[AnchorDef], seed: u32, attempt: u8) -> Option<Vec<PlacedAnchor>> {
    let mut out: Vec<PlacedAnchor> = Vec::new();
    let mut along: Vec<Option<Along>> = vec![None; rows.len()];
    let road_of = |a: u8, b: u8| {
        let (a, b) = (RoadEnd::Site(u16::from(a)), RoadEnd::Site(u16::from(b)));
        ctx.roads.iter().position(|r| (r.from == a && r.to == b) || (r.from == b && r.to == a))
    };
    let site = |i: u8| ctx.sites.iter().find(|s| s.row == i);
    let area = |i: u8| ctx.areas.iter().find(|a| a.row == i);

    // Worked out once for the whole map: the rows below ask these of every cell, several times over.
    let mut open_mask = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    let mut roadside = Grid::new(SKEL_W as u32, SKEL_H as u32, false);
    for y in 2..SKEL_H - 2 {
        for x in 2..SKEL_W - 2 {
            if ctx.road.read(x, y, 0) & ROAD != 0 {
                for oy in -2..=2 {
                    for ox in -2..=2 {
                        roadside.set(x + ox, y + oy, true);
                    }
                }
                continue;
            }
            if ctx.t.water.read(x, y, Water::Dry) != Water::Dry || ctx.near_rail.read(x, y, false) {
                continue;
            }
            // Never inside a set chunk: the chunk would clear it away.
            let clear = ctx.sites.iter().all(|s| {
                let keep10 = (i64::from(s.def.hub.unwrap_or(0)) * 9).max(560);
                !nearer_than(x, y, s.mx, s.my, keep10)
            });
            open_mask.set(x, y, clear);
        }
    }
    let open = |x: i32, y: i32| inside(x, y) && open_mask.read(x, y, false);
    // Open cells off a road cell, the nearest ring first. Two macro cells off the road is 32 cells or
    // more: off the screen of somebody walking past, so the far ring only backs the near one up.
    let beside = |(cx, cy): (i32, i32), rings: i32| -> Vec<u32> {
        let mut near = Vec::new();
        let mut far = Vec::new();
        for oy in -rings..=rings {
            for ox in -rings..=rings {
                if (ox, oy) == (0, 0) || !open(cx + ox, cy + oy) {
                    continue;
                }
                let list = if ox.abs().max(oy.abs()) <= 1 { &mut near } else { &mut far };
                list.push(cell_of(cx + ox, cy + oy));
            }
        }
        near.extend(far);
        near
    };

    for (ix, row) in rows.iter().enumerate() {
        let w = &row.at;
        let mut candidates: Vec<u32>;

        if let Some(after) = w.after {
            let from = along.get(usize::from(after.anchor)).copied().flatten()?;
            let cells = &ctx.roads[from.road].cells;
            let index = (from.index + usize::from(after.steps)).min(cells.len().saturating_sub(2));
            let near = beside(cells[index], 1);
            candidates = if near.len() >= 3 { near } else { beside(cells[index], 2) };
            candidates.truncate(8);
            along[ix] = Some(Along { road: from.road, index });
        } else if w.last_lamp {
            // The longest road of the row's region that is not the first walk. Roads run parent to
            // child, which is away from the town.
            let mut pool: Vec<usize> = (0..ctx.roads.len())
                .filter(|&i| {
                    let r = &ctx.roads[i];
                    let own =
                        r.cells.iter().filter(|&&(x, y)| ctx.t.region.read(x, y, Region::Lowfields) == row.region);
                    !ctx.first_walk.contains(&(r.from, r.to)) && own.count() * 10 > r.cells.len() * 7
                })
                .collect();
            pool.sort_by_key(|&i| (Reverse(ctx.roads[i].cells.len()), i));
            let ri = *pool.first()?;
            let cells = &ctx.roads[ri].cells;
            if cells.len() < 12 {
                return None;
            }
            // Roads merge, so this one may share its first stretch with the first walk, whose lamps are
            // never put out. Start the dark where the next few cells are this road's own.
            let mut index = if cells.len() >= DARK_RUN + 10 { cells.len() * 2 / 5 } else { 3 };
            let shared = |k: usize| cells.iter().take(k + 10).skip(k + 1).any(|&(x, y)| ctx.safe.read(x, y, false));
            while index < cells.len() - 10 && shared(index) {
                index += 1;
            }
            if shared(index) {
                return None;
            }
            for (k, &(x, y)) in cells.iter().enumerate() {
                let bits = ctx.road.read(x, y, 0);
                if k == index {
                    ctx.road.set(x, y, bits | ROAD_LIT);
                } else if k > index && k <= index + DARK_RUN && !ctx.safe.read(x, y, false) {
                    ctx.road.set(x, y, bits & !ROAD_LIT);
                }
            }
            candidates = beside(cells[index], 2);
            along[ix] = Some(Along { road: ri, index });
        } else {
            let road = match w.road {
                Some((a, b)) => Some(road_of(a, b)?),
                None => None,
            };
            let in_area = match w.area {
                Some(i) => Some(area(i)?),
                None => None,
            };
            let rim = match w.rim {
                Some(r) => Some((area(r.area)?, site(r.toward)?)),
                None => None,
            };
            let near_area = match w.near_area {
                Some(i) => Some(area(i)?),
                None => None,
            };
            let dist_to = match w.dist {
                Some(d) => Some((site(d.to)?, d)),
                None => None,
            };
            let near_anchor = match w.near_anchor {
                Some(n) => Some((*out.iter().find(|a| a.row == n.anchor)?, n)),
                None => None,
            };

            let scan = |c: u32| -> bool {
                let (x, y) = xy_of(c);
                if !open(x, y) {
                    return false;
                }
                if near_area.is_none() && rim.is_none() && ctx.t.region.read(x, y, Region::Lowfields) != row.region {
                    return false;
                }
                if let Some((s, band)) = dist_to {
                    let d2 = metres_sq(x, y, s.mx, s.my);
                    let lo = i64::from(band.min.unwrap_or(0));
                    if d2 < lo * lo || band.max.is_some_and(|m| d2 > i64::from(m) * i64::from(m)) {
                        return false;
                    }
                }
                if let Some((a, band)) = near_anchor {
                    let d2 = metres_sq(x, y, a.mx, a.my);
                    if d2 < i64::from(band.min).pow(2) || d2 > i64::from(band.max).pow(2) {
                        return false;
                    }
                }
                if let Some(a) = in_area {
                    // Inside seven tenths of its radius.
                    if metres_sq(x, y, a.mx, a.my) * 100 > (i64::from(a.def.radius) * 7).pow(2) {
                        return false;
                    }
                }
                if let Some((a, _)) = rim {
                    let d2 = metres_sq(x, y, a.mx, a.my);
                    let r = i64::from(a.def.radius);
                    if d2 < (r - 26).max(0).pow(2) || d2 > (r + 10).pow(2) {
                        return false;
                    }
                }
                near_area.is_none() || roadside.read(x, y, false)
            };
            candidates = match road {
                // In cell order, so the seed's pick is stable; the near ring alone when it offers enough.
                Some(ri) => {
                    let cells = &ctx.roads[ri].cells;
                    let mut near: Vec<u32> = cells.iter().flat_map(|&c| beside(c, 1)).collect();
                    near.sort();
                    near.dedup();
                    let mut pool = if near.len() >= 8 {
                        near
                    } else {
                        let mut all: Vec<u32> = cells.iter().flat_map(|&c| beside(c, 2)).collect();
                        all.sort();
                        all.dedup();
                        all
                    };
                    pool.retain(|&c| scan(c));
                    pool
                }
                None => (0..cell_of(0, SKEL_H)).filter(|&c| scan(c)).collect(),
            };

            // "Nearest" rules keep the best few rather than any: the rim toward a site, the roadside
            // nearest a patch.
            let nearest_to = |list: &mut Vec<u32>, tx: i32, ty: i32, keep: usize| {
                list.sort_by_key(|&c| {
                    let (x, y) = xy_of(c);
                    (metres_sq(x, y, tx, ty), c)
                });
                list.truncate(keep);
            };
            if let Some((_, s)) = rim {
                nearest_to(&mut candidates, s.mx, s.my, 6);
            }
            if let Some(s) = w.nearest.and_then(site) {
                nearest_to(&mut candidates, s.mx, s.my, 3);
            }
            if let Some(a) = near_area {
                nearest_to(&mut candidates, a.mx, a.my, 6);
            }
        }

        // Spacing: from every other anchor, and the row's own `apart`.
        candidates.retain(|&c| {
            let (x, y) = xy_of(c);
            out.iter().all(|a| {
                let need = match w.apart {
                    Some(ap) if ap.from == a.row => i64::from(ap.min),
                    _ if w.after.is_some() || w.near_anchor.is_some() => 0,
                    _ => APART,
                };
                !nearer_than(x, y, a.mx, a.my, need * 10)
            })
        });
        // Each anchor ranks its own candidates: a cell ruled out moves it only if it was the pick.
        let c = ranked(rank_base(seed, Step::SkelAnchor, attempt, ix as i32, 0), &candidates)?;
        let (mx, my) = xy_of(c);
        out.push(PlacedAnchor { row: ix as u8, kind: row.kind, mx, my });
    }

    // An anchor is a small place. Whatever the seed rolled within 100 m of one gives way to it.
    ctx.pois.retain(|p| !out.iter().any(|a| nearer_than(a.mx, a.my, p.mx, p.my, 1000)));
    for a in &out {
        let row = &rows[usize::from(a.row)];
        ctx.pois.push(PlacedPoi {
            kind: row.kind,
            name: row.name,
            mx: a.mx,
            my: a.my,
            region: ctx.t.region.read(a.mx, a.my, Region::Lowfields),
            anchor: Some(a.row),
        });
    }
    Some(out)
}
