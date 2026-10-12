//! The terraced county's roads and footpaths (PLAN-map-r4 sections 5.1 and 5.2). A road is the plan's
//! route as the carve's cell waypoints, each pair of them joined by a short search over the ground as
//! it is (a road never climbs a face: it takes a ramp, or a deck), or by a straight run where the
//! carve laid a ramp or a deck and its approaches. The metal and its verge go down with a round
//! brush as the plain county's do, but never over a face, water, a stair, a ledge or the ground under
//! a deck. Footpaths are routed the same way on foot, which may take a stair.

use alloc::vec::Vec;
use jane_core::blueprint::Span;
use jane_core::num::octile10;
use jane_core::search::{Astar, PathEnd, PathQuery};
use jane_core::{Rect, Tile};
use jane_data::Via;

use super::carve::ways::RoadWay;
use super::carve::{Carved, W};
use crate::bits::Bits;
use crate::county::roads::{ROAD_WIDTH, VERGE};
use crate::county::ways::{Way, WayKind};
use crate::county::{County, Footpath, centre};
use crate::kit::js_round;
use crate::skeleton::ROAD_LIT;

/// How a search may move: a road never takes a stair; a foot does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Road,
    Foot,
}

/// Cells a search may look beyond the box of its two ends.
const MARGIN: i32 = 32;

/// What a step costs: straight, diagonal, and more through growth (a road clears it, but goes round if
/// it can).
const STEP: u32 = 10;
const DIAG: u32 = 14;
const GROWTH: u32 = 10;
/// How far a waypoint on ground it cannot use is moved to find ground it can.
const SNAP: i32 = 8;

/// The ground a search never enters: the land's own joints, water, and the rails. A foot may take
/// the stairs and the stones; a road may not.
fn passable(t: Tile, mode: Mode) -> bool {
    match t {
        Tile::Cliff
        | Tile::Water
        | Tile::Waterfall
        | Tile::LedgeN
        | Tile::LedgeE
        | Tile::LedgeS
        | Tile::LedgeW
        | Tile::Void
        | Tile::Rail
        | Tile::Track => false,
        Tile::Stair | Tile::Ladder | Tile::Stepping => mode == Mode::Foot,
        _ => true,
    }
}

/// Growth a road or a path clears.
fn growth(t: Tile) -> bool {
    matches!(t, Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Hedge | Tile::Rubble | Tile::Fence)
}

/// Ground the metal and the verge are never laid over.
fn kept(t: Tile) -> bool {
    !passable(t, Mode::Foot) || matches!(t, Tile::Stair | Tile::Ladder | Tile::Stepping | Tile::Boardwalk)
}

/// A windowed A* kept between legs.
#[derive(Debug)]
pub struct Router {
    astar: Astar,
    out: Vec<(i32, i32)>,
    /// The side of the window it can search, in cells.
    side: i32,
}

impl Router {
    /// A router whose searches may cover a box `side` cells across: a road's leg is a few macro cells
    /// (128 holds it), a footpath's runs from one site to another (512).
    pub fn new(side: u32) -> Self {
        Self { astar: Astar::new(side, side), out: Vec::new(), side: side as i32 }
    }

    /// The cheapest way from `a` to `b` over the ground as it lies, `a` first, or `None`: a search in
    /// the box of the two ends and a margin, as wide a margin as the router's window holds. On foot a
    /// goal the window does not hold, or ground does not reach, is gone toward as far as the ground
    /// goes (a path may stop short); a road wants all of the way or none.
    pub fn route(
        &mut self,
        c: &County<'_>,
        decks: &[Span],
        a: (i32, i32),
        b: (i32, i32),
        mode: Mode,
    ) -> Option<Vec<(i32, i32)>> {
        let (w, h) = (c.k.w(), c.k.h());
        let mut margin = MARGIN;
        let whole = Rect::new(0, 0, w, h);
        let window = loop {
            let (x0, y0) = (a.0.min(b.0) - margin, a.1.min(b.1) - margin);
            let r =
                Rect::new(x0, y0, a.0.max(b.0) + margin - x0 + 1, a.1.max(b.1) + margin - y0 + 1).intersect(whole)?;
            if r.w <= self.side && r.h <= self.side {
                break r;
            }
            margin -= 8;
            if margin < 0 {
                // Too far for the window: the window about the start, the goal beyond it.
                if mode == Mode::Road {
                    return None;
                }
                let (x0, y0) =
                    ((a.0 - self.side / 2).clamp(0, w - self.side), (a.1 - self.side / 2).clamp(0, h - self.side));
                break Rect::new(x0, y0, self.side, self.side);
            }
        };
        let q = PathQuery {
            grid_w: w as u32,
            grid_h: h as u32,
            start: a,
            goal: b,
            max_cost: u32::MAX,
            budget: window.area() as u32,
            partial: mode == Mode::Foot,
            window: Some(window),
            cut_corners: false,
        };
        let step = |from: (i32, i32), to: (i32, i32)| -> Option<u32> {
            let t = c.k.get(to.0, to.1);
            if !passable(t, mode) || decks.iter().any(|s| s.rect.contains(to.0, to.1)) {
                return None;
            }
            let base = if from.0 != to.0 && from.1 != to.1 { DIAG } else { STEP };
            Some(base + if growth(t) { GROWTH } else { 0 })
        };
        // A foot crosses a whole deck from one end to the other: a jump over the cells between.
        let jump = |from: (i32, i32), to: (i32, i32)| -> Option<((i32, i32), u32)> {
            if mode != Mode::Foot {
                return None;
            }
            decks.iter().filter(|s| !s.broken).find_map(|s| {
                let [e0, e1] = s.ends();
                [(e0, e1), (e1, e0)].into_iter().find_map(|(near, far)| {
                    if !(near.contains(from.0, from.1) && s.rect.contains(to.0, to.1)) {
                        return None;
                    }
                    let (dx, dy) = (far.x - near.x, far.y - near.y);
                    Some(((from.0 + dx, from.1 + dy), (dx.abs() + dy.abs()) as u32 * STEP))
                })
            })
        };
        let goal = b;
        let h = move |(x, y): (i32, i32)| octile10(goal.0 - x, goal.1 - y);
        match self.astar.find_with_jumps(&q, step, jump, h, &mut self.out) {
            PathEnd::Found | PathEnd::Partial => {
                let mut cells = Vec::with_capacity(self.out.len() + 1);
                cells.push(a);
                cells.extend_from_slice(&self.out);
                Some(cells)
            }
            PathEnd::None => None,
        }
    }
}

/// Every cell of the straight line from `a` to `b`, in `max(|dx|, |dy|)` steps.
pub fn straight(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    if steps == 0 {
        return alloc::vec![a];
    }
    let along = |p: i32, q: i32, n: i32| p + js_round(i64::from(q - p) * i64::from(n), i64::from(steps)) as i32;
    (0..=steps).map(|n| (along(a.0, b.0, n), along(a.1, b.1, n))).collect()
}

/// The nearest cell to `p`, within [`SNAP`], that `mode` may enter and stands at `p`'s level; `p`
/// itself if it will do.
fn snap(c: &County<'_>, carved: &Carved, p: (i32, i32), mode: Mode) -> Option<(i32, i32)> {
    if !c.k.inside(p.0, p.1) {
        return None;
    }
    let want = carved.level_at(p.0, p.1);
    let ok = |x: i32, y: i32| {
        c.k.inside(x, y)
            && passable(c.k.get(x, y), mode)
            && carved.level_at(x, y) == want
            && !carved.spans.iter().any(|s| s.rect.contains(x, y))
    };
    for r in 0..=SNAP {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox.abs().max(oy.abs()) == r && ok(p.0 + ox, p.1 + oy) {
                    return Some((p.0 + ox, p.1 + oy));
                }
            }
        }
    }
    None
}

/// The cells of one leg: a search over the ground, or the straight line when the search finds no way
/// (the metal then stops at what it must not cross).
fn leg(
    c: &County<'_>,
    carved: &Carved,
    router: &mut Router,
    a: (i32, i32),
    b: (i32, i32),
    mode: Mode,
) -> Vec<(i32, i32)> {
    router.route(c, &carved.spans, a, b, mode).unwrap_or_else(|| straight(a, b))
}

/// A way's line: its waypoints snapped to ground, each pair joined by a search, or by a straight run
/// where the carve marked one.
fn way_line(c: &County<'_>, carved: &Carved, router: &mut Router, way: &RoadWay) -> Vec<(i32, i32)> {
    let pts: Vec<(i32, i32)> = way.pts.iter().map(|&p| snap(c, carved, p, Mode::Road).unwrap_or(p)).collect();
    let mut line: Vec<(i32, i32)> = Vec::new();
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let run = way.straight.iter().any(|&(f, l)| usize::from(f) == k - 1 && usize::from(l) == k);
        let cells = if run { straight(a, b) } else { leg(c, carved, router, a, b, Mode::Road) };
        line.extend(cells.into_iter().skip(usize::from(!line.is_empty())));
    }
    if line.is_empty() {
        line.extend(pts.first().copied());
    }
    line
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

/// The roads: the plan's routes as the carve's waypoints, each laid as a line with its metal and
/// verge, in the order of the skeleton's roads.
pub fn lay_roads(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let sk = c.sk;
    c.before = Some(before_of(&carved));
    let mut router = Router::new(128);
    for way in &carved.ways {
        let line = way_line(c, &carved, &mut router, way);
        lay_metal(c, &line, &carved.spans);
        let lit = line.iter().map(|&(x, y)| sk.road.read(x >> 4, y >> 4, 0) & ROAD_LIT != 0).collect();
        c.lines.push(line);
        c.lit.push(lit);
    }
    c.terraced = Some(carved);
}

/// The footpaths: the burial's from the graveyard, then `data/paths.json`'s, site to site by way of a
/// named small place or patch. Routed on foot, so a path takes a stair where the ground has one, and laid
/// as trodden dirt under any road it meets.
pub fn lay_paths(c: &mut County<'_>) {
    let Some(carved) = c.terraced.take() else { return };
    let sk = c.sk;
    let site = |id: &str| sk.sites.iter().find(|s| s.def.id == id);
    let mut router = Router::new(512);
    if let (Some(grave), Some(burial)) = (site("graveyard"), site("burial")) {
        let pts = [(centre(grave.mx), centre(grave.my)), (centre(burial.mx), centre(burial.my))];
        if let Some(line) = foot_line(c, &carved, &mut router, &pts) {
            tread_line(c, &line, 2, &carved.spans);
            c.lines.push(line);
            c.lit.push(Vec::new());
        }
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
        let pts =
            [(centre(from.mx), centre(from.my)), (centre(via.0), centre(via.1) + 5), (centre(to.mx), centre(to.my))];
        let Some(line) = foot_line(c, &carved, &mut router, &pts) else { continue };
        tread_line(c, &line, i32::from(p.width), &carved.spans);
        c.footpaths.push(Footpath { row, line: c.lines.len() });
        c.lines.push(line);
        c.lit.push(Vec::new());
    }
    c.terraced = Some(carved);
}

/// A footpath's line from `pts`, the places it leaves, goes by and ends at, each snapped to ground a foot
/// may stand on: from the first toward the middle one, and from the last toward it, the second back to
/// front, so each end is at its place. Where the ground does not take a leg all the way (a goal too far
/// for the window, a stair the plan has not laid) it stops where the ground stops; `None` if neither leg
/// takes a step.
fn foot_line(c: &County<'_>, carved: &Carved, router: &mut Router, pts: &[(i32, i32)]) -> Option<Vec<(i32, i32)>> {
    let pts: Vec<(i32, i32)> = pts.iter().map(|&p| snap(c, carved, p, Mode::Foot)).collect::<Option<_>>()?;
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let mid = if pts.len() > 2 { pts[1] } else { last };
    let mut line = router.route(c, &carved.spans, first, mid, Mode::Foot).unwrap_or_default();
    if pts.len() > 2 {
        let mut back = router.route(c, &carved.spans, last, mid, Mode::Foot).unwrap_or_default();
        back.reverse();
        let skip = usize::from(line.last().is_some() && line.last() == back.first());
        line.extend(back.into_iter().skip(skip));
    }
    (line.len() > 1).then_some(line)
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
