//! Levels on 4-cell blocks (PLAN-map-r4 section 2): the site pads, the warped districts' levels,
//! and the tidy sweeps that make the faces read as authored ground (islands gone, no spikes, steps
//! of two at most, no short south faces, no long straight ones). Everything is integer and a pure
//! function of the plan and the salt.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::{Rect, ZoneId};

use super::noise::{Lattice, salt_b};
use super::{B, BW, CarveFail, W};
use crate::county::centre;
use crate::county::terraced::macro_plan::{MacroPlan, Reg, Role, Wet, idx};
use crate::steps::{Step, dice};

/// A site's pad: every block of its grown box is the site's district and level.
pub const PAD: u8 = 1;
/// The river's floor, and the band along the Lowfields-Waters border: level 0.
pub const VALLEY: u8 = 2;
/// A dyke: a block raised where two regions meet at one level.
pub const LEVEE: u8 = 4;
/// A river crossing's neck: the floor is held to the channel and a margin.
pub const NECK: u8 = 8;
/// A join, a span, a bay or an embankment reserved this block.
pub const FIXED: u8 = 16;
/// A bay or a cutting.
pub const CUT: u8 = 32;
/// The river's channel runs on this block (it keeps its floor).
pub const RIVER: u8 = 64;
/// A made face (the wall above a bay): exempt from the jog rule, but a chain may cut its shelf there.
pub const WALL: u8 = 128;
/// Blocks tidy never changes.
pub const IMMUTABLE: u8 = PAD | FIXED | NECK | RIVER | LEVEE;
/// Blocks whose faces need no jog (made, not grown).
pub const EXEMPT: u8 = PAD | FIXED | NECK | CUT | LEVEE | RIVER | WALL;

/// Blocks to a group's side: levels are laid in groups of 2 x 2 blocks (8 cells), so a diagonal
/// border steps in runs of two blocks, not one (a run of one block is a face of 4 cells).
pub const GROUP: i32 = 2;
/// The most blocks a south face runs straight (MAP.md 4.2: 12 cells).
pub const MAX_STRAIGHT: i32 = 3;
/// The fewest blocks of a south-face run (8 cells; MAP.md asks 6).
pub const MIN_RUN: i32 = 2;
/// Tidy's sweeps.
pub const SWEEPS: usize = 8;
/// A same-level component of fewer than this many blocks is an island (PLAN-map-r4 says 4; a pillar of
/// one or two groups, 8 to 16 cells, standing in a valley floor reads as an error, so 9).
pub const ISLAND: usize = 9;
/// Cells a chunk's box is claimed beyond (county `CHUNK_CLAIM`).
pub const CHUNK_CLAIM: i32 = 6;

const N: usize = (BW * BW) as usize;

/// Level, district and flags of every block, row-major (`by * BW + bx`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocks {
    pub district: Vec<u8>,
    pub level: Vec<u8>,
    pub flags: Vec<u8>,
}

impl Blocks {
    pub fn new() -> Self {
        Self { district: vec![0; N], level: vec![0; N], flags: vec![0; N] }
    }

    #[inline]
    pub const fn ix(bx: i32, by: i32) -> usize {
        (by * BW + bx) as usize
    }

    #[inline]
    pub const fn inside(bx: i32, by: i32) -> bool {
        bx >= 0 && by >= 0 && bx < BW && by < BW
    }

    /// The level of a block, or `None` off the map.
    #[inline]
    pub fn lv(&self, bx: i32, by: i32) -> Option<u8> {
        Self::inside(bx, by).then(|| self.level[Self::ix(bx, by)])
    }

    #[inline]
    pub fn fl(&self, bx: i32, by: i32) -> u8 {
        if Self::inside(bx, by) { self.flags[Self::ix(bx, by)] } else { 0 }
    }

    /// The level of the block holding cell `(x, y)`.
    #[inline]
    pub fn level_of_cell(&self, x: i32, y: i32) -> u8 {
        self.level[Self::ix(x / B, y / B)]
    }

    pub fn set(&mut self, bx: i32, by: i32, level: u8, district: u8) {
        let i = Self::ix(bx, by);
        self.level[i] = level;
        self.district[i] = district;
    }

    pub fn flag(&mut self, bx: i32, by: i32, f: u8) {
        if Self::inside(bx, by) {
            self.flags[Self::ix(bx, by)] |= f;
        }
    }

    pub fn flag_rect(&mut self, r: BRect, f: u8) {
        for by in r.y.max(0)..r.bottom().min(BW) {
            for bx in r.x.max(0)..r.right().min(BW) {
                self.flags[Self::ix(bx, by)] |= f;
            }
        }
    }
}

impl Default for Blocks {
    fn default() -> Self {
        Self::new()
    }
}

/// A rect of blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl BRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub const fn right(self) -> i32 {
        self.x + self.w
    }
    pub const fn bottom(self) -> i32 {
        self.y + self.h
    }
    pub const fn grow(self, n: i32) -> Self {
        Self { x: self.x - n, y: self.y - n, w: self.w + 2 * n, h: self.h + 2 * n }
    }
    pub const fn contains(self, bx: i32, by: i32) -> bool {
        bx >= self.x && by >= self.y && bx < self.x + self.w && by < self.y + self.h
    }
    /// The blocks a cell rect touches, rounded out to groups of [`GROUP`] x [`GROUP`] blocks.
    pub fn of_cells(r: Rect) -> Self {
        let g = B * GROUP;
        let (x0, y0) = (r.x.div_euclid(g) * GROUP, r.y.div_euclid(g) * GROUP);
        let (x1, y1) = ((r.right() + g - 1).div_euclid(g) * GROUP, (r.bottom() + g - 1).div_euclid(g) * GROUP);
        Self { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
    /// The cells of the blocks.
    pub const fn cells(self) -> Rect {
        Rect { x: self.x * B, y: self.y * B, w: self.w * B, h: self.h * B }
    }
}

/// A set place's pad: its chunk box, the box grown by the claim, and the level and district the
/// blocks it covers take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pad {
    pub site: &'static str,
    pub row: u8,
    pub boxr: Rect,
    pub grown: Rect,
    pub level: u8,
    pub district: u8,
}

impl Pad {
    pub fn blocks(&self) -> BRect {
        BRect::of_cells(self.grown)
    }
}

/// The box of `def` for a site origin, as `ChunkDef::origin` puts it but with the pin left out:
/// the R0 plan puts the Halt wherever Station Fields allow, and the pin would stamp its box on the
/// west fence, away from the site. (The stamper needs the same unpinned box: see the report.)
pub fn box_of(def: &jane_data::ChunkDef, ox: i32, oy: i32, margin: i32) -> Rect {
    let clamp =
        |o: i32, a: u16, size: u16, map: i32| (o - i32::from(a)).min(map - i32::from(size) - margin).max(margin);
    Rect::new(clamp(ox, def.anchor.x, def.w, W), clamp(oy, def.anchor.y, def.h, W), i32::from(def.w), i32::from(def.h))
}

pub fn pads(plan: &MacroPlan) -> Vec<Pad> {
    let cat = jane_data::catalog();
    let margin = i32::from(cat.chunks.tuning.box_margin);
    let mut out = Vec::new();
    for s in &plan.sites {
        let Some(row) = cat.county.site_ix(s.id) else { continue };
        let Some(def) = cat.chunks.at_site(row) else { continue };
        let boxr = box_of(def, centre(i32::from(s.pos.0)), centre(i32::from(s.pos.1)), margin);
        let level = plan.districts[usize::from(s.district)].level;
        out.push(Pad { site: s.id, row, boxr, grown: boxr.grow(CHUNK_CLAIM), level, district: s.district });
    }
    out
}

/// The blocks of the district under each block (the warped macro grid), at the district's level;
/// pads and the first walk override.
pub fn lay_blocks(plan: &MacroPlan, salt: u8, pads: &[Pad]) -> Blocks {
    let (seed, at) = (plan.seed, plan.attempt);
    let wx = Lattice::new(seed, at, Step::MapCarve, 1, salt_b(salt, 0), W, 32, -4, 4);
    let wy = Lattice::new(seed, at, Step::MapCarve, 2, salt_b(salt, 0), W, 32, -4, 4);
    let mut b = Blocks::new();
    let g = B * GROUP;
    for gy in 0..BW / GROUP {
        for gx in 0..BW / GROUP {
            let (cx, cy) = (gx * g + g / 2, gy * g + g / 2);
            let (u, v) = ((cx + wx.at(cx, cy)).clamp(0, W - 1), (cy + wy.at(cx, cy)).clamp(0, W - 1));
            let d = plan.district_of[idx(u / 16, v / 16)];
            let l = plan.districts[usize::from(d)].level;
            for by in gy * GROUP..gy * GROUP + GROUP {
                for bx in gx * GROUP..gx * GROUP + GROUP {
                    b.set(bx, by, l, d);
                }
            }
        }
    }
    demote_inland(plan, &mut b);
    for p in pads {
        let r = p.blocks();
        for by in r.y.max(0)..r.bottom().min(BW) {
            for bx in r.x.max(0)..r.right().min(BW) {
                b.set(bx, by, p.level, p.district);
                b.flags[Blocks::ix(bx, by)] |= PAD;
            }
        }
    }
    // The first walk never crosses a level (MAP.md 4.8): the blocks within two of the route from
    // the Halt to Julie's and from Julie's to the Castle are level 1 pad. A pad of another level on the
    // way (the mine's, on the road to the Castle in some plans) is not cut: the corridor goes round it.
    let obstacles: Vec<(i32, i32, i32, i32)> = pads
        .iter()
        .filter(|p| p.level != 1)
        .map(|p| {
            let r = p.blocks().grow(GROUP);
            (r.x, r.y, r.right(), r.bottom())
        })
        .collect();
    for (from, to) in [("station", "julie_house"), ("julie_house", "town")] {
        let Some(route) = plan.routes.iter().find(|r| r.from == from && r.to == to) else { continue };
        // Where the route crosses the river (a wet macro cell, and two cells either side of it, for the
        // river wanders that far) the corridor is left to the valley and the deck: the first walk
        // crosses the river on a level-1 deck, with no level change.
        let wet: Vec<bool> =
            route.pts.iter().map(|q| plan.wet[idx(i32::from(q.0), i32::from(q.1))] == Wet::River).collect();
        let near: Vec<bool> =
            (0..wet.len()).map(|k| (k.saturating_sub(2)..=(k + 2).min(wet.len() - 1)).any(|j| wet[j])).collect();
        let mut legs: Vec<Vec<(i32, i32)>> = vec![Vec::new()];
        for (k, q) in route.pts.iter().enumerate() {
            if near[k] {
                legs.push(Vec::new());
            } else if let Some(leg) = legs.last_mut() {
                leg.push((i32::from(q.0) * 16 / B + 2, i32::from(q.1) * 16 / B + 2));
            }
        }
        for leg in legs.iter().filter(|l| !l.is_empty()) {
            for (x, y) in corridor_line(leg, &obstacles) {
                // Within two blocks of the line, rounded out to whole groups.
                let (gx0, gy0) = ((x - 2).max(0).div_euclid(GROUP), (y - 2).max(0).div_euclid(GROUP));
                let (gx1, gy1) = ((x + 2).min(BW - 1).div_euclid(GROUP), (y + 2).min(BW - 1).div_euclid(GROUP));
                for by in gy0 * GROUP..(gy1 + 1) * GROUP {
                    for bx in gx0 * GROUP..(gx1 + 1) * GROUP {
                        let i = Blocks::ix(bx, by);
                        // A site's own pad keeps its level (and its shape): the corridor is for the open ground.
                        if b.flags[i] & PAD != 0 {
                            continue;
                        }
                        b.level[i] = 1;
                        b.flags[i] |= PAD;
                    }
                }
            }
        }
    }
    valley_band(plan, &mut b);
    b
}

/// The first walk's line in blocks: the points joined a block apart, each stretch that runs through an
/// obstacle (a block rect, half open) taken round it instead, by the shorter way along its edge. A line
/// that begins or ends inside an obstacle is left as it is.
fn corridor_line(pts: &[(i32, i32)], obstacles: &[(i32, i32, i32, i32)]) -> Vec<(i32, i32)> {
    let mut path: Vec<(i32, i32)> = Vec::new();
    for w in pts.windows(2) {
        let n = (w[1].0 - w[0].0).abs().max((w[1].1 - w[0].1).abs()).max(1);
        for k in 0..n {
            path.push((w[0].0 + (w[1].0 - w[0].0) * k / n, w[0].1 + (w[1].1 - w[0].1) * k / n));
        }
    }
    if let Some(&last) = pts.last() {
        path.push(last);
    }
    for &r in obstacles {
        let inside = |p: (i32, i32)| p.0 >= r.0 && p.0 < r.2 && p.1 >= r.1 && p.1 < r.3;
        let mut out: Vec<(i32, i32)> = Vec::with_capacity(path.len());
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
            out.extend(around(r, path[start - 1], path[i]));
        }
        path = out;
    }
    path
}

/// The points that take a line from `e` to `x`, both just outside rect `r` (half open), round it along
/// its edge, clockwise or counter-clockwise, whichever is shorter.
fn around(r: (i32, i32, i32, i32), e: (i32, i32), x: (i32, i32)) -> Vec<(i32, i32)> {
    let (x0, y0, x1, y1) = (r.0, r.1, r.2 - 1, r.3 - 1);
    let (w, h) = (x1 - x0, y1 - y0);
    let per = 2 * (w + h);
    let on = |p: (i32, i32)| -> (i32, (i32, i32)) {
        if p.1 < y0 {
            let q = (p.0.clamp(x0, x1), y0);
            (q.0 - x0, q)
        } else if p.1 > y1 {
            let q = (p.0.clamp(x0, x1), y1);
            (w + h + (x1 - q.0), q)
        } else if p.0 < x0 {
            let q = (x0, p.1.clamp(y0, y1));
            (2 * w + h + (y1 - q.1), q)
        } else {
            let q = (x1, p.1.clamp(y0, y1));
            (w + (q.1 - y0), q)
        }
    };
    let (te, be) = on(e);
    let (tx, bx) = on(x);
    let corners = [(0, (x0, y0)), (w, (x1, y0)), (w + h, (x1, y1)), (2 * w + h, (x0, y1))];
    let cw = (tx - te).rem_euclid(per) <= (te - tx).rem_euclid(per);
    let span = if cw { (tx - te).rem_euclid(per) } else { (te - tx).rem_euclid(per) };
    let mut passed: Vec<(i32, (i32, i32))> = corners
        .iter()
        .filter_map(|&(t, q)| {
            let d = if cw { (t - te).rem_euclid(per) } else { (te - t).rem_euclid(per) };
            (d > 0 && d < span).then_some((d, q))
        })
        .collect();
    passed.sort_by_key(|&(d, _)| d);
    let mut out = vec![be];
    out.extend(passed.into_iter().map(|(_, q)| q));
    out.push(bx);
    // The edge points joined a block apart.
    let mut dense: Vec<(i32, i32)> = Vec::new();
    for pair in out.windows(2) {
        let n = (pair[1].0 - pair[0].0).abs().max((pair[1].1 - pair[0].1).abs()).max(1);
        for k in 0..n {
            dense.push((pair[0].0 + (pair[1].0 - pair[0].0) * k / n, pair[0].1 + (pair[1].1 - pair[0].1) * k / n));
        }
    }
    dense.push(bx);
    dense
}

/// The inland districts (the Top Field, the foothills, the cutting, the slag mere) keep clear of every
/// other region by one macro cell: R0 meant their edges to be a district of the level beside them, and
/// the plans do not all hold to it, so the groups within 16 cells of a foreign region take that level
/// (1 for the Lowfields' rises, 2 for the Works' sunken ground). Pads override this.
fn demote_inland(plan: &MacroPlan, b: &mut Blocks) {
    let gw = BW / GROUP;
    let mut demote: Vec<(i32, i32, u8)> = Vec::new();
    for gy in 0..gw {
        for gx in 0..gw {
            let d = b.district[Blocks::ix(gx * GROUP, gy * GROUP)];
            let to = match plan.districts[usize::from(d)].role {
                Role::TopField | Role::Foothills => 1,
                Role::Cutting | Role::Slagmere => 2,
                _ => continue,
            };
            let me = plan.districts[usize::from(d)].reg;
            let touches = (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let (nx, ny) = (gx + dx, gy + dy);
                    (0..gw).contains(&nx) && (0..gw).contains(&ny) && group_reg(plan, b, nx, ny) != me
                })
            });
            if touches {
                demote.push((gx, gy, to));
            }
        }
    }
    for (gx, gy, to) in demote {
        for j in 0..GROUP {
            for i in 0..GROUP {
                b.level[Blocks::ix(gx * GROUP + i, gy * GROUP + j)] = to;
            }
        }
    }
}

/// The region of the district laid in group `(gx, gy)`.
fn group_reg(plan: &MacroPlan, b: &Blocks, gx: i32, gy: i32) -> Reg {
    plan.districts[usize::from(b.district[Blocks::ix(gx * GROUP, gy * GROUP)])].reg
}

/// Border rule 1 (section 2.3): the Waters' groups within two blocks of the Lowfields go to level 0,
/// a band 8 cells wide along the river valley: the wall MAP.md 4.1 asks of the Lowfields-Waters
/// border, with a face either side of it.
fn valley_band(plan: &MacroPlan, b: &mut Blocks) {
    let gw = BW / GROUP;
    let mut band: Vec<(i32, i32)> = Vec::new();
    for gy in 0..gw {
        for gx in 0..gw {
            if group_reg(plan, b, gx, gy) != Reg::Waters || b.flags[Blocks::ix(gx * GROUP, gy * GROUP)] & PAD != 0 {
                continue;
            }
            let near = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (gx + dx, gy + dy);
                    (0..gw).contains(&nx) && (0..gw).contains(&ny) && group_reg(plan, b, nx, ny) == Reg::Lowfields
                })
            });
            if near {
                band.push((gx, gy));
            }
        }
    }
    for (gx, gy) in band {
        for j in 0..GROUP {
            for i in 0..GROUP {
                let ix = Blocks::ix(gx * GROUP + i, gy * GROUP + j);
                b.level[ix] = 0;
                b.flags[ix] |= VALLEY;
            }
        }
    }
}

/// Border rule 2 (section 2.3): where two groups of different regions touch at one level, the one on
/// the lower-numbered region's side (Lowfields, then Waters, then Works) rises a level, a dyke with a
/// walkable top between two basins.
pub fn levees(plan: &MacroPlan, b: &mut Blocks) {
    let gw = BW / GROUP;
    let mut rise: Vec<(i32, i32)> = Vec::new();
    for gy in 0..gw {
        for gx in 0..gw {
            let (ra, la) = (group_reg(plan, b, gx, gy), b.level[Blocks::ix(gx * GROUP, gy * GROUP)]);
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (gx + dx, gy + dy);
                if nx >= gw || ny >= gw {
                    continue;
                }
                let (rb, lb) = (group_reg(plan, b, nx, ny), b.level[Blocks::ix(nx * GROUP, ny * GROUP)]);
                if ra == rb || la != lb {
                    continue;
                }
                let (lower, upper) = if (ra as u8) < (rb as u8) { ((gx, gy), (nx, ny)) } else { ((nx, ny), (gx, gy)) };
                // The lower-numbered region's group rises, unless it is a pad or the river's own floor
                // (then the other side does).
                let free = |g: (i32, i32)| b.flags[Blocks::ix(g.0 * GROUP, g.1 * GROUP)] & (PAD | RIVER) == 0;
                let pick = if free(lower) {
                    Some(lower)
                } else if free(upper) {
                    Some(upper)
                } else {
                    None
                };
                if let Some(g) = pick {
                    if !rise.contains(&g) {
                        rise.push(g);
                    }
                }
            }
        }
    }
    for (gx, gy) in rise {
        for j in 0..GROUP {
            for i in 0..GROUP {
                let ix = Blocks::ix(gx * GROUP + i, gy * GROUP + j);
                if b.level[ix] < 3 {
                    b.level[ix] += 1;
                }
                b.flags[ix] |= LEVEE;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tidy
// ---------------------------------------------------------------------------------------------

fn immutable(b: &Blocks, bx: i32, by: i32) -> bool {
    b.flags[Blocks::ix(bx, by)] & IMMUTABLE != 0
}

/// Tidy the levels (section 2.4): up to [`SWEEPS`] sweeps of rules 1 to 4, then the jogs of rule
/// 5, then rules 1 to 4 again for what the jogs disturbed. What is still wrong is a `Shape`.
pub fn tidy(b: &mut Blocks, plan: &MacroPlan, salt: u8) -> Result<(), CarveFail> {
    sweeps(b, SWEEPS);
    jogs(b, plan, salt);
    sweeps(b, 2 * SWEEPS);
    match shape_issue(b) {
        Some((rule, at)) => Err(CarveFail::Shape(rule, at)),
        None => Ok(()),
    }
}

/// Up to `n` sweeps of rules 1 to 4, each over the blocks that touch a change of level (the only
/// blocks a rule can change), stopping at the first that changes nothing.
fn sweeps(b: &mut Blocks, n: usize) {
    for _ in 0..n {
        let cand = boundary(b);
        if !(islands(b, &cand) | thin(b, &cand) | step_limit(b, &cand) | short_faces(b, &cand)) {
            break;
        }
    }
}

/// The blocks with a 4-neighbour of another level, ascending: where every tidy rule acts.
pub(super) fn boundary(b: &Blocks) -> Vec<u32> {
    let w = BW as usize;
    let mut mark = vec![false; N];
    for by in 0..w {
        for bx in 0..w {
            let i = by * w + bx;
            let l = b.level[i];
            if bx + 1 < w && b.level[i + 1] != l {
                mark[i] = true;
                mark[i + 1] = true;
            }
            if by + 1 < w && b.level[i + w] != l {
                mark[i] = true;
                mark[i + w] = true;
            }
        }
    }
    mark.iter().enumerate().filter_map(|(i, &m)| m.then_some(i as u32)).collect()
}

fn xy(i: u32) -> (i32, i32) {
    (i as i32 % BW, i as i32 / BW)
}

const D4: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// Rule 1: a 4-connected same-level component under [`ISLAND`] blocks takes the level and district of its
/// most common neighbour (ties to the lowest district id).
fn islands(b: &mut Blocks, cand: &[u32]) -> bool {
    // `done`: in a component of [`ISLAND`] or more, or judged already. `stamp`: reached by this walk.
    let mut done = vec![false; N];
    let mut stamp = vec![0u32; N];
    let mut generation = 0u32;
    let mut changed = false;
    let mut comp: Vec<(i32, i32)> = Vec::new();
    let mut stack: Vec<(i32, i32)> = Vec::new();
    {
        for &ci in cand {
            let (bx, by) = xy(ci);
            let i = Blocks::ix(bx, by);
            if done[i] {
                continue;
            }
            let l = b.level[i];
            generation += 1;
            comp.clear();
            stack.clear();
            stack.push((bx, by));
            stamp[i] = generation;
            let mut mutable = true;
            let mut big = false;
            while let Some((x, y)) = stack.pop() {
                comp.push((x, y));
                mutable &= b.flags[Blocks::ix(x, y)] & IMMUTABLE == 0;
                if comp.len() >= ISLAND {
                    big = true;
                    break;
                }
                for (dx, dy) in D4 {
                    let (nx, ny) = (x + dx, y + dy);
                    if Blocks::inside(nx, ny)
                        && stamp[Blocks::ix(nx, ny)] != generation
                        && b.level[Blocks::ix(nx, ny)] == l
                    {
                        stamp[Blocks::ix(nx, ny)] = generation;
                        stack.push((nx, ny));
                    }
                }
            }
            for &(x, y) in comp.iter().chain(&stack) {
                done[Blocks::ix(x, y)] = true;
            }
            if big || !mutable {
                continue;
            }
            // Votes of the neighbours round the component, by (district, level).
            let mut votes: Vec<(u8, u8, u32)> = Vec::new();
            for &(x, y) in &comp {
                for (dx, dy) in D4 {
                    let (nx, ny) = (x + dx, y + dy);
                    if !Blocks::inside(nx, ny) || comp.contains(&(nx, ny)) {
                        continue;
                    }
                    let j = Blocks::ix(nx, ny);
                    let key = (b.district[j], b.level[j]);
                    match votes.iter_mut().find(|v| (v.0, v.1) == key) {
                        Some(v) => v.2 += 1,
                        None => votes.push((key.0, key.1, 1)),
                    }
                }
            }
            let Some(best) = votes.iter().max_by_key(|v| (v.2, core::cmp::Reverse(v.0))) else { continue };
            let (d, lv) = (best.0, best.1);
            for &(x, y) in &comp {
                b.set(x, y, lv, d);
            }
            changed = true;
        }
    }
    changed
}

/// Rule 2: a block higher than both its N and S neighbours, or both its E and W neighbours, drops
/// to the higher of them.
fn thin(b: &mut Blocks, cand: &[u32]) -> bool {
    let mut changed = false;
    {
        for &ci in cand {
            let (bx, by) = xy(ci);
            if immutable(b, bx, by) {
                continue;
            }
            let l = b.level[Blocks::ix(bx, by)];
            for ((ax, ay), (cx, cy)) in [((0, -1), (0, 1)), ((-1, 0), (1, 0))] {
                let (Some(p), Some(q)) = (b.lv(bx + ax, by + ay), b.lv(bx + cx, by + cy)) else { continue };
                if p < l && q < l {
                    let (drop_to, from) = if p >= q { (p, (bx + ax, by + ay)) } else { (q, (bx + cx, by + cy)) };
                    let d = b.district[Blocks::ix(from.0, from.1)];
                    b.set(bx, by, drop_to, d);
                    changed = true;
                    break;
                }
            }
        }
    }
    changed
}

/// Rule 3: 4-neighbour levels differ by two at most. Where they differ by more the higher block
/// drops one (the Crown keeps its level and its lower neighbour rises instead).
fn step_limit(b: &mut Blocks, cand: &[u32]) -> bool {
    let crown = crate::county::terraced::macro_plan::Role::Crown.id();
    let mut changed = false;
    {
        for &ci in cand {
            let (bx, by) = xy(ci);
            let l = b.level[Blocks::ix(bx, by)];
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (bx + dx, by + dy);
                let Some(m) = b.lv(nx, ny) else { continue };
                if l.abs_diff(m) < 3 {
                    continue;
                }
                let ((hx, hy), (lx, ly)) = if l > m { ((bx, by), (nx, ny)) } else { ((nx, ny), (bx, by)) };
                let hi = b.level[Blocks::ix(hx, hy)];
                let crowned = b.district[Blocks::ix(hx, hy)] == crown;
                if crowned || immutable(b, hx, hy) {
                    if !immutable(b, lx, ly) {
                        let i = Blocks::ix(lx, ly);
                        b.level[i] += 1;
                        changed = true;
                    } else if !immutable(b, hx, hy) {
                        // The low side is held (the river's floor, a pad): the crown's edge recedes.
                        b.level[Blocks::ix(hx, hy)] = hi - 1;
                        changed = true;
                    }
                } else {
                    b.level[Blocks::ix(hx, hy)] = hi - 1;
                    changed = true;
                }
            }
        }
    }
    changed
}

/// The south-face run containing block `(x, y)` in row `y`: the maximal run of blocks higher than
/// the block below by the same drop between the same two levels (a face of one level and a face of two
/// have their cliff rows in different places, so they do not make one straight edge), as
/// `(first, length)`; `None` if `(x, y)` is not in one.
fn face_run(b: &Blocks, x: i32, y: i32) -> Option<(i32, i32)> {
    let drop = |x: i32| match (b.lv(x, y), b.lv(x, y + 1)) {
        (Some(u), Some(l)) if u > l => Some((u, l)),
        _ => None,
    };
    let kind = drop(x)?;
    let (mut x0, mut x1) = (x, x);
    while drop(x0 - 1) == Some(kind) {
        x0 -= 1;
    }
    while drop(x1 + 1) == Some(kind) {
        x1 += 1;
    }
    Some((x0, x1 - x0 + 1))
}

/// Rule 4: a south-face run of one block is mended: the block below is raised to the upper level if
/// that makes no new short run, else the upper block is lowered.
fn short_faces(b: &mut Blocks, cand: &[u32]) -> bool {
    let mut changed = false;
    // The run last judged in a row: later blocks of it are skipped.
    let (mut row, mut until) = (-1, 0);
    for &ci in cand {
        let (bx, by) = xy(ci);
        if by >= BW - 1 {
            break;
        }
        {
            if by == row && bx < until {
                continue;
            }
            let Some((x0, n)) = face_run(b, bx, by) else { continue };
            if n >= MIN_RUN {
                (row, until) = (by, x0 + n);
                continue;
            }
            let up = b.level[Blocks::ix(x0, by)];
            let lo = b.level[Blocks::ix(x0, by + 1)];
            let ud = b.district[Blocks::ix(x0, by)];
            let ld = b.district[Blocks::ix(x0, by + 1)];
            let mut fixed = false;
            // Lowering the upper block must not open a step of three beside it: then it is the block
            // below that rises, run after run down the face if need be (the next rows mend each).
            let lower_ok = (-1..=1).all(|d| {
                [(d, 0), (0, d)]
                    .iter()
                    .all(|&(dx, dy)| (dx, dy) == (0, 0) || b.lv(x0 + dx, by + dy).is_none_or(|m| m.abs_diff(lo) <= 2))
            });
            if !immutable(b, x0, by + 1) {
                // Try raising the block below, and see what run it leaves in row y + 1.
                b.set(x0, by + 1, up, ud);
                let new_run = face_run(b, x0, by + 1).is_some_and(|(_, m)| m < MIN_RUN);
                if new_run && lower_ok {
                    b.set(x0, by + 1, lo, ld);
                } else {
                    fixed = true;
                }
            }
            if !fixed && !immutable(b, x0, by) {
                b.set(x0, by, lo, ld);
                fixed = true;
            }
            changed |= fixed;
            (row, until) = (by, x0 + n);
        }
    }
    changed
}

/// Rule 5: a south-face run longer than [`MAX_STRAIGHT`] blocks is cut into pieces of two or three
/// blocks, every other one pushed a block south (the block below raised to the upper level), so no
/// face runs straight for more than 12 cells and none is shorter than 8. The first piece's kind and
/// each straight piece's length come from the dice.
pub(super) fn jogs(b: &mut Blocks, plan: &MacroPlan, salt: u8) {
    let cand = boundary(b);
    let (mut row, mut until) = (-1, 0);
    for &ci in &cand {
        let (bx, by) = xy(ci);
        if by >= BW - 2 {
            break;
        }
        {
            if by == row && bx < until {
                continue;
            }
            let Some((x0, n)) = face_run(b, bx, by) else { continue };
            (row, until) = (by, x0 + n);
            if n <= MAX_STRAIGHT {
                continue;
            }
            // Only a run of grown ground: a pad or a made block anywhere in it keeps it straight.
            if (x0..x0 + n).any(|x| (b.fl(x, by) | b.fl(x, by + 1)) & EXEMPT != 0) {
                continue;
            }
            let mut r = dice(plan.seed, ZoneId::County, Step::MapCarve, plan.attempt, 3 + by, salt_b(salt, x0));
            let (mut pos, end, mut push) = (x0, x0 + n, r.below(2) == 0);
            while pos < end {
                let rem = end - pos;
                let mut len = if push { 2 } else { 2 + r.below(2) as i32 };
                len = len.min(rem);
                if rem - len == 1 {
                    len = if len >= 3 { len - 1 } else { len + 1 };
                }
                if push {
                    for x in pos..pos + len {
                        let i = Blocks::ix(x, by + 1);
                        // Raise the block below to the upper level, if it is free ground below the face.
                        if b.flags[i] & IMMUTABLE == 0 && b.level[i] < b.level[Blocks::ix(x, by)] {
                            let (l, d) = (b.level[Blocks::ix(x, by)], b.district[Blocks::ix(x, by)]);
                            b.set(x, by + 1, l, d);
                        }
                    }
                }
                pos += len;
                push = !push;
            }
        }
    }
}

/// The first thing wrong with the levels, if any: the shape rules of section 2.4 that tidy must
/// leave true, as `(rule, cell)`. Every rule acts where the level changes, so only those blocks are read.
pub fn shape_issue(b: &Blocks) -> Option<(&'static str, (i32, i32))> {
    let cand = boundary(b);
    shape_issue_cells(b, cand.iter().map(|&i| xy(i)), BW)
}

/// [`shape_issue`] over the blocks `x0..x1` by `y0..y1` only (the made structures check their own
/// neighbourhood before they are kept).
pub fn shape_issue_in(b: &Blocks, x0: i32, y0: i32, x1: i32, y1: i32) -> Option<(&'static str, (i32, i32))> {
    let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(BW), y1.min(BW));
    shape_issue_cells(b, (y0..y1).flat_map(move |by| (x0..x1).map(move |bx| (bx, by))), y1)
}

/// The shape rules over `cells` (ascending in raster order), the face runs only in rows above `y1`.
fn shape_issue_cells(
    b: &Blocks,
    cells: impl Iterator<Item = (i32, i32)> + Clone,
    y1: i32,
) -> Option<(&'static str, (i32, i32))> {
    for (bx, by) in cells.clone() {
        let l = b.level[Blocks::ix(bx, by)];
        if let Some(m) = b.lv(bx + 1, by) {
            if l.abs_diff(m) > 2 {
                return Some(("step", (bx * B, by * B)));
            }
        }
        if let Some(m) = b.lv(bx, by + 1) {
            if l.abs_diff(m) > 2 {
                return Some(("step", (bx * B, by * B)));
            }
        }
        if b.fl(bx, by) & IMMUTABLE != 0 {
            continue;
        }
        for ((ax, ay), (cx, cy)) in [((0, -1), (0, 1)), ((-1, 0), (1, 0))] {
            if let (Some(p), Some(q)) = (b.lv(bx + ax, by + ay), b.lv(bx + cx, by + cy)) {
                if p < l && q < l {
                    return Some(("thin", (bx * B, by * B)));
                }
            }
        }
    }
    let (mut row, mut until) = (-1, 0);
    for (bx, by) in cells {
        if by >= y1.min(BW - 1) {
            break;
        }
        if by == row && bx < until {
            continue;
        }
        let Some((r0, n)) = face_run(b, bx, by) else { continue };
        (row, until) = (by, r0 + n);
        let free = (r0..r0 + n).all(|x| (b.fl(x, by) | b.fl(x, by + 1)) & EXEMPT == 0);
        if n < MIN_RUN && (r0..r0 + n).all(|x| b.fl(x, by) & IMMUTABLE == 0 && b.fl(x, by + 1) & IMMUTABLE == 0) {
            return Some(("short face", (r0 * B, by * B)));
        }
        if n > MAX_STRAIGHT && free {
            return Some(("straight face", (r0 * B, by * B)));
        }
    }
    None
}
