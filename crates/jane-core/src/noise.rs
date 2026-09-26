//! Integer value noise in `Q16` (PORT.md §6.b).
//!
//! **Rule:** noise output is a pure function of `(x, y, salt)`. No state, no float, no table.

use crate::grid::Rect;
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

/// [`fbm`] at every point of box `b`, row by row: the same numbers,
/// worked out a row and a column at a time. Each octave's lattice values are hashed once for the
/// box, not four times a point, and where a point falls between them once a column and once a row.
pub fn fbm_box(b: Rect, period: i32, salt: u32, octaves: u32, gain_shift: u32) -> Vec<Q16> {
    let (x0, y0, w, h) = (b.x, b.y, b.w.max(0) as usize, b.h.max(0) as usize);
    let mut sum = vec![0i64; w * h];
    let mut total: i64 = 0;
    let mut amp: i64 = i64::from(Q16_ONE);
    // Where each column (row) falls on an octave's lattice: the cell, and the smoothed fraction.
    let place = |from: i32, n: usize, p: i32| -> Vec<(i32, Q16)> {
        (0..n as i32)
            .map(|i| {
                let t = Q16::ratio(from + i, p);
                (t.floor(), Q16::smooth(t.frac()))
            })
            .collect()
    };
    for o in 0..octaves {
        let p = (period >> o).max(2);
        let octave_salt = salt.wrapping_add(o.wrapping_mul(7919));
        let (cols, rows) = (place(x0, w, p), place(y0, h, p));
        let (Some(&(lx, _)), Some(&(ly, _))) = (cols.first(), rows.first()) else { return Vec::new() };
        let lw = (cols[w - 1].0 - lx + 2) as usize;
        let lh = (rows[h - 1].0 - ly + 2) as usize;
        let lattice: Vec<Q16> =
            (0..lh * lw).map(|i| lattice(lx + (i % lw) as i32, ly + (i / lw) as i32, octave_salt)).collect();
        for (j, &(cy, ty)) in rows.iter().enumerate() {
            let top = &lattice[(cy - ly) as usize * lw..];
            let bottom = &lattice[(cy - ly + 1) as usize * lw..];
            for (i, &(cx, tx)) in cols.iter().enumerate() {
                let k = (cx - lx) as usize;
                let upper = Q16::lerp(top[k], top[k + 1], tx);
                let lower = Q16::lerp(bottom[k], bottom[k + 1], tx);
                let v = Q16::lerp(upper, lower, ty);
                sum[j * w + i] += (i64::from(v.0) * amp) >> 16;
            }
        }
        total += amp;
        amp >>= gain_shift;
        if amp == 0 {
            break;
        }
    }
    if total == 0 {
        return vec![Q16::ZERO; w * h];
    }
    sum.into_iter().map(|s| Q16(((s << 16) / total) as i32)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fbm_box_is_fbm_at_every_point() {
        let mut rng = crate::rng::Sfc32::seeded(3, 0);
        for _ in 0..60 {
            let (x0, y0) = (rng.below(200) as i32 - 100, rng.below(200) as i32 - 100);
            let (w, h) = (1 + rng.below(40), 1 + rng.below(40));
            let period = rng.below(60) as i32 - 2;
            let (salt, octaves, gain) = (rng.next_u32(), rng.below(6), rng.below(3));
            let got = fbm_box(Rect::new(x0, y0, w as i32, h as i32), period, salt, octaves, gain);
            for y in 0..h as i32 {
                for x in 0..w as i32 {
                    let want = fbm(x0 + x, y0 + y, period, salt, octaves, gain);
                    assert_eq!(got[(y * w as i32 + x) as usize], want, "({x0}+{x}, {y0}+{y}) period {period}");
                }
            }
        }
    }

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
