//! The macro plan: MAP.md section 4.1's K1 to K5 on a 96 x 96 grid of 16-cell macro cells
//! (1,536 cells square). A pure function of the seed (and an attempt number, so a failing
//! layout re-rolls and is never shown), integer only, `no_std` plus `alloc`.
//!
//! K1/K2: the three regions (the Works across the north, the river valley between the Lowfields
//! and the Waters) cut into 14 districts by a seeded Voronoi, each with its level from the
//! region's table (MAP.md 4.2). K3: the gate graph: every border open, joined or wall, with its
//! gates typed (4.3), ledges laid toward hubs (4.5) and verb gates. K4: the story's sites by
//! district rule. K5: roads routed over road-carrying gates only.

use crate::skeleton::Biome;
use crate::steps::{Step, dice};
use alloc::collections::{BinaryHeap, VecDeque};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Reverse;
use jane_core::ZoneId;

/// The macro grid, in macro cells, and the cells in one.
pub const MW: i32 = 96;
pub const MH: i32 = 96;
pub const CELLS: usize = (MW * MH) as usize;
pub const STEP_CELLS: i32 = 16;
/// Tries before a seed is given up on (MAP.md 4.7: a failing seed re-rolls, never shown).
pub const ATTEMPTS: u8 = 250;

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
    /// Inland: its cells never touch another region; edge cells go to a district of this level.
    const fn inland(self) -> Option<u8> {
        match self {
            Role::TopField | Role::Foothills => Some(1),
            Role::Cutting | Role::Slagmere => Some(2),
            _ => None,
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
}

type Cell = (i32, i32);
type Graphs = (Vec<Vec<u16>>, Vec<Vec<u16>>, Vec<Vec<u16>>);
type Row = (Cell, GateKind, &'static str, bool);
type Pair = (Cell, Cell);

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

/// Integer value noise: a lattice every `sc` macro cells, bilinear between.
struct Noise {
    g: Vec<i32>,
    sc: i32,
    n: i32,
}

impl Noise {
    fn new(seed: u32, attempt: u8, salt: i32, sc: i32, amp: i32) -> Self {
        let n = MW / sc + 2;
        let g = (0..n * n)
            .map(|i| dice(seed, ZoneId::County, Step::MapLand, attempt, 100 + salt, i).range(-amp, amp))
            .collect();
        Self { g, sc, n }
    }
    fn at(&self, x: i32, y: i32) -> i32 {
        let (gx, gy, fx, fy) = (x / self.sc, y / self.sc, x % self.sc, y % self.sc);
        let v = |i: i32, j: i32| self.g[(j * self.n + i) as usize];
        let top = v(gx, gy) * (self.sc - fx) + v(gx + 1, gy) * fx;
        let bot = v(gx, gy + 1) * (self.sc - fx) + v(gx + 1, gy + 1) * fx;
        (top * (self.sc - fy) + bot * fy) / (self.sc * self.sc)
    }
}

/// A pair of noises at two scales: the warp that makes borders read as ground, not polygons.
struct Warp(Noise, Noise, Noise, Noise);

impl Warp {
    fn new(seed: u32, attempt: u8, salt: i32, big: i32, small: i32) -> Self {
        Self(
            Noise::new(seed, attempt, salt, 24, big),
            Noise::new(seed, attempt, salt + 1, 9, small),
            Noise::new(seed, attempt, salt + 2, 24, big),
            Noise::new(seed, attempt, salt + 3, 9, small),
        )
    }
    fn at(&self, x: i32, y: i32) -> (i32, i32) {
        (x + self.0.at(x, y) + self.1.at(x, y), y + self.2.at(x, y) + self.3.at(x, y))
    }
}

const COS15: [i32; 7] = [1000, 966, 866, 707, 500, 259, 0];

/// One of 24 directions, scaled by 1000.
fn dir24(i: i32) -> (i32, i32) {
    let i = i.rem_euclid(24);
    let (q, r) = (i / 6, (i % 6) as usize);
    let (mut x, mut y) = (COS15[r], COS15[6 - r]);
    for _ in 0..q {
        (x, y) = (-y, x);
    }
    (x, y)
}

/// A monotone pseudo-angle in 0..360 (the sectors' edges are random anyway).
fn pangle(dx: i32, dy: i32) -> i32 {
    let (ax, ay) = (dx.abs(), dy.abs());
    let t = 90 * ay / (ax + ay).max(1);
    match (dx >= 0, dy >= 0) {
        (true, true) => t,
        (false, true) => 180 - t,
        (false, false) => 180 + t,
        (true, false) => (360 - t) % 360,
    }
}

fn man(a: Cell, b: Cell) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Share of its region's area a district should take, per mille.
#[allow(clippy::match_same_arms)]
const fn share(r: Role) -> i32 {
    match r {
        Role::StationFields => 300,
        Role::SallowBottom => 130,
        Role::TopField => 110,
        Role::CastleTerrace => 250,
        Role::Foothills => 250,
        Role::MuseumTerrace => 300,
        Role::LibraryTerrace => 220,
        Role::ButterflyHollow => 130,
        Role::Lake => 380,
        Role::Crown => 300,
        Role::Lip => 0,
        Role::Cutting => 130,
        Role::Plateau => 330,
        Role::Slagmere => 130,
    }
}

/// K1: the three regions' arrangement, one of four archetypes chosen by the seed, each with its
/// own random centre, angles and sizes, then warped.
fn regions(seed: u32, attempt: u8, warp: &Warp) -> (Vec<Reg>, &'static str) {
    let d = |n: i32| dice(seed, ZoneId::County, Step::MapLand, attempt, 200 + n, 0);
    let pick = d(0).below(10);
    let arch = match pick {
        0..=2 => "junction",
        3 | 4 => "island",
        5 => "basin",
        6 | 7 => "lake",
        _ => "ridge",
    };
    let (cx, cy) = (d(1).range(34, 62), d(2).range(34, 62));
    let a0 = d(3).below(360) as i32;
    let sl = d(4).range(120, 180);
    let s2 = d(5).range(100, 360 - sl - 100);
    let flip = d(6).below(2) == 0;
    let (rx, ry) = (d(7).range(22, 32), d(8).range(20, 30));
    let (dx, dy) = dir24(d(9).below(24) as i32);
    let off = d(10).range(-6, 6);
    let wsec = d(11).range(130, 200);
    let rl = d(12).range(24, 32);
    let (lx, ly) = (d(13).range(-5, 5), d(14).range(-10, 4));
    let disp = d(15).range(-6, 8);
    let (hw, rlen) = (d(18).range(10, 15), d(19).range(8, 28));
    let (lrx, lry) = (d(16).range(20, 28), d(17).range(18, 26));
    let mut out = vec![Reg::Lowfields; CELLS];
    for y in 0..MH {
        for x in 0..MW {
            let (u, v) = warp.at(x, y);
            let r = match arch {
                "junction" => {
                    let a = (pangle(u - cx, v - cy) - a0).rem_euclid(360);
                    let (second, third) = if flip { (Reg::Works, Reg::Waters) } else { (Reg::Waters, Reg::Works) };
                    if a < sl {
                        Reg::Lowfields
                    } else if a < sl + s2 {
                        second
                    } else {
                        third
                    }
                }
                "island" => {
                    let (ex, ey) = (u - cx, v - cy);
                    if ex * ex * ry * ry + ey * ey * rx * rx < rx * rx * ry * ry {
                        Reg::Works
                    } else if (ex * dx + ey * dy) / 1000 > off {
                        Reg::Waters
                    } else {
                        Reg::Lowfields
                    }
                }
                "basin" => {
                    let (ex, ey) = (u - cx, v - cy);
                    let (ra, rb) = (rl, rl + lx);
                    if ex * ex * rb * rb + ey * ey * ra * ra < ra * ra * rb * rb {
                        Reg::Lowfields
                    } else if (pangle(ex, ey) - a0).rem_euclid(360) < wsec {
                        Reg::Works
                    } else {
                        Reg::Waters
                    }
                }
                "ridge" => {
                    // A long ridge of upland from off the map to a pass; the county's two halves
                    // meet beyond its end.
                    let (px, py) = (u - cx, v - cy);
                    let s = ((px * dx + py * dy) / 1000).clamp(-110, rlen);
                    let (qx, qy) = (px - s * dx / 1000, py - s * dy / 1000);
                    if qx * qx + qy * qy <= hw * hw {
                        Reg::Works
                    } else if (px * dy - py * dx > 0) == flip {
                        Reg::Lowfields
                    } else {
                        Reg::Waters
                    }
                }
                _ => {
                    let line = ((u - 48) * dx + (v - 48) * dy) / 1000;
                    let (wx, wy) = (48 + (ly * dx / 1000) + disp * dx / 1000, 48 + (ly * dy / 1000) + disp * dy / 1000);
                    let (wx, wy) = (wx.clamp(22, 74), wy.clamp(22, 74));
                    let (ex, ey) = (u - wx, v - wy);
                    if ex * ex * lry * lry + ey * ey * lrx * lrx < lrx * lrx * lry * lry {
                        Reg::Waters
                    } else if line < ly {
                        Reg::Works
                    } else {
                        Reg::Lowfields
                    }
                }
            };
            out[idx(x, y)] = r;
        }
    }
    let mut ids: Vec<u8> = out.iter().map(|&r| r as u8).collect();
    heal_by(&mut ids, 3, &|_, _| true);
    for (o, i) in out.iter_mut().zip(ids) {
        *o = [Reg::Lowfields, Reg::Waters, Reg::Works][i as usize];
    }
    (out, arch)
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

/// A district's biome, drawn per seed from what its place allows (no reed on the crown, marsh and
/// reed only where water lies, woods on slopes). Only biomes the game already paints.
fn biome_of(r: Role, seed: u32, attempt: u8) -> Biome {
    let opts: &[Biome] = match r {
        Role::StationFields => &[Biome::Field, Biome::Hedge, Biome::Garden, Biome::Field],
        Role::SallowBottom => &[Biome::WetWood, Biome::Marsh, Biome::WetWood],
        Role::TopField => &[Biome::Wood, Biome::Hill, Biome::Wood],
        Role::CastleTerrace => &[Biome::Field, Biome::Garden, Biome::Hedge],
        Role::Foothills => &[Biome::Foothill, Biome::Hill, Biome::Foothill, Biome::Wood],
        Role::MuseumTerrace => &[Biome::Field, Biome::Hedge, Biome::Garden],
        Role::LibraryTerrace => &[Biome::Wood, Biome::Hedge, Biome::Field],
        Role::ButterflyHollow => &[Biome::WetWood, Biome::Wood],
        Role::Lake => &[Biome::Reed, Biome::Marsh, Biome::Reed],
        Role::Crown => &[Biome::Hill, Biome::Foothill],
        Role::Lip => &[Biome::Foothill, Biome::Wood, Biome::Hill],
        Role::Cutting => &[Biome::Wood, Biome::WetWood, Biome::Hill],
        Role::Plateau => &[Biome::Slag, Biome::Yard, Biome::Hill],
        Role::Slagmere => &[Biome::Slag, Biome::Marsh],
    };
    let n = dice(seed, ZoneId::County, Step::MapDistrict, attempt, 50 + i32::from(r.id()), 3).below(opts.len() as u32);
    opts[n as usize]
}

/// The plan for `seed`, attempt `attempt`: layout only, not yet proven (see `macro_check`).
pub fn build(seed: u32, attempt: u8) -> MacroPlan {
    let (reg, arch) = regions(seed, attempt, &Warp::new(seed, attempt, 1, 6, 2));
    let warp = Warp::new(seed, attempt, 9, 8, 3);
    let dreg: [Vec<i32>; 3] = [Reg::Lowfields, Reg::Waters, Reg::Works].map(|r| dist_from(&|c| reg[c] == r));
    let rdepth = |x: i32, y: i32| -> i32 {
        let c = idx(x, y);
        (0..3).filter(|&r| r != reg[c] as usize).map(|r| dreg[r][c]).min().unwrap_or(0)
    };
    let (l, wa, wk) = (Reg::Lowfields as usize, Reg::Waters as usize, Reg::Works as usize);
    let area_of = |r: Reg| reg.iter().filter(|&&q| q == r).count() as i32;
    // K2: district seeds, each placed by what the story needs of it, drawn from the best few cells.
    let mut seeds: [Cell; 14] = [(-200, -200); 14];
    let pick =
        |placed: &[Cell; 14], role: Role, ok: &dyn Fn(i32, i32) -> bool, score: &dyn Fn(i32, i32) -> i32| -> Cell {
            let want = role.reg();
            let mut c: Vec<(i32, usize)> = Vec::new();
            // Seeds keep 9 cells apart if the region has room, then 5, then any.
            'found: for sp in [9, 5, 0] {
                for m in [3, 1, 0] {
                    for y in 0..MH {
                        for x in 0..MW {
                            if reg[idx(x, y)] == want
                                && rdepth(x, y) >= m
                                && ok(x, y)
                                && placed.iter().all(|&q| man((x, y), q) >= sp)
                            {
                                c.push((score(x, y), idx(x, y)));
                            }
                        }
                    }
                    if c.len() >= 8 {
                        break 'found;
                    }
                    c.clear();
                }
            }
            if c.is_empty() {
                return (0..CELLS).find(|&i| reg[i] == want).map_or((48, 48), |i| ((i as i32) % MW, (i as i32) / MW));
            }
            c.sort_by_key(|&(s, i)| (core::cmp::Reverse(s), i));
            let k = (c.len() / 20).max(3).min(c.len()) as i32;
            let n = dice(seed, ZoneId::County, Step::MapDistrict, attempt, i32::from(role.id()), 7).range(0, k - 1)
                as usize;
            let i = c[n].1;
            ((i as i32) % MW, (i as i32) / MW)
        };
    let any = |_: i32, _: i32| true;
    let id = |r: Role| r.id() as usize;
    seeds[id(Role::CastleTerrace)] =
        pick(&seeds, Role::CastleTerrace, &any, &|x, y| -(dreg[wa][idx(x, y)] - 9).abs() * 3 + rdepth(x, y).min(6));
    let ct = seeds[id(Role::CastleTerrace)];
    seeds[id(Role::StationFields)] =
        pick(&seeds, Role::StationFields, &any, &|x, y| man((x, y), ct).min(30) + rdepth(x, y).min(6));
    let sf = seeds[id(Role::StationFields)];
    seeds[id(Role::TopField)] = pick(&seeds, Role::TopField, &any, &|x, y| {
        -(man((x, y), ct) - 11).abs() * 2 + dreg[wk][idx(x, y)].min(14) * 2 + dreg[wa][idx(x, y)].min(10)
    });
    let tf = seeds[id(Role::TopField)];
    seeds[id(Role::SallowBottom)] = pick(&seeds, Role::SallowBottom, &any, &|x, y| {
        -(man((x, y), sf) - 16).abs() * 2 + man((x, y), ct).min(24) + man((x, y), tf).min(20) + rdepth(x, y).min(8)
    });
    let sb = seeds[id(Role::SallowBottom)];
    // The mine is a five-to-seven-minute walk from the Halt: about 135 macro steps in all, of which
    // the first walk and the road to the Castle take their share.
    let to_mine = (156 - 36 - man(sf, ct)).clamp(10, 50);
    seeds[id(Role::Foothills)] = pick(&seeds, Role::Foothills, &any, &|x, y| {
        let c = (x, y);
        -(man(c, ct) - to_mine).abs() * 3 - (man(c, sf).min(man(c, ct)) - 15).abs()
            + man(c, tf).min(14)
            + man(c, sb).min(14)
    });
    seeds[id(Role::MuseumTerrace)] = pick(&seeds, Role::MuseumTerrace, &any, &|x, y| {
        -(dreg[l][idx(x, y)] - 8).abs() * 3 - man((x, y), ct) / 2 + rdepth(x, y).min(8)
    });
    let mt = seeds[id(Role::MuseumTerrace)];
    seeds[id(Role::Lake)] = pick(&seeds, Role::Lake, &|x, y| dreg[wk][idx(x, y)] >= 7, &|x, y| {
        dreg[l][idx(x, y)].min(dreg[wk][idx(x, y)]).min(20) * 2 + man((x, y), mt).min(26) + rdepth(x, y).min(10)
    });
    let lk = seeds[id(Role::Lake)];
    seeds[id(Role::LibraryTerrace)] = pick(&seeds, Role::LibraryTerrace, &any, &|x, y| {
        -(man((x, y), mt) - 18).abs() * 2 - dreg[wk][idx(x, y)].min(30) + rdepth(x, y).min(6)
    });
    let lt = seeds[id(Role::LibraryTerrace)];
    seeds[id(Role::ButterflyHollow)] =
        pick(&seeds, Role::ButterflyHollow, &|x, y| dreg[wk][idx(x, y)] >= 9 && dreg[l][idx(x, y)] >= 6, &|x, y| {
            man((x, y), mt).min(man((x, y), lt)).min(man((x, y), lk)).min(26) * 2 - dreg[wk][idx(x, y)]
                + rdepth(x, y).min(6)
        });
    // The School's crown: deep in the upland, and within sight of the town (80 macro cells).
    seeds[id(Role::Crown)] = pick(
        &seeds,
        Role::Crown,
        &|x, y| man((x, y), ct) <= 62 && man((x, y), sf) <= 62 && rdepth(x, y) >= 9,
        &|x, y| rdepth(x, y).min(20) * 3 - man((x, y), ct) / 4,
    );
    let cr = seeds[id(Role::Crown)];
    seeds[id(Role::Plateau)] =
        pick(&seeds, Role::Plateau, &any, &|x, y| -(man((x, y), cr) - 18).abs() * 2 - dreg[wa][idx(x, y)].min(24));
    let pl = seeds[id(Role::Plateau)];
    seeds[id(Role::Cutting)] = pick(&seeds, Role::Cutting, &any, &|x, y| {
        -(man((x, y), cr) - 22).abs() - (dreg[l][idx(x, y)] - 11).abs() * 2 + rdepth(x, y).min(8)
    });
    seeds[id(Role::Slagmere)] = pick(&seeds, Role::Slagmere, &any, &|x, y| {
        man((x, y), cr).min(30) + man((x, y), pl).min(24) - (dreg[wa][idx(x, y)] - 10).abs() * 2 + rdepth(x, y).min(8)
    });
    // The lip's graveyard end: the band cell nearest the town.
    seeds[id(Role::Lip)] = (0..CELLS)
        .filter(|&c| reg[c] == Reg::Works && dreg[l][c] == 4)
        .min_by_key(|&c| (man(((c as i32) % MW, (c as i32) / MW), ct), c))
        .map_or(seeds[id(Role::Plateau)], |c| ((c as i32) % MW, (c as i32) / MW));
    let mut r2s: Vec<i32> = ROLES.iter().map(|r| area_of(r.reg()) * share(*r) / 3140).collect();
    let nearest_w = |x: i32, y: i32, ok: &dyn Fn(Role) -> bool, w: &[i32]| -> u8 {
        let (u, v) = warp.at(x, y);
        let mut best = (INF, 0u8);
        for r in ROLES {
            if !ok(r) {
                continue;
            }
            let (sx, sy) = seeds[r.id() as usize];
            let d = 4 * ((u - sx) * (u - sx) + (v - sy) * (v - sy)) - 2 * w[r.id() as usize];
            if d < best.0 {
                best = (d, r.id());
            }
        }
        best.1
    };
    let mut dm = vec![0u8; CELLS];
    // The districts' reaches are balanced over a few passes so each takes about its share of its
    // region (the power diagram's weights chase the areas).
    for pass in 0..16 {
        for y in 0..MH {
            for x in 0..MW {
                let c = idx(x, y);
                let r = reg[c];
                dm[c] = if r == Reg::Works && dreg[l][c] <= 6 {
                    Role::Lip.id()
                } else if r == Reg::Works && dreg[wa][c] <= 5 {
                    Role::Plateau.id()
                } else if r == Reg::Lowfields && dreg[wk][c] <= 4 {
                    nearest_w(x, y, &|q| q == Role::StationFields || q == Role::CastleTerrace, &r2s)
                } else {
                    nearest_w(x, y, &|q| q.reg() == r && q != Role::Lip, &r2s)
                };
            }
        }
        if pass == 15 {
            break;
        }
        let mut got = [0i32; 14];
        for &d in &dm {
            got[d as usize] += 1;
        }
        let w = &mut r2s;
        for r in ROLES {
            if r == Role::Lip {
                continue;
            }
            let (mut have, mut sum) = (0, 0);
            for q in ROLES {
                if q.reg() == r.reg() && q != Role::Lip {
                    have += got[q.id() as usize];
                    sum += share(q);
                }
            }
            let want = have * share(r) / sum.max(1);
            let i = r.id() as usize;
            w[i] = (w[i] + (want - got[i]) / 2).clamp(0, 12000);
        }
    }
    let nearest = |x: i32, y: i32, ok: &dyn Fn(Role) -> bool| nearest_w(x, y, ok, &r2s);
    // Every district keeps ground round its seed (a small one may lose it to a neighbour's reach).
    for r in ROLES {
        if r == Role::Lip {
            continue;
        }
        let (sx, sy) = seeds[r.id() as usize];
        let rad = if r == Role::Crown { 6 } else { 3 };
        for y in (sy - rad).max(0)..=(sy + rad).min(MH - 1) {
            for x in (sx - rad).max(0)..=(sx + rad).min(MW - 1) {
                if (x - sx) * (x - sx) + (y - sy) * (y - sy) <= rad * rad && reg[idx(x, y)] == r.reg() {
                    dm[idx(x, y)] = r.id();
                }
            }
        }
    }
    let role = |id: u8| ROLES[id as usize];
    // Inland districts (the cutting, the mere, the Top Field) never touch another region. A step of
    // two levels between districts is a closed cliff (the crown's four-cell faces), not a join.
    heal(&mut dm, &role);
    let inland = |dm: &mut Vec<u8>| {
        for _ in 0..3 {
            let mut ch = Vec::new();
            for y in 0..MH {
                for x in 0..MW {
                    let r = role(dm[idx(x, y)]);
                    let Some(lv) = r.inland() else { continue };
                    let edge = D4.iter().any(|&(dx, dy)| {
                        let (nx, ny) = (x + dx, y + dy);
                        inside(nx, ny) && role(dm[idx(nx, ny)]).reg() != r.reg()
                    });
                    if edge {
                        ch.push((idx(x, y), nearest(x, y, &|q| q.reg() == r.reg() && q.level() == lv)));
                    }
                }
            }
            for (i, d) in ch {
                dm[i] = d;
            }
        }
    };
    inland(&mut dm);
    heal(&mut dm, &role);
    inland(&mut dm);
    heal(&mut dm, &role);
    let mut area = [0u32; 14];
    for &d in &dm {
        area[d as usize] += 1;
    }
    let districts: Vec<District> = ROLES
        .iter()
        .map(|r| District {
            role: *r,
            reg: r.reg(),
            level: r.level(),
            seed: seeds[r.id() as usize],
            area: area[r.id() as usize],
            biome: biome_of(*r, seed, attempt),
        })
        .collect();
    let mut plan = MacroPlan {
        seed,
        attempt,
        district_of: dm,
        districts,
        gates: Vec::new(),
        sites: Vec::new(),
        routes: Vec::new(),
        arch,
    };
    place_sites(&mut plan);
    lay_gates(&mut plan);
    fit_mine(&mut plan);
    route_roads(&mut plan);
    plan
}

/// Pieces of a district apart from its largest go to the neighbour they touch most (in the same
/// region), so every district is one body.
fn heal(dm: &mut [u8], role: &dyn Fn(u8) -> Role) {
    heal_by(dm, 14, &|a, b| role(a).reg() == role(b).reg());
}

/// `heal` over `classes` values, a piece joining only a neighbour `ok` allows.
fn heal_by(dm: &mut [u8], classes: usize, ok: &dyn Fn(u8, u8) -> bool) {
    for _ in 0..8 {
        let mut seen = vec![false; CELLS];
        let mut comps: Vec<(u8, Vec<usize>)> = Vec::new();
        for c0 in 0..CELLS {
            if seen[c0] {
                continue;
            }
            let d = dm[c0];
            let mut comp = Vec::new();
            let mut stack = vec![c0];
            seen[c0] = true;
            while let Some(c) = stack.pop() {
                comp.push(c);
                let (x, y) = ((c as i32) % MW, (c as i32) / MW);
                for (dx, dy) in D4 {
                    let (nx, ny) = (x + dx, y + dy);
                    if inside(nx, ny) && !seen[idx(nx, ny)] && dm[idx(nx, ny)] == d {
                        seen[idx(nx, ny)] = true;
                        stack.push(idx(nx, ny));
                    }
                }
            }
            comps.push((d, comp));
        }
        let mut best = vec![0usize; classes];
        for (d, comp) in &comps {
            best[*d as usize] = best[*d as usize].max(comp.len());
        }
        let mut changed = false;
        for (d, comp) in &comps {
            if comp.len() == best[*d as usize] {
                best[*d as usize] = usize::MAX;
                continue;
            }
            let mut votes = vec![0u32; classes];
            for &c in comp {
                let (x, y) = ((c as i32) % MW, (c as i32) / MW);
                for (dx, dy) in D4 {
                    let (nx, ny) = (x + dx, y + dy);
                    if inside(nx, ny) {
                        let n = dm[idx(nx, ny)];
                        if n != *d && ok(n, *d) {
                            votes[n as usize] += 1;
                        }
                    }
                }
            }
            let to = (0..classes).max_by_key(|&i| (votes[i], core::cmp::Reverse(i))).unwrap_or(0);
            if votes[to] > 0 {
                for &c in comp {
                    dm[c] = to as u8;
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// The cell of district `d` nearest `target`, keeping `margin` cells off the border if it can.
fn snap(plan: &MacroPlan, d: Role, target: Cell, margin: i32) -> Cell {
    for m in (0..=margin).rev() {
        let mut best = (INF, (0, 0));
        for y in 0..MH {
            for x in 0..MW {
                if plan.district_of[idx(x, y)] != d.id() {
                    continue;
                }
                let clear = (-m..=m).all(|dy| {
                    (-m..=m).all(|dx| !inside(x + dx, y + dy) || plan.district_of[idx(x + dx, y + dy)] == d.id())
                });
                if !clear {
                    continue;
                }
                let s = (x - target.0) * (x - target.0) + (y - target.1) * (y - target.1);
                if s < best.0 {
                    best = (s, (x, y));
                }
            }
        }
        if best.0 != INF {
            return best.1;
        }
    }
    plan.districts[d.id() as usize].seed
}

/// Where the story starts: a Station Fields cell a good walk from Julie's (22 to 40 macro cells),
/// on the nearest edge half the time and anywhere inland the rest.
fn station_pos(p: &MacroPlan) -> Cell {
    let j = p.site_cell("julie_house");
    let sf = Role::StationFields.id();
    let mut cands: Vec<(i32, Cell)> = Vec::new();
    for (lo, hi) in [(340, 540), (300, 600), (200, 900)] {
        for y in 0..MH {
            for x in 0..MW {
                let clear = (-2..=2)
                    .all(|dy| (-2..=2).all(|dx| !inside(x + dx, y + dy) || p.district_of[idx(x + dx, y + dy)] == sf));
                let (ax, ay) = ((x - j.0).abs(), (y - j.1).abs());
                let m = 16 * ax.max(ay) + 7 * ax.min(ay);
                if p.district_of[idx(x, y)] == sf && clear && (lo..=hi).contains(&m) {
                    cands.push((x.min(y).min(MW - 1 - x).min(MH - 1 - y), (x, y)));
                }
            }
        }
        if !cands.is_empty() {
            break;
        }
    }
    if cands.is_empty() {
        return snap(p, Role::StationFields, (2, j.1), 1);
    }
    cands.sort();
    let mut d = dice(p.seed, ZoneId::County, Step::MapGate, p.attempt, 900, 0);
    if d.below(2) == 0 { cands[0].1 } else { cands[d.below(cands.len() as u32) as usize].1 }
}

/// A hub's house stands by a one-level drop (so a ledge can land beside it): the pair is drawn at
/// random from those of the hub's district, and the site is the district cell nearest the drop.
fn hub_site(p: &MacroPlan, role: Role, fallback: Cell, id: &str) -> Cell {
    let me = role.id();
    let mut cands: Vec<Cell> = Vec::new();
    for y in 0..MH {
        for x in 0..MW {
            for (dx, dy) in D4 {
                let (nx, ny) = (x + dx, y + dy);
                if !inside(nx, ny) {
                    continue;
                }
                let (da, db) = (p.district_of[idx(x, y)], p.district_of[idx(nx, ny)]);
                if da == db || (da != me && db != me) {
                    continue;
                }
                let (ra, rb) = (&p.districts[da as usize], &p.districts[db as usize]);
                if ra.level != rb.level + 1
                    || ra.role.pocket()
                    || rb.role.pocket()
                    || ra.reg != rb.reg && (ra.reg != Reg::Works && rb.reg != Reg::Works)
                {
                    continue;
                }
                cands.push(if da == me { (x, y) } else { (nx, ny) });
            }
        }
    }
    if cands.is_empty() {
        return snap(p, role, fallback, 1);
    }
    let mut d = dice(p.seed, ZoneId::County, Step::MapGate, p.attempt, 910 + id.len() as i32, 0);
    let t = cands[d.below(cands.len() as u32) as usize];
    snap(p, role, t, 1)
}

/// The mine mouth: the Foothills cell that makes the Halt-to-mine road take about six minutes
/// (macro steps run 0.72 of a Manhattan step at 16 cells, 7.5 cells a second, a road's wind 1.5).
fn mine_pos(p: &MacroPlan, fallback: Cell) -> Cell {
    let (st, ju, town) = (p.site_cell("station"), p.site_cell("julie_house"), p.site_cell("town"));
    let fh = Role::Foothills.id();
    let mut best: Option<(i32, i32, Cell)> = None;
    for y in 0..MH {
        for x in 0..MW {
            let clear = (-1..=1)
                .all(|dy| (-1..=1).all(|dx| !inside(x + dx, y + dy) || p.district_of[idx(x + dx, y + dy)] == fh));
            if p.district_of[idx(x, y)] != fh || !clear {
                continue;
            }
            let total = man(st, ju) + man(ju, town) + man((x, y), town);
            let key = ((total * 23 / 10) - 360).abs();
            if best.is_none_or(|b| key < b.0) {
                best = Some((key, 0, (x, y)));
            }
        }
    }
    best.map_or_else(|| snap(p, Role::Foothills, fallback, 1), |b| b.2)
}

fn place_sites(p: &mut MacroPlan) {
    let sd = |r: Role| p.districts[r.id() as usize].seed;
    let sf = sd(Role::StationFields);
    let sb = sd(Role::SallowBottom);
    let fh = sd(Role::Foothills);
    let lk = sd(Role::Lake);
    let pl = sd(Role::Plateau);
    let lp = sd(Role::Lip);
    // (id, name, role, target, margin, dungeon)
    let rows: [(&'static str, &'static str, Role, Cell, i32, bool); 16] = [
        ("julie_house", "Julie's", Role::StationFields, sb, 1, false),
        ("station", "The Halt", Role::StationFields, (2, sf.1 + 4), 2, false),
        ("town", "Castle", Role::CastleTerrace, sd(Role::TopField), 1, false),
        ("farm", "The Farm", Role::StationFields, (sf.0 + 10, sf.1 + 14), 2, false),
        ("gold_mine", "The Gold Mine", Role::Foothills, (fh.0, fh.1 + 3), 2, true),
        ("museum", "The Museum", Role::MuseumTerrace, lk, 1, false),
        ("reed_camp", "The Reed Camp", Role::Lake, (lk.0 - 9, lk.1 - 7), 2, false),
        ("library", "The Library", Role::LibraryTerrace, sd(Role::LibraryTerrace), 2, false),
        ("butterfly_forest", "Butterfly Forest", Role::ButterflyHollow, sd(Role::ButterflyHollow), 2, false),
        ("lake_statue", "The Statue", Role::Lake, lk, 3, false),
        ("graveyard", "The Graveyard", Role::Lip, (lp.0 + 2, lp.1 + 2), 2, false),
        ("factory", "The Factory", Role::Plateau, (pl.0 - 4, pl.1 + 4), 2, false),
        ("canteen", "The Canteen", Role::Plateau, (pl.0 + 9, pl.1 - 2), 2, false),
        ("school", "The School", Role::Crown, sd(Role::Crown), 3, true),
        ("car_wood", "Car Wood", Role::StationFields, (sf.0 - 6, sf.1 + 18), 2, false),
        ("burial", "The Burial", Role::Cutting, sd(Role::Cutting), 2, true),
    ];
    for (id, name, role, target, margin, dungeon) in rows {
        let pos = match id {
            "station" => station_pos(p),
            "julie_house" | "town" | "museum" => hub_site(p, role, target, id),
            "gold_mine" => mine_pos(p, target),
            _ => snap(p, role, target, margin),
        };
        p.sites.push(Site { id, name, district: role.id(), pos: (pos.0 as u8, pos.1 as u8), dungeon });
    }
}

/// Orthogonally adjacent cell pairs `(in a, in b)` for districts accepted by `fa` and `fb`.
fn pairs(p: &MacroPlan, fa: &dyn Fn(u8) -> bool, fb: &dyn Fn(u8) -> bool) -> Vec<Pair> {
    let mut out = Vec::new();
    for y in 0..MH {
        for x in 0..MW {
            let da = p.district_of[idx(x, y)];
            if !fa(da) {
                continue;
            }
            for (dx, dy) in D4 {
                let (nx, ny) = (x + dx, y + dy);
                if inside(nx, ny) {
                    let db = p.district_of[idx(nx, ny)];
                    if db != da && fb(db) {
                        out.push(((x, y), (nx, ny)));
                    }
                }
            }
        }
    }
    out
}

fn cheb(a: Cell, b: Cell) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// The index of the pair nearest `target` along `key`, then outward until `sep` clear of `used`.
fn pick(prs: &mut [Pair], key: &dyn Fn(&Pair) -> i32, target: i32, used: &[Cell], sep: i32) -> Option<Pair> {
    prs.sort_by_key(|q| (key(q), q.0));
    for s in [sep, 2, 0] {
        let mut best: Option<(i32, usize)> = None;
        for (i, q) in prs.iter().enumerate() {
            if used.iter().any(|&u| cheb(u, q.0) < s || cheb(u, q.1) < s) {
                continue;
            }
            let d = (key(q) - target).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
        if let Some((_, i)) = best {
            return Some(prs[i]);
        }
    }
    None
}

/// The pair whose first cell is nearest `anchor`, `sep` clear of the gates already laid.
fn pick_near(prs: &[Pair], anchor: Cell, used: &[Cell], sep: i32) -> Option<Pair> {
    for s in [sep, 6, 2, 0] {
        let mut best: Option<(i32, usize)> = None;
        for (i, q) in prs.iter().enumerate() {
            if used.iter().any(|&u| cheb(u, q.0) < s || cheb(u, q.1) < s) {
                continue;
            }
            let d = man(q.0, anchor);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
        if let Some((_, i)) = best {
            return Some(prs[i]);
        }
    }
    None
}

#[allow(clippy::single_element_loop)]
fn lay_gates(p: &mut MacroPlan) {
    let mut used: Vec<Cell> = Vec::new();
    let reg_pair = |a: Reg, b: Reg, p: &MacroPlan| {
        let (r1, r2) = (a, b);
        let d = p.districts.clone();
        pairs(p, &|x| d[x as usize].reg == r1 && !d[x as usize].role.pocket(), &|x| {
            d[x as usize].reg == r2 && !d[x as usize].role.pocket()
        })
    };
    let town = p.site_cell("town");
    let reed = p.site_cell("reed_camp");
    let fact = p.site_cell("factory");
    let bfly = p.site_cell("butterfly_forest");
    let (sd0, at0) = (p.seed, p.attempt);
    let slot = move |n: i32, ax: i32| dice(sd0, ZoneId::County, Step::MapGate, at0, n, ax);
    let put = |p: &mut MacroPlan,
               used: &mut Vec<Cell>,
               q: Pair,
               kind: GateKind,
               name: &'static str,
               road: bool,
               border: Border| {
        let (da, db) = (p.district_of[idx(q.0.0, q.0.1)], p.district_of[idx(q.1.0, q.1.1)]);
        p.gates.push(Gate {
            kind,
            name,
            a: (q.0.0 as u8, q.0.1 as u8),
            b: (q.1.0 as u8, q.1.1 as u8),
            da,
            db,
            road,
            border,
        });
        used.push(q.0);
        used.push(q.1);
    };
    let grave = p.site_cell("graveyard");
    let stat = p.site_cell("station");
    let museum = p.site_cell("museum");
    let canteen = p.site_cell("canteen");
    let library = p.site_cell("library");
    let rand_at = |prs: &[Pair], n: i32| -> Cell {
        if prs.is_empty() { (48, 48) } else { prs[slot(n, 9).below(prs.len() as u32) as usize].0 }
    };
    // Lowfields to Works: the escarpment. The Cutting (the Castle road), the Church steps, the rail
    // arch, and a Grow vine most of the time.
    let lw = reg_pair(Reg::Lowfields, Reg::Works, p);
    let mut rows: Vec<Row> = vec![
        (town, GateKind::RampRoad, "The Cutting", true),
        (grave, GateKind::Stair, "The Church Steps", false),
        (stat, GateKind::Underpass, "The Rail Arch", true),
    ];
    if slot(3, 1).below(1000) < 700 {
        rows.push((rand_at(&lw, 3), GateKind::Verb(Verb::Grow), "The Grow Vines", false));
    }
    // Lowfields to Waters: the viaduct, the footbridge (Repair), the stones, a towpath under.
    let lwa = reg_pair(Reg::Lowfields, Reg::Waters, p);
    let mid = ((town.0 + museum.0) / 2, (town.1 + museum.1) / 2);
    let mut rows2: Vec<Row> = vec![
        (mid, GateKind::Span, "The Castle Viaduct", true),
        (mid, GateKind::Underpass, "The Towpath", true),
        (reed, GateKind::Verb(Verb::Repair), "The Footbridge", false),
    ];
    if slot(4, 1).below(1000) < 700 {
        rows2.push((rand_at(&lwa, 5), GateKind::SteppingStones, "The Stepping Stones", false));
    }
    // Waters to Works: the slag cliffs.
    let wwk = reg_pair(Reg::Waters, Reg::Works, p);
    let mut rows3: Vec<Row> = vec![
        (fact, GateKind::CompanyGate, "The Company Gate", true),
        (canteen, GateKind::Span, "The Pipe Walkway", false),
        (library, GateKind::RampRoad, "The Slag Lane", true),
    ];
    if slot(6, 1).below(1000) < 400 {
        rows3.push((rand_at(&wwk, 6), GateKind::Underpass, "The Culvert", true));
    }
    for (prs, rows) in [(&lw, rows), (&lwa, rows2), (&wwk, rows3)] {
        for (anchor, kind, name, road) in rows {
            if let Some(q) = pick_near(prs, anchor, &used, 10) {
                put(p, &mut used, q, kind, name, road, Border::Wall);
            }
        }
    }
    // Ledges, one-way, down toward hubs (MAP.md 4.5): `a` the top, `b` the landing.
    let hub_julie = p.site_cell("julie_house");
    let hub_museum = p.site_cell("museum");
    let d2 = |a: Cell, b: Cell| (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1);
    let ledge = |p: &MacroPlan,
                 used: &[Cell],
                 hubs: &[Cell],
                 top_ok: &dyn Fn(u8) -> bool,
                 land_ok: &dyn Fn(u8) -> bool|
     -> Option<Pair> {
        let near = |c: Cell| hubs.iter().map(|&h| d2(c, h)).min().unwrap_or(0);
        let mut best: Option<(i32, Pair)> = None;
        for q in pairs(p, &|x| top_ok(x), &|x| land_ok(x)) {
            let (da, db) = (p.district_of[idx(q.0.0, q.0.1)], p.district_of[idx(q.1.0, q.1.1)]);
            let (ra, rb) = (&p.districts[da as usize], &p.districts[db as usize]);
            if ra.level != rb.level + 1 || ra.role.pocket() || rb.role.pocket() {
                continue;
            }
            if (ra.reg == Reg::Lowfields && rb.reg == Reg::Waters)
                || (ra.reg == Reg::Waters && rb.reg == Reg::Lowfields)
            {
                continue;
            }
            if near(q.0) <= near(q.1) || used.iter().any(|&u| cheb(u, q.0) < 4 || cheb(u, q.1) < 4) {
                continue;
            }
            let s = near(q.1);
            if best.is_none_or(|(bs, _)| s < bs) {
                best = Some((s, q));
            }
        }
        best.map(|b| b.1)
    };
    let dis: Vec<Reg> = p.districts.iter().map(|d| d.reg).collect();
    let reg_is = |r: Reg| {
        let dis = dis.clone();
        move |x: u8| dis[x as usize] == r
    };
    let any = |_: u8| true;
    let mut hub_ledges: Vec<(Option<Pair>, &'static str)> = Vec::new();
    for (hub, name) in [(hub_julie, "Julie's Drop"), (town, "The Castle Drop"), (hub_museum, "The Museum Drop")] {
        let l = ledge(p, &used, &[hub], &any, &any);
        if let Some(q) = l {
            used.push(q.0);
            used.push(q.1);
        }
        hub_ledges.push((l, name));
    }
    let mine = ledge(p, &used, &[hub_julie, town], &|x| x == Role::Foothills.id(), &any);
    if let Some(q) = mine {
        used.push(q.0);
        used.push(q.1);
    }
    let mut extra: Vec<(Option<Pair>, &'static str)> = vec![(mine, "The Mine Drop")];
    for name in ["The Escarpment Drop"] {
        let l = ledge(p, &used, &[town], &reg_is(Reg::Works), &reg_is(Reg::Lowfields));
        if let Some(q) = l {
            used.push(q.0);
            used.push(q.1);
        }
        extra.push((l, name));
    }
    let l = ledge(p, &used, &[bfly], &reg_is(Reg::Works), &reg_is(Reg::Waters));
    extra.push((l, "The Butterfly Drop"));
    for (l, name) in hub_ledges.into_iter().chain(extra) {
        if let Some(q) = l {
            let (da, db) = (p.district_of[idx(q.0.0, q.0.1)], p.district_of[idx(q.1.0, q.1.1)]);
            let border = if p.districts[da as usize].reg == p.districts[db as usize].reg {
                Border::Joined
            } else {
                Border::Wall
            };
            put(p, &mut used, q, GateKind::Ledge, name, false, border);
        }
    }
    // Inside a region: open borders get gaps, joined ones a ramp road and a stair. The crown and
    // the hollow are pockets with their own rules.
    let crown = Role::Crown.id();
    let hollow = Role::ButterflyHollow.id();
    let mut crown_way: Option<u8> = None;
    for i in 0..14u8 {
        for j in (i + 1)..14u8 {
            let (ri, rj) = (ROLES[i as usize], ROLES[j as usize]);
            if ri.reg() != rj.reg() {
                continue;
            }
            let mut prs = pairs(p, &|x| x == i, &|x| x == j);
            if prs.len() < 3 {
                continue;
            }
            let dl = ri.level().abs_diff(rj.level());
            if dl >= 2 && i != crown && j != crown {
                continue;
            }
            let n = prs.len() as i32;
            if i == crown || j == crown {
                // One switchback ramp (to the busiest level-2 neighbour), and the lift.
                let other = if i == crown { j } else { i };
                let better = crown_way.is_none_or(|c| {
                    let cn = pairs(p, &|x| x == crown, &|x| x == c).len();
                    prs.len() > cn || (prs.len() == cn && other == Role::Plateau.id())
                });
                if better {
                    crown_way = Some(other);
                }
                continue;
            }
            let mut specs: Vec<(GateKind, &'static str, bool, Border)> = Vec::new();
            if dl == 0 {
                specs.push((GateKind::Gap, "Gap", true, Border::Open));
                specs.push((GateKind::Gap, "Gap", true, Border::Open));
            } else {
                specs.push((GateKind::RampRoad, "Ramp", true, Border::Joined));
                specs.push((GateKind::Stair, "Stair", false, Border::Joined));
            }
            if i == hollow || j == hollow {
                specs.truncate(1);
            }
            let ns = specs.len() as i32;
            for (k, (kind, name, road, border)) in specs.into_iter().enumerate() {
                let tgt = (k as i32 * 2 + 1) * n / (ns * 2);
                let kk = |q: &Pair| q.0.0 + q.0.1;
                let mut sorted = prs.clone();
                sorted.sort_by_key(|q| (kk(q), q.0));
                let target = kk(&sorted[tgt.clamp(0, n - 1) as usize]);
                if let Some(q) = pick(&mut prs, &kk, target, &used, 3) {
                    put(p, &mut used, q, kind, name, road, border);
                }
            }
            // Foothills: a rockfall that opens a second way up (Explosion).
            if (i == Role::Foothills.id() || j == Role::Foothills.id()) && dl == 1 && n >= 8 {
                let kk = |q: &Pair| q.0.0 + q.0.1;
                let target = kk(&prs[(n / 2) as usize]);
                if let Some(q) = pick(&mut prs, &kk, target, &used, 3) {
                    put(p, &mut used, q, GateKind::Verb(Verb::Explosion), "The Rockfall", false, Border::Joined);
                }
            }
        }
    }
    // Every district keeps three ways in (MAP.md 4.5), pockets apart: the lacking ones are added
    // along the longest border it has with a neighbour of its region.
    for _ in 0..2 {
        for i in 0..14u8 {
            let ri = ROLES[i as usize];
            if ri.pocket() {
                continue;
            }
            let have = p
                .gates
                .iter()
                .filter(|g| !matches!(g.kind, GateKind::Ledge | GateKind::Verb(_)) && (g.da == i || g.db == i))
                .count();
            if have >= 3 {
                continue;
            }
            let mut best: Option<(usize, u8, Vec<Pair>)> = None;
            for j in 0..14u8 {
                let rj = ROLES[j as usize];
                if j == i || rj.reg() != ri.reg() || rj.pocket() || ri.level().abs_diff(rj.level()) >= 2 {
                    continue;
                }
                let prs = pairs(p, &|x| x == i, &|x| x == j);
                if prs.len() >= 3 && best.as_ref().is_none_or(|b| prs.len() > b.0) {
                    best = Some((prs.len(), j, prs));
                }
            }
            if let Some((n, j, mut prs)) = best {
                let kk = |q: &Pair| q.0.0 + q.0.1;
                let mut sorted = prs.clone();
                sorted.sort_by_key(|q| (kk(q), q.0));
                let target = kk(&sorted[(n / 5 + have * n / 4).min(n - 1)]);
                let (kind, name, border) = if ri.level() == ROLES[j as usize].level() {
                    (GateKind::Gap, "Gap", Border::Open)
                } else {
                    (GateKind::Stair, "Stair", Border::Joined)
                };
                if let Some(q) = pick(&mut prs, &kk, target, &used, 3) {
                    put(p, &mut used, q, kind, name, false, border);
                }
            }
        }
    }
    if let Some(o) = crown_way {
        let mut prs = pairs(p, &|x| x == crown, &|x| x == o);
        let kk = |q: &Pair| q.0.0 + q.0.1;
        prs.sort_by_key(|q| (kk(q), q.0));
        let t = kk(&prs[prs.len() / 2]);
        if let Some(q) = pick(&mut prs, &kk, t, &used, 3) {
            // Stored from the lower side so a/b read like every other gate.
            put(p, &mut used, (q.1, q.0), GateKind::RampRoad, "The Switchback", true, Border::Joined);
        }
        if let Some(q) = pick(&mut prs, &kk, t + 12, &used, 3) {
            put(p, &mut used, (q.1, q.0), GateKind::Verb(Verb::Electric), "The Company Lift", false, Border::Joined);
        }
    }
}

/// The mine mouth by the real roads: the Foothills cell that puts the Halt-to-mine walk nearest
/// six minutes (the first walk and the road to the Castle are what they are).
fn fit_mine(p: &mut MacroPlan) {
    let (_, _, road) = p.graphs(false);
    let (st, ju, town) = (p.site_cell("station"), p.site_cell("julie_house"), p.site_cell("town"));
    let first = p.dijkstra(&road, idx(st.0, st.1), None).0[idx(ju.0, ju.1)];
    let jt = p.dijkstra(&road, idx(ju.0, ju.1), None).0[idx(town.0, town.1)];
    if first == INF || jt == INF {
        return;
    }
    let from_town = p.dijkstra(&road, idx(town.0, town.1), None).0;
    let fh = Role::Foothills.id();
    let mut best: Option<(i32, Cell)> = None;
    for y in 0..MH {
        for x in 0..MW {
            let clear = (-1..=1)
                .all(|dy| (-1..=1).all(|dx| !inside(x + dx, y + dy) || p.district_of[idx(x + dx, y + dy)] == fh));
            let d = from_town[idx(x, y)];
            if p.district_of[idx(x, y)] != fh || !clear || d == INF {
                continue;
            }
            let key = ((first + jt + d) / 5 - 360).abs();
            if best.is_none_or(|b| key < b.0) {
                best = Some((key, (x, y)));
            }
        }
    }
    if let (Some((_, c)), Some(site)) = (best, p.sites.iter_mut().find(|s| s.id == "gold_mine")) {
        site.pos = (c.0 as u8, c.1 as u8);
    }
}

/// K5: roads over road-carrying gates only.
#[allow(clippy::items_after_statements)]
fn route_roads(p: &mut MacroPlan) {
    let (_, _, road) = p.graphs(false);
    const ROUTES: [(&str, &str); 14] = [
        ("station", "julie_house"),
        ("julie_house", "town"),
        ("julie_house", "farm"),
        ("town", "gold_mine"),
        ("town", "museum"),
        ("museum", "library"),
        ("museum", "reed_camp"),
        ("reed_camp", "lake_statue"),
        ("town", "graveyard"),
        ("graveyard", "factory"),
        ("factory", "canteen"),
        ("factory", "school"),
        ("factory", "library"),
        ("library", "butterfly_forest"),
    ];
    for (from, to) in ROUTES {
        let (s, t) = (p.site_cell(from), p.site_cell(to));
        let (src, dst) = (idx(s.0, s.1), idx(t.0, t.1));
        let (dist, par) = p.dijkstra(&road, src, Some(dst));
        if dist[dst] == INF {
            continue;
        }
        let mut pts = Vec::new();
        let mut c = dst as u32;
        while c != u32::MAX {
            pts.push((c as i32 % MW, c as i32 / MW));
            c = par[c as usize];
        }
        pts.reverse();
        p.routes.push(Route {
            from,
            to,
            pts: pts.into_iter().map(|(x, y)| (x as u8, y as u8)).collect(),
            cells: dist[dst],
        });
    }
}

/// A cheap screen on a roll's regions alone: each of a fair size, all three borders long enough
/// to hold their gates, and the upland within reach of the Lowfields (so the School can be seen).
pub fn region_screen(seed: u32, attempt: u8) -> bool {
    let (a, b, near, _) = screen_info(seed, attempt);
    a.iter().all(|&v| (1300..=4300).contains(&v)) && near && b.iter().all(|&v| v >= 28)
}

/// The numbers behind [`region_screen`]: region areas, border lengths (L-Wa, L-W, Wa-W), whether
/// the upland is within reach of the Lowfields, and the archetype.
pub fn screen_info(seed: u32, attempt: u8) -> ([i32; 3], [i32; 3], bool, &'static str) {
    let (reg, arch) = regions(seed, attempt, &Warp::new(seed, attempt, 1, 6, 2));
    let mut area = [0i32; 3];
    let (mut sx, mut sy) = (0, 0);
    for y in 0..MH {
        for x in 0..MW {
            let r = reg[idx(x, y)];
            area[r as usize] += 1;
            if r == Reg::Lowfields {
                sx += x;
                sy += y;
            }
        }
    }
    let (cx, cy) = (sx / area[0].max(1), sy / area[0].max(1));
    let dl = dist_from(&|c| reg[c] == Reg::Lowfields);
    let dw = dist_from(&|c| reg[c] == Reg::Waters);
    let mut border = [0i32; 3];
    let mut near = false;
    for y in 0..MH {
        for x in 0..MW {
            let r = reg[idx(x, y)];
            near |= r == Reg::Works && dl[idx(x, y)].min(dw[idx(x, y)]) >= 10 && (x - cx).abs() + (y - cy).abs() <= 52;
            for (dx, dy) in [(1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < MW && ny < MH {
                    let q = reg[idx(nx, ny)];
                    if q != r {
                        border[(r as usize + q as usize) - 1] += 1;
                    }
                }
            }
        }
    }
    (area, border, near, arch)
}

/// The plan for `seed`: the first attempt whose check is clean, or the last with its issues.
pub fn plan(seed: u32) -> (MacroPlan, Vec<super::macro_check::Issue>) {
    let mut last = None;
    for a in 0..ATTEMPTS {
        if !region_screen(seed, a) {
            continue;
        }
        let p = build(seed, a);
        let issues = super::macro_check::check(&p);
        if issues.is_empty() {
            return (p, issues);
        }
        last = Some((p, issues));
    }
    last.unwrap_or_else(|| (build(seed, 0), Vec::new()))
}

/// Region name, for labels.
pub fn reg_name(r: Reg) -> String {
    String::from(match r {
        Reg::Lowfields => "lowfields",
        Reg::Waters => "waters",
        Reg::Works => "works",
    })
}
