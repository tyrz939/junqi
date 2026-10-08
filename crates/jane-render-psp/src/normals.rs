//! C2's normal lighting (owner, 2026-10-08: the PSP keeps the lighting; PORT.md §13.5, §13.12).
//! The bake quantises each sprite's normal layer to sixteen directions and ships it as a `T4`
//! page beside the albedo page, the same rects; each frame the CPU fills a sixteen-entry CLUT
//! with each direction's response to the sun, and the GE lays that page over the sprite with a
//! doubled multiply, so a face turned to the sun keeps its light and one turned away takes the
//! shade. Index 0 is flat (or clear): no change, drawn clear. Integer only.

use jane_core::Angle;
use jane_core::angle::{cos_q15, iatan2, sin_q15};
use jane_present::{Directional, Rgb};

/// Directions in a normal page: 0 flat, then 15 sectors round the compass, tilted.
pub const DIRS: usize = 16;
/// Sectors round the compass.
const SECTORS: u32 = 15;
/// A normal tipped less than this from up (`[u8; 2]` units, 128 is 1) is flat.
const FLAT: i32 = 26;
/// A sector's normal's tilt: across the ground and up, Q8 (about 33 degrees).
const ACROSS: i32 = 141;
const UP: i32 = 214;

/// The direction index of a tangent-space normal (`[nx, ny]`, 128 is 0, `+x` east, `+y` south).
pub fn quantize(n: [u8; 2]) -> u8 {
    let (x, y) = (i32::from(n[0]) - 128, i32::from(n[1]) - 128);
    if x * x + y * y < FLAT * FLAT {
        return 0;
    }
    let a = u32::from(iatan2(y, x).0);
    (1 + ((a * SECTORS + 32768) >> 16) % SECTORS) as u8
}

/// Direction `i`'s normal, Q8: `(east, south, up)`.
pub fn dir(i: u8) -> (i32, i32, i32) {
    if i == 0 || usize::from(i) >= DIRS {
        return (0, 0, 256);
    }
    let a = Angle(((u32::from(i) - 1) * 65536 / SECTORS) as u16);
    ((cos_q15(a).0 * ACROSS) >> 15, (sin_q15(a).0 * ACROSS) >> 15, UP)
}

/// The sixteen-entry CLUT (`0xAABBGGRR`) for the sun `sun` with shadowed ground taking `shade`
/// (the silhouettes' tint: the light a surface turned from the sun keeps): each direction's light
/// over flat ground's, halved for the GE's doubled multiply. `None` with the sun down.
pub fn clut(sun: &Directional, shade: Rgb) -> Option<[u32; DIRS]> {
    let (se, ce) = (sin_q15(sun.elevation).0, cos_q15(sun.elevation).0);
    if se <= 0 {
        return None;
    }
    let (ca, sa) = (cos_q15(sun.azimuth).0, sin_q15(sun.azimuth).0);
    // Toward the sun, Q15.
    let l = ((ce * ca) >> 15, (ce * sa) >> 15, se);
    let mut out = [0u32; DIRS];
    for (i, o) in out.iter_mut().enumerate().skip(1) {
        let (x, y, z) = dir(i as u8);
        // n . l over flat's (se), Q8, from 0 (turned away) up.
        let dot = ((x * l.0 + y * l.1 + z * l.2) >> 8).max(0);
        let lit = (dot * 256 / se).min(512);
        let ch = |s: u8| {
            let s = i32::from(s);
            // shade + (1 - shade) * lit, of 255; halved into the texel.
            let f = s * 256 + (255 - s) * lit;
            (f / 512).clamp(0, 255) as u32
        };
        *o = 0xff00_0000 | ch(shade[2]) << 16 | ch(shade[1]) << 8 | ch(shade[0]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_quantises_to_its_sector_and_flat_to_none() {
        assert_eq!(quantize([128, 128]), 0);
        assert_eq!(quantize([140, 130]), 0);
        // East is sector 0, index 1; south a quarter round (15 sectors: 3.75, rounds to 4).
        assert_eq!(quantize([255, 128]), 1);
        assert_eq!(quantize([128, 255]), 5);
        let (x, y, z) = dir(1);
        assert!(x > 100 && y == 0 && z > 200);
    }

    #[test]
    fn a_face_to_the_sun_keeps_its_light_and_one_away_takes_the_shade() {
        // The sun low in the east.
        let sun = Directional { azimuth: Angle(0), elevation: Angle(4096), colour: [255; 3], spread: 0, strength: 255 };
        let c = clut(&sun, [100, 100, 140]).unwrap();
        let r = |e: u32| e & 0xff;
        // Facing east: brighter than flat (over 127); facing west (sector 7 or 8): the shade's half.
        assert!(r(c[1]) > 127, "{:08x}", c[1]);
        assert!(r(c[8]) <= 100 / 2 + 1, "{:08x}", c[8]);
        let night = Directional { elevation: Angle(0), ..sun };
        assert!(clut(&night, [100; 3]).is_none());
    }
}
