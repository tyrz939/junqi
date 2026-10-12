//! The macro railway (PLAN-map-r4 section 5.3): the main line on level-1 Lowfields, from the Halt's
//! macro cell out to the map's edge each way, as the macro cells `Skeleton.rail` holds. The county's
//! `lay_railway` lays it unchanged.

use alloc::collections::BinaryHeap;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;

use jane_core::ZoneId;

use super::CarveFail;
use super::blocks::Pad;
use crate::county::terraced::macro_plan::{CELLS, D4, MH, MW, MacroPlan, Reg, Wet, idx};
use crate::steps::{Step, dice};

const INF: i32 = i32::MAX;
/// The longest a leg runs (macro cells), and how far past the halt a line that ends in buffers goes.
pub const LEG_MAX: i32 = 40;
pub const TERMINUS: (i32, i32) = (6, 10);

/// May the line use macro cell `(x, y)`: level-1 Lowfields, dry, off every pad but the station's.
fn usable(plan: &MacroPlan, pads: &[Pad], x: i32, y: i32, station: usize) -> bool {
    if !(0..MW).contains(&x) || !(0..MH).contains(&y) {
        return false;
    }
    let i = idx(x, y);
    if plan.districts[usize::from(plan.district_of[i])].reg != Reg::Lowfields
        || plan.level_at(x, y) != 1
        || plan.wet[i] != Wet::Dry
    {
        return false;
    }
    let cell = super::Rect::new(x * 16, y * 16, 16, 16);
    pads.iter().enumerate().all(|(k, p)| k == station || !p.grown.overlaps(cell))
}

/// Dijkstra (16 an orthogonal step, 23 a diagonal) from `from` to the nearest macro cell on the map's
/// edge among those the line may use; the cells in order, or `None`.
fn to_edge(
    plan: &MacroPlan,
    pads: &[Pad],
    from: (i32, i32),
    first: (i32, i32),
    station: usize,
) -> Option<Vec<(i32, i32)>> {
    let edge = |x: i32, y: i32| x == 0 || y == 0 || x == MW - 1 || y == MH - 1;
    let mut dist = vec![INF; CELLS];
    let mut par = vec![u32::MAX; CELLS];
    let mut heap: BinaryHeap<Reverse<(i32, u32)>> = BinaryHeap::new();
    // The line leaves the halt along the platform's axis: its first step is `first`.
    let s = idx(from.0, from.1);
    dist[s] = 0;
    heap.push(Reverse((0, s as u32)));
    let mut goal = None;
    while let Some(Reverse((d, c))) = heap.pop() {
        let c = c as usize;
        if d > dist[c] {
            continue;
        }
        let (cx, cy) = ((c as i32) % MW, (c as i32) / MW);
        if c != s && edge(cx, cy) {
            goal = Some(c);
            break;
        }
        for (dx, dy) in D4 {
            if c == s && (dx, dy) != first {
                continue;
            }
            let (nx, ny) = (cx + dx, cy + dy);
            if !usable(plan, pads, nx, ny, station) {
                continue;
            }
            let n = idx(nx, ny);
            // A turn costs a little: the line is straight where it can be.
            let turn = i32::from(
                par[c] != u32::MAX && {
                    let p = par[c] as i32;
                    (cx - p % MW, cy - p / MW) != (dx, dy)
                },
            ) * 6;
            if d + 16 + turn < dist[n] {
                dist[n] = d + 16 + turn;
                par[n] = c as u32;
                heap.push(Reverse((dist[n], n as u32)));
            }
        }
    }
    let mut path = Vec::new();
    let mut q = goal? as u32;
    while q != u32::MAX {
        path.push(((q as i32) % MW, (q as i32) / MW));
        q = par[q as usize];
    }
    path.reverse();
    Some(path)
}

/// The macro cells past the halt on its axis a siding of `len` runs: as far as the ground allows.
fn stub(plan: &MacroPlan, pads: &[Pad], from: (i32, i32), dir: i32, len: i32, station: usize) -> Vec<(i32, i32)> {
    (1..=len)
        .map_while(|k| usable(plan, pads, from.0, from.1 + dir * k, station).then_some((from.0, from.1 + dir * k)))
        .collect()
}

/// The railway's macro cells in order, from one end through the halt to the other. Prefers two legs to
/// the map's edge (each at most [`LEG_MAX`] cells); else one leg and a terminus at buffers 6 to 10 cells
/// past the halt; and where the Lowfields give neither leg a way to the edge, a siding with buffers at
/// both ends (the plan's fallback taken twice: its own text has no third case, and re-rolling lost a
/// quarter of the plans). `station_pad` indexes the station's pad.
pub fn macro_rail(plan: &MacroPlan, pads: &[Pad], station_pad: usize) -> Result<Vec<(i32, i32)>, CarveFail> {
    let st = plan.site_cell("station");
    let s = (st.0, st.1);
    // The halt's rails run north-south (the chunk lays them down its west side): the line leaves by
    // the north and the south.
    let north = to_edge(plan, pads, s, (0, -1), station_pad).filter(|p| p.len() as i32 <= LEG_MAX);
    let south = to_edge(plan, pads, s, (0, 1), station_pad).filter(|p| p.len() as i32 <= LEG_MAX);
    let mut r = dice(plan.seed, ZoneId::County, Step::MapRail, plan.attempt, 0, 0);
    let (len_n, len_s) = (r.range(TERMINUS.0, TERMINUS.1), r.range(TERMINUS.0, TERMINUS.1));
    // One leg out and the siding the other way: the siding's far end, the halt, the leg.
    let with_leg = |leg: Vec<(i32, i32)>, dir: i32, len: i32| -> Result<Vec<(i32, i32)>, CarveFail> {
        let mut line: Vec<(i32, i32)> = stub(plan, pads, s, dir, len, station_pad).into_iter().rev().collect();
        line.push(s);
        line.extend(leg.into_iter().skip(1));
        if line.len() < 2 { Err(CarveFail::Rail) } else { Ok(line) }
    };
    match (north, south) {
        (Some(n), Some(so)) => {
            let mut line: Vec<(i32, i32)> = n.into_iter().rev().collect();
            line.extend(so.into_iter().skip(1));
            Ok(line)
        }
        (Some(leg), None) => with_leg(leg, 1, len_s),
        (None, Some(leg)) => with_leg(leg, -1, len_n),
        (None, None) => {
            let (up, down) = (stub(plan, pads, s, -1, len_n, station_pad), stub(plan, pads, s, 1, len_s, station_pad));
            if up.len() + down.len() < 2 {
                return Err(CarveFail::Rail);
            }
            let mut line: Vec<(i32, i32)> = up.into_iter().rev().collect();
            line.push(s);
            line.extend(down);
            Ok(line)
        }
    }
}
