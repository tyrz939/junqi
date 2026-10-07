//! The `Ui` pass's draw list (PRESENTATION.md §3.1): what every backend draws after every other
//! pass, unlit, in canvas px, in list order. This is the whole contract between the UI and a
//! backend; it names no texture, shader or pixel format.
//!
//! Four commands:
//!
//! | Command | Draws |
//! | --- | --- |
//! | [`UiCmd::Fill`] | a rect of one colour, blended over what is under it by the colour's alpha |
//! | [`UiCmd::Sprite`] | a rect of an atlas page (`AtlasPages::pages[page]`), each texel its CLUT colour or, when `ink` is not 0, the CLUT colour `ink` (text, the tinted marks) |
//! | [`UiCmd::Image`] | a rect of one of the frame's dynamic images (`Frame::ui_images[slot]`: the map, the loading card) |
//! | [`UiCmd::Clip`] | every later command is cut to this rect, until the next `Clip` |
//!
//! The rules a backend keeps, so every backend draws the same pixels:
//!
//! - **Order.** Commands draw in list order, after the last pass of the frame (after `Post` on
//!   T2): the UI is never lit, graded, bloomed or fogged. The frame starts with no clip.
//! - **Blend.** `out = under + (colour - under) * a / 255` per channel, with `a` the command's
//!   coverage. A backend may round either way; `soft` is the reference.
//! - **Sprite texels.** Albedo index 0 is clear (skipped). Index 1, the contact shadow, darkens
//!   what is under it to three quarters (as the world blit does). Any other index is
//!   `clut[index]` (or `clut[ink]` when `ink != 0`) at coverage `alpha`. A sprite is
//!   **stretched** nearest-neighbour from `src` to `dst` when their sizes differ: destination
//!   px `(dx, dy)` samples source texel `(src.x + dx * src.w / dst.w, src.y + dy * src.h / dst.h)`,
//!   integer division. `mirror` walks the source columns backwards.
//! - **Image texels** are `0xAARRGGBB`; each pixel's own alpha times the command's `alpha` / 255
//!   is its coverage. Sampled as a sprite is. A backend caches an image by `(slot, generation)`
//!   and uploads it again only when the generation moves.
//! - **Clip** is a scissor in canvas px; a clip of `Rect::CANVAS` (or any rect covering the
//!   canvas) is none.
//!
//! Everything is integer. No command carries a float.

use crate::frame::Src;
use alloc::vec;
use alloc::vec::Vec;

/// A rect in canvas px. `w` and `h` of 0 or less draw nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
}

impl Rect {
    /// Any canvas (4096 px wide at most, `input::canvas_size`): as a clip, no clip at all.
    pub const CANVAS: Rect = Rect { x: 0, y: 0, w: i16::MAX, h: i16::MAX };

    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x: x as i16, y: y as i16, w: w as i16, h: h as i16 }
    }

    pub const fn right(self) -> i32 {
        self.x as i32 + self.w as i32
    }

    pub const fn bottom(self) -> i32 {
        self.y as i32 + self.h as i32
    }

    pub fn contains(self, p: (i32, i32)) -> bool {
        p.0 >= i32::from(self.x) && p.1 >= i32::from(self.y) && p.0 < self.right() && p.1 < self.bottom()
    }

    /// This rect shrunk by `d` px on every side (grown when `d` is negative).
    pub const fn inset(self, d: i32) -> Rect {
        Rect::new(self.x as i32 + d, self.y as i32 + d, self.w as i32 - 2 * d, self.h as i32 - 2 * d)
    }

    /// Where two rects overlap (empty when they do not).
    pub fn intersect(self, o: Rect) -> Rect {
        let x0 = i32::from(self.x).max(i32::from(o.x));
        let y0 = i32::from(self.y).max(i32::from(o.y));
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// One draw of the `Ui` pass. See the module doc for how each is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCmd {
    /// `dst` in `argb` (`0xAARRGGBB`; the alpha byte is the coverage).
    Fill { dst: Rect, argb: u32 },
    /// `src` of atlas page `page` into `dst`. `ink` 0 keeps the texels' own colours; any other
    /// value is a CLUT index every opaque texel takes. `alpha` is the coverage, 255 opaque.
    Sprite { page: u8, src: Src, dst: Rect, ink: u16, alpha: u8, mirror: bool },
    /// `src` of `Frame::ui_images[slot]` into `dst`, at `alpha` times each pixel's own alpha.
    Image { slot: u16, src: Src, dst: Rect, alpha: u8 },
    /// Cut every later command to `dst` (`Rect::CANVAS`: no cut).
    Clip(Rect),
}

/// A picture the UI makes at run time (the map's chart, the loading screen's county), carried by
/// the frame in a slot that persists across frames. `argb` is `w * h`, row-major, `0xAARRGGBB`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiImage {
    /// Moves every time the pixels change: a backend re-uploads on a new `(slot, generation)`.
    pub generation: u32,
    pub w: u16,
    pub h: u16,
    pub argb: Vec<u32>,
}

impl UiImage {
    pub fn new(w: u16, h: u16) -> UiImage {
        UiImage { generation: 0, w, h, argb: vec![0; usize::from(w) * usize::from(h)] }
    }
}
