//! The UI's atlas page (ART.md §6, §7): every glyph of the stroke font in its four faces, regular
//! and bold; the conic sweeps of the slots; the marks (cursors, pad buttons, the sun and the
//! moon); and the item and spell icons. Built once at boot into its own page, which the
//! presenter appends to its atlas, so every backend that uploads the atlas has it.
//!
//! Glyphs are packed as one-bit masks in a single opaque index and always drawn with an `ink`
//! ([`super::cmd::UiCmd::Sprite`]); the marks and icons keep their own colours.

use jane_art::canvas::Canvas;
use jane_art::chrome;
use jane_art::font::{self, Face};
use jane_art::palette::{Ix, Ramp, Tone};
use jane_core::ids::SpriteId;

use crate::backend::Page;
use crate::frame::Src;
use crate::ui::icons;

/// The page is this wide; it grows downward as it fills.
const PAGE_W: u16 = 1024;
/// The index a glyph's ink is packed as (never shown: glyphs are drawn with an ink).
const GLYPH: Ix = Ix::INK;
/// Steps of a slot's cooldown sweep, a full turn.
pub const SWEEP_STEPS: u16 = 48;
/// The sizes of slot a sweep is cut for: the bar and bag (36), a status chip (20).
pub const SWEEP_SIZES: [i32; 2] = [36, 20];

/// A mark: a small picture the chrome draws in its own colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Mark {
    Arrow,
    Reticle,
    Hand,
    Sun,
    Moon,
    PadA,
    PadB,
    PadX,
    PadY,
    PadShoulder,
    PadStick,
    Diamond,
    Skull,
    Lock,
    Heart,
    Flake,
    Bolt,
    Star,
}

impl Mark {
    pub const ALL: [Mark; 18] = [
        Mark::Arrow,
        Mark::Reticle,
        Mark::Hand,
        Mark::Sun,
        Mark::Moon,
        Mark::PadA,
        Mark::PadB,
        Mark::PadX,
        Mark::PadY,
        Mark::PadShoulder,
        Mark::PadStick,
        Mark::Diamond,
        Mark::Skull,
        Mark::Lock,
        Mark::Heart,
        Mark::Flake,
        Mark::Bolt,
        Mark::Star,
    ];
}

/// Where everything on the UI page is.
#[derive(Clone, Debug, Default)]
pub struct UiArt {
    /// The page's index in the atlas.
    pub page: u8,
    /// `[face][bold][glyph slot]`, flattened.
    glyphs: Vec<Src>,
    n_glyphs: usize,
    /// Per face, the glyph box's offset inside its cell.
    insets: [(i16, i16); 4],
    /// Per sweep size, `SWEEP_STEPS + 1` frames (0 is nothing left, the last a full turn).
    sweeps: Vec<Vec<Src>>,
    marks: Vec<Src>,
    /// Item and spell icons by the sprite id their rows name, sorted; 32 px.
    icons: Vec<(SpriteId, Src)>,
    /// The same icons at 16 px (chips, the tracker, the map's legend).
    icons_small: Vec<(SpriteId, Src)>,
    icon_blank: Src,
    icon_blank_small: Src,
}

/// A shelf packer onto one page.
struct Packer {
    page: Page,
    shelf: (u16, u16, u16),
}

impl Packer {
    fn new() -> Packer {
        Packer { page: Page { w: PAGE_W, ..Page::default() }, shelf: (0, 0, 0) }
    }

    /// Pack a `w x h` grid of indices given by `px(x, y)`, with one texel of clear around it so
    /// a stretched or filtered sample never bleeds a neighbour.
    fn add(&mut self, w: i32, h: i32, px: impl Fn(i32, i32) -> Ix) -> Src {
        let (w, h) = (w as u16, h as u16);
        let (bw, bh) = (w + 2, h + 2);
        let (mut sx, mut sy, mut sh) = self.shelf;
        if sx + bw > PAGE_W {
            sx = 0;
            sy += sh;
            sh = 0;
        }
        sh = sh.max(bh);
        let p = &mut self.page;
        let albedo = std::sync::Arc::make_mut(&mut p.albedo);
        if sy + bh > p.h {
            p.h = sy + bh;
            albedo.resize(usize::from(p.w) * usize::from(p.h), 0);
        }
        for y in 0..h {
            for x in 0..w {
                let i = usize::from(sy + 1 + y) * usize::from(p.w) + usize::from(sx + 1 + x);
                albedo[i] = px(i32::from(x), i32::from(y)).0;
            }
        }
        self.shelf = (sx + bw, sy, sh);
        Src { x: sx + 1, y: sy + 1, w, h }
    }

    fn add_canvas(&mut self, c: &Canvas) -> Src {
        self.add(c.w(), c.h(), |x, y| c.get(x, y))
    }
}

impl UiArt {
    /// Draws and packs everything; returns the table and the page, which the caller appends to
    /// the atlas as page `page`.
    pub fn build(page: u8) -> (UiArt, Page) {
        let mut p = Packer::new();
        let chars: Vec<char> = font::chars().collect();
        let mut glyphs = Vec::with_capacity(Face::ALL.len() * 2 * chars.len());
        let mut insets = [(0, 0); 4];
        for (fi, &face) in Face::ALL.iter().enumerate() {
            let (cw, ch) = face.cell();
            let plain = font::rasterise(font::strokes('M').unwrap_or(&[]), face, false);
            insets[fi] = (((cw - plain.w) / 2) as i16, ((ch - plain.h) / 2) as i16);
            for bold in [false, true] {
                for &c in &chars {
                    let g = font::rasterise(font::strokes(c).unwrap_or(&[]), face, bold);
                    glyphs.push(p.add(g.w, g.h, |x, y| if g.at(x, y) { GLYPH } else { Ix::CLEAR }));
                }
            }
        }
        let sweeps = SWEEP_SIZES
            .iter()
            .map(|&s| {
                (0..=SWEEP_STEPS)
                    .map(|k| p.add_canvas(&chrome::sweep(s, i32::from(k) * 1000 / i32::from(SWEEP_STEPS))))
                    .collect()
            })
            .collect();
        let marks = Mark::ALL.iter().map(|&m| p.add_canvas(&icons::mark(m))).collect();
        let cat = jane_data::catalog();
        let mut named: Vec<SpriteId> = cat.combat.items.iter().map(|i| i.icon).collect();
        named.extend(cat.combat.spells.iter().map(|s| s.icon));
        named.extend(cat.combat.effects.iter().map(|e| e.icon));
        named.sort_unstable();
        named.dedup();
        let mut icons = Vec::with_capacity(named.len());
        let mut icons_small = Vec::with_capacity(named.len());
        for id in named {
            let name = cat.sprites.get(usize::from(id.0)).copied().unwrap_or("");
            let (big, small) = icons::both(name);
            icons.push((id, p.add_canvas(&big)));
            icons_small.push((id, p.add_canvas(&small)));
        }
        let blank = icons::icon("");
        let icon_blank = p.add_canvas(&blank);
        let icon_blank_small = p.add_canvas(&icons::half(&blank));
        let art = UiArt {
            page,
            n_glyphs: chars.len(),
            glyphs,
            insets,
            sweeps,
            marks,
            icons,
            icons_small,
            icon_blank,
            icon_blank_small,
        };
        (art, p.page)
    }

    /// The glyph for `c` in `face`, and where its box sits in its cell; `?` for a character the
    /// font lacks.
    pub fn glyph(&self, face: Face, bold: bool, c: char) -> (Src, (i16, i16)) {
        let slot = match c {
            ' '..='~' => c as usize - 32,
            _ => font::EXTRAS.iter().position(|&e| e == c).map_or('?' as usize - 32, |i| 95 + i),
        };
        let fi = face as usize;
        (self.glyphs[(fi * 2 + usize::from(bold)) * self.n_glyphs + slot], self.insets[fi])
    }

    /// The sweep frame for a slot of `size` px with `left` permille of a turn still to run.
    pub fn sweep(&self, size: i32, left: u16) -> Option<Src> {
        let i = SWEEP_SIZES.iter().position(|&s| s == size)?;
        let k = (u32::from(left.min(1000)) * u32::from(SWEEP_STEPS)).div_ceil(1000);
        self.sweeps[i].get(k as usize).copied()
    }

    pub fn mark(&self, m: Mark) -> Src {
        self.marks[m as usize]
    }

    /// An item's or a spell's icon at 32 px (a blank parcel for an id with none).
    pub fn icon(&self, id: SpriteId) -> Src {
        self.icons.binary_search_by_key(&id, |e| e.0).map_or(self.icon_blank, |i| self.icons[i].1)
    }

    /// The same at 16 px.
    pub fn icon_small(&self, id: SpriteId) -> Src {
        self.icons_small.binary_search_by_key(&id, |e| e.0).map_or(self.icon_blank_small, |i| self.icons_small[i].1)
    }
}

/// An ink that reads as the chrome's "lit" edge on a mark (for tests and the sheet).
pub fn mark_ink() -> Ix {
    Ramp::UiGold.at(Tone::Light)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_and_mark_is_on_the_page_and_apart() {
        let (art, page) = UiArt::build(1);
        let mut rects: Vec<Src> = art.glyphs.clone();
        rects.extend(art.marks.iter().copied());
        rects.extend(art.icons.iter().map(|e| e.1));
        for (i, a) in rects.iter().enumerate() {
            assert!(a.x + a.w <= page.w && a.y + a.h <= page.h, "{a:?} off the page");
            for b in &rects[i + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
        // The Small face's M is 10 x 16 and sits in its 12 x 18 cell by (1, 1).
        let (m, inset) = art.glyph(Face::Small, false, 'M');
        assert_eq!((m.w, m.h, inset), (10, 16, (1, 1)));
        assert_eq!(art.glyph(Face::Fine, false, '\u{1F600}').0, art.glyph(Face::Fine, false, '?').0);
    }

    #[test]
    fn a_sweep_rounds_up_so_a_cooldown_never_shows_done_early() {
        let (art, _) = UiArt::build(1);
        assert_eq!(art.sweep(36, 0), Some(art.sweeps[0][0]));
        assert_eq!(art.sweep(36, 1), Some(art.sweeps[0][1]));
        assert_eq!(art.sweep(36, 1000), Some(art.sweeps[0][SWEEP_STEPS as usize]));
        assert_eq!(art.sweep(20, 500), Some(art.sweeps[1][SWEEP_STEPS as usize / 2]));
        assert_eq!(art.sweep(24, 500), None);
    }
}
