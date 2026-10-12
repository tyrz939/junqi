//! Water (PLAN-map-r4 section 3): the river's centreline from the plan's macro chain (smoothed,
//! meandering, held off the region borders and straight and square at its crossings), the valley
//! floor cut round it, the channel's cells, and the lake, mere and ponds as cell bits.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::{Rect, ZoneId};

use super::blocks::{Blocks, GROUP, PAD, Pad, RIVER, VALLEY};
use super::noise::{Lattice, salt_b};
use super::{B, BW, W};
use crate::bits::Bits;
use crate::county::terraced::macro_plan::{MH, MW, MacroPlan, Over, Reg, STEP_CELLS, Wet, idx};
use crate::steps::{Step, dice};

/// The centreline's resampling step, in cells.
pub const STEP: i32 = 4;
/// A point nearer than this many cells to a region border or a pad is pushed away (anti-hug), by
/// at most `HUG_STEP` cells a round, for `HUG_ROUNDS` rounds.
pub const HUG_NEED: i32 = 10;
pub const HUG_STEP: i32 = 6;
pub const HUG_ROUNDS: usize = 6;
/// `border_dist` looks this far (cells).
pub const HUG: i32 = 12;
/// The straight window each side of a crossing (cells along the channel), and the ramp out of it.
pub const STRAIGHT: i32 = 12;
pub const STRAIGHT_RAMP: i32 = 8;
/// Fixed point: the centreline is carried in sixteenths of a cell.
const FP: i64 = 16;

/// One point of the channel's centreline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiverPt {
    pub x: i32,
    pub y: i32,
    /// Half the channel's width: the channel is `2 * half + 1` cells across.
    pub half: u8,
    /// The ground's level under it.
    pub level: u8,
    /// 0 the inflow (spring to the lake), 1 the outflow (lake to the map edge).
    pub reach: u8,
}

/// The water of the carve: the channel's centreline and the cells every body of water covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaterCut {
    /// Water cells, `y * W + x`.
    pub bits: Bits,
    /// The centreline, the inflow's points then the outflow's.
    pub channel: Vec<RiverPt>,
    /// Where the river falls over a face (the cliff cells its channel crosses).
    pub falls: Vec<Rect>,
    /// The spring.
    pub source: Option<(i32, i32)>,
}

impl WaterCut {
    pub fn empty() -> Self {
        Self { bits: Bits::new((W * W) as usize, false), channel: Vec::new(), falls: Vec::new(), source: None }
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < W && y < W && self.bits.get((y * W + x) as usize)
    }
}

pub fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// An integer triangle wave, `-16..=16`, of period `l` at `s`, its peaks rounded (the cubic
/// `t (3 - t^2) / 2` of the triangle `t`: a sine's shape, so a meander has bends, not corners).
fn tri(s: i64, l: i64) -> i64 {
    let p = s.rem_euclid(l) * 64 / l;
    let t = if p < 16 {
        p
    } else if p < 48 {
        32 - p
    } else {
        p - 64
    };
    t * (3 * 256 - t * t) / 512
}

type P = (i64, i64);

fn chaikin(pts: &[P]) -> Vec<P> {
    let mut out = vec![pts[0]];
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        out.push(((3 * a.0 + b.0) / 4, (3 * a.1 + b.1) / 4));
        out.push(((a.0 + 3 * b.0) / 4, (a.1 + 3 * b.1) / 4));
    }
    out.push(pts[pts.len() - 1]);
    out
}

fn dist(a: P, b: P) -> i64 {
    isqrt((a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1))
}

/// The polyline at points every `step` of arc length (in the polyline's units), the last point
/// last.
fn resample(pts: &[P], step: i64) -> Vec<P> {
    let mut out = vec![pts[0]];
    let mut carry = 0i64;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = dist(a, b);
        if len == 0 {
            continue;
        }
        let mut at = step - carry;
        while at <= len {
            out.push((a.0 + (b.0 - a.0) * at / len, a.1 + (b.1 - a.1) * at / len));
            at += step;
        }
        carry = len - (at - step);
    }
    let last = pts[pts.len() - 1];
    if out.last() != Some(&last) {
        out.push(last);
    }
    out
}

/// The macro cell's centre, in sixteenths of a cell.
fn mc(c: (u8, u8)) -> P {
    ((i64::from(c.0) * i64::from(STEP_CELLS) + 8) * FP, (i64::from(c.1) * i64::from(STEP_CELLS) + 8) * FP)
}

/// The region of the block under cell `(x, y)` (clamped to the map).
pub(super) fn region_at(plan: &MacroPlan, b: &Blocks, x: i32, y: i32) -> Reg {
    let i = Blocks::ix((x / B).clamp(0, BW - 1), (y / B).clamp(0, BW - 1));
    plan.districts[usize::from(b.district[i])].reg
}

/// Cells from `(x, y)` to the nearest block of a region other than `me`, or a pad, up to `HUG + B`
/// (more is one past it).
pub(super) fn border_dist(plan: &MacroPlan, b: &Blocks, x: i32, y: i32, me: Reg) -> i32 {
    let r = (HUG + B) / B;
    let (bx, by) = (x / B, y / B);
    let mut best = HUG + B + 1;
    for oy in -r..=r {
        for ox in -r..=r {
            let (nx, ny) = (bx + ox, by + oy);
            if !Blocks::inside(nx, ny) {
                continue;
            }
            let i = Blocks::ix(nx, ny);
            // Another region, or a pad (a set place's ground the river keeps clear of).
            if plan.districts[usize::from(b.district[i])].reg != me || b.flags[i] & PAD != 0 {
                // The nearest cell of that block.
                let dx = (nx * B - x).max(0).max(x - (nx * B + B - 1));
                let dy = (ny * B - y).max(0).max(y - (ny * B + B - 1));
                best = best.min(dx.max(dy));
            }
        }
    }
    best
}

/// The macro cell of kind `lake` next to `c`, if any.
fn lake_next(plan: &MacroPlan, c: (u8, u8)) -> Option<(u8, u8)> {
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
        let (nx, ny) = (i32::from(c.0) + dx, i32::from(c.1) + dy);
        if (0..MW).contains(&nx) && (0..MH).contains(&ny) && plan.wet[idx(nx, ny)] == Wet::Lake {
            return Some((nx as u8, ny as u8));
        }
    }
    None
}

/// The Works cell next to the source (the way the river comes down from), as a unit step.
fn works_next(plan: &MacroPlan, c: (u8, u8)) -> Option<(i32, i32)> {
    for (dx, dy) in [(0, -1), (1, 0), (-1, 0), (0, 1)] {
        let (nx, ny) = (i32::from(c.0) + dx, i32::from(c.1) + dy);
        if (0..MW).contains(&nx) && (0..MH).contains(&ny) && plan.reg_at(nx, ny) == Reg::Works {
            return Some((dx, dy));
        }
    }
    None
}

/// How far (cells) the river keeps off a pad's grown box, at most: the repel distance, the meander's
/// swing and the corner a smoothing cuts. A line that begins or ends nearer than that (by the lake, at
/// a spring) is held to the next margin down.
const PAD_MARGINS: [i32; 4] = [30, 20, 12, 6];

/// The polyline `pts` with each stretch that runs through a pad's ground (the grown box and a margin)
/// routed round the pad's rectangle instead, by the shorter way that keeps to the river's own region
/// and the map. A stretch with an end of the line inside a pad's rect has no way round, and is tried
/// again with the next margin. Sixteenths of a cell throughout.
fn detour(plan: &MacroPlan, pads: &[Pad], pts: &[P]) -> Option<Vec<P>> {
    let mut path = pts.to_vec();
    let mut any = false;
    for margin in PAD_MARGINS {
        // The pads' rects, merged where they overlap (a canteen beside its factory is one obstacle).
        let mut rects: Vec<(i32, i32, i32, i32)> = pads
            .iter()
            .map(|p| {
                let g = p.grown.grow(margin);
                (g.x, g.y, g.right(), g.bottom())
            })
            .collect();
        loop {
            let mut merged = false;
            'scan: for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    let (a, c) = (rects[i], rects[j]);
                    if a.0 < c.2 && c.0 < a.2 && a.1 < c.3 && c.1 < a.3 {
                        rects[i] = (a.0.min(c.0), a.1.min(c.1), a.2.max(c.2), a.3.max(c.3));
                        rects.remove(j);
                        merged = true;
                        break 'scan;
                    }
                }
            }
            if !merged {
                break;
            }
        }
        for pass in 0..3 {
            let mut changed = false;
            for &r in &rects {
                let inside = |p: P| {
                    let (x, y) = ((p.0 / FP) as i32, (p.1 / FP) as i32);
                    x >= r.0 && x < r.2 && y >= r.1 && y < r.3
                };
                let mut out: Vec<P> = Vec::with_capacity(path.len() + 8);
                let mut i = 0;
                while i < path.len() {
                    if !inside(path[i]) {
                        out.push(path[i]);
                        i += 1;
                        continue;
                    }
                    let start = i;
                    while i < path.len() && inside(path[i]) {
                        i += 1;
                    }
                    if start == 0 || i == path.len() {
                        out.extend_from_slice(&path[start..i]);
                        continue;
                    }
                    let (e, x) = (path[start - 1], path[i]);
                    out.extend(round_rect(plan, r, e, x, &rects));
                    changed = true;
                }
                path = out;
            }
            any |= changed;
            if !changed || pass == 2 {
                break;
            }
        }
    }
    any.then_some(path)
}

/// The points that take a line from `e` (just outside rect `r`) round it to `x`: the nearest boundary
/// points and the corners between, clockwise or counter-clockwise, whichever costs less.
fn round_rect(plan: &MacroPlan, r: (i32, i32, i32, i32), e: P, x: P, all: &[(i32, i32, i32, i32)]) -> Vec<P> {
    let (x0, y0, x1, y1) = (r.0, r.1, r.2 - 1, r.3 - 1);
    let (w, h) = (x1 - x0, y1 - y0);
    let per = 2 * (w + h);
    let cx = |v: i64| ((v / FP) as i32).clamp(x0, x1);
    let cy = |v: i64| ((v / FP) as i32).clamp(y0, y1);
    // A point's place on the perimeter, clockwise from the top-left corner, and its boundary point.
    let on = |p: P| -> (i32, (i32, i32)) {
        let (px, py) = ((p.0 / FP) as i32, (p.1 / FP) as i32);
        if py < y0 {
            let q = (cx(p.0), y0);
            (q.0 - x0, q)
        } else if py > y1 {
            let q = (cx(p.0), y1);
            (w + h + (x1 - q.0), q)
        } else if px < x0 {
            let q = (x0, cy(p.1));
            (2 * w + h + (y1 - q.1), q)
        } else {
            let q = (x1, cy(p.1));
            (w + (q.1 - y0), q)
        }
    };
    let (te, be) = on(e);
    let (tx, bx) = on(x);
    let corners = [(0, (x0, y0)), (w, (x1, y0)), (w + h, (x1, y1)), (2 * w + h, (x0, y1))];
    let me = plan.reg_at(
        (((e.0 / FP) as i32) / STEP_CELLS).clamp(0, MW - 1),
        (((e.1 / FP) as i32) / STEP_CELLS).clamp(0, MH - 1),
    );
    let mut best: Option<(i32, Vec<(i32, i32)>)> = None;
    for cw in [true, false] {
        // Corners passed walking from `te` to `tx` this way.
        let span = if cw { (tx - te).rem_euclid(per) } else { (te - tx).rem_euclid(per) };
        let mut path: Vec<(i32, i32)> = vec![be];
        let mut passed: Vec<(i32, (i32, i32))> = corners
            .iter()
            .filter_map(|&(t, q)| {
                let d = if cw { (t - te).rem_euclid(per) } else { (te - t).rem_euclid(per) };
                (d > 0 && d < span).then_some((d, q))
            })
            .collect();
        passed.sort_by_key(|&(d, _)| d);
        path.extend(passed.into_iter().map(|(_, q)| q));
        path.push(bx);
        // The cost: the length, the samples off the river's region or inside another pad's ground, and
        // each corner that falls off the map.
        let mut cost = span;
        for pair in path.windows(2) {
            let n = ((pair[1].0 - pair[0].0).abs().max((pair[1].1 - pair[0].1).abs()) / 8).max(1);
            for k in 0..=n {
                let (sx, sy) =
                    (pair[0].0 + (pair[1].0 - pair[0].0) * k / n, pair[0].1 + (pair[1].1 - pair[0].1) * k / n);
                let (mx, my) = ((sx / STEP_CELLS).clamp(0, MW - 1), (sy / STEP_CELLS).clamp(0, MH - 1));
                if plan.reg_at(mx, my) != me {
                    cost += 6;
                }
                if all.iter().any(|&o| o != r && sx >= o.0 && sx < o.2 && sy >= o.1 && sy < o.3) {
                    cost += 12;
                }
            }
        }
        for &(px, py) in &path {
            if px < 6 || py < 6 || px > W - 7 || py > W - 7 {
                cost += 200;
            }
        }
        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
            best = Some((cost, path));
        }
    }
    let path = best.map(|(_, p)| p).unwrap_or_default();
    path.into_iter().map(|(px, py)| (i64::from(px.clamp(6, W - 7)) * FP, i64::from(py.clamp(6, W - 7)) * FP)).collect()
}

/// One reach of the river: the macro chain `chain` with its end points as given, smoothed, meandered
/// and held off the borders. `ends` are the extra points before and after the chain's cell centres.
#[allow(clippy::too_many_arguments)]
fn reach_line(
    plan: &MacroPlan,
    salt: u8,
    b: &Blocks,
    pads: &[Pad],
    pts: &[P],
    reach: u8,
    cross: &[P],
    free: i64,
) -> Vec<(P, u8)> {
    let (seed, at) = (plan.seed, plan.attempt);
    let mut r = dice(seed, ZoneId::County, Step::MapRiver, at, 10 + i32::from(reach), salt_b(salt, 0));
    // The plan's A1 6 to 12 and L1 80 to 128 leave 48-cell windows that are straight to within a cell
    // (a long wave's flank), which its own C-3 refuses; a wave of 40 to 52 cells and 10 to 14 of swing
    // has no window straighter than 5 cells (simulated over the second wave's range and every phase).
    let a1 = r.range(10, 14) as i64;
    let l1 = r.range(40, 52) as i64;
    let a2 = r.range(2, 4) as i64;
    let l2 = r.range(24, 40) as i64;
    let phase = r.range(0, 127) as i64;
    // The swing breathes between three quarters and the whole of A1 over a long wave, so a long reach
    // is not one sine (never below 7 cells: the windows stay bent).
    let l3 = r.range(140, 230) as i64;
    let p3 = r.range(0, 127) as i64;
    // The macro chain steps in 16-cell stairs, which Chaikin leaves as a ripple: two boxcar passes
    // (eight cells each way) take it out and leave the line's real course.
    let smooth = |pts: &[P]| -> Vec<P> {
        let mut base = resample(&chaikin(&chaikin(pts)), i64::from(STEP) * FP);
        for _ in 0..2 {
            let prev = base.clone();
            for k in 1..base.len() - 1 {
                let (lo, hi) = (k.saturating_sub(2), (k + 2).min(prev.len() - 1));
                let m = (hi - lo + 1) as i64;
                base[k] = (
                    prev[lo..=hi].iter().map(|p| p.0).sum::<i64>() / m,
                    prev[lo..=hi].iter().map(|p| p.1).sum::<i64>() / m,
                );
            }
        }
        base
    };
    let mut base = smooth(pts);
    // A stretch through a pad is routed round it, and the line rounded again.
    if let Some(routed) = detour(plan, pads, &base) {
        base = smooth(&routed);
    }
    let n = base.len();
    let s_of = |i: usize| i as i64 * i64::from(STEP);
    let total = s_of(n - 1);
    let cell = |p: P| -> (i32, i32) { (((p.0 + FP / 2) / FP) as i32, ((p.1 + FP / 2) / FP) as i32) };
    // The river's own region at each point: the one its macro chain runs in (not whatever a push has
    // moved the point over).
    // The region of the nearest cell of the macro chain: the smoothed line cuts the chain's corners and
    // so can lie over a neighbouring cell of another region.
    let chain_reg = |p: P| -> Reg {
        let q = pts.iter().min_by_key(|&&c| dist(c, p)).copied().unwrap_or(p);
        plan.reg_at(((q.0 / FP) as i32 / 16).clamp(0, MW - 1), ((q.1 / FP) as i32 / 16).clamp(0, MH - 1))
    };
    let regs: Vec<Reg> = base.iter().map(|&p| chain_reg(p)).collect();
    // Held off the region borders and the pads (anti-hug): a point nearer than `need` cells is pushed
    // along the gradient of its distance, and the line is rounded again.
    let repel = |line: &mut Vec<P>, need: i32, rounds: usize| {
        for _ in 0..rounds {
            for k in 1..n - 1 {
                if s_of(k) < free {
                    continue;
                }
                let (x, y) = cell(line[k]);
                let me = regs[k];
                let d = border_dist(plan, b, x, y, me);
                if d >= need {
                    continue;
                }
                let gx = border_dist(plan, b, x + 4, y, me) - border_dist(plan, b, x - 4, y, me);
                let gy = border_dist(plan, b, x, y + 4, me) - border_dist(plan, b, x, y - 4, me);
                let m = gx.abs().max(gy.abs());
                if m == 0 {
                    continue;
                }
                let push = i64::from((need - d).min(HUG_STEP)) * FP;
                line[k].0 += i64::from(gx) * push / i64::from(m);
                line[k].1 += i64::from(gy) * push / i64::from(m);
            }
            let prev = line.clone();
            for k in 1..n - 1 {
                if s_of(k) >= free {
                    line[k] = (
                        (prev[k - 1].0 + 2 * prev[k].0 + prev[k + 1].0) / 4,
                        (prev[k - 1].1 + 2 * prev[k].1 + prev[k + 1].1) / 4,
                    );
                }
            }
        }
    };
    // The centre line is first pushed far enough off the borders for the meander to swing either way
    // without touching them.
    repel(&mut base, HUG_NEED + a1 as i32 + 2, HUG_ROUNDS);
    // The tangent's normal, in 256ths.
    let nrm: Vec<P> = (0..n)
        .map(|i| {
            let (a, c) = (base[i.saturating_sub(1)], base[(i + 1).min(n - 1)]);
            let (tx, ty) = (c.0 - a.0, c.1 - a.1);
            let len = isqrt(tx * tx + ty * ty).max(1);
            (-ty * 256 / len, tx * 256 / len)
        })
        .collect();
    // The raw offset, in sixteenths: two waves, faded near the ends and the crossings.
    let o: Vec<i64> = (0..n)
        .map(|i| {
            let s = s_of(i);
            let breath = 14 + tri(s + p3, l3) / 8;
            let wave = a1 * breath * tri(s, l1) / 16 + a2 * tri(s + phase, l2);
            let end = s.min(total - s).min(24);
            let mut fade = end * 16 / 24;
            // The spring leg runs straight to the fall, and the meander grows in below it.
            if free > 0 {
                fade = fade.min(((s - free).clamp(0, 24)) * 16 / 24);
            }
            for &c in cross {
                let d = (0..n).map(|j| (j, dist(base[j], c))).min_by_key(|&(j, d)| (d, j)).map_or(0, |(j, _)| s_of(j));
                let t = (s - d).abs();
                let f = if t <= i64::from(STRAIGHT) {
                    0
                } else if t >= i64::from(STRAIGHT + STRAIGHT_RAMP) {
                    16
                } else {
                    (t - i64::from(STRAIGHT)) * 16 / i64::from(STRAIGHT_RAMP)
                };
                fade = fade.min(f);
            }
            wave * fade / 16
        })
        .collect();
    let mut line: Vec<P> =
        (0..n).map(|i| (base[i].0 + nrm[i].0 * o[i] / 256, base[i].1 + nrm[i].1 * o[i] / 256)).collect();
    repel(&mut line, HUG_NEED, 2);
    // The clamp (section 3.1, step 6) comes last, below: a point off its own ground is moved onto it.
    let valid = |k: usize, p: P| -> bool {
        let (x, y) = cell(p);
        if !(0..W).contains(&x) || !(0..W).contains(&y) {
            return false;
        }
        let (bx, by) = (x / B, y / B);
        let reg = plan.districts[usize::from(b.district[Blocks::ix(bx, by)])].reg;
        // Never the Works' ground below the spring leg (the river only starts there).
        reg != Reg::Works
            && (reg == regs[k] || reg == Reg::Waters)
            && (-1..=1).all(|oy| (-1..=1).all(|ox| b.fl(bx + ox, by + oy) & PAD == 0))
    };
    let mut out: Vec<(P, u8)> = line.into_iter().map(|p| (p, reach)).collect();
    // Straight and square at each crossing: the perpendicular drift is held out inside the window.
    for &c in cross {
        // The window's centre: the sample nearest the crossing.
        let Some(k0) = (0..n).min_by_key(|&j| (dist(out[j].0, c), j)) else { continue };
        let centre = out[k0].0;
        // The channel's heading there, from the line before the meander: the deck lies across it, so a
        // channel running mostly north-south takes an east-west deck, and the other way about (the
        // plan's own axis is the road's, which may run along a river's bend).
        let (ha, hz) = (base[k0.saturating_sub(3)], base[(k0 + 3).min(n - 1)]);
        let (tx, ty) = (hz.0 - ha.0, hz.1 - ha.1);
        let vertical = ty.abs() >= tx.abs();
        let sign = if (if vertical { ty } else { tx }) < 0 { -1 } else { 1 };
        for (k, o) in out.iter_mut().enumerate() {
            let t = (s_of(k) - s_of(k0)).abs();
            let w = if t <= i64::from(STRAIGHT) {
                16
            } else if t >= i64::from(STRAIGHT + STRAIGHT_RAMP) {
                0
            } else {
                16 - (t - i64::from(STRAIGHT)) * 16 / i64::from(STRAIGHT_RAMP)
            };
            if w == 0 {
                continue;
            }
            // Straight: the samples along the axis through the centre, a step apart.
            let ds = (s_of(k) - s_of(k0)) * FP * sign;
            let p = o.0;
            let q = if vertical { (centre.0, centre.1 + ds) } else { (centre.0 + ds, centre.1) };
            o.0 = ((p.0 * (16 - w) + q.0 * w) / 16, (p.1 * (16 - w) + q.1 * w) / 16);
        }
    }
    // The clamp: a point that has landed off its own ground (another region's blocks but the Waters',
    // the Works', a pad's) is moved along the normal to the nearest ground that is its own, or back to the
    // centre line; the neighbours are rounded to the move, and the pass runs again for what that undid.
    for _ in 0..4 {
        let mut moved = false;
        for k in 1..n - 1 {
            if s_of(k) < free || valid(k, out[k].0) {
                continue;
            }
            let p = out[k].0;
            let snapped = (1..=24).find_map(|d| {
                [1i64, -1].iter().find_map(|&sg| {
                    let q = (p.0 + nrm[k].0 * sg * d * 2 * FP / 256, p.1 + nrm[k].1 * sg * d * 2 * FP / 256);
                    valid(k, q).then_some(q)
                })
            });
            out[k].0 = snapped.unwrap_or(base[k]);
            moved = true;
        }
        if !moved {
            break;
        }
        // Round the line to the move, but never back onto ground that is not the river's.
        let prev: Vec<P> = out.iter().map(|o| o.0).collect();
        for k in 1..n - 1 {
            if s_of(k) >= free {
                let q = (
                    (prev[k - 1].0 + 2 * prev[k].0 + prev[k + 1].0) / 4,
                    (prev[k - 1].1 + 2 * prev[k].1 + prev[k + 1].1) / 4,
                );
                if valid(k, q) {
                    out[k].0 = q;
                }
            }
        }
    }
    out
}

/// The river's centreline (sections 3.1 and 3.3): the inflow from its spring to the lake, the
/// outflow from the lake to the map's edge. Empty with no river.
pub fn river_line(plan: &MacroPlan, salt: u8, b: &Blocks, pads: &[Pad]) -> (Vec<RiverPt>, usize, Option<(i32, i32)>) {
    if plan.river_in.is_empty() || plan.river_out.is_empty() {
        return (Vec::new(), 0, None);
    }
    // Where the plan's roads and gates cross the river: the straight windows.
    let mut cross_in: Vec<P> = Vec::new();
    let mut cross_out: Vec<P> = Vec::new();
    for c in &plan.crossings {
        if c.over != Over::River {
            continue;
        }
        let item = mc(c.at);
        let near = |chain: &[(u8, u8)]| {
            chain.iter().any(|q| {
                (i32::from(q.0) - i32::from(c.at.0)).abs() <= 1 && (i32::from(q.1) - i32::from(c.at.1)).abs() <= 1
            })
        };
        if near(&plan.river_in) {
            cross_in.push(item);
        } else if near(&plan.river_out) {
            cross_out.push(item);
        }
    }
    // The inflow's polyline: a spring inside the Works if the plan has a fall, then the chain's
    // centres, then into the lake.
    let mut pin: Vec<P> = plan.river_in.iter().map(|&c| mc(c)).collect();
    let mut free = 0;
    let mut spring = None;
    if let Some(dir) = plan.fall.and_then(|f| works_next(plan, f)) {
        // 16 to 32 cells inside the Works, but within the Works' ground as the blocks have it: the ray
        // from the fall's cell in the Works' direction gives the cells to the border and the depth.
        let mut r = dice(plan.seed, ZoneId::County, Step::MapRiver, plan.attempt, 5, salt_b(salt, 0));
        let mut inside = r.range(16, 32);
        let works_at = |k: i32| -> bool {
            let (x, y) = (
                ((pin[0].0 / FP) as i32 + dir.0 * k).clamp(0, W - 1),
                ((pin[0].1 / FP) as i32 + dir.1 * k).clamp(0, W - 1),
            );
            plan.districts[usize::from(b.district[Blocks::ix(x / B, y / B)])].reg == Reg::Works
        };
        let edge = (0..64).find(|&k| works_at(k)).unwrap_or(8);
        let depth = (edge..edge + 64).take_while(|&k| works_at(k)).count() as i32;
        inside = inside.min(depth - 8).max(depth / 3).max(2);
        let d = i64::from(edge + inside) * FP;
        let s = (pin[0].0 + i64::from(dir.0) * d, pin[0].1 + i64::from(dir.1) * d);
        let s = (s.0.clamp(8 * FP, i64::from(W - 8) * FP), s.1.clamp(8 * FP, i64::from(W - 8) * FP));
        spring = Some(((s.0 / FP) as i32, (s.1 / FP) as i32));
        free = dist(s, pin[0]) / FP + 4;
        pin.insert(0, s);
    }
    if let Some(last) = plan.river_in.last().and_then(|&c| lake_next(plan, c)) {
        pin.push(mc(last));
    }
    let mut pout: Vec<P> = plan.river_out.iter().map(|&c| mc(c)).collect();
    if let Some(first) = plan.river_out.first().and_then(|&c| lake_next(plan, c)) {
        pout.insert(0, mc(first));
    }
    // To the map's very edge.
    if let Some(&e) = plan.river_out.last() {
        let (ex, ey) = (i32::from(e.0), i32::from(e.1));
        let p = mc(e);
        let q = if ex == 0 {
            (0, p.1)
        } else if ex == MW - 1 {
            (i64::from(W - 1) * FP, p.1)
        } else if ey == 0 {
            (p.0, 0)
        } else {
            (p.0, i64::from(W - 1) * FP)
        };
        pout.push(q);
    }
    // The spring leg stays straight (free of the meander) and carries its own fade.
    let inflow = reach_line(plan, salt, b, pads, &pin, 0, &cross_in, free);
    let outflow = reach_line(plan, salt, b, pads, &pout, 1, &cross_out, 0);
    let split = inflow.len();
    let cell = |p: P| ((p.0 + FP / 2) / FP, (p.1 + FP / 2) / FP);
    let mut chan = Vec::with_capacity(split + outflow.len());
    for (k, &(p, reach)) in inflow.iter().chain(&outflow).enumerate() {
        let (x, y) = cell(p);
        let s = if reach == 0 { k as i64 * i64::from(STEP) } else { 1000 };
        let half = if reach == 1 {
            3
        } else if s < 24 || (spring.is_some() && s < free + 8) {
            1
        } else {
            2
        };
        // A bend that overshoots stays on the map.
        // The spring leg runs on the Works' ground (level 2), the rest on the valley floor.
        let level = if reach == 0 && spring.is_some() && s < free { 2 } else { 0 };
        chan.push(RiverPt { x: (x as i32).clamp(0, W - 1), y: (y as i32).clamp(0, W - 1), half, level, reach });
    }
    (chan, split, spring)
}

/// Whether `d` (an integer squared distance) is inside a channel of half-width `half`.
fn in_disc(d2: i32, half: i32) -> bool {
    d2 <= half * half + half
}

/// The channel's cells and the floor round it.
pub fn lay_channel(channel: &[RiverPt], bits: &mut Bits) {
    for w in channel.windows(2) {
        let (a, c) = (w[0], w[1]);
        if a.reach != c.reach {
            continue;
        }
        let n = (c.x - a.x).abs().max((c.y - a.y).abs()).max(1);
        let half = i32::from(a.half.min(c.half));
        let half = half.max(i32::from(a.half));
        for k in 0..=n {
            let (x, y) = (a.x + (c.x - a.x) * k / n, a.y + (c.y - a.y) * k / n);
            for dy in -half..=half {
                for dx in -half..=half {
                    if in_disc(dx * dx + dy * dy, half) {
                        let (px, py) = (x + dx, y + dy);
                        if px >= 0 && py >= 0 && px < W && py < W {
                            bits.set((py * W + px) as usize, true);
                        }
                    }
                }
            }
        }
    }
}

/// Where the water falls (section 3.3): the face and rim cells the channel's water covers, as rects a
/// row high merged down a fall's height. Their tiles are `Waterfall`; the plunge pool is the channel's
/// own water below the foot.
pub fn falls(b: &Blocks, out: &mut WaterCut) {
    let wpr = W as usize / 64;
    let mut rects: Vec<Rect> = Vec::new();
    for y in 0..W {
        let mut runs: Vec<(i32, i32)> = Vec::new();
        for k in 0..wpr {
            let mut word = out.bits.word(y as usize * wpr + k);
            while word != 0 {
                let x = (k * 64) as i32 + word.trailing_zeros() as i32;
                word &= word - 1;
                if super::tiles::cliff_at(b, x, y) {
                    match runs.last_mut() {
                        Some(r) if r.1 == x => r.1 = x + 1,
                        _ => runs.push((x, x + 1)),
                    }
                }
            }
        }
        for (x0, x1) in runs {
            match rects.iter_mut().rev().take(8).find(|r| r.bottom() == y && r.x == x0 && r.right() == x1) {
                Some(r) => r.h += 1,
                None => rects.push(Rect::new(x0, y, x1 - x0, 1)),
            }
        }
    }
    out.falls = rects;
}

/// The valley floor (section 3.2): every group of blocks whose centre is within `half + 7` cells of
/// the centreline goes to level 0, except pads and the Works; the groups the channel itself runs on
/// are the river's (`RIVER`), which tidy leaves be.
pub fn valley(plan: &MacroPlan, b: &mut Blocks, channel: &[RiverPt]) {
    let g = B * GROUP;
    for w in channel.windows(2) {
        let (a, c) = (w[0], w[1]);
        if a.reach != c.reach {
            continue;
        }
        let n = (c.x - a.x).abs().max((c.y - a.y).abs()).max(1);
        // The points along the segment, a cell apart.
        for k in 0..=n {
            let (x, y) = (a.x + (c.x - a.x) * k / n, a.y + (c.y - a.y) * k / n);
            let me = region_at(plan, b, x, y);
            let half = i32::from(a.half);
            let reach = half + 7 + g / 2;
            let (gx0, gx1) = ((x - reach).max(0) / g, ((x + reach) / g).min(BW / GROUP - 1));
            let (gy0, gy1) = ((y - reach).max(0) / g, ((y + reach) / g).min(BW / GROUP - 1));
            for gy in gy0..=gy1 {
                for gx in gx0..=gx1 {
                    let (cx, cy) = (gx * g + g / 2, gy * g + g / 2);
                    let dx = (cx - x).abs();
                    let dy = (cy - y).abs();
                    let d = dx.max(dy);
                    let (bx, by) = (gx * GROUP, gy * GROUP);
                    let i = Blocks::ix(bx, by);
                    // The floor is cut in the river's own region (never over a border) and never in the
                    // Works or over a pad.
                    let reg = plan.districts[usize::from(b.district[i])].reg;
                    // The group the channel's own cells touch: the river's, whatever ground it was.
                    let wet = dx <= half + g / 2 && dy <= half + g / 2;
                    if b.flags[i] & PAD != 0 || reg == Reg::Works || (!wet && reg != me) {
                        continue;
                    }
                    // Within two groups of the Works the floor is held to the channel's own groups, so a
                    // strip of terrace is left at the foot of the Works' faces for the gates.
                    let gw = BW / GROUP;
                    let near_works = (-2..=2).any(|oy| {
                        (-2..=2).any(|ox| {
                            let (nx, ny) = (gx + ox, gy + oy);
                            (0..gw).contains(&nx)
                                && (0..gw).contains(&ny)
                                && plan.districts[usize::from(b.district[Blocks::ix(nx * GROUP, ny * GROUP)])].reg
                                    == Reg::Works
                        })
                    });
                    if (d <= half + 7 && !near_works) || wet {
                        for j in 0..GROUP {
                            for i2 in 0..GROUP {
                                let ix = Blocks::ix(bx + i2, by + j);
                                b.level[ix] = 0;
                                b.flags[ix] |= if wet { VALLEY | RIVER } else { VALLEY };
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The lake, mere and ponds (section 3.4): each body's macro cells, eroded by a shore of 3 to 8 cells
/// with a natural edge, as cells. Only cells on ground of the body's level (and not a face) hold water.
pub fn bodies(plan: &MacroPlan, salt: u8, b: &Blocks, out: &mut WaterCut) {
    let noise = Lattice::new(plan.seed, plan.attempt, Step::MapWater, 40, salt_b(salt, 0), W, 8, 0, 5);
    let body = |mx: i32, my: i32| (0..MW).contains(&mx) && (0..MH).contains(&my) && plan.wet[idx(mx, my)].body();
    let level_of = |w: Wet| -> u8 {
        match w {
            Wet::Mere => 1,
            _ => 0,
        }
    };
    for my in 0..MH {
        for mx in 0..MW {
            let w = plan.wet[idx(mx, my)];
            if !w.body() {
                continue;
            }
            let want = level_of(w);
            let (x0, y0) = (mx * STEP_CELLS, my * STEP_CELLS);
            for y in y0..y0 + STEP_CELLS {
                for x in x0..x0 + STEP_CELLS {
                    // Chebyshev distance to the nearest cell of a macro cell that is no body.
                    let mut d = i32::MAX;
                    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                        if !body(mx + dx, my + dy) {
                            let (sx0, sy0) = ((mx + dx) * STEP_CELLS, (my + dy) * STEP_CELLS);
                            let ex = (sx0 - x).max(0).max(x - (sx0 + STEP_CELLS - 1));
                            let ey = (sy0 - y).max(0).max(y - (sy0 + STEP_CELLS - 1));
                            d = d.min(ex.max(ey));
                        }
                    }
                    if d < 3 + noise.at(x, y) {
                        continue;
                    }
                    // A set place's ground stays dry: its chunk is stamped there.
                    if b.level_of_cell(x, y) == want && b.flags[Blocks::ix(x / B, y / B)] & PAD == 0 {
                        out.bits.set((y * W + x) as usize, true);
                    }
                }
            }
        }
    }
}
