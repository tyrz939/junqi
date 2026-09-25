//! The hash pair: FNV-1a over explicit little-endian bytes, and murmur3's 32-bit finaliser.
//!
//! Every seed derivation and every rank-by-hash pick in the game goes through these two
//! (PORT.md §6.a, §6.e). Bytes are always written little-endian and never through `usize`,
//! so a hash is the same on every target.

pub const FNV_OFFSET: u32 = 0x811c_9dc5;
pub const FNV_PRIME: u32 = 0x0100_0193;

/// FNV-1a over bytes.
pub const fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u32;
        h = h.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    h
}

/// murmur3's `fmix32`: every input bit moves every output bit.
pub const fn mix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// A 2-D lattice hash: `(x, y, salt)` to 32 well-mixed bits. The one used by noise and cell picks.
pub const fn hash2(x: i32, y: i32, salt: u32) -> u32 {
    Fnv::new().i32(x).i32(y).u32(salt).mix()
}

/// FNV-1a as a builder over typed fields, each written as little-endian bytes.
///
/// ```
/// use jane_core::hash::Fnv;
/// let h = Fnv::new().u16(3).u16(7).u8(0).i32(-4).i32(9).finish();
/// assert_eq!(h, Fnv::new().u16(3).u16(7).u8(0).i32(-4).i32(9).finish());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv(u32);

impl Default for Fnv {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv {
    pub const fn new() -> Self {
        Self(FNV_OFFSET)
    }

    pub const fn byte(self, b: u8) -> Self {
        Self((self.0 ^ b as u32).wrapping_mul(FNV_PRIME))
    }

    pub const fn bytes(mut self, bytes: &[u8]) -> Self {
        let mut i = 0;
        while i < bytes.len() {
            self = self.byte(bytes[i]);
            i += 1;
        }
        self
    }

    pub const fn u8(self, v: u8) -> Self {
        self.byte(v)
    }

    pub const fn u16(self, v: u16) -> Self {
        let b = v.to_le_bytes();
        self.byte(b[0]).byte(b[1])
    }

    pub const fn u32(self, v: u32) -> Self {
        let b = v.to_le_bytes();
        self.byte(b[0]).byte(b[1]).byte(b[2]).byte(b[3])
    }

    pub const fn i32(self, v: i32) -> Self {
        self.u32(v as u32)
    }

    pub const fn u64(self, v: u64) -> Self {
        self.u32(v as u32).u32((v >> 32) as u32)
    }

    /// A string as its UTF-8 bytes followed by a zero, so `("ab", "c")` and `("a", "bc")` differ.
    pub const fn str(self, s: &str) -> Self {
        self.bytes(s.as_bytes()).byte(0)
    }

    pub const fn finish(self) -> u32 {
        self.0
    }

    /// `mix32(finish())`: the form every seed and rank uses.
    pub const fn mix(self) -> u32 {
        mix32(self.0)
    }
}

/// A `std::hash::Hasher` over FNV-1a for [`crate::Lookup`]: fixed, so no process sees a different table.
#[derive(Clone, Copy, Debug, Default)]
pub struct FnvHasher(Fnv);

impl std::hash::Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        u64::from(self.0.mix())
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0 = self.0.bytes(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_the_reference_vectors() {
        // The published FNV-1a 32-bit test vectors.
        assert_eq!(fnv1a(b""), 0x811c_9dc5);
        assert_eq!(fnv1a(b"a"), 0xe40c_292c);
        assert_eq!(fnv1a(b"foobar"), 0xbf9c_f968);
    }

    #[test]
    fn the_builder_is_fnv1a_over_le_bytes() {
        assert_eq!(Fnv::new().bytes(b"foobar").finish(), fnv1a(b"foobar"));
        assert_eq!(Fnv::new().u32(0x0403_0201).finish(), fnv1a(&[1, 2, 3, 4]));
        assert_eq!(Fnv::new().u16(0x0201).finish(), fnv1a(&[1, 2]));
        assert_eq!(Fnv::new().i32(-1).finish(), fnv1a(&[0xff; 4]));
    }

    #[test]
    fn mix32_matches_murmur3_fmix32() {
        for &(x, want) in crate::rng_vectors::MIX32 {
            assert_eq!(mix32(x), want, "{x}");
        }
    }

    #[test]
    fn hash2_separates_neighbours_and_salts() {
        let a = hash2(10, 20, 1);
        assert_ne!(a, hash2(11, 20, 1));
        assert_ne!(a, hash2(10, 21, 1));
        assert_ne!(a, hash2(10, 20, 2));
        assert_ne!(hash2(1, 2, 0), hash2(2, 1, 0));
    }

    #[test]
    fn strings_are_terminated() {
        assert_ne!(Fnv::new().str("ab").str("c").finish(), Fnv::new().str("a").str("bc").finish());
    }
}
