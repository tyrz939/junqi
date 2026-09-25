//! Picks by rank rather than by dice (PORT.md §6.a): a named ranking hashes every candidate, and
//! the pick is the cell that ranks first. Rule out any other cell (the rail took it, a new row
//! filled it) and the pick stays where it was, where a roll into the list would land elsewhere.

use jane_core::ZoneId;
use jane_core::hash::{Fnv, mix32};

use crate::steps::Step;

/// The base of one ranking: a `(seed, zone, step, attempt, a, b)` key, like `dice`.
pub fn rank_base(seed: u32, step: Step, attempt: u8, a: i32, b: i32) -> u32 {
    let key = Fnv::new().u16(ZoneId::County.into()).u16(step.into()).u8(attempt).i32(a).i32(b).finish();
    mix32(seed ^ mix32(key))
}

/// A thing's place in a ranking, 32 bits.
pub fn rank_of(base: u32, n: u32) -> u32 {
    mix32(base ^ n.wrapping_add(1).wrapping_mul(0x9e37_79b1))
}

/// A rank as a fraction in thousandths, for a threshold written as a chance.
pub fn rank_permille(base: u32, n: u32) -> i32 {
    ((u64::from(rank_of(base, n)) * 1000) >> 32) as i32
}

/// The macro cell (as `y * w + x`) that ranks first; ties go to the lower cell. `None` for none.
pub fn ranked(base: u32, cells: &[u32]) -> Option<u32> {
    cells.iter().copied().min_by_key(|&c| (rank_of(base, c), c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pick_survives_losing_another_cell() {
        let base = rank_base(7, Step::SkelAreas, 0, 3, 0);
        let cells: Vec<u32> = (0..500).collect();
        let pick = ranked(base, &cells).unwrap();
        let fewer: Vec<u32> =
            cells.iter().copied().filter(|&c| c != (pick + 1) % 500 && c != (pick + 7) % 500).collect();
        assert_eq!(ranked(base, &fewer), Some(pick));
        assert_ne!(ranked(rank_base(7, Step::SkelAreas, 0, 4, 0), &cells), Some(pick), "another row ranks its own way");
        assert_eq!(ranked(base, &[]), None);
        assert!((0..1000).contains(&rank_permille(base, 3)));
    }
}
