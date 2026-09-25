//! The county: 2000 x 2000 cells, 2 km square, five minutes across by road. Built from the
//! skeleton (`crate::skeleton`), which decided on a coarse grid where the river runs, where the
//! story's places stand, which roads join them, which lamps still work and how dangerous each patch
//! is. This turns that into cells. Carries `jane/src/world/county.ts`.
//!
//! The pipeline, in the TypeScript's order ([`STAGES`]). Each entry is one function over the
//! [`County`] being built, and each draws only from its own dice (PORT.md §6.a). PORT.md §6.m
//! numbers the port's stages; an entry that belongs to a later port stage is a function that does
//! nothing yet, and says which stage lands there.
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
//! | `path_ends` | a mark at each footpath end, and a fingerpost saying where it goes | 5 |
//! | `doors` | the ways into the dungeons | 6 |
//! | `place_chunks` | placement rows inside the chunks, then the chunks claimed | 7 |
//! | `road_furniture` | lamps, bridge lamps, forks, milestones; the roads' margins claimed | 7 |
//! | `relays` | relay boxes and dead lamp runs | 7 |
//! | `small_places` | the skeleton's small places dressed, signposts given words, placements | 7 |
//! | `country` | field edges, hamlets, farms, camps, dens, ruins, ponds | 7 |
//! | `stories` | stories claim places, boards go up, the stories' rows | 8 |
//! | `scatter` | herbs and rocks | 9 |
//! | `wildlife` | by region, biome and threat | 9 |
//! | `cut_through` | a way cut to any named place the wood closed round | 9 |
//! | `drop_unreachable` | small places and creatures nobody can reach are dropped | 9 |
//!
//! Same seed, same county.

pub mod chunks;
pub mod country;
pub mod doors;
pub mod land;
pub mod links;
pub mod paths;
pub mod rail;
pub mod roads;
pub mod small;

use jane_core::num::Permille;
use jane_core::{Blueprint, Grid, Rect, Tile, ZoneId};

pub use self::chunks::Chunk;
use crate::kit::Kit;
use crate::skeleton::{COUNTY_H, COUNTY_W, MACRO, Skeleton, SkeletonError, SkeletonRows, build_skeleton};

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
    /// The skeleton's small places, in its order; the placements stage may re-kind a rolled one
    /// before `small_places` dresses them.
    pub pois: Vec<small::PoiSpot>,
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
            pois: small::poi_spots(sk),
        }
    }

    /// The finished blueprint.
    pub fn done(self) -> Blueprint {
        self.k.done("Castle", false, Permille::ONE)
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
    ("path_ends", paths::path_ends),
    ("doors", doors::set_doors),
    ("place_chunks", place_chunks),
    ("road_furniture", road_furniture),
    ("relays", relays),
    ("small_places", small_places),
    ("country", country),
    ("stories", stories),
    ("scatter", scatter),
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

/// The county over a skeleton already built (the caller keeps it: the county's re-rolls and its
/// tests read it).
pub fn build_county_on(sk: &Skeleton, attempt: u8) -> Blueprint {
    let mut c = County::new(sk, attempt);
    for (_, stage) in STAGES {
        stage(&mut c);
    }
    c.done()
}

// --- stages of later ports -------------------------------------------------------------------

/// Patches that are places, dressed before the roads so a road that crosses one simply crosses
/// it (`AREA_DRESS`, `world/areas.ts`). PORT.md §6.m stage 7 lands here.
fn areas(_: &mut County<'_>) {}

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
    links::link_lines(c);
}

/// Placement rows that go inside a chunk, while its open ground is still open; then every chunk's
/// ground claimed. PORT.md §6.m stage 7 lands here.
fn place_chunks(_: &mut County<'_>) {}

/// The roads' furniture while their margins are open (lamps, a lamp at each end of a bridge, a
/// fingerpost at every fork, milestones: `country::furnish_roads`), then every road's, path's
/// and footpath's margin claimed, three cells each side of its centre line.
fn road_furniture(c: &mut County<'_>) {
    // Every chunk's ground, six cells round its box, is spoken for before anything is built
    // round it. `place_chunks` claims it too once its rows are in; a claim twice is one claim.
    for ch in &c.chunks {
        c.k.claim(ch.bounds.grow(6));
    }
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

/// The skeleton's small places dressed as their kind, roadside signposts given words, each
/// needed place's rect (`small::small_places`). The `pois` placement rows, the small places'
/// claim (`small::claim_small_places`) and the `areas` placement rows follow, in that order.
fn small_places(c: &mut County<'_>) {
    small::small_places(c);
}

/// Everything else a county has in it: the small places claimed (again, if the placements stage
/// did), then field edges, the stories' quota, hamlets, farms, camps, dens, ruins, ponds
/// (`country::furnish_places`). Leaves `County::places` for the stories stage.
fn country(c: &mut County<'_>) {
    small::claim_small_places(c);
    country::furnish_places(c);
}

/// The stories claim places, the boards go up with the places' names, each story's rows go into
/// its place, and `Blueprint::stories` records where each landed. PORT.md §6.m stage 8 lands here.
fn stories(_: &mut County<'_>) {}

/// Herbs and rocks on open unclaimed ground. PORT.md §6.m stage 9 lands here.
fn scatter(_: &mut County<'_>) {}

/// Wildlife by region, biome and threat, wanderers along the field edges, then something small
/// on every screen still empty (`country::furnish_life`).
fn wildlife(c: &mut County<'_>) {
    country::furnish_life(c);
}

/// A way cut through thicket to any named place the flood from `start` never reached. PORT.md
/// §6.m stage 9 lands here.
fn cut_through(_: &mut County<'_>) {}

/// Small places and creatures the flood from `start` never reached are dropped. PORT.md §6.m
/// stage 9 lands here.
fn drop_unreachable(_: &mut County<'_>) {}
