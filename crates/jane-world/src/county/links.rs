//! The link lanes (`linkRoad`, `connect` in `county.ts`). Where a road or a footpath runs into a
//! chunk's box the stamp has drawn over it. Each place a line crosses into or out of the box (grown
//! by two cells) is joined to the chunk's nearest gate by a lane three cells wide that goes ROUND
//! the box, on a ring four cells out, so it never cuts a fence, a wall or a kitchen. No dice: the
//! lanes are geometry.
//!
//! Where this parts from the TypeScript: every gate gets a step of lane whether a line reached it
//! or not (a gate is a walkable cell outside the box, never the tree the land left there); a lane
//! never paints inside any chunk's box (the ring of the halt, pinned to the county's edge, ran
//! through its own platform, and one place's ring could run through a neighbour); and a lane leaves
//! a road's metal, the line's rails and a bridge's planks as they are, and crosses water on planks
//! rather than filling it in with dirt.

use alloc::vec::Vec;
use jane_core::{Rect, Tile};

use super::County;
use super::chunks::Chunk;
use super::ways::{Way, WayKind};
use crate::kit::{Kit, js_round};

/// Cells from the box to the ring the lanes run round it on.
pub const RING: i32 = 4;
/// The ring keeps this far in from the county's edge.
const RING_EDGE: i32 = 6;
/// A line counts as reaching a chunk within this many cells of its box.
const REACH: i32 = 2;

/// Every road's, path's and footpath's line joined to every chunk it reaches; then a step of lane
/// on every gate, so a gate no line reached is still a walkable cell outside the box, as a gate
/// is, rather than the tree or the water the land left there.
pub fn link_lines(c: &mut County<'_>) {
    let boxes: Vec<Rect> = c.chunks.iter().map(|ch| ch.bounds).collect();
    let mut ways = Vec::new();
    for line in &c.lines {
        for ch in &c.chunks {
            link_line(line, ch, c.k.w(), c.k.h(), &mut ways);
        }
    }
    for way in &ways {
        for &q in way {
            lane(&mut c.k, &mut c.trodden, q, &boxes);
        }
    }
    c.ways.extend(ways.into_iter().map(|line| Way { kind: WayKind::Link, line, door: None }));
    for ch in &c.chunks {
        for &g in &ch.gates {
            lane(&mut c.k, &mut c.trodden, g, &boxes);
        }
    }
}

/// Join `line` to `ch` at every point it crosses into or out of the box grown by [`REACH`], from
/// the last point outside.
pub fn link_line(line: &[(i32, i32)], ch: &Chunk, w: i32, h: i32, ways: &mut Vec<Vec<(i32, i32)>>) {
    let near = ch.bounds.grow(REACH);
    let Some(&(x0, y0)) = line.first() else { return };
    let mut was = near.contains(x0, y0);
    for i in 1..line.len() {
        let now = near.contains(line[i].0, line[i].1);
        if now != was {
            if let Some(way) = connect(if now { line[i - 1] } else { line[i] }, ch, w, h) {
                ways.push(way);
            }
        }
        was = now;
    }
}

/// A lane from `p` to the chunk's gate nearest it (by Manhattan distance, the first of equals):
/// straight out to the ring, the shorter way round it, and straight in to the gate. Its centre
/// line, from the gate out.
fn connect(p: (i32, i32), ch: &Chunk, cw: i32, ch_h: i32) -> Option<Vec<(i32, i32)>> {
    let &gate = ch.gates.iter().min_by_key(|g| (g.0 - p.0).abs() + (g.1 - p.1).abs())?;
    let b = ch.bounds;
    let (x0, y0) = ((b.x - RING).max(RING_EDGE), (b.y - RING).max(RING_EDGE));
    let x1 = (b.right() + RING - 1).min(cw - RING_EDGE - 1);
    let y1 = (b.bottom() + RING - 1).min(ch_h - RING_EDGE - 1);
    let path = perimeter(Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1));
    if path.is_empty() {
        return None;
    }
    let nearest = |q: (i32, i32)| {
        let mut at = 0;
        let mut best = i32::MAX;
        for (n, &(x, y)) in path.iter().enumerate() {
            let d = (x - q.0).abs() + (y - q.1).abs();
            if d < best {
                best = d;
                at = n;
            }
        }
        at
    };
    let (a, g) = (nearest(p), nearest(gate));
    let len = path.len();
    let forward = (g + len - a) % len;
    let back = forward > len - forward;
    let mut out = Vec::new();
    straight(p, path[a], |q| out.push(q));
    let mut n = a;
    loop {
        out.push(path[n]);
        if n == g {
            break;
        }
        n = if back { (n + len - 1) % len } else { (n + 1) % len };
    }
    straight(path[g], gate, |q| out.push(q));
    out.reverse();
    Some(out)
}

/// Three cells square of lane round `(x, y)`: trodden dirt, planks over water; a road, the
/// railway and every chunk's box left as they are.
fn lane(k: &mut Kit, trodden: &mut crate::bits::Bits, (x, y): (i32, i32), boxes: &[Rect]) {
    for cy in y - 1..=y + 1 {
        for cx in x - 1..=x + 1 {
            if !k.inside(cx, cy) || boxes.iter().any(|b| b.contains(cx, cy)) {
                continue;
            }
            match k.get(cx, cy) {
                Tile::Road | Tile::Boardwalk | Tile::Track | Tile::Rail => {}
                Tile::Water => k.set(cx, cy, Tile::Boardwalk),
                _ => {
                    k.set(cx, cy, Tile::Dirt);
                    super::ways::tread(trodden, k, cx, cy);
                }
            }
        }
    }
}

/// The cells round the edge of `r`, clockwise from its top-left.
pub fn perimeter(r: Rect) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    if r.w <= 0 || r.h <= 0 {
        return out;
    }
    let (right, bottom) = (r.right() - 1, r.bottom() - 1);
    out.extend((r.x..=right).map(|x| (x, r.y)));
    out.extend((r.y + 1..=bottom).map(|y| (right, y)));
    out.extend((r.x..right).rev().map(|x| (x, bottom)));
    out.extend((r.y + 1..bottom).rev().map(|y| (r.x, y)));
    out
}

/// Every cell of the straight line from `a` to `b`, in `max(|dx|, |dy|)` steps, rounded as
/// JavaScript rounds.
fn straight(a: (i32, i32), b: (i32, i32), mut f: impl FnMut((i32, i32))) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    if steps == 0 {
        f(a);
        return;
    }
    for n in 0..=steps {
        let along = |p: i32, q: i32| p + js_round(i64::from(q - p) * i64::from(n), i64::from(steps)) as i32;
        f((along(a.0, b.0), along(a.1, b.1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_perimeter_goes_round_once() {
        let p = perimeter(Rect::new(2, 3, 4, 3));
        assert_eq!(p.len(), 2 * 4 + 2 * 3 - 4);
        assert_eq!(p[0], (2, 3));
        assert_eq!(p[3], (5, 3));
        assert_eq!(p[p.len() - 1], (2, 4));
        for w in p.windows(2) {
            assert_eq!((w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs(), 1, "{w:?}");
        }
    }

    #[test]
    fn a_straight_line_reaches_both_ends() {
        let mut cells = Vec::new();
        straight((0, 0), (5, 2), |q| cells.push(q));
        assert_eq!(cells.first(), Some(&(0, 0)));
        assert_eq!(cells.last(), Some(&(5, 2)));
        assert_eq!(cells.len(), 6);
    }
}
