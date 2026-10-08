//! What C2's water and wet ground are drawn from (PRESENTATION.md §1.8, T1's terms): a sine in
//! integers for the ripple and the shimmer's crest rows, and the puddles' noise tile. Pure and
//! integer, so a PC test reads what the PSP draws.

use alloc::vec::Vec;

/// A full turn in [`isin`]'s units.
pub const TURN: i32 = 65_536;

/// `sin(a)` of 4096, `a` in [`TURN`]s of 65536 (any `i32`: a turn's multiple wraps), by a
/// table of 1024 steps (Bhaskara's rational form, within 0.2 %): no division on the PSP.
#[inline]
pub fn isin(a: i32) -> i32 {
    i32::from(SINE[((a as u32) >> 6) as usize & 1023])
}

/// Radians as [`TURN`]s, per px or per tick: `r * 65536 / (2 pi)`, `r` in thousandths.
pub const fn turns(milli_rad: i32) -> i32 {
    ((milli_rad as i64) * 10_430 / 1000) as i32
}

static SINE: [i16; 1024] = sine();

const fn sine() -> [i16; 1024] {
    let mut t = [0i16; 1024];
    let mut i = 0;
    while i < 1024 {
        let a = (i as i64) * 64 + 32;
        let half = (TURN / 2) as i64;
        let (x, sign) = if a < half { (a, 1) } else { (a - half, -1) };
        let p = x * (half - x);
        t[i] = (sign * 16 * p * 4096 / (5 * half * half - 4 * p)) as i16;
        i += 1;
    }
    t
}

/// The puddles' noise tile's side.
pub const NOISE: usize = 256;

/// T1's puddle noise (`shaders.rs` `COMPOSE_FS`: a value noise stretched along the rows, 0.7 of
/// it, and a finer one, 0.3), seamless over [`NOISE`] px, with the 4 x 4 ordered dither of its rim
/// baked in (a px's threshold moved by its Bayer step over the rim's width): a frame's CLUT
/// keeps what is over the rain's edge. Each texel 0..=255.
pub fn noise_tile() -> Vec<u8> {
    let hash = |x: i32, y: i32, s: u32| -> u32 {
        let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ s;
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        h & 0xff
    };
    // A lattice `(sx, sy)` px apart, seamless over the tile: a smoothstep blend of its corners.
    // `(cx, cy)` lattice cells across the tile (T1's 22 px across and 1.6 times finer down).
    let value = |x: i32, y: i32, cx: i32, cy: i32, s: u32| -> u32 {
        let (px, py) = (x * cx * 256 / NOISE as i32, y * cy * 256 / NOISE as i32);
        let (bx, by) = (px >> 8, py >> 8);
        let (fx, fy) = (px & 255, py & 255);
        let ease = |f: i32| (f * f * (768 - 2 * f) / 65_536) as u32;
        let (ex, ey) = (ease(fx), ease(fy));
        let at = |i: i32, j: i32| hash(i.rem_euclid(cx), j.rem_euclid(cy), s);
        let top = at(bx, by) * (256 - ex) + at(bx + 1, by) * ex;
        let bot = at(bx, by + 1) * (256 - ex) + at(bx + 1, by + 1) * ex;
        (top * (256 - ey) + bot * ey) >> 16
    };
    let bayer = |x: i32, y: i32| -> i32 {
        const B: [i32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
        B[((y & 3) * 4 + (x & 3)) as usize]
    };
    let mut out = alloc::vec![0u8; NOISE * NOISE];
    for y in 0..NOISE as i32 {
        for x in 0..NOISE as i32 {
            let n = value(x, y, 12, 19, 0x5eed) * 7 / 10 + value(x, y, 37, 37, 0xf1e5) * 3 / 10;
            // The rim: about 0.07 of the range wide, a px's step of it by the dither.
            let d = (bayer(x, y) - 8) * 18 / 16;
            out[y as usize * NOISE + x as usize] = (n as i32 + d).clamp(0, 255) as u8;
        }
    }
    out
}

/// The puddles' edge on the noise (T1's): `0.72 - (wet - 0.35) * 0.3` of 255, `wet` of 255;
/// none under 0.35.
pub fn puddle_edge(wet: u8) -> Option<u32> {
    let w = u32::from(wet);
    (w * 100 >= 35 * 255).then(|| (184 - (w - 89) * 3 / 10).clamp(100, 255))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_is_close_and_the_tile_is_seamless_and_spread() {
        assert!(isin(0).abs() <= 13);
        assert!((isin(TURN / 4) - 4096).abs() <= 8);
        assert!((isin(TURN * 3 / 4) + 4096).abs() <= 8);
        assert!((isin(TURN / 12) - 2048).abs() <= 16, "{}", isin(TURN / 12));
        assert_eq!(isin(TURN / 4), isin(TURN / 4 + 7 * TURN));
        let t = noise_tile();
        let (lo, hi) = (t.iter().filter(|&&v| v < 96).count(), t.iter().filter(|&&v| v > 160).count());
        assert!(lo > 1000 && hi > 1000, "{lo} {hi}");
        // The last column runs on into the first.
        let step =
            (0..NOISE).map(|y| i32::from(t[y * NOISE]) - i32::from(t[y * NOISE + NOISE - 1])).map(i32::abs).max();
        assert!(step.unwrap() < 40);
        assert!(puddle_edge(60).is_none() && puddle_edge(255).is_some());
    }
}
