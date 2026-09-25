//! Integer value noise in `Q16` (PORT.md §6.b).
//!
//! **Rule:** noise output is a pure function of `(x, y, salt)`. No state, no float, no table.

use crate::hash::hash2;
use crate::num::{Q16, Q16_ONE};

/// A lattice value in `0..1` at integer point `(ix, iy)`: the top 16 bits of `hash2`.
pub const fn lattice(ix: i32, iy: i32, salt: u32) -> Q16 {
    Q16((hash2(ix, iy, salt) >> 16) as i32)
}

/// Smoothed value noise at `(x, y)`, both `Q16` in lattice units. Output in `0..1`.
pub fn value(x: Q16, y: Q16, salt: u32) -> Q16 {
    let x0 = x.floor();
    let y0 = y.floor();
    let tx = Q16::smooth(x.frac());
    let ty = Q16::smooth(y.frac());
    let a = lattice(x0, y0, salt);
    let b = lattice(x0 + 1, y0, salt);
    let c = lattice(x0, y0 + 1, salt);
    let d = lattice(x0 + 1, y0 + 1, salt);
    let top = Q16::lerp(a, b, tx);
    let bottom = Q16::lerp(c, d, tx);
    Q16::lerp(top, bottom, ty)
}

/// Value noise at integer point `(x, y)` with a lattice every `period` units. Output in `0..1`.
pub fn value_at(x: i32, y: i32, period: i32, salt: u32) -> Q16 {
    let p = period.max(1);
    value(Q16::ratio(x, p), Q16::ratio(y, p), salt)
}

/// Fractal sum of `octaves` of [`value_at`]: each octave halves the period (never below 2) and
/// scales its weight by `>> gain_shift`; the sum is normalised back to `0..1`.
pub fn fbm(x: i32, y: i32, period: i32, salt: u32, octaves: u32, gain_shift: u32) -> Q16 {
    let mut sum: i64 = 0;
    let mut total: i64 = 0;
    let mut amp: i64 = i64::from(Q16_ONE);
    for o in 0..octaves {
        let p = (period >> o).max(2);
        let v = value_at(x, y, p, salt.wrapping_add(o.wrapping_mul(7919)));
        sum += (i64::from(v.0) * amp) >> 16;
        total += amp;
        amp >>= gain_shift;
        if amp == 0 {
            break;
        }
    }
    if total == 0 {
        return Q16::ZERO;
    }
    Q16(((sum << 16) / total) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_is_in_range_and_pure() {
        for y in -20..20 {
            for x in -20..20 {
                let v = value_at(x * 3, y * 5, 7, 99);
                assert!((0..Q16_ONE).contains(&v.0));
                assert_eq!(v, value_at(x * 3, y * 5, 7, 99));
            }
        }
    }

    #[test]
    fn value_hits_the_lattice_at_integer_points() {
        assert_eq!(value(Q16::from_int(3), Q16::from_int(-4), 5), lattice(3, -4, 5));
    }

    #[test]
    fn value_is_continuous() {
        // Neighbouring samples one unit apart at period 16 never jump by more than a sixteenth
        // of the range times the steepest smoothstep slope (1.5).
        let max_step = Q16_ONE * 3 / 2 / 16 + 2;
        for y in 0..40 {
            for x in 0..200 {
                let a = value_at(x, y, 16, 1).0;
                let b = value_at(x + 1, y, 16, 1).0;
                let c = value_at(x, y + 1, 16, 1).0;
                assert!((a - b).abs() <= max_step, "x step at {x},{y}: {a} {b}");
                assert!((a - c).abs() <= max_step, "y step at {x},{y}: {a} {c}");
            }
        }
    }

    #[test]
    fn fbm_is_in_range_and_uses_its_range() {
        let mut lo = Q16_ONE;
        let mut hi = 0;
        for y in 0..64 {
            for x in 0..64 {
                let v = fbm(x * 4, y * 4, 32, 7, 3, 1).0;
                assert!((0..Q16_ONE).contains(&v));
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        assert!(hi - lo > Q16_ONE / 3, "fbm spread {lo}..{hi}");
    }

    #[test]
    fn salts_differ() {
        assert_ne!(value_at(10, 10, 7, 1), value_at(10, 10, 7, 2));
    }
}
