//! A packed plane of flags, a bit a cell (PORT.md §13.3): what a `Vec<bool>` the size of the
//! county was, at an eighth of the bytes (4 000 000 cells: 488 KB, not 3.8 MB). Reads index as a
//! `Vec<bool>` does (`bits[i]`); writes are [`Bits::set`] and [`Bits::fill`]. Out of range
//! panics, as a slice does.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Index, Range};

/// An empty plane, to lend where one is missing.
pub static EMPTY: Bits = Bits::empty();

/// `len` flags, a bit each, row-major where it is a grid (`y * w + x`). Bits past `len` in the
/// last word are always clear, so two planes of the same flags are equal.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Bits {
    words: Vec<u64>,
    len: usize,
}

impl Bits {
    /// No flags.
    pub const fn empty() -> Self {
        Self { words: Vec::new(), len: 0 }
    }

    /// `len` flags from words, flag `i` at bit `i & 63` of word `i >> 6`; bits past `len` must be
    /// clear.
    pub fn from_words(words: Vec<u64>, len: usize) -> Self {
        assert_eq!(words.len(), len.div_ceil(64), "{len} bits");
        let b = Self { words, len };
        debug_assert!(len % 64 == 0 || b.words.last().is_none_or(|w| w >> (len % 64) == 0), "bits past the end");
        b
    }

    /// `len` flags, each `v`.
    pub fn new(len: usize, v: bool) -> Self {
        let mut b = Self { words: vec![if v { u64::MAX } else { 0 }; len.div_ceil(64)], len };
        b.trim();
        b
    }

    fn trim(&mut self) {
        if self.len % 64 != 0 {
            if let Some(last) = self.words.last_mut() {
                *last &= (1u64 << (self.len % 64)) - 1;
            }
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn get(&self, i: usize) -> bool {
        assert!(i < self.len, "bit {i} of {}", self.len);
        self.words[i >> 6] >> (i & 63) & 1 != 0
    }

    #[inline]
    pub fn set(&mut self, i: usize, v: bool) {
        assert!(i < self.len, "bit {i} of {}", self.len);
        let m = 1u64 << (i & 63);
        if v {
            self.words[i >> 6] |= m;
        } else {
            self.words[i >> 6] &= !m;
        }
    }

    /// Every flag in `r` set to `v`.
    pub fn fill(&mut self, r: Range<usize>, v: bool) {
        assert!(r.start <= r.end && r.end <= self.len, "bits {r:?} of {}", self.len);
        let (mut i, end) = (r.start, r.end);
        while i < end {
            if i & 63 == 0 && end - i >= 64 {
                self.words[i >> 6] = if v { u64::MAX } else { 0 };
                i += 64;
            } else {
                self.set(i, v);
                i += 1;
            }
        }
    }

    /// Every flag clear.
    pub fn clear_all(&mut self) {
        self.words.fill(0);
    }

    /// `len` flags, all clear, in the words already held.
    pub fn reset(&mut self, len: usize) {
        self.words.clear();
        self.words.resize(len.div_ceil(64), 0);
        self.len = len;
    }

    /// Flags `64 k` to `64 k + 63`, flag `64 k + j` at bit `j` (past `len`, clear).
    #[inline]
    pub fn word(&self, k: usize) -> u64 {
        self.words[k]
    }

    /// Every flag, in order.
    pub fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        (0..self.len).map(|i| self.words[i >> 6] >> (i & 63) & 1 != 0)
    }

    /// Every flag set that is set in `words` (word `k` over flags `64 k` on; as long as ours).
    pub fn or_words(&mut self, words: &[u64]) {
        assert_eq!(words.len(), self.words.len(), "{} bits", self.len);
        for (w, o) in self.words.iter_mut().zip(words) {
            *w |= o;
        }
        self.trim();
    }

    /// How many flags are set.
    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }
}

impl Index<usize> for Bits {
    type Output = bool;

    #[inline]
    fn index(&self, i: usize) -> &bool {
        if self.get(i) { &true } else { &false }
    }
}

#[cfg(test)]
mod tests {
    use super::Bits;

    #[test]
    fn bits_read_as_a_vec_of_bools_does() {
        let mut b = Bits::new(200, false);
        let mut v = alloc::vec![false; 200];
        for i in [0, 1, 63, 64, 65, 127, 128, 199] {
            b.set(i, true);
            v[i] = true;
        }
        b.fill(10..150, true);
        v[10..150].fill(true);
        b.fill(70..80, false);
        v[70..80].fill(false);
        assert!(b.iter().eq(v.iter().copied()));
        assert!((0..200).all(|i| b[i] == v[i]));
        assert_eq!(b.count_ones() as usize, v.iter().filter(|&&x| x).count());
        let t = Bits::new(70, true);
        assert_eq!(t.count_ones(), 70);
        let mut u = Bits::new(70, false);
        u.fill(0..70, true);
        assert_eq!(t, u);
    }
}
