//! The macro plan: MAP.md section 4.1's K1 to K5 on a 96 x 96 grid of 16-cell macro cells
//! (1,536 cells square), integer only, `no_std` plus `alloc`. The county's is authored, not rolled:
//! `data/world/county/plan.txt` ([`super::source`]).
//!
//! K1/K2: the three regions (the Works across the north, the river valley between the Lowfields
//! and the Waters) cut into 14 districts, each with its level (MAP.md 4.2). K3: the gate graph: every
//! border open, joined or wall, with its gates typed (4.3), ledges toward hubs (4.5) and verb gates.
//! K4: the story's sites. K5: roads over road-carrying gates only.

use crate::skeleton::Biome;
use alloc::collections::{BinaryHeap, VecDeque};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;
use jane_core::blueprint::SpanKind;

/// The macro grid, in macro cells, and the cells in one.
pub const MW: i32 = 96;
pub const MH: i32 = 96;
pub const CELLS: usize = (MW * MH) as usize;
pub const STEP_CELLS: i32 = 16;
/// A body cell's room: at least this (two dry cells of its own district, or the map edge).
pub const SHORE: i32 = 3;
/// The largest a pond may be, in macro cells.
pub const POND_MAX: i32 = 6;

const INF: i32 = i32::MAX;
pub const D4: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reg {
    Lowfields,
    Waters,
    Works,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Role {
    StationFields,
    SallowBottom,
    TopField,
    CastleTerrace,
    Foothills,
    MuseumTerrace,
    LibraryTerrace,
    ButterflyHollow,
    Lake,
    Crown,
    Lip,
    Cutting,
    Plateau,
    Slagmere,
}

/// Every district, by id (a district's id is its index here).
pub const ROLES: [Role; 14] = [
    Role::StationFields,
    Role::SallowBottom,
    Role::TopField,
    Role::CastleTerrace,
    Role::Foothills,
    Role::MuseumTerrace,
    Role::LibraryTerrace,
    Role::ButterflyHollow,
    Role::Lake,
    Role::Crown,
    Role::Lip,
    Role::Cutting,
    Role::Plateau,
    Role::Slagmere,
];

impl Role {
    pub const fn id(self) -> u8 {
        self as u8
    }
    /// (region, level, name, seed x, seed y)
    const fn def(self) -> (Reg, u8, &'static str, i32, i32) {
        match self {
            Role::StationFields => (Reg::Lowfields, 1, "Station Fields", 14, 66),
            Role::SallowBottom => (Reg::Lowfields, 0, "Sallow Bottom", 38, 72),
            Role::TopField => (Reg::Lowfields, 2, "Top Field", 28, 62),
            Role::CastleTerrace => (Reg::Lowfields, 1, "Castle Terrace", 42, 48),
            Role::Foothills => (Reg::Lowfields, 2, "Foothills", 42, 90),
            Role::MuseumTerrace => (Reg::Waters, 1, "Museum Terrace", 68, 64),
            Role::LibraryTerrace => (Reg::Waters, 1, "Library Terrace", 82, 48),
            Role::ButterflyHollow => (Reg::Waters, 0, "Butterfly Hollow", 90, 38),
            Role::Lake => (Reg::Waters, 0, "The Lake", 82, 86),
            Role::Crown => (Reg::Works, 3, "The Crown", 47, 11),
            Role::Lip => (Reg::Works, 2, "Escarpment Lip", 30, 28),
            Role::Cutting => (Reg::Works, 1, "The Burial Cutting", 20, 18),
            Role::Plateau => (Reg::Works, 2, "Works Plateau", 68, 19),
            Role::Slagmere => (Reg::Works, 1, "Slag Mere", 87, 13),
        }
    }
    pub const fn reg(self) -> Reg {
        self.def().0
    }
    pub const fn level(self) -> u8 {
        self.def().1
    }
    pub const fn name(self) -> &'static str {
        self.def().2
    }
    /// A pocket by design (MAP.md 4.5): one way in, not a dead-end fault.
    pub const fn pocket(self) -> bool {
        matches!(self, Role::Crown | Role::ButterflyHollow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Repair,
    Grow,
    Explosion,
    Electric,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    RampRoad,
    Stair,
    /// A bridge, viaduct or walkway: two layers, walkable along its deck.
    Span,
    Underpass,
    CompanyGate,
    SteppingStones,
    /// Closed until the verb is learned; never the only way (MAP.md 4.5).
    Verb(Verb),
    /// An opening in a hedge or wood edge between districts of one level.
    Gap,
    /// One-way: `a` is the top, `b` the landing.
    Ledge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    Open,
    Joined,
    Wall,
}

#[derive(Clone, Debug)]
pub struct Gate {
    pub kind: GateKind,
    pub name: &'static str,
    /// The cell on the first district's side (a ledge's top) and the cell across (its landing).
    pub a: (u8, u8),
    pub b: (u8, u8),
    pub da: u8,
    pub db: u8,
    /// Carries a road (a road never takes a stair or a ledge).
    pub road: bool,
    pub border: Border,
}

impl Gate {
    /// A ledge's hop direction, one of the four unit steps.
    pub fn dir(&self) -> (i32, i32) {
        (i32::from(self.b.0) - i32::from(self.a.0), i32::from(self.b.1) - i32::from(self.a.1))
    }
}

#[derive(Clone, Debug)]
pub struct District {
    pub role: Role,
    pub reg: Reg,
    pub level: u8,
    pub seed: (i32, i32),
    pub area: u32,
    /// The ground it is dressed in: one of the game's own biomes, drawn per seed within what its place allows.
    pub biome: Biome,
}

#[derive(Clone, Debug)]
pub struct Site {
    pub id: &'static str,
    pub name: &'static str,
    pub district: u8,
    pub pos: (u8, u8),
    /// A dungeon's way in (the mine, the burial, the School).
    pub dungeon: bool,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub from: &'static str,
    pub to: &'static str,
    pub pts: Vec<(u8, u8)>,
    /// Length in cells (16 a macro step).
    pub cells: i32,
}

/// Water on a macro cell (MAP.md 2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Wet {
    Dry,
    River,
    Lake,
    Mere,
    Pond,
}

impl Wet {
    /// Standing water: impassable at macro scale.
    pub const fn body(self) -> bool {
        matches!(self, Wet::Lake | Wet::Mere | Wet::Pond)
    }
}

/// What a crossing goes over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Over {
    River,
    Valley,
    Cutting,
    Yard,
}

/// A span's place in the plan: the data R4 turns into a `blueprint::Span`.
#[derive(Clone, Debug)]
pub struct Crossing {
    pub at: (u8, u8),
    /// The deck's axis (the way feet go over it).
    pub along_x: bool,
    pub kind: SpanKind,
    pub over: Over,
    /// Index into `routes`, for a road bridge.
    pub route: Option<u8>,
    /// Index into `gates`, for a span gate.
    pub gate: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct MacroPlan {
    pub seed: u32,
    /// The attempt that passed (0 is the first roll); a plan that never passed carries the last.
    pub attempt: u8,
    pub district_of: Vec<u8>,
    pub districts: Vec<District>,
    pub gates: Vec<Gate>,
    pub sites: Vec<Site>,
    pub routes: Vec<Route>,
    /// The arrangement of regions this seed drew.
    pub arch: &'static str,
    /// Water per macro cell (`CELLS` long). Bodies are standing water at their district's level;
    /// river cells are a channel R4 carves to level 0 with its banks (MAP.md 2.6).
    pub wet: Vec<Wet>,
    /// The river from its source to the lake: `river_in[0]` is the source, the last cell is
    /// 4-adjacent to a lake cell (the inlet).
    pub river_in: Vec<(u8, u8)>,
    /// From the lake to the county edge: `river_out[0]` is 4-adjacent to a lake cell (the outlet),
    /// and the last cell is on the map edge (the mouth).
    pub river_out: Vec<(u8, u8)>,
    /// The waterfall where the river leaves the upland, if its source touches the Works.
    pub fall: Option<(u8, u8)>,
    pub crossings: Vec<Crossing>,
}

type Cell = (i32, i32);
type Graphs = (Vec<Vec<u16>>, Vec<Vec<u16>>, Vec<Vec<u16>>);

pub const fn idx(x: i32, y: i32) -> usize {
    (y * MW + x) as usize
}
const fn inside(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < MW && y < MH
}

impl MacroPlan {
    pub fn district_at(&self, x: i32, y: i32) -> u8 {
        self.district_of[idx(x, y)]
    }
    pub fn level_at(&self, x: i32, y: i32) -> u8 {
        self.districts[self.district_at(x, y) as usize].level
    }
    pub fn reg_at(&self, x: i32, y: i32) -> Reg {
        self.districts[self.district_at(x, y) as usize].reg
    }
    pub fn site(&self, id: &str) -> Option<&Site> {
        self.sites.iter().find(|s| s.id == id)
    }
    pub fn site_cell(&self, id: &str) -> Cell {
        self.site(id).map_or((0, 0), |s| (i32::from(s.pos.0), i32::from(s.pos.1)))
    }

    /// The walking graph's cross-district edges: `(forward, reverse, road-only)` lists per cell.
    /// Verb gates are closed (no verb learned); ledges are one-way.
    pub fn graphs(&self, ledges: bool) -> Graphs {
        let mut fwd = vec![Vec::new(); CELLS];
        let mut rev = vec![Vec::new(); CELLS];
        let mut road = vec![Vec::new(); CELLS];
        for g in &self.gates {
            if matches!(g.kind, GateKind::Verb(_)) {
                continue;
            }
            let a = idx(i32::from(g.a.0), i32::from(g.a.1));
            let b = idx(i32::from(g.b.0), i32::from(g.b.1));
            if g.kind == GateKind::Ledge {
                if ledges {
                    fwd[a].push(b as u16);
                    rev[b].push(a as u16);
                }
                continue;
            }
            fwd[a].push(b as u16);
            fwd[b].push(a as u16);
            rev[a].push(b as u16);
            rev[b].push(a as u16);
            if g.road {
                road[a].push(b as u16);
                road[b].push(a as u16);
            }
        }
        (fwd, rev, road)
    }

    /// Dijkstra over the macro cells: moves inside a district, crossings only on `cross`.
    /// Returns (cost in cells, parent) per cell; `stop` ends the search when popped.
    pub fn dijkstra(&self, cross: &[Vec<u16>], src: usize, stop: Option<usize>) -> (Vec<i32>, Vec<u32>) {
        self.dijkstra_wet(cross, src, stop, 0)
    }

    /// [`Self::dijkstra`] with the water honoured. Standing water (lake, mere, pond) is impassable at
    /// macro scale, and a diagonal also skips a body at either orthogonal corner. River cells stay
    /// passable (a macro cell holds a 3 to 8 cell channel; R4's cell solver proves the real
    /// crossings) but entering one costs `wet_cost` more.
    pub fn dijkstra_wet(
        &self,
        cross: &[Vec<u16>],
        src: usize,
        stop: Option<usize>,
        wet_cost: i32,
    ) -> (Vec<i32>, Vec<u32>) {
        let mut dist = vec![INF; CELLS];
        let mut par = vec![u32::MAX; CELLS];
        let mut heap = BinaryHeap::new();
        dist[src] = 0;
        heap.push(Reverse((0, src as u32)));
        while let Some(Reverse((d, c))) = heap.pop() {
            let c = c as usize;
            if d > dist[c] {
                continue;
            }
            if Some(c) == stop {
                break;
            }
            let (cx, cy) = ((c as i32) % MW, (c as i32) / MW);
            let dc = self.district_of[c];
            let mut relax = |n: usize, w: i32, heap: &mut BinaryHeap<Reverse<(i32, u32)>>| {
                if self.wet[n].body() {
                    return;
                }
                let w = w + if self.wet[n] == Wet::River { wet_cost } else { 0 };
                if d + w < dist[n] {
                    dist[n] = d + w;
                    par[n] = c as u32;
                    heap.push(Reverse((d + w, n as u32)));
                }
            };
            for (dx, dy) in D4 {
                let (nx, ny) = (cx + dx, cy + dy);
                if inside(nx, ny) && self.district_of[idx(nx, ny)] == dc {
                    relax(idx(nx, ny), STEP_CELLS, &mut heap);
                }
            }
            for (dx, dy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if inside(nx, ny)
                    && self.district_of[idx(nx, ny)] == dc
                    && self.district_of[idx(cx + dx, cy)] == dc
                    && self.district_of[idx(cx, cy + dy)] == dc
                    && !self.wet[idx(cx + dx, cy)].body()
                    && !self.wet[idx(cx, cy + dy)].body()
                {
                    relax(idx(nx, ny), 23, &mut heap);
                }
            }
            for &n in &cross[c] {
                relax(n as usize, STEP_CELLS, &mut heap);
            }
        }
        (dist, par)
    }
}
/// Four-neighbour distance from every cell to the nearest cell where `hit` holds.
#[allow(clippy::needless_range_loop)]
fn dist_from(hit: &dyn Fn(usize) -> bool) -> Vec<i32> {
    let mut dist = vec![999; CELLS];
    let mut q = VecDeque::new();
    for c in 0..CELLS {
        if hit(c) {
            dist[c] = 0;
            q.push_back(c);
        }
    }
    while let Some(c) = q.pop_front() {
        let (x, y) = ((c as i32) % MW, (c as i32) / MW);
        for (dx, dy) in D4 {
            let (nx, ny) = (x + dx, y + dy);
            if inside(nx, ny) && dist[idx(nx, ny)] > dist[c] + 1 {
                dist[idx(nx, ny)] = dist[c] + 1;
                q.push_back(idx(nx, ny));
            }
        }
    }
    dist
}

/// Every cell's room for a body of district `d`: the least of its four-neighbour distance from
/// another district and from the map edge (the edge cell has room 1).
pub fn room_map(p: &MacroPlan, d: u8) -> Vec<i32> {
    let din = dist_from(&|c| p.district_of[c] != d);
    (0..CELLS)
        .map(|c| {
            let (x, y) = ((c as i32) % MW, (c as i32) / MW);
            din[c].min(x + 1).min(y + 1).min(MW - x).min(MH - y)
        })
        .collect()
}

/// Region name, for labels.
pub fn reg_name(r: Reg) -> String {
    String::from(match r {
        Reg::Lowfields => "lowfields",
        Reg::Waters => "waters",
        Reg::Works => "works",
    })
}
