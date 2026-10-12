//! The roads' waypoints (PLAN-map-r4 section 5.1) and the night gates (MAP.md 7.1). A road is the plan's
//! macro route turned into cell points: the macro cells' centres, with the foot and top of each ramp
//! it takes and the two end points of each deck it crosses put in order between them, and the legs that
//! must run straight (the ramps and the decks with their approaches) marked.

use alloc::vec::Vec;
use jane_core::Rect;
use jane_core::blueprint::Span;

use super::CarveFail;
use super::joins::{Join, JoinKind};
use super::spans::SpanMeta;
use crate::county::terraced::macro_plan::{GateKind, MacroPlan, Reg, STEP_CELLS};
use crate::steps::{Step, dice};
use jane_core::ZoneId;

/// A route as cell points, and the point ranges (first, last index) laid as straight legs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadWay {
    pub route: u8,
    pub pts: Vec<(i32, i32)>,
    pub straight: Vec<(u16, u16)>,
}

fn centre(c: (u8, u8)) -> (i32, i32) {
    (i32::from(c.0) * STEP_CELLS + STEP_CELLS / 2, i32::from(c.1) * STEP_CELLS + STEP_CELLS / 2)
}

fn d1(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Two points in the order a road walking from `from` toward `to` meets them.
fn ordered(from: (i32, i32), to: (i32, i32), p: (i32, i32), q: (i32, i32)) -> [(i32, i32); 2] {
    if d1(from, p) + d1(to, q) <= d1(from, q) + d1(to, p) { [p, q] } else { [q, p] }
}

/// The waypoints of every route of the plan over the carved joins, spans and planks.
pub fn road_ways(
    plan: &MacroPlan,
    joins: &[Join],
    meta: &[SpanMeta],
    _spans: &[Span],
    planks: &[Rect],
) -> Vec<RoadWay> {
    let mut out = Vec::new();
    for (ri, route) in plan.routes.iter().enumerate() {
        let mut pts: Vec<(i32, i32)> = Vec::new();
        let mut straight: Vec<(u16, u16)> = Vec::new();
        for (k, &q) in route.pts.iter().enumerate() {
            let here = centre(q);
            if let Some(&prev) = route.pts.get(k.wrapping_sub(1)).filter(|_| k > 0) {
                let from = centre(prev);
                // A gate between the two macro cells: its ramp, or its span's ends.
                let gi = plan.gates.iter().position(|g| g.road && ((g.a, g.b) == (prev, q) || (g.a, g.b) == (q, prev)));
                let mut laid: Option<[(i32, i32); 2]> = None;
                if let Some(gi) = gi {
                    if let Some(j) = joins.iter().find(|j| j.gate == Some(gi as u8) && matches!(j.kind, JoinKind::Ramp))
                    {
                        laid = Some(ordered(from, here, j.foot, j.top));
                    } else if let Some(c) = plan.crossings.iter().position(|c| c.gate == Some(gi as u8)) {
                        if let Some(m) = meta.iter().find(|m| usize::from(m.crossing) == c) {
                            laid = Some(ordered(from, here, m.ends[0], m.ends[1]));
                        }
                    }
                }
                // A road bridge or plank where the route crosses the river at `q`.
                if laid.is_none() {
                    if let Some(c) = plan.crossings.iter().position(|c| c.route == Some(ri as u8) && c.at == q) {
                        if let Some(m) = meta.iter().find(|m| usize::from(m.crossing) == c) {
                            laid = Some(ordered(from, here, m.ends[0], m.ends[1]));
                        } else if let Some(r) = planks
                            .iter()
                            .min_by_key(|r| d1(r.centre(), here))
                            .filter(|r| d1(r.centre(), here) <= 3 * STEP_CELLS)
                        {
                            // Boards, no span: the road runs straight over them, six cells either side.
                            let ends = if r.w >= r.h {
                                let y = r.y + r.h / 2;
                                [(r.x - 6, y), (r.right() + 5, y)]
                            } else {
                                let x = r.x + r.w / 2;
                                [(x, r.y - 6), (x, r.bottom() + 5)]
                            };
                            laid = Some(ordered(from, here, ends[0], ends[1]));
                        }
                    }
                }
                if let Some(pair) = laid {
                    let first = pts.len() as u16;
                    pts.extend(pair);
                    straight.push((first, first + 1));
                }
            }
            pts.push(here);
        }
        out.push(RoadWay { route: ri as u8, pts, straight });
    }
    out
}

/// The two gates the night's barriers close (N3): road gates of a region border, built, never the
/// Cutting, and never one whose border would keep fewer than two ways open. Their indices in
/// `plan.gates`; `Ok` with fewer than two if the plan has no more to give.
pub fn night_gates(plan: &MacroPlan, joins: &[Join], spans: &[Span], meta: &[SpanMeta]) -> Result<Vec<u8>, CarveFail> {
    let border = |gi: usize| -> Option<(Reg, Reg)> {
        let g = &plan.gates[gi];
        let (a, b) = (plan.districts[usize::from(g.da)].reg, plan.districts[usize::from(g.db)].reg);
        (a != b).then_some(if (a as u8) < (b as u8) { (a, b) } else { (b, a) })
    };
    // The gates that are open ways: a built ramp, stair or stones, or a whole span.
    let built = |gi: usize| -> bool {
        joins
            .iter()
            .any(|j| j.gate == Some(gi as u8) && matches!(j.kind, JoinKind::Ramp | JoinKind::Stair | JoinKind::Stones))
            || plan.crossings.iter().enumerate().any(|(c, x)| {
                x.gate == Some(gi as u8) && meta.iter().any(|m| usize::from(m.crossing) == c) && !spans.is_empty()
            })
    };
    let mut candidates: Vec<usize> = Vec::new();
    for (gi, g) in plan.gates.iter().enumerate() {
        if border(gi).is_none() || !g.road || g.name == "The Cutting" || !built(gi) {
            continue;
        }
        candidates.push(gi);
    }
    // Not on the first walk: a gate's cells off the routes from the Halt to Julie's and to the Castle.
    let first_walk: Vec<(u8, u8)> = plan
        .routes
        .iter()
        .filter(|r| (r.from, r.to) == ("station", "julie_house") || (r.from, r.to) == ("julie_house", "town"))
        .flat_map(|r| r.pts.iter().copied())
        .collect();
    candidates.retain(|&gi| {
        let g = &plan.gates[gi];
        !first_walk.contains(&g.a) && !first_walk.contains(&g.b)
    });
    let mut rng = dice(plan.seed, ZoneId::County, Step::MapRoad, plan.attempt, 99, 0);
    rng.shuffle(&mut candidates);
    let mut chosen: Vec<u8> = Vec::new();
    for gi in candidates {
        let bd = border(gi);
        let open_after = (0..plan.gates.len())
            .filter(|&o| o != gi && border(o) == bd && built(o) && !chosen.iter().any(|&c| usize::from(c) == o))
            .count();
        let same_border = chosen.iter().any(|&c| border(usize::from(c)) == bd);
        if open_after < 2 || same_border {
            continue;
        }
        // With it shut the macro graph must still reach every site and have no pit.
        let mut shut: Vec<usize> = chosen.iter().map(|&c| usize::from(c)).collect();
        shut.push(gi);
        if macro_ok(plan, &shut) {
            chosen.push(gi as u8);
            if chosen.len() == 2 {
                break;
            }
        }
    }
    Ok(chosen)
}

/// The plan's walking graph with the gates `shut` closed: forward and reverse edges, as `plan.graphs`
/// makes them (verb gates shut, ledges one way).
fn macro_graphs(plan: &MacroPlan, shut: &[usize]) -> (Vec<Vec<u16>>, Vec<Vec<u16>>) {
    use crate::county::terraced::macro_plan::{CELLS, idx};
    let mut fwd = alloc::vec![Vec::new(); CELLS];
    let mut rev = alloc::vec![Vec::new(); CELLS];
    for (gi, g) in plan.gates.iter().enumerate() {
        if matches!(g.kind, GateKind::Verb(_)) || shut.contains(&gi) {
            continue;
        }
        let a = idx(i32::from(g.a.0), i32::from(g.a.1));
        let b = idx(i32::from(g.b.0), i32::from(g.b.1));
        fwd[a].push(b as u16);
        rev[b].push(a as u16);
        if g.kind != GateKind::Ledge {
            fwd[b].push(a as u16);
            rev[a].push(b as u16);
        }
    }
    (fwd, rev)
}

/// Every site reachable from the Halt and no pit on the macro graph with the gates `shut` closed.
fn macro_ok(plan: &MacroPlan, shut: &[usize]) -> bool {
    use crate::county::terraced::macro_plan::{CELLS, idx};
    let (fwd, rev) = macro_graphs(plan, shut);
    let st = plan.site_cell("station");
    let src = idx(st.0, st.1);
    let (f, _) = plan.dijkstra(&fwd, src, None);
    let (r, _) = plan.dijkstra(&rev, src, None);
    plan.sites.iter().all(|s| f[idx(i32::from(s.pos.0), i32::from(s.pos.1))] != i32::MAX)
        && (0..CELLS).all(|c| plan.wet[c].body() || f[c] == i32::MAX || r[c] != i32::MAX)
}
