//! Light that means something to the sim (`sim/light.ts`). A lamp is safety in the county, a
//! sentry's eye in the Factory, what keeps the dead off in the Burial and where things grow in
//! Butterfly Forest; all of them ask one question: is this point lit? Grow asks a narrower one,
//! is it lit from the sky ([`grows_at`]): a lamp or a light stone does not fool a seed.
//!
//! - **Props only.** Her glow, a bolt in flight and a bat's lamp are presentation.
//! - **One rule.** [`light_showing`] is THE rule for whether a prop's light is on; `View` hands
//!   the same function to presentation (ARCHITECTURE.md §11), so what she sees lit and what the
//!   sim calls lit cannot drift.
//! - **No cache.** The answer is read off the prop buckets each time it is asked: a cached map
//!   would have to hear about every lever, frost-lit torch, pushed brazier and both ends of the
//!   night, and the first one forgotten would make a loaded game differ from the saved one.
//!
//! A point counts as lit inside two thirds of the radius (`r * r * 4 / 9` in Fx², §2), where the
//! radial sprite's long soft tail still looks lit.

use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Fx, Vec2};
use jane_data::{Light, PropDef};

use crate::runtime::ZoneRuntime;
use crate::state::{Prop, ZoneState};
use crate::tuning::{LAMPS_OFF, LAMPS_ON};

/// Lamp posts burn from 18:30 to 06:30, as they did in 2020 (`clock` is ticks since midnight).
pub const fn lamps_lit(clock: u32) -> bool {
    clock > LAMPS_ON || clock < LAMPS_OFF
}

/// Is this prop's light on? `lamps` is [`lamps_lit`], asked once by the caller; `wetness` is the
/// rain ramp where the prop stands (ARCHITECTURE.md §4.6.b, [`prop_wetness`]): a light whose def
/// has a `douse` is out while the ramp stands at or over it (an open fire in the rain; still a
/// fire to rest at).
pub fn light_showing(def: &'static PropDef, p: &Prop, lamps: bool, wetness: u8) -> Option<&'static Light> {
    if p.hidden || !def.light_shows(p.on, lamps) || def.douse.is_some_and(|d| wetness >= d) {
        return None;
    }
    def.light.as_ref()
}

/// The rain ramp a prop stands under: that of the region of its cell.
pub fn prop_wetness(zone: &ZoneState, rt: &ZoneRuntime, p: &Prop) -> u8 {
    crate::living::wetness_at(zone, rt, i32::from(p.cell.x), i32::from(p.cell.y))
}

/// The middle of a prop's footprint.
pub fn prop_centre(def: &PropDef, p: &Prop) -> Vec2 {
    Vec2::new(
        Fx(i32::from(p.cell.x) * CELL_FX + i32::from(def.w) * CELL_FX / 2),
        Fx(i32::from(p.cell.y) * CELL_FX + i32::from(def.h) * CELL_FX / 2),
    )
}

/// How far a light of `radius` counts as lit, squared: two thirds of it.
pub const fn reach_sq(radius: Fx) -> i64 {
    let r = radius.0 as i64;
    r * r * 4 / 9
}

/// The largest light radius in the catalog: how far a query reaches back for one.
pub fn max_light_radius() -> Fx {
    static R: std::sync::OnceLock<Fx> = std::sync::OnceLock::new();
    *R.get_or_init(|| {
        let cat = jane_data::catalog();
        cat.story.props.iter().filter_map(|d| d.light.map(|l| l.radius)).max().unwrap_or(Fx::ZERO)
    })
}

/// Does a showing prop light cover the point? `warm_only` leaves out lights marked `cold` (the
/// Burial's blue torches show what is there and keep nothing off).
pub fn lit_at(zone: &ZoneState, rt: &ZoneRuntime, clock: u32, at: Vec2, warm_only: bool) -> bool {
    lit_by(zone, rt, clock, at, |l| !(warm_only && l.cold))
}

/// Will something grow at the point? Only in the sky's light: the sun, in the county (the one
/// zone under the open sky; the Forest's canopy lets it down only in its beams) while the lamps
/// are out, 06:30 to 18:30 (when a glade's `dayOnly` beam shows too); else a showing prop light
/// marked `sky` (a sunbeam, a moonbeam, the library's roof). A lamp, a fire, a torch, a light
/// stone set down, a bloomed bud's glow: none of them.
pub fn grows_at(zone: &ZoneState, rt: &ZoneRuntime, clock: u32, at: Vec2) -> bool {
    (zone.id == jane_core::ZoneId::County && !lamps_lit(clock)) || lit_by(zone, rt, clock, at, |l| l.sky)
}

/// Does a showing prop light that `counts` cover the point?
fn lit_by(zone: &ZoneState, rt: &ZoneRuntime, clock: u32, at: Vec2, counts: impl Fn(&Light) -> bool) -> bool {
    let reach = max_light_radius();
    if reach.0 <= 0 {
        return false;
    }
    let cat = jane_data::catalog();
    let lamps = lamps_lit(clock);
    let (x0, y0) = (Fx(at.x.0 - reach.0).cell(), Fx(at.y.0 - reach.0).cell());
    let (x1, y1) = (Fx(at.x.0 + reach.0).cell(), Fx(at.y.0 + reach.0).cell());
    rt.props.any_in(x0, y0, x1, y1, |ix| {
        let p = &zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        let Some(light) = light_showing(def, p, lamps, prop_wetness(zone, rt, p)) else { return false };
        counts(light) && dist_sq(prop_centre(def, p), at) <= reach_sq(light.radius)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::TICKS_PER_HOUR;

    #[test]
    fn lamps_burn_from_half_past_six_to_half_past_six() {
        assert!(!lamps_lit(18 * TICKS_PER_HOUR));
        assert!(!lamps_lit(18 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2));
        assert!(lamps_lit(18 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2 + 1));
        assert!(lamps_lit(0));
        assert!(lamps_lit(6 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2 - 1));
        assert!(!lamps_lit(6 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2));
    }

    #[test]
    fn two_thirds_of_the_radius() {
        // 36 px: lit to 24 px.
        let r = Fx::from_px(36);
        assert!(i64::from(Fx::from_px(24).0).pow(2) <= reach_sq(r));
        assert!(i64::from(Fx::from_px(25).0).pow(2) > reach_sq(r));
    }
}
