//! An edge round every named patch (the world audit's Lynch rec. 9): a patch was a circle on the
//! threat field with nothing on the ground to say she had crossed in. Now each has a perimeter of
//! its own kind: an old field's boundary, seven to ten runs corner to corner round the patch's
//! centre, at 78 to 95 in a hundred of its radius, each run square to the grid with a jog in it
//! (so the hedge tiles join as they do along a field edge, never in a staircase):
//!
//! | Edge | Where | What |
//! | --- | --- | --- |
//! | **Hedge** | the Lowfields' wild patches, Mother's Garden, Bellfield | a hedge one to two deep, an oak now and then and at every corner; long grass along its foot |
//! | **Wall** | fields and yards: the Top Field, the allotments, Quarry Steps, Glasshouse Row, Chapel Rise | a low dry-stone wall, a bush at the corners, a tuft at its foot |
//! | **Reed edge** | the Waters | a drain one to two wide, reed along both banks, a clump of willow scrub |
//! | **Slag bank** | the Works | a bank of slag two deep, cinder earth either side |
//!
//! Laid after the stories and before the scatter and the ways, on open ground only: never on a
//! claimed cell (a road's margin, a set place, a small place), a way, water, a prop's footprint, a
//! mark, a unit or a named rect, nor on ground a dresser laid (the allotments' plots, the Top
//! Field's drills). So every road, lane, footpath and rail that crosses an edge crosses it through
//! a gap at least five cells wide, and the edge stops short of every place the quests stand on.
//! Three to five gateways are left in runs nothing crosses, so it reads as a field's edge and not
//! a pen.
//!
//! **Nothing is shut in.** Before and after laying it, the patch's box is flooded from its border;
//! any open cell the edge cut off from where it got to before gets a gap knocked through the
//! nearest stretch of edge, until none is. No dice but the patch's own ([`Step::CountyPerimeter`]);
//! integers only.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::grid::DIRS4;
use jane_core::hash::mix32;
use jane_core::num::isqrt;
use jane_core::search::{Fill, fill};
use jane_core::tile::F_SOLID;
use jane_core::{Rect, Tile};

use super::{County, centre};
use crate::skeleton::Region;
use crate::steps::Step;

/// What an edge is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Hedge,
    Wall,
    Reed,
    Slag,
}

/// One patch's edge as laid, for the tests and the viewers.
#[derive(Clone, Debug, Default)]
pub struct Perimeter {
    /// The patch's row in the area table.
    pub row: u8,
    pub edge: Option<Edge>,
    pub centre: (i32, i32),
    pub radius: i32,
    /// Every cell of the edge's core (the hedge, the wall, the drain, the bank) as it stands.
    pub core: Vec<(i32, i32)>,
    /// Gaps knocked through so nothing was shut in.
    pub knocked: u32,
    /// Whether anything was still shut in when the knocking gave up (the edge is then taken up).
    pub gave_up: bool,
}

/// The patches whose edge is not their region's: fields and yards get a wall.
const WALLED: &[&str] = &["top_field", "allotments", "quarry_steps", "glasshouse_row", "chapel_rise"];
const HEDGED: &[&str] = &["mothers_garden", "bellfield"];

/// The edge a patch gets.
pub fn edge_of(id: &str, region: Region) -> Edge {
    if WALLED.contains(&id) {
        Edge::Wall
    } else if HEDGED.contains(&id) {
        Edge::Hedge
    } else {
        match region {
            Region::Lowfields => Edge::Hedge,
            Region::Waters => Edge::Reed,
            Region::Works => Edge::Slag,
        }
    }
}

/// A cell of the edge's core (the hedge, the wall, the drain, the bank), and of its foot.
const CORE: u8 = 1;
const FOOT: u8 = 2;
/// The diamond angle's full turn.
const TURN: i32 = 4096;
/// Cells either side of a way, a mark or a prop the edge keeps off: a gap is at least this wide
/// each side of what goes through it.
const CLEAR: i32 = 2;
/// The box flooded for the shut-in check reaches this far past the patch's radius.
const BOX_PAD: i32 = 12;
/// Gaps knocked before the edge is given up.
const KNOCKS: u32 = 48;
/// A pocket the edge shut of at most this many cells is grown over, not knocked through.
const POCKET: usize = 6;

/// The direction of diamond angle `pa` (`0..TURN`, a quarter turn a quadrant), as `(dx, dy)` of
/// length `TURN / 4`.
fn direction(pa: i32) -> (i32, i32) {
    let q = TURN / 4;
    let pa = pa.rem_euclid(TURN);
    let (k, p) = (pa / q, pa % q);
    let (dx, dy) = (q - p, p);
    let (dx, dy) = match k {
        0 => (dx, dy),
        1 => (-dy, dx),
        2 => (-dx, -dy),
        _ => (dy, -dx),
    };
    let len = isqrt((dx * dx + dy * dy) as u64).max(1) as i32;
    (dx * q / len, dy * q / len)
}

/// The cells of a line from `a` to `b`, eight-connected, both ends in.
fn line(a: (i32, i32), b: (i32, i32), out: &mut Vec<(i32, i32)>) {
    let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
    let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
    loop {
        out.push((x, y));
        if (x, y) == b {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// A cell's own roll, `0..256`.
fn roll(salt: u32, x: i32, y: i32) -> u32 {
    mix32(salt ^ (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1)) & 255
}

/// Ground an edge may be laid on: the land's own, nothing anyone built.
fn wild(t: Tile) -> bool {
    matches!(t, Tile::Grass | Tile::GrassTall | Tile::Dirt | Tile::Moss | Tile::DryBed | Tile::Bush)
}

/// A way's tile: whatever she walks along.
fn way_tile(t: Tile) -> bool {
    matches!(
        t,
        Tile::Road | Tile::Track | Tile::Rail | Tile::GrownPath | Tile::Cobble | Tile::Boardwalk | Tile::Stepping
    )
}

/// Every patch's edge, in the skeleton's order. Their record goes to `County::perimeters`.
pub fn lay_perimeters(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    for i in 0..c.sk.areas.len() {
        let a = c.sk.areas[i];
        let edge = edge_of(cat.name(a.def.id), a.def.region);
        let p = lay(c, a.row, edge, (centre(a.mx), centre(a.my)), i32::from(a.def.radius));
        c.perimeters.push(p);
    }
}

/// Over the box `bx` (a cell at `(y - bx.y) * bx.w + x - bx.x`): `keep`, the cells nothing may be
/// laid on or beside (under a prop, a mark, a unit, a named rect), and `blocked`, under a prop
/// that stops feet (`finish::blocked_by_props`). Nothing the edges lay moves a prop, a mark, a unit
/// or a rect, so a box's are the county's there whenever it is asked (PORT.md §13.3, phase 3:
/// the two whole-county planes were a megabyte).
fn keep_and_blocked(k: &crate::kit::Kit, bx: Rect) -> (Vec<bool>, Vec<bool>) {
    let cat = jane_data::catalog();
    let n = (bx.w * bx.h) as usize;
    let (mut keep, mut blocked) = (alloc::vec![false; n], alloc::vec![false; n]);
    let put = |plane: &mut Vec<bool>, r: Rect| {
        if let Some(r) = r.intersect(bx) {
            for (x, y) in r.cells() {
                plane[((y - bx.y) * bx.w + (x - bx.x)) as usize] = true;
            }
        }
    };
    let bp = k.blueprint();
    for p in &bp.props {
        let d = cat.story.prop(p.def);
        let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
        put(&mut keep, Rect::new(x, y, i32::from(d.w).max(1), i32::from(d.h).max(1)));
        if !(p.hidden || d.gate || !d.solid || d.push || d.carry) {
            put(&mut blocked, Rect::new(x, y, i32::from(d.w), i32::from(d.h)));
        }
    }
    for m in bp.marks.values() {
        put(&mut keep, Rect::new(i32::from(m.cell.x), i32::from(m.cell.y), 1, 1));
    }
    for u in &bp.units {
        put(&mut keep, Rect::new(i32::from(u.cell.x), i32::from(u.cell.y), 1, 1));
    }
    for r in bp.rects.values() {
        put(&mut keep, *r);
    }
    (keep, blocked)
}

/// The cells of the edge's line round `(cx, cy)`, each with whether it is a corner.
fn runs(rng: &mut jane_core::Sfc32, edge: Edge, (cx, cy): (i32, i32), radius: i32) -> Vec<((i32, i32), bool)> {
    let q = TURN / 4;
    // The corners: seven to ten, evenly round with a little give, each its own distance out.
    let n = 7 + rng.below(4) as i32;
    let turn0 = rng.below(TURN as u32) as i32;
    let corners: Vec<(i32, i32)> = (0..n)
        .map(|i| {
            let give = TURN / n / 4;
            let pa = turn0 + i * TURN / n + rng.range(-give, give);
            let r = radius * rng.range(78, 95) / 100;
            let (dx, dy) = direction(pa);
            (cx + dx * r / q, cy + dy * r / q)
        })
        .collect();
    // Three to five of the runs have a gateway.
    let mut gated: Vec<i32> = (0..n).collect();
    rng.shuffle(&mut gated);
    gated.truncate(3 + rng.below(3) as usize);
    let mut cells = Vec::new();
    let mut run: Vec<((i32, i32), bool)> = Vec::new();
    let mut seg = Vec::new();
    for i in 0..n {
        let (a, b) = (corners[i as usize], corners[((i + 1) % n) as usize]);
        // Square to the grid, as old field boundaries on a tithe map: along the run's long way to a
        // point somewhere in its middle, across, then on along the long way to the next corner.
        let long_x = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
        let t = rng.range(30, 71);
        let legs = if long_x {
            let sx = a.0 + (b.0 - a.0) * t / 100;
            [a, (sx, a.1), (sx, b.1), b]
        } else {
            let sy = a.1 + (b.1 - a.1) * t / 100;
            [a, (a.0, sy), (b.0, sy), b]
        };
        run.clear();
        for l in 0..3 {
            seg.clear();
            line(legs[l], legs[l + 1], &mut seg);
            seg.pop();
            let across = legs[l].0 == legs[l + 1].0;
            run.extend(seg.iter().map(|&c| (c, across)));
        }
        let deep = match edge {
            Edge::Wall => 1,
            Edge::Slag => 2,
            Edge::Hedge | Edge::Reed => 1 + rng.below(2) as i32,
        };
        // A gateway: four to six cells left out, somewhere in the run's middle half.
        let gate = gated.contains(&i).then(|| {
            let at = run.len() as i32 / 4 + rng.below((run.len() as u32 / 2).max(1)) as i32;
            (at, at + 4 + rng.below(3) as i32)
        });
        for (j, &((x, y), vertical)) in run.iter().enumerate() {
            if gate.is_some_and(|(g0, g1)| (g0..g1).contains(&(j as i32))) {
                continue;
            }
            cells.push(((x, y), j == 0));
            if deep == 2 {
                // The second row on the side away from the centre, so a corner closes.
                let out = if vertical { (x + (x - cx).signum(), y) } else { (x, y + (y - cy).signum()) };
                cells.push((out, j == 0));
            }
        }
    }
    cells
}

/// One patch's edge round `(cx, cy)`; `keep` and `blocked` are by county cell.
fn lay(c: &mut County<'_>, row: u8, edge: Edge, (cx, cy): (i32, i32), radius: i32) -> Perimeter {
    let mut rng = c.k.dice(Step::CountyPerimeter, i32::from(row), 0);
    let salt = rng.next_u32();
    let (w, h) = (c.k.w(), c.k.h());
    let mut out = Perimeter { row, edge: Some(edge), centre: (cx, cy), radius, ..Perimeter::default() };
    let cells = runs(&mut rng, edge, (cx, cy), radius);
    let reach = radius + BOX_PAD;
    let Some(bx) = Rect::new(cx - reach, cy - reach, 2 * reach + 1, 2 * reach + 1).intersect(Rect::new(0, 0, w, h))
    else {
        return out;
    };
    let (bw, bh) = (bx.w, bx.h);
    let local = |x: i32, y: i32| ((y - bx.y) * bw + (x - bx.x)) as usize;
    let inside = |x: i32, y: i32| x >= bx.x && y >= bx.y && x < bx.x + bw && y < bx.y + bh;
    // The box's tile ids, a row at a time (`read`), and its tiles as they were.
    let mut ids = vec![0u8; (bw * bh) as usize];
    let read = |k: &crate::kit::Kit, ids: &mut [u8]| {
        for (r, row) in ids.chunks_exact_mut(bw as usize).enumerate() {
            k.row_ids(bx.x, bx.y + r as i32, row);
        }
    };
    read(&c.k, &mut ids);
    let tiles_before: Vec<Tile> = ids.iter().map(|&id| Tile::from_id(id).unwrap_or(Tile::Void)).collect();
    let (keep, blocked) = keep_and_blocked(&c.k, bx);
    // Within `CLEAR` of a way or of anything kept: a square dilation, rows then columns.
    let mut near = vec![false; (bw * bh) as usize];
    for (x, y) in bx.cells() {
        let i = (y * w + x) as usize;
        near[local(x, y)] = c.trodden[i] || keep[local(x, y)] || way_tile(tiles_before[local(x, y)]);
    }
    // Each pass counts the set cells in a window sliding along the line rather than looking at
    // all of them for every cell: the same answer.
    let mut rows = vec![false; near.len()];
    for y in 0..bh {
        let at = |x: i32| (y * bw + x) as usize;
        let mut count = (0..CLEAR.min(bw)).filter(|&x| near[at(x)]).count();
        for x in 0..bw {
            if x + CLEAR < bw && near[at(x + CLEAR)] {
                count += 1;
            }
            if x - CLEAR > 0 && near[at(x - CLEAR - 1)] {
                count -= 1;
            }
            rows[at(x)] = count > 0;
        }
    }
    for x in 0..bw {
        let at = |y: i32| (y * bw + x) as usize;
        let mut count = (0..CLEAR.min(bh)).filter(|&y| rows[at(y)]).count();
        for y in 0..bh {
            if y + CLEAR < bh && rows[at(y + CLEAR)] {
                count += 1;
            }
            if y - CLEAR > 0 && rows[at(y - CLEAR - 1)] {
                count -= 1;
            }
            near[at(y)] = count > 0;
        }
    }
    let free = |c: &County<'_>, x: i32, y: i32| {
        inside(x, y) && wild(c.k.get(x, y)) && !c.k.is_claimed(x, y) && !near[local(x, y)]
    };
    // The core, then its foot along both sides.
    let mut laid: Vec<(i32, i32, Tile)> = Vec::new();
    let mut part = vec![0u8; near.len()];
    for &((x, y), corner) in &cells {
        if !free(c, x, y) || part[local(x, y)] != 0 {
            continue;
        }
        let r = roll(salt, x, y);
        let t = match (edge, corner) {
            (Edge::Hedge, true) => Tile::Tree,
            (Edge::Hedge, false) if r < 9 => Tile::Tree,
            (Edge::Hedge, false) => Tile::Hedge,
            (Edge::Wall | Edge::Reed, true) => Tile::Bush,
            (Edge::Wall, false) => Tile::StoneWall,
            (Edge::Reed, false) => Tile::Water,
            (Edge::Slag, _) if r < 220 => Tile::Rubble,
            (Edge::Slag, _) => Tile::DryBed,
        };
        // Scrub stays shut: the edge never opens ground.
        if c.k.solid(x, y) && t.flags() & F_SOLID == 0 {
            continue;
        }
        part[local(x, y)] = CORE;
        laid.push((x, y, t));
    }
    for k in 0..laid.len() {
        let (x, y, _) = laid[k];
        for (dx, dy) in DIRS4 {
            let (fx, fy) = (x + dx, y + dy);
            if !free(c, fx, fy) || part[local(fx, fy)] != 0 || c.k.solid(fx, fy) {
                continue;
            }
            part[local(fx, fy)] = FOOT;
            let r = roll(salt ^ 0x5bd1_e995, fx, fy);
            let t = match edge {
                Edge::Hedge if r < 150 => Tile::GrassTall,
                Edge::Wall if r < 60 => Tile::GrassTall,
                Edge::Hedge | Edge::Wall => continue,
                Edge::Reed if r < 16 => Tile::Bush,
                Edge::Reed if r < 210 => Tile::GrassTall,
                Edge::Reed => Tile::Moss,
                Edge::Slag if r < 150 => Tile::Dirt,
                Edge::Slag => Tile::DryBed,
            };
            laid.push((fx, fy, t));
        }
    }
    // Lay it, then knock gaps through wherever it shut something in.
    let border: Vec<(i32, i32)> =
        (0..bw).flat_map(|x| [(x, 0), (x, bh - 1)]).chain((1..bh - 1).flat_map(|y| [(0, y), (bw - 1, y)])).collect();
    let mut before = Fill::new();
    fill(bw as u32, bh as u32, &border, |i| tiles_before[i].flags() & F_SOLID == 0 && !blocked[i], &mut before);
    for &(x, y, t) in &laid {
        c.k.set(x, y, t);
    }
    // Nothing laid opens ground (the foot goes only on open ground, the core only on open ground
    // or scrub), so what is reached after is inside what was reached before, and something is shut
    // in exactly when fewer cells are reached than before less those the edge now stands on. Any
    // pocket so made borders the edge.
    let mut after = Fill::new();
    let mut handled = vec![false; near.len()];
    let mut grown = Vec::new();
    loop {
        let k = &c.k;
        read(k, &mut ids);
        let open = |id: u8| Tile::from_id(id).unwrap_or(Tile::Void).flags() & F_SOLID == 0;
        fill(bw as u32, bh as u32, &border, |i| open(ids[i]) && !blocked[i], &mut after);
        let stood_on =
            laid.iter().filter(|&&(x, y, _)| c.k.solid(x, y) && before.reached(x - bx.x, y - bx.y)).count() as u32;
        if after.count() + stood_on >= before.count() {
            break;
        }
        // Each pocket once, by the first stretch of edge in the order laid that has it against it.
        let shut_in = |x: i32, y: i32| {
            let (lx, ly) = (x - bx.x, y - bx.y);
            inside(x, y) && before.reached(lx, ly) && !after.reached(lx, ly) && !c.k.solid(x, y)
        };
        handled.fill(false);
        let mut knock = Vec::new();
        for &(x, y, t) in &laid {
            if c.k.get(x, y) != t || t.flags() & F_SOLID == 0 {
                continue;
            }
            for (dx, dy) in DIRS4 {
                let (sx, sy) = (x + dx, y + dy);
                if !shut_in(sx, sy) || handled[local(sx, sy)] {
                    continue;
                }
                handled[local(sx, sy)] = true;
                let mut pocket = vec![(sx, sy)];
                let mut k = 0;
                while k < pocket.len() {
                    let (px, py) = pocket[k];
                    k += 1;
                    for (ex, ey) in DIRS4 {
                        let (qx, qy) = (px + ex, py + ey);
                        if shut_in(qx, qy) && !handled[local(qx, qy)] {
                            handled[local(qx, qy)] = true;
                            pocket.push((qx, qy));
                        }
                    }
                }
                // A corner of a few cells nobody needs: the edge grows into it. Anything bigger
                // gets a gap.
                if pocket.len() <= POCKET && pocket.iter().all(|&(px, py)| free(c, px, py)) {
                    grown.extend(pocket);
                } else {
                    knock.push((x, y));
                }
            }
        }
        if knock.is_empty() && grown.is_empty() {
            out.gave_up = true;
            break;
        }
        if !grown.is_empty() {
            let t = match edge {
                Edge::Hedge => Tile::Hedge,
                Edge::Wall | Edge::Reed => Tile::Bush,
                Edge::Slag => Tile::Rubble,
            };
            for (x, y) in grown.drain(..) {
                c.k.set(x, y, t);
                part[local(x, y)] = CORE;
                laid.push((x, y, t));
            }
        }
        if knock.is_empty() {
            continue;
        }
        if out.knocked + knock.len() as u32 > KNOCKS {
            out.gave_up = true;
            break;
        }
        out.knocked += knock.len() as u32;
        // Everything laid within three cells of each is taken up.
        for &(x, y, t) in &laid {
            if knock.iter().any(|&(nx, ny)| (x - nx).abs() <= 3 && (y - ny).abs() <= 3) && c.k.get(x, y) == t {
                c.k.set(x, y, tiles_before[local(x, y)]);
            }
        }
    }
    if out.gave_up {
        for &(x, y, _) in &laid {
            c.k.set(x, y, tiles_before[local(x, y)]);
        }
        return out;
    }
    // What stands: the core for the record, and the ground of all of it nobody else's (the
    // scatter's rocks keep off it). Earth the edge laid is wild earth, as the slag's is.
    for &(x, y, t) in &laid {
        if c.k.get(x, y) != t {
            continue;
        }
        if t == Tile::Dirt {
            c.wild_earth.set((y * w + x) as usize, true);
        }
        if part[local(x, y)] == CORE {
            out.core.push((x, y));
        }
        c.k.claim(Rect::new(x, y, 1, 1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_direction_is_a_quarter_turn_long_and_goes_round() {
        let q = TURN / 4;
        let mut last: Option<(i32, i32)> = None;
        for pa in (0..TURN).step_by(64) {
            let (dx, dy) = direction(pa);
            let len = isqrt((dx * dx + dy * dy) as u64) as i32;
            assert!((len - q).abs() <= 2, "{pa}: {len}");
            if let Some((lx, ly)) = last {
                assert!(lx * dy - ly * dx > 0, "turns one way at {pa}");
            }
            last = Some((dx, dy));
        }
    }

    #[test]
    fn a_line_joins_its_ends_eight_ways() {
        let mut v = Vec::new();
        line((3, 4), (17, -2), &mut v);
        assert_eq!(v.first(), Some(&(3, 4)));
        assert_eq!(v.last(), Some(&(17, -2)));
        assert!(v.windows(2).all(|p| (p[0].0 - p[1].0).abs() <= 1 && (p[0].1 - p[1].1).abs() <= 1));
    }
}
