//! Light (PRESENTATION.md §1.7): the sky by the clock. Outdoors the ambient comes from the
//! clock's keyframes and the sun and the moon cross the sky as one directional light; indoors
//! the zone's own light, blue-tinted, and no sky at all. What is lit (a lamp, a glow) is the
//! view's to say; this module says only what colour the sky makes it, and how a flame flickers.
//!
//! Integer throughout, so the sun's angle a T0 frame shears its silhouettes by is the same on
//! every target.

use jane_core::Angle;
use jane_core::angle::sin_q15;
use jane_data::Region;

use crate::frame::{Directional, Post, Rgb};

/// Ticks in an hour of the clock (60 ticks a second, 2 real minutes an hour).
const HOUR: i32 = 7200;

const NIGHT: [i32; 3] = [58, 66, 116];
const DAWN: [i32; 3] = [230, 190, 190];
const DAY: [i32; 3] = [255, 255, 255];
const SUNSET: [i32; 3] = [255, 196, 150];
const DUSK: [i32; 3] = [150, 120, 170];

/// `(tick of the day, colour)`: linear between them (the TS build's keys, so 17:00 when she
/// steps off the train lands in the sunset).
const KEYS: [(i32, [i32; 3]); 9] = [
    (0, NIGHT),
    (HOUR * 9 / 2, NIGHT),
    (HOUR * 6, DAWN),
    (HOUR * 15 / 2, DAY),
    (HOUR * 33 / 2, DAY),
    (HOUR * 18, SUNSET),
    (HOUR * 39 / 2, DUSK),
    (HOUR * 21, NIGHT),
    (HOUR * 24, NIGHT),
];

/// The sky's own light, what a shadow is lit by on T1 and T2: blue by day, violet at dusk, a
/// deep blue-violet at night that keeps its value (ART.md §3.1, "night is beautiful").
const FILL_KEYS: [(i32, [i32; 3]); 10] = [
    (0, [62, 62, 128]),
    (HOUR * 9 / 2, [62, 62, 128]),
    (HOUR * 6, [140, 112, 150]),
    (HOUR * 15 / 2, [132, 150, 196]),
    (HOUR * 16, [132, 150, 196]),
    (HOUR * 17, [126, 124, 186]),
    (HOUR * 18, [104, 100, 168]),
    (HOUR * 39 / 2, [66, 72, 146]),
    (HOUR * 21, [62, 62, 128]),
    (HOUR * 24, [62, 62, 128]),
];

/// Sunrise and sunset: the sun is up between them and its share is gone at 18:30, when the
/// lamps come on.
const RISE: i32 = HOUR * 11 / 2;
const SET: i32 = HOUR * 37 / 2;
/// The sun's height at noon and the moon's at its highest.
const SUN_TOP: i32 = 46;
const MOON_TOP: i32 = 38;
/// The sun on flat ground high in the sky, and low, on the horizon's edge.
const SUN_HIGH: [i32; 3] = [214, 204, 184];
const SUN_LOW: [i32; 3] = [255, 176, 96];
/// The full moon on flat ground.
const MOON: [i32; 3] = [58, 74, 120];

/// The whole sky at one moment: what the `Lights`, `Silhouettes` and `Post` passes carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sky {
    /// The flat light T0 multiplies by (the keyframes of [`ambient`]).
    pub ambient: Rgb,
    /// The sky's own light, what a surface in shadow is lit by on T1 and T2.
    pub fill: Rgb,
    /// The sun by day, the moon by night; none indoors.
    pub sun: Option<Directional>,
    /// A silhouette shadow's multiplier on T0 and T1: the fill's share of the light, cool.
    pub shade: Rgb,
    /// The grade of the region at this hour.
    pub post: Post,
}

/// The ambient per channel (255 is full light) at `clock` ticks since midnight; indoors, the
/// zone's `permille` instead, a little blue.
pub fn ambient(clock: u32, indoor: bool, permille: i16) -> [u8; 3] {
    if indoor {
        let v = i32::from(permille.clamp(0, 1000)) * 255 / 1000;
        return [(v - v / 10) as u8, (v - v / 20) as u8, v as u8];
    }
    keyed(&KEYS, clock)
}

/// A keyframe table at `clock`, linear between keys.
fn keyed(keys: &[(i32, [i32; 3])], clock: u32) -> [u8; 3] {
    let t = (clock % (24 * HOUR as u32)) as i32;
    for w in keys.windows(2) {
        let ((t0, c0), (t1, c1)) = (w[0], w[1]);
        if t >= t0 && t <= t1 {
            let span = (t1 - t0).max(1);
            return [0, 1, 2].map(|k| (c0[k] + (c1[k] - c0[k]) * (t - t0) / span).clamp(0, 255) as u8);
        }
    }
    keys[0].1.map(|c| c.clamp(0, 255) as u8)
}

/// Whole degrees as an [`Angle`] (a turn is 65536).
const fn deg(d: i32) -> i32 {
    d * 65536 / 360
}

/// A body crossing the sky from east to west over `from..to` (ticks of the day, `to` may pass
/// midnight), `top` degrees high at its height: `(azimuth, elevation, sin elevation Q15)`, or
/// `None` below the horizon.
fn arc(t: i32, from: i32, to: i32, top: i32) -> Option<(Angle, Angle, i32)> {
    let day = 24 * HOUR;
    let (len, into) = ((to - from).rem_euclid(day), (t - from).rem_euclid(day));
    if into >= len {
        return None;
    }
    // East (0) through south (a quarter turn) to west (a half turn), swung west by up to 16
    // degrees at its height: the county is north of the sun, so at noon shadows lean
    // north-north-east, clear of what casts them, and at five they point east, a little north.
    let half = (into as i64 * 32768 / len as i64) as i32;
    let rise = sin_q15(Angle(half as u16)).0;
    let elevation = (deg(top) * rise) >> 15;
    let sin_el = sin_q15(Angle(elevation as u16)).0;
    Some((Angle((half + ((deg(16) * rise) >> 15)) as u16), Angle(elevation as u16), sin_el))
}

/// `a` toward `b` by `num / den`.
fn mix(a: [i32; 3], b: [i32; 3], num: i32, den: i32) -> [i32; 3] {
    let n = num.clamp(0, den);
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * n / den.max(1))
}

/// The moon's brightness by the day, 0..=256: full on day 0 of 16, new on day 8 (never quite
/// dark, so a new-moon night still reads).
fn moon_phase(day: u32) -> i32 {
    let d = (day % 16) as i32;
    let from_full = d.min(16 - d);
    256 - from_full * 22
}

/// The sky at `clock` ticks since midnight on `day`: outdoors by the hour, indoors the zone's
/// `permille`, in `region` (its grade).
pub fn sky(clock: u32, day: u32, indoor: bool, permille: i16, region: Region) -> Sky {
    let flat = ambient(clock, indoor, permille);
    if indoor {
        let [r, g, b] = flat;
        // Indoors the zone's light fills the room; the lamps and the fire do the rest.
        let fill = [r / 2 + r / 8, g / 2 + g / 8, b / 2 + b / 4];
        let post = Post { tint: [255, 250, 240], lift: [10, 8, 18], saturation: 136, bloom: 150, exposure: 136 };
        return Sky { ambient: flat, fill, sun: None, shade: [150, 150, 190], post };
    }
    let t = (clock % (24 * HOUR as u32)) as i32;
    let fill = keyed(&FILL_KEYS, clock);
    let sun = if let Some((az, el, s)) = arc(t, RISE, SET, SUN_TOP) {
        // Warm and low toward the horizon, white overhead; gone as it touches the horizon.
        let colour = mix(SUN_LOW, SUN_HIGH, s, 20000);
        let colour = mix([0; 3], colour, s, 3400);
        Some(Directional {
            azimuth: az,
            elevation: el,
            colour: colour.map(|c| c.clamp(0, 255) as u8),
            spread: deg(2) as u16 + (deg(3) * (20000 - s.min(20000)) / 20000) as u16,
        })
    } else if let Some((az, el, s)) = arc(t, SET, RISE, MOON_TOP) {
        let phase = moon_phase(day);
        let colour = mix([0; 3], MOON.map(|c| (c * phase) >> 8), s, 6000);
        Some(Directional {
            azimuth: az,
            elevation: el,
            colour: colour.map(|c| c.clamp(0, 255) as u8),
            spread: deg(3) as u16,
        })
    } else {
        None
    };
    let shade = shade(fill, sun.map_or([0; 3], |s| s.colour));
    Sky { ambient: flat, fill, sun, shade, post: grade(region, t) }
}

/// A silhouette's multiplier: the fill's share of fill and sun, eased toward none so a shadow
/// darkens by at most about half.
fn shade(fill: Rgb, sun: Rgb) -> Rgb {
    [0, 1, 2].map(|k| {
        let (f, s) = (i32::from(fill[k]), i32::from(sun[k]));
        let share = f * 255 / (f + s).max(1);
        (255 - (255 - share) * 7 / 8).clamp(0, 255) as u8
    })
}

/// A region's grade at tick `t` of the day (§1.9): the Lowfields warm and green-gold, the Waters
/// cold blue-green, the Works sodium and soot; dusk leans violet and night blue.
fn grade(region: Region, t: i32) -> Post {
    let (tint, lift, saturation): (Rgb, Rgb, u8) = match region {
        Region::Lowfields => ([255, 250, 238], [5, 3, 12], 136),
        Region::Waters => ([238, 250, 255], [2, 7, 14], 130),
        Region::Works => ([255, 240, 218], [8, 5, 6], 116),
    };
    let dusk = (HOUR * 17..HOUR * 21).contains(&t);
    let night = !(HOUR * 5..HOUR * 21).contains(&t);
    let lift = if night {
        [lift[0] / 2 + 3, lift[1] / 2 + 4, lift[2] + 8]
    } else if dusk {
        [lift[0] + 2, lift[1] / 2, lift[2] + 4]
    } else {
        lift
    };
    let exposure = if night { 150 } else { 128 };
    Post { tint, lift, saturation, bloom: if night || dusk { 190 } else { 110 }, exposure }
}

/// A flame's brightness this tick, 0..=255 of full: `dip` permille of full at most, by a
/// 64-step table walked at `rate` steps a second from a place set by `id` (§1.7).
pub fn flicker(tick: u32, id: u32, dip: i16, rate: u32) -> u8 {
    if dip <= 0 {
        return 255;
    }
    // Ticks per step, and where in the step this tick is.
    let per = (60 / rate.max(1)).max(1);
    let at = tick / per + (id.wrapping_mul(2_654_435_761) >> 26);
    let frac = (tick % per) as i32;
    let a = i32::from(FLICKER[(at & 63) as usize]);
    let b = i32::from(FLICKER[((at + 1) & 63) as usize]);
    let n = a + (b - a) * frac / per as i32;
    let dip = i32::from(dip.clamp(0, 1000));
    (255 - n * dip / 1000).clamp(0, 255) as u8
}

/// How deep a flame dips at each step, 0..=255: mostly shallow, now and then a gutter.
const FLICKER: [u8; 64] = [
    40, 72, 30, 96, 58, 20, 110, 64, 36, 150, 88, 44, 18, 70, 124, 60, 32, 84, 200, 90, 46, 24, 66, 102, 52, 28, 140,
    76, 38, 12, 92, 56, 118, 48, 26, 80, 170, 94, 42, 22, 62, 108, 54, 34, 16, 86, 132, 68, 30, 100, 60, 26, 184, 82,
    40, 20, 74, 114, 50, 24, 96, 144, 58, 36,
];

/// Her lantern is lit when the flat light is below 750 permille (§1.7).
pub fn lantern_lit(ambient: Rgb) -> bool {
    let luma = u32::from(ambient[0]) * 3 + u32::from(ambient[1]) * 6 + u32::from(ambient[2]);
    luma * 1000 / (255 * 10) < 750
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_full_and_midnight_is_dark_and_blue() {
        assert_eq!(ambient(12 * 7200, false, 1000), [255; 3]);
        let n = ambient(0, false, 1000);
        assert!(n[2] > n[0] && n[0] < 100, "{n:?}");
        assert_eq!(ambient(23 * 7200, false, 1000), n);
        // 17:00 is on the way to sunset; 20:00 is on the way to night.
        let five = ambient(17 * 7200, false, 1000);
        assert!(five[0] == 255 && five[2] < 255, "{five:?}");
        assert!(ambient(20 * 7200, false, 1000)[0] < five[0]);
    }

    #[test]
    fn indoors_is_the_zone_light() {
        assert_eq!(ambient(0, true, 1000), [230, 243, 255]);
        let dim = ambient(12 * 7200, true, 200);
        assert!(dim[2] < 60, "{dim:?}");
        assert!(sky(12 * 7200, 0, true, 500, Region::Lowfields).sun.is_none());
    }

    #[test]
    fn at_five_the_sun_is_low_in_the_west_and_gone_by_half_past_six() {
        let at = |h: i32, m: i32| sky((h * HOUR + m * 120) as u32, 0, false, 1000, Region::Lowfields);
        let five = at(17, 0).sun.expect("the sun is up at five");
        // Low: under 25 degrees, over 10, so a person's shadow is two to five times her height.
        assert!(five.elevation.0 > deg(10) as u16 && five.elevation.0 < deg(25) as u16, "{five:?}");
        // West, within 20 degrees: shadows point east.
        assert!(five.azimuth.0.abs_diff(Angle::WEST.0) < deg(20) as u16, "{five:?}");
        // Warm: more red than blue.
        assert!(five.colour[0] > five.colour[2] + 40, "{five:?}");
        let noon = at(12, 0).sun.unwrap();
        // High and south-south-west, so its short shadows lean clear of what casts them.
        assert!(noon.elevation.0 > five.elevation.0 * 2, "{noon:?}");
        assert!(noon.azimuth.0 > Angle::SOUTH.0 && noon.azimuth.0 < Angle::SOUTH.0 + deg(25) as u16, "{noon:?}");
        // By 18:30 the sun's share is gone, and the moon has only begun.
        let sunset = at(18, 29).sun.map_or(0, |s| s.colour.iter().map(|&c| u32::from(c)).sum::<u32>());
        assert!(sunset < 30, "{sunset}");
        // Midnight: a dim blue moon, and a blue-violet sky that keeps its value.
        let night = at(0, 0);
        let moon = night.sun.expect("the moon is up at midnight");
        assert!(moon.colour[2] > moon.colour[0] && moon.colour[2] < 140, "{moon:?}");
        assert!(night.fill[2] > night.fill[0] && night.fill[2] > 90, "{:?}", night.fill);
    }

    #[test]
    fn shadows_are_tinted_never_grey() {
        for h in [8, 12, 17] {
            let s = sky((h * HOUR) as u32, 0, false, 1000, Region::Lowfields);
            assert!(s.shade[2] > s.shade[0], "{h}: {:?}", s.shade);
            assert!(s.shade.iter().all(|&c| c > 100), "{h}: never black: {:?}", s.shade);
        }
    }

    #[test]
    fn a_flame_dips_no_deeper_than_its_row_and_a_steady_light_never_does() {
        assert!((0..600).all(|t| flicker(t, 7, 0, 12) == 255));
        let v: Vec<u8> = (0..600).map(|t| flicker(t, 7, 300, 12)).collect();
        assert!(v.iter().all(|&b| b >= 255 - 77), "{:?}", v.iter().min());
        assert!(v.windows(2).any(|w| w[0] != w[1]));
        // Two flames side by side do not dip together.
        assert_ne!(flicker(100, 1, 300, 12), flicker(100, 2, 300, 12));
    }

    #[test]
    fn her_lantern_is_lit_at_dusk_and_not_by_day() {
        assert!(!lantern_lit(ambient(12 * 7200, false, 1000)));
        assert!(!lantern_lit(ambient(17 * 7200, false, 1000)));
        assert!(lantern_lit(ambient(22 * 7200, false, 1000)));
        assert!(lantern_lit(ambient(0, true, 300)));
    }
}
