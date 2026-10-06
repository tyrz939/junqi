//! The numeric types (ARCHITECTURE.md §2). This module is authoritative for them.
//!
//! **Rule:** division is floor. `mul_div_round` only where the TypeScript rounded and the call
//! site says so. Every intermediate that can leave `i32` is `i64`.

use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// Fixed-point position and distance: 1/256 px. A cell is 8 px, so `CELL_FX = 2048`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fx(pub i32);

pub const FX_ONE: i32 = 256;
/// Cells are 8 px everywhere: path, collision, occupancy.
pub const CELL_PX: i32 = 8;
pub const CELL_SHIFT: u32 = 11;
pub const CELL_FX: i32 = CELL_PX * FX_ONE;
/// Combat talks in metres, path in pixels; one metre is one cell (2020's rule).
pub const METRE_FX: i32 = CELL_FX;

impl Fx {
    pub const ZERO: Fx = Fx(0);

    pub const fn from_px(px: i32) -> Fx {
        Fx(px * FX_ONE)
    }

    pub const fn from_cells(cells: i32) -> Fx {
        Fx(cells * CELL_FX)
    }

    pub const fn from_metres(m: i32) -> Fx {
        Fx(m * METRE_FX)
    }

    /// The cell a coordinate is in (floor).
    pub const fn cell(self) -> i32 {
        self.0 >> CELL_SHIFT
    }

    /// The centre of a cell.
    pub const fn centre_of(cell: i32) -> Fx {
        Fx((cell << CELL_SHIFT) + CELL_FX / 2)
    }

    /// Whole pixels, floored.
    pub const fn px(self) -> i32 {
        self.0 >> 8
    }
}

impl Add for Fx {
    type Output = Fx;
    fn add(self, o: Fx) -> Fx {
        Fx(self.0 + o.0)
    }
}
impl Sub for Fx {
    type Output = Fx;
    fn sub(self, o: Fx) -> Fx {
        Fx(self.0 - o.0)
    }
}
impl Neg for Fx {
    type Output = Fx;
    fn neg(self) -> Fx {
        Fx(-self.0)
    }
}
impl AddAssign for Fx {
    fn add_assign(&mut self, o: Fx) {
        self.0 += o.0;
    }
}
impl SubAssign for Fx {
    fn sub_assign(&mut self, o: Fx) {
        self.0 -= o.0;
    }
}

/// A point in a zone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec2 {
    pub x: Fx,
    pub y: Fx,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: Fx(0), y: Fx(0) };

    pub const fn new(x: Fx, y: Fx) -> Self {
        Self { x, y }
    }

    /// The centre of a cell.
    pub const fn centre(cx: i32, cy: i32) -> Self {
        Self { x: Fx::centre_of(cx), y: Fx::centre_of(cy) }
    }

    pub const fn cell(self) -> (i32, i32) {
        (self.x.cell(), self.y.cell())
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x + o.x, y: self.y + o.y }
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2 { x: self.x - o.x, y: self.y - o.y }
    }
}

/// Squared distance, never a square root to compare.
pub const fn dist_sq(a: Vec2, b: Vec2) -> i64 {
    let dx = a.x.0 as i64 - b.x.0 as i64;
    let dy = a.y.0 as i64 - b.y.0 as i64;
    dx * dx + dy * dy
}

/// `|a - b| <= r` without a root.
pub const fn within(a: Vec2, b: Vec2, r: Fx) -> bool {
    dist_sq(a, b) <= r.0 as i64 * r.0 as i64
}

/// Thousandths of a point: hp, mp, energy, hits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Milli(pub i32);

impl Milli {
    pub const ZERO: Milli = Milli(0);
    pub const fn from_points(p: i32) -> Milli {
        Milli(p * 1000)
    }
    /// Whole points, floored.
    pub const fn points(self) -> i32 {
        self.0.div_euclid(1000)
    }
}

impl Add for Milli {
    type Output = Milli;
    fn add(self, o: Milli) -> Milli {
        Milli(self.0 + o.0)
    }
}
impl Sub for Milli {
    type Output = Milli;
    fn sub(self, o: Milli) -> Milli {
        Milli(self.0 - o.0)
    }
}
impl AddAssign for Milli {
    fn add_assign(&mut self, o: Milli) {
        self.0 += o.0;
    }
}
impl SubAssign for Milli {
    fn sub_assign(&mut self, o: Milli) {
        self.0 -= o.0;
    }
}

/// Thousandths of one: chances, resists, effect speed, lifesteal, fractions of health.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Permille(pub i16);

impl Permille {
    pub const ZERO: Permille = Permille(0);
    pub const ONE: Permille = Permille(1000);
}

/// `a * p / 1000`, floored.
pub const fn scale(a: i32, p: Permille) -> i32 {
    mul_div_floor(a, p.0 as i32, 1000)
}

/// A count of 60 Hz ticks: 2.2 years before it wraps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tick(pub u32);

pub const TICK_RATE: u32 = 60;

/// The day clock: one game hour is one real minute (a 24-minute day; WORLD.md §2.1). Kept here,
/// under the sim's tuning, so the art and the presentation read the same hour as the sim.
pub const TICKS_PER_HOUR: u32 = 60 * TICK_RATE;
/// A game minute of the clock: one real second.
pub const TICKS_PER_MINUTE: u32 = TICKS_PER_HOUR / 60;

impl Tick {
    pub const ZERO: Tick = Tick(0);
    pub const fn from_secs(s: u32) -> Tick {
        Tick(s * TICK_RATE)
    }
    /// `self + d`, saturating: a timer set past the end of time never fires rather than firing now.
    pub const fn after(self, d: Tick) -> Tick {
        Tick(self.0.saturating_add(d.0))
    }
    pub const fn since(self, earlier: Tick) -> Tick {
        Tick(self.0.saturating_sub(earlier.0))
    }
}

/// Q15: cos and sin table entries, 32768 = 1.0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Q15(pub i32);

pub const Q15_ONE: i32 = 1 << 15;

/// 16.16 fixed point for worldgen noise, terrain fields and county lattices. Never in the sim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Q16(pub i32);

pub const Q16_ONE: i32 = 1 << 16;

impl Q16 {
    pub const ZERO: Q16 = Q16(0);
    pub const ONE: Q16 = Q16(Q16_ONE);
    pub const HALF: Q16 = Q16(Q16_ONE / 2);

    pub const fn from_int(v: i32) -> Q16 {
        Q16(v << 16)
    }

    /// `num / den` as Q16, floored.
    pub const fn ratio(num: i32, den: i32) -> Q16 {
        Q16(mul_div_floor(num, Q16_ONE, den))
    }

    /// `self * o`, floored.
    pub const fn mul(self, o: Q16) -> Q16 {
        Q16(((self.0 as i64 * o.0 as i64) >> 16) as i32)
    }

    /// Integer part (floor).
    pub const fn floor(self) -> i32 {
        self.0 >> 16
    }

    /// Fraction part, `0..Q16_ONE`.
    pub const fn frac(self) -> Q16 {
        Q16(self.0 & (Q16_ONE - 1))
    }

    /// Linear interpolation `a + (b - a) * t`.
    pub const fn lerp(a: Q16, b: Q16, t: Q16) -> Q16 {
        Q16(a.0 + (((b.0 as i64 - a.0 as i64) * t.0 as i64) >> 16) as i32)
    }

    /// The smoothstep `t * t * (3 - 2t)` on `0..=1`.
    pub const fn smooth(t: Q16) -> Q16 {
        let t = t.0 as i64;
        let t2 = (t * t) >> 16;
        Q16(((t2 * (3 * Q16_ONE as i64 - 2 * t)) >> 16) as i32)
    }

    pub const fn clamp01(self) -> Q16 {
        if self.0 < 0 {
            Q16::ZERO
        } else if self.0 > Q16_ONE {
            Q16::ONE
        } else {
            self
        }
    }
}

impl Add for Q16 {
    type Output = Q16;
    fn add(self, o: Q16) -> Q16 {
        Q16(self.0 + o.0)
    }
}
impl Sub for Q16 {
    type Output = Q16;
    fn sub(self, o: Q16) -> Q16 {
        Q16(self.0 - o.0)
    }
}

/// `a * b / c` through `i64`, floored (Euclidean for a positive divisor).
pub const fn mul_div_floor(a: i32, b: i32, c: i32) -> i32 {
    let n = a as i64 * b as i64;
    let d = c as i64;
    let q = n.div_euclid(d);
    // div_euclid floors for a positive divisor; for a negative one, floor explicitly.
    let q = if d < 0 && q * d != n { q - 1 } else { q };
    q as i32
}

/// `a * b / c` through `i64`, rounded half away from zero. Only where the TypeScript rounded.
pub const fn mul_div_round(a: i32, b: i32, c: i32) -> i32 {
    div_round(a as i64 * b as i64, c as i64) as i32
}

/// `a / b` rounded half away from zero (JavaScript's `Math.round` for positive values).
pub const fn div_round(a: i64, b: i64) -> i64 {
    let (a, b) = if b < 0 { (-a, -b) } else { (a, b) };
    if a >= 0 { (2 * a + b) / (2 * b) } else { -((-2 * a + b) / (2 * b)) }
}

/// `a / b` floored.
pub const fn div_floor(a: i64, b: i64) -> i64 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) { q - 1 } else { q }
}

/// Exact integer square root, floored.
pub const fn isqrt(n: u64) -> u32 {
    if n < 2 {
        return n as u32;
    }
    // Newton from a power of two at or above the root.
    let bits = 64 - n.leading_zeros();
    let mut x: u64 = 1 << bits.div_ceil(2);
    loop {
        let y = x.midpoint(n / x);
        if y >= x {
            return x as u32;
        }
        x = y;
    }
}

/// Octile distance in tenths of a cell: 10 per straight step, 14 per diagonal.
pub const fn octile10(dx: i32, dy: i32) -> u32 {
    let ax = dx.unsigned_abs();
    let ay = dy.unsigned_abs();
    let (lo, hi) = if ax < ay { (ax, ay) } else { (ay, ax) };
    10 * hi + 4 * lo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_and_centres() {
        assert_eq!(Fx::from_cells(3).cell(), 3);
        assert_eq!(Fx::centre_of(3).cell(), 3);
        assert_eq!(Fx::centre_of(0).0, 1024);
        assert_eq!(Fx(-1).cell(), -1);
        assert_eq!(Fx::from_px(8).0, CELL_FX);
        assert_eq!(Vec2::centre(2, 5).cell(), (2, 5));
    }

    #[test]
    fn division_floors_and_rounds_as_documented() {
        assert_eq!(mul_div_floor(7, 1, 2), 3);
        assert_eq!(mul_div_floor(-7, 1, 2), -4);
        assert_eq!(mul_div_floor(7, 1, -2), -4);
        assert_eq!(mul_div_floor(-7, 1, -2), 3);
        assert_eq!(mul_div_round(5, 1, 2), 3);
        assert_eq!(mul_div_round(-5, 1, 2), -3);
        assert_eq!(mul_div_round(4, 1, 3), 1);
        assert_eq!(div_floor(-1, 3), -1);
        assert_eq!(div_floor(1, 3), 0);
        assert_eq!(div_floor(-3, 3), -1);
        assert_eq!(scale(1000, Permille(620)), 620);
        assert_eq!(scale(-1, Permille(500)), -1);
        // i64 intermediate: no overflow on the product.
        assert_eq!(mul_div_floor(i32::MAX, 1000, 1000), i32::MAX);
    }

    #[test]
    fn isqrt_is_exact() {
        for n in 0u64..20_000 {
            let r = u64::from(isqrt(n));
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
        for n in [u64::MAX, u64::MAX - 1, 1 << 62, (1 << 32) - 1, 1 << 32, 999_999_999_999] {
            let r = u64::from(isqrt(n));
            assert!(r * r <= n);
            assert!((r + 1).checked_mul(r + 1).is_none_or(|s| s > n), "{n}");
        }
    }

    #[test]
    fn q16_basics() {
        assert_eq!(Q16::ratio(1, 2), Q16::HALF);
        assert_eq!(Q16::HALF.mul(Q16::HALF).0, Q16_ONE / 4);
        assert_eq!(Q16::smooth(Q16::ZERO), Q16::ZERO);
        assert_eq!(Q16::smooth(Q16::ONE), Q16::ONE);
        assert_eq!(Q16::smooth(Q16::HALF), Q16::HALF);
        let mut prev = -1;
        for t in (0..=Q16_ONE).step_by(97) {
            let s = Q16::smooth(Q16(t)).0;
            assert!(s >= prev, "smooth is monotone");
            prev = s;
        }
        assert_eq!(Q16::lerp(Q16::from_int(2), Q16::from_int(4), Q16::HALF), Q16::from_int(3));
        assert_eq!(Q16(-1).floor(), -1);
        assert_eq!(Q16(Q16_ONE + 5).frac().0, 5);
    }

    #[test]
    fn distances() {
        let a = Vec2::centre(0, 0);
        let b = Vec2::centre(3, 4);
        assert_eq!(dist_sq(a, b), 25 * i64::from(CELL_FX) * i64::from(CELL_FX));
        assert!(within(a, b, Fx::from_cells(5)));
        assert!(!within(a, b, Fx(Fx::from_cells(5).0 - 1)));
        assert_eq!(octile10(3, -4), 40 + 12);
    }

    #[test]
    fn ticks_saturate() {
        assert_eq!(Tick(u32::MAX - 1).after(Tick(5)), Tick(u32::MAX));
        assert_eq!(Tick(3).since(Tick(5)), Tick(0));
        assert_eq!(Tick::from_secs(2), Tick(120));
    }
}
