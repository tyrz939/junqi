//! The weighted pick, sort helpers, `Lookup`, and the view constants.

use core::hash::{BuildHasherDefault, Hash};

// std's HashMap is this map; hashbrown carries it to `no_std` (PORT.md §13.9). `Lookup` cannot be
// iterated and its hasher is fixed, so the table it gives is the same on every target.
#[allow(clippy::disallowed_types)]
use hashbrown::HashMap;

use crate::hash::FnvHasher;
use crate::rng::Sfc32;

/// The ordered map of the float-free crates, with `Lookup`'s fixed FNV hasher. An `IndexMap`'s
/// order is the order of insertion whatever its hasher (the hasher only finds a key's slot), so
/// this is the same map std's default hasher gave, and it builds without std (PORT.md §13.9).
/// Make one with `default()`; `new()` belongs to std's hasher.
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, BuildHasherDefault<FnvHasher>>;

/// A weighted pick over an ordered slice from data (PORT.md §6.d replaces `Object.entries`
/// order). Zero weights are never picked; `None` when every weight is zero.
pub fn pick_weighted<'a, T>(items: &'a [(T, u32)], rng: &mut Sfc32) -> Option<&'a T> {
    let total: u64 = items.iter().map(|(_, w)| u64::from(*w)).sum();
    if total == 0 {
        return None;
    }
    // Draw in u32 space when it fits (always, for content weights), else two draws.
    let mut r = if let Ok(t) = u32::try_from(total) {
        u64::from(rng.below(t))
    } else {
        ((u64::from(rng.next_u32()) << 32) | u64::from(rng.next_u32())) % total
    };
    for (item, w) in items {
        let w = u64::from(*w);
        if r < w {
            return Some(item);
        }
        r -= w;
    }
    None
}

/// Sort by a total key. The key must end in something unique (an id, an index): a comparator
/// returns `Equal` only for the same element (PORT.md §6.d). Stable either way.
pub fn sort_by_total_key<T, K: Ord>(items: &mut [T], key: impl FnMut(&T) -> K) {
    items.sort_by_key(key);
}

/// A hash map that cannot be iterated: get, insert, remove, nothing else (ARCHITECTURE.md §0).
/// Iteration order is array or `BTreeMap` order everywhere in the deterministic crates, and this
/// type makes that unforgeable. The hasher is fixed (FNV-1a), so no process sees another table.
#[derive(Clone, Debug)]
pub struct Lookup<K, V> {
    #[allow(clippy::disallowed_types)]
    map: HashMap<K, V, BuildHasherDefault<FnvHasher>>,
}

impl<K: Eq + Hash, V> Default for Lookup<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash, V> Lookup<K, V> {
    pub fn new() -> Self {
        #[allow(clippy::disallowed_types)]
        let map = HashMap::default();
        Self { map }
    }

    pub fn with_capacity(n: usize) -> Self {
        #[allow(clippy::disallowed_types)]
        let map = HashMap::with_capacity_and_hasher(n, BuildHasherDefault::default());
        Self { map }
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        self.map.get(k)
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.map.get_mut(k)
    }

    pub fn contains(&self, k: &K) -> bool {
        self.map.contains_key(k)
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        self.map.remove(k)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Empty it, keeping the allocation.
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

/// The camera's size, in one place (PORT.md §6.h). Every guarantee a screen makes (what it
/// shows, half-screen distances, the density rule) is stated at 48 x 27 cells and holds when a
/// wider window shows more.
///
/// **Rule:** no literal 48, 27, 24, 13, 22 or 12 in `jane-world`; read these.
pub mod view {
    /// Pixels per cell in sim units (the internal canvas draws each at 16 screen px).
    pub const CELL_PX: u32 = 8;
    /// The guaranteed frame height in sim px.
    pub const FRAME_H_PX: u32 = 216;
    /// The narrowest aspect the guarantees hold for: 16:9.
    pub const MIN_ASPECT: (u32, u32) = (16, 9);
    pub const FRAME_W_PX: u32 = FRAME_H_PX * MIN_ASPECT.0 / MIN_ASPECT.1;
    pub const VIEW_W_CELLS: u32 = FRAME_W_PX / CELL_PX;
    pub const VIEW_H_CELLS: u32 = FRAME_H_PX / CELL_PX;
    /// Half a screen: how far from the centre a thing can be and still be seen.
    pub const HALF_W_CELLS: u32 = VIEW_W_CELLS / 2;
    pub const HALF_H_CELLS: u32 = VIEW_H_CELLS / 2;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_is_48_by_27() {
        assert_eq!((view::VIEW_W_CELLS, view::VIEW_H_CELLS), (48, 27));
        assert_eq!((view::HALF_W_CELLS, view::HALF_H_CELLS), (24, 13));
    }

    #[test]
    fn weighted_pick_follows_weights_and_skips_zero() {
        let items = [("a", 0u32), ("b", 3), ("c", 1)];
        let mut rng = Sfc32::seeded(1, 2);
        let mut counts = [0u32; 3];
        for _ in 0..4000 {
            match *pick_weighted(&items, &mut rng).unwrap() {
                "a" => counts[0] += 1,
                "b" => counts[1] += 1,
                _ => counts[2] += 1,
            }
        }
        assert_eq!(counts[0], 0);
        assert!(counts[1] > 2 * counts[2], "{counts:?}");
        assert!(pick_weighted::<u8>(&[(1, 0)], &mut rng).is_none());
        assert!(pick_weighted::<u8>(&[], &mut rng).is_none());
    }

    #[test]
    fn lookup_is_a_map() {
        let mut l = Lookup::new();
        assert!(l.insert(3u32, "x").is_none());
        assert_eq!(l.get(&3), Some(&"x"));
        assert_eq!(l.remove(&3), Some("x"));
        assert!(l.is_empty());
    }
}
