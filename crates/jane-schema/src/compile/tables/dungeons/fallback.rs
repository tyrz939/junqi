//! The hand-placed fallback, stamped at build (layout.ts `embedFallback`): every critical node
//! placed by a row, in a transform its template allows and fits, on free bays, and joined to
//! every placed neighbour by a free lane. The runtime spends it on its last attempt, so a row
//! that does not stamp is a build error, never a throw at the player.
//!
//! The lane graph and `connect` are layout.ts's; the P3 layout owns the seeded search.

use super::pools::Library;
use super::raw::{RawEdgeKind, RawMission, RawNode};
use super::room::{Door, Shape};
use crate::model::RoomSide;

/// The lines between bays as a graph: corners, and the middle of every bay side (the port a
/// door on that side opens onto).
pub struct Lanes {
    pub cols: i32,
    pub rows: i32,
    corners: i32,
    hmids: i32,
    pub used: Vec<bool>,
    links: Vec<Vec<usize>>,
}

impl Lanes {
    pub fn new(cols: i32, rows: i32) -> Lanes {
        let corners = (cols + 1) * (rows + 1);
        let hmids = cols * (rows + 1);
        let count = (corners + hmids + (cols + 1) * rows) as usize;
        let mut l = Lanes { cols, rows, corners, hmids, used: vec![false; count], links: vec![Vec::new(); count] };
        for j in 0..=rows {
            for i in 0..cols {
                l.link(l.hmid(i, j), l.corner(i, j));
                l.link(l.hmid(i, j), l.corner(i + 1, j));
            }
        }
        for j in 0..rows {
            for i in 0..=cols {
                l.link(l.vmid(i, j), l.corner(i, j));
                l.link(l.vmid(i, j), l.corner(i, j + 1));
            }
        }
        l
    }

    fn link(&mut self, a: usize, b: usize) {
        self.links[a].push(b);
        self.links[b].push(a);
    }

    fn corner(&self, i: i32, j: i32) -> usize {
        (j * (self.cols + 1) + i) as usize
    }

    /// Middle of the horizontal line above bay row j, over bay column i.
    fn hmid(&self, i: i32, j: i32) -> usize {
        (self.corners + j * self.cols + i) as usize
    }

    /// Middle of the vertical line left of bay column i, beside bay row j.
    fn vmid(&self, i: i32, j: i32) -> usize {
        (self.corners + self.hmids + j * (self.cols + 1) + i) as usize
    }

    /// The port a door opens onto, for a room whose first bay is (bx, by).
    pub fn port(&self, bx: i32, by: i32, bays: (i32, i32), d: &Door) -> usize {
        match d.side {
            RoomSide::N => self.hmid(bx + d.bay, by),
            RoomSide::S => self.hmid(bx + d.bay, by + bays.1),
            RoomSide::W => self.vmid(bx, by + d.bay),
            RoomSide::E => self.vmid(bx + bays.0, by + d.bay),
        }
    }

    /// Lines that run through the inside of a room of more than one bay carry nothing.
    fn block_inside(&mut self, bx: i32, by: i32, bays: (i32, i32), value: bool) {
        for j in by..by + bays.1 {
            for i in bx + 1..bx + bays.0 {
                let n = self.vmid(i, j);
                self.used[n] = value;
            }
        }
        for j in by + 1..by + bays.1 {
            for i in bx..bx + bays.0 {
                let n = self.hmid(i, j);
                self.used[n] = value;
            }
        }
        for j in by + 1..by + bays.1 {
            for i in bx + 1..bx + bays.0 {
                let n = self.corner(i, j);
                self.used[n] = value;
            }
        }
    }

    fn inside_taken(&self, bx: i32, by: i32, bays: (i32, i32)) -> bool {
        for j in by..by + bays.1 {
            for i in bx + 1..bx + bays.0 {
                if self.used[self.vmid(i, j)] {
                    return true;
                }
            }
        }
        for j in by + 1..by + bays.1 {
            for i in bx..bx + bays.0 {
                if self.used[self.hmid(i, j)] {
                    return true;
                }
            }
        }
        for j in by + 1..by + bays.1 {
            for i in bx + 1..bx + bays.0 {
                if self.used[self.corner(i, j)] {
                    return true;
                }
            }
        }
        false
    }

    /// Breadth-first from a port over free lane nodes: (dist, prev), -1 where it never got.
    fn flood(&self, from: usize) -> (Vec<i32>, Vec<i32>) {
        let mut dist = vec![-1; self.used.len()];
        let mut prev = vec![-1; self.used.len()];
        if self.used[from] {
            return (dist, prev);
        }
        let mut queue = vec![from];
        dist[from] = 0;
        let mut q = 0;
        while q < queue.len() {
            let n = queue[q];
            q += 1;
            for &m in &self.links[n] {
                if dist[m] >= 0 || self.used[m] {
                    continue;
                }
                dist[m] = dist[n] + 1;
                prev[m] = n as i32;
                queue.push(m);
            }
        }
        (dist, prev)
    }
}

struct Placed<'a> {
    node: &'a RawNode,
    shape: &'a Shape,
    bx: i32,
    by: i32,
    used_doors: Vec<String>,
}

/// Critical nodes by `order`, each only once a neighbour of it stands; then the side rooms. A
/// room of more than one bay goes down as soon as a neighbour of it stands (layout.ts `placingOrder`).
fn placing_order<'a>(m: &'a RawMission, lib: &Library) -> Vec<&'a RawNode> {
    let mut critical: Vec<(usize, &RawNode)> = m.nodes.iter().enumerate().filter(|(_, n)| n.critical).collect();
    critical.sort_by_key(|(i, n)| (n.order, *i));
    let big = |n: &RawNode| lib.pool(&n.pool).any(|t| t.bays.0 * t.bays.1 > 1);
    let mut out: Vec<&RawNode> = Vec::new();
    let mut rest: Vec<&RawNode> = critical.into_iter().map(|(_, n)| n).collect();
    let joined = |n: &RawNode, out: &[&RawNode]| {
        m.edges.iter().any(|e| {
            !matches!(e.kind, RawEdgeKind::Sight)
                && ((e.from == n.id && out.iter().any(|o| o.id == e.to))
                    || (e.to == n.id && out.iter().any(|o| o.id == e.from)))
        })
    };
    while !rest.is_empty() {
        let mut at = if out.is_empty() { Some(0) } else { rest.iter().position(|n| big(n) && joined(n, &out)) };
        if at.is_none() {
            at = rest.iter().position(|n| joined(n, &out));
        }
        out.push(rest.remove(at.unwrap_or(0)));
    }
    out.extend(m.nodes.iter().filter(|n| !n.critical));
    out
}

struct State<'a> {
    m: &'a RawMission,
    lanes: Lanes,
    owner: Vec<i32>,
    placed: Vec<Placed<'a>>,
}

impl<'a> State<'a> {
    fn bay_free(&self, bx: i32, by: i32, bays: (i32, i32)) -> bool {
        if bx < 0 || by < 0 || bx + bays.0 > self.lanes.cols || by + bays.1 > self.lanes.rows {
            return false;
        }
        for j in by..by + bays.1 {
            for i in bx..bx + bays.0 {
                if self.owner[(j * self.lanes.cols + i) as usize] >= 0 {
                    return false;
                }
            }
        }
        // A room of more than one bay swallows the lines between its bays, so nothing may be running on them.
        !self.lanes.inside_taken(bx, by, bays)
    }

    fn put(&mut self, p: Placed<'a>) {
        let index = self.placed.len() as i32;
        for j in p.by..p.by + p.shape.bays.1 {
            for i in p.bx..p.bx + p.shape.bays.0 {
                self.owner[(j * self.lanes.cols + i) as usize] = index;
            }
        }
        self.lanes.block_inside(p.bx, p.by, p.shape.bays, true);
        self.placed.push(p);
    }

    /// Join the last placed room to every placed neighbour: for each edge, the pair of free
    /// doors with the shortest lane between them; a tie goes to the earlier door.
    fn connect(&mut self) -> bool {
        let me = self.placed.len() - 1;
        let id = self.placed[me].node.id.clone();
        for e in &self.m.edges {
            if matches!(e.kind, RawEdgeKind::Sight) || (e.from != id && e.to != id) {
                continue;
            }
            let other_id = if e.from == id { &e.to } else { &e.from };
            let Some(other) = self.placed.iter().position(|p| p.node.id == *other_id) else { continue };
            if other == me {
                continue;
            }
            let mut best: Option<(String, String, Vec<usize>)> = None;
            let (mine_p, their_p) = (&self.placed[me], &self.placed[other]);
            for mine in &mine_p.shape.doors {
                if mine_p.used_doors.contains(&mine.id) {
                    continue;
                }
                let from = self.lanes.port(mine_p.bx, mine_p.by, mine_p.shape.bays, mine);
                let (dist, prev) = self.lanes.flood(from);
                for theirs in &their_p.shape.doors {
                    if their_p.used_doors.contains(&theirs.id) {
                        continue;
                    }
                    let to = self.lanes.port(their_p.bx, their_p.by, their_p.shape.bays, theirs);
                    if dist[to] < 0 {
                        continue;
                    }
                    if best.as_ref().is_some_and(|b| b.2.len() as i32 <= dist[to] + 1) {
                        continue;
                    }
                    let mut lane = Vec::new();
                    let mut n = to as i32;
                    while n >= 0 {
                        lane.push(n as usize);
                        n = prev[n as usize];
                    }
                    lane.reverse();
                    best = Some((mine.id.clone(), theirs.id.clone(), lane));
                }
            }
            let Some((mine, theirs, lane)) = best else { return false };
            for n in lane {
                self.lanes.used[n] = true;
            }
            self.placed[me].used_doors.push(mine);
            self.placed[other].used_doors.push(theirs);
        }
        true
    }
}

/// Stamp the fallback. Each error is (where in the file, what).
pub fn check(m: &RawMission, lib: &Library) -> Vec<(String, String)> {
    let mut errors = Vec::new();
    for (i, row) in m.fallback.iter().enumerate() {
        let p = format!("fallback[{i}]");
        match m.nodes.iter().find(|n| n.id == row.node) {
            None => errors.push((p.clone(), format!("the fallback places \"{}\", which is no node", row.node))),
            Some(n) => {
                if lib.by_id(&row.template).is_some_and(|t| lib.rooms[t].pool != n.pool) {
                    errors.push((p.clone(), format!("{} is not in {}'s pool {}", row.template, n.id, n.pool)));
                }
            }
        }
        if m.fallback[..i].iter().any(|r| r.node == row.node) {
            errors.push((p, format!("the fallback places \"{}\" twice", row.node)));
        }
    }
    if !errors.is_empty() {
        return errors;
    }
    let (cols, rows) = (i32::from(m.lattice.cols), i32::from(m.lattice.rows));
    let mut s =
        State { m, lanes: Lanes::new(cols, rows), owner: vec![-1; (cols * rows).max(0) as usize], placed: Vec::new() };
    for node in placing_order(m, lib) {
        let Some((i, row)) = m.fallback.iter().enumerate().find(|(_, f)| f.node == node.id) else {
            if node.critical {
                errors.push(("fallback".to_owned(), format!("the fallback does not place \"{}\"", node.id)));
                return errors;
            }
            continue;
        };
        let p = format!("fallback[{i}]");
        let shape = lib.by_id(&row.template).and_then(|t| {
            lib.shapes[t].iter().find(|x| x.turn == i32::from(row.turn) && x.mirror == row.mirror && x.fits)
        });
        let (bx, by) = (i32::from(row.bay[0]), i32::from(row.bay[1]));
        let Some(shape) = shape.filter(|sh| s.bay_free(bx, by, sh.bays)) else {
            errors.push((p, format!("the fallback row for \"{}\" does not fit", node.id)));
            return errors;
        };
        s.put(Placed { node, shape, bx, by, used_doors: Vec::new() });
        if !s.connect() {
            errors.push((p, format!("the fallback cannot join \"{}\" to its neighbours", node.id)));
            return errors;
        }
    }
    // Every door a template insists on must be spoken for.
    for pl in &s.placed {
        for d in &pl.shape.doors {
            if d.required && !pl.used_doors.contains(&d.id) {
                errors.push((
                    "fallback".to_owned(),
                    format!("{}'s required door {} is not used by the fallback", pl.node.id, d.id),
                ));
            }
        }
    }
    errors
}
