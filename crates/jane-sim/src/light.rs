//! Light the sim can see (`sim/light.ts`). In the county a lamp is safety; in the Works it is a
//! sentry's eye; in the Burial it keeps the dead off; in Butterfly Forest it is where things
//! grow. Every one of them asks the same question: is this point lit?
//!
//! - **Props only.** Her own glow, a bolt in flight and a bat's red lamp are presentation; if
//!   they counted she would light herself up for every sentry by casting.
//! - **One rule.** [`light_showing`] decides whether a prop's light is on, and presentation asks
//!   the same function (ARCHITECTURE.md §11 `light_showing`, `lamps_lit`), so what she sees lit
//!   and what the sim calls lit cannot drift.
//! - **No cache.** The answer is read off the prop buckets each time: a cached light map would
//!   have to hear of every lever, frost-lit torch, pushed brazier and both ends of the night,
//!   and the first it missed would make a loaded game differ from the one saved.
//!
//! A point is lit inside two thirds of a light's radius (`r * r * 4 / 9` in Fx², §2), where the
//! sprite's long soft tail still reads as light. A path search asks per cell through
//! [`crate::ai::LitField`], which gathers the lights near its window once.

use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Fx, Vec2};
use jane_data::{Light, PropDef};

use crate::runtime::ZoneRuntime;
use crate::state::{Prop, ZoneState};
use crate::tuning::{LAMPS_OFF, LAMPS_ON};

/// Lamp posts burn from 18:30 to 06:30, as in 2020. `clock` is ticks since midnight.
pub const fn lamps_lit(clock: u32) -> bool {
    clock > LAMPS_ON || clock < LAMPS_OFF
}

/// The prop's light, if it is showing. `lamps` is [`lamps_lit`], asked once by the caller.
pub fn light_showing(def: &'static PropDef, p: &Prop, lamps: bool) -> Option<&'static Light> {
    if p.hidden || !def.light_shows(p.on, lamps) { None } else { def.light.as_ref() }
}

/// Where a prop's light is: the middle of its footprint.
pub fn prop_centre(def: &PropDef, p: &Prop) -> Vec2 {
    let half = |cell: u16, cells: u8| Fx(i32::from(cell) * CELL_FX + i32::from(cells) * CELL_FX / 2);
    Vec2::new(half(p.cell.x, def.w), half(p.cell.y, def.h))
}

/// How far a light of `radius` counts as lit, squared.
pub const fn reach_sq(radius: Fx) -> i64 {
    let r = radius.0 as i64;
    r * r * 4 / 9
}

/// The widest light in the catalog: how far a question reaches back for one.
pub fn max_light_radius() -> Fx {
    static R: std::sync::OnceLock<Fx> = std::sync::OnceLock::new();
    *R.get_or_init(|| {
        jane_data::catalog().story.props.iter().filter_map(|d| d.light.map(|l| l.radius)).max().unwrap_or(Fx::ZERO)
    })
}

/// Is `at` inside a showing prop's light? `warm_only` leaves out lights marked `cold` (the
/// Burial's blue torches show what is there and keep nothing off).
pub fn lit_at(zone: &ZoneState, rt: &ZoneRuntime, clock: u32, at: Vec2, warm_only: bool) -> bool {
    let reach = max_light_radius().0;
    if reach <= 0 {
        return false;
    }
    let cat = jane_data::catalog();
    let lamps = lamps_lit(clock);
    let (x0, y0) = (Fx(at.x.0 - reach).cell(), Fx(at.y.0 - reach).cell());
    let (x1, y1) = (Fx(at.x.0 + reach).cell(), Fx(at.y.0 + reach).cell());
    rt.props.any_in(x0, y0, x1, y1, |ix| {
        let p = &zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        light_showing(def, p, lamps)
            .is_some_and(|l| !(warm_only && l.cold) && dist_sq(prop_centre(def, p), at) <= reach_sq(l.radius))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::TICKS_PER_HOUR;

    /// verbs2.test.ts E8 "a lamp post lights the ground from 18:30 to 06:30".
    #[test]
    fn the_lamps_burn_from_half_past_six_to_half_past_six() {
        let h = TICKS_PER_HOUR;
        assert!(!lamps_lit(12 * h));
        assert!(!lamps_lit(18 * h + h / 2));
        assert!(lamps_lit(18 * h + h / 2 + 1));
        assert!(lamps_lit(19 * h));
        assert!(lamps_lit(6 * h + h / 4));
        assert!(!lamps_lit(6 * h + h / 2));
    }

    /// verbs2.test.ts E8 "inside two thirds of the radius": a 60 px lamp lights 40 px.
    #[test]
    fn two_thirds_of_the_radius() {
        let r = reach_sq(Fx::from_px(60));
        assert!(i64::from(Fx::from_px(39).0).pow(2) <= r);
        assert!(i64::from(Fx::from_px(40).0).pow(2) <= r);
        assert!(i64::from(Fx::from_px(41).0).pow(2) > r);
    }
}
