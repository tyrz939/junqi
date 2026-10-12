//! The carve's checks (PLAN-map-r4 section 6.1), run on the tiles-only sketch: C-1 shape, C-2 spans,
//! C-3 water, C-4 reach and no pits, C-5 gates, C-6 ledges, C-7 the first walk. A plan or salt that
//! fails any is re-rolled. Nothing here reads a tile the carve did not lay.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::blueprint::Span;
use jane_core::search::Fill;
use jane_core::tile::F_SOLID;
use jane_core::{Rect, Tile};

use super::blocks::shape_issue;
use super::joins::JoinKind;
use super::water::{STRAIGHT, STRAIGHT_RAMP, border_dist, isqrt, region_at};
use super::{Carved, W};
use crate::bits::Bits;
use crate::county::terraced::macro_check::LEDGE_MIN_BACK;
use crate::county::terraced::macro_plan::{Border, GateKind, Over, Reg, STEP_CELLS, Verb, idx};
use crate::solve::portals::{Crossing, Portals, spread};

/// What is wrong with a carve: a check's code and the words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub code: &'static str,
    pub text: String,
}

fn issue(code: &'static str, text: String) -> Issue {
    Issue { code, text }
}

/// A ledge's walk back from its landing to its top is at most this many cells on the macro walking graph
/// (R0's `LEDGE_MAX_BACK`: the macro graph counts a step as 16 cells, and its walk is that of an ideal
/// path, the plan's 337 cells a cell-level walk at 3.75 cells a second that no R0-clean plan keeps to).
pub const LEDGE_BACK_MAX: i32 = crate::county::terraced::macro_check::LEDGE_MAX_BACK;
/// The hubs a ledge must land by: the Halt, Julie's, the Castle, the Museum (the Halt has none to need).
const HUBS: [&str; 4] = ["station", "julie_house", "town", "museum"];
/// A hub's ledge lands within this many cells (about 8 s).
pub const HUB_LEDGE: i32 = 60;
/// Region borders keep this many passable gates, at least `ROAD_GATES` of them by road.
pub const BORDER_GATES: usize = 3;
pub const ROAD_GATES: usize = 2;

const WORDS: usize = (W * W) as usize / 64;

const WPR: usize = W as usize / 64;
type Words = [u64; WPR];

/// The sketch's cells as the checks read them: open (not solid), and open on level one; and C-1's cell
/// rule: the walkable cells (no join's) standing above lower walkable ground with no face between,
/// counted word-parallel (`holes`, the first at `first_hole`).
#[derive(Debug)]
pub struct Planes {
    pub open: Bits,
    pub first: Bits,
    pub holes: u32,
    pub first_hole: Option<(i32, i32)>,
}

/// One row's cells as masks: walkable, walkable and not a join's, and level at least 1, 2, 3.
#[derive(Clone, Copy)]
struct RowMasks {
    walk: Words,
    free: Words,
    g: [Words; 3],
}

impl RowMasks {
    const EMPTY: Self = Self { walk: [0; WPR], free: [0; WPR], g: [[0; WPR]; 3] };

    fn of(row: &[Option<Tile>], lrow: &[u8]) -> Self {
        let mut m = Self::EMPTY;
        for k in 0..WPR {
            let tiles = &row[k * 64..k * 64 + 64];
            let levels = &lrow[k * 64..k * 64 + 64];
            // Most of a row is bare ground of one level: those chunks need no cell-by-cell reading.
            if tiles.iter().all(Option::is_none) {
                m.walk[k] = !0;
                m.free[k] = !0;
            } else {
                let (mut walk, mut free) = (0u64, 0u64);
                for (j, t) in tiles.iter().enumerate() {
                    let solid = t.is_some_and(|t| t.flags() & F_SOLID != 0);
                    let join = matches!(t, Some(Tile::Stair | Tile::Ladder | Tile::Road | Tile::Boardwalk));
                    walk |= u64::from(!solid) << j;
                    free |= u64::from(!solid && !join) << j;
                }
                m.walk[k] = walk;
                m.free[k] = free;
            }
            let l0 = levels[0];
            if levels.iter().all(|&l| l == l0) {
                for (g, lv) in m.g.iter_mut().zip(1u8..) {
                    g[k] = if l0 >= lv { !0 } else { 0 };
                }
            } else {
                let (mut g1, mut g2, mut g3) = (0u64, 0u64, 0u64);
                for (j, &l) in levels.iter().enumerate() {
                    g1 |= u64::from(l >= 1) << j;
                    g2 |= u64::from(l >= 2) << j;
                    g3 |= u64::from(l >= 3) << j;
                }
                m.g[0][k] = g1;
                m.g[1][k] = g2;
                m.g[2][k] = g3;
            }
        }
        m
    }

    /// Walkable cells of a level below `l` (1 to 3).
    fn low(&self, l: usize, k: usize) -> u64 {
        self.walk[k] & !self.g[l - 1][k]
    }
}

/// Build the planes a row at a time from the carve's own tiles and levels.
pub fn planes(c: &Carved) -> Planes {
    let w = W as usize;
    let mut open = vec![0u64; WPR * w];
    let mut first = vec![0u64; WPR * w];
    let mut row: Vec<Option<Tile>> = vec![None; w];
    let mut lrow = vec![0u8; w];
    let mut load = |y: i32| -> RowMasks {
        row.fill(None);
        c.tile_row(y, &mut row);
        c.level.row(0, y as u32, &mut lrow);
        RowMasks::of(&row, &lrow)
    };
    let (mut prev, mut cur) = (RowMasks::EMPTY, load(0));
    let (mut holes, mut first_hole) = (0u32, None);
    for y in 0..W {
        let next = if y + 1 < W { load(y + 1) } else { RowMasks::EMPTY };
        let base = y as usize * WPR;
        for k in 0..WPR {
            open[base + k] = cur.walk[k];
            first[base + k] = cur.walk[k] & cur.g[0][k] & !cur.g[1][k];
            let mut hole = 0u64;
            for l in 1..=3usize {
                let above = if l < 3 { cur.g[l][k] } else { 0 };
                let own = cur.free[k] & cur.g[l - 1][k] & !above;
                if own == 0 {
                    continue;
                }
                let here = cur.low(l, k);
                let left = (here << 1) | if k > 0 { cur.low(l, k - 1) >> 63 } else { 0 };
                let right = (here >> 1) | if k + 1 < WPR { cur.low(l, k + 1) << 63 } else { 0 };
                hole |= own & (prev.low(l, k) | next.low(l, k) | left | right);
            }
            if hole != 0 {
                holes += hole.count_ones();
                first_hole.get_or_insert(((k * 64) as i32 + hole.trailing_zeros() as i32, y));
            }
        }
        prev = cur;
        cur = next;
    }
    let n = w * w;
    Planes { open: Bits::from_words(open, n), first: Bits::from_words(first, n), holes, first_hole }
}

/// What one flood reached from `starts` across the portals: cells as words. The solver's own fill
/// (`solve::portals::spread`) over the sketch's open cells.
pub fn reach_words(open: &Bits, p: &Portals, starts: &[(i32, i32)], broken: u64, backward: bool) -> Vec<u64> {
    let cross = Crossing { portals: p, broken, backward };
    let (reached, _) = spread(W as u32, W as u32, &mut |k| open.word(k), &cross, starts, &mut Fill::direct());
    (0..WORDS).map(|k| reached.word(k)).collect()
}

fn bit(words: &[u64], (x, y): (i32, i32)) -> bool {
    x >= 0 && y >= 0 && x < W && y < W && {
        let i = (y * W + x) as usize;
        words[i >> 6] >> (i & 63) & 1 != 0
    }
}

/// The spans' broken bits as built.
fn built_broken(spans: &[Span]) -> u64 {
    spans.iter().enumerate().fold(0, |b, (i, s)| b | u64::from(s.broken) << i)
}

/// The pad of site `id`'s centre.
fn pad_centre(c: &Carved, id: &str) -> Option<(i32, i32)> {
    c.pads.iter().find(|p| p.site == id).map(|p| p.boxr.centre())
}

/// Every check, on the carve's sketch. Empty when the carve holds.
pub fn check(c: &Carved) -> Vec<Issue> {
    check_with(c, false)
}

/// The checks in the order of their cost, each only if the ones before it held when `early`: the first
/// stage with something wrong is the whole answer then.
pub fn check_with(c: &Carved, early: bool) -> Vec<Issue> {
    let mut out = Vec::new();
    shape(c, &mut out);
    if early && !out.is_empty() {
        return out;
    }
    spans(c, &mut out);
    water(c, &mut out);
    gates(c, &mut out);
    ledges(c, &mut out);
    if early && !out.is_empty() {
        return out;
    }
    let pl = planes(c);
    holes(&pl, &mut out);
    if early && !out.is_empty() {
        return out;
    }
    reach(c, &pl, &mut out);
    out
}

// --- C-1 -------------------------------------------------------------------------------------------

/// C-1: the levels' shape rules on the blocks.
fn shape(c: &Carved, out: &mut Vec<Issue>) {
    if let Some((rule, at)) = shape_issue(&c.blocks) {
        out.push(issue("shape", format!("{rule} at {at:?}")));
    }
    // A set place's box stands on one level: a join or a bay cut into a pad would put a face through it.
    for pad in &c.pads {
        if !crate::county::terraced::one_level(&c.level, pad.boxr) {
            out.push(issue("shape", format!("the {} pad's box crosses a face", pad.site)));
        }
    }
}

/// C-1's cell rule: every level step is walled (a face or rim tile, or a join, a ledge or a fall in its
/// place), read off the planes.
fn holes(pl: &Planes, out: &mut Vec<Issue>) {
    if pl.holes > 0 {
        out.push(issue(
            "shape",
            format!("{} walkable cells above lower ground with no face, the first at {:?}", pl.holes, pl.first_hole),
        ));
    }
}

// --- C-2 -------------------------------------------------------------------------------------------

fn spans(c: &Carved, out: &mut Vec<Issue>) {
    if c.spans.len() > jane_core::blueprint::SPANS_MAX {
        out.push(issue("spans", format!("{} spans", c.spans.len())));
    }
    let mut tiles: Vec<Option<Tile>> = vec![None; W as usize];
    for (i, s) in c.spans.iter().enumerate() {
        let r = s.rect;
        let n = if s.along_x { r.h } else { r.w };
        if !(2..=4).contains(&n) || s.length() > 40 || s.length() < 2 {
            out.push(issue("spans", format!("span {i} is {n} wide and {} long", s.length())));
            continue;
        }
        let (a, e) = if s.along_x {
            (Rect::new(r.x, r.y, 1, r.h), Rect::new(r.right() - 1, r.y, 1, r.h))
        } else {
            (Rect::new(r.x, r.y, r.w, 1), Rect::new(r.x, r.bottom() - 1, r.w, 1))
        };
        let solid = |q: Rect, tiles: &mut Vec<Option<Tile>>| {
            q.cells().all(|(x, y)| {
                tiles.fill(None);
                c.tile_row(y, tiles);
                tiles[x as usize].is_some_and(|t| t.flags() & F_SOLID != 0)
            })
        };
        if !solid(a, &mut tiles) || !solid(e, &mut tiles) {
            out.push(issue("spans", format!("span {i}: an end row is not solid")));
        }
        for end in s.ends() {
            let ok = end.cells().all(|(x, y)| {
                tiles.fill(None);
                c.tile_row(y, &mut tiles);
                tiles[x as usize].is_none_or(|t| t.flags() & F_SOLID == 0) && c.level_at(x, y) == s.deck_level
            });
            if !ok {
                out.push(issue("spans", format!("span {i}: an end is not open ground at level {}", s.deck_level)));
            }
        }
        // Under it: water or lower ground (or the cutting's floor), never open ground at the deck's level.
        let under_bad = r.cells().any(|(x, y)| {
            let on_end = a.contains(x, y) || e.contains(x, y);
            if on_end {
                return false;
            }
            tiles.fill(None);
            c.tile_row(y, &mut tiles);
            let wet = tiles[x as usize] == Some(Tile::Water);
            let open = tiles[x as usize].is_none_or(|t| t.flags() & F_SOLID == 0);
            !wet && open && c.level_at(x, y) >= s.deck_level
        });
        if under_bad {
            out.push(issue("spans", format!("span {i}: open ground at its level under it")));
        }
    }
    for (i, s) in c.spans.iter().enumerate() {
        for (j, t) in c.spans.iter().enumerate().skip(i + 1) {
            if s.rect.grow(2).overlaps(t.rect) {
                out.push(issue("spans", format!("spans {i} and {j} within two cells")));
            }
        }
    }
}

// --- C-3 -------------------------------------------------------------------------------------------

fn water(c: &Carved, out: &mut Vec<Issue>) {
    let plan = &c.plan;
    let ch = &c.water.channel;
    if ch.is_empty() {
        out.push(issue("water", String::from("no river")));
        return;
    }
    // Levels never increase along the flow.
    for r in 0..2u8 {
        let pts: Vec<_> = ch.iter().filter(|p| p.reach == r).collect();
        for w in pts.windows(2) {
            let (a, b) = (c.level_at(w[0].x, w[0].y), c.level_at(w[1].x, w[1].y));
            if b > a {
                out.push(issue("water", format!("the river rises from level {a} to {b} at ({}, {})", w[1].x, w[1].y)));
                break;
            }
        }
    }
    // The crossings' zones (the channel's samples within the straight window of one) and the fades at
    // each reach's ends: windows over them are straight by design.
    let mut zones: Vec<(u8, usize)> = Vec::new();
    for cr in &plan.crossings {
        if cr.over != Over::River {
            continue;
        }
        let mid = (i32::from(cr.at.0) * STEP_CELLS + 8, i32::from(cr.at.1) * STEP_CELLS + 8);
        let nearest = ch.iter().enumerate().min_by_key(|(_, p)| ((p.x - mid.0).abs() + (p.y - mid.1).abs(), p.x, p.y));
        if let Some((k, p)) = nearest {
            let local = ch[..k].iter().filter(|q| q.reach == p.reach).count();
            zones.push((p.reach, local));
        }
    }
    let zone_k = ((STRAIGHT + STRAIGHT_RAMP) / super::water::STEP) as usize + 2;
    let near_zone = |p: &super::water::RiverPt| {
        zones.iter().any(|&(r, k)| {
            r == p.reach && {
                let local = ch.iter().take_while(|q| !core::ptr::eq(*q, p)).filter(|q| q.reach == r).count();
                local.abs_diff(k) <= zone_k
            }
        })
    };
    // Straightness: in every window of 48 cells of the centreline the line strays 3 cells from its chord.
    for r in 0..2u8 {
        let pts: Vec<_> = ch.iter().filter(|p| p.reach == r).collect();
        let n = 12usize;
        let mut i = 0;
        while i + n < pts.len() {
            let win = &pts[i..=i + n];
            let in_zone = zones.iter().any(|&(zr, k)| zr == r && i <= k + zone_k && i + n + zone_k >= k);
            let at_end = i < 8 || i + n + 8 >= pts.len();
            let exempt = in_zone || at_end;
            if !exempt {
                let (p0, p1) = (win[0], win[n]);
                let (dx, dy) = ((p1.x - p0.x) as i64, (p1.y - p0.y) as i64);
                let len = isqrt(dx * dx + dy * dy).max(1);
                let stray = win
                    .iter()
                    .map(|p| ((p.x - p0.x) as i64 * dy - (p.y - p0.y) as i64 * dx).abs() / len)
                    .max()
                    .unwrap_or(0);
                if stray < 3 {
                    out.push(issue("water", format!("the river runs straight for 48 cells at ({}, {})", p0.x, p0.y)));
                    break;
                }
            }
            i += 4;
        }
    }
    // Held off the region borders: at most a tenth of the line within six cells of one (the crossings and the
    // spring excepted).
    let mut near = 0;
    let mut counted = 0;
    for p in ch {
        if near_zone(p) {
            continue;
        }
        counted += 1;
        let me = region_at(plan, &c.blocks, p.x, p.y);
        let (mx, my) = ((p.x / 16).clamp(0, 95), (p.y / 16).clamp(0, 95));
        let intended = plan.reg_at(mx, my);
        if me == Reg::Works || intended == Reg::Works {
            continue;
        }
        if border_dist(plan, &c.blocks, p.x, p.y, intended) <= 6 {
            near += 1;
        }
    }
    if counted > 0 && near * 10 > counted {
        out.push(issue("water", format!("{near} of {counted} river points hug a border or a pad")));
    }
    // Every water cell is water, a fall in the Works, a plank or stepping stones; a fall outside the Works is a rim grazed.
    let mut row: Vec<Option<Tile>> = vec![None; W as usize];
    let mut bad = 0;
    let mut first: Option<(i32, i32)> = None;
    for y in 0..W {
        let base = y as usize * (W as usize / 64);
        if (0..W as usize / 64).all(|k| c.water.bits.word(base + k) == 0) {
            continue;
        }
        row.fill(None);
        c.tile_row(y, &mut row);
        for (x, &t) in (0..W).zip(&row) {
            if !c.water.at(x, y) {
                continue;
            }
            let fine = matches!(t, Some(Tile::Water | Tile::Boardwalk | Tile::Stepping))
                || (t == Some(Tile::Waterfall) && region_at(plan, &c.blocks, x, y) == Reg::Works);
            if !fine {
                bad += 1;
                first.get_or_insert((x, y));
            }
        }
    }
    if bad > 0 {
        out.push(issue("water", format!("{bad} water cells are not water, the first at {first:?}")));
    }
}

// --- C-5 -------------------------------------------------------------------------------------------

/// What a plan gate is on the carve: a way across (and by road), or not built.
fn gate_way(c: &Carved, gi: usize) -> Option<bool> {
    let g = &c.plan.gates[gi];
    let gi8 = gi as u8;
    if let Some(j) = c
        .joins
        .iter()
        .find(|j| j.gate == Some(gi8) && matches!(j.kind, JoinKind::Stair | JoinKind::Ramp | JoinKind::Stones))
    {
        return Some(j.kind == JoinKind::Ramp);
    }
    if let Some(ci) = c.plan.crossings.iter().position(|x| x.gate == Some(gi8)) {
        if c.span_meta.iter().any(|m| usize::from(m.crossing) == ci) {
            let road = matches!(
                c.plan.crossings[ci].kind,
                jane_core::blueprint::SpanKind::Viaduct | jane_core::blueprint::SpanKind::Bridge
            );
            return Some(road && g.road);
        }
    }
    None
}

fn gates(c: &Carved, out: &mut Vec<Issue>) {
    let plan = &c.plan;
    for (ra, rb, name) in [
        (Reg::Lowfields, Reg::Works, "escarpment"),
        (Reg::Lowfields, Reg::Waters, "river valley"),
        (Reg::Waters, Reg::Works, "slag cliffs"),
    ] {
        let (mut n, mut road) = (0, 0);
        for (gi, g) in plan.gates.iter().enumerate() {
            if matches!(g.kind, GateKind::Ledge | GateKind::Gap)
                || g.border != Border::Wall
                || matches!(g.kind, GateKind::Verb(v) if v != Verb::Repair)
            {
                continue;
            }
            let (x, y) = (plan.districts[usize::from(g.da)].reg, plan.districts[usize::from(g.db)].reg);
            if !((x == ra && y == rb) || (x == rb && y == ra)) {
                continue;
            }
            if let Some(by_road) = gate_way(c, gi) {
                n += 1;
                road += usize::from(by_road);
            }
        }
        if n < BORDER_GATES || road < ROAD_GATES {
            out.push(issue("gates", format!("{name}: {n} ways across, {road} by road")));
        }
    }
}

// --- C-6 -------------------------------------------------------------------------------------------

fn ledges(c: &Carved, out: &mut Vec<Issue>) {
    let plan = &c.plan;
    let (two, _, _) = plan.graphs(false);
    let mut served = [false; 4];
    for (gi, g) in plan.gates.iter().enumerate() {
        if g.kind != GateKind::Ledge {
            continue;
        }
        let Some(j) = c.joins.iter().find(|j| j.gate == Some(gi as u8)) else { continue };
        // The walk back from the landing to the top, measured on the macro graph R0 proves.
        let (top, land) = ((i32::from(g.a.0), i32::from(g.a.1)), (i32::from(g.b.0), i32::from(g.b.1)));
        let (dist, _) = plan.dijkstra(&two, idx(land.0, land.1), Some(idx(top.0, top.1)));
        let back = dist[idx(top.0, top.1)];
        if !(STEP_CELLS + LEDGE_MIN_BACK..=LEDGE_BACK_MAX).contains(&back) {
            out.push(issue("ledges", format!("{}: walk back {back} cells", g.name)));
        }
        let lc = j.landings[0].centre();
        for (k, id) in HUBS.iter().enumerate() {
            let Some(pad) = c.pads.iter().find(|p| p.site == *id) else { continue };
            // From the landing to the pad's box (the nearest cell of it).
            let r = pad.boxr;
            let (dx, dy) =
                ((r.x - lc.0).max(0).max(lc.0 - (r.right() - 1)), (r.y - lc.1).max(0).max(lc.1 - (r.bottom() - 1)));
            if i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy)
                <= i64::from(HUB_LEDGE) * i64::from(HUB_LEDGE)
            {
                served[k] = true;
            }
        }
    }
    for (k, id) in HUBS.iter().enumerate() {
        if *id != "station" && !served[k] && pad_centre(c, id).is_some() {
            out.push(issue("ledges", format!("no ledge lands within {HUB_LEDGE} cells of {id}")));
        }
    }
}

// --- C-4 and C-7 -----------------------------------------------------------------------------------

fn reach(c: &Carved, pl: &Planes, out: &mut Vec<Issue>) {
    let Some(start) = pad_centre(c, "station") else {
        out.push(issue("reach", String::from("no station pad")));
        return;
    };
    let p = c.portals();
    let broken = built_broken(&c.spans);
    let pads: Vec<(&str, (i32, i32))> = c.pads.iter().map(|p| (p.site, p.boxr.centre())).collect();
    for (label, bits) in [("as built", broken), ("mended", 0)] {
        let fwd = reach_words(&pl.open, &p, &[start], bits, false);
        for &(site, at) in &pads {
            if !bit(&fwd, at) {
                out.push(issue("reach", format!("{site} cannot be reached from the Halt ({label})")));
            }
        }
        let back = reach_words(&pl.open, &p, &[start], bits, true);
        let pit = (0..WORDS).find_map(|k| {
            let m = fwd[k] & !back[k];
            (m != 0).then(|| (k * 64 + m.trailing_zeros() as usize) as i32)
        });
        if let Some(i) = pit {
            out.push(issue("pit", format!("a cell at ({}, {}) can be reached but not left ({label})", i % W, i / W)));
        }
    }
    // The night's gates closed.
    if !c.night_gates.is_empty() {
        let mut closed = pl.open.clone();
        for r in c.night_rects() {
            for (x, y) in r.cells() {
                if x >= 0 && y >= 0 && x < W && y < W {
                    closed.set((y * W + x) as usize, false);
                }
            }
        }
        let fwd = reach_words(&closed, &p, &[start], broken, false);
        for &(site, at) in &pads {
            if !bit(&fwd, at) {
                out.push(issue("reach", format!("{site} cannot be reached with the night gates shut")));
            }
        }
        let back = reach_words(&closed, &p, &[start], broken, true);
        if (0..WORDS).any(|k| fwd[k] & !back[k] != 0) {
            out.push(issue("pit", String::from("a pit with the night gates shut")));
        }
    }
    // C-7: a flood over level-one cells alone joins the Halt, Julie's and the Castle. The road to the
    // Castle crosses the river in some plans: a level-1 deck over the valley is no climb, so decks count
    // (as built); no join and no ledge does.
    let flat = Portals { ledges: Vec::new(), spans: c.spans.clone() };
    let first = reach_words(&pl.first, &flat, &[start], broken, false);
    for id in ["julie_house", "town"] {
        if let Some(at) = pad_centre(c, id) {
            if !bit(&first, at) {
                out.push(issue("first_walk", format!("{id} is not on the Halt's level ground")));
            }
        }
    }
}
