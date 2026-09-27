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

/// The least light a zone indoors has, of 255.
const INDOOR_FLOOR: i32 = 70;

const NIGHT: [i32; 3] = [80, 104, 140];

/// `(tick of the day, colour)`: the flat light by the clock, linear between keys, 255 the art as
/// drawn and more past it. T2 is the reference look (decided 2026-09-27: the tiers are one game
/// at a glance), and these are what T2's fill and sun give flat ground on average, measured off
/// the review set with the grade off (`tools/art-review.sh`: the yard and the square, T0's
/// median against T2's per channel), the day's kept warm rather than the measured mauve (a flat
/// light has no blue shade beside a gold lit side to read against). T0 multiplies by them (past
/// 255 through its grade's exposure, `t0_gain`); T1 scales its fill and sun to their luma.
const KEYS: [(i32, [i32; 3]); 14] = [
    (0, NIGHT),
    (HOUR * 9 / 2, [76, 98, 136]),
    (HOUR * 6, [214, 168, 200]),
    (HOUR * 15 / 2, [268, 248, 240]),
    // Noon is white, so a T0 frame at midday needs no light pass (day is free) and only its
    // exposure: T2's noon is about an eighth brighter than the art as drawn.
    (HOUR * 12, [292, 292, 292]),
    (HOUR * 33 / 2, [270, 240, 222]),
    (HOUR * 17, [256, 222, 200]),
    (HOUR * 35 / 2, [240, 204, 180]),
    (HOUR * 18, [184, 160, 170]),
    (HOUR * 37 / 2, [163, 140, 160]),
    (HOUR * 75 / 4, [150, 132, 160]),
    (HOUR * 39 / 2, [104, 108, 155]),
    (HOUR * 21, NIGHT),
    (HOUR * 24, NIGHT),
];

/// The sky's own light, what a shadow is lit by on T1 and T2: blue by day, a deeper blue at dusk, a
/// deep blue-violet at night that keeps its value (ART.md §3.1, "night is beautiful").
const FILL_KEYS: [(i32, [i32; 3]); 10] = [
    (0, [48, 72, 132]),
    (HOUR * 9 / 2, [48, 72, 132]),
    (HOUR * 6, [140, 112, 150]),
    (HOUR * 15 / 2, [132, 150, 196]),
    (HOUR * 16, [132, 150, 196]),
    (HOUR * 17, [116, 132, 186]),
    (HOUR * 18, [84, 106, 170]),
    (HOUR * 39 / 2, [52, 76, 148]),
    (HOUR * 21, [48, 72, 132]),
    (HOUR * 24, [48, 72, 132]),
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
/// The broadest sun that still throws light shafts on T2 (5 degrees, and a little): the sun in
/// clear air and light mist does, the afterglow does not.
pub const SHAFTS_SPREAD: u16 = (5 * 65536 / 360) as u16;

/// A shadow at least this dark (of 255, [`Directional::strength`]) is laid as a silhouette on T0
/// and T1: the sun and the moon in clear air and light weather; not the afterglow, nor a sun lost
/// in cloud or mist, whose shadows on T2 are faint.
pub const SILHOUETTE_STRENGTH: u8 = 112;
/// The afterglow's shadow: faint.
const GLOW_STRENGTH: u8 = 96;

/// How soft the sun's or the moon's shadows are in clear air at `sin_el` (its elevation's sine,
/// Q15): the light crosses more air the lower it is, and the air scatters it round what casts a
/// shadow. One degree of spread high in the sky (crisp edges at noon), widening to five at the
/// horizon; the same for the moon.
pub fn spread(sin_el: i32) -> u16 {
    let low = 20000 - sin_el.clamp(0, 20000);
    (deg(1) + deg(4) * low / 20000) as u16
}

/// How dark the sun's or the moon's shadows are in clear air at `sin_el`: the whole of its light
/// taken away high in the sky, a quarter less at the horizon, where the scattered light fills
/// the umbra.
pub fn strength(sin_el: i32) -> u8 {
    let low = 20000 - sin_el.clamp(0, 20000);
    (255 - 64 * low / 20000) as u8
}

/// The weather's part (`atmos::Atmos::light`): `cloud` (0..=65535, rain or mist) spreads the
/// sun into the sky, its shadows widening by up to ten degrees and fading to half their strength
/// (its colour, which the weather dims too, takes the rest of the contrast away).
pub fn diffuse(sun: &mut Directional, cloud: u32) {
    let cloud = cloud.min(65535);
    sun.spread = sun.spread.saturating_add((deg(10) as u32 * cloud / 65535) as u16);
    sun.strength = (u32::from(sun.strength) * (65535 - cloud / 2) / 65535) as u8;
}

/// The western sky after sunset on flat ground, and how long it glows: the sun's gold held low
/// in the west, so the warmth stays where the sun went down while the fill turns the rest blue.
/// Yellow more than orange and not bright: flat ground takes it and the fill at once, and an
/// orange one over the blue made every lit face lavender.
const AFTERGLOW: [i32; 3] = [112, 102, 68];
const GLOW: i32 = HOUR * 3 / 4;
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
    /// T0's light past `ambient`, Q8 (256 none): its grade's exposure takes it ([`t0_gain`]).
    pub t0_gain: u16,
}

/// The ambient per channel (255 is full light) at `clock` ticks since midnight; indoors, the
/// zone's `permille` instead, a little blue.
pub fn ambient(clock: u32, indoor: bool, permille: i16) -> [u8; 3] {
    if indoor {
        // Dark is the mood and never black (ART.md §3.1): a zone's light runs from a floor of
        // about a quarter up, so a mine at 160 permille is dim and still reads.
        let v = INDOOR_FLOOR + i32::from(permille.clamp(0, 1000)) * (255 - INDOOR_FLOOR) / 1000;
        return [(v - v / 10) as u8, (v - v / 20) as u8, v as u8];
    }
    // Past 255 the light's hue stays here and its surplus is T0's exposure (`t0_gain`).
    let c = keyed_wide(&KEYS, clock);
    let top = c.iter().copied().max().unwrap_or(0).max(255);
    c.map(|v| (v * 255 / top).clamp(0, 255) as u8)
}

/// How much brighter than [`ambient`] T0's flat light is, Q8 (256 is none): the keys past 255
/// (T2's noon is brighter than the art as drawn), which T0's grade takes as exposure.
pub fn t0_gain(clock: u32, indoor: bool) -> u32 {
    if indoor {
        return 256;
    }
    let top = keyed_wide(&KEYS, clock).iter().copied().max().unwrap_or(0).max(255);
    (top * 256 / 255) as u32
}

/// A keyframe table at `clock`, linear between keys, clamped to a byte.
fn keyed(keys: &[(i32, [i32; 3])], clock: u32) -> [u8; 3] {
    keyed_wide(keys, clock).map(|c| c.clamp(0, 255) as u8)
}

/// A keyframe table at `clock`, linear between keys.
fn keyed_wide(keys: &[(i32, [i32; 3])], clock: u32) -> [i32; 3] {
    let t = (clock % (24 * HOUR as u32)) as i32;
    for w in keys.windows(2) {
        let ((t0, c0), (t1, c1)) = (w[0], w[1]);
        if t >= t0 && t <= t1 {
            let span = (t1 - t0).max(1);
            return [0, 1, 2].map(|k| c0[k] + (c1[k] - c0[k]) * (t - t0) / span);
        }
    }
    keys[0].1
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
        let post = Post { tint: [255, 250, 240], lift: [6, 8, 18], saturation: 136, bloom: 150, exposure: 136 };
        return Sky { ambient: flat, fill, sun: None, shade: [150, 150, 190], post, t0_gain: 256 };
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
            spread: spread(s),
            strength: strength(s),
        })
    } else if (SET..SET + GLOW).contains(&t) {
        // The afterglow: for three quarters of an hour after sunset the western sky is the
        // brightest thing in it, a low amber light from where the sun went down, fading.
        let left = SET + GLOW - t;
        Some(Directional {
            azimuth: Angle((deg(180) + deg(16) / 4) as u16),
            elevation: Angle(deg(10) as u16),
            colour: mix([0; 3], AFTERGLOW, left, GLOW).map(|c| c.clamp(0, 255) as u8),
            // A glow over a broad band of sky: its shadows are faint, and too faint for a
            // silhouette.
            spread: deg(7) as u16,
            strength: GLOW_STRENGTH,
        })
    } else if let Some((az, el, s)) = arc(t, SET, RISE, MOON_TOP) {
        let phase = moon_phase(day);
        let colour = mix([0; 3], MOON.map(|c| (c * phase) >> 8), s, 6000);
        Some(Directional {
            azimuth: az,
            elevation: el,
            colour: colour.map(|c| c.clamp(0, 255) as u8),
            spread: spread(s),
            strength: strength(s),
        })
    } else {
        None
    };
    let shade = shade(fill, sun.map_or([0; 3], |s| s.colour));
    Sky { ambient: flat, fill, sun, shade, post: grade(region, t), t0_gain: t0_gain(clock, false) as u16 }
}

/// A silhouette's multiplier: the fill's share of fill and sun, eased toward none so a shadow
/// darkens by at most about half.
pub fn shade(fill: Rgb, sun: Rgb) -> Rgb {
    [0, 1, 2].map(|k| {
        let (f, s) = (i32::from(fill[k]), i32::from(sun[k]));
        let share = f * 255 / (f + s).max(1);
        (255 - (255 - share) * 7 / 8).clamp(0, 255) as u8
    })
}

/// A region's grade at tick `t` of the day (§1.9): the Lowfields warm and green-gold, the Waters
/// cold blue-green, the Works sodium and soot; dusk and night lift their shadows blue (a lift
/// with more red than green in it is the mauve that made a dusk read violet-rose).
fn grade(region: Region, t: i32) -> Post {
    let (tint, lift, saturation): (Rgb, Rgb, u8) = match region {
        Region::Lowfields => ([255, 250, 238], [3, 4, 12], 136),
        Region::Waters => ([238, 250, 255], [2, 7, 14], 130),
        Region::Works => ([255, 240, 218], [8, 5, 6], 116),
    };
    let dusk = (HOUR * 17..HOUR * 21).contains(&t);
    let night = !(HOUR * 5..HOUR * 21).contains(&t);
    let lift = if night {
        [lift[0] / 2 + 3, lift[1] / 2 + 4, lift[2] + 8]
    } else if dusk {
        [lift[0] / 2, lift[1] / 2 + 2, lift[2] + 6]
    } else {
        lift
    };
    let exposure = if night { 150 } else { 128 };
    // The eye sees less colour by night: the lamps' warmth against the blue would otherwise
    // meet in a saturated mauve where a pool fades.
    let saturation = if night { saturation - 14 } else { saturation };
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
        // T2's noon is brighter than the art as drawn: T0 takes the rest as exposure.
        assert!(t0_gain(12 * 7200, false) > 280 && t0_gain(0, false) == 256);
        let n = ambient(0, false, 1000);
        assert!(n[2] > n[0] && n[0] < 100, "{n:?}");
        assert_eq!(ambient(23 * 7200, false, 1000), n);
        // 17:00 is on the way to sunset; 20:00 is on the way to night.
        let five = ambient(17 * 7200, false, 1000);
        assert!(five[0] == 255 && five[2] < 230, "{five:?}");
        assert!(ambient(20 * 7200, false, 1000)[0] < five[0]);
    }

    #[test]
    fn noon_shadows_are_crisp_and_dark_and_a_low_sun_softer_and_lighter() {
        let at = |h: i32| sky((h * HOUR) as u32, 0, false, 1000, Region::Lowfields).sun.expect("the sun is up");
        let (noon, five, seven) = (at(12), at(17), at(7));
        for low in [five, seven] {
            assert!(noon.spread < low.spread, "noon {} against {}", noon.spread, low.spread);
            assert!(noon.strength > low.strength, "noon {} against {}", noon.strength, low.strength);
            // T0 and T1 feather the low sun's edge wider.
            assert!(crate::shadow::feather(noon.spread) < crate::shadow::feather(low.spread));
        }
        assert!(noon.spread <= deg(1) as u16 + 60 && noon.strength >= 250, "{noon:?}");
        // The moon the same way, by its height.
        let moon = sky(0, 0, false, 1000, Region::Lowfields).sun.expect("the moon is up at midnight");
        let s = sin_q15(moon.elevation).0;
        assert_eq!((moon.spread, moon.strength), (spread(s), strength(s)));
        // Each lays a silhouette; the afterglow does not.
        assert!(noon.silhouettes() && five.silhouettes() && moon.silhouettes());
        let glow = sky((SET + HOUR / 4) as u32, 0, false, 1000, Region::Lowfields).sun.expect("the afterglow");
        assert!(!glow.silhouettes());
    }

    #[test]
    fn cloud_spreads_the_sun_and_fades_its_shadows() {
        let noon = sky(12 * 7200, 0, false, 1000, Region::Lowfields).sun.expect("the sun");
        let (mut rain, mut mist) = (noon, noon);
        diffuse(&mut rain, 65535);
        diffuse(&mut mist, 65535 * 3 / 8);
        assert!(rain.strength < mist.strength && mist.strength < noon.strength, "{rain:?} {mist:?}");
        assert!(rain.spread > mist.spread && mist.spread > noon.spread);
        assert_eq!(crate::shadow::feather(rain.spread), 3);
    }

    #[test]
    fn indoors_is_the_zone_light() {
        assert_eq!(ambient(0, true, 1000), [230, 243, 255]);
        let dim = ambient(12 * 7200, true, 200);
        assert!(dim[2] < 120 && dim[2] > 60, "dim and never black: {dim:?}");
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
