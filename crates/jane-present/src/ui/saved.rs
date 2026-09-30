//! The save card (PRESENTATION.md §3.2): the world written down, said plainly and in the corner.
//! A small plate slides up in the bottom-right: a book lies open, a quill's nib runs along its
//! right-hand page, the book closes, and a gold glint catches its clasp; "Saved" beside it in
//! gold. About two seconds, fading in and out. A save that failed says so in red and stays twice
//! as long. At a table the host's card says whose rest it was ("Saved by the teal coat"), and a
//! guest who rested sees her own.

use jane_art::font::Face;
use jane_art::palette::{Ix, Ramp, Tone};

use crate::ui::art::Mark;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui, line_h, text_w};
use crate::ui::style::{self, argb, fade};
use crate::view::{SavedCard, ViewBuffers};

/// Ticks the card fades in over, and out over at its end.
pub const FADE_IN: u32 = 12;
pub const FADE_OUT: u32 = 30;
/// The book is open and written in until this tick, closes over the next, and glints after.
const WRITE: u32 = 34;
const CLOSE: u32 = 14;
const GLINT: u32 = 22;

/// The card's plate on a canvas `(cw, ch)` for `card`: the bottom-right corner, clear of the bar.
pub fn rect(canvas: (i32, i32), card: &SavedCard) -> Rect {
    let (cw, ch) = canvas;
    let w = (text_w(Face::Small, &card.text) + 52).clamp(120, cw - 24);
    let h = 34;
    Rect::new(cw - w - 12, ch - h - 64, w, h)
}

/// Its coverage `age` ticks after it went up, 0 to 255.
pub fn alpha(card: &SavedCard, age: u32) -> u8 {
    let total = card.ticks();
    let a = if age < FADE_IN {
        age * 255 / FADE_IN
    } else if age + FADE_OUT >= total {
        total.saturating_sub(age) * 255 / FADE_OUT
    } else {
        255
    };
    a.min(255) as u8
}

/// Draws the card, if one is up.
pub fn draw(ui: &mut Ui, b: &ViewBuffers) {
    let Some(card) = &b.saved else { return };
    let age = b.tick.wrapping_sub(card.born);
    if age >= card.ticks() {
        return;
    }
    let a = alpha(card, age);
    // It rises a few px into place.
    let rise = (6 - age as i32 / 2).max(0);
    let r = rect(ui.canvas, card);
    let (x, y, w, h) = (i32::from(r.x), i32::from(r.y) + rise, i32::from(r.w), i32::from(r.h));
    // A small plate of the HUD's glass: a dark rim, the panel's fill, a lit inner edge, a gold
    // rule along its top.
    ui.fill(Rect::new(x, y, w, h), fade(argb(style::INK, 230), a));
    ui.fill(Rect::new(x + 1, y + 1, w - 2, h - 2), fade(argb(style::panel_top(), 236), a));
    ui.fill(Rect::new(x + 1, y + h / 2, w - 2, h / 2 - 1), fade(argb(style::panel_bottom(), 160), a));
    ui.fill(Rect::new(x + 2, y + 1, w - 4, 1), fade(argb(style::panel_lit(), 200), a));
    let rule = if card.ok { style::gold_deep() } else { style::bad() };
    ui.fill(Rect::new(x + 6, y - 1, w - 12, 1), fade(argb(rule, 255), a));
    let (bx, by) = (x + 10, y + 9);
    if card.ok {
        book(ui, (bx, by), age, a);
    } else {
        // A blot on the page: the ink ran.
        book(ui, (bx, by), 0, a);
        ui.fill(Rect::new(bx + 11, by + 5, 4, 4), fade(argb(Ramp::ClothRed.at(Tone::Mid), 255), a));
    }
    let ink = if card.ok { style::gold() } else { style::bad() };
    let ty = y + (h - line_h(Face::Small)) / 2 + 1;
    ui.text(x + 40, ty, &card.text, Ink::small(ink).shadow().alpha(a));
}

/// The book at `(x, y)`, 22 by 16, `t` ticks in: open and written in, closing, then shut with a
/// glint on its clasp.
fn book(ui: &mut Ui, (x, y): (i32, i32), t: u32, a: u8) {
    let px = |ui: &mut Ui, r: Rect, ix: Ix| ui.fill(r, fade(argb(ix, 255), a));
    let cover = Ramp::ClothNavy;
    let page = Ramp::ClothLinen;
    if t < WRITE + CLOSE {
        // The right page narrows as the cover comes over it.
        let shut = t.saturating_sub(WRITE).min(CLOSE) as i32;
        let right = 10 - shut * 10 / CLOSE as i32;
        // The boards under the pages, a spine between them.
        px(ui, Rect::new(x - 1, y + 1, 12 + right + 1, 14), cover.at(Tone::Deep));
        px(ui, Rect::new(x, y, 10, 13), page.at(Tone::Light));
        px(ui, Rect::new(x, y + 12, 10, 1), page.at(Tone::Mid));
        px(ui, Rect::new(x + 10, y, 1, 14), cover.at(Tone::Base));
        // Lines written on the left page.
        for (i, l) in [7, 8, 5, 8, 6].into_iter().enumerate() {
            px(ui, Rect::new(x + 1, y + 2 + i as i32 * 2, l, 1), Ramp::Slate.at(Tone::Mid));
        }
        if right > 0 {
            px(ui, Rect::new(x + 11, y, right, 13), page.at(Tone::High));
            px(ui, Rect::new(x + 11, y + 12, right, 1), page.at(Tone::Mid));
            // The quill writes the right page a line at a time while the book is open.
            let written = (t.min(WRITE) * 20 / WRITE) as i32;
            for i in 0..4 {
                let len = (written - i * 5).clamp(0, 5).min(right - 1);
                if len > 0 {
                    px(ui, Rect::new(x + 12, y + 2 + i * 2, len, 1), Ramp::Slate.at(Tone::Mid));
                }
            }
            if t < WRITE {
                // The nib at the end of the line, and the feather leaning up and away.
                let line = (written / 5).min(3);
                let nx = x + 12 + (written - line * 5).clamp(0, 5);
                let ny = y + 1 + line * 2;
                px(ui, Rect::new(nx, ny, 1, 1), style::INK);
                for k in 1..7 {
                    px(ui, Rect::new(nx + k, ny - k, 1, 1), Ramp::ClothCream.at(Tone::High));
                    px(ui, Rect::new(nx + k + 1, ny - k, 1, 1), Ramp::ClothCream.at(Tone::Mid));
                }
            }
        }
    } else {
        // Shut: the cover, its lit edge, the pages' edge showing, a brass clasp.
        px(ui, Rect::new(x, y + 1, 14, 14), cover.at(Tone::Base));
        px(ui, Rect::new(x, y + 1, 14, 1), cover.at(Tone::Light));
        px(ui, Rect::new(x, y + 1, 2, 14), cover.at(Tone::Shade));
        px(ui, Rect::new(x + 2, y + 14, 12, 1), page.at(Tone::Light));
        px(ui, Rect::new(x + 12, y + 7, 3, 3), Ramp::Brass.at(Tone::Light));
        let g = t - WRITE - CLOSE;
        if g < GLINT {
            // The glint: the star at its brightest halfway.
            let k = if g < GLINT / 2 { g } else { GLINT - g };
            let ga = (k * 255 / (GLINT / 2)).min(255) as u8;
            ui.mark(Mark::Star, x + 9, y + 3, ((u32::from(ga) * u32::from(a)) / 255) as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(ok: bool) -> SavedCard {
        SavedCard { text: "Saved".into(), ok, born: 0 }
    }

    #[test]
    fn it_fades_in_holds_and_fades_out_in_about_two_seconds() {
        let c = card(true);
        assert_eq!(alpha(&c, 0), 0);
        assert_eq!(alpha(&c, FADE_IN), 255);
        assert_eq!(alpha(&c, c.ticks() / 2), 255);
        assert!(alpha(&c, c.ticks() - 5) < 64);
        assert!((100..=150).contains(&c.ticks()), "about two seconds at 60 a second");
        assert!(card(false).ticks() > c.ticks(), "a failure stays to be read");
    }

    #[test]
    fn the_card_stays_in_the_corner_on_any_canvas() {
        for canvas in [(768, 432), (1008, 432), (640, 360)] {
            let c = SavedCard { text: "Saved by the ochre coat".into(), ok: true, born: 0 };
            let r = rect(canvas, &c);
            assert!(r.x >= 0 && r.right() <= canvas.0 && r.y >= 0 && r.bottom() <= canvas.1, "{r:?} on {canvas:?}");
        }
    }
}
