//! The pixel path (PRESENTATION.md §1.4): integer only, scalar code that autovectorises, no
//! intrinsics, no floats, so a frame is the same bytes on every target.

use jane_present::{AO_TINT, Flags, Foot, Page, Src, Tint};

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
/// columns backwards. Clipped to the target. `behind` is where it stands when the terrain stands
/// in front of it, with the terrain's heights over the target: a px the terrain hides is left
/// as it is ([`Foot::skips`]).
#[allow(clippy::too_many_arguments)]
pub fn sprite(
    t: &mut Target<'_>,
    page: &Page,
    clut: &[u32],
    src: Src,
    x: i32,
    y: i32,
    flags: Flags,
    behind: Option<(Foot, &[u8])>,
) {
    let (sw, sh) = (i32::from(src.w), i32::from(src.h));
    let reach = flags.bend.reach();
    let (y0, y1) = (y.max(0), (y + sh).min(t.h));
    if x - reach >= t.w || x + sw + reach <= 0 || y0 >= y1 {
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
        // Bent in the wind, the row is drawn its shift east (`Bend::shift`).
        let x = x + flags.bend.shift(r);
        let (x0, x1) = (x.max(0), (x + sw).min(t.w));
        if x0 >= x1 {
            continue;
        }
        let srow = row(r);
        let near_ao = has_ao(r - 1) || has_ao(r) || has_ao(r + 1);
        let drow = &mut t.px[(dy * t.w) as usize..((dy + 1) * t.w) as usize];
        for dx in x0..x1 {
            let col = dx - x;
            let sx = if flags.mirror { sw - 1 - col } else { col };
            let i = srow[sx as usize];
            if let Some((f, hs)) = behind
                && f.skips(hs[(dy * t.w + dx) as usize], dx, dy, i)
            {
                continue;
            }
            let d = &mut drow[dx as usize];
            match i {
                0 | 1 if flags.tint == Tint::Seen => {}
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
                        Tint::Seen if Tint::seen_at(dx, dy) => c,
                        Tint::Seen => *d,
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

/// Copies a `side`-square block of heights with its top-left at `(x, y)` into `out`, a `(w, h)`
/// canvas of them, clipped: the terrain under each px, for the silhouettes to climb.
pub fn heights(out: &mut [u8], (w, h): (i32, i32), src: &[u8], side: i32, x: i32, y: i32) {
    let (x0, x1) = (x.max(0), (x + side).min(w));
    let (y0, y1) = (y.max(0), (y + side).min(h));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = (x1 - x0) as usize;
    for dy in y0..y1 {
        let s = ((dy - y) * side + (x0 - x)) as usize;
        let d = (dy * w + x0) as usize;
        out[d..d + n].copy_from_slice(&src[s..s + n]);
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
        Page { w: 4, h: 1, albedo: vec![0, 1, 2, 3].into(), ..Page::default() }
    }

    fn blit(flags: Flags) -> Vec<u32> {
        let mut px = vec![GREY; 6];
        let mut t = Target { px: &mut px, w: 6, h: 1 };
        sprite(&mut t, &page(), &clut(), Src { x: 0, y: 0, w: 4, h: 1 }, 1, 0, flags, None);
        px
    }

    /// A bent column: each row its own whole px east, none under `from`, clipped at the edge.
    #[test]
    fn a_bent_sprite_shifts_each_row_by_its_share_of_the_lean() {
        let page = Page { w: 1, h: 5, albedo: vec![2; 5], ..Page::default() };
        for lean in [-3i8, 2, 3] {
            let bend = jane_present::Bend { lean, from: 4, span: 4 };
            let mut px = vec![GREY; 8 * 5];
            let mut t = Target { px: &mut px, w: 8, h: 5 };
            let flags = Flags { bend, ..Flags::default() };
            sprite(&mut t, &page, &clut(), Src { x: 0, y: 0, w: 1, h: 5 }, 3, 0, flags, None);
            for r in 0..5 {
                let at: Vec<usize> = (0..8).filter(|&x| px[r * 8 + x] == RED).collect();
                assert_eq!(at, [(3 + bend.shift(r as i32)) as usize], "lean {lean} row {r}");
            }
            assert_eq!(bend.shift(0), i32::from(lean));
            assert_eq!(bend.shift(4), 0);
        }
    }

    #[test]
    fn the_contact_shadow_is_cool_and_soft_edged_and_colour_is_the_clut() {
        let px = blit(Flags::default());
        // The shadow texel and the clear texel beside it each see one shadow texel in their 3 x 3.
        let s1 = shadow(GREY, 1);
        assert_eq!(px, [GREY, s1, s1, RED, 0xff20_20c0, GREY]);
        assert!(s1 < GREY && s1 & 0xff > (s1 >> 16) & 0xff);
        // Full cover is AO_TINT of each channel, blue held up most; alpha kept.
        assert_eq!(shadow(GREY, 9), 0xff4b_4f63);
        assert_eq!(shadow(GREY, 0), GREY);
    }

    #[test]
    fn a_mirrored_sprite_walks_its_columns_backwards() {
        let px = blit(Flags { mirror: true, tint: Tint::None, bend: jane_present::Bend::NONE });
        let s1 = shadow(GREY, 1);
        assert_eq!(px, [GREY, 0xff20_20c0, RED, s1, s1, GREY]);
    }

    #[test]
    fn flash_goes_toward_white_and_ghost_lets_the_ground_through() {
        let px = blit(Flags { mirror: false, tint: Tint::Flash(255), bend: jane_present::Bend::NONE });
        assert_eq!(px[3], 0xffff_ffff);
        let px = blit(Flags { mirror: false, tint: Tint::Flash(128), bend: jane_present::Bend::NONE });
        assert_eq!(px[3], lerp(RED, 0xffff_ffff, 129));
        assert!(px[3] > RED && px[3] < 0xffff_ffff);
        let px = blit(Flags { mirror: false, tint: Tint::Ghost(128), bend: jane_present::Bend::NONE });
        // Half the red over the grey; the shadow still darkens; clear still skips.
        assert_eq!(px[3], lerp(GREY, RED, 129));
        assert_eq!((px[1], px[2]), (shadow(GREY, 1), shadow(GREY, 1)));
        let px = blit(Flags { mirror: false, tint: Tint::Ghost(0), bend: jane_present::Bend::NONE });
        assert_eq!(px[3], GREY);
    }

    #[test]
    fn blits_clip_at_every_edge() {
        let mut px = vec![0u32; 16];
        let mut t = Target { px: &mut px, w: 4, h: 4 };
        let page = Page { w: 3, h: 3, albedo: vec![2; 9].into(), ..Page::default() };
        for (x, y) in [(-2, -2), (3, 3), (-5, 0), (0, 9)] {
            sprite(&mut t, &page, &clut(), Src { x: 0, y: 0, w: 3, h: 3 }, x, y, Flags::default(), None);
        }
        assert_eq!(px.iter().filter(|&&p| p == RED).count(), 2);
        let mut px = vec![0u32; 16];
        let mut t = Target { px: &mut px, w: 4, h: 4 };
        chunk(&mut t, &[7; 9], 3, 2, -1);
        assert_eq!(px.iter().filter(|&&p| p == 7).count(), 4);
        assert_eq!(px[2], 7);
    }

    #[test]
    fn what_stands_behind_the_terrain_is_cut_where_it_stands_in_front() {
        // A 3 x 3 sprite of red standing on row 3, over a roof 40 px up on the left two columns
        // (its ground rows south of her feet) and flat ground on the right.
        let page = Page { w: 3, h: 3, albedo: vec![2; 9].into(), ..Page::default() };
        let mut heights = vec![0u8; 16];
        for y in 0..4 {
            heights[y * 4] = 40;
            heights[y * 4 + 1] = 40;
        }
        let draw = |see: bool| {
            let mut px = vec![GREY; 16];
            let mut t = Target { px: &mut px, w: 4, h: 4 };
            let foot = Foot { y: 3, see };
            let src = Src { x: 0, y: 0, w: 3, h: 3 };
            sprite(&mut t, &page, &clut(), src, 0, 0, Flags::default(), Some((foot, &heights)));
            px
        };
        let hid = draw(false);
        for y in 0..3 {
            assert_eq!(&hid[y * 4..y * 4 + 3], &[GREY, GREY, RED], "row {y}");
        }
        // Seen through: one px in two of what is hidden, on the canvas's checker.
        let seen = draw(true);
        for y in 0..3usize {
            for x in 0..2usize {
                assert_eq!(seen[y * 4 + x], if (x + y) % 2 == 0 { RED } else { GREY }, "({x}, {y})");
            }
        }
        // A kerb's relief hides nothing; a wall's face whose foot is her row hides nothing.
        assert!(!Foot::hides(Foot::RELIEF, 0, 3));
        assert!(!Foot::hides(12, 0, 10));
        assert!(Foot::hides(12, 0, 9));
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
