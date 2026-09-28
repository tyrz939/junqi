//! Where the places go: the field edges along the roads, the stories' quota first, then something
//! beside every road at a steady beat, then the lattice over the whole county. What each place
//! holds is [`super::furnish`].

use jane_core::num::Permille;
use jane_core::{Rect, Sfc32, Tile, pick_weighted};
use jane_data::{PlaceKind, Region, RuinKind};

use super::{FIRST_CLEAR, Normal, Place, Q, camp_back, clear, dist, distance_field, ground, near_chunk, room, run};
use crate::county::{County, centre};
use crate::skeleton::place::walk_key;
use crate::skeleton::{Biome, COUNTY_W, Skeleton};
use crate::steps::Step;

/// Every kind of place the country builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Hamlet,
    Farmstead,
    Cottage,
    Inn,
    Orchard,
    Field,
    Shrine,
    Well,
    Wreck,
    Hay,
    Meadow,
    Herd,
    Woodcutter,
    Pond,
    Stones,
    Camp,
    Den,
    Ruin,
    Outcrop,
    Reedhut,
    Glass,
    Slag,
    Graves,
}

impl Kind {
    /// The id the TypeScript used (`"farmstead"`).
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Hamlet => "hamlet",
            Kind::Farmstead => "farmstead",
            Kind::Cottage => "cottage",
            Kind::Inn => "inn",
            Kind::Orchard => "orchard",
            Kind::Field => "field",
            Kind::Shrine => "shrine",
            Kind::Well => "well",
            Kind::Wreck => "wreck",
            Kind::Hay => "hay",
            Kind::Meadow => "meadow",
            Kind::Herd => "herd",
            Kind::Woodcutter => "woodcutter",
            Kind::Pond => "pond",
            Kind::Stones => "stones",
            Kind::Camp => "camp",
            Kind::Den => "den",
            Kind::Ruin => "ruin",
            Kind::Outcrop => "outcrop",
            Kind::Reedhut => "reedhut",
            Kind::Glass => "glass",
            Kind::Slag => "slag",
            Kind::Graves => "graves",
        }
    }

    /// The kind a story may claim this as, if any.
    pub const fn story_kind(self) -> Option<PlaceKind> {
        match self {
            Kind::Hamlet => Some(PlaceKind::Hamlet),
            Kind::Farmstead => Some(PlaceKind::Farmstead),
            Kind::Cottage => Some(PlaceKind::Cottage),
            Kind::Inn => Some(PlaceKind::Inn),
            Kind::Woodcutter => Some(PlaceKind::Woodcutter),
            Kind::Camp => Some(PlaceKind::Camp),
            Kind::Ruin => Some(PlaceKind::Ruin),
            _ => None,
        }
    }

    const fn of_story(k: PlaceKind) -> Kind {
        match k {
            PlaceKind::Hamlet => Kind::Hamlet,
            PlaceKind::Farmstead => Kind::Farmstead,
            PlaceKind::Cottage => Kind::Cottage,
            PlaceKind::Inn => Kind::Inn,
            PlaceKind::Woodcutter => Kind::Woodcutter,
            PlaceKind::Camp => Kind::Camp,
            PlaceKind::Ruin => Kind::Ruin,
        }
    }

    /// The footprint it asks for, centred on its point. Zelda-tight: a cottage is 22 x 16 with its
    /// garden, a hamlet 36 x 30. *Tuning.*
    pub const fn size(self) -> (i32, i32) {
        match self {
            Kind::Hamlet => (36, 30),
            Kind::Farmstead => (40, 30),
            Kind::Cottage => (22, 16),
            Kind::Inn => (28, 20),
            Kind::Orchard => (20, 16),
            Kind::Field => (24, 16),
            Kind::Shrine => (10, 9),
            Kind::Well | Kind::Den | Kind::Outcrop => (12, 10),
            Kind::Wreck => (12, 9),
            Kind::Hay | Kind::Pond | Kind::Slag | Kind::Graves => (16, 12),
            Kind::Meadow | Kind::Herd => (14, 12),
            Kind::Woodcutter => (20, 18),
            Kind::Stones => (15, 15),
            Kind::Camp => (16, 14),
            Kind::Ruin | Kind::Reedhut => (16, 13),
            Kind::Glass => (18, 13),
        }
    }

    /// Kinds that must stand beside a road, their front to it.
    const fn roadside(self) -> bool {
        matches!(
            self,
            Kind::Hamlet
                | Kind::Farmstead
                | Kind::Cottage
                | Kind::Inn
                | Kind::Shrine
                | Kind::Well
                | Kind::Wreck
                | Kind::Reedhut
        )
    }

    /// Kinds the stories ask the builder for by name, before the dice have the county.
    const fn quota(self) -> bool {
        matches!(self, Kind::Hamlet | Kind::Farmstead | Kind::Cottage | Kind::Inn | Kind::Woodcutter)
    }
}

type Weights = &'static [(Kind, u32)];

/// What the lattice builds, by region: beside a road (within 28 cells of one) and deep. *Tuning.*
const TABLES: [(Weights, Weights); 3] = [
    (
        &[
            (Kind::Hamlet, 6),
            (Kind::Farmstead, 7),
            (Kind::Cottage, 10),
            (Kind::Inn, 1),
            (Kind::Orchard, 3),
            (Kind::Field, 4),
            (Kind::Shrine, 2),
            (Kind::Well, 2),
            (Kind::Wreck, 2),
            (Kind::Hay, 2),
            (Kind::Meadow, 3),
            (Kind::Herd, 4),
            (Kind::Woodcutter, 6),
            (Kind::Pond, 2),
            (Kind::Stones, 1),
        ],
        &[
            (Kind::Camp, 5),
            (Kind::Den, 3),
            (Kind::Ruin, 4),
            (Kind::Pond, 3),
            (Kind::Outcrop, 2),
            (Kind::Stones, 2),
            (Kind::Orchard, 1),
            (Kind::Herd, 5),
            (Kind::Woodcutter, 2),
            (Kind::Meadow, 3),
            (Kind::Field, 2),
            (Kind::Farmstead, 1),
        ],
    ),
    (
        &[
            (Kind::Reedhut, 6),
            (Kind::Cottage, 3),
            (Kind::Ruin, 4),
            (Kind::Glass, 3),
            (Kind::Pond, 3),
            (Kind::Shrine, 2),
            (Kind::Meadow, 3),
            (Kind::Wreck, 2),
            (Kind::Well, 1),
            (Kind::Herd, 1),
            (Kind::Graves, 1),
        ],
        &[
            (Kind::Camp, 6),
            (Kind::Den, 4),
            (Kind::Ruin, 4),
            (Kind::Pond, 4),
            (Kind::Meadow, 3),
            (Kind::Stones, 2),
            (Kind::Glass, 2),
            (Kind::Outcrop, 1),
            (Kind::Reedhut, 1),
        ],
    ),
    (
        &[(Kind::Ruin, 6), (Kind::Slag, 5), (Kind::Wreck, 3), (Kind::Graves, 2), (Kind::Shrine, 1), (Kind::Outcrop, 1)],
        &[(Kind::Camp, 9), (Kind::Den, 4), (Kind::Ruin, 5), (Kind::Slag, 4), (Kind::Outcrop, 3), (Kind::Graves, 2)],
    ),
];

/// What the roadside beat builds, by region: the road table without the kinds that belong in the
/// middle of a field. *Tuning.*
const ALONG: [Weights; 3] = [
    &[
        (Kind::Hamlet, 6),
        (Kind::Farmstead, 7),
        (Kind::Cottage, 10),
        (Kind::Inn, 1),
        (Kind::Orchard, 3),
        (Kind::Field, 4),
        (Kind::Shrine, 2),
        (Kind::Well, 2),
        (Kind::Wreck, 2),
        (Kind::Hay, 2),
        (Kind::Meadow, 2),
        (Kind::Woodcutter, 6),
        (Kind::Pond, 1),
    ],
    &[
        (Kind::Reedhut, 6),
        (Kind::Cottage, 3),
        (Kind::Ruin, 4),
        (Kind::Glass, 3),
        (Kind::Pond, 2),
        (Kind::Shrine, 2),
        (Kind::Meadow, 2),
        (Kind::Wreck, 2),
        (Kind::Well, 1),
        (Kind::Graves, 1),
    ],
    &[(Kind::Ruin, 6), (Kind::Slag, 5), (Kind::Wreck, 3), (Kind::Graves, 2), (Kind::Shrine, 1)],
];

/// The lattice places are thrown on: 44 on the 3.6 km county; 41 since it came in to 2 km
/// square, where the roads, havens and set places take a bigger share of the ground. *Tuning.*
pub const LATTICE: i32 = 41;
/// A field edge's run along a road, in points of the line, and the gap between runs. *Tuning.*
const FENCE_SPAN: usize = 26;
const FENCE_GAP: usize = 6;
/// A gate in a field edge every this many cells. *Tuning.*
const FENCE_GATES: i32 = 13;
/// Places held for the stories keep this far apart (cells): a story will not stand within 60 of
/// another's. *Tuning.*
const QUOTA_APART: i64 = 66;
/// Nothing a story claims within these of the station and of Julie's (cells). *Tuning.*
const OPENING_STATION: i64 = 190;
const OPENING_JULIE: i64 = 120;
/// The patches `areas` draws as places of their own, and the share of their radius nothing is built
/// over, in tenths (plus 20 cells). *Tuning.*
const DRESSED: [(&str, i64); 3] = [("allotments", 7), ("top_field", 7), ("quarry_steps", 5)];
/// Lattice points and the quota's off-road spots keep this far in from the county's edge. *Tuning.*
const EDGE: i32 = 24;

fn region_ix(r: Region) -> usize {
    match r {
        Region::Lowfields => 0,
        Region::Waters => 1,
        Region::Works => 2,
    }
}

/// A road by what it joins, for naming its dice: stable when another road is added.
pub fn road_key(sk: &Skeleton, n: usize) -> i32 {
    walk_key(sk.roads[n].from, sk.roads[n].to)
}

// --- field edges ---------------------------------------------------------------------------

/// Field edges along the roads: where a road runs straight for a while through farmland, the
/// field beside it has a fence (or a hedge, or a low wall) along it, on the side the lamps are
/// not, in a straight line with a gate every so often. Each road on its own dice.
pub fn fences(c: &mut County<'_>) {
    for n in 0..c.sk.roads.len() {
        let mut rng = c.k.dice(Step::CountyFences, road_key(c.sk, n), 0);
        fence_road(c, &mut rng, n);
    }
}

fn fence_road(c: &mut County<'_>, rng: &mut Sfc32, n: usize) {
    let line = c.lines[n].clone();
    let mut i = 10;
    while i + FENCE_SPAN + 10 < line.len() {
        let (a, b) = (line[i], line[i + FENCE_SPAN]);
        let g = ground(c.sk, a.0, a.1);
        let t = match (g.region, g.biome) {
            (Region::Lowfields, Biome::Hedge) => Some(Tile::Bush),
            (Region::Lowfields, Biome::Field) | (Region::Waters, Biome::Reed) => Some(Tile::Fence),
            (Region::Lowfields, Biome::Foothill) | (Region::Waters, Biome::Garden) => Some(Tile::StoneWall),
            _ => None,
        };
        let here = i;
        i += FENCE_SPAN + FENCE_GAP;
        let Some(t) = t else { continue };
        if !rng.chance(Permille(700)) {
            continue;
        }
        let (dx, dy) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
        let (horizontal, vertical) = (dx >= dy * 3, dy >= dx * 3);
        if !horizontal && !vertical {
            continue;
        }
        // The field side: away from the lamps, which stand on the right of travel (+normal), four
        // cells past the road's furthest wobble.
        let nrm = Normal::of(&line, here + (FENCE_SPAN >> 1));
        let span = &line[here..=here + FENCE_SPAN];
        if horizontal {
            let north = nrm.y > 0;
            let y = if north {
                span.iter().map(|p| p.1).min().unwrap_or(a.1) - 4
            } else {
                span.iter().map(|p| p.1).max().unwrap_or(a.1) + 4
            };
            run(c, a.0.min(b.0), y, dx + 1, true, t, FENCE_GATES);
            c.k.claim(Rect::new(a.0.min(b.0), y, dx + 1, 1));
        } else {
            let west = nrm.x > 0;
            let x = if west {
                span.iter().map(|p| p.0).min().unwrap_or(a.0) - 4
            } else {
                span.iter().map(|p| p.0).max().unwrap_or(a.0) + 4
            };
            run(c, x, a.1.min(b.1), dy + 1, false, t, FENCE_GATES);
            c.k.claim(Rect::new(x, a.1.min(b.1), 1, dy + 1));
        }
    }
}

// --- the places ----------------------------------------------------------------------------

/// The stories' quota, the roadside beat, then the lattice.
pub fn places(c: &mut County<'_>) {
    story_quota(c);
    along_roads(c);
    let cols = c.k.w() / LATTICE;
    let rows = c.k.h() / LATTICE;
    for gy in 0..rows {
        for gx in 0..cols {
            // Each point of the lattice on its own dice: where it lands and what it becomes.
            let mut rng = c.k.dice(Step::CountyLattice, gx, gy);
            lattice_point(c, &mut rng, gx, gy);
        }
    }
}

fn lattice_point(c: &mut County<'_>, rng: &mut Sfc32, gx: i32, gy: i32) {
    let x = gx * LATTICE + 6 + rng.irandom(LATTICE - 12);
    let y = gy * LATTICE + 6 + rng.irandom(LATTICE - 12);
    if x < EDGE || y < EDGE || x > c.k.w() - EDGE || y > c.k.h() - EDGE {
        return;
    }
    if near_chunk(c, x, y, 16) || in_dressed_area(c, x, y) {
        return;
    }
    let g = ground(c.sk, x, y);
    if g.wet && c.k.get(x, y) == Tile::Water {
        return;
    }
    let d = dist(&c.country.d_road, x, y);
    if d < 7 {
        return;
    }
    let (road, deep) = TABLES[region_ix(g.region)];
    let table = if d <= 28 { road } else { deep };
    // Three throws: the kind the dice chose, and then others, until one the ground allows fits.
    for _ in 0..3 {
        let Some(&kind) = pick_weighted(table, rng) else { continue };
        if allowed(c, kind, x, y, Some(d)) && stamp(c, kind, x, y) {
            break;
        }
    }
}

/// The roads first: something beside every road at a steady beat, the way farms and cottages
/// string out along a lane. Each road is walked and, every forty-odd cells, a place is set just
/// off its verge on one side or the other, its front toward the road. Each beat on its own dice.
fn along_roads(c: &mut County<'_>) {
    for n in 0..c.sk.roads.len() {
        let key = road_key(c.sk, n);
        let line = c.lines[n].clone();
        let mut next = 20 + c.k.dice(Step::CountyAlong, key, -1).irandom(30) as usize;
        let dense_road = c.country.first_lines[n];
        for i in 12..line.len().saturating_sub(12) {
            if i < next {
                continue;
            }
            let mut rng = c.k.dice(Step::CountyAlong, key, i as i32);
            let (lx, ly) = line[i];
            if near_chunk(c, lx, ly, 20) || in_dressed_area(c, lx, ly) {
                continue;
            }
            let region = ground(c.sk, lx, ly).region;
            let mut placed = false;
            for _ in 0..3 {
                let Some(&kind) = pick_weighted(ALONG[region_ix(region)], &mut rng) else { break };
                let (w, h) = kind.size();
                let nrm = Normal::of(&line, i);
                let first = if rng.chance(Permille(500)) { 1 } else { -1 };
                'sides: for side in [first, -first] {
                    // Far enough out that the whole footprint clears the road and its verge.
                    let reach = nrm.reach(w, h, 8);
                    for extra in [0, 5, 10] {
                        let (x, y) = nrm.along((lx, ly), (reach + extra * Q) * side);
                        if allowed(c, kind, x, y, None) && stamp(c, kind, x, y) {
                            placed = true;
                            break 'sides;
                        }
                    }
                }
                if placed {
                    break;
                }
            }
            // The first walk is the establishing shot, and gets something about twice as often;
            // the Lowfields' lanes are strung with houses the way the first walk is.
            let dense = dense_road || region == Region::Lowfields;
            next = i + if !placed {
                10
            } else if dense {
                36 + rng.irandom(20) as usize
            } else {
                44 + rng.irandom(26) as usize
            };
        }
    }
}

/// Inside one of the patches `areas` dresses as a place of its own: nothing is built over it.
fn in_dressed_area(c: &County<'_>, x: i32, y: i32) -> bool {
    let cat = jane_data::catalog();
    c.sk.areas.iter().any(|a| {
        let id = cat.name(a.def.id);
        let Some(&(_, share)) = DRESSED.iter().find(|(n, _)| *n == id) else { return false };
        let (dx, dy) = (i64::from(centre(a.mx) - x), i64::from(centre(a.my) - y));
        let r10 = i64::from(a.def.radius) * share + 200;
        100 * (dx * dx + dy * dy) < r10 * r10
    })
}

/// Whether the ground at `(x, y)` takes a place of this kind. `d` is the cells to the nearest road;
/// `None` from a roadside pass, which has put it beside the road already.
fn allowed(c: &County<'_>, kind: Kind, x: i32, y: i32, d: Option<u8>) -> bool {
    let g = ground(c.sk, x, y);
    let big = matches!(kind, Kind::Hamlet | Kind::Farmstead | Kind::Inn);
    if let Some(d) = d {
        let (lo, hi) = if big { (14, 28) } else { (9, 22) };
        if kind.roadside() && (d < lo || d > hi) {
            return false;
        }
    }
    match kind {
        Kind::Hamlet | Kind::Farmstead | Kind::Inn => {
            !matches!(g.biome, Biome::Wood | Biome::WetWood | Biome::Marsh) && g.threat <= 3
        }
        Kind::Cottage => g.threat <= 3 && g.biome != Biome::Marsh,
        Kind::Camp | Kind::Den => {
            d.is_some_and(|d| d >= camp_back(g.region))
                && g.threat > 0
                && dist(&c.country.d_first, x, y) >= FIRST_CLEAR + 8
        }
        // The copses of the hedged fields are worked too.
        Kind::Woodcutter => matches!(g.biome, Biome::Wood | Biome::WetWood | Biome::Foothill | Biome::Hedge),
        Kind::Reedhut => matches!(g.biome, Biome::Reed | Biome::Marsh | Biome::WetWood | Biome::Garden),
        Kind::Outcrop => matches!(g.biome, Biome::Foothill | Biome::Hill | Biome::Slag | Biome::Field | Biome::Yard),
        Kind::Field | Kind::Orchard | Kind::Hay => matches!(g.biome, Biome::Field | Biome::Hedge | Biome::Garden),
        _ => true,
    }
}

/// Build a place of `kind` centred on `(x, y)` if its footprint is open: growth cleared, the
/// place furnished on dice of its own (its kind and where it stands, nothing else), its footprint
/// claimed, and a [`Place`] on the county's list. False if there was no room.
pub fn stamp(c: &mut County<'_>, kind: Kind, x: i32, y: i32) -> bool {
    let (w, h) = kind.size();
    let (x0, y0) = (x - (w >> 1), y - (h >> 1));
    if !room(c, x0, y0, w, h) {
        return false;
    }
    let bounds = Rect::new(x0, y0, w, h);
    clear(c, bounds, if kind == Kind::Woodcutter { Tile::GrassTall } else { Tile::Grass });
    let g = ground(c.sk, x, y);
    c.country.cur = Some(Place {
        n: y0 * COUNTY_W + x0,
        kind,
        region: g.region,
        threat: g.threat,
        bounds,
        road: dist(&c.country.d_road, x, y),
        first: dist(&c.country.d_first, x, y),
        slots: Vec::new(),
        things: Vec::new(),
        folk: Vec::new(),
        hostiles: Vec::new(),
    });
    let mut rng = c.k.dice(Step::CountyPlace, y * COUNTY_W + x, kind as i32);
    super::furnish::furnish(c, &mut rng, kind, x, y, bounds);
    if let Some(p) = c.country.cur.take() {
        c.places.push(p);
    }
    c.k.claim(bounds);
    true
}

// --- the stories' quota --------------------------------------------------------------------

/// Whether a place's centre is nearer `(x, y)` than `r` cells.
fn within(p: &Place, x: i32, y: i32, r: i64) -> bool {
    let (cx2, cy2) = p.centre2();
    let (dx, dy) = (i64::from(cx2 - 2 * x), i64::from(cy2 - 2 * y));
    dx * dx + dy * dy < 4 * r * r
}

/// Open for a story's place: clear of the set places, the dressed patches, the opening (the
/// station and Julie's gate) and every other story kind's place.
fn free(c: &County<'_>, x: i32, y: i32) -> bool {
    let sk = c.sk;
    let opening = [(sk.named.station, OPENING_STATION), (sk.named.julie_house, OPENING_JULIE)];
    !near_chunk(c, x, y, 20)
        && !in_dressed_area(c, x, y)
        && !opening.iter().any(|&(s, r)| {
            let s = sk.site(s);
            let (dx, dy) = (i64::from(centre(s.mx) - x), i64::from(centre(s.my) - y));
            dx * dx + dy * dy < r * r
        })
        && !c.places.iter().any(|p| p.kind.quota() && within(p, x, y, QUOTA_APART))
}

/// A place of `kind` for a story at `(x, y)`, if the ground there will take it: in the region, on
/// threat no worse than 2 (a tale's ruin: 3, or 5 in the Works, which is threatened ground all
/// over), free, and a ruin clear of every other ruin.
fn try_at(c: &mut County<'_>, region: Region, kind: Kind, x: i32, y: i32) -> bool {
    let g = ground(c.sk, x, y);
    let most = match (kind, region) {
        (Kind::Ruin, Region::Works) => 5,
        (Kind::Ruin, _) => 3,
        _ => 2,
    };
    g.region == region
        && g.threat <= most
        && free(c, x, y)
        && (kind != Kind::Ruin || !c.places.iter().any(|p| p.kind == Kind::Ruin && within(p, x, y, QUOTA_APART)))
        && allowed(c, kind, x, y, None)
        && stamp(c, kind, x, y)
}

/// Beside road `n`, every twelve points, until `count` more stand; how many did.
fn along_line(c: &mut County<'_>, n: usize, region: Region, kind: Kind, count: usize) -> usize {
    let (w, h) = kind.size();
    let line = c.lines[n].clone();
    let mut done = 0;
    let mut i = 12;
    while i + 12 < line.len() && done < count {
        let nrm = Normal::of(&line, i);
        let reach = nrm.reach(w, h, 8);
        'search: for side in [1, -1] {
            for extra in [0, 6, 12] {
                let (x, y) = nrm.along(line[i], (reach + extra * Q) * side);
                if try_at(c, region, kind, x, y) {
                    done += 1;
                    break 'search;
                }
            }
        }
        i += 12;
    }
    done
}

/// Up to `count` of a kind off the road, `near` to `far` cells back from a made road (a story lays
/// its footpath from a road, not from another footpath), on ground `biome` allows, in an order the
/// seed shuffles on dice of its own (`name`), so they spread over the region rather than fill its
/// top. How many stood.
#[allow(clippy::too_many_arguments)]
fn off_road(
    c: &mut County<'_>,
    made: &mut Option<jane_core::Grid<u8>>,
    region: Region,
    kind: Kind,
    count: usize,
    name: (i32, i32),
    near: u8,
    far: u8,
    biome: fn(Biome) -> bool,
) -> usize {
    if count == 0 {
        return 0;
    }
    let roads = c.sk.roads.len();
    let made = made.get_or_insert_with(|| distance_field(c.k.w(), c.k.h(), &c.lines, |n| n < roads));
    let mut spots = Vec::new();
    for y in (EDGE..c.k.h() - EDGE).step_by(20) {
        for x in (EDGE..c.k.w() - EDGE).step_by(20) {
            let d = dist(made, x, y);
            let g = ground(c.sk, x, y);
            if d >= near && d <= far && g.region == region && biome(g.biome) {
                spots.push((x, y));
            }
        }
    }
    c.k.dice(Step::CountyQuota, name.0, name.1).shuffle(&mut spots);
    let mut done = 0;
    for (x, y) in spots {
        if done >= count {
            break;
        }
        if try_at(c, region, kind, x, y) {
            done += 1;
        }
    }
    done
}

fn any_biome(_: Biome) -> bool {
    true
}

fn woodland(b: Biome) -> bool {
    matches!(b, Biome::Wood | Biome::WetWood | Biome::Foothill | Biome::Hedge)
}

/// The order the quota builds in: the big ones first (a farm needs a field's width of open
/// roadside, and cottages fit in round it).
const QUOTA_ORDER: [Kind; 5] = [Kind::Hamlet, Kind::Farmstead, Kind::Inn, Kind::Cottage, Kind::Woodcutter];

/// The places the stories will ask for, built first. Every story claims a place of its kind in its
/// region, 60 cells or more from another's; left to the dice a seed could come up with nine
/// cottages for eleven cottage stories. So before the roadside beat: houses, farms, hamlets and
/// inns are set beside the roads (the first walk's first); woodyards off the road, 30 to 110 cells
/// back, in the woods and the hedged fields. Twice as many as the stories need and one over,
/// because some are turned down.
fn story_quota(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let mut made = None;
    // Stories that must stand by the first walk get theirs there before anything else takes the roadside.
    for s in cat.county.stories {
        let kind = Kind::of_story(s.kind);
        if s.first.is_none() || !kind.quota() {
            continue;
        }
        let mut left = 2;
        for n in 0..c.sk.roads.len() {
            if c.country.first_lines[n] && left > 0 {
                left -= along_line(c, n, s.region, kind, left);
            }
        }
    }
    // (kind order, region) -> how many stories want one.
    let mut want: Vec<((usize, Region), usize)> = Vec::new();
    for s in cat.county.stories {
        let kind = Kind::of_story(s.kind);
        let Some(order) = QUOTA_ORDER.iter().position(|&k| k == kind) else { continue };
        match want.iter_mut().find(|(k, _)| *k == (order, s.region)) {
            Some((_, n)) => *n += 1,
            None => want.push(((order, s.region), 1)),
        }
    }
    want.sort_by_key(|&((order, region), _)| (order, region_ix(region)));
    for ((order, region), need) in want {
        let kind = QUOTA_ORDER[order];
        let target = need * 2 + 1;
        let mut have = c.places.iter().filter(|p| p.kind == kind && p.region == region).count();
        if kind == Kind::Woodcutter {
            let name = (region_ix(region) as i32, kind as i32);
            off_road(c, &mut made, region, kind, target.saturating_sub(have), name, 30, 110, woodland);
            continue;
        }
        for n in 0..c.sk.roads.len() {
            if have >= target {
                break;
            }
            have += along_line(c, n, region, kind, target - have);
        }
    }
    tale_quota(c, &mut made);
}

/// The tales' ruins, built beside the roads before the dice have the county: a tale needs a ruin
/// of its own sort (a roofless cottage, or four walls) in its region, seen from a road and clear of
/// every other story's place. Three of each sort a tale asks for and two over; a region whose
/// roadsides the stories' houses have taken gets its ruins back from the road, where ruins stand.
fn tale_quota(c: &mut County<'_>, made: &mut Option<jane_core::Grid<u8>>) {
    let cat = jane_data::catalog();
    let mut want: Vec<((Region, RuinKind), usize)> = Vec::new();
    for s in cat.county.stories {
        if !s.tale || s.kind != PlaceKind::Ruin {
            continue;
        }
        let key = (s.region, s.ruin.unwrap_or(RuinKind::Walls));
        match want.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => want.push((key, 1)),
        }
    }
    want.sort_by_key(|&((region, ruin), _)| (region_ix(region), ruin == RuinKind::Walls));
    for ((region, ruin), need) in want {
        let built = |c: &County<'_>| {
            c.places
                .iter()
                .filter(|p| {
                    p.kind == Kind::Ruin
                        && p.region == region
                        && p.thing("house").is_some() == (ruin == RuinKind::House)
                })
                .count()
        };
        c.country.ruin_as = Some(ruin);
        let target = need * 3 + 2;
        for n in 0..c.sk.roads.len() {
            let have = built(c);
            if have >= target {
                break;
            }
            along_line(c, n, region, Kind::Ruin, target - have);
        }
        let have = built(c);
        if have < target {
            let name = (region_ix(region) as i32 + 8, i32::from(ruin == RuinKind::Walls));
            off_road(c, made, region, Kind::Ruin, target - have, name, 24, 90, any_biome);
        }
        c.country.ruin_as = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_weighs_something_and_every_kind_fits_a_lattice_cell() {
        for (road, deep) in TABLES {
            assert!(road.iter().chain(deep).all(|&(_, w)| w > 0));
        }
        for k in ALONG {
            assert!(!k.is_empty());
        }
    }
}
