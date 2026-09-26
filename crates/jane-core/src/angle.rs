//! Angles: the only direction there is (ARCHITECTURE.md §2). 65 536 per turn, 0 = east,
//! clockwise on screen (y grows down). There is no `normalize`.

use crate::num::{Fx, Q15, Vec2};
use crate::trig_table::{ATAN, SIN_QUARTER};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Angle(pub u16);

impl Angle {
    pub const EAST: Angle = Angle(0);
    pub const SOUTH: Angle = Angle(16384);
    pub const WEST: Angle = Angle(32768);
    pub const NORTH: Angle = Angle(49152);

    /// Whole degrees, rounded to the nearest unit. Negative and past-a-turn values wrap.
    pub const fn from_degrees(deg: i32) -> Angle {
        let units = crate::num::div_round(deg as i64 * 65536, 360);
        Angle(units.rem_euclid(65536) as u16)
    }

    /// Tenths of a degree, for data that says `4.8`.
    pub const fn from_decidegrees(dd: i32) -> Angle {
        let units = crate::num::div_round(dd as i64 * 65536, 3600);
        Angle(units.rem_euclid(65536) as u16)
    }

    pub const fn wrapping_add(self, d: i32) -> Angle {
        Angle((self.0 as i32 + d).rem_euclid(65536) as u16)
    }

    /// The signed shortest turn from `self` to `to`, in `-32768..=32767`. Computed in `i32`.
    pub const fn diff(self, to: Angle) -> i32 {
        let d = to.0 as i32 - self.0 as i32;
        if d >= 32768 {
            d - 65536
        } else if d < -32768 {
            d + 65536
        } else {
            d
        }
    }

    /// `|diff|`.
    pub const fn gap(self, to: Angle) -> u32 {
        self.diff(to).unsigned_abs()
    }
}

/// sin, Q15, from the 4096-entry table (`a >> 4`).
pub fn sin_q15(a: Angle) -> Q15 {
    let i = usize::from(a.0 >> 4);
    let j = i & 1023;
    let v = match i >> 10 {
        0 => i32::from(SIN_QUARTER[j]),
        1 => i32::from(SIN_QUARTER[1024 - j]),
        2 => -i32::from(SIN_QUARTER[j]),
        _ => -i32::from(SIN_QUARTER[1024 - j]),
    };
    Q15(v)
}

/// cos, Q15.
pub fn cos_q15(a: Angle) -> Q15 {
    sin_q15(Angle(a.0.wrapping_add(16384)))
}

/// The bearing from the origin to `(dx, dy)`: octant reduction and a 257-entry table with a
/// linear step between entries. `(0, 0)` is east.
pub fn iatan2(dy: i32, dx: i32) -> Angle {
    if dx == 0 && dy == 0 {
        return Angle::EAST;
    }
    let ax = i64::from(dx).unsigned_abs();
    let ay = i64::from(dy).unsigned_abs();
    let (lo, hi, steep) = if ax >= ay { (ay, ax, false) } else { (ax, ay, true) };
    // t = lo / hi in 0..=1 as 16 bits of fraction.
    let t = ((lo << 16) / hi) as u32;
    let idx = (t >> 8) as usize;
    let frac = t & 0xff;
    let a0 = u32::from(ATAN[idx]);
    let a1 = u32::from(ATAN[(idx + 1).min(256)]);
    let oct = a0 + (((a1 - a0) * frac + 128) >> 8);
    let base = if steep { 16384 - oct } else { oct } as i32;
    let full = match (dx >= 0, dy >= 0) {
        (true, true) => base,
        (false, true) => 32768 - base,
        (false, false) => 32768 + base,
        (true, false) => 65536 - base,
    };
    Angle(full.rem_euclid(65536) as u16)
}

/// The step of length `s` along `a`.
pub fn along(a: Angle, s: Fx) -> Vec2 {
    let c = cos_q15(a).0;
    let si = sin_q15(a).0;
    Vec2 {
        x: Fx(((i64::from(s.0) * i64::from(c)) >> 15) as i32),
        y: Fx(((i64::from(s.0) * i64::from(si)) >> 15) as i32),
    }
}

/// The bearing from `a` to `b`.
pub fn bearing(a: Vec2, b: Vec2) -> Angle {
    iatan2(b.y.0 - a.y.0, b.x.0 - a.x.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_points() {
        assert_eq!(sin_q15(Angle::EAST).0, 0);
        assert_eq!(cos_q15(Angle::EAST).0, 32768);
        assert_eq!(sin_q15(Angle::SOUTH).0, 32768);
        assert_eq!(cos_q15(Angle::WEST).0, -32768);
        assert_eq!(sin_q15(Angle::NORTH).0, -32768);
        assert_eq!(iatan2(0, 1), Angle::EAST);
        assert_eq!(iatan2(1, 0), Angle::SOUTH);
        assert_eq!(iatan2(0, -1), Angle::WEST);
        assert_eq!(iatan2(-1, 0), Angle::NORTH);
        assert_eq!(iatan2(5, 5), Angle(8192));
        assert_eq!(iatan2(-5, -5), Angle(8192 + 32768));
    }

    #[test]
    fn sin_squared_plus_cos_squared_is_one() {
        for a in (0..=65535u32).step_by(37) {
            let a = Angle(a as u16);
            let s = i64::from(sin_q15(a).0);
            let c = i64::from(cos_q15(a).0);
            let one = 1i64 << 30;
            assert!((s * s + c * c - one).abs() < one / 500, "{a:?}");
        }
    }

    #[test]
    fn atan_inverts_along() {
        // Round trip within one table step (16 units, 0.09 degrees): the table is 4096 entries.
        for a in (0..=65535u32).step_by(101) {
            let a = Angle(a as u16);
            let v = along(a, Fx(1 << 20));
            let back = iatan2(v.y.0, v.x.0);
            assert!(a.gap(back) <= 17, "{a:?} -> {back:?}");
        }
    }

    #[test]
    fn diff_is_the_short_way_round() {
        assert_eq!(Angle(0).diff(Angle(100)), 100);
        assert_eq!(Angle(100).diff(Angle(0)), -100);
        assert_eq!(Angle(65500).diff(Angle(36)), 72);
        assert_eq!(Angle(36).diff(Angle(65500)), -72);
        assert_eq!(Angle(0).diff(Angle(32768)), -32768);
        assert_eq!(Angle(0).gap(Angle(32768)), 32768);
    }

    #[test]
    fn degrees() {
        assert_eq!(Angle::from_degrees(90), Angle::SOUTH);
        assert_eq!(Angle::from_degrees(-90), Angle::NORTH);
        assert_eq!(Angle::from_degrees(360), Angle::EAST);
        assert_eq!(Angle::from_degrees(20).0, 3641);
        assert_eq!(Angle::from_decidegrees(48).0, 874);
    }

    #[test]
    fn iatan2_takes_extremes() {
        let _ = iatan2(i32::MIN, i32::MIN);
        let _ = iatan2(i32::MAX, i32::MIN);
        assert_eq!(iatan2(0, i32::MIN), Angle::WEST);
    }
}
