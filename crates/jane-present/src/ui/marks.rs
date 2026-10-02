//! The quest marks over heads (PRESENTATION.md §3.8): the one marker the world has. A small gold
//! glyph in the stroke font over whoever (or whatever: a book, a board, a door) has a quest to
//! give her, and over whoever takes one back once it is ready; a gentle bob; a soft glow round
//! it after dark.
//!
//! Which mark is the sim's word (`View::quest_mark`, per seat), gathered by the presenter with
//! where each head is ([`Present::marks`](crate::Present::marks)): so each machine at a table
//! shows its own, and none over someone she is talking to.

use jane_art::font::Face;
use jane_art::palette::{Ramp, Tone};
use jane_sim::view::QuestMark;

use crate::present::QuestMarker;
use crate::ui::cmd::Rect;
use crate::ui::core::{Ink, Ui, advance, line_h};
use crate::ui::style::{self, argb};

/// The glyph over someone with a quest to give her: WoW's way round, which players already read
/// (PLAY-PLAN.md D10): a "!" offers, a "?" takes back.
pub const OFFER_GLYPH: &str = "!";
/// The glyph over someone a ready quest goes back to.
pub const HAND_IN_GLYPH: &str = "?";

/// Ticks for one bob, up and down, and how many rows it rises.
pub const BOB_TICKS: u32 = 72;
pub const BOB_ROWS: i32 = 2;

/// The glyph for a mark.
pub fn glyph(m: QuestMark) -> &'static str {
    match m {
        QuestMark::Offer => OFFER_GLYPH,
        QuestMark::HandIn => HAND_IN_GLYPH,
    }
}

/// Rows the mark stands up at `tick`, 0 to `BOB_ROWS`: a slow triangle, each person on their own
/// phase so a crowd does not bob as one.
pub fn bob(tick: u32, id: u32) -> i32 {
    let t = (tick.wrapping_add(id.wrapping_mul(29))) % BOB_TICKS;
    let half = BOB_TICKS / 2;
    let up = if t < half { t } else { BOB_TICKS - t };
    (up as i32 * (2 * BOB_ROWS + 1) / BOB_TICKS as i32).min(BOB_ROWS)
}

/// The glyph's box over a head at canvas `(x, y)` (its foot), risen `bob` rows.
pub fn rect(m: &QuestMarker, bob: i32) -> Rect {
    let (w, h) = (advance(Face::Small), line_h(Face::Small));
    Rect::new(m.x - w / 2, m.y - h - bob, w, h)
}

/// Draws every mark: under the rest of the UI, over the world. `dark`: they glow.
pub fn draw(ui: &mut Ui, marks: &[QuestMarker], dark: bool, tick: u32) {
    for m in marks {
        let r = rect(m, bob(tick, m.id));
        if dark {
            let (cx, cy) = (i32::from(r.x) + i32::from(r.w) / 2, i32::from(r.y) + i32::from(r.h) / 2);
            glow(ui, (cx, cy), 10, argb(Ramp::UiGold.at(Tone::Light), 62));
            glow(ui, (cx, cy), 6, argb(Ramp::UiGold.at(Tone::High), 96));
        }
        match m.mark {
            QuestMark::Offer => offer(ui, i32::from(r.x), i32::from(r.y)),
            QuestMark::HandIn => {
                ui.text(i32::from(r.x), i32::from(r.y), glyph(m.mark), Ink::small(style::gold()).outline());
            }
        }
    }
}

/// The offer's "!", drawn rather than set: the font's is one 2 px stroke, where the "?" it stands
/// beside spans 10 px. WoW's weight instead: a bar 6 px wide that narrows to 4, a gap the ink
/// outline keeps dark, and a dot as wide as the bar's foot; lit down its left and shaded down
/// its right, outlined in ink like the "?". It fills the same 12 rows of the cell as the "?".
pub const OFFER_MASK: [&str; 12] = [
    ".####.", //
    "######", //
    "######", //
    "######", //
    "######", //
    ".####.", //
    ".####.", //
    ".####.", //
    "......", //
    "......", //
    ".####.", //
    ".####.", //
];
/// Where the mask sits in the glyph's cell: centred on the "?" (ink columns 1 to 10, rows 1 to 12).
const OFFER_AT: (i32, i32) = (3, 1);

/// Is `(x, y)` of the "!" inked?
pub fn offer_at(x: i32, y: i32) -> bool {
    usize::try_from(y)
        .ok()
        .and_then(|y| OFFER_MASK.get(y))
        .and_then(|row| usize::try_from(x).ok().and_then(|x| row.as_bytes().get(x)))
        .is_some_and(|&b| b == b'#')
}

/// Draws the "!" in the cell whose top-left is `(x, y)`: its ink outline, then the gold a row at
/// a time, in runs of one tone.
fn offer(ui: &mut Ui, x: i32, y: i32) {
    let (x, y) = (x + OFFER_AT.0, y + OFFER_AT.1);
    let rows = OFFER_MASK.len() as i32;
    let cols = OFFER_MASK[0].len() as i32;
    let ink = argb(style::INK, 255);
    for oy in -1..=rows {
        let mut ox = -1;
        while ox <= cols {
            let edge = |ox: i32| {
                !offer_at(ox, oy)
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| offer_at(ox + dx, oy + dy))
            };
            if edge(ox) {
                let start = ox;
                while ox <= cols && edge(ox) {
                    ox += 1;
                }
                ui.fill(Rect::new(x + start, y + oy, ox - start, 1), ink);
            } else {
                ox += 1;
            }
        }
    }
    for oy in 0..rows {
        let mut ox = 0;
        while ox < cols {
            let tone = |ox: i32| {
                if !offer_at(ox, oy) {
                    None
                } else if oy == 0 || !offer_at(ox - 1, oy) {
                    Some(Tone::High)
                } else if !offer_at(ox + 1, oy) || !offer_at(ox, oy + 1) {
                    Some(Tone::Base)
                } else {
                    Some(Tone::Light)
                }
            };
            match tone(ox) {
                None => ox += 1,
                Some(t) => {
                    let start = ox;
                    while ox < cols && tone(ox) == Some(t) {
                        ox += 1;
                    }
                    ui.fill(Rect::new(x + start, y + oy, ox - start, 1), argb(Ramp::UiGold.at(t), 255));
                }
            }
        }
    }
}

/// An emote over a head (ART-PLAN B3): a small white bubble with a mark in it, keyed on what
/// the person is saying or doing (`Present::emotes`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emote {
    /// Surprised, or saying so: a red "!".
    Bang,
    /// Asking, or puzzled: a "?".
    Ask,
    /// Lost for words, or thinking: three dots.
    Dots,
    /// Whistling at work, singing in the evening: a note.
    Note,
    /// Fond: a heart.
    Heart,
}

/// Ticks an emote shows, from its pop to gone.
pub const EMOTE_TICKS: u32 = 96;

/// An emote this frame: whose (its bob's phase), the head's top as a mark's, which, how old.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmoteMarker {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub emote: Emote,
    pub age: u32,
}

/// The bubble's inside, px; its ink line goes round it and its tail hangs under its left half.
const BUBBLE: (i32, i32) = (10, 8);

/// Each emote's mark, 8 x 6, inside the bubble a px in from its edge.
fn emote_mask(e: Emote) -> [&'static str; 6] {
    match e {
        Emote::Bang => ["...##...", "...##...", "...##...", "...##...", "........", "...##..."],
        Emote::Ask => ["..####..", ".##..##.", ".....##.", "...###..", "........", "...##..."],
        Emote::Dots => ["........", "........", "........", "##.##.##", "##.##.##", "........"],
        Emote::Note => ["....###.", "....#.##", "....#...", "..###...", ".####...", "..##...."],
        Emote::Heart => [".##..##.", "########", "########", ".######.", "..####..", "...##..."],
    }
}

/// The mark's ink: red for a surprise and a heart, ink for the rest.
fn emote_ink(e: Emote) -> (Ramp, Tone) {
    match e {
        Emote::Bang => (Ramp::ClothRed, Tone::Base),
        Emote::Heart => (Ramp::ClothRose, Tone::Base),
        Emote::Note => (Ramp::ClothNavy, Tone::Base),
        Emote::Ask | Emote::Dots => (Ramp::ClothNavy, Tone::Shade),
    }
}

/// Draws every emote: a pop up out of the head over its first ticks, a gentle bob, gone in a
/// fade over its last. Over the world, under the rest of the UI, as the marks.
pub fn draw_emotes(ui: &mut Ui, emotes: &[EmoteMarker], tick: u32) {
    for e in emotes {
        if e.age >= EMOTE_TICKS {
            continue;
        }
        // It rises out of the head over four ticks, then bobs as a mark does.
        let rise = (4 - e.age.min(4) as i32) * 2;
        let a = if e.age + 12 > EMOTE_TICKS { ((EMOTE_TICKS - e.age) * 21).min(255) as u8 } else { 255 };
        let (w, h) = (BUBBLE.0 + 2, BUBBLE.1 + 2);
        let x = e.x - w / 2;
        let y = e.y - h - 3 + rise - bob(tick, e.id) / 2;
        let ink = argb(style::INK, a);
        let paper = argb(Ramp::HairWhite.at(Tone::High), a);
        let shade = argb(Ramp::HairWhite.at(Tone::Base), a);
        // The ink line round the bubble, its corners cut.
        ui.fill(Rect::new(x + 1, y, w - 2, 1), ink);
        ui.fill(Rect::new(x + 1, y + h - 1, w - 2, 1), ink);
        ui.fill(Rect::new(x, y + 1, 1, h - 2), ink);
        ui.fill(Rect::new(x + w - 1, y + 1, 1, h - 2), ink);
        // The paper, a shade along its bottom and right.
        ui.fill(Rect::new(x + 1, y + 1, w - 2, h - 2), paper);
        ui.fill(Rect::new(x + 2, y + h - 2, w - 3, 1), shade);
        ui.fill(Rect::new(x + w - 2, y + 2, 1, h - 3), shade);
        // The tail: two rows down and to the left, toward the head.
        let tx = x + w / 2 - 2;
        ui.fill(Rect::new(tx, y + h - 1, 3, 1), paper);
        ui.fill(Rect::new(tx - 1, y + h - 1, 1, 2), ink);
        ui.fill(Rect::new(tx + 3, y + h - 1, 1, 1), ink);
        ui.fill(Rect::new(tx, y + h, 2, 1), paper);
        ui.fill(Rect::new(tx + 2, y + h, 1, 1), ink);
        ui.fill(Rect::new(tx - 1, y + h + 1, 2, 1), ink);
        // The mark, a row at a time in runs.
        let (ramp, tone) = emote_ink(e.emote);
        let c = argb(ramp.at(tone), a);
        let lit = argb(ramp.at(tone.step(1)), a);
        for (r, row) in emote_mask(e.emote).iter().enumerate() {
            let b = row.as_bytes();
            let mut k = 0;
            while k < b.len() {
                if b[k] != b'#' {
                    k += 1;
                    continue;
                }
                let start = k;
                while k < b.len() && b[k] == b'#' {
                    k += 1;
                }
                let colour = if r == 0 || start == 0 { lit } else { c };
                ui.fill(Rect::new(x + 2 + start as i32, y + 2 + r as i32, (k - start) as i32, 1), colour);
            }
        }
    }
}

/// A soft disc of `c` round `(cx, cy)`, radius `r`, a row at a time.
fn glow(ui: &mut Ui, (cx, cy): (i32, i32), r: i32, c: u32) {
    for dy in -r..=r {
        let w = isqrt(r * r - dy * dy);
        if w > 0 {
            ui.fill(Rect::new(cx - w, cy + dy, 2 * w + 1, 1), c);
        }
    }
}

fn isqrt(n: i32) -> i32 {
    let mut x = 0;
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wow_s_way_round_an_exclamation_offers_and_a_question_takes_back() {
        assert_eq!(glyph(QuestMark::Offer), "!");
        assert_eq!(glyph(QuestMark::HandIn), "?");
    }

    /// The "!" carries the "?"'s weight: as tall (12 rows), at least twice the font stroke's
    /// ink, wider over its top than its foot, and a dark gap between bar and dot.
    #[test]
    fn the_offer_is_as_bold_as_the_question() {
        let inked = |y: i32| (0..6).filter(|&x| offer_at(x, y)).count();
        let total: usize = (0..12).map(inked).sum();
        assert_eq!(OFFER_MASK.len(), 12);
        assert!(total >= 2 * 20, "{total} px against the font's 20");
        assert!(inked(1) > inked(7) && inked(7) >= 4, "tapers, and the foot is no stroke");
        assert!((0..12).any(|y| inked(y) == 0) && inked(11) > 0, "a gap, then the dot");
    }

    #[test]
    fn every_emote_has_a_mark_that_fits_its_bubble() {
        for e in [Emote::Bang, Emote::Ask, Emote::Dots, Emote::Note, Emote::Heart] {
            let m = emote_mask(e);
            assert!(m.iter().all(|r| r.len() as i32 == BUBBLE.0 - 2), "{e:?}");
            assert!(m.iter().any(|r| r.contains('#')), "{e:?}");
        }
    }

    #[test]
    fn a_bob_is_gentle_and_each_head_keeps_its_own_time() {
        let rows: Vec<i32> = (0..BOB_TICKS).map(|t| bob(t, 0)).collect();
        assert!(rows.iter().all(|&r| (0..=BOB_ROWS).contains(&r)));
        assert!(rows.windows(2).all(|w| (w[0] - w[1]).abs() <= 1), "never jumps: {rows:?}");
        assert_eq!(*rows.iter().max().unwrap(), BOB_ROWS);
        assert!((0..BOB_TICKS).any(|t| bob(t, 0) != bob(t, 1)), "two heads, two phases");
    }
}
