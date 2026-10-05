//! Content writes seconds, pixels, metres, degrees and fractions as JSON numbers (`0.16`, `1.5`),
//! and this turns each into the integer the game runs on, on the host, at build time.
//!
//! No float (Grok #7): a number is read as the decimal content wrote, digits and a power of ten
//! (`serde_json::Number`'s own text, never a float we compute with), and every unit is an exact
//! integer ratio of that decimal, rounded half away from zero, as the TypeScript's `Math.round`
//! did for the positive values content holds. An edge `.5` rounds the same on every host. Units
//! are in `jane-schema/UNITS.md`.

use serde::{Deserialize, Deserializer};

use jane_core::Angle;
use jane_core::num::{Fx, Milli, Permille, Q16, Tick};

/// A JSON number as content wrote it: `m / 10^e`, with no trailing zero in `m` while `e > 0`.
/// Read it only through the unit methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Num {
    m: i64,
    e: u8,
}

/// The largest power of ten a content number may carry (18 places fit `i64`).
const MAX_PLACES: u8 = 18;

impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = serde_json::Number::deserialize(d)?;
        Num::parse(&n.to_string()).map_err(serde::de::Error::custom)
    }
}

fn fit<T: TryFrom<i64>>(v: i64, what: &str) -> Result<T, String> {
    T::try_from(v).map_err(|_| format!("{v} is out of range for {what}"))
}

/// `a / b` rounded half away from zero (`b > 0`).
fn div_round(a: i128, b: i128) -> i128 {
    let q = a / b;
    let r = a % b;
    if 2 * r.abs() >= b { q + a.signum() } else { q }
}

impl Num {
    /// For tests and tools: a number from an integer.
    pub fn from_int(v: i32) -> Num {
        Num { m: i64::from(v), e: 0 }
    }

    /// A decimal as JSON writes one: `-12.5`, `3`, `1e-7`, `2.5E+3`.
    pub fn parse(s: &str) -> Result<Num, String> {
        let bad = || format!("{s} is not a usable number");
        let (neg, body) = match s.strip_prefix('-') {
            Some(b) => (true, b),
            None => (false, s),
        };
        let (mant, exp) = match body.find(['e', 'E']) {
            Some(i) => (&body[..i], body[i + 1..].trim_start_matches('+').parse::<i32>().map_err(|_| bad())?),
            None => (body, 0),
        };
        let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
        if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return Err(bad());
        }
        let digits = format!("{int}{frac}");
        let digits = digits.trim_start_matches('0');
        let mut places = i32::try_from(frac.len()).map_err(|_| bad())? - exp;
        let mut m: i128 = 0;
        for b in digits.bytes() {
            m = m * 10 + i128::from(b - b'0');
            if m > i128::from(i64::MAX) {
                return Err(bad());
            }
        }
        while places < 0 {
            m *= 10;
            places += 1;
            if m > i128::from(i64::MAX) {
                return Err(bad());
            }
        }
        while places > 0 && m % 10 == 0 {
            m /= 10;
            places -= 1;
        }
        let e = u8::try_from(places).ok().filter(|&p| p <= MAX_PLACES).ok_or_else(bad)?;
        let m = i64::try_from(if neg { -m } else { m }).map_err(|_| bad())?;
        Ok(Num { m, e })
    }

    fn pow10(&self) -> i128 {
        10i128.pow(u32::from(self.e))
    }

    /// `self * num / den`, rounded half away from zero (`den > 0`).
    fn ratio(self, num: i64, den: i64) -> Result<i64, String> {
        let v = div_round(i128::from(self.m) * i128::from(num), i128::from(den) * self.pow10());
        i64::try_from(v)
            .ok()
            .filter(|v| v.abs() <= 9_000_000_000_000_000)
            .ok_or_else(|| format!("{self} is not a usable number"))
    }

    /// An exact integer, or an error naming the number.
    pub fn int(self) -> Result<i64, String> {
        if self.e != 0 {
            return Err(format!("{self} must be a whole number"));
        }
        Ok(self.m)
    }

    /// True for a value below or at 1: the TypeScript's "fraction of" convention (heal).
    pub fn at_most_one(self) -> bool {
        i128::from(self.m) <= self.pow10()
    }

    pub fn is_positive(self) -> bool {
        self.m > 0
    }

    pub fn is_negative(self) -> bool {
        self.m < 0
    }

    /// Seconds to 60 Hz ticks.
    pub fn ticks(self) -> Result<Tick, String> {
        if self.is_negative() {
            return Err(format!("{self} seconds is negative"));
        }
        Ok(Tick(fit(self.ratio(60, 1)?, "ticks")?))
    }

    /// A fraction (`0.62`) to thousandths.
    pub fn permille(self) -> Result<Permille, String> {
        Ok(Permille(fit(self.ratio(1000, 1)?, "permille")?))
    }

    /// Points (hp, mp, damage) to thousandths of a point.
    pub fn milli(self) -> Result<Milli, String> {
        Ok(Milli(fit(self.ratio(1000, 1)?, "milli")?))
    }

    /// Pixels (per tick, or a radius) to 1/256 px.
    pub fn fx_px(self) -> Result<Fx, String> {
        Ok(Fx(fit(self.ratio(256, 1)?, "Fx")?))
    }

    /// Metres (a cell each) to 1/256 px.
    pub fn fx_metres(self) -> Result<Fx, String> {
        Ok(Fx(fit(self.ratio(2048, 1)?, "Fx")?))
    }

    /// Degrees to an `Angle` (65 536 per turn); a full turn (360) is kept as 65 535 so a ring
    /// stays distinguishable from zero.
    pub fn angle(self) -> Result<Angle, String> {
        let units = self.ratio(65536, 360)?;
        if units >= 65536 {
            return Ok(Angle(u16::MAX));
        }
        Ok(Angle(units.rem_euclid(65536) as u16))
    }

    /// A fraction or field value to 16.16.
    pub fn q16(self) -> Result<Q16, String> {
        Ok(Q16(fit(self.ratio(65536, 1)?, "Q16")?))
    }

    /// Tenths (`72.5` to 725), for ranges content writes with a half.
    pub fn tenths(self) -> Result<i32, String> {
        fit(self.ratio(10, 1)?, "tenths")
    }
}

impl std::fmt::Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.e == 0 {
            return write!(f, "{}", self.m);
        }
        let p = 10u64.pow(u32::from(self.e));
        let a = self.m.unsigned_abs();
        let sign = if self.m < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:0w$}", a / p, a % p, w = usize::from(self.e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Num {
        Num::parse(s).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn units() {
        assert_eq!(n("1.5").ticks(), Ok(Tick(90)));
        assert_eq!(n("0.62").permille(), Ok(Permille(620)));
        assert_eq!(n("0.55").fx_px(), Ok(Fx(141)));
        assert_eq!(n("1.2").fx_px(), Ok(Fx(307)));
        assert_eq!(n("2.0").fx_metres(), Ok(Fx(4096)));
        assert_eq!(n("90.0").angle(), Ok(Angle(16384)));
        assert_eq!(n("360").angle(), Ok(Angle(65535)));
        assert_eq!(n("0.5").q16(), Ok(Q16(32768)));
        assert_eq!(n("72.5").tenths(), Ok(725));
        assert!(n("1.5").int().is_err());
        assert_eq!(n("3.0").int(), Ok(3));
        assert!(n("-1").ticks().is_err());
    }

    /// The edge `.5` a float product can miss: `0.0145 * 1000` is 14.499999999999998 in a double,
    /// so float rounding gave 14; the decimal content wrote is 14.5, which rounds to 15.
    #[test]
    fn an_edge_half_rounds_as_written() {
        assert_eq!(n("0.0145").permille(), Ok(Permille(15)));
        assert_eq!(n("-0.0025").milli(), Ok(Milli(-3)));
        assert_eq!(n("1.0049999999999999").permille(), Ok(Permille(1005)));
    }

    #[test]
    fn json_spellings() {
        let read = |s: &str| serde_json::from_str::<Num>(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(read("0.16"), n("0.16"));
        assert_eq!(read("1e-7"), n("0.0000001"));
        assert_eq!(read("2.5E+3"), n("2500"));
        assert_eq!(read("-12"), Num::from_int(-12));
        assert_eq!(read("1.50"), n("1.5"));
        assert_eq!(n("0.16").to_string(), "0.16");
        assert_eq!(n("-0.05").to_string(), "-0.05");
        assert!(n("1").at_most_one() && !n("1.01").at_most_one() && n("0.3").at_most_one());
        assert!(Num::parse("1.2.3").is_err() && Num::parse("x").is_err() && Num::parse(".5").is_err());
    }
}
