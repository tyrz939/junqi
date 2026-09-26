//! C8: the first completion is the right length. Critical nodes in the mission's order; between
//! each pair, the real walking distance with the locks she has opened so far out of the way. She
//! opens a lock when her next room is behind it and she holds what it wants (`firstCompletion`).
//! C7 reads where along this walk the rest room fell.

use std::collections::{BTreeMap, BTreeSet};

use jane_core::action::FlagKey;
use jane_core::ids::{ItemId, Key, NameId, SpellId};
use jane_core::tile::F_SOLID;
use jane_core::{Blueprint, Grid};
use jane_data::{MissionEdgeKind, MissionNodeKind, catalog};

use super::{Check, Ctx, Fault, gains_of};
use crate::dungeon::generate::{BuildInfo, RoomInfo};

/// How far a room's middle may be from the nearest floor the walk can stand on.
const NEAR: i32 = 8;

/// The first completion, walked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Walk {
    /// Cells of walking.
    pub cells: i32,
    /// Node indices, in the order she reached them.
    pub order: Vec<usize>,
    /// Node index to the cells walked when she first got there.
    pub reached_at: BTreeMap<usize, i32>,
    /// Why the walk could not be finished (a C8 fault each).
    pub errors: Vec<String>,
}

pub fn check(c: &Ctx<'_>) -> Vec<Fault> {
    let w = &c.walk;
    let mut out: Vec<Fault> = w.errors.iter().map(|e| Fault::new(Check::C8, e.clone())).collect();
    let (lo, hi) = c.m.budget.crit_path_cells;
    if w.errors.is_empty() && (w.cells < i32::from(lo) || w.cells > i32::from(hi)) {
        out.push(Fault::new(
            Check::C8,
            format!("the first completion is {} cells of walking, outside {lo} to {hi}", w.cells),
        ));
    }
    out
}

/// What she holds on the walk.
#[derive(Debug, Default)]
struct Hand {
    keys: BTreeMap<NameId, i32>,
    items: BTreeMap<ItemId, i32>,
    verbs: Vec<SpellId>,
    flags: Vec<FlagKey>,
    controlled: Vec<u8>,
}

/// The first completion (`firstCompletion`).
pub fn first_completion(bp: &Blueprint, info: &BuildInfo) -> Walk {
    let m = info.mission;
    let mut out = Walk::default();
    let entrance = info.rooms.iter().find(|r| m.nodes[r.node].kind == MissionNodeKind::Entrance);
    let (Some(entrance), Some(layout)) = (entrance, info.layout.as_ref()) else {
        out.errors.push("no entrance".into());
        return out;
    };
    let corridors = &layout.corridors;
    let mut todo: Vec<&RoomInfo> =
        info.rooms.iter().filter(|r| m.nodes[r.node].critical && r.node != entrance.node).collect();
    todo.sort_by_key(|r| (m.nodes[r.node].order, r.node));
    let mut hand = Hand {
        keys: m.given_keys.iter().map(|&t| (t, 99)).collect(),
        verbs: m.given_verbs.to_vec(),
        ..Hand::default()
    };
    let mut opened: BTreeSet<Key> = BTreeSet::new();
    let mut open_edges: Vec<usize> = Vec::new();
    let collect = |hand: &mut Hand, node: usize| {
        let g = gains_of(bp, &m.nodes[node]);
        for (t, n) in g.keys {
            *hand.keys.entry(t).or_insert(0) += n;
        }
        for (t, n) in g.items {
            *hand.items.entry(t).or_insert(0) += n;
        }
        hand.verbs.extend(g.verbs);
        hand.flags.extend(g.flags);
        hand.controlled.extend(g.states);
    };
    let can_open = |hand: &Hand, kind: &MissionEdgeKind| -> bool {
        match *kind {
            MissionEdgeKind::State { var, is, .. } => {
                hand.controlled.contains(&var) || m.states[usize::from(var)].initial == is
            }
            MissionEdgeKind::Key { tag, .. } => hand.keys.get(&tag).copied().unwrap_or(0) > 0,
            MissionEdgeKind::Verb { verb, needs, .. } => {
                hand.verbs.contains(&verb)
                    && needs.iter().all(|n| hand.items.get(&n.item).copied().unwrap_or(0) >= i32::from(n.qty))
            }
            MissionEdgeKind::Oneway { flag, .. } => hand.flags.contains(&FlagKey::Named(Key::Name(flag))),
            _ => true,
        }
    };
    let pay = |hand: &mut Hand, kind: &MissionEdgeKind| match *kind {
        MissionEdgeKind::Key { tag, .. } => {
            let k = hand.keys.entry(tag).or_insert(0);
            if *k < 99 {
                *k -= 1;
            }
        }
        MissionEdgeKind::Verb { needs, .. } => {
            for n in needs {
                *hand.items.entry(n.item).or_insert(0) -= i32::from(n.qty);
            }
        }
        _ => {}
    };
    let floor = floor_of(bp);
    let mut here = entrance;
    collect(&mut hand, here.node);
    out.order.push(here.node);
    out.reached_at.insert(here.node, 0);
    while !todo.is_empty() {
        // The first room in the order that she can get to now, by the fewest doors.
        let mut pick: Option<(usize, Vec<usize>)> = None;
        for (ti, room) in todo.iter().enumerate() {
            let mut prev: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
            let mut queue = vec![here.node];
            let mut q = 0;
            while q < queue.len() && !prev.contains_key(&room.node) {
                for cor in corridors {
                    let e = &m.edges[cor.edge];
                    let (from, to) = (usize::from(e.from), usize::from(e.to));
                    let next = if from == queue[q] {
                        to
                    } else if to == queue[q] {
                        from
                    } else {
                        continue;
                    };
                    if next == here.node || prev.contains_key(&next) {
                        continue;
                    }
                    if !open_edges.contains(&cor.edge)
                        && !std::iter::once(&e.kind).chain(e.also).all(|k| can_open(&hand, k))
                    {
                        continue;
                    }
                    prev.insert(next, (queue[q], cor.edge));
                    queue.push(next);
                }
                q += 1;
            }
            if !prev.contains_key(&room.node) {
                continue;
            }
            let mut path = Vec::new();
            let mut at = room.node;
            while at != here.node {
                let (from, edge) = prev[&at];
                path.push(edge);
                at = from;
            }
            pick = Some((ti, path));
            break;
        }
        let Some((ti, path)) = pick else {
            let left: Vec<&str> = todo.iter().map(|r| m.nodes[r.node].id).collect();
            out.errors.push(format!(
                "the walk is stuck in {}, with {} still to reach",
                m.nodes[here.node].id,
                left.join(", ")
            ));
            return out;
        };
        for edge in path {
            if open_edges.contains(&edge) {
                continue;
            }
            open_edges.push(edge);
            let e = &m.edges[edge];
            for k in std::iter::once(&e.kind).chain(e.also) {
                pay(&mut hand, k);
            }
            for l in info.locks.iter().filter(|l| l.edge == edge) {
                opened.insert(l.prop);
            }
        }
        let to = todo[ti];
        let d = distance_to(&walk(bp, &floor, here.centre, &opened, Some(to.centre)), to.centre);
        let Some(d) = d else {
            out.errors.push(format!("no way to walk from {} to {}", m.nodes[here.node].id, m.nodes[to.node].id));
            return out;
        };
        out.cells += d;
        here = to;
        todo.remove(ti);
        out.order.push(here.node);
        out.reached_at.insert(here.node, out.cells);
        collect(&mut hand, here.node);
    }
    out
}

/// Walking distance in cells from one cell to everywhere, with the named props out of the way;
/// -1 where it cannot get. A locked gate and anything solid that is neither pushed nor carried
/// stand in the way; a hidden prop does not.
pub fn distances(bp: &Blueprint, from: (i32, i32), open: &BTreeSet<Key>) -> Grid<i32> {
    walk(bp, &floor_of(bp), from, open, None)
}

/// A walk's cells before it starts: [`FREE`] on the floor, [`CLOSED`] on solid ground.
fn floor_of(bp: &Blueprint) -> Vec<i32> {
    bp.tiles.as_slice().iter().map(|t| if t.flags() & F_SOLID == 0 { FREE } else { CLOSED }).collect()
}

/// A walk's cell not reached yet that can be; one that cannot (solid, or something stands on it).
/// Every other value is the distance there. The grid handed out reads -1 for both.
const FREE: i32 = -1;
const CLOSED: i32 = -2;

/// [`distances`], stopped as soon as `to` has its distance, if it is given: [`distance_to`] of
/// `to` reads only that cell when it has one, and the whole flood when it has none. `floor` is
/// [`floor_of`] the blueprint.
fn walk(bp: &Blueprint, floor: &[i32], from: (i32, i32), open: &BTreeSet<Key>, to: Option<(i32, i32)>) -> Grid<i32> {
    let cat = catalog();
    let (w, h) = (bp.w() as i32, bp.h() as i32);
    // What she can stand on: the floor less what stands on it.
    let mut cells = floor.to_vec();
    for p in &bp.props {
        if p.hidden || open.contains(&p.key) {
            continue;
        }
        let d = cat.story.prop(p.def);
        let shut = if d.gate { p.locked } else { d.solid && !d.push && !d.carry };
        if shut {
            let r = jane_core::Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
            if let Some(r) = r.intersect(jane_core::Rect::new(0, 0, w, h)) {
                for y in r.y..r.bottom() {
                    cells[(y * w + r.x) as usize..(y * w + r.right()) as usize].fill(CLOSED);
                }
            }
        }
    }
    let done = |mut cells: Vec<i32>| {
        for c in &mut cells {
            *c = (*c).max(FREE);
        }
        Grid::from_vec(bp.w(), bp.h(), cells)
    };
    let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h;
    let free = |cells: &[i32], x: i32, y: i32| inside(x, y) && cells[(y * w + x) as usize] == FREE;
    // The room's middle may have a pillar on it: start from a cell near it that is floor.
    let mut start = from;
    let mut r = 0;
    while r < NEAR && !free(&cells, start.0, start.1) {
        for y in from.1 - r..=from.1 + r {
            for x in from.0 - r..=from.0 + r {
                if free(&cells, x, y) {
                    start = (x, y);
                }
            }
        }
        r += 1;
    }
    // Found nothing: the flood starts where it stands and reaches what is free beside it.
    if !inside(start.0, start.1) {
        return done(cells);
    }
    cells[(start.1 * w + start.0) as usize] = 0;
    if to == Some(start) {
        return done(cells);
    }
    let mut queue = vec![start];
    let mut head = 0;
    while head < queue.len() {
        let (x, y) = queue[head];
        head += 1;
        let d = cells[(y * w + x) as usize] + 1;
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if free(&cells, nx, ny) {
                cells[(ny * w + nx) as usize] = d;
                if to == Some((nx, ny)) {
                    return done(cells);
                }
                queue.push((nx, ny));
            }
        }
    }
    done(cells)
}

/// The nearest reachable cell to a room's middle, as a distance.
pub fn distance_to(dist: &Grid<i32>, at: (i32, i32)) -> Option<i32> {
    for r in 0..NEAR {
        let mut best: Option<i32> = None;
        for y in at.1 - r..=at.1 + r {
            for x in at.0 - r..=at.0 + r {
                let d = dist.read(x, y, -1);
                if d >= 0 && best.is_none_or(|b| d < b) {
                    best = Some(d);
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}
