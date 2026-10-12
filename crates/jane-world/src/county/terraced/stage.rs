//! The terraced county's own stages (PLAN-map-r4 section 1.3), the ones the plain county has no
//! word for: the carve's tiles laid over the land, the railway from the carve's macro line, and the
//! pits mended before the cut through thicket. Every other stage is the plain county's, over the
//! skeleton the adapter made ([`super::skel`]); the proofs ([`super::prove`]) judge that none of them
//! laid anything over the carve's joints.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::blueprint::SpanKind;
use jane_core::search::{Conn, Reach, flood};
use jane_core::{Key, Rect, Tile};

use super::carve::joins::JoinKind;
use super::carve::{Carved, W, check};
use super::macro_plan::{MH, MW, STEP_CELLS};
use super::prove::blocked_words;
use crate::bits::Bits;
use crate::county::County;
use crate::county::finish::cuttable;
use crate::county::rail::{BEND, RAIL_WEST_X, lay_line, line_through};
use crate::skeleton::Region;
use crate::solve::portals::reach_words;

/// Growth, rubble and water the land laid that a made landing or approach must not have standing on
/// it.
fn in_the_way(t: Tile) -> bool {
    matches!(t, Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Hedge | Tile::Rubble | Tile::Fence | Tile::Water)
}

/// The rects the night's barriers stand on, one a closed gate: its join, or for a span the end of
/// the deck.
pub fn night_barriers(carved: &Carved) -> Vec<Rect> {
    carved
        .night_gates
        .iter()
        .filter_map(|&g| {
            carved
                .joins
                .iter()
                .find(|j| j.gate == Some(g) && !matches!(j.kind, JoinKind::Closed(_)))
                .map(|j| j.rect)
                .or_else(|| {
                    carved
                        .spans
                        .iter()
                        .zip(&carved.span_meta)
                        .find(|(_, m)| carved.plan.crossings[usize::from(m.crossing)].gate == Some(g))
                        .map(|(s, _)| s.ends()[0])
                })
        })
        .collect()
}

/// The carve's tiles laid over the land: the land's crag made rubble (a face is the carve's alone),
/// growth cleared from every landing and approach, then each row's faces, rims, water, joins, boards,
/// abutments and track (`Carved::tile_row`). The carve's claims, its levels and spans, and the
/// portals the floods cross are set, and the night's barriers named.
pub fn carve_tiles(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let (w, h) = (c.k.w(), c.k.h());
    debug_assert_eq!((w, h), (W, W));
    let cliff = Tile::Cliff.id();
    let mut ids = vec![0u8; w as usize];
    for y in 0..h {
        c.k.row_ids(0, y, &mut ids);
        for x in (0..w as usize).filter(|&x| ids[x] == cliff) {
            c.k.set(x as i32, y, Tile::Rubble);
        }
    }
    let claims = carved.claims();
    for r in &claims {
        for (x, y) in r.cells() {
            if c.k.inside(x, y) && in_the_way(c.k.get(x, y)) {
                let works = c.sk.region_at(x >> 4, y >> 4) == Region::Works;
                c.k.set(x, y, if works { Tile::Dirt } else { Tile::Grass });
            }
        }
    }
    let mut row: Vec<Option<Tile>> = vec![None; w as usize];
    for y in 0..h {
        row.fill(None);
        carved.tile_row(y, &mut row);
        for (x, t) in row.iter().enumerate() {
            if let Some(t) = *t {
                c.k.set(x as i32, y, t);
            }
        }
    }
    for r in claims {
        c.k.claim(r);
    }
    claim_pockets(c, &carved);
    c.level = Some(carved.level.clone());
    c.spans.clone_from(&carved.spans);
    c.portals = Some(carved.portals());
    for (i, r) in night_barriers(&carved).into_iter().enumerate() {
        let key = c.k.local(&alloc::format!("night_barrier_{i}"));
        c.k.rect(key, r);
    }
    // The east bridge: where the omens count who crosses. The county names it again where the east
    // road itself crosses water; a road that goes the long way round, by the Works, crosses none.
    if let (Some(e), Some(r)) = (jane_data::catalog().county.furnishing.east_road, east_bridge(&carved)) {
        c.k.rect(Key::Name(e.bridge), r);
    }
    c.terraced = Some(carved);
}

/// Open ground the carve leaves no way to, from the Halt, with the spans as built and the ledges
/// crossed (a pocket the faces and the water shut in, a shelf above a ledge no stair climbs to): claimed,
/// so nothing the story walks to is set down there.
fn claim_pockets(c: &mut County<'_>, carved: &Carved) {
    let Some(start) = carved.pads.iter().find(|p| p.site == "station").map(|p| p.boxr.centre()) else { return };
    let open = check::planes(carved).open;
    let reached = check::reach_words(&open, &carved.portals(), &[start], carved.portals().built_broken(), false);
    let words_per_row = W as usize / 64;
    for y in 0..W {
        let mut run: Option<i32> = None;
        for k in 0..words_per_row {
            let at = y as usize * words_per_row + k;
            let shut = open.word(at) & !reached[at];
            if shut == 0 && run.is_none() {
                continue;
            }
            for j in 0..64 {
                let x = (k * 64 + j) as i32;
                match (shut >> j & 1 != 0, run) {
                    (true, None) => run = Some(x),
                    (false, Some(x0)) => {
                        c.k.claim(Rect::new(x0, y, x - x0, 1));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        if let Some(x0) = run {
            c.k.claim(Rect::new(x0, y, W - x0, 1));
        }
    }
}

/// The crossing nearest the road from the Castle to the Museum, a road bridge or viaduct before boards
/// or any other span, as the rect the east bridge's omens read: the deck grown by three cells.
fn east_bridge(carved: &Carved) -> Option<Rect> {
    let plan = &carved.plan;
    let road = plan.routes.iter().position(|r| r.from == "town" && r.to == "museum")?;
    let pts = &carved.ways.get(road)?.pts;
    let near = |r: Rect| {
        let (cx, cy) = r.centre();
        pts.iter().map(|&(x, y)| (x - cx).abs().max((y - cy).abs())).min().unwrap_or(i32::MAX)
    };
    let road_span = |k: SpanKind| matches!(k, SpanKind::Bridge | SpanKind::Viaduct);
    let spans = carved.spans.iter().filter(|s| road_span(s.kind)).map(|s| s.rect);
    let best = |rects: &mut dyn Iterator<Item = Rect>| rects.min_by_key(|&r| (near(r), r.y, r.x));
    best(&mut spans.clone())
        .or_else(|| best(&mut carved.planks.iter().copied()))
        .or_else(|| best(&mut carved.spans.iter().map(|s| s.rect)))
        .map(|r| r.grow(3))
}

/// The railway: the carve's macro line as cell points, its run through the Halt's macro cell taken
/// to the platform's column (the box is the carve's, not pinned to the fence), and the plain county's
/// raster over it. An end that stops short ends at buffers.
pub fn lay_rail(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let line = rail_cells(c, &carved);
    lay_line(c, &line);
    c.terraced = Some(carved);
}

fn rail_cells(c: &County<'_>, carved: &Carved) -> Vec<(i32, i32)> {
    let rail = &carved.rail;
    if rail.len() < 2 {
        return Vec::new();
    }
    let mut xs: Vec<i32> =
        rail.iter().map(|&(mx, _)| if mx == 0 { RAIL_WEST_X } else { mx * STEP_CELLS + STEP_CELLS / 2 }).collect();
    let station = carved.plan.site_cell("station");
    if let (Some(at), Some(pad)) = (
        rail.iter().position(|&q| q == station),
        c.sk.sites.iter().find(|s| s.def.id == "station").and_then(|s| carved.pad_of(s.row)),
    ) {
        // The run of macro cells in the Halt's column, either side of it, is the platform's.
        let platform = pad.boxr.x + 1;
        let (mut lo, mut hi) = (at, at);
        while lo > 0 && rail[lo - 1].0 == station.0 {
            lo -= 1;
        }
        while hi + 1 < rail.len() && rail[hi + 1].0 == station.0 {
            hi += 1;
        }
        xs[lo..=hi].fill(platform);
    }
    let pts: Vec<(i32, i32)> =
        xs.iter().zip(rail).map(|(&x, &(_, my))| (x, my * STEP_CELLS + STEP_CELLS / 2)).collect();
    let on_edge = |(mx, my): (i32, i32)| mx == 0 || my == 0 || mx == MW - 1 || my == MH - 1;
    let ends = [on_edge(rail[0]), on_edge(rail[rail.len() - 1])];
    line_through(c.sk, pts, ends, BEND)
}

/// Rounds of pit repair: each cuts one way out for what the last found shut in.
const PIT_ROUNDS: u32 = 8;
/// Cells a way cut out of a pit may search (the cut through thicket's budget, a few dozen cells of growth).
const PIT_BUDGET: u32 = 400_000;

/// No pits (MAP.md 4.7): wherever she can walk to, down a ledge or over a deck, she can walk back. The land
/// laid growth (a wood, a field of rubble, a hedge) over ground the carve proved open both ways, and a
/// landing hemmed in by it is a pit. Flood from the Halt forward and back across the ledges and spans,
/// as the proofs will; the cells reached but not returned from are shut in, and a way is cut to each
/// from the ground that is not, through growth only (never a face, water or a prop). Before the cut
/// through thicket, which judges the ground as it will be played.
pub fn repair_pits(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let Some(start) = jane_data::catalog()
        .name_id("start")
        .and_then(|id| c.k.blueprint().marks.get(&Key::Name(id)).map(|m| (i32::from(m.cell.x), i32::from(m.cell.y))))
    else {
        c.terraced = Some(carved);
        return;
    };
    let (w, h) = (c.k.w(), c.k.h());
    let words = (w * h) as usize / 64;
    let portals = carved.portals();
    let broken = portals.built_broken();
    for _ in 0..PIT_ROUNDS {
        let blocked = blocked_words(c.k.blueprint());
        let open: Vec<u64> = (0..words).map(|k| !c.k.solid_word(k) & !blocked[k]).collect();
        let fwd = reach_words(w as u32, h as u32, &open, &portals, &[start], broken, false);
        let back = reach_words(w as u32, h as u32, &open, &portals, &[start], broken, true);
        let pit: Vec<u64> = (0..words).map(|k| fwd.word(k) & !back.word(k)).collect();
        if pit.iter().all(|&m| m == 0) || !cut_out(c, &fwd, &back, &pit, &blocked) {
            break;
        }
    }
    c.terraced = Some(carved);
}

/// A way cut from the pit `pit` to the ground that is reached and returned from: the shortest through
/// growth, from a growth cell against the pit to one against, or on, that ground. True if it cut.
fn cut_out(c: &mut County<'_>, fwd: &Bits, back: &Bits, pit: &[u64], blocked: &[u64]) -> bool {
    let (w, h) = (c.k.w(), c.k.h());
    let levels = c.level.is_some();
    let ix = |x: i32, y: i32| y as usize * w as usize + x as usize;
    let bit = |words: &[u64], i: usize| words[i >> 6] >> (i & 63) & 1 != 0;
    let growth = |k: &crate::kit::Kit, x: i32, y: i32| {
        k.inside(x, y) && !bit(blocked, ix(x, y)) && cuttable(levels, k.get(x, y)) && k.solid(x, y)
    };
    let mut starts: Vec<(i32, i32)> = Vec::new();
    for (k, &word) in pit.iter().enumerate() {
        let mut m = word;
        while m != 0 {
            let i = (k << 6) + m.trailing_zeros() as usize;
            m &= m - 1;
            let (x, y) = ((i % w as usize) as i32, (i / w as usize) as i32);
            for (dx, dy) in jane_core::grid::DIRS4 {
                if growth(&c.k, x + dx, y + dy) {
                    starts.push((x + dx, y + dy));
                }
            }
        }
    }
    starts.sort();
    starts.dedup();
    if starts.is_empty() {
        return false;
    }
    let good = |x: i32, y: i32| c.k.inside(x, y) && fwd[ix(x, y)] && back[ix(x, y)];
    let mut out = Reach::new();
    flood(w as u32, h as u32, &starts, Conn::Four, PIT_BUDGET, |x, y| growth(&c.k, x, y) || good(x, y), &mut out);
    let Some((mut x, mut y)) = out
        .order()
        .iter()
        .map(|q| ((q.0 % w as u32) as i32, (q.0 / w as u32) as i32))
        .find(|&(x, y)| good(x, y) && !pit_at(pit, ix(x, y)))
    else {
        return false;
    };
    // Back along the flood to a start, a step nearer each time, taking the growth up.
    let mut d = out.dist(x, y);
    while d > 0 {
        let Some(&(dx, dy)) = jane_core::grid::DIRS4.iter().find(|&&(dx, dy)| out.dist(x + dx, y + dy) == d - 1) else {
            break;
        };
        (x, y, d) = (x + dx, y + dy, d - 1);
        if c.k.solid(x, y) {
            c.k.set(x, y, Tile::Dirt);
        }
    }
    true
}

fn pit_at(pit: &[u64], i: usize) -> bool {
    pit[i >> 6] >> (i & 63) & 1 != 0
}
