//! T0's point-light shadows (PRESENTATION.md §1.3 `shadows`, §1.7): for each light the frame
//! says casts (the nearest few, `Features::shadows`), every caster's and every block's slabs
//! (`jane_present::shadow::{row_slabs, block_slabs}`, the geometry T1 draws) rasterised into a
//! reach mask of its own, how high the light's shadow stands over each ground px. Laid under the
//! standing things, as the sun's silhouettes are: a px of the ground or of the terrain whose
//! ground point lies lower than a light's shadow there loses that light's pool, taken off in the
//! ratio of the lightmap with and without it (`LightMap::corners`), so the light pass then
//! lights it by the rest. Integer only, like every pixel `soft` writes.

use jane_present::shadow::{self, FRONT, GROUND, Lamp, SUB, Slab};
use jane_present::{Frame, Light, Page, rows_up};

use crate::blit::Target;
use crate::lightmap::{LightMap, OWN};

/// `(x0, y0, x1, y1)`, canvas px.
type Rect = (i32, i32, i32, i32);

/// A light that casts as the masks are built: its slot, its geometry, what holds it, and the
/// box its mask was written in.
type Casting = (usize, Lamp, Option<u32>, Option<Rect>);

/// Rows over a mask's box a lifted receiver may stand and still take its shadow from inside it.
const UP: i32 = rows_up(100) + FRONT;

/// The masks, a byte a casting light in each px, the canvas's size, and the box each was
/// written in.
#[derive(Debug, Default)]
pub struct PointShadows {
    reach: Vec<[u8; OWN]>,
    w: i32,
    h: i32,
    /// Per light that casts: its mask's slot, and the box written.
    lights: Vec<(usize, Option<Rect>)>,
    rows: Vec<(i32, i32, i32)>,
}

impl PointShadows {
    fn fit(&mut self, w: i32, h: i32) {
        let n = (w * h) as usize;
        if self.reach.len() != n {
            self.reach.clear();
            self.reach.resize(n, [0; OWN]);
        }
        (self.w, self.h) = (w, h);
    }

    /// Builds the mask of each light in `points` that casts and the lightmap keeps apart: the
    /// shadows of `frame`'s casters `casters` (their silhouettes' rows from `pages`) and blocks
    /// `blocks`. Returns whether any light casts.
    pub fn build(
        &mut self,
        frame: &Frame,
        points: &[Light],
        lm: &LightMap,
        (casters, blocks): (jane_present::Span, jane_present::Span),
        pages: &[Page],
        (w, h): (i32, i32),
    ) -> bool {
        self.fit(w, h);
        self.lights.clear();
        // The lights that cast and the lightmap keeps apart: slot, geometry, holder, box written.
        let mut lamps: [Option<Casting>; OWN] = [None; OWN];
        for (li, l) in points.iter().enumerate() {
            if let Some(slot) = lm.own_slot(li) {
                lamps[slot] = Some((slot, Lamp::of(l), l.holder, None));
            }
        }
        let mut rows = std::mem::take(&mut self.rows);
        let mask = &mut self.reach;
        // Each caster's rows once, for every light near enough for it to cast.
        for c in frame.casters_in(casters) {
            // A light never shadows what holds it: her lantern's hand, a lamp's post.
            let near = |l: &Casting| l.2 != Some(c.sprite) && shadow::reaches(c, &l.1);
            if !lamps.iter().flatten().any(near) {
                continue;
            }
            let Some(s) = frame.sprites.get(c.sprite as usize) else { continue };
            let Some(page) = pages.get(usize::from(s.page)) else { continue };
            rows.clear();
            shadow::rows(&page.albedo, page.w, s, c, &mut rows);
            for l in lamps.iter_mut().flatten() {
                if near(l) {
                    let (slot, lamp, _, dirty) = l;
                    shadow::row_slabs(&rows, i32::from(s.x), c, lamp, |q| slab(mask, *slot, (w, h), &q, dirty));
                }
            }
        }
        for b in frame.blocks_in(blocks) {
            for (slot, lamp, _, dirty) in lamps.iter_mut().flatten() {
                shadow::block_slabs(b, lamp, |q| slab(mask, *slot, (w, h), &q, dirty));
            }
        }
        self.lights.extend(lamps.iter().flatten().map(|l| (l.0, l.3)));
        self.rows = rows;
        self.lights.iter().any(|l| l.1.is_some())
    }

    /// Takes each shadowed light's pool off what `t` holds (the ground and the terrain, before
    /// the standing things are drawn), each px at the ground under it (`heights`: the terrain's
    /// height under each px of `t`; empty: all ground) where a light's shadow there stands higher
    /// than it; and clears the masks. Returns pixels written.
    pub fn apply(&mut self, t: &mut Target<'_>, lm: &LightMap, heights: &[u8]) -> u64 {
        let (w, h) = (self.w, self.h);
        let Some((x0, y0, x1, y1)) = self
            .lights
            .iter()
            .filter_map(|l| l.1)
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        else {
            return 0;
        };
        let (x0, x1, y1) = (x0.max(0), x1.min(w), y1.min(h));
        let mut written = 0;
        let mut cached: Option<((i32, i32, u8), crate::lightmap::Corners)> = None;
        for y in (y0 - UP).max(0)..y1 {
            let row = (y * w) as usize;
            for x in x0..x1 {
                let i = row + x as usize;
                let lift = heights.get(i).copied().unwrap_or(0);
                let g = if i32::from(lift) <= GROUND {
                    if y < y0 {
                        continue;
                    }
                    i
                } else {
                    let gy = y + rows_up(i32::from(lift)) + FRONT;
                    if gy < y0 || gy >= y1 {
                        continue;
                    }
                    (gy * w + x) as usize
                };
                let m = self.reach[g];
                if m == [0; OWN] {
                    continue;
                }
                let gone = m.iter().enumerate().fold(0u8, |a, (k, &r)| a | u8::from(r > lift) << k);
                if gone == 0 {
                    continue;
                }
                let key = (x / 4, y / 4, gone);
                if cached.is_none_or(|c| c.0 != key) {
                    cached = Some((key, lm.corners(x, y, gone)));
                }
                let keep = LightMap::blend(&cached.as_ref().expect("just set").1, x, y);
                let d = &mut t.px[i];
                let c = *d;
                let ch = |shift: u32, k: usize| ((((c >> shift) & 0xff) * keep[k] + 128) >> 8).min(255);
                *d = 0xff00_0000 | ch(16, 0) << 16 | ch(8, 1) << 8 | ch(0, 2);
                written += 1;
            }
        }
        for y in y0.max(0)..y1 {
            let a = (y * w + x0) as usize;
            self.reach[a..a + (x1 - x0) as usize].fill([0; OWN]);
        }
        self.lights.clear();
        written
    }
}

/// Rasterises slab `q` (two triangles, `a0 b0 a1` and `b0 b1 a1`) into `mask` (`w x h`), the
/// highest reach kept, growing `dirty` by what it covers.
fn slab(mask: &mut [[u8; OWN]], slot: usize, wh: (i32, i32), q: &Slab, dirty: &mut Option<Rect>) {
    tri(mask, slot, wh, [q.c[0], q.c[1], q.c[2]], dirty);
    tri(mask, slot, wh, [q.c[1], q.c[3], q.c[2]], dirty);
}

/// One triangle, its corners in [`SUB`] steps with their reach: each px whose middle is inside
/// it takes the reach there (linear across it, as the GPU interpolates it on T1).
fn tri(mask: &mut [[u8; OWN]], slot: usize, (w, h): (i32, i32), v: [(i32, i32, i32); 3], dirty: &mut Option<Rect>) {
    let [(ax, ay, ar), (bx, by, br), (cx, cy, cr)] = v.map(|(x, y, r)| (i64::from(x), i64::from(y), i64::from(r)));
    let area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay);
    if area == 0 {
        return;
    }
    // The reach times the area, linear in the px's middle `(px, py)`: `alpha px + beta py + gamma`.
    let alpha = -(cy - by) * ar - (ay - cy) * br - (by - ay) * cr;
    let beta = (cx - bx) * ar + (ax - cx) * br + (bx - ax) * cr;
    let gamma = (bx * cy - cx * by) * ar + (cx * ay - ax * cy) * br + (ax * by - bx * ay) * cr;
    let sub = i64::from(SUB);
    let half = sub / 2;
    let (lo, hi) = (ay.min(by).min(cy), ay.max(by).max(cy));
    // Rows whose middle lies in `[lo, hi]`.
    let r0 = ((lo - half).div_euclid(sub) + i64::from((lo - half).rem_euclid(sub) != 0)).max(0);
    let r1 = (hi - half).div_euclid(sub).min(i64::from(h) - 1);
    if r0 > r1 {
        return;
    }
    // Each edge's x at the first row's middle and its step a row, in 1/65536 of a sub-px: two
    // divisions an edge, none a row.
    let yc0 = r0 * sub + half;
    let edges = [((ax, ay), (bx, by)), ((bx, by), (cx, cy)), ((cx, cy), (ax, ay))].map(|((px, py), (qx, qy))| {
        if py == qy {
            (py, qy, px.min(qx) << 16, px.max(qx) << 16, 0)
        } else {
            let x = (px << 16) + (qx - px) * (yc0 - py) * 65536 / (qy - py);
            (py.min(qy), py.max(qy), x, x, (qx - px) * sub * 65536 / (qy - py))
        }
    });
    // The reach in 1/65536 px at the middle of px `(col, row)`: `r00 + aq col + bq row`.
    let r00 = (alpha * half + beta * half + gamma) * 65536 / area;
    let (aq, bq) = (alpha * sub * 65536 / area, beta * sub * 65536 / area);
    for row in r0..=r1 {
        let yc = row * sub + half;
        let k = row - r0;
        let (mut xl, mut xr) = (i64::MAX, i64::MIN);
        for &(lo, hi, a, b, step) in &edges {
            if lo <= yc && yc <= hi {
                xl = xl.min(a + step * k);
                xr = xr.max(b + step * k);
            }
        }
        if xl > xr {
            continue;
        }
        let (xl, xr) = (xl >> 16, xr >> 16);
        let c0 = ((xl - half).div_euclid(sub) + i64::from((xl - half).rem_euclid(sub) != 0)).max(0);
        let c1 = (xr - half).div_euclid(sub).min(i64::from(w) - 1);
        if c0 > c1 {
            continue;
        }
        let base = (row * i64::from(w)) as usize;
        let mut r = r00 + aq * c0 + bq * row;
        for m in &mut mask[base + c0 as usize..=base + c1 as usize] {
            m[slot] = m[slot].max((r >> 16).clamp(0, 255) as u8);
            r += aq;
        }
        let (x0, x1, y) = (c0 as i32, c1 as i32 + 1, row as i32);
        *dirty = Some(match *dirty {
            None => (x0, y, x1, y + 1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y), c.max(x1), d.max(y + 1)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::frame::Block;
    use jane_present::{LightKind, Span};

    fn lamp(x: i32, y: i32) -> Light {
        Light {
            pos: (x, y),
            height: 30,
            colour: [255, 200, 120],
            radius: 80,
            size: 4,
            casts: true,
            kind: LightKind::Point,
            holder: None,
            base: 0,
        }
    }

    #[test]
    fn a_wall_between_a_lamp_and_the_ground_takes_its_pool_off_the_ground_behind_it() {
        let (w, h) = (160, 120);
        let mut f = Frame::new(jane_present::Tier::T0);
        f.canvas = (w as u16, h as u16);
        // A wall 60 px tall, 8 rows deep, across the middle; the lamp south of it.
        f.blocks.push(Block { x0: 40, y0: 50, x1: 120, y1: 58, height: 60, ..Block::default() });
        let lights = [lamp(80, 90)];
        let mut lm = LightMap::default();
        lm.build((w, h), [40, 40, 60], &lights, jane_present::Band::NONE);
        let mut ps = PointShadows::default();
        let spans = (Span::default(), Span { start: 0, len: 1 });
        assert!(ps.build(&f, &lights, &lm, spans, &[], (w, h)));
        let mut px = vec![0xff80_8080u32; (w * h) as usize];
        let mut t = Target { px: &mut px, w, h };
        assert!(ps.apply(&mut t, &lm, &[]) > 0);
        let at = |x: i32, y: i32| px[(y * w + x) as usize];
        // North of the wall, the pool is gone: darker than the grey by the ratio; south of it,
        // between the wall and the lamp, it is left as it was.
        assert!(at(80, 40) & 0xff < 0x80, "{:08x}", at(80, 40));
        assert_eq!(at(80, 75), 0xff80_8080);
        // Well to the side of the wall's shadow, lit.
        assert_eq!(at(10, 100), 0xff80_8080);
        // And the masks are clear for the next frame.
        assert!(ps.reach.iter().all(|&r| r == [0; OWN]));
    }
}
