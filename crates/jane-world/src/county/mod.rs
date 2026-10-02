//! The county: 2000 x 2000 cells, 2 km square, five minutes across by road. Built from the
//! skeleton (`crate::skeleton`), which decided on a coarse grid where the river runs, where the
//! story's places stand, which roads join them, which lamps still work and how dangerous each patch
//! is. This turns that into cells. Carries `jane/src/world/county.ts`.
//!
//! The pipeline, in the TypeScript's order ([`STAGES`]). Each entry is one function over the
//! [`County`] being built, and each draws only from its own dice (PORT.md §6.a). PORT.md §6.m
//! numbers the port's stages; every one has landed.
//!
//! | Entry | What | Port stage |
//! | --- | --- | --- |
//! | `land` | every cell's ground from the skeleton's biome and water, edges softened | 5 |
//! | `areas` | patches that are places (the allotments, the quarry face) | 7 |
//! | `edge` | the tree line round the county | 5 |
//! | `roads` | the skeleton's routes, a verge each side, plank bridges over water | 5 |
//! | `paths` | the burial footpath and `data/paths.json`'s footpaths | 5 |
//! | `chunks` | the set places stamped where the skeleton put them, their rects, the link lanes | 6 |
//! | `rail` | the railway raster: track, ballast, trestle, crossings, the fences at its ends | 5 |
//! | `path_ends` | the footpaths and link lanes claimed; a mark at each footpath end, and a fingerpost saying where it goes | 5 |
//! | `doors` | the ways into the dungeons | 6 |
//! | `place_chunks` | placement rows inside the chunks, then the chunks claimed | 7 |
//! | `road_furniture` | lamps, bridge lamps, forks, milestones; the roads' margins claimed | 7 |
//! | `relays` | relay boxes and dead lamp runs | 7 |
//! | `small_places` | the skeleton's small places dressed, signposts given words, anchors' rects | 7 |
//! | `place_pois` | placement rows at the small places, then the small places claimed | 7 |
//! | `place_areas` | placement rows in the named patches | 7 |
//! | `country` | field edges, hamlets, farms, camps, dens, ruins, ponds | 7 |
//! | `stories` | stories claim places, boards go up, the stories' rows | 8 |
//! | `perimeters` | an edge round every named patch: a hedge, a field wall, a reed edge, a slag bank | 9 |
//! | `pit_wood` | deadwood (a stump) by every made fire's pit that has none within 12 cells | 8 |
//! | `scatter` | herbs and rocks | 9 |
//! | `ways` | every way and door step cleared: growth gives way, a fence a gate, a thing on it moved aside | 9 |
//! | `wildlife` | by region, biome and threat | 9 |
//! | `cut_through` | a way cut to any named place the wood closed round | 9 |
//! | `drop_unreachable` | small places and creatures nobody can reach are dropped | 9 |
//!
//! [`build_proven`] is the county `jane_world::build_zone` hands out: built, judged by the solver,
//! re-rolled over the next valid skeleton on a refusal. Same seed, same county.

pub mod areas;
pub mod chunks;
pub mod country;
pub mod doors;
pub mod finish;
pub mod land;
pub mod links;
pub mod paths;
pub mod perimeter;
pub mod placements;
pub mod rail;
pub mod roads;
pub mod small;
pub mod stories;
pub mod tale_ground;
pub mod ways;

use jane_core::blueprint::{Area, RegionMap, ZONE_ATTEMPTS};
use jane_core::num::Permille;
use jane_core::{Blueprint, Grid, Key, NameId, Rect, Tile, ZoneId};

pub use self::chunks::Chunk;
use self::placements::{PoiSpot, Stage, apply_placements, claim_pois};
use crate::kit::Kit;
use crate::skeleton::{
    COUNTY_H, COUNTY_W, MACRO, SKEL_H, SKEL_W, Skeleton, SkeletonError, SkeletonRows, build_skeleton,
};
use crate::solve::{ZoneRules, validate};

/// A footpath as laid: its row in `data/paths.json` and its centre line (an index into `lines`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footpath {
    pub row: usize,
    pub line: usize,
}

/// The county being built: the kit, and what one stage leaves for the next.
#[derive(Debug)]
pub struct County<'a> {
    pub sk: &'a Skeleton,
    pub k: Kit,
    /// Every road's, path's and footpath's centre line, in the order laid.
    pub lines: Vec<Vec<(i32, i32)>>,
    /// Per line, whether each point stands on a lit stretch of road; empty for a path.
    pub lit: Vec<Vec<bool>>,
    /// The ground as it was before any road: where a road crosses water it is a bridge.
    pub before: Option<Grid<Tile>>,
    pub footpaths: Vec<Footpath>,
    /// The set places as stamped, in site row order.
    pub chunks: Vec<Chunk>,
    /// What the country's three stages share: the distance fields, the lamps stood so far.
    pub country: country::Ctx,
    /// The places the country built, in build order: what the stories stage claims.
    pub places: Vec<country::Place>,
    /// Exact cells the dressed patches offer placement rows, by name (`quarry_adit`).
    pub area_slots: Vec<(NameId, (i32, i32))>,
    /// The skeleton's small places, rolled then anchors', each with the kind it is dressed as: the
    /// seed's roll, or the kind a `poi` placement row needed (set at `place_chunks`, before the
    /// small places are dressed).
    pub pois: Vec<PoiSpot>,
    /// Which small place each `poi` placement row claimed: `(row key, index into pois)`.
    pub claimed: Vec<(NameId, usize)>,
    /// Every cell the flood from `start` reached once the way was cut through (`y * w + x`), for
    /// `drop_unreachable` right after; taken by it. Stale if the ground changes in between.
    pub reached: Option<Vec<bool>>,
    /// Which place each story claimed, why the others found none, the footpaths laid to places
    /// off the road (the stories stage).
    pub story_claims: stories::Claims,
    /// The ground she could walk to as the county stood when the stories claimed (`y * w + x`),
    /// if a tale asked: a tale's rows are set down only where she can get to them.
    pub ground: Option<Vec<bool>>,
    /// Ground a short walk from each tale's place (an index into `places`), kept from when the
    /// tale was fitted there, for its rows.
    pub on_foot: Vec<(usize, tale_ground::OnFoot)>,
    /// Every cell a footpath, a lane or a link lane trod (`y * w + x`): kept clear of whatever is
    /// set down after it (`ways`).
    pub trodden: Vec<bool>,
    /// Every cell the land laid as open earth (`y * w + x`): what is still dirt of it at the end,
    /// off every way and set place, is painted `Material::WildEarth` (`land::wild_earth`).
    pub wild_earth: Vec<bool>,
    /// Every lane, link lane and footpath as laid: its centre line and where it is meant to meet
    /// its place (`ways::Way`).
    pub ways: Vec<ways::Way>,
    /// Every fork's fingerpost as `country::roads::forks` set it up: its key, where it stands and
    /// its words, so the stories stage can add an arm for a place nearby (`stories::posts`).
    pub fork_posts: Vec<(Key, (i32, i32), String)>,
    /// Each named patch's edge as laid, in the skeleton's order (`perimeter::lay_perimeters`).
    pub perimeters: Vec<perimeter::Perimeter>,
}

/// The centre cell of macro cell `m`, on either axis.
pub const fn centre(m: i32) -> i32 {
    m * MACRO + MACRO / 2
}

impl<'a> County<'a> {
    /// An empty county of grass over `sk`, for `seed` at county attempt `attempt`.
    pub fn new(sk: &'a Skeleton, attempt: u8) -> Self {
        Self {
            sk,
            k: Kit::new(ZoneId::County, COUNTY_W as u32, COUNTY_H as u32, sk.seed, attempt, Tile::Grass, true),
            lines: Vec::new(),
            lit: Vec::new(),
            before: None,
            footpaths: Vec::new(),
            chunks: Vec::new(),
            country: country::Ctx::default(),
            places: Vec::new(),
            area_slots: Vec::new(),
            pois: PoiSpot::all(sk),
            claimed: Vec::new(),
            reached: None,
            story_claims: stories::Claims::default(),
            ground: None,
            on_foot: Vec::new(),
            trodden: vec![false; (COUNTY_W * COUNTY_H) as usize],
            wild_earth: vec![false; (COUNTY_W * COUNTY_H) as usize],
            ways: Vec::new(),
            fork_posts: Vec::new(),
            perimeters: Vec::new(),
        }
    }

    /// The finished blueprint, with the skeleton's patches as placed: each a square of its radius
    /// about its centre (the ecology's areas, ARCHITECTURE.md §4.6.c); and the skeleton's region
    /// of every macro cell, so the sky that rains on a cell is its region's (§4.6.b).
    pub fn done(mut self) -> Blueprint {
        let earth = land::wild_earth(&self);
        self.k.paint_all(earth, jane_core::Material::WildEarth);
        let areas = self
            .sk
            .areas
            .iter()
            .map(|a| {
                let r = i32::from(a.def.radius);
                Area {
                    name: Key::Name(a.def.id),
                    rect: Rect::new(centre(a.mx) - r, centre(a.my) - r, 2 * r + 1, 2 * r + 1),
                }
            })
            .collect();
        let mut regions = RegionMap::new(MACRO as u16, SKEL_W as u16, SKEL_H as u16, 0);
        for my in 0..SKEL_H {
            for mx in 0..SKEL_W {
                regions.set(mx as u16, my as u16, self.sk.region_at(mx, my) as u8);
            }
        }
        let mut bp = self.k.done("Castle", false, Permille::ONE);
        bp.areas = areas;
        bp.regions = regions;
        bp
    }
}

/// One stage of the county's build.
pub type StageFn = for<'a, 'b> fn(&'b mut County<'a>);

/// The county's build, in order: `(name, stage)`. [`build_county_on`] runs them all; a tool may
/// run them one by one to time them.
pub const STAGES: &[(&str, StageFn)] = &[
    ("land", land::paint_land),
    ("areas", areas),
    ("edge", land::tree_line),
    ("roads", roads::lay_roads),
    ("paths", roads::lay_paths),
    ("chunks", stamp_chunks),
    ("rail", rail::lay_railway),
    ("path_ends", path_ends),
    ("doors", doors::set_doors),
    ("place_chunks", place_chunks),
    ("road_furniture", road_furniture),
    ("relays", relays),
    ("small_places", small_places),
    ("place_pois", place_pois),
    ("place_areas", place_areas),
    ("country", country),
    ("stories", stories),
    ("perimeters", perimeter::lay_perimeters),
    ("pit_wood", pit_wood),
    ("scatter", scatter),
    ("ways", ways::clear_ways),
    ("wildlife", wildlife),
    ("cut_through", cut_through),
    ("drop_unreachable", drop_unreachable),
];

/// The `attempt`-th valid skeleton of `seed`: attempt 0 is the first that passes from skeleton
/// attempt 0 (the county the seed viewer shows), attempt 1 the next after it, and so on.
pub fn county_skeleton(seed: u32, attempt: u8) -> Result<Skeleton, SkeletonError> {
    let rows = SkeletonRows::catalog();
    let mut sk = build_skeleton(seed, &rows, 0)?;
    for _ in 0..attempt {
        let Some(from) = sk.attempt.checked_add(1) else { break };
        sk = build_skeleton(seed, &rows, from)?;
    }
    Ok(sk)
}

/// The county of `seed` at county attempt `attempt`, over its `attempt`-th skeleton.
pub fn build_county(seed: u32, attempt: u8) -> Result<Blueprint, SkeletonError> {
    let sk = county_skeleton(seed, attempt)?;
    Ok(build_county_on(&sk, attempt))
}

/// The county of `seed`, proven (`buildZone` over `buildCounty`): attempt 0 over the seed's first
/// valid skeleton, judged by the solver with the county's rules (its contract and what the
/// placement rows promise); a county the solver refuses is re-rolled at the next attempt over the
/// next valid skeleton ([`county_skeleton`], each asked for from the one before rather than from
/// the start). If none of [`ZONE_ATTEMPTS`] holds, the last is returned, `attempts ==
/// ZONE_ATTEMPTS`, for the caller to refuse. An error only for rows no skeleton can satisfy.
pub fn build_proven(seed: u32) -> Result<Blueprint, SkeletonError> {
    build_proven_with(seed, &mut |_| {})
}

/// [`build_proven`], saying `"skeleton"`, each stage's name and `"solve"` to `report` as each
/// starts (a re-roll says them again). Listening changes nothing that is built.
pub fn build_proven_with(seed: u32, report: crate::Report<'_>) -> Result<Blueprint, SkeletonError> {
    let rules = ZoneRules::for_zone(ZoneId::County);
    let rows = SkeletonRows::catalog();
    report("skeleton");
    let mut sk = build_skeleton(seed, &rows, 0)?;
    let mut attempt = 0u8;
    loop {
        let bp = build_county_on_with(&sk, attempt, report);
        if attempt + 1 >= ZONE_ATTEMPTS {
            return Ok(bp);
        }
        report("solve");
        if validate(&bp, &rules).ok() {
            return Ok(bp);
        }
        let Some(from) = sk.attempt.checked_add(1) else { return Ok(bp) };
        sk = build_skeleton(seed, &rows, from)?;
        attempt += 1;
    }
}

/// The county over a skeleton already built (the caller keeps it: the county's re-rolls and its
/// tests read it).
pub fn build_county_on(sk: &Skeleton, attempt: u8) -> Blueprint {
    build_county_on_with(sk, attempt, &mut |_| {})
}

/// [`build_county_on`], saying each stage's name to `report` as it starts.
pub fn build_county_on_with(sk: &Skeleton, attempt: u8, report: crate::Report<'_>) -> Blueprint {
    let mut c = County::new(sk, attempt);
    for &(name, stage) in STAGES {
        report(name);
        stage(&mut c);
    }
    c.done()
}

// --- the stages ---------------------------------------------------------------------------------

/// Patches that are places, dressed before the roads so a road that crosses one simply crosses
/// it (`AREA_DRESS`, `world/areas.ts`).
fn areas(c: &mut County<'_>) {
    areas::dress_areas(c);
}

/// The set places, stamped where the skeleton put them, in site row order (`chunks::stamp`);
/// each one's ground as a `site_<id>` rect, so a measure of the open country can tell it from a
/// town; then every road, path and footpath joined round the box to its nearest gate
/// (`links::link_lines`).
fn stamp_chunks(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    for s in &c.sk.sites {
        if let Some(def) = cat.chunks.at_site(s.row) {
            let ch = chunks::stamp(&mut c.k, def, s.row, centre(s.mx), centre(s.my));
            c.chunks.push(ch);
        }
    }
    for i in 0..c.chunks.len() {
        let key = c.k.local(&format!("site_{}", c.chunks[i].id()));
        if !c.k.blueprint().rects.contains_key(&key) {
            c.k.rect(key, c.chunks[i].bounds);
        }
    }
    // A footpath laid before the stamps is the chunk's ground where it crossed a box.
    let w = c.k.w();
    for ch in &c.chunks {
        for (x, y) in ch.bounds.cells() {
            if c.k.inside(x, y) {
                c.trodden[(y * w + x) as usize] = false;
            }
        }
    }
    links::link_lines(c);
}

/// Every cell the footpaths and the link lanes trod claimed, so nothing set down after stands on
/// a way; then each footpath's ends marked and signed (`paths::path_ends`).
fn path_ends(c: &mut County<'_>) {
    ways::claim_trodden(c);
    paths::path_ends(c);
}

/// Cells round a chunk's box closed to what comes after its placement rows.
const CHUNK_CLAIM: i32 = 6;
/// A small place's ground, closed once its placement rows are down: this far each side of its
/// centre, and this far above and below it.
const POI_CLAIM_X: i32 = 7;
const POI_CLAIM_Y: i32 = 6;

/// Which small place each `poi` placement row gets (so the small places are dressed as the rows
/// need them); the placement rows that go inside a chunk, while its open ground is still open; then
/// every chunk's ground claimed. A gate is a way in: it is claimed first, so nothing placed by name
/// stands on it (a sign set down by the graveyard's gate once shut it).
fn place_chunks(c: &mut County<'_>) {
    let rows = jane_data::catalog().county.placements;
    for i in 0..c.chunks.len() {
        for g in 0..c.chunks[i].gates.len() {
            let (x, y) = c.chunks[i].gates[g];
            c.k.claim(Rect::new(x, y, 1, 1));
        }
    }
    c.claimed = claim_pois(c.sk, &mut c.pois, rows);
    apply_placements(c, Stage::Chunks, rows);
    for i in 0..c.chunks.len() {
        c.k.claim(c.chunks[i].bounds.grow(CHUNK_CLAIM));
    }
}

/// The placement rows at the small places, once they are dressed; then each small place's ground
/// claimed.
fn place_pois(c: &mut County<'_>) {
    apply_placements(c, Stage::Pois, jane_data::catalog().county.placements);
    for i in 0..c.pois.len() {
        let p = c.pois[i];
        c.k.claim(Rect::new(p.x - POI_CLAIM_X, p.y - POI_CLAIM_Y, 2 * POI_CLAIM_X + 1, 2 * POI_CLAIM_Y + 1));
    }
}

/// What the quests put in the named patches, before the country fills up round it.
fn place_areas(c: &mut County<'_>) {
    apply_placements(c, Stage::Areas, jane_data::catalog().county.placements);
}

/// The roads' furniture while their margins are open (lamps, a lamp at each end of a bridge, a
/// fingerpost at every fork, milestones: `country::furnish_roads`), then every road's, path's
/// and footpath's margin claimed, three cells each side of its centre line.
fn road_furniture(c: &mut County<'_>) {
    country::furnish_roads(c);
    for line in &c.lines {
        for &(x, y) in line {
            c.k.claim(Rect::new(x - 3, y - 3, 7, 7));
        }
    }
}

/// The longest dark stretches get a relay box and a run of dead lamps (`country::relays`).
fn relays(c: &mut County<'_>) {
    country::relays::relay_runs(c);
}

/// The skeleton's small places dressed as the kind each is (the seed's roll, or the kind a `poi`
/// placement row needed), roadside signposts given words, each needed place's clearing, mark and
/// rect (`small::small_places`). The `pois` placement rows and the small places' claim follow
/// (`place_pois`), then the `areas` rows (`place_areas`).
fn small_places(c: &mut County<'_>) {
    small::small_places(c);
}

/// Everything else a county has in it: field edges, the stories' quota, hamlets, farms, camps,
/// dens, ruins, ponds (`country::furnish_places`). Leaves `County::places` for the stories stage.
fn country(c: &mut County<'_>) {
    country::furnish_places(c);
}

/// The stories claim places, the boards go up with the places' names, each story's rows go into
/// its place (`placements::apply_placements(c, Stage::Places, ..)`), and `Blueprint::stories`
/// records where each landed (`stories::stories`).
fn stories(c: &mut County<'_>) {
    stories::stories(c);
}

/// How near a made fire's pit its deadwood stands, cells (PLAY-PLAN.md §2.2, the L1 proof).
pub const PIT_WOOD_CELLS: i32 = 12;

/// Deadwood by every made fire's pit (a cold pit, a camp's fire, an old grate): a stump set down
/// by any with no wood (a stump, a woodpile, a log) within [`PIT_WOOD_CELLS`], origin to origin
/// (`country::wood_by`). No dice.
fn pit_wood(c: &mut County<'_>) {
    let cat = jane_data::catalog();
    let Some(stump) = cat.story.prop_id("stump") else { return };
    let at = |p: &jane_core::blueprint::PropSpawn| (i32::from(p.cell.x), i32::from(p.cell.y));
    let props = &c.k.blueprint().props;
    let pits: Vec<(i32, i32)> = props.iter().filter(|p| cat.story.prop(p.def).made).map(at).collect();
    let mut woods: Vec<(i32, i32)> = props.iter().filter(|p| cat.story.prop(p.def).wood > 0).map(at).collect();
    let r2 = PIT_WOOD_CELLS * PIT_WOOD_CELLS;
    for (x, y) in pits {
        if woods.iter().any(|&(wx, wy)| (wx - x) * (wx - x) + (wy - y) * (wy - y) <= r2) {
            continue;
        }
        if country::wood_by(c, stump, x, y).is_some() {
            if let Some(p) = c.k.blueprint().props.last() {
                woods.push(at(p));
            }
        }
    }
}

/// Herbs and rocks on open unclaimed ground.
fn scatter(c: &mut County<'_>) {
    finish::scatter(c);
}

/// Wildlife by region, biome and threat, wanderers along the field edges, then something small
/// on every screen still empty (`country::furnish_life`).
fn wildlife(c: &mut County<'_>) {
    country::furnish_life(c);
}

/// A way cut through thicket to any named place the flood from `start` never reached.
fn cut_through(c: &mut County<'_>) {
    finish::cut_through(c);
}

/// Small places and creatures the flood from `start` never reached are dropped.
fn drop_unreachable(c: &mut County<'_>) {
    finish::drop_unreachable(c);
}
