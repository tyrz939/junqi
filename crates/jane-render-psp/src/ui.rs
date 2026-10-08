//! The `Ui` pass on the GE (PORT.md §13.13): the frame's [`UiCmd`]s as quads, after every other
//! pass, unlit, in list order (the contract in `jane_present::ui::cmd`).
//!
//! - **Sprites** are the UI page's pictures (`UiArt::rects`), packed into the PSP pack as `ui`
//!   sprites keyed by their place in that list, trimmed: each draws from its pack page, the
//!   trimmed rect laid where it sits in the untrimmed one (stretched as the command stretches).
//!   An `ink` is the page drawn through an all-white CLUT ([`Tex::Ink`]) times the ink's colour.
//! - **Fills** are flat quads at the colour's coverage.
//! - **Images** (the title's backdrop, the map's chart, the loading card) are `8888` textures the
//!   GE glue converts when their generation moves ([`Tex::Image`]).
//! - **Clips** are applied here, in integer px, to every later quad (texels cut in step), so the
//!   GE's scissor stays the canvas.
//!
//! Pure and integer, so a PC test reads what the PSP draws.

use alloc::vec::Vec;

use jane_present::Frame;
use jane_present::ui::{Rect, UiArt, UiCmd};

use crate::list::{Mode, Quad, Tex, abgr};
use crate::pack::Pack;

/// The PSP pack's UI category (`jane-cli`'s `Cat::Ui`).
pub const CAT: u8 = 8;

/// A UI picture as the pack holds it: its rect on the presenter's UI page, and the pack's page
/// and trimmed rect, with where that rect starts in the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pic {
    src: (u16, u16, u16, u16),
    page: u16,
    u: u16,
    v: u16,
    w: u16,
    h: u16,
    tx: u16,
    ty: u16,
}

/// The UI page's pictures resolved to the pack, and the quads of the last frame's `Ui` pass.
#[derive(Debug, Default)]
pub struct UiLister {
    /// The presenter's UI page index (`UiArt::page`).
    page: u8,
    /// Sorted by `src`.
    pics: Vec<Pic>,
    pub quads: Vec<Quad>,
    /// Sprites this frame named that the pack has no picture for.
    pub misses: u32,
}

impl UiLister {
    pub fn new(art: &UiArt, pack: &Pack) -> UiLister {
        let mut pics: Vec<Pic> = art
            .rects()
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let k = pack
                    .recs
                    .binary_search_by_key(&(CAT, i as u16, 0u8, 0u8), |x| (x.cat, x.sprite, x.vs, x.frame))
                    .ok()?;
                let rec = pack.recs[k];
                Some(Pic {
                    src: (r.x, r.y, r.w, r.h),
                    page: rec.page,
                    u: rec.u,
                    v: rec.v,
                    w: rec.w,
                    h: rec.h,
                    tx: (-rec.ax).max(0) as u16,
                    ty: (-rec.ay).max(0) as u16,
                })
            })
            .collect();
        pics.sort_unstable_by_key(|p| p.src);
        pics.dedup_by_key(|p| p.src);
        UiLister { page: art.page, pics, quads: Vec::with_capacity(1024), misses: 0 }
    }

    /// How many pictures the pack resolved.
    pub fn len(&self) -> usize {
        self.pics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pics.is_empty()
    }

    /// The frame's `Ui` pass as quads, into [`quads`](Self::quads).
    pub fn build(&mut self, frame: &Frame) -> &[Quad] {
        self.quads.clear();
        self.misses = 0;
        let canvas = Rect::new(0, 0, i32::from(frame.canvas.0), i32::from(frame.canvas.1));
        let mut clip = canvas;
        for c in &frame.ui {
            match *c {
                UiCmd::Clip(r) => clip = r.intersect(canvas),
                UiCmd::Fill { dst, argb } => {
                    let colour = abgr(argb);
                    push(&mut self.quads, Tex::None, colour, clip, (dst, (0, 0, 0, 0)), false);
                }
                UiCmd::Sprite { page, src, dst, ink, alpha, mirror } => {
                    if page != self.page {
                        self.misses += 1;
                        continue;
                    }
                    let Ok(k) = self.pics.binary_search_by_key(&(src.x, src.y, src.w, src.h), |p| p.src) else {
                        self.misses += 1;
                        continue;
                    };
                    let p = self.pics[k];
                    if p.page == u16::MAX || p.w == 0 || src.w == 0 || src.h == 0 {
                        continue;
                    }
                    // The trimmed rect where it lies in the picture, scaled as `src` is to `dst`.
                    let (sw, sh) = (i32::from(src.w), i32::from(src.h));
                    let (dw, dh) = (i32::from(dst.w), i32::from(dst.h));
                    let lx = if mirror { sw - i32::from(p.tx) - i32::from(p.w) } else { i32::from(p.tx) };
                    let x0 = i32::from(dst.x) + lx * dw / sw;
                    let x1 = i32::from(dst.x) + (lx + i32::from(p.w)) * dw / sw;
                    let y0 = i32::from(dst.y) + i32::from(p.ty) * dh / sh;
                    let y1 = i32::from(dst.y) + (i32::from(p.ty) + i32::from(p.h)) * dh / sh;
                    let (u0, u1) = (i32::from(p.u), i32::from(p.u) + i32::from(p.w));
                    let (v0, v1) = (i32::from(p.v), i32::from(p.v) + i32::from(p.h));
                    let (tex, colour) = if ink != 0 {
                        let [r, g, b] = jane_art::palette::rgb(jane_art::palette::Ix(ink));
                        (Tex::Ink(p.page), crate::list::rgba([r, g, b], alpha))
                    } else {
                        (Tex::Page(p.page), u32::from(alpha) << 24 | 0x00ff_ffff)
                    };
                    let r = Rect::new(x0, y0, x1 - x0, y1 - y0);
                    push(&mut self.quads, tex, colour, clip, (r, (u0, v0, u1, v1)), mirror);
                }
                UiCmd::Image { slot, src, dst, alpha } => {
                    let uv = (
                        i32::from(src.x),
                        i32::from(src.y),
                        i32::from(src.x) + i32::from(src.w),
                        i32::from(src.y) + i32::from(src.h),
                    );
                    let colour = u32::from(alpha) << 24 | 0x00ff_ffff;
                    push(&mut self.quads, Tex::Image(slot), colour, clip, (dst, uv), false);
                }
            }
        }
        &self.quads
    }
}

/// One quad of `dst` sampling `uv` (`u0, v0, u1, v1`, one past the last texel), cut to `clip`
/// with its texels cut in step; mirrored, `u` runs backward.
fn push(out: &mut Vec<Quad>, tex: Tex, colour: u32, clip: Rect, (dst, uv): (Rect, (i32, i32, i32, i32)), mirror: bool) {
    let (x0, y0, x1, y1) = (i32::from(dst.x), i32::from(dst.y), dst.right(), dst.bottom());
    if x1 <= x0 || y1 <= y0 || colour >> 24 == 0 {
        return;
    }
    let (cx0, cy0, cx1, cy1) = (i32::from(clip.x), i32::from(clip.y), clip.right(), clip.bottom());
    let (nx0, ny0, nx1, ny1) = (x0.max(cx0), y0.max(cy0), x1.min(cx1), y1.min(cy1));
    if nx1 <= nx0 || ny1 <= ny0 {
        return;
    }
    let (u0, v0, u1, v1) = uv;
    // Texels along an axis cut in step with the px: `t(p)` is the texel edge at px edge `p`.
    let at = |p: i32, p0: i32, p1: i32, t0: i32, t1: i32| t0 + (p - p0) * (t1 - t0) / (p1 - p0);
    let (mut a0, mut a1) = (at(nx0, x0, x1, u0, u1), at(nx1, x0, x1, u0, u1));
    if mirror {
        // The leftmost px shows the last texel: `u` from the right edge back.
        (a0, a1) = (u1 - (a0 - u0), u1 - (a1 - u0));
    }
    let (b0, b1) = (at(ny0, y0, y1, v0, v1), at(ny1, y0, y1, v0, v1));
    out.push(Quad {
        tex,
        mode: Mode::Alpha,
        colour,
        x0: nx0 as i16,
        y0: ny0 as i16,
        x1: nx1 as i16,
        y1: ny1 as i16,
        u0: a0.max(0) as u16,
        v0: b0.max(0) as u16,
        u1: a1.max(0) as u16,
        v1: b1.max(0) as u16,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::frame::Src;

    fn lister() -> UiLister {
        UiLister {
            page: 3,
            pics: alloc::vec![Pic { src: (10, 20, 8, 12), page: 5, u: 100, v: 40, w: 6, h: 9, tx: 1, ty: 2 }],
            quads: Vec::new(),
            misses: 0,
        }
    }

    fn frame(cmds: &[UiCmd]) -> Frame {
        let mut f = Frame::new(jane_present::Tier::T0);
        f.canvas = (480, 272);
        f.ui = cmds.to_vec();
        f
    }

    #[test]
    fn a_sprite_lays_its_trimmed_rect_where_it_sits_and_an_ink_takes_the_white_clut() {
        let mut l = lister();
        let src = Src { x: 10, y: 20, w: 8, h: 12 };
        let dst = Rect::new(50, 60, 8, 12);
        let f = frame(&[
            UiCmd::Sprite { page: 3, src, dst, ink: 0, alpha: 255, mirror: false },
            UiCmd::Sprite { page: 3, src, dst, ink: 7, alpha: 128, mirror: true },
            UiCmd::Sprite { page: 2, src, dst, ink: 0, alpha: 255, mirror: false },
        ]);
        let q = l.build(&f).to_vec();
        assert_eq!(q.len(), 2);
        assert_eq!((q[0].tex, q[0].x0, q[0].y0, q[0].x1, q[0].y1), (Tex::Page(5), 51, 62, 57, 71));
        assert_eq!((q[0].u0, q[0].v0, q[0].u1, q[0].v1), (100, 40, 106, 49));
        assert_eq!(q[0].colour, 0xffff_ffff);
        // Mirrored: the trim from the other side, `u` backward.
        assert_eq!((q[1].tex, q[1].x0, q[1].x1, q[1].u0, q[1].u1), (Tex::Ink(5), 51, 57, 106, 100));
        assert_eq!(q[1].colour >> 24, 128);
        assert_eq!(l.misses, 1);
    }

    #[test]
    fn a_clip_cuts_the_quads_and_their_texels_in_step() {
        let mut l = lister();
        let f = frame(&[
            UiCmd::Clip(Rect::new(0, 0, 54, 100)),
            UiCmd::Sprite {
                page: 3,
                src: Src { x: 10, y: 20, w: 8, h: 12 },
                dst: Rect::new(50, 60, 8, 12),
                ink: 0,
                alpha: 255,
                mirror: false,
            },
            UiCmd::Fill { dst: Rect::new(40, 0, 40, 10), argb: 0x80ff_0000 },
            UiCmd::Clip(Rect::CANVAS),
            UiCmd::Fill { dst: Rect::new(-10, 260, 600, 40), argb: 0xff00_00ff },
        ]);
        let q = l.build(&f).to_vec();
        assert_eq!((q[0].x0, q[0].x1, q[0].u0, q[0].u1), (51, 54, 100, 103));
        assert_eq!((q[1].x0, q[1].x1, q[1].colour), (40, 54, 0x8000_00ff));
        assert_eq!((q[2].x0, q[2].y0, q[2].x1, q[2].y1), (0, 260, 480, 272));
    }
}
