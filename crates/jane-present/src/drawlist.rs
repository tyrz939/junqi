//! The draw list without allocation (PRESENTATION.md §1.5): commands pushed in any order, then a
//! counting sort on `y` (canvas rows) with ties broken by `key` (the id), into buffers reserved
//! once. Linear, stable, and the same order for the same set however it was pushed.

use crate::frame::{Caster, SpriteCmd};

/// One thing that stands: sorted by `y` (its feet, canvas px), then `key`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawCmd {
    pub y: i32,
    pub key: u32,
    pub sprite: SpriteCmd,
    /// What it throws a shadow as, if it throws one (`sprite` is filled when the frame is).
    pub caster: Option<Caster>,
}

/// The standing draw list.
#[derive(Debug)]
pub struct DrawList {
    cmds: Vec<DrawCmd>,
    sorted: Vec<DrawCmd>,
    counts: Vec<u32>,
}

/// Commands reserved for up front (§1.5).
pub const RESERVE: usize = 4096;

impl Default for DrawList {
    fn default() -> Self {
        DrawList { cmds: Vec::with_capacity(RESERVE), sorted: Vec::with_capacity(RESERVE), counts: Vec::new() }
    }
}

impl DrawList {
    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    pub fn push(&mut self, c: DrawCmd) {
        self.cmds.push(c);
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// Sorts by `(y, key)`: a counting sort on `y` over `y0..y0 + rows` (anything outside
    /// is held to the nearest end bucket), then each bucket put in `key` order by insertion.
    /// Returns the list in draw order.
    pub fn sort(&mut self, y0: i32, rows: u32) -> &[DrawCmd] {
        let n = rows.max(1) as usize;
        let bucket = |y: i32| (y - y0).clamp(0, n as i32 - 1) as usize;
        self.counts.clear();
        self.counts.resize(n + 1, 0);
        for c in &self.cmds {
            self.counts[bucket(c.y) + 1] += 1;
        }
        for i in 0..n {
            self.counts[i + 1] += self.counts[i];
        }
        self.sorted.clear();
        self.sorted.resize(self.cmds.len(), DrawCmd { y: 0, key: 0, sprite: EMPTY, caster: None });
        for c in &self.cmds {
            let b = bucket(c.y);
            self.sorted[self.counts[b] as usize] = *c;
            self.counts[b] += 1;
        }
        // Each bucket now spans counts[b - 1]..counts[b]; within one, order by (y, key). Buckets
        // hold a handful, so insertion is linear in practice. Clamped ends may mix rows.
        let mut start = 0;
        for b in 0..n {
            let end = self.counts[b] as usize;
            let run = &mut self.sorted[start..end];
            for i in 1..run.len() {
                let mut j = i;
                while j > 0 && (run[j - 1].y, run[j - 1].key) > (run[j].y, run[j].key) {
                    run.swap(j - 1, j);
                    j -= 1;
                }
            }
            start = end;
        }
        &self.sorted
    }
}

const EMPTY: SpriteCmd = SpriteCmd {
    page: 0,
    src: crate::frame::Src { x: 0, y: 0, w: 0, h: 0 },
    x: 0,
    y: 0,
    flags: crate::frame::Flags { mirror: false, tint: crate::frame::Tint::None, bend: crate::frame::Bend::NONE },
    height_px: 0,
    foot: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(y: i32, key: u32) -> DrawCmd {
        DrawCmd { y, key, sprite: SpriteCmd { x: key as i16, ..EMPTY }, caster: None }
    }

    fn order(l: &mut DrawList, y0: i32, rows: u32) -> Vec<(i32, u32)> {
        l.sort(y0, rows).iter().map(|c| (c.y, c.key)).collect()
    }

    #[test]
    fn it_sorts_by_y_then_key_whatever_the_push_order() {
        let set = [(30, 5), (10, 9), (30, 2), (20, 1), (10, 3), (30, 7), (-4, 8), (500, 6), (499, 4)];
        let mut want: Vec<(i32, u32)> = set.to_vec();
        want.sort();
        // Every rotation and the reverse push the same set: one order out.
        for r in 0..set.len() {
            let mut l = DrawList::default();
            let mut s = set.to_vec();
            s.rotate_left(r);
            if r % 2 == 1 {
                s.reverse();
            }
            for &(y, k) in &s {
                l.push(cmd(y, k));
            }
            // Rows 0..432: -4 is held to the first bucket, 499 and 500 to the last; still in order.
            assert_eq!(order(&mut l, 0, 432), want, "rotation {r}");
        }
    }

    #[test]
    fn equal_keys_keep_push_order() {
        let mut l = DrawList::default();
        l.push(DrawCmd { y: 5, key: 1, sprite: SpriteCmd { x: 100, ..EMPTY }, caster: None });
        l.push(DrawCmd { y: 5, key: 1, sprite: SpriteCmd { x: 200, ..EMPTY }, caster: None });
        let xs: Vec<i16> = l.sort(0, 10).iter().map(|c| c.sprite.x).collect();
        assert_eq!(xs, [100, 200]);
    }

    #[test]
    fn sorting_again_reuses_its_buffers() {
        let mut l = DrawList::default();
        for i in 0..1000 {
            l.push(cmd((i * 37) % 400, i as u32));
        }
        l.sort(0, 432);
        let caps = (l.cmds.capacity(), l.sorted.capacity(), l.counts.capacity());
        l.clear();
        for i in 0..1000 {
            l.push(cmd((i * 53) % 400, i as u32));
        }
        let got = order(&mut l, 0, 432);
        assert!(got.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(caps, (l.cmds.capacity(), l.sorted.capacity(), l.counts.capacity()));
    }
}
