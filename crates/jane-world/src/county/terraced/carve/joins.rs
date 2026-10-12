//! Joins (PLAN-map-r4 section 2.6): one per gate of the plan that is not a gap, laid in the faces
//! tidy left. A stair is the face rows, a ramp the face rows and two rows either side, a ledge a run
//! of face cells hopped one way, a closed verb gate a rect left whole. Joins run north-south: up a
//! south face (the upper ground north of the lower) or down a north rim (the upper ground south of
//! it, where the art draws a north flight). Where no face of the right districts lies near the
//! gate, a bay is cut into the upper ground and the join goes in its north wall.

use alloc::vec::Vec;
use jane_core::{Rect, Tile, ZoneId};

use super::blocks::{BRect, Blocks, CUT, EXEMPT, FIXED, GROUP, LEVEE, NECK, PAD, VALLEY, WALL, shape_issue_in};
use super::noise::salt_b;
use super::tiles::cliff_at;
use super::water::WaterCut;
use super::{B, BW, CarveFail, W};
use crate::county::terraced::macro_plan::{Gate, GateKind, MacroPlan, STEP_CELLS, Verb};
use crate::steps::{Step, dice};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinKind {
    Stair,
    Ramp,
    Ladder,
    Ledge,
    Stones,
    /// A verb gate left closed: the rect where its ladder or stair would go.
    Closed(Verb),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Join {
    pub kind: JoinKind,
    /// The plan's gate, if one made it.
    pub gate: Option<u8>,
    /// The cells it lays (the face rows, a ramp's six rows, a ledge's run, the stones).
    pub rect: Rect,
    /// What it lays there.
    pub tile: Tile,
    /// The way up (a stair's or ramp's climb), or a ledge's hop.
    pub up: (i8, i8),
    pub lo: u8,
    pub hi: u8,
    /// The centre of the run's lower end and upper end, a cell outside the rect.
    pub foot: (i32, i32),
    pub top: (i32, i32),
    /// The open ground kept at each end: the foot's, then the top's.
    pub landings: [Rect; 2],
    /// The half of the run that stands at the lower level, where it differs from its block's.
    pub lower: Option<Rect>,
}

impl Join {
    /// Cells it holds for good: its rect and its landings.
    pub fn claims(&self) -> Vec<Rect> {
        let mut v = alloc::vec![self.rect, self.landings[0], self.landings[1]];
        v.retain(|r| r.w > 0 && r.h > 0);
        v
    }
}

/// What a join keeps off other joins: pads, other joins, spans, bays, levees and the river.
const KEEP: u8 = FIXED | CUT | NECK | LEVEE;

/// Which face a join cuts: a south face (up goes north) or a north rim (up goes south).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Face {
    South,
    North,
}

/// A run of face between two kinds of ground, in blocks: the upper blocks of district `ud` and level
/// `up` in row `by`, over (south) or under (north) the lower of district `ld` and level `lo`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Seg {
    pub(super) face: Face,
    pub(super) by: i32,
    pub(super) x0: i32,
    pub(super) x1: i32,
    pub(super) up: u8,
    pub(super) lo: u8,
    pub(super) ud: u8,
    pub(super) ld: u8,
}

/// The south-face and north-rim runs in the block window `bx` x `by` (half open).
pub(super) fn segs(b: &Blocks, bx: (i32, i32), by: (i32, i32)) -> Vec<Seg> {
    let mut out = Vec::new();
    for face in [Face::South, Face::North] {
        let dy = if face == Face::South { 1 } else { -1 };
        for y in by.0.max(1)..by.1.min(BW - 1) {
            let mut x = bx.0.max(0);
            while x < bx.1.min(BW) {
                let (up, lo) = (b.level[Blocks::ix(x, y)], b.level[Blocks::ix(x, y + dy)]);
                if up <= lo {
                    x += 1;
                    continue;
                }
                let (ud, ld) = (b.district[Blocks::ix(x, y)], b.district[Blocks::ix(x, y + dy)]);
                let x0 = x;
                while x < bx.1.min(BW)
                    && b.level[Blocks::ix(x, y)] == up
                    && b.level[Blocks::ix(x, y + dy)] == lo
                    && b.district[Blocks::ix(x, y)] == ud
                    && b.district[Blocks::ix(x, y + dy)] == ld
                {
                    x += 1;
                }
                out.push(Seg { face, by: y, x0, x1: x, up, lo, ud, ld });
            }
        }
    }
    out
}

/// A cell's ground for a landing: open (no face, no water) at level `lv`.
fn open_at(b: &Blocks, w: &WaterCut, x: i32, y: i32, lv: u8) -> bool {
    x >= 0 && y >= 0 && x < W && y < W && !cliff_at(b, x, y) && !w.at(x, y) && b.level[Blocks::ix(x / B, y / B)] == lv
}

fn rect_open(b: &Blocks, w: &WaterCut, r: Rect, lv: u8) -> bool {
    // Open ground at the level, and a bank between it and any water.
    r.cells().all(|(x, y)| open_at(b, w, x, y, lv)) && !r.grow(1).cells().any(|(x, y)| w.at(x, y))
}

/// Does any block of `r` (cells, rounded out by `margin` blocks) hold a flag in `mask`?
fn flagged(b: &Blocks, r: Rect, margin: i32, mask: u8) -> bool {
    let br = BRect::of_cells(r).grow(margin);
    (br.y.max(0)..br.bottom().min(BW))
        .any(|y| (br.x.max(0)..br.right().min(BW)).any(|x| b.flags[Blocks::ix(x, y)] & mask != 0))
}

fn dist1(a: (i32, i32), c: (i32, i32)) -> i32 {
    (a.0 - c.0).abs() + (a.1 - c.1).abs()
}

/// The middle of the gate's two macro cells, in cells.
pub(super) fn macro_mid(g: &Gate) -> (i32, i32) {
    let ca = |c: (u8, u8)| (i32::from(c.0) * STEP_CELLS + 8, i32::from(c.1) * STEP_CELLS + 8);
    let (a, c) = (ca(g.a), ca(g.b));
    ((a.0 + c.0) / 2, (a.1 + c.1) / 2)
}

fn reserve(b: &mut Blocks, r: Rect) {
    b.flag_rect(BRect::of_cells(r).grow(1), FIXED);
}

/// How well a face between districts `ud` (upper) and `ld` fits gate `g`: `Some(0)` between exactly its two
/// districts, `Some(penalty)` between districts of the same two regions (any place along that border
/// serves as a way across it), `None` otherwise.
pub(super) fn fit(plan: &MacroPlan, g: &Gate, ud: u8, ld: u8) -> Option<i32> {
    if (ud == g.da && ld == g.db) || (ud == g.db && ld == g.da) {
        return Some(0);
    }
    let reg = |d: u8| plan.districts[usize::from(d)].reg;
    let (a, c) = (reg(g.da), reg(g.db));
    let (x, y) = (reg(ud), reg(ld));
    ((x == a && y == c) || (x == c && y == a)).then_some(48)
}

/// A candidate stair, ramp or closed gate: its join, scored by its distance to the gate's middle.
#[allow(clippy::too_many_arguments)]
pub(super) fn find_join(
    plan: &MacroPlan,
    b: &Blocks,
    w: &WaterCut,
    g: &Gate,
    mid: (i32, i32),
    width: i32,
    kind: JoinKind,
    reach: i32,
) -> Option<Join> {
    let (mbx, mby) = (mid.0 / B, mid.1 / B);
    let r = reach / B;
    let mut best: Option<((i32, i32, i32), Join)> = None;
    for s in segs(b, (mbx - r, mbx + r), (mby - r, mby + r)) {
        let Some(pen) = fit(plan, g, s.ud, s.ld) else { continue };
        if s.up != s.lo + 1 {
            continue;
        }
        let (xlo, xhi) = (s.x0 * B, s.x1 * B - 1);
        for x in xlo + 1..=xhi - width {
            let Some(j) = make_join(b, w, s.face, s.by, x, width, kind, s.up, s.lo) else {
                continue;
            };
            let (y0, y1) = (
                j.rect.y.min(j.landings[0].y).min(j.landings[1].y),
                j.rect.bottom().max(j.landings[0].bottom()).max(j.landings[1].bottom()),
            );
            if flagged(b, Rect::new(x - 1, y0, width + 2, y1 - y0), 0, KEEP) {
                continue;
            }
            let ry = if s.face == Face::South { s.by * B + B - 2 } else { s.by * B };
            let key = (dist1((x + width / 2, ry), mid) + pen, ry, x);
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((key, j));
            }
        }
    }
    best.map(|(_, j)| j)
}

/// The stair, ramp or closed gate of `width` at column `x` in the face of block row `by` (a south face
/// or a north rim), climbing from level `lo` to `up`, if its landings are open and the face stands
/// either side of it. Not scored, not checked against other joins.
#[allow(clippy::too_many_arguments)]
pub(super) fn make_join(
    b: &Blocks,
    w: &WaterCut,
    face: Face,
    by: i32,
    x: i32,
    width: i32,
    kind: JoinKind,
    up: u8,
    lo: u8,
) -> Option<Join> {
    let ramp = kind == JoinKind::Ramp;
    let (rect, lower, foot, top, climb) = match face {
        Face::South => {
            // The face rows are the last two of the upper block row; a ramp adds two each side.
            let fy = by * B + B - 2;
            let (above, below) = if ramp { (2, 2) } else { (0, 0) };
            let full = Rect::new(x, fy - above, width, 2 + above + below);
            (
                full,
                Some(Rect::new(x, fy + 1, width, 1 + below)),
                Rect::new(x - 1, full.bottom(), width + 2, 2),
                Rect::new(x - 1, full.y - 2, width + 2, 2),
                (0, -1),
            )
        }
        Face::North => {
            // The rim row is the first of the upper block row; a ramp has three rows north of it.
            let ry = by * B;
            if ramp {
                let full = Rect::new(x, ry - 3, width, 6);
                (
                    full,
                    None,
                    Rect::new(x - 1, full.y - 2, width + 2, 2),
                    Rect::new(x - 1, full.bottom(), width + 2, 2),
                    (0, 1),
                )
            } else {
                let full = Rect::new(x, ry, width, 1);
                (full, None, Rect::new(x - 1, ry - 2, width + 2, 2), Rect::new(x - 1, ry + 1, width + 2, 2), (0, 1))
            }
        }
    };
    if !rect_open(b, w, foot, lo) || !rect_open(b, w, top, up) {
        return None;
    }
    // Walls either side of the cut: the face or rim cells beside it.
    let ry = if face == Face::South { by * B + B - 2 } else { by * B };
    let rows = if face == Face::South { 2 } else { 1 };
    if !(0..rows).all(|k| cliff_at(b, x - 1, ry + k) && cliff_at(b, x + width, ry + k)) {
        return None;
    }
    // A ramp's cells off the face itself are open ground at their half's level.
    if ramp {
        let level_of = |yy: i32| if (yy < ry) == (face == Face::South) { up } else { lo };
        if !rect.cells().all(|(cx, cy)| (cy >= ry && cy < ry + rows) || open_at(b, w, cx, cy, level_of(cy))) {
            return None;
        }
    }
    let c = x + width / 2;
    let (fp, tp) = match face {
        Face::South => ((c, rect.bottom() + 1), (c, rect.y - 2)),
        Face::North => ((c, rect.y - 2), (c, rect.bottom() + 1)),
    };
    let tile = if ramp {
        Tile::Road
    } else if kind == JoinKind::Stair {
        Tile::Stair
    } else {
        Tile::Cliff
    };
    Some(Join { kind, gate: None, rect, tile, up: climb, lo, hi: up, foot: fp, top: tp, landings: [foot, top], lower })
}

/// Cut a bay (section 2.6): a notch of the lower level, two groups wide and one deep, into the upper
/// district beside its meeting with the lower one, near `mid`; flagged `CUT | FIXED`.
pub(super) fn cut_bay(b: &mut Blocks, g: &Gate, mid: (i32, i32), reach: i32) -> bool {
    let (mbx, mby) = (mid.0 / B, mid.1 / B);
    let r = reach / B;
    // The levels each district really has in the window (the plan's levels are R0's; the valley, the
    // demoted edges and the levees have changed some).
    let levels = |d: u8| -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        for y in (mby - r).max(0)..(mby + r).min(BW) {
            for x in (mbx - r).max(0)..(mbx + r).min(BW) {
                let i = Blocks::ix(x, y);
                if b.district[i] == d && !v.contains(&b.level[i]) {
                    v.push(b.level[i]);
                }
            }
        }
        v.sort();
        v
    };
    let (nw, nh) = (2 * GROUP, GROUP);
    let (lv_a, lv_b) = (levels(g.da), levels(g.db));
    for (upper, lower_d, ups, los) in [(g.da, g.db, &lv_a, &lv_b), (g.db, g.da, &lv_b, &lv_a)] {
        for &up_level in ups.iter().rev() {
            for &lower in los {
                if up_level <= lower || up_level - lower > 2 {
                    continue;
                }
                let mut cands: Vec<((i32, i32, i32), i32, i32)> = Vec::new();
                let mut ny = (mby - r).max(5 * GROUP);
                ny -= ny.rem_euclid(GROUP);
                while ny < (mby + r).min(BW - 3 * GROUP) {
                    let mut nx = (mbx - r).max(GROUP);
                    nx -= nx.rem_euclid(GROUP);
                    while nx < (mbx + r).min(BW - nw - GROUP) {
                        let ok_blocks = (ny..ny + nh).all(|y| {
                            (nx..nx + nw).all(|x| {
                                let i = Blocks::ix(x, y);
                                b.district[i] == upper && b.level[i] == up_level && b.flags[i] & (EXEMPT | VALLEY) == 0
                            })
                        });
                        if ok_blocks {
                            // Open to lower ground along one whole side (west, east or south), walled
                            // north by upper ground for the face and its landings (and for a climb of
                            // two levels the shelf and the second flight).
                            let side = |x: i32| (ny..ny + nh).all(|y| b.lv(x, y) == Some(lower));
                            let below = (nx..nx + nw).all(|x| b.lv(x, ny + nh) == Some(lower));
                            let open = side(nx - 1) || side(nx + nw) || below;
                            let rows = 2 * i32::from(up_level - lower);
                            let north = (nx..nx + nw).all(|x| (1..=rows).all(|k| b.lv(x, ny - k) == Some(up_level)));
                            let cells = BRect::new(nx, ny, nw, nh).cells();
                            if open && north && !flagged(b, cells, 2, KEEP | VALLEY | PAD) {
                                cands.push(((dist1((nx * B + nw * B / 2, ny * B), mid), ny, nx), nx, ny));
                            }
                        }
                        nx += GROUP;
                    }
                    ny += GROUP;
                }
                cands.sort();
                for &(_, nx, ny) in cands.iter().take(12) {
                    let win = (nx - 3, ny - 4, nx + nw + 3, ny + nh + 3);
                    let was_clean = shape_issue_in(b, win.0, win.1, win.2, win.3).is_none();
                    let saved: Vec<(usize, u8, u8, u8)> = (ny - 1..ny + nh)
                        .flat_map(|y| (nx..nx + nw).map(move |x| Blocks::ix(x, y)))
                        .map(|i| (i, b.level[i], b.district[i], b.flags[i]))
                        .collect();
                    for y in ny..ny + nh {
                        for x in nx..nx + nw {
                            let i = Blocks::ix(x, y);
                            // The notch is lower ground: the lower district's, so the join in its north wall
                            // is between the gate's two districts.
                            b.level[i] = lower;
                            b.district[i] = lower_d;
                            b.flags[i] |= CUT | FIXED;
                        }
                    }
                    // The wall above the notch is a made face: exempt from the jog rule.
                    for x in nx..nx + nw {
                        b.flags[Blocks::ix(x, ny - 1)] |= WALL;
                    }
                    if !was_clean || shape_issue_in(b, win.0, win.1, win.2, win.3).is_none() {
                        return true;
                    }
                    for (i, l, d, f) in saved {
                        b.level[i] = l;
                        b.district[i] = d;
                        b.flags[i] = f;
                    }
                }
            }
        }
    }
    false
}

/// A climb of two levels (section 2.6: a gate that climbs two levels chains two ramps with a
/// shelf): a shelf of the middle level is cut into the upper ground at the top of the south face, two
/// groups deep and four across, and a stair or ramp goes up each of its two faces, side by side so the
/// way doubles back. Returns the two joins, the lower first.
#[allow(clippy::too_many_arguments)]
pub(super) fn chain_join(
    plan: &MacroPlan,
    b: &mut Blocks,
    w: &WaterCut,
    g: &Gate,
    mid: (i32, i32),
    width: i32,
    kind: JoinKind,
    reach: i32,
) -> Vec<Join> {
    let (mbx, mby) = (mid.0 / B, mid.1 / B);
    let r = reach / B;
    let ramp = kind == JoinKind::Ramp;
    let mut cands: Vec<((i32, i32, i32), Seg, i32)> = Vec::new();
    for s in segs(b, (mbx - r, mbx + r), (mby - r, mby + r)) {
        let Some(pen) = fit(plan, g, s.ud, s.ld) else { continue };
        if s.face != Face::South || s.up != s.lo + 2 || s.by < 4 || s.by % 2 == 0 {
            continue;
        }
        // The shelf is four blocks wide on the groups' grid, inside the run.
        let mut x0 = s.x0 + (s.x0 % 2);
        while x0 + 4 <= s.x1 {
            cands.push(((dist1((x0 * B + 8, s.by * B), mid) + pen, s.by, x0), s, x0));
            x0 += 2;
        }
    }
    cands.sort_by_key(|c| c.0);
    for (_, s, x0) in cands.into_iter().take(8) {
        let by = s.by;
        let win = (x0 - 3, by - 4, x0 + 7, by + 4);
        let was_clean = shape_issue_in(b, win.0, win.1, win.2, win.3).is_none();
        let saved: Vec<(usize, u8, u8)> = (by - 1..=by)
            .flat_map(|y| (x0..x0 + 4).map(move |x| Blocks::ix(x, y)))
            .map(|i| (i, b.level[i], b.flags[i]))
            .collect();
        let blocked = saved.iter().any(|&(_, _, f)| f & ((EXEMPT & !WALL) | VALLEY) != 0)
            || (x0..x0 + 4).any(|x| {
                b.flags[Blocks::ix(x, by - 2)] & EXEMPT != 0
                    || b.lv(x, by - 2) != Some(s.up)
                    || b.lv(x, by + 1) != Some(s.lo)
            });
        if blocked {
            continue;
        }
        for &(i, _, _) in &saved {
            b.level[i] = s.lo + 1;
            b.flags[i] |= CUT | FIXED;
        }
        let xs = x0 * B;
        let (above, below) = if ramp { (2, 2) } else { (0, 0) };
        let tile = if ramp { Tile::Road } else { Tile::Stair };
        let one = |x: i32, fy: i32, lo: u8, hi: u8| -> Join {
            let full = Rect::new(x, fy - above, width, 2 + above + below);
            Join {
                kind,
                gate: None,
                rect: full,
                tile,
                up: (0, -1),
                lo,
                hi,
                foot: (x + width / 2, full.bottom() + 1),
                top: (x + width / 2, full.y - 2),
                landings: [Rect::new(x - 1, full.bottom(), width + 2, 2), Rect::new(x - 1, full.y - 2, width + 2, 2)],
                lower: Some(Rect::new(x, fy + 1, width, 1 + below)),
            }
        };
        // The lower flight up the shelf's south face (the last two rows of block row `by`) and the
        // upper flight up its north face, each at the first place along it that fits: the lower from
        // the west, the upper from the east, so the way doubles back along the shelf.
        let fits = |j: &Join| {
            let (a, c) = (rect_open(b, w, j.landings[0], j.lo), rect_open(b, w, j.landings[1], j.hi));
            let ry = j.rect.y + above;
            let walls = (0..2).all(|k| cliff_at(b, j.rect.x - 1, ry + k) && cliff_at(b, j.rect.right(), ry + k));
            a && c && walls
        };
        let span = 4 * B;
        let j1 = (xs + 1..xs + span - width).map(|x| one(x, by * B + B - 2, s.lo, s.lo + 1)).find(|j| fits(j));
        let j2 =
            (xs + 1..xs + span - width).rev().map(|x| one(x, (by - 2) * B + B - 2, s.lo + 1, s.up)).find(|j| fits(j));
        if let (Some(j1), Some(j2)) = (j1, j2) {
            if !was_clean || shape_issue_in(b, win.0, win.1, win.2, win.3).is_none() {
                for x in x0..x0 + 4 {
                    b.flags[Blocks::ix(x, by - 2)] |= FIXED;
                }
                return alloc::vec![j1, j2];
            }
        }
        for (i, l, f) in saved {
            b.level[i] = l;
            b.flags[i] = f;
        }
    }
    Vec::new()
}

/// The stair, ramp or closed gate of `g` of `width`: a place in a face between its districts, else a
/// chain of two over a face of two levels, else in a bay cut for it.
pub(super) fn place(plan: &MacroPlan, b: &mut Blocks, w: &WaterCut, g: &Gate, width: i32, kind: JoinKind) -> Vec<Join> {
    let mid = macro_mid(g);
    let mut joins: Vec<Join> = find_join(plan, b, w, g, mid, width, kind, 48).into_iter().collect();
    if joins.is_empty() && kind != JoinKind::Closed(Verb::Repair) {
        joins = chain_join(plan, b, w, g, mid, width, kind, 64);
        if joins.is_empty() && cut_bay(b, g, mid, 96) {
            joins = find_join(plan, b, w, g, mid, width, kind, 96).into_iter().collect();
            if joins.is_empty() {
                joins = chain_join(plan, b, w, g, mid, width, kind, 96);
            }
        }
        // Still none: any face along the same border, further afield.
        if joins.is_empty() {
            joins = find_join(plan, b, w, g, mid, width, kind, 160).into_iter().collect();
        }
        if joins.is_empty() {
            joins = chain_join(plan, b, w, g, mid, width, kind, 160);
        }
        // Last, anywhere along the border: the macro walk does not mind where the way across is.
        if joins.is_empty() {
            joins = find_join(plan, b, w, g, mid, width, kind, 480).into_iter().collect();
        }
    }
    for j in &joins {
        reserve(b, j.rect);
        for l in j.landings {
            reserve(b, l);
        }
    }
    joins
}

/// A ledge of the gate: a run of `len` face cells hopped south over a south face, or east or west
/// over a rim, nearest `mid`. A north hop moves to the nearest of the others on the same border.
fn ledge_join(b: &mut Blocks, w: &WaterCut, g: &Gate, mid: (i32, i32), len: i32, target: Option<Rect>) -> Option<Join> {
    let (top_d, low_d) = (g.da, g.db);
    let (mbx, mby) = (mid.0 / B, mid.1 / B);
    // A ledge that serves a hub is looked for round the hub's pad too, and the one that lands nearest the
    // pad's box wins; the others land nearest the gate's middle.
    let r = if target.is_some() { 8 * STEP_CELLS / B } else { 3 * STEP_CELLS / B + 2 };
    let score = |p: (i32, i32)| -> i32 {
        match target {
            Some(t) => {
                let (dx, dy) =
                    ((t.x - p.0).max(0).max(p.0 - (t.right() - 1)), (t.y - p.1).max(0).max(p.1 - (t.bottom() - 1)));
                dx + dy
            }
            None => dist1(p, mid),
        }
    };
    let want = g.dir();
    let mut best: Option<((i32, i32, i32), Join)> = None;
    // South hops.
    for s in segs(b, (mbx - r, mbx + r), (mby - r, mby + r)) {
        if s.face != Face::South || s.ud != top_d || s.ld != low_d || s.up != s.lo + 1 {
            continue;
        }
        let (xlo, xhi) = (s.x0 * B, s.x1 * B - 1);
        let fy = s.by * B + B - 2;
        for x in xlo + 1..=xhi - len {
            let rect = Rect::new(x, fy, len, 2);
            let top = Rect::new(x, fy - 2, len, 2);
            let foot = Rect::new(x, fy + 2, len, 2);
            if !rect_open(b, w, top, s.up) || !rect_open(b, w, foot, s.lo) {
                continue;
            }
            if flagged(b, Rect::new(x, fy - 2, len, 6), 0, KEEP) {
                continue;
            }
            let pen = if want == (0, 1) { 0 } else { 40 };
            let key = (score((x + len / 2, fy + 3)) + pen, fy, x);
            if best.as_ref().is_none_or(|(k, _)| key < *k) {
                best = Some((
                    key,
                    Join {
                        kind: JoinKind::Ledge,
                        gate: None,
                        rect,
                        tile: Tile::LedgeS,
                        up: (0, 1),
                        lo: s.lo,
                        hi: s.up,
                        foot: (x + len / 2, fy + 3),
                        top: (x + len / 2, fy - 1),
                        landings: [foot, top],
                        lower: None,
                    },
                ));
            }
        }
    }
    // East and west hops over a rim: the upper ground west (hop east) or east (hop west) of it. A
    // vertical run is the blocks of one column that share their grounds.
    for (dx, tile, hop) in [(1, Tile::LedgeE, (1, 0)), (-1, Tile::LedgeW, (-1, 0))] {
        for bx in mbx - r..mbx + r {
            let mut by0 = mby - r;
            while by0 < mby + r {
                let (Some(up), Some(lo)) = (b.lv(bx, by0), b.lv(bx + dx, by0)) else {
                    by0 += 1;
                    continue;
                };
                let (ud, ld) = (b.district[Blocks::ix(bx, by0)], b.district[Blocks::ix(bx + dx, by0)]);
                if up != lo + 1 || ud != top_d || ld != low_d {
                    by0 += 1;
                    continue;
                }
                let mut by1 = by0;
                while b.lv(bx, by1) == Some(up)
                    && b.lv(bx + dx, by1) == Some(lo)
                    && b.district[Blocks::ix(bx, by1)] == ud
                    && b.district[Blocks::ix(bx + dx, by1)] == ld
                {
                    by1 += 1;
                }
                // The rim column: x = 4 bx + 3 hopping east, 4 bx hopping west; rows of the run.
                let rx = if dx > 0 { bx * B + B - 1 } else { bx * B };
                let (y0, y1) = (by0 * B, by1 * B - 1);
                for y in y0 + 1..=y1 - len {
                    let rect = Rect::new(rx, y, 1, len);
                    let (foot, top) = if dx > 0 {
                        (Rect::new(rx + 1, y, 2, len), Rect::new(rx - 2, y, 2, len))
                    } else {
                        (Rect::new(rx - 2, y, 2, len), Rect::new(rx + 1, y, 2, len))
                    };
                    if !rect_open(b, w, foot, lo) || !rect_open(b, w, top, up) {
                        continue;
                    }
                    if rect.cells().any(|(x, yy)| !cliff_at(b, x, yy))
                        || flagged(b, Rect::new(rx - 2, y, 5, len), 0, KEEP)
                    {
                        continue;
                    }
                    let pen = if want == hop { 0 } else { 40 };
                    let key = (score((if dx > 0 { rx + 2 } else { rx - 2 }, y + len / 2)) + pen, y, rx);
                    if best.as_ref().is_none_or(|(k, _)| key < *k) {
                        best = Some((
                            key,
                            Join {
                                kind: JoinKind::Ledge,
                                gate: None,
                                rect,
                                tile,
                                up: (dx as i8, 0),
                                lo,
                                hi: up,
                                foot: (if dx > 0 { rx + 2 } else { rx - 2 }, y + len / 2),
                                top: (if dx > 0 { rx - 2 } else { rx + 2 }, y + len / 2),
                                landings: [foot, top],
                                lower: None,
                            },
                        ));
                    }
                }
                by0 = by1;
            }
        }
    }
    let (_, j) = best?;
    let r = j.rect;
    let wide = match j.tile {
        Tile::LedgeS => Rect::new(r.x, r.y - 2, r.w, 6),
        _ => Rect::new(r.x - 2, r.y, 5, r.h),
    };
    b.flag_rect(BRect::of_cells(wide).grow(1), FIXED);
    Some(j)
}

/// The stepping stones (section 2.6): a stair down the terrace's face to the valley floor, found as any
/// other stair of the gate, and a line of `Stepping` three wide straight across the channel (seven cells
/// or narrower) at the narrowest place within reach of the stair's foot, open floor at both ends, so
/// the floor under the stair has the far bank of the river. The plan draws the stair, the stones and the
/// far stair on one line; the channel meanders, so here the stones stand where the channel allows
/// them. Returns the stair (or its chain) and the stones; the stair alone where the channel has no
/// such place, and nothing where the gate has no stair.
fn stones_join(plan: &MacroPlan, b: &mut Blocks, w: &WaterCut, g: &Gate) -> Vec<Join> {
    let mut joins = place(plan, b, w, g, 4, JoinKind::Stair);
    let Some(foot) = joins.first().map(|j| j.foot) else { return joins };
    let dry = |r: Rect| r.cells().all(|(x, y)| open_at(b, w, x, y, 0));
    let mut best: Option<((i32, i32, i32), Join)> = None;
    for (k, p) in w.channel.iter().enumerate() {
        let reach = dist1((p.x, p.y), foot);
        if reach > 96 {
            continue;
        }
        // The channel's heading (samples two either side, in its reach): the stones run across it.
        let near = |j: usize| w.channel.get(j).filter(|q| q.reach == p.reach).unwrap_or(p);
        let (a, z) = (near(k.saturating_sub(2)), near(k + 2));
        let vertical = (z.y - a.y).abs() >= (z.x - a.x).abs();
        // `along` is the stones' own axis, `cross` the three cells they are wide.
        let wet =
            |al: i32, cr: i32| -> bool { (-1..=1).all(|d| if vertical { w.at(al, cr + d) } else { w.at(cr + d, al) }) };
        let (al0, cr0) = if vertical { (p.x, p.y) } else { (p.y, p.x) };
        if !wet(al0, cr0) {
            continue;
        }
        let (mut lo, mut hi) = (al0, al0);
        while wet(lo - 1, cr0) && al0 - lo < 8 {
            lo -= 1;
        }
        while wet(hi + 1, cr0) && hi - al0 < 8 {
            hi += 1;
        }
        let len = hi - lo + 1;
        if !(3..=7).contains(&len) {
            continue;
        }
        // `vertical` here: the channel runs north-south, so the stones run east-west (along x).
        let (rect, l0, l1) = if vertical {
            (Rect::new(lo, cr0 - 1, len, 3), Rect::new(lo - 3, cr0 - 1, 3, 3), Rect::new(hi + 1, cr0 - 1, 3, 3))
        } else {
            (Rect::new(cr0 - 1, lo, 3, len), Rect::new(cr0 - 1, lo - 3, 3, 3), Rect::new(cr0 - 1, hi + 1, 3, 3))
        };
        if !dry(l0) || !dry(l1) || flagged(b, rect.grow(3), 0, KEEP) {
            continue;
        }
        let key = (reach, rect.y, rect.x);
        if best.as_ref().is_none_or(|(kk, _)| key < *kk) {
            let (foot_c, top_c) = (l0.centre(), l1.centre());
            best = Some((
                key,
                Join {
                    kind: JoinKind::Stones,
                    gate: None,
                    rect,
                    tile: Tile::Stepping,
                    up: if vertical { (1, 0) } else { (0, 1) },
                    lo: 0,
                    hi: 0,
                    foot: foot_c,
                    top: top_c,
                    landings: [l0, l1],
                    lower: None,
                },
            ));
        }
    }
    if let Some((_, j)) = best {
        reserve(b, j.rect);
        for l in j.landings {
            reserve(b, l);
        }
        joins.push(j);
    }
    joins
}

/// The width of a stair or ramp the gate carries.
fn width_of(g: &Gate) -> i32 {
    match (g.kind, g.name) {
        (GateKind::Stair, "The Church Steps") => 5,
        (GateKind::Stair, _) => 4,
        _ => 6,
    }
}

/// Every join the plan's gates need on the blocks `b` (section 2.6), and the gates that found no
/// place. Spans, the rail arch, the repaired footbridge and the stones are the span stage's.
pub(super) fn lay_all(plan: &MacroPlan, salt: u8, b: &mut Blocks, w: &WaterCut) -> (Vec<Join>, Vec<(u8, bool)>) {
    let pads = super::blocks::pads(plan);
    let mut out: Vec<Join> = Vec::new();
    let mut failed: Vec<(u8, bool)> = Vec::new();
    for (gi, g) in plan.gates.iter().enumerate() {
        let gi8 = gi as u8;
        let mid = macro_mid(g);
        let mut r = dice(plan.seed, ZoneId::County, Step::MapJoin, plan.attempt, gi as i32, salt_b(salt, 0));
        let join = match g.kind {
            GateKind::Gap | GateKind::Span | GateKind::Verb(Verb::Repair) => continue,
            GateKind::SteppingStones => stones_join(plan, b, w, g),
            GateKind::Underpass if g.name == "The Rail Arch" => continue,
            GateKind::Stair => place(plan, b, w, g, width_of(g), JoinKind::Stair),
            GateKind::RampRoad | GateKind::CompanyGate | GateKind::Underpass => place(plan, b, w, g, 6, JoinKind::Ramp),
            GateKind::Verb(v) => {
                let j = place(plan, b, w, g, 4, JoinKind::Closed(v));
                // A closed verb gate is never the only way: with no ground for its rect it is left out.
                if j.is_empty() {
                    continue;
                }
                j
            }
            GateKind::Ledge => {
                let len = r.range(6, 10);
                // The hub (the Halt, Julie's, the Castle, the Museum) this ledge lands by, if any: the
                // nearest of their pads to the landing's macro cell, within 12 macro cells.
                let land = (i32::from(g.b.0) * STEP_CELLS + 8, i32::from(g.b.1) * STEP_CELLS + 8);
                let target = ["julie_house", "town", "museum"]
                    .iter()
                    .filter_map(|id| pads.iter().find(|p| p.site == *id))
                    .map(|p| (dist1(p.boxr.centre(), land), p.boxr))
                    .filter(|&(d, _)| d <= 12 * STEP_CELLS)
                    .min_by_key(|&(d, _)| d)
                    .map(|(_, r)| r);
                ledge_join(b, w, g, mid, len, target).into_iter().collect()
            }
        };
        if join.is_empty() {
            failed.push((gi8, g.kind == GateKind::Ledge));
            continue;
        }
        for mut j in join {
            j.gate = Some(gi8);
            out.push(j);
        }
    }
    (out, failed)
}

/// [`lay_all`], refused at the first gate with no place.
pub fn lay_joins(plan: &MacroPlan, salt: u8, b: &mut Blocks, w: &WaterCut) -> Result<Vec<Join>, CarveFail> {
    let (out, failed) = lay_all(plan, salt, b, w);
    match failed.first() {
        Some(&(g, true)) => Err(CarveFail::Ledge(g)),
        Some(&(g, false)) => Err(CarveFail::Join(g)),
        None => Ok(out),
    }
}
