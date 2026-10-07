//! The open country: everything between the set places, furnished. Carries
//! `jane/src/world/country.ts`. The skeleton decided where the story stands and where the roads
//! go; the earlier stages drew the ground, the roads and the chunks; this fills the rest of the
//! county the way somewhere lived in and then left is filled.
//!
//! - **The roads** ([`furnish_roads`], [`roads`]): lamps at an even step on one side of each lit
//!   road, set just off its verge; a lamp at each end of a bridge; a fingerpost at every fork that
//!   names where each way goes and how far; milestones.
//! - **The places** ([`furnish_places`], [`places`], [`furnish`]): a fence, a hedge or a wall in
//!   straight runs along the field side of the roads; the places the stories will ask for, built
//!   first; something beside every road at a steady beat; then a jittered lattice over the whole
//!   county, one candidate every 41 cells, each the kind of place its ground allows. Every place
//!   built is a [`Place`] on [`County::places`], for the stories stage to claim.
//! - **The living** ([`furnish_life`], [`life`]): wildlife in every macro cell's chance, thicker
//!   with the threat and kept back from the roads; wanderers along the field edges; last, any
//!   screen of the county still empty gets something small of its own.
//!
//! Every choice is thrown on dice of its own ([`Step`]), named for the one thing it decides: a
//! road's fences, a beat of a road, a point of the lattice, a place's furnishings (by its kind and
//! where it stands), a macro cell's creature, an empty screen. So a change to one moves only what
//! it decides.
//!
//! Where this parts from the TypeScript:
//!
//! - integer throughout: a road's normal is a Q16 unit vector, distances walked along a line are
//!   thousandths of a cell, the pads' ellipses compare in whole numbers, `WILD` is permille, a
//!   weighted pick draws `below(total)`;
//! - the distance fields are `jane_core::search::chamfer` on the 4-cell grid (a diagonal step is
//!   5.6 cells rather than 6);
//! - the Kit's footprints come from the catalog (`put` never names a size);
//! - the story quota's shuffle is Fisher-Yates proper (the TypeScript's `roll(i + 1)` could swap
//!   past the end of the list);
//! - `FOLK_VARIANTS` was empty, so `variant` is the def itself;
//! - a lamp, a milestone or a relay box treats a bridge's planks as road, and a lane never paints
//!   inside a set place's box;
//! - a relay run's head is its first point clear of the set places, and a box or a dead lamp may
//!   stand east or west of a road that runs north and south (five seeds in 64 had no relay run);
//! - an empty screen with no room for a clearing gets flowers on any open cell.
//!
//! The tables that were constants in the TypeScript are Rust `const`s here, marked *tuning*: they
//! move to `data/tuning/country.json` with PORT.md §6.g. What a chest and an orchard hold is there
//! already (the catalog's `county.furnishing`, which an item's `replaceable` flag counts).

use alloc::vec;
use alloc::vec::Vec;

pub mod defs;
pub mod furnish;
pub mod life;
pub mod places;
pub mod relays;
pub mod roads;

use jane_core::blueprint::{PropSpawn, Waypoint};
use jane_core::num::{Permille, div_floor, div_round, isqrt};
use jane_core::search::chamfer;
use jane_core::{Grid, Key, Rect, Sfc32, Tick, Tile, UnitDefId};
use jane_data::{Region, RuinKind};

pub use self::places::Kind;
use super::County;
use crate::kit::js_round;
use crate::skeleton::{Biome, SKEL_H, SKEL_W, Skeleton, Water};

/// Distances are kept on a 4-cell grid: fine enough to stand a camp by, sixteen times cheaper.
pub const DB: i32 = 4;
/// Nothing that bites stands nearer a road than this (cells): its eye is 16 m. *Tuning.*
pub const ROAD_CLEAR: u8 = 22;
/// Camps stand back from the road by this much, so a careful walker passes them. *Tuning.*
pub const CAMP_BACK: u8 = 26;
/// The Works' road is walkable with care, and the ground off it as harsh as ever: its creatures
/// see 18 m by day and half as far again after dark, so they stand this much further back from a
/// road there (the sweep of 28 September 2026: every model died on the Works' roads). *Tuning.*
pub const WORKS_BACK: u8 = 14;

/// Nearer a road than this nothing that bites stands, on the ground of `region`.
pub const fn road_clear(region: Region) -> u8 {
    match region {
        Region::Works => ROAD_CLEAR + WORKS_BACK,
        _ => ROAD_CLEAR,
    }
}

/// How far back from a road a camp or a den stands, on the ground of `region`.
pub const fn camp_back(region: Region) -> u8 {
    match region {
        Region::Works => CAMP_BACK + WORKS_BACK,
        _ => CAMP_BACK,
    }
}
/// Nothing that bites within this of the first walk (station, Julie's, the town). *Tuning.*
pub const FIRST_CLEAR: u8 = 44;
/// A distance field's reading off the grid, and its cap.
const FAR: u8 = u8::MAX;
/// Q16's one: a road's normal is a unit vector in sixteenths of sixteen bits.
const Q: i64 = 1 << 16;

/// Something the builder made that a story may claim: a hamlet, a farm, a cottage, an inn, a
/// woodcutters' clearing, a camp, a ruin (and every other kind, which no story claims yet). It
/// remembers where it stands and what is in it, by name, so a story row can say "the hen house at
/// some farm in the Lowfields" without knowing where any farm is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// The place's number on this seed: its top-left cell (`y * COUNTY_W + x`), so it names the
    /// place by where it stands and not by how many were built before it. A story's pick and a
    /// board's key hash from it.
    pub n: i32,
    pub kind: Kind,
    pub region: Region,
    /// The daytime threat of the macro cell under its centre.
    pub threat: u8,
    /// The footprint. All of it is claimed once the place is built.
    pub bounds: Rect,
    /// Cells from the centre to the nearest road or footpath, and to the first walk (the 4-cell
    /// fields, capped at 255).
    pub road: u8,
    pub first: u8,
    /// Open cells kept free inside the place for a story to use, by name: `board` (two cells wide,
    /// on the edge that faces the road), `yard`, `green`, `plot`, `field`, `pen`, `garden`, `back`,
    /// `clearing`, `ground`, `inside`, `step`. Each is claimed, and the cell below it is open.
    pub slots: Vec<(&'static str, (i32, i32))>,
    /// The place's own props by name (`house`, `house_2`, `well`, `barn`, `coop`, `chest`, ...),
    /// as prop keys. The first of a name wins.
    pub things: Vec<(&'static str, Key)>,
    /// The people who live there (units whose def starts `folk_`), in the order they were stood.
    pub folk: Vec<Key>,
    /// What bites, for a camp or a den.
    pub hostiles: Vec<Key>,
}

impl Place {
    /// A kept cell by name.
    pub fn slot(&self, name: &str) -> Option<(i32, i32)> {
        self.slots.iter().find(|(n, _)| *n == name).map(|&(_, c)| c)
    }

    /// One of its props by name.
    pub fn thing(&self, name: &str) -> Option<Key> {
        self.things.iter().find(|(n, _)| *n == name).map(|&(_, k)| k)
    }

    /// Twice its centre: exact for an odd width.
    pub fn centre2(&self) -> (i32, i32) {
        (2 * self.bounds.x + self.bounds.w, 2 * self.bounds.y + self.bounds.h)
    }
}

/// What the three stages share (the TypeScript's cached context, passed rather than cached,
/// PORT.md §6.e). Built by the first of them from the lines laid so far.
#[derive(Debug)]
pub struct Ctx {
    /// Cells to the nearest road, path or footpath, on the 4-cell grid, capped at 255.
    pub d_road: Grid<u8>,
    /// The same to the first walk's two roads only.
    pub d_first: Grid<u8>,
    /// Per line: whether it is one of the first walk's two roads.
    pub first_lines: Vec<bool>,
    /// Every lamp stood so far, for the spacing.
    lamp_at: Vec<(i32, i32)>,
    /// The place being stamped, while it is stamped: what it puts, it owns.
    cur: Option<Place>,
    /// A ruin built for a tale is the one the tale asked for; unset, the dice say.
    ruin_as: Option<RuinKind>,
}

impl Ctx {
    /// The distance fields dropped, once nothing reads them (`County::release_after`).
    pub(crate) fn release_fields(&mut self) {
        self.d_road = Grid::new(0, 0, FAR);
        self.d_first = Grid::new(0, 0, FAR);
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            d_road: Grid::new(0, 0, FAR),
            d_first: Grid::new(0, 0, FAR),
            first_lines: Vec::new(),
            lamp_at: Vec::new(),
            cur: None,
            ruin_as: None,
        }
    }
}

/// Build the context on first use, from the lines laid so far.
fn ready(c: &mut County<'_>) {
    if c.country.d_road.w() != 0 {
        return;
    }
    let named = c.sk.named;
    let first = named.first_walk();
    let mut first_lines = vec![false; c.lines.len()];
    for (n, r) in c.sk.roads.iter().enumerate() {
        first_lines[n] = first.contains(&(r.from, r.to));
    }
    c.country.d_road = distance_field(c.k.w(), c.k.h(), &c.lines, |_| true);
    c.country.d_first = distance_field(c.k.w(), c.k.h(), &c.lines, |n| first_lines[n]);
    c.country.first_lines = first_lines;
}

/// Chamfer distance, in cells on the 4-cell grid, from every point of the lines `use_line` picks.
pub fn distance_field(w: i32, h: i32, lines: &[Vec<(i32, i32)>], use_line: impl Fn(usize) -> bool) -> Grid<u8> {
    let (dw, dh) = ((w + DB - 1) / DB, (h + DB - 1) / DB);
    let mut on = Grid::new(dw as u32, dh as u32, false);
    for (n, line) in lines.iter().enumerate() {
        if use_line(n) {
            for &(x, y) in line {
                on.set(x.div_euclid(DB), y.div_euclid(DB), true);
            }
        }
    }
    let tenths = chamfer(dw as u32, dh as u32, |x, y| on.read(x, y, false));
    let cells = tenths.as_slice().iter().map(|&t| div_round(i64::from(t) * i64::from(DB), 10).min(255) as u8).collect();
    Grid::from_vec(dw as u32, dh as u32, cells)
}

/// A distance field's reading at a cell; 255 off the grid.
pub fn dist(field: &Grid<u8>, x: i32, y: i32) -> u8 {
    field.read(x.div_euclid(DB), y.div_euclid(DB), FAR)
}

/// The macro cell under a cell, clamped to the skeleton.
pub fn macro_of(x: i32, y: i32) -> (i32, i32) {
    ((x >> 4).clamp(0, SKEL_W - 1), (y >> 4).clamp(0, SKEL_H - 1))
}

/// What the skeleton says of the macro cell under a cell.
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub region: Region,
    pub biome: Biome,
    pub threat: u8,
    pub wet: bool,
}

pub fn ground(sk: &Skeleton, x: i32, y: i32) -> Ground {
    let (mx, my) = macro_of(x, y);
    Ground {
        region: sk.region_at(mx, my),
        biome: sk.terrain.biome.read(mx, my, Biome::Field),
        threat: sk.threat.read(mx, my, 0),
        wet: sk.terrain.water.read(mx, my, Water::Dry) != Water::Dry,
    }
}

/// Within `margin` of a set place's box.
pub fn near_chunk(c: &County<'_>, x: i32, y: i32, margin: i32) -> bool {
    c.chunks.iter().any(|ch| ch.bounds.grow(margin).contains(x, y))
}

/// Inside a set place's box: the chunk's own cells, which nothing of the country's paints over
/// (a lane, a small place's pad and outskirt). The TypeScript let a lane cross a box's grass.
pub fn in_box(c: &County<'_>, x: i32, y: i32) -> bool {
    near_chunk(c, x, y, 0)
}

/// What the builder may build over: open ground and growth, never water, a road or anything made.
pub const fn buildable(t: Tile) -> bool {
    matches!(
        t,
        Tile::Grass
            | Tile::GrassTall
            | Tile::Dirt
            | Tile::Bush
            | Tile::Tree
            | Tile::DeadTree
            | Tile::Moss
            | Tile::Garden
            | Tile::Crops
            | Tile::FlowerBed
            | Tile::DryBed
            | Tile::Sand
            | Tile::Rubble
            | Tile::Cobble
            | Tile::Track
            | Tile::Rail
    )
}

/// Ground a lane may be laid over.
const fn soft(t: Tile) -> bool {
    matches!(
        t,
        Tile::Grass
            | Tile::GrassTall
            | Tile::Bush
            | Tile::Tree
            | Tile::DeadTree
            | Tile::Moss
            | Tile::DryBed
            | Tile::Sand
    )
}

// --- the three stages ----------------------------------------------------------------------

/// The roads' furniture: a lamp at each end of every bridge, a fingerpost at every fork, lamps
/// along the lit stretches, milestones. No dice: it is all geometry.
pub fn furnish_roads(c: &mut County<'_>) {
    ready(c);
    roads::bridges(c);
    roads::forks(c);
    roads::lamps(c);
    roads::milestones(c);
}

/// Field edges along the roads, then the places.
pub fn furnish_places(c: &mut County<'_>) {
    ready(c);
    places::fences(c);
    places::places(c);
}

/// Wildlife, wanderers, and something small on every screen still empty; then what stands up
/// after the bell, which fills no screen by day.
pub fn furnish_life(c: &mut County<'_>) {
    ready(c);
    life::yard_bones(c);
    life::wildlife(c);
    life::wanderers(c);
    life::gaps(c);
    life::night_shift(c);
}

// --- small building tools ------------------------------------------------------------------

/// Every cell of the rect is open or growing ground nobody has claimed, eight cells in from the
/// county's edge.
pub fn room(c: &County<'_>, x0: i32, y0: i32, w: i32, h: i32) -> bool {
    let k = &c.k;
    if x0 < 8 || y0 < 8 || x0 + w > k.w() - 8 || y0 + h > k.h() - 8 {
        return false;
    }
    (y0..y0 + h).all(|y| (x0..x0 + w).all(|x| buildable(k.get(x, y)) && !k.is_claimed(x, y)))
}

/// Growth gives way: trees, bushes, rubble become the ground a place stands on.
pub fn clear(c: &mut County<'_>, r: Rect, t: Tile) {
    for (x, y) in r.cells() {
        if matches!(c.k.get(x, y), Tile::Tree | Tile::DeadTree | Tile::Bush | Tile::Rubble) {
            c.k.set(x, y, t);
        }
    }
}

/// `i² / rx² + j² / ry²` against `hundredths / 100`, with the radii given doubled (`rx2 = 2 rx`,
/// so a radius of 2.5 is 5): `4 (i² ry2² + j² rx2²) * 100 <= hundredths * rx2² ry2²`.
pub fn ellipse_within(i: i32, j: i32, rx2: i32, ry2: i32, hundredths: i64) -> bool {
    let (i, j, a, b) = (i64::from(i), i64::from(j), i64::from(rx2 * rx2), i64::from(ry2 * ry2));
    400 * (i * i * b + j * j * a) <= hundredths * a * b
}

/// Worn ground: an ellipse of `t` with a ragged rim, radii doubled as in [`ellipse_within`].
pub fn pad(c: &mut County<'_>, rng: &mut Sfc32, cx: i32, cy: i32, rx2: i32, ry2: i32, t: Tile) {
    // ceil(r) + 1 from a doubled radius.
    let (jn, inn) = ((ry2 + 1) / 2 + 1, (rx2 + 1) / 2 + 1);
    for j in -jn..=jn {
        for i in -inn..=inn {
            let (x, y) = (cx + i, cy + j);
            if !buildable(c.k.get(x, y)) || c.k.is_claimed(x, y) {
                continue;
            }
            if ellipse_within(i, j, rx2, ry2, 75) || (ellipse_within(i, j, rx2, ry2, 115) && rng.chance(Permille(600)))
            {
                c.k.set(x, y, t);
            }
        }
    }
}

/// A prop of `def` at `(x, y)` if its footprint is open, with `f` to set the rest of the row.
pub fn put_with(
    c: &mut County<'_>,
    def: jane_core::PropDefId,
    x: i32,
    y: i32,
    f: impl FnOnce(&mut PropSpawn),
) -> Option<Key> {
    let row = jane_data::catalog().story.prop(def);
    if !c.k.fits(x, y, i32::from(row.w), i32::from(row.h), 0) {
        return None;
    }
    let p = c.k.prop(None, def, x, y);
    f(p);
    Some(p.key)
}

/// A prop of `def` at `(x, y)` if its footprint is open.
pub fn put(c: &mut County<'_>, def: jane_core::PropDefId, x: i32, y: i32) -> Option<Key> {
    put_with(c, def, x, y, |_| {})
}

/// Deadwood by a fire pit (PLAY-PLAN.md §2.2: "deadwood within 12 cells of every cold pit"): a
/// stump of `def` at the first open cell in rings three to eight cells out from `(x, y)`, the
/// ring's cells in a fixed order. No dice. `None` if every cell is taken (the L1 test says so).
pub fn wood_by(c: &mut County<'_>, def: jane_core::PropDefId, x: i32, y: i32) -> Option<Key> {
    for r in 3..=8 {
        for i in -r..=r {
            for (dx, dy) in [(i, -r), (r, i), (-i, r), (-r, -i)] {
                if let Some(k) = put(c, def, x + dx, y + dy) {
                    return Some(k);
                }
            }
        }
    }
    None
}

/// A prop the place being stamped is known by ("the well", "the barn"), for a story to find by
/// name. The first of a name wins.
pub fn own(c: &mut County<'_>, name: &'static str, p: Option<Key>) -> Option<Key> {
    if let (Some(key), Some(cur)) = (p, c.country.cur.as_mut()) {
        if cur.thing(name).is_none() {
            cur.things.push((name, key));
        }
    }
    p
}

/// Keep open ground near `(x, y)` free for a story: the yard, the garden, the green. Taken at the
/// end of a place's build, out of what the build left open, in rings out to three cells; the cell
/// below it must be open too, somewhere to stand in front of it. No dice.
pub fn keep(c: &mut County<'_>, name: &'static str, x: i32, y: i32) {
    if c.country.cur.is_none() {
        return;
    }
    for r in 0..=3i32 {
        for oy in -r..=r {
            for ox in -r..=r {
                if ox.abs().max(oy.abs()) != r {
                    continue;
                }
                let (px, py) = (x + ox, y + oy);
                let k = &c.k;
                if !k.fits(px, py, 1, 1, 0)
                    || k.solid(px, py + 1)
                    || k.is_claimed(px, py + 1)
                    || k.get(px, py) == Tile::Water
                {
                    continue;
                }
                c.k.claim(Rect::new(px, py, 1, 1));
                if let Some(cur) = c.country.cur.as_mut() {
                    cur.slots.push((name, (px, py)));
                }
                return;
            }
        }
    }
}

/// Somebody who lives here: walks a short round between a few points, and stands at each.
pub fn folk(c: &mut County<'_>, rng: &mut Sfc32, def: UnitDefId, x: i32, y: i32, reach: i32) {
    if c.k.solid(x, y) || c.k.is_claimed(x, y) {
        return;
    }
    let mut patrol = vec![waypoint(x, y, 120 + rng.irandom(240))];
    for _ in 0..2 {
        let px = x + rng.range(-reach, reach);
        let py = y + rng.range(-reach, reach);
        if !c.k.solid(px, py) {
            patrol.push(waypoint(px, py, 90 + rng.irandom(300)));
        }
    }
    if patrol.len() < 2 {
        patrol.clear();
    }
    let key = c.k.unit(None, def, x, y, patrol).key;
    if jane_data::catalog().combat.unit(def).id.starts_with("folk_") {
        if let Some(cur) = c.country.cur.as_mut() {
            cur.folk.push(key);
        }
    }
}

/// A stop on a round: where, and how many ticks it stands there.
pub fn waypoint(x: i32, y: i32, ticks: i32) -> Waypoint {
    Waypoint { cell: crate::kit::cell(x, y), dwell: Tick(ticks.unsigned_abs()) }
}

/// Something that bites, playing at the threat of the ground it stands on: never on something
/// solid, claimed or wet, never in a haven (threat 0), never within [`FIRST_CLEAR`] of the first
/// walk. True if it stood.
pub fn hostile(c: &mut County<'_>, def: UnitDefId, x: i32, y: i32, patrol: Vec<Waypoint>) -> bool {
    if c.k.solid(x, y) || c.k.is_claimed(x, y) || c.k.get(x, y) == Tile::Water {
        return false;
    }
    let threat = ground(c.sk, x, y).threat;
    if threat == 0 || dist(&c.country.d_first, x, y) < FIRST_CLEAR {
        return false;
    }
    let u = c.k.unit(None, def, x, y, patrol);
    u.phase = threat;
    let key = u.key;
    if let Some(cur) = c.country.cur.as_mut() {
        cur.hostiles.push(key);
    }
    true
}

/// The way from `(x, y)` down the distance field to the nearest road or footpath, as the cells of
/// its centre line in order (a joint twice): thirty steps of the 4-cell grid at most, straight steps
/// before diagonal ones, stopping two cells short of the line.
pub fn downhill(field: &Grid<u8>, x: i32, y: i32) -> Vec<(i32, i32)> {
    let (mut bx, mut by) = (x.div_euclid(DB), y.div_euclid(DB));
    let (mut px, mut py) = (x, y);
    let (dw, dh) = (field.w() as i32, field.h() as i32);
    let mut out = Vec::new();
    for _ in 0..30 {
        let here = field.read(bx, by, FAR);
        if here <= 2 {
            break;
        }
        let (mut best, mut nx, mut ny) = (here, bx, by);
        // Straight steps first: a lane runs square to things where it can.
        for (ox, oy) in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
            let (qx, qy) = (bx + ox, by + oy);
            if qx < 0 || qy < 0 || qx >= dw || qy >= dh {
                continue;
            }
            let v = field.read(qx, qy, FAR);
            if v < best {
                (best, nx, ny) = (v, qx, qy);
            }
        }
        if (nx, ny) == (bx, by) {
            break;
        }
        let (tx, ty) = (nx * DB + 1, ny * DB + 1);
        let n = (tx - px).abs().max((ty - py).abs());
        for s in 0..=n {
            let lx = px + js_round(i64::from((tx - px) * s), i64::from(n.max(1))) as i32;
            let ly = py + js_round(i64::from((ty - py) * s), i64::from(n.max(1))) as i32;
            out.push((lx, ly));
        }
        (px, py, bx, by) = (tx, ty, nx, ny);
    }
    out
}

/// Where a way is laid to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    /// A cell of road under its centre line.
    Road,
    /// Road, or a way already trodden.
    Way,
    /// Road, a way, or any worn ground: a green, a yard.
    Worn,
}

/// How far a lane's way is looked for round where it starts, each way.
const LANE_REACH: i32 = 120;
/// How far round a way to a known cell of road may stray outside the box of its two ends.
const LANE_MARGIN: i32 = 24;
/// What a step of lane costs, and what it costs more to turn, to clear growth, to cross a
/// place's claimed ground, to walk over a bed or a crop row, and to open a fence for a gate.
const STEP: u32 = 10;
const TURN: u32 = 14;
const GROWTH: u32 = 4;
const CLAIMED: u32 = 4;
const BEDS: u32 = 160;
const GATE: u32 = 400;
const RUBBLE: u32 = 20;

/// Every place's board (put up later, by the stories), and the cells in front of it she reads it
/// from: no way is trodden there.
fn board_cells(c: &County<'_>) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for p in c.places.iter().chain(c.country.cur.as_ref()) {
        for &(name, (bx, by)) in &p.slots {
            if name == "board" {
                out.extend([(bx, by), (bx + 1, by), (bx, by + 1), (bx + 1, by + 1)]);
            }
        }
    }
    out
}

/// The way a lane `width` wide takes from `(x, y)` to the road: the cheapest walk to a cell of
/// road, or of a way already trodden, round every building, rock and trunk (their feet), every
/// set place's box, water and whatever else stops feet, preferring to run straight, to keep off
/// beds and crops, and to go round a fence rather than through it (where it must, it gets a
/// gate). Its centre line from `(x, y)`; `None` if nothing is in reach.
fn route(
    c: &County<'_>,
    x: i32,
    y: i32,
    width: i32,
    reach: Reach,
    toward: Option<(i32, i32)>,
) -> Option<Vec<(i32, i32)>> {
    use alloc::collections::BinaryHeap;
    use core::cmp::Reverse;
    const DIRS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    let cat = jane_data::catalog();
    // Round the start, or round both ends of a way to a known cell of road.
    let win = match toward {
        None => Rect::new(x - LANE_REACH, y - LANE_REACH, 2 * LANE_REACH + 1, 2 * LANE_REACH + 1),
        Some((tx, ty)) => {
            let (x0, y0) = (x.min(tx) - LANE_MARGIN, y.min(ty) - LANE_MARGIN);
            Rect::new(x0, y0, x.max(tx) + LANE_MARGIN - x0 + 1, y.max(ty) + LANE_MARGIN - y0 + 1)
        }
    };
    let win = win.intersect(Rect::new(0, 0, c.k.w(), c.k.h()))?;
    if !win.contains(x, y) {
        return None;
    }
    let (ww, wh) = (win.w, win.h);
    let ix = |cx: i32, cy: i32| ((cy - win.y) * ww + (cx - win.x)) as usize;
    // What stops a lane's cells: every solid prop's feet in the window.
    let mut feet = vec![false; (ww * wh) as usize];
    for p in &c.k.blueprint().props {
        let d = cat.story.prop(p.def);
        if !super::ways::stops_feet(d, p) {
            continue;
        }
        let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
        if !r.overlaps(win) {
            continue;
        }
        for (fx, fy) in super::ways::feet_cells(d, r.x, r.y) {
            if win.contains(fx, fy) {
                feet[ix(fx, fy)] = true;
            }
        }
    }
    for (fx, fy) in board_cells(c) {
        if win.contains(fx, fy) {
            feet[ix(fx, fy)] = true;
        }
    }
    let boxes: Vec<Rect> = c.chunks.iter().map(|ch| ch.bounds).filter(|b| b.grow(1).overlaps(win)).collect();
    let cell_cost = |cx: i32, cy: i32| -> Option<u32> {
        if !win.contains(cx, cy) || feet[ix(cx, cy)] || boxes.iter().any(|b| b.contains(cx, cy)) {
            return None;
        }
        let t = c.k.get(cx, cy);
        Some(match t {
            Tile::Fence | Tile::StoneWall | Tile::Hedge => GATE,
            Tile::Crops | Tile::Garden | Tile::FlowerBed => BEDS,
            Tile::Tree | Tile::DeadTree | Tile::Bush => GROWTH,
            Tile::Rubble => RUBBLE,
            Tile::Water => return None,
            _ if t.flags() & jane_core::tile::F_SOLID != 0 => return None,
            Tile::Road | Tile::Boardwalk | Tile::Dirt | Tile::Cobble => 0,
            _ if c.k.is_claimed(cx, cy) => CLAIMED,
            _ => 0,
        })
    };
    let brush_cost = |cx: i32, cy: i32| -> Option<u32> {
        let mut sum = 0;
        for j in 0..width {
            for i in 0..width {
                sum += cell_cost(cx + i, cy + j)?;
            }
        }
        Some(sum)
    };
    let goal = |cx: i32, cy: i32| {
        if toward == Some((cx, cy)) {
            return true;
        }
        if reach == Reach::Road {
            return c.k.get(cx, cy) == Tile::Road;
        }
        (0..width).any(|j| {
            (0..width).any(|i| {
                let (gx, gy) = (cx + i, cy + j);
                let t = c.k.get(gx, gy);
                matches!(t, Tile::Road | Tile::Boardwalk)
                    || (reach == Reach::Worn && matches!(t, Tile::Dirt | Tile::Cobble))
                    || super::ways::trodden_at(c, gx, gy)
            })
        })
    };
    // A lane set to start on a board, or on anything else in the way, starts beside it.
    let (x, y) = if brush_cost(x, y).is_some() {
        (x, y)
    } else {
        let mut near = None;
        'rings: for r in 1..=3i32 {
            for j in -r..=r {
                for i in -r..=r {
                    if i.abs().max(j.abs()) == r && brush_cost(x + i, y + j).is_some() {
                        near = Some((x + i, y + j));
                        break 'rings;
                    }
                }
            }
        }
        near?
    };
    // Dijkstra over (cell, heading): the heading makes a turn cost. Ties go to the lower state
    // index, so the same ground always gives the same lane.
    let n = (ww * wh) as usize;
    let mut best = vec![u32::MAX; n * 4];
    let mut from = vec![u32::MAX; n * 4];
    let mut heap = BinaryHeap::new();
    for d in 0..4 {
        best[ix(x, y) * 4 + d] = 0;
        heap.push(Reverse((0u32, (ix(x, y) * 4 + d) as u32)));
    }
    let mut end = None;
    while let Some(Reverse((cost, s))) = heap.pop() {
        let s = s as usize;
        if cost > best[s] {
            continue;
        }
        let (i, d) = (s / 4, s % 4);
        let (cx, cy) = (win.x + (i as i32 % ww), win.y + (i as i32 / ww));
        if goal(cx, cy) {
            end = Some(s);
            break;
        }
        for (nd, &(ox, oy)) in DIRS.iter().enumerate() {
            let (nx, ny) = (cx + ox, cy + oy);
            let Some(extra) = brush_cost(nx, ny) else { continue };
            let turn = if nd == d { 0 } else { TURN };
            let next = cost + STEP + turn + extra;
            let t = ix(nx, ny) * 4 + nd;
            if next < best[t] {
                best[t] = next;
                from[t] = s as u32;
                heap.push(Reverse((next, t as u32)));
            }
        }
    }
    let mut s = end?;
    let mut line = Vec::new();
    loop {
        let i = s / 4;
        line.push((win.x + (i as i32 % ww), win.y + (i as i32 / ww)));
        let f = from[s];
        if f == u32::MAX {
            break;
        }
        s = f as usize;
    }
    line.reverse();
    Some(line)
}

/// A lane of worn dirt `width` wide from a place's front to the nearest road or way already
/// trodden ([`route`]; down the distance field, [`downhill`], if none is in reach). A fence it
/// must cross gets a gate. Its cells are claimed as they are trodden: nothing placed after
/// stands on it.
pub fn lane(c: &mut County<'_>, x: i32, y: i32, width: i32, door: Option<(i32, i32)>) {
    tread_way(c, (x, y), width, door, Reach::Way, None, super::ways::WayKind::Lane);
}

/// A walk two cells wide from a door's step to the nearest worn ground (a green, a yard, a lane).
pub fn walk(c: &mut County<'_>, (x, y): (i32, i32)) {
    tread_way(c, (x, y), 2, Some((x, y)), Reach::Worn, None, super::ways::WayKind::Lane);
}

/// [`lane`], [`walk`] and a story's path: to what `reach` says, or to `toward` (a known
/// cell of road), recorded as a way of `kind`. Its centre line, from `(x, y)`.
pub fn tread_way(
    c: &mut County<'_>,
    (x, y): (i32, i32),
    width: i32,
    door: Option<(i32, i32)>,
    reach: Reach,
    toward: Option<(i32, i32)>,
    kind: super::ways::WayKind,
) -> Vec<(i32, i32)> {
    let routed = route(c, x, y, width, reach, toward);
    let line = match &routed {
        Some(l) => l.clone(),
        None if reach == Reach::Worn => vec![(x, y)],
        None => match toward {
            // Straight to the road, as the way to a story's place always went.
            Some((tx, ty)) => {
                let n = (tx - x).abs().max((ty - y).abs());
                let steps = i64::from(n.max(1));
                (0..=n)
                    .map(|s| {
                        let at = |a: i32, b: i32| a + js_round(i64::from(b - a) * i64::from(s), steps) as i32;
                        (at(x, tx), at(y, ty))
                    })
                    .collect()
            }
            None => downhill(&c.country.d_road, x, y),
        },
    };
    // Down the field, nothing has looked for what stands in the way: it is trodden round.
    let mut under = Vec::new();
    if routed.is_none() {
        under = board_cells(c);
        let cat = jane_data::catalog();
        let (x0, y0) = line.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
        let (x1, y1) = line.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
        let span = Rect::new(x0 - 1, y0 - 1, x1 - x0 + width + 2, y1 - y0 + width + 2);
        for p in &c.k.blueprint().props {
            let d = cat.story.prop(p.def);
            let r = Rect::new(i32::from(p.cell.x), i32::from(p.cell.y), i32::from(d.w), i32::from(d.h));
            if super::ways::stops_feet(d, p) && r.overlaps(span) {
                under.extend(super::ways::feet_cells(d, r.x, r.y));
            }
        }
    }
    for &(lx, ly) in &line {
        for j in 0..width {
            for i in 0..width {
                let (cx, cy) = (lx + i, ly + j);
                if in_box(c, cx, cy) || !c.k.inside(cx, cy) || under.contains(&(cx, cy)) {
                    continue;
                }
                let t = c.k.get(cx, cy);
                let paint = if routed.is_some() {
                    // The route went only where feet may go once growth is cleared and a fence
                    // opened: all of it is trodden, road and cobbles kept.
                    matches!(
                        t,
                        Tile::Grass
                            | Tile::GrassTall
                            | Tile::Moss
                            | Tile::DryBed
                            | Tile::Sand
                            | Tile::Tree
                            | Tile::DeadTree
                            | Tile::Bush
                            | Tile::Rubble
                            | Tile::Fence
                            | Tile::StoneWall
                            | Tile::Hedge
                            | Tile::Crops
                            | Tile::Garden
                            | Tile::FlowerBed
                    )
                } else if matches!(t, Tile::Fence | Tile::StoneWall | Tile::Hedge)
                    && (toward.is_some() || dist(&c.country.d_road, cx, cy) <= 6)
                {
                    true
                } else if matches!(t, Tile::Dirt | Tile::Cobble) {
                    false
                } else if !soft(t) || (c.k.is_claimed(cx, cy) && !matches!(t, Tile::Grass | Tile::GrassTall)) {
                    // Over growth and open grass only; claimed ground is crossed only where it is
                    // still grass (a road's margin).
                    continue;
                } else {
                    true
                };
                if paint {
                    c.k.set(cx, cy, Tile::Dirt);
                }
                if matches!(c.k.get(cx, cy), Tile::Dirt | Tile::Cobble) {
                    super::ways::tread(&mut c.trodden, &c.k, cx, cy);
                    c.k.claim(Rect::new(cx, cy, 1, 1));
                }
            }
        }
    }
    c.ways.push(super::ways::Way { kind, line: line.clone(), door });
    line
}

/// A straight fence, hedge or wall with a gate of three every `gate_every` cells (0: none);
/// never across a path, a road or anything claimed.
pub fn run(c: &mut County<'_>, x0: i32, y0: i32, len: i32, horizontal: bool, t: Tile, gate_every: i32) {
    for i in 0..len {
        let (x, y) = if horizontal { (x0 + i, y0) } else { (x0, y0 + i) };
        if gate_every > 0 && i % gate_every >= gate_every - 3 {
            continue;
        }
        let was = c.k.get(x, y);
        if !matches!(was, Tile::Grass | Tile::GrassTall | Tile::Moss | Tile::Bush | Tile::Tree) || c.k.is_claimed(x, y)
        {
            continue;
        }
        c.k.set(x, y, t);
    }
}

/// A side of a rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    N,
    S,
    E,
    W,
}

/// A closed fence round a rect, with a gate of three in the middle of one side.
pub fn pen(c: &mut County<'_>, r: Rect, gate: Side, t: Tile) {
    let g = |i: i32, len: i32| i >= (len >> 1) - 1 && i <= (len >> 1) + 1;
    for i in 0..r.w {
        if !(gate == Side::N && g(i, r.w)) {
            c.k.set(r.x + i, r.y, t);
        }
        if !(gate == Side::S && g(i, r.w)) {
            c.k.set(r.x + i, r.y + r.h - 1, t);
        }
    }
    for j in 0..r.h {
        if !(gate == Side::W && g(j, r.h)) {
            c.k.set(r.x, r.y + j, t);
        }
        if !(gate == Side::E && g(j, r.h)) {
            c.k.set(r.x + r.w - 1, r.y + j, t);
        }
    }
}

/// How far out [`road_side`] looks for the road.
const SIDE_LOOK: i32 = 12;

/// Which way the road is from here, as a side of a rect (south when nothing is nearer).
pub fn road_side(c: &County<'_>, x: i32, y: i32) -> Side {
    let mut best = FAR;
    let mut side = Side::S;
    for (s, ox, oy) in
        [(Side::S, 0, SIDE_LOOK), (Side::N, 0, -SIDE_LOOK), (Side::E, SIDE_LOOK, 0), (Side::W, -SIDE_LOOK, 0)]
    {
        let v = dist(&c.country.d_road, x + ox, y + oy);
        if v < best {
            best = v;
            side = s;
        }
    }
    side
}

/// A line's unit normal at a point, to the right of travel, in Q16: from five points back to
/// five on. Zero where the line stands still.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Normal {
    pub x: i64,
    pub y: i64,
}

impl Normal {
    pub fn of(line: &[(i32, i32)], i: usize) -> Self {
        let a = line[i.saturating_sub(5)];
        let b = line[(i + 5).min(line.len() - 1)];
        let (dx, dy) = (i64::from(b.0 - a.0), i64::from(b.1 - a.1));
        let len = i64::from(isqrt(((dx * dx + dy * dy) as u64) << 32));
        if len == 0 {
            return Self { x: 0, y: 0 };
        }
        Self { x: div_round(-dy * Q * Q, len), y: div_round(dx * Q * Q, len) }
    }

    /// `p` moved `dist` along the normal (`dist` in Q16, signed), rounded as JavaScript rounds.
    pub fn along(self, p: (i32, i32), dist: i64) -> (i32, i32) {
        (p.0 + js_round(self.x * dist, Q * Q) as i32, p.1 + js_round(self.y * dist, Q * Q) as i32)
    }

    /// `p` moved `cells` whole cells along the normal.
    pub fn cells(self, p: (i32, i32), cells: i32) -> (i32, i32) {
        self.along(p, i64::from(cells) * Q)
    }

    /// Far enough out, in Q16, that a `w x h` footprint centred there clears the line by `gap`.
    pub fn reach(self, w: i32, h: i32, gap: i32) -> i64 {
        div_floor(self.x.abs() * i64::from(w) + self.y.abs() * i64::from(h), 2) + i64::from(gap) * Q
    }
}

/// The length of one step along a line, in thousandths of a cell.
pub fn step_milli(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(b.0 - a.0), i64::from(b.1 - a.1));
    i64::from(isqrt(((dx * dx + dy * dy) * 1_000_000) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_is_a_unit_vector_to_the_right_of_travel() {
        let east: Vec<(i32, i32)> = (0..20).map(|x| (x, 5)).collect();
        let n = Normal::of(&east, 10);
        assert_eq!(n, Normal { x: 0, y: Q });
        assert_eq!(n.cells((10, 5), 3), (10, 8));
        let diag: Vec<(i32, i32)> = (0..20).map(|i| (i, i)).collect();
        let n = Normal::of(&diag, 10);
        assert_eq!(n.cells((10, 10), 3), (8, 12));
        assert_eq!(n.reach(10, 10, 0) / Q, 7);
    }

    #[test]
    fn ellipses_compare_like_the_floats_did() {
        // rx 2.5, ry 2: (2, 0) is 0.64 in; (0, 2) is 1.0.
        assert!(ellipse_within(2, 0, 5, 4, 64));
        assert!(!ellipse_within(2, 0, 5, 4, 63));
        assert!(ellipse_within(0, 2, 5, 4, 100));
        assert!(!ellipse_within(0, 2, 5, 4, 99));
    }

    #[test]
    fn steps_along_a_line_measure_true() {
        assert_eq!(step_milli((0, 0), (1, 0)), 1000);
        assert_eq!(step_milli((0, 0), (1, 1)), 1414);
        assert_eq!(step_milli((3, 3), (3, 3)), 0);
    }
}
