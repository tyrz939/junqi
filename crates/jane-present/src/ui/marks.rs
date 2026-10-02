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
    fn a_bob_is_gentle_and_each_head_keeps_its_own_time() {
        let rows: Vec<i32> = (0..BOB_TICKS).map(|t| bob(t, 0)).collect();
        assert!(rows.iter().all(|&r| (0..=BOB_ROWS).contains(&r)));
        assert!(rows.windows(2).all(|w| (w[0] - w[1]).abs() <= 1), "never jumps: {rows:?}");
        assert_eq!(*rows.iter().max().unwrap(), BOB_ROWS);
        assert!((0..BOB_TICKS).any(|t| bob(t, 0) != bob(t, 1)), "two heads, two phases");
    }
}
