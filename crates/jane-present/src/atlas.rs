//! The atlas (PRESENTATION.md §1.4): albedo pages of master-palette indices, packed at boot, and
//! the sprite table the scene looks frames up in. Nothing renders from a look during play.

use jane_art::Canvas;
use jane_art::palette::{self, Ix};

use crate::backend::{AtlasPages, CLUT_LEN, Page};
use crate::frame::Src;

/// Where a sprite lives and where its anchor is (ART.md §1: units by the feet, props by the
/// bottom of the footprint).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRef {
    pub page: u8,
    pub src: Src,
    /// The anchor inside the sprite, px from its top-left.
    pub ax: i16,
    pub ay: i16,
    /// How tall the thing stands, px.
    pub height: u8,
}

/// Index into [`Atlas::refs`].
pub type RefId = u16;

/// The packed pages and their sprite table.
#[derive(Debug)]
pub struct Atlas {
    pub pages: AtlasPages,
    pub refs: Vec<SpriteRef>,
    shelf: (u16, u16, u16),
}

/// The width of a page as packed here. 2048 is the ceiling (ART.md §5); the stand-ins need far
/// less and a page grows downward as it fills.
const PAGE_W: u16 = 512;

impl Atlas {
    /// An empty atlas with the master palette as its CLUT.
    pub fn new() -> Atlas {
        let mut clut = vec![0xff00_0000; CLUT_LEN];
        for (i, c) in palette::PALETTE.iter().enumerate() {
            clut[i] = 0xff00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
        }
        Atlas {
            pages: AtlasPages { clut, pages: vec![Page { w: PAGE_W, h: 0, albedo: Vec::new() }] },
            refs: Vec::new(),
            shelf: (0, 0, 0),
        }
    }

    /// Packs a `w x h` grid of indices given by `px(x, y)`; returns its id.
    pub fn add(&mut self, w: u16, h: u16, anchor: (i16, i16), height: u8, px: impl Fn(i32, i32) -> Ix) -> RefId {
        assert!(w <= PAGE_W, "a sprite wider than a page");
        let (mut sx, mut sy, mut sh) = self.shelf;
        if sx + w > PAGE_W {
            sx = 0;
            sy += sh;
            sh = 0;
        }
        sh = sh.max(h);
        let page = &mut self.pages.pages[0];
        if sy + h > page.h {
            page.h = sy + h;
            page.albedo.resize(usize::from(page.w) * usize::from(page.h), 0);
        }
        for y in 0..h {
            for x in 0..w {
                let i = usize::from(sy + y) * usize::from(page.w) + usize::from(sx + x);
                page.albedo[i] = px(i32::from(x), i32::from(y)).0;
            }
        }
        self.shelf = (sx + w, sy, sh);
        self.refs.push(SpriteRef { page: 0, src: Src { x: sx, y: sy, w, h }, ax: anchor.0, ay: anchor.1, height });
        (self.refs.len() - 1) as RefId
    }

    /// Packs a canvas's albedo through `remap` (a swap: ART.md §1, "swaps are index remaps").
    pub fn add_canvas(&mut self, c: &Canvas, anchor: (i16, i16), height: u8, remap: impl Fn(Ix) -> Ix) -> RefId {
        self.add(c.w() as u16, c.h() as u16, anchor, height, |x, y| remap(c.get(x, y)))
    }

    /// Packs a canvas at half size, 2:1 decimated: each 2 x 2 block takes its first opaque
    /// pixel, else the contact shadow if it has one, else clear.
    pub fn add_canvas_half(&mut self, c: &Canvas, anchor: (i16, i16), height: u8) -> RefId {
        self.add((c.w() / 2) as u16, (c.h() / 2) as u16, anchor, height, |x, y| {
            let block = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| c.get(2 * x + dx, 2 * y + dy));
            block
                .iter()
                .copied()
                .find(|i| i.is_opaque())
                .or_else(|| block.iter().copied().find(|&i| i == Ix::AO))
                .unwrap_or(Ix::CLEAR)
        })
    }

    pub fn get(&self, id: RefId) -> &SpriteRef {
        &self.refs[usize::from(id)]
    }
}

impl Default for Atlas {
    fn default() -> Self {
        Atlas::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clut_is_the_master_palette() {
        let a = Atlas::new();
        assert_eq!(a.pages.clut.len(), CLUT_LEN);
        let [r, g, b] = palette::rgb(Ix::INK);
        assert_eq!(
            a.pages.clut[usize::from(Ix::INK.0)],
            0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
        );
        const { assert!(palette::LEN > 256, "the palette outgrew 8-bit albedo: why the pages are u16") };
    }

    #[test]
    fn shelves_never_overlap() {
        let mut a = Atlas::new();
        let ids: Vec<RefId> =
            (0..40).map(|i| a.add(40 + (i % 7) * 13, 10 + (i % 5) * 9, (0, 0), 1, |_, _| Ix(2))).collect();
        for (i, &p) in ids.iter().enumerate() {
            for &q in &ids[i + 1..] {
                let (p, q) = (a.get(p).src, a.get(q).src);
                let apart = p.x + p.w <= q.x || q.x + q.w <= p.x || p.y + p.h <= q.y || q.y + q.h <= p.y;
                assert!(apart, "{p:?} overlaps {q:?}");
            }
        }
    }
}
