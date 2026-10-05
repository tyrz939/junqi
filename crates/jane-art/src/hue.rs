//! Hue-shifted ramps (the owner's craft rule, 2026-09-27): a ramp is not one hue walked from
//! dark to light. Its shadows step toward a cool hue (violet-blue; rose-violet for skin), its
//! lights toward a warm one (yellow; peach for skin), and its saturation peaks in the midtones
//! and falls at both ends. The people's ramps and the terrain's and flora's are built this
//! way; the rest (the chrome, the step-1 odds and ends) keep their straight mix.
//!
//! All integer: hue in tenths of a degree (0..3600), saturation and lightness in thousandths.

use crate::palette::Ramp;

/// The cool hue a ramp's shadows lean to, if the ramp is hue-shifted.
pub const fn shadow_hue(r: Ramp) -> Option<i32> {
    match r {
        Ramp::Skin | Ramp::SkinPale | Ramp::SkinDark => Some(SKIN_COOL),
        Ramp::HairDark
        | Ramp::HairFair
        | Ramp::HairBrown
        | Ramp::HairGrey
        | Ramp::HairWhite
        | Ramp::HairRed
        | Ramp::HairBlack
        | Ramp::ClothPlum
        | Ramp::ClothMustard
        | Ramp::ClothBrown
        | Ramp::ClothGrey
        | Ramp::ClothRed
        | Ramp::ClothBlue
        | Ramp::ClothGreen
        | Ramp::ClothTeal
        | Ramp::ClothMoss
        | Ramp::ClothOchre
        | Ramp::ClothNavy
        | Ramp::ClothBlack
        | Ramp::ClothTweed
        | Ramp::ClothRose
        | Ramp::ClothCream
        | Ramp::ClothSky
        | Ramp::ClothBrick
        | Ramp::ClothLinen
        | Ramp::Leather
        | Ramp::Pool
        // The terrain and flora (ART.md §2.6): the ground, its walls and roofs, what grows on it.
        // `stone` keeps the straight mix (violet shade, cream light): the lit-sphere test's ramp,
        // whose crown the wider HSL steps would light brighter than a low side light.
        | Ramp::Grass
        | Ramp::Leaf
        | Ramp::Turf
        | Ramp::TurfDry
        | Ramp::TurfSlag
        | Ramp::Marsh
        | Ramp::Hedge
        | Ramp::LeafOlive
        | Ramp::LeafDeep
        | Ramp::LeafBeech
        | Ramp::LeafOak
        | Ramp::LeafMaple
        | Ramp::LeafYew
        | Ramp::PlasterPink
        | Ramp::PlasterOchre
        | Ramp::Limewash
        | Ramp::Flint
        | Ramp::RoofTileNew
        | Ramp::Oxblood
        | Ramp::RacingGreen
        | Ramp::Wisteria
        | Ramp::Needle
        | Ramp::Shrub
        | Ramp::Crop
        | Ramp::Earth
        | Ramp::Gravel
        | Ramp::Sand
        | Ramp::Mud
        | Ramp::Soil
        | Ramp::Ballast
        | Ramp::Cave
        | Ramp::Reed
        | Ramp::Thatch
        | Ramp::Slate
        | Ramp::Setts
        | Ramp::Rock
        | Ramp::RockFace
        | Ramp::FloorStone
        | Ramp::WallDark
        | Ramp::CaveWall
        | Ramp::Temple
        | Ramp::TempleWall
        | Ramp::Museum
        | Ramp::MuseumWall
        | Ramp::Pipe
        | Ramp::PipeWall
        | Ramp::Works
        | Ramp::WorksWall
        | Ramp::SchoolWall
        | Ramp::Deadwood
        | Ramp::Water
        | Ramp::Ice
        | Ramp::Brick
        | Ramp::RoofTile
        | Ramp::Bark
        | Ramp::WoodOak
        | Ramp::Plaster
        // The kit's and the creatures' own materials (ART.md §8 steps 4 and 5): woods, metals,
        // glass and bone.
        | Ramp::WoodDark
        | Ramp::WoodPale
        | Ramp::Iron
        | Ramp::Brass
        | Ramp::Copper
        | Ramp::Glass
        | Ramp::Bone => Some(COOL),
        _ => None,
    }
}

/// The warm hue a ramp's lights lean to.
pub const fn light_hue(r: Ramp) -> i32 {
    match r {
        Ramp::Skin | Ramp::SkinPale | Ramp::SkinDark => SKIN_WARM,
        _ => WARM,
    }
}

/// Violet-blue.
pub const COOL: i32 = 2500;
/// Warm yellow.
pub const WARM: i32 = 500;
/// Rose-violet: skin in shadow is never grey-brown.
pub const SKIN_COOL: i32 = 3350;
/// Peach.
pub const SKIN_WARM: i32 = 320;

/// Per tone, darkest first: lightness as thousandths of the way to black (negative) or white.
const LIGHT: [i32; 8] = [-640, -420, -210, 0, 150, 320, 510, 700];
/// Per tone: how far the hue turns toward the cool (shadows) or warm (lights) hue, tenths of a
/// degree, never past it.
const TURN: [i32; 8] = [320, 210, 100, 0, 60, 130, 210, 280];
/// Per tone: saturation, thousandths of the key's. Peaks in the midtones.
const SAT: [i32; 8] = [720, 900, 1060, 1000, 1030, 930, 780, 600];
/// Skin's lightness steps: shallower, so a face in shade is still a face.
const SKIN_LIGHT: [i32; 8] = [-520, -330, -160, 0, 130, 290, 460, 650];
/// Skin's saturation: a light key reads loud in HSL, so skin runs quieter than cloth, peaking
/// just above its base.
const SKIN_SAT: [i32; 8] = [420, 560, 720, 820, 840, 780, 680, 560];
/// Skin's hue turn: its shade leans rose without reaching red.
const SKIN_TURN: [i32; 8] = [200, 150, 80, 0, 50, 110, 170, 220];
/// A key this unsaturated is a grey: its tones take a tint of their own instead.
const GREY: i32 = 90;
/// A grey's tint per tone, thousandths of saturation: cool shadows, warm lights.
const GREY_TINT: [i32; 8] = [110, 80, 50, 0, 25, 40, 45, 35];

const fn max3(a: i32, b: i32, c: i32) -> i32 {
    let m = if a > b { a } else { b };
    if m > c { m } else { c }
}

const fn min3(a: i32, b: i32, c: i32) -> i32 {
    let m = if a < b { a } else { b };
    if m < c { m } else { c }
}

/// RGB to hue (0..3600), saturation and lightness (0..=1000).
pub const fn hsl(c: [u8; 3]) -> (i32, i32, i32) {
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    let (mx, mn) = (max3(r, g, b), min3(r, g, b));
    let l = (mx + mn) * 1000 / 510;
    let d = mx - mn;
    if d == 0 {
        return (0, 0, l);
    }
    let s = if mx + mn <= 255 { d * 1000 / (mx + mn) } else { d * 1000 / (510 - mx - mn) };
    let h = if mx == r {
        (600 * (g - b) / d + 3600) % 3600
    } else if mx == g {
        600 * (b - r) / d + 1200
    } else {
        600 * (r - g) / d + 2400
    };
    (h, s, l)
}

/// Hue, saturation and lightness back to RGB.
pub const fn rgb(h: i32, s: i32, l: i32) -> [u8; 3] {
    let h = h.rem_euclid(3600);
    let s = if s < 0 {
        0
    } else if s > 1000 {
        1000
    } else {
        s
    };
    let l = if l < 0 {
        0
    } else if l > 1000 {
        1000
    } else {
        l
    };
    let two_l = 2 * l - 1000;
    let c = (1000 - if two_l < 0 { -two_l } else { two_l }) * s / 1000;
    let f = h % 1200 - 600;
    let x = c * (600 - if f < 0 { -f } else { f }) / 600;
    let m = l - c / 2;
    let (r, g, b) = match h / 600 {
        0 => (c, x, 0),
        1 => (x, c, 0),
        2 => (0, c, x),
        3 => (0, x, c),
        4 => (x, 0, c),
        _ => (c, 0, x),
    };
    [channel(r, m), channel(g, m), channel(b, m)]
}

const fn channel(v: i32, m: i32) -> u8 {
    let v = ((v + m) * 255 + 500) / 1000;
    (if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v
    }) as u8
}

/// `from` turned toward `to` by at most `by` tenths of a degree, the short way round.
const fn turn(from: i32, to: i32, by: i32) -> i32 {
    let mut d = (to - from) % 3600;
    if d > 1800 {
        d -= 3600;
    }
    if d < -1800 {
        d += 3600;
    }
    let step = if d > by {
        by
    } else if d < -by {
        -by
    } else {
        d
    };
    from + step
}

/// Tone `k` (0 darkest) of the hue-shifted ramp on `key`, shadows toward `cool`, lights toward
/// `warm`.
pub const fn tone(key: [u8; 3], k: usize, cool: i32, warm: i32) -> [u8; 3] {
    let (h, s, l) = hsl(key);
    let skin = cool == SKIN_COOL;
    let dl = if skin { SKIN_LIGHT[k] } else { LIGHT[k] };
    let l = if dl < 0 { l * (1000 + dl) / 1000 } else { l + (1000 - l) * dl / 1000 };
    let dark = k < 3;
    let target = if dark { cool } else { warm };
    let (h, s) = if s < GREY {
        (target, s + GREY_TINT[k])
    } else {
        (turn(h, target, if skin { SKIN_TURN[k] } else { TURN[k] }), s * if skin { SKIN_SAT[k] } else { SAT[k] } / 1000)
    };
    let s = if s > 1000 { 1000 } else { s };
    rgb(h, s, l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_round_trips_within_a_step() {
        for c in [[200u8, 40, 60], [30, 90, 200], [128, 128, 128], [250, 240, 200], [20, 10, 30]] {
            let (h, s, l) = hsl(c);
            let back = rgb(h, s, l);
            for k in 0..3 {
                assert!(c[k].abs_diff(back[k]) <= 2, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn turning_goes_the_short_way_and_stops_at_the_target() {
        assert_eq!(turn(100, 3500, 300), -100);
        assert_eq!(turn(3500, 100, 50), 3550);
        assert_eq!(turn(2400, 2500, 300), 2500);
    }
}
