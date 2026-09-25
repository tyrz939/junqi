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
//! | `small_places` | the skeleton's small places dressed, signposts given words, anchors' rects | 7 |
//! | `place_pois` | placement rows at the small places, then the small places claimed | 7 |
//! | `place_areas` | placement rows in the named patches | 7 |
//! | `country` | field edges, hamlets, farms, camps, dens, ruins, ponds | 7 |
//! | `stories` | stories claim places, boards go up, the stories' rows | 8 |
//! | `scatter` | herbs and rocks | 9 |
//! | `wildlife` | by region, biome and threat | 9 |
//! | `cut_through` | a way cut to any named place the wood closed round | 9 |
//! | `drop_unreachable` | small places and creatures nobody can reach are dropped | 9 |
//!
//! Same seed, same county.

pub mod areas;
pub mod chunks;
pub mod doors;
pub mod finish;
pub mod land;
pub mod links;
pub mod paths;
pub mod placements;
pub mod rail;
pub mod roads;
pub mod tale_ground;

use jane_core::num::Permille;
use jane_core::{Blueprint, Grid, NameId, Rect, Tile, ZoneId};

pub use self::chunks::Chunk;
use self::placements::{PoiSpot, Stage, apply_placements, claim_pois};
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
            area_slots: Vec::new(),
            pois: PoiSpot::all(sk),
            claimed: Vec::new(),
            reached: None,
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
    ("place_pois", place_pois),
    ("place_areas", place_areas),
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
    links::link_lines(c);
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
/// fingerpost at every fork, milestones), then every road's margin claimed. PORT.md §6.m stage 7
/// lands here.
fn road_furniture(_: &mut County<'_>) {}

/// The longest dark stretches get a relay box and a run of dead lamps (`relayRuns`). PORT.md
/// §6.m stage 7 lands here.
fn relays(_: &mut County<'_>) {}

/// The skeleton's small places dressed as their kind (`smallPlace`), roadside signposts given
/// words, each needed place's rect, and the `pois` and `areas` placement rows. PORT.md §6.m
/// stage 7 lands here.
fn small_places(_: &mut County<'_>) {}

/// Everything else a county has in it (`furnishCountry(..., "places")`). PORT.md §6.m stage 7
/// lands here.
fn country(_: &mut County<'_>) {}

/// The stories claim places, the boards go up with the places' names, each story's rows go into
/// its place (`placements::apply_placements(c, Stage::Places, ..)`), and `Blueprint::stories`
/// records where each landed. PORT.md §6.m stage 8 lands here.
fn stories(_: &mut County<'_>) {}

/// Herbs and rocks on open unclaimed ground.
fn scatter(c: &mut County<'_>) {
    finish::scatter(c);
}

/// Wildlife by region, biome and threat (`furnishCountry(..., "life")`). PORT.md §6.m stage 9
/// lands here.
fn wildlife(_: &mut County<'_>) {}

/// A way cut through thicket to any named place the flood from `start` never reached.
fn cut_through(c: &mut County<'_>) {
    finish::cut_through(c);
}

/// Small places and creatures the flood from `start` never reached are dropped.
fn drop_unreachable(c: &mut County<'_>) {
    finish::drop_unreachable(c);
}
