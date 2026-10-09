//! Level regions (MAP R2): the coarse layer under the paths of a zone with levels, so that a
//! search across height first asks the small graph whether the goal can be reached at all, and
//! then heads for the right way up rather than flooding the foot of a cliff.
//!
//! - **A region** is a connected piece (four ways) of the cells at one level that are not a
//!   height barrier ([`barrier`]: a cliff's face or rim, a ledge, a waterfall). Walls, water,
//!   props and bodies are ignored: a region holds everything feet could reach at its level
//!   and more, so whatever the graph says cannot be reached cannot, and every cost it gives is
//!   a lower bound.
//! - **Portals** are the graph's edges, each a small rect of cells on either side: a **join**
//!   (two cells side by side at different levels: a stair's, a ramp's or a ladder's step, both
//!   ways), a **ledge** (its top to its landing, one way, at the A*'s own jump cost) and a
//!   **span** (one end of a whole deck to the other, both ways, at the deck's length).
//! - **Stored** by chunk: each cell's label within its chunk in a [`Plane`] (one value in most
//!   chunks, a bit or two a cell where a face crosses one), the region of each label in a table.
//!   Derived from the level plane, the barrier tiles and the spans' broken bits, which is all it
//!   reads; built with the grid and again when one of those changes (a rare verb gate). Never
//!   saved, never hashed: a pure function of the grid, so a rebuilt runtime holds the same.
//!
//! [`Route`] is one search's use of it (`path.rs`): the cheapest way, by these lower bounds,
//! from each portal to the goal (a Dijkstra over the portals, backwards from the goal), and from
//! that an admissible, consistent heuristic for the A*: within the goal's region the octile
//! distance, elsewhere the cheapest way out of the cell's region toward the goal.

use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::Reverse;

use jane_core::num::octile10;
use jane_core::plane::CHUNK;
use jane_core::{Plane, Rect, Tile};

use crate::grid::ZoneGrid;
use crate::tuning::{LEDGE_FACE_MAX, LEDGE_PATH_EXTRA};

/// Cells in a chunk.
const CELLS: usize = (CHUNK * CHUNK) as usize;

/// Not a region: a barrier cell, outside, or a heuristic with no way to the goal.
pub const NONE: u16 = u16::MAX;
/// A lower bound for a way that does not exist (well under `u32::MAX`, so sums stay finite).
pub const FAR: u32 = u32::MAX / 4;

/// A height barrier: what separates two levels' ground. Its cells are no region's.
#[inline]
pub fn barrier(t: Tile) -> bool {
    matches!(t, Tile::Cliff | Tile::Waterfall) || t.ledge_dir().is_some()
}

/// Which kind of way a portal is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Join,
    Ledge,
    Span,
}

/// A portal: from cells `a` (in region `from`) to cells `b` (in region `to`) for at least `cost`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: u16,
    pub to: u16,
    pub kind: Kind,
    pub a: Rect,
    pub b: Rect,
    pub cost: u32,
}

/// The regions and portals of one grid.
#[derive(Clone, Debug)]
pub struct Regions {
    /// Each cell's label in its chunk: 0 a barrier, else 1 to the chunk's count.
    labels: Plane,
    /// The first node (a chunk's label) of each chunk.
    base: Vec<u32>,
    /// The region of each node.
    node_region: Vec<u16>,
    /// The level of each region.
    level: Vec<u8>,
    /// Portals by `from` (then `to`, kind, cells): `out[first_out[r]..first_out[r + 1]]`.
    edges: Vec<Edge>,
    first_out: Vec<u32>,
    /// Portal indices by `to`: `into[first_in[r]..first_in[r + 1]]`.
    into: Vec<u32>,
    first_in: Vec<u32>,
    /// The most entries a [`Route`]'s heap can hold: each portal once, and once more for each
    /// way it can be improved (a portal into the region it leads out of).
    heap_max: usize,
}

/// Disjoint sets over nodes, smaller index the root (so the numbering is the scan's).
fn find(parent: &mut [u32], mut n: u32) -> u32 {
    while parent[n as usize] != n {
        let p = parent[n as usize];
        parent[n as usize] = parent[p as usize];
        n = p;
    }
    n
}

fn union(parent: &mut [u32], a: u32, b: u32) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb {
        let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
        parent[hi as usize] = lo;
    }
}

/// Octile distance between two rects of cells (0 when they meet).
#[inline]
pub fn rect_gap(a: Rect, b: Rect) -> u32 {
    let gx = (b.x - (a.right() - 1)).max(a.x - (b.right() - 1)).max(0);
    let gy = (b.y - (a.bottom() - 1)).max(a.y - (b.bottom() - 1)).max(0);
    octile10(gx, gy)
}

/// Octile distance from a cell to the nearest cell of a rect.
#[inline]
pub fn cell_gap((x, y): (i32, i32), r: Rect) -> u32 {
    let gx = (r.x - x).max(x - (r.right() - 1)).max(0);
    let gy = (r.y - y).max(y - (r.bottom() - 1)).max(0);
    octile10(gx, gy)
}

impl Regions {
    /// The regions of `g` (which has levels), or `None` if it has more than `u16` can number.
    pub fn build(g: &ZoneGrid) -> Option<Regions> {
        let (w, h) = (g.w(), g.h());
        let cw = w.div_ceil(CHUNK);
        let mut base: Vec<u32> = Vec::with_capacity((cw * h.div_ceil(CHUNK)) as usize);
        let mut nodes = 0u32;
        let mut node_level: Vec<u8> = Vec::new();
        let mut stack: Vec<u16> = Vec::with_capacity(CELLS);
        let labels = Plane::pack_chunks(w, h, |cx, cy, cells: &mut [u8; CELLS]| {
            base.push(nodes);
            cells.fill(0);
            let (x0, y0) = ((cx * CHUNK) as i32, (cy * CHUNK) as i32);
            let n = CHUNK as i32;
            let open = |k: i32| {
                let (x, y) = (x0 + k % n, y0 + k / n);
                g.inside(x, y) && !barrier(g.tile_at(x, y))
            };
            let mut next = 0u8;
            for k in 0..CELLS as i32 {
                if cells[k as usize] != 0 || !open(k) {
                    continue;
                }
                next += 1;
                let lv = g.level_at(x0 + k % n, y0 + k / n);
                node_level.push(lv);
                cells[k as usize] = next;
                stack.clear();
                stack.push(k as u16);
                while let Some(c) = stack.pop() {
                    let c = i32::from(c);
                    let (lx, ly) = (c % n, c / n);
                    for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                        let (nx, ny) = (lx + dx, ly + dy);
                        if nx < 0 || ny < 0 || nx >= n || ny >= n {
                            continue;
                        }
                        let j = ny * n + nx;
                        if cells[j as usize] == 0 && open(j) && g.level_at(x0 + nx, y0 + ny) == lv {
                            cells[j as usize] = next;
                            stack.push(j as u16);
                        }
                    }
                }
            }
            nodes += u32::from(next);
        });
        let node = |x: i32, y: i32| -> Option<u32> {
            let l = labels.get(x as u32, y as u32);
            (l != 0).then(|| base[((y as u32 / CHUNK) * cw + x as u32 / CHUNK) as usize] + u32::from(l) - 1)
        };
        // Join the pieces across chunk borders: same level, side by side.
        let mut parent: Vec<u32> = (0..nodes).collect();
        let s = CHUNK as i32;
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let right = (x + 1) % s == 0 && x + 1 < w as i32;
                let down = (y + 1) % s == 0 && y + 1 < h as i32;
                if !right && !down {
                    continue;
                }
                let Some(a) = node(x, y) else { continue };
                for (bx, by, edge) in [(x + 1, y, right), (x, y + 1, down)] {
                    if edge
                        && let Some(b) = node(bx, by)
                        && node_level[a as usize] == node_level[b as usize]
                    {
                        union(&mut parent, a, b);
                    }
                }
            }
        }
        let mut node_region = alloc::vec![NONE; nodes as usize];
        let mut level: Vec<u8> = Vec::new();
        for n in 0..nodes {
            let r = find(&mut parent, n);
            if node_region[r as usize] == NONE {
                if level.len() >= usize::from(NONE) {
                    return None;
                }
                node_region[r as usize] = level.len() as u16;
                level.push(node_level[r as usize]);
            }
            node_region[n as usize] = node_region[r as usize];
        }
        let mut r = Regions {
            labels,
            base,
            node_region,
            level,
            edges: Vec::new(),
            first_out: Vec::new(),
            into: Vec::new(),
            first_in: Vec::new(),
            heap_max: 0,
        };
        r.portals(g);
        Some(r)
    }

    /// The portals, found afresh (the spans' broken bits changed, or the regions are new).
    pub fn portals(&mut self, g: &ZoneGrid) {
        let mut raw: Vec<Edge> = Vec::new();
        let one = |x: i32, y: i32| Rect::new(x, y, 1, 1);
        for y in 0..g.h() as i32 {
            for x in 0..g.w() as i32 {
                if !g.level_changes_near(x, y) {
                    continue;
                }
                let t = g.tile_at(x, y);
                // A ledge's top face cell: one way, from the cell behind it to the landing.
                if let Some((dx, dy)) = t.ledge_dir() {
                    let (tx, ty) = (x - dx, y - dy);
                    if g.tile_at(tx, ty) == t {
                        continue;
                    }
                    let (mut cx, mut cy, mut faces) = (x, y, 0);
                    while g.inside(cx, cy) && g.tile_at(cx, cy) == t && faces < LEDGE_FACE_MAX {
                        cx += dx;
                        cy += dy;
                        faces += 1;
                    }
                    if let (Some(from), Some(to)) = (self.region(tx, ty), self.region(cx, cy)) {
                        raw.push(Edge {
                            from,
                            to,
                            kind: Kind::Ledge,
                            a: one(tx, ty),
                            b: one(cx, cy),
                            cost: (faces as u32 + 1) * 10 + LEDGE_PATH_EXTRA,
                        });
                    }
                    continue;
                }
                // A join: two ground cells side by side at different levels, both ways; and two
                // corner to corner in different regions with ground at both other corners (a
                // diagonal step a search may take across), so `b` holds every cell a crossing
                // enters.
                let Some(ra) = self.region(x, y) else { continue };
                for (bx, by) in [(x + 1, y), (x, y + 1), (x + 1, y + 1), (x - 1, y + 1)] {
                    let Some(rb) = self.region(bx, by) else { continue };
                    let cross = if bx != x && by != y {
                        rb != ra && self.region(bx, y).is_some() && self.region(x, by).is_some()
                    } else {
                        self.level[usize::from(ra)] != self.level[usize::from(rb)]
                    };
                    if cross {
                        raw.push(Edge { from: ra, to: rb, kind: Kind::Join, a: one(x, y), b: one(bx, by), cost: 0 });
                        raw.push(Edge { from: rb, to: ra, kind: Kind::Join, a: one(bx, by), b: one(x, y), cost: 0 });
                    }
                }
            }
        }
        // A whole span: each lane's end to its other end, both ways.
        for (i, s) in g.spans().iter().enumerate() {
            if !g.span_whole(i as u8) {
                continue;
            }
            let [e0, e1] = s.ends();
            for (a, b) in e0.cells().zip(e1.cells()) {
                if let (Some(ra), Some(rb)) = (self.region(a.0, a.1), self.region(b.0, b.1)) {
                    let cost = (s.length() as u32 + 1) * 10;
                    raw.push(Edge { from: ra, to: rb, kind: Kind::Span, a: one(a.0, a.1), b: one(b.0, b.1), cost });
                    raw.push(Edge { from: rb, to: ra, kind: Kind::Span, a: one(b.0, b.1), b: one(a.0, a.1), cost });
                }
            }
        }
        // One portal for each run of cells side by side: a stair four wide is one, not four.
        raw.sort_by_key(|e| (e.from, e.to, e.kind, e.a.y, e.a.x, e.b.y, e.b.x));
        let grow = |r: Rect, o: Rect| {
            let (x, y) = (r.x.min(o.x), r.y.min(o.y));
            Rect::new(x, y, r.right().max(o.right()) - x, r.bottom().max(o.bottom()) - y)
        };
        self.edges.clear();
        for e in raw {
            if let Some(last) = self.edges.last_mut()
                && (last.from, last.to, last.kind, last.cost) == (e.from, e.to, e.kind, e.cost)
                && last.a.grow(1).overlaps(e.a)
                && last.b.grow(1).overlaps(e.b)
            {
                last.a = grow(last.a, e.a);
                last.b = grow(last.b, e.b);
                continue;
            }
            self.edges.push(e);
        }
        let n = self.level.len();
        self.first_out.clear();
        self.first_out.resize(n + 1, 0);
        self.first_in.clear();
        self.first_in.resize(n + 1, 0);
        for e in &self.edges {
            self.first_out[usize::from(e.from) + 1] += 1;
            self.first_in[usize::from(e.to) + 1] += 1;
        }
        for r in 0..n {
            self.first_out[r + 1] += self.first_out[r];
            self.first_in[r + 1] += self.first_in[r];
        }
        self.heap_max = self.edges.len()
            + self
                .edges
                .iter()
                .map(|e| (self.first_in[usize::from(e.from) + 1] - self.first_in[usize::from(e.from)]) as usize)
                .sum::<usize>();
        let mut fill = self.first_in.clone();
        self.into.clear();
        self.into.resize(self.edges.len(), 0);
        for (i, e) in self.edges.iter().enumerate() {
            let at = &mut fill[usize::from(e.to)];
            self.into[*at as usize] = i as u32;
            *at += 1;
        }
    }

    /// How many regions.
    pub fn count(&self) -> usize {
        self.level.len()
    }

    /// The portals.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The level of region `r`.
    pub fn level_of(&self, r: u16) -> u8 {
        self.level[usize::from(r)]
    }

    /// The region of in-grid ground cell `(x, y)`, or `None` for a barrier or outside.
    #[inline]
    pub fn region(&self, x: i32, y: i32) -> Option<u16> {
        if !self.labels.inside(x, y) {
            return None;
        }
        let l = self.labels.get(x as u32, y as u32);
        if l == 0 {
            return None;
        }
        let cw = self.labels.w().div_ceil(CHUNK);
        let b = self.base[((y as u32 / CHUNK) * cw + x as u32 / CHUNK) as usize];
        Some(self.node_region[(b + u32::from(l) - 1) as usize])
    }

    /// The region feet at `(x, y)` stand in: a cell's own, or for a cliff's north lip (where feet
    /// come a little way into the rock, `grid::CLIFF_LIP_OPEN`) the ground north of it, its only
    /// way out.
    #[inline]
    pub fn region_of_feet(&self, g: &ZoneGrid, x: i32, y: i32) -> Option<u16> {
        self.region(x, y).or_else(|| if g.tile_at(x, y) == Tile::Cliff { self.region(x, y - 1) } else { None })
    }

    fn out_of(&self, r: u16) -> &[Edge] {
        &self.edges[self.first_out[usize::from(r)] as usize..self.first_out[usize::from(r) + 1] as usize]
    }

    fn entering(&self, r: u16) -> &[u32] {
        &self.into[self.first_in[usize::from(r)] as usize..self.first_in[usize::from(r) + 1] as usize]
    }
}

/// One search's view of the regions: the goal, which portals it may take, and the lower bound of
/// each portal's way to the goal. Its buffers are kept and reused (`PathScratch`).
#[derive(Debug, Default)]
pub struct Route {
    start: (i32, i32),
    goal: (i32, i32),
    goal_region: u16,
    /// Per portal: a lower bound from its far cells to the goal ([`FAR`]: none, or it cannot be
    /// taken now).
    after: Vec<u32>,
    /// Per portal: 0 not yet asked, 1 open now, 2 shut now ([`Route::open`]).
    open: Vec<u8>,
    heap: BinaryHeap<Reverse<(u32, u32)>>,
}

impl Route {
    /// Ready for a search over `g` toward `goal` (a cell whose region is `goal_region`), hopping
    /// ledges whose landing lies within `ledges`' reach of its home (as the search's own jumps
    /// do), or none.
    pub fn prepare(
        &mut self,
        r: &Regions,
        g: &ZoneGrid,
        (start, goal): ((i32, i32), (i32, i32)),
        goal_region: u16,
        ledges: Option<((i32, i32), i32)>,
    ) {
        self.start = start;
        self.goal = goal;
        self.goal_region = goal_region;
        self.after.clear();
        self.after.resize(r.edges.len(), FAR);
        self.open.clear();
        self.open.resize(r.edges.len(), 0);
        self.heap.clear();
        // Never grown mid-search: as much as any search over these portals can push.
        self.heap.reserve(r.heap_max);
        for &i in r.entering(goal_region) {
            if self.open(r, g, i, ledges) {
                let d = cell_gap(goal, r.edges[i as usize].b);
                self.after[i as usize] = d;
                self.heap.push(Reverse((d, i)));
            }
        }
        while let Some(Reverse((d, i))) = self.heap.pop() {
            if d > self.after[i as usize] {
                continue;
            }
            let f = r.edges[i as usize];
            // Every portal into `f.from`: from its far cells across that region to `f`'s near ones.
            for &j in r.entering(f.from) {
                let e = r.edges[j as usize];
                let via = rect_gap(e.b, f.a).saturating_add(f.cost).saturating_add(d);
                if via < self.after[j as usize] && self.open(r, g, j, ledges) {
                    self.after[j as usize] = via;
                    self.heap.push(Reverse((via, j)));
                }
            }
        }
    }

    /// Can portal `i` be taken now, as the search's own steps would take it? Only if some cell on
    /// its near side is open and unheld (or is the start) and some cell it enters is (or is the
    /// goal: the search enters its goal whoever stands there); a ledge only by a walker that
    /// hops, landing within its reach of home. Asked once a search.
    fn open(&mut self, r: &Regions, g: &ZoneGrid, i: u32, ledges: Option<((i32, i32), i32)>) -> bool {
        let known = self.open[i as usize];
        if known != 0 {
            return known == 1;
        }
        let e = &r.edges[i as usize];
        let free = |b: Rect, or: (i32, i32)| {
            b.contains(or.0, or.1)
                || b.cells()
                    .any(|(x, y)| g.flags_at(x, y) & (jane_core::tile::BLOCK_MOVE | jane_core::tile::F_OCC) == 0)
        };
        let ok = match e.kind {
            Kind::Ledge => ledges.is_some_and(|((hx, hy), reach)| {
                let (nx, ny) = (hx.clamp(e.b.x, e.b.right() - 1), hy.clamp(e.b.y, e.b.bottom() - 1));
                let (dx, dy) = (i64::from(nx - hx), i64::from(ny - hy));
                dx * dx + dy * dy <= i64::from(reach) * i64::from(reach)
                    && free(e.a, self.start)
                    && free(e.b, self.goal)
            }),
            Kind::Join => free(e.a, self.start) && free(e.b, self.goal),
            Kind::Span => true,
        };
        self.open[i as usize] = if ok { 1 } else { 2 };
        ok
    }

    /// The portal out of region `from` that a walker at `c` should take toward the goal: the one
    /// with the least lower bound through it, the first such in the portals' order. `None` if
    /// none leads there.
    pub fn best_exit(&self, r: &Regions, from: u16, c: (i32, i32)) -> Option<Edge> {
        let first = r.first_out[usize::from(from)] as usize;
        let mut best: Option<(u32, Edge)> = None;
        for (k, e) in r.out_of(from).iter().enumerate() {
            let after = self.after[first + k];
            if after >= FAR {
                continue;
            }
            let via = cell_gap(c, e.a).saturating_add(e.cost).saturating_add(after);
            if best.is_none_or(|(b, _)| via < b) {
                best = Some((via, *e));
            }
        }
        best.map(|(_, e)| e)
    }

    /// A lower bound of the cost from a cell of region `from` at `c` to the goal: [`FAR`] if there
    /// is no way.
    #[inline]
    pub fn bound(&self, r: &Regions, from: u16, c: (i32, i32)) -> u32 {
        let straight = octile10(self.goal.0 - c.0, self.goal.1 - c.1);
        if from == self.goal_region {
            return straight;
        }
        let first = r.first_out[usize::from(from)] as usize;
        let mut best = FAR;
        for (k, e) in r.out_of(from).iter().enumerate() {
            let after = self.after[first + k];
            if after >= FAR {
                continue;
            }
            best = best.min(cell_gap(c, e.a).saturating_add(e.cost).saturating_add(after));
        }
        best.max(straight)
    }
}
