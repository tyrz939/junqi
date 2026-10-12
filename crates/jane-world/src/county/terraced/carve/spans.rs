//! Spans (PLAN-map-r4 section 4): a span only where the plan has a crossing, of the kind the plan
//! gives it. A deck is a straight run 2 to 4 cells wide and at most 40 long over ground that is
//! water or lower than the deck, its two end rows solid (the abutments: the face, the rim) and the
//! rows just beyond them open ground at the deck's level. The place is found on the carved ground, not
//! assumed: the line across the valley (or the river) nearest the plan's crossing whose run is the
//! shortest. Where the far side has no ground at the deck's level, an embankment is raised there
//! with a stair or ramp down; the rail arch cuts a cutting into the plateau and bridges it.

use alloc::vec::Vec;
use jane_core::blueprint::{Span, SpanKind};
use jane_core::{Rect, Tile};

use super::blocks::{BRect, Blocks, CUT, EXEMPT, FIXED, GROUP, PAD, VALLEY, WALL, shape_issue_in};
use super::joins::{Face, Join, JoinKind, make_join};
use super::tiles::cliff_at;
use super::water::WaterCut;
use super::{B, BW, CarveFail, W};
use crate::county::terraced::macro_plan::{Crossing, MacroPlan, Over, STEP_CELLS};

/// The longest deck (MAP.md 2.5), and the shortest.
pub const SPAN_MAX: i32 = 40;
pub const SPAN_MIN: i32 = 4;
/// How far across the line from the plan's crossing a deck is looked for (cells), and along it.
pub const SEARCH_ACROSS: i32 = 40;
pub const SEARCH_ALONG: i32 = 56;
/// An embankment: 8 cells across and 16 along (two groups by four blocks).
pub const EMB_ACROSS: i32 = 8;
pub const EMB_ALONG: i32 = 16;

/// What `lay_spans` makes: the spans, what the carve keeps of each, and the plank bridges.
pub type Laid = (Vec<Span>, Vec<SpanMeta>, Vec<Rect>);

/// A cutting's candidate place: its score, the notch's origin in blocks, and the levels above and below.
type Notch = ((i32, i32, i32), i32, i32, u8, u8);

/// What the carve keeps of a span beside the `Span` the blueprint gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanMeta {
    /// Index into `plan.crossings`.
    pub crossing: u8,
    pub over: Over,
    /// The deck's centreline, 6 cells out from each end (the road's straight legs run between).
    pub ends: [(i32, i32); 2],
    /// The strip raised at an end whose ground lies below the deck.
    pub embankment: Option<Rect>,
    /// Abutments made where the banks are level (a footbridge over a stream at the valley's floor).
    pub abutments: Vec<Rect>,
    /// A rail arch's track on the plateau, and the buffers it ends at.
    pub track: Vec<Rect>,
    pub buffers: Vec<Rect>,
}

/// The cell `(x, y)`: open ground at level `lv` (no face, no water).
fn open_at(b: &Blocks, w: &WaterCut, x: i32, y: i32, lv: u8) -> bool {
    x >= 0 && y >= 0 && x < W && y < W && !cliff_at(b, x, y) && !w.at(x, y) && b.level[Blocks::ix(x / B, y / B)] == lv
}

fn level_at(b: &Blocks, x: i32, y: i32) -> u8 {
    b.level[Blocks::ix(x / B, y / B)]
}

/// A deck found: its rect, axis and level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deck {
    pub rect: Rect,
    pub along_x: bool,
    pub level: u8,
    /// Over water alone, between two banks of one level: boards, not a span.
    pub plank: bool,
}

/// Is `s` a well-formed span as the sim reads one (`jane_sim::span::well_formed`, which this agrees with):
/// its two end rows solid, the rows just beyond them open ground at the deck's level, 2 to 4 cells
/// wide and at most 40 long.
pub fn well_formed(level: &dyn Fn(i32, i32) -> u8, solid: &dyn Fn(i32, i32) -> bool, s: &Span) -> bool {
    let r = s.rect;
    let (a, b) = if s.along_x {
        (Rect::new(r.x, r.y, 1, r.h), Rect::new(r.right() - 1, r.y, 1, r.h))
    } else {
        (Rect::new(r.x, r.y, r.w, 1), Rect::new(r.x, r.bottom() - 1, r.w, 1))
    };
    let all_solid = |q: Rect| q.cells().all(|(x, y)| solid(x, y));
    let ends = s.ends().iter().all(|e| e.cells().all(|(x, y)| !solid(x, y) && level(x, y) == s.deck_level));
    all_solid(a) && all_solid(b) && ends && (2..=4).contains(&if s.along_x { r.h } else { r.w }) && s.length() <= 40
}

/// Is `rect` a deck at `lv`: its first and last rows along the axis solid, the rows beyond them open
/// ground at `lv`, and between them (over the deck's whole width) only water, lower ground or faces?
pub fn deck_ok(b: &Blocks, w: &WaterCut, rect: Rect, along_x: bool, lv: u8) -> bool {
    let (a0, a1, l0, width) =
        if along_x { (rect.x, rect.right() - 1, rect.y, rect.h) } else { (rect.y, rect.bottom() - 1, rect.x, rect.w) };
    (0..width).all(|k| {
        let l = l0 + k;
        let cell = |a: i32| if along_x { (a, l) } else { (l, a) };
        let (fx, fy) = cell(a0);
        let (ex, ey) = cell(a1);
        let (bx, by) = cell(a0 - 1);
        let (nx, ny) = cell(a1 + 1);
        cliff_at(b, fx, fy)
            && cliff_at(b, ex, ey)
            && open_at(b, w, bx, by, lv)
            && open_at(b, w, nx, ny, lv)
            && (a0 + 1..a1).all(|m| {
                let (mx, my) = cell(m);
                w.at(mx, my) || level_at(b, mx, my) < lv || cliff_at(b, mx, my)
            })
    })
}

fn rect_of(along_x: bool, a0: i32, a1: i32, line_lo: i32, width: i32) -> Rect {
    if along_x { Rect::new(a0, line_lo, a1 - a0 + 1, width) } else { Rect::new(line_lo, a0, width, a1 - a0 + 1) }
}

/// The decks of `width` along `along_x` over a gap in the ground that a line within reach of `base`
/// crosses, nearest first: `(distance, deck)`. A gap is a run of cells that are water, lower than the
/// level at its ends, or the faces and rims between, bounded by open ground of one level; `want` fixes
/// that level (a road's bridge is at the terrace's level, not the valley floor's).
pub fn decks_near(
    b: &Blocks,
    w: &WaterCut,
    base: (i32, i32),
    along_x: bool,
    width: i32,
    want: Option<u8>,
) -> Vec<(i32, Deck)> {
    let mut out: Vec<(i32, Deck)> = Vec::new();
    let half = width / 2;
    for t in -SEARCH_ACROSS..=SEARCH_ACROSS {
        // The line: a row for an east-west deck (y = base.y + t), a column otherwise.
        let (line0, along0) = if along_x { (base.1 + t, base.0) } else { (base.0 + t, base.1) };
        if line0 - half < 1 || line0 + half >= W - 1 {
            continue;
        }
        let cell = |a: i32, l: i32| if along_x { (a, l) } else { (l, a) };
        let (lo, hi) = ((along0 - SEARCH_ALONG).max(2), (along0 + SEARCH_ALONG).min(W - 3));
        for a in lo..hi {
            let (x, y) = cell(a, line0);
            let lv = level_at(b, x, y);
            if want.is_some_and(|l| l != lv) || !open_at(b, w, x, y, lv) {
                continue;
            }
            // The last open cell of a run at its level: look on, a deck's length at most, for the next
            // open cell of that level across a gap.
            let (nx, ny) = cell(a + 1, line0);
            if open_at(b, w, nx, ny, lv) {
                continue;
            }
            let mut c = a + 2;
            while c < hi && c <= a + SPAN_MAX + 1 {
                let (cx, cy) = cell(c, line0);
                if open_at(b, w, cx, cy, lv) {
                    break;
                }
                c += 1;
            }
            let len = c - a - 1;
            let min_len = if lv == 0 { 2 } else { SPAN_MIN };
            if c < hi && len >= min_len && len <= SPAN_MAX {
                // Water alone between two banks of one level: a plank bridge.
                let plank = (a + 1..c).all(|m| {
                    (0..width).all(|k| {
                        let (mx, my) = cell(m, line0 - half + k);
                        w.at(mx, my)
                    })
                }) && (0..width).all(|k| {
                    let (bx, by) = cell(a, line0 - half + k);
                    let (nx, ny) = cell(c, line0 - half + k);
                    open_at(b, w, bx, by, lv) && open_at(b, w, nx, ny, lv)
                });
                let rect = rect_of(along_x, a + 1, c - 1, line0 - half, width);
                if plank || (lv > 0 && deck_ok(b, w, rect, along_x, lv)) {
                    let d = t.abs() * 2 + ((a + c) / 2 - along0).abs();
                    out.push((d, Deck { rect, along_x, level: lv, plank }));
                }
            }
        }
    }
    out.sort_by_key(|(d, k)| (*d, k.rect.y, k.rect.x));
    out
}

/// Is a deck clear of everything else: no join or other span within two cells, and (when `pads`) no
/// pad's ground under it? Boards over the channel may lie in a pad's claim margin: a site on the river's
/// bank is reached by them.
fn clear(b: &Blocks, joins: &[Join], spans: &[Span], r: Rect, pads: bool) -> bool {
    let near = r.grow(2);
    joins.iter().all(|j| !j.rect.overlaps(near) && j.landings.iter().all(|l| !l.overlaps(near)))
        && spans.iter().all(|s| !s.rect.overlaps(near))
        && (!pads || {
            let br = BRect::of_cells(r);
            (br.y.max(0)..br.bottom().min(BW))
                .all(|y| (br.x.max(0)..br.right().min(BW)).all(|x| b.flags[Blocks::ix(x, y)] & PAD == 0))
        })
}

fn deck_width(k: SpanKind) -> i32 {
    match k {
        SpanKind::Footbridge | SpanKind::Walkway => 2,
        _ => 3,
    }
}

/// The deck's centreline points 6 cells out from each end.
fn ends_of(r: Rect, along_x: bool) -> [(i32, i32); 2] {
    if along_x {
        let y = r.y + r.h / 2;
        [(r.x - 7, y), (r.right() + 6, y)]
    } else {
        let x = r.x + r.w / 2;
        [(x, r.y - 7), (x, r.bottom() + 6)]
    }
}

/// What the road or path onto a deck is: a ramp for a road span, a stair for a footpath.
fn climb_kind(k: SpanKind) -> (JoinKind, i32) {
    match k {
        SpanKind::Footbridge | SpanKind::Walkway => (JoinKind::Stair, 4),
        _ => (JoinKind::Ramp, 6),
    }
}

/// A deck from a terrace out over lower ground to an embankment raised for it (section 4): found
/// nearest `base`, the platform's blocks raised to the deck's level, a stair or ramp down its far end.
/// `level` fixes the deck's level (the Works' for a walkway). The blocks are raised only if all of it fits.
#[allow(clippy::too_many_arguments)]
fn platform_deck(
    b: &mut Blocks,
    w: &WaterCut,
    base: (i32, i32),
    along_x: bool,
    width: i32,
    kind: SpanKind,
    level: Option<u8>,
    joins: &[Join],
    spans: &[Span],
) -> Option<(Deck, Join, BRect)> {
    let half = width / 2;
    let (jkind, jw) = climb_kind(kind);
    let mut ts: Vec<i32> = (-SEARCH_ACROSS..=SEARCH_ACROSS).collect();
    ts.sort_by_key(|t| (t.abs(), *t));
    for t in ts {
        let (line0, along0) = if along_x { (base.1 + t, base.0) } else { (base.0 + t, base.1) };
        let cell = |a: i32, l: i32| if along_x { (a, l) } else { (l, a) };
        // The platform's cross-axis origin: a multiple of 8 holding the deck with a cell either side.
        let lo_q = line0 + half - (EMB_ACROSS - 2);
        let q0 = lo_q + (EMB_ACROSS - lo_q.rem_euclid(EMB_ACROSS)) % EMB_ACROSS;
        if q0 + 1 > line0 - half || line0 + half > q0 + EMB_ACROSS - 2 || q0 < 8 || q0 + EMB_ACROSS >= W - 8 {
            continue;
        }
        for dir in [1i32, -1] {
            // The terrace's edge: an open cell of the deck's level with a face or rim just past it.
            let mut edges: Vec<(i32, i32, u8)> = Vec::new();
            for a in (along0 - SEARCH_ALONG).max(40)..=(along0 + SEARCH_ALONG).min(W - 40) {
                let (x, y) = cell(a, line0);
                let lv = level_at(b, x, y);
                let (nx, ny) = cell(a + dir, line0);
                if lv >= 1 && level.is_none_or(|l| l == lv) && open_at(b, w, x, y, lv) && cliff_at(b, nx, ny) {
                    edges.push(((a - along0).abs(), a, lv));
                }
            }
            edges.sort();
            for &(_, a, lv) in edges.iter().take(3) {
                // The platform's along-axis origins: multiples of 8 that leave the deck 6 to 36 long.
                let (lo_p, hi_p) = if dir > 0 { (a + 6, a + 36) } else { (a - 52, a - 19) };
                let first = lo_p + (8 - lo_p.rem_euclid(8)) % 8;
                for p0 in (first..=hi_p).step_by(8) {
                    // The deck's far abutment along the axis, by side: the platform's near rim, or for
                    // a platform on the low side its face's first row (a platform laid across: its rim).
                    let abut = if dir > 0 {
                        p0
                    } else if along_x {
                        p0 + EMB_ALONG - 1
                    } else {
                        p0 + EMB_ALONG - 2
                    };
                    let (a0, a1) = if dir > 0 { (a + 1, abut) } else { (abut, a - 1) };
                    let len = a1 - a0 + 1;
                    if !(6..=SPAN_MAX - 4).contains(&len) {
                        continue;
                    }
                    // The platform's blocks: 2 across the axis, 4 along it.
                    let blocks = if along_x {
                        BRect::new(p0 / B, q0 / B, EMB_ALONG / B, EMB_ACROSS / B)
                    } else {
                        BRect::new(q0 / B, p0 / B, EMB_ACROSS / B, EMB_ALONG / B)
                    };
                    if blocks.x < 2 || blocks.y < 2 || blocks.right() >= BW - 2 || blocks.bottom() >= BW - 2 {
                        continue;
                    }
                    let free = (blocks.y..blocks.bottom()).all(|y| {
                        (blocks.x..blocks.right()).all(|x| {
                            let i = Blocks::ix(x, y);
                            b.level[i] == lv - 1 && b.flags[i] & (EXEMPT & !WALL) == 0
                        })
                    });
                    let cells = blocks.cells();
                    if !free || cells.grow(1).cells().any(|(x, y)| w.at(x, y)) {
                        continue;
                    }
                    let deck_rect = rect_of(along_x, a0, a1, line0 - half, width);
                    if !clear(b, joins, spans, deck_rect.grow(1), true) || !clear(b, joins, spans, cells, true) {
                        continue;
                    }
                    // Raise it, then see that the deck and a way down fit.
                    let win = (blocks.x - 3, blocks.y - 3, blocks.right() + 3, blocks.bottom() + 3);
                    let was_clean = shape_issue_in(b, win.0, win.1, win.2, win.3).is_none();
                    let saved: Vec<(usize, u8, u8)> = (blocks.y..blocks.bottom())
                        .flat_map(|y| (blocks.x..blocks.right()).map(move |x| Blocks::ix(x, y)))
                        .map(|i| (i, b.level[i], b.flags[i]))
                        .collect();
                    for &(i, _, _) in &saved {
                        b.level[i] = lv;
                        b.flags[i] |= CUT | FIXED;
                    }
                    if deck_ok(b, w, deck_rect, along_x, lv) {
                        // The way down: on the platform's south face (its last block row for a platform
                        // laid across, or at its far end), nearest the deck's axis.
                        let (face, by, xs) = if along_x {
                            let by = blocks.bottom() - 1;
                            let far = if dir > 0 { cells.right() - 2 - jw } else { cells.x + 2 };
                            (Face::South, by, far)
                        } else if dir > 0 {
                            (Face::South, blocks.bottom() - 1, q0 + 1)
                        } else {
                            (Face::North, blocks.y, q0 + 1)
                        };
                        if let Some(j) = make_join(b, w, face, by, xs, jw, jkind, lv, lv - 1) {
                            if !was_clean || shape_issue_in(b, win.0, win.1, win.2, win.3).is_none() {
                                return Some((Deck { rect: deck_rect, along_x, level: lv, plank: false }, j, blocks));
                            }
                        }
                    }
                    for (i, l, f) in saved {
                        b.level[i] = l;
                        b.flags[i] = f;
                    }
                }
            }
        }
    }
    None
}

/// The rail arch (section 4): a cutting into the plateau at the Lowfields' face, a ramp at its head up
/// onto the plateau, the arch over it near the mouth, and track on the plateau either side ending at
/// buffers. Returns the span, its meta and the ramp.
fn rail_arch(b: &mut Blocks, w: &WaterCut, plan: &MacroPlan, c: &Crossing, ci: u8) -> Option<(Span, SpanMeta, Join)> {
    let gi = c.gate?;
    let g = &plan.gates[usize::from(gi)];
    let mid = {
        let ca = |q: (u8, u8)| (i32::from(q.0) * STEP_CELLS + 8, i32::from(q.1) * STEP_CELLS + 8);
        let (p, q) = (ca(g.a), ca(g.b));
        ((p.0 + q.0) / 2, (p.1 + q.1) / 2)
    };
    let (mbx, mby) = (mid.0 / B, mid.1 / B);
    let r = 96 / B;
    let (cw, ch) = (GROUP, 2 * GROUP);
    let mut best: Option<Notch> = None;
    // Levels the two districts really have here.
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
    for (upper, lower_d) in [(g.db, g.da), (g.da, g.db)] {
        let (ups, los) = (levels(upper), levels(lower_d));
        for &up in ups.iter().rev() {
            for &lo in &los {
                if up != lo + 1 {
                    continue;
                }
                let mut ny = ((mby - r).max(3 * GROUP) / GROUP) * GROUP;
                while ny < (mby + r).min(BW - 4 * GROUP) {
                    let mut nx = ((mbx - r).max(2 * GROUP) / GROUP) * GROUP;
                    while nx < (mbx + r).min(BW - 4 * GROUP) {
                        let notch = BRect::new(nx, ny, cw, ch);
                        let blocks_ok = (ny..ny + ch).all(|y| {
                            (nx..nx + cw).all(|x| {
                                let i = Blocks::ix(x, y);
                                b.district[i] == upper && b.level[i] == up && b.flags[i] & (EXEMPT | VALLEY) == 0
                            })
                        });
                        // Open to the lower ground at its south end; upper ground to the north.
                        let mouth = (nx..nx + cw).all(|x| b.lv(x, ny + ch) == Some(lo));
                        let head = (nx..nx + cw).all(|x| (1..=4).all(|k| b.lv(x, ny - k) == Some(up)));
                        let walls = (ny..ny + ch).all(|y| b.lv(nx - 1, y) == Some(up) && b.lv(nx + cw, y) == Some(up));
                        if blocks_ok && mouth && head && walls {
                            let area = notch.grow(2).cells();
                            let hit = {
                                let br = BRect::of_cells(area);
                                (br.y.max(0)..br.bottom().min(BW)).any(|y| {
                                    (br.x.max(0)..br.right().min(BW))
                                        .any(|x| b.flags[Blocks::ix(x, y)] & (EXEMPT | VALLEY) != 0)
                                })
                            };
                            if !hit {
                                let key = ((nx * B + 4 - mid.0).abs() + ((ny + ch) * B - mid.1).abs(), ny, nx);
                                if best.is_none_or(|(k, ..)| key < k) {
                                    best = Some((key, nx, ny, up, lo));
                                }
                            }
                        }
                        nx += GROUP;
                    }
                    ny += GROUP;
                }
            }
        }
    }
    let (_, nx, ny, up, lo) = best?;
    let win = (nx - 3, ny - 5, nx + cw + 3, ny + ch + 3);
    let was_clean = shape_issue_in(b, win.0, win.1, win.2, win.3).is_none();
    let lower_d = b.district[Blocks::ix(nx, ny + ch)];
    let saved: Vec<(usize, u8, u8, u8)> = (ny - 1..ny + ch)
        .flat_map(|y| (nx..nx + cw).map(move |x| Blocks::ix(x, y)))
        .map(|i| (i, b.level[i], b.flags[i], b.district[i]))
        .collect();
    for y in ny..ny + ch {
        for x in nx..nx + cw {
            let i = Blocks::ix(x, y);
            b.level[i] = lo;
            b.district[i] = lower_d;
            b.flags[i] |= CUT | FIXED;
        }
    }
    for x in nx..nx + cw {
        b.flags[Blocks::ix(x, ny - 1)] |= WALL;
    }
    // The ramp at the head: the face of the block row above the notch, in the cutting's width.
    let ramp = make_join(b, w, Face::South, ny - 1, nx * B + 1, 6, JoinKind::Ramp, up, lo);
    // The arch across the cutting, near its mouth: rims either side are its abutments.
    let (x0, x1) = (nx * B - 1, (nx + cw) * B);
    let mouth_y = (ny + ch) * B;
    let mut arch: Option<Rect> = None;
    for dy in 4..(ch * B - 8) {
        let rect = Rect::new(x0, mouth_y - dy - 3, x1 - x0 + 1, 3);
        if deck_ok(b, w, rect, true, up) {
            arch = Some(rect);
            break;
        }
    }
    let ok_shape = !was_clean || shape_issue_in(b, win.0, win.1, win.2, win.3).is_none();
    if let (Some(mut ramp), Some(rect), true) = (ramp, arch, ok_shape) {
        ramp.gate = c.gate;
        // Track runs 13 cells onto the plateau each side of the arch (as far as it is open ground) and
        // ends at a buffer.
        let ym = rect.y + 1;
        let (mut track, mut buffers) = (Vec::new(), Vec::new());
        for dir in [-1i32, 1] {
            let start = if dir < 0 { rect.x - 1 } else { rect.right() };
            let mut n = 0;
            while n < 13 && open_at(b, w, start + dir * n, ym, up) {
                n += 1;
            }
            if n >= 1 {
                track.push(Rect::new(if dir < 0 { start - n + 1 } else { start }, ym, n, 1));
            }
            if open_at(b, w, start + dir * n, ym, up) {
                buffers.push(Rect::new(start + dir * n, ym, 1, 1));
            }
        }
        let span = Span { rect, along_x: true, deck_level: up, broken: false, kind: SpanKind::Rail };
        let meta = SpanMeta {
            crossing: ci,
            over: Over::Cutting,
            ends: ends_of(rect, true),
            embankment: None,
            abutments: Vec::new(),
            track,
            buffers,
        };
        b.flag_rect(BRect::of_cells(rect).grow(1), FIXED);
        return Some((span, meta, ramp));
    }
    for (i, l, f, d) in saved {
        b.level[i] = l;
        b.flags[i] = f;
        b.district[i] = d;
    }
    None
}

/// The spans of the plan's crossings (section 4), the plank bridges, and any joins a span makes (an
/// embankment's stair, a cutting's ramp). A crossing no ground will hold is `CarveFail::Span`.
pub fn lay_spans(
    plan: &MacroPlan,
    _salt: u8,
    b: &mut Blocks,
    w: &mut WaterCut,
    joins: &mut Vec<Join>,
) -> Result<Laid, CarveFail> {
    let mut spans: Vec<Span> = Vec::new();
    let mut meta: Vec<SpanMeta> = Vec::new();
    let mut planks: Vec<Rect> = Vec::new();
    for (ci, c) in plan.crossings.iter().enumerate() {
        let ci8 = ci as u8;
        let width = deck_width(c.kind);
        let base = base_of(c, w);
        if c.kind == SpanKind::Rail {
            match rail_arch(b, w, plan, c, ci8) {
                Some((s, m, ramp)) => {
                    spans.push(s);
                    meta.push(m);
                    joins.push(ramp);
                }
                // No ground for a cutting: the gate is an ordinary ramp road up the face.
                None => fallback(plan, b, w, c, 6, JoinKind::Ramp, joins),
            }
            continue;
        }
        let mut placed = false;
        let along_x = deck_axis(c, w);
        let want = deck_level_wanted(plan, c);
        if c.kind != SpanKind::Walkway {
            for (_, d) in decks_near(b, w, base, along_x, width, want) {
                if !clear(b, joins, &spans, d.rect.grow(1), !d.plank) {
                    continue;
                }
                if d.plank && c.route.is_some() && c.kind == SpanKind::Bridge {
                    // Both banks level with the water between: boards, not a span. One cell of bank
                    // each side carries them.
                    let r = d.rect;
                    planks.push(if d.along_x {
                        Rect::new(r.x - 1, r.y, r.w + 2, r.h)
                    } else {
                        Rect::new(r.x, r.y - 1, r.w, r.h + 2)
                    });
                    b.flag_rect(BRect::of_cells(d.rect).grow(1), FIXED);
                    placed = true;
                    break;
                }
                let (rect, abutments) = if d.plank {
                    // A span over a stream between level banks: stone abutments are made at each end.
                    let r = if d.along_x {
                        Rect::new(d.rect.x - 1, d.rect.y, d.rect.w + 2, d.rect.h)
                    } else {
                        Rect::new(d.rect.x, d.rect.y - 1, d.rect.w, d.rect.h + 2)
                    };
                    let ends = if d.along_x {
                        alloc::vec![Rect::new(r.x, r.y, 1, r.h), Rect::new(r.right() - 1, r.y, 1, r.h)]
                    } else {
                        alloc::vec![Rect::new(r.x, r.y, r.w, 1), Rect::new(r.x, r.bottom() - 1, r.w, 1)]
                    };
                    (r, ends)
                } else {
                    (d.rect, Vec::new())
                };
                spans.push(Span {
                    rect,
                    along_x: d.along_x,
                    deck_level: d.level,
                    broken: c.kind == SpanKind::Footbridge,
                    kind: c.kind,
                });
                meta.push(SpanMeta {
                    crossing: ci8,
                    over: c.over,
                    ends: ends_of(rect, d.along_x),
                    embankment: None,
                    abutments,
                    track: Vec::new(),
                    buffers: Vec::new(),
                });
                b.flag_rect(BRect::of_cells(rect).grow(1), FIXED);
                placed = true;
                break;
            }
        }
        if !placed {
            let level = if c.kind == SpanKind::Walkway { walkway_level(plan, c) } else { want.filter(|&l| l >= 1) };
            let Some((d, mut j, blocks)) = platform_deck(b, w, base, along_x, width, c.kind, level, joins, &spans)
            else {
                match c.kind {
                    // No ground for a walkway: its gate is a stair between the levels.
                    SpanKind::Walkway => fallback(plan, b, w, c, 4, JoinKind::Stair, joins),
                    // The broken footbridge is a verb's shortcut, never the only way.
                    SpanKind::Footbridge => {}
                    // No ground for the viaduct: its gate is an ordinary ramp road.
                    SpanKind::Viaduct => fallback(plan, b, w, c, 6, JoinKind::Ramp, joins),
                    // A road over a river that no deck holds: boards across the channel at the floor,
                    // where a bank of one level faces another over the water alone.
                    SpanKind::Bridge => {
                        let plank = decks_near(b, w, base, along_x, width, None)
                            .into_iter()
                            .find(|(_, d)| d.plank && clear(b, joins, &spans, d.rect.grow(1), false));
                        let Some((_, d)) = plank else { return Err(CarveFail::Span(ci8)) };
                        let r = d.rect;
                        planks.push(if d.along_x {
                            Rect::new(r.x - 1, r.y, r.w + 2, r.h)
                        } else {
                            Rect::new(r.x, r.y - 1, r.w, r.h + 2)
                        });
                        b.flag_rect(BRect::of_cells(d.rect).grow(1), FIXED);
                    }
                    SpanKind::Rail => return Err(CarveFail::Span(ci8)),
                }
                continue;
            };
            j.gate = c.gate;
            b.flag_rect(BRect::of_cells(d.rect).grow(1), FIXED);
            b.flag_rect(blocks.grow(1), FIXED);
            for l in j.landings {
                b.flag_rect(BRect::of_cells(l).grow(1), FIXED);
            }
            let emb = blocks.cells();
            spans.push(Span {
                rect: d.rect,
                along_x: d.along_x,
                deck_level: d.level,
                broken: c.kind == SpanKind::Footbridge,
                kind: c.kind,
            });
            meta.push(SpanMeta {
                crossing: ci8,
                over: c.over,
                ends: ends_of(d.rect, d.along_x),
                embankment: Some(emb),
                abutments: Vec::new(),
                track: Vec::new(),
                buffers: Vec::new(),
            });
            joins.push(j);
        }
    }
    Ok((spans, meta, planks))
}

/// A crossing with no span: its gate laid as a plain join of `kind`, if the plan's gate is a join's.
fn fallback(
    plan: &MacroPlan,
    b: &mut Blocks,
    w: &WaterCut,
    c: &Crossing,
    width: i32,
    kind: JoinKind,
    joins: &mut Vec<Join>,
) {
    let Some(gi) = c.gate else { return };
    for mut j in super::joins::place(plan, b, w, &plan.gates[usize::from(gi)], width, kind) {
        j.gate = c.gate;
        joins.push(j);
    }
}

/// The level a crossing's deck is at, where the plan fixes it (section 4): a road's bridge at the
/// terrace's level, the highest of the ground the route runs on at the river (planks serve where that is
/// the valley floor's own level 0); a viaduct at the higher end of its gate. A footbridge has no say.
pub(super) fn deck_level_wanted(plan: &MacroPlan, c: &Crossing) -> Option<u8> {
    let lv = |q: (u8, u8)| plan.level_at(i32::from(q.0), i32::from(q.1));
    match c.kind {
        SpanKind::Bridge => {
            let route = &plan.routes[usize::from(c.route?)];
            let k = route.pts.iter().position(|&q| q == c.at)?;
            let before = k.checked_sub(1).map(|j| lv(route.pts[j]));
            let after = route.pts.get(k + 1).map(|&q| lv(q));
            Some(before.unwrap_or(0).max(after.unwrap_or(0)).max(lv(c.at)))
        }
        SpanKind::Viaduct => {
            let g = &plan.gates[usize::from(c.gate?)];
            Some(plan.districts[usize::from(g.da)].level.max(plan.districts[usize::from(g.db)].level))
        }
        _ => None,
    }
}

/// A walkway's deck level: the Works' side of its gate.
fn walkway_level(plan: &MacroPlan, c: &Crossing) -> Option<u8> {
    let g = &plan.gates[usize::from(c.gate?)];
    let (la, lb) = (plan.districts[usize::from(g.da)].level, plan.districts[usize::from(g.db)].level);
    Some(la.max(lb))
}

/// The axis of a crossing's deck. Over a river it lies across the channel's heading where the channel
/// is nearest the crossing (the plan's own axis is the road's, and a road may run along a bend of the
/// river); a vertical channel takes an east-west deck.
fn deck_axis(c: &Crossing, w: &WaterCut) -> bool {
    if c.over != Over::River {
        return c.along_x;
    }
    let mid = (i32::from(c.at.0) * STEP_CELLS + 8, i32::from(c.at.1) * STEP_CELLS + 8);
    let Some((k, p)) =
        w.channel.iter().enumerate().min_by_key(|(_, p)| ((p.x - mid.0).abs() + (p.y - mid.1).abs(), p.x, p.y))
    else {
        return c.along_x;
    };
    let near = |j: usize| w.channel.get(j).filter(|q| q.reach == p.reach).unwrap_or(p);
    let (a, z) = (near(k.saturating_sub(2)), near(k + 2));
    (z.y - a.y).abs() >= (z.x - a.x).abs()
}

/// Where a crossing is looked for: the river's point nearest it for a river crossing, else the
/// middle of its macro cell.
fn base_of(c: &Crossing, w: &WaterCut) -> (i32, i32) {
    let mid = (i32::from(c.at.0) * STEP_CELLS + 8, i32::from(c.at.1) * STEP_CELLS + 8);
    if c.over == Over::River {
        if let Some(p) = w.channel.iter().min_by_key(|p| ((p.x - mid.0).abs() + (p.y - mid.1).abs(), p.x, p.y)) {
            return (p.x, p.y);
        }
    }
    mid
}

/// The tile a made abutment lays.
pub const ABUTMENT: Tile = Tile::Cliff;
