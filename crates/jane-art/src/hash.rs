//! `h32` and the salts every generator hashes with (ART.md §1): one helper over jane-core's
//! FNV-1a builder and `mix32`, so art has no hash function of its own.
//!
//! A sprite's seed is a stable id (a prop's row id, a unit's id, a cell), never the sim's RNG.

use jane_core::hash::Fnv;

/// 32 well-mixed bits from `(a, b, salt)`: `mix32(fnv1a(a, b, salt))`, each field written as
/// four little-endian bytes.
pub const fn h32(a: u32, b: u32, salt: u32) -> u32 {
    Fnv::new().u32(a).u32(b).u32(salt).mix()
}

/// `h32` reduced to `0..n` (by multiply-shift, so every residue is as likely as the next).
/// `n` must be positive.
pub const fn below(h: u32, n: u32) -> u32 {
    ((h as u64 * n as u64) >> 32) as u32
}

/// Salts: one per use, so two generators reading the same seed never see the same bits.
pub mod salt {
    /// `Canvas::strokes`: stroke placement, length and tone.
    pub const STROKES: u32 = 0x5354_524b;
    /// `vary`: the renderer's pick of a unit's variant (ART.md §3).
    pub const VARY: u32 = 0x5641_5259;
    /// Demo sprites on the step-1 sheets.
    pub const DEMO: u32 = 0x4445_4d4f;
    /// The people composer: hair strokes, the fallen's pool.
    pub const PERSON: u32 = 0x5045_5253;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salts_and_fields_separate() {
        let a = h32(1, 2, salt::STROKES);
        assert_ne!(a, h32(2, 1, salt::STROKES));
        assert_ne!(a, h32(1, 2, salt::VARY));
        assert_eq!(a, h32(1, 2, salt::STROKES));
    }

    #[test]
    fn below_stays_below() {
        for i in 0..1000 {
            assert!(below(h32(i, 0, 0), 7) < 7);
        }
        assert_eq!(below(u32::MAX, 3), 2);
        assert_eq!(below(0, 3), 0);
    }
}
