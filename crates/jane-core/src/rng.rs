//! Seeded randomness: sfc32 with a splitmix32 seed, and `dice()`, the one worldgen seed scheme.
//!
//! The generator is the TypeScript build's (`jane/src/sim/rng.ts`), bit for bit: the same
//! `(seed, stream)` gives the same `u32` sequence. Everything above `next_u32` is new and
//! integer-only: there is no float draw (`rngFloat` is gone, ARCHITECTURE.md §2).

use crate::hash::{Fnv, mix32};
use crate::num::Permille;

/// sfc32: four words of state, passes PractRand, integer ops only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sfc32 {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

impl Sfc32 {
    /// `rngSeed(seed, stream)`: a splitmix32 spread of the seed over the state, then 12 warm-ups.
    pub fn seeded(seed: u32, stream: u32) -> Self {
        let mut h = seed ^ stream.wrapping_add(1).wrapping_mul(0x9e37_79b9);
        let mut next = || {
            h = h.wrapping_add(0x9e37_79b9);
            let mut z = h;
            z = (z ^ (z >> 16)).wrapping_mul(0x21f0_aaad);
            z = (z ^ (z >> 15)).wrapping_mul(0x735a_2d97);
            z ^ (z >> 15)
        };
        let mut s = Self { a: next(), b: next(), c: next(), d: next() };
        for _ in 0..12 {
            s.next_u32();
        }
        s
    }

    pub fn next_u32(&mut self) -> u32 {
        let t = self.a.wrapping_add(self.b).wrapping_add(self.d);
        self.d = self.d.wrapping_add(1);
        self.a = self.b ^ (self.b >> 9);
        self.b = self.c.wrapping_add(self.c << 3);
        self.c = self.c.rotate_left(21).wrapping_add(t);
        t
    }

    /// Uniform in `0..n` by Lemire's multiply-shift. `n == 0` gives 0.
    pub fn below(&mut self, n: u32) -> u32 {
        ((u64::from(self.next_u32()) * u64::from(n)) >> 32) as u32
    }

    /// GameMaker's `irandom(n)`: uniform in `0..=n`; `n < 0` gives 0.
    pub fn irandom(&mut self, n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        self.below(n as u32 + 1) as i32
    }

    /// Uniform in `lo..=hi`; `hi < lo` gives `lo`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (i64::from(hi) - i64::from(lo)) as u32;
        (i64::from(lo) + i64::from(self.below(span.wrapping_add(1)))) as i32
    }

    /// True with probability `p / 1000`.
    pub fn chance(&mut self, p: Permille) -> bool {
        (self.below(1000) as i32) < i32::from(p.0)
    }

    /// One in `n`: true with probability `1 / n`; `n <= 1` is always true.
    pub fn one_in(&mut self, n: u32) -> bool {
        n <= 1 || self.below(n) == 0
    }

    /// A uniform element; `None` for an empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let i = self.below(u32::try_from(items.len()).unwrap_or(u32::MAX));
        items.get(i as usize)
    }

    /// Fisher-Yates, high index first. Deterministic for a given state.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let mut i = items.len();
        while i > 1 {
            let j = self.below(i as u32) as usize;
            i -= 1;
            items.swap(i, j);
        }
    }
}

/// The dice of one step of a build: `(seed, zone, step, attempt, a, b)` to a stream of its own
/// (PORT.md §6.a). The six seed schemes of the TypeScript become this one call.
///
/// `zone` and `step` are the `u16` of `ZoneId` and `jane_world::Step`; `a` and `b` name the row,
/// cell or point a per-row stage throws for (0 when the stage throws once).
///
/// **Rule:** every stage draws only from its own `dice()`.
pub fn dice(seed: u32, zone: u16, step: u16, attempt: u8, a: i32, b: i32) -> Sfc32 {
    let key = Fnv::new().u16(zone).u16(step).u8(attempt).i32(a).i32(b).finish();
    Sfc32::seeded(seed ^ mix32(key), 0)
}

/// A rank by hash for picks that must not draw: `mix32(fnv1a(step, id, n))` (PORT.md §6.a).
pub fn rank(step: u16, id: u32, n: u32) -> u32 {
    Fnv::new().u16(step).u32(id).u32(n).mix()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known answers from the TypeScript generator (`rngSeed`, `rngU32` in jane/src/sim/rng.ts),
    /// produced by `tools/gen/rng-vectors.mjs`. Integers match: the generator carries bit for bit.
    #[test]
    fn sfc32_matches_the_typescript_known_answers() {
        for &(seed, stream, expect) in crate::rng_vectors::VECTORS {
            let mut r = Sfc32::seeded(seed, stream);
            let got: [u32; 8] = std::array::from_fn(|_| r.next_u32());
            assert_eq!(got, expect, "seed {seed} stream {stream}");
        }
    }

    #[test]
    fn below_stays_below_and_zero_is_zero() {
        let mut r = Sfc32::seeded(7, 0);
        assert_eq!(r.below(0), 0);
        for n in [1u32, 2, 3, 10, 1000, u32::MAX] {
            for _ in 0..200 {
                assert!(r.below(n) < n.max(1));
            }
        }
    }

    #[test]
    fn range_is_inclusive_and_covers_both_ends() {
        let mut r = Sfc32::seeded(11, 3);
        let mut seen = [false; 5];
        for _ in 0..500 {
            let v = r.range(-2, 2);
            assert!((-2..=2).contains(&v));
            seen[(v + 2) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(r.range(5, 5), 5);
        assert_eq!(r.range(5, 1), 5);
        let v = r.range(i32::MIN, i32::MAX);
        let _ = v;
    }

    #[test]
    fn irandom_is_gamemakers() {
        let mut r = Sfc32::seeded(1, 1);
        assert_eq!(r.irandom(0), 0);
        assert_eq!(r.irandom(-5), 0);
        for _ in 0..200 {
            assert!((0..=3).contains(&r.irandom(3)));
        }
    }

    #[test]
    fn chance_edges() {
        let mut r = Sfc32::seeded(3, 0);
        for _ in 0..200 {
            assert!(!r.chance(Permille(0)));
            assert!(r.chance(Permille(1000)));
        }
    }

    #[test]
    fn dice_is_distinct_per_step_attempt_and_row() {
        let first = |mut r: Sfc32| r.next_u32();
        let base = first(dice(42, 0, 1, 0, 0, 0));
        assert_eq!(base, first(dice(42, 0, 1, 0, 0, 0)));
        for other in [
            dice(43, 0, 1, 0, 0, 0),
            dice(42, 1, 1, 0, 0, 0),
            dice(42, 0, 2, 0, 0, 0),
            dice(42, 0, 1, 1, 0, 0),
            dice(42, 0, 1, 0, 1, 0),
            dice(42, 0, 1, 0, 0, 1),
        ] {
            assert_ne!(base, first(other));
        }
        // (a, b) and (b, a) are different rows.
        assert_ne!(first(dice(42, 0, 1, 0, 3, 4)), first(dice(42, 0, 1, 0, 4, 3)));
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut r = Sfc32::seeded(9, 9);
        let mut v: Vec<u32> = (0..50).collect();
        r.shuffle(&mut v);
        let mut s = v.clone();
        s.sort_by_key(|&x| x);
        assert_eq!(s, (0..50).collect::<Vec<_>>());
        assert_ne!(v, s);
    }
}
