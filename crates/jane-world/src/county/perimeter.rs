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
//! | **Reed edge** | the Waters | a drain one to two wide, reed along both banks, a clump of willow scrub; cut through a wood, the trees on its banks felled so it shows |
//! | **Slag bank** | the Works | a cinder ridge swelling from one cell wide to three, its slag in clumps on dark ground, thinning to a few stones at a gap; the odd lump rolled off its foot |
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
    let (w, h) = (c.k.w(), c.k.h());
    // Cells nothing may be laid on or beside: under a prop, a mark, a unit, a named rect.
    let mut keep = vec![false; (w * h) as usize];
    {
        let bp = c.k.blueprint();
        let mut put = |x0: i32, y0: i32, x1: i32, y1: i32| {
            for y in y0.max(0)..=y1.min(h - 1) {
                let row = (y * w) as usize;
                keep[row + x0.max(0) as usize..=row + x1.min(w - 1) as usize].fill(true);
            }
        };
        for p in &bp.props {
            let d = cat.story.prop(p.def);
            let (x, y) = (i32::from(p.cell.x), i32::from(p.cell.y));
            put(x, y, x + i32::from(d.w).max(1) - 1, y + i32::from(d.h).max(1) - 1);
        }
        for m in bp.marks.values() {
            put(i32::from(m.cell.x), i32::from(m.cell.y), i32::from(m.cell.x), i32::from(m.cell.y));
        }
        for u in &bp.units {
            put(i32::from(u.cell.x), i32::from(u.cell.y), i32::from(u.cell.x), i32::from(u.cell.y));
        }
        for r in bp.rects.values() {
            put(r.x, r.y, r.x + r.w - 1, r.y + r.h - 1);
        }
    }
    let blocked = super::finish::blocked_by_props(&c.k);
    for i in 0..c.sk.areas.len() {
        let a = c.sk.areas[i];
        let edge = edge_of(cat.name(a.def.id), a.def.region);
        let p = lay(c, &keep, &blocked, a.row, edge, (centre(a.mx), centre(a.my)), i32::from(a.def.radius));
        c.perimeters.push(p);
    }
}

/// A cell of the edge's line.
#[derive(Clone, Copy, Debug)]
struct Spot {
    at: (i32, i32),
    /// A run's first cell: where two runs meet.
    corner: bool,
    /// Within a few cells of a gateway: a bank thins out to a few stones there.
    taper: bool,
}

/// A slag bank's reach either side of its line along a run, `0..=1` cells: smooth along the run
/// (a lattice every [`BANK_LUMP`] cells), so the bank swells and narrows from one to three wide.
fn bank_reach(salt: u32, j: i32) -> i32 {
    let (k, f) = (j / BANK_LUMP, j % BANK_LUMP);
    let v = |k: i32| (mix32(salt ^ (k as u32).wrapping_mul(0x9e37_79b9)) & 255) as i32;
    let at = (v(k) * (BANK_LUMP - f) + v(k + 1) * f) / BANK_LUMP;
    i32::from(at >= 120)
}

/// Cells along a run between a slag bank's swells.
const BANK_LUMP: i32 = 5;
/// Cells either side of a gateway a slag bank tapers over.
const TAPER: i32 = 3;

/// The cells of the edge's line round `(cx, cy)`.
fn runs(rng: &mut jane_core::Sfc32, edge: Edge, (cx, cy): (i32, i32), radius: i32) -> Vec<Spot> {
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
    let (bank_in, bank_out) = (rng.next_u32(), rng.next_u32());
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
            Edge::Wall | Edge::Slag => 1,
            Edge::Hedge | Edge::Reed => 1 + rng.below(2) as i32,
        };
        let j0 = cells.len() as i32;
        // A gateway: four to six cells left out, somewhere in the run's middle half.
        let gate = gated.contains(&i).then(|| {
            let at = run.len() as i32 / 4 + rng.below((run.len() as u32 / 2).max(1)) as i32;
            (at, at + 4 + rng.below(3) as i32)
        });
        for (j, &((x, y), vertical)) in run.iter().enumerate() {
            let j = j as i32;
            if gate.is_some_and(|(g0, g1)| (g0..g1).contains(&j)) {
                continue;
            }
            let corner = j == 0;
            let taper = gate.is_some_and(|(g0, g1)| (g0 - TAPER..g1 + TAPER).contains(&j));
            cells.push(Spot { at: (x, y), corner, taper });
            // Away from the centre, and towards it.
            let (ox, oy) = if vertical { ((x - cx).signum(), 0) } else { (0, (y - cy).signum()) };
            let (outer, inner) = match edge {
                // A bank swells and narrows on each side on its own.
                Edge::Slag => (bank_reach(bank_out, j0 + j), bank_reach(bank_in, j0 + j)),
                // The second row on the side away from the centre, so a corner closes.
                _ => (deep - 1, 0),
            };
            for k in 1..=outer {
                cells.push(Spot { at: (x + ox * k, y + oy * k), corner, taper });
            }
            for k in 1..=inner {
                cells.push(Spot { at: (x - ox * k, y - oy * k), corner, taper });
            }
        }
    }
    cells
}

/// One patch's edge round `(cx, cy)`; `keep` and `blocked` are by county cell.
fn lay(
    c: &mut County<'_>,
    keep: &[bool],
    blocked: &[bool],
    row: u8,
    edge: Edge,
    (cx, cy): (i32, i32),
    radius: i32,
) -> Perimeter {
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
    let mut tiles_before: Vec<Tile> = bx.cells().map(|(x, y)| c.k.get(x, y)).collect();
    // Within `CLEAR` of a way or of anything kept: a square dilation, rows then columns.
    let mut near = vec![false; (bw * bh) as usize];
    for (x, y) in bx.cells() {
        let i = (y * w + x) as usize;
        near[local(x, y)] = c.trodden[i] || keep[i] || way_tile(tiles_before[local(x, y)]);
    }
    let mut rows = vec![false; near.len()];
    for y in 0..bh {
        for x in 0..bw {
            rows[(y * bw + x) as usize] =
                ((x - CLEAR).max(0)..=(x + CLEAR).min(bw - 1)).any(|xx| near[(y * bw + xx) as usize]);
        }
    }
    for y in 0..bh {
        for x in 0..bw {
            near[(y * bw + x) as usize] =
                ((y - CLEAR).max(0)..=(y + CLEAR).min(bh - 1)).any(|yy| rows[(yy * bw + x) as usize]);
        }
    }
    // A drain through a wood is cut: the trees on its line, beside it and just south of it (whose
    // crowns would be drawn over it) are felled first, so it shows. Done before anything is
    // flooded, so the felling is the ground the edge is judged on.
    if edge == Edge::Reed {
        for s in &cells {
            let (x, y) = s.at;
            if !inside(x, y) || near[local(x, y)] || c.k.is_claimed(x, y) {
                continue;
            }
            for dy in -1..=2 {
                for dx in -1..=1 {
                    let (tx, ty) = (x + dx, y + dy);
                    if inside(tx, ty)
                        && !near[local(tx, ty)]
                        && !c.k.is_claimed(tx, ty)
                        && c.k.get(tx, ty) == Tile::Tree
                    {
                        c.k.set(tx, ty, Tile::GrassTall);
                        tiles_before[local(tx, ty)] = Tile::GrassTall;
                    }
                }
            }
        }
    }
    let free = |c: &County<'_>, x: i32, y: i32| {
        inside(x, y) && wild(c.k.get(x, y)) && !c.k.is_claimed(x, y) && !near[local(x, y)]
    };
    // The core, then its foot along both sides.
    let mut laid: Vec<(i32, i32, Tile)> = Vec::new();
    let mut part = vec![0u8; near.len()];
    // Close to a way (inside `CLEAR + 2` of it) a bank thins out as at a gateway.
    let by_way = |x: i32, y: i32| {
        [(-2, -2), (0, -2), (2, -2), (-2, 0), (2, 0), (-2, 2), (0, 2), (2, 2)]
            .iter()
            .any(|&(dx, dy)| !inside(x + dx, y + dy) || near[local(x + dx, y + dy)])
    };
    for &Spot { at: (x, y), corner, taper } in &cells {
        if !free(c, x, y) || part[local(x, y)] != 0 {
            continue;
        }
        let r = roll(salt, x, y);
        // A bank's slag lies in clumps: a coarse roll every three cells decides where.
        let clump = roll(salt ^ 0x68e3_1da4, x.div_euclid(3), y.div_euclid(3));
        let thin = taper || by_way(x, y);
        let t = match (edge, corner) {
            (Edge::Hedge, true) => Tile::Tree,
            (Edge::Hedge, false) if r < 9 => Tile::Tree,
            (Edge::Hedge, false) => Tile::Hedge,
            (Edge::Wall | Edge::Reed, true) => Tile::Bush,
            (Edge::Wall, false) => Tile::StoneWall,
            (Edge::Reed, false) => Tile::Water,
            (Edge::Slag, _) if thin && r < 40 => Tile::Rubble,
            (Edge::Slag, _) if !thin && (r < 48 || (clump > 100 && r < 215)) => Tile::Rubble,
            (Edge::Slag, _) if r % 4 == 0 => Tile::Dirt,
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
                Edge::Reed if r < 16 => Tile::Bush,
                Edge::Reed if r < 210 => Tile::GrassTall,
                Edge::Reed => Tile::Moss,
                // The odd lump rolled off the bank's foot.
                Edge::Slag if r < 9 => Tile::Rubble,
                Edge::Slag if r < 130 => Tile::Dirt,
                Edge::Slag if r < 200 => Tile::DryBed,
                Edge::Hedge | Edge::Wall | Edge::Slag => continue,
            };
            laid.push((fx, fy, t));
        }
    }
    // Lay it, then knock gaps through wherever it shut something in.
    let bx_ix = |i: usize| ((bx.y + i as i32 / bw) * w + bx.x + i as i32 % bw) as usize;
    let border: Vec<(i32, i32)> =
        (0..bw).flat_map(|x| [(x, 0), (x, bh - 1)]).chain((1..bh - 1).flat_map(|y| [(0, y), (bw - 1, y)])).collect();
    let mut before = Fill::new();
    fill(bw as u32, bh as u32, &border, |i| tiles_before[i].flags() & F_SOLID == 0 && !blocked[bx_ix(i)], &mut before);
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
        let tiles = c.k.tiles().as_slice();
        fill(
            bw as u32,
            bh as u32,
            &border,
            |i| tiles[bx_ix(i)].flags() & F_SOLID == 0 && !blocked[bx_ix(i)],
            &mut after,
        );
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
            c.wild_earth[(y * w + x) as usize] = true;
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
