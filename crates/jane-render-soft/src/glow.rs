//! T0's glow and bloom (PRESENTATION.md §1.3 `glow` and `bloom`, 2026-09-27): what the lit tiers
//! draw from the emissive layer, as far as a CPU affords it.
//!
//! **Glow.** A T0 page and chunk carry what glows sparse (`Page::glow`, `ChunkLayers::glow`). As
//! each chunk and sprite is drawn, its glowing px are noted with the colour the px was left with;
//! a later draw over a px changes that colour, and the px is dropped when the light pass comes.
//! After the lightmap the survivors take their emissive added in linear light at T2's gain, so a
//! lit window and a lamp's glass shine at night rather than being darkened with the wall.
//!
//! **Bloom.** The same px, summed into a buffer at a quarter of the canvas each way, blurred twice
//! (a narrow tent and a wide one, as T2's chain sums a close level and far ones), and added in
//! linear light before the grade where anything glows, bilinear between cells, so a window has a
//! halo and a lamp a soft heart. Only blocks near a glow are touched.
//!
//! Integer throughout: a frame is the same bytes on every target.

use jane_present::{Flags, Page, Src, Tint};

use crate::blit::Target;
use crate::grade::{S2L, to_display_fast};

/// T2's emissive gain, of 256 (`jane-render-wgpu`'s `EMISSIVE_GAIN`, 1.35).
const GAIN: u32 = 346;
/// Canvas px per bloom cell, each way.
const CELL: i32 = 4;
/// The most glowing px a frame keeps (reserved once).
const MOST: usize = 16_384;
/// A bloom fainter than this (linear light of 65535) adds under half a level even in the dark:
/// its blocks are left alone.
const FAINT: u32 = 48;
/// Cells of blur each side: the narrow tent's and the wide one's.
const NEAR: usize = 2;
const FAR: usize = 6;

/// One frame's glowing px and the bloom's buffers, reused frame to frame.
#[derive(Debug, Default)]
pub struct Glow {
    /// `(px index, emissive colour, the colour the px was drawn)`.
    px: Vec<(u32, u32, u32)>,
    /// The bloom: per cell, linear light of 65535 per channel.
    cells: Vec<[u32; 3]>,
    near: Vec<[u32; 3]>,
    tmp: Vec<[u32; 3]>,
    w: usize,
    h: usize,
}

impl Glow {
    /// A new frame: nothing glows yet.
    pub fn clear(&mut self) {
        if self.px.capacity() < MOST {
            self.px.reserve(MOST);
        }
        self.px.clear();
    }

    /// Whether anything glows this frame.
    pub fn any(&self) -> bool {
        !self.px.is_empty()
    }

    fn note(&mut self, t: &Target<'_>, x: i32, y: i32, em: u32) {
        if x < 0 || y < 0 || x >= t.w || y >= t.h || self.px.len() >= MOST {
            return;
        }
        let i = (y * t.w + x) as usize;
        self.px.push((i as u32, em, t.px[i]));
    }

    /// Notes the glowing texels of `src` of `page` (its sparse glow, sorted by index), drawn with
    /// its top-left at `(x, y)` just now. A ghost does not glow (the lit tiers draw it albedo only).
    #[allow(clippy::too_many_arguments)]
    pub fn sprite(
        &mut self,
        t: &Target<'_>,
        page: &Page,
        glow: &[(u32, u16)],
        clut: &[u32],
        src: Src,
        (x, y): (i32, i32),
        flags: Flags,
        behind: Option<(jane_present::Foot, &[u8])>,
    ) {
        if glow.is_empty() || matches!(flags.tint, Tint::Ghost(_) | Tint::Seen) || src.w == 0 || src.h == 0 {
            return;
        }
        let pw = u32::from(page.w);
        let (sx, sy, sw, sh) = (u32::from(src.x), u32::from(src.y), u32::from(src.w), u32::from(src.h));
        let first = sy * pw + sx;
        let last = (sy + sh - 1) * pw + sx + sw;
        let mut k = glow.partition_point(|g| g.0 < first);
        while k < glow.len() && glow[k].0 < last {
            let (i, e) = glow[k];
            k += 1;
            let (gx, gy) = (i % pw, i / pw);
            if gx < sx || gx >= sx + sw {
                continue;
            }
            let col = (gx - sx) as i32;
            let col = if flags.mirror { sw as i32 - 1 - col } else { col };
            let row = (gy - sy) as i32;
            let (px, py) = (x + col + flags.bend.shift(row), y + row);
            // What the terrain hides of it does not glow, seen through or not (the lit tiers draw
            // what shows through as colour alone).
            if let Some((f, hs)) = behind
                && (0..t.w).contains(&px)
                && (0..t.h).contains(&py)
                && jane_present::Foot::hides(hs[(py * t.w + px) as usize], py, i32::from(f.y))
            {
                continue;
            }
            self.note(t, px, py, clut.get(usize::from(e)).copied().unwrap_or(0));
        }
    }

    /// Notes a chunk's glowing px (`ChunkLayers::glow`), the chunk's top-left at canvas `(x, y)`;
    /// rows above `top` are the sky's and were not drawn.
    pub fn chunk(&mut self, t: &Target<'_>, glow: &[(u16, u32)], side: i32, (x, y): (i32, i32), top: i32) {
        for &(k, em) in glow {
            let (cx, cy) = (x + i32::from(k) % side, y + i32::from(k) / side);
            if cy >= top {
                self.note(t, cx, cy, em);
            }
        }
    }

    /// Drops the px a later draw covered: their colour is not the one they were drawn.
    pub fn check(&mut self, t: &Target<'_>) {
        self.px.retain(|&(i, _, drawn)| t.px[i as usize] == drawn);
    }

    /// Takes each px's colour as it is now (after the silhouettes darkened what they lie on: a
    /// window in a shadow still glows).
    pub fn refresh(&mut self, t: &Target<'_>) {
        for p in &mut self.px {
            p.2 = t.px[p.0 as usize];
        }
    }

    /// After the lightmap (the px checked before it, `check`): each px that still shows what was
    /// drawn there takes its emissive, in linear light at T2's gain (`lit = albedo * light +
    /// emissive * 1.35`). Returns px written.
    pub fn add(&self, t: &mut Target<'_>) -> u64 {
        for &(i, em, _) in &self.px {
            let d = &mut t.px[i as usize];
            *d = add_linear(*d, |k| u32::from(S2L[((em >> k) & 0xff) as usize]) * GAIN / 256);
        }
        self.px.len() as u64
    }

    /// The bloom over `t`, before the grade: what glows, blurred at a quarter size, at `strength`
    /// of 256, added in linear light. Returns px written.
    pub fn bloom(&mut self, t: &mut Target<'_>, strength: u32) -> u64 {
        if self.px.is_empty() || strength == 0 {
            return 0;
        }
        let (w, h) = ((t.w / CELL) as usize + 1, (t.h / CELL) as usize + 1);
        (self.w, self.h) = (w, h);
        let n = w * h;
        for b in [&mut self.cells, &mut self.near, &mut self.tmp] {
            b.clear();
            b.resize(n, [0; 3]);
        }
        // The source: each glowing px's emissive at T2's gain, a quarter to its cell (four
        // times the cell's average: T2's chain sums five levels), and the box of cells it touches.
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for &(i, em, _) in &self.px {
            let (px, py) = (i as usize % t.w as usize, i as usize / t.w as usize);
            let (cx, cy) = (px / CELL as usize, py / CELL as usize);
            let c = &mut self.cells[cy * w + cx];
            for (k, s) in [16u32, 8, 0].into_iter().enumerate() {
                c[k] += u32::from(S2L[((em >> s) & 0xff) as usize]) * GAIN / 256 / 4;
            }
            (x0, y0, x1, y1) = (x0.min(cx), y0.min(cy), x1.max(cx), y1.max(cy));
        }
        let reach = NEAR * 2 + FAR * 2 + 1;
        let (x0, y0) = (x0.saturating_sub(reach), y0.saturating_sub(reach));
        let (x1, y1) = ((x1 + reach).min(w - 1), (y1 + reach).min(h - 1));
        let rect = (x0, y0, x1, y1);
        // The narrow tent (two boxes), then the wide one over it; the bloom is both.
        blur(&mut self.cells, &mut self.tmp, w, rect, NEAR);
        blur(&mut self.cells, &mut self.tmp, w, rect, NEAR);
        self.near.copy_from_slice(&self.cells);
        blur(&mut self.cells, &mut self.tmp, w, rect, FAR);
        blur(&mut self.cells, &mut self.tmp, w, rect, FAR);
        for (c, n) in self.cells.iter_mut().zip(&self.near) {
            for k in 0..3 {
                c[k] = (c[k] * 3 / 2 + n[k]) * strength / 256;
            }
        }
        // Added between cells, bilinear, only in blocks where any of the four cells glows.
        let mut written = 0u64;
        let cells = &self.cells;
        let tw = t.w as usize;
        for cy in y0..y1 {
            for cx in x0..x1 {
                let (a, b, c, d) = (
                    cells[cy * w + cx],
                    cells[cy * w + cx + 1],
                    cells[(cy + 1) * w + cx],
                    cells[(cy + 1) * w + cx + 1],
                );
                if (0..3).all(|k| (a[k] | b[k] | c[k] | d[k]) < FAINT) {
                    continue;
                }
                // The cell's middle is at its px 2: the block from there to the next middle.
                let (bx, by) = (cx * CELL as usize + 2, cy * CELL as usize + 2);
                for fy in 0..CELL as usize {
                    let y = by + fy;
                    if y >= t.h as usize {
                        break;
                    }
                    for fx in 0..CELL as usize {
                        let x = bx + fx;
                        if x >= tw {
                            break;
                        }
                        let (wx, wy) = (fx as u32, fy as u32);
                        let v = |k: usize| {
                            let top = a[k] * (4 - wx) + b[k] * wx;
                            let bot = c[k] * (4 - wx) + d[k] * wx;
                            (top * (4 - wy) + bot * wy) / 16
                        };
                        let add = [v(0), v(1), v(2)];
                        let p = &mut t.px[y * tw + x];
                        *p = add_linear(*p, |s| add[(16 - s as usize) / 8]);
                        written += 1;
                    }
                }
            }
        }
        written
    }
}

/// `d` with `add(shift)` (linear light of 65535 for the channel at `shift`: 16 red, 8 green, 0
/// blue) added to each channel in linear light.
#[inline]
fn add_linear(d: u32, add: impl Fn(u32) -> u32) -> u32 {
    let ch = |s: u32| {
        let l = u32::from(S2L[((d >> s) & 0xff) as usize]) + add(s);
        u32::from(to_display_fast(l)) << s
    };
    0xff00_0000 | ch(16) | ch(8) | ch(0)
}

/// A box blur `r` cells each side over `rect` of `buf` (row width `w`), across then down, by
/// running sums (a cost per cell whatever `r`); past the rect's edge counts as dark.
fn blur(
    buf: &mut [[u32; 3]],
    tmp: &mut [[u32; 3]],
    w: usize,
    (x0, y0, x1, y1): (usize, usize, usize, usize),
    r: usize,
) {
    let n = (2 * r + 1) as u32;
    for y in y0..=y1 {
        let mut s = [0u32; 3];
        for x in x0..=(x0 + r).min(x1) {
            let c = buf[y * w + x];
            for k in 0..3 {
                s[k] += c[k];
            }
        }
        for x in x0..=x1 {
            tmp[y * w + x] = s.map(|v| v / n);
            let add = x + r + 1;
            let a = if add <= x1 { buf[y * w + add] } else { [0; 3] };
            let d = if x >= x0 + r { buf[y * w + x - r] } else { [0; 3] };
            for k in 0..3 {
                s[k] = s[k] + a[k] - d[k];
            }
        }
    }
    for x in x0..=x1 {
        let mut s = [0u32; 3];
        for y in y0..=(y0 + r).min(y1) {
            let c = tmp[y * w + x];
            for k in 0..3 {
                s[k] += c[k];
            }
        }
        for y in y0..=y1 {
            buf[y * w + x] = s.map(|v| v / n);
            let add = y + r + 1;
            let a = if add <= y1 { tmp[add * w + x] } else { [0; 3] };
            let d = if y >= y0 + r { tmp[(y - r) * w + x] } else { [0; 3] };
            for k in 0..3 {
                s[k] = s[k] + a[k] - d[k];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::{Flags, Page, Src, Tint};

    /// A 4 x 1 page whose third texel glows index 3.
    fn page() -> (Page, Vec<(u32, u16)>, Vec<u32>) {
        let page = Page { w: 4, h: 1, albedo: vec![2, 2, 2, 2].into(), ..Page::default() };
        let mut clut = vec![0xff00_0000; 1024];
        clut[2] = 0xff30_3030;
        clut[3] = 0xffff_d080;
        (page, vec![(2, 3)], clut)
    }

    const PLAIN: Flags = Flags { mirror: false, tint: Tint::None, bend: jane_present::Bend::NONE };

    #[test]
    fn a_glowing_texel_shines_over_the_dark_and_a_covered_one_does_not() {
        let (page, glow, clut) = page();
        let mut px = vec![0xff10_1010; 8];
        let mut t = Target { px: &mut px, w: 8, h: 1 };
        let src = Src { x: 0, y: 0, w: 4, h: 1 };
        let mirror = Flags { mirror: true, tint: Tint::None, bend: jane_present::Bend::NONE };
        let mut g = Glow::default();
        g.clear();
        crate::blit::sprite(&mut t, &page, &clut, src, 0, 0, PLAIN, None);
        g.sprite(&t, &page, &glow, &clut, src, (0, 0), PLAIN, None);
        crate::blit::sprite(&mut t, &page, &clut, src, 4, 0, mirror, None);
        g.sprite(&t, &page, &glow, &clut, src, (4, 0), mirror, None);
        // Something is drawn over the first sprite's glowing px.
        t.px[2] = 0xff20_2020;
        g.check(&t);
        g.add(&mut t);
        assert_eq!(t.px[2], 0xff20_2020, "covered: no glow");
        // The mirrored sprite's glowing texel lands at 4 + (3 - 2) = 5, lit past its albedo.
        assert!((t.px[5] >> 16) & 0xff > 0xf0, "{:08x}", t.px[5]);
        assert_eq!(t.px[4], 0xff30_3030);
    }

    #[test]
    fn the_bloom_lights_round_a_glow_and_fades() {
        let (page, glow, clut) = page();
        let mut px = vec![0xff10_1010; 64 * 64];
        let mut t = Target { px: &mut px, w: 64, h: 64 };
        let src = Src { x: 0, y: 0, w: 4, h: 1 };
        let mut g = Glow::default();
        g.clear();
        crate::blit::sprite(&mut t, &page, &clut, src, 30, 32, PLAIN, None);
        g.sprite(&t, &page, &glow, &clut, src, (30, 32), PLAIN, None);
        g.bloom(&mut t, 256);
        let red = |x: usize, y: usize| (t.px[y * 64 + x] >> 16) & 0xff;
        assert!(red(32, 28) > 0x10, "a halo over it");
        assert!(red(32, 28) >= red(32, 20) && red(32, 20) >= red(32, 4), "fading out");
        assert!(red(2, 2) <= 0x11, "far away, next to nothing");
    }
}
