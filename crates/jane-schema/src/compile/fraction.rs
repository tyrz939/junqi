//! The one place a float is named in the float-free crates (PORT.md §3.4): content writes
//! seconds, pixels, metres, degrees and fractions as JSON numbers (`0.16`, `1.5`), and this
//! turns each into the integer the game runs on, on the host, at build time. The float gate
//! skips this file by name and no other.
//!
//! Every conversion rounds half away from zero, as the TypeScript's `Math.round` did for the
//! positive values content holds. Units are in `jane-schema/UNITS.md`.

#![allow(clippy::float_arithmetic, clippy::float_cmp)]

use serde::Deserialize;

use jane_core::Angle;
use jane_core::num::{Fx, Milli, Permille, Q16, Tick};

/// A JSON number as content wrote it. Read it only through the unit methods.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Num(f64);

fn round(v: f64) -> Result<i64, String> {
    if !v.is_finite() || v.abs() > 9.0e15 {
        return Err(format!("{v} is not a usable number"));
    }
    Ok(v.round() as i64)
}

fn fit<T: TryFrom<i64>>(v: i64, what: &str) -> Result<T, String> {
    T::try_from(v).map_err(|_| format!("{v} is out of range for {what}"))
}

impl Num {
    /// For tests and tools: a number from an integer.
    pub fn from_int(v: i32) -> Num {
        Num(f64::from(v))
    }

    /// An exact integer, or an error naming the number.
    pub fn int(self) -> Result<i64, String> {
        if self.0.fract() != 0.0 {
            return Err(format!("{} must be a whole number", self.0));
        }
        round(self.0)
    }

    /// True for a value below or at 1: the TypeScript's "fraction of" convention (heal).
    pub fn at_most_one(self) -> bool {
        self.0 <= 1.0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0.0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0.0
    }

    /// Seconds to 60 Hz ticks.
    pub fn ticks(self) -> Result<Tick, String> {
        if self.0 < 0.0 {
            return Err(format!("{} seconds is negative", self.0));
        }
        Ok(Tick(fit(round(self.0 * 60.0)?, "ticks")?))
    }

    /// A fraction (`0.62`) to thousandths.
    pub fn permille(self) -> Result<Permille, String> {
        Ok(Permille(fit(round(self.0 * 1000.0)?, "permille")?))
    }

    /// Points (hp, mp, damage) to thousandths of a point.
    pub fn milli(self) -> Result<Milli, String> {
        Ok(Milli(fit(round(self.0 * 1000.0)?, "milli")?))
    }

    /// Pixels (per tick, or a radius) to 1/256 px.
    pub fn fx_px(self) -> Result<Fx, String> {
        Ok(Fx(fit(round(self.0 * 256.0)?, "Fx")?))
    }

    /// Metres (a cell each) to 1/256 px.
    pub fn fx_metres(self) -> Result<Fx, String> {
        Ok(Fx(fit(round(self.0 * 2048.0)?, "Fx")?))
    }

    /// Degrees to an `Angle` (65 536 per turn); a full turn (360) is kept as 65 535 so a ring
    /// stays distinguishable from zero.
    pub fn angle(self) -> Result<Angle, String> {
        let units = round(self.0 * 65536.0 / 360.0)?;
        if units >= 65536 {
            return Ok(Angle(u16::MAX));
        }
        Ok(Angle(units.rem_euclid(65536) as u16))
    }

    /// A fraction or field value to 16.16.
    pub fn q16(self) -> Result<Q16, String> {
        Ok(Q16(fit(round(self.0 * 65536.0)?, "Q16")?))
    }

    /// Tenths (`72.5` to 725), for ranges content writes with a half.
    pub fn tenths(self) -> Result<i32, String> {
        fit(round(self.0 * 10.0)?, "tenths")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: f64) -> Num {
        Num(v)
    }

    #[test]
    fn units() {
        assert_eq!(n(1.5).ticks(), Ok(Tick(90)));
        assert_eq!(n(0.62).permille(), Ok(Permille(620)));
        assert_eq!(n(0.55).fx_px(), Ok(Fx(141)));
        assert_eq!(n(1.2).fx_px(), Ok(Fx(307)));
        assert_eq!(n(2.0).fx_metres(), Ok(Fx(4096)));
        assert_eq!(n(90.0).angle(), Ok(Angle(16384)));
        assert_eq!(n(360.0).angle(), Ok(Angle(65535)));
        assert_eq!(n(0.5).q16(), Ok(Q16(32768)));
        assert_eq!(n(72.5).tenths(), Ok(725));
        assert!(n(1.5).int().is_err());
        assert_eq!(n(3.0).int(), Ok(3));
        assert!(n(-1.0).ticks().is_err());
    }
}
