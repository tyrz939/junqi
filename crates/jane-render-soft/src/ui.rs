//! The `Ui` pass on T0 (PRESENTATION.md §3.1): the reference for how every backend draws a
//! `UiCmd` (`jane_present::ui::cmd`'s module doc). Integer only.

use jane_present::frame::Src;
use jane_present::ui::{Rect, UiCmd, UiImage};
use jane_present::{AtlasPages, Frame};

use crate::blit::{Target, shadow};

/// `d` toward `c` by `a` of 255, opaque out.
#[inline]
fn blend(d: u32, c: u32, a: u32) -> u32 {
    if a >= 255 {
        return 0xff00_0000 | c;
    }
    let w = a + (a >> 7);
    let v = 256 - w;
    let rb = (((d & 0x00ff_00ff) * v + (c & 0x00ff_00ff) * w) >> 8) & 0x00ff_00ff;
    let g = (((d & 0x0000_ff00) * v + (c & 0x0000_ff00) * w) >> 8) & 0x0000_ff00;
    0xff00_0000 | rb | g
}

/// The part of `dst` inside the clip and the target, as `(x0, y0, x1, y1)`.
fn cut(t: &Target<'_>, clip: Rect, dst: Rect) -> Option<(i32, i32, i32, i32)> {
    let x0 = i32::from(dst.x).max(i32::from(clip.x)).max(0);
    let y0 = i32::from(dst.y).max(i32::from(clip.y)).max(0);
    let x1 = dst.right().min(clip.right()).min(t.w);
    let y1 = dst.bottom().min(clip.bottom()).min(t.h);
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// The source texel column/row for destination offset `d` of `dn`, over `sn` source texels.
#[inline]
fn sample(d: i32, dn: i32, s0: u16, sn: u16) -> usize {
    usize::from(s0) + if i32::from(sn) == dn { d as usize } else { (d * i32::from(sn) / dn.max(1)) as usize }
}

/// Draws `frame.ui` over the target. Returns the pixels written.
pub fn draw(t: &mut Target<'_>, frame: &Frame, atlas: &AtlasPages) -> u64 {
    let mut clip = Rect::CANVAS;
    let mut written = 0u64;
    for cmd in &frame.ui {
        match *cmd {
            UiCmd::Clip(r) => clip = r,
            UiCmd::Fill { dst, argb } => {
                let a = argb >> 24;
                let Some((x0, y0, x1, y1)) = cut(t, clip, dst) else { continue };
                let c = argb & 0x00ff_ffff;
                for y in y0..y1 {
                    let row = &mut t.px[(y * t.w + x0) as usize..(y * t.w + x1) as usize];
                    for d in row {
                        *d = blend(*d, c, a);
                    }
                }
                written += ((x1 - x0) * (y1 - y0)) as u64;
            }
            UiCmd::Sprite { page, src, dst, ink, alpha, mirror } => {
                let Some(p) = atlas.pages.get(usize::from(page)) else { continue };
                written += sprite(t, clip, &p.albedo, p.w, &atlas.clut, src, dst, ink, u32::from(alpha), mirror);
            }
            UiCmd::Image { slot, src, dst, alpha } => {
                let Some(img) = frame.ui_images.get(usize::from(slot)) else { continue };
                written += image(t, clip, img, src, dst, u32::from(alpha));
            }
        }
    }
    written
}

#[allow(clippy::too_many_arguments)]
fn sprite(
    t: &mut Target<'_>,
    clip: Rect,
    albedo: &[u16],
    pw: u16,
    clut: &[u32],
    src: Src,
    dst: Rect,
    ink: u16,
    alpha: u32,
    mirror: bool,
) -> u64 {
    let Some((x0, y0, x1, y1)) = cut(t, clip, dst) else { return 0 };
    let (dw, dh) = (i32::from(dst.w), i32::from(dst.h));
    let pw = usize::from(pw);
    let inked = clut.get(usize::from(ink)).copied().unwrap_or(0) & 0x00ff_ffff;
    for y in y0..y1 {
        let sy = sample(y - i32::from(dst.y), dh, src.y, src.h);
        let row = sy * pw;
        for x in x0..x1 {
            let mut dx = x - i32::from(dst.x);
            if mirror {
                dx = dw - 1 - dx;
            }
            let sx = sample(dx, dw, src.x, src.w);
            let Some(&i) = albedo.get(row + sx) else { continue };
            let d = &mut t.px[(y * t.w + x) as usize];
            match i {
                0 => {}
                1 => *d = blend(*d, shadow(*d, 9) & 0x00ff_ffff, alpha),
                _ => {
                    let c = if ink != 0 { inked } else { clut.get(usize::from(i)).copied().unwrap_or(0) & 0x00ff_ffff };
                    *d = blend(*d, c, alpha);
                }
            }
        }
    }
    ((x1 - x0) * (y1 - y0)) as u64
}

fn image(t: &mut Target<'_>, clip: Rect, img: &UiImage, src: Src, dst: Rect, alpha: u32) -> u64 {
    let Some((x0, y0, x1, y1)) = cut(t, clip, dst) else { return 0 };
    let (dw, dh) = (i32::from(dst.w), i32::from(dst.h));
    let w = usize::from(img.w);
    for y in y0..y1 {
        let sy = sample(y - i32::from(dst.y), dh, src.y, src.h);
        for x in x0..x1 {
            let sx = sample(x - i32::from(dst.x), dw, src.x, src.w);
            let Some(&p) = img.argb.get(sy * w + sx) else { continue };
            let a = ((p >> 24) * (alpha + 1)) >> 8;
            if a == 0 {
                continue;
            }
            let d = &mut t.px[(y * t.w + x) as usize];
            *d = blend(*d, p & 0x00ff_ffff, a);
        }
    }
    ((x1 - x0) * (y1 - y0)) as u64
}

#[cfg(test)]
mod tests {
    use jane_present::Tier;
    use jane_present::backend::Page;

    use super::*;

    fn run(cmds: &[UiCmd], images: Vec<UiImage>) -> Vec<u32> {
        let mut f = Frame::new(Tier::T0);
        f.ui.extend_from_slice(cmds);
        f.ui_images = images;
        let mut clut = vec![0xff00_0000; 1024];
        clut[2] = 0xff11_2233;
        clut[5] = 0xffff_0000;
        let atlas = AtlasPages {
            clut,
            pages: vec![Page { w: 2, h: 1, albedo: vec![5, 1].into(), ..Page::default() }],
            ..AtlasPages::default()
        };
        let mut px = vec![0xff80_8080; 16];
        let mut t = Target { px: &mut px, w: 4, h: 4 };
        draw(&mut t, &f, &atlas);
        px
    }

    #[test]
    fn a_fill_blends_by_its_alpha_and_keeps_to_the_clip() {
        let px = run(
            &[
                UiCmd::Clip(Rect::new(0, 0, 2, 4)),
                UiCmd::Fill { dst: Rect::new(0, 0, 4, 1), argb: 0xff00_00ff },
                UiCmd::Clip(Rect::CANVAS),
                UiCmd::Fill { dst: Rect::new(0, 1, 4, 1), argb: 0x8000_0000 },
            ],
            vec![],
        );
        assert_eq!(&px[0..4], &[0xff00_00ff, 0xff00_00ff, 0xff80_8080, 0xff80_8080]);
        assert_eq!(px[4], blend(0xff80_8080, 0, 0x80));
        assert!(px[4] < 0xff80_8080 && px[4] > 0xff30_3030);
    }

    #[test]
    fn a_sprite_stretches_nearest_takes_an_ink_and_mirrors() {
        let src = Src { x: 0, y: 0, w: 2, h: 1 };
        let px = run(
            &[UiCmd::Sprite { page: 0, src, dst: Rect::new(0, 0, 4, 1), ink: 0, alpha: 255, mirror: false }],
            vec![],
        );
        // Two texels over four px: red, red, then the shadow twice.
        assert_eq!(&px[0..2], &[0xffff_0000, 0xffff_0000]);
        assert_eq!(px[2], shadow(0xff80_8080, 9));
        let px = run(
            &[UiCmd::Sprite { page: 0, src, dst: Rect::new(0, 0, 2, 1), ink: 2, alpha: 255, mirror: true }],
            vec![],
        );
        assert_eq!(&px[0..2], &[shadow(0xff80_8080, 9), 0xff11_2233], "mirrored, the red texel inked");
    }

    #[test]
    fn an_image_blends_by_its_own_alpha_times_the_commands() {
        let img = UiImage { generation: 1, w: 1, h: 1, argb: vec![0x80ff_ffff] };
        let px = run(
            &[UiCmd::Image { slot: 0, src: Src { x: 0, y: 0, w: 1, h: 1 }, dst: Rect::new(1, 1, 2, 2), alpha: 255 }],
            vec![img],
        );
        assert_eq!(px[0], 0xff80_8080);
        assert!(px[5] > 0xffb0_b0b0 && px[5] < 0xffd0_d0d0, "{:08x}", px[5]);
        assert_eq!(px[5], px[10]);
    }
}
