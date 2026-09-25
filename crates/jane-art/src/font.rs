//! The font (ART.md §6): stroke-defined glyphs rasterised at boot. No bitmap in source and no
//! system font.
//!
//! A glyph is a list of segments with integer endpoints on a 5 x 8 lattice: columns 0 to 4, cap
//! height 6 (rows 0 to 5), baseline on row 5, lower-case bodies on rows 2 to 5, descenders on
//! rows 6 and 7. A face scales the lattice and draws each segment with a Bresenham [`pen`] of
//! its width; from a 3 px pen up the pen's corners are rounded.
//!
//! | Face | Lattice | Pen | Cell (advance x line) |
//! | --- | --- | --- | --- |
//! | Fine | 1x | 1 px | 8 x 12 |
//! | Small | 2x | 2 px | 12 x 18 |
//! | Head | 3x | 3 px | 18 x 27 |
//! | Title | 5x | 4 px | 30 x 45 |

use crate::canvas::{Canvas, bresenham, pen};
use crate::palette::Ix;

/// A stroke from lattice point `(x0, y0)` to `(x1, y1)`; a point when the ends meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seg(pub u8, pub u8, pub u8, pub u8);

/// A face: one scale of the same strokes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// 1x: dense UI, the terminal, lists, tooltips, map labels (96 x 36 at 768 x 432).
    Fine,
    /// 2x: body text, HUD, dialogue (64 x 24 at 768 x 432).
    Small,
    /// 3x: headings, crits, the zone banner.
    Head,
    /// 5x: the title.
    Title,
}

impl Face {
    /// Every face, smallest first.
    pub const ALL: [Face; 4] = [Face::Fine, Face::Small, Face::Head, Face::Title];

    /// Px per lattice step.
    pub const fn scale(self) -> i32 {
        match self {
            Face::Fine => 1,
            Face::Small => 2,
            Face::Head => 3,
            Face::Title => 5,
        }
    }

    /// Pen width in px (bold adds one).
    pub const fn pen(self) -> i32 {
        match self {
            Face::Fine => 1,
            Face::Small => 2,
            Face::Head => 3,
            Face::Title => 4,
        }
    }

    /// The character cell in px, `(advance, line)`: text is monospaced.
    pub const fn cell(self) -> (i32, i32) {
        match self {
            Face::Fine => (8, 12),
            Face::Small => (12, 18),
            Face::Head => (18, 27),
            Face::Title => (30, 45),
        }
    }

    /// The offset in px of the shadow pass: 1, or 2 for Head and Title.
    pub const fn shadow(self) -> i32 {
        match self {
            Face::Fine | Face::Small => 1,
            Face::Head | Face::Title => 2,
        }
    }

    /// The glyph box in px for a pen `bold` px wider.
    const fn glyph_size(self, bold: bool) -> (i32, i32) {
        let p = self.pen() + bold as i32;
        (4 * self.scale() + p, 7 * self.scale() + p)
    }

    /// Where the glyph box sits in its cell, px from the cell's top-left.
    const fn inset(self) -> (i32, i32) {
        let (cw, ch) = self.cell();
        let (gw, gh) = self.glyph_size(false);
        ((cw - gw) / 2, (ch - gh) / 2)
    }
}

/// How a run of text is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// Which face.
    pub face: Face,
    /// The text's colour.
    pub ink: Ix,
    /// One px more pen.
    pub bold: bool,
    /// A second pass in `k` offset down and right by [`Face::shadow`].
    pub shadow: bool,
    /// A 4-way dilate in `k` under the ink.
    pub outline: bool,
}

impl Style {
    /// Plain `ink` in `face`.
    pub const fn plain(face: Face, ink: Ix) -> Style {
        Style { face, ink, bold: false, shadow: false, outline: false }
    }
}

/// The glyphs beyond ASCII 32 to 126: the pound, four arrows, bullet, degree, and the marks the
/// UI and the viewer use (middle dot, square, down triangle, diamond, disc, tick, cross,
/// infinity).
pub const EXTRAS: [char; 15] = ['£', '←', '↑', '→', '↓', '•', '°', '·', '■', '▼', '◆', '●', '✓', '✗', '∞'];

/// Glyphs that reach rows 6 and 7; no other glyph does.
pub const DESCENDERS: [char; 8] = ['g', 'j', 'p', 'q', 'y', ',', ';', '|'];

/// Every character the font has, ASCII first.
pub fn chars() -> impl Iterator<Item = char> {
    (32u8..=126).map(char::from).chain(EXTRAS)
}

fn slot(ch: char) -> Option<usize> {
    match ch {
        ' '..='~' => Some(ch as usize - 32),
        _ => EXTRAS.iter().position(|&e| e == ch).map(|i| 95 + i),
    }
}

/// The strokes of `ch`, or `None` if the font has no such glyph.
#[rustfmt::skip]
pub fn strokes(ch: char) -> Option<&'static [Seg]> {
    Some(match ch {
        ' ' => &[],
        '!' => &[Seg(2, 0, 2, 3), Seg(2, 5, 2, 5)],
        '"' => &[Seg(1, 0, 1, 1), Seg(3, 0, 3, 1)],
        '#' => &[Seg(1, 0, 1, 5), Seg(3, 0, 3, 5), Seg(0, 1, 4, 1), Seg(0, 4, 4, 4)],
        '$' => &[Seg(2, 0, 2, 4), Seg(1, 1, 4, 1), Seg(0, 2, 0, 2), Seg(1, 3, 3, 3), Seg(4, 4, 4, 4), Seg(0, 5, 3, 5), Seg(0, 2, 1, 3), Seg(1, 1, 0, 2), Seg(3, 3, 4, 4), Seg(4, 4, 3, 5)],
        '%' => &[Seg(0, 0, 1, 0), Seg(0, 1, 1, 1), Seg(4, 0, 0, 5), Seg(3, 4, 4, 4), Seg(3, 5, 4, 5), Seg(1, 1, 2, 2), Seg(2, 3, 3, 4)],
        '&' => &[Seg(1, 0, 2, 0), Seg(0, 1, 0, 1), Seg(3, 1, 3, 1), Seg(1, 2, 4, 5), Seg(0, 3, 0, 4), Seg(1, 5, 2, 5), Seg(4, 3, 3, 4), Seg(0, 1, 1, 2), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(1, 2, 0, 3), Seg(2, 0, 3, 1), Seg(3, 4, 2, 5)],
        '\'' => &[Seg(2, 0, 2, 1), Seg(1, 2, 1, 2), Seg(2, 1, 1, 2)],
        '(' => &[Seg(3, 0, 3, 0), Seg(2, 1, 2, 4), Seg(3, 5, 3, 5), Seg(2, 4, 3, 5), Seg(3, 0, 2, 1)],
        ')' => &[Seg(1, 0, 1, 0), Seg(2, 1, 2, 4), Seg(1, 5, 1, 5), Seg(1, 0, 2, 1), Seg(2, 4, 1, 5)],
        '*' => &[Seg(1, 1, 3, 3), Seg(3, 1, 1, 3), Seg(0, 2, 4, 2)],
        '+' => &[Seg(2, 1, 2, 5), Seg(0, 3, 4, 3)],
        ',' => &[Seg(2, 4, 2, 5), Seg(1, 6, 1, 6), Seg(2, 5, 1, 6)],
        '-' => &[Seg(1, 3, 3, 3)],
        '.' => &[Seg(1, 4, 2, 4), Seg(1, 5, 2, 5)],
        '/' => &[Seg(4, 0, 0, 5)],
        '0' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 4), Seg(4, 1, 4, 4), Seg(1, 5, 3, 5), Seg(3, 2, 1, 4), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(4, 4, 3, 5)],
        '1' => &[Seg(1, 1, 2, 0), Seg(2, 0, 2, 5), Seg(1, 5, 3, 5)],
        '2' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 1), Seg(4, 1, 4, 1), Seg(3, 2, 1, 4), Seg(0, 5, 4, 5), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(4, 1, 3, 2)],
        '3' => &[Seg(0, 0, 3, 0), Seg(4, 1, 4, 1), Seg(1, 2, 3, 2), Seg(4, 3, 4, 4), Seg(0, 5, 3, 5), Seg(3, 0, 4, 1), Seg(3, 2, 4, 3), Seg(4, 1, 3, 2), Seg(4, 4, 3, 5)],
        '4' => &[Seg(0, 0, 0, 2), Seg(0, 2, 4, 2), Seg(3, 0, 3, 5)],
        '5' => &[Seg(0, 0, 4, 0), Seg(0, 1, 0, 2), Seg(1, 2, 3, 2), Seg(4, 3, 4, 4), Seg(0, 4, 0, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(3, 2, 4, 3), Seg(4, 4, 3, 5)],
        '6' => &[Seg(2, 0, 3, 0), Seg(1, 1, 1, 1), Seg(0, 2, 0, 4), Seg(1, 3, 3, 3), Seg(4, 4, 4, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(1, 1, 0, 2), Seg(2, 0, 1, 1), Seg(3, 3, 4, 4), Seg(4, 4, 3, 5)],
        '7' => &[Seg(0, 0, 4, 0), Seg(4, 1, 4, 1), Seg(3, 2, 3, 2), Seg(2, 3, 2, 5), Seg(3, 2, 2, 3), Seg(4, 1, 3, 2)],
        '8' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 1), Seg(4, 1, 4, 1), Seg(1, 2, 3, 2), Seg(0, 3, 0, 4), Seg(4, 3, 4, 4), Seg(1, 5, 3, 5), Seg(0, 1, 1, 2), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(1, 2, 0, 3), Seg(3, 0, 4, 1), Seg(3, 2, 4, 3), Seg(4, 1, 3, 2), Seg(4, 4, 3, 5)],
        '9' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 2), Seg(4, 1, 4, 4), Seg(1, 3, 3, 3), Seg(1, 5, 3, 5), Seg(0, 2, 1, 3), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(4, 4, 3, 5)],
        ':' => &[Seg(1, 1, 2, 1), Seg(1, 2, 2, 2), Seg(1, 4, 2, 4), Seg(1, 5, 2, 5)],
        ';' => &[Seg(1, 1, 2, 1), Seg(1, 2, 2, 2), Seg(2, 4, 2, 5), Seg(1, 6, 1, 6), Seg(2, 5, 1, 6)],
        '<' => &[Seg(3, 1, 1, 3), Seg(1, 3, 3, 5)],
        '=' => &[Seg(0, 2, 4, 2), Seg(0, 4, 4, 4)],
        '>' => &[Seg(1, 1, 3, 3), Seg(3, 3, 1, 5)],
        '?' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 1), Seg(4, 1, 4, 1), Seg(3, 2, 3, 2), Seg(2, 3, 2, 3), Seg(2, 5, 2, 5), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(3, 2, 2, 3), Seg(4, 1, 3, 2)],
        '@' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 4), Seg(4, 1, 4, 3), Seg(2, 2, 4, 2), Seg(2, 2, 2, 4), Seg(3, 4, 3, 4), Seg(1, 5, 4, 5), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(4, 3, 3, 4)],
        'A' => &[Seg(0, 5, 0, 1), Seg(0, 1, 1, 0), Seg(1, 0, 3, 0), Seg(3, 0, 4, 1), Seg(4, 1, 4, 5), Seg(0, 3, 4, 3)],
        'B' => &[Seg(0, 0, 0, 5), Seg(0, 0, 3, 0), Seg(4, 1, 4, 1), Seg(0, 2, 3, 2), Seg(4, 3, 4, 4), Seg(0, 5, 3, 5), Seg(3, 0, 4, 1), Seg(3, 2, 4, 3), Seg(4, 1, 3, 2), Seg(4, 4, 3, 5)],
        'C' => &[Seg(1, 0, 4, 0), Seg(0, 1, 0, 4), Seg(1, 5, 4, 5), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1)],
        'D' => &[Seg(0, 0, 0, 5), Seg(0, 0, 2, 0), Seg(2, 0, 4, 2), Seg(4, 2, 4, 3), Seg(4, 3, 2, 5), Seg(0, 5, 2, 5)],
        'E' => &[Seg(0, 0, 0, 5), Seg(0, 0, 4, 0), Seg(0, 2, 3, 2), Seg(0, 5, 4, 5)],
        'F' => &[Seg(0, 0, 0, 5), Seg(0, 0, 4, 0), Seg(0, 2, 3, 2)],
        'G' => &[Seg(1, 0, 4, 0), Seg(0, 1, 0, 4), Seg(1, 5, 3, 5), Seg(4, 4, 4, 3), Seg(2, 3, 4, 3), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(4, 4, 3, 5)],
        'H' => &[Seg(0, 0, 0, 5), Seg(4, 0, 4, 5), Seg(0, 2, 4, 2)],
        'I' => &[Seg(1, 0, 3, 0), Seg(2, 0, 2, 5), Seg(1, 5, 3, 5)],
        'J' => &[Seg(2, 0, 4, 0), Seg(3, 0, 3, 4), Seg(0, 4, 0, 4), Seg(1, 5, 2, 5), Seg(0, 4, 1, 5), Seg(3, 4, 2, 5)],
        'K' => &[Seg(0, 0, 0, 5), Seg(1, 2, 2, 2), Seg(3, 1, 4, 0), Seg(3, 3, 4, 4), Seg(4, 4, 4, 5), Seg(2, 2, 3, 3), Seg(3, 1, 2, 2)],
        'L' => &[Seg(0, 0, 0, 5), Seg(0, 5, 4, 5)],
        'M' => &[Seg(0, 0, 0, 5), Seg(4, 0, 4, 5), Seg(1, 1, 1, 1), Seg(3, 1, 3, 1), Seg(2, 2, 2, 3), Seg(1, 1, 2, 2), Seg(3, 1, 2, 2)],
        'N' => &[Seg(0, 0, 0, 5), Seg(4, 0, 4, 5), Seg(1, 1, 3, 3)],
        'O' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 4), Seg(4, 1, 4, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1), Seg(4, 4, 3, 5)],
        'P' => &[Seg(0, 0, 0, 5), Seg(0, 0, 3, 0), Seg(4, 1, 4, 2), Seg(0, 3, 3, 3), Seg(3, 0, 4, 1), Seg(4, 2, 3, 3)],
        'Q' => &[Seg(1, 0, 3, 0), Seg(0, 1, 0, 4), Seg(4, 1, 4, 4), Seg(1, 5, 3, 5), Seg(2, 3, 4, 5), Seg(0, 4, 1, 5), Seg(1, 0, 0, 1), Seg(3, 0, 4, 1)],
        'R' => &[Seg(0, 0, 0, 5), Seg(0, 0, 3, 0), Seg(4, 1, 4, 1), Seg(0, 2, 3, 2), Seg(2, 3, 4, 5), Seg(3, 0, 4, 1), Seg(4, 1, 3, 2)],
        'S' => &[Seg(1, 0, 4, 0), Seg(0, 1, 0, 1), Seg(1, 2, 3, 2), Seg(4, 3, 4, 4), Seg(0, 5, 3, 5), Seg(0, 1, 1, 2), Seg(1, 0, 0, 1), Seg(3, 2, 4, 3), Seg(4, 4, 3, 5)],
        'T' => &[Seg(0, 0, 4, 0), Seg(2, 0, 2, 5)],
        'U' => &[Seg(0, 0, 0, 4), Seg(4, 0, 4, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(4, 4, 3, 5)],
        'V' => &[Seg(0, 0, 0, 2), Seg(4, 0, 4, 2), Seg(1, 3, 1, 4), Seg(3, 3, 3, 4), Seg(2, 5, 2, 5), Seg(0, 2, 1, 3), Seg(1, 4, 2, 5), Seg(3, 4, 2, 5), Seg(4, 2, 3, 3)],
        'W' => &[Seg(0, 0, 0, 5), Seg(4, 0, 4, 5), Seg(2, 3, 2, 3), Seg(1, 4, 1, 4), Seg(3, 4, 3, 4), Seg(2, 3, 3, 4), Seg(2, 3, 1, 4)],
        'X' => &[Seg(0, 0, 2, 2), Seg(4, 0, 2, 2), Seg(2, 3, 0, 5), Seg(2, 3, 4, 5)],
        'Y' => &[Seg(0, 0, 2, 2), Seg(4, 0, 2, 2), Seg(2, 2, 2, 5)],
        'Z' => &[Seg(0, 0, 4, 0), Seg(4, 1, 0, 5), Seg(0, 5, 4, 5)],
        '[' => &[Seg(1, 0, 3, 0), Seg(1, 0, 1, 5), Seg(1, 5, 3, 5)],
        '\\' => &[Seg(0, 0, 4, 5)],
        ']' => &[Seg(1, 0, 3, 0), Seg(3, 0, 3, 5), Seg(1, 5, 3, 5)],
        '^' => &[Seg(0, 2, 2, 0), Seg(2, 0, 4, 2)],
        '_' => &[Seg(0, 5, 4, 5)],
        '`' => &[Seg(1, 0, 3, 2)],
        'a' => &[Seg(1, 2, 2, 2), Seg(3, 2, 3, 5), Seg(0, 3, 0, 4), Seg(1, 5, 2, 5), Seg(4, 5, 4, 5), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3)],
        'b' => &[Seg(0, 0, 0, 5), Seg(1, 2, 3, 2), Seg(4, 3, 4, 4), Seg(1, 5, 3, 5), Seg(3, 2, 4, 3), Seg(4, 4, 3, 5)],
        'c' => &[Seg(1, 2, 4, 2), Seg(0, 3, 0, 4), Seg(1, 5, 4, 5), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3)],
        'd' => &[Seg(4, 0, 4, 5), Seg(1, 2, 3, 2), Seg(0, 3, 0, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3)],
        'e' => &[Seg(1, 2, 3, 2), Seg(0, 3, 4, 3), Seg(0, 4, 0, 4), Seg(1, 5, 4, 5), Seg(0, 4, 1, 5)],
        'f' => &[Seg(2, 0, 3, 0), Seg(1, 1, 1, 5), Seg(0, 2, 2, 2), Seg(2, 0, 1, 1)],
        'g' => &[Seg(1, 2, 4, 2), Seg(0, 3, 0, 4), Seg(4, 2, 4, 6), Seg(1, 5, 4, 5), Seg(0, 7, 3, 7), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3), Seg(4, 6, 3, 7)],
        'h' => &[Seg(0, 0, 0, 5), Seg(1, 2, 3, 2), Seg(4, 3, 4, 5), Seg(3, 2, 4, 3)],
        'i' => &[Seg(2, 0, 2, 0), Seg(1, 2, 2, 2), Seg(2, 3, 2, 4), Seg(1, 5, 3, 5)],
        'j' => &[Seg(3, 0, 3, 0), Seg(2, 2, 3, 2), Seg(3, 3, 3, 6), Seg(0, 6, 0, 6), Seg(1, 7, 2, 7), Seg(0, 6, 1, 7), Seg(3, 6, 2, 7)],
        'k' => &[Seg(0, 0, 0, 5), Seg(3, 2, 3, 2), Seg(1, 3, 2, 3), Seg(3, 4, 3, 4), Seg(4, 5, 4, 5), Seg(2, 3, 3, 4), Seg(3, 2, 2, 3), Seg(3, 4, 4, 5)],
        'l' => &[Seg(1, 0, 2, 0), Seg(2, 1, 2, 4), Seg(3, 5, 3, 5), Seg(2, 4, 3, 5)],
        'm' => &[Seg(0, 2, 0, 5), Seg(1, 2, 1, 2), Seg(3, 2, 3, 2), Seg(2, 3, 2, 5), Seg(4, 3, 4, 5), Seg(1, 2, 2, 3), Seg(3, 2, 4, 3), Seg(3, 2, 2, 3)],
        'n' => &[Seg(0, 2, 0, 5), Seg(2, 2, 3, 2), Seg(1, 3, 1, 3), Seg(4, 3, 4, 5), Seg(2, 2, 1, 3), Seg(3, 2, 4, 3)],
        'o' => &[Seg(1, 2, 3, 2), Seg(0, 3, 0, 4), Seg(4, 3, 4, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3), Seg(3, 2, 4, 3), Seg(4, 4, 3, 5)],
        'p' => &[Seg(0, 2, 0, 7), Seg(1, 2, 3, 2), Seg(4, 3, 4, 4), Seg(1, 5, 3, 5), Seg(3, 2, 4, 3), Seg(4, 4, 3, 5)],
        'q' => &[Seg(4, 2, 4, 7), Seg(1, 2, 3, 2), Seg(0, 3, 0, 4), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5), Seg(1, 2, 0, 3)],
        'r' => &[Seg(0, 2, 0, 5), Seg(1, 3, 1, 3), Seg(2, 2, 4, 2), Seg(2, 2, 1, 3)],
        's' => &[Seg(4, 2, 1, 2), Seg(1, 2, 0, 3), Seg(0, 3, 1, 3), Seg(1, 3, 3, 4), Seg(3, 4, 4, 4), Seg(4, 4, 3, 5), Seg(3, 5, 0, 5)],
        't' => &[Seg(1, 0, 1, 4), Seg(0, 2, 3, 2), Seg(2, 5, 3, 5), Seg(1, 4, 2, 5)],
        'u' => &[Seg(0, 2, 0, 4), Seg(4, 2, 4, 5), Seg(1, 5, 3, 5), Seg(0, 4, 1, 5)],
        'v' => &[Seg(0, 2, 0, 3), Seg(4, 2, 4, 3), Seg(1, 4, 1, 4), Seg(3, 4, 3, 4), Seg(2, 5, 2, 5), Seg(0, 3, 1, 4), Seg(1, 4, 2, 5), Seg(3, 4, 2, 5), Seg(4, 3, 3, 4)],
        'w' => &[Seg(0, 2, 0, 4), Seg(4, 2, 4, 4), Seg(2, 3, 2, 4), Seg(1, 5, 1, 5), Seg(3, 5, 3, 5), Seg(0, 4, 1, 5), Seg(2, 4, 3, 5), Seg(2, 4, 1, 5), Seg(4, 4, 3, 5)],
        'x' => &[Seg(0, 2, 3, 5), Seg(3, 2, 0, 5)],
        'y' => &[Seg(0, 2, 0, 4), Seg(4, 2, 4, 6), Seg(1, 5, 3, 5), Seg(0, 7, 3, 7), Seg(0, 4, 1, 5), Seg(4, 6, 3, 7)],
        'z' => &[Seg(0, 2, 4, 2), Seg(4, 2, 0, 5), Seg(0, 5, 4, 5)],
        '{' => &[Seg(2, 0, 3, 0), Seg(2, 1, 2, 4), Seg(1, 2, 1, 2), Seg(2, 5, 3, 5)],
        '|' => &[Seg(2, 0, 2, 7)],
        '}' => &[Seg(1, 0, 2, 0), Seg(2, 1, 2, 4), Seg(3, 2, 3, 2), Seg(1, 5, 2, 5)],
        '~' => &[Seg(0, 3, 1, 2), Seg(1, 2, 3, 4), Seg(3, 4, 4, 3)],
        '£' => &[Seg(2, 0, 3, 0), Seg(4, 1, 4, 1), Seg(1, 1, 1, 4), Seg(0, 2, 2, 2), Seg(0, 5, 4, 5), Seg(2, 0, 1, 1), Seg(3, 0, 4, 1)],
        '←' => &[Seg(0, 3, 4, 3), Seg(2, 1, 0, 3), Seg(0, 3, 2, 5)],
        '↑' => &[Seg(2, 1, 2, 5), Seg(0, 3, 2, 1), Seg(2, 1, 4, 3)],
        '→' => &[Seg(0, 3, 4, 3), Seg(2, 1, 4, 3), Seg(4, 3, 2, 5)],
        '↓' => &[Seg(2, 1, 2, 5), Seg(0, 3, 2, 5), Seg(2, 5, 4, 3)],
        '•' => {
            &[Seg(1, 2, 3, 2), Seg(1, 3, 3, 3), Seg(1, 4, 3, 4), Seg(1, 2, 1, 4), Seg(2, 2, 2, 4), Seg(3, 2, 3, 4)]
        }
        '°' => &[Seg(1, 0, 2, 0), Seg(0, 1, 0, 1), Seg(3, 1, 3, 1), Seg(1, 2, 2, 2), Seg(0, 1, 1, 2), Seg(1, 0, 0, 1), Seg(2, 0, 3, 1), Seg(3, 1, 2, 2)],
        '·' => &[Seg(1, 2, 2, 2), Seg(1, 3, 2, 3), Seg(1, 2, 1, 3), Seg(2, 2, 2, 3)],
        '■' => &[Seg(0, 1, 4, 1), Seg(0, 2, 4, 2), Seg(0, 3, 4, 3), Seg(0, 4, 4, 4), Seg(0, 5, 4, 5), Seg(0, 1, 0, 5), Seg(1, 1, 1, 5), Seg(2, 1, 2, 5), Seg(3, 1, 3, 5), Seg(4, 1, 4, 5)],
        '▼' => &[Seg(0, 2, 4, 2), Seg(1, 3, 3, 3), Seg(2, 4, 2, 4)],
        '◆' => {
            &[Seg(2, 1, 0, 3), Seg(0, 3, 2, 5), Seg(2, 5, 4, 3), Seg(4, 3, 2, 1), Seg(1, 3, 3, 3), Seg(2, 2, 2, 4)]
        }
        '●' => &[Seg(1, 1, 3, 1), Seg(0, 2, 4, 2), Seg(0, 3, 4, 3), Seg(0, 4, 4, 4), Seg(1, 5, 3, 5), Seg(0, 2, 0, 4), Seg(1, 1, 1, 5), Seg(2, 1, 2, 5), Seg(3, 1, 3, 5), Seg(4, 2, 4, 4)],
        '✓' => &[Seg(0, 3, 1, 4), Seg(1, 4, 1, 5), Seg(1, 5, 4, 1)],
        '✗' => &[Seg(0, 1, 4, 5), Seg(4, 1, 0, 5)],
        '∞' => &[Seg(0, 2, 0, 3), Seg(1, 1, 1, 1), Seg(1, 4, 1, 4), Seg(2, 2, 2, 3), Seg(3, 1, 3, 1), Seg(3, 4, 3, 4), Seg(4, 2, 4, 3), Seg(0, 3, 1, 4), Seg(1, 1, 2, 2), Seg(1, 1, 0, 2), Seg(2, 3, 3, 4), Seg(2, 3, 1, 4), Seg(3, 1, 4, 2), Seg(3, 1, 2, 2), Seg(4, 3, 3, 4)],
        _ => return None,
    })
}

/// One rasterised glyph: a 1-bit mask, row-major.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Width in px.
    pub w: i32,
    /// Height in px.
    pub h: i32,
    /// `w * h` ink flags.
    pub mask: Vec<bool>,
}

impl Glyph {
    /// Whether `(x, y)` is inked.
    pub fn at(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.mask[(y * self.w + x) as usize]
    }
}

/// Draw `segs` at `face`'s scale with its pen (one wider when `bold`).
pub fn rasterise(segs: &[Seg], face: Face, bold: bool) -> Glyph {
    let (w, h) = face.glyph_size(bold);
    let (s, p) = (face.scale(), face.pen() + i32::from(bold));
    let mut mask = vec![false; (w * h) as usize];
    for &Seg(x0, y0, x1, y1) in segs {
        let (x0, y0, x1, y1) = (i32::from(x0) * s, i32::from(y0) * s, i32::from(x1) * s, i32::from(y1) * s);
        bresenham(x0, y0, x1, y1, |x, y| {
            for (dx, dy) in pen(p) {
                let (px, py) = (x + dx, y + dy);
                if px < w && py < h {
                    mask[(py * w + px) as usize] = true;
                }
            }
        });
    }
    Glyph { w, h, mask }
}

/// Every glyph in every face, regular and bold, rasterised once at boot.
#[derive(Clone, Debug)]
pub struct Font {
    /// `[face][bold][slot]`.
    glyphs: Vec<[Vec<Glyph>; 2]>,
}

impl Font {
    /// Rasterise every glyph of every face.
    pub fn build() -> Font {
        let glyphs = Face::ALL
            .iter()
            .map(|&f| {
                let set = |bold| chars().map(|c| rasterise(strokes(c).unwrap_or(&[]), f, bold)).collect();
                [set(false), set(true)]
            })
            .collect();
        Font { glyphs }
    }

    /// The glyph for `ch` in `face`; `?` for a character the font lacks.
    pub fn glyph(&self, face: Face, bold: bool, ch: char) -> &Glyph {
        let i = slot(ch).or_else(|| slot('?')).unwrap_or(0);
        &self.glyphs[face as usize][usize::from(bold)][i]
    }

    /// The advance in px: every glyph of a face is one cell wide.
    pub const fn advance(face: Face) -> i32 {
        face.cell().0
    }

    /// The width of `text` in px.
    pub fn measure(face: Face, text: &str) -> i32 {
        text.chars().count() as i32 * Self::advance(face)
    }

    /// `text` broken greedily on spaces into lines at most `width` px wide. A word wider than a
    /// line gets a line of its own.
    pub fn wrap(face: Face, text: &str, width: i32) -> Vec<&str> {
        let cols = (width / Self::advance(face)).max(1) as usize;
        let mut lines = Vec::new();
        let mut start: Option<usize> = None;
        let mut end = 0;
        let mut len = 0;
        for (i, word) in text.split(' ').scan(0usize, |at, w| {
            let i = *at;
            *at += w.len() + 1;
            Some((i, w))
        }) {
            if word.is_empty() {
                continue;
            }
            let n = word.chars().count();
            match start {
                Some(_) if len + 1 + n <= cols => {
                    len += 1 + n;
                    end = i + word.len();
                }
                Some(s) => {
                    lines.push(&text[s..end]);
                    (start, end, len) = (Some(i), i + word.len(), n);
                }
                None => (start, end, len) = (Some(i), i + word.len(), n),
            }
        }
        if let Some(s) = start {
            lines.push(&text[s..end]);
        }
        lines
    }

    /// Draw `text` with the top-left of its first cell at `(x, y)`; returns the x after it.
    /// Passes: the shadow, then the outline, then the ink.
    pub fn draw(&self, c: &mut Canvas, x: i32, y: i32, text: &str, style: Style) -> i32 {
        let f = style.face;
        let (ix, iy) = f.inset();
        let adv = Self::advance(f);
        let mut passes: Vec<(i32, i32, Ix)> = Vec::new();
        if style.shadow {
            passes.push((f.shadow(), f.shadow(), Ix::INK));
        }
        if style.outline {
            passes.extend([(1, 0, Ix::INK), (-1, 0, Ix::INK), (0, 1, Ix::INK), (0, -1, Ix::INK)]);
        }
        passes.push((0, 0, style.ink));
        for (dx, dy, ink) in passes {
            for (k, ch) in text.chars().enumerate() {
                let g = self.glyph(f, style.bold, ch);
                c.mask(x + k as i32 * adv + ix + dx, y + iy + dy, g.w, &g.mask, ink, 1);
            }
        }
        x + Self::measure(f, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fine(ch: char) -> Glyph {
        rasterise(strokes(ch).unwrap(), Face::Fine, false)
    }

    fn rows(g: &Glyph) -> Vec<i32> {
        (0..g.h).filter(|&y| (0..g.w).any(|x| g.at(x, y))).collect()
    }

    fn xor(a: &Glyph, b: &Glyph) -> usize {
        a.mask.iter().zip(&b.mask).filter(|(p, q)| p != q).count()
    }

    #[test]
    fn every_character_has_strokes_inside_the_lattice() {
        assert_eq!(chars().count(), 95 + EXTRAS.len());
        for ch in chars() {
            let segs = strokes(ch).unwrap_or_else(|| panic!("no glyph for {ch:?}"));
            for s in segs {
                assert!(s.0 <= 4 && s.2 <= 4 && s.1 <= 7 && s.3 <= 7, "{ch:?} leaves the box: {s:?}");
            }
        }
    }

    #[test]
    fn glyphs_are_pairwise_distinct() {
        let all: Vec<(char, Glyph)> = chars().map(|c| (c, fine(c))).collect();
        let mut close = Vec::new();
        for (i, (a, ga)) in all.iter().enumerate() {
            for (b, gb) in &all[i + 1..] {
                if xor(ga, gb) < 3 {
                    close.push(format!("{a:?} {b:?} by {}", xor(ga, gb)));
                }
            }
        }
        assert!(close.is_empty(), "too alike: {close:?}");
    }

    #[test]
    fn capitals_and_digits_touch_the_cap_line_and_the_baseline() {
        for ch in ('A'..='Z').chain('0'..='9') {
            let r = rows(&fine(ch));
            assert_eq!((r[0], *r.last().unwrap()), (0, 5), "{ch:?}");
        }
    }

    #[test]
    fn descenders_alone_reach_rows_six_and_seven() {
        for ch in chars() {
            let low = rows(&fine(ch)).last().is_some_and(|&y| y >= 6);
            assert_eq!(low, DESCENDERS.contains(&ch), "{ch:?}");
        }
    }

    #[test]
    fn the_confusable_sets_are_distinct() {
        for set in [&['l', 'I', '1', '|', '!', 'i'][..], &['O', '0'], &['S', '5', 'Z', '2', 'B', '8']] {
            for (i, &a) in set.iter().enumerate() {
                for &b in &set[i + 1..] {
                    assert!(xor(&fine(a), &fine(b)) >= 3, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn the_small_face_is_the_fine_strokes_at_twice_the_scale() {
        let font = Font::build();
        for ch in chars() {
            let (f, s) = (font.glyph(Face::Fine, false, ch), font.glyph(Face::Small, false, ch));
            assert_eq!((s.w, s.h), (2 * f.w, 2 * f.h));
            // Every Fine pixel's 2 x 2 block is inked in Small, and Small inks no block whose
            // Fine pixel and its neighbours are all blank.
            for y in 0..f.h {
                for x in 0..f.w {
                    let block = (0..4).any(|k| s.at(2 * x + k % 2, 2 * y + k / 2));
                    let near = (-1..=1).any(|dy| (-1..=1).any(|dx| f.at(x + dx, y + dy)));
                    assert!(!f.at(x, y) || block, "{ch:?} lost ({x}, {y}) at 2x");
                    assert!(!block || near, "{ch:?} grew ({x}, {y}) at 2x");
                }
            }
        }
    }

    #[test]
    fn wrap_breaks_on_spaces() {
        assert_eq!(Font::wrap(Face::Fine, "the lamp stops the train", 10 * 8), vec!["the lamp", "stops the", "train"]);
        assert_eq!(Font::wrap(Face::Fine, "  a  b ", 80), vec!["a  b"]);
        assert_eq!(Font::measure(Face::Small, "abc"), 36);
    }
}
