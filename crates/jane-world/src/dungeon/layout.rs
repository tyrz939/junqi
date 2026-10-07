//! From graph to bays (DUNGEONS.md §2.5, steps 1 to 3; `layout.ts`). Nothing here touches a
//! cell: a layout is which template fills which node, in which bays, turned how, and which lane
//! each corridor runs along. Cells come later, in `generate.rs`.
//!
//! The lattice is a grid of bays. Corridors run on the LINES between bays, three cells wide,
//! and a room's rim keeps its distance from them, so a corridor can never break into a room.
//! The lines make a small graph:
//!
//! ```text
//!        corner ---- mid ---- corner        a `mid` is the middle of one bay side: the port
//!          |                    |           of the door on that side (of either bay)
//!         mid       bay        mid          a `corner` is where lines cross
//!          |                    |
//!        corner ---- mid ---- corner
//! ```
//!
//! Two doors that face each other across a line share a port: a straight stub. Anything else
//! (a cycle closer, a shortcut) is routed mid to corner to corner to mid. A lane node belongs
//! to one corridor only, so corridors never merge or cross, which is what keeps a lock a lock.
//!
//! The template choice draws from `Step::DunChoose`, the embedding's orders and shape picks from
//! `Step::DunEmbed`, each per embedding try (`a` = the try). Lists are in a fixed order before
//! any pick, and every sort is total.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use jane_core::{Sfc32, TemplateId, ZoneId};
use jane_data::{
    BAY_H, BAY_W, BORDER, MOUTH_X, MOUTH_Y, MissionDef, MissionEdgeKind, MissionNodeKind, MissionPlacement, RoomDoor,
    RoomShape, RoomSide, catalog,
};

use crate::steps::{Step, dice};

/// Whole embeddings tried per attempt before the attempt is given up. They cost no cells.
pub const EMBED_TRIES: u8 = 4;
/// Rooms put down, in all, by one embedding before it is given up.
const EMBED_BUDGET: i32 = 160;
/// How many places are tried for one room: the best three in a seeded order, then the rest by score.
const EMBED_WIDTH: usize = 6;

/// One end of a corridor: a door of a placed room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DoorUse {
    /// Node index.
    pub node: usize,
    /// Index into the placed shape's `doors`.
    pub door: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corridor {
    /// Index into the mission's `edges`.
    pub edge: usize,
    /// The edge's `from` end.
    pub a: DoorUse,
    /// The edge's `to` end.
    pub b: DoorUse,
    /// Lane graph nodes from a's port to b's port. One entry when the two doors face each other.
    pub lane: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// In the order they were put down.
    pub placements: Vec<MissionPlacement>,
    /// Sorted by edge.
    pub corridors: Vec<Corridor>,
    /// Side nodes that did not fit this seed (node indices).
    pub dropped: Vec<u8>,
    /// The mission's hand-placed embedding.
    pub fallback: bool,
}

/// The lines between bays, as a graph of lane nodes.
#[derive(Clone, Debug)]
pub struct Lanes {
    pub cols: i32,
    pub rows: i32,
    pub count: usize,
    /// A corridor (or the inside of a big room) runs here.
    pub used: Vec<bool>,
    corners: usize,
    hmids: usize,
    /// Node `n`'s neighbours are `link_to[link_at[n]..link_at[n + 1]]`, in the order linked.
    link_at: Vec<u32>,
    link_to: Vec<u16>,
}

/// `Flood::dist` of a lane in use: never reached.
const IN_USE: i16 = -2;

/// A breadth-first flood over free lane nodes: `dist[n]` is negative and `prev[n]` -1 where it
/// never got (`dist` is [`IN_USE`] on a lane in use).
#[derive(Clone, Debug, Default)]
pub struct Flood {
    pub dist: Vec<i16>,
    pub prev: Vec<i16>,
}

impl Lanes {
    pub fn new(cols: i32, rows: i32) -> Self {
        let corners = ((cols + 1) * (rows + 1)) as usize;
        let hmids = (cols * (rows + 1)) as usize;
        let count = corners + hmids + ((cols + 1) * rows) as usize;
        let mut l = Self {
            cols,
            rows,
            count,
            used: vec![false; count],
            corners,
            hmids,
            link_at: Vec::new(),
            link_to: Vec::new(),
        };
        let link = |links: &mut Vec<Vec<u16>>, a: usize, b: usize| {
            links[a].push(b as u16);
            links[b].push(a as u16);
        };
        let mut links = vec![Vec::new(); count];
        for j in 0..=rows {
            for i in 0..cols {
                link(&mut links, l.hmid(i, j), l.corner(i, j));
                link(&mut links, l.hmid(i, j), l.corner(i + 1, j));
            }
        }
        for j in 0..rows {
            for i in 0..=cols {
                link(&mut links, l.vmid(i, j), l.corner(i, j));
                link(&mut links, l.vmid(i, j), l.corner(i, j + 1));
            }
        }
        l.link_at.push(0);
        for n in &links {
            l.link_to.extend_from_slice(n);
            l.link_at.push(l.link_to.len() as u32);
        }
        l
    }

    /// The lane nodes `n` joins, in the order they were linked.
    pub fn links(&self, n: usize) -> &[u16] {
        &self.link_to[self.link_at[n] as usize..self.link_at[n + 1] as usize]
    }

    pub fn corner(&self, i: i32, j: i32) -> usize {
        (j * (self.cols + 1) + i) as usize
    }

    /// Middle of the horizontal line above bay row j, over bay column i.
    pub fn hmid(&self, i: i32, j: i32) -> usize {
        self.corners + (j * self.cols + i) as usize
    }

    /// Middle of the vertical line left of bay column i, beside bay row j.
    pub fn vmid(&self, i: i32, j: i32) -> usize {
        self.corners + self.hmids + (j * (self.cols + 1) + i) as usize
    }

    /// Centre cell of a lane node, in zone cells.
    pub fn cell(&self, n: usize) -> (i32, i32) {
        let c1 = self.cols + 1;
        if n < self.corners {
            let n = n as i32;
            return (BORDER + (n % c1) * BAY_W, BORDER + (n / c1) * BAY_H);
        }
        if n < self.corners + self.hmids {
            let m = (n - self.corners) as i32;
            return (BORDER + (m % self.cols) * BAY_W + MOUTH_X, BORDER + (m / self.cols) * BAY_H);
        }
        let m = (n - self.corners - self.hmids) as i32;
        (BORDER + (m % c1) * BAY_W, BORDER + (m / c1) * BAY_H + MOUTH_Y)
    }

    /// The port a door opens onto, for a room whose first bay is (bx, by).
    pub fn port(&self, bx: i32, by: i32, bays: (u8, u8), d: &RoomDoor) -> usize {
        let bay = i32::from(d.bay);
        match d.side {
            RoomSide::N => self.hmid(bx + bay, by),
            RoomSide::S => self.hmid(bx + bay, by + i32::from(bays.1)),
            RoomSide::W => self.vmid(bx, by + bay),
            RoomSide::E => self.vmid(bx + i32::from(bays.0), by + bay),
        }
    }

    /// The lane nodes inside a room of more than one bay: lines that run through it carry nothing.
    fn inside(&self, bx: i32, by: i32, bays: (u8, u8), mut f: impl FnMut(usize) -> bool) -> bool {
        let (w, h) = (i32::from(bays.0), i32::from(bays.1));
        for j in by..by + h {
            for i in bx + 1..bx + w {
                if f(self.vmid(i, j)) {
                    return true;
                }
            }
        }
        for j in by + 1..by + h {
            for i in bx..bx + w {
                if f(self.hmid(i, j)) {
                    return true;
                }
            }
        }
        for j in by + 1..by + h {
            for i in bx + 1..bx + w {
                if f(self.corner(i, j)) {
                    return true;
                }
            }
        }
        false
    }

    fn block_inside(&mut self, bx: i32, by: i32, bays: (u8, u8), value: bool) {
        let mut cells = Vec::new();
        self.inside(bx, by, bays, |n| {
            cells.push(n);
            false
        });
        for n in cells {
            self.used[n] = value;
        }
    }

    /// A corridor already runs where the inside of this room would be.
    fn inside_taken(&self, bx: i32, by: i32, bays: (u8, u8)) -> bool {
        self.inside(bx, by, bays, |n| self.used[n])
    }

    /// Breadth-first from a port over free lane nodes.
    pub fn flood(&self, from: usize) -> Flood {
        let mut f = Flood { dist: Vec::with_capacity(self.count), prev: Vec::with_capacity(self.count) };
        self.flood_into(from, None, true, &mut f, &mut Vec::with_capacity(self.count));
        f
    }

    /// [`Self::flood`] into `f`, with `queue` for scratch: the search asks thousands of times.
    /// With `until`, it stops once every node named there has been reached: a breadth-first
    /// flood never changes a node's `dist` or `prev` once it has reached it, so those nodes, and
    /// every node on the way back from them, read as the whole flood would leave them. Without
    /// `ways`, only `dist` is kept (`prev` is left as it was).
    pub fn flood_into(&self, from: usize, until: Option<&[usize]>, ways: bool, f: &mut Flood, queue: &mut Vec<u16>) {
        self.flood_from(&[from], until, ways, f, queue);
    }

    /// [`Self::flood_into`] from several nodes at once: `dist` is the steps to the nearest of them
    /// (a lane in use starts nothing).
    fn flood_from(&self, from: &[usize], until: Option<&[usize]>, ways: bool, f: &mut Flood, queue: &mut Vec<u16>) {
        // A lane in use starts out "reached", so a step asks one thing of where it goes.
        f.dist.clear();
        f.dist.extend(self.used.iter().map(|&u| if u { IN_USE } else { -1 }));
        if ways {
            f.prev.clear();
            f.prev.resize(self.count, -1);
        }
        queue.clear();
        for &n in from {
            if f.dist[n] == -1 {
                queue.push(n as u16);
                f.dist[n] = 0;
            }
        }
        let mut left = until.map_or(usize::MAX, |t| t.iter().filter(|&&n| f.dist[n] == -1).count());
        let mut q = 0;
        while q < queue.len() && left > 0 {
            let n = usize::from(queue[q]);
            q += 1;
            let d = f.dist[n] + 1;
            for &m in self.links(n) {
                let m = usize::from(m);
                if f.dist[m] != -1 {
                    continue;
                }
                f.dist[m] = d;
                if ways {
                    f.prev[m] = n as i16;
                }
                queue.push(m as u16);
                if let Some(t) = until {
                    left -= t.iter().filter(|&&x| x == m).count();
                }
            }
        }
    }
}

/// The mission's graph, worked out once per embedding, because the search asks thousands of times.
#[derive(Debug)]
struct Graph {
    m: &'static MissionDef,
    /// Node -> the edges that need a corridor from it (a `sight` edge rides on another edge's).
    edges_of: Vec<Vec<usize>>,
    /// Node -> the critical nodes its edges lead to, once per edge.
    critical_edges: Vec<Vec<usize>>,
}

impl Graph {
    fn new(m: &'static MissionDef) -> Self {
        let n = m.nodes.len();
        let mut edges_of = vec![Vec::new(); n];
        let mut critical_edges = vec![Vec::new(); n];
        for (i, e) in m.edges.iter().enumerate() {
            if e.kind == MissionEdgeKind::Sight {
                continue;
            }
            let (from, to) = (usize::from(e.from), usize::from(e.to));
            edges_of[from].push(i);
            edges_of[to].push(i);
            if m.nodes[to].critical {
                critical_edges[from].push(to);
            }
            if m.nodes[from].critical {
                critical_edges[to].push(from);
            }
        }
        Self { m, edges_of, critical_edges }
    }
}

#[derive(Clone, Debug)]
struct Placed {
    node: usize,
    template: TemplateId,
    shape: &'static RoomShape,
    bx: i32,
    by: i32,
    /// Doors in use, a bit per index into `shape.doors`.
    used: u32,
}

impl Placed {
    fn uses(&self, door: usize) -> bool {
        self.used & (1 << door) != 0
    }
}

/// Floods from a port, by port, for as long as the lanes do not change.
type FloodCache = Vec<Option<Flood>>;

#[derive(Debug)]
struct State {
    lanes: Lanes,
    /// Bay -> index into `placed`, or -1.
    owner: Vec<i8>,
    placed: Vec<Placed>,
    corridors: Vec<Corridor>,
    /// Node -> where it stands (index into `placed`), while it stands.
    at: Vec<Option<usize>>,
    /// Scratch for the floods `connect` does not cache.
    scratch: Flood,
    queue: Vec<u16>,
}

impl State {
    fn new(m: &MissionDef) -> Self {
        let (cols, rows) = (i32::from(m.cols), i32::from(m.rows));
        Self {
            lanes: Lanes::new(cols, rows),
            owner: vec![-1; (cols * rows) as usize],
            placed: Vec::new(),
            corridors: Vec::new(),
            at: vec![None; m.nodes.len()],
            scratch: Flood::default(),
            queue: Vec::new(),
        }
    }

    fn port_of(&self, p: &Placed, door: usize) -> usize {
        self.lanes.port(p.bx, p.by, p.shape.bays, &p.shape.doors[door])
    }

    fn put(&mut self, p: Placed) {
        let index = self.placed.len();
        let cols = self.lanes.cols;
        for j in p.by..p.by + i32::from(p.shape.bays.1) {
            for i in p.bx..p.bx + i32::from(p.shape.bays.0) {
                self.owner[(j * cols + i) as usize] = index as i8;
            }
        }
        self.lanes.block_inside(p.bx, p.by, p.shape.bays, true);
        self.at[p.node] = Some(index);
        self.placed.push(p);
    }

    fn take(&mut self) {
        let p = self.placed.pop().expect("take after put");
        self.at[p.node] = None;
        let cols = self.lanes.cols;
        for j in p.by..p.by + i32::from(p.shape.bays.1) {
            for i in p.bx..p.bx + i32::from(p.shape.bays.0) {
                self.owner[(j * cols + i) as usize] = -1;
            }
        }
        self.lanes.block_inside(p.bx, p.by, p.shape.bays, false);
    }

    fn bay_free(&self, bx: i32, by: i32, bays: (u8, u8)) -> bool {
        let (w, h) = (i32::from(bays.0), i32::from(bays.1));
        if bx < 0 || by < 0 || bx + w > self.lanes.cols || by + h > self.lanes.rows {
            return false;
        }
        for j in by..by + h {
            for i in bx..bx + w {
                if self.owner[(j * self.lanes.cols + i) as usize] >= 0 {
                    return false;
                }
            }
        }
        // A room of more than one bay swallows the lines between its bays, so nothing may be running on them.
        !self.lanes.inside_taken(bx, by, bays)
    }

    /// Edges of a placed node that lead to a critical node not placed yet.
    fn pending_of(&self, g: &Graph, node: usize) -> usize {
        g.critical_edges[node].iter().filter(|&&o| self.at[o].is_none()).count()
    }

    /// A placed room that still owes doors to rooms not yet placed must have them, on free ports.
    fn starved(&self, g: &Graph) -> bool {
        self.placed.iter().any(|p| {
            let owes = self.pending_of(g, p.node);
            owes > 0
                && (0..p.shape.doors.len()).filter(|&d| !p.uses(d) && !self.lanes.used[self.port_of(p, d)]).count()
                    < owes
        })
    }

    /// Room for what is still to come: for every placed room that owes doors to rooms not yet
    /// placed, how many of its free doors look straight into a free bay. A door that does not
    /// can still be used, by a corridor that goes round; those are what fill the lanes up.
    fn room_to_grow(&self, g: &Graph) -> i32 {
        let mut score = 0;
        for p in &self.placed {
            let owes = self.pending_of(g, p.node) as i32;
            if owes == 0 {
                continue;
            }
            let mut straight = 0;
            for (i, d) in p.shape.doors.iter().enumerate() {
                if p.uses(i) || self.lanes.used[self.port_of(p, i)] {
                    continue;
                }
                let (fx, fy) = facing_bay(p, d);
                if self.bay_free(fx, fy, (1, 1)) {
                    straight += 1;
                }
            }
            score += 2 * straight.min(owes) - 4 * (owes - straight).max(0);
        }
        score
    }

    /// Join the last placed room to every placed neighbour. For each edge, the pair of free doors
    /// with the shortest lane between them; a tie goes to the earlier door. Returns the corridors
    /// made (already marked on the lanes), or `None` if some edge could not be joined, in which
    /// case the lanes are left as they were.
    fn connect(&mut self, g: &Graph, mut floods: Option<&mut FloodCache>) -> Option<Vec<Corridor>> {
        let me = self.placed.len() - 1;
        let me_node = self.placed[me].node;
        let mut made: Vec<Corridor> = Vec::new();
        let (mut scratch, mut queue) = (core::mem::take(&mut self.scratch), core::mem::take(&mut self.queue));
        let mut targets: Vec<(usize, usize)> = Vec::new();
        for &ei in &g.edges_of[me_node] {
            let e = &g.m.edges[ei];
            let other_node = if usize::from(e.from) == me_node { usize::from(e.to) } else { usize::from(e.from) };
            let Some(oi) = self.at[other_node] else { continue };
            if oi == me {
                continue;
            }
            // Their free doors, as (door, port).
            let other = &self.placed[oi];
            targets.clear();
            targets
                .extend((0..other.shape.doors.len()).filter(|&t| !other.uses(t)).map(|t| (t, self.port_of(other, t))));
            let ports: Vec<usize> = targets.iter().map(|&(_, to)| to).collect();
            // The first corridor of a one-bay room is routed over lanes nobody has touched since
            // the cache was started, so a flood from the same port is the same flood; and a lane
            // is as long from either end, so the cache floods from their doors, which stay put
            // while this room is tried everywhere, instead of from each of its own.
            let cached = made.is_empty() && self.placed[me].shape.bays == (1, 1);
            // My free doors, as (door, port).
            let mine_ports: Vec<(usize, usize)> = (0..self.placed[me].shape.doors.len())
                .filter(|&d| !self.placed[me].uses(d))
                .map(|d| (d, self.port_of(&self.placed[me], d)))
                .collect();
            // The pair of doors with the shortest lane, the first such in (mine, theirs) order,
            // and the lane, as the flood from my door gives it: back along it from theirs.
            let lane_of = |lanes: &Lanes, scratch: &mut Flood, queue: &mut Vec<u16>, from: usize, to: usize, d: i16| {
                lanes.flood_into(from, Some(&[to]), true, scratch, queue);
                let mut lane = Vec::with_capacity(d as usize + 1);
                let mut n = to as i16;
                while n >= 0 {
                    lane.push(n as u16);
                    n = scratch.prev[n as usize];
                }
                lane.reverse();
                lane
            };
            let best = match floods.as_deref_mut() {
                Some(cache) if cached => {
                    // (my door, their door, lane length - 1, my port, their port).
                    let mut best: Option<(usize, usize, i16, usize, usize)> = None;
                    for &(mine, from) in &mine_ports {
                        for &(theirs, to) in &targets {
                            let d = cache[to]
                                .get_or_insert_with(|| {
                                    let mut f = Flood::default();
                                    self.lanes.flood_into(to, None, false, &mut f, &mut queue);
                                    f
                                })
                                .dist[from];
                            if d < 0 || best.is_some_and(|b| b.2 <= d) {
                                continue;
                            }
                            best = Some((mine, theirs, d, from, to));
                        }
                    }
                    best.map(|(mine, theirs, d, from, to)| {
                        (mine, theirs, lane_of(&self.lanes, &mut scratch, &mut queue, from, to, d))
                    })
                }
                _ => {
                    // One flood from all their doors at once says how near the nearest is to each
                    // of mine: the first of mine that is nearest is the pair's. Then a flood from
                    // it says which of theirs is that near first.
                    let mine_only: Vec<usize> = mine_ports.iter().map(|&(_, p)| p).collect();
                    self.lanes.flood_from(&ports, Some(&mine_only), false, &mut scratch, &mut queue);
                    let mut pick: Option<(usize, usize, i16)> = None;
                    for &(mine, from) in &mine_ports {
                        let d = scratch.dist[from];
                        if d >= 0 && pick.is_none_or(|p| d < p.2) {
                            pick = Some((mine, from, d));
                        }
                    }
                    pick.and_then(|(mine, from, d)| {
                        self.lanes.flood_into(from, Some(&ports), false, &mut scratch, &mut queue);
                        let &(theirs, to) = targets.iter().find(|&&(_, to)| scratch.dist[to] == d)?;
                        Some((mine, theirs, lane_of(&self.lanes, &mut scratch, &mut queue, from, to, d)))
                    })
                }
            };
            let Some((mine, theirs, lane)) = best else {
                self.disconnect(&made);
                (self.scratch, self.queue) = (scratch, queue);
                return None;
            };
            for &n in &lane {
                self.lanes.used[usize::from(n)] = true;
            }
            self.placed[me].used |= 1 << mine;
            self.placed[oi].used |= 1 << theirs;
            let a = DoorUse { node: me_node, door: mine };
            let b = DoorUse { node: other_node, door: theirs };
            made.push(if usize::from(e.from) == me_node {
                Corridor { edge: ei, a, b, lane }
            } else {
                let mut lane = lane;
                lane.reverse();
                Corridor { edge: ei, a: b, b: a, lane }
            });
        }
        (self.scratch, self.queue) = (scratch, queue);
        Some(made)
    }

    /// Take back what `connect` did for the last placed room: free its lanes and the doors it
    /// used on its neighbours.
    fn disconnect(&mut self, made: &[Corridor]) {
        let me = self.placed.len() - 1;
        let me_node = self.placed[me].node;
        for c in made {
            for &n in &c.lane {
                self.lanes.used[usize::from(n)] = false;
            }
            let (other, theirs) = if c.a.node == me_node { (c.b.node, c.b.door) } else { (c.a.node, c.a.door) };
            if let Some(oi) = self.at[other] {
                self.placed[oi].used &= !(1 << theirs);
            }
        }
        self.placed[me].used = 0;
    }

    fn finish(&self, dropped: Vec<u8>, fallback: bool) -> Layout {
        let placements = self
            .placed
            .iter()
            .map(|p| MissionPlacement {
                node: p.node as u8,
                template: p.template,
                bay: (p.bx as u8, p.by as u8),
                turn: p.shape.turn,
                mirror: p.shape.mirror,
            })
            .collect();
        let mut corridors = self.corridors.clone();
        corridors.sort_by_key(|c| c.edge);
        Layout { placements, corridors, dropped, fallback }
    }
}

fn bay_distance(a: &Placed, bx: i32, by: i32) -> i32 {
    (a.bx - bx).abs() + (a.by - by).abs()
}

/// The bay a door looks into, which may be off the lattice.
fn facing_bay(p: &Placed, d: &RoomDoor) -> (i32, i32) {
    let bay = i32::from(d.bay);
    match d.side {
        RoomSide::N => (p.bx + bay, p.by - 1),
        RoomSide::S => (p.bx + bay, p.by + i32::from(p.shape.bays.1)),
        RoomSide::W => (p.bx - 1, p.by + bay),
        RoomSide::E => (p.bx + i32::from(p.shape.bays.0), p.by + bay),
    }
}

/// Shapes that offer the same doors are the same shape to the layout: judge one, then pick
/// among them. Doors are in id order in every shape, so equal lists are equal offers.
fn by_doors(shapes: &'static [RoomShape]) -> Vec<Vec<&'static RoomShape>> {
    let same = |a: &RoomShape, b: &RoomShape| {
        a.bays == b.bays
            && a.doors.len() == b.doors.len()
            && a.doors.iter().zip(b.doors).all(|(x, y)| x.id == y.id && x.required == y.required)
    };
    let mut groups: Vec<Vec<&'static RoomShape>> = Vec::new();
    for shape in shapes {
        match groups.iter_mut().find(|g| same(g[0], shape)) {
            Some(g) => g.push(shape),
            None => groups.push(vec![shape]),
        }
    }
    groups
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    group: usize,
    bx: i32,
    by: i32,
    score: i32,
}

fn candidates_for(
    s: &mut State,
    g: &Graph,
    node: usize,
    tid: TemplateId,
    groups: &[Vec<&'static RoomShape>],
    first: bool,
) -> Vec<Candidate> {
    let m = g.m;
    let mut out = Vec::new();
    let (cols, rows) = (s.lanes.cols, s.lanes.rows);
    let kind = m.nodes[node].kind;
    let mut floods: FloodCache = vec![None; s.lanes.count];
    for (gi, group) in groups.iter().enumerate() {
        let shape = group[0];
        let (bw, bh) = (i32::from(shape.bays.0), i32::from(shape.bays.1));
        for by in 0..=rows - bh {
            for bx in 0..=cols - bw {
                if !s.bay_free(bx, by, shape.bays) {
                    continue;
                }
                // The way in from outside is on an edge of the lattice, as a mine mouth is on the side of its hill.
                if first && bx > 0 && by > 0 && bx + bw < cols && by + bh < rows {
                    continue;
                }
                s.put(Placed { node, template: tid, shape, bx, by, used: 0 });
                if let Some(made) = s.connect(g, Some(&mut floods)) {
                    let me = s.placed.len() - 1;
                    let mut ok = !s.starved(g);
                    // Every door the template insists on must be spoken for by the time its node has all its edges.
                    if ok && s.pending_of(g, node) == 0 {
                        let p = &s.placed[me];
                        ok = shape.doors.iter().enumerate().all(|(i, d)| !d.required || p.uses(i));
                    }
                    if ok {
                        let mut score = s.room_to_grow(g);
                        for c in &made {
                            score -= 8 * (c.lane.len() as i32 - 1);
                        }
                        if kind == MissionNodeKind::Boss {
                            if let Some(entrance) = s.placed.first() {
                                score += 2 * bay_distance(entrance, bx, by);
                            }
                        }
                        if kind == MissionNodeKind::Rest {
                            if let Some(hub) = s.placed.iter().find(|p| m.nodes[p.node].kind == MissionNodeKind::Hub) {
                                score -= 2 * bay_distance(hub, bx, by);
                            }
                        }
                        // The rule Gungeon uses: when this room is one end of a cycle still open, sit near the other end.
                        for e in m.edges {
                            let (from, to) = (usize::from(e.from), usize::from(e.to));
                            let via = if from == node {
                                to
                            } else if to == node {
                                from
                            } else {
                                continue;
                            };
                            if s.at[via].is_some() || e.kind == MissionEdgeKind::Sight {
                                continue;
                            }
                            for f in m.edges {
                                let (ff, ft) = (usize::from(f.from), usize::from(f.to));
                                let far = if ff == via {
                                    ft
                                } else if ft == via {
                                    ff
                                } else {
                                    continue;
                                };
                                if far == node {
                                    continue;
                                }
                                if let Some(pi) = s.at[far] {
                                    if pi != me {
                                        score -= (bay_distance(&s.placed[pi], bx, by) - 2).max(0);
                                    }
                                }
                            }
                        }
                        out.push(Candidate { group: gi, bx, by, score });
                    }
                    s.disconnect(&made);
                }
                s.take();
            }
        }
    }
    out
}

fn is_big(m: &MissionDef, node: usize) -> bool {
    catalog().dungeons.templates_of(m.nodes[node].pool).any(|t| u16::from(t.bays.0) * u16::from(t.bays.1) > 1)
}

/// Critical nodes by `order`, each only once a neighbour of it stands; then the side rooms.
/// One exception to the order: a room of more than one bay goes down as soon as a neighbour of
/// it stands. The big rooms are the hard ones to fit, and a lattice that is already full of
/// small rooms has nowhere left for a boss arena; placed early, they shape everything else.
pub fn placing_order(m: &MissionDef) -> Vec<usize> {
    let mut rest: Vec<usize> = (0..m.nodes.len()).filter(|&i| m.nodes[i].critical).collect();
    rest.sort_by_key(|&i| (m.nodes[i].order, i));
    let mut out: Vec<usize> = Vec::new();
    let joined = |out: &[usize], n: usize| {
        m.edges.iter().any(|e| {
            e.kind != MissionEdgeKind::Sight
                && ((usize::from(e.from) == n && out.contains(&usize::from(e.to)))
                    || (usize::from(e.to) == n && out.contains(&usize::from(e.from))))
        })
    };
    while !rest.is_empty() {
        let at = if out.is_empty() {
            0
        } else {
            rest.iter()
                .position(|&n| is_big(m, n) && joined(&out, n))
                .or_else(|| rest.iter().position(|&n| joined(&out, n)))
                .unwrap_or(0)
        };
        out.push(rest.remove(at));
    }
    out.extend((0..m.nodes.len()).filter(|&i| !m.nodes[i].critical));
    out
}

/// One embedding's depth-first search.
struct Embed<'g> {
    g: &'g Graph,
    s: State,
    order: Vec<usize>,
    used_templates: Vec<TemplateId>,
    dropped: Vec<u8>,
    sides: u8,
    budget: i32,
    choose: Sfc32,
    place_rng: Sfc32,
}

impl Embed<'_> {
    /// Place the nodes in order, depth first. At each node the candidates are scored and the
    /// best three are tried in a seeded order, then the rest by score; a node with nowhere to go
    /// sends the search back to re-place the one before it (Gungeon backs up and re-picks too).
    /// `budget` bounds the whole search, so a hopeless start is given up.
    fn place(&mut self, at: usize) -> bool {
        let m = self.g.m;
        if at == self.order.len() {
            return self.sides >= m.budget.side_rooms.0;
        }
        let node = self.order[at];
        let critical = m.nodes[node].critical;
        // Side rooms are placed until the budget's upper end is met; one that does not fit is left
        // out, and the layout only fails if fewer than the lower end found a place.
        if !critical && self.sides >= m.budget.side_rooms.1 {
            return self.drop_and_go_on(node, at);
        }
        // Choose: a template of the pool not used yet in this dungeon (no room twice), seeded. One
        // that has nowhere to go at all is re-picked, up to three times; one that had somewhere to
        // go and led nowhere is not, or every dead end would be explored once per variant.
        let dungeons = &catalog().dungeons;
        let pool = dungeons.pool(m.nodes[node].pool);
        let mut choices: Vec<TemplateId> =
            pool.templates.iter().copied().filter(|t| !self.used_templates.contains(t)).collect();
        self.choose.shuffle(&mut choices);
        choices.truncate(3);
        for tid in choices {
            let template = dungeons.template(tid);
            let groups = by_doors(template.shapes);
            let first = self.s.placed.is_empty();
            let mut found = candidates_for(&mut self.s, self.g, node, tid, &groups, first);
            if found.is_empty() {
                continue;
            }
            // The sort is total: score, then position, then the turn and mirror of the first shape.
            found.sort_by(|a, b| {
                let (sa, sb) = (groups[a.group][0], groups[b.group][0]);
                b.score
                    .cmp(&a.score)
                    .then(a.by.cmp(&b.by))
                    .then(a.bx.cmp(&b.bx))
                    .then(sa.turn.cmp(&sb.turn))
                    .then(sa.mirror.cmp(&sb.mirror))
            });
            let mut tries: Vec<Candidate> = found.iter().take(3).copied().collect();
            self.place_rng.shuffle(&mut tries);
            tries.extend(found.iter().skip(3).take(EMBED_WIDTH - 3).copied());
            for pick in tries {
                let left = self.budget;
                self.budget -= 1;
                if left <= 0 {
                    return false;
                }
                let group = &groups[pick.group];
                let shape = *self.place_rng.pick(group).expect("a group is never empty");
                self.s.put(Placed { node, template: tid, shape, bx: pick.bx, by: pick.by, used: 0 });
                let made = self.s.connect(self.g, None).expect("a candidate that connected once connects again");
                let before = self.s.corridors.len();
                self.s.corridors.extend(made.iter().cloned());
                self.used_templates.push(tid);
                if !critical {
                    self.sides += 1;
                }
                if self.place(at + 1) {
                    return true;
                }
                if !critical {
                    self.sides -= 1;
                }
                self.used_templates.pop();
                self.s.corridors.truncate(before);
                self.s.disconnect(&made);
                self.s.take();
            }
            break;
        }
        // A side room that does not fit this seed is left out. A critical one sends the search back.
        if critical {
            return false;
        }
        self.drop_and_go_on(node, at)
    }

    fn drop_and_go_on(&mut self, node: usize, at: usize) -> bool {
        self.dropped.push(node as u8);
        if self.place(at + 1) {
            return true;
        }
        self.dropped.pop();
        false
    }
}

/// Up to `EMBED_TRIES` seeded embeddings for one attempt; `None` if none of them fits.
pub fn embed(m: &'static MissionDef, seed: u32, zone: ZoneId, attempt: u8) -> Option<Layout> {
    let g = Graph::new(m);
    let order = placing_order(m);
    for t in 0..EMBED_TRIES {
        let mut e = Embed {
            g: &g,
            s: State::new(m),
            order: order.clone(),
            used_templates: Vec::new(),
            dropped: Vec::new(),
            sides: 0,
            budget: EMBED_BUDGET,
            choose: dice(seed, zone, Step::DunChoose, attempt, i32::from(t), 0),
            place_rng: dice(seed, zone, Step::DunEmbed, attempt, i32::from(t), 0),
        };
        if e.place(0) {
            let dropped = core::mem::take(&mut e.dropped);
            return Some(e.s.finish(dropped, false));
        }
    }
    None
}

/// The hand-placed embedding: the same joining of doors, with nothing left to chance. An error
/// names what the author got wrong (the build lints the fallback, so this is a bug if it fires).
pub fn embed_fallback(m: &'static MissionDef) -> Result<Layout, String> {
    let g = Graph::new(m);
    let mut s = State::new(m);
    let dungeons = &catalog().dungeons;
    for node in placing_order(m) {
        let n = &m.nodes[node];
        let Some(row) = m.fallback.iter().find(|f| usize::from(f.node) == node) else {
            if n.critical {
                return Err(format!("dungeon \"{}\": the fallback does not place \"{}\"", m.id, n.id));
            }
            continue;
        };
        let template = dungeons.template(row.template);
        let (bx, by) = (i32::from(row.bay.0), i32::from(row.bay.1));
        let Some(shape) = template.shape(row.turn, row.mirror).filter(|sh| s.bay_free(bx, by, sh.bays)) else {
            return Err(format!("dungeon \"{}\": the fallback row for \"{}\" does not fit", m.id, n.id));
        };
        s.put(Placed { node, template: row.template, shape, bx, by, used: 0 });
        let Some(made) = s.connect(&g, None) else {
            return Err(format!("dungeon \"{}\": the fallback cannot join \"{}\" to its neighbours", m.id, n.id));
        };
        s.corridors.extend(made);
    }
    let dropped = (0..m.nodes.len()).filter(|&i| s.at[i].is_none()).map(|i| i as u8).collect();
    Ok(s.finish(dropped, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_nodes_sit_on_the_lines() {
        let l = Lanes::new(4, 3);
        assert_eq!(l.count, 20 + 16 + 15);
        assert_eq!(l.cell(l.corner(0, 0)), (BORDER, BORDER));
        assert_eq!(l.cell(l.hmid(1, 2)), (BORDER + BAY_W + MOUTH_X, BORDER + 2 * BAY_H));
        assert_eq!(l.cell(l.vmid(4, 1)), (BORDER + 4 * BAY_W, BORDER + BAY_H + MOUTH_Y));
        // Every mid joins two corners; a corner joins two to four mids.
        for n in 0..l.count {
            let deg = l.links(n).len();
            assert!((2..=4).contains(&deg), "lane {n} has {deg} links");
        }
        let f = l.flood(l.hmid(0, 0));
        assert!(f.dist.iter().all(|&d| d >= 0), "the lattice's lines are one graph");
    }
}
