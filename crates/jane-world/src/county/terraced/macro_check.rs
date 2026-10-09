//! MAP.md R0's checks on the macro graph: the districts are whole and their level steps are
//! single; each region border has 3 to 4 gates, two by road; every district meets its neighbours
//! through two openings and is on a loop; ledges point toward hubs and are real shortcuts; every
//! site is reachable from the Halt on foot honouring one-way ledges; and no pits: wherever she
//! can reach she can walk back (forward set within backward set).

use super::macro_plan::{CELLS, D4, GateKind, MH, MW, MacroPlan, Reg, Role, STEP_CELLS, idx};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Debug)]
pub struct Issue {
    pub code: &'static str,
    pub text: String,
}

fn issue(code: &'static str, text: String) -> Issue {
    Issue { code, text }
}

/// A ledge walk-back must cost at least this much more than the hop (cells), and at most 90 s of
/// walking at 7.5 cells a second.
pub const LEDGE_MIN_BACK: i32 = 40;
pub const LEDGE_MAX_BACK: i32 = 675;
/// A hub's ledge lands within this many macro cells (about 60 cells), squared.
pub const HUB_NEAR2: i32 = 25;

fn two_way(k: GateKind) -> bool {
    !matches!(k, GateKind::Ledge | GateKind::Verb(_))
}

pub fn check(p: &MacroPlan) -> Vec<Issue> {
    let mut out = Vec::new();
    // Districts: whole and big enough.
    for (i, d) in p.districts.iter().enumerate() {
        if d.area < 24 {
            out.push(issue("district_small", format!("{} has {} macro cells", d.role.name(), d.area)));
            continue;
        }
        let start = (0..CELLS).find(|&c| p.district_of[c] as usize == i);
        let Some(start) = start else { continue };
        let mut seen = vec![false; CELLS];
        let mut stack = vec![start];
        seen[start] = true;
        let mut n = 0;
        while let Some(c) = stack.pop() {
            n += 1;
            let (x, y) = ((c as i32) % MW, (c as i32) / MW);
            for (dx, dy) in D4 {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0
                    && ny >= 0
                    && nx < MW
                    && ny < MH
                    && !seen[idx(nx, ny)]
                    && p.district_of[idx(nx, ny)] as usize == i
                {
                    seen[idx(nx, ny)] = true;
                    stack.push(idx(nx, ny));
                }
            }
        }
        if n != d.area {
            out.push(issue("district_split", format!("{} is in pieces ({n} of {})", d.role.name(), d.area)));
        }
    }
    // Gates per region border (MAP.md 4.3): 3 to 4, two by road.
    for (ra, rb, name) in [
        (Reg::Lowfields, Reg::Works, "escarpment"),
        (Reg::Lowfields, Reg::Waters, "river valley"),
        (Reg::Waters, Reg::Works, "slag cliffs"),
    ] {
        let (mut n, mut roads) = (0, 0);
        for g in &p.gates {
            if g.kind == GateKind::Ledge || g.border != super::macro_plan::Border::Wall {
                continue;
            }
            let (x, y) = (p.districts[g.da as usize].reg, p.districts[g.db as usize].reg);
            if (x == ra && y == rb) || (x == rb && y == ra) {
                n += 1;
                roads += usize::from(g.road);
            }
        }
        if !(3..=4).contains(&n) || roads < 2 {
            out.push(issue("border_gates", format!("{name}: {n} gates, {roads} by road")));
        }
    }
    for d in &p.districts {
        if d.area > 2600 {
            out.push(issue("district_big", format!("{} is {} macro cells", d.role.name(), d.area)));
        }
        if d.role == Role::Crown && d.area < 60 {
            out.push(issue("crown_small", format!("the crown is only {} macro cells", d.area)));
        }
    }
    // Loops: every district has two ways in (two gates) unless it is a pocket by design.
    let mut degree = [0u8; 14];
    for g in &p.gates {
        if two_way(g.kind) {
            degree[g.da as usize] += 1;
            degree[g.db as usize] += 1;
        }
    }
    for (i, d) in p.districts.iter().enumerate() {
        if degree[i] < 2 && !d.role.pocket() {
            out.push(issue("dead_end", format!("{} has {} two-way gate(s)", d.role.name(), degree[i])));
        }
        if degree[i] == 0 {
            out.push(issue("no_gate", format!("{} has no gate at all", d.role.name())));
        }
    }
    let crown_ways =
        p.gates.iter().filter(|g| two_way(g.kind) && (g.da == Role::Crown.id() || g.db == Role::Crown.id())).count();
    if crown_ways != 1 {
        out.push(issue("crown_ways", format!("the crown has {crown_ways} two-way ways up (want exactly 1)")));
    }
    // Ledges: toward hubs, and shortcuts.
    let hubs: Vec<(i32, i32)> = ["julie_house", "town", "museum"].iter().map(|id| p.site_cell(id)).collect();
    let d2 = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1);
    let bfly = p.site_cell("butterfly_forest");
    let (two, _, _) = p.graphs(false);
    let mut served = [false; 3];
    for g in p.gates.iter().filter(|g| g.kind == GateKind::Ledge) {
        let (top, land) = ((i32::from(g.a.0), i32::from(g.a.1)), (i32::from(g.b.0), i32::from(g.b.1)));
        let (lt, ll) = (p.level_at(top.0, top.1), p.level_at(land.0, land.1));
        if lt <= ll {
            out.push(issue("ledge_up", format!("{} does not fall", g.name)));
        }
        if !hubs.iter().chain(core::iter::once(&bfly)).any(|&h| d2(land, h) < d2(top, h)) {
            out.push(issue("ledge_away", format!("{} points away from every hub", g.name)));
        }
        for (k, &h) in hubs.iter().enumerate() {
            if d2(land, h) <= HUB_NEAR2 {
                served[k] = true;
            }
        }
        let (dist, _) = p.dijkstra(&two, idx(land.0, land.1), Some(idx(top.0, top.1)));
        let back = dist[idx(top.0, top.1)];
        if !(STEP_CELLS + LEDGE_MIN_BACK..=LEDGE_MAX_BACK).contains(&back) {
            out.push(issue("ledge_walk", format!("{}: walk back {back} cells", g.name)));
        }
    }
    for (k, id) in ["julie_house", "town", "museum"].iter().enumerate() {
        if !served[k] {
            out.push(issue("hub_ledge", format!("no ledge lands within 60 cells of {id}")));
        }
    }
    if !p.gates.iter().any(|g| g.kind == GateKind::Ledge && g.da == Role::Foothills.id()) {
        out.push(issue("mine_ledge", String::from("no ledge from the foothills toward Julie's")));
    }
    // Reach and no pits.
    let (fwd, rev, _) = p.graphs(true);
    let st = p.site_cell("station");
    let (f, _) = p.dijkstra(&fwd, idx(st.0, st.1), None);
    let (b, _) = p.dijkstra(&rev, idx(st.0, st.1), None);
    for s in &p.sites {
        if f[idx(i32::from(s.pos.0), i32::from(s.pos.1))] == i32::MAX {
            out.push(issue("unreachable", format!("{} cannot be walked to from the Halt", s.name)));
        }
        if p.district_of[idx(i32::from(s.pos.0), i32::from(s.pos.1))] != s.district {
            out.push(issue("site_district", format!("{} is not in its district", s.name)));
        }
    }
    let pits = (0..CELLS).filter(|&c| f[c] != i32::MAX && b[c] == i32::MAX).count();
    if pits > 0 {
        out.push(issue("pit", format!("{pits} macro cells can be reached but not left for the Halt")));
    }
    let lost = (0..CELLS).filter(|&c| f[c] == i32::MAX).count();
    if lost > 0 {
        out.push(issue("unreached", format!("{lost} macro cells cannot be reached on foot")));
    }
    story(p, &mut out);
    if p.routes.len() != 13 {
        out.push(issue("routes", format!("{} of 13 routes found", p.routes.len())));
    }
    out
}

/// How far (macro cells, Chebyshev) a site stands from the nearest route point.
pub fn off_route(p: &MacroPlan, id: &str) -> i32 {
    let c = p.site_cell(id);
    p.routes
        .iter()
        .flat_map(|r| r.pts.iter())
        .map(|q| (i32::from(q.0) - c.0).abs().max((i32::from(q.1) - c.1).abs()))
        .min()
        .unwrap_or(i32::MAX)
}

/// Road time in seconds: macro length at 7.5 cells a second, times 1.25 for the road's wind.
pub fn secs(p: &MacroPlan, from: &str, to: &str) -> i32 {
    p.routes.iter().find(|r| r.from == from && r.to == to).map_or(-1, |r| r.cells / 6)
}

/// What the story needs of any arrangement (MAP.md R0, the owner's rule): regions of a size, the
/// School alone on the highest ground and seen from the Halt, Julie's and the Castle, a clear first
/// walk of today's length on one level, the mine's and the School's times in their bands, and the
/// dungeons in the story's order along the roads.
fn story(p: &MacroPlan, out: &mut Vec<Issue>) {
    for (r, name) in [(Reg::Lowfields, "Lowfields"), (Reg::Waters, "Waters"), (Reg::Works, "Works")] {
        let n: u32 = p.districts.iter().filter(|d| d.reg == r).map(|d| d.area).sum();
        if n < 1100 {
            out.push(issue("region_small", format!("{name} is only {n} macro cells")));
        }
    }
    if p.districts.iter().filter(|d| d.level == 3).count() != 1
        || p.site("school").is_none_or(|s| s.district != Role::Crown.id())
    {
        out.push(issue("school_crown", String::from("the School is not alone on the crown")));
    }
    let school = p.site_cell("school");
    for id in ["station", "julie_house", "town"] {
        let c = p.site_cell(id);
        let d2 = (c.0 - school.0) * (c.0 - school.0) + (c.1 - school.1) * (c.1 - school.1);
        if d2 > 80 * 80 {
            out.push(issue(
                "school_unseen",
                format!("the School is {} macro cells from {id} (the silhouette reaches 80)", isqrt(d2)),
            ));
        }
    }
    // The first walk: a road on one district (no face crossed, no gate), 45 to 150 s.
    let first = secs(p, "station", "julie_house");
    if !(45..=150).contains(&first) {
        out.push(issue("first_walk", format!("Halt to Julie's takes {first} s (want 45 to 150)")));
    }
    if let Some(r) = p.routes.iter().find(|r| r.from == "station" && r.to == "julie_house") {
        let d0 = p.district_of[idx(i32::from(r.pts[0].0), i32::from(r.pts[0].1))];
        if r.pts.iter().any(|q| p.district_of[idx(i32::from(q.0), i32::from(q.1))] != d0) {
            out.push(issue("first_walk_gate", String::from("the first walk leaves its district")));
        }
    }
    let mine = first + secs(p, "julie_house", "town") + secs(p, "town", "gold_mine");
    if !(240..=540).contains(&mine) {
        out.push(issue("mine_time", format!("Halt to the mine mouth by road takes {mine} s (want 240 to 540)")));
    }
    let to_school = first
        + secs(p, "julie_house", "town")
        + secs(p, "town", "graveyard")
        + secs(p, "graveyard", "factory")
        + secs(p, "factory", "school");
    if to_school + 60 < mine || secs(p, "town", "graveyard") <= 0 || to_school > 900 {
        out.push(issue(
            "school_order",
            format!("the School ({to_school} s) is not the last of the dungeons (mine {mine} s)"),
        ));
    }
}

fn isqrt(n: i32) -> i32 {
    let mut r = 0;
    while (r + 1) * (r + 1) <= n {
        r += 1;
    }
    r
}
