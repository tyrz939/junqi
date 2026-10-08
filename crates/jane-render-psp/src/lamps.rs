//! The static lamps' shadow cache (PORT.md §13.12, owner 2026-10-08: every light casts). A light
//! that stands still (a lamp, a fire, a lit window: the same ground point, height and reach as
//! the frame before) has its pool drawn once into a small texture with the shadows of what stands
//! still round it (the terrain's blocks, props, buildings) already taken out of it; each frame the
//! GE lays that texture into the lightmap at the light's flickered colour, bilinear (the
//! shadows' edges soft), and only what moves (people, creatures) is cast per frame. The textures
//! sit in VRAM, [`SLOTS`] of them, least recently used first reused; one is built a frame at most
//! and is built again when what stands round the light changes (a checksum of it). Pure and
//! integer: the GE glue uploads what [`LampCache::uploads`] holds.

use alloc::vec;
use alloc::vec::Vec;

use jane_present::shadow::{Lamp, SUB, Slab};

/// A cached pool's side, texels (`T8`, a grey ramp CLUT: the texel is the pool's strength).
pub const TEX: usize = 128;
/// Cached pools held: 16 KB each, 256 KB of VRAM.
pub const SLOTS: usize = 16;

/// Draws a cached pool is kept at least before what stands round it is looked at again.
pub const REBUILD_AFTER: u32 = 30;

/// A light by where it stands in the zone: ground point (zone px), height, reach.
pub type Key = (i32, i32, u8, u16);

#[derive(Clone, Copy, Debug)]
struct Entry {
    key: Key,
    /// What stood round it when its texture was built: a checksum and how many.
    sum: u32,
    count: u32,
    /// The draw it was last used in, and last built in.
    last: u32,
    built: u32,
    /// Built (uploaded) at all.
    ready: bool,
}

/// What a light gets this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Its cached texture in `slot`, texels `cell` px wide.
    Cached { slot: u16, cell: i32 },
    /// Build it into `slot` now (texels `cell` px wide), then use it.
    Build { slot: u16, cell: i32 },
    /// Draw it as a plain pool with every shadow cast this frame (it moves, or the frame has
    /// built its one texture already, or the cache is full of this frame's).
    Plain,
}

/// The cache's bookkeeping, the textures built this frame and the builder's scratch.
#[derive(Debug)]
pub struct LampCache {
    slots: Vec<Option<Entry>>,
    /// Lights seen last frame and this one: a light seen again where it was stands still.
    seen: Vec<Key>,
    seen_now: Vec<Key>,
    clock: u32,
    built_now: u32,
    /// Textures built this frame: `(slot, TEX * TEX bytes)`, for the GE to copy to VRAM.
    pub uploads: Vec<(u16, Vec<u8>)>,
    /// Builds since the start (a stat), and this frame's.
    pub builds: u32,
    /// Most textures built a frame.
    pub per_frame: u32,
    grid: Vec<bool>,
}

impl Default for LampCache {
    fn default() -> Self {
        LampCache::new(SLOTS)
    }
}

/// A light's texel size: the smallest of 4, 8 and 16 px that fits its reach in [`TEX`] texels.
pub fn cell_of(radius: u16) -> i32 {
    let need = 2 * i32::from(radius) + 2;
    [2, 4, 8].into_iter().find(|&c| c * TEX as i32 >= need).unwrap_or(8)
}

impl LampCache {
    pub fn new(slots: usize) -> LampCache {
        LampCache {
            slots: vec![None; slots],
            seen: Vec::new(),
            seen_now: Vec::new(),
            clock: 0,
            built_now: 0,
            uploads: Vec::new(),
            builds: 0,
            per_frame: 1,
            grid: vec![false; TEX * TEX],
        }
    }

    /// A new frame: last frame's lights are the ones that may stand still now.
    pub fn begin(&mut self) {
        core::mem::swap(&mut self.seen, &mut self.seen_now);
        self.seen_now.clear();
        self.clock = self.clock.wrapping_add(1);
        self.built_now = 0;
        self.uploads.clear();
    }

    /// Cached pools ready now (a stat).
    pub fn held(&self) -> usize {
        self.slots.iter().flatten().filter(|e| e.ready).count()
    }

    /// Light `key`'s cached pool if it is ready and was built lately (within [`REBUILD_AFTER`]):
    /// what stands round it need not be summed this frame.
    pub fn fresh(&mut self, key: Key) -> Option<Use> {
        let clock = self.clock;
        let e = self.slots.iter_mut().flatten().find(|e| e.key == key && e.ready)?;
        if clock.wrapping_sub(e.built) >= REBUILD_AFTER {
            return None;
        }
        e.last = clock;
        self.seen_now.push(key);
        let slot = self.slots.iter().position(|x| x.is_some_and(|x| x.key == key))? as u16;
        Some(Use::Cached { slot, cell: cell_of(key.3) })
    }

    /// What light `key` gets, `count` things standing round it summing to `sum`. Built again when
    /// more stand round it than when it was built (a chunk painted near it, a prop set down), or
    /// as many but others; fewer is the frame's culling (what stood there went off its band) and
    /// keeps the texture.
    pub fn lookup(&mut self, key: Key, sum: u32, count: u32) -> Use {
        self.seen_now.push(key);
        let cell = cell_of(key.3);
        if let Some(i) = self.slots.iter().position(|e| e.is_some_and(|e| e.key == key)) {
            let e = self.slots[i].as_mut().expect("found");
            e.last = self.clock;
            // What stands round it as it was, or fewer (culled), or built lately (a moving
            // camera's band brings things in and out a frame at a time): as it is.
            let fresh = self.clock.wrapping_sub(e.built) < REBUILD_AFTER;
            if e.ready && (count < e.count || count == e.count && sum == e.sum || fresh) {
                return Use::Cached { slot: i as u16, cell };
            }
            if self.built_now < self.per_frame {
                self.built_now += 1;
                (e.sum, e.count, e.built) = (sum, count, self.clock);
                return Use::Build { slot: i as u16, cell };
            }
            if e.ready {
                // Stale a frame or two, until its turn: better than none.
                return Use::Cached { slot: i as u16, cell };
            }
            return Use::Plain;
        }
        // New to the cache: only a light that stood here last frame too, and one a frame.
        if !self.seen.contains(&key) || self.built_now >= self.per_frame {
            return Use::Plain;
        }
        let clock = self.clock;
        let free = self.slots.iter().position(Option::is_none).or_else(|| {
            (0..self.slots.len())
                .filter(|&i| self.slots[i].is_some_and(|e| e.last != clock))
                .min_by_key(|&i| self.slots[i].map_or(0, |e| e.last))
        });
        let Some(i) = free else {
            return Use::Plain;
        };
        self.slots[i] = Some(Entry { key, sum, count, last: self.clock, built: self.clock, ready: false });
        self.built_now += 1;
        Use::Build { slot: i as u16, cell }
    }

    /// Builds the texture for slot `slot`: a light at canvas px `(lx, ly)` of reach `radius`, its
    /// texels `cell` px, the pool's falloff ([`crate::light::falloff_at`]) where nothing stands in
    /// its way and its bounce (`shadow::LAMP_BOUNCE`) in the `slabs`' shadows (quads, corners in
    /// [`SUB`] steps of canvas px, `a0, b0, a1, b1`).
    pub fn build(&mut self, slot: u16, (lx, ly): (i32, i32), radius: u16, cell: i32, slabs: &[Slab]) {
        let half = cell * TEX as i32 / 2;
        let (ox, oy) = ((lx - half) * SUB, (ly - half) * SUB);
        let step = cell * SUB;
        self.grid.fill(false);
        // Each quad filled a texel row at a time by its middles.
        for q in slabs {
            let pts = [q.c[0], q.c[1], q.c[3], q.c[2]].map(|(x, y, _)| (x - ox, y - oy));
            let (y0, y1) = pts.iter().fold((i32::MAX, i32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
            let r0 = ((y0 - step / 2).div_euclid(step) + 1).max(0);
            let r1 = ((y1 - step / 2).div_euclid(step)).min(TEX as i32 - 1);
            for row in r0..=r1 {
                let yc = row * step + step / 2;
                let mut xs = [0i32; 4];
                let mut n = 0;
                for i in 0..4 {
                    let ((xa, ya), (xb, yb)) = (pts[i], pts[(i + 1) % 4]);
                    if (ya > yc) != (yb > yc) {
                        xs[n] = xa + (i64::from(xb - xa) * i64::from(yc - ya) / i64::from(yb - ya)) as i32;
                        n += 1;
                    }
                }
                xs[..n].sort_unstable();
                for pair in xs[..n].chunks(2) {
                    if let [a, b] = *pair {
                        let c0 = ((a - step / 2).div_euclid(step) + 1).max(0);
                        let c1 = ((b - step / 2).div_euclid(step)).min(TEX as i32 - 1);
                        for c in c0..=c1 {
                            self.grid[row as usize * TEX + c as usize] = true;
                        }
                    }
                }
            }
        }
        let r2 = (i32::from(radius) * i32::from(radius)).max(1) as u32;
        let bounce = jane_present::shadow::LAMP_BOUNCE;
        let mut tex = vec![0u8; TEX * TEX];
        // The falloff is the same in each quarter (a texel's middle is as far from the light as
        // its mirror's, the cell being even): worked out for one quarter, a division a texel,
        // and read back mirrored for the rest. `u16::MAX` past the reach (the falloff's table is
        // `u16` below it).
        let falloff = |i: i32, j: i32| -> u16 {
            let (dx, dy) = (i * cell + cell / 2 - half, j * cell + cell / 2 - half);
            let d2 = (dx * dx + dy * dy) as u32;
            if d2 >= r2 { u16::MAX } else { crate::light::falloff_at(d2 * 255 / r2) as u16 }
        };
        let q = TEX / 2;
        let mut quarter = [0u16; (TEX / 2) * (TEX / 2)];
        let even = cell % 2 == 0;
        if even {
            for j in 0..q {
                for i in 0..q {
                    quarter[j * q + i] = falloff(i as i32, j as i32);
                }
            }
        }
        for j in 0..TEX {
            for i in 0..TEX {
                let k = if even {
                    quarter[j.min(TEX - 1 - j) * q + i.min(TEX - 1 - i)]
                } else {
                    falloff(i as i32, j as i32)
                };
                if k == u16::MAX {
                    continue;
                }
                let k = u32::from(k);
                let k = if self.grid[j * TEX + i] { k * bounce / 256 } else { k };
                tex[j * TEX + i] = (k * 255 / 256).min(255) as u8;
            }
        }
        if let Some(Some(e)) = self.slots.get_mut(usize::from(slot)) {
            e.ready = true;
        }
        self.uploads.push((slot, tex));
        self.builds += 1;
    }
}

/// Whether `lamp` is in reach of a point `(x, y)` canvas px for a cached pool (its texture's
/// square).
pub fn in_reach(lamp: &Lamp, cell: i32, (x, y): (i32, i32)) -> bool {
    let half = cell * TEX as i32 / 2 * SUB;
    (x * SUB - lamp.x).abs() <= half && (y * SUB - lamp.y).abs() <= half
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pool's texels as they were worked out before the quarters were mirrored: each texel's
    /// own distance, its own division.
    fn plain_pool(radius: u16, cell: i32, shaded: &[bool]) -> Vec<u8> {
        let half = cell * TEX as i32 / 2;
        let r2 = (i32::from(radius) * i32::from(radius)).max(1) as u32;
        let bounce = jane_present::shadow::LAMP_BOUNCE;
        let mut tex = vec![0u8; TEX * TEX];
        for j in 0..TEX as i32 {
            let dy = j * cell + cell / 2 - half;
            for i in 0..TEX as i32 {
                let dx = i * cell + cell / 2 - half;
                let d2 = (dx * dx + dy * dy) as u32;
                if d2 >= r2 {
                    continue;
                }
                let k = crate::light::falloff_at(d2 * 255 / r2);
                let k = if shaded[j as usize * TEX + i as usize] { k * bounce / 256 } else { k };
                tex[j as usize * TEX + i as usize] = (k * 255 / 256).min(255) as u8;
            }
        }
        tex
    }

    #[test]
    fn a_mirrored_pool_is_the_pool_texel_for_texel() {
        let mut c = LampCache::new(1);
        // A shadow across the middle, so the bounce is in it too.
        let slab =
            jane_present::shadow::Slab { c: [(1500, 1400, 40), (2100, 1400, 40), (1500, 2400, 40), (2100, 2400, 40)] };
        for radius in [6u16, 40, 90, 127, 200, 255] {
            let cell = cell_of(radius);
            c.build(0, (100, 100), radius, cell, &[slab]);
            let (_, tex) = c.uploads.pop().expect("built");
            assert_eq!(tex, plain_pool(radius, cell, &c.grid), "radius {radius} cell {cell}");
        }
    }

    #[test]
    fn a_light_is_cached_once_it_stands_still_and_built_one_a_frame() {
        let mut c = LampCache::new(4);
        let (a, b) = ((10, 10, 20, 136), (50, 10, 20, 64));
        c.begin();
        assert_eq!(c.lookup(a, 1, 5), Use::Plain, "first seen: it may be moving");
        assert_eq!(c.lookup(b, 1, 5), Use::Plain);
        c.begin();
        assert_eq!(c.lookup(a, 1, 5), Use::Build { slot: 0, cell: 4 });
        assert_eq!(c.lookup(b, 1, 5), Use::Plain, "one build a frame");
        c.build(0, (0, 0), 136, 4, &[]);
        assert_eq!(c.uploads.len(), 1);
        c.begin();
        assert_eq!(c.lookup(a, 1, 5), Use::Cached { slot: 0, cell: 4 });
        assert!(matches!(c.lookup(b, 1, 5), Use::Build { .. }));
        c.begin();
        assert_eq!(c.lookup(a, 7, 4), Use::Cached { slot: 0, cell: 4 }, "fewer: culled, kept");
        assert_eq!(c.lookup(a, 2, 6), Use::Cached { slot: 0, cell: 4 }, "built lately: kept");
        for _ in 0..REBUILD_AFTER {
            c.begin();
        }
        assert!(matches!(c.lookup(a, 2, 6), Use::Build { slot: 0, .. }), "more round it");
    }

    #[test]
    fn a_slab_takes_its_texels_down_to_the_bounce() {
        let mut c = LampCache::new(1);
        // A quad over the texels east of the light, from 8 to 40 px, 4 px each side of its row.
        let q = Slab {
            c: [(8 * SUB, -4 * SUB, 0), (40 * SUB, -4 * SUB, 0), (8 * SUB, 4 * SUB, 0), (40 * SUB, 4 * SUB, 0)],
        };
        c.build(0, (0, 0), 100, 4, &[q]);
        let t = &c.uploads[0].1;
        let at = |x: i32, y: i32| t[((y + 256) / 4) as usize * TEX + ((x + 256) / 4) as usize];
        assert!(at(-20, 0) > 100, "west of the light: lit");
        assert!(at(20, 0) < 40 && at(-20, 0) > 150, "in the shadow: its bounce");
        assert_eq!(at(200, 200), 0, "past its reach: nothing");
    }
}
