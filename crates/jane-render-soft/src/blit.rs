//! The pixel path (PRESENTATION.md §1.4): integer only, scalar code that autovectorises, no
//! intrinsics, no floats, so a frame is the same bytes on every target.

use jane_present::{AO_TINT, Flags, Page, Src, Tint};

/// A `u32` canvas, `0xAARRGGBB`, row-major.
#[derive(Debug)]
pub struct Target<'a> {
    pub px: &'a mut [u32],
    pub w: i32,
    pub h: i32,
}

/// The contact shadow (index 1) at `cover` of 9 (how much of the texel's 3 x 3 the index-1 mask
/// covers): each channel toward `AO_TINT` of itself, a cool darkening with a soft edge
/// (`jane_art::palette::ao`, which this matches bit for bit). Alpha stays.
#[inline]
pub fn shadow(d: u32, cover: u32) -> u32 {
    let cover = cover.min(9);
    let ch = |shift: u32, k: usize| {
        let f = 256 - (256 - u32::from(AO_TINT[k])) * cover / 9;
        (((d >> shift) & 0xff) * f / 256) << shift
    };
    (d & 0xff00_0000) | ch(16, 0) | ch(8, 1) | ch(0, 2)
}

/// `x` toward `y` by `a` of 256, two multiplies on packed channel pairs. Opaque out.
#[inline]
pub fn lerp(x: u32, y: u32, a: u32) -> u32 {
    let b = 256 - a;
    let rb = (((x & 0x00ff_00ff) * b + (y & 0x00ff_00ff) * a) >> 8) & 0x00ff_00ff;
    let g = (((x & 0x0000_ff00) * b + (y & 0x0000_ff00) * a) >> 8) & 0x0000_ff00;
    0xff00_0000 | rb | g
}

/// 0..=255 as a weight 0..=256.
#[inline]
fn weight(a: u8) -> u32 {
    u32::from(a) + u32::from(a >> 7)
}

/// Blits `src` of `page` with its top-left at `(x, y)`: index 0 skipped, index 1 darkens what is
/// under it by how much of its 3 x 3 the contact shadow covers (so a clear texel beside one takes
/// a little of it), every other index its CLUT colour, tinted. A mirrored sprite walks its source
/// columns backwards. Clipped to the target.
pub fn sprite(t: &mut Target<'_>, page: &Page, clut: &[u32], src: Src, x: i32, y: i32, flags: Flags) {
    let (sw, sh) = (i32::from(src.w), i32::from(src.h));
    let (x0, x1) = (x.max(0), (x + sw).min(t.w));
    let (y0, y1) = (y.max(0), (y + sh).min(t.h));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let pw = usize::from(page.w);
    let row = |r: i32| -> &[u16] {
        let sy = usize::from(src.y) + r as usize;
        &page.albedo[sy * pw + usize::from(src.x)..sy * pw + usize::from(src.x) + sw as usize]
    };
    // Whether source row r holds any contact shadow: only rows next to one need the 3 x 3.
    let has_ao = |r: i32| r >= 0 && r < sh && row(r).contains(&1);
    for dy in y0..y1 {
        let r = dy - y;
        let srow = row(r);
        let near_ao = has_ao(r - 1) || has_ao(r) || has_ao(r + 1);
        let drow = &mut t.px[(dy * t.w) as usize..((dy + 1) * t.w) as usize];
        for dx in x0..x1 {
            let col = dx - x;
            let sx = if flags.mirror { sw - 1 - col } else { col };
            let i = srow[sx as usize];
            let d = &mut drow[dx as usize];
            match i {
                0 | 1 => {
                    if near_ao {
                        let mut cover = 0;
                        for yy in (r - 1).max(0)..=(r + 1).min(sh - 1) {
                            let rr = row(yy);
                            for xx in (sx - 1).max(0)..=(sx + 1).min(sw - 1) {
                                cover += u32::from(rr[xx as usize] == 1);
                            }
                        }
                        if cover > 0 {
                            *d = shadow(*d, cover);
                        }
                    }
                }
                _ => {
                    let c = clut[usize::from(i)];
                    *d = match flags.tint {
                        Tint::None => c,
                        Tint::Flash(a) => lerp(c, 0xffff_ffff, weight(a)),
                        Tint::Ghost(a) => lerp(*d, c, weight(a)),
                    };
                }
            }
        }
    }
}

/// Copies a `side`-square block of resolved pixels with its top-left at `(x, y)`, clipped.
pub fn chunk(t: &mut Target<'_>, px: &[u32], side: i32, x: i32, y: i32) {
    let (x0, x1) = (x.max(0), (x + side).min(t.w));
    let (y0, y1) = (y.max(0), (y + side).min(t.h));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = (x1 - x0) as usize;
    for dy in y0..y1 {
        let s = ((dy - y) * side + (x0 - x)) as usize;
        let d = (dy * t.w + x0) as usize;
        t.px[d..d + n].copy_from_slice(&px[s..s + n]);
    }
}

/// Multiplies every pixel by `ambient` per channel (255 is full): `dst = dst * (L + 1) >> 8`.
pub fn multiply(t: &mut Target<'_>, ambient: [u8; 3]) {
    let [r, g, b] = ambient.map(|c| u32::from(c) + 1);
    for d in t.px.iter_mut() {
        let c = *d;
        *d = 0xff00_0000
            | (((((c >> 16) & 0xff) * r) >> 8) << 16)
            | (((((c >> 8) & 0xff) * g) >> 8) << 8)
            | (((c & 0xff) * b) >> 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: u32 = 0xffc0_2020;
    const GREY: u32 = 0xff80_8080;

    /// A CLUT where index 2 is red and 3 is blue.
    fn clut() -> Vec<u32> {
        let mut c = vec![0xff00_0000; 1024];
        c[2] = RED;
        c[3] = 0xff20_20c0;
        c
    }

    /// A 3 x 1 page: clear, shadow, red, then blue at the far end of a 4 x 1.
    fn page() -> Page {
        Page { w: 4, h: 1, albedo: vec![0, 1, 2, 3], ..Page::default() }
    }

    fn blit(flags: Flags) -> Vec<u32> {
        let mut px = vec![GREY; 6];
        let mut t = Target { px: &mut px, w: 6, h: 1 };
        sprite(&mut t, &page(), &clut(), Src { x: 0, y: 0, w: 4, h: 1 }, 1, 0, flags);
        px
    }

    #[test]
    fn the_contact_shadow_is_cool_and_soft_edged_and_colour_is_the_clut() {
        let px = blit(Flags::default());
        // The shadow texel and the clear texel beside it each see one shadow texel in their 3 x 3.
        let s1 = shadow(GREY, 1);
        assert_eq!(px, [GREY, s1, s1, RED, 0xff20_20c0, GREY]);
        assert!(s1 < GREY && s1 & 0xff > (s1 >> 16) & 0xff);
        // Full cover is AO_TINT of each channel, blue held up most; alpha kept.
        assert_eq!(shadow(GREY, 9), 0xff53_5667);
        assert_eq!(shadow(GREY, 0), GREY);
    }

    #[test]
    fn a_mirrored_sprite_walks_its_columns_backwards() {
        let px = blit(Flags { mirror: true, tint: Tint::None });
        let s1 = shadow(GREY, 1);
        assert_eq!(px, [GREY, 0xff20_20c0, RED, s1, s1, GREY]);
    }

    #[test]
    fn flash_goes_toward_white_and_ghost_lets_the_ground_through() {
        let px = blit(Flags { mirror: false, tint: Tint::Flash(255) });
        assert_eq!(px[3], 0xffff_ffff);
        let px = blit(Flags { mirror: false, tint: Tint::Flash(128) });
        assert_eq!(px[3], lerp(RED, 0xffff_ffff, 129));
        assert!(px[3] > RED && px[3] < 0xffff_ffff);
        let px = blit(Flags { mirror: false, tint: Tint::Ghost(128) });
        // Half the red over the grey; the shadow still darkens; clear still skips.
        assert_eq!(px[3], lerp(GREY, RED, 129));
        assert_eq!((px[1], px[2]), (shadow(GREY, 1), shadow(GREY, 1)));
        let px = blit(Flags { mirror: false, tint: Tint::Ghost(0) });
        assert_eq!(px[3], GREY);
    }

    #[test]
    fn blits_clip_at_every_edge() {
        let mut px = vec![0u32; 16];
        let mut t = Target { px: &mut px, w: 4, h: 4 };
        let page = Page { w: 3, h: 3, albedo: vec![2; 9], ..Page::default() };
        for (x, y) in [(-2, -2), (3, 3), (-5, 0), (0, 9)] {
            sprite(&mut t, &page, &clut(), Src { x: 0, y: 0, w: 3, h: 3 }, x, y, Flags::default());
        }
        assert_eq!(px.iter().filter(|&&p| p == RED).count(), 2);
        let mut px = vec![0u32; 16];
        let mut t = Target { px: &mut px, w: 4, h: 4 };
        chunk(&mut t, &[7; 9], 3, 2, -1);
        assert_eq!(px.iter().filter(|&&p| p == 7).count(), 4);
        assert_eq!(px[2], 7);
    }

    #[test]
    fn full_ambient_changes_nothing_and_dark_scales() {
        let mut px = vec![0xff12_3456, 0xffff_ffff];
        multiply(&mut Target { px: &mut px, w: 2, h: 1 }, [255; 3]);
        assert_eq!(px, [0xff12_3456, 0xffff_ffff]);
        multiply(&mut Target { px: &mut px, w: 2, h: 1 }, [127, 0, 255]);
        assert_eq!(px, [0xff09_0056, 0xff7f_00ff]);
    }
}
