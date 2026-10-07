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

use alloc::boxed::Box;

use jane_core::hash::mix32;
use jane_core::num::{CELL_FX, dist_sq};
use jane_core::{Fx, Vec2};
use jane_data::{Light, PropDef};
use once_cell::race::OnceBox;

use crate::runtime::ZoneRuntime;
use crate::state::{Prop, ZoneState};
use crate::tuning::{LAMP_STAGGER, LAMPS_OFF, LAMPS_ON, TICKS_PER_DAY};

/// Lamp posts burn from 18:30 to 06:30, as they did in 2020 (`clock` is ticks since midnight):
/// the night as the county keeps it. Each lamp keeps it a little early or late ([`lamp_lit`]).
pub const fn lamps_lit(clock: u32) -> bool {
    clock > LAMPS_ON || clock < LAMPS_OFF
}

/// Is lamp `id` alight at `clock`? [`lamps_lit`] a little early or late for each: it comes on
/// and goes out at its own minute within [`LAMP_STAGGER`] either side of the hour, the same
/// minute every day, from a hash of its id (the owner's first playtest: "lights all come on at
/// once"). What a sentry sees by and what is drawn lit are this, through [`light_showing`].
pub const fn lamp_lit(clock: u32, id: u32) -> bool {
    let off = mix32(id ^ 0x6c61_6d70) % (2 * LAMP_STAGGER);
    lamps_lit((clock % TICKS_PER_DAY + TICKS_PER_DAY + LAMP_STAGGER - off) % TICKS_PER_DAY)
}

/// Is this prop's light on at `clock`? Its lamp hours are [`lamp_lit`]'s, by its id; `wetness`
/// is the rain ramp where the prop stands (ARCHITECTURE.md §4.6.b, [`prop_wetness`]): a light
/// whose def has a `douse` is out while the ramp stands at or over it (an open fire in the rain;
/// still a fire to rest at).
pub fn light_showing(def: &'static PropDef, p: &Prop, clock: u32, wetness: u8) -> Option<&'static Light> {
    if p.hidden || def.light.is_none() {
        return None;
    }
    let lamps = lamp_lit(clock, p.id.get());
    if !def.light_shows(p.on, lamps) || def.douse.is_some_and(|d| wetness >= d) {
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
    static R: OnceBox<Fx> = OnceBox::new();
    *R.get_or_init(|| {
        let cat = jane_data::catalog();
        Box::new(cat.story.props.iter().filter_map(|d| d.light.map(|l| l.radius)).max().unwrap_or(Fx::ZERO))
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
    let (x0, y0) = (Fx(at.x.0 - reach.0).cell(), Fx(at.y.0 - reach.0).cell());
    let (x1, y1) = (Fx(at.x.0 + reach.0).cell(), Fx(at.y.0 + reach.0).cell());
    rt.props.any_in(x0, y0, x1, y1, |ix| {
        let p = &zone.props[ix as usize];
        let def = cat.story.prop(p.def);
        let Some(light) = light_showing(def, p, clock, prop_wetness(zone, rt, p)) else { return false };
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

    /// The lamps come on one at a time over half an hour round half past six, each at its own
    /// minute every evening, and go out so in the morning; the county's night is the same.
    #[test]
    fn each_lamp_comes_on_at_its_own_minute() {
        let (on, off) = (18 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2, 6 * TICKS_PER_HOUR + TICKS_PER_HOUR / 2);
        let lamps = 1..=400u32;
        // Its own minute: the first tick it is lit, per lamp.
        let lit_from = |id: u32| (on - LAMP_STAGGER..=on + LAMP_STAGGER).find(|&c| lamp_lit(c, id)).unwrap();
        let mut minutes: Vec<u32> = lamps.clone().map(|id| lit_from(id) / (TICKS_PER_HOUR / 60)).collect();
        for id in lamps.clone() {
            let from = lit_from(id);
            assert!((on - LAMP_STAGGER..=on + LAMP_STAGGER).contains(&from), "lamp {id} at {from}");
            assert!(!lamp_lit(from - 1, id) && lamp_lit(from + TICKS_PER_HOUR, id), "lamp {id} stays on");
            // Every day the same.
            assert_eq!(lamp_lit(from + TICKS_PER_DAY, id), lamp_lit(from, id));
            // Out in the morning within the same half hour, and all day.
            assert!(lamp_lit(off - LAMP_STAGGER - 1, id) && !lamp_lit(off + LAMP_STAGGER, id));
            assert!(!lamp_lit(12 * TICKS_PER_HOUR, id) && lamp_lit(0, id));
        }
        minutes.sort();
        minutes.dedup();
        assert!(minutes.len() >= 25, "spread over the half hour: {} minutes of 30", minutes.len());
        // Half past six: about half are lit.
        let half = lamps.filter(|&id| lamp_lit(on, id)).count();
        assert!((120..=280).contains(&half), "{half} of 400 lit at the hour");
    }

    #[test]
    fn two_thirds_of_the_radius() {
        // 36 px: lit to 24 px.
        let r = Fx::from_px(36);
        assert!(i64::from(Fx::from_px(24).0).pow(2) <= reach_sq(r));
        assert!(i64::from(Fx::from_px(25).0).pow(2) > reach_sq(r));
    }
}
