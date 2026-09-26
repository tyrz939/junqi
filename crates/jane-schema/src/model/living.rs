//! The living world's tables (ARCHITECTURE.md §4.6, WORLD.md): the weather by region, the
//! ecology of the county's patches, the consequences a quest or a death leaves, and the sim's
//! own tuning for them.
//!
//! Units: hours are 0..=23 and a span `hour_from..hour_to` may wrap midnight; weights are out of
//! 1000; a duration band is whole hours; pressure, weights, holds and recovery are plain counts
//! (a kill adds its row's `weight`, every ten game minutes the area's `recover` comes off, give
//! or take half).

use jane_core::action::{Condition, ListRef};
use jane_core::ids::{NameId, TextId, UnitDefId, ZoneId};
use jane_core::num::Tick;

use crate::model::{Region, Spreads};
use crate::{model, model_enum};

model_enum! {
    /// What the sky is doing (WORLD.md §5.1). Index order is the weight order of a band.
    pub enum Sky { Clear, Mist, Rain, Storm }
}

impl Sky {
    pub const ALL: [Sky; 4] = [Sky::Clear, Sky::Mist, Sky::Rain, Sky::Storm];

    /// Rain and storm wet the ground; clear and mist dry it.
    pub const fn wets(self) -> bool {
        matches!(self, Sky::Rain | Sky::Storm)
    }
}

model_enum! {
    /// The season a band belongs to. Autumn is the fixed season (WORLD.md §2.4); the column is
    /// there so a drift over the weeks is a row, not a system.
    pub enum Season { Autumn }
}

model! {
    /// One hour band of a region's weather: the chance of each kind, out of 1000, when the hour
    /// turns inside `hour_from..hour_to`.
    pub struct SkyBand {
        pub season: Season,
        pub hour_from: u8,
        pub hour_to: u8,
        /// By [`Sky`] order; they sum to 1000.
        pub weights: [u16; 4],
    }
}

model! {
    /// A region's sky (`data/weather.json`, one row per region, in region order).
    pub struct RegionWeather {
        pub region: Region,
        /// The zones under this sky; every zone is under exactly one.
        pub zones: &'static [ZoneId],
        /// Every hour of the season covered exactly once.
        pub bands: &'static [SkyBand],
        /// How long a kind lasts once drawn, whole hours `(min, max)`, by [`Sky`] order; min >= 1.
        pub last: [(u8, u8); 4],
    }
}

impl RegionWeather {
    /// The band an hour falls in (the season is autumn).
    pub fn band_at(&self, hour: u8) -> Option<&'static SkyBand> {
        let bands: &'static [SkyBand] = self.bands;
        bands.iter().find(|b| in_span(hour, b.hour_from, b.hour_to))
    }
}

/// `from` up to `to`, wrapping midnight; `from == to` is the whole day.
pub const fn in_span(hour: u8, from: u8, to: u8) -> bool {
    if from < to {
        hour >= from && hour < to
    } else if from > to {
        hour >= from || hour < to
    } else {
        true
    }
}

model! {
    /// One population of a patch: a unit row, how many may stand at once, what a kill costs the
    /// patch and the line past which nothing stands up again (ARCHITECTURE.md §4.6.c).
    pub struct Population {
        pub unit: UnitDefId,
        /// Living units of this row whose home is in the patch, at most; >= 1.
        pub cap: u8,
        /// Pressure a kill of one adds; >= 1.
        pub weight: u16,
        /// While the patch's pressure is at or over this, a corpse due waits for the next ten-minute
        /// mark; >= 1.
        pub hold: u16,
    }
}

model! {
    /// A patch's ecology (`data/ecology.json`): what lives there and how fast it forgets a hunt.
    pub struct EcologyDef {
        /// A skeleton area (`data/areas.json`).
        pub area: NameId,
        /// Pressure taken off every ten game minutes, give or take half (drawn from the world
        /// stream).
        pub recover: u16,
        pub populations: &'static [Population],
    }
}

model! {
    /// Something the county does once, for good, when a quest is done, a flag is set or a named
    /// thing dies (`data/consequences.json`; ARCHITECTURE.md §4.6.d). Indexed by `ConsequenceId`,
    /// ids in sorted order.
    pub struct ConsequenceDef {
        /// The content id (`"allotments_thinned"`).
        pub id: &'static str,
        /// `Flag`, `QuestDone` or `Dead`, never negated.
        pub on: Condition,
        /// Where the edits land: now if someone is there, else when the zone is next live.
        pub zone: ZoneId,
        /// World verbs only (`Show`, `Hide`, `Lock`, `Unlock`, `Switch`, `Spawn`, `Despawn`, `Fill`,
        /// `Send`, `Flag`: `Action::is_world_verb`).
        pub edits: ListRef,
        /// A claim the world bears out, as the journal records it.
        pub confirms: Option<TextId>,
        /// A claim the world gives the lie to.
        pub contradicts: Option<TextId>,
        /// Who hears of it, and when: each group from the tick it fires plus its `after`, the
        /// first to hear first (the town's news; `Condition::SpeakerHeard`).
        pub spreads: &'static [Spreads],
    }
}

model! {
    /// The rain ramp (`data/tuning/sim.json` `wetness`): every `every` ticks each outdoor zone's
    /// wetness climbs by `rise` while its sky wets and falls by `fall` while it does not, 0..=255.
    pub struct WetnessTuning {
        pub every: Tick,
        pub rise: u8,
        pub fall: u8,
    }
}

model! {
    /// `data/tuning/sim.json`: the sim's numbers that are content.
    pub struct SimTuning {
        /// Journal entries kept per kind (ARCHITECTURE.md §3.7, §12).
        pub journal_ring: u16,
        pub wetness: WetnessTuning,
        /// Hours from New Game in which the sky stays clear whatever is drawn (the first walk).
        pub clear_hours: u8,
    }
}

model! {
    pub struct Living {
        /// One row per region, in region order.
        pub weather: &'static [RegionWeather],
        /// In file order.
        pub ecology: &'static [EcologyDef],
        /// Indexed by `ConsequenceId`.
        pub consequences: &'static [ConsequenceDef],
        pub tuning: SimTuning,
    }
}

impl Living {
    /// A region's sky.
    pub fn sky(&self, r: Region) -> Option<&'static RegionWeather> {
        let rows: &'static [RegionWeather] = self.weather;
        rows.iter().find(|w| w.region == r)
    }

    /// The region a zone is under (the first row naming it; Lowfields if none does).
    pub fn region_of(&self, z: ZoneId) -> Region {
        self.weather.iter().find(|w| w.zones.contains(&z)).map_or(Region::Lowfields, |w| w.region)
    }

    /// A patch's ecology row by its area name.
    pub fn ecology_of(&self, area: NameId) -> Option<&'static EcologyDef> {
        let rows: &'static [EcologyDef] = self.ecology;
        rows.iter().find(|e| e.area == area)
    }

    /// A consequence row's id by its content id, by a scan: for tools and tests.
    pub fn consequence_id(&self, id: &str) -> Option<jane_core::ids::ConsequenceId> {
        self.consequences.iter().position(|c| c.id == id).map(|i| jane_core::ids::ConsequenceId(i as u16))
    }
}

impl EcologyDef {
    pub fn population(&self, unit: UnitDefId) -> Option<&'static Population> {
        let rows: &'static [Population] = self.populations;
        rows.iter().find(|p| p.unit == unit)
    }
}
