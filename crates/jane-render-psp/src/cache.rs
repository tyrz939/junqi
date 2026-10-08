//! The bookkeeping of what is held where (PORT.md §13.12): which pack pages are in main RAM
//! (an LRU under a byte budget, loaded from the Memory Stick on a miss) and which sit in a VRAM
//! slot (an LRU of fixed slots, uploaded from RAM on a miss). The bytes themselves are the GE
//! glue's; this decides, and a PC test can hold it to its rules.

use alloc::vec::Vec;

/// Pages held under a byte budget, least recently used first out. A page used this frame is
/// never put out to make room: the frame needs it, so the budget is exceeded instead (and
/// [`Lru::over`] says by how much) until the next frame lets it go.
#[derive(Clone, Debug)]
pub struct Lru {
    /// `(page, bytes, last frame used)`.
    held: Vec<(u16, u32, u32)>,
    budget: u32,
    bytes: u32,
    frame: u32,
    pub loads: u32,
    pub hits: u32,
}

impl Lru {
    pub fn new(budget: u32) -> Lru {
        Lru { held: Vec::new(), budget, bytes: 0, frame: 1, loads: 0, hits: 0 }
    }

    /// A new frame: what the last one used may now go.
    pub fn next_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// `page` (of `bytes`) is wanted now. `true` if it is held; else it is taken in, and the
    /// pages put out to make room are pushed to `out` (the caller frees them) before the caller
    /// loads it.
    pub fn want(&mut self, page: u16, bytes: u32, out: &mut Vec<u16>) -> bool {
        if let Some(e) = self.held.iter_mut().find(|e| e.0 == page) {
            e.2 = self.frame;
            self.hits += 1;
            return true;
        }
        while self.bytes + bytes > self.budget {
            // The least recently used page not wanted this frame.
            let Some(i) =
                (0..self.held.len()).filter(|&i| self.held[i].2 != self.frame).min_by_key(|&i| self.held[i].2)
            else {
                break;
            };
            let (p, b, _) = self.held.swap_remove(i);
            self.bytes -= b;
            out.push(p);
        }
        self.held.push((page, bytes, self.frame));
        self.bytes += bytes;
        self.loads += 1;
        false
    }

    /// Bytes held.
    /// The byte budget.
    pub fn budget(&self) -> u32 {
        self.budget
    }

    pub fn bytes(&self) -> u32 {
        self.bytes
    }

    /// Bytes of the pages this frame has wanted (its working set).
    pub fn frame_bytes(&self) -> u32 {
        self.held.iter().filter(|e| e.2 == self.frame).map(|e| e.1).sum()
    }

    /// Bytes held past the budget (pages one frame needs at once).
    pub fn over(&self) -> u32 {
        self.bytes.saturating_sub(self.budget)
    }

    /// Lets go of `page` (its load failed: it is not held, and its bytes are not counted).
    pub fn forget(&mut self, page: u16) {
        if let Some(i) = self.held.iter().position(|e| e.0 == page) {
            let (_, b, _) = self.held.swap_remove(i);
            self.bytes -= b;
        }
    }

    /// Whether `page` is held.
    pub fn holds(&self, page: u16) -> bool {
        self.held.iter().any(|e| e.0 == page)
    }
}

/// Fixed slots (a VRAM page each), least recently used first reused. A slot drawn from this
/// frame is never reused in it: the GE may not have read it yet while the list is still being
/// written, so a page with no free slot is drawn from RAM instead.
#[derive(Clone, Debug)]
pub struct Slots {
    /// Per slot: the page in it and the frame it was last used in.
    slot: Vec<(Option<u16>, u32)>,
    frame: u32,
    pub uploads: u32,
}

impl Slots {
    pub fn new(n: usize) -> Slots {
        Slots { slot: alloc::vec![(None, 0); n], frame: 1, uploads: 0 }
    }

    pub fn len(&self) -> usize {
        self.slot.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slot.is_empty()
    }

    /// A new frame: the last one's slots may be reused.
    pub fn next_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// The slot `page` is in now, and whether it must be uploaded there first; `None` when every
    /// slot is in use this frame. The slot used longest ago is taken for a page not in one.
    pub fn place(&mut self, page: u16) -> Option<(usize, bool)> {
        if let Some(i) = self.slot.iter().position(|s| s.0 == Some(page)) {
            self.slot[i].1 = self.frame;
            return Some((i, false));
        }
        let i = (0..self.slot.len())
            .filter(|&i| self.slot[i].1 != self.frame)
            .min_by_key(|&i| (self.slot[i].0.is_some(), self.slot[i].1))?;
        self.slot[i] = (Some(page), self.frame);
        self.uploads += 1;
        Some((i, true))
    }

    /// Every slot empty (VRAM not trusted after a sleep: each page uploads again when drawn).
    pub fn clear(&mut self) {
        for s in &mut self.slot {
            s.0 = None;
        }
    }

    /// Slots holding a page.
    pub fn filled(&self) -> usize {
        self.slot.iter().filter(|s| s.0.is_some()).count()
    }

    /// Forgets `page` (its RAM copy went: a slot holding it is stale).
    pub fn forget(&mut self, page: u16) {
        for s in &mut self.slot {
            if s.0 == Some(page) {
                s.0 = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_recently_used_page_goes_first_but_never_one_this_frame_needs() {
        let mut l = Lru::new(300);
        let mut out = Vec::new();
        assert!(!l.want(1, 100, &mut out));
        l.next_frame();
        assert!(!l.want(2, 100, &mut out));
        l.next_frame();
        assert!(!l.want(3, 100, &mut out));
        assert!(l.want(1, 100, &mut out), "page 1 is held");
        l.next_frame();
        // Full: 2 is the least recently used.
        assert!(!l.want(4, 100, &mut out));
        assert_eq!(out, [2]);
        // Every page used this frame: the budget gives.
        out.clear();
        assert!(l.want(1, 100, &mut out) && l.want(3, 100, &mut out));
        assert!(!l.want(5, 100, &mut out));
        assert!(out.is_empty(), "nothing this frame used went");
        assert_eq!(l.over(), 100);
        assert_eq!(l.bytes(), 400);
        // A page whose load failed is let go: not held, its bytes uncounted, wanted anew.
        l.forget(5);
        assert!(!l.holds(5));
        assert_eq!(l.bytes(), 300);
        assert!(!l.want(5, 100, &mut out), "a forgotten page loads again");
    }

    #[test]
    fn every_slot_empties_at_once_after_a_resume() {
        let mut s = Slots::new(3);
        s.place(1);
        s.place(2);
        s.next_frame();
        s.clear();
        assert_eq!(s.filled(), 0);
        assert!(s.place(1).is_some_and(|p| p.1), "uploaded again");
        assert!(s.place(2).is_some_and(|p| p.1), "uploaded again");
    }

    #[test]
    fn slots_reuse_the_one_used_longest_ago_but_never_one_this_frame() {
        let mut s = Slots::new(2);
        assert_eq!(s.place(7), Some((0, true)));
        s.next_frame();
        assert_eq!(s.place(8), Some((1, true)));
        assert_eq!(s.place(9), Some((0, true)), "7 was used longest ago");
        assert_eq!(s.place(10), None, "both slots are this frame's");
        assert_eq!(s.place(8), Some((1, false)));
        s.next_frame();
        s.forget(9);
        assert_eq!(s.place(9), Some((0, true)), "a forgotten page uploads again");
        assert_eq!(s.uploads, 4);
    }
}
