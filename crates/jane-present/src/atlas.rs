//! The atlas (PRESENTATION.md §1.4): pages of four layers (albedo and emissive as master-palette
//! indices, normal, height), packed at boot, and the sprite table the scene looks frames up in.
//! Nothing renders from a look during play. An atlas for `soft` alone packs the albedo only.

use jane_art::Canvas;
use jane_art::canvas::{FLAT, Normal};
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
    /// The tallest drawn px in its height layer, true px: how tall it throws its shadow (a
    /// garden bed seen from above is many rows tall on the screen and a hand high).
    pub top: u8,
}

/// Index into [`Atlas::refs`].
pub type RefId = u16;

/// One texel of the four layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texel {
    pub albedo: Ix,
    pub normal: Normal,
    pub emissive: Ix,
    pub height: u8,
}

impl Texel {
    /// Clear in every layer.
    pub const CLEAR: Texel = Texel { albedo: Ix::CLEAR, normal: FLAT, emissive: Ix::CLEAR, height: 0 };

    /// A canvas's texel at `(x, y)`.
    pub fn of(c: &Canvas, x: i32, y: i32) -> Texel {
        Texel {
            albedo: c.get(x, y),
            normal: c.normal_at(x, y),
            emissive: c.emissive_at(x, y),
            height: c.height_at(x, y),
        }
    }
}

/// The packed pages and their sprite table.
#[derive(Debug)]
pub struct Atlas {
    pub pages: AtlasPages,
    pub refs: Vec<SpriteRef>,
    /// The page being filled, and its shelf: x, y and height of the row being laid.
    shelf: (usize, u16, u16, u16),
    /// Whether the normal, emissive and height layers are packed (T1 and up).
    lit: bool,
}

/// The most texels a page has on a side: the largest texture every T1 and T2 machine takes (a
/// Pi 2 or 3's VC4 under GLES 2; `gl2` refuses a context that cannot, so this is every backend's
/// `Caps::max_texture` or less). Fixed, not the backend's cap, so the packing (and with it every
/// `SpriteCmd` of a `Frame`) is the same on every machine and every tier: `soft` and `gl2` draw
/// the same `Frame` (PRESENTATION.md §1.3), and the atlas is built before a backend exists.
pub const PAGE_SIDE: u16 = 2048;

/// The width of a page as packed here, and the most a page grows downward before the next is
/// begun.
const PAGE_W: u16 = PAGE_SIDE;
const PAGE_H: u16 = PAGE_SIDE;

impl Atlas {
    /// An empty atlas with the master palette as its CLUT, albedo only (for `soft`).
    pub fn new() -> Atlas {
        Atlas::with_layers(false)
    }

    /// An empty atlas; `lit` packs the normal, emissive and height layers beside the albedo.
    pub fn with_layers(lit: bool) -> Atlas {
        let mut clut = vec![0xff00_0000; CLUT_LEN];
        for (i, c) in palette::PALETTE.iter().enumerate() {
            clut[i] = 0xff00_0000 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
        }
        Atlas {
            pages: AtlasPages { clut, pages: vec![Page { w: PAGE_W, ..Page::default() }], mist: Vec::new() },
            refs: Vec::new(),
            shelf: (0, 0, 0, 0),
            lit,
        }
    }

    /// Whether the atlas packs all four layers.
    pub fn lit(&self) -> bool {
        self.lit
    }

    /// Packs a `w x h` grid of indices given by `px(x, y)`, flat and dark, one px above the
    /// ground where opaque; returns its id.
    pub fn add(&mut self, w: u16, h: u16, anchor: (i16, i16), height: u8, px: impl Fn(i32, i32) -> Ix) -> RefId {
        self.add_texels(w, h, anchor, height, |x, y| {
            let a = px(x, y);
            Texel { albedo: a, height: u8::from(a.is_opaque()), ..Texel::CLEAR }
        })
    }

    /// Packs a `w x h` grid of texels given by `px(x, y)`; returns its id.
    pub fn add_texels(
        &mut self,
        w: u16,
        h: u16,
        anchor: (i16, i16),
        height: u8,
        px: impl Fn(i32, i32) -> Texel,
    ) -> RefId {
        assert!(w <= PAGE_W && h <= PAGE_H, "a sprite larger than a page");
        let (mut pi, mut sx, mut sy, mut sh) = self.shelf;
        if sx + w > PAGE_W {
            sx = 0;
            sy += sh;
            sh = 0;
        }
        if sy + h > PAGE_H {
            // This page is full: the next one, from its top.
            pi += 1;
            (sx, sy, sh) = (0, 0, 0);
        }
        if pi == self.pages.pages.len() {
            self.pages.pages.push(Page { w: PAGE_W, ..Page::default() });
        }
        sh = sh.max(h);
        let lit = self.lit;
        let page = &mut self.pages.pages[pi];
        if sy + h > page.h {
            page.h = sy + h;
            let n = usize::from(page.w) * usize::from(page.h);
            page.albedo.resize(n, 0);
            if lit {
                page.normal.resize(n, FLAT);
                page.emissive.resize(n, 0);
                page.height.resize(n, 0);
            }
        }
        let mut top = 0u8;
        for y in 0..h {
            for x in 0..w {
                let i = usize::from(sy + y) * usize::from(page.w) + usize::from(sx + x);
                let t = px(i32::from(x), i32::from(y));
                if t.albedo.is_opaque() {
                    top = top.max(t.height);
                }
                page.albedo[i] = t.albedo.0;
                if lit {
                    page.normal[i] = t.normal;
                    page.emissive[i] = t.emissive.0;
                    page.height[i] = t.height;
                } else if t.albedo.is_opaque() && t.emissive.is_opaque() {
                    // T0 keeps what glows sparse: a few texels a lamp, a window.
                    page.glow.push((i as u32, t.emissive.0));
                }
            }
        }
        self.shelf = (pi, sx + w, sy, sh);
        self.refs.push(SpriteRef {
            page: pi as u8,
            src: Src { x: sx, y: sy, w, h },
            ax: anchor.0,
            ay: anchor.1,
            height,
            top,
        });
        (self.refs.len() - 1) as RefId
    }

    /// Packs a canvas's four layers, each texel through `map` (a swap is an albedo remap, ART.md
    /// §1, "swaps are index remaps"; a stand-in may reshape its height).
    pub fn add_canvas(
        &mut self,
        c: &Canvas,
        anchor: (i16, i16),
        height: u8,
        map: impl Fn(i32, i32, Texel) -> Texel,
    ) -> RefId {
        self.add_texels(c.w() as u16, c.h() as u16, anchor, height, |x, y| map(x, y, Texel::of(c, x, y)))
    }

    /// Packs a canvas at half size, 2:1 decimated: each 2 x 2 block takes its first opaque
    /// texel, else the contact shadow if it has one, else clear; `map` as in
    /// [`add_canvas`](Self::add_canvas), in the half-size coordinates.
    pub fn add_canvas_half(
        &mut self,
        c: &Canvas,
        anchor: (i16, i16),
        height: u8,
        map: impl Fn(i32, i32, Texel) -> Texel,
    ) -> RefId {
        self.add_texels((c.w() / 2) as u16, (c.h() / 2) as u16, anchor, height, |x, y| {
            let block = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| Texel::of(c, 2 * x + dx, 2 * y + dy));
            let t = block
                .iter()
                .copied()
                .find(|t| t.albedo.is_opaque())
                .or_else(|| block.iter().copied().find(|t| t.albedo == Ix::AO))
                .unwrap_or(Texel::CLEAR);
            map(x, y, Texel { height: t.height / 2, ..t })
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

    /// Every page of the whole game's atlas, the UI's with it, is at most 2048 on a side, and
    /// every sprite lies inside its own page: a Pi 2's VC4 can hold them all.
    #[test]
    fn everything_packs_under_a_2048_cap() {
        let p = crate::Present::new(crate::Tier::T1);
        let a = p.atlas();
        for (k, page) in a.pages.iter().enumerate() {
            assert!(page.w <= 2048 && page.h <= 2048, "page {k} is {} x {}", page.w, page.h);
            assert_eq!(page.albedo.len(), usize::from(page.w) * usize::from(page.h));
            assert!(page.lit(), "a T1 atlas carries four layers on page {k}");
        }
        // More than a page's worth forces a second, and nothing crosses a page's edge.
        let mut at = Atlas::with_layers(false);
        let ids: Vec<RefId> =
            (0..900).map(|i| at.add(200 + (i % 3) * 30, 90 + (i % 4) * 11, (0, 0), 1, |_, _| Ix(2))).collect();
        assert!(at.pages.pages.len() > 2);
        for &id in &ids {
            let r = at.get(id);
            let page = &at.pages.pages[usize::from(r.page)];
            assert!(page.w <= PAGE_SIDE && page.h <= PAGE_SIDE);
            assert!(r.src.x + r.src.w <= page.w && r.src.y + r.src.h <= page.h, "{r:?}");
        }
        for (i, &p) in ids.iter().enumerate() {
            for &q in &ids[i + 1..] {
                let (p, q) = (at.get(p), at.get(q));
                if p.page != q.page {
                    continue;
                }
                let (p, q) = (p.src, q.src);
                let apart = p.x + p.w <= q.x || q.x + q.w <= p.x || p.y + p.h <= q.y || q.y + q.h <= p.y;
                assert!(apart, "{p:?} overlaps {q:?}");
            }
        }
    }

    #[test]
    fn a_lit_atlas_packs_four_layers_and_a_soft_one_the_albedo_alone() {
        let lamp = jane_art::demo::lamp();
        for lit in [false, true] {
            let mut a = Atlas::with_layers(lit);
            let id = a.add_canvas(&lamp, (0, 48), 48, |_, _, t| t);
            let page = &a.pages.pages[0];
            assert_eq!(page.lit(), lit);
            if lit {
                let r = a.get(id).src;
                let (x, y) = (16, 14);
                let i = (usize::from(r.y) + y) * usize::from(page.w) + usize::from(r.x) + x;
                assert_eq!(page.emissive[i], lamp.emissive_at(x as i32, y as i32).0);
                assert!(page.emissive[i] != 0, "the lamp's glass glows");
                assert_eq!(page.height[i], lamp.height_at(x as i32, y as i32));
                assert_eq!(page.normal[i], lamp.normal_at(x as i32, y as i32));
            } else {
                assert!(page.normal.is_empty() && page.emissive.is_empty() && page.height.is_empty());
            }
        }
    }

    /// T0's atlas has no emissive layer but keeps what glows sparse (§1.3 `glow`): lamp glass and
    /// lit windows, each on an opaque texel.
    #[test]
    fn a_t0_atlas_keeps_what_glows_sparse() {
        let p = crate::Present::new(crate::Tier::T0);
        let pages = &p.atlas().pages;
        let n: usize = pages.iter().map(|g| g.glow.len()).sum();
        assert!(n > 100 && n < 200_000, "{n} glowing texels");
        for g in pages {
            assert!(!g.lit());
            for &(i, e) in &g.glow {
                assert!(e > 1 && g.albedo[i as usize] > 1);
            }
        }
    }
}
