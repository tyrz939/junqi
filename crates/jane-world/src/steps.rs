//! Every worldgen stage that throws dice, as a closed enum (PORT.md §6.a). A stage draws only
//! from `dice(seed, zone, step, attempt, a, b)` with its own `Step`, so re-tuning one stage
//! moves nothing drawn under any other.
//!
//! **Rule:** append only. A step's number is part of every seed's county.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum Step {
    /// The land: river, hill, lake, region borders.
    SkelTerrain = 1,
    /// Salts for the terrain's noise fields (one draw per field, in order).
    SkelTerrainNoise,
    SkelSite,
    SkelRoad,
    SkelRail,
    SkelAreas,
    SkelLamps,
    SkelPoi,
    SkelAnchor,
    CountyLand,
    CountyRoad,
    CountyPath,
    CountyChunk,
    CountyDoor,
    CountySmall,
    CountyPlaceRow,
    CountyStory,
    CountyNamePool,
    CountyHerbs,
    CountyRocks,
    CountyWild,
    DunChoose,
    DunEmbed,
    DunFill,
    DunLights,
    DunDress,
    /// Julie's house: `a` is the decision (0 the kitchen doorway, 1 the front room's table).
    IntHouse,
    /// The cellar: `a` is the decision (the rooms' depths, each pile, the rats, the roses).
    IntCellar,
    /// A road's field edges: `a` is the road (`walk_key` of its ends).
    CountyFences,
    /// The roadside beat: `a` is the road, `b` the point of its line (-1: where the beat starts).
    CountyAlong,
    /// A point of the country's lattice: `(a, b)` is its column and row.
    CountyLattice,
    /// A country place's furnishings: `a` is its centre cell (`y * COUNTY_W + x`), `b` its kind.
    CountyPlace,
    /// The stories' quota's shuffle of off-road spots: `a` names the region and kind, `b` the pass.
    CountyQuota,
    /// A road's wanderers: `a` is the road.
    CountyWander,
    /// An empty screen's something small: `(a, b)` is the screen.
    CountyGap,
    /// A person's hours on a day (not worldgen: the sim draws it, the same for every seat and
    /// every load): `a` is the person's name, `b` the day (WORLD.md §3.1).
    SimHours,
    /// What stands up at the bell on a road's unlit edge and on the rough ground: `a` is the road
    /// (`-1`: the ground), `b` the point of its line (the macro cell's `y * SKEL_W + x`).
    CountyNight,
    /// The bones about Julie's yard, outside its fence (`life::yard_bones`): one set of dice.
    CountyYard,
    /// A named patch's edge (`county::perimeter`): `a` is the patch's row in the area table.
    CountyPerimeter,
}

impl From<Step> for u16 {
    fn from(s: Step) -> u16 {
        s as u16
    }
}

/// The dice of one step of one zone's build.
pub fn dice(seed: u32, zone: jane_core::ZoneId, step: Step, attempt: u8, a: i32, b: i32) -> jane_core::Sfc32 {
    jane_core::dice(seed, zone.into(), step.into(), attempt, a, b)
}
