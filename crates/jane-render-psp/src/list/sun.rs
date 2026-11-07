//! The sun's shadows on C2 (`soft`'s method, `jane_present::shadow`): the casters' rows kept
//! from frame to frame (`RowsKey`, `KeptRows`, `rows_of`, the opacity masks they are read from),
//! the pre-cull of casters with no band on the canvas (`sun_off_canvas`), blocks swept along the
//! sun into polygons (`block_poly`, `sweep_hull`), and `silhouettes`, which lays them all through
//! the stencil. `silhouettes` is one long fn and is left whole.

use alloc::vec::Vec;

use jane_present::{Frame, SpriteCmd};

use super::{Lister, Mode, PagePx, Quad, SUN_POLYS_MOST, SUN_QUADS_MOST, Target, Tex, rgba};
use crate::capture::pass as P;

/// What a caster's rows (`shadow::rows`) depend on: its sprite's rect and page, mirror and
/// bend, its foot's row under the sprite's top, the rows it burns and its height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RowsKey {
    src: (u16, u16, u16, u16, u8),
    mirror: bool,
    bend: u32,
    foot: i16,
    burn: (u8, u8),
    height: u8,
}

/// A caster's sun bands kept: the shear and depth they are for, and the rects.
type SunBands = ((i32, i32), u8, Vec<(i32, i32, i32, i32, u8)>, (i32, i32, i32, i32));

/// A caster's rows kept: what they depend on (its hash in `Lister::rows_hash`), the rows, the
/// draw they were last used in, and its sun bands for a shear and a depth (merged runs of rows,
/// `(x0, y0, x1, y1, level)` from its sprite's left edge and its foot row).
#[derive(Debug)]
pub(super) struct KeptRows {
    key: RowsKey,
    pub(super) rows: Vec<(i32, i32, i32)>,
    clock: u32,
    sun: Option<SunBands>,
}

impl RowsKey {
    fn hash(&self) -> u32 {
        let (x, y, w, h, p) = self.src;
        let mut v = u32::from(x) << 16 ^ u32::from(y) ^ (u32::from(w) << 8 ^ u32::from(h)).rotate_left(7);
        v = v.wrapping_mul(0x9e37_79b1) ^ u32::from(p) ^ u32::from(self.mirror) << 8 ^ self.bend.rotate_left(13);
        v = v.wrapping_mul(0x85eb_ca77)
            ^ self.foot as u32
            ^ u32::from(self.burn.0) << 16
            ^ u32::from(self.burn.1) << 24;
        v.wrapping_mul(0xc2b2_ae3d) ^ u32::from(self.height)
    }
}

/// Casters' rows kept between frames. (Measured at the PSP v0.4.1 plan's dense wood, 393,1305 at
/// 17:22 on PPSSPP: 0.45 to 1.1 rows built a frame with 105 to 180 casters, so a swaying caster's
/// `bend` in the key costs a miss or two a frame, not the plan's three; the key stays.)
const ROWS_KEPT: usize = 128;
/// Bytes of casters' opacity kept between frames.
const MASKS_KEPT: usize = 96 * 1024;

/// Block `b`'s shadow as a convex polygon: its footprint at the height its shadow starts from
/// (the ground, or a fence rail's underside) and at its top, offset along the sun `k` (Q8 px a
/// px of height), and their hull. `None` for a block too low to cast, or off a `w x h` canvas.
/// Whether a quad of mode `m` reads or writes the stencil (the framebuffer's alpha), or leaves
/// the frame (the lightmap's target), or writes the alpha on purpose: what must come after the
/// sun's pass for the frame's clear to stand for its stencil clear.
fn touches_stencil(m: Mode) -> bool {
    matches!(
        m,
        Mode::StencilClear
            | Mode::StencilMark
            | Mode::RaisedMark
            | Mode::ShadowBand
            | Mode::PoolLit
            | Mode::PoolShade
            | Mode::RtBegin
            | Mode::RtEnd
            | Mode::Lut(_)
            | Mode::Mark { .. }
            | Mode::Masked(..)
    )
}

pub(super) fn block_poly(b: &jane_present::Block, k: (i32, i32), (w, h): (i32, i32)) -> Option<([(i16, i16); 8], u8)> {
    let pts = block_sweep(b, k, (0, 0))?;
    let (mx0, my0) = pts.iter().fold((i32::MAX, i32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
    let (mx1, my1) = pts.iter().fold((i32::MIN, i32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
    if mx1 <= 0 || my1 <= 0 || mx0 >= w || my0 >= h {
        return None;
    }
    Some(sweep_hull(&pts))
}

/// [`hull`] of [`block_sweep`]'s eight points (a rect at its foot and the same rect at its top)
/// without sorting them: the hull of a rect swept along a segment is the rect plus the segment,
/// whose edges are the rect's four and the sweep's two (merged with the rect's where they lie
/// along an axis) in order of angle, from the lowest of the leftmost corners: the same corners
/// in the same order as `hull` lays them (a test holds it to that).
pub(super) fn sweep_hull(p: &[(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    let (a, b) = (p[0], p[4]);
    let (w, h) = (p[1].0 - p[0].0, p[3].1 - p[0].1);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let start = if a <= b { a } else { b };
    let (ex, ey) = match (dx, dy) {
        (_, 0) => (w + dx.abs(), h),
        (0, _) => (w, h + dy.abs()),
        _ => (w, h),
    };
    // The sweep pointing right (none along an axis: merged into the rect's edges above).
    let v = if dx == 0 || dy == 0 {
        None
    } else if dx > 0 {
        Some((dx, dy))
    } else {
        Some((-dx, -dy))
    };
    let mut edges = [(0i32, 0i32); 6];
    let mut n = 0;
    let mut edge = |e: (i32, i32)| {
        edges[n] = e;
        n += 1;
    };
    let down = v.filter(|v| v.1 < 0);
    let up = v.filter(|v| v.1 > 0);
    if let Some(v) = down {
        edge(v);
    }
    edge((ex, 0));
    if let Some(v) = up {
        edge(v);
    }
    edge((0, ey));
    if let Some(v) = down {
        edge((-v.0, -v.1));
    }
    edge((-ex, 0));
    if let Some(v) = up {
        edge((-v.0, -v.1));
    }
    edge((0, -ey));
    let mut r = [(0i16, 0i16); 8];
    let mut at = start;
    for (d, e) in r.iter_mut().zip(&edges[..n]) {
        *d = (at.0.clamp(-2048, 2047) as i16, at.1.clamp(-2048, 2047) as i16);
        at = (at.0 + e.0, at.1 + e.1);
    }
    (r, n as u8)
}

/// [`block_poly`]'s eight points before the hull, `o` px over the block's (a camera's offset:
/// zone px for a kept polygon); `None` for a block too low to cast.
pub(super) fn block_sweep(b: &jane_present::Block, (kx, ky): (i32, i32), o: (i32, i32)) -> Option<[(i32, i32); 8]> {
    use jane_present::shadow;
    let hgt = i32::from(b.height);
    let (mut x0, y0, mut x1, y1) =
        (i32::from(b.x0) + o.0, i32::from(b.y0) + o.1, i32::from(b.x1) + o.0, i32::from(b.y1) + o.1);
    if hgt <= shadow::GROUND || x0 >= x1 || y0 >= y1 {
        return None;
    }
    let lo = if shadow::spills(b) {
        // A block of the height field stands a px wider each side than what is drawn.
        if !b.fence && x1 - x0 > 2 {
            (x0, x1) = (x0 + 1, x1 - 1);
        }
        i32::from(b.lo).min(hgt)
    } else {
        0
    };
    let (ax, ay, bx, by) = ((lo * kx) >> 8, (lo * ky) >> 8, (hgt * kx) >> 8, (hgt * ky) >> 8);
    let pts = [
        (x0 + ax, y0 + ay),
        (x1 + ax, y0 + ay),
        (x1 + ax, y1 + ay),
        (x0 + ax, y1 + ay),
        (x0 + bx, y0 + by),
        (x1 + bx, y0 + by),
        (x1 + bx, y1 + by),
        (x0 + bx, y1 + by),
    ];
    Some(pts)
}

/// The convex hull of eight points, round it (Andrew's monotone chain), at most eight corners:
/// what [`sweep_hull`] is held to.
#[cfg(test)]
pub(super) fn hull(p: [(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    // Within 8192 px of the canvas's corner every cross product fits an `i32` (the MIPS has no
    // 64-bit multiply): the same answers, a third of the time.
    if p.iter().all(|q| q.0.unsigned_abs() < 8192 && q.1.unsigned_abs() < 8192) {
        hull_with::<true>(p)
    } else {
        hull_with::<false>(p)
    }
}

#[cfg(test)]
fn hull_with<const SMALL: bool>(mut p: [(i32, i32); 8]) -> ([(i16, i16); 8], u8) {
    p.sort_unstable();
    let cross = |o: (i32, i32), a: (i32, i32), b: (i32, i32)| {
        if SMALL {
            i64::from((a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0))
        } else {
            i64::from(a.0 - o.0) * i64::from(b.1 - o.1) - i64::from(a.1 - o.1) * i64::from(b.0 - o.0)
        }
    };
    let mut out = [(0i32, 0i32); 17];
    let mut n = 0usize;
    for &q in &p {
        while n >= 2 && cross(out[n - 2], out[n - 1], q) <= 0 {
            n -= 1;
        }
        out[n] = q;
        n += 1;
    }
    let lower = n + 1;
    for &q in p.iter().rev().skip(1) {
        while n >= lower && cross(out[n - 2], out[n - 1], q) <= 0 {
            n -= 1;
        }
        out[n] = q;
        n += 1;
    }
    let n = (n - 1).min(8);
    let mut r = [(0i16, 0i16); 8];
    for (d, s) in r.iter_mut().zip(&out[..n]) {
        *d = (s.0.clamp(-2048, 2047) as i16, s.1.clamp(-2048, 2047) as i16);
    }
    (r, n as u8)
}

/// Whether every sun band caster `c` can lay lies off the `(w, h)` canvas, from its sprite's
/// columns (and its sway's reach), its height and depth and shear `k` alone (`shadow::bands`'
/// geometry, bounded over every row up to its height, 4 px slack each way), without its rows.
pub(super) fn sun_off_canvas(
    frame: &Frame,
    c: &jane_present::Caster,
    (kx, ky): (i32, i32),
    (w, h): (i32, i32),
) -> bool {
    let Some(s) = frame.sprites.get(c.sprite as usize) else { return false };
    let tall = i32::from(c.height).max(1);
    let (sx, sy) = ((tall * kx) >> 8, (tall * ky) >> 8);
    let r = s.flags.bend.reach() + 4;
    let x0 = i32::from(s.x) - r + sx.min(0);
    let x1 = i32::from(s.x) + i32::from(s.src.w) + r + sx.max(0);
    let fy = jane_present::shadow::foot_y(c);
    let y0 = fy + sy.min(0) - i32::from(c.depth).max(2) - 4;
    let y1 = fy + sy.max(0) + 4;
    x1 <= 0 || x0 >= w || y1 <= 0 || y0 >= h
}

impl Lister {
    /// Caster `c`'s rows (`shadow::rows`) from its pack page, kept from frame to frame while its
    /// px and where it stands over its foot hold (a prop, a house; a walking unit's frame
    /// changes): their index in `rows_kept`, and its sprite's left edge.
    pub(super) fn rows_of(
        &mut self,
        frame: &Frame,
        c: &jane_present::Caster,
        px: &mut dyn PagePx,
    ) -> Option<(usize, i32)> {
        use jane_present::shadow;
        let s = frame.sprites.get(c.sprite as usize)?;
        let i = self.find(s.page, s.src.x, s.src.y)?;
        let t = self.targets[i]?;
        let (rx, ry) = self.rects[i];
        let key = RowsKey {
            src: (s.src.x, s.src.y, s.src.w, s.src.h, s.page),
            mirror: s.flags.mirror,
            bend: s.flags.bend.packed(),
            foot: c.foot.1 - s.y,
            burn: c.burn,
            height: c.height,
        };
        self.rows_clock = self.rows_clock.wrapping_add(1);
        let hash = key.hash();
        let mut at = 0;
        while let Some(o) = self.rows_hash[at..].iter().position(|&h| h == hash) {
            let e = at + o;
            if self.rows_kept[e].key != key {
                at = e + 1;
                continue;
            }
            self.rows_kept[e].clock = self.rows_clock;
            return Some((e, i32::from(s.x)));
        }
        // The sprite's px in the presenter's rect, from its pack page: 2 opaque, 1 the contact
        // shadow, 0 clear (all `shadow::rows` asks).
        let (sw, sh) = (usize::from(s.src.w), usize::from(s.src.h));
        let (lx0, ly0) = (i32::from(s.src.x) - i32::from(rx), i32::from(s.src.y) - i32::from(ry));
        let mask = self.opacity(i, t, px);
        let tw = usize::from(t.w);
        self.caster_px.clear();
        self.caster_px.resize(sw * sh, 0);
        for v in 0..sh as i32 {
            let ty = ly0 + v - i32::from(t.ty);
            if ty < 0 || ty >= i32::from(t.h) {
                continue;
            }
            for u in 0..sw as i32 {
                let tx = lx0 + u - i32::from(t.tx);
                if tx < 0 || tx >= i32::from(t.w) {
                    continue;
                }
                let k = ty as usize * tw + tx as usize;
                let o = (self.masks[mask].1[k / 4] >> ((k % 4) * 2)) & 3;
                self.caster_px[v as usize * sw + u as usize] = u16::from(o);
            }
        }
        let local = SpriteCmd { src: jane_present::Src { x: 0, y: 0, w: s.src.w, h: s.src.h }, ..*s };
        let mut rows = Vec::new();
        shadow::rows(&self.caster_px, s.src.w, &local, c, &mut rows);
        if self.rows_kept.len() >= ROWS_KEPT {
            let old = (0..self.rows_kept.len()).min_by_key(|&e| self.rows_kept[e].clock).unwrap_or(0);
            self.rows_kept.swap_remove(old);
            self.rows_hash.swap_remove(old);
        }
        self.rows_kept.push(KeptRows { key, rows, clock: self.rows_clock, sun: None });
        self.rows_hash.push(hash);
        self.prof[2] += 1;
        Some((self.rows_kept.len() - 1, i32::from(s.x)))
    }

    /// The sun's silhouettes (`soft`'s method, `jane_present::shadow`): each caster's opaque rows
    /// read from its pack page (kept while they hold), sheared along the sun into bands, and each
    /// block of the terrain swept by its height. The GE lays them with its stencil (the
    /// framebuffer's alpha): cleared, then raised terrain marked from each chunk's height layer
    /// (it keeps its light), then the bands strongest first, each px shaded once toward `shade`
    /// by the strongest band over it, as `soft`'s mask takes the most. Strengths in sixteenths.
    pub(super) fn silhouettes(
        &mut self,
        frame: &Frame,
        sun: &jane_present::Directional,
        shade: jane_present::Rgb,
        (casters, blocks): (jane_present::Span, jane_present::Span),
        px: &mut dyn PagePx,
    ) {
        use jane_present::shadow;
        let Some(k) = shadow::shear(sun) else { return };
        let (w, h) = (self.w, self.h);
        let mut levels = core::mem::take(&mut self.shadow_levels);
        levels.resize_with(16, Vec::new);
        for l in &mut levels {
            l.clear();
        }
        let mut band = |b: shadow::Band| {
            let (x0, x1, y0, y1) = (b.x0.max(0), b.x1.min(w), b.y0.max(0), b.y1.min(h));
            if x0 < x1 && y0 < y1 && b.strength >= 16 {
                let (x0, y0, x1, y1) = (x0 as i16, y0 as i16, x1 as i16, y1 as i16);
                let l = &mut levels[usize::from(b.strength >> 4)];
                // A band just under the last of its strength, as wide (a fence's rows, a
                // column's), or just beside it, as tall: one rect.
                match l.last_mut() {
                    Some(r) if r.0 == x0 && r.2 == x1 && (r.3 == y0 || r.1 == y1) => {
                        (r.1, r.3) = (r.1.min(y0), r.3.max(y1));
                    }
                    Some(r) if r.1 == y0 && r.3 == y1 && (r.2 == x0 || r.0 == x1) => {
                        (r.0, r.2) = (r.0.min(x0), r.2.max(x1));
                    }
                    _ => l.push((x0, y0, x1, y1)),
                }
            }
        };
        let t_c = self.now();
        for c in frame.casters_in(casters) {
            // Wholly off the canvas by its height alone (half the wood's casters, which the
            // presenter sends for shadows that may come in): no rows looked up for it.
            let off = sun_off_canvas(frame, c, k, (w, h));
            if off && !cfg!(test) {
                continue;
            }
            let Some((e, x)) = self.rows_of(frame, c, px) else { continue };
            // Its bands from its own rows once for this sun (they move with it, so are kept from
            // its left edge and foot row), runs of rows merged; then laid where it stands.
            let kept = &mut self.rows_kept[e];
            if kept.sun.as_ref().is_none_or(|s| s.0 != k || s.1 != c.depth) {
                let mut rects: Vec<(i32, i32, i32, i32, u8)> = Vec::new();
                let at_home = jane_present::Caster { foot: (c.foot.0, 0), ..*c };
                shadow::bands(&kept.rows, 0, &at_home, k, |b| {
                    if b.strength < 16 {
                        return;
                    }
                    let lv = b.strength >> 4;
                    // A row within a px of the run's each side joins it (C2: a crown's ragged edge
                    // a px coarser, a third of the rects).
                    match rects.iter_mut().rev().find(|r| r.4 == lv) {
                        Some(r) if (r.0 - b.x0).abs() <= 1 && (r.2 - b.x1).abs() <= 1 && b.y0 <= r.3 && b.y1 >= r.1 => {
                            (r.0, r.1, r.2, r.3) = (r.0.min(b.x0), r.1.min(b.y0), r.2.max(b.x1), r.3.max(b.y1));
                        }
                        _ => rects.push((b.x0, b.y0, b.x1, b.y1, lv)),
                    }
                });
                let bounds = rects.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |a, r| {
                    (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3))
                });
                kept.sun = Some((k, c.depth, rects, bounds));
            }
            let fy = i32::from(c.foot.1);
            if let Some((_, _, rects, b)) = &self.rows_kept[e].sun {
                // Wholly off the canvas: every band of it would be clipped away.
                if b.2 + x <= 0 || b.0 + x >= w || b.3 + fy <= 0 || b.1 + fy >= h {
                    continue;
                }
                // The bound holds: what it culls has no band on the canvas.
                assert!(!(cfg!(test) && off), "sun_off_canvas culled a caster with bands on the canvas");
                for &(x0, y0, x1, y1, lv) in rects {
                    band(shadow::Band {
                        x0: x0 + x,
                        y0: y0 + fy,
                        x1: x1 + x,
                        y1: y1 + fy,
                        strength: lv << 4,
                        reach: 1,
                    });
                }
            }
        }
        self.prof[8] += self.now().wrapping_sub(t_c);
        // The terrain's blocks: each its footprint swept along the sun over its height, a
        // convex polygon at full strength (where `soft` lays the same sweep a row at a time).
        let mut polys = core::mem::take(&mut self.polys);
        let t_b = self.now();
        for b in frame.blocks_in(blocks) {
            if let Some(p) = block_poly(b, k, (w, h)) {
                // A frame's polygons are held ([`SUN_POLYS_MOST`]): the rest cast no shadow.
                if polys.len() >= SUN_POLYS_MOST {
                    self.polys_skipped += 1;
                    continue;
                }
                polys.push(p);
            }
        }
        self.prof[9] += self.now().wrapping_sub(t_b);
        if levels.iter().all(Vec::is_empty) && polys.is_empty() {
            self.polys = polys;
            self.shadow_levels = levels;
            return;
        }
        let quad = |tex: Tex, mode: Mode, colour: u32, (x0, y0, x1, y1): (i16, i16, i16, i16), uv: (u16, u16)| Quad {
            tex,
            mode,
            colour,
            x0,
            y0,
            x1,
            y1,
            u0: 0,
            v0: 0,
            u1: uv.0,
            v1: uv.1,
        };
        let all = (0, 0, w as i16, h as i16);
        let first = self.quads.len();
        // The stencil clear: the frame's own `sceGuClear` when nothing drawn before this pass
        // touched the stencil (`stencil_from_clear`: the GE keeps the alpha unwritten until here),
        // else a quad. The quads the pass may lay are counted as they were with the quad and one
        // whole-chunk mark a chunk, so the bands it keeps are the same.
        let mut counted = 1;
        if self.quads.iter().any(|q| touches_stencil(q.mode)) {
            self.sub_mark(P::SUN_CLEAR);
            self.quads.push(quad(Tex::None, Mode::StencilClear, 0xff00_0000, all, (0, 0)));
        } else {
            self.stencil_from_clear = Some(first as u32);
        }
        self.sub_mark(P::SUN_RAISED);
        // Raised terrain marked where the chunk has relief alone (most cells are flat), and on
        // the canvas alone: the same texels at the same px as a whole chunk's quad marks.
        self.relief_fresh(frame);
        for k in 0..self.placed.len() {
            let (cx, cy, slot) = self.placed[k];
            if !frame.layers.get(usize::from(slot)).is_some_and(jane_present::ChunkLayers::has_height) {
                continue;
            }
            counted += 1;
            let Some(Some((_, rects))) = self.relief_rects.get(usize::from(slot)) else { continue };
            for &(rx0, ry0, rx1, ry1) in rects {
                let c = |v: u8| i32::from(v) * jane_present::frame::CELL;
                let (x0, y0) = ((cx + c(rx0)).max(0), (cy + c(ry0)).max(0));
                let (x1, y1) = ((cx + c(rx1)).min(w), (cy + c(ry1)).min(h));
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                self.quads.push(Quad {
                    tex: Tex::Height(slot),
                    mode: Mode::RaisedMark,
                    colour: 0xffff_ffff,
                    x0: x0 as i16,
                    y0: y0 as i16,
                    x1: x1 as i16,
                    y1: y1 as i16,
                    u0: (x0 - cx) as u16,
                    v0: (y0 - cy) as u16,
                    u1: (x1 - cx) as u16,
                    v1: (y1 - cy) as u16,
                });
            }
        }
        self.sub_mark(P::SUN_POLYS);
        let full = rgba(shadow::shade_at(shade, 255), 255);
        for (i, (pts, n)) in polys.iter().enumerate() {
            let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
            for &(x, y) in &pts[..usize::from(*n)] {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            self.quads.push(quad(Tex::Poly(i as u16), Mode::ShadowBand, full, (x0, y0, x1, y1), (0, 0)));
            self.shadow_runs += 1;
            counted += 1;
        }
        self.polys = polys;
        // The bands strongest first, as many as the pass has room for: what it must leave out is
        // the faintest.
        self.sub_mark(P::SUN_BANDS);
        let mut room = SUN_QUADS_MOST.saturating_sub(counted);
        for (lv, rects) in levels.iter().enumerate().rev() {
            let tint = rgba(shadow::shade_at(shade, (lv << 4 | lv) as u8), 255);
            for &r in rects {
                if room == 0 {
                    self.bands_skipped += 1;
                    continue;
                }
                room -= 1;
                self.quads.push(quad(Tex::None, Mode::ShadowBand, tint, r, (0, 0)));
                self.shadow_runs += 1;
            }
        }
        self.shadow_levels = levels;
    }

    /// Ref `i`'s opacity on its trimmed rect (2 bits a px: 0 clear, 1 the contact shadow, 2
    /// opaque), read from its pack page once and kept (the least recently used goes past
    /// [`MASKS_KEPT`] bytes). Its index in `masks`.
    fn opacity(&mut self, i: usize, t: Target, px: &mut dyn PagePx) -> usize {
        self.mask_clock = self.mask_clock.wrapping_add(1);
        if let Some(m) = self.masks.iter().position(|m| m.0 == i) {
            self.masks[m].2 = self.mask_clock;
            return m;
        }
        let (w, h) = (usize::from(t.w), usize::from(t.h));
        let mut bits = alloc::vec![0u8; (w * h).div_ceil(4)];
        for y in 0..h {
            for x in 0..w {
                let o = px.texel(t.page, t.u + x as u16, t.v + y as u16).min(2);
                let k = y * w + x;
                bits[k / 4] |= o << ((k % 4) * 2);
            }
        }
        let mut held: usize = self.masks.iter().map(|m| m.1.len()).sum::<usize>() + bits.len();
        while held > MASKS_KEPT && !self.masks.is_empty() {
            let old = (0..self.masks.len()).min_by_key(|&m| self.masks[m].2).unwrap_or(0);
            held -= self.masks[old].1.len();
            self.masks.swap_remove(old);
        }
        self.masks.push((i, bits, self.mask_clock));
        self.masks.len() - 1
    }
}
