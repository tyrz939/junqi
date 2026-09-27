//! The atmosphere's layers (WORLD.md §5.3; PRESENTATION.md §1.9): fog volumes the renderer
//! draws from data, keyed to the skeleton's areas, the zone's rects (its sites), a region or a
//! zone, so a layer lands where its place is on every seed. Presentation only: like the looks, a
//! static of its own outside the catalog and its content hash, and it changes no state.
//!
//! Units: hours 0..=23, a span `hour_from..hour_to` wrapping midnight (`from == to` is all day);
//! density per mille; drift canvas px a second; spread cells.

use jane_core::ids::ZoneId;

use crate::model::Region;
use crate::{model, model_enum};

model_enum! {
    /// How high a layer stands: a ground fog to `top` px (a thing taller shows its head and
    /// shoulders over it), or the full height of the air.
    pub enum FogHeight { Ground, Full }
}

model! {
    /// One layer (`data/atmosphere.json`, keyed by its id).
    pub struct AtmosLayer {
        /// The content id (`"fen_mist"`).
        pub id: &'static str,
        /// Skeleton areas it covers (`View::areas`), by name.
        pub areas: &'static [&'static str],
        /// Zone rects it covers (a site is `site_<name>`), by name.
        pub rects: &'static [&'static str],
        /// Regions whose sky it hangs under, over the whole view while she stands in one.
        pub regions: &'static [Region],
        /// Zones it fills whole.
        pub zones: &'static [ZoneId],
        pub hour_from: u8,
        pub hour_to: u8,
        /// By `Sky` order (clear, mist, rain, storm): the weather it shows in.
        pub needs: [bool; 4],
        /// `0xRRGGBB`.
        pub colour: u32,
        /// At its thickest, per mille.
        pub density: u16,
        pub height: FogHeight,
        /// A ground fog's height, px.
        pub top: u8,
        /// Canvas px a second, east and south.
        pub drift: (i8, i8),
        /// Cells it spills past the rects it covers, fading.
        pub spread: u8,
    }
}

model! {
    /// Every layer, in id order.
    pub struct Atmosphere {
        pub layers: &'static [AtmosLayer],
    }
}

impl Atmosphere {
    /// No layers: what a fixture without `data/atmosphere.json` compiles to.
    pub const EMPTY: Atmosphere = Atmosphere { layers: &[] };
}

impl AtmosLayer {
    /// Whether the layer shows at `hour` under a sky of kind `sky` (its index in `Sky` order).
    pub fn shows(&self, hour: u8, sky: usize) -> bool {
        crate::model::in_span(hour, self.hour_from, self.hour_to) && self.needs.get(sky).copied().unwrap_or(false)
    }
}
