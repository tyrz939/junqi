//! The terrain chunk cache (PRESENTATION.md §1.6): 16 x 16 cells painted into `CHUNK_PX`
//! square layers, LRU 48, keyed by `(id, generation)`, invalidated per rect on `Event::Tiles(rect)`,
//! dropped whole on a zone change. The layers themselves live in the `Frame` (by slot), so a
//! backend reads them from the frame it is handed; this is the bookkeeping.
//!
//! A chunk is painted properly (the terrain painter) or roughly (its cells' flat swatches, for
//! the ticks the painter's budget has not reached it yet, §1.6). A stale chunk keeps drawing
//! what it last had until it is painted again.

use alloc::vec;
use alloc::vec::Vec;
use jane_core::Rect;

use crate::frame::{CHUNK_CELLS, ChunkId, ChunkLayers, Tier};

/// Chunks kept at most: the GPU backends' slot atlases are this many.
pub const LRU: usize = 48;

/// Slots kept beyond what the view and its paint-ahead band want, so a step back over a chunk
/// just left finds it painted (PLAY-PLAN.md §7: the cache is sized by need, not by `LRU`).
pub const SLACK: usize = 6;

/// Slots made at boot: a canvas's view and band on the county, so walking never allocates.
pub const RESERVE: usize = 24;

#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    id: Option<ChunkId>,
    generation: u32,
    /// The tick it was last wanted.
    used: u32,
    /// Its cells changed since it was painted.
    stale: bool,
    /// Painted as swatches, waiting for the painter.
    rough: bool,
}

/// What a chunk needs before it is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    /// Painted properly and fresh.
    Nothing,
    /// Painted as swatches: the painter, when the budget allows.
    Paint,
    /// Painted, but its cells have changed since.
    Repaint,
    /// Not in the cache: nothing to draw until it is painted.
    Missing,
}

#[derive(Debug)]
pub struct ChunkCache {
    slots: Vec<Slot>,
    /// What a slot's layers are made for: the albedo alone at T0, all four above.
    tier: Tier,
    /// The most slots it may hold now ([`ChunkCache::fit`]): what the view wants, plus [`SLACK`].
    cap: usize,
    /// The most it ever holds: [`LRU`], or a console's fixed count ([`ChunkCache::reserved`]).
    most: usize,
    /// Its layers are a console's `T8` ones (`ChunkLayers::new_t8`).
    t8: bool,
    next_gen: u32,
    /// Chunks painted since New Game (a test reads it).
    pub painted: u32,
    /// Of them, those the terrain painter painted (not swatches).
    pub landed: u32,
}

impl ChunkCache {
    /// An empty cache whose slots are made as they are first wanted.
    pub fn new(tier: Tier) -> ChunkCache {
        ChunkCache { slots: Vec::new(), tier, cap: LRU, most: LRU, t8: false, next_gen: 0, painted: 0, landed: 0 }
    }

    /// A cache with [`RESERVE`] slots' layers made now (about 330 KB each on `soft`, 704 KB each
    /// with the four layers of T1 and T2), so walking the county never allocates; it grows past
    /// them only as far as [`fit`](Self::fit) lets it.
    pub fn reserved(layers: &mut Vec<ChunkLayers>, tier: Tier) -> ChunkCache {
        ChunkCache::fixed(layers, tier, RESERVE, LRU, false)
    }

    /// A cache with `n` slots made now, never more than `most`, `T8` ones if `t8` (a console's
    /// memory: PORT.md §13.12).
    pub fn fixed(layers: &mut Vec<ChunkLayers>, tier: Tier, n: usize, most: usize, t8: bool) -> ChunkCache {
        layers.clear();
        layers.extend((0..n).map(|_| if t8 { ChunkLayers::new_t8() } else { ChunkLayers::new(tier) }));
        ChunkCache { slots: vec![Slot::default(); n], cap: n, most, t8, ..ChunkCache::new(tier) }
    }

    /// Sizes the cache by need (PLAY-PLAN.md §7): `want` chunks are under the view and its
    /// paint-ahead band this tick, so it may grow to that plus [`SLACK`], never past [`LRU`].
    /// It never shrinks here (that is [`shrink`](Self::shrink), at a zone change).
    pub fn fit(&mut self, want: usize) {
        self.cap = self.cap.max((want + SLACK).min(self.most));
    }

    /// After [`drop_all`](Self::drop_all): lets go of every slot past `keep` and its layers (a
    /// house wants a few chunks, not the county's band), and sizes the cap to `keep`.
    pub fn shrink(&mut self, layers: &mut Vec<ChunkLayers>, keep: usize) {
        let keep = keep.clamp(1, self.most);
        self.slots.truncate(keep);
        layers.truncate(keep);
        self.cap = keep;
    }

    /// Slots held now (`jane bench --mem`, a test).
    pub fn held(&self) -> usize {
        self.slots.len()
    }

    /// The slot holding `id` and its generation: what draws it, stale or rough or not.
    pub fn find(&self, id: ChunkId) -> Option<(u16, u32)> {
        self.slots.iter().position(|s| s.id == Some(id)).map(|i| (i as u16, self.slots[i].generation))
    }

    /// What `id` needs.
    pub fn need(&self, id: ChunkId) -> Need {
        match self.slots.iter().find(|s| s.id == Some(id)) {
            None => Need::Missing,
            Some(s) if s.stale => Need::Repaint,
            Some(s) if s.rough => Need::Paint,
            Some(_) => Need::Nothing,
        }
    }

    /// Stamps `id` as wanted at `now` without painting it.
    pub fn touch(&mut self, id: ChunkId, now: u32) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.id == Some(id)) {
            s.used = now;
        }
    }

    /// Marks every chunk touching `cells` stale (`Event::Tiles`).
    pub fn invalidate(&mut self, cells: Rect) {
        for s in &mut self.slots {
            if let Some(id) = s.id {
                let r =
                    Rect::new(i32::from(id.cx) * CHUNK_CELLS, i32::from(id.cy) * CHUNK_CELLS, CHUNK_CELLS, CHUNK_CELLS);
                if r.overlaps(cells) {
                    s.stale = true;
                }
            }
        }
    }

    /// Forgets every chunk (a zone change). The layers' memory is kept for the next zone.
    pub fn drop_all(&mut self) {
        for s in &mut self.slots {
            *s = Slot::default();
        }
    }

    /// Makes sure `id` is painted and fresh, painting it into `layers` with `paint(slot, layers)`
    /// if not; `rough` says the paint is the swatches, which a proper paint later replaces (a
    /// rough `want` of a chunk painted properly and fresh paints nothing). `now` stamps it as
    /// wanted. A new slot is taken while there are fewer than [`LRU`], then the least recently
    /// wanted one is reused.
    pub fn want(
        &mut self,
        id: ChunkId,
        now: u32,
        layers: &mut Vec<ChunkLayers>,
        rough: bool,
        paint: impl FnOnce(u16, &mut ChunkLayers),
    ) {
        if let Some(i) = self.slots.iter().position(|s| s.id == Some(id)) {
            let s = &mut self.slots[i];
            s.used = now;
            if !s.stale && (rough || !s.rough) {
                return;
            }
            self.paint_into(i, id, now, layers, rough, paint);
            return;
        }
        let i = if let Some(free) = self.slots.iter().position(|s| s.id.is_none()) {
            free
        } else if self.slots.len() < self.cap {
            self.slots.push(Slot::default());
            layers.push(if self.t8 { ChunkLayers::new_t8() } else { ChunkLayers::new(self.tier) });
            self.slots.len() - 1
        } else {
            // Least recently wanted; ties to the lowest slot.
            (0..self.slots.len()).min_by_key(|&i| self.slots[i].used).expect("LRU > 0")
        };
        self.paint_into(i, id, now, layers, rough, paint);
    }

    fn paint_into(
        &mut self,
        i: usize,
        id: ChunkId,
        now: u32,
        layers: &mut [ChunkLayers],
        rough: bool,
        paint: impl FnOnce(u16, &mut ChunkLayers),
    ) {
        paint(i as u16, &mut layers[i]);
        self.next_gen = self.next_gen.wrapping_add(1);
        self.slots[i] = Slot { id: Some(id), generation: self.next_gen, used: now, stale: false, rough };
        self.painted += 1;
        self.landed += u32::from(!rough);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(cx: u16, cy: u16) -> ChunkId {
        ChunkId { cx, cy }
    }

    #[test]
    fn a_chunk_is_painted_once_then_again_when_its_tiles_change() {
        let (mut c, mut layers) = (ChunkCache::new(Tier::T0), Vec::new());
        c.want(id(1, 1), 0, &mut layers, false, |_, l| l.albedo.fill(7));
        c.want(id(1, 1), 1, &mut layers, false, |_, _| panic!("painted twice"));
        let (slot, generation) = c.find(id(1, 1)).unwrap();
        assert_eq!(layers[usize::from(slot)].albedo[0], 7);
        // A tile changed in a neighbouring chunk: this one is untouched.
        c.invalidate(Rect::new(40, 16, 2, 2));
        assert_eq!(c.find(id(1, 1)), Some((slot, generation)));
        // One changed inside it: it draws what it had until it is repainted under a new generation.
        c.invalidate(Rect::new(20, 20, 1, 1));
        assert_eq!(c.need(id(1, 1)), Need::Repaint);
        assert_eq!(c.find(id(1, 1)), Some((slot, generation)));
        c.want(id(1, 1), 2, &mut layers, false, |_, l| l.albedo.fill(9));
        let (slot2, gen2) = c.find(id(1, 1)).unwrap();
        assert_eq!(slot2, slot);
        assert_ne!(gen2, generation);
        assert_eq!(layers[usize::from(slot)].albedo[0], 9);
        assert_eq!(c.need(id(1, 1)), Need::Nothing);
    }

    #[test]
    fn a_rough_chunk_is_painted_again_properly_and_never_the_other_way() {
        let (mut c, mut layers) = (ChunkCache::new(Tier::T0), Vec::new());
        assert_eq!(c.need(id(2, 3)), Need::Missing);
        c.want(id(2, 3), 0, &mut layers, true, |_, l| l.albedo.fill(1));
        assert_eq!(c.need(id(2, 3)), Need::Paint);
        c.want(id(2, 3), 1, &mut layers, true, |_, _| panic!("a swatch over a swatch"));
        c.want(id(2, 3), 2, &mut layers, false, |slot, l| {
            assert_eq!(slot, 0);
            l.albedo.fill(2);
        });
        assert_eq!(c.need(id(2, 3)), Need::Nothing);
        c.want(id(2, 3), 3, &mut layers, true, |_, _| panic!("a swatch over the painter's work"));
        assert_eq!(layers[0].albedo[0], 2);
    }

    #[test]
    fn it_keeps_48_and_reuses_the_least_recently_wanted() {
        let (mut c, mut layers) = (ChunkCache::new(Tier::T0), Vec::new());
        for i in 0..LRU as u16 {
            c.want(id(i, 0), u32::from(i), &mut layers, false, |_, _| {});
        }
        // Chunk 0 wanted again, so chunk 1 is now the oldest.
        c.want(id(0, 0), 100, &mut layers, false, |_, _| {});
        c.want(id(99, 0), 101, &mut layers, false, |_, _| {});
        assert_eq!(layers.len(), LRU);
        assert!(c.find(id(0, 0)).is_some());
        assert!(c.find(id(1, 0)).is_none());
        assert!(c.find(id(99, 0)).is_some());
    }

    #[test]
    fn a_zone_change_drops_every_chunk_and_keeps_the_memory() {
        let (mut c, mut layers) = (ChunkCache::new(Tier::T0), Vec::new());
        for i in 0..5 {
            c.want(id(i, 0), 0, &mut layers, false, |_, _| {});
        }
        c.drop_all();
        assert!((0..5).all(|i| c.find(id(i, 0)).is_none()));
        c.want(id(0, 0), 1, &mut layers, false, |_, _| {});
        assert_eq!(layers.len(), 5);
    }
}
