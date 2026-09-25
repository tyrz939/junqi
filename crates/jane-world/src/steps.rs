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
