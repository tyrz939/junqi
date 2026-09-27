//! The `Ui` pass on T1 (PRESENTATION.md §3.1; `jane_present::ui::cmd`): after the compose, into
//! the canvas, unlit. One quad per command, straight alpha; runs sharing a clip, an image and a
//! page share a draw, and a clip is a scissor. Plain CPU work here; `lib.rs` draws the runs.
//!
//! The contact shadow of a UI sprite (index 1) is black at a quarter of its coverage, as T2 draws
//! it, so one blend draws the whole pass (soft multiplies by `AO_TINT`: the contract lets a
//! backend round either way).

use jane_present::Frame;
use jane_present::ui::{Rect, UiCmd};

/// One draw: quads `first..first + count`, the scissor they share, the image they read (`None`:
/// fills and sprites), and the atlas page their sprites read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiDraw {
    pub first: usize,
    pub count: usize,
    pub clip: Rect,
    pub image: Option<u16>,
    pub page: u8,
}

/// Floats a vertex: pos 2, loc 2, src 4, dst 4, col 4.
pub const PER_VERTEX: usize = 16;

/// Fills `v` and `draws` from `frame.ui`. `has_image(slot)` says whether a slot's image is on
/// the GPU (a command for one that is not is skipped).
pub fn build(frame: &Frame, v: &mut Vec<f32>, draws: &mut Vec<UiDraw>, has_image: impl Fn(u16) -> bool) {
    v.clear();
    draws.clear();
    let mut clip = Rect::CANVAS;
    let mut n = 0usize;
    let mut quad = |v: &mut Vec<f32>, draws: &mut Vec<UiDraw>, clip: Rect, image, page, dst: Rect, rest: [f32; 12]| {
        let (x, y, w, h) = (f32::from(dst.x), f32::from(dst.y), f32::from(dst.w), f32::from(dst.h));
        for (px, py, lx, ly) in [(x, y, 0.0, 0.0), (x + w, y, w, 0.0), (x, y + h, 0.0, h), (x + w, y + h, w, h)] {
            v.extend_from_slice(&[px, py, lx, ly]);
            v.extend_from_slice(&rest);
        }
        match draws.last_mut() {
            Some(d) if d.clip == clip && d.image == image && d.page == page => d.count += 1,
            _ => draws.push(UiDraw { first: n, count: 1, clip, image, page }),
        }
        n += 1;
    };
    for c in &frame.ui {
        match *c {
            UiCmd::Clip(r) => clip = r,
            UiCmd::Fill { dst, argb } => {
                if dst.is_empty() {
                    continue;
                }
                let [a, r, g, b] = argb.to_be_bytes().map(|c| f32::from(c) / 255.0);
                let (w, h) = (f32::from(dst.w), f32::from(dst.h));
                let page = draws.last().map_or(0, |d| d.page);
                quad(v, draws, clip, None, page, dst, [0.0, 0.0, 1.0, 1.0, w, h, 0.0, 0.0, r, g, b, a]);
            }
            UiCmd::Sprite { page, src, dst, ink, alpha, mirror } => {
                if dst.is_empty() {
                    continue;
                }
                let s = [f32::from(src.x), f32::from(src.y), f32::from(src.w), f32::from(src.h)];
                let d = [f32::from(dst.w), f32::from(dst.h), 1.0, if mirror { 1.0 } else { 0.0 }];
                let col = [f32::from(ink), f32::from(alpha) / 255.0, 0.0, 0.0];
                quad(
                    v,
                    draws,
                    clip,
                    None,
                    page,
                    dst,
                    [s[0], s[1], s[2], s[3], d[0], d[1], d[2], d[3], col[0], col[1], col[2], col[3]],
                );
            }
            UiCmd::Image { slot, src, dst, alpha } => {
                if dst.is_empty() || !has_image(slot) {
                    continue;
                }
                let s = [f32::from(src.x), f32::from(src.y), f32::from(src.w), f32::from(src.h)];
                let page = draws.last().map_or(0, |d| d.page);
                let rest = [
                    s[0],
                    s[1],
                    s[2],
                    s[3],
                    f32::from(dst.w),
                    f32::from(dst.h),
                    2.0,
                    0.0,
                    0.0,
                    f32::from(alpha) / 255.0,
                    0.0,
                    0.0,
                ];
                quad(v, draws, clip, Some(slot), page, dst, rest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jane_present::Tier;
    use jane_present::frame::Src;

    #[test]
    fn runs_break_at_a_clip_an_image_or_a_page() {
        let mut f = Frame::new(Tier::T1);
        let src = Src { x: 0, y: 0, w: 2, h: 1 };
        f.ui.extend([
            UiCmd::Fill { dst: Rect::new(0, 0, 8, 4), argb: 0xff10_2030 },
            UiCmd::Sprite { page: 0, src, dst: Rect::new(0, 3, 4, 1), ink: 0, alpha: 255, mirror: false },
            UiCmd::Clip(Rect::new(0, 0, 4, 4)),
            UiCmd::Fill { dst: Rect::new(0, 1, 8, 1), argb: 0x8000_0000 },
            UiCmd::Image { slot: 0, src, dst: Rect::new(0, 0, 2, 1), alpha: 255 },
            UiCmd::Image { slot: 1, src, dst: Rect::new(0, 0, 2, 1), alpha: 255 },
            UiCmd::Fill { dst: Rect::new(0, 0, 0, 1), argb: 0xff00_0000 },
        ]);
        let (mut v, mut d) = (Vec::new(), Vec::new());
        build(&f, &mut v, &mut d, |s| s == 0);
        let got: Vec<(usize, usize, Option<u16>)> = d.iter().map(|d| (d.first, d.count, d.image)).collect();
        // Slot 1 is not on the GPU and the empty fill draws nothing.
        assert_eq!(got, [(0, 2, None), (2, 1, None), (3, 1, Some(0))]);
        assert_eq!(v.len(), 4 * 4 * PER_VERTEX);
    }
}
